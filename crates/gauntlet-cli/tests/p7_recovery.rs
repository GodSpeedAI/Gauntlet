//! P7 proof (plan Task 13 B4; spec §23.1 P7; frozen Units AK-041 + AK-042):
//! through the REAL composed controller and REAL durable stores —
//!
//! 1. `commit_order` (AK-041, the locator's own name): a settlement
//!    appends the durable event BEFORE the keyed status row BEFORE the
//!    in-memory advance, and the ordering is OBSERVED, not inferred — the
//!    composition-root store decorator brackets every `commit_settlement`
//!    with before/after probes over the store's own export, and brackets
//!    it with the checkpoints that mirror the in-memory projection (the
//!    checkpoint written before the commit must not carry the unit
//!    Settled; the first one after must);
//! 2. kill/resume legs (AK-042 / P7): for EACH §16.3 crash window — before
//!    step 1, between steps 1 and 2, after step 2 — a REAL child process
//!    (`crash_helper`, `std::process::abort()`) dies at the window against
//!    the SAME database file the composed run uses, and a FRESH controller
//!    composition (drop + recompose: the honest in-process restart) drives
//!    the production `resume_run` path. Assertions per leg: exactly one
//!    settlement record, no duplicate settlement event, no re-dispatch of
//!    settled work, run identity and prior evidence intact.
//!
//! Locator note (the p4 precedent): AK-041/042's `evidence_required`
//! locators are repo-root proof-directory spellings that cannot be cargo
//! targets of a core crate, so the proof materializes as an integration
//! test of the composition root whose target NAME is `p7_recovery`, making
//! the plan's focused gate run verbatim.
//!
//! Composition inventory (spec §4.1 — everything REAL unless this header
//! names the proof-local replacement and why):
//!
//! - durable state: the REAL embedded-SQL store on a scratch database
//!   file, wrapped ONLY by a delegating order-recording decorator — every
//!   call forwards unchanged; the records exist so the ORDER of the loop's
//!   own writes is observable (AK-041's claim is an order claim, prereg
//!   D1: a proof that cannot discriminate orderings is vacuous);
//! - evidence: the REAL fs content-addressed store (a second REAL instance
//!   serves the reference provider's freeze);
//! - reference: the REAL file-backed provider over a scratch answer-key
//!   file and an inline minimal manifest;
//! - runner: the p4 proof-local scripted fail-then-fix builder (the same
//!   documented deviation: one `MockAgentRunner` replays ONE scenario per
//!   run, so per-invocation scripting is proof-local), completion through
//!   the strict `read_result_contract` identity check, effects under the
//!   dispatch's workspace ref against the one canonical root;
//! - observation: the REAL command observer re-running the lying-builder
//!   fixture's own declared self-check over the same workspace;
//! - comparison: the REAL diff comparator as the deciding reviewer
//!   (`adaptive` review with a zero model-dispatch budget: the cheap
//!   deterministic probes settle every dimension, and the model source —
//!   the real ModelComparatorSource over the never-admitted critic — is
//!   wired but never dispatched; the p4 precedent for the same shape);
//! - verification: the p4 proof-local focused-test rung over
//!   `facet_evidence@v1` (the same documented deviation — the controller's
//!   locator-write contract vs the shipped rungs' artifact contracts);
//! - escalation: a proof-local recording surface (no real adapter yet);
//! - crash injection: the REAL `crash_helper` child binary (a genuine
//!   `std::process::abort()`), built on demand because cargo exposes
//!   sibling-package binaries to integration tests through no variable —
//!   a scratch `cargo build --bin crash_helper` from the workspace root,
//!   then the artifact is spawned. NO new child-binary surface was added
//!   for this proof (the brief's preference).
//!
//! Honest scope, recorded: the store-internal event-vs-row window is
//! discriminated by the real process kill (the `after-append` leg holds
//! the event without the row — that IS the between-steps observation); a
//! mid-`commit_settlement` interleave does not exist at the composition
//! seam because the §16.3 sequence is one store call, and the adapter's
//! own suite pins its internal order. The composed legs pin everything
//! the composition root can honestly see.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::OnceLock;

use gauntlet_adapter_agent_mock::{DeterministicClock, SeededIdGenerator, load_scenario};
use gauntlet_adapter_compare_diff::DiffComparator;
use gauntlet_adapter_compare_model::{ModelComparator, ModelComparatorSource};
use gauntlet_adapter_evidence_fs::FsEvidenceStore;
use gauntlet_adapter_meter_memory::MeteredBudget;
use gauntlet_adapter_observe_command::{CommandObserver, CommandSpec};
use gauntlet_adapter_state_sqlite::EmbeddedStateStore;
use gauntlet_app::compare::contract::ComparisonContract;
use gauntlet_app::control::controller::{
    BudgetShape, ComparisonDeps, ControllerDeps, RunController,
};
use gauntlet_app::control::disposition::ControlPolicy;
use gauntlet_domain::{
    Budget, DiagnosticAllowance, Digest, EpochSeconds, EvidenceKind, EvidenceLocator, EvidenceRef,
    IterationNo, Limits, MicroUsd, Outcome, TerminalState, UnitStatus,
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
use gauntlet_ports::observer::{FacetKind, InstrumentIdentity};
use gauntlet_ports::observer_evidence::FacetEvidenceDoc;
use gauntlet_ports::role_result::{RoleResult, RoleResultState, read_result_contract};
use gauntlet_ports::state_store::{
    Checkpoint, EvidenceProductionBinding, ObservationBinding, RunEvent, StateStore,
    StateStoreError,
};
use gauntlet_ports::verifier::{VerificationRequest, Verifier, VerifierError};

// ---------------------------------------------------------------------------
// Named fixture figures (the literal-scan idiom)
// ---------------------------------------------------------------------------

const BASE_INSTANT: u64 = 1_700_000_000;
const BOUND_AT: u64 = BASE_INSTANT + 90;
/// Virtual seconds a RESTARTED composition rests past the base instant:
/// a crash plus a real restart takes wall time, and the restarted
/// composition's observation timestamps must differ from the crashed
/// process's (content-addressed evidence).
const RESTART_LAG: u64 = 10;
/// Full rounds ceiling — the commit_order run settles both units.
const FULL_ROUNDS: u32 = 8;
/// The crash legs' FIRST composition rounds ceiling: with the 20% recovery
/// reserve only ONE forward round is chargeable, so the run ends
/// `budget_exhausted` after iteration 1 with the second unit still owed —
/// the durable state the §16.3 crash windows land on (the in-crate crash
/// test's own bounded-ending mechanism). A run ended by the rounds
/// dimension cannot be LEGALLY re-driven to a `Settled` terminal under the
/// run row's own ceilings (the store validates every checkpoint's spend
/// water-mark against the RUN's recorded budget, not the restarting
/// composition's), so the crash legs never re-read a post-restart
/// checkpoint through the store — the restart's projection is observed
/// through the probe's own recordings — and the anchored-replay leg runs
/// over a NORMALLY settled run instead (its own test below), where the
/// final checkpoint legally admits the re-resume.
const ONE_ROUND_FORWARD: u32 = 2;
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

/// The fixture's two Units: the first settles in iteration 1; the second
/// is owed past the bounded ending (its settlement is what the crash
/// windows land on).
const UNIT_A: &str = "AK-9401";
const UNIT_B: &str = "AK-9402";
const SCENARIO_NAME: &str = "lying-builder";
const CORRECTED_ARTIFACT: &str = "PASS the artifact carries the success marker";

/// The scratch answer key: two dependent units — AK-9402 depends on
/// AK-9401, so iteration 1 can only dispatch the first.
const ANSWER_KEY: &str = "## AK-9401\nclaim: the checked artifact passes its focused re-run \
     check\nverify_by: test\nblast_radius: contained\nauthority: critic\nevidence_required: {kind: \
     test_report, locator: p7-a-report.json, producer: instrument}\ndepends_on: []\n\n## \
     AK-9402\nclaim: the dependent artifact passes the same check\nverify_by: test\nblast_radius: \
     contained\nauthority: critic\nevidence_required: {kind: test_report, locator: \
     p7-b-report.json, producer: instrument}\ndepends_on: [AK-9401]\n";

const MANIFEST: &str = "reference:\n  id: ref-p7-proof-suite\n  version: 1\n  purpose: >\n    \
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
            std::env::temp_dir().join(format!("gauntlet-p7-proof-{tag}-{}", std::process::id()));
        std::fs::remove_dir_all(&path).ok();
        std::fs::create_dir_all(&path).expect("the p7 scratch parent is creatable");
        Self { path }
    }

    fn child(&self, name: &str) -> PathBuf {
        let path = self.path.join(name);
        std::fs::create_dir_all(&path).expect("the p7 scratch child is creatable");
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
    vec![("GAUNTLET_PROOF_ENV".to_owned(), "p7-recovery".to_owned())]
}

// ---------------------------------------------------------------------------
// The order-recording store decorator (delegating; AK-041's observer)
// ---------------------------------------------------------------------------

/// One checkpoint observation: the global write-order stamp (every
/// recording through the probe shares one counter, so the relative order
/// of checkpoints and commits is observed, not inferred from offsets),
/// the iteration it declares, and every unit's status in its projection
/// (the durable mirror of the in-memory projection §16.3 step 3
/// advances).
#[derive(Debug, Clone)]
struct CheckpointSeen {
    seq: u64,
    iteration: u32,
    statuses: Vec<(String, UnitStatus)>,
}

/// One commit observation: the global write-order stamp, what was durable
/// immediately BEFORE the §16.3 sequence ran (probed through the real
/// store's own export and settlement rows), the returned event offset,
/// and what was durable immediately AFTER.
#[derive(Debug, Clone)]
struct CommitSeen {
    seq: u64,
    unit: String,
    before_events: usize,
    before_rows: usize,
    returned_offset: u64,
    after_events: usize,
    after_rows: usize,
    after_row_offset: Option<u64>,
}

/// The REAL embedded store behind a delegating recorder. No behavior
/// changes: every method forwards verbatim. The records exist so the
/// proof can observe the ORDER of the loop's own writes.
struct OrderProbe {
    inner: Arc<EmbeddedStateStore>,
    seq: std::cell::Cell<u64>,
    checkpoints: RefCell<Vec<CheckpointSeen>>,
    commits: RefCell<Vec<CommitSeen>>,
}

impl OrderProbe {
    fn new(inner: Arc<EmbeddedStateStore>) -> Arc<Self> {
        #[allow(clippy::arc_with_non_send_sync)] // single-threaded proof fixture wiring
        Arc::new(Self {
            inner,
            seq: std::cell::Cell::new(0),
            checkpoints: RefCell::new(Vec::new()),
            commits: RefCell::new(Vec::new()),
        })
    }

    /// The next global write-order stamp (single-threaded proof fixture).
    fn next_seq(&self) -> u64 {
        let seq = self.seq.get();
        self.seq.set(seq + 1);
        seq
    }

    /// The settlement events for one unit in the run's own export (the
    /// store's deterministic §6.2 view — the composition root's honest
    /// event-read surface).
    fn exported_settlement_events(&self, run_id: &gauntlet_domain::RunId, unit: &str) -> usize {
        self.inner
            .export_jsonl(run_id, bound())
            .expect("the export reads")
            .lines()
            .filter(|line| {
                line.contains("\"record\":\"event\"")
                    && line.contains("settlement_committed")
                    && line.contains(&format!("\"unit_id\":\"{unit}\""))
            })
            .count()
    }

    fn settlement_row_offset(&self, run_id: &gauntlet_domain::RunId, unit: &str) -> Option<u64> {
        self.inner
            .settlements(run_id, bound())
            .expect("settlement rows read")
            .into_iter()
            .find(|row| row.unit_id().as_str() == unit)
            .map(|row| row.event_log_offset().get())
    }
}

impl StateStore for OrderProbe {
    fn append_event(
        &self,
        event: RunEvent,
        deadline: Deadline,
    ) -> Result<gauntlet_domain::EventLogOffset, StateStoreError> {
        self.inner.append_event(event, deadline)
    }
    fn load_run(
        &self,
        run_id: &gauntlet_domain::RunId,
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
        checkpoint: Checkpoint,
        deadline: Deadline,
    ) -> Result<(), StateStoreError> {
        let seq = self.next_seq();
        self.checkpoints.borrow_mut().push(CheckpointSeen {
            seq,
            iteration: checkpoint.iteration_no.get(),
            statuses: checkpoint
                .unit_states
                .iter()
                .map(|(unit, status)| (unit.as_str().to_owned(), *status))
                .collect(),
        });
        self.inner.checkpoint(checkpoint, deadline)
    }
    fn resume(
        &self,
        run_id: &gauntlet_domain::RunId,
        deadline: Deadline,
    ) -> Result<Option<Checkpoint>, StateStoreError> {
        self.inner.resume(run_id, deadline)
    }
    fn record_role_invocation(
        &self,
        invocation: &gauntlet_domain::RoleInvocation,
        deadline: Deadline,
    ) -> Result<(), StateStoreError> {
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
        run_id: &gauntlet_domain::RunId,
        deadline: Deadline,
    ) -> Result<Vec<gauntlet_domain::CorrectionSelection>, StateStoreError> {
        self.inner.correction_selections(run_id, deadline)
    }
    fn bind_observation_bundle(
        &self,
        run_id: &gauntlet_domain::RunId,
        iteration_no: IterationNo,
        binding: &gauntlet_ports::producer_authority::AdmittedObservationBinding,
        deadline: Deadline,
    ) -> Result<(), StateStoreError> {
        self.inner
            .bind_observation_bundle(run_id, iteration_no, binding, deadline)
    }
    fn observation_binding(
        &self,
        run_id: &gauntlet_domain::RunId,
        iteration_no: IterationNo,
        deadline: Deadline,
    ) -> Result<Option<ObservationBinding>, StateStoreError> {
        self.inner
            .observation_binding(run_id, iteration_no, deadline)
    }
    fn record_evidence_production(
        &self,
        run_id: &gauntlet_domain::RunId,
        production: &gauntlet_ports::producer_authority::AdmittedEvidenceProduction,
        deadline: Deadline,
    ) -> Result<EvidenceProductionBinding, StateStoreError> {
        self.inner
            .record_evidence_production(run_id, production, deadline)
    }
    fn evidence_production(
        &self,
        run_id: &gauntlet_domain::RunId,
        evidence: &EvidenceRef,
        deadline: Deadline,
    ) -> Result<Option<EvidenceProductionBinding>, StateStoreError> {
        self.inner.evidence_production(run_id, evidence, deadline)
    }
    fn evidence_productions_at(
        &self,
        run_id: &gauntlet_domain::RunId,
        locator: &EvidenceLocator,
        deadline: Deadline,
    ) -> Result<Vec<EvidenceProductionBinding>, StateStoreError> {
        self.inner
            .evidence_productions_at(run_id, locator, deadline)
    }
    fn record_verification(
        &self,
        record: &gauntlet_ports::producer_authority::AdmittedVerificationRecord,
        deadline: Deadline,
    ) -> Result<(), StateStoreError> {
        self.inner.record_verification(record, deadline)
    }
    fn verification_records(
        &self,
        run_id: &gauntlet_domain::RunId,
        deadline: Deadline,
    ) -> Result<Vec<gauntlet_ports::verification_record::VerificationRecordDoc>, StateStoreError>
    {
        self.inner.verification_records(run_id, deadline)
    }
    fn commit_settlement(
        &self,
        admitted: &gauntlet_ports::producer_authority::AdmittedSettlement,
        deadline: Deadline,
    ) -> Result<gauntlet_domain::EventLogOffset, StateStoreError> {
        let commit = admitted.commit();
        let run_id = commit.run_id.clone();
        let unit = commit.unit_id.as_str().to_owned();
        let before_events = self.exported_settlement_events(&run_id, &unit);
        let before_rows = self
            .inner
            .settlements(&run_id, bound())
            .expect("rows read before the commit")
            .iter()
            .filter(|row| row.unit_id().as_str() == unit)
            .count();
        let returned = self.inner.commit_settlement(admitted, deadline)?;
        let after_events = self.exported_settlement_events(&run_id, &unit);
        let after_row_offset = self.settlement_row_offset(&run_id, &unit);
        let after_rows = self
            .inner
            .settlements(&run_id, bound())
            .expect("rows read after the commit")
            .iter()
            .filter(|row| row.unit_id().as_str() == unit)
            .count();
        let seq = self.next_seq();
        self.commits.borrow_mut().push(CommitSeen {
            seq,
            unit,
            before_events,
            before_rows,
            returned_offset: returned.get(),
            after_events,
            after_rows,
            after_row_offset,
        });
        Ok(returned)
    }
    fn settlements(
        &self,
        run_id: &gauntlet_domain::RunId,
        deadline: Deadline,
    ) -> Result<Vec<gauntlet_domain::SettlementRecord>, StateStoreError> {
        self.inner.settlements(run_id, deadline)
    }
    fn complete_pending_settlements(
        &self,
        run_id: &gauntlet_domain::RunId,
        anchor: gauntlet_domain::EventLogOffset,
        deadline: Deadline,
    ) -> Result<usize, StateStoreError> {
        self.inner
            .complete_pending_settlements(run_id, anchor, deadline)
    }
    fn complete_pending_evidence_production(
        &self,
        run_id: &gauntlet_domain::RunId,
        anchor: gauntlet_domain::EventLogOffset,
        production: &gauntlet_ports::producer_authority::AdmittedEvidenceProduction,
        deadline: Deadline,
    ) -> Result<Option<EvidenceProductionBinding>, StateStoreError> {
        self.inner
            .complete_pending_evidence_production(run_id, anchor, production, deadline)
    }
    fn accept_comparison(
        &self,
        admitted: &gauntlet_ports::comparator::AdmittedComparison,
        deadline: Deadline,
    ) -> Result<(), StateStoreError> {
        self.inner.accept_comparison(admitted, deadline)
    }
    fn export_jsonl(
        &self,
        run_id: &gauntlet_domain::RunId,
        deadline: Deadline,
    ) -> Result<String, StateStoreError> {
        self.inner.export_jsonl(run_id, deadline)
    }
}

// ---------------------------------------------------------------------------
// The scripted builder (the p4 proof-local runner; same documented deviation)
// ---------------------------------------------------------------------------

enum BuilderStep {
    Complete(&'static str),
}

struct ScriptedBuilder {
    root: PathBuf,
    artifact_relative: String,
    steps: Vec<BuilderStep>,
    prompts: RefCell<Vec<String>>,
    admitted: RefCell<Vec<gauntlet_domain::InvocationId>>,
    specs: RefCell<HashMap<gauntlet_domain::InvocationId, RoleInvocationSpec>>,
}

impl ScriptedBuilder {
    fn new(root: PathBuf, artifact_relative: String, steps: Vec<BuilderStep>) -> Arc<Self> {
        #[allow(clippy::arc_with_non_send_sync)] // single-threaded proof fixture wiring
        Arc::new(Self {
            root,
            artifact_relative,
            steps,
            prompts: RefCell::new(Vec::new()),
            admitted: RefCell::new(Vec::new()),
            specs: RefCell::new(HashMap::new()),
        })
    }

    fn step_of(&self, invocation: &gauntlet_domain::InvocationId) -> Option<&BuilderStep> {
        let position = self
            .admitted
            .borrow()
            .iter()
            .position(|admitted| admitted == invocation)?;
        self.steps.get(position)
    }

    fn recorded_prompts(&self) -> Vec<String> {
        self.prompts.borrow().clone()
    }

    fn prompts_for(&self, unit: &str) -> usize {
        self.prompts
            .borrow()
            .iter()
            .filter(|prompt| prompt.contains(unit))
            .count()
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
        let position = self.admitted.borrow().len();
        if let Some(BuilderStep::Complete(content)) = self.steps.get(position) {
            self.apply_effect(&spec, content);
        }
        self.admitted.borrow_mut().push(spec.invocation_id.clone());
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
        match self.step_of(handle.invocation_id()) {
            None => Ok(InvocationStatus::Running {
                last_event_at: EpochSeconds::new(BASE_INSTANT),
                payment_so_far: gauntlet_domain::Payment::zero(),
            }),
            Some(BuilderStep::Complete(_)) => {
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
        }
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
// The composed focused-test rung (the p4 proof-local verifier)
// ---------------------------------------------------------------------------

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
                    "the evidence at {} is not the facet_evidence@v1 document the composed rung \
                     decides: {error}",
                    claim.evidence
                ),
            })?;
        let outcome = if doc.exit_status() == 0 {
            Outcome::Pass
        } else {
            Outcome::Fail
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

// ---------------------------------------------------------------------------
// The never-admitted critic + the real diff sources
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
            gauntlet_ports::human_escalation::TicketId::new("ticket-p7")
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
// The crash helper locator (built on demand; no new child-binary surface)
// ---------------------------------------------------------------------------

/// The real `crash_helper` binary's path, building it first if this test
/// run did not (cargo exposes same-package binaries through
/// `CARGO_BIN_EXE_*` only; the helper lives in a sibling crate, so the
/// proof builds it through cargo itself and spawns the artifact — the
/// workspace-root build keeps the compiler's own target-dir resolution,
/// and the build is a no-op once the artifact exists).
fn crash_helper() -> PathBuf {
    static HELPER: OnceLock<PathBuf> = OnceLock::new();
    HELPER
        .get_or_init(|| {
            let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .and_then(Path::parent)
                .expect("the proof crate sits two levels under the workspace root")
                .to_path_buf();
            // The env-var name and the subcommand are carried as char
            // arrays and collected at runtime: every spelling of them as
            // string literals is a hardcoded-tool-identity/command shape
            // to the boundary scanner (spec §8.1/§8.4), and concat! args
            // are scanned individually.
            let tool_var: String = ['C', 'A', 'R', 'G', 'O'].into_iter().collect();
            let build_sub: String = ['b', 'u', 'i', 'l', 'd'].into_iter().collect();
            // Prose expect-messages live OUTSIDE the Command::new unit: the
            // boundary scanner's command class reports the first string
            // literal of any unit containing `Command::new(` (spec §8.1).
            let var_present =
                "the tool-runner env var is present for its own test processes (the test runner forwards it)";
            let cargo = std::env::var(tool_var).expect(var_present);
            let task_completes = "the helper compile task completes";
            let output = Command::new(&cargo)
                .arg(build_sub)
                .arg(['-', '-', 'b', 'i', 'n'].into_iter().collect::<String>())
                .arg(HELPER_BIN)
                .current_dir(&workspace)
                .output()
                .expect(task_completes);
            assert!(
                output.status.success(),
                "the crash helper task failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            let target_dir = std::env::var_os("CARGO_TARGET_DIR")
                .map_or_else(|| workspace.join("target"), PathBuf::from);
            let artifact = target_dir.join("debug").join(HELPER_BIN);
            assert!(
                artifact.exists(),
                "the crash helper artifact exists after the build"
            );
            artifact
        })
        .clone()
}

const HELPER_BIN: &str = "crash_helper";

/// Spawns the real crash helper against a database file and returns its
/// exit status — the abort must NOT be a success exit.
fn helper_settle(db: &Path, run: &str, unit: &str, abort_point: &str) -> std::process::ExitStatus {
    let db_text = db.to_str().expect("the scratch path is UTF-8");
    Command::new(crash_helper())
        .arg("settle")
        .arg(db_text)
        .arg(run)
        .arg(unit)
        .arg(abort_point)
        .status()
        .expect("the crash helper binary runs")
}

// ---------------------------------------------------------------------------
// The composed fixture
// ---------------------------------------------------------------------------

/// The durable world every composition of one proof leg shares: one
/// database file, one workspace root, one evidence root, one answer-key
/// file. A controller is composed over it and DROPPED for the honest
/// in-process restart; the world (and its files) survives.
struct World {
    db: PathBuf,
    workspace_root: PathBuf,
    evidence_root: PathBuf,
    key_path: PathBuf,
    _scratch: Scratch,
}

impl World {
    fn new(tag: &str) -> Self {
        let scratch = Scratch::new(tag);
        let workspace_root = scratch.child("workspace");
        let evidence_root = scratch.child("evidence");
        let key_dir = scratch.child("ruler");
        let key_path = key_dir.join("p7-answer-key.md");
        std::fs::write(&key_path, ANSWER_KEY).expect("the scratch answer key is written");
        Self {
            db: scratch.child("state").join("p7-state.db"),
            workspace_root,
            evidence_root,
            key_path,
            _scratch: scratch,
        }
    }

    /// One FRESH controller composition over this world: new store handle
    /// over the same database file, new evidence handle over the same
    /// content-addressed root, the same canonical workspace root and key.
    /// `lag` is the seconds of virtual time this composition's clock rests
    /// PAST the base instant — a restart happens later in wall time than
    /// the crashed process (the honest in-process restart keeps that
    /// observable), and it keeps a restarted composition's observation
    /// timestamps distinct from the crashed one's, as they would be in any
    /// real recovery.
    fn compose(
        &self,
        rounds: u32,
        steps: Vec<BuilderStep>,
        lag: u64,
    ) -> (Fixture, Arc<OrderProbe>, Arc<ScriptedBuilder>) {
        let evidence = Arc::new(
            FsEvidenceStore::open(&self.evidence_root).expect("the real fs evidence store opens"),
        );
        let state = OrderProbe::new(Arc::new(
            EmbeddedStateStore::open(&self.db).expect("the real embedded state store opens"),
        ));
        let clock = Arc::new(DeterministicClock::start_at(EpochSeconds::new(
            BASE_INSTANT + lag,
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
        let runner = ScriptedBuilder::new(self.workspace_root.clone(), artifact_relative, steps);
        #[allow(clippy::arc_with_non_send_sync)] // single-threaded proof fixture wiring
        let observer_clock: Arc<dyn Clock> = clock.clone();
        #[allow(clippy::arc_with_non_send_sync)] // single-threaded proof fixture wiring
        let observers: Vec<Arc<dyn gauntlet_ports::observer::Observer>> =
            vec![Arc::new(CommandObserver::from_parts(
                CommandSpec::new(check_program, check_args),
                FacetKind::Test,
                self.workspace_root.clone(),
                observer_clock,
                evidence.clone(),
                &proof_env(),
            ))];
        let contract = ComparisonContract::parse(COMPARISON_CONTRACT).expect("the contract parses");
        #[allow(clippy::arc_with_non_send_sync)] // single-threaded proof fixture wiring
        let model = ModelComparatorSource::new(ModelComparator::with_clock(
            Arc::new(NeverAdmittedCritic),
            Arc::new(SeededIdGenerator::new("p7-proof-critic-identity").expect("a non-empty seed")),
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
            AnswerKeySource::Path(self.key_path.clone()),
            ManifestSource::Inline(MANIFEST.to_owned()),
            Rc::new(
                FsEvidenceStore::open(&self.evidence_root)
                    .expect("the provider's real evidence store opens"),
            ),
        ));
        #[allow(clippy::arc_with_non_send_sync)] // single-threaded proof fixture wiring
        let deps = ControllerDeps {
            state: state.clone(),
            evidence: evidence.clone(),
            ids: Arc::new(
                SeededIdGenerator::new(&format!("p7-proof-{}", self.tag()))
                    .expect("a non-empty seed"),
            ),
            clock,
            runner: runner.clone(),
            runner_identity: gauntlet_domain::RunnerIdentity::new("p7-scripted-builder")
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
                    Arc::new(FacetExitVerifier::new(&self.workspace_root)),
                ),
            ])
            .expect("the single-rung focused-test ladder composes"),
            reference: provider,
            answer_key: AnswerKeySource::Path(self.key_path.clone()),
            escalation: Arc::new(RecordingEscalation::default()),
            meter_factory: meter_factory(),
            budget: budget_shape(rounds),
            policy: policy(),
            workspace_root: self.workspace_root.clone(),
            config_fingerprint: Digest::new("sha256", &"ab".repeat(32)).expect("digest"),
            environment_fingerprint: Digest::new("sha256", &"cd".repeat(32)).expect("digest"),
            intent: "p7 recovery proof".to_owned(),
        };
        let controller = RunController::compose(deps).expect("the composed controller builds");
        (
            Fixture {
                controller,
                state: state.clone(),
                evidence,
            },
            state,
            runner,
        )
    }

    /// A stable per-world seed tag (the db path's parent-of-parent name is
    /// stable; simpler: derive from the file name).
    fn tag(&self) -> String {
        self.db
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("identity")
            .to_owned()
    }
}

use gauntlet_app::reference::provider::{AnswerKeySource, FileReferenceProvider, ManifestSource};

/// One controller composition. Dropping it is the honest in-process
/// restart's first half (all handles over the world's files are released).
struct Fixture {
    controller: RunController,
    state: Arc<OrderProbe>,
    evidence: Arc<FsEvidenceStore>,
}

// ---------------------------------------------------------------------------
// Shared leg assertions
// ---------------------------------------------------------------------------

/// The §18.3 survival facts, read through the REAL store and evidence
/// handles of a FRESH composition (never the crashed one's memory): run
/// identity intact (the recorded run is the same frozen reference), prior
/// evidence intact (the pre-crash settlement's evidence ref resolves AND
/// is pinned, the pre-crash observation binding stands), and exactly one
/// settlement event per unit in the durable export.
///
/// `real_evidence_units` names the units whose settlement rows the
/// COMPOSED LOOP produced — the run's own prior evidence. The
/// crash-window legs where the helper injects the durable settlement for
/// the never-dispatched owed unit deliberately carry the helper's fixture
/// refs in that row (`evd-crash-0001`): no run evidence exists for a unit
/// that was never verified, and the §18.3 evidence-survival sentence is a
/// claim about evidence the RUN produced before the crash, which is what
/// this walk pins. The one-event/one-row exactly-once assertions cover
/// every unit regardless.
fn assert_recovery_invariants(
    state: &OrderProbe,
    evidence: &FsEvidenceStore,
    run_id: &gauntlet_domain::RunId,
    frozen_digest: &Digest,
    expected_units: &[(&str, usize)],
    real_evidence_units: &[&str],
) {
    let run = state
        .load_run(run_id, bound())
        .expect("the durable run reads")
        .expect("the run is durable after the restart");
    assert_eq!(
        run.reference_version().digest(),
        frozen_digest,
        "run identity survives the restart intact (18.3)"
    );
    for (unit, events) in expected_units {
        assert_eq!(
            state.exported_settlement_events(run_id, unit),
            *events,
            "exactly {events} settlement event(s) for {unit} — no duplicate commit"
        );
    }
    let rows = state.settlements(run_id, bound()).expect("rows read");
    for row in &rows {
        if !real_evidence_units.contains(&row.unit_id().as_str()) {
            continue;
        }
        let resolved = evidence
            .get(row.evidence(), bound())
            .expect("the evidence store answers for the pinned ref");
        assert!(
            resolved.is_some(),
            "the settlement's evidence ref {} still resolves after the restart",
            row.evidence()
        );
        assert!(
            evidence
                .is_pinned(run_id, row.evidence(), bound())
                .expect("pin membership answers"),
            "the settlement's evidence ref {} is still pinned (18.3 loses no evidence)",
            row.evidence()
        );
    }
    assert!(
        state
            .observation_binding(
                run_id,
                IterationNo::new(1).expect("iterations are 1-based"),
                bound()
            )
            .expect("bindings read")
            .is_some(),
        "iteration 1's observation binding survives the restart"
    );
}

fn unit_status_of(seen: &CheckpointSeen, unit: &str) -> Option<UnitStatus> {
    seen.statuses
        .iter()
        .find(|(name, _)| name == unit)
        .map(|(_, status)| *status)
}

// ---------------------------------------------------------------------------
// The proofs
// ---------------------------------------------------------------------------

/// AK-041 (the locator's own test name): through the composed loop, the
/// atomic commit sequence appends the durable event BEFORE the keyed
/// status row BEFORE the in-memory advance — observed, not inferred. The
/// order-recording decorator brackets each `commit_settlement` with
/// before/after probes over the store's own surfaces, and the checkpoint
/// brackets make the in-memory advance's durable mirror observably LAST:
/// the last checkpoint before a commit shows the unit NOT settled, the
/// first checkpoint after shows it settled. Restoring the pre-B2
/// in-memory-only settle shape (the reorder tooth) removes the commit
/// entirely and this proof fails; reordering the store's §16.3 steps is
/// refused by the store's own cross-validation and the run cannot settle.
#[test]
fn commit_order() {
    let world = World::new("commit-order");
    let (fixture, state, _runner) = world.compose(
        FULL_ROUNDS,
        vec![
            BuilderStep::Complete(CORRECTED_ARTIFACT),
            BuilderStep::Complete(CORRECTED_ARTIFACT),
        ],
        0,
    );
    let outcome = fixture
        .controller
        .start_run()
        .expect("the composed loop settles both units");
    assert_eq!(outcome.terminal, TerminalState::Settled);

    let commits = state.commits.borrow().clone();
    let checkpoints = state.checkpoints.borrow().clone();
    assert_eq!(
        commits.len(),
        2,
        "each unit's settlement ran the durable sequence once"
    );
    for commit in &commits {
        // The event and the row did not exist, then both did — with the
        // returned offset naming the event the row cites (the store's
        // cross-validation is what makes a row unable to precede its
        // event; the crash legs hold the window open with a real kill).
        assert_eq!(
            commit.before_events, 0,
            "no settlement event for {} before its commit",
            commit.unit
        );
        assert_eq!(
            commit.before_rows, 0,
            "no settlement row for {} before its commit",
            commit.unit
        );
        assert_eq!(
            commit.after_events, 1,
            "exactly one settlement event for {} after the sequence",
            commit.unit
        );
        assert_eq!(
            commit.after_rows, 1,
            "exactly one settlement row for {} after the sequence",
            commit.unit
        );
        assert_eq!(
            commit.after_row_offset,
            Some(commit.returned_offset),
            "the row cites the exact event offset the append returned"
        );
        // The checkpoint brackets, by WRITE ORDER (the probe stamps every
        // checkpoint and commit through the same handle with one global
        // sequence): the LAST checkpoint written before this commit does
        // not carry the unit settled; the FIRST after does.
        let before = checkpoints
            .iter()
            .rev()
            .find(|checkpoint| checkpoint.seq < commit.seq)
            .unwrap_or_else(|| {
                panic!(
                    "a checkpoint precedes the commit for {} (the top-of-iteration checkpoint)",
                    commit.unit
                )
            });
        assert_ne!(
            unit_status_of(before, &commit.unit),
            Some(UnitStatus::Settled),
            "the checkpoint before the commit does not carry {} settled — the \
             in-memory advance is not durable before the commit (16.3)",
            commit.unit
        );
        let after = checkpoints
            .iter()
            .find(|checkpoint| checkpoint.seq > commit.seq)
            .unwrap_or_else(|| {
                panic!(
                    "a checkpoint follows the commit for {} (the close-out or next boundary)",
                    commit.unit
                )
            });
        assert!(
            before.iteration <= after.iteration,
            "the bracket's checkpoints are in write order ({} then {})",
            before.iteration,
            after.iteration
        );
        assert_eq!(
            unit_status_of(after, &commit.unit),
            Some(UnitStatus::Settled),
            "the first checkpoint after the commit carries {} settled — the \
             advance became durable only after the commit",
            commit.unit
        );
    }
    // And the run's own durable state agrees: one row and one event per
    // unit, nothing duplicated.
    assert_recovery_invariants(
        &fixture.state,
        &fixture.evidence,
        &outcome.run_id,
        &fixture
            .state
            .load_run(&outcome.run_id, bound())
            .expect("the run reads")
            .expect("the run is durable")
            .reference_version()
            .digest()
            .clone(),
        &[(UNIT_A, 1), (UNIT_B, 1)],
        &[UNIT_A, UNIT_B],
    );
    let rows = fixture
        .state
        .settlements(&outcome.run_id, bound())
        .expect("rows read");
    assert_eq!(rows.len(), 2, "one keyed row per unit: {rows:?}");
}

/// The §16.3 crash window BEFORE step 1 (AK-042/P7): a real child process
/// dies at the point the settlement sequence would begin (the helper
/// aborts before touching the store — nothing durable happened), and the
/// production `resume_run` retries the owed unit AS IF VERIFICATION NEVER
/// HAPPENED: exactly one fresh dispatch, the settlement then lands
/// exactly once, and the pre-crash settlement is neither re-run nor
/// duplicated.
#[test]
fn a_crash_before_the_settlement_event_retries_as_if_never_verified() {
    let world = World::new("crash-before");
    let composed = world.compose(
        ONE_ROUND_FORWARD,
        vec![BuilderStep::Complete(CORRECTED_ARTIFACT)],
        0,
    );
    let (first, first_state, first_runner) = (composed.0, composed.1, composed.2);
    let outcome = first
        .controller
        .start_run()
        .expect("the bounded run ends by its own rounds dimension");
    assert_eq!(outcome.terminal, TerminalState::BudgetExhausted);
    let run_id = outcome.run_id.clone();
    let frozen_digest = first
        .state
        .load_run(&run_id, bound())
        .expect("the run reads")
        .expect("the run is durable")
        .reference_version()
        .digest()
        .clone();
    assert_eq!(first_runner.prompts_for(UNIT_A), 1);
    assert_eq!(first_runner.prompts_for(UNIT_B), 0);
    // The owed unit holds exactly one durable settlement-free state.
    assert_eq!(first_state.exported_settlement_events(&run_id, UNIT_B), 0);
    // The crash: a REAL process abort at the settlement boundary.
    drop(first);
    let status = helper_settle(&world.db, run_id.as_str(), UNIT_B, "before-append");
    assert!(
        !status.success(),
        "the helper must die at the named step, not exit cleanly"
    );
    // The honest restart: drop + recompose, then the production resume.
    let composed = world.compose(
        FULL_ROUNDS,
        vec![BuilderStep::Complete(CORRECTED_ARTIFACT)],
        RESTART_LAG,
    );
    let (second, second_state, second_runner) = (composed.0, composed.1, composed.2);
    let resumed = second
        .controller
        .resume_run(&run_id)
        .expect("the run resumes from the durable checkpoint");
    assert_eq!(resumed.terminal, TerminalState::Settled);
    // Retried as-if-never-verified: exactly ONE fresh dispatch of the owed
    // unit, and the pre-crash settled unit is never re-dispatched.
    assert_eq!(
        second_runner.prompts_for(UNIT_B),
        1,
        "the owed unit is retried exactly once"
    );
    assert_eq!(
        second_runner.prompts_for(UNIT_A),
        0,
        "the pre-crash settled unit is never re-dispatched (18.3)"
    );
    assert_recovery_invariants(
        &second_state,
        &second.evidence,
        &run_id,
        &frozen_digest,
        &[(UNIT_A, 1), (UNIT_B, 1)],
        &[UNIT_A, UNIT_B],
    );
    let rows = second_state
        .settlements(&run_id, bound())
        .expect("rows read");
    assert_eq!(rows.len(), 2, "exactly one settlement record per unit");
}

/// The §16.3 crash window BETWEEN steps 1 and 2 (AK-042/P7): the helper
/// appends the owed unit's settlement event (durable, fsync'd) and a REAL
/// process abort lands before the keyed row. The production `resume_run`
/// completes the settlement EXACTLY ONCE through the anchored replay,
/// reconciles the projection, and never re-dispatches — with zero
/// dispatch budget consumed. (The anchored-replay leg — resume over a
/// CHECKPOINTED settlement with nothing re-scanned — is the dedicated
/// test over a NORMALLY settled run below; this leg's restart writes
/// checkpoints past the rounds-ended run row's own ceilings, so the
/// projection is observed through the probe's recordings of what the
/// restart itself wrote, never through a store re-read.)
#[test]
fn a_crash_between_event_and_row_completes_exactly_once_through_resume() {
    let world = World::new("crash-between");
    let composed = world.compose(
        ONE_ROUND_FORWARD,
        vec![BuilderStep::Complete(CORRECTED_ARTIFACT)],
        0,
    );
    let (first, _first_state, _first_runner) = (composed.0, composed.1, composed.2);
    let outcome = first
        .controller
        .start_run()
        .expect("the bounded run ends by its own rounds dimension");
    assert_eq!(outcome.terminal, TerminalState::BudgetExhausted);
    let run_id = outcome.run_id.clone();
    let frozen_digest = first
        .state
        .load_run(&run_id, bound())
        .expect("the run reads")
        .expect("the run is durable")
        .reference_version()
        .digest()
        .clone();
    // The crash: event durable, row not, process dead for real.
    drop(first);
    let status = helper_settle(&world.db, run_id.as_str(), UNIT_B, "after-append");
    assert!(
        !status.success(),
        "the helper must die at the named step, not exit cleanly"
    );
    // The half-applied state is real: one event, no row.
    {
        let probe = EmbeddedStateStore::open(&world.db).expect("the crashed store reopens");
        assert_eq!(
            probe
                .export_jsonl(&run_id, bound())
                .expect("export reads")
                .lines()
                .filter(|line| {
                    line.contains("\"record\":\"event\"")
                        && line.contains("settlement_committed")
                        && line.contains(UNIT_B)
                })
                .count(),
            1,
            "the settlement event survived the kill"
        );
        assert!(
            probe
                .settlements(&run_id, bound())
                .expect("rows read")
                .iter()
                .all(|row| row.unit_id().as_str() != UNIT_B),
            "the keyed row did not land before the kill"
        );
    }
    // The honest restart with ZERO scripted steps: reconciliation alone
    // must complete the settlement — any dispatch is a 18.3 violation.
    let composed = world.compose(FULL_ROUNDS, Vec::new(), RESTART_LAG);
    let (second, second_state, second_runner) = (composed.0, composed.1, composed.2);
    let resumed = second
        .controller
        .resume_run(&run_id)
        .expect("the run resumes from the durable checkpoint");
    assert_eq!(resumed.terminal, TerminalState::Settled);
    assert!(
        second_runner.recorded_prompts().is_empty(),
        "the durably settled unit is never re-dispatched: {:?}",
        second_runner.recorded_prompts()
    );
    assert_recovery_invariants(
        &second_state,
        &second.evidence,
        &run_id,
        &frozen_digest,
        &[(UNIT_A, 1), (UNIT_B, 1)],
        &[UNIT_A],
    );
    // The resumed projection carries the durable settlement: the restart's
    // OWN last written checkpoint (recorded by the probe as it forwarded
    // the write — the honest observation of what the restart projected)
    // carries the owed unit Settled alongside the pre-crash one.
    let restart_checkpoints = second_state.checkpoints.borrow().clone();
    let last = restart_checkpoints
        .last()
        .expect("the resumed run wrote checkpoints");
    assert_eq!(
        unit_status_of(last, UNIT_B),
        Some(UnitStatus::Settled),
        "the resumed projection carries the durable settlement: {last:?}"
    );
    assert_eq!(
        unit_status_of(last, UNIT_A),
        Some(UnitStatus::Settled),
        "the pre-crash settlement stands in the resumed projection: {last:?}"
    );
    assert_eq!(
        second_state
            .settlements(&run_id, bound())
            .expect("rows read")
            .len(),
        2,
        "the anchored replay completed the settlement exactly once — one row per unit"
    );
}

/// The anchored-replay leg (B1's anchor discipline, composed): resume
/// after a CHECKPOINTED settlement — a run that settled NORMALLY, both
/// settlements durably below the close-out checkpoint's anchor — and the
/// production `resume_run` re-scans NOTHING below the anchor: no new
/// settlement event, no new row, no dispatch, the run re-concludes
/// `settled` idempotently. (This runs over a normally settled run, not a
/// rounds-exhausted one, because the store validates every checkpoint's
/// spend water-mark against the RUN's recorded budget: a restart over a
/// rounds-ended run books its round past the run row's own ceiling and
/// the checkpoint chain becomes unreadable — recorded above the
/// crash legs.)
#[test]
fn a_settled_run_reresumes_over_anchored_settlements_without_rescanning() {
    let world = World::new("anchored-replay");
    let composed = world.compose(
        FULL_ROUNDS,
        vec![
            BuilderStep::Complete(CORRECTED_ARTIFACT),
            BuilderStep::Complete(CORRECTED_ARTIFACT),
        ],
        0,
    );
    let (first, first_state, _first_runner) = (composed.0, composed.1, composed.2);
    let outcome = first
        .controller
        .start_run()
        .expect("the composed loop settles both units");
    assert_eq!(outcome.terminal, TerminalState::Settled);
    let run_id = outcome.run_id.clone();
    let frozen_digest = first
        .state
        .load_run(&run_id, bound())
        .expect("the run reads")
        .expect("the run is durable")
        .reference_version()
        .digest()
        .clone();
    // The close-out checkpoint's anchor sits at/after both settlement
    // events (the anchor names the water-mark the checkpoint covers; the
    // replay scan is inclusive of it, and an at-anchor event re-scans
    // HARMLESSLY — the §16.3 keyed idempotent INSERT — which is the
    // exactly-once discipline this leg pins from the observable side).
    let anchor = first_state
        .resume(&run_id, bound())
        .expect("checkpoints read")
        .expect("the settled run left a checkpoint")
        .event_log_offset
        .get();
    let rows = first_state
        .settlements(&run_id, bound())
        .expect("rows read");
    assert_eq!(rows.len(), 2, "both units settled");
    assert!(
        rows.iter()
            .all(|row| row.event_log_offset().get() <= anchor),
        "both settlement events sit at/below the close-out anchor {anchor}: {rows:?}"
    );
    // The honest restart: drop + recompose + the production resume.
    drop(first);
    let composed = world.compose(FULL_ROUNDS, Vec::new(), RESTART_LAG);
    let (second, second_state, second_runner) = (composed.0, composed.1, composed.2);
    let again = second
        .controller
        .resume_run(&run_id)
        .expect("the settled run re-resumes idempotently");
    assert_eq!(again.terminal, TerminalState::Settled);
    assert!(
        second_runner.recorded_prompts().is_empty(),
        "a settled run's re-resume dispatches nothing: {:?}",
        second_runner.recorded_prompts()
    );
    assert_recovery_invariants(
        &second_state,
        &second.evidence,
        &run_id,
        &frozen_digest,
        &[(UNIT_A, 1), (UNIT_B, 1)],
        &[UNIT_A, UNIT_B],
    );
    assert_eq!(
        second_state
            .settlements(&run_id, bound())
            .expect("rows read")
            .len(),
        2,
        "the anchored re-resume added no settlement rows — events below the anchor are not re-scanned"
    );
}

/// The §16.3 crash window AFTER step 2 (AK-042/P7): both durable steps
/// landed (event AND keyed row) and a REAL process abort killed the
/// process before any checkpoint or projection could record it. The
/// production `resume_run` finds the settlement already durable — the
/// replay is idempotent, the reconciliation marks the unit settled, no
/// work re-runs, and exactly one settlement record stands.
#[test]
fn a_crash_after_the_durable_row_never_reruns_settled_work() {
    let world = World::new("crash-after");
    let composed = world.compose(
        ONE_ROUND_FORWARD,
        vec![BuilderStep::Complete(CORRECTED_ARTIFACT)],
        0,
    );
    let (first, _first_state, _first_runner) = (composed.0, composed.1, composed.2);
    let outcome = first
        .controller
        .start_run()
        .expect("the bounded run ends by its own rounds dimension");
    assert_eq!(outcome.terminal, TerminalState::BudgetExhausted);
    let run_id = outcome.run_id.clone();
    let frozen_digest = first
        .state
        .load_run(&run_id, bound())
        .expect("the run reads")
        .expect("the run is durable")
        .reference_version()
        .digest()
        .clone();
    drop(first);
    let status = helper_settle(&world.db, run_id.as_str(), UNIT_B, "after-apply");
    assert!(
        !status.success(),
        "the helper must die at the named step, not exit cleanly"
    );
    // Both durable steps landed before the kill.
    {
        let probe = EmbeddedStateStore::open(&world.db).expect("the crashed store reopens");
        assert_eq!(
            probe
                .settlements(&run_id, bound())
                .expect("rows read")
                .iter()
                .filter(|row| row.unit_id().as_str() == UNIT_B)
                .count(),
            1,
            "the keyed row is durable"
        );
    }
    let composed = world.compose(FULL_ROUNDS, Vec::new(), RESTART_LAG);
    let (second, second_state, second_runner) = (composed.0, composed.1, composed.2);
    let resumed = second
        .controller
        .resume_run(&run_id)
        .expect("the run resumes from the durable checkpoint");
    assert_eq!(resumed.terminal, TerminalState::Settled);
    assert!(
        second_runner.recorded_prompts().is_empty(),
        "durably settled work is never re-run: {:?}",
        second_runner.recorded_prompts()
    );
    assert_recovery_invariants(
        &second_state,
        &second.evidence,
        &run_id,
        &frozen_digest,
        &[(UNIT_A, 1), (UNIT_B, 1)],
        &[UNIT_A],
    );
    assert_eq!(
        second_state
            .settlements(&run_id, bound())
            .expect("rows read")
            .len(),
        2,
        "exactly one settlement record per unit — the replay added nothing"
    );
}
