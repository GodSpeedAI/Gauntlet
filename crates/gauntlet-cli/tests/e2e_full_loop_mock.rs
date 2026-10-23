//! E2E: the FULL Gauntlet loop through the REAL `gauntlet` binary on the
//! mock runner (plan Task 20; ruler AK-060/AK-061; completed by the
//! COR-B3a composition correction after B3's honest live-failure stop).
//!
//! The test copies the seeded fixture target project
//! (`tests/fixtures/target`, B3's fixture: its own GAUNTLET.md with
//! `runners.default: mock-agent`, its own reference + frozen lock, its
//! real check script) to a per-test scratch directory, runs THERE, and
//! drives the composed production path from freeze to report through
//! child processes:
//!
//! 1. the happy leg (scenario `e2e-happy`): the loop settles EVERY
//!    blocking unit — record-derived assertions over the printed report,
//!    `status`, `inspect`, `verify`, and `export`; the builder-produced
//!    verification artifacts are read back from the DURABLE evidence
//!    store (the ingested bytes the §14.3 production records bound) and
//!    the pinned observation manifests prove the work landed INSIDE the
//!    admitted per-unit workspaces (COR-D3, §17.5.1 invariant 1);
//!    `verify` re-decides from the durable records and NEVER settles;
//! 2. the discrimination tooth (scenario `e2e-lying`): the result
//!    document claims success while the planted reality fails the
//!    project's own check — the loop ends in a BOUNDED honest
//!    non-settlement (non-zero exit, a bounded terminal, zero
//!    settlements, zero proven units), never a crash and never a
//!    settlement on the builder's claim;
//! 3. the report-forging liar (scenario `e2e-forged-report`, COR-F1 —
//!    the falsifier's round-1 blocking attack shipped as a tooth): the
//!    locator artifacts FORGE passing reports over marker-less source.
//!    The fixture fronts the `test` facet (GAUNTLET_TOOL_TEST binds the
//!    project's own scripts/check.sh), so the run's own instrument
//!    executes the check and observes the failure; the COR-F1 settlement
//!    gate refuses to settle an instrument-produced unit over bytes no
//!    executed passing test facet corroborates — bounded non-settled
//!    terminal, zero settlements, zero proven units, the typed
//!    `producer_uncorroborated` block pinned as evidence;
//! 4. the dry-run leg: the full read-only preflight with ZERO writes
//!    (charter D1), observed by a before/after file-set diff.
//!
//! Every assertion is RECORD-DERIVED (what the commands print and what
//! the durable stores hold), never a claim about internals.
//!
//! Composition notes (the COR-B3a correction, recorded): the fixture
//! units ride the §14.2 test-report rung — the scenario parser carries
//! single-line effect scalars only, so the two-line artifact contracts
//! (digest manifest, command transcript) cannot be expressed by the mock;
//! the controller's produce step binds the locator's ACTUAL content
//! (ingest-if-present, materialize-observation-facet-if-absent) and never
//! overwrites a produced artifact; `GAUNTLET_CLOCK=deterministic` (the
//! Annex E hook, now wired through the composition) keeps the mock's
//! fixed-2023 event epoch from stall-escalating under the wall clock.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

/// The real composed binary (same package: `src/bin/gauntlet.rs`).
const GAUNTLET: &str = env!("CARGO_BIN_EXE_gauntlet");

/// The workspace root: this package's manifest sits two levels below it
/// (`crates/gauntlet-cli`) — derived, never a path literal (AK-016).
fn workspace_root() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop();
    path.pop();
    path
}

/// The fixture target project (the workspace root's `tests/fixtures/
/// target` — B3's seeded project), resolved, never spelled.
fn fixture_target() -> PathBuf {
    workspace_root()
        .join("tests")
        .join("fixtures")
        .join("target")
}

/// The scenario fixture by stem, through the mock adapter's public
/// loader — the SINGLE source of truth for the planted workspace effects
/// (the tests assert reality against the fixture, never against a copied
/// literal).
fn scenario_effects(stem: &str) -> std::collections::BTreeMap<String, String> {
    let scenario =
        gauntlet_adapter_agent_mock::load_scenario(stem).expect("the scenario fixture loads");
    scenario
        .workspace_effects()
        .iter()
        .map(|effect| (effect.path().to_owned(), effect.content().to_owned()))
        .collect()
}

/// One scratch world: a fresh copy of the fixture project plus the
/// per-run state/evidence directories under it. Removed on drop.
struct World {
    root: PathBuf,
}

impl World {
    /// Copies the fixture and lays out the state/evidence roots. The
    /// scenario selection (`GAUNTLET_MOCK_SCENARIO`) is per-command, so
    /// one world can serve both the happy and the lying leg only when the
    /// caller wants shared state — the tests here create one world per
    /// leg so the runs never share a store.
    fn new(tag: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "gauntlet-e2e-full-loop-{tag}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("the scratch root is creatable");
        let source = fixture_target();
        assert!(
            source.is_dir(),
            "the fixture target project exists at {}",
            source.display()
        );
        copy_tree(&source, &root.join("target"));
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

    /// Creates the state/evidence dirs up front so the zero-write
    /// observation covers the fixture tree only (the run legs create
    /// these; the dry-run leg must create nothing, so its world observes
    /// the whole scratch).
    fn ensure_layout(&self) {
        std::fs::create_dir_all(self.state_dir()).expect("state dir");
        std::fs::create_dir_all(self.evidence_dir()).expect("evidence dir");
    }

    /// The command environment for one leg.
    fn env(&self, scenario: &str, seed: &str) -> BTreeMap<&'static str, String> {
        let mut env = BTreeMap::new();
        env.insert("GAUNTLET_STATE_DIR", self.state_dir().display().to_string());
        env.insert(
            "GAUNTLET_ARTIFACT_DIR",
            self.evidence_dir().display().to_string(),
        );
        env.insert("GAUNTLET_MOCK_SCENARIO", scenario.to_owned());
        env.insert("GAUNTLET_ID_SEED", seed.to_owned());
        // The Annex E clock hook, through the composition (the COR-B3a
        // wiring): the virtual clock rests at epoch 0, before the mock's
        // fixed-2023 event epoch, so the stall detector can never fire.
        env.insert("GAUNTLET_CLOCK", "deterministic".to_owned());
        // The required control settings (environment-only today; Finding 3
        // of B3's report stands as an operator-experience residual).
        env.insert("GAUNTLET_STALL_TIMEOUT_MS", "60000".to_owned());
        env.insert("GAUNTLET_RETRY_BASE_MS", "100".to_owned());
        env.insert("GAUNTLET_RETRY_MAX_MS", "2000".to_owned());
        env.insert("GAUNTLET_MAX_ATTEMPTS_PER_UNIT", "3".to_owned());
        // COR-F1: the fixture GAUNTLET.md fronts the `test` facet, whose
        // composition REQUIRES the tool binding — the binding names THIS
        // world's own check command, run over the workspace where the
        // dispatched work lands (the check reads `src/gadget.rs` relative
        // to its cwd). The command is DERIVED from the fixture's scripts
        // dir and made executable here: a single-token binding, and the
        // proof never leans on checkout exec-bit semantics.
        env.insert("GAUNTLET_TOOL_TEST", check_tool_of(&self.target()));
        env
    }

    /// Runs the real binary in the fixture copy with the leg's env.
    fn run(&self, env: &BTreeMap<&'static str, String>, args: &[&str]) -> Output {
        let mut command = Command::new(GAUNTLET);
        command.current_dir(self.target());
        for (key, value) in env {
            command.env(key, value);
        }
        command
            .args(args)
            .output()
            .expect("the gauntlet binary runs")
    }
}

impl Drop for World {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// Recursively copies `from` to `to` (fixture trees are small).
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

/// The fixture target's one check command, as a tool binding: the
/// scripts dir's single file, made executable for the run's test
/// instrument. Derived, never spelled (AK-016): the command's name is
/// the fixture's own content.
fn check_tool_of(target: &Path) -> String {
    let scripts = target.join("scripts");
    let mut scripts_files: Vec<PathBuf> = std::fs::read_dir(&scripts)
        .expect("the fixture scripts dir reads")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .collect();
    scripts_files.sort();
    assert_eq!(
        scripts_files.len(),
        1,
        "the fixture carries exactly one check command: {scripts_files:?}"
    );
    let script = scripts_files.remove(0);
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))
        .expect("the fixture check command is made executable");
    script.display().to_string()
}

/// The stdout of one command, as lines.
fn lines(output: &Output) -> Vec<String> {
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::to_owned)
        .collect()
}

/// The run id the run leg printed (`run: run_id=<id>`).
fn run_id_of(output: &Output) -> String {
    lines(output)
        .iter()
        .find(|line| line.starts_with("run: run_id="))
        .and_then(|line| line.strip_prefix("run: run_id="))
        .map(|id| id.to_owned())
        .expect("the run prints its run id")
}

/// The report JSON line (the canonical record-derived report the run leg
/// prints last).
fn report_json_of(output: &Output) -> String {
    lines(output)
        .iter()
        .find(|line| line.starts_with("{\"run_id\""))
        .cloned()
        .expect("the run prints its canonical report JSON")
}

/// Counts occurrences of a needle in a text.
fn count(haystack: &str, needle: &str) -> usize {
    haystack.matches(needle).count()
}

// ---------------------------------------------------------------------------
// Durable-store readers (COR-D3): the builder's per-unit workspaces are
// RELEASED after each completion (§16 ordering — cleanup never precedes the
// settlement commit), so the run's post-run reality lives in the durable
// evidence store: the ingested locator artifacts (content-addressed
// `objects/` tree) and the pinned observation manifests the filesystem
// instrument wrote INSIDE each per-unit workspace mid-run. Reading them is
// the record-derived way to prove execution anchored where admission said
// it did — no assertion here trusts a report's say-so.
// ---------------------------------------------------------------------------

/// Whether every one of `bodies` is the exact content of some object in
/// the evidence store (the `objects/<hex>` payload tree).
fn store_holds_exact_objects(evidence_dir: &Path, bodies: &[String]) -> bool {
    let mut wanted: Vec<&String> = bodies.iter().collect();
    let mut stack = vec![evidence_dir.join("objects")];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if let Ok(content) = std::fs::read(&path) {
                wanted.retain(|body| content.as_slice() != body.as_bytes());
            }
        }
    }
    wanted.is_empty()
}

/// Whether any object in the evidence store CONTAINS `needle` (the
/// observation manifests are text documents pinned in the same tree).
fn store_holds_text_containing(evidence_dir: &Path, needle: &str) -> bool {
    !records_containing(evidence_dir, needle).is_empty()
}

/// Every evidence-store OBJECT whose text contains `needle` — the
/// record-derived reader for the recovery's pinned forensic records (the
/// tests assert what the pins SAY, never silence).
fn records_containing(evidence_dir: &Path, needle: &str) -> Vec<String> {
    let mut hits = Vec::new();
    let mut stack = vec![evidence_dir.join("objects")];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if let Ok(content) = std::fs::read_to_string(&path)
                && content.contains(needle)
            {
                hits.push(content);
            }
        }
    }
    hits
}

/// Out-of-band DAMAGE (COR-R3-1's E2E leg): deletes the durable
/// evidence_productions rows out of the settled run's state database — the
/// falsifier's exact reproduction surgery. The write-once store has, by
/// design, no production delete, so the surgery goes through the state
/// adapter's own doc-hidden test-support instrument (beside `crash_helper`,
/// where the engine's name belongs). Returns how many rows the statement
/// removed (the callers ASSERT the count: damage that touched nothing must
/// fail the test, not pass it silently).
fn delete_production_rows(state_db: &Path, locator: Option<&str>) -> usize {
    gauntlet_adapter_state_postgres::test_support::delete_evidence_production_rows(
        state_db, locator,
    )
    .expect("the damage injection runs")
}

/// Out-of-band SURGERY (COR-R4-1's E2E leg): regresses the LATEST
/// checkpoint's event-log anchor below the produce event — the falsifier
/// round 4 reproduction's precondition, an A5-shaped orphan above the
/// anchor on an otherwise settled run. This is the RECORDED
/// "checkpoint() forged-row projection class (planning-layer)" deferral,
/// exercised as GIVEN damage (never widened, never repaired here). Returns
/// the PREVIOUS offset so the caller asserts the forge landed.
fn regress_latest_checkpoint_offset(state_db: &Path, run_id: &str, new_offset: u64) -> u64 {
    gauntlet_adapter_state_postgres::test_support::regress_latest_checkpoint_offset(
        state_db, run_id, new_offset,
    )
    .expect("the anchor surgery runs")
}

/// The durable `evidence_produced` events as
/// `(offset, evidence ref, locator, stored timestamp)` — the event-side
/// facts the COR-R4-1 leg pins the re-minted row against.
fn evidence_produced_events(state_db: &Path) -> Vec<(u64, String, String, i64)> {
    gauntlet_adapter_state_postgres::test_support::evidence_produced_events(state_db)
        .expect("the event reader runs")
}

/// The durable evidence-production rows as
/// `(evidence ref, locator, produced_at, event-log offset)` — the row-side
/// facts the COR-R4-1 leg pins the re-mint against.
fn evidence_production_rows(state_db: &Path) -> Vec<(String, String, i64, i64)> {
    gauntlet_adapter_state_postgres::test_support::evidence_production_rows(state_db)
        .expect("the production reader runs")
}

// ---------------------------------------------------------------------------
// Leg 1 — the happy loop: every blocking unit settles.
// ---------------------------------------------------------------------------

#[test]
fn the_composed_loop_settles_every_unit_and_pins_the_artifacts() {
    let world = World::new("happy");
    let env = world.env("e2e-happy", "e2e-happy-seed");

    // THE RUN: freeze → dispatch → observe → verify → settle → report.
    let run = world.run(&env, &["run", "seeded bounded task"]);
    assert!(
        run.status.success(),
        "the happy leg exits 0: stdout={}\nstderr={}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    let stdout = String::from_utf8_lossy(&run.stdout).into_owned();
    assert!(
        stdout.contains("terminal=settled"),
        "the loop reaches the settled terminal: {stdout}"
    );
    // Record-derived report (COR-D10): the unit->category pairs come from
    // the PARSED report JSON — never from exact prose, so a wording change
    // in the report module cannot break this proof without a behavior
    // change. Every blocking unit lands in the proven category.
    let report = report_json_of(&run);
    use gauntlet_ports::json::JsonValue;
    let parsed =
        gauntlet_ports::json::parse(&report).expect("the printed report is valid canonical JSON");
    let units = match parsed.get("units") {
        Some(JsonValue::Array(units)) => units,
        other => panic!("the report carries the units array: {other:?}"),
    };
    for unit in ["AK-901", "AK-902", "AK-903"] {
        let entry = units
            .iter()
            .find(|entry| matches!(entry.get("unit"), Some(JsonValue::Str(name)) if name == unit))
            .unwrap_or_else(|| panic!("unit {unit} appears in the report: {report}"));
        let category = match entry.get("category") {
            Some(JsonValue::Str(category)) => Some(category.as_str()),
            _ => None,
        };
        assert_eq!(
            category,
            Some("implemented-and-proven"),
            "unit {unit} is proven: {report}"
        );
    }
    let run_id = run_id_of(&run);

    // The builder-produced verification artifacts are DURABLE: each
    // fixture artifact's exact bytes were INGESTED into the content-
    // addressed evidence store at its locator (the produce step bound
    // them; the §14.3 three-way equality had to hold for the unit to
    // settle, so the settled report above already proves the digests —
    // this is the byte-level half, read from the store the run wrote).
    // The planted work is NOT at the shared workspace root: execution
    // anchored inside the admitted per-unit workspaces (COR-D3,
    // §17.5.1 invariant 1), and those are released after each completion
    // (§16 ordering) — no artifact segment survives outside a per-unit
    // workspace, and the pinned observation manifests carry the walked
    // per-unit trees that name the gadget the fixture's marker demands.
    let happy_effects = scenario_effects("e2e-happy");
    let artifact_bodies: Vec<String> = [
        "marker-check-report.json",
        "check-report.json",
        "full-check-report.json",
    ]
    .iter()
    .map(|name| {
        happy_effects
            .get(&format!("evidence/{name}"))
            .expect("the scenario plants the artifact")
            .clone()
    })
    .collect();
    assert!(
        store_holds_exact_objects(&world.evidence_dir(), &artifact_bodies),
        "every planted verification artifact is durably ingested at its locator"
    );
    let workspaces_root = world.state_dir().join("workspaces");
    assert!(
        !workspaces_root.join("evidence").exists(),
        "no artifact lands at the shared workspace root: execution anchors at the per-unit \
         workspaces (COR-D3)"
    );
    // The planted source's manifest path, built segment-wise (a joined
    // path literal is environment coupling the literals scanner denies —
    // AK-016 — and the segments are the fixture's own layout).
    let planted_source_path = ["src", "gadget.rs"].join("/");
    assert!(
        store_holds_text_containing(&world.evidence_dir(), &planted_source_path),
        "the observation manifests record the per-unit workspace trees that held the planted \
         work (execution-in-admitted-workspace, red when bypassed)"
    );

    // status: the durable projection says three of three settled.
    let status = world.run(&env, &["status", "--run", &run_id]);
    let status_text = String::from_utf8_lossy(&status.stdout).into_owned();
    assert!(
        status_text.contains("settled=3"),
        "status reports three settlements: {status_text}"
    );

    // inspect: three durable PASS verification records, one per unit.
    let inspect = world.run(&env, &["inspect", "--run", &run_id, "--verifications"]);
    let inspect_text = String::from_utf8_lossy(&inspect.stdout).into_owned();
    for unit in ["AK-901", "AK-902", "AK-903"] {
        assert!(
            inspect_text.contains(&format!("unit={unit} outcome=PASS")),
            "the durable verification for {unit} is PASS: {inspect_text}"
        );
    }

    // verify: the read-side re-run re-decides from the durable records and
    // NEVER settles — the operator leg prints three-state outcomes and the
    // settlement count is untouched afterwards. (Every binding re-derives
    // from the durable records; the §22.2 locator re-read resolves under
    // the canonical workspace root, where the released per-unit workspaces'
    // artifacts no longer stand, so the re-decided outcomes are honest
    // INCONCLUSIVEs rather than PASSes — recorded in the COR-D report,
    // feeding the locator-semantics ruler-owner question. Nothing is
    // refused: `refused=0` pins that every binding re-derivation went
    // through the typed chain.)
    let verify = world.run(&env, &["verify", "--run", &run_id]);
    assert!(
        verify.status.success(),
        "the verify leg refuses nothing: {}",
        String::from_utf8_lossy(&verify.stderr)
    );
    let verify_text = String::from_utf8_lossy(&verify.stdout).into_owned();
    assert!(
        verify_text.contains("refused=0"),
        "no unit is refused at re-verification: {verify_text}"
    );
    assert!(
        verify_text.contains("NOTHING settled"),
        "the operator verify never settles: {verify_text}"
    );
    let status_after = world.run(&env, &["status", "--run", &run_id]);
    assert!(
        String::from_utf8_lossy(&status_after.stdout).contains("settled=3"),
        "the settlement count is exactly the loop's own (verify settled nothing)"
    );

    // export: the canonical record stream admits the whole run — the
    // §16.3 settlement rows, the durable PASS records, and the §15.1
    // correction selections.
    let out_path = world.root.join("export.jsonl");
    let export = world.run(
        &env,
        &[
            "export",
            "--run",
            &run_id,
            "--out",
            out_path.to_str().expect("a plain path"),
        ],
    );
    assert!(
        export.status.success(),
        "the export leg succeeds: {}",
        String::from_utf8_lossy(&export.stderr)
    );
    let exported = std::fs::read_to_string(&out_path).expect("the export file is written");
    assert!(
        count(&exported, "\"record\":\"settlement\"") == 3,
        "three settlement records are exported: {exported}"
    );
    assert!(
        exported.contains("\"record\":\"verification_record\""),
        "the durable PASS records are exported"
    );
    assert!(
        exported.contains("\"record\":\"correction_selection\""),
        "the §15.1 selection records are exported"
    );
}

// ---------------------------------------------------------------------------
// Leg 2 — the discrimination tooth: the lying builder never settles.
// ---------------------------------------------------------------------------

#[test]
fn the_lying_builder_is_never_settled() {
    let world = World::new("lying");
    let env = world.env("e2e-lying", "e2e-lying-seed");

    // The result document claims success; the planted reality fails the
    // project's own check, and the locator artifacts RECORD the failure
    // (tests ran, failures present — present-and-wrong is a FAIL DECISION
    // under the test-report rung, never an instrument failure).
    let run = world.run(&env, &["run", "seeded bounded task"]);
    assert!(
        !run.status.success(),
        "the lying leg does not succeed: stdout={}",
        String::from_utf8_lossy(&run.stdout)
    );
    let stdout = String::from_utf8_lossy(&run.stdout).into_owned();
    // A BOUNDED terminal, not a crash: the loop ran to an honest
    // non-settlement end under its own budget dimensions.
    assert!(
        stdout.contains("terminal=") && !stdout.contains("terminal=settled"),
        "the run ends in a bounded non-settled terminal: {stdout}"
    );
    let report = report_json_of(&run);
    for unit in ["AK-901", "AK-902", "AK-903"] {
        assert!(
            !report.contains(&format!(
                "\"unit\":\"{unit}\",\"category\":\"implemented-and-proven\""
            )),
            "the lie bought unit {unit} no proof: {report}"
        );
    }
    // (COR-D10: the old `contains("configured-but-untested")` check was
    // VACUOUS — every report carries all six category tokens regardless of
    // outcome — and is removed; the real load stays on the per-unit
    // negative above, `settled=0`, and the export's zero settlement rows.)
    let run_id = run_id_of(&run);

    // The durable projection: ZERO settlements.
    let status = world.run(&env, &["status", "--run", &run_id]);
    let status_text = String::from_utf8_lossy(&status.stdout).into_owned();
    assert!(
        status_text.contains("settled=0"),
        "the lying builder settles nothing: {status_text}"
    );

    // The export stream carries no settlement record at all — the
    // non-settlement is a durable fact, not a display choice.
    let out_path = world.root.join("lying-export.jsonl");
    let export = world.run(
        &env,
        &[
            "export",
            "--run",
            &run_id,
            "--out",
            out_path.to_str().expect("a plain path"),
        ],
    );
    assert!(
        export.status.success(),
        "the export leg succeeds on the lying run too: {}",
        String::from_utf8_lossy(&export.stderr)
    );
    let exported = std::fs::read_to_string(&out_path).expect("the export file is written");
    assert_eq!(
        count(&exported, "\"record\":\"settlement\""),
        0,
        "no settlement record exists for the lying run"
    );

    // The planted failing reality is DURABLE: the failing report the lying
    // scenario planted at the FIRST dispatched unit's locator was ingested
    // at that unit's per-unit workspace (COR-D3) and lives in the evidence
    // store's objects, byte-identical to the fixture — the divergence the
    // loop discriminated is reconstructable from the store alone. (The run
    // ends bounded BEFORE later units dispatch — its budget/attempt
    // dimensions are spent re-correcting the first unit under §11.5 row 1 —
    // so the durable failing bytes are exactly the first unit's locator's.)
    let lying_effects = scenario_effects("e2e-lying");
    let marker_report_key = ["evidence", "marker-check-report.json"].join("/");
    let marker_report = lying_effects
        .get(marker_report_key.as_str())
        .expect("the lying scenario plants the first unit's failing report");
    assert!(
        marker_report.contains("\"failures\": 1"),
        "the fixture is the source of truth: the planted report records a failure"
    );
    assert!(
        store_holds_exact_objects(&world.evidence_dir(), std::slice::from_ref(marker_report)),
        "the planted failing report is durably on record — the divergence the loop \
         discriminated is reconstructable from the store alone"
    );
}

// ---------------------------------------------------------------------------
// Leg 3 — the report-forging liar (COR-F1, the falsifier's own round-1
// blocking attack shipped as a regression tooth): the builder forges the
// instrument reports too, and the loop must NOT settle.
// ---------------------------------------------------------------------------

#[test]
fn the_report_forging_liar_is_never_settled() {
    let world = World::new("forged");
    let env = world.env("e2e-forged-report", "e2e-forged-seed");

    // The lie, exactly as the falsifier executed it: locator artifacts
    // claim tests>0 / failures=0 (the §14.2 rung would decide PASS over
    // those bytes) while the planted source fails the project's real
    // check. The run's own test instrument executes scripts/check.sh over
    // the workspace and observes exit 1 — no executed passing test facet
    // corroborates the forged reports, so the COR-F1 settlement gate
    // blocks every instrument-produced unit.
    let run = world.run(&env, &["run", "seeded bounded task"]);
    assert!(
        !run.status.success(),
        "the forging leg does not succeed: stdout={}",
        String::from_utf8_lossy(&run.stdout)
    );
    let stdout = String::from_utf8_lossy(&run.stdout).into_owned();
    // A BOUNDED terminal, not a crash: the loop ran to an honest
    // non-settlement end under its own budget dimensions.
    assert!(
        stdout.contains("terminal=") && !stdout.contains("terminal=settled"),
        "the run ends in a bounded non-settled terminal: {stdout}"
    );
    // The per-unit negative (COR-D10 record-derived shape): NO unit lands
    // in the proven category — the forged reports bought nothing.
    let report = report_json_of(&run);
    for unit in ["AK-901", "AK-902", "AK-903"] {
        assert!(
            !report.contains(&format!(
                "\"unit\":\"{unit}\",\"category\":\"implemented-and-proven\""
            )),
            "the forged report bought unit {unit} no proof: {report}"
        );
    }
    let run_id = run_id_of(&run);

    // The durable projection: ZERO settlements — the lie never entered
    // the settlement record stream.
    let status = world.run(&env, &["status", "--run", &run_id]);
    let status_text = String::from_utf8_lossy(&status.stdout).into_owned();
    assert!(
        status_text.contains("settled=0"),
        "the report-forging liar settles nothing: {status_text}"
    );

    // The export stream carries no settlement record at all.
    let out_path = world.root.join("forged-export.jsonl");
    let export = world.run(
        &env,
        &[
            "export",
            "--run",
            &run_id,
            "--out",
            out_path.to_str().expect("a plain path"),
        ],
    );
    assert!(
        export.status.success(),
        "the export leg succeeds on the forged run too: {}",
        String::from_utf8_lossy(&export.stderr)
    );
    let exported = std::fs::read_to_string(&out_path).expect("the export file is written");
    assert_eq!(
        count(&exported, "\"record\":\"settlement\""),
        0,
        "no settlement record exists for the forged run"
    );

    // The forged passing report was INGESTED (the produce step binds the
    // locator's actual content) and the typed non-corroboration block was
    // PINNED: both halves of the discrimination are reconstructable from
    // the durable store alone — the bytes were there, the instrument's
    // own contradicting execution was the reason they did not settle.
    let forged_effects = scenario_effects("e2e-forged-report");
    let marker_report_key = ["evidence", "marker-check-report.json"].join("/");
    let forged_report = forged_effects
        .get(marker_report_key.as_str())
        .expect("the forging scenario plants the first unit's forged report");
    assert!(
        forged_report.contains("\"failures\": 0"),
        "the fixture is the source of truth: the planted report FORGES a pass"
    );
    assert!(
        store_holds_exact_objects(&world.evidence_dir(), std::slice::from_ref(forged_report)),
        "the forged report is durably on record — the bytes the loop refused to settle over"
    );
    assert!(
        store_holds_text_containing(&world.evidence_dir(), "producer_uncorroborated"),
        "the typed non-corroboration block is pinned evidence, not silence"
    );
}

// ---------------------------------------------------------------------------
// Leg 4 — the dry-run: the full read-only preflight, zero writes.
// ---------------------------------------------------------------------------

#[test]
fn the_dry_run_preflight_writes_nothing() {
    let world = World::new("dryrun");
    let env = world.env("e2e-happy", "e2e-dryrun-seed");

    // Snapshot the WHOLE scratch (fixture copy + state/evidence roots) —
    // COR-D6: each file's CONTENT is hashed into the snapshot, so the
    // zero-write claim covers in-place overwrites, not just the path set.
    let file_set = |root: &Path| -> Vec<(String, u64)> {
        let mut entries: Vec<(String, u64)> = Vec::new();
        walk(root, root, &mut entries);
        entries.sort();
        entries
    };
    let before = file_set(&world.root);

    let dry = world.run(&env, &["run", "--dry-run", "seeded bounded task"]);
    assert!(
        dry.status.success(),
        "the dry-run exits 0: stdout={}\nstderr={}",
        String::from_utf8_lossy(&dry.stdout),
        String::from_utf8_lossy(&dry.stderr)
    );
    let stdout = String::from_utf8_lossy(&dry.stdout).into_owned();
    assert!(
        stdout.contains("dry-run") && stdout.contains("zero writes"),
        "the dry-run announces its read-only posture: {stdout}"
    );
    assert!(
        stdout.contains("lock verified"),
        "the dry-run verified the frozen lock (the fixture's regenerated digest): {stdout}"
    );
    assert!(
        stdout.contains("units=[AK-901]") && stdout.contains("units=[AK-903]"),
        "the plan preview names the fixture units: {stdout}"
    );
    assert!(
        !stdout.contains("run: run_id="),
        "no run was created: {stdout}"
    );

    let after = file_set(&world.root);
    assert_eq!(before, after, "the dry-run wrote nothing anywhere");
}

/// Collects every file under `root` as (relative path, content hash) for
/// the zero-write diff (COR-D6: the content hash makes an in-place
/// overwrite observable). The hash is a std SipHash over the bytes —
/// deterministic within this process, which is all a before/after
/// comparison needs.
fn walk(root: &Path, dir: &Path, paths: &mut Vec<(String, u64)>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(root, &path, paths);
        } else if let Ok(relative) = path.strip_prefix(root) {
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            if let Ok(bytes) = std::fs::read(&path) {
                use std::hash::Hasher;
                hasher.write(&bytes);
            }
            use std::hash::Hasher;
            paths.push((relative.display().to_string(), hasher.finish()));
        }
    }
}

// ---------------------------------------------------------------------------
// Leg 5 — max_parallel: 2 over the multi-unit fixture (COR-D2): the
// observation binding is write-once per (run, iteration) while dispatch
// admits two units into the SAME iteration. The surplus completion CARRIES
// to the following iteration and binds there — the loop settles every unit
// with one binding per iteration and never surfaces the store's collision
// refusal as a run error.
// ---------------------------------------------------------------------------

#[test]
fn parallel_completions_carry_and_never_collide() {
    let world = World::new("parallel");
    let mut env = world.env("e2e-happy", "e2e-parallel-seed");
    env.insert("GAUNTLET_MAX_PARALLEL", "2".to_owned());

    // THE RUN: two units dispatch into iteration 1; both complete in the
    // same drain pass; the second carries. Iterations 2 and 3 absorb the
    // carried completions (and dispatch the remaining unit).
    let run = world.run(&env, &["run", "seeded bounded task"]);
    assert!(
        run.status.success(),
        "the parallel leg exits 0 — no raw store error out of the drain: stdout={}\nstderr={}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    let stdout = String::from_utf8_lossy(&run.stdout).into_owned();
    assert!(
        stdout.contains("terminal=settled"),
        "every unit settles despite the parallel burst: {stdout}"
    );
    let run_id = run_id_of(&run);

    // status: three of three settled.
    let status = world.run(&env, &["status", "--run", &run_id]);
    let status_text = String::from_utf8_lossy(&status.stdout).into_owned();
    assert!(
        status_text.contains("settled=3"),
        "the parallel run settles all three units: {status_text}"
    );

    // Exactly ONE observation binding per (run, iteration), across THREE
    // iterations — the carried completions bound at their own iteration's
    // anchor, pairwise distinct bundles, never a second bundle for one key.
    let mut bundles: Vec<String> = Vec::new();
    for iteration in ["1", "2", "3"] {
        let inspect = world.run(
            &env,
            &["inspect", "--run", &run_id, "--iteration", iteration],
        );
        let text = String::from_utf8_lossy(&inspect.stdout).into_owned();
        let binding = text
            .lines()
            .find(|line| line.starts_with(&format!("inspect: iteration={iteration} observation=")))
            .expect("the iteration holds its own observation binding")
            .to_owned();
        let bundle_ref = binding
            .split("digest=")
            .next()
            .expect("the binding line carries a bundle ref")
            .to_owned();
        assert!(
            !bundles.contains(&bundle_ref),
            "each iteration binds a DIFFERENT bundle: {binding}"
        );
        bundles.push(bundle_ref);
    }
}

// ---------------------------------------------------------------------------
// Leg 6 — the settled-resume no-op through the binary (COR-R2-1, falsifier
// round 2's blocking reproduction). The composed binary runs the
// produced-artifact world, whose productions are keyed by the locator
// artifacts' content refs; before the production key surface was unified
// the FIRST `gauntlet resume` of this fully settled run exited 1 reporting
// a dishonest `budget_exhausted` and burned an iteration into the durable
// checkpoint.
// ---------------------------------------------------------------------------

#[test]
fn the_first_resume_of_a_fully_settled_run_is_a_settled_noop() {
    let world = World::new("settled-resume");
    let env = world.env("e2e-happy", "e2e-happy-seed");
    let run = world.run(&env, &["run", "seeded bounded task"]);
    assert!(
        run.status.success(),
        "the happy leg settles first: stdout={}\nstderr={}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    let run_stdout = String::from_utf8_lossy(&run.stdout).into_owned();
    let settled_line = run_stdout
        .lines()
        .find(|line| line.starts_with("run: terminal=settled iterations="))
        .expect("the run settles")
        .to_owned();
    let iterations = settled_line
        .rsplit('=')
        .next()
        .expect("the iteration count is printed")
        .to_owned();
    let run_id = run_id_of(&run);

    // The durable baseline: the export stream, byte for byte.
    let before = world.run(&env, &["export", "--run", &run_id]);
    assert!(
        before.status.success(),
        "the settled run exports: {}",
        String::from_utf8_lossy(&before.stderr)
    );
    let stream_before = String::from_utf8_lossy(&before.stdout).into_owned();

    // Zero-write observation over the WHOLE scratch (the dry-run leg's
    // content-hashing shape, reused): each file's bytes are hashed, so
    // in-place durable overwrites are observable.
    let file_set = |root: &Path| -> Vec<(String, u64)> {
        let mut entries: Vec<(String, u64)> = Vec::new();
        walk(root, root, &mut entries);
        entries.sort();
        entries
    };
    let snapshot = file_set(&world.root);

    // THE FIRST RESUME — the exact leg the falsifier executed twice.
    let resume = world.run(&env, &["resume", &run_id]);
    let resume_stdout = String::from_utf8_lossy(&resume.stdout).into_owned();
    assert!(
        resume.status.success(),
        "the FIRST resume of the fully settled run exits 0: stdout={resume_stdout}\nstderr={}",
        String::from_utf8_lossy(&resume.stderr)
    );
    assert!(
        resume_stdout.contains("terminal=settled"),
        "the resume reports the honest settled terminal: {resume_stdout}"
    );
    assert!(
        resume_stdout.contains(&format!("iterations={iterations}")),
        "zero iterations are burned (run settled at {iterations}): {resume_stdout}"
    );
    assert!(
        resume_stdout.contains("implemented-and-proven 3"),
        "the settled report stands: {resume_stdout}"
    );

    // Zero writes: the durable record is byte-identical and no file in
    // the world changed.
    let after = world.run(&env, &["export", "--run", &run_id]);
    let stream_after = String::from_utf8_lossy(&after.stdout).into_owned();
    assert_eq!(
        stream_after, stream_before,
        "the settled no-op wrote nothing to the durable record"
    );
    let after_set = file_set(&world.root);
    assert_eq!(
        snapshot, after_set,
        "the settled no-op wrote nothing anywhere"
    );
}

// ---------------------------------------------------------------------------
// Leg 7 — the damaged-settled-run resumes (COR-R3-1, falsifier round 3's
// F-R3-1 exact reproductions through the real binary). The composed binary
// runs the produced-artifact world; the falsifier deleted the durable
// evidence_productions rows of a fully settled run and observed, before the
// correction, FALSE forensic pins ("no frozen locator agrees … not durably
// recoverable" over bytes that WERE in the store, citing events that did not
// exist for those refs) plus a dishonest `budget_exhausted` terminal on the
// all-rows case and silent masking on the one-row case. The honest outcomes
// are pinned here: terminal=settled at the checkpoint's own iteration, one
// forensic record per damaged locator (world + locator + what is actually
// missing, bounded by the resume anchor), the intact locators silent, and
// the durable event log byte-identical.
// ---------------------------------------------------------------------------

/// The fixture units' frozen locators, DERIVED from the fixture's own
/// answer key (AK-016: paths are resolved from their source of truth,
/// never spelled — the locator literals would be environment coupling and
/// the hardcoded-literal gate rightly refuses them).
fn fixture_locators() -> Vec<String> {
    let key = std::fs::read_to_string(fixture_target().join(".wayfinder").join("answer-key.md"))
        .expect("the fixture answer key reads");
    let mut locators: Vec<String> = Vec::new();
    for line in key.lines() {
        let Some(rest) = line.trim().strip_prefix("evidence_required:") else {
            continue;
        };
        let Some(at) = rest.find("locator: ") else {
            continue;
        };
        let value = &rest[at + "locator: ".len()..];
        let end = value.find([',', '}']).unwrap_or(value.len());
        locators.push(value[..end].trim().to_owned());
    }
    locators.sort();
    assert_eq!(
        locators.len(),
        3,
        "the fixture freezes three located units: {locators:?}"
    );
    locators
}

#[test]
fn the_resume_of_a_settled_run_with_all_production_rows_deleted_is_honest() {
    let world = World::new("settled-damage-all");
    let env = world.env("e2e-happy", "e2e-happy-seed");
    let run = world.run(&env, &["run", "seeded bounded task"]);
    assert!(
        run.status.success(),
        "the happy leg settles first: stdout={}\nstderr={}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    let run_stdout = String::from_utf8_lossy(&run.stdout).into_owned();
    let iterations = run_stdout
        .lines()
        .find(|line| line.starts_with("run: terminal=settled iterations="))
        .expect("the run settles")
        .rsplit('=')
        .next()
        .expect("the iteration count is printed")
        .to_owned();
    let run_id = run_id_of(&run);
    let before = world.run(&env, &["export", "--run", &run_id]);
    assert!(before.status.success(), "the settled run exports");
    let stream_before = String::from_utf8_lossy(&before.stdout).into_owned();

    // THE DAMAGE: every production row deleted; every durable event and
    // evidence byte left in place (LESS damage than any real crash).
    let deleted = delete_production_rows(&world.state_dir().join("gauntlet-state.db"), None);
    assert_eq!(
        deleted, 3,
        "the damage deleted exactly the run's three rows"
    );

    // THE RESUME: honest settled terminal, zero burned iterations, honest
    // per-locator forensics, never a budget_exhausted lie.
    let resume = world.run(&env, &["resume", &run_id]);
    let resume_stdout = String::from_utf8_lossy(&resume.stdout).into_owned();
    let resume_stderr = String::from_utf8_lossy(&resume.stderr).into_owned();
    assert!(
        resume.status.success(),
        "the damaged settled run resumes honestly: stdout={resume_stdout}\nstderr={resume_stderr}"
    );
    assert!(
        resume_stdout.contains("terminal=settled"),
        "the honest settled terminal: {resume_stdout}"
    );
    assert!(
        resume_stdout.contains(&format!("iterations={iterations}")),
        "zero iterations are burned: {resume_stdout}"
    );
    assert!(
        !resume_stdout.contains("budget_exhausted") && !resume_stderr.contains("budget_exhausted"),
        "recovery bookkeeping never reports a budget terminal: {resume_stdout} {resume_stderr}"
    );
    assert!(
        resume_stdout.contains("implemented-and-proven 3"),
        "the settled report stands: {resume_stdout}"
    );

    // The forensic records: one per damaged locator, each naming the
    // world, the locator, the missing row, and the anchor-bounded search —
    // never a false byte-presence claim (the artifacts ARE in the store).
    let forensics = records_containing(&world.evidence_dir(), "produced world:");
    assert_eq!(
        forensics.len(),
        3,
        "exactly one forensic record per damaged locator: {forensics:?}"
    );
    for locator in fixture_locators() {
        let record = forensics
            .iter()
            .find(|record| record.contains(locator.as_str()))
            .unwrap_or_else(|| panic!("the locator {locator} is named: {forensics:?}"));
        assert!(
            record.contains("is missing"),
            "the record names the missing row: {record}"
        );
        assert!(
            record.contains("no EvidenceProduced event at or after the resume anchor"),
            "the record states the anchor-bounded fact, never a global claim: {record}"
        );
        assert!(
            !record.contains("does not resolve in the evidence store"),
            "the bytes ARE in the store — no false byte claim: {record}"
        );
    }

    // The durable run record is untouched by the recovery bookkeeping.
    let after = world.run(&env, &["export", "--run", &run_id]);
    let stream_after = String::from_utf8_lossy(&after.stdout).into_owned();
    assert_eq!(
        stream_after, stream_before,
        "the forensic-only resume wrote nothing to the durable run record"
    );
}

#[test]
fn the_resume_of_a_settled_run_names_exactly_the_damaged_locator() {
    // The one-row case: the falsifier's "one intact locator masks a
    // sibling's missing row" shape. Per-locator granularity pins the
    // OPPOSITE: exactly ONE forensic record, naming exactly the damaged
    // locator; the intact locators are never masked, never mentioned.
    let world = World::new("settled-damage-one");
    let env = world.env("e2e-happy", "e2e-happy-seed");
    let run = world.run(&env, &["run", "seeded bounded task"]);
    assert!(
        run.status.success(),
        "the happy leg settles first: stdout={}\nstderr={}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    let run_id = run_id_of(&run);
    let locators = fixture_locators();
    let damaged = locators[0].clone();
    let deleted = delete_production_rows(
        &world.state_dir().join("gauntlet-state.db"),
        Some(damaged.as_str()),
    );
    assert_eq!(deleted, 1, "the damage deleted exactly the one row");

    let resume = world.run(&env, &["resume", &run_id]);
    let resume_stdout = String::from_utf8_lossy(&resume.stdout).into_owned();
    assert!(
        resume.status.success() && resume_stdout.contains("terminal=settled"),
        "the damaged settled run resumes honestly: {resume_stdout}"
    );
    let forensics = records_containing(&world.evidence_dir(), "produced world:");
    assert_eq!(
        forensics.len(),
        1,
        "exactly the damaged locator is named: {forensics:?}"
    );
    assert!(
        forensics[0].contains(&damaged),
        "the damaged locator is named: {}",
        forensics[0]
    );
    for intact in &locators[1..] {
        assert!(
            !forensics[0].contains(intact.as_str()),
            "the intact locator {intact} is never mentioned: {}",
            forensics[0]
        );
    }
}

// ---------------------------------------------------------------------------
// Leg 7b — COR-R4-1 (falsifier round 4's F-R4-3, the major): a recovery
// that COMPLETES a production on an otherwise-all-settled run closes as the
// honest settled no-op — terminal=settled, exit 0, ZERO additional
// iterations. The damage is the falsifier's exact compound reproduction:
// one production row deleted AND the close-out checkpoint's anchor
// regressed below the durable produce event (the A5 end-state — event
// durable above the anchor, row missing). The recovery half mints the row
// from the EVENT's own facts; before COR-R4-1 the loop half then burned a
// zero-dispatch iteration into `budget_exhausted` while every unit stood
// SETTLED. The forged-anchor precondition is the RECORDED "checkpoint()
// forged-row projection class (planning-layer)" deferral — this leg pins
// the DISTINCTION: recovery-completed damage ⇒ settled no-op; the offset
// forgery itself stays out of task20's scope, exercised here only as given
// damage through the state adapter's doc-hidden test-support instrument.
// ---------------------------------------------------------------------------

#[test]
fn the_resume_of_a_settled_run_with_a_recovery_completed_production_is_the_settled_noop() {
    let world = World::new("settled-damage-mint");
    let env = world.env("e2e-happy", "e2e-happy-seed");
    let run = world.run(&env, &["run", "seeded bounded task"]);
    assert!(
        run.status.success(),
        "the happy leg settles first: stdout={}\nstderr={}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    let run_stdout = String::from_utf8_lossy(&run.stdout).into_owned();
    let iterations = run_stdout
        .lines()
        .find(|line| line.starts_with("run: terminal=settled iterations="))
        .expect("the run settles")
        .rsplit('=')
        .next()
        .expect("the iteration count is printed")
        .to_owned();
    let run_id = run_id_of(&run);
    let state_db = world.state_dir().join("gauntlet-state.db");

    // THE DAMAGE: the falsifier's compound shape — one production row
    // deleted, and the close-out checkpoint's anchor regressed strictly
    // below that locator's durable produce event.
    let damaged = fixture_locators()[0].clone();
    let deleted = delete_production_rows(&state_db, Some(damaged.as_str()));
    assert_eq!(deleted, 1, "the damage deleted exactly the one row");
    let produced = evidence_produced_events(&state_db);
    let produce = produced
        .iter()
        .find(|(_, _, locator, _)| locator == damaged.as_str())
        .cloned()
        .expect("the damaged locator's produce event is durable");
    let (produce_offset, produce_ref, _, produce_at) = produce;
    let previous = regress_latest_checkpoint_offset(&state_db, &run_id, produce_offset - 1);
    assert!(
        previous > produce_offset - 1,
        "the forge regressed an anchor that stood above the produce event: {previous}"
    );
    // The durable-record baseline is taken AFTER the surgery, so the
    // before/after brackets exactly the resume (the export's checkpoint
    // segment carries each checkpoint's own event_log_offset, so the
    // surgery itself is stream-visible and must not leak into this
    // assertion).
    let before = world.run(&env, &["export", "--run", &run_id]);
    assert!(before.status.success(), "the settled run exports");
    let stream_before = String::from_utf8_lossy(&before.stdout).into_owned();

    // THE RESUME: exit 0, terminal=settled at the checkpoint's own
    // iteration, never a budget_exhausted lie, and NO forensic pins — the
    // mint SUCCEEDED, so nothing is unrecoverable.
    let resume = world.run(&env, &["resume", &run_id]);
    let resume_stdout = String::from_utf8_lossy(&resume.stdout).into_owned();
    let resume_stderr = String::from_utf8_lossy(&resume.stderr).into_owned();
    assert!(
        resume.status.success(),
        "the recovery-completed settled run resumes honestly: \
         stdout={resume_stdout}\nstderr={resume_stderr}"
    );
    assert!(
        resume_stdout.contains("terminal=settled"),
        "the honest settled terminal: {resume_stdout}"
    );
    assert!(
        resume_stdout.contains(&format!("iterations={iterations}")),
        "zero additional iterations: a fully settled projection books no round: {resume_stdout}"
    );
    assert!(
        !resume_stdout.contains("budget_exhausted") && !resume_stderr.contains("budget_exhausted"),
        "recovery bookkeeping never reports a budget terminal: {resume_stdout} {resume_stderr}"
    );
    assert!(
        resume_stdout.contains("implemented-and-proven 3"),
        "the settled report stands: {resume_stdout}"
    );
    let forensics = records_containing(&world.evidence_dir(), "produced world:");
    assert!(
        forensics.is_empty(),
        "a completed production is not forensic damage: {forensics:?}"
    );

    // THE MINT: the row is back, bound to the EVENT'S OWN identity — the
    // event's ref, the event's stored timestamp, the event's offset
    // (byte-level, the way the falsifier verified it) — and the durable
    // run record is untouched by the recovery.
    let rows = evidence_production_rows(&state_db);
    assert_eq!(
        rows.len(),
        3,
        "the re-mint restored the row set exactly: {rows:?}"
    );
    let minted = rows
        .iter()
        .find(|(reference, locator, _, _)| {
            locator == damaged.as_str() && reference == produce_ref.as_str()
        })
        .expect("the deleted row was re-minted from the durable event: {rows:?}");
    assert_eq!(
        minted.2, produce_at,
        "the re-minted row takes the event's stored timestamp, never the recovery's clock"
    );
    assert_eq!(
        minted.3, produce_offset as i64,
        "the re-minted row binds to the event's own offset"
    );
    let after = world.run(&env, &["export", "--run", &run_id]);
    let stream_after = String::from_utf8_lossy(&after.stdout).into_owned();
    assert_eq!(
        stream_after, stream_before,
        "the recovery-completed resume wrote nothing to the durable run record"
    );

    // The SECOND resume is the byte-identical settled no-op (the whole
    // scratch hashes the same: no dispatch, no checkpoint, no lock left).
    let file_set = |root: &Path| -> Vec<(String, u64)> {
        let mut entries: Vec<(String, u64)> = Vec::new();
        walk(root, root, &mut entries);
        entries.sort();
        entries
    };
    let snapshot = file_set(&world.root);
    let second = world.run(&env, &["resume", &run_id]);
    let second_stdout = String::from_utf8_lossy(&second.stdout).into_owned();
    assert!(
        second.status.success()
            && second_stdout.contains("terminal=settled")
            && second_stdout.contains(&format!("iterations={iterations}")),
        "the second resume is the honest settled no-op: {second_stdout}"
    );
    assert_eq!(
        file_set(&world.root),
        snapshot,
        "the second resume wrote nothing anywhere"
    );
}

// ---------------------------------------------------------------------------
// Leg 7c — COR-R5-1 (falsifier round 5's F-R5-1, the major): the SETTLEMENT
// arm of the recovery-completed shape closes as the honest settled no-op.
// The falsifier's exact reproduction through the real binary: one keyed
// settlement row deleted AND the close-out checkpoint's anchor regressed
// strictly below that unit's durable settlement event (the §16.3/A5
// end-state, reachable from an ordinary crash — no forgery needed for the
// natural shape; the anchor regression is the recorded "checkpoint()
// forged-row projection class (planning-layer)" deferral, exercised here as
// GIVEN damage exactly like the COR-R4-1 production twin above). Before
// COR-R5-1 the resume re-entered the loop on this all-settled projection,
// whose fresh round booking was REFUSED by the shipped budget posture (4
// rounds, 15% reserve) and terminalized `budget_exhausted` exit 1 BEFORE
// the all-closed settled close could fire — over a run whose every unit is
// SETTLED. Pinned here: exit 0, terminal=settled at the run's own
// iteration, zero re-dispatch, the replay re-inserting the row at the
// EVENT'S OWN identity (offset, timestamp, refs — byte-level), no forensic
// pins of either class (recovered damage is not damage), the export stream
// gaining exactly the one re-completed settlement record and nothing else,
// and the second resume a byte-identical zero-write no-op.
// ---------------------------------------------------------------------------

/// Out-of-band DAMAGE (COR-R5-1's E2E leg): deletes the durable `settlements`
/// rows out of the settled run's state database — or only the rows of `unit`
/// when one is given. The keyed table is write-once by design, so the
/// surgery goes through the state adapter's doc-hidden test-support
/// instrument (beside the production-row damage injection). Returns how many
/// rows the statement removed (callers ASSERT the count).
fn delete_settlement_rows(state_db: &Path, unit: Option<&str>) -> usize {
    gauntlet_adapter_state_postgres::test_support::delete_settlement_rows(state_db, unit)
        .expect("the settlement damage injection runs")
}

/// The durable `settlement_committed` events as
/// `(event-log offset, unit id, verification ref, evidence ref, stored
/// timestamp)` — the event-side facts the COR-R5-1 leg pins the re-inserted
/// settlement row against.
fn settlement_committed_events(
    state_db: &Path,
) -> Vec<gauntlet_adapter_state_postgres::test_support::SettlementEventFacts> {
    gauntlet_adapter_state_postgres::test_support::settlement_committed_events(state_db)
        .expect("the settlement event reader runs")
}

/// The durable settlement rows as
/// `(unit id, verification ref, evidence ref, event-log offset, settled_at)`
/// — the row-side facts the COR-R5-1 leg pins the re-completed settlement
/// against.
fn settlement_rows(
    state_db: &Path,
) -> Vec<gauntlet_adapter_state_postgres::test_support::SettlementRowFacts> {
    gauntlet_adapter_state_postgres::test_support::settlement_rows(state_db)
        .expect("the settlement row reader runs")
}

#[test]
fn the_resume_of_a_settled_run_with_a_recovery_completed_settlement_is_the_settled_noop() {
    let world = World::new("settled-damage-settle");
    let env = world.env("e2e-happy", "e2e-happy-seed");
    let run = world.run(&env, &["run", "seeded bounded task"]);
    assert!(
        run.status.success(),
        "the happy leg settles first: stdout={}\nstderr={}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    let run_stdout = String::from_utf8_lossy(&run.stdout).into_owned();
    let iterations = run_stdout
        .lines()
        .find(|line| line.starts_with("run: terminal=settled iterations="))
        .expect("the run settles")
        .rsplit('=')
        .next()
        .expect("the iteration count is printed")
        .to_owned();
    let run_id = run_id_of(&run);
    let state_db = world.state_dir().join("gauntlet-state.db");

    // THE DAMAGE: the falsifier's compound shape — one keyed settlement row
    // deleted (the first-settled unit's), and the close-out checkpoint's
    // anchor regressed strictly below that unit's settlement event.
    let events = settlement_committed_events(&state_db);
    assert_eq!(
        events.len(),
        3,
        "the settled run holds three settlement events: {events:?}"
    );
    let (settle_offset, settle_unit, settle_verification, settle_evidence, settle_at) = events
        .iter()
        .cloned()
        .min_by_key(|(offset, _, _, _, _)| *offset)
        .expect("the first settlement event is durable");
    let deleted = delete_settlement_rows(&state_db, Some(settle_unit.as_str()));
    assert_eq!(deleted, 1, "the damage deleted exactly the one row");
    let previous = regress_latest_checkpoint_offset(&state_db, &run_id, settle_offset - 1);
    assert!(
        previous > settle_offset - 1,
        "the forge regressed an anchor that stood above the settlement event: {previous}"
    );
    // The durable-record baseline is taken AFTER the surgery, so the
    // before/after brackets exactly the resume (the COR-R4-1 leg's
    // bracketing rule: the anchor surgery is stream-visible and must not
    // leak into the delta assertion).
    let before = world.run(&env, &["export", "--run", &run_id]);
    assert!(before.status.success(), "the settled run exports");
    let stream_before = String::from_utf8_lossy(&before.stdout).into_owned();

    // THE RESUME: exit 0, terminal=settled at the run's own iteration,
    // never a budget_exhausted lie — an all-settled projection closes
    // BEFORE (and regardless of) iteration booking.
    let resume = world.run(&env, &["resume", &run_id]);
    let resume_stdout = String::from_utf8_lossy(&resume.stdout).into_owned();
    let resume_stderr = String::from_utf8_lossy(&resume.stderr).into_owned();
    assert!(
        resume.status.success(),
        "the recovery-completed settled run resumes honestly: \
         stdout={resume_stdout}\nstderr={resume_stderr}"
    );
    assert!(
        resume_stdout.contains("terminal=settled"),
        "the honest settled terminal: {resume_stdout}"
    );
    assert!(
        resume_stdout.contains(&format!("iterations={iterations}")),
        "zero booked rounds: a fully settled projection closes before/regardless of booking: \
         {resume_stdout}"
    );
    assert!(
        !resume_stdout.contains("budget_exhausted") && !resume_stderr.contains("budget_exhausted"),
        "recovery bookkeeping never reports a budget terminal: {resume_stdout} {resume_stderr}"
    );
    assert!(
        resume_stdout.contains("implemented-and-proven 3"),
        "the settled report stands: {resume_stdout}"
    );

    // NO forensic pins of either class: the settlement replay COMPLETED the
    // row, and the production half had nothing to complete — recovered
    // damage is not damage.
    let forensics = records_containing(&world.evidence_dir(), "produced world:");
    assert!(
        forensics.is_empty(),
        "the production half completed nothing, so it pins nothing: {forensics:?}"
    );
    let settle_pins = records_containing(&world.evidence_dir(), "settlement_row_unrecoverable");
    assert!(
        settle_pins.is_empty(),
        "a recovery-completed settlement is not forensic damage: {settle_pins:?}"
    );

    // THE REPLAY: the row is back, bound to the EVENT'S OWN identity — the
    // event's offset, the event's stored timestamp, the event's own refs
    // (byte-level, the way the falsifier verified it).
    let rows = settlement_rows(&state_db);
    assert_eq!(
        rows.len(),
        3,
        "the replay restored the settlement row set exactly: {rows:?}"
    );
    let restored = rows
        .iter()
        .find(|(unit, verification, _, _, _)| {
            unit.as_str() == settle_unit.as_str()
                && verification.as_str() == settle_verification.as_str()
        })
        .expect("the deleted row was re-completed from the durable event: {rows:?}");
    assert_eq!(
        restored.2, settle_evidence,
        "the re-completed row names the event's own evidence ref"
    );
    assert_eq!(
        restored.3, settle_offset as i64,
        "the re-completed row binds to the event's own offset"
    );
    assert_eq!(
        restored.4, settle_at,
        "the re-completed row takes the event's stored timestamp, never the recovery's clock"
    );

    // THE DURABLE RECORD: the export stream gains EXACTLY the one
    // re-completed settlement record — no new events (zero booked rounds),
    // no new checkpoints, no re-dispatched invocations, nothing removed.
    let after = world.run(&env, &["export", "--run", &run_id]);
    let stream_after = String::from_utf8_lossy(&after.stdout).into_owned();
    let lines_of = |stream: &str| -> Vec<String> {
        let mut lines: Vec<String> = stream.lines().map(str::to_owned).collect();
        lines.sort();
        lines
    };
    let lines_before = lines_of(&stream_before);
    let lines_after = lines_of(&stream_after);
    let removed: Vec<&String> = lines_before
        .iter()
        .filter(|line| !lines_after.contains(line))
        .collect();
    let added: Vec<&String> = lines_after
        .iter()
        .filter(|line| !lines_before.contains(line))
        .collect();
    assert!(
        removed.is_empty(),
        "the resume removed nothing from the durable record: {removed:?}"
    );
    assert_eq!(
        added.len(),
        1,
        "the resume added exactly the re-completed settlement record: {added:?}"
    );
    assert!(
        added[0].contains("\"record\":\"settlement\"") && added[0].contains(settle_unit.as_str()),
        "the added record is the completed settlement of the damaged unit: {}",
        added[0]
    );
    let of_kind = |stream: &str, kind: &str| -> Vec<String> {
        let needle = format!("\"record\":\"{kind}\"");
        let mut lines: Vec<String> = stream
            .lines()
            .filter(|line| line.contains(&needle))
            .map(str::to_owned)
            .collect();
        lines.sort();
        lines
    };
    assert_eq!(
        of_kind(&stream_after, "event"),
        of_kind(&stream_before, "event"),
        "zero booked rounds: the event log is untouched by the settled close"
    );
    assert_eq!(
        of_kind(&stream_after, "checkpoint"),
        of_kind(&stream_before, "checkpoint"),
        "no reconciliation checkpoint: the replay's row is durable by itself"
    );
    assert_eq!(
        of_kind(&stream_after, "role_invocation"),
        of_kind(&stream_before, "role_invocation"),
        "zero re-dispatch: no invocation was re-admitted"
    );

    // The SECOND resume is the byte-identical settled no-op (the replay is
    // idempotent over the restored row; no dispatch, no checkpoint, no pin,
    // no lock left).
    let file_set = |root: &Path| -> Vec<(String, u64)> {
        let mut entries: Vec<(String, u64)> = Vec::new();
        walk(root, root, &mut entries);
        entries.sort();
        entries
    };
    let snapshot = file_set(&world.root);
    let second = world.run(&env, &["resume", &run_id]);
    let second_stdout = String::from_utf8_lossy(&second.stdout).into_owned();
    assert!(
        second.status.success()
            && second_stdout.contains("terminal=settled")
            && second_stdout.contains(&format!("iterations={iterations}")),
        "the second resume is the honest settled no-op: {second_stdout}"
    );
    assert_eq!(
        file_set(&world.root),
        snapshot,
        "the second resume wrote nothing anywhere"
    );
}

// ---------------------------------------------------------------------------
// Leg 7d — COR-R5-2 (falsifier round 5's F-R5-2, the minor): below-anchor
// settlement-row damage on a certified settled no-op is PINNED per row, the
// same honest forensics the production arm emits for the identical shape
// (shipped legs 7/7a). The damage is the falsifier's NATURAL cell-D2
// end-state: one keyed settlement row deleted while the close-out anchor
// stays intact — the damaged unit's settlement event sits BELOW the anchor
// (an earlier iteration settled it), so the anchor-bounded §18.3 replay
// cannot re-complete it. Nothing false is asserted by the replay and the
// event is the durable truth, so the run still closes as the settled no-op —
// but the divergence is now a named, per-row forensic record (checked
// against the store's own settlement rows), never silence; the intact
// siblings are never named, and the durable run record is untouched by the
// recovery bookkeeping. Over-rejection guard: the no-damage resumes pin
// nothing (the shipped first-resume no-op leg and leg 7c's second resume
// pin the zero-write shape).
// ---------------------------------------------------------------------------

#[test]
fn the_settled_noop_over_below_anchor_settlement_damage_pins_the_missing_row() {
    let world = World::new("settled-damage-settle-pin");
    let env = world.env("e2e-happy", "e2e-happy-seed");
    let run = world.run(&env, &["run", "seeded bounded task"]);
    assert!(
        run.status.success(),
        "the happy leg settles first: stdout={}\nstderr={}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    let run_stdout = String::from_utf8_lossy(&run.stdout).into_owned();
    let iterations = run_stdout
        .lines()
        .find(|line| line.starts_with("run: terminal=settled iterations="))
        .expect("the run settles")
        .rsplit('=')
        .next()
        .expect("the iteration count is printed")
        .to_owned();
    let run_id = run_id_of(&run);
    let state_db = world.state_dir().join("gauntlet-state.db");

    // THE DAMAGE: the first-settled unit's keyed settlement row deleted;
    // its settlement event sits below the INTACT close-out anchor, so the
    // anchored replay cannot re-complete it. The baseline export is taken
    // after the surgery, bracketing exactly the resume.
    let events = settlement_committed_events(&state_db);
    assert_eq!(
        events.len(),
        3,
        "the settled run holds three settlement events: {events:?}"
    );
    let (_, settle_unit, _, _, _) = events
        .iter()
        .cloned()
        .min_by_key(|(offset, _, _, _, _)| *offset)
        .expect("the first settlement event is durable");
    let deleted = delete_settlement_rows(&state_db, Some(settle_unit.as_str()));
    assert_eq!(deleted, 1, "the damage deleted exactly the one row");
    let before = world.run(&env, &["export", "--run", &run_id]);
    assert!(before.status.success(), "the settled run exports");
    let stream_before = String::from_utf8_lossy(&before.stdout).into_owned();

    // THE RESUME: the certified settled no-op, WITH the honest pin.
    let resume = world.run(&env, &["resume", &run_id]);
    let resume_stdout = String::from_utf8_lossy(&resume.stdout).into_owned();
    let resume_stderr = String::from_utf8_lossy(&resume.stderr).into_owned();
    assert!(
        resume.status.success() && resume_stdout.contains("terminal=settled"),
        "the damaged settled run still resumes as the settled no-op (the durable event is \
         the truth): stdout={resume_stdout}\nstderr={resume_stderr}"
    );
    assert!(
        resume_stdout.contains(&format!("iterations={iterations}")),
        "the forensic-only resume books no round: {resume_stdout}"
    );
    assert!(
        !resume_stdout.contains("budget_exhausted") && !resume_stderr.contains("budget_exhausted"),
        "recovery bookkeeping never reports a budget terminal: {resume_stdout} {resume_stderr}"
    );
    // The report HONESTLY reflects the keyed divergence (the proven count
    // reads the keyed settlement surface; the terminal decision reads the
    // event-ordered projection) — the missing row's unit reports
    // provisional, and the forensic pin names exactly that divergence. The
    // contrast with leg 7c is the commission: recovered keyed damage
    // reports 3 proven; unrecoverable keyed damage reports 2 + provisional
    // and is PINNED, never silent and never lied about.
    assert!(
        resume_stdout.contains("implemented-and-proven 2")
            && resume_stdout.contains("provisional 1"),
        "the report honestly reflects the missing keyed row: {resume_stdout}"
    );

    // THE PIN: exactly one settlement forensic record, naming the damaged
    // unit, the anchor-bounded fact, and the checked-against-the-store
    // posture; the intact siblings are never named.
    let forensics = records_containing(&world.evidence_dir(), "settlement_row_unrecoverable");
    assert_eq!(
        forensics.len(),
        1,
        "exactly the damaged row is pinned: {forensics:?}"
    );
    let record = &forensics[0];
    assert!(
        record.contains(settle_unit.as_str()),
        "the missing row's unit is named: {record}"
    );
    for (_, intact_unit, _, _, _) in &events {
        if intact_unit == &settle_unit {
            continue;
        }
        assert!(
            !record.contains(intact_unit.as_str()),
            "the intact sibling {intact_unit} is never named: {record}"
        );
    }
    assert!(
        record.contains("anchor-bounded"),
        "the record states the anchor-bounded replay fact: {record}"
    );
    assert!(
        record.contains("checked against the store's settlement rows"),
        "the record names what was checked: {record}"
    );

    // THE HONEST REMAINDER: the row stays missing (the replay cannot
    // fabricate it), the production arm pinned nothing, and the durable
    // run record is byte-identical — the forensics live in the evidence
    // store, never in the run's own record.
    let rows = settlement_rows(&state_db);
    assert_eq!(
        rows.len(),
        2,
        "the anchored replay cannot re-complete the below-anchor row: {rows:?}"
    );
    let production_pins = records_containing(&world.evidence_dir(), "produced world:");
    assert!(
        production_pins.is_empty(),
        "the production half is undamaged, so it pins nothing: {production_pins:?}"
    );
    let after = world.run(&env, &["export", "--run", &run_id]);
    let stream_after = String::from_utf8_lossy(&after.stdout).into_owned();
    assert_eq!(
        stream_after, stream_before,
        "the forensic-only resume wrote nothing to the durable run record"
    );
}

// ---------------------------------------------------------------------------
// Leg 8 — the state-dir single-writer guard (COR-R3-3, falsifier round 3's
// M-R3-1 concurrent double-resume attack through the real binary). Before
// the guard two concurrent resume drivers were BOTH admitted over one
// durable run. Now: a held writer lock refuses a second driver typed (naming
// the holder pid and the manual stale-lock recovery), the honest manual
// recovery path (operator removes the stale file) resumes clean, and the
// executed concurrent attack is bounded — every non-zero exit is the typed
// refusal, the settled no-ops write nothing, and the durable record stands
// byte-identical.
// ---------------------------------------------------------------------------

#[test]
fn the_single_writer_guard_bounds_the_concurrent_double_resume_attack() {
    let world = World::new("single-writer");
    let env = world.env("e2e-happy", "e2e-happy-seed");
    let run = world.run(&env, &["run", "seeded bounded task"]);
    assert!(
        run.status.success(),
        "the happy leg settles first: stdout={}\nstderr={}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    let run_id = run_id_of(&run);
    let before = world.run(&env, &["export", "--run", &run_id]);
    assert!(before.status.success(), "the settled run exports");
    let stream_before = String::from_utf8_lossy(&before.stdout).into_owned();
    let file_set = |root: &Path| -> Vec<(String, u64)> {
        let mut entries: Vec<(String, u64)> = Vec::new();
        walk(root, root, &mut entries);
        entries.sort();
        entries
    };
    let snapshot = file_set(&world.root);

    // (a) A HELD lock refuses a second driver typed. The test process is
    // the (live, unrelated) holder; the refusal names its pid and the
    // manual recovery — fail-closed, no liveness guessing, no takeover.
    // COR-R4-3 mechanical adaptation: the refusal's wording is now the
    // hedged "stale or unreadable writer lock" (the guard never asserts a
    // live holder — pid reuse is always plausible when liveness is never
    // probed), so the pinned token tracks the message. The assertion's
    // strength is unchanged: exit non-zero, typed refusal, pid named,
    // manual recovery + no-takeover named.
    let lock_path = world.state_dir().join("writer.lock");
    std::fs::write(&lock_path, format!("{}\n", std::process::id()))
        .expect("the held lock is planted");
    let refused = world.run(&env, &["resume", &run_id]);
    let refused_stderr = String::from_utf8_lossy(&refused.stderr).into_owned();
    assert!(
        !refused.status.success(),
        "a second driver is refused while a writer holds the state dir"
    );
    assert!(
        refused_stderr.contains("stale or unreadable writer lock"),
        "the refusal is typed: {refused_stderr}"
    );
    assert!(
        refused_stderr.contains(&std::process::id().to_string()),
        "the refusal names the holder pid: {refused_stderr}"
    );
    assert!(
        refused_stderr.contains("remove the lock file manually")
            && refused_stderr.contains("never takes over automatically"),
        "the refusal names the manual recovery and the no-takeover posture: {refused_stderr}"
    );

    // (b) The honest manual recovery: the operator removes the stale file,
    // and the resume is the clean settled no-op again.
    std::fs::remove_file(&lock_path).expect("the operator's manual removal");
    let recovered = world.run(&env, &["resume", &run_id]);
    let recovered_stdout = String::from_utf8_lossy(&recovered.stdout).into_owned();
    assert!(
        recovered.status.success() && recovered_stdout.contains("terminal=settled"),
        "after the manual recovery the resume is the honest settled no-op: \
         {recovered_stdout}",
    );

    // (c) The EXECUTED attack: two concurrent resume drivers race on the
    // same run. Every non-zero exit must be the typed writer-lock refusal,
    // and at least one driver is admitted (the O_EXCL create decides —
    // exactly one holder at a time).
    let spawn_resume = || {
        let mut command = Command::new(GAUNTLET);
        command.current_dir(world.target());
        for (key, value) in &env {
            command.env(key, value);
        }
        command
            .args(["resume", &run_id])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("the resume driver spawns")
    };
    let first = spawn_resume();
    let second = spawn_resume();
    let first_out = first.wait_with_output().expect("the first driver runs");
    let second_out = second.wait_with_output().expect("the second driver runs");
    let shaped = |outcome: &Output| {
        let stderr = String::from_utf8_lossy(&outcome.stderr).into_owned();
        // COR-R4-3 mechanical adaptation: the typed refusal's stable token
        // is the hedged "stale or unreadable writer lock" wording.
        outcome.status.success() || stderr.contains("stale or unreadable writer lock")
    };
    assert!(
        shaped(&first_out) && shaped(&second_out),
        "every exit is either an honest settled no-op or the typed refusal: {first_out:?} \
         {second_out:?}"
    );
    assert!(
        first_out.status.success() || second_out.status.success(),
        "the O_EXCL admission admits at least one driver: no deadlock, no double refusal"
    );

    // The durable record stands: settled no-ops write nothing, so the
    // export stream and the whole scratch are byte-identical to before the
    // attack (no double dispatch, no corruption, no leftover lock files).
    let after = world.run(&env, &["export", "--run", &run_id]);
    let stream_after = String::from_utf8_lossy(&after.stdout).into_owned();
    assert_eq!(
        stream_after, stream_before,
        "the concurrent attack moved no durable fact"
    );
    let after_set = file_set(&world.root);
    assert_eq!(
        snapshot, after_set,
        "the attack wrote nothing anywhere: every writer lock was released"
    );
}
