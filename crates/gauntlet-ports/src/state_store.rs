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
    CorrectionSelection, Digest, EpochSeconds, EventLogOffset, EvidenceLocator, EvidenceRef,
    InvocationId, InvocationState, IterationNo, ResultContractPath, RoleInvocation, Run, RunId,
    SettlementRecord, Spent, TerminalState, UnitId, UnitStatus, VerificationRef,
};

use crate::comparator::AdmittedComparison;
use crate::deadline::Deadline;
use crate::observer::InstrumentIdentity;
use crate::verification_record::VerificationRecordDoc;

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
    /// The append refused the authority-minted settlement event kind: it is
    /// earned only through [`StateStore::commit_settlement`], which consumes
    /// an [`crate::producer_authority::AdmittedSettlement`] minted by
    /// [`SettlementCommitAuthority::admit`](crate::producer_authority::SettlementCommitAuthority::admit)
    /// after the §16.1 conditions are validated against durable state. The
    /// generic event append (and the `EventLog` port over the same
    /// substrate) never mint it — a caller-built
    /// [`RunEventKind::SettlementCommitted`] names invented facts no
    /// admission ever earned (Task 13 COR3, confirmation round 1 F-1: the
    /// §18.3 replay mints a durable settlement row from the event alone, so
    /// an ungated append forged the exact record every settlement consumer
    /// reads).
    SettlementEventNotEarned { run_id: RunId, unit_id: UnitId },
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
            Self::SettlementEventNotEarned { run_id, unit_id } => write!(
                f,
                "the SettlementCommitted event for unit {unit_id} of run {run_id} is earned only \
                 through StateStore::commit_settlement, which consumes an AdmittedSettlement \
                 minted by SettlementCommitAuthority::admit — the generic event append refuses \
                 the settlement kind for every caller (COR3, F-1); nothing was recorded"
            ),
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
    /// The evidence owner recorded the production of one evidence ref
    /// (plan Task 11, §14.3): the durable event behind
    /// [`StateStore::record_evidence_production`]. Its append offset IS the
    /// ordering anchor of the production binding — the monotonic fact
    /// §14.3's no-stale-evidence condition is decided against.
    EvidenceProduced {
        evidence: EvidenceRef,
        locator: EvidenceLocator,
    },
    /// A UNIT-ATTRIBUTED dispatch was admitted (corrections round 2,
    /// F-R4): the unit the invocation was FOR — the fact the §26 report's
    /// record-derived classification needs and the §5.3.5 invocation
    /// record itself cannot carry (its field set is spec-frozen and names
    /// no unit; the workspace ref is provider-shaped, never parsed) —
    /// plus the invocation identity. Appended by the runtime at dispatch,
    /// after the runner admitted the invocation.
    UnitDispatch {
        unit: UnitId,
        invocation_id: InvocationId,
    },
    /// The run recorded a terminal state (corrections round 2, F-R4 and
    /// F-R5). APPEND-ONLY by design: a resumed pass that re-terminalizes
    /// appends the new terminal beside the old one — the transition is
    /// visible in the record (and in every export), never a silent
    /// rewrite; the latest entry is the run's real final state.
    RunTerminal { state: TerminalState },
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

/// The production facts the evidence owner records when it produces one
/// evidence ref (plan Task 11; spec §14.3). The `EvidenceStore` is
/// content-addressed bytes with no metadata — producer, locator, and
/// iteration facts therefore live HERE, in a durable record the producer
/// side wrote. `produced_at` is the wall-clock provenance field (Annex B.5
/// carries it); it is NOT the recency anchor — ordering is decided by the
/// event-log offset the STORE mints when the [`RunEventKind::EvidenceProduced`]
/// event is appended, a value no caller supplies.
///
/// `content` mirrors [`ObservationBinding`]'s content-digest discipline
/// (correction round 1, B-M4): the digest of the bytes the producer
/// ingested at that locator and stored at the ref. Binding the pointer
/// alone leaves the examined content and the pinned content disconnected —
/// a store serving different bytes at the ref would change what the
/// instrument's decision certifies. The verification chain re-derives the
/// digest from the bytes the store serves and refuses anything that is not
/// the recorded content, whoever serves them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceProduction {
    evidence: EvidenceRef,
    content: Digest,
    producer: InstrumentIdentity,
    locator: EvidenceLocator,
    iteration_no: IterationNo,
    produced_at: EpochSeconds,
}

impl EvidenceProduction {
    /// The digest of the ingested content — the bytes the producer wrote at
    /// the locator and stored at the ref. (Six producer-side facts; the
    /// repo's `mint`-style allow.)
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        evidence: EvidenceRef,
        content: Digest,
        producer: InstrumentIdentity,
        locator: EvidenceLocator,
        iteration_no: IterationNo,
        produced_at: EpochSeconds,
    ) -> Self {
        Self {
            evidence,
            content,
            producer,
            locator,
            iteration_no,
            produced_at,
        }
    }

    pub fn evidence(&self) -> &EvidenceRef {
        &self.evidence
    }

    /// The §12.4-style content digest of the bytes ingested at the
    /// production's locator — what the chain re-derives from the store's
    /// bytes and compares.
    pub fn content(&self) -> &Digest {
        &self.content
    }

    /// The named instrument that produced the evidence (§14.3 condition 2).
    pub fn producer(&self) -> &InstrumentIdentity {
        &self.producer
    }

    /// The locator the evidence was produced at (§14.3 condition 4).
    pub fn locator(&self) -> &EvidenceLocator {
        &self.locator
    }

    /// The loop pass the production happened in (§14.3 condition 3's
    /// in-this-run half).
    pub fn iteration_no(&self) -> IterationNo {
        self.iteration_no
    }

    pub fn produced_at(&self) -> EpochSeconds {
        self.produced_at
    }
}

/// The durable production record: the production facts plus the run they
/// belong to and the SERVER-MINTED ordering anchor (the offset of the
/// `EvidenceProduced` event). This is what §14.3's recency decision reads —
/// a value written by the store, never an argument of any verification call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceProductionBinding {
    run_id: RunId,
    production: EvidenceProduction,
    event_log_offset: EventLogOffset,
}

impl EvidenceProductionBinding {
    pub fn new(
        run_id: RunId,
        production: EvidenceProduction,
        event_log_offset: EventLogOffset,
    ) -> Self {
        Self {
            run_id,
            production,
            event_log_offset,
        }
    }

    pub fn run_id(&self) -> &RunId {
        &self.run_id
    }

    pub fn production(&self) -> &EvidenceProduction {
        &self.production
    }

    /// The ordering anchor: the event-log offset of the durable
    /// `EvidenceProduced` event (monotonic; store-minted).
    pub fn event_log_offset(&self) -> EventLogOffset {
        self.event_log_offset
    }
}

/// One durable [`RunEventKind::EvidenceProduced`] event at or after a resume
/// anchor — the A5 crash window's durable fact the §18.3 resume-completion
/// recovery completes the missing row from (COR-R3-1). The event is
/// deliberately minimal (evidence ref and locator — see
/// [`StateStore::complete_pending_evidence_production`]'s contract): the
/// row's producer, iteration, and content-digest facts are NOT here and are
/// never fabricated from this item — the recovery re-mints them through the
/// production authority, deriving the content digest from the evidence
/// store's own bytes at [`Self::evidence`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrphanEvidenceProduction {
    evidence: EvidenceRef,
    offset: EventLogOffset,
    at: EpochSeconds,
}

impl OrphanEvidenceProduction {
    pub fn new(evidence: EvidenceRef, offset: EventLogOffset, at: EpochSeconds) -> Self {
        Self {
            evidence,
            offset,
            at,
        }
    }

    /// The evidence ref the durable event names — the ref the produce step
    /// actually recorded (in the produced-artifact world, the locator
    /// artifact's own content ref: a ref no observation facet names).
    pub fn evidence(&self) -> &EvidenceRef {
        &self.evidence
    }

    /// The orphan event's own append offset — the anchor the completed row
    /// binds to (the settlement-replay pattern).
    pub fn offset(&self) -> EventLogOffset {
        self.offset
    }

    /// The orphan event's own stored timestamp — the `produced_at` fact the
    /// completed row takes (the A3 discipline: the durable event is the
    /// authority, never the re-mint's clock).
    pub fn at(&self) -> EpochSeconds {
        self.at
    }
}

/// The §16.3 settlement commit payload: the settlement event's fields
/// verbatim (`SettlementEvent{unit_id, evidence_ref, verification_ref,
/// digest, ts}`). `at` is the event CONSTRUCTION timestamp (from the
/// `Clock` port) — the row's `settled_at` is the DURABLE event's stored
/// timestamp, never this field (plan Task 13 B1, A3: the durable event is
/// the authority; the store cross-validates the two and refuses a commit
/// whose constructed timestamp disagrees with what it stored).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettlementCommit {
    pub run_id: RunId,
    pub unit_id: UnitId,
    pub evidence: EvidenceRef,
    pub verification: VerificationRef,
    pub reference_digest: Digest,
    pub at: EpochSeconds,
}

/// The run's lifecycle facts the §26 report classifies from (corrections
/// round 2, F-R4): every unit-attributed dispatch the run recorded, every
/// invocation terminal, and the run's own recorded terminals in append
/// order. All of it is READ-side projection over the durable event log —
/// the report classifies from records, never from claims.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RunLifecycleFacts {
    /// Every `(unit, invocation)` dispatch pair, in append order — the
    /// record-derived join the §5.3.5 invocation record cannot carry.
    pub unit_dispatches: Vec<(UnitId, InvocationId)>,
    /// Every `(invocation, terminal state)` the run recorded, in append
    /// order.
    pub invocation_terminals: Vec<(InvocationId, InvocationState)>,
    /// The run's own recorded terminals, in append order; the LAST entry
    /// is the run's real final state (a resumed pass that re-terminalizes
    /// appends — the transition stays visible in the record).
    pub run_terminals: Vec<TerminalState>,
}

/// The run-state port (spec §6.2). Every method performs durable IO, so
/// every method carries a deadline (§6 preamble).
pub trait StateStore {
    /// Appends one run event durably — the ORDINARY runtime kinds only
    /// (invocation lifecycle, checkpoints, evidence production).
    ///
    /// [`RunEventKind::SettlementCommitted`] is REFUSED here for every
    /// caller with the typed
    /// [`StateStoreError::SettlementEventNotEarned`](Self::SettlementEventNotEarned):
    /// the settlement event is authority-minted, and its only producer is
    /// [`Self::commit_settlement`], which consumes an
    /// [`crate::producer_authority::AdmittedSettlement`]. A caller-built
    /// settlement event names invented facts; the §18.3 replay would mint a
    /// durable settlement row from it with no §16.1 condition ever holding
    /// (Task 13 COR3, confirmation round 1 F-1 — the same owner-write
    /// forgery the verification-record mint (COR-1) and the generic record
    /// append's kind gate (COR2-4) close on the sibling surfaces). The
    /// `EventLog` port's append refuses the kind for the same reason.
    fn append_event(
        &self,
        event: RunEvent,
        deadline: Deadline,
    ) -> Result<EventLogOffset, StateStoreError>;

    /// Loads a run's durable state; `None` when the run was never stored.
    fn load_run(&self, run_id: &RunId, deadline: Deadline) -> Result<Option<Run>, StateStoreError>;

    /// The latest run this store knows (greatest ULID — ULID order is
    /// time order); `None` when no run was ever stored. The operator
    /// surface derives its context run from the runs themselves, not
    /// only from escalation tickets (a cleanly settled run names no
    /// ticket). Default: unknown — existing doubles compile unchanged.
    fn latest_run(
        &self,
        #[allow(unused_variables)] deadline: Deadline,
    ) -> Result<Option<RunId>, StateStoreError> {
        Ok(None)
    }

    /// Records the authoritative run identity (spec §5.3.3: "a `Run` has
    /// exactly one frozen `reference_version` for its whole life"; §18.1's
    /// minimum durable records include runs). Plan Task 12's controller
    /// composition is the production caller: the governed authorities open
    /// only over a durable run, so the loop must be able to make its run
    /// durable through the same port it reads it from.
    ///
    /// Idempotent by `run_id`: re-recording the SAME identity is a no-op,
    /// and a re-recording that disagrees on any identity field is a typed
    /// error naming the disagreement — never a silent overwrite of the
    /// run's one frozen reference.
    fn record_run(&self, _run: &Run, _deadline: Deadline) -> Result<(), StateStoreError> {
        Err(StateStoreError::WriteFailed {
            detail: "this state store does not implement durable run identity recording".to_owned(),
        })
    }

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

    /// Durably records one controller correction selection (spec §15.1
    /// step 4: "Record the selection **and the rejected alternatives with
    /// reasons** on the iteration"; AK-037 is a recording claim — the
    /// record IS the obligation).
    ///
    /// The `record_role_invocation` precedent: the argument is the exact
    /// domain aggregate, and the writer is the RUNTIME itself — §16.4's
    /// single-writer discipline makes the runtime the sole writer of this
    /// record, so there is no minting authority to earn (unlike the
    /// evidence/observation owner writes, which roles must earn). The
    /// aggregate's own constructor has already validated that the
    /// selection carries its rejected alternatives and a trace naming all
    /// ten §15.1 dimensions; a store cannot be handed a selection without
    /// them.
    ///
    /// Appends one row per call (a selection is immutable once recorded —
    /// the same iteration may legitimately record a corrected selection
    /// later, which is a new row, never an edit of the old one).
    ///
    /// `at` (W4, correction task26-dcritic-apparatus-repair) is the caller's
    /// own clock instant for the selection — the runtime is the single
    /// writer (see above), so it is the runtime's clock reading, never a
    /// store-invented value. `CorrectionSelection` itself carries no
    /// timestamp field (the column IS the fact; the decoded aggregate needs
    /// none), so a store that persists `at` exposes it only through its own
    /// storage-specific read surface, if any.
    fn record_correction_selection(
        &self,
        _selection: &CorrectionSelection,
        _at: EpochSeconds,
        _deadline: Deadline,
    ) -> Result<(), StateStoreError> {
        Err(StateStoreError::WriteFailed {
            detail: "this state store does not implement durable correction selections".to_owned(),
        })
    }

    /// Every durable correction selection of one run, in record order
    /// (append order) — the read side of
    /// [`StateStore::record_correction_selection`]. A run with no
    /// selections is an empty vector, never an error.
    fn correction_selections(
        &self,
        _run_id: &RunId,
        _deadline: Deadline,
    ) -> Result<Vec<CorrectionSelection>, StateStoreError> {
        Ok(Vec::new())
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
    ///
    /// The argument is an [`AdmittedObservationBinding`], which has no
    /// constructor outside the [`crate::producer_authority`] module (plan
    /// Task 12, the `AdmittedComparison` precedent): the observation owner
    /// earns the write through
    /// [`ObservationBindingAuthority::bind`](crate::producer_authority::ObservationBindingAuthority::bind),
    /// which validates the run and the declared iteration against the
    /// durable anchor and derives the §12.4 content digest from the bytes
    /// the evidence store serves. A caller holding a hand-built
    /// [`ObservationBinding`] has nothing to pass here.
    fn bind_observation_bundle(
        &self,
        _run_id: &RunId,
        _iteration_no: IterationNo,
        _binding: &crate::producer_authority::AdmittedObservationBinding,
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

    /// Records the production of one evidence ref in one run — the
    /// evidence owner's durable fact behind every §14.3
    /// producer/locator/recency decision (plan Task 11).
    ///
    /// The store appends the durable [`RunEventKind::EvidenceProduced`]
    /// event and MINTS the ordering anchor from that append's offset; the
    /// caller supplies the production facts (including the ingested
    /// content's digest), never the ordering. The row's IDENTITY —
    /// evidence ref, content digest, producer, locator — is write-once:
    /// a record naming a DIFFERENT producer, locator, or content digest
    /// for the same ref is refused, never overwritten — a later
    /// substitute cannot displace what the evidence owner certified.
    ///
    /// The row's RECENCY is not part of that identity (task26 correction):
    /// re-recording the SAME facts at the SAME declared iteration is
    /// idempotent (the original binding returns, nothing new is appended);
    /// re-recording the IDENTICAL content/producer/locator at a LATER
    /// declared iteration ADVANCES the row's recency anchor (iteration,
    /// instant, event-log offset) in place and appends a fresh
    /// `EvidenceProduced` event — §14.3 condition 3 reads "produced ... at
    /// or after the transform it certifies", a fact about when bytes were
    /// made, not whether a row already exists; an instrument that
    /// genuinely re-ran and reproduced identical bytes this iteration made
    /// that fact true again. A declared iteration at or before the
    /// existing row's is refused exactly as a differing identity is — a
    /// stale or replayed claim never re-certifies. This is reachable only
    /// through the minting authority below, which re-reads the bytes from
    /// the store and validates the declared iteration against the run's
    /// own durable checkpoint before minting, so an advance is always
    /// backed by a real, governed re-production — never a caller's say-so.
    ///
    /// The argument is an [`AdmittedEvidenceProduction`], which has no
    /// constructor outside the [`crate::producer_authority`] module (plan
    /// Task 12, closing the residual recorded here since Task 11): the
    /// evidence owner earns the write through
    /// [`EvidenceProductionAuthority::record`](crate::producer_authority::EvidenceProductionAuthority::record),
    /// which validates the run and the declared iteration against the
    /// durable anchor and DERIVES the content digest from the bytes the
    /// evidence store serves — a caller-supplied digest has no parameter to
    /// travel through. A caller holding a hand-built [`EvidenceProduction`]
    /// has nothing to pass here.
    fn record_evidence_production(
        &self,
        _run_id: &RunId,
        _production: &crate::producer_authority::AdmittedEvidenceProduction,
        _deadline: Deadline,
    ) -> Result<EvidenceProductionBinding, StateStoreError> {
        Err(StateStoreError::WriteFailed {
            detail: "this state store does not implement durable evidence-production bindings"
                .to_owned(),
        })
    }

    /// The durable production record for one `(run, evidence ref)`;
    /// `None` when no production was ever recorded. §14.3's producer,
    /// locator, and recency facts are read from HERE — never from a
    /// verification request.
    fn evidence_production(
        &self,
        _run_id: &RunId,
        _evidence: &EvidenceRef,
        _deadline: Deadline,
    ) -> Result<Option<EvidenceProductionBinding>, StateStoreError> {
        Ok(None)
    }

    /// Every durable production record the run recorded at one locator,
    /// ordered by their anchors (oldest first). This is how the
    /// verification chain finds the evidence for
    /// `evidence_required.locator` without accepting any caller-named
    /// evidence ref.
    fn evidence_productions_at(
        &self,
        _run_id: &RunId,
        _locator: &EvidenceLocator,
        _deadline: Deadline,
    ) -> Result<Vec<EvidenceProductionBinding>, StateStoreError> {
        Ok(Vec::new())
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

    /// Records one §18.1/§5.3.6 verification decision durably: the typed
    /// Annex B.5 document, stored under the run it names. This is the
    /// durable fact every §16.1 condition-1 check reads — a settlement
    /// admission mints only over a PASS record that is durable HERE first
    /// (plan Task 13; the store is the single writer surface, the runtime
    /// its only caller, §16.4).
    ///
    /// The argument is an [`AdmittedVerificationRecord`], which has no
    /// constructor outside the [`crate::producer_authority`] module (Task 13
    /// correction round 1, closing the early probe's C1: before the gate,
    /// any in-process store holder could build a PASS document with the
    /// public `VerificationRecordDoc::new` and write it raw — forging the
    /// exact fact the settlement admission validates against, with
    /// `SettlementCommitAuthority::admit` + `commit_settlement` then
    /// completing the bypass). The recording is earned through
    /// [`VerificationRecordAuthority::record`](crate::producer_authority::VerificationRecordAuthority::record),
    /// which validates that the document's run is durable and its declared
    /// iteration is the run's durable anchor. A caller holding a hand-built
    /// document has nothing to pass here.
    ///
    /// Append-only per document; the row's `at` is the document's own
    /// `produced_at` (a provenance fact the record carries, never a value
    /// minted here).
    fn record_verification(
        &self,
        _record: &crate::producer_authority::AdmittedVerificationRecord,
        _deadline: Deadline,
    ) -> Result<(), StateStoreError> {
        Err(StateStoreError::WriteFailed {
            detail: "this state store does not implement durable verification records".to_owned(),
        })
    }

    /// The run's durable verification records, in record order (append
    /// order). Strict decode through the Annex B.5 codec: a stored payload
    /// that does not decode is `Corrupt`, never a degraded record. A run
    /// with no records reads as an empty vector, never as an error.
    ///
    /// Fails closed (COR-7, early probe m1): a store that does not
    /// implement this read refuses with the typed not-implemented error —
    /// never a silent empty that would under-inform the settlement
    /// admission and the re-entry disagreement checks.
    fn verification_records(
        &self,
        _run_id: &RunId,
        _deadline: Deadline,
    ) -> Result<Vec<VerificationRecordDoc>, StateStoreError> {
        Err(StateStoreError::ReadFailed {
            detail: "this state store does not implement durable verification records".to_owned(),
        })
    }

    /// The run's lifecycle facts (corrections round 2, F-R4) — see
    /// [`RunLifecycleFacts`]. A run with none of them reads as the empty
    /// projection, never as an error.
    ///
    /// Fails closed (the COR-7 posture this trait's report-critical reads
    /// share): a store that does not implement this read refuses with the
    /// typed not-implemented error — never a silent empty, which would
    /// let the §26 report describe an ATTEMPTED unit as never-attempted.
    fn run_lifecycle_facts(
        &self,
        _run_id: &RunId,
        _deadline: Deadline,
    ) -> Result<RunLifecycleFacts, StateStoreError> {
        Err(StateStoreError::ReadFailed {
            detail: "this state store does not implement the run lifecycle facts read".to_owned(),
        })
    }

    /// The §16.3 atomic commit sequence, in order, no reordering:
    ///
    /// 1. `EventLog.append(SettlementEvent{...})` — durable, fsync-on-append;
    /// 2. the settlement row — idempotent by `(run_id, unit_id,
    ///    verification_ref)`.
    ///
    /// The argument is an [`AdmittedSettlement`], which has no constructor
    /// outside the [`crate::producer_authority`] module (plan Task 13, the
    /// `AdmittedComparison`/owner-write precedent): the settlement is
    /// earned through
    /// [`SettlementCommitAuthority::admit`](crate::producer_authority::SettlementCommitAuthority::admit),
    /// which validates §16.1 condition 1 against DURABLE state — a PASS
    /// verification record for this (run, unit) whose evidence ref is the
    /// committed one — and refuses a key that is already committed. A
    /// caller holding a hand-built [`SettlementCommit`] has nothing to pass
    /// here.
    ///
    /// Division of labor (recorded in the Task-13 report): the AUTHORITY
    /// checks the durable preconditions above; the STORE cross-validates
    /// the row against the durable log — the claimed offset must name the
    /// settlement event that made it real, with every field and the stored
    /// event timestamp agreeing (spec §5.3.6). Both must hold; neither
    /// substitutes for the other.
    fn commit_settlement(
        &self,
        _admitted: &crate::producer_authority::AdmittedSettlement,
        _deadline: Deadline,
    ) -> Result<EventLogOffset, StateStoreError> {
        Err(StateStoreError::WriteFailed {
            detail: "this state store does not implement the 16.3 settlement commit".to_owned(),
        })
    }

    /// The run's durable settlement rows, ordered by `(unit_id,
    /// verification_ref)` — the order the export pins. A run with no
    /// settlements reads as an empty vector, never as an error.
    ///
    /// Fails closed (COR-7, early probe m1): a store that does not
    /// implement this read refuses with the typed not-implemented error —
    /// never a silent empty that would under-inform the §18.3 re-entry
    /// reconciliation and the admission's key check.
    fn settlements(
        &self,
        _run_id: &RunId,
        _deadline: Deadline,
    ) -> Result<Vec<SettlementRecord>, StateStoreError> {
        Err(StateStoreError::ReadFailed {
            detail: "this state store does not implement durable settlement rows".to_owned(),
        })
    }

    /// §18.3 resume step 1: replays the run's settlement events at or after
    /// `anchor` and completes any half-applied settlement — idempotent by
    /// `(run_id, unit_id, verification_ref)` (§16.3). Returns how many
    /// settlement rows this call added; a second call over the same range
    /// adds none.
    ///
    /// `anchor` is the §18.3 replay anchor: the run's latest checkpoint's
    /// `event_log_offset` (the caller composes `resume` → checkpoint →
    /// this method). Events BELOW the anchor are the checkpoint's own
    /// certification — a checkpoint stands at an offset only after
    /// everything at or below it was applied — and are not re-scanned; a
    /// run with no checkpoint replays from offset 0 (the whole log). The
    /// entry AT the anchor itself is re-verified idempotently (completion
    /// is keyed, so re-application is a no-op).
    ///
    /// Fails closed: a store that cannot perform the replay refuses, never
    /// reports zero half-applied settlements it did not look for.
    fn complete_pending_settlements(
        &self,
        _run_id: &RunId,
        _anchor: EventLogOffset,
        _deadline: Deadline,
    ) -> Result<usize, StateStoreError> {
        Err(StateStoreError::WriteFailed {
            detail: "this state store does not implement 18.3 settlement replay completion"
                .to_owned(),
        })
    }

    /// Completes one half-applied evidence production (plan Task 13, A5):
    /// the durable `EvidenceProduced` event exists at or after `anchor`,
    /// but the production row was never written (the crash window Task 11's
    /// error string names).
    ///
    /// The `EvidenceProduced` event is deliberately minimal — evidence ref
    /// and locator; the producer, iteration, and content-digest facts live
    /// in the ROW, which is the authority — so the row cannot be rebuilt
    /// from the event alone and no fact is ever fabricated. The missing
    /// facts arrive as the authority-MINTED [`AdmittedEvidenceProduction`]
    /// (same mint `record_evidence_production` takes, so a caller supplies
    /// producer facts and a derived content digest, never a free-form row):
    ///
    /// - a row already exists for `(run, evidence)` → `Ok(None)` — already
    ///   applied, nothing added (idempotent on the production identity);
    /// - an orphan event exists at or after the anchor whose locator agrees
    ///   with the minted production → the row is completed BOUND TO THAT
    ///   EVENT'S OFFSET (no second event appended — the settlement-replay
    ///   pattern: the row names the durable event that made it real), with
    ///   `produced_at` taken from the event's own stored timestamp (the
    ///   A3 discipline, mirrored). A second call finds the row and adds
    ///   nothing;
    /// - no orphan event → `Ok(None)`: nothing is pending; the caller
    ///   records normally through `record_evidence_production`.
    ///
    /// Fails closed: a store that cannot replay refuses, never reports
    /// "nothing pending" without looking.
    fn complete_pending_evidence_production(
        &self,
        _run_id: &RunId,
        _anchor: EventLogOffset,
        _production: &crate::producer_authority::AdmittedEvidenceProduction,
        _deadline: Deadline,
    ) -> Result<Option<EvidenceProductionBinding>, StateStoreError> {
        Err(StateStoreError::WriteFailed {
            detail: "this state store does not implement evidence-production replay completion"
                .to_owned(),
        })
    }

    /// The durable [`RunEventKind::EvidenceProduced`] events of one run at or
    /// after `anchor` naming `locator`, in offset order (oldest first) — the
    /// orphan facts the §18.3 resume-completion recovery completes a missing
    /// production row from (COR-R3-1, falsifier round 3, F-R3-1).
    ///
    /// Why the recovery needs this read: it must re-mint the missing row at
    /// the ref the produce step ACTUALLY recorded in its world. In the
    /// produced-artifact world that ref is the locator artifact's own
    /// content ref — a ref NO observation facet names — so the recovery can
    /// neither guess it from the bundle nor rebuild it from the event's
    /// minimal payload without reading it here. The returned facts are
    /// durable events only; every remaining row fact (producer, iteration,
    /// content digest) is re-minted by the recovery's authority, and
    /// [`StateStore::complete_pending_evidence_production`] cross-validates
    /// the mint against the same events before any row lands.
    ///
    /// Fails closed: a store that cannot replay refuses, never a silent
    /// empty that would under-inform the §18.3 re-entry reconciliation (the
    /// COR-7 discipline for the settlement-surface reads).
    fn pending_evidence_production_events(
        &self,
        _run_id: &RunId,
        _anchor: EventLogOffset,
        _locator: &EvidenceLocator,
        _deadline: Deadline,
    ) -> Result<Vec<OrphanEvidenceProduction>, StateStoreError> {
        Err(StateStoreError::ReadFailed {
            detail: "this state store does not implement evidence-production orphan reads"
                .to_owned(),
        })
    }

    /// Deterministic JSONL export of one run (spec §6.2: "Export is
    /// deterministic" — same run, byte-identical output).
    fn export_jsonl(&self, run_id: &RunId, deadline: Deadline) -> Result<String, StateStoreError>;
}

#[cfg(test)]
mod tests {
    use gauntlet_domain::RunId;

    use super::{StateStore, StateStoreError};
    use crate::deadline::Deadline;
    use gauntlet_domain::EpochSeconds;

    const BASE_INSTANT: u64 = 1_700_000_000;
    /// The suite's bound span, on its own line (numeric literals never share
    /// a line with a duration-shaped identifier).
    const BOUND_SPAN: u64 = 60;

    fn bound() -> Deadline {
        Deadline::at_epoch(EpochSeconds::new(BASE_INSTANT + BOUND_SPAN))
    }

    fn run_id() -> RunId {
        RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("valid run id")
    }

    /// A stub implementing ONLY the trait's required methods — every
    /// defaulted method, read or write, is the default. The write defaults
    /// already fail closed; COR-7 (early probe m1) makes the two
    /// settlement-surface READ defaults fail closed too, so a store that
    /// implements writes but not reads surfaces LOUD, never a silent
    /// empty that under-informs re-entry and the disagreement checks.
    struct WriteOnlyStub;

    impl StateStore for WriteOnlyStub {
        fn append_event(
            &self,
            _: super::RunEvent,
            _: Deadline,
        ) -> Result<gauntlet_domain::EventLogOffset, StateStoreError> {
            Err(StateStoreError::WriteFailed {
                detail: "the stub never appends".to_owned(),
            })
        }
        fn load_run(
            &self,
            _: &RunId,
            _: Deadline,
        ) -> Result<Option<gauntlet_domain::Run>, StateStoreError> {
            Ok(None)
        }
        fn checkpoint(&self, _: super::Checkpoint, _: Deadline) -> Result<(), StateStoreError> {
            Err(StateStoreError::WriteFailed {
                detail: "the stub never checkpoints".to_owned(),
            })
        }
        fn resume(
            &self,
            _: &RunId,
            _: Deadline,
        ) -> Result<Option<super::Checkpoint>, StateStoreError> {
            Ok(None)
        }
        fn export_jsonl(&self, _: &RunId, _: Deadline) -> Result<String, StateStoreError> {
            Ok(String::new())
        }
    }

    /// COR-7 fail-first: a write-only stub's settlement-surface reads must
    /// be the typed not-implemented refusal, never a silent empty.
    #[test]
    fn settlement_surface_read_defaults_fail_closed() {
        let store: &dyn StateStore = &WriteOnlyStub;
        match store.settlements(&run_id(), bound()) {
            Err(StateStoreError::ReadFailed { detail }) => assert!(
                detail.contains("does not implement"),
                "the refusal names its locus: {detail}"
            ),
            other => panic!("settlements fails closed, got {other:?}"),
        }
        match store.verification_records(&run_id(), bound()) {
            Err(StateStoreError::ReadFailed { detail }) => assert!(
                detail.contains("does not implement"),
                "the refusal names its locus: {detail}"
            ),
            other => panic!("verification_records fails closed, got {other:?}"),
        }
    }
}
