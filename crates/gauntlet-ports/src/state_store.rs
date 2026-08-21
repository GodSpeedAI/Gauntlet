//! `StateStore` — spec §6.2: "`append_event` · `load_run` · `checkpoint` ·
//! `resume(run_id)` · `export_jsonl` — Default adapter: embedded SQL.
//! Export is deterministic."
//!
//! The event vocabulary below is the minimum the spec names: the settlement
//! event's fields verbatim (§16.3 `SettlementEvent{unit_id, evidence_ref,
//! verification_ref, digest, ts}`), the invocation lifecycle states (§5.3.5),
//! and the checkpoint marker. The default embedded-SQL adapter is Task 6;
//! product identifiers belong to that adapter, never here (§24.3).

use core::fmt;

use gauntlet_domain::{
    Digest, EpochSeconds, EventLogOffset, EvidenceRef, InvocationId, InvocationState, IterationNo,
    ResultContractPath, RoleInvocation, Run, RunId, Spent, UnitId, UnitStatus, VerificationRef,
};

use crate::comparator::AdmittedComparison;
use crate::deadline::Deadline;

/// `StateStore` failures — typed (spec §21.2).
#[derive(Debug, Clone, PartialEq)]
pub enum StateStoreError {
    /// A durable write failed.
    WriteFailed { detail: String },
    /// A read failed.
    ReadFailed { detail: String },
    /// Stored state exists but does not decode into the domain types.
    Corrupt { detail: String },
    /// A stored projection omits an immutable object required to safely
    /// rehydrate it. Callers must migrate or recover it; no binding is
    /// fabricated from a placeholder.
    Unavailable { detail: String },
}

impl fmt::Display for StateStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WriteFailed { detail } => write!(f, "state write failed: {detail}"),
            Self::ReadFailed { detail } => write!(f, "state read failed: {detail}"),
            Self::Corrupt { detail } => {
                write!(f, "stored state is corrupt: {detail}")
            }
            Self::Unavailable { detail } => {
                write!(f, "stored state is unavailable: {detail}")
            }
        }
    }
}

impl std::error::Error for StateStoreError {}

/// One durable run event. The vocabulary is the minimum the spec names;
/// Task 6 (state store) extends it as the persistence schema lands.
#[derive(Debug, Clone, PartialEq)]
pub struct RunEvent {
    pub run_id: RunId,
    pub kind: RunEventKind,
    /// When the event happened (from the `Clock` port; stored, never
    /// re-derived at export time).
    pub at: EpochSeconds,
}

/// The named event kinds.
#[derive(Debug, Clone, PartialEq)]
pub enum RunEventKind {
    /// §16.3 step 1: `EventLog.append(SettlementEvent{unit_id,
    /// evidence_ref, verification_ref, digest, ts})` — field for field.
    SettlementCommitted {
        unit_id: UnitId,
        evidence: EvidenceRef,
        verification: VerificationRef,
        reference_digest: Digest,
    },
    /// An invocation was admitted (§5.3.5).
    InvocationAdmitted { invocation_id: InvocationId },
    /// An invocation reached a terminal state (§5.3.5).
    InvocationTerminal {
        invocation_id: InvocationId,
        state: InvocationState,
    },
    /// A checkpoint was made durable (§18).
    CheckpointWritten {
        iteration_no: IterationNo,
        event_log_offset: EventLogOffset,
    },
}

/// A durable checkpoint (spec §18, quoted: "Every iteration boundary writes
/// a durable checkpoint containing: run id, reference digest, iteration
/// number, unit-state projection, budget spent, in-flight invocation ids
/// with their result-contract paths, and the event-log offset.") — every
/// listed item is a required field.
#[derive(Debug, Clone, PartialEq)]
pub struct Checkpoint {
    pub run_id: RunId,
    pub reference_digest: Digest,
    pub iteration_no: IterationNo,
    pub unit_states: Vec<(UnitId, UnitStatus)>,
    pub budget_spent: Spent,
    pub in_flight: Vec<(InvocationId, ResultContractPath)>,
    pub event_log_offset: EventLogOffset,
}

/// The durable answer to "what did this run observe at this iteration?"
/// (spec §5.3.6, §18.1): the evidence the observation owner recorded AND the
/// §12.4 digest of its content.
///
/// The digest is part of the binding, not a convenience. Binding the pointer
/// alone terminates provenance only as far as the `EvidenceStore` handed to
/// the comparison chain is honest: a store that returns different bytes at
/// the bound ref would silently change what the critic compares. With the
/// content digest bound durably, the comparison port re-derives the digest
/// from the bytes it was handed and refuses anything that is not the content
/// the run recorded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservationBinding {
    evidence: EvidenceRef,
    digest: Digest,
}

impl ObservationBinding {
    pub fn new(evidence: EvidenceRef, digest: Digest) -> Self {
        Self { evidence, digest }
    }

    pub fn evidence(&self) -> &EvidenceRef {
        &self.evidence
    }

    /// The §12.4 observation digest of the bound bundle's content.
    pub fn digest(&self) -> &Digest {
        &self.digest
    }
}

/// The run-state port (spec §6.2). Every method performs durable IO, so
/// every method carries a deadline (§6 preamble).
pub trait StateStore {
    /// Appends one run event durably.
    fn append_event(
        &self,
        event: RunEvent,
        deadline: Deadline,
    ) -> Result<EventLogOffset, StateStoreError>;

    /// Loads a run's durable state; `None` when the run was never stored.
    fn load_run(&self, run_id: &RunId, deadline: Deadline) -> Result<Option<Run>, StateStoreError>;

    /// Writes a durable checkpoint (spec §18). Critical run state is never
    /// reconstructed from conversational memory (I11).
    fn checkpoint(&self, checkpoint: Checkpoint, deadline: Deadline)
    -> Result<(), StateStoreError>;

    /// Returns the run's latest checkpoint to resume from (spec §18);
    /// `None` when the run has no checkpoint yet.
    fn resume(
        &self,
        run_id: &RunId,
        deadline: Deadline,
    ) -> Result<Option<Checkpoint>, StateStoreError>;

    /// Persists the exact admitted domain invocation before its handle is
    /// allowed to enter a polling path. Implementations retain every §5.3.5
    /// binding field, not merely its id/event projection.
    fn record_role_invocation(
        &self,
        _invocation: &RoleInvocation,
        _deadline: Deadline,
    ) -> Result<(), StateStoreError> {
        Err(StateStoreError::WriteFailed {
            detail: "this state store does not implement durable role-invocation records"
                .to_owned(),
        })
    }

    /// Binds the run/iteration's authoritative observation bundle evidence.
    ///
    /// This is the observation owner's write (spec §5.3.6, §18.1): it is what
    /// makes one pinned bundle *this* run's observation at *this* iteration,
    /// rather than merely a well-formed document that happens to name them.
    /// The binding is write-once — a second call naming a DIFFERENT binding
    /// for the same (run, iteration) is refused, never overwritten — so a
    /// later substitute cannot displace the bundle a comparison was opened
    /// over. It records the bundle's CONTENT digest as well as its ref, so
    /// what is bound is the observation itself, not a pointer that some
    /// evidence store might resolve differently.
    fn bind_observation_bundle(
        &self,
        _run_id: &RunId,
        _iteration_no: IterationNo,
        _binding: &ObservationBinding,
        _deadline: Deadline,
    ) -> Result<(), StateStoreError> {
        Err(StateStoreError::WriteFailed {
            detail: "this state store does not implement durable observation bindings".to_owned(),
        })
    }

    /// The durably bound observation bundle for one (run, iteration); `None`
    /// when the observation owner has bound none yet. The comparison chain
    /// resolves the bundle it compares over from HERE — it never accepts a
    /// caller-selected evidence ref, so presenting a different, individually
    /// valid, correctly hashed, pinned bundle has no API to travel through;
    /// and it re-derives the content digest, so bytes that are not the bound
    /// content are refused even when the store serving them says otherwise.
    fn observation_binding(
        &self,
        _run_id: &RunId,
        _iteration_no: IterationNo,
        _deadline: Deadline,
    ) -> Result<Option<ObservationBinding>, StateStoreError> {
        Ok(None)
    }

    /// Atomically admits a comparison only when the durable run and its exact
    /// admitted critic invocation already exist and the complete identity tuple
    /// agrees. Evidence must have been pinned before this call; implementations
    /// record no comparison on any mismatch.
    ///
    /// The argument is an [`AdmittedComparison`], which has no constructor
    /// outside the comparator port: a caller holding a `ComparisonRecordDoc`
    /// it built itself has nothing to pass here. Durable comparison state is
    /// therefore reachable only from a document a comparator returned under a
    /// governed dispatch capability.
    fn accept_comparison(
        &self,
        _admitted: &AdmittedComparison,
        _deadline: Deadline,
    ) -> Result<(), StateStoreError> {
        Err(StateStoreError::WriteFailed {
            detail: "this state store does not implement mandatory comparison acceptance"
                .to_owned(),
        })
    }

    /// Deterministic JSONL export of one run (spec §6.2: "Export is
    /// deterministic" — same run, byte-identical output).
    fn export_jsonl(&self, run_id: &RunId, deadline: Deadline) -> Result<String, StateStoreError>;
}
