//! P11 proof (plan Task 13 B4; spec §23.1 P11; I3/AK-005/AK-031 ride
//! along): a comparator returning `pass` — a CLEAN comparison admitted
//! through the real comparison chain — while NO verification record
//! exists does NOT settle a unit. Through the REAL composed controller
//! and REAL durable stores, for every honest no-record shape:
//!
//! 1. the verifier REFUSES (a supported rung breaks mechanically — the
//!    §18.4 instrument-failure class): the run errors out of the drain
//!    with the typed ladder refusal, and durably NOTHING settled;
//! 2. NO RUNG CAN DECIDE the requirement (the `ladder absent` shape —
//!    an empty ladder is refused at composition, so the honest spelling
//!    is a rung that declines the requirement): the same typed refusal,
//!    nothing settled;
//! 3. verification returns INCONCLUSIVE (AK-005's three-state rule at
//!    this seam): the claim reaches the settlement service and is
//!    REFUSED at the entry gate — no `SettlementCommitted` event, no
//!    settlements row, the projection stays open, and the pinned
//!    forensic evidence names INCONCLUSIVE distinctly;
//! 4. verification returns FAIL over the same clean comparison: refused
//!    the same way, the pinned forensic evidence names FAIL — the
//!    refusal names the real cause, and the comparator's pass never
//!    closes the unit (§13.1: the comparator "never closes a Unit, and
//!    never issues a settlement").
//!
//! A PASS control leg (the same composition, the rung deciding PASS)
//! settles the unit through the durable sequence — the no-settle
//! outcomes above are attributable to the verification gap, never to a
//! broken fixture.
//!
//! Locator note (the p4/p7 precedent): P11's `evidence_required`
//! locators are repo-root proof-directory spellings that cannot be cargo
//! targets of a core crate, so the proof materializes as an integration
//! test of the composition root whose target NAME is
//! `p11_critic_is_not_verifier`.
//!
//! Composition inventory (spec §4.1 — everything REAL unless this header
//! names the proof-local replacement and why): identical to the p7
//! proof's inventory (real embedded-SQL state store, real fs
//! content-addressed evidence store behind a DELEGATING recorder that
//! captures what the loop pins as forensic evidence, real file-backed
//! reference provider, the p4 proof-local scripted builder completing
//! through the strict result-contract identity check, the REAL command
//! observer re-running the fixture's declared self-check, the REAL diff
//! comparator deciding under `adaptive` review with a zero model
//! budget) with ONE deliberate difference: the verification rung is a
//! proof-local VERDICT rung whose decision the test fixes per leg — the
//! §18.4 separation this proof pins is exactly that the verifier's
//! question is not the comparator's question, so the rung's verdict is
//! the test's controlled variable.
//!
//! COR2-1a/1b interaction (recorded for correction round 2): COR2-1a's
//! one-settlement-per-(run,unit) admission extends the SAME
//! `SettlementCommitAuthority::admit` this proof's tooth neuters; no p11
//! leg re-settles a unit, so the admission change does not move these
//! outcomes. COR2-1b's dispatched-set completion guard reads the
//! builder's declared `attempted` list against the flight's dispatched
//! set — this proof's scripted completions declare NO attempted units,
//! so the guard never engages here; after COR2-1b a leg wanting a
//! completion-path settlement must declare the dispatched unit.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use gauntlet_adapter_agent_mock::{DeterministicClock, SeededIdGenerator, load_scenario};
use gauntlet_adapter_compare_diff::DiffComparator;
use gauntlet_adapter_compare_model::{ModelComparator, ModelComparatorSource};
use gauntlet_adapter_evidence_fs::FsEvidenceStore;
use gauntlet_adapter_meter_memory::MeteredBudget;
use gauntlet_adapter_observe_command::{CommandObserver, CommandSpec};
use gauntlet_adapter_state_postgres::EmbeddedStateStore;
use gauntlet_app::compare::contract::ComparisonContract;
use gauntlet_app::control::controller::{
    BudgetShape, ComparisonDeps, ControllerDeps, RunController,
};
use gauntlet_app::control::disposition::ControlPolicy;
use gauntlet_app::reference::provider::{AnswerKeySource, FileReferenceProvider, ManifestSource};
use gauntlet_domain::{
    Budget, DiagnosticAllowance, Digest, EpochSeconds, EvidenceKind, EvidenceRef, Limits, MicroUsd,
    Outcome, TerminalState, UnitStatus,
};
use gauntlet_ports::agent_runner::{
    AgentError, AgentRunner, CancelReason, InvocationHandle, InvocationStatus, Isolation,
    ResultDelivery, RoleInvocationSpec, RunnerCapabilities,
};
use gauntlet_ports::budget_meter::BudgetMeter;
use gauntlet_ports::clock::Clock;
use gauntlet_ports::comparator::{Comparator, ComparatorSource};
use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::evidence_store::EvidenceStore;
use gauntlet_ports::json::{JsonNumber, JsonValue};
use gauntlet_ports::observer::FacetKind;
use gauntlet_ports::role_result::{RoleResult, RoleResultState, read_result_contract};
use gauntlet_ports::state_store::StateStore;
use gauntlet_ports::verifier::{VerificationRequest, Verifier, VerifierError};

// ---------------------------------------------------------------------------
// Named fixture figures (the literal-scan idiom)
// ---------------------------------------------------------------------------

const BASE_INSTANT: u64 = 1_700_000_000;
const BOUND_AT: u64 = BASE_INSTANT + 90;
/// One forward round, the p7 crash legs' bounded-ending shape: the run
/// ends `budget_exhausted` after iteration 1 — no restart is involved in
/// this proof, so the run-row ceiling discipline never bites.
const ONE_ROUND_FORWARD: u32 = 2;
const FULL_ROUNDS: u32 = 8;
const PARALLELISM: u32 = 2;
const DEPTH_LIMIT: u32 = 2;
const WALL_LIMIT: u64 = 3600;
const TOKEN_CEILING: u64 = 1_000;
const COST_CEILING_MICRO: u64 = 500;
const RESERVE_PCT: u8 = 20;
const DIAGNOSTIC_TOKENS: u64 = 100;
const DIAGNOSTIC_COST_MICRO: u64 = 50;
const TICK_WINDOW: u64 = 1_000;
const QUIET_WINDOW: u64 = 30_000;
const PAUSE_FLOOR: u64 = 1;
const PAUSE_CEILING: u64 = 4;
const ATTEMPT_CEILING: u32 = 4;
const NONCONVERGENCE_N: u32 = 3;
const SCRIPTED_INPUT_TOKENS: u64 = 5;
const SCRIPTED_OUTPUT_TOKENS: u64 = 5;

/// The fixture's single Unit: it settles in iteration 1 when (and only
/// when) the verification chain decides PASS.
const UNIT: &str = "AK-9501";
const SCENARIO_NAME: &str = "lying-builder";
const CORRECTED_ARTIFACT: &str = "PASS the artifact carries the success marker";

const ANSWER_KEY: &str = "## AK-9501\nclaim: the checked artifact passes its focused re-run \
     check\nverify_by: test\nblast_radius: contained\nauthority: critic\nevidence_required: {kind: \
     test_report, locator: p11-report.json, producer: instrument}\ndepends_on: []\n";

const MANIFEST: &str = "reference:\n  id: ref-p11-proof-suite\n  version: 1\n  purpose: >\n    \
     One paragraph.\n  desired_state: >\n    The destination.\n  settlement:\n    \
     success_criteria:\n      - the destination is reached\n    minimum_reliability: >\n      \
     spec 19.2 default: >= 1 settlements before promotion.\n    terminal_conditions: \
     [settled, aborted]\n";

const COMPARISON_CONTRACT: &str = "version: 1\ndimensions:\n  - functional behavior\n\
     severity_rubric:\n  - blocking | invalidates the iteration\n  - major | real, not \
     invalidating\n  - minor | small\n  - informational | recorded only\ncomparator_receives:\n  \
     - the allowlist members\nresidual_definition:\n  - a difference\n";

// ---------------------------------------------------------------------------
// Scratch
// ---------------------------------------------------------------------------

struct Scratch {
    path: PathBuf,
}

impl Scratch {
    fn new(tag: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("gauntlet-p11-proof-{tag}-{}", std::process::id()));
        std::fs::remove_dir_all(&path).ok();
        std::fs::create_dir_all(&path).expect("the p11 scratch parent is creatable");
        Self { path }
    }

    fn child(&self, name: &str) -> PathBuf {
        let path = self.path.join(name);
        std::fs::create_dir_all(&path).expect("the p11 scratch child is creatable");
        path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.path).ok();
    }
}

fn bound() -> Deadline {
    Deadline::at_epoch(EpochSeconds::new(BOUND_AT))
}

fn policy() -> ControlPolicy {
    ControlPolicy::new(
        TICK_WINDOW,
        QUIET_WINDOW,
        PAUSE_FLOOR,
        PAUSE_CEILING,
        ATTEMPT_CEILING,
        NONCONVERGENCE_N,
    )
    .expect("the proof control policy is usable")
}

fn budget_shape(rounds: u32) -> BudgetShape {
    BudgetShape {
        limits: Limits::new(
            rounds,
            PARALLELISM,
            DEPTH_LIMIT,
            WALL_LIMIT,
            TOKEN_CEILING,
            MicroUsd::new(COST_CEILING_MICRO),
        )
        .expect("proof limits usable"),
        recovery_reserve_pct: RESERVE_PCT,
        diagnostic: DiagnosticAllowance::new(
            DIAGNOSTIC_TOKENS,
            MicroUsd::new(DIAGNOSTIC_COST_MICRO),
        ),
    }
}

fn meter_factory() -> Arc<dyn Fn(Budget) -> Arc<dyn BudgetMeter> + Send + Sync> {
    Arc::new(|budget| Arc::new(MeteredBudget::new(budget)))
}

fn proof_env() -> Vec<(String, String)> {
    vec![(
        "GAUNTLET_PROOF_ENV".to_owned(),
        "p11-critic-is-not-verifier".to_owned(),
    )]
}

// ---------------------------------------------------------------------------
// The forensic-recording evidence decorator (delegating; P11's observer)
// ---------------------------------------------------------------------------

/// The REAL fs store behind a delegating recorder. No behavior changes:
/// every method forwards verbatim. The records exist so the proof can
/// read what the loop PINNED as forensic evidence when the settlement
/// service refused — the refusal arm's pinned record is the durable
/// place the real cause is named.
struct ForensicRecorder {
    inner: Arc<FsEvidenceStore>,
    pinned_forensics: RefCell<Vec<String>>,
}

impl ForensicRecorder {
    fn new(inner: Arc<FsEvidenceStore>) -> Arc<Self> {
        #[allow(clippy::arc_with_non_send_sync)] // single-threaded proof fixture wiring
        Arc::new(Self {
            inner,
            pinned_forensics: RefCell::new(Vec::new()),
        })
    }

    /// Every forensic record the loop wrote for a refused settlement,
    /// decoded as text (the refusal arm writes UTF-8 JSON lines).
    fn refused_settlement_records(&self) -> Vec<String> {
        self.pinned_forensics.borrow().clone()
    }
}

impl EvidenceStore for ForensicRecorder {
    fn put(
        &self,
        bytes: &[u8],
        kind: gauntlet_domain::EvidenceKind,
        deadline: Deadline,
    ) -> Result<EvidenceRef, gauntlet_ports::evidence_store::EvidenceStoreError> {
        if kind == gauntlet_domain::EvidenceKind::Log
            && let Ok(text) = std::str::from_utf8(bytes)
            && text.contains("settlement_refused")
        {
            self.pinned_forensics.borrow_mut().push(text.to_owned());
        }
        self.inner.put(bytes, kind, deadline)
    }
    fn get(
        &self,
        reference: &EvidenceRef,
        deadline: Deadline,
    ) -> Result<Option<Vec<u8>>, gauntlet_ports::evidence_store::EvidenceStoreError> {
        self.inner.get(reference, deadline)
    }
    fn pin(
        &self,
        run_id: &gauntlet_domain::RunId,
        reference: &EvidenceRef,
        deadline: Deadline,
    ) -> Result<(), gauntlet_ports::evidence_store::EvidenceStoreError> {
        self.inner.pin(run_id, reference, deadline)
    }
    fn is_pinned(
        &self,
        run_id: &gauntlet_domain::RunId,
        reference: &EvidenceRef,
        deadline: Deadline,
    ) -> Result<bool, gauntlet_ports::evidence_store::EvidenceStoreError> {
        self.inner.is_pinned(run_id, reference, deadline)
    }
}

// ---------------------------------------------------------------------------
// The verdict rung (the proof's controlled variable)
// ---------------------------------------------------------------------------

/// What the verification rung decides for the leg's claim. `Decline` is
/// the `ladder absent` shape: an empty ladder is refused at composition
/// (`LadderError::Empty`), so the honest spelling of "no rung exists for
/// this requirement" is a rung whose `supports` refuses it — the ladder
/// then reports `NoRungCanDecide`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Verdict {
    Pass,
    Fail,
    Inconclusive,
    /// A supported rung breaking mechanically (§18.4 instrument failure).
    Break,
    /// No rung declares it can decide the requirement.
    Decline,
}

struct VerdictRung {
    verdict: Verdict,
}

impl VerdictRung {
    const INSTRUMENT: &'static str = "gauntlet-p11-verdict-rung";
}

impl Verifier for VerdictRung {
    fn verify(
        &self,
        claim: &VerificationRequest,
        _: Deadline,
    ) -> Result<gauntlet_domain::VerificationRecord, VerifierError> {
        let outcome = match self.verdict {
            Verdict::Pass => Outcome::Pass,
            Verdict::Fail => Outcome::Fail,
            Verdict::Inconclusive => Outcome::Inconclusive,
            Verdict::Break => {
                return Err(VerifierError::InstrumentFailed {
                    detail: "the p11 proof's rung broke on purpose (the 18.4 instrument-failure \
                             class)"
                        .to_owned(),
                });
            }
            Verdict::Decline => {
                return Err(VerifierError::RequirementUnsupported {
                    detail: "the p11 proof's rung declines the requirement".to_owned(),
                });
            }
        };
        Ok(gauntlet_domain::VerificationRecord::new(
            claim.id.clone(),
            claim.run_id.clone(),
            claim.unit_id.clone(),
            EvidenceRef::new(claim.evidence.as_str())
                .expect("a single-segment locator is a valid ref string"),
            outcome,
            gauntlet_domain::InstrumentName::new(Self::INSTRUMENT)
                .expect("the rung's instrument name is valid"),
        ))
    }

    fn is_deterministic(&self) -> bool {
        true
    }

    fn instrument(&self) -> gauntlet_ports::observer::InstrumentIdentity {
        gauntlet_ports::observer::InstrumentIdentity::new(
            gauntlet_domain::InstrumentName::new(Self::INSTRUMENT)
                .expect("the rung's instrument name is valid"),
            "1".to_owned(),
        )
    }

    fn supports(&self, requirement: &gauntlet_domain::EvidenceRequirement) -> bool {
        if self.verdict == Verdict::Decline {
            return false;
        }
        requirement.kind() == EvidenceKind::TestReport
    }
}

// ---------------------------------------------------------------------------
// The scripted builder (the p4 proof-local runner; same documented deviation)
// ---------------------------------------------------------------------------

struct ScriptedBuilder {
    root: PathBuf,
    artifact_relative: String,
    prompts: RefCell<Vec<String>>,
    run_id: RefCell<Option<gauntlet_domain::RunId>>,
    specs: RefCell<HashMap<gauntlet_domain::InvocationId, RoleInvocationSpec>>,
}

impl ScriptedBuilder {
    fn new(root: PathBuf, artifact_relative: String) -> Arc<Self> {
        #[allow(clippy::arc_with_non_send_sync)] // single-threaded proof fixture wiring
        Arc::new(Self {
            root,
            artifact_relative,
            prompts: RefCell::new(Vec::new()),
            run_id: RefCell::new(None),
            specs: RefCell::new(HashMap::new()),
        })
    }

    fn prompts(&self) -> usize {
        self.prompts.borrow().len()
    }

    /// The run the dispatch flew under — the honest run identity for the
    /// legs whose `start_run` errors before returning an outcome.
    fn run_id(&self) -> gauntlet_domain::RunId {
        self.run_id
            .borrow()
            .clone()
            .expect("the builder was dispatched under a run id")
    }

    fn apply_effect(&self, spec: &RoleInvocationSpec, content: &str) {
        let target = self
            .root
            .join(spec.workspace.as_str())
            .join(&self.artifact_relative);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).expect("the artifact's parent is creatable");
        }
        std::fs::write(&target, content).expect("the scripted builder writes its artifact");
    }

    fn result_document(spec: &RoleInvocationSpec) -> String {
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
                    ("attempted".to_owned(), JsonValue::Array(vec![])),
                    ("changed_paths".to_owned(), JsonValue::Array(vec![])),
                    ("commands_run".to_owned(), JsonValue::Array(vec![])),
                    ("artifacts".to_owned(), JsonValue::Array(vec![])),
                    (
                        "claims".to_owned(),
                        JsonValue::Array(vec![JsonValue::Str(
                            "the slice is complete and the check passes".to_owned(),
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
                        JsonValue::Number(JsonNumber::Uint(SCRIPTED_INPUT_TOKENS)),
                    ),
                    (
                        "output_tokens".to_owned(),
                        JsonValue::Number(JsonNumber::Uint(SCRIPTED_OUTPUT_TOKENS)),
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

impl AgentRunner for ScriptedBuilder {
    fn invoke(&self, spec: RoleInvocationSpec) -> Result<InvocationHandle, AgentError> {
        self.prompts.borrow_mut().push(spec.prompt.clone());
        if self.run_id.borrow().is_none() {
            self.run_id.borrow_mut().replace(spec.run_id.clone());
        }
        self.apply_effect(&spec, CORRECTED_ARTIFACT);
        self.specs
            .borrow_mut()
            .insert(spec.invocation_id.clone(), spec.clone());
        Ok(InvocationHandle::new(spec.invocation_id, None))
    }

    fn poll(&self, handle: &InvocationHandle) -> Result<InvocationStatus, AgentError> {
        let spec = self
            .specs
            .borrow()
            .get(handle.invocation_id())
            .cloned()
            .expect("the handle was admitted");
        let document = Self::result_document(&spec);
        read_result_contract(
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

    fn collect(&self, handle: &InvocationHandle, _: Deadline) -> Result<RoleResult, AgentError> {
        let spec = self
            .specs
            .borrow()
            .get(handle.invocation_id())
            .cloned()
            .expect("the handle was admitted");
        let document = Self::result_document(&spec);
        read_result_contract(
            &document,
            handle.invocation_id(),
            spec.context_envelope.digest(),
        )
        .map_err(|error| AgentError::CollectionFailed {
            invocation_id: handle.invocation_id().clone(),
            detail: error.to_string(),
        })
    }

    fn cancel(&self, _: &InvocationHandle, _: CancelReason, _: Deadline) -> Result<(), AgentError> {
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

// ---------------------------------------------------------------------------
// The never-admitted critic + the real diff sources (the p7 inventory)
// ---------------------------------------------------------------------------

struct NeverAdmittedCritic;

impl AgentRunner for NeverAdmittedCritic {
    fn invoke(&self, spec: RoleInvocationSpec) -> Result<InvocationHandle, AgentError> {
        Err(AgentError::AdmissionRefused {
            detail: format!(
                "the proof's zero model allowance was bypassed for invocation {}",
                spec.invocation_id
            ),
        })
    }
    fn poll(&self, handle: &InvocationHandle) -> Result<InvocationStatus, AgentError> {
        Err(AgentError::AdmissionRefused {
            detail: format!("invocation {} was never admitted", handle.invocation_id()),
        })
    }
    fn collect(&self, handle: &InvocationHandle, _: Deadline) -> Result<RoleResult, AgentError> {
        Err(AgentError::CollectionFailed {
            invocation_id: handle.invocation_id().clone(),
            detail: "the proof's critic was never admitted".to_owned(),
        })
    }
    fn cancel(
        &self,
        handle: &InvocationHandle,
        _: CancelReason,
        _: Deadline,
    ) -> Result<(), AgentError> {
        Err(AgentError::AdmissionRefused {
            detail: format!("invocation {} was never admitted", handle.invocation_id()),
        })
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

struct DiffSource;

impl ComparatorSource for DiffSource {
    fn across_dimensions(&self) -> Result<Box<dyn Comparator>, String> {
        Ok(Box::new(DiffComparator::across_dimensions()))
    }
    fn for_dimension(&self, dimension: &str) -> Result<Box<dyn Comparator>, String> {
        Ok(Box::new(DiffComparator::for_dimension(dimension)))
    }
}

/// The recording escalation surface (§11.6; proof-local, no real adapter).
#[derive(Default)]
struct RecordingEscalation {
    tickets: std::sync::Mutex<Vec<String>>,
}

impl gauntlet_ports::human_escalation::HumanEscalation for RecordingEscalation {
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
            gauntlet_ports::human_escalation::TicketId::new("ticket-p11")
                .expect("the fixture ticket id is valid"),
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

// ---------------------------------------------------------------------------
// The composed fixture
// ---------------------------------------------------------------------------

struct Fixture {
    controller: RunController,
    state: Arc<EmbeddedStateStore>,
    evidence: Arc<ForensicRecorder>,
    runner: Arc<ScriptedBuilder>,
    /// Holds the scratch world alive for the fixture's lifetime.
    _scratch: Scratch,
}

/// Composes the real controller over a scratch world with the verdict
/// rung fixed to the leg's decision. `rounds` picks the bounded ending
/// (the PASS control leg needs the full ceiling; the no-settle legs use
/// the one-forward-round shape).
fn compose(tag: &str, verdict: Verdict, rounds: u32) -> Fixture {
    let scratch = Scratch::new(tag);
    let workspace_root = scratch.child("workspace");
    let evidence_root = scratch.child("evidence");
    let key_dir = scratch.child("ruler");
    let key_path = key_dir.join("p11-answer-key.md");
    std::fs::write(&key_path, ANSWER_KEY).expect("the scratch answer key is written");
    let db = scratch.child("state").join("p11-state.db");

    let evidence = ForensicRecorder::new(Arc::new(
        FsEvidenceStore::open(&evidence_root).expect("the real fs evidence store opens"),
    ));
    let state: Arc<EmbeddedStateStore> =
        Arc::new(EmbeddedStateStore::open_at(&db).expect("the real embedded state store opens"));
    let clock = Arc::new(DeterministicClock::start_at(EpochSeconds::new(
        BASE_INSTANT,
    )));
    let scenario = load_scenario(SCENARIO_NAME).expect("the REQUIRED fixture parses");
    let declared = scenario
        .payload()
        .expect("the fixture declares its payload")
        .self_checks()
        .first()
        .expect("the fixture declares exactly the self-check to re-run")
        .command();
    let mut tokens = declared.split_whitespace();
    let check_program = tokens
        .next()
        .expect("the declared check names a program")
        .to_owned();
    let check_args: Vec<String> = tokens.map(str::to_owned).collect();
    let artifact_relative = scenario
        .payload()
        .expect("the payload is declared")
        .changed_paths()
        .first()
        .expect("the fixture names its changed path")
        .clone();
    let runner = ScriptedBuilder::new(workspace_root.clone(), artifact_relative);
    #[allow(clippy::arc_with_non_send_sync)] // single-threaded proof fixture wiring
    let observer_clock: Arc<dyn Clock> = clock.clone();
    #[allow(clippy::arc_with_non_send_sync)] // single-threaded proof fixture wiring
    let observers: Vec<Arc<dyn gauntlet_ports::observer::Observer>> =
        vec![Arc::new(CommandObserver::from_parts(
            CommandSpec::new(check_program, check_args),
            FacetKind::Test,
            workspace_root.clone(),
            observer_clock,
            evidence.clone(),
            &proof_env(),
        ))];
    let contract = ComparisonContract::parse(COMPARISON_CONTRACT).expect("the contract parses");
    #[allow(clippy::arc_with_non_send_sync)] // single-threaded proof fixture wiring
    let model = ModelComparatorSource::new(ModelComparator::with_clock(
        Arc::new(NeverAdmittedCritic),
        Arc::new(SeededIdGenerator::new("p11-proof-critic-identity").expect("a non-empty seed")),
        None,
        Budget::new(
            Limits::new(
                FULL_ROUNDS,
                PARALLELISM,
                DEPTH_LIMIT,
                WALL_LIMIT,
                TOKEN_CEILING,
                MicroUsd::new(COST_CEILING_MICRO),
            )
            .expect("proof limits usable"),
            RESERVE_PCT,
            DiagnosticAllowance::new(DIAGNOSTIC_TOKENS, MicroUsd::new(DIAGNOSTIC_COST_MICRO)),
        )
        .expect("proof budget usable"),
        clock.clone(),
    ));
    #[allow(clippy::arc_with_non_send_sync)] // single-threaded proof fixture wiring
    let provider = Arc::new(FileReferenceProvider::from_sources(
        AnswerKeySource::Path(key_path.clone()),
        ManifestSource::Inline(MANIFEST.to_owned()),
        Rc::new(
            FsEvidenceStore::open(&evidence_root)
                .expect("the provider's real evidence store opens"),
        ),
    ));
    #[allow(clippy::arc_with_non_send_sync)] // single-threaded proof fixture wiring
    let deps = ControllerDeps {
        state: state.clone(),
        evidence: evidence.clone(),
        ids: Arc::new(
            SeededIdGenerator::new(&format!("p11-proof-{tag}")).expect("a non-empty seed"),
        ),
        clock,
        runner: runner.clone(),
        runner_identity: gauntlet_domain::RunnerIdentity::new("p11-scripted-builder")
            .expect("runner identity"),
        observers,
        comparison: ComparisonDeps {
            contract,
            deterministic: Arc::new(DiffSource),
            model: Arc::new(model),
            adversarial: None,
            review_mode: gauntlet_domain::ReviewMode::Adaptive,
            model_dispatches: 0,
            role_deny: gauntlet_app::roles::ContextDenySet::new(Vec::new()),
        },
        ladder_for: Arc::new(move |_| {
            Ok(gauntlet_app::verify::ladder::Ladder::new(vec![
                gauntlet_app::verify::ladder::LadderRung::new(
                    gauntlet_app::verify::ladder::RungKind::TestReport,
                    Arc::new(VerdictRung { verdict }),
                ),
            ])
            .expect("the single-rung verdict ladder composes"))
        }),
        reference: provider,
        answer_key: AnswerKeySource::Path(key_path),
        escalation: Arc::new(RecordingEscalation::default()),
        meter_factory: meter_factory(),
        budget: budget_shape(rounds),
        policy: policy(),
        workspace_root,
        config_fingerprint: Digest::new("sha256", &"ab".repeat(32)).expect("digest"),
        // The §8.6 conservative posture (plan Task 14): contained
        // critic-tier dispatches run ungated by containment; the mock
        // runner's honest none isolation makes above-contained refusal
        // the correct posture here.
        govern: gauntlet_app::govern::GovernDeps::none_backend(),
        workspaces: None,
        environment_fingerprint: Digest::new("sha256", &"cd".repeat(32)).expect("digest"),
        intent: "p11 critic-is-not-verifier proof".to_owned(),
    };
    let controller = RunController::compose(deps).expect("the composed controller builds");
    Fixture {
        controller,
        state,
        evidence,
        runner,
        _scratch: scratch,
    }
}

// ---------------------------------------------------------------------------
// Shared leg assertions
// ---------------------------------------------------------------------------

/// The P11 core, read through the REAL durable surfaces: NO
/// `SettlementCommitted` event in the run's own export, NO settlements
/// row, NO verification record (the claim never became durable — the
/// service refuses before `ensure_verification_recorded` for non-PASS
/// claims, and a refused chain never mints one), and the projection
/// stays open at the last durable checkpoint.
///
/// The comparison's cleanliness is observed by POSITION, not by proxy:
/// every leg's primary observable sits DOWNSTREAM of the comparison's
/// acceptance — the ladder refusals (shapes 1-2) can only fire after the
/// drain passed compare, and the settle-time forensic pins (shapes 3-4)
/// can only be written after compare admitted the observation — and a
/// blocking comparison residual would have produced a §16.1 condition-4
/// refusal naming the residual instead of the outcome each leg pins.
fn assert_never_settled(fixture: &Fixture, run_id: &gauntlet_domain::RunId) {
    let events = fixture
        .state
        .export_jsonl(run_id, bound())
        .expect("the export reads");
    assert!(
        !events
            .lines()
            .any(|line| line.contains("settlement_committed")),
        "no SettlementCommitted event exists for the comparator-clean unit: {events}"
    );
    assert!(
        fixture
            .state
            .settlements(run_id, bound())
            .expect("settlement rows read")
            .is_empty(),
        "no settlements row exists"
    );
    assert!(
        fixture
            .state
            .verification_records(run_id, bound())
            .expect("verification records read")
            .is_empty(),
        "no verification record exists — the comparator's pass never minted one (P11 core)"
    );
    let checkpoint = fixture
        .state
        .resume(run_id, bound())
        .expect("checkpoints read")
        .expect("the run left a checkpoint");
    assert_eq!(
        checkpoint
            .unit_states
            .iter()
            .find(|(unit, _)| unit.as_str() == UNIT)
            .map(|(_, status)| *status),
        Some(UnitStatus::Open),
        "the projection stays open at the last durable checkpoint"
    );
}

fn unit_was_attempted_once(fixture: &Fixture) {
    assert_eq!(
        fixture.runner.prompts(),
        1,
        "the unit was dispatched exactly once — the loop reached comparison and verification"
    );
}

// ---------------------------------------------------------------------------
// The proofs
// ---------------------------------------------------------------------------

/// The PASS control: the same composition with the rung deciding PASS
/// settles the unit through the durable sequence — the no-settle
/// outcomes in the other legs are attributable to the verification gap,
/// never to a broken fixture.
#[test]
fn a_pass_verdict_settles_the_same_composition() {
    let fixture = compose("pass-control", Verdict::Pass, FULL_ROUNDS);
    let outcome = fixture
        .controller
        .start_run()
        .expect("the composed loop settles the unit when verification decides PASS");
    assert_eq!(outcome.terminal, TerminalState::Settled);
    let rows = fixture
        .state
        .settlements(&outcome.run_id, bound())
        .expect("rows read");
    assert_eq!(rows.len(), 1, "the PASS claim settled durably: {rows:?}");
    assert_eq!(
        rows[0].unit_id().as_str(),
        UNIT,
        "the settled unit is the proof's unit"
    );
    assert!(
        !fixture
            .state
            .verification_records(&outcome.run_id, bound())
            .expect("records read")
            .is_empty(),
        "the PASS claim became a durable verification record"
    );
}

/// P11, refusal shape 1 (§18.4 instrument failure): the comparison is
/// clean, the builder completed, the observation bound — and the
/// supported rung BREAKS. The run errors out of the drain with the typed
/// ladder refusal naming the broken instrument, and durably nothing
/// settled: no event, no row, no verification record, projection open.
#[test]
fn a_broken_verifier_refuses_and_nothing_settles() {
    let fixture = compose("rung-broken", Verdict::Break, ONE_ROUND_FORWARD);
    let error = fixture
        .controller
        .start_run()
        .expect_err("a supported rung breaking mechanically errors the run (18.4)");
    let text = error.to_string();
    assert!(
        text.contains("the verification ladder refused")
            && text.contains("gauntlet-p11-verdict-rung")
            && text.contains("broke on purpose"),
        "the refusal names the broken instrument: {text}"
    );
    let run_id = fixture.runner.run_id();
    assert_never_settled(&fixture, &run_id);
    unit_was_attempted_once(&fixture);
}

/// P11, refusal shape 2 (the `ladder absent` shape): no rung declares it
/// can decide the requirement — the honest spelling of an absent ladder,
/// since an empty ladder is refused at composition (`LadderError::Empty`)
/// and cannot be wired at all. The same typed refusal, the same durable
/// nothing.
#[test]
fn no_rung_can_decide_and_nothing_settles() {
    let fixture = compose("rung-absent", Verdict::Decline, ONE_ROUND_FORWARD);
    let error = fixture
        .controller
        .start_run()
        .expect_err("a requirement no rung can decide errors the run");
    let text = error.to_string();
    assert!(
        text.contains("the verification ladder refused")
            && text.contains("no rung on the ladder can decide the requirement"),
        "the refusal names the undecided requirement: {text}"
    );
    let run_id = fixture.runner.run_id();
    assert_never_settled(&fixture, &run_id);
    unit_was_attempted_once(&fixture);
}

/// P11, outcome shape INCONCLUSIVE (AK-005's three-state rule at the
/// settlement seam): the chain MINTS the inconclusive claim over the
/// clean comparison, the settlement service REFUSES it at the entry
/// gate, the refusal is pinned as forensic evidence naming
/// INCONCLUSIVE — visibly distinct from FAIL — and nothing settles.
#[test]
fn an_inconclusive_verdict_never_settles_and_names_itself() {
    let fixture = compose("inconclusive", Verdict::Inconclusive, ONE_ROUND_FORWARD);
    let outcome = fixture
        .controller
        .start_run()
        .expect("a refused settlement is a loop fact, never a run error");
    assert_eq!(outcome.terminal, TerminalState::BudgetExhausted);
    assert_never_settled(&fixture, &outcome.run_id);
    unit_was_attempted_once(&fixture);
    let forensics = fixture.evidence.refused_settlement_records();
    assert!(
        forensics
            .iter()
            .any(|record| record.contains("settlement_refused")
                && record.contains("outcome is INCONCLUSIVE")),
        "the pinned forensic evidence names the INCONCLUSIVE outcome: {forensics:?}"
    );
    assert!(
        forensics
            .iter()
            .all(|record| !record.contains("outcome is FAIL")),
        "the INCONCLUSIVE leg never names FAIL — three states, never collapsed: {forensics:?}"
    );
}

/// P11, outcome shape FAIL over the same clean comparison: the service
/// entry gate refuses, the pinned forensic evidence names FAIL, nothing
/// settles — the comparator's pass never closes the unit (§13.1).
#[test]
fn a_fail_verdict_never_settles_and_names_the_real_cause() {
    let fixture = compose("fail", Verdict::Fail, ONE_ROUND_FORWARD);
    let outcome = fixture
        .controller
        .start_run()
        .expect("a refused settlement is a loop fact, never a run error");
    assert_eq!(outcome.terminal, TerminalState::BudgetExhausted);
    assert_never_settled(&fixture, &outcome.run_id);
    unit_was_attempted_once(&fixture);
    let forensics = fixture.evidence.refused_settlement_records();
    assert!(
        forensics
            .iter()
            .any(|record| record.contains("settlement_refused")
                && record.contains("the settling claim's own outcome is FAIL")),
        "the pinned forensic evidence names the FAIL outcome — the real cause: {forensics:?}"
    );
}
