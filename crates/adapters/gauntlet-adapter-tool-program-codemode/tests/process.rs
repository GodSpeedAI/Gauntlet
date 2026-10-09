//! Process-level tests: a hostile or broken child fails closed, a run past its
//! bound is cancelled and its whole process group is killed and reaped, and
//! the child's environment is the allowlist.
//!
//! The children are small `sh` scripts written per test, so each protocol
//! violation is exact and repeatable. None of these tests needs bun.

mod support;

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use gauntlet_adapter_tool_program_codemode::{CodemodeRuntime, HostCommand};
use gauntlet_ports::tool_program::{
    FailureClass, ProgramLimits, ProgramOutcome, ProgramStatus, ToolProgramRequest,
    ToolProgramRuntime,
};

use support::{RecordingHost, limits, request};

/// A private directory for one test's scripts and marker files.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("gauntlet-codemode-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("the scratch directory is created");
    dir
}

/// A host command that runs `script` with `sh`.
fn sh_host(dir: &Path, script: &str) -> HostCommand {
    let path = dir.join("host.sh");
    fs::write(&path, script).expect("the fake host is written");
    HostCommand {
        executable: "sh".to_owned(),
        args: vec![path.display().to_string()],
        workdir: dir.to_path_buf(),
    }
}

fn run_with(
    host: HostCommand,
    grace: Duration,
    text: &str,
    bounds: ProgramLimits,
) -> (ProgramOutcome, RecordingHost, ToolProgramRequest) {
    let runtime = CodemodeRuntime::new(host).with_grace(grace);
    let request = request(text, &["tools.read"], bounds);
    let recording = RecordingHost::new(&request);
    let outcome = runtime.run(&request, &recording);
    (outcome, recording, request)
}

fn failure_message(outcome: &ProgramOutcome) -> String {
    match &outcome.status {
        ProgramStatus::Failed { class, message } => {
            assert_eq!(
                *class,
                FailureClass::Runtime,
                "a host fault is a runtime failure"
            );
            message.clone()
        }
        other => panic!("expected a typed failure, got {other:?}"),
    }
}

const GRACE: Duration = Duration::from_millis(300);

#[test]
fn a_line_over_the_cap_fails_closed() {
    let dir = scratch("oversize");
    let host = sh_host(
        &dir,
        "read -r _run\nhead -c 9000000 /dev/zero | tr '\\0' 'a'\necho\n",
    );
    let (outcome, _, _) = run_with(host, GRACE, "return 1;", limits(5_000, 8, 4, 4096));
    assert!(failure_message(&outcome).contains("exceeds"), "{outcome:?}");
}

#[test]
fn a_line_that_is_not_json_fails_closed() {
    let dir = scratch("garbage");
    let host = sh_host(&dir, "read -r _run\necho 'this is not json'\n");
    let (outcome, _, _) = run_with(host, GRACE, "return 1;", limits(5_000, 8, 4, 4096));
    assert!(
        failure_message(&outcome).contains("not JSON"),
        "{outcome:?}"
    );
}

#[test]
fn a_wrong_protocol_version_fails_closed() {
    let dir = scratch("version");
    let host = sh_host(
        &dir,
        "read -r _run\necho '{\"v\":2,\"t\":\"done\",\"status\":\"completed\",\"value\":1}'\n",
    );
    let (outcome, _, _) = run_with(host, GRACE, "return 1;", limits(5_000, 8, 4, 4096));
    assert!(failure_message(&outcome).contains("version"), "{outcome:?}");
}

#[test]
fn a_host_that_exits_without_done_fails_closed_with_its_stderr() {
    let dir = scratch("crash");
    let host = sh_host(
        &dir,
        "read -r _run\necho 'boom from the host' >&2\nexit 3\n",
    );
    let (outcome, _, _) = run_with(host, GRACE, "return 1;", limits(5_000, 8, 4, 4096));
    let message = failure_message(&outcome);
    assert!(
        message.contains("closed its output before done"),
        "{message}"
    );
    assert!(message.contains("boom from the host"), "{message}");
}

#[test]
fn a_call_to_a_tool_that_was_not_admitted_fails_closed() {
    let dir = scratch("not-admitted");
    let host = sh_host(
        &dir,
        "read -r _run\n\
         echo '{\"v\":1,\"t\":\"call\",\"id\":1,\"tool\":\"tools.write\",\"args\":{}}'\n\
         read -r _result\n\
         echo '{\"v\":1,\"t\":\"done\",\"status\":\"completed\",\"value\":1}'\n",
    );
    let (outcome, host, _) = run_with(host, GRACE, "return 1;", limits(5_000, 8, 4, 4096));
    assert!(
        failure_message(&outcome).contains("not admitted"),
        "{outcome:?}"
    );
    assert!(
        host.executed().is_empty(),
        "a non-admitted call must never execute"
    );
}

#[test]
fn a_repeated_call_id_fails_closed() {
    let dir = scratch("repeat-id");
    let call = "echo '{\"v\":1,\"t\":\"call\",\"id\":1,\"tool\":\"tools.read\",\"args\":{}}'";
    let host = sh_host(
        &dir,
        &format!(
            "read -r _run\n{call}\n{call}\necho '{{\"v\":1,\"t\":\"done\",\"status\":\"completed\",\"value\":1}}'\n"
        ),
    );
    let (outcome, _, _) = run_with(host, GRACE, "return 1;", limits(5_000, 8, 4, 4096));
    assert!(failure_message(&outcome).contains("repeats"), "{outcome:?}");
}

/// A call the program never awaited can still be in flight when the program
/// finishes. The host reports `done` and the run completes; the call is
/// recorded by the host, and the trace comes from that record.
#[test]
fn an_unawaited_call_is_recorded_and_the_program_completes() {
    let dir = scratch("unawaited");
    let host = sh_host(
        &dir,
        "read -r _run\n\
         echo '{\"v\":1,\"t\":\"call\",\"id\":1,\"tool\":\"tools.read\",\"args\":{\"n\":1}}'\n\
         echo '{\"v\":1,\"t\":\"done\",\"status\":\"completed\",\"value\":5}'\n",
    );
    let (outcome, recording, _) = run_with(
        host,
        GRACE,
        "tools.read({n:1}); return 5;",
        limits(5_000, 8, 4, 4096),
    );
    assert_eq!(
        outcome.status,
        ProgramStatus::Completed(gauntlet_ports::json::parse("5").expect("json"))
    );
    assert_eq!(recording.executed().len(), 1);
    assert_eq!(
        outcome.trace,
        gauntlet_ports::tool_program::ProgramHost::record(&recording).trace
    );
}

#[test]
fn a_run_past_its_bound_is_cancelled_and_its_group_is_killed() {
    let dir = scratch("timeout-group");
    let pid_file = dir.join("grandchild.pid");
    let host = sh_host(
        &dir,
        &format!(
            "read -r _run\nsleep 30 &\necho $! > '{}'\nwait\n",
            pid_file.display()
        ),
    );
    let (outcome, _, _) = run_with(host, GRACE, "return 1;", limits(200, 8, 4, 4096));
    assert_eq!(outcome.status, ProgramStatus::TimedOut);
    assert!(
        outcome.duration < Duration::from_secs(5),
        "{:?}",
        outcome.duration
    );

    let pid: u32 = fs::read_to_string(&pid_file)
        .expect("the grandchild recorded its pid before the kill")
        .trim()
        .parse()
        .expect("the pid file holds a pid");
    assert!(
        wait_until_gone(pid, Duration::from_secs(3)),
        "the grandchild {pid} survived the group kill"
    );
}

#[test]
fn the_cancel_reaches_the_host_before_the_group_is_killed() {
    let dir = scratch("cancel");
    let marker = dir.join("cancelled");
    let host = sh_host(
        &dir,
        &format!(
            "read -r _run\n\
             while read -r line; do\n\
               case \"$line\" in\n\
                 *'\"t\":\"cancel\"'*)\n\
                   echo cancelled > '{}'\n\
                   echo '{{\"v\":1,\"t\":\"done\",\"status\":\"cancelled\",\"value\":null,\"message\":\"cancelled\"}}'\n\
                   exit 0;;\n\
               esac\n\
             done\n",
            marker.display()
        ),
    );
    let (outcome, _, _) = run_with(
        host,
        Duration::from_millis(200),
        "return 1;",
        limits(200, 8, 4, 4096),
    );
    assert_eq!(outcome.status, ProgramStatus::TimedOut);
    assert!(
        marker.is_file(),
        "the host must receive the cancel before the kill"
    );
}

#[test]
fn the_child_inherits_only_the_allowlisted_environment() {
    let dir = scratch("env");
    let dump = dir.join("env.txt");
    let host = sh_host(
        &dir,
        &format!(
            "read -r _run\nenv > '{}'\necho '{{\"v\":1,\"t\":\"done\",\"status\":\"completed\",\"value\":1}}'\n",
            dump.display()
        ),
    );
    let (outcome, _, _) = run_with(host, GRACE, "return 1;", limits(5_000, 8, 4, 4096));
    assert_eq!(
        outcome.status,
        ProgramStatus::Completed(gauntlet_ports::json::parse("1").expect("json"))
    );
    let env = fs::read_to_string(&dump).expect("the child wrote its environment");
    assert!(
        !env.lines().any(|line| line.starts_with("HOME=")),
        "HOME must not leak:\n{env}"
    );
    assert!(
        !env.lines().any(|line| line.starts_with("CARGO_")),
        "no build variable may leak:\n{env}"
    );
}

#[test]
fn a_host_that_cannot_start_is_a_typed_failure_and_nothing_runs() {
    let dir = scratch("spawn");
    let host = HostCommand {
        executable: "/nonexistent/gauntlet-codemode-host".to_owned(),
        args: Vec::new(),
        workdir: dir,
    };
    let (outcome, _, _) = run_with(host, GRACE, "return 1;", limits(5_000, 8, 4, 4096));
    assert!(
        failure_message(&outcome).contains("could not start"),
        "{outcome:?}"
    );
}

#[test]
fn zero_parallel_calls_is_refused_before_any_process_starts() {
    let dir = scratch("zero-parallel");
    let marker = dir.join("started");
    let host = sh_host(&dir, &format!("echo started > '{}'\n", marker.display()));
    let (outcome, _, _) = run_with(host, GRACE, "return 1;", limits(5_000, 8, 0, 4096));
    assert!(
        failure_message(&outcome).contains("max_parallel_calls"),
        "{outcome:?}"
    );
    assert!(!marker.exists(), "no host may start for invalid limits");
}

/// Whether `pid` is gone: absent, or a zombie that no longer runs.
fn wait_until_gone(pid: u32, bound: Duration) -> bool {
    let until = std::time::Instant::now() + bound;
    loop {
        if process_is_gone(pid) {
            return true;
        }
        if std::time::Instant::now() >= until {
            return false;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn process_is_gone(pid: u32) -> bool {
    match fs::read_to_string(format!("/proc/{pid}/stat")) {
        Err(_) => true,
        Ok(stat) => stat
            .rsplit(')')
            .next()
            .and_then(|rest| rest.split_whitespace().next())
            .is_some_and(|state| state == "Z"),
    }
}
