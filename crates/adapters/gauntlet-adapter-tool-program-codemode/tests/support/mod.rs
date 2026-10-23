//! Fixtures shared by the codemode adapter's integration tests.
//!
//! The codemode tests run the repository's real workbench host. They locate
//! it relative to this crate's manifest directory, so no environment variable
//! is needed, and they FAIL, never skip, when bun or the host entry is absent.

#![allow(dead_code)]

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use gauntlet_adapter_tool_program_codemode::HostCommand;
use gauntlet_domain::InvocationId;
use gauntlet_ports::json::JsonValue;
use gauntlet_ports::tool_catalog::ToolName;
use gauntlet_ports::tool_program::{
    HostRecord, ProgramHost, ProgramLimit, ProgramLimits, ProgramSource, ToolProgramRequest,
    TraceEntry, TraceOutcomeKind, args_digest,
};
use gauntlet_ports::tool_surface::ToolOutcome;

/// The caller identity every test request carries.
pub const CALLER: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";

/// The repository's workbench directory, located from this crate's manifest.
pub fn workbench_dir() -> PathBuf {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../workbench");
    let entry = dir.join("src/codemode/cli.ts");
    assert!(
        entry.is_file(),
        "the codemode host entry is missing at {}; these tests run the repository's workbench \
         host, so they need a full checkout (they do not skip)",
        entry.display()
    );
    dir.canonicalize()
        .expect("the workbench directory resolves to a real path")
}

/// Fails loudly unless `bun` runs. The codemode tests never skip.
pub fn require_bun() {
    match Command::new("bun")
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
    {
        Ok(status) if status.success() => {}
        Ok(status) => panic!("`bun --version` failed ({status}); install bun to run these tests"),
        Err(error) => panic!(
            "bun is not runnable ({error}); install bun and put it on PATH. These tests fail \
             rather than skip when the codemode host cannot run"
        ),
    }
}

/// The real host command: `bun run codemode-host` in the workbench.
pub fn bun_host() -> HostCommand {
    require_bun();
    HostCommand {
        executable: "bun".to_owned(),
        args: vec!["run".to_owned(), "codemode-host".to_owned()],
        workdir: workbench_dir(),
    }
}

pub fn caller() -> InvocationId {
    InvocationId::new(CALLER).expect("the test caller is a valid invocation id")
}

pub fn limits(
    timeout_ms: u64,
    max_tool_calls: u32,
    max_parallel_calls: u32,
    max_result_bytes: usize,
) -> ProgramLimits {
    ProgramLimits {
        timeout: std::time::Duration::from_millis(timeout_ms),
        max_tool_calls,
        max_parallel_calls,
        max_result_bytes,
        max_source_bytes: 64 * 1024,
    }
}

pub fn request(text: &str, admitted: &[&str], limits: ProgramLimits) -> ToolProgramRequest {
    let source = ProgramSource::new(text, limits.max_source_bytes).expect("the source fits");
    let admitted = admitted
        .iter()
        .map(|name| ToolName::new(name).expect("the test tool names are valid"))
        .collect();
    ToolProgramRequest::new(caller(), source, admitted, limits).expect("the request is valid")
}

/// A recording host for the JavaScript-native tests. It enforces admission and
/// the call-count bound the way a real host does, and it records what reached
/// it, so the trace can be compared with the outcome.
pub struct RecordingHost {
    admitted: Vec<ToolName>,
    limits: ProgramLimits,
    state: RefCell<State>,
}

#[derive(Default)]
struct State {
    trace: Vec<TraceEntry>,
    executed: Vec<ToolName>,
    attempts: u32,
    breach: Option<ProgramLimit>,
}

impl RecordingHost {
    pub fn new(request: &ToolProgramRequest) -> Self {
        Self {
            admitted: request.admitted().to_vec(),
            limits: request.limits(),
            state: RefCell::new(State::default()),
        }
    }

    /// The tools that executed, in order.
    pub fn executed(&self) -> Vec<ToolName> {
        self.state.borrow().executed.clone()
    }
}

impl ProgramHost for RecordingHost {
    fn call(&self, name: &ToolName, args: &JsonValue) -> ToolOutcome {
        let mut state = self.state.borrow_mut();
        let seq = u32::try_from(state.trace.len()).unwrap_or(u32::MAX);
        let digest = args_digest(args);
        if state.breach.is_some() {
            state.trace.push(TraceEntry {
                seq,
                tool: name.clone(),
                args_digest: digest,
                outcome: TraceOutcomeKind::Limit,
            });
            return refused("limit_refused");
        }
        state.attempts = state.attempts.saturating_add(1);
        if state.attempts > self.limits.max_tool_calls {
            state.breach = Some(ProgramLimit::ToolCalls);
            state.trace.push(TraceEntry {
                seq,
                tool: name.clone(),
                args_digest: digest,
                outcome: TraceOutcomeKind::Limit,
            });
            return refused("limit_refused");
        }
        if !self.admitted.contains(name) {
            state.trace.push(TraceEntry {
                seq,
                tool: name.clone(),
                args_digest: digest,
                outcome: TraceOutcomeKind::Refused,
            });
            return refused("not_admitted");
        }
        state.trace.push(TraceEntry {
            seq,
            tool: name.clone(),
            args_digest: digest,
            outcome: TraceOutcomeKind::Ok,
        });
        state.executed.push(name.clone());
        ToolOutcome::Ok(JsonValue::Object(vec![(
            "ok".to_owned(),
            JsonValue::Bool(true),
        )]))
    }

    fn call_group(&self, calls: &[(ToolName, JsonValue)]) -> Vec<ToolOutcome> {
        calls
            .iter()
            .map(|(name, args)| self.call(name, args))
            .collect()
    }

    fn record(&self) -> HostRecord {
        let state = self.state.borrow();
        HostRecord {
            trace: state.trace.clone(),
            breach: state.breach,
        }
    }
}

fn refused(code: &str) -> ToolOutcome {
    ToolOutcome::Refused {
        code: code.to_owned(),
        detail: "refused by the recording host".to_owned(),
    }
}
