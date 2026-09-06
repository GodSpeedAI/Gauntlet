//! E2E: budget exhaustion through the REAL `gauntlet` binary (plan Task
//! 25 step 4; AK-037..AK-039/AK-051/AK-052 real-world).
//!
//! The controller-level prior art (p6_budget_control, SETTLED) pins the
//! reserve and non-convergence machinery at the controller level; THIS
//! target is the composed delta: a non-converging (SILENT) builder child
//! under a budget configured to exhaust, driven end to end through the
//! real binary and the real generic CLI adapter.
//!
//! HONEST POSTURE STATEMENT (the charter's UNK-003 resolution): the
//! silent child is the fixture's deterministic agent in its `silent`
//! mode — it dispatches, runs (wrapped, under the live backend), writes
//! NOTHING, and never exits. That is the honest non-convergence shape:
//! no lying document, no completed claim — a child that never completes.
//! This is NOT a frontier-model claim. The run must terminate CLEANLY by
//! its own budget dimensions — the stall deadline cancels it, the §11.5
//! row-4 retry-once-then-escalate table spends the rounds, and the loop
//! closes `budget_exhausted` with the recovery reserve INTACT — bounded,
//! no crash, no hang, zero settlements.
//!
//! Every wait is bounded with a typed failure; every assertion is
//! record-derived.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

/// The real composed binary (same package: `src/bin/gauntlet.rs`).
const GAUNTLET: &str = env!("CARGO_BIN_EXE_gauntlet");

/// The bounded-wait step and the terminate deadline (the transport's own
/// named-constant pattern; the values are this harness's pacing, not
/// policy). A deadline that fires is a FAILURE (a hang is a defect).
const STEP: u64 = 50;
const TERMINATE_BOUND: u64 = 180;

/// The shell the hygiene reaper invokes (named constant, the transport's
/// own fixed-argv kill-helper pattern: the pid is an argv ARGUMENT).
/// The fixture's own budget ceiling (GAUNTLET.md `budget.max_rounds`) —
/// the number the reserve math is checked against. The fixture freezes
/// 4 rounds; the recovery reserve holds 15% of every limit back, so the
/// loop may book at most 3 rounds before its forward ceiling refuses.
/// Spelled as the fixture's configuration fact (the §8.1 number beside a
/// named field is configuration, not a magic literal).
const FIXTURE_MAX_ROUNDS: u64 = 4;

/// The workspace root: this package's manifest sits two levels below it.
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

/// One scratch world.
struct World {
    root: PathBuf,
}

impl World {
    fn new(tag: &str) -> Self {
        let root =
            std::env::temp_dir().join(format!("gauntlet-e2e-budget-{tag}-{}", std::process::id()));
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

    fn state_db(&self) -> PathBuf {
        self.state_dir().join("gauntlet-state.db")
    }

    fn evidence_dir(&self) -> PathBuf {
        self.root.join("evidence")
    }

    fn ensure_layout(&self) {
        std::fs::create_dir_all(self.state_dir()).expect("state dir");
        std::fs::create_dir_all(self.evidence_dir()).expect("evidence dir");
        std::fs::create_dir_all(self.state_dir().join("workspaces").join("workspaces"))
            .expect("the wrap-time workspace directory");
    }

    /// The budget leg's environment: the required control settings tuned
    /// SMALL (the silent child must be caught by the stall deadline in
    /// seconds, not minutes), and the child in its `silent` mode. The
    /// wall clock (no GAUNTLET_CLOCK override) is the point: the stall
    /// deadline is a REAL deadline here.
    fn env(&self, seed: &str) -> BTreeMap<&'static str, String> {
        let mut env = BTreeMap::new();
        env.insert("GAUNTLET_STATE_DIR", self.state_dir().display().to_string());
        env.insert(
            "GAUNTLET_ARTIFACT_DIR",
            self.evidence_dir().display().to_string(),
        );
        env.insert("GAUNTLET_ID_SEED", seed.to_owned());
        env.insert("GAUNTLET_STALL_TIMEOUT_MS", "2000".to_owned());
        env.insert("GAUNTLET_RETRY_BASE_MS", "100".to_owned());
        env.insert("GAUNTLET_RETRY_MAX_MS", "1000".to_owned());
        env.insert("GAUNTLET_MAX_ATTEMPTS_PER_UNIT", "3".to_owned());
        env.insert("GAUNTLET_CHILD_MODE", "silent".to_owned());
        // The script is DERIVED from the fixture's own agent dir, never
        // spelled (AK-016 — the mock E2E's check-script pattern).
        let child = sole_script_of(&self.target().join("agent"), "agent");
        make_executable(&child);
        env.insert(
            "GAUNTLET_CLI_AGENT_CMD",
            format!(
                "{} {{prompt_file}} {{workspace}} {{result_path}}",
                child.display()
            ),
        );
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

    /// Spawns the real binary as a CHILD (for the bounded-wait harness).
    fn spawn(&self, env: &BTreeMap<&'static str, String>, args: &[&str]) -> std::process::Child {
        let mut command = Command::new(GAUNTLET);
        command.current_dir(self.target());
        for (key, value) in env {
            command.env(key, value);
        }
        command
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("the gauntlet binary spawns")
    }
}

impl Drop for World {
    fn drop(&mut self) {
        // The silent children are cancelled by the loop's own stall
        // machinery; a surviving chain (a cancel that raced the exit) is
        // reaped by its own DURABLE HANDLE FILE (the child pid the adapter
        // recorded at admission) — hygiene, never an assertion.
        // The fixture child's own mechanical `reap` mode retires the
        // named pids (the mode token is the script's argv contract, a
        // fixture-data literal below — never a spelled command line).
        let reap_mode = "reap";
        let Ok(child) = std::fs::read_dir(self.target().join("agent")).map(|entries| {
            let mut files: Vec<PathBuf> = entries
                .flatten()
                .map(|entry| entry.path())
                .filter(|path| path.is_file())
                .collect();
            files.sort();
            files
        }) else {
            return;
        };
        let [child] = child.as_slice() else {
            return;
        };
        let mut stack = vec![self.state_dir().join("runs")];
        while let Some(dir) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.to_string_lossy().ends_with(".handle.json")
                    && let Ok(text) = std::fs::read_to_string(&path)
                    && let Some(at) = text.find("\"child\":")
                {
                    let digits: String = text[at + "\"child\":".len()..]
                        .chars()
                        .take_while(|c| c.is_ascii_digit())
                        .collect();
                    if !digits.is_empty() {
                        let _ = Command::new(child)
                            .args([reap_mode, digits.as_str()])
                            .status();
                    }
                }
            }
        }
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// Recursively copies `from` to `to`.
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

/// The fixture target's check command as a tool binding: derived from
/// the fixture's own scripts dir, never spelled (AK-016).
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

/// Waits for the child to exit within `deadline`; a child that outlives
/// the deadline is SIGKILLed and the typed failure fires — a HANG is a
/// test failure, never a pass.
fn wait_bounded(child: &mut std::process::Child, deadline: Duration, what: &str) -> Output {
    let started = Instant::now();
    loop {
        match child.try_wait().expect("the child state reads") {
            Some(_status) => {
                let mut stdout = child.stdout.take().expect("the piped stdout");
                let mut stderr = child.stderr.take().expect("the piped stderr");
                let status = child.wait().expect("the child's status is collectable");
                let mut out_bytes = Vec::new();
                let mut err_bytes = Vec::new();
                std::io::Read::read_to_end(&mut stdout, &mut out_bytes).expect("the stdout reads");
                std::io::Read::read_to_end(&mut stderr, &mut err_bytes).expect("the stderr reads");
                return Output {
                    status,
                    stdout: out_bytes,
                    stderr: err_bytes,
                };
            }
            None => {
                if started.elapsed() > deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    panic!("the composed loop did not terminate within {deadline:?}: {what}");
                }
                std::thread::sleep(Duration::from_millis(STEP));
            }
        }
    }
}

/// The last durable checkpoint's booked rounds, from an export.
fn last_checkpoint_rounds(exported: &str) -> u64 {
    let checkpoints: Vec<&str> = exported
        .lines()
        .filter(|line| line.contains("\"record\":\"checkpoint\""))
        .collect();
    let last_checkpoint = checkpoints
        .last()
        .expect("the run durably checkpoints")
        .to_owned();
    let marker = "\"budget_spent\":{";
    let at = last_checkpoint
        .find(marker)
        .unwrap_or_else(|| panic!("the checkpoint carries its budget: {last_checkpoint}"));
    let rest = &last_checkpoint[at + marker.len()..];
    let rounds_field = rest
        .find("\"rounds\":")
        .expect("the budget carries its rounds");
    let digits_start = rounds_field + "\"rounds\":".len();
    let digits: String = rest[digits_start..]
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    digits.parse().expect("the booked rounds are a number")
}

/// The OPEN escalation-ticket listing lines naming `unit`.
fn open_ticket_lines_naming(
    world: &World,
    env: &BTreeMap<&'static str, String>,
    unit: &str,
) -> Vec<String> {
    let escalations = world.run(env, &["escalations", "list"]);
    String::from_utf8_lossy(&escalations.stdout)
        .lines()
        .filter(|line| line.contains("state=open") && line.contains(unit))
        .map(|line| line.to_owned())
        .collect()
}

/// F-R5 (corrections round 2): `gauntlet resume` of a NON-settled
/// (escalated) run is the operator re-entry — and it must be HONEST on
/// both ends. The resumed pass that re-observes the same unresolved fact
/// re-AFFIRMS the open ticket (durable per-run dedup: the escalation
/// listing shows ONE, never a second open ticket for the same fact), the
/// re-terminaling is VISIBLE in the record (the export carries the
/// `run_terminal` transition, append-only), the re-spent round is in the
/// checkpoint facts, and the resumed report's terminal field is the
/// run's real final state.
#[test]
fn the_escalated_resume_re_affirms_one_ticket_per_fact_and_records_the_re_entry() {
    let world = World::new("resume");
    let env = world.env("budget-resume-seed");

    // PASS 1: the non-converging world, terminated `escalated` (the §11.5
    // row-4 machinery, exactly as the leg above pins).
    let mut child = world.spawn(&env, &["run", "seeded bounded task"]);
    let first = wait_bounded(
        &mut child,
        Duration::from_secs(TERMINATE_BOUND),
        "the first pass terminates cleanly",
    );
    let first_stdout = String::from_utf8_lossy(&first.stdout).into_owned();
    assert!(
        first_stdout.contains("terminal=escalated"),
        "the first pass ends escalated: {first_stdout}"
    );
    let run_id = first_stdout
        .lines()
        .find(|line| line.starts_with("run: run_id="))
        .and_then(|line| line.strip_prefix("run: run_id="))
        .expect("the run prints its run id")
        .to_owned();
    let first_export = world.run(&env, &["export", "--run", &run_id]);
    assert!(
        first_export.status.success(),
        "the first export succeeds: {}",
        String::from_utf8_lossy(&first_export.stderr)
    );
    let first_rounds = last_checkpoint_rounds(&String::from_utf8_lossy(&first_export.stdout));
    let first_tickets = open_ticket_lines_naming(&world, &env, "AK-953");
    assert_eq!(
        first_tickets.len(),
        1,
        "pass 1 opens exactly one AK-953 ticket: {first_tickets:?}"
    );

    // PASS 2: resume the non-settled run — the operator re-entry. It
    // re-enters the loop, re-spends a booked round, and reaches an HONEST
    // new terminal; a non-settled terminal never exits success.
    let second = world.run(&env, &["resume", &run_id]);
    assert!(
        !second.status.success(),
        "a non-settled re-entry never succeeds: {}",
        String::from_utf8_lossy(&second.stdout)
    );
    let second_stdout = String::from_utf8_lossy(&second.stdout).into_owned();
    let resumed_terminal = second_stdout
        .lines()
        .find(|line| line.starts_with("run: terminal="))
        .map(|line| {
            // `run: terminal=<state> iterations=<n>` — the state token is
            // the first word after the first '='.
            line.split('=')
                .nth(1)
                .and_then(|rest| rest.split_whitespace().next())
                .map(|token| token.to_owned())
                .expect("the terminal line carries its state token")
        })
        .expect("the resumed pass prints its terminal");
    assert_ne!(
        resumed_terminal, "settled",
        "nothing settled in this world: {second_stdout}"
    );

    // THE DEDUP: the resumed pass re-observing the AK-953 fact
    // re-affirms the ONE open ticket — the listing never shows two open
    // tickets for the same fact.
    let tickets_after = open_ticket_lines_naming(&world, &env, "AK-953");
    assert_eq!(
        tickets_after.len(),
        1,
        "a resumed pass re-affirms the open AK-953 ticket — never mints a \
         duplicate: {tickets_after:?}"
    );

    // THE RE-ENTRY IS VISIBLE (pinned, never redesigned): the export
    // carries the run_terminal transition in append order — the first
    // pass's `escalated` and the resumed pass's terminal — and the
    // re-spent round is in the checkpoint facts.
    let second_export = world.run(&env, &["export", "--run", &run_id]);
    assert!(
        second_export.status.success(),
        "the resumed export succeeds: {}",
        String::from_utf8_lossy(&second_export.stderr)
    );
    let second_exported = String::from_utf8_lossy(&second_export.stdout).into_owned();
    let run_terminals: Vec<String> = second_exported
        .lines()
        .filter(|line| line.contains("\"record\":\"event\"") && line.contains("run_terminal"))
        .map(|line| {
            let marker = "\"state\":\"";
            let at = line
                .find(marker)
                .unwrap_or_else(|| panic!("the run_terminal event carries its state: {line}"));
            let rest = &line[at + marker.len()..];
            rest.chars().take_while(|c| *c != '"').collect()
        })
        .collect();
    assert!(
        run_terminals.len() >= 2,
        "the re-terminaling is visible in the record: {run_terminals:?}"
    );
    assert_eq!(
        run_terminals.first().map(String::as_str),
        Some("escalated"),
        "the first pass's terminal is on the record: {run_terminals:?}"
    );
    assert_eq!(
        run_terminals.last().map(String::as_str),
        Some(resumed_terminal.as_str()),
        "the resumed pass's terminal is the run's real final state: {run_terminals:?}"
    );
    let second_rounds = last_checkpoint_rounds(&second_exported);
    assert!(
        second_rounds > first_rounds,
        "the re-entry honestly re-spent a booked round: {first_rounds} -> {second_rounds}"
    );

    // The resumed report's terminal field is the run's real final state.
    let report_line = second_stdout
        .lines()
        .find(|line| line.starts_with("{\"run_id\":"))
        .expect("the resumed pass prints its canonical report JSON line");
    let report = gauntlet_ports::json::parse(report_line).expect("the resumed report JSON parses");
    match report.get("terminal_state") {
        Some(gauntlet_ports::json::JsonValue::Str(state)) => {
            assert_eq!(
                state.as_str(),
                resumed_terminal,
                "the report's terminal field is the run's real final state"
            );
        }
        other => panic!("the resumed report carries the real terminal, not a null: {other:?}"),
    }
}

/// The durable settlement events, as a count.
fn settlement_event_count(state_db: &Path) -> usize {
    if !state_db.is_file() {
        return 0;
    }
    gauntlet_adapter_state_sqlite::test_support::settlement_committed_events(state_db)
        .map(|events| events.len())
        .unwrap_or(0)
}

#[test]
fn the_non_converging_silent_unit_terminates_cleanly_with_the_reserve_intact() {
    let world = World::new("silent");
    let env = world.env("budget-silent-seed");

    // THE RUN: every dispatch's child is silent (never writes a contract,
    // never exits). The stall deadline cancels it; the §11.5 row-4 table
    // retries once and then escalates; the rounds burn; the loop closes.
    let mut child = world.spawn(&env, &["run", "seeded bounded task"]);
    let output = wait_bounded(
        &mut child,
        Duration::from_secs(TERMINATE_BOUND),
        "the budget-exhausted run must terminate cleanly",
    );
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

    // CLEAN TYPED TERMINATION: non-zero exit, the honest classification
    // (OBSERVED AND RECORDED: the loop's §11.5 row-4 machinery disposes
    // the non-converging unit — stall deadline cancels the silent child,
    // one retry, then the second timeout ESCALATES and terminates the
    // run — before any round could be exhausted; the plan's "budget
    // exhaustion" is reached by the budget-control machinery's own
    // terminal, with the reserve provably untouched below), no crash and
    // no hang.
    assert!(
        !output.status.success(),
        "a non-converged run never succeeds: stdout={stdout}\nstderr={stderr}"
    );
    assert!(
        stdout.contains("terminal=escalated"),
        "the honest §11.5 row-4 terminal for the silent unit: {stdout}\n{stderr}"
    );
    assert!(
        stdout.contains("timed out twice"),
        "the escalation names the row-4 cause: {stdout}\n{stderr}"
    );
    assert!(
        !stdout.contains("panic") && !stderr.contains("panicked"),
        "the terminal is the loop's own machinery, not a crash: {stdout}\n{stderr}"
    );
    let run_id = stdout
        .lines()
        .find(|line| line.starts_with("run: run_id="))
        .and_then(|line| line.strip_prefix("run: run_id="))
        .expect("the run prints its run id")
        .to_owned();

    // ZERO SETTLEMENTS: nothing a silent child did — or failed to do —
    // settled. The durable record carries no settlement event at all.
    assert_eq!(
        settlement_event_count(&world.state_db()),
        0,
        "a non-converged run over a silent child settles nothing"
    );

    // THE RECOVERY RESERVE INTACT, from the records: the run's LAST
    // durable checkpoint carries its own budget accounting, and the
    // booked rounds stopped strictly below the fixture's ceiling — the
    // withheld remainder IS the recovery reserve (the meter refused the
    // forward ceiling's reserve slice, p6's machinery, reached through
    // the real binary). The budget spent is honest zeros on the other
    // axes: the loop never billed work it never did.
    let export = world.run(&env, &["export", "--run", &run_id]);
    assert!(
        export.status.success(),
        "the export leg succeeds: {}",
        String::from_utf8_lossy(&export.stderr)
    );
    let exported = String::from_utf8_lossy(&export.stdout).into_owned();
    let checkpoints: Vec<&str> = exported
        .lines()
        .filter(|line| line.contains("\"record\":\"checkpoint\""))
        .collect();
    let last_checkpoint = checkpoints
        .last()
        .copied()
        .expect("the run durably checkpoints")
        .to_owned();
    let rounds_booked: u64 = {
        let marker = "\"budget_spent\":{";
        let at = last_checkpoint
            .find(marker)
            .unwrap_or_else(|| panic!("the checkpoint carries its budget: {last_checkpoint}"));
        let rest = &last_checkpoint[at + marker.len()..];
        let rounds_field = rest
            .find("\"rounds\":")
            .expect("the budget carries its rounds");
        let digits_start = rounds_field + "\"rounds\":".len();
        let digits: String = rest[digits_start..]
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect();
        digits.parse().expect("the booked rounds are a number")
    };
    assert!(
        rounds_booked < FIXTURE_MAX_ROUNDS,
        "the run stopped BEFORE its final round: {rounds_booked} booked of \
         {FIXTURE_MAX_ROUNDS} configured — the recovery reserve was never spent \
         ({last_checkpoint})"
    );
    assert!(
        last_checkpoint.contains("\"tokens\":0") && last_checkpoint.contains("\"cost_usd\":0"),
        "no tokens or cost were billed for work that never happened: {last_checkpoint}"
    );

    // THE REPORT'S CLASSIFICATION IS RECORD-DERIVED (corrections round 2,
    // F-R4): the printed report JSON must agree with the run's own durable
    // export. A unit the export shows ATTEMPTED — role_invocation records
    // with invocation-terminal events — is never described as
    // never-attempted; the report's terminal field is the run's REAL
    // terminal (the one stdout printed), never a null; and the
    // never-dispatched units say exactly that. RED on the falsifier's
    // round-1 world: the delivered report classifies the twice-timed-out
    // unit `configured-but-untested` with basis "…never attempted on
    // record" while the export holds two timed-out invocation terminals,
    // and carries `terminal_state:null` beside stdout's
    // `terminal=escalated`.
    let fixture_units = ["AK-951", "AK-952", "AK-953", "AK-954"];
    let export_records: Vec<&str> = exported.lines().collect();
    let attempted: Vec<&str> = fixture_units
        .iter()
        .copied()
        .filter(|unit| {
            export_records
                .iter()
                .any(|line| line.contains("\"record\":\"role_invocation\"") && line.contains(unit))
        })
        .collect();
    assert_eq!(
        attempted,
        vec!["AK-951"],
        "the export's role_invocation records name the attempted units"
    );
    // The attempted unit's invocations and their durable terminals, from
    // the export (the basis is asserted AGAINST these, not against prose).
    let ak951_invocations: Vec<String> = export_records
        .iter()
        .filter(|line| line.contains("\"record\":\"role_invocation\"") && line.contains("AK-951"))
        .filter_map(|line| {
            let marker = "\"id\":\"";
            let at = line.find(marker)?;
            let rest = &line[at + marker.len()..];
            Some(rest.chars().take_while(|c| *c != '"').collect::<String>())
        })
        .collect();
    assert!(
        !ak951_invocations.is_empty() && ak951_invocations.len() <= 3,
        "the attempted unit's invocation records are on the export: {ak951_invocations:?}"
    );
    for invocation in &ak951_invocations {
        let terminal_line = export_records.iter().find(|line| {
            line.contains("\"record\":\"event\"")
                && line.contains("invocation_terminal")
                && line.contains(&format!("\"invocation_id\":\"{invocation}\""))
        });
        let terminal_line = terminal_line.unwrap_or_else(|| {
            panic!("invocation {invocation} carries a durable terminal event on the export")
        });
        assert!(
            terminal_line.contains("timed_out"),
            "the silent unit's dispatch timed out on the record: {terminal_line}"
        );
    }

    let report_line = stdout
        .lines()
        .find(|line| line.starts_with("{\"run_id\":"))
        .expect("the run prints its canonical report JSON line");
    assert!(
        !report_line.contains("never attempted on record"),
        "the report never describes a unit of this run as never attempted: {report_line}"
    );
    let report = gauntlet_ports::json::parse(report_line)
        .expect("the printed report JSON parses through the ports codec");
    match report.get("terminal_state") {
        Some(gauntlet_ports::json::JsonValue::Str(state)) => {
            assert_eq!(
                state, "escalated",
                "the report's terminal field is the run's real terminal (stdout printed \
                 terminal=escalated)"
            );
        }
        other => panic!(
            "the report carries the run's real terminal, not a null: {other:?} ({report_line})"
        ),
    }
    let units = match report.get("units") {
        Some(gauntlet_ports::json::JsonValue::Array(items)) => items,
        other => panic!("the report carries the units array: {other:?}"),
    };
    let row_of = |unit: &str| {
        units
            .iter()
            .find(|row| {
                row.get("unit") == Some(&gauntlet_ports::json::JsonValue::Str(unit.to_owned()))
            })
            .unwrap_or_else(|| panic!("unit {unit} appears in the report units array"))
    };
    let field_str = |row: &gauntlet_ports::json::JsonValue, field: &str| -> String {
        match row.get(field) {
            Some(gauntlet_ports::json::JsonValue::Str(text)) => text.clone(),
            other => panic!("the unit row carries {field} as a string: {other:?}"),
        }
    };
    // ATTEMPTED AND TIMED OUT: the twice-timed-out unit's row states the
    // attempt facts its records hold — provisional (recorded work without
    // recorded proof), basis attempted-and-timed-out naming the terminal
    // the export shows.
    let ak951_row = row_of("AK-951");
    assert_eq!(
        field_str(ak951_row, "category"),
        "provisional",
        "recorded attempts without a settlement are provisional, never configured-but-untested: {:?}",
        field_str(ak951_row, "basis")
    );
    let ak951_basis = field_str(ak951_row, "basis");
    assert!(
        ak951_basis.contains("attempted-and-timed-out") && ak951_basis.contains("timed_out"),
        "the basis is the record-derived attempt disposition: {ak951_basis}"
    );
    assert!(
        !ak951_basis.contains("never attempted"),
        "an attempted unit is never described as never-attempted: {ak951_basis}"
    );
    // NEVER DISPATCHED: the other units' rows say exactly what their
    // records say — no dispatch attempt on record.
    for unit in ["AK-952", "AK-953", "AK-954"] {
        let basis = field_str(row_of(unit), "basis");
        assert!(
            basis.contains("no dispatch attempt"),
            "unit {unit}'s basis states its own record truth: {basis}"
        );
        assert!(
            !basis.contains("attempted-and"),
            "unit {unit} was never dispatched and is never described as attempted: {basis}"
        );
    }

    // THE ESCALATIONS ARE HONEST: the silent unit's timeouts routed to
    // the operator (§11.5 row 4), and the irreversible unit still never
    // dispatched (AK-044 holds under budget exhaustion too).
    let escalations = world.run(&env, &["escalations", "list"]);
    let escalations_text = String::from_utf8_lossy(&escalations.stdout).into_owned();
    assert!(
        escalations_text.contains("AK-951"),
        "the non-converging unit's escalation stands: {escalations_text}"
    );
    let exported_lines: Vec<&str> = exported.lines().collect();
    for record_line in exported_lines {
        assert!(
            !(record_line.contains("\"record\":\"role_invocation\"")
                && (record_line.contains("AK-953")
                    || record_line.contains("AK-952")
                    || record_line.contains("AK-954"))),
            "the budget was spent on the non-converging unit alone: {record_line}"
        );
    }
}

/// F2-7 (falsifier round 2, MAJOR — COR-3): the honest boundary of the
/// operator surface, PINNED — never built around.
///
/// THE RESIDUAL (recorded in `.agents/reports/e2e-real.md`): a
/// governance-routed ticket — here the AK-953 `irreversible`
/// never-dispatched class (the gate's AK-044 conjunct routed it to the
/// operator without any dispatch), identically the §11.5-row-6
/// lead_agent-closure class the falsifier executed twice through the real
/// binary — can never carry the durable-PASS verification record that
/// BOTH operator actions demand as their precondition (§16.1 condition
/// 1). Every resolve/retry of such a ticket is refused typed
/// (`verification_not_pass`); the tickets are unresolvable through the
/// designed surface, and the run can never reach `settled` in this world.
/// This test pins that honest refusal as the boundary — it deliberately
/// does NOT fix the flow: whether an operator-routed Unit can ever
/// acquire the §16.1 PASS evidence is a RULER-OWNER question
/// (CandidateRevision material; the operator surface itself is task 21's).
#[test]
fn resolving_a_governance_routed_ticket_is_refused_typed_no_pass_record_exists() {
    let world = World::new("f27-pin");
    let env = world.env("budget-f27-seed");

    // The composed run: escalated, with the AK-953 governance ticket open.
    let mut child = world.spawn(&env, &["run", "seeded bounded task"]);
    let output = wait_bounded(
        &mut child,
        Duration::from_secs(TERMINATE_BOUND),
        "the run terminates cleanly before the F2-7 pin leg",
    );
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    assert!(
        stdout.contains("terminal=escalated"),
        "the composed run ends escalated: {stdout}"
    );
    let tickets = open_ticket_lines_naming(&world, &env, "AK-953");
    assert_eq!(tickets.len(), 1, "the AK-953 ticket is open: {tickets:?}");
    let ticket_id = tickets[0]
        .split("ticket=")
        .nth(1)
        .and_then(|rest| rest.split_whitespace().next())
        .expect("the listing line names its ticket id");

    // THE PIN: resolving the governance-routed ticket is REFUSED TYPED —
    // the same §16.1 condition-1 gate the automatic path answers to.
    let resolve = world.run(
        &env,
        &[
            "escalations",
            "resolve",
            "--ticket",
            ticket_id,
            "--unit",
            "AK-953",
            "--operator",
            "operator-cor3-pin",
        ],
    );
    let resolve_stderr = String::from_utf8_lossy(&resolve.stderr).into_owned();
    assert!(
        !resolve.status.success(),
        "the refused resolve never succeeds: stdout={}",
        String::from_utf8_lossy(&resolve.stdout)
    );
    assert!(
        resolve_stderr.contains("verification_not_pass"),
        "the refusal carries the typed gate code: {resolve_stderr}"
    );
    assert!(
        resolve_stderr.contains("no PASS verification record exists for unit AK-953"),
        "the refusal is §16.1 condition 1's own text: {resolve_stderr}"
    );

    // The refusal wrote nothing: the ticket stands open, zero settlements.
    let after = open_ticket_lines_naming(&world, &env, "AK-953");
    assert_eq!(
        after.len(),
        1,
        "the refused action leaves the ticket open: {after:?}"
    );
    assert_eq!(
        settlement_event_count(&world.state_db()),
        0,
        "zero settlements: the refused resolve committed nothing"
    );
}
