//! The process transport seam (plan Task 16; R-1b compliance): the
//! adapter's ONE channel to child processes, shaped like the task-15
//! `KernelTransport` pattern applied to processes.
//!
//! The runner is GENERIC over [`ProcessTransport`]:
//!
//! - the production implementation, [`ProcessChildTransport`], spawns REAL
//!   child processes through `std::process` — no external dependency — and
//!   reaps them non-blockingly (`try_wait` in [`ProcessTransport::state`]);
//! - the shared contract suite binds script-following IN-MEMORY doubles
//!   (`crate::testing`), so the default battery spawns NO real process
//!   (spec §6.3: the child is what gets doubled, not the adapter's
//!   logic; R-1b: no real builder executes).
//!
//! ## Environment inheritance (correction RES-D3)
//!
//! Spec §10.3 rule 1 requires TWO channels to the child: the prompt file
//! AND the environment. The child inherits the parent's environment
//! verbatim (the `std::process` default) plus exactly one variable,
//! [`RESULT_PATH_ENV_VAR`] (`GAUNTLET_RESULT_PATH`), naming the
//! result-contract path. Credential pruning of the inherited environment
//! belongs to the gate/sandbox composition (plan Task 20), never to this
//! adapter — recorded as the posture, not a TODO here.
//!
//! ## Cancellation (correction RES-D12)
//!
//! `stop` is SIGTERM, then a bounded wait, then SIGKILL: the graceful leg
//! sends `kill -TERM` through a FIXED-ARGV helper (`sh -c 'kill -TERM
//! "$1"' sh <pid>` — the pid is an argv argument, no operator data is
//! ever in the command; std exposes no signal API, and a `libc`/`nix`
//! dependency remains an ask-first decision — RECORDED). A child that
//! already exited makes stop a success (the goal is achieved); reaping
//! after the SIGKILL has a dedicated grace of its own (COR-F1 — the
//! graceful leg may have consumed the caller's bound, and a landed kill
//! with a pending reap is a SUCCESS, not a failure), and the deadline
//! rule of §6 preamble applies to every blocking leg.
//!
//! ## The child's process group (corrections round 2, F-R3)
//!
//! A WRAPPED child is a TREE, not a process: the sandbox (or its driver
//! shell) is the first process and forks the actual payload underneath
//! it. The round-1 falsifier's budget world leaked a live sandbox-driver pair +
//! in-sandbox shell past a cancel whose signals reached only the direct
//! child (`--die-with-parent` does not reap it). Every spawned child is
//! therefore its own process-group leader (`process_group(0)`), and BOTH
//! stop legs address the WHOLE GROUP (`kill -TERM -- -<pid>` /
//! `kill -KILL -- -<pid>`): one bounded signal retires the driver AND
//! everything it forked that has not escaped the group. The stop's
//! contract is unchanged otherwise: bounded legs, a landed kill with a
//! pending reap is `Ok(None)`, never a fire-and-forget signal. (A
//! grandchild that both ignores TERM and outlives its driver's own TERM
//! death — or escapes the group via `setsid` — is beyond this bounded
//! claim; the driver's own death then ends the observable handle. That
//! boundary is recorded in the round-2 report, not papered over here.)

use core::cell::RefCell;
use core::fmt;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

#[cfg(unix)]
use std::os::unix::process::CommandExt;

use gauntlet_ports::deadline::Deadline;

use crate::RESULT_PATH_ENV_VAR;

/// A transport-level child identifier. The production transport uses the
/// child's OS pid; doubles use their own counters. Opaque to the runner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ChildId(u64);

impl ChildId {
    pub(crate) fn new(raw: u64) -> Self {
        Self(raw)
    }

    /// The transport's raw child number (the OS pid for the production
    /// transport — the value the forensic handle record cites, RES-D12).
    pub fn raw(self) -> u64 {
        self.0
    }
}

/// The observed process state. Exit status is LIVENESS evidence only
/// (spec §6.1, §10.3): nothing in the adapter classifies completion from
/// it. `code` is `None` when the child was killed by a signal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessState {
    Running,
    Exited { code: Option<i32> },
}

/// One child to spawn: the rendered argv (the child program is spawned
/// DIRECTLY from these words — no shell wraps them), the working
/// directory, and the result-contract path value for
/// [`RESULT_PATH_ENV_VAR`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessSpec {
    pub argv: Vec<String>,
    pub cwd: PathBuf,
    pub result_path: String,
}

/// Transport failures — typed, carrying the locus (spec §21.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportError {
    /// The child could not start (program missing, cwd unusable, ...).
    SpawnFailed { program: String, detail: String },
    /// The child id is not known to this transport.
    UnknownChild { child: ChildId },
    /// The state probe failed.
    PollFailed { child: ChildId, detail: String },
    /// The bounded stop failed: the child could not be terminated (the
    /// kill itself failed) or the state wait broke. A SIGKILL that
    /// LANDED with only the reap pending is NOT this — see
    /// [`ProcessTransport::stop`] (COR-F1).
    StopFailed { child: ChildId, detail: String },
}

impl fmt::Display for TransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SpawnFailed { program, detail } => {
                write!(f, "spawning {program:?} failed: {detail}")
            }
            Self::UnknownChild { child } => {
                write!(f, "child {} is not known to this transport", child.raw())
            }
            Self::PollFailed { child, detail } => {
                write!(f, "probing child {} failed: {detail}", child.raw())
            }
            Self::StopFailed { child, detail } => {
                write!(f, "stopping child {} failed: {detail}", child.raw())
            }
        }
    }
}

impl std::error::Error for TransportError {}

/// The process seam (spec §6.1's subprocess mechanics, behind a port so
/// the suite runs without a real process). Every blocking operation takes
/// a deadline (§6 preamble).
pub trait ProcessTransport: fmt::Debug {
    /// Spawns the child. Returns promptly after the fork/exec; the child
    /// keeps running.
    fn spawn(&self, spec: &ProcessSpec) -> Result<ChildId, TransportError>;

    /// A non-blocking state probe. MUST NOT block; reaps the child as a
    /// side effect of observing it (no zombies).
    fn state(&self, child: ChildId) -> Result<ProcessState, TransportError>;

    /// Stops the child: SIGTERM, a bounded wait, then SIGKILL — each leg
    /// addressed to the child's WHOLE process group (F-R3: the wrapped
    /// child is a tree; one group signal retires driver and payload). A
    /// child that already exited maps to SUCCESS (the goal is achieved)
    /// with its exit code when known. `Ok(None)` (COR-F1): the SIGKILL
    /// LANDED but the exit code was not observed within the stop's
    /// dedicated reap grace — the child IS stopped; a later state probe
    /// reaps it. `Err(StopFailed)` is reserved for a failed kill or a
    /// broken wait, never a pending reap. Idempotence at the runner
    /// level means a second cancel never reaches the seam.
    fn stop(&self, child: ChildId, deadline: Deadline) -> Result<Option<i32>, TransportError>;
}

#[derive(Debug)]
enum ChildRecord {
    Live(Box<Child>),
    Exited(Option<i32>),
}

/// The production transport: real child processes through `std::process`.
/// Child streams are null on purpose — the child's stdout/stderr are NOT
/// evidence channels (the result contract at the declared path is, spec
/// §10.3), and a chatty agent must never block on a full pipe.
#[derive(Debug, Default)]
pub struct ProcessChildTransport {
    children: RefCell<BTreeMap<u64, ChildRecord>>,
}

impl ProcessChildTransport {
    pub fn new() -> Self {
        Self::default()
    }
}

impl ProcessTransport for ProcessChildTransport {
    fn spawn(&self, spec: &ProcessSpec) -> Result<ChildId, TransportError> {
        let program = spec.argv.first().cloned().unwrap_or_default();
        let mut command = Command::new(&program);
        command
            .args(&spec.argv[1..])
            .current_dir(&spec.cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            // §10.3 rule 1, environment channel (RES-D3): the parent
            // environment is inherited verbatim (the std default) plus
            // exactly this one variable.
            .env(RESULT_PATH_ENV_VAR, &spec.result_path);
        // F-R3: the child is its OWN process-group leader, so the stop
        // path can retire the whole wrapped tree (the driver AND its
        // forked payload) with one bounded group signal — a child that
        // shares the parent's group could only ever be signaled one pid
        // at a time, which is exactly the round-1 leak.
        #[cfg(unix)]
        command.process_group(0);
        let child = command
            .spawn()
            .map_err(|error| TransportError::SpawnFailed {
                detail: error.to_string(),
                program: program.clone(),
            })?;
        let id = child.id() as u64;
        self.children
            .borrow_mut()
            .insert(id, ChildRecord::Live(Box::new(child)));
        Ok(ChildId::new(id))
    }

    fn state(&self, child: ChildId) -> Result<ProcessState, TransportError> {
        let mut children = self.children.borrow_mut();
        let Some(record) = children.get_mut(&child.0) else {
            return Err(TransportError::UnknownChild { child });
        };
        match record {
            ChildRecord::Exited(code) => Ok(ProcessState::Exited { code: *code }),
            ChildRecord::Live(inner) => match inner.try_wait() {
                // try_wait REAPS the exited child — the non-blocking
                // reaping story (RES-D12).
                Ok(Some(status)) => {
                    let code = status.code();
                    *record = ChildRecord::Exited(code);
                    Ok(ProcessState::Exited { code })
                }
                Ok(None) => Ok(ProcessState::Running),
                Err(error) => Err(TransportError::PollFailed {
                    child,
                    detail: error.to_string(),
                }),
            },
        }
    }

    fn stop(&self, child: ChildId, deadline: Deadline) -> Result<Option<i32>, TransportError> {
        // Kill-after-exit maps to success (RES-D12): an already-exited
        // child reports its stored code and nothing else happens.
        if let ProcessState::Exited { code } = self.state(child)? {
            return Ok(code);
        }
        let bound = deadline.instant().get();
        let mut children = self.children.borrow_mut();
        let Some(record) = children.get_mut(&child.0) else {
            return Err(TransportError::UnknownChild { child });
        };
        let ChildRecord::Live(inner) = record else {
            // Raced between the probe and the borrow: an exit is a success.
            return Ok(None);
        };
        // Graceful leg: SIGTERM to the child's WHOLE PROCESS GROUP
        // through a fixed-argv helper. The (negated) pid is an argv
        // ARGUMENT; no operator data is ever part of the helper's command
        // words (see the module docs for the std signal-API gap this
        // works around, and for the F-R3 group mechanics: the wrapped
        // child is a tree, and one group signal retires it).
        let pid = format!("-{}", inner.id());
        // (The negated pid is passed as ONE argv word; dash's kill
        // builtin rejects a `--` separator, so the negated form IS the
        // portable group-pid spelling.)
        let _ = Command::new("sh")
            .args(["-c", "kill -TERM \"$1\"", "sh", &pid])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        // Bounded wait for the graceful exit.
        while wall_now() <= bound {
            match inner.try_wait() {
                Ok(Some(status)) => {
                    let code = status.code();
                    *record = ChildRecord::Exited(code);
                    return Ok(code);
                }
                Ok(None) => std::thread::sleep(STOP_POLL_STEP),
                Err(error) => {
                    return Err(TransportError::StopFailed {
                        child,
                        detail: error.to_string(),
                    });
                }
            }
        }
        // Escalation leg: SIGKILL to the WHOLE GROUP (F-R3), the direct
        // pid as the backstop, then a bounded reap. COR-F1: SIGKILL
        // cannot be caught — once the kill lands the child IS stopped,
        // and only the reap observation is pending. The graceful leg may
        // have consumed the caller's whole bound, so the reap gets its
        // own dedicated grace measured from the START of this leg;
        // expiring it is `Ok(None)` (stopped; exit code unobserved — a
        // later state probe reaps), never `StopFailed`, whose honest
        // scope is a failed kill or a broken wait.
        let _ = Command::new("sh")
            .args(["-c", "kill -KILL \"$1\" 2>/dev/null || true", "sh", &pid])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        if let Err(error) = inner.kill() {
            return Err(TransportError::StopFailed {
                child,
                detail: format!("the kill itself failed ({error}); the child may already be gone"),
            });
        }
        let reap_deadline = wall_now().saturating_add(REAP_GRACE_SECS);
        loop {
            match inner.try_wait() {
                Ok(Some(status)) => {
                    let code = status.code();
                    *record = ChildRecord::Exited(code);
                    return Ok(code);
                }
                Ok(None) => {
                    if wall_now() >= reap_deadline {
                        return Ok(None);
                    }
                    std::thread::sleep(STOP_POLL_STEP);
                }
                Err(error) => {
                    return Err(TransportError::StopFailed {
                        child,
                        detail: error.to_string(),
                    });
                }
            }
        }
    }
}

/// The transport's own pacing clock for its bounded blocking legs (§6
/// preamble): the wall clock, read only inside the production stop's
/// wait loops — never a state input (RES-D9: the runner's ONLY time
/// source is the injected `Clock`).
fn wall_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_secs())
        .unwrap_or(0)
}

/// The longest single wait inside a stop's bounded poll loop.
const STOP_POLL_STEP: std::time::Duration = std::time::Duration::from_millis(20);

/// The dedicated reap grace after a LANDED SIGKILL (COR-F1), in seconds,
/// measured from the start of the escalation leg. SIGKILL is
/// uncatchable: expiring this grace means "stopped; exit code
/// unobserved" (`Ok(None)`), never a failed stop.
const REAP_GRACE_SECS: u64 = 2;

#[cfg(test)]
mod tests {
    use super::*;
    use gauntlet_domain::EpochSeconds;

    fn bound_in(seconds: u64) -> Deadline {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|since| since.as_secs())
            .unwrap_or(0);
        Deadline::at_epoch(EpochSeconds::new(now + seconds))
    }

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "gauntlet-cli-adapter-transport-{tag}-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).expect("scratch is creatable");
        dir
    }

    fn spec(program: &str, tag: &str) -> ProcessSpec {
        ProcessSpec {
            argv: vec!["sh".to_owned(), "-c".to_owned(), program.to_owned()],
            cwd: scratch(tag),
            result_path: "/unused/result.json".to_owned(),
        }
    }

    /// A live child reports Running; a stop terminates and REAPS it; the
    /// state after the stop is Exited (RES-D12's reaping story).
    #[test]
    fn a_live_child_stops_within_its_bound_and_is_reaped() {
        let transport = ProcessChildTransport::new();
        let child = transport
            .spawn(&spec("sleep 30", "stop"))
            .expect("the sleep spawns");
        assert_eq!(
            transport.state(child).expect("state"),
            ProcessState::Running
        );
        let code = transport.stop(child, bound_in(10)).expect("stop reaps");
        assert_eq!(
            transport.state(child).expect("state after stop"),
            ProcessState::Exited { code }
        );
    }

    /// Stopping a child that already exited is a SUCCESS with its code
    /// (RES-D12: kill-after-exit maps to success).
    #[test]
    fn stopping_an_already_exited_child_succeeds() {
        let transport = ProcessChildTransport::new();
        let child = transport.spawn(&spec("exit 3", "exited")).expect("spawns");
        // Poll until the (fast) exit is observed and reaped.
        let mut state = transport.state(child).expect("first state");
        for _ in 0..50 {
            if state != ProcessState::Running {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
            state = transport.state(child).expect("state");
        }
        assert_eq!(state, ProcessState::Exited { code: Some(3) });
        assert_eq!(
            transport.stop(child, bound_in(5)).expect("stop succeeds"),
            Some(3)
        );
    }

    /// The environment channel (RES-D3): the child sees
    /// GAUNTLET_RESULT_PATH in addition to the inherited environment.
    #[test]
    fn the_child_sees_the_result_path_variable() {
        let transport = ProcessChildTransport::new();
        let out = scratch("env").join("observed");
        let mut spec = spec("printf '%s' \"$GAUNTLET_RESULT_PATH\" > observed", "env");
        spec.cwd = scratch("env");
        spec.result_path = out.to_string_lossy().into_owned();
        let child = transport.spawn(&spec).expect("spawns");
        let mut state = transport.state(child).expect("state");
        for _ in 0..100 {
            if state != ProcessState::Running {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
            state = transport.state(child).expect("state");
        }
        assert!(out.is_file(), "the child must observe GAUNTLET_RESULT_PATH");
    }
}
