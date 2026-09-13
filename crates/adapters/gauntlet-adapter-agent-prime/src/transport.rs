//! The adapter's two harness seams (plan Task 15 B1: parameterize the
//! runner over a kernel transport and a harness control).
//!
//! **Kernel transport** — submits a RENDERED kernel program and observes
//! the ADMISSION outcome. The return type is the unrepresentability
//! device: [`KernelTransport::submit`] returns an
//! [`AdmissionHandle`], whose fields are exactly the four admission
//! fields the harness documents (`rlm_child_id`, `name`, `session_dir`,
//! `model`) — there is no field a child ANSWER could occupy (Annex C.2
//! rule 1: "Never await a child answer from `rlm(...)`"; §10.5: "it never
//! returns the child's answer"). The result contract is the only path a
//! result may take, and it is read exclusively by the runner's poll and
//! collect (the arch await-ban test pins that this module, the process
//! spawn site, cannot touch it — the parent print run's stdout is
//! captured for forensics only, never parsed as a result source).
//!
//! **Harness control** — `stop` for cancellation and `list` for
//! liveness/forensics (§10.5: "`rlm.list_subagents()` is used for
//! liveness and forensics only"; Annex C.4: a poll that finds no process
//! must NOT conclude failure — so `list` is never a poll input; the
//! runner calls `stop` on cancellation and nothing else).
//!
//! **Parent process ownership (task26-dcritic-apparatus-repair, RC1
//! correction — supersedes the premise below).** [`KernelTransport::submit`]
//! now returns the admission handle TOGETHER WITH a [`ParentHandle`]: the
//! live parent print-run process, handed to the caller so the INVOCATION
//! owns its lifetime, not the transport. The transport's earlier posture —
//! "a print run is disposable; its daemon-backed child is what survives" —
//! was FALSE for prime-agent 0.8.0 (verified by execution, task26 real-run
//! evidence): a child of a print-mode parent materializes and keeps
//! running ONLY while the parent session stays alive (see `program.rs`'s
//! module docs, COR-D2 divergence #4). Disposing of the parent right after
//! admission — the previous "detached bounded reaper" — killed the parent
//! the child depended on mid-work, which is exactly the defect this
//! correction fixes: real work aborts with `Request was aborted`, the
//! runner then polls for a result that can never arrive, and the eventual
//! timeout's `stop` on a harness child that was never daemon-registered
//! failed and was (before RC3, below) treated as fatal.
//!
//! Both seams have ONE production implementation apiece, shelling out to
//! the configured binary through `std::process` — no external dependency
//! — and in-memory doubles live in the adapter's own test targets (the
//! shared contract suite runs against those, never against a harness).

use core::fmt;
use std::path::{Path, PathBuf};

use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::json::{self, JsonValue};

/// The admission-handle fields, exactly as the harness documents them
/// (`RLMSpawnHandle`: `rlm_child_id`, `name`, `session_dir`, `model`).
///
/// This type is the await-ban made structural: four strings, no channel
/// for an answer. Pinned by the arch suite's `admission_handle_fields`
/// test — adding a field there is a deliberate, reviewed edit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmissionHandle {
    pub rlm_child_id: String,
    pub name: String,
    pub session_dir: String,
    pub model: String,
}

impl AdmissionHandle {
    /// The declared handle fields (strict parse: unknown fields and
    /// missing fields are typed errors — the handle document is written
    /// by a model-run program, so it is UNTRUSTED EXTERNAL INPUT; spec
    /// §21.2: parse, validate, then admit).
    pub const FIELDS: &[&str] = &["rlm_child_id", "name", "session_dir", "model"];

    pub fn parse(text: &str) -> Result<Self, TransportError> {
        let document = json::parse(text).map_err(|error| TransportError::HandleUnparsable {
            detail: format!("the admission handle document is not valid JSON: {error}"),
        })?;
        let object = match &document {
            JsonValue::Object(_) => &document,
            _ => {
                return Err(TransportError::HandleUnparsable {
                    detail: "the admission handle document is not a JSON object".to_owned(),
                });
            }
        };
        for key in object.keys() {
            if !Self::FIELDS.contains(&key.as_str()) {
                return Err(TransportError::HandleUnparsable {
                    detail: format!("the admission handle carries unknown field {key:?}"),
                });
            }
        }
        let field = |name: &str| -> Result<String, TransportError> {
            match object.get(name) {
                Some(JsonValue::Str(value)) if !value.trim().is_empty() => Ok(value.clone()),
                Some(JsonValue::Str(_)) => Err(TransportError::HandleUnparsable {
                    detail: format!("the admission handle field {name:?} is empty"),
                }),
                Some(_) => Err(TransportError::HandleUnparsable {
                    detail: format!("the admission handle field {name:?} is not a string"),
                }),
                None => Err(TransportError::HandleUnparsable {
                    detail: format!("the admission handle is missing field {name:?}"),
                }),
            }
        };
        Ok(Self {
            rlm_child_id: field("rlm_child_id")?,
            name: field("name")?,
            session_dir: field("session_dir")?,
            model: field("model")?,
        })
    }

    /// Serializes the handle the way the rendered program writes it —
    /// the production transport and the test doubles both read through
    /// [`AdmissionHandle::parse`], so the suite exercises the real
    /// strict parse.
    pub fn to_document(&self) -> String {
        let string = |value: &str| json::JsonValue::Str(value.to_owned()).to_json_text();
        format!(
            "{{\"rlm_child_id\":{},\"name\":{},\"session_dir\":{},\"model\":{}}}",
            string(&self.rlm_child_id),
            string(&self.name),
            string(&self.session_dir),
            string(&self.model),
        )
    }
}

/// One kernel-program submission: the rendered program text, the handle
/// path the program writes, the workspace the parent run starts in, and
/// the model hint the harness may honor (`rlm.run` `model` selection).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KernelSubmission {
    pub program: String,
    pub handle_path: PathBuf,
    pub workspace: PathBuf,
    pub model_hint: Option<String>,
}

/// Kernel-transport failures — typed, carrying the locus (spec §21.2).
///
/// The failure taxonomy is the correction RES-D4's: a handle that is
/// ABSENT or UNDECODABLE means nothing was (verifiably) admitted —
/// [`TransportError::HandleAbsent`] / [`TransportError::HandleUnparsable`],
/// which the runner maps to `AgentError::AdmissionRefused` and follows
/// with an orphan sweep. A TRANSPORT or binary failure — the parent run
/// could not even start — is [`TransportError::SpawnFailed`], mapped to
/// `AgentError::Transport`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportError {
    /// The parent run could not be started (binary/transport failure).
    SpawnFailed { binary: PathBuf, detail: String },
    /// The bounded handle watch ended without a readable handle
    /// document: either the watch passed its bound, or the parent run
    /// ended without the program ever writing the handle. No admission
    /// is verifiable.
    HandleAbsent { detail: String },
    /// The handle document was read but rejected by the strict parse —
    /// the child may have been admitted before the write went wrong.
    HandleUnparsable { detail: String },
}

impl fmt::Display for TransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SpawnFailed { binary, detail } => {
                write!(
                    f,
                    "the parent run could not start ({}): {detail}",
                    binary.display()
                )
            }
            Self::HandleAbsent { detail } => {
                write!(
                    f,
                    "admission handle absent — nothing verifiably admitted: {detail}"
                )
            }
            Self::HandleUnparsable { detail } => {
                write!(f, "the admission handle document was rejected: {detail}")
            }
        }
    }
}

impl std::error::Error for TransportError {}

/// The kernel-transport seam. Implementations submit the program and
/// return the ADMISSION outcome only — never a child answer, never a
/// result document (the type cannot carry one; see the module docs) —
/// TOGETHER WITH a [`ParentHandle`] the caller now owns (RC1 correction):
/// the transport's job ends at admission; what happens to the parent
/// process for the rest of the invocation's life is the RUNNER's call.
pub trait KernelTransport: fmt::Debug {
    fn submit(
        &self,
        submission: &KernelSubmission,
        deadline: Deadline,
    ) -> Result<(AdmissionHandle, Box<dyn ParentHandle>), TransportError>;
}

/// Shared ownership for transports whose forensic surface (e.g. the
/// live probe's D1 evidence via [`ProcessKernelTransport::take_forensics`])
/// must outlive construction of the runner.
impl<T: KernelTransport + ?Sized> KernelTransport for std::sync::Arc<T> {
    fn submit(
        &self,
        submission: &KernelSubmission,
        deadline: Deadline,
    ) -> Result<(AdmissionHandle, Box<dyn ParentHandle>), TransportError> {
        (**self).submit(submission, deadline)
    }
}

/// The live parent print-run process, handed back at admission (RC1
/// correction, task26-dcritic-apparatus-repair): the INVOCATION owns this
/// for as long as it lives. The runner may [`ParentHandle::try_reap`] it
/// opportunistically as FORENSICS ONLY — never as a completion or failure
/// signal (Annex C.2/C.4: a poll that finds no process must not conclude
/// failure while the result contract is still pending and the deadline
/// has not expired) — and must [`ParentHandle::terminate`] it once the
/// invocation reaches a terminal state (completion, timeout, or cancel),
/// so a finished invocation never leaks a live process.
pub trait ParentHandle: fmt::Debug {
    /// A best-effort, NON-BLOCKING reap: `Some(note)` the first time the
    /// parent is observed to have exited, `None` otherwise (including
    /// every call after the first `Some`, so a caller polling this
    /// repeatedly never re-triggers OS-level reap errors on an
    /// already-collected child). FORENSICS ONLY.
    fn try_reap(&mut self) -> Option<String>;

    /// Terminates the parent (SIGTERM, a bounded grace, then a
    /// last-resort kill) and returns a forensic note describing the
    /// disposition. Idempotent: calling it again after the parent is
    /// already gone is a cheap no-op that still returns a note.
    fn terminate(&mut self) -> String;
}

/// A [`ParentHandle`] for kernel-transport doubles that model no real
/// process (the harness is what tests double, never this adapter's own
/// owned-lifetime logic — spec §6.3). Provided so suite/unit doubles need
/// not each define their own trivial implementation.
#[derive(Debug, Default)]
pub struct NullParentHandle;

impl ParentHandle for NullParentHandle {
    fn try_reap(&mut self) -> Option<String> {
        None
    }

    fn terminate(&mut self) -> String {
        "no-op: this double models no real parent process".to_owned()
    }
}

/// Reads and strictly parses the admission-handle document the rendered
/// program writes. The production transport and the suite doubles share
/// this path, so the suite exercises the real untrusted-input parse.
pub fn read_admission(handle_path: &Path) -> Result<AdmissionHandle, TransportError> {
    let text =
        std::fs::read_to_string(handle_path).map_err(|error| TransportError::HandleAbsent {
            detail: format!(
                "no readable handle document at {} ({error})",
                handle_path.display()
            ),
        })?;
    AdmissionHandle::parse(&text)
}

/// Harness-control failures — typed, carrying the locus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlError {
    /// The stop command failed for a child that should exist.
    StopFailed { child: String, detail: String },
    /// The child is already gone. Cancellation's goal is achieved; the
    /// runner treats this as success (idempotent cancel, spec §7.3).
    NoSuchChild { child: String },
    /// The liveness/forensics listing failed.
    ListFailed { detail: String },
}

impl fmt::Display for ControlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StopFailed { child, detail } => {
                write!(f, "stopping child {child} failed: {detail}")
            }
            Self::NoSuchChild { child } => {
                write!(f, "child {child} is already gone")
            }
            Self::ListFailed { detail } => write!(f, "the harness listing failed: {detail}"),
        }
    }
}

impl std::error::Error for ControlError {}

/// One harness child, as the liveness/forensics listing reports it.
/// FORENSIC ONLY (Annex C.2: "Treat `list_subagents` as liveness/
/// forensics — not as a completion signal"): nothing branches runner
/// state on this.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubagentRecord {
    pub session_name: String,
    pub status: String,
    pub session_dir: String,
}

/// The harness-control seam: `stop` for cancellation, `list` for
/// forensics.
pub trait HarnessControl: fmt::Debug {
    fn stop(&self, child: &str, deadline: Deadline) -> Result<(), ControlError>;
    fn list(&self, deadline: Deadline) -> Result<Vec<SubagentRecord>, ControlError>;
}

/// The poll interval of the bounded process waits, in milliseconds.
const PROCESS_POLL_MILLIS: u64 = 50;

/// The grace a graceful stop allows before the last-resort kill, in
/// seconds (a whole second: a print run is disposable, and the stop must
/// never approach the admission bound in usable configurations).
const STOP_GRACE_SECS: u64 = 1;

/// The production kernel transport: it receives the rendered submission,
/// starts ONE headless parent print run over the configured binary with
/// the program inline (delimited, verbatim), and then — this is the load-
/// bearing shape (correction RES-D1) — watches for the HANDLE FILE under
/// a bounded deadline. `invoke` returns the moment the handle exists and
/// strict-parses; the parent print run's LIFETIME is NOT admission. On
/// the SUCCESS path the still-live parent [`std::process::Child`] is
/// wrapped as a [`RealParentHandle`] and handed back to the CALLER (RC1
/// correction, task26-dcritic-apparatus-repair): the transport never
/// disposes of a successfully-admitted parent itself — the runner owns
/// its lifetime for as long as the invocation lives (see the module
/// docs). On the failure paths nothing was admitted, so the bounded stop
/// stays immediate and inline. Every step's outcome is recorded as
/// forensics. A parent model that rambles after the cell — or waits on
/// its child — cannot stretch admission, and invoke can never collapse
/// admission and completion into one blocking call (spec §6.1: "invoke
/// MUST return promptly after admission"; AK-009).
///
/// The parent run's stdout/stderr are piped and DISCARDED beyond this
/// module: they are never a result source (the arch await-ban scan pins
/// that no result-decoding call exists outside the runner's poll/collect).
#[derive(Debug)]
pub struct ProcessKernelTransport {
    binary: PathBuf,
    admission_timeout: std::time::Duration,
    /// The last submission's forensic note (D1 split evidence: the
    /// parent's try_wait state at handle parse). A mutex: `submit` takes
    /// `&self` (the transport is shared infrastructure), so recording the
    /// note needs interior mutability even though disposition is now
    /// synchronous (RC1: no detached thread appends to it after return).
    forensics: std::sync::Mutex<Option<String>>,
}

impl ProcessKernelTransport {
    pub fn new(binary: PathBuf, admission_timeout: std::time::Duration) -> Self {
        Self {
            binary,
            admission_timeout,
            forensics: std::sync::Mutex::new(None),
        }
    }

    /// The last submission's forensic note, if any (D1 evidence leg for
    /// the live probe).
    pub fn take_forensics(&self) -> Option<String> {
        self.forensics.lock().expect("the forensics mutex").take()
    }

    /// The parent run's message: instruction + the program, delimited.
    fn parent_message(program: &str) -> String {
        format!(
            "Execute the following Python program VERBATIM as a single IPython cell. \
             Do not modify it, do not summarize it, do not answer its prompt yourself. \
             It delegates work and records an admission handle to a file; your part \
             ends when the cell returns. Do not wait for the delegated child.\n\
             <<<GAUNTLET_PROGRAM_BEGIN>>>\n{program}<<<GAUNTLET_PROGRAM_END>>>\n"
        )
    }

    /// The system-now seconds. The transport is infrastructure and owns
    /// its admission bound (the runner's port-level bound stays the
    /// spec's deadline).
    fn now_secs() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|since| since.as_secs())
            .unwrap_or(0)
    }
}

/// Bounded disposition of a parent run: wait `linger` for a natural exit
/// (a zero linger skips straight to the stop), then a best-effort
/// GRACEFUL stop: SIGTERM through the platform `kill` utility when
/// available (unix), a short bounded grace, then the last-resort
/// [`std::process::Child::kill`]. Every step's outcome lands in the
/// returned forensic text — the stop is NEVER allowed to block its
/// caller beyond its bound. Free function (not tied to
/// `ProcessKernelTransport`) so [`RealParentHandle`] can call it too — it
/// is the ONE termination routine, invoked by `submit`'s failure path and
/// by `RealParentHandle::terminate`/`Drop` (RC1 correction).
fn stop_bounded(child: &mut std::process::Child, linger: std::time::Duration) -> String {
    let mut note = String::new();
    // The linger wait: a natural exit during the linger is the
    // expected shape when the caller lingered on purpose.
    let linger_ends = ProcessKernelTransport::now_secs().saturating_add(linger.as_secs());
    while child.try_wait().map_or(true, |exited| exited.is_none()) {
        if ProcessKernelTransport::now_secs() >= linger_ends {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(PROCESS_POLL_MILLIS));
    }
    if let Ok(Some(status)) = child.try_wait() {
        note.push_str(&format!("parent exited ({status})"));
        return note;
    }
    #[cfg(unix)]
    {
        let id = child.id();
        let term = std::process::Command::new("kill")
            .arg(id.to_string())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
        match term {
            Ok(status) if status.success() => note.push_str("SIGTERM sent; "),
            other => note.push_str(&format!("SIGTERM unavailable ({other:?}); ")),
        }
    }
    // Bounded grace for a graceful exit; the last-resort kill is
    // unconditional on the grace expiring.
    let grace_ends = ProcessKernelTransport::now_secs().saturating_add(STOP_GRACE_SECS);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                note.push_str(&format!("parent exited ({status})"));
                return note;
            }
            Ok(None) => {
                if ProcessKernelTransport::now_secs() >= grace_ends {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(PROCESS_POLL_MILLIS));
            }
            Err(error) => {
                note.push_str(&format!("waiting after stop failed: {error}; "));
                break;
            }
        }
    }
    match child.kill() {
        Ok(()) => note.push_str("last-resort kill sent"),
        Err(error) => note.push_str(&format!("kill failed: {error}")),
    }
    let _ = child.wait();
    note
}

/// The production [`ParentHandle`]: a live parent-run [`std::process::Child`]
/// (RC1 correction). `try_reap` is a cheap, non-blocking, ONE-SHOT
/// `try_wait` — forensics only, never a completion/failure signal. Once
/// the process is observed exited it is marked `reaped` so a second
/// `try_wait` never runs (the OS has already collected the child; calling
/// it again is at best a wasted syscall and at worst an error on some
/// platforms).
///
/// **Drop**: if the runner (and this handle with it) is dropped while the
/// parent is still alive — e.g. an interrupted process, a panicking
/// caller — the parent is TERMINATED rather than leaked: `Drop` calls the
/// same bounded [`stop_bounded`] routine used everywhere else, with a
/// zero linger (the invocation is going away; there is nothing left to
/// linger for).
#[derive(Debug)]
pub struct RealParentHandle {
    child: std::process::Child,
    reaped: bool,
}

impl ParentHandle for RealParentHandle {
    fn try_reap(&mut self) -> Option<String> {
        if self.reaped {
            return None;
        }
        match self.child.try_wait() {
            Ok(Some(status)) => {
                self.reaped = true;
                Some(format!("parent exited ({status})"))
            }
            _ => None,
        }
    }

    fn terminate(&mut self) -> String {
        if self.reaped {
            return "parent already reaped; nothing to terminate".to_owned();
        }
        let note = stop_bounded(&mut self.child, std::time::Duration::ZERO);
        self.reaped = true;
        note
    }
}

impl Drop for RealParentHandle {
    fn drop(&mut self) {
        // Best-effort only: Drop cannot propagate a failure, and a
        // process already reaped (`self.reaped`) or already exited on
        // its own needs no action — prefer terminating a still-live
        // parent over leaking it (see the type docs).
        if !self.reaped && matches!(self.child.try_wait(), Ok(None)) {
            let _ = stop_bounded(&mut self.child, std::time::Duration::ZERO);
        }
    }
}

impl KernelTransport for ProcessKernelTransport {
    fn submit(
        &self,
        submission: &KernelSubmission,
        deadline: Deadline,
    ) -> Result<(AdmissionHandle, Box<dyn ParentHandle>), TransportError> {
        use std::process::{Command, Stdio};
        *self.forensics.lock().expect("the forensics mutex") = None;
        let mut note = String::new();
        let timeout_ends = Self::now_secs().saturating_add(self.admission_timeout.as_secs().max(1));
        let mut child = Command::new(&self.binary)
            .arg("--print")
            .arg(Self::parent_message(&submission.program))
            .current_dir(&submission.workspace)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| TransportError::SpawnFailed {
                binary: self.binary.clone(),
                detail: error.to_string(),
            })?;
        note.push_str(&format!("parent run pid {} started; ", child.id()));
        // THE BOUNDED HANDLE WATCH (RES-D1): the deadline guards the
        // watch for HANDLE_PATH — never the parent process lifetime.
        // A parse failure while the parent is STILL RUNNING is treated as
        // a partial write (the program's write_text is not atomic) and
        // retried within the bound; a parse failure after the parent
        // ended is final.
        let outcome = loop {
            let parent_gone = matches!(child.try_wait(), Ok(Some(_)));
            // Handle first: the earliest evidence wins.
            if submission.handle_path.is_file() {
                match read_admission(&submission.handle_path) {
                    Ok(handle) => break Ok(handle),
                    Err(error) => {
                        if parent_gone {
                            note.push_str("parent exited with an unreadable handle; ");
                            break Err(error);
                        }
                        // Partial write in flight: bounded retry.
                    }
                }
            } else if parent_gone {
                note.push_str("parent exited before any handle; ");
                break Err(TransportError::HandleAbsent {
                    detail: format!(
                        "the parent run ended without writing the handle document at {}",
                        submission.handle_path.display()
                    ),
                });
            }
            let now = Self::now_secs();
            let deadline_passed = deadline.exceeded_at(gauntlet_domain::EpochSeconds::new(now));
            let timeout_passed = now >= timeout_ends;
            if deadline_passed || timeout_passed {
                note.push_str(&format!(
                    "handle watch ended (deadline passed: {deadline_passed}, admission \
                     timeout passed: {timeout_passed}); "
                ));
                break Err(TransportError::HandleAbsent {
                    detail: format!(
                        "no readable handle document appeared at {} within the admission \
                         bound (deadline passed: {deadline_passed}, admission timeout \
                         passed: {timeout_passed})",
                        submission.handle_path.display()
                    ),
                });
            }
            std::thread::sleep(std::time::Duration::from_millis(PROCESS_POLL_MILLIS));
        };
        // Post-parse disposition. The D1 SPLIT evidence lives here: the
        // try_wait state AT PARSE TIME, taken before any disposition.
        // RC1 correction (task26-dcritic-apparatus-repair, supersedes the
        // earlier "detached bounded reaper" here): on the SUCCESS path
        // the still-live parent is wrapped as a [`RealParentHandle`] and
        // handed back to the CALLER — the transport does not stop it,
        // does not detach a thread to stop it later, and does not decide
        // when its life ends. Failure paths keep the IMMEDIATE bounded
        // stop (nothing was admitted that needs a living parent; the
        // orphan sweep handles any stray child). invoke NEVER waits for
        // any of this (RES-D1 untouched).
        match outcome {
            Ok(handle) => {
                let at_parse = match child.try_wait() {
                    Ok(None) => "NotExited",
                    Ok(Some(_)) => "Exited",
                    Err(_) => "Unknown",
                };
                note.push_str(&format!(
                    "handle parsed while parent.try_wait()={at_parse}; parent handle returned \
                     to the caller — the invocation now owns its lifetime (RC1, \
                     task26-dcritic-apparatus-repair)."
                ));
                *self.forensics.lock().expect("the forensics mutex") = Some(note);
                Ok((
                    handle,
                    Box::new(RealParentHandle {
                        child,
                        reaped: false,
                    }) as Box<dyn ParentHandle>,
                ))
            }
            Err(error) => {
                note.push_str(&stop_bounded(&mut child, std::time::Duration::ZERO));
                note.push('.');
                *self.forensics.lock().expect("the forensics mutex") = Some(note);
                Err(error)
            }
        }
    }
}

/// The production harness control: `prime-agent stop <child>` and
/// `prime-agent list --json`, both bounded waits over `std::process`.
#[derive(Debug, Clone)]
pub struct ProcessHarnessControl {
    binary: PathBuf,
}

impl ProcessHarnessControl {
    pub fn new(binary: PathBuf) -> Self {
        Self { binary }
    }

    fn run_bounded(&self, args: &[&str], deadline: Deadline) -> Result<String, ControlError> {
        use std::process::{Command, Stdio};
        let mut child = Command::new(&self.binary)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| ControlError::StopFailed {
                child: args.last().copied().unwrap_or("").to_owned(),
                detail: format!("spawning {} failed: {error}", self.binary.display()),
            })?;
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) => {
                    if deadline.exceeded_at(gauntlet_domain::EpochSeconds::new(
                        std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|since| since.as_secs())
                            .unwrap_or(0),
                    )) {
                        let _ = child.kill();
                        let _ = child.wait();
                        return Err(ControlError::StopFailed {
                            child: args.last().copied().unwrap_or("").to_owned(),
                            detail: "the control command passed its deadline".to_owned(),
                        });
                    }
                    std::thread::sleep(std::time::Duration::from_millis(PROCESS_POLL_MILLIS));
                }
                Err(error) => {
                    return Err(ControlError::StopFailed {
                        child: args.last().copied().unwrap_or("").to_owned(),
                        detail: format!("waiting failed: {error}"),
                    });
                }
            }
        };
        let mut stdout = String::new();
        if let Some(mut pipe) = child.stdout.take() {
            use std::io::Read;
            let _ = pipe.read_to_string(&mut stdout);
        }
        // RC3 correction (task26-dcritic-apparatus-repair): the real
        // binary prints its failure ("Error: Unknown active session:
        // <id>") to STDERR, not stdout — a detail built from stdout alone
        // could never contain it, so the idempotency guard below could
        // never fire. Both streams are captured and both land in the
        // detail.
        let mut stderr = String::new();
        if let Some(mut pipe) = child.stderr.take() {
            use std::io::Read;
            let _ = pipe.read_to_string(&mut stderr);
        }
        if !status.success() {
            return Err(ControlError::StopFailed {
                child: args.last().copied().unwrap_or("").to_owned(),
                detail: format!(
                    "{args:?} exited with {status}: stdout={stdout:?} stderr={stderr:?}"
                ),
            });
        }
        Ok(stdout)
    }
}

impl HarnessControl for ProcessHarnessControl {
    fn stop(&self, child: &str, deadline: Deadline) -> Result<(), ControlError> {
        match self.run_bounded(&["stop", child], deadline) {
            Ok(_output) => Ok(()),
            // Already gone: cancellation's goal is achieved (idempotent).
            // "Unknown active session" is the RC3 addition
            // (task26-dcritic-apparatus-repair): prime-agent 0.8.0's rlm
            // children are never daemon-registered sessions, so its
            // real `stop` on one always answers this way, on stderr,
            // exit 1 — verified by execution (`prime-agent stop <child>`
            // against a real daemon while a child runs).
            Err(ControlError::StopFailed { child, detail })
                if detail.contains("No such")
                    || detail.contains("not found")
                    || detail.contains("Unknown active session") =>
            {
                Err(ControlError::NoSuchChild { child })
            }
            Err(other) => Err(other),
        }
    }

    fn list(&self, deadline: Deadline) -> Result<Vec<SubagentRecord>, ControlError> {
        let text = self
            .run_bounded(&["list", "--json"], deadline)
            .map_err(|error| ControlError::ListFailed {
                detail: error.to_string(),
            })?;
        parse_listing(&text)
    }
}

/// Strictly parses the `list --json` document (COR-D2 divergence #2:
/// the harness returns a top-level OBJECT wrapping the array —
/// `{"sessions": [...]}` — not a bare array; verified by execution).
/// EXACTLY that shape is accepted: a missing, wrong-shaped, or
/// extra-keyed wrapper is a typed error, never coerced. Entry parsing
/// keeps the documented forensic residual: the three recorded fields
/// must be present strings; extra upstream fields are ignored (this
/// document feeds no control state).
pub fn parse_listing(text: &str) -> Result<Vec<SubagentRecord>, ControlError> {
    let document = json::parse(text).map_err(|error| ControlError::ListFailed {
        detail: format!("the listing is not valid JSON: {error}"),
    })?;
    let object = match &document {
        JsonValue::Object(_) => &document,
        _ => {
            return Err(ControlError::ListFailed {
                detail: "the listing is not a JSON object — the harness's `list --json` is \
                         {\"sessions\": [...]}"
                    .to_owned(),
            });
        }
    };
    for key in object.keys() {
        if key != "sessions" {
            return Err(ControlError::ListFailed {
                detail: format!(
                    "the listing carries unexpected key {key:?} — exactly {{\"sessions\": \
                     [...]}} is accepted"
                ),
            });
        }
    }
    let items = match object.get("sessions") {
        Some(JsonValue::Array(items)) => items,
        Some(_) => {
            return Err(ControlError::ListFailed {
                detail: "the listing's `sessions` field is not a JSON array".to_owned(),
            });
        }
        None => {
            return Err(ControlError::ListFailed {
                detail: "the listing carries no `sessions` array".to_owned(),
            });
        }
    };
    let mut records = Vec::new();
    for item in items {
        let string_field = |name: &str| -> Option<String> {
            match item.get(name) {
                Some(JsonValue::Str(value)) => Some(value.clone()),
                _ => None,
            }
        };
        let record = SubagentRecord {
            session_name: string_field("session_name").ok_or(ControlError::ListFailed {
                detail: "a listing entry carries no session_name".to_owned(),
            })?,
            status: string_field("status").unwrap_or_default(),
            session_dir: string_field("session_dir").unwrap_or_default(),
        };
        records.push(record);
    }
    Ok(records)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tiny throwaway shell script standing in for the real binary: it
    /// answers any invocation with the exact failure shape prime-agent
    /// 0.8.0 gives for a `stop` on a non-daemon-registered rlm child —
    /// `Error: Unknown active session: <id>` on STDERR, exit 1 (task26
    /// real-run evidence). Returns the script path; caller cleans up its
    /// parent scratch dir.
    fn fake_unknown_session_binary(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("gauntlet-prime-rc3-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("the rc3 scratch is creatable");
        let script = dir.join("fake-prime-agent.sh");
        std::fs::write(
            &script,
            "#!/bin/sh\necho 'Error: Unknown active session: sub-x' 1>&2\nexit 1\n",
        )
        .expect("the fake binary script is writable");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&script)
                .expect("the fake binary has metadata")
                .permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&script, perms).expect("the fake binary is executable");
        }
        script
    }

    fn far_future_deadline() -> Deadline {
        Deadline::at_epoch(gauntlet_domain::EpochSeconds::new(u64::MAX / 2))
    }

    /// RC3, first half (task26-dcritic-apparatus-repair): `run_bounded`
    /// captures STDERR into the `StopFailed` detail — before this fix the
    /// detail was built from stdout alone, and the real binary's failure
    /// text lands on stderr, so the idempotency guard could never see it.
    #[test]
    fn stderr_reaches_the_stop_failed_detail() {
        let binary = fake_unknown_session_binary("stderr-detail");
        let control = ProcessHarnessControl::new(binary.clone());
        let error = control
            .run_bounded(&["stop", "sub-x"], far_future_deadline())
            .expect_err("the fake binary always fails");
        let ControlError::StopFailed { detail, .. } = &error else {
            panic!("expected StopFailed, got {error:?}")
        };
        assert!(
            detail.contains("Unknown active session: sub-x"),
            "the stderr text must reach the detail: {detail}"
        );
        std::fs::remove_dir_all(binary.parent().expect("has a parent")).ok();
    }

    /// RC3, second half: `stop` recognizes `Unknown active session` as
    /// `NoSuchChild` — cancellation's goal (the child is gone) is achieved
    /// even though the harness never registered it as a daemon session in
    /// the first place (prime-agent 0.8.0's rlm children never are).
    #[test]
    fn unknown_active_session_maps_to_no_such_child() {
        let binary = fake_unknown_session_binary("no-such-child");
        let control = ProcessHarnessControl::new(binary.clone());
        let error = control
            .stop("sub-x", far_future_deadline())
            .expect_err("the fake binary always fails");
        assert!(
            matches!(error, ControlError::NoSuchChild { ref child } if child == "sub-x"),
            "got {error:?}"
        );
        std::fs::remove_dir_all(binary.parent().expect("has a parent")).ok();
    }

    /// THE PIN (COR-D2 divergence #2): the harness's `list --json`
    /// returns a top-level OBJECT wrapping the array — exactly
    /// `{"sessions": [...]}` is accepted (verified by execution).
    #[test]
    fn the_sessions_wrapper_is_the_accepted_listing_shape() {
        let records = parse_listing(
            r#"{"sessions": [{"session_name": "probe-child-6", "status": "completed",
             "session_dir": "/sessions/sub-76db0de9", "active_session_id": "abc"}]}"#,
        )
        .expect("the harness shape parses");
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].session_name, "probe-child-6");
        assert_eq!(records[0].status, "completed");
        // Extra upstream fields are the documented forensic residual:
        // ignored, never admitted anywhere.
    }

    /// A bare array — the shape this parser originally expected — is
    /// REFUSED (typed error, never coerced into an empty success).
    #[test]
    fn a_bare_array_listing_is_refused() {
        let error = parse_listing(r#"[{"session_name": "x"}]"#)
            .expect_err("a bare array is not the harness shape");
        assert!(error.to_string().contains("not a JSON object"), "{error}");
    }

    /// A missing or wrong-shaped `sessions` field is a typed error.
    #[test]
    fn missing_or_wrong_shaped_sessions_are_refused() {
        for hostile in [r#"{}"#, r#"{"sessions": 3}"#, r#"{"children": []}"#] {
            let error = parse_listing(hostile)
                .expect_err("a missing/wrong-shaped sessions field must be refused");
            assert!(
                error.to_string().contains("sessions"),
                "{hostile} -> {error}"
            );
        }
    }

    /// Invalid JSON and entries without a session_name are typed errors.
    #[test]
    fn malformed_listings_are_typed_errors() {
        assert!(parse_listing("not json").is_err());
        let error = parse_listing(r#"{"sessions": [{"status": "running"}]}"#)
            .expect_err("an entry without session_name is refused");
        assert!(error.to_string().contains("session_name"), "{error}");
    }

    /// An empty sessions array is the honest empty listing.
    #[test]
    fn an_empty_sessions_array_is_an_empty_listing() {
        let records = parse_listing(r#"{"sessions": []}"#).expect("empty parses");
        assert!(records.is_empty());
    }
}

#[cfg(test)]
mod handle_decode_teeth {
    //! COR-D3/F1 teeth: the strict admission-handle decode is
    //! untrusting end to end. The falsifier's mutation M2 (accepting
    //! unknown handle fields) passed the entire battery while admitting
    //! an answer-shaped extra field on the model-written handle
    //! document — these tests are the committed teeth that must fail if
    //! the strictness regresses. Classes follow the falsifier's A5 list
    //! (16 hostiles): 1 unknown field, 4 missing-field, 2 empty-string,
    //! 3 wrong-typed, 6 wrong-shape.

    use super::*;
    use std::path::PathBuf;

    fn valid_document() -> String {
        r#"{"rlm_child_id":"sub-1","name":"probe","session_dir":"/sessions/s1","model":"m"}"#
            .to_owned()
    }

    /// The one valid shape parses (the control that keeps the refusals
    /// honest — a parser that refused EVERYTHING would pass the loop
    /// below).
    #[test]
    fn the_valid_handle_document_parses() {
        let handle = AdmissionHandle::parse(&valid_document()).expect("valid parses");
        assert_eq!(handle.rlm_child_id, "sub-1");
        assert_eq!(handle.session_dir, "/sessions/s1");
        assert_eq!(handle.model, "m");
    }

    /// Every hostile class is refused WHOLE with the typed
    /// `HandleUnparsable` — never partially parsed, never coerced.
    #[test]
    fn every_hostile_handle_class_is_refused() {
        let hostiles: Vec<(&str, String)> = vec![
            // 1 unknown field — ANSWER-SHAPED: a child answer smuggled
            // through the handle document itself.
            (
                "unknown answer-shaped field",
                r#"{"rlm_child_id":"sub-1","name":"probe","session_dir":"/s","model":"m","answer":"the result is 42"}"#
                    .to_owned(),
            ),
            // 4 missing-field (one per schema field).
            (
                "missing rlm_child_id",
                r#"{"name":"probe","session_dir":"/s","model":"m"}"#.to_owned(),
            ),
            (
                "missing name",
                r#"{"rlm_child_id":"sub-1","session_dir":"/s","model":"m"}"#.to_owned(),
            ),
            (
                "missing session_dir",
                r#"{"rlm_child_id":"sub-1","name":"probe","model":"m"}"#.to_owned(),
            ),
            (
                "missing model",
                r#"{"rlm_child_id":"sub-1","name":"probe","session_dir":"/s"}"#.to_owned(),
            ),
            // 2 empty-string.
            (
                "empty rlm_child_id",
                r#"{"rlm_child_id":"","name":"probe","session_dir":"/s","model":"m"}"#.to_owned(),
            ),
            (
                "empty session_dir",
                r#"{"rlm_child_id":"sub-1","name":"probe","session_dir":"","model":"m"}"#
                    .to_owned(),
            ),
            // 3 wrong-typed values.
            (
                "number where a string goes",
                r#"{"rlm_child_id":7,"name":"probe","session_dir":"/s","model":"m"}"#.to_owned(),
            ),
            (
                "null value",
                r#"{"rlm_child_id":"sub-1","name":null,"session_dir":"/s","model":"m"}"#
                    .to_owned(),
            ),
            (
                "object value",
                r#"{"rlm_child_id":"sub-1","name":"probe","session_dir":{"path":"/s"},"model":"m"}"#
                    .to_owned(),
            ),
            // 6 wrong shapes (the document itself).
            ("top-level array", r#"["sub-1","probe","/s","m"]"#.to_owned()),
            ("bare string", r#""sub-1""#.to_owned()),
            ("bare number", "42".to_owned()),
            ("bare null", "null".to_owned()),
            ("empty object", "{}".to_owned()),
            ("non-JSON", "not json at all {{{".to_owned()),
        ];
        assert_eq!(hostiles.len(), 16, "the A5 class inventory is pinned");
        for (class, document) in &hostiles {
            let error =
                AdmissionHandle::parse(document).expect_err(&format!("{class} must be refused"));
            assert!(
                matches!(error, TransportError::HandleUnparsable { .. }),
                "{class} must be HandleUnparsable, got {error:?}"
            );
        }
    }

    /// The answer-shaped unknown field's refusal NAMES the field: the
    /// locus is diagnosable, and a silent-ignore regression (M2) fails
    /// here by construction.
    #[test]
    fn the_answer_shaped_unknown_field_is_named_in_the_refusal() {
        let document = r#"{"rlm_child_id":"sub-1","name":"probe","session_dir":"/s","model":"m","answer":"the result is 42"}"#;
        let error = AdmissionHandle::parse(document).expect_err("unknown field refused");
        let TransportError::HandleUnparsable { detail } = &error else {
            panic!("expected HandleUnparsable, got {error:?}")
        };
        assert!(detail.contains("answer"), "{detail}");
    }

    /// The same strictness through the FILE path (`read_admission`) the
    /// production transport and the suite doubles share.
    #[test]
    fn read_admission_refuses_a_hostile_file() {
        let dir = std::env::temp_dir().join(format!(
            "gauntlet-prime-handle-teeth-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).expect("teeth scratch is creatable");
        let path = dir.join("handle.json");
        std::fs::write(
            &path,
            r#"{"rlm_child_id":"sub-1","name":"probe","session_dir":"/s","model":"m","answer":"the result is 42"}"#,
        )
        .expect("the hostile handle document is writable");
        let error = read_admission(&path).expect_err("the file parse refuses");
        assert!(matches!(error, TransportError::HandleUnparsable { .. }));
        std::fs::remove_dir_all(&dir).ok();
        let _ = PathBuf::new();
    }
}
