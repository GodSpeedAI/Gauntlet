//! The model-facing affordance layer for delegated agents (F1): `agents.spawn`,
//! `agents.status`, `agents.collect`, `agents.cancel`, and `agents.list`.
//!
//! Every operation is a projection of [`DelegationBroker`]. The service holds no
//! authority of its own:
//!
//! - The caller is fixed when the service is built for one invocation. A caller
//!   can never name a different parent, because spawn takes no parent argument.
//! - Spawn is admitted by the authority gate on the CALLER's recorded facts, so
//!   a child works on the same unit with the same blast radius and authority.
//! - Ownership is a graph read. Only strict descendants of the caller are
//!   reachable; anything else is refused before the broker is touched.
//! - A child's result is an untrusted claim. Completion here is invocation
//!   completion, never settlement, and this module exposes no settlement path.
//!
//! Spawn order is frozen. Every refusal before step (5) has zero side effects.
//! The only side effect a later refusal can leave is a workspace, which is
//! cleaned up whenever the graph holds no node for the invocation.

use std::collections::HashMap;
use std::fmt;

use gauntlet_domain::{
    AdmissionKey, Authority, BlastRadius, Budget, BudgetDimension, BudgetError,
    DiagnosticAllowance, EpochSeconds, HarnessId, InvocationId, IterationNo, Limits, MicroUsd,
    PermissionSetRef, ResultContractPath, RoleId, RunId, RunnerProfileId, SandboxPolicyRef, UnitId,
    WorkspaceRef,
};
use gauntlet_ports::agent_runner::{CancelReason, Isolation};
use gauntlet_ports::invocation_graph::{
    GraphError, GraphLimits, InvocationGraph, InvocationNode, NodeStatus,
};
use gauntlet_ports::role_result::RoleResult;
use gauntlet_ports::workspace_provider::{
    WorkspaceProvider, WorkspaceProviderError, WorkspaceSubject,
};
use gauntlet_ports::{BudgetSlice, Deadline};

use super::DelegationBroker;
use super::broker::{DelegationOutcome, DelegationRefusal, DelegationRequest};
use super::context::{ContextItem, ContextPolicy, ContextRefusal, build_envelope};
use super::reconcile::{BrokerError, CollectOutcome};
use crate::fabric::selection::{Preference, Requirements};
use crate::govern::{AuthorityGate, GateFacts, GateRefusal, GovernDeps};

/// The contract name every delegated child writes its result document under.
/// Relative by construction, like every contract path.
const RESULT_CONTRACT_NAME: &str = "delegate-result-document";

/// The reason recorded when an owning invocation cancels its subtree.
const AGENT_CANCEL_REASON: &str = "cancelled by the owning invocation";

/// The iteration every delegated child is admitted under (1-based).
const CHILD_ITERATION: u32 = 1;

/// Parallel slots a delegated child may use. A child is one sequential worker.
const CHILD_PARALLEL: u32 = 1;

/// The recovery reserve a delegated child's budget keeps back, in percent.
const RECOVERY_RESERVE_PCT: u8 = 20;

/// A delegated child carries no diagnostic allowance of its own.
const NO_DIAGNOSTIC_TOKENS: u64 = 0;
const NO_DIAGNOSTIC_COST: u64 = 0;

/// The Crockford Base32 alphabet an invocation id is written in.
const CROCKFORD: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
/// An invocation id is 26 characters of 5 bits each (130 bits, top bits zero).
const ID_CHARS: u32 = 26;
const BITS_PER_CHAR: u32 = 5;
const CHAR_MASK: u128 = 0x1f;
/// Two independent FNV-1a lanes give 128 bits of derivation.
const LANE_SEED_A: u64 = 0xcbf2_9ce4_8422_2325;
const LANE_SEED_B: u64 = 0x8422_2325_cbf2_9ce4;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// Derives the invocation id for one (run, idempotency key) pair. The same pair
/// always derives the same id, so a retried spawn names the same node. The
/// derivation is a stable digest, never a random draw.
pub fn derive_invocation_id(run_id: &RunId, key: &AdmissionKey) -> InvocationId {
    let parts: [&[u8]; 2] = [run_id.as_str().as_bytes(), key.as_str().as_bytes()];
    let value = (u128::from(fnv1a_lane(LANE_SEED_A, &parts)) << 64)
        | u128::from(fnv1a_lane(LANE_SEED_B, &parts));
    let text: String = (0..ID_CHARS)
        .rev()
        .map(|index| {
            let digit = (value >> (index * BITS_PER_CHAR)) & CHAR_MASK;
            char::from(CROCKFORD[digit as usize])
        })
        .collect();
    InvocationId::new(&text).expect("the derived text is in the invocation id grammar")
}

/// One FNV-1a lane over length-prefixed parts, so no two part splits collide.
fn fnv1a_lane(seed: u64, parts: &[&[u8]]) -> u64 {
    let mut hash = seed;
    for part in parts {
        for byte in (part.len() as u64).to_le_bytes().iter().chain(part.iter()) {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(FNV_PRIME);
        }
    }
    hash
}

/// Where a role's delegated children may read from. Only `Fresh` is
/// implemented; `ReadOnlyOfCaller` is a typed refusal until it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspacePolicy {
    Fresh,
    ReadOnlyOfCaller,
}

/// What one role may do when it is the child of a delegation.
#[derive(Debug, Clone)]
pub struct RoleGrant {
    pub context: ContextPolicy,
    pub permissions: PermissionSetRef,
    pub sandbox_policy: SandboxPolicyRef,
    pub workspace: WorkspacePolicy,
    /// Whether an invocation of this role may spawn children of its own.
    pub may_spawn: bool,
}

/// The affordance policy shared by every service in one run.
///
/// A role absent from `roles` cannot be spawned, and a caller whose role is
/// absent cannot spawn either.
#[derive(Debug, Clone)]
pub struct AgentsPolicy {
    pub roles: HashMap<RoleId, RoleGrant>,
    pub limits: GraphLimits,
    /// The per-spawn budget ceiling, componentwise.
    pub max_budget: BudgetSlice,
    /// The runner isolation fact the authority gate consumes.
    pub isolation: Isolation,
}

/// The caller of one service. Fixed when the service is built for one
/// invocation; the caller of any spawn is the parent of what it spawns.
#[derive(Debug, Clone)]
pub struct AgentsCaller {
    pub run_id: RunId,
    pub invocation_id: InvocationId,
    pub role: RoleId,
    pub unit: UnitId,
    pub workspace: WorkspaceRef,
    /// The caller's recorded (blast radius, authority) pair.
    pub recorded: (BlastRadius, Authority),
}

/// The arguments of one spawn. There is deliberately no parent field.
#[derive(Debug, Clone)]
pub struct SpawnArgs {
    pub role: RoleId,
    pub items: Vec<ContextItem>,
    pub requirements: Requirements,
    pub preference: Preference,
    pub budget: BudgetSlice,
    pub idempotency_key: AdmissionKey,
    pub deadline: Deadline,
    pub at: EpochSeconds,
}

/// The wire-neutral facts of one spawn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpawnOutcome {
    pub invocation_id: InvocationId,
    pub depth: u32,
    pub runner_profile: RunnerProfileId,
    pub harness: HarnessId,
    /// `false` when the key replayed an invocation that was already launched.
    pub launched: bool,
}

/// A wire-neutral projection of one graph node.
#[derive(Debug, Clone, PartialEq)]
pub struct AgentView {
    pub invocation_id: InvocationId,
    pub parent: Option<InvocationId>,
    pub depth: u32,
    pub role: RoleId,
    pub runner_profile: RunnerProfileId,
    pub harness: HarnessId,
    pub status: NodeStatus,
    pub admitted_at: EpochSeconds,
    pub terminal_at: Option<EpochSeconds>,
}

impl AgentView {
    fn of(node: &InvocationNode) -> Self {
        Self {
            invocation_id: node.invocation_id.clone(),
            parent: node.parent.clone(),
            depth: node.depth,
            role: node.role.clone(),
            runner_profile: node.runner_profile.clone(),
            harness: node.harness.clone(),
            status: node.status,
            admitted_at: node.admitted_at,
            terminal_at: node.terminal_at,
        }
    }
}

/// What `collect` reports. A `Claimed` result is an untrusted claim: the child
/// completed its contract, which is not settlement.
#[derive(Debug, Clone, PartialEq)]
pub enum CollectView {
    StillRunning(AgentView),
    Claimed {
        agent: AgentView,
        claim: Box<RoleResult>,
    },
    Failed(AgentView),
    TimedOut(AgentView),
    Cancelled(AgentView),
    AlreadyTerminal(AgentView),
}

/// What `cancel` reports. Ids only, so runner error detail never leaks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CancelView {
    pub cancelled: Vec<InvocationId>,
    /// Cancelled nodes whose runner did not confirm the cancel.
    pub unconfirmed: Vec<InvocationId>,
}

/// Why an affordance call was refused. Every variant except `Delegation`,
/// `Broker`, `Graph`, and `Workspace` is raised before any side effect.
#[derive(Debug)]
pub enum AgentsRefusal {
    /// The caller's own role may not delegate, or is absent from the policy.
    SpawnNotPermitted { role: RoleId },
    /// The child role is absent from the policy.
    RoleNotGranted { role: RoleId },
    /// The child role's workspace policy is not implemented yet.
    WorkspacePolicyNotSupported { role: RoleId },
    /// The requested slice exceeds the policy ceiling in this dimension.
    BudgetAboveCeiling { dimension: BudgetDimension },
    /// The requested slice cannot form a runner budget.
    InvalidBudget(BudgetError),
    /// The authority gate refused the caller's recorded facts.
    Authority(GateRefusal),
    /// The items do not form an allowed envelope for the child role.
    Context(ContextRefusal),
    /// The workspace provider failed to create or read a workspace.
    Workspace(WorkspaceProviderError),
    /// The broker refused the delegation.
    Delegation(DelegationRefusal),
    /// The broker could not reconcile the target.
    Broker(BrokerError),
    /// The graph could not be read.
    Graph(GraphError),
    /// The target is not a strict descendant of the caller. Carries no detail,
    /// so a caller cannot probe ids outside its subtree.
    NotYourSubtree,
}

impl fmt::Display for AgentsRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SpawnNotPermitted { role } => {
                write!(f, "role {role} may not spawn delegated work")
            }
            Self::RoleNotGranted { role } => write!(f, "role {role} is not granted to spawn"),
            Self::WorkspacePolicyNotSupported { role } => {
                write!(
                    f,
                    "role {role} asks for a workspace policy that is not supported yet"
                )
            }
            Self::BudgetAboveCeiling { dimension } => {
                write!(f, "the requested slice exceeds the ceiling in {dimension}")
            }
            Self::InvalidBudget(error) => write!(f, "the requested slice is not a budget: {error}"),
            Self::Authority(refusal) => write!(f, "authority refused: {}", refusal.summary()),
            Self::Context(refusal) => write!(f, "context refused: {refusal}"),
            Self::Workspace(error) => write!(f, "workspace refused: {error}"),
            Self::Delegation(refusal) => write!(f, "delegation refused: {refusal}"),
            Self::Broker(error) => write!(f, "reconciliation failed: {error}"),
            Self::Graph(error) => write!(f, "invocation graph failed: {error}"),
            Self::NotYourSubtree => f.write_str("the invocation is not in your subtree"),
        }
    }
}

impl std::error::Error for AgentsRefusal {}

/// The affordance service for one caller. Borrowed ports only.
pub struct AgentsService<'a> {
    caller: AgentsCaller,
    policy: &'a AgentsPolicy,
    broker: &'a DelegationBroker<'a>,
    graph: &'a dyn InvocationGraph,
    gate: &'a AuthorityGate,
    govern: &'a GovernDeps,
    workspaces: &'a dyn WorkspaceProvider,
}

impl<'a> AgentsService<'a> {
    /// Builds the service for one invocation. `graph` must be the graph the
    /// broker reads, or ownership and admission would disagree.
    pub fn new(
        caller: AgentsCaller,
        policy: &'a AgentsPolicy,
        broker: &'a DelegationBroker<'a>,
        graph: &'a dyn InvocationGraph,
        gate: &'a AuthorityGate,
        govern: &'a GovernDeps,
        workspaces: &'a dyn WorkspaceProvider,
    ) -> Self {
        Self {
            caller,
            policy,
            broker,
            graph,
            gate,
            govern,
            workspaces,
        }
    }

    /// Spawns one child of the caller. Frozen order:
    ///
    /// 1. the caller's grant may spawn, and the child's grant exists and is
    ///    implemented;
    /// 2. the slice fits the ceiling;
    /// 3. the gate admits the caller's recorded facts, now;
    /// 4. the invocation id is derived from (run, key);
    /// 5. the envelope is checked, then a workspace is created only when the
    ///    graph holds no node for the id;
    /// 6. the broker delegates, and a refusal that left no node cleans up the
    ///    workspace.
    pub fn spawn(&self, args: SpawnArgs) -> Result<SpawnOutcome, AgentsRefusal> {
        // (1) A role that may not delegate cannot, whatever it asks for.
        match self.policy.roles.get(&self.caller.role) {
            Some(grant) if grant.may_spawn => {}
            _ => {
                return Err(AgentsRefusal::SpawnNotPermitted {
                    role: self.caller.role.clone(),
                });
            }
        }
        let grant =
            self.policy
                .roles
                .get(&args.role)
                .ok_or_else(|| AgentsRefusal::RoleNotGranted {
                    role: args.role.clone(),
                })?;
        if grant.workspace != WorkspacePolicy::Fresh {
            return Err(AgentsRefusal::WorkspacePolicyNotSupported {
                role: args.role.clone(),
            });
        }

        // (2) The slice may not exceed the per-spawn ceiling.
        if let Some(dimension) = above_ceiling(args.budget, self.policy.max_budget) {
            return Err(AgentsRefusal::BudgetAboveCeiling { dimension });
        }

        // (3) The child shares the caller's unit, tier, and authority, so the
        // gate decides on the caller's recorded facts, freshly, at execution.
        let facts = GateFacts {
            unit: self.caller.unit.clone(),
            tier: self.caller.recorded.0,
            authority: self.caller.recorded.1,
            runner_isolation: self.policy.isolation,
            workspace: self.caller.workspace.clone(),
        };
        self.gate
            .admit(
                self.govern,
                &facts,
                Some(self.caller.recorded),
                args.deadline,
            )
            .map_err(AgentsRefusal::Authority)?;

        // (4) The same (run, key) always names the same invocation.
        let invocation_id = derive_invocation_id(&self.caller.run_id, &args.idempotency_key);

        // (5) Check the envelope before any workspace exists, so a context
        // refusal leaves nothing behind.
        build_envelope(&grant.context, &args.items).map_err(AgentsRefusal::Context)?;
        let existing = self
            .graph
            .node(&invocation_id, args.deadline)
            .map_err(AgentsRefusal::Graph)?;
        let (workspace, created) = match existing {
            Some(node) => (node.workspace, None),
            None => {
                let allocated = self
                    .workspaces
                    .create(
                        &self.caller.run_id,
                        WorkspaceSubject::Unit(self.caller.unit.clone()),
                        args.deadline,
                    )
                    .map_err(AgentsRefusal::Workspace)?;
                (allocated.reference.clone(), Some(allocated.reference))
            }
        };

        // (6) Delegate. The child's parent is the caller, always.
        let request = DelegationRequest {
            run_id: self.caller.run_id.clone(),
            parent: Some(self.caller.invocation_id.clone()),
            role: args.role.clone(),
            items: args.items,
            policy: grant.context.clone(),
            requirements: args.requirements,
            preference: args.preference,
            workspace,
            permissions: grant.permissions.clone(),
            sandbox_policy: grant.sandbox_policy.clone(),
            budget: args.budget,
            spec_budget: child_budget(args.budget, self.policy.limits.max_depth)?,
            limits: self.policy.limits,
            admission_key: args.idempotency_key,
            invocation_id: invocation_id.clone(),
            result_contract: ResultContractPath::new(RESULT_CONTRACT_NAME)
                .expect("the result contract name is a relative path constant"),
            deadline: args.deadline,
            iteration_no: IterationNo::new(CHILD_ITERATION).expect("iterations are 1-based"),
            at: args.at,
        };
        match self.broker.delegate(request) {
            Ok(delegated) => Ok(SpawnOutcome {
                invocation_id,
                depth: delegated.node.depth,
                runner_profile: delegated.node.runner_profile,
                harness: delegated.node.harness,
                launched: delegated.outcome == DelegationOutcome::Launched,
            }),
            Err(refusal) => {
                if let Some(reference) = created {
                    // No node exists, so nothing can ever use this workspace.
                    if matches!(self.graph.node(&invocation_id, args.deadline), Ok(None)) {
                        let _ = self.workspaces.cleanup(&reference, args.deadline);
                    }
                }
                Err(AgentsRefusal::Delegation(refusal))
            }
        }
    }

    /// One strict descendant's graph state. Polling never changes graph status.
    pub fn status(
        &self,
        id: &InvocationId,
        deadline: Deadline,
    ) -> Result<AgentView, AgentsRefusal> {
        self.owns(id, deadline)?;
        let status = self
            .broker
            .status(id, deadline)
            .map_err(AgentsRefusal::Broker)?;
        Ok(AgentView::of(&status.node))
    }

    /// Polls one strict descendant. A completed claim is returned untrusted.
    pub fn collect(
        &self,
        id: &InvocationId,
        at: EpochSeconds,
        deadline: Deadline,
    ) -> Result<CollectView, AgentsRefusal> {
        self.owns(id, deadline)?;
        let outcome = self
            .broker
            .collect(id, at, deadline)
            .map_err(AgentsRefusal::Broker)?;
        Ok(match outcome {
            CollectOutcome::StillRunning(node) => CollectView::StillRunning(AgentView::of(&node)),
            CollectOutcome::Completed { node, result } => CollectView::Claimed {
                agent: AgentView::of(&node),
                claim: result,
            },
            CollectOutcome::Failed(node) => CollectView::Failed(AgentView::of(&node)),
            CollectOutcome::TimedOut(node) => CollectView::TimedOut(AgentView::of(&node)),
            CollectOutcome::Cancelled(node) => CollectView::Cancelled(AgentView::of(&node)),
            CollectOutcome::AlreadyTerminal(node) => {
                CollectView::AlreadyTerminal(AgentView::of(&node))
            }
        })
    }

    /// Cancels one strict descendant and its subtree, leaf-first at the runner.
    pub fn cancel(
        &self,
        id: &InvocationId,
        at: EpochSeconds,
        deadline: Deadline,
    ) -> Result<CancelView, AgentsRefusal> {
        self.owns(id, deadline)?;
        let reason = CancelReason::new(AGENT_CANCEL_REASON)
            .expect("the agent cancel reason is a non-empty constant");
        let report = self
            .broker
            .cancel(id, reason, at, deadline)
            .map_err(AgentsRefusal::Broker)?;
        Ok(CancelView {
            cancelled: report.cancelled,
            unconfirmed: report.runner_errors.into_iter().map(|(id, _)| id).collect(),
        })
    }

    /// The caller's direct children, in admission order.
    pub fn list(&self, deadline: Deadline) -> Result<Vec<AgentView>, AgentsRefusal> {
        let children = self
            .graph
            .children(&self.caller.invocation_id, deadline)
            .map_err(AgentsRefusal::Graph)?;
        Ok(children.iter().map(AgentView::of).collect())
    }

    /// Ownership: the target must be a strict descendant of the caller. Self,
    /// ancestors, siblings, and unknown ids are all the same refusal.
    fn owns(&self, id: &InvocationId, deadline: Deadline) -> Result<(), AgentsRefusal> {
        let descendants = self
            .graph
            .descendants(&self.caller.invocation_id, deadline)
            .map_err(AgentsRefusal::Graph)?;
        if descendants.iter().any(|node| &node.invocation_id == id) {
            Ok(())
        } else {
            Err(AgentsRefusal::NotYourSubtree)
        }
    }
}

/// The first dimension in which `slice` exceeds `ceiling`, in declaration order.
fn above_ceiling(slice: BudgetSlice, ceiling: BudgetSlice) -> Option<BudgetDimension> {
    if slice.rounds > ceiling.rounds {
        Some(BudgetDimension::Rounds)
    } else if slice.wall_seconds > ceiling.wall_seconds {
        Some(BudgetDimension::WallSeconds)
    } else if slice.tokens > ceiling.tokens {
        Some(BudgetDimension::Tokens)
    } else if slice.cost_usd.get() > ceiling.cost_usd.get() {
        Some(BudgetDimension::CostUsd)
    } else {
        None
    }
}

/// The runner-facing budget object for one child, built from its slice.
fn child_budget(slice: BudgetSlice, max_depth: u32) -> Result<Budget, AgentsRefusal> {
    let limits = Limits::new(
        slice.rounds,
        CHILD_PARALLEL,
        max_depth,
        slice.wall_seconds,
        slice.tokens,
        slice.cost_usd,
    )
    .map_err(AgentsRefusal::InvalidBudget)?;
    let diagnostic =
        DiagnosticAllowance::new(NO_DIAGNOSTIC_TOKENS, MicroUsd::new(NO_DIAGNOSTIC_COST));
    Budget::new(limits, RECOVERY_RESERVE_PCT, diagnostic).map_err(AgentsRefusal::InvalidBudget)
}
