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
//! **Parent process ownership (task26-dcritic-apparatus-repair).**
//! [`KernelTransport::submit`] returns the admission handle TOGETHER WITH a
//! [`ParentHandle`]: the live parent process, handed to the caller so the
//! INVOCATION owns its lifetime, not the transport. An earlier posture
//! here — "a print run is disposable; its daemon-backed child is what
//! survives" — was FALSE for prime-agent 0.8.0 (verified by execution,
//! task26 real-run evidence): a child of a `--print` parent materializes
//! and keeps running ONLY while the parent session stays alive, and
//! `--print`'s own system prompt forbids the keep-alive loop that
//! requires ("start the work, record its handle or output location, then
//! end your turn … Do not keep the turn open by polling with
//! `time.sleep()`") — a parent model shown that loop-bearing cell refused
//! to run it, and nothing was ever admitted.
//!
//! **rpc-mode correction (supersedes `--print`).** The parent is now
//! spawned `<binary> --mode rpc`: it reads JSON commands on stdin and
//! stays alive until stdin closes (then waits for idle work and exits),
//! independent of what any one rendered cell does. `submit` writes ONE
//! `{"type":"prompt",...}` line and keeps stdin OPEN — the returned
//! [`ParentHandle`] owns that pipe, so closing it (on `terminate`/`Drop`)
//! is what ends the parent's turn. A background reader thread parses the
//! newline-delimited JSON events the harness prints on stdout (parent
//! usage, tool-execution counts, per-child telemetry) into a shared
//! [`ParentTelemetry`] the runner reads for `payment_so_far` and the
//! measured payment at collection (see `runner.rs`).
//!
//! Both seams have ONE production implementation apiece, shelling out to
//! the configured binary through `std::process` — no external dependency
//! — and in-memory doubles live in the adapter's own test targets (the
//! shared contract suite runs against those, never against a harness).

use core::fmt;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use gauntlet_domain::MicroUsd;
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

/// One harness child's latest telemetry, as the rpc-mode parent's
/// `rlm_child_update` stdout events report it (task26-dcritic-apparatus-
/// repair W1). The event is emitted repeatedly per child; only the LATEST
/// record per child id is kept.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChildTelemetry {
    pub status: String,
    pub token_count: u64,
    pub tool_use_count: u64,
    pub duration_ms: u64,
    pub session_dir: Option<String>,
}

/// The rpc-mode parent's live telemetry, accumulated by
/// [`ProcessKernelTransport`]'s stdout reader thread from the harness's
/// newline-delimited JSON events (task26-dcritic-apparatus-repair W1):
/// the PARENT's own usage (summed over every `message_end` whose message
/// `role` is `assistant` — a user-role `message_end` carries no
/// meaningful usage and is skipped), its tool-execution count, and the
/// LATEST per-child record from `rlm_child_update`. FORENSIC/METERING
/// DATA ONLY: nothing here decides `Completed`/`Failed` (that is still
/// the result contract alone, Annex C.2/C.4) — the runner reads it only
/// for `payment_so_far` and the measured payment installed at `collect`.
///
/// A plain public struct with a zero [`Default`] (`NullParentHandle`
/// reports exactly this): every reader gets an owned snapshot via
/// [`ParentHandle::telemetry`], never a lock held across a call.
#[derive(Debug, Clone, PartialEq)]
pub struct ParentTelemetry {
    /// `Some(true)`/`Some(false)` once the admission prompt's `response`
    /// event lands; `None` while still awaited. `Some(false)` carries its
    /// text in `prompt_error`.
    pub prompt_accepted: Option<bool>,
    pub prompt_error: Option<String>,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
    pub total_tokens: u64,
    pub cost_usd: MicroUsd,
    pub tool_execution_count: u64,
    pub children: HashMap<String, ChildTelemetry>,
    /// The wall-clock epoch second of the last stdout event the reader
    /// thread parsed (any type, including `rlm_child_update`) — the
    /// runner's `Running.last_event_at` floor, so a long-running child
    /// that is still emitting telemetry is never mistaken for a stalled
    /// invocation (coordinator addendum, task26-dcritic-apparatus-repair).
    pub last_event_epoch: u64,
    /// The count of `agent_end` events observed (coordinator review
    /// finding, task26-dcritic-apparatus-repair): under rpc mode a parent
    /// that REFUSES the cell does not exit — its turn just ends and it
    /// idles — so `agent_end` with no handle file is the refusal signal
    /// the handle watch needs; a real admission never emits `agent_end`
    /// before the handle exists (see `a_refusal_ends_admission_promptly`).
    pub agent_end_count: u64,
    /// The last assistant `message_end`'s text content, concatenated from
    /// every `content[]` item with `type:"text"`, truncated to 2000 bytes
    /// on a char boundary — carried into the refusal detail so the
    /// operator sees WHY the parent refused, not just that it did.
    pub last_assistant_text: Option<String>,
}

/// The last-assistant-text cap, in bytes (truncated on a char boundary).
const LAST_ASSISTANT_TEXT_CAP_BYTES: usize = 2000;

impl Default for ParentTelemetry {
    fn default() -> Self {
        Self {
            prompt_accepted: None,
            prompt_error: None,
            input_tokens: 0,
            output_tokens: 0,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
            total_tokens: 0,
            cost_usd: MicroUsd::new(0),
            tool_execution_count: 0,
            children: HashMap::new(),
            last_event_epoch: 0,
            agent_end_count: 0,
            last_assistant_text: None,
        }
    }
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

    /// Terminates the parent (stdin EOF first for an rpc-mode process, a
    /// bounded grace, then a last-resort SIGTERM/kill) and returns a
    /// forensic note describing the disposition. Idempotent: calling it
    /// again after the parent is already gone is a cheap no-op that still
    /// returns a note.
    fn terminate(&mut self) -> String;

    /// A snapshot of the parent's accumulated telemetry (task26-dcritic-
    /// apparatus-repair W1) — zeroed/`None` fields for a double that
    /// models no real process. FORENSIC/METERING DATA ONLY (see
    /// [`ParentTelemetry`]'s docs).
    fn telemetry(&self) -> ParentTelemetry;
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

    fn telemetry(&self) -> ParentTelemetry {
        ParentTelemetry::default()
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

/// The grace an rpc-mode parent gets, after its stdin closes (EOF), to
/// notice and exit cleanly before falling back to the SIGTERM/kill ladder
/// (task26-dcritic-apparatus-repair: `--mode rpc` waits for idle work and
/// then exits 0 on stdin EOF — this is the bounded wait for that clean
/// exit, not a substitute for it).
const RPC_EOF_GRACE_SECS: u64 = 3;

/// The grace the handle watch gives a genuine admission whose atomic
/// handle write is still in flight when `agent_end` fires, before
/// concluding the parent refused the cell (coordinator review finding,
/// task26-dcritic-apparatus-repair) — measured from the FIRST observed
/// `agent_end`, not from admission start.
const AGENT_END_REFUSAL_GRACE_SECS: u64 = 3;

/// The maximum single stdout line the rpc-mode event reader will attempt
/// to parse; longer lines are skipped whole rather than grown without
/// bound (a hostile or malfunctioning harness must not exhaust memory).
const RPC_STDOUT_LINE_CAP_BYTES: usize = 4 * 1024 * 1024;

/// The bounded buffer the stderr-drain thread accumulates for forensics;
/// stderr is never a result or telemetry source, only diagnostic text.
const RPC_STDERR_BUFFER_CAP_BYTES: usize = 64 * 1024;

/// The fixed id of the one prompt line `submit` writes — stable so a
/// fixture or a future multi-command protocol can recognize it.
const RPC_PROMPT_ID: &str = "gauntlet-admit";

/// The production kernel transport: it receives the rendered submission,
/// starts ONE `<binary> --mode rpc` process over the configured binary,
/// writes ONE prompt line carrying the program (delimited, verbatim) on
/// its stdin, and then — this is the load-bearing shape (correction
/// RES-D1) — watches for the HANDLE FILE under a bounded deadline.
/// `invoke` returns the moment the handle exists and strict-parses; the
/// parent process's LIFETIME is NOT admission. On the SUCCESS path the
/// still-live parent [`std::process::Child`], its still-OPEN stdin pipe,
/// and the shared telemetry sink are wrapped as a [`RealParentHandle`]
/// and handed back to the CALLER (task26-dcritic-apparatus-repair): the
/// transport never disposes of a successfully-admitted parent itself —
/// the runner owns its lifetime for as long as the invocation lives (see
/// the module docs). On the failure paths nothing was admitted, so stdin
/// is closed and the bounded stop runs immediately and inline. Every
/// step's outcome is recorded as forensics. `invoke` can never collapse
/// admission and completion into one blocking call (spec §6.1: "invoke
/// MUST return promptly after admission"; AK-009).
///
/// The parent's stdout is piped to a background reader thread that
/// parses the harness's newline-delimited JSON events into a shared
/// [`ParentTelemetry`] — metering/forensics only, never a result source
/// (the arch await-ban scan pins that no result-decoding call exists
/// outside the runner's poll/collect). Its stderr is drained into a
/// bounded buffer by a second thread so the pipe can never block the
/// parent; that buffer is diagnostic text only.
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

    /// The parent's message: instruction + the program, delimited. The
    /// rpc-mode correction (task26-dcritic-apparatus-repair) drops the
    /// old keep-alive justification entirely — the parent PROCESS now
    /// outlives this one turn on its own (it stays alive until its stdin
    /// closes), so the instruction is exactly what prime-agent's own
    /// system prompt asks for: start the work, record the handle, end the
    /// turn.
    fn parent_message(program: &str) -> String {
        format!(
            "Run the following Python program VERBATIM as a single IPython cell, exactly \
             once. Do not modify it, do not summarize it, do not answer its prompt \
             yourself. It starts delegated work and records that delegation's admission \
             handle to a file. Once the cell returns, end your turn. If a delegated \
             worker later sends you a message, take no further action — end your turn \
             again.\n\
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

/// Closes the parent's stdin (an rpc-mode process reads EOF as "no more
/// commands", waits for idle work, and exits 0) and gives it
/// [`RPC_EOF_GRACE_SECS`] to do exactly that before falling back to the
/// SIGTERM/kill ladder ([`stop_bounded`], zero linger — nothing left to
/// linger for once stdin is already closed). The ONE shutdown routine,
/// shared by `submit`'s failure path and [`RealParentHandle::terminate`].
fn close_then_stop(
    child: &mut std::process::Child,
    stdin: Option<std::process::ChildStdin>,
) -> String {
    let mut note = String::new();
    if stdin.is_some() {
        note.push_str("stdin closed (rpc EOF); ");
    }
    drop(stdin); // closing stdin is the signal; nothing more to write.
    let grace_ends = ProcessKernelTransport::now_secs().saturating_add(RPC_EOF_GRACE_SECS);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                note.push_str(&format!("parent exited after stdin EOF ({status})"));
                return note;
            }
            Ok(None) => {
                if ProcessKernelTransport::now_secs() >= grace_ends {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(PROCESS_POLL_MILLIS));
            }
            Err(error) => {
                note.push_str(&format!("waiting after stdin EOF failed: {error}; "));
                break;
            }
        }
    }
    note.push_str(&stop_bounded(child, std::time::Duration::ZERO));
    note
}

/// The value at `field` inside a JSON string field, if present and typed.
pub(crate) fn json_str<'a>(value: &'a JsonValue, field: &str) -> Option<&'a str> {
    match value.get(field) {
        Some(JsonValue::Str(text)) => Some(text.as_str()),
        _ => None,
    }
}

/// The value at `field` as a non-negative integer, if present and typed
/// (a non-negative float is accepted and rounded — some upstream counters
/// may be emitted as floats).
pub(crate) fn json_u64(value: &JsonValue, field: &str) -> Option<u64> {
    match value.get(field) {
        Some(JsonValue::Number(number)) => match number.as_u64() {
            Some(exact) => Some(exact),
            None => {
                let approx = number.as_f64();
                (approx >= 0.0).then_some(approx.round() as u64)
            }
        },
        _ => None,
    }
}

/// The value at `field` as a finite `f64`, if present and typed.
pub(crate) fn json_f64(value: &JsonValue, field: &str) -> Option<f64> {
    match value.get(field) {
        Some(JsonValue::Number(number)) => Some(number.as_f64()),
        _ => None,
    }
}

/// Truncates `text` to at most `cap` bytes, backing off to the nearest
/// preceding char boundary so the result is always valid UTF-8 (never
/// splits a multi-byte character).
fn truncate_on_char_boundary(mut text: String, cap: usize) -> String {
    if text.len() <= cap {
        return text;
    }
    let mut end = cap;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    text.truncate(end);
    text
}

/// One rpc-mode stdout event line, applied to the shared telemetry
/// (task26-dcritic-apparatus-repair W1). Unrecognized event types and
/// unparseable/oversized lines are silently skipped — telemetry is
/// best-effort metering, never a state input (Annex C.2/C.4).
fn apply_rpc_event(telemetry: &Mutex<ParentTelemetry>, document: &JsonValue) {
    let Some(event_type) = json_str(document, "type") else {
        return;
    };
    let now = ProcessKernelTransport::now_secs();
    let mut state = telemetry
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    // Every recognized event — including `rlm_child_update` — advances
    // the last-event floor: a long-running child that is still emitting
    // telemetry must never read as a stalled parent (coordinator
    // addendum, task26-dcritic-apparatus-repair).
    state.last_event_epoch = state.last_event_epoch.max(now);
    match event_type {
        "response" => {
            if document.get("command").and_then(|value| match value {
                JsonValue::Str(command) if command == "prompt" => Some(()),
                _ => None,
            }) != Some(())
            {
                return;
            }
            let Some(JsonValue::Bool(success)) = document.get("success") else {
                return;
            };
            state.prompt_accepted = Some(*success);
            if !*success {
                state.prompt_error = Some(
                    json_str(document, "error")
                        .unwrap_or("the parent refused the prompt")
                        .to_owned(),
                );
            }
        }
        "message_end" => {
            let Some(message) = document.get("message") else {
                return;
            };
            if json_str(message, "role") != Some("assistant") {
                // A user-role message_end carries no meaningful usage.
                return;
            }
            // The text content is captured independent of `usage` — a
            // refusal message_end may carry text without meaningful usage
            // (coordinator review finding, task26-dcritic-apparatus-repair).
            if let Some(JsonValue::Array(items)) = message.get("content") {
                let mut text = String::new();
                for item in items {
                    if json_str(item, "type") == Some("text")
                        && let Some(fragment) = json_str(item, "text")
                    {
                        text.push_str(fragment);
                    }
                }
                if !text.is_empty() {
                    state.last_assistant_text = Some(truncate_on_char_boundary(
                        text,
                        LAST_ASSISTANT_TEXT_CAP_BYTES,
                    ));
                }
            }
            let Some(usage) = message.get("usage") else {
                return;
            };
            state.input_tokens += json_u64(usage, "input").unwrap_or(0);
            state.output_tokens += json_u64(usage, "output").unwrap_or(0);
            state.cache_read_tokens += json_u64(usage, "cacheRead").unwrap_or(0);
            state.cache_write_tokens += json_u64(usage, "cacheWrite").unwrap_or(0);
            state.total_tokens += json_u64(usage, "totalTokens").unwrap_or(0);
            if let Some(cost) = usage.get("cost")
                && let Some(total) = json_f64(cost, "total")
                && total.is_finite()
                && total >= 0.0
            {
                let micros = (total * 1_000_000.0).round();
                let micros = if micros >= u64::MAX as f64 {
                    u64::MAX
                } else {
                    micros as u64
                };
                state.cost_usd = MicroUsd::new(state.cost_usd.get().saturating_add(micros));
            }
        }
        "agent_end" => {
            state.agent_end_count = state.agent_end_count.saturating_add(1);
        }
        "tool_execution_start" => {
            state.tool_execution_count = state.tool_execution_count.saturating_add(1);
        }
        "rlm_child_update" => {
            let Some(child) = document.get("child") else {
                return;
            };
            let Some(id) = json_str(child, "id") else {
                return;
            };
            state.children.insert(
                id.to_owned(),
                ChildTelemetry {
                    status: json_str(child, "status").unwrap_or_default().to_owned(),
                    token_count: json_u64(child, "tokenCount").unwrap_or(0),
                    tool_use_count: json_u64(child, "toolUseCount").unwrap_or(0),
                    duration_ms: json_u64(child, "durationMs").unwrap_or(0),
                    session_dir: json_str(child, "sessionDir").map(str::to_owned),
                },
            );
        }
        _ => {}
    }
}

/// The background stdout reader: one JSON line per event, cap-then-skip
/// on oversized/unparsable lines, ends when the pipe closes (the parent
/// exited or its stdout was otherwise closed).
fn read_rpc_events(stdout: std::process::ChildStdout, telemetry: Arc<Mutex<ParentTelemetry>>) {
    let reader = BufReader::new(stdout);
    for line in reader.lines() {
        let Ok(line) = line else { break };
        if line.len() > RPC_STDOUT_LINE_CAP_BYTES {
            continue;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Ok(document) = json::parse(trimmed) {
            apply_rpc_event(&telemetry, &document);
        }
    }
}

/// The background stderr drain: accumulates into a bounded buffer so the
/// pipe can never block the parent; forensics only.
fn drain_stderr(stderr: std::process::ChildStderr, buffer: Arc<Mutex<String>>) {
    let reader = BufReader::new(stderr);
    for line in reader.lines() {
        let Ok(line) = line else { break };
        let mut text = buffer.lock().unwrap_or_else(|poison| poison.into_inner());
        if text.len() < RPC_STDERR_BUFFER_CAP_BYTES {
            text.push_str(&line);
            text.push('\n');
        }
    }
}

/// The prompt-line JSON `submit` writes to the parent's stdin:
/// `{"id":…,"type":"prompt","message":…}`.
fn encode_prompt_line(id: &str, message: &str) -> String {
    let document = JsonValue::Object(vec![
        ("id".to_owned(), JsonValue::Str(id.to_owned())),
        ("type".to_owned(), JsonValue::Str("prompt".to_owned())),
        ("message".to_owned(), JsonValue::Str(message.to_owned())),
    ]);
    format!("{}\n", document.to_json_text())
}

/// The production [`ParentHandle`]: a live rpc-mode parent
/// [`std::process::Child`], its still-open stdin (closing it is the
/// shutdown signal), and the shared telemetry sink its reader thread
/// fills. `try_reap` is a cheap, non-blocking, ONE-SHOT `try_wait` —
/// forensics only, never a completion/failure signal. Once the process is
/// observed exited it is marked `reaped` so a second `try_wait` never
/// runs (the OS has already collected the child; calling it again is at
/// best a wasted syscall and at worst an error on some platforms).
///
/// **Drop**: if the runner (and this handle with it) is dropped while the
/// parent is still alive — e.g. an interrupted process, a panicking
/// caller — the parent is TERMINATED rather than leaked: `Drop` closes
/// stdin and calls the same bounded [`close_then_stop`] routine used
/// everywhere else.
#[derive(Debug)]
pub struct RealParentHandle {
    child: std::process::Child,
    stdin: Option<std::process::ChildStdin>,
    telemetry: Arc<Mutex<ParentTelemetry>>,
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
        let note = close_then_stop(&mut self.child, self.stdin.take());
        self.reaped = true;
        note
    }

    fn telemetry(&self) -> ParentTelemetry {
        self.telemetry
            .lock()
            .map(|guard| guard.clone())
            .unwrap_or_else(|poison| poison.into_inner().clone())
    }
}

impl Drop for RealParentHandle {
    fn drop(&mut self) {
        // Best-effort only: Drop cannot propagate a failure, and a
        // process already reaped (`self.reaped`) or already exited on
        // its own needs no action — prefer terminating a still-live
        // parent over leaking it (see the type docs).
        if !self.reaped && matches!(self.child.try_wait(), Ok(None)) {
            let _ = close_then_stop(&mut self.child, self.stdin.take());
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
            .arg("--mode")
            .arg("rpc")
            .current_dir(&submission.workspace)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| TransportError::SpawnFailed {
                binary: self.binary.clone(),
                detail: error.to_string(),
            })?;
        note.push_str(&format!("rpc parent pid {} started; ", child.id()));

        // Stderr is drained into a bounded buffer so the pipe can never
        // block the parent — forensics only, never a telemetry source.
        let stderr_buffer: Arc<Mutex<String>> = Arc::new(Mutex::new(String::new()));
        if let Some(stderr) = child.stderr.take() {
            let buffer = Arc::clone(&stderr_buffer);
            std::thread::spawn(move || drain_stderr(stderr, buffer));
        }
        // The shared telemetry sink; the reader thread fills it for as
        // long as stdout stays open (the life of the returned handle).
        let telemetry: Arc<Mutex<ParentTelemetry>> =
            Arc::new(Mutex::new(ParentTelemetry::default()));
        if let Some(stdout) = child.stdout.take() {
            let sink = Arc::clone(&telemetry);
            std::thread::spawn(move || read_rpc_events(stdout, sink));
        }

        // THE ONE PROMPT LINE. stdin STAYS OPEN past this write — closing
        // it later is what ends the parent's turn (rpc mode reads EOF as
        // "no more commands").
        let mut stdin = child.stdin.take();
        let message = Self::parent_message(&submission.program);
        let prompt_line = encode_prompt_line(RPC_PROMPT_ID, &message);
        let write_outcome = match stdin.as_mut() {
            Some(pipe) => pipe
                .write_all(prompt_line.as_bytes())
                .and_then(|()| pipe.flush())
                .map_err(|error| error.to_string()),
            None => Err("the parent's stdin pipe was not available".to_owned()),
        };
        if let Err(detail) = write_outcome {
            let stop_note = close_then_stop(&mut child, stdin.take());
            note.push_str(&format!(
                "writing the rpc prompt line failed: {detail}; {stop_note}."
            ));
            *self.forensics.lock().expect("the forensics mutex") = Some(note);
            return Err(TransportError::SpawnFailed {
                binary: self.binary.clone(),
                detail: format!("writing the rpc prompt line failed: {detail}"),
            });
        }

        // THE BOUNDED HANDLE WATCH (RES-D1): the deadline guards the
        // watch for HANDLE_PATH — never the parent process lifetime.
        // A parse failure while the parent is STILL RUNNING is treated as
        // a partial write (the program's write_text is not atomic) and
        // retried within the bound; a parse failure after the parent
        // ended is final. A prompt `response` with `success:false` ends
        // the watch immediately — the parent refused admission outright,
        // named by its error text.
        // The epoch at which `agent_end` was FIRST observed with no handle
        // yet on disk — the floor the refusal grace below measures from
        // (coordinator review finding, task26-dcritic-apparatus-repair).
        let mut agent_end_seen_at: Option<u64> = None;
        let outcome = loop {
            let parent_gone = matches!(child.try_wait(), Ok(Some(_)));
            if let Ok(state) = telemetry.lock()
                && state.prompt_accepted == Some(false)
            {
                let error_text = state
                    .prompt_error
                    .clone()
                    .unwrap_or_else(|| "the parent refused the prompt".to_owned());
                drop(state);
                note.push_str("the parent refused the prompt (success:false); ");
                break Err(TransportError::HandleAbsent {
                    detail: format!("the parent refused admission: {error_text}"),
                });
            }
            // Handle next: the earliest evidence wins.
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
            // A REFUSAL under rpc mode (coordinator review finding,
            // task26-dcritic-apparatus-repair): the parent does NOT exit
            // when the model refuses the cell — its turn just ends
            // (`agent_end`) and it idles — so without this check the
            // watch would run all the way to the admission timeout on a
            // refusal. Once `agent_end` has fired at least once AND no
            // handle file exists yet, a short grace (measured from the
            // FIRST observed `agent_end`) gives a genuine admission's
            // still-in-flight write a chance to land; past the grace the
            // watch ends as a refusal, carrying the parent's last
            // assistant text so the operator sees WHY. A handle file that
            // EXISTS but does not parse yet keeps the ordinary
            // partial-write tolerance above — this check only fires when
            // there is no handle file at all.
            if !submission.handle_path.is_file() {
                let agent_ended = telemetry
                    .lock()
                    .map(|state| state.agent_end_count >= 1)
                    .unwrap_or(false);
                if agent_ended {
                    let first_seen = *agent_end_seen_at.get_or_insert_with(Self::now_secs);
                    if Self::now_secs().saturating_sub(first_seen) >= AGENT_END_REFUSAL_GRACE_SECS {
                        let last_text = telemetry
                            .lock()
                            .ok()
                            .and_then(|state| state.last_assistant_text.clone());
                        note.push_str(
                            "the parent's turn ended (agent_end) without writing a handle; ",
                        );
                        break Err(TransportError::HandleAbsent {
                            detail: format!(
                                "the parent's turn ended without writing the admission handle \
                                 at {}{}",
                                submission.handle_path.display(),
                                last_text
                                    .map(|text| format!(" — last assistant text: {text:?}"))
                                    .unwrap_or_default()
                            ),
                        });
                    }
                }
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
        // try_wait state AT PARSE TIME, taken before any disposition. On
        // the SUCCESS path the still-live parent — stdin kept open, its
        // telemetry sink shared — is wrapped as a [`RealParentHandle`]
        // and handed back to the CALLER: the transport does not stop it,
        // does not detach a thread to stop it later, and does not decide
        // when its life ends. Failure paths keep the IMMEDIATE bounded
        // shutdown (nothing was admitted that needs a living parent; the
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
                    "handle parsed while parent.try_wait()={at_parse}; parent handle (rpc mode, \
                     stdin kept open) returned to the caller — the invocation now owns its \
                     lifetime."
                ));
                *self.forensics.lock().expect("the forensics mutex") = Some(note);
                Ok((
                    handle,
                    Box::new(RealParentHandle {
                        child,
                        stdin,
                        telemetry,
                        reaped: false,
                    }) as Box<dyn ParentHandle>,
                ))
            }
            Err(error) => {
                note.push_str(&close_then_stop(&mut child, stdin.take()));
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

    /// A unique scratch dir per test (pid + an atomic counter — the repo
    /// recently fixed parallel-test scratch collisions; this mirrors that
    /// fix rather than reintroducing the bug).
    fn rpc_scratch(tag: &str) -> PathBuf {
        use std::sync::atomic::{AtomicU32, Ordering};
        static SEQUENCE: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "gauntlet-prime-rpc-{tag}-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).expect("the rpc scratch is creatable");
        dir
    }

    /// A fake `--mode rpc` binary (python3, mirroring the real rpc
    /// protocol described in the W1 plan): reads the ONE prompt line on
    /// stdin, regex-extracts the handle path the embedded program writes
    /// to, emits a `response` (success unless `refuse` is set, in which
    /// case it carries an error and writes NO handle).
    ///
    /// If `cell_refusal` is set instead (coordinator review finding,
    /// task26-dcritic-apparatus-repair: a rpc-mode parent that refuses the
    /// CELL — as opposed to the prompt — does not exit; its turn just
    /// ends), the prompt is ACCEPTED, but the model then emits an
    /// assistant `message_end` carrying refusal text and an `agent_end`,
    /// and never writes a handle at all.
    ///
    /// Otherwise it emits an assistant `message_end` with usage, a
    /// `tool_execution_start`, and an `rlm_child_update` naming
    /// `child_session_dir`, writes the admission-handle document, and THEN
    /// an `agent_end` (proving a late `agent_end` never disturbs an
    /// already-admitted invocation) — and FINALLY blocks reading stdin
    /// until EOF before exiting 0 (the real rpc-mode shutdown shape:
    /// stdin EOF, not any in-cell behavior, ends the turn).
    #[cfg(unix)]
    fn fake_rpc_binary(
        dir: &Path,
        child_session_dir: &Path,
        refuse: bool,
        cell_refusal: bool,
    ) -> PathBuf {
        let script = dir.join("fake-prime-agent-rpc.py");
        let refuse_literal = if refuse { "True" } else { "False" };
        let cell_refusal_literal = if cell_refusal { "True" } else { "False" };
        let session_dir_raw = child_session_dir.to_string_lossy().into_owned();
        let session_dir_py_literal = {
            let mut escaped = String::from("\"");
            for ch in session_dir_raw.chars() {
                match ch {
                    '\\' => escaped.push_str("\\\\"),
                    '"' => escaped.push_str("\\\""),
                    other => escaped.push(other),
                }
            }
            escaped.push('"');
            escaped
        };
        let source = format!(
            r#"#!/usr/bin/env python3
import json, os, re, sys

REFUSE = {refuse_literal}
CELL_REFUSAL = {cell_refusal_literal}
SESSION_DIR = {session_dir_py_literal}

def emit(obj):
    sys.stdout.write(json.dumps(obj) + "\n")
    sys.stdout.flush()

line = sys.stdin.readline()
cmd = json.loads(line) if line.strip() else {{}}
message = cmd.get("message", "")
match = re.search(r"pathlib\.Path\('([^']+)'\)\.write_text\(json\.dumps\(\{{", message)
handle_path = match.group(1) if match else None

if REFUSE:
    emit({{"id": cmd.get("id", ""), "type": "response", "command": "prompt",
           "success": False, "error": "fake refusal: nonblocking-delegation policy"}})
elif CELL_REFUSAL:
    # Accepted at the prompt level, but the model refuses the CELL: its
    # turn just ends (agent_end) — the parent process itself does NOT
    # exit (rpc mode idles on stdin), and no handle is ever written.
    emit({{"id": cmd.get("id", ""), "type": "response", "command": "prompt", "success": True}})
    emit({{"type": "agent_start"}})
    emit({{"type": "message_end", "message": {{"role": "assistant", "content": [
        {{"type": "text", "text": "I can't run that"}}
    ]}}}})
    emit({{"type": "agent_end"}})
else:
    emit({{"id": cmd.get("id", ""), "type": "response", "command": "prompt", "success": True}})
    emit({{"type": "agent_start"}})
    emit({{"type": "message_end", "message": {{"role": "user", "usage": {{"totalTokens": 999999}}}}}})
    emit({{"type": "message_end", "message": {{"role": "assistant", "usage": {{
        "input": 100, "output": 50, "cacheRead": 10, "cacheWrite": 5,
        "totalTokens": 165,
        "cost": {{"input": 0.001, "output": 0.002, "cacheRead": 0.0001,
                  "cacheWrite": 0.0001, "total": 0.0032}}
    }}}}}})
    emit({{"type": "tool_execution_start", "toolCallId": "t1", "toolName": "ipython", "args": {{}}}})
    emit({{"type": "rlm_child_update", "child": {{
        "id": "sub-fake01", "sessionName": "fake-child", "model": "fake-model",
        "status": "running", "durationMs": 1234, "toolUseCount": 3,
        "tokenCount": 777, "sessionDir": SESSION_DIR
    }}}})
    if handle_path:
        doc = json.dumps({{
            "rlm_child_id": "sub-fake01", "name": "fake-invocation",
            "session_dir": SESSION_DIR, "model": "fake-model"
        }})
        tmp = handle_path + ".tmp"
        with open(tmp, "w") as fh:
            fh.write(doc)
        os.replace(tmp, handle_path)
    # A LATE agent_end — after the handle already landed — must never be
    # mistaken for a refusal (the happy-path regression this proves).
    emit({{"type": "agent_end"}})

# rpc mode: stay alive until stdin closes, then exit clean.
while True:
    remaining = sys.stdin.readline()
    if remaining == "":
        break
sys.exit(0)
"#
        );
        std::fs::write(&script, source).expect("the fake rpc binary is writable");
        crate::test_support::make_executable_ready(&script);
        // python3 interprets its own shebang; Command::new(&script) works
        // directly on unix (the kernel reads the `#!` line).
        script
    }

    fn far_future_deadline_rpc() -> Deadline {
        Deadline::at_epoch(gauntlet_domain::EpochSeconds::new(u64::MAX / 2))
    }

    /// The REAL renderer's output — the fake binary regexes the handle
    /// path out of exactly what `invoke` would actually send, never a
    /// hand-rolled stand-in.
    fn rpc_submission(dir: &Path) -> KernelSubmission {
        let handle_path = dir.join("result.handle");
        let program = crate::program::render(
            &crate::program::KernelProgramSpec {
                envelope_path: dir.join("envelope.json").to_string_lossy().into_owned(),
                child_input_path: dir.join("child-input.json").to_string_lossy().into_owned(),
                handle_path: handle_path.to_string_lossy().into_owned(),
                invocation_name: "fake-invocation".to_owned(),
                role_prompt: "do the fake work".to_owned(),
                model_hint: None,
            },
            crate::config::SpawnDialect::Spawn09,
        );
        KernelSubmission {
            program,
            handle_path,
            workspace: dir.to_path_buf(),
            model_hint: None,
        }
    }

    /// (a) `submit` returns the handle while the parent is still alive
    /// AND stdin is still open — the invocation owns the parent's
    /// lifetime, never the transport.
    #[cfg(unix)]
    #[test]
    fn submit_returns_the_handle_while_the_parent_stays_alive() {
        let dir = rpc_scratch("alive");
        let child_dir = dir.join("child-session");
        std::fs::create_dir_all(&child_dir).expect("child session dir creatable");
        let binary = fake_rpc_binary(&dir, &child_dir, false, false);
        let transport = ProcessKernelTransport::new(binary, std::time::Duration::from_secs(30));
        let (handle, mut parent) = transport
            .submit(&rpc_submission(&dir), far_future_deadline_rpc())
            .expect("a cooperating fake rpc binary admits");
        assert_eq!(handle.rlm_child_id, "sub-fake01");
        // Give the reader/stderr threads a moment, then assert the parent
        // has NOT exited (it is blocked on stdin, which is still open).
        std::thread::sleep(std::time::Duration::from_millis(200));
        assert!(
            parent.try_reap().is_none(),
            "the parent must still be alive — stdin was never closed"
        );
        parent.terminate();
        std::fs::remove_dir_all(&dir).ok();
    }

    /// (b) telemetry sums the parent's usage/tool-execution counts and
    /// keeps the latest per-child record.
    #[cfg(unix)]
    #[test]
    fn telemetry_sums_parent_and_child_counters() {
        let dir = rpc_scratch("telemetry");
        let child_dir = dir.join("child-session");
        std::fs::create_dir_all(&child_dir).expect("child session dir creatable");
        let binary = fake_rpc_binary(&dir, &child_dir, false, false);
        let transport = ProcessKernelTransport::new(binary, std::time::Duration::from_secs(30));
        let (_handle, mut parent) = transport
            .submit(&rpc_submission(&dir), far_future_deadline_rpc())
            .expect("admission succeeds");
        // Bounded wait for the reader thread to observe the events.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        let telemetry = loop {
            let snapshot = parent.telemetry();
            if snapshot.total_tokens > 0 && !snapshot.children.is_empty() {
                break snapshot;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "telemetry never arrived"
            );
            std::thread::sleep(std::time::Duration::from_millis(20));
        };
        assert_eq!(telemetry.input_tokens, 100);
        assert_eq!(telemetry.output_tokens, 50);
        assert_eq!(telemetry.cache_read_tokens, 10);
        assert_eq!(telemetry.cache_write_tokens, 5);
        assert_eq!(telemetry.total_tokens, 165);
        assert_eq!(telemetry.cost_usd, MicroUsd::new(3200));
        assert_eq!(telemetry.tool_execution_count, 1);
        let child = telemetry
            .children
            .get("sub-fake01")
            .expect("child recorded");
        assert_eq!(child.token_count, 777);
        assert_eq!(child.tool_use_count, 3);
        assert_eq!(child.status, "running");
        parent.terminate();
        std::fs::remove_dir_all(&dir).ok();
    }

    /// (f) events after admission — including `rlm_child_update` — must
    /// advance `last_event_epoch` PAST the moment admission returned
    /// (liveness addendum: a builder still emitting child telemetry is
    /// never mistaken for stalled).
    #[cfg(unix)]
    #[test]
    fn telemetry_events_after_admission_advance_last_event_epoch() {
        let dir = rpc_scratch("last-event");
        let child_dir = dir.join("child-session");
        std::fs::create_dir_all(&child_dir).expect("child session dir creatable");
        let binary = fake_rpc_binary(&dir, &child_dir, false, false);
        let before_admission = ProcessKernelTransport::now_secs();
        let transport = ProcessKernelTransport::new(binary, std::time::Duration::from_secs(30));
        let (_handle, mut parent) = transport
            .submit(&rpc_submission(&dir), far_future_deadline_rpc())
            .expect("admission succeeds");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            if parent.telemetry().last_event_epoch >= before_admission {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "last_event_epoch never advanced past admission"
            );
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        parent.terminate();
        std::fs::remove_dir_all(&dir).ok();
    }

    /// (c) `terminate` closes stdin and the fake exits 0 within the rpc
    /// EOF grace — no SIGKILL needed (the forensic note names a clean
    /// exit, not a kill).
    #[cfg(unix)]
    #[test]
    fn terminate_closes_stdin_and_the_fake_exits_clean() {
        let dir = rpc_scratch("terminate");
        let child_dir = dir.join("child-session");
        std::fs::create_dir_all(&child_dir).expect("child session dir creatable");
        let binary = fake_rpc_binary(&dir, &child_dir, false, false);
        let transport = ProcessKernelTransport::new(binary, std::time::Duration::from_secs(30));
        let (_handle, mut parent) = transport
            .submit(&rpc_submission(&dir), far_future_deadline_rpc())
            .expect("admission succeeds");
        let note = parent.terminate();
        assert!(
            note.contains("stdin closed") && note.contains("parent exited after stdin EOF"),
            "terminate must close stdin and observe a clean exit, not fall through to a kill: \
             {note}"
        );
        assert!(
            !note.contains("kill"),
            "no kill should have been necessary: {note}"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    /// (d) a `success:false` prompt response ends admission as
    /// `HandleAbsent`, naming the refusal error text.
    #[cfg(unix)]
    #[test]
    fn a_prompt_refusal_ends_admission_as_handle_absent_naming_the_error() {
        let dir = rpc_scratch("refused");
        let child_dir = dir.join("child-session");
        std::fs::create_dir_all(&child_dir).expect("child session dir creatable");
        let binary = fake_rpc_binary(&dir, &child_dir, true, false);
        let transport = ProcessKernelTransport::new(binary, std::time::Duration::from_secs(30));
        let error = transport
            .submit(&rpc_submission(&dir), far_future_deadline_rpc())
            .expect_err("a refused prompt must not admit");
        let TransportError::HandleAbsent { detail } = error else {
            panic!("expected HandleAbsent, got {error:?}")
        };
        assert!(
            detail.contains("fake refusal: nonblocking-delegation policy"),
            "the refusal detail must name the parent's error text: {detail}"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    /// Coordinator review finding (task26-dcritic-apparatus-repair): under
    /// rpc mode the parent does NOT exit when the model refuses the CELL
    /// (as opposed to the prompt) — its turn just ends (`agent_end`) and
    /// it idles. Without the `agent_end`-aware grace the handle watch
    /// would run all the way to the 600 s admission timeout; this test
    /// pins that it instead ends promptly (well under 15 s) as
    /// `HandleAbsent`, carrying the parent's last assistant text.
    #[cfg(unix)]
    #[test]
    fn a_cell_refusal_ends_admission_promptly_naming_the_last_assistant_text() {
        let dir = rpc_scratch("cell-refused");
        let child_dir = dir.join("child-session");
        std::fs::create_dir_all(&child_dir).expect("child session dir creatable");
        let binary = fake_rpc_binary(&dir, &child_dir, false, true);
        // The admission timeout is deliberately generous (600 s, matching
        // the production default) — the assertion below proves the watch
        // ends on the agent_end grace, never on this bound.
        let transport = ProcessKernelTransport::new(binary, std::time::Duration::from_secs(600));
        let started = std::time::Instant::now();
        let error = transport
            .submit(&rpc_submission(&dir), far_future_deadline_rpc())
            .expect_err("a cell refusal must not admit");
        let elapsed = started.elapsed();
        assert!(
            elapsed < std::time::Duration::from_secs(15),
            "the agent_end-aware grace must end the watch promptly, not at the 600 s admission \
             timeout: took {elapsed:?}"
        );
        let TransportError::HandleAbsent { detail } = error else {
            panic!("expected HandleAbsent, got {error:?}")
        };
        assert!(
            detail.contains("I can't run that"),
            "the refusal detail must carry the parent's last assistant text: {detail}"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    /// The happy path still passes when `agent_end` arrives AFTER the
    /// handle write — a late `agent_end` on an already-admitted invocation
    /// must never be mistaken for a refusal.
    #[cfg(unix)]
    #[test]
    fn a_late_agent_end_after_the_handle_write_does_not_disturb_admission() {
        let dir = rpc_scratch("late-agent-end");
        let child_dir = dir.join("child-session");
        std::fs::create_dir_all(&child_dir).expect("child session dir creatable");
        let binary = fake_rpc_binary(&dir, &child_dir, false, false);
        let transport = ProcessKernelTransport::new(binary, std::time::Duration::from_secs(30));
        let (handle, mut parent) = transport
            .submit(&rpc_submission(&dir), far_future_deadline_rpc())
            .expect("a cooperating fake rpc binary admits despite its later agent_end");
        assert_eq!(handle.rlm_child_id, "sub-fake01");
        parent.terminate();
        std::fs::remove_dir_all(&dir).ok();
    }

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
        crate::test_support::make_executable_ready(&script);
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
