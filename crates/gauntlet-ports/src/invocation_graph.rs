//! `InvocationGraph` port (E2 design): the Gauntlet-owned lineage of role
//! invocations. Every delegated invocation is a node with a parent, a root,
//! a depth, a reserved budget slice, and a status. The graph is the single
//! source of lineage truth; no harness topology is ever consulted.
//!
//! Atomicity contract: every method is one atomic step. A refused or failed
//! call leaves the graph exactly as it was. Terminal states are final.
//!
//! Ordering contract for queries: `children`, `descendants`, and
//! `active_nodes` return nodes in admission order; `descendants` sorts by
//! depth first, then admission order.

use core::fmt;

use gauntlet_domain::{
    AdmissionKey, BudgetDimension, Digest, EpochSeconds, HarnessId, InvocationId,
    ResultContractPath, RoleId, RunId, RunnerProfileId, SessionRef, WorkspaceRef,
};

use crate::agent_runner::InvocationFailure;
use crate::budget_meter::BudgetSlice;
use crate::deadline::Deadline;
use crate::lifecycle::{HarnessObservation, LifecycleView, ObservationOutcome, ObservationRecord};

/// Lifecycle status of a graph node. Harness lifecycle states are NOT node
/// status; they are liveness evidence owned by the observation path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeStatus {
    Admitted,
    Running,
    Completed,
    Failed(InvocationFailure),
    TimedOut,
    Cancelled,
}

impl NodeStatus {
    /// Completed, failed, timed out, and cancelled are final.
    pub fn is_terminal(self) -> bool {
        !matches!(self, Self::Admitted | Self::Running)
    }
}

impl fmt::Display for NodeStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Admitted => f.write_str("admitted"),
            Self::Running => f.write_str("running"),
            Self::Completed => f.write_str("completed"),
            Self::Failed(class) => write!(f, "failed({class})"),
            Self::TimedOut => f.write_str("timed_out"),
            Self::Cancelled => f.write_str("cancelled"),
        }
    }
}

/// The native session a node is currently bound to, with the epoch that
/// names this binding. Epochs start at 1 and increase on each attach.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeSessionBinding {
    pub session: SessionRef,
    pub epoch: u32,
}

/// One node of the invocation graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvocationNode {
    pub invocation_id: InvocationId,
    pub run_id: RunId,
    pub parent: Option<InvocationId>,
    pub root: InvocationId,
    pub depth: u32,
    pub role: RoleId,
    pub runner_profile: RunnerProfileId,
    pub harness: HarnessId,
    pub workspace: WorkspaceRef,
    pub context_digest: Digest,
    pub result_contract: ResultContractPath,
    pub admission_key: AdmissionKey,
    /// The budget reserved for this node and its subtree accounting.
    pub budget: BudgetSlice,
    pub status: NodeStatus,
    pub native_session: Option<NativeSessionBinding>,
    /// The digest of the schema-valid result, set by `complete`. `None`
    /// until the node completes; never set by any other transition.
    pub result_digest: Option<Digest>,
    pub admitted_at: EpochSeconds,
    pub terminal_at: Option<EpochSeconds>,
}

/// Structural limits applied at admission.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GraphLimits {
    /// The deepest node a child may occupy (the root is depth 0).
    pub max_depth: u32,
    /// Live (non-terminal) children a parent may hold at once.
    pub max_children_per_parent: u32,
}

/// The immutable facts of a node, plus the limits and admission instant.
/// Status, native session, and terminal time are not part of a request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmissionRequest {
    pub invocation_id: InvocationId,
    pub run_id: RunId,
    /// `None` admits a root. A run has exactly one root.
    pub parent: Option<InvocationId>,
    pub role: RoleId,
    pub runner_profile: RunnerProfileId,
    pub harness: HarnessId,
    pub workspace: WorkspaceRef,
    pub context_digest: Digest,
    pub result_contract: ResultContractPath,
    pub admission_key: AdmissionKey,
    pub budget: BudgetSlice,
    pub limits: GraphLimits,
    pub at: EpochSeconds,
}

/// Why an admission was refused. A refusal creates nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdmissionRefusal {
    /// The named parent is not in the graph.
    UnknownParent,
    /// The parent is in a different run than the request.
    WrongRun,
    /// The parent is terminal; no new work hangs under a settled node.
    ParentTerminal,
    /// The child would sit deeper than `max`.
    DepthLimit { max: u32, would_be: u32 },
    /// The parent already holds `max` live children.
    ChildLimit { max: u32 },
    /// The live children's reserved budget plus this slice exceeds the
    /// parent's budget in `dimension`.
    BudgetExceeded { dimension: BudgetDimension },
    /// The admission key is already bound to different immutable facts.
    KeyReusedWithDifferentFacts,
    /// The invocation id is already a node under a different key.
    InvocationIdReused,
    /// The run already has its root.
    RootAlreadyExists,
}

impl fmt::Display for AdmissionRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownParent => f.write_str("the parent is not in the graph"),
            Self::WrongRun => f.write_str("the parent belongs to a different run"),
            Self::ParentTerminal => f.write_str("the parent is terminal"),
            Self::DepthLimit { max, would_be } => {
                write!(f, "child depth {would_be} exceeds the maximum depth {max}")
            }
            Self::ChildLimit { max } => write!(f, "the parent already holds {max} live children"),
            Self::BudgetExceeded { dimension } => {
                write!(f, "the slice exceeds the parent budget in {dimension}")
            }
            Self::KeyReusedWithDifferentFacts => {
                f.write_str("the admission key is bound to different facts")
            }
            Self::InvocationIdReused => {
                f.write_str("the invocation id is already a node under another key")
            }
            Self::RootAlreadyExists => f.write_str("the run already has a root"),
        }
    }
}

/// The result of an admission attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdmissionOutcome {
    Admitted(InvocationNode),
    /// Same key, identical facts: the existing node, nothing created.
    AlreadyAdmitted(InvocationNode),
    Refused(AdmissionRefusal),
}

/// The result binding presented at completion. Both digests must match the
/// node's admitted facts for the completion to be accepted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResultBinding {
    pub contract_path: ResultContractPath,
    pub context_digest: Digest,
    pub result_digest: Digest,
}

/// `InvocationGraph` failures — typed (spec §21.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphError {
    /// A durable write failed. The graph is unchanged.
    WriteFailed { detail: String },
    /// A read failed.
    ReadFailed { detail: String },
    /// Stored lineage violates an invariant.
    Corrupt { detail: String },
    /// The backing store could not be reached before the deadline.
    Unavailable { detail: String },
    /// No node has this invocation id.
    UnknownInvocation { id: InvocationId },
    /// The transition is not legal from the node's current status. Also
    /// used for attaching a native session to a terminal node, where
    /// `from` and `to` are both the terminal status.
    IllegalTransition {
        id: InvocationId,
        from: NodeStatus,
        to: NodeStatus,
    },
    /// The completion binding does not match the node's admitted contract
    /// path or context digest. The node is unchanged.
    ContractMismatch { id: InvocationId },
}

impl fmt::Display for GraphError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WriteFailed { detail } => write!(f, "graph write failed: {detail}"),
            Self::ReadFailed { detail } => write!(f, "graph read failed: {detail}"),
            Self::Corrupt { detail } => write!(f, "graph lineage is corrupt: {detail}"),
            Self::Unavailable { detail } => write!(f, "graph unavailable: {detail}"),
            Self::UnknownInvocation { id } => write!(f, "no invocation {id} in the graph"),
            Self::IllegalTransition { id, from, to } => {
                write!(f, "invocation {id} cannot move from {from} to {to}")
            }
            Self::ContractMismatch { id } => write!(
                f,
                "completion binding for {id} does not match its admitted contract"
            ),
        }
    }
}

impl std::error::Error for GraphError {}

/// The Gauntlet-owned invocation graph.
pub trait InvocationGraph {
    /// Admits a node atomically: checks every refusal rule against the
    /// current graph, then inserts. Refusals are `Ok(Refused(..))` and
    /// create nothing. Replays of the same key and facts return
    /// `AlreadyAdmitted` even when the parent has since become terminal.
    fn admit(
        &self,
        request: AdmissionRequest,
        deadline: Deadline,
    ) -> Result<AdmissionOutcome, GraphError>;

    /// Binds a native session to a non-terminal node. Epochs are 1, 2, 3, …
    /// per node. Refused on terminal nodes.
    fn attach_native_session(
        &self,
        id: &InvocationId,
        session: SessionRef,
        deadline: Deadline,
    ) -> Result<NativeSessionBinding, GraphError>;

    /// Admitted → Running.
    fn mark_running(
        &self,
        id: &InvocationId,
        deadline: Deadline,
    ) -> Result<InvocationNode, GraphError>;

    /// Admitted|Running → Completed, only when the binding matches the
    /// node's admitted contract path and context digest.
    fn complete(
        &self,
        id: &InvocationId,
        binding: ResultBinding,
        at: EpochSeconds,
        deadline: Deadline,
    ) -> Result<InvocationNode, GraphError>;

    /// Admitted|Running → Failed(class).
    fn fail(
        &self,
        id: &InvocationId,
        class: InvocationFailure,
        at: EpochSeconds,
        deadline: Deadline,
    ) -> Result<InvocationNode, GraphError>;

    /// Admitted|Running → TimedOut.
    fn time_out(
        &self,
        id: &InvocationId,
        at: EpochSeconds,
        deadline: Deadline,
    ) -> Result<InvocationNode, GraphError>;

    /// Marks the node and every non-terminal descendant Cancelled in one
    /// atomic step. Returns exactly the ids newly cancelled, subtree root
    /// first, then descendants in `descendants` order. Idempotent: a second
    /// call returns an empty list.
    fn cancel_subtree(
        &self,
        id: &InvocationId,
        at: EpochSeconds,
        deadline: Deadline,
    ) -> Result<Vec<InvocationId>, GraphError>;

    /// One node, or `None` when it was never admitted.
    fn node(
        &self,
        id: &InvocationId,
        deadline: Deadline,
    ) -> Result<Option<InvocationNode>, GraphError>;

    /// Direct children in admission order.
    fn children(
        &self,
        id: &InvocationId,
        deadline: Deadline,
    ) -> Result<Vec<InvocationNode>, GraphError>;

    /// All descendants at every depth: depth first, then admission order.
    fn descendants(
        &self,
        id: &InvocationId,
        deadline: Deadline,
    ) -> Result<Vec<InvocationNode>, GraphError>;

    /// Parent first, root last. Empty for the root itself.
    fn ancestors(
        &self,
        id: &InvocationId,
        deadline: Deadline,
    ) -> Result<Vec<InvocationNode>, GraphError>;

    /// Non-terminal nodes of a run, in admission order.
    fn active_nodes(
        &self,
        run_id: &RunId,
        deadline: Deadline,
    ) -> Result<Vec<InvocationNode>, GraphError>;

    /// Records one lifecycle observation (E3a). Liveness and forensics only:
    /// this never changes node status, completes or fails an invocation, or
    /// touches its budget. The outcome is decided against the node's current
    /// binding and per-key sequencing; see [`crate::lifecycle`]. Every
    /// received observation is appended to the history, whatever its outcome.
    /// Unknown invocation: `UnknownInvocation`.
    fn record_observation(
        &self,
        observation: HarnessObservation,
        received_at: EpochSeconds,
        deadline: Deadline,
    ) -> Result<ObservationOutcome, GraphError>;

    /// The current lifecycle view of a node, computed under its current
    /// epoch. Unknown invocation: `UnknownInvocation`.
    fn lifecycle(&self, id: &InvocationId, deadline: Deadline)
    -> Result<LifecycleView, GraphError>;

    /// Every observation received for a node, in receipt order, with its
    /// outcome. Append-only. Unknown invocation: `UnknownInvocation`.
    fn observation_history(
        &self,
        id: &InvocationId,
        deadline: Deadline,
    ) -> Result<Vec<ObservationRecord>, GraphError>;
}
