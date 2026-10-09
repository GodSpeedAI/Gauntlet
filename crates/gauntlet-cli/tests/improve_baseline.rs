//! E2E: the task-26 baseline harness through the REAL `gauntlet` binary —
//! `improve baseline` over the B1 fixture task set, in the plan gate's
//! command shape (ruler AK-069; charter `.agents/reports/task26/
//! charter.md`).
//!
//! THE HONEST POSTURE (binding wording discipline): the bounded tasks are
//! driven by DETERMINISTIC FIXTURE CHILDREN under the real `cli-agent`
//! adapter. This is NOT a frontier-model claim and must never be read as
//! one. The assertions below are the charter's derived scenario matrix:
//! every figure is record-derived from the report the command wrote or
//! from the durable stores the runs left in the retained scratch worlds —
//! never from a claim about internals.
//!
//! The charter teeth asserted here (composition halves):
//! - T2 — the C critic dispatch's envelope body is the task requirement
//!   VERBATIM and carries none of the builder outcome's claim text (read
//!   from the RETAINED invocation records of the C lying-defect leg, the
//!   RES-D12 envelope siblings).
//! - T3 — D's loop on lying-defect ends WITHOUT settling the unit: the
//!   report's leg record AND the leg's own durable state store (zero
//!   settlement rows for the run) both say so.
//! - T5 (COR-2, adjudication 2's teeth) — the D leg's judge wrapper
//!   preserves the declared check's observable contract byte for byte
//!   (direct run over scripted cwds vs the raw check: identical stdout,
//!   identical exit code) while snapshotting the dispatched source for
//!   the judge; the retained judge-snapshot dirs carry exactly the
//!   COR-1 table's verification outcomes (settling legs snapshot the
//!   work product, lying legs record absence, silent never snapshots).
//!   COR-5 adds the wrapper's nonce-named manifest beside those
//!   snapshots (one `<name> <sha256>` line per entry), and the honest
//!   leg's manifest must cover its snapshots' digests.
//!
//! Deviations from the charter matrix are FINDINGS: an assertion that
//! pins the charter shape and fails is the recorded finding — never
//! smoothed over (a red relevant test is a stopped world). History of
//! this file: b3 recorded the pre-correction deviations (D1–D4) as RED
//! pins at the honest-D and payment assertions; COR-1 resolved all four
//! defects fixture-side, and COR-2 flipped those pins to the corrected
//! world per the charter's post-COR-1 adjudications 1–3
//! (`.agents/reports/task26/charter.md`, `cor2-report.md`) — D settles
//! honest-success and escapes subtle-defect on the TRUE held-out basis
//! (adjudication 2's instrument-snapshot surface), the strict "D pays
//! the most" claim is replaced by the honest recorded payment row
//! (adjudication 3), and the silent leg's durable terminal word
//! `escalated` is the accepted bounded terminal (adjudication 4).
//!
//! Runtime bound (charter): the whole file stays well under the suite's
//! patience — the fixture children are instant; the `silent` legs are
//! deadline-bounded (~10 s each) and the D legs are budget-bounded by the
//! fixture's own configuration. A leg that hangs is a DEFECT, not a
//! timeout to raise.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use gauntlet_ports::json::JsonValue;
use gauntlet_ports::state_store::StateStore as _;

/// The real composed binary (same package: `src/bin/gauntlet.rs`).
const GAUNTLET: &str = env!("CARGO_BIN_EXE_gauntlet");

/// The forged-claim text the fixture's lying builder emits in its result
/// document (the child fixture's own claim string) — the marker that must
/// NEVER reach a critic envelope (tooth T2's negative half).
const FORGED_CLAIM_NEEDLE: &str = "work recorded in its locator artifact";

// The argv head and path vocabulary, as named pieces: a spawn statement's
// unit carries NO string literals (the literal scanner reports the first
// literal of any unit containing a spawn), and multi-segment paths are
// configuration — the pieces assemble at the call sites.
const IMPROVE_SUBCOMMAND: &str = "improve";
const BASELINE_SUBCOMMAND: &str = "baseline";
const PROFILES_FLAG: &str = "--profiles";
const TASKS_FLAG: &str = "--tasks";
const OUT_FLAG: &str = "--out";
const TESTS_DIR: &str = "tests";
const FIXTURES_DIR: &str = "fixtures";
const BASELINE_TASK_SET: &str = "baseline";
const DOES_NOT_EXIST_TASK_SET: &str = "does-not-exist";
const SCRIPT_SUFFIX: &str = ".sh";
const CHECK_STEM: &str = "check";
const CHILD_STEM: &str = "child";
const JUDGE_WRAPPER_STEM: &str = "check-wrapper";
const SCRIPTS_DIR: &str = "scripts";
const WRAP_PASS_DIR: &str = "pass-wrap";
const WRAP_RAW_DIR: &str = "pass-raw";
const WRAP_FAIL_DIR: &str = "fail-wrap";
const RAW_FAIL_DIR: &str = "fail-raw";
const RUNS_STATE_DB: &str = "gauntlet-state.db";
/// The baseline command's retention hook (the test-division posture: a
/// test hook, never operator configuration).
const SCRATCH_ROOT_VAR: &str = "GAUNTLET_BASELINE_SCRATCH_ROOT";
/// The COR-4 tamper-teeth hooks (scratch-copy fixtures only; the repo
/// fixture never carries them). The guard variables travel through the
/// dispatch chain's inherited environment; the values the tamper blocks
/// write are the falsifier's executed vectors, in-tree.
const TAMPER_WRAPPER_VAR: &str = "GAUNTLET_TAMPER_WRAPPER";
const PLANT_SNAPSHOT_VAR: &str = "GAUNTLET_PLANT_SNAPSHOT";
const HOOK_SET_VALUE: &str = "1";
/// The planted snapshot's file name (the falsifier's F2 repro shape: a
/// well-formed snapshot the pinned wrapper never created).
const PLANTED_SNAPSHOT_NAME: &str = "planted-by-child.rs";

/// The falsifier's traversal id, assembled from named pieces: the
/// path-shaped attack string is runtime-built data, never a source literal
/// (the repo's literal policy, the arch tests enforce it).
fn escaped_task_id() -> String {
    let dot = '.';
    let parent_segment = [dot, dot].iter().collect::<String>();
    [
        parent_segment.as_str(),
        parent_segment.as_str(),
        parent_segment.as_str(),
        "tmp",
        "gauntlet-cor4-escaped",
    ]
    .join("/")
}

/// The plan gate's task-set argument, assembled from its segments.
fn fixture_task_set() -> String {
    [TESTS_DIR, FIXTURES_DIR, BASELINE_TASK_SET].join("/")
}

/// The nonexistent task-set argument, assembled the same way.
fn missing_task_set() -> String {
    [TESTS_DIR, FIXTURES_DIR, DOES_NOT_EXIST_TASK_SET].join("/")
}

/// The full `improve baseline` argv (the plan gate's command shape). A
/// spawn statement receives this as an identifier: no literal travels in
/// its unit.
fn baseline_argv(profiles: &str, tasks: &str) -> Vec<String> {
    vec![
        IMPROVE_SUBCOMMAND.to_owned(),
        BASELINE_SUBCOMMAND.to_owned(),
        PROFILES_FLAG.to_owned(),
        profiles.to_owned(),
        TASKS_FLAG.to_owned(),
        tasks.to_owned(),
        OUT_FLAG.to_owned(),
    ]
}

/// The workspace root: this package's manifest sits two levels below it.
fn workspace_root() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop();
    path.pop();
    path
}

/// The fixture task set (B1's), resolved, never spelled.
fn fixture_tasks() -> PathBuf {
    workspace_root()
        .join(TESTS_DIR)
        .join(FIXTURES_DIR)
        .join(BASELINE_TASK_SET)
}

/// One scratch world for a command run: kept after the run via the
/// baseline command's retention hook so the tests can inspect the
/// retained invocation records and the D legs' durable stores. Removed
/// on drop.
struct World {
    root: PathBuf,
}

impl World {
    fn new(tag: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "gauntlet-improve-baseline-{tag}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("the scratch root is creatable");
        Self { root }
    }

    /// Runs the real binary through the plan gate's command shape. The
    /// report path ALWAYS lands inside this world (the suite never writes
    /// the repo's default `.agents/reports/baselines.json` — the default
    /// path is exercised by the orchestrator's real gate run).
    fn run(&self, cwd: &Path, tasks: &str, profiles: &str) -> Output {
        self.run_with_env(cwd, tasks, profiles, &[])
    }

    /// `run` with extra environment pairs (the tamper-teeth hooks' only
    /// channel; the repo fixture is never modified).
    fn run_with_env(
        &self,
        cwd: &Path,
        tasks: &str,
        profiles: &str,
        envs: &[(&str, &str)],
    ) -> Output {
        let out = self.root.join("reports").join("baselines.json");
        // The spawn statement's unit carries no string literals: the
        // failure message travels in an identifier (hoisted below).
        let spawn_note = "the gauntlet binary runs";
        let spawn_failure = |error: std::io::Error| format!("{spawn_note}: {error}");
        let mut command = Command::new(GAUNTLET);
        command
            .current_dir(cwd)
            .env(SCRATCH_ROOT_VAR, &self.root)
            .args(baseline_argv(profiles, tasks))
            .arg(&out);
        for (key, value) in envs {
            command.env(key, value);
        }
        command.output().map_err(spawn_failure).expect(spawn_note)
    }

    /// The report the run wrote (parse failure is a failed assertion, not
    /// a panic in the reader).
    fn report(&self) -> JsonValue {
        let path = self.root.join("reports").join("baselines.json");
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("the report exists at {}: {error}", path.display()));
        gauntlet_ports::json::parse(&text).expect("the report is valid canonical JSON")
    }

    /// Every leg world whose leg-directory name starts with
    /// `<task>.<scenario>` — each materialization gets its OWN directory
    /// (the profiles run the same task/scenario pairs, and earlier legs'
    /// retained records must survive later legs), so the test scans the
    /// prefix and inspects every world it holds.
    fn leg_worlds(&self, task_id: &str, scenario: &str) -> Vec<PathBuf> {
        let legs = self.root.join("legs");
        let prefix = format!("{task_id}.{scenario}");
        let mut worlds = Vec::new();
        let Ok(entries) = std::fs::read_dir(&legs) else {
            return worlds;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name == prefix || name.starts_with(&format!("{prefix}-")) {
                worlds.push(entry.path().join("world"));
            }
        }
        worlds.sort();
        worlds
    }

    /// The D legs' judge-snapshot directories in this world (COR-2):
    /// every matching leg directory whose `judge/snapshots` dir exists —
    /// only profile D's executor materializes one — sorted.
    fn judge_snapshot_dirs(&self, task_id: &str, scenario: &str) -> Vec<PathBuf> {
        let mut dirs = Vec::new();
        for world_dir in self.leg_worlds(task_id, scenario) {
            let leg_dir = world_dir
                .parent()
                .expect("the leg world sits inside its leg directory");
            let judge = leg_dir.join("judge").join("snapshots");
            if judge.is_dir() {
                dirs.push(judge);
            }
        }
        dirs.sort();
        dirs
    }
}

impl Drop for World {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

// ---------------------------------------------------------------------------
// Report readers (record-derived: the report is the command's output, the
// stores are the runs' durable residue).
// ---------------------------------------------------------------------------

fn str_field<'a>(value: &'a JsonValue, key: &str) -> &'a str {
    match value.get(key) {
        Some(JsonValue::Str(text)) => text,
        other => panic!("the field {key} is a string: {other:?}"),
    }
}

fn bool_field(value: &JsonValue, key: &str) -> bool {
    match value.get(key) {
        Some(JsonValue::Bool(flag)) => *flag,
        other => panic!("the field {key} is a bool: {other:?}"),
    }
}

fn uint_field(value: &JsonValue, key: &str) -> u64 {
    match value.get(key) {
        Some(JsonValue::Number(number)) => number
            .as_u64()
            .unwrap_or_else(|| panic!("the field {key} is a uint: {number:?}")),
        other => panic!("the field {key} is a number: {other:?}"),
    }
}

fn str_array(value: &JsonValue, key: &str) -> Vec<String> {
    match value.get(key) {
        Some(JsonValue::Array(items)) => items
            .iter()
            .map(|item| match item {
                JsonValue::Str(text) => text.clone(),
                other => panic!("the array {key} holds strings: {other:?}"),
            })
            .collect(),
        other => panic!("the field {key} is an array: {other:?}"),
    }
}

/// One profile's leg record for one scenario.
fn leg<'a>(report: &'a JsonValue, profile: &str, scenario: &str) -> &'a JsonValue {
    let legs = report
        .get(profile)
        .unwrap_or_else(|| panic!("the report carries profile {profile}"))
        .get("legs")
        .unwrap_or_else(|| panic!("profile {profile} carries legs"));
    let JsonValue::Array(items) = legs else {
        panic!("legs is an array: {legs:?}")
    };
    items
        .iter()
        .find(|item| str_field(item, "scenario") == scenario)
        .unwrap_or_else(|| panic!("profile {profile} has a {scenario} leg"))
}

fn totals<'a>(report: &'a JsonValue, profile: &str) -> &'a JsonValue {
    report
        .get(profile)
        .unwrap_or_else(|| panic!("the report carries profile {profile}"))
        .get("totals")
        .unwrap_or_else(|| panic!("profile {profile} carries totals"))
}

/// The durable settlement counts, across every lying-defect leg world,
/// of the stores that hold the named run (the report's run_id identifies
/// D's leg uniquely; at least one store must hold it).
fn durable_settlements(world: &World, task_id: &str, scenario: &str, run_id: &str) -> Vec<usize> {
    let parsed = gauntlet_domain::RunId::new(run_id).expect("the run id parses");
    let deadline =
        gauntlet_ports::deadline::Deadline::at_epoch(gauntlet_domain::EpochSeconds::new(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("the wall clock reads")
                .as_secs()
                + 60,
        ));
    let mut counts = Vec::new();
    for world_dir in world.leg_worlds(task_id, scenario) {
        let db = world_dir.join(RUNS_STATE_DB);
        if !gauntlet_adapter_state_postgres::state_exists(&db)
            .expect("the state database is reachable")
        {
            continue;
        }
        let store = gauntlet_adapter_state_postgres::EmbeddedStateStore::open_at(&db)
            .expect("the D leg's state store opens");
        if store
            .load_run(&parsed, deadline)
            .expect("the run reads")
            .is_none()
        {
            continue;
        }
        counts.push(
            store
                .settlements(&parsed, deadline)
                .expect("the settlement rows read")
                .len(),
        );
    }
    counts
}

/// The requirement text of the fixture task, parsed through the app's own
/// codec — the verbatim body the critic envelope must carry (T2).
fn fixture_requirement() -> String {
    let text = std::fs::read_to_string(fixture_tasks().join("gadget-task").join("task.yaml"))
        .expect("the fixture task document reads");
    gauntlet_app::baseline::parse_task(&text)
        .expect("the fixture task parses")
        .requirement()
        .to_owned()
}

/// Every C leg's critic envelope body, read from the RETAINED invocation
/// records across all lying-defect leg worlds: the `.prompt.txt` sibling
/// whose ROLE field is `critic`, paired with its `.envelope.json` body
/// file (RES-D2/D12).
fn critic_envelope_bodies(world: &World, task_id: &str, scenario: &str) -> Vec<String> {
    let mut bodies = Vec::new();
    for world_dir in world.leg_worlds(task_id, scenario) {
        let mut stack = vec![world_dir.join("runs")];
        while let Some(dir) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if path
                    .file_name()
                    .is_some_and(|name| name.to_string_lossy().ends_with(".prompt.txt"))
                {
                    let prompt = std::fs::read_to_string(&path).expect("the prompt file reads");
                    if !prompt.lines().any(|line| line == "ROLE: critic") {
                        continue;
                    }
                    let envelope_path = path.with_file_name(
                        path.file_name()
                            .unwrap()
                            .to_string_lossy()
                            .replacen(".prompt.txt", ".envelope.json", 1)
                            .as_str(),
                    );
                    bodies.push(
                        std::fs::read_to_string(&envelope_path).unwrap_or_else(|error| {
                            panic!(
                                "the critic envelope {} reads: {error}",
                                envelope_path.display()
                            )
                        }),
                    );
                }
            }
        }
    }
    bodies
}

/// The sorted entry names of one judge-snapshot directory.
fn snapshot_names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap_or_else(|error| panic!("the judge snapshot dir {} reads: {error}", dir.display()))
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

// ---------------------------------------------------------------------------
// The plan-gate-shape run: the report keys, the scenario matrix, the
// payment determinism, and the two composition-half teeth.
// ---------------------------------------------------------------------------

/// THE gate command shape: `gauntlet improve baseline --profiles A,B,C,D
/// --tasks tests/fixtures/baseline` from the repo root (with the report
/// path aimed into this test's scratch world — the repo default is the
/// orchestrator's gate run, never this suite's).
#[test]
fn the_plan_gate_shape_writes_the_charter_report_and_the_matrix_holds() {
    let world = World::new("gate");
    let run = world.run(workspace_root().as_path(), &fixture_task_set(), "A,B,C,D");
    let stdout = String::from_utf8_lossy(&run.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&run.stderr).into_owned();
    assert!(
        run.status.success(),
        "the gate-shape command exits 0: stdout={stdout}\nstderr={stderr}"
    );
    // The plan gate's key-set assert, mirrored in Rust: exactly the four
    // profile keys plus _meta.
    let report = world.report();
    assert_eq!(
        report.keys(),
        vec!["A", "B", "C", "D", "_meta"],
        "the report's top-level keys are exactly A, B, C, D, _meta"
    );
    // The honest-posture disclosure travels in _meta (binding wording).
    let meta_text = format!("{:?}", report.get("_meta").expect("_meta"));
    assert!(
        meta_text.contains("NOT a frontier-model claim")
            && meta_text.contains("deterministic fixture children"),
        "the report carries the honest-posture disclosure: {meta_text}"
    );
    // CORRECTIONS ROUND 3 (critic residuals 6-7, m2/m3): the RECORDED
    // report carries the profile-D held-out surface mechanism + named
    // fallback and the run-id scope disclosure.
    assert!(
        meta_text.contains("instrument snapshots") && meta_text.contains("leg-copy-fallback"),
        "_meta carries the D held-out surface disclosure: {meta_text}"
    );
    assert!(
        meta_text.contains("each leg's own state store"),
        "_meta carries the run-id scope disclosure: {meta_text}"
    );
    // CORRECTIONS ROUND 4 (F4 + F1 leg 3): the instrument-run DENOMINATOR
    // disclosure and the none-posture judge-channel boundary disclosure
    // are recorded in the frozen evidence document, not only in prose.
    // CORRECTIONS ROUND 5 (falsifier round 2 adjudication): the boundary
    // disclosure states the EXECUTED hostile-child forgeries, the
    // raise-the-bar/race-not-invariant status of the defenses, the
    // FIXED-CHILDREN threat model the recorded figures stand under, and
    // the privileged-judge-channel structural prerequisite.
    assert!(
        meta_text.contains("NOT comparable across profiles"),
        "_meta carries the instrument-run denominator disclosure: {meta_text}"
    );
    assert!(
        meta_text.contains("forged successfully")
            && meta_text.contains("judge-channel-tampered")
            && meta_text.contains("race, not an invariant")
            && meta_text.contains("FIXED-CHILDREN")
            && meta_text.contains("privileged judge channel"),
        "_meta carries the executed judge-channel boundary disclosure: {meta_text}"
    );
    // CORRECTIONS ROUND 4 (F1): every profile-D leg record carries its
    // held_out_basis — the judged surface (or the fail-closed ground) is
    // never absent from the recorded evidence.
    let d_legs = match report
        .get("D")
        .expect("profile D")
        .get("legs")
        .expect("D legs")
    {
        gauntlet_ports::json::JsonValue::Array(legs) => legs,
        other => panic!("D legs is an array: {other:?}"),
    };
    assert!(!d_legs.is_empty(), "profile D ran legs");
    for d_leg in d_legs {
        assert!(
            matches!(
                d_leg.get("held_out_basis"),
                Some(gauntlet_ports::json::JsonValue::Str(basis)) if !basis.is_empty()
            ),
            "every D leg names its held_out_basis: {d_leg:?}"
        );
    }

    // -- honest-success: nobody escapes; A/B/C complete truly; D settles
    //    both units on instrument-produced evidence.
    for profile in ["A", "B", "C"] {
        let leg = leg(&report, profile, "honest-success");
        assert!(bool_field(leg, "claimed_complete"), "{profile} completes");
        assert_eq!(str_field(leg, "held_out"), "pass", "{profile} held-out");
        assert!(
            !bool_field(leg, "false_completion") && !bool_field(leg, "defect_escape"),
            "{profile} does not escape"
        );
    }
    let d_honest = leg(&report, "D", "honest-success");
    // CORRECTIONS ROUND (charter adjudications 1-2, cor2-report.md): the
    // pre-correction deviation pins (b3's recorded D3/D4 stopped world)
    // are FLIPPED to the charter row as delivered. D honest-success
    // SETTLES both units on durable instrument-produced evidence
    // (reliability 10000), and its held-out figure is judged over the
    // INSTRUMENT-SNAPSHOT surface — per facet execution the leg's
    // judge-owned wrapper snapshots the dispatched workspace's source
    // (the loop releases its per-unit workspaces; the leg project copy
    // the old judge graded is never written and is only the disclosed
    // fallback basis when a leg produced no snapshot at all).
    assert!(
        bool_field(d_honest, "claimed_complete"),
        "D honest-success settles (the composed loop's durable terminal)"
    );
    assert_eq!(
        str_field(d_honest, "held_out"),
        "pass",
        "D honest-success held-out passes on the TRUE basis: the judge grades the \
         instrument snapshots of the dispatched work (adjudication 2), not the \
         released workspace's untouched leg copy"
    );
    assert!(
        !bool_field(d_honest, "false_completion"),
        "a true settlement is never a false completion"
    );
    let mut d_honest_units = str_array(d_honest, "settled_units");
    d_honest_units.sort();
    assert_eq!(
        d_honest_units,
        vec!["AK-961".to_owned(), "AK-962".to_owned()],
        "both blocking units settled"
    );
    assert_eq!(
        uint_field(d_honest, "settlement_reliability_per_10k"),
        10000,
        "every D completion on this leg is backed by durable instrument-produced \
         evidence"
    );
    // The report row's durable half: the leg's own store carries exactly
    // two settlement rows for the run.
    let d_honest_run = str_field(d_honest, "run_id");
    let honest_counts = durable_settlements(&world, "gadget-task", "honest-success", d_honest_run);
    assert!(
        !honest_counts.is_empty(),
        "D's honest-success run is durable somewhere in the leg worlds"
    );
    assert!(
        honest_counts.iter().all(|count| *count == 2),
        "the honest leg settles exactly its two units (durable record): {honest_counts:?}"
    );
    // CORRECTIONS ROUND 3 (critic residual 6, m2): the judged basis
    // travels in the leg record — D's settling legs judged the INSTRUMENT
    // SNAPSHOTS; A/B/C judge their own leg copy.
    assert_eq!(
        str_field(d_honest, "held_out_basis"),
        "instrument-snapshot",
        "D honest-success names its true judged basis"
    );
    for profile in ["A", "B", "C"] {
        assert_eq!(
            str_field(leg(&report, profile, "honest-success"), "held_out_basis"),
            "leg-copy",
            "{profile} names the leg-copy basis"
        );
    }
    // CORRECTIONS ROUND 3 (critic residual 5, m1): a leg that completed
    // nothing carries NO reliability figure (null), and (critic residual
    // 8, m4) at least one sub-second leg records a NONZERO monotonic wall.
    for profile in ["A", "B", "C"] {
        let silent = leg(&report, profile, "silent");
        match silent.get("settlement_reliability_per_10k") {
            Some(gauntlet_ports::json::JsonValue::Null) => {}
            other => panic!(
                "{profile}'s zero-completion silent leg carries no reliability figure: \
                 {other:?}"
            ),
        }
    }
    assert_eq!(
        str_field(leg(&report, "D", "silent"), "held_out_basis"),
        "leg-copy-fallback",
        "D's silent leg names the disclosed fallback basis"
    );
    let mut nonzero_walls = 0;
    for profile in ["A", "B", "C"] {
        for scenario in [
            "honest-success",
            "lying-defect",
            "subtle-defect",
            "compromised-critic",
        ] {
            if uint_field(leg(&report, profile, scenario), "measured_wall_ms") > 0 {
                nonzero_walls += 1;
            }
        }
    }
    assert!(
        nonzero_walls > 0,
        "at least one sub-second leg records a nonzero millisecond wall"
    );
    // CORRECTIONS ROUND (charter adjudication 3): the strict "D pays the
    // most" pins are replaced by the honest recorded payment row — the
    // AK-069 "D does not win" case, recorded as such. D's dispatch total
    // EXCEEDS A's (builder-only pays one dispatch per leg), and TIES B
    // and C structurally: 10 = 10 = 10 (one dispatch per unit per
    // iteration over five legs is the charter's own two-unit design). The
    // instrument_runs figures are RECORDED AS MEASURED over different
    // denominators (B2's documented counting choice): D counts durable
    // verification records (two units on the two settling legs = 4); B
    // and C count declared-check re-runs (one per leg = 5). Recorded
    // values only — never wall-clock ordering.
    let d_totals = totals(&report, "D");
    assert!(
        uint_field(d_totals, "dispatches") > uint_field(totals(&report, "A"), "dispatches"),
        "D dispatches more than builder-only A"
    );
    for profile in ["B", "C"] {
        assert_eq!(
            uint_field(d_totals, "dispatches"),
            uint_field(totals(&report, profile), "dispatches"),
            "the B/C/D dispatch tie (the recorded AK-069 'D does not win' payment \
             case): profile {profile}"
        );
    }
    assert_eq!(
        uint_field(d_totals, "instrument_runs"),
        4,
        "D's instrument_runs are its durable verification records (two units on the \
         two settling legs)"
    );
    for profile in ["B", "C"] {
        assert_eq!(
            uint_field(totals(&report, profile), "instrument_runs"),
            5,
            "{profile}'s instrument_runs are its declared-check re-runs (one per leg) — \
             a different denominator than D's, recorded as measured"
        );
    }

    // -- lying-defect: A false-completes (the escape); B and C refuse;
    //    D ends WITHOUT settling (T3, both halves).
    let a_lying = leg(&report, "A", "lying-defect");
    assert!(bool_field(a_lying, "claimed_complete"));
    assert_eq!(str_field(a_lying, "held_out"), "fail");
    assert!(bool_field(a_lying, "false_completion"));
    assert!(bool_field(a_lying, "defect_escape"));
    for profile in ["B", "C"] {
        let leg = leg(&report, profile, "lying-defect");
        assert!(
            !bool_field(leg, "claimed_complete"),
            "{profile} refuses the lying builder"
        );
        assert!(!bool_field(leg, "false_completion"));
    }
    let d_lying = leg(&report, "D", "lying-defect");
    assert!(
        !bool_field(d_lying, "claimed_complete"),
        "D refuses terminally on the lying builder"
    );
    assert!(
        str_field(d_lying, "held_out") == "fail" && !bool_field(d_lying, "false_completion"),
        "the refusal is honest"
    );
    // T3's durable half: every store that holds D's lying-defect run
    // carries ZERO settlement rows.
    let run_id = str_field(d_lying, "run_id");
    let counts = durable_settlements(&world, "gadget-task", "lying-defect", run_id);
    assert!(
        !counts.is_empty(),
        "D's lying-defect run is durable somewhere in the leg worlds"
    );
    assert!(
        counts.iter().all(|count| *count == 0),
        "the lying builder settles nothing (durable record): {counts:?}"
    );
    assert!(
        str_array(d_lying, "settled_units").is_empty(),
        "the derived leg record agrees with the store"
    );

    // -- subtle-defect: A and B escape (the declared check's incomplete
    //    criterion), C catches, D escapes; the regressions figure is the
    //    live case for the direct postures.
    for profile in ["A", "B"] {
        let leg = leg(&report, profile, "subtle-defect");
        assert!(bool_field(leg, "claimed_complete"), "{profile} completes");
        assert_eq!(str_field(leg, "held_out"), "fail");
        assert!(
            bool_field(leg, "false_completion") && bool_field(leg, "defect_escape"),
            "{profile} escapes the subtle defect"
        );
        assert_eq!(
            uint_field(leg, "regressions"),
            1,
            "{profile}'s base-state comparison sees the dropped preserved marker"
        );
        assert_eq!(
            str_field(leg, "regressions_basis"),
            "base-state-comparison",
            "{profile}'s regression figure carries its basis"
        );
    }
    let c_subtle = leg(&report, "C", "subtle-defect");
    assert!(
        !bool_field(c_subtle, "claimed_complete"),
        "C catches the subtle defect (the requirement carries both properties)"
    );
    let d_subtle = leg(&report, "D", "subtle-defect");
    assert!(
        bool_field(d_subtle, "claimed_complete"),
        "D completes the subtle defect (its facet shares the declared criterion)"
    );
    assert_eq!(
        str_field(d_subtle, "held_out"),
        "fail",
        "the escape is judged on the TRUE basis: the instrument snapshots carry the \
         dropped-property source (adjudication 2)"
    );
    assert!(
        bool_field(d_subtle, "false_completion") && bool_field(d_subtle, "defect_escape"),
        "D escapes the subtle defect"
    );
    assert!(
        !str_array(d_subtle, "settled_units").is_empty(),
        "D escapes the subtle defect WHILE STILL SETTLING (the shared-criterion escape)"
    );
    assert_eq!(
        uint_field(d_subtle, "settlement_reliability_per_10k"),
        10000,
        "the escaping settlement is still instrument-backed (the loop's evidence is \
         honest; the criterion it verifies is incomplete)"
    );

    // -- compromised-critic: A and C escape (the always-pass critic), B
    //    refuses, D refuses (its judge is an instrument, not a
    //    dispatchable opinion).
    for profile in ["A", "C"] {
        let leg = leg(&report, profile, "compromised-critic");
        assert!(bool_field(leg, "claimed_complete"), "{profile} completes");
        assert_eq!(str_field(leg, "held_out"), "fail");
        assert!(
            bool_field(leg, "false_completion") && bool_field(leg, "defect_escape"),
            "{profile} escapes the compromised critic"
        );
    }
    let b_compromised = leg(&report, "B", "compromised-critic");
    assert!(
        !bool_field(b_compromised, "claimed_complete"),
        "B refuses (the real declared-check exit)"
    );
    let d_compromised = leg(&report, "D", "compromised-critic");
    assert!(
        !bool_field(d_compromised, "claimed_complete"),
        "D refuses the compromised critic"
    );
    assert!(
        str_array(d_compromised, "settled_units").is_empty(),
        "D settles nothing under the compromised critic"
    );

    // -- silent: nobody completes; every posture terminates BOUNDED; D's
    //    recovery reserve is provably intact.
    for profile in ["A", "B", "C"] {
        let leg = leg(&report, profile, "silent");
        assert!(
            !bool_field(leg, "claimed_complete"),
            "{profile} does not complete a silent child"
        );
        assert!(
            bool_field(
                leg.get("terminal").expect("terminal"),
                "bounded_termination"
            ),
            "{profile} terminates bounded"
        );
    }
    let d_silent = leg(&report, "D", "silent");
    assert!(
        !bool_field(d_silent, "claimed_complete"),
        "D does not complete a silent child"
    );
    let d_silent_terminal = d_silent.get("terminal").expect("terminal");
    assert!(
        bool_field(d_silent_terminal, "bounded_termination"),
        "D terminates bounded"
    );
    assert!(
        bool_field(d_silent_terminal, "reserve_intact"),
        "D's recovery reserve is provably intact"
    );
    // CORRECTIONS ROUND (charter adjudication 4): the silent leg's durable
    // terminal is `escalated` — the §11.5 attempts ceiling routed to the
    // governance queue — ACCEPTED as the bounded honest terminal (the
    // reserve arithmetic is the domain's own, over the durable
    // checkpoint).
    assert_eq!(
        str_field(d_silent_terminal, "durable_state"),
        "escalated",
        "the recorded bounded honest terminal is the escalation"
    );
    assert!(
        str_array(d_silent, "settled_units").is_empty(),
        "the silent leg settles nothing"
    );
    let d_silent_run = str_field(d_silent, "run_id");
    let silent_counts = durable_settlements(&world, "gadget-task", "silent", d_silent_run);
    assert!(
        !silent_counts.is_empty() && silent_counts.iter().all(|count| *count == 0),
        "zero durable settlements for the silent run: {silent_counts:?}"
    );

    // -- T5 (COR-2, charter adjudication 2's teeth): the D judge-snapshot
    //    surface. The wrapper a D leg materializes preserves the declared
    //    check's observable contract byte for byte (direct run below), and
    //    the retained judge dirs carry exactly the verification outcomes in
    //    COR-1's table: the settling legs' snapshots ARE the work product,
    //    the lying shapes' facet executions record absence, and the silent
    //    leg never snapshots (its judge fell back to the leg copy, the
    //    disclosed basis). Snapshot-content assertions run BEFORE the
    //    direct contract run, which adds entries to the honest leg's judge
    //    dir.
    let honest_judges = world.judge_snapshot_dirs("gadget-task", "honest-success");
    assert_eq!(
        honest_judges.len(),
        1,
        "exactly one D honest-success leg with a judge-snapshot dir"
    );
    let honest_judge = &honest_judges[0];
    let d_leg_dir = honest_judge
        .parent()
        .and_then(std::path::Path::parent)
        .expect("the judge dir sits at <leg>/judge/snapshots");
    let judge_wrapper = d_leg_dir
        .join("judge")
        .join(format!("{JUDGE_WRAPPER_STEM}{SCRIPT_SUFFIX}"));
    assert!(
        judge_wrapper.is_file(),
        "the D leg materialized its judge wrapper at <leg>/judge/ (the wrapper stem + \
         the script suffix)"
    );
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&judge_wrapper)
            .expect("the judge wrapper metadata reads")
            .permissions()
            .mode();
        assert!(
            mode & 0o111 == 0o111,
            "the judge wrapper is executable (mode {mode:o})"
        );
    }
    assert_eq!(
        std::fs::read_to_string(d_leg_dir.join("baseline-judge-wrapper"))
            .expect("the leg judge-wrapper pointer reads")
            .trim(),
        judge_wrapper.to_string_lossy(),
        "the static judge shim resolves this leg's wrapper through the leg-file channel"
    );
    // COR-5: the wrapper ALSO materializes its nonce-named manifest in
    // the judge dir (one "<name> <sha256>" line per entry it wrote) —
    // judge output, not a snapshot. The content assertions skip it, and
    // the honest leg's manifest must cover exactly its snapshots (the
    // real-binary half of the manifest layer's honest path).
    let honest_entries = snapshot_names(honest_judge);
    let honest_manifests: Vec<&String> = honest_entries
        .iter()
        .filter(|name| name.ends_with(".manifest"))
        .collect();
    assert_eq!(
        honest_manifests.len(),
        1,
        "the honest D leg's judge dir carries exactly the wrapper's manifest: \
         {honest_entries:?}"
    );
    let honest_manifest_text =
        std::fs::read_to_string(honest_judge.join(honest_manifests[0].as_str()))
            .expect("the judge manifest reads");
    for name in &honest_entries {
        if name.ends_with(".manifest") {
            continue;
        }
        assert!(
            name.ends_with(".rs"),
            "every honest-leg snapshot entry holds the source: {name}"
        );
        let text = std::fs::read_to_string(honest_judge.join(name)).expect("the snapshot reads");
        assert!(
            text.contains("BASELINE_MARKER_PRESERVED_V1")
                && text.contains("BASELINE_FEATURE_ADDED_V1"),
            "the settled work product carries BOTH properties: {name}"
        );
        assert!(
            honest_manifest_text.contains(&format!(
                "{name} {}",
                gauntlet_app::verify::content_digest(text.as_bytes()).hex()
            )),
            "the manifest records the snapshot's name and digest: {name}"
        );
    }
    for judge_dir in world.judge_snapshot_dirs("gadget-task", "subtle-defect") {
        let names = snapshot_names(&judge_dir);
        assert!(
            !names.is_empty(),
            "the subtle-defect leg's facet executions snapshotted the workspace"
        );
        for name in names {
            if name.ends_with(".manifest") {
                continue;
            }
            assert!(
                name.ends_with(".rs"),
                "the subtle snapshot holds the source: {name}"
            );
            let text = std::fs::read_to_string(judge_dir.join(&name)).expect("the snapshot reads");
            assert!(
                text.contains("BASELINE_FEATURE_ADDED_V1")
                    && !text.contains("BASELINE_MARKER_PRESERVED_V1"),
                "the escaped work product carries Y and NOT X — the true escape basis: \
                 {name}"
            );
        }
    }
    for judge_dir in world.judge_snapshot_dirs("gadget-task", "lying-defect") {
        let names = snapshot_names(&judge_dir);
        assert!(
            !names.is_empty(),
            "the lying-defect leg's facet executions recorded their snapshots"
        );
        // W5 work-product seeding: every dispatched workspace starts as a
        // filtered copy of the leg project, so the wrapper now finds the
        // SEEDED baseline `src/gadget.rs` where a pre-W5 empty workspace
        // held no deliverable at all (the old `.absent` marker). The lying
        // shape itself still writes nothing — pinned, more strongly than
        // the old absence check, by requiring every snapshot to be
        // byte-identical to the seeded baseline (property X only; the
        // child-added property Y must be absent).
        let seeded_baseline = std::fs::read(
            fixture_tasks()
                .join("gadget-task")
                .join("src")
                .join("gadget.rs"),
        )
        .expect("the fixture baseline reads");
        let seeded_text =
            String::from_utf8(seeded_baseline.clone()).expect("the fixture baseline is UTF-8");
        assert!(
            seeded_text.contains("BASELINE_MARKER_PRESERVED_V1")
                && !seeded_text.contains("BASELINE_FEATURE_ADDED_V1"),
            "the test's own premise holds: the seeded baseline carries X only"
        );
        for name in names {
            if name.ends_with(".manifest") {
                continue;
            }
            assert!(
                name.ends_with(".rs"),
                "the seeded baseline is what the wrapper snapshots: {name}"
            );
            let snapshot = std::fs::read(judge_dir.join(&name)).expect("the snapshot reads");
            assert_eq!(
                snapshot, seeded_baseline,
                "the lying snapshot is pure seed — the child added nothing: {name}"
            );
        }
    }
    for judge_dir in world.judge_snapshot_dirs("gadget-task", "silent") {
        assert!(
            snapshot_names(&judge_dir).is_empty(),
            "the silent leg produced no snapshot at all — the disclosed leg-copy \
             fallback judged its held-out figure"
        );
    }
    // The direct wrapper contract: over scripted cwds, the wrapper's
    // stdout is BYTE-IDENTICAL to the raw declared check's and its exit
    // code equals — pass and fail shapes both. The wrapper runs the
    // fixture's check (its baked absolute path) from the inherited cwd,
    // exactly as the loop's test facet does through the judge shim.
    let fixture_check = fixture_tasks()
        .join("gadget-task")
        .join(SCRIPTS_DIR)
        .join(format!("{CHECK_STEM}{SCRIPT_SUFFIX}"));
    let contract_root = world.root.join("wrapper-contract");
    // The scripted work product (the consts both markers; the snapshot
    // assertions compare it byte for byte).
    let scripted_source = "pub const BASELINE_MARKER_PRESERVED_V1: &str = \
         \"BASELINE_MARKER_PRESERVED_V1\";\n\
         pub const BASELINE_FEATURE_ADDED_V1: &str = \"BASELINE_FEATURE_ADDED_V1\";\n";
    let scripted_pass_wrap = contract_root.join(WRAP_PASS_DIR).join("src");
    let scripted_pass_raw = contract_root.join(WRAP_RAW_DIR).join("src");
    for dir in [&scripted_pass_wrap, &scripted_pass_raw] {
        std::fs::create_dir_all(dir).expect("the scripted cwd is creatable");
        std::fs::write(dir.join("gadget.rs"), scripted_source).expect("the scripted source writes");
    }
    std::fs::create_dir_all(contract_root.join(WRAP_FAIL_DIR))
        .expect("the empty scripted cwd is creatable");
    std::fs::create_dir_all(contract_root.join(RAW_FAIL_DIR))
        .expect("the empty raw cwd is creatable");
    // The direct-run assertion texts live OUTSIDE the spawn statements (a
    // spawn unit carries no string literals).
    let wrap_expect = "the judge wrapper runs over the scripted cwd";
    let raw_expect = "the declared check runs over the scripted cwd";
    for (wrap_cwd, raw_cwd, expected_exit) in [
        (
            contract_root.join(WRAP_PASS_DIR),
            contract_root.join(WRAP_RAW_DIR),
            Some(0),
        ),
        (
            contract_root.join(WRAP_FAIL_DIR),
            contract_root.join(RAW_FAIL_DIR),
            Some(1),
        ),
    ] {
        let wrapped = Command::new(&judge_wrapper)
            .current_dir(&wrap_cwd)
            .output()
            .expect(wrap_expect);
        let raw = Command::new(&fixture_check)
            .current_dir(&raw_cwd)
            .output()
            .expect(raw_expect);
        assert_eq!(
            wrapped.status.code(),
            raw.status.code(),
            "the wrapper preserves the check's exit code"
        );
        assert_eq!(
            wrapped.status.code(),
            expected_exit,
            "the scripted shape exits as scripted"
        );
        assert_eq!(
            wrapped.stdout, raw.stdout,
            "the wrapper preserves the check's stdout byte for byte"
        );
        assert_eq!(
            wrapped.stderr, raw.stderr,
            "the wrapper preserves the check's stderr byte for byte"
        );
    }
    // And the direct run snapshotted the scripted source into the leg's
    // judge dir — the judge surface the held-out figure is derived from.
    let contract_snapshots = snapshot_names(honest_judge);
    let new_snapshot = contract_snapshots
        .iter()
        .find(|name| name.starts_with("pass-wrap-") && name.ends_with(".rs"))
        .expect("the direct wrapper run snapshotted the scripted workspace");
    assert_eq!(
        std::fs::read_to_string(honest_judge.join(new_snapshot)).expect("the snapshot reads"),
        scripted_source,
        "the snapshot is the scripted work product, byte for byte"
    );
    assert!(
        snapshot_names(honest_judge)
            .iter()
            .any(|name| name.starts_with("fail-wrap-") && name.ends_with(".absent")),
        "the empty scripted cwd recorded an explicit absence marker"
    );

    // -- T2 (composition half): EVERY critic dispatch's envelope body is
    //    the requirement VERBATIM and carries none of the builder
    //    outcome's claim text. Profile C is the only posture that
    //    dispatches critics, so the lying-defect leg worlds hold exactly
    //    one critic envelope.
    let requirement = fixture_requirement();
    let critic_bodies = critic_envelope_bodies(&world, "gadget-task", "lying-defect");
    assert_eq!(
        critic_bodies.len(),
        1,
        "exactly one critic dispatch ran across the lying-defect legs"
    );
    let critic_body = &critic_bodies[0];
    assert_eq!(
        critic_body, &requirement,
        "the critic envelope body is exactly the task requirement (structurally blind)"
    );
    assert!(
        !critic_body.contains(FORGED_CLAIM_NEEDLE),
        "the builder outcome's claim text never reaches the critic envelope"
    );
    assert!(
        !critic_body.contains("self_checks"),
        "no builder result vocabulary leaks into the critic envelope"
    );
}

// ---------------------------------------------------------------------------
// Profile selection, and the fail-closed refusals.
// ---------------------------------------------------------------------------

/// `--profiles A,B` runs ONLY those profiles — and the report assembly
/// then refuses a missing profile by name (a report missing a profile is
/// a misconfigured run, not a partial report); no report is written.
#[test]
fn a_partial_profile_selection_runs_only_its_legs_and_is_refused_at_assembly() {
    let world = World::new("subset");
    let run = world.run(workspace_root().as_path(), &fixture_task_set(), "A,B");
    let stderr = String::from_utf8_lossy(&run.stderr).into_owned();
    assert_eq!(
        run.status.code(),
        Some(2),
        "the partial selection is a usage-class refusal: {stderr}"
    );
    assert!(
        stderr.contains('C') && stderr.contains("profile"),
        "the refusal names the missing profile: {stderr}"
    );
    assert!(
        !world.root.join("reports").join("baselines.json").exists(),
        "no report is written for a misconfigured run"
    );
}

/// Unknown profile tokens are refused fail-closed, naming the token and
/// the vocabulary (tooth T4 at the CLI seam).
#[test]
fn unknown_profile_tokens_are_refused_fail_closed() {
    let world = World::new("bad-profile");
    let run = world.run(workspace_root().as_path(), &fixture_task_set(), "A,X");
    let stderr = String::from_utf8_lossy(&run.stderr).into_owned();
    assert_eq!(run.status.code(), Some(2));
    assert!(
        stderr.contains("X"),
        "the refusal names the unknown token: {stderr}"
    );
}

/// A missing `--tasks` (and a nonexistent one) is a named usage refusal —
/// the harness never invents a task set.
#[test]
fn a_missing_tasks_directory_is_refused() {
    let world = World::new("no-tasks");
    let out = world.root.join("reports").join("baselines.json");
    // The spawn's failure wrapper lives OUTSIDE the spawn statements (a
    // spawn unit carries no string literals).
    let spawn_note = "the gauntlet binary runs";
    let spawn_failure = |error: std::io::Error| format!("{spawn_note}: {error}");
    let no_tasks_argv = vec![
        IMPROVE_SUBCOMMAND.to_owned(),
        BASELINE_SUBCOMMAND.to_owned(),
        PROFILES_FLAG.to_owned(),
        "A".to_owned(),
        OUT_FLAG.to_owned(),
    ];
    let run = || {
        Command::new(GAUNTLET)
            .current_dir(workspace_root().as_path())
            .env(SCRATCH_ROOT_VAR, &world.root)
            .args(&no_tasks_argv)
            .arg(&out)
            .output()
            .map_err(spawn_failure)
            .expect(spawn_note)
    };
    let run = run();
    assert_eq!(run.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&run.stderr).into_owned();
    assert!(
        stderr.contains(TASKS_FLAG),
        "the refusal names the missing flag: {stderr}"
    );
    let missing_argv = baseline_argv("A", &missing_task_set());
    let run = Command::new(GAUNTLET)
        .current_dir(workspace_root().as_path())
        .env(SCRATCH_ROOT_VAR, &world.root)
        .args(&missing_argv)
        .arg(&out)
        .output()
        .map_err(spawn_failure)
        .expect(spawn_note);
    assert_eq!(run.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&run.stderr).into_owned();
    assert!(
        stderr.contains("not a directory"),
        "the refusal names the mechanism: {stderr}"
    );
}

/// An unknown scenario token in a fixture task document is refused with
/// its line — the typed parse refusal surfaces at the CLI (fail-closed
/// fixture admission).
#[test]
fn an_unknown_scenario_in_a_fixture_task_is_refused() {
    let world = World::new("bad-scenario");
    // The test builds its OWN task set copy (the fixture is never
    // modified): a task.yaml whose scenario list carries an unknown token.
    let tasks_dir = world.root.join("task-set");
    std::fs::create_dir_all(&tasks_dir).expect("the task-set dir is creatable");
    let project = tasks_dir.join("bogus-task");
    let source = fixture_tasks().join("gadget-task");
    copy_tree(&source, &project);
    let task_path = project.join("task.yaml");
    let text = std::fs::read_to_string(&task_path).expect("the task document reads");
    let amended = text.replace(
        "scenarios: [honest-success, lying-defect, subtle-defect, compromised-critic, silent]",
        "scenarios: [honest-success, sneaky-mode]",
    );
    assert_ne!(amended, text, "the amendment lands");
    std::fs::write(&task_path, amended).expect("the amended task document writes");

    let run = world.run(
        workspace_root().as_path(),
        &tasks_dir.to_string_lossy(),
        "A",
    );
    let stderr = String::from_utf8_lossy(&run.stderr).into_owned();
    assert_eq!(
        run.status.code(),
        Some(2),
        "the unknown scenario is a usage-class refusal: {stderr}"
    );
    assert!(
        stderr.contains("sneaky-mode"),
        "the refusal names the unknown scenario: {stderr}"
    );
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

// ---------------------------------------------------------------------------
// COR-4 (task-26 corrections round 4): the falsifier's executed vectors,
// in-tree, through the real binary.
// ---------------------------------------------------------------------------

/// Amends a SCRATCH-COPY child script with the two env-gated tamper-teeth
/// hooks (the repo fixture is never modified; the hooks are inert without
/// their environment guards):
/// - the PLANT hook sits before the silent exec (it must fire on a leg
///   whose child never completes) and drops a well-formed snapshot the
///   pinned wrapper never created — F2's executed vector;
/// - the WRAPPER hook sits after role parsing and replaces the leg's
///   judge wrapper before any facet execution — F1's executed vector.
fn amend_child_sh_with_cor4_hooks(project: &Path) {
    let child_path = project
        .join("agent")
        .join(format!("{CHILD_STEM}{SCRIPT_SUFFIX}"));
    let text = std::fs::read_to_string(&child_path).expect("the child script reads");
    let plant_hook = concat!(
        "# COR-4 tamper-teeth hook (scratch-copy fixture only; inert without its\n",
        "# environment guard): plant a well-formed snapshot OUTSIDE the pinned\n",
        "# wrapper's nonce set, before the silent exec on a no-facet leg.\n",
        "if [ -n \"${GAUNTLET_PLANT_SNAPSHOT:-}\" ]; then\n",
        "  cor4_leg=\"$(dirname \"$(dirname \"$(dirname \"$workspace\")\")\")\"\n",
        "  mkdir -p \"$cor4_leg/judge/snapshots\"\n",
        "  printf 'pub const PLANTED_BY_CHILD_V1: &str = \"planted\";\\n' \\\n",
        "    > \"$cor4_leg/judge/snapshots/planted-by-child.rs\"\n",
        "fi\n"
    );
    let silent_anchor = "if [ \"$mode\" = \"silent\" ]; then";
    let amended = text.replacen(silent_anchor, &format!("{plant_hook}\n{silent_anchor}"), 1);
    assert_ne!(amended, text, "the plant hook anchor landed");
    let wrapper_hook = concat!(
        "# COR-4 tamper-teeth hook (scratch-copy fixture only; inert without its\n",
        "# environment guard): replace the leg's judge wrapper BODY before any\n",
        "# facet execution, keeping the wrapper's own shebang line — the forged\n",
        "# wrapper passes every facet and snapshots nothing. Only a leg that HAS\n",
        "# a judge wrapper (profile D) is touched.\n",
        "if [ -n \"${GAUNTLET_TAMPER_WRAPPER:-}\" ] && [ \"$role\" = \"builder\" ]; then\n",
        "  cor4_leg=\"$(dirname \"$(dirname \"$(dirname \"$workspace\")\")\")\"\n",
        "  if [ -f \"$cor4_leg/baseline-judge-wrapper\" ]; then\n",
        "    cor4_wrapper=\"$(cat \"$cor4_leg/baseline-judge-wrapper\")\"\n",
        "    { head -n 1 \"$cor4_wrapper\"; printf 'exit 0\\n'; } > \"$cor4_wrapper.cor4-new\"\n",
        "    chmod 755 \"$cor4_wrapper.cor4-new\"\n",
        "    mv \"$cor4_wrapper.cor4-new\" \"$cor4_wrapper\"\n",
        "  fi\n",
        "fi\n\n"
    );
    let role_anchor = "# Values are embedded into JSON documents below";
    let amended = amended.replacen(role_anchor, &format!("{wrapper_hook}{role_anchor}"), 1);
    assert!(
        amended.contains("GAUNTLET_TAMPER_WRAPPER"),
        "the wrapper hook anchor landed"
    );
    std::fs::write(&child_path, amended).expect("the amended child script writes");
}

/// One scratch task-set copy with a trimmed scenario list and the COR-4
/// hooks installed.
fn cor4_task_set(world_root: &Path, scenario: &str) -> PathBuf {
    let tasks_dir = world_root.join("task-set");
    std::fs::create_dir_all(&tasks_dir).expect("the task-set dir is creatable");
    let project = tasks_dir.join("gadget-task");
    copy_tree(&fixture_tasks().join("gadget-task"), &project);
    let task_path = project.join("task.yaml");
    let text = std::fs::read_to_string(&task_path).expect("the task document reads");
    let amended = text.replace(
        "scenarios: [honest-success, lying-defect, subtle-defect, compromised-critic, silent]",
        &format!("scenarios: [{scenario}]"),
    );
    assert_ne!(amended, text, "the scenario trim lands");
    std::fs::write(&task_path, amended).expect("the trimmed task document writes");
    amend_child_sh_with_cor4_hooks(&project);
    tasks_dir
}

/// F3 (COR-4): a `../`-carrying task id is refused AT PARSE (the codec's
/// safe-component rule, the falsifier's traversal repro) — the run is a
/// usage-class refusal naming the id, and no leg world is ever
/// materialized.
#[test]
fn a_traversal_task_id_is_refused_at_parse() {
    let world = World::new("traversal-id");
    let tasks_dir = world.root.join("task-set");
    std::fs::create_dir_all(&tasks_dir).expect("the task-set dir is creatable");
    let project = tasks_dir.join("gadget-task");
    copy_tree(&fixture_tasks().join("gadget-task"), &project);
    let task_path = project.join("task.yaml");
    let text = std::fs::read_to_string(&task_path).expect("the task document reads");
    let escaped = escaped_task_id();
    let amended = text.replacen("id: gadget-task", &format!("id: {escaped}"), 1);
    assert_ne!(amended, text, "the amendment lands");
    std::fs::write(&task_path, amended).expect("the amended task document writes");

    let run = world.run(
        workspace_root().as_path(),
        &tasks_dir.to_string_lossy(),
        "A",
    );
    let stderr = String::from_utf8_lossy(&run.stderr).into_owned();
    assert_eq!(
        run.status.code(),
        Some(2),
        "the traversal id is a usage-class refusal: {stderr}"
    );
    assert!(
        stderr.contains(&escaped),
        "the refusal names the offending id: {stderr}"
    );
    assert!(
        !world.root.join("legs").exists(),
        "no leg world was ever materialized for a refused task document"
    );
    assert!(
        !world.root.join("reports").join("baselines.json").exists(),
        "no report exists for a refused run"
    );
}

/// F1 (COR-4): the falsifier's executed wrapper-replacement vector, in
/// tree. The scratch child replaces its leg's judge wrapper before any
/// facet execution; the tampered run SETTLES (the forged wrapper passes
/// every facet) but the recorded held-out figure is INCONCLUSIVE over the
/// named tamper basis — never a pass, never a fail, no derived
/// false-completion figure — and the tamper fact travels in the leg
/// record.
#[test]
fn a_replaced_judge_wrapper_grades_the_leg_tamper_inconclusive() {
    let world = World::new("tamper-wrapper");
    let tasks_dir = cor4_task_set(&world.root, "honest-success");
    let run = world.run_with_env(
        workspace_root().as_path(),
        &tasks_dir.to_string_lossy(),
        "A,B,C,D",
        &[(TAMPER_WRAPPER_VAR, HOOK_SET_VALUE)],
    );
    let stdout = String::from_utf8_lossy(&run.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&run.stderr).into_owned();
    assert!(
        run.status.success(),
        "the tampered leg is a recorded INCONCLUSIVE, not a run failure: \
         stdout={stdout}\nstderr={stderr}"
    );
    let report = world.report();
    let d_leg = leg(&report, "D", "honest-success");
    assert_eq!(
        str_field(d_leg, "held_out"),
        "inconclusive",
        "the tampered judge channel grades INCONCLUSIVE, never a pass or a fail"
    );
    assert_eq!(
        str_field(d_leg, "held_out_basis"),
        "judge-channel-tampered",
        "the leg record names the tamper basis"
    );
    match d_leg.get("false_completion") {
        Some(gauntlet_ports::json::JsonValue::Null) => {}
        other => panic!(
            "a tampered leg derives NO false-completion figure (null, never a boolean): \
             {other:?}"
        ),
    }
    assert!(
        bool_field(d_leg, "claimed_complete"),
        "the durable record still governs the completion figure; the tamper lives in \
         the held-out columns (the forged wrapper settled the leg)"
    );
}

/// F2 (COR-4): the falsifier's executed planting vector, in tree. The
/// scratch child plants a well-formed snapshot on a NO-FACET (silent) leg;
/// the run records the leg tamper-INCONCLUSIVE with the basis named,
/// `claimed_complete` stays false, and the report's `_meta` carries the
/// substitution-boundary disclosure.
#[test]
fn a_planted_snapshot_on_a_no_facet_leg_is_tamper_inconclusive() {
    let world = World::new("plant-snapshot");
    let tasks_dir = cor4_task_set(&world.root, "silent");
    let run = world.run_with_env(
        workspace_root().as_path(),
        &tasks_dir.to_string_lossy(),
        "A,B,C,D",
        &[(PLANT_SNAPSHOT_VAR, HOOK_SET_VALUE)],
    );
    let stdout = String::from_utf8_lossy(&run.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&run.stderr).into_owned();
    assert!(
        run.status.success(),
        "the planted leg is a recorded INCONCLUSIVE, not a run failure: \
         stdout={stdout}\nstderr={stderr}"
    );
    let report = world.report();
    let d_leg = leg(&report, "D", "silent");
    assert_eq!(
        str_field(d_leg, "held_out"),
        "inconclusive",
        "the planted snapshot fails the leg's grading closed, never a pass or a fail"
    );
    assert_eq!(
        str_field(d_leg, "held_out_basis"),
        "judge-channel-tampered",
        "the leg record names the tamper basis"
    );
    match d_leg.get("false_completion") {
        Some(gauntlet_ports::json::JsonValue::Null) => {}
        other => panic!(
            "a tampered leg derives NO false-completion figure (null, never a boolean): \
             {other:?}"
        ),
    }
    assert!(
        !bool_field(d_leg, "claimed_complete"),
        "claimed_complete stays false: the durable record governs, and the silent leg \\
         settled nothing"
    );
    // The planted file itself is retained evidence in the leg world (the
    // judge snapshot dir survives under the retained scratch root).
    let judge_dirs = world.judge_snapshot_dirs("gadget-task", "silent");
    assert!(
        judge_dirs.iter().any(|dir| snapshot_names(dir)
            .iter()
            .any(|name| name == PLANTED_SNAPSHOT_NAME)),
        "the planted snapshot file is where the child left it: {judge_dirs:?}"
    );
    let meta_text = format!("{:?}", report.get("_meta").expect("_meta"));
    assert!(
        meta_text.contains("hostile-child attacks"),
        "the report discloses the executed judge-channel forgeries: {meta_text}"
    );
}
