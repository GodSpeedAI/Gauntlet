//! The env-gated live probe (plan Task 15; recon §3/§6; correction
//! RES-D1's evidence leg). NOT part of the default battery: it runs only
//! with `GAUNTLET_PRIME_LIVE_PROBE=1` and costs ONE real model round
//! trip through the production kernel transport — a TRIVIAL child that
//! writes a minimal valid result file and replies. It is not dispatched
//! for any Unit, passes through no authority gate, and exercises no
//! workspace composition (R-1b stays closed to real builders).
//!
//! Evidence the probe produces:
//!
//! 1. the version probe resolves against the injected supported range
//!    (test-local values mirroring the owning app defaults — this crate
//!    cannot depend on the application layer);
//! 2. **the D1 split**: `invoke` returns with the admission handle while
//!    the parent print run was STILL RUNNING at handle-parse time — the
//!    transport's forensic note records `parent.try_wait()` state taken
//!    BEFORE the graceful stop — and the parent is recorded exited before
//!    any completion polling, so the `Completed` below derives purely
//!    from the result FILE, which also evidences daemon-backed child
//!    survival;
//! 3. honest completion: `poll` reaches `Completed` only via the result
//!    contract at the declared path; `collect` returns the matching
//!    document;
//! 4. idempotent cancellation: cancel on the Completed invocation is the
//!    typed no-op, twice; a stranger handle is `UnknownHandle`.
//!
//! Any failure surfaces as a plain test failure naming the locus.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gauntlet_adapter_agent_prime::transport::{ProcessHarnessControl, ProcessKernelTransport};
use gauntlet_adapter_agent_prime::{
    PrimeAdapterConfig, PrimeAgentRunner, SystemClock, VersionRange, probe,
};
use gauntlet_domain::{
    Budget, DiagnosticAllowance, Digest, EpochSeconds, InvocationId, Limits, MicroUsd,
    PermissionSetRef, ResultContractPath, RoleId, RunId, WorkspaceRef,
};
use gauntlet_ports::agent_runner::{
    AgentError, AgentRunner, CancelReason, ContextEnvelope, InvocationStatus, RoleInvocationSpec,
};
use gauntlet_ports::deadline::Deadline;

/// The gate: the probe is opt-in and never runs in the default battery.
fn live_probe_requested() -> bool {
    std::env::var("GAUNTLET_PRIME_LIVE_PROBE").as_deref() == Ok("1")
}

/// The test-local supported range — mirrors the owning app defaults
/// module (this adapter crate cannot depend on the application layer to
/// read them; the values are re-derived here and the doctor surface
/// checks the owned ones).
const SUPPORTED_MIN: (u64, u64, u64) = (0, 8, 0);
const SUPPORTED_LIMIT: (u64, u64, u64) = (1, 0, 0);

/// Dispatch facts for the probe invocation (fixed valid ULIDs).
const PROBE_ULID: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
/// The envelope digest the probe dispatches (valid digest shape).
const PROBE_DIGEST_SEED: &str = "0123456789abcdef";

/// Overall wall-clock bound for the whole probe: generous (admission is a
/// full parent model session), but bounded.
const PROBE_TOTAL_SECONDS: u64 = 1200;

/// The poll cadence once admission has returned.
const POLL_PAUSE_SECONDS: u64 = 5;

fn scratch(tag: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("gauntlet-prime-live-{tag}-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).expect("the live-probe scratch is creatable");
    dir
}

fn bound_from_now(seconds: u64) -> Deadline {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("the wall clock reads")
        .as_secs();
    Deadline::at_epoch(EpochSeconds::new(now + seconds))
}

fn format_version(version: (u64, u64, u64)) -> String {
    format!("{}.{}.{}", version.0, version.1, version.2)
}

/// The probe.
#[test]
fn one_real_admission_completes_through_the_result_contract() {
    if !live_probe_requested() {
        // Honest skip: the probe is opt-in; silence here is the recorded
        // skip, not a pass claim.
        eprintln!(
            "skipped: GAUNTLET_PRIME_LIVE_PROBE != 1 — the live probe costs one real model \
             round trip and is opt-in"
        );
        return;
    }
    let started = Instant::now();
    let config = PrimeAdapterConfig::default();

    // 1. Version probe: the harness API surface behind the injected range.
    let range = VersionRange {
        min: SUPPORTED_MIN,
        limit: SUPPORTED_LIMIT,
    };
    let report = probe(&config.binary, &range, bound_from_now(120))
        .expect("the harness binary must probe clean for the live probe");
    eprintln!(
        "probe: {} at {:?}",
        format_version(report.version),
        report.binary
    );

    // 2. The production composition: process transport + process control
    //    + the system clock, with temp roots injected (RES-D3). The
    //    transport is shared so its forensic note survives the runner.
    let state_dir = scratch("state");
    let workspace_root = scratch("cwd");
    let transport = Arc::new(ProcessKernelTransport::new(
        config.binary.clone(),
        config.admission_timeout(),
    ));
    let control = ProcessHarnessControl::new(config.binary.clone());
    let runner = PrimeAgentRunner::new(
        config.clone(),
        state_dir.clone(),
        workspace_root.clone(),
        Arc::clone(&transport),
        control,
        Arc::new(SystemClock::new()),
    );

    // The dispatch: the child's whole job is to write the exact document
    // (embedded in its prompt) atomically, then reply. The runner composes
    // the full contract instruction around this body.
    let contract_leaf = "result.json";
    let digest = Digest::parse(&format!("sha256:{PROBE_DIGEST_SEED}")).expect("valid digest");
    let document = format!(
        "{{\"schema\":\"role_result@v1\",\"invocation_id\":\"{PROBE_ULID}\",\
         \"run_id\":\"{PROBE_ULID}\",\"iteration_no\":1,\"role\":\"builder\",\
         \"context_envelope_digest\":\"sha256:{PROBE_DIGEST_SEED}\",\"state\":\"completed\",\
         \"payload_schema\":\"builder_result@v1\",\
         \"payload\":{{\"attempted\":[],\"changed_paths\":[],\"commands_run\":[],\
         \"artifacts\":[],\"claims\":[\"live probe\"],\"self_checks\":[],\"blocked_on\":[]}},\
         \"payment\":{{\"input_tokens\":0,\"output_tokens\":0,\"tool_calls\":0,\
         \"wall_seconds\":0,\"cost_usd\":0}},\"unavailable_evidence\":[],\
         \"errors\":[]}}"
    );
    let spec = RoleInvocationSpec {
        invocation_id: InvocationId::new(PROBE_ULID).expect("valid id"),
        run_id: RunId::new(PROBE_ULID).expect("valid id"),
        iteration_no: gauntlet_domain::IterationNo::new(1).expect("1-based"),
        role: RoleId::Builder,
        prompt: format!(
            "This is a LIVE PROBE of the gauntlet prime-agent adapter. Your entire task: \
             write EXACTLY the following JSON document to the result-contract path named in \
             the instructions below (atomic tmp+rename, exactly as instructed), then reply \
             with the single word DONE. Do nothing else.\n\n{document}\n"
        ),
        context_envelope: ContextEnvelope::new(
            "the live probe's allowlisted context".to_owned(),
            digest.clone(),
        ),
        workspace: WorkspaceRef::new("ws-live-probe").expect("valid ref"),
        permissions: PermissionSetRef::new("perm-live-probe").expect("valid ref"),
        result_contract_path: ResultContractPath::new(contract_leaf).expect("valid ref"),
        deadline: bound_from_now(PROBE_TOTAL_SECONDS),
        model_hint: None,
        sandbox_policy: None,
        budget_slice: Budget::new(
            Limits::new(8, 2, 2, PROBE_TOTAL_SECONDS, 1000, MicroUsd::new(500))
                .expect("limits usable"),
            20,
            DiagnosticAllowance::new(100, MicroUsd::new(50)),
        )
        .expect("budget usable"),
    };

    // 3. ADMISSION. The invoke returns after the handle parses — it must
    //    NOT wait for the parent run to end (RES-D1).
    let handle = runner
        .invoke(spec)
        .expect("admission through the real harness must succeed");
    eprintln!(
        "admitted: invocation {} with session_ref {:?}",
        handle.invocation_id(),
        handle.session_ref()
    );

    // 4. THE D1 SPLIT EVIDENCE — the transport's forensic note records,
    //    in order: the parent pid; the parent's try_wait state AT PARSE
    //    TIME (`handle parsed while parent.try_wait()=NotExited` means
    //    invoke did NOT wait for the parent); and the RC1 disposition
    //    marker (task26-dcritic-apparatus-repair, supersedes the earlier
    //    "detached reaper" shape this comment used to name): the parent
    //    handle is returned to the CALLER — the invocation, not the
    //    transport, now owns the parent's lifetime, so it is never
    //    stopped at parse and never inside invoke. These tokens are the
    //    transport's documented forensic vocabulary (same crate).
    let forensics = transport
        .take_forensics()
        .expect("the production transport records its admission forensics");
    eprintln!("forensics: {forensics}");
    assert!(
        forensics.contains("handle parsed while parent.try_wait()=NotExited"),
        "RES-D1 split violated: the handle was not parsed while the parent run was still \
         in flight — invoke waited on the parent run. forensics: {forensics}"
    );
    assert!(
        forensics.contains("returned to the caller"),
        "RC1 violated: the transport must hand the live parent to the caller at admission, \
         never dispose of it itself. forensics: {forensics}"
    );

    // 5. Poll to Completed — the FILE decides, never a process (RC1: the
    //    runner now owns the parent's process, polling it opportunistically
    //    for forensics only; the child materializes because the parent's
    //    cell keeps polling for the result file itself (RC2), and that
    //    result file is what completion means).
    let poll_started = Instant::now();
    let status = loop {
        match runner
            .poll(&handle)
            .expect("poll must not error while the contract is pending")
        {
            status if status.is_terminal() => break status,
            other => {
                eprintln!("poll: {other:?}");
                assert!(
                    poll_started.elapsed() < Duration::from_secs(PROBE_TOTAL_SECONDS),
                    "the live probe passed its wall-clock bound polling the result contract \
                     under the state root"
                );
                std::thread::sleep(Duration::from_secs(POLL_PAUSE_SECONDS));
            }
        }
    };
    let InvocationStatus::Completed { result_ref } = status else {
        panic!("the live probe must reach Completed, got {status:?}")
    };
    assert_eq!(result_ref.as_str(), contract_leaf);

    // 6. Collection returns the validated document.
    let result = runner
        .collect(&handle, bound_from_now(60))
        .expect("a Completed invocation is collectible");
    assert_eq!(result.invocation_id().as_str(), PROBE_ULID);
    assert_eq!(result.context_envelope_digest(), &digest);
    eprintln!(
        "collected: state={:?} role={:?}",
        result.state(),
        result.role()
    );

    // 7. Idempotent cancellation: the first cancel on COMPLETED work is
    //    the typed no-op; so is the second. A stranger is UnknownHandle.
    let reason = CancelReason::new("live probe: completion already reached").expect("non-empty");
    let bound = bound_from_now(60);
    match runner.cancel(&handle, reason.clone(), bound) {
        Ok(()) | Err(AgentError::AlreadyTerminal { .. }) => {}
        other => panic!("cancel after Completed must be Ok or the typed no-op, got {other:?}"),
    }
    match runner.cancel(&handle, reason, bound) {
        Ok(()) | Err(AgentError::AlreadyTerminal { .. }) => {}
        other => panic!("the second cancel must stay idempotent, got {other:?}"),
    }
    let stranger = gauntlet_ports::agent_runner::InvocationHandle::new(
        InvocationId::new("01ARZ3NDEKTSV4RRFFQ69G5FAB").expect("distinct valid id"),
        None,
    );
    assert!(matches!(
        runner.cancel(
            &stranger,
            CancelReason::new("stray").expect("non-empty"),
            bound
        ),
        Err(AgentError::UnknownHandle { .. })
    ));

    std::fs::remove_dir_all(&state_dir).ok();
    std::fs::remove_dir_all(&workspace_root).ok();
    eprintln!("live probe finished in {:?}", started.elapsed());
}

/// The gate for the long-child probe below: separate from
/// [`live_probe_requested`] so an operator can run the cheap admission
/// probe without also paying for the slower one (real model time).
fn long_child_probe_requested() -> bool {
    std::env::var("GAUNTLET_PRIME_LIVE_LINGER_PROBE").as_deref() == Ok("1")
}

/// The long-child real-binary proof (task26-dcritic-apparatus-repair
/// rpc-mode correction): a child instructed to work for ~90 seconds
/// before writing its result must still survive and land its result
/// contract. Under the DEFECT the `--print` transport had — the parent
/// session ending (at the end of one fixed-length cell, or when its
/// process was disposed shortly after admission) aborted any `rlm` child
/// still in flight (`Request was aborted`), exactly as the real run
/// evidence showed. Under the rpc-mode fix, the parent process's
/// lifetime is owned by the INVOCATION (its stdin stays open until the
/// runner disposes it, never by any fixed span inside one rendered
/// cell), so a child that takes ~90 s survives regardless of any
/// per-cell timing.
///
/// Gated by `GAUNTLET_PRIME_LIVE_LINGER_PROBE=1`; off by default (one
/// real, slower model round trip).
#[test]
fn a_long_running_child_still_lands_its_result() {
    if !long_child_probe_requested() {
        eprintln!(
            "skipped: GAUNTLET_PRIME_LIVE_LINGER_PROBE != 1 — this probe costs one real, \
             slower (~90s+) model round trip and is opt-in"
        );
        return;
    }
    let started = Instant::now();
    let config = PrimeAdapterConfig::default();

    let range = VersionRange {
        min: SUPPORTED_MIN,
        limit: SUPPORTED_LIMIT,
    };
    let report = probe(&config.binary, &range, bound_from_now(120))
        .expect("the harness binary must probe clean for the live probe");
    eprintln!(
        "probe: {} at {:?}",
        format_version(report.version),
        report.binary
    );

    let state_dir = scratch("linger-state");
    let workspace_root = scratch("linger-cwd");
    let transport = Arc::new(ProcessKernelTransport::new(
        config.binary.clone(),
        config.admission_timeout(),
    ));
    let control = ProcessHarnessControl::new(config.binary.clone());
    let runner = PrimeAgentRunner::new(
        config.clone(),
        state_dir.clone(),
        workspace_root.clone(),
        Arc::clone(&transport),
        control,
        Arc::new(SystemClock::new()),
    );

    let contract_leaf = "result.json";
    let digest = Digest::parse(&format!("sha256:{PROBE_DIGEST_SEED}")).expect("valid digest");
    let document = format!(
        "{{\"schema\":\"role_result@v1\",\"invocation_id\":\"{PROBE_ULID}\",\
         \"run_id\":\"{PROBE_ULID}\",\"iteration_no\":1,\"role\":\"builder\",\
         \"context_envelope_digest\":\"sha256:{PROBE_DIGEST_SEED}\",\"state\":\"completed\",\
         \"payload_schema\":\"builder_result@v1\",\
         \"payload\":{{\"attempted\":[],\"changed_paths\":[],\"commands_run\":[],\
         \"artifacts\":[],\"claims\":[\"long-child live probe\"],\"self_checks\":[],\
         \"blocked_on\":[]}},\"payment\":{{\"input_tokens\":0,\"output_tokens\":0,\
         \"tool_calls\":0,\"wall_seconds\":0,\"cost_usd\":0}},\"unavailable_evidence\":[],\
         \"errors\":[]}}"
    );
    // The child's ENTIRE job: wait past the default linger doing real
    // ipython work (a busy-wait loop, not a shell sleep — this is a
    // GENUINE tool call taking real wall time, the same shape as the
    // real-run evidence's builder), THEN write the result document.
    let spec = RoleInvocationSpec {
        invocation_id: InvocationId::new(PROBE_ULID).expect("valid id"),
        run_id: RunId::new(PROBE_ULID).expect("valid id"),
        iteration_no: gauntlet_domain::IterationNo::new(1).expect("1-based"),
        role: RoleId::Builder,
        prompt: format!(
            "This is a LIVE PROBE of the gauntlet prime-agent adapter's tolerance for a slow \
             child (task26-dcritic-apparatus-repair). First, run this exact ipython cell and \
             wait for it to finish — it deliberately takes about 90 seconds:\n\n\
             import time\ntime.sleep(90)\n\n\
             THEN, and only then, write EXACTLY the following JSON document to the \
             result-contract path named in the instructions below (atomic tmp+rename, exactly \
             as instructed), then reply with the single word DONE. Do nothing else.\n\n{document}\n"
        ),
        context_envelope: ContextEnvelope::new(
            "the long-child live probe's allowlisted context".to_owned(),
            digest.clone(),
        ),
        workspace: WorkspaceRef::new("ws-live-linger-probe").expect("valid ref"),
        permissions: PermissionSetRef::new("perm-live-linger-probe").expect("valid ref"),
        result_contract_path: ResultContractPath::new(contract_leaf).expect("valid ref"),
        deadline: bound_from_now(PROBE_TOTAL_SECONDS),
        model_hint: None,
        sandbox_policy: None,
        budget_slice: Budget::new(
            Limits::new(8, 2, 2, PROBE_TOTAL_SECONDS, 1000, MicroUsd::new(500))
                .expect("limits usable"),
            20,
            DiagnosticAllowance::new(100, MicroUsd::new(50)),
        )
        .expect("budget usable"),
    };

    let handle = runner
        .invoke(spec)
        .expect("admission through the real harness must succeed");
    eprintln!(
        "admitted: invocation {} with session_ref {:?}",
        handle.invocation_id(),
        handle.session_ref()
    );

    let poll_started = Instant::now();
    let status = loop {
        match runner
            .poll(&handle)
            .expect("poll must not error while the contract is pending")
        {
            status if status.is_terminal() => break status,
            other => {
                eprintln!("poll at {:?}: {other:?}", poll_started.elapsed());
                assert!(
                    poll_started.elapsed() < Duration::from_secs(PROBE_TOTAL_SECONDS),
                    "the long-child live probe passed its wall-clock bound"
                );
                std::thread::sleep(Duration::from_secs(POLL_PAUSE_SECONDS));
            }
        }
    };
    let elapsed = poll_started.elapsed();
    let InvocationStatus::Completed { result_ref } = status else {
        panic!(
            "the long-child live probe must reach Completed (the defect this fixes made a \
             child working past the parent's own turn die with an aborted tool call), got \
             {status:?} after {elapsed:?}"
        )
    };
    assert_eq!(result_ref.as_str(), contract_leaf);
    /// The floor the ~90 s child's real work must clear for this probe to
    /// prove anything (well below its target, generous against jitter).
    const MINIMUM_PROOF_SECONDS: u64 = 60;
    assert!(
        elapsed >= Duration::from_secs(MINIMUM_PROOF_SECONDS),
        "the child's real work ({elapsed:?}) must have taken at least \
         {MINIMUM_PROOF_SECONDS} s for this probe to prove anything"
    );

    let result = runner
        .collect(&handle, bound_from_now(60))
        .expect("a Completed invocation is collectible");
    assert_eq!(result.invocation_id().as_str(), PROBE_ULID);
    assert_eq!(result.context_envelope_digest(), &digest);

    let forensics = transport.take_forensics();
    eprintln!("forensics: {forensics:?}");
    eprintln!(
        "long-child live probe finished in {:?} (child work took {elapsed:?})",
        started.elapsed()
    );

    std::fs::remove_dir_all(&state_dir).ok();
    std::fs::remove_dir_all(&workspace_root).ok();
}
