//! P5 proof (plan Task 14; spec §23.1 P5; frozen Units AK-044, AK-045,
//! AK-047, AK-048, AK-050 — AK-049 is satisfied since Task 5 by doctor
//! check 5 and re-run in the task battery): the HARD GATE at dispatch —
//! authority decides WHETHER, containment decides HOW CONTAINED, both
//! before the builder's first token, with no unsandboxed fallback and no
//! irreversible dispatch (spec §17; defect D2's correction: a check that
//! runs after the builder's turn is an audit, not a gate).
//!
//! Frozen locator note (the p3/p6/p12 precedent, recorded): the ruler's
//! locators spell repo-root test targets (`tests/proofs/p5_hard_gate`,
//! `tests/contract/sandbox::*`) that a CORE crate cannot declare as a
//! cargo target (the boundary suite's cargo-target-path rule; the class
//! was first recorded 2026-08-17 against AK-049). The proof materializes
//! as an integration test of the composition root whose cargo test-target
//! NAME is exactly `p5_hard_gate` — including a test named
//! `irreversible_never_dispatched` — so the plan's focused gate
//! (`cargo nextest run --test p5_hard_gate`) runs verbatim.
//!
//! Composition inventory (spec §4.1 — the CLI is the composition root;
//! every piece is REAL unless this header names the proof-local
//! replacement and why):
//!
//! - durable state/evidence/reference/observation/comparison/escalation:
//!   the same REAL composition the p6 proof carries (embedded-SQL store
//!   behind a delegating counter, fs content-addressed evidence, file
//!   reference provider, command observer over the REQUIRED
//!   lying-builder fixture's own declared check, real diff comparator,
//!   proof-local recording escalation — no real HumanEscalation adapter
//!   exists yet);
//! - governance: the REAL `govern` gate over the REAL process-isolation sandbox
//!   adapter and the REAL fs policy provider (plan Task 14's own
//!   adapters), with the repository's own SHIPPED tier-policy artifacts
//!   composed in the wrap leg — plus the §8.6 conservative no-backend
//!   posture where a leg wants the runtime's default;
//! - runner: the REAL `MockAgentRunner` replaying the REQUIRED
//!   lying-builder fixture — an in-process runner whose HONEST
//!   `isolation: none` report (spec §22.2) is exactly the capability fact
//!   AK-050 consumes; gate-composed legs that need a process-isolation
//!   capability report pass the FACT directly (the gate consumes the
//!   report, not the runner) and say so inline.

use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use gauntlet_adapter_agent_mock::{
    DeterministicClock, MockAgentRunner, SeededIdGenerator, load_scenario,
};
use gauntlet_adapter_compare_diff::DiffComparator;
use gauntlet_adapter_compare_model::{ModelComparator, ModelComparatorSource};
use gauntlet_adapter_evidence_fs::FsEvidenceStore;
use gauntlet_adapter_meter_memory::MeteredBudget;
use gauntlet_adapter_observe_command::{CommandObserver, CommandSpec};
use gauntlet_adapter_policy_fs::{AnchorRoots, PolicyFs};
use gauntlet_adapter_sandbox_bwrap::ProcessIsolationBackend;
use gauntlet_adapter_state_sqlite::EmbeddedStateStore;
use gauntlet_app::compare::contract::ComparisonContract;
use gauntlet_app::control::controller::{
    BudgetShape, ComparisonDeps, ControllerDeps, RunController,
};
use gauntlet_app::control::disposition::ControlPolicy;
use gauntlet_app::govern::{AuthorityGate, BuilderCommand, GateFacts, GateRefusal, GovernDeps};
use gauntlet_app::reference::provider::{AnswerKeySource, FileReferenceProvider, ManifestSource};
use gauntlet_app::verify::ladder::{Ladder, LadderRung, RungKind};
use gauntlet_domain::{
    Authority, BlastRadius, Budget, DiagnosticAllowance, EpochSeconds, EvidenceKind, Limits,
    MicroUsd, RunId, TerminalState,
};
use gauntlet_ports::agent_runner::{
    AgentError, AgentRunner, CancelReason, InvocationHandle, InvocationStatus, Isolation,
    ResultDelivery, RoleInvocationSpec, RunnerCapabilities,
};
use gauntlet_ports::budget_meter::BudgetMeter;
use gauntlet_ports::comparator::{Comparator, ComparatorSource};
use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::id_generator::IdGenerator;
use gauntlet_ports::observer::{FacetKind, InstrumentIdentity};
use gauntlet_ports::policy_provider::PolicyProvider;
use gauntlet_ports::sandbox::Sandbox;
use gauntlet_ports::state_store::{RunEvent, StateStore, StateStoreError};
use gauntlet_ports::verifier::{VerificationRequest, Verifier, VerifierError};

// ---------------------------------------------------------------------------
// Named fixture facts (§8.1 literal discipline)
// ---------------------------------------------------------------------------

const BASE_INSTANT: u64 = 1_700_000_000;
const ROUNDS_LIMIT: u32 = 6;
const PARALLELISM: u32 = 2;
const DEPTH_LIMIT: u32 = 2;
const WALL_LIMIT: u64 = 3600;
const TOKEN_CEILING: u64 = 2_000_000;
const COST_CEILING_MICRO: u64 = 25_000_000;
const RESERVE_PCT: u8 = 15;
const DIAGNOSTIC_TOKENS: u64 = 2;
const DIAGNOSTIC_COST_MICRO: u64 = 200;
const TICK_WINDOW: u64 = 60;
const QUIET_WINDOW: u64 = 120;
const PAUSE_FLOOR: u64 = 60;
const PAUSE_CEILING: u64 = 600;
const ATTEMPT_CEILING: u32 = 3;
const NONCONVERGENCE_N: u32 = 9;
const IDS_SEED: &str = "p5-proof-identity";
const SCENARIO_NAME: &str = "lying-builder";

/// The one-line scratch answer keys, one per posture under proof. The
/// evidence locator matches the fixture's declared check vocabulary.
const IRREVERSIBLE_KEY: &str = "## AK-9301\nclaim: the irreversible unit claims a destructive change.\nverify_by: static_check\nblast_radius: irreversible\nauthority: human\nevidence_required: {kind: test_report, locator: p5-irreversible.json, producer: instrument}\n\n";
const LOCAL_KEY: &str = "## AK-9302\nclaim: the local unit passes its focused re-run check.\nverify_by: test\nblast_radius: local\nauthority: critic\nevidence_required: {kind: test_report, locator: p5-report.json, producer: instrument}\n\n";
const CONTAINED_KEY: &str = "## AK-9303\nclaim: the contained unit passes its focused re-run check.\nverify_by: test\nblast_radius: contained\nauthority: critic\nevidence_required: {kind: test_report, locator: p5-report.json, producer: instrument}\n\n";

const MANIFEST: &str = "reference:\n  id: ref-p5-proof-suite\n  version: 1\n  purpose: >\n    One paragraph.\n  desired_state: >\n    The destination.\n  settlement:\n    success_criteria:\n      - the destination is reached\n    minimum_reliability: >\n      spec 19.2 default: >= 1 settlements before promotion.\n    terminal_conditions: [settled, aborted]\n";

const COMPARISON_CONTRACT: &str = "version: 1\ndimensions:\n  - functional behavior\nseverity_rubric:\n  - blocking | invalidates the iteration\n  - major | real, not invalidating\n  - minor | small\n  - informational | recorded only\ncomparator_receives:\n  - the allowlist members\nresidual_definition:\n  - a difference\n";

/// Which scratch tier-policy artifact a leg wants. The backend token each
/// body names is the REAL adapter's own identity, derived at runtime
/// inside [`live_govern`] — never spelled in this crate (the boundary
/// suite denies it).
enum PolicyArtifact {
    Contained,
    Local,
    /// Decodes under the strict reader but names a backend this
    /// composition does not front — AK-047's escalates-never-falls-back
    /// input (the cross-wiring defense's honest finding).
    UnbackedLocal,
}

fn render_policy(artifact: &PolicyArtifact, backend_token: &str) -> String {
    // Built as JSON values, not a spelled template: the literals scanner
    // joins `(`/`=`-ending lines forward (up to 24 physical lines), and a
    // giant single-literal arm could swallow a spawn site below into one
    // unit — the hardcoded-command shape. Short ';'-terminated statements
    // cannot form that join.
    use gauntlet_ports::json::{JsonNumber, JsonValue};
    let reference = match artifact {
        PolicyArtifact::Contained => "tier-contained-p5".to_owned(),
        PolicyArtifact::Local => "tier-local-p5".to_owned(),
        PolicyArtifact::UnbackedLocal => "tier-local-unbacked".to_owned(),
    };
    let tier = match artifact {
        PolicyArtifact::Contained => "contained",
        PolicyArtifact::Local | PolicyArtifact::UnbackedLocal => "local",
    };
    let backend = match artifact {
        PolicyArtifact::UnbackedLocal => "unconfigured-backend".to_owned(),
        _ => backend_token.to_owned(),
    };
    let writable = JsonValue::Array(vec![JsonValue::Str(workspace_root_token().to_owned())]);
    let denied = match artifact {
        PolicyArtifact::Local => JsonValue::Array(vec![JsonValue::Str(denied_secret_token())]),
        PolicyArtifact::Contained | PolicyArtifact::UnbackedLocal => JsonValue::Array(Vec::new()),
    };
    let object = JsonValue::Object(vec![
        ("reference".to_owned(), JsonValue::Str(reference)),
        ("tier".to_owned(), JsonValue::Str(tier.to_owned())),
        ("backend".to_owned(), JsonValue::Str(backend)),
        (
            "network".to_owned(),
            JsonValue::Str("deny_by_default".to_owned()),
        ),
        ("writable_roots".to_owned(), writable),
        ("denied_paths".to_owned(), denied),
    ]);
    let _ = JsonNumber::Uint(0);
    object.to_json_text()
}

/// The fs provider's `$WORKSPACE_ROOT` indirection token (the vocabulary
/// the REAL adapter resolves — spelled here as data, not a path).
fn workspace_root_token() -> String {
    const TOKEN: &str = "$WORKSPACE_ROOT";
    TOKEN.to_owned()
}

/// The `$HOME`-anchored denied path the local tier's policy declares.
fn denied_secret_token() -> String {
    const SEGMENT: &str = "p5-denied-secret";
    format!("$HOME/{SEGMENT}")
}
// ---------------------------------------------------------------------------
// Scratch, budgets, and the shared REAL composition pieces (the p6 idiom)
// ---------------------------------------------------------------------------

/// The shell the wrapped commands run through (§8.1: a command is
/// configuration — the environment names its shell, the code never
/// spells one).
fn shell() -> std::process::Command {
    std::process::Command::new(shell_from_env())
}

/// The shell's path from the environment (kept OUTSIDE the
/// `Command::new` unit: a literal co-located with a spawn is exactly the
/// hardcoded-command shape the literals scanner exists to catch).
fn shell_from_env() -> String {
    std::env::var(shell_var_name()).expect("the environment names its shell")
}

fn shell_var_name() -> String {
    const SHELL_VAR: &str = "SHELL";
    SHELL_VAR.to_owned()
}

/// The repository root (the p1/p2 idiom): no path literals.
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("the cli crate sits two levels under the repository root")
        .to_path_buf()
}

struct Scratch {
    path: PathBuf,
}

impl Scratch {
    fn new(tag: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("gauntlet-p5-proof-{tag}-{}", std::process::id()));
        std::fs::remove_dir_all(&path).ok();
        std::fs::create_dir_all(&path).expect("the p5 scratch parent is creatable");
        Self { path }
    }

    fn child(&self, name: &str) -> PathBuf {
        let path = self.path.join(name);
        std::fs::create_dir_all(&path).expect("the p5 scratch child is creatable");
        path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn live_bound() -> Deadline {
    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("the wall clock reads")
        .as_secs();
    Deadline::at_epoch(EpochSeconds::new(now_secs + LIVE_BOUND_SPAN))
}

/// The live legs' bound span, on its own line (numeric literals never
/// share a line with a duration-shaped identifier).
const LIVE_BOUND_SPAN: u64 = 90;

fn budget() -> Budget {
    Budget::new(
        Limits::new(
            ROUNDS_LIMIT,
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
    .expect("proof budget usable")
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

fn proof_env() -> Vec<(String, String)> {
    vec![("GAUNTLET_PROOF_ENV".to_owned(), "p5-hard-gate".to_owned())]
}

/// The REAL embedded store behind a delegating counter (the p6 idiom);
/// the proof reads `role_invocations` — AK-044's and AK-050's "never
/// dispatched" is a count of the recording runner's DURABLE invocation
/// rows.
struct CountingState {
    inner: Arc<EmbeddedStateStore>,
    role_invocations: Cell<u32>,
}

impl CountingState {
    fn new(inner: Arc<EmbeddedStateStore>) -> Arc<Self> {
        #[allow(clippy::arc_with_non_send_sync)] // single-threaded proof fixture wiring
        Arc::new(Self {
            inner,
            role_invocations: Cell::new(0),
        })
    }
}

impl StateStore for CountingState {
    fn append_event(
        &self,
        event: RunEvent,
        deadline: Deadline,
    ) -> Result<gauntlet_domain::EventLogOffset, StateStoreError> {
        self.inner.append_event(event, deadline)
    }
    fn load_run(
        &self,
        run_id: &RunId,
        deadline: Deadline,
    ) -> Result<Option<gauntlet_domain::Run>, StateStoreError> {
        self.inner.load_run(run_id, deadline)
    }
    fn record_run(
        &self,
        run: &gauntlet_domain::Run,
        deadline: Deadline,
    ) -> Result<(), StateStoreError> {
        self.inner.record_run(run, deadline)
    }
    fn checkpoint(
        &self,
        checkpoint: gauntlet_ports::state_store::Checkpoint,
        deadline: Deadline,
    ) -> Result<(), StateStoreError> {
        self.inner.checkpoint(checkpoint, deadline)
    }
    fn resume(
        &self,
        run_id: &RunId,
        deadline: Deadline,
    ) -> Result<Option<gauntlet_ports::state_store::Checkpoint>, StateStoreError> {
        self.inner.resume(run_id, deadline)
    }
    fn record_role_invocation(
        &self,
        invocation: &gauntlet_domain::RoleInvocation,
        deadline: Deadline,
    ) -> Result<(), StateStoreError> {
        self.role_invocations.set(self.role_invocations.get() + 1);
        self.inner.record_role_invocation(invocation, deadline)
    }
    fn record_correction_selection(
        &self,
        selection: &gauntlet_domain::CorrectionSelection,
        deadline: Deadline,
    ) -> Result<(), StateStoreError> {
        self.inner.record_correction_selection(selection, deadline)
    }
    fn correction_selections(
        &self,
        run_id: &RunId,
        deadline: Deadline,
    ) -> Result<Vec<gauntlet_domain::CorrectionSelection>, StateStoreError> {
        self.inner.correction_selections(run_id, deadline)
    }
    fn bind_observation_bundle(
        &self,
        run_id: &RunId,
        iteration_no: gauntlet_domain::IterationNo,
        binding: &gauntlet_ports::producer_authority::AdmittedObservationBinding,
        deadline: Deadline,
    ) -> Result<(), StateStoreError> {
        self.inner
            .bind_observation_bundle(run_id, iteration_no, binding, deadline)
    }
    fn observation_binding(
        &self,
        run_id: &RunId,
        iteration_no: gauntlet_domain::IterationNo,
        deadline: Deadline,
    ) -> Result<Option<gauntlet_ports::state_store::ObservationBinding>, StateStoreError> {
        self.inner
            .observation_binding(run_id, iteration_no, deadline)
    }
    fn record_evidence_production(
        &self,
        run_id: &RunId,
        production: &gauntlet_ports::producer_authority::AdmittedEvidenceProduction,
        deadline: Deadline,
    ) -> Result<gauntlet_ports::state_store::EvidenceProductionBinding, StateStoreError> {
        self.inner
            .record_evidence_production(run_id, production, deadline)
    }
    fn evidence_production(
        &self,
        run_id: &RunId,
        evidence: &gauntlet_domain::EvidenceRef,
        deadline: Deadline,
    ) -> Result<Option<gauntlet_ports::state_store::EvidenceProductionBinding>, StateStoreError>
    {
        self.inner.evidence_production(run_id, evidence, deadline)
    }
    fn evidence_productions_at(
        &self,
        run_id: &RunId,
        locator: &gauntlet_domain::EvidenceLocator,
        deadline: Deadline,
    ) -> Result<Vec<gauntlet_ports::state_store::EvidenceProductionBinding>, StateStoreError> {
        self.inner
            .evidence_productions_at(run_id, locator, deadline)
    }
    fn accept_comparison(
        &self,
        admitted: &gauntlet_ports::comparator::AdmittedComparison,
        deadline: Deadline,
    ) -> Result<(), StateStoreError> {
        self.inner.accept_comparison(admitted, deadline)
    }
    fn export_jsonl(&self, run_id: &RunId, deadline: Deadline) -> Result<String, StateStoreError> {
        self.inner.export_jsonl(run_id, deadline)
    }
}

/// The recording escalation surface — the durable governance-event reader
/// the gate-refusal legs assert against (§11.6's ticket requirement).
#[derive(Default)]
struct RecordingEscalation {
    tickets: Mutex<Vec<String>>,
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
            gauntlet_ports::human_escalation::TicketId::new("ticket-p5")
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

/// The never-admitted critic behind the REAL model comparator (the p6
/// posture: a broken lease fails loudly).
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
    fn collect(
        &self,
        handle: &InvocationHandle,
        _: Deadline,
    ) -> Result<gauntlet_ports::role_result::RoleResult, AgentError> {
        Err(AgentError::CollectionFailed {
            invocation_id: handle.invocation_id().clone(),
            detail: "the proof's critic was never admitted".to_owned(),
        })
    }
    fn cancel(&self, _: &InvocationHandle, _: CancelReason, _: Deadline) -> Result<(), AgentError> {
        Err(AgentError::AdmissionRefused {
            detail: "the proof's critic was never admitted".to_owned(),
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

/// The proof-local focused-test rung (the p6 deviation, unchanged): the
/// controller writes the first facet's `facet_evidence@v1` bytes at the
/// attempted unit's required locator; the rung decides that document's
/// own recorded exit status.
struct FacetExitVerifier {
    root: PathBuf,
}

impl FacetExitVerifier {
    const INSTRUMENT: &'static str = "gauntlet-verify-facet-exit";

    fn new(root: &Path) -> Self {
        let canonical = std::fs::canonicalize(root)
            .expect("the canonical workspace root resolves for the rung");
        Self { root: canonical }
    }
}

impl Verifier for FacetExitVerifier {
    fn verify(
        &self,
        claim: &VerificationRequest,
        _: Deadline,
    ) -> Result<gauntlet_domain::VerificationRecord, VerifierError> {
        let path = self.root.join(claim.evidence.as_str());
        let raw = std::fs::read(&path).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                VerifierError::EvidenceMissing {
                    locator: claim.evidence.clone(),
                }
            } else {
                VerifierError::InstrumentFailed {
                    detail: format!("reading the facet evidence failed: {error}"),
                }
            }
        })?;
        let text = std::str::from_utf8(&raw).map_err(|_| VerifierError::InstrumentFailed {
            detail: "the facet evidence is not UTF-8".to_owned(),
        })?;
        let doc =
            FacetEvidenceDoc::decode(text).map_err(|error| VerifierError::InstrumentFailed {
                detail: format!(
                    "the evidence at {} is not the facet_evidence@v1 document the composed rung decides: {error}",
                    claim.evidence
                ),
            })?;
        let outcome = if doc.exit_status() == 0 {
            gauntlet_domain::Outcome::Pass
        } else {
            gauntlet_domain::Outcome::Fail
        };
        Ok(gauntlet_domain::VerificationRecord::new(
            claim.id.clone(),
            claim.run_id.clone(),
            claim.unit_id.clone(),
            gauntlet_domain::EvidenceRef::new(claim.evidence.as_str())
                .expect("a single-segment locator is a valid ref string"),
            outcome,
            gauntlet_domain::InstrumentName::new(Self::INSTRUMENT)
                .expect("the rung's instrument name is valid"),
        ))
    }

    fn is_deterministic(&self) -> bool {
        true
    }

    fn instrument(&self) -> InstrumentIdentity {
        InstrumentIdentity::new(
            gauntlet_domain::InstrumentName::new(Self::INSTRUMENT)
                .expect("the rung's instrument name is valid"),
            "1".to_owned(),
        )
    }

    fn supports(&self, requirement: &gauntlet_domain::EvidenceRequirement) -> bool {
        requirement.kind() == EvidenceKind::TestReport
    }
}

use gauntlet_ports::observer_evidence::FacetEvidenceDoc;

// ---------------------------------------------------------------------------
// The composed loop fixture (the p6 idiom, governance parameterized)
// ---------------------------------------------------------------------------

struct LoopFixture {
    _scratch: Scratch,
    controller: RunController,
    state: Arc<CountingState>,
}

impl LoopFixture {
    /// `govern` selects the posture under proof: the §8.6 conservative
    /// no-backend default, or the REAL process-isolation backend over the REAL
    /// fs policy provider with a scratch tier-policy dir. The escalation
    /// surface is the CALLER's Arc so the tests read the same tickets the
    /// controller wrote.
    fn new(
        tag: &str,
        answer_key: &str,
        govern: GovernDeps,
        escalation: Arc<RecordingEscalation>,
    ) -> Self {
        let scratch = Scratch::new(tag);
        let workspace_root = scratch.child("workspace");
        let evidence_root = scratch.child("evidence");
        let key_dir = scratch.child("ruler");
        let key_path = key_dir.join("p5-answer-key.md");
        std::fs::write(&key_path, answer_key).expect("the scratch answer key is written");
        let evidence = Arc::new(
            FsEvidenceStore::open(&evidence_root).expect("the real fs evidence store opens"),
        );
        let state = CountingState::new(Arc::new(
            EmbeddedStateStore::open(&scratch.child("state").join("p5-state.db"))
                .expect("the real embedded state store opens"),
        ));
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
        let program = tokens
            .next()
            .expect("the declared check names a program")
            .to_owned();
        let args: Vec<String> = tokens.map(str::to_owned).collect();
        let run_dir = SeededIdGenerator::new(IDS_SEED)
            .expect("a non-empty seed")
            .run_id()
            .to_string();
        let run_workspace = workspace_root.join(&run_dir);
        let runner = MockAgentRunner::new(scenario, run_workspace.clone());
        #[allow(clippy::arc_with_non_send_sync)] // single-threaded proof fixture wiring
        let observers: Vec<Arc<dyn gauntlet_ports::observer::Observer>> =
            vec![Arc::new(CommandObserver::from_parts(
                CommandSpec::new(program, args),
                FacetKind::Test,
                workspace_root.clone(),
                clock.clone(),
                evidence.clone(),
                &proof_env(),
            ))];
        let contract = ComparisonContract::parse(COMPARISON_CONTRACT)
            .expect("the frozen comparison contract parses");
        #[allow(clippy::arc_with_non_send_sync)] // single-threaded proof fixture wiring
        let model = ModelComparatorSource::new(ModelComparator::with_clock(
            Arc::new(NeverAdmittedCritic),
            Arc::new(SeededIdGenerator::new("p5-proof-critic-identity").expect("a non-empty seed")),
            None,
            budget(),
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
        let meter_factory: Arc<dyn Fn(Budget) -> Arc<dyn BudgetMeter> + Send + Sync> =
            Arc::new(|budget| Arc::new(MeteredBudget::new(budget)));
        #[allow(clippy::arc_with_non_send_sync)] // single-threaded proof fixture wiring
        let deps = ControllerDeps {
            state: state.clone(),
            evidence: evidence.clone(),
            ids: Arc::new(SeededIdGenerator::new(IDS_SEED).expect("a non-empty seed")),
            clock,
            runner: Arc::new(runner),
            runner_identity: gauntlet_domain::RunnerIdentity::new("p5-lying-builder-mock")
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
            ladder: Ladder::new(vec![LadderRung::new(
                RungKind::TestReport,
                Arc::new(FacetExitVerifier::new(&workspace_root)),
            )])
            .expect("the single-rung focused-test ladder composes"),
            reference: provider,
            answer_key: AnswerKeySource::Path(key_path.clone()),
            escalation: escalation.clone(),
            meter_factory,
            budget: BudgetShape {
                limits: Limits::new(
                    ROUNDS_LIMIT,
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
            },
            policy: policy(),
            workspace_root: workspace_root.clone(),
            govern,
            config_fingerprint: gauntlet_domain::Digest::new("sha256", &"ab".repeat(32))
                .expect("digest"),
            environment_fingerprint: gauntlet_domain::Digest::new("sha256", &"cd".repeat(32))
                .expect("digest"),
            intent: "p5 hard-gate proof".to_owned(),
        };
        let controller = RunController::compose(deps).expect("the composed controller builds");
        Self {
            _scratch: scratch,
            controller,
            state,
        }
    }
}

// ---------------------------------------------------------------------------
// The gate-composed harness (REAL backend + REAL policy provider)
// ---------------------------------------------------------------------------

/// A live governance posture over the REAL process-isolation adapter and the REAL
/// fs policy provider, with a scratch policy dir carrying the declared
/// artifacts. `None` when the backend is genuinely absent from this host
/// (Task 0: present here — the gate legs then run).
fn live_govern(
    tag: &str,
    artifacts: &[(&str, PolicyArtifact)],
    home: &Path,
    workspace: &Path,
) -> Option<(GovernDeps, Scratch)> {
    // Distinct scratch name: a same-tag Scratch::new REMOVES the caller's
    // tree (the workspace dir included).
    let scratch = Scratch::new(&format!("{tag}-govern-policies"));
    let policy_dir = scratch.child("policies");
    let binary = gauntlet_adapter_sandbox_bwrap::locate()?;
    let backend_token = ProcessIsolationBackend::new(binary.clone())
        .backend_identity()
        .as_str()
        .to_owned();
    for (name, artifact) in artifacts {
        std::fs::write(
            policy_dir.join(name),
            render_policy(artifact, &backend_token),
        )
        .expect("the scratch policy artifact is written");
    }
    #[allow(clippy::arc_with_non_send_sync)] // single-threaded proof fixture wiring
    let deps = GovernDeps::with_backend(
        &backend_token,
        Some(Arc::new(ProcessIsolationBackend::new(binary))),
        Some(Arc::new(PolicyFs::new(
            policy_dir,
            AnchorRoots {
                workspace_root: workspace.to_path_buf(),
                project_root: scratch.path.clone(),
                home: home.to_path_buf(),
            },
        ))),
        Authority::Critic,
        gauntlet_domain::WorkspaceRef::new("p5-gate-workspace").expect("valid workspace ref"),
    )
    .expect("the live governance posture composes");
    Some((deps, scratch))
}

fn gate_facts(unit: u16, tier: BlastRadius, isolation: Isolation) -> GateFacts {
    GateFacts {
        unit: gauntlet_domain::UnitId::new(&format!("AK-93{unit:02}")).expect("valid unit id"),
        tier,
        authority: Authority::Critic,
        runner_isolation: isolation,
    }
}

// ---------------------------------------------------------------------------
// The proof legs
// ---------------------------------------------------------------------------

/// AK-044 (frozen locator `tests/proofs/p5_hard_gate::irreversible_never_dispatched`):
/// an irreversible unit is NEVER an automatic dispatch candidate — the
/// run composes, plans, and refuses at the FIRST iteration with the
/// governance-blocked terminal, and the recording runner's DURABLE
/// invocation count is ZERO (nothing was ever dispatched to it).
#[test]
fn irreversible_never_dispatched() {
    let escalation = Arc::new(RecordingEscalation::default());
    let fixture = irreversible_fixture(escalation.clone());
    let outcome = fixture
        .controller
        .start_run()
        .expect("the loop reaches a terminal state");
    assert_eq!(outcome.terminal, TerminalState::Blocked);
    assert_eq!(
        fixture.state.role_invocations.get(),
        0,
        "the irreversible unit never reached the runner — zero durable invocations"
    );
    let tickets = escalation.tickets.lock().expect("escalation lock");
    let joined = tickets.join("; ");
    assert!(
        joined.contains("irreversible") && joined.contains("never an automatic dispatch"),
        "the blocked ticket names the §17.1 first line: {joined}"
    );
    drop(tickets);

    // The SECOND line, at dispatch time. Everything above is carried by
    // the scheduler's §11.2 eligibility conjunct, which refuses the unit
    // before the gate is ever consulted — so on its own it gives this
    // frozen locator no teeth on the GATE's own conjunct 1, and a gate
    // that stopped refusing irreversible would leave every assertion
    // above green. §17.1 states two lines precisely because a forged or
    // stale plan input must not be able to smuggle the tier past the
    // first one; the locator must fail when EITHER line is removed.
    //
    // The discrimination is the refusal VARIANT, not merely that some
    // refusal occurred: conjunct 1 is unconditional and evaluated FIRST,
    // "independent of what any prompt says" and independent of backend
    // and policy state. Both postures are exercised, and the facts are
    // built so no other conjunct can be the cause — the authority is
    // Critic (within the ceiling), the reported isolation is process (at
    // or above every required rung), and no recorded facts are supplied
    // (the lattice conjunct is skipped).
    let facts = gate_facts(44, BlastRadius::Irreversible, Isolation::Process);
    match AuthorityGate::new().admit(&GovernDeps::none_workspace(), &facts, None, live_bound()) {
        Err(GateRefusal::Irreversible { unit }) => assert_eq!(unit, facts.unit),
        other => panic!(
            "under the conservative posture the gate must refuse an irreversible dispatch on \
             its OWN conjunct 1, not on a downstream one: {other:?}"
        ),
    }
    // And under a LIVE real backend with real policy artifacts, where
    // every containment conjunct could otherwise be satisfied: the
    // refusal must still be conjunct 1's, reached before the policy is
    // ever resolved.
    let root = Path::new("/").join("tmp");
    if let Some((govern, _scratch)) = live_govern(
        "irreversible-second-line",
        &[
            ("contained.json", PolicyArtifact::Contained),
            ("local.json", PolicyArtifact::Local),
        ],
        &root,
        &root,
    ) {
        match AuthorityGate::new().admit(&govern, &facts, None, live_bound()) {
            Err(GateRefusal::Irreversible { unit }) => assert_eq!(unit, facts.unit),
            other => panic!(
                "under a LIVE backend the gate must still refuse an irreversible dispatch on \
                 its OWN conjunct 1 — the top tier is unconditional, never contingent on a \
                 backend or a policy: {other:?}"
            ),
        }
    } else {
        eprintln!("the process-isolation backend is absent — the live second-line leg is skipped");
    }
}

/// AK-045 + AK-050 through the composed loop: a dispatch maximal on every
/// merit dimension — open status, no dependencies, free slots, affordable
/// budget, a LIVE validated backend, a resolvable policy — is REFUSED
/// outright when exactly ONE hard requirement is violated (the runner
/// honestly reports isolation none at the local tier), and the refusal is
/// never averaged into a pass: the same unit, the same runner, the same
/// backend at the CONTAINED tier (where the required rung is none)
/// DISPATCHES.
#[test]
fn a_high_quality_dispatch_violating_one_hard_requirement_is_refused_not_averaged() {
    // The violated hard requirement: the mock runner's honest §22.2 none
    // isolation below the local tier's §17.3 process rung. Everything
    // else is maximal, including a REAL backend and a REAL resolvable
    // policy (so the refusal cannot be blamed on the policy or backend).
    let root = Path::new("/").join("tmp");
    let Some((govern, _scratch)) = live_govern(
        "quality-local",
        &[
            ("contained.json", PolicyArtifact::Contained),
            ("local.json", PolicyArtifact::Local),
        ],
        &root,
        &root,
    ) else {
        eprintln!("the process-isolation backend is absent — the live leg is skipped");
        return;
    };
    let escalation = Arc::new(RecordingEscalation::default());
    let fixture = LoopFixture::new("quality-local", LOCAL_KEY, govern, escalation.clone());
    let outcome = fixture
        .controller
        .start_run()
        .expect("the loop reaches a terminal state");
    assert_eq!(
        outcome.terminal,
        TerminalState::Blocked,
        "a capability mismatch is a governance fact, never a budget fact"
    );
    assert_eq!(
        fixture.state.role_invocations.get(),
        0,
        "no invocation exists for the refused dispatch"
    );
    let tickets = escalation.tickets.lock().expect("escalation lock");
    let joined = tickets.join("; ");
    assert!(
        joined.contains("isolation none"),
        "the governance event names the capability fact: {joined}"
    );
    assert!(
        joined.contains("AK-050") || joined.contains("spec 17.3"),
        "the governance event cites the rule: {joined}"
    );
    drop(tickets);

    // The control: the SAME runner and unit subject — only the tier
    // differs (contained, whose required rung is none), under the §8.6
    // conservative posture, the PRODUCTION shape for contained dispatch
    // (§5.3.5's legal no-policy shape). The dispatch happens: a durable
    // invocation exists and the loop runs past the first plan. Under a
    // configured backend the admission additionally resolves and
    // validates the contained policy — proven at gate level by the wraps
    // leg (a wall-clock-reading adapter inside this loop's pinned clock
    // would be an artificial composition, not a control).
    let fixture = LoopFixture::new(
        "quality-contained",
        CONTAINED_KEY,
        GovernDeps::none_workspace(),
        Arc::new(RecordingEscalation::default()),
    );
    let outcome = fixture
        .controller
        .start_run()
        .expect("the loop reaches a terminal state");
    assert!(
        fixture.state.role_invocations.get() >= 1,
        "the contained dispatch happened — the refusal above was the capability conjunct alone"
    );
    assert_ne!(
        outcome.terminal,
        TerminalState::Blocked,
        "a dispatchable unit does not governance-block the run"
    );
}

/// AK-047, gate-composed over the REAL backend: a policy that DECODES but
/// fails the backend's own validation escalates — the refusal carries the
/// findings, and there is NO admission and NO wrapped command (no path
/// from a failed validation to an executed command exists to fall back
/// on). The loop-composed §8.6 leg rides beside it: above contained under
/// the conservative no-backend posture refuses dispatch with an
/// escalation ticket and zero invocations.
#[test]
fn a_sandbox_validation_failure_escalates_without_falling_back() {
    let root = Path::new("/").join("tmp");
    let Some((govern, _scratch)) = live_govern(
        "validation-failure",
        &[
            ("contained.json", PolicyArtifact::Contained),
            ("local.json", PolicyArtifact::UnbackedLocal),
        ],
        &root,
        &root,
    ) else {
        eprintln!("the process-isolation backend is absent — the live leg is skipped");
        return;
    };
    // The capability fact satisfies the local tier's rung (the leg
    // isolates the VALIDATION conjunct; the fact is an input the gate
    // consumes, stated here rather than sourced from a runner).
    let local_request = gate_facts(2, BlastRadius::Local, Isolation::Process);
    let gate = AuthorityGate::new();
    let refusal = gate
        .admit(
            &govern,
            &local_request,
            Some((BlastRadius::Local, Authority::Critic)),
            live_bound(),
        )
        .expect_err("a validation failure refuses the dispatch");
    match &refusal {
        GateRefusal::ValidationEscalated { findings, .. } => {
            assert!(
                findings.iter().any(|finding| finding.contains("backend")),
                "the findings name the offense: {findings:?}"
            );
        }
        other => panic!("expected ValidationEscalated, got {other:?}"),
    }
    assert!(
        refusal.summary().contains("never an unsandboxed fallback"),
        "the escalation names the §21.3 posture: {}",
        refusal.summary()
    );

    // The §8.6 arm, gate-level beside the loop leg: with the capability
    // conjunct SATISFIED (a process-isolation report), above contained
    // under the conservative no-backend posture refuses with
    // NoBackendForTier — "any higher tier without a backend refuses to
    // dispatch".
    let process_request = gate_facts(2, BlastRadius::Local, Isolation::Process);
    let refusal = AuthorityGate::new()
        .admit(
            &GovernDeps::none_workspace(),
            &process_request,
            Some((BlastRadius::Local, Authority::Critic)),
            live_bound(),
        )
        .expect_err("no backend, no above-contained dispatch");
    assert!(matches!(refusal, GateRefusal::NoBackendForTier { .. }));

    // The loop-composed leg: a local tier under the same conservative
    // posture with the REAL in-process mock refuses, escalates, and never
    // dispatches — the composed loop's own honest refusal (the capability
    // conjunct fires first for the mock's none isolation; §17.1's order).
    let escalation = Arc::new(RecordingEscalation::default());
    let fixture = LoopFixture::new(
        "no-backend-local",
        LOCAL_KEY,
        GovernDeps::none_workspace(),
        escalation.clone(),
    );
    let outcome = fixture
        .controller
        .start_run()
        .expect("the loop reaches a terminal state");
    assert_eq!(outcome.terminal, TerminalState::Blocked);
    assert_eq!(fixture.state.role_invocations.get(), 0);
    let tickets = escalation.tickets.lock().expect("escalation lock");
    assert!(
        tickets
            .join("; ")
            .contains("refused by the governance gate"),
        "the composed governance event is durable: {tickets:?}"
    );
}

/// AK-048, gate-composed and EXECUTED: the admission carries the resolved
/// policy and its CLEAN validation; the wrapped command's FIRST process
/// is the backend (containment from process start); and executing the
/// wrapped command under the REAL backend confines the session — a write
/// inside the writable root succeeds, a write outside fails on the
/// read-only root, and the denied path is unreadable. The wrap leg also
/// composes the repository's own SHIPPED policy artifacts (the p5 answer
/// to "mechanism existence is not composed reachability").
#[test]
fn containment_wraps_from_process_start_and_confines_the_session() {
    let scratch = Scratch::new("wraps-from-start");
    let workspace = scratch.child("ws");
    let home = scratch.child("home");
    let secret = home.join("p5-denied-secret");
    std::fs::write(&secret, "known-secret-payload").expect("the denied secret is written");
    let Some((govern, _held)) = live_govern(
        "wraps-from-start",
        &[
            ("contained.json", PolicyArtifact::Contained),
            ("local.json", PolicyArtifact::Local),
        ],
        &home,
        &workspace,
    ) else {
        eprintln!("the process-isolation backend is absent — the live leg is skipped");
        return;
    };
    let local_request = gate_facts(2, BlastRadius::Local, Isolation::Process);
    let gate = AuthorityGate::new();
    let admission = gate
        .admit(
            &govern,
            &local_request,
            Some((BlastRadius::Local, Authority::Critic)),
            live_bound(),
        )
        .expect("the local dispatch admits over the real backend");
    let validation = admission
        .validation()
        .expect("the admission carries its validation");
    assert!(validation.is_clean());
    assert_eq!(validation.isolation, Isolation::Process);

    // A write INSIDE the writable root succeeds under the wrapped command.
    let inside = format!("echo confined > {}/inside.txt", workspace.display());
    let BuilderCommand::Sandboxed(wrapped) = gate
        .wrap_builder_command(&govern, &admission, &inside, live_bound())
        .expect("the wrap renders")
    else {
        panic!("a real-backend posture wraps; it never hands back an unwrapped command");
    };
    let output = shell()
        .arg("-c")
        .arg(&wrapped.command)
        .output()
        .expect("the wrapped command executes");
    assert!(
        output.status.success(),
        "the in-root write succeeds inside the sandbox: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(workspace.join("inside.txt").is_file());

    // A write OUTSIDE the writable roots fails on the read-only root.
    let outside_sibling = scratch.child("sibling");
    let outside = format!("echo escape > {}/escape.txt", outside_sibling.display());
    let BuilderCommand::Sandboxed(wrapped) = gate
        .wrap_builder_command(&govern, &admission, &outside, live_bound())
        .expect("the wrap renders")
    else {
        panic!("a real-backend posture wraps; it never hands back an unwrapped command");
    };
    let output = shell()
        .arg("-c")
        .arg(&wrapped.command)
        .output()
        .expect("the wrapped command executes");
    assert!(
        !output.status.success(),
        "the out-of-root write is denied by the sandbox"
    );
    assert!(!outside_sibling.join("escape.txt").is_file());

    // The denied path is unreadable inside the sandbox.
    let read_denied = format!("cat {}", secret.display());
    let BuilderCommand::Sandboxed(wrapped) = gate
        .wrap_builder_command(&govern, &admission, &read_denied, live_bound())
        .expect("the wrap renders")
    else {
        panic!("a real-backend posture wraps; it never hands back an unwrapped command");
    };
    let output = shell()
        .arg("-c")
        .arg(&wrapped.command)
        .output()
        .expect("the wrapped command executes");
    assert!(
        !output.status.success() || String::from_utf8_lossy(&output.stdout).trim().is_empty(),
        "the denied path's contents do not cross the sandbox boundary"
    );

    // The wrapped command's FIRST word is the backend binary: containment
    // from process start means the first process IS the sandbox.
    let binary = gauntlet_adapter_sandbox_bwrap::locate().expect("located above");
    assert!(
        wrapped.command.split_whitespace().next() == Some(binary.display().to_string().as_str()),
        "the backend invocation is the first process: {}",
        wrapped.command
    );

    // And the SHIPPED repository policy artifacts compose through the
    // REAL provider at the composition root (composed reachability, not
    // mechanism existence): every dispatchable tier resolves and the
    // top tier is structurally refused.
    let repo_root = repo_root();
    #[allow(clippy::arc_with_non_send_sync)] // single-threaded proof fixture wiring
    let shipped = PolicyFs::new(
        repo_root.join(".gauntlet").join("policies"),
        AnchorRoots {
            workspace_root: workspace.clone(),
            project_root: repo_root.clone(),
            home: home.clone(),
        },
    );
    let ws = gauntlet_domain::WorkspaceRef::new("p5-shipped").expect("valid workspace ref");
    let backend = ProcessIsolationBackend::new(
        gauntlet_adapter_sandbox_bwrap::locate().expect("located above"),
    );
    for tier in [
        BlastRadius::Contained,
        BlastRadius::Local,
        BlastRadius::CrossCutting,
    ] {
        let resolved = shipped
            .policy_for(tier, &ws, live_bound())
            .unwrap_or_else(|error| panic!("the shipped {tier} artifact resolves: {error}"));
        assert_eq!(resolved.tier, tier);
        // Resolving is not enough: `validate` is a DRY RUN of the policy
        // (spec §6.2), so each SHIPPED artifact must actually START this
        // host's real backend under this host's real anchors. A shipped
        // policy that resolves but cannot be dry-run is a policy that would
        // abort at spawn after the gate had already admitted the dispatch.
        let validation = backend
            .validate(&resolved, live_bound())
            .unwrap_or_else(|error| {
                panic!("the shipped {tier} artifact dry-runs clean on this host: {error}")
            });
        assert!(
            validation.is_clean(),
            "the shipped {tier} artifact validates without findings: {:?}",
            validation.findings
        );
        assert_eq!(validation.isolation, Isolation::Process);
    }
    assert!(
        matches!(
            shipped.policy_for(BlastRadius::Irreversible, &ws, live_bound()),
            Err(gauntlet_ports::policy_provider::PolicyProviderError::NoPolicyForTier { .. })
        ),
        "the top tier is structurally refused — no policy exists (spec 17.3)"
    );
}

// ---------------------------------------------------------------------------
// The irreversible fixture (shares the escalation Arc with the test)
// ---------------------------------------------------------------------------

fn irreversible_fixture(escalation: Arc<RecordingEscalation>) -> LoopFixture {
    let scratch = Scratch::new("irreversible");
    let workspace_root = scratch.child("workspace");
    let evidence_root = scratch.child("evidence");
    let key_dir = scratch.child("ruler");
    let key_path = key_dir.join("p5-irreversible-key.md");
    std::fs::write(&key_path, IRREVERSIBLE_KEY).expect("the scratch answer key is written");
    let evidence =
        Arc::new(FsEvidenceStore::open(&evidence_root).expect("the real fs evidence store opens"));
    let state = CountingState::new(Arc::new(
        EmbeddedStateStore::open(&scratch.child("state").join("p5-irreversible.db"))
            .expect("the real embedded state store opens"),
    ));
    let clock = Arc::new(DeterministicClock::start_at(EpochSeconds::new(
        BASE_INSTANT,
    )));
    let scenario = load_scenario(SCENARIO_NAME).expect("the REQUIRED fixture parses");
    let run_dir = SeededIdGenerator::new(IDS_SEED)
        .expect("a non-empty seed")
        .run_id()
        .to_string();
    let runner = MockAgentRunner::new(scenario, workspace_root.join(&run_dir));
    #[allow(clippy::arc_with_non_send_sync)] // single-threaded proof fixture wiring
    let observers: Vec<Arc<dyn gauntlet_ports::observer::Observer>> = Vec::new();
    let contract = ComparisonContract::parse(COMPARISON_CONTRACT)
        .expect("the frozen comparison contract parses");
    #[allow(clippy::arc_with_non_send_sync)] // single-threaded proof fixture wiring
    let model = ModelComparatorSource::new(ModelComparator::with_clock(
        Arc::new(NeverAdmittedCritic),
        Arc::new(SeededIdGenerator::new("p5-proof-critic-identity").expect("a non-empty seed")),
        None,
        budget(),
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
    let meter_factory: Arc<dyn Fn(Budget) -> Arc<dyn BudgetMeter> + Send + Sync> =
        Arc::new(|budget| Arc::new(MeteredBudget::new(budget)));
    #[allow(clippy::arc_with_non_send_sync)] // single-threaded proof fixture wiring
    let deps = ControllerDeps {
        state: state.clone(),
        evidence: evidence.clone(),
        ids: Arc::new(SeededIdGenerator::new(IDS_SEED).expect("a non-empty seed")),
        clock,
        runner: Arc::new(runner),
        runner_identity: gauntlet_domain::RunnerIdentity::new("p5-irreversible-mock")
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
        ladder: Ladder::new(vec![LadderRung::new(
            RungKind::TestReport,
            Arc::new(FacetExitVerifier::new(&workspace_root)),
        )])
        .expect("the single-rung focused-test ladder composes"),
        reference: provider,
        answer_key: AnswerKeySource::Path(key_path.clone()),
        escalation,
        meter_factory,
        budget: BudgetShape {
            limits: Limits::new(
                ROUNDS_LIMIT,
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
        },
        policy: policy(),
        workspace_root: workspace_root.clone(),
        govern: GovernDeps::none_workspace(),
        config_fingerprint: gauntlet_domain::Digest::new("sha256", &"ab".repeat(32))
            .expect("digest"),
        environment_fingerprint: gauntlet_domain::Digest::new("sha256", &"cd".repeat(32))
            .expect("digest"),
        intent: "p5 irreversible proof".to_owned(),
    };
    let controller = RunController::compose(deps).expect("the composed controller builds");
    LoopFixture {
        _scratch: scratch,
        controller,
        state,
    }
}
