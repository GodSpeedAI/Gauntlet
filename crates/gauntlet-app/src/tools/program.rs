//! The tool-program driver (H2): the one host a program's calls pass through.
//!
//! The driver never widens authority. It derives the admitted set from a
//! catalog, the host's [`EffectPolicy`], and an explicit allow list, and it
//! dispatches each admitted call through the host's [`ToolInvoker`] with the
//! request's caller. Admission, schema validation, and canonical admission still
//! happen inside that invoker.
//!
//! The host records every attempt, in order, and enforces two bounds: the
//! total call count (attempts, admitted or not) and the parallel count (calls
//! outstanding at once). A breach refuses the call and is reported as
//! `LimitExceeded`, even when the program then returns a value.

use std::cell::RefCell;
use std::fmt;

use gauntlet_domain::{Digest, InvocationId};
use gauntlet_ports::json::JsonValue;
use gauntlet_ports::tool_catalog::{ToolCatalog, ToolInvocation, ToolInvoker, ToolName};
use gauntlet_ports::tool_program::{
    HostRecord, ProgramHost, ProgramLimit, ProgramLimits, ProgramOutcome, ToolProgramRequest,
    ToolProgramRuntime, TraceEntry, TraceOutcomeKind, args_digest, finish_outcome,
};
use gauntlet_ports::tool_surface::ToolOutcome;

use super::runtime::EffectPolicy;

/// The stable refusal code for a name outside the request's admitted set.
pub const TOOL_NOT_ADMITTED: &str = "tool_not_admitted";

/// The stable refusal code for a call refused because a bound was exceeded.
pub const CALL_LIMIT_REFUSED: &str = "call_limit_refused";

/// Why an admitted set could not be derived.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdmissionError {
    /// The allow list names a tool the catalog does not offer.
    UnknownTool(String),
}

impl fmt::Display for AdmissionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownTool(name) => write!(f, "the allow list names unknown tool {name}"),
        }
    }
}

impl std::error::Error for AdmissionError {}

/// The tools a program may name: those the allow list names, that the catalog
/// offers, and whose effect class the policy admits. A tool the policy forbids
/// is omitted, so a program cannot even name it. The result is in catalog order.
pub fn admitted_subset(
    catalog: &dyn ToolCatalog,
    policy: &EffectPolicy,
    allow: &[ToolName],
) -> Result<Vec<ToolName>, AdmissionError> {
    let operations = catalog.operations();
    for name in allow {
        if !operations.iter().any(|operation| &operation.name == name) {
            return Err(AdmissionError::UnknownTool(name.as_str().to_owned()));
        }
    }
    Ok(operations
        .into_iter()
        .filter(|operation| allow.contains(&operation.name) && policy.admits(operation.effect))
        .map(|operation| operation.name)
        .collect())
}

/// Runs programs against one invoker. The caller and admitted set come from
/// each request, which the host that built it fixed.
pub struct ProgramDriver<'a> {
    invoker: &'a dyn ToolInvoker,
}

impl<'a> ProgramDriver<'a> {
    pub fn new(invoker: &'a dyn ToolInvoker) -> Self {
        Self { invoker }
    }

    /// Runs one request. The outcome is assembled from this driver's own host
    /// record, so the trace and any breach are never the runtime's claim.
    pub fn run(
        &self,
        runtime: &dyn ToolProgramRuntime,
        request: &ToolProgramRequest,
    ) -> ProgramOutcome {
        let host = DriverHost::new(self.invoker, request);
        let outcome = runtime.run(request, &host);
        finish_outcome(outcome.status, host.record(), request, outcome.duration)
    }
}

/// The host a single run sees. It is the only route from a program to a tool.
struct DriverHost<'a> {
    invoker: &'a dyn ToolInvoker,
    caller: InvocationId,
    admitted: Vec<ToolName>,
    limits: ProgramLimits,
    state: RefCell<DriverState>,
}

#[derive(Default)]
struct DriverState {
    trace: Vec<TraceEntry>,
    attempts: u32,
    in_flight: u32,
    breach: Option<ProgramLimit>,
}

impl<'a> DriverHost<'a> {
    fn new(invoker: &'a dyn ToolInvoker, request: &ToolProgramRequest) -> Self {
        Self {
            invoker,
            caller: request.caller().clone(),
            admitted: request.admitted().to_vec(),
            limits: request.limits(),
            state: RefCell::new(DriverState::default()),
        }
    }

    fn dispatch(&self, state: &mut DriverState, name: &ToolName, args: &JsonValue) -> ToolOutcome {
        let digest = args_digest(args);
        if state.breach.is_some() {
            push(&mut state.trace, name, &digest, TraceOutcomeKind::Limit);
            return refused(CALL_LIMIT_REFUSED, "a bound was already exceeded");
        }
        state.attempts = state.attempts.saturating_add(1);
        if state.attempts > self.limits.max_tool_calls {
            state.breach = Some(ProgramLimit::ToolCalls);
            push(&mut state.trace, name, &digest, TraceOutcomeKind::Limit);
            return refused(CALL_LIMIT_REFUSED, "the call-count bound is exceeded");
        }
        if !self.admitted.contains(name) {
            push(&mut state.trace, name, &digest, TraceOutcomeKind::Refused);
            return refused(TOOL_NOT_ADMITTED, "the request does not admit this tool");
        }
        let outcome = self.invoker.invoke(&ToolInvocation {
            caller: self.caller.clone(),
            name: name.clone(),
            args: args.clone(),
        });
        let kind = match &outcome {
            ToolOutcome::Ok(_) => TraceOutcomeKind::Ok,
            ToolOutcome::Refused { .. } => TraceOutcomeKind::Refused,
            ToolOutcome::Invalid { .. } => TraceOutcomeKind::Invalid,
        };
        push(&mut state.trace, name, &digest, kind);
        outcome
    }
}

fn refused(code: &str, detail: &str) -> ToolOutcome {
    ToolOutcome::Refused {
        code: code.to_owned(),
        detail: detail.to_owned(),
    }
}

fn push(trace: &mut Vec<TraceEntry>, tool: &ToolName, digest: &Digest, outcome: TraceOutcomeKind) {
    let seq = u32::try_from(trace.len()).unwrap_or(u32::MAX);
    trace.push(TraceEntry {
        seq,
        tool: tool.clone(),
        args_digest: digest.clone(),
        outcome,
    });
}

impl ProgramHost for DriverHost<'_> {
    fn call(&self, name: &ToolName, args: &JsonValue) -> ToolOutcome {
        let mut state = self.state.borrow_mut();
        self.dispatch(&mut state, name, args)
    }

    fn call_group(&self, calls: &[(ToolName, JsonValue)]) -> Vec<ToolOutcome> {
        let mut state = self.state.borrow_mut();
        let size = u32::try_from(calls.len()).unwrap_or(u32::MAX);
        if state.breach.is_none()
            && state.in_flight.saturating_add(size) > self.limits.max_parallel_calls
        {
            state.breach = Some(ProgramLimit::ParallelCalls);
        }
        if state.breach.is_some() {
            return calls
                .iter()
                .map(|(name, args)| {
                    push(
                        &mut state.trace,
                        name,
                        &args_digest(args),
                        TraceOutcomeKind::Limit,
                    );
                    refused(CALL_LIMIT_REFUSED, "a bound was exceeded for this group")
                })
                .collect();
        }
        // The host is synchronous, so the group is the whole in-flight set
        // while it runs. The bound above is what limits it.
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
