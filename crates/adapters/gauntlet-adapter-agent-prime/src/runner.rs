//! The prime-agent `AgentRunner` (spec §6.1, §10.5, Annex C; plan Task 15):
//! a REAL admission-based runner over the harness's recursive sub-agent
//! mechanism.
//!
//! The state machine has ONE input, and it is not the harness: every state
//! transition derives from the declared result contract at the invocation's
//! resolved path (§10.3), read through
//! [`read_result_contract`](gauntlet_ports::role_result::read_result_contract)
//! — strict decode plus the §10.3 rule-6 identity check, failures mapped by
//! [`failure_class`](gauntlet_ports::role_result::RoleResultError::failure_class).
//!
//! - `invoke` renders the Annex C.1 kernel program, writes the runtime
//!   envelope beside the contract path, submits through the kernel
//!   transport, parses the ADMISSION handle (the transport's type cannot
//!   carry a child answer), records `session_dir` as `session_ref` only
//!   (Annex C.2), and returns — never blocking on the child (§6.1).
//! - `poll` classifies ONLY from the result contract and the spec's
//!   deadline: valid matching document ⇒ `Completed`; malformed ⇒
//!   `Failed{malformed_result}`; mismatched identity ⇒
//!   `Failed{contract_mismatch}`; deadline passed without a document ⇒
//!   `TimedOut` (and cancel, preserving `session_ref`, §10.3 rule 5);
//!   otherwise `Running`. A MAILBOX REPLY IS NEVER CONSULTED — there is no
//!   code path from any mailbox signal into this state machine (§10.5:
//!   "Treat a mailbox message with no valid result file as `Running`"). A
//!   missing live process is likewise never a state input (Annex C.4:
//!   idle children unload and reload; liveness is forensics only).
//! - `collect` returns the validated document after `Completed`, the same
//!   document on every collect.
//! - `cancel` is idempotent; the first cancel stops the child through the
//!   harness control; a stranger handle is a typed
//!   [`AgentError::UnknownHandle`].
//! - `capabilities` is HONEST (§6.1: "A runner reporting `isolation: none`
//!   MUST NOT be selected for a tier requiring containment"): the kernel
//!   runs model-generated code with the worker's OS permissions — a
//!   control environment, not a security sandbox (Annex C.2) — so
//!   `isolation` is `None` unless something wraps this adapter, delivery
//!   is `File` (the contract), native subagents are real, streaming is
//!   not part of this adapter's print-mode surface, and `max_depth` is
//!   the configured value.
//!
//! Time comes from the INJECTED `Clock` port — never the system clock
//! (the shared suite's pinned deadlines live in the past; only an
//! injected clock drives them deterministically). `poll` reads the clock
//! exactly once per call, after the contract read, so child-side
//! behaviors keyed to virtual time stay deterministic under the suite.
//!
//! The result contract is read in exactly two places, `poll` and
//! `collect` (the crate-wide await-ban arch test pins this). The file
//! resolution is containment-checked: a contract path that escapes the
//! composition-supplied state root is a typed refusal at admission
//! (the mock adapter's two-guard posture).

use std::cell::RefCell;
use std::collections::HashMap;
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

use crate::config::PrimeAdapterConfig;
use crate::program::{KernelProgramSpec, compose_child_prompt, render};
use crate::transport::{
    ControlError, HarnessControl, KernelSubmission, KernelTransport, SubagentRecord,
};

/// One admitted invocation's live state.
struct PrimeEntry {
    spec: RoleInvocationSpec,
    /// The contract document's resolved file (under the invocation
    /// directory).
    contract_file: PathBuf,
    /// The harness child identifier from the admission handle.
    child: String,
    /// The deterministic `rlm(name=…)` invocation name — the orphan
    /// sweep searches the harness by it.
    invocation_name: String,
    /// `session_dir` as `session_ref` — forensic evidence only (Annex C.2).
    session_ref: Option<SessionRef>,
    /// The admission instant (the `last_event_at` fallback — RES-D12).
    admission_at: EpochSeconds,
    terminal: Option<InvocationStatus>,
    result: Option<RoleResult>,
    /// A control-operation note (e.g. a stop that failed at timeout) kept
    /// for forensics; never a state input.
    control_note: Option<String>,
}

/// The ONE filesystem anchor rule (correction RES-D3): `invoke` creates
/// `<state_dir>/runs/<run_id>/inv/<invocation_id>/`; the opaque
/// `ResultContractPath` token resolves as the LITERAL LEAF inside that
/// directory; the envelope, child-input, and handle files are derived
/// siblings. A token that is not a single leaf component (absolute,
/// parent-directory segments, nested paths) is a dispatch defect, refused
/// at admission.
struct InvocationDir {
    dir: PathBuf,
    contract_file: PathBuf,
    envelope_file: PathBuf,
    child_input_file: PathBuf,
    handle_file: PathBuf,
}

fn invocation_dir(
    state_dir: &Path,
    spec: &RoleInvocationSpec,
) -> Result<InvocationDir, AgentError> {
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
    // The ids are validated ULIDs and the key a single component, so the
    // join cannot escape the state root; the check stays as the second
    // guard (the mock adapter's two-guard posture).
    if !dir.starts_with(state_dir) {
        return Err(refused(
            "the invocation directory does not resolve under the state root".to_owned(),
        ));
    }
    Ok(InvocationDir {
        contract_file: dir.join(path),
        envelope_file: dir.join(format!("{key}.envelope.json")),
        child_input_file: dir.join(format!("{key}.child-input.json")),
        handle_file: dir.join(format!("{key}.handle.json")),
        dir,
    })
}

/// The invocation name the kernel program passes as `rlm(name=…)`, per
/// Annex C.1: "<role>-<unit_or_slice>-<invocation_id_short>". The dispatch
/// spec carries no unit id, so the run id stands in for the slice: role,
/// run, and invocation stay distinguishable and unique.
fn invocation_name(spec: &RoleInvocationSpec) -> String {
    let short = |ulid: &str| {
        let tail: String = ulid.chars().rev().take(6).collect();
        tail.chars().rev().collect::<String>()
    };
    format!(
        "{}-{}-{}",
        spec.role,
        short(spec.run_id.as_str()),
        short(spec.invocation_id.as_str())
    )
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

/// One admitted invocation's forensic record — evidence, never control
/// state (I11, Annex C.2). See [`PrimeAgentRunner::forensic_record`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvocationForensics {
    pub invocation_name: String,
    pub session_ref: Option<SessionRef>,
    pub control_note: Option<String>,
}

/// The real prime-agent runner. Generic over the two harness seams; the
/// production composition binds [`crate::transport::ProcessKernelTransport`]
/// and [`crate::transport::ProcessHarnessControl`]; the shared contract
/// suite binds in-memory doubles that execute the rendered program's
/// semantics (spec §6.3: the harness is what gets doubled, not the
/// adapter's logic).
pub struct PrimeAgentRunner<T: KernelTransport, C: HarnessControl> {
    config: PrimeAdapterConfig,
    state_dir: PathBuf,
    /// The parent print run's working directory — an INJECTED typed input
    /// from the composition root (correction RES-D3: this adapter never
    /// resolves a `WorkspaceRef` into a path; production workspace
    /// resolution is unreachable this task, R-1b's territory).
    parent_cwd: PathBuf,
    transport: T,
    control: C,
    clock: Arc<dyn Clock>,
    entries: RefCell<HashMap<InvocationId, PrimeEntry>>,
}

impl<T: KernelTransport, C: HarnessControl> PrimeAgentRunner<T, C> {
    /// A runner over the supplied seams, anchoring invocation IO under
    /// `state_dir` and starting parent runs in `parent_cwd` (both
    /// composition-supplied: this adapter holds no absolute path
    /// literal and resolves no workspace reference).
    pub fn new(
        config: PrimeAdapterConfig,
        state_dir: PathBuf,
        parent_cwd: PathBuf,
        transport: T,
        control: C,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            config,
            state_dir,
            parent_cwd,
            transport,
            control,
            clock,
            entries: RefCell::new(HashMap::new()),
        }
    }

    /// The configured runner capabilities — the honest report, stable for
    /// the runner's lifetime.
    pub fn config(&self) -> &PrimeAdapterConfig {
        &self.config
    }

    /// The invocation's forensic record (§10.3 rule 5: the timeout path
    /// "preserves the session reference for forensics"): the preserved
    /// session reference, the deterministic invocation name the orphan
    /// sweep would search by, and any control note. FOR EVIDENCE ONLY —
    /// never control state (I11, Annex C.2).
    pub fn forensic_record(&self, handle: &InvocationHandle) -> Option<InvocationForensics> {
        let entries = self.entries.borrow();
        let entry = entries.get(handle.invocation_id())?;
        Some(InvocationForensics {
            invocation_name: entry.invocation_name.clone(),
            session_ref: entry.session_ref.clone(),
            control_note: entry.control_note.clone(),
        })
    }

    /// Stops the harness child, mapping the already-gone case to success
    /// (cancellation's goal is achieved). A real failure is returned.
    /// The bound is the caller's: `cancel` passes the port's deadline;
    /// the timeout path bounds a fresh stop past now.
    fn stop_child(&self, entry: &mut PrimeEntry, bound: Deadline) -> Result<(), AgentError> {
        match self.control.stop(&entry.child, bound) {
            Ok(()) => Ok(()),
            Err(error @ ControlError::NoSuchChild { .. }) => {
                entry.control_note = Some(error.to_string());
                Ok(())
            }
            Err(error) => Err(AgentError::CancelFailed {
                invocation_id: entry.spec.invocation_id.clone(),
                detail: error.to_string(),
            }),
        }
    }

    /// The RES-D4 orphan sweep: a best-effort search of the harness
    /// listing by the deterministic invocation name, stopping any child
    /// discovered. The outcome is appended to `detail` — recorded in the
    /// error, never swallowed.
    fn sweep_orphans(&self, name: &str, detail: &mut String) {
        let bound = Deadline::at_epoch(EpochSeconds::new(
            self.clock.now().get().saturating_add(SWEEP_SPAN_SECONDS),
        ));
        match self.control.list(bound) {
            Ok(records) => {
                let orphans: Vec<&SubagentRecord> = records
                    .iter()
                    .filter(|record| record.session_name == name)
                    .collect();
                if orphans.is_empty() {
                    detail.push_str(&format!(" orphan sweep: no harness child named {name:?}"));
                    return;
                }
                for orphan in orphans {
                    match self.control.stop(&orphan.session_name, bound) {
                        Ok(()) => detail.push_str(&format!(
                            " orphan sweep: stopped discovered child {:?}",
                            orphan.session_name
                        )),
                        Err(error) => detail.push_str(&format!(
                            " orphan sweep: stopping {:?} FAILED: {error}",
                            orphan.session_name
                        )),
                    }
                }
            }
            Err(error) => detail.push_str(&format!(
                " orphan sweep FAILED (the harness listing could not be consulted: {error})"
            )),
        }
    }
}

/// The cancel/stop bound spans, in seconds past now: bounded blocking
/// control operations, never unbounded ones (spec §6 preamble).
const CANCEL_SPAN_SECONDS: u64 = 60;
const SWEEP_SPAN_SECONDS: u64 = 60;

impl<T: KernelTransport, C: HarnessControl> AgentRunner for PrimeAgentRunner<T, C> {
    fn invoke(&self, spec: RoleInvocationSpec) -> Result<InvocationHandle, AgentError> {
        let invocation_id = spec.invocation_id.clone();
        // Duplicate admission is refused: resetting an entry would
        // resurrect terminal state or restart a script (the ports' pinned
        // terminal stability), so a known id is never re-admitted.
        if self.entries.borrow().contains_key(&invocation_id) {
            return Err(AgentError::AdmissionRefused {
                detail: format!(
                    "invocation {invocation_id} is already admitted to this runner; duplicate \
                     admission is refused and the existing entry is untouched"
                ),
            });
        }
        // RES-D3's one anchor rule: the invocation directory under the
        // state root, the contract token as the literal leaf.
        let files = invocation_dir(&self.state_dir, &spec)?;
        // §10.3 rule 1: the runtime creates the invocation directory and
        // writes the envelope; the handle file is a derived sibling.
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

        // The child prompt: role body + explicit result-contract
        // instruction (Annex C.1: the prompt carries the path and the
        // atomic-write protocol).
        let name = invocation_name(&spec);
        let prompt = compose_child_prompt(
            &spec.prompt,
            &files.contract_file,
            spec.invocation_id.as_str(),
            &spec.context_envelope.digest().to_string(),
        );
        let program = render(&KernelProgramSpec {
            envelope_path: files.envelope_file.to_string_lossy().into_owned(),
            child_input_path: files.child_input_file.to_string_lossy().into_owned(),
            handle_path: files.handle_file.to_string_lossy().into_owned(),
            invocation_name: name.clone(),
            role_prompt: prompt,
            model_hint: spec
                .model_hint
                .as_ref()
                .map(|model| model.as_str().to_owned()),
            // COR-D2 #4: the parent's cell lingers past admission so the
            // child materializes — the PARENT's linger, never a wait in
            // invoke (D1 untouched).
            linger_seconds: self.config.admission_linger_seconds,
        });
        // ADMISSION ONLY (RES-D1): the transport watches for the HANDLE
        // FILE, parses it, disposes of the parent run best-effort, and
        // returns the four admission fields — never the parent run's
        // lifetime, never a child answer.
        let submission = KernelSubmission {
            program,
            handle_path: files.handle_file,
            workspace: self.parent_cwd.clone(),
            model_hint: spec
                .model_hint
                .as_ref()
                .map(|model| model.as_str().to_owned()),
        };
        let admission = match self.transport.submit(&submission, spec.deadline) {
            Ok(handle) => handle,
            Err(error) => {
                // RES-D4 taxonomy: transport/binary failure ⇒ Transport
                // (no sweep — no child can exist); handle absent or
                // undecodable ⇒ AdmissionRefused WITH a best-effort
                // orphan sweep by the deterministic invocation name.
                let mut detail = match &error {
                    crate::transport::TransportError::SpawnFailed { .. } => {
                        return Err(AgentError::Transport {
                            detail: format!(
                                "admission of {invocation_id} failed at the transport: {error}"
                            ),
                        });
                    }
                    other => format!("admission of {invocation_id} refused: {other}"),
                };
                self.sweep_orphans(&name, &mut detail);
                return Err(AgentError::AdmissionRefused { detail });
            }
        };
        // The session reference (Annex C.2: `session_dir` as
        // `session_ref` only). The domain's `SessionRef` refuses
        // path-rooted tokens — a real harness session_dir IS a path — so
        // the adapter records it under an explicit, LOSSLESS scheme
        // prefix: `session-dir:<session_dir>` verbatim. Nothing is
        // stripped or coerced; forensics recover the exact path, and the
        // reference is never control state (I11).
        let session_ref = SessionRef::new(&format!("session-dir:{}", admission.session_dir))
            .map_err(|error| AgentError::AdmissionRefused {
                detail: format!(
                    "the harness reported an unusable session_dir {:?}: {error}",
                    admission.session_dir
                ),
            })?;
        let session_ref = Some(session_ref);
        // One clock read at admission: the Running payload's fallback
        // instant (RES-D12).
        let admission_at = self.clock.now();
        self.entries.borrow_mut().insert(
            invocation_id.clone(),
            PrimeEntry {
                spec,
                contract_file: files.contract_file,
                child: admission.rlm_child_id,
                invocation_name: name,
                session_ref: session_ref.clone(),
                admission_at,
                terminal: None,
                result: None,
                control_note: None,
            },
        );
        // Prompt return after admission; the child keeps running (the
        // harness's children are daemon-backed; Annex C.2).
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
        // The ONE state input: the result contract at the declared path.
        // Liveness is NOT consulted (Annex C.4); no mailbox is read; the
        // parent run's output is never parsed (see the transport docs).
        let classification = match std::fs::read_to_string(&entry.contract_file) {
            Ok(text) => {
                let outcome = read_result_contract(
                    &text,
                    &entry.spec.invocation_id,
                    entry.spec.context_envelope.digest(),
                );
                // Model output is untrusted external input: parse,
                // validate, admit — or classify whole (§10.3 rules 3/4/6).
                match outcome {
                    Ok(result) => {
                        entry.result = Some(result);
                        Some(InvocationStatus::Completed {
                            result_ref: entry.spec.result_contract_path.clone(),
                        })
                    }
                    Err(error) => Some(InvocationStatus::Failed {
                        class: error.failure_class(),
                    }),
                }
            }
            // No document YET: the deadline decides between Running and
            // TimedOut (§10.3 rule 5).
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => {
                // An unreadable contract is an observation failure, never
                // a coerced Running (§21.2: uncertainty is never success).
                return Err(AgentError::Transport {
                    detail: format!(
                        "the result contract at {} could not be read ({error})",
                        entry.contract_file.display()
                    ),
                });
            }
        };
        if let Some(terminal) = classification {
            entry.terminal = Some(terminal.clone());
            return Ok(terminal);
        }
        // One clock read per poll (see the module docs). A timeout stops
        // the child with a FRESH bound past that now — the stop is its
        // own bounded blocking operation (§6 preamble).
        let now = self.clock.now();
        if entry.spec.deadline.exceeded_at(now) {
            let stop_bound = Deadline::at_epoch(EpochSeconds::new(
                now.get().saturating_add(CANCEL_SPAN_SECONDS),
            ));
            // The deadline verdict is the runtime's (§10.3 rule 5); the
            // cancel is best-effort with its locus recorded — a stop that
            // fails does not un-time-out the invocation.
            if let Err(error) = self.stop_child(entry, stop_bound) {
                entry.control_note = Some(error.to_string());
            }
            let terminal = InvocationStatus::TimedOut;
            entry.terminal = Some(terminal.clone());
            return Ok(terminal);
        }
        // RES-D12: `last_event_at` is the last observed state change of
        // the result-contract path (mtime), falling back to the admission
        // instant. `payment_so_far` is ZERO and is NOT A MEASUREMENT:
        // child spend is not observable through this adapter's surface —
        // budget truth stays with the runtime's `BudgetMeter` and the
        // collected document's payment block (Annex B.1), never with this
        // value.
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
            // Idempotent: the typed no-op, never a second effect.
            return Err(AgentError::AlreadyTerminal {
                invocation_id: handle.invocation_id().clone(),
                state: status_state(entry.terminal.as_ref()),
            });
        }
        // The FIRST cancel performs the cancellation: stop the child
        // through the harness control under the CALLER'S bound, then pin
        // Cancelled. A failed stop leaves the state unpinned so the
        // cancel can be retried.
        self.stop_child(entry, deadline)?;
        entry.terminal = Some(InvocationStatus::Cancelled);
        Ok(())
    }

    fn capabilities(&self) -> RunnerCapabilities {
        // Honest by construction (Annex C.2): no containment — the kernel
        // runs with worker OS permissions; file delivery — the contract
        // is the file; native subagents are the mechanism itself; print
        // mode streams nothing; depth is the configured value.
        RunnerCapabilities {
            supports_native_subagents: true,
            supports_streaming_events: false,
            supports_model_selection: true,
            isolation: Isolation::None,
            result_delivery: ResultDelivery::File,
            max_depth: self.config.max_depth,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::dispatch_spec;
    use gauntlet_ports::agent_runner::InvocationFailure;

    /// The invocation name mapping (Annex C.1: "<role>-<unit_or_slice>-
    /// <invocation_id_short>"; the run id stands in for the slice).
    #[test]
    fn invocation_names_are_role_run_invocation_short() {
        let mut spec = dispatch_spec();
        spec.role = gauntlet_domain::RoleId::Critic;
        // The suite ULIDs end ...Q69G5FAV; the shorts are the last six
        // characters.
        assert_eq!(invocation_name(&spec), "critic-9G5FAV-9G5FAV");
    }

    /// The RES-D3 anchor: the contract token resolves as the literal leaf
    /// inside runs/<run_id>/inv/<invocation_id>/; anything else is refused.
    #[test]
    fn the_contract_token_is_a_leaf_inside_the_invocation_dir() {
        let spec = dispatch_spec();
        let files = invocation_dir(Path::new("/state"), &spec).expect("a leaf token resolves");
        let expected = Path::new("/state")
            .join("runs")
            .join(spec.run_id.as_str())
            .join("inv")
            .join(spec.invocation_id.as_str());
        assert_eq!(files.dir, expected);
        assert_eq!(files.contract_file, expected.join("suite-result-document"));
        assert_eq!(
            files.envelope_file,
            expected.join("suite-result-document.envelope.json")
        );
        assert_eq!(
            files.handle_file,
            expected.join("suite-result-document.handle.json")
        );
        // First guard: the DOMAIN type refuses absolute tokens outright.
        assert!(gauntlet_domain::ResultContractPath::new("/etc/passwd").is_err());
        // Second guard: this adapter refuses nested and parent-directory
        // tokens the type admits — the token must be a leaf.
        for hostile in ["../outside.json", "a/b.json"] {
            let mut hostile_spec = dispatch_spec();
            hostile_spec.result_contract_path =
                gauntlet_domain::ResultContractPath::new(hostile).expect("the type admits it");
            assert!(
                invocation_dir(Path::new("/state"), &hostile_spec).is_err(),
                "{hostile} must be refused"
            );
        }
    }

    /// The failure-class vocabulary stays reachable through the ports
    /// mapping this runner uses (pin the surface the await-ban arch test
    /// allows).
    #[test]
    fn the_failure_classes_come_from_the_ports_mapping() {
        assert_ne!(
            InvocationFailure::MalformedResult.to_string(),
            InvocationFailure::ContractMismatch.to_string()
        );
    }
}
