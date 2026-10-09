//! The codemode runtime: one `ToolProgramRuntime` run is one host process.
//!
//! The parent answers each `call` as it arrives. The host bounds parallel calls
//! by the sandbox's own count of unsettled calls at each call, which does not
//! depend on when those answers arrive. The host refuses an over-bound call
//! inside the sandbox and ends the run with `limit_exceeded: parallel`; the
//! parent cannot see group boundaries, so calls the sandbox sent before that
//! refusal were dispatched. The trace always comes from the host's record.

use std::collections::HashSet;
use std::time::{Duration, Instant};

use gauntlet_ports::tool_program::{
    FailureClass, ProgramHost, ProgramOutcome, ProgramStatus, ToolProgramRequest,
    ToolProgramRuntime, finish_outcome,
};

use crate::process::{HostCommand, Next, Running, STDERR_SETTLE};
use crate::protocol::{self, HostMessage};

/// How long the parent waits past the program's own timeout before it stops
/// the host. The host enforces the program timeout itself; this bound is for
/// a host that has stopped responding.
pub const DEFAULT_GRACE: Duration = Duration::from_secs(1);

/// The wall-clock bound the adapter's own one-shot programs run under. A
/// caller that builds a request picks its own bound; this one is the default.
pub const DEFAULT_PROGRAM_TIMEOUT: Duration = Duration::from_secs(10);

/// The smallest host line the parent accepts. A larger line is a fault.
const MIN_LINE_CAP: usize = 8 * 1024 * 1024;

/// Headroom above the result cap, for the envelope around a result value.
const LINE_HEADROOM: usize = 64 * 1024;

/// How long a cancel gets to reach the host before the group is killed.
const CANCEL_ACK: Duration = Duration::from_millis(200);

/// The codemode runtime. It holds configuration only; no process starts until
/// [`ToolProgramRuntime::run`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodemodeRuntime {
    host: HostCommand,
    grace: Duration,
}

impl CodemodeRuntime {
    /// Configures a runtime over `host`. Spawns nothing.
    pub fn new(host: HostCommand) -> Self {
        Self {
            host,
            grace: DEFAULT_GRACE,
        }
    }

    /// The same runtime with a different parent-side grace.
    pub fn with_grace(mut self, grace: Duration) -> Self {
        self.grace = grace;
        self
    }
}

/// How the run's conversation with the host ended.
enum Ending {
    Done(ProgramStatus),
    Fault(String),
    Deadline,
}

impl ToolProgramRuntime for CodemodeRuntime {
    fn run(&self, request: &ToolProgramRequest, host: &dyn ProgramHost) -> ProgramOutcome {
        let started = Instant::now();
        let status = self.execute(request, host, started);
        finish_outcome(status, host.record(), request, started.elapsed())
    }
}

impl CodemodeRuntime {
    fn execute(
        &self,
        request: &ToolProgramRequest,
        host: &dyn ProgramHost,
        started: Instant,
    ) -> ProgramStatus {
        let limits = request.limits();
        if limits.max_parallel_calls == 0 {
            return runtime_failure("max_parallel_calls must be at least 1".to_owned());
        }
        let timeout_ms = u64::try_from(limits.timeout.as_millis())
            .unwrap_or(u64::MAX)
            .max(1);
        let line_cap = line_cap(limits.max_result_bytes);
        let mut running = match Running::spawn(&self.host, line_cap) {
            Ok(running) => running,
            Err(detail) => {
                return runtime_failure(format!("the codemode host could not start: {detail}"));
            }
        };
        running.send(protocol::encode_run(request, timeout_ms));
        let deadline = started
            .checked_add(limits.timeout)
            .and_then(|at| at.checked_add(self.grace))
            .unwrap_or(started);
        let mut seen_ids = HashSet::new();
        let ending = loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            match running.next(remaining) {
                Next::Frame(crate::process::Frame::Line(text)) => match protocol::decode(&text) {
                    Ok(HostMessage::Call { id, tool, args }) => {
                        if !seen_ids.insert(id) {
                            break Ending::Fault(format!("call id {id} repeats"));
                        }
                        if !request.admitted().contains(&tool) {
                            break Ending::Fault(format!(
                                "the host called {}, which is not admitted",
                                tool.as_str()
                            ));
                        }
                        let outcome = host.call(&tool, &args);
                        running.send(protocol::encode_result(id, &outcome));
                    }
                    Ok(HostMessage::Done(status)) => break Ending::Done(status),
                    Err(detail) => break Ending::Fault(detail),
                },
                Next::Frame(crate::process::Frame::TooLong) => {
                    break Ending::Fault(format!("a host line exceeds the {line_cap}-byte cap"));
                }
                Next::Frame(crate::process::Frame::End) => {
                    break Ending::Fault("the host closed its output before done".to_owned());
                }
                Next::Frame(crate::process::Frame::IoError(detail)) => {
                    break Ending::Fault(detail);
                }
                Next::Timeout => break Ending::Deadline,
                Next::Disconnected => {
                    break Ending::Fault("the host output ended without a frame".to_owned());
                }
            }
        };
        match ending {
            Ending::Done(status) => {
                running.wait_for_exit(self.grace);
                running.stop();
                status
            }
            Ending::Fault(detail) => {
                let stderr = running.stop_and_collect_stderr(STDERR_SETTLE);
                runtime_failure(format!("codemode host protocol fault: {detail}{stderr}"))
            }
            Ending::Deadline => {
                running.send(protocol::encode_cancel());
                let _ = running.next(CANCEL_ACK);
                running.stop();
                ProgramStatus::TimedOut
            }
        }
    }
}

fn runtime_failure(message: String) -> ProgramStatus {
    ProgramStatus::Failed {
        class: FailureClass::Runtime,
        message,
    }
}

/// The longest host line the parent accepts for a run with this result cap.
pub(crate) fn line_cap(max_result_bytes: usize) -> usize {
    MIN_LINE_CAP.max(
        max_result_bytes
            .saturating_mul(2)
            .saturating_add(LINE_HEADROOM),
    )
}
