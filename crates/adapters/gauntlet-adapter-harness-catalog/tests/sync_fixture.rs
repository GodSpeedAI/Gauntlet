//! Hermetic tests for the report-only donor sync against a tiny fixture donor in a temp dir.

use gauntlet_adapter_harness_catalog::{
    HERDR_PINNED_COMMIT, HarnessDefinition, Origin, SyncReport, check_against_donor,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const MOD_RS: &str = "//! fixture detect module
fn lookup_agent(name: &str) -> Option<Agent> {
    let name = path_basename(name);
    match name {
        \"claude\" | \"claude-code\" => Some(Agent::Claude),
        _ => None,
    }
}
";

const RESUME_RS: &str = "//! fixture resume plan
pub fn plan() {
    match agent {
        (\"herdr:claude\", \"claude\", Id) => {
            vec![\"claude\".into(), \"--resume\".into(), value]
        }
        _ => None,
    }
}
";

const REGISTRY_RS: &str = "//! fixture registry
const BUNDLED_MANIFESTS: &[(&str, &str)] = &[
    (\"claude\", include_str!(\"manifests/claude.toml\")),
];
";

/// Refs for the fixture claude definition, computed from the fixture text so tests stay honest.
fn fixture_refs() -> (usize, usize, usize) {
    let arm = MOD_RS
        .lines()
        .position(|l| l.contains("\"claude\" | "))
        .unwrap()
        + 1;
    let resume = RESUME_RS
        .lines()
        .position(|l| l.contains("\"--resume\""))
        .unwrap()
        + 1;
    let registry = REGISTRY_RS
        .lines()
        .position(|l| l.contains("manifests/claude.toml"))
        .unwrap()
        + 1;
    (arm, resume, registry)
}

fn definition(refs: &[String]) -> HarnessDefinition {
    let refs_toml: Vec<String> = refs.iter().map(|r| format!("\"{r}\"")).collect();
    let toml = format!(
        r#"schema = 1
id = "claude"
label = "Claude Code"
executables = ["claude", "claude-code"]

[provenance]
source = "herdr"
commit = "{HERDR_PINNED_COMMIT}"
refs = [{}]
license = "Apache-2.0"

[launch]
args = []
prompt_mode = "unknown"

[session]
identity = "id"
resume_argv = ["--resume", "{{session}}"]

[lifecycle]
source = "screen"
detection_manifest = "claude"

[control]
interrupt = "none"

[capabilities]
interactive = false
resumable = true
model_selection = false
native_subagents = false
structured_events = false
support_level = 0
"#,
        refs_toml.join(", ")
    );
    HarnessDefinition::from_toml_str(&toml, Origin::BuiltIn).expect("fixture definition parses")
}

fn fixture_definitions() -> Vec<HarnessDefinition> {
    let (arm, resume, registry) = fixture_refs();
    vec![definition(&[
        format!("executables=src/detect/mod.rs:{arm}"),
        format!("session.resume_argv=src/agent_resume.rs:{resume}"),
        format!("lifecycle.detection_manifest=src/detect/manifest.rs:{registry}"),
    ])]
}

fn write(root: &Path, rel: &str, text: &str) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

fn make_donor(tag: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root =
        std::env::temp_dir().join(format!("harness-sync-{tag}-{}-{nanos}", std::process::id()));
    write(&root, "src/detect/mod.rs", MOD_RS);
    write(&root, "src/agent_resume.rs", RESUME_RS);
    write(&root, "src/detect/manifest.rs", REGISTRY_RS);
    write(
        &root,
        "src/detect/manifests/claude.toml",
        "id = \"claude\"\n",
    );
    root
}

fn run(donor: &Path, head: Option<&str>) -> SyncReport {
    check_against_donor(&fixture_definitions(), donor, head)
}

fn has(report: &SyncReport, needle: &str) -> bool {
    report.drift.iter().any(|d| d.contains(needle))
}

#[test]
fn in_sync_fixture_reports_no_drift() {
    let donor = make_donor("in-sync");
    let report = run(&donor, Some(HERDR_PINNED_COMMIT));
    assert!(
        report.drift.is_empty(),
        "unexpected drift: {:?}",
        report.drift
    );
    assert_eq!(report.refs_checked, 3);
    assert_eq!(report.refs_ok, 3);
    let _ = fs::remove_dir_all(&donor);
}

#[test]
fn moved_range_is_reported_as_drift() {
    let donor = make_donor("moved");
    // Insert a line above the lookup arm: the recorded line now points elsewhere.
    write(
        &donor,
        "src/detect/mod.rs",
        &format!("// inserted\n{MOD_RS}"),
    );
    let report = run(&donor, Some(HERDR_PINNED_COMMIT));
    assert!(
        has(&report, "executable \"claude\" not in range"),
        "{:?}",
        report.drift
    );
    let _ = fs::remove_dir_all(&donor);
}

#[test]
fn missing_file_is_reported_as_drift() {
    let donor = make_donor("missing");
    fs::remove_file(donor.join("src/agent_resume.rs")).unwrap();
    let report = run(&donor, Some(HERDR_PINNED_COMMIT));
    assert!(
        has(&report, "missing file src/agent_resume.rs"),
        "{:?}",
        report.drift
    );
    let _ = fs::remove_dir_all(&donor);
}

#[test]
fn new_donor_agent_is_reported_as_drift() {
    let donor = make_donor("new-agent");
    let with_new = MOD_RS.replace(
        "        _ => None,",
        "        \"newbot\" => Some(Agent::NewBot),\n        _ => None,",
    );
    write(&donor, "src/detect/mod.rs", &with_new);
    write(
        &donor,
        "src/detect/manifests/newbot.toml",
        "id = \"newbot\"\n",
    );
    let report = run(&donor, Some(HERDR_PINNED_COMMIT));
    assert!(
        has(&report, "donor agent alias \"newbot\" not in catalog"),
        "{:?}",
        report.drift
    );
    assert!(
        has(&report, "donor manifest newbot not in catalog"),
        "{:?}",
        report.drift
    );
    let _ = fs::remove_dir_all(&donor);
}

#[test]
fn head_mismatch_and_unknown_head_are_drift() {
    let donor = make_donor("head");
    let other = run(&donor, Some("deadbeef"));
    assert!(has(&other, "differs from pinned"), "{:?}", other.drift);
    let unknown = run(&donor, None);
    assert!(
        has(&unknown, "could not be determined"),
        "{:?}",
        unknown.drift
    );
    let _ = fs::remove_dir_all(&donor);
}
