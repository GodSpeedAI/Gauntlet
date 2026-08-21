//! The eight REQUIRED scenario fixtures, played back through the mock
//! runner — each driving a distinct, asserted runtime state (plan Task 8
//! step 2; AK-059: "The mock AgentRunner can replay scripted results
//! including malformed output, timeout, and a reference-mutation
//! attempt"). The fixtures live at `tests/fixtures/mock_runner_scenarios`
//! under the repository root — AK-059's frozen locator spelling.

use std::path::{Path, PathBuf};

use gauntlet_adapter_agent_mock::{MockAgentRunner, load_scenario, scenario_fixtures_dir};
use gauntlet_domain::{
    Budget, DiagnosticAllowance, Digest, EpochSeconds, InvocationId, InvocationState, Limits,
    MicroUsd, PermissionSetRef, ResultContractPath, RoleId, RunId, WorkspaceRef,
};
use gauntlet_ports::agent_runner::{
    AgentError, AgentRunner, ContextEnvelope, InvocationFailure, InvocationStatus,
    RoleInvocationSpec,
};
use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::role_result::{RolePayload, RoleResultState};

const ULID: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
const DIGEST: &str = "sha256:a1b2c3d4e5f6a7b8";

/// The eight REQUIRED scenario names, pinned: the fixture inventory is
/// part of the locator AK-059 spells.
const REQUIRED: &[&str] = &[
    "happy",
    "malformed",
    "no-result",
    "wrong-invocation-id",
    "stale-envelope-digest",
    "lying-builder",
    "answer-key-write-attempt",
    "critic-pass-without-evidence",
];

/// The repository root, reached through the crate's manifest dir — the
/// established no-absolute-path pattern (read-only: seeding copies and
/// byte-comparing the real ruler).
fn repo_root() -> PathBuf {
    scenario_fixtures_dir()
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .expect("the fixtures dir sits three levels under the repository root")
        .to_path_buf()
}

/// Isolated scratch workspace per test; removed on drop. `tempfile` is not
/// on any layer's external allowlist, and a test helper is not a reason to
/// widen one.
struct Scratch {
    path: PathBuf,
}

impl Scratch {
    fn new(tag: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "gauntlet-mock-playback-{tag}-{}",
            std::process::id()
        ));
        std::fs::remove_dir_all(&path).ok();
        std::fs::create_dir_all(&path).expect("playback scratch is creatable");
        Self { path }
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.path).ok();
    }
}

fn dispatch_spec() -> RoleInvocationSpec {
    let budget = Budget::new(
        Limits::new(8, 2, 2, 3600, 1000, MicroUsd::new(500)).expect("playback limits usable"),
        20,
        DiagnosticAllowance::new(100, MicroUsd::new(50)),
    )
    .expect("playback budget usable");
    RoleInvocationSpec {
        invocation_id: InvocationId::new(ULID).expect("playback invocation id is a ULID"),
        run_id: RunId::new(ULID).expect("playback run id is a ULID"),
        iteration_no: gauntlet_domain::IterationNo::new(1).expect("1-based iteration"),
        role: RoleId::Builder,
        prompt: "run the assigned slice and write the typed result".to_owned(),
        context_envelope: ContextEnvelope::new(
            "the runtime-built allowlisted context".to_owned(),
            Digest::parse(DIGEST).expect("playback digest is well formed"),
        ),
        workspace: WorkspaceRef::new("ws-playback").expect("playback workspace ref"),
        permissions: PermissionSetRef::new("perm-readonly").expect("playback permission ref"),
        result_contract_path: ResultContractPath::new("playback-result-document")
            .expect("playback contract path"),
        deadline: Deadline::at_epoch(EpochSeconds::new(1_700_000_090)),
        model_hint: None,
        sandbox_policy: None,
        budget_slice: budget,
    }
}

fn runner_for(name: &str, root: &Path) -> MockAgentRunner {
    MockAgentRunner::new(
        load_scenario(name).expect("the REQUIRED fixture parses"),
        root.to_path_buf(),
    )
}

/// Polls until a terminal status appears, bounded: a playback that never
/// terminates is a defect, not a pass (the Task-7 lesson travels).
fn drive_to_terminal(
    runner: &MockAgentRunner,
    handle: &gauntlet_ports::agent_runner::InvocationHandle,
) -> InvocationStatus {
    for _ in 0..24 {
        let status = runner.poll(handle).expect("poll must not error");
        if status.is_terminal() {
            return status;
        }
    }
    panic!("no terminal state within the bounded probe run — a non-terminating script is a defect");
}

#[test]
fn the_required_fixture_inventory_exists_and_parses() {
    // AK-059's locator, materialized as written: every REQUIRED scenario
    // is present, parses, and declares its own stem as its identity.
    for name in REQUIRED {
        let scenario = load_scenario(name).unwrap_or_else(|error| panic!("{name}: {error}"));
        assert_eq!(scenario.name(), *name);
    }
    assert!(
        scenario_fixtures_dir().join("happy.yaml").is_file(),
        "the fixtures directory is the locator's exact spelling"
    );
}

#[test]
fn happy_completes_and_collects_echoing_dispatch_identity_exactly() {
    let root = Scratch::new("happy");
    let runner = runner_for("happy", &root.path);
    let spec = dispatch_spec();
    let handle = runner.invoke(spec.clone()).expect("admission");

    // One probe of in-flight work (the fixture's probes_before_write: 1):
    // admission never blocks on completion.
    assert!(
        matches!(runner.poll(&handle), Ok(InvocationStatus::Running { .. })),
        "the scripted in-flight probe must report Running"
    );
    match drive_to_terminal(&runner, &handle) {
        InvocationStatus::Completed { result_ref } => {
            assert_eq!(result_ref.as_str(), spec.result_contract_path.as_str());
        }
        other => panic!("happy must Complete, got {other:?}"),
    }
    let result = runner
        .collect(
            &handle,
            Deadline::at_epoch(EpochSeconds::new(1_700_000_090)),
        )
        .expect("a completed invocation is collectible");
    // The document echoes the dispatch identity exactly (§10.3 rule 6).
    assert_eq!(result.invocation_id().as_str(), spec.invocation_id.as_str());
    assert_eq!(result.run_id().as_str(), spec.run_id.as_str());
    assert_eq!(result.iteration_no(), spec.iteration_no);
    assert_eq!(
        result.context_envelope_digest(),
        spec.context_envelope.digest()
    );
    assert_eq!(result.state(), RoleResultState::Completed);
    // Repeated collects return the same document.
    let again = runner
        .collect(
            &handle,
            Deadline::at_epoch(EpochSeconds::new(1_700_000_090)),
        )
        .expect("collectible again");
    assert_eq!(result, again);
}

#[test]
fn malformed_fails_and_is_never_collectible() {
    let root = Scratch::new("malformed");
    let runner = runner_for("malformed", &root.path);
    let handle = runner.invoke(dispatch_spec()).expect("admission");
    match drive_to_terminal(&runner, &handle) {
        InvocationStatus::Failed {
            class: InvocationFailure::MalformedResult,
        } => {}
        other => panic!("a malformed document must be Failed{{malformed_result}}, got {other:?}"),
    }
    // Stable forever after.
    for _ in 0..3 {
        assert!(matches!(
            runner.poll(&handle),
            Ok(InvocationStatus::Failed {
                class: InvocationFailure::MalformedResult
            })
        ));
    }
    let error = runner
        .collect(
            &handle,
            Deadline::at_epoch(EpochSeconds::new(1_700_000_090)),
        )
        .expect_err("a malformed document is never collectible");
    assert!(
        matches!(error, AgentError::ResultNotReady { .. }),
        "got {error:?}"
    );
}

#[test]
fn no_result_times_out_and_never_completes() {
    let root = Scratch::new("no-result");
    let runner = runner_for("no-result", &root.path);
    let handle = runner.invoke(dispatch_spec()).expect("admission");
    // The fixture stalls for three probes: two Running, then TimedOut.
    assert!(matches!(
        runner.poll(&handle),
        Ok(InvocationStatus::Running { .. })
    ));
    assert!(matches!(
        runner.poll(&handle),
        Ok(InvocationStatus::Running { .. })
    ));
    assert_eq!(
        drive_to_terminal(&runner, &handle),
        InvocationStatus::TimedOut
    );
    assert_eq!(
        runner.poll(&handle).expect("terminal stability"),
        InvocationStatus::TimedOut
    );
    let error = runner
        .collect(
            &handle,
            Deadline::at_epoch(EpochSeconds::new(1_700_000_090)),
        )
        .expect_err("TimedOut is never collectible");
    assert!(
        matches!(
            error,
            AgentError::ResultNotReady {
                state: InvocationState::TimedOut,
                ..
            }
        ),
        "got {error:?}"
    );
}

#[test]
fn wrong_invocation_id_fails_contract_mismatch() {
    let root = Scratch::new("wrong-invocation-id");
    let runner = runner_for("wrong-invocation-id", &root.path);
    let handle = runner.invoke(dispatch_spec()).expect("admission");
    match drive_to_terminal(&runner, &handle) {
        InvocationStatus::Failed {
            class: InvocationFailure::ContractMismatch,
        } => {}
        other => {
            panic!("a foreign invocation id must be Failed{{contract_mismatch}}, got {other:?}")
        }
    }
}

#[test]
fn stale_envelope_digest_fails_contract_mismatch() {
    let root = Scratch::new("stale-envelope-digest");
    let runner = runner_for("stale-envelope-digest", &root.path);
    let handle = runner.invoke(dispatch_spec()).expect("admission");
    match drive_to_terminal(&runner, &handle) {
        InvocationStatus::Failed {
            class: InvocationFailure::ContractMismatch,
        } => {}
        other => panic!("a stale digest must be Failed{{contract_mismatch}}, got {other:?}"),
    }
}

#[test]
fn lying_builder_divergence_is_observable() {
    let root = Scratch::new("lying-builder");
    let runner = runner_for("lying-builder", &root.path);
    let handle = runner.invoke(dispatch_spec()).expect("admission");
    match drive_to_terminal(&runner, &handle) {
        InvocationStatus::Completed { .. } => {}
        other => panic!("the lying builder Completes — the lie is in the claims, got {other:?}"),
    }
    let result = runner
        .collect(
            &handle,
            Deadline::at_epoch(EpochSeconds::new(1_700_000_090)),
        )
        .expect("collectible: the envelope itself is valid");
    // Side A — the artifact on disk fails the check (instrument view).
    let artifact = std::fs::read_to_string(root.path.join("out/check-target.txt"))
        .expect("the scripted effect wrote the artifact");
    assert!(
        artifact.contains("FAIL") && !artifact.contains("PASS"),
        "the artifact fails the declared self-check: {artifact:?}"
    );
    // Side B — the document claims success (forensic view).
    let RolePayload::BuilderResult(payload) = result.payload() else {
        panic!("the builder fixture writes the builder payload")
    };
    assert!(
        !payload.claims().is_empty(),
        "the claims carry the success story"
    );
    let check = &payload.self_checks()[0];
    assert_eq!(check.command(), "grep PASS out/check-target.txt");
    assert_eq!(check.exit_code(), 0, "the document claims the check passed");
    // The divergence is exactly the two sides coexisting.
    assert!(
        artifact.contains("FAIL") && check.exit_code() == 0,
        "the divergence is observable: failing artifact AND passing claim"
    );
}

#[test]
fn answer_key_write_attempt_is_observable_and_the_real_ruler_stays_untouched() {
    let root = Scratch::new("answer-key");
    let runner = runner_for("answer-key-write-attempt", &root.path);
    // The stand-in ruler the scenario targets, pre-seeded inside the
    // test-provided workspace root — the fixture names relative paths only.
    let standin = root.path.join("ruler-standin/answer-key.md");
    std::fs::create_dir_all(standin.parent().expect("the stand-in has a parent"))
        .expect("stand-in directory is creatable");
    const SENTINEL: &str = "the frozen ruler's original bytes";
    std::fs::write(&standin, SENTINEL).expect("the stand-in is seedable");

    // The REAL ruler, byte-read before and after — the scenario must never
    // reach it (the final `just ruler` gate re-verifies this at the
    // digest level).
    let real_ruler = repo_root().join(".wayfinder/answer-key.md");
    let ruler_before = std::fs::read(&real_ruler).expect("the real ruler is readable");

    let handle = runner.invoke(dispatch_spec()).expect("admission");
    match drive_to_terminal(&runner, &handle) {
        InvocationStatus::Completed { .. } => {}
        other => panic!("the mutating child Completes so the attempt is auditable, got {other:?}"),
    }
    // The attempt is observable: the stand-in's bytes changed…
    let mutated = std::fs::read_to_string(&standin).expect("the stand-in is readable");
    assert_ne!(mutated, SENTINEL, "the write attempt changed the stand-in");
    assert!(mutated.contains("MUTATED BY THE SCRIPTED CHILD"));
    // …and the document owns up to the path it touched.
    let result = runner
        .collect(
            &handle,
            Deadline::at_epoch(EpochSeconds::new(1_700_000_090)),
        )
        .expect("collectible");
    let RolePayload::BuilderResult(payload) = result.payload() else {
        panic!("the builder fixture writes the builder payload")
    };
    assert_eq!(
        payload.changed_paths(),
        &["ruler-standin/answer-key.md".to_owned()]
    );
    // The real ruler is byte-identical.
    let ruler_after = std::fs::read(&real_ruler).expect("the real ruler is still readable");
    assert_eq!(
        ruler_before, ruler_after,
        "the scripted child never touched the real ruler"
    );
}

#[test]
fn critic_pass_without_evidence_pins_the_p11_shape_at_the_envelope_level() {
    // Design note (recorded in the Task-8 report): Annex B defines exactly
    // one role-result payload schema (builder_result@v1); whether a
    // critic's comparison-record output flows through the runner port is
    // a Task 10 question. This fixture pins the P11 shape HERE: a
    // schema-valid envelope whose claims carry the pass claim while both
    // evidence-bearing payload arrays are empty.
    let root = Scratch::new("critic");
    let runner = runner_for("critic-pass-without-evidence", &root.path);
    let handle = runner.invoke(dispatch_spec()).expect("admission");
    match drive_to_terminal(&runner, &handle) {
        InvocationStatus::Completed { .. } => {}
        other => panic!("the envelope is schema-valid, so it Completes, got {other:?}"),
    }
    let result = runner
        .collect(
            &handle,
            Deadline::at_epoch(EpochSeconds::new(1_700_000_090)),
        )
        .expect("completed and collectible");
    assert_eq!(result.role(), &RoleId::Critic);
    assert_eq!(result.state(), RoleResultState::Completed);
    let RolePayload::BuilderResult(payload) = result.payload() else {
        panic!("the builder fixture writes the builder payload")
    };
    assert!(!payload.claims().is_empty(), "the pass claim is present");
    assert!(
        payload
            .claims()
            .iter()
            .any(|claim| claim.contains("passes")),
        "the claim says pass: {:?}",
        payload.claims()
    );
    assert!(
        payload.artifacts().is_empty(),
        "the evidence-bearing artifacts array is empty"
    );
    assert!(
        payload.self_checks().is_empty(),
        "the evidence-bearing self_checks array is empty"
    );
}
