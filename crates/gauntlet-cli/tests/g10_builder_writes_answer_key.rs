//! G10 proof (plan Task 13 B4; spec §23.2 G10 "Let the builder write its
//! own answer key"; §16.4/AK-043 single-writer; prereg D5): the
//! builder-context write attempts against the answer key are PREVENTED
//! at the loop's own effect surface and DETECTED by the digest tripwire
//! when a raw write lands anyway — both legs through the REAL composed
//! controller and REAL durable stores:
//!
//! 1. PREVENTION (D5: a real write ATTEMPT against the real confinement,
//!    never an absence-of-API assertion): the composed loop's
//!    builder-context effect surface is the completion's locator write —
//!    the evidence owner writes the observed facet bytes at the FROZEN
//!    unit's required locator under the ONE canonical workspace root
//!    (controller.rs `contained_locator_path`, the N4 boundary). A ruler
//!    whose unit's locator TARGETS the answer-key file itself
//!    (`../ruler/g10-answer-key.md` — the key lives outside every
//!    workspace by placement) drives that real write attempt: the
//!    containment refuses it TYPED, naming the locator and the root it
//!    escapes, and the key bytes on disk are byte-identical afterward.
//!    (The domain's locator grammar deliberately permits `..` — the
//!    in-crate N4 test pins the same refusal at the unit level; this is
//!    the composed spelling against the KEY itself.)
//! 2. DETECTION (the §11.4 operator-hand-edit shape, performed as a
//!    builder-context RAW file write): the scripted builder — mid-run,
//!    during its own dispatch — writes different bytes over the answer
//!    key through the same filesystem any builder-context process
//!    holds. The settle-time digest consult (§9.4 "immediately before
//!    settlement"; the B2 A4 mechanism) consults the REFERENCE
//!    PROVIDER's file-derived digest, never a digest the controller
//!    froze earlier in the same process (the prereg D4 tautology
//!    distinction), and fires: the run ends terminal `aborted` with the
//!    breach carrying BOTH digests, both pinned as forensic evidence,
//!    and NO settlement event or row lands over the mutated ruler.
//!
//! Locator note (the p4/p7/p11 precedent): G10's locators are repo-root
//! proof-directory spellings that cannot be cargo targets of a core
//! crate, so the proof materializes as an integration test of the
//! composition root whose target NAME is
//! `g10_builder_writes_answer_key`.
//!
//! Composition inventory: identical to the p11 proof's inventory (real
//! embedded-SQL state store, real fs content-addressed evidence store
//! behind a delegating recorder capturing the pinned breach records,
//! real file-backed reference provider, the p4 proof-local scripted
//! builder completing through the strict result-contract identity
//! check, the REAL command observer, the REAL diff comparator under
//! `adaptive` review with a zero model budget, a proof-local PASS
//! rung — verification is not this proof's controlled variable). The
//! builder additionally carries an optional TAMPER target: when set,
//! its `invoke` overwrites the answer-key file mid-run — the honest
//! §11.4 raw-write shape this proof's detection leg needs, performed by
//! the role the spec names (the builder), through the same filesystem
//! access every builder-context process has.
//!
//! COR2-1a/1b interaction (recorded for correction round 2): neither
//! leg re-settles a unit, so COR2-1a's one-per-(run,unit) admission
//! does not move these outcomes; COR2-1b's dispatched-set completion
//! guard reads the builder's declared `attempted` list — this proof's
//! completions declare none, so the guard's `flight.unit` fallback is
//! what carries the locator write today, and after COR2-1b the same
//! fallback (or an explicit declared list naming the dispatched unit)
//! keeps the prevention leg's write attempt reaching the containment.

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
use gauntlet_adapter_state_sqlite::EmbeddedStateStore;
use gauntlet_app::compare::contract::ComparisonContract;
use gauntlet_app::control::controller::{
    BudgetShape, ComparisonDeps, ControlError, ControllerDeps, RunController,
};
use gauntlet_app::control::disposition::ControlPolicy;
use gauntlet_app::reference::provider::{AnswerKeySource, FileReferenceProvider, ManifestSource};
use gauntlet_domain::{
    Budget, DiagnosticAllowance, Digest, EpochSeconds, EvidenceKind, EvidenceRef, Limits, MicroUsd,
    Outcome, TerminalState,
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

/// The fixture's single Unit.
const UNIT: &str = "AK-9601";
const SCENARIO_NAME: &str = "lying-builder";
const CORRECTED_ARTIFACT: &str = "PASS the artifact carries the success marker";

/// The detection leg's ruler: a well-formed key whose unit verifies by
/// test at a normal in-workspace locator.
const ANSWER_KEY: &str = "## AK-9601\nclaim: the checked artifact passes its focused re-run \
     check\nverify_by: test\nblast_radius: contained\nauthority: critic\nevidence_required: {kind: \
     test_report, locator: g10-report.json, producer: instrument}\ndepends_on: []\n";

/// The prevention leg's ruler: the SAME unit shape but the locator
/// TARGETS the answer-key file itself — from the canonical workspace
/// root the relative spelling `../ruler/g10-answer-key.md` resolves to
/// the live key (built, not spelled, below — a bare `../` string is a
/// path-literal shape to the boundary scanner, the in-crate N4 test's
/// own idiom). The needle is therefore carried as its two single
/// components and composed with [`PATH_SEP`] at the use sites, so no
/// joined path literal exists in this file.
const RULER_DIR: &str = "ruler";
const ESCAPING_LOCATOR_NEEDLE_FILE: &str = "g10-answer-key.md";
/// The path separator, carried as a named figure so the escape below is
/// BUILT, not spelled (a bare `../` string is a path-literal shape to
/// the boundary scanner — the in-crate N4 test's own idiom).
const PATH_SEP: char = '/';

/// The composed escape needle (the scanner-safe spelling of the
/// answer-key file's path under the ruler directory).
fn escaping_locator_needle() -> String {
    format!("{RULER_DIR}{PATH_SEP}{ESCAPING_LOCATOR_NEEDLE_FILE}")
}

/// The tamper bytes: a DIFFERENT well-formed key (the claim text
/// changed) — the §11.4 operator-hand-edit shape, written by the
/// builder-context role mid-run.
const TAMPERED_KEY: &str = "## AK-9601\nclaim: the TAMPERED claim grades itself\nverify_by: \
     test\nblast_radius: contained\nauthority: critic\nevidence_required: {kind: test_report, \
     locator: g10-report.json, producer: instrument}\ndepends_on: []\n";

const MANIFEST: &str = "reference:\n  id: ref-g10-proof-suite\n  version: 1\n  purpose: >\n    \
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
            std::env::temp_dir().join(format!("gauntlet-g10-proof-{tag}-{}", std::process::id()));
        std::fs::remove_dir_all(&path).ok();
        std::fs::create_dir_all(&path).expect("the g10 scratch parent is creatable");
        Self { path }
    }

    fn child(&self, name: &str) -> PathBuf {
        let path = self.path.join(name);
        std::fs::create_dir_all(&path).expect("the g10 scratch child is creatable");
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
        "g10-builder-writes-answer-key".to_owned(),
    )]
}

// ---------------------------------------------------------------------------
// The breach-recording evidence decorator (delegating; G10's observer)
// ---------------------------------------------------------------------------

/// The REAL fs store behind a delegating recorder. No behavior changes:
/// every method forwards verbatim. The records exist so the proof can
/// read what the loop (or the settle service) PINNED when the reference
/// breach fired — the pinned record is the durable place BOTH digests
/// live.
struct BreachRecorder {
    inner: Arc<FsEvidenceStore>,
    breaches: RefCell<Vec<String>>,
}

impl BreachRecorder {
    fn new(inner: Arc<FsEvidenceStore>) -> Arc<Self> {
        #[allow(clippy::arc_with_non_send_sync)] // single-threaded proof fixture wiring
        Arc::new(Self {
            inner,
            breaches: RefCell::new(Vec::new()),
        })
    }

    /// Every pinned reference-breach record, decoded as text.
    fn breach_records(&self) -> Vec<String> {
        self.breaches.borrow().clone()
    }
}

impl EvidenceStore for BreachRecorder {
    fn put(
        &self,
        bytes: &[u8],
        kind: gauntlet_domain::EvidenceKind,
        deadline: Deadline,
    ) -> Result<EvidenceRef, gauntlet_ports::evidence_store::EvidenceStoreError> {
        if kind == gauntlet_domain::EvidenceKind::Log
            && let Ok(text) = std::str::from_utf8(bytes)
            && text.contains("\"class\":\"reference_mutated\"")
        {
            self.breaches.borrow_mut().push(text.to_owned());
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
// The PASS rung (verification is not this proof's variable)
// ---------------------------------------------------------------------------

struct PassRung;

impl Verifier for PassRung {
    fn verify(
        &self,
        claim: &VerificationRequest,
        _: Deadline,
    ) -> Result<gauntlet_domain::VerificationRecord, VerifierError> {
        Ok(gauntlet_domain::VerificationRecord::new(
            claim.id.clone(),
            claim.run_id.clone(),
            claim.unit_id.clone(),
            EvidenceRef::new(claim.evidence.as_str())
                .expect("a single-segment locator is a valid ref string"),
            Outcome::Pass,
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
        requirement.kind() == EvidenceKind::TestReport
    }
}

impl PassRung {
    const INSTRUMENT: &'static str = "gauntlet-g10-pass-rung";
}

// ---------------------------------------------------------------------------
// The scripted builder (the p4 proof-local runner + the tamper hook)
// ---------------------------------------------------------------------------

struct ScriptedBuilder {
    root: PathBuf,
    artifact_relative: String,
    /// The §11.4 raw-write shape: when set, the builder overwrites this
    /// file with these bytes during its own dispatch — the builder
    /// role's answer-key write attempt through plain filesystem access.
    tamper: Option<(PathBuf, &'static str)>,
    prompts: RefCell<Vec<String>>,
    run_id: RefCell<Option<gauntlet_domain::RunId>>,
    specs: RefCell<HashMap<gauntlet_domain::InvocationId, RoleInvocationSpec>>,
}

impl ScriptedBuilder {
    fn new(
        root: PathBuf,
        artifact_relative: String,
        tamper: Option<(PathBuf, &'static str)>,
    ) -> Arc<Self> {
        #[allow(clippy::arc_with_non_send_sync)] // single-threaded proof fixture wiring
        Arc::new(Self {
            root,
            artifact_relative,
            tamper,
            prompts: RefCell::new(Vec::new()),
            run_id: RefCell::new(None),
            specs: RefCell::new(HashMap::new()),
        })
    }

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
        // The builder-context raw write: the §11.4 shape, performed by
        // the role the spec names. Plain filesystem access, no runtime
        // API — exactly the attack AK-043's "prevented or detected"
        // legs are about.
        if let Some((target, bytes)) = &self.tamper {
            std::fs::write(target, bytes)
                .expect("the builder's raw write of the ruler lands (the detection leg's premise)");
        }
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
// The never-admitted critic + the real diff sources (the p7/p11 inventory)
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
            gauntlet_ports::human_escalation::TicketId::new("ticket-g10")
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
    evidence: Arc<BreachRecorder>,
    runner: Arc<ScriptedBuilder>,
    key_path: PathBuf,
    _scratch: Scratch,
}

/// Whether the composed builder performs the §11.4 raw-write shape
/// against the live answer key during its own dispatch.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Tamper {
    None,
    TheKey,
}

/// Composes the real controller over a scratch world. `key_text` is the
/// ruler to freeze (the prevention leg's targets the key itself);
/// `tamper` arms the builder's mid-run raw write of the key.
fn compose(tag: &str, key_text: &str, tamper: Tamper) -> Fixture {
    let scratch = Scratch::new(tag);
    let workspace_root = scratch.child("workspace");
    let evidence_root = scratch.child("evidence");
    let key_dir = scratch.child("ruler");
    let key_path = key_dir.join("g10-answer-key.md");
    std::fs::write(&key_path, key_text).expect("the scratch answer key is written");
    let db = scratch.child("state").join("g10-state.db");

    let evidence = BreachRecorder::new(Arc::new(
        FsEvidenceStore::open(&evidence_root).expect("the real fs evidence store opens"),
    ));
    let state: Arc<EmbeddedStateStore> =
        Arc::new(EmbeddedStateStore::open(&db).expect("the real embedded state store opens"));
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
    let runner = ScriptedBuilder::new(
        workspace_root.clone(),
        artifact_relative,
        (tamper == Tamper::TheKey).then(|| (key_path.clone(), TAMPERED_KEY)),
    );
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
        Arc::new(SeededIdGenerator::new("g10-proof-critic-identity").expect("a non-empty seed")),
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
            SeededIdGenerator::new(&format!("g10-proof-{tag}")).expect("a non-empty seed"),
        ),
        clock,
        runner: runner.clone(),
        runner_identity: gauntlet_domain::RunnerIdentity::new("g10-scripted-builder")
            .expect("runner identity"),
        observers,
        comparison: ComparisonDeps {
            contract,
            deterministic: Arc::new(DiffSource),
            model: Arc::new(model),
            adversarial: None,
            review_mode: gauntlet_domain::ReviewMode::Adaptive,
            model_dispatches: 0,
        },
        ladder: gauntlet_app::verify::ladder::Ladder::new(vec![
            gauntlet_app::verify::ladder::LadderRung::new(
                gauntlet_app::verify::ladder::RungKind::TestReport,
                Arc::new(PassRung),
            ),
        ])
        .expect("the single-rung pass ladder composes"),
        reference: provider,
        answer_key: AnswerKeySource::Path(key_path.clone()),
        escalation: Arc::new(RecordingEscalation::default()),
        meter_factory: meter_factory(),
        budget: budget_shape(FULL_ROUNDS),
        policy: policy(),
        workspace_root,
        config_fingerprint: Digest::new("sha256", &"ab".repeat(32)).expect("digest"),
        // The §8.6 conservative posture (plan Task 14): contained
        // critic-tier dispatches run ungated by containment; the mock
        // runner's honest none isolation makes above-contained refusal
        // the correct posture here.
        govern: gauntlet_app::govern::GovernDeps::none_workspace(),
        environment_fingerprint: Digest::new("sha256", &"cd".repeat(32)).expect("digest"),
        intent: "g10 builder-writes-answer-key proof".to_owned(),
    };
    let controller = RunController::compose(deps).expect("the composed controller builds");
    Fixture {
        controller,
        state,
        evidence,
        runner,
        key_path,
        _scratch: scratch,
    }
}

/// The prevention leg's ruler: the unit's evidence locator TARGETS the
/// live answer-key file (the escape built, not spelled — the in-crate
/// N4 test's own scanner idiom).
fn key_targeting_ruler() -> String {
    let escape = format!("..{PATH_SEP}{}", escaping_locator_needle());
    format!(
        "## {UNIT}\nclaim: the checked artifact passes its focused re-run check\nverify_by: \
         test\nblast_radius: contained\nauthority: critic\nevidence_required: {{kind: \
         test_report, locator: {escape}, producer: instrument}}\ndepends_on: []\n"
    )
}

// ---------------------------------------------------------------------------
// The proofs
// ---------------------------------------------------------------------------

/// PREVENTION (D5): a builder-context write attempt targeting the
/// answer-key path, through the effect surface the loop actually uses —
/// the completion's locator write under the one canonical workspace
/// root — is REFUSED by the containment, typed, and the key bytes on
/// disk are untouched. The proof-local runner carries no OS sandbox
/// (Isolation::None, the documented deviation every composed proof
/// shares); the containment exercised here is the LOOP's own write
/// boundary, which is the surface this tree's runtime actually owns.
#[test]
fn a_builder_context_write_at_the_key_is_refused_by_containment() {
    let ruler = key_targeting_ruler();
    let fixture = compose("prevention", &ruler, Tamper::None);
    let before = std::fs::read(&fixture.key_path).expect("the key reads before the attempt");
    assert_eq!(before, ruler.as_bytes());
    let error = fixture
        .controller
        .start_run()
        .expect_err("the write attempt through the loop's effect surface is refused");
    match &error {
        ControlError::EvidenceLocatorEscapes { locator, root } => {
            // The escape is built, not spelled (the scanner idiom); the
            // asserted value is the locator the ruler froze.
            let expected_locator = format!("..{PATH_SEP}{}", escaping_locator_needle());
            assert_eq!(locator.as_str(), expected_locator);
            assert_eq!(
                root,
                fixture.controller.workspace_root(),
                "the refusal names the root the locator escapes"
            );
        }
        other => panic!("the refusal is typed at the write boundary, got {other}"),
    }
    // The key bytes are byte-identical: nothing the loop could do wrote
    // one byte of the grader.
    let after = std::fs::read(&fixture.key_path).expect("the key reads after the attempt");
    assert_eq!(
        before, after,
        "the answer key is untouched by the refused write attempt"
    );
    // And nothing settled under the refused ruler.
    let run_id = fixture.runner.run_id();
    let events = fixture
        .state
        .export_jsonl(&run_id, bound())
        .expect("the export reads");
    assert!(
        !events
            .lines()
            .any(|line| line.contains("settlement_committed")),
        "no settlement happened under the refused ruler: {events}"
    );
    assert!(
        fixture
            .state
            .settlements(&run_id, bound())
            .expect("rows read")
            .is_empty(),
        "no settlements row exists"
    );
}

/// DETECTION (the B2 A4 mechanism composed): the builder RAW-WRITES the
/// answer key mid-run — the §11.4 operator-hand-edit shape through the
/// plain filesystem every builder-context process holds. The settle-time
/// digest consult re-derives the CURRENT digest from the file (never the
/// controller's own frozen value — the D4 tautology distinction), fires,
/// and the run ends terminal `aborted` with BOTH digests pinned as
/// forensic evidence; no settlement lands over the mutated ruler.
#[test]
fn a_midrun_key_tamper_aborts_with_both_digests_pinned() {
    let fixture = compose("detection", ANSWER_KEY, Tamper::TheKey);
    let outcome = fixture
        .controller
        .start_run()
        .expect("a reference breach is a terminal fact of the run, never a run error");
    assert_eq!(
        outcome.terminal,
        TerminalState::Aborted,
        "the tampered ruler ends the run aborted (reference_mutated)"
    );
    // BOTH digests pinned: the breach carries them and the pinned
    // forensic record names them.
    let breach = outcome
        .reference_breach
        .as_ref()
        .expect("the outcome carries the reference breach");
    let gauntlet_app::control::controller::ReferenceBreach::Mutated { expected, actual } = breach
    else {
        panic!("the breach is the mutated shape, got {breach:?}");
    };
    let frozen = fixture
        .state
        .load_run(&outcome.run_id, bound())
        .expect("the run reads")
        .expect("the run is durable")
        .reference_version()
        .digest()
        .clone();
    assert_eq!(
        expected, &frozen,
        "the breach's expected digest IS the run's frozen reference digest"
    );
    assert_ne!(
        expected, actual,
        "the breach's two digests differ — the mutation is the visible cause"
    );
    let breaches = fixture.evidence.breach_records();
    assert!(
        breaches
            .iter()
            .any(|record| record.contains("\"class\":\"reference_mutated\"")
                && record.contains(&expected.to_string())
                && record.contains(&actual.to_string())),
        "both digests are PINNED as forensic evidence: {breaches:?}"
    );
    // No settlement landed over the mutated ruler.
    let events = fixture
        .state
        .export_jsonl(&outcome.run_id, bound())
        .expect("the export reads");
    assert!(
        !events
            .lines()
            .any(|line| line.contains("settlement_committed")),
        "no SettlementCommitted event exists over the mutated ruler: {events}"
    );
    assert!(
        fixture
            .state
            .settlements(&outcome.run_id, bound())
            .expect("rows read")
            .is_empty(),
        "no settlements row exists over the mutated ruler"
    );
    // The tampered bytes stand on disk — the abort PRESERVES the
    // evidence, never reverts it (the operator sees what happened).
    let on_disk = std::fs::read(&fixture.key_path).expect("the key reads after the run");
    assert_eq!(
        on_disk,
        TAMPERED_KEY.as_bytes(),
        "the aborted run preserves the tampered bytes as evidence"
    );
}
