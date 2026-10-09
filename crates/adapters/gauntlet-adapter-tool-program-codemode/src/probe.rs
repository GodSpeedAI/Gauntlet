//! The codemode capability probe, for `gauntlet doctor`.
//!
//! Three facts are measured, in order: the host runner is present, the host
//! entry is present, and a one-shot program (`return 1+1`, no tools) runs to
//! completion. Each check reports on its own. Absence is `Absent`, which is a
//! plain report and not a failure of anything else.

use std::path::Path;
use std::process::{Command, Stdio};

use gauntlet_domain::InvocationId;
use gauntlet_ports::json::{JsonValue, parse};
use gauntlet_ports::tool_catalog::ToolName;
use gauntlet_ports::tool_program::{
    HostRecord, ProgramHost, ProgramLimits, ProgramSource, ProgramStatus, ToolProgramRequest,
    ToolProgramRuntime,
};
use gauntlet_ports::tool_surface::ToolOutcome;

use crate::process::{HostCommand, allowed_env};
use crate::runtime::{CodemodeRuntime, DEFAULT_PROGRAM_TIMEOUT};

/// The workbench directory, relative to the working directory, that the
/// doctor probes. The composition root passes it to [`probe_codemode_in`].
pub const DEFAULT_WORKBENCH_DIR: &str = "workbench";

/// The host entry, relative to the workbench directory.
pub const HOST_ENTRY: &str = "src/codemode/cli.ts";

/// The runner the host is started with.
const RUNNER: &str = "bun";
const RUNNER_ARGS: &[&str] = &["run", "codemode-host"];

const PROBE_CALLER: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
const PROBE_PROGRAM: &str = "return 1+1";

/// One measured fact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProbeCheck {
    Ok,
    /// Not present on this machine or in this checkout.
    Absent(String),
    /// Present, but the measurement failed.
    Failed(String),
    /// Not attempted because an earlier fact is missing.
    Skipped,
}

/// The three facts the doctor reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodemodeProbe {
    pub runner: ProbeCheck,
    pub entry: ProbeCheck,
    pub one_shot: ProbeCheck,
}

impl CodemodeProbe {
    /// Whether every fact was measured clean.
    pub fn available(&self) -> bool {
        [&self.runner, &self.entry, &self.one_shot]
            .iter()
            .all(|check| **check == ProbeCheck::Ok)
    }

    /// One plain line for the human summary.
    pub fn summary(&self) -> String {
        if self.available() {
            return "available: bun runner, host entry, and one-shot program measured clean"
                .to_owned();
        }
        format!(
            "not available: runner {}; entry {}; one-shot {}",
            render(&self.runner),
            render(&self.entry),
            render(&self.one_shot)
        )
    }
}

fn render(check: &ProbeCheck) -> String {
    match check {
        ProbeCheck::Ok => "ok".to_owned(),
        ProbeCheck::Absent(detail) => format!("absent ({detail})"),
        ProbeCheck::Failed(detail) => format!("failed ({detail})"),
        ProbeCheck::Skipped => "skipped".to_owned(),
    }
}

/// The real host command: `bun run codemode-host` in the workbench directory.
pub fn bun_codemode_host(workbench_dir: &Path) -> HostCommand {
    HostCommand {
        executable: RUNNER.to_owned(),
        args: RUNNER_ARGS.iter().map(|arg| (*arg).to_owned()).collect(),
        workdir: workbench_dir.to_path_buf(),
    }
}

/// Probes the codemode host in the workbench at `workbench_dir`.
pub fn probe_codemode_in(workbench_dir: &Path) -> CodemodeProbe {
    probe_host(&bun_codemode_host(workbench_dir))
}

/// Probes an arbitrary host command with the same three facts.
pub fn probe_host(host: &HostCommand) -> CodemodeProbe {
    let runner = probe_runner(&host.executable);
    let entry = probe_entry(&host.workdir);
    let one_shot = if runner == ProbeCheck::Ok && entry == ProbeCheck::Ok {
        probe_one_shot(host)
    } else {
        ProbeCheck::Skipped
    };
    CodemodeProbe {
        runner,
        entry,
        one_shot,
    }
}

fn probe_runner(executable: &str) -> ProbeCheck {
    let result = Command::new(executable)
        .arg("--version")
        .env_clear()
        .envs(allowed_env())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    match result {
        Ok(status) if status.success() => ProbeCheck::Ok,
        Ok(status) => ProbeCheck::Failed(format!("{executable} --version exited {status}")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            ProbeCheck::Absent(format!("{executable} is not on PATH"))
        }
        Err(error) => ProbeCheck::Failed(format!("{executable} could not run: {error}")),
    }
}

fn probe_entry(workbench_dir: &Path) -> ProbeCheck {
    let entry = workbench_dir.join(HOST_ENTRY);
    if entry.is_file() {
        ProbeCheck::Ok
    } else {
        ProbeCheck::Absent(format!("{} is not present", entry.display()))
    }
}

fn probe_one_shot(host: &HostCommand) -> ProbeCheck {
    let runtime = CodemodeRuntime::new(host.clone());
    let limits = ProgramLimits {
        timeout: DEFAULT_PROGRAM_TIMEOUT,
        max_tool_calls: 0,
        max_parallel_calls: 1,
        max_result_bytes: 4096,
        max_source_bytes: 4096,
    };
    let caller = match InvocationId::new(PROBE_CALLER) {
        Ok(caller) => caller,
        Err(error) => return ProbeCheck::Failed(format!("the probe caller is invalid: {error:?}")),
    };
    let source = match ProgramSource::new(PROBE_PROGRAM, limits.max_source_bytes) {
        Ok(source) => source,
        Err(error) => return ProbeCheck::Failed(error.to_string()),
    };
    let request = match ToolProgramRequest::new(caller, source, Vec::<ToolName>::new(), limits) {
        Ok(request) => request,
        Err(error) => return ProbeCheck::Failed(error.to_string()),
    };
    let outcome = runtime.run(&request, &NoTools);
    let expected = parse("2").ok();
    match outcome.status {
        ProgramStatus::Completed(value) if Some(&value) == expected.as_ref() => ProbeCheck::Ok,
        ProgramStatus::Completed(value) => ProbeCheck::Failed(format!(
            "the one-shot program returned {}, not 2",
            value.to_json_text()
        )),
        other => ProbeCheck::Failed(format!("the one-shot program ended {other:?}")),
    }
}

/// The host a probe program runs against. It admits no tools, so any call is
/// refused, and it records nothing.
struct NoTools;

impl ProgramHost for NoTools {
    fn call(&self, _name: &ToolName, _args: &JsonValue) -> ToolOutcome {
        ToolOutcome::Refused {
            code: "not_admitted".to_owned(),
            detail: "the probe admits no tools".to_owned(),
        }
    }

    fn call_group(&self, calls: &[(ToolName, JsonValue)]) -> Vec<ToolOutcome> {
        calls
            .iter()
            .map(|(name, args)| self.call(name, args))
            .collect()
    }

    fn record(&self) -> HostRecord {
        HostRecord {
            trace: Vec::new(),
            breach: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_absent_workbench_is_reported_plainly_and_is_not_available() {
        let missing = Path::new("/nonexistent/gauntlet-workbench-for-probe-test");
        let probe = probe_codemode_in(missing);
        assert!(matches!(probe.entry, ProbeCheck::Absent(_)), "{probe:?}");
        assert_eq!(probe.one_shot, ProbeCheck::Skipped);
        assert!(!probe.available());
        assert!(probe.summary().starts_with("not available:"));
    }

    #[test]
    fn a_missing_runner_is_absent_not_failed() {
        let host = HostCommand {
            executable: "gauntlet-no-such-runner-for-probe".to_owned(),
            args: Vec::new(),
            workdir: Path::new("/nonexistent").to_path_buf(),
        };
        let probe = probe_host(&host);
        assert!(matches!(probe.runner, ProbeCheck::Absent(_)), "{probe:?}");
        assert_eq!(probe.one_shot, ProbeCheck::Skipped);
    }
}
