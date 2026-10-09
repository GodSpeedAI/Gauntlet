//! Deterministic tests for the tool-program driver. The recording invoker is the
//! only tool effect, so "the invoker saw X" is the proof a call reached a tool.

use std::cell::{Cell, RefCell};
use std::time::Duration;

use gauntlet_domain::{EffectClass, InvocationId};
use gauntlet_ports::contract::tool_program_reference::MockProgramRuntime;
use gauntlet_ports::json::{JsonValue, parse};
use gauntlet_ports::tool_catalog::{
    ToolCatalog, ToolInvocation, ToolInvoker, ToolName, ToolOperation,
};
use gauntlet_ports::tool_program::{
    ProgramLimit, ProgramLimits, ProgramOutcome, ProgramSource, ProgramStatus, ToolProgramRequest,
    TraceOutcomeKind,
};
use gauntlet_ports::tool_surface::ToolOutcome;

use super::program::{AdmissionError, ProgramDriver, admitted_subset};
use super::runtime::{EffectPolicy, SemanticToolRuntime};

const CALLER_TEXT: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
const OTHER_CALLER_TEXT: &str = "01BX5ZZKBKACTAV9WEVGEMMVRZ";
const EMPTY_SCHEMA: &str = r#"{"type":"object","additionalProperties":false,"properties":{}}"#;

fn caller(text: &str) -> InvocationId {
    InvocationId::new(text).expect("a valid invocation id")
}

fn name(text: &str) -> ToolName {
    ToolName::new(text).expect("a valid tool name")
}

fn operation(text: &str, effect: EffectClass) -> ToolOperation {
    ToolOperation {
        name: name(text),
        description: format!("{text} description"),
        input_schema: parse(EMPTY_SCHEMA).expect("the schema is JSON"),
        effect,
    }
}

struct FixedCatalog(Vec<ToolOperation>);

impl ToolCatalog for FixedCatalog {
    fn operations(&self) -> Vec<ToolOperation> {
        self.0.clone()
    }

    fn get(&self, target: &ToolName) -> Option<ToolOperation> {
        self.0.iter().find(|op| &op.name == target).cloned()
    }
}

fn catalog() -> FixedCatalog {
    FixedCatalog(vec![
        operation("tools.read", EffectClass::Read),
        operation("tools.write", EffectClass::WorkspaceMutation),
        operation("tools.external", EffectClass::ExternalEffect),
    ])
}

/// Records every invocation it receives and answers `Ok` to each.
#[derive(Default)]
struct RecordingInvoker {
    seen: RefCell<Vec<ToolInvocation>>,
}

impl RecordingInvoker {
    fn names(&self) -> Vec<String> {
        self.seen
            .borrow()
            .iter()
            .map(|invocation| invocation.name.as_str().to_owned())
            .collect()
    }
}

impl ToolInvoker for RecordingInvoker {
    fn invoke(&self, invocation: &ToolInvocation) -> ToolOutcome {
        self.seen.borrow_mut().push(invocation.clone());
        ToolOutcome::Ok(JsonValue::Object(vec![(
            "ok".to_owned(),
            JsonValue::Bool(true),
        )]))
    }
}

/// The byte cap every fixture uses for both source and result.
const CAP_BYTES: usize = 4096;

fn limits(max_tool_calls: u32, max_parallel_calls: u32) -> ProgramLimits {
    ProgramLimits {
        timeout: Duration::from_secs(1),
        max_tool_calls,
        max_parallel_calls,
        max_result_bytes: CAP_BYTES,
        max_source_bytes: CAP_BYTES,
    }
}

fn request(
    caller_text: &str,
    text: &str,
    admitted: &[&str],
    limits: ProgramLimits,
) -> ToolProgramRequest {
    let source = ProgramSource::new(text, limits.max_source_bytes).expect("the source fits");
    let admitted = admitted.iter().map(|text| name(text)).collect();
    ToolProgramRequest::new(caller(caller_text), source, admitted, limits)
        .expect("the request is well formed")
}

fn run(invoker: &dyn ToolInvoker, request: &ToolProgramRequest) -> ProgramOutcome {
    ProgramDriver::new(invoker).run(&MockProgramRuntime, request)
}

#[test]
fn a_program_cannot_call_a_tool_outside_its_admitted_set() {
    let invoker = RecordingInvoker::default();
    let req = request(
        CALLER_TEXT,
        r#"[{"call":"tools.read"},{"call":"tools.write"}]"#,
        &["tools.read"],
        limits(8, 4),
    );
    let outcome = run(&invoker, &req);
    assert_eq!(invoker.names(), ["tools.read"]);
    assert!(outcome.trace.iter().any(|entry| {
        entry.tool == name("tools.write") && entry.outcome == TraceOutcomeKind::Refused
    }));
}

#[test]
fn a_tool_the_policy_forbids_is_not_admitted_even_when_allowed() {
    let policy = EffectPolicy::read_only();
    let allow = [name("tools.read"), name("tools.write")];
    let admitted = admitted_subset(&catalog(), &policy, &allow).expect("the allow list is known");
    assert_eq!(admitted, [name("tools.read")]);

    let invoker = RecordingInvoker::default();
    let names: Vec<&str> = admitted.iter().map(|n| n.as_str()).collect();
    let req = request(
        CALLER_TEXT,
        r#"[{"call":"tools.write"}]"#,
        &names,
        limits(8, 4),
    );
    run(&invoker, &req);
    assert!(invoker.names().is_empty(), "the policy-forbidden tool ran");
}

#[test]
fn the_runtime_refuses_a_policy_forbidden_tool_even_if_forced_into_admission() {
    let writes = Cell::new(0_u32);
    let catalog = catalog();
    let mut runtime =
        SemanticToolRuntime::new(&catalog, EffectPolicy::read_only()).expect("the catalog builds");
    runtime
        .register(&name("tools.write"), |_: &ToolInvocation| {
            writes.set(writes.get() + 1);
            ToolOutcome::Ok(JsonValue::Null)
        })
        .expect("the handler registers");

    let req = request(
        CALLER_TEXT,
        r#"[{"call":"tools.write"}]"#,
        &["tools.write"],
        limits(8, 4),
    );
    let outcome = run(&runtime, &req);
    assert_eq!(writes.get(), 0, "the forbidden handler ran");
    let refused = outcome
        .trace
        .iter()
        .any(|entry| entry.outcome == TraceOutcomeKind::Refused);
    assert!(refused, "the runtime refusal is a trace entry");
}

#[test]
fn an_unknown_allow_list_name_is_a_typed_error() {
    let error = admitted_subset(
        &catalog(),
        &EffectPolicy::new(&[EffectClass::Read]),
        &[name("tools.nope")],
    )
    .expect_err("an unknown name is refused");
    assert_eq!(error, AdmissionError::UnknownTool("tools.nope".to_owned()));
}

#[test]
fn the_admitted_subset_follows_catalog_order_and_never_widens() {
    let policy = EffectPolicy::new(&[EffectClass::Read, EffectClass::WorkspaceMutation]);
    let allow = [name("tools.write"), name("tools.read")];
    let admitted = admitted_subset(&catalog(), &policy, &allow).expect("known names");
    assert_eq!(admitted, [name("tools.read"), name("tools.write")]);
}

#[test]
fn the_call_count_bound_refuses_the_extra_call_and_reports_it() {
    let invoker = RecordingInvoker::default();
    let req = request(
        CALLER_TEXT,
        r#"[{"call":"tools.read"},{"call":"tools.read"},{"call":"tools.read"}]"#,
        &["tools.read"],
        limits(2, 4),
    );
    let outcome = run(&invoker, &req);
    assert_eq!(invoker.names().len(), 2);
    assert_eq!(
        outcome.status,
        ProgramStatus::LimitExceeded(ProgramLimit::ToolCalls)
    );
    assert_eq!(
        outcome.trace.last().map(|entry| entry.outcome),
        Some(TraceOutcomeKind::Limit)
    );
}

#[test]
fn the_parallel_bound_refuses_an_oversize_group_whole() {
    let invoker = RecordingInvoker::default();
    let req = request(
        CALLER_TEXT,
        r#"[{"group":[{"call":"tools.read"},{"call":"tools.read"},{"call":"tools.read"}]}]"#,
        &["tools.read"],
        limits(8, 2),
    );
    let outcome = run(&invoker, &req);
    assert!(invoker.names().is_empty());
    assert_eq!(
        outcome.status,
        ProgramStatus::LimitExceeded(ProgramLimit::ParallelCalls)
    );
}

#[test]
fn a_group_at_the_parallel_bound_runs_completely() {
    let invoker = RecordingInvoker::default();
    let req = request(
        CALLER_TEXT,
        r#"[{"group":[{"call":"tools.read"},{"call":"tools.read"}]}]"#,
        &["tools.read"],
        limits(8, 2),
    );
    let outcome = run(&invoker, &req);
    assert_eq!(invoker.names().len(), 2);
    assert!(matches!(outcome.status, ProgramStatus::Completed(_)));
}

#[test]
fn trace_digest_is_identical_for_identical_runs_and_differs_for_other_args() {
    let same = r#"[{"call":"tools.read","args":{"n":1}}]"#;
    let other = r#"[{"call":"tools.read","args":{"n":2}}]"#;
    let first = run(
        &RecordingInvoker::default(),
        &request(CALLER_TEXT, same, &["tools.read"], limits(8, 4)),
    );
    let second = run(
        &RecordingInvoker::default(),
        &request(CALLER_TEXT, same, &["tools.read"], limits(8, 4)),
    );
    let third = run(
        &RecordingInvoker::default(),
        &request(CALLER_TEXT, other, &["tools.read"], limits(8, 4)),
    );
    assert_eq!(first.trace, second.trace);
    assert_eq!(first.trace_digest, second.trace_digest);
    assert_ne!(first.trace_digest, third.trace_digest);
    let seqs: Vec<u32> = first.trace.iter().map(|entry| entry.seq).collect();
    assert_eq!(seqs, [0]);
}

#[test]
fn every_invocation_carries_the_request_caller() {
    let invoker = RecordingInvoker::default();
    let req = request(
        OTHER_CALLER_TEXT,
        r#"[{"call":"tools.read"},{"group":[{"call":"tools.read"}]}]"#,
        &["tools.read"],
        limits(8, 4),
    );
    run(&invoker, &req);
    let expected = caller(OTHER_CALLER_TEXT);
    let seen = invoker.seen.borrow();
    assert_eq!(seen.len(), 2);
    assert!(seen.iter().all(|invocation| invocation.caller == expected));
}

#[test]
fn a_completed_value_reaches_no_state_beyond_the_tool_calls() {
    let invoker = RecordingInvoker::default();
    let req = request(
        CALLER_TEXT,
        r#"[{"call":"tools.read","args":{}},{"return":{"status":"done","settled":true}}]"#,
        &["tools.read"],
        limits(8, 4),
    );
    let outcome = run(&invoker, &req);
    let expected = parse(r#"{"status":"done","settled":true}"#).expect("the value is JSON");
    assert_eq!(outcome.status, ProgramStatus::Completed(expected));
    assert_eq!(invoker.names(), ["tools.read"]);
    assert_eq!(outcome.trace.len(), 1);
}

#[test]
fn a_breach_overrides_a_completed_return_value() {
    let invoker = RecordingInvoker::default();
    let req = request(
        CALLER_TEXT,
        r#"[{"call":"tools.read"},{"call":"tools.read"},{"return":"ok"}]"#,
        &["tools.read"],
        limits(1, 4),
    );
    let outcome = run(&invoker, &req);
    assert_eq!(
        outcome.status,
        ProgramStatus::LimitExceeded(ProgramLimit::ToolCalls)
    );
}

#[test]
fn the_driver_leaves_no_call_outstanding_after_a_group() {
    let invoker = RecordingInvoker::default();
    let req = request(
        CALLER_TEXT,
        r#"[{"group":[{"call":"tools.read"},{"call":"tools.read"}]},{"group":[{"call":"tools.read"},{"call":"tools.read"}]}]"#,
        &["tools.read"],
        limits(8, 2),
    );
    let outcome = run(&invoker, &req);
    assert!(matches!(outcome.status, ProgramStatus::Completed(_)));
    assert_eq!(invoker.names().len(), 4);
}
