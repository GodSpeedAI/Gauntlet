//! E2E: recovery through the REAL `gauntlet` binary (plan Task 25 steps
//! 3+; AK-040/AK-042 real-world; R16-F2's discharge; the task-12
//! produce-window crash-injection trigger).
//!
//! The store-level prior art (p7_recovery, SETTLED) covers every §16.3
//! crash window with the crash_helper child and fresh-controller
//! recomposition; the budget prior art (p6) covers reserve/non-convergence
//! at the controller level. THIS target is the delta task 25 exists for:
//! the same classes driven through the real binary as a child process —
//! observed-durable-boundary SIGKILL, then `gauntlet resume`, with every
//! assertion record-derived from the durable stores.
//!
//! HONEST POSTURE STATEMENT (the charter's UNK-003 resolution): the real
//! generic CLI adapter is driven by the fixture's deterministic child;
//! this is NOT a frontier-model claim. The kill boundaries are OBSERVED
//! DURABLE FACTS (the state adapter's doc-hidden test-support readers,
//! polled from this test process), never sleep-and-hope: every wait
//! carries a bounded deadline and a typed failure.
//!
//! Legs:
//! 1. mid-settlement SIGKILL: poll until the first settlement event is
//!    durable, SIGKILL the binary, resume — exactly-once settlement,
//!    preserved run identity, prior evidence intact;
//! 2. the produce window (task-12 trigger): poll until the locator
//!    artifact's bytes are in the evidence store, SIGKILL, resume — the
//!    artifact's production is recovered exactly once;
//! 3. R16-F2 discharge: the hostile child poisons its own contract path
//!    (a DIRECTORY at the result path) and stays alive — the composed
//!    loop reaches a BOUNDED honest terminal through its own machinery,
//!    and a subsequent resume terminates too (liveness preserved).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

/// The real composed binary (same package: `src/bin/gauntlet.rs`).
const GAUNTLET: &str = env!("CARGO_BIN_EXE_gauntlet");

/// The observed-fact poll tick and the bounded-wait step (the transport's
/// own named-constant pattern; the values are this harness's pacing, not
/// policy).
const TICK: u64 = 20;
const STEP: u64 = 50;

/// The bounded deadline for reaching an observed durable boundary, and
/// the bounded deadline for a composed run to terminate on its own. A
/// deadline that fires is a FAILURE (a hang is a defect), never patience.
const BOUNDARY_BOUND: u64 = 120;
const TERMINATE_BOUND: u64 = 180;
/// The bounded deadline for the retirement liveness poll (corrections
/// round 1, F6): how long the run's own cancels get to reap every
/// hostile child before this test calls the leak a defect again.
const LIVENESS_BOUND: u64 = 30;
/// The bounded deadline for the database connections of a killed child to
/// drain (the quiescence barrier after every SIGKILL).
const QUIESCE_BOUND: u64 = 10;

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

/// One scratch world: a fresh copy of the fixture project plus the
/// per-run state/evidence directories under it. Removed on drop.
struct World {
    root: PathBuf,
}

impl World {
    fn new(tag: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "gauntlet-e2e-recovery-{tag}-{}",
            std::process::id()
        ));
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

    /// Creates the state/evidence dirs up front — plus the workspace
    /// subdirectory the composition-time wrap check scopes
    /// `$WORKSPACE_ROOT` into (the live backend's dry-run bind source).
    fn ensure_layout(&self) {
        std::fs::create_dir_all(self.state_dir()).expect("state dir");
        std::fs::create_dir_all(self.evidence_dir()).expect("evidence dir");
        std::fs::create_dir_all(self.state_dir().join("workspaces").join("workspaces"))
            .expect("the wrap-time workspace directory");
    }

    /// The command environment for one leg.
    fn env(&self, seed: &str) -> BTreeMap<&'static str, String> {
        let mut env = BTreeMap::new();
        env.insert("GAUNTLET_STATE_DIR", self.state_dir().display().to_string());
        env.insert(
            "GAUNTLET_ARTIFACT_DIR",
            self.evidence_dir().display().to_string(),
        );
        env.insert("GAUNTLET_ID_SEED", seed.to_owned());
        env.insert("GAUNTLET_STALL_TIMEOUT_MS", "60000".to_owned());
        env.insert("GAUNTLET_RETRY_BASE_MS", "100".to_owned());
        env.insert("GAUNTLET_RETRY_MAX_MS", "2000".to_owned());
        env.insert("GAUNTLET_MAX_ATTEMPTS_PER_UNIT", "3".to_owned());
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

    /// Spawns the real binary as a CHILD (for observed-boundary kills).
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

/// The bounded poll: every wait in this file runs through this loop —
/// a durable-fact predicate, a hard deadline, a typed failure. No
/// sleep-and-hope anywhere.
fn poll_until<F>(what: &str, deadline: Duration, mut fact: F) -> String
where
    F: FnMut() -> Option<String>,
{
    let started = Instant::now();
    loop {
        if let Some(found) = fact() {
            return found;
        }
        if started.elapsed() > deadline {
            panic!("the observed durable boundary never appeared within {deadline:?}: {what}");
        }
        std::thread::sleep(Duration::from_millis(TICK));
    }
}

/// The durable `settlement_committed` events, as a count (the state
/// adapter's doc-hidden test-support reader — the same instrument the
/// settled p7/mock proofs read through).
fn settlement_event_count(state_db: &Path) -> usize {
    if !gauntlet_adapter_state_postgres::state_exists(state_db)
        .expect("the state database is reachable")
    {
        return 0;
    }
    gauntlet_adapter_state_postgres::test_support::settlement_committed_events(state_db)
        .map(|events| events.len())
        .unwrap_or(0)
}

/// The durable settlement ROWS for one unit, as a count.
fn settlement_row_count(state_db: &Path, unit: &str) -> usize {
    if !gauntlet_adapter_state_postgres::state_exists(state_db)
        .expect("the state database is reachable")
    {
        return 0;
    }
    gauntlet_adapter_state_postgres::test_support::settlement_rows(state_db)
        .map(|rows| {
            rows.iter()
                .filter(|(row_unit, ..)| row_unit.as_str() == unit)
                .count()
        })
        .unwrap_or(0)
}

/// Whether the durable evidence production rows are fewer than `n`.
fn production_rows_below(state_db: &Path, n: usize) -> bool {
    if !gauntlet_adapter_state_postgres::state_exists(state_db)
        .expect("the state database is reachable")
    {
        return true;
    }
    gauntlet_adapter_state_postgres::test_support::evidence_production_rows(state_db)
        .map(|rows| rows.len() < n)
        .unwrap_or(true)
}

/// How many durable evidence production rows name a locator containing
/// `needle` (leg 2b's window fact: the unit's own production record).
fn production_rows_for_locator(state_db: &Path, needle: &str) -> usize {
    if !gauntlet_adapter_state_postgres::state_exists(state_db)
        .expect("the state database is reachable")
    {
        return 0;
    }
    gauntlet_adapter_state_postgres::test_support::evidence_production_rows(state_db)
        .map(|rows| {
            rows.iter()
                .filter(|(_, locator, ..)| locator.contains(needle))
                .count()
        })
        .unwrap_or(0)
}

/// The durable production rows at the marker locator, as
/// `(ref, locator, produced_at, offset)` tuples ordered by offset —
/// the adjudicated assertions read the crashed attempt's row and the
/// resumed pass's own row from this surface.
fn marker_rows(state_db: &Path) -> Vec<(String, String, i64, i64)> {
    if !gauntlet_adapter_state_postgres::state_exists(state_db)
        .expect("the state database is reachable")
    {
        return Vec::new();
    }
    gauntlet_adapter_state_postgres::test_support::evidence_production_rows(state_db)
        .map(|rows| {
            rows.into_iter()
                .filter(|(_, locator, ..)| locator.contains("real-marker-report"))
                .collect()
        })
        .unwrap_or_default()
}

/// AK-951's durable dispatch (role invocation) count in the run's export —
/// the adjudicated no-dispatch-storm fact (one record per RoleInvocation;
/// the record names its unit, the way leg 1's AK-953 absence check reads).
fn ak951_dispatch_count(
    world: &World,
    env: &BTreeMap<&'static str, String>,
    run_id: &str,
) -> usize {
    let export = world.run(env, &["export", "--run", run_id]);
    assert!(
        export.status.success(),
        "the run exports: {}",
        String::from_utf8_lossy(&export.stderr)
    );
    String::from_utf8_lossy(&export.stdout)
        .lines()
        .filter(|line| line.contains("\"record\":\"role_invocation\"") && line.contains("AK-951"))
        .count()
}

/// AK-951's durable settlement-event count (the §16.3 step-1 append —
/// the branch discriminator and the exactly-once event fact).
fn ak951_settlement_event_count(state_db: &Path) -> usize {
    if !gauntlet_adapter_state_postgres::state_exists(state_db)
        .expect("the state database is reachable")
    {
        return 0;
    }
    gauntlet_adapter_state_postgres::test_support::settlement_committed_events(state_db)
        .map(|events| {
            events
                .iter()
                .filter(|(_, unit, ..)| unit == "AK-951")
                .count()
        })
        .unwrap_or(0)
}

/// Whether any object in the evidence store contains `needle` (the
/// record-derived reader over the content-addressed objects tree).
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

/// SIGKILLs the child and collects its (partial) output.
///
/// The kill does NOT freeze the database: a COMMIT the child already sent
/// can still complete on the server after the child is dead. So after the
/// reap, the state instance at `state_db` is quiesced (every backend the
/// child opened has exited) before the caller reads any durable state.
fn kill_and_collect(child: &mut std::process::Child, state_db: &Path) -> Output {
    child.kill().expect("the SIGKILL lands");
    let mut stdout = child.stdout.take().expect("the piped stdout");
    let mut stderr = child.stderr.take().expect("the piped stderr");
    let status = child.wait().expect("the killed child reaps");
    gauntlet_adapter_state_postgres::test_support::quiesce_writers(
        state_db,
        Duration::from_secs(QUIESCE_BOUND),
    )
    .expect("the killed child's database connections drain");
    let mut out_bytes = Vec::new();
    let mut err_bytes = Vec::new();
    std::io::Read::read_to_end(&mut stdout, &mut out_bytes).expect("the stdout reads");
    std::io::Read::read_to_end(&mut stderr, &mut err_bytes).expect("the stderr reads");
    Output {
        status,
        stdout: out_bytes,
        stderr: err_bytes,
    }
}

// ---------------------------------------------------------------------------
// Leg 1 — mid-settlement SIGKILL: kill at the observed first-settlement
// boundary, resume, and pin exactly-once settlement + preserved run
// identity + intact prior evidence (AK-040/AK-042 real-world).
// ---------------------------------------------------------------------------

#[test]
fn the_kill_at_the_first_settlement_boundary_resumes_to_exactly_once() {
    let world = World::new("mid-settle");
    let env = world.env("recovery-mid-settle-seed");

    // THE RUN, as a child process. An auto-closable unit (AK-951) settles
    // first; the moment a settlement event is DURABLE, the binary dies by
    // SIGKILL — the kill lands at the observed durable boundary (its
    // exact side of the §16.3 commit is, by construction, either side).
    let mut child = world.spawn(&env, &["run", "seeded bounded task"]);
    poll_until(
        "the first settlement event",
        Duration::from_secs(BOUNDARY_BOUND),
        || {
            let count = settlement_event_count(&world.state_db());
            (count >= 1).then(|| format!("settlement events durable: {count}"))
        },
    );
    let output = kill_and_collect(&mut child, &world.state_db());
    assert!(
        !output.status.success(),
        "a SIGKILLed run did not succeed — the kill was real"
    );

    // The pre-resume durable baseline: the settlement event may or may
    // not have its keyed row (either side of the commit is reachable) —
    // the assertions below hold REGARDLESS of which side the kill landed.
    let run_id = {
        // The killed child's stdout is a pipe that died with it; the run
        // id is durable in the state dir's own layout.
        let runs = world.state_dir().join("runs");
        let mut ids: Vec<String> = std::fs::read_dir(&runs)
            .expect("the run io root reads")
            .filter_map(Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();
        ids.sort();
        assert_eq!(
            ids.len(),
            1,
            "exactly one run exists in the killed world: {ids:?}"
        );
        ids.remove(0)
    };

    // THE OPERATOR'S DOCUMENTED RECOVERY PATH (the single-writer guard's
    // own typed refusal, COR-R3-3): the SIGKILL leaves the lock file; the
    // first resume is REFUSED typed (stale lock, holder pid named, manual
    // recovery named, no takeover); the operator — having confirmed the
    // writer is dead (it reaped above) — removes the lock file manually;
    // the resume then proceeds.
    let refused = world.run(&env, &["resume", &run_id]);
    let refused_stderr = String::from_utf8_lossy(&refused.stderr).into_owned();
    assert!(
        !refused.status.success() && refused_stderr.contains("stale or unreadable writer lock"),
        "the post-crash resume is refused typed by the single-writer guard: {refused_stderr}"
    );
    assert!(
        refused_stderr.contains("remove the lock file manually"),
        "the refusal names the operator's manual recovery: {refused_stderr}"
    );
    std::fs::remove_file(world.state_dir().join("writer.lock"))
        .expect("the operator's manual removal of the stale lock");

    // THE RESUME: the real binary, fresh process, same run.
    let resume = world.run(&env, &["resume", &run_id]);
    let resume_stdout = String::from_utf8_lossy(&resume.stdout).into_owned();
    let resume_stderr = String::from_utf8_lossy(&resume.stderr).into_owned();
    assert!(
        resume_stdout.contains("run_id=") || resume_stdout.contains("terminal="),
        "the resume completes: {resume_stdout}\n{resume_stderr}"
    );

    // EXACTLY ONCE (corrections round 1, F8: the fixture carries a unit
    // per dispatchable tier, so TWO auto-closable units — AK-951 and
    // AK-954 — settle across the crash and the resume): each auto-closable
    // unit's durable settlement is ONE event shared between exactly TWO
    // settlement events and ONE row per unit — the crash never duplicated
    // a settlement, and the recovery never re-minted what was already
    // durable. AK-952 settles nothing (its closure is operator-routed,
    // §16.1) and AK-953 was never dispatched.
    let events_after = settlement_event_count(&world.state_db());
    let ak951_rows_after = settlement_row_count(&world.state_db(), "AK-951");
    let ak954_rows_after = settlement_row_count(&world.state_db(), "AK-954");
    assert_eq!(
        events_after, 2,
        "exactly the two auto-closable units' settlement events exist after the resume \
         (exactly-once, AK-040)"
    );
    assert_eq!(
        ak951_rows_after, 1,
        "exactly one durable settlement row exists for AK-951 (exactly-once)"
    );
    assert_eq!(
        ak954_rows_after, 1,
        "exactly one durable settlement row exists for AK-954 (exactly-once)"
    );

    // PRESERVED RUN IDENTITY: the export stream answers for the SAME run
    // id, and the resumed terminal is the bounded honest one.
    let export = world.run(&env, &["export", "--run", &run_id]);
    assert!(
        export.status.success(),
        "the export of the resumed run succeeds: {}",
        String::from_utf8_lossy(&export.stderr)
    );
    let exported = String::from_utf8_lossy(&export.stdout).into_owned();
    assert!(
        exported.contains("\"record\":\"settlement\"") && exported.contains("AK-951"),
        "the settlement stands on the resumed run's own durable record"
    );
    for record_line in exported.lines() {
        assert!(
            !(record_line.contains("\"record\":\"role_invocation\"")
                && record_line.contains("AK-953")),
            "the irreversible unit was never dispatched, before or after the kill: \
             {record_line}"
        );
    }

    // PRIOR EVIDENCE INTACT: the settled unit's locator artifact — the
    // bytes the child wrote INSIDE its wrapped workspace (tokened by the
    // child-work-record kind only the artifact carries) — is still in the
    // durable evidence store after the crash and the resume.
    assert!(
        store_holds_text_containing(&world.evidence_dir(), "\"kind\":\"child-work-record\""),
        "the pre-kill locator artifact survives the crash and the resume"
    );
}

// ---------------------------------------------------------------------------
// Leg 2 — the produce window (the task-12 ledger's crash-injection
// trigger): the kill lands between the evidence artifact's durable write
// and its durable production record. The observed boundary: the artifact
// bytes are IN the store while the production rows are still short.
//
// ADJUDICATED ASSERTIONS (COR-4 round, orchestrator disposition (a) —
// re-scope, no production change; full case:
// .agents/reports/task25/cor4-report.md, adjudication section): the
// resume's recovery semantics are the spec's own —
//   - the §16.3 crash-before-settlement-event retry is bounded by the
//     §11.5 attempts ceiling and never a storm; an already-settled unit
//     is never re-dispatched. The observed shape (finding F-C4-1, below
//     and in the report): when the crashed attempt's production row
//     exists, the resume's first re-dispatch inherits the crashed
//     workspace, its produce dedupes onto the pre-crash row, and the
//     §14.3 staleness rule classes it inconclusive — one §11.5
//     re-attempt in a fresh workspace settles;
//   - every pre-crash production row survives the resume untouched
//     (write-once per-ref, the crashed iteration's own fact);
//   - the resumed pass mints its OWN per-iteration production (a second
//     row with its own ref IS the spec shape);
//   - the settlement is EXACTLY ONCE for the unit, over the CURRENT
//     (freshest) ref, with run identity and prior artifact bytes
//     preserved.
// The per-locator production count is NOT asserted: exactly-once
// production is NOT the spec's shape (the corrective loop legitimately
// re-produces at a locator). The ruler-owner question this re-scope
// closes is on record verbatim: "should §18.3 recovery adopt pre-crash
// productions (changing the §14.3 staleness economics)? —
// CandidateRevision, reference-candidates at task25 settlement."
// ---------------------------------------------------------------------------

#[test]
fn the_kill_in_the_produce_window_resumes_with_the_production_recovered() {
    let world = World::new("produce-window");
    let env = world.env("recovery-produce-seed");

    // THE RUN, as a child. The produce boundary observed here: the
    // locator artifact's bytes are durable in the evidence store while
    // the durable production rows are still short of the unit's own.
    let mut child = world.spawn(&env, &["run", "seeded bounded task"]);
    poll_until(
        "the artifact-without-production boundary",
        Duration::from_secs(BOUNDARY_BOUND),
        || {
            let artifact_durable = store_holds_text_containing(
                &world.evidence_dir(),
                "\"kind\":\"child-work-record\"",
            );
            if artifact_durable && production_rows_below(&world.state_db(), 1) {
                Some("the artifact is durable and the production row is not".to_owned())
            } else if artifact_durable {
                // The window closed before the kill could land in it — the
                // production row won the race (the MISSED-window branch).
                // The boundary is still the artifact's durability. The
                // branch claims are NOT symmetric and the assertions below
                // are stated against the POST-KILL DURABLE FACTS, so both
                // landings are honest (leg 2b places this branch
                // deterministically; the adjudicated semantics are in this
                // leg's header).
                Some("the artifact and its production row are both durable".to_owned())
            } else {
                None
            }
        },
    );
    let _ = kill_and_collect(&mut child, &world.state_db());

    // THE ADJUDICATED PRE-RESUME FACTS (post-kill durable reads): which
    // side of the produce/settlement pair the kill landed on is a durable
    // fact, and the assertions below are stated against it — never
    // guessed. The pre-crash production rows are captured as tuples so
    // their survival is asserted byte-level (write-once per-ref).
    let rows_before = marker_rows(&world.state_db());
    let ak951_settled_before = ak951_settlement_event_count(&world.state_db());

    let run_id = {
        let runs = world.state_dir().join("runs");
        let mut ids: Vec<String> = std::fs::read_dir(&runs)
            .expect("the run io root reads")
            .filter_map(Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();
        ids.sort();
        assert_eq!(ids.len(), 1, "exactly one run exists: {ids:?}");
        ids.remove(0)
    };

    // The operator's typed stale-lock refusal and manual recovery, exactly
    // as leg 1 models it.
    let refused = world.run(&env, &["resume", &run_id]);
    let refused_stderr = String::from_utf8_lossy(&refused.stderr).into_owned();
    assert!(
        !refused.status.success() && refused_stderr.contains("stale or unreadable writer lock"),
        "the post-crash resume is refused typed by the single-writer guard: {refused_stderr}"
    );
    std::fs::remove_file(world.state_dir().join("writer.lock"))
        .expect("the operator's manual removal of the stale lock");

    // THE RESUME, under the ADJUDICATED recovery semantics (COR-4 report,
    // adjudication section): the §16.3 crash-before-settlement-event
    // retry is EXACTLY ONE fresh dispatch; the pre-crash production rows
    // survive untouched; the resumed pass mints its OWN per-iteration
    // production; the settlement is EXACTLY ONCE over the CURRENT
    // (freshest) ref. A kill that landed AFTER the settlement event finds
    // the unit settled — settled work is never re-dispatched, nothing
    // new mints, and the same exactly-once settlement stands (p7's other
    // pin).
    let resume = world.run(&env, &["resume", &run_id]);
    let resume_stdout = String::from_utf8_lossy(&resume.stdout).into_owned();
    assert!(
        resume_stdout.contains("run_id=") || resume_stdout.contains("terminal="),
        "the resume completes: {resume_stdout}"
    );

    // EXACTLY-ONCE-BOUNDED DISPATCH, never a storm. The durable dispatch
    // count across both passes is asserted against the shipped §11.5
    // shape (finding F-C4-1, cor4-report.md): when the crashed attempt's
    // production row ALREADY exists at the kill, the resume's first
    // re-dispatch deterministically inherits the crashed attempt's
    // workspace (the provider's counter restarts with the process), the
    // child rewrites byte-identical artifact bytes, the produce step
    // dedupes onto the pre-crash row, and the §14.3 staleness rule
    // classes the attempt inconclusive — the loop then §11.5-re-attempts
    // ONCE more in a fresh workspace, which settles. When no row existed
    // (the true in-window landing), the first re-dispatch's produce mints
    // fresh and settles directly. A kill that landed after the settlement
    // event finds settled work, which is never re-dispatched.
    let resumed_dispatches: usize = if ak951_settled_before == 0 {
        if rows_before.is_empty() { 1 } else { 2 }
    } else {
        0
    };
    let dispatches_after = ak951_dispatch_count(&world, &env, &run_id);
    assert_eq!(
        dispatches_after,
        1 + resumed_dispatches,
        "the durable AK-951 dispatch count across both passes (the crashed attempt's own \
         dispatch plus {resumed_dispatches} fresh resumed dispatch(es), bounded by the §11.5 \
         attempts ceiling — finding F-C4-1)"
    );

    // THE PRE-CRASH ROWS SURVIVE UNTOUCHED; the resumed pass mints its own
    // row exactly when it re-dispatched.
    let rows_after = marker_rows(&world.state_db());
    for before in &rows_before {
        assert!(
            rows_after.contains(before),
            "the pre-crash production row {before:?} survived the resume untouched (write-once)"
        );
    }
    if ak951_settled_before == 0 {
        assert_eq!(
            rows_after.len(),
            rows_before.len() + 1,
            "the resumed pass minted exactly its own production beside the pre-crash rows: \
             {rows_after:?}"
        );
    } else {
        assert_eq!(
            rows_after.len(),
            rows_before.len(),
            "a settled unit's locator mints nothing new on resume: {rows_after:?}"
        );
    }

    // EXACTLY-ONCE SETTLEMENT, over the CURRENT (freshest) ref — one keyed
    // row and one event, bound to the production the freshest pass
    // certified; the fixture's auto-closable pair (AK-951 + AK-954) is
    // exactly two settlement events in total.
    let settlements =
        gauntlet_adapter_state_postgres::test_support::settlement_rows(&world.state_db())
            .expect("the settlement row reader runs");
    let ak951_settlements: Vec<_> = settlements
        .iter()
        .filter(|(unit, ..)| unit == "AK-951")
        .collect();
    assert_eq!(
        ak951_settlements.len(),
        1,
        "exactly-once settlement for AK-951: {settlements:?}"
    );
    assert_eq!(
        ak951_settlement_event_count(&world.state_db()),
        1,
        "exactly-once settlement EVENT for AK-951"
    );
    let freshest_ref = rows_after
        .last()
        .map(|(reference, ..)| reference.clone())
        .expect("the marker locator holds at least the settled production");
    assert_eq!(
        ak951_settlements[0].2, freshest_ref,
        "the settlement stands over the CURRENT (freshest) production ref"
    );
    assert_eq!(
        settlement_event_count(&world.state_db()),
        2,
        "exactly the fixture's two auto-closable settlements (AK-951 + AK-954)"
    );

    // RUN IDENTITY PRESERVED: the export answers for the same run, and the
    // settlement stands on the run's own durable record.
    let export = world.run(&env, &["export", "--run", &run_id]);
    assert!(
        export.status.success(),
        "the export of the resumed run succeeds: {}",
        String::from_utf8_lossy(&export.stderr)
    );
    let exported = String::from_utf8_lossy(&export.stdout).into_owned();
    assert!(
        exported.contains("\"record\":\"settlement\"") && exported.contains("AK-951"),
        "the settlement stands on the resumed run's own durable record"
    );

    // PRIOR ARTIFACT BYTES INTACT.
    assert!(
        store_holds_text_containing(&world.evidence_dir(), "\"kind\":\"child-work-record\""),
        "the artifact bytes survive the crash and the resume"
    );
}

// ---------------------------------------------------------------------------
// Leg 2b — the MISSED produce window, placed as closely to DETERMINISTICALLY
// as an external observer can (COR-4). Leg 2's poll races the real produce
// window; on an idle host the kill almost always lands in-window (artifact
// durable, production row not), which is why this leg passed every battery
// until the ORCHESTRATOR's loaded full-workspace run executed the
// missed-window branch for real:
//
//   the_kill_in_the_produce_window_resumes_with_the_production_recovered
//   assertion `left == right` failed: the locator's production is durable
//   EXACTLY once after the resume: left: 2, right: 1
//
// THE PLACEMENT (deterministic, no race): the branch's span in the binary —
// from the production record's durability to the §16.3 settlement-event
// append — is a few milliseconds of record-derived work, so no external
// poll could reliably react inside it (the earlier busy-poll landed after
// the settlement event on loaded GitHub runners, 12/13 at best). The binary
// therefore carries a TEST-ONLY park point, inert unless armed by env var
// (`GAUNTLET_TEST_PARK_AFTER_PRODUCTION_RECORD`): right after the production
// record is durable it writes a marker and parks before verification and
// settlement. This test kills the child while it is provably parked, and
// CLASSIFIES the landing from the durable state itself: the §16.3
// settlement-event count must be absent — a §16.3 crash BEFORE step 1, the
// branch this leg exists for.
//
// COR-4 DIAGNOSIS AND ADJUDICATION (full case:
// .agents/reports/task25/cor4-report.md): the kill is a §16.3 crash
// BEFORE the settlement event, whose frozen disposition is "the unit is
// retried as if verification never happened" — pinned by the SETTLED p7
// proof `a_crash_before_the_settlement_event_retries_as_if_never_verified`
// ("exactly ONE fresh dispatch"). The retried attempt produces ANEW (a
// fresh per-dispatch workspace path is embedded in the fixture child's
// artifact; the write-once production rule is per-ref, never
// per-locator), and the pre-crash production can never be verified by
// the resumed pass: the C5 anchor rule pins the verification anchor to
// the latest checkpoint iteration and §14.3's staleness rule classes any
// older-iteration production `evidence_stale` — the resumed unit's ONLY
// machinery-legal settlement route is the fresh production. The
// orchestrator ADJUDICATED (disposition (a), re-scope, no production
// change): the per-locator exactly-once claim was stronger than the
// spec; this leg now pins the ADJUDICATED semantics — the resume's
// re-dispatch bounded by the §11.5 attempts ceiling (the observed shape,
// finding F-C4-1: TWO fresh resumed dispatches — the first inherits the
// crashed workspace, dedupes onto the pre-crash row, and verifies stale;
// the second settles), the pre-crash production row
// intact byte-level, the resumed pass's OWN per-iteration production
// beside it, exactly-once SETTLEMENT over the CURRENT (freshest) ref,
// run identity and prior artifact bytes preserved.
//
// The ruler-owner question this adjudication closes is on record
// verbatim: "should §18.3 recovery adopt pre-crash productions (changing
// the §14.3 staleness economics)? — CandidateRevision,
// reference-candidates at task25 settlement."
//
// LOAD-SENSITIVITY RECORD (COR-4, brief disposition 4): leg 2's real
// produce window is sub-millisecond against the artifact-write-to-
// production-record pair on this host, so leg 2's branch is
// load-sensitive — it passed every prior battery in the in-window branch
// and failed only under the orchestrator's full-workspace load. This leg
// is the deterministic-class regression pin of the missed-window branch
// itself; the original leg keeps racing the real window.
// ---------------------------------------------------------------------------

#[test]
fn the_kill_after_the_production_record_pins_the_resume_exactly_once_claim() {
    // DETERMINISTIC PLACEMENT: the binary's test-only park point
    // (`GAUNTLET_TEST_PARK_AFTER_PRODUCTION_RECORD`, inert unless set —
    // see `park_after_production_record_if_armed` in gauntlet-app's
    // controller) writes a marker the moment the locator's production row
    // is durable, then parks BEFORE verification and the §16.3 settlement
    // event. The child cannot advance past that point until it is killed,
    // so the kill lands at the missed-window boundary every time — no
    // race against a few-millisecond span, no retry rounds, no sleep-and-hope.
    let world = World::new("missed-window");
    let env = world.env("recovery-missed-seed");
    let marker = world.state_dir().join("parked-after-production-record");
    let mut park_env = env.clone();
    park_env.insert(
        "GAUNTLET_TEST_PARK_AFTER_PRODUCTION_RECORD",
        marker.display().to_string(),
    );

    // THE RUN, as a child, parked at the boundary; bounded wait for the
    // park marker (a durable-side fact the child itself writes).
    let mut child = world.spawn(&park_env, &["run", "seeded bounded task"]);
    let started = Instant::now();
    while !marker.exists() {
        if started.elapsed() > Duration::from_secs(BOUNDARY_BOUND) {
            // The parked child never reaches its own exit: kill it before
            // the typed failure so no orphan outlives this test.
            let _ = kill_and_collect(&mut child, &world.state_db());
            panic!(
                "the child never parked after the locator's production record \
                 within {BOUNDARY_BOUND:?}"
            );
        }
        std::thread::sleep(Duration::from_millis(STEP));
    }
    // The parked child is killed before any further assertion can fail,
    // so it never outlives this test.
    let _ = kill_and_collect(&mut child, &world.state_db());
    assert!(
        production_rows_for_locator(&world.state_db(), "real-marker-report") >= 1,
        "the park follows the locator's durable production record"
    );

    // THE CLASSIFICATION (durable, post-kill): the park sits BEFORE
    // settlement step 1, so the §16.3 settlement event is absent — the
    // missed-window branch, the crash class this leg discharges.
    assert_eq!(
        settlement_event_count(&world.state_db()),
        0,
        "the parked kill landed before the §16.3 settlement event"
    );

    // THE ADJUDICATED PRE-RESUME FACTS: exactly the crashed attempt's
    // own production row is durable, and no settlement stands.
    let rows_before = marker_rows(&world.state_db());
    assert_eq!(
        rows_before.len(),
        1,
        "the crashed attempt's own production row is durable: {rows_before:?}"
    );
    let crashed_row = rows_before[0].clone();
    assert_eq!(
        ak951_settlement_event_count(&world.state_db()),
        0,
        "the crash landed before settlement step 1"
    );

    let run_id = {
        let runs = world.state_dir().join("runs");
        let mut ids: Vec<String> = std::fs::read_dir(&runs)
            .expect("the run io root reads")
            .filter_map(Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();
        ids.sort();
        assert_eq!(ids.len(), 1, "exactly one run exists: {ids:?}");
        ids.remove(0)
    };

    // The operator's typed stale-lock refusal and manual recovery,
    // exactly as legs 1 and 2 model it.
    let refused = world.run(&env, &["resume", &run_id]);
    let refused_stderr = String::from_utf8_lossy(&refused.stderr).into_owned();
    assert!(
        !refused.status.success() && refused_stderr.contains("stale or unreadable writer lock"),
        "the post-crash resume is refused typed by the single-writer guard: {refused_stderr}"
    );
    std::fs::remove_file(world.state_dir().join("writer.lock"))
        .expect("the operator's manual removal of the stale lock");

    // THE RESUME, under the ADJUDICATED recovery semantics (COR-4
    // report, adjudication section).
    let resume = world.run(&env, &["resume", &run_id]);
    let resume_stdout = String::from_utf8_lossy(&resume.stdout).into_owned();
    assert!(
        resume_stdout.contains("run_id=") || resume_stdout.contains("terminal="),
        "the resume completes: {resume_stdout}"
    );

    // EXACTLY-ONCE-BOUNDED DISPATCH on the resume — never a storm, and
    // never a wasted extra attempt either (task26 correction).
    //
    // F-C4-1 (cor4-report.md) originally OBSERVED "TWO fresh resumed
    // dispatches" and adjudicated it as the recovery's intended shape:
    // the resume's first re-dispatch deterministically inherits the
    // crashed attempt's workspace, "dedupes onto that row, and
    // verifies stale (§14.3)", forcing a second, fresh-workspace
    // attempt before the unit could settle. That was the DEFECT this
    // task fixes, not a feature: §14.3 condition 3 requires evidence
    // "produced in this run AT OR AFTER the transform it certifies" —
    // a fact about WHEN the instrument made bytes — and the resumed
    // pass's first re-dispatch genuinely re-ran the instrument THIS
    // iteration; treating that real, fresh production as stale just
    // because a durable row already named its ref was the bug. The
    // frozen, settled p7 proof
    // `a_crash_before_the_settlement_event_retries_as_if_never_verified`
    // already pins the correct shape — "exactly ONE fresh dispatch" —
    // and this fix moves the resumed-shape leg here to match it: the
    // first resumed re-dispatch now earns its OWN durable production
    // (never mutating or discarding the crashed attempt's), verifies
    // fresh, and settles without needing a second, fresh-workspace
    // attempt.
    //
    // The ruler-owner CandidateRevision this leg's original
    // adjudication raised — "should §18.3 recovery ADOPT the
    // pre-crash production outright, without re-verifying?" — remains
    // OPEN and is untouched by this fix: nothing here credits the
    // pre-crash row itself as sufficient: the settlement below still
    // stands over a FRESH, current-iteration production the resumed
    // pass's own instrument execution actually made, never the
    // crashed attempt's bytes on say-so alone.
    assert_eq!(
        ak951_dispatch_count(&world, &env, &run_id),
        2,
        "the durable AK-951 dispatch count across both passes: the crashed attempt's own \
         dispatch plus exactly ONE fresh resumed dispatch — the frozen p7 shape, not the \
         F-C4-1 defect's wasted extra attempt"
    );

    // THE PRE-CRASH ROW INTACT (write-once, the crashed iteration's own
    // fact); the resumed pass's OWN per-iteration production beside it,
    // under its own ref.
    let rows_after = marker_rows(&world.state_db());
    assert_eq!(
        rows_after.len(),
        2,
        "the resumed pass minted its own production beside the crashed attempt's: \
         {rows_after:?}"
    );
    assert!(
        rows_after.contains(&crashed_row),
        "the pre-crash production row {crashed_row:?} survived the resume untouched \
         (write-once)"
    );
    let freshest_ref = rows_after
        .last()
        .map(|(reference, ..)| reference.clone())
        .expect("the marker locator holds the productions");
    // task26: row identity is (evidence_ref, iteration_no) now — the
    // ref may legitimately repeat because identical bytes were
    // genuinely re-produced; what proves "the resumed pass's own
    // record, not the pre-crash one adopted" is the ROW, keyed by its
    // iteration and ordering anchor.
    let marker_iterations: Vec<_> =
        gauntlet_adapter_state_postgres::test_support::evidence_production_rows_with_iteration(
            &world.state_db(),
        )
        .expect("the iteration-aware production reader runs")
        .into_iter()
        .filter(|(_, locator, ..)| locator.contains("real-marker-report"))
        .collect();
    assert_eq!(
        marker_iterations.len(),
        2,
        "the crashed and resumed productions are two distinct rows: {marker_iterations:?}"
    );
    let crashed_iteration = &marker_iterations[0];
    let freshest_iteration = &marker_iterations[1];
    assert_eq!(
        crashed_iteration.0, freshest_iteration.0,
        "the resumed pass genuinely re-produced the identical bytes (the fixture inherits \
         the crashed workspace on its first resumed dispatch), so the content-addressed ref \
         legitimately repeats: {marker_iterations:?}"
    );
    assert!(
        freshest_iteration.2 > crashed_iteration.2,
        "the freshest row's iteration_no is strictly greater than the crashed row's — a \
         DIFFERENT row, the resumed pass's own: {marker_iterations:?}"
    );
    assert!(
        freshest_iteration.4 > crashed_iteration.4,
        "the freshest row's event-log offset is later than the crashed row's own: \
         {marker_iterations:?}"
    );

    // EXACTLY-ONCE SETTLEMENT over the CURRENT (freshest) ref — one
    // keyed row and one event for AK-951; the fixture's auto-closable
    // pair (AK-951 + AK-954) is exactly two settlement events in total.
    let settlements =
        gauntlet_adapter_state_postgres::test_support::settlement_rows(&world.state_db())
            .expect("the settlement row reader runs");
    let ak951_settlements: Vec<_> = settlements
        .iter()
        .filter(|(unit, ..)| unit == "AK-951")
        .collect();
    assert_eq!(
        ak951_settlements.len(),
        1,
        "exactly-once settlement for AK-951: {settlements:?}"
    );
    assert_eq!(
        ak951_settlements[0].2, freshest_ref,
        "the settlement stands over the CURRENT (freshest) production ref"
    );
    // The ref alone no longer distinguishes the two rows (task26: it
    // legitimately repeats). The settlement's own event-log offset
    // must stand AT OR AFTER the freshest row's offset — a
    // pre-crash-only production (the earlier offset) could never have
    // satisfied a settlement committed this late.
    assert!(
        ak951_settlements[0].3 >= freshest_iteration.4,
        "the settlement (offset {}) stands at or after the freshest production's own \
         offset ({}), never the crashed row's alone",
        ak951_settlements[0].3,
        freshest_iteration.4
    );
    assert_eq!(
        ak951_settlement_event_count(&world.state_db()),
        1,
        "exactly-once settlement EVENT for AK-951"
    );
    assert_eq!(
        settlement_event_count(&world.state_db()),
        2,
        "exactly the fixture's two auto-closable settlements (AK-951 + AK-954)"
    );

    // RUN IDENTITY PRESERVED: the export answers for the same run, and
    // the settlement stands on the run's own durable record.
    let export = world.run(&env, &["export", "--run", &run_id]);
    assert!(
        export.status.success(),
        "the export of the resumed run succeeds: {}",
        String::from_utf8_lossy(&export.stderr)
    );
    let exported = String::from_utf8_lossy(&export.stdout).into_owned();
    assert!(
        exported.contains("\"record\":\"settlement\"") && exported.contains("AK-951"),
        "the settlement stands on the resumed run's own durable record"
    );

    // PRIOR ARTIFACT BYTES INTACT.
    assert!(
        store_holds_text_containing(&world.evidence_dir(), "\"kind\":\"child-work-record\""),
        "the artifact bytes survive the crash and the resume"
    );
}

// ---------------------------------------------------------------------------
// Leg 3 — R16-F2 discharged against the COMPOSED loop: the hostile child
// plants a directory at its own contract path (the poll's typed transport
// error, forever unreachable TimedOut) and stays alive ignoring SIGTERM.
// The composed loop must reach a BOUNDED honest terminal through its own
// machinery, and a subsequent resume must terminate too (liveness
// preserved).
//
// CORRECTIONS ROUND 1 (F6, fixed here and asserted): the transient
// disposition (§11.5 row 2) now retires every unknown-fate invocation
// through the AgentRunner port's own idempotent cancel BEFORE the row-2
// re-dispatch, and a terminal run retires anything still in flight. The
// defect this leg used to pin as evidence — the hostile child OUTLIVING
// the run and the resume (P8's cancel contract enforced nowhere on this
// path) — is closed, and this leg now asserts the CLOSURE: the pinned
// `prior_invocation_retired_before_retry` records name every poisoned
// dispatch with the port-cancel `how` token, and a bounded signal-0
// liveness poll (through the fixture child's own mechanical `alive`
// mode) over every durable forensic handle file's own child id shows no
// survivor of either the run or the resume. The fixture child's `reap`
// mode stays as scoped hygiene — with the fix it has nothing left to
// reap, which is the point.
// ---------------------------------------------------------------------------

/// The child pids recorded in this world's durable forensic handle files
/// (the adapter's own admission record — evidence, not control state),
/// sorted and deduplicated.
fn handle_file_child_pids(world: &World) -> Vec<u64> {
    let mut pids: Vec<u64> = Vec::new();
    let mut stack = vec![world.state_dir().join("runs")];
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
                if let Ok(pid) = digits.parse::<u64>() {
                    pids.push(pid);
                }
            }
        }
    }
    pids.sort_unstable();
    pids.dedup();
    pids
}

/// Whether the process still exists, probed through the fixture child's
/// own mechanical `alive` mode (signal 0 — it delivers nothing; the
/// fixture script's argv contract, the reap mode's symmetric read).
/// Same-user pids only, which this world's own children are.
fn process_alive(child: &Path, pid: u64) -> bool {
    Command::new(child)
        .args(["alive", &pid.to_string()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

/// Reaps this world's surviving child chains, driven by the DURABLE
/// FORENSIC HANDLE FILES the adapter writes at admission (the child pid
/// is the handle's own record) and retired through the fixture child's
/// own mechanical `reap` mode. Test hygiene — with the F6 fix the run
/// retires its own children, so this has nothing left to reap; it stays
/// so a future leak can never escape this world.
fn reap_world_children(world: &World) {
    let child = sole_script_of(&world.target().join("agent"), "agent");
    make_executable(&child);
    // The fixture child's own reap-mode token (fixture protocol; the
    // script's argv contract, never a spelled command line here).
    let reap_mode = "reap";
    for pid in handle_file_child_pids(world) {
        let _ = Command::new(&child)
            .args([reap_mode, &pid.to_string()])
            .status();
    }
}

#[test]
fn the_contract_path_poisoning_reaches_a_bounded_terminal_and_resume_terminates() {
    let world = World::new("poison");
    let mut env = world.env("recovery-poison-seed");
    env.insert("GAUNTLET_CHILD_MODE", "poison".to_owned());
    // The §11.4 stall window is ALSO the loop's own cancel-operation
    // bound (corrections round 1, F6): the graceful leg of a cancel
    // waits this window, then the backend's stop escalates to SIGKILL.
    // The poison child ignores SIGTERM on purpose, so THIS leg tightens
    // the window to keep every retirement inside the leg's own bounded
    // deadlines — the same env knob the fixture's other legs already
    // drive, never a bypass.
    env.insert("GAUNTLET_STALL_TIMEOUT_MS", "2000".to_owned());

    // THE RUN: every dispatch's child poisons its contract path. The run
    // must TERMINATE — bounded, honest, no hang; a hang is this test
    // failing, which is the R16-F2 trigger's whole point.
    let mut child = world.spawn(&env, &["run", "seeded bounded task"]);
    let output = wait_bounded(
        &mut child,
        Duration::from_secs(TERMINATE_BOUND),
        "the poisoned run must reach a bounded terminal",
    );
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert!(
        !output.status.success(),
        "a poisoned run never settles: stdout={stdout}\nstderr={stderr}"
    );
    assert!(
        stdout.contains("terminal=") && !stdout.contains("terminal=settled"),
        "the bounded honest terminal: {stdout}\n{stderr}"
    );
    assert!(
        !stdout.contains("panic") && !stderr.contains("panicked"),
        "the bounded terminal is the loop's own machinery, not a crash: {stdout}\n{stderr}"
    );

    // The poison is DURABLE EVIDENCE: the loop pinned its typed transient
    // runner errors (the contract path that could never be read).
    assert!(
        store_holds_text_containing(&world.evidence_dir(), "transient_runner_error"),
        "the typed transport errors are pinned evidence, not silence"
    );
    let run_id = run_id_of(&output);

    // ZERO settlements: a child that can never write a contract settles
    // nothing.
    assert_eq!(
        settlement_event_count(&world.state_db()),
        0,
        "a poisoned world settles nothing"
    );

    // THE RESUME terminates too (liveness preserved across recovery): a
    // fresh driver over the same run re-enters the same bounded poison
    // world and reaches its own bounded honest terminal.
    let mut resumed = world.spawn(&env, &["resume", &run_id]);
    let resume_output = wait_bounded(
        &mut resumed,
        Duration::from_secs(TERMINATE_BOUND),
        "the resume of a poisoned run must terminate",
    );
    let resume_stdout = String::from_utf8_lossy(&resume_output.stdout).into_owned();
    assert!(
        resume_stdout.contains("terminal=") && !resume_stdout.contains("terminal=settled"),
        "the resume is bounded and honest: {resume_stdout}"
    );
    assert_eq!(
        settlement_event_count(&world.state_db()),
        0,
        "still zero settlements after the resumed pass"
    );

    // CORRECTIONS ROUND 1 (F6): the leak is closed and the closure is
    // asserted. The pinned durable retirement records name every poisoned
    // dispatch, the `how` token proves the retirement was the PORT'S OWN
    // CANCEL (not the typed already-terminal no-op), and — the fact the
    // recorded defect was about — a bounded liveness poll over every
    // durable handle file's own child id shows NO hostile child outlives
    // the run or the resume. A deadline that fires here is the leak back
    // again: a typed failure, never patience.
    assert!(
        store_holds_text_containing(
            &world.evidence_dir(),
            "\"class\":\"prior_invocation_retired_before_retry\"",
        ),
        "every poisoned dispatch's retirement is pinned durable evidence"
    );
    assert!(
        store_holds_text_containing(&world.evidence_dir(), "\"how\":\"cancelled\""),
        "the retirement was the port's own cancel, not a no-op"
    );
    let handle_pids = handle_file_child_pids(&world);
    assert!(
        !handle_pids.is_empty(),
        "the poisoned dispatches left their durable forensic handles"
    );
    let child = sole_script_of(&world.target().join("agent"), "agent");
    make_executable(&child);
    poll_until(
        "every hostile child retired by the run's own cancel machinery",
        Duration::from_secs(LIVENESS_BOUND),
        || {
            let survivors: Vec<u64> = handle_pids
                .iter()
                .copied()
                .filter(|pid| process_alive(&child, *pid))
                .collect();
            survivors
                .is_empty()
                .then(|| format!("all {} handle children retired", handle_pids.len()))
        },
    );

    // THE REAP (test hygiene — see the leg docs): with the F6 fix the
    // run retires its own children, so this has nothing left to reap.
    reap_world_children(&world);
}
