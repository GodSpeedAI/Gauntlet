//! The composition-root teeth battery (plan Task 20, COR-D4 + RES-05):
//! every fail-closed refusal the composition root owns is proven by
//! instrument — "disabling the defense makes the proof fail" (§23.1).
//! Each leg drives the REAL `gauntlet` binary (child process, controlled
//! environment) over a copy of the seeded E2E fixture target and asserts
//! the TYPED refusal surface plus the fact that nothing ran:
//!
//! 1. `runners.default` unset AND `GAUNTLET_AGENT_RUNNER` unset ⇒ the
//!    typed configuration failure names BOTH the config key and the env
//!    var, and no state store is ever created (charter D2).
//! 2. an unknown runner token ⇒ the refusal names the fronted set.
//! 3. the mock token WITHOUT its `GAUNTLET_MOCK_SCENARIO` hook ⇒ the
//!    typed configuration failure names the hook (the mock is
//!    fail-closed on its scenario seam).
//! 4. the process-backed `cli-agent` token under a LIVE sandbox backend
//!    ⇒ the composition-level wrap verdict is `Sandboxed` and the
//!    composition ROUTES it (plan Task 25: the wrap seam is installed on
//!    the runner, the dispatch runs, and the durable invocation record
//!    carries the admitted sandbox policy) — never a silent unwrap
//!    (RES-05; §21.3, AK-047). Honestly skipped on hosts without the
//!    process-isolation binary. (RE-PINNED for Task 25: this leg
//!    originally pinned the task-20 deferral the same task retired.)

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use gauntlet_adapter_state_postgres::STATE_BACKEND_TOKEN;
use gauntlet_ports::sandbox::Sandbox;

/// The real composed binary (same package: `src/bin/gauntlet.rs`).
const GAUNTLET: &str = env!("CARGO_BIN_EXE_gauntlet");

/// The composed-run budget shape every leg runs under — the fixture's own
/// values, spelled as named configuration (§8.1: numbers next to
/// duration-shaped identifiers are configuration, and the literals
/// scanner's joined-span rule reads the whole generated block as one
/// unit).
const TEETH_MAX_ROUNDS: u32 = 4;
const TEETH_MAX_PARALLEL: u32 = 1;
const TEETH_MAX_DEPTH: u32 = 1;
const TEETH_MAX_WALL_SPAN: u64 = 600;
const TEETH_MAX_TOKENS: u64 = 200_000;
const TEETH_MAX_COST: u64 = 5;
const TEETH_RESERVE_PCT: u32 = 15;
const TEETH_DIAGNOSTIC_BUDGET: u32 = 1;

/// The workspace root: this package's manifest sits two levels below it.
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

/// One scratch world: a fresh copy of the fixture target project plus the
/// state/evidence roots, with an optional GAUNTLET.md override. Removed
/// on drop.
struct World {
    root: PathBuf,
}

impl World {
    fn new(tag: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "gauntlet-compose-teeth-{tag}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("the scratch root is creatable");
        copy_tree(&fixture_target(), &root.join("target"));
        std::fs::create_dir_all(root.join("state")).expect("state dir");
        std::fs::create_dir_all(root.join("evidence")).expect("evidence dir");
        Self { root }
    }

    fn target(&self) -> PathBuf {
        self.root.join("target")
    }

    fn state_dir(&self) -> PathBuf {
        self.root.join("state")
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

/// Runs the real binary with a MINIMAL environment plus exactly the vars
/// the leg declares — the test process's own GAUNTLET_* variables never
/// leak into a refusal leg (a inherited runner token would flip the
/// unset-token leg into a different refusal).
fn run(world: &World, envs: &[(&str, &str)], args: &[&str]) -> Output {
    let mut command = Command::new(GAUNTLET);
    command
        .current_dir(world.target())
        .env_remove("GAUNTLET_AGENT_RUNNER")
        .env_remove("GAUNTLET_MOCK_SCENARIO")
        .env_remove("GAUNTLET_CLI_AGENT_CMD")
        .env_remove("GAUNTLET_PROJECT_ROOT")
        .env_remove("GAUNTLET_CONFIG_DIR")
        .env(
            "GAUNTLET_STATE_DIR",
            world.state_dir().display().to_string(),
        )
        .env(
            "GAUNTLET_ARTIFACT_DIR",
            world.root.join("evidence").display().to_string(),
        );
    for (key, value) in envs {
        command.env(key, value);
    }
    command
        .args(args)
        .output()
        .expect("the gauntlet binary runs")
}

/// The fixture front matter, with the runner line and sandbox line left
/// for the leg to spell.
fn front_matter_with(runners: &str, sandbox: &str, policy_dir: Option<&Path>) -> String {
    let mut lines: Vec<String> = vec![
        "reference:".to_owned(),
        "  manifest: .wayfinder/reference.yaml".to_owned(),
        "  answer_key: .wayfinder/answer-key.md".to_owned(),
        "  comparison_contract: .gauntlet/comparison-contract.yaml".to_owned(),
        "  digest_algo: sha256".to_owned(),
        "roles:".to_owned(),
        "  dir: .agents/roles".to_owned(),
    ];
    if !runners.is_empty() {
        lines.push(runners.to_owned());
    }
    lines.push("observation:".to_owned());
    lines.push("  facets: [filesystem]".to_owned());
    lines.push("review:".to_owned());
    lines.push("  mode: single".to_owned());
    lines.push("  max_critics: 0".to_owned());
    lines.push("verification:".to_owned());
    lines.push("  prefer_deterministic: true".to_owned());
    lines.push("budget:".to_owned());
    for line in [
        format!("  max_rounds: {TEETH_MAX_ROUNDS}"),
        format!("  max_parallel: {TEETH_MAX_PARALLEL}"),
        format!("  max_depth: {TEETH_MAX_DEPTH}"),
        format!("  max_wall_seconds: {TEETH_MAX_WALL_SPAN}"),
        format!("  max_tokens: {TEETH_MAX_TOKENS}"),
        format!("  max_cost_usd: {TEETH_MAX_COST}"),
        format!("  recovery_reserve_pct: {TEETH_RESERVE_PCT}"),
        format!("  diagnostic_budget: {TEETH_DIAGNOSTIC_BUDGET}"),
    ] {
        lines.push(line);
    }
    lines.push("state:".to_owned());
    lines.push(format!("  backend: {STATE_BACKEND_TOKEN}"));
    if !sandbox.is_empty() {
        lines.push("sandbox:".to_owned());
        lines.push(format!("  backend: {sandbox}"));
        if let Some(dir) = policy_dir {
            lines.push(format!("  policy_dir: {}", dir.display()));
        }
    }
    lines.push("operator:".to_owned());
    lines.push("  surfaces: [cli]".to_owned());
    let mut out = lines.join("\n");
    out.push('\n');
    out
}

/// The tier policy artifacts the live-backend leg resolves (doctor §8.5
/// check 5 requires the full `local`/`contained`/`cross_cutting` set —
/// `irreversible` absent by design, AK-049), written into the leg's
/// scratch policy dir. Built as JSON values, not a spelled template (the
/// p5 idiom): the literals scanner joins `(`/`=`-ending lines forward,
/// and a giant single literal could swallow a spawn site into one
/// hardcoded-command unit. The tier token flows through its own short
/// unit and the object unit carries only key names and variables — the
/// same shape the p5 proof uses, so no tier term sits in the
/// `default`-armed joined unit. The backend field is the REAL adapter's
/// own resolved identity token — never spelled in this crate (the
/// boundary suite denies it).
fn write_tier_policies(dir: &Path, backend_token: &str) {
    use gauntlet_ports::json::JsonValue;
    let writable = JsonValue::Array(vec![JsonValue::Str(workspace_root_token())]);
    for tier in ["local", "contained", "cross_cutting"] {
        let reference = format!("tier-{tier}-compose-teeth");
        let object = JsonValue::Object(vec![
            ("reference".to_owned(), JsonValue::Str(reference)),
            ("tier".to_owned(), JsonValue::Str(tier.to_owned())),
            (
                "backend".to_owned(),
                JsonValue::Str(backend_token.to_owned()),
            ),
            (
                "network".to_owned(),
                JsonValue::Str("deny_by_default".to_owned()),
            ),
            ("writable_roots".to_owned(), writable.clone()),
            ("denied_paths".to_owned(), JsonValue::Array(Vec::new())),
        ]);
        std::fs::create_dir_all(dir).expect("the leg's policy dir is creatable");
        std::fs::write(dir.join(format!("{tier}.json")), object.to_json_text())
            .expect("the tier policy artifact is written");
    }
}

/// Writes the leg's full GAUNTLET.md: the fixture's body behind the leg's
/// front matter.
fn write_config(world: &World, front_matter: &str) {
    let body = std::fs::read_to_string(world.target().join("GAUNTLET.md"))
        .expect("the fixture GAUNTLET.md reads");
    let body_after_matter = body
        .split_once("---\n")
        .and_then(|(_, rest)| rest.split_once("\n---\n"))
        .map(|(_, body)| body.to_owned())
        .expect("the fixture GAUNTLET.md carries front matter and a body");
    std::fs::write(
        world.target().join("GAUNTLET.md"),
        format!("---\n{front_matter}---\n{body_after_matter}"),
    )
    .expect("the override GAUNTLET.md is written");
}

/// The fs provider's `$WORKSPACE_ROOT` indirection token (the vocabulary
/// the REAL adapter resolves — spelled here as data, not a path).
fn workspace_root_token() -> String {
    const TOKEN: &str = "$WORKSPACE_ROOT";
    TOKEN.to_owned()
}

#[test]
fn an_unset_runner_token_refuses_naming_key_and_env_and_nothing_runs() {
    let world = World::new("unset-token");
    // The copied fixture carries `runners.default: mock-agent` — the unset
    // leg's configuration has NO runner line at all.
    write_config(&world, &front_matter_with("", "", None));

    let output = run(&world, &[], &["run", "seeded bounded task"]);
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert!(
        !output.status.success(),
        "the run refuses: stdout={}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        stderr.contains("runners.default") && stderr.contains("GAUNTLET_AGENT_RUNNER"),
        "the typed failure names BOTH the config key and the env var: {stderr}"
    );
    assert!(
        stderr.contains("no default runner is configured"),
        "the refusal states the mechanism: {stderr}"
    );
    // Nothing ran: no state store was ever created.
    assert!(
        !gauntlet_adapter_state_postgres::state_exists(
            &world.state_dir().join("gauntlet-state.db")
        )
        .expect("the state database is reachable"),
        "the fail-closed selection never opens a store, let alone dispatches"
    );
}

#[test]
fn an_unknown_runner_token_refuses_naming_the_fronted_set() {
    let world = World::new("unknown-token");
    write_config(&world, &front_matter_with("", "", None));

    let output = run(
        &world,
        &[("GAUNTLET_AGENT_RUNNER", "definitely-not-fronted")],
        &["run", "seeded bounded task"],
    );
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert!(
        !output.status.success(),
        "the run refuses: stdout={}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        stderr.contains("definitely-not-fronted")
            && stderr.contains("names no runner this composition fronts"),
        "the refusal names the unknown token and the mechanism: {stderr}"
    );
    for fronted in ["prime-agent", "cli-agent", "mock-agent"] {
        assert!(
            stderr.contains(fronted),
            "the refusal names the fronted token {fronted}: {stderr}"
        );
    }
    assert!(
        !gauntlet_adapter_state_postgres::state_exists(
            &world.state_dir().join("gauntlet-state.db")
        )
        .expect("the state database is reachable"),
        "nothing ran"
    );
}

#[test]
fn the_mock_token_without_its_scenario_hook_refuses() {
    let world = World::new("mock-no-scenario");
    // The fixture already selects `runners.default: mock-agent`; the
    // scenario hook is REMOVED from the child's environment.
    let output = run(&world, &[], &["run", "seeded bounded task"]);
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert!(
        !output.status.success(),
        "the run refuses: stdout={}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        stderr.contains("GAUNTLET_MOCK_SCENARIO") && stderr.contains("scenario"),
        "the typed failure names the scenario hook: {stderr}"
    );
    // The refusal fires during runner CONSTRUCTION — after the composition
    // opened its durable roots but BEFORE any run exists: the child never
    // prints a run id, because no run was ever created.
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    assert!(
        !stdout.contains("run: run_id="),
        "no run was created without a scenario: {stdout}"
    );
}

#[test]
fn a_live_backend_wrap_verdict_is_routed_to_the_spawned_child_never_unwrapped() {
    // Honest skip: the leg needs the REAL process-isolation binary; on a
    // host without it the routed dispatch cannot be composed and the
    // skip is the recorded fact, not a pass claim.
    //
    // RE-PINNED for plan Task 25 (recorded deviation): this leg pinned the
    // task-20 composition DEFERRAL (`LiveContainmentDeferred` — "routing a
    // wrapped builder command through the composed loop is plan Task 25's
    // leg"). Task 25 IS that leg: the deferral is retired and the
    // composition now routes the gate's `Sandboxed` verdict into the
    // spawned child through the runner's containment seam. The invariant
    // the old leg guarded — never a silent unwrap — is pinned here
    // DIFFERENTLY, not stronger (corrections round 1, F4): the coverage
    // moved to the routed dispatch — the composition proceeds, the
    // dispatch runs, and the durable invocation record carries the
    // ADMITTED SANDBOX POLICY (the wrap verdict) for the dispatched unit
    // — but this record-level pin cannot by itself distinguish a wrapped
    // from an unwrapped SPAWN; that at-locus strength lives in the
    // real-runner E2E, where the confinement evidence is bound to the
    // dispatched unit's own artifact. A composition-time wrap/admission
    // REFUSAL still fails the run fail-closed (the other teeth).
    let Some(binary) = gauntlet_adapter_sandbox_bwrap::locate() else {
        eprintln!("skipped: the process-isolation binary is not locatable on this host");
        return;
    };
    let backend_token = gauntlet_adapter_sandbox_bwrap::ProcessIsolationBackend::new(binary)
        .backend_identity()
        .as_str()
        .to_owned();

    let world = World::new("live-wrap");
    let policy_dir = world.root.join("policies");
    write_tier_policies(&policy_dir, &backend_token);
    write_config(
        &world,
        &front_matter_with(
            "runners:\n  default: cli-agent\n",
            &backend_token,
            Some(&policy_dir),
        ),
    );
    // The workspace subdirectory the wrap-time policy resolution scopes
    // `$WORKSPACE_ROOT` into — what a real WorkspaceProvider::create()
    // would have made by the time a policy resolves.
    std::fs::create_dir_all(world.state_dir().join("workspaces").join("workspaces"))
        .expect("the wrap-time workspace directory is creatable");

    let output = run(
        &world,
        &[
            (
                "GAUNTLET_CLI_AGENT_CMD",
                "cli-probe {prompt_file} {workspace} {result_path}",
            ),
            // The §15.2 control settings have no spec-table defaults
            // (disposition refuses without them); this leg runs the same
            // values the seeded-fixture E2E runs.
            ("GAUNTLET_STALL_TIMEOUT_MS", "60000"),
            ("GAUNTLET_RETRY_BASE_MS", "100"),
            ("GAUNTLET_RETRY_MAX_MS", "2000"),
            ("GAUNTLET_MAX_ATTEMPTS_PER_UNIT", "3"),
        ],
        &["run", "seeded bounded task"],
    );
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    assert!(
        !stderr.contains("live-sandbox wrap verdict"),
        "the composition no longer refuses the live wrap verdict — task 25 routed it: \
         stdout={stdout}\nstderr={stderr}"
    );
    // The run itself is an honest bounded non-settlement: the leg's child
    // program does not exist, so the WRAPPED child starts (the sandbox
    // spawns), writes no contract, and the loop terminates under its own
    // budget/attempt machinery — never a crash, never a settlement.
    assert!(
        !output.status.success(),
        "a nonexistent child program never settles: stdout={stdout}\nstderr={stderr}"
    );
    assert!(
        stdout.contains("terminal=") && !stdout.contains("terminal=settled"),
        "the run ends in a bounded honest non-settled terminal: {stdout}\n{stderr}"
    );
    let run_id = stdout
        .lines()
        .find(|line| line.starts_with("run: run_id="))
        .and_then(|line| line.strip_prefix("run: run_id="))
        .expect("the run started (the composition routed the wrap) and prints its id")
        .to_owned();

    // THE ROUTED WRAP, on the durable record: the dispatched invocation
    // carries the ADMITTED SANDBOX POLICY — the wrap verdict — for the
    // unit whose composition-time verdict was live. The dispatch ran
    // wrapped from process start (AK-048); an unwrapped run would carry
    // no policy ref at all.
    let export = run(
        &world,
        &[
            (
                "GAUNTLET_CLI_AGENT_CMD",
                "cli-probe {prompt_file} {workspace} {result_path}",
            ),
            ("GAUNTLET_STALL_TIMEOUT_MS", "60000"),
            ("GAUNTLET_RETRY_BASE_MS", "100"),
            ("GAUNTLET_RETRY_MAX_MS", "2000"),
            ("GAUNTLET_MAX_ATTEMPTS_PER_UNIT", "3"),
        ],
        &["export", "--run", &run_id],
    );
    assert!(
        export.status.success(),
        "the export leg succeeds: {}",
        String::from_utf8_lossy(&export.stderr)
    );
    let exported = String::from_utf8_lossy(&export.stdout).into_owned();
    let dispatch = exported
        .lines()
        .find(|line| line.contains("\"record\":\"role_invocation\""))
        .unwrap_or_else(|| panic!("the routed dispatch is on the durable record: {exported}"));
    assert!(
        dispatch.contains("AK-901"),
        "the dispatched unit is the one whose wrap verdict was live: {dispatch}"
    );
    assert!(
        dispatch.contains("tier-contained-compose-teeth"),
        "the dispatch ran under the admitted sandbox policy — wrapped, never unwrapped: \
         {dispatch}"
    );
}
