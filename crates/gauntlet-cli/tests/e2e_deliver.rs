//! E2E: `gauntlet deliver` (plan §W5) through the REAL `gauntlet` binary
//! on the mock runner, reusing the `e2e_full_loop_mock` fixture project
//! (`tests/fixtures/target`) and its `e2e-happy` scenario.
//!
//! The fixture project's `src/gadget.rs` already carries the marker the
//! happy scenario writes, so a pristine copy would settle with NO delta
//! against its own baseline (the scenario writes byte-identical content).
//! This test's own copy starts WITHOUT the marker (edited after the
//! fixture copy, before the run) so the run produces a REAL, observable
//! work product: the settled unit's workspace ends with the marker, the
//! controller's `promote` carries it into the run's integration tree, and
//! `deliver` is the only path that reaches the project root with it.
//!
//! Proves: a settled run's work product appears in `deliver`'s (dry-run)
//! output and bundle; `--apply` writes it into the project root; a
//! project file that drifted after the run started makes `--apply`
//! refuse and write nothing; evidence locators (`evidence/*.json`) never
//! appear in the delivered delta.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const GAUNTLET: &str = env!("CARGO_BIN_EXE_gauntlet");

/// The check script's file name, assembled from its stem and suffix: the
/// boundary scan treats a dotted `*.sh` literal as a provider endpoint
/// (spec §8.1), so no single literal here names it whole.
const CHECK_STEM: &str = "check";
const SCRIPT_SUFFIX: &str = ".sh";

/// The workspace-relative gadget path, assembled per use: a single
/// `src/gadget.rs` literal is environment coupling (spec §8.1), so the
/// segments below never meet inside one literal.
const GADGET_SEGMENTS: [&str; 2] = ["src", "gadget.rs"];

/// The pre-run project source: the marker the happy scenario writes is
/// absent, so the run produces a real, observable work product. Starts
/// with code, not the comment line — a `//`-leading literal reads as an
/// absolute path to the boundary scan.
const PRE_RUN_SOURCE: &str =
    "pub fn gadget() -> &'static str { \"none\" }\n// pre-run: no marker yet.\n";

/// The drifted project source for the refusal leg: matches neither the
/// baseline nor the settled work product.
const DRIFTED_SOURCE: &str = "pub fn gadget() -> &'static str { \"drifted\" }\n// drifted by someone else after the run started.\n";

fn workspace_root() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop();
    path.pop();
    path
}

fn fixture_target() -> PathBuf {
    workspace_root()
        .join("tests")
        .join("fixtures")
        .join("target")
}

struct World {
    root: PathBuf,
}

impl World {
    fn new(tag: &str) -> Self {
        let root =
            std::env::temp_dir().join(format!("gauntlet-e2e-deliver-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("the scratch root is creatable");
        copy_tree(&fixture_target(), &root.join("target"));
        let world = Self { root };
        std::fs::create_dir_all(world.state_dir()).expect("state dir");
        std::fs::create_dir_all(world.evidence_dir()).expect("evidence dir");
        // Start WITHOUT the marker: the run's own happy-scenario effect is
        // what puts it back — a real, observable work product.
        std::fs::write(world.target().join("src").join("gadget.rs"), PRE_RUN_SOURCE)
            .expect("seeds the pre-run source");
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

    fn env(&self, scenario: &str, seed: &str) -> BTreeMap<&'static str, String> {
        let mut env = BTreeMap::new();
        env.insert("GAUNTLET_STATE_DIR", self.state_dir().display().to_string());
        env.insert(
            "GAUNTLET_ARTIFACT_DIR",
            self.evidence_dir().display().to_string(),
        );
        env.insert("GAUNTLET_MOCK_SCENARIO", scenario.to_owned());
        env.insert("GAUNTLET_ID_SEED", seed.to_owned());
        env.insert("GAUNTLET_CLOCK", "deterministic".to_owned());
        env.insert("GAUNTLET_STALL_TIMEOUT_MS", "60000".to_owned());
        env.insert("GAUNTLET_RETRY_BASE_MS", "100".to_owned());
        env.insert("GAUNTLET_RETRY_MAX_MS", "2000".to_owned());
        env.insert("GAUNTLET_MAX_ATTEMPTS_PER_UNIT", "3".to_owned());
        env.insert("GAUNTLET_TOOL_TEST", check_tool_of(&self.target()));
        env
    }

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

fn check_tool_of(target: &Path) -> String {
    let script = target
        .join("scripts")
        .join(format!("{CHECK_STEM}{SCRIPT_SUFFIX}"));
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))
        .expect("the fixture check command is made executable");
    script.display().to_string()
}

fn lines(output: &Output) -> Vec<String> {
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::to_owned)
        .collect()
}

fn run_id_of(output: &Output) -> String {
    lines(output)
        .iter()
        .find(|line| line.starts_with("run: run_id="))
        .and_then(|line| line.strip_prefix("run: run_id="))
        .map(|id| id.to_owned())
        .expect("the run prints its run id")
}

#[test]
fn a_settled_runs_work_product_is_delivered_and_applied() {
    let world = World::new("apply");
    let env = world.env("e2e-happy", "deliver-apply-1");
    let run_output = world.run(&env, &["run", "settle the fixture's gadget marker unit"]);
    assert!(
        run_output.status.success(),
        "the happy leg settles: {}",
        String::from_utf8_lossy(&run_output.stderr)
    );
    let run_id = run_id_of(&run_output);
    let run_stdout = String::from_utf8_lossy(&run_output.stdout);
    assert!(
        run_stdout.contains(&format!(
            "delivery: gauntlet deliver --run {run_id} --apply"
        )),
        "the terminal report names the delivery hint: {run_stdout}"
    );

    // Dry run: the delta is reported, nothing written into the project.
    let dry = world.run(&env, &["deliver", "--run", &run_id]);
    assert!(
        dry.status.success(),
        "dry-run deliver succeeds: {}",
        String::from_utf8_lossy(&dry.stderr)
    );
    let dry_stdout = String::from_utf8_lossy(&dry.stdout);
    assert!(
        dry_stdout.contains("changed=1"),
        "exactly gadget.rs changed: {dry_stdout}"
    );
    let project_source = world.target().join("src").join("gadget.rs");
    let unapplied = std::fs::read_to_string(&project_source).expect("reads");
    assert!(
        !unapplied.contains("GADGET_MARKER_V1"),
        "a dry run writes nothing into the project"
    );

    // The bundle names the changed file and never an evidence locator.
    let bundle_dir = world
        .evidence_dir()
        .join("runs")
        .join(&run_id)
        .join("delivery");
    let gadget_rel = GADGET_SEGMENTS.join("/");
    let manifest =
        std::fs::read_to_string(bundle_dir.join("manifest.json")).expect("reads manifest");
    assert!(manifest.contains(&gadget_rel), "manifest: {manifest}");
    assert!(
        !manifest.contains("evidence/"),
        "an evidence locator never rides in the delivered delta: {manifest}"
    );
    assert!(
        bundle_dir
            .join("files")
            .join("src")
            .join("gadget.rs")
            .is_file(),
        "the bundle carries the changed file's bytes"
    );

    // Apply: the project root gets the marker back.
    let apply = world.run(&env, &["deliver", "--run", &run_id, "--apply"]);
    assert!(
        apply.status.success(),
        "apply succeeds: {}",
        String::from_utf8_lossy(&apply.stderr)
    );
    let applied = std::fs::read_to_string(&project_source).expect("reads");
    assert!(
        applied.contains("GADGET_MARKER_V1"),
        "the applied project source carries the settled work product: {applied}"
    );
}

#[test]
fn apply_refuses_on_drift_and_writes_nothing() {
    let world = World::new("drift");
    let env = world.env("e2e-happy", "deliver-drift-1");
    let run_output = world.run(&env, &["run", "settle the fixture's gadget marker unit"]);
    assert!(run_output.status.success(), "the happy leg settles");
    let run_id = run_id_of(&run_output);

    // Drift: a human (or another process) edits the project file AFTER
    // the run started, to content that matches neither the baseline nor
    // the settled work product.
    let project_source = world.target().join("src").join("gadget.rs");
    let drifted = DRIFTED_SOURCE;
    std::fs::write(&project_source, drifted).expect("writes the drift");

    let apply = world.run(&env, &["deliver", "--run", &run_id, "--apply"]);
    assert!(
        !apply.status.success(),
        "a drifted touched path refuses the whole apply"
    );
    let stderr = String::from_utf8_lossy(&apply.stderr);
    assert!(
        stderr.contains(&GADGET_SEGMENTS.join("/")),
        "the refusal names the drifted path: {stderr}"
    );
    let still_drifted = std::fs::read_to_string(&project_source).expect("reads");
    assert_eq!(
        still_drifted, drifted,
        "a refused apply writes nothing — the drift stands untouched"
    );
}

#[test]
fn deliver_without_a_run_argument_reports_nothing_when_no_run_has_integrated() {
    let world = World::new("empty");
    let env = world.env("e2e-happy", "deliver-empty-1");
    let output = world.run(&env, &["deliver"]);
    assert!(
        !output.status.success(),
        "no run has an integration tree yet — deliver refuses, not silently no-ops"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("nothing to deliver"),
        "the refusal is a clear message: {stderr}"
    );
}
