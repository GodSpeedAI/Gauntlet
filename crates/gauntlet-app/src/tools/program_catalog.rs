//! The `program.run` operation (Phase I slice 1a): one bounded tool program,
//! exposed to a calling agent as one model-facing tool.
//!
//! [`ProgramCatalog`] offers exactly one [`ToolOperation`], with effect class
//! `ControlPlane` because it dispatches other tools. Its handler is built by
//! [`program_runtime`]:
//!
//! 1. The admitted subset is derived by [`admitted_subset`] from the inner
//!    catalog, the host's [`EffectPolicy`], and the `allow` list (default: every
//!    inner tool the policy permits). `program.run` is removed from that set
//!    explicitly, so a program can never name itself.
//! 2. One [`ProgramDriver`] runs the program. Its invoker is the SAME semantic
//!    runtime the host uses for direct calls, so every inner call passes the
//!    same argument validation, effect policy, and canonical admission, under
//!    the host-fixed caller. No argument can name a caller.
//! 3. The outcome is encoded by [`outcome_json`]. A `completed` value is data
//!    only: it never settles or completes any invocation.
//!
//! The runtime is injected as `&dyn ToolProgramRuntime`, so this module names
//! no adapter. The operation name and schema are the only vocabulary here.

use std::time::Duration;

use gauntlet_domain::{EffectClass, InvocationId};
use gauntlet_ports::json::{JsonNumber, JsonValue, parse};
use gauntlet_ports::tool_catalog::{
    ToolCatalog, ToolInvocation, ToolInvoker, ToolName, ToolOperation,
};
use gauntlet_ports::tool_program::{
    FailureClass, ProgramLimit, ProgramLimits, ProgramOutcome, ProgramSource, ProgramStatus,
    ToolProgramRequest, ToolProgramRuntime, TraceOutcomeKind,
};
use gauntlet_ports::tool_surface::ToolOutcome;

use super::program::{ProgramDriver, admitted_subset};
use super::runtime::{EffectPolicy, MAX_RESULT_BYTES, RuntimeError, SemanticToolRuntime};

/// The one operation this module offers.
pub const PROGRAM_RUN_TOOL: &str = "program.run";

/// The largest program source the operation accepts, in bytes. Fixed, not an
/// operator ceiling: the source is model output and must stay small.
pub const PROGRAM_SOURCE_LIMIT_BYTES: usize = 64 * 1024;

/// The most calls that may be outstanding at once, whatever the ceiling says.
pub const MAX_PARALLEL_CALLS_CEILING: u32 = 64;

/// The call-count ceiling when the operator sets none.
pub const DEFAULT_PROGRAM_MAX_TOOL_CALLS: u32 = 16;
/// The parallel-call ceiling when the operator sets none.
pub const DEFAULT_PROGRAM_MAX_PARALLEL_CALLS: u32 = 4;
/// The result-size ceiling when the operator sets none.
pub const DEFAULT_PROGRAM_MAX_RESULT_BYTES: usize = 64 * 1024;

const CALLER_MISMATCH: &str = "caller_mismatch";

const SCHEMA: &str = r#"{"type":"object","additionalProperties":false,"required":["source"],"properties":{"source":{"type":"string","description":"The program body: the body of an async function whose only parameter is tools. Its return value is the result."},"allow":{"type":"array","items":{"type":"string"},"description":"Tool names the program may call. Default: every agents tool the caller's policy permits. program.run cannot be named."},"timeout_ms":{"type":"integer","minimum":1,"description":"Wall-clock bound in milliseconds, clamped to the host ceiling."}}}"#;

const DESCRIPTION: &str = "Run one bounded tool program. The source is the body of an async \
function whose only parameter is tools; call a tool as await tools.<name>(args), for example \
tools.agents.list({}). Inside the program exist only the tools named by allow (default: every \
agents tool your policy permits); program.run is never one of them, and an unlisted tool is \
undefined. Each inner call passes the same validation, effect policy, and admission as a direct \
call, under your identity; no argument can name a caller or parent. The result is a status, the \
program's value (or a message), the ordered trace of inner calls, and digests. The value is DATA \
ONLY: a completed program never settles, completes, or changes the status of any invocation, \
including yours. Only a schema-valid result contract does that.";

/// Why operator ceilings were refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CeilingError {
    /// A bound is outside its allowed range.
    OutOfRange {
        field: &'static str,
        min: u64,
        max: u64,
    },
}

impl std::fmt::Display for CeilingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OutOfRange { field, min, max } => {
                write!(f, "{field} must be between {min} and {max}")
            }
        }
    }
}

impl std::error::Error for CeilingError {}

/// The operator ceilings of `program.run`. A request may ask for less than
/// these bounds, never more: the timeout is clamped to the ceiling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProgramCeilings {
    timeout: Duration,
    max_tool_calls: u32,
    max_parallel_calls: u32,
    max_result_bytes: usize,
}

impl ProgramCeilings {
    /// Checks every bound. The timeout must be at least one millisecond, the
    /// call bounds at least one, and the result cap no larger than the
    /// runtime's own result cap.
    pub fn new(
        timeout: Duration,
        max_tool_calls: u32,
        max_parallel_calls: u32,
        max_result_bytes: usize,
    ) -> Result<Self, CeilingError> {
        let millis = u64::try_from(timeout.as_millis()).unwrap_or(u64::MAX);
        if millis == 0 {
            return Err(CeilingError::OutOfRange {
                field: "timeout",
                min: 1,
                max: u64::MAX,
            });
        }
        if max_tool_calls == 0 {
            return Err(CeilingError::OutOfRange {
                field: "max_tool_calls",
                min: 1,
                max: u64::from(u32::MAX),
            });
        }
        if !(1..=MAX_PARALLEL_CALLS_CEILING).contains(&max_parallel_calls) {
            return Err(CeilingError::OutOfRange {
                field: "max_parallel_calls",
                min: 1,
                max: u64::from(MAX_PARALLEL_CALLS_CEILING),
            });
        }
        if !(1..=MAX_RESULT_BYTES).contains(&max_result_bytes) {
            return Err(CeilingError::OutOfRange {
                field: "max_result_bytes",
                min: 1,
                max: u64::try_from(MAX_RESULT_BYTES).unwrap_or(u64::MAX),
            });
        }
        Ok(Self {
            timeout,
            max_tool_calls,
            max_parallel_calls,
            max_result_bytes,
        })
    }

    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    pub fn max_tool_calls(&self) -> u32 {
        self.max_tool_calls
    }

    pub fn max_parallel_calls(&self) -> u32 {
        self.max_parallel_calls
    }

    pub fn max_result_bytes(&self) -> usize {
        self.max_result_bytes
    }
}

/// The catalog of one operation: `program.run`, effect `ControlPlane`.
pub struct ProgramCatalog;

impl ToolCatalog for ProgramCatalog {
    fn operations(&self) -> Vec<ToolOperation> {
        let name = ToolName::new(PROGRAM_RUN_TOOL).expect("program.run is a valid tool name");
        vec![ToolOperation {
            name,
            description: DESCRIPTION.to_owned(),
            input_schema: parse(SCHEMA).expect("the program.run schema is JSON"),
            effect: EffectClass::ControlPlane,
        }]
    }

    fn get(&self, name: &ToolName) -> Option<ToolOperation> {
        self.operations()
            .into_iter()
            .find(|operation| &operation.name == name)
    }
}

/// The `program.run` runtime for one caller. The outer policy must admit the
/// operation's own `ControlPlane` class; `inner_policy` decides which inner
/// tools a program may be given. Both come from the host.
///
/// `invoker` is the host's semantic runtime for direct calls, so an inner call
/// is admitted exactly as a direct call would be.
pub fn program_runtime<'s>(
    inner: &'s dyn ToolCatalog,
    invoker: &'s dyn ToolInvoker,
    program: &'s dyn ToolProgramRuntime,
    policy: EffectPolicy,
    ceilings: ProgramCeilings,
    caller: InvocationId,
) -> Result<SemanticToolRuntime<'s>, RuntimeError> {
    let mut runtime = SemanticToolRuntime::new(&ProgramCatalog, policy.clone())?;
    let name = ToolName::new(PROGRAM_RUN_TOOL).expect("program.run is a valid tool name");
    let bound = caller;
    runtime.register(&name, move |invocation: &ToolInvocation| {
        if invocation.caller != bound {
            return ToolOutcome::Refused {
                code: CALLER_MISMATCH.to_owned(),
                detail: "the invocation names another caller".to_owned(),
            };
        }
        run(
            inner,
            invoker,
            program,
            &policy,
            ceilings,
            &bound,
            &invocation.args,
        )
    })?;
    Ok(runtime)
}

/// Decodes the (already schema-checked) arguments, builds the request, runs
/// it, and encodes the outcome. A malformed argument is `Invalid`; nothing runs.
fn run(
    inner: &dyn ToolCatalog,
    invoker: &dyn ToolInvoker,
    program: &dyn ToolProgramRuntime,
    policy: &EffectPolicy,
    ceilings: ProgramCeilings,
    caller: &InvocationId,
    args: &JsonValue,
) -> ToolOutcome {
    let request = match build_request(inner, policy, ceilings, caller, args) {
        Ok(request) => request,
        Err(detail) => return ToolOutcome::Invalid { detail },
    };
    let outcome = ProgramDriver::new(invoker).run(program, &request);
    ToolOutcome::Ok(outcome_json(&outcome))
}

fn build_request(
    inner: &dyn ToolCatalog,
    policy: &EffectPolicy,
    ceilings: ProgramCeilings,
    caller: &InvocationId,
    args: &JsonValue,
) -> Result<ToolProgramRequest, String> {
    let source_text = match args.get("source") {
        Some(JsonValue::Str(text)) => text.as_str(),
        _ => return Err("arguments.source: expected a string".to_owned()),
    };
    let source = ProgramSource::new(source_text, PROGRAM_SOURCE_LIMIT_BYTES)
        .map_err(|error| format!("arguments.source: {error}"))?;

    let allow = match args.get("allow") {
        None => inner
            .operations()
            .into_iter()
            .map(|operation| operation.name)
            .collect(),
        Some(JsonValue::Array(items)) => {
            let mut names = Vec::with_capacity(items.len());
            for (index, item) in items.iter().enumerate() {
                let JsonValue::Str(text) = item else {
                    return Err(format!("arguments.allow[{index}]: expected a string"));
                };
                let name = ToolName::new(text)
                    .map_err(|error| format!("arguments.allow[{index}]: {error}"))?;
                names.push(name);
            }
            names
        }
        Some(_) => return Err("arguments.allow: expected an array".to_owned()),
    };
    let mut admitted = admitted_subset(inner, policy, &allow)
        .map_err(|error| format!("arguments.allow: {error}"))?;
    // The operation never admits itself, whatever the inner catalog holds.
    admitted.retain(|name| name.as_str() != PROGRAM_RUN_TOOL);

    let requested = match args.get("timeout_ms") {
        None => ceilings.timeout(),
        Some(JsonValue::Number(number)) => {
            let millis = number
                .as_u64()
                .ok_or_else(|| "arguments.timeout_ms: expected a positive integer".to_owned())?;
            Duration::from_millis(millis)
        }
        Some(_) => return Err("arguments.timeout_ms: expected an integer".to_owned()),
    };
    let limits = ProgramLimits {
        timeout: requested.min(ceilings.timeout()),
        max_tool_calls: ceilings.max_tool_calls(),
        max_parallel_calls: ceilings.max_parallel_calls(),
        max_result_bytes: ceilings.max_result_bytes(),
        max_source_bytes: PROGRAM_SOURCE_LIMIT_BYTES,
    };
    ToolProgramRequest::new(caller.clone(), source, admitted, limits)
        .map_err(|error| format!("arguments: {error}"))
}

/// The wire encoding of one run. The trace and digests always come from the
/// driver's record, never from the runtime's claim. Durations are omitted: the
/// result is a function of the program, its inputs, and its trace.
pub fn outcome_json(outcome: &ProgramOutcome) -> JsonValue {
    let mut fields: Vec<(&str, JsonValue)> = Vec::new();
    match &outcome.status {
        ProgramStatus::Completed(value) => {
            fields.push(("status", text("completed")));
            fields.push(("value", value.clone()));
        }
        ProgramStatus::Failed { class, message } => {
            fields.push(("status", text("failed")));
            fields.push(("class", text(failure_token(*class))));
            fields.push(("message", text(message)));
        }
        ProgramStatus::TimedOut => {
            fields.push(("status", text("timed_out")));
            fields.push(("message", text("the program exceeded its time bound")));
        }
        ProgramStatus::LimitExceeded(limit) => {
            fields.push(("status", text("limit_exceeded")));
            fields.push(("limit", text(limit_token(*limit))));
        }
        ProgramStatus::Cancelled => {
            fields.push(("status", text("cancelled")));
            fields.push(("message", text("the program was cancelled")));
        }
    }
    let trace = outcome
        .trace
        .iter()
        .map(|entry| {
            JsonValue::Object(vec![
                ("seq".to_owned(), number(u64::from(entry.seq))),
                ("tool".to_owned(), text(entry.tool.as_str())),
                ("outcome".to_owned(), text(trace_token(entry.outcome))),
            ])
        })
        .collect();
    fields.push(("trace", JsonValue::Array(trace)));
    fields.push(("trace_digest", text(&outcome.trace_digest.to_string())));
    fields.push(("program_digest", text(&outcome.program_digest.to_string())));
    JsonValue::Object(
        fields
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value))
            .collect(),
    )
}

fn failure_token(class: FailureClass) -> &'static str {
    match class {
        FailureClass::Script => "script",
        FailureClass::Runtime => "runtime",
    }
}

fn limit_token(limit: ProgramLimit) -> &'static str {
    match limit {
        ProgramLimit::ToolCalls => "tool_calls",
        ProgramLimit::ParallelCalls => "parallel_calls",
        ProgramLimit::ResultBytes => "result_bytes",
        ProgramLimit::SourceBytes => "source_bytes",
        ProgramLimit::Memory => "heap",
    }
}

fn trace_token(kind: TraceOutcomeKind) -> &'static str {
    match kind {
        TraceOutcomeKind::Ok => "ok",
        TraceOutcomeKind::Refused => "refused",
        TraceOutcomeKind::Invalid => "invalid",
        TraceOutcomeKind::Limit => "limit",
    }
}

fn text(value: &str) -> JsonValue {
    JsonValue::Str(value.to_owned())
}

fn number(value: u64) -> JsonValue {
    JsonValue::Number(JsonNumber::Uint(value))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(value: u64) -> Duration {
        Duration::from_millis(value)
    }

    #[test]
    fn the_schema_is_in_the_runtime_subset_and_refuses_unknown_keys() {
        let operations = ProgramCatalog.operations();
        assert_eq!(operations.len(), 1);
        let operation = &operations[0];
        assert_eq!(operation.name.as_str(), PROGRAM_RUN_TOOL);
        assert_eq!(operation.effect, EffectClass::ControlPlane);
        assert!(super::super::schema::check_subset(&operation.input_schema).is_ok());
        let caller_smuggled =
            parse(r#"{"source":"return 1;","caller":"x"}"#).expect("a valid document");
        assert!(super::super::schema::validate(&operation.input_schema, &caller_smuggled).is_err());
        let timeout_zero =
            parse(r#"{"source":"return 1;","timeout_ms":0}"#).expect("a valid document");
        assert!(super::super::schema::validate(&operation.input_schema, &timeout_zero).is_err());
    }

    #[test]
    fn ceilings_refuse_out_of_range_bounds() {
        let ok = ProgramCeilings::new(ms(5), 1, 1, 1);
        assert!(ok.is_ok());
        assert!(ProgramCeilings::new(Duration::ZERO, 1, 1, 1).is_err());
        assert!(ProgramCeilings::new(ms(5), 0, 1, 1).is_err());
        assert!(ProgramCeilings::new(ms(5), 1, 0, 1).is_err());
        assert!(ProgramCeilings::new(ms(5), 1, MAX_PARALLEL_CALLS_CEILING + 1, 1).is_err());
        assert!(ProgramCeilings::new(ms(5), 1, 1, MAX_RESULT_BYTES + 1).is_err());
    }
}
