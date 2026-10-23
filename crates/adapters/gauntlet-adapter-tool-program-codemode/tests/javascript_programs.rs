//! JavaScript-native contract checks for the codemode runtime. Each test runs
//! a real JavaScript program through the real host and asserts the port's
//! property for it. Together they cover every property the shared suite
//! names, and they replace the two cases that the shared suite cannot run as
//! written (see `tests/contract_suite.rs`).

mod support;

use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use gauntlet_adapter_tool_program_codemode::{CodemodeRuntime, HostCommand};
use gauntlet_ports::json::{JsonValue, parse};
use gauntlet_ports::tool_catalog::ToolName;
use gauntlet_ports::tool_program::{
    FailureClass, HostRecord, ProgramHost, ProgramLimit, ProgramLimits, ProgramOutcome,
    ProgramStatus, ToolProgramRequest, ToolProgramRuntime, TraceOutcomeKind, trace_digest,
};
use gauntlet_ports::tool_surface::ToolOutcome;

use support::{RecordingHost, bun_host, limits, request, workbench_dir};

fn run(
    text: &str,
    admitted: &[&str],
    bounds: ProgramLimits,
) -> (ProgramOutcome, RecordingHost, ToolProgramRequest) {
    let runtime = CodemodeRuntime::new(bun_host());
    let request = request(text, admitted, bounds);
    let host = RecordingHost::new(&request);
    let outcome = runtime.run(&request, &host);
    (outcome, host, request)
}

fn tool(name: &str) -> ToolName {
    ToolName::new(name).expect("a valid tool name")
}

/// A name that is not admitted is never executed, and the program cannot
/// reach it by any spelling it can name. Replaces `only_admitted_tools_execute`.
#[test]
fn a_program_reaches_only_admitted_tools() {
    let text = r#"await tools.tools.read({});
try { await tools.tools.write({ path: "x" }); return "reached"; } catch (error) { return "refused"; }"#;
    let (outcome, host, _) = run(text, &["tools.read"], limits(8_000, 8, 4, 4096));
    assert_eq!(host.executed(), vec![tool("tools.read")]);
    assert!(
        !outcome
            .trace
            .iter()
            .any(|entry| entry.tool == tool("tools.write") && entry.outcome == TraceOutcomeKind::Ok),
        "a non-admitted name must never execute: {:?}",
        outcome.trace
    );
    assert_eq!(
        outcome.status,
        ProgramStatus::Completed(parse("\"refused\"").expect("json"))
    );
}

/// Past `max_tool_calls` the program is `LimitExceeded(ToolCalls)`, and only
/// the bounded calls executed. Replaces `call_limit_is_enforced_and_reported`.
#[test]
fn the_call_limit_is_enforced_and_reported() {
    let text = r#"await tools.tools.read({});
await tools.tools.read({});
await tools.tools.read({});
return "done";"#;
    let (outcome, host, _) = run(text, &["tools.read"], limits(8_000, 2, 4, 4096));
    assert_eq!(
        outcome.status,
        ProgramStatus::LimitExceeded(ProgramLimit::ToolCalls)
    );
    assert_eq!(host.executed().len(), 2);
}

/// A group over the parallel bound fails the run as `LimitExceeded(Parallel)`.
/// The host refuses the over-bound call inside the sandbox, so the two in-bound
/// calls of the group were sent. Replaces `parallel_limit_refuses_the_group`.
#[test]
fn the_parallel_limit_ends_the_run_at_the_over_bound_call() {
    let text = r#"await Promise.all([tools.tools.read({}), tools.tools.read({}), tools.tools.read({})]);
return "done";"#;
    let (outcome, host, _) = run(text, &["tools.read"], limits(8_000, 8, 2, 4096));
    assert_eq!(
        outcome.status,
        ProgramStatus::LimitExceeded(ProgramLimit::ParallelCalls)
    );
    assert_eq!(
        host.executed().len(),
        2,
        "only the in-bound calls reach the parent; the third is refused in the sandbox"
    );
}

/// The wall-clock bound for the memory program. Exhaustion should come well
/// inside it; if it did not, the run would be `TimedOut`, not a memory limit.
const MEMORY_PROGRAM_TIMEOUT_MS: u64 = 8_000;

/// Each kept string is one MiB, so the 64 MiB guest heap cap is reached after
/// a few dozen live allocations.
const MEMORY_PROGRAM_CHUNK_CHARS: usize = 1 << 20;

/// A program that keeps every allocation alive until the 64 MiB guest heap cap
/// is exhausted is `LimitExceeded(Memory)`, not a runtime failure.
#[test]
fn guest_heap_exhaustion_is_a_memory_limit() {
    let text = format!(
        r#"const kept = [];
while (true) {{ kept.push("x".repeat({MEMORY_PROGRAM_CHUNK_CHARS}) + kept.length); }}"#
    );
    let (outcome, _, _) = run(
        &text,
        &["tools.read"],
        limits(MEMORY_PROGRAM_TIMEOUT_MS, 8, 4, 4096),
    );
    assert_eq!(
        outcome.status,
        ProgramStatus::LimitExceeded(ProgramLimit::Memory)
    );
}

/// A program that never ends is `TimedOut`, never `Completed`.
#[test]
fn a_program_that_never_ends_is_timed_out() {
    let (outcome, _, _) = run("while (true) {}", &["tools.read"], limits(300, 8, 4, 4096));
    assert_eq!(outcome.status, ProgramStatus::TimedOut);
    assert!(
        outcome.duration >= Duration::from_millis(300),
        "{:?}",
        outcome.duration
    );
}

/// A result over the cap is `LimitExceeded(ResultBytes)`, never truncated.
#[test]
fn a_result_over_the_cap_is_a_result_bytes_limit() {
    let (outcome, _, _) = run(
        r#"return "x".repeat(64);"#,
        &["tools.read"],
        limits(8_000, 8, 4, 16),
    );
    assert_eq!(
        outcome.status,
        ProgramStatus::LimitExceeded(ProgramLimit::ResultBytes)
    );
}

/// A returned value is data: it triggers no host call, and the trace holds
/// exactly the one call the program issued.
#[test]
fn a_completed_value_is_data_only() {
    let text = r#"await tools.tools.read({});
return { done: true };"#;
    let (outcome, host, _) = run(text, &["tools.read"], limits(8_000, 8, 4, 4096));
    assert_eq!(
        outcome.status,
        ProgramStatus::Completed(parse(r#"{"done":true}"#).expect("json"))
    );
    assert_eq!(host.executed(), vec![tool("tools.read")]);
    assert_eq!(outcome.trace.len(), 1);
}

/// The outcome's trace is the host's record, and its digests derive from it
/// and from the source as the suite pins them.
#[test]
fn the_trace_is_the_host_record() {
    let text = r#"await tools.tools.read({ n: 1 });
return 1;"#;
    let (outcome, host, request) = run(text, &["tools.read"], limits(8_000, 8, 4, 4096));
    let record = host.record();
    assert_eq!(outcome.trace, record.trace);
    assert_eq!(outcome.trace_digest, trace_digest(&record.trace));
    assert_eq!(outcome.program_digest, *request.source().digest());
}

/// Identical runs have identical trace digests; a different argument changes it.
#[test]
fn the_trace_digest_is_stable_and_argument_sensitive() {
    let same = r#"await tools.tools.read({ n: 1 }); return null;"#;
    let other = r#"await tools.tools.read({ n: 2 }); return null;"#;
    let (first, _, _) = run(same, &["tools.read"], limits(8_000, 8, 4, 4096));
    let (second, _, _) = run(same, &["tools.read"], limits(8_000, 8, 4, 4096));
    let (third, _, _) = run(other, &["tools.read"], limits(8_000, 8, 4, 4096));
    assert_eq!(first.trace_digest, second.trace_digest);
    assert_ne!(first.trace_digest, third.trace_digest);
}

/// A source the runtime cannot parse is `Failed(Script)` and runs no tool.
#[test]
fn a_malformed_program_fails_as_a_script_error() {
    let (outcome, host, _) = run("not a program", &["tools.read"], limits(8_000, 8, 4, 4096));
    assert!(
        matches!(
            outcome.status,
            ProgramStatus::Failed {
                class: FailureClass::Script,
                ..
            }
        ),
        "{:?}",
        outcome.status
    );
    assert!(host.executed().is_empty());
}

/// The wall-clock bound of the cancelled program. A cancel must end the run well
/// inside it.
const CANCEL_PROGRAM_TIMEOUT_MS: u64 = 8_000;

/// When the owning invocation is cancelled, measured from the start of the run.
const CANCEL_AFTER_MS: u64 = 200;

/// A tool-call bound the program cannot reach within the cancel window, so only
/// the cancel can end it.
const CANCEL_PROGRAM_MAX_CALLS: u32 = 10_000_000;

/// How long the test waits for the host process to be reaped after the run.
const HOST_GONE_BOUND: Duration = Duration::from_secs(3);

/// The pause between checks while the test waits on the host or the process.
const WAIT_STEP: Duration = Duration::from_millis(20);

/// Wraps the recording host and counts every call that arrives after the owner
/// reports cancellation. The owner's flag is the only cancellation source.
struct CancellingHost<'a> {
    inner: &'a RecordingHost,
    cancelled: Arc<AtomicBool>,
    calls_seen: Arc<AtomicU32>,
    calls_after_cancel: AtomicU32,
}

impl CancellingHost<'_> {
    fn count(&self) {
        self.calls_seen.fetch_add(1, Ordering::SeqCst);
        if self.cancelled.load(Ordering::SeqCst) {
            self.calls_after_cancel.fetch_add(1, Ordering::SeqCst);
        }
    }
}

impl ProgramHost for CancellingHost<'_> {
    fn call(&self, name: &ToolName, args: &JsonValue) -> ToolOutcome {
        self.count();
        self.inner.call(name, args)
    }

    fn call_group(&self, calls: &[(ToolName, JsonValue)]) -> Vec<ToolOutcome> {
        for _ in calls {
            self.count();
        }
        self.inner.call_group(calls)
    }

    fn record(&self) -> HostRecord {
        self.inner.record()
    }

    fn cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
}

/// The real host, started through `sh` so the shell writes the host's own pid
/// and then `exec`s into it: the pid is the Bun process.
fn pid_recording_host(pid_file: &Path) -> HostCommand {
    // `bun_host()` fails loudly when bun or the workbench host is absent.
    let _ = bun_host();
    HostCommand {
        executable: "sh".to_owned(),
        args: vec![
            "-c".to_owned(),
            r#"echo $$ > "$1"; exec bun run codemode-host"#.to_owned(),
            "sh".to_owned(),
            pid_file.display().to_string(),
        ],
        workdir: workbench_dir(),
    }
}

/// Whether `pid` is gone: absent, or a zombie that no longer runs.
fn host_process_is_gone(pid: u32) -> bool {
    match fs::read_to_string(format!("/proc/{pid}/stat")) {
        Err(_) => true,
        Ok(stat) => stat
            .rsplit(')')
            .next()
            .map(str::trim_start)
            .is_some_and(|state| state.starts_with('Z')),
    }
}

/// A program that awaits cheap tool calls forever. Only the owner's cancel can
/// end it inside the window the test allows, so the run must end as `Cancelled`
/// with no call after the cancel and the Bun host process gone.
#[test]
fn a_cancelled_owner_stops_the_program_before_its_timeout() {
    let text = r#"for (let i = 0; ; i += 1) { await tools.tools.read({ n: i }); }"#;
    let bounds = limits(CANCEL_PROGRAM_TIMEOUT_MS, CANCEL_PROGRAM_MAX_CALLS, 4, 4096);
    let request = request(text, &["tools.read"], bounds);
    let recording = RecordingHost::new(&request);
    let cancelled = Arc::new(AtomicBool::new(false));
    let calls_seen = Arc::new(AtomicU32::new(0));
    let host = CancellingHost {
        inner: &recording,
        cancelled: Arc::clone(&cancelled),
        calls_seen: Arc::clone(&calls_seen),
        calls_after_cancel: AtomicU32::new(0),
    };

    let pid_file = Path::new(env!("CARGO_TARGET_TMPDIR")).join("codemode-cancel-host.pid");
    let _ = fs::remove_file(&pid_file);
    // The cancel lands CANCEL_AFTER_MS after the first call, so the run has
    // made calls to stop; a cold Bun start does not eat into that window.
    let flip = Arc::clone(&cancelled);
    let first_call = Arc::clone(&calls_seen);
    thread::spawn(move || {
        let waited = Instant::now();
        while first_call.load(Ordering::SeqCst) == 0
            && waited.elapsed() < Duration::from_millis(CANCEL_PROGRAM_TIMEOUT_MS)
        {
            thread::sleep(WAIT_STEP);
        }
        thread::sleep(Duration::from_millis(CANCEL_AFTER_MS));
        flip.store(true, Ordering::SeqCst);
    });

    let runtime = CodemodeRuntime::new(pid_recording_host(&pid_file));
    let outcome = runtime.run(&request, &host);

    assert_eq!(
        outcome.status,
        ProgramStatus::Cancelled,
        "a cancelled owner must end the run as Cancelled, not {:?}",
        outcome.status
    );
    assert!(
        outcome.duration < Duration::from_millis(CANCEL_PROGRAM_TIMEOUT_MS) / 2,
        "the cancelled run took {:?}; it must end well before its timeout",
        outcome.duration
    );
    assert!(
        !recording.executed().is_empty(),
        "the program must have made calls before the cancel"
    );
    assert_eq!(
        host.calls_after_cancel.load(Ordering::SeqCst),
        0,
        "no host call may arrive after the owner is cancelled"
    );

    let pid: u32 = fs::read_to_string(&pid_file)
        .expect("the host recorded its pid before the run")
        .trim()
        .parse()
        .expect("the pid file holds a pid");
    let started = Instant::now();
    while !host_process_is_gone(pid) && started.elapsed() < HOST_GONE_BOUND {
        thread::sleep(WAIT_STEP);
    }
    assert!(
        host_process_is_gone(pid),
        "the Bun host process {pid} survived a cancelled run"
    );
}
