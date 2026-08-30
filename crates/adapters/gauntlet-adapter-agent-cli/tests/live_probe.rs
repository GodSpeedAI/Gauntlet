//! The env-gated live probe (plan Task 16; the prime adapter's posture):
//! NOT part of the default battery — it runs only with
//! `GAUNTLET_CLI_AGENT_LIVE_PROBE=1` and costs ONE real child process
//! spawned through the production transport. It is not dispatched for any
//! Unit, passes through no authority gate, and exercises no workspace
//! composition (R-1b stays closed to real builders).
//!
//! Evidence the probe produces:
//!
//! 1. the REAL command-template render becomes argv for a REAL process
//!    (`/bin/sh` running a trivial probe script written to the scratch
//!    root — no agent binary, no model, no vendor product);
//! 2. the prompt-file format is parseable by a THIRD PARTY: the probe
//!    script extracts the fields with plain `sed` (the documented
//!    grammar, RES-D2's envelope-path reference included) and writes a
//!    valid `role_result@v1` document to the RESULT-PATH it parsed;
//! 3. honest completion: `poll` reaches `Completed` only via the result
//!    contract at the declared path; `collect` returns the matching
//!    document;
//! 4. idempotent cancellation: cancel on the Completed invocation is the
//!    typed no-op, twice; a stranger handle is `UnknownHandle`.
//!
//! Probe hygiene: every artifact lives under one temp scratch root,
//! removed at the end; nothing is ever written under the repository's
//! `.gauntlet/` tree; `just doctor` must stay OK. Any failure surfaces as
//! a plain test failure naming the locus — never retried into green.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gauntlet_adapter_agent_cli::clock::SystemClock;
use gauntlet_adapter_agent_cli::config::CliAgentConfig;
use gauntlet_adapter_agent_cli::runner::CliAgentRunner;
use gauntlet_adapter_agent_cli::transport::ProcessChildTransport;
use gauntlet_domain::{
    Budget, DiagnosticAllowance, Digest, EpochSeconds, InvocationId, IterationNo, Limits, MicroUsd,
    PermissionSetRef, ResultContractPath, RoleId, RunId, WorkspaceRef,
};
use gauntlet_ports::agent_runner::{
    AgentError, AgentRunner, CancelReason, ContextEnvelope, InvocationStatus, RoleInvocationSpec,
};
use gauntlet_ports::deadline::Deadline;

/// The gate: the probe is opt-in and never runs in the default battery.
fn live_probe_requested() -> bool {
    std::env::var("GAUNTLET_CLI_AGENT_LIVE_PROBE").as_deref() == Ok("1")
}

/// Dispatch facts for the probe invocation (a fixed valid ULID).
const PROBE_ULID: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
const PROBE_DIGEST_SEED: &str = "0123456789abcdef";
const CONTRACT_LEAF: &str = "result.json";

/// Overall wall-clock bound for the whole probe: generous for a real
/// process, but bounded.
const PROBE_TOTAL_SECONDS: u64 = 60;

/// The poll cadence once admission has returned.
const POLL_PAUSE_MILLIS: u64 = 250;

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("gauntlet-cli-live-{tag}-{}", std::process::id()));
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

/// The trivial probe child: a POSIX shell script that parses the prompt
/// file per the documented grammar and writes a valid result contract to
/// the RESULT-PATH it parsed. It is the format's first third-party
/// consumer — exactly what a real headless agent would be, minus
/// everything a real agent does.
const PROBE_SCRIPT: &str = r#"#!/bin/sh
# gauntlet cli-agent live-probe child (format: gauntlet-cli-agent-prompt@1)
set -eu
prompt="$1"
result_path=$(sed -n 's/^RESULT-PATH: //p' "$prompt")
invocation=$(sed -n 's/^INVOCATION-ID: //p' "$prompt")
run=$(sed -n 's/^RUN-ID: //p' "$prompt")
iteration=$(sed -n 's/^ITERATION-NO: //p' "$prompt")
role=$(sed -n 's/^ROLE: //p' "$prompt")
digest=$(sed -n 's/^ENVELOPE-DIGEST: //p' "$prompt")
[ -n "$result_path" ] || { echo "probe: no RESULT-PATH in $prompt" >&2; exit 2; }
cat > "$result_path" <<EOF
{"schema":"role_result@v1","invocation_id":"$invocation","run_id":"$run","iteration_no":$iteration,"role":"$role","context_envelope_digest":"$digest","state":"completed","payload_schema":"builder_result@v1","payload":{"attempted":[],"changed_paths":[],"commands_run":[],"artifacts":[],"claims":["cli-agent live probe"],"self_checks":[],"blocked_on":[]},"payment":{"input_tokens":0,"output_tokens":0,"tool_calls":0,"wall_seconds":0,"cost_usd":0},"unavailable_evidence":[],"errors":[]}
EOF
"#;

fn spec() -> RoleInvocationSpec {
    let digest = Digest::new("sha256", PROBE_DIGEST_SEED).expect("valid digest");
    let budget = Budget::new(
        Limits::new(8, 2, 2, PROBE_TOTAL_SECONDS, 1000, MicroUsd::new(500)).expect("limits usable"),
        20,
        DiagnosticAllowance::new(100, MicroUsd::new(50)),
    )
    .expect("budget usable");
    RoleInvocationSpec {
        invocation_id: InvocationId::new(PROBE_ULID).expect("valid id"),
        run_id: RunId::new(PROBE_ULID).expect("valid id"),
        iteration_no: IterationNo::new(1).expect("1-based"),
        role: RoleId::Builder,
        prompt: "This is a LIVE PROBE of the gauntlet command-template runner adapter. Your \
                 entire task: write a valid role_result@v1 document (echoing INVOCATION-ID and \
                 ENVELOPE-DIGEST) to the RESULT-PATH named above, atomically, then exit. Do \
                 nothing else."
            .to_owned(),
        context_envelope: ContextEnvelope::new(
            "the live probe's allowlisted context".to_owned(),
            digest,
        ),
        workspace: WorkspaceRef::new("ws-live-probe").expect("valid ref"),
        permissions: PermissionSetRef::new("perm-live-probe").expect("valid ref"),
        result_contract_path: ResultContractPath::new(CONTRACT_LEAF).expect("valid ref"),
        deadline: bound_from_now(PROBE_TOTAL_SECONDS),
        model_hint: None,
        sandbox_policy: None,
        budget_slice: budget,
    }
}

/// The probe.
#[test]
fn one_real_subprocess_completes_through_the_result_contract() {
    if !live_probe_requested() {
        // Honest skip: the probe is opt-in; silence here is the recorded
        // skip, not a pass claim.
        eprintln!(
            "skipped: GAUNTLET_CLI_AGENT_LIVE_PROBE != 1 — the live probe spawns one real \
             process and is opt-in"
        );
        return;
    }
    let started = Instant::now();
    let state_dir = scratch("state");
    let workspace = scratch("ws");

    // 1. The probe script lands in the scratch root; the command template
    //    (a literal here, RES-D10) names it through plain shell-word
    //    syntax — no shell interpolation ever renders it.
    let script_path = workspace.join("probe-child.sh");
    std::fs::write(&script_path, PROBE_SCRIPT).expect("the probe script is writable");
    let template = format!("sh {} {{prompt_file}}", script_path.to_string_lossy());

    // 2. The production composition: REAL process transport + system
    //    clock (RES-D9: production adapters read the wall clock), temp
    //    roots injected.
    let runner = CliAgentRunner::new(
        CliAgentConfig::new(template),
        state_dir.clone(),
        workspace.clone(),
        ProcessChildTransport::new(),
        Arc::new(SystemClock::new()),
    )
    .expect("the probe template parses");

    // 3. ADMISSION through the REAL spawn path.
    let handle = runner
        .invoke(spec())
        .expect("admission through the real transport must succeed");
    eprintln!(
        "admitted: invocation {} with session_ref {:?}",
        handle.invocation_id(),
        handle.session_ref()
    );

    // 4. Poll to Completed — the FILE decides; a real child writes it.
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
                    "the live probe passed its wall-clock bound polling the result contract"
                );
                std::thread::sleep(Duration::from_millis(POLL_PAUSE_MILLIS));
            }
        }
    };
    let InvocationStatus::Completed { result_ref } = status else {
        panic!("the live probe must reach Completed, got {status:?}")
    };
    assert_eq!(result_ref.as_str(), CONTRACT_LEAF);

    // 5. Collection returns the validated document.
    let result = runner
        .collect(&handle, bound_from_now(30))
        .expect("a Completed invocation is collectible");
    assert_eq!(result.invocation_id().as_str(), PROBE_ULID);
    eprintln!(
        "collected: state={:?} role={:?}",
        result.state(),
        result.role()
    );

    // 6. Idempotent cancellation: the first cancel on COMPLETED work is
    //    the typed no-op; so is the second. A stranger is UnknownHandle.
    let reason = CancelReason::new("live probe: completion already reached").expect("non-empty");
    let bound = bound_from_now(30);
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

    // 7. Probe hygiene: the scratch roots go away; nothing was ever
    //    written under the repository's .gauntlet/ tree.
    std::fs::remove_dir_all(&state_dir).ok();
    std::fs::remove_dir_all(&workspace).ok();
    eprintln!("live probe finished in {:?}", started.elapsed());
}
