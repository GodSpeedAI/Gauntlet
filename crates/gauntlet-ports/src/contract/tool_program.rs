//! The `ToolProgramRuntime` shared contract suite (H1). Runtime-independent: it
//! runs any runtime built by a [`ToolProgramFactory`] against a recording
//! [`ContractHost`], so the same cases hold for the reference runtime and for a
//! future engine-backed runtime.
//!
//! Pinned semantics:
//!
//! - A program reaches a tool only through the host, and the host dispatches
//!   only admitted tools. A refused name is recorded, never executed.
//! - Every bound breach is a typed [`ProgramStatus`], never silence.
//! - The trace is the host's record, and its digest is a function of that
//!   record alone.
//! - A `Completed` value is data. It triggers no host call and marks nothing
//!   complete.

use std::cell::RefCell;

use gauntlet_domain::{Digest, InvocationId};

use crate::contract::ContractFinding;
use crate::json::{JsonValue, parse};
use crate::tool_catalog::ToolName;
use crate::tool_program::{
    FailureClass, HostRecord, ProgramHost, ProgramLimit, ProgramLimits, ProgramOutcome,
    ProgramSource, ProgramStatus, ToolProgramRequest, ToolProgramRuntime, TraceEntry,
    TraceOutcomeKind, args_digest, trace_digest,
};
use crate::tool_surface::ToolOutcome;

/// The suite's name in findings.
pub const SUITE: &str = "tool_program";

const CALLER_TEXT: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
const READ: &str = "tools.read";
const WRITE: &str = "tools.write";
/// The refusal code for a name the request did not admit.
pub const NOT_ADMITTED: &str = "not_admitted";
/// The refusal code for a call past a bound.
pub const LIMIT_REFUSED: &str = "limit_refused";

/// Constructs the runtime under test.
pub trait ToolProgramFactory {
    fn label(&self) -> &'static str;

    fn build(&self) -> Result<Box<dyn ToolProgramRuntime>, String>;
}

/// The host the suite runs against. It enforces admission and the call bounds
/// the way a real host must, and it records the tools that actually executed.
pub struct ContractHost {
    admitted: Vec<ToolName>,
    limits: ProgramLimits,
    /// The host reports cancellation once this many calls have executed. `None`
    /// never cancels; `Some(0)` is cancelled from the start.
    cancel_after: Option<usize>,
    state: RefCell<ContractState>,
}

#[derive(Default)]
struct ContractState {
    trace: Vec<TraceEntry>,
    executed: Vec<ToolName>,
    attempts: u32,
    in_flight: u32,
    breach: Option<ProgramLimit>,
    /// Calls the host received while it already reported cancellation.
    calls_after_cancel: u32,
}

impl ContractHost {
    fn new(admitted: Vec<ToolName>, limits: ProgramLimits, cancel_after: Option<usize>) -> Self {
        Self {
            admitted,
            limits,
            cancel_after,
            state: RefCell::new(ContractState::default()),
        }
    }

    fn cancelled_in(&self, state: &ContractState) -> bool {
        self.cancel_after
            .is_some_and(|after| state.executed.len() >= after)
    }

    /// Calls that arrived after the host already reported cancellation.
    fn calls_after_cancel(&self) -> u32 {
        self.state.borrow().calls_after_cancel
    }

    /// The tools that executed, in order. A refused or limited call never
    /// appears here.
    fn executed(&self) -> Vec<ToolName> {
        self.state.borrow().executed.clone()
    }

    fn dispatch(
        &self,
        state: &mut ContractState,
        name: &ToolName,
        args: &JsonValue,
    ) -> ToolOutcome {
        if self.cancelled_in(state) {
            state.calls_after_cancel = state.calls_after_cancel.saturating_add(1);
        }
        let seq = u32::try_from(state.trace.len()).unwrap_or(u32::MAX);
        let digest = args_digest(args);
        let mut record = |outcome: TraceOutcomeKind| {
            state.trace.push(TraceEntry {
                seq,
                tool: name.clone(),
                args_digest: digest.clone(),
                outcome,
            });
        };
        if state.breach.is_some() {
            record(TraceOutcomeKind::Limit);
            return refused(LIMIT_REFUSED);
        }
        state.attempts = state.attempts.saturating_add(1);
        if state.attempts > self.limits.max_tool_calls {
            state.breach = Some(ProgramLimit::ToolCalls);
            record(TraceOutcomeKind::Limit);
            return refused(LIMIT_REFUSED);
        }
        if !self.admitted.contains(name) {
            record(TraceOutcomeKind::Refused);
            return refused(NOT_ADMITTED);
        }
        record(TraceOutcomeKind::Ok);
        state.executed.push(name.clone());
        ToolOutcome::Ok(JsonValue::Object(vec![(
            "ok".to_owned(),
            JsonValue::Bool(true),
        )]))
    }
}

fn refused(code: &str) -> ToolOutcome {
    ToolOutcome::Refused {
        code: code.to_owned(),
        detail: "refused by the contract host".to_owned(),
    }
}

impl ProgramHost for ContractHost {
    fn call(&self, name: &ToolName, args: &JsonValue) -> ToolOutcome {
        let mut state = self.state.borrow_mut();
        self.dispatch(&mut state, name, args)
    }

    fn cancelled(&self) -> bool {
        let state = self.state.borrow();
        self.cancelled_in(&state)
    }

    fn call_group(&self, calls: &[(ToolName, JsonValue)]) -> Vec<ToolOutcome> {
        let mut state = self.state.borrow_mut();
        let size = u32::try_from(calls.len()).unwrap_or(u32::MAX);
        if state.in_flight.saturating_add(size) > self.limits.max_parallel_calls {
            state.breach = Some(ProgramLimit::ParallelCalls);
            return calls
                .iter()
                .map(|(name, args)| {
                    let seq = u32::try_from(state.trace.len()).unwrap_or(u32::MAX);
                    state.trace.push(TraceEntry {
                        seq,
                        tool: name.clone(),
                        args_digest: args_digest(args),
                        outcome: TraceOutcomeKind::Limit,
                    });
                    refused(LIMIT_REFUSED)
                })
                .collect();
        }
        state.in_flight += size;
        let outcomes = calls
            .iter()
            .map(|(name, args)| self.dispatch(&mut state, name, args))
            .collect();
        state.in_flight -= size;
        outcomes
    }

    fn record(&self) -> HostRecord {
        let state = self.state.borrow();
        HostRecord {
            trace: state.trace.clone(),
            breach: state.breach,
        }
    }
}

fn finding(case: &'static str, rule: &'static str, detail: String) -> ContractFinding {
    ContractFinding {
        suite: SUITE,
        case,
        rule,
        detail,
    }
}

fn caller() -> InvocationId {
    InvocationId::new(CALLER_TEXT).expect("suite caller is a valid invocation id")
}

fn tool(text: &str) -> ToolName {
    ToolName::new(text).expect("suite tool names are valid")
}

/// The source cap every case uses.
const SOURCE_CAP_BYTES: usize = 64 * 1024;

fn limits(max_tool_calls: u32, max_parallel_calls: u32, max_result_bytes: usize) -> ProgramLimits {
    ProgramLimits {
        timeout: core::time::Duration::from_secs(1),
        max_tool_calls,
        max_parallel_calls,
        max_result_bytes,
        max_source_bytes: SOURCE_CAP_BYTES,
    }
}

fn request(
    text: &str,
    admitted: &[&str],
    limits: ProgramLimits,
) -> Result<ToolProgramRequest, String> {
    let source = ProgramSource::new(text, limits.max_source_bytes).map_err(|e| e.to_string())?;
    let admitted = admitted.iter().map(|name| tool(name)).collect();
    ToolProgramRequest::new(caller(), source, admitted, limits).map_err(|e| e.to_string())
}

/// Builds the runtime and runs one program. A build or request failure is a
/// finding with the case name, never a panic.
fn run(
    factory: &dyn ToolProgramFactory,
    case: &'static str,
    rule: &'static str,
    text: &str,
    admitted: &[&str],
    limits: ProgramLimits,
) -> Result<(ProgramOutcome, ContractHost, ToolProgramRequest), Vec<ContractFinding>> {
    run_with_cancel(factory, case, rule, text, admitted, limits, None)
}

/// As [`run`], with a host that reports cancellation after `cancel_after`
/// executed calls.
fn run_with_cancel(
    factory: &dyn ToolProgramFactory,
    case: &'static str,
    rule: &'static str,
    text: &str,
    admitted: &[&str],
    limits: ProgramLimits,
    cancel_after: Option<usize>,
) -> Result<(ProgramOutcome, ContractHost, ToolProgramRequest), Vec<ContractFinding>> {
    let runtime = factory.build().map_err(|detail| {
        vec![finding(
            case,
            rule,
            format!("the factory could not build a runtime: {detail}"),
        )]
    })?;
    let request = request(text, admitted, limits).map_err(|detail| {
        vec![finding(
            case,
            rule,
            format!("the case request was refused: {detail}"),
        )]
    })?;
    let host = ContractHost::new(request.admitted().to_vec(), limits, cancel_after);
    let outcome = runtime.run(&request, &host);
    Ok((outcome, host, request))
}

/// Case: a program reaches only admitted tools. A name outside the admitted set
/// is recorded as refused and never executes.
fn only_admitted_tools_execute(factory: &dyn ToolProgramFactory) -> Vec<ContractFinding> {
    const CASE: &str = "only_admitted_tools_execute";
    const RULE: &str =
        "a tool program names only tools admitted to its request; a refused name never executes";
    let text = format!(r#"[{{"call":"{READ}"}},{{"call":"{WRITE}","args":{{"path":"x"}}}}]"#);
    let (outcome, host, _) = match run(factory, CASE, RULE, &text, &[READ], limits(8, 4, 4096)) {
        Ok(parts) => parts,
        Err(findings) => return findings,
    };
    let mut findings = Vec::new();
    if host.executed() != [tool(READ)] {
        findings.push(finding(
            CASE,
            RULE,
            format!("the executed tools were {:?}", host.executed()),
        ));
    }
    let refused_write = outcome
        .trace
        .iter()
        .any(|entry| entry.tool == tool(WRITE) && entry.outcome == TraceOutcomeKind::Refused);
    if !refused_write {
        findings.push(finding(
            CASE,
            RULE,
            "the refused name is absent from the trace as a refusal".to_owned(),
        ));
    }
    findings
}

/// Case: the call-count bound is enforced and reported as `LimitExceeded`.
fn call_limit_is_enforced_and_reported(factory: &dyn ToolProgramFactory) -> Vec<ContractFinding> {
    const CASE: &str = "call_limit_is_enforced_and_reported";
    const RULE: &str = "a program past max_tool_calls is LimitExceeded(ToolCalls), never Completed";
    let text = format!(r#"[{{"call":"{READ}"}},{{"call":"{READ}"}},{{"call":"{READ}"}}]"#);
    let (outcome, host, _) = match run(factory, CASE, RULE, &text, &[READ], limits(2, 4, 4096)) {
        Ok(parts) => parts,
        Err(findings) => return findings,
    };
    let mut findings = Vec::new();
    if outcome.status != ProgramStatus::LimitExceeded(ProgramLimit::ToolCalls) {
        findings.push(finding(
            CASE,
            RULE,
            format!("the status was {:?}", outcome.status),
        ));
    }
    if host.executed().len() != 2 {
        findings.push(finding(
            CASE,
            RULE,
            format!("{} tools executed; the bound is 2", host.executed().len()),
        ));
    }
    findings
}

/// Case: a group larger than the parallel bound is refused whole, and nothing
/// in it executes.
fn parallel_limit_refuses_the_group(factory: &dyn ToolProgramFactory) -> Vec<ContractFinding> {
    const CASE: &str = "parallel_limit_refuses_the_group";
    const RULE: &str =
        "a call group over max_parallel_calls is LimitExceeded(ParallelCalls) and executes nothing";
    let text =
        format!(r#"[{{"group":[{{"call":"{READ}"}},{{"call":"{READ}"}},{{"call":"{READ}"}}]}}]"#);
    let (outcome, host, _) = match run(factory, CASE, RULE, &text, &[READ], limits(8, 2, 4096)) {
        Ok(parts) => parts,
        Err(findings) => return findings,
    };
    let mut findings = Vec::new();
    if outcome.status != ProgramStatus::LimitExceeded(ProgramLimit::ParallelCalls) {
        findings.push(finding(
            CASE,
            RULE,
            format!("the status was {:?}", outcome.status),
        ));
    }
    if !host.executed().is_empty() {
        findings.push(finding(
            CASE,
            RULE,
            format!(
                "{} tools executed from a refused group",
                host.executed().len()
            ),
        ));
    }
    findings
}

/// Case: a program that never ends is `TimedOut`, not `Completed`.
fn timeout_is_a_typed_status(factory: &dyn ToolProgramFactory) -> Vec<ContractFinding> {
    const CASE: &str = "timeout_is_a_typed_status";
    const RULE: &str = "a program past its timeout is TimedOut, never Completed";
    let (outcome, _, _) = match run(
        factory,
        CASE,
        RULE,
        r#"[{"spin":true}]"#,
        &[READ],
        limits(8, 4, 4096),
    ) {
        Ok(parts) => parts,
        Err(findings) => return findings,
    };
    if outcome.status == ProgramStatus::TimedOut {
        Vec::new()
    } else {
        vec![finding(
            CASE,
            RULE,
            format!("the status was {:?}", outcome.status),
        )]
    }
}

/// Case: a `Completed` value is data. It triggers no host call beyond the calls
/// the program issued, and the trace holds exactly those calls.
fn completed_is_data_only(factory: &dyn ToolProgramFactory) -> Vec<ContractFinding> {
    const CASE: &str = "completed_is_data_only";
    const RULE: &str =
        "a Completed value marks nothing complete; the host sees only the calls the program issued";
    let text = format!(r#"[{{"call":"{READ}"}},{{"return":{{"done":true}}}}]"#);
    let (outcome, host, _) = match run(factory, CASE, RULE, &text, &[READ], limits(8, 4, 4096)) {
        Ok(parts) => parts,
        Err(findings) => return findings,
    };
    let mut findings = Vec::new();
    match &outcome.status {
        ProgramStatus::Completed(value) => {
            if value != &parse(r#"{"done":true}"#).unwrap_or(JsonValue::Null) {
                findings.push(finding(
                    CASE,
                    RULE,
                    "the completed value was not the value the program returned".to_owned(),
                ));
            }
        }
        other => findings.push(finding(CASE, RULE, format!("the status was {other:?}"))),
    }
    if host.executed() != [tool(READ)] || outcome.trace.len() != 1 {
        findings.push(finding(
            CASE,
            RULE,
            format!(
                "executed {:?} with {} trace entries; expected one call",
                host.executed(),
                outcome.trace.len()
            ),
        ));
    }
    findings
}

/// Case: the outcome's trace and digests are the host's record, not the
/// runtime's claim.
fn trace_is_the_host_record(factory: &dyn ToolProgramFactory) -> Vec<ContractFinding> {
    const CASE: &str = "trace_is_the_host_record";
    const RULE: &str = "the outcome trace is the host's record; trace_digest and program_digest derive from it and the source";
    let text = format!(r#"[{{"call":"{READ}","args":{{"n":1}}}},{{"call":"{WRITE}"}}]"#);
    let (outcome, host, request) =
        match run(factory, CASE, RULE, &text, &[READ], limits(8, 4, 4096)) {
            Ok(parts) => parts,
            Err(findings) => return findings,
        };
    let record = host.record();
    let mut findings = Vec::new();
    if outcome.trace != record.trace {
        findings.push(finding(
            CASE,
            RULE,
            "the outcome trace differs from the host's record".to_owned(),
        ));
    }
    if outcome.trace_digest != trace_digest(&record.trace) {
        findings.push(finding(
            CASE,
            RULE,
            "trace_digest does not derive from the trace".to_owned(),
        ));
    }
    if outcome.program_digest != *request.source().digest() {
        findings.push(finding(
            CASE,
            RULE,
            "program_digest does not derive from the source".to_owned(),
        ));
    }
    findings
}

/// Case: the trace digest is stable for an identical run and changes when only
/// an argument changes.
fn trace_digest_is_stable_and_argument_sensitive(
    factory: &dyn ToolProgramFactory,
) -> Vec<ContractFinding> {
    const CASE: &str = "trace_digest_is_stable_and_argument_sensitive";
    const RULE: &str =
        "identical runs have identical trace digests; a different argument changes it";
    let same = format!(r#"[{{"call":"{READ}","args":{{"n":1}}}}]"#);
    let other = format!(r#"[{{"call":"{READ}","args":{{"n":2}}}}]"#);
    let mut digests: Vec<Digest> = Vec::new();
    for text in [same.as_str(), same.as_str(), other.as_str()] {
        match run(factory, CASE, RULE, text, &[READ], limits(8, 4, 4096)) {
            Ok((outcome, _, _)) => digests.push(outcome.trace_digest),
            Err(findings) => return findings,
        }
    }
    let mut findings = Vec::new();
    if digests[0] != digests[1] {
        findings.push(finding(
            CASE,
            RULE,
            "two identical runs produced different trace digests".to_owned(),
        ));
    }
    if digests[0] == digests[2] {
        findings.push(finding(
            CASE,
            RULE,
            "a different argument produced the same trace digest".to_owned(),
        ));
    }
    findings
}

/// Case: a host that reports cancellation stops the run as `Cancelled`, and
/// the runtime issues no call after the report. A host cancelled from the start
/// sees no call at all; a host cancelled after one call sees exactly that call.
fn cancellation_stops_further_calls(factory: &dyn ToolProgramFactory) -> Vec<ContractFinding> {
    const CASE: &str = "cancellation_stops_further_calls";
    const RULE: &str = "a host reporting cancellation gets no call after the report; the run ends Cancelled, never Completed";
    let text = format!(r#"[{{"call":"{READ}"}},{{"call":"{READ}"}},{{"return":"done"}}]"#);
    let mut findings = Vec::new();

    let bounds = limits(8, 4, 4096);
    let (outcome, host, _) =
        match run_with_cancel(factory, CASE, RULE, &text, &[READ], bounds, Some(0)) {
            Ok(parts) => parts,
            Err(found) => return found,
        };
    if outcome.status != ProgramStatus::Cancelled {
        findings.push(finding(
            CASE,
            RULE,
            format!(
                "cancelled from the start, the status was {:?}",
                outcome.status
            ),
        ));
    }
    if !host.executed().is_empty() || host.calls_after_cancel() != 0 {
        findings.push(finding(
            CASE,
            RULE,
            format!(
                "cancelled from the start, {} tools executed and {} calls arrived after the report",
                host.executed().len(),
                host.calls_after_cancel()
            ),
        ));
    }

    let (outcome, host, _) =
        match run_with_cancel(factory, CASE, RULE, &text, &[READ], bounds, Some(1)) {
            Ok(parts) => parts,
            Err(found) => return found,
        };
    if outcome.status != ProgramStatus::Cancelled {
        findings.push(finding(
            CASE,
            RULE,
            format!(
                "cancelled after one call, the status was {:?}",
                outcome.status
            ),
        ));
    }
    if host.executed() != [tool(READ)] || host.calls_after_cancel() != 0 {
        findings.push(finding(
            CASE,
            RULE,
            format!(
                "cancelled after one call, {} tools executed and {} calls arrived after the report",
                host.executed().len(),
                host.calls_after_cancel()
            ),
        ));
    }
    findings
}

/// Case: a `Completed` value over the result cap is `LimitExceeded(ResultBytes)`.
fn result_cap_is_enforced(factory: &dyn ToolProgramFactory) -> Vec<ContractFinding> {
    const CASE: &str = "result_cap_is_enforced";
    const RULE: &str =
        "a result over max_result_bytes is LimitExceeded(ResultBytes), never truncated";
    let long = "x".repeat(64);
    let text = format!(r#"[{{"return":"{long}"}}]"#);
    let (outcome, _, _) = match run(factory, CASE, RULE, &text, &[READ], limits(8, 4, 16)) {
        Ok(parts) => parts,
        Err(findings) => return findings,
    };
    if outcome.status == ProgramStatus::LimitExceeded(ProgramLimit::ResultBytes) {
        Vec::new()
    } else {
        vec![finding(
            CASE,
            RULE,
            format!("the status was {:?}", outcome.status),
        )]
    }
}

/// Case: a source the runtime cannot parse is `Failed(Script)`, never
/// `Completed`.
fn malformed_source_fails_typed(factory: &dyn ToolProgramFactory) -> Vec<ContractFinding> {
    const CASE: &str = "malformed_source_fails_typed";
    const RULE: &str = "a source the runtime cannot parse is Failed(Script), never Completed";
    let (outcome, host, _) = match run(
        factory,
        CASE,
        RULE,
        "not a program",
        &[READ],
        limits(8, 4, 4096),
    ) {
        Ok(parts) => parts,
        Err(findings) => return findings,
    };
    let mut findings = Vec::new();
    if !matches!(
        outcome.status,
        ProgramStatus::Failed {
            class: FailureClass::Script,
            ..
        }
    ) {
        findings.push(finding(
            CASE,
            RULE,
            format!("the status was {:?}", outcome.status),
        ));
    }
    if !host.executed().is_empty() {
        findings.push(finding(
            CASE,
            RULE,
            "a malformed program executed a tool".to_owned(),
        ));
    }
    findings
}

/// Runs the full `ToolProgramRuntime` contract suite against one factory.
pub fn tool_program_suite(factory: &dyn ToolProgramFactory) -> Vec<ContractFinding> {
    let mut findings = Vec::new();
    findings.extend(only_admitted_tools_execute(factory));
    findings.extend(call_limit_is_enforced_and_reported(factory));
    findings.extend(parallel_limit_refuses_the_group(factory));
    findings.extend(timeout_is_a_typed_status(factory));
    findings.extend(completed_is_data_only(factory));
    findings.extend(trace_is_the_host_record(factory));
    findings.extend(trace_digest_is_stable_and_argument_sensitive(factory));
    findings.extend(result_cap_is_enforced(factory));
    findings.extend(malformed_source_fails_typed(factory));
    findings.extend(cancellation_stops_further_calls(factory));
    findings
}
