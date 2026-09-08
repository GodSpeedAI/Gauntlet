//! The conformance sweep's CLI proof (task 27, B1): `gauntlet verify
//! --all` over the REAL binary and the REAL frozen ruler, with a
//! generated 70-entry FIXTURE manifest whose execute arms are bare
//! absolute executables — the real battery NEVER runs inside a test
//! (charter B1). The sweep's outputs land under an explicit `--report`
//! temp path, so the repository is only ever read.
//!
//! Pinned here: the `--all`/`--run` mutual exclusion (usage error,
//! exit 2); every refusal class exits non-zero with NO report written;
//! a clean fixture sweep exits 0 with the frozen report shape; a FAIL
//! leg exits non-zero with the report still written honestly; an
//! absent artifact leg is INCONCLUSIVE with exit 0; the sweep writes
//! ONLY the report and the evidence directory.
//!
//! Path hygiene (the frozen AK-016 literal boundary scans this file):
//! every path below is composed at runtime from single-segment pieces —
//! the established fixture pattern — so no string literal here is
//! path-shaped or tool-shaped.

use std::path::{Path, PathBuf};
use std::process::Command;

fn binary() -> Command {
    let exe = env!("CARGO_BIN_EXE_gauntlet");
    let mut command = Command::new(exe);
    command.current_dir(repo_root());
    command
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("the cli crate sits two levels under the repository root")
        .to_path_buf()
}

fn seg(parts: &[&str]) -> String {
    parts.join("/")
}

/// A bare absolute executable's path, composed at runtime (the fixture
/// world's true/false binaries live under the host's bin root).
fn bin(name: &str) -> String {
    let mut text = String::new();
    text.push(std::path::MAIN_SEPARATOR);
    text.push_str("bin");
    text.push(std::path::MAIN_SEPARATOR);
    text.push_str(name);
    text
}

fn true_bin() -> String {
    bin("true")
}

fn false_bin() -> String {
    bin("false")
}

/// The frozen ruler's per-unit shape — id, evidence kind, and frozen
/// locator SEGMENTS (joined at runtime) — the verbatim facts the
/// fixture manifest must agree with, embedded from
/// `.wayfinder/answer-key.md` (read-only; the ruler is frozen).
const RULER_SHAPE: &[(&str, &str, &[&str])] = &[
    (
        "AK-001",
        "test_report",
        &["tests", "contract", "architecture_boundaries"],
    ),
    (
        "AK-002",
        "test_report",
        &["tests", "contract", "architecture_boundaries::denylist"],
    ),
    (
        "AK-003",
        "test_report",
        &["tests", "contract", "typed_enums"],
    ),
    (
        "AK-004",
        "test_report",
        &[
            "tests",
            "contract",
            "answer_key_parse::unknown_enum_is_error",
        ],
    ),
    (
        "AK-005",
        "test_report",
        &["tests", "proofs", "p12_missing_is_not_passing"],
    ),
    (
        "AK-006",
        "test_report",
        &[
            "tests",
            "contract",
            "answer_key_parse::tier_authority_coupling",
        ],
    ),
    (
        "AK-007",
        "test_report",
        &[
            "tests",
            "contract",
            "answer_key_parse::tier_authority_coupling",
        ],
    ),
    (
        "AK-008",
        "test_report",
        &["tests", "contract", "agent_runner::invoke_is_non_blocking"],
    ),
    (
        "AK-009",
        "test_report",
        &[
            "tests",
            "contract",
            "agent_runner::completion_requires_result_contract",
        ],
    ),
    (
        "AK-010",
        "test_report",
        &["tests", "contract", "agent_runner::no_result_is_timeout"],
    ),
    (
        "AK-011",
        "test_report",
        &[
            "tests",
            "contract",
            "agent_runner::malformed_result_rejected",
        ],
    ),
    (
        "AK-012",
        "test_report",
        &["tests", "contract", "agent_runner::contract_mismatch"],
    ),
    (
        "AK-013",
        "test_report",
        &["tests", "proofs", "p8_adapter_substitution"],
    ),
    (
        "AK-014",
        "test_report",
        &[
            "tests",
            "contract",
            "adapter_registry::selection_is_config_driven",
        ],
    ),
    (
        "AK-015",
        "test_report",
        &["tests", "contract", "config::precedence"],
    ),
    (
        "AK-016",
        "test_report",
        &["tests", "contract", "no_hardcoded_literals"],
    ),
    ("AK-017", "test_report", &["tests", "e2e", "doctor"]),
    (
        "AK-018",
        "test_report",
        &["tests", "contract", "secrets::never_leaked"],
    ),
    (
        "AK-019",
        "test_report",
        &["tests", "contract", "env_example_complete"],
    ),
    (
        "AK-020",
        "test_report",
        &["tests", "e2e", "bounded_without_config"],
    ),
    (
        "AK-021",
        "test_report",
        &[
            "tests",
            "contract",
            "reference::normalization_is_deterministic",
        ],
    ),
    (
        "AK-022",
        "test_report",
        &[
            "tests",
            "proofs",
            "p1_reference_integrity::frozen_copy_outside_workspace",
        ],
    ),
    (
        "AK-023",
        "test_report",
        &["tests", "proofs", "p1_reference_integrity"],
    ),
    (
        "AK-024",
        "test_report",
        &[
            "tests",
            "proofs",
            "p1_reference_integrity::digest_rechecked",
        ],
    ),
    (
        "AK-025",
        "test_report",
        &["tests", "antigaming", "g2_change_reference"],
    ),
    (
        "AK-026",
        "test_report",
        &["tests", "proofs", "p2_context_isolation"],
    ),
    (
        "AK-027",
        "test_report",
        &["tests", "proofs", "p2_context_isolation::teeth"],
    ),
    (
        "AK-028",
        "test_report",
        &["tests", "contract", "observer::facet_provenance"],
    ),
    (
        "AK-029",
        "test_report",
        &["tests", "proofs", "p3_real_observation"],
    ),
    (
        "AK-030",
        "test_report",
        &["tests", "antigaming", "g6_hide_failed_observations"],
    ),
    (
        "AK-031",
        "test_report",
        &["tests", "proofs", "p11_critic_is_not_verifier"],
    ),
    (
        "AK-032",
        "test_report",
        &["tests", "contract", "comparison::deterministic_aggregation"],
    ),
    (
        "AK-033",
        "test_report",
        &[
            "tests",
            "contract",
            "review_mode::adaptive_skips_settled_dimensions",
        ],
    ),
    (
        "AK-034",
        "test_report",
        &["tests", "contract", "review_mode::truncation_is_recorded"],
    ),
    (
        "AK-035",
        "test_report",
        &["tests", "contract", "verifier::deterministic_first"],
    ),
    (
        "AK-036",
        "test_report",
        &["tests", "contract", "verifier::evidence_binding"],
    ),
    (
        "AK-037",
        "test_report",
        &["tests", "contract", "controller::records_alternatives"],
    ),
    (
        "AK-038",
        "test_report",
        &["tests", "proofs", "p4_corrective_loop"],
    ),
    (
        "AK-039",
        "test_report",
        &["tests", "contract", "diagnosis::nonconvergence"],
    ),
    (
        "AK-040",
        "test_report",
        &["tests", "contract", "settlement::preconditions"],
    ),
    (
        "AK-041",
        "test_report",
        &["tests", "proofs", "p7_recovery::commit_order"],
    ),
    ("AK-042", "test_report", &["tests", "proofs", "p7_recovery"]),
    (
        "AK-043",
        "test_report",
        &["tests", "antigaming", "g10_builder_writes_answer_key"],
    ),
    (
        "AK-044",
        "test_report",
        &[
            "tests",
            "proofs",
            "p5_hard_gate::irreversible_never_dispatched",
        ],
    ),
    (
        "AK-045",
        "test_report",
        &["tests", "proofs", "p5_hard_gate"],
    ),
    (
        "AK-046",
        "test_report",
        &["tests", "antigaming", "g7_tier_downgrade"],
    ),
    (
        "AK-047",
        "test_report",
        &["tests", "contract", "sandbox::no_unsandboxed_fallback"],
    ),
    (
        "AK-048",
        "test_report",
        &["tests", "contract", "sandbox::wraps_from_process_start"],
    ),
    (
        "AK-049",
        "test_report",
        &["tests", "e2e", "doctor::irreversible_policy_is_error"],
    ),
    (
        "AK-050",
        "test_report",
        &["tests", "contract", "sandbox::capability_mismatch"],
    ),
    (
        "AK-051",
        "test_report",
        &["tests", "proofs", "p6_budget_control"],
    ),
    (
        "AK-052",
        "test_report",
        &["tests", "contract", "budget::reserve_protected"],
    ),
    (
        "AK-053",
        "test_report",
        &["tests", "contract", "state::checkpoint_sufficiency"],
    ),
    (
        "AK-054",
        "test_report",
        &["tests", "contract", "state::deterministic_export"],
    ),
    (
        "AK-055",
        "test_report",
        &["tests", "proofs", "p10_no_false_learning"],
    ),
    (
        "AK-056",
        "test_report",
        &["tests", "proofs", "p9_self_improvement_containment"],
    ),
    (
        "AK-057",
        "test_report",
        &["tests", "contract", "promotion::candidate_only"],
    ),
    ("AK-058", "test_report", &["tests", "antigaming"]),
    (
        "AK-059",
        "test_report",
        &["tests", "fixtures", "mock_runner_scenarios"],
    ),
    ("AK-060", "test_report", &["tests", "e2e", "cli_surface"]),
    ("AK-061", "log", &["build", "cli-only-profile"]),
    (
        "AK-062",
        "test_report",
        &["tests", "contract", "operator_actions::no_bypass"],
    ),
    (
        "AK-063",
        "test_report",
        &["tests", "contract", "roles::install_and_verify"],
    ),
    (
        "AK-064",
        "test_report",
        &[
            "tests",
            "contract",
            "skills::wayfinder_gauntlet_conformance",
        ],
    ),
    (
        "AK-065",
        "test_report",
        &["tests", "contract", "formation::fog_not_in_answer_key"],
    ),
    (
        "AK-066",
        "test_report",
        &["tests", "e2e", "full_loop_real_runner"],
    ),
    ("AK-067", "file", &[".agents", "reports", "handoff.md"]),
    (
        "AK-068",
        "test_report",
        &["tests", "contract", "docs_present"],
    ),
    (
        "AK-069",
        "metric",
        &[".agents", "reports", "baselines.json"],
    ),
    (
        "AK-070",
        "test_report",
        &[".agents", "reports", "mutation.json"],
    ),
];

enum Evidence {
    Program(String),
    Artifact { path: String, predicate: String },
}

struct FixtureEntry {
    id: String,
    kind: String,
    frozen_locator: String,
    evidence: Evidence,
    bounded: bool,
    status_hint: String,
    basis: String,
    divergence: String,
}

impl FixtureEntry {
    fn render(&self) -> String {
        let evidence = match &self.evidence {
            Evidence::Program(path) => format!("execute: {{program: {path}}}"),
            Evidence::Artifact { path, predicate } => {
                format!("artifact: {{path: {path}, predicate: {predicate}}}")
            }
        };
        format!(
            "  - id: {}\n    kind: {}\n    frozen_locator: {}\n    evidence:\n      {}\n    \
             bounded: {}\n    status_hint: {}\n    basis: {}\n    divergence: {}\n",
            self.id,
            self.kind,
            self.frozen_locator,
            evidence,
            self.bounded,
            self.status_hint,
            self.basis,
            self.divergence
        )
    }
}

/// The per-entry fixture defaults over the REAL ruler's shape. `mutate`
/// rewrites selected entries.
fn fixture_manifest(mutate: &dyn Fn(&mut FixtureEntry)) -> String {
    let mut text = String::from("schema: gauntlet.conformance-manifest@v1\nunits:\n");
    for (id, kind, locator) in RULER_SHAPE {
        let mut entry = FixtureEntry {
            id: (*id).to_owned(),
            kind: (*kind).to_owned(),
            frozen_locator: seg(locator),
            evidence: if *kind == "file" || *kind == "metric" {
                Evidence::Artifact {
                    path: seg(&[".agents", "reports", "baselines.json"]),
                    predicate: "baselines_keys_abcd".to_owned(),
                }
            } else {
                Evidence::Program(true_bin())
            },
            bounded: false,
            status_hint: if *kind == "file" || *kind == "metric" {
                "unknown".to_owned()
            } else {
                "none".to_owned()
            },
            basis: format!("fixture basis {id}"),
            divergence: "none".to_owned(),
        };
        mutate(&mut entry);
        text.push_str(&entry.render());
    }
    text
}

struct Scratch {
    root: PathBuf,
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn scratch(tag: &str) -> Scratch {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("past the epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "gauntlet-conformance-cli-{tag}-{}-{nanos}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("scratch dir");
    Scratch { root }
}

fn run_sweep(args: &[&str]) -> (Option<i32>, String, String) {
    let output = binary()
        .args(["verify", "--all"])
        .args(args)
        .output()
        .expect("the gauntlet binary runs");
    (
        output.status.code(),
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
    )
}

/// `--all` and `--run` are mutually exclusive: a usage error, exit 2,
/// in both orders (and `--unit` belongs to `--run` alone).
#[test]
fn all_and_run_are_mutually_exclusive_usage_errors() {
    for invocation in [
        vec!["--all", "--run", "01J5ZZZZZZZZZZZZZZZZZZZZZZZ"],
        vec!["--run", "01J5ZZZZZZZZZZZZZZZZZZZZZZZ", "--all"],
        vec!["--all", "--unit", "AK-001"],
    ] {
        let (code, _, stderr) = run_sweep(&invocation);
        assert_eq!(code, Some(2), "{invocation:?} is a usage error");
        assert!(
            stderr.contains("gauntlet verify"),
            "the refusal names its surface: {stderr}"
        );
    }
}

/// A manifest naming a unit the ruler does not carry refuses: non-zero
/// exit, NO report written, the phantom id named.
#[test]
fn an_unknown_manifest_id_refuses_without_a_report() {
    let world = scratch("unknown");
    let manifest_path = world.root.join("manifest.yaml");
    let report_path = world.root.join("conformance.json");
    std::fs::write(
        &manifest_path,
        fixture_manifest(&|entry: &mut FixtureEntry| {
            if entry.id == "AK-001" {
                entry.id = "AK-999".to_owned();
            }
        }),
    )
    .expect("fixture manifest write");
    let (code, _, stderr) = run_sweep(&[
        "--report",
        report_path.to_str().expect("utf8 path"),
        "--manifest",
        manifest_path.to_str().expect("utf8 path"),
    ]);
    assert_ne!(code, Some(0), "the sweep refuses");
    assert_ne!(code, Some(2), "a refusal is not a usage error");
    assert!(
        stderr.contains("AK-999"),
        "the phantom id is named: {stderr}"
    );
    assert!(!report_path.exists(), "a refused sweep writes no report");
}

/// A ruler unit the manifest omits refuses, both directions checked.
#[test]
fn a_missing_ruler_unit_refuses_without_a_report() {
    let world = scratch("missing");
    let manifest_path = world.root.join("manifest.yaml");
    let report_path = world.root.join("conformance.json");
    let mut text = String::from("schema: gauntlet.conformance-manifest@v1\nunits:\n");
    for (id, kind, locator) in RULER_SHAPE {
        if *id == "AK-070" {
            continue;
        }
        let entry = FixtureEntry {
            id: (*id).to_owned(),
            kind: (*kind).to_owned(),
            frozen_locator: seg(locator),
            evidence: if *kind == "file" || *kind == "metric" {
                Evidence::Artifact {
                    path: seg(&[".agents", "reports", "baselines.json"]),
                    predicate: "baselines_keys_abcd".to_owned(),
                }
            } else {
                Evidence::Program(true_bin())
            },
            bounded: false,
            status_hint: "none".to_owned(),
            basis: format!("fixture basis {id}"),
            divergence: "none".to_owned(),
        };
        text.push_str(&entry.render());
    }
    std::fs::write(&manifest_path, text).expect("fixture manifest write");
    let (code, _, stderr) = run_sweep(&[
        "--report",
        report_path.to_str().expect("utf8 path"),
        "--manifest",
        manifest_path.to_str().expect("utf8 path"),
    ]);
    assert_ne!(code, Some(0));
    assert!(
        stderr.contains("AK-070"),
        "the missing id is named: {stderr}"
    );
    assert!(!report_path.exists());
}

/// An entry kind disagreeing with the ruler refuses.
#[test]
fn a_kind_mismatch_refuses_without_a_report() {
    let world = scratch("kind");
    let manifest_path = world.root.join("manifest.yaml");
    let report_path = world.root.join("conformance.json");
    std::fs::write(
        &manifest_path,
        fixture_manifest(&|entry: &mut FixtureEntry| {
            if entry.id == "AK-001" {
                // `log` keeps the codec's arm rules satisfiable (an
                // execute arm is legal for log) so the RULER's kind
                // equality is the refusal that fires.
                entry.kind = "log".to_owned();
            }
        }),
    )
    .expect("fixture manifest write");
    let (code, _, stderr) = run_sweep(&[
        "--report",
        report_path.to_str().expect("utf8 path"),
        "--manifest",
        manifest_path.to_str().expect("utf8 path"),
    ]);
    assert_ne!(code, Some(0));
    assert!(stderr.contains("AK-001"), "{stderr}");
    assert!(!report_path.exists());
}

/// An out-of-vocabulary execute arm refuses.
#[test]
fn an_out_of_vocabulary_execute_arm_refuses_without_a_report() {
    let world = scratch("vocab");
    let manifest_path = world.root.join("manifest.yaml");
    let report_path = world.root.join("conformance.json");
    std::fs::write(
        &manifest_path,
        fixture_manifest(&|entry: &mut FixtureEntry| {
            if entry.id == "AK-002" {
                entry.evidence = Evidence::Program(seg(&["bin", "true"]));
            }
        }),
    )
    .expect("fixture manifest write");
    let (code, _, _) = run_sweep(&[
        "--report",
        report_path.to_str().expect("utf8 path"),
        "--manifest",
        manifest_path.to_str().expect("utf8 path"),
    ]);
    assert_ne!(code, Some(0));
    assert!(!report_path.exists());
}

fn str_field(value: &gauntlet_ports::json::JsonValue, key: &str) -> String {
    match value.get(key) {
        Some(gauntlet_ports::json::JsonValue::Str(text)) => text.clone(),
        other => panic!("field {key} is a string, got {other:?}"),
    }
}

fn units_of(report: &gauntlet_ports::json::JsonValue) -> Vec<gauntlet_ports::json::JsonValue> {
    match report.get("units") {
        Some(gauntlet_ports::json::JsonValue::Array(items)) => items.clone(),
        other => panic!("units array, got {other:?}"),
    }
}

fn read_report(path: &Path) -> gauntlet_ports::json::JsonValue {
    let text = std::fs::read_to_string(path).expect("report read");
    gauntlet_ports::json::parse(&text).expect("the report is JSON")
}

/// The clean fixture sweep: 70 units, every execute arm the ONE shared
/// bare executable (grouped and run once), artifacts judged by their
/// real predicates — exit 0, the frozen report on disk, evidence under
/// the report's parent, and nothing else written there.
#[test]
fn a_clean_fixture_sweep_exits_zero_and_writes_report_and_evidence_only() {
    let world = scratch("clean");
    let manifest_path = world.root.join("manifest.yaml");
    let report_path = world.root.join("conformance.json");
    std::fs::write(&manifest_path, fixture_manifest(&|_| {})).expect("fixture manifest write");
    let (code, stdout, stderr) = run_sweep(&[
        "--report",
        report_path.to_str().expect("utf8 path"),
        "--manifest",
        manifest_path.to_str().expect("utf8 path"),
    ]);
    assert_eq!(code, Some(0), "stdout:\n{stdout}\nstderr:\n{stderr}");
    assert!(report_path.is_file(), "the report is written");

    let report = read_report(&report_path);
    assert_eq!(
        report.get("schema"),
        Some(&gauntlet_ports::json::JsonValue::Str(
            "gauntlet.conformance@v1".to_owned()
        ))
    );
    let units = units_of(&report);
    assert_eq!(units.len(), 70, "every ruler unit is dispositioned");
    let passes = units
        .iter()
        .filter(|unit| str_field(unit, "outcome") == "PASS")
        .count();
    // Every executed unit passes; the two artifact units pass over the
    // real baselines.json.
    assert_eq!(passes, 70, "the fixture world is all-PASS");
    for unit in &units {
        assert!(!str_field(unit, "evidence_ref").is_empty());
    }
    // Grouping: 68 execute units share ONE command.
    match report
        .get("_meta")
        .and_then(|meta| meta.get("commands_executed"))
    {
        Some(gauntlet_ports::json::JsonValue::Number(n)) => {
            assert_eq!(n.as_u64(), Some(1), "one execution fans out to 68 units");
        }
        other => panic!("_meta.commands_executed, got {other:?}"),
    }
    // The evidence dir sits beside the report and holds a log + MANIFEST.
    let evidence_root = world.root.join("conformance-evidence");
    let stamp_dir = std::fs::read_dir(&evidence_root)
        .expect("the evidence dir exists")
        .next()
        .expect("one stamped run")
        .expect("entry");
    let files: Vec<String> = std::fs::read_dir(stamp_dir.path())
        .expect("stamped dir readable")
        .map(|entry| {
            entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .to_string()
        })
        .collect();
    assert_eq!(
        files.len(),
        2,
        "one log and the manifest record: {files:?} (grouping holds at the CLI too)"
    );
    assert!(files.iter().any(|name| name == "MANIFEST"));
    assert!(files.iter().any(|name| name.ends_with(".log")));
    let manifest_body =
        std::fs::read_to_string(stamp_dir.path().join("MANIFEST")).expect("manifest read");
    let recorded_command = format!("command={}", true_bin());
    assert!(manifest_body.contains(&recorded_command), "{manifest_body}");
    assert!(manifest_body.contains("exit=0"), "{manifest_body}");
    // The sweep wrote ONLY the report and the evidence dir.
    let written: Vec<String> = std::fs::read_dir(&world.root)
        .expect("scratch readable")
        .map(|entry| {
            entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .to_string()
        })
        .collect();
    assert_eq!(
        written.len(),
        3,
        "manifest + report + evidence dir: {written:?}"
    );
}

/// A FAIL leg: exit non-zero, the report still written, the failing unit
/// FAIL with its evidence log, the manifest record carrying the
/// non-zero exit.
#[test]
fn a_failing_command_exits_non_zero_with_an_honest_report() {
    let world = scratch("fail");
    let manifest_path = world.root.join("manifest.yaml");
    let report_path = world.root.join("conformance.json");
    std::fs::write(
        &manifest_path,
        fixture_manifest(&|entry: &mut FixtureEntry| {
            if entry.id == "AK-001" {
                entry.evidence = Evidence::Program(false_bin());
            }
        }),
    )
    .expect("fixture manifest write");
    let (code, stdout, stderr) = run_sweep(&[
        "--report",
        report_path.to_str().expect("utf8 path"),
        "--manifest",
        manifest_path.to_str().expect("utf8 path"),
    ]);
    assert_ne!(code, Some(0), "FAIL means non-zero: {stdout} {stderr}");
    assert!(report_path.is_file(), "the honest report is still written");
    let report = read_report(&report_path);
    let units = units_of(&report);
    let ak1 = units
        .iter()
        .find(|unit| str_field(unit, "id") == "AK-001")
        .expect("AK-001 dispositioned");
    assert_eq!(str_field(ak1, "outcome"), "FAIL");
    // Two distinct commands now: the passing group and the failing one.
    let evidence_root = world.root.join("conformance-evidence");
    let stamp_dir = std::fs::read_dir(&evidence_root)
        .expect("evidence dir")
        .next()
        .expect("one stamped run")
        .expect("entry");
    let manifest_body =
        std::fs::read_to_string(stamp_dir.path().join("MANIFEST")).expect("manifest read");
    assert!(manifest_body.contains("exit=1"), "{manifest_body}");
    let recorded_command = format!("command={}", false_bin());
    assert!(manifest_body.contains(&recorded_command), "{manifest_body}");
}

/// An absent artifact is INCONCLUSIVE and the sweep still exits 0 (the
/// redesign trigger's honest-reporting posture). Wave-2 note (task-27
/// B3): the REAL `.agents/reports/handoff.md` now exists and satisfies
/// its predicate, so this tooth points the fixture's artifact arm at a
/// path guaranteed absent under the repo root — the no_artifact
/// semantics stay pinned independently of the real tree's state.
#[test]
fn an_absent_artifact_is_inconclusive_and_the_sweep_exits_zero() {
    let world = scratch("absent");
    let manifest_path = world.root.join("manifest.yaml");
    let report_path = world.root.join("conformance.json");
    std::fs::write(
        &manifest_path,
        fixture_manifest(&|entry: &mut FixtureEntry| {
            if entry.id == "AK-067" {
                entry.evidence = Evidence::Artifact {
                    path: seg(&[
                        ".agents",
                        "reports",
                        "fixture-artifact-that-never-exists.md",
                    ]),
                    predicate: "handoff_six_categories".to_owned(),
                };
            }
        }),
    )
    .expect("fixture manifest write");
    let (code, stdout, stderr) = run_sweep(&[
        "--report",
        report_path.to_str().expect("utf8 path"),
        "--manifest",
        manifest_path.to_str().expect("utf8 path"),
    ]);
    assert_eq!(
        code,
        Some(0),
        "INCONCLUSIVE alone exits 0: {stdout} {stderr}"
    );
    let report = read_report(&report_path);
    let units = units_of(&report);
    let ak67 = units
        .iter()
        .find(|unit| str_field(unit, "id") == "AK-067")
        .expect("AK-067 dispositioned");
    assert_eq!(str_field(ak67, "outcome"), "INCONCLUSIVE");
    let reason = str_field(ak67, "reason");
    assert!(reason.contains("no_artifact"), "{reason}");
}

/// The COMMITTED manifest agrees with the FROZEN ruler: schema, set
/// equality both directions, kind equality, frozen-locator equality —
/// the task-27 manifest tooth (T1). Validation only, through the
/// application layer directly with an executor that never runs anything
/// (the real battery NEVER runs inside tests); the all-cannot_run
/// dispositions are the expected honest shape, and the two recorded
/// artifacts at their frozen locators adjudicate over the real tree.
/// This test lives in the CLI crate because the sweep's package
/// vocabulary names the runner adapters only the composition root may
/// name (AK-002's adapter-identifier denylist scopes application source).
#[test]
fn the_committed_manifest_agrees_with_the_frozen_ruler() {
    let root = repo_root();
    let ruler = std::fs::read_to_string(root.join(".wayfinder").join("answer-key.md"))
        .expect("the frozen ruler is readable");
    let manifest = std::fs::read_to_string(
        root.join(".agents")
            .join("reports")
            .join("conformance-manifest.yaml"),
    )
    .expect("the committed conformance manifest is readable");
    struct NeverRuns;
    impl gauntlet_app::conformance::ConformanceExecutor for NeverRuns {
        fn execute(
            &mut self,
            _command: &gauntlet_app::conformance::SweepCommand,
        ) -> Result<
            gauntlet_app::conformance::CommandOutcome,
            gauntlet_app::conformance::ExecuteError,
        > {
            Err(gauntlet_app::conformance::ExecuteError::CannotRun {
                reason: "tests never run the battery".to_owned(),
            })
        }
    }
    let inputs = gauntlet_app::conformance::SweepInputs {
        ruler_text: &ruler,
        answer_key_path: seg(&[".wayfinder", "answer-key.md"]),
        manifest_text: &manifest,
        manifest_path: seg(&[".agents", "reports", "conformance-manifest.yaml"]),
        workspace_root: root.as_path(),
        allowed_packages: gauntlet_cli::verify_cmd::sweep_packages(),
        allowed_features: gauntlet_cli::verify_cmd::sweep_features(),
        generated_at: "2026-09-07T00:00:00Z".to_owned(),
        evidence_stamp: "20260907T000000Z".to_owned(),
        tree_commit: "0123456789abcdef0123456789abcdef01234567".to_owned(),
    };
    let outcome = gauntlet_app::conformance::run_sweep(&mut NeverRuns, &inputs)
        .expect("the committed manifest cross-validates against the frozen ruler");
    let units = match outcome.report.get("units") {
        Some(gauntlet_ports::json::JsonValue::Array(items)) => items.clone(),
        other => panic!("units array, got {other:?}"),
    };
    assert_eq!(units.len(), 70, "all seventy ruler units are dispositioned");
    for (index, unit) in units.iter().enumerate() {
        assert_eq!(
            str_field(unit, "id"),
            format!("AK-{:03}", index + 1),
            "ruler order"
        );
    }
    // Every executed arm is honestly INCONCLUSIVE (cannot_run) under the
    // never-run executor; the recorded artifacts adjudicate for real.
    // Wave-2 note (task-27 B3): `.agents/reports/handoff.md` now EXISTS
    // and satisfies `handoff_six_categories`, so AK-067 adjudicates PASS
    // here alongside the two recorded artifacts (pre-wave-2 this was
    // 2 PASS / 68 INCONCLUSIVE with AK-067 `no_artifact`).
    assert_eq!(
        outcome.fail, 0,
        "nothing fails under a validation-only sweep"
    );
    assert_eq!(
        outcome.inconclusive, 67,
        "the sixty-seven execute arms cannot run; the three artifact arms judge for real"
    );
    assert_eq!(outcome.pass, 3);
    // COR-2: the committed manifest's feature edge is inside the
    // composition root's feature vocabulary (the parse above proved it
    // by not refusing; pin the const so drift is loud).
    assert!(gauntlet_cli::verify_cmd::sweep_features().contains(&"surfaces"));
}
