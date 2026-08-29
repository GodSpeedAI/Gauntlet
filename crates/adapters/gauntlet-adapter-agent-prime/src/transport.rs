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
/// result document (the type cannot carry one; see the module docs).
pub trait KernelTransport: fmt::Debug {
    fn submit(
        &self,
        submission: &KernelSubmission,
        deadline: Deadline,
    ) -> Result<AdmissionHandle, TransportError>;
}

/// Shared ownership for transports whose forensic surface (e.g. the
/// live probe's D1 evidence via [`ProcessKernelTransport::take_forensics`])
/// must outlive construction of the runner.
impl<T: KernelTransport + ?Sized> KernelTransport for std::sync::Arc<T> {
    fn submit(
        &self,
        submission: &KernelSubmission,
        deadline: Deadline,
    ) -> Result<AdmissionHandle, TransportError> {
        (**self).submit(submission, deadline)
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
/// the SUCCESS path the disposition is a DETACHED bounded reaper
/// (COR-D2 divergence #4: a child of a print-mode parent materializes
/// ONLY while the parent stays alive past admission, so the parent must
/// NOT be stopped at parse — the reaper waits the linger + grace, then
/// stops it if still alive), and on the failure paths the bounded stop
/// is immediate. Every step's outcome is recorded as forensics. A parent
/// model that rambles after the cell — or waits on its child — cannot
/// stretch admission, and invoke can never collapse admission and
/// completion into one blocking call (spec §6.1: "invoke MUST return
/// promptly after admission"; AK-009).
///
/// The parent run's stdout/stderr are piped and DISCARDED beyond this
/// module: they are never a result source (the arch await-ban scan pins
/// that no result-decoding call exists outside the runner's poll/collect).
#[derive(Debug)]
pub struct ProcessKernelTransport {
    binary: PathBuf,
    admission_timeout: std::time::Duration,
    /// The in-cell linger the rendered cell carries (COR-D2 #4): the
    /// reaper waits at least this long past admission before stopping
    /// the parent.
    admission_linger: std::time::Duration,
    /// The last submission's forensic note (D1 split evidence: the
    /// parent's try_wait state at handle parse, the reaper's outcome). A
    /// mutex under an Arc: the transport is shared infrastructure AND
    /// the detached reaper thread appends its outcome after invoke has
    /// returned.
    forensics: std::sync::Arc<std::sync::Mutex<Option<String>>>,
}

impl ProcessKernelTransport {
    pub fn new(
        binary: PathBuf,
        admission_timeout: std::time::Duration,
        admission_linger: std::time::Duration,
    ) -> Self {
        Self {
            binary,
            admission_timeout,
            admission_linger,
            forensics: std::sync::Arc::new(std::sync::Mutex::new(None)),
        }
    }

    /// The last submission's forensic note, if any (D1 evidence leg for
    /// the live probe). The detached reaper may append its outcome AFTER
    /// this take — the note is a forensic stream, not a snapshot.
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

    /// Bounded disposition of the parent run: wait `linger` for a natural
    /// exit (a zero linger skips straight to the stop — the failure-path
    /// behavior), then a best-effort GRACEFUL stop: SIGTERM through the
    /// platform `kill` utility when available (unix), a short bounded
    /// grace, then the last-resort [`std::process::Child::kill`]. Every
    /// step's outcome lands in the returned forensic text — the stop is
    /// NEVER allowed to block its caller beyond its bound. Runs on the
    /// detached reaper thread (success path) or inline (failure paths).
    fn stop_bounded(child: &mut std::process::Child, linger: std::time::Duration) -> String {
        let mut note = String::new();
        // The linger wait: a natural exit during the linger is the
        // expected shape on the success path (the parent's own cell ends
        // after its linger).
        let linger_ends = Self::now_secs().saturating_add(linger.as_secs());
        while child.try_wait().map_or(true, |exited| exited.is_none()) {
            if Self::now_secs() >= linger_ends {
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
        // unconditional on the grace expiring (a print run is disposable;
        // its daemon-backed child is what survives, per the harness docs).
        let grace_ends = Self::now_secs().saturating_add(STOP_GRACE_SECS);
        loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    note.push_str(&format!("parent exited ({status})"));
                    return note;
                }
                Ok(None) => {
                    if Self::now_secs() >= grace_ends {
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
}

impl KernelTransport for ProcessKernelTransport {
    fn submit(
        &self,
        submission: &KernelSubmission,
        deadline: Deadline,
    ) -> Result<AdmissionHandle, TransportError> {
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
        // COR-D2 divergence #4 (load-bearing): a child of a print-mode
        // parent materializes ONLY while the parent stays alive past
        // admission, so on the SUCCESS path the parent is NOT stopped at
        // parse — a DETACHED bounded reaper (moved Child; shared
        // forensics) waits the linger + grace and stops it only if still
        // alive. Failure paths keep the IMMEDIATE bounded stop (nothing
        // was admitted that needs a living parent; the orphan sweep
        // handles any stray child). invoke NEVER waits for any of this
        // (RES-D1 untouched).
        if outcome.is_ok() {
            let at_parse = match child.try_wait() {
                Ok(None) => "NotExited",
                Ok(Some(_)) => "Exited",
                Err(_) => "Unknown",
            };
            note.push_str(&format!(
                "handle parsed while parent.try_wait()={at_parse}; reaper dispatched (linger \
                 {}s); .",
                self.admission_linger.as_secs()
            ));
            *self.forensics.lock().expect("the forensics mutex") = Some(note);
            let forensics = std::sync::Arc::clone(&self.forensics);
            let linger = self.admission_linger;
            std::thread::spawn(move || {
                let outcome = Self::stop_bounded(&mut child, linger);
                if let Ok(mut guard) = forensics.lock() {
                    match guard.as_mut() {
                        Some(existing) => existing.push_str(&format!("reaper: {outcome}")),
                        None => *guard = Some(format!("reaper: {outcome}")),
                    }
                }
            });
        } else {
            note.push_str(&Self::stop_bounded(&mut child, std::time::Duration::ZERO));
            note.push('.');
            *self.forensics.lock().expect("the forensics mutex") = Some(note);
        }
        outcome
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
            .stderr(Stdio::null())
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
        if !status.success() {
            return Err(ControlError::StopFailed {
                child: args.last().copied().unwrap_or("").to_owned(),
                detail: format!("{args:?} exited with {status}"),
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
            Err(ControlError::StopFailed { child, detail })
                if detail.contains("No such") || detail.contains("not found") =>
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
