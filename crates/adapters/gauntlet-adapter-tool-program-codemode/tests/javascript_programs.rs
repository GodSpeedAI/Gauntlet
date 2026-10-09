//! JavaScript-native contract checks for the codemode runtime. Each test runs
//! a real JavaScript program through the real host and asserts the port's
//! property for it. Together they cover every property the shared suite
//! names, and they replace the two cases that the shared suite cannot run as
//! written (see `tests/contract_suite.rs`).

mod support;

use std::time::Duration;

use gauntlet_adapter_tool_program_codemode::CodemodeRuntime;
use gauntlet_ports::json::parse;
use gauntlet_ports::tool_catalog::ToolName;
use gauntlet_ports::tool_program::{
    FailureClass, ProgramHost, ProgramLimit, ProgramLimits, ProgramOutcome, ProgramStatus,
    ToolProgramRequest, ToolProgramRuntime, TraceOutcomeKind, trace_digest,
};

use support::{RecordingHost, bun_host, limits, request};

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
