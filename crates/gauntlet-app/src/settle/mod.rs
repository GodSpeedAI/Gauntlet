//! The settlement service (plan Task 13 B2; spec §16, §18.3 step 1, §20.2)
//! — the ONLY settle path in the application layer.
//!
//! [`SettlementService::settle`] takes what the loop EARNED and grounds
//! every fact on durable state:
//!
//! - the [`VerifiedClaim`] the governed verification chain minted, and its
//!   [`SettlementReadiness`] (§16.1 conditions 1–2, already computed in the
//!   loop) — the readiness is CONSUMED by value: it is a token the
//!   precondition layer minted over the claim, never re-derived here from
//!   caller-held values;
//! - the FROZEN unit — loaded HERE from the durable run's own frozen
//!   [`ReferenceVersion`](gauntlet_domain::ReferenceVersion) through the
//!   port's digest-verifying projection, never an in-memory caller
//!   snapshot (the P4 seam's condition-3 half, grounded now);
//! - the closing context ([`ClosingActor`]) and the tracked blocking
//!   residuals.
//!
//! It then enforces, in order, with typed refusals (failure class
//! `settlement`, spec §21.2):
//!
//! 1. **condition 3** — the authority chain over the FROZEN unit's
//!    `authority` (§16.1: critic → automatic; lead_agent → orchestrator
//!    with a RECORDED rationale; human → operator action with a RECORDED
//!    identity). An in-loop automatic closure of authority the runtime
//!    cannot exercise REFUSES and routes to the escalation surface
//!    (§11.5 row 6 — escalate, never loop past it);
//! 2. **condition 4** — no blocking residual references the unit, checked
//!    against the tracked view AND the durable side (below);
//! 3. **condition 5** — a settle-time `verify_unchanged` consult against
//!    the reference PROVIDER's own file-derived answer (§9.4's
//!    "immediately before settlement" leg; NEVER a comparison against a
//!    digest the controller froze itself). A mutation persists BOTH
//!    digests as pinned evidence and refuses with the typed abort error
//!    the controller maps to the run's terminal `aborted{reference_mutated}`
//!    path; an unavailability refuses without aborting (§21.3 keeps the
//!    process alive);
//! 4. **the commit** — the verification document is durably recorded if
//!    not already (idempotent on its identity), the
//!    [`SettlementCommit`](gauntlet_ports::state_store::SettlementCommit) is
//!    admitted by the ports' `SettlementCommitAuthority` (§16.1 condition 1
//!    against durable PASS records) and committed through the store's
//!    §16.3 sequence (fsync'd event append → keyed row). `AlreadyCommitted`
//!    for the SAME `(run, unit, verification)` key is an idempotent
//!    SUCCESS returning the existing durable offset — the loop must be
//!    able to re-enter after a crash;
//! 5. **the in-memory advance is LAST** — the service applies the domain's
//!    `Unit::settle` to the FROZEN unit it loaded and returns the advanced
//!    projection beside the durable offset, so the caller's projection
//!    update is observably AFTER the durable commit (B4's commit-order
//!    proof reads exactly this).
//!
//! ## Documented mappings (recorded, not silent)
//!
//! - **The B.6 `authority_decision` shape.** The durable `settlements`
//!   schema (migration 0002, frozen — Task 13 adds NO migration) carries no
//!   `authority_decision` column, so the B.6
//!   `{required, exercised_by, operator_identity}` triple is recorded as an
//!   ACCOMPANYING pinned evidence document
//!   (`settlement_authority_decision@v1`) written at commit time, naming
//!   the run, unit, and verification the decision belongs to. The mapping
//!   is: `required` ← the frozen unit's `authority`; `exercised_by` ← the
//!   closing actor (`runtime` | `orchestrator` | `operator`);
//!   `operator_identity` ← the recorded identity when the actor is an
//!   operator.
//! - **Condition 4's durable half.** No port exposes a typed durable
//!   residual-ROW read (residuals live inside comparison payloads; a
//!   residual-row read is a ports revision, recorded as a known gap). The
//!   durable state this service CAN read is the run's durable CHECKPOINT —
//!   §5.3.3: `units_state` is "projection, rebuilt from events", and
//!   `blocked` is the status a blocking residual produces. A unit the
//!   durable projection holds BLOCKED while the tracked view reports no
//!   blocking residual is the preregistered P4 divergence: the DURABLE
//!   state wins and the divergence itself is the typed refusal cause
//!   ([`SettleError::DurableBlockDiverged`]) — never a silent
//!   tracked-view settlement.
//! - **The actor-permission table.** `critic` authority is closable by any
//!   actor (authority is a minimum bar, §16.2: it decides WHETHER closure
//!   may happen); `lead_agent` requires the orchestrator route with a
//!   recorded non-blank rationale (an operator may also close it — the
//!   escalation model's higher tier); `human` requires the operator route
//!   with a recorded identity — an orchestrator cannot close human work
//!   even with a rationale.
//!
//! ## §18.3 resume completion
//!
//! [`SettlementService::complete_pending_after_checkpoint`] is the step-1
//! composition the controller's `resume_run` calls: the anchored settlement
//! replay, the evidence-production orphan completion (re-minted from
//! DURABLE facts only — the bound observation bundle's facets name the
//! evidence refs and producer identities; the frozen units' locators are
//! the locator facts; the store binds the completed row to the orphan
//! event's own offset and timestamp; an orphan whose facts are not durably
//! recoverable is a RECORDED limitation, never a fabrication), and the
//! durable-settlement reconciliation input.

use std::sync::Arc;

use gauntlet_domain::{
    Authority, Digest, EpochSeconds, EventLogOffset, EvidenceKind, EvidenceRef, InstrumentName,
    IterationNo, Run, RunId, Unit, UnitError, UnitId, UnitStatus,
};
use gauntlet_ports::clock::Clock;
use gauntlet_ports::comparator::FrozenReferenceProjection;
use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::evidence_store::{EvidenceStore, EvidenceStoreError};
use gauntlet_ports::human_escalation::{EscalationItem, HumanEscalation, HumanEscalationError};
use gauntlet_ports::observer::InstrumentIdentity;
use gauntlet_ports::producer_authority::{
    EvidenceProductionAuthority, ProducerAuthorityError, SettlementAdmissionError,
    SettlementCommitAuthority,
};
use gauntlet_ports::reference_provider::{ReferenceProvider, ReferenceProviderError};
use gauntlet_ports::state_store::{SettlementCommit, StateStore, StateStoreError};

use crate::verify::authority::VerifiedClaim;
use crate::verify::precondition::SettlementReadiness;

/// The kind of actor closing the unit (the B.6 `exercised_by` vocabulary).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActorKind {
    /// The runtime's in-loop automatic closure.
    Runtime,
    /// The orchestrator (§16.1's `lead_agent` route).
    Orchestrator,
    /// An operator action (§16.1's `human` route; §20.2 action endpoints).
    Operator,
}

impl ActorKind {
    /// The B.6 wire token.
    pub fn token(&self) -> &'static str {
        match self {
            Self::Runtime => "runtime",
            Self::Orchestrator => "orchestrator",
            Self::Operator => "operator",
        }
    }
}

/// A recorded operator identity (§16.1 condition 3's `human` route; AK-062:
/// the identity is LOGGED, never implicit). Non-empty by construction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperatorIdentity(String);

impl OperatorIdentity {
    /// Accepts any non-blank identity string.
    pub fn new(identity: &str) -> Result<Self, OperatorIdentityError> {
        if identity.trim().is_empty() {
            return Err(OperatorIdentityError::Blank);
        }
        Ok(Self(identity.trim().to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Why an operator identity could not be recorded — typed (spec §21.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperatorIdentityError {
    /// The identity is blank — an anonymous operator action is exactly the
    /// unattributed closure §16.1 condition 3 refuses.
    Blank,
}

impl core::fmt::Display for OperatorIdentityError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Blank => write!(
                f,
                "the operator identity is blank — an operator action must be attributable (spec \
                 16.1 condition 3)"
            ),
        }
    }
}

impl std::error::Error for OperatorIdentityError {}

/// The closing context: WHO is closing the unit, with the recorded inputs
/// each §16.1 condition-3 route requires. Absence is representable (a
/// rationale-free orchestrator, an anonymous operator) because refusing it
/// is the check, not a construction error.
#[derive(Debug, Clone)]
pub enum ClosingActor {
    /// In-loop automatic closure (exercised_by: runtime).
    Automatic,
    /// Orchestrator closure — REQUIRES a recorded rationale for
    /// `lead_agent`-authority units.
    Orchestrator { rationale: Option<String> },
    /// Operator action — REQUIRES a recorded identity for `human`-authority
    /// units.
    Operator { identity: Option<OperatorIdentity> },
}

impl ClosingActor {
    /// The actor's kind (the B.6 `exercised_by` value).
    pub fn kind(&self) -> ActorKind {
        match self {
            Self::Automatic => ActorKind::Runtime,
            Self::Orchestrator { .. } => ActorKind::Orchestrator,
            Self::Operator { .. } => ActorKind::Operator,
        }
    }

    fn rationale_present(&self) -> bool {
        matches!(
            self,
            Self::Orchestrator {
                rationale: Some(rationale),
            } if !rationale.trim().is_empty()
        )
    }
}

/// One tracked blocking residual, attributed to the unit it references
/// (§16.1 condition 4's tracked view — the controller's own §13.5
/// attribution).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockingResidual {
    pub unit: UnitId,
    pub detail: String,
}

/// The settlement request: the earned claim, its CONSUMED readiness token,
/// the closing context, and the tracked blocking residuals.
pub struct SettleRequest<'a> {
    pub claim: &'a VerifiedClaim,
    /// Consumed by value: a readiness token is single-use evidence the
    /// precondition layer minted, never a value to re-derive or replay.
    pub readiness: SettlementReadiness,
    pub actor: ClosingActor,
    pub tracked_residuals: &'a [BlockingResidual],
}

/// The B.6 `authority_decision` triple recorded alongside the settlement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorityDecision {
    /// The frozen unit's `authority` (the level closure required).
    pub required: Authority,
    /// Who exercised the closure.
    pub exercised_by: ActorKind,
    /// The recorded operator identity, when the actor is an operator.
    pub operator_identity: Option<OperatorIdentity>,
    /// Whether a rationale was recorded with the closure.
    pub rationale_recorded: bool,
}

/// A completed settlement: the durable facts plus the advanced projection,
/// in commit order.
#[derive(Debug, Clone)]
pub struct SettlementOutcome {
    /// The event-log offset of the durable `SettlementCommitted` event —
    /// §16.3 step 1's own anchor, returned so the caller threads it into
    /// its offset water-mark and so the commit ORDER is observable (the
    /// event, then the row it made real, then the advance that rides this
    /// return).
    pub event_log_offset: EventLogOffset,
    /// Whether THIS call performed the durable commit. `false` means the
    /// §16.3 key was already durable and this call re-entered idempotently
    /// (the post-crash resume shape): the existing offset is returned.
    pub committed_now: bool,
    /// The B.6 authority decision that closed the unit.
    pub authority_decision: AuthorityDecision,
    /// The in-memory advance (§16.3 step 3, applied LAST): the frozen unit
    /// with the domain's own `settle` transition applied. The caller's
    /// projection update follows this, never precedes the durable commit.
    pub settled_unit: Unit,
}

/// An operator-action-shaped settlement request (§20.2, AK-062's half):
/// run, unit, and the operator's recorded identity. It deliberately
/// carries NO claim and NO readiness — see
/// [`SettlementService::settle_operator_action`].
pub struct OperatorActionRequest {
    pub run_id: RunId,
    pub unit_id: UnitId,
    pub identity: Option<OperatorIdentity>,
}

/// The §18.3 step-1 completion facts.
#[derive(Debug, Clone, Default)]
pub struct ResumeCompletion {
    /// How many half-applied settlement rows the anchored replay completed.
    pub settlements_completed: usize,
    /// The evidence-production rows completed from durable facts, each
    /// bound to its orphan event's offset.
    pub productions_completed: Vec<gauntlet_ports::state_store::EvidenceProductionBinding>,
    /// Recorded limitations: orphans whose producer facts were not durably
    /// recoverable (named, pinned as evidence, never fabricated around).
    pub limitations: Vec<String>,
}

/// Why a settlement was refused — typed, carrying the failure locus (spec
/// §21.2; failure class `settlement`). `PASS`, `FAIL`, and `INCONCLUSIVE`
/// stay three states throughout: the outcome-level distinctions live on
/// the verification records, and nothing here collapses them.
#[derive(Debug)]
pub enum SettleError {
    /// The run is not durable — nothing in it can settle.
    RunNotDurable { run_id: RunId },
    /// The unit is absent from the run's own frozen reference (the
    /// verification chain would have refused it; the service refuses it
    /// again at its own boundary).
    UnitNotFrozen {
        run_id: RunId,
        unit_id: UnitId,
        detail: String,
    },
    /// §16.1 condition 3: the closing actor cannot exercise the frozen
    /// unit's required authority. For an in-loop AUTOMATIC closure this
    /// refusal is also ROUTED to the escalation surface (§11.5 row 6) by
    /// the service before the error is returned.
    AuthorityNotPermitted {
        run_id: RunId,
        unit_id: UnitId,
        required: Authority,
        actor: ActorKind,
    },
    /// §16.1 condition 3, `lead_agent` route: the orchestrator closure
    /// arrived without a recorded rationale (absent or blank).
    RationaleRequired { run_id: RunId, unit_id: UnitId },
    /// §16.1 condition 3, `human` route: the operator action arrived
    /// without a recorded identity.
    OperatorIdentityRequired { run_id: RunId, unit_id: UnitId },
    /// §16.1 condition 4, tracked half: a blocking residual references the
    /// unit.
    BlockingResidualOpen {
        run_id: RunId,
        unit_id: UnitId,
        detail: String,
    },
    /// §16.1 condition 4, durable half (the P4 seam): the DURABLE
    /// checkpoint projection blocks the unit while the tracked view
    /// reported it clear — the durable state wins and the divergence is
    /// the refusal cause.
    DurableBlockDiverged {
        run_id: RunId,
        unit_id: UnitId,
        durable_status: UnitStatus,
    },
    /// §16.1 condition 5 / §9.4: the reference mutated between freeze and
    /// settle. BOTH digests were persisted as pinned evidence by the
    /// service BEFORE this error is returned; the controller maps it to
    /// the run's terminal `aborted{reference_mutated}` path. Boxed: two
    /// full digests plus the ids would otherwise inflate every
    /// `Result<_, SettleError>` return past the lint's large-`Err` bar.
    ReferenceMutated(Box<ReferenceMutatedRefusal>),
    /// §16.1 condition 5 / §21.3: the reference provider could not answer
    /// `verify_unchanged` at settle time. The settle is refused (missing
    /// evidence is not passing evidence, I8) WITHOUT aborting — the caller
    /// records the gap and the loop continues.
    ReferenceUnverifiable {
        run_id: RunId,
        unit_id: UnitId,
        detail: String,
    },
    /// A residual provider failure class the service does not decode.
    Reference(ReferenceProviderError),
    /// The durable verification records already hold a record with this
    /// claim's identity but DIFFERENT content — an integrity refusal,
    /// never a silent overwrite.
    VerificationRecordDisagrees {
        run_id: RunId,
        unit_id: UnitId,
        detail: String,
    },
    /// §16.1 condition 1 against DURABLE state: no PASS verification
    /// record exists for the (run, unit). The SAME variant refuses the
    /// automatic path's admission and the operator-action shape — the
    /// AK-062 no-bypass guarantee: an operator-action-shaped request
    /// lacking PASS evidence is refused by the identical typed error.
    VerificationNotPass {
        run_id: RunId,
        unit_id: UnitId,
        detail: String,
    },
    /// AK-062's honest boundary: the operator-action shape carries no
    /// chain-minted readiness, and settlement requires one (§16.1
    /// conditions 1–2 are earned by the governed verification chain, not
    /// re-derivable from an operator's say-so). The full operator surface
    /// (Task 21) re-enters through the governed chain; this service
    /// exposes no side door.
    OperatorActionRequiresEarnedReadiness { run_id: RunId, unit_id: UnitId },
    /// Any other admission refusal (store-side, typed).
    Admission(SettlementAdmissionError),
    /// A durable state operation failed.
    State(StateStoreError),
    /// The evidence store failed.
    Evidence(EvidenceStoreError),
    /// The escalation surface failed while routing an authority gap.
    Escalation(HumanEscalationError),
    /// The domain's own settle transition refused (defense in depth — the
    /// chain's PASS was already checked; a refusal here is an integrity
    /// signal, never retried past).
    DomainTransition(UnitError),
}

impl core::fmt::Display for SettleError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::RunNotDurable { run_id } => {
                write!(f, "run {run_id} is not durable; nothing in it can settle")
            }
            Self::UnitNotFrozen {
                run_id,
                unit_id,
                detail,
            } => write!(
                f,
                "unit {unit_id} is absent from run {run_id}'s frozen reference: {detail} (spec \
                 16.1 — the settling unit is the frozen one)"
            ),
            Self::AuthorityNotPermitted {
                run_id,
                unit_id,
                required,
                actor,
            } => write!(
                f,
                "unit {unit_id} of run {run_id} requires {required} authority for closure; the \
                 closing actor is {actor} (spec 16.1 condition 3)"
            ),
            Self::RationaleRequired { run_id, unit_id } => write!(
                f,
                "unit {unit_id} of run {run_id} requires lead_agent authority: the orchestrator \
                 closure must carry a recorded rationale (spec 16.1 condition 3)"
            ),
            Self::OperatorIdentityRequired { run_id, unit_id } => write!(
                f,
                "unit {unit_id} of run {run_id} requires human authority: the operator action \
                 must carry a recorded identity (spec 16.1 condition 3)"
            ),
            Self::BlockingResidualOpen {
                run_id,
                unit_id,
                detail,
            } => write!(
                f,
                "a blocking residual still references unit {unit_id} of run {run_id} (spec 16.1 \
                 condition 4): {detail}"
            ),
            Self::DurableBlockDiverged {
                run_id,
                unit_id,
                durable_status,
            } => write!(
                f,
                "the durable projection holds unit {unit_id} of run {run_id} as {durable_status} \
                 while the tracked residuals report it clear — the durable state wins and the \
                 settlement is refused (spec 16.1 condition 4; 5.3.3)"
            ),
            Self::ReferenceMutated(refusal) => write!(
                f,
                "the reference mutated before unit {} of run {} could settle: \
                 frozen {}, current {} (spec 16.1 condition 5; 9.4 — both digests \
                 are pinned evidence)",
                refusal.unit_id, refusal.run_id, refusal.expected, refusal.actual
            ),
            Self::ReferenceUnverifiable {
                run_id,
                unit_id,
                detail,
            } => write!(
                f,
                "the reference could not be re-verified before unit {unit_id} of run {run_id} \
                 settled (spec 16.1 condition 5): {detail} — refusing settlement rather than \
                 guessing (I8)"
            ),
            Self::Reference(error) => {
                write!(f, "the reference provider refused at settle time: {error}")
            }
            Self::VerificationRecordDisagrees {
                run_id,
                unit_id,
                detail,
            } => write!(
                f,
                "the durable verification records for unit {unit_id} of run {run_id} disagree \
                 with the settling claim's record: {detail}"
            ),
            Self::VerificationNotPass {
                run_id,
                unit_id,
                detail,
            } => write!(
                f,
                "no PASS verification record exists for unit {unit_id} of run {run_id} — \
                 settlement requires one (spec 16.1 condition 1): {detail}"
            ),
            Self::OperatorActionRequiresEarnedReadiness { run_id, unit_id } => write!(
                f,
                "the operator action for unit {unit_id} of run {run_id} carries no chain-earned \
                 readiness; settlement is earned through the governed verification chain, never \
                 re-derived from an operator's request (spec 16.1, 20.2; AK-062)"
            ),
            Self::Admission(error) => {
                write!(f, "the settlement admission refused: {error}")
            }
            Self::State(error) => write!(f, "a durable settlement step failed: {error}"),
            Self::Evidence(error) => write!(f, "the evidence store failed: {error}"),
            Self::Escalation(error) => write!(f, "routing an authority gap failed: {error}"),
            Self::DomainTransition(error) => {
                write!(f, "the domain refused the settle transition: {error}")
            }
        }
    }
}

impl std::error::Error for SettleError {}

/// The boxed payload of [`SettleError::ReferenceMutated`]: the failing
/// locus plus both digests (§9.4's abort evidence).
#[derive(Debug)]
pub struct ReferenceMutatedRefusal {
    pub run_id: RunId,
    pub unit_id: UnitId,
    pub expected: Digest,
    pub actual: Digest,
}

impl From<StateStoreError> for SettleError {
    fn from(source: StateStoreError) -> Self {
        Self::State(source)
    }
}

impl From<EvidenceStoreError> for SettleError {
    fn from(source: EvidenceStoreError) -> Self {
        Self::Evidence(source)
    }
}

impl From<HumanEscalationError> for SettleError {
    fn from(source: HumanEscalationError) -> Self {
        Self::Escalation(source)
    }
}

impl From<SettlementAdmissionError> for SettleError {
    fn from(source: SettlementAdmissionError) -> Self {
        match source {
            // The same gate, the same variant, whichever caller hit it.
            SettlementAdmissionError::VerificationNotPass {
                run_id,
                unit_id,
                detail,
            } => Self::VerificationNotPass {
                run_id,
                unit_id,
                detail,
            },
            other => Self::Admission(other),
        }
    }
}

impl core::fmt::Display for ActorKind {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.token())
    }
}

/// The settlement service — the only settle path in the application layer
/// (spec §4.1 `SettlementService`; §16 is its contract).
#[derive(Clone)]
pub struct SettlementService {
    state: Arc<dyn StateStore>,
    evidence: Arc<dyn EvidenceStore>,
    reference: Arc<dyn ReferenceProvider>,
    escalation: Arc<dyn HumanEscalation>,
    clock: Arc<dyn Clock>,
    commit_authority: SettlementCommitAuthority,
    production_authority: EvidenceProductionAuthority,
}

impl SettlementService {
    /// Composes the service over the same ports the controller holds. The
    /// commit and production authorities are built HERE from the state
    /// store (the Task-12/13 precedent: the controller never assembles an
    /// authority itself, it composes the service that owns one).
    pub fn new(
        state: Arc<dyn StateStore>,
        evidence: Arc<dyn EvidenceStore>,
        reference: Arc<dyn ReferenceProvider>,
        escalation: Arc<dyn HumanEscalation>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            commit_authority: SettlementCommitAuthority::new(state.clone()),
            production_authority: EvidenceProductionAuthority::new(state.clone(), evidence.clone()),
            state,
            evidence,
            reference,
            escalation,
            clock,
        }
    }

    /// Settles one unit: conditions 3/4/5 in order, then the durable
    /// §16.3 commit, then (as the return value) the in-memory advance.
    pub fn settle(
        &self,
        request: SettleRequest<'_>,
        deadline: Deadline,
    ) -> Result<SettlementOutcome, SettleError> {
        let record = request.claim.record();
        let run_id = record.run_id().clone();
        let unit_id = record.unit_id().clone();
        // Durable grounding: the run and its FROZEN unit — never a caller
        // snapshot of either.
        let run = self
            .state
            .load_run(&run_id, deadline)?
            .ok_or_else(|| SettleError::RunNotDurable {
                run_id: run_id.clone(),
            })?;
        let unit = self.frozen_unit(&run, &unit_id, deadline)?;
        // Condition 3 — the authority chain over the frozen unit.
        let decision =
            self.check_authority(&run_id, &unit_id, unit.authority(), &request.actor, deadline)?;
        // Condition 4 — blocking residuals (tracked, then durable).
        self.check_blocking_residuals(&run_id, &unit_id, request.tracked_residuals, deadline)?;
        // Condition 5 — the settle-time reference re-verify.
        self.settle_time_verify(&run, &unit_id, deadline)?;
        // The commit: the verification document durable first (idempotent
        // on its identity), then the admitted §16.3 sequence.
        self.ensure_verification_recorded(request.claim, deadline)?;
        let commit = SettlementCommit {
            run_id: run_id.clone(),
            unit_id: unit_id.clone(),
            evidence: request.readiness.evidence().clone(),
            verification: request.readiness.verification().clone(),
            reference_digest: run.reference_version().digest().clone(),
            at: self.clock.now(),
        };
        let (offset, committed_now) = match self.commit_authority.admit(commit, deadline) {
            Ok(admitted) => (self.state.commit_settlement(&admitted, deadline)?, true),
            // §16.3 recovery: re-entry of the SAME key is idempotent
            // success carrying the existing durable facts.
            Err(SettlementAdmissionError::AlreadyCommitted {
                event_log_offset, ..
            }) => (event_log_offset, false),
            Err(other) => return Err(other.into()),
        };
        // The B.6 authority_decision accompanying record (documented
        // mapping: the settlements schema carries no such column).
        self.record_authority_decision(&run_id, &unit_id, request.readiness.verification(), &decision, deadline)?;
        // In-memory advance LAST (§16.3 step 3): the domain's own gate over
        // the frozen unit; the caller's projection update follows the
        // return, never precedes the durable commit.
        let mut settled = unit;
        settled
            .settle(
                run_id.clone(),
                request.readiness.evidence().clone(),
                request.claim.domain_record(),
            )
            .map_err(SettleError::DomainTransition)?;
        Ok(SettlementOutcome {
            event_log_offset: offset,
            committed_now,
            authority_decision: decision,
            settled_unit: settled,
        })
    }

    /// The operator-action-shaped settle request (§20.2, AK-062's
    /// service-level half). The service treats every caller identically:
    /// the request hits the SAME durable-PASS gate first — lacking PASS
    /// evidence it is refused with the SAME
    /// [`SettleError::VerificationNotPass`] the automatic path's admission
    /// produces — and with PASS evidence present it STILL cannot settle,
    /// because settlement requires the chain-minted readiness this request
    /// shape cannot carry (the full operator surface is Task 21). There is
    /// no operator bypass at this layer, structurally.
    pub fn settle_operator_action(
        &self,
        request: OperatorActionRequest,
        deadline: Deadline,
    ) -> Result<(), SettleError> {
        self.state
            .load_run(&request.run_id, deadline)?
            .ok_or_else(|| SettleError::RunNotDurable {
                run_id: request.run_id.clone(),
            })?;
        let records = self
            .state
            .verification_records(&request.run_id, deadline)?;
        let unit_records: Vec<_> = records
            .iter()
            .filter(|record| record.unit_id() == &request.unit_id)
            .collect();
        let detail = if unit_records.is_empty() {
            format!(
                "the run holds no verification record for unit {} at all",
                request.unit_id
            )
        } else {
            let outcomes: Vec<String> = unit_records
                .iter()
                .map(|record| record.outcome().to_string())
                .collect();
            format!(
                "the run's records for unit {} hold outcomes {} — none is PASS",
                request.unit_id,
                outcomes.join(", ")
            )
        };
        if !unit_records
            .iter()
            .any(|record| record.outcome() == gauntlet_domain::Outcome::Pass)
        {
            return Err(SettleError::VerificationNotPass {
                run_id: request.run_id,
                unit_id: request.unit_id,
                detail,
            });
        }
        if request.identity.is_none() {
            return Err(SettleError::OperatorIdentityRequired {
                run_id: request.run_id,
                unit_id: request.unit_id,
            });
        }
        Err(SettleError::OperatorActionRequiresEarnedReadiness {
            run_id: request.run_id,
            unit_id: request.unit_id,
        })
    }

    /// §18.3 step 1: complete the half-applied durable writes at or after
    /// the checkpoint anchor. See the module doc's resume-completion
    /// section for the recovery mapping and its recorded limitations.
    pub fn complete_pending_after_checkpoint(
        &self,
        run_id: &RunId,
        anchor: EventLogOffset,
        deadline: Deadline,
    ) -> Result<ResumeCompletion, SettleError> {
        let mut completion = ResumeCompletion::default();
        completion.settlements_completed =
            self.state.complete_pending_settlements(run_id, anchor, deadline)?;
        // The evidence-production orphans: only the CURRENT durable
        // iteration's writes can sit at/after the anchor (the top-of-k
        // checkpoint is the last event below it), and their facts are
        // recoverable only from the bound bundle's facets plus the frozen
        // units' locators.
        let durable = self.state.resume(run_id, deadline)?.map_or_else(
            || IterationNo::new(1).expect("iterations are 1-based"),
            |checkpoint| checkpoint.iteration_no,
        );
        let run = self
            .state
            .load_run(run_id, deadline)?
            .ok_or_else(|| SettleError::RunNotDurable {
                run_id: run_id.clone(),
            })?;
        let Some(binding) = self.state.observation_binding(run_id, durable, deadline)? else {
            return Ok(completion);
        };
        let facets = match self.bundle_facets(binding.evidence(), deadline) {
            Ok(facets) => facets,
            Err(detail) => {
                self.pin_limitation(
                    run_id,
                    &format!("the bound observation bundle is unusable: {detail}"),
                    deadline,
                )?;
                completion
                    .limitations
                    .push("the bound observation bundle is unusable".to_owned());
                return Ok(completion);
            }
        };
        let units = self.frozen_units(&run, deadline).map_err(|detail| {
            SettleError::UnitNotFrozen {
                run_id: run.id().clone(),
                // The §5.2 grammar-safe placeholder (the run's frozen
                // ruler could not be parsed at all — no real unit id
                // exists to name).
                unit_id: UnitId::new("AK-000")
                    .expect("the grammar-safe placeholder unit id is well formed"),
                detail,
            }
        })?;
        for facet in facets {
            let Ok(evidence_ref) = EvidenceRef::new(facet.evidence_ref()) else {
                let detail =
                    format!("the facet's evidence ref {:?} is not a valid ref", facet.evidence_ref());
                self.pin_limitation(run_id, &detail, deadline)?;
                completion.limitations.push(detail);
                continue;
            };
            // Already applied: nothing pending for this facet.
            if self
                .state
                .evidence_production(run_id, &evidence_ref, deadline)?
                .is_some()
            {
                continue;
            }
            let mut recovered = false;
            let mut mint_refused: Option<String> = None;
            for unit in &units {
                let locator = unit.evidence_required().locator().clone();
                let Ok(name) = InstrumentName::new(facet.instrument_name()) else {
                    mint_refused = Some(format!(
                        "the facet's instrument name {:?} is not a valid instrument name",
                        facet.instrument_name()
                    ));
                    break;
                };
                let instrument =
                    InstrumentIdentity::new(name, facet.instrument_version().to_owned());
                let produced_at =
                    EpochSeconds::new(facet.ended_at().unwrap_or_else(|| facet.started_at().unwrap_or(0)));
                match self.production_authority.mint(
                    run_id,
                    durable,
                    &evidence_ref,
                    &instrument,
                    &locator,
                    produced_at,
                    deadline,
                ) {
                    Ok(minted) => {
                        match self.state.complete_pending_evidence_production(
                            run_id,
                            anchor,
                            &minted,
                            deadline,
                        ) {
                            Ok(Some(binding)) => {
                                completion.productions_completed.push(binding);
                                recovered = true;
                                break;
                            }
                            // No orphan event at this locator (or the row
                            // appeared): try the next unit's locator.
                            Ok(None) => {}
                            // The store's own cross-validation: this
                            // orphan belongs to a different production —
                            // the next frozen locator may be the agreeing
                            // one.
                            Err(StateStoreError::WriteFailed { .. }) => {}
                            Err(other) => return Err(other.into()),
                        }
                    }
                    // The bytes the facet names are not in the store: no
                    // locator can recover them — a recorded limitation,
                    // never a fabricated digest.
                    Err(ProducerAuthorityError::EvidenceAbsent { .. })
                    | Err(ProducerAuthorityError::EvidenceUnreadable { .. }) => {
                        mint_refused = Some(format!(
                            "the facet's evidence {} does not resolve in the evidence store",
                            evidence_ref
                        ));
                        break;
                    }
                    // The durable anchor does not support the orphan's
                    // iteration — honest limitation, recorded.
                    Err(ProducerAuthorityError::IterationUnsupported { .. }) => {
                        mint_refused = Some(format!(
                            "the durable iteration anchor does not support re-minting {}",
                            evidence_ref
                        ));
                        break;
                    }
                    Err(ProducerAuthorityError::State(state_error)) => {
                        return Err(state_error.into());
                    }
                    Err(other) => {
                        return Err(SettleError::State(StateStoreError::WriteFailed {
                            detail: format!("minting the orphan production failed: {other}"),
                        }));
                    }
                }
            }
            if !recovered {
                let detail = mint_refused.unwrap_or_else(|| {
                    format!(
                        "no frozen locator agrees with the orphan EvidenceProduced event for \
                         {evidence_ref} — its locator is not durably recoverable"
                    )
                });
                self.pin_limitation(run_id, &detail, deadline)?;
                completion.limitations.push(detail);
            }
        }
        Ok(completion)
    }

    /// The frozen unit from the durable run's own reference version — the
    /// condition-3 input (the P4 seam's durable-side grounding).
    fn frozen_unit(&self, run: &Run, unit_id: &UnitId, deadline: Deadline) -> Result<Unit, SettleError> {
        let units = self.frozen_units(run, deadline).map_err(|detail| {
            SettleError::UnitNotFrozen {
                run_id: run.id().clone(),
                unit_id: unit_id.clone(),
                detail,
            }
        })?;
        units
            .into_iter()
            .find(|unit| unit.id() == unit_id)
            .ok_or_else(|| SettleError::UnitNotFrozen {
                run_id: run.id().clone(),
                unit_id: unit_id.clone(),
                detail: format!(
                    "no block with that id exists in the frozen reference (digest {})",
                    run.reference_version().digest()
                ),
            })
    }

    /// The frozen reference's unit list (the controller's `frozen_units`
    /// rule, mirrored: digest-verified bytes at the version's pinned
    /// evidence ref, parsed by this crate's own answer-key parser).
    fn frozen_units(&self, run: &Run, deadline: Deadline) -> Result<Vec<Unit>, String> {
        let projection = FrozenReferenceProjection::load_version(
            run.reference_version(),
            self.evidence.as_ref(),
            deadline,
        )
        .map_err(|error| format!("the frozen projection refused to load: {error}"))?;
        Ok(crate::reference::parse::parse_answer_key(projection.view()).units)
    }

    /// §16.1 condition 3. The permission table is the module doc's; the
    /// in-loop automatic refusals are ROUTED to the escalation surface
    /// (§11.5 row 6) before the typed error returns.
    fn check_authority(
        &self,
        run_id: &RunId,
        unit_id: &UnitId,
        required: Authority,
        actor: &ClosingActor,
        deadline: Deadline,
    ) -> Result<AuthorityDecision, SettleError> {
        let permitted = match (required, actor) {
            // Critic authority is closable by any actor.
            (Authority::Critic, _) => true,
            (Authority::LeadAgent, ClosingActor::Automatic)
            | (Authority::Human, ClosingActor::Automatic) => {
                self.escalate_authority_gap(run_id, unit_id, required, ActorKind::Runtime, deadline)?;
                false
            }
            (Authority::LeadAgent, ClosingActor::Orchestrator { .. }) => actor.rationale_present(),
            (Authority::LeadAgent, ClosingActor::Operator { identity }) => identity.is_some(),
            // An orchestrator cannot close human work, rationale or not.
            (Authority::Human, ClosingActor::Orchestrator { .. }) => false,
            (Authority::Human, ClosingActor::Operator { identity }) => identity.is_some(),
        };
        if !permitted {
            return Err(match (required, actor) {
                (Authority::LeadAgent, ClosingActor::Orchestrator { .. })
                    if !actor.rationale_present() =>
                {
                    SettleError::RationaleRequired {
                        run_id: run_id.clone(),
                        unit_id: unit_id.clone(),
                    }
                }
                // The operator route without a recorded identity is its own
                // refused condition (the §16.1 identity requirement), never a
                // generic not-permitted — same discipline as the rationale.
                (_, ClosingActor::Operator { identity: None }) => {
                    SettleError::OperatorIdentityRequired {
                        run_id: run_id.clone(),
                        unit_id: unit_id.clone(),
                    }
                }
                _ => SettleError::AuthorityNotPermitted {
                    run_id: run_id.clone(),
                    unit_id: unit_id.clone(),
                    required,
                    actor: actor.kind(),
                },
            });
        }
        Ok(AuthorityDecision {
            required,
            exercised_by: actor.kind(),
            operator_identity: match actor {
                ClosingActor::Operator { identity } => identity.clone(),
                _ => None,
            },
            rationale_recorded: actor.rationale_present(),
        })
    }

    /// §11.5 row 6's routing for an in-loop automatic closure the runtime
    /// cannot exercise: pinned evidence + the escalation ticket. Escalate,
    /// never loop past it.
    fn escalate_authority_gap(
        &self,
        run_id: &RunId,
        unit_id: &UnitId,
        required: Authority,
        actor: ActorKind,
        deadline: Deadline,
    ) -> Result<(), SettleError> {
        let summary = format!(
            "unit {unit_id} of run {run_id} requires {required} authority for closure; the \
             in-loop actor is {actor} — settlement refused and routed to the operator (spec \
             16.1 condition 3, 11.5 row 6)"
        );
        let record = format!(
            "{{\"class\":\"settlement_authority_gap\",\"run\":\"{run_id}\",\"unit\":\"{unit_id}\",\"required\":\"{required}\",\"exercised_by\":\"{}\"}}",
            actor.token()
        );
        let context = self
            .evidence
            .put(record.as_bytes(), EvidenceKind::Log, deadline)?;
        self.evidence.pin(run_id, &context, deadline)?;
        self.escalation.escalate(
            EscalationItem {
                run_id: run_id.clone(),
                summary,
                context,
            },
            deadline,
        )?;
        Ok(())
    }

    /// §16.1 condition 4: the tracked view, then the durable projection.
    /// On divergence the DURABLE state wins and the divergence is the
    /// refusal cause.
    fn check_blocking_residuals(
        &self,
        run_id: &RunId,
        unit_id: &UnitId,
        tracked: &[BlockingResidual],
        deadline: Deadline,
    ) -> Result<(), SettleError> {
        if let Some(blocking) = tracked
            .iter()
            .find(|residual| &residual.unit == unit_id)
        {
            return Err(SettleError::BlockingResidualOpen {
                run_id: run_id.clone(),
                unit_id: unit_id.clone(),
                detail: blocking.detail.clone(),
            });
        }
        if let Some(checkpoint) = self.state.resume(run_id, deadline)? {
            if let Some((_, status)) = checkpoint
                .unit_states
                .iter()
                .find(|(unit, _)| unit == unit_id)
            {
                if matches!(status, UnitStatus::Blocked) {
                    return Err(SettleError::DurableBlockDiverged {
                        run_id: run_id.clone(),
                        unit_id: unit_id.clone(),
                        durable_status: *status,
                    });
                }
            }
        }
        Ok(())
    }

    /// §16.1 condition 5 / §9.4: the settle-time `verify_unchanged`
    /// consult against the PROVIDER's file-derived answer. A mutation
    /// persists BOTH digests as pinned evidence before refusing with the
    /// typed abort error; an unavailability refuses without aborting.
    fn settle_time_verify(
        &self,
        run: &Run,
        unit_id: &UnitId,
        deadline: Deadline,
    ) -> Result<(), SettleError> {
        match self
            .reference
            .verify_unchanged(run.reference_version(), deadline)
        {
            Ok(()) => Ok(()),
            Err(ReferenceProviderError::ReferenceMutated { expected, actual }) => {
                let record = format!(
                    "{{\"run\":\"{}\",\"unit\":\"{unit_id}\",\"class\":\"reference_mutated\",\"frozen_digest\":\"{expected}\",\"current_digest\":\"{actual}\"}}",
                    run.id()
                );
                let evidence =
                    self.evidence
                        .put(record.as_bytes(), EvidenceKind::Log, deadline)?;
                self.evidence.pin(run.id(), &evidence, deadline)?;
                Err(SettleError::ReferenceMutated(Box::new(
                    ReferenceMutatedRefusal {
                        run_id: run.id().clone(),
                        unit_id: unit_id.clone(),
                        expected,
                        actual,
                    },
                )))
            }
            Err(ReferenceProviderError::Unavailable { detail }) => {
                Err(SettleError::ReferenceUnverifiable {
                    run_id: run.id().clone(),
                    unit_id: unit_id.clone(),
                    detail,
                })
            }
            Err(other) => Err(SettleError::Reference(other)),
        }
    }

    /// The verification document is durable before the admission reads it:
    /// idempotent on its identity (an equal record already present adds
    /// nothing), refused when the same identity holds DIFFERENT content.
    fn ensure_verification_recorded(
        &self,
        claim: &VerifiedClaim,
        deadline: Deadline,
    ) -> Result<(), SettleError> {
        let record = claim.record();
        let existing = self.state.verification_records(record.run_id(), deadline)?;
        if existing.iter().any(|stored| stored == record) {
            return Ok(());
        }
        let identity_of = |stored: &gauntlet_ports::verification_record::VerificationRecordDoc| {
            (stored.unit_id().clone(), stored.evidence().clone(), stored.outcome())
        };
        if let Some(stored) = existing
            .iter()
            .find(|stored| identity_of(stored) == identity_of(record))
        {
            return Err(SettleError::VerificationRecordDisagrees {
                run_id: record.run_id().clone(),
                unit_id: record.unit_id().clone(),
                detail: format!(
                    "the durable record ({}) and the settling claim's record share the identity \
                     (unit, evidence, outcome) but differ in content",
                    stored.detail()
                ),
            });
        }
        self.state.record_verification(record, deadline)?;
        Ok(())
    }

    /// The B.6 `authority_decision` accompanying record: pinned evidence
    /// naming the run, unit, and verification the decision closed.
    fn record_authority_decision(
        &self,
        run_id: &RunId,
        unit_id: &UnitId,
        verification: &gauntlet_domain::VerificationRef,
        decision: &AuthorityDecision,
        deadline: Deadline,
    ) -> Result<(), SettleError> {
        let operator = decision
            .operator_identity
            .as_ref()
            .map(|identity| format!("\"{}\"", identity.as_str()))
            .unwrap_or_else(|| "null".to_owned());
        let record = format!(
            "{{\"schema\":\"settlement_authority_decision@v1\",\"run\":\"{run_id}\",\"unit\":\"{unit_id}\",\"verification\":\"{verification}\",\"required\":\"{}\",\"exercised_by\":\"{}\",\"operator_identity\":{operator},\"rationale_recorded\":{}}}",
            decision.required,
            decision.exercised_by.token(),
            decision.rationale_recorded
        );
        let evidence = self
            .evidence
            .put(record.as_bytes(), EvidenceKind::Log, deadline)?;
        self.evidence.pin(run_id, &evidence, deadline)?;
        Ok(())
    }

    /// The bound bundle's facet list, decoded from the evidence store's
    /// own bytes.
    fn bundle_facets(
        &self,
        bundle: &EvidenceRef,
        deadline: Deadline,
    ) -> Result<Vec<crate::observation::bundle::FacetRecord>, String> {
        let bytes = self
            .evidence
            .get(bundle, deadline)
            .map_err(|error| format!("reading the bundle failed: {error}"))?
            .ok_or_else(|| format!("the bundle {bundle} does not resolve in the evidence store"))?;
        let text = String::from_utf8(bytes)
            .map_err(|error| format!("the bundle bytes are not UTF-8: {error}"))?;
        let doc = crate::observation::bundle::ObservationBundleDoc::decode(&text)
            .map_err(|error| format!("the bundle does not decode: {error}"))?;
        Ok(doc.facets().to_vec())
    }

    /// Pins one §18.3 recovery limitation as durable evidence — a FACT,
    /// never silence and never a fabrication.
    fn pin_limitation(
        &self,
        run_id: &RunId,
        detail: &str,
        deadline: Deadline,
    ) -> Result<(), SettleError> {
        let record = format!(
            "{{\"run\":\"{run_id}\",\"class\":\"evidence_production_orphan_unrecoverable\",\"detail\":\"{}\"}}",
            crate::control::controller::escape_json_text(detail)
        );
        let evidence = self
            .evidence
            .put(record.as_bytes(), EvidenceKind::Log, deadline)?;
        self.evidence.pin(run_id, &evidence, deadline)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::arc_with_non_send_sync)]

    use std::cell::{Cell, RefCell};
    use std::path::PathBuf;
    use std::sync::Arc;

    use super::{
        ActorKind, BlockingResidual, ClosingActor, OperatorActionRequest, OperatorIdentity,
        ResumeCompletion, SettlementService, SettleError, SettleRequest,
    };
    use crate::observation::bundle::{BundleContent, ClaimsFacet, FacetRecord, ObservationBundleDoc};
    use crate::verify::authority::{VerificationAuthority, VerifiedClaim};
    use crate::verify::ladder::{Ladder, LadderRung, RungKind};
    use crate::verify::precondition::{SettlementReadiness, settlement_precondition};
    use crate::verify::testing::{FakeDeterministicVerifier, PinningEvidence, SeededIds};
    use gauntlet_domain::{
        Authority, Budget, DiagnosticAllowance, Digest, EpochSeconds, EvidenceKind, EvidenceLocator,
        EvidenceRef, InstrumentName, IterationNo, Limits, MicroUsd, Outcome, ReferenceId,
        ReferenceVersion, Run, RunDefinition, RunId, UnitId, UnitStatus,
    };
    use gauntlet_ports::clock::{Clock, ClockError};
    use gauntlet_ports::deadline::Deadline;
    use gauntlet_ports::evidence_store::EvidenceStore as _;
    use gauntlet_ports::human_escalation::{
        EscalationItem, EscalationState, EscalationTicket, HumanEscalation, HumanEscalationError,
        TicketId,
    };
    use gauntlet_ports::observer::{FacetKind, InstrumentIdentity, ObservationFacet};
    use gauntlet_ports::reference_provider::{
        ReferenceDraft, ReferenceProvider, ReferenceProviderError,
    };
    use gauntlet_ports::state_store::{
        Checkpoint, EvidenceProduction, EvidenceProductionBinding, ObservationBinding, RunEvent,
        RunEventKind, StateStore, StateStoreError,
    };
    use gauntlet_ports::verifier::VerificationRequest;

    const BASE_INSTANT: u64 = 1_700_000_000;
    const RUN: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";

    /// The fixture ruler: one unit per §16.1-condition-3 authority level,
    /// each requiring an instrument-produced test report at its own locator.
    const KEY: &str = "## AK-9501\nclaim: the critic fixture claim\nverify_by: \
         test\nblast_radius: contained\nauthority: critic\nevidence_required: {kind: \
         test_report, locator: settle-9501.json, producer: instrument}\n\n## AK-9502\nclaim: the \
         lead-agent fixture claim\nverify_by: test\nblast_radius: contained\nauthority: \
         lead_agent\nevidence_required: {kind: test_report, locator: settle-9502.json, producer: \
         instrument}\n\n## AK-9503\nclaim: the human fixture claim\nverify_by: \
         human_signoff\nblast_radius: contained\nauthority: human\nevidence_required: {kind: \
         test_report, locator: settle-9503.json, producer: instrument}\n";

    fn run_id() -> RunId {
        RunId::new(RUN).expect("fixture run id")
    }

    fn unit_id(unit: &str) -> UnitId {
        UnitId::new(unit).expect("fixture unit id")
    }

    fn locator_of(unit: &str) -> &'static str {
        match unit {
            "AK-9502" => "settle-9502.json",
            "AK-9503" => "settle-9503.json",
            _ => "settle-9501.json",
        }
    }

    fn bound() -> Deadline {
        Deadline::at_epoch(EpochSeconds::new(BASE_INSTANT + 90))
    }

    /// The AK-040 matrix's discriminator: each variant's own name.
    impl SettleError {
        fn variant_name(&self) -> String {
            let name = match self {
                Self::RunNotDurable { .. } => "RunNotDurable",
                Self::UnitNotFrozen { .. } => "UnitNotFrozen",
                Self::AuthorityNotPermitted { .. } => "AuthorityNotPermitted",
                Self::RationaleRequired { .. } => "RationaleRequired",
                Self::OperatorIdentityRequired { .. } => "OperatorIdentityRequired",
                Self::BlockingResidualOpen { .. } => "BlockingResidualOpen",
                Self::DurableBlockDiverged { .. } => "DurableBlockDiverged",
                Self::ReferenceMutated { .. } => "ReferenceMutated",
                Self::ReferenceUnverifiable { .. } => "ReferenceUnverifiable",
                Self::Reference(..) => "Reference",
                Self::VerificationRecordDisagrees { .. } => "VerificationRecordDisagrees",
                Self::VerificationNotPass { .. } => "VerificationNotPass",
                Self::OperatorActionRequiresEarnedReadiness { .. } => {
                    "OperatorActionRequiresEarnedReadiness"
                }
                Self::Admission(..) => "Admission",
                Self::State(..) => "State",
                Self::Evidence(..) => "Evidence",
                Self::Escalation(..) => "Escalation",
                Self::DomainTransition(..) => "DomainTransition",
            };
            name.to_owned()
        }
    }

    fn instrument() -> InstrumentIdentity {
        InstrumentIdentity::new(
            InstrumentName::new("settle-fixture-verifier").expect("plain token"),
            "1".to_owned(),
        )
    }

    // ------------------------------------------------------------------
    // Fakes
    // ------------------------------------------------------------------

    struct FixedClock(Cell<u64>);

    impl Clock for FixedClock {
        fn now(&self) -> EpochSeconds {
            EpochSeconds::new(self.0.get())
        }
        fn sleep_until(&self, at: EpochSeconds) -> Result<(), ClockError> {
            if at.get() > self.0.get() {
                self.0.set(at.get());
            }
            Ok(())
        }
    }

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum ProviderMode {
        Verifies,
        Mutated,
        Unavailable,
    }

    struct ScriptedProvider {
        version: ReferenceVersion,
        mode: Cell<ProviderMode>,
    }

    impl ReferenceProvider for ScriptedProvider {
        fn resolve(
            &self,
            _: &str,
            _: Deadline,
        ) -> Result<ReferenceDraft, ReferenceProviderError> {
            Err(ReferenceProviderError::Unavailable {
                detail: "the settle fixture never resolves".to_owned(),
            })
        }
        fn normalize(
            &self,
            _: ReferenceDraft,
            _: Deadline,
        ) -> Result<gauntlet_domain::Reference, ReferenceProviderError> {
            Err(ReferenceProviderError::Unavailable {
                detail: "the settle fixture never normalizes".to_owned(),
            })
        }
        fn digest(
            &self,
            _: &gauntlet_domain::Reference,
            _: Deadline,
        ) -> Result<Digest, ReferenceProviderError> {
            Err(ReferenceProviderError::Unavailable {
                detail: "the settle fixture never digests".to_owned(),
            })
        }
        fn freeze(
            &self,
            _: &RunId,
            _: &gauntlet_domain::Reference,
            _: Deadline,
        ) -> Result<ReferenceVersion, ReferenceProviderError> {
            Err(ReferenceProviderError::Unavailable {
                detail: "the settle fixture never freezes".to_owned(),
            })
        }
        fn verify_unchanged(
            &self,
            _: &ReferenceVersion,
            _: Deadline,
        ) -> Result<(), ReferenceProviderError> {
            match self.mode.get() {
                ProviderMode::Verifies => Ok(()),
                ProviderMode::Mutated => Err(ReferenceProviderError::ReferenceMutated {
                    expected: self.version.digest().clone(),
                    actual: crate::verify::content_digest(b"the mutated ruler bytes"),
                }),
                ProviderMode::Unavailable => Err(ReferenceProviderError::Unavailable {
                    detail: "the fixture provider is unavailable".to_owned(),
                }),
            }
        }
    }

    #[derive(Default)]
    struct RecordingEscalation {
        tickets: RefCell<Vec<String>>,
    }

    impl HumanEscalation for RecordingEscalation {
        fn escalate(
            &self,
            item: EscalationItem,
            _: Deadline,
        ) -> Result<EscalationTicket, HumanEscalationError> {
            self.tickets.borrow_mut().push(item.summary);
            Ok(EscalationTicket::new(
                TicketId::new("ticket-9").expect("ticket id"),
                EpochSeconds::new(BASE_INSTANT),
            ))
        }
        fn poll(
            &self,
            _: &EscalationTicket,
            _: Deadline,
        ) -> Result<EscalationState, HumanEscalationError> {
            Ok(EscalationState::Open)
        }
    }

    /// The durable substrate the service composes: the run, its checkpoint
    /// (the §18.3 anchor and the condition-4 durable projection), the event
    /// log with real offsets, verification records, keyed settlement rows,
    /// production rows, and the iteration's observation binding — keyed and
    /// replayed the way the real adapter keys and replays them.
    struct SettleState {
        run: Option<Run>,
        workspace: PathBuf,
        checkpoint: RefCell<Option<Checkpoint>>,
        events: RefCell<Vec<(u64, RunEvent)>>,
        next_offset: Cell<u64>,
        verifications: RefCell<Vec<gauntlet_ports::verification_record::VerificationRecordDoc>>,
        settlement_rows: RefCell<Vec<(u64, gauntlet_ports::state_store::SettlementCommit)>>,
        productions: RefCell<Vec<EvidenceProductionBinding>>,
        binding: RefCell<Option<(u32, ObservationBinding)>>,
    }

    impl Drop for SettleState {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.workspace);
        }
    }

    impl SettleState {
        fn new(run: Run) -> Self {
            Self::with_optional_run(Some(run))
        }

        fn empty() -> Self {
            Self::with_optional_run(None)
        }

        fn with_optional_run(run: Option<Run>) -> Self {
            static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let seq = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let workspace = std::env::temp_dir().join(format!(
                "gauntlet-settle-{}-{seq}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&workspace);
            std::fs::create_dir_all(&workspace).expect("the fixture workspace root is creatable");
            Self {
                run,
                workspace,
                checkpoint: RefCell::new(None),
                events: RefCell::new(Vec::new()),
                next_offset: Cell::new(1),
                verifications: RefCell::new(Vec::new()),
                settlement_rows: RefCell::new(Vec::new()),
                productions: RefCell::new(Vec::new()),
                binding: RefCell::new(None),
            }
        }

        fn workspace_root(&self) -> &std::path::Path {
            &self.workspace
        }

        /// The producer-side row the loop records (the honest producer:
        /// artifact at the locator under the root, row in the store).
        fn record(&self, evidence: &str, locator: &str, iteration: u32, bytes: &[u8]) {
            let path = self.workspace.join(locator);
            std::fs::write(&path, bytes).expect("the fixture artifact lands at the locator");
            let production = EvidenceProduction::new(
                EvidenceRef::new(evidence).expect("fixture evidence ref"),
                crate::verify::content_digest(bytes),
                instrument(),
                EvidenceLocator::new(locator).expect("fixture locator"),
                IterationNo::new(iteration).expect("1-based"),
                EpochSeconds::new(BASE_INSTANT + u64::from(iteration) * 10),
            );
            let offset = self.next_offset.get();
            self.next_offset.set(offset + 1);
            self.productions
                .borrow_mut()
                .push(EvidenceProductionBinding::new(
                    run_id(),
                    production,
                    gauntlet_domain::EventLogOffset::new(offset),
                ));
        }

        fn checkpoint_at(&self, iteration: u32, unit_states: Vec<(UnitId, UnitStatus)>) {
            *self.checkpoint.borrow_mut() = Some(Checkpoint {
                run_id: run_id(),
                reference_digest: self
                    .run
                    .as_ref()
                    .expect("the fixture holds its run")
                    .reference_version()
                    .digest()
                    .clone(),
                iteration_no: IterationNo::new(iteration).expect("1-based"),
                unit_states,
                budget_spent: *Budget::new(
                    Limits::new(8, 2, 2, 3600, 1000, MicroUsd::new(500)).expect("limits"),
                    20,
                    DiagnosticAllowance::new(100, MicroUsd::new(50)),
                )
                .expect("budget")
                .spent(),
                in_flight: Vec::new(),
                event_log_offset: gauntlet_domain::EventLogOffset::new(0),
            });
        }

        fn hold_binding(&self, iteration: u32, evidence: EvidenceRef, digest: Digest) {
            *self.binding.borrow_mut() = Some((
                iteration,
                ObservationBinding::new(evidence, digest),
            ));
        }

        fn settlement_events(&self) -> usize {
            self.events
                .borrow()
                .iter()
                .filter(|(_, event)| {
                    matches!(event.kind, RunEventKind::SettlementCommitted { .. })
                })
                .count()
        }

        fn settlement_row(&self, unit: &str) -> Option<u64> {
            self.settlement_rows
                .borrow()
                .iter()
                .find(|(_, commit)| commit.unit_id.as_str() == unit)
                .map(|(offset, _)| *offset)
        }

        fn production_row(&self, evidence: &EvidenceRef) -> Option<EvidenceProductionBinding> {
            self.productions
                .borrow()
                .iter()
                .find(|binding| binding.production().evidence() == evidence)
                .cloned()
        }
    }

    impl StateStore for SettleState {
        fn append_event(
            &self,
            event: RunEvent,
            _: Deadline,
        ) -> Result<gauntlet_domain::EventLogOffset, StateStoreError> {
            let offset = self.next_offset.get();
            self.next_offset.set(offset + 1);
            self.events.borrow_mut().push((offset, event));
            Ok(gauntlet_domain::EventLogOffset::new(offset))
        }
        fn load_run(&self, run_id: &RunId, _: Deadline) -> Result<Option<Run>, StateStoreError> {
            Ok(self
                .run
                .as_ref()
                .filter(|run| run.id() == run_id)
                .cloned())
        }
        fn checkpoint(&self, checkpoint: Checkpoint, _: Deadline) -> Result<(), StateStoreError> {
            *self.checkpoint.borrow_mut() = Some(checkpoint);
            Ok(())
        }
        fn resume(
            &self,
            _: &RunId,
            _: Deadline,
        ) -> Result<Option<Checkpoint>, StateStoreError> {
            Ok(self.checkpoint.borrow().clone())
        }
        fn record_verification(
            &self,
            record: &gauntlet_ports::verification_record::VerificationRecordDoc,
            _: Deadline,
        ) -> Result<(), StateStoreError> {
            self.verifications.borrow_mut().push(record.clone());
            Ok(())
        }
        fn verification_records(
            &self,
            run_id: &RunId,
            _: Deadline,
        ) -> Result<Vec<gauntlet_ports::verification_record::VerificationRecordDoc>, StateStoreError>
        {
            Ok(self
                .verifications
                .borrow()
                .iter()
                .filter(|record| record.run_id() == run_id)
                .cloned()
                .collect())
        }
        fn commit_settlement(
            &self,
            admitted: &gauntlet_ports::producer_authority::AdmittedSettlement,
            _: Deadline,
        ) -> Result<gauntlet_domain::EventLogOffset, StateStoreError> {
            let commit = admitted.commit().clone();
            let offset = self.next_offset.get();
            self.next_offset.set(offset + 1);
            self.events.borrow_mut().push((
                offset,
                RunEvent {
                    run_id: commit.run_id.clone(),
                    kind: RunEventKind::SettlementCommitted {
                        unit_id: commit.unit_id.clone(),
                        evidence: commit.evidence.clone(),
                        verification: commit.verification.clone(),
                        reference_digest: commit.reference_digest.clone(),
                    },
                    at: commit.at,
                },
            ));
            let mut rows = self.settlement_rows.borrow_mut();
            if !rows.iter().any(|(_, row)| {
                row.unit_id == commit.unit_id && row.verification == commit.verification
            }) {
                rows.push((offset, commit));
            }
            Ok(gauntlet_domain::EventLogOffset::new(offset))
        }
        fn settlements(
            &self,
            run_id: &RunId,
            _: Deadline,
        ) -> Result<Vec<gauntlet_domain::SettlementRecord>, StateStoreError> {
            let mut rows = self.settlement_rows.borrow().clone();
            rows.sort_by(|left, right| {
                (left.1.unit_id.as_str(), left.1.verification.as_str())
                    .cmp(&(right.1.unit_id.as_str(), right.1.verification.as_str()))
            });
            Ok(rows
                .into_iter()
                .filter(|(_, commit)| &commit.run_id == run_id)
                .map(|(offset, commit)| {
                    gauntlet_domain::SettlementRecord::new(
                        commit.run_id,
                        commit.unit_id,
                        commit.evidence,
                        commit.verification,
                        gauntlet_domain::EventLogOffset::new(offset),
                    )
                })
                .collect())
        }
        fn complete_pending_settlements(
            &self,
            run_id: &RunId,
            anchor: gauntlet_domain::EventLogOffset,
            _: Deadline,
        ) -> Result<usize, StateStoreError> {
            let events = self.events.borrow().clone();
            let mut added = 0;
            for (offset, event) in events {
                if offset < anchor.get() || &event.run_id != run_id {
                    continue;
                }
                let RunEventKind::SettlementCommitted {
                    unit_id,
                    evidence,
                    verification,
                    reference_digest,
                } = event.kind
                else {
                    continue;
                };
                let commit = gauntlet_ports::state_store::SettlementCommit {
                    run_id: event.run_id.clone(),
                    unit_id,
                    evidence,
                    verification,
                    reference_digest,
                    at: event.at,
                };
                let mut rows = self.settlement_rows.borrow_mut();
                if !rows.iter().any(|(_, row)| {
                    row.unit_id == commit.unit_id && row.verification == commit.verification
                }) {
                    rows.push((offset, commit));
                    added += 1;
                }
            }
            Ok(added)
        }
        fn record_evidence_production(
            &self,
            run_id: &RunId,
            production: &gauntlet_ports::producer_authority::AdmittedEvidenceProduction,
            _: Deadline,
        ) -> Result<EvidenceProductionBinding, StateStoreError> {
            let payload = production.production().clone();
            let offset = self.next_offset.get();
            self.next_offset.set(offset + 1);
            self.events.borrow_mut().push((
                offset,
                RunEvent {
                    run_id: run_id.clone(),
                    kind: RunEventKind::EvidenceProduced {
                        evidence: payload.evidence().clone(),
                        locator: payload.locator().clone(),
                    },
                    at: payload.produced_at(),
                },
            ));
            let binding =
                EvidenceProductionBinding::new(run_id.clone(), payload.clone(), gauntlet_domain::EventLogOffset::new(offset));
            self.productions.borrow_mut().push(binding.clone());
            Ok(binding)
        }
        fn evidence_production(
            &self,
            run_id: &RunId,
            evidence: &EvidenceRef,
            _: Deadline,
        ) -> Result<Option<EvidenceProductionBinding>, StateStoreError> {
            Ok(self
                .productions
                .borrow()
                .iter()
                .find(|binding| {
                    binding.run_id() == run_id && binding.production().evidence() == evidence
                })
                .cloned())
        }
        fn evidence_productions_at(
            &self,
            run_id: &RunId,
            locator: &EvidenceLocator,
            _: Deadline,
        ) -> Result<Vec<EvidenceProductionBinding>, StateStoreError> {
            let mut found: Vec<EvidenceProductionBinding> = self
                .productions
                .borrow()
                .iter()
                .filter(|binding| {
                    binding.run_id() == run_id && binding.production().locator() == locator
                })
                .cloned()
                .collect();
            found.sort_by_key(|binding| binding.event_log_offset().get());
            Ok(found)
        }
        fn bind_observation_bundle(
            &self,
            _: &RunId,
            iteration_no: IterationNo,
            binding: &gauntlet_ports::producer_authority::AdmittedObservationBinding,
            _: Deadline,
        ) -> Result<(), StateStoreError> {
            *self.binding.borrow_mut() = Some((
                iteration_no.get(),
                binding.binding().clone(),
            ));
            Ok(())
        }
        fn observation_binding(
            &self,
            _: &RunId,
            iteration_no: IterationNo,
            _: Deadline,
        ) -> Result<Option<ObservationBinding>, StateStoreError> {
            Ok(self
                .binding
                .borrow()
                .as_ref()
                .filter(|(iteration, _)| *iteration == iteration_no.get())
                .map(|(_, binding)| binding.clone()))
        }
        fn complete_pending_evidence_production(
            &self,
            run_id: &RunId,
            anchor: gauntlet_domain::EventLogOffset,
            production: &gauntlet_ports::producer_authority::AdmittedEvidenceProduction,
            _: Deadline,
        ) -> Result<Option<EvidenceProductionBinding>, StateStoreError> {
            let payload = production.production();
            if self
                .productions
                .borrow()
                .iter()
                .any(|binding| binding.production().evidence() == payload.evidence())
            {
                return Ok(None);
            }
            let events = self.events.borrow().clone();
            let Some((offset, event)) = events.into_iter().find(|(offset, event)| {
                *offset >= anchor.get()
                    && &event.run_id == run_id
                    && matches!(
                        &event.kind,
                        RunEventKind::EvidenceProduced { evidence, .. }
                        if evidence == payload.evidence()
                    )
            }) else {
                return Ok(None);
            };
            let RunEventKind::EvidenceProduced { evidence, locator } = event.kind else {
                return Ok(None);
            };
            if &locator != payload.locator() {
                return Err(StateStoreError::WriteFailed {
                    detail: format!(
                        "the orphan EvidenceProduced event at offset {offset} names locator \
                         {locator}, not {} — the orphan belongs to a different production",
                        payload.locator()
                    ),
                });
            }
            // The A3 discipline, mirrored: the completed row takes the
            // EVENT's stored timestamp, never the re-mint's clock.
            let completed = EvidenceProduction::new(
                evidence,
                payload.content().clone(),
                payload.producer().clone(),
                locator,
                payload.iteration_no(),
                event.at,
            );
            let binding = EvidenceProductionBinding::new(
                run_id.clone(),
                completed,
                gauntlet_domain::EventLogOffset::new(offset),
            );
            self.productions.borrow_mut().push(binding.clone());
            Ok(Some(binding))
        }
        fn export_jsonl(&self, _: &RunId, _: Deadline) -> Result<String, StateStoreError> {
            Ok(String::new())
        }
    }

    // ------------------------------------------------------------------
    // The fixture
    // ------------------------------------------------------------------

    struct Fixture {
        state: Arc<SettleState>,
        evidence: Arc<PinningEvidence>,
        provider: Arc<ScriptedProvider>,
        escalation: Arc<RecordingEscalation>,
        service: SettlementService,
    }

    fn fixture() -> Fixture {
        let evidence = Arc::new(PinningEvidence::default());
        let frozen = evidence.insert("evd-settle-frozen", KEY.as_bytes());
        let version = ReferenceVersion::with_evidence(
            ReferenceId::new("settle-fixture-ref").expect("reference id"),
            1,
            crate::verify::content_digest(KEY.as_bytes()),
            frozen,
        );
        let run = Run::new(RunDefinition {
            id: run_id(),
            created_at: EpochSeconds::new(BASE_INSTANT),
            reference_version: version.clone(),
            intent: "settle fixture".to_owned(),
            config_fingerprint: Digest::new("sha256", &"aa".repeat(32)).expect("digest"),
            environment_fingerprint: Digest::new("sha256", &"bb".repeat(32)).expect("digest"),
            budget: Budget::new(
                Limits::new(8, 2, 2, 3600, 1000, MicroUsd::new(500)).expect("limits"),
                20,
                DiagnosticAllowance::new(100, MicroUsd::new(50)),
            )
            .expect("budget"),
            parent_run: None,
        })
        .expect("legal run");
        let state = Arc::new(SettleState::new(run));
        let provider = Arc::new(ScriptedProvider {
            version,
            mode: Cell::new(ProviderMode::Verifies),
        });
        let escalation = Arc::new(RecordingEscalation::default());
        let service = SettlementService::new(
            state.clone(),
            evidence.clone(),
            provider.clone(),
            escalation.clone(),
            Arc::new(FixedClock(Cell::new(BASE_INSTANT))),
        );
        Fixture {
            state,
            evidence,
            provider,
            escalation,
            service,
        }
    }

    fn ladder() -> Ladder {
        Ladder::new(vec![LadderRung::new(
            RungKind::TestReport,
            Arc::new(FakeDeterministicVerifier {
                name: "settle-fixture-verifier",
                kinds: &[EvidenceKind::TestReport],
                passing: vec![
                    "settle-9501.json".to_owned(),
                    "settle-9502.json".to_owned(),
                    "settle-9503.json".to_owned(),
                ],
            }),
        )])
        .expect("the fixture ladder composes")
    }

    fn unit_request(unit: &str) -> VerificationRequest {
        VerificationRequest {
            id: gauntlet_domain::VerificationRef::new("vrf-settle-caller").expect("ref"),
            run_id: run_id(),
            unit_id: unit_id(unit),
            claim: "the caller's claim".to_owned(),
            requirement: gauntlet_domain::EvidenceRequirement::new(
                EvidenceKind::TestReport,
                EvidenceLocator::new(locator_of(unit)).expect("locator"),
                gauntlet_domain::Producer::Instrument,
            ),
            evidence: EvidenceLocator::new(locator_of(unit)).expect("locator"),
        }
    }

    /// Earns the claim AND its readiness the honest way: a real production
    /// row, real store bytes, a real pin, a real checkpoint anchor, the
    /// governed chain, and the precondition layer.
    fn earned_claim(f: &Fixture, unit: &str, iteration: u32) -> (VerifiedClaim, SettlementReadiness) {
        let reference = f
            .evidence
            .insert(&format!("evd-{unit}"), b"the fixture report bytes");
        f.state.record(reference.as_str(), locator_of(unit), iteration, b"the fixture report bytes");
        f.evidence.pin(&run_id(), &reference);
        f.state.checkpoint_at(iteration, Vec::new());
        let governed = VerificationAuthority::new(
            f.state.clone(),
            f.evidence.clone(),
            Arc::new(SeededIds::new()),
        )
        .for_run(
            &run_id(),
            IterationNo::new(iteration).expect("1-based"),
            ladder(),
            f.state.workspace_root().to_owned(),
            bound(),
        )
        .expect("the fixture run opens");
        let claim = governed
            .verify_claim(&unit_request(unit), bound())
            .expect("the chain mints a PASS");
        assert_eq!(claim.record().outcome(), Outcome::Pass);
        let readiness = settlement_precondition(&claim, &run_id(), bound())
            .expect("the fixture claim is ready");
        (claim, readiness)
    }

    /// The pinned records whose text contains `needle` — pin status is
    /// checked (the name is the contract), matching `is_pinned_record`.
    fn pinned_records_containing(evidence: &PinningEvidence, needle: &str) -> Vec<String> {
        let objects = evidence.objects.lock().expect("not poisoned");
        let pins = evidence.pins.lock().expect("not poisoned");
        objects
            .iter()
            .filter(|(reference, bytes)| {
                String::from_utf8_lossy(bytes).contains(needle)
                    && pins.contains(&(run_id(), reference.clone()))
            })
            .map(|(_, bytes)| String::from_utf8_lossy(bytes).into_owned())
            .collect()
    }

    fn is_pinned_record(evidence: &PinningEvidence, needle: &str) -> bool {
        let objects = evidence.objects.lock().expect("not poisoned");
        let pins = evidence.pins.lock().expect("not poisoned");
        objects
            .iter()
            .any(|(reference, bytes)| {
                String::from_utf8_lossy(bytes).contains(needle)
                    && pins.contains(&(run_id(), reference.clone()))
            })
    }

    // ------------------------------------------------------------------
    // Condition 3 — the authority chain
    // ------------------------------------------------------------------

    #[test]
    fn a_critic_unit_settles_durably_under_automatic_closure() {
        let f = fixture();
        let (claim, readiness) = earned_claim(&f, "AK-9501", 1);
        let outcome = f
            .service
            .settle(
                SettleRequest {
                    claim: &claim,
                    readiness,
                    actor: ClosingActor::Automatic,
                    tracked_residuals: &[],
                },
                bound(),
            )
            .expect("a critic-authority PASS with readiness settles");
        assert!(outcome.committed_now);
        // The commit order is OBSERVABLE: the returned offset names the
        // durable settlement event, the row names the SAME event, and the
        // advanced projection rides the return (after both).
        let offset = outcome.event_log_offset.get();
        assert!(offset > 0);
        assert_eq!(f.state.settlement_row("AK-9501"), Some(offset));
        assert_eq!(f.state.settlement_events(), 1);
        assert_eq!(outcome.settled_unit.status(), UnitStatus::Settled);
        assert!(outcome.settled_unit.verification().is_some());
        assert_eq!(outcome.authority_decision.required, Authority::Critic);
        assert_eq!(outcome.authority_decision.exercised_by, ActorKind::Runtime);
        assert!(outcome.authority_decision.operator_identity.is_none());
        // The verification document is durable (the admission's substrate).
        assert_eq!(f.state.verification_records(&run_id(), bound()).unwrap().len(), 1);
        // The B.6 authority_decision accompanying record is PINNED evidence.
        assert!(is_pinned_record(&f.evidence, "settlement_authority_decision"));
    }

    #[test]
    fn a_lead_agent_unit_requires_a_recorded_rationale() {
        let f = fixture();
        let (claim, readiness) = earned_claim(&f, "AK-9502", 1);
        let absent = f.service.settle(
            SettleRequest {
                claim: &claim,
                readiness: readiness, // consumed? no — clone for the second call is impossible; re-earn below
                actor: ClosingActor::Orchestrator { rationale: None },
                tracked_residuals: &[],
            },
            bound(),
        );
        assert!(matches!(absent, Err(SettleError::RationaleRequired { .. })));
        let (claim, readiness) = earned_claim(&f, "AK-9502", 1);
        let blank = f.service.settle(
            SettleRequest {
                claim: &claim,
                readiness,
                actor: ClosingActor::Orchestrator {
                    rationale: Some("   ".to_owned()),
                },
                tracked_residuals: &[],
            },
            bound(),
        );
        assert!(
            matches!(blank, Err(SettleError::RationaleRequired { .. })),
            "a blank rationale is an absent rationale: {blank:?}"
        );
        assert_eq!(f.state.settlement_events(), 0, "nothing committed");
        let (claim, readiness) = earned_claim(&f, "AK-9502", 1);
        let outcome = f
            .service
            .settle(
                SettleRequest {
                    claim: &claim,
                    readiness,
                    actor: ClosingActor::Orchestrator {
                        rationale: Some("the orchestrator's closure rationale".to_owned()),
                    },
                    tracked_residuals: &[],
                },
                bound(),
            )
            .expect("an orchestrator with a recorded rationale closes lead-agent work");
        assert!(outcome.committed_now);
        assert_eq!(outcome.authority_decision.exercised_by, ActorKind::Orchestrator);
        assert!(outcome.authority_decision.rationale_recorded);
    }

    #[test]
    fn a_human_authority_unit_refuses_automatic_closure_and_escalates() {
        let f = fixture();
        let (claim, readiness) = earned_claim(&f, "AK-9503", 1);
        let refusal = f.service.settle(
            SettleRequest {
                claim: &claim,
                readiness,
                actor: ClosingActor::Automatic,
                tracked_residuals: &[],
            },
            bound(),
        );
        match refusal {
            Err(SettleError::AuthorityNotPermitted {
                required, actor, ..
            }) => {
                assert_eq!(required, Authority::Human);
                assert_eq!(actor, ActorKind::Runtime);
            }
            other => panic!("the human-under-automatic refusal is typed, got {other:?}"),
        }
        // §11.5 row 6: the gap is ROUTED to the escalation surface — the
        // loop never spins on it, and nothing settled.
        assert!(
            !f.escalation.tickets.borrow().is_empty(),
            "the authority gap is escalated, never looped past"
        );
        assert_eq!(f.state.settlement_events(), 0);
        assert!(f.state.settlement_row("AK-9503").is_none());
        // The operator route, with a recorded identity, closes it.
        let (claim, readiness) = earned_claim(&f, "AK-9503", 1);
        let identity = OperatorIdentity::new("operator-one").expect("identity");
        let outcome = f
            .service
            .settle(
                SettleRequest {
                    claim: &claim,
                    readiness,
                    actor: ClosingActor::Operator {
                        identity: Some(identity.clone()),
                    },
                    tracked_residuals: &[],
                },
                bound(),
            )
            .expect("an operator action with recorded identity closes human work");
        assert_eq!(outcome.authority_decision.exercised_by, ActorKind::Operator);
        assert_eq!(
            outcome.authority_decision.operator_identity.as_ref(),
            Some(&identity)
        );
        // Absent identity on the operator route refuses.
        let (claim, readiness) = earned_claim(&f, "AK-9503", 1);
        let anonymous = f.service.settle(
            SettleRequest {
                claim: &claim,
                readiness,
                actor: ClosingActor::Operator { identity: None },
                tracked_residuals: &[],
            },
            bound(),
        );
        assert!(matches!(anonymous, Err(SettleError::OperatorIdentityRequired { .. })));
    }

    // ------------------------------------------------------------------
    // Condition 4 — blocking residuals
    // ------------------------------------------------------------------

    #[test]
    fn a_blocking_residual_referencing_the_unit_refuses_settlement() {
        let f = fixture();
        let (claim, readiness) = earned_claim(&f, "AK-9501", 1);
        let other = BlockingResidual {
            unit: unit_id("AK-9502"),
            detail: "expected: the report; observed: absent".to_owned(),
        };
        let unaffected = f.service.settle(
            SettleRequest {
                claim: &claim,
                readiness, // moved below for the blocking leg — rebind
                actor: ClosingActor::Automatic,
                tracked_residuals: std::slice::from_ref(&other),
            },
            bound(),
        );
        assert!(
            unaffected.is_ok(),
            "a residual for ANOTHER unit does not block this one: {unaffected:?}"
        );
        let (claim, readiness) = earned_claim(&f, "AK-9501", 2);
        let blocking = BlockingResidual {
            unit: unit_id("AK-9501"),
            detail: "expected: the report; observed: absent".to_owned(),
        };
        let refused = f.service.settle(
            SettleRequest {
                claim: &claim,
                readiness,
                actor: ClosingActor::Automatic,
                tracked_residuals: std::slice::from_ref(&blocking),
            },
            bound(),
        );
        match refused {
            Err(SettleError::BlockingResidualOpen {
                unit_id: blocked_unit,
                detail,
                ..
            }) => {
                assert_eq!(blocked_unit, unit_id("AK-9501"));
                assert!(detail.contains("expected: the report"));
            }
            other => panic!("the blocking residual is a typed refusal, got {other:?}"),
        }
    }

    #[test]
    fn a_durably_blocked_projection_wins_over_a_clear_tracked_view() {
        // The preregistered P4 seam: in-memory tracking and durable state
        // diverge — the DURABLE state wins and the divergence itself is the
        // typed refusal cause.
        let f = fixture();
        let (claim, readiness) = earned_claim(&f, "AK-9501", 1);
        f.state
            .checkpoint_at(1, vec![(unit_id("AK-9501"), UnitStatus::Blocked)]);
        let refused = f.service.settle(
            SettleRequest {
                claim: &claim,
                readiness,
                actor: ClosingActor::Automatic,
                tracked_residuals: &[],
            },
            bound(),
        );
        match refused {
            Err(SettleError::DurableBlockDiverged {
                durable_status, ..
            }) => assert_eq!(durable_status, UnitStatus::Blocked),
            other => panic!("the durable block diverges loudly, got {other:?}"),
        }
        assert_eq!(f.state.settlement_events(), 0, "nothing committed");
    }

    // ------------------------------------------------------------------
    // Condition 5 — the settle-time reference re-verify
    // ------------------------------------------------------------------

    #[test]
    fn a_settle_time_reference_mutation_aborts_with_both_digests_as_pinned_evidence() {
        let f = fixture();
        let (claim, readiness) = earned_claim(&f, "AK-9501", 1);
        f.provider.mode.set(ProviderMode::Mutated);
        let refusal = f.service.settle(
            SettleRequest {
                claim: &claim,
                readiness,
                actor: ClosingActor::Automatic,
                tracked_residuals: &[],
            },
            bound(),
        );
        let expected = crate::verify::content_digest(KEY.as_bytes());
        let actual = crate::verify::content_digest(b"the mutated ruler bytes");
        match refusal {
            Err(SettleError::ReferenceMutated {
                expected: found_expected,
                actual: found_actual,
                ..
            }) => {
                assert_eq!(found_expected, expected);
                assert_eq!(found_actual, actual);
            }
            other => panic!("the mutation is the typed abort refusal, got {other:?}"),
        }
        // BOTH digests are persisted as PINNED evidence (§9.4), and nothing
        // settled under the moved ruler. They ride ONE pinned abort record.
        assert!(is_pinned_record(&f.evidence, &expected.to_string()));
        assert!(is_pinned_record(&f.evidence, &actual.to_string()));
        assert!(is_pinned_record(&f.evidence, "reference_mutated"));
        let abort_records = pinned_records_containing(&f.evidence, "reference_mutated");
        assert!(
            abort_records
                .iter()
                .any(|record| record.contains(&expected.to_string())
                    && record.contains(&actual.to_string())),
            "one pinned abort record carries BOTH digests: {abort_records:?}"
        );
        assert_eq!(f.state.settlement_events(), 0);
    }

    #[test]
    fn a_settle_time_unavailability_refuses_without_aborting_or_escalating() {
        let f = fixture();
        let (claim, readiness) = earned_claim(&f, "AK-9501", 1);
        f.provider.mode.set(ProviderMode::Unavailable);
        let refusal = f.service.settle(
            SettleRequest {
                claim: &claim,
                readiness,
                actor: ClosingActor::Automatic,
                tracked_residuals: &[],
            },
            bound(),
        );
        assert!(matches!(
            refusal,
            Err(SettleError::ReferenceUnverifiable { .. })
        ));
        assert!(f.escalation.tickets.borrow().is_empty());
        assert_eq!(f.state.settlement_events(), 0);
    }

    // ------------------------------------------------------------------
    // The durable substrate: recording, admission, idempotence
    // ------------------------------------------------------------------

    #[test]
    fn a_re_entry_of_the_same_settlement_key_is_idempotent_success() {
        let f = fixture();
        let (claim, readiness) = earned_claim(&f, "AK-9501", 1);
        let first = f
            .service
            .settle(
                SettleRequest {
                    claim: &claim,
                    readiness,
                    actor: ClosingActor::Automatic,
                    tracked_residuals: &[],
                },
                bound(),
            )
            .expect("the first settle commits");
        // A crash AFTER the durable commit: the loop re-enters with a
        // freshly earned claim whose verification ref is the SAME §16.3 key
        // (the seeded fixture mints the same ref) — re-entry must be an
        // idempotent SUCCESS carrying the existing durable facts.
        let (claim, readiness) = earned_claim(&f, "AK-9501", 1);
        let second = f
            .service
            .settle(
                SettleRequest {
                    claim: &claim,
                    readiness,
                    actor: ClosingActor::Automatic,
                    tracked_residuals: &[],
                },
                bound(),
            )
            .expect("re-entry of the committed key is success, not an error");
        assert!(!second.committed_now);
        assert_eq!(
            second.event_log_offset, first.event_log_offset,
            "the existing durable facts are returned"
        );
        assert_eq!(f.state.settlement_events(), 1, "no second event");
        assert_eq!(
            f.state.settlements(&run_id(), bound()).unwrap().len(),
            1,
            "no second row"
        );
    }

    #[test]
    fn a_disagreeing_durable_record_for_the_same_identity_refuses() {
        let f = fixture();
        // A record with the same identity tuple (unit, evidence, outcome)
        // but different content is an integrity refusal, never a silent
        // overwrite.
        let mut disagreeing = claim_document(&f, "AK-9501");
        let content = disagreeing.content();
        let mut altered = content;
        altered.detail = "a different judgment".to_owned();
        disagreeing = gauntlet_ports::verification_record::VerificationRecordDoc::new(altered)
            .expect("altered content is well formed");
        f.state
            .record_verification(&disagreeing, bound())
            .expect("the disagreeing record lands");
        let (claim, readiness) = earned_claim(&f, "AK-9501", 1);
        let refused = f.service.settle(
            SettleRequest {
                claim: &claim,
                readiness,
                actor: ClosingActor::Automatic,
                tracked_residuals: &[],
            },
            bound(),
        );
        assert!(matches!(
            refused,
            Err(SettleError::VerificationRecordDisagrees { .. })
        ));
    }

    fn claim_document(
        f: &Fixture,
        unit: &str,
    ) -> gauntlet_ports::verification_record::VerificationRecordDoc {
        let _ = f;
        gauntlet_ports::verification_record::VerificationRecordDoc::new(
            gauntlet_ports::verification_record::VerificationContent {
                run_id: run_id(),
                iteration_no: IterationNo::new(1).expect("1-based"),
                unit_id: unit_id(unit),
                claim: "the fixture claim".to_owned(),
                requirement: gauntlet_ports::verification_record::RequirementDoc::new(
                    EvidenceKind::TestReport,
                    EvidenceLocator::new(locator_of(unit)).expect("locator"),
                    gauntlet_domain::Producer::Instrument,
                ),
                instrument: gauntlet_ports::verification_record::VerifyingInstrument::new(
                    "settle-fixture-verifier".to_owned(),
                    "1".to_owned(),
                    true,
                ),
                evidence: EvidenceRef::new(&format!("evd-{unit}")).expect("ref"),
                produced_at: EpochSeconds::new(BASE_INSTANT),
                outcome: Outcome::Pass,
                class: None,
                detail: "the fixture judgment".to_owned(),
            },
        )
        .expect("fixture document is well formed")
    }

    #[test]
    fn an_undurable_run_refuses_every_settlement() {
        // A claim earned in a durable substrate, settled through a service
        // whose store holds NO run: the durable grounding refuses.
        let f = fixture();
        let (claim, readiness) = earned_claim(&f, "AK-9501", 1);
        let service = SettlementService::new(
            Arc::new(SettleState::empty()),
            f.evidence.clone(),
            f.provider.clone(),
            f.escalation.clone(),
            Arc::new(FixedClock(Cell::new(BASE_INSTANT))),
        );
        let refused = service.settle(
            SettleRequest {
                claim: &claim,
                readiness,
                actor: ClosingActor::Automatic,
                tracked_residuals: &[],
            },
            bound(),
        );
        assert!(matches!(refused, Err(SettleError::RunNotDurable { .. })));
    }

    // ------------------------------------------------------------------
    // The AK-040 service-level preconditions matrix — each refused
    // condition is a DISTINCT typed refusal.
    // ------------------------------------------------------------------

    #[test]
    fn each_refused_condition_is_a_distinct_typed_refusal() {
        let variants: Vec<String> = vec![
            SettleError::RationaleRequired {
                run_id: run_id(),
                unit_id: unit_id("AK-9502"),
            }
            .variant_name(),
            SettleError::OperatorIdentityRequired {
                run_id: run_id(),
                unit_id: unit_id("AK-9503"),
            }
            .variant_name(),
            SettleError::AuthorityNotPermitted {
                run_id: run_id(),
                unit_id: unit_id("AK-9503"),
                required: Authority::Human,
                actor: ActorKind::Runtime,
            }
            .variant_name(),
            SettleError::BlockingResidualOpen {
                run_id: run_id(),
                unit_id: unit_id("AK-9501"),
                detail: String::new(),
            }
            .variant_name(),
            SettleError::DurableBlockDiverged {
                run_id: run_id(),
                unit_id: unit_id("AK-9501"),
                durable_status: UnitStatus::Blocked,
            }
            .variant_name(),
            SettleError::ReferenceMutated {
                run_id: run_id(),
                unit_id: unit_id("AK-9501"),
                expected: crate::verify::content_digest(b"a"),
                actual: crate::verify::content_digest(b"b"),
            }
            .variant_name(),
            SettleError::ReferenceUnverifiable {
                run_id: run_id(),
                unit_id: unit_id("AK-9501"),
                detail: String::new(),
            }
            .variant_name(),
            SettleError::RunNotDurable { run_id: run_id() }.variant_name(),
        ];
        let mut distinct = variants.clone();
        distinct.sort();
        distinct.dedup();
        assert_eq!(
            distinct.len(),
            variants.len(),
            "every refused condition names its own variant: {variants:?}"
        );
    }

    // ------------------------------------------------------------------
    // §18.3 step 1 — resume completion
    // ------------------------------------------------------------------

    #[test]
    fn a_half_applied_settlement_completes_exactly_once_from_the_anchor() {
        let f = fixture();
        f.state.checkpoint_at(1, Vec::new());
        // The crash window: the settlement EVENT is durable, the row is not.
        f.state
            .append_event(
                RunEvent {
                    run_id: run_id(),
                    kind: RunEventKind::SettlementCommitted {
                        unit_id: unit_id("AK-9501"),
                        evidence: EvidenceRef::new("evd-AK-9501").expect("ref"),
                        verification: gauntlet_domain::VerificationRef::new("vrf-9501")
                            .expect("ref"),
                        reference_digest: crate::verify::content_digest(KEY.as_bytes()),
                    },
                    at: EpochSeconds::new(BASE_INSTANT + 3),
                },
                bound(),
            )
            .expect("the half-applied event lands");
        let completion: ResumeCompletion = f
            .service
            .complete_pending_after_checkpoint(
                &run_id(),
                gauntlet_domain::EventLogOffset::new(0),
                bound(),
            )
            .expect("the replay completes");
        assert_eq!(completion.settlements_completed, 1);
        assert!(f.state.settlement_row("AK-9501").is_some());
        // Idempotent: the second replay adds nothing.
        let again = f
            .service
            .complete_pending_after_checkpoint(
                &run_id(),
                gauntlet_domain::EventLogOffset::new(0),
                bound(),
            )
            .expect("the second replay completes");
        assert_eq!(again.settlements_completed, 0);
        // Anchored: an event BELOW the anchor is the checkpoint's own
        // certification, never re-scanned.
        let f2 = fixture();
        f2.state.checkpoint_at(1, Vec::new());
        let offset = f2
            .state
            .append_event(
                RunEvent {
                    run_id: run_id(),
                    kind: RunEventKind::SettlementCommitted {
                        unit_id: unit_id("AK-9501"),
                        evidence: EvidenceRef::new("evd-AK-9501").expect("ref"),
                        verification: gauntlet_domain::VerificationRef::new("vrf-9501")
                            .expect("ref"),
                        reference_digest: crate::verify::content_digest(KEY.as_bytes()),
                    },
                    at: EpochSeconds::new(BASE_INSTANT + 3),
                },
                bound(),
            )
            .expect("lands")
            .get();
        let anchored = f2
            .service
            .complete_pending_after_checkpoint(
                &run_id(),
                gauntlet_domain::EventLogOffset::new(offset + 1),
                bound(),
            )
            .expect("completes");
        assert_eq!(
            anchored.settlements_completed, 0,
            "events below the anchor are not re-scanned"
        );
        assert!(f2.state.settlement_row("AK-9501").is_none());
    }

    #[test]
    fn a_half_applied_evidence_production_completes_from_durable_facts_only() {
        let f = fixture();
        f.state.checkpoint_at(1, Vec::new());
        // The orphan's durable facts: the facet bytes in the store, the
        // bound bundle that names them, and the orphan EVENT (row missing).
        let reference = f.evidence.insert("evd-orphan", b"the orphan facet bytes");
        let orphan_offset = f
            .state
            .append_event(
                RunEvent {
                    run_id: run_id(),
                    kind: RunEventKind::EvidenceProduced {
                        evidence: reference.clone(),
                        locator: EvidenceLocator::new("settle-9501.json").expect("locator"),
                    },
                    at: EpochSeconds::new(BASE_INSTANT + 7),
                },
                bound(),
            )
            .expect("the orphan event lands")
            .get();
        let facet = ObservationFacet::new(FacetKind::Test, instrument(), reference.clone())
            .with_window(
                Some(EpochSeconds::new(BASE_INSTANT + 1)),
                Some(EpochSeconds::new(BASE_INSTANT + 2)),
            );
        let doc = ObservationBundleDoc::new(BundleContent {
            run_id: run_id(),
            iteration_no: IterationNo::new(1).expect("1-based"),
            observed_at: BASE_INSTANT + 2,
            project_revision: "rev".to_owned(),
            environment_fingerprint: "env".to_owned(),
            observation_digest: crate::verify::content_digest(b"bundle"),
            facets: vec![FacetRecord::from_facet(&facet)],
            claims_facet: ClaimsFacet::new(ClaimsFacet::render_content(&[])),
            unavailable_evidence: Vec::new(),
            observation_failures: Vec::new(),
        });
        let bundle_ref = f
            .evidence
            .put(doc.to_json().as_bytes(), EvidenceKind::Log, bound())
            .expect("the bundle stores");
        f.state
            .hold_binding(1, bundle_ref, crate::verify::content_digest(b"bundle"));
        let completion = f
            .service
            .complete_pending_after_checkpoint(
                &run_id(),
                gauntlet_domain::EventLogOffset::new(0),
                bound(),
            )
            .expect("the completion runs");
        assert_eq!(
            completion.productions_completed.len(),
            1,
            "the orphan completes: {:?}",
            completion.limitations
        );
        let completed = &completion.productions_completed[0];
        assert_eq!(
            completed.event_log_offset().get(),
            orphan_offset,
            "the row binds to the ORPHAN EVENT's offset"
        );
        assert_eq!(
            completed.production().produced_at(),
            EpochSeconds::new(BASE_INSTANT + 7),
            "produced_at is EVENT-derived, never the re-mint's clock"
        );
        assert_eq!(
            completed.production().content(),
            &crate::verify::content_digest(b"the orphan facet bytes"),
            "the content digest is derived from the store's own bytes"
        );
        assert!(
            completion.limitations.is_empty(),
            "a fully recoverable orphan records no limitation: {:?}",
            completion.limitations
        );
        // Idempotent: the second call finds the row.
        let again = f
            .service
            .complete_pending_after_checkpoint(
                &run_id(),
                gauntlet_domain::EventLogOffset::new(0),
                bound(),
            )
            .expect("the second completion runs");
        assert!(again.productions_completed.is_empty());
    }

    #[test]
    fn an_unrecoverable_orphan_is_a_recorded_limitation_never_a_fabrication() {
        let f = fixture();
        f.state.checkpoint_at(1, Vec::new());
        // The bundle names a facet whose bytes NEVER landed in the store —
        // the mint has nothing to derive the digest from.
        let ghost = EvidenceRef::new("evd-never-stored").expect("ref");
        let facet = ObservationFacet::new(FacetKind::Test, instrument(), ghost.clone())
            .with_window(
                Some(EpochSeconds::new(BASE_INSTANT + 1)),
                Some(EpochSeconds::new(BASE_INSTANT + 2)),
            );
        let doc = ObservationBundleDoc::new(BundleContent {
            run_id: run_id(),
            iteration_no: IterationNo::new(1).expect("1-based"),
            observed_at: BASE_INSTANT + 2,
            project_revision: "rev".to_owned(),
            environment_fingerprint: "env".to_owned(),
            observation_digest: crate::verify::content_digest(b"bundle"),
            facets: vec![FacetRecord::from_facet(&facet)],
            claims_facet: ClaimsFacet::new(ClaimsFacet::render_content(&[])),
            unavailable_evidence: Vec::new(),
            observation_failures: Vec::new(),
        });
        let bundle_ref = f
            .evidence
            .put(doc.to_json().as_bytes(), EvidenceKind::Log, bound())
            .expect("the bundle stores");
        f.state
            .hold_binding(1, bundle_ref, crate::verify::content_digest(b"bundle"));
        let completion = f
            .service
            .complete_pending_after_checkpoint(
                &run_id(),
                gauntlet_domain::EventLogOffset::new(0),
                bound(),
            )
            .expect("the completion runs");
        assert!(
            completion.productions_completed.is_empty(),
            "nothing is fabricated"
        );
        assert_eq!(completion.limitations.len(), 1, "the gap is recorded");
        assert!(is_pinned_record(
            &f.evidence,
            "evidence_production_orphan_unrecoverable"
        ));
        // No row for the ghost.
        assert!(f.state.production_row(&ghost).is_none());
    }

    // ------------------------------------------------------------------
    // AK-062 — the service-level no-bypass half
    // ------------------------------------------------------------------

    #[test]
    fn an_operator_action_without_pass_evidence_hits_the_same_gate_as_the_loop() {
        let f = fixture();
        f.state.checkpoint_at(1, Vec::new());
        // No durable PASS record at all: the operator-shaped request is
        // refused by the SAME typed error the automatic path's admission
        // produces.
        let refusal = f.service.settle_operator_action(
            OperatorActionRequest {
                run_id: run_id(),
                unit_id: unit_id("AK-9501"),
                identity: Some(OperatorIdentity::new("operator-one").expect("identity")),
            },
            bound(),
        );
        match refusal {
            Err(SettleError::VerificationNotPass {
                unit_id: gated_unit,
                ..
            }) => {
                assert_eq!(gated_unit, unit_id("AK-9501"));
            }
            other => panic!("the operator action hits the same PASS gate, got {other:?}"),
        }
        // With a durable PASS record the operator shape STILL cannot bypass:
        // settlement requires the chain-earned readiness, which only the
        // governed verification chain can mint (Task 21 composes that
        // surface; this service exposes no side door).
        f.state
            .record_verification(&claim_document(&f, "AK-9501"), bound())
            .expect("the PASS record lands");
        let still = f.service.settle_operator_action(
            OperatorActionRequest {
                run_id: run_id(),
                unit_id: unit_id("AK-9501"),
                identity: Some(OperatorIdentity::new("operator-one").expect("identity")),
            },
            bound(),
        );
        assert!(matches!(
            still,
            Err(SettleError::OperatorActionRequiresEarnedReadiness { .. })
        ));
        assert!(
            f.state.settlements(&run_id(), bound()).unwrap().is_empty(),
            "the operator shape never settles by itself"
        );
    }

    #[test]
    fn operator_action_refusals_name_the_three_states_and_the_shared_gate_variant() {
        // AK-062's structural half (Task 13 B2 Item 5): the operator seam's
        // PASS gate IS the automatic path's admission gate — the ports'
        // `SettlementAdmissionError::VerificationNotPass` maps onto the
        // SAME `SettleError::VerificationNotPass` variant ("the same gate,
        // the same variant, whichever caller hit it"), and a run whose
        // records hold FAIL and INCONCLUSIVE outcomes refuses naming BOTH
        // — INCONCLUSIVE is never collapsed into FAIL through the gate
        // (AK-005's three-state rule, held at the operator seam).
        let f = fixture();
        f.state.checkpoint_at(1, Vec::new());
        // (a) the shared-variant mapping: what the ADMISSION refuses with,
        // the operator path refuses with too — one variant, both callers.
        let admission = gauntlet_ports::producer_authority::SettlementAdmissionError::VerificationNotPass {
            run_id: run_id(),
            unit_id: unit_id("AK-9501"),
            detail: "the admission's own wording".to_owned(),
        };
        assert!(matches!(
            SettleError::from(admission),
            SettleError::VerificationNotPass { .. }
        ));
        // (b) FAIL and INCONCLUSIVE records are named as themselves —
        // neither is a soft PASS, and they stay distinct in the refusal.
        // (Both sit on the QUERIED unit; outcome is part of the record
        // identity, so the two records coexist.)
        for outcome in [
            gauntlet_domain::Outcome::Fail,
            gauntlet_domain::Outcome::Inconclusive,
        ] {
            let mut content = claim_document(&f, "AK-9501");
            let altered = {
                let mut content = content.content().clone();
                content.outcome = outcome;
                content
            };
            content = gauntlet_ports::verification_record::VerificationRecordDoc::new(altered)
                .expect("altered content is well formed");
            f.state
                .record_verification(&content, bound())
                .expect("the non-PASS record lands");
        }
        let refusal = f.service.settle_operator_action(
            OperatorActionRequest {
                run_id: run_id(),
                unit_id: unit_id("AK-9501"),
                identity: Some(OperatorIdentity::new("operator-two").expect("identity")),
            },
            bound(),
        );
        match refusal {
            Err(SettleError::VerificationNotPass { detail, .. }) => {
                assert!(
                    detail.contains("FAIL") && detail.contains("INCONCLUSIVE"),
                    "the refusal names BOTH outcomes as themselves: {detail}"
                );
                assert!(
                    detail.contains("none is PASS"),
                    "the refusal names the gate's reason: {detail}"
                );
            }
            other => panic!("the non-PASS records refuse the operator action, got {other:?}"),
        }
        assert!(
            f.state.settlements(&run_id(), bound()).unwrap().is_empty(),
            "nothing settled"
        );
    }

    // ------------------------------------------------------------------
    // Shared plumbing sanity (the fixture itself must be honest)
    // ------------------------------------------------------------------

    #[test]
    fn the_fixture_key_parses_into_three_authority_levels() {
        let parsed = crate::reference::parse::parse_answer_key(KEY);
        assert_eq!(parsed.units.len(), 3);
        assert_eq!(parsed.failures.len(), 0, "the fixture key parses strictly");
        let authorities: Vec<Authority> =
            parsed.units.iter().map(|unit| unit.authority()).collect();
        assert_eq!(
            authorities,
            vec![Authority::Critic, Authority::LeadAgent, Authority::Human]
        );
    }
}
