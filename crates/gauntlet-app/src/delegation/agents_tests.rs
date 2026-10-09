//! Deterministic tests for the agents affordance layer. Every operation is a
//! projection of the DelegationBroker, so these tests drive the broker's own
//! scripted runner and reference graph, plus a counting workspace provider.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeSet, HashMap};
use std::fmt::Debug;
use std::path::PathBuf;
use std::sync::Arc;

use gauntlet_domain::{
    AdmissionKey, Authority, BlastRadius, BudgetDimension, EpochSeconds, InvocationId,
    PermissionSetRef, RoleId, RunId, SandboxPolicyRef, UnitId, WorkspaceRef,
};
use gauntlet_ports::agent_runner::Isolation;
use gauntlet_ports::invocation_graph::{
    AdmissionRefusal, GraphLimits, InvocationGraph, InvocationNode, NodeStatus,
};
use gauntlet_ports::workspace_provider::{
    Workspace, WorkspaceHook, WorkspaceProvider, WorkspaceProviderError, WorkspaceSubject,
};
use gauntlet_ports::{BudgetSlice, Deadline};

use super::DelegationBroker;
use super::agents::{
    AgentsCaller, AgentsPolicy, AgentsRefusal, AgentsService, CollectView, RoleGrant, SpawnArgs,
    SpawnOutcome, WorkspacePolicy,
};
use super::broker::{DelegationOutcome, DelegationRefusal};
use super::context::{ContextItem, ContextItemKind, ContextPolicy, ContextRefusal};
use super::test_support::*;
use crate::fabric::selection::{Preference, Requirements};
use crate::govern::{AuthorityGate, GateRefusal, GovernDeps};

const UNIT_TAG: &str = "UN-001";
const ROOT_WORKSPACE: &str = "workspace-a";
const PIN_PROFILE: &str = "profile-a";
const KEY_THIRD: &str = "key-third";
const KEY_GREAT: &str = "key-great";
const OVER_TOKENS: u64 = 1;
const GRANTED_KINDS: [ContextItemKind; 4] = [
    ContextItemKind::TaskStatement,
    ContextItemKind::ReferenceExcerpt,
    ContextItemKind::ConstraintStatement,
    ContextItemKind::ResultSchema,
];

/// A counting workspace provider. It hands out a fresh ref per create and
/// records every cleanup.
#[derive(Default)]
struct CountingWorkspaces {
    created: Cell<u32>,
    cleaned: RefCell<Vec<WorkspaceRef>>,
}

impl WorkspaceProvider for CountingWorkspaces {
    fn create(
        &self,
        _run_id: &RunId,
        _subject: WorkspaceSubject,
        _deadline: Deadline,
    ) -> Result<Workspace, WorkspaceProviderError> {
        let index = self.created.get() + 1;
        self.created.set(index);
        let reference = WorkspaceRef::new(&format!("workspace-child-{index}"))
            .expect("the workspace ref is non-empty");
        Ok(Workspace {
            reference,
            root: PathBuf::new(),
            key: format!("key-{index}"),
        })
    }

    fn cleanup(
        &self,
        workspace: &WorkspaceRef,
        _deadline: Deadline,
    ) -> Result<(), WorkspaceProviderError> {
        self.cleaned.borrow_mut().push(workspace.clone());
        Ok(())
    }

    fn hooks(&self) -> Vec<WorkspaceHook> {
        Vec::new()
    }
}

/// Everything one test wires together: the broker's lab (graph, registry, and
/// the scripted runner `a`), the policy, the gate, and the workspace counter.
struct Env {
    lab: Lab,
    a: Arc<ScriptedRunner>,
    policy: AgentsPolicy,
    gate: AuthorityGate,
    govern: GovernDeps,
    workspaces: CountingWorkspaces,
}

impl Env {
    fn new() -> Self {
        let (lab, a, _b, _c) = standard_lab();
        Self {
            lab,
            a,
            policy: policy(),
            gate: AuthorityGate::new(),
            govern: GovernDeps::none_backend(),
            workspaces: CountingWorkspaces::default(),
        }
    }

    /// Builds the service for one caller over this env's ports.
    fn service<'a>(
        &'a self,
        broker: &'a DelegationBroker<'a>,
        caller: AgentsCaller,
    ) -> AgentsService<'a> {
        AgentsService::new(
            caller,
            &self.policy,
            broker,
            &self.lab.graph,
            &self.gate,
            &self.govern,
            &self.workspaces,
        )
    }

    fn node(&self, id: &InvocationId) -> InvocationNode {
        self.lab
            .graph
            .node(id, deadline())
            .expect("the graph reads")
            .expect("the node exists")
    }

    fn effects(&self) -> Effects {
        Effects {
            live_workspaces: self.workspaces.created.get() as usize
                - self.workspaces.cleaned.borrow().len(),
            invokes: self.a.invokes(),
            active: self.lab.active_count(),
        }
    }
}

/// The observable effects a refusal must leave unchanged.
#[derive(Debug, PartialEq, Eq)]
struct Effects {
    live_workspaces: usize,
    invokes: u32,
    active: usize,
}

fn policy() -> AgentsPolicy {
    let mut roles = HashMap::new();
    roles.insert(RoleId::Orchestrator, grant(true));
    roles.insert(RoleId::Builder, grant(true));
    roles.insert(RoleId::Critic, grant(false));
    AgentsPolicy {
        roles,
        limits: GraphLimits {
            max_depth: MAX_DEPTH,
            max_children_per_parent: MAX_CHILDREN,
        },
        max_budget: root_slice(),
        isolation: Isolation::Process,
    }
}

fn grant(may_spawn: bool) -> RoleGrant {
    RoleGrant {
        context: ContextPolicy {
            allow: GRANTED_KINDS.into_iter().collect::<BTreeSet<_>>(),
            deny: BTreeSet::new(),
        },
        permissions: PermissionSetRef::new("perm-builder").expect("valid permissions"),
        sandbox_policy: SandboxPolicyRef::new("sandbox-readonly").expect("valid sandbox"),
        workspace: WorkspacePolicy::Fresh,
        may_spawn,
    }
}

/// A root invocation launched through the broker, owned by the orchestrator.
fn rooted() -> Env {
    let env = Env::new();
    let root = env
        .lab
        .broker()
        .delegate(pinned(
            root_request(ROOT_TAG, KEY_ROOT, items_for(ROOT_MARKER)),
            PIN_PROFILE,
        ))
        .expect("the root launches");
    assert_eq!(root.outcome, DelegationOutcome::Launched);
    env
}

fn unit() -> UnitId {
    UnitId::new(UNIT_TAG).expect("valid unit id")
}

fn root_caller() -> AgentsCaller {
    AgentsCaller {
        run_id: run_id(),
        invocation_id: inv(ROOT_TAG),
        role: RoleId::Orchestrator,
        unit: unit(),
        workspace: WorkspaceRef::new(ROOT_WORKSPACE).expect("valid workspace"),
        recorded: (BlastRadius::Contained, Authority::Critic),
    }
}

/// The caller for an existing node, built from that node's own facts.
fn caller_for(env: &Env, id: &InvocationId, role: RoleId) -> AgentsCaller {
    AgentsCaller {
        run_id: run_id(),
        invocation_id: id.clone(),
        role,
        unit: unit(),
        workspace: env.node(id).workspace,
        recorded: (BlastRadius::Contained, Authority::Critic),
    }
}

fn pin_preference() -> Preference {
    Preference {
        pin: Some(profile_id(PIN_PROFILE)),
        ..Preference::default()
    }
}

fn args_with(role: RoleId, key: &str, items: Vec<ContextItem>, budget: BudgetSlice) -> SpawnArgs {
    SpawnArgs {
        role,
        items,
        requirements: Requirements::default(),
        preference: pin_preference(),
        budget,
        idempotency_key: AdmissionKey::new(key).expect("valid admission key"),
        deadline: deadline(),
        at: EpochSeconds::new(ADMIT_AT),
    }
}

fn args(role: RoleId, key: &str, marker: &str) -> SpawnArgs {
    args_with(role, key, items_for(marker), child_slice())
}

fn refused<T: Debug>(result: Result<T, AgentsRefusal>) -> AgentsRefusal {
    match result {
        Err(refusal) => refusal,
        Ok(value) => panic!("expected a refusal, got {value:?}"),
    }
}

fn spawned(result: Result<SpawnOutcome, AgentsRefusal>) -> SpawnOutcome {
    result.expect("the spawn is admitted")
}

#[test]
fn agents_spawn_binds_the_child_to_its_caller() {
    let env = rooted();
    let broker = env.lab.broker();
    let root = env.service(&broker, root_caller());

    let child = spawned(root.spawn(args(RoleId::Builder, KEY_CHILD, CHILD_MARKER)));

    assert!(child.launched);
    assert_eq!(child.depth, 1);
    assert_eq!(child.runner_profile, profile_id(PIN_PROFILE));
    let node = env.node(&child.invocation_id);
    assert_eq!(node.parent, Some(inv(ROOT_TAG)));
    assert_eq!(node.role, RoleId::Builder);
    assert_eq!(node.status, NodeStatus::Running);
    assert_eq!(node.workspace.as_str(), "workspace-child-1");
    assert_eq!(env.workspaces.created.get(), 1);
    assert_eq!(env.a.invokes(), 2);
}

#[test]
fn agents_spawn_refuses_a_role_absent_from_the_grant_map_with_zero_effects() {
    let env = rooted();
    let broker = env.lab.broker();
    let root = env.service(&broker, root_caller());
    let before = env.effects();

    let refusal = refused(root.spawn(args(RoleId::Verifier, KEY_CHILD, CHILD_MARKER)));

    assert!(matches!(refusal, AgentsRefusal::RoleNotGranted { .. }));
    assert_eq!(env.effects(), before);
    assert_eq!(env.workspaces.created.get(), 0);
}

#[test]
fn agents_spawn_refuses_a_caller_whose_role_may_not_delegate() {
    let env = rooted();
    let broker = env.lab.broker();
    let critic = AgentsCaller {
        role: RoleId::Critic,
        ..root_caller()
    };
    let before = env.effects();

    let refusal = refused(env.service(&broker, critic).spawn(args(
        RoleId::Builder,
        KEY_CHILD,
        CHILD_MARKER,
    )));

    assert!(matches!(refusal, AgentsRefusal::SpawnNotPermitted { .. }));
    assert_eq!(env.effects(), before);
}

#[test]
fn agents_spawn_refuses_a_slice_above_the_policy_ceiling_with_zero_effects() {
    let env = rooted();
    let broker = env.lab.broker();
    let root = env.service(&broker, root_caller());
    let before = env.effects();
    let over = BudgetSlice {
        tokens: ROOT_TOKENS + OVER_TOKENS,
        ..child_slice()
    };

    let refusal = refused(root.spawn(args_with(
        RoleId::Builder,
        KEY_CHILD,
        items_for(CHILD_MARKER),
        over,
    )));

    assert!(matches!(
        refusal,
        AgentsRefusal::BudgetAboveCeiling {
            dimension: BudgetDimension::Tokens
        }
    ));
    assert_eq!(env.effects(), before);
}

#[test]
fn agents_spawn_refuses_an_irreversible_unit_with_zero_effects() {
    let env = rooted();
    let broker = env.lab.broker();
    let caller = AgentsCaller {
        recorded: (BlastRadius::Irreversible, Authority::Critic),
        ..root_caller()
    };
    let before = env.effects();

    let refusal = refused(env.service(&broker, caller).spawn(args(
        RoleId::Builder,
        KEY_CHILD,
        CHILD_MARKER,
    )));

    assert!(matches!(
        refusal,
        AgentsRefusal::Authority(GateRefusal::Irreversible { .. })
    ));
    assert_eq!(env.effects(), before);
}

#[test]
fn agents_spawn_refuses_authority_above_the_run_ceiling_with_zero_effects() {
    let env = rooted();
    let broker = env.lab.broker();
    let caller = AgentsCaller {
        recorded: (BlastRadius::Contained, Authority::LeadAgent),
        ..root_caller()
    };
    let before = env.effects();

    let refusal = refused(env.service(&broker, caller).spawn(args(
        RoleId::Builder,
        KEY_CHILD,
        CHILD_MARKER,
    )));

    assert!(matches!(
        refusal,
        AgentsRefusal::Authority(GateRefusal::AuthorityAboveCeiling { .. })
    ));
    assert_eq!(env.effects(), before);
}

#[test]
fn agents_spawn_gate_judges_the_callers_recorded_tier() {
    // The gate decides on the caller's recorded tier. A tier above what the
    // conservative posture dispatches is refused before any side effect.
    let env = rooted();
    let broker = env.lab.broker();
    let caller = AgentsCaller {
        recorded: (BlastRadius::Local, Authority::Critic),
        ..root_caller()
    };
    let before = env.effects();

    let refusal = refused(env.service(&broker, caller).spawn(args(
        RoleId::Builder,
        KEY_CHILD,
        CHILD_MARKER,
    )));

    assert!(matches!(
        refusal,
        AgentsRefusal::Authority(GateRefusal::NoBackendForTier { .. })
    ));
    assert_eq!(env.effects(), before);
}

#[test]
fn agents_spawn_retry_with_the_same_key_reuses_the_node_and_workspace() {
    let env = rooted();
    let broker = env.lab.broker();
    let root = env.service(&broker, root_caller());

    let first = spawned(root.spawn(args(RoleId::Builder, KEY_CHILD, CHILD_MARKER)));
    let retry = spawned(root.spawn(args(RoleId::Builder, KEY_CHILD, CHILD_MARKER)));

    assert_eq!(retry.invocation_id, first.invocation_id);
    assert!(first.launched);
    assert!(!retry.launched, "the replay names an existing launch");
    assert_eq!(env.workspaces.created.get(), 1, "one workspace for the key");
    assert_eq!(env.a.invokes(), 2, "one runner invoke for the child");
}

#[test]
fn agents_spawn_child_limit_refusal_cleans_up_the_workspace() {
    let env = rooted();
    let broker = env.lab.broker();
    let root = env.service(&broker, root_caller());
    spawned(root.spawn(args(RoleId::Builder, KEY_CHILD, CHILD_MARKER)));
    spawned(root.spawn(args(RoleId::Builder, KEY_SECOND, OTHER_REF_TEXT)));
    let before = env.effects();

    let refusal = refused(root.spawn(args(RoleId::Builder, KEY_THIRD, CHILD_MARKER)));

    assert!(matches!(
        refusal,
        AgentsRefusal::Delegation(DelegationRefusal::Admission(
            AdmissionRefusal::ChildLimit { .. }
        ))
    ));
    assert_eq!(
        env.workspaces.created.get(),
        3,
        "the third spawn allocated one"
    );
    assert_eq!(
        env.workspaces.cleaned.borrow().as_slice(),
        [WorkspaceRef::new("workspace-child-3").expect("valid workspace")],
        "and the refusal cleaned it up"
    );
    assert_eq!(env.effects(), before, "the graph is unchanged");
    assert_eq!(env.lab.descendant_count(ROOT_TAG), 2);
}

#[test]
fn agents_spawn_fresh_context_carries_items_only() {
    let env = rooted();
    let broker = env.lab.broker();
    let root = env.service(&broker, root_caller());

    spawned(root.spawn(args(RoleId::Builder, KEY_CHILD, CHILD_MARKER)));

    let body = env.a.last_spec().context_envelope.body().to_owned();
    assert_eq!(
        decode(&body),
        vec![
            triple(
                "task-statement",
                "task",
                &format!("task for {CHILD_MARKER}")
            ),
            triple("reference-excerpt", "notes", CHILD_MARKER),
        ],
        "the child receives exactly the items the caller passed"
    );
}

#[test]
fn agents_spawn_refuses_a_denied_kind_with_zero_effects() {
    let env = rooted();
    let broker = env.lab.broker();
    let root = env.service(&broker, root_caller());
    let before = env.effects();
    let items = vec![
        task("review the change"),
        item(ContextItemKind::FileExcerpt, "file", "fn main() {}"),
    ];

    let refusal = refused(root.spawn(args_with(RoleId::Builder, KEY_CHILD, items, child_slice())));

    assert!(matches!(
        refusal,
        AgentsRefusal::Context(ContextRefusal::KindNotAllowed {
            kind: ContextItemKind::FileExcerpt
        })
    ));
    assert_eq!(env.effects(), before);
    assert_eq!(env.workspaces.created.get(), 0);
}

#[test]
fn agents_spawn_refuses_a_workspace_policy_not_yet_implemented() {
    let mut env = rooted();
    env.policy.roles.insert(
        RoleId::Observer,
        RoleGrant {
            workspace: WorkspacePolicy::ReadOnlyOfCaller,
            ..grant(false)
        },
    );
    let broker = env.lab.broker();
    let root = env.service(&broker, root_caller());
    let before = env.effects();

    let refusal = refused(root.spawn(args(RoleId::Observer, KEY_CHILD, CHILD_MARKER)));

    assert!(matches!(
        refusal,
        AgentsRefusal::WorkspacePolicyNotSupported { .. }
    ));
    assert_eq!(env.effects(), before);
}

#[test]
fn agents_ownership_refuses_self_ancestor_sibling_and_unknown_without_broker_calls() {
    let env = rooted();
    let broker = env.lab.broker();
    let root = env.service(&broker, root_caller());
    let child = spawned(root.spawn(args(RoleId::Builder, KEY_CHILD, CHILD_MARKER))).invocation_id;
    let sibling =
        spawned(root.spawn(args(RoleId::Builder, KEY_SECOND, OTHER_REF_TEXT))).invocation_id;
    let child_svc = env.service(&broker, caller_for(&env, &child, RoleId::Builder));
    let before = (env.a.polls(), env.a.collects(), env.a.cancels());

    for target in [inv(ROOT_TAG), child.clone(), sibling, inv(UNKNOWN_TAG)] {
        assert!(matches!(
            refused(child_svc.status(&target, deadline())),
            AgentsRefusal::NotYourSubtree
        ));
        assert!(matches!(
            refused(child_svc.collect(&target, at(), deadline())),
            AgentsRefusal::NotYourSubtree
        ));
        assert!(matches!(
            refused(child_svc.cancel(&target, at(), deadline())),
            AgentsRefusal::NotYourSubtree
        ));
    }
    assert_eq!(before, (env.a.polls(), env.a.collects(), env.a.cancels()));

    // The root cannot reach itself either.
    assert!(matches!(
        refused(root.status(&inv(ROOT_TAG), deadline())),
        AgentsRefusal::NotYourSubtree
    ));
    assert!(matches!(
        refused(root.collect(&inv(ROOT_TAG), at(), deadline())),
        AgentsRefusal::NotYourSubtree
    ));
    assert!(matches!(
        refused(root.cancel(&inv(ROOT_TAG), at(), deadline())),
        AgentsRefusal::NotYourSubtree
    ));
    assert_eq!(before, (env.a.polls(), env.a.collects(), env.a.cancels()));
    assert_eq!(env.node(&inv(ROOT_TAG)).status, NodeStatus::Running);
}

#[test]
fn agents_ownership_reaches_children_and_grandchildren() {
    let env = rooted();
    let broker = env.lab.broker();
    let root = env.service(&broker, root_caller());
    let child = spawned(root.spawn(args(RoleId::Builder, KEY_CHILD, CHILD_MARKER))).invocation_id;
    let child_svc = env.service(&broker, caller_for(&env, &child, RoleId::Builder));
    let grand =
        spawned(child_svc.spawn(args(RoleId::Builder, KEY_GRAND, GRAND_MARKER))).invocation_id;

    assert_eq!(
        root.status(&child, deadline())
            .expect("a child is reachable")
            .invocation_id,
        child
    );
    assert_eq!(
        root.status(&grand, deadline())
            .expect("a grandchild is reachable")
            .invocation_id,
        grand
    );
    assert_eq!(
        child_svc
            .status(&grand, deadline())
            .expect("a child reaches its child")
            .invocation_id,
        grand
    );
    assert!(matches!(
        refused(child_svc.status(&child, deadline())),
        AgentsRefusal::NotYourSubtree
    ));
}

#[test]
fn agents_recursion_refuses_a_third_level_by_depth_limit_with_zero_effects() {
    let env = rooted();
    let broker = env.lab.broker();
    let root = env.service(&broker, root_caller());
    let child = spawned(root.spawn(args(RoleId::Builder, KEY_CHILD, CHILD_MARKER))).invocation_id;
    let child_svc = env.service(&broker, caller_for(&env, &child, RoleId::Builder));
    let grand = spawned(child_svc.spawn(args_with(
        RoleId::Builder,
        KEY_GRAND,
        items_for(GRAND_MARKER),
        grand_slice(),
    )))
    .invocation_id;
    let grand_svc = env.service(&broker, caller_for(&env, &grand, RoleId::Builder));
    let before = env.effects();

    let refusal = refused(grand_svc.spawn(args_with(
        RoleId::Builder,
        KEY_GREAT,
        items_for(GRAND_MARKER),
        grand_slice(),
    )));

    assert!(matches!(
        refusal,
        AgentsRefusal::Delegation(DelegationRefusal::Admission(
            AdmissionRefusal::DepthLimit { .. }
        ))
    ));
    assert_eq!(env.effects(), before);
}

#[test]
fn agents_cancel_from_the_root_cancels_the_child_and_grandchild_leaf_first() {
    let env = rooted();
    let log: CancelLog = Arc::new(RefCell::new(Vec::new()));
    env.a.share_cancel_log(&log);
    let broker = env.lab.broker();
    let root = env.service(&broker, root_caller());
    let child = spawned(root.spawn(args(RoleId::Builder, KEY_CHILD, CHILD_MARKER))).invocation_id;
    let child_svc = env.service(&broker, caller_for(&env, &child, RoleId::Builder));
    let grand =
        spawned(child_svc.spawn(args(RoleId::Builder, KEY_GRAND, GRAND_MARKER))).invocation_id;

    let view = root
        .cancel(&child, at(), deadline())
        .expect("the child is cancelled");

    assert_eq!(view.cancelled, vec![child.clone(), grand.clone()]);
    assert!(view.unconfirmed.is_empty());
    assert_eq!(
        log.borrow().as_slice(),
        [grand.clone(), child.clone()],
        "the runner hears the grandchild first"
    );
    assert_eq!(env.node(&child).status, NodeStatus::Cancelled);
    assert_eq!(env.node(&grand).status, NodeStatus::Cancelled);
    assert_eq!(env.node(&inv(ROOT_TAG)).status, NodeStatus::Running);
}

#[test]
fn agents_list_returns_direct_children_only() {
    let env = rooted();
    let broker = env.lab.broker();
    let root = env.service(&broker, root_caller());
    let child = spawned(root.spawn(args(RoleId::Builder, KEY_CHILD, CHILD_MARKER))).invocation_id;
    let sibling =
        spawned(root.spawn(args(RoleId::Builder, KEY_SECOND, OTHER_REF_TEXT))).invocation_id;
    let child_svc = env.service(&broker, caller_for(&env, &child, RoleId::Builder));
    let grand =
        spawned(child_svc.spawn(args(RoleId::Builder, KEY_GRAND, GRAND_MARKER))).invocation_id;

    let root_children: Vec<InvocationId> = root
        .list(deadline())
        .expect("the root lists")
        .into_iter()
        .map(|view| view.invocation_id)
        .collect();
    assert_eq!(root_children, vec![child.clone(), sibling]);
    let child_children: Vec<InvocationId> = child_svc
        .list(deadline())
        .expect("the child lists")
        .into_iter()
        .map(|view| view.invocation_id)
        .collect();
    assert_eq!(child_children, vec![grand]);
}

#[test]
fn agents_collect_reports_a_completed_child_as_an_untrusted_claim() {
    let env = rooted();
    let broker = env.lab.broker();
    let root = env.service(&broker, root_caller());
    let child = spawned(root.spawn(args(RoleId::Builder, KEY_CHILD, CHILD_MARKER))).invocation_id;
    let digest = env.node(&child).context_digest;
    env.a.set_status(&child, completed_status());
    env.a.set_result(builder_result(&child, &digest, false));

    let view = root
        .collect(&child, at(), deadline())
        .expect("the child is collected");

    match view {
        CollectView::Claimed { agent, claim } => {
            assert_eq!(agent.status, NodeStatus::Completed);
            assert_eq!(claim.invocation_id(), &child);
        }
        other => panic!("expected a completed claim, got {other:?}"),
    }
}

#[test]
fn agents_spawn_roots_the_child_at_the_run_root() {
    let env = rooted();
    let broker = env.lab.broker();
    let root = env.service(&broker, root_caller());
    let outcome = spawned(root.spawn(args(RoleId::Builder, KEY_CHILD, CHILD_MARKER)));
    let node = env.node(&outcome.invocation_id);
    assert_eq!(node.parent, Some(inv(ROOT_TAG)));
    assert_eq!(node.root, inv(ROOT_TAG));
}

#[test]
fn agents_derived_ids_are_stable_per_run_and_key() {
    use super::agents::derive_invocation_id;
    let run = run_id();
    let key = AdmissionKey::new(KEY_CHILD).expect("valid key");
    let other = AdmissionKey::new(KEY_SECOND).expect("valid key");
    let first = derive_invocation_id(&run, &key);
    assert_eq!(first, derive_invocation_id(&run, &key));
    assert_ne!(first, derive_invocation_id(&run, &other));
}
