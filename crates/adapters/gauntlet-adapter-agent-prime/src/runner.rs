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
//!   harness control AND terminates the parent process (RC1 correction,
//!   below); a stranger handle is a typed [`AgentError::UnknownHandle`].
//!
//! **Parent process ownership (RC1 correction, task26-dcritic-apparatus-
//! repair).** [`crate::transport::KernelTransport::submit`] now returns the
//! live parent process alongside the admission handle, and THIS runner —
//! not the transport — owns its lifetime for as long as the invocation
//! lives (`PrimeEntry::parent`). This corrects a real, reproduced defect:
//! the transport used to dispose of the parent shortly after admission on
//! the premise that "a print run is disposable; its daemon-backed child
//! is what survives" — FALSE for prime-agent 0.8.0, whose rlm children
//! materialize and keep running ONLY while the parent session stays alive
//! (`program.rs`'s module docs, COR-D2 divergence #4). Killing the parent
//! aborted real in-flight child work (`Request was aborted`), the runner
//! then polled for a result that could never arrive, and the eventual
//! timeout's `stop` on a harness child that was never daemon-registered
//! failed and was (before the RC3 correction below) treated as fatal.
//! `poll` may [`crate::transport::ParentHandle::try_reap`] the parent
//! opportunistically — FORENSICS ONLY (`control_note`), never a
//! completion/failure signal (Annex C.2/C.4) — and every terminal
//! transition (`Completed`/`Failed` via the contract, `TimedOut` via the
//! deadline) and every `cancel` disposes of it
//! (`dispose_parent`/[`crate::transport::ParentHandle::terminate`]). If the
//! runner itself is dropped with a live entry still inside it, the
//! `Box<dyn ParentHandle>`'s own `Drop` (see
//! [`crate::transport::RealParentHandle`]) terminates the process rather
//! than leaking it — this runner adds no `Drop` impl of its own because
//! the ownership already lives one level down, in the handle type itself.
//! - `capabilities` is HONEST (§6.1: "A runner reporting `isolation: none`
//!   MUST NOT be selected for a tier requiring containment"): the kernel
//!   runs model-generated code with the worker's OS permissions — a
//!   control environment, not a security sandbox (Annex C.2) — so
//!   `isolation` is `None` unless something wraps this adapter, delivery
//!   is `File` (the contract), native subagents are real, streaming is
//!   not part of this adapter's rpc-mode surface, and `max_depth` is
//!   the configured value.
//!
//! **Execution anchoring (RC4 correction, task26-dcritic-apparatus-
//! repair).** `invoke` no longer starts every parent run in one fixed
//! `parent_cwd` injected at construction. R-1b (this runner's deferred
//! workspace resolution) was CLOSED for the rest of the system in Task 20
//! (`.agents/CURRENT_STATUS.yml`: "R-1b CLOSED both halves"), but this
//! runner was never brought along — a real run then went most of the way
//! (a correct `calculator.html`, its real `check.sh` PASSING) and still
//! failed, because the builder child wrote into the fixed parent-run cwd
//! (the project root) instead of its admitted per-unit workspace: the
//! filesystem facet observed `empty workspace: no entries` at the
//! allocated workspace, and verification then refused for want of a test
//! report. Worse, the project root holds `.wayfinder/answer-key.md`
//! (spec §8.5 check 7 / §22 item 5, invariant I1: the answer key must sit
//! outside every builder-writable workspace) — an INTEGRITY defect, not
//! merely a wrong directory, regardless of whether any given child
//! happens to touch it. `invoke` now resolves the parent's working
//! directory PER INVOCATION as the canonical workspace root joined with
//! the validated `spec.workspace` segment (`execution_root`, mirroring
//! [`gauntlet_adapter_agent_cli::runner`]'s COR-D3 anchor) — never a
//! fallback to the workspace root or the project root on refusal.
//!
//! **What this anchor can and cannot enforce.** The parent process's
//! `cwd` is set to the per-unit workspace, so the harness's `rlm(...)`
//! call — issued by that same in-process Python kernel — inherits that
//! cwd for any RELATIVE path a child resolves through it. But `rlm`
//! children are the harness's own recursive sessions: this adapter does
//! not control, and cannot observe from outside, whether a given
//! prime-agent version re-roots a child's own session cwd independently
//! of its parent's (e.g. from the child's own session directory or a
//! harness-internal default). Containment here is `isolation: none` per
//! Annex C.2 — a CONTROL environment, not a security boundary — so this
//! anchor is a strong steer for a cooperating child, never a guaranteed
//! confinement; only the composition's PER-DISPATCH facts (workspace
//! observation, the produced-artifact ladder) can catch a child that
//! still writes elsewhere.
//!
//! Time comes from the INJECTED `Clock` port — never the system clock
//! (the shared suite's pinned deadlines live in the past; only an
//! injected clock drives them deterministically). `poll` reads the clock
//! at most once per call for the deadline/`Running` path (after the
//! contract read), so child-side behaviors keyed to virtual time stay
//! deterministic under the suite; a Completed transition reads it once
//! more, for the measured payment's `wall_seconds` (task26-dcritic-
//! apparatus-repair W1) — the same RES-D12 pattern `admission_at` already
//! used at `invoke`.
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

use gauntlet_domain::{EpochSeconds, InvocationId, InvocationState, MicroUsd, Payment, SessionRef};
use gauntlet_ports::agent_runner::{
    AgentError, AgentRunner, CancelReason, InvocationHandle, InvocationStatus, Isolation,
    ResultDelivery, RoleInvocationSpec, RunnerCapabilities,
};
use gauntlet_ports::clock::Clock;
use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::role_result::{ResultPayment, RoleResult, read_result_contract};

use crate::config::PrimeAdapterConfig;
use crate::program::{KernelProgramSpec, compose_child_prompt, render};
use crate::transport::{
    ControlError, HarnessControl, KernelSubmission, KernelTransport, ParentHandle, ParentTelemetry,
    SubagentRecord, json_f64, json_str, json_u64,
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
    /// The live parent print-run process (RC1 correction,
    /// task26-dcritic-apparatus-repair): the INVOCATION owns this for as
    /// long as it lives. `poll` may opportunistically reap it for
    /// forensics only (`control_note`) — see the module docs — and it is
    /// terminated at every terminal transition (`dispose_parent`) and on
    /// `cancel`. It is NEVER consulted to decide `Completed`/`Failed`.
    parent: Box<dyn ParentHandle>,
    terminal: Option<InvocationStatus>,
    result: Option<RoleResult>,
    /// A control-operation note (e.g. a stop that failed at timeout, or a
    /// parent-process forensic) kept for forensics; never a state input.
    control_note: Option<String>,
}

/// Appends a forensic note to `entry.control_note`, joining with an
/// earlier note rather than overwriting it — every note stays readable
/// (Annex C.2: forensics, never control state).
fn append_control_note(entry: &mut PrimeEntry, note: String) {
    entry.control_note = Some(match entry.control_note.take() {
        Some(existing) => format!("{existing}; {note}"),
        None => note,
    });
}

/// The measured tokens the rpc telemetry supports without any file IO —
/// cheap enough to read on every `poll`: the parent's NEW tokens (input,
/// cache writes, output) plus each child's latest token_count. Cache
/// READS are not counted as tokens: a long session re-reads its context
/// every turn, so counting them would exhaust a token ceiling on reuse
/// alone; their price still reaches the cost dimension.
fn measured_tokens(telemetry: &ParentTelemetry) -> u64 {
    parent_new_tokens(telemetry).saturating_add(
        telemetry
            .children
            .values()
            .map(|child| child.token_count)
            .fold(0u64, u64::saturating_add),
    )
}

/// The parent's new tokens: uncached input, cache writes, and output.
fn parent_new_tokens(telemetry: &ParentTelemetry) -> u64 {
    telemetry
        .input_tokens
        .saturating_add(telemetry.cache_write_tokens)
        .saturating_add(telemetry.output_tokens)
}

/// The measured (parent tool executions + sum of latest child
/// tool_use_count) pair, the same cheap telemetry-only shape as
/// [`measured_tokens`].
fn measured_tool_calls(telemetry: &ParentTelemetry) -> u64 {
    telemetry.tool_execution_count.saturating_add(
        telemetry
            .children
            .values()
            .map(|child| child.tool_use_count)
            .fold(0u64, u64::saturating_add),
    )
}

/// One child session directory's summed assistant usage, from its
/// `*.jsonl` files (task26-dcritic-apparatus-repair W1: `collect`'s
/// measured payment sums child cost/tokens from these files — the rpc
/// stdout telemetry alone never carries per-child cost). `None` means the
/// scan found nothing usable (directory unreadable, or no `.jsonl` file
/// readable there) — the caller falls back to the cheaper telemetry
/// counts for that child rather than treating it as zero spend.
struct ChildSessionUsage {
    input_tokens: u64,
    output_tokens: u64,
    cost_usd: MicroUsd,
}

/// A single child session file larger than this is skipped rather than
/// read whole — bounded IO, never a load-bearing budget input.
const CHILD_SESSION_FILE_CAP_BYTES: u64 = 64 * 1024 * 1024;

fn scan_child_session_usage(session_dir: &str) -> Option<ChildSessionUsage> {
    let entries = std::fs::read_dir(session_dir).ok()?;
    let mut usage = ChildSessionUsage {
        input_tokens: 0,
        output_tokens: 0,
        cost_usd: MicroUsd::new(0),
    };
    let mut any_file_read = false;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("jsonl") {
            continue;
        }
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        if metadata.len() > CHILD_SESSION_FILE_CAP_BYTES {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        any_file_read = true;
        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let Ok(document) = gauntlet_ports::json::parse(trimmed) else {
                continue;
            };
            if json_str(&document, "type") != Some("message") {
                continue;
            }
            let Some(message) = document.get("message") else {
                continue;
            };
            if json_str(message, "role") != Some("assistant") {
                continue;
            }
            let Some(usage_value) = message.get("usage") else {
                continue;
            };
            usage.input_tokens += json_u64(usage_value, "input").unwrap_or(0);
            usage.input_tokens += json_u64(usage_value, "cacheWrite").unwrap_or(0);
            usage.output_tokens += json_u64(usage_value, "output").unwrap_or(0);
            if let Some(cost) = usage_value.get("cost")
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
                usage.cost_usd = MicroUsd::new(usage.cost_usd.get().saturating_add(micros));
            }
        }
    }
    any_file_read.then_some(usage)
}

/// The MEASURED payment at completion (task26-dcritic-apparatus-repair
/// W1): parent usage from the telemetry snapshot, plus every child's
/// usage — precisely from its session JSONL when that scan finds
/// something, falling back to the child's cheap telemetry counters
/// (`token_count` attributed to `input_tokens`, since the split is
/// unknown from `rlm_child_update` alone) when it does not. `tool_calls`
/// always comes from the cheap telemetry sum (no JSONL field carries it).
/// `wall_seconds` is `completed_at - admission_at`, saturating.
fn measured_payment(
    telemetry: &ParentTelemetry,
    admission_at: EpochSeconds,
    completed_at: EpochSeconds,
) -> ResultPayment {
    // Cache reads are priced (cost) but not counted as tokens — see
    // [`measured_tokens`].
    let mut input_tokens = telemetry
        .input_tokens
        .saturating_add(telemetry.cache_write_tokens);
    let mut output_tokens = telemetry.output_tokens;
    let mut cost_micros = telemetry.cost_usd.get();
    for child in telemetry.children.values() {
        match child
            .session_dir
            .as_deref()
            .and_then(scan_child_session_usage)
        {
            Some(usage) => {
                input_tokens = input_tokens.saturating_add(usage.input_tokens);
                output_tokens = output_tokens.saturating_add(usage.output_tokens);
                cost_micros = cost_micros.saturating_add(usage.cost_usd.get());
            }
            None => {
                input_tokens = input_tokens.saturating_add(child.token_count);
            }
        }
    }
    let wall_seconds = completed_at.get().saturating_sub(admission_at.get());
    ResultPayment::new(
        input_tokens,
        output_tokens,
        measured_tool_calls(telemetry),
        wall_seconds,
        MicroUsd::new(cost_micros),
    )
}

/// Disposes of the parent process at a terminal transition (RC1
/// correction): `try_reap` first (cheap, avoids an unnecessary stop
/// attempt on a parent that already exited on its own — e.g. its own
/// linger loop ended), falling back to `terminate`. The outcome is always
/// recorded as forensics; it is never a control-state input.
fn dispose_parent(entry: &mut PrimeEntry) {
    let note = match entry.parent.try_reap() {
        Some(note) => note,
        None => entry.parent.terminate(),
    };
    append_control_note(entry, format!("parent disposed: {note}"));
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
    /// The CANONICAL root the dispatch's per-unit workspaces live under —
    /// an INJECTED typed input from the composition root, the SAME root
    /// `gauntlet-adapter-agent-cli`'s runner anchors under (correction
    /// task26-dcritic-apparatus-repair, R-1b CLOSED per
    /// `.agents/CURRENT_STATUS.yml`: this adapter now resolves every
    /// invocation's parent working directory as `<workspace_root>/
    /// <spec.workspace>/`, never a fixed project-root cwd — see
    /// `execution_root` below, which mirrors
    /// `gauntlet_adapter_agent_cli::runner::execution_root`).
    workspace_root: PathBuf,
    transport: T,
    control: C,
    clock: Arc<dyn Clock>,
    entries: RefCell<HashMap<InvocationId, PrimeEntry>>,
}

/// The EXECUTION anchor for one dispatch (mirrors
/// `gauntlet_adapter_agent_cli::runner::execution_root`, plan Task 20
/// COR-D3, §17.5.1 invariant 1): the canonical workspace root joined with
/// the dispatch's `spec.workspace` segment — the per-unit workspace the
/// controller allocated and admitted the dispatch against, never the
/// project root the frozen answer key and its `.wayfinder/` live in
/// (spec §8.5 check 7 / §22 item 5, invariant I1: the answer key must sit
/// outside every builder-writable workspace). The ref is untrusted
/// external input: it must be a single key-class component
/// (`[A-Za-z0-9._-]`, the workspace provider's §17.5.3 class, re-derived
/// here because this adapter depends on domain and ports only) before it
/// may join, and the directory is ensured (the parent process needs a
/// real cwd; the provider normally created it before dispatch). Any
/// refusal is a typed `AdmissionRefused` — fail-closed at admission,
/// before the kernel program is ever submitted, nothing left behind, and
/// NEVER a fallback to the workspace root or the project root.
fn execution_root(
    workspace_root: &Path,
    workspace: &gauntlet_domain::WorkspaceRef,
) -> Result<PathBuf, AgentError> {
    let reference = workspace.as_str();
    // `.` and `..` are all-dot, slash-free, and pass the byte class below,
    // yet `root.join("..")` names the state dir itself — a single component
    // that still escapes. Refused by name, not by character class.
    let key_ok = !reference.is_empty()
        && reference != "."
        && reference != ".."
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
            "the per-unit workspace {} could not be ensured as the parent run's execution \
             anchor ({error})",
            root.display()
        ),
    })?;
    Ok(root)
}

impl<T: KernelTransport, C: HarnessControl> PrimeAgentRunner<T, C> {
    /// A runner over the supplied seams, anchoring invocation IO under
    /// `state_dir` and starting each parent run inside that invocation's
    /// admitted per-unit workspace under `workspace_root` (both
    /// composition-supplied: this adapter holds no absolute path literal).
    pub fn new(
        config: PrimeAdapterConfig,
        state_dir: PathBuf,
        workspace_root: PathBuf,
        transport: T,
        control: C,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            config,
            state_dir,
            workspace_root,
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
        // The EXECUTION anchor (task26-dcritic-apparatus-repair, R-1b
        // CLOSED): the admitted per-unit workspace the parent process
        // starts in — resolved and validated BEFORE any filesystem effect
        // or kernel submission, so a hostile/escaping `spec.workspace` is a
        // typed refusal with zero spawn (never a fallback to the project
        // root, which must stay outside every builder-writable workspace).
        let execution_root = execution_root(&self.workspace_root, &spec.workspace)?;
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
        let program = render(
            &KernelProgramSpec {
                envelope_path: files.envelope_file.to_string_lossy().into_owned(),
                child_input_path: files.child_input_file.to_string_lossy().into_owned(),
                handle_path: files.handle_file.to_string_lossy().into_owned(),
                invocation_name: name.clone(),
                role_prompt: prompt,
                model_hint: spec
                    .model_hint
                    .as_ref()
                    .map(|model| model.as_str().to_owned()),
            },
            self.config().spawn_dialect,
        );
        // ADMISSION ONLY (RES-D1): the transport watches for the HANDLE
        // FILE, parses it, and returns the four admission fields TOGETHER
        // WITH the live parent process (RC1 correction,
        // task26-dcritic-apparatus-repair) — never the parent run's
        // lifetime collapsed into admission, never a child answer. The
        // parent's disposal is THIS runner's job from here on (see
        // `dispose_parent` and the module docs), never the transport's.
        let submission = KernelSubmission {
            program,
            handle_path: files.handle_file,
            workspace: execution_root,
            model_hint: spec
                .model_hint
                .as_ref()
                .map(|model| model.as_str().to_owned()),
        };
        let (admission, parent) = match self.transport.submit(&submission, spec.deadline) {
            Ok(pair) => pair,
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
                parent,
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
        // RC1 correction (task26-dcritic-apparatus-repair): an
        // opportunistic, non-blocking reap of the parent, FORENSICS ONLY
        // (Annex C.2/C.4) — a parent that already exited (on its own, via
        // its own bounded result-poll loop ending, or otherwise) is
        // recorded, but NEVER read as completion or failure. The ONE
        // state input stays the result contract, read next.
        if let Some(note) = entry.parent.try_reap() {
            append_control_note(entry, format!("parent exited early: {note}"));
        }
        // The telemetry snapshot (task26-dcritic-apparatus-repair W1):
        // read once here — cheap (an `Arc<Mutex<_>>` clone), used below
        // for BOTH the Completed measured-payment installation and the
        // Running `payment_so_far`/`last_event_at`. It is METERING DATA
        // ONLY; nothing branches admission/completion/timeout on it
        // (Annex C.2/C.4 — the ONE state input stays the result contract,
        // read next).
        let telemetry = entry.parent.telemetry();
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
            if matches!(terminal, InvocationStatus::Completed { .. })
                && let Some(result) = entry.result.take()
            {
                // The measured payment is installed HERE — while the
                // parent is still live and its telemetry still valid —
                // BEFORE `dispose_parent` below (task26-dcritic-apparatus-
                // repair W1: "take telemetry BEFORE dispose_parent").
                let completed_at = self.clock.now();
                let measured = measured_payment(&telemetry, entry.admission_at, completed_at);
                let reported = result.payment();
                let combined = ResultPayment::new(
                    reported.input_tokens().max(measured.input_tokens()),
                    reported.output_tokens().max(measured.output_tokens()),
                    reported.tool_calls().max(measured.tool_calls()),
                    reported.wall_seconds().max(measured.wall_seconds()),
                    MicroUsd::new(reported.cost_usd().get().max(measured.cost_usd().get())),
                );
                entry.result = Some(result.with_payment(combined));
            }
            // The parent's job ended when the result contract landed
            // (RC1): dispose of it now rather than leave it running.
            dispose_parent(entry);
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
            // The parent's own bounded loop is itself bounded by this
            // same deadline, but dispose of it explicitly rather than
            // wait for it to notice on its own (RC1).
            dispose_parent(entry);
            let terminal = InvocationStatus::TimedOut;
            entry.terminal = Some(terminal.clone());
            return Ok(terminal);
        }
        // RES-D12 (revised, task26-dcritic-apparatus-repair W1 addendum):
        // `last_event_at` is the LATEST of three floors — the
        // result-contract path's mtime, the rpc telemetry's last parsed
        // stdout event (any type, including `rlm_child_update` — a
        // builder still emitting child telemetry is not stalled even
        // though it has not yet written a contract file), and the
        // admission instant. Without the telemetry floor a real builder
        // working past the controller's stall window (liveness addendum)
        // would read as stalled and be cancelled mid-work.
        let contract_event_at = std::fs::symlink_metadata(&entry.contract_file)
            .ok()
            .and_then(|meta| meta.modified().ok())
            .and_then(|modified| modified.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|since| EpochSeconds::new(since.as_secs()));
        let telemetry_event_at =
            (telemetry.last_event_epoch > 0).then(|| EpochSeconds::new(telemetry.last_event_epoch));
        let last_event_at = [
            contract_event_at,
            telemetry_event_at,
            Some(entry.admission_at),
        ]
        .into_iter()
        .flatten()
        .max()
        .unwrap_or(entry.admission_at);
        // `payment_so_far` is now MEASURED (task26-dcritic-apparatus-
        // repair W1): tokens/tool_calls are the cheap telemetry sums
        // (parent + latest per-child counters — no file IO on the poll
        // hot path), wall is since admission, and cost is the PARENT's
        // measured cost only (child cost needs the bounded session-JSONL
        // scan, done once, at `collect`, never on every poll).
        let payment_so_far = Payment::new(
            measured_tokens(&telemetry),
            now.get().saturating_sub(entry.admission_at.get()),
            measured_tool_calls(&telemetry),
            telemetry.cost_usd,
        );
        Ok(InvocationStatus::Running {
            last_event_at,
            payment_so_far,
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
        // RC1: cancel also terminates the PARENT process — an
        // already-gone child (the common case: prime-agent 0.8.0 never
        // daemon-registers these rlm children, so `stop_child` above
        // routinely resolves via the RC3 `NoSuchChild` idempotency path)
        // must not leave the parent running.
        dispose_parent(entry);
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

    // -----------------------------------------------------------------
    // task26-dcritic-apparatus-repair W1: measured payment (telemetry-
    // backed `payment_so_far`, and `collect`'s max(reported, measured)).
    // -----------------------------------------------------------------

    use crate::transport::{AdmissionHandle, ChildTelemetry, TransportError};

    /// A [`ParentHandle`] double that reports a FIXED telemetry snapshot —
    /// never a real process — so these tests never spawn anything.
    #[derive(Debug, Clone)]
    struct TelemetryParentHandle {
        telemetry: ParentTelemetry,
    }

    impl ParentHandle for TelemetryParentHandle {
        fn try_reap(&mut self) -> Option<String> {
            None
        }

        fn terminate(&mut self) -> String {
            "no-op: telemetry double".to_owned()
        }

        fn telemetry(&self) -> ParentTelemetry {
            self.telemetry.clone()
        }
    }

    /// A [`KernelTransport`] double that admits immediately and hands back
    /// [`TelemetryParentHandle`] — the admission side effects the real
    /// transport performs (spawning, writing/parsing a handle file) are
    /// not this test's concern; only telemetry propagation is.
    #[derive(Debug)]
    struct TelemetryTransport {
        telemetry: ParentTelemetry,
    }

    impl KernelTransport for TelemetryTransport {
        fn submit(
            &self,
            _submission: &KernelSubmission,
            _deadline: Deadline,
        ) -> Result<(AdmissionHandle, Box<dyn ParentHandle>), TransportError> {
            Ok((
                AdmissionHandle {
                    rlm_child_id: "sub-telemetry01".to_owned(),
                    name: "telemetry-invocation".to_owned(),
                    session_dir: "/sessions/telemetry01".to_owned(),
                    model: "fake-model".to_owned(),
                },
                Box::new(TelemetryParentHandle {
                    telemetry: self.telemetry.clone(),
                }) as Box<dyn ParentHandle>,
            ))
        }
    }

    #[derive(Debug, Default)]
    struct NoopControl;

    impl HarnessControl for NoopControl {
        fn stop(&self, _child: &str, _deadline: Deadline) -> Result<(), ControlError> {
            Ok(())
        }

        fn list(&self, _deadline: Deadline) -> Result<Vec<SubagentRecord>, ControlError> {
            Ok(Vec::new())
        }
    }

    /// A deterministic clock advancing by ONE second per `now()` call,
    /// starting at [`crate::test_support::BASE_INSTANT`] — well under
    /// `dispatch_spec()`'s deadline (`BASE_INSTANT + 30`), so these tests
    /// never trip a timeout.
    #[derive(Debug, Default)]
    struct StepClock {
        calls: std::sync::atomic::AtomicU64,
    }

    impl Clock for StepClock {
        fn now(&self) -> EpochSeconds {
            let next = self
                .calls
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                + 1;
            EpochSeconds::new(crate::test_support::BASE_INSTANT + next)
        }

        fn sleep_until(&self, _at: EpochSeconds) -> Result<(), gauntlet_ports::clock::ClockError> {
            Ok(())
        }
    }

    /// A fixed telemetry snapshot: nonzero parent usage, one child whose
    /// `session_dir` is absent so [`scan_child_session_usage`] falls back
    /// (proving the fallback path, not only the JSONL-scan path).
    fn fixed_telemetry() -> ParentTelemetry {
        let mut children = std::collections::HashMap::new();
        children.insert(
            "sub-child1".to_owned(),
            ChildTelemetry {
                status: "done".to_owned(),
                token_count: 30,
                tool_use_count: 1,
                duration_ms: 500,
                session_dir: None,
            },
        );
        ParentTelemetry {
            input_tokens: 40,
            output_tokens: 20,
            cache_read_tokens: 5,
            cache_write_tokens: 5,
            total_tokens: 70,
            cost_usd: MicroUsd::new(1234),
            tool_execution_count: 2,
            children,
            ..ParentTelemetry::default()
        }
    }

    fn scratch(tag: &str) -> PathBuf {
        use std::sync::atomic::{AtomicU32, Ordering};
        static SEQUENCE: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "gauntlet-prime-runner-{tag}-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).expect("the runner test scratch is creatable");
        dir
    }

    /// `poll`'s `Running.payment_so_far` is now MEASURED and nonzero:
    /// parent totalTokens + the one child's token_count, parent tool
    /// executions + the child's tool_use_count, and the parent's cost.
    #[test]
    fn poll_reports_nonzero_measured_payment_so_far() {
        let runner = PrimeAgentRunner::new(
            PrimeAdapterConfig::default(),
            scratch("payment-state"),
            scratch("payment-cwd"),
            TelemetryTransport {
                telemetry: fixed_telemetry(),
            },
            NoopControl,
            Arc::new(StepClock::default()),
        );
        let spec = dispatch_spec();
        let handle = runner.invoke(spec).expect("the telemetry double admits");
        let status = runner.poll(&handle).expect("poll succeeds");
        let InvocationStatus::Running { payment_so_far, .. } = status else {
            panic!("expected Running (no contract file written), got {status:?}")
        };
        assert_eq!(
            payment_so_far.tokens(),
            95,
            "65 new parent tokens (40 input + 5 cache write + 20 output) + 30 child"
        );
        assert_eq!(payment_so_far.tool_calls(), 3, "2 parent + 1 child");
        assert_eq!(payment_so_far.cost_usd(), MicroUsd::new(1234));
    }

    /// `collect`'s payment is the per-dimension max(reported, measured):
    /// a document reporting an ALL-ZERO payment is completed against
    /// nonzero telemetry, so every dimension of the collected payment
    /// comes from the measured side.
    #[test]
    fn collect_installs_the_max_of_reported_and_measured_payment() {
        let state_dir = scratch("collect-state");
        let runner = PrimeAgentRunner::new(
            PrimeAdapterConfig::default(),
            state_dir.clone(),
            scratch("collect-cwd"),
            TelemetryTransport {
                telemetry: fixed_telemetry(),
            },
            NoopControl,
            Arc::new(StepClock::default()),
        );
        let spec = dispatch_spec();
        let files = invocation_dir(&state_dir, &spec).expect("the leaf token resolves");
        let handle = runner
            .invoke(spec.clone())
            .expect("the telemetry double admits");
        // A validated document reporting an ALL-ZERO payment.
        let document = format!(
            "{{\"schema\":\"role_result@v1\",\"invocation_id\":\"{id}\",\"run_id\":\"{run}\",\
             \"iteration_no\":1,\"role\":\"builder\",\"context_envelope_digest\":\"{digest}\",\
             \"state\":\"completed\",\"payload_schema\":\"builder_result@v1\",\"payload\":{{\
             \"attempted\":[],\"changed_paths\":[],\"commands_run\":[],\"artifacts\":[],\
             \"claims\":[],\"self_checks\":[],\"blocked_on\":[]}},\"payment\":{{\
             \"input_tokens\":0,\"output_tokens\":0,\"tool_calls\":0,\"wall_seconds\":0,\
             \"cost_usd\":0}},\"unavailable_evidence\":[],\"errors\":[]}}",
            id = spec.invocation_id.as_str(),
            run = spec.run_id.as_str(),
            digest = spec.context_envelope.digest(),
        );
        std::fs::write(&files.contract_file, document).expect("the contract document is writable");
        let status = runner.poll(&handle).expect("poll classifies the document");
        assert!(
            matches!(status, InvocationStatus::Completed { .. }),
            "got {status:?}"
        );
        let result = runner
            .collect(&handle, Deadline::at_epoch(EpochSeconds::new(u64::MAX / 2)))
            .expect("a Completed invocation is collectible");
        let payment = result.payment();
        assert_eq!(
            payment.input_tokens(),
            75,
            "(40 input + 5 cache write parent) + 30 child fallback; cache reads are not tokens"
        );
        assert_eq!(
            payment.output_tokens(),
            20,
            "parent only — no child JSONL scan hit"
        );
        assert_eq!(payment.tool_calls(), 3, "2 parent + 1 child");
        assert_eq!(payment.cost_usd(), MicroUsd::new(1234));
        assert!(
            payment.wall_seconds() >= 1,
            "wall must be measured, not the reported 0"
        );
        std::fs::remove_dir_all(&state_dir).ok();
    }
}
