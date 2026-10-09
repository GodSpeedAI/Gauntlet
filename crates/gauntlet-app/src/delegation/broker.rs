//! The DelegationBroker's DELEGATE path (E4a).
//!
//! Frozen order: (1) build the fresh-context envelope, (2) select a runner
//! through the fabric (an explicit pin is honored or refused, never replaced),
//! (3) admit the node in the invocation graph, (4) invoke the runner once and
//! record its native session. A refusal at (1)-(3) makes zero runner calls.
//! An invoke failure leaves the node Admitted so a retry with the same key
//! launches it.

use core::fmt;
use std::collections::BTreeMap;
use std::rc::Rc;

use gauntlet_domain::{
    AdmissionKey, Budget, EpochSeconds, InvocationId, IterationNo, PermissionSetRef,
    ResultContractPath, RoleId, RunId, RunnerProfileId, SandboxPolicyRef, WorkspaceRef,
};
use gauntlet_ports::agent_runner::{AgentError, AgentRunner, InvocationHandle, RoleInvocationSpec};
use gauntlet_ports::invocation_graph::{
    AdmissionOutcome, AdmissionRefusal, AdmissionRequest, GraphError, GraphLimits, InvocationGraph,
    InvocationNode,
};
use gauntlet_ports::{BudgetSlice, Deadline};

use super::context::{ContextItem, ContextPolicy, ContextRefusal, build_envelope};
use crate::fabric::RunnerRegistry;
use crate::fabric::selection::{Preference, Requirements, SelectionRefusal, select};

/// The fixed instruction every delegated child receives. The task itself lives
/// in the envelope, so the prompt carries no caller content.
const DELEGATED_PROMPT: &str = "perform the role named in the context envelope and write the typed result document at the declared contract path";

/// Everything one delegation needs. The broker derives the envelope, the
/// selection, and the admission from these facts; nothing is read from ambient
/// state.
#[derive(Debug, Clone)]
pub struct DelegationRequest {
    pub run_id: RunId,
    /// `None` admits a root.
    pub parent: Option<InvocationId>,
    pub role: RoleId,
    pub items: Vec<ContextItem>,
    pub policy: ContextPolicy,
    pub requirements: Requirements,
    pub preference: Preference,
    pub workspace: WorkspaceRef,
    pub permissions: PermissionSetRef,
    pub sandbox_policy: SandboxPolicyRef,
    /// The slice reserved in the graph for this node's subtree.
    pub budget: BudgetSlice,
    /// The budget object the runner's spec carries. Its slice is `budget`.
    pub spec_budget: Budget,
    pub limits: GraphLimits,
    pub admission_key: AdmissionKey,
    pub invocation_id: InvocationId,
    pub result_contract: ResultContractPath,
    pub deadline: Deadline,
    pub iteration_no: IterationNo,
    pub at: EpochSeconds,
}

/// Whether this call launched the invocation or found it already launched.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DelegationOutcome {
    Launched,
    AlreadyLaunched,
}

/// A delegation that reached the runner boundary.
#[derive(Debug, Clone)]
pub struct Delegated {
    pub node: InvocationNode,
    pub handle: InvocationHandle,
    pub outcome: DelegationOutcome,
}

/// Why a delegation was not launched. Every variant except `RunnerAdmission`
/// is raised before the runner is called.
#[derive(Debug, Clone, PartialEq)]
pub enum DelegationRefusal {
    Context(ContextRefusal),
    NoEligibleRunner(SelectionRefusal),
    /// The selected profile has no runner in the [`RunnerSet`].
    RunnerUnbound {
        profile: RunnerProfileId,
    },
    Admission(AdmissionRefusal),
    Graph(GraphError),
    /// The runner refused or failed to admit. The node stays Admitted.
    RunnerAdmission(AgentError),
}

impl fmt::Display for DelegationRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Context(refusal) => write!(f, "context refused: {refusal}"),
            Self::NoEligibleRunner(refusal) => write!(f, "no eligible runner: {refusal}"),
            Self::RunnerUnbound { profile } => {
                write!(f, "runner profile {profile} has no bound runner")
            }
            Self::Admission(refusal) => write!(f, "admission refused: {refusal}"),
            Self::Graph(error) => write!(f, "invocation graph failed: {error}"),
            Self::RunnerAdmission(error) => write!(f, "runner admission failed: {error}"),
        }
    }
}

impl std::error::Error for DelegationRefusal {}

/// Why a [`RunnerSet`] does not match its registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunnerSetError {
    DuplicateRunner {
        profile: RunnerProfileId,
    },
    /// A runner was supplied for a profile the registry does not hold.
    Unregistered {
        profile: RunnerProfileId,
    },
    /// A registered profile has no runner.
    Missing {
        profile: RunnerProfileId,
    },
}

impl fmt::Display for RunnerSetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateRunner { profile } => {
                write!(f, "runner profile {profile} is bound twice")
            }
            Self::Unregistered { profile } => {
                write!(f, "runner profile {profile} is not in the registry")
            }
            Self::Missing { profile } => write!(f, "registry profile {profile} has no runner"),
        }
    }
}

impl std::error::Error for RunnerSetError {}

/// Runners keyed by profile, built together with the registry they serve.
#[derive(Clone, Default)]
pub struct RunnerSet {
    runners: BTreeMap<RunnerProfileId, Rc<dyn AgentRunner>>,
}

impl RunnerSet {
    /// Binds exactly one runner to each registry profile. Duplicates, runners
    /// for unknown profiles, and profiles without a runner are typed refusals.
    pub fn for_registry(
        registry: &RunnerRegistry,
        entries: Vec<(RunnerProfileId, Rc<dyn AgentRunner>)>,
    ) -> Result<Self, RunnerSetError> {
        let mut runners = BTreeMap::new();
        for (profile, runner) in entries {
            if registry.get(&profile).is_none() {
                return Err(RunnerSetError::Unregistered { profile });
            }
            if runners.insert(profile.clone(), runner).is_some() {
                return Err(RunnerSetError::DuplicateRunner { profile });
            }
        }
        for instance in registry.instances() {
            if !runners.contains_key(&instance.profile.id) {
                return Err(RunnerSetError::Missing {
                    profile: instance.profile.id.clone(),
                });
            }
        }
        Ok(Self { runners })
    }

    fn get(&self, profile: &RunnerProfileId) -> Option<&Rc<dyn AgentRunner>> {
        self.runners.get(profile)
    }
}

/// The harness-neutral delegation broker. Holds borrowed ports only.
pub struct DelegationBroker<'g> {
    graph: &'g dyn InvocationGraph,
    registry: &'g RunnerRegistry,
    runners: &'g RunnerSet,
}

impl<'g> DelegationBroker<'g> {
    pub fn new(
        graph: &'g dyn InvocationGraph,
        registry: &'g RunnerRegistry,
        runners: &'g RunnerSet,
    ) -> Self {
        Self {
            graph,
            registry,
            runners,
        }
    }

    /// Launches one delegated invocation, at most once per admission key.
    pub fn delegate(&self, request: DelegationRequest) -> Result<Delegated, DelegationRefusal> {
        let envelope =
            build_envelope(&request.policy, &request.items).map_err(DelegationRefusal::Context)?;

        // Fresh context is structural, so every delegation requires a runner
        // that can start fresh, whatever the caller's requirements said.
        let mut requirements = request.requirements.clone();
        requirements.fresh_context = true;
        let selected = select(self.registry, &requirements, &request.preference)
            .map_err(DelegationRefusal::NoEligibleRunner)?;
        let profile = selected.instance.profile;
        let runner =
            self.runners
                .get(&profile.id)
                .ok_or_else(|| DelegationRefusal::RunnerUnbound {
                    profile: profile.id.clone(),
                })?;

        let deadline = request.deadline;
        let admission = AdmissionRequest {
            invocation_id: request.invocation_id.clone(),
            run_id: request.run_id.clone(),
            parent: request.parent.clone(),
            role: request.role.clone(),
            runner_profile: profile.id.clone(),
            harness: profile.harness.clone(),
            workspace: request.workspace.clone(),
            context_digest: envelope.digest().clone(),
            result_contract: request.result_contract.clone(),
            admission_key: request.admission_key.clone(),
            budget: request.budget,
            limits: request.limits,
            at: request.at,
        };
        let node = match self
            .graph
            .admit(admission, deadline)
            .map_err(DelegationRefusal::Graph)?
        {
            AdmissionOutcome::Admitted(node) => node,
            AdmissionOutcome::AlreadyAdmitted(node) => match node.native_session.clone() {
                Some(binding) => {
                    let handle =
                        InvocationHandle::new(node.invocation_id.clone(), Some(binding.session));
                    return Ok(Delegated {
                        node,
                        handle,
                        outcome: DelegationOutcome::AlreadyLaunched,
                    });
                }
                None => node,
            },
            AdmissionOutcome::Refused(refusal) => {
                return Err(DelegationRefusal::Admission(refusal));
            }
        };

        let spec = RoleInvocationSpec {
            invocation_id: node.invocation_id.clone(),
            run_id: request.run_id.clone(),
            iteration_no: request.iteration_no,
            role: request.role.clone(),
            prompt: DELEGATED_PROMPT.to_owned(),
            context_envelope: envelope,
            workspace: request.workspace.clone(),
            permissions: request.permissions.clone(),
            result_contract_path: request.result_contract.clone(),
            deadline,
            model_hint: None,
            sandbox_policy: Some(request.sandbox_policy.clone()),
            budget_slice: request.spec_budget.clone(),
        };
        let handle = runner
            .invoke(spec)
            .map_err(DelegationRefusal::RunnerAdmission)?;
        if let Some(session) = handle.session_ref() {
            self.graph
                .attach_native_session(&node.invocation_id, session.clone(), deadline)
                .map_err(DelegationRefusal::Graph)?;
        }
        let node = self
            .graph
            .mark_running(&node.invocation_id, deadline)
            .map_err(DelegationRefusal::Graph)?;
        Ok(Delegated {
            node,
            handle,
            outcome: DelegationOutcome::Launched,
        })
    }
}
