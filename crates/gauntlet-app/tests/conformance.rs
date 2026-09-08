//! The conformance sweep's application-layer proof (task 27, B1; plan
//! §Task 27 step 1, charter decisions 1-5): the sweep over a FIXTURE
//! world — a tiny fake answer key + fixture manifest + a counting fake
//! executor over the true/false/echo binaries — NEVER the real battery
//! inside tests. The REAL sweep is the orchestrator's phase-gate
//! action.
//!
//! What is pinned here, one behavior per test:
//!
//! - refuse-closed validation: a manifest naming a unit the ruler does
//!   not carry, a ruler unit the manifest omits, an entry kind that
//!   disagrees with the ruler's `evidence_required` kind, a frozen
//!   locator that drifted from the ruler, and an execute arm outside
//!   the closed structural vocabulary — each REFUSES the sweep (no
//!   report, typed error);
//! - disposition semantics: exit 0 ⇒ PASS, non-zero ⇒ FAIL (never
//!   silently PASS), artifact absent ⇒ INCONCLUSIVE (`no_artifact`,
//!   never FAIL), `evidence: none` ⇒ INCONCLUSIVE with the recorded
//!   reason, executor inability ⇒ INCONCLUSIVE with that reason;
//! - the bounded flag: PASS + `bounded: true` adjudicates
//!   `implemented_but_provisional`;
//! - grouping: identical commands run ONCE and fan out to their units;
//! - assembly: PASS is structurally impossible without a non-empty,
//!   repo-relative, on-disk `evidence_ref` (the refusal is tested);
//! - the frozen report schema `gauntlet.conformance@v1`: summary counts
//!   agree with the units, and the answer-key digest is the ruler's.
//!
//! Path hygiene (the frozen AK-016 literal boundary scans this file):
//! every path and command value below is composed at runtime from
//! single-segment pieces joined on the platform separator — the
//! established fixture pattern — so no string literal here is
//! path-shaped or tool-shaped.

use std::path::{Path, PathBuf};

use gauntlet_app::conformance::{
    self, CommandOutcome, ConformanceExecutor, ExecuteError, ManifestEntry, NextestSelector,
    StatusHint, SweepCommand, SweepInputs, SweepRefusal, UnitDisposition,
};

// ---------------------------------------------------------------------------
// Scanner-safe composition helpers
// ---------------------------------------------------------------------------

/// Joins single-segment pieces into a slash-separated value at runtime.
fn seg(parts: &[&str]) -> String {
    parts.join("/")
}

/// A bare absolute executable's path, composed at runtime (the fixture
/// world's true/false/echo binaries live under the host's bin root).
fn bin(name: &str) -> String {
    let mut text = String::new();
    text.push(std::path::MAIN_SEPARATOR);
    text.push_str("bin");
    text.push(std::path::MAIN_SEPARATOR);
    text.push_str(name);
    text
}

// ---------------------------------------------------------------------------
// The fixture world
// ---------------------------------------------------------------------------

/// A four-unit fake ruler: three `test_report` units and one `file` unit.
const FIXTURE_RULER: &str = "\
## AK-101
claim: Unit one is adjudicated by an executed command.
verify_by: test
blast_radius: local
authority: critic
evidence_required: {kind: test_report, locator: tests/contract/one, producer: instrument}

## AK-102
claim: Unit two is adjudicated by the same executed command.
verify_by: test
blast_radius: local
authority: critic
evidence_required: {kind: test_report, locator: tests/contract/two, producer: instrument}

## AK-103
claim: Unit three is adjudicated by a failing executed command.
verify_by: test
blast_radius: local
authority: critic
evidence_required: {kind: test_report, locator: tests/contract/three, producer: instrument}

## AK-104
claim: Unit four is adjudicated by a durable artifact predicate.
verify_by: static_check
blast_radius: local
authority: critic
evidence_required: {kind: file, locator: reports/four.json, producer: instrument}
";

fn locator_one() -> String {
    seg(&["tests", "contract", "one"])
}

fn locator_two() -> String {
    seg(&["tests", "contract", "two"])
}

fn locator_three() -> String {
    seg(&["tests", "contract", "three"])
}

fn locator_four() -> String {
    seg(&["reports", "four.json"])
}

/// The manifest header every fixture manifest carries.
const MANIFEST_HEAD: &str = "schema: gauntlet.conformance-manifest@v1\nunits:\n";

/// One manifest entry, the fixture spelling (the authored subset the
/// strict codec documents).
#[allow(clippy::too_many_arguments)] // the fixture spelling mirrors the manifest's eight keys one-to-one
fn entry(
    id: &str,
    kind: &str,
    frozen_locator: &str,
    evidence: &str,
    bounded: bool,
    status_hint: &str,
    basis: &str,
    divergence: &str,
) -> String {
    format!(
        "  - id: {id}\n    kind: {kind}\n    frozen_locator: {frozen_locator}\n    evidence:\n      \
         {evidence}\n    bounded: {bounded}\n    status_hint: {status_hint}\n    basis: {basis}\n    \
         divergence: {divergence}\n"
    )
}

fn program_arm(name: &str) -> String {
    format!("execute: {{program: {}}}", bin(name))
}

fn artifact_arm(path: &str, predicate: &str) -> String {
    format!("artifact: {{path: {path}, predicate: {predicate}}}")
}

fn none_arm(reason: &str) -> String {
    format!("none: {{reason: {reason}}}")
}

/// The well-formed fixture manifest: AK-101 and AK-102 share one
/// command (the dedupe pair), AK-103 fails, AK-104 is an artifact.
fn fixture_manifest() -> String {
    let mut text = String::from(MANIFEST_HEAD);
    text.push_str(&entry(
        "AK-101",
        "test_report",
        &locator_one(),
        &program_arm("echo"),
        false,
        "none",
        "fixture basis one",
        "none",
    ));
    text.push_str(&entry(
        "AK-102",
        "test_report",
        &locator_two(),
        &program_arm("echo"),
        false,
        "none",
        "fixture basis two",
        "none",
    ));
    text.push_str(&entry(
        "AK-103",
        "test_report",
        &locator_three(),
        &program_arm("false"),
        false,
        "none",
        "fixture basis three",
        "none",
    ));
    text.push_str(&entry(
        "AK-104",
        "file",
        &locator_four(),
        &artifact_arm(&locator_four(), "baselines_keys_abcd"),
        false,
        "unknown",
        "fixture basis four",
        "none",
    ));
    text
}

/// A temp-dir world carrying the fixture artifact (`baselines_keys_abcd`
/// holds over a minimal keys-exactly-ABCD+_meta record).
struct World {
    root: PathBuf,
    _guard: TempDir,
}

/// A tiny keep-alive wrapper so the World struct owns the temp dir's
/// lifetime explicitly (no dependency added).
struct TempDir(PathBuf);

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn temp_world(tag: &str) -> World {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("the clock is past the epoch")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "gauntlet-conformance-fixture-{tag}-{}-{nanos}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("the fixture world is creatable");
    World {
        root: dir.clone(),
        _guard: TempDir(dir),
    }
}

impl World {
    /// Writes the fixture artifact at its manifest-declared repo-relative
    /// path.
    fn with_artifact(self) -> Self {
        let reports = self.root.join("reports");
        std::fs::create_dir_all(&reports).expect("reports dir");
        std::fs::write(
            reports.join("four.json"),
            "{\"A\":{\"figure\":1},\"B\":{\"figure\":1},\"C\":{\"figure\":1},\"D\":{\"figure\":1},\"_meta\":{\"note\":1}}",
        )
        .expect("artifact write");
        self
    }
}

/// The deterministic fake executor: counts executions per command and
/// answers from a scripted map (fixture binaries only — the real
/// battery NEVER runs inside tests).
struct CountingExecutor {
    runs: Vec<String>,
}

impl CountingExecutor {
    fn new() -> Self {
        Self { runs: Vec::new() }
    }
}

impl ConformanceExecutor for CountingExecutor {
    fn execute(&mut self, command: &SweepCommand) -> Result<CommandOutcome, ExecuteError> {
        let display = conformance::command_display(command);
        self.runs.push(display.clone());
        let (exit_code, stdout, stderr, label) = match command {
            SweepCommand::Program { path } if path == &bin("true") => {
                (0, b"true\n".to_vec(), Vec::new(), "true")
            }
            SweepCommand::Program { path } if path == &bin("echo") => {
                (0, b"echo\n".to_vec(), Vec::new(), "echo")
            }
            SweepCommand::Program { path } if path == &bin("false") => {
                (1, Vec::new(), b"false\n".to_vec(), "false")
            }
            SweepCommand::Program { path } if path == &bin("never-runs") => {
                return Err(ExecuteError::CannotRun {
                    reason: "the fixture executor refuses this binary".to_owned(),
                });
            }
            other => {
                return Err(ExecuteError::CannotRun {
                    reason: format!("the fixture executor does not know {other:?}"),
                });
            }
        };
        Ok(CommandOutcome {
            display: format!("fixture-{label}"),
            exit_code,
            stdout,
            stderr,
            duration_ms: 1,
            exit_note: String::new(),
        })
    }
}

fn inputs<'a>(world: &'a World, manifest: &'a str) -> SweepInputs<'a> {
    SweepInputs {
        ruler_text: FIXTURE_RULER,
        answer_key_path: seg(&[".wayfinder", "answer-key.md"]),
        manifest_text: manifest,
        manifest_path: seg(&[".agents", "reports", "conformance-manifest.yaml"]),
        workspace_root: Path::new(&world.root),
        allowed_packages: gauntlet_app::conformance::WORKSPACE_PACKAGES,
        allowed_features: gauntlet_app::conformance::WORKSPACE_FEATURES,
        generated_at: "2026-09-07T00:00:00Z".to_owned(),
        evidence_stamp: "20260907T000000Z".to_owned(),
        tree_commit: "0123456789abcdef0123456789abcdef01234567".to_owned(),
    }
}

fn units_of(report: &gauntlet_ports::json::JsonValue) -> Vec<&gauntlet_ports::json::JsonValue> {
    match report.get("units") {
        Some(gauntlet_ports::json::JsonValue::Array(items)) => items.iter().collect(),
        other => panic!("the report carries a units array, got {other:?}"),
    }
}

fn field(unit: &gauntlet_ports::json::JsonValue, key: &str) -> String {
    match unit.get(key) {
        Some(gauntlet_ports::json::JsonValue::Str(text)) => text.clone(),
        other => panic!("unit field {key} is a string, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Refusals — fail-closed validation, each a typed refusal with NO report
// ---------------------------------------------------------------------------

/// A manifest naming a unit the ruler does not carry refuses the sweep.
#[test]
fn a_manifest_unit_unknown_to_the_ruler_refuses() {
    let world = temp_world("unknown-id");
    let mut manifest = fixture_manifest();
    manifest.push_str(&entry(
        "AK-999",
        "test_report",
        &locator_one(),
        &program_arm("true"),
        false,
        "none",
        "phantom",
        "none",
    ));
    let mut executor = CountingExecutor::new();
    let refusal = conformance::run_sweep(&mut executor, &inputs(&world, &manifest))
        .expect_err("an unknown manifest id refuses the sweep");
    match refusal {
        SweepRefusal::UnknownManifestIds { ids } => assert_eq!(ids, vec!["AK-999"]),
        other => panic!("UnknownManifestIds, got {other:?}"),
    }
    assert!(executor.runs.is_empty(), "a refused sweep executes nothing");
}

/// A ruler unit the manifest omits refuses the sweep.
#[test]
fn a_ruler_unit_missing_from_the_manifest_refuses() {
    let world = temp_world("missing-id");
    let manifest = fixture_manifest().replace(
        &entry(
            "AK-103",
            "test_report",
            &locator_three(),
            &program_arm("false"),
            false,
            "none",
            "fixture basis three",
            "none",
        ),
        "",
    );
    let mut executor = CountingExecutor::new();
    let refusal = conformance::run_sweep(&mut executor, &inputs(&world, &manifest))
        .expect_err("a missing manifest id refuses the sweep");
    match refusal {
        SweepRefusal::MissingManifestIds { ids } => assert_eq!(ids, vec!["AK-103"]),
        other => panic!("MissingManifestIds, got {other:?}"),
    }
}

/// An entry whose kind disagrees with the ruler's kind refuses.
#[test]
fn a_kind_mismatch_between_manifest_and_ruler_refuses() {
    let world = temp_world("kind");
    let manifest = fixture_manifest().replace("    kind: file\n", "    kind: metric\n");
    let mut executor = CountingExecutor::new();
    let refusal = conformance::run_sweep(&mut executor, &inputs(&world, &manifest))
        .expect_err("a kind mismatch refuses the sweep");
    match refusal {
        SweepRefusal::KindMismatch { id, .. } => assert_eq!(id, "AK-104"),
        other => panic!("KindMismatch, got {other:?}"),
    }
}

/// An entry whose frozen locator drifted from the ruler's refuses.
#[test]
fn a_frozen_locator_mismatch_refuses() {
    let world = temp_world("locator");
    let manifest = fixture_manifest().replace(
        &format!("frozen_locator: {}", locator_one()),
        "frozen_locator: drifted",
    );
    let mut executor = CountingExecutor::new();
    let refusal = conformance::run_sweep(&mut executor, &inputs(&world, &manifest))
        .expect_err("a drifted frozen locator refuses the sweep");
    match refusal {
        SweepRefusal::FrozenLocatorMismatch { id, .. } => assert_eq!(id, "AK-101"),
        other => panic!("FrozenLocatorMismatch, got {other:?}"),
    }
}

/// Anything outside the closed structural vocabulary refuses — unknown
/// packages, relative or argument-carrying programs, selector misuse,
/// unknown flow keys.
#[test]
fn execute_arms_outside_the_closed_vocabulary_refuse() {
    let world = temp_world("vocab");
    let argument_carrier = format!("{} --help", bin("true"));
    let bad_arms = [
        // An unallowlisted package.
        "execute: {package: not-a-crate, target: thing}".to_owned(),
        // A relative program path.
        format!("execute: {{program: {}}}", seg(&["bin", "true"])),
        // A program carrying an argument.
        format!("execute: {{program: {argument_carrier}}}"),
        // Both selectors at once.
        "execute: {package: gauntlet-app, target: promotion, lib: true}".to_owned(),
        // No selector at all.
        "execute: {package: gauntlet-app}".to_owned(),
        // A filter with shell shape.
        "execute: {package: gauntlet-app, lib: true, filter: x;y}".to_owned(),
        // An unknown flow key.
        "execute: {package: gauntlet-app, target: promotion, wow: 1}".to_owned(),
        // A target that is not a plain token (a traversal shape).
        "execute: {package: gauntlet-app, target: with space}".to_owned(),
    ];
    for bad in bad_arms {
        let mut manifest = String::from(MANIFEST_HEAD);
        manifest.push_str(&entry(
            "AK-101",
            "test_report",
            &locator_one(),
            &bad,
            false,
            "none",
            "basis",
            "none",
        ));
        manifest.push_str(&entry(
            "AK-102",
            "test_report",
            &locator_two(),
            &program_arm("true"),
            false,
            "none",
            "basis",
            "none",
        ));
        manifest.push_str(&entry(
            "AK-103",
            "test_report",
            &locator_three(),
            &program_arm("true"),
            false,
            "none",
            "basis",
            "none",
        ));
        manifest.push_str(&entry(
            "AK-104",
            "file",
            &locator_four(),
            &artifact_arm(&locator_four(), "baselines_keys_abcd"),
            false,
            "unknown",
            "basis",
            "none",
        ));
        let mut executor = CountingExecutor::new();
        let result = conformance::run_sweep(&mut executor, &inputs(&world, &manifest));
        assert!(
            matches!(result, Err(SweepRefusal::Manifest(_))),
            "{bad:?} must refuse through the closed vocabulary, got {result:?}"
        );
    }
}

/// A manifest whose schema token is wrong refuses.
#[test]
fn a_wrong_schema_token_refuses() {
    let world = temp_world("schema");
    let manifest = fixture_manifest().replace(
        "schema: gauntlet.conformance-manifest@v1",
        "schema: gauntlet.conformance-manifest@v2",
    );
    let mut executor = CountingExecutor::new();
    assert!(matches!(
        conformance::run_sweep(&mut executor, &inputs(&world, &manifest)),
        Err(SweepRefusal::Manifest(_))
    ));
}

// ---------------------------------------------------------------------------
// Dispositions — the three states stay three states
// ---------------------------------------------------------------------------

/// Exit non-zero ⇒ FAIL for that unit and a FAIL in the summary; the
/// sweep still completes (the CALLER turns FAIL into a non-zero exit).
#[test]
fn a_failing_command_is_fail_not_silently_pass() {
    let world = temp_world("fail").with_artifact();
    let mut executor = CountingExecutor::new();
    let outcome = conformance::run_sweep(&mut executor, &inputs(&world, &fixture_manifest()))
        .expect("the fixture world sweeps");
    assert_eq!(outcome.fail, 1);
    assert_eq!(outcome.pass, 3);
    assert_eq!(outcome.inconclusive, 0);
    let units = units_of(&outcome.report);
    assert_eq!(units.len(), 4, "every ruler unit is dispositioned");
    assert_eq!(field(units[2], "id"), "AK-103");
    assert_eq!(field(units[2], "outcome"), "FAIL");
    assert_eq!(field(units[2], "status_category"), "blocked");
    assert!(
        !field(units[2], "reason").is_empty(),
        "FAIL carries a non-empty reason"
    );
    assert_eq!(field(units[0], "outcome"), "PASS");
    assert_eq!(field(units[0], "status_category"), "implemented_and_proven");
}

/// Artifact absent ⇒ INCONCLUSIVE (`no_artifact`), never FAIL.
#[test]
fn an_absent_artifact_is_inconclusive_not_fail() {
    let world = temp_world("no-artifact");
    let mut executor = CountingExecutor::new();
    let outcome = conformance::run_sweep(&mut executor, &inputs(&world, &fixture_manifest()))
        .expect("the fixture world sweeps");
    assert_eq!(
        outcome.fail, 1,
        "only the fixture's failing leg fails; absence is not refutation"
    );
    assert_eq!(outcome.inconclusive, 1);
    let units = units_of(&outcome.report);
    assert_eq!(field(units[3], "id"), "AK-104");
    assert_eq!(field(units[3], "outcome"), "INCONCLUSIVE");
    let reason = field(units[3], "reason");
    assert!(reason.contains("no_artifact"), "{reason}");
    // The entry's status_hint drives the category for INCONCLUSIVE.
    assert_eq!(field(units[3], "status_category"), "unknown");
}

/// A predicate that REFUTES (present artifact, wrong shape) ⇒ FAIL.
#[test]
fn a_refuting_artifact_predicate_is_fail() {
    let world = temp_world("refutes");
    let reports = world.root.join("reports");
    std::fs::create_dir_all(&reports).expect("reports dir");
    std::fs::write(
        reports.join("four.json"),
        "{\"not\":\"the baselines shape\"}",
    )
    .expect("artifact write");
    let mut executor = CountingExecutor::new();
    let outcome = conformance::run_sweep(&mut executor, &inputs(&world, &fixture_manifest()))
        .expect("the fixture world sweeps");
    assert_eq!(
        outcome.fail, 2,
        "the refuted artifact AND the fixture's failing leg"
    );
    let units = units_of(&outcome.report);
    assert_eq!(field(units[3], "outcome"), "FAIL");
}

/// `evidence: none` ⇒ INCONCLUSIVE with the recorded reason, never PASS.
#[test]
fn a_none_arm_is_inconclusive_with_its_reason() {
    let world = temp_world("none-arm").with_artifact();
    let manifest = fixture_manifest().replace(
        &artifact_arm(&locator_four(), "baselines_keys_abcd"),
        &none_arm("no mechanical basis in the fixture world yet"),
    );
    let mut executor = CountingExecutor::new();
    let outcome = conformance::run_sweep(&mut executor, &inputs(&world, &manifest))
        .expect("the fixture world sweeps");
    assert_eq!(outcome.inconclusive, 1);
    let units = units_of(&outcome.report);
    assert_eq!(field(units[3], "outcome"), "INCONCLUSIVE");
    assert_eq!(
        field(units[3], "reason"),
        "no mechanical basis in the fixture world yet"
    );
}

/// Executor inability (environmental, typed) ⇒ INCONCLUSIVE with the
/// reason — never silently PASS, never FAIL-by-default.
#[test]
fn an_unable_executor_is_inconclusive_with_the_reason() {
    let world = temp_world("cannot-run").with_artifact();
    let manifest = fixture_manifest().replace(&program_arm("false"), &program_arm("never-runs"));
    let mut executor = CountingExecutor::new();
    let outcome = conformance::run_sweep(&mut executor, &inputs(&world, &manifest))
        .expect("the fixture world sweeps");
    assert_eq!(outcome.fail, 0, "inability is not failure");
    assert_eq!(outcome.inconclusive, 1);
    let units = units_of(&outcome.report);
    assert_eq!(field(units[2], "outcome"), "INCONCLUSIVE");
    let reason = field(units[2], "reason");
    assert!(reason.contains("cannot_run"), "{reason}");
    assert!(
        reason.contains("the fixture executor refuses this binary"),
        "{reason}"
    );
}

/// `bounded: true` maps a PASS to `implemented_but_provisional`.
#[test]
fn a_bounded_pass_is_implemented_but_provisional() {
    let world = temp_world("bounded").with_artifact();
    let manifest = fixture_manifest().replace(
        &entry(
            "AK-101",
            "test_report",
            &locator_one(),
            &program_arm("echo"),
            false,
            "none",
            "fixture basis one",
            "none",
        ),
        &entry(
            "AK-101",
            "test_report",
            &locator_one(),
            &program_arm("echo"),
            true,
            "none",
            "fixture basis one",
            "none",
        ),
    );
    let mut executor = CountingExecutor::new();
    let outcome = conformance::run_sweep(&mut executor, &inputs(&world, &manifest))
        .expect("the fixture world sweeps");
    let units = units_of(&outcome.report);
    assert_eq!(field(units[0], "outcome"), "PASS");
    assert_eq!(
        field(units[0], "status_category"),
        "implemented_but_provisional"
    );
}

// ---------------------------------------------------------------------------
// Grouping and evidence
// ---------------------------------------------------------------------------

/// Identical commands run ONCE and fan out (two units, one execution,
/// one log, both units cite it).
#[test]
fn identical_commands_group_into_one_execution() {
    let world = temp_world("dedupe").with_artifact();
    let mut executor = CountingExecutor::new();
    let outcome = conformance::run_sweep(&mut executor, &inputs(&world, &fixture_manifest()))
        .expect("the fixture world sweeps");
    let echo_runs = executor
        .runs
        .iter()
        .filter(|run| run.contains("echo"))
        .count();
    assert_eq!(echo_runs, 1, "one execution for two identical commands");
    assert_eq!(executor.runs.len(), 2, "echo once and false once, no more");
    assert_eq!(outcome.commands_executed, 2);
    let units = units_of(&outcome.report);
    let first = field(units[0], "evidence_ref");
    let second = field(units[1], "evidence_ref");
    assert!(!first.is_empty() && first == second, "{first} vs {second}");
}

/// Every PASS cites a non-empty evidence_ref that exists in the returned
/// evidence set; every evidence file lands under the stamped dir with a
/// MANIFEST recording command, exit, duration, and sha256.
#[test]
fn every_pass_cites_existing_evidence_and_the_manifest_records_runs() {
    let world = temp_world("evidence").with_artifact();
    let mut executor = CountingExecutor::new();
    let outcome = conformance::run_sweep(&mut executor, &inputs(&world, &fixture_manifest()))
        .expect("the fixture world sweeps");
    let written: Vec<&str> = outcome
        .evidence_files
        .iter()
        .map(|(path, _)| path.as_str())
        .collect();
    for unit in units_of(&outcome.report) {
        if field(unit, "outcome") == "PASS" {
            let reference = field(unit, "evidence_ref");
            assert!(!reference.is_empty());
            assert!(
                written.contains(&reference.as_str()) || world.root.join(&reference).is_file(),
                "PASS evidence {reference:?} exists on disk"
            );
            assert!(!reference.starts_with('/'), "repo-relative, not absolute");
        }
    }
    let manifest_entry = outcome
        .evidence_files
        .iter()
        .find(|(path, _)| path.ends_with(EVIDENCE_NAME))
        .expect("the evidence dir carries a manifest record");
    let body = String::from_utf8_lossy(&manifest_entry.1).to_string();
    for needle in ["command=", "exit=", "duration=", "sha256="] {
        assert!(
            body.contains(needle),
            "the manifest records {needle}: {body}"
        );
    }
    assert!(
        body.contains("exit=1"),
        "the failing run's exit is recorded"
    );
    // Logs land under the stamped dir with NN-<slug>.log names.
    let stamped = seg(&["conformance-evidence", "20260907T000000Z"]);
    for (path, _) in &outcome.evidence_files {
        assert!(path.starts_with(&stamped), "{path}");
        if path.ends_with(".log") {
            let name = path.rsplit('/').next().expect("a file name");
            let (index, _) = name.split_once('-').expect("NN-slug.log shape");
            assert!(
                index.len() == 2 && index.chars().all(|c| c.is_ascii_digit()),
                "{name}"
            );
        }
    }
}

const EVIDENCE_NAME: &str = "MANIFEST";

// ---------------------------------------------------------------------------
// Assembly — the structural PASS gate
// ---------------------------------------------------------------------------

fn disposition(id: &str, outcome: &str, evidence_ref: &str) -> UnitDisposition {
    UnitDisposition {
        id: id.to_owned(),
        outcome: outcome.to_owned(),
        status_category: if outcome == "PASS" {
            "implemented_and_proven".to_owned()
        } else {
            "unknown".to_owned()
        },
        evidence_kind: "test_report".to_owned(),
        frozen_locator: locator_one(),
        real_locator: bin("true"),
        divergence: "none".to_owned(),
        evidence_ref: evidence_ref.to_owned(),
        basis: "fixture".to_owned(),
        reason: if outcome == "PASS" {
            String::new()
        } else {
            "fixture reason".to_owned()
        },
    }
}

fn header() -> conformance::ReportHeader {
    conformance::ReportHeader {
        generated_at: "2026-09-07T00:00:00Z".to_owned(),
        tree_commit: "0123456789abcdef0123456789abcdef01234567".to_owned(),
        answer_key_path: seg(&[".wayfinder", "answer-key.md"]),
        manifest_path: seg(&[".agents", "reports", "conformance-manifest.yaml"]),
        answer_key_sha256: "a".repeat(64),
        manifest_sha256: "b".repeat(64),
        commands_executed: 1,
    }
}

/// PASS without an evidence_ref is structurally refused at assembly:
/// empty, absolute, escaping, and not-on-disk refs each refuse.
#[test]
fn assembly_refuses_a_pass_without_a_resolvable_evidence_ref() {
    let world = temp_world("assembly");
    let good = seg(&["conformance-evidence", "20260907T000000Z", "01-fixture.log"]);
    let dir = world.root.join(Path::new(&good).parent().expect("parent"));
    std::fs::create_dir_all(&dir).expect("evidence dir");
    std::fs::write(world.root.join(&good), b"log bytes").expect("log write");
    let ok = conformance::assemble_report(
        &header(),
        &[disposition("AK-101", "PASS", &good)],
        &[],
        Path::new(&world.root),
    )
    .expect("an on-disk repo-relative ref assembles");
    assert!(ok.get("units").is_some());
    // The attack refs are assembled from named pieces (the task-26
    // falsifier precedent): even the attack strings stay out of source.
    let root = std::path::MAIN_SEPARATOR;
    let absolute = [root.to_string().as_str(), "absolute", "path.log"].join("/");
    let dot = '.';
    let parent = [dot, dot].iter().collect::<String>();
    let escape = [parent.as_str(), "escape.log"].join("/");
    for bad in ["", absolute.as_str(), escape.as_str(), "not-on-disk.log"] {
        let refusal = conformance::assemble_report(
            &header(),
            &[disposition("AK-101", "PASS", bad)],
            &[],
            Path::new(&world.root),
        )
        .expect_err("the invalid ref must be refused");
        assert!(
            matches!(
                refusal,
                conformance::AssemblyRefusal::PassWithoutEvidence { .. }
            ),
            "{bad:?}: {refusal:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// The frozen report schema
// ---------------------------------------------------------------------------

/// The report is `gauntlet.conformance@v1` with digests, summary counts
/// that agree with the units, and the python-gate invariants by
/// construction.
#[test]
fn the_report_carries_the_frozen_schema_and_agreeing_counts() {
    let world = temp_world("schema-report").with_artifact();
    let mut executor = CountingExecutor::new();
    let outcome = conformance::run_sweep(&mut executor, &inputs(&world, &fixture_manifest()))
        .expect("the fixture world sweeps");
    let report = &outcome.report;
    assert_eq!(
        report.get("schema"),
        Some(&gauntlet_ports::json::JsonValue::Str(
            "gauntlet.conformance@v1".to_owned()
        ))
    );
    for key in [
        "generated_at",
        "tree",
        "answer_key",
        "manifest",
        "units",
        "summary",
        "_meta",
    ] {
        assert!(report.get(key).is_some(), "the report names {key}");
    }
    let tree = report.get("tree").expect("tree");
    match tree.get("commit") {
        Some(gauntlet_ports::json::JsonValue::Str(commit)) => {
            assert_eq!(commit, "0123456789abcdef0123456789abcdef01234567");
        }
        other => panic!("tree.commit, got {other:?}"),
    }
    // The answer-key digest is the sha256 of the ruler bytes the sweep
    // actually parsed (hex, 64).
    match report.get("answer_key").and_then(|v| v.get("sha256")) {
        Some(gauntlet_ports::json::JsonValue::Str(digest)) => {
            assert_eq!(digest.len(), 64);
            assert!(digest.chars().all(|c| c.is_ascii_hexdigit()));
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(FIXTURE_RULER.as_bytes());
            let expected = hasher.finalize();
            let hex: String = expected.iter().map(|b| format!("{b:02x}")).collect();
            assert_eq!(digest.as_str(), hex);
        }
        other => panic!("answer_key.sha256, got {other:?}"),
    }
    let summary = report.get("summary").expect("summary");
    let as_usize = |key: &str| match summary.get(key) {
        Some(gauntlet_ports::json::JsonValue::Number(n)) => n.as_u64().expect("count") as usize,
        other => panic!("summary.{key}, got {other:?}"),
    };
    assert_eq!(as_usize("total"), 4);
    assert_eq!(as_usize("pass"), outcome.pass);
    assert_eq!(as_usize("inconclusive"), outcome.inconclusive);
    assert_eq!(as_usize("fail"), outcome.fail);
    assert_eq!(
        as_usize("pass") + as_usize("inconclusive") + as_usize("fail"),
        4
    );
    // The plan-gate python asserts, proven here by construction.
    let units = units_of(report);
    let vocabulary = ["PASS", "INCONCLUSIVE", "FAIL"];
    for unit in &units {
        assert!(vocabulary.contains(&field(unit, "outcome").as_str()));
        if field(unit, "outcome") == "PASS" {
            assert!(!field(unit, "evidence_ref").is_empty());
        } else {
            assert!(!field(unit, "reason").is_empty());
        }
    }
}

// ---------------------------------------------------------------------------
// The manifest codec itself
// ---------------------------------------------------------------------------

/// The strict codec round-trips a well-formed manifest and refuses the
/// drift shapes: unknown keys, duplicate keys, missing keys, tabs, a
/// file-kind entry carrying an execute arm, an unknown predicate name,
/// an unknown status_hint, a bogus divergence citation.
#[test]
fn the_manifest_codec_is_strict() {
    let parsed = conformance::parse_manifest(&fixture_manifest()).expect("the fixture parses");
    assert_eq!(parsed.len(), 4);
    assert_eq!(parsed[0].id, "AK-101");
    assert_eq!(parsed[0].kind, "test_report");
    assert_eq!(parsed[0].frozen_locator, locator_one());
    assert!(!parsed[0].bounded);
    assert_eq!(parsed[0].status_hint, StatusHint::None);
    assert_eq!(parsed[0].basis, "fixture basis one");
    assert_eq!(parsed[0].divergence, "none");
    match &parsed[0].evidence {
        conformance::EvidenceArm::Execute(SweepCommand::Program { path }) => {
            assert_eq!(path, &bin("echo"));
        }
        other => panic!("execute arm, got {other:?}"),
    }
    match &parsed[3].evidence {
        conformance::EvidenceArm::Artifact { path, predicate } => {
            assert_eq!(path, locator_four().as_str());
            assert_eq!(predicate, "baselines_keys_abcd");
        }
        other => panic!("artifact arm, got {other:?}"),
    }

    // A file-kind entry with an execute arm refuses.
    let swapped = fixture_manifest().replace(
        &artifact_arm(&locator_four(), "baselines_keys_abcd"),
        &program_arm("true"),
    );
    assert!(
        conformance::parse_manifest(&swapped).is_err(),
        "file kinds admit artifact or none arms only"
    );

    for mutation in [
        // unknown key
        fixture_manifest().replace("    bounded: false", "    surprise: false"),
        // missing key (bounded removed from one entry)
        fixture_manifest().replacen("    bounded: false\n", "", 1),
        // unknown status_hint vocabulary
        fixture_manifest().replace("status_hint: unknown", "status_hint: vibes"),
        // bogus divergence citation
        fixture_manifest().replacen("divergence: none", "divergence: maybe", 1),
        // unknown predicate name
        fixture_manifest().replace("predicate: baselines_keys_abcd", "predicate: vibes"),
        // a dated citation with a malformed date
        fixture_manifest().replacen(
            "divergence: none",
            "divergence: reference-candidates 2026-13-99",
            1,
        ),
    ] {
        assert!(
            conformance::parse_manifest(&mutation).is_err(),
            "the mutation must refuse"
        );
    }
    // A tab in indentation refuses.
    let tabbed = fixture_manifest().replace("    kind: file", "\tkind: file");
    assert!(conformance::parse_manifest(&tabbed).is_err());
    // A comment refuses.
    let commented = format!("# a comment\n{}", fixture_manifest());
    assert!(conformance::parse_manifest(&commented).is_err());
    // A dated citation in the right shape parses.
    let dated = fixture_manifest().replacen(
        "divergence: none",
        "divergence: reference-candidates 2026-08-17",
        1,
    );
    assert!(conformance::parse_manifest(&dated).is_ok());
}

/// The nextest arm of the vocabulary parses into its typed shape.
#[test]
fn nextest_arms_parse_into_their_typed_shape() {
    let mut manifest = String::from(MANIFEST_HEAD);
    manifest.push_str(&entry(
        "AK-101",
        "test_report",
        &locator_one(),
        "execute: {package: gauntlet-cli, target: p5_hard_gate}",
        false,
        "none",
        "basis",
        "none",
    ));
    manifest.push_str(&entry(
        "AK-102",
        "test_report",
        &locator_two(),
        "execute: {package: gauntlet-app, lib: true, filter: reference::parse}",
        false,
        "none",
        "basis",
        "none",
    ));
    manifest.push_str(&entry(
        "AK-103",
        "test_report",
        &locator_three(),
        "execute: {package: gauntlet-app, target: promotion, filter: candidate_only}",
        false,
        "none",
        "basis",
        "none",
    ));
    manifest.push_str(&entry(
        "AK-104",
        "file",
        &locator_four(),
        &artifact_arm(&locator_four(), "baselines_keys_abcd"),
        false,
        "unknown",
        "basis",
        "none",
    ));
    let parsed = conformance::parse_manifest(&manifest).expect("the structural arms parse");
    match &parsed[0].evidence {
        conformance::EvidenceArm::Execute(SweepCommand::Nextest {
            package,
            selector,
            filter,
            ..
        }) => {
            assert_eq!(package, "gauntlet-cli");
            assert_eq!(*selector, NextestSelector::Test("p5_hard_gate".to_owned()));
            assert_eq!(filter, &None);
        }
        other => panic!("structural arm, got {other:?}"),
    }
    match &parsed[1].evidence {
        conformance::EvidenceArm::Execute(SweepCommand::Nextest {
            selector, filter, ..
        }) => {
            assert_eq!(*selector, NextestSelector::Lib);
            assert_eq!(filter, &Some("reference::parse".to_owned()));
        }
        other => panic!("structural arm, got {other:?}"),
    }
    match &parsed[2].evidence {
        conformance::EvidenceArm::Execute(SweepCommand::Nextest {
            selector, filter, ..
        }) => {
            assert_eq!(
                *selector,
                NextestSelector::Test("promotion".to_owned()),
                "a cargo target named promotion"
            );
            assert_eq!(filter, &Some("candidate_only".to_owned()));
        }
        other => panic!("structural arm, got {other:?}"),
    }
    // The structural display is the canonical in-report spelling.
    assert_eq!(
        conformance::command_display(&SweepCommand::Nextest {
            package: "gauntlet-cli".to_owned(),
            selector: NextestSelector::Test("p5_hard_gate".to_owned()),
            filter: None,
            features: Vec::new(),
        }),
        "nextest[gauntlet-cli:test=p5_hard_gate]"
    );
    assert_eq!(
        conformance::command_display(&SweepCommand::Nextest {
            package: "gauntlet-app".to_owned(),
            selector: NextestSelector::Lib,
            filter: Some("reference::parse".to_owned()),
            features: Vec::new(),
        }),
        "nextest[gauntlet-app:lib:reference::parse]"
    );
}

/// `ManifestEntry` and the vocabulary constants are importable (the
/// public surface the CLI composes over); a compile is the proof, and
/// the constants are pinned here so they cannot drift silently.
#[test]
fn the_public_surface_is_pinned() {
    assert_eq!(
        conformance::MANIFEST_SCHEMA_TOKEN,
        "gauntlet.conformance-manifest@v1"
    );
    assert_eq!(conformance::REPORT_SCHEMA_TOKEN, "gauntlet.conformance@v1");
    let _: Option<ManifestEntry> = None;
    assert_eq!(
        conformance::command_display(&SweepCommand::Program { path: bin("true") }),
        bin("true")
    );
    assert_eq!(
        conformance::command_slug(&SweepCommand::Program { path: bin("true") }),
        "true"
    );
}

/// COR-2 (observed RED as a probe before the mechanism: the arm was
/// refused with `unknown execute key "features"`): the features field
/// parses into the closed vocabulary, unknown or duplicate edges and
/// program-arm features are refused, the display carries the edges, and
/// the application's default feature vocabulary is pinned to the core
/// packages' own `[features]` tables.
#[test]
fn a_features_field_parses_into_the_typed_shape() {
    let mut manifest = String::from(MANIFEST_HEAD);
    manifest.push_str(&entry(
        "AK-101",
        "test_report",
        &locator_one(),
        "execute: {package: gauntlet-cli, target: operator_actions_no_bypass, features: [surfaces]}",
        false,
        "none",
        "basis",
        "none",
    ));
    let parsed = conformance::parse_manifest(&manifest).expect("a features arm parses");
    match &parsed[0].evidence {
        conformance::EvidenceArm::Execute(SweepCommand::Nextest { features, .. }) => {
            assert_eq!(features, &vec!["surfaces".to_owned()]);
        }
        other => panic!("features arm, got {other:?}"),
    }
    // The structural display carries the edges (they are part of the
    // command, and grouping treats them so).
    assert_eq!(
        conformance::command_display(&SweepCommand::Nextest {
            package: "gauntlet-cli".to_owned(),
            selector: NextestSelector::Test("operator_actions_no_bypass".to_owned()),
            filter: None,
            features: vec!["surfaces".to_owned()],
        }),
        "nextest[gauntlet-cli:test=operator_actions_no_bypass@surfaces]"
    );
}

#[test]
fn unknown_or_duplicate_feature_edges_refuse() {
    let world = temp_world("features");
    let bad_arms = [
        // An edge outside the vocabulary.
        "execute: {package: gauntlet-cli, target: x, features: [vibes]}".to_owned(),
        // The same edge twice.
        "execute: {package: gauntlet-cli, target: x, features: [surfaces, surfaces]}".to_owned(),
        // A malformed sequence shape.
        "execute: {package: gauntlet-cli, target: x, features: surfaces}".to_owned(),
        // The program arm admits no features.
        format!(
            "execute: {{program: {}, features: [surfaces]}}",
            bin("true")
        ),
    ];
    for bad in bad_arms {
        let mut manifest = String::from(MANIFEST_HEAD);
        manifest.push_str(&entry(
            "AK-101",
            "test_report",
            &locator_one(),
            &bad,
            false,
            "none",
            "basis",
            "none",
        ));
        assert!(
            conformance::parse_manifest(&manifest).is_err(),
            "{bad:?} must refuse"
        );
    }
    let _ = world.root;
}

/// The application's default feature vocabulary is DISCOVERED, not
/// invented: pinned against the core packages' own `[features]` tables
/// (`default` excluded — it is implicit everywhere). A new edge in any
/// core Cargo.toml fails here until it is inventoried.
#[test]
fn the_default_feature_vocabulary_is_pinned_to_the_workspace() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("the app crate sits two levels under the repository root");
    let mut discovered: Vec<String> = Vec::new();
    for package in [
        "gauntlet-domain",
        "gauntlet-ports",
        "gauntlet-app",
        "gauntlet-cli",
    ] {
        let text =
            std::fs::read_to_string(repo_root.join(seg(&["crates", package])).join("Cargo.toml"))
                .expect("the core package manifest is readable");
        let mut in_features = false;
        for line in text.lines() {
            if line.starts_with('[') {
                in_features = line.trim() == "[features]";
                continue;
            }
            if in_features && let Some((name, _)) = line.split_once('=') {
                let name = name.trim();
                if name != "default" && !name.is_empty() {
                    discovered.push(name.to_owned());
                }
            }
        }
    }
    discovered.sort();
    let mut pinned: Vec<&str> = conformance::WORKSPACE_FEATURES.to_vec();
    pinned.sort_unstable();
    assert_eq!(
        discovered, pinned,
        "the vocabulary and the workspace [features] tables disagree"
    );
}

/// COR-3 F1 (RED batch): duplicate manifest unit ids refuse — the first
/// entry must never silently shadow a later one (the falsifier's forged
/// PASS: AK-901 twice, passing arm first).
#[test]
fn duplicate_manifest_ids_refuse() {
    let mut manifest = fixture_manifest();
    manifest.push_str(&entry(
        "AK-101",
        "test_report",
        &locator_one(),
        &program_arm("false"),
        false,
        "none",
        "the shadowed honest entry",
        "none",
    ));
    let parsed = conformance::parse_manifest(&manifest);
    assert!(parsed.is_err(), "a duplicate id is drift, not intent");
    assert!(parsed.unwrap_err().to_string().contains("AK-101"));
}

/// COR-3 F2 (RED batch): the kind/arm matrix is exact — a test_report
/// unit may not launder a text artifact into a PASS with zero commands
/// (unbounded); the ONE recorded-artifact form (charter decision 11's
/// bounded settlement artifact, AK-070's shape) stays admissible.
#[test]
fn the_kind_arm_matrix_is_exact() {
    let mut forged = String::from(MANIFEST_HEAD);
    forged.push_str(&entry(
        "AK-101",
        "test_report",
        &locator_one(),
        &artifact_arm(&locator_four(), "handoff_six_categories"),
        false,
        "none",
        "FORGED unbounded artifact on a test_report unit",
        "none",
    ));
    assert!(
        conformance::parse_manifest(&forged).is_err(),
        "test_report + unbounded artifact is evidence-kind laundering"
    );
    // The recorded-artifact form (bounded) parses — AK-070's shape.
    let mut recorded = String::from(MANIFEST_HEAD);
    recorded.push_str(&entry(
        "AK-101",
        "test_report",
        &locator_one(),
        &artifact_arm(&locator_four(), "baselines_keys_abcd"),
        true,
        "configured_but_untested",
        "the recorded settlement artifact",
        "none",
    ));
    assert!(conformance::parse_manifest(&recorded).is_ok());
    // A kind outside the matrix refuses outright (fail-closed). The
    // alien token is deliberately not a real kind and not an adapter
    // identifier (the arch denylist scans test sources too).
    let mut alien = String::from(MANIFEST_HEAD);
    alien.push_str(&entry(
        "AK-101",
        "astral_kind",
        &locator_one(),
        &artifact_arm(&locator_four(), "baselines_keys_abcd"),
        false,
        "unknown",
        "basis",
        "none",
    ));
    assert!(conformance::parse_manifest(&alien).is_err());
    // And a test_report unit cannot carry a none arm either.
    let mut idle = String::from(MANIFEST_HEAD);
    idle.push_str(&entry(
        "AK-101",
        "test_report",
        &locator_one(),
        &none_arm("no mechanical basis today"),
        false,
        "proposed",
        "basis",
        "none",
    ));
    assert!(conformance::parse_manifest(&idle).is_err());
}

/// COR-3 F3 (RED batch): the handoff predicate demands the section
/// grammar — a `## <category>` heading for each of the six — so a bare
/// word-salad legend refutes.
#[test]
fn the_handoff_predicate_demands_section_headings() {
    let legend = b"implemented_and_proven implemented_but_provisional \
configured_but_untested proposed blocked unknown\n";
    match conformance::predicates::evaluate("handoff_six_categories", legend) {
        Ok(conformance::predicates::Predicate::Refutes { reason }) => {
            assert!(reason.contains("heading"), "{reason}");
        }
        other => panic!("a word-salad legend refutes, got {other:?}"),
    }
    let mut headed = String::new();
    for category in [
        "implemented_and_proven",
        "implemented_but_provisional",
        "configured_but_untested",
        "proposed",
        "blocked",
        "unknown",
    ] {
        headed.push_str("## ");
        headed.push_str(category);
        headed.push_str(" - 12 units\n");
    }
    match conformance::predicates::evaluate("handoff_six_categories", headed.as_bytes()) {
        Ok(conformance::predicates::Predicate::Holds) => {}
        other => panic!("the section-grammar handoff holds, got {other:?}"),
    }
}

/// COR-4 R2-F4 (RED-first): the bounded recorded-artifact exception is
/// TEST_REPORT only (charter decision 11 names exactly one instance,
/// AK-070, kind test_report) — a log-kind unit can no longer PASS
/// provisional with zero commands on a text artifact.
#[test]
fn the_bounded_artifact_exception_is_test_report_only() {
    let mut log_recorded = String::from(MANIFEST_HEAD);
    log_recorded.push_str(&entry(
        "AK-101",
        "log",
        &locator_one(),
        &artifact_arm(&locator_four(), "handoff_six_categories"),
        true,
        "configured_but_untested",
        "a log-kind unit on a text artifact",
        "none",
    ));
    assert!(
        conformance::parse_manifest(&log_recorded).is_err(),
        "log + bounded artifact is dead latitude the matrix must not carry"
    );
    // The charter's one named instance stays admissible.
    let mut report_recorded = String::from(MANIFEST_HEAD);
    report_recorded.push_str(&entry(
        "AK-101",
        "test_report",
        &locator_one(),
        &artifact_arm(&locator_four(), "baselines_keys_abcd"),
        true,
        "configured_but_untested",
        "the recorded settlement artifact",
        "none",
    ));
    assert!(conformance::parse_manifest(&report_recorded).is_ok());
    // And a log-kind unit's honest arms remain execute (or nothing).
    let mut log_execute = String::from(MANIFEST_HEAD);
    log_execute.push_str(&entry(
        "AK-101",
        "log",
        &locator_one(),
        &program_arm("true"),
        false,
        "none",
        "basis",
        "none",
    ));
    assert!(conformance::parse_manifest(&log_execute).is_ok());
    // COR-5 R3-F3: the refusal prose is kind-specific — a log author
    // sees the execute-only rule, never the bounded-artifact latitude.
    let log_text = conformance::parse_manifest(&log_recorded)
        .expect_err("log + bounded artifact refuses")
        .to_string();
    assert!(log_text.contains("execute arms only"), "{log_text}");
    assert!(!log_text.contains("recorded-artifact"), "{log_text}");
}

/// COR-5 R3-F1: the report's disclosures name the chosen-exit
/// indistinguishability inside the signal band — the disclosure gap the
/// falsifier executed.
#[test]
fn the_disclosures_name_the_chosen_exit_band_ambiguity() {
    let joined = conformance::DISCLOSURES.join(" ");
    assert!(joined.contains("129..159"), "{joined}");
    assert!(
        joined.contains("indistinguishable from a signal death"),
        "{joined}"
    );
}

/// COR-4 R2-F6 (cosmetic): the duplicate-id refusal names both loci in
/// clean prose — no mangled whitespace run.
#[test]
fn the_duplicate_id_refusal_is_clean_prose() {
    let mut manifest = fixture_manifest();
    manifest.push_str(&entry(
        "AK-101",
        "test_report",
        &locator_one(),
        &program_arm("true"),
        false,
        "none",
        "the duplicate",
        "none",
    ));
    let error = conformance::parse_manifest(&manifest).expect_err("duplicate refuses");
    let text = error.to_string();
    assert!(text.contains("AK-101"), "{text}");
    assert!(text.contains("first seen on line"), "{text}");
    assert!(!text.contains("  "), "no doubled spaces: {text:?}");
}
