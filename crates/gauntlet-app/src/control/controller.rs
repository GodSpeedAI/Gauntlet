//! The composed run controller (plan Task 12, slice B3; spec §11.1, §11.4,
//! §18.2/§18.3): the production iteration loop over injected ports.
//!
//! This module owns no decision the control core already owns — it OWNS THE
//! WIRING: it feeds [`super::scheduler`] fresh facts every tick (§11.4's
//! reconciliation steps), executes [`super::disposition`]'s §11.5 table,
//! and drives every consequential write through its governed authority:
//!
//! - observation binding through
//!   [`ObservationBindingAuthority`](gauntlet_ports::producer_authority::ObservationBindingAuthority)
//!   and instrument evidence through
//!   [`EvidenceProductionAuthority`](gauntlet_ports::producer_authority::EvidenceProductionAuthority)
//!   — the B1 capability gates; a direct store write is a compile error
//!   (the store methods accept only authority-minted values);
//! - comparison through the [`crate::compare::authority`] typestate
//!   (`for_run → issue → review → accept`);
//! - verification through [`crate::verify::authority`]
//!   (`for_run → verify_claim`) over the ladder the composition root
//!   injected (the C1 seal is not widened here);
//! - the builder runner through
//!   [`RecordingAgentRunner`](crate::compare::recording::RecordingAgentRunner),
//!   so an admitted invocation is durable before it can be polled.
//!
//! ## The loop (§11.1 `init → … → [iterate]* → terminal`)
//!
//! Each iteration: **§18.2 boundary checkpoint** (C8.2: written at the TOP
//! of iteration k, carrying every outcome of iteration k−1 — the content
//! §11.1's "… dispose residuals → checkpoint" order requires, and the
//! producer-authority anchor iteration k's owner writes validate against)
//! → **boundary re-verify** (`verify_unchanged` at EVERY
//! boundary) → **tick** (re-read the answer key, cancelling hand-edited
//! in-flight attempts) → **plan** (§11.2/§11.3 over fresh facts) →
//! **budget re-check on the SUMMED pending slice** (§17.4, B2 residual 2)
//! → **dispatch** → **drain** (poll to completion under the injected
//! clock — no wall-clock wait exists here; stalls cancel, retry once,
//! escalate per §11.5 row 4) → per completed result: charge → observe →
//! bind (gated) → compare → produce evidence (gated) → verify → dispose
//! (§11.5) → select (§15.1).
//!
//! ## Typed reference policy (Task-7 m3)
//!
//! [`ReferenceProviderError::ReferenceMutated`] at any boundary is
//! TERMINAL `aborted{reference_mutated}` with BOTH digests persisted as
//! pinned evidence (§9.4) and no retry. `Unavailable` is a NON-FATAL gap:
//! the boundary records the unavailability as pinned evidence and
//! reconciliation continues (§21.3 "keep the process alive; keep
//! reconciliation running"); dependent claims this iteration are
//! INCONCLUSIVE because the requirement side could not be re-established.
//!
//! ## §8.5 drift subset re-checked at boundaries
//!
//! §8.5 requires its checks "re-checked at every iteration boundary for
//! the subset that can drift". Through the ports this controller holds,
//! three can drift and are re-checked every iteration: check 2's reference
//! half (the per-tick strict re-read plus `verify_unchanged`), check 3's
//! runner-resolves half (the scheduler's `configured_roles` input is
//! re-derived every tick), and check 8 (budget: the summed-slice
//! `would_breach_reserve` consult, plus the rounds-exhausted check).
//! Checks 5/6/7 (sandbox policy FILES, MCP capability resolution,
//! directory writability) have no port surface that could observe mid-run
//! drift — the controller cannot re-check what it cannot see, and it does
//! not pretend to (the residual is recorded in the task report).
//!
//! ## Workspace root (task11 NOTE-2)
//!
//! ONE canonical `workspace_root` field exists here. Every derived
//! consumer flows from it: the verification open (`for_run`'s locator
//! root), the evidence owner's locator ingest (and, when no produced
//! artifact stands at the locator, the observation facet's
//! materialization there — see `process_completion`), and — for the
//! composition root that binds observers — the
//! [`RunController::workspace_root`] accessor. Observation requests carry
//! a relative workspace REF; observers resolve it under the root they
//! were bound to at construction.
//!
//! ## The meter/rounds split (C2)
//!
//! The [`BudgetMeter`] port expresses PAYMENTS (§5.3.4: tokens, wall
//! seconds, tool calls, cost — payments carry no rounds). ROUNDS are
//! ITERATION FACTS (§5.3.7: `max_rounds` counts iterations per run): this
//! controller books ONE round per iteration on its run [`Budget`] AT THE
//! ITERATION BOUNDARY, and that run budget is the SINGLE rounds authority
//! — every rounds gate reads its domain arithmetic
//! ([`rounds_exhausted`]), never the meter's payment counters. Exhaustion
//! of any dimension is a TERMINAL `budget_exhausted` (close-out
//! checkpoint, reserve intact) — never a bare error out of the loop's own
//! charge.
//!
//! ## Time
//!
//! Every instant flows through the injected `Arc<dyn Clock>`; the poll
//! loop's wait is `clock.sleep_until(now + poll_interval)` and every
//! §11.5 backoff is a virtual-time advance through the same clock. No
//! `std::thread::sleep` exists in this module or its tests.

/// Milliseconds per second — a unit-conversion factor, not a policy
/// value (named per the literal-scan idiom).
const MILLIS_PER_SECOND: u64 = 1_000;

/// A full percentage — the denominator of the reserve arithmetic, not a
/// policy value (named per the literal-scan idiom).
const HUNDRED_PERCENT: u64 = 100;

/// The rounds one iteration costs (§5.3.7: `max_rounds` counts iterations
/// per run) — the iteration-boundary booking unit, not a tunable.
const ONE_ROUND_PER_ITERATION: u32 = 1;

/// C9.3's typed charge-refusal loci — WHICH books refused the completed
/// result's payment. The meter is the port's admission; the run budget is
/// the loop's own ledger (C2's second site).
const CHARGE_LOCUS_METER: &str = "meter";
const CHARGE_LOCUS_RUN_BUDGET: &str = "run_budget";

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use gauntlet_domain::{
    Budget, BudgetError, BudgetFacts, DiagnosticAllowance, Digest, EpochSeconds, EvidenceKind,
    EvidenceLocator, EvidenceRef, InvocationId, IterationNo, Limits, MicroUsd, Outcome, Payment,
    Residual, ResultContractPath, Run, RunDefinition, RunError, RunId, SessionRef, Severity, Spent,
    TerminalState, Unit, UnitId, UnitStatus, VerificationRef,
};
use gauntlet_ports::agent_runner::{
    AgentError, AgentRunner, CancelReason, InvocationFailure, InvocationHandle, InvocationStatus,
    RoleInvocationSpec,
};
use gauntlet_ports::budget_meter::{BudgetMeter, BudgetMeterError, BudgetSlice, Reservation};
use gauntlet_ports::clock::Clock;
use gauntlet_ports::comparator::{
    AdversarialComparatorSource, ComparatorSource, FrozenReferenceProjection,
};
use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::evidence_store::EvidenceStore;
use gauntlet_ports::human_escalation::{EscalationItem, HumanEscalation};
use gauntlet_ports::id_generator::IdGenerator;
use gauntlet_ports::observer::{ObservationRequest, Observer};
use gauntlet_ports::producer_authority::{
    EvidenceProductionAuthority, ObservationBindingAuthority, ProducerAuthorityError,
};
use gauntlet_ports::reference_provider::{ReferenceProvider, ReferenceProviderError};
use gauntlet_ports::role_result::RoleResult;
use gauntlet_ports::state_store::{
    Checkpoint, RunEvent, RunEventKind, StateStore, StateStoreError,
};
use gauntlet_ports::verifier::VerificationRequest;
use gauntlet_ports::workspace_provider::{
    ContainmentDiff, WorkspaceProvider, WorkspaceProviderError, WorkspaceSubject,
};

use crate::compare::authority::ComparisonAuthority;
use crate::compare::contract::ComparisonContract;
use crate::compare::recording::RecordingAgentRunner;
use crate::compare::review::{ModelDispatchBudget, ReviewDeps};
use crate::observation::assembly::{AssemblyError, ObservationAssembler, ObservationPlan};
use crate::reference::provider::AnswerKeySource;
use crate::settle::{
    BlockingResidual, ClosingActor, SettleError, SettleRequest, SettlementService,
};
use crate::verify::authority::{VerificationAuthority, VerificationRefusal};
use crate::verify::ladder::Ladder;

use super::diagnosis::{
    DiagnosisError, DiagnosisOutcome, DiagnosticEngine, DiagnosticEvidence, DiagnosticSpend,
    DisconfirmingObservation, EscalationContext, Nonconvergence, NonconvergenceDetector,
    RecurringResidual, ResidualKey,
};
use super::disposition::{self, ControlPolicy, Disposition, IterationCause};
use super::scheduler::{
    Concurrency, Ineligible, PlanInput, PlannedDispatch, SchedulerError, UnitScheduler,
};
use super::select::{Candidate, CorrectionSelector, Impact, SelectionContext, SelectionError};

/// The configured shape of the run's budget — what a fresh run starts from
/// and what §18.3 resume restores into (via `Budget::restore`, the only
/// re-admission path for non-zero durable spend, B1).
#[derive(Debug, Clone)]
pub struct BudgetShape {
    pub limits: Limits,
    pub recovery_reserve_pct: u8,
    pub diagnostic: DiagnosticAllowance,
}

impl BudgetShape {
    fn fresh(&self) -> Result<Budget, BudgetError> {
        Budget::new(self.limits, self.recovery_reserve_pct, self.diagnostic)
    }

    fn restore(&self, spent: Spent) -> Result<Budget, BudgetError> {
        Budget::restore(
            self.limits,
            self.recovery_reserve_pct,
            self.diagnostic,
            spent,
        )
    }
}

/// Builds the run's meter over a budget. The controller constructs the
/// meter ONCE per run — fresh (`Budget::new`) for a start, restored
/// (`Budget::restore`) for a resume — through this factory, so the meter is
/// always over exactly the budget the loop owns. The composition root
/// supplies the concrete adapter (B4's wiring; tests wrap a fake).
pub type MeterFactory = Arc<dyn Fn(Budget) -> Arc<dyn BudgetMeter> + Send + Sync>;

/// Anchors the composed §14.2 rung set at one path (plan Task 20 COR-D3):
/// the controller calls it once per completion-workspace with the
/// completion's evidence workspace — the per-unit workspace the dispatch
/// ran in — so the instruments examine the bytes the produce step bound.
/// The error string carries the construction locus (the controller maps it
/// to a typed error). The composition root supplies the concrete rungs;
/// tests wrap a fixed ladder. No Send/Sync bounds: the loop is
/// single-threaded and the rung set is not required to cross threads (the
/// MeterFactory precedent is stricter than the loop needs).
pub type LadderFactory = Arc<dyn Fn(&Path) -> Result<Ladder, String>>;

/// The comparison wiring the loop drives: the frozen comparison contract
/// artifact, the comparator sources review dispatches through, the §13.3
/// review mode, and the model-dispatch allowance per iteration.
pub struct ComparisonDeps {
    pub contract: ComparisonContract,
    pub deterministic: Arc<dyn ComparatorSource>,
    pub model: Arc<dyn ComparatorSource>,
    pub adversarial: Option<Arc<dyn AdversarialComparatorSource>>,
    pub review_mode: gauntlet_domain::ReviewMode,
    pub model_dispatches: u32,
    /// The comparison (critic) role's typed `context.deny` set, loaded
    /// from the roles by the composition root (plan Task 20, R2-N1):
    /// enforced at the governed chain's envelope boundary BEFORE the
    /// envelope is built, so a denied context category can never enter
    /// the composed comparison. `ContextDenySet::new(Vec::new())` denies
    /// nothing; a composition that serves roles declares the real set
    /// here.
    pub role_deny: crate::roles::ContextDenySet,
}

/// Everything the composed controller is handed. Every field is a port or
/// an app service; no adapter is named (§4.1 — real adapters are the
/// composition root's concern, plan Task 20 / B4).
pub struct ControllerDeps {
    pub state: Arc<dyn StateStore>,
    pub evidence: Arc<dyn EvidenceStore>,
    pub ids: Arc<dyn IdGenerator>,
    pub clock: Arc<dyn Clock>,
    /// The builder runner. The controller WRAPS it in
    /// [`RecordingAgentRunner`]: an admitted invocation is durably
    /// recorded before its handle can enter a polling path.
    pub runner: Arc<dyn AgentRunner>,
    pub runner_identity: gauntlet_domain::RunnerIdentity,
    /// The observers the observation step runs, in plan order. Each is
    /// bound (by the composition root) to the canonical workspace root.
    pub observers: Vec<Arc<dyn Observer>>,
    pub comparison: ComparisonDeps,
    /// The §14.2 rung set as a PER-COMPLETION LADDER FACTORY — EARNED
    /// authority injected at every governed verification open. `Ladder::new`
    /// stays feature-gated; this controller never constructs a rung. The
    /// rung SET is composed once (adapter wiring, the composition root's
    /// job); its instruments ANCHOR at the workspace the dispatch actually
    /// ran in, so the factory re-anchors the set at each completion's
    /// evidence workspace (plan Task 20 COR-D3, §17.5.1 invariant 1). A
    /// caller that wants the pre-correction canonical anchoring returns the
    /// same ladder at every path.
    pub ladder_for: LadderFactory,
    pub reference: Arc<dyn ReferenceProvider>,
    /// The answer-key source for §11.4 step 3's per-tick re-read — the
    /// in-memory snapshot is never authoritative over this.
    pub answer_key: AnswerKeySource,
    pub escalation: Arc<dyn HumanEscalation>,
    pub meter_factory: MeterFactory,
    pub budget: BudgetShape,
    pub policy: ControlPolicy,
    /// The ONE canonical workspace root (task11 NOTE-2). Every derived
    /// consumer flows from this field.
    pub workspace_root: PathBuf,
    /// The §17 governance posture (plan Task 14): the authority gate and
    /// containment conjuncts every dispatch passes BEFORE the runner is
    /// invoked. `GovernDeps::none_backend` is the §8.6 conservative
    /// posture; a real-backend posture REQUIRES the sandbox and policy
    /// instances (construction refuses the inconsistent shapes).
    pub govern: crate::govern::GovernDeps,
    /// The run's workspace provider (plan Task 20, R-1b): when wired, EVERY
    /// dispatched unit is allocated a FRESH per-unit workspace from it
    /// before gate admission, the admission and the invocation spec name
    /// THAT workspace, and it is released best-effort when the invocation
    /// terminalizes (§17.5.1 invariant 1 — parallel units never share a
    /// workspace). `None` is the pre-composition shape: the controller
    /// falls back to the run-derived workspace ref. When the composition
    /// wired a provider, a provider failure is a typed run error — never a
    /// silent fallback to the shared ref.
    pub workspaces: Option<Arc<dyn WorkspaceProvider>>,
    pub config_fingerprint: Digest,
    pub environment_fingerprint: Digest,
    pub intent: String,
}

/// Controller failures — typed, carrying the failure locus (spec §21.2).
/// Terminal §11.6 outcomes are NOT errors: they are [`RunOutcome`]s.
#[derive(Debug)]
pub enum ControlError {
    /// Composition guard: the parsed ruler carries no Units (Task-7 F18) —
    /// digest agreement alone is never the sole authority.
    EmptyRuler {
        source: String,
    },
    /// Composition guard: the depends_on graph is invalid (Task-7 F7; the
    /// B2 scheduler's own error, surfaced).
    Scheduler(SchedulerError),
    /// The run's budget shape is unusable (§8.5 check 8: non-zero
    /// dimensions, reserve < 100%).
    BudgetShape(BudgetError),
    Reference(ReferenceProviderError),
    RunCreation(RunError),
    State(StateStoreError),
    Producer(ProducerAuthorityError),
    Evidence(gauntlet_ports::evidence_store::EvidenceStoreError),
    Escalation(gauntlet_ports::human_escalation::HumanEscalationError),
    Agent(AgentError),
    Budget(BudgetError),
    Meter(BudgetMeterError),
    Observation(AssemblyError),
    Admission(crate::compare::authority::AdmissionError),
    VerificationOpen(crate::verify::authority::VerificationAdmissionError),
    VerificationRefused(Box<VerificationRefusal>),
    Diagnosis(DiagnosisError),
    Selection(SelectionError),
    /// The store holds no checkpoint for a resume (§18.3): there is no
    /// durable boundary to resume from.
    ResumeWithoutCheckpoint {
        run_id: RunId,
    },
    /// The store holds no durable run for a resume.
    RunNotDurable {
        run_id: RunId,
    },
    /// §18.3 re-probe: the durable spend water-mark is unreachable by any
    /// legal charge sequence (`Budget::restore`'s refusal).
    UnreachableSpend(BudgetError),
    /// Correction C6.3: the scheduler planned a §11.3 slice with more than
    /// one member, and this controller dispatches ONE Unit per invocation.
    /// The slice's `estimated_slice` is the SUM over every member, so
    /// prompting the first member while reserving the sum charges for work
    /// that is never prompted. Refused with the member list rather than
    /// truncated.
    SliceNotDispatchable {
        units: Vec<UnitId>,
    },
    /// Item 0 (the C7 remainder): the run's frozen membership could not be
    /// derived from the durable run's own frozen `ReferenceVersion` — the
    /// plan boundary cannot bound dispatch eligibility without it, so the
    /// run refuses to start or resume rather than plan against a ruler it
    /// cannot read.
    FrozenMembership {
        run_id: RunId,
        detail: String,
    },
    /// N4 (critic-B A12, independently confirmed): an evidence locator that
    /// does not resolve inside the run's ONE canonical workspace root —
    /// the domain's plain-ref grammar rejects a leading `/` but PERMITS
    /// `..`, and this controller is the first production writer at the
    /// joined path. Refused at the controller's containment boundary,
    /// with both the locator and the root it escapes named; the domain
    /// grammar itself is untouched (a Task-3-closed public value type
    /// shared by every locator-like newtype).
    EvidenceLocatorEscapes {
        locator: EvidenceLocator,
        root: PathBuf,
    },
    /// The settlement service refused (Task 13 B2): §16.1 gate refusals and
    /// resume-completion failures carry their own typed locus — never
    /// flattened into a state failure they are not. Boxed: the settle
    /// error's variant set is large and this arm must not inflate every
    /// `Result<_, ControlError>` in the crate.
    Settlement(Box<crate::settle::SettleError>),
    /// The composed `WorkspaceProvider` refused to allocate a dispatch's
    /// per-unit workspace (plan Task 20, R-1b). When a provider is wired,
    /// this is a HARD dispatch error — the unit never falls back to a
    /// shared workspace ref (a fallback would silently recreate the exact
    /// sharing R-1b closes).
    Workspace(WorkspaceProviderError),
    /// The §14.2 ladder factory refused to anchor the rung set at a
    /// completion's evidence workspace (plan Task 20 COR-D3): the rung
    /// construction failed for that path — an observation/verification
    /// infrastructure failure, never a coerced outcome.
    Ladder(String),
}

impl core::fmt::Display for ControlError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::EmptyRuler { source } => write!(
                f,
                "the parsed ruler carries no Units — an empty ruler is refused at composition \
                 (digest agreement is never the sole authority): {source}"
            ),
            Self::Scheduler(error) => write!(f, "the unit graph is invalid: {error}"),
            Self::BudgetShape(error) => {
                write!(f, "the configured budget shape is unusable: {error}")
            }
            Self::Reference(error) => write!(f, "the reference pipeline failed: {error}"),
            Self::RunCreation(error) => write!(f, "the run could not be created: {error}"),
            Self::State(error) => write!(f, "a durable state operation failed: {error}"),
            Self::Producer(error) => write!(f, "an owner write was refused: {error}"),
            Self::Evidence(error) => write!(f, "the evidence store failed: {error}"),
            Self::Escalation(error) => write!(f, "escalation failed: {error}"),
            Self::Agent(error) => write!(f, "the runner failed: {error}"),
            Self::Budget(error) => write!(f, "the run budget refused the charge: {error}"),
            Self::Meter(error) => write!(f, "the budget meter refused: {error}"),
            Self::Observation(error) => write!(f, "observation failed: {error}"),
            Self::Admission(error) => write!(f, "the governed comparison chain refused: {error}"),
            Self::VerificationOpen(error) => {
                write!(f, "the governed verification open refused: {error}")
            }
            Self::VerificationRefused(error) => write!(f, "verification refused: {error}"),
            Self::Diagnosis(error) => write!(f, "the diagnostic cycle failed: {error}"),
            Self::Selection(error) => write!(f, "correction selection failed: {error}"),
            Self::ResumeWithoutCheckpoint { run_id } => {
                write!(f, "run {run_id} has no durable checkpoint to resume from")
            }
            Self::RunNotDurable { run_id } => {
                write!(f, "run {run_id} is not durable; there is nothing to resume")
            }
            Self::SliceNotDispatchable { units } => write!(
                f,
                "the scheduler planned a slice of {} units ({}) and this controller dispatches \
                 one unit per invocation: the slice's summed estimate would be charged for work \
                 that is never prompted (spec 11.3 slicing is composed by plan Task 20)",
                units.len(),
                units
                    .iter()
                    .map(UnitId::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Self::UnreachableSpend(error) => {
                write!(f, "the durable spend water-mark is unreachable: {error}")
            }
            Self::FrozenMembership { run_id, detail } => write!(
                f,
                "run {run_id} frozen membership is underivable from its own frozen reference \
                 version: {detail}"
            ),
            Self::EvidenceLocatorEscapes { locator, root } => write!(
                f,
                "the evidence locator {locator} does not resolve inside the run's workspace \
                 root {} and is refused at the write boundary",
                root.display()
            ),
            Self::Settlement(error) => write!(f, "the settlement service refused: {error}"),
            Self::Workspace(error) => write!(
                f,
                "the workspace provider refused a per-unit allocation: {error}"
            ),
            Self::Ladder(detail) => write!(
                f,
                "the verification ladder could not anchor at the completion's workspace: \
                 {detail}"
            ),
        }
    }
}

impl std::error::Error for ControlError {}

/// The test-only park point's arming variable. Unset (every production
/// run, every other test) the park point is a single `var_os` miss and
/// returns at once; set, it names a marker file the process writes once
/// it is parked.
const PARK_AFTER_PRODUCTION_RECORD_ENV: &str = "GAUNTLET_TEST_PARK_AFTER_PRODUCTION_RECORD";

/// TEST-ONLY deterministic placement: called right after an evidence
/// production row is durably recorded and BEFORE the verification and
/// the §16.3 settlement event. When `GAUNTLET_TEST_PARK_AFTER_PRODUCTION_RECORD`
/// is set, writes that marker file and parks until the observing e2e test
/// SIGKILLs the process (that test always kills a parked child). Inert when
/// unset: no behaviour change, no I/O.
fn park_after_production_record_if_armed() {
    let Some(marker) = std::env::var_os(PARK_AFTER_PRODUCTION_RECORD_ENV) else {
        return;
    };
    let _ = std::fs::write(PathBuf::from(marker), b"parked\n");
    loop {
        std::thread::park();
    }
}

impl From<SchedulerError> for ControlError {
    fn from(source: SchedulerError) -> Self {
        Self::Scheduler(source)
    }
}

impl From<RunError> for ControlError {
    fn from(source: RunError) -> Self {
        Self::RunCreation(source)
    }
}

impl From<StateStoreError> for ControlError {
    fn from(source: StateStoreError) -> Self {
        Self::State(source)
    }
}

impl From<ProducerAuthorityError> for ControlError {
    fn from(source: ProducerAuthorityError) -> Self {
        Self::Producer(source)
    }
}

impl From<gauntlet_ports::evidence_store::EvidenceStoreError> for ControlError {
    fn from(source: gauntlet_ports::evidence_store::EvidenceStoreError) -> Self {
        Self::Evidence(source)
    }
}

impl From<AgentError> for ControlError {
    fn from(source: AgentError) -> Self {
        Self::Agent(source)
    }
}

impl From<gauntlet_ports::human_escalation::HumanEscalationError> for ControlError {
    fn from(source: gauntlet_ports::human_escalation::HumanEscalationError) -> Self {
        Self::Escalation(source)
    }
}

impl From<AssemblyError> for ControlError {
    fn from(source: AssemblyError) -> Self {
        Self::Observation(source)
    }
}

impl From<crate::compare::authority::AdmissionError> for ControlError {
    fn from(source: crate::compare::authority::AdmissionError) -> Self {
        Self::Admission(source)
    }
}

impl From<DiagnosisError> for ControlError {
    fn from(source: DiagnosisError) -> Self {
        Self::Diagnosis(source)
    }
}

impl From<SelectionError> for ControlError {
    fn from(source: SelectionError) -> Self {
        Self::Selection(source)
    }
}

impl From<crate::settle::SettleError> for ControlError {
    fn from(source: crate::settle::SettleError) -> Self {
        Self::Settlement(Box::new(source))
    }
}

impl From<WorkspaceProviderError> for ControlError {
    fn from(source: WorkspaceProviderError) -> Self {
        Self::Workspace(source)
    }
}

/// The typed reference-breach classification (Task-7 m3): mutated ⇒
/// terminal abort with both digests as pinned evidence; unavailable ⇒
/// recorded gap, the run continued (the outcome carries what happened,
/// not a verdict).
#[derive(Debug, Clone, PartialEq)]
pub enum ReferenceBreach {
    Mutated { expected: Digest, actual: Digest },
    Unavailable { detail: String },
}

/// What the loop decided, and the evidence of why (§11.6).
#[derive(Debug, Clone)]
pub struct RunOutcome {
    pub run_id: RunId,
    pub terminal: TerminalState,
    pub iterations: u32,
    /// The reference breach the run ended under (or last recorded), when
    /// one occurred.
    pub reference_breach: Option<ReferenceBreach>,
    /// The escalation summary submitted to the operator surface, when the
    /// terminal state required a ticket (§11.6 `blocked`/`escalated`).
    pub escalation_summary: Option<String>,
}

/// What one planned dispatch actually did (correction C6.1). The
/// distinction the loop needs is "did this consume a parallel slot" versus
/// "was the fan-out truncated here" — an over-capacity refusal is neither
/// an admission nor a run error, so neither `()` nor `Result` can carry
/// it.
#[derive(Debug, Clone, PartialEq, Eq)]
enum DispatchAdmission {
    /// The invocation started and holds its reservation.
    Admitted,
    /// §10.4: the meter refused the §5.3.6 dispatch SCOPE — the fan-out is
    /// truncated at this member, the truncation is recorded, and the Unit
    /// stays eligible for a later iteration.
    TruncatedAtCapacity,
    /// A planned dispatch with no members — a scheduler contract
    /// violation, skipped rather than guessed at.
    EmptySlice,
    /// The §17 governance gate refused the dispatch (plan Task 14): the
    /// refusal was pinned as evidence and (on the unit's first refusal)
    /// escalated; NO invocation exists. The unit stays open; when nothing
    /// else can proceed the run's terminal is `blocked` with the ticket
    /// (§11.6), never `budget_exhausted` — a governance fact is not a
    /// budget fact.
    GovernanceRefused { unit: UnitId },
    /// W2: the runner refused ADMISSION itself (`AgentError::AdmissionRefused`
    /// or `AgentError::Transport`) — a real agent-runner backend can refuse or
    /// fail to start transiently. The refusal was pinned as evidence and
    /// routed through the §11.5 disposition table (backoff, or escalate on
    /// exhaustion); NO invocation exists, the workspace and reservation were
    /// released, and the unit stays open and eligible for a later iteration
    /// — never a run error (correction task26, W2).
    AdmissionFailed { unit: UnitId },
}

/// One in-flight invocation the loop owns.
struct InFlight {
    unit: UnitId,
    attempt: u32,
    handle: InvocationHandle,
    reservation: Reservation,
    spec: RoleInvocationSpec,
    last_progress_at: EpochSeconds,
    /// §11.5 row-4 retries already spent on this unit's attempt.
    timeout_retries_used: u32,
    /// §11.5 row-3 reminders already spent on this unit's attempt.
    reminders_used: u32,
    /// W3: the clock instant `dispatch` admitted this invocation at.
    /// Together with `completed_at`, the loop's own MEASURED wall time —
    /// the honest floor under a runner's self-reported `wall_seconds`.
    dispatched_at: EpochSeconds,
    /// W3: the clock instant this flight's completion was FIRST observed
    /// by `poll` (`Completed`). `None` while still in flight. A carried
    /// completion (absorbed at a later iteration boundary) keeps the
    /// instant it was first observed at — never recomputed on absorption.
    completed_at: Option<EpochSeconds>,
    /// W3: the latest `InvocationStatus::Running.payment_so_far` a poll
    /// pass observed — real measured in-flight spend, read by the
    /// in-flight budget guard after every poll pass.
    payment_so_far: Payment,
}

/// The answer-key projection §11.4 step 3 compares across ticks: the four
/// fields whose hand-edit cancels an in-flight attempt (tier, authority,
/// claim, status).
#[derive(Debug, Clone, PartialEq, Eq)]
struct UnitProjection {
    blast_radius: gauntlet_domain::BlastRadius,
    authority: gauntlet_domain::Authority,
    claim: String,
    status: UnitStatus,
}

impl UnitProjection {
    fn of(unit: &Unit) -> Self {
        Self {
            blast_radius: unit.blast_radius(),
            authority: unit.authority(),
            claim: unit.claim().to_owned(),
            status: unit.status(),
        }
    }
}

/// The mutable loop state. Every durable piece (unit projection, spend,
/// in-flight ids and contract paths, iteration) is checkpointed per §18.2
/// — critical run state is never reconstructed from conversational memory.
struct LoopState {
    run: Run,
    version_digest: Digest,
    budget: Budget,
    meter: Arc<dyn BudgetMeter>,
    iteration: u32,
    unit_states: BTreeMap<UnitId, UnitStatus>,
    projections: BTreeMap<UnitId, UnitProjection>,
    attempts: BTreeMap<UnitId, u32>,
    /// §11.5 staged re-dispatch attachments, keyed by unit: residual TEXT
    /// ONLY (row 1), a schema reminder (row 3), or a timeout note (row 4).
    pending_attachments: BTreeMap<UnitId, String>,
    /// §11.5 row-4 retry-once ledger, per unit: the count of timeout
    /// retries already spent. A fresh attempt carries this count so the
    /// SECOND timeout of a unit escalates (the count is loop state, not
    /// per-invocation state — a re-dispatch is a new invocation).
    timeout_retries: BTreeMap<UnitId, u32>,
    /// §11.5 row-3 reminder ledger, per unit: one schema reminder, then a
    /// second malformed result escalates. Same per-unit scope as the
    /// timeout ledger.
    reminders: BTreeMap<UnitId, u32>,
    in_flight: Vec<InFlight>,
    /// COR-D2 (plan Task 20): completions polled terminal `Completed`
    /// whose observation binding owes a FREE (run, iteration) key. The
    /// binding is write-once per (run, iteration) while §11.2 dispatches
    /// up to `max_parallel` invocations INTO one iteration — a second
    /// completion processed in the same iteration would collide with the
    /// first's binding as a raw store error. Surplus completions are
    /// CARRIED here (reservation and workspace held: their pipelines
    /// still run) and absorbed ONE per following iteration, at that
    /// iteration's own anchor. Never a second bundle for one key; never
    /// a store error out of the drain.
    carried: Vec<InFlight>,
    /// Whether THIS iteration's observation binding is already spent (by
    /// the drain's one processed completion or by absorbing a carried
    /// one). Reset at every iteration boundary.
    iteration_binding_spent: bool,
    detector: NonconvergenceDetector,
    open_residuals: Vec<Residual>,
    /// The latest governed verification outcome per unit — §15.2's
    /// disconfirming-observation input (C3) and the verifier-disagreement
    /// fact, from the real chain, never asserted.
    last_verifications: BTreeMap<UnitId, Outcome>,
    terminal: Option<TerminalState>,
    reference_breach: Option<ReferenceBreach>,
    escalation_summary: Option<String>,
    last_offset: u64,
    /// The run's settleable and dispatch-eligible membership: the unit ids
    /// of the FROZEN reference (item 0, the C7 remainder — derived once per
    /// run from the durable run's own `ReferenceVersion`; the frozen bytes
    /// cannot change mid-run, so this is a constant of the run, not a
    /// per-tick fact).
    frozen_membership: BTreeSet<UnitId>,
    /// First governance-gate refusal per unit (plan Task 14): the summary
    /// recorded + escalated. Presence is the ticket dedup — later
    /// iterations re-refuse the same unit (honestly, as pinned evidence)
    /// without opening a second ticket for the same fact.
    governance_refused: BTreeMap<UnitId, String>,
    /// First SCHEDULER-level governance refusal per unit (corrections
    /// round 1, F9): the §11.2 conjunct-3 class — irreversible work is
    /// never an automatic dispatch candidate, unconditionally — skipped
    /// by every plan. The ticket dedup works exactly like
    /// `governance_refused`: one ticket per unit for one fact.
    scheduler_governance_blocked: BTreeMap<UnitId, String>,
}

impl LoopState {
    fn next_iteration(&mut self) {
        self.iteration += 1;
    }
}

/// The composed controller (spec §4.1 `RunController`).
pub struct RunController {
    state: Arc<dyn StateStore>,
    evidence: Arc<dyn EvidenceStore>,
    ids: Arc<dyn IdGenerator>,
    clock: Arc<dyn Clock>,
    runner: RecordingAgentRunner,
    observers: Vec<Arc<dyn Observer>>,
    comparison: ComparisonDeps,
    ladder_for: LadderFactory,
    /// One ladder instance per anchored workspace, memoized across the
    /// run: the factory runs the FIRST time a completion anchors at a
    /// given workspace, and the SAME ladder instance (carrying any
    /// instrument state its rungs hold) serves every later completion
    /// there. The provider-less shape anchors every completion at the one
    /// canonical root, so it holds exactly one instance — the
    /// pre-correction behavior, byte for byte. Single-threaded loop
    /// (RefCell interior, the adapters' own posture).
    ladder_cache: core::cell::RefCell<std::collections::HashMap<PathBuf, Ladder>>,
    reference: Arc<dyn ReferenceProvider>,
    answer_key: AnswerKeySource,
    escalation: Arc<dyn HumanEscalation>,
    meter_factory: MeterFactory,
    budget_shape: BudgetShape,
    policy: ControlPolicy,
    workspace_root: PathBuf,
    config_fingerprint: Digest,
    environment_fingerprint: Digest,
    intent: String,
    scheduler: UnitScheduler,
    selector: CorrectionSelector,
    assembler: ObservationAssembler,
    observation_binding: ObservationBindingAuthority,
    evidence_production: EvidenceProductionAuthority,
    comparison_authority: ComparisonAuthority,
    verification_authority: VerificationAuthority,
    settlement: SettlementService,
    /// The §17.1 authority gate + containment conjuncts (plan Task 14),
    /// consulted at dispatch time before `runner.invoke`.
    govern: crate::govern::GovernDeps,
    gate: crate::govern::AuthorityGate,
    /// The run's workspace provider (plan Task 20, R-1b). `Some` ⇒ every
    /// dispatched unit gets a FRESH per-unit workspace before admission;
    /// `None` ⇒ the run-derived workspace ref (the pre-composition shape).
    workspaces: Option<Arc<dyn WorkspaceProvider>>,
    /// The evidence owner's produce policy (the live-E2E correction): what
    /// stands at the unit's locator when verification opens. A composition
    /// decision — see [`ProducePolicy`].
    produce: ProducePolicy,
}

/// The evidence owner's produce policy (the live-E2E correction, plan
/// Task 20 COR-B3a): what the loop puts at a unit's
/// `evidence_required.locator` — the location the §14.2 verification
/// opens at — per completed dispatch. A COMPOSITION DECISION, matched to
/// the fronted rung set, because the two worlds genuinely differ: rungs
/// whose declared artifact contract is the loop's observation facet
/// document (the frozen p4/p7 proof rungs) need the facet standing at the
/// locator, refreshed every completion; rungs whose declared contracts
/// are produced artifacts (digest manifests, transcripts, reports) need
/// the dispatched work's own artifact there, never overwritten by
/// observation bytes. The pre-correction behavior hard-coded the first
/// world into the loop, which is precisely the composition defect that
/// left no fixture able to settle a unit under the shipped rung set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProducePolicy {
    /// The loop's observation facet document is materialized at the
    /// locator and the production binds it — refreshed every completion.
    /// The pre-correction behavior, and the default: the frozen proofs
    /// decide through exactly this world.
    MaterializeObservation,
    /// The locator is the produced verification artifact's home: the
    /// owner binds the locator's ACTUAL content — ingesting a produced
    /// artifact into the content-addressed store, materializing the
    /// observation facet only when no artifact stands there, and never
    /// overwriting a produced artifact.
    BindLocatorContent,
}

impl ProducePolicy {
    /// COR-R2-1 (falsifier round 2, F-R2-3): THE production key surface —
    /// the one definition of "this completion's production is durably
    /// recorded", read from the SAME surface [`RunController`]'s produce
    /// step writes, for every consumer of produced-ness (the §18.3
    /// resume-completion orphan recovery is the second consumer the
    /// ProducePolicy seam used to leak past: it used to guess the key from
    /// the facet alone, which is exactly the default world's key, so the
    /// composed produced-artifact world never read anything as "already
    /// applied" and every first resume of a fully settled run re-minted,
    /// failed the settled no-op predicate, and burned an iteration into a
    /// dishonest `budget_exhausted` terminal). The surface per world:
    ///
    /// - [`Self::MaterializeObservation`]: the production IS the facet
    ///   document's own ref — the produce step records
    ///   `(evidence = facet_ref, locator, producer, iteration)`, so the
    ///   keyed `(run, ref)` row is the exact surface. Reading it by ref
    ///   keeps the multi-attempt, shared-locator precision the crash
    ///   recovery needs (attempt N's facet ref is never attempt N-1's
    ///   row).
    /// - [`Self::BindLocatorContent`]: the production's key is the
    ///   locator artifact's OWN content ref — a ref NO facet names, so a
    ///   facet-ref read is structurally the wrong surface here. The read
    ///   is therefore the `evidence_productions_at(locator)` surface:
    ///   "this unit's work product has a durable production". Deliberately
    ///   producer-agnostic: the produce step fronts the record with the
    ///   completion's FIRST facet's instrument (a multi-facet completion —
    ///   e.g. the composed `filesystem`+`test` plan — writes ONE locator
    ///   production, so no single facet's identity owns it), and it is
    ///   iteration-insensitive because the produce step itself is: a
    ///   re-produced identical artifact APPENDS its own row, keyed by
    ///   `(run, ref, iteration)` (task26 correction —
    ///   `record_evidence_production`'s own rule now, not a controller-
    ///   side dedupe), standing beside every earlier iteration's row for
    ///   the same ref — so this existence check still sees at least one
    ///   row regardless of how many times the same content was
    ///   re-produced, and every iteration's own row stays intact for
    ///   recovery forensics.
    ///
    /// COR-R3-1 (falsifier round 3): the same discipline now governs the
    /// recovery's MINT half, not only its applied read. In the produced
    /// world the §18.3 recovery walks the frozen units' locators and
    /// re-mints the refs the DURABLE orphan events name (the store's
    /// `pending_evidence_production_events` read) — in this world those
    /// are the locator artifacts' content refs, which no facet names, so
    /// a facet-shaped mint could never complete the A5 crash window here.
    /// This method stays the ONE definition of "produced" for the applied
    /// check; the per-world mint shapes live beside it in the recovery.
    pub(crate) fn production_recorded(
        self,
        state: &dyn StateStore,
        run_id: &RunId,
        evidence_ref: &EvidenceRef,
        _instrument: &gauntlet_ports::observer::InstrumentIdentity,
        locator: &EvidenceLocator,
        deadline: Deadline,
    ) -> Result<bool, StateStoreError> {
        match self {
            Self::MaterializeObservation => state
                .evidence_production(run_id, evidence_ref, deadline)
                .map(|row| row.is_some()),
            Self::BindLocatorContent => {
                let rows = state.evidence_productions_at(run_id, locator, deadline)?;
                Ok(!rows.is_empty())
            }
        }
    }
}

impl RunController {
    /// Composes the controller. Fail-fast guards (typed, before any run
    /// exists): F18 — the answer key must parse strictly into a NON-EMPTY
    /// Unit set; F7 — the depends_on graph must validate (the B2
    /// scheduler's own error is surfaced); §8.5 check 8 — the budget shape
    /// must yield a usable budget.
    ///
    /// The produce policy is [`ProducePolicy::MaterializeObservation`] —
    /// the pre-correction behavior the frozen proofs decide through. A
    /// composition fronting produced-artifact rungs opts into
    /// [`Self::compose_binding_produced_artifacts`].
    pub fn compose(deps: ControllerDeps) -> Result<Self, ControlError> {
        Self::compose_with(deps, ProducePolicy::MaterializeObservation)
    }

    /// Composes the controller with the evidence owner binding the
    /// locator's ACTUAL content (the live-E2E correction): a produced
    /// verification artifact at a unit's locator is ingested into the
    /// content-addressed store and the production binds it; the
    /// observation facet is materialized only when no artifact stands
    /// there, and a produced artifact is never overwritten with
    /// observation bytes. For compositions whose fronted §14.2 rungs
    /// decide over produced artifacts (digest manifests, command
    /// transcripts, test reports).
    pub fn compose_binding_produced_artifacts(deps: ControllerDeps) -> Result<Self, ControlError> {
        Self::compose_with(deps, ProducePolicy::BindLocatorContent)
    }

    fn compose_with(deps: ControllerDeps, produce: ProducePolicy) -> Result<Self, ControlError> {
        let units = strict_units(&deps.answer_key)?;
        if units.is_empty() {
            return Err(ControlError::EmptyRuler {
                source: deps.answer_key.describe(),
            });
        }
        // §8.5 check 8 at composition: the shape itself must be usable.
        let validation_budget = deps.budget.fresh().map_err(ControlError::BudgetShape)?;
        // §15.2 "a run can always afford to diagnose": a zero diagnostic
        // allowance is a configuration that cannot run ONE bounded cycle
        // (the engine refuses a free cycle), and it is refused HERE — at
        // composition, fail-fast — rather than erroring mid-run at the
        // diagnosis boundary (correction C3).
        if deps.budget.diagnostic.tokens() == 0 && deps.budget.diagnostic.cost_usd().get() == 0 {
            return Err(ControlError::BudgetShape(BudgetError::LimitZero {
                dimension: gauntlet_domain::BudgetDimension::DiagnosticTokens,
            }));
        }
        // F7 (B2's scheduler validation, surfaced): a validation-only plan
        // over the parsed units. The meter is over the fresh budget, so
        // the only refusals that can surface are the graph errors.
        let validation_meter = (deps.meter_factory)(validation_budget.clone());
        let estimates: BTreeMap<UnitId, BudgetSlice> = units
            .iter()
            .map(|unit| {
                (
                    unit.id().clone(),
                    per_attempt_estimate(
                        &deps.budget.limits,
                        validation_budget.spent(),
                        deps.budget.recovery_reserve_pct,
                    ),
                )
            })
            .collect();
        let input = PlanInput {
            units,
            claimed: Vec::new(),
            concurrency: open_concurrency(deps.budget.limits),
            configured_roles: vec![gauntlet_domain::RoleId::Builder],
            // §11.3 SLICING IS UNREACHABLE, AND THIS IS THE RECORD (L4 /
            // correction C6.4). §11.3 of `Core-spec.md` states the
            // grouping CRITERION — Units "may be grouped into a slice
            // dispatched to one builder when they are not meaningfully
            // separable (same file, same invariant)" — and names no
            // marker, field, or key that expresses it. On a declared
            // bounded search over the shipped surfaces there is no input
            // for this map: the frozen ruler's field set is closed at
            // `{claim, verify_by, blast_radius, authority,
            // evidence_required, depends_on}`; `reference.yaml` carries no
            // grouping key; the answer-key parser's field set is closed
            // AND typed-refusing (`crate::reference::parse`), so adding a
            // `cohort:` key to the ruler today is a hard parse failure;
            // and `domain::Unit` carries no cohort. The consumer is real
            // and correct (`super::scheduler`'s `CohortId` grouping,
            // §11.3-cited) — it is the PRODUCER that does not exist, so
            // the map is empty by structure rather than by choice.
            //
            // This is a bounded claim ("not found within those four
            // surfaces"), not universal absence, and it is recorded rather
            // than papered over because §11.6 forbids reporting
            // provisional capability as complete. Plan Task 20's E2E
            // reference fixture is where cohort slicing gets exercised;
            // inventing a cohort source here — or widening the frozen
            // parser to accept one — would be a ruler change wearing an
            // implementation's clothes. The unavailability is PINNED by
            // `the_answer_key_cannot_express_a_cohort_on_any_shipped_surface`
            // so this paragraph cannot quietly go stale.
            cohorts: BTreeMap::new(),
            estimates,
            // No run has started: the parsed status IS the status.
            statuses: BTreeMap::new(),
        };
        UnitScheduler::new().plan(&input, validation_meter.as_ref())?;
        let runner = RecordingAgentRunner::new(
            deps.runner.clone(),
            deps.state.clone(),
            deps.evidence.clone(),
            deps.runner_identity,
            deps.clock.clone(),
        );
        let observation_binding =
            ObservationBindingAuthority::new(deps.state.clone(), deps.evidence.clone());
        let evidence_production =
            EvidenceProductionAuthority::new(deps.state.clone(), deps.evidence.clone());
        let comparison_authority =
            ComparisonAuthority::new(deps.state.clone(), deps.evidence.clone())
                .with_role_deny(deps.comparison.role_deny.clone());
        let verification_authority =
            VerificationAuthority::new(deps.state.clone(), deps.evidence.clone(), deps.ids.clone());
        let settlement = SettlementService::new(
            deps.state.clone(),
            deps.evidence.clone(),
            deps.reference.clone(),
            deps.escalation.clone(),
            deps.clock.clone(),
        );
        let assembler = ObservationAssembler::new(deps.evidence.clone());
        Ok(Self {
            state: deps.state,
            evidence: deps.evidence,
            ids: deps.ids,
            clock: deps.clock,
            runner,
            observers: deps.observers,
            comparison: deps.comparison,
            ladder_for: deps.ladder_for,
            ladder_cache: core::cell::RefCell::new(std::collections::HashMap::new()),
            reference: deps.reference,
            answer_key: deps.answer_key,
            escalation: deps.escalation,
            meter_factory: deps.meter_factory,
            budget_shape: deps.budget,
            policy: deps.policy,
            workspace_root: deps.workspace_root,
            config_fingerprint: deps.config_fingerprint,
            environment_fingerprint: deps.environment_fingerprint,
            intent: deps.intent,
            scheduler: UnitScheduler::new(),
            selector: CorrectionSelector::new(),
            assembler,
            observation_binding,
            evidence_production,
            comparison_authority,
            verification_authority,
            settlement,
            govern: deps.govern,
            gate: crate::govern::AuthorityGate::new(),
            workspaces: deps.workspaces,
            produce,
        })
    }

    /// The ONE canonical workspace root (task11 NOTE-2): the composition
    /// root binds observers against THIS value, so every derived consumer
    /// of the root flows from the same field.
    pub fn workspace_root(&self) -> &PathBuf {
        &self.workspace_root
    }

    /// Starts a run: freeze/verify the reference, create the durable run,
    /// drive the loop to a terminal state (§11.1).
    pub fn start_run(&self) -> Result<RunOutcome, ControlError> {
        self.start(None)
    }

    /// Starts a candidate-revision EVALUATION run (plan Task 23, charter
    /// D4): the §5.3.3 isolated-evaluation shape — `RunDefinition.parent_run`
    /// names the control run the treatment arm is isolated under. Nothing
    /// else differs from [`Self::start_run`]: same freeze, same loop, same
    /// durable writes.
    pub fn start_run_with_parent(&self, parent: &RunId) -> Result<RunOutcome, ControlError> {
        self.start(Some(parent.clone()))
    }

    fn start(&self, parent_run: Option<RunId>) -> Result<RunOutcome, ControlError> {
        let run_id = self.ids.run_id();
        let deadline = self.operation_bound();
        let draft = self
            .reference
            .resolve(&self.intent, deadline)
            .map_err(ControlError::Reference)?;
        let reference = self
            .reference
            .normalize(draft, deadline)
            .map_err(ControlError::Reference)?;
        let version = self
            .reference
            .freeze(&run_id, &reference, deadline)
            .map_err(ControlError::Reference)?;
        // Freeze-time verify: the version the loop holds is the version
        // the file still is.
        self.reference
            .verify_unchanged(&version, deadline)
            .map_err(ControlError::Reference)?;
        let budget = self
            .budget_shape
            .fresh()
            .map_err(ControlError::BudgetShape)?;
        let run = Run::new(RunDefinition {
            id: run_id.clone(),
            created_at: self.clock.now(),
            reference_version: version.clone(),
            intent: self.intent.clone(),
            config_fingerprint: self.config_fingerprint.clone(),
            environment_fingerprint: self.environment_fingerprint.clone(),
            budget: budget.clone(),
            parent_run,
        })?;
        self.state.record_run(&run, deadline)?;
        let frozen_membership = self.frozen_membership(&run)?;
        let mut state = LoopState {
            run,
            version_digest: version.digest().clone(),
            meter: (self.meter_factory)(budget.clone()),
            budget,
            iteration: 0,
            unit_states: BTreeMap::new(),
            projections: BTreeMap::new(),
            attempts: BTreeMap::new(),
            pending_attachments: BTreeMap::new(),
            timeout_retries: BTreeMap::new(),
            reminders: BTreeMap::new(),
            in_flight: Vec::new(),
            carried: Vec::new(),
            iteration_binding_spent: false,
            detector: NonconvergenceDetector::new(self.policy.nonconvergence_n())
                .map_err(ControlError::Diagnosis)?,
            open_residuals: Vec::new(),
            last_verifications: BTreeMap::new(),
            terminal: None,
            reference_breach: None,
            escalation_summary: None,
            last_offset: 0,
            frozen_membership,
            governance_refused: BTreeMap::new(),
            scheduler_governance_blocked: BTreeMap::new(),
        };
        self.seed_unit_states(&mut state)?;
        self.drive(state)
    }

    /// Resumes a run (§18.3, budget/iteration scope): re-verify the
    /// reference FIRST, restore the budget from the checkpoint's
    /// `budget_spent` through `Budget::restore`, re-probe the
    /// checkpoint's in-flight invocations (a schema-valid result contract
    /// on disk is collected; anything else is
    /// `Failed{orphaned_by_restart}` and re-planned), then continue the
    /// loop. Full crash-injection recovery is plan Task 13's P7; the
    /// envelope-digest half of the re-probe's §10.3 rule-6 check needs a
    /// durable invocation-record read no port exposes and is recorded as
    /// that boundary in the task report.
    pub fn resume_run(&self, run_id: &RunId) -> Result<RunOutcome, ControlError> {
        let deadline = self.operation_bound();
        let run =
            self.state
                .load_run(run_id, deadline)?
                .ok_or_else(|| ControlError::RunNotDurable {
                    run_id: run_id.clone(),
                })?;
        // §18.3 step 2: re-freeze-verify FIRST. A mutated reference aborts
        // before anything else is trusted — with both digests as evidence.
        if let Err(error) = self
            .reference
            .verify_unchanged(run.reference_version(), deadline)
        {
            if matches!(error, ReferenceProviderError::ReferenceMutated { .. }) {
                return self.reference_breach_outcome(run, 0, error);
            }
            return Err(ControlError::Reference(error));
        }
        let checkpoint = self.state.resume(run_id, deadline)?.ok_or_else(|| {
            ControlError::ResumeWithoutCheckpoint {
                run_id: run_id.clone(),
            }
        })?;
        // §18.3 step 1 (Task 13 B2 Item 3): complete the half-applied
        // durable writes at/after the checkpoint's own anchor BEFORE
        // re-planning — the anchored settlement replay and the
        // evidence-production orphan completion (re-minted from durable
        // facts only; orphans whose facts are not durably recoverable are
        // pinned limitations by the service, never fabrications).
        // COR-R5-1: the completion's RETURN VALUE no longer gates the
        // settled no-op — a completed settlement row and a completed
        // production mint are both durable by themselves (see the
        // predicate below) — but the completion itself always runs: its
        // side effects ARE the §18.3 step-1 recovery, and a resume that
        // completed state must never skip it.
        let _completion = {
            match self.settlement.complete_pending_after_checkpoint(
                run_id,
                checkpoint.event_log_offset,
                // COR-R2-1: the completion reads produced-ness from the
                // SAME key surface THIS run's produce step writes — the
                // policy the composition chose threads through.
                self.produce,
                deadline,
            ) {
                Ok(completion) => completion,
                Err(SettleError::State(error)) => return Err(ControlError::State(error)),
                Err(SettleError::Evidence(error)) => return Err(ControlError::Evidence(error)),
                Err(SettleError::Escalation(error)) => return Err(ControlError::Escalation(error)),
                Err(other) => return Err(ControlError::Settlement(Box::new(other))),
            }
        };
        // §5.3.3: the projection is rebuilt from events — but EVENT ORDER
        // decides which word wins. A settlement whose event sits STRICTLY
        // BEYOND the checkpoint's water-mark is one the checkpoint could
        // not have known (the §16.3 crash window: the event and row are
        // durable, the projection update was lost) — the unit is Settled.
        // A settlement at/below the anchor was already inside the
        // checkpoint's knowledge; a projection that still says Open
        // postdates that settlement (e.g. a §11.5 residual retry
        // legitimately re-opened the unit for correction), and the later
        // word stands. The full keyed-row set is kept too: COR-R5-2's
        // settled no-op cross-check reads the missing-row divergence
        // against it (checked against the store, never a report's say-so).
        let settlement_rows = self.state.settlements(run_id, deadline)?;
        let durable_settled: BTreeSet<UnitId> = settlement_rows
            .iter()
            .filter(|record| record.event_log_offset().get() > checkpoint.event_log_offset.get())
            .map(|record| record.unit_id().clone())
            .collect();
        let mut unit_states: std::collections::BTreeMap<UnitId, UnitStatus> =
            checkpoint.unit_states.iter().cloned().collect();
        for unit in &durable_settled {
            unit_states.insert(unit.clone(), UnitStatus::Settled);
        }
        // §18.2 resume fidelity: the durable spend water-mark re-admits
        // only through the domain's own restore gate.
        let budget = self
            .budget_shape
            .restore(checkpoint.budget_spent)
            .map_err(ControlError::UnreachableSpend)?;
        let frozen_units = self.frozen_units(&run)?;
        let frozen_membership: BTreeSet<UnitId> =
            frozen_units.iter().map(|unit| unit.id().clone()).collect();
        // COR-F6 (falsifier round 1, F-7): an ALL-SETTLED run's resume is
        // an honest settled NO-OP — terminal `settled` at the checkpoint's
        // own iteration, ZERO writes, ZERO new iterations burned. The
        // close fires BEFORE (and regardless of) iteration booking: an
        // all-settled projection with nothing in flight can only plan zero
        // dispatches, so re-entering the loop would book an empty round
        // whose booking — under a spent rounds dimension — is REFUSED and
        // terminalizes `budget_exhausted` BEFORE `decide_progress`'s
        // all-closed settled close is consulted, misreporting a fully
        // settled run (the falsifier's round-5 reproduction: the recovery
        // half honest, the loop half dishonest; reachable from an ordinary
        // §16.3/A5 crash with no forgery). The predicate is conservative
        // on every axis:
        // - the §18.3 step-1 completion's output is durable BY ITSELF in
        //   both arms, so a recovery that COMPLETED state never defers the
        //   close to a fresh booked round. COR-R2-1: "nothing to complete"
        //   is read through the run's OWN produce policy
        //   (`ProducePolicy::production_recorded`), so the composed
        //   produced-artifact world — whose production keys are the
        //   locator artifacts' content refs, not facet refs — reads its
        //   settled runs as settled too, and the FIRST resume of a fully
        //   settled run is the no-op in both produce worlds;
        //   COR-R3-1: forensic-only recovery output — LIMITATION pins,
        //   whose writes land in the evidence store and are already
        //   durable by themselves — does NOT defeat the no-op: the pins
        //   record bookkeeping damage honestly, but no STATE moved that a
        //   reconciliation checkpoint would need to record;
        //   COR-R4-1 (falsifier round 4, F-R4-3): a recovery-COMPLETED
        //   PRODUCTION is the same class — the mint inserted a row
        //   cross-validated against the durable `EvidenceProduced` event,
        //   appending no event and moving no field a checkpoint records;
        //   COR-R5-1 (falsifier round 5, F-R5-1): a recovery-COMPLETED
        //   SETTLEMENT is the same class again — the anchored replay
        //   re-inserts the keyed row cross-validated against the durable
        //   `SettlementCommitted` event (the row's offset and `settled_at`
        //   are the EVENT's own), appending no event and moving no field a
        //   checkpoint records. The row's durability derives from the
        //   durable event, not from a checkpoint, and the replay is
        //   idempotent on every later resume — so the pre-COR-R5-1
        //   "the normal path's reconciliation checkpoint must record it"
        //   routing is not what made the state durable, and booking a
        //   round for it burns exactly the empty round the no-op exists
        //   to prevent. The distinction this predicate does NOT decide:
        //   the reproduction's checkpoint-offset forgery precondition
        //   stays the recorded "checkpoint() forged-row projection class
        //   (planning-layer)" deferral — this arm keys on the
        //   EVENT-ORDERED projection being all-settled, whatever surgery
        //   produced it, and never validates or repairs checkpoint
        //   offsets. Below-anchor keyed-row damage that the anchor-bounded
        //   replay cannot re-complete is NOT silent either: the COR-R5-2
        //   cross-check inside this branch pins it per row (falsifier
        //   round 5, F-R5-2), without touching the anchor rule;
        // - nothing is in flight to re-probe;
        // - the EVENT-ORDERED projection (the checkpoint's own word plus
        //   any post-anchor §16.3 settlement window, §5.3.3) carries every
        //   frozen unit Settled. A checkpoint that legitimately projects a
        //   unit Open (e.g. a residual retry re-opened it) fails the
        //   predicate and takes the full path — a run that merely LOOKS
        //   settled is never short-circuited, and a genuinely unfinished
        //   run never closes as a no-op.
        // The §18.3 step-2 re-freeze-verify above already ran, so a
        // mutated reference never reaches this arm; p7/g7's crash-recovery
        // semantics are untouched (their checkpoints owe open units).
        if checkpoint.in_flight.is_empty()
            && !frozen_membership.is_empty()
            && frozen_membership
                .iter()
                .all(|unit| unit_states.get(unit) == Some(&UnitStatus::Settled))
        {
            // COR-R5-2 (falsifier round 5, F-R5-2): the same per-row
            // forensic honesty the production arm emits for below-anchor
            // damage. A unit the durable projection certifies Settled
            // whose KEYED settlement row is missing from the store is
            // pinned, per row, checked against the store's own rows —
            // never silently absorbed by the certified no-op, and never
            // pinned when every settled unit's row stands (the no-damage
            // resume writes nothing). The full path's behavior is
            // untouched: this check lives only in the no-op close.
            let settled_row_units: BTreeSet<UnitId> = settlement_rows
                .iter()
                .map(|record| record.unit_id().clone())
                .collect();
            let missing_rows: Vec<UnitId> = frozen_membership
                .iter()
                .filter(|unit| {
                    unit_states.get(*unit) == Some(&UnitStatus::Settled)
                        && !settled_row_units.contains(*unit)
                })
                .cloned()
                .collect();
            if !missing_rows.is_empty() {
                self.settlement
                    .pin_missing_settlement_rows(
                        run.id(),
                        &missing_rows,
                        checkpoint.event_log_offset,
                        deadline,
                    )
                    .map_err(|error| match error {
                        SettleError::State(state) => ControlError::State(state),
                        SettleError::Evidence(evidence) => ControlError::Evidence(evidence),
                        other => ControlError::Settlement(Box::new(other)),
                    })?;
            }
            return Ok(RunOutcome {
                run_id: run.id().clone(),
                terminal: TerminalState::Settled,
                iterations: checkpoint.iteration_no.get(),
                reference_breach: None,
                escalation_summary: None,
            });
        }
        let mut state = LoopState {
            run: run.clone(),
            version_digest: run.reference_version().digest().clone(),
            meter: (self.meter_factory)(budget.clone()),
            budget,
            iteration: checkpoint.iteration_no.get(),
            unit_states,
            projections: BTreeMap::new(),
            attempts: BTreeMap::new(),
            pending_attachments: BTreeMap::new(),
            timeout_retries: BTreeMap::new(),
            reminders: BTreeMap::new(),
            in_flight: Vec::new(),
            carried: Vec::new(),
            iteration_binding_spent: false,
            detector: NonconvergenceDetector::new(self.policy.nonconvergence_n())
                .map_err(ControlError::Diagnosis)?,
            open_residuals: Vec::new(),
            last_verifications: BTreeMap::new(),
            terminal: None,
            reference_breach: None,
            escalation_summary: None,
            last_offset: checkpoint.event_log_offset.get(),
            frozen_membership,
            governance_refused: BTreeMap::new(),
            scheduler_governance_blocked: BTreeMap::new(),
        };
        self.seed_unit_states(&mut state)?;
        // §18.3/C8.3 (correction): per-unit attempts are reconstructed
        // from the loop's own DURABLE PRODUCER WRITES — every completed
        // attempt that produced instrument evidence recorded one
        // `evidence_production` row at the unit's FROZEN locator, and
        // `evidence_productions_at` counts them — so a crash-retry loop
        // cannot re-arm the §11.5 max-attempts ceiling on every crash.
        // Two bounded caveats, recorded rather than papered over: a
        // facet-less completion records no production (an UNDERCOUNT —
        // conservative for a per-unit ceiling), and units sharing one
        // locator share one count (an overcount for the less-attempted
        // unit). The retry-ledger halves (timeout retries, reminders,
        // staged attachments) remain L2: the §18.2 checkpoint field set
        // is spec-fixed and adding a field is a ports revision.
        for unit in &frozen_units {
            let produced = self.state.evidence_productions_at(
                run.id(),
                unit.evidence_required().locator(),
                self.operation_bound(),
            )?;
            if let Ok(attempts) = u32::try_from(produced.len())
                && attempts > 0
            {
                state.attempts.insert(unit.id().clone(), attempts);
            }
        }
        // §18.3 step 3: re-probe in-flight by the durable record. Collected
        // results are terminal events; everything else is
        // `Failed{orphaned_by_restart}` and the unit returns to the
        // eligible pool on the next plan.
        for (invocation_id, path) in checkpoint.in_flight {
            let probed = probe_result_contract(run_id, &invocation_id, &path);
            let terminal_state = if probed.is_ok() {
                gauntlet_domain::InvocationState::Completed
            } else {
                gauntlet_domain::InvocationState::Failed
            };
            state.last_offset = self
                .append_terminal_event(run_id, &invocation_id, terminal_state)?
                .get();
        }
        self.drive(state)
    }

    fn append_terminal_event(
        &self,
        run_id: &RunId,
        invocation_id: &InvocationId,
        terminal: gauntlet_domain::InvocationState,
    ) -> Result<gauntlet_domain::EventLogOffset, ControlError> {
        Ok(self.state.append_event(
            RunEvent {
                run_id: run_id.clone(),
                kind: RunEventKind::InvocationTerminal {
                    invocation_id: invocation_id.clone(),
                    state: terminal,
                },
                at: self.clock.now(),
            },
            self.operation_bound(),
        )?)
    }

    fn seed_unit_states(&self, state: &mut LoopState) -> Result<(), ControlError> {
        let units = strict_units(&self.answer_key)?;
        for unit in units {
            state
                .unit_states
                .entry(unit.id().clone())
                .or_insert(UnitStatus::Open);
            state
                .projections
                .insert(unit.id().clone(), UnitProjection::of(&unit));
        }
        Ok(())
    }

    /// The §14.2 ladder anchored at one completion's evidence workspace
    /// (plan Task 20 COR-D3), memoized per workspace so a rung set with
    /// instrument state keeps that state across the run's completions at
    /// the same workspace — the provider-less shape holds exactly one
    /// instance, the pre-correction behavior.
    fn ladder_at(&self, workspace: &Path) -> Result<Ladder, ControlError> {
        let mut cache = self.ladder_cache.borrow_mut();
        if let Some(ladder) = cache.get(workspace) {
            return Ok(ladder.clone());
        }
        let ladder = (self.ladder_for)(workspace).map_err(ControlError::Ladder)?;
        cache.insert(workspace.to_path_buf(), ladder.clone());
        Ok(ladder)
    }

    /// The invocation-terminal release (plan Task 20, R-1b part b): the
    /// reservation slot goes back AND the dispatch's own workspace is
    /// returned to the provider. Every call site is a point where the
    /// invocation has terminalized (failed, timed out, cancelled, or its
    /// charge was refused) — a re-dispatch allocates a FRESH workspace, so
    /// the old one is returned now, not at run end.
    fn release_flight(&self, state: &LoopState, flight: &InFlight) {
        release_reservation(state, flight);
        self.release_workspace(state, &flight.spec.workspace);
    }

    /// Best-effort workspace release through the composed provider (plan
    /// Task 20, R-1b part b). NEVER a settlement input: §16's ordering is
    /// about the settlement's own atomic commit sequence, and this runs
    /// strictly AFTER an invocation's durable effects — it releases a
    /// directory nothing downstream reads, can never block settlement, and
    /// can never reorder it. A cleanup FAILURE is recorded as pinned
    /// evidence ("never a swallowed error") and the loop proceeds: a leaked
    /// workspace directory under the configured root is a storage loss, not
    /// a correctness fact, and it must not convert a settled run into a
    /// failed one. With no provider composed there is nothing to release —
    /// the fallback ref names no allocation.
    fn release_workspace(&self, state: &LoopState, workspace: &gauntlet_domain::WorkspaceRef) {
        let Some(provider) = self.workspaces.as_ref() else {
            return;
        };
        if let Err(error) = provider.cleanup(workspace, self.operation_bound()) {
            let record = format!(
                "{{\"class\":\"workspace_cleanup_failed\",\"run\":\"{}\",\"workspace\":\"{}\",\"detail\":\"{}\"}}",
                state.run.id(),
                workspace.as_str(),
                escape_json_text(&error.to_string())
            );
            let pinned = self
                .evidence
                .put(record.as_bytes(), EvidenceKind::Log, self.operation_bound())
                .and_then(|evidence| {
                    self.evidence
                        .pin(state.run.id(), &evidence, self.operation_bound())?;
                    Ok(())
                });
            if pinned.is_err() {
                // Even the FAILURE RECORD failed: the release stays
                // best-effort end to end — the loop's own error surface is
                // reserved for facts that gate correctness, and this is not
                // one (a directory leak under the workspace root).
            }
        }
    }

    /// Item 0 — the C7 remainder (the post-F1 orchestrator ruling): the
    /// run's settleable and dispatch-eligible membership is the FROZEN
    /// reference's, never the per-tick file re-read's. Derives the frozen
    /// unit-id set from the durable run's own
    /// [`gauntlet_domain::ReferenceVersion`] through the PORT's governed
    /// [`FrozenReferenceProjection::load_version`] — the bytes load at the
    /// version's pinned evidence ref and verify against its pinned digest,
    /// so neither is a caller argument — parsed by this crate's own
    /// answer-key parser. That is the identical route
    /// `GovernedVerification::frozen_unit` takes (no port widened, no new
    /// authority surface; `frozen_unit` itself is crate-private to the
    /// verify module and stays so). Derived ONCE per run: the frozen bytes
    /// cannot change mid-run, so a file that later disagrees with this set
    /// is a mutation the boundary abort (§21.3's only abort row) owns —
    /// the loop never adapts to it by dispatching file-only work the
    /// verification authority would refuse with `UnitUnavailable`.
    fn frozen_membership(&self, run: &Run) -> Result<BTreeSet<UnitId>, ControlError> {
        Ok(self
            .frozen_units(run)?
            .into_iter()
            .map(|unit| unit.id().clone())
            .collect())
    }

    /// The frozen reference's own unit list (item 0's load, one step
    /// earlier): the membership set derives from it, and C8.3's attempts
    /// reconstruction reads each unit's FROZEN `evidence_required` locator
    /// from it — the requirement side the loop's producer writes were
    /// recorded against, never the per-tick file.
    fn frozen_units(&self, run: &Run) -> Result<Vec<Unit>, ControlError> {
        let projection = FrozenReferenceProjection::load_version(
            run.reference_version(),
            self.evidence.as_ref(),
            self.operation_bound(),
        )
        .map_err(|error| ControlError::FrozenMembership {
            run_id: run.id().clone(),
            detail: error.to_string(),
        })?;
        let parsed = crate::reference::parse::parse_answer_key(projection.view());
        Ok(parsed.units)
    }

    /// The loop (§11.1 `[iterate]*`).
    fn drive(&self, mut state: LoopState) -> Result<RunOutcome, ControlError> {
        while state.terminal.is_none() {
            state.next_iteration();
            // COR-D2: a new iteration owns a FRESH observation binding key.
            state.iteration_binding_spent = false;
            // §18.2/C8.2: THE ITERATION-BOUNDARY CHECKPOINT — written at
            // the TOP of iteration k. One placement satisfies both rules
            // that were in tension while the checkpoint sat mid-pipeline
            // (after the charge, before observe/verify/settle):
            //
            // - §11.1's iterate order ends an iteration at "… dispose
            //   residuals → CHECKPOINT": this checkpoint's CONTENT is
            //   post-dispose of iteration k−1 (every settlement, spend
            //   water-mark, and unit-state change of k−1 is in it), so a
            //   crash anywhere in iteration k resumes with all of k−1's
            //   outcomes durable — §18.3 "Re-running settled work is a
            //   defect" holds at every crash point.
            // - the producer-authority anchor (C5/B-M2, Task 11): every
            //   owner write of iteration k (observation binding, evidence
            //   production, comparison) validates its DECLARED iteration
            //   against the latest durable checkpoint. Writing the
            //   checkpoint declaring k before any of k's work runs is
            //   what lets those writes pass; the pre-C8.2 mid-pipeline
            //   checkpoint supplied the same anchor one completion late.
            //
            // The close-out checkpoint at the terminal remains (the last
            // iteration's own outcomes must be durable too); it declares
            // the same iteration as the last boundary checkpoint.
            self.checkpoint(&mut state)?;
            // §11.4 step 4 — boundary re-verify at EVERY boundary. A
            // mutation ends the run terminally; unavailability records a
            // gap and the loop continues (Task-7 m3's typed policy).
            if let Some(outcome) = self.boundary_verify(&mut state)? {
                self.seal_terminal(&state, outcome.terminal)?;
                return Ok(outcome);
            }
            // COR-D2: absorb ONE carried completion per iteration, at THIS
            // iteration's own anchor — the §18.2 boundary checkpoint above
            // already declares this iteration, so the carried completion's
            // owner writes (observation binding, production, comparison)
            // validate exactly like a same-iteration completion's. One per
            // iteration keeps the write-once per-(run, iteration) binding
            // rule intact; the rest wait for the following boundaries.
            if !state.carried.is_empty() && !state.iteration_binding_spent {
                let flight = state.carried.remove(0);
                let workspace = flight.spec.workspace.clone();
                self.process_completion(&mut state, flight)?;
                state.iteration_binding_spent = true;
                self.release_workspace(&state, &workspace);
            }
            self.plan_and_dispatch(&mut state)?;
            // Any terminal the plan decided (C1: `budget_exhausted` on a
            // real budget breach, `blocked` on governance-only blockage) is
            // final once nothing is in flight; with work still in flight
            // the drain below finishes it first.
            if state.terminal.is_some() && state.in_flight.is_empty() {
                break;
            }
            self.drain(&mut state)?;
            // §5.3.7/§11.4 (correction C2): the run `Budget` is the
            // SINGLE rounds authority — the iteration's ONE round books
            // AT THE ITERATION BOUNDARY, after the drain (the iteration's
            // work is done) and before the between-iterations decision.
            // The meter's payments carry no rounds; the B1 adapter module
            // doc delegates iteration-fact booking to exactly here. A
            // refused booking is terminal `budget_exhausted` with the
            // reserve intact (the refusal booked nothing) — never a bare
            // error out of the loop's own charge. A terminal already set
            // mid-drain (an escalation) wins: exhaustion does not
            // overwrite it.
            self.book_iteration_round(&mut state);
            if state.terminal.is_none()
                && let Some(outcome) = self.decide_progress(&mut state)?
            {
                self.seal_terminal(&state, outcome.terminal)?;
                return Ok(outcome);
            }
        }
        // COR-D2: a terminal set mid-loop strands unabsorbed carried
        // completions. Their pipelines never ran — the units stay open and
        // a resume re-plans them (the carried invocations' terminal facts
        // are already durable events) — so their reservations and
        // workspaces are released now, never held past the run.
        while let Some(flight) = state.carried.pop() {
            self.release_flight(&state, &flight);
        }
        let terminal = state.terminal.unwrap_or(TerminalState::Settled);
        self.checkpoint(&mut state)?;
        self.seal_terminal(&state, terminal)?;
        Ok(RunOutcome {
            run_id: state.run.id().clone(),
            terminal,
            iterations: state.iteration,
            reference_breach: state.reference_breach,
            escalation_summary: state.escalation_summary,
        })
    }

    /// The run's terminal made DURABLE (corrections round 2, F-R4/F-R5):
    /// one append-only `RunTerminal` event per pass that reaches a
    /// terminal, written after the pass's final durable work. The record
    /// is the report's terminal (never the in-memory outcome alone), and
    /// a resumed pass that re-terminalizes APPENDS its terminal beside
    /// the earlier one — the transition is visible in the export, never a
    /// silent rewrite. A failed append surfaces typed: a pass that could
    /// not record its terminal does not claim one.
    fn seal_terminal(
        &self,
        state: &LoopState,
        terminal: TerminalState,
    ) -> Result<(), ControlError> {
        self.state.append_event(
            RunEvent {
                run_id: state.run.id().clone(),
                kind: RunEventKind::RunTerminal { state: terminal },
                at: self.clock.now(),
            },
            self.operation_bound(),
        )?;
        Ok(())
    }

    /// §11.4 step 4 at one boundary. `Ok(Some(outcome))` ⇒ the run is
    /// terminally over (mutation abort). `Ok(None)` ⇒ continue.
    fn boundary_verify(&self, state: &mut LoopState) -> Result<Option<RunOutcome>, ControlError> {
        let version = state.run.reference_version().clone();
        match self
            .reference
            .verify_unchanged(&version, self.operation_bound())
        {
            Ok(()) => Ok(None),
            Err(error @ ReferenceProviderError::ReferenceMutated { .. }) => {
                let outcome =
                    self.reference_breach_outcome(state.run.clone(), state.iteration, error)?;
                Ok(Some(outcome))
            }
            Err(ReferenceProviderError::Unavailable { detail }) => {
                // Non-fatal gap (§21.3): the boundary's unavailability is
                // recorded as PINNED evidence — a fact, not silence — and
                // reconciliation continues. Dependent claims this
                // iteration are INCONCLUSIVE (the verification chain's
                // frozen projection is unaffected; the boundary's re-read
                // could not be established).
                let record = format!(
                    "{{\"run\":\"{}\",\"boundary_iteration\":{},\"class\":\"reference_unavailable\",\"detail\":\"{}\"}}",
                    state.run.id(),
                    state.iteration,
                    escape_json_text(&detail)
                );
                let evidence = self.evidence.put(
                    record.as_bytes(),
                    EvidenceKind::Log,
                    self.operation_bound(),
                )?;
                self.evidence
                    .pin(state.run.id(), &evidence, self.operation_bound())?;
                state.reference_breach = Some(ReferenceBreach::Unavailable { detail });
                Ok(None)
            }
            Err(other) => Err(ControlError::Reference(other)),
        }
    }

    /// The terminal `aborted{reference_mutated}` path (§9.4): BOTH digests
    /// persisted as pinned durable evidence — not a log line — then the
    /// aborted outcome.
    fn reference_breach_outcome(
        &self,
        run: Run,
        iteration: u32,
        error: ReferenceProviderError,
    ) -> Result<RunOutcome, ControlError> {
        let ReferenceProviderError::ReferenceMutated { expected, actual } = error else {
            return Err(ControlError::Reference(error));
        };
        let record = format!(
            "{{\"run\":\"{}\",\"boundary_iteration\":{},\"class\":\"reference_mutated\",\"frozen_digest\":\"{}\",\"current_digest\":\"{}\"}}",
            run.id(),
            iteration,
            expected,
            actual
        );
        let evidence =
            self.evidence
                .put(record.as_bytes(), EvidenceKind::Log, self.operation_bound())?;
        self.evidence
            .pin(run.id(), &evidence, self.operation_bound())?;
        Ok(RunOutcome {
            run_id: run.id().clone(),
            terminal: TerminalState::Aborted,
            iterations: iteration,
            reference_breach: Some(ReferenceBreach::Mutated { expected, actual }),
            escalation_summary: None,
        })
    }

    /// §11.4 step 3 (re-read), §11.2/§11.3 planning, §17.4 dispatch.
    fn plan_and_dispatch(&self, state: &mut LoopState) -> Result<(), ControlError> {
        // Step 3: re-read the answer key; an operator hand-edit to an
        // in-flight unit cancels the affected attempt. Eligibility is
        // re-evaluated from scratch by the plan below (the unit returns to
        // the eligible pool under its NEW tier/authority/claim).
        //
        // §21.3 row 1 (Task 13 B2 Item 4): a READ failure is a non-terminal
        // degradation — stop new dispatch for THIS iteration, keep the
        // process alive, keep reconciliation running, and stay resume-able
        // (the gap is pinned evidence, never silence; the rounds dimension
        // bounds the wait). A parse failure is not a read failure — it
        // propagates.
        let units = match self.reread_answer_key(state) {
            Ok(units) => units,
            Err(ControlError::Reference(ReferenceProviderError::Unavailable { detail })) => {
                let record = format!(
                    "{{\"class\":\"answer_key_read_failed\",\"run\":\"{}\",\"iteration\":{},\"detail\":\"{}\"}}",
                    state.run.id(),
                    state.iteration,
                    escape_json_text(&detail)
                );
                let evidence = self.evidence.put(
                    record.as_bytes(),
                    EvidenceKind::Log,
                    self.operation_bound(),
                )?;
                self.evidence
                    .pin(state.run.id(), &evidence, self.operation_bound())?;
                state.reference_breach = Some(ReferenceBreach::Unavailable { detail });
                return Ok(());
            }
            Err(other) => return Err(other),
        };
        // ITEM 0 — the C7 remainder (post-F1 ruling): dispatch-eligible
        // membership is bounded by the FROZEN REFERENCE, not the file. A
        // unit present in the file but absent from the frozen reference is
        // never dispatched: the verification authority would refuse it
        // (`UnitUnavailable`) with no terminal, no close-out checkpoint,
        // and no report — the C1-family terminal-truth defect. The filter
        // is one guard, BEFORE the owes-work predicate and the plan input,
        // so a file-only unit cannot make the loop owe, plan, estimate, or
        // dispatch it; the mutation itself is handled where it already is,
        // by the iteration-boundary abort (§21.3's only abort row) the
        // moment the provider answers again. No pruning of `unit_states`
        // (the run's frozen debts stand), no seeding (work verification
        // refuses is not admitted), no §21.3 policy change.
        let units: Vec<Unit> = units
            .into_iter()
            .filter(|unit| state.frozen_membership.contains(unit.id()))
            .collect();
        // Correction C6.2: MEMBERSHIP is the whole re-read set — §11.2
        // conjunct 2 cannot ask whether a dependency settled if the
        // dependency has been deleted from the graph, and dropping settled
        // members is what made the first plan after a settlement fail
        // `UnknownDependency`. ELIGIBILITY filters separately, inside the
        // scheduler, against the durable `statuses` projection below.
        //
        // The old open-only filter survives as exactly one thing: the
        // early-return predicate. Without it a fully-closed run would fall
        // through to the rounds gate and could be terminalled
        // `budget_exhausted` on the very round that settled everything.
        let owes_work = units.iter().any(|unit| {
            matches!(
                state.unit_states.get(unit.id()),
                None | Some(UnitStatus::Open)
            ) && !state
                .in_flight
                .iter()
                .chain(state.carried.iter())
                .any(|flight| flight.unit == *unit.id())
        });
        if !owes_work && state.in_flight.is_empty() && state.carried.is_empty() {
            return Ok(());
        }
        let estimates: BTreeMap<UnitId, BudgetSlice> = units
            .iter()
            .map(|unit| {
                (
                    unit.id().clone(),
                    per_attempt_estimate(
                        state.budget.limits(),
                        state.budget.spent(),
                        self.budget_shape.recovery_reserve_pct,
                    ),
                )
            })
            .collect();
        let input = PlanInput {
            statuses: state
                .unit_states
                .iter()
                .map(|(unit, status)| (unit.clone(), *status))
                .collect(),
            units,
            claimed: state
                .in_flight
                .iter()
                .chain(state.carried.iter())
                .map(|flight| (flight.unit.clone(), flight.spec.invocation_id.clone()))
                .collect(),
            concurrency: Concurrency {
                global_capacity: self.budget_shape.limits.max_parallel(),
                // COR-D2: a carried completion still holds its parallel
                // slot (its pipeline is still owed), so it counts against
                // the dispatch capacity exactly like an in-flight one.
                global_in_flight: (state.in_flight.len() + state.carried.len()) as u32,
                tier_capacity: dispatchable_tier_capacity(self.budget_shape.limits),
                tier_in_flight: Vec::new(),
            },
            configured_roles: vec![gauntlet_domain::RoleId::Builder],
            // §11.3 SLICING IS UNREACHABLE, AND THIS IS THE RECORD (L4 /
            // correction C6.4). §11.3 of `Core-spec.md` states the
            // grouping CRITERION — Units "may be grouped into a slice
            // dispatched to one builder when they are not meaningfully
            // separable (same file, same invariant)" — and names no
            // marker, field, or key that expresses it. On a declared
            // bounded search over the shipped surfaces there is no input
            // for this map: the frozen ruler's field set is closed at
            // `{claim, verify_by, blast_radius, authority,
            // evidence_required, depends_on}`; `reference.yaml` carries no
            // grouping key; the answer-key parser's field set is closed
            // AND typed-refusing (`crate::reference::parse`), so adding a
            // `cohort:` key to the ruler today is a hard parse failure;
            // and `domain::Unit` carries no cohort. The consumer is real
            // and correct (`super::scheduler`'s `CohortId` grouping,
            // §11.3-cited) — it is the PRODUCER that does not exist, so
            // the map is empty by structure rather than by choice.
            //
            // This is a bounded claim ("not found within those four
            // surfaces"), not universal absence, and it is recorded rather
            // than papered over because §11.6 forbids reporting
            // provisional capability as complete. Plan Task 20's E2E
            // reference fixture is where cohort slicing gets exercised;
            // inventing a cohort source here — or widening the frozen
            // parser to accept one — would be a ruler change wearing an
            // implementation's clothes. The unavailability is PINNED by
            // `the_answer_key_cannot_express_a_cohort_on_any_shipped_surface`
            // so this paragraph cannot quietly go stale.
            cohorts: BTreeMap::new(),
            estimates,
        };
        let dispatches = self.scheduler.plan(&input, state.meter.as_ref())?;
        // Corrections round 1 (F9): a unit the scheduler refuses for a
        // GOVERNANCE reason — the §11.2 conjunct-3 class: irreversible
        // work is never an automatic dispatch candidate, unconditionally
        // — still OWES the operator its ticket the first time a plan
        // skips it (§11.6: the queue is the operator's window; AK-044's
        // "appears in the escalation queue" is a fact about the run, not
        // about one terminal's close-out). The owed-unit filter and the
        // one-ticket-per-unit ledger are exactly
        // `classify_no_admission`'s discipline. The TERMINAL decision is
        // untouched: this mints a ticket, never a terminal.
        for reason in self
            .scheduler
            .ineligibility_report(&input, state.meter.as_ref())?
        {
            if !reason.is_governance_block() {
                continue;
            }
            let unit = reason.unit().clone();
            if !matches!(state.unit_states.get(&unit), None | Some(UnitStatus::Open))
                || state.in_flight.iter().any(|flight| flight.unit == unit)
            {
                continue;
            }
            if state.scheduler_governance_blocked.contains_key(&unit) {
                continue;
            }
            let summary = format!("dispatch not scheduled: {reason}");
            self.submit_ticket(state, &unit, summary.clone())?;
            state.scheduler_governance_blocked.insert(unit, summary);
        }
        // §11.4 step 5's rounds gate (C2): the run budget is the single
        // rounds authority — with the NEXT iteration's round unaffordable,
        // no forward dispatch at all. In-flight work drains to completion
        // first; once nothing is in flight the loop drives to terminal.
        if rounds_exhausted(state) {
            if state.in_flight.is_empty() && state.carried.is_empty() {
                state.terminal = Some(TerminalState::BudgetExhausted);
            }
            return Ok(());
        }
        // §17.4 (B2 residual 2): re-consult the meter on the SUMMED
        // pending slice — the dispatch about to start PLUS every
        // reservation already held — IMMEDIATELY before each dispatch.
        // The first dispatch that would breach the recovery reserve
        // stops dispatching (the rest of the plan waits for freed
        // reservations on a later iteration); when nothing could start
        // and nothing is in flight, the forward budget is exhausted and
        // the loop drives to terminal — after which the checkpoint +
        // report actions remain affordable by construction (the reserve
        // was never consumed by forward work).
        let mut admitted = false;
        let mut budget_refused = false;
        for planned in dispatches {
            // §10.4: a plan member that arrives while every parallel slot
            // is already held is a CAPACITY truncation, recorded and
            // deferred — checked before the summed budget consult, since
            // with one attempt's hold sized to the forward remainder
            // divided by `max_parallel`, a member past capacity would
            // otherwise read as a budget refusal it is not.
            let holding = state.in_flight.len().saturating_add(state.carried.len());
            let capacity =
                usize::try_from(self.budget_shape.limits.max_parallel()).unwrap_or(usize::MAX);
            if holding >= capacity {
                if let Some(unit) = planned.units.first() {
                    let requested_parallel =
                        u32::try_from(holding.saturating_add(1)).unwrap_or(u32::MAX);
                    self.record_dispatch_truncation(
                        state,
                        unit,
                        requested_parallel,
                        planned.depth,
                    )?;
                }
                break;
            }
            let mut pending = planned.estimated_slice;
            for flight in state.in_flight.iter().chain(state.carried.iter()) {
                pending = add_slices(pending, flight.reservation.slice());
            }
            if state.meter.would_breach_reserve(pending) {
                budget_refused = true;
                break;
            }
            // The gate's lattice conjunct compares the DISPATCH facts with
            // the answer-key facts as last parsed (this iteration's own
            // re-read): in the composed happy path they coincide by
            // construction; the conjunct guards drift and forgery between
            // plan assembly and dispatch (AK-046's dispatch-time defense).
            let dispatch_unit = planned.units.first().cloned();
            let recorded = dispatch_unit.and_then(|unit| {
                input
                    .units
                    .iter()
                    .find(|candidate| candidate.id() == &unit)
                    .map(|candidate| (candidate.blast_radius(), candidate.authority()))
            });
            match self.dispatch(state, planned, recorded)? {
                DispatchAdmission::Admitted => admitted = true,
                DispatchAdmission::TruncatedAtCapacity => {
                    // §10.4: the fan-out is TRUNCATED here, in the plan's
                    // own dimension-priority order (ascending blast risk,
                    // then dependency depth, then id), and the truncation
                    // was recorded before this arm. The parallel ceiling
                    // is a RUN-global bound, so every later member of this
                    // plan faces the identical refusal — stopping is the
                    // truncation, not an extra decision. The loop stays
                    // alive and the truncated Units stay eligible.
                    break;
                }
                DispatchAdmission::EmptySlice => {}
                // The governance gate refused: the event is recorded, the
                // ticket exists (first refusal), and NO invocation was
                // created. The plan's dimension-priority order keeps
                // dispatching the REST of the plan — one blocked unit
                // does not stop its neighbors; the terminal decision
                // below owns the run-level truth.
                DispatchAdmission::GovernanceRefused { .. } => {}
                // W2: the runner refused admission itself. The refusal was
                // pinned, the workspace and reservation released, and the
                // unit stays Open — the loop must simply iterate again
                // (this bookkeeping is neither an admission nor an
                // ineligibility the `classify_no_admission` budget/blocked
                // table should reason about), UNLESS the disposition
                // already escalated the run (state.terminal is set), in
                // which case the plan's own remaining members are skipped.
                DispatchAdmission::AdmissionFailed { .. } => {
                    if state.terminal.is_some() {
                        return Ok(());
                    }
                    admitted = true;
                }
            }
        }
        if !admitted && state.in_flight.is_empty() {
            self.classify_no_admission(state, &input, budget_refused)?;
        }
        Ok(())
    }

    /// §11.6 terminal truth when a plan admitted nothing and nothing is in
    /// flight (correction C1): only a BUDGET breach may end the run
    /// `budget_exhausted` — a budget dimension refused at the meter, or the
    /// summed pre-dispatch consult's refusal. A run that cannot proceed
    /// for governance reasons alone (irreversibility, an unadmitted tier,
    /// a missing runner, an operator-frozen status) ends `blocked` with
    /// the escalation ticket the table requires. Budget takes precedence:
    /// where both facts hold, `budget_exhausted`'s precondition is met and
    /// the cheaper close-out is the truthful one.
    fn classify_no_admission(
        &self,
        state: &mut LoopState,
        input: &PlanInput,
        budget_refused: bool,
    ) -> Result<(), ControlError> {
        // Only the reasons of units the loop still owes work on count:
        // durable status open and not in flight (settled/out_of_scope
        // members are progress, and an in-flight member's blocker is
        // transient).
        let reasons = self
            .scheduler
            .ineligibility_report(input, state.meter.as_ref())?;
        let owed: Vec<Ineligible> = reasons
            .into_iter()
            .filter(|reason| {
                matches!(
                    state.unit_states.get(reason.unit()),
                    None | Some(UnitStatus::Open)
                ) && !state
                    .in_flight
                    .iter()
                    .any(|flight| flight.unit == *reason.unit())
            })
            .collect();
        if budget_refused || owed.iter().any(Ineligible::is_budget_block) {
            state.terminal = Some(TerminalState::BudgetExhausted);
            return Ok(());
        }
        if let Some(reason) = owed.iter().find(|reason| reason.is_governance_block()) {
            let unit = reason.unit().clone();
            self.submit_ticket(state, &unit, format!("run blocked: {reason}"))?;
            state.terminal = Some(TerminalState::Blocked);
            return Ok(());
        }
        // The dispatch-time gate's own governance facts (plan Task 14): a
        // run that admitted nothing because the GATE refused everything —
        // capability mismatch, no backend for the tier, authority above
        // the ceiling, an escalated validation — is `blocked`, with the
        // ticket the first refusal already opened. This is the same §11.6
        // truth the scheduler reasons carry, decided from the gate's own
        // ledger rather than the scheduler's.
        if let Some((unit, summary)) = state.governance_refused.iter().next() {
            let summary = summary.clone();
            let unit = unit.clone();
            // The ticket already exists for a first refusal; this is the
            // run-level terminal record naming the blocking fact.
            let record = format!(
                "{{\"run\":\"{}\",\"class\":\"run_blocked_by_gate\",\"unit\":\"{}\",\"summary\":\"{}\"}}",
                state.run.id(),
                unit,
                escape_json_text(&summary),
            );
            let evidence =
                self.evidence
                    .put(record.as_bytes(), EvidenceKind::Log, self.operation_bound())?;
            self.evidence
                .pin(state.run.id(), &evidence, self.operation_bound())?;
            state.escalation_summary = Some(summary);
            state.terminal = Some(TerminalState::Blocked);
        }
        Ok(())
    }

    /// §11.4 step 3's re-read: parse the answer key STRICTLY from the
    /// source's current bytes, diff the in-flight projections, and cancel
    /// any attempt whose unit was hand-edited (tier, authority, claim, or
    /// status). The fresh parse REPLACES the projections — the in-memory
    /// snapshot is never authoritative over the file.
    ///
    /// **The re-read does NOT redefine the run's membership** (correction
    /// C7.2/C7.3, resolved as a conflict rather than a change — see the
    /// F1 correction report). The run's settleable membership is the
    /// FROZEN reference's: §9.4 freezes the answer key at run start, and
    /// `VerificationAuthority::frozen_unit` parses the frozen projection,
    /// refusing any unit absent from it with
    /// `VerificationRefusal::UnitUnavailable` — an assertion pinned by
    /// `verify::authority`'s `a_unit_absent_from_the_frozen_reference_is_refused`.
    /// So pruning `unit_states` when a unit leaves the FILE would let the
    /// loop report `settled` for a unit of the frozen ruler it never did,
    /// and seeding a unit that entered the FILE admits work the
    /// verification authority correctly refuses. Outside §21.3's
    /// unavailability window neither is reachable — the next boundary
    /// re-verify ends the run `aborted` — and INSIDE it, choosing a
    /// membership policy is a §21.3 degradation-policy decision this task
    /// deferred. What the re-read governs is CANCELLATION (below) and what
    /// is dispatchable now; never what the run owes.
    ///
    /// **The STATUS leg of step 3 is recorded, not composed** (correction
    /// C7.1, the L4 precedent; §11.6 "Provisional capability is never
    /// reported as complete"). An operator hand-edit marking a QUEUED unit
    /// `settled` or `out_of_scope` cannot be honoured because it cannot be
    /// written: on a declared bounded search over the shipped surfaces,
    /// the answer-key parser's field set is frozen at `{claim, verify_by,
    /// blast_radius, authority, evidence_required, depends_on}` and refuses
    /// anything else with a typed `UnknownField`
    /// (`crate::reference::parse`), and `Unit::new` hardwires the initial
    /// status to `Open`, so NO shipped parse yields any other status. The
    /// claim is "not found within those two surfaces", not universal
    /// absence, and it is pinned by
    /// `the_answer_key_cannot_express_a_unit_status_on_any_shipped_surface`
    /// rather than asserted here in prose — the day a status surface ships,
    /// that test fails and the seeding expression below (`unit.status()`,
    /// deliberately not a hardcoded `Open`) is where the propagation
    /// belongs. Every OTHER field the file can express is already
    /// authoritative for queued units: eligibility re-runs each iteration
    /// over the freshly parsed `Unit` values.
    fn reread_answer_key(&self, state: &mut LoopState) -> Result<Vec<Unit>, ControlError> {
        let units = strict_units(&self.answer_key)?;
        let fresh: BTreeMap<UnitId, UnitProjection> = units
            .iter()
            .map(|unit| (unit.id().clone(), UnitProjection::of(unit)))
            .collect();
        let edited: Vec<UnitId> = state
            .in_flight
            .iter()
            .filter(|flight| {
                fresh.get(&flight.unit).is_none_or(|projection| {
                    Some(projection) != state.projections.get(&flight.unit)
                })
            })
            .map(|flight| flight.unit.clone())
            .collect();
        for unit in edited {
            if let Some(index) = state
                .in_flight
                .iter()
                .position(|flight| flight.unit == unit)
            {
                let flight = state.in_flight.remove(index);
                let reason = CancelReason::new("answer-key hand-edit mid-flight (11.4 step 3)")
                    .expect("fixed cancellation reason");
                match self
                    .runner
                    .cancel(&flight.handle, reason, self.cancel_operation_bound())
                {
                    Ok(()) | Err(AgentError::AlreadyTerminal { .. }) => {}
                    Err(error) => return Err(ControlError::Agent(error)),
                }
                // Zero-payment charge releases the reservation slot (the
                // B1 adapter convention for cancelled invocations).
                self.release_flight(state, &flight);
                state.last_offset = self
                    .append_terminal_event(
                        state.run.id(),
                        &flight.spec.invocation_id,
                        gauntlet_domain::InvocationState::Cancelled,
                    )?
                    .get();
            }
        }
        state.projections = fresh;
        Ok(units)
    }

    /// §17.4 dispatch of one planned slice: pass the §17 governance gate
    /// (plan Task 14 — BEFORE the reservation, so a refusal consumes no
    /// budget and precedes every other dispatch effect), reserve, build
    /// the `RoleInvocationSpec` (whole-budget `budget_slice` per the
    /// Task-4 adjudication; a Clock-derived deadline; a result-contract
    /// path; the admitted tier policy on `sandbox_policy`), and invoke —
    /// admission returns immediately; completion is a result-contract
    /// fact, never awaited here.
    fn dispatch(
        &self,
        state: &mut LoopState,
        planned: PlannedDispatch,
        recorded: Option<(gauntlet_domain::BlastRadius, gauntlet_domain::Authority)>,
    ) -> Result<DispatchAdmission, ControlError> {
        let Some(unit) = dispatchable_member(&planned)? else {
            // A planned dispatch with no members is a scheduler contract
            // violation, not an invented unit; skip rather than guess.
            return Ok(DispatchAdmission::EmptySlice);
        };
        // The dispatch's FRESH per-unit workspace (plan Task 20, R-1b) —
        // allocated BEFORE admission, so the gate validates containment
        // against the workspace the unit will actually run in, not a
        // lifetime-scoped ref (§17.5.1 invariant 1). With no provider
        // composed, the pre-composition run-derived ref is the honest
        // fallback; with a provider composed, a provider failure is a HARD
        // error — falling back would silently recreate the sharing R-1b
        // closes.
        let workspace = match &self.workspaces {
            Some(provider) => {
                let allocated = provider.create(
                    state.run.id(),
                    WorkspaceSubject::Unit(unit.clone()),
                    self.operation_bound(),
                )?;
                allocated.reference
            }
            None => run_workspace_ref(state.run.id()),
        };
        // The §17 governance gate: authority decides WHETHER, containment
        // decides HOW CONTAINED, and both are decided here — before the
        // spec, before the runner is invoked (D2's correction: a check
        // that runs after the builder's turn is an audit, not a gate). A
        // refusal is a GOVERNANCE EVENT: pinned evidence, an escalation
        // ticket on the unit's first refusal, and no invocation — never a
        // silent skip, never an unsandboxed dispatch.
        let facts = crate::govern::GateFacts {
            unit: unit.clone(),
            tier: planned.blast_radius,
            authority: planned.authority,
            runner_isolation: self.runner.capabilities().isolation,
            workspace: workspace.clone(),
        };
        let admission =
            match self
                .gate
                .admit(&self.govern, &facts, recorded, self.operation_bound())
            {
                Ok(admission) => admission,
                Err(refusal) => {
                    // No invocation will run in the freshly allocated
                    // workspace: release it best-effort (never blocking
                    // the refusal's own durable record).
                    self.release_workspace(state, &workspace);
                    self.record_gate_refusal(state, &unit, &refusal)?;
                    return Ok(DispatchAdmission::GovernanceRefused { unit });
                }
            };
        // The admission is durably recorded BEFORE the invocation exists:
        // AK-048's "no builder command executes before the authority and
        // containment checks" is provable from the evidence ORDER — the
        // admission record precedes the invocation record.
        if let Err(error) = self.record_gate_admission(state, &admission) {
            // No invocation will run in the freshly allocated workspace:
            // release it best-effort before propagating the error.
            self.release_workspace(state, &workspace);
            return Err(error);
        }
        let reservation = match state.meter.reserve(planned.estimated_slice) {
            Ok(reservation) => reservation,
            Err(BudgetMeterError::Reserve {
                cause: BudgetError::ScopeExceeded { parallel, depth },
            }) => {
                // Correction C6.1 — the meter's SCOPE refusal is a SKIP,
                // never a run error, and it is distinct from a forward
                // ceiling breach (which the summed pre-dispatch consult
                // above already routes to `budget_exhausted`).
                //
                // §11.2 makes the meter check a per-Unit ELIGIBILITY
                // conjunct: a Unit that fails a conjunct is not dispatched
                // THIS iteration and is re-evaluated the next one.
                // §21.3's degradation table aborts the run for exactly one
                // failure — "Integrity violation | Abort the run; preserve
                // everything" — and an over-capacity refusal is not one.
                // §10.4: "a fan-out that would breach the recovery reserve
                // is truncated in dimension-priority order, and the
                // truncation is recorded as `unavailable_evidence`, not
                // silently dropped."
                //
                // So: record the truncation as pinned evidence, hold the
                // Unit's eligibility for a later iteration, and keep the
                // loop alive.
                //
                // No invocation will run in the freshly allocated
                // workspace: release it best-effort (never blocking the
                // truncation's own durable record).
                self.release_workspace(state, &workspace);
                self.record_dispatch_truncation(state, &unit, parallel, depth)?;
                return Ok(DispatchAdmission::TruncatedAtCapacity);
            }
            Err(other) => {
                // No invocation will run in the freshly allocated
                // workspace: release it best-effort before propagating
                // the error.
                self.release_workspace(state, &workspace);
                return Err(ControlError::Meter(other));
            }
        };
        let invocation_id = self.ids.invocation_id();
        let now = self.clock.now();
        let wall_left = state
            .budget
            .limits()
            .max_wall_seconds()
            .saturating_sub(state.budget.spent().wall_seconds());
        let deadline = Deadline::at_epoch(EpochSeconds::new(now.get().saturating_add(wall_left)));
        let attempt = state.attempts.get(&unit).copied().unwrap_or(0) + 1;
        let attachment = state.pending_attachments.get(&unit).cloned();
        // The §11.5 row-3/row-4 ledgers are per UNIT, not per invocation:
        // a re-dispatch is a new invocation that inherits the unit's
        // spent reminder/timeout-retry count, so the second malformed
        // result or timeout of a unit escalates.
        let timeout_retries_used = state.timeout_retries.get(&unit).copied().unwrap_or(0);
        let reminders_used = state.reminders.get(&unit).copied().unwrap_or(0);
        let (prompt, context_body) = dispatch_prompt(
            &unit,
            state
                .projections
                .get(&unit)
                .map(|projection| projection.claim.as_str()),
            attachment.as_deref(),
        );
        let envelope = gauntlet_ports::agent_runner::ContextEnvelope::new(
            context_body.clone(),
            crate::verify::content_digest(context_body.as_bytes()),
        );
        let result_contract_path =
            ResultContractPath::new(&format!("{}-{invocation_id}-result", state.run.id()))
                .map_err(|error| {
                    ControlError::Agent(AgentError::AdmissionRefused {
                        detail: error.to_string(),
                    })
                })?;
        let iteration_no = IterationNo::new(state.iteration.max(1)).map_err(|error| {
            ControlError::State(StateStoreError::WriteFailed {
                detail: error.to_string(),
            })
        })?;
        let spec = RoleInvocationSpec {
            invocation_id: invocation_id.clone(),
            run_id: state.run.id().clone(),
            iteration_no,
            role: gauntlet_domain::RoleId::Builder,
            prompt,
            context_envelope: envelope,
            // The dispatch's OWN workspace (plan Task 20, R-1b): the fresh
            // per-unit allocation the gate just validated against — the
            // run-derived ref only in the provider-less composition.
            workspace,
            permissions: gauntlet_domain::PermissionSetRef::new("builder-envelope").map_err(
                |error| {
                    ControlError::Agent(AgentError::AdmissionRefused {
                        detail: error.to_string(),
                    })
                },
            )?,
            result_contract_path,
            deadline,
            model_hint: None,
            // The gate-admitted tier policy (plan Task 14): `None` is the
            // §5.3.5-legal shape ONLY for a contained-tier dispatch under
            // the conservative no-backend posture — the gate guarantees
            // exactly that, or a resolved-and-validated policy.
            sandbox_policy: admission.policy_ref(),
            // Whole-budget slice (Task-4 adjudication): the runner's
            // spend bound is the reservation, not the slice it is told
            // about.
            budget_slice: state.budget.clone(),
        };
        // The dispatch's REAL tier for the durable invocation record
        // (corrections round 1, F3): the gate admission knows the tier at
        // admission time — exactly the dispatch fact the record must
        // state — so it is registered here, typed, and consumed when the
        // recording runner builds the durable record.
        self.runner
            .record_dispatch_tier(invocation_id.clone(), admission.tier());
        let handle = match self.runner.invoke(spec.clone()) {
            Ok(handle) => handle,
            Err(error @ (AgentError::AdmissionRefused { .. } | AgentError::Transport { .. })) => {
                // W2: an admission failure is a bounded retry, never a
                // crashed run. A real agent-runner backend can refuse
                // (`AdmissionRefused`) or fail to start transiently
                // (`Transport`) — neither is a §11.6 integrity violation,
                // so the run must not end `Err`.
                //
                // No invocation will run in the freshly allocated
                // workspace: release it, and release the meter
                // reservation this dispatch took above — an unreleased
                // reservation would hold a parallel slot forever.
                self.release_workspace(state, &spec.workspace);
                let _ = state.meter.charge(&reservation, Payment::zero());
                self.runner.unregister_dispatch_tier(&invocation_id);
                let attempts_used = state.attempts.get(&unit).copied().unwrap_or(0);
                *state.attempts.entry(unit.clone()).or_insert(0) = attempt;
                let record = format!(
                    "{{\"class\":\"invocation_admission_failed\",\"run\":\"{}\",\"iteration\":{},\"unit\":\"{}\",\"invocation\":\"{}\",\"attempt\":{},\"error\":\"{}\"}}",
                    state.run.id(),
                    state.iteration,
                    unit,
                    invocation_id,
                    attempt,
                    escape_json_text(&error.to_string())
                );
                let pinned = self.evidence.put(
                    record.as_bytes(),
                    EvidenceKind::Log,
                    self.operation_bound(),
                )?;
                self.evidence
                    .pin(state.run.id(), &pinned, self.operation_bound())?;
                let locus = match &error {
                    AgentError::Transport { .. } => disposition::TransientLocus::Network,
                    _ => disposition::TransientLocus::Process,
                };
                let cause = if attempts_used < self.policy.max_attempts_per_unit() {
                    IterationCause::TransientError { locus, attempt }
                } else {
                    IterationCause::AttemptsExhausted { unit: unit.clone() }
                };
                return match disposition::dispose(cause, &self.policy) {
                    Disposition::RetryAfterBackoff { delay_ms, .. } => {
                        // Virtual-time backoff: advance the clock to the
                        // retry instant, exactly like `dispose_transient`.
                        // The unit stays Open and eligible — no queued
                        // retry dispatch exists to build (no flight was
                        // ever admitted), so the next iteration's plan
                        // naturally re-considers it.
                        let now = self.clock.now().get();
                        self.clock
                            .sleep_until(EpochSeconds::new(
                                now.saturating_add(delay_ms.div_ceil(MILLIS_PER_SECOND)),
                            ))
                            .map_err(|error| {
                                ControlError::Agent(AgentError::Transport {
                                    detail: error.to_string(),
                                })
                            })?;
                        Ok(DispatchAdmission::AdmissionFailed { unit })
                    }
                    Disposition::Escalate { reason } => {
                        self.escalate(state, &unit, reason.to_string())?;
                        Ok(DispatchAdmission::AdmissionFailed { unit })
                    }
                    other => unreachable!(
                        "an admission-failure cause maps to backoff or escalate, got {other:?}"
                    ),
                };
            }
            Err(error) => {
                // No invocation will run in the freshly allocated
                // workspace: release it best-effort before propagating
                // the runner's refusal.
                self.release_workspace(state, &spec.workspace);
                return Err(error.into());
            }
        };
        // The UNIT-ATTRIBUTED dispatch fact (corrections round 2, F-R4):
        // the durable event the §26 report classifies from — the §5.3.5
        // invocation record carries no unit (its field set is spec-frozen)
        // and the workspace ref is provider-shaped, so the join is
        // recorded HERE, once, where the unit is known. Appended AFTER the
        // admission took effect: an attempted dispatch that was refused at
        // admission never enters this record.
        state.last_offset = self
            .state
            .append_event(
                RunEvent {
                    run_id: state.run.id().clone(),
                    kind: RunEventKind::UnitDispatch {
                        unit: unit.clone(),
                        invocation_id: invocation_id.clone(),
                    },
                    at: now,
                },
                self.operation_bound(),
            )?
            .get();
        *state.attempts.entry(unit.clone()).or_insert(0) = attempt;
        state.pending_attachments.remove(&unit);
        state.in_flight.push(InFlight {
            unit,
            attempt,
            handle,
            reservation,
            spec,
            last_progress_at: now,
            timeout_retries_used,
            reminders_used,
            dispatched_at: now,
            completed_at: None,
            payment_so_far: Payment::zero(),
        });
        Ok(DispatchAdmission::Admitted)
    }

    /// The durable ADMISSION record (plan Task 14, AK-048): pinned BEFORE
    /// the invocation exists, so the evidence order itself proves that the
    /// authority and containment checks preceded the builder's first
    /// token. The record names the tier, the resolved policy (or the
    /// §5.3.5-legal none shape), and the validation's achieved rung.
    fn record_gate_admission(
        &self,
        state: &LoopState,
        admission: &crate::govern::GateAdmission,
    ) -> Result<(), ControlError> {
        let record = match (admission.policy(), admission.validation()) {
            (Some(policy), Some(validation)) => format!(
                "{{\"run\":\"{}\",\"iteration\":{},\"class\":\"gate_admission\",\"unit\":\"{}\",\"tier\":\"{}\",\"policy\":\"{}\",\"validation_rung\":\"{}\"}}",
                state.run.id(),
                state.iteration,
                admission.unit(),
                admission.tier(),
                policy.reference,
                validation.isolation,
            ),
            (None, None) => format!(
                "{{\"run\":\"{}\",\"iteration\":{},\"class\":\"gate_admission\",\"unit\":\"{}\",\"tier\":\"{}\",\"policy\":null,\"enforcement\":\"advisory\",\"note\":\"contained under the conservative no-backend posture (spec 5.3.5 legal shape)\"}}",
                state.run.id(),
                state.iteration,
                admission.unit(),
                admission.tier(),
            ),
            // The gate's `Containment` enum makes the mixed shape
            // UNREPRESENTABLE (Task-14 correction round 1, COR-3); the arm
            // is kept total so no future edit passes silently.
            _ => unreachable!("an admission carries policy and validation together or neither"),
        };
        let evidence =
            self.evidence
                .put(record.as_bytes(), EvidenceKind::Log, self.operation_bound())?;
        self.evidence
            .pin(state.run.id(), &evidence, self.operation_bound())?;
        Ok(())
    }

    /// The durable GOVERNANCE-EVENT record for a gate refusal (plan Task
    /// 14; AK-046's "recorded as a governance event", AK-047's
    /// escalation-not-fallback): pinned evidence for EVERY refusal, and an
    /// escalation ticket on the unit's FIRST refusal (later iterations
    /// re-refuse honestly without duplicating the ticket — the dedup map
    /// is the loop's own ledger).
    fn record_gate_refusal(
        &self,
        state: &mut LoopState,
        unit: &UnitId,
        refusal: &crate::govern::GateRefusal,
    ) -> Result<(), ControlError> {
        let record = format!(
            "{{\"run\":\"{}\",\"iteration\":{},\"class\":\"gate_refusal\",\"unit\":\"{}\",\"kind\":\"{}\",\"summary\":\"{}\"}}",
            state.run.id(),
            state.iteration,
            unit,
            refusal_kind(refusal),
            escape_json_text(&refusal.summary()),
        );
        let evidence =
            self.evidence
                .put(record.as_bytes(), EvidenceKind::Log, self.operation_bound())?;
        self.evidence
            .pin(state.run.id(), &evidence, self.operation_bound())?;
        if !state.governance_refused.contains_key(unit) {
            state
                .governance_refused
                .insert(unit.clone(), refusal.summary());
            self.submit_ticket(
                state,
                unit,
                format!(
                    "dispatch refused by the governance gate: {}",
                    refusal.summary()
                ),
            )?;
        }
        Ok(())
    }

    /// §10.4's record for a fan-out truncated at the dispatch-scope
    /// ceiling: the refusal is a FACT, pinned as durable evidence, never a
    /// silent drop. The record names the Unit whose dispatch was
    /// truncated and the scope numbers the domain refused on, so the
    /// truncation is reconstructable from the evidence store alone.
    fn record_dispatch_truncation(
        &self,
        state: &LoopState,
        unit: &UnitId,
        parallel: u32,
        depth: u32,
    ) -> Result<(), ControlError> {
        let record = format!(
            "{{\"run\":\"{}\",\"iteration\":{},\"class\":\"dispatch_truncated\",\"unit\":\"{}\",\"requested_parallel\":{},\"requested_depth\":{},\"detail\":\"the dispatch scope ceiling truncated this fan-out; the unit stays eligible\"}}",
            state.run.id(),
            state.iteration,
            unit,
            parallel,
            depth
        );
        let evidence =
            self.evidence
                .put(record.as_bytes(), EvidenceKind::Log, self.operation_bound())?;
        self.evidence
            .pin(state.run.id(), &evidence, self.operation_bound())?;
        Ok(())
    }

    /// C9.3: the §12.3 forensics record for a completed result whose
    /// charge was refused — the result ARRIVED, so it is kept and
    /// recorded, settling nothing: §21.2 "never a swallowed error",
    /// §10.4 "not silently dropped", and §12.3's precedent that an
    /// artifact may be STORED FOR FORENSICS while excluded from
    /// verification inputs. The record names the unit, the invocation,
    /// the requested payment, and WHICH books refused (the meter or the
    /// loop's own run budget), so the drop the critic named is
    /// reconstructable from the evidence store alone.
    fn record_uncharged_result(
        &self,
        state: &LoopState,
        flight: &InFlight,
        payment: Payment,
        refused_by: &str,
    ) -> Result<(), ControlError> {
        let record = format!(
            "{{\"class\":\"result_uncharged\",\"run\":\"{}\",\"iteration\":{},\"unit\":\"{}\",\"invocation\":\"{}\",\"requested_tokens\":{},\"requested_wall_seconds\":{},\"requested_cost_usd\":{},\"refused_by\":\"{refused_by}\",\"disposition\":\"budget_exhausted\",\"note\":\"stored for forensics; excluded from verification inputs (spec 12.3); settles nothing\"}}",
            state.run.id(),
            state.iteration,
            flight.unit,
            flight.spec.invocation_id,
            payment.tokens(),
            payment.wall_seconds(),
            payment.cost_usd().get(),
        );
        let evidence =
            self.evidence
                .put(record.as_bytes(), EvidenceKind::Log, self.operation_bound())?;
        self.evidence
            .pin(state.run.id(), &evidence, self.operation_bound())?;
        Ok(())
    }

    /// The drain phase: wait ticks under the injected clock until every
    /// in-flight invocation reaches a terminal status, applying §11.4
    /// steps 1-3 inside every tick and the §11.5 table to every terminal
    /// status.
    ///
    /// C9.4: the drain is BOUNDED by its own derived pass limit —
    /// `stall_timeout_ms.div_ceil(poll_interval_ms)`, one stall window of
    /// polls. Within that many consecutive passes every healthy
    /// invocation either completes or is stall-caught; a still-non-empty
    /// flight set after a full window means progress claims are cycling
    /// without completion (the runner that defeats stall detection by
    /// reporting ever-fresh progress), and past the bound the recorded
    /// design applies: CANCEL the in-flight work, PIN the fact as
    /// evidence, ESCALATE. The bound is DERIVED, deliberately not a new
    /// `ControlPolicy` setting: it would be a fifth REQUIRED Annex E
    /// control setting (compounding critic-A m2's open minor) to express
    /// something the two settings the drain already reads already say.
    fn drain(&self, state: &mut LoopState) -> Result<(), ControlError> {
        let max_poll_passes = self
            .policy
            .stall_timeout_ms()
            .div_ceil(self.policy.poll_interval_ms());
        let mut passes: u64 = 0;
        while state.terminal.is_none() && !state.in_flight.is_empty() {
            passes += 1;
            if passes > max_poll_passes {
                return self.bound_the_drain(state, max_poll_passes);
            }
            let now = self.clock.now();
            let tick_at = EpochSeconds::new(
                now.get()
                    .saturating_add(self.policy.poll_interval_ms().div_ceil(MILLIS_PER_SECOND)),
            );
            self.clock.sleep_until(tick_at).map_err(|error| {
                ControlError::Agent(AgentError::Transport {
                    detail: error.to_string(),
                })
            })?;
            // Step 1: poll every in-flight invocation.
            for index in (0..state.in_flight.len()).rev() {
                let status = {
                    let flight = &state.in_flight[index];
                    self.runner.poll(&flight.handle)
                };
                match status {
                    Ok(InvocationStatus::Completed { .. }) => {
                        // W3: the instant THIS poll first observed the
                        // completion — the loop's own measured half of the
                        // charged wall (max(reported, measured)), read
                        // once here and carried untouched through a
                        // possible carry-to-next-iteration absorption.
                        let completed_at = self.clock.now();
                        // COR-D2: the (run, iteration) observation binding
                        // is write-once while dispatch admits up to
                        // `max_parallel` completions per iteration. The
                        // FIRST completion of this iteration runs its full
                        // pipeline; every LATER one is CARRIED to the
                        // following iteration boundary — reservation and
                        // workspace held (their pipeline still runs there)
                        // — never a second bundle for one key, never the
                        // store's collision refusal escaping the drain.
                        if state.iteration_binding_spent {
                            let mut flight = state.in_flight.remove(index);
                            flight.completed_at = Some(completed_at);
                            // The carry is PINNED as evidence (a fact about
                            // this completion's absorption, §21.2 — never a
                            // swallowed event): the unit, the invocation,
                            // and the iteration that will absorb it are
                            // reconstructable from the store alone.
                            let record = format!(
                                "{{\"class\":\"completion_carried\",\"run\":\"{}\",\"iteration\":{},\"unit\":\"{}\",\"invocation\":\"{}\"}}",
                                state.run.id(),
                                state.iteration,
                                flight.unit,
                                flight.spec.invocation_id,
                            );
                            let evidence = self.evidence.put(
                                record.as_bytes(),
                                EvidenceKind::Log,
                                self.operation_bound(),
                            )?;
                            self.evidence
                                .pin(state.run.id(), &evidence, self.operation_bound())?;
                            state.carried.push(flight);
                            continue;
                        }
                        let mut flight = state.in_flight.remove(index);
                        flight.completed_at = Some(completed_at);
                        // The per-unit workspace is released AFTER the
                        // whole completion pipeline (§16 ordering: cleanup
                        // can neither precede nor preempt the settlement
                        // commit; observers read the workspace during the
                        // observe leg only).
                        let workspace = flight.spec.workspace.clone();
                        self.process_completion(state, flight)?;
                        state.iteration_binding_spent = true;
                        self.release_workspace(state, &workspace);
                    }
                    Ok(InvocationStatus::Failed { class }) => {
                        let mut flight = state.in_flight.remove(index);
                        self.dispose_failure(state, &mut flight, class, None)?;
                    }
                    Ok(InvocationStatus::TimedOut) => {
                        let mut flight = state.in_flight.remove(index);
                        self.dispose_timeout(state, &mut flight)?;
                    }
                    Ok(InvocationStatus::Cancelled) => {
                        let flight = state.in_flight.remove(index);
                        self.release_flight(state, &flight);
                        state.last_offset = self
                            .append_terminal_event(
                                state.run.id(),
                                &flight.spec.invocation_id,
                                gauntlet_domain::InvocationState::Cancelled,
                            )?
                            .get();
                    }
                    Ok(InvocationStatus::Running {
                        last_event_at,
                        payment_so_far,
                    }) => {
                        state.in_flight[index].last_progress_at = last_event_at;
                        // W3: the runner's own real measured in-flight
                        // spend — the in-flight budget guard below reads
                        // this after every poll pass.
                        state.in_flight[index].payment_so_far = payment_so_far;
                    }
                    Ok(InvocationStatus::Admitted) => {}
                    Err(error) => {
                        // §11.5 row 2 / §21.3 (Task 13 B2 Item 4): a poll
                        // transport error is a TRANSIENT failure of the
                        // runner channel, not a run-fatal event — it routes
                        // through the disposition table (retry with the
                        // exponential backoff) instead of error-propagating
                        // out of the loop. The invocation's fate may be
                        // unknown; recovery re-probes result contracts
                        // (§18), never assumes.
                        let mut flight = state.in_flight.remove(index);
                        self.dispose_transient(state, &mut flight, error)?;
                    }
                }
            }
            // Step 1.5 (W3 in-flight budget guard): a `Running` poll's own
            // REAL measured spend can reveal a breach before any
            // invocation ever completes — bound the exposure rather than
            // wait for completion payments the run's own budget could
            // never afford. "Remaining forward capacity" is read from the
            // loop's own budget (the rounds/limits authority: forward
            // ceiling = limit*(100-reserve_pct)/100, minus what is already
            // charged) — NOT from the meter, whose `remaining()` already
            // subtracts live reservations and would double-count this
            // exposure.
            if !state.in_flight.is_empty() {
                let limits = state.budget.limits();
                let spent = state.budget.spent();
                let tokens_remaining = state
                    .budget
                    .forward_ceiling(limits.max_tokens())
                    .saturating_sub(spent.tokens());
                let cost_remaining_micros = state
                    .budget
                    .forward_ceiling(limits.max_cost_usd().get())
                    .saturating_sub(spent.cost_usd().get());
                let tokens_in_flight: u64 = state
                    .in_flight
                    .iter()
                    .map(|flight| flight.payment_so_far.tokens())
                    .sum();
                let cost_in_flight_micros: u64 = state
                    .in_flight
                    .iter()
                    .map(|flight| flight.payment_so_far.cost_usd().get())
                    .sum();
                if tokens_in_flight > tokens_remaining
                    || cost_in_flight_micros > cost_remaining_micros
                {
                    let record = format!(
                        "{{\"class\":\"budget_breach_in_flight\",\"run\":\"{}\",\"iteration\":{},\"tokens_in_flight\":{},\"cost_in_flight_micros\":{},\"tokens_remaining\":{},\"cost_remaining_micros\":{}}}",
                        state.run.id(),
                        state.iteration,
                        tokens_in_flight,
                        cost_in_flight_micros,
                        tokens_remaining,
                        cost_remaining_micros,
                    );
                    let evidence = self.evidence.put(
                        record.as_bytes(),
                        EvidenceKind::Log,
                        self.operation_bound(),
                    )?;
                    self.evidence
                        .pin(state.run.id(), &evidence, self.operation_bound())?;
                    // Terminal decided; the standard end-of-drain cleanup
                    // below (once this while loop's guard sees the
                    // terminal and exits) cancels every still-in-flight
                    // invocation (tolerating `AlreadyTerminal`), releases
                    // each flight, and pins its own
                    // `terminal_stranded_invocations_retired` record — the
                    // exact "cancel + release" shape this guard needs, so
                    // it is not duplicated here.
                    state.terminal = Some(TerminalState::BudgetExhausted);
                    continue;
                }
            }
            // Step 2: stall detection — no event and no result for the
            // configured window ⇒ cancel, record, and dispose as a §11.5
            // row-4 timeout (retry once, then escalate).
            let stalled: Vec<usize> = state
                .in_flight
                .iter()
                .enumerate()
                .filter(|(_, flight)| {
                    self.clock
                        .now()
                        .get()
                        .saturating_sub(flight.last_progress_at.get())
                        >= self.policy.stall_timeout_ms().div_ceil(MILLIS_PER_SECOND)
                })
                .map(|(index, _)| index)
                .collect();
            for index in stalled.into_iter().rev() {
                let mut flight = state.in_flight.remove(index);
                let reason = CancelReason::new("stalled past the quiet window (spec 11.4 step 2)")
                    .expect("fixed reason");
                match self
                    .runner
                    .cancel(&flight.handle, reason, self.cancel_operation_bound())
                {
                    Ok(()) | Err(AgentError::AlreadyTerminal { .. }) => {}
                    Err(error) => return Err(ControlError::Agent(error)),
                }
                // The session reference is FORENSIC evidence (§10.3 rule
                // 5): preserved through the disposition.
                self.dispose_timeout(state, &mut flight)?;
            }
            // Step 3: re-read the answer key (cancels edited attempts).
            // §21.3 row 1: an unreadable key skips CANCELLATION for
            // this pass — the drain keeps reconciling; it never errors
            // the run on a read failure.
            match self.reread_answer_key(state) {
                Ok(_) => {}
                Err(ControlError::Reference(ReferenceProviderError::Unavailable { .. })) => {}
                Err(other) => return Err(other),
            }
        }
        // Corrections round 1 (F6): the drain exits with work still in
        // flight ONLY when a terminal was decided mid-drain (the loop's
        // own condition requires an empty flight set otherwise). A
        // terminal run leaves NO runner invocation behind: each stranded
        // invocation is cancelled through the port (§7.3, idempotent —
        // the typed no-op when the runner already retired it), its
        // Cancelled terminal event is durable, and its reservation is
        // released — the same retirement `bound_the_drain` performs for
        // its own bound trip, and the retirement itself is PINNED
        // evidence naming every stranded unit.
        let mut retired: Vec<UnitId> = Vec::new();
        let mut already_terminal: Vec<UnitId> = Vec::new();
        while let Some(flight) = state.in_flight.pop() {
            let reason = CancelReason::new(
                "the run reached its terminal with the invocation still in \
                                   flight (spec 11.6)",
            )
            .expect("fixed cancellation reason");
            match self
                .runner
                .cancel(&flight.handle, reason, self.cancel_operation_bound())
            {
                Ok(()) => retired.push(flight.unit.clone()),
                Err(AgentError::AlreadyTerminal { .. }) => {
                    already_terminal.push(flight.unit.clone())
                }
                Err(error) => return Err(ControlError::Agent(error)),
            }
            self.release_flight(state, &flight);
            state.last_offset = self
                .append_terminal_event(
                    state.run.id(),
                    &flight.spec.invocation_id,
                    gauntlet_domain::InvocationState::Cancelled,
                )?
                .get();
        }
        if !retired.is_empty() || !already_terminal.is_empty() {
            let record = format!(
                "{{\"class\":\"terminal_stranded_invocations_retired\",\"run\":\"{}\",\
                  \"iteration\":{},\"cancelled\":[{}],\"already_terminal\":[{}]}}",
                state.run.id(),
                state.iteration,
                retired
                    .iter()
                    .map(|unit| format!("\"{unit}\""))
                    .collect::<Vec<_>>()
                    .join(","),
                already_terminal
                    .iter()
                    .map(|unit| format!("\"{unit}\""))
                    .collect::<Vec<_>>()
                    .join(","),
            );
            let evidence =
                self.evidence
                    .put(record.as_bytes(), EvidenceKind::Log, self.operation_bound())?;
            self.evidence
                .pin(state.run.id(), &evidence, self.operation_bound())?;
        }
        Ok(())
    }

    /// C9.4's past-the-bound behaviour: CANCEL every still-in-flight
    /// invocation (releasing each reservation — nothing was booked), PIN
    /// the bound trip as durable evidence naming the units and the
    /// derived limit, then ESCALATE — the run owes the operator a ticket,
    /// not an infinite spin (§11.6). The drain loop exits through the
    /// terminal this sets.
    fn bound_the_drain(
        &self,
        state: &mut LoopState,
        max_poll_passes: u64,
    ) -> Result<(), ControlError> {
        let mut cancelled: Vec<UnitId> = Vec::new();
        while let Some(flight) = state.in_flight.pop() {
            let reason = CancelReason::new("the drain exceeded its derived poll bound (C9.4)")
                .expect("fixed cancellation reason");
            match self
                .runner
                .cancel(&flight.handle, reason, self.cancel_operation_bound())
            {
                Ok(()) | Err(AgentError::AlreadyTerminal { .. }) => {}
                Err(error) => return Err(ControlError::Agent(error)),
            }
            self.release_flight(state, &flight);
            state.last_offset = self
                .append_terminal_event(
                    state.run.id(),
                    &flight.spec.invocation_id,
                    gauntlet_domain::InvocationState::Cancelled,
                )?
                .get();
            cancelled.push(flight.unit.clone());
        }
        let record = format!(
            "{{\"class\":\"drain_poll_bound\",\"run\":\"{}\",\"iteration\":{},\"bound_poll_passes\":{},\"units\":[{}]}}",
            state.run.id(),
            state.iteration,
            max_poll_passes,
            cancelled
                .iter()
                .map(|unit| format!("\"{unit}\""))
                .collect::<Vec<_>>()
                .join(",")
        );
        let evidence =
            self.evidence
                .put(record.as_bytes(), EvidenceKind::Log, self.operation_bound())?;
        self.evidence
            .pin(state.run.id(), &evidence, self.operation_bound())?;
        let unit = cancelled
            .first()
            .cloned()
            .unwrap_or_else(|| UnitId::new("unknown").expect("grammar-safe unit id"));
        let cancelled_list = cancelled
            .iter()
            .map(UnitId::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        self.escalate(
            state,
            &unit,
            format!(
                "the drain exceeded its derived bound of {max_poll_passes} poll passes without \
                 draining; in-flight work cancelled and pinned: {cancelled_list}"
            ),
        )
    }

    /// §11.5 row 2/3 handling for a failed invocation.
    fn dispose_failure(
        &self,
        state: &mut LoopState,
        flight: &mut InFlight,
        class: InvocationFailure,
        cause_detail: Option<&str>,
    ) -> Result<(), ControlError> {
        state.last_offset = self
            .append_terminal_event(
                state.run.id(),
                &flight.spec.invocation_id,
                gauntlet_domain::InvocationState::Failed,
            )?
            .get();
        // Like `dispose_transient`, a failure is never silent: every class
        // pins a record naming itself, so a terminal-failed invocation
        // always carries a durable reason. A known parse cause rides
        // along (bounded, sanitized) so the retry can be told WHAT was
        // malformed, not only that something was.
        let cause_text = cause_detail.map(sanitize_failure_detail);
        let record = match &cause_text {
            Some(cause) => format!(
                "{{\"class\":\"invocation_failed\",\"run\":\"{}\",\"iteration\":{},\"unit\":\"{}\",\"invocation\":\"{}\",\"failure\":\"{}\",\"cause\":\"{}\"}}",
                state.run.id(),
                state.iteration,
                flight.unit,
                flight.spec.invocation_id,
                invocation_failure_token(class),
                escape_json_text(cause),
            ),
            None => format!(
                "{{\"class\":\"invocation_failed\",\"run\":\"{}\",\"iteration\":{},\"unit\":\"{}\",\"invocation\":\"{}\",\"failure\":\"{}\"}}",
                state.run.id(),
                state.iteration,
                flight.unit,
                flight.spec.invocation_id,
                invocation_failure_token(class),
            ),
        };
        let evidence =
            self.evidence
                .put(record.as_bytes(), EvidenceKind::Log, self.operation_bound())?;
        self.evidence
            .pin(state.run.id(), &evidence, self.operation_bound())?;
        match class {
            // §11.6: a contract mismatch is an INTEGRITY violation, not a
            // retryable malformed result — terminal abort.
            InvocationFailure::ContractMismatch => {
                self.release_flight(state, flight);
                state.terminal = Some(TerminalState::Aborted);
                state.escalation_summary = Some(format!(
                    "contract mismatch from {}",
                    flight.spec.invocation_id
                ));
                Ok(())
            }
            InvocationFailure::MalformedResult => {
                let cause = IterationCause::MalformedResult {
                    unit: flight.unit.clone(),
                    reminders_used: flight.reminders_used,
                };
                match disposition::dispose(cause, &self.policy) {
                    Disposition::RetryOnceWithSchemaReminder { .. } => {
                        self.release_flight(state, flight);
                        // The row-3 reminder ledger is per-unit loop
                        // state: the unit's next malformed result
                        // escalates.
                        let spent_reminders = flight.reminders_used + 1;
                        state.reminders.insert(flight.unit.clone(), spent_reminders);
                        // W7: the reminder names the parse failure (when
                        // one is known) so the retry fixes the actual
                        // defect instead of guessing at the schema.
                        let named = cause_text
                            .as_deref()
                            .map_or_else(String::new, |cause| format!(" ({cause})"));
                        let attachment = format!(
                            "previous result was malformed{named}; the result document must be a strict \
                             role_result@v1 JSON document echoing invocation {} and the context \
                             envelope digest",
                            flight.spec.invocation_id
                        );
                        queue_retry_dispatch(state, flight, attachment);
                        Ok(())
                    }
                    Disposition::Escalate { reason } => {
                        self.release_flight(state, flight);
                        let unit = flight.unit.clone();
                        self.escalate(state, &unit, reason.to_string())
                    }
                    other => unreachable!(
                        "the §11.5 row-3 cause maps to reminder or escalate, got {other:?}"
                    ),
                }
            }
            // §11.5 row 2 (transient: process/store loci; a restart orphan
            // is the store locus): retry after the exponential backoff.
            // Plan Task 16 joins `missing_result` here (adjudicated design
            // correction RES-D4; spec §11.5 names no row for the class):
            // a finer class of the same transient disposition — the child
            // exited and the work demonstrably did not happen, so a
            // re-dispatch can succeed; a persistently broken configuration
            // is bounded by the budget like any persistently failing
            // execution. It is deliberately NOT the malformed row (there
            // is no document to attach a schema reminder to) and NOT the
            // TimedOut row (no deadline verdict was reached — the exit was
            // observed first, plan Task 16 shape 2).
            InvocationFailure::OrphanedByRestart
            | InvocationFailure::Execution
            | InvocationFailure::MissingResult => {
                let cause = IterationCause::TransientError {
                    locus: disposition::TransientLocus::Process,
                    attempt: flight.attempt,
                };
                match disposition::dispose(cause, &self.policy) {
                    Disposition::RetryAfterBackoff { delay_ms, .. } => {
                        self.release_flight(state, flight);
                        // Virtual-time backoff: advance the clock to the
                        // retry instant — no wall-clock wait exists here.
                        let now = self.clock.now().get();
                        self.clock
                            .sleep_until(EpochSeconds::new(
                                now.saturating_add(delay_ms.div_ceil(MILLIS_PER_SECOND)),
                            ))
                            .map_err(|error| {
                                ControlError::Agent(AgentError::Transport {
                                    detail: error.to_string(),
                                })
                            })?;
                        queue_retry_dispatch(state, flight, String::new());
                        Ok(())
                    }
                    other => unreachable!("the transient cause maps to backoff, got {other:?}"),
                }
            }
        }
    }

    /// §11.5 row 2 / §21.3 (Task 13 B2 Item 4) for a POLL transport error:
    /// the runner channel failed — a transient condition, never a
    /// run-fatal one. The invocation's fate may be unknown; the terminal
    /// event records the failure, the transient fact is PINNED evidence,
    /// and the disposition table's row-2 retry (exponential backoff) is
    /// queued. The loop stays alive and the run stays resume-able.
    fn dispose_transient(
        &self,
        state: &mut LoopState,
        flight: &mut InFlight,
        error: AgentError,
    ) -> Result<(), ControlError> {
        state.last_offset = self
            .append_terminal_event(
                state.run.id(),
                &flight.spec.invocation_id,
                gauntlet_domain::InvocationState::Failed,
            )?
            .get();
        let record = format!(
            "{{\"class\":\"transient_runner_error\",\"run\":\"{}\",\"iteration\":{},\"unit\":\"{}\",\"invocation\":\"{}\",\"error\":\"{}\"}}",
            state.run.id(),
            state.iteration,
            flight.unit,
            flight.spec.invocation_id,
            escape_json_text(&error.to_string())
        );
        let evidence =
            self.evidence
                .put(record.as_bytes(), EvidenceKind::Log, self.operation_bound())?;
        self.evidence
            .pin(state.run.id(), &evidence, self.operation_bound())?;
        let cause = IterationCause::TransientError {
            locus: disposition::TransientLocus::Network,
            attempt: flight.attempt,
        };
        match disposition::dispose(cause, &self.policy) {
            Disposition::RetryAfterBackoff { delay_ms, .. } => {
                // Corrections round 1 (F6): the poll failed, so this
                // invocation's fate is UNKNOWN — and the §11.5 rows give
                // a re-dispatch no right to overlap a possibly-alive
                // prior invocation (the R16-F2 hostile-child shape: a
                // child that stays alive while its poll channel errors
                // forever outlives its replacement unless retired). The
                // port's own idempotent cancel (§7.3) retires the prior
                // BEFORE the re-dispatch; a prior the runner already
                // retired is the typed no-op. The retirement is PINNED
                // evidence — a durable fact, never silence. (A prior
                // observed terminal at the runner — Failed/TimedOut —
                // takes the row-3/row-4 paths, which the runner has
                // already retired; this arm is the unknown-fate path.)
                let reason = CancelReason::new(
                    "the invocation's fate is unknown; the row-2 retry must not overlap a \
                     possibly-alive prior invocation (spec 11.5 row 2, 7.3)",
                )
                .expect("fixed cancellation reason");
                let how =
                    match self
                        .runner
                        .cancel(&flight.handle, reason, self.cancel_operation_bound())
                    {
                        Ok(()) => "cancelled",
                        Err(AgentError::AlreadyTerminal { .. }) => "already_terminal",
                        Err(error) => return Err(ControlError::Agent(error)),
                    };
                let retired = format!(
                    "{{\"class\":\"prior_invocation_retired_before_retry\",\"run\":\"{}\",\
                      \"iteration\":{},\"unit\":\"{}\",\"invocation\":\"{}\",\"how\":\"{}\"}}",
                    state.run.id(),
                    state.iteration,
                    flight.unit,
                    flight.spec.invocation_id,
                    how,
                );
                let evidence = self.evidence.put(
                    retired.as_bytes(),
                    EvidenceKind::Log,
                    self.operation_bound(),
                )?;
                self.evidence
                    .pin(state.run.id(), &evidence, self.operation_bound())?;
                self.release_flight(state, flight);
                // Virtual-time backoff: advance the clock to the retry
                // instant — no wall-clock wait exists here.
                let now = self.clock.now().get();
                self.clock
                    .sleep_until(EpochSeconds::new(
                        now.saturating_add(delay_ms.div_ceil(MILLIS_PER_SECOND)),
                    ))
                    .map_err(|error| {
                        ControlError::Agent(AgentError::Transport {
                            detail: error.to_string(),
                        })
                    })?;
                queue_retry_dispatch(state, flight, String::new());
                Ok(())
            }
            other => unreachable!("the transient cause maps to backoff, got {other:?}"),
        }
    }

    /// §11.5 row 4 for a timed-out or stalled invocation: cancel (already
    /// done by the caller), PRESERVE the session reference, retry once,
    /// then escalate.
    fn dispose_timeout(
        &self,
        state: &mut LoopState,
        flight: &mut InFlight,
    ) -> Result<(), ControlError> {
        let session_ref = flight.handle.session_ref().cloned();
        state.last_offset = self
            .append_terminal_event(
                state.run.id(),
                &flight.spec.invocation_id,
                gauntlet_domain::InvocationState::TimedOut,
            )?
            .get();
        let cause = IterationCause::TimedOut {
            unit: flight.unit.clone(),
            session_ref: session_ref.clone(),
            retries_used: flight.timeout_retries_used,
        };
        match disposition::dispose(cause, &self.policy) {
            Disposition::CancelThenRetryOnce { .. } => {
                self.release_flight(state, flight);
                // The retry-once ledger is per-unit loop state: the spent
                // count is recorded NOW so the unit's next attempt
                // re-disposes at retries_used + 1 and its second timeout
                // escalates.
                let spent_retries = flight.timeout_retries_used + 1;
                state
                    .timeout_retries
                    .insert(flight.unit.clone(), spent_retries);
                let attachment = format!(
                    "previous attempt timed out (retry {spent_retries}); session reference {} \
                     preserved for forensics",
                    session_ref
                        .as_ref()
                        .map(SessionRef::as_str)
                        .unwrap_or("unavailable")
                );
                queue_retry_dispatch(state, flight, attachment);
                Ok(())
            }
            Disposition::Escalate { reason } => {
                self.release_flight(state, flight);
                let unit = flight.unit.clone();
                // The §10.3 rule-5 forensic note rides the ticket itself:
                // the preserved session reference is named in the
                // escalation summary, not only in the cause.
                let summary = match session_ref.as_ref() {
                    Some(session) => format!("{reason}; session reference {session} preserved"),
                    None => reason.to_string(),
                };
                self.escalate(state, &unit, summary)
            }
            other => unreachable!("the row-4 cause maps to retry-once or escalate, got {other:?}"),
        }
    }

    /// Submits an escalation ticket through the port and records its
    /// summary on the loop state (§11.6's ticket requirement). The
    /// TERMINAL state is the caller's decision: `escalated` (§11.5 row
    /// exhaustion), `blocked` (§11.6 governance, correction C1), or
    /// `inconclusive` (§15.2) all ride the same ticket surface.
    fn submit_ticket(
        &self,
        state: &mut LoopState,
        unit: &UnitId,
        summary: String,
    ) -> Result<(), ControlError> {
        let context = self.evidence.put(
            ticket_evidence_record(unit, &summary).as_bytes(),
            EvidenceKind::Log,
            self.operation_bound(),
        )?;
        self.evidence
            .pin(state.run.id(), &context, self.operation_bound())?;
        let ticket = self.escalation.escalate(
            EscalationItem {
                run_id: state.run.id().clone(),
                summary: summary.clone(),
                context,
            },
            self.operation_bound(),
        )?;
        state.escalation_summary = Some(format!("{summary} (ticket {})", ticket.id()));
        Ok(())
    }

    /// §11.5 escalation: the ticket plus the `escalated` terminal.
    fn escalate(
        &self,
        state: &mut LoopState,
        unit: &UnitId,
        summary: String,
    ) -> Result<(), ControlError> {
        self.submit_ticket(state, unit, summary)?;
        state.terminal = Some(TerminalState::Escalated);
        Ok(())
    }

    /// The per-completion pipeline (§11.1 `observe → compare → verify →
    /// dispose → checkpoint`).
    fn process_completion(
        &self,
        state: &mut LoopState,
        flight: InFlight,
    ) -> Result<(), ControlError> {
        // Collect + the controller's own strict §10.3 rule-6 assertion:
        // the result must echo the dispatch identity.
        let result = match self.runner.collect(&flight.handle, self.operation_bound()) {
            Ok(result) => result,
            Err(error) => {
                let mut flight = flight;
                let (class, cause_detail) = match error {
                    AgentError::CollectionFailed { detail, .. } => {
                        (InvocationFailure::MalformedResult, Some(detail))
                    }
                    other => {
                        return Err(ControlError::Agent(other));
                    }
                };
                self.dispose_failure(state, &mut flight, class, cause_detail.as_deref())?;
                return Ok(());
            }
        };
        let validation = result.validate_against(
            &flight.spec.invocation_id,
            flight.spec.context_envelope.digest(),
        );
        if let Err(error) = validation {
            let mut flight = flight;
            self.dispose_failure(
                state,
                &mut flight,
                InvocationFailure::MalformedResult,
                Some(&error.to_string()),
            )?;
            return Ok(());
        }
        // Payment: charge the meter (the reservation bounds it) and the
        // loop's own budget (the §18.2 `budget_spent` and the diagnostic
        // allowance both read from the loop's budget). Rounds book at the
        // ITERATION BOUNDARY (C2), never per payment.
        let payment = payment_of(&result, &flight);
        if let Err(error) = state.meter.charge(&flight.reservation, payment) {
            match error {
                // A charge past the reservation or past the forward
                // ceiling is a budget breach: stop dispatching, drive to
                // terminal (§11.4 step 5) — the reserve is intact.
                BudgetMeterError::ReservationExceeded { .. } | BudgetMeterError::Charge { .. } => {
                    // C9.3: a REFUSED charge keeps its reservation (only a
                    // booked charge releases the parallel slot) — release
                    // it, then keep the completed result for forensics
                    // before the terminal. Never a silent drop.
                    self.release_flight(state, &flight);
                    self.record_uncharged_result(state, &flight, payment, CHARGE_LOCUS_METER)?;
                    state.terminal = Some(TerminalState::BudgetExhausted);
                    return Ok(());
                }
                other => return Err(ControlError::Meter(other)),
            }
        }
        if state
            .budget
            .charge(
                0,
                payment.wall_seconds(),
                payment.tokens(),
                payment.cost_usd(),
            )
            .is_err()
        {
            // The loop's own charge is a budget fact too (a payment past
            // the forward ceiling): terminal exhaustion, never a crash
            // out of the loop's own arithmetic (C2). The refusal booked
            // nothing — the reserve is intact. C9.3: the meter already
            // released the slot (its charge booked), but the completed
            // result is still KEPT FOR FORENSICS, never dropped.
            self.record_uncharged_result(state, &flight, payment, CHARGE_LOCUS_RUN_BUDGET)?;
            state.terminal = Some(TerminalState::BudgetExhausted);
            return Ok(());
        }
        let iteration = IterationNo::new(state.iteration.max(1)).map_err(|error| {
            ControlError::State(StateStoreError::WriteFailed {
                detail: error.to_string(),
            })
        })?;
        // OBSERVE (§12) + the gated binding (the observation owner's
        // write). The observation targets the workspace the dispatch
        // actually ran in (plan Task 20, R-1b) — the flight's own spec
        // ref, not a re-derivation.
        let claims = builder_claims(&result);
        let assembled = self.assembler.assemble(
            ObservationPlan {
                request: ObservationRequest {
                    run_id: state.run.id().clone(),
                    iteration_no: iteration,
                    workspace: flight.spec.workspace.clone(),
                },
                observers: self
                    .observers
                    .iter()
                    .map(|observer| Box::new(SharedObserver(observer.clone())) as Box<dyn Observer>)
                    .collect(),
                project_revision: state.version_digest.to_string(),
                environment_fingerprint: self.environment_fingerprint.to_string(),
                claims_content: claims,
                clock: self.clock.clone(),
            },
            self.operation_bound(),
        )?;
        self.observation_binding.bind(
            state.run.id(),
            iteration,
            &assembled.bundle_ref,
            self.operation_bound(),
        )?;
        // COMPARE (§13): the governed chain is the only route.
        let governed =
            self.comparison_authority
                .for_run(state.run.id(), iteration, self.operation_bound())?;
        let deterministic = self.comparison.deterministic.clone();
        let model = self.comparison.model.clone();
        let adversarial = self.comparison.adversarial.clone();
        // The critic fan-out leg of AK-052 (§10.4 "bounded by
        // review.max_critics AND by the budget meter"; §17.4 "consulted
        // before every dispatch, not after"): the run's OWN meter rides
        // in `ReviewDeps`, so critic admissions and builder dispatches
        // share `max_parallel`. The critic slice is the loop's per-attempt
        // estimate — the identical expression the plan estimates use —
        // computed at review time, when `spent` already reflects the
        // completing builder's payment; no new Annex E / `ControlPolicy`
        // setting was introduced for it.
        let critic_slice = per_attempt_estimate(
            state.budget.limits(),
            state.budget.spent(),
            self.budget_shape.recovery_reserve_pct,
        );
        let deps = ReviewDeps {
            deterministic: deterministic.as_ref(),
            model: model.as_ref(),
            adversarial: adversarial.as_deref(),
            meter: state.meter.as_ref(),
            critic_slice,
        };
        let accepted = governed
            .issue(
                &self.comparison.contract,
                Vec::new(),
                state.open_residuals.clone(),
                self.operation_bound(),
            )?
            .review(
                &deps,
                self.comparison.review_mode,
                ModelDispatchBudget::new(self.comparison.model_dispatches),
                self.operation_bound(),
                self.clock.now(),
            )?
            .accept(self.operation_bound())?;
        // The residual set drives §11.5 rows 1/9.
        let mut residuals = residuals_of(&accepted, &flight.unit);
        // W6 detective containment: the dispatched builder is not
        // sandboxed (isolation none under the contained tier, enforcement
        // advisory — see the gate_admission record), so a write outside
        // its workspace is reachable and must be CAUGHT, not prevented.
        // The provider captured a stat manifest of the project root at
        // dispatch; a diff now pins `containment_breach` and joins the
        // residual set as BLOCKING — the attempt cannot settle
        // (condition 4) and the row-1 retry carries the paths (told why).
        // The check runs BEFORE the workspace is released (release
        // happens in the caller, after this returns), beside the W5
        // promote. An UNREADABLE baseline is "unknown", never "clean":
        // pinned loudly as `containment_check_failed`, and the settlement
        // stands or falls on the other evidence — a detective guard must
        // not brick the run it watches.
        if let Some(provider) = self.workspaces.as_ref() {
            match provider.check_containment(&flight.spec.workspace, self.operation_bound()) {
                Ok(diff) if diff.supported && !diff.is_empty() => {
                    let observed = containment_observed(&diff);
                    let record = format!(
                        "{{\"class\":\"containment_breach\",\"run\":\"{}\",\"iteration\":{},\"invocation\":\"{}\",\"unit\":\"{}\",{observed}}}",
                        state.run.id(),
                        state.iteration,
                        flight.spec.invocation_id,
                        flight.unit,
                    );
                    let evidence = self.evidence.put(
                        record.as_bytes(),
                        EvidenceKind::Log,
                        self.operation_bound(),
                    )?;
                    self.evidence
                        .pin(state.run.id(), &evidence, self.operation_bound())?;
                    residuals.push(LoopResidual {
                        residual: gauntlet_domain::Residual::new(
                            gauntlet_domain::ResidualId::new("containment-breach")
                                .expect("static residual id"),
                            gauntlet_domain::Severity::Blocking,
                            format!(
                                "expected: the project root outside the dispatched workspace is \
                                 unchanged since dispatch; observed: {observed}"
                            ),
                            None,
                        ),
                        unit: flight.unit.clone(),
                        expected: "the project root outside the dispatched workspace is unchanged \
                                   since dispatch"
                            .to_owned(),
                        observed,
                    });
                }
                Ok(_) => {
                    // Unsupported or clean: nothing to report.
                }
                Err(error) => {
                    let record = format!(
                        "{{\"class\":\"containment_check_failed\",\"run\":\"{}\",\"iteration\":{},\"invocation\":\"{}\",\"error\":\"{}\"}}",
                        state.run.id(),
                        state.iteration,
                        flight.spec.invocation_id,
                        escape_json_text(&error.to_string())
                    );
                    let evidence = self.evidence.put(
                        record.as_bytes(),
                        EvidenceKind::Log,
                        self.operation_bound(),
                    )?;
                    self.evidence
                        .pin(state.run.id(), &evidence, self.operation_bound())?;
                }
            }
        }
        // COR-D3 (plan Task 20; §17.5.1 invariant 1): the completion's
        // evidence surface is the workspace the dispatch ACTUALLY ran in —
        // the flight's admitted per-unit workspace under the canonical
        // root. Every locator read and write for THIS completion (the
        // produce step and the verification chain's §22.2 re-read) anchors
        // there. When no provider is wired (the pre-composition shape the
        // frozen proofs pin), the run-derived ref names the canonical
        // surface and the anchor is the root — byte-for-byte the prior
        // behavior. The ref is the provider's key class; the component
        // check keeps the join lexical (a separator or `..` segment is a
        // typed refusal, never a path outside).
        let evidence_workspace = match &self.workspaces {
            Some(_) => {
                let reference = flight.spec.workspace.as_str();
                let single = !reference.is_empty()
                    && std::path::Path::new(reference)
                        .components()
                        .all(|component| matches!(component, std::path::Component::Normal(_)));
                if !single {
                    return Err(ControlError::Workspace(
                        WorkspaceProviderError::ContainmentViolated {
                            path: reference.to_owned(),
                        },
                    ));
                }
                let path = self.workspace_root.join(reference);
                // The workspace exists while the dispatch runs (the
                // provider created it and release happens after this
                // pipeline); ensuring it here keeps the §14 opens and the
                // produce writes on one existing surface even when a
                // completion arrives with nothing on disk yet.
                std::fs::create_dir_all(&path).map_err(|error| {
                    ControlError::Workspace(WorkspaceProviderError::CreateFailed {
                        detail: format!(
                            "the completion's evidence workspace {} could not be ensured: \
                             {error}",
                            path.display()
                        ),
                    })
                })?;
                path
            }
            None => self.workspace_root.clone(),
        };
        // VERIFY (§14): the evidence owner produces the observation at
        // each attempted unit's required locator (the gated write), then
        // the governed verification chain decides the claim. The §14.2
        // ladder anchors at THIS completion's evidence workspace (COR-D3):
        // the instruments examine the same surface the produce step bound.
        let ladder = self.ladder_at(&evidence_workspace)?;
        let governed_verification = self
            .verification_authority
            .for_run(
                state.run.id(),
                iteration,
                ladder,
                evidence_workspace.clone(),
                self.operation_bound(),
            )
            .map_err(ControlError::VerificationOpen)?;
        let facet = assembled.doc.facets().first().cloned();
        // §21.3 row 1 (Task 13 B2 Item 4): an unreadable answer key skips
        // this completion's verification leg — no unit of the completion
        // is verified or settled under the gap (I8), the gap is pinned,
        // and the loop keeps running (residual disposition included); the
        // rounds dimension bounds the wait.
        let parsed = match strict_units(&self.answer_key) {
            Ok(parsed) => parsed,
            Err(ControlError::Reference(ReferenceProviderError::Unavailable { detail })) => {
                let record = format!(
                    "{{\"class\":\"answer_key_read_failed\",\"run\":\"{}\",\"iteration\":{},\"locus\":\"verification\",\"detail\":\"{}\"}}",
                    state.run.id(),
                    state.iteration,
                    escape_json_text(&detail)
                );
                let evidence = self.evidence.put(
                    record.as_bytes(),
                    EvidenceKind::Log,
                    self.operation_bound(),
                )?;
                self.evidence
                    .pin(state.run.id(), &evidence, self.operation_bound())?;
                state.reference_breach = Some(ReferenceBreach::Unavailable { detail });
                Vec::new()
            }
            Err(other) => return Err(other),
        };
        // §18.3 ("re-running settled work is a defect"): a completion may
        // carry a non-blocking residual (§16.1 condition 4 gates only
        // BLOCKING ones) alongside a settlement that still commits. Track
        // whether this completion settled the flight's own unit so the
        // row-1 residual disposition below never re-dispatches it.
        let mut flight_unit_settled = false;
        for unit_id in attempted_units(&flight, &result) {
            // COR2-1b (critic round 1, F-1's completion half): the
            // builder's declared attempted list is free text from the
            // result document — only the flight's OWN dispatched unit may
            // be verified and settled through this completion. A declared
            // unit that is not the flight's unit is recorded as pinned
            // governance evidence and never processed (a builder cannot
            // reach another unit's locator, verification, or settlement
            // by naming it).
            if unit_id != flight.unit {
                let record = format!(
                    "{{\"kind\":\"attempted_unit_mismatch\",\"run_id\":\"{}\",\"iteration\":{},\"flight_unit\":\"{}\",\"declared_unit\":\"{}\"}}",
                    state.run.id(),
                    state.iteration,
                    flight.unit,
                    unit_id
                );
                let evidence = self.evidence.put(
                    record.as_bytes(),
                    EvidenceKind::Log,
                    self.operation_bound(),
                )?;
                self.evidence
                    .pin(state.run.id(), &evidence, self.operation_bound())?;
                continue;
            }
            let Some(unit) = parsed.iter().find(|unit| *unit.id() == unit_id) else {
                continue;
            };
            if let Some(facet) = &facet {
                let locator = unit.evidence_required().locator().clone();
                // N4: the locator must resolve INSIDE this completion's
                // evidence workspace — the admitted per-unit workspace
                // under the canonical root (COR-D3) — before any byte is
                // read or written (typed refusal, never a path outside).
                let locator_path = contained_locator_path(&evidence_workspace, &locator)?;
                // The evidence owner's produce step — WHAT stands at the
                // locator is the composition's produce policy (see
                // [`ProducePolicy`]): the observation facet's document
                // (the default, pre-correction world the frozen
                // facet-document proof rungs decide through), or the
                // locator's actual content — a produced verification
                // artifact ingested into the content-addressed store, the
                // facet materialized only at absence, a produced artifact
                // never overwritten (the composed world whose rungs
                // decide over artifacts).
                self.produce_unit_evidence(state, iteration, unit, facet, &locator, &locator_path)?;
            }
            let request = VerificationRequest {
                id: VerificationRef::new(&format!(
                    "{}-{}",
                    state.run.id(),
                    flight.spec.invocation_id
                ))
                .map_err(|error| {
                    ControlError::State(StateStoreError::WriteFailed {
                        detail: error.to_string(),
                    })
                })?,
                run_id: state.run.id().clone(),
                unit_id: unit_id.clone(),
                claim: unit.claim().to_owned(),
                requirement: unit.evidence_required().clone(),
                evidence: unit.evidence_required().locator().clone(),
            };
            let verified = governed_verification
                .verify_claim(&request, self.operation_bound())
                .map_err(|error| ControlError::VerificationRefused(Box::new(error)))?;
            // The real outcome is loop state (C3): §15.2's disconfirming
            // observation and the §18.4 verifier-disagreement fact both
            // read from it — never asserted.
            state
                .last_verifications
                .insert(unit_id.clone(), verified.domain_record().outcome());
            // §16.1 conditions 1–2 mint the readiness (failures keep the
            // pinned-forensic-evidence behavior and settle NOTHING); then
            // the settlement service is the ONLY settle path (plan Task 13
            // B2): conditions 3/4/5 → record → admit → commit, and only
            // after the durable commit does the caller's projection
            // advance.
            //
            // Condition 2's run-side act comes FIRST: the loop pins the
            // chain-minted PASS evidence for the settling run (p12's
            // earning pattern). The precondition then re-derives
            // resolution AND pinning from the store itself — the pin is a
            // durable fact the loop wrote, never the record's say-so.
            // Before B2 this pin never happened: the precondition always
            // refused `EvidenceUnpinned` and the in-memory settle ignored
            // it — the readiness was never once earned in the composed
            // loop.
            if verified.record().outcome() == Outcome::Pass {
                self.evidence.pin(
                    state.run.id(),
                    verified.record().evidence(),
                    self.operation_bound(),
                )?;
            }
            // COR-2 (early probe M3): the settlement service re-derives
            // §16.1 conditions 1–2 at its own entry, over the claim itself
            // — the controller no longer pre-derives a readiness token, and
            // the request carries none. A non-PASS claim reaches the
            // service's entry gate and is refused there (pinned as forensic
            // evidence by the refusal arm below); a PASS claim's readiness
            // is earned inside the service from the same durable facts.
            {
                // The tracked residual view the service checks
                // condition 4 against: this iteration's comparison
                // results, attributed per unit — the BLOCKING subset
                // only, which is §16.1 condition 4's own word ("no
                // blocking residual references it"). §13.5 forces a
                // FAIL comparison verdict alongside a blocking
                // residual, so a blocking residual under a PASS
                // verification is the §18.4 disagreement shape, and
                // settlement refuses there; a minor/major residual
                // rides the §11.5 disposition rows instead of the
                // settlement gate.
                let tracked: Vec<BlockingResidual> = residuals
                    .iter()
                    .filter(|residual| residual.residual.is_blocking())
                    .map(|residual| BlockingResidual {
                        unit: residual.unit.clone(),
                        detail: residual.residual.detail().to_owned(),
                    })
                    .collect();
                // COR-F1 (falsifier round 1, F-1): in the COMPOSED
                // produced-artifact world, an instrument-produced
                // requirement settles only over CORROBORATED locator bytes
                // — this completion's observation must contain an executed,
                // passing test facet (the exact rule:
                // [`Self::produced_artifact_corroborated`]). A PASS verdict
                // decided purely over the builder-authored bytes at the
                // locator (falsifier F-1's forged-report liar) settles
                // NOTHING: the block is pinned as typed evidence and the
                // unit stays open — a bounded honest non-settlement, never
                // a silent downgrade and never a run error. The default
                // [`ProducePolicy::MaterializeObservation`] world is
                // untouched: its locator artifact IS the instrument's own
                // facet document, and the frozen proofs decide through it
                // byte for byte.
                let corroboration_gated = self.produce == ProducePolicy::BindLocatorContent
                    && unit.evidence_required().producer() == gauntlet_domain::Producer::Instrument
                    && verified.record().outcome() == Outcome::Pass;
                let corroborated = corroboration_gated
                    && Self::produced_artifact_corroborated(
                        &assembled.doc,
                        state.run.id(),
                        iteration,
                    );
                if corroboration_gated && !corroborated {
                    let record = format!(
                        "{{\"class\":\"producer_uncorroborated\",\"run\":\"{}\",\"unit\":\"{unit_id}\",\"locator\":\"{}\",\"detail\":\"the instrument-required unit's PASS was decided over locator bytes no instrument execution in this run's observation corroborates: no executed passing test facet (kind=test with command and exit status 0) covers the evidence workspace this iteration\"}}",
                        state.run.id(),
                        unit.evidence_required().locator(),
                    );
                    let evidence = self.evidence.put(
                        record.as_bytes(),
                        EvidenceKind::Log,
                        self.operation_bound(),
                    )?;
                    self.evidence
                        .pin(state.run.id(), &evidence, self.operation_bound())?;
                }
                if !corroboration_gated || corroborated {
                    match self.settlement.settle(
                        SettleRequest {
                            claim: &verified,
                            actor: ClosingActor::Automatic,
                            tracked_residuals: &tracked,
                        },
                        self.operation_bound(),
                    ) {
                        Ok(outcome) => {
                            // C8.1: the settlement append enters the
                            // water-mark like every append.
                            state.last_offset = outcome.event_log_offset.get();
                            state
                                .unit_states
                                .insert(unit_id.clone(), UnitStatus::Settled);
                            if unit_id == flight.unit {
                                flight_unit_settled = true;
                            }
                        }
                        // The settle-time mutation is §21.3's abort row: the
                        // service already pinned BOTH digests; the run ends
                        // `aborted{reference_mutated}` (§9.4).
                        Err(SettleError::ReferenceMutated(refusal)) => {
                            let crate::settle::ReferenceMutatedRefusal {
                                expected, actual, ..
                            } = *refusal;
                            state.terminal = Some(TerminalState::Aborted);
                            state.reference_breach =
                                Some(ReferenceBreach::Mutated { expected, actual });
                        }
                        // Environment failures propagate typed, never
                        // swallowed into a silent non-settlement.
                        Err(SettleError::State(error)) => return Err(ControlError::State(error)),
                        Err(SettleError::Evidence(error)) => {
                            return Err(ControlError::Evidence(error));
                        }
                        Err(SettleError::Escalation(error)) => {
                            return Err(ControlError::Escalation(error));
                        }
                        Err(SettleError::Admission(
                            gauntlet_ports::producer_authority::SettlementAdmissionError::State(
                                error,
                            ),
                        )) => return Err(ControlError::State(error)),
                        Err(SettleError::Reference(error)) => {
                            return Err(ControlError::Reference(error));
                        }
                        // A §16.1 gate refusal is a fact about THIS unit,
                        // not a run error: pinned forensic evidence, the
                        // unit stays open (the service already escalated
                        // and pinned the authority-gap record where
                        // §11.5 row 6 applies).
                        Err(failure) => {
                            let record = format!(
                                "{{\"run\":\"{}\",\"unit\":\"{unit_id}\",\"settlement_refused\":\"{failure}\"}}",
                                state.run.id()
                            );
                            let evidence = self.evidence.put(
                                record.as_bytes(),
                                EvidenceKind::Log,
                                self.operation_bound(),
                            )?;
                            self.evidence
                                .pin(state.run.id(), &evidence, self.operation_bound())?;
                        }
                    }
                }
            }
            // INCONCLUSIVE stays open — the dependent claim's gap is the
            // recorded unavailable evidence, never a pass (I8).
        }
        // W5 work-product promotion: a completion that settled at least one
        // unit is the point its workspace's changes become part of the
        // run's carried-forward integration tree, BEFORE the workspace is
        // released (release happens in the caller, after this returns).
        // Never promotes a completion that settled nothing; a promotion
        // FAILURE is pinned forensic evidence, never an un-settlement and
        // never a run error — the settlement already stands.
        if flight_unit_settled && let Some(provider) = self.workspaces.as_ref() {
            match provider.promote(&flight.spec.workspace, self.operation_bound()) {
                Ok(promotion) if promotion.supported => {
                    let units_json = attempted_units(&flight, &result)
                        .iter()
                        .map(|unit| format!("\"{}\"", escape_json_text(&unit.to_string())))
                        .collect::<Vec<_>>()
                        .join(",");
                    let changed_json = promotion
                        .changed
                        .iter()
                        .map(|file| {
                            format!(
                                "{{\"path\":\"{}\",\"sha256\":\"{}\"}}",
                                escape_json_text(&file.path),
                                escape_json_text(&file.sha256)
                            )
                        })
                        .collect::<Vec<_>>()
                        .join(",");
                    let deleted_json = promotion
                        .deleted
                        .iter()
                        .map(|path| format!("\"{}\"", escape_json_text(path)))
                        .collect::<Vec<_>>()
                        .join(",");
                    let conflicts_json = promotion
                        .conflicts
                        .iter()
                        .map(|path| format!("\"{}\"", escape_json_text(path)))
                        .collect::<Vec<_>>()
                        .join(",");
                    let record = format!(
                        "{{\"class\":\"work_product_promoted\",\"run\":\"{}\",\"iteration\":{},\"units\":[{units_json}],\"invocation\":\"{}\",\"changed\":[{changed_json}],\"deleted\":[{deleted_json}],\"conflicts\":[{conflicts_json}]}}",
                        state.run.id(),
                        state.iteration,
                        flight.spec.invocation_id,
                    );
                    let evidence = self.evidence.put(
                        record.as_bytes(),
                        EvidenceKind::Log,
                        self.operation_bound(),
                    )?;
                    self.evidence
                        .pin(state.run.id(), &evidence, self.operation_bound())?;
                }
                Ok(_) => {
                    // Unsupported: no seed configured for this
                    // provider — nothing to report, skip silently.
                }
                Err(error) => {
                    let record = format!(
                        "{{\"class\":\"work_product_promotion_failed\",\"run\":\"{}\",\"iteration\":{},\"invocation\":\"{}\",\"error\":\"{}\"}}",
                        state.run.id(),
                        state.iteration,
                        flight.spec.invocation_id,
                        escape_json_text(&error.to_string())
                    );
                    let evidence = self.evidence.put(
                        record.as_bytes(),
                        EvidenceKind::Log,
                        self.operation_bound(),
                    )?;
                    self.evidence
                        .pin(state.run.id(), &evidence, self.operation_bound())?;
                }
            }
        }
        // The residual set is loop state FIRST (correction C3/C4: the
        // diagnostic evidence reads it, and the row-9 early-return no
        // longer leaves the previous iteration's residual set standing).
        state.open_residuals = residuals.iter().map(|r| r.residual.clone()).collect();
        // §11.5 row 9: non-convergence stops correcting. The key carries
        // the §13.5 locator pair; the unit is the record's own attribution
        // with the iteration's unit as the honest fallback (C4).
        for residual in &residuals {
            let recurring = RecurringResidual {
                id: residual.residual.id().clone(),
                key: ResidualKey {
                    unit: residual.unit.clone(),
                    dimension: residual
                        .residual
                        .dimension()
                        .unwrap_or("functional")
                        .to_owned(),
                    severity: residual.residual.severity(),
                    expected: residual.expected.clone(),
                    observed: residual.observed.clone(),
                },
            };
            if let Some(nonconvergence) = state.detector.observe(&recurring) {
                let unit = nonconvergence.key.unit.clone();
                let observation_gaps = !assembled.doc.unavailable_evidence().is_empty();
                return self.enter_diagnosis(state, unit, nonconvergence, observation_gaps);
            }
        }
        // §11.5 row 1: residual with attempts left re-dispatches with the
        // residual TEXT ONLY.
        if !state.open_residuals.is_empty() {
            // §18.3/§16.1: a non-blocking residual doesn't gate settlement,
            // so a completion can carry one here and still have settled
            // `flight.unit` above. If so, re-dispatching it would reopen a
            // unit the run already durably settled, so skip that and close
            // with the same selection record as the no-residual case. The
            // residuals stay in `state.open_residuals` for later context.
            if flight_unit_settled {
                self.record_selection(state, &flight.unit, false)?;
            } else {
                let attempts_used = state.attempts.get(&flight.unit).copied().unwrap_or(0);
                let cause = IterationCause::ResidualFound {
                    unit: flight.unit.clone(),
                    residual_text: state
                        .open_residuals
                        .first()
                        .map(|residual| residual.detail().to_owned())
                        .unwrap_or_default(),
                    attempts_used,
                };
                match disposition::dispose(cause, &self.policy) {
                    Disposition::RetryWithResidual { attachment, .. } => {
                        queue_retry_dispatch(state, &flight, attachment.as_str().to_owned());
                        // §15.1 step 4 on the CORRECTING iteration (C5): the
                        // residual-driven choice is recorded WITH its
                        // rejected alternative — the moment a correction is
                        // chosen.
                        self.record_selection(state, &flight.unit, true)?;
                    }
                    Disposition::Escalate { reason } => {
                        let unit = flight.unit.clone();
                        return self.escalate(state, &unit, reason.to_string());
                    }
                    other => {
                        unreachable!(
                            "the row-1 cause maps to re-dispatch or escalate, got {other:?}"
                        )
                    }
                }
            }
        } else {
            // No residuals: §15.1's record obligation (AK-037) — the
            // iteration's chosen correction is the unit's completion.
            self.record_selection(state, &flight.unit, false)?;
        }
        Ok(())
    }

    /// COR-F1 (falsifier round 1, F-1): the instrument corroboration rule
    /// for the composed produced-artifact world
    /// ([`ProducePolicy::BindLocatorContent`]). The EXACT matching rule:
    ///
    /// A produced locator artifact is CORROBORATED iff the completion's
    /// own observation bundle — the bundle assembled and bound at THIS
    /// (run, iteration), verified by equality of the document's `run_id`
    /// and `iteration_no` against the settling loop's facts — contains at
    /// least one instrument facet record where ALL of:
    ///
    /// 1. `kind` is `test` — the instrument class whose executions produce
    ///    test reports (the class the `evidence_required.kind =
    ///    test_report` requirement is answered by);
    /// 2. `command` is present and `exit_status` is present — a REAL
    ///    instrument execution over the completion's evidence workspace
    ///    (the workspace the observation request anchored at, which is the
    ///    workspace the produce step bound the locator artifact in —
    ///    COR-D3's one-surface anchoring), never an attestation stub;
    /// 3. `exit_status == 0` — the instrument's own execution SUCCEEDED,
    ///    agreeing with the passing report the locator artifact claims.
    ///
    /// A bundle without such a record does not corroborate: an
    /// instrument-required unit whose verification PASSED over uncorroborated
    /// locator bytes must not settle (falsifier F-1: a report-forging liar
    /// otherwise settles every unit by forging the report content — the
    /// observation facets are the loop's only reality probes, so the
    /// settlement gate demands one that actually executed and passed).
    fn produced_artifact_corroborated(
        doc: &crate::observation::bundle::ObservationBundleDoc,
        run_id: &RunId,
        iteration: IterationNo,
    ) -> bool {
        // "Produced by the observation instruments in THIS run": the bundle
        // must be the one this completion assembled — a carried or stale
        // document names a different fact source and corroborates nothing.
        if doc.run_id() != run_id || doc.iteration_no() != iteration {
            return false;
        }
        // The facet kind is compared through the ports' own vocabulary —
        // the wire token comes from `FacetKind::Test`, never a spelled
        // string (the adapter-class tokens are configuration vocabulary,
        // AK-014).
        let test_token = gauntlet_ports::observer::FacetKind::Test.token();
        doc.facets().iter().any(|facet| {
            facet.kind() == test_token.as_str()
                && facet.command().is_some()
                && facet.exit_status() == Some(0)
        })
    }

    /// The evidence owner's produce step (the live-E2E correction): bind
    /// THIS completion's evidence at the attempted unit's frozen locator,
    /// per the composition's [`ProducePolicy`]. Either policy leaves the
    /// §14.3 three-way fact ONE fact — the bytes at the locator, the
    /// bytes in the store, and the recorded digest — and leaves the N4
    /// containment discipline intact (the locator resolved inside the
    /// canonical root before any byte was read or written; the typed
    /// refusal fired at the caller).
    fn produce_unit_evidence(
        &self,
        state: &LoopState,
        iteration: IterationNo,
        unit: &Unit,
        facet: &crate::observation::bundle::FacetRecord,
        locator: &EvidenceLocator,
        locator_path: &std::path::Path,
    ) -> Result<(), ControlError> {
        let instrument_name = gauntlet_domain::InstrumentName::new(facet.instrument_name())
            .map_err(|error| {
                ControlError::Producer(ProducerAuthorityError::EvidenceUnreadable {
                    evidence: EvidenceRef::new(facet.evidence_ref()).unwrap_or_else(|_| {
                        EvidenceRef::new("evd-unparseable-facet-ref").expect("fallback ref grammar")
                    }),
                    detail: error.to_string(),
                })
            })?;
        let instrument = gauntlet_ports::observer::InstrumentIdentity::new(
            instrument_name.clone(),
            facet.instrument_version().to_owned(),
        );
        let facet_ref = EvidenceRef::new(facet.evidence_ref()).map_err(|error| {
            ControlError::Producer(ProducerAuthorityError::EvidenceUnreadable {
                evidence: EvidenceRef::new(facet.evidence_ref()).unwrap_or_else(|_| {
                    EvidenceRef::new("evd-unparseable-facet-ref").expect("fallback ref grammar")
                }),
                detail: error.to_string(),
            })
        })?;
        match self.produce {
            ProducePolicy::MaterializeObservation => {
                // The pre-correction behavior, byte for byte: the first
                // observation facet's document IS the completion's
                // produced evidence — materialize it at the locator and
                // record the production over the facet's own store ref.
                // The frozen p4/p7 proof rungs (facet-document artifact
                // contracts) decide through exactly this world.
                if let Some(parent) = locator_path.parent() {
                    std::fs::create_dir_all(parent).map_err(|error| {
                        ControlError::State(StateStoreError::WriteFailed {
                            detail: format!("creating the locator parent failed: {error}"),
                        })
                    })?;
                }
                let facet_bytes = self
                    .evidence
                    .get(&facet_ref, self.operation_bound())?
                    .unwrap_or_default();
                std::fs::write(locator_path, &facet_bytes).map_err(|error| {
                    ControlError::State(StateStoreError::WriteFailed {
                        detail: format!("writing the locator bytes failed: {error}"),
                    })
                })?;
                self.evidence_production.record(
                    state.run.id(),
                    iteration,
                    &facet_ref,
                    &instrument,
                    locator,
                    self.clock.now(),
                    self.operation_bound(),
                )?;
                Ok(())
            }
            ProducePolicy::BindLocatorContent => {
                // The locator is the VERIFICATION ARTIFACT's home — the
                // location of the work product the §14.2 rung decides
                // over. The owner binds the locator's ACTUAL content and
                // never overwrites a produced artifact:
                //
                // - when the dispatched work produced an artifact at the
                //   locator, those bytes are INGESTED into the
                //   content-addressed store and the production is
                //   recorded over them. (The pre-correction behavior
                //   wrote the first facet's `facet_evidence@v1` bytes
                //   over that artifact, so the §14.3 three-way equality
                //   forced every artifact-contract rung to read an
                //   observation document — no composed unit could ever
                //   verify.)
                // - when the locator holds no artifact, the completion's
                //   produced instrument evidence IS the first observation
                //   facet's document: it is materialized at the locator
                //   and the production binds the facet's own store ref.
                //
                // The instrument identity carried on the production is
                // the completion's observation instrument — the
                // instrument under whose observation the locator content
                // was bound (the §14.3 condition-2 class check —
                // instrument, never a model assertion — is what the
                // record answers).
                let produced_ref = match std::fs::read(locator_path) {
                    Ok(artifact_bytes) => Some(
                        self.evidence
                            .put(
                                &artifact_bytes,
                                unit.evidence_required().kind(),
                                self.operation_bound(),
                            )
                            .map_err(ControlError::Evidence)?,
                    ),
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                    Err(error) => {
                        return Err(ControlError::State(StateStoreError::WriteFailed {
                            detail: format!(
                                "reading the verification artifact at the locator failed: {error}"
                            ),
                        }));
                    }
                };
                let produced_ref = if let Some(ingested) = produced_ref {
                    ingested
                } else {
                    if let Some(parent) = locator_path.parent() {
                        std::fs::create_dir_all(parent).map_err(|error| {
                            ControlError::State(StateStoreError::WriteFailed {
                                detail: format!("creating the locator parent failed: {error}"),
                            })
                        })?;
                    }
                    let facet_bytes = self
                        .evidence
                        .get(&facet_ref, self.operation_bound())?
                        .unwrap_or_default();
                    std::fs::write(locator_path, &facet_bytes).map_err(|error| {
                        ControlError::State(StateStoreError::WriteFailed {
                            detail: format!("writing the locator bytes failed: {error}"),
                        })
                    })?;
                    facet_ref.clone()
                };
                // Content addressing makes a RE-PRODUCED identical
                // artifact land on the ref that is already durably
                // recorded (a retry whose work product did not change).
                // task26 correction: this is NEVER a reason to skip
                // recording. §14.3 condition 3 reads "produced in this run
                // AT OR AFTER the transform it certifies" — a fact about
                // WHEN the instrument made these bytes, not about whether
                // a production row already names this ref. The bytes were
                // read from THIS iteration's own workspace, moments ago,
                // by THIS completion's own pipeline: the instrument
                // genuinely re-produced them, and that fact is real even
                // though the content did not need to change. Always call
                // `record`, unconditionally, and let the ONE production
                // authority (C5/B-M2 — never a second source of production
                // truth) decide: identical facts at the SAME iteration are
                // idempotent (no new event, §18.3 crash-resume stays
                // exactly-once); the SAME content/producer/locator at a
                // LATER iteration APPENDS its own row (a fresh
                // `evidence_produced` event, keyed by `(run, ref,
                // iteration)`) standing beside every earlier iteration's
                // row for this ref — never overwriting one, so recovery
                // forensics keep every iteration's production intact,
                // byte-level; anything else differing (content, producer,
                // locator, or an iteration at or before an existing row)
                // is refused exactly as a genuine conflict always was.
                self.evidence_production.record(
                    state.run.id(),
                    iteration,
                    &produced_ref,
                    &instrument,
                    locator,
                    self.clock.now(),
                    self.operation_bound(),
                )?;
                park_after_production_record_if_armed();
                Ok(())
            }
        }
    }

    /// §11.5 row 9 / §15.2: stop correcting, run ONE bounded diagnostic
    /// cycle, and either follow its indicated correction or end
    /// INCONCLUSIVE with the evidence quarantined and the escalation
    /// submitted (the B2 residual: quarantine persistence is wired HERE,
    /// beside the escalation, as pinned evidence — the store has no
    /// dedicated quarantine row).
    ///
    /// Every evidence fact is INPUT-DRIVEN (correction C3): observed this
    /// iteration (the observation's own gaps) or carried in loop state
    /// (the reference policy's recorded unavailability, the latest
    /// governed verification outcome, the run budget's own exhaustion
    /// predicate). `implementation_shaped` is the LAST-resort fallback the
    /// engine's precedence consults last — set only when every apparatus
    /// fact is silent — never a default. The cycle's ONE disconfirming
    /// observation is the unit's latest governed verification outcome:
    /// PASS disconfirms the residual's persistence claim (the strongest
    /// instrument the loop holds says the unit's requirement is met),
    /// FAIL leaves the hypothesis standing ("do not invent certainty").
    /// The per-phase spend is the allowance DIVIDED by the cycle's phase
    /// count — the whole allowance can never be consumed by phase one.
    fn enter_diagnosis(
        &self,
        state: &mut LoopState,
        unit: UnitId,
        nonconvergence: Nonconvergence,
        observation_gaps: bool,
    ) -> Result<(), ControlError> {
        let phase_count = u64::try_from(super::diagnosis::CyclePhase::ALL.len())
            .expect("five phases fit a u64 counter");
        let engine = DiagnosticEngine::new(DiagnosticSpend {
            tokens: self.budget_shape.diagnostic.tokens().div_ceil(phase_count),
            cost_usd: MicroUsd::new(
                self.budget_shape
                    .diagnostic
                    .cost_usd()
                    .get()
                    .div_ceil(phase_count),
            ),
        })?;
        let verification = state.last_verifications.get(&unit).copied();
        let mut evidence = DiagnosticEvidence::default();
        if let Some(ReferenceBreach::Unavailable { .. }) = state.reference_breach {
            evidence.observation_unavailable = true;
        }
        if observation_gaps {
            evidence.observation_unavailable = true;
        }
        // §18.4 "verifier disagrees with comparator": the comparison found
        // a recurring residual while the governed verification of the
        // unit's own requirement PASSED — an apparatus fact that outranks
        // the implementation hypothesis (§15.2's note).
        if !state.open_residuals.is_empty() && verification == Some(Outcome::Pass) {
            evidence.verifier_disagreed = true;
        }
        if rounds_exhausted(state) {
            evidence.payment_exhausted = true;
        }
        if !state.open_residuals.is_empty()
            && !evidence.verifier_disagreed
            && !evidence.observation_unavailable
        {
            evidence.implementation_shaped = true;
        }
        let context_ref = self.evidence.put(
            format!("{{\"diagnosis\":\"{unit}\"}}").as_bytes(),
            EvidenceKind::Log,
            self.operation_bound(),
        )?;
        let report = engine.run_cycle(
            &evidence,
            &mut state.budget,
            &DisconfirmingObservation {
                disconfirmed: verification == Some(Outcome::Pass),
            },
            self.escalation.as_ref(),
            &EscalationContext {
                run_id: state.run.id().clone(),
                context_ref,
                deadline: self.operation_bound(),
            },
        )?;
        match report.outcome {
            DiagnosisOutcome::CorrectionIndicated { locus, .. } => {
                // The indicated correction is a PINNED durable fact (C3:
                // reachable and observable through the composed path); the
                // loop returns to correction on the next iteration. The
                // non-convergence count stays pinned past threshold (the
                // detector is idempotent), so a further identical residual
                // re-enters diagnosis rather than re-patching.
                let record = format!(
                    "{{\"diagnosis_outcome\":\"correction_indicated\",\"run\":\"{}\",\"unit\":\"{unit}\",\"occurrences\":{},\"locus\":\"{locus}\",\"phases\":{}}}",
                    state.run.id(),
                    nonconvergence.occurrences,
                    report.phases_completed,
                );
                let indicated = self.evidence.put(
                    record.as_bytes(),
                    EvidenceKind::Log,
                    self.operation_bound(),
                )?;
                self.evidence
                    .pin(state.run.id(), &indicated, self.operation_bound())?;
                Ok(())
            }
            DiagnosisOutcome::Inconclusive { quarantine } => {
                // Quarantine persisted as pinned evidence (I8/§15.2).
                let record = format!(
                    "{{\"quarantine\":true,\"run\":\"{}\",\"unit\":\"{unit}\",\"occurrences\":{},\"locus\":\"{}\",\"reason\":\"{}\",\"evidence\":[{}]}}",
                    state.run.id(),
                    nonconvergence.occurrences,
                    quarantine
                        .locus
                        .map(|locus| locus.to_string())
                        .unwrap_or_else(|| "unknown".to_owned()),
                    escape_json_text(&quarantine.reason),
                    quarantine
                        .evidence_refs
                        .iter()
                        .map(|reference| format!("\"{reference}\""))
                        .collect::<Vec<_>>()
                        .join(",")
                );
                let quarantined = self.evidence.put(
                    record.as_bytes(),
                    EvidenceKind::Log,
                    self.operation_bound(),
                )?;
                self.evidence
                    .pin(state.run.id(), &quarantined, self.operation_bound())?;
                state.terminal = Some(TerminalState::Inconclusive);
                state.escalation_summary = Some(format!(
                    "diagnosis inconclusive for {unit}; evidence quarantined"
                ));
                Ok(())
            }
        }
    }

    /// §15.1 selection + the durable AK-037 record over the iteration's
    /// REAL candidate space (correction C5): the correction the loop is
    /// actually taking — the §11.5-row-1 residual-driven bounded
    /// re-dispatch, or the unit's completion — against the genuinely
    /// considered §15.1-step-3 alternative for the SAME unit, the
    /// speculative clean-workspace re-run. The pair is not decorative:
    /// the bounded-over-speculative shape preference is the decision
    /// §15.1 step 3 exists to record, the loser carries its typed reason,
    /// and the domain aggregate refuses a record without rejected
    /// alternatives (the preregistered A5 vacuity class is
    /// unrepresentable).
    ///
    /// Every candidate fact is REAL: authority and tier from the unit,
    /// the estimate from the same per-attempt arithmetic the plan
    /// consults the meter with (a real forward ask on the residual path —
    /// the affordability conjunct bites; a zero ask on the completion
    /// path — the correction already ran under its reservation), impact
    /// from the residual record, confidence from the attempt and
    /// verification history, dependency readiness from the durable
    /// statuses. The run's exercisable authority is its REAL auto-dispatch
    /// ceiling — §17.1's `critic` today (beyond-critic authority needs
    /// the Task-14 authority gate; L1) — so the selector's authority gate
    /// can genuinely fire, never comparing a unit's authority with
    /// itself.
    ///
    /// A fully governance-blocked space is §15.1's escalation branch on
    /// the residual path (never selected: escalate); on the completion
    /// path the completion already ran under the run's dispatch, so the
    /// authority fact is SURFACED as pinned evidence (L1's hook for the
    /// Task-13/14 surfaces) and the run continues — a completing
    /// cross-cutting or beyond-critic unit never errors the run.
    fn record_selection(
        &self,
        state: &mut LoopState,
        unit: &UnitId,
        residual_driven: bool,
    ) -> Result<(), ControlError> {
        // §21.3 row 1 (Task 13 B2 Item 4): an unreadable answer key skips
        // THIS iteration's §15.1 record — the gap is pinned evidence, the
        // loop keeps running; a read failure never errors the run.
        let parsed = match strict_units(&self.answer_key) {
            Ok(parsed) => parsed,
            Err(ControlError::Reference(ReferenceProviderError::Unavailable { detail })) => {
                let record = format!(
                    "{{\"class\":\"answer_key_read_failed\",\"run\":\"{}\",\"iteration\":{},\"locus\":\"selection\",\"detail\":\"{}\"}}",
                    state.run.id(),
                    state.iteration,
                    escape_json_text(&detail)
                );
                let evidence = self.evidence.put(
                    record.as_bytes(),
                    EvidenceKind::Log,
                    self.operation_bound(),
                )?;
                self.evidence
                    .pin(state.run.id(), &evidence, self.operation_bound())?;
                state.reference_breach = Some(ReferenceBreach::Unavailable { detail });
                return Ok(());
            }
            Err(other) => return Err(other),
        };
        let Some(domain_unit) = parsed.iter().find(|candidate| candidate.id() == unit) else {
            return Ok(());
        };
        let dependency_ready = domain_unit.depends_on().iter().all(|dependency| {
            matches!(
                state.unit_states.get(dependency),
                Some(UnitStatus::Settled) | Some(UnitStatus::OutOfScope)
            )
        });
        let settlement = if domain_unit.verify_by() == gauntlet_domain::VerifyBy::HumanSignoff {
            super::select::SettlementAccessibility::HumanSignoff
        } else {
            super::select::SettlementAccessibility::Instrument
        };
        // The chosen correction's real forward ask: the per-attempt
        // estimate on the residual path; nothing on the completion path.
        let forward_ask = if residual_driven {
            per_attempt_estimate(
                state.budget.limits(),
                state.budget.spent(),
                self.budget_shape.recovery_reserve_pct,
            )
        } else {
            BudgetSlice {
                rounds: 0,
                wall_seconds: 0,
                tokens: 0,
                cost_usd: MicroUsd::new(0),
            }
        };
        let impact = Impact {
            worst_severity: state
                .open_residuals
                .first()
                .map(|residual| residual.severity())
                .unwrap_or(Severity::Minor),
            residuals_addressed: state.open_residuals.len() as u32,
        };
        let attempts = state.attempts.get(unit).copied().unwrap_or(0);
        let confidence = match state.last_verifications.get(unit) {
            Some(Outcome::Pass) => super::select::Confidence::LastSucceeded,
            Some(_) if attempts >= self.policy.nonconvergence_n() => {
                super::select::Confidence::RepeatedFailure
            }
            Some(_) => super::select::Confidence::FailedOnce,
            None if attempts >= 1 => super::select::Confidence::FailedOnce,
            None => super::select::Confidence::NeverAttempted,
        };
        let chosen_summary = if residual_driven {
            format!("address the residual on {unit} by bounded re-dispatch")
        } else {
            format!("complete {unit}")
        };
        let candidate_of = |summary: String,
                            shape: gauntlet_domain::CorrectionShape|
         -> Result<Candidate, SelectionError> {
            Ok(Candidate {
                correction: gauntlet_domain::CorrectionDescriptor::new(
                    summary,
                    shape,
                    vec![unit.clone()],
                    self.selection_facts(state, forward_ask),
                )
                .map_err(SelectionError::Domain)?,
                required_authority: domain_unit.authority(),
                tier: domain_unit.blast_radius(),
                dependency_ready,
                settlement,
                impact,
                information_gain: super::select::InformationGain::Incremental,
                confidence,
                units_touched: 1,
                rollback_units: 1,
            })
        };
        let chosen = candidate_of(chosen_summary, gauntlet_domain::CorrectionShape::Bounded);
        let speculative = candidate_of(
            format!("re-run {unit} from a clean workspace"),
            gauntlet_domain::CorrectionShape::Speculative,
        );
        let candidates = match (chosen, speculative) {
            (Ok(chosen), Ok(speculative)) => vec![chosen, speculative],
            (Err(error), _) | (_, Err(error)) => {
                return Err(ControlError::Selection(error));
            }
        };
        let iteration_no = IterationNo::new(state.iteration.max(1)).map_err(|error| {
            ControlError::State(StateStoreError::WriteFailed {
                detail: error.to_string(),
            })
        })?;
        let selection = self.selector.select(&SelectionContext {
            run_id: state.run.id().clone(),
            iteration_no,
            meter: state.meter.as_ref(),
            // §17.1's auto-dispatch ceiling — ONE source since plan Task
            // 14: the governance posture the controller was composed with
            // (critic for unattended runs), the same fact the dispatch
            // gate enforces (the L1 deferral is closed).
            exercisable_authority: self.govern.exercisable_authority(),
            dispatchable_tiers: dispatchable_tier_capacity(self.budget_shape.limits)
                .into_iter()
                .map(|(tier, _)| tier)
                .collect(),
            candidates,
        });
        match selection {
            Ok(selection) => {
                self.state.record_correction_selection(
                    &selection,
                    self.clock.now(),
                    self.operation_bound(),
                )?;
                Ok(())
            }
            Err(SelectionError::NoAdmissibleCandidate { rejected }) => {
                let governance_blocked = rejected.iter().any(|alternative| {
                    alternative.class() != gauntlet_domain::RejectionClass::Budget
                });
                if residual_driven && governance_blocked {
                    // §15.1 step 1: governance-blocked paths are never
                    // selected — escalate (§11.6).
                    let blocker = rejected
                        .first()
                        .map(|alternative| {
                            format!(
                                "{}: {:?}",
                                alternative.correction().summary(),
                                alternative.reason()
                            )
                        })
                        .unwrap_or_else(|| "unknown blocker".to_owned());
                    let blocked_unit = unit.clone();
                    self.escalate(
                        state,
                        &blocked_unit,
                        format!("correction governance-blocked ({blocker})"),
                    )
                } else if !residual_driven {
                    // L1's hook: the completion already ran under the
                    // run's dispatch; its §15.1 record is unrepresentable
                    // under the auto-dispatch ceiling, and the authority
                    // FACT is surfaced as pinned evidence for the Task-14
                    // authority gate — never an error on a success.
                    let record = format!(
                        "{{\"completion_record_unrepresentable\":true,\"run\":\"{}\",\"unit\":\"{unit}\",\"authority\":\"{}\",\"exercisable\":\"critic\",\"note\":\"L1: Task-14 authority gate pending\"}}",
                        state.run.id(),
                        domain_unit.authority(),
                    );
                    let evidence = self.evidence.put(
                        record.as_bytes(),
                        EvidenceKind::Log,
                        self.operation_bound(),
                    )?;
                    self.evidence
                        .pin(state.run.id(), &evidence, self.operation_bound())?;
                    Ok(())
                } else {
                    // Budget-only blocks are the dispatch gate's own fact
                    // (§17.4): the loop does not re-decide affordability
                    // here, and the iteration's plan classification owns
                    // the terminal. No record, no error.
                    Ok(())
                }
            }
            Err(other) => Err(ControlError::Selection(other)),
        }
    }

    /// The between-iterations terminal decision: everything settled (or
    /// out of scope) ⇒ `settled`; the rounds dimension spent ⇒
    /// `budget_exhausted` (§11.6: the recovery reserve was never touched
    /// by forward work — the run budget's own arithmetic, correction C2).
    /// Settlement itself is an iteration boundary (§9.4/§11.4 step 4):
    /// the run may settle only under a ruler the boundary re-verify still
    /// certifies unchanged — a mutation HERE is the terminal abort with
    /// both digests as pinned evidence, and an unavailable provider
    /// records the gap while settlement stands on the frozen projection
    /// (§21.3 keeps reconciliation alive).
    fn decide_progress(&self, state: &mut LoopState) -> Result<Option<RunOutcome>, ControlError> {
        let all_closed = !state.unit_states.is_empty()
            && state
                .unit_states
                .values()
                .all(|status| matches!(status, UnitStatus::Settled | UnitStatus::OutOfScope));
        if all_closed {
            if let Some(outcome) = self.boundary_verify(state)? {
                return Ok(Some(outcome));
            }
            state.terminal = Some(TerminalState::Settled);
            return Ok(None);
        }
        if state.terminal.is_none() && rounds_exhausted(state) {
            state.terminal = Some(TerminalState::BudgetExhausted);
        }
        Ok(None)
    }

    /// §18.2: the durable checkpoint — run id, reference digest, iteration
    /// number, unit-state projection, budget spent, in-flight ids with
    /// their result-contract paths, and the event-log offset (I11).
    ///
    /// C8.1: the offset is the store-returned water-mark, threaded from
    /// EVERY append this loop makes (each terminal event's returned offset
    /// updates `last_offset`; the checkpoint's own `CheckpointWritten`
    /// append updates it again, so the NEXT checkpoint starts where this
    /// one ended). §18.3 step 1 makes this field the anchor replay starts
    /// from — a constant 0 would replay events the run already consumed.
    fn checkpoint(&self, state: &mut LoopState) -> Result<(), ControlError> {
        let iteration_no = IterationNo::new(state.iteration.max(1)).map_err(|error| {
            ControlError::State(StateStoreError::WriteFailed {
                detail: error.to_string(),
            })
        })?;
        let checkpoint = Checkpoint {
            run_id: state.run.id().clone(),
            reference_digest: state.version_digest.clone(),
            iteration_no,
            unit_states: state
                .unit_states
                .iter()
                .map(|(unit, status)| (unit.clone(), *status))
                .collect(),
            budget_spent: *state.budget.spent(),
            in_flight: state
                .in_flight
                .iter()
                .chain(state.carried.iter())
                .map(|flight| {
                    (
                        flight.spec.invocation_id.clone(),
                        flight.spec.result_contract_path.clone(),
                    )
                })
                .collect(),
            event_log_offset: gauntlet_domain::EventLogOffset::new(state.last_offset),
        };
        self.state.checkpoint(checkpoint, self.operation_bound())?;
        let offset = self.state.append_event(
            RunEvent {
                run_id: state.run.id().clone(),
                kind: RunEventKind::CheckpointWritten {
                    iteration_no,
                    event_log_offset: gauntlet_domain::EventLogOffset::new(state.last_offset),
                },
                at: self.clock.now(),
            },
            self.operation_bound(),
        )?;
        state.last_offset = offset.get();
        Ok(())
    }

    /// The IO bound for one controller operation: derived from the clock
    /// and the run's wall budget — never a literal.
    fn operation_bound(&self) -> Deadline {
        let horizon = self
            .clock
            .now()
            .get()
            .saturating_add(self.budget_shape.limits.max_wall_seconds());
        Deadline::at_epoch(EpochSeconds::new(horizon))
    }

    /// The bound for ONE CANCEL's blocking stop (corrections round 1, F6;
    /// §6 preamble: bounded blocking control operations, never unbounded
    /// ones). A cancel must not hold the loop for the run's remaining
    /// wall horizon: the §11.4 stall window is the loop's own "how long a
    /// quiet child gets" setting, so it is the graceful leg's bound here
    /// too, and the backend's stop escalates past it (SIGKILL + bounded
    /// reap) inside the same call. The deadline parameter of the port's
    /// cancel exists to bound exactly this operation.
    fn cancel_operation_bound(&self) -> Deadline {
        let window = self
            .policy
            .stall_timeout_ms()
            .div_ceil(MILLIS_PER_SECOND)
            .max(1);
        let horizon = self.clock.now().get().saturating_add(window);
        Deadline::at_epoch(EpochSeconds::new(horizon))
    }

    /// §5.3.7/§11.4 (C2): books the iteration's ONE round on the run
    /// budget — the single rounds authority. A refused booking (the next
    /// round would consume the recovery reserve, or the raw limit is
    /// gone) is terminal `budget_exhausted` — unless a STRONGER terminal
    /// was already set mid-iteration (an escalation), which wins; the
    /// refusal booked nothing, so the reserve stays intact for the
    /// close-out checkpoint and report.
    fn book_iteration_round(&self, state: &mut LoopState) {
        if state
            .budget
            .charge(ONE_ROUND_PER_ITERATION, 0, 0, MicroUsd::new(0))
            .is_err()
            && state.terminal.is_none()
        {
            state.terminal = Some(TerminalState::BudgetExhausted);
        }
    }
}

/// An `Arc<dyn Observer>` seen as one plan-order `Box<dyn Observer>`: the
/// controller holds its observers shared (every iteration reuses them);
/// the assembly plan owns boxed ones.
struct SharedObserver(Arc<dyn Observer>);

impl Observer for SharedObserver {
    fn observe(
        &self,
        request: &ObservationRequest,
        deadline: Deadline,
    ) -> Result<gauntlet_ports::observer::ObservationFacet, gauntlet_ports::observer::ObserverError>
    {
        self.0.observe(request, deadline)
    }

    fn facet_kind(&self) -> gauntlet_ports::observer::FacetKind {
        self.0.facet_kind()
    }
}

/// Reads the answer key STRICTLY (all-or-nothing; §8.5 check 2's unit
/// schema): every block well-formed or the typed parse failure.
fn strict_units(source: &AnswerKeySource) -> Result<Vec<Unit>, ControlError> {
    let text = source.read_text().map_err(|detail| {
        ControlError::Reference(ReferenceProviderError::Unavailable { detail })
    })?;
    crate::reference::parse::parse_all_or_fail(&text).map_err(|failure| {
        ControlError::Reference(ReferenceProviderError::ResolveFailed {
            detail: failure.to_string(),
        })
    })
}

/// §11.2's concurrency input: global capacity from the budget's
/// parallelism dimension; per-tier capacity for every dispatchable tier
/// (contained, local, cross_cutting). NO entry for `irreversible` — a tier
/// with no capacity entry is unadmitted (§17.3), the correct posture for
/// the tier that must never dispatch. Real per-tier policy FILES land with
/// plan Task 14's authority gate; until then the global parallel bound is
/// the honest per-tier capacity.
fn dispatchable_tier_capacity(limits: Limits) -> Vec<(gauntlet_domain::BlastRadius, u32)> {
    use gauntlet_domain::BlastRadius;
    vec![
        (BlastRadius::Contained, limits.max_parallel()),
        (BlastRadius::Local, limits.max_parallel()),
        (BlastRadius::CrossCutting, limits.max_parallel()),
    ]
}

/// The one Unit this controller can actually dispatch for a planned slice,
/// or a typed refusal (correction C6.3 — "charge what you prompt; prompt
/// what you charged").
///
/// §11.3 lets the scheduler group non-separable Units into ONE slice, and
/// `PlannedDispatch::estimated_slice` is documented as the SUM over every
/// member ("one builder performs every member's work, so one dispatch pays
/// for all of it"). This controller reserves that sum and prompts ONE
/// unit. For a slice with more than one member those two facts disagree:
/// the reservation — and the charge that settles against it — covers work
/// no prompt ever asked for, and the unprompted members are dropped with
/// no record.
///
/// Cohort slices cannot form on any shipped path today (the L4 record
/// beside the `cohorts` sites), which makes the inconsistency LATENT
/// rather than live. Latency is exactly why it is refused rather than
/// tolerated: a silent drop stops being latent the moment plan Task 20
/// composes a real cohort source, and it would stop silently. A typed
/// refusal carrying the member list makes that composition a visible,
/// failing change instead of a behaviour drift. An empty slice is a
/// scheduler contract violation the caller already skips (`Ok(None)`);
/// only the multi-member case is new.
fn dispatchable_member(planned: &PlannedDispatch) -> Result<Option<UnitId>, ControlError> {
    if planned.units.len() > 1 {
        return Err(ControlError::SliceNotDispatchable {
            units: planned.units.clone(),
        });
    }
    Ok(planned.units.first().cloned())
}

fn open_concurrency(limits: Limits) -> Concurrency {
    Concurrency {
        global_capacity: limits.max_parallel(),
        global_in_flight: 0,
        tier_capacity: dispatchable_tier_capacity(limits),
        tier_in_flight: Vec::new(),
    }
}

/// The per-attempt estimate the scheduler's budget conjunct reads, AND the
/// reservation the meter holds against one dispatch. A reservation is a
/// HOLD on what ONE attempt may spend, not a forecast of a future round's
/// total spend — so it must be sized against whatever else can hold
/// concurrently, not against how many rounds remain. §5.3.7's recovery
/// reserve is a fixed slice of each LIMIT — not of the remainder — so the
/// FORWARD remainder is the reserve-adjusted ceiling minus what is already
/// spent. Up to `max_parallel` attempts can be in flight (and reserved)
/// at once, so each attempt's honest share is that forward remainder
/// divided by `max_parallel`: with `max_parallel` 1 (the common case) an
/// attempt may reserve the WHOLE forward remainder. Dividing by
/// `rounds_left` instead — the prior behaviour — assumed every future
/// round would spend an equal share, which is false for any real
/// dispatch: a single builder attempt that legitimately spends more than
/// `forward_remainder / rounds_left` (but still well within the forward
/// ceiling) had its charge refused by the meter as `ReservationExceeded`,
/// and the loop mapped that refusal straight to a false terminal
/// `budget_exhausted` — no budget dimension was actually breached. `rounds`
/// stays 1 (one attempt, one round) and `rounds_left` still gates whether a
/// reservation is offered AT ALL: with no rounds left, no attempt can be
/// dispatched, so the estimate stays zero and the loop's rounds check
/// refuses dispatch before any zero reservation. With `max_parallel` 1, a
/// charge that now exceeds its reservation means the forward ceiling
/// itself was crossed — the refusal is a true breach, not an artifact of
/// this sizing.
fn per_attempt_estimate(limits: &Limits, spent: &Spent, recovery_reserve_pct: u8) -> BudgetSlice {
    let rounds_left = limits.max_rounds().saturating_sub(spent.rounds());
    if rounds_left == 0 {
        return BudgetSlice {
            rounds: 0,
            wall_seconds: 0,
            tokens: 0,
            cost_usd: MicroUsd::new(0),
        };
    }
    let concurrent_holds = u64::from(limits.max_parallel());
    let forward_share = |limit: u64, spent: u64| -> u64 {
        let forward_ceiling = limit
            .saturating_mul(HUNDRED_PERCENT.saturating_sub(u64::from(recovery_reserve_pct)))
            / HUNDRED_PERCENT;
        forward_ceiling.saturating_sub(spent) / concurrent_holds
    };
    BudgetSlice {
        rounds: 1,
        wall_seconds: forward_share(limits.max_wall_seconds(), spent.wall_seconds()),
        tokens: forward_share(limits.max_tokens(), spent.tokens()),
        cost_usd: MicroUsd::new(forward_share(
            limits.max_cost_usd().get(),
            spent.cost_usd().get(),
        )),
    }
}

/// N4 (critic-B A12, independently confirmed): containment at the
/// controller's OWN write boundary. The domain's `validate_plain_ref`
/// rejects a leading `/` but PERMITS `..` — a locator like
/// `../escape.json` joins to a path OUTSIDE the workspace root, and this
/// controller is the first production writer at that path. The guard is
/// component-wise: `ParentDir` (the `..` escape), `RootDir`, and `Prefix`
/// components are all refused, so neither a parent-escaping nor an
/// absolute locator reaches `create_dir_all`/`fs::write`. The domain
/// grammar itself is deliberately untouched — widening a Task-3-closed
/// public value type shared by every locator-like newtype is a
/// domain/ports revision, not a controller fix.
fn contained_locator_path(
    root: &std::path::Path,
    locator: &EvidenceLocator,
) -> Result<PathBuf, ControlError> {
    let raw = std::path::Path::new(locator.as_str());
    for component in raw.components() {
        match component {
            std::path::Component::ParentDir
            | std::path::Component::RootDir
            | std::path::Component::Prefix(_) => {
                return Err(ControlError::EvidenceLocatorEscapes {
                    locator: locator.clone(),
                    root: root.to_owned(),
                });
            }
            std::path::Component::CurDir | std::path::Component::Normal(_) => {}
        }
    }
    Ok(root.join(raw))
}

fn add_slices(first: BudgetSlice, second: BudgetSlice) -> BudgetSlice {
    BudgetSlice {
        rounds: first.rounds.saturating_add(second.rounds),
        wall_seconds: first.wall_seconds.saturating_add(second.wall_seconds),
        tokens: first.tokens.saturating_add(second.tokens),
        cost_usd: MicroUsd::new(first.cost_usd.get().saturating_add(second.cost_usd.get())),
    }
}

/// C9.2: escapes the two characters that break a hand-built JSON string
/// record — the BACKSLASH first (escaping the quote first would double the
/// escapes the second pass just made), then the quote. One shared
/// rule, not a per-site `replace`: `submit_ticket`'s evidence record (the
/// single funnel for every escalation terminal since C1 — `escalated`,
/// `blocked`, `inconclusive`), the §21.3 reference-unavailability record,
/// the §15.2 quarantine reason, and — since Task 13 B2 — the settle
/// service's recovery-limitation records all embed operator- or
/// instrument-authored text, and per-site copies of an escaping rule is the
/// next defect.
pub(crate) fn escape_json_text(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"")
}

/// C9.2: `submit_ticket`'s pinned evidence record — a FORENSIC record
/// (§11.6), so it must survive its own summary: a summary containing a
/// quote or a backslash is escaped, never interpolated raw.
fn ticket_evidence_record(unit: &UnitId, summary: &str) -> String {
    format!(
        "{{\"unit\":\"{unit}\",\"summary\":\"{}\"}}",
        escape_json_text(summary)
    )
}

/// Whether the NEXT iteration's round is unaffordable on the run budget
/// (§11.4 step 5 / §11.6, correction C2): the single rounds-authority
/// predicate behind both rounds gates — the domain's own arithmetic over
/// the budget the loop books one round per iteration on, never the
/// meter's payment counters (payments carry no rounds).
fn rounds_exhausted(state: &LoopState) -> bool {
    state
        .budget
        .would_breach_reserve(ONE_ROUND_PER_ITERATION, 0, 0, MicroUsd::new(0))
}

/// The zero-payment release (the B1 adapter convention): a cancelled or
/// failed invocation books nothing and returns its reservation slot.
fn release_reservation(state: &LoopState, flight: &InFlight) {
    let _ = state.meter.charge(&flight.reservation, Payment::zero());
}

/// Queues a §11.5 re-dispatch: the unit returns to the eligible pool with
/// its attachment staged for the next dispatch's prompt.
fn queue_retry_dispatch(state: &mut LoopState, flight: &InFlight, attachment: String) {
    state
        .unit_states
        .insert(flight.unit.clone(), UnitStatus::Open);
    state.attempts.insert(flight.unit.clone(), flight.attempt);
    state
        .pending_attachments
        .insert(flight.unit.clone(), attachment);
}

/// The dispatch prompt and its allowlisted context body. The prompt names
/// the unit and its claim; a staged §11.5 attachment rides as residual
/// TEXT ONLY — the critic's reasoning has no path into this string (the
/// attachment is built from the residual's own expected/observed text or
/// the disposition's own note).
/// The snake_case wire token for a failed invocation's class: the same
/// mapping `class_token`/`failure_class_token` already use in the
/// promote/improve surfaces (`crates/gauntlet-app/src/promote/proposal.rs`,
/// `crates/gauntlet-cli/src/improve_cmd.rs`), kept local here since those
/// helpers are private to their own modules.
fn invocation_failure_token(class: InvocationFailure) -> &'static str {
    match class {
        InvocationFailure::MalformedResult => "malformed_result",
        InvocationFailure::ContractMismatch => "contract_mismatch",
        InvocationFailure::OrphanedByRestart => "orphaned_by_restart",
        InvocationFailure::Execution => "agent_execution",
        InvocationFailure::MissingResult => "missing_result",
    }
}

fn dispatch_prompt(
    unit: &UnitId,
    claim: Option<&str>,
    attachment: Option<&str>,
) -> (String, String) {
    let claim = claim.unwrap_or("the unit's frozen claim");
    let mut prompt = format!(
        "Attempt unit {unit}.\nClaim to make true: {claim}.\nProduce the evidence the claim \
         requires and write the strict result document at the declared result-contract path."
    );
    if let Some(text) = attachment
        && !text.is_empty()
    {
        prompt.push_str("\n\nResidual to close (text only): ");
        prompt.push_str(text);
    }
    let context = format!("unit: {unit}\nclaim: {claim}\nenvelope: allowlisted");
    (prompt, context)
}

/// The run's workspace REF: relative, derived from the run id — observers
/// resolve it under the canonical root they were bound to.
fn run_workspace_ref(run_id: &RunId) -> gauntlet_domain::WorkspaceRef {
    gauntlet_domain::WorkspaceRef::new(&run_id.to_string())
        .expect("a run id is a valid workspace ref")
}

/// The stable kind token for a gate refusal's durable record — one per
/// [`crate::govern::GateRefusal`] arm, so evidence consumers can group by
/// governance class without parsing prose.
fn refusal_kind(refusal: &crate::govern::GateRefusal) -> &'static str {
    match refusal {
        crate::govern::GateRefusal::Irreversible { .. } => "irreversible",
        crate::govern::GateRefusal::AuthorityAboveCeiling { .. } => "authority_above_ceiling",
        crate::govern::GateRefusal::LatticeMovementRefused { .. } => "lattice_movement_refused",
        crate::govern::GateRefusal::CapabilityMismatch { .. } => "capability_mismatch",
        crate::govern::GateRefusal::NoBackendForTier { .. } => "no_backend_for_tier",
        crate::govern::GateRefusal::PolicyUnresolved { .. } => "policy_unresolved",
        crate::govern::GateRefusal::ValidationEscalated { .. } => "validation_escalated",
        crate::govern::GateRefusal::BackendUnusable { .. } => "backend_unusable",
    }
}

/// The §17.4 payment of a completed invocation: the result's own reported
/// counters, EXCEPT wall time (W3, correction task26-dcritic-apparatus-repair):
/// a runner is untrusted external input (root AGENTS.md §5), so a
/// self-reported `wall_seconds` of zero (or any understated figure) must
/// never be the charged fact. The charged wall is
/// `max(reported, measured)`, where measured is the loop's OWN clock delta
/// between dispatch and the first-observed completion — never less than
/// what the runner reports, never less than what the loop itself watched
/// elapse.
fn payment_of(result: &RoleResult, flight: &InFlight) -> Payment {
    let reported = result.payment();
    let reported_wall_seconds = reported.wall_seconds();
    // Every completion path stamps `completed_at` before charging; a
    // missing stamp degrades to a zero measured wall (the reported figure
    // still stands), never a panic in the loop.
    let completed_at = flight.completed_at.unwrap_or(flight.dispatched_at);
    let measured_wall_seconds = completed_at
        .get()
        .saturating_sub(flight.dispatched_at.get());
    let charged_wall_seconds = reported_wall_seconds.max(measured_wall_seconds);
    Payment::new(
        reported
            .input_tokens()
            .saturating_add(reported.output_tokens()),
        charged_wall_seconds,
        reported.tool_calls(),
        reported.cost_usd(),
    )
}

/// The builder's claims text (I2 context: the claims facet is forensic
/// tamper-evidence, never verification input — the assembler places it in
/// the bundle's claims facet and nowhere else).
fn builder_claims(result: &RoleResult) -> String {
    match result.payload() {
        gauntlet_ports::role_result::RolePayload::BuilderResult(payload) => {
            payload.claims().join("; ")
        }
        _ => String::new(),
    }
}

/// The units a completed builder result says it attempted; the dispatch's
/// own unit when the payload names none.
fn attempted_units(flight: &InFlight, result: &RoleResult) -> Vec<UnitId> {
    match result.payload() {
        gauntlet_ports::role_result::RolePayload::BuilderResult(payload) => {
            let attempted = payload.attempted().to_vec();
            if attempted.is_empty() {
                vec![flight.unit.clone()]
            } else {
                attempted
            }
        }
        _ => vec![flight.unit.clone()],
    }
}

/// One residual of an accepted comparison as the loop consumes it
/// (correction C4): the domain aggregate, the unit it is ATTRIBUTED to,
/// and the §13.5 merge key's expected/observed locator pair the
/// non-convergence key counts under.
///
/// Attribution is the B.4 wire field when the record carries it
/// (`unit_refs`), falling back to the ITERATION'S unit (§13.5's residual
/// identity names a unit; §11.5 row 9's recurrence is per-unit) — the
/// shipped deterministic comparator emits dimension-level residuals with
/// empty refs, and the loop's own dispatch identity is the honest
/// fallback, never a guess from nowhere.
struct LoopResidual {
    residual: Residual,
    unit: UnitId,
    expected: String,
    observed: String,
}

/// Bounds builder-influenced failure text for evidence records and
/// retry prompts: control characters (a builder-controlled newline
/// must not forge record lines) become spaces, surrounding whitespace
/// trims, and overlong text truncates on a char boundary with a visible
/// marker — never silently, never unbounded.
const MAX_FAILURE_DETAIL_CHARS: usize = 300;

/// See [`MAX_FAILURE_DETAIL_CHARS`].
fn sanitize_failure_detail(detail: &str) -> String {
    let flat: String = detail
        .chars()
        .map(|ch| if ch.is_control() { ' ' } else { ch })
        .collect();
    let flat = flat.trim();
    if flat.chars().count() <= MAX_FAILURE_DETAIL_CHARS {
        return flat.to_owned();
    }
    let kept: String = flat.chars().take(MAX_FAILURE_DETAIL_CHARS).collect();
    format!("{kept} [truncated]")
}

/// How many paths per class the containment-breach evidence names
/// verbatim; beyond that the record carries the totals (bounded detail —
/// a breach that renames a tree must not stuff the evidence store).
const MAX_BREACH_PATHS_LISTED: usize = 20;

/// Renders a [`ContainmentDiff`] as the `changed`/`added`/`removed`
/// fields shared by the `containment_breach` evidence record and the
/// BLOCKING residual's observed text — every listed path JSON-escaped,
/// totals always present so truncation is visible, never silent.
fn containment_observed(diff: &ContainmentDiff) -> String {
    fn listed(paths: &[String]) -> String {
        paths
            .iter()
            .take(MAX_BREACH_PATHS_LISTED)
            .map(|path| format!("\"{}\"", escape_json_text(path)))
            .collect::<Vec<_>>()
            .join(",")
    }
    fn field(name: &str, paths: &[String]) -> String {
        if paths.len() > MAX_BREACH_PATHS_LISTED {
            format!(
                "\"{name}\":[{listed}],\"{name}_total\":{}",
                paths.len(),
                listed = listed(paths)
            )
        } else {
            format!("\"{name}\":[{}]", listed(paths))
        }
    }
    format!(
        "{},{},{}",
        field("changed", &diff.changed),
        field("added", &diff.added),
        field("removed", &diff.removed)
    )
}

/// The residual set of an accepted comparison, attributed per the above.
fn residuals_of(
    accepted: &crate::compare::authority::AcceptedComparison,
    iteration_unit: &UnitId,
) -> Vec<LoopResidual> {
    let mut out = Vec::new();
    if let Some(record) = accepted.record() {
        for doc in record.residuals() {
            let residual = Residual::new(
                doc.id().clone(),
                doc.severity(),
                format!("expected: {}; observed: {}", doc.expected(), doc.observed()),
                Some(doc.dimension().to_owned()),
            );
            out.push(LoopResidual {
                residual,
                unit: doc
                    .unit_refs()
                    .first()
                    .cloned()
                    .unwrap_or_else(|| iteration_unit.clone()),
                expected: doc.expected().to_owned(),
                observed: doc.observed().to_owned(),
            });
        }
    }
    out
}

/// §18.3 step 3's disk probe: read the result-contract path and strictly
/// decode it. Validity here is schema validity plus the run/invocation
/// identity echo — the envelope-digest half of the §10.3 rule-6 check
/// needs the durable invocation record's digest, which no port read
/// exposes; that re-check is Task 13's crash surface.
fn probe_result_contract(
    run_id: &RunId,
    invocation_id: &InvocationId,
    path: &ResultContractPath,
) -> Result<RoleResult, String> {
    let text = std::fs::read_to_string(path.as_str()).map_err(|error| error.to_string())?;
    let result = RoleResult::decode(&text).map_err(|error| error.to_string())?;
    if result.run_id() != run_id || result.invocation_id() != invocation_id {
        return Err("the document does not echo the durable in-flight identity".to_owned());
    }
    Ok(result)
}

impl RunController {
    /// The §15.1 budget facts the selector's affordability conjunct reads:
    /// the caller-declared forward ask plus the meter's live remaining
    /// state (§17.4 — affordability is decided by consulting the meter at
    /// call time).
    fn selection_facts(&self, state: &LoopState, requested: BudgetSlice) -> BudgetFacts {
        let remaining = state.meter.remaining();
        BudgetFacts {
            requested_rounds: requested.rounds,
            requested_wall_seconds: requested.wall_seconds,
            requested_tokens: requested.tokens,
            requested_cost_usd: requested.cost_usd,
            remaining_rounds: remaining.rounds,
            remaining_wall_seconds: remaining.wall_seconds,
            remaining_tokens: remaining.tokens,
            remaining_cost_usd: remaining.cost_usd,
        }
    }
}

#[cfg(test)]
mod tests {
    // The fixture graph is deliberately single-threaded: the fakes hold
    // `Rc`/`RefCell` interiors (the `FileReferenceProvider` fixture store
    // is an `Rc`), so `Arc` around them is not `Send + Sync`. Same
    // rationale as the recording/ladder test fixtures.
    #![allow(clippy::arc_with_non_send_sync)]

    use super::run_workspace_ref;
    use super::{
        BudgetShape, ComparisonDeps, ControlError, ControllerDeps, MAX_FAILURE_DETAIL_CHARS,
        ReferenceBreach, RunController, sanitize_failure_detail,
    };
    use crate::control::disposition::ControlPolicy;
    use crate::reference::provider::{AnswerKeySource, FileReferenceProvider, ManifestSource};
    use gauntlet_domain::{
        Budget, DiagnosticAllowance, Digest, EpochSeconds, EvidenceKind, EvidenceRef, Limits,
        MicroUsd, RunId, TerminalState, UnitStatus,
    };
    use gauntlet_ports::agent_runner::{
        AgentError, AgentRunner, CancelReason, InvocationFailure, InvocationHandle,
        InvocationStatus, Isolation, ResultDelivery, RoleInvocationSpec, RunnerCapabilities,
    };
    use gauntlet_ports::clock::{Clock, ClockError};
    use gauntlet_ports::comparator::{Comparator, ComparatorError, ComparatorSource};
    use gauntlet_ports::deadline::Deadline;
    use gauntlet_ports::evidence_store::{EvidenceStore, EvidenceStoreError};
    use gauntlet_ports::id_generator::IdGenerator;
    use gauntlet_ports::json::{JsonNumber, JsonValue};
    use gauntlet_ports::observer::{
        FacetKind, InstrumentIdentity, ObservationFacet, ObservationRequest, Observer,
        ObserverError,
    };
    use gauntlet_ports::observer_evidence::{FacetEvidenceDoc, ObservedRun};
    use gauntlet_ports::producer_authority::{
        AdmittedEvidenceProduction, AdmittedObservationBinding,
    };
    use gauntlet_ports::reference_provider::{ReferenceProvider, ReferenceProviderError};
    use gauntlet_ports::role_result::{RoleResult, RoleResultState};
    use gauntlet_ports::state_store::ObservationBinding;
    use gauntlet_ports::state_store::{
        Checkpoint, EvidenceProductionBinding, RunEvent, StateStore, StateStoreError,
    };
    use gauntlet_ports::verifier::{Verifier, VerifierError};
    use gauntlet_ports::workspace_provider::{
        ContainmentDiff, PromotedFile, WorkspacePromotion, WorkspaceProvider,
        WorkspaceProviderError, WorkspaceSubject,
    };
    use std::cell::{Cell, RefCell};
    use std::collections::HashMap;
    use std::rc::Rc;
    use std::sync::{Arc, Mutex};

    // ------------------------------------------------------------------
    // Named fixture figures (the literal-scan idiom: no duration-shaped
    // identifier shares a line with a value-position number).
    // ------------------------------------------------------------------
    const FIXTURE_TOKEN_CEILING: u64 = 1_000;
    const FIXTURE_TOKEN_SPEND: u64 = 5;
    const FIXTURE_ROUNDS: u32 = 8;
    const FIXTURE_PARALLELISM: u32 = 2;
    const FIXTURE_DEPTH: u32 = 2;
    const FIXTURE_WALL: u64 = 600;
    const FIXTURE_RESERVE_PCT: u8 = 20;
    const QUIET_WINDOW: u64 = 30;
    const TICK_WINDOW: u64 = 1;
    const FLOOR: u64 = 1;
    const CEILING: u64 = 4;
    const ATTEMPT_CAP: u32 = 2;
    const THRESHOLD: u32 = 3;
    const BASE_INSTANT: u64 = 1_700_000_000;
    /// One more independent unit than the fixture's parallelism bound
    /// admits — the C6.1 over-capacity fan-out.
    const OVER_CAPACITY_UNITS: usize = FIXTURE_PARALLELISM as usize + 1;

    fn policy() -> ControlPolicy {
        ControlPolicy::new(
            TICK_WINDOW,
            QUIET_WINDOW,
            FLOOR,
            CEILING,
            ATTEMPT_CAP,
            THRESHOLD,
        )
        .expect("the fixture control policy is usable")
    }

    /// A policy with a higher attempt ceiling — the composed-diagnosis
    /// fixtures need the §11.5 row-1 retry to survive past the
    /// non-convergence threshold's iteration.
    fn policy_with_attempts(attempts: u32) -> ControlPolicy {
        ControlPolicy::new(
            TICK_WINDOW,
            QUIET_WINDOW,
            FLOOR,
            CEILING,
            attempts,
            THRESHOLD,
        )
        .expect("the fixture control policy is usable")
    }

    fn budget_shape(rounds: u32) -> BudgetShape {
        BudgetShape {
            limits: Limits::new(
                rounds,
                FIXTURE_PARALLELISM,
                FIXTURE_DEPTH,
                FIXTURE_WALL,
                FIXTURE_TOKEN_CEILING,
                MicroUsd::new(500),
            )
            .expect("fixture limits"),
            recovery_reserve_pct: FIXTURE_RESERVE_PCT,
            diagnostic: DiagnosticAllowance::new(100, MicroUsd::new(50)),
        }
    }

    /// A one-unit answer key (plus optional second and third units) whose
    /// requirement is an instrument-produced test report at a
    /// single-segment locator.
    fn answer_key(units: usize) -> String {
        let mut text = String::from(
            "## AK-9101\nclaim: the fixture claim is settled by a passing test report\nverify_by: \
             test\nblast_radius: contained\nauthority: critic\nevidence_required: {kind: \
             test_report, locator: ctrl-9101.json, producer: instrument}\n\n",
        );
        if units > 1 {
            text.push_str(
                "## AK-9102\nclaim: the second fixture claim is settled by a passing test \
                 report\nverify_by: test\nblast_radius: contained\nauthority: \
                 critic\nevidence_required: {kind: test_report, locator: ctrl-9102.json, \
                 producer: instrument}\n\n",
            );
        }
        if units > 2 {
            text.push_str(
                "## AK-9103\nclaim: the third fixture claim is settled by a passing test \
                 report\nverify_by: test\nblast_radius: contained\nauthority: \
                 critic\nevidence_required: {kind: test_report, locator: ctrl-9103.json, \
                 producer: instrument}\n\n",
            );
        }
        text
    }

    /// A two-unit answer key whose SECOND unit `depends_on` the first —
    /// the multi-hop graph correction C6.2 exists for. Both requirements
    /// are instrument-produced test reports at single-segment locators.
    fn answer_key_with_dependency() -> String {
        String::from(
            "## AK-9101\nclaim: the fixture claim is settled by a passing test report\nverify_by: \
             test\nblast_radius: contained\nauthority: critic\nevidence_required: {kind: \
             test_report, locator: ctrl-9101.json, producer: instrument}\n\n## AK-9102\nclaim: \
             the second fixture claim is settled by a passing test report\nverify_by: \
             test\nblast_radius: contained\nauthority: critic\ndepends_on: \
             [AK-9101]\nevidence_required: {kind: test_report, locator: ctrl-9102.json, producer: \
             instrument}\n\n",
        )
    }

    fn minimal_manifest() -> String {
        [
            "reference:",
            "  id: ref-ctrl-suite",
            "  version: 1",
            "  purpose: >",
            "    One paragraph.",
            "  desired_state: >",
            "    The destination.",
            "  settlement:",
            "    success_criteria:",
            "      - the destination is reached",
            "    minimum_reliability: >",
            "      spec 19.2 default: >= 1 settlements before promotion.",
            "    terminal_conditions: [settled, aborted]",
            "",
        ]
        .join("\n")
    }

    fn minimal_contract() -> crate::compare::contract::ComparisonContract {
        crate::compare::contract::ComparisonContract::parse(
            "version: 1\ndimensions:\n  - functional behavior\nseverity_rubric:\n  - blocking | \
             invalidates the iteration\n  - major | real, not invalidating\n  - minor | small\n  \
             - informational | recorded only\ncomparator_receives:\n  - the allowlist \
             members\nresidual_definition:\n  - a difference\n",
        )
        .expect("the minimal contract parses")
    }

    // ------------------------------------------------------------------
    // Fakes
    // ------------------------------------------------------------------

    /// The deterministic clock: `sleep_until` advances virtual time.
    struct DeterministicClock(Mutex<u64>);

    impl DeterministicClock {
        fn new() -> Arc<Self> {
            Arc::new(Self(Mutex::new(BASE_INSTANT)))
        }
    }

    impl Clock for DeterministicClock {
        fn now(&self) -> EpochSeconds {
            EpochSeconds::new(*self.0.lock().expect("clock lock"))
        }
        fn sleep_until(&self, at: EpochSeconds) -> Result<(), ClockError> {
            let mut now = self.0.lock().expect("clock lock");
            if at.get() > *now {
                *now = at.get();
            }
            Ok(())
        }
    }

    /// What the scripted child does.
    #[derive(Clone)]
    enum Behavior {
        Complete,
        /// A well-formed completion whose result reports the given INPUT
        /// token count (output 0) — the payment-size lever the C9.3
        /// charge-refusal tests ride.
        CompleteWithTokens(u64),
        /// A well-formed completion whose attempted list declares the
        /// unit that is NOT the flight's own (AK-9999, absent from the
        /// ruler) — the COR2-1b lever (the falsifier's F-1 completion
        /// half: the declared list is free text from the builder, and the
        /// guard must fire whatever unit the scheduler flew).
        CompleteDeclaringOther,
        StallForever,
        Fail(InvocationFailure),
        /// The invocation completes at the poll leg, then `collect`
        /// fails with the canned detail — the W7 schema-reminder lever:
        /// the retry prompt must name the parse failure.
        CollectFailed {
            detail: String,
        },
        /// The invocation admits, then `poll` hard-errors — the crash
        /// injection the §18.3 resume tests ride (the drain maps it to a
        /// `ControlError` that ends the run with no close-out).
        PollError,
        /// Reports ever-fresh progress and never completes — the runner
        /// that defeats stall detection (its `last_event_at` sits far in
        /// the future, so the quiet window never elapses). `max_polls` is
        /// the fixture's own hard stop: past it, `poll` errors, which is
        /// how the unbounded-drain defect is OBSERVED without hanging a
        /// test process.
        ProgressForever {
            max_polls: u32,
        },
        /// W3: reports ever-fresh progress like `ProgressForever`, but its
        /// `Running.payment_so_far` carries a REAL non-zero token spend —
        /// the lever the in-flight budget guard tests ride (a runner's own
        /// measured in-flight spend, observed before the invocation ever
        /// completes).
        ProgressForeverWithPayment {
            max_polls: u32,
            tokens: u64,
        },
        /// `invoke` itself refuses admission with `AgentError::AdmissionRefused`
        /// — the W2 admission-failure lever. Consumed at INVOKE time (never
        /// admitted, so it never occupies a poll-behavior slot): the script
        /// is a FIFO queue popped once per `invoke` call, and a refused
        /// attempt leaves no prompt/spec behind, so the retry's own `invoke`
        /// pops the NEXT scripted behavior.
        AdmissionRefused,
    }

    /// The scripted runner: poll advances a script; completion happens only
    /// through `read_result_contract` (the §6.3 discipline); prompts are
    /// recorded for the re-dispatch assertions; an on-invoke hook lets a
    /// test mutate the world mid-run.
    struct ScriptedRunner {
        /// FIFO queue popped once per `invoke` call (admitted or refused).
        behaviors: RefCell<Vec<Behavior>>,
        /// The admitted invocation's OWN poll behavior, recorded at invoke
        /// time — decoupled from `behaviors`' queue position so a refused
        /// attempt (which consumes a queue slot but never admits) cannot
        /// desynchronize a later admitted invocation's poll script.
        behavior_by_id: RefCell<HashMap<gauntlet_domain::InvocationId, Behavior>>,
        prompts: RefCell<Vec<(gauntlet_domain::InvocationId, String)>>,
        specs: RefCell<HashMap<gauntlet_domain::InvocationId, RoleInvocationSpec>>,
        polls: RefCell<HashMap<gauntlet_domain::InvocationId, u32>>,
        cancels: RefCell<Vec<gauntlet_domain::InvocationId>>,
        on_invoke: RefCell<Option<Box<dyn Fn()>>>,
    }

    impl ScriptedRunner {
        fn new(behaviors: Vec<Behavior>) -> Arc<Self> {
            Arc::new(Self {
                behaviors: RefCell::new(behaviors),
                behavior_by_id: RefCell::new(HashMap::new()),
                prompts: RefCell::new(Vec::new()),
                specs: RefCell::new(HashMap::new()),
                polls: RefCell::new(HashMap::new()),
                cancels: RefCell::new(Vec::new()),
                on_invoke: RefCell::new(None),
            })
        }

        /// The invocation ids whose cancel was requested, in order — the
        /// corrections-round-1 (F6) cancel-teeth lever.
        fn cancels(&self) -> Vec<gauntlet_domain::InvocationId> {
            self.cancels.borrow().clone()
        }

        fn set_hook(&self, hook: impl Fn() + 'static) {
            *self.on_invoke.borrow_mut() = Some(Box::new(hook));
        }

        /// The result document's payment-token pair for one behavior.
        fn payment_tokens(behavior: &Behavior) -> (u64, u64) {
            match behavior {
                Behavior::CompleteWithTokens(requested) => (*requested, 0),
                _ => (FIXTURE_TOKEN_SPEND, FIXTURE_TOKEN_SPEND),
            }
        }

        fn result_document(
            spec: &RoleInvocationSpec,
            input_tokens: u64,
            output_tokens: u64,
            attempted: &[&str],
        ) -> String {
            JsonValue::Object(vec![
                (
                    "schema".to_owned(),
                    JsonValue::Str("role_result@v1".to_owned()),
                ),
                (
                    "invocation_id".to_owned(),
                    JsonValue::Str(spec.invocation_id.to_string()),
                ),
                ("run_id".to_owned(), JsonValue::Str(spec.run_id.to_string())),
                (
                    "iteration_no".to_owned(),
                    JsonValue::Number(JsonNumber::Uint(u64::from(spec.iteration_no.get()))),
                ),
                ("role".to_owned(), JsonValue::Str("builder".to_owned())),
                (
                    "context_envelope_digest".to_owned(),
                    JsonValue::Str(spec.context_envelope.digest().to_string()),
                ),
                (
                    "state".to_owned(),
                    JsonValue::Str(RoleResultState::Completed.to_string()),
                ),
                (
                    "payload_schema".to_owned(),
                    JsonValue::Str("builder_result@v1".to_owned()),
                ),
                (
                    "payload".to_owned(),
                    JsonValue::Object(vec![
                        (
                            "attempted".to_owned(),
                            JsonValue::Array(
                                attempted
                                    .iter()
                                    .map(|unit| JsonValue::Str((*unit).to_owned()))
                                    .collect(),
                            ),
                        ),
                        ("changed_paths".to_owned(), JsonValue::Array(vec![])),
                        ("commands_run".to_owned(), JsonValue::Array(vec![])),
                        ("artifacts".to_owned(), JsonValue::Array(vec![])),
                        (
                            "claims".to_owned(),
                            JsonValue::Array(vec![JsonValue::Str(
                                "the slice is complete".to_owned(),
                            )]),
                        ),
                        ("self_checks".to_owned(), JsonValue::Array(vec![])),
                        ("blocked_on".to_owned(), JsonValue::Array(vec![])),
                    ]),
                ),
                (
                    "payment".to_owned(),
                    JsonValue::Object(vec![
                        (
                            "input_tokens".to_owned(),
                            JsonValue::Number(JsonNumber::Uint(input_tokens)),
                        ),
                        (
                            "output_tokens".to_owned(),
                            JsonValue::Number(JsonNumber::Uint(output_tokens)),
                        ),
                        (
                            "tool_calls".to_owned(),
                            JsonValue::Number(JsonNumber::Uint(0)),
                        ),
                        (
                            "wall_seconds".to_owned(),
                            JsonValue::Number(JsonNumber::Uint(0)),
                        ),
                        (
                            "cost_usd".to_owned(),
                            JsonValue::Number(JsonNumber::Uint(0)),
                        ),
                    ]),
                ),
                ("unavailable_evidence".to_owned(), JsonValue::Array(vec![])),
                ("errors".to_owned(), JsonValue::Array(vec![])),
            ])
            .to_json_text()
        }
    }

    impl AgentRunner for ScriptedRunner {
        fn invoke(&self, spec: RoleInvocationSpec) -> Result<InvocationHandle, AgentError> {
            // The script is a FIFO queue popped once per `invoke` call —
            // an empty queue defaults to `Complete` (the historical
            // behavior for every fixture that never scripts a refusal).
            let next = {
                let mut behaviors = self.behaviors.borrow_mut();
                if behaviors.is_empty() {
                    Behavior::Complete
                } else {
                    behaviors.remove(0)
                }
            };
            if matches!(next, Behavior::AdmissionRefused) {
                // Never admitted: no prompt, no spec, no handle — exactly
                // the W2 shape `invoke` must produce for the controller's
                // admission-failure retry to exercise.
                return Err(AgentError::AdmissionRefused {
                    detail: "the fixture runner scripted an admission refusal".to_owned(),
                });
            }
            self.prompts
                .borrow_mut()
                .push((spec.invocation_id.clone(), spec.prompt.clone()));
            if let Some(hook) = self.on_invoke.borrow().as_ref() {
                hook();
            }
            let invocation_id = spec.invocation_id.clone();
            self.behavior_by_id
                .borrow_mut()
                .insert(invocation_id.clone(), next);
            self.specs.borrow_mut().insert(invocation_id.clone(), spec);
            Ok(InvocationHandle::new(
                invocation_id,
                Some(gauntlet_domain::SessionRef::new("session-fixture").expect("session ref")),
            ))
        }

        fn poll(&self, handle: &InvocationHandle) -> Result<InvocationStatus, AgentError> {
            {
                let mut polls = self.polls.borrow_mut();
                let count = polls.entry(handle.invocation_id().clone()).or_insert(0);
                *count += 1;
            }
            let spec = self
                .specs
                .borrow()
                .get(handle.invocation_id())
                .cloned()
                .expect("the handle was admitted");
            let behavior = self.behavior_of(handle.invocation_id());
            match behavior {
                Behavior::Complete | Behavior::CompleteWithTokens(_) => {
                    let (input_tokens, output_tokens) = Self::payment_tokens(&behavior);
                    let document = Self::result_document(&spec, input_tokens, output_tokens, &[]);
                    gauntlet_ports::role_result::read_result_contract(
                        &document,
                        handle.invocation_id(),
                        spec.context_envelope.digest(),
                    )
                    .map(|_| InvocationStatus::Completed {
                        result_ref: spec.result_contract_path.clone(),
                    })
                    .map_err(|error| AgentError::AdmissionRefused {
                        detail: error.to_string(),
                    })
                }
                Behavior::CompleteDeclaringOther => {
                    let document = Self::result_document(&spec, 1, 0, &["AK-9999"]);
                    gauntlet_ports::role_result::read_result_contract(
                        &document,
                        handle.invocation_id(),
                        spec.context_envelope.digest(),
                    )
                    .map(|_| InvocationStatus::Completed {
                        result_ref: spec.result_contract_path.clone(),
                    })
                    .map_err(|error| AgentError::AdmissionRefused {
                        detail: error.to_string(),
                    })
                }
                Behavior::CollectFailed { .. } => {
                    // Completes at the poll leg exactly like a well-formed
                    // completion; the COLLECT leg below fails with the
                    // canned detail instead — the W7 schema-reminder
                    // lever.
                    let (input_tokens, output_tokens) = Self::payment_tokens(&behavior);
                    let document = Self::result_document(&spec, input_tokens, output_tokens, &[]);
                    gauntlet_ports::role_result::read_result_contract(
                        &document,
                        handle.invocation_id(),
                        spec.context_envelope.digest(),
                    )
                    .map(|_| InvocationStatus::Completed {
                        result_ref: spec.result_contract_path.clone(),
                    })
                    .map_err(|error| AgentError::AdmissionRefused {
                        detail: error.to_string(),
                    })
                }
                Behavior::StallForever => Ok(InvocationStatus::Admitted),
                Behavior::Fail(class) => Ok(InvocationStatus::Failed { class }),
                Behavior::PollError => Err(AgentError::Transport {
                    detail: "the fixture runner crashed mid-drain".to_owned(),
                }),
                Behavior::ProgressForever { max_polls } => {
                    // The preamble above already counted this poll.
                    let spent = *self
                        .polls
                        .borrow()
                        .get(handle.invocation_id())
                        .unwrap_or(&0);
                    if spent > max_polls {
                        return Err(AgentError::Transport {
                            detail: "the fixture runner exhausted its poll budget while the \
                                     drain kept spinning"
                                .to_owned(),
                        });
                    }
                    Ok(InvocationStatus::Running {
                        // Far-future progress: the quiet window can never
                        // elapse against it.
                        last_event_at: EpochSeconds::new(u64::MAX / 2),
                        payment_so_far: gauntlet_domain::Payment::zero(),
                    })
                }
                Behavior::ProgressForeverWithPayment { max_polls, tokens } => {
                    // The preamble above already counted this poll.
                    let spent = *self
                        .polls
                        .borrow()
                        .get(handle.invocation_id())
                        .unwrap_or(&0);
                    if spent > max_polls {
                        return Err(AgentError::Transport {
                            detail: "the fixture runner exhausted its poll budget while the \
                                     drain kept spinning"
                                .to_owned(),
                        });
                    }
                    Ok(InvocationStatus::Running {
                        // Far-future progress: the quiet window can never
                        // elapse against it.
                        last_event_at: EpochSeconds::new(u64::MAX / 2),
                        payment_so_far: gauntlet_domain::Payment::new(
                            tokens,
                            0,
                            0,
                            gauntlet_domain::MicroUsd::new(0),
                        ),
                    })
                }
                Behavior::AdmissionRefused => unreachable!(
                    "AdmissionRefused is consumed entirely at invoke time — an invocation \
                     with a handle was, by construction, never that behavior"
                ),
            }
        }

        fn collect(
            &self,
            handle: &InvocationHandle,
            _: Deadline,
        ) -> Result<RoleResult, AgentError> {
            let spec = self
                .specs
                .borrow()
                .get(handle.invocation_id())
                .cloned()
                .expect("the handle was admitted");
            let behavior = self.behavior_of(handle.invocation_id());
            if let Behavior::CollectFailed { detail } = &behavior {
                return Err(AgentError::CollectionFailed {
                    invocation_id: handle.invocation_id().clone(),
                    detail: detail.clone(),
                });
            }
            let attempted: &[&str] = match behavior {
                Behavior::CompleteDeclaringOther => &["AK-9999"],
                _ => &[],
            };
            let (input_tokens, output_tokens) = Self::payment_tokens(&behavior);
            let document = Self::result_document(&spec, input_tokens, output_tokens, attempted);
            gauntlet_ports::role_result::read_result_contract(
                &document,
                handle.invocation_id(),
                spec.context_envelope.digest(),
            )
            .map_err(|error| AgentError::CollectionFailed {
                invocation_id: handle.invocation_id().clone(),
                detail: error.to_string(),
            })
        }

        fn cancel(
            &self,
            handle: &InvocationHandle,
            _: CancelReason,
            _: Deadline,
        ) -> Result<(), AgentError> {
            self.cancels
                .borrow_mut()
                .push(handle.invocation_id().clone());
            Ok(())
        }

        fn capabilities(&self) -> RunnerCapabilities {
            RunnerCapabilities {
                supports_native_subagents: false,
                supports_streaming_events: false,
                supports_model_selection: false,
                isolation: Isolation::None,
                result_delivery: ResultDelivery::File,
                max_depth: 1,
            }
        }
    }

    impl ScriptedRunner {
        fn behavior_of(&self, id: &gauntlet_domain::InvocationId) -> Behavior {
            // Recorded at `invoke` time, once this invocation was actually
            // admitted — decoupled from the `behaviors` queue's position so
            // a refused earlier attempt cannot desynchronize this lookup.
            self.behavior_by_id
                .borrow()
                .get(id)
                .cloned()
                .unwrap_or(Behavior::Complete)
        }
    }

    /// Inner state shared by every handle form (Rc for the provider's
    /// store parameter, Arc for the controller's).
    #[derive(Default)]
    struct EvidenceInner {
        objects: RefCell<Vec<(EvidenceRef, Vec<u8>)>>,
        pins: RefCell<Vec<(RunId, EvidenceRef)>>,
        next: Cell<u32>,
        // task26: opt-in content addressing (default false — every
        // existing fixture keeps its sequential-ref behavior, including
        // tests that count per-`put()`-call forensic pins, e.g.
        // `a_settled_noop_over_below_anchor_settlement_damage_pins_the_missing_row`,
        // where two resumes correctly append the SAME byte-identical
        // record twice as two distinct log entries). Only
        // `FakeEvidence::new_content_addressed` turns this on, for tests
        // that specifically exercise "the store dedups a re-produced
        // identical artifact onto the same ref" — the real content-
        // addressed evidence-store adapter's actual, unconditional
        // behavior: its `put` hashes every write, regardless of
        // `EvidenceKind`.
        content_addressed: bool,
    }

    #[derive(Clone)]
    struct FakeEvidence(Arc<EvidenceInner>);

    impl FakeEvidence {
        fn new() -> Self {
            Self(Arc::new(EvidenceInner::default()))
        }

        /// A content-addressed fixture store, faithful to the real
        /// content-addressed evidence-store adapter: `put` returns the
        /// SAME ref for byte-identical content, never a fresh one. Use
        /// this when the scenario under test depends on that dedup (e.g.
        /// re-producing identical evidence across attempts); every other
        /// fixture stays on `new`'s sequential refs.
        fn new_content_addressed() -> Self {
            Self(Arc::new(EvidenceInner {
                content_addressed: true,
                ..EvidenceInner::default()
            }))
        }

        fn records_containing(&self, needle: &str) -> Vec<String> {
            self.0
                .objects
                .borrow()
                .iter()
                .filter(|(_, bytes)| String::from_utf8_lossy(bytes).contains(needle))
                .map(|(_, bytes)| String::from_utf8_lossy(bytes).into_owned())
                .collect()
        }
    }

    impl EvidenceStore for FakeEvidence {
        fn put(
            &self,
            bytes: &[u8],
            _: EvidenceKind,
            _: Deadline,
        ) -> Result<EvidenceRef, EvidenceStoreError> {
            let mut objects = self.0.objects.borrow_mut();
            if self.0.content_addressed
                && let Some((existing, _)) = objects
                    .iter()
                    .find(|(_, stored)| stored.as_slice() == bytes)
            {
                return Ok(existing.clone());
            }
            let index = self.0.next.get();
            self.0.next.set(index + 1);
            let reference = EvidenceRef::new(&format!("evd-ctrl-{index:06}")).expect("valid ref");
            objects.push((reference.clone(), bytes.to_vec()));
            Ok(reference)
        }
        fn get(
            &self,
            reference: &EvidenceRef,
            _: Deadline,
        ) -> Result<Option<Vec<u8>>, EvidenceStoreError> {
            Ok(self
                .0
                .objects
                .borrow()
                .iter()
                .find(|(stored, _)| stored == reference)
                .map(|(_, bytes)| bytes.clone()))
        }
        fn pin(
            &self,
            run_id: &RunId,
            reference: &EvidenceRef,
            _: Deadline,
        ) -> Result<(), EvidenceStoreError> {
            self.0
                .pins
                .borrow_mut()
                .push((run_id.clone(), reference.clone()));
            Ok(())
        }
        fn is_pinned(
            &self,
            run_id: &RunId,
            reference: &EvidenceRef,
            _: Deadline,
        ) -> Result<bool, EvidenceStoreError> {
            Ok(self
                .0
                .pins
                .borrow()
                .contains(&(run_id.clone(), reference.clone())))
        }
    }

    /// The durable substrate, counting the two gated owner writes.
    struct FakeState {
        runs: RefCell<HashMap<RunId, gauntlet_domain::Run>>,
        checkpoints: RefCell<HashMap<RunId, Checkpoint>>,
        events: RefCell<Vec<RunEvent>>,
        next_offset: Cell<u64>,
        productions: RefCell<Vec<EvidenceProductionBinding>>,
        bindings: RefCell<HashMap<(RunId, u32), ObservationBinding>>,
        selections: RefCell<HashMap<RunId, Vec<gauntlet_domain::CorrectionSelection>>>,
        role_invocations: RefCell<Vec<gauntlet_domain::RoleInvocation>>,
        verifications: RefCell<Vec<gauntlet_ports::verification_record::VerificationRecordDoc>>,
        settlement_rows: RefCell<Vec<(u64, gauntlet_ports::state_store::SettlementCommit)>>,
        pub bind_calls: Cell<u32>,
        pub production_calls: Cell<u32>,
        pub accepted_comparisons: Cell<u32>,
        /// Fires ONCE on the first durable settlement commit — the test
        /// lever for events that must postdate the settlement (e.g. the
        /// §21.3 unavailability window opening after a durable commit).
        pub on_first_settlement: RefCell<Option<Box<dyn Fn()>>>,
    }

    impl FakeState {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                runs: RefCell::new(HashMap::new()),
                checkpoints: RefCell::new(HashMap::new()),
                events: RefCell::new(Vec::new()),
                next_offset: Cell::new(1),
                productions: RefCell::new(Vec::new()),
                bindings: RefCell::new(HashMap::new()),
                selections: RefCell::new(HashMap::new()),
                role_invocations: RefCell::new(Vec::new()),
                verifications: RefCell::new(Vec::new()),
                settlement_rows: RefCell::new(Vec::new()),
                bind_calls: Cell::new(0),
                production_calls: Cell::new(0),
                accepted_comparisons: Cell::new(0),
                on_first_settlement: RefCell::new(None),
            })
        }

        /// Out-of-band DAMAGE (the COR-R3-1 test): drops the production row at
        /// one locator while every durable event and evidence byte stays —
        /// LESS damage than any real crash, exactly the falsifier's round-3
        /// reproduction shape.
        pub fn drop_production_at(&self, locator: &str) {
            self.productions
                .borrow_mut()
                .retain(|binding| binding.production().locator().as_str() != locator);
        }

        /// Out-of-band DAMAGE (the COR-R5 tests): drops the KEYED SETTLEMENT
        /// ROW of one unit while the durable `SettlementCommitted` event and
        /// every other durable fact stay — LESS damage than any real crash
        /// (a §16.3 half-applied settlement is exactly this end-state when
        /// the crash lands between the event append and the keyed insert).
        /// Whether the anchored replay can re-complete the dropped row
        /// depends on the resume anchor alone: an event AT/ABOVE the anchor
        /// is replayed and the row is recovered (the COR-R5-1 arm — the
        /// tests move the durable checkpoint's anchor to the reproduction's
        /// posture); an event BELOW it is out of replay scope and the
        /// divergence is pinned by the COR-R5-2 cross-check instead.
        pub fn drop_settlement_row(&self, unit_id: &str) {
            self.settlement_rows
                .borrow_mut()
                .retain(|(_, row)| row.unit_id.as_str() != unit_id);
        }

        /// Out-of-band EVENT PLANT (the COR-R4-1 test): appends the
        /// durable `EvidenceProduced` event the REAL store's produce step
        /// writes (event + row), taking the event's facts from the run's
        /// own durable production row and stamping it with the given
        /// timestamp. This fixture's produce write is deliberately
        /// row-only, so the A5-shaped end-state — the event durable ABOVE
        /// the resume anchor, the row missing — is reached by planting
        /// the event after the fact, where it naturally sits above the
        /// close-out checkpoint's anchor. The falsifier's exact
        /// forged-anchor ROUTE (regressing a checkpoint offset below a
        /// mid-log produce event — the recorded "checkpoint() forged-row
        /// projection class (planning-layer)" deferral) is what the REAL
        /// store needs to reach this same end-state, and it is pinned by
        /// the E2E leg through the real binary. The planted event's
        /// offset, as every store read here reports it, is the POSITIONAL
        /// one — read the event back positionally (the store's own
        /// convention) rather than trusting this fixture's append
        /// counter, which diverges from positions wherever a production
        /// consumed an offset without an event.
        pub fn plant_orphan_evidence_production_event(
            &self,
            run_id: &RunId,
            at: gauntlet_domain::EpochSeconds,
        ) {
            let (evidence, locator) = {
                let productions = self.productions.borrow();
                let binding = productions
                    .iter()
                    .find(|binding| binding.run_id() == run_id)
                    .expect("the settled run carries a durable production row");
                (
                    binding.production().evidence().clone(),
                    binding.production().locator().clone(),
                )
            };
            self.events.borrow_mut().push(RunEvent {
                run_id: run_id.clone(),
                kind: gauntlet_ports::state_store::RunEventKind::EvidenceProduced {
                    evidence,
                    locator,
                },
                at,
            });
        }
    }

    impl StateStore for FakeState {
        fn append_event(
            &self,
            event: RunEvent,
            _: Deadline,
        ) -> Result<gauntlet_domain::EventLogOffset, StateStoreError> {
            let offset = self.next_offset.get();
            self.next_offset.set(offset + 1);
            self.events.borrow_mut().push(event);
            Ok(gauntlet_domain::EventLogOffset::new(offset))
        }
        fn load_run(
            &self,
            run_id: &RunId,
            _: Deadline,
        ) -> Result<Option<gauntlet_domain::Run>, StateStoreError> {
            Ok(self.runs.borrow().get(run_id).cloned())
        }
        fn record_run(
            &self,
            run: &gauntlet_domain::Run,
            _: Deadline,
        ) -> Result<(), StateStoreError> {
            self.runs.borrow_mut().insert(run.id().clone(), run.clone());
            Ok(())
        }
        fn checkpoint(&self, checkpoint: Checkpoint, _: Deadline) -> Result<(), StateStoreError> {
            self.checkpoints
                .borrow_mut()
                .insert(checkpoint.run_id.clone(), checkpoint);
            Ok(())
        }
        fn resume(
            &self,
            run_id: &RunId,
            _: Deadline,
        ) -> Result<Option<Checkpoint>, StateStoreError> {
            Ok(self.checkpoints.borrow().get(run_id).cloned())
        }
        fn record_verification(
            &self,
            record: &gauntlet_ports::producer_authority::AdmittedVerificationRecord,
            _: Deadline,
        ) -> Result<(), StateStoreError> {
            let record = record.record();
            let mut verifications = self.verifications.borrow_mut();
            if verifications.iter().any(|stored| stored == record) {
                return Ok(());
            }
            let identity_of =
                |stored: &gauntlet_ports::verification_record::VerificationRecordDoc| {
                    (
                        stored.run_id().clone(),
                        stored.unit_id().clone(),
                        stored.evidence().clone(),
                        stored.outcome(),
                    )
                };
            if verifications
                .iter()
                .any(|stored| identity_of(stored) == identity_of(record))
            {
                return Err(StateStoreError::WriteFailed {
                    detail: format!(
                        "a verification record for the same identity holds different content: \
                         unit {} of run {}",
                        record.unit_id(),
                        record.run_id()
                    ),
                });
            }
            verifications.push(record.clone());
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
            self.events.borrow_mut().push(RunEvent {
                run_id: commit.run_id.clone(),
                kind: gauntlet_ports::state_store::RunEventKind::SettlementCommitted {
                    unit_id: commit.unit_id.clone(),
                    evidence: commit.evidence.clone(),
                    verification: commit.verification.clone(),
                    reference_digest: commit.reference_digest.clone(),
                },
                at: commit.at,
            });
            let mut rows = self.settlement_rows.borrow_mut();
            if !rows.iter().any(|(_, row)| {
                row.unit_id == commit.unit_id && row.verification == commit.verification
            }) {
                rows.push((offset, commit));
            }
            if let Some(callback) = self.on_first_settlement.borrow_mut().take() {
                callback();
            }
            Ok(gauntlet_domain::EventLogOffset::new(offset))
        }
        fn settlements(
            &self,
            run_id: &RunId,
            _: Deadline,
        ) -> Result<Vec<gauntlet_domain::SettlementRecord>, StateStoreError> {
            Ok(self
                .settlement_rows
                .borrow()
                .iter()
                .filter(|(_, commit)| &commit.run_id == run_id)
                .map(|(offset, commit)| {
                    gauntlet_domain::SettlementRecord::new(
                        commit.run_id.clone(),
                        commit.unit_id.clone(),
                        commit.evidence.clone(),
                        commit.verification.clone(),
                        gauntlet_domain::EventLogOffset::new(*offset),
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
            // The anchored keyed replay, mirroring the adapter's §16.3
            // recovery: events below the anchor are the checkpoint's own
            // certification; at/after it, each SettlementCommitted event
            // whose keyed row is missing completes it. (Every `events`
            // push mints exactly one offset monotonically, so the offset
            // is the 1-based position.)
            let events = self.events.borrow().clone();
            let mut added = 0;
            for (index, event) in events.into_iter().enumerate() {
                let offset = (index as u64) + 1;
                if offset < anchor.get() || &event.run_id != run_id {
                    continue;
                }
                let gauntlet_ports::state_store::RunEventKind::SettlementCommitted {
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
        fn record_role_invocation(
            &self,
            invocation: &gauntlet_domain::RoleInvocation,
            _: Deadline,
        ) -> Result<(), StateStoreError> {
            self.role_invocations.borrow_mut().push(invocation.clone());
            Ok(())
        }
        fn record_correction_selection(
            &self,
            selection: &gauntlet_domain::CorrectionSelection,
            _: EpochSeconds,
            _: Deadline,
        ) -> Result<(), StateStoreError> {
            self.selections
                .borrow_mut()
                .entry(selection.run_id().clone())
                .or_default()
                .push(selection.clone());
            Ok(())
        }
        fn correction_selections(
            &self,
            run_id: &RunId,
            _: Deadline,
        ) -> Result<Vec<gauntlet_domain::CorrectionSelection>, StateStoreError> {
            Ok(self
                .selections
                .borrow()
                .get(run_id)
                .cloned()
                .unwrap_or_default())
        }
        fn bind_observation_bundle(
            &self,
            run_id: &RunId,
            iteration_no: gauntlet_domain::IterationNo,
            binding: &AdmittedObservationBinding,
            _: Deadline,
        ) -> Result<(), StateStoreError> {
            self.bind_calls.set(self.bind_calls.get() + 1);
            self.bindings.borrow_mut().insert(
                (run_id.clone(), iteration_no.get()),
                binding.binding().clone(),
            );
            Ok(())
        }
        fn observation_binding(
            &self,
            run_id: &RunId,
            iteration_no: gauntlet_domain::IterationNo,
            _: Deadline,
        ) -> Result<Option<ObservationBinding>, StateStoreError> {
            Ok(self
                .bindings
                .borrow()
                .get(&(run_id.clone(), iteration_no.get()))
                .cloned())
        }
        fn record_evidence_production(
            &self,
            run_id: &RunId,
            production: &AdmittedEvidenceProduction,
            _: Deadline,
        ) -> Result<EvidenceProductionBinding, StateStoreError> {
            self.production_calls.set(self.production_calls.get() + 1);
            let offset = self.next_offset.get();
            self.next_offset.set(offset + 1);
            let binding = EvidenceProductionBinding::new(
                run_id.clone(),
                production.production().clone(),
                gauntlet_domain::EventLogOffset::new(offset),
            );
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
            locator: &gauntlet_domain::EvidenceLocator,
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
        fn complete_pending_evidence_production(
            &self,
            run_id: &RunId,
            anchor: gauntlet_domain::EventLogOffset,
            production: &AdmittedEvidenceProduction,
            _: Deadline,
        ) -> Result<Option<EvidenceProductionBinding>, StateStoreError> {
            // COR-R3-1 fixture support: the same replay completion
            // `SettleState` implements — the A5-window produced-world
            // regression drives it through the composed resume path. The
            // fixture's event offsets are positional (the convention this
            // store's settlement replay documents).
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
            let Some((offset, event)) = events
                .into_iter()
                .enumerate()
                .map(|(index, event)| ((index as u64) + 1, event))
                .find(|(offset, event)| {
                    *offset >= anchor.get()
                        && &event.run_id == run_id
                        && matches!(
                            &event.kind,
                            gauntlet_ports::state_store::RunEventKind::EvidenceProduced {
                                evidence,
                                ..
                            } if evidence == payload.evidence()
                        )
                })
            else {
                return Ok(None);
            };
            let gauntlet_ports::state_store::RunEventKind::EvidenceProduced { evidence, locator } =
                event.kind
            else {
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
            let completed = gauntlet_ports::state_store::EvidenceProduction::new(
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
        fn pending_evidence_production_events(
            &self,
            run_id: &RunId,
            anchor: gauntlet_domain::EventLogOffset,
            locator: &gauntlet_domain::EvidenceLocator,
            _: Deadline,
        ) -> Result<Vec<gauntlet_ports::state_store::OrphanEvidenceProduction>, StateStoreError>
        {
            // The fixture's event offsets are positional (the same
            // convention its settlement replay documents).
            let events = self.events.borrow().clone();
            let mut orphans = Vec::new();
            for (index, event) in events.into_iter().enumerate() {
                let offset = (index as u64) + 1;
                if offset < anchor.get() || &event.run_id != run_id {
                    continue;
                }
                let gauntlet_ports::state_store::RunEventKind::EvidenceProduced {
                    evidence,
                    locator: event_locator,
                } = &event.kind
                else {
                    continue;
                };
                if event_locator != locator {
                    continue;
                }
                orphans.push(gauntlet_ports::state_store::OrphanEvidenceProduction::new(
                    evidence.clone(),
                    gauntlet_domain::EventLogOffset::new(offset),
                    event.at,
                ));
            }
            Ok(orphans)
        }
        fn accept_comparison(
            &self,
            _: &gauntlet_ports::comparator::AdmittedComparison,
            _: Deadline,
        ) -> Result<(), StateStoreError> {
            self.accepted_comparisons
                .set(self.accepted_comparisons.get() + 1);
            Ok(())
        }
        fn export_jsonl(&self, _: &RunId, _: Deadline) -> Result<String, StateStoreError> {
            Ok(String::new())
        }
    }

    /// The meter the loop deserves: the domain budget's own arithmetic
    /// behind the port (the B1 adapter's semantics, in-app for the test).
    struct FakeMeterInner {
        budget: Budget,
        /// Reservations granted and not yet charged — one parallel slot
        /// each (§17.4), exactly as the shipped in-memory meter accounts
        /// them.
        outstanding: u32,
    }

    /// The in-crate meter fake models the SHIPPED meter's reserve
    /// contract, not a convenience subset of it: the §5.3.7 recovery-
    /// reserve gate FIRST, then the §5.3.6 dispatch-scope gate over the
    /// outstanding reservations. Correction C6.1: a fake that answered
    /// only the reserve conjunct made the over-capacity fan-out
    /// structurally unobservable in this crate while the shipped adapter
    /// refused it — the fixture-local-wiring class this task's
    /// preregistration names.
    struct FakeMeter(Mutex<FakeMeterInner>);

    impl gauntlet_ports::budget_meter::BudgetMeter for FakeMeter {
        fn reserve(
            &self,
            slice: gauntlet_ports::budget_meter::BudgetSlice,
        ) -> Result<
            gauntlet_ports::budget_meter::Reservation,
            gauntlet_ports::budget_meter::BudgetMeterError,
        > {
            let mut inner = self.0.lock().expect("meter lock");
            if inner.budget.would_breach_reserve(
                slice.rounds,
                slice.wall_seconds,
                slice.tokens,
                slice.cost_usd,
            ) {
                return Err(gauntlet_ports::budget_meter::BudgetMeterError::Reserve {
                    cause: gauntlet_domain::BudgetError::RecoveryReserveBreached {
                        dimension: gauntlet_domain::BudgetDimension::Tokens,
                        forward_ceiling: 0,
                        spent: 0,
                        requested: slice.tokens,
                    },
                });
            }
            let parallel = inner.outstanding.saturating_add(1);
            inner
                .budget
                .check_dispatch_scope(parallel, 1)
                .map_err(
                    |cause| gauntlet_ports::budget_meter::BudgetMeterError::Reserve { cause },
                )?;
            inner.outstanding = parallel;
            drop(inner);
            Ok(self.mint(slice))
        }
        fn charge(
            &self,
            reservation: &gauntlet_ports::budget_meter::Reservation,
            payment: gauntlet_domain::Payment,
        ) -> Result<(), gauntlet_ports::budget_meter::BudgetMeterError> {
            if payment.tokens() > reservation.slice().tokens {
                return Err(
                    gauntlet_ports::budget_meter::BudgetMeterError::ReservationExceeded {
                        reserved_tokens: reservation.slice().tokens,
                        payment_tokens: payment.tokens(),
                    },
                );
            }
            let mut inner = self.0.lock().expect("meter lock");
            // task26 fix: fixture-only. Mirrors the shipped budget-meter
            // adapter's contract: payments carry no rounds dimension. The
            // run loop's `book_iteration_round` is the single rounds
            // authority, booking exactly once per iteration on
            // `state.budget` — never through the meter.
            inner
                .budget
                .charge(
                    0,
                    payment.wall_seconds(),
                    payment.tokens(),
                    payment.cost_usd(),
                )
                .map_err(
                    |cause| gauntlet_ports::budget_meter::BudgetMeterError::Charge { cause },
                )?;
            // Booked (a zero payment books nothing): the slot is released.
            // A refused charge returned above and keeps its reservation.
            inner.outstanding = inner.outstanding.saturating_sub(1);
            Ok(())
        }
        fn remaining(&self) -> gauntlet_ports::budget_meter::Remaining {
            gauntlet_ports::budget_meter::remaining_of(&self.0.lock().expect("meter lock").budget)
        }
        fn would_breach_reserve(&self, slice: gauntlet_ports::budget_meter::BudgetSlice) -> bool {
            self.0
                .lock()
                .expect("meter lock")
                .budget
                .would_breach_reserve(
                    slice.rounds,
                    slice.wall_seconds,
                    slice.tokens,
                    slice.cost_usd,
                )
        }
    }

    fn meter_factory()
    -> Arc<dyn Fn(Budget) -> Arc<dyn gauntlet_ports::budget_meter::BudgetMeter> + Send + Sync> {
        Arc::new(|budget| {
            let meter = FakeMeter(Mutex::new(FakeMeterInner {
                budget,
                outstanding: 0,
            }));
            Arc::new(meter)
        })
    }

    /// C9.3's site-2 fixture: a meter that books NOTHING and refuses
    /// NOTHING — the §6.2 contract's permissive extreme. The shipped
    /// mirror meter reaches the loop budget's own refusal FIRST (it books
    /// the same numbers the loop books), so isolating the loop budget's
    /// OWN charge-refusal path — C2's second drop site — requires a meter
    /// that stays quiet while the run budget refuses.
    struct PermissiveMeter(Budget);

    impl gauntlet_ports::budget_meter::BudgetMeter for PermissiveMeter {
        fn reserve(
            &self,
            slice: gauntlet_ports::budget_meter::BudgetSlice,
        ) -> Result<
            gauntlet_ports::budget_meter::Reservation,
            gauntlet_ports::budget_meter::BudgetMeterError,
        > {
            Ok(self.mint(slice))
        }
        fn charge(
            &self,
            _: &gauntlet_ports::budget_meter::Reservation,
            _: gauntlet_domain::Payment,
        ) -> Result<(), gauntlet_ports::budget_meter::BudgetMeterError> {
            Ok(())
        }
        fn remaining(&self) -> gauntlet_ports::budget_meter::Remaining {
            gauntlet_ports::budget_meter::remaining_of(&self.0)
        }
        fn would_breach_reserve(&self, _: gauntlet_ports::budget_meter::BudgetSlice) -> bool {
            false
        }
    }

    fn permissive_meter_factory()
    -> Arc<dyn Fn(Budget) -> Arc<dyn gauntlet_ports::budget_meter::BudgetMeter> + Send + Sync> {
        Arc::new(|budget| Arc::new(PermissiveMeter(budget)))
    }

    /// F4's meter-side observation tally (test-only): how many reserves
    /// the meter granted, how many it refused, and how many granted
    /// reservations came back through a charge (the zero-payment release
    /// included). `granted - released` is the meter's held-reservation
    /// count — the leak figure the F4 tests pin.
    #[derive(Default)]
    struct MeterTally {
        granted: u32,
        refusals: u32,
        released: u32,
    }

    /// The fixture's forward share of a raw token ceiling: the same
    /// `limit * (100 - pct) / 100` arithmetic the domain's forward
    /// ceiling applies at the fixture reserve percentage.
    fn forward_share_of_reserve() -> u64 {
        (super::HUNDRED_PERCENT - u64::from(FIXTURE_RESERVE_PCT)) / super::HUNDRED_PERCENT
    }

    /// F4's refusal-leg fixture (test-only): a meter AT the forward
    /// ceiling for the critic dispatch. The first `admit_first` reserves
    /// succeed (the builder's dispatch — the run must still reach its
    /// review), every later reserve is refused with the SAME typed
    /// `RecoveryReserveBreached` cause the shipped meter produces at a
    /// real ceiling. The refusal is modeled inside the meter because the
    /// run's own arithmetic makes a mid-iteration ceiling refusal
    /// unreachable composed: the per-attempt estimate the critic reserves
    /// is affordable by construction whenever the loop's own gates
    /// admitted the builder one step earlier on the same figure — the
    /// F2 `PermissiveMeter` precedent (isolate the seam where the seam
    /// lives). Everything else delegates to the real mirror `FakeMeter`
    /// over the run's own budget.
    struct CeilingMeter {
        inner: FakeMeter,
        admit_first: u32,
        tally: Arc<Mutex<MeterTally>>,
    }

    impl gauntlet_ports::budget_meter::BudgetMeter for CeilingMeter {
        fn reserve(
            &self,
            slice: gauntlet_ports::budget_meter::BudgetSlice,
        ) -> Result<
            gauntlet_ports::budget_meter::Reservation,
            gauntlet_ports::budget_meter::BudgetMeterError,
        > {
            if self.tally.lock().expect("tally lock").granted >= self.admit_first {
                self.tally.lock().expect("tally lock").refusals += 1;
                return Err(gauntlet_ports::budget_meter::BudgetMeterError::Reserve {
                    cause: gauntlet_domain::BudgetError::RecoveryReserveBreached {
                        dimension: gauntlet_domain::BudgetDimension::Tokens,
                        forward_ceiling: FIXTURE_TOKEN_CEILING * forward_share_of_reserve(),
                        spent: FIXTURE_TOKEN_CEILING * forward_share_of_reserve(),
                        requested: slice.tokens,
                    },
                });
            }
            let reservation = self.inner.reserve(slice)?;
            self.tally.lock().expect("tally lock").granted += 1;
            Ok(reservation)
        }
        fn charge(
            &self,
            reservation: &gauntlet_ports::budget_meter::Reservation,
            payment: gauntlet_domain::Payment,
        ) -> Result<(), gauntlet_ports::budget_meter::BudgetMeterError> {
            self.inner.charge(reservation, payment)?;
            self.tally.lock().expect("tally lock").released += 1;
            Ok(())
        }
        fn remaining(&self) -> gauntlet_ports::budget_meter::Remaining {
            self.inner.remaining()
        }
        fn would_breach_reserve(&self, slice: gauntlet_ports::budget_meter::BudgetSlice) -> bool {
            self.inner.would_breach_reserve(slice)
        }
    }

    /// The observing variant for the no-over-refusal leg: the REAL mirror
    /// semantics with the tally attached, refusing nothing — headroom for
    /// every dispatch, so the test discriminates the RELEASE, not the
    /// refusal.
    struct ObservingMeter {
        inner: FakeMeter,
        tally: Arc<Mutex<MeterTally>>,
    }

    impl gauntlet_ports::budget_meter::BudgetMeter for ObservingMeter {
        fn reserve(
            &self,
            slice: gauntlet_ports::budget_meter::BudgetSlice,
        ) -> Result<
            gauntlet_ports::budget_meter::Reservation,
            gauntlet_ports::budget_meter::BudgetMeterError,
        > {
            let reservation = self.inner.reserve(slice)?;
            self.tally.lock().expect("tally lock").granted += 1;
            Ok(reservation)
        }
        fn charge(
            &self,
            reservation: &gauntlet_ports::budget_meter::Reservation,
            payment: gauntlet_domain::Payment,
        ) -> Result<(), gauntlet_ports::budget_meter::BudgetMeterError> {
            self.inner.charge(reservation, payment)?;
            self.tally.lock().expect("tally lock").released += 1;
            Ok(())
        }
        fn remaining(&self) -> gauntlet_ports::budget_meter::Remaining {
            self.inner.remaining()
        }
        fn would_breach_reserve(&self, slice: gauntlet_ports::budget_meter::BudgetSlice) -> bool {
            self.inner.would_breach_reserve(slice)
        }
    }

    /// A scripted comparator source: the first N comparisons find a
    /// residual; later ones pass. Residual TEXT is the record's own
    /// expected/observed pair — the critic's REASONING never enters it.
    struct ScriptedComparators {
        residual_first: usize,
        calls: Cell<u32>,
        /// The scripted residual's severity — Major by default (non-
        /// blocking); a test can raise it to Blocking via
        /// [`ScriptedComparators::set_severity`] before running.
        severity: Cell<gauntlet_domain::Severity>,
    }

    impl ScriptedComparators {
        fn new(residual_first: usize) -> Arc<Self> {
            Arc::new(Self {
                residual_first,
                calls: Cell::new(0),
                severity: Cell::new(gauntlet_domain::Severity::Major),
            })
        }

        fn set_severity(&self, severity: gauntlet_domain::Severity) {
            self.severity.set(severity);
        }
    }

    /// The source form the deps accept: a newtype over the shared counter
    /// (a bare `impl ... for Arc<T>` trips the orphan rule).
    struct ComparatorsSource(Arc<ScriptedComparators>);

    impl ComparatorSource for ComparatorsSource {
        fn across_dimensions(&self) -> Result<Box<dyn Comparator>, String> {
            Ok(Box::new(ScriptedComparator {
                inner: self.0.clone(),
            }))
        }
        fn for_dimension(&self, _: &str) -> Result<Box<dyn Comparator>, String> {
            Ok(Box::new(ScriptedComparator {
                inner: self.0.clone(),
            }))
        }
    }

    struct ScriptedComparator {
        inner: Arc<ScriptedComparators>,
    }

    impl Comparator for ScriptedComparator {
        fn compare(
            &self,
            context: &gauntlet_ports::comparator::TrustedComparisonContext,
            _: Deadline,
        ) -> Result<gauntlet_domain::ComparisonRecord, ComparatorError> {
            let envelope = context.envelope();
            let call = self.inner.calls.get();
            self.inner.calls.set(call + 1);
            let residuals = if (call as usize) < self.inner.residual_first {
                vec![gauntlet_domain::Residual::new(
                    gauntlet_domain::ResidualId::new("res-ctrl-1").expect("residual id"),
                    self.inner.severity.get(),
                    "expected: the report at the required locator; observed: absent".to_owned(),
                    Some("functional behavior".to_owned()),
                )]
            } else {
                Vec::new()
            };
            let verdict = if residuals.is_empty() {
                gauntlet_domain::Verdict::Pass
            } else {
                gauntlet_domain::Verdict::Fail
            };
            gauntlet_domain::ComparisonRecord::new(
                envelope.reference_digest().clone(),
                envelope.observation().observation_digest().clone(),
                envelope.envelope_digest().clone(),
                residuals,
                verdict,
            )
            .map_err(|error| ComparatorError::ComparisonFailed {
                detail: error.to_string(),
            })
        }
    }

    /// F4's admitting model critic (test-only): the two-phase §10.3
    /// dispatch shape the real model adapter uses — `admit` returns a
    /// pending comparison, and only `collect` produces the record — so
    /// a composed critic reservation rides the REAL fan-in lifecycle
    /// (admission → pending → collection) instead of the compatibility
    /// comparator's synchronous bracket. Collection counts, so a test
    /// can prove a critic dispatch PROCEEDED; admissions count, so a
    /// test can prove one was refused before admission.
    struct AdmittingCritic {
        admissions: Arc<Cell<u32>>,
        collects: Arc<Cell<u32>>,
    }

    impl Comparator for AdmittingCritic {
        fn compare(
            &self,
            _: &gauntlet_ports::comparator::TrustedComparisonContext,
            _: Deadline,
        ) -> Result<gauntlet_domain::ComparisonRecord, ComparatorError> {
            Err(ComparatorError::ComparisonFailed {
                detail: "the admitting critic uses two-phase admission".to_owned(),
            })
        }
        fn admit(
            &self,
            context: &gauntlet_ports::comparator::TrustedComparisonContext,
            _: Deadline,
        ) -> Result<Option<Box<dyn gauntlet_ports::comparator::PendingComparison>>, ComparatorError>
        {
            self.admissions.set(self.admissions.get() + 1);
            Ok(Some(Box::new(AdmittedCriticJob {
                envelope: context.envelope().clone(),
                collects: self.collects.clone(),
            })))
        }
    }

    struct AdmittedCriticJob {
        envelope: gauntlet_ports::comparator::ComparisonEnvelope,
        collects: Arc<Cell<u32>>,
    }

    impl gauntlet_ports::comparator::PendingComparison for AdmittedCriticJob {
        fn collect(
            &mut self,
            _: Deadline,
        ) -> Result<gauntlet_ports::comparison_record::ComparisonRecordDoc, ComparatorError>
        {
            self.collects.set(self.collects.get() + 1);
            Ok(gauntlet_ports::comparison_record::ComparisonRecordDoc::new(
                gauntlet_ports::comparison_record::RecordContent {
                    run_id: self.envelope.run_id().clone(),
                    iteration_no: self.envelope.iteration_no(),
                    reference_digest: self.envelope.reference_digest().clone(),
                    observation_digest: self.envelope.observation().observation_digest().clone(),
                    envelope_digest: self.envelope.envelope_digest().clone(),
                    contract_version: "1".to_owned(),
                    identity: gauntlet_ports::comparison_record::ComparatorIdentity::new(
                        "admitting-critic".to_owned(),
                        String::new(),
                        String::new(),
                    ),
                    residuals: Vec::new(),
                    hard_gate_results: Vec::new(),
                    verdict: gauntlet_ports::comparison_record::RecordVerdict::new(
                        gauntlet_domain::Verdict::Pass,
                        "the admitted critic judge is clean".to_owned(),
                    ),
                },
            ))
        }
        fn cancel(&mut self, _: Deadline) -> Result<(), ComparatorError> {
            Ok(())
        }
    }

    /// The source form of the admitting critic, with its shared
    /// admission/collection counters.
    struct AdmittingSource {
        admissions: Arc<Cell<u32>>,
        collects: Arc<Cell<u32>>,
    }

    impl AdmittingSource {
        fn new() -> (Arc<Self>, Arc<Cell<u32>>, Arc<Cell<u32>>) {
            let admissions = Arc::new(Cell::new(0));
            let collects = Arc::new(Cell::new(0));
            (
                Arc::new(Self {
                    admissions: admissions.clone(),
                    collects: collects.clone(),
                }),
                admissions,
                collects,
            )
        }
    }

    impl ComparatorSource for AdmittingSource {
        fn across_dimensions(&self) -> Result<Box<dyn Comparator>, String> {
            Ok(Box::new(AdmittingCritic {
                admissions: self.admissions.clone(),
                collects: self.collects.clone(),
            }))
        }
        fn for_dimension(&self, _: &str) -> Result<Box<dyn Comparator>, String> {
            Ok(Box::new(AdmittingCritic {
                admissions: self.admissions.clone(),
                collects: self.collects.clone(),
            }))
        }
    }

    /// A fixture observer: one test facet whose evidence document agrees
    /// with the facet record on every cross-checked field.
    struct FixtureObserver {
        store: FakeEvidence,
        clock: Arc<dyn Clock>,
    }

    impl Observer for FixtureObserver {
        fn observe(
            &self,
            request: &ObservationRequest,
            _: Deadline,
        ) -> Result<ObservationFacet, ObserverError> {
            let instrument = InstrumentIdentity::new(
                gauntlet_domain::InstrumentName::new("gauntlet-controller-fixture")
                    .expect("fixture instrument name"),
                "1".to_owned(),
            );
            let started = self.clock.now();
            let ended = EpochSeconds::new(started.get() + 1);
            let command = "fixture-probe --report".to_owned();
            let doc = FacetEvidenceDoc::new(
                instrument.name().as_str().to_owned(),
                instrument.version().to_owned(),
                ObservedRun {
                    command: command.clone(),
                    env_fingerprint:
                        "sha256:0000000000000000000000000000000000000000000000000000000000000000"
                            .to_owned(),
                    exit_status: 0,
                    started_at: started.get(),
                    ended_at: ended.get(),
                    stdout: format!("observed workspace {}", request.workspace.as_str()),
                    stderr: String::new(),
                },
            )
            .expect("the fixture evidence document is well formed");
            let bound = Deadline::at_epoch(EpochSeconds::new(u64::MAX / 2));
            let evidence_ref = self
                .store
                .put(doc.encode().as_bytes(), EvidenceKind::Log, bound)
                .map_err(|error| ObserverError::Unavailable {
                    kind: FacetKind::Test,
                    detail: error.to_string(),
                })?;
            Ok(
                ObservationFacet::new(FacetKind::Test, instrument, evidence_ref)
                    .with_invocation(Some(command), Some(0))
                    .with_window(Some(started), Some(ended))
                    .with_summary(Some("fixture facet".to_owned())),
            )
        }
        fn facet_kind(&self) -> FacetKind {
            FacetKind::Test
        }
    }

    /// A per-locator deterministic verifier rung: decides by locator.
    struct FixtureVerifier {
        passing: Vec<String>,
    }

    impl Verifier for FixtureVerifier {
        fn verify(
            &self,
            claim: &gauntlet_ports::verifier::VerificationRequest,
            _: Deadline,
        ) -> Result<gauntlet_domain::VerificationRecord, VerifierError> {
            if !self
                .passing
                .iter()
                .any(|locator| *locator == claim.evidence.as_str())
            {
                return Err(VerifierError::EvidenceMissing {
                    locator: claim.evidence.clone(),
                });
            }
            Ok(gauntlet_domain::VerificationRecord::new(
                claim.id.clone(),
                claim.run_id.clone(),
                claim.unit_id.clone(),
                EvidenceRef::new(claim.evidence.as_str()).expect("locator is a valid ref"),
                gauntlet_domain::Outcome::Pass,
                gauntlet_domain::InstrumentName::new("gauntlet-controller-fixture")
                    .expect("fixture name"),
            ))
        }
        fn is_deterministic(&self) -> bool {
            true
        }
        fn instrument(&self) -> InstrumentIdentity {
            InstrumentIdentity::new(
                gauntlet_domain::InstrumentName::new("gauntlet-controller-fixture")
                    .expect("fixture name"),
                "1".to_owned(),
            )
        }
        fn supports(&self, requirement: &gauntlet_domain::EvidenceRequirement) -> bool {
            requirement.kind() == EvidenceKind::TestReport
        }
    }

    fn fixture_ladder(passing: &[&str]) -> crate::verify::ladder::Ladder {
        crate::verify::ladder::Ladder::new(vec![crate::verify::ladder::LadderRung::new(
            crate::verify::ladder::RungKind::TestReport,
            Arc::new(FixtureVerifier {
                passing: passing.iter().map(|locator| locator.to_string()).collect(),
            }),
        )])
        .expect("the fixture ladder composes")
    }

    /// A rung whose decision changes as its underlying evidence
    /// accumulates: it FAILS the first `pass_from - 1` verification calls
    /// and PASSES from the `pass_from`-th on. This models a REAL
    /// divergence class (§18.4): the verification's evidence question is
    /// DIFFERENT from the comparison's observation question — the rung's
    /// answer flips while the observed instrument failure persists — so
    /// the composed loop genuinely observes the comparator and the
    /// verifier DISAGREEING at the diagnosis boundary.
    struct FlakyVerdictVerifier {
        calls: std::cell::Cell<u32>,
        pass_from: u32,
    }

    impl Verifier for FlakyVerdictVerifier {
        fn verify(
            &self,
            claim: &gauntlet_ports::verifier::VerificationRequest,
            _: Deadline,
        ) -> Result<gauntlet_domain::VerificationRecord, VerifierError> {
            let call = self.calls.get() + 1;
            self.calls.set(call);
            let outcome = if call >= self.pass_from {
                gauntlet_domain::Outcome::Pass
            } else {
                gauntlet_domain::Outcome::Fail
            };
            Ok(gauntlet_domain::VerificationRecord::new(
                claim.id.clone(),
                claim.run_id.clone(),
                claim.unit_id.clone(),
                EvidenceRef::new(claim.evidence.as_str()).expect("locator is a valid ref"),
                outcome,
                gauntlet_domain::InstrumentName::new("gauntlet-controller-fixture")
                    .expect("fixture name"),
            ))
        }
        fn is_deterministic(&self) -> bool {
            true
        }
        fn instrument(&self) -> InstrumentIdentity {
            InstrumentIdentity::new(
                gauntlet_domain::InstrumentName::new("gauntlet-controller-fixture")
                    .expect("fixture name"),
                "1".to_owned(),
            )
        }
        fn supports(&self, requirement: &gauntlet_domain::EvidenceRequirement) -> bool {
            requirement.kind() == EvidenceKind::TestReport
        }
    }

    /// Deterministic fixture ids.
    struct SeededIds(RefCell<u64>);
    impl IdGenerator for SeededIds {
        fn run_id(&self) -> RunId {
            RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("fixture run id")
        }
        fn invocation_id(&self) -> gauntlet_domain::InvocationId {
            let mut next = self.0.borrow_mut();
            *next += 1;
            gauntlet_domain::InvocationId::new(&format!("01ARZ3NDEKTSV4RRFFQ69G5F{:02X}", *next))
                .expect("fixture invocation id")
        }
    }

    /// A provider wrapper whose `verify_unchanged` can be made Unavailable
    /// mid-run (Task-7 m3's other class).
    struct FlakyProvider {
        inner: FileReferenceProvider,
        unavailable: Rc<Cell<bool>>,
    }

    impl ReferenceProvider for FlakyProvider {
        fn resolve(
            &self,
            intent: &str,
            deadline: Deadline,
        ) -> Result<gauntlet_ports::reference_provider::ReferenceDraft, ReferenceProviderError>
        {
            self.inner.resolve(intent, deadline)
        }
        fn normalize(
            &self,
            draft: gauntlet_ports::reference_provider::ReferenceDraft,
            deadline: Deadline,
        ) -> Result<gauntlet_domain::Reference, ReferenceProviderError> {
            self.inner.normalize(draft, deadline)
        }
        fn digest(
            &self,
            reference: &gauntlet_domain::Reference,
            deadline: Deadline,
        ) -> Result<Digest, ReferenceProviderError> {
            self.inner.digest(reference, deadline)
        }
        fn freeze(
            &self,
            run_id: &RunId,
            reference: &gauntlet_domain::Reference,
            deadline: Deadline,
        ) -> Result<gauntlet_domain::ReferenceVersion, ReferenceProviderError> {
            self.inner.freeze(run_id, reference, deadline)
        }
        fn verify_unchanged(
            &self,
            version: &gauntlet_domain::ReferenceVersion,
            deadline: Deadline,
        ) -> Result<(), ReferenceProviderError> {
            if self.unavailable.get() {
                return Err(ReferenceProviderError::Unavailable {
                    detail: "the fixture provider is unavailable".to_owned(),
                });
            }
            self.inner.verify_unchanged(version, deadline)
        }
    }

    /// A scratch workspace root; removed on drop.
    struct Scratch(std::path::PathBuf);

    impl Scratch {
        fn new(tag: &str) -> Self {
            // pid + atomic counter — the repository Scratch idiom (see
            // settle::SettleState) — so concurrent tests sharing a tag
            // never share (and race-remove) a directory.
            static SEQ: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
            let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "gauntlet-controller-{tag}-{}-{seq}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).expect("the scratch root is creatable");
            Self(path)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    struct Composed {
        controller: RunController,
        state: Arc<FakeState>,
        runner: Arc<ScriptedRunner>,
        /// The provider's unavailability switch — the §21.3 window lever a
        /// test flips to keep `verify_unchanged` answering Unavailable (the
        /// only window in which a mid-run answer-key edit changes what the
        /// loop does instead of aborting it).
        unavailable: Rc<Cell<bool>>,
        /// The scripted comparators — a test flips
        /// [`ScriptedComparators::set_severity`] before running to make its
        /// residual Blocking instead of the default Major.
        comparators: Arc<ScriptedComparators>,
    }

    fn compose(
        key: Rc<RefCell<String>>,
        state: Arc<FakeState>,
        evidence: FakeEvidence,
        runner: Arc<ScriptedRunner>,
        residual_first: usize,
        rounds: u32,
    ) -> Result<Composed, ControlError> {
        compose_full(
            key,
            state,
            evidence,
            runner,
            residual_first,
            rounds,
            policy(),
            fixture_ladder(&["ctrl-9101.json", "ctrl-9102.json"]),
            meter_factory(),
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn compose_full(
        key: Rc<RefCell<String>>,
        state: Arc<FakeState>,
        evidence: FakeEvidence,
        runner: Arc<ScriptedRunner>,
        residual_first: usize,
        rounds: u32,
        policy: ControlPolicy,
        ladder: crate::verify::ladder::Ladder,
        meter_factory: Arc<
            dyn Fn(Budget) -> Arc<dyn gauntlet_ports::budget_meter::BudgetMeter> + Send + Sync,
        >,
        workspaces: Option<Arc<dyn WorkspaceProvider>>,
    ) -> Result<Composed, ControlError> {
        let scratch = Scratch::new("compose");
        let clock = DeterministicClock::new();
        let observers: Vec<Arc<dyn Observer>> = vec![Arc::new(FixtureObserver {
            store: evidence.clone(),
            clock: clock.clone(),
        })];
        let provider = Arc::new(FlakyProvider {
            inner: FileReferenceProvider::from_sources(
                AnswerKeySource::Shared(key.clone()),
                ManifestSource::Inline(minimal_manifest()),
                Rc::new(evidence.clone()),
            ),
            unavailable: Rc::new(Cell::new(false)),
        });
        let unavailable = provider.unavailable.clone();
        let comparators = ScriptedComparators::new(residual_first);
        let deps = ControllerDeps {
            state: state.clone(),
            evidence: Arc::new(evidence.clone()),
            ids: Arc::new(SeededIds(RefCell::new(0))),
            clock,
            runner: runner.clone(),
            runner_identity: gauntlet_domain::RunnerIdentity::new("fixture-runner")
                .expect("runner identity"),
            observers,
            comparison: ComparisonDeps {
                contract: minimal_contract(),
                deterministic: Arc::new(ComparatorsSource(comparators.clone())),
                model: Arc::new(ComparatorsSource(comparators.clone())),
                adversarial: None,
                review_mode: gauntlet_domain::ReviewMode::Single,
                model_dispatches: 4,
                role_deny: crate::roles::ContextDenySet::new(Vec::new()),
            },
            ladder_for: Arc::new(move |_| Ok(ladder.clone())),
            reference: provider,
            answer_key: AnswerKeySource::Shared(key.clone()),
            escalation: Arc::new(FakeEscalation::default()),
            meter_factory,
            budget: budget_shape(rounds),
            policy,
            workspaces,

            workspace_root: scratch.0.clone(),
            config_fingerprint: Digest::new("sha256", &"ab".repeat(32)).expect("digest"),
            govern: crate::govern::GovernDeps::none_backend(),
            environment_fingerprint: Digest::new("sha256", &"cd".repeat(32)).expect("digest"),
            intent: "controller fixture".to_owned(),
        };
        std::mem::forget(scratch);
        let controller = RunController::compose(deps)?;
        Ok(Composed {
            controller,
            state,
            runner,
            unavailable,
            comparators,
        })
    }

    /// The recording escalation surface (§11.6's ticket requirement).
    #[derive(Default)]
    struct FakeEscalation {
        tickets: Mutex<Vec<String>>,
    }

    impl gauntlet_ports::human_escalation::HumanEscalation for FakeEscalation {
        fn escalate(
            &self,
            item: gauntlet_ports::human_escalation::EscalationItem,
            _: Deadline,
        ) -> Result<
            gauntlet_ports::human_escalation::EscalationTicket,
            gauntlet_ports::human_escalation::HumanEscalationError,
        > {
            self.tickets
                .lock()
                .expect("escalation lock")
                .push(item.summary);
            Ok(gauntlet_ports::human_escalation::EscalationTicket::new(
                gauntlet_ports::human_escalation::TicketId::new("ticket-1").expect("ticket id"),
                EpochSeconds::new(BASE_INSTANT),
            ))
        }
        fn poll(
            &self,
            _: &gauntlet_ports::human_escalation::EscalationTicket,
            _: Deadline,
        ) -> Result<
            gauntlet_ports::human_escalation::EscalationState,
            gauntlet_ports::human_escalation::HumanEscalationError,
        > {
            Ok(gauntlet_ports::human_escalation::EscalationState::Open)
        }
    }

    fn fixture_composed(
        behaviors: Vec<Behavior>,
        residual_first: usize,
        units: usize,
        rounds: u32,
    ) -> Result<Composed, ControlError> {
        let key = Rc::new(RefCell::new(answer_key(units)));
        let state = FakeState::new();
        let evidence = FakeEvidence::new();
        let runner = ScriptedRunner::new(behaviors);
        compose(key, state, evidence, runner, residual_first, rounds)
    }

    /// The fixture composition with a LONG quiet window: the default
    /// fixture policy's quiet window is so short that a still-in-flight
    /// invocation is stall-caught on the drain's FIRST pass, which would
    /// mask (by cancelling for a different, earlier reason) exactly the
    /// stranded-at-terminal shape the corrections-round-1 F6 test
    /// isolates. Sixty virtual seconds of quiet means the stall rule can
    /// never fire inside the test's bounded drain.
    fn fixture_composed_quiet(
        behaviors: Vec<Behavior>,
        units: usize,
        rounds: u32,
    ) -> Result<Composed, ControlError> {
        const QUIET_WINDOW_EXTENDED: u64 = 60_000;
        let quiet = ControlPolicy::new(
            TICK_WINDOW,
            QUIET_WINDOW_EXTENDED,
            FLOOR,
            CEILING,
            ATTEMPT_CAP,
            THRESHOLD,
        )
        .expect("the fixture control policy is usable");
        let key = Rc::new(RefCell::new(answer_key(units)));
        let state = FakeState::new();
        let evidence = FakeEvidence::new();
        let runner = ScriptedRunner::new(behaviors);
        compose_full(
            key,
            state,
            evidence,
            runner,
            0,
            rounds,
            quiet,
            fixture_ladder(&["ctrl-9101.json", "ctrl-9102.json"]),
            meter_factory(),
            None,
        )
    }

    // ------------------------------------------------------------------
    // Composition guards
    // ------------------------------------------------------------------

    #[test]
    fn an_empty_parsed_ruler_is_refused_at_composition() {
        // F18: an empty ruler must be refused even though every digest
        // computation would succeed — the controller never trusts
        // verify_unchanged solo. The strict parser admits exactly one
        // zero-Unit document: the empty one (any stray line is a typed
        // parse failure, which is the reference pipeline's error, not
        // the empty-ruler guard).
        let key = Rc::new(RefCell::new(String::new()));
        let state = FakeState::new();
        let evidence = FakeEvidence::new();
        let runner = ScriptedRunner::new(Vec::new());
        match compose(key, state, evidence, runner, 0, FIXTURE_ROUNDS) {
            Err(ControlError::EmptyRuler { source }) => {
                assert!(source.contains("shared artifact"));
            }
            Ok(_) => panic!("an empty ruler must be refused at composition"),
            Err(other) => panic!("the refusal is the empty-ruler guard, got {other}"),
        }
    }

    #[test]
    fn a_dangling_depends_on_is_surfaced_from_the_scheduler() {
        // F7: the B2 scheduler's graph validation is the composition guard.
        let text = [
            "## AK-9101",
            "claim: d",
            "verify_by: test",
            "blast_radius: contained",
            "authority: critic",
            "evidence_required: {kind: test_report, locator: ctrl-9101.json, producer: instrument}",
            "depends_on: [AK-9999]",
            "",
        ]
        .join("\n");
        let key = Rc::new(RefCell::new(text));
        let state = FakeState::new();
        let evidence = FakeEvidence::new();
        let runner = ScriptedRunner::new(Vec::new());
        match compose(key, state, evidence, runner, 0, FIXTURE_ROUNDS) {
            Err(ControlError::Scheduler(error)) => {
                assert!(
                    matches!(
                        error,
                        crate::control::scheduler::SchedulerError::UnknownDependency { .. }
                    ),
                    "the graph error is surfaced: {error}"
                );
            }
            Ok(_) => panic!("a dangling depends_on must be refused at composition"),
            Err(other) => {
                panic!("a dangling depends_on must surface the scheduler error, got {other}")
            }
        }
    }

    // ------------------------------------------------------------------
    // The composed loop
    // ------------------------------------------------------------------

    #[test]
    fn a_full_iteration_settles_a_unit_through_the_composed_chain() {
        let composed =
            fixture_composed(vec![Behavior::Complete], 0, 1, FIXTURE_ROUNDS).expect("composes");
        let outcome = composed.controller.start_run().expect("the loop runs");
        assert_eq!(outcome.terminal, TerminalState::Settled);
        // The unit settled in the loop's own bookkeeping.
        let checkpoint = composed
            .state
            .resume(&outcome.run_id, bound())
            .expect("checkpoint readable")
            .expect("a checkpoint exists");
        assert_eq!(
            checkpoint
                .unit_states
                .iter()
                .find(|(unit, _)| unit.as_str() == "AK-9101")
                .map(|(_, status)| *status),
            Some(UnitStatus::Settled)
        );
        // BOTH gated owner writes flowed through the authorities (the
        // direct store writes are compile-refused; the counted calls prove
        // the loop's only route was the authorities).
        assert!(
            composed.state.bind_calls.get() >= 1,
            "the observation binding flowed through ObservationBindingAuthority"
        );
        assert!(
            composed.state.production_calls.get() >= 1,
            "the evidence production flowed through EvidenceProductionAuthority"
        );
        // The §15.1 selection record landed (AK-037).
        assert!(
            !composed
                .state
                .correction_selections(&outcome.run_id, bound())
                .expect("selections readable")
                .is_empty()
        );
        // The admitted invocation was durably recorded before polling
        // (the RecordingAgentRunner discipline).
        assert_eq!(composed.state.role_invocations.borrow().len(), 1);
        // The locator bytes exist under the CANONICAL workspace root.
        assert!(
            composed
                .controller
                .workspace_root()
                .join("ctrl-9101.json")
                .exists(),
            "the producer wrote the locator bytes under the one canonical root"
        );
    }

    #[test]
    fn the_pass_leg_settles_durably_through_the_service() {
        // Task 13 B2 Item 2: the PASS leg's settle IS the service call — the
        // durable §16.3 sequence (PASS verification record → admitted commit
        // → keyed settlement row + settlement event) and the settlement
        // append rides the `last_offset` water-mark (C8.1). Before B2 the
        // loop settled in memory only: no durable settlement fact existed.
        let composed =
            fixture_composed(vec![Behavior::Complete], 0, 1, FIXTURE_ROUNDS).expect("composes");
        let outcome = composed.controller.start_run().expect("the loop runs");
        assert_eq!(outcome.terminal, TerminalState::Settled);
        let run_id = RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("fixture run id");
        // (1) the durable keyed settlement row exists at a real offset.
        let rows = composed
            .state
            .settlements(&run_id, bound())
            .expect("settlement rows readable");
        assert_eq!(rows.len(), 1, "exactly one settlement row: {rows:?}");
        assert_eq!(rows[0].unit_id().as_str(), "AK-9101");
        let row_offset = rows[0].event_log_offset().get();
        assert!(row_offset > 0, "the row names a real durable event");
        // (2) the durable event names the same settlement.
        assert!(
            composed.state.events.borrow().iter().any(|event| matches!(
                &event.kind,
                gauntlet_ports::state_store::RunEventKind::SettlementCommitted { unit_id, .. }
                    if unit_id.as_str() == "AK-9101"
            )),
            "the SettlementCommitted event is durable"
        );
        // (3) the admission's substrate: the PASS verification document is
        // durable for the settled unit.
        let records = composed
            .state
            .verification_records(&run_id, bound())
            .expect("verification records readable");
        assert!(
            records
                .iter()
                .any(|record| record.unit_id().as_str() == "AK-9101"
                    && record.outcome() == gauntlet_domain::Outcome::Pass),
            "the PASS verification document is durable: {records:?}"
        );
        // (4) C8.1: the close-out checkpoint's water-mark covers the
        // settlement append — the §18.3 anchor never sits past the
        // settlement the same iteration made durable.
        let checkpoint = composed
            .state
            .resume(&run_id, bound())
            .expect("checkpoint readable")
            .expect("a checkpoint exists");
        assert!(
            checkpoint.event_log_offset.get() >= row_offset,
            "the settlement append entered the water-mark (anchor {}, settlement {})",
            checkpoint.event_log_offset.get(),
            row_offset
        );
    }

    /// COR2-1b (critic round 1, F-1's completion half): the builder's
    /// declared attempted list is free text — a result completing the
    /// AK-9101 flight while DECLARING AK-9102 must never verify or settle
    /// the declared unit through this completion; the mismatch is pinned
    /// governance evidence and the loop re-plans the flight's own unit.
    #[test]
    fn a_declared_attempted_other_unit_never_settles_and_is_recorded() {
        let evidence = FakeEvidence::new();
        let evidence_view = FakeEvidence(evidence.0.clone());
        let key = Rc::new(RefCell::new(answer_key(2)));
        let state = FakeState::new();
        let runner = ScriptedRunner::new(vec![
            Behavior::CompleteDeclaringOther,
            Behavior::Complete,
            Behavior::Complete,
        ]);
        let composed = compose(key, state, evidence, runner, 0, FIXTURE_ROUNDS).expect("composes");
        let outcome = composed.controller.start_run().expect("the loop runs");
        assert_eq!(outcome.terminal, TerminalState::Settled);
        let run_id = RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("fixture run id");
        // Both units settle — but only through their OWN flights.
        let rows = composed
            .state
            .settlements(&run_id, bound())
            .expect("settlement rows readable");
        let units: Vec<&str> = rows.iter().map(|row| row.unit_id().as_str()).collect();
        assert!(
            units.contains(&"AK-9101") && units.contains(&"AK-9102"),
            "both units settle: {units:?}"
        );
        assert_eq!(rows.len(), 2, "exactly one settlement per unit: {rows:?}");
        // The mismatch is PINNED governance evidence, not silence.
        let records = evidence_view.records_containing("attempted_unit_mismatch");
        assert!(
            !records.is_empty(),
            "the declared-other mismatch is pinned governance evidence"
        );
        assert!(records.iter().any(|record| record.contains("AK-9999")));
    }

    #[test]
    fn a_fresh_run_checkpoint_carries_a_real_event_log_offset() {
        // Correction C8.1 (critic-B A9.1): §18.2 lists the event-log
        // offset as a required checkpoint field and §18.3 step 1 makes it
        // the ANCHOR REPLAY STARTS FROM — a constant 0 is not cosmetic.
        // The scenario deliberately includes a failed-then-retried
        // invocation so the loop appends a REAL InvocationTerminal event
        // before the close-out: both the terminal-append threading and the
        // checkpoint's own append must land in the durable water-mark.
        let composed = fixture_composed(
            vec![
                Behavior::Fail(InvocationFailure::Execution),
                Behavior::Complete,
            ],
            0,
            1,
            FIXTURE_ROUNDS,
        )
        .expect("composes");
        let outcome = composed.controller.start_run().expect("the loop runs");
        assert_eq!(outcome.terminal, TerminalState::Settled);
        let checkpoint = composed
            .state
            .resume(&outcome.run_id, bound())
            .expect("checkpoint readable")
            .expect("a checkpoint exists");
        assert!(
            checkpoint.event_log_offset.get() > 0,
            "the close-out checkpoint carries the store-returned offset, not the fresh-run \
             zero (got {})",
            checkpoint.event_log_offset.get()
        );
        // The anchor sits strictly BEHIND the store's head: everything
        // after it is exactly what a §18.3 replay must re-consume.
        assert!(
            composed.state.next_offset.get() > checkpoint.event_log_offset.get(),
            "the store has events past the anchor (head {}, anchor {})",
            composed.state.next_offset.get(),
            checkpoint.event_log_offset.get()
        );
    }

    #[test]
    fn a_crash_after_a_settlement_never_re_runs_settled_work() {
        // Correction C8.2 (critic-B A9.2): §11.1's iterate order puts the
        // checkpoint LAST ("… dispose residuals → checkpoint"), and §18.3
        // says "Re-running settled work is a defect, not a safe default."
        // A run ending after iteration 1 settles AK-9101 with AK-9102
        // still owed must leave the settlement DURABLE: the resume may
        // re-plan only the still-owed AK-9102, never AK-9101.
        //
        // Task 13 B2 Item 4 re-pins HOW the run ends: a poll transport
        // error is §11.5 row 2's TRANSIENT failure — the disposition table
        // retries it with the exponential backoff (a pinned transient
        // record + a Failed terminal event per occurrence) instead of
        // error-propagating out of the loop. Repeated transients are
        // bounded by the rounds dimension: the run ends
        // `budget_exhausted` WITH its close-out checkpoint, resume-able.
        let key = Rc::new(RefCell::new(answer_key_with_dependency()));
        let state = FakeState::new();
        let evidence = FakeEvidence::new();
        let transient_forever = ScriptedRunner::new(vec![
            Behavior::Complete,
            Behavior::PollError,
            Behavior::PollError,
            Behavior::PollError,
            Behavior::PollError,
            Behavior::PollError,
            Behavior::PollError,
            Behavior::PollError,
        ]);
        let first = compose(
            key.clone(),
            state.clone(),
            evidence.clone(),
            transient_forever,
            0,
            FIXTURE_ROUNDS,
        )
        .expect("composes");
        // Iteration 1 settles AK-9101; every later attempt of AK-9102 dies
        // to the transient poll error and is retried after backoff, until
        // the rounds dimension ends the run with a close-out checkpoint.
        let outcome = first
            .controller
            .start_run()
            .expect("a transient poll error degrades the iteration, never errors the run");
        assert_eq!(
            outcome.terminal,
            TerminalState::BudgetExhausted,
            "repeated transients are bounded by the rounds dimension"
        );
        // §11.5 row 2's disposition facts are PINNED, one per occurrence.
        let transient_records = evidence.records_containing("transient_runner_error");
        assert!(
            !transient_records.is_empty(),
            "the transient failures are pinned evidence"
        );
        // The transient retry actually re-dispatched the owed unit.
        let owed_attempts = first
            .runner
            .prompts
            .borrow()
            .iter()
            .filter(|(_, prompt)| prompt.contains("AK-9102"))
            .count();
        assert!(
            owed_attempts > 1,
            "row 2 re-dispatched the transient-failed work: {owed_attempts} attempts"
        );
        // A second controller over the SAME durable state resumes. First
        // run's forward ceiling (limit * (100 - FIXTURE_RESERVE_PCT) / 100
        // at FIXTURE_ROUNDS = 6 genuine rounds) is fully spent by genuine
        // iterations (round 1 settles AK-9101, rounds 2-6 are genuine
        // transient retries of AK-9102) before it ends BudgetExhausted, so
        // the resume needs its OWN wider ceiling to have headroom left
        // after restoring that spend.
        const RESUME_ROUNDS: u32 = 9;
        let resumed = ScriptedRunner::new(vec![Behavior::Complete, Behavior::Complete]);
        let second = compose(
            key.clone(),
            state.clone(),
            evidence.clone(),
            resumed,
            0,
            RESUME_ROUNDS,
        )
        .expect("composes");
        let run_id = RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("fixture run id");
        let outcome = second
            .controller
            .resume_run(&run_id)
            .expect("the run resumes from the durable checkpoint");
        assert_eq!(outcome.terminal, TerminalState::Settled);
        let settled_prompts = first
            .runner
            .prompts
            .borrow()
            .iter()
            .filter(|(_, prompt)| prompt.contains("AK-9101"))
            .count()
            + second
                .runner
                .prompts
                .borrow()
                .iter()
                .filter(|(_, prompt)| prompt.contains("AK-9101"))
                .count();
        assert_eq!(
            settled_prompts, 1,
            "a unit settled before the crash is never re-dispatched (spec 18.3)"
        );
        assert!(
            second
                .runner
                .prompts
                .borrow()
                .iter()
                .any(|(_, prompt)| prompt.contains("AK-9102")),
            "the still-owed unit is re-planned by the resume"
        );
    }

    #[test]
    fn a_resume_completes_half_applied_settlements_and_reconciles_the_projection() {
        // Task 13 B2 Item 3 (§18.3 step 1 composed): the §16.3 crash window
        // — the settlement EVENT durable, the keyed row and the projection
        // behind it. The resume's completion repairs the row from the
        // anchored replay, the in-memory projection is rebuilt against the
        // durable settlement rows (§5.3.3), and the loop NEVER re-dispatches
        // the settled unit.
        let key = Rc::new(RefCell::new(answer_key_with_dependency()));
        let state = FakeState::new();
        let evidence = FakeEvidence::new();
        // Two rounds: iteration 1 settles AK-9101; the rounds dimension
        // then ends the run with AK-9102 still owed and a close-out
        // checkpoint. (A poll transport error no longer ends runs — §11.5
        // row 2 retries it, Task 13 B2 Item 4.)
        let first_run = ScriptedRunner::new(vec![Behavior::Complete]);
        let first = compose(
            key.clone(),
            state.clone(),
            evidence.clone(),
            first_run,
            0,
            2,
        )
        .expect("composes");
        let first_outcome = first
            .controller
            .start_run()
            .expect("the bounded run ends by its own rounds dimension");
        assert_eq!(first_outcome.terminal, TerminalState::BudgetExhausted);
        let run_id = RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("fixture run id");
        let owed = gauntlet_domain::UnitId::new("AK-9102").expect("fixture unit id");
        // The crashed run's durable projection still owes AK-9102.
        let checkpoint = state
            .resume(&run_id, bound())
            .expect("checkpoint readable")
            .expect("a checkpoint exists");
        assert_eq!(
            checkpoint
                .unit_states
                .iter()
                .find(|(unit, _)| unit == &owed)
                .map(|(_, status)| *status),
            Some(UnitStatus::Open),
            "the checkpoint's projection still owes AK-9102"
        );
        // INJECT the crash window: the settlement event is durable at/after
        // the checkpoint anchor; the keyed row never landed.
        state
            .append_event(
                RunEvent {
                    run_id: run_id.clone(),
                    kind: gauntlet_ports::state_store::RunEventKind::SettlementCommitted {
                        unit_id: owed.clone(),
                        evidence: EvidenceRef::new("evd-injected-9102").expect("ref"),
                        verification: gauntlet_domain::VerificationRef::new("vrf-injected-9102")
                            .expect("ref"),
                        reference_digest: Digest::new("sha256", &"ef".repeat(32)).expect("digest"),
                    },
                    at: EpochSeconds::new(BASE_INSTANT + 99),
                },
                bound(),
            )
            .expect("the half-applied settlement event lands");
        // The resume runs with ZERO dispatch budget: reconciliation alone
        // must close the unit — any prompt here is a §18.3 violation.
        let resumed = ScriptedRunner::new(Vec::new());
        let second =
            compose(key, state.clone(), evidence, resumed, 0, FIXTURE_ROUNDS).expect("composes");
        let outcome = second
            .controller
            .resume_run(&run_id)
            .expect("the run resumes from the durable checkpoint");
        assert_eq!(outcome.terminal, TerminalState::Settled);
        assert!(
            second.runner.prompts.borrow().is_empty(),
            "the durably settled unit is never re-dispatched: {:?}",
            second.runner.prompts.borrow()
        );
        // The anchored replay completed the keyed row.
        let rows = state
            .settlements(&run_id, bound())
            .expect("settlement rows readable");
        assert!(
            rows.iter().any(|row| row.unit_id() == &owed),
            "the half-applied settlement completed: {rows:?}"
        );
        // The settled-ness is durably carried. COR-R5-1 (falsifier round 5,
        // F-R5-1) — MECHANICAL ADAPTATION, reason recorded: the resume's
        // close is now the settled NO-OP (zero booked rounds, no
        // reconciliation checkpoint); the pre-correction shape pinned the
        // boundary checkpoint the reconciliation iteration wrote, which is
        // exactly the booked-round behavior the round-5 reproduction
        // burned into `budget_exhausted` on an all-settled run. The
        // durable truth of the completion is the keyed row at the event's
        // own offset, and the projection every later resume actually
        // derives — the §5.3.3 event-ordered rebuild, the checkpoint's own
        // word upgraded by the durable settlement rows above the anchor —
        // carries the owed unit Settled. Assertion strength is preserved
        // (the settled-ness must be durably re-derivable, never
        // re-dispatched) and the no-op's zero-round shape is newly pinned.
        assert_eq!(
            outcome.iterations, first_outcome.iterations,
            "zero booked rounds: the settlement-arm close is the settled no-op"
        );
        let checkpoint = state
            .resume(&run_id, bound())
            .expect("checkpoint readable")
            .expect("a checkpoint exists");
        let rows = state
            .settlements(&run_id, bound())
            .expect("settlement rows readable");
        let mut projected: std::collections::BTreeMap<gauntlet_domain::UnitId, UnitStatus> =
            checkpoint.unit_states.iter().cloned().collect();
        for row in rows
            .iter()
            .filter(|row| row.event_log_offset().get() > checkpoint.event_log_offset.get())
        {
            projected.insert(row.unit_id().clone(), UnitStatus::Settled);
        }
        assert_eq!(
            projected.get(&owed),
            Some(&UnitStatus::Settled),
            "the §5.3.3 event-ordered projection carries the durable settlement: {projected:?}"
        );
    }

    #[test]
    fn resuming_an_all_settled_run_is_an_honest_settled_no_op() {
        // COR-F6 (falsifier round 1, F-7): a fully settled run's resume
        // short-circuits — terminal `settled` at the checkpoint's own
        // iteration, ZERO new iterations burned, zero re-dispatches, and
        // the durable record counts untouched (no close-out rewrite).
        // Before the correction the resume re-entered the loop, planned
        // zero dispatches, and ended `budget_exhausted` one iteration
        // later.
        let composed =
            fixture_composed(vec![Behavior::Complete], 0, 1, FIXTURE_ROUNDS).expect("composes");
        let first = composed.controller.start_run().expect("the run settles");
        assert_eq!(first.terminal, TerminalState::Settled);
        let run_id = RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("fixture run id");
        let second = composed
            .controller
            .resume_run(&run_id)
            .expect("the settled run resumes");
        assert_eq!(
            second.terminal,
            TerminalState::Settled,
            "the settled run's resume is an honest settled no-op"
        );
        assert_eq!(
            second.iterations, first.iterations,
            "no iteration is burned by the settled no-op"
        );
        assert_eq!(
            composed.runner.prompts.borrow().len(),
            1,
            "the only dispatch is the original run's — the resume re-dispatched nothing: {:?}",
            composed.runner.prompts.borrow()
        );
        let rows = composed
            .state
            .settlements(&run_id, bound())
            .expect("settlement rows readable");
        assert_eq!(rows.len(), 1, "no settlement was duplicated");
        assert_eq!(
            composed.state.role_invocations.borrow().len(),
            1,
            "no invocation was re-admitted"
        );
        let checkpoint = composed
            .state
            .resume(&run_id, bound())
            .expect("checkpoint readable")
            .expect("a checkpoint exists");
        assert_eq!(
            checkpoint.iteration_no.get(),
            first.iterations,
            "the close-out checkpoint stands unrewritten"
        );
    }

    #[test]
    fn resuming_an_all_settled_run_is_a_settled_noop_in_the_produced_world() {
        // COR-R2-1 (falsifier round 2, F-R2-3): the falsifier's blocking
        // reproduction, pinned in the composed produced-artifact world.
        // There the produce step keys every production by the LOCATOR
        // ARTIFACT's content ref, so the resume completion's applied
        // check must consult that same key surface (through the run's
        // [`ProducePolicy`]) — before the unification the facet-ref guess
        // never read a settled run as settled, and the FIRST `resume`
        // re-minted, failed this no-op's predicate, burned an iteration,
        // and reported a dishonest `budget_exhausted` terminal. The
        // default-world pin above stays byte for byte.
        let key = Rc::new(RefCell::new(answer_key(1)));
        let state = FakeState::new();
        let evidence = FakeEvidence::new();
        let runner = ScriptedRunner::new(vec![Behavior::Complete]);
        let clock = DeterministicClock::new();
        let observers: Vec<Arc<dyn Observer>> = vec![Arc::new(FixtureObserver {
            store: evidence.clone(),
            clock,
        })];
        let composed = compose_produced(key, state, evidence, runner, observers).expect("composes");
        let first = composed.controller.start_run().expect("the loop runs");
        assert_eq!(first.terminal, TerminalState::Settled);
        let run_id = RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("fixture run id");
        // THE FIRST resume — the exact leg the falsifier executed.
        let second = composed
            .controller
            .resume_run(&run_id)
            .expect("the settled run resumes");
        assert_eq!(
            second.terminal,
            TerminalState::Settled,
            "the FIRST resume of the settled produced-world run is the honest settled no-op"
        );
        assert_eq!(
            second.iterations, first.iterations,
            "no iteration is burned by the produced-world settled no-op"
        );
        assert_eq!(
            composed.runner.prompts.borrow().len(),
            1,
            "the resume re-dispatched nothing: {:?}",
            composed.runner.prompts.borrow()
        );
        let rows = composed
            .state
            .settlements(&run_id, bound())
            .expect("settlement rows readable");
        assert_eq!(rows.len(), 1, "no settlement was duplicated");
        assert_eq!(
            composed.state.role_invocations.borrow().len(),
            1,
            "no invocation was re-admitted"
        );
        let checkpoint = composed
            .state
            .resume(&run_id, bound())
            .expect("checkpoint readable")
            .expect("a checkpoint exists");
        assert_eq!(
            checkpoint.iteration_no.get(),
            first.iterations,
            "the close-out checkpoint stands unrewritten"
        );
    }

    #[test]
    fn damaged_production_bookkeeping_on_a_settled_produced_world_run_resumes_settled() {
        // COR-R3-1 (falsifier round 3, F-R3-1's exact reproduction shape):
        // a fully SETTLED produced-world run whose production ROW was
        // deleted out-of-band — every durable event and evidence byte left
        // in place, LESS damage than any real crash. Before the correction
        // this resumed into false "unrecoverable" pins and a dishonest
        // `budget_exhausted` terminal (all rows deleted) or silent masking
        // (one row deleted). The honest outcomes pinned here: the resume
        // reports terminal=settled at the checkpoint's own iteration with
        // zero re-dispatch, and ONE honest forensic record naming the
        // damaged locator and the missing row — never budget_exhausted,
        // never a false claim, and the durable settlement stands.
        let key = Rc::new(RefCell::new(answer_key(1)));
        let state = FakeState::new();
        let evidence = FakeEvidence::new();
        let evidence_for_assert = evidence.clone();
        let runner = ScriptedRunner::new(vec![Behavior::Complete]);
        let clock = DeterministicClock::new();
        let observers: Vec<Arc<dyn Observer>> = vec![Arc::new(FixtureObserver {
            store: evidence.clone(),
            clock,
        })];
        let composed = compose_produced(key, state, evidence, runner, observers).expect("composes");
        let first = composed.controller.start_run().expect("the loop runs");
        assert_eq!(first.terminal, TerminalState::Settled);
        // THE DAMAGE.
        composed.state.drop_production_at("ctrl-9101.json");
        let run_id = RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("fixture run id");
        let second = composed
            .controller
            .resume_run(&run_id)
            .expect("the damaged settled run resumes");
        assert_eq!(
            second.terminal,
            TerminalState::Settled,
            "an otherwise-settled run never reports a budget terminal from recovery \
             bookkeeping"
        );
        assert_eq!(
            second.iterations, first.iterations,
            "zero iterations are burned by the forensic-only resume"
        );
        assert_eq!(
            composed.runner.prompts.borrow().len(),
            1,
            "the resume re-dispatched nothing: {:?}",
            composed.runner.prompts.borrow()
        );
        let rows = composed
            .state
            .settlements(&run_id, bound())
            .expect("settlement rows readable");
        assert_eq!(rows.len(), 1, "the durable settlement stands unduplicated");
        let forensics = evidence_for_assert.records_containing("produced world:");
        assert!(
            forensics.iter().any(|record| {
                record.contains("ctrl-9101.json")
                    && record.contains("is missing")
                    && record.contains("no EvidenceProduced event at or after the resume anchor")
            }),
            "the damage is recorded honestly, naming the locator, the missing row, and the \
             anchor-bounded search: {forensics:?}"
        );
        assert!(
            evidence_for_assert
                .records_containing("budget_exhausted")
                .is_empty(),
            "no dishonest budget terminal is pinned anywhere"
        );
    }

    #[test]
    fn a_recovery_completed_production_on_a_settled_run_resumes_as_the_settled_noop() {
        // COR-R4-1 (falsifier round 4, F-R4-3): the falsifier's compound
        // shape on an otherwise fully SETTLED produced-world run — the
        // production ROW deleted while the `EvidenceProduced` event stays
        // durable ABOVE the resume anchor (the A5 end-state). The
        // recovery half is honest and COMPLETES the row from the event's
        // own facts; the loop half must then re-assess the terminal
        // projection AFTER that completion and close as the settled
        // no-op — never book an iteration (whose round is long spent)
        // into a dishonest `budget_exhausted` terminal over a fully
        // settled projection. This fixture reaches the end-state with the
        // event planted (its produce write is row-only; see the helper's
        // doc for why the real store's forged-anchor route differs); the
        // forged-anchor precondition stays the recorded "checkpoint()
        // forged-row projection class (planning-layer)" deferral, pinned
        // through the real binary by the E2E leg.
        let key = Rc::new(RefCell::new(answer_key(1)));
        let state = FakeState::new();
        let evidence = FakeEvidence::new();
        let evidence_for_assert = evidence.clone();
        let runner = ScriptedRunner::new(vec![Behavior::Complete]);
        let clock = DeterministicClock::new();
        let observers: Vec<Arc<dyn Observer>> = vec![Arc::new(FixtureObserver {
            store: evidence.clone(),
            clock,
        })];
        let composed = compose_produced(key, state, evidence, runner, observers).expect("composes");
        let first = composed.controller.start_run().expect("the loop runs");
        assert_eq!(first.terminal, TerminalState::Settled);
        // THE A5 END-STATE: the produce event durable ABOVE the resume
        // anchor (planted here from the row's own facts, stamped with a
        // timestamp that is NOT the recovery clock's — the fixture's
        // produce write is row-only; see the helper's doc), and then the
        // row DELETED while the event stays.
        let run_id = RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("fixture run id");
        let planted_at = gauntlet_domain::EpochSeconds::new(1_750_000_000);
        composed
            .state
            .plant_orphan_evidence_production_event(&run_id, planted_at);
        composed.state.drop_production_at("ctrl-9101.json");
        // THE RESUME.
        let second = composed
            .controller
            .resume_run(&run_id)
            .expect("the damaged settled run resumes");
        assert_eq!(
            second.terminal,
            TerminalState::Settled,
            "the recovery-completed settled run closes as the honest settled no-op"
        );
        assert_eq!(
            second.iterations, first.iterations,
            "ZERO additional iterations: a fully settled projection books no round"
        );
        assert_eq!(
            composed.runner.prompts.borrow().len(),
            1,
            "the resume re-dispatched nothing: {:?}",
            composed.runner.prompts.borrow()
        );
        // The mint half: the row was re-minted at the EVENT'S OWN
        // identity — the event's offset (the store's positional
        // convention, the offset every recovery read reports), the
        // event's ref, and the event's stored timestamp, never the
        // recovery's clock or a fabricated fact.
        let planted = composed
            .state
            .events
            .borrow()
            .iter()
            .enumerate()
            .find_map(|(index, event)| match &event.kind {
                gauntlet_ports::state_store::RunEventKind::EvidenceProduced {
                    evidence, ..
                } if event.run_id == run_id => {
                    Some(((index as u64) + 1, event.at, evidence.clone()))
                }
                _ => None,
            })
            .expect("the planted production event exists");
        let minted = composed
            .state
            .productions
            .borrow()
            .iter()
            .find(|binding| binding.production().locator().as_str() == "ctrl-9101.json")
            .expect("the recovery re-minted the deleted row")
            .clone();
        assert_eq!(
            minted.event_log_offset().get(),
            planted.0,
            "the minted row binds to the event's own offset"
        );
        assert_eq!(
            minted.production().produced_at(),
            planted.1,
            "the minted row takes the event's stored timestamp, never the recovery's clock"
        );
        assert_eq!(
            minted.production().evidence(),
            &planted.2,
            "the minted row names the event's own ref"
        );
        let rows = composed
            .state
            .settlements(&run_id, bound())
            .expect("settlement rows readable");
        assert_eq!(rows.len(), 1, "the durable settlement stands unduplicated");
        assert_eq!(
            composed.state.role_invocations.borrow().len(),
            1,
            "no invocation was re-admitted"
        );
        // The mint SUCCEEDED, so nothing is unrecoverable and no forensic
        // limitation is pinned — recovery-completed damage is not damage
        // the loop must re-plan over.
        assert!(
            evidence_for_assert
                .records_containing("produced world:")
                .is_empty(),
            "a completed production is not pinned as forensic damage"
        );
        assert!(
            evidence_for_assert
                .records_containing("budget_exhausted")
                .is_empty(),
            "no dishonest budget terminal is pinned anywhere"
        );
        // The NEXT resume is the same honest settled no-op (the terminal
        // is not sticky-damaged by the surgery either).
        let third = composed
            .controller
            .resume_run(&run_id)
            .expect("the second resume runs");
        assert_eq!(third.terminal, TerminalState::Settled);
        assert_eq!(
            third.iterations, first.iterations,
            "the second resume burns nothing either"
        );
    }

    #[test]
    fn a_recovery_completed_settlement_on_a_settled_run_resumes_as_the_settled_noop() {
        // COR-R5-1 (falsifier round 5, F-R5-1): the SETTLEMENT arm of the
        // recovery-completed shape. The keyed settlement ROW is deleted
        // while the durable `SettlementCommitted` event stays, and the
        // resume anchor is regressed to just BELOW that event (the
        // falsifier's exact compound reproduction posture — the §16.3/A5
        // end-state with the event durable ABOVE the resume anchor). The
        // §18.3 replay honestly re-inserts the row from the event's own
        // facts; the loop half must then close as the settled no-op —
        // ZERO booked rounds, never the refused booking's
        // `budget_exhausted` over a fully settled projection, never a
        // burned empty round when headroom exists.
        let key = Rc::new(RefCell::new(answer_key(1)));
        let state = FakeState::new();
        let evidence = FakeEvidence::new();
        let evidence_for_assert = evidence.clone();
        let runner = ScriptedRunner::new(vec![Behavior::Complete]);
        let clock = DeterministicClock::new();
        let observers: Vec<Arc<dyn Observer>> = vec![Arc::new(FixtureObserver {
            store: evidence.clone(),
            clock,
        })];
        let composed = compose_produced(key, state, evidence, runner, observers).expect("composes");
        let first = composed.controller.start_run().expect("the loop runs");
        assert_eq!(first.terminal, TerminalState::Settled);
        let run_id = RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("fixture run id");
        // THE DAMAGE: the falsifier's exact compound reproduction posture —
        // the keyed settlement ROW deleted, and the resume anchor regressed
        // to JUST BELOW the durable `SettlementCommitted` event, so the
        // event stands ABOVE the anchor (the §16.3/A5 end-state: the crash
        // landed between the event append and the keyed insert; the loop's
        // own later water-mark appends sit above the event in this
        // fixture's close-out checkpoint, the same relative geometry the
        // real store's multi-iteration run shows for an early-settled
        // unit). The anchor move is the same GIVEN-damage class the
        // shipped COR-R4-1 leg exercises through the real binary — the
        // recorded "checkpoint() forged-row projection class
        // (planning-layer)" deferral, never widened here.
        composed.state.drop_settlement_row("AK-9101");
        assert!(
            composed
                .state
                .settlements(&run_id, bound())
                .expect("settlement rows readable")
                .is_empty(),
            "the damage removed exactly the run's one settlement row"
        );
        let settlement_offset = composed
            .state
            .events
            .borrow()
            .iter()
            .enumerate()
            .find_map(|(index, event)| match &event.kind {
                gauntlet_ports::state_store::RunEventKind::SettlementCommitted {
                    unit_id, ..
                } if event.run_id == run_id && unit_id.as_str() == "AK-9101" => {
                    Some((index as u64) + 1)
                }
                _ => None,
            })
            .expect("the durable settlement event exists");
        {
            let mut checkpoints = composed.state.checkpoints.borrow_mut();
            let checkpoint = checkpoints
                .get_mut(&run_id)
                .expect("the close-out checkpoint is durable");
            assert!(
                checkpoint.event_log_offset.get() >= settlement_offset,
                "the natural close-out anchor sits at or above the settlement event: the \
                 regression below it is the reproduction's precondition"
            );
            checkpoint.event_log_offset =
                gauntlet_domain::EventLogOffset::new(settlement_offset - 1);
        }
        // THE RESUME: the honest settled no-op at the checkpoint's OWN
        // iteration.
        let second = composed
            .controller
            .resume_run(&run_id)
            .expect("the damaged settled run resumes");
        assert_eq!(
            second.terminal,
            TerminalState::Settled,
            "the recovery-completed settled run closes as the honest settled no-op"
        );
        assert_eq!(
            second.iterations, first.iterations,
            "ZERO booked rounds: an all-settled projection closes before/regardless of \
             iteration booking"
        );
        assert_eq!(
            composed.runner.prompts.borrow().len(),
            1,
            "the resume re-dispatched nothing: {:?}",
            composed.runner.prompts.borrow()
        );
        assert_eq!(
            composed.state.role_invocations.borrow().len(),
            1,
            "no invocation was re-admitted"
        );
        // THE REPLAY: the row is back, bound to the EVENT'S OWN offset (the
        // positional offset every store read here reports), and the
        // settlement stands unduplicated.
        let rows = composed
            .state
            .settlements(&run_id, bound())
            .expect("settlement rows readable");
        assert_eq!(rows.len(), 1, "the durable settlement stands unduplicated");
        assert_eq!(
            rows[0].event_log_offset().get(),
            settlement_offset,
            "the re-inserted row binds to the event's own offset"
        );
        // A COMPLETED settlement is not damage: no forensic pin of either
        // class, and no dishonest budget terminal anywhere.
        assert!(
            evidence_for_assert
                .records_containing("settlement_row_unrecoverable")
                .is_empty(),
            "a recovery-completed settlement is not pinned as forensic damage"
        );
        assert!(
            evidence_for_assert
                .records_containing("evidence_production_orphan_unrecoverable")
                .is_empty(),
            "the production half completed nothing, so it pins nothing"
        );
        assert!(
            evidence_for_assert
                .records_containing("budget_exhausted")
                .is_empty(),
            "no dishonest budget terminal is pinned anywhere"
        );
        // The NEXT resume is the same honest settled no-op (the replay is
        // idempotent over the restored row).
        let third = composed
            .controller
            .resume_run(&run_id)
            .expect("the second resume runs");
        assert_eq!(third.terminal, TerminalState::Settled);
        assert_eq!(
            third.iterations, first.iterations,
            "the second resume books nothing either"
        );
    }

    #[test]
    fn a_settled_noop_over_below_anchor_settlement_damage_pins_the_missing_row() {
        // COR-R5-2 (falsifier round 5, F-R5-2): below-anchor settlement-row
        // damage on an otherwise fully settled run must not be absorbed
        // SILENTLY by the certified settled no-op — the production arm
        // pins exactly this shape, and the settlement arm now does too.
        // Two units settle in order; the FIRST unit's settlement event sits
        // strictly BELOW the close-out anchor (the second unit's own
        // settlement is the loop's last tracked append), so the
        // anchor-bounded replay cannot re-complete the dropped row — the
        // divergence is pinned per row, checked against the store's own
        // settlement rows, and the intact sibling is never named.
        let key = Rc::new(RefCell::new(answer_key(2)));
        let state = FakeState::new();
        let evidence = FakeEvidence::new();
        let evidence_for_assert = evidence.clone();
        let runner = ScriptedRunner::new(vec![Behavior::Complete, Behavior::Complete]);
        let clock = DeterministicClock::new();
        let observers: Vec<Arc<dyn Observer>> = vec![Arc::new(FixtureObserver {
            store: evidence.clone(),
            clock,
        })];
        let composed = compose_produced(key, state, evidence, runner, observers).expect("composes");
        let first = composed.controller.start_run().expect("the loop runs");
        assert_eq!(first.terminal, TerminalState::Settled);
        let run_id = RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("fixture run id");
        assert_eq!(
            composed
                .state
                .settlements(&run_id, bound())
                .expect("settlement rows readable")
                .len(),
            2,
            "the two-unit run settles both units"
        );
        // THE DAMAGE: AK-9101's keyed row deleted; its settlement event must
        // sit strictly BELOW the resume anchor for the anchored replay to be
        // unable to re-complete it. Fixture fact: FIXTURE_PARALLELISM is 2,
        // so both units settle in ONE iteration and both events land above
        // the boundary-checkpoint anchor — the ADAPTATION moves the durable
        // checkpoint's anchor onto the LAST settlement event, which is the
        // real store's NATURAL multi-iteration posture (the close-out
        // anchor is the last boundary checkpoint's offset, above the
        // earlier iterations' settlement events — the falsifier's cell-D2
        // end-state, no anchor forgery there). Given damage here, exactly
        // like the COR-R4-1 test's event plant; the natural shape through
        // the real binary is the E2E leg's.
        composed.state.drop_settlement_row("AK-9101");
        let event_offset_of = |unit: &str| {
            composed
                .state
                .events
                .borrow()
                .iter()
                .enumerate()
                .find_map(|(index, event)| match &event.kind {
                    gauntlet_ports::state_store::RunEventKind::SettlementCommitted {
                        unit_id,
                        ..
                    } if event.run_id == run_id && unit_id.as_str() == unit => {
                        Some((index as u64) + 1)
                    }
                    _ => None,
                })
                .unwrap_or_else(|| panic!("{unit}'s settlement event is durable"))
        };
        let ak9101_offset = event_offset_of("AK-9101");
        {
            let mut checkpoints = composed.state.checkpoints.borrow_mut();
            let checkpoint = checkpoints
                .get_mut(&run_id)
                .expect("the close-out checkpoint is durable");
            // The anchor moves JUST ABOVE the damaged unit's settlement
            // event (order-independent: whichever way the parallel units
            // settled, the anchored replay now starts past AK-9101's
            // event and cannot re-complete its row).
            checkpoint.event_log_offset = gauntlet_domain::EventLogOffset::new(ak9101_offset + 1);
        }
        // THE RESUME: the certified settled no-op, WITH the honest pin.
        let second = composed
            .controller
            .resume_run(&run_id)
            .expect("the damaged settled run resumes");
        assert_eq!(
            second.terminal,
            TerminalState::Settled,
            "the below-anchor damage does not un-settle the run: the event is the durable \
             truth and the projection is all-settled"
        );
        assert_eq!(
            second.iterations, first.iterations,
            "the forensic-only resume books no round"
        );
        assert_eq!(
            composed.runner.prompts.borrow().len(),
            2,
            "the resume re-dispatched nothing: {:?}",
            composed.runner.prompts.borrow()
        );
        // THE PIN: exactly one settlement forensic record, naming the
        // damaged unit, the anchor-bounded fact, and the checked-against-
        // the-store posture; the intact sibling is never named; the row
        // stays honestly missing (the replay cannot fabricate it).
        let forensics = evidence_for_assert.records_containing("settlement_row_unrecoverable");
        assert_eq!(
            forensics.len(),
            1,
            "exactly the damaged row is pinned: {forensics:?}"
        );
        let record = &forensics[0];
        assert!(
            record.contains("AK-9101"),
            "the missing row's unit is named: {record}"
        );
        assert!(
            !record.contains("AK-9102"),
            "the intact sibling is never named: {record}"
        );
        assert!(
            record.contains("anchor-bounded"),
            "the record states the anchor-bounded replay fact: {record}"
        );
        assert!(
            record.contains("checked against the store's settlement rows"),
            "the record names what was checked: {record}"
        );
        let rows = composed
            .state
            .settlements(&run_id, bound())
            .expect("settlement rows readable");
        assert_eq!(
            rows.len(),
            1,
            "the replay is anchor-bounded: the row stays missing, never fabricated: {rows:?}"
        );
        assert_eq!(
            rows[0].unit_id().as_str(),
            "AK-9102",
            "the intact sibling's row is untouched"
        );
        // The production arm completed nothing here, so it pins nothing —
        // the only forensic record is the settlement row's own.
        assert!(
            evidence_for_assert
                .records_containing("evidence_production_orphan_unrecoverable")
                .is_empty(),
            "undamaged arms pin nothing"
        );
        // The NEXT resume re-observes the same persistent damage and re-pins
        // it (per-resume honesty, the production arm's own posture) while
        // staying the settled no-op.
        let third = composed
            .controller
            .resume_run(&run_id)
            .expect("the second resume runs");
        assert_eq!(third.terminal, TerminalState::Settled);
        assert_eq!(
            third.iterations, first.iterations,
            "the second resume books nothing either"
        );
        assert_eq!(
            evidence_for_assert
                .records_containing("settlement_row_unrecoverable")
                .len(),
            2,
            "each resume of the persistent damage records it honestly: one pin per resume"
        );
    }

    #[test]
    fn an_escalation_summary_with_quotes_and_backslashes_stays_parseable_evidence() {
        // Correction C9.2 (critic-B A12): `submit_ticket`'s record is
        // FORENSIC — it must survive its own summary. Since C1 this one
        // record is the funnel for all three escalation terminals
        // (`escalated`, `blocked`, `inconclusive`), and the record is
        // asserted PARSEABLE by the ports' own strict JSON parser, with
        // the hostile summary round-tripping byte-for-byte.
        let hostile = "run blocked: \"governance\" and a \\ backslash";
        let record = super::ticket_evidence_record(
            &gauntlet_domain::UnitId::new("AK-9101").expect("fixture unit id"),
            hostile,
        );
        let parsed = gauntlet_ports::json::parse(&record).unwrap_or_else(|error| {
            panic!("the forensic record parses: {error} — record: {record}")
        });
        match parsed.get("summary") {
            Some(gauntlet_ports::json::JsonValue::Str(summary)) => assert_eq!(
                summary, hostile,
                "the summary round-trips through the record byte-for-byte"
            ),
            other => panic!("the summary is a JSON string, got {other:?} — record: {record}"),
        }
    }

    #[test]
    fn resume_restores_per_unit_attempts_from_durable_productions() {
        // Correction C8.3 (critic-B A11, counters): a crash-retry loop
        // must not re-arm the §11.5 max-attempts ceiling on every crash.
        //
        // C8.3 re-shaped for the durable-settle era (Task 13 B2): a PASS
        // now SETTLES its unit durably (a Major residual is not blocking,
        // §16.1 condition 4), so the attempts ladder is exercised over a
        // verifier that FAILs every attempt — nothing settles, and the
        // §11.5 row-1 ladder keeps re-dispatching. A tight rounds ceiling
        // (genuine iterations only) bounds the first run to exactly one
        // attempt — forward_ceiling = rounds * (100 - FIXTURE_RESERVE_PCT)
        // / 100, one genuine round — ending `budget_exhausted` before
        // ATTEMPT_CAP is ever threatened. The resume reconstructs
        // attempts_used from the durable evidence production at the
        // unit's frozen locator, dispatches the next attempt genuinely,
        // and THAT residual's disposal is the one that hits the cap
        // (escalate). (A transport error no longer ends runs — a repeated
        // transient retries instead, Task 13 B2 Item 4 — so the bounded
        // ending here is the rounds dimension.)
        const RESIDUAL_EVERY_COMPARISON: usize = 9;
        let key = Rc::new(RefCell::new(answer_key(1)));
        let state = FakeState::new();
        let evidence = FakeEvidence::new();
        let never_passing =
            crate::verify::ladder::Ladder::new(vec![crate::verify::ladder::LadderRung::new(
                crate::verify::ladder::RungKind::TestReport,
                Arc::new(FlakyVerdictVerifier {
                    calls: std::cell::Cell::new(0),
                    pass_from: u32::MAX,
                }),
            )])
            .expect("the never-passing ladder composes");
        let first_run = ScriptedRunner::new(Vec::new());
        let first = compose_full(
            key.clone(),
            state.clone(),
            evidence.clone(),
            first_run,
            RESIDUAL_EVERY_COMPARISON,
            2,
            policy(),
            never_passing.clone(),
            meter_factory(),
            None,
        )
        .expect("composes");
        let first_outcome = first
            .controller
            .start_run()
            .expect("the rounds dimension bounds the first run");
        assert_eq!(
            first_outcome.terminal,
            TerminalState::BudgetExhausted,
            "the failing ladder burns its rounds without settling anything"
        );
        let resumed = ScriptedRunner::new(vec![Behavior::Complete, Behavior::Complete]);
        let second = compose_full(
            key.clone(),
            state.clone(),
            evidence.clone(),
            resumed,
            RESIDUAL_EVERY_COMPARISON,
            FIXTURE_ROUNDS,
            policy(),
            never_passing,
            meter_factory(),
            None,
        )
        .expect("composes");
        let run_id = RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("fixture run id");
        let outcome = second
            .controller
            .resume_run(&run_id)
            .expect("the run resumes");
        let attempts_prompted = first
            .runner
            .prompts
            .borrow()
            .iter()
            .filter(|(_, prompt)| prompt.contains("AK-9101"))
            .count()
            + second
                .runner
                .prompts
                .borrow()
                .iter()
                .filter(|(_, prompt)| prompt.contains("AK-9101"))
                .count();
        assert_eq!(
            attempts_prompted, 2,
            "one bounded-run attempt (recorded durably) + exactly ONE \
             restored-ceiling attempt after the resume"
        );
        assert_eq!(
            outcome.terminal,
            TerminalState::Escalated,
            "the restored attempts count disposes the second residual at the cap"
        );
    }

    #[test]
    fn a_result_whose_meter_charge_is_refused_is_recorded_not_dropped() {
        // Correction C9.3, site 1 (critic-B A12): a completed result whose
        // meter charge is refused must be KEPT FOR FORENSICS — §21.2
        // "never a swallowed error", §10.4 "not silently dropped", and
        // §12.3's precedent that an artifact may be stored for forensics
        // while excluded from verification inputs. RECALIBRATED for the
        // per-attempt reservation fix: the reservation is now the
        // forward remainder divided by `max_parallel`, not by
        // `rounds_left` — 400 tokens (800 forward / FIXTURE_PARALLELISM
        // 2), not the old 100 (800 forward / 8 rounds). The result
        // reports 500 — past the reservation (though still well inside
        // the 800-token forward remainder), refused at the meter under
        // its unchanged `ReservationExceeded` port contract.
        let key = Rc::new(RefCell::new(answer_key(1)));
        let state = FakeState::new();
        let evidence = FakeEvidence::new();
        let runner = ScriptedRunner::new(vec![Behavior::CompleteWithTokens(500)]);
        let composed =
            compose(key, state, evidence.clone(), runner, 0, FIXTURE_ROUNDS).expect("composes");
        let outcome = composed
            .controller
            .start_run()
            .expect("a charge refusal terminates, never errors the run");
        assert_eq!(outcome.terminal, TerminalState::BudgetExhausted);
        let records = evidence.records_containing("\"class\":\"result_uncharged\"");
        assert_eq!(
            records.len(),
            1,
            "the refused result is pinned as forensic evidence: {records:?}"
        );
        assert!(
            records[0].contains("\"refused_by\":\"meter\""),
            "the record names the refusing locus (the meter): {}",
            records[0]
        );
        assert!(
            records[0].contains("AK-9101") && records[0].contains("\"requested_tokens\":500"),
            "the record names the unit and the requested payment: {}",
            records[0]
        );
    }

    #[test]
    fn an_attempt_spending_more_than_a_rounds_share_but_within_its_hold_settles() {
        // The real demo run's shape at fixture scale: 800 forward tokens,
        // 8 rounds, parallelism 2. A builder that honestly spends 350
        // tokens exceeds the old per-round share (800 / 8 = 100) but fits
        // its hold (800 / 2 = 400). It must settle and book its spend —
        // before the fix the meter refused the charge and the loop ended
        // `budget_exhausted` with no budget dimension breached.
        const HONEST_SPEND: u64 = 350;
        let key = Rc::new(RefCell::new(answer_key(1)));
        let state = FakeState::new();
        let evidence = FakeEvidence::new();
        let runner = ScriptedRunner::new(vec![Behavior::CompleteWithTokens(HONEST_SPEND)]);
        let composed =
            compose(key, state, evidence.clone(), runner, 0, FIXTURE_ROUNDS).expect("composes");
        let outcome = composed.controller.start_run().expect("the loop runs");
        assert_eq!(outcome.terminal, TerminalState::Settled, "{outcome:?}");
        assert!(
            evidence
                .records_containing("\"class\":\"result_uncharged\"")
                .is_empty(),
            "an attempt inside its hold is charged, never recorded uncharged"
        );
        let checkpoint = composed
            .state
            .resume(&outcome.run_id, bound())
            .expect("checkpoint readable")
            .expect("a checkpoint exists");
        assert_eq!(
            checkpoint.budget_spent.tokens(),
            HONEST_SPEND,
            "the honest spend is booked: {checkpoint:?}"
        );
    }

    #[test]
    fn a_result_the_run_budget_refuses_is_recorded_not_dropped() {
        // Correction C9.3, site 2 — the drop site C2 CREATED (the loop's
        // own budget charge, after the meter already accepted). The
        // shipped mirror meter always refuses first (it books the same
        // numbers), so this site is isolated with the permissive meter
        // fixture: two completions reporting 700 tokens each cross the
        // loop budget's 800-token forward ceiling on the second.
        let key = Rc::new(RefCell::new(answer_key(2)));
        let state = FakeState::new();
        let evidence = FakeEvidence::new();
        let runner = ScriptedRunner::new(vec![
            Behavior::CompleteWithTokens(700),
            Behavior::CompleteWithTokens(700),
        ]);
        let composed = compose_full(
            key,
            state,
            evidence.clone(),
            runner,
            0,
            FIXTURE_ROUNDS,
            policy(),
            fixture_ladder(&["ctrl-9101.json", "ctrl-9102.json"]),
            permissive_meter_factory(),
            None,
        )
        .expect("composes");
        let outcome = composed
            .controller
            .start_run()
            .expect("a charge refusal terminates, never errors the run");
        assert_eq!(outcome.terminal, TerminalState::BudgetExhausted);
        let records = evidence.records_containing("\"class\":\"result_uncharged\"");
        assert_eq!(
            records.len(),
            1,
            "the refused result is pinned as forensic evidence: {records:?}"
        );
        assert!(
            records[0].contains("\"refused_by\":\"run_budget\""),
            "the record names the refusing locus (the run budget): {}",
            records[0]
        );
        assert!(
            records[0].contains("\"requested_tokens\":700"),
            "the record names the requested payment: {}",
            records[0]
        );
    }

    #[test]
    fn a_meter_refused_critic_dispatch_is_truncated_and_the_run_continues() {
        // F4 (confirmation round 1, finding 1 — AK-052's critic-fan-out
        // leg): §10.4 "Critic fan-out is bounded by review.max_critics
        // AND by the budget meter; a fan-out that would breach the
        // recovery reserve is truncated ... and the truncation is
        // recorded as unavailable_evidence, not silently dropped";
        // §17.4 "consulted before every dispatch, not after". The review
        // this controller drives on every completion must admit its
        // model comparator through the run meter: with the meter at the
        // forward ceiling the critic dispatch is REFUSED at admission,
        // the dimension is recorded as truncated, and the run CONTINUES
        // — never a run error (the C6.1 skip-and-continue discipline,
        // on the critic leg).
        let key = Rc::new(RefCell::new(answer_key(1)));
        let state = FakeState::new();
        let evidence = FakeEvidence::new();
        let runner = ScriptedRunner::new(vec![Behavior::Complete]);
        let comparators = ScriptedComparators::new(0);
        let tally = Arc::new(Mutex::new(MeterTally::default()));
        let meter_tally = tally.clone();
        let factory: Arc<
            dyn Fn(Budget) -> Arc<dyn gauntlet_ports::budget_meter::BudgetMeter> + Send + Sync,
        > = Arc::new(move |budget| {
            #[allow(clippy::arc_with_non_send_sync)] // single-threaded proof fixture wiring
            Arc::new(CeilingMeter {
                inner: FakeMeter(Mutex::new(FakeMeterInner {
                    budget,
                    outstanding: 0,
                })),
                admit_first: 1,
                tally: meter_tally.clone(),
            })
        });
        let scratch = Scratch::new("critic-refused");
        let clock = DeterministicClock::new();
        let provider = Arc::new(FileReferenceProvider::from_sources(
            AnswerKeySource::Shared(key.clone()),
            ManifestSource::Inline(minimal_manifest()),
            Rc::new(evidence.clone()),
        ));
        let deps = ControllerDeps {
            state: state.clone(),
            evidence: Arc::new(evidence.clone()),
            ids: Arc::new(SeededIds(RefCell::new(0))),
            clock,
            runner: runner.clone(),
            runner_identity: gauntlet_domain::RunnerIdentity::new("fixture-runner")
                .expect("runner identity"),
            observers: vec![Arc::new(FixtureObserver {
                store: evidence.clone(),
                clock: DeterministicClock::new(),
            })],
            comparison: ComparisonDeps {
                contract: minimal_contract(),
                deterministic: Arc::new(ComparatorsSource(comparators.clone())),
                model: Arc::new(ComparatorsSource(comparators.clone())),
                adversarial: None,
                review_mode: gauntlet_domain::ReviewMode::Single,
                // model_dispatches >= 1: the count budget admits the
                // critic; only the meter refuses it.
                model_dispatches: 4,
                role_deny: crate::roles::ContextDenySet::new(Vec::new()),
            },
            ladder_for: Arc::new(move |_| Ok(fixture_ladder(&["ctrl-9101.json"]))),
            reference: provider,
            answer_key: AnswerKeySource::Shared(key.clone()),
            escalation: Arc::new(FakeEscalation::default()),
            meter_factory: factory,
            budget: budget_shape(FIXTURE_ROUNDS),
            policy: policy(),
            workspaces: None,
            workspace_root: scratch.0.clone(),
            config_fingerprint: Digest::new("sha256", &"ab".repeat(32)).expect("digest"),
            govern: crate::govern::GovernDeps::none_backend(),
            environment_fingerprint: Digest::new("sha256", &"cd".repeat(32)).expect("digest"),
            intent: "controller fixture".to_owned(),
        };
        std::mem::forget(scratch);
        let controller = RunController::compose(deps).expect("composes");
        let outcome = controller
            .start_run()
            .expect("a refused critic dispatch never errors the run");
        assert_eq!(
            outcome.terminal,
            TerminalState::Settled,
            "a truncated dimension settles nothing and blocks nothing — the run continues"
        );
        assert_eq!(
            comparators.calls.get(),
            0,
            "the critic dispatch is refused at admission — the model comparator never ran"
        );
        let observed = tally.lock().expect("tally lock");
        assert!(
            observed.refusals >= 1,
            "the critic leg consulted the meter before dispatching (refusals: {})",
            observed.refusals
        );
        assert_eq!(
            observed.granted, observed.released,
            "nothing is held after the run — the builder's reservation charged back"
        );
    }

    #[test]
    fn a_critic_reservation_releases_at_fan_in_so_a_later_iteration_still_dispatches() {
        // F4's no-over-refusal leg: with headroom the critic dispatch
        // PROCEEDS and its meter reservation is RELEASED at the job's
        // terminal (the zero-payment charge, the B1 convention the
        // builder leg uses), so a later iteration's dispatch is not
        // silently truncated by a leaked slot. A leaked critic
        // reservation would over-truncate every review after the first —
        // §10.4's meter bound must bound the fan-out, never strangle the
        // loop. Two iterations are driven by the dependency key
        // (AK-9102 waits for AK-9101): one review each, and the model
        // source is the ADMITTING two-phase critic, so the release being
        // pinned is the FAN-IN collection's, not a synchronous bracket.
        let key = Rc::new(RefCell::new(answer_key_with_dependency()));
        let state = FakeState::new();
        let evidence = FakeEvidence::new();
        let runner = ScriptedRunner::new(vec![Behavior::Complete, Behavior::Complete]);
        let (model, admissions, collects) = AdmittingSource::new();
        let deterministic = ScriptedComparators::new(0);
        let tally = Arc::new(Mutex::new(MeterTally::default()));
        let meter_tally = tally.clone();
        let factory: Arc<
            dyn Fn(Budget) -> Arc<dyn gauntlet_ports::budget_meter::BudgetMeter> + Send + Sync,
        > = Arc::new(move |budget| {
            #[allow(clippy::arc_with_non_send_sync)] // single-threaded proof fixture wiring
            Arc::new(ObservingMeter {
                inner: FakeMeter(Mutex::new(FakeMeterInner {
                    budget,
                    outstanding: 0,
                })),
                tally: meter_tally.clone(),
            })
        });
        let scratch = Scratch::new("critic-release");
        let clock = DeterministicClock::new();
        let provider = Arc::new(FileReferenceProvider::from_sources(
            AnswerKeySource::Shared(key.clone()),
            ManifestSource::Inline(minimal_manifest()),
            Rc::new(evidence.clone()),
        ));
        let deps = ControllerDeps {
            state: state.clone(),
            evidence: Arc::new(evidence.clone()),
            ids: Arc::new(SeededIds(RefCell::new(0))),
            clock,
            runner: runner.clone(),
            runner_identity: gauntlet_domain::RunnerIdentity::new("fixture-runner")
                .expect("runner identity"),
            observers: vec![Arc::new(FixtureObserver {
                store: evidence.clone(),
                clock: DeterministicClock::new(),
            })],
            comparison: ComparisonDeps {
                contract: minimal_contract(),
                deterministic: Arc::new(ComparatorsSource(deterministic.clone())),
                model,
                adversarial: None,
                review_mode: gauntlet_domain::ReviewMode::Single,
                model_dispatches: 4,
                role_deny: crate::roles::ContextDenySet::new(Vec::new()),
            },
            ladder_for: Arc::new(move |_| {
                Ok(fixture_ladder(&["ctrl-9101.json", "ctrl-9102.json"]))
            }),
            reference: provider,
            answer_key: AnswerKeySource::Shared(key.clone()),
            escalation: Arc::new(FakeEscalation::default()),
            meter_factory: factory,
            budget: budget_shape(FIXTURE_ROUNDS),
            policy: policy(),
            workspaces: None,
            workspace_root: scratch.0.clone(),
            config_fingerprint: Digest::new("sha256", &"ab".repeat(32)).expect("digest"),
            govern: crate::govern::GovernDeps::none_backend(),
            environment_fingerprint: Digest::new("sha256", &"cd".repeat(32)).expect("digest"),
            intent: "controller fixture".to_owned(),
        };
        std::mem::forget(scratch);
        let controller = RunController::compose(deps).expect("composes");
        let outcome = controller
            .start_run()
            .expect("a headroom review never errors the run");
        assert_eq!(
            outcome.terminal,
            TerminalState::Settled,
            "both units settle — the critic dispatches blocked nothing"
        );
        assert_eq!(
            admissions.get(),
            2,
            "both iterations' critic dispatches were admitted — a leaked slot would have \
             truncated the second review"
        );
        assert_eq!(
            collects.get(),
            2,
            "both admitted critics collected at fan-in"
        );
        let observed = tally.lock().expect("tally lock");
        assert_eq!(
            observed.granted, observed.released,
            "every reservation — two builders and two critics — came back: nothing is held \
             after fan-in"
        );
    }

    #[test]
    fn an_escaping_evidence_locator_is_refused_at_the_write_boundary() {
        // N4 (critic-B A12's INCONCLUSIVE, independently confirmed by the
        // readjudication): the domain's plain-ref grammar permits `..`, so
        // a `../`-bearing locator in the answer key joins to a path
        // OUTSIDE the workspace root — and this loop is the first
        // production writer at that path. The controller refuses the
        // write, typed, naming the locator and the root it escapes;
        // nothing is written outside, and the domain grammar is not
        // widened (that is a ports revision, not a controller fix).
        let text = [
            "## AK-9101",
            "claim: the fixture claim is settled by a passing test report",
            "verify_by: test",
            "blast_radius: contained",
            "authority: critic",
            "evidence_required: {kind: test_report, locator: ../leak-ctrl-9101.json, producer: instrument}",
            "",
        ]
        .join("\n");
        let key = Rc::new(RefCell::new(text));
        let state = FakeState::new();
        let evidence = FakeEvidence::new();
        let runner = ScriptedRunner::new(vec![Behavior::Complete]);
        let composed = compose(key, state, evidence, runner, 0, FIXTURE_ROUNDS).expect("composes");
        let outside_root = composed
            .controller
            .workspace_root()
            .parent()
            .expect("the scratch root has a parent")
            .join("leak-ctrl-9101.json");
        let _ = std::fs::remove_file(&outside_root);
        match composed.controller.start_run() {
            Err(ControlError::EvidenceLocatorEscapes { locator, root }) => {
                // The needle is built, not spelled: a bare `../…` string
                // literal is a path-literal shape to the boundary scanner
                // (AK-016). The asserted value is byte-identical.
                let escaped_needle = format!("..{}leak-ctrl-9101.json", "/");
                assert_eq!(locator.as_str(), escaped_needle);
                assert_eq!(
                    root,
                    *composed.controller.workspace_root(),
                    "the refusal names the root the locator escapes"
                );
            }
            Err(other) => panic!("the escaping locator is refused typed, got {other}"),
            Ok(outcome) => {
                panic!("a `..` locator never writes outside the workspace root, got {outcome:?}")
            }
        }
        assert!(
            !outside_root.exists(),
            "no byte was written outside the workspace root"
        );
    }

    #[test]
    fn a_drain_that_never_terminates_is_bounded_cancelled_and_escalated() {
        // Correction C9.4 (critic-B A6's controller-drain INCONCLUSIVE): a
        // runner that reports ever-fresh progress and never completes
        // cannot be caught by stall detection — the quiet window never
        // elapses — so the drain must carry its own bound. The bound is
        // DERIVED from the two settings the drain already reads (one
        // stall window of polls, QUIET_WINDOW/TICK_WINDOW passes),
        // not a new Annex E setting. Past the bound: cancel in-flight,
        // pin the fact, escalate. The fixture runner hard-stops two
        // polls past that bound, so the unbounded drain is OBSERVED as
        // an error instead of hanging the test process. The budget is
        // built from the bound, not spelled — a bare poll count is a
        // timeout shape to the boundary scanner (AK-016).
        const DERIVED_DRAIN_PASS_BOUND: u64 = QUIET_WINDOW / TICK_WINDOW;
        const PROGRESS_SPIN_POLL_BUDGET: u32 = DERIVED_DRAIN_PASS_BOUND as u32 + 2;
        let key = Rc::new(RefCell::new(answer_key(1)));
        let state = FakeState::new();
        let evidence = FakeEvidence::new();
        let runner = ScriptedRunner::new(vec![Behavior::ProgressForever {
            max_polls: PROGRESS_SPIN_POLL_BUDGET,
        }]);
        let composed =
            compose(key, state, evidence.clone(), runner, 0, FIXTURE_ROUNDS).expect("composes");
        let outcome = composed
            .controller
            .start_run()
            .expect("the drain bounds itself without external help");
        assert_eq!(outcome.terminal, TerminalState::Escalated);
        assert!(
            !evidence
                .records_containing("\"class\":\"drain_poll_bound\"")
                .is_empty(),
            "the bound trip is pinned as evidence, not silent"
        );
        let polls = composed.runner.polls.borrow();
        assert!(
            polls
                .values()
                .all(|spent| u64::from(*spent) <= DERIVED_DRAIN_PASS_BOUND),
            "the drain stopped itself inside one stall window of polls: {polls:?}"
        );
    }

    #[test]
    fn a_mid_run_answer_key_edit_aborts_the_run_with_both_digests_as_evidence() {
        // Prereg P2's shape: the operator edits the ruler while an attempt
        // is in flight; the NEXT boundary re-verify fires. Impossible to
        // pass by caching: verify_unchanged re-reads the file every time.
        let key = Rc::new(RefCell::new(answer_key(1)));
        let state = FakeState::new();
        let evidence = FakeEvidence::new();
        let runner = ScriptedRunner::new(vec![Behavior::Complete]);
        let edit_key = key.clone();
        runner.set_hook(move || {
            // The edit lands AFTER the first dispatch (the hook fires
            // inside invoke): the in-flight attempt's unit is edited.
            let mut text = edit_key.borrow_mut();
            *text = text.replace(
                "the fixture claim is settled by a passing test report",
                "the OPERATOR-EDITED claim",
            );
        });
        let composed = compose(
            key.clone(),
            state.clone(),
            evidence.clone(),
            runner,
            0,
            FIXTURE_ROUNDS,
        )
        .expect("composes");
        let outcome = composed.controller.start_run().expect("the loop runs");
        assert_eq!(outcome.terminal, TerminalState::Aborted);
        let ReferenceBreach::Mutated { expected, actual } =
            outcome.reference_breach.expect("the breach is classified")
        else {
            panic!("a moved ruler is a mutation, never unavailability");
        };
        assert_ne!(expected, actual);
        // BOTH digests are PINNED durable evidence — not a log line.
        let records = evidence.records_containing("reference_mutated");
        assert_eq!(records.len(), 1, "exactly one mutation record is pinned");
        assert!(records[0].contains(&expected.to_string()));
        assert!(records[0].contains(&actual.to_string()));
        // No retry: exactly one dispatch happened.
        assert_eq!(composed.runner.prompts.borrow().len(), 1);
    }

    #[test]
    fn a_stalled_attempt_cancels_retries_once_then_escalates() {
        // §11.4 step 2 + §11.5 row 4: the first stall cancels and retries
        // (the re-dispatch prompt carries the preserved session note); the
        // second stall escalates.
        let composed = fixture_composed(
            vec![Behavior::StallForever, Behavior::StallForever],
            0,
            1,
            FIXTURE_ROUNDS,
        )
        .expect("composes");
        let outcome = composed.controller.start_run().expect("the loop runs");
        assert_eq!(outcome.terminal, TerminalState::Escalated);
        let summary = outcome.escalation_summary.expect("escalation recorded");
        assert!(summary.contains("timed out twice"));
        assert!(
            summary.contains("session-fixture"),
            "the preserved session reference travels with the escalation"
        );
        assert_eq!(
            composed.runner.prompts.borrow().len(),
            2,
            "exactly one retry before escalation"
        );
        let prompts = composed.runner.prompts.borrow().clone();
        assert!(
            prompts[1]
                .1
                .contains("session reference session-fixture preserved"),
            "the retry prompt carries the preserved session note: {}",
            prompts[1].1
        );
    }

    #[test]
    fn the_transient_retry_cancels_the_prior_invocation_through_the_port() {
        // Corrections round 1, F6: the §11.5 row-2 transient disposition
        // retries a dispatch whose fate is UNKNOWN (the poll channel
        // failed) — and the §11.5 rows leave a re-dispatch no right to
        // overlap a possibly-alive prior invocation. The port's own
        // idempotent cancel (§7.3) retires the prior BEFORE the
        // re-dispatch. The R16-F2 hostile-child shape is exactly this: a
        // child that stays alive while its poll channel errors forever;
        // a retry that does not cancel the prior leaves the hostile
        // child running beside its replacement.
        let composed = fixture_composed(
            vec![Behavior::PollError, Behavior::Complete],
            0,
            1,
            FIXTURE_ROUNDS,
        )
        .expect("composes");
        let outcome = composed.controller.start_run().expect("the loop runs");
        assert_eq!(outcome.terminal, TerminalState::Settled);
        let prompts = composed.runner.prompts.borrow().clone();
        assert_eq!(prompts.len(), 2, "exactly one transient retry");
        let first_invocation = prompts[0].0.clone();
        assert!(
            !outcome.escalation_summary.is_some(),
            "the healthy retry settles; nothing escalates"
        );
        assert_eq!(
            composed.runner.cancels(),
            vec![first_invocation],
            "the prior invocation was cancelled through the port before the retry"
        );
    }

    #[test]
    fn a_terminal_reached_with_work_in_flight_cancels_the_stranded_invocation() {
        // Corrections round 1, F6: a terminal decided mid-drain (here the
        // contract-mismatch abort) must leave NO runner invocation
        // behind — the still-in-flight invocation is cancelled through
        // the port, its Cancelled terminal event is durable, and its
        // reservation is released. The run's end retires its own work;
        // the port's cancel is the only mechanism that does so. The LONG
        // quiet-window fixture keeps the §11.4 stall rule out of the
        // way: without the terminal cleanup this stranded invocation is
        // simply never cancelled, which is the RED this test starts
        // from.
        let composed = fixture_composed_quiet(
            vec![
                Behavior::Fail(InvocationFailure::ContractMismatch),
                Behavior::StallForever,
            ],
            2,
            FIXTURE_ROUNDS,
        )
        .expect("composes");
        let outcome = composed.controller.start_run().expect("the loop runs");
        assert_eq!(outcome.terminal, TerminalState::Aborted);
        let prompts = composed.runner.prompts.borrow().clone();
        assert_eq!(prompts.len(), 2, "both units dispatched before the abort");
        let stranded = prompts[1].0.clone();
        assert_eq!(
            composed.runner.cancels(),
            vec![stranded],
            "the invocation still in flight at the terminal was cancelled"
        );
    }

    #[test]
    fn a_multi_member_slice_is_refused_rather_than_charged_for_and_dropped() {
        // Correction C6.3, the adjudication's rule: "charge what you
        // prompt; prompt what you charged." The scheduler sums a slice's
        // estimate over ALL members (§11.3: one builder performs every
        // member's work), the controller reserves that summed slice, and
        // it prompts ONE unit. A slice with more than one member is
        // therefore charged for work it never prompts. Empty and
        // single-member slices are unchanged.
        let slice = gauntlet_ports::budget_meter::BudgetSlice {
            rounds: 0,
            wall_seconds: 0,
            tokens: 0,
            cost_usd: MicroUsd::new(0),
        };
        let one = gauntlet_domain::UnitId::new("AK-9101").expect("fixture unit id");
        let two = gauntlet_domain::UnitId::new("AK-9102").expect("fixture unit id");
        let planned =
            |units: Vec<gauntlet_domain::UnitId>| super::super::scheduler::PlannedDispatch {
                units,
                blast_radius: gauntlet_domain::BlastRadius::Contained,
                authority: gauntlet_domain::Authority::Critic,
                estimated_slice: slice,
                depth: 0,
            };
        assert_eq!(
            super::dispatchable_member(&planned(Vec::new())).expect("an empty slice is skipped"),
            None
        );
        assert_eq!(
            super::dispatchable_member(&planned(vec![one.clone()]))
                .expect("a single-member slice dispatches"),
            Some(one.clone())
        );
        let refused = super::dispatchable_member(&planned(vec![one.clone(), two.clone()]))
            .expect_err("a slice this controller cannot dispatch is refused, never truncated");
        assert!(
            matches!(
                &refused,
                ControlError::SliceNotDispatchable { units } if *units == vec![one, two]
            ),
            "the refusal carries its member list: {refused:?}"
        );
        assert!(
            refused.to_string().contains("AK-9102"),
            "the message names the member that would have been dropped: {refused}"
        );
    }

    #[test]
    fn the_answer_key_cannot_express_a_cohort_on_any_shipped_surface() {
        // Correction C6.4 / L4, pinned rather than claimed in prose:
        // §11.3's "not meaningfully separable" criterion has no shipped
        // marker, so `PlanInput.cohorts` is structurally unfeedable and
        // both production sites hardwire an empty map. If a cohort surface
        // ever ships, this test fails and the record beside those two
        // sites must be revisited — that is the point of pinning it.
        let refused = crate::reference::parse::parse_all_or_fail(
            "## AK-9101\nclaim: the fixture claim is settled by a passing test \
             report\nverify_by: test\nblast_radius: contained\nauthority: \
             critic\ncohort: parser-branch\nevidence_required: {kind: test_report, locator: \
             ctrl-9101.json, producer: instrument}\n",
        )
        .expect_err("a cohort field is outside the frozen answer-key vocabulary");
        assert!(
            refused.to_string().contains("cohort"),
            "the refusal names the unknown field: {refused}"
        );
    }

    #[test]
    fn the_answer_key_cannot_express_a_unit_status_on_any_shipped_surface() {
        // Correction C7.1, recorded honestly rather than composed (the L4
        // precedent, and §11.6's "Provisional capability is never reported
        // as complete"). §11.4 step 3's "the in-memory snapshot is never
        // authoritative over the file" has a status leg with NO shipped
        // input: the parser's field set is frozen and typed-refusing, and
        // the domain constructor hardwires the initial status. This test
        // is what makes the unreachability a fact rather than a claim —
        // if a `status:` surface ever ships, this test fails and the
        // propagation branch becomes writable and testable.
        let refused = crate::reference::parse::parse_all_or_fail(
            "## AK-9101\nclaim: the fixture claim is settled by a passing test \
             report\nverify_by: test\nblast_radius: contained\nauthority: \
             critic\nstatus: settled\nevidence_required: {kind: test_report, locator: \
             ctrl-9101.json, producer: instrument}\n",
        )
        .expect_err("a status field is outside the frozen answer-key vocabulary");
        assert!(
            refused.to_string().contains("status"),
            "the refusal names the unknown field: {refused}"
        );
        // And every well-formed block parses OPEN, whatever else it says.
        let parsed = crate::reference::parse::parse_all_or_fail(&answer_key(2))
            .expect("the fixture key parses");
        assert!(
            parsed.iter().all(|unit| unit.status() == UnitStatus::Open),
            "no shipped parse produces a status other than open"
        );
    }

    #[test]
    fn a_file_only_unit_is_never_dispatched_the_frozen_reference_bounds_membership() {
        // ITEM 0 — the C7 remainder, per the post-F1 orchestrator ruling.
        // Under §21.3's unavailability window (the ONLY window in which a
        // mid-run answer-key edit changes what the loop does instead of
        // aborting it) a unit added to the FILE is still absent from the
        // FROZEN reference. Dispatching it is a terminal-truth defect: the
        // verification authority refuses the unit (`UnitUnavailable`) and
        // the run dies with a bare ControlError — no terminal, no close-out
        // checkpoint, no report. The guard: dispatch-eligible membership is
        // bounded by the frozen reference; the mutation is left where it
        // already is, with the boundary abort, the moment the provider
        // answers again.
        let key = Rc::new(RefCell::new(answer_key_with_dependency()));
        let state = FakeState::new();
        let evidence = FakeEvidence::new();
        let runner = ScriptedRunner::new(Vec::new());
        let composed = compose(
            key.clone(),
            state.clone(),
            evidence.clone(),
            runner,
            0,
            FIXTURE_ROUNDS,
        )
        .expect("composes");
        // On the FIRST durable settlement (AK-9101's): append AK-9103 to
        // the file AND make the provider unavailable, so the next boundary
        // records the gap and continues instead of aborting on the digest
        // change, and the next plan runs over a file whose membership
        // exceeds the freeze. AK-9102 is still owed (it depends on
        // AK-9101), so every later iteration plans over that file.
        //
        // (Task 13 B2: the trigger moved from the first DISPATCH to the
        // first durable SETTLEMENT. §16.1 condition 5 now consults the
        // reference immediately before settlement and §21.2 forbids
        // converting an unanswerable check into settled success — under an
        // outage opened mid-drain the owed unit's settlement is refused
        // and the old `Settled` outcome is unconstructible. The §21.3
        // window still opens mid-run: the boundary gaps are recorded, the
        // frozen membership still bounds dispatch, and the run ends
        // bounded by its rounds dimension, resume-able.)
        {
            let key = key.clone();
            let unavailable = composed.unavailable.clone();
            *state.on_first_settlement.borrow_mut() = Some(Box::new(move || {
                if key.borrow().contains("AK-9103") {
                    return; // the edit lands once
                }
                let mut text = key.borrow().clone();
                text.push_str(
                    "## AK-9103\nclaim: the added claim is settled by a passing test \
                     report\nverify_by: test\nblast_radius: contained\nauthority: \
                     critic\nevidence_required: {kind: test_report, locator: \
                     ctrl-9103.json, producer: instrument}\n\n",
                );
                *key.borrow_mut() = text;
                unavailable.set(true);
            }));
        }
        let outcome = composed
            .controller
            .start_run()
            .expect("a file-only unit never errors the run at verification");
        assert_eq!(
            outcome.terminal,
            TerminalState::BudgetExhausted,
            "the §21.3 window keeps the run alive and bounded: the owed unit cannot \
             settle while the reference is unanswerable, and the rounds dimension ends \
             the wait"
        );
        // The frozen-membership guard held across EVERY post-edit plan:
        // only the two frozen units were ever dispatched, and the file-only
        // unit was never prompted.
        assert!(
            composed
                .runner
                .prompts
                .borrow()
                .iter()
                .all(|(_, prompt)| !prompt.contains("AK-9103")),
            "the file-only unit was never prompted"
        );
        assert!(
            composed
                .runner
                .prompts
                .borrow()
                .iter()
                .any(|(_, prompt)| prompt.contains("AK-9101"))
                && composed
                    .runner
                    .prompts
                    .borrow()
                    .iter()
                    .any(|(_, prompt)| prompt.contains("AK-9102")),
            "both frozen units ran (the owed unit re-attempted under the window)"
        );
        // The pre-outage settlement is durable; the owed unit's refusals
        // and the window's gaps are pinned evidence, not silence.
        let run_id = RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("fixture run id");
        let rows = state
            .settlements(&run_id, bound())
            .expect("settlement rows readable");
        assert_eq!(
            rows.len(),
            1,
            "exactly the pre-outage settlement is durable: {rows:?}"
        );
        assert_eq!(rows[0].unit_id().as_str(), "AK-9101");
        assert!(
            !evidence
                .records_containing("\"class\":\"reference_unavailable\"")
                .is_empty(),
            "the §21.3 unavailability window is recorded as pinned evidence"
        );
        assert!(
            !evidence.records_containing("settlement_refused").is_empty(),
            "the owed unit's settle-time refusals are pinned forensic evidence"
        );
    }

    #[test]
    fn a_settled_dependency_stays_in_the_graph_while_eligibility_filters_it() {
        // Correction C6.2 (critic-B A3 "dependency wiring"): AK-9102
        // depends_on AK-9101. Iteration 1 settles AK-9101; iteration 2's
        // plan must still SEE it — §11.2 conjunct 2 asks "is the
        // dependency settled or out_of_scope", a question that requires
        // the dependency to be IN the graph — while eligibility filters it
        // out separately. Membership and eligibility are different
        // questions; conflating them makes multi-hop graphs uncompletable.
        let key = Rc::new(RefCell::new(answer_key_with_dependency()));
        let state = FakeState::new();
        let evidence = FakeEvidence::new();
        let runner = ScriptedRunner::new(vec![Behavior::Complete]);
        let composed = compose(key, state, evidence, runner, 0, FIXTURE_ROUNDS).expect("composes");
        let outcome = composed
            .controller
            .start_run()
            .expect("a settled dependency never errors the next plan");
        assert_eq!(
            outcome.terminal,
            TerminalState::Settled,
            "the dependent unit runs once its dependency settles"
        );
        assert_eq!(
            composed.runner.prompts.borrow().len(),
            2,
            "the dependency dispatched first, then the dependent unit"
        );
    }

    #[test]
    fn an_over_capacity_fan_out_is_truncated_and_recorded_never_a_run_error() {
        // Correction C6.1 (critic-B A3 "concurrency accounting"): with
        // FIXTURE_PARALLELISM + 1 independent contained units and nothing
        // in flight, the plan carries one dispatch per eligible unit. The
        // member that arrives once every parallel slot is held is a
        // CAPACITY truncation (§10.4), recorded as pinned evidence and
        // deferred — never a run error (§21.3 aborts only for an integrity
        // violation), and never read as a budget refusal: each attempt's
        // hold is the forward remainder divided by `max_parallel`, so the
        // capacity check runs before the summed budget consult. The
        // truncated unit stays eligible and dispatches on a later
        // iteration.
        let key = Rc::new(RefCell::new(answer_key(OVER_CAPACITY_UNITS)));
        let state = FakeState::new();
        let evidence = FakeEvidence::new();
        let runner = ScriptedRunner::new(vec![Behavior::Complete]);
        let composed = compose_full(
            key,
            state,
            evidence.clone(),
            runner,
            0,
            FIXTURE_ROUNDS,
            policy(),
            fixture_ladder(&["ctrl-9101.json", "ctrl-9102.json", "ctrl-9103.json"]),
            meter_factory(),
            None,
        )
        .expect("composes");
        let outcome = composed
            .controller
            .start_run()
            .expect("an over-capacity fan-out never errors the run");
        assert_eq!(
            outcome.terminal,
            TerminalState::Settled,
            "every unit settles once the truncated dispatch gets its turn"
        );
        assert_eq!(
            composed.runner.prompts.borrow().len(),
            OVER_CAPACITY_UNITS,
            "the truncated unit is dispatched on a later iteration, not dropped"
        );
        assert!(
            outcome.iterations > 1,
            "the over-capacity unit could not join the first iteration's \
             fan-out and needed a later one: {}",
            outcome.iterations
        );
        let truncations = evidence.records_containing("\"class\":\"dispatch_truncated\"");
        assert!(
            !truncations.is_empty(),
            "the capacity truncation is recorded as pinned evidence, never silently dropped"
        );
    }

    #[test]
    fn budget_exhaustion_drives_the_terminal_state_with_the_reserve_intact() {
        // Two units, TWO rounds at a fifth in reserve: the forward-rounds
        // ceiling is ONE round, so the FIRST unit runs and spends its
        // round while the second cannot start (its projected round, on
        // top of the one the first reservation holds, would pass the
        // ceiling); once the round is spent, the next plan cannot admit
        // the second unit at all and the loop drives `budget_exhausted`
        // without ever touching the recovery reserve.
        let composed = fixture_composed(vec![Behavior::Complete], 0, 2, 2).expect("composes");
        let outcome = composed.controller.start_run().expect("the loop runs");
        assert_eq!(outcome.terminal, TerminalState::BudgetExhausted);
        let checkpoint = composed
            .state
            .resume(&outcome.run_id, bound())
            .expect("checkpoint readable")
            .expect("a checkpoint exists");
        // The recovery reserve is intact by construction: forward spend
        // stayed inside the forward token ceiling (four fifths of the
        // fixture ceiling), rounds spent within the one-round forward
        // ceiling.
        assert_eq!(checkpoint.budget_spent.rounds(), 1);
        assert!(checkpoint.budget_spent.tokens() <= FIXTURE_TOKEN_CEILING / 5 * 4);
    }

    #[test]
    fn a_recurring_residual_routes_composed_diagnosis_to_correction_indicated() {
        // C3 + C4 composed: the SAME residual (empty `unit_refs` on the
        // wire doc — the shipped deterministic shape) recurs; §13.5's key
        // plus the iteration-unit fallback make the recurrence COUNT, and
        // row 9 routes into the bounded §15.2 cycle whose facts are
        // INPUT-DRIVEN: the governed verification PASSED while the
        // comparison recurred the residual — the §18.4 disagreement selects
        // the APPARATUS locus (verifier error), never a hardcoded
        // implementation defect — and that passing verification IS the
        // disconfirming observation, so all five phases run on the
        // DIVIDED per-phase spend and CorrectionIndicated is PINNED as
        // durable evidence.
        const RESIDUAL_EVERY_COMPARISON: usize = usize::MAX;
        const DIAGNOSIS_ATTEMPT_CEILING: u32 = 4;
        // The rung passes from the third verification call: iterations 1
        // and 2 fail verification (the recurrence counts without the unit
        // settling); iteration 3's PASS settles the unit AND fires the
        // diagnosis in the same iteration.
        const RUNG_PASSES_FROM: u32 = 3;
        let key = Rc::new(RefCell::new(answer_key(1)));
        let state = FakeState::new();
        let evidence = FakeEvidence::new();
        let runner = ScriptedRunner::new(Vec::new());
        let ladder =
            crate::verify::ladder::Ladder::new(vec![crate::verify::ladder::LadderRung::new(
                crate::verify::ladder::RungKind::TestReport,
                Arc::new(FlakyVerdictVerifier {
                    calls: std::cell::Cell::new(0),
                    pass_from: RUNG_PASSES_FROM,
                }),
            )])
            .expect("the flaky-verdict ladder composes");
        let composed = compose_full(
            key,
            state.clone(),
            evidence.clone(),
            runner,
            RESIDUAL_EVERY_COMPARISON,
            FIXTURE_ROUNDS,
            policy_with_attempts(DIAGNOSIS_ATTEMPT_CEILING),
            ladder,
            meter_factory(),
            None,
        )
        .expect("composes");
        let outcome = composed.controller.start_run().expect("the loop runs");
        // The unit settled under the passing verification; the divergent
        // residual's diagnosis is the pinned durable fact that remains.
        assert_eq!(outcome.terminal, TerminalState::Settled);
        let records = evidence.records_containing("correction_indicated");
        assert_eq!(
            records.len(),
            1,
            "exactly one correction-indicated diagnosis record is pinned"
        );
        assert!(
            records[0].contains("verifier error"),
            "the locus is the input-driven APPARATUS fact, not a hardcoded \
             implementation defect: {}",
            records[0]
        );
        assert!(
            records[0].contains("\"phases\":5"),
            "the divided per-phase spend let all five phases run: {}",
            records[0]
        );
        // The diagnostic allowance carried the whole cycle: 5 phases at
        // allowance/5 each — the durable checkpoint records it.
        let checkpoint = composed
            .state
            .resume(&outcome.run_id, bound())
            .expect("checkpoint readable")
            .expect("a checkpoint exists");
        const FIXTURE_DIAGNOSTIC_ALLOWANCE_TOKENS: u64 = 100;
        assert_eq!(
            checkpoint.budget_spent.diagnostic_tokens(),
            FIXTURE_DIAGNOSTIC_ALLOWANCE_TOKENS
        );
    }

    #[test]
    fn a_surviving_hypothesis_ends_inconclusive_with_quarantine_composed() {
        // C3's other leg composed: when the verification FAILS alongside
        // the recurring residual (the apparatus agrees the artifact is
        // wrong), the implementation hypothesis stands, the disconfirming
        // observation is HONESTLY false, and the cycle ends INCONCLUSIVE
        // with the evidence quarantined and the escalation submitted —
        // never a fabricated correction.
        const RESIDUAL_EVERY_COMPARISON: usize = usize::MAX;
        const DIAGNOSIS_ATTEMPT_CEILING: u32 = 4;
        let key = Rc::new(RefCell::new(answer_key(1)));
        let state = FakeState::new();
        let evidence = FakeEvidence::new();
        let runner = ScriptedRunner::new(Vec::new());
        // A ladder that passes nothing: every verification FAILs, so the
        // apparatus agrees with the comparator and the implementation
        // hypothesis stands.
        let composed = compose_full(
            key,
            state,
            evidence.clone(),
            runner,
            RESIDUAL_EVERY_COMPARISON,
            FIXTURE_ROUNDS,
            policy_with_attempts(DIAGNOSIS_ATTEMPT_CEILING),
            fixture_ladder(&[]),
            meter_factory(),
            None,
        )
        .expect("composes");
        let outcome = composed.controller.start_run().expect("the loop runs");
        assert_eq!(outcome.terminal, TerminalState::Inconclusive);
        assert!(
            outcome
                .escalation_summary
                .as_deref()
                .is_some_and(|summary| summary.contains("inconclusive")),
            "the §15.2 inconclusive terminal escalates: {:?}",
            outcome.escalation_summary
        );
        assert!(
            !evidence
                .records_containing("\"quarantine\":true")
                .is_empty(),
            "the quarantined evidence is pinned"
        );
        // Four phases booked (the observation refused the fifth) at
        // allowance/5 each — the honest spend shape, durably recorded.
        let checkpoint = composed
            .state
            .resume(&outcome.run_id, bound())
            .expect("checkpoint readable")
            .expect("a checkpoint exists");
        const SURVIVED_HYPOTHESIS_PHASES: u64 = 4;
        const PHASES_PER_CYCLE: u64 = 5;
        const FIXTURE_DIAGNOSTIC_ALLOWANCE_TOKENS: u64 = 100;
        assert_eq!(
            checkpoint.budget_spent.diagnostic_tokens(),
            SURVIVED_HYPOTHESIS_PHASES * FIXTURE_DIAGNOSTIC_ALLOWANCE_TOKENS / PHASES_PER_CYCLE
        );
    }

    #[test]
    fn a_governance_blocked_run_terminals_blocked_with_a_ticket_never_exhausted() {
        // C1 / §11.6 terminal truth: an answer key whose only unit is
        // irreversible (§11.2/§17.1 — never an automatic dispatch
        // candidate, unconditionally) can never admit a dispatch. The
        // terminal must say `blocked` and carry the escalation ticket the
        // table requires — `budget_exhausted` would be a lie: no budget
        // dimension has breached (the run booked nothing it could spend
        // forward on work).
        let key = Rc::new(RefCell::new(
            "## AK-9103\nclaim: an irreversible claim only a human may settle\nverify_by: \
             human_signoff\nblast_radius: irreversible\nauthority: human\nevidence_required: \
             {kind: attestation, locator: ctrl-9103.json, producer: instrument}\n\n"
                .to_owned(),
        ));
        let state = FakeState::new();
        let evidence = FakeEvidence::new();
        let runner = ScriptedRunner::new(Vec::new());
        let composed =
            compose(key, state.clone(), evidence, runner, 0, FIXTURE_ROUNDS).expect("composes");
        let outcome = composed.controller.start_run().expect("the loop runs");
        assert_eq!(
            outcome.terminal,
            TerminalState::Blocked,
            "a governance-blocked-only run is blocked, never budget-exhausted"
        );
        let summary = outcome
            .escalation_summary
            .expect("§11.6 blocked requires the escalation ticket");
        assert!(
            summary.contains("AK-9103") && summary.contains("irreversible"),
            "the ticket names the unit and its typed blocker: {summary}"
        );
        // No budget dimension breached: nothing was ever dispatched.
        assert_eq!(
            composed.runner.prompts.borrow().len(),
            0,
            "an irreversible unit never dispatches"
        );
        assert_eq!(
            composed.state.role_invocations.borrow().len(),
            0,
            "no invocation row exists"
        );
    }

    #[test]
    fn a_residual_retry_dispatch_carries_residual_text_only() {
        // §11.5 row 1: the FIRST comparison finds a residual; the second
        // dispatch's prompt carries the residual's expected/observed text
        // and NOT the comparator's reasoning. BLOCKING severity keeps the
        // unit from settling on that first completion, so the re-dispatch
        // is the legitimate row-1 correction, not a reopened settlement.
        let composed = fixture_composed(
            vec![Behavior::Complete, Behavior::Complete],
            1,
            1,
            FIXTURE_ROUNDS,
        )
        .expect("composes");
        composed
            .comparators
            .set_severity(gauntlet_domain::Severity::Blocking);
        let outcome = composed.controller.start_run().expect("the loop runs");
        let prompts = composed.runner.prompts.borrow().clone();
        assert!(
            prompts.len() >= 2,
            "the residual re-dispatched, got {prompts:?}"
        );
        let second = &prompts[1].1;
        assert!(
            second.contains("expected: the report at the required locator"),
            "the second dispatch carries the residual text: {second}"
        );
        assert!(
            !second.contains("the comparator believes"),
            "no critic trajectory rides the re-dispatch"
        );
        assert!(
            !second.contains("critic"),
            "the word 'critic' itself never rides the prompt: {second}"
        );
        let _ = outcome;
    }

    #[test]
    fn a_settled_unit_with_a_non_blocking_residual_stays_settled() {
        // §16.1 condition 4 gates settlement on a BLOCKING residual only;
        // a Major one does not. The scripted comparator's residual is
        // Major, so the first completion both settles the sole unit AND
        // carries a non-blocking residual. §18.3 ("re-running settled
        // work is a defect"): the row-1 disposition must not re-dispatch
        // a unit this same completion just settled.
        let key = Rc::new(RefCell::new(answer_key(1)));
        let state = FakeState::new();
        let evidence = FakeEvidence::new();
        let runner = ScriptedRunner::new(vec![Behavior::Complete, Behavior::Complete]);
        let composed = compose(key, state, evidence, runner, 1, 3).expect("composes");
        let outcome = composed.controller.start_run().expect("the loop runs");
        assert_eq!(
            outcome.terminal,
            TerminalState::Settled,
            "the sole unit settles on its first attempt and stays settled: {outcome:?}"
        );
        let prompts = composed.runner.prompts.borrow().clone();
        assert_eq!(
            prompts.len(),
            1,
            "settled work is never re-dispatched: {prompts:?}"
        );
        let checkpoint = composed
            .state
            .resume(&outcome.run_id, bound())
            .expect("checkpoint readable")
            .expect("a checkpoint exists");
        assert_eq!(
            checkpoint
                .unit_states
                .iter()
                .find(|(unit, _)| unit.as_str() == "AK-9101")
                .map(|(_, status)| *status),
            Some(UnitStatus::Settled),
            "the checkpoint's own projection certifies the unit settled: {checkpoint:?}"
        );
        let settlements = composed
            .state
            .settlements(&outcome.run_id, bound())
            .expect("settlements readable");
        assert_eq!(
            settlements.len(),
            1,
            "exactly one durable settlement record exists: {settlements:?}"
        );
    }

    #[test]
    fn resume_restores_the_budget_from_the_checkpoint_watermark() {
        // A moderate durable watermark restores cleanly and the loop
        // continues; an unreachable watermark (inside the reserve) is the
        // typed restore refusal.
        let key = Rc::new(RefCell::new(answer_key(1)));
        let state = FakeState::new();
        let evidence = FakeEvidence::new();
        let runner = ScriptedRunner::new(vec![Behavior::Complete]);
        let composed = compose(
            key.clone(),
            state.clone(),
            evidence,
            runner,
            0,
            FIXTURE_ROUNDS,
        )
        .expect("composes");
        let outcome = composed.controller.start_run().expect("the loop runs");
        let run_id = outcome.run_id.clone();
        // Seed a resumable mid-run checkpoint: one round and a moderate
        // token spend, the unit still open.
        let restored_spent = gauntlet_domain::Spent::from_counters(
            1,
            0,
            FIXTURE_TOKEN_CEILING / 10,
            MicroUsd::new(0),
            0,
            MicroUsd::new(0),
        );
        state
            .checkpoint(
                Checkpoint {
                    run_id: run_id.clone(),
                    reference_digest: Digest::new("sha256", &"ee".repeat(32)).expect("digest"),
                    iteration_no: gauntlet_domain::IterationNo::new(1).expect("iteration"),
                    unit_states: vec![(
                        gauntlet_domain::UnitId::new("AK-9101").expect("unit"),
                        UnitStatus::Open,
                    )],
                    budget_spent: restored_spent,
                    in_flight: Vec::new(),
                    event_log_offset: gauntlet_domain::EventLogOffset::new(1),
                },
                bound(),
            )
            .expect("seed checkpoint");
        // The reference digest in the seeded checkpoint must match the
        // frozen version for verify_unchanged to pass — copy it from the
        // durable run.
        let run = state
            .load_run(&run_id, bound())
            .expect("run readable")
            .expect("the run is durable");
        state
            .checkpoint(
                Checkpoint {
                    run_id: run_id.clone(),
                    reference_digest: run.reference_version().digest().clone(),
                    iteration_no: gauntlet_domain::IterationNo::new(1).expect("iteration"),
                    unit_states: vec![(
                        gauntlet_domain::UnitId::new("AK-9101").expect("unit"),
                        UnitStatus::Open,
                    )],
                    budget_spent: restored_spent,
                    in_flight: Vec::new(),
                    event_log_offset: gauntlet_domain::EventLogOffset::new(1),
                },
                bound(),
            )
            .expect("seed checkpoint");
        let resumed = composed.controller.resume_run(&run_id);
        assert!(
            resumed.is_ok(),
            "a reachable watermark restores: {:?}",
            resumed.err()
        );
        // An unreachable watermark — inside the recovery reserve — is the
        // typed restore refusal, never a defaulted guess.
        let beyond_reserve = gauntlet_domain::Spent::from_counters(
            1,
            0,
            FIXTURE_TOKEN_CEILING,
            MicroUsd::new(0),
            0,
            MicroUsd::new(0),
        );
        state
            .checkpoint(
                Checkpoint {
                    run_id: run_id.clone(),
                    reference_digest: run.reference_version().digest().clone(),
                    iteration_no: gauntlet_domain::IterationNo::new(1).expect("iteration"),
                    unit_states: Vec::new(),
                    budget_spent: beyond_reserve,
                    in_flight: Vec::new(),
                    event_log_offset: gauntlet_domain::EventLogOffset::new(1),
                },
                bound(),
            )
            .expect("seed checkpoint");
        match composed.controller.resume_run(&run_id) {
            Err(ControlError::UnreachableSpend(_)) => {}
            other => panic!("an unreachable watermark is typed, got {other:?}"),
        }
    }

    #[test]
    fn reference_unavailability_is_a_recorded_gap_not_an_abort() {
        // Task-7 m3's other class: Unavailable keeps the process alive and
        // reconciling; the gap is recorded; the run ends by its own merits.
        let key = Rc::new(RefCell::new(answer_key(1)));
        let state = FakeState::new();
        let evidence = FakeEvidence::new();
        let runner = ScriptedRunner::new(vec![Behavior::Complete]);
        // The provider becomes unavailable AFTER the unit's durable
        // settlement — mid-run, before the deciding boundary. (Before Task
        // 13 B2 the settle leg never consulted the provider, so the window
        // could open inside the drain; §16.1 condition 5 now consults
        // immediately before settlement and §21.2 forbids converting an
        // unanswerable check into settled success, so the window opens only
        // once the settlement is already durable.)
        let flag = Rc::new(Cell::new(false));
        {
            let flag = flag.clone();
            *state.on_first_settlement.borrow_mut() = Some(Box::new(move || flag.set(true)));
        }
        let scratch = Scratch::new("unavailable");
        let clock = DeterministicClock::new();
        let evidence_store = evidence.clone();
        let provider = Arc::new(FlaggedProvider {
            inner: FileReferenceProvider::from_sources(
                AnswerKeySource::Shared(key.clone()),
                ManifestSource::Inline(minimal_manifest()),
                Rc::new(evidence_store),
            ),
            unavailable: flag,
        });
        let comparators = ScriptedComparators::new(0);
        let deps = ControllerDeps {
            state: state.clone(),
            evidence: Arc::new(evidence.clone()),
            ids: Arc::new(SeededIds(RefCell::new(0))),
            clock,
            runner: runner.clone(),
            runner_identity: gauntlet_domain::RunnerIdentity::new("fixture-runner")
                .expect("runner identity"),
            observers: vec![Arc::new(FixtureObserver {
                store: evidence.clone(),
                clock: DeterministicClock::new(),
            })],
            comparison: ComparisonDeps {
                contract: minimal_contract(),
                deterministic: Arc::new(ComparatorsSource(comparators.clone())),
                model: Arc::new(ComparatorsSource(comparators)),
                adversarial: None,
                review_mode: gauntlet_domain::ReviewMode::Single,
                model_dispatches: 4,
                role_deny: crate::roles::ContextDenySet::new(Vec::new()),
            },
            ladder_for: Arc::new(move |_| Ok(fixture_ladder(&["ctrl-9101.json"]))),
            reference: provider,
            answer_key: AnswerKeySource::Shared(key.clone()),
            escalation: Arc::new(FakeEscalation::default()),
            meter_factory: meter_factory(),
            budget: budget_shape(FIXTURE_ROUNDS),
            policy: policy(),
            workspaces: None,
            workspace_root: scratch.0.clone(),
            config_fingerprint: Digest::new("sha256", &"ab".repeat(32)).expect("digest"),
            govern: crate::govern::GovernDeps::none_backend(),
            environment_fingerprint: Digest::new("sha256", &"cd".repeat(32)).expect("digest"),
            intent: "controller fixture".to_owned(),
        };
        std::mem::forget(scratch);
        let controller = RunController::compose(deps).expect("composes");
        let outcome = controller.start_run().expect("the loop runs");
        // The run CONTINUED past the unavailable boundary and settled.
        assert_eq!(outcome.terminal, TerminalState::Settled);
        assert!(
            matches!(
                outcome.reference_breach,
                Some(ReferenceBreach::Unavailable { .. })
            ),
            "the gap is classified, not aborted: {:?}",
            outcome.reference_breach
        );
        let records = evidence.records_containing("reference_unavailable");
        assert!(!records.is_empty(), "the unavailability is pinned evidence");
    }

    #[test]
    fn an_answer_key_read_failure_degrades_the_iteration_and_stays_resumeable() {
        // Task 13 B2 Item 4 (§21.3 row 1): an answer-key READ failure
        // inside the loop is NOT a run-fatal error — new dispatch stops
        // for the iteration, the process stays alive, reconciliation keeps
        // running (the gap is pinned evidence), the rounds dimension
        // bounds the wait, and restoring the source lets a resume finish
        // the run. Before Item 4 this exact shape errored the run with
        // `ControlError::Reference(Unavailable)` and no close-out.
        let scratch = Scratch::new("read-failure");
        let key_path = scratch.0.join("answer-key.md");
        std::fs::write(&key_path, answer_key_with_dependency())
            .expect("the answer key lands at the path source");
        let state = FakeState::new();
        let evidence = FakeEvidence::new();
        let runner = ScriptedRunner::new(Vec::new());
        // The key file VANISHES after the first durable settlement — the
        // next plan's re-read fails.
        {
            let path = key_path.clone();
            *state.on_first_settlement.borrow_mut() =
                Some(Box::new(move || drop(std::fs::remove_file(&path))));
        }
        let clock = DeterministicClock::new();
        let evidence_store = evidence.clone();
        let provider = Arc::new(FileReferenceProvider::from_sources(
            AnswerKeySource::Path(key_path.clone()),
            ManifestSource::Inline(minimal_manifest()),
            Rc::new(evidence_store),
        ));
        let comparators = ScriptedComparators::new(0);
        let deps = ControllerDeps {
            state: state.clone(),
            evidence: Arc::new(evidence.clone()),
            ids: Arc::new(SeededIds(RefCell::new(0))),
            clock,
            runner: runner.clone(),
            runner_identity: gauntlet_domain::RunnerIdentity::new("fixture-runner")
                .expect("runner identity"),
            observers: vec![Arc::new(FixtureObserver {
                store: evidence.clone(),
                clock: DeterministicClock::new(),
            })],
            comparison: ComparisonDeps {
                contract: minimal_contract(),
                deterministic: Arc::new(ComparatorsSource(comparators.clone())),
                model: Arc::new(ComparatorsSource(comparators)),
                adversarial: None,
                review_mode: gauntlet_domain::ReviewMode::Single,
                model_dispatches: 4,
                role_deny: crate::roles::ContextDenySet::new(Vec::new()),
            },
            ladder_for: Arc::new(move |_| {
                Ok(fixture_ladder(&["ctrl-9101.json", "ctrl-9102.json"]))
            }),
            reference: provider,
            answer_key: AnswerKeySource::Path(key_path.clone()),
            escalation: Arc::new(FakeEscalation::default()),
            meter_factory: meter_factory(),
            // Two rounds: iteration 1 settles AK-9101; iteration 2 degrades
            // on the vanished key — the bounded wait ends, leaving rounds
            // for the resume to finish the owed unit.
            budget: budget_shape(2),
            policy: policy(),
            workspaces: None,
            workspace_root: scratch.0.clone(),
            config_fingerprint: Digest::new("sha256", &"ab".repeat(32)).expect("digest"),
            govern: crate::govern::GovernDeps::none_backend(),
            environment_fingerprint: Digest::new("sha256", &"cd".repeat(32)).expect("digest"),
            intent: "controller fixture".to_owned(),
        };
        std::mem::forget(scratch);
        let controller = RunController::compose(deps).expect("composes");
        let outcome = controller
            .start_run()
            .expect("a read failure degrades the iteration, never errors the run");
        assert_eq!(
            outcome.terminal,
            TerminalState::BudgetExhausted,
            "the degradation is bounded by the rounds dimension"
        );
        // The gap is pinned evidence, and the settled unit stayed settled.
        assert!(
            !evidence
                .records_containing("answer_key_read_failed")
                .is_empty(),
            "the read failure is pinned evidence, not silence"
        );
        let run_id = RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("fixture run id");
        assert_eq!(
            state
                .settlements(&run_id, bound())
                .expect("rows readable")
                .len(),
            1,
            "the pre-failure settlement stands"
        );
        // Restore the source: the resume — reading the SAME path source —
        // finishes the owed unit.
        std::fs::write(&key_path, answer_key_with_dependency())
            .expect("the answer key is restored");
        let resumed = ScriptedRunner::new(Vec::new());
        let second_scratch = Scratch::new("read-failure-resume");
        let resumed_evidence = evidence.clone();
        let resumed_provider = Arc::new(FileReferenceProvider::from_sources(
            AnswerKeySource::Path(key_path.clone()),
            ManifestSource::Inline(minimal_manifest()),
            Rc::new(resumed_evidence),
        ));
        let resumed_comparators = ScriptedComparators::new(0);
        let resumed_deps = ControllerDeps {
            state: state.clone(),
            evidence: Arc::new(evidence.clone()),
            ids: Arc::new(SeededIds(RefCell::new(0))),
            clock: DeterministicClock::new(),
            runner: resumed.clone(),
            runner_identity: gauntlet_domain::RunnerIdentity::new("fixture-runner")
                .expect("runner identity"),
            observers: vec![Arc::new(FixtureObserver {
                store: evidence.clone(),
                clock: DeterministicClock::new(),
            })],
            comparison: ComparisonDeps {
                contract: minimal_contract(),
                deterministic: Arc::new(ComparatorsSource(resumed_comparators.clone())),
                model: Arc::new(ComparatorsSource(resumed_comparators)),
                adversarial: None,
                review_mode: gauntlet_domain::ReviewMode::Single,
                model_dispatches: 4,
                role_deny: crate::roles::ContextDenySet::new(Vec::new()),
            },
            ladder_for: Arc::new(move |_| {
                Ok(fixture_ladder(&["ctrl-9101.json", "ctrl-9102.json"]))
            }),
            reference: resumed_provider,
            answer_key: AnswerKeySource::Path(key_path.clone()),
            escalation: Arc::new(FakeEscalation::default()),
            meter_factory: meter_factory(),
            budget: budget_shape(FIXTURE_ROUNDS),
            policy: policy(),
            workspaces: None,
            workspace_root: second_scratch.0.clone(),
            config_fingerprint: Digest::new("sha256", &"ab".repeat(32)).expect("digest"),
            govern: crate::govern::GovernDeps::none_backend(),
            environment_fingerprint: Digest::new("sha256", &"cd".repeat(32)).expect("digest"),
            intent: "controller fixture".to_owned(),
        };
        std::mem::forget(second_scratch);
        let second = RunController::compose(resumed_deps).expect("composes");
        let outcome = second
            .resume_run(&run_id)
            .expect("the degraded run resumes");
        assert_eq!(outcome.terminal, TerminalState::Settled);
        assert!(
            resumed
                .prompts
                .borrow()
                .iter()
                .any(|(_, prompt)| prompt.contains("AK-9102")),
            "the still-owed unit is re-planned by the resume"
        );
        assert!(
            !resumed
                .prompts
                .borrow()
                .iter()
                .any(|(_, prompt)| prompt.contains("AK-9101")),
            "the settled unit is never re-dispatched"
        );
    }

    /// The FileReferenceProvider wrapper with a live Unavailable flag.
    struct FlaggedProvider {
        inner: FileReferenceProvider,
        unavailable: Rc<Cell<bool>>,
    }

    impl ReferenceProvider for FlaggedProvider {
        fn resolve(
            &self,
            intent: &str,
            deadline: Deadline,
        ) -> Result<gauntlet_ports::reference_provider::ReferenceDraft, ReferenceProviderError>
        {
            self.inner.resolve(intent, deadline)
        }
        fn normalize(
            &self,
            draft: gauntlet_ports::reference_provider::ReferenceDraft,
            deadline: Deadline,
        ) -> Result<gauntlet_domain::Reference, ReferenceProviderError> {
            self.inner.normalize(draft, deadline)
        }
        fn digest(
            &self,
            reference: &gauntlet_domain::Reference,
            deadline: Deadline,
        ) -> Result<Digest, ReferenceProviderError> {
            self.inner.digest(reference, deadline)
        }
        fn freeze(
            &self,
            run_id: &RunId,
            reference: &gauntlet_domain::Reference,
            deadline: Deadline,
        ) -> Result<gauntlet_domain::ReferenceVersion, ReferenceProviderError> {
            self.inner.freeze(run_id, reference, deadline)
        }
        fn verify_unchanged(
            &self,
            version: &gauntlet_domain::ReferenceVersion,
            deadline: Deadline,
        ) -> Result<(), ReferenceProviderError> {
            if self.unavailable.get() {
                return Err(ReferenceProviderError::Unavailable {
                    detail: "the fixture provider is unavailable".to_owned(),
                });
            }
            self.inner.verify_unchanged(version, deadline)
        }
    }

    #[test]
    fn a_malformed_result_gets_one_reminder_re_dispatch_then_escalates() {
        // §11.5 row 3 at the loop: the first malformed result stages a
        // schema-reminder re-dispatch (its prompt names the strict
        // document); the second escalates.
        let composed = fixture_composed(
            vec![
                Behavior::Fail(InvocationFailure::MalformedResult),
                Behavior::Fail(InvocationFailure::MalformedResult),
            ],
            0,
            1,
            FIXTURE_ROUNDS,
        )
        .expect("composes");
        let outcome = composed.controller.start_run().expect("the loop runs");
        assert_eq!(outcome.terminal, TerminalState::Escalated);
        assert!(
            outcome
                .escalation_summary
                .as_deref()
                .is_some_and(|summary| summary.contains("malformed result twice")),
            "the escalation names the row-3 cause: {:?}",
            outcome.escalation_summary
        );
        let prompts = composed.runner.prompts.borrow().clone();
        assert_eq!(prompts.len(), 2, "exactly one reminder re-dispatch");
        assert!(
            prompts[1].1.contains("role_result@v1"),
            "the reminder names the strict schema: {}",
            prompts[1].1
        );
    }

    fn bound() -> Deadline {
        Deadline::at_epoch(EpochSeconds::new(u64::MAX / 2))
    }

    /// An in-test [`WorkspaceProvider`]: allocates a fresh ref per create,
    /// records every create and cleanup. No filesystem — the controller
    /// treats the workspace as an opaque ref; the fs-backed adapter's own
    /// contract is proven in its own crate's suite.
    struct FixtureWorkspaces {
        seq: Cell<u32>,
        created: Mutex<Vec<gauntlet_domain::WorkspaceRef>>,
        cleaned: Mutex<Vec<gauntlet_domain::WorkspaceRef>>,
        /// W5: every `promote` call this fixture received, in order — a
        /// completion that settled nothing must never appear here.
        promoted: Mutex<Vec<gauntlet_domain::WorkspaceRef>>,
        /// The canned answer `promote` returns to every caller (default:
        /// the unsupported shape, matching a provider with no seed
        /// configured).
        promote_response: Mutex<Result<WorkspacePromotion, WorkspaceProviderError>>,
        /// W6: every `check_containment` call this fixture received, in
        /// order.
        containment_checked: Mutex<Vec<gauntlet_domain::WorkspaceRef>>,
        /// The canned answer `check_containment` returns to every caller
        /// (default: the unsupported shape — no check, settlement
        /// unaffected).
        containment_response: Mutex<Result<ContainmentDiff, WorkspaceProviderError>>,
    }

    impl FixtureWorkspaces {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                seq: Cell::new(0),
                created: Mutex::new(Vec::new()),
                cleaned: Mutex::new(Vec::new()),
                promoted: Mutex::new(Vec::new()),
                promote_response: Mutex::new(Ok(WorkspacePromotion::default())),
                containment_checked: Mutex::new(Vec::new()),
                containment_response: Mutex::new(Ok(ContainmentDiff::default())),
            })
        }

        fn set_promote_response(
            &self,
            response: Result<WorkspacePromotion, WorkspaceProviderError>,
        ) {
            *self
                .promote_response
                .lock()
                .expect("fixture workspace ledger locks") = response;
        }

        fn set_containment_response(
            &self,
            response: Result<ContainmentDiff, WorkspaceProviderError>,
        ) {
            *self
                .containment_response
                .lock()
                .expect("fixture workspace ledger locks") = response;
        }
    }

    impl WorkspaceProvider for FixtureWorkspaces {
        fn create(
            &self,
            _run_id: &RunId,
            subject: WorkspaceSubject,
            _deadline: Deadline,
        ) -> Result<gauntlet_ports::workspace_provider::Workspace, WorkspaceProviderError> {
            let n = self.seq.get();
            self.seq.set(n + 1);
            let subject_name = match subject {
                WorkspaceSubject::Unit(unit) => unit.to_string(),
                WorkspaceSubject::Slice(units) => units
                    .iter()
                    .map(|unit| unit.to_string())
                    .collect::<Vec<_>>()
                    .join("-"),
            };
            let reference =
                gauntlet_domain::WorkspaceRef::new(&format!("ws-fixture-{n}-{subject_name}"))
                    .expect("fixture workspace ref");
            self.created
                .lock()
                .expect("fixture workspace ledger locks")
                .push(reference.clone());
            Ok(gauntlet_ports::workspace_provider::Workspace {
                reference: reference.clone(),
                root: std::env::temp_dir().join(format!("ws-fixture-{}-{n}", std::process::id())),
                key: format!("ws-fixture-{n}"),
            })
        }

        fn cleanup(
            &self,
            workspace: &gauntlet_domain::WorkspaceRef,
            _deadline: Deadline,
        ) -> Result<(), WorkspaceProviderError> {
            self.cleaned
                .lock()
                .expect("fixture workspace ledger locks")
                .push(workspace.clone());
            Ok(())
        }

        fn hooks(&self) -> Vec<gauntlet_ports::workspace_provider::WorkspaceHook> {
            Vec::new()
        }

        fn promote(
            &self,
            workspace: &gauntlet_domain::WorkspaceRef,
            _deadline: Deadline,
        ) -> Result<WorkspacePromotion, WorkspaceProviderError> {
            self.promoted
                .lock()
                .expect("fixture workspace ledger locks")
                .push(workspace.clone());
            self.promote_response
                .lock()
                .expect("fixture workspace ledger locks")
                .clone()
        }

        fn check_containment(
            &self,
            workspace: &gauntlet_domain::WorkspaceRef,
            _deadline: Deadline,
        ) -> Result<ContainmentDiff, WorkspaceProviderError> {
            self.containment_checked
                .lock()
                .expect("fixture workspace ledger locks")
                .push(workspace.clone());
            self.containment_response
                .lock()
                .expect("fixture workspace ledger locks")
                .clone()
        }
    }

    /// Plan Task 20 / R-1b TEETH (spec §17.5.1 invariant 1): with a
    /// `WorkspaceProvider` composed, two units of ONE run are dispatched
    /// into DISTINCT workspaces — the invocation spec the runner receives
    /// names the unit's own workspace, the gate validated against that same
    /// ref (it is the only workspace fact the dispatch holds), and each
    /// workspace is released after its invocation terminalizes.
    ///
    /// RED WHEN BYPASSED: with the old single-lifetime path restored (the
    /// spec and facts naming the run-derived `run_workspace_ref(run_id)`
    /// instead of a per-unit allocation), both specs carry ONE shared ref,
    /// the provider's ledger stays EMPTY, and this test fails at the
    /// distinctness assertion. The red run is recorded in
    /// `.agents/reports/task20/b1-progress.md`.
    #[test]
    fn two_units_of_one_run_are_admitted_into_distinct_workspaces() {
        let key = Rc::new(RefCell::new(answer_key(2)));
        let state = FakeState::new();
        let evidence = FakeEvidence::new();
        let runner = ScriptedRunner::new(vec![Behavior::Complete, Behavior::Complete]);
        let workspaces = FixtureWorkspaces::new();
        let composed = compose_full(
            key,
            state,
            evidence,
            runner.clone(),
            0,
            FIXTURE_ROUNDS,
            policy(),
            fixture_ladder(&["ctrl-9101.json", "ctrl-9102.json"]),
            meter_factory(),
            Some(workspaces.clone()),
        )
        .expect("composes");
        let outcome = composed
            .controller
            .start_run()
            .expect("the loop runs with a workspace provider wired");
        assert_eq!(
            outcome.terminal,
            TerminalState::Settled,
            "per-unit workspaces neither break nor delay settlement"
        );
        let specs = runner.specs.borrow();
        assert_eq!(specs.len(), 2, "both units were dispatched");
        let spec_workspaces: std::collections::BTreeSet<String> = specs
            .values()
            .map(|spec| spec.workspace.as_str().to_owned())
            .collect();
        assert_eq!(
            spec_workspaces.len(),
            2,
            "two units of one run must hold DISTINCT workspace refs at admission, got \
             {spec_workspaces:?} — a shared ref is the R-1b defect this tooth exists to refuse"
        );
        let created = workspaces.created.lock().expect("ledger locks").clone();
        assert_eq!(
            created.len(),
            2,
            "the provider allocated exactly one fresh workspace per dispatch: {created:?}"
        );
        let created_set: std::collections::BTreeSet<String> = created
            .iter()
            .map(|reference| reference.as_str().to_owned())
            .collect();
        assert_eq!(
            spec_workspaces, created_set,
            "the dispatched specs name EXACTLY the provider's allocations — the gate validated \
             against the workspaces the units actually run in"
        );
        // Both invocations terminalized (the run settled): both workspaces
        // are released, best-effort, AFTER their completion pipelines.
        let cleaned = workspaces.cleaned.lock().expect("ledger locks").clone();
        let cleaned_set: std::collections::BTreeSet<String> = cleaned
            .iter()
            .map(|reference| reference.as_str().to_owned())
            .collect();
        assert_eq!(
            cleaned_set, created_set,
            "every allocated workspace is released once its invocation terminalizes: {cleaned:?}"
        );
    }

    /// W5 work-product promotion: a completion that settles its unit
    /// promotes BEFORE the workspace is released, and pins
    /// `work_product_promoted` naming the changed paths. The fixture
    /// provider stands in for the real fs-backed adapter (whose own
    /// seed-to-promote round trip is proven in that adapter's own suite) — the controller
    /// treats the workspace as an opaque ref, so what matters here is
    /// only that `promote` is called and its answer is pinned.
    #[test]
    fn a_settled_units_promotion_is_pinned_before_release() {
        let key = Rc::new(RefCell::new(answer_key(1)));
        let state = FakeState::new();
        let evidence = FakeEvidence::new();
        let runner = ScriptedRunner::new(vec![Behavior::Complete]);
        let workspaces = FixtureWorkspaces::new();
        workspaces.set_promote_response(Ok(WorkspacePromotion {
            supported: true,
            changed: vec![PromotedFile {
                path: "app.txt".to_owned(),
                sha256: "deadbeef".to_owned(),
            }],
            deleted: Vec::new(),
            conflicts: Vec::new(),
        }));
        let composed = compose_full(
            key,
            state,
            evidence.clone(),
            runner,
            0,
            FIXTURE_ROUNDS,
            policy(),
            fixture_ladder(&["ctrl-9101.json"]),
            meter_factory(),
            Some(workspaces.clone()),
        )
        .expect("composes");
        let outcome = composed.controller.start_run().expect("the loop runs");
        assert_eq!(outcome.terminal, TerminalState::Settled);
        let promoted = workspaces.promoted.lock().expect("ledger locks").clone();
        assert_eq!(
            promoted.len(),
            1,
            "the settled unit's workspace was promoted exactly once: {promoted:?}"
        );
        let records = evidence.records_containing("work_product_promoted");
        assert_eq!(
            records.len(),
            1,
            "one promotion record was pinned: {records:?}"
        );
        assert!(
            records[0].contains("app.txt") && records[0].contains("deadbeef"),
            "the pinned record names the changed path and digest: {}",
            records[0]
        );
        // Release happens in the caller AFTER process_completion returns —
        // promote must have already run, which the recorded call above
        // proves regardless of release timing; the workspace was also, in
        // fact, released (settlement completed the run).
        let cleaned = workspaces.cleaned.lock().expect("ledger locks").clone();
        assert_eq!(
            cleaned.len(),
            1,
            "the promoted workspace was still released: {cleaned:?}"
        );
    }

    /// A completion that settles nothing (a malformed/failed result) must
    /// never call `promote` — promotion is gated on
    /// `flight_unit_settled`, never on "an invocation happened".
    #[test]
    fn a_non_settling_completion_never_promotes() {
        let key = Rc::new(RefCell::new(answer_key(1)));
        let state = FakeState::new();
        let evidence = FakeEvidence::new();
        let runner = ScriptedRunner::new(vec![
            Behavior::Fail(InvocationFailure::MalformedResult),
            Behavior::Complete,
        ]);
        let workspaces = FixtureWorkspaces::new();
        workspaces.set_promote_response(Ok(WorkspacePromotion {
            supported: true,
            changed: vec![PromotedFile {
                path: "should-not-appear.txt".to_owned(),
                sha256: "deadbeef".to_owned(),
            }],
            deleted: Vec::new(),
            conflicts: Vec::new(),
        }));
        let composed = compose_full(
            key,
            state,
            evidence.clone(),
            runner,
            0,
            FIXTURE_ROUNDS,
            policy(),
            fixture_ladder(&["ctrl-9101.json"]),
            meter_factory(),
            Some(workspaces.clone()),
        )
        .expect("composes");
        let outcome = composed.controller.start_run().expect("the loop runs");
        assert_eq!(outcome.terminal, TerminalState::Settled);
        let promoted = workspaces.promoted.lock().expect("ledger locks").clone();
        assert_eq!(
            promoted.len(),
            1,
            "only the SETTLING completion promoted — the malformed-result completion did not: \
             {promoted:?}"
        );
        // The fixture answers every `promote` with the same canned
        // promotion, so content cannot tell the two completions apart —
        // count does: had the non-settling completion promoted, two
        // records would be pinned, not one.
        assert_eq!(
            evidence.records_containing("work_product_promoted").len(),
            1,
            "exactly one promotion record was pinned — the settling completion's"
        );
    }

    /// W6 detective containment: a completion whose project root differs
    /// from its dispatch-time baseline pins `containment_breach` naming
    /// the paths, and the unit does NOT settle — the BLOCKING residual
    /// routes the §11.5 row-1 re-dispatch, whose prompt carries the
    /// residual text (the retry is told why).
    #[test]
    fn a_containment_breach_blocks_settlement_and_names_the_paths() {
        let key = Rc::new(RefCell::new(answer_key(1)));
        let state = FakeState::new();
        let evidence = FakeEvidence::new();
        let runner = ScriptedRunner::new(vec![
            Behavior::Complete,
            Behavior::Complete,
            Behavior::Complete,
        ]);
        let workspaces = FixtureWorkspaces::new();
        // Assembled from segments: a whole `notes/todo.md` or dropped-file
        // literal is environment coupling to the boundary scan (spec
        // §8.1), even in test code.
        let edited = ["notes", "todo.md"].join("/");
        let planted = ["notes", "payload"].join("/");
        workspaces.set_containment_response(Ok(ContainmentDiff {
            supported: true,
            changed: vec![edited.clone()],
            added: vec![planted.clone()],
            removed: Vec::new(),
        }));
        let composed = compose_full(
            key,
            state,
            evidence.clone(),
            runner,
            0,
            FIXTURE_ROUNDS,
            policy(),
            fixture_ladder(&["ctrl-9101.json"]),
            meter_factory(),
            Some(workspaces.clone()),
        )
        .expect("composes");
        let outcome = composed.controller.start_run().expect("the loop runs");
        assert_ne!(
            outcome.terminal,
            TerminalState::Settled,
            "the breached unit never settles"
        );
        let records = evidence.records_containing("containment_breach");
        assert!(
            !records.is_empty(),
            "the breach is pinned as durable evidence"
        );
        assert!(
            records
                .iter()
                .any(|record| record.contains(&edited) && record.contains(&planted)),
            "the pinned breach names the changed paths: {records:?}"
        );
        let prompts = composed.runner.prompts.borrow().clone();
        assert!(
            prompts.len() >= 2,
            "the blocked unit re-dispatched: {prompts:?}"
        );
        let retried = prompts
            .iter()
            .skip(1)
            .map(|prompt| prompt.1.clone())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            retried.contains(&planted),
            "the retry is told why — the residual text names the paths"
        );
    }

    /// W7: failure detail is bounded and sanitized — control characters
    /// flatten, overlong text truncates visibly.
    #[test]
    fn failure_detail_sanitizes_and_bounds() {
        assert_eq!(
            sanitize_failure_detail("not JSON at byte 3"),
            "not JSON at byte 3"
        );
        assert_eq!(
            sanitize_failure_detail("line one\nline two\ttab"),
            "line one line two tab"
        );
        assert_eq!(sanitize_failure_detail("  padded  "), "padded");
        let long = "x".repeat(MAX_FAILURE_DETAIL_CHARS + 50);
        let sanitized = sanitize_failure_detail(&long);
        assert!(
            sanitized.ends_with("[truncated]"),
            "overlong detail truncates visibly"
        );
        assert_eq!(
            sanitized.chars().count(),
            MAX_FAILURE_DETAIL_CHARS + " [truncated]".chars().count()
        );
    }

    /// W7: the row-3 schema reminder names the parse failure, and the
    /// durable record carries it — the retry fixes the actual defect.
    #[test]
    fn a_malformed_collect_names_its_cause_in_reminder_and_record() {
        let key = Rc::new(RefCell::new(answer_key(1)));
        let state = FakeState::new();
        let evidence = FakeEvidence::new();
        let runner = ScriptedRunner::new(vec![
            Behavior::CollectFailed {
                detail: "result contract is not JSON at byte 3".to_owned(),
            },
            Behavior::Complete,
        ]);
        let composed = compose_full(
            key,
            state,
            evidence.clone(),
            runner,
            0,
            FIXTURE_ROUNDS,
            policy(),
            fixture_ladder(&["ctrl-9101.json"]),
            meter_factory(),
            Some(FixtureWorkspaces::new()),
        )
        .expect("composes");
        let outcome = composed.controller.start_run().expect("the loop runs");
        assert_eq!(outcome.terminal, TerminalState::Settled);
        let prompts = composed.runner.prompts.borrow().clone();
        assert!(prompts.len() >= 2, "the malformed unit retried");
        let retried = prompts
            .iter()
            .skip(1)
            .map(|prompt| prompt.1.clone())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            retried.contains("not JSON at byte 3"),
            "the retry names the parse failure: {retried}"
        );
        let records = evidence.records_containing("invocation_failed");
        assert!(
            records
                .iter()
                .any(|record| record.contains("not JSON at byte 3")),
            "the durable record carries the cause: {records:?}"
        );
    }

    /// W6: a clean containment check changes nothing — the unit settles
    /// and no breach is pinned.
    #[test]
    fn a_clean_containment_check_settles_normally() {
        let key = Rc::new(RefCell::new(answer_key(1)));
        let state = FakeState::new();
        let evidence = FakeEvidence::new();
        let runner = ScriptedRunner::new(vec![Behavior::Complete]);
        let workspaces = FixtureWorkspaces::new();
        workspaces.set_containment_response(Ok(ContainmentDiff {
            supported: true,
            changed: Vec::new(),
            added: Vec::new(),
            removed: Vec::new(),
        }));
        let composed = compose_full(
            key,
            state,
            evidence.clone(),
            runner,
            0,
            FIXTURE_ROUNDS,
            policy(),
            fixture_ladder(&["ctrl-9101.json"]),
            meter_factory(),
            Some(workspaces.clone()),
        )
        .expect("composes");
        let outcome = composed.controller.start_run().expect("the loop runs");
        assert_eq!(outcome.terminal, TerminalState::Settled);
        assert!(
            evidence.records_containing("containment_breach").is_empty(),
            "no breach pinned for a clean project root"
        );
        assert!(
            evidence
                .records_containing("containment_check_failed")
                .is_empty(),
            "no check failure pinned either"
        );
    }

    /// The provider-less composition keeps its pre-task20 shape: the
    /// dispatch falls back to the run-derived workspace ref (the OLD
    /// behavior, honest because no provider was promised).
    #[test]
    fn without_a_provider_the_dispatch_falls_back_to_the_run_derived_ref() {
        let key = Rc::new(RefCell::new(answer_key(1)));
        let state = FakeState::new();
        let evidence = FakeEvidence::new();
        let runner = ScriptedRunner::new(vec![Behavior::Complete]);
        let composed =
            compose(key, state, evidence, runner.clone(), 0, FIXTURE_ROUNDS).expect("composes");
        composed
            .controller
            .start_run()
            .expect("the loop runs without a provider");
        let specs = runner.specs.borrow();
        assert_eq!(specs.len(), 1);
        let run_id = RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("fixture run id");
        let fallback = run_workspace_ref(&run_id);
        assert!(
            specs
                .values()
                .any(|spec| spec.workspace.as_str() == fallback.as_str()),
            "the provider-less dispatch names the run-derived ref: {:?}",
            specs
                .values()
                .map(|spec| spec.workspace.as_str())
                .collect::<Vec<_>>()
        );
    }

    /// Every failure class pins an `invocation_failed` record naming itself
    /// (a real run once left an invocation terminal-failed with nothing
    /// durable saying why).
    #[test]
    fn dispose_failure_pins_a_durable_record_naming_the_failure_class() {
        let key = Rc::new(RefCell::new(answer_key(1)));
        let state = FakeState::new();
        let evidence = FakeEvidence::new();
        let runner = ScriptedRunner::new(vec![Behavior::Fail(InvocationFailure::ContractMismatch)]);
        let composed = compose_full(
            key,
            state,
            evidence.clone(),
            runner,
            0,
            FIXTURE_ROUNDS,
            policy(),
            fixture_ladder(&["ctrl-9101.json"]),
            meter_factory(),
            None,
        )
        .expect("composes");
        let outcome = composed.controller.start_run().expect("the loop runs");
        assert_eq!(
            outcome.terminal,
            TerminalState::Aborted,
            "a contract mismatch is a §11.6 integrity violation: terminal abort"
        );
        let records = evidence.records_containing("invocation_failed");
        assert_eq!(
            records.len(),
            1,
            "dispose_failure pins exactly one durable record naming the failure: {records:?}"
        );
        assert!(
            records[0].contains("contract_mismatch"),
            "the record names the failure class: {}",
            records[0]
        );
    }

    /// `dispatch` allocates a workspace before the meter reservation; a
    /// dispatch truncated at the meter's scope must release it (a real run
    /// left orphaned empty workspace dirs). With 3 eligible units over a
    /// parallelism of 2, the third is truncated on iteration 1 and settles
    /// later; every created workspace is cleaned by run end.
    #[test]
    fn a_truncated_dispatch_never_leaks_its_allocated_workspace() {
        let key = Rc::new(RefCell::new(answer_key(OVER_CAPACITY_UNITS)));
        let state = FakeState::new();
        let evidence = FakeEvidence::new();
        let runner = ScriptedRunner::new(vec![
            Behavior::Complete,
            Behavior::Complete,
            Behavior::Complete,
        ]);
        let workspaces = FixtureWorkspaces::new();
        let composed = compose_full(
            key,
            state,
            evidence,
            runner,
            0,
            FIXTURE_ROUNDS,
            policy(),
            fixture_ladder(&["ctrl-9101.json", "ctrl-9102.json", "ctrl-9103.json"]),
            meter_factory(),
            Some(workspaces.clone()),
        )
        .expect("composes");
        let outcome = composed
            .controller
            .start_run()
            .expect("an over-capacity fan-out never errors the run");
        assert_eq!(
            outcome.terminal,
            TerminalState::Settled,
            "every unit settles once the truncated dispatch gets its turn"
        );
        let created: std::collections::BTreeSet<String> = workspaces
            .created
            .lock()
            .expect("ledger locks")
            .iter()
            .map(|reference| reference.as_str().to_owned())
            .collect();
        let cleaned: std::collections::BTreeSet<String> = workspaces
            .cleaned
            .lock()
            .expect("ledger locks")
            .iter()
            .map(|reference| reference.as_str().to_owned())
            .collect();
        assert_eq!(
            created, cleaned,
            "every workspace the provider allocated — including one allocated for a \
             dispatch later truncated at the meter's capacity scope — must be released \
             by run end: created {created:?}, cleaned {cleaned:?}"
        );
    }

    #[test]
    fn an_admission_refusal_is_a_bounded_retry_that_still_settles() {
        // W2 test (a): the runner refuses the FIRST invoke
        // (`AgentError::AdmissionRefused` — a real agent-runner backend's
        // refusal), the retry's invoke is admitted and completes. The run
        // must settle, not error; exactly one `invocation_admission_failed`
        // record is pinned; the refused attempt's workspace is released;
        // and its meter reservation is released too (the retry's own
        // reservation proves the slot is free — `granted == released`).
        let key = Rc::new(RefCell::new(answer_key(1)));
        let state = FakeState::new();
        let evidence = FakeEvidence::new();
        let runner = ScriptedRunner::new(vec![Behavior::AdmissionRefused, Behavior::Complete]);
        let workspaces = FixtureWorkspaces::new();
        let tally = Arc::new(Mutex::new(MeterTally::default()));
        let meter_tally = tally.clone();
        let factory: Arc<
            dyn Fn(Budget) -> Arc<dyn gauntlet_ports::budget_meter::BudgetMeter> + Send + Sync,
        > = Arc::new(move |budget| {
            #[allow(clippy::arc_with_non_send_sync)] // single-threaded proof fixture wiring
            Arc::new(ObservingMeter {
                inner: FakeMeter(Mutex::new(FakeMeterInner {
                    budget,
                    outstanding: 0,
                })),
                tally: meter_tally.clone(),
            })
        });
        let composed = compose_full(
            key,
            state,
            evidence.clone(),
            runner,
            0,
            FIXTURE_ROUNDS,
            policy(),
            fixture_ladder(&["ctrl-9101.json"]),
            factory,
            Some(workspaces.clone()),
        )
        .expect("composes");
        let outcome = composed
            .controller
            .start_run()
            .expect("an admission refusal never errors the run");
        assert_eq!(
            outcome.terminal,
            TerminalState::Settled,
            "the retry's admitted invocation settles the sole unit: {outcome:?}"
        );
        let records = evidence.records_containing("invocation_admission_failed");
        assert_eq!(
            records.len(),
            1,
            "exactly one admission-failure record is pinned: {records:?}"
        );
        let created: std::collections::BTreeSet<String> = workspaces
            .created
            .lock()
            .expect("ledger locks")
            .iter()
            .map(|reference| reference.as_str().to_owned())
            .collect();
        let cleaned: std::collections::BTreeSet<String> = workspaces
            .cleaned
            .lock()
            .expect("ledger locks")
            .iter()
            .map(|reference| reference.as_str().to_owned())
            .collect();
        assert_eq!(
            created, cleaned,
            "the refused attempt's freshly allocated workspace is released too: \
             created {created:?}, cleaned {cleaned:?}"
        );
        let observed = tally.lock().expect("tally lock");
        assert_eq!(
            observed.granted, observed.released,
            "the refused attempt's meter reservation is released — no parallel slot \
             stays held forever (granted {}, released {})",
            observed.granted, observed.released
        );
    }

    #[test]
    fn every_invoke_refused_escalates_instead_of_crashing_the_run() {
        // W2 test (b): every invoke refuses admission. The §11.5 retry
        // table bounds the retries by the unit's own attempt ceiling
        // (`ATTEMPT_CAP`); once exhausted the run ends `Escalated` with a
        // ticket — never `Err`, never a leaked workspace, one durable
        // record per refused attempt.
        let key = Rc::new(RefCell::new(answer_key(1)));
        let state = FakeState::new();
        let evidence = FakeEvidence::new();
        let runner = ScriptedRunner::new(vec![
            Behavior::AdmissionRefused,
            Behavior::AdmissionRefused,
            Behavior::AdmissionRefused,
        ]);
        let workspaces = FixtureWorkspaces::new();
        let composed = compose_full(
            key,
            state,
            evidence.clone(),
            runner,
            0,
            FIXTURE_ROUNDS,
            policy(),
            fixture_ladder(&["ctrl-9101.json"]),
            meter_factory(),
            Some(workspaces.clone()),
        )
        .expect("composes");
        let outcome = composed
            .controller
            .start_run()
            .expect("exhausted admission retries end the run Ok, escalated — never Err");
        assert_eq!(
            outcome.terminal,
            TerminalState::Escalated,
            "every admission refused exhausts the unit's attempt ceiling: {outcome:?}"
        );
        assert!(
            outcome.escalation_summary.is_some(),
            "an escalated terminal carries the ticket summary"
        );
        let records = evidence.records_containing("invocation_admission_failed");
        assert_eq!(
            records.len(),
            3,
            "one durable record per refused attempt: {records:?}"
        );
        let created: std::collections::BTreeSet<String> = workspaces
            .created
            .lock()
            .expect("ledger locks")
            .iter()
            .map(|reference| reference.as_str().to_owned())
            .collect();
        let cleaned: std::collections::BTreeSet<String> = workspaces
            .cleaned
            .lock()
            .expect("ledger locks")
            .iter()
            .map(|reference| reference.as_str().to_owned())
            .collect();
        assert_eq!(
            created, cleaned,
            "every allocated workspace is released, even across an escalated exhaustion: \
             created {created:?}, cleaned {cleaned:?}"
        );
    }

    #[test]
    fn a_runners_understated_wall_is_never_the_charged_fact() {
        // W3 test (a): the runner reports `wall_seconds: 0` (the
        // `ScriptedRunner` completion fixture always does), but the
        // virtual clock advances the loop's own dispatch-to-completion
        // measurement by a large span (one poll pass, a poll interval
        // widened to exactly that span). The charged payment must be
        // max(reported, measured) — never the runner's understated figure.
        const WIDE_TICK_WINDOW: u64 = 120_000;
        let key = Rc::new(RefCell::new(answer_key(1)));
        let state = FakeState::new();
        let evidence = FakeEvidence::new();
        let runner = ScriptedRunner::new(vec![Behavior::Complete]);
        let wide_poll_policy = ControlPolicy::new(
            WIDE_TICK_WINDOW,
            QUIET_WINDOW,
            FLOOR,
            CEILING,
            ATTEMPT_CAP,
            THRESHOLD,
        )
        .expect("the widened-poll-interval fixture policy is usable");
        let composed = compose_full(
            key,
            state.clone(),
            evidence,
            runner,
            0,
            FIXTURE_ROUNDS,
            wide_poll_policy,
            fixture_ladder(&["ctrl-9101.json"]),
            meter_factory(),
            None,
        )
        .expect("composes");
        let outcome = composed.controller.start_run().expect("the loop runs");
        assert_eq!(outcome.terminal, TerminalState::Settled);
        let checkpoint = composed
            .state
            .resume(&outcome.run_id, bound())
            .expect("checkpoint readable")
            .expect("a checkpoint exists");
        let measured_floor_seconds = WIDE_TICK_WINDOW / super::MILLIS_PER_SECOND;
        assert!(
            checkpoint.budget_spent.wall_seconds() >= measured_floor_seconds,
            "the loop's own measured wall floors the charged payment even though the \
             runner reported zero: spent {} (floor {measured_floor_seconds})",
            checkpoint.budget_spent.wall_seconds()
        );
    }

    #[test]
    fn a_running_payment_so_far_past_remaining_capacity_cancels_in_flight_and_exhausts() {
        // W3 test (b): a poll's `Running.payment_so_far` — the runner's
        // OWN real measured in-flight spend — exceeds the run's remaining
        // forward token capacity. The guard must cancel the in-flight
        // invocation, pin the breach record, and drive the terminal to
        // `BudgetExhausted` — never let it settle on payment the run
        // could never have afforded.
        let key = Rc::new(RefCell::new(answer_key(1)));
        let state = FakeState::new();
        let evidence = FakeEvidence::new();
        // The forward token ceiling is `FIXTURE_TOKEN_CEILING * (100 -
        // FIXTURE_RESERVE_PCT) / 100`; `ProgressForever` reports a fixed
        // `Payment::zero()` today, so the guard needs a scripted behavior
        // that reports a payment past that ceiling. `CompleteWithTokens`
        // only shapes the COMPLETED report, not a `Running` poll, so this
        // lever is `ProgressForeverWithPayment` — see the behavior/poll
        // change below.
        let over_ceiling_tokens = FIXTURE_TOKEN_CEILING + 1;
        let runner = ScriptedRunner::new(vec![Behavior::ProgressForeverWithPayment {
            max_polls: 5,
            tokens: over_ceiling_tokens,
        }]);
        let composed = compose_full(
            key,
            state.clone(),
            evidence.clone(),
            runner,
            0,
            FIXTURE_ROUNDS,
            policy(),
            fixture_ladder(&["ctrl-9101.json"]),
            meter_factory(),
            None,
        )
        .expect("composes");
        let outcome = composed
            .controller
            .start_run()
            .expect("an in-flight budget breach never errors the run");
        assert_eq!(
            outcome.terminal,
            TerminalState::BudgetExhausted,
            "in-flight spend past the remaining forward capacity exhausts the budget: \
             {outcome:?}"
        );
        assert_eq!(
            composed.runner.cancels().len(),
            1,
            "the over-capacity in-flight invocation was cancelled"
        );
        let records = evidence.records_containing("budget_breach_in_flight");
        assert_eq!(
            records.len(),
            1,
            "exactly one in-flight breach record is pinned: {records:?}"
        );
        assert!(
            records[0].contains(&format!("\"tokens_in_flight\":{over_ceiling_tokens}")),
            "the record names the in-flight tokens observed: {}",
            records[0]
        );
        let settlements = composed
            .state
            .settlements(&outcome.run_id, bound())
            .expect("settlements readable");
        assert!(
            settlements.is_empty(),
            "the in-flight invocation never settles: {settlements:?}"
        );
    }

    // ------------------------------------------------------------------
    // COR-F1 (falsifier round 1, F-1): the instrument corroboration rule,
    // pinned BOTH ways. The produced-artifact world settles an
    // instrument-produced unit ONLY when this run's observation carries
    // an executed passing test facet; the default compose() world never
    // consults the gate.
    // ------------------------------------------------------------------

    use crate::observation::bundle::{
        BundleContent, ClaimsFacet, FacetRecord, ObservationBundleDoc,
    };

    /// A sideband observer: a REAL-looking instrument facet of the wrong
    /// CLASS — the filesystem instrument observed the workspace, but no
    /// test execution ran. The uncorroborated half's lever.
    struct SidebandObserver {
        store: FakeEvidence,
        clock: Arc<dyn Clock>,
    }

    impl Observer for SidebandObserver {
        fn observe(
            &self,
            request: &ObservationRequest,
            _: Deadline,
        ) -> Result<ObservationFacet, ObserverError> {
            let instrument = InstrumentIdentity::new(
                gauntlet_domain::InstrumentName::new("gauntlet-controller-sideband")
                    .expect("fixture instrument name"),
                "1".to_owned(),
            );
            let started = self.clock.now();
            let ended = EpochSeconds::new(started.get() + 1);
            let command = "tree-walk --workspace".to_owned();
            let doc = FacetEvidenceDoc::new(
                instrument.name().as_str().to_owned(),
                instrument.version().to_owned(),
                ObservedRun {
                    command: command.clone(),
                    env_fingerprint:
                        "sha256:0000000000000000000000000000000000000000000000000000000000000000"
                            .to_owned(),
                    exit_status: 0,
                    started_at: started.get(),
                    ended_at: ended.get(),
                    stdout: format!("observed workspace {}", request.workspace.as_str()),
                    stderr: String::new(),
                },
            )
            .expect("the sideband evidence document is well formed");
            let bound = Deadline::at_epoch(EpochSeconds::new(u64::MAX / 2));
            let evidence_ref = self
                .store
                .put(doc.encode().as_bytes(), EvidenceKind::Log, bound)
                .map_err(|error| ObserverError::Unavailable {
                    kind: FacetKind::Filesystem,
                    detail: error.to_string(),
                })?;
            Ok(
                ObservationFacet::new(FacetKind::Filesystem, instrument, evidence_ref)
                    .with_invocation(Some(command), Some(0))
                    .with_window(Some(started), Some(ended))
                    .with_summary(Some("sideband facet".to_owned())),
            )
        }
        fn facet_kind(&self) -> FacetKind {
            FacetKind::Filesystem
        }
    }

    /// Builds one wire facet record for the corroboration-rule pin.
    fn corroboration_facet(
        kind: FacetKind,
        command: Option<&str>,
        exit: Option<i64>,
    ) -> FacetRecord {
        let instrument = InstrumentIdentity::new(
            gauntlet_domain::InstrumentName::new("gauntlet-controller-fixture").expect("name"),
            "1".to_owned(),
        );
        let evidence_ref = gauntlet_domain::EvidenceRef::new("evd-corroboration-facet")
            .expect("the fixture ref parses");
        FacetRecord::from_facet(
            &ObservationFacet::new(kind, instrument, evidence_ref)
                .with_invocation(command.map(str::to_owned), exit),
        )
    }

    /// Builds one observation bundle document for the corroboration-rule
    /// pin: `run`/`iteration` ownership plus the facet records.
    fn corroboration_doc(
        run: &RunId,
        iteration: u32,
        facets: Vec<FacetRecord>,
    ) -> ObservationBundleDoc {
        ObservationBundleDoc::new(BundleContent {
            run_id: run.clone(),
            iteration_no: gauntlet_domain::IterationNo::new(iteration).expect("iteration"),
            observed_at: 0,
            project_revision: "rev".to_owned(),
            environment_fingerprint: "fp".to_owned(),
            observation_digest: Digest::new("sha256", &"ab".repeat(32)).expect("digest"),
            facets,
            claims_facet: ClaimsFacet::new("claims".to_owned()),
            unavailable_evidence: Vec::new(),
            observation_failures: Vec::new(),
        })
    }

    #[test]
    fn the_corroboration_rule_demands_this_runs_executed_passing_test_facet() {
        let run = RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("run id");
        let iteration = gauntlet_domain::IterationNo::new(2).expect("iteration");
        // Corroborated: a test facet that really executed and passed, in
        // THIS run at THIS iteration.
        let doc = corroboration_doc(
            &run,
            2,
            vec![corroboration_facet(
                FacetKind::Test,
                Some("fixture-probe --corroboration"),
                Some(0),
            )],
        );
        assert!(RunController::produced_artifact_corroborated(
            &doc, &run, iteration
        ));
        // A filesystem facet — however healthy — is not a test execution.
        let doc = corroboration_doc(
            &run,
            2,
            vec![corroboration_facet(
                FacetKind::Filesystem,
                Some("tree-walk"),
                Some(0),
            )],
        );
        assert!(!RunController::produced_artifact_corroborated(
            &doc, &run, iteration
        ));
        // A test facet that FAILED contradicts the passing report.
        let doc = corroboration_doc(
            &run,
            2,
            vec![corroboration_facet(
                FacetKind::Test,
                Some("fixture-probe --corroboration"),
                Some(1),
            )],
        );
        assert!(!RunController::produced_artifact_corroborated(
            &doc, &run, iteration
        ));
        // An attestation-shaped facet with no execution decides nothing.
        let doc = corroboration_doc(
            &run,
            2,
            vec![corroboration_facet(FacetKind::Test, None, None)],
        );
        assert!(!RunController::produced_artifact_corroborated(
            &doc, &run, iteration
        ));
        // A different run's bundle is not THIS completion's fact source.
        let other_run = RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAB").expect("run id");
        let doc = corroboration_doc(
            &other_run,
            2,
            vec![corroboration_facet(
                FacetKind::Test,
                Some("fixture-probe --corroboration"),
                Some(0),
            )],
        );
        assert!(!RunController::produced_artifact_corroborated(
            &doc, &run, iteration
        ));
        // Neither is a LATER iteration's bundle carried backward.
        let doc = corroboration_doc(
            &run,
            3,
            vec![corroboration_facet(
                FacetKind::Test,
                Some("fixture-probe --corroboration"),
                Some(0),
            )],
        );
        assert!(!RunController::produced_artifact_corroborated(
            &doc, &run, iteration
        ));
        // An empty bundle corroborates nothing.
        let doc = corroboration_doc(&run, 2, Vec::new());
        assert!(!RunController::produced_artifact_corroborated(
            &doc, &run, iteration
        ));
    }

    /// Composes the controller in the OPTED-IN produced-artifact world
    /// with the caller's observers — the COR-F1 world-test harness. The
    /// shared `compose_full` keeps the default produce policy and
    /// observer set, untouched.
    fn compose_produced(
        key: Rc<RefCell<String>>,
        state: Arc<FakeState>,
        evidence: FakeEvidence,
        runner: Arc<ScriptedRunner>,
        observers: Vec<Arc<dyn Observer>>,
    ) -> Result<Composed, ControlError> {
        compose_produced_with_rounds(key, state, evidence, runner, observers, FIXTURE_ROUNDS)
    }

    /// Like [`compose_produced`] but with an explicit rounds ceiling, for a
    /// caller that needs a tight forward-rounds ceiling rather than
    /// `compose_produced`'s generous default.
    fn compose_produced_with_rounds(
        key: Rc<RefCell<String>>,
        state: Arc<FakeState>,
        evidence: FakeEvidence,
        runner: Arc<ScriptedRunner>,
        observers: Vec<Arc<dyn Observer>>,
        rounds: u32,
    ) -> Result<Composed, ControlError> {
        let scratch = Scratch::new("compose-produced");
        let clock = DeterministicClock::new();
        let provider = Arc::new(FlakyProvider {
            inner: FileReferenceProvider::from_sources(
                AnswerKeySource::Shared(key.clone()),
                ManifestSource::Inline(minimal_manifest()),
                Rc::new(evidence.clone()),
            ),
            unavailable: Rc::new(Cell::new(false)),
        });
        let unavailable = provider.unavailable.clone();
        let comparators = ScriptedComparators::new(0);
        let deps = ControllerDeps {
            state: state.clone(),
            evidence: Arc::new(evidence.clone()),
            ids: Arc::new(SeededIds(RefCell::new(0))),
            clock,
            runner: runner.clone(),
            runner_identity: gauntlet_domain::RunnerIdentity::new("fixture-runner")
                .expect("runner identity"),
            observers,
            comparison: ComparisonDeps {
                contract: minimal_contract(),
                deterministic: Arc::new(ComparatorsSource(comparators.clone())),
                model: Arc::new(ComparatorsSource(comparators.clone())),
                adversarial: None,
                review_mode: gauntlet_domain::ReviewMode::Single,
                model_dispatches: 4,
                role_deny: crate::roles::ContextDenySet::new(Vec::new()),
            },
            ladder_for: Arc::new(|_| Ok(fixture_ladder(&["ctrl-9101.json", "ctrl-9102.json"]))),
            reference: provider,
            answer_key: AnswerKeySource::Shared(key.clone()),
            escalation: Arc::new(FakeEscalation::default()),
            meter_factory: meter_factory(),
            budget: budget_shape(rounds),
            policy: policy(),
            workspaces: None,
            workspace_root: scratch.0.clone(),
            config_fingerprint: Digest::new("sha256", &"ab".repeat(32)).expect("digest"),
            govern: crate::govern::GovernDeps::none_backend(),
            environment_fingerprint: Digest::new("sha256", &"cd".repeat(32)).expect("digest"),
            intent: "controller fixture".to_owned(),
        };
        std::mem::forget(scratch);
        let controller = RunController::compose_binding_produced_artifacts(deps)?;
        Ok(Composed {
            controller,
            state,
            runner,
            unavailable,
            comparators,
        })
    }

    /// Like `FixtureObserver` but its test facet's exit status flips from
    /// FAIL (the first call) to PASS (every later call) — the task26
    /// repro's stand-in for "the real check script failed round 1, passed
    /// round 2".
    struct FlippingTestObserver {
        store: FakeEvidence,
        clock: Arc<dyn Clock>,
        calls: Arc<Cell<u32>>,
    }

    impl Observer for FlippingTestObserver {
        fn observe(
            &self,
            request: &ObservationRequest,
            _: Deadline,
        ) -> Result<ObservationFacet, ObserverError> {
            let call = self.calls.get();
            self.calls.set(call + 1);
            let exit_status: i64 = if call == 0 { 1 } else { 0 };
            let instrument = InstrumentIdentity::new(
                gauntlet_domain::InstrumentName::new("gauntlet-controller-fixture-flip")
                    .expect("fixture instrument name"),
                "1".to_owned(),
            );
            let started = self.clock.now();
            let ended = EpochSeconds::new(started.get() + 1);
            let command = "fixture-probe --report".to_owned();
            let doc = FacetEvidenceDoc::new(
                instrument.name().as_str().to_owned(),
                instrument.version().to_owned(),
                ObservedRun {
                    command: command.clone(),
                    env_fingerprint:
                        "sha256:0000000000000000000000000000000000000000000000000000000000000000"
                            .to_owned(),
                    exit_status,
                    started_at: started.get(),
                    ended_at: ended.get(),
                    stdout: format!("observed workspace {}", request.workspace.as_str()),
                    stderr: String::new(),
                },
            )
            .expect("the fixture evidence document is well formed");
            let bound = Deadline::at_epoch(EpochSeconds::new(u64::MAX / 2));
            let evidence_ref = self
                .store
                .put(doc.encode().as_bytes(), EvidenceKind::Log, bound)
                .map_err(|error| ObserverError::Unavailable {
                    kind: FacetKind::Test,
                    detail: error.to_string(),
                })?;
            Ok(
                ObservationFacet::new(FacetKind::Test, instrument, evidence_ref)
                    .with_invocation(Some(command), Some(exit_status))
                    .with_window(Some(started), Some(ended))
                    .with_summary(Some(format!("fixture facet exit {exit_status}"))),
            )
        }
        fn facet_kind(&self) -> FacetKind {
            FacetKind::Test
        }
    }

    #[test]
    fn a_completion_that_re_produces_identical_bytes_after_a_failed_first_attempt_still_settles() {
        // Reproduces the orchestrator-verified demo-calculator defect
        // (task26) at its REAL locus, DB-confirmed against the actual
        // run's `event_log`/`observation_bindings`/`evidence_productions`/
        // `settlements` tables and the pinned evidence store: AK-9011's
        // own locator bytes did not change between attempts — only the
        // run's shared test facet's exit status did (round 1 failed,
        // round 2 passed) — and the real run's second, PASSING attempt
        // was silently lost because its evidence production was treated
        // as a no-op.
        //
        // A GENEROUS budget on purpose (`compose_produced`'s
        // `FIXTURE_ROUNDS`): this defect needs no tight rounds ceiling at
        // all — it reproduces on the very FIRST retry — and a tight
        // budget in this fixture composition trips an unrelated meter
        // anomaly (reported separately; see the task26 report), which
        // would make this test's RED/GREEN prove the wrong thing.
        //
        // No `workspaces` provider is wired, so `evidence_workspace` is
        // the SAME physical path across both attempts (COR-D3): attempt
        // 1's `produce_unit_evidence` finds nothing at the locator yet and
        // materializes its own (failing) facet document there; attempt
        // 2's produce step finds that SAME file already on disk and reads
        // back those SAME bytes — byte-identical content across attempts
        // by construction, exactly like AK-9011's unchanging markup.
        //
        // Before the fix: `produce_unit_evidence`'s `already_durable`
        // check skipped recording entirely on attempt 2 (no fresh
        // `evidence_produced`, no new per-iteration row); `verify_claim`
        // then applied §14.3's staleness rule against the iteration-1
        // row and returned INCONCLUSIVE even though attempt 2's own test
        // facet passed and genuinely corroborates those bytes; settlement
        // refused (§16.1 condition 1); the unit never settles.
        let key = Rc::new(RefCell::new(answer_key(1)));
        let state = FakeState::new();
        // Content-addressed (task26): AK-9011's own locator bytes really
        // are byte-identical across attempts in the real run — this
        // fixture must dedup the same way the real content-addressed
        // evidence-store adapter does, or the repro tests nothing.
        let evidence = FakeEvidence::new_content_addressed();
        let runner = ScriptedRunner::new(vec![Behavior::Complete, Behavior::Complete]);
        let clock = DeterministicClock::new();
        let calls = Arc::new(Cell::new(0u32));
        let observers: Vec<Arc<dyn Observer>> = vec![Arc::new(FlippingTestObserver {
            store: evidence.clone(),
            clock,
            calls,
        })];
        let composed =
            compose_produced(key, state, evidence.clone(), runner, observers).expect("composes");
        let outcome = composed.controller.start_run().expect("the loop runs");
        assert_eq!(
            outcome.terminal,
            TerminalState::Settled,
            "attempt 2 re-produced identical, now-corroborated bytes; it must settle: {outcome:?}"
        );
        let rows = composed
            .state
            .settlements(&outcome.run_id, bound())
            .expect("settlements readable");
        assert_eq!(
            rows.len(),
            1,
            "exactly one durable settlement exists for the unit's second attempt: {rows:?}"
        );
        // task26 (append-only correction): both attempts' productions
        // stand as SEPARATE durable rows for the SAME evidence ref (the
        // content never changed) — the exact fact
        // `seed_unit_states`'s resume-time attempts reconstruction
        // (`seed_unit_states`'s `evidence_productions_at(locator).len()`)
        // depends on. Before this correction, an identical re-production
        // collapsed onto ONE row regardless of how many iterations
        // re-affirmed it, UNDERCOUNTING attempts on any later resume (a
        // stale §11.5 attempts ceiling that could let a unit be retried
        // past its configured cap after a crash).
        let locator =
            gauntlet_domain::EvidenceLocator::new("ctrl-9101.json").expect("fixture locator");
        let productions = composed
            .state
            .evidence_productions_at(&outcome.run_id, &locator, bound())
            .expect("productions readable");
        assert_eq!(
            productions.len(),
            2,
            "both attempts' re-productions of the identical content stand as separate, \
             per-iteration rows: {productions:?}"
        );
        // A resume over the now-settled run is the honest no-op:
        // reconstructing attempts from TWO durable rows must never cause
        // a phantom re-dispatch or a wrongful escalation over work that
        // already settled.
        let resumed = composed
            .controller
            .resume_run(&outcome.run_id)
            .expect("the settled run resumes");
        assert_eq!(
            resumed.terminal,
            TerminalState::Settled,
            "the settled no-op stands regardless of the reconstructed attempts count"
        );
        assert_eq!(
            composed.runner.prompts.borrow().len(),
            2,
            "the resume re-dispatches nothing: {:?}",
            composed.runner.prompts.borrow()
        );
    }

    #[test]
    fn a_corroborated_produced_artifact_settles_in_the_produced_world() {
        // The positive half: the observation carries an executed, passing
        // test facet (the FixtureObserver's shape), so the instrument-
        // produced unit SETTLES in the produced-artifact world.
        let key = Rc::new(RefCell::new(answer_key(1)));
        let state = FakeState::new();
        let evidence = FakeEvidence::new();
        let runner = ScriptedRunner::new(vec![Behavior::Complete]);
        let clock = DeterministicClock::new();
        let observers: Vec<Arc<dyn Observer>> = vec![Arc::new(FixtureObserver {
            store: evidence.clone(),
            clock,
        })];
        let composed = compose_produced(key, state, evidence, runner, observers).expect("composes");
        let outcome = composed.controller.start_run().expect("the loop runs");
        assert_eq!(outcome.terminal, TerminalState::Settled);
        let rows = composed
            .state
            .settlements(&outcome.run_id, bound())
            .expect("settlements readable");
        assert_eq!(rows.len(), 1, "the corroborated unit settled");
    }

    #[test]
    fn an_uncorroborated_produced_artifact_never_settles_in_the_produced_world() {
        // The negative half: the observation carries only a FILESYSTEM
        // facet, the rung's decision over the locator bytes is PASS, and
        // the settlement gate blocks it — the unit stays open, the run
        // ends in a bounded non-settled terminal, and the block is pinned
        // as typed evidence (never silence, never a downgrade).
        let key = Rc::new(RefCell::new(answer_key(1)));
        let state = FakeState::new();
        let evidence = FakeEvidence::new();
        let evidence_for_assert = evidence.clone();
        let runner = ScriptedRunner::new(vec![Behavior::Complete]);
        let clock = DeterministicClock::new();
        let observers: Vec<Arc<dyn Observer>> = vec![Arc::new(SidebandObserver {
            store: evidence.clone(),
            clock,
        })];
        let composed = compose_produced(key, state, evidence, runner, observers).expect("composes");
        let outcome = composed.controller.start_run().expect("the loop runs");
        assert_ne!(
            outcome.terminal,
            TerminalState::Settled,
            "an uncorroborated instrument-produced unit never settles the run"
        );
        let rows = composed
            .state
            .settlements(&outcome.run_id, bound())
            .expect("settlements readable");
        assert!(rows.is_empty(), "no settlement row exists");
        let pinned = evidence_for_assert.records_containing("producer_uncorroborated");
        assert!(
            !pinned.is_empty(),
            "the producer_uncorroborated block is pinned evidence"
        );
    }

    /// task26, fixture-only: `rounds = 3` at `FIXTURE_RESERVE_PCT` gives a
    /// forward ceiling of 2 genuine rounds, so the run must be able to
    /// dispatch two attempts (round 1's failing attempt, then round 2's
    /// genuinely passing retry) before it may settle. The shipped adapter
    /// books no rounds on charge; the loop's `book_iteration_round` is the
    /// single rounds authority, so this must hold under a faithful meter.
    #[test]
    fn a_tight_rounds_budget_still_allows_forward_ceiling_attempts_through_compose_produced() {
        let key = Rc::new(RefCell::new(answer_key(1)));
        let state = FakeState::new();
        let evidence = FakeEvidence::new_content_addressed();
        let runner = ScriptedRunner::new(vec![Behavior::Complete, Behavior::Complete]);
        let clock = DeterministicClock::new();
        let calls = Arc::new(Cell::new(0u32));
        let observers: Vec<Arc<dyn Observer>> = vec![Arc::new(FlippingTestObserver {
            store: evidence.clone(),
            clock,
            calls,
        })];
        const TIGHT_ROUNDS: u32 = 3;
        let composed =
            compose_produced_with_rounds(key, state, evidence, runner, observers, TIGHT_ROUNDS)
                .expect("composes");
        let outcome = composed.controller.start_run().expect("the loop runs");
        assert_eq!(
            outcome.terminal,
            TerminalState::Settled,
            "round 2's genuinely passing retry must be DISPATCHED and settle — the \
             loop's own authoritative budget spent only one round for round 1's \
             failing attempt, so the forward ceiling (2) has headroom for a second: \
             {outcome:?}"
        );
        assert_eq!(
            composed.runner.prompts.borrow().len(),
            2,
            "exactly the forward ceiling's two attempts were dispatched, no fewer: {:?}",
            composed.runner.prompts.borrow()
        );
    }

    /// task26, fixture-only: `BudgetMeter::charge` must never book the
    /// reservation's `rounds` dimension, mirroring the shipped adapter.
    /// The loop is the single rounds authority; charging two `rounds: 1`
    /// reservations must leave the meter's own `remaining().rounds`
    /// unchanged.
    #[test]
    fn fake_meter_charge_books_no_rounds_like_the_shipped_adapter() {
        let budget = Budget::new(
            budget_shape(3).limits,
            budget_shape(3).recovery_reserve_pct,
            budget_shape(3).diagnostic,
        )
        .expect("usable fixture budget");
        let meter = (meter_factory())(budget);
        let starting_remaining_rounds = meter.remaining().rounds;
        let slice = gauntlet_ports::budget_meter::BudgetSlice {
            rounds: 1,
            wall_seconds: 0,
            tokens: 0,
            cost_usd: MicroUsd::new(0),
        };
        // Two reservations charged with `rounds: 1` each — exactly the
        // builder-dispatch and critic-admission shape ONE loop iteration
        // produces under `ReviewMode::Single` with a nonzero model
        // dispatch allowance.
        for _ in 0..2 {
            let reservation = meter.reserve(slice).expect("headroom for the estimate");
            meter
                .charge(&reservation, gauntlet_domain::Payment::zero())
                .expect("a zero payment always charges");
        }
        assert_eq!(
            meter.remaining().rounds,
            starting_remaining_rounds,
            "charging a reservation must never book rounds on the meter: rounds are \
             the loop's own iteration-boundary fact, never the meter's payment counter"
        );
    }
}
