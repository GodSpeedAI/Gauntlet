//! CLASSIFICATION / PROCESS-LIFECYCLE / CHANNEL ATTACKS through the REAL
//! `ProcessChildTransport` — falsifier round 1 (composed blind by the
//! falsifier agent; harness bugs repaired and the round completed by the
//! orchestrator after the agent's usage-window death — see
//! .agents/reports/task16/{falsifier-round1.md, IN-FLIGHT.md}). Retained
//! as the round's regression teeth: c_sigterm pins COR-F1 (a landed
//! SIGKILL with a pending reap is a stopped child, not a failed cancel)
//! and c_hang pins COR-F4 (a terminal pin retires a live child — no
//! invocation leaves a live child behind). Tiny bounded `sh` probe
//! children only (AUTHORIZED: no agent binary, no model, no Unit
//! dispatch, no authority gate; every child bounded and cleaned up).

// The doubles are deliberately single-threaded (the mock runner's own
// posture): one runner instance is driven from one thread, so the Arcs
// that hand the transport/clock to the runner are knowingly non-Send/Sync.
#![allow(clippy::arc_with_non_send_sync)]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use gauntlet_adapter_agent_cli::clock::SystemClock;
use gauntlet_adapter_agent_cli::config::CliAgentConfig;
use gauntlet_adapter_agent_cli::runner::CliAgentRunner;
use gauntlet_adapter_agent_cli::testing::{BASE_INSTANT, ScriptedProcessTransport, SteppingClock};
use gauntlet_adapter_agent_cli::transport::ProcessChildTransport;
use gauntlet_domain::{
    Budget, DiagnosticAllowance, Digest, EpochSeconds, InvocationId, IterationNo, Limits, MicroUsd,
    ModelIdentity, Payment, PermissionSetRef, ResultContractPath, RoleId, RunId, WorkspaceRef,
};
use gauntlet_ports::Clock;
use gauntlet_ports::agent_runner::{
    AgentError, AgentRunner, CancelReason, ContextEnvelope, InvocationFailure, InvocationHandle,
    InvocationStatus, Isolation, RoleInvocationSpec,
};
use gauntlet_ports::deadline::Deadline;

const ID_A: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
const ID_B: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAB";
const DIGEST_HEX: &str = "e5f6a7b8c4d3e2f1a0b1c2d3";

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "gauntlet-falsifier-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
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

fn spec_with(id: &str, deadline_seconds: u64, model: Option<&str>) -> RoleInvocationSpec {
    let budget = Budget::new(
        Limits::new(8, 2, 2, 3600, 1000, MicroUsd::new(500)).expect("limits"),
        20,
        DiagnosticAllowance::new(100, MicroUsd::new(50)),
    )
    .expect("budget");
    RoleInvocationSpec {
        invocation_id: InvocationId::new(id).expect("valid id"),
        run_id: RunId::new(ID_A).expect("valid id"),
        iteration_no: IterationNo::new(1).expect("1-based"),
        role: RoleId::Builder,
        prompt: "falsifier probe: write the typed result to the declared path".to_owned(),
        context_envelope: ContextEnvelope::new(
            "line one of the envelope body\nline two with \"quotes\" and {braces} and $dollar"
                .to_owned(),
            Digest::new("sha256", DIGEST_HEX).expect("digest"),
        ),
        workspace: WorkspaceRef::new("ws-falsifier").expect("workspace ref"),
        permissions: PermissionSetRef::new("perm-falsifier").expect("permission ref"),
        result_contract_path: ResultContractPath::new("result.json").expect("contract path"),
        deadline: bound_from_now(deadline_seconds),
        model_hint: model.map(|m| ModelIdentity::new(m).expect("model identity")),
        sandbox_policy: None,
        budget_slice: budget,
    }
}

/// A tiny probe child. Parses the prompt file per the documented grammar
/// and behaves per `$MODE`, writing a JSON document to the RESULT-PATH.
fn probe_script(mode: &str) -> String {
    format!(
        r#"#!/bin/sh
set -eu
prompt="$1"; ws="$2"; result="$3"
result_path=$(sed -n 's/^RESULT-PATH: //p' "$prompt")
inv=$(sed -n 's/^INVOCATION-ID: //p' "$prompt")
digest=$(sed -n 's/^ENVELOPE-DIGEST: //p' "$prompt")
case "{mode}" in
complete)
  cat > "$result_path" <<EOF
{{"schema":"role_result@v1","invocation_id":"$inv","run_id":"01ARZ3NDEKTSV4RRFFQ69G5FAV","iteration_no":1,"role":"builder","context_envelope_digest":"$digest","state":"completed","payload_schema":"builder_result@v1","payload":{{"attempted":[],"changed_paths":[],"commands_run":[],"artifacts":[],"claims":[],"self_checks":[],"blocked_on":[]}},"payment":{{"input_tokens":0,"output_tokens":0,"tool_calls":0,"wall_seconds":0,"cost_usd":0}},"unavailable_evidence":[],"errors":[]}}
EOF
  ;;
exit7)
  cat > "$result_path" <<EOF
{{"schema":"role_result@v1","invocation_id":"$inv","run_id":"01ARZ3NDEKTSV4RRFFQ69G5FAV","iteration_no":1,"role":"builder","context_envelope_digest":"$digest","state":"completed","payload_schema":"builder_result@v1","payload":{{"attempted":[],"changed_paths":[],"commands_run":[],"artifacts":[],"claims":[],"self_checks":[],"blocked_on":[]}},"payment":{{"input_tokens":0,"output_tokens":0,"tool_calls":0,"wall_seconds":0,"cost_usd":0}},"unavailable_evidence":[],"errors":[]}}
EOF
  exit 7
  ;;
envobs)
  printf '%s' "$GAUNTLET_RESULT_PATH" > "$result_path.envobs"
  cp "$prompt" "$result_path.promptobs"
  ;;
never)
  sleep 121 & sp=$!
  echo "$sp" > "$result_path.sleeppid"
  wait "$sp"
  ;;
garbage)
  printf 'this is not json {{{{' > "$result_path"
  ;;
tornwrite)
  printf 'this is not json {{{{' > "$result_path"
  sleep 2
  cat > "$result_path" <<EOF
{{"schema":"role_result@v1","invocation_id":"$inv","run_id":"01ARZ3NDEKTSV4RRFFQ69G5FAV","iteration_no":1,"role":"builder","context_envelope_digest":"$digest","state":"completed","payload_schema":"builder_result@v1","payload":{{"attempted":[],"changed_paths":[],"commands_run":[],"artifacts":[],"claims":[],"self_checks":[],"blocked_on":[]}},"payment":{{"input_tokens":0,"output_tokens":0,"tool_calls":0,"wall_seconds":0,"cost_usd":0}},"unavailable_evidence":[],"errors":[]}}
EOF
  ;;
tornstillbad)
  printf 'this is not json {{{{' > "$result_path"
  sleep 2
  ;;
emptyfile)
  : > "$result_path"
  ;;
wrongpath)
  cat > "$result_path.wrong" <<EOF
{{"schema":"role_result@v1","invocation_id":"$inv","run_id":"01ARZ3NDEKTSV4RRFFQ69G5FAV","iteration_no":1,"role":"builder","context_envelope_digest":"$digest","state":"completed","payload_schema":"builder_result@v1","payload":{{"attempted":[],"changed_paths":[],"commands_run":[],"artifacts":[],"claims":[],"self_checks":[],"blocked_on":[]}},"payment":{{"input_tokens":0,"output_tokens":0,"tool_calls":0,"wall_seconds":0,"cost_usd":0}},"unavailable_evidence":[],"errors":[]}}
EOF
  ;;
foreignid)
  cat > "$result_path" <<EOF
{{"schema":"role_result@v1","invocation_id":"01ARZ3NDEKTSV4RRFFQ69G5FZZ","run_id":"01ARZ3NDEKTSV4RRFFQ69G5FAV","iteration_no":1,"role":"builder","context_envelope_digest":"$digest","state":"completed","payload_schema":"builder_result@v1","payload":{{"attempted":[],"changed_paths":[],"commands_run":[],"artifacts":[],"claims":[],"self_checks":[],"blocked_on":[]}},"payment":{{"input_tokens":0,"output_tokens":0,"tool_calls":0,"wall_seconds":0,"cost_usd":0}},"unavailable_evidence":[],"errors":[]}}
EOF
  ;;
foreigndigest)
  cat > "$result_path" <<EOF
{{"schema":"role_result@v1","invocation_id":"$inv","run_id":"01ARZ3NDEKTSV4RRFFQ69G5FAV","iteration_no":1,"role":"builder","context_envelope_digest":"sha256:deadbeefdeadbeef","state":"completed","payload_schema":"builder_result@v1","payload":{{"attempted":[],"changed_paths":[],"commands_run":[],"artifacts":[],"claims":[],"self_checks":[],"blocked_on":[]}},"payment":{{"input_tokens":0,"output_tokens":0,"tool_calls":0,"wall_seconds":0,"cost_usd":0}},"unavailable_evidence":[],"errors":[]}}
EOF
  ;;
diratpath)
  mkdir "$result_path"
  ;;
exited_nowrite)
  ;;
hangafterwrite)
  cat > "$result_path" <<EOF
{{"schema":"role_result@v1","invocation_id":"$inv","run_id":"01ARZ3NDEKTSV4RRFFQ69G5FAV","iteration_no":1,"role":"builder","context_envelope_digest":"$digest","state":"completed","payload_schema":"builder_result@v1","payload":{{"attempted":[],"changed_paths":[],"commands_run":[],"artifacts":[],"claims":[],"self_checks":[],"blocked_on":[]}},"payment":{{"input_tokens":0,"output_tokens":0,"tool_calls":0,"wall_seconds":0,"cost_usd":0}},"unavailable_evidence":[],"errors":[]}}
EOF
  sleep 121 & sp=$!
  echo "$sp" > "$result_path.sleeppid"
  wait "$sp"
  ;;
ignoreterm)
  trap '' TERM
  sleep 121 & sp=$!
  echo "$sp" > "$result_path.sleeppid"
  wait "$sp"
  ;;
esac
"#
    )
}

struct Live {
    state_dir: PathBuf,
    ws: PathBuf,
}

/// The wall-clock slack `wait_terminal` grants a live child before it
/// reports the still-Running status. This is test-infrastructure slack,
/// NOT a mechanism input: every asserted behavior here (contract wins
/// over exit status, process state never consulted) is timing
/// independent, and the spec-deadline path is tested separately. The
/// budget is sized for a LOADED host — this suite's full-battery runs
/// spawn many live children in parallel, and a 10s budget starved twice
/// on the 6-core WSL host (2026-09-01) while the same legs passed in
/// isolation every time.
const WAIT_BUDGET: u64 = 60;

fn live(mode: &str) -> (CliAgentRunner<ProcessChildTransport>, Live) {
    let env = Live {
        state_dir: scratch("state"),
        ws: scratch("ws"),
    };
    let script_path = env.ws.join("probe-child.sh");
    std::fs::write(&script_path, probe_script(mode)).expect("probe script writable");
    let template = format!(
        "sh {} {{prompt_file}} {{workspace}} {{result_path}}",
        script_path.to_string_lossy()
    );
    let runner = CliAgentRunner::new(
        CliAgentConfig::new(template),
        env.state_dir.clone(),
        env.ws.clone(),
        ProcessChildTransport::new(),
        Arc::new(SystemClock::new()),
    )
    .expect("template parses");
    (runner, env)
}

fn contract_file(env: &Live, id: &str) -> PathBuf {
    env.state_dir
        .join("runs")
        .join(ID_A)
        .join("inv")
        .join(id)
        .join("result.json")
}

fn child_pid_from_handle_file(env: &Live, id: &str) -> Option<u64> {
    let path = env
        .state_dir
        .join("runs")
        .join(ID_A)
        .join("inv")
        .join(id)
        .join("result.json.handle.json");
    let text = std::fs::read_to_string(path).ok()?;
    let at = text.find("\"child\":")?;
    let rest = &text[at + "\"child\":".len()..];
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

fn pid_alive(pid: u64) -> bool {
    Path::new("/proc").join(pid.to_string()).exists()
}

fn kill_pid(pid: u64) {
    std::process::Command::new("sh")
        .args(["-c", &format!("kill -9 {pid} 2>/dev/null || true")])
        .status()
        .ok();
}

fn wait_for(cond: impl Fn() -> bool, seconds: u64) -> bool {
    for _ in 0..(seconds * 20) {
        if cond() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    cond()
}

/// Harness note (orchestrator completing the fallen falsifier's round):
/// a REAL child needs a moment after invoke; a single immediate poll
/// lawfully reports Running. Bounded poll-until-terminal, 25 ms steps.
fn wait_terminal(
    runner: &CliAgentRunner<ProcessChildTransport>,
    handle: &InvocationHandle,
    seconds: u64,
) -> InvocationStatus {
    for _ in 0..(seconds * 40) {
        if let Ok(status) = runner.poll(handle)
            && status.is_terminal()
        {
            return status;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    runner.poll(handle).expect("poll after bounded wait")
}

/// Harness note: `with_extension` REPLACES ".json" — the probe children
/// SUFFIX the contract path, so siblings are built by file name.
fn sibling(env: &Live, id: &str, suffix: &str) -> PathBuf {
    contract_file(env, id)
        .parent()
        .expect("contract parent")
        .join(format!("result.json.{suffix}"))
}

// B1: a valid matching contract completes even on a hostile exit 7; the
// process state is NEVER consulted on this path (forensics stays None).
/// The torn-write alive window this regression polls through: long enough
/// that dozens of polls land inside it, short enough to keep the test
/// fast (the child's own `sleep 2` is the other half of the window).
const TORN_WINDOW: u64 = 1200;

// B6: the torn-read grace (plan task20). A LIVE child half-writes its
// document and the runner polls mid-write: the read fails to parse, and
// pre-correction that pinned `Failed{malformed_result}` over bytes that
// were still landing (observed under full-battery load: `MalformedResult`
// while the finished document sat complete on disk). A parse failure is
// decidable only once the child is terminal.
#[test]
fn b6_a_torn_write_while_alive_is_never_a_terminal_verdict() {
    let (runner, env) = live("tornwrite");
    let handle = runner.invoke(spec_with(ID_A, 30, None)).expect("admission");
    // Inside the child's alive window every poll must be non-terminal.
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(TORN_WINDOW);
    let mut polls = 0;
    while std::time::Instant::now() < deadline {
        match runner.poll(&handle).expect("poll reads") {
            InvocationStatus::Running { .. } => {}
            other => panic!("a live child's torn document must never terminalize: {other:?}"),
        }
        polls += 1;
        std::thread::sleep(Duration::from_millis(25));
    }
    assert!(polls > 0, "the alive window was polled");
    // The child then completes the document and exits: the exit re-read
    // admits it — the torn write cost nothing.
    match wait_terminal(&runner, &handle, WAIT_BUDGET) {
        InvocationStatus::Completed { .. } => {}
        other => panic!("the completed-over document wins at the exit re-read: {other:?}"),
    }
    let _ = std::fs::remove_dir_all(&env.state_dir);
    let _ = std::fs::remove_dir_all(&env.ws);
}

// B9 (torn-read half): the same torn write that NEVER completes — the
// malformed verdict must still arrive, at the exit re-read (rule 4), not
// before.
#[test]
fn b9_a_torn_write_that_stays_garbage_is_malformed_at_exit_not_before() {
    let (runner, env) = live("tornstillbad");
    let handle = runner.invoke(spec_with(ID_A, 30, None)).expect("admission");
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(TORN_WINDOW);
    while std::time::Instant::now() < deadline {
        match runner.poll(&handle).expect("poll reads") {
            InvocationStatus::Running { .. } => {}
            other => panic!("a live child's torn document must never terminalize: {other:?}"),
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    match wait_terminal(&runner, &handle, WAIT_BUDGET) {
        InvocationStatus::Failed {
            class: InvocationFailure::MalformedResult,
        } => {}
        other => panic!("the garbage document is malformed at exit: {other:?}"),
    }
    let _ = std::fs::remove_dir_all(&env.state_dir);
    let _ = std::fs::remove_dir_all(&env.ws);
}

#[test]
fn b1_contract_wins_over_exit7_and_process_is_never_consulted() {
    let (runner, env) = live("exit7");
    let handle = runner.invoke(spec_with(ID_A, 30, None)).expect("admission");
    match wait_terminal(&runner, &handle, WAIT_BUDGET) {
        InvocationStatus::Completed { .. } => {}
        other => panic!("expected Completed, got {other:?}"),
    }
    let forensics = runner.forensics(&handle).expect("forensics");
    assert_eq!(
        forensics.exit_status, None,
        "the exit status must never be a state input"
    );
    let collected = runner
        .collect(&handle, bound_from_now(5))
        .expect("collectible");
    assert_eq!(collected.invocation_id().as_str(), ID_A);
    let _ = std::fs::remove_dir_all(&env.state_dir);
    let _ = std::fs::remove_dir_all(&env.ws);
}

// B2/B3: garbage and EMPTY documents are malformed_result, on the FIRST
// poll — never Completed, never partially admitted.
#[test]
fn b2_garbage_and_empty_documents_are_malformed() {
    for mode in ["garbage", "emptyfile"] {
        let (runner, env) = live(mode);
        let handle = runner.invoke(spec_with(ID_A, 30, None)).expect("admission");
        match wait_terminal(&runner, &handle, WAIT_BUDGET) {
            InvocationStatus::Failed {
                class: InvocationFailure::MalformedResult,
            } => {}
            other => panic!("{mode}: expected malformed_result, got {other:?}"),
        }
        assert!(
            runner.collect(&handle, bound_from_now(5)).is_err(),
            "{mode}: a malformed document is never collectible"
        );
        let _ = std::fs::remove_dir_all(&env.state_dir);
        let _ = std::fs::remove_dir_all(&env.ws);
    }
}

// B4 (orchestrator-corrected to the pinned shape-2 order the falsifier
// was itself attacking): a child that writes to a DIFFERENT path and then
// EXITS is Failed{missing_result} BEFORE the deadline — a dead child can
// never write the declared path. The timeout path (TimedOut + stop) is
// separately proven with a child that STAYS RUNNING and silent.
#[test]
fn b4_wrong_path_exited_child_is_missing_result_before_deadline() {
    let (runner, env) = live("wrongpath");
    let handle = runner.invoke(spec_with(ID_A, 30, None)).expect("admission");
    match wait_terminal(&runner, &handle, WAIT_BUDGET) {
        InvocationStatus::Failed {
            class: InvocationFailure::MissingResult,
        } => {}
        other => panic!("exited wrong-path child must be missing_result, got {other:?}"),
    }
    let _ = std::fs::remove_dir_all(&env.state_dir);
    let _ = std::fs::remove_dir_all(&env.ws);
}

#[test]
fn b4b_running_silent_child_past_deadline_is_timed_out_and_stopped() {
    let (runner, env) = live("never");
    let handle = runner.invoke(spec_with(ID_A, 2, None)).expect("admission");
    let pid = child_pid_from_handle_file(&env, ID_A).expect("handle file has the pid");
    assert!(pid_alive(pid), "the child must be alive right after invoke");
    if let InvocationStatus::Running { payment_so_far, .. } =
        runner.poll(&handle).expect("first poll")
    {
        assert_eq!(
            payment_so_far,
            Payment::zero(),
            "the adapter's payment figure is zero — never a measurement"
        );
    }
    // Deadline seconds truncate; sleep past the NEXT second boundary.
    std::thread::sleep(Duration::from_millis(3200));
    match runner.poll(&handle).expect("timeout poll") {
        InvocationStatus::TimedOut => {}
        other => panic!("expected TimedOut, got {other:?}"),
    }
    assert!(
        wait_for(|| !pid_alive(pid), 5),
        "the TimedOut path must stop the child"
    );
    // Post-terminal stability.
    assert!(matches!(
        runner.poll(&handle).expect("stable poll"),
        InvocationStatus::TimedOut
    ));
    let _ = std::fs::remove_dir_all(&env.state_dir);
    let _ = std::fs::remove_dir_all(&env.ws);
}

// B5: foreign invocation_id AND stale envelope digest are
// contract_mismatch through the REAL transport.
#[test]
fn b5_foreign_identity_is_contract_mismatch() {
    for mode in ["foreignid", "foreigndigest"] {
        let (runner, env) = live(mode);
        let handle = runner.invoke(spec_with(ID_A, 30, None)).expect("admission");
        match wait_terminal(&runner, &handle, WAIT_BUDGET) {
            InvocationStatus::Failed {
                class: InvocationFailure::ContractMismatch,
            } => {}
            other => panic!("{mode}: expected contract_mismatch, got {other:?}"),
        }
        let _ = std::fs::remove_dir_all(&env.state_dir);
        let _ = std::fs::remove_dir_all(&env.ws);
    }
}

// D: the §10.3 rule-1 channels — the child RECEIVES
// GAUNTLET_RESULT_PATH (equal to the declared path), the prompt file
// carries the same RESULT-PATH, and the envelope BODY sibling is a
// byte-exact copy of what the runtime carried.
#[test]
fn d_env_and_prompt_channels_and_envelope_sibling_are_exact() {
    let (runner, env) = live("envobs");
    let handle = runner.invoke(spec_with(ID_A, 30, None)).expect("admission");
    // The child observes its channels, then EXITS without writing the
    // contract — the pinned verdict is Failed{missing_result} (before the
    // deadline). The observations must all be present first.
    assert!(wait_for(|| sibling(&env, ID_A, "envobs").is_file(), 5));
    let declared = contract_file(&env, ID_A);
    let envobs = std::fs::read_to_string(sibling(&env, ID_A, "envobs")).expect("env observed");
    assert_eq!(
        Path::new(&envobs),
        declared.as_path(),
        "GAUNTLET_RESULT_PATH must equal the declared contract path"
    );
    let prompt_copy =
        std::fs::read_to_string(sibling(&env, ID_A, "promptobs")).expect("prompt observed");
    let result_line = prompt_copy
        .lines()
        .find(|line| line.starts_with("RESULT-PATH: "))
        .expect("RESULT-PATH field");
    assert_eq!(
        result_line,
        format!("RESULT-PATH: {}", declared.display()),
        "the prompt channel carries the same path"
    );
    assert!(
        prompt_copy.contains("MODEL: \n"),
        "no model hint means an EMPTY MODEL field, not a lie"
    );
    // The envelope sibling: byte-exact with the runtime-carried body.
    let envelope = env
        .state_dir
        .join("runs")
        .join(ID_A)
        .join("inv")
        .join(ID_A)
        .join("result.json.envelope.json");
    let body = std::fs::read_to_string(&envelope).expect("envelope sibling");
    assert_eq!(
        body, "line one of the envelope body\nline two with \"quotes\" and {braces} and $dollar",
        "the envelope body travels verbatim (never recomputed)"
    );
    // Settle the invocation: the observing child EXITED without writing
    // the contract — missing_result, not TimedOut.
    assert!(matches!(
        wait_terminal(&runner, &handle, WAIT_BUDGET),
        InvocationStatus::Failed {
            class: InvocationFailure::MissingResult
        }
    ));
    let _ = std::fs::remove_dir_all(&env.state_dir);
    let _ = std::fs::remove_dir_all(&env.ws);
}

// B7: a DIRECTORY at the contract path is an observation failure (typed
// error), never a coerced Running and never a verdict.
#[test]
fn b7_directory_at_contract_path_is_an_error_not_running() {
    let (runner, env) = live("diratpath");
    let handle = runner.invoke(spec_with(ID_A, 30, None)).expect("admission");
    // The directory appears once the child has run; poll until the
    // observation failure surfaces (bounded), then require persistence.
    let mut first = Ok(InvocationStatus::Admitted);
    for _ in 0..200 {
        first = runner.poll(&handle);
        if first.is_err() {
            break;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    assert!(
        first.is_err(),
        "an unreadable contract must be a typed error, got {first:?}"
    );
    // And it must not have pinned any terminal state.
    let second = runner.poll(&handle);
    assert!(
        second.is_err(),
        "the observation failure persists, {second:?}"
    );
    let _ = std::fs::remove_dir_all(&env.state_dir);
    let _ = std::fs::remove_dir_all(&env.ws);
}

// B8: a contract that appears only AFTER the missing_result verdict can
// never resurrect it (post-terminal stability against a late writer).
#[test]
fn b8_late_contract_never_resurrects_missing_result() {
    let (runner, env) = live("exited_nowrite");
    let handle = runner.invoke(spec_with(ID_A, 30, None)).expect("admission");
    match wait_terminal(&runner, &handle, WAIT_BUDGET) {
        InvocationStatus::Failed {
            class: InvocationFailure::MissingResult,
        } => {}
        other => panic!("expected missing_result, got {other:?}"),
    }
    // The hostile late writer: a VALID matching document lands after the
    // verdict.
    std::fs::write(
        contract_file(&env, ID_A),
        format!(
            r#"{{"schema":"role_result@v1","invocation_id":"{ID_A}","run_id":"{ID_A}","iteration_no":1,"role":"builder","context_envelope_digest":"sha256:{DIGEST_HEX}","state":"completed","payload_schema":"builder_result@v1","payload":{{"attempted":[],"changed_paths":[],"commands_run":[],"artifacts":[],"claims":[],"self_checks":[],"blocked_on":[]}},"payment":{{"input_tokens":0,"output_tokens":0,"tool_calls":0,"wall_seconds":0,"cost_usd":0}},"unavailable_evidence":[],"errors":[]}}"#
        ),
    )
    .expect("late write");
    for attempt in 1..=3 {
        match runner.poll(&handle).expect("post-terminal poll") {
            InvocationStatus::Failed {
                class: InvocationFailure::MissingResult,
            } => {}
            other => panic!("attempt {attempt}: a late writer resurrected {other:?}"),
        }
    }
    let _ = std::fs::remove_dir_all(&env.state_dir);
    let _ = std::fs::remove_dir_all(&env.ws);
}

// C: cancel lifecycle — first cancel stops the real child under the
// caller's bound; the state pins Cancelled; a second cancel is the typed
// no-op; a stranger is UnknownHandle; a cancel on an already-exited
// (unpolled) child succeeds; cancel after terminal is the no-op.
#[test]
fn c_cancel_lifecycle_with_real_children() {
    // (a) live child.
    let (runner, env) = live("never");
    let handle = runner
        .invoke(spec_with(ID_A, 120, None))
        .expect("admission");
    let pid = child_pid_from_handle_file(&env, ID_A).expect("pid");
    assert!(pid_alive(pid));
    let started = std::time::Instant::now();
    runner
        .cancel(
            &handle,
            CancelReason::new("falsifier").expect("reason"),
            bound_from_now(5),
        )
        .expect("first cancel");
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "cancel is bounded by the caller's deadline"
    );
    assert!(
        wait_for(|| !pid_alive(pid), 5),
        "the first cancel must stop the real child"
    );
    assert!(matches!(
        runner.poll(&handle).expect("poll after cancel"),
        InvocationStatus::Cancelled
    ));
    // (b) second cancel: the typed no-op, never a second effect.
    assert!(matches!(
        runner.cancel(
            &handle,
            CancelReason::new("again").expect("reason"),
            bound_from_now(5)
        ),
        Err(AgentError::AlreadyTerminal { .. })
    ));
    // (c) a stranger handle.
    let stranger = InvocationHandle::new(InvocationId::new(ID_B).expect("id"), None);
    assert!(matches!(
        runner.cancel(
            &stranger,
            CancelReason::new("stray").expect("reason"),
            bound_from_now(5)
        ),
        Err(AgentError::UnknownHandle { .. })
    ));
    // Clean up the script's sleep grandchild if any.
    if let Ok(sleep_pid) = std::fs::read_to_string(sibling(&env, ID_A, "sleeppid")) {
        kill_pid(sleep_pid.trim().parse().expect("sleep pid"));
    }
    let _ = std::fs::remove_dir_all(&env.state_dir);
    let _ = std::fs::remove_dir_all(&env.ws);

    // (d) cancel on an already-exited child that was never polled.
    let (runner2, env2) = live("exited_nowrite");
    let handle2 = runner2
        .invoke(spec_with(ID_A, 120, None))
        .expect("admission");
    runner2
        .cancel(
            &handle2,
            CancelReason::new("post-exit").expect("reason"),
            bound_from_now(5),
        )
        .expect("cancel after natural exit succeeds");
    assert!(matches!(
        runner2.poll(&handle2).expect("poll"),
        InvocationStatus::Cancelled
    ));
    let _ = std::fs::remove_dir_all(&env2.state_dir);
    let _ = std::fs::remove_dir_all(&env2.ws);
}

// C: a child that IGNORES SIGTERM is escalated to SIGKILL within the
// bound (bounded escalation).
#[test]
fn c_sigterm_ignoring_child_is_escalated_within_bound() {
    let (runner, env) = live("ignoreterm");
    let handle = runner
        .invoke(spec_with(ID_A, 120, None))
        .expect("admission");
    let pid = child_pid_from_handle_file(&env, ID_A).expect("pid");
    assert!(
        wait_for(|| sibling(&env, ID_A, "sleeppid").is_file(), 5),
        "the probe child writes its sleep pid"
    );
    let sleep_pid: u64 = std::fs::read_to_string(sibling(&env, ID_A, "sleeppid"))
        .expect("sleep pid file")
        .trim()
        .parse()
        .expect("pid number");
    let started = std::time::Instant::now();
    runner
        .cancel(
            &handle,
            CancelReason::new("stubborn").expect("reason"),
            bound_from_now(4),
        )
        .expect("bounded escalation must succeed");
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "escalation must stay bounded, took {:?}",
        started.elapsed()
    );
    assert!(wait_for(|| !pid_alive(pid), 5), "the child must be dead");
    kill_pid(sleep_pid);
    let _ = std::fs::remove_dir_all(&env.state_dir);
    let _ = std::fs::remove_dir_all(&env.ws);
}

// C FINDING → CORRECTED (COR-F4): a child that writes the contract then
// HANGS completes WITHOUT EXIT (correct — the contract is the completion
// fact), and the terminal pin RETIRES the live child best-effort: no
// invocation leaves a live child behind, and the stop's outcome is never
// a state input (the verdict stays Completed).
#[test]
fn c_hang_after_write_completes_and_the_child_is_retired() {
    let (runner, env) = live("hangafterwrite");
    let handle = runner
        .invoke(spec_with(ID_A, 120, None))
        .expect("admission");
    let pid = child_pid_from_handle_file(&env, ID_A).expect("pid");
    assert!(
        wait_for(|| sibling(&env, ID_A, "sleeppid").is_file(), 5),
        "the probe child writes its sleep pid"
    );
    let sleep_pid: u64 = std::fs::read_to_string(sibling(&env, ID_A, "sleeppid"))
        .expect("sleep pid file")
        .trim()
        .parse()
        .expect("pid number");
    assert!(matches!(
        wait_terminal(&runner, &handle, 20),
        InvocationStatus::Completed { .. }
    ));
    // CORRECTED: the completed invocation's child is retired (bounded).
    assert!(
        wait_for(|| !pid_alive(pid), 15),
        "the terminal pin must retire the live child"
    );
    // The verdict was never changed by the retirement.
    assert!(matches!(
        runner.poll(&handle).expect("stable poll"),
        InvocationStatus::Completed { .. }
    ));
    kill_pid(sleep_pid);
    let _ = std::fs::remove_dir_all(&env.state_dir);
    let _ = std::fs::remove_dir_all(&env.ws);
}

// C: two invocations on ONE runner instance — no cross-wiring of
// handles, children, or verdicts; duplicate admission is refused with
// the original entry intact.
#[test]
fn c_two_invocations_no_cross_wiring() {
    let (runner, env) = live("complete");
    let good = runner
        .invoke(spec_with(ID_A, 30, None))
        .expect("first admission");
    let second = runner.invoke(spec_with(ID_A, 30, None));
    assert!(
        matches!(&second, Err(AgentError::AdmissionRefused { .. })),
        "duplicate admission must be refused, got {second:?}"
    );
    let silent = runner
        .invoke(spec_with(ID_B, 30, None))
        .expect("second admission");
    // Both children are complete-mode: both terminal is Completed. The
    // property under attack is NO CROSS-WIRING — each verdict belongs to
    // its own invocation, and the collected identities differ.
    assert!(matches!(
        wait_terminal(&runner, &good, 10),
        InvocationStatus::Completed { .. }
    ));
    assert!(matches!(
        wait_terminal(&runner, &silent, 10),
        InvocationStatus::Completed { .. }
    ));
    assert_eq!(
        runner
            .collect(&good, bound_from_now(5))
            .expect("good collect")
            .invocation_id()
            .as_str(),
        ID_A
    );
    assert_eq!(
        runner
            .collect(&silent, bound_from_now(5))
            .expect("silent collect")
            .invocation_id()
            .as_str(),
        ID_B,
        "the second invocation must collect ITS OWN document, never the first's"
    );
    assert!(
        matches!(
            runner.poll(&good).expect("good stays"),
            InvocationStatus::Completed { .. }
        ),
        "no cross-wiring from the second invocation"
    );
    let _ = std::fs::remove_dir_all(&env.state_dir);
    let _ = std::fs::remove_dir_all(&env.ws);
}

// C: forged handles are typed UnknownHandle everywhere.
#[test]
fn c_forged_handle_is_unknown_everywhere() {
    let (runner, env) = live("complete");
    let forged = InvocationHandle::new(InvocationId::new(ID_B).expect("id"), None);
    assert!(matches!(
        runner.poll(&forged),
        Err(AgentError::UnknownHandle { .. })
    ));
    assert!(matches!(
        runner.collect(&forged, bound_from_now(5)),
        Err(AgentError::UnknownHandle { .. })
    ));
    assert!(matches!(
        runner.cancel(
            &forged,
            CancelReason::new("x").expect("r"),
            bound_from_now(5)
        ),
        Err(AgentError::UnknownHandle { .. })
    ));
    let _ = std::fs::remove_dir_all(&env.state_dir);
    let _ = std::fs::remove_dir_all(&env.ws);
}

// B14: a spawn failure is the typed Transport class, not a silent
// admission.
#[test]
fn b14_spawn_failure_is_transport() {
    let state = scratch("spawn-state");
    let ws = scratch("spawn-ws");
    let runner = CliAgentRunner::new(
        CliAgentConfig::new("/nonexistent/falsifier-program {prompt_file}"),
        state.clone(),
        ws.clone(),
        ProcessChildTransport::new(),
        Arc::new(SystemClock::new()),
    )
    .expect("template parses");
    let error = runner
        .invoke(spec_with(ID_A, 30, None))
        .expect_err("spawn must fail");
    assert!(
        matches!(error, AgentError::Transport { .. }),
        "spawn failure must be Transport, got {error:?}"
    );
    let _ = std::fs::remove_dir_all(&state);
    let _ = std::fs::remove_dir_all(&ws);
}

// B15: the deadline boundary is EXACT through the real runner — a poll
// at now == deadline is NOT TimedOut (exceeded_at is strict), the next
// instant is.
#[test]
fn b15_deadline_boundary_is_exact() {
    let transport = ScriptedProcessTransport::new(
        gauntlet_ports::contract::agent_runner::RunnerScript::NeverWriteResult,
    );
    let state = scratch("b15-state");
    let ws = scratch("b15-ws");
    let clock = Arc::new(SteppingClock::new());
    let runner = CliAgentRunner::new(
        CliAgentConfig::new("cli-probe {prompt_file} {workspace} {result_path}"),
        state.clone(),
        ws.clone(),
        transport,
        clock.clone(),
    )
    .expect("parses");
    let mut spec = spec_with(ID_A, 3600, None);
    spec.deadline = Deadline::at_epoch(EpochSeconds::new(BASE_INSTANT + 30));
    let handle = runner.invoke(spec).expect("admission");
    // now() == BASE+30 exactly: NOT past the deadline.
    clock
        .sleep_until(EpochSeconds::new(BASE_INSTANT + 28))
        .expect("stepped to the boundary");
    match runner.poll(&handle).expect("boundary poll") {
        InvocationStatus::Running { .. } => {}
        other => panic!("now == deadline must still be Running, got {other:?}"),
    }
    // The next instant IS past it.
    match runner.poll(&handle).expect("past poll") {
        InvocationStatus::TimedOut => {}
        other => panic!("now > deadline must be TimedOut, got {other:?}"),
    }
    let _ = std::fs::remove_dir_all(&state);
    let _ = std::fs::remove_dir_all(&ws);
}

// H (§18.3): after the RUNNER (runtime) dies, the retained forensic
// handle file persists, is parseable, and cites a REAL child pid; a
// fresh runner instance knows nothing (discovery is deferred to the
// checkpoint path, not this runner) — the file is evidence, and the
// contract path itself is re-probeable per §18.3 step 3.
#[test]
fn h_orphan_handle_file_survives_runtime_death_and_is_parseable() {
    let (runner, env) = live("never");
    let handle = runner
        .invoke(spec_with(ID_A, 120, None))
        .expect("admission");
    let pid = child_pid_from_handle_file(&env, ID_A).expect("handle file parseable with pid");
    assert!(pid_alive(pid), "the cited pid is the real child");
    let handle_path = env
        .state_dir
        .join("runs")
        .join(ID_A)
        .join("inv")
        .join(ID_A)
        .join("result.json.handle.json");
    let doc = std::fs::read_to_string(&handle_path).expect("handle file readable");
    assert!(doc.contains(ID_A), "carries the invocation identity: {doc}");
    // "Runtime death": drop the runner without any terminal transition.
    drop(runner);
    let doc2 = std::fs::read_to_string(&handle_path).expect("survives the runtime");
    assert!(doc2 == doc, "unchanged evidence");
    // A FRESH runner instance: the orphan is discovered via the retained
    // contract path per §18.3 (a valid document would be collected; here
    // the child never wrote one). The fresh instance itself refuses the
    // old handle — admission state does not survive restart.
    let fresh = CliAgentRunner::new(
        CliAgentConfig::new(format!(
            "sh {} {{prompt_file}} {{workspace}} {{result_path}}",
            env.ws.join("probe-child.sh").to_string_lossy()
        )),
        env.state_dir.clone(),
        env.ws.clone(),
        ProcessChildTransport::new(),
        Arc::new(SystemClock::new()),
    )
    .expect("fresh runner");
    assert!(
        matches!(fresh.poll(&handle), Err(AgentError::UnknownHandle { .. })),
        "a fresh instance must not fake knowledge of the orphan"
    );
    // §18.3 step 3's actual discovery input: the contract path. The
    // orphaned invocation's contract file does not exist (nothing valid
    // to collect) — the resume story is the runtime's, not the runner's.
    assert!(!contract_file(&env, ID_A).exists());
    kill_pid(pid);
    if let Ok(sleep_pid) = std::fs::read_to_string(sibling(&env, ID_A, "sleeppid")) {
        kill_pid(sleep_pid.trim().parse().expect("pid"));
    }
    let _ = std::fs::remove_dir_all(&env.state_dir);
    let _ = std::fs::remove_dir_all(&env.ws);
}

// F-candidate (doc claim): lib.rs says the contract is read "in exactly
// two places — poll ... and collect". Collect actually serves the
// cache from poll; the file is NOT re-read (tamper-proof by snapshot,
// but the doc's claim is false). Execute the observable.
#[test]
fn f_collect_serves_the_poll_snapshot_not_the_file() {
    let (runner, env) = live("complete");
    let handle = runner.invoke(spec_with(ID_A, 30, None)).expect("admission");
    assert!(matches!(
        wait_terminal(&runner, &handle, WAIT_BUDGET),
        InvocationStatus::Completed { .. }
    ));
    // Tamper with the on-disk contract AFTER completion.
    std::fs::write(contract_file(&env, ID_A), "tampered after the fact").expect("tamper");
    let collected = runner
        .collect(&handle, bound_from_now(5))
        .expect("collect still serves the validated snapshot");
    assert_eq!(collected.invocation_id().as_str(), ID_A);
    // collect BEFORE any poll (document exists, verdict not yet reached):
    // the cached result is empty, so collect refuses — the doc's "read at
    // collect" does not happen.
    let _ = std::fs::remove_dir_all(&env.state_dir);
    let _ = std::fs::remove_dir_all(&env.ws);
}

// J: capabilities honesty with a {model} template: model selection is a
// capability EXACTLY when configured; isolation stays none; delivery
// stays file.
#[test]
fn j_capabilities_derive_from_the_template() {
    let state = scratch("j-state");
    let ws = scratch("j-ws");
    let runner = CliAgentRunner::new(
        CliAgentConfig::new("agent --model {model} --prompt {prompt_file}"),
        state.clone(),
        ws.clone(),
        ScriptedProcessTransport::new(
            gauntlet_ports::contract::agent_runner::RunnerScript::CompleteImmediately,
        ),
        Arc::new(SteppingClock::new()),
    )
    .expect("parses");
    let caps = runner.capabilities();
    assert!(
        caps.supports_model_selection,
        "a {{model}} template must report it"
    );
    assert_eq!(caps.isolation, Isolation::None);
    assert_eq!(
        caps.result_delivery,
        gauntlet_ports::agent_runner::ResultDelivery::File
    );
    assert!(!caps.supports_native_subagents);
    let _ = std::fs::remove_dir_all(&state);
    let _ = std::fs::remove_dir_all(&ws);
}
