//! The tool-program port (H1): one bounded, synchronous program that may call
//! only the tools it was admitted to, and only through one host.
//!
//! - [`ProgramSource`] is the program text, size-checked and digested.
//! - [`ProgramLimits`] bounds one run. A runtime enforces the timeout and the
//!   source and result sizes; the host enforces the tool-call counts.
//! - [`ToolProgramRuntime`] runs one [`ToolProgramRequest`]. Its only route to
//!   the world is the [`ProgramHost`], and the host records the trace, so a
//!   runtime cannot forge one.
//! - [`finish_outcome`] is the single place a [`ProgramOutcome`] is assembled.
//!   It applies the host's recorded breach and the result cap, and digests the
//!   trace. It is idempotent, so a caller may apply it again to a runtime's
//!   outcome.
//!
//! A `Completed` value is data. It never marks an invocation complete or
//! settled; only a schema-valid result contract does (spec §10.3).
//!
//! This module names capabilities only. It carries no vendor or engine
//! identifiers.

use std::fmt::Write as _;
use std::time::Duration;

use gauntlet_domain::{Digest, InvocationId};
use sha2::{Digest as _, Sha256};

use crate::json::JsonValue;
use crate::tool_catalog::ToolName;
use crate::tool_surface::ToolOutcome;

/// Domain-separates the trace digest from every other SHA-256 in the system.
const TRACE_DIGEST_PREFIX: &str = "gauntlet.tool_program.trace.v1\n";

/// The program text and its digest. Constructed only through [`Self::new`],
/// so a source is never larger than the limit it was checked against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramSource {
    text: String,
    digest: Digest,
}

/// Why a program source was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProgramSourceError {
    TooLarge { len: usize, limit: usize },
}

impl std::fmt::Display for ProgramSourceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooLarge { len, limit } => {
                write!(f, "the program is {len} bytes; the limit is {limit}")
            }
        }
    }
}

impl std::error::Error for ProgramSourceError {}

impl ProgramSource {
    /// Accepts program text no larger than `max_bytes`, and digests it.
    pub fn new(text: &str, max_bytes: usize) -> Result<Self, ProgramSourceError> {
        if text.len() > max_bytes {
            return Err(ProgramSourceError::TooLarge {
                len: text.len(),
                limit: max_bytes,
            });
        }
        Ok(Self {
            text: text.to_owned(),
            digest: sha256_digest(text.as_bytes()),
        })
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn digest(&self) -> &Digest {
        &self.digest
    }
}

/// The bounds of one run. Copied into every request; no run is unbounded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProgramLimits {
    /// Wall-clock bound, enforced by the runtime.
    pub timeout: Duration,
    /// Tool-call attempts, admitted or not, that the host will dispatch or
    /// record. The attempt that exceeds the bound is refused.
    pub max_tool_calls: u32,
    /// Calls that may be outstanding at once, enforced by the host.
    pub max_parallel_calls: u32,
    /// Largest serialized `Completed` value, in bytes.
    pub max_result_bytes: usize,
    /// Largest program source, in bytes.
    pub max_source_bytes: usize,
}

/// Which bound a run exceeded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgramLimit {
    ToolCalls,
    ParallelCalls,
    ResultBytes,
    SourceBytes,
}

/// Why a request was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProgramRequestError {
    Source(ProgramSourceError),
    DuplicateAdmitted(ToolName),
}

impl std::fmt::Display for ProgramRequestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Source(error) => write!(f, "{error}"),
            Self::DuplicateAdmitted(name) => {
                write!(f, "tool {} is admitted twice", name.as_str())
            }
        }
    }
}

impl std::error::Error for ProgramRequestError {}

/// One run to perform. `admitted` is the only set of tools the program may
/// name; the admitting authority derives it (see the app layer), so a program
/// cannot grant itself more.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolProgramRequest {
    caller: InvocationId,
    source: ProgramSource,
    admitted: Vec<ToolName>,
    limits: ProgramLimits,
}

impl ToolProgramRequest {
    /// Checks the source against `limits` and refuses duplicate admissions.
    pub fn new(
        caller: InvocationId,
        source: ProgramSource,
        admitted: Vec<ToolName>,
        limits: ProgramLimits,
    ) -> Result<Self, ProgramRequestError> {
        if source.text().len() > limits.max_source_bytes {
            return Err(ProgramRequestError::Source(ProgramSourceError::TooLarge {
                len: source.text().len(),
                limit: limits.max_source_bytes,
            }));
        }
        for (index, name) in admitted.iter().enumerate() {
            if admitted[..index].contains(name) {
                return Err(ProgramRequestError::DuplicateAdmitted(name.clone()));
            }
        }
        Ok(Self {
            caller,
            source,
            admitted,
            limits,
        })
    }

    pub fn caller(&self) -> &InvocationId {
        &self.caller
    }

    pub fn source(&self) -> &ProgramSource {
        &self.source
    }

    pub fn admitted(&self) -> &[ToolName] {
        &self.admitted
    }

    pub fn limits(&self) -> ProgramLimits {
        self.limits
    }
}

/// Why a program failed on its own terms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureClass {
    /// The source is not a program the runtime accepts.
    Script,
    /// The program started and then raised an error.
    Runtime,
}

/// How a run ended.
#[derive(Debug, Clone, PartialEq)]
pub enum ProgramStatus {
    /// The program returned this value. Data only.
    Completed(JsonValue),
    Failed {
        class: FailureClass,
        message: String,
    },
    TimedOut,
    LimitExceeded(ProgramLimit),
    Cancelled,
}

/// How one host-dispatched call ended, as the trace records it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraceOutcomeKind {
    Ok,
    /// A governed refusal: the name is not admitted, or the invoker refused.
    Refused,
    /// The arguments were malformed.
    Invalid,
    /// The call was refused because a bound was exceeded.
    Limit,
}

/// One call attempt, in the order the host saw it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceEntry {
    pub seq: u32,
    pub tool: ToolName,
    pub args_digest: Digest,
    pub outcome: TraceOutcomeKind,
}

/// What a host recorded during one run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostRecord {
    pub trace: Vec<TraceEntry>,
    /// The first bound the host refused on, if any.
    pub breach: Option<ProgramLimit>,
}

/// The only way a program touches the world. Implemented by the app layer,
/// which owns admission, the call bounds, and the trace.
pub trait ProgramHost {
    /// One call. The host decides admission and the caller.
    fn call(&self, name: &ToolName, args: &JsonValue) -> ToolOutcome;

    /// Calls issued together. The host refuses the whole group when it would
    /// exceed the parallel bound. Returns one outcome per call, in order.
    fn call_group(&self, calls: &[(ToolName, JsonValue)]) -> Vec<ToolOutcome>;

    /// What the host has recorded so far.
    fn record(&self) -> HostRecord;
}

/// The result of one run.
#[derive(Debug, Clone, PartialEq)]
pub struct ProgramOutcome {
    pub status: ProgramStatus,
    pub trace: Vec<TraceEntry>,
    pub trace_digest: Digest,
    pub program_digest: Digest,
    pub duration: Duration,
}

/// Runs one program. Synchronous from the port's view. Implementations route
/// every tool call through `host` and assemble their outcome with
/// [`finish_outcome`].
pub trait ToolProgramRuntime {
    fn run(&self, request: &ToolProgramRequest, host: &dyn ProgramHost) -> ProgramOutcome;
}

/// Assembles a [`ProgramOutcome`] from a runtime's status and the host's own
/// record. The host's breach overrides any status, because a refused call the
/// program swallowed still counts. A `Completed` value over the result cap
/// becomes `LimitExceeded(ResultBytes)`. The trace always comes from the host.
pub fn finish_outcome(
    status: ProgramStatus,
    record: HostRecord,
    request: &ToolProgramRequest,
    duration: Duration,
) -> ProgramOutcome {
    let status = match (record.breach, status) {
        (Some(limit), _) => ProgramStatus::LimitExceeded(limit),
        (None, ProgramStatus::Completed(value))
            if value.to_json_text().len() > request.limits().max_result_bytes =>
        {
            ProgramStatus::LimitExceeded(ProgramLimit::ResultBytes)
        }
        (None, other) => other,
    };
    ProgramOutcome {
        status,
        trace_digest: trace_digest(&record.trace),
        trace: record.trace,
        program_digest: request.source().digest().clone(),
        duration,
    }
}

/// The digest of the arguments as the host received them. Key order is part
/// of the text, so the same text always digests the same.
pub fn args_digest(args: &JsonValue) -> Digest {
    sha256_digest(args.to_json_text().as_bytes())
}

/// The digest of a trace: one line per entry, in order.
pub fn trace_digest(trace: &[TraceEntry]) -> Digest {
    let mut text = String::from(TRACE_DIGEST_PREFIX);
    for entry in trace {
        // Writing to a String cannot fail.
        let _ = writeln!(
            text,
            "{} {} {} {:?}",
            entry.seq,
            entry.tool.as_str(),
            entry.args_digest,
            entry.outcome
        );
    }
    sha256_digest(text.as_bytes())
}

fn sha256_digest(bytes: &[u8]) -> Digest {
    let hex: String = Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    // SHA-256 hex satisfies the Digest grammar by construction.
    Digest::new("sha256", &hex).expect("SHA-256 hex satisfies the Digest grammar by construction")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn caller() -> InvocationId {
        InvocationId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("a valid invocation id")
    }

    /// Fixture bounds, named so each literal stands alone.
    const BOUND: Duration = Duration::from_secs(1);
    const CALLS: u32 = 4;
    const PARALLEL: u32 = 1;
    const RESULT_CAP: usize = 1024;

    fn limits(max_source_bytes: usize) -> ProgramLimits {
        ProgramLimits {
            timeout: BOUND,
            max_tool_calls: CALLS,
            max_parallel_calls: PARALLEL,
            max_result_bytes: RESULT_CAP,
            max_source_bytes,
        }
    }

    #[test]
    fn a_source_over_its_limit_is_refused_and_a_request_cannot_outgrow_its_limit() {
        let refused = ProgramSource::new("[null]", 1);
        assert!(matches!(refused, Err(ProgramSourceError::TooLarge { .. })));

        let source = ProgramSource::new("[]", 1024).expect("the source fits");
        let request = ToolProgramRequest::new(caller(), source, Vec::new(), limits(1));
        assert!(matches!(
            request,
            Err(ProgramRequestError::Source(
                ProgramSourceError::TooLarge { .. }
            ))
        ));
    }

    #[test]
    fn a_duplicate_admission_is_refused() {
        let name = ToolName::new("tools.read").expect("a valid tool name");
        let source = ProgramSource::new("[]", 1024).expect("the source fits");
        let request = ToolProgramRequest::new(
            caller(),
            source,
            vec![name.clone(), name.clone()],
            limits(1024),
        );
        assert_eq!(request, Err(ProgramRequestError::DuplicateAdmitted(name)));
    }
}
