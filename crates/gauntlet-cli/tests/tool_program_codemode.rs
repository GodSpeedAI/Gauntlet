//! The codemode runtime through the application's `ProgramDriver` and
//! `SemanticToolRuntime`. The recording handler is the only tool effect, so
//! "the handler saw X" is the proof that a nested call reached a tool, and the
//! caller it recorded is the caller the request names.
//!
//! The adapter cannot depend on the app, so this composition lives here. The
//! test runs the repository's real workbench host through `bun`, and it FAILS
//! (it never skips) when bun or the workbench entry is missing.

use std::cell::RefCell;
use std::path::{Path, PathBuf};

use gauntlet_adapter_tool_program_codemode::{
    CodemodeRuntime, DEFAULT_PROGRAM_TIMEOUT, DEFAULT_WORKBENCH_DIR, HOST_ENTRY, HostCommand,
    bun_codemode_host, probe_codemode_in,
};
use gauntlet_app::tools::{EffectPolicy, ProgramDriver, SemanticToolRuntime, admitted_subset};
use gauntlet_domain::{EffectClass, InvocationId};
use gauntlet_ports::json::parse;
use gauntlet_ports::tool_catalog::{ToolCatalog, ToolInvocation, ToolName, ToolOperation};
use gauntlet_ports::tool_program::{
    NeverCancelled, ProgramLimit, ProgramLimits, ProgramSource, ProgramStatus, ToolProgramRequest,
    TraceOutcomeKind,
};
use gauntlet_ports::tool_surface::ToolOutcome;

/// The result and source byte cap for every program in this file.
const BYTE_CAP: usize = 64 * 1024;
const CALLER: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
const ID_SCHEMA: &str =
    r#"{"type":"object","additionalProperties":false,"properties":{"id":{"type":"string"}}}"#;

struct Catalog(Vec<ToolOperation>);

impl ToolCatalog for Catalog {
    fn operations(&self) -> Vec<ToolOperation> {
        self.0.clone()
    }

    fn get(&self, target: &ToolName) -> Option<ToolOperation> {
        self.0.iter().find(|op| &op.name == target).cloned()
    }
}

fn operation(name: &str, effect: EffectClass) -> ToolOperation {
    ToolOperation {
        name: ToolName::new(name).expect("a valid tool name"),
        description: format!("{name} description"),
        input_schema: parse(ID_SCHEMA).expect("the schema is JSON"),
        effect,
    }
}

fn catalog() -> Catalog {
    Catalog(vec![
        operation("tools.read", EffectClass::Read),
        operation("tools.write", EffectClass::WorkspaceMutation),
    ])
}

fn name(text: &str) -> ToolName {
    ToolName::new(text).expect("a valid tool name")
}

/// The workbench, located by walking up from this crate's manifest to the
/// first directory that holds it. Fails loudly when the host entry is absent.
fn workbench_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .map(|dir| dir.join(DEFAULT_WORKBENCH_DIR))
        .find(|dir| dir.join(HOST_ENTRY).is_file())
        .unwrap_or_else(|| {
            panic!(
                "the codemode host entry ({HOST_ENTRY} under {DEFAULT_WORKBENCH_DIR}) is missing; \
                 this test runs the repository's workbench host and fails rather than skips"
            )
        })
}

/// The real host, provided only when the probe measures it available: bun on
/// PATH, the host entry present, and a one-shot program that completes.
fn bun_host() -> HostCommand {
    let workbench = workbench_dir();
    let probe = probe_codemode_in(&workbench);
    assert!(
        probe.available(),
        "the codemode host must be available for this composition test: {}",
        probe.summary()
    );
    bun_codemode_host(&workbench)
}

fn limits(max_tool_calls: u32, max_parallel_calls: u32) -> ProgramLimits {
    ProgramLimits {
        timeout: DEFAULT_PROGRAM_TIMEOUT,
        max_tool_calls,
        max_parallel_calls,
        max_result_bytes: BYTE_CAP,
        max_source_bytes: BYTE_CAP,
    }
}

/// Admits `tools.read` only, derived through the app's admission path.
fn admitted_read(catalog: &Catalog) -> Vec<ToolName> {
    admitted_subset(catalog, &EffectPolicy::read_only(), &[name("tools.read")])
        .expect("the allow list names a catalog tool")
}

#[test]
fn nested_calls_reach_the_handler_with_the_callers_identity() {
    let catalog = catalog();
    let seen: RefCell<Vec<(InvocationId, String)>> = RefCell::new(Vec::new());
    let mut runtime =
        SemanticToolRuntime::new(&catalog, EffectPolicy::read_only()).expect("the catalog builds");
    runtime
        .register(&name("tools.read"), |invocation: &ToolInvocation| {
            seen.borrow_mut().push((
                invocation.caller.clone(),
                invocation.name.as_str().to_owned(),
            ));
            ToolOutcome::Ok(invocation.args.clone())
        })
        .expect("the handler registers");

    let source = ProgramSource::new(
        r#"const a = await tools.tools.read({ id: "a1" });
const pair = await Promise.all([tools.tools.read({ id: "b" }), tools.tools.read({ id: "c" })]);
return { a: a, pair: pair };"#,
        BYTE_CAP,
    )
    .expect("the source fits");
    let caller = InvocationId::new(CALLER).expect("a valid caller");
    let request = ToolProgramRequest::new(
        caller.clone(),
        source,
        admitted_read(&catalog),
        limits(8, 2),
    )
    .expect("the request is valid");

    let driver = ProgramDriver::new(&runtime, &NeverCancelled);
    let outcome = driver.run(&CodemodeRuntime::new(bun_host()), &request);

    assert_eq!(
        outcome.status,
        ProgramStatus::Completed(
            parse(r#"{"a":{"id":"a1"},"pair":[{"id":"b"},{"id":"c"}]}"#).expect("json")
        ),
        "the program's nested results must come back as its value"
    );
    let calls = seen.borrow();
    assert_eq!(calls.len(), 3, "three nested calls reach the handler");
    assert!(
        calls
            .iter()
            .all(|(seen_caller, tool)| seen_caller == &caller && tool == "tools.read"),
        "every nested call carries the request's caller: {calls:?}"
    );
    assert!(
        outcome
            .trace
            .iter()
            .all(|entry| entry.outcome == TraceOutcomeKind::Ok),
        "the host recorded three successful calls: {:?}",
        outcome.trace
    );
}

#[test]
fn a_tool_outside_the_admitted_set_never_reaches_its_handler() {
    let catalog = catalog();
    let writes = RefCell::new(0usize);
    let mut runtime =
        SemanticToolRuntime::new(&catalog, EffectPolicy::read_only()).expect("the catalog builds");
    runtime
        .register(&name("tools.read"), |invocation: &ToolInvocation| {
            ToolOutcome::Ok(invocation.args.clone())
        })
        .expect("the read handler registers");
    runtime
        .register(&name("tools.write"), |_: &ToolInvocation| {
            *writes.borrow_mut() += 1;
            ToolOutcome::Ok(gauntlet_ports::json::JsonValue::Null)
        })
        .expect("the write handler registers");

    let source = ProgramSource::new(
        r#"try { await tools.tools.write({ id: "x" }); return "reached"; } catch (error) { return "refused"; }"#,
        BYTE_CAP,
    )
    .expect("the source fits");
    let request = ToolProgramRequest::new(
        InvocationId::new(CALLER).expect("a valid caller"),
        source,
        admitted_read(&catalog),
        limits(8, 2),
    )
    .expect("the request is valid");

    let driver = ProgramDriver::new(&runtime, &NeverCancelled);
    let outcome = driver.run(&CodemodeRuntime::new(bun_host()), &request);

    assert_eq!(
        outcome.status,
        ProgramStatus::Completed(parse(r#""refused""#).expect("json"))
    );
    assert_eq!(
        *writes.borrow(),
        0,
        "the non-admitted handler must never run"
    );
}

#[test]
fn the_driver_call_bound_applies_to_codemode_nested_calls() {
    let catalog = catalog();
    let mut runtime =
        SemanticToolRuntime::new(&catalog, EffectPolicy::read_only()).expect("the catalog builds");
    runtime
        .register(&name("tools.read"), |invocation: &ToolInvocation| {
            ToolOutcome::Ok(invocation.args.clone())
        })
        .expect("the handler registers");

    let source = ProgramSource::new(
        r#"await tools.tools.read({ id: "1" });
await tools.tools.read({ id: "2" });
await tools.tools.read({ id: "3" });
return "done";"#,
        BYTE_CAP,
    )
    .expect("the source fits");
    let request = ToolProgramRequest::new(
        InvocationId::new(CALLER).expect("a valid caller"),
        source,
        admitted_read(&catalog),
        limits(2, 2),
    )
    .expect("the request is valid");

    let driver = ProgramDriver::new(&runtime, &NeverCancelled);
    let outcome = driver.run(&CodemodeRuntime::new(bun_host()), &request);
    assert_eq!(
        outcome.status,
        ProgramStatus::LimitExceeded(ProgramLimit::ToolCalls)
    );
    assert_eq!(
        outcome.trace.len(),
        2,
        "only the bounded calls are recorded"
    );
}
