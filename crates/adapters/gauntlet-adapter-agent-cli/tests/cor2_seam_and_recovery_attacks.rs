//! COR-2 (task 25 corrections round 2) adapter-level teeth, driven
//! against the REAL types the composition binds. Round-1 falsifier
//! findings bound here:
//!
//! - F-R1: the no-seam fail-closed arm — a policy-carrying dispatch into
//!   a runner with NO containment seam installed must be a typed
//!   admission refusal with zero spawns and zero invocation artifacts
//!   (the delivered claim in `lib.rs` had zero executed evidence; the
//!   round-1 disabling mutant survived the entire battery).
//! - F-R3: a post-spawn refusal's cancel-for-recovery arm must retire
//!   the JUST-SPAWNED WRAPPED CHILD — the whole process tree, within the
//!   bounded cancel span — before the refusal surfaces (the round-1
//!   mutant-triggered path left a live wrapped pair + in-sandbox shell).
//!
//! Tiny bounded `sh` probe children only (the round-1 live-attack
//! posture: no agent binary, no model, no Unit dispatch, no authority
//! gate; every child bounded and cleaned up).

// The doubles are deliberately single-threaded (the mock runner's own
// posture): one runner instance is driven from one thread, so the Arcs
// that hand the transport/clock to the runner are knowingly non-Send/Sync.
#![allow(clippy::arc_with_non_send_sync)]

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use gauntlet_adapter_agent_cli::clock::SystemClock;
use gauntlet_adapter_agent_cli::config::CliAgentConfig;
use gauntlet_adapter_agent_cli::runner::{CliAgentRunner, CommandContainment, ContainmentRefusal};
use gauntlet_adapter_agent_cli::testing::{ScriptedProcessTransport, SteppingClock};
use gauntlet_adapter_agent_cli::transport::ProcessChildTransport;
use gauntlet_domain::{
    Budget, DiagnosticAllowance, Digest, EpochSeconds, InvocationId, IterationNo, Limits, MicroUsd,
    PermissionSetRef, ResultContractPath, RoleId, RunId, SandboxPolicyRef, WorkspaceRef,
};
use gauntlet_ports::agent_runner::{
    AgentError, AgentRunner, CancelReason, ContextEnvelope, InvocationHandle, InvocationStatus,
    RoleInvocationSpec, RunnerCapabilities,
};
use gauntlet_ports::contract::agent_runner::RunnerScript;
use gauntlet_ports::deadline::Deadline;

const ID_A: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
const RUN_ID: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
const POLICY_REF: &str = "tier-cross-cutting-real-v1";

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "gauntlet-cor2-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).expect("scratch creatable");
    dir
}

fn bound_from_now(seconds: u64) -> Deadline {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_secs();
    Deadline::at_epoch(EpochSeconds::new(now + seconds))
}

fn spec_with(id: &str, deadline_seconds: u64) -> RoleInvocationSpec {
    let budget = Budget::new(
        Limits::new(8, 2, 2, 3600, 1000, MicroUsd::new(500)).expect("limits"),
        20,
        DiagnosticAllowance::new(100, MicroUsd::new(50)),
    )
    .expect("budget");
    RoleInvocationSpec {
        invocation_id: InvocationId::new(id).expect("valid id"),
        run_id: RunId::new(RUN_ID).expect("valid id"),
        iteration_no: IterationNo::new(1).expect("1-based"),
        role: RoleId::Builder,
        prompt: "cor2 probe: write the typed result to the declared path".to_owned(),
        context_envelope: ContextEnvelope::new(
            "the runtime-built allowlisted context".to_owned(),
            Digest::new("sha256", "e5f6a7b8c4d3e2f1a0b1c2d3").expect("digest"),
        ),
        workspace: WorkspaceRef::new("ws-cor2").expect("workspace ref"),
        permissions: PermissionSetRef::new("perm-cor2").expect("permission ref"),
        result_contract_path: ResultContractPath::new("result.json").expect("contract path"),
        deadline: bound_from_now(deadline_seconds),
        model_hint: None,
        sandbox_policy: None,
        budget_slice: budget,
    }
}

fn files_under(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}

// ---------------------------------------------------------------------------
// F-R1: the no-seam fail-closed arm's teeth.
// ---------------------------------------------------------------------------

/// F-R1 (round-1 falsifier, MAJOR): a dispatch carrying an admitted
/// sandbox policy into a runner with NO containment seam installed is a
/// TYPED admission refusal — nothing spawns, and no invocation artifact
/// lands anywhere. This is the executed evidence the delivered lib.rs
/// claim ("a policy-carrying dispatch with no seam installed is a typed
/// admission refusal, never an unwrapped run") was missing: the round-1
/// disabling mutant (`(None, Some(_)) => argv` — run it unwrapped)
/// survived the ENTIRE delivered battery; this test is the locus that
/// kills it. The refusal must fire before any spawn, so no real sandbox
/// backend is needed — the double's spawn counter is the discriminator.
#[test]
fn fr1_a_policy_carrying_dispatch_without_the_seam_is_a_typed_refusal_that_spawns_nothing() {
    let transport = ScriptedProcessTransport::new(RunnerScript::CompleteImmediately);
    let state_dir = scratch("fr1-state");
    let ws = scratch("fr1-ws");
    // The historical template-only posture: `new()` WITHOUT
    // `with_containment` — exactly the shape the composed `improve` path
    // and the §8.6 conservative world bind.
    let runner = CliAgentRunner::new(
        CliAgentConfig::new("agent {prompt_file} {workspace} {result_path}"),
        state_dir.clone(),
        ws.clone(),
        transport.clone(),
        Arc::new(SteppingClock::new()),
    )
    .expect("the template parses");
    let mut spec = spec_with(ID_A, 30);
    spec.sandbox_policy = Some(SandboxPolicyRef::new(POLICY_REF).expect("policy ref"));

    let error = runner
        .invoke(spec)
        .expect_err("a policy-carrying dispatch with no seam must be refused, never run unwrapped");
    assert!(
        matches!(error, AgentError::AdmissionRefused { .. }),
        "the refusal is the typed ADMISSION class: {error:?}"
    );
    let detail = error.to_string();
    assert!(
        detail.contains(POLICY_REF) && detail.contains("no containment seam"),
        "the refusal names the admitted policy and its locus — the missing seam: {detail}"
    );

    // NOTHING SPAWNED: the child never runs unwrapped. (The spawn counter
    // is the load-bearing discriminator — under the disabling mutant the
    // argv is spawned verbatim and this assertion is the one that goes
    // red; see the RED-first record in .agents/reports/task25/cor2-report.md.)
    assert!(
        transport.spawned_argv().is_empty(),
        "a refused policy-carrying dispatch spawns nothing, wrapped or otherwise"
    );
    // ZERO INVOCATION EFFECTS: the invocation directory tree is never
    // anchored for a refused dispatch (no envelope, no prompt, no handle
    // record), and the per-unit workspace carries no artifact. (The
    // anchor DIRECTORY itself is the documented admission behavior for
    // every dispatch — the workspace provider normally creates it before
    // the dispatch anyway — so the artifact-level check is the honest one.)
    assert!(
        !state_dir.join("runs").exists(),
        "a refused dispatch anchors no invocation directory"
    );
    let workspace_files = files_under(&ws);
    assert!(
        workspace_files.is_empty(),
        "a refused dispatch leaves no workspace artifact: {workspace_files:?}"
    );
}

// ---------------------------------------------------------------------------
// F-R3: the cancel-for-recovery arm retires the JUST-SPAWNED WRAPPED CHILD.
// ---------------------------------------------------------------------------

/// The containment wrap that reproduces the observed leak SHAPE (the
/// round-1 budget world's live-sandbox pair + in-sandbox shell): the wrapped
/// command is a DRIVER that forks the real payload in the background and
/// waits — so the driver and the payload are distinct processes, and a
/// signal aimed at the driver alone orphans the payload. (the live backend's
/// `--die-with-parent` did not prevent the round-1 leak for the same
/// structural reason; this wrap is the vendor-free equivalent shape.)
#[derive(Debug)]
struct TreeWrap;

impl CommandContainment for TreeWrap {
    fn wrap(
        &self,
        _workspace: &WorkspaceRef,
        _policy: &SandboxPolicyRef,
        command: &str,
    ) -> Result<String, ContainmentRefusal> {
        Ok(format!("{command} & wait"))
    }
}

/// The probe child: writes NOTHING to its result contract, records its
/// OWN background-sleep pid for the liveness poll, and stays alive. The
/// exact round-1 `never` shape (sleep-guarded).
fn tree_probe_script() -> String {
    r#"#!/bin/sh
set -eu
prompt="$1"; ws="$2"; result="$3"
sleep 121 & sp=$!
echo "$sp" > "$result.sleeppid"
wait "$sp"
"#
    .to_owned()
}

fn pid_alive(pid: u64) -> bool {
    Path::new("/proc").join(pid.to_string()).exists()
}

/// Whether ANY process still belongs to the child's process group (the
/// group the transport puts every spawned child in): `kill -0 -- -PGID`
/// succeeds while a member lives and fails with ESRCH once the tree is
/// gone. The AUTHORITATIVE tree-liveness check — the recorded payload
/// pid alone can be raced (a retired-before-announcement payload never
/// writes its pid file) and can be fooled by pid reuse.
fn group_has_members(pid: u64) -> bool {
    std::process::Command::new("sh")
        .args(["-c", "kill -0 \"$1\" 2>/dev/null", "sh", &format!("-{pid}")])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn kill_pid(pid: u64) {
    std::process::Command::new("sh")
        .args(["-c", &format!("kill -9 {pid} 2>/dev/null || true")])
        .status()
        .ok();
}

fn pid_from_handle_file(handle_file: &Path) -> Option<u64> {
    let text = std::fs::read_to_string(handle_file).ok()?;
    let at = text.find("\"child\":")?;
    let rest = &text[at + "\"child\":".len()..];
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

/// A faithful STAND-IN for the application's `RecordingAgentRunner`
/// persistence-refusal arm (`cancel_after_persistence_failure`, the
/// no-tier-registration shape): the inner runner ADMITS and spawns the
/// child, the durable record construction then refuses, and the arm
/// cancels the already-admitted child under the SPEC's deadline before
/// surfacing the typed refusal. The arm's mechanics — including the
/// exact bound it passes — are reproduced verbatim so the retirement
/// assertion binds the real recovery path.
struct RefusingAfterSpawn<A>(Arc<A>);

impl<A: AgentRunner> AgentRunner for RefusingAfterSpawn<A> {
    fn invoke(&self, spec: RoleInvocationSpec) -> Result<InvocationHandle, AgentError> {
        let handle = self.0.invoke(spec.clone())?;
        // The durable record refused post-spawn (the no-tier-registration
        // shape): cancel for recovery — the CALLER'S bound is the spec
        // deadline, exactly as `cancel_after_persistence_failure` does —
        // then refuse the admission.
        let reason = CancelReason::new("durable invocation recording failed before polling")
            .expect("fixed cancellation reason");
        let _ = self.0.cancel(&handle, reason, spec.deadline);
        Err(AgentError::AdmissionRefused {
            detail: format!(
                "no dispatch tier fact was recorded for policy-carrying invocation {} — \
                 the admitting composition must record the gate-admitted tier before the \
                 dispatch (the durable record must not guess a tier)",
                spec.invocation_id
            ),
        })
    }
    fn poll(&self, handle: &InvocationHandle) -> Result<InvocationStatus, AgentError> {
        self.0.poll(handle)
    }
    fn collect(
        &self,
        handle: &InvocationHandle,
        deadline: Deadline,
    ) -> Result<gauntlet_ports::role_result::RoleResult, AgentError> {
        self.0.collect(handle, deadline)
    }
    fn cancel(
        &self,
        handle: &InvocationHandle,
        reason: CancelReason,
        deadline: Deadline,
    ) -> Result<(), AgentError> {
        self.0.cancel(handle, reason, deadline)
    }
    fn capabilities(&self) -> RunnerCapabilities {
        self.0.capabilities()
    }
}

/// F-R3 (round-1 falsifier, minor): the cancel-for-recovery arm of a
/// post-spawn admission refusal must RETIRE the just-spawned WRAPPED
/// child — the whole process tree — within the bounded cancel span,
/// never an unwaited fire-and-forget signal. The round-1 budget world
/// leaked a live sandbox-driver pair + in-sandbox shell past the refusal. RED on
/// the delivered stop mechanics: the stop's SIGTERM aims at the DIRECT
/// child only, so a wrapped driver's forked payload survives its
/// driver's death (the leak shape probed live in this round: `sh -c
/// '<cmd> & wait'`, driver TERM'd → payload orphaned).
#[test]
fn fr3_a_post_spawn_refusal_retires_the_whole_wrapped_child_tree() {
    let state_dir = scratch("fr3-state");
    let ws = scratch("fr3-ws");
    let script_path = ws.join("tree-probe-child.sh");
    std::fs::write(&script_path, tree_probe_script()).expect("probe script writable");
    let template = format!(
        "sh {} {{prompt_file}} {{workspace}} {{result_path}}",
        script_path.to_string_lossy()
    );
    let inner = CliAgentRunner::new(
        CliAgentConfig::new(template),
        state_dir.clone(),
        ws.clone(),
        ProcessChildTransport::new(),
        Arc::new(SystemClock::new()),
    )
    .expect("template parses")
    .with_containment(Arc::new(TreeWrap));
    let runner = RefusingAfterSpawn(Arc::new(inner));

    let mut spec = spec_with(ID_A, 30);
    spec.sandbox_policy = Some(SandboxPolicyRef::new(POLICY_REF).expect("policy ref"));
    let error = runner
        .invoke(spec)
        .expect_err("the no-tier-registration shape refuses after the spawn");
    assert!(
        matches!(error, AgentError::AdmissionRefused { .. }),
        "the post-spawn refusal is the typed ADMISSION class: {error:?}"
    );

    // The child really spawned (wrapped — the sandbox is the first
    // process); the forensic handle record names the driver pid.
    let inv_dir = state_dir.join("runs").join(RUN_ID).join("inv").join(ID_A);
    let driver_pid = pid_from_handle_file(&inv_dir.join("result.json.handle.json"))
        .expect("the forensic handle record names the spawned child");
    // The wrapped payload records its own pid when it gets far enough to
    // — and a race here is REAL and deliberate: the cancel may
    // legitimately retire the tree before the payload announces itself
    // (the refusal fires within microseconds of the spawn). Bounded
    // wait; an absent pid file after the bound is a
    // retired-before-announcement outcome, and the group-liveness check
    // below is then the authority.
    let sleeper_file = inv_dir.join("result.json.sleeppid");
    let mut sleeper_pid = None;
    for _ in 0..(3 * 20) {
        if let Ok(text) = std::fs::read_to_string(&sleeper_file)
            && let Ok(pid) = text.trim().parse::<u64>()
        {
            sleeper_pid = Some(pid);
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    // THE RETIREMENT OBLIGATION: bounded poll — after the refusal
    // returns, NO child of the dispatch may outlive the call. The
    // authoritative condition is the PROCESS GROUP being empty (every
    // member retired); the recorded payload pid, when it announced
    // itself, must be dead too. A surviving tree is the leak, and is
    // killed before the typed failure fires — hygiene.
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        let sleeper_survived = sleeper_pid.is_some_and(pid_alive);
        let group_alive = group_has_members(driver_pid);
        if !sleeper_survived && !group_alive {
            break;
        }
        if std::time::Instant::now() >= deadline {
            if let Some(pid) = sleeper_pid {
                kill_pid(pid);
            }
            kill_pid(driver_pid);
            panic!(
                "the post-spawn refusal left the wrapped tree alive: driver pid \
                 {driver_pid} alive={}, wrapped payload pid {sleeper_pid:?} \
                 alive={sleeper_survived}, process group still has members={group_alive} \
                 — the cancel-for-recovery arm leaked the child",
                pid_alive(driver_pid),
            );
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}
