//! The generic CLI agent runner (spec §4.1 `cli-agent`, §6.1, §10.3; plan
//! Task 16): a REAL admission-based `AgentRunner` over a configured
//! command template and real child processes.
//!
//! The state machine has ONE completion input — the result contract at
//! the invocation's resolved path (§10.3), read through the strict
//! [`read_result_contract`](gauntlet_ports::role_result::read_result_contract)
//! codec — plus ONE process-liveness observation, which can only ever
//! make things WORSE (a dead child with no document), never complete
//! anything. The classification is a TOTAL ORDER in `poll` (RES-D8a):
//!
//! 1. contract read: valid matching document ⇒ `Completed` (whatever the
//!    exit status says — the contract wins, §6.1/§10.3; the exit status
//!    is recorded in adapter-local forensics only, never obeyed — the
//!    Annex B.1 strict codec admits no extra result fields); foreign
//!    identity ⇒ `Failed{contract_mismatch}` (rule 6); a PARSE failure
//!    is decidable only once the child is terminal (a live child may be
//!    mid-write — a torn read of a landing document never pins a
//!    terminal verdict; the exit-path re-read then classifies rule 4's
//!    `Failed{malformed_result}`);
//! 2. child EXITED (any exit status — clean, nonzero, signal-death: the
//!    collapse is deliberate, RES-D8c) with no valid document ⇒
//!    `Failed{missing_result}`, decided BEFORE the deadline — a dead
//!    child can never write. The exit observation triggers ONE final
//!    contract re-read: the child's last write races the exit, and the
//!    re-read closes it;
//! 3. still RUNNING, silent, past the spec deadline ⇒ `TimedOut` (rule
//!    5; the stop is a separate bounded operation, best-effort — a failed
//!    stop never un-time-outs the invocation);
//! 4. otherwise `Running` (RES-D12: the `last_event_at` fallback is the
//!    admission instant; `payment_so_far` is ZERO and is NOT A
//!    MEASUREMENT — budget truth stays with the runtime's `BudgetMeter`).
//!
//! Once terminal, the SAME status is reported forever (the ports' pinned
//! post-terminal stability); a late writer can never resurrect a verdict.
//!
//! - `invoke` refuses duplicates, anchors the invocation directory, renders
//!   the template (typed failures ⇒ `AdmissionRefused`), writes the
//!   envelope-body sibling (RES-D2) and the prompt file, spawns through
//!   the seam, records the forensic handle file (RES-D12), and returns
//!   promptly (§6.1: never blocks on completion).
//! - `cancel` is idempotent: the first cancel stops the child under the
//!   CALLER'S bound (SIGTERM, bounded wait, SIGKILL — the seam's job) and
//!   pins `Cancelled`; a stranger handle is `UnknownHandle`; a failed
//!   stop leaves the state unpinned so the cancel can be retried.
//! - `collect` returns the validated document after `Completed` only.
//! - `capabilities` is HONEST (§7.3: "a runner reporting `isolation:
//!   none` MUST NOT be selected for a tier requiring containment"):
//!   no containment (external wrapping is the Sandbox port's job, RES-D7),
//!   file delivery, no native subagents, streaming not part of this
//!   adapter's surface, `supports_model_selection` TRUE exactly when the
//!   configured template uses `{model}` — the capability report derives
//!   from configuration, not aspiration — and `max_depth` from
//!   configuration.
//!
//! Time comes from the INJECTED `Clock` port — never the system clock
//! (RES-D9). The one filesystem anchor rule (prime's RES-D3 precedent):
//! `invoke` creates `<state_dir>/runs/<run_id>/inv/<invocation_id>/`; the
//! opaque `ResultContractPath` token resolves as the LITERAL LEAF inside
//! it; the envelope body, prompt, and handle files are derived siblings,
//! and the invocation directory is RETAINED after terminal states
//! (RES-D12 forensics; §18.3's orphan discovery reads the handle file).

use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use gauntlet_domain::{EpochSeconds, InvocationId, InvocationState, Payment, SessionRef};
use gauntlet_ports::agent_runner::{
    AgentError, AgentRunner, CancelReason, InvocationHandle, InvocationStatus, Isolation,
    ResultDelivery, RoleInvocationSpec, RunnerCapabilities,
};
use gauntlet_ports::clock::Clock;
use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::role_result::{RoleResult, read_result_contract};

use crate::config::CliAgentConfig;
use crate::prompt::render_prompt;
use crate::template::{Template, TemplateValues, render as render_argv};
use crate::transport::{ProcessSpec, ProcessTransport};

/// Why the containment seam refused to wrap a dispatch (plan Task 25).
/// Every refusal means the dispatch MUST NOT run — the runner maps it to
/// a typed admission refusal, never an unwrapped execution (§21.3,
/// AK-047).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainmentRefusal {
    detail: String,
}

impl ContainmentRefusal {
    pub fn new(detail: impl Into<String>) -> Self {
        Self {
            detail: detail.into(),
        }
    }
}

impl fmt::Display for ContainmentRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.detail)
    }
}

impl std::error::Error for ContainmentRefusal {}

/// The containment wrap seam (plan Task 25; spec §17.5.4, AK-048): the
/// ONE channel through which a dispatch's admitted sandbox policy becomes
/// the command actually executed. Installed by the composition root when
/// the posture fronts a LIVE sandbox backend; never set otherwise.
///
/// R-2 posture re-adjudication (recorded for the operator): this adapter
/// historically consumed ONLY a command template and reported
/// `isolation: none`. With the seam installed, a dispatch whose spec
/// carries a live-backend policy ref is WRAPPED through this seam — the
/// wrapped command's FIRST process is the sandbox — and the runner
/// honestly reports `Isolation::Process`, derived from that
/// configuration exactly the way `supports_model_selection` derives from
/// the template. A dispatch carrying a policy with NO seam installed is
/// a typed admission refusal: fail-closed, never an unwrapped run.
pub trait CommandContainment: fmt::Debug + Send + Sync {
    /// Wraps ONE rendered child command for ONE admitted dispatch under
    /// the policy the gate admitted (`policy` — the ref the controller's
    /// durable admission record names). Returns the ONE shell-invocable
    /// command to execute, or a typed refusal. There is no arm that means
    /// "run it unwrapped instead" — that arm is the unsandboxed fallback
    /// AK-047 forbids.
    fn wrap(
        &self,
        workspace: &gauntlet_domain::WorkspaceRef,
        policy: &gauntlet_domain::SandboxPolicyRef,
        command: &str,
    ) -> Result<String, ContainmentRefusal>;
}

/// POSIX single-word quoting: the rendered argv words must survive ONE
/// shell evaluation round-trip exactly (the wrapped command string is
/// evaluated by the sandbox's own `sh -c` payload). Single quotes with
/// the standard `'\' escape keep every byte verbatim.
fn shell_word(word: &str) -> String {
    format!("'{}'", word.replace('\'', "'\\''"))
}

/// Joins rendered argv words into ONE shell command string — the input
/// shape the gate's wrap (and every `SandboxedCommand`) is defined over.
fn shell_join(argv: &[String]) -> String {
    argv.iter()
        .map(|word| shell_word(word))
        .collect::<Vec<_>>()
        .join(" ")
}

/// One admitted invocation's live state.
struct CliEntry {
    spec: RoleInvocationSpec,
    /// The contract document's resolved file (under the invocation dir).
    contract_file: PathBuf,
    /// The transport-level child identifier.
    child: crate::transport::ChildId,
    /// The admission instant (the `last_event_at` fallback).
    admission_at: EpochSeconds,
    /// The observed exit status, when the child exited — ADAPTER-LOCAL
    /// FORENSICS ONLY (RES-D8b): never a completion input, never a result
    /// field. `None` = still running; `Some(None)` = killed by a signal.
    exit_status: Option<Option<i32>>,
    terminal: Option<InvocationStatus>,
    result: Option<RoleResult>,
    /// A control-operation note (e.g. a timeout stop that failed) kept
    /// for forensics; never a state input.
    control_note: Option<String>,
}

/// The invocation directory anchor (prime's one-anchor rule): the
/// invocation directory under the state root, the contract token as the
/// LITERAL LEAF, the envelope body / prompt / handle files as derived
/// siblings. A token that is not a single leaf component is a dispatch
/// defect, refused at admission; the ids are validated ULIDs, and the
/// `starts_with` check stays as the second guard (the two-guard posture).
struct InvocationFiles {
    dir: PathBuf,
    contract_file: PathBuf,
    envelope_file: PathBuf,
    prompt_file: PathBuf,
    handle_file: PathBuf,
}

fn invocation_files(
    state_dir: &Path,
    spec: &RoleInvocationSpec,
) -> Result<InvocationFiles, AgentError> {
    let refused = |detail: String| AgentError::AdmissionRefused { detail };
    let key = spec.result_contract_path.as_str();
    let path = Path::new(key);
    let leaf_ok = path
        .components()
        .next()
        .is_some_and(|first| matches!(first, std::path::Component::Normal(_)))
        && path.components().count() == 1;
    if !leaf_ok {
        return Err(refused(format!(
            "the result-contract token {key:?} must be a single leaf component; the runtime \
             anchors it under runs/<run_id>/inv/<invocation_id>/"
        )));
    }
    let dir = state_dir
        .join("runs")
        .join(spec.run_id.as_str())
        .join("inv")
        .join(spec.invocation_id.as_str());
    if !dir.starts_with(state_dir) {
        return Err(refused(
            "the invocation directory does not resolve under the state root".to_owned(),
        ));
    }
    Ok(InvocationFiles {
        contract_file: dir.join(path),
        envelope_file: dir.join(format!("{key}.envelope.json")),
        prompt_file: dir.join(format!("{key}.prompt.txt")),
        handle_file: dir.join(format!("{key}.handle.json")),
        dir,
    })
}

/// The EXECUTION anchor for one dispatch (plan Task 20 COR-D3,
/// §17.5.1 invariant 1): the canonical workspace root joined with the
/// dispatch's `spec.workspace` segment — the workspace the controller
/// allocated and admitted the dispatch against. The ref is untrusted
/// external input: it must be a single key-class component (`[A-Za-z0-9._-]`,
/// the workspace provider's §17.5.3 class, re-derived here because the
/// adapter depends on domain and ports only) before it may join, and the
/// directory is ensured (a real child needs a cwd; the provider normally
/// created it before dispatch). Any refusal is typed
/// `AdmissionRefused` — fail-closed at admission, nothing left behind.
fn execution_root(
    workspace_root: &Path,
    workspace: &gauntlet_domain::WorkspaceRef,
) -> Result<PathBuf, AgentError> {
    let reference = workspace.as_str();
    let key_ok = !reference.is_empty()
        && !reference.contains(['/', '\\'])
        && reference.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || byte == b'.' || byte == b'_' || byte == b'-'
        });
    if !key_ok {
        return Err(AgentError::AdmissionRefused {
            detail: format!(
                "workspace ref {reference:?} is outside the single-component key class — the \
                 execution anchor refuses to join it onto the workspace root"
            ),
        });
    }
    let root = workspace_root.join(reference);
    std::fs::create_dir_all(&root).map_err(|error| AgentError::AdmissionRefused {
        detail: format!(
            "the per-unit workspace {} could not be ensured as the execution anchor ({error})",
            root.display()
        ),
    })?;
    Ok(root)
}

/// The coarse state an [`InvocationStatus`] maps to, for the ports' typed
/// `ResultNotReady`/`AlreadyTerminal` loci.
fn status_state(status: Option<&InvocationStatus>) -> InvocationState {
    match status {
        Some(InvocationStatus::Completed { .. }) => InvocationState::Completed,
        Some(InvocationStatus::Failed { .. }) => InvocationState::Failed,
        Some(InvocationStatus::TimedOut) => InvocationState::TimedOut,
        Some(InvocationStatus::Cancelled) => InvocationState::Cancelled,
        Some(InvocationStatus::Admitted) | Some(InvocationStatus::Running { .. }) => {
            InvocationState::Running
        }
        None => InvocationState::Running,
    }
}

/// One admitted invocation's forensic snapshot — evidence, never control
/// state (I11). The exit status is recorded HERE and nowhere else
/// (RES-D8b).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliForensics {
    /// The observed exit status: `None` while running; `Some(None)` when
    /// the child was killed by a signal.
    pub exit_status: Option<Option<i32>>,
    pub control_note: Option<String>,
}

/// The bounded span of the terminal RETIREMENT stop (COR-F4): hygiene
/// after the verdict is fixed, deliberately shorter than a
/// cancellation's configured span.
const RETIRE_SPAN_SECONDS: u64 = 5;

/// The real generic CLI runner. Generic over the process seam; the
/// production composition binds
/// [`crate::transport::ProcessChildTransport`]; the shared contract suite
/// binds script-following in-memory doubles (spec §6.3: the child is what
/// gets doubled, not the adapter's logic).
pub struct CliAgentRunner<T: ProcessTransport> {
    config: CliAgentConfig,
    /// The template, parsed ONCE at construction (a template that cannot
    /// parse never admits anything).
    template: Template,
    state_dir: PathBuf,
    /// The CANONICAL root the dispatch's per-unit workspaces live under
    /// (injected from the composition root). Each invocation's EXECUTION
    /// anchor is `<workspace_root>/<spec.workspace>/` — the workspace the
    /// controller allocated and admitted the dispatch against (plan Task
    /// 20 COR-D3, §17.5.1 invariant 1: builder processes run only inside
    /// their assigned workspace) — and it is what the child's cwd, the
    /// `{workspace}` template placeholder, and the prompt's WORKSPACE
    /// field carry.
    workspace_root: PathBuf,
    transport: T,
    clock: Arc<dyn Clock>,
    /// The containment wrap seam (plan Task 25), installed by the
    /// composition root exactly when the posture fronts a LIVE sandbox
    /// backend. `None` is the historical template-only posture (§8.6's
    /// conservative world, whose dispatches carry no policy).
    containment: Option<Arc<dyn CommandContainment>>,
    entries: RefCell<HashMap<InvocationId, CliEntry>>,
}

impl<T: ProcessTransport> CliAgentRunner<T> {
    /// A runner over the supplied seam, anchoring invocation IO under
    /// `state_dir` and starting children in `workspace_root`. The
    /// template is parsed here: an unparseable template is a typed
    /// configuration failure before any admission.
    pub fn new(
        config: CliAgentConfig,
        state_dir: PathBuf,
        workspace_root: PathBuf,
        transport: T,
        clock: Arc<dyn Clock>,
    ) -> Result<Self, crate::config::ConfigError> {
        let template = config.parsed_template()?;
        Ok(Self {
            config,
            template,
            state_dir,
            workspace_root,
            transport,
            clock,
            containment: None,
            entries: RefCell::new(HashMap::new()),
        })
    }

    /// Installs the containment wrap seam (plan Task 25). The composition
    /// root calls this exactly when its posture fronts a LIVE sandbox
    /// backend; the same posture's wrap then routes into every spawned
    /// child. Builder-style: `CliAgentRunner::new(...)?.with_containment(..)`.
    pub fn with_containment(mut self, containment: Arc<dyn CommandContainment>) -> Self {
        self.containment = Some(containment);
        self
    }

    /// The configured runner capabilities' source configuration.
    pub fn config(&self) -> &CliAgentConfig {
        &self.config
    }

    /// The invocation's forensic snapshot (see [`CliForensics`]).
    pub fn forensics(&self, handle: &InvocationHandle) -> Option<CliForensics> {
        let entries = self.entries.borrow();
        let entry = entries.get(handle.invocation_id())?;
        Some(CliForensics {
            exit_status: entry.exit_status,
            control_note: entry.control_note.clone(),
        })
    }

    /// The one contract classification: strict decode plus the §10.3
    /// rule-6 identity check, failures mapped by the ports' vocabulary
    /// (§10.3 rules 3/4/6). `Ok(None)` = no document yet.
    fn classify_contract(
        entry: &mut CliEntry,
        malformed_decidable: bool,
    ) -> Result<Option<InvocationStatus>, AgentError> {
        match std::fs::read_to_string(&entry.contract_file) {
            Ok(text) => {
                let outcome = read_result_contract(
                    &text,
                    &entry.spec.invocation_id,
                    entry.spec.context_envelope.digest(),
                );
                // Model output is untrusted external input: parse,
                // validate, admit — or classify whole.
                match outcome {
                    Ok(result) => {
                        entry.result = Some(result);
                        Ok(Some(InvocationStatus::Completed {
                            result_ref: entry.spec.result_contract_path.clone(),
                        }))
                    }
                    Err(error) => {
                        // A PARSE failure is only decidable once the child
                        // is terminal: a live child may be mid-write, and
                        // a torn read of a landing document must never pin
                        // a terminal verdict (the task20 torn-read
                        // correction — observed as `MalformedResult` over
                        // a document that was complete on disk). While
                        // alive, the read stays undecidable; the exit path
                        // re-reads and classifies then. A VALID parse
                        // (Completed, mismatch) needs no such grace: a
                        // torn write does not strictly validate.
                        if !malformed_decidable {
                            return Ok(None);
                        }
                        Ok(Some(InvocationStatus::Failed {
                            class: error.failure_class(),
                        }))
                    }
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => {
                // An unreadable contract is an observation failure, never
                // a coerced Running (§21.2: uncertainty is never success).
                Err(AgentError::Transport {
                    detail: format!(
                        "the result contract at {} could not be read ({error})",
                        entry.contract_file.display()
                    ),
                })
            }
        }
    }

    /// Pins and returns a terminal verdict.
    fn pin(entry: &mut CliEntry, status: InvocationStatus) -> InvocationStatus {
        entry.terminal = Some(status.clone());
        status
    }

    /// The bounded stop used by cancel and the timeout path. `deadline`
    /// bounds THIS blocking operation (§6 preamble).
    fn stop_child(&self, entry: &mut CliEntry, deadline: Deadline) -> Result<(), AgentError> {
        match self.transport.stop(entry.child, deadline) {
            Ok(_) => Ok(()),
            Err(error @ crate::transport::TransportError::UnknownChild { .. }) => {
                // Already gone (or the transport restarted): the stop's
                // goal is achieved; record the note, never fail the cancel.
                entry.control_note = Some(error.to_string());
                Ok(())
            }
            Err(error) => Err(AgentError::CancelFailed {
                invocation_id: entry.spec.invocation_id.clone(),
                detail: error.to_string(),
            }),
        }
    }

    /// A fresh bound for the runner's OWN stop operation, past now (§6
    /// preamble: bounded blocking control operations).
    fn fresh_stop_bound(&self) -> Deadline {
        let now = self.clock.now();
        Deadline::at_epoch(EpochSeconds::new(
            now.get().saturating_add(self.config.cancel_span_seconds),
        ))
    }

    /// A fresh bound for the terminal RETIREMENT stop (COR-F4): shorter
    /// than a cancellation's span — this is hygiene after the verdict is
    /// already fixed, not the verdict's own act.
    fn fresh_retire_bound(&self) -> Deadline {
        let now = self.clock.now();
        Deadline::at_epoch(EpochSeconds::new(
            now.get().saturating_add(RETIRE_SPAN_SECONDS),
        ))
    }

    /// COR-F4: best-effort retirement of the child at a terminal pin.
    /// The verdict is already fixed by the contract or the observation;
    /// a child still alive holds no remaining authority (post-terminal
    /// stability — a late write can never resurrect the verdict), so
    /// leaving it running is a LEAK, not a participation. The stop's
    /// outcome is never a state input: a failure surfaces at most as a
    /// control note, and the verdict stands.
    fn retire_child(&self, entry: &mut CliEntry) {
        let bound = self.fresh_retire_bound();
        if let Err(error) = self.stop_child(entry, bound) {
            entry.control_note = Some(error.to_string());
        }
    }
}

impl<T: ProcessTransport> AgentRunner for CliAgentRunner<T> {
    fn invoke(&self, spec: RoleInvocationSpec) -> Result<InvocationHandle, AgentError> {
        let invocation_id = spec.invocation_id.clone();
        // Duplicate admission is refused: resetting an entry would
        // resurrect terminal state (the ports' pinned terminal stability).
        if self.entries.borrow().contains_key(&invocation_id) {
            return Err(AgentError::AdmissionRefused {
                detail: format!(
                    "invocation {invocation_id} is already admitted to this runner; duplicate \
                     admission is refused and the existing entry is untouched"
                ),
            });
        }
        // The one anchor rule: invocation dir under the state root, the
        // contract token as the literal leaf.
        let files = invocation_files(&self.state_dir, &spec)?;
        // The EXECUTION anchor (plan Task 20 COR-D3, §17.5.1 invariant 1):
        // the admitted per-unit workspace — the canonical root joined with
        // the dispatch's `spec.workspace` segment. The ref is untrusted
        // external input: it must be a single key-class component before
        // it may join (the workspace provider's §17.5.3 class, re-derived
        // here because the adapter depends on domain and ports only); the
        // provider normally created the directory before dispatch, and the
        // adapter ensures it (a real child needs a cwd) — failure is a
        // typed admission refusal, never a guessed-at path.
        let execution_root = execution_root(&self.workspace_root, &spec.workspace)?;
        // Render the template BEFORE any filesystem effect: a template
        // failure is a typed configuration refusal and must leave nothing
        // behind (fail-closed at admission; RES-D8's refusal set).
        let argv = render_argv(
            &self.template,
            &TemplateValues {
                prompt_file: &files.prompt_file.to_string_lossy(),
                workspace: &execution_root.to_string_lossy(),
                result_path: &files.contract_file.to_string_lossy(),
                model: spec.model_hint.as_ref().map(|model| model.as_str()),
            },
        )
        .map_err(|error| AgentError::AdmissionRefused {
            detail: format!("the configured command template refused this dispatch: {error}"),
        })?;
        // THE CONTAINMENT WRAP (plan Task 25, AK-048): between admission
        // and process start, a dispatch the gate admitted WITH a live
        // sandbox policy spawns the gate's wrapped command — the sandbox
        // is the FIRST process — and one without a seam installed is a
        // typed refusal, never an unwrapped run (§21.3, AK-047). A
        // no-policy dispatch (the §8.6 contained/no-backend shape) spawns
        // the rendered argv verbatim, exactly as before this seam
        // existed. The wrap runs BEFORE any filesystem effect: a refusal
        // leaves nothing behind (fail-closed at admission).
        let spawn_argv = match (&self.containment, spec.sandbox_policy.as_ref()) {
            (Some(containment), Some(policy_ref)) => {
                let command = shell_join(&argv);
                let wrapped = containment
                    .wrap(&spec.workspace, policy_ref, &command)
                    .map_err(|error| AgentError::AdmissionRefused {
                        detail: format!(
                            "the containment wrap refused this dispatch: {error} — not \
                             executed unwrapped (spec 21.3, AK-047)"
                        ),
                    })?;
                vec![crate::WRAPPED_SHELL.to_owned(), "-c".to_owned(), wrapped]
            }
            (None, Some(policy_ref)) => {
                return Err(AgentError::AdmissionRefused {
                    detail: format!(
                        "the dispatch carries the admitted sandbox policy {policy_ref} but \
                         this runner has no containment seam installed — refusing to run it \
                         unwrapped (spec 21.3, AK-047)"
                    ),
                });
            }
            (_, None) => argv,
        };
        // §10.3 rule 1: the invocation directory, the envelope BODY
        // sibling (RES-D2), and the prompt file (§10.3 rule 1's prompt
        // channel).
        std::fs::create_dir_all(&files.dir).map_err(|error| AgentError::AdmissionRefused {
            detail: format!(
                "the invocation directory for {invocation_id} could not be created ({error})"
            ),
        })?;
        std::fs::write(&files.envelope_file, spec.context_envelope.body()).map_err(|error| {
            AgentError::AdmissionRefused {
                detail: format!(
                    "the context envelope for {invocation_id} could not be written ({error})"
                ),
            }
        })?;
        std::fs::write(
            &files.prompt_file,
            render_prompt(
                &spec,
                &files.envelope_file,
                &files.contract_file,
                &execution_root,
            ),
        )
        .map_err(|error| AgentError::AdmissionRefused {
            detail: format!("the prompt file for {invocation_id} could not be written ({error})"),
        })?;
        // SPAWN: the child gets the (possibly wrapped) argv, the ADMITTED
        // PER-UNIT workspace as its cwd, and the environment channel
        // (parent env verbatim + GAUNTLET_RESULT_PATH, RES-D3). A spawn
        // failure is the port's Transport class (the environment may be
        // fixable and the dispatch retryable).
        let child = self
            .transport
            .spawn(&ProcessSpec {
                argv: spawn_argv,
                cwd: execution_root,
                result_path: files.contract_file.to_string_lossy().into_owned(),
            })
            .map_err(|error| match error {
                crate::transport::TransportError::SpawnFailed { detail, program } => {
                    AgentError::Transport {
                        detail: format!("spawning {program:?} for {invocation_id}: {detail}"),
                    }
                }
                other => AgentError::Transport {
                    detail: format!("admitting {invocation_id} at the process seam: {other}"),
                },
            })?;
        // Forensic handle record (RES-D12, §18.3): the child identifier
        // (the OS pid for the production transport) plus the invocation
        // identity, so a post-crash resume can find orphans. Written
        // through the ports' JSON writer; untrusted output is never read
        // back from it — it is evidence, not control state.
        let handle_document = gauntlet_ports::json::JsonValue::Object(vec![
            (
                "invocation_id".to_owned(),
                gauntlet_ports::json::JsonValue::Str(invocation_id.as_str().to_owned()),
            ),
            (
                "run_id".to_owned(),
                gauntlet_ports::json::JsonValue::Str(spec.run_id.as_str().to_owned()),
            ),
            (
                "child".to_owned(),
                gauntlet_ports::json::JsonValue::Number(gauntlet_ports::json::JsonNumber::Uint(
                    child.raw(),
                )),
            ),
        ])
        .to_json_text();
        let _ = std::fs::write(&files.handle_file, handle_document);
        // One clock read at admission: the Running payload's fallback
        // instant.
        let admission_at = self.clock.now();
        // The session reference is the child identifier under an explicit
        // scheme — forensic evidence only, never control state (I11).
        let session_ref = SessionRef::new(&format!("cli-child:{}", child.raw())).ok();
        self.entries.borrow_mut().insert(
            invocation_id.clone(),
            CliEntry {
                spec,
                contract_file: files.contract_file,
                child,
                admission_at,
                exit_status: None,
                terminal: None,
                result: None,
                control_note: None,
            },
        );
        Ok(InvocationHandle::new(invocation_id, session_ref))
    }

    fn poll(&self, handle: &InvocationHandle) -> Result<InvocationStatus, AgentError> {
        let mut entries = self.entries.borrow_mut();
        let Some(entry) = entries.get_mut(handle.invocation_id()) else {
            return Err(AgentError::UnknownHandle {
                invocation_id: handle.invocation_id().clone(),
            });
        };
        // Post-terminal stability: the SAME terminal status forever.
        if let Some(terminal) = &entry.terminal {
            return Ok(terminal.clone());
        }
        // TOTAL ORDER (RES-D8a), step 1: the contract read — the ONE
        // completion fact. A parse failure is not yet decidable here (the
        // child may be mid-write; see the torn-read note in
        // `classify_contract`), so only a VALID document completes at this
        // step.
        if let Some(terminal) = Self::classify_contract(entry, false)? {
            self.retire_child(entry);
            return Ok(Self::pin(entry, terminal));
        }
        // Step 2: the process observation — it can only ever worsen the
        // verdict (a dead child with no document), never complete one.
        // A transport failure is an observation failure (§21.2): typed,
        // never a coerced Running.
        let observed =
            self.transport
                .state(entry.child)
                .map_err(|error| AgentError::Transport {
                    detail: format!(
                        "probing the child of {invocation} failed: {error}",
                        invocation = entry.spec.invocation_id
                    ),
                })?;
        match observed {
            crate::transport::ProcessState::Exited { code } => {
                // Forensics only (RES-D8b); the value is never obeyed.
                entry.exit_status = Some(code);
                // The exit-triggered FINAL re-read: the child's last
                // write raced the exit; give the document its last chance
                // before declaring it missing forever. The child is now
                // terminal, so a parse failure IS decidable here (the
                // torn-read grace ends with the process).
                if let Some(terminal) = Self::classify_contract(entry, true)? {
                    self.retire_child(entry);
                    return Ok(Self::pin(entry, terminal));
                }
                // Exited, no valid document, before (or regardless of) any
                // deadline: a dead child can never write (RES-D8c —
                // signal-death and nonzero exits collapse here on
                // purpose).
                return Ok(Self::pin(
                    entry,
                    InvocationStatus::Failed {
                        class: gauntlet_ports::agent_runner::InvocationFailure::MissingResult,
                    },
                ));
            }
            crate::transport::ProcessState::Running => {}
        }
        // Step 3: the deadline verdict, for a child still RUNNING and
        // silent (rule 5). One clock read per poll, only reached when the
        // verdict is not yet decidable.
        let now = self.clock.now();
        if entry.spec.deadline.exceeded_at(now) {
            let stop_bound = self.fresh_stop_bound();
            if let Err(error) = self.stop_child(entry, stop_bound) {
                entry.control_note = Some(error.to_string());
            }
            return Ok(Self::pin(entry, InvocationStatus::TimedOut));
        }
        // Step 4: in flight. `last_event_at` is the last observed change
        // of the contract path (mtime), falling back to the admission
        // instant; `payment_so_far` is ZERO and is NOT A MEASUREMENT.
        let last_event_at = std::fs::symlink_metadata(&entry.contract_file)
            .ok()
            .and_then(|meta| meta.modified().ok())
            .and_then(|modified| modified.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|since| EpochSeconds::new(since.as_secs()))
            .unwrap_or(entry.admission_at);
        Ok(InvocationStatus::Running {
            last_event_at,
            payment_so_far: Payment::zero(),
        })
    }

    fn collect(
        &self,
        handle: &InvocationHandle,
        _deadline: Deadline,
    ) -> Result<RoleResult, AgentError> {
        let entries = self.entries.borrow();
        let Some(entry) = entries.get(handle.invocation_id()) else {
            return Err(AgentError::UnknownHandle {
                invocation_id: handle.invocation_id().clone(),
            });
        };
        match (&entry.terminal, &entry.result) {
            (Some(InvocationStatus::Completed { .. }), Some(result)) => Ok(result.clone()),
            (terminal, _) => Err(AgentError::ResultNotReady {
                invocation_id: handle.invocation_id().clone(),
                state: status_state(terminal.as_ref()),
            }),
        }
    }

    fn cancel(
        &self,
        handle: &InvocationHandle,
        _reason: CancelReason,
        deadline: Deadline,
    ) -> Result<(), AgentError> {
        let mut entries = self.entries.borrow_mut();
        let Some(entry) = entries.get_mut(handle.invocation_id()) else {
            return Err(AgentError::UnknownHandle {
                invocation_id: handle.invocation_id().clone(),
            });
        };
        if entry.terminal.is_some() {
            // Idempotent: the typed no-op, never a second effect (a second
            // cancel never reaches the process seam).
            return Err(AgentError::AlreadyTerminal {
                invocation_id: handle.invocation_id().clone(),
                state: status_state(entry.terminal.as_ref()),
            });
        }
        // The FIRST cancel performs the cancellation: stop the child
        // under the CALLER'S bound, then pin Cancelled. A failed stop
        // leaves the state unpinned so the cancel can be retried.
        self.stop_child(entry, deadline)?;
        entry.terminal = Some(InvocationStatus::Cancelled);
        Ok(())
    }

    fn capabilities(&self) -> RunnerCapabilities {
        // Honest by construction (§7.3, RES-D7): file delivery — the
        // contract is the file; no native subagents — this adapter spawns
        // exactly one child and no more; model selection is a capability
        // ONLY when the configured template actually uses {model}; depth
        // is the configured value. The isolation rung is DERIVED from the
        // composition, never aspirational: with the containment seam
        // installed every policy-carrying child starts INSIDE the sandbox
        // (its first process — plan Task 25, AK-048), which is the
        // process-isolation rung; without the seam the child runs with
        // this process's permissions (isolation none), exactly as the
        // §8.6 conservative posture assumes.
        RunnerCapabilities {
            supports_native_subagents: false,
            supports_streaming_events: false,
            supports_model_selection: self.template.uses_model(),
            isolation: if self.containment.is_some() {
                Isolation::Process
            } else {
                Isolation::None
            },
            result_delivery: ResultDelivery::File,
            max_depth: self.config.max_depth,
        }
    }
}

#[cfg(test)]
mod tests {
    // The suite doubles are deliberately single-threaded (the mock's own
    // posture: one runner replays one child, driven from one thread), so
    // every `Arc<dyn Clock>` handoff below is knowingly non-Send/Sync.
    #![allow(clippy::arc_with_non_send_sync)]

    use super::*;
    use crate::config::CliAgentConfig;
    use crate::testing::{SUITE_TEMPLATE, ScriptedProcessTransport, SteppingClock};
    use gauntlet_domain::{
        Budget, DiagnosticAllowance, Digest, IterationNo, Limits, MicroUsd, PermissionSetRef,
        ResultContractPath, RoleId, RunId, WorkspaceRef,
    };
    use gauntlet_ports::agent_runner::{ContextEnvelope, InvocationFailure};

    const BASE_INSTANT: u64 = 1_700_000_000;

    fn spec() -> RoleInvocationSpec {
        let budget = Budget::new(
            Limits::new(8, 2, 2, 3600, 1000, MicroUsd::new(500)).expect("limits usable"),
            20,
            DiagnosticAllowance::new(100, MicroUsd::new(50)),
        )
        .expect("budget usable");
        RoleInvocationSpec {
            invocation_id: InvocationId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("valid id"),
            run_id: RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("valid id"),
            iteration_no: IterationNo::new(1).expect("1-based"),
            role: RoleId::Builder,
            prompt: "run the assigned slice and write the typed result".to_owned(),
            context_envelope: ContextEnvelope::new(
                "the runtime-built allowlisted context".to_owned(),
                Digest::new("sha256", "e5f6a7b8c4d3e2f1a0b1c2d3").expect("digest"),
            ),
            workspace: WorkspaceRef::new("ws-suite-01").expect("suite workspace ref"),
            permissions: PermissionSetRef::new("perm-suite-readonly")
                .expect("suite permission ref"),
            result_contract_path: ResultContractPath::new("suite-result-document")
                .expect("suite contract path"),
            deadline: Deadline::at_epoch(EpochSeconds::new(BASE_INSTANT + 30)),
            model_hint: None,
            sandbox_policy: None,
            budget_slice: budget,
        }
    }

    fn scratch(tag: &str) -> PathBuf {
        use std::sync::atomic::{AtomicU32, Ordering};
        static SEQUENCE: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "gauntlet-cli-adapter-runner-{tag}-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).expect("scratch creatable");
        dir
    }

    fn runner_with(
        script: gauntlet_ports::contract::agent_runner::RunnerScript,
    ) -> (CliAgentRunner<ScriptedProcessTransport>, PathBuf) {
        let transport = ScriptedProcessTransport::new(script);
        let state_dir = scratch("state");
        let runner = CliAgentRunner::new(
            CliAgentConfig::new(SUITE_TEMPLATE),
            state_dir.clone(),
            scratch("ws"),
            transport,
            Arc::new(SteppingClock::new()),
        )
        .expect("the suite template parses");
        (runner, state_dir)
    }

    /// The classification's total order, shape by shape, through the
    /// doubles and the REAL files (RES-D8a): completing children complete;
    /// malformed and mismatched documents classify by the contract; an
    /// exited child with no document is `Failed{missing_result}` on the
    /// FIRST poll; a silent live child times out at the (virtual)
    /// deadline.
    #[test]
    fn classification_follows_the_total_order() {
        use gauntlet_ports::contract::agent_runner::RunnerScript::*;
        // Exited + no document ⇒ missing_result on the FIRST probe.
        let (runner, state_dir) = runner_with(ExitWithoutResult);
        let handle = runner.invoke(spec()).expect("admission");
        match runner.poll(&handle).expect("first poll") {
            InvocationStatus::Failed {
                class: InvocationFailure::MissingResult,
            } => {}
            other => panic!("shape 2 must be missing_result, got {other:?}"),
        }
        // The exit status is recorded in adapter forensics only.
        assert_eq!(
            runner.forensics(&handle).expect("forensics").exit_status,
            Some(Some(0))
        );
        // The invocation directory is RETAINED after the terminal state
        // (RES-D12), with the prompt, envelope, and handle siblings. The
        // contract file itself never existed — a dead child wrote nothing
        // — and the directory still records everything a resume needs.
        let inv_dir = state_dir
            .join("runs")
            .join("01ARZ3NDEKTSV4RRFFQ69G5FAV")
            .join("inv")
            .join("01ARZ3NDEKTSV4RRFFQ69G5FAV");
        for name in [
            "suite-result-document.envelope.json",
            "suite-result-document.prompt.txt",
            "suite-result-document.handle.json",
        ] {
            assert!(inv_dir.join(name).is_file(), "{name} must be retained");
        }
        assert!(
            !inv_dir.join("suite-result-document").exists(),
            "no contract document exists for a child that never wrote one"
        );
        // The handle record carries the child identifier + identity.
        let handle_doc = std::fs::read_to_string(inv_dir.join("suite-result-document.handle.json"))
            .expect("handle record readable");
        assert!(
            handle_doc.contains("01ARZ3NDEKTSV4RRFFQ69G5FAV"),
            "{handle_doc}"
        );
        assert!(handle_doc.contains("child"), "{handle_doc}");
    }

    /// An EXITED child whose document IS valid and matching completes:
    /// the contract wins, the exit status is never obeyed (RES-D8a/b) —
    /// the child dies with a deliberately hostile nonzero exit.
    #[test]
    fn a_matching_contract_beats_the_exit_status() {
        use gauntlet_ports::contract::agent_runner::RunnerScript;
        let transport = ScriptedProcessTransport::new(RunnerScript::CompleteImmediately);
        transport.set_exit_code(7);
        let runner = CliAgentRunner::new(
            CliAgentConfig::new(SUITE_TEMPLATE),
            scratch("exit7-state"),
            scratch("exit7-ws"),
            transport,
            Arc::new(SteppingClock::new()),
        )
        .expect("the suite template parses");
        let handle = runner.invoke(spec()).expect("admission");
        assert!(
            matches!(
                runner.poll(&handle).expect("poll"),
                InvocationStatus::Completed { .. }
            ),
            "a valid matching contract completes despite the hostile exit"
        );
        // STRONGER than "never obeyed": the contract settled the verdict
        // at step 1 of the total order, so the process was never even
        // CONSULTED — the exit status never reached the classification.
        assert_eq!(
            runner.forensics(&handle).expect("forensics").exit_status,
            None,
            "the exit status is forensics only and was never a state input"
        );
    }

    /// A template that does not parse fails the CONSTRUCTION (typed
    /// configuration error), before any admission can happen.
    #[test]
    fn an_unparseable_template_refuses_construction() {
        let Err(error) = CliAgentRunner::new(
            CliAgentConfig::new("agent {fabricated}"),
            scratch("bad-template-state"),
            scratch("bad-template-ws"),
            ScriptedProcessTransport::new(
                gauntlet_ports::contract::agent_runner::RunnerScript::CompleteImmediately,
            ),
            Arc::new(SteppingClock::new()),
        ) else {
            panic!("an unknown placeholder must refuse construction");
        };
        assert!(
            error.to_string().contains("fabricated"),
            "the locus is the placeholder: {error}"
        );
    }

    /// `{model}` with no hint refuses AT ADMISSION (render time), leaving
    /// nothing on disk.
    #[test]
    fn a_model_template_without_a_hint_refuses_admission_cleanly() {
        let transport = ScriptedProcessTransport::new(
            gauntlet_ports::contract::agent_runner::RunnerScript::CompleteImmediately,
        );
        let state_dir = scratch("model-state");
        let ws = scratch("model-ws");
        let runner = CliAgentRunner::new(
            CliAgentConfig::new("agent {model}"),
            state_dir.clone(),
            ws.clone(),
            transport,
            Arc::new(SteppingClock::new()),
        )
        .expect("the template parses");
        let error = runner.invoke(spec()).expect_err("admission refuses");
        assert!(
            error.to_string().contains("model"),
            "the locus is the missing model value: {error}"
        );
        // Nothing was anchored: a refusal leaves no filesystem effects.
        assert!(
            !state_dir.join("runs").exists(),
            "a render refusal must leave nothing behind"
        );
    }

    /// Honest capabilities: model selection EXACTLY when the template
    /// uses {model}; no containment; file delivery; configured depth.
    #[test]
    fn capabilities_are_derived_from_configuration() {
        let (runner, _) =
            runner_with(gauntlet_ports::contract::agent_runner::RunnerScript::CompleteImmediately);
        let caps = runner.capabilities();
        assert!(!caps.supports_native_subagents);
        assert!(!caps.supports_streaming_events);
        assert!(
            !caps.supports_model_selection,
            "the suite template uses no model placeholder"
        );
        assert_eq!(caps.isolation, Isolation::None);
        assert_eq!(caps.result_delivery, ResultDelivery::File);
        assert_eq!(caps.max_depth, crate::config::DEFAULT_MAX_DEPTH);
    }

    /// COR-D3 teeth (red when bypassed): the child's cwd, the
    /// `{workspace}` template value, and the prompt's WORKSPACE field all
    /// anchor at the ADMITTED PER-UNIT workspace
    /// `<root>/<spec.workspace>/` — never at the shared root (§17.5.1
    /// invariant 1: builder processes run only inside their assigned
    /// workspace).
    #[test]
    fn execution_anchors_at_the_admitted_per_unit_workspace() {
        let transport = ScriptedProcessTransport::new(
            gauntlet_ports::contract::agent_runner::RunnerScript::CompleteImmediately,
        );
        let state_dir = scratch("anchor-state");
        let ws = scratch("anchor-ws");
        let runner = CliAgentRunner::new(
            CliAgentConfig::new(SUITE_TEMPLATE),
            state_dir.clone(),
            ws.clone(),
            transport.clone(),
            Arc::new(SteppingClock::new()),
        )
        .expect("the suite template parses");
        runner.invoke(spec()).expect("admission");
        let expected = ws.join("ws-suite-01");
        // The spawn's cwd IS the per-unit workspace, and it exists (the
        // child needs a real cwd).
        let cwds = transport.spawned_cwds();
        assert_eq!(cwds.len(), 1, "exactly one spawn");
        assert_eq!(
            cwds[0],
            expected.to_string_lossy(),
            "the child starts inside the admitted per-unit workspace"
        );
        assert!(expected.is_dir(), "the execution anchor exists");
        // The {workspace} placeholder carries the same anchor...
        let argv = &transport.spawned_argv()[0];
        assert_eq!(
            argv[2],
            expected.to_string_lossy(),
            "the template placeholder binds the per-unit workspace (SUITE_TEMPLATE word 3)"
        );
        // ...and so does the prompt file's WORKSPACE field (the prompt
        // lives at the adapter's documented anchor:
        // runs/<run>/inv/<invocation>/<leaf>.prompt.txt).
        let prompt = std::fs::read_to_string(
            state_dir
                .join("runs")
                .join("01ARZ3NDEKTSV4RRFFQ69G5FAV")
                .join("inv")
                .join("01ARZ3NDEKTSV4RRFFQ69G5FAV")
                .join("suite-result-document.prompt.txt"),
        )
        .expect("the prompt file reads");
        assert!(
            prompt.contains(&format!("WORKSPACE: {}", expected.display())),
            "the prompt's WORKSPACE field is the per-unit workspace: {prompt}"
        );
    }

    /// A workspace ref outside the single-component key class is a typed
    /// ADMISSION refusal — the anchor never joins a separator or
    /// traversal segment onto the root, and nothing is left behind.
    #[test]
    fn a_hostile_workspace_ref_refuses_admission_cleanly() {
        let transport = ScriptedProcessTransport::new(
            gauntlet_ports::contract::agent_runner::RunnerScript::CompleteImmediately,
        );
        let state_dir = scratch("hostile-state");
        let ws = scratch("hostile-ws");
        let runner = CliAgentRunner::new(
            CliAgentConfig::new(SUITE_TEMPLATE),
            state_dir.clone(),
            ws,
            transport,
            Arc::new(SteppingClock::new()),
        )
        .expect("the suite template parses");
        let mut hostile = spec();
        hostile.workspace =
            WorkspaceRef::new("seg/../escape").expect("the ref grammar admits the string");
        let error = runner.invoke(hostile).expect_err("admission refuses");
        assert!(
            error.to_string().contains("key class"),
            "the locus is the ref's key class: {error}"
        );
        assert!(
            !state_dir.join("runs").exists(),
            "a ref refusal leaves nothing behind"
        );
    }
}
