//! Reconciliation of delegated invocations (E4a): status, collect, cancel, and
//! recover, all on [`DelegationBroker`].
//!
//! Authority rules:
//!
//! - The graph is the only state. A node is read, never inferred from harness
//!   topology, lifecycle, or forensics.
//! - Only a runner's contract-derived status drives a transition. A runner or
//!   harness lifecycle that says "done" or "idle" changes nothing here, and
//!   `status` polling never changes graph status by itself.
//! - Only `AgentError::UnknownHandle` is authoritative evidence that the runner
//!   lost an invocation. Every other runner error leaves the graph untouched
//!   and is reported as unreconciled, so the next recovery asks again.
//! - When a node is failed or orphaned, its active descendants are cancelled
//!   through the graph (`cancel_subtree` on the terminal node, which leaves the
//!   node itself alone), then the runners are told best-effort, leaf-first.
//! - Completion here is INVOCATION completion: a schema-valid result document at
//!   the declared contract path, bound to the node by the graph. Completion is
//!   not settlement. Settlement (observation, comparison, verification) lives
//!   elsewhere, and this module neither imports nor calls any settlement or
//!   verification API. A [`CollectOutcome::Completed`] result is therefore an
//!   untrusted claim.

use core::fmt;
use std::sync::Arc;

use gauntlet_domain::{
    Digest, EpochSeconds, InvocationId, ResultContractPath, RunId, RunnerProfileId,
};
use gauntlet_ports::Deadline;
use gauntlet_ports::agent_runner::{
    AgentError, AgentRunner, CancelReason, InvocationFailure, InvocationHandle, InvocationStatus,
};
use gauntlet_ports::invocation_graph::{GraphError, InvocationNode, NodeStatus, ResultBinding};
use gauntlet_ports::lifecycle::LifecycleView;
use gauntlet_ports::role_result::{RoleResult, encode_role_result};

use super::broker::DelegationBroker;

/// The reason recorded when the runner reports its own invocation cancelled.
const RUNNER_CANCELLED_REASON: &str = "runner reported the invocation cancelled";

/// The reason recorded on descendants of a node that was failed or orphaned.
const ANCESTOR_FAILED_REASON: &str = "an ancestor was failed or orphaned";

/// The class recorded when recovery finds a node whose runner lost it.
const ORPHANED_BY_RESTART: InvocationFailure = InvocationFailure::OrphanedByRestart;

/// A reconciliation failure that is not a node outcome. Typed, never swallowed.
#[derive(Debug, Clone, PartialEq)]
pub enum BrokerError {
    Graph(GraphError),
    UnknownInvocation { id: InvocationId },
    RunnerUnbound { profile: RunnerProfileId },
    Runner(AgentError),
}

impl fmt::Display for BrokerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Graph(error) => write!(f, "invocation graph failed: {error}"),
            Self::UnknownInvocation { id } => write!(f, "no invocation {id} in the graph"),
            Self::RunnerUnbound { profile } => {
                write!(f, "runner profile {profile} has no bound runner")
            }
            Self::Runner(error) => write!(f, "runner failed: {error}"),
        }
    }
}

impl std::error::Error for BrokerError {}

/// One node's graph state beside what its runner reports.
#[derive(Debug, Clone, PartialEq)]
pub struct DelegationStatus {
    pub node: InvocationNode,
    /// `None` when the node is terminal, unbound, never launched, or its
    /// runner could not answer (see `runner_unavailable`).
    pub runner_status: Option<InvocationStatus>,
    /// `true` when the node is live and bound but its runner could not answer
    /// the poll (for example a session lost across a restart). The graph is
    /// canonical: the view is still returned, and harness state is forensics.
    pub runner_unavailable: bool,
    pub lifecycle: LifecycleView,
}

/// What `collect` observed for one invocation.
#[derive(Debug, Clone, PartialEq)]
pub enum CollectOutcome {
    StillRunning(InvocationNode),
    /// The graph bound the result to the node. The result is an untrusted
    /// claim; this outcome carries no settlement.
    Completed {
        node: InvocationNode,
        result: Box<RoleResult>,
    },
    Failed(InvocationNode),
    TimedOut(InvocationNode),
    /// The runner reported cancellation; the subtree was cancelled.
    Cancelled(InvocationNode),
    AlreadyTerminal(InvocationNode),
}

/// The outcome of one cancellation. `cancelled` is the graph's root-first list
/// of newly cancelled ids. Runner calls ran leaf-first, and a failed call is
/// recorded in `runner_errors` without undoing the graph.
#[derive(Debug, Clone, PartialEq)]
pub struct CancelReport {
    pub cancelled: Vec<InvocationId>,
    pub runner_errors: Vec<(InvocationId, AgentError)>,
}

/// What recovery did to one active node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryAction {
    /// The runner still holds the invocation and it is live.
    Reattached,
    /// A transition was applied from the runner's terminal status.
    Settled(NodeStatus),
    /// The runner reported the handle unknown, so the node was failed as
    /// orphaned.
    Orphaned,
    /// The node was admitted with no binding (a crash between admission and
    /// launch). It is failed as orphaned. A relaunch is a new delegation with a
    /// new key.
    NeverLaunched,
}

/// A node recovery could not reconcile. The graph was not changed for it, so
/// the next recovery of the run asks again.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unreconciled {
    pub id: InvocationId,
    pub detail: String,
}

/// Recovery's report for one run. Every active node is either in `actions` or
/// in `unreconciled`; one node's failure never stops the others.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RecoveryReport {
    /// Per-node actions in admission order.
    pub actions: Vec<(InvocationId, RecoveryAction)>,
    pub unreconciled: Vec<Unreconciled>,
    /// Active descendants cancelled because their ancestor was failed or
    /// orphaned, in the order the graph reported them.
    pub cancelled_descendants: Vec<InvocationId>,
    /// Best-effort runner cancels that failed during those cascades.
    pub cascade_errors: Vec<(InvocationId, AgentError)>,
}

/// The internal result of applying one runner status to one node.
enum Settlement {
    Open(InvocationNode),
    Completed {
        node: InvocationNode,
        result: Box<RoleResult>,
    },
    Failed(InvocationNode),
    TimedOut(InvocationNode),
    Cancelled(InvocationNode),
}

/// Descendant cancellations accumulated while settling nodes.
#[derive(Default)]
struct Cascade {
    cancelled: Vec<InvocationId>,
    runner_errors: Vec<(InvocationId, AgentError)>,
}

/// The canonical digest of a result document: the content digest of its
/// canonical encoding. Equal results digest equal, and any changed field
/// changes the digest.
fn result_digest(result: &RoleResult) -> Digest {
    crate::verify::content_digest(encode_role_result(result).as_bytes())
}

fn bound_handle(node: &InvocationNode) -> Option<InvocationHandle> {
    node.native_session.as_ref().map(|binding| {
        InvocationHandle::new(node.invocation_id.clone(), Some(binding.session.clone()))
    })
}

impl DelegationBroker<'_> {
    /// Reads one node and, when it is live and bound, polls its runner. Polling
    /// never changes graph status.
    pub fn status(
        &self,
        id: &InvocationId,
        deadline: Deadline,
    ) -> Result<DelegationStatus, BrokerError> {
        let node = self.node_of(id, deadline)?;
        let lifecycle = self
            .graph
            .lifecycle(id, deadline)
            .map_err(BrokerError::Graph)?;
        // A poll failure is a lost or unreachable harness session, never a
        // graph fact: it reports `runner_unavailable` and the graph view stands.
        // Graph and profile-binding errors above and below still fail.
        let (runner_status, runner_unavailable) = match bound_handle(&node) {
            Some(handle) if !node.status.is_terminal() => {
                let runner = self.runner_for(&node.runner_profile)?;
                match runner.poll(&handle) {
                    Ok(status) => (Some(status), false),
                    Err(_) => (None, true),
                }
            }
            _ => (None, false),
        };
        Ok(DelegationStatus {
            node,
            runner_status,
            runner_unavailable,
            lifecycle,
        })
    }

    /// Polls the runner and applies only its contract-derived status. A live
    /// runner status leaves the node open, whatever the child claims.
    ///
    /// A failed or timed-out node has its active descendants cancelled as well.
    /// That cascade's runner errors are best-effort and are reported by
    /// [`DelegationBroker::recover`] only, so collect drops them here; the
    /// graph state is authoritative either way.
    pub fn collect(
        &self,
        id: &InvocationId,
        at: EpochSeconds,
        deadline: Deadline,
    ) -> Result<CollectOutcome, BrokerError> {
        let node = self.node_of(id, deadline)?;
        if node.status.is_terminal() {
            return Ok(CollectOutcome::AlreadyTerminal(node));
        }
        let Some(handle) = bound_handle(&node) else {
            return Ok(CollectOutcome::StillRunning(node));
        };
        let runner = self.runner_for(&node.runner_profile)?;
        let status = runner.poll(&handle).map_err(BrokerError::Runner)?;
        let mut cascade = Cascade::default();
        Ok(
            match self.apply_status(node, &handle, status, at, deadline, &mut cascade)? {
                Settlement::Open(node) => CollectOutcome::StillRunning(node),
                Settlement::Completed { node, result } => {
                    CollectOutcome::Completed { node, result }
                }
                Settlement::Failed(node) => CollectOutcome::Failed(node),
                Settlement::TimedOut(node) => CollectOutcome::TimedOut(node),
                Settlement::Cancelled(node) => CollectOutcome::Cancelled(node),
            },
        )
    }

    /// Cancels the graph subtree first (authoritative), then calls each bound
    /// runner best-effort, leaf-first. A second cancel is a no-op.
    pub fn cancel(
        &self,
        id: &InvocationId,
        reason: CancelReason,
        at: EpochSeconds,
        deadline: Deadline,
    ) -> Result<CancelReport, BrokerError> {
        let cancelled = self
            .graph
            .cancel_subtree(id, at, deadline)
            .map_err(BrokerError::Graph)?;
        let mut runner_errors = Vec::new();
        for victim in cancelled.iter().rev() {
            let node = self.node_of(victim, deadline)?;
            let Some(handle) = bound_handle(&node) else {
                continue;
            };
            let outcome = match self.runners.get(&node.runner_profile) {
                Some(runner) => runner.cancel(&handle, reason.clone(), deadline),
                None => Err(AgentError::CancelFailed {
                    invocation_id: victim.clone(),
                    detail: format!("runner profile {} has no bound runner", node.runner_profile),
                }),
            };
            if let Err(error) = outcome {
                runner_errors.push((victim.clone(), error));
            }
        }
        Ok(CancelReport {
            cancelled,
            runner_errors,
        })
    }

    /// Reconstructs the run's active nodes from the graph alone, in admission
    /// order. Terminal runner statuses are applied once; a second call on a
    /// settled run reports only reattachments. A node whose runner cannot answer
    /// is left unchanged and reported as unreconciled. Only a failure to read the
    /// run's nodes is an error.
    pub fn recover(
        &self,
        run_id: &RunId,
        at: EpochSeconds,
        deadline: Deadline,
    ) -> Result<RecoveryReport, BrokerError> {
        let active = self
            .graph
            .active_nodes(run_id, deadline)
            .map_err(BrokerError::Graph)?;
        let mut cascade = Cascade::default();
        let mut actions = Vec::with_capacity(active.len());
        let mut unreconciled = Vec::new();
        for snapshot in active {
            let id = snapshot.invocation_id;
            // An earlier node's cascade may have settled this one already.
            let node = match self.graph.node(&id, deadline) {
                Ok(Some(node)) if !node.status.is_terminal() => node,
                Ok(_) => continue,
                Err(error) => {
                    unreconciled.push(Unreconciled {
                        id,
                        detail: error.to_string(),
                    });
                    continue;
                }
            };
            match self.recover_node(node, at, deadline, &mut cascade) {
                Ok(action) => actions.push((id, action)),
                Err(error) => unreconciled.push(Unreconciled {
                    id,
                    detail: error.to_string(),
                }),
            }
        }
        Ok(RecoveryReport {
            actions,
            unreconciled,
            cancelled_descendants: cascade.cancelled,
            cascade_errors: cascade.runner_errors,
        })
    }

    fn recover_node(
        &self,
        node: InvocationNode,
        at: EpochSeconds,
        deadline: Deadline,
        cascade: &mut Cascade,
    ) -> Result<RecoveryAction, BrokerError> {
        let id = node.invocation_id.clone();
        let Some(handle) = bound_handle(&node) else {
            self.fail_orphan(&id, at, deadline, cascade)?;
            return Ok(RecoveryAction::NeverLaunched);
        };
        let runner = self.runner_for(&node.runner_profile)?;
        let status = match runner.poll(&handle) {
            Ok(status) => status,
            Err(AgentError::UnknownHandle { .. }) => {
                self.fail_orphan(&id, at, deadline, cascade)?;
                return Ok(RecoveryAction::Orphaned);
            }
            // Any other runner error is not evidence of loss: leave the node.
            Err(other) => return Err(BrokerError::Runner(other)),
        };
        Ok(
            match self.apply_status(node, &handle, status, at, deadline, cascade)? {
                Settlement::Open(_) => RecoveryAction::Reattached,
                Settlement::Completed { node, .. }
                | Settlement::Failed(node)
                | Settlement::TimedOut(node)
                | Settlement::Cancelled(node) => RecoveryAction::Settled(node.status),
            },
        )
    }

    fn node_of(
        &self,
        id: &InvocationId,
        deadline: Deadline,
    ) -> Result<InvocationNode, BrokerError> {
        self.graph
            .node(id, deadline)
            .map_err(BrokerError::Graph)?
            .ok_or_else(|| BrokerError::UnknownInvocation { id: id.clone() })
    }

    fn runner_for(&self, profile: &RunnerProfileId) -> Result<&Arc<dyn AgentRunner>, BrokerError> {
        self.runners
            .get(profile)
            .ok_or_else(|| BrokerError::RunnerUnbound {
                profile: profile.clone(),
            })
    }

    /// Fails a node as orphaned, then cancels its active descendants.
    fn fail_orphan(
        &self,
        id: &InvocationId,
        at: EpochSeconds,
        deadline: Deadline,
        cascade: &mut Cascade,
    ) -> Result<(), BrokerError> {
        self.graph
            .fail(id, ORPHANED_BY_RESTART, at, deadline)
            .map_err(BrokerError::Graph)?;
        self.cascade_from(id, at, deadline, cascade)
    }

    /// Cancels the active descendants of a node that is already terminal. The
    /// graph leaves the terminal node itself alone; each runner is then told,
    /// best-effort and leaf-first.
    fn cascade_from(
        &self,
        id: &InvocationId,
        at: EpochSeconds,
        deadline: Deadline,
        cascade: &mut Cascade,
    ) -> Result<(), BrokerError> {
        let reason = CancelReason::new(ANCESTOR_FAILED_REASON)
            .expect("the ancestor-failed reason is a non-empty constant");
        let report = self.cancel(id, reason, at, deadline)?;
        cascade.cancelled.extend(report.cancelled);
        cascade.runner_errors.extend(report.runner_errors);
        Ok(())
    }

    /// Applies one runner status to one node. Non-terminal statuses heal a lost
    /// Running transition and leave the node open. Terminal statuses are
    /// applied through the graph, which decides every binding. A failed or
    /// timed-out node cascades to its active descendants.
    fn apply_status(
        &self,
        node: InvocationNode,
        handle: &InvocationHandle,
        status: InvocationStatus,
        at: EpochSeconds,
        deadline: Deadline,
        cascade: &mut Cascade,
    ) -> Result<Settlement, BrokerError> {
        let id = node.invocation_id.clone();
        match status {
            InvocationStatus::Admitted | InvocationStatus::Running { .. } => {
                let node = if node.status == NodeStatus::Admitted {
                    self.graph
                        .mark_running(&id, deadline)
                        .map_err(BrokerError::Graph)?
                } else {
                    node
                };
                Ok(Settlement::Open(node))
            }
            InvocationStatus::Completed { result_ref } => {
                self.complete_from_runner(node, result_ref, handle, at, deadline, cascade)
            }
            InvocationStatus::Failed { class } => {
                let node = self
                    .graph
                    .fail(&id, class, at, deadline)
                    .map_err(BrokerError::Graph)?;
                self.cascade_from(&id, at, deadline, cascade)?;
                Ok(Settlement::Failed(node))
            }
            InvocationStatus::TimedOut => {
                let node = self
                    .graph
                    .time_out(&id, at, deadline)
                    .map_err(BrokerError::Graph)?;
                self.cascade_from(&id, at, deadline, cascade)?;
                Ok(Settlement::TimedOut(node))
            }
            InvocationStatus::Cancelled => {
                let reason = CancelReason::new(RUNNER_CANCELLED_REASON)
                    .expect("the runner-cancelled reason is a non-empty constant");
                let report = self.cancel(&id, reason, at, deadline)?;
                cascade.cancelled.extend(report.cancelled);
                cascade.runner_errors.extend(report.runner_errors);
                Ok(Settlement::Cancelled(self.node_of(&id, deadline)?))
            }
        }
    }

    /// Reads the result the runner names and binds it to the node. The binding
    /// is built from the result document, never from the node, so a mismatched
    /// result is refused by the graph, or by the broker when the invocation id
    /// disagrees (identical envelopes share a digest across siblings).
    fn complete_from_runner(
        &self,
        node: InvocationNode,
        contract_path: ResultContractPath,
        handle: &InvocationHandle,
        at: EpochSeconds,
        deadline: Deadline,
        cascade: &mut Cascade,
    ) -> Result<Settlement, BrokerError> {
        let runner = self.runner_for(&node.runner_profile)?;
        let result = runner
            .collect(handle, deadline)
            .map_err(BrokerError::Runner)?;
        if result.invocation_id() != &node.invocation_id {
            return self.refuse_contract(&node.invocation_id, at, deadline, cascade);
        }
        let binding = ResultBinding {
            contract_path,
            context_digest: result.context_envelope_digest().clone(),
            result_digest: result_digest(&result),
        };
        match self
            .graph
            .complete(&node.invocation_id, binding, at, deadline)
        {
            Ok(node) => Ok(Settlement::Completed {
                node,
                result: Box::new(result),
            }),
            Err(GraphError::ContractMismatch { .. }) => {
                self.refuse_contract(&node.invocation_id, at, deadline, cascade)
            }
            Err(error) => Err(BrokerError::Graph(error)),
        }
    }

    fn refuse_contract(
        &self,
        id: &InvocationId,
        at: EpochSeconds,
        deadline: Deadline,
        cascade: &mut Cascade,
    ) -> Result<Settlement, BrokerError> {
        let node = self
            .graph
            .fail(id, InvocationFailure::ContractMismatch, at, deadline)
            .map_err(BrokerError::Graph)?;
        self.cascade_from(id, at, deadline, cascade)?;
        Ok(Settlement::Failed(node))
    }
}
