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
    ResultContractPath, Run, RunId, Spent, UnitId, UnitStatus, VerificationRef,
};

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
}

impl fmt::Display for StateStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WriteFailed { detail } => write!(f, "state write failed: {detail}"),
            Self::ReadFailed { detail } => write!(f, "state read failed: {detail}"),
            Self::Corrupt { detail } => {
                write!(f, "stored state is corrupt: {detail}")
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

    /// Deterministic JSONL export of one run (spec §6.2: "Export is
    /// deterministic" — same run, byte-identical output).
    fn export_jsonl(&self, run_id: &RunId, deadline: Deadline) -> Result<String, StateStoreError>;
}
