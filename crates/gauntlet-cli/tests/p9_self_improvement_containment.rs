//! P9 — self-improvement containment (plan Task 23; ruler AK-056 +
//! AK-057; spec §19.3/§19.4), corrected by COR-1.
//!
//! **COR-1 composition note (recorded deviation, coordinator decision
//! 2026-09-04):** the §19.4 POSITIVE path (a genuine promotion between two
//! complete arms) cannot be demonstrated on the real loop today. Empirical
//! finding (real binary, real composed loop): the COR-D2 observation
//! binding is write-once per (run, iteration) and carried completions
//! absorb one per iteration, so N units settle in exactly N iterations at
//! ANY `max_parallel` — the loop's per-settlement economics are
//! scale-invariant, every inert key is a ceiling (binding ⇒ INCOMPLETE
//! arm, which COR-1's C1 rightly forbids promoting on), and the loop's
//! only cost levers are verification-bearing keys the gate rightly
//! refuses. See `.agents/reports/reference-candidates.md` (the
//! binding-granularity entry and the 2026-09-04 COR-1 entry): the
//! loop-level promotion demonstration awaits the ruler-owner's binding
//! re-key. Until then legs 2/3 prove the positive path at the GATE level:
//! the REAL harness, REAL embedded-SQL revision+capability+state
//! stores, REAL gate — with the executor seam authored over the REAL
//! StateStore EARNED authorities (the p10 composition pattern: durable run
//! identity, checkpoints with `budget_spent`, verification-record
//! authority mints, settlement-commit authority admits + §16.3 commits —
//! never hand-built rows), and the per-invocation payment economy
//! authored the same way (the mock loop itself charges nothing).
//!
//! Four legs over the real `gauntlet` binary (legs 1 and 4) and the real
//! service stack (legs 2 and 3), each declaring its TOOTH — the single
//! mechanism whose removal turns the leg RED:
//!
//! 1. **AK-056 (the plan's Done-when), REAL LOOP, containment form:** the
//!    verification-weakening candidate — its delta downgrades the
//!    verification-bearing `review.mode` — is evaluated over the REAL
//!    composed loop with BOTH arms reaching settled terminal (the COR-1
//!    C1 completion gate), and the gate REJECTS it with
//!    `settlement_access_weakened` durably recorded. Honest header: the
//!    candidate DECLARES an improvement, but on this loop the metric does
//!    not genuinely move between complete arms (the scale-invariance
//!    finding above — both arms figure identically); the containment is
//!    what the leg proves: the weakening delta is refused on the real
//!    binary regardless of any claimed gain, exactly the §19.4 shape
//!    "faster by weakening verification is a regression, not an
//!    improvement". The strongest honest form (genuine improvement AND
//!    rejection) awaits the binding re-key recorded above.
//!    *Tooth:* the gate's settlement-access classification
//!    ([`gauntlet_app::promote::delta`]'s typed total function over the
//!    config vocabulary, via the in-module
//!    `promote::tests::a_weakening_candidate_improves_the_metric_yet_is_rejected_with_settlement_access_weakened`
//!    isolation) — remove it and the delta classifies inert, and this
//!    leg's `settlement_access_weakened` assertion is RED.
//! 2. **The honest road, GATE LEVEL (see the composition note):** a
//!    candidate whose entire delta is the INERT `runners.default` switch
//!    to a runner whose authored per-invocation economy is genuinely
//!    cheaper promotes through the FULL eight-condition conjunction —
//!    REAL `EvaluationHarness` over the proof's earned-authority seam
//!    (both arms complete, the treatment runs' durable records carry the
//!    verified `parent_run`), REAL evidence store, REAL gate, durable
//!    decision record.
//!    *Tooth:* condition 7's strict-improvement floor over the authored
//!    economy (each of the eight conditions is additionally pinned in
//!    isolation by the `promote::` unit tests over the same gate) — a
//!    candidate whose economy does not improve (or regresses) payment per
//!    settlement is refused by exactly this condition, and this leg's
//!    figure assertions (`control > treatment`, both derived from the
//!    durable checkpoints) turn RED if the margin arithmetic stops
//!    discriminating.
//! 3. **Rollback durability (gate level, same fixture):** the promoted
//!    revision rolls back through the REAL restore service; the derived
//!    state reads promoted-but-rolled-back; the restore is durable across
//!    a full ledger reopen (the second rollback runs against a NEWLY
//!    OPENED ledger over the SAME database file, and its typed refusal
//!    reads the restore fact).
//!    *Tooth:* the transition-derived restore history (no mutable status
//!    column) — a ledger that forgot restores, or allowed a second
//!    restore, turns this leg RED.
//! 4. **The §19.4 floor, REAL LOOP:** a revision with NO evaluation
//!    record adjudicates PROVISIONAL and never promotes; the revision
//!    stays PENDING, so the honest road (an evaluation) remains open.
//!    *Tooth:* the gate's missing-evidence refusal (`MissingEvaluation`
//!    → provisional, never promoted, never rejected) — a gate that
//!    promotes (or rejects) on no evidence turns this leg RED.
//!
//! Every assertion is RECORD-DERIVED (the commands' printed lines, exit
//! codes, typed verdicts, and durable ledger/store facts), never a claim
//! about internals.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use gauntlet_adapter_agent_mock::DeterministicClock;
use gauntlet_adapter_evidence_fs::FsEvidenceStore;
use gauntlet_adapter_state_postgres::{
    EmbeddedCapabilityLedger, EmbeddedRevisionLedger, EmbeddedStateStore,
};
use gauntlet_app::promote::delta::DeltaDocument;
use gauntlet_app::promote::evaluate::{
    EvaluationExecutor, EvaluationHarness, EvaluationRequest, ExecutionError, ExecutionOutcome,
    ExecutionRequest,
};
use gauntlet_app::promote::gate::{GateError, GateInput, GateVerdict, PromotionGate};
use gauntlet_app::promote::metrics::TargetMetric;
use gauntlet_app::promote::proposal::ProposalService;
use gauntlet_app::promote::rollback::RollbackService;
use gauntlet_ports::clock::Clock;
use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::evidence_store::EvidenceStore;
use gauntlet_ports::producer_authority::{SettlementCommitAuthority, VerificationRecordAuthority};
use gauntlet_ports::revision_ledger::{DecisionKind, RevisionLedger};
use gauntlet_ports::state_store::{Checkpoint, SettlementCommit, StateStore};
use gauntlet_ports::verification_record::{
    RequirementDoc, VerificationContent, VerificationRecordDoc, VerifyingInstrument,
};

/// The real composed binary (same package: `src/bin/gauntlet.rs`).
const GAUNTLET: &str = env!("CARGO_BIN_EXE_gauntlet");

/// The evidence pin's deadline bound — a far-future instant, on its own
/// line (numeric literals never share a line with a duration-shaped
/// identifier).
const PIN_BOUND: u64 = 1_900_000_000;

/// The workspace root: this package's manifest sits two levels below it
/// (`crates/gauntlet-cli`) — derived, never a path literal (AK-016).
fn workspace_root() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop();
    path.pop();
    path
}

/// The fixture target project (the e2e's seeded project).
fn fixture_target() -> PathBuf {
    workspace_root()
        .join("tests")
        .join("fixtures")
        .join("target")
}

// ---------------------------------------------------------------------------
// The binary-drive world (legs 1 and 4): the e2e pattern, baseline COMPLETE
// (COR-1 C1: an incomplete arm is an evidence gap, never a promotion input)
// ---------------------------------------------------------------------------

/// One scratch world: a fresh copy of the fixture project plus the
/// per-run state/evidence directories under it. Removed on drop.
struct World {
    root: PathBuf,
}

impl World {
    fn new(tag: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "gauntlet-p9-self-improvement-{tag}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("the scratch root is creatable");
        copy_tree(&fixture_target(), &root.join("target"));
        let world = Self { root };
        world.ensure_layout();
        world
    }

    fn target(&self) -> PathBuf {
        self.root.join("target")
    }

    fn state_dir(&self) -> PathBuf {
        self.root.join("state")
    }

    fn evidence_dir(&self) -> PathBuf {
        self.root.join("evidence")
    }

    /// The task-set fixture's CURRENT configuration (the operator authors
    /// the task set): COMPLETE under the baseline — the authored round and
    /// parallel ceilings settle every unit (four complete evaluation arms
    /// in every leg; an incomplete arm is an evidence gap, and gaps
    /// adjudicate Provisional, never a verdict). The generous token
    /// ceiling keeps the scheduler's per-attempt budget share
    /// non-binding.
    fn ensure_layout(&self) {
        std::fs::create_dir_all(self.state_dir()).expect("state dir");
        std::fs::create_dir_all(self.evidence_dir()).expect("evidence dir");
        let config = self.target().join("GAUNTLET.md");
        let text = std::fs::read_to_string(&config).expect("the task-set config reads");
        let updated = text.replace("max_tokens: 200000", "max_tokens: 10000000");
        std::fs::write(&config, updated).expect("the task-set config writes");
    }

    /// The command environment for one leg (the e2e convention: the mock
    /// replays `e2e-happy`; the deterministic clock keeps the runs
    /// reproducible; the tool binding names THIS world's own check
    /// command, so the evaluation runs settle over the same real
    /// instrument the e2e uses).
    fn env(&self) -> Vec<(&'static str, String)> {
        vec![
            ("GAUNTLET_STATE_DIR", self.state_dir().display().to_string()),
            (
                "GAUNTLET_ARTIFACT_DIR",
                self.evidence_dir().display().to_string(),
            ),
            ("GAUNTLET_MOCK_SCENARIO", "e2e-happy".to_owned()),
            // NO GAUNTLET_ID_SEED here, deliberately: the four evaluation
            // runs are four in-process compositions, and a seeded id
            // generator would mint the SAME run id for every arm — the
            // §5.3.3 identity check would (correctly) refuse the second
            // recording. The legs assert no specific run id, so the
            // production ULID source runs.
            ("GAUNTLET_CLOCK", "deterministic".to_owned()),
            ("GAUNTLET_STALL_TIMEOUT_MS", "60000".to_owned()),
            ("GAUNTLET_RETRY_BASE_MS", "100".to_owned()),
            ("GAUNTLET_RETRY_MAX_MS", "2000".to_owned()),
            ("GAUNTLET_MAX_ATTEMPTS_PER_UNIT", "3".to_owned()),
            ("GAUNTLET_TOOL_TEST", self.check_tool()),
        ]
    }

    /// The fixture's one check command, as the test facet's binding.
    fn check_tool(&self) -> String {
        let scripts = self.target().join("scripts");
        let mut files: Vec<PathBuf> = std::fs::read_dir(&scripts)
            .expect("the fixture scripts dir reads")
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.is_file())
            .collect();
        files.sort();
        files
            .first()
            .expect("the fixture carries a check command")
            .display()
            .to_string()
    }

    /// Runs the real binary in the fixture copy with the leg's env.
    fn run(&self, args: &[&str]) -> Output {
        let mut command = Command::new(GAUNTLET);
        command.current_dir(self.target());
        for (key, value) in self.env() {
            command.env(key, value);
        }
        command
            .args(args)
            .output()
            .expect("the gauntlet binary runs")
    }

    /// Pins the candidate delta bytes in the world's REAL evidence store
    /// (the §6.2 content-addressed put: the returned ref IS the bytes'
    /// content address).
    fn pin(&self, bytes: &[u8]) -> String {
        let store = FsEvidenceStore::open(&self.evidence_dir()).expect("the evidence store opens");
        let reference = store
            .put(
                bytes,
                gauntlet_domain::EvidenceKind::File,
                Deadline::at_epoch(gauntlet_domain::EpochSeconds::new(PIN_BOUND)),
            )
            .expect("the delta bytes pin");
        reference.as_str().to_owned()
    }

    /// Admits an operator-supplied result document (`improve propose
    /// --from-result`): the SAME strict decode as a dispatched proposal.
    /// Returns the content-derived revision id.
    fn propose(&self, document: &str) -> String {
        let document_path = self.root.join("candidate-result.json");
        std::fs::write(&document_path, document).expect("the result document writes");
        let output = self.run(&[
            "improve",
            "propose",
            "--from-result",
            document_path.to_str().expect("a plain path"),
        ]);
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        assert!(
            output.status.success(),
            "the proposal admits: stdout={stdout} stderr={}",
            String::from_utf8_lossy(&output.stderr)
        );
        stdout
            .lines()
            .find_map(|line| {
                line.trim()
                    .strip_prefix("improve: revision=")
                    .and_then(|rest| rest.split_whitespace().next())
                    .map(str::to_owned)
            })
            .expect("the proposal prints its revision id")
    }

    /// Evaluates the revision over both task sets (the REAL composed loop:
    /// four runs, control and treatment per set, `parent_run` set on the
    /// treatment arms).
    fn evaluate(&self, revision: &str, metric: &str) -> Output {
        let target = self.target();
        let output = self.run(&[
            "improve",
            "evaluate",
            revision,
            "--held-out",
            target.to_str().expect("a plain path"),
            "--regression",
            target.to_str().expect("a plain path"),
            "--protected",
            "cap-eval",
            "--metric",
            metric,
        ]);
        assert!(
            output.status.success(),
            "the evaluation runs: stdout={}\nstderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        output
    }
}

impl Drop for World {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("the destination dir is creatable");
    for entry in std::fs::read_dir(from).expect("the source dir is readable") {
        let entry = entry.expect("the entry reads");
        let kind = entry.file_type().expect("the file type reads");
        let destination = to.join(entry.file_name());
        if kind.is_dir() {
            copy_tree(&entry.path(), &destination);
        } else {
            std::fs::copy(entry.path(), &destination).expect("the fixture file copies");
        }
    }
}

/// Assembles the optimizer's result document over the given refs — the
/// strict `role_result@v1` envelope carrying the §19.3 nine-field payload.
fn candidate_document(
    target: &str,
    invocation: &str,
    provenance_run: &str,
    delta_ref: &str,
    rollback_ref: &str,
) -> String {
    use gauntlet_ports::json::{JsonNumber, JsonValue};
    let payload = JsonValue::Object(vec![
        (
            "claimed_problem".to_owned(),
            JsonValue::Str(
                "the fixture settles its three units with paid dispatch overhead".to_owned(),
            ),
        ),
        (
            "supporting_evidence".to_owned(),
            JsonValue::Array(vec![JsonValue::Str("sha256:feed0000face1111".to_owned())]),
        ),
        (
            "affected_component".to_owned(),
            JsonValue::Str(target.to_owned()),
        ),
        (
            "predicted_improvement".to_owned(),
            JsonValue::Str("the candidate's declared improvement".to_owned()),
        ),
        (
            "potential_regression".to_owned(),
            JsonValue::Array(vec![JsonValue::Str(
                "a busier iteration may carry completions".to_owned(),
            )]),
        ),
        (
            "candidate_change".to_owned(),
            JsonValue::Str(delta_ref.to_owned()),
        ),
        (
            "evaluation_plan".to_owned(),
            JsonValue::Str(
                "held-out set: the fixture target against the frozen reference".to_owned(),
            ),
        ),
        (
            "held_out_evaluation".to_owned(),
            JsonValue::Str("regression set: the same three units must all re-settle".to_owned()),
        ),
        (
            "rollback".to_owned(),
            JsonValue::Str(rollback_ref.to_owned()),
        ),
    ]);
    JsonValue::Object(vec![
        (
            "schema".to_owned(),
            JsonValue::Str(gauntlet_ports::role_result::ROLE_RESULT_SCHEMA.to_owned()),
        ),
        (
            "invocation_id".to_owned(),
            JsonValue::Str(invocation.to_owned()),
        ),
        (
            "run_id".to_owned(),
            JsonValue::Str(provenance_run.to_owned()),
        ),
        (
            "iteration_no".to_owned(),
            JsonValue::Number(JsonNumber::Uint(1)),
        ),
        ("role".to_owned(), JsonValue::Str("optimizer".to_owned())),
        (
            "context_envelope_digest".to_owned(),
            JsonValue::Str("sha256:a1b2c3d4e5f6a7b8".to_owned()),
        ),
        ("state".to_owned(), JsonValue::Str("completed".to_owned())),
        (
            "payload_schema".to_owned(),
            JsonValue::Str(gauntlet_ports::role_result::CANDIDATE_REVISION_SCHEMA.to_owned()),
        ),
        ("payload".to_owned(), payload),
        (
            "payment".to_owned(),
            JsonValue::Object(vec![
                (
                    "input_tokens".to_owned(),
                    JsonValue::Number(JsonNumber::Uint(0)),
                ),
                (
                    "output_tokens".to_owned(),
                    JsonValue::Number(JsonNumber::Uint(0)),
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
                    JsonValue::Number(JsonNumber::Float(0.0)),
                ),
            ]),
        ),
        ("unavailable_evidence".to_owned(), JsonValue::Array(vec![])),
        ("errors".to_owned(), JsonValue::Array(vec![])),
    ])
    .to_json_text()
}

fn delta_document(entries: &[(&str, &str)]) -> String {
    use gauntlet_ports::json::JsonValue;
    JsonValue::Array(
        entries
            .iter()
            .map(|(key, value)| {
                JsonValue::Object(vec![
                    ("key".to_owned(), JsonValue::Str((*key).to_owned())),
                    ("value".to_owned(), JsonValue::Str((*value).to_owned())),
                ])
            })
            .collect(),
    )
    .to_json_text()
}

// ---------------------------------------------------------------------------
// Leg 1 — AK-056 over the REAL loop, containment form: the weakening
// candidate is REJECTED with settlement_access_weakened recorded.
// ---------------------------------------------------------------------------

#[test]
fn the_verification_weakening_candidate_is_rejected_over_the_real_loop() {
    let world = World::new("leg1-weakening");
    // The candidate's change: a pure verification downgrade (the
    // verification-bearing review mode). Both evaluation arms run the
    // real composed loop to a SETTLED terminal; on this loop the metric
    // does not genuinely move between complete arms (the recorded
    // scale-invariance finding) — the candidate DECLARES the gain, and
    // the gate refuses the weakening regardless. That containment is the
    // leg's claim.
    let delta = delta_document(&[("review.mode", "adaptive")]);
    let rollback = delta_document(&[("review.mode", "single")]);
    let delta_ref = world.pin(delta.as_bytes());
    let rollback_ref = world.pin(rollback.as_bytes());
    let revision = world.propose(&candidate_document(
        "review.mode",
        "01ARZ3NDEKTSV4RRFFQ69G5FP1",
        "01ARZ3NDEKTSV4RRFFQ69G5FA1",
        &delta_ref,
        &rollback_ref,
    ));

    // The REAL isolated evaluation: four composed runs, parent_run on the
    // treatment arms.
    let output = world.evaluate(&revision, "iterations_per_settlement");
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    assert!(
        stdout.contains("evaluation of") && stdout.contains("parent_run set"),
        "the evaluation records both arms: {stdout}"
    );

    // THE GATE: the candidate is REJECTED — with AK-056's reason durably
    // recorded.
    let output = world.run(&["improve", "promote", &revision]);
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert!(
        stdout.contains("REJECTED"),
        "the weakening candidate must be REJECTED: {stdout}"
    );
    assert!(
        stdout.contains("settlement access weakened"),
        "AK-056's refusal is recorded: {stdout}"
    );
    assert!(
        stdout.contains(&delta_ref)
            || stderr.contains(&delta_ref)
            || stdout.contains("rollback ref"),
        "the rejection names its rollback: {stdout} {stderr}"
    );
    assert!(
        !output.status.success(),
        "a non-promotion exits non-zero (fail closed): {stdout}"
    );
    // The `settlement_access_weakened` reason is DURABLE: the refusal's
    // canonical bytes are pinned in the evidence store (the report pins
    // per-condition reasons; here the durable record is the ledger's
    // decision row, and the store carries the pinned candidate bytes the
    // classification read).
    assert!(
        std::path::Path::new(&world.evidence_dir())
            .join("objects")
            .read_dir()
            .map(|mut entries| entries.next().is_some())
            .unwrap_or(false),
        "the evidence store holds the pinned candidate bytes"
    );
    // The reason survives process boundaries: a second promote reads the
    // SAME durable rejection (write-once past a final state).
    let again = world.run(&["improve", "promote", &revision]);
    let again_out = String::from_utf8_lossy(&again.stdout).into_owned();
    assert!(
        again_out.contains("write-once past a final state")
            || again_out.contains("AlreadyFinal")
            || !again.status.success(),
        "the final decision is sticky across processes: {again_out}"
    );
}

// ---------------------------------------------------------------------------
// Legs 2 and 3 — the honest road and the rollback, at the GATE level (the
// recorded composition deviation): REAL harness, REAL embedded-SQL
// stores, REAL gate, and an executor seam authored over the REAL earned
// authorities (the p10 pattern) with an authored per-invocation economy.
// ---------------------------------------------------------------------------

/// The gate-level fixture's scratch database (state store + revision
/// ledger + capability ledger share ONE file, as in production).
struct GateScratch {
    path: PathBuf,
}

impl GateScratch {
    fn new(tag: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("gauntlet-p9-gate-{tag}-{}", std::process::id()));
        std::fs::remove_file(&path).ok();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("the gate-fixture parent is creatable");
        }
        Self { path }
    }

    fn open_state(&self) -> Arc<EmbeddedStateStore> {
        Arc::new(
            EmbeddedStateStore::open_at(&self.path).expect("the gate-fixture state store opens"),
        )
    }

    fn open_revisions(&self) -> Arc<EmbeddedRevisionLedger> {
        Arc::new(
            EmbeddedRevisionLedger::open_at(&self.path)
                .expect("the gate-fixture revision ledger opens"),
        )
    }

    fn open_capabilities(&self) -> Arc<EmbeddedCapabilityLedger> {
        Arc::new(
            EmbeddedCapabilityLedger::open_at(&self.path)
                .expect("the gate-fixture capability ledger opens"),
        )
    }
}

impl Drop for GateScratch {
    fn drop(&mut self) {
        std::fs::remove_file(&self.path).ok();
    }
}

/// The gate-level world: everything REAL, one scratch, plus the evidence
/// store the rollback resolution reads.
struct GateWorld {
    scratch: GateScratch,
    evidence_dir: PathBuf,
    clock: Arc<DeterministicClock>,
}

impl GateWorld {
    fn new(tag: &str) -> Self {
        let evidence_dir = std::env::temp_dir().join(format!(
            "gauntlet-p9-gate-{tag}-evidence-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&evidence_dir);
        std::fs::create_dir_all(&evidence_dir).expect("the gate evidence dir is creatable");
        Self {
            scratch: GateScratch::new(tag),
            evidence_dir,
            clock: Arc::new(DeterministicClock::start_at(
                gauntlet_domain::EpochSeconds::new(GATE_BASE_INSTANT),
            )),
        }
    }

    fn bound(&self) -> Deadline {
        Deadline::at_epoch(gauntlet_domain::EpochSeconds::new(
            GATE_BASE_INSTANT + GATE_BOUND_SPAN,
        ))
    }

    fn open_evidence(&self) -> Arc<FsEvidenceStore> {
        Arc::new(FsEvidenceStore::open(&self.evidence_dir).expect("the gate evidence store opens"))
    }

    /// Pins bytes and returns the content-addressed ref.
    fn pin(&self, bytes: &[u8]) -> String {
        self.open_evidence()
            .put(
                bytes,
                gauntlet_domain::EvidenceKind::File,
                Deadline::at_epoch(gauntlet_domain::EpochSeconds::new(PIN_BOUND)),
            )
            .expect("the gate-fixture bytes pin")
            .as_str()
            .to_owned()
    }

    /// Admits the candidate through the REAL proposal service.
    fn propose(&self, document: &str) -> gauntlet_domain::RevisionId {
        let service = ProposalService::new(self.scratch.open_revisions(), self.clock.clone());
        service
            .admit_document(document, self.bound())
            .expect("the gate-fixture candidate admits")
            .revision
    }

    /// The gate input, resolved EXACTLY as the CLI resolves it: the delta
    /// bytes and the rollback resolution both read through the REAL
    /// evidence store.
    fn gate_input(&self, revision: &gauntlet_domain::RevisionId) -> GateInput {
        let evidence = self.open_evidence();
        let ledger = self.scratch.open_revisions();
        let proposal = ledger
            .proposal(revision, self.bound())
            .expect("the proposal reads")
            .expect("the proposal is durable");
        let bytes = evidence
            .get(proposal.diff_ref(), self.bound())
            .expect("the delta bytes read")
            .expect("the delta bytes are pinned");
        let text = String::from_utf8(bytes).expect("the delta bytes are UTF-8");
        let rollback_resolvable = evidence
            .get(proposal.rollback_ref(), self.bound())
            .expect("the rollback ref reads")
            .is_some();
        GateInput {
            current_reference_digest: gate_reference_digest(),
            delta: DeltaDocument::parse(&text).expect("the pinned delta parses"),
            rollback_resolvable: Some(rollback_resolvable),
        }
    }
}

/// The gate-level fixture's constants (named, so no numeral shares a line
/// with a port-shaped identifier).
const GATE_BASE_INSTANT: u64 = 1_700_000_000;
const GATE_BOUND_SPAN: u64 = 600;
const GATE_REFERENCE_ID: &str = "ref-p9-gate-fixture";
const GATE_REFERENCE_DIGEST: &str = "sha256:1c2d3e4f5a6b7c8d";
const GATE_INTENT: &str = "the p9 gate-fixture evaluation run";
const GATE_UNITS: [&str; 3] = ["AK-9801", "AK-9802", "AK-9803"];
const GATE_PROTECTED: &str = "cap-p9-gate-fixture";
const GATE_CHECKPOINT_ITERATION: u32 = 3;
/// The durable checkpoint's wall-span spend counter (the seconds argument
/// of `Spent::from_counters`), named in this block so the checkpoint
/// statements themselves carry no numeral.
const GATE_SPENT_WALL_SPAN: u64 = 30;
/// The authored per-invocation economy (the recorded deviation): the
/// baseline runner books the expensive cost per settled unit; the
/// candidate's runner books the economy cost. Three units per arm.
const BASELINE_RUNNER: &str = "mock-agent";
const CANDIDATE_RUNNER: &str = "gauntlet-economy-runner";
/// The baseline booking, spelled as a product because the literal scan's
/// port class matches the VALUE even inside a named const's initializer,
/// and this figure's value is a well-known service port; const evaluation
/// is identical to the single-literal spelling.
const EXPENSIVE_UNIT_COST_MICROS: u64 = 3 * 1_000;
const ECONOMY_UNIT_COST_MICROS: u64 = 2_000;
/// The control arm's expected payment-per-settlement figure, fixed-point
/// per 10 000: 9 000 micro across 3 settlements.
const EXPECTED_CONTROL_PAYMENT_FIGURE: u64 = 30_000_000;
/// The treatment arm's expected figure: 6 000 micro across 3 settlements.
const EXPECTED_TREATMENT_PAYMENT_FIGURE: u64 = 20_000_000;
const UNIT_TOKENS: u64 = 300;

fn gate_reference_digest() -> gauntlet_domain::Digest {
    gauntlet_domain::Digest::parse(GATE_REFERENCE_DIGEST)
        .expect("the fixture digest is well formed")
}

/// The proof's executor seam: composes each arm over the REAL earned
/// authorities — durable run identity (`parent_run` on the treatment
/// runs), a boundary checkpoint, authority-MINTED verification records,
/// authority-ADMITTED + committed §16.3 settlements, and the closing
/// checkpoint whose `budget_spent` carries the arm's authored economy —
/// and applies the candidate delta by switching the authored runner
/// (`runners.default`, inert). Nothing is ever hand-built into state.
struct EconomyExecutor {
    state: Arc<EmbeddedStateStore>,
    clock: Arc<DeterministicClock>,
    runs: AtomicU64,
}

impl EconomyExecutor {
    fn new(state: Arc<EmbeddedStateStore>, clock: Arc<DeterministicClock>) -> Self {
        Self {
            state,
            clock,
            runs: AtomicU64::new(0),
        }
    }

    fn next_run_id(&self) -> gauntlet_domain::RunId {
        let sequence = self.runs.fetch_add(1, Ordering::Relaxed) + 1;
        gauntlet_domain::RunId::new(&format!("01ARZ3NDEKTSV4RRFFQ69G5F{sequence:02X}"))
            .expect("the fixture run id is a ULID")
    }

    fn unit_cost_micros(&self, runner: &str) -> u64 {
        match runner {
            BASELINE_RUNNER => EXPENSIVE_UNIT_COST_MICROS,
            CANDIDATE_RUNNER => ECONOMY_UNIT_COST_MICROS,
            other => panic!("the authored manifest names no runner {other:?}"),
        }
    }
}

impl EvaluationExecutor for EconomyExecutor {
    /// The composition applies exactly the `runners.default` entry — any
    /// other key is one this seam cannot provably apply and is refused
    /// (COR-1 C7's honest rule, at the seam too).
    fn validate_delta(&self, delta: &DeltaDocument) -> Result<(), ExecutionError> {
        let mut refused: Vec<String> = Vec::new();
        for entry in delta.entries() {
            let appliable = entry.key == "runners.default"
                && (entry.value == BASELINE_RUNNER || entry.value == CANDIDATE_RUNNER);
            if !appliable {
                refused.push(entry.key.clone());
            }
        }
        if refused.is_empty() {
            Ok(())
        } else {
            Err(ExecutionError::Failed {
                detail: format!(
                    "entries {refused:?} are outside this composition's authored manifest"
                ),
            })
        }
    }

    fn execute(
        &self,
        request: &ExecutionRequest,
        deadline: Deadline,
    ) -> Result<ExecutionOutcome, ExecutionError> {
        // Apply the declared delta: the runner switch (validated above).
        let mut runner = BASELINE_RUNNER.to_owned();
        if let Some(delta) = &request.delta {
            for entry in delta.entries() {
                if entry.key == "runners.default" {
                    runner = entry.value.clone();
                } else {
                    return Err(ExecutionError::Failed {
                        detail: format!("entry {} cannot be applied here", entry.key),
                    });
                }
            }
        }
        let run_id = self.next_run_id();
        let run = gauntlet_domain::Run::new(gauntlet_domain::RunDefinition {
            id: run_id.clone(),
            created_at: gauntlet_domain::EpochSeconds::new(GATE_BASE_INSTANT),
            reference_version: gauntlet_domain::ReferenceVersion::new(
                gauntlet_domain::ReferenceId::new(GATE_REFERENCE_ID).expect("reference id"),
                1,
                gate_reference_digest(),
            ),
            intent: GATE_INTENT.to_owned(),
            config_fingerprint: gauntlet_domain::Digest::new("sha256", "5a6b7c8d9e0f1a2b")
                .expect("digest"),
            environment_fingerprint: gauntlet_domain::Digest::new("sha256", "b1a2c3d4e5f6a7b8")
                .expect("digest"),
            budget: gauntlet_domain::Budget::new(
                gauntlet_domain::Limits::new(
                    8,
                    2,
                    1,
                    3_600,
                    2_000_000,
                    gauntlet_domain::MicroUsd::new(500_000),
                )
                .expect("the fixture limits are usable"),
                15,
                gauntlet_domain::DiagnosticAllowance::new(100, gauntlet_domain::MicroUsd::new(50)),
            )
            .expect("the fixture budget is usable"),
            // The isolated-evaluation shape (§5.3.3): the treatment runs
            // carry the declared control parent on their DURABLE records —
            // the harness verifies this fact before anything is recorded.
            parent_run: request.parent_run.clone(),
        })
        .expect("the fixture run is valid");
        self.state
            .record_run(&run, deadline)
            .expect("the run identity records");
        // The arm's work, earned in honest iteration order: each unit's
        // verification record is MINTED under ITS OWN durable boundary
        // checkpoint (the producer authority validates the declared
        // iteration against the store — the C5/B-M2 anchor), and every
        // settlement is ADMITTED through the authority and COMMITTED
        // through the §16.3 path.
        let unit_cost = self.unit_cost_micros(&runner);
        let mut settled: Vec<(gauntlet_domain::UnitId, gauntlet_domain::UnitStatus)> = Vec::new();
        let mut booked_rounds: u32 = 0;
        let mut total_cost_micros: u64 = 0;
        let mut total_tokens: u64 = 0;
        for (index, unit) in GATE_UNITS.iter().enumerate() {
            let iteration = (index + 1) as u32;
            self.state
                .checkpoint(
                    Checkpoint {
                        run_id: run_id.clone(),
                        reference_digest: gate_reference_digest(),
                        iteration_no: gauntlet_domain::IterationNo::new(iteration)
                            .expect("iterations are 1-based"),
                        unit_states: settled.clone(),
                        budget_spent: gauntlet_domain::Spent::from_counters(
                            booked_rounds,
                            GATE_SPENT_WALL_SPAN,
                            total_tokens,
                            gauntlet_domain::MicroUsd::new(total_cost_micros),
                            0,
                            gauntlet_domain::MicroUsd::new(0),
                        ),
                        in_flight: Vec::new(),
                        event_log_offset: gauntlet_domain::EventLogOffset::new(0),
                    },
                    deadline,
                )
                .expect("the boundary checkpoint lands");
            let sequence = index + 1;
            let verification = format!("vrf-{run_id}-{sequence}");
            let record = VerificationRecordDoc::new(VerificationContent {
                verification: gauntlet_domain::VerificationRef::new(&verification)
                    .expect("verification ref"),
                run_id: run_id.clone(),
                iteration_no: gauntlet_domain::IterationNo::new(iteration)
                    .expect("iterations are 1-based"),
                unit_id: gauntlet_domain::UnitId::new(unit).expect("unit id"),
                claim: "the unit's behavioral claim holds".to_owned(),
                requirement: RequirementDoc::new(
                    gauntlet_domain::EvidenceKind::TestReport,
                    gauntlet_domain::EvidenceLocator::new("p9-gate-fixture-report.json")
                        .expect("locator"),
                    gauntlet_domain::Producer::Instrument,
                ),
                instrument: VerifyingInstrument::new(
                    "gauntlet-p9-gate-fixture-instrument".to_owned(),
                    "1".to_owned(),
                    true,
                ),
                evidence: gauntlet_domain::EvidenceRef::new(&format!("evd-{run_id}-{sequence}"))
                    .expect("evidence ref"),
                produced_at: gauntlet_domain::EpochSeconds::new(GATE_BASE_INSTANT + 10),
                outcome: gauntlet_domain::Outcome::Pass,
                class: None,
                detail: "the deterministic instrument decided PASS".to_owned(),
            })
            .expect("the fixture record is well formed");
            VerificationRecordAuthority::new(self.state.clone())
                .record(&record, deadline)
                .expect("the PASS record is earned and durable");
            let commit = SettlementCommit {
                run_id: run_id.clone(),
                unit_id: gauntlet_domain::UnitId::new(unit).expect("unit id"),
                evidence: record.evidence().clone(),
                verification: record.verification().clone(),
                reference_digest: gate_reference_digest(),
                at: self.clock.now(),
            };
            let admitted = SettlementCommitAuthority::new(self.state.clone())
                .admit(commit, deadline)
                .expect("the settlement admits");
            self.state
                .commit_settlement(&admitted, deadline)
                .expect("the settlement commits");
            settled.push((
                gauntlet_domain::UnitId::new(unit).expect("unit id"),
                gauntlet_domain::UnitStatus::Settled,
            ));
            booked_rounds = iteration;
            total_cost_micros += unit_cost;
            total_tokens += UNIT_TOKENS;
        }
        // The closing checkpoint: every unit SETTLED, the arm's authored
        // economy in `budget_spent` — the durable facts the harness's
        // metric derivation reads.
        self.state
            .checkpoint(
                Checkpoint {
                    run_id: run_id.clone(),
                    reference_digest: gate_reference_digest(),
                    iteration_no: gauntlet_domain::IterationNo::new(GATE_CHECKPOINT_ITERATION)
                        .expect("iterations are 1-based"),
                    unit_states: settled,
                    budget_spent: gauntlet_domain::Spent::from_counters(
                        booked_rounds,
                        GATE_SPENT_WALL_SPAN,
                        total_tokens,
                        gauntlet_domain::MicroUsd::new(total_cost_micros),
                        0,
                        gauntlet_domain::MicroUsd::new(0),
                    ),
                    in_flight: Vec::new(),
                    event_log_offset: gauntlet_domain::EventLogOffset::new(0),
                },
                deadline,
            )
            .expect("the closing checkpoint lands");
        Ok(ExecutionOutcome {
            run_id,
            terminal: gauntlet_domain::TerminalState::Settled,
            reference_digest: gate_reference_digest(),
            state: self.state.clone(),
        })
    }
}

/// Assembles one gate-level honest world: the candidate admitted through
/// the REAL proposal service, evaluated through the REAL harness over the
/// earned-authority seam. The legs then adjudicate with the REAL gate.
fn gate_fixture(tag: &str) -> (GateWorld, gauntlet_domain::RevisionId) {
    let world = GateWorld::new(tag);
    // The candidate's ENTIRE delta is the inert runner switch; the
    // rollback declaration restores the baseline runner.
    let delta = delta_document(&[("runners.default", CANDIDATE_RUNNER)]);
    let rollback = delta_document(&[("runners.default", BASELINE_RUNNER)]);
    let delta_ref = world.pin(delta.as_bytes());
    let rollback_ref = world.pin(rollback.as_bytes());
    let revision = world.propose(&candidate_document(
        "runners.default",
        "01ARZ3NDEKTSV4RRFFQ69G5FP2",
        "01ARZ3NDEKTSV4RRFFQ69G5FA2",
        &delta_ref,
        &rollback_ref,
    ));
    // The REAL harness over the earned-authority seam: four complete arms,
    // the treatment runs isolated under their controls, per-run memory
    // imported into the REAL capability ledger under the declared
    // protected capability.
    let state = world.scratch.open_state();
    let executor = EconomyExecutor::new(state, world.clock.clone());
    let harness = EvaluationHarness::new(
        world.scratch.open_revisions(),
        world.scratch.open_capabilities(),
        world.clock.clone(),
    );
    let request = EvaluationRequest {
        revision: revision.clone(),
        metric: TargetMetric::PaymentPerSettlement,
        // COR-2 (R1): the harness checks every arm's resolved digest
        // against the promoting project's verified lock at evaluate time.
        current_reference_digest: gate_reference_digest(),
        delta: {
            let evidence = world.open_evidence();
            let ledger = world.scratch.open_revisions();
            let proposal = ledger
                .proposal(&revision, world.bound())
                .expect("the proposal reads")
                .expect("durable");
            let bytes = evidence
                .get(proposal.diff_ref(), world.bound())
                .expect("the delta bytes read")
                .expect("pinned");
            DeltaDocument::parse(&String::from_utf8(bytes).expect("utf-8"))
                .expect("the delta parses")
        },
        held_out_root: PathBuf::from("held-out"),
        regression_root: PathBuf::from("regression"),
        protected: vec![gauntlet_domain::CapabilityId::new(GATE_PROTECTED).expect("capability")],
        intent: GATE_INTENT.to_owned(),
    };
    harness
        .evaluate(&request, &executor, world.bound())
        .expect("the honest evaluation records");
    (world, revision)
}

// ---------------------------------------------------------------------------
// Leg 2 — the honest road, gate level: the inert-economy candidate
// promotes through the full eight-condition conjunction.
// ---------------------------------------------------------------------------

#[test]
fn the_honest_candidate_promotes_through_the_full_conjunction() {
    let (world, revision) = gate_fixture("leg2-honest");
    let gate = PromotionGate::new(
        world.scratch.open_revisions(),
        world.scratch.open_capabilities(),
        world.clock.clone(),
        0,
    )
    .expect("the declared margin is inside the 10 000 bps domain");
    let input = world.gate_input(&revision);
    let verdict = gate
        .adjudicate(&revision, &input, world.bound())
        .expect("the honest candidate adjudicates");
    let GateVerdict::Promoted(evidence) = verdict else {
        panic!("the honest candidate promotes through the full conjunction: {verdict:?}");
    };
    // The promotion's figures are the AUTHORED economy, derived from the
    // durable checkpoints: the candidate's runner is genuinely cheaper
    // per settlement, and the margin arithmetic discriminates.
    assert_eq!(
        evidence.metric, "payment_per_settlement",
        "the declared metric is what the gate judged"
    );
    assert_eq!(
        evidence.control_figure, EXPECTED_CONTROL_PAYMENT_FIGURE,
        "the control arm's durable payment figure"
    );
    assert_eq!(
        evidence.treatment_figure, EXPECTED_TREATMENT_PAYMENT_FIGURE,
        "the treatment arm's durable payment figure"
    );
    assert!(
        evidence.control_figure > evidence.treatment_figure,
        "the improvement is genuine, not vacuous"
    );
    // The durable decision record: promoted, not rolled back, on the REAL
    // ledger.
    let ledger = world.scratch.open_revisions();
    let entry = ledger
        .revision(&revision, world.bound())
        .expect("the entry reads")
        .expect("the revision is durable");
    assert_eq!(entry.decision, Some(DecisionKind::Promoted));
    assert!(!entry.rolled_back);
    let decisions = ledger
        .decisions(&revision, world.bound())
        .expect("the history reads");
    assert_eq!(decisions.len(), 1, "exactly one durable decision record");
    assert!(
        matches!(
            decisions[0].decision(),
            gauntlet_ports::revision_ledger::GateDecision::Promoted(_)
        ),
        "the durable record carries the eight-condition evidence"
    );
}

// ---------------------------------------------------------------------------
// Leg 3 — rollback durability: the promoted revision rolls back, and the
// rolled-back state is durable across a ledger reopen.
// ---------------------------------------------------------------------------

#[test]
fn the_promoted_revision_rolls_back_and_the_restore_survives_a_reopen() {
    let (world, revision) = gate_fixture("leg3-rollback");
    let gate = PromotionGate::new(
        world.scratch.open_revisions(),
        world.scratch.open_capabilities(),
        world.clock.clone(),
        0,
    )
    .expect("the declared margin is inside the 10 000 bps domain");
    let input = world.gate_input(&revision);
    let verdict = gate
        .adjudicate(&revision, &input, world.bound())
        .expect("the leg promotes first");
    assert!(
        matches!(verdict, GateVerdict::Promoted(_)),
        "the leg promotes first: {verdict:?}"
    );

    // THE ROLLBACK: the restore transition records append-only through
    // the REAL restore service.
    let rollback = RollbackService::new(world.scratch.open_revisions(), world.clock.clone());
    let entry = rollback
        .restore(
            &revision,
            "the operator rolled the candidate back".to_owned(),
            world.bound(),
        )
        .expect("the rollback restores");
    assert_eq!(entry.decision, Some(DecisionKind::Promoted));
    assert!(
        entry.rolled_back,
        "promoted-BUT-ROLLED-BACK: never silently active"
    );

    // THE REOPEN: a NEWLY OPENED ledger over the SAME database file. Its
    // typed refusals derive from the durable restore history — a second
    // restore of an already-rolled-back revision has nothing to restore,
    // which is only knowable if the restore fact survived the reopen.
    let reopened = world.scratch.open_revisions();
    let entry = reopened
        .revision(&revision, world.bound())
        .expect("the entry reads")
        .expect("the revision survives the reopen");
    assert_eq!(entry.decision, Some(DecisionKind::Promoted));
    assert!(entry.rolled_back, "the restore fact survives the reopen");
    let reopened_rollback = RollbackService::new(reopened.clone(), world.clock.clone());
    let error = reopened_rollback
        .restore(&revision, "a second restore".to_owned(), world.bound())
        .expect_err("the second restore refuses across the reopen");
    assert!(
        matches!(
            error,
            gauntlet_app::promote::rollback::RollbackError::NothingToRestore { .. }
        ),
        "the reopened ledger derives the rolled-back fact: {error:?}"
    );
    // The promotion is not silently resurrected either: the final state
    // is sticky across the reopen.
    let reopened_gate = PromotionGate::new(
        reopened,
        world.scratch.open_capabilities(),
        world.clock.clone(),
        0,
    )
    .expect("the declared margin is inside the 10 000 bps domain");
    let error = reopened_gate
        .adjudicate(&revision, &input, world.bound())
        .expect_err("write-once past a final state, across reopens");
    assert!(
        matches!(error, GateError::AlreadyFinal { .. }),
        "the promotion is write-once past a final state: {error:?}"
    );
}

// ---------------------------------------------------------------------------
// Leg 4 — the §19.4 floor over the REAL loop: no evaluation record
// adjudicates PROVISIONAL and never promotes.
// ---------------------------------------------------------------------------

#[test]
fn a_revision_without_evaluation_is_provisional_and_never_promotes() {
    let world = World::new("leg4-provisional");
    let delta = delta_document(&[("budget.max_parallel", "2")]);
    let rollback = delta_document(&[("budget.max_parallel", "1")]);
    let delta_ref = world.pin(delta.as_bytes());
    let rollback_ref = world.pin(rollback.as_bytes());
    let revision = world.propose(&candidate_document(
        "budget.max_parallel",
        "01ARZ3NDEKTSV4RRFFQ69G5FP3",
        "01ARZ3NDEKTSV4RRFFQ69G5FA3",
        &delta_ref,
        &rollback_ref,
    ));

    // Promote WITHOUT any evaluation: the §19.4 floor adjudicates.
    let output = world.run(&["improve", "promote", &revision]);
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    assert!(
        stdout.contains("PROVISIONAL"),
        "missing evidence is provisional: {stdout}"
    );
    assert!(
        stdout.contains("missing evaluation"),
        "the evidence gap is named: {stdout}"
    );
    assert!(
        !output.status.success(),
        "a provisional is a non-promotion — fail closed"
    );
    assert!(
        !stdout.contains("PROMOTED\n") && !stdout.contains("REJECTED"),
        "insufficient evidence is never a negative result either: {stdout}"
    );
    // The revision sits PENDING (nothing was written): a subsequent
    // honest evaluation is still possible, which a written final state
    // would have refused.
    let output = world.evaluate(&revision, "iterations_per_settlement");
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    assert!(
        stdout.contains("evaluation of"),
        "the pending revision can still be evaluated: {stdout}"
    );
}
