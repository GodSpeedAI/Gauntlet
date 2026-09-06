//! E2E: the FULL Gauntlet loop through the REAL `gauntlet` binary on the
//! REAL generic CLI adapter (`cli-agent`) under the LIVE containment
//! backend (plan Task 25; ruler AK-066, plus real-world confirmation of
//! AK-044, AK-047, AK-048).
//!
//! HONEST POSTURE STATEMENT (the charter's UNK-003 resolution, recorded):
//! the "real AgentRunner" here is the generic CLI adapter — a REAL runner
//! proven by P8 (AK-013/AK-014) — driven by a deterministic fixture child
//! (`agent/child.sh` in the fixture project). A deterministic child under
//! the real adapter is NOT a frontier-model claim and must never be
//! worded as one: the adapter, transport, contract delivery, poll
//! classification, deadline and cancel mechanics are the real production
//! code paths; the child's "intelligence" is fixture data.
//!
//! The test copies the seeded real-runner fixture target project
//! (`tests/fixtures/target-real`: its own GAUNTLET.md selecting
//! `runners.default: cli-agent` and a live process-isolation sandbox
//! backend, its own reference + frozen lock, its own tier policies, its
//! real check script, its deterministic child) to a per-test scratch
//! world and drives the composed production path from freeze to report
//! through child processes. Every assertion is RECORD-DERIVED (what the
//! commands print and what the durable stores hold), never a claim about
//! internals.
//!
//! The auto-closable dispatchable units settle (each dispatch WRAPPED
//! from process start — the wrapped child observes its own confinement,
//! the wrap verdict is bound to the dispatching unit's own artifact, and
//! the durable record states the dispatch's real tier). The
//! `irreversible` unit is never an automatic dispatch candidate (AK-044):
//! it stands in the escalation queue, and the run's honest terminal is
//! the §11.6 `blocked` — a `settled` claim over an escalated unit would
//! be a lie. A policy-validation failure escalates without unsandboxed
//! fallback (AK-047). The report classification derives from the records.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

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

/// The real-runner fixture target project, resolved, never spelled.
fn fixture_target_real() -> PathBuf {
    workspace_root()
        .join("tests")
        .join("fixtures")
        .join("target-real")
}

/// One scratch world: a fresh copy of the fixture project plus the
/// per-run state/evidence directories under it. Removed on drop.
struct World {
    root: PathBuf,
}

impl World {
    fn new(tag: &str) -> Self {
        let root =
            std::env::temp_dir().join(format!("gauntlet-e2e-real-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("the scratch root is creatable");
        let source = fixture_target_real();
        assert!(
            source.is_dir(),
            "the real-runner fixture target project exists at {}",
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

    /// Creates the state/evidence dirs up front — plus the workspace
    /// subdirectory the composition-time wrap check scopes `$WORKSPACE_ROOT`
    /// into (the policy's bind source must exist when the live backend
    /// dry-runs the policy; the compose_teeth precedent).
    fn ensure_layout(&self) {
        std::fs::create_dir_all(self.state_dir()).expect("state dir");
        std::fs::create_dir_all(self.evidence_dir()).expect("evidence dir");
        std::fs::create_dir_all(self.state_dir().join("workspaces").join("workspaces"))
            .expect("the wrap-time workspace directory");
    }

    /// The command environment for one leg: the fixture's own runner
    /// selection (`cli-agent`) with the template aimed at the world's own
    /// deterministic child, the world's own check script as the test
    /// instrument, and the required control settings.
    fn env(&self, seed: &str) -> BTreeMap<&'static str, String> {
        let mut env = BTreeMap::new();
        env.insert("GAUNTLET_STATE_DIR", self.state_dir().display().to_string());
        env.insert(
            "GAUNTLET_ARTIFACT_DIR",
            self.evidence_dir().display().to_string(),
        );
        env.insert("GAUNTLET_ID_SEED", seed.to_owned());
        // The required control settings (the Annex E hooks). Generous
        // here: the healthy legs' children complete in milliseconds; only
        // the budget target tightens these.
        env.insert("GAUNTLET_STALL_TIMEOUT_MS", "60000".to_owned());
        env.insert("GAUNTLET_RETRY_BASE_MS", "100".to_owned());
        env.insert("GAUNTLET_RETRY_MAX_MS", "2000".to_owned());
        env.insert("GAUNTLET_MAX_ATTEMPTS_PER_UNIT", "3".to_owned());
        // The command template: the world's own deterministic child, made
        // executable here (the proof never leans on checkout exec-bit
        // semantics — the mock E2E's precedent for its check script). The
        // script is DERIVED from the fixture's own agent dir, never
        // spelled (AK-016).
        let child = sole_script_of(&self.target().join("agent"), "agent");
        make_executable(&child);
        env.insert(
            "GAUNTLET_CLI_AGENT_CMD",
            format!(
                "{} {{prompt_file}} {{workspace}} {{result_path}}",
                child.display()
            ),
        );
        // The `test` facet's tool binding: THIS world's own check command,
        // executable here, run by the run's own instrument over the
        // workspace where the dispatched work landed.
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

fn make_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
        .expect("the fixture script is made executable");
}

/// The fixture target's check command as a tool binding: derived, never
/// spelled (AK-016), made executable for the run's test instrument.
fn check_tool_of(target: &Path) -> String {
    let script = sole_script_of(&target.join("scripts"), "scripts");
    make_executable(&script);
    script.display().to_string()
}

/// The directory's single script file, DERIVED from the fixture's own
/// layout — exactly the mock E2E's check-script pattern (AK-016: the
/// command's name is the fixture's own content, never a spelled path).
fn sole_script_of(dir: &Path, what: &str) -> PathBuf {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap_or_else(|error| panic!("the fixture {what} dir reads: {error}"))
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .collect();
    files.sort();
    assert_eq!(
        files.len(),
        1,
        "the fixture {what} dir carries exactly one script: {files:?}"
    );
    files.remove(0)
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

/// Every durable export-record line of the run (the export stream, one
/// JSON record per line) — the record-derived way to assert what the run
/// durably holds, never a report's say-so.
fn export_lines(world: &World, env: &BTreeMap<&'static str, String>, run_id: &str) -> Vec<String> {
    let export = world.run(env, &["export", "--run", run_id]);
    assert!(
        export.status.success(),
        "the export leg succeeds: {}",
        String::from_utf8_lossy(&export.stderr)
    );
    String::from_utf8_lossy(&export.stdout)
        .lines()
        .map(str::to_owned)
        .collect()
}

/// Whether any object in the evidence store contains `needle` (the
/// record-derived reader over the content-addressed `objects/` tree).
fn store_holds_text_containing(evidence_dir: &Path, needle: &str) -> bool {
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
                return true;
            }
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Leg 1 — the happy loop on the real adapter under the live backend: the
// auto-closable units (the contained and the local unit) settle, the
// cross_cutting dispatch is WRAPPED (the child observed its own
// confinement, bound to AK-952's own artifact; the durable invocation
// record states the dispatch's real tier), and the irreversible unit is
// never dispatched (AK-044).
//
// HONEST TERMINAL, OBSERVED AND RECORDED: the cross_cutting unit's
// authority is `lead_agent` (the §5.3.2 coupling forbids critic authority
// on cross_cutting outright), and §16.1 condition 3 forbids AUTOMATIC
// closure of lead_agent work — the runtime's settlement is refused and
// ROUTED to the operator (§11.5 row 6). The unit therefore never settles
// unattended, its bounded re-dispatches exhaust the run's rounds, and the
// run's honest terminal is `budget_exhausted` — never a settled claim
// over authority the run cannot close. This is the frozen semantics
// working, pinned here as the real-world answer to AK-066's "settles";
// the divergence (a cross_cutting unit in a seeded reference cannot
// settle unattended) is recorded for the ruler's owner in the task
// report.
// ---------------------------------------------------------------------------

#[test]
fn the_composed_loop_settles_the_auto_closable_units_wraps_the_cross_cutting_dispatch_and_escalates_the_rest()
 {
    let world = World::new("happy");
    let env = world.env("real-happy-seed");

    // THE RUN: freeze → dispatch → observe → verify → settle → report.
    let run = world.run(&env, &["run", "seeded bounded task"]);
    let stdout = String::from_utf8_lossy(&run.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&run.stderr).into_owned();
    assert!(
        !run.status.success(),
        "the run cannot settle a lead_agent unit unattended: stdout={stdout}\nstderr={stderr}"
    );
    assert!(
        stdout.contains("terminal=budget_exhausted"),
        "the bounded honest terminal (the closure-escalated unit's re-dispatches exhaust the \
         rounds): {stdout}\n{stderr}"
    );
    let run_id = run_id_of(&run);

    // The report classification, derived from the records: the
    // auto-closable units (contained, local) are PROVEN; the
    // cross_cutting unit (dispatched, closure routed to the operator)
    // and the irreversible unit (never dispatched) are NOT.
    let report = report_json_of(&run);
    use gauntlet_ports::json::JsonValue;
    let parsed = gauntlet_ports::json::parse(&report).expect("the printed report is valid JSON");
    let units = match parsed.get("units") {
        Some(JsonValue::Array(units)) => units,
        other => panic!("the report carries the units array: {other:?}"),
    };
    let category_of = |unit: &str| -> Option<String> {
        units
            .iter()
            .find(|entry| matches!(entry.get("unit"), Some(JsonValue::Str(name)) if name == unit))
            .and_then(|entry| entry.get("category"))
            .and_then(|value| match value {
                JsonValue::Str(token) => Some(token.as_str().to_owned()),
                _ => None,
            })
    };
    for unit in ["AK-951", "AK-954"] {
        assert_eq!(
            category_of(unit).as_deref(),
            Some("implemented-and-proven"),
            "the auto-closable unit {unit} is proven: {report}"
        );
    }
    for unit in ["AK-952", "AK-953"] {
        assert_ne!(
            category_of(unit).as_deref(),
            Some("implemented-and-proven"),
            "unit {unit} is never settled unattended: {report}"
        );
    }

    // AK-048, the WRAP REALITY: the cross_cutting child ran INSIDE the
    // sandbox and observed it — pid 1 of its process namespace was the
    // sandbox itself, and writes outside the writable workspace root were
    // denied by the read-only baseline. The child computed the verdict
    // from those two facts and the verdict rides its locator artifact
    // into the durable evidence store.
    assert!(
        store_holds_text_containing(&world.evidence_dir(), "confinement=confined"),
        "the wrapped child observed confinement in its own process namespace"
    );
    assert!(
        store_holds_text_containing(&world.evidence_dir(), "tmp-write=denied"),
        "the wrapped child observed the read-only baseline deny out-of-root writes"
    );
    // THE UNIT BINDING (corrections round 1, F2): the wrapped-execution
    // facts are asserted ON THE CROSS_CUTTING UNIT'S OWN ARTIFACT. The
    // AK-951 (contained) child runs under the same live backend and the
    // same template, so the whole-store scans above alone cannot
    // distinguish a wrapped AK-952 dispatch from an unwrapped one — a
    // tier-conditional seam mutation (wrap contained, spawn cross_cutting
    // verbatim) kept both legs green against those scans alone. The
    // AK-952 child-work-record itself must carry the confinement verdict
    // and the read-only baseline's denial.
    let ak952_artifact = records_containing(&world.evidence_dir(), "\"unit\":\"AK-952\"")
        .into_iter()
        .find(|record| record.contains("\"kind\":\"child-work-record\""))
        .unwrap_or_else(|| panic!("the AK-952 dispatch's own locator artifact is pinned"));
    assert!(
        ak952_artifact.contains("confinement=confined")
            && ak952_artifact.contains("tmp-write=denied"),
        "the AK-952 (cross_cutting) child ran INSIDE the sandbox and observed it — its own \
         artifact must carry the verdict: {ak952_artifact}"
    );

    // The durable projection: exactly the auto-closable units' (the
    // contained and the local unit's) settlements — per-tier, from the
    // status surface.
    let status = world.run(&env, &["status", "--run", &run_id]);
    let status_text = String::from_utf8_lossy(&status.stdout).into_owned();
    for tier_line in [
        "tier=contained units=1 settled=1",
        "tier=local units=1 settled=1",
        "tier=cross_cutting units=1 settled=0",
    ] {
        assert!(
            status_text.contains(tier_line),
            "exactly the auto-closable units settle, per tier ({tier_line}): {status_text}"
        );
    }

    // The durable invocation records: the cross_cutting dispatch CARRIES
    // the admitted policy ref — the wrap verdict in the durable record
    // (the gate admitted it with a resolved, validated policy) — AND the
    // record's own tier field names the dispatch's REAL tier
    // (corrections round 1, F3: the record used to hardcode `contained`
    // in RecordingAgentRunner::admitted, misstating every cross_cutting
    // dispatch; the durable record must not lie about the tier). The
    // irreversible unit was NEVER dispatched.
    let exported = export_lines(&world, &env, &run_id).join("\n");
    let cross_dispatch = exported
        .lines()
        .find(|line| line.contains("\"record\":\"role_invocation\"") && line.contains("AK-952"))
        .unwrap_or_else(|| panic!("AK-952 was dispatched: {exported}"));
    assert!(
        cross_dispatch.contains("tier-cross-cutting-real-v1"),
        "the durable invocation record carries the dispatch's admitted policy: \
         {cross_dispatch}"
    );
    assert!(
        cross_dispatch.contains("\"tier\":\"cross_cutting\""),
        "the durable invocation record states the dispatch's REAL tier, not a hardcoded \
         default: {cross_dispatch}"
    );
    // The local unit (corrections round 1, F8 — the fixture carries a
    // unit for every dispatchable tier up to cross_cutting): it
    // DISPATCHES, its durable record states the dispatch's real tier, it
    // SETTLES, and its own locator artifact is PINNED evidence carrying
    // the child's inside-the-sandbox verdict — the same shape AK-951's
    // proof takes.
    let local_dispatch = exported
        .lines()
        .find(|line| line.contains("\"record\":\"role_invocation\"") && line.contains("AK-954"))
        .unwrap_or_else(|| panic!("the local unit was dispatched: {exported}"));
    assert!(
        local_dispatch.contains("\"tier\":\"local\""),
        "the local dispatch's durable record states its real tier: {local_dispatch}"
    );
    assert!(
        exported
            .lines()
            .any(|line| line.contains("\"record\":\"settlement\"")
                && line.contains("\"unit_id\":\"AK-954\"")),
        "the local unit settled on the durable record: {exported}"
    );
    assert!(
        exported
            .lines()
            .any(|line| line.contains("\"record\":\"settlement\"")
                && line.contains("\"unit_id\":\"AK-951\"")),
        "the contained unit settled on the durable record: {exported}"
    );
    let ak954_artifact = records_containing(&world.evidence_dir(), "\"unit\":\"AK-954\"")
        .into_iter()
        .find(|record| record.contains("\"kind\":\"child-work-record\""))
        .unwrap_or_else(|| panic!("the AK-954 dispatch's own locator artifact is pinned"));
    assert!(
        ak954_artifact.contains("confinement=confined")
            && ak954_artifact.contains("tmp-write=denied"),
        "the local-tier child ran inside the sandbox and observed it: {ak954_artifact}"
    );
    for record_line in exported.lines() {
        assert!(
            !(record_line.contains("\"record\":\"role_invocation\"")
                && record_line.contains("AK-953")),
            "the irreversible unit was never dispatched: {record_line}"
        );
    }
    assert!(
        exported.contains("\"record\":\"settlement\""),
        "a settlement stands"
    );

    // The gate admission evidence (pinned BEFORE the invocation existed,
    // AK-048): the cross_cutting admission names the tier, the resolved
    // policy, and the validation rung the wrap consumed.
    let admission = records_containing(&world.evidence_dir(), "\"class\":\"gate_admission\"")
        .into_iter()
        .find(|record| record.contains("AK-952"))
        .unwrap_or_else(|| panic!("AK-952's gate admission is pinned: {exported}"));
    assert!(
        admission.contains("cross_cutting")
            && admission.contains("tier-cross-cutting-real-v1")
            && admission.contains("process"),
        "the admission names tier, policy, and the validation rung: {admission}"
    );

    // The §16.1 condition-3 routing, real-world: the cross_cutting
    // unit's closure was refused for the runtime actor and ESCALATED.
    let escalations = world.run(&env, &["escalations", "list"]);
    let escalations_text = String::from_utf8_lossy(&escalations.stdout).into_owned();
    assert!(
        escalations_text.contains("AK-952") && escalations_text.contains("lead_agent"),
        "the cross_cutting unit's closure is routed to the operator: {escalations_text}"
    );
    // AK-044's queue presence in the SAME run as the never-dispatched
    // scan (corrections round 1, F9): the irreversible unit's ticket
    // stands in the queue with its typed blocker — the scheduler's
    // unconditional refusal is a fact the operator must see whatever
    // terminal the run reaches, not a fact only a `blocked` close mints.
    let ak953 = escalations_text
        .lines()
        .find(|line| line.contains("AK-953"))
        .unwrap_or_else(|| {
            panic!("AK-953's escalation ticket stands in this run's queue: {escalations_text}")
        });
    assert!(
        ak953.contains("irreversible") && ak953.contains("never an automatic dispatch"),
        "the irreversible unit's ticket names its typed blocker: {ak953}"
    );
}

/// Every pinned evidence document whose text contains `needle` (the
/// record-derived reader over the content-addressed objects tree).
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

// ---------------------------------------------------------------------------
// Leg 2 — AK-047 + the governance close: a policy-validation failure
// ESCALATES (never an unsandboxed fallback, never a degraded run), and
// the irreversible unit's own escalation ticket stands in the queue —
// the §11.6 governance close the frozen controller reaches once nothing
// budget-blocked remains owed.
// ---------------------------------------------------------------------------

#[test]
fn a_policy_validation_failure_escalates_and_never_runs_unsandboxed() {
    let world = World::new("ak047");
    let env = world.env("real-ak047-seed");

    // THE BREAK, in the scratch world only (probe hygiene — the repo's
    // own .gauntlet/policies is never touched): the cross_cutting policy
    // is rewritten to name the conservative no-backend token — a backend
    // the live posture does not front. The policy RESOLVES (the artifact
    // decodes) and FAILS backend validation — the exact escalation arm
    // AK-047 names: a failed validation authorizes nothing, and the
    // runtime never falls back to unsandboxed.
    let policy_path = world
        .target()
        .join(".gauntlet")
        .join("policies")
        .join("cross_cutting.json");
    let policy = std::fs::read_to_string(&policy_path).expect("the tier policy reads");
    let backend_field = "\"backend\": \"";
    let value_start = policy
        .find(backend_field)
        .expect("the policy names its backend");
    let value_start = value_start + backend_field.len();
    let value_end = value_start + policy[value_start..].find('"').expect("a quoted token");
    let broken = format!(
        "{}{}{}",
        &policy[..value_start],
        gauntlet_app::config::defaults::SANDBOX_BACKEND,
        &policy[value_end..]
    );
    assert_ne!(policy, broken, "the scratch policy mutation landed");
    std::fs::write(&policy_path, broken).expect("the scratch policy is broken");

    let run = world.run(&env, &["run", "seeded bounded task"]);
    let stdout = String::from_utf8_lossy(&run.stdout).into_owned();
    assert!(
        !run.status.success(),
        "the validation-failure leg does not settle: stdout={stdout}"
    );
    assert!(
        stdout.contains("terminal=blocked"),
        "the §11.6 governance close: {stdout}"
    );

    // The escalations, by name: AK-952's typed validation escalation with
    // the never-fallback rule, and AK-953's own ticket — the irreversible
    // unit IS in the escalation queue (AK-044), scheduler-level and
    // unconditional.
    let escalations = world.run(&env, &["escalations", "list"]);
    let escalations_text = String::from_utf8_lossy(&escalations.stdout).into_owned();
    let ak952 = escalations_text
        .lines()
        .find(|line| line.contains("AK-952"))
        .unwrap_or_else(|| panic!("AK-952's escalation ticket stands: {escalations_text}"));
    assert!(
        ak952.contains("failed validation") && ak952.contains("never an unsandboxed fallback"),
        "the escalation is the typed AK-047 one: {ak952}"
    );
    let ak953 = escalations_text
        .lines()
        .find(|line| line.contains("AK-953"))
        .unwrap_or_else(|| panic!("AK-953's escalation ticket stands: {escalations_text}"));
    assert!(
        ak953.contains("irreversible") && ak953.contains("never an automatic dispatch"),
        "the irreversible unit's ticket names its typed blocker: {ak953}"
    );

    // Nothing unsandboxed ran: the only dispatches on the durable record
    // are the units whose policies stayed valid — the contained unit and
    // the local unit (corrections round 1, F8; each durable record also
    // states its real tier). The unit whose policy was broken (AK-952)
    // never dispatched: the validation failure authorized nothing, and
    // there is no fallback arm.
    let run_id = run_id_of(&run);
    let exported = export_lines(&world, &env, &run_id).join("\n");
    let dispatches: Vec<&str> = exported
        .lines()
        .filter(|line| line.contains("\"record\":\"role_invocation\""))
        .collect();
    assert_eq!(
        dispatches.len(),
        2,
        "exactly the intact-policy dispatches ran: {exported}"
    );
    for (unit, tier) in [
        ("AK-951", "\"tier\":\"contained\""),
        ("AK-954", "\"tier\":\"local\""),
    ] {
        let dispatch = dispatches
            .iter()
            .find(|line| line.contains(unit))
            .unwrap_or_else(|| panic!("the intact-policy unit {unit} dispatched: {exported}"));
        assert!(
            dispatch.contains(tier),
            "the dispatched record states the unit's real tier ({unit}): {dispatch}"
        );
    }
    for record_line in &dispatches {
        assert!(
            !record_line.contains("AK-952"),
            "the broken-policy unit never dispatched: {record_line}"
        );
    }
    let status = world.run(&env, &["status", "--run", &run_id]);
    let status_text = String::from_utf8_lossy(&status.stdout).into_owned();
    for tier_line in [
        "tier=contained units=1 settled=1",
        "tier=local units=1 settled=1",
    ] {
        assert!(
            status_text.contains(tier_line),
            "exactly the intact-policy units settle ({tier_line}): {status_text}"
        );
    }
}
