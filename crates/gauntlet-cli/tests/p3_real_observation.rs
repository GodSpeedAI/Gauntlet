//! P3 proof (plan Task 9 step 5; spec §23.1 P3; AK-029 with AK-028
//! corroborating): REAL observation of a lying builder's resulting
//! reality, composed at the composition root — the one crate allowed to
//! name adapters (spec §4.1).
//!
//! Location rationale (recorded per the task brief): the plan spells the
//! proof `tests/proofs/p3_real_observation`, but the architecture
//! boundary suite's cargo-target-path rule (exemption list exactly
//! `crates/gauntlet-arch-tests`) forbids a core crate from declaring a
//! repository-root test target, so that spelling cannot be materialized —
//! an additional instance of the locator-materialization class first
//! recorded 2026-08-17 against AK-049. The proof therefore lives as an
//! integration test of the composition root with the target NAME the
//! plan's gate uses (`cargo nextest run --test p3_real_observation`;
//! `p1_reference_integrity.rs` precedent).
//!
//! The scenario (Task 8's REQUIRED `lying-builder` fixture): the child
//! writes an artifact whose content FAILS its own declared self-check
//! while its result document claims the check passed. P3 is the
//! divergence recorded as DATA: the observation bundle's command facet
//! carries the re-run self-check's real exit status 1 with the claims
//! facet still asserting the story — and the claims text reaches NONE of
//! the verification inputs.
//!
//! The self-check command is DERIVED from the result document's own
//! `self_checks` entry (split into program + args), not spelled here:
//! the fixture's declared check, re-run as an instrument, is exactly the
//! observation P3 describes.

use std::path::PathBuf;
use std::sync::Arc;

use gauntlet_adapter_agent_mock::{DeterministicClock, MockAgentRunner, load_scenario};
use gauntlet_adapter_evidence_fs::FsEvidenceStore;
use gauntlet_adapter_observe_command::{CommandObserver, CommandSpec};
use gauntlet_adapter_observe_filesystem::FilesystemObserver;
use gauntlet_app::observation::assembly::{ObservationAssembler, ObservationPlan};
use gauntlet_app::observation::bundle::ClaimsFacet;
use gauntlet_app::observation::digest::observation_digest;
use gauntlet_app::observation::input::VerificationInputs;
use gauntlet_domain::{
    Budget, DiagnosticAllowance, Digest, EpochSeconds, IterationNo, Limits, MicroUsd,
    PermissionSetRef, ResultContractPath, RoleId, RunId, WorkspaceRef,
};
use gauntlet_ports::agent_runner::{
    AgentRunner, ContextEnvelope, InvocationHandle, InvocationStatus, RoleInvocationSpec,
};
use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::evidence_store::EvidenceStore;
use gauntlet_ports::observer::{FacetKind, ObservationRequest, Observer};
use gauntlet_ports::observer_evidence::FacetEvidenceDoc;
use gauntlet_ports::role_result::{RolePayload, read_result_contract};

const ULID: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
const ENVELOPE_DIGEST: &str = "sha256:a1b2c3d4e5f6a7b8";
/// Fixed points far from any wall clock; named consts (the pipeline under
/// test must never read a clock beyond the injected one).
const BASE_INSTANT: u64 = 1_700_000_000;
const BOUND_AT: u64 = 1_700_000_090;
/// The workspace the lying builder "works" in, relative to the observers'
/// root (single segment; the artifact path itself comes from the result
/// document, never spelled here).
const WORKSPACE_KEY: &str = "ws-lying-builder";

/// Isolated scratch directory per test; removed on drop (the repo's
/// pid-tagged Scratch idiom).
struct Scratch {
    path: PathBuf,
}

impl Scratch {
    fn new(tag: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "gauntlet-p3-observation-{tag}-{}",
            std::process::id()
        ));
        std::fs::remove_dir_all(&path).ok();
        std::fs::create_dir_all(&path).expect("the p3 scratch parent is creatable");
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
        Limits::new(8, 2, 2, 3600, 1000, MicroUsd::new(500)).expect("proof limits usable"),
        20,
        DiagnosticAllowance::new(100, MicroUsd::new(50)),
    )
    .expect("proof budget usable");
    RoleInvocationSpec {
        invocation_id: gauntlet_domain::InvocationId::new(ULID).expect("valid invocation id"),
        run_id: RunId::new(ULID).expect("valid run id"),
        iteration_no: IterationNo::new(1).expect("1-based iteration"),
        role: RoleId::Builder,
        prompt: "run the assigned slice and write the typed result".to_owned(),
        context_envelope: ContextEnvelope::new(
            "the runtime-built allowlisted context".to_owned(),
            Digest::parse(ENVELOPE_DIGEST).expect("proof digest is well formed"),
        ),
        workspace: WorkspaceRef::new(WORKSPACE_KEY).expect("proof workspace ref"),
        permissions: PermissionSetRef::new("perm-readonly").expect("proof permission ref"),
        result_contract_path: ResultContractPath::new("proof-result-document")
            .expect("proof contract path"),
        deadline: Deadline::at_epoch(EpochSeconds::new(BOUND_AT)),
        model_hint: None,
        sandbox_policy: None,
        budget_slice: budget,
    }
}

/// Polls until a terminal status appears, bounded (the Task-7 lesson: a
/// playback that never terminates is a defect, not a pass).
fn drive_to_terminal(runner: &MockAgentRunner, handle: &InvocationHandle) -> InvocationStatus {
    for _ in 0..24 {
        let status = runner.poll(handle).expect("poll must not error");
        if status.is_terminal() {
            return status;
        }
    }
    panic!("no terminal state within the bounded probe run — a non-terminating script is a defect");
}

fn bound() -> Deadline {
    Deadline::at_epoch(EpochSeconds::new(BOUND_AT))
}

fn proof_env() -> Vec<(String, String)> {
    vec![(
        "GAUNTLET_PROOF_ENV".to_owned(),
        "p3-real-observation".to_owned(),
    )]
}

/// The proof: drive the lying builder to completion, observe the SAME
/// workspace with real observers, assemble the bundle, and assert the
/// divergence lives in the bundle with claims excluded from verification.
#[test]
fn the_lying_builders_claim_diverges_from_reality_inside_the_bundle() {
    let scratch = Scratch::new("main");
    let workspaces_root = scratch.path.join("workspaces");
    let workspace_dir = workspaces_root.join(WORKSPACE_KEY);
    std::fs::create_dir_all(&workspace_dir).expect("the lying builder's workspace is creatable");

    // 1-2. Drive the scripted child to completion over that workspace.
    let runner = MockAgentRunner::new(
        load_scenario("lying-builder").expect("the REQUIRED fixture parses"),
        workspace_dir.clone(),
    );
    let spec = dispatch_spec();
    let handle = runner.invoke(spec.clone()).expect("admission");
    match drive_to_terminal(&runner, &handle) {
        InvocationStatus::Completed { .. } => {}
        other => panic!("the lying builder Completes — the lie is in the claims, got {other:?}"),
    }
    let result = runner
        .collect(&handle, bound())
        .expect("a completed invocation is collectible");
    // The collected document also validates against the dispatch identity
    // through the raw-contract reader (§10.3 rule 6, composed).
    let raw = runner
        .written_document(&spec.result_contract_path)
        .expect("the File-delivery emulation kept the document");
    let rechecked = read_result_contract(&raw, &spec.invocation_id, spec.context_envelope.digest())
        .expect("the written document matches the dispatch identity");
    assert_eq!(rechecked, result);

    let RolePayload::BuilderResult(payload) = result.payload() else {
        panic!("the lying builder writes the builder payload")
    };
    let claims: Vec<String> = payload.claims().to_vec();
    assert!(!claims.is_empty(), "the claims carry the success story");
    let declared = &payload.self_checks()[0];
    assert_eq!(
        declared.exit_code(),
        0,
        "the document claims the check passed"
    );
    // The artifact the scenario wrote (path from the document, not spelled
    // here).
    let artifact_path = payload.changed_paths()[0].clone();
    let artifact = std::fs::read_to_string(workspace_dir.join(&artifact_path))
        .expect("the scripted effect wrote the artifact");
    assert!(
        artifact.contains("FAIL"),
        "reality fails the check: {artifact:?}"
    );

    // 3. Observe the SAME workspace: the filesystem manifest and the
    // fixture's own declared self-check, re-run as a command instrument.
    let store = Arc::new(
        FsEvidenceStore::open(&scratch.path.join("evidence")).expect("the evidence store opens"),
    );
    let clock = Arc::new(DeterministicClock::start_at(EpochSeconds::new(
        BASE_INSTANT,
    )));
    let env = proof_env();
    let fingerprint = gauntlet_adapter_observe_command::env_fingerprint(&env);

    let filesystem =
        FilesystemObserver::from_parts(workspaces_root.clone(), clock.clone(), store.clone(), &env);
    let mut tokens = declared.command().split_whitespace();
    let program = tokens
        .next()
        .expect("the declared check names a program")
        .to_owned();
    let args: Vec<String> = tokens.map(str::to_owned).collect();
    let command = CommandObserver::from_parts(
        CommandSpec::new(program, args),
        FacetKind::Test,
        workspaces_root.clone(),
        clock.clone(),
        store.clone(),
        &env,
    );

    // 4. Assemble with the collected claims as the claims-facet content.
    let observers: Vec<Box<dyn Observer>> = vec![Box::new(filesystem), Box::new(command)];
    let plan = ObservationPlan {
        request: ObservationRequest {
            run_id: spec.run_id.clone(),
            iteration_no: spec.iteration_no,
            workspace: spec.workspace.clone(),
        },
        observers,
        project_revision: "rev-proof-01".to_owned(),
        environment_fingerprint: fingerprint.clone(),
        claims_content: ClaimsFacet::render_content(&claims),
        clock: clock.clone(),
    };
    let assembled = ObservationAssembler::new(store.clone())
        .assemble(plan, bound())
        .expect("the composed assembly succeeds");
    let doc = &assembled.doc;

    // 5a. The command facet carries the FAILING reality (P3's core).
    let command_facet = doc
        .facets()
        .iter()
        .find(|facet| facet.kind() == "test")
        .expect("the command facet exists");
    assert_eq!(
        command_facet.exit_status(),
        Some(1),
        "the re-run self-check fails and the facet carries that"
    );
    assert_eq!(command_facet.command(), Some(declared.command()));
    let command_evidence = FacetEvidenceDoc::decode(&String::from_utf8_lossy(
        &store
            .get(
                &gauntlet_domain::EvidenceRef::new(command_facet.evidence_ref())
                    .expect("the facet's evidence ref is well formed"),
                bound(),
            )
            .expect("the evidence store reads")
            .expect("the command facet's evidence is present"),
    ))
    .expect("the command facet's evidence strict-decodes");
    assert_eq!(command_evidence.exit_status(), 1);
    assert_eq!(command_evidence.command(), declared.command());
    assert!(!command_evidence.env_fingerprint().is_empty());

    // 5b. The filesystem manifest lists the artifact.
    let filesystem_facet = doc
        .facets()
        .iter()
        .find(|facet| facet.kind() == "filesystem")
        .expect("the filesystem facet exists");
    let manifest = FacetEvidenceDoc::decode(&String::from_utf8_lossy(
        &store
            .get(
                &gauntlet_domain::EvidenceRef::new(filesystem_facet.evidence_ref())
                    .expect("the facet's evidence ref is well formed"),
                bound(),
            )
            .expect("the evidence store reads")
            .expect("the filesystem facet's evidence is present"),
    ))
    .expect("the filesystem facet's evidence strict-decodes");
    assert!(
        manifest
            .stdout()
            .contains(&format!("\"path\":\"{artifact_path}\"")),
        "the manifest lists the artifact: {}",
        manifest.stdout()
    );

    // 5c. The claims facet carries the document's claims verbatim.
    assert_eq!(
        doc.claims_facet().content(),
        ClaimsFacet::render_content(&claims)
    );
    assert!(doc.claims_facet().content().contains("self-check"));
    assert!(doc.claims_facet().excluded_from_verification());

    // No gaps: both observers ran.
    assert_eq!(doc.unavailable_evidence(), &[]);
    assert_eq!(doc.observation_failures(), &[]);

    // 5d. Verification inputs carry the FAIL-facing facet and NONE of the
    // claims text (I2, structurally).
    let inputs = VerificationInputs::of_bundle(doc, assembled.bundle_ref.clone());
    let inputs_text = inputs.to_json();
    assert!(inputs_text.contains("\"exit_status\":1"));
    assert!(inputs_text.contains("\"kind\":\"test\""));
    for claim in &claims {
        assert!(
            !inputs_text.contains(claim.as_str()),
            "the claim {claim:?} must never reach verification inputs"
        );
    }
    assert!(!inputs_text.contains("success story"));

    // 5e. Digest stability and sensitivity, at the composition root.
    assert_eq!(
        doc.observation_digest(),
        &observation_digest(doc),
        "the stored digest is the recomputed digest"
    );

    // Re-assembly with the same clock and environment is byte-identical.
    let filesystem_two =
        FilesystemObserver::from_parts(workspaces_root.clone(), clock.clone(), store.clone(), &env);
    let command_two = CommandObserver::from_parts(
        CommandSpec::new(
            declared
                .command()
                .split_whitespace()
                .next()
                .expect("program"),
            declared
                .command()
                .split_whitespace()
                .skip(1)
                .map(str::to_owned)
                .collect(),
        ),
        FacetKind::Test,
        workspaces_root,
        clock.clone(),
        store.clone(),
        &env,
    );
    let plan_two = ObservationPlan {
        request: ObservationRequest {
            run_id: spec.run_id.clone(),
            iteration_no: spec.iteration_no,
            workspace: spec.workspace.clone(),
        },
        observers: vec![Box::new(filesystem_two), Box::new(command_two)],
        project_revision: "rev-proof-01".to_owned(),
        environment_fingerprint: fingerprint,
        claims_content: ClaimsFacet::render_content(&claims),
        clock,
    };
    let assembled_two = ObservationAssembler::new(store.clone())
        .assemble(plan_two, bound())
        .expect("the second composed assembly succeeds");
    assert_eq!(
        doc.to_json(),
        assembled_two.doc.to_json(),
        "same clock, environment, and reality assemble byte-identical bundles"
    );
}

fn proof_clock() -> Arc<DeterministicClock> {
    Arc::new(DeterministicClock::start_at(EpochSeconds::new(
        BASE_INSTANT,
    )))
}

/// Claims sensitivity, kept as its own test so the main proof stays the
/// story of one divergence: swapping the claims content changes the
/// digest (forensic tamper-evidence) while the facet inputs stand still.
#[test]
fn a_claims_swap_moves_the_digest_but_not_the_facet_inputs() {
    let scratch = Scratch::new("claims-swap");
    let workspaces_root = scratch.path.join("workspaces");
    let workspace_dir = workspaces_root.join(WORKSPACE_KEY);
    std::fs::create_dir_all(&workspace_dir).expect("the workspace is creatable");
    std::fs::write(
        workspace_dir.join("probe.txt"),
        b"p3 claims-sensitivity probe\n",
    )
    .expect("the probe file is seedable");

    let runner = MockAgentRunner::new(
        load_scenario("lying-builder").expect("the REQUIRED fixture parses"),
        workspace_dir,
    );
    let spec = dispatch_spec();
    let handle = runner.invoke(spec.clone()).expect("admission");
    match drive_to_terminal(&runner, &handle) {
        InvocationStatus::Completed { .. } => {}
        other => panic!("the lying builder Completes, got {other:?}"),
    }
    let result = runner.collect(&handle, bound()).expect("collectible");
    let RolePayload::BuilderResult(payload) = result.payload() else {
        panic!("the lying builder writes the builder payload")
    };
    let claims: Vec<String> = payload.claims().to_vec();

    let store = Arc::new(
        FsEvidenceStore::open(&scratch.path.join("evidence")).expect("the evidence store opens"),
    );
    let fingerprint = gauntlet_adapter_observe_command::env_fingerprint(&proof_env());

    let assemble_with_claims = |claims_content: String| {
        let filesystem = FilesystemObserver::from_parts(
            workspaces_root.clone(),
            proof_clock(),
            store.clone(),
            &proof_env(),
        );
        let plan = ObservationPlan {
            request: ObservationRequest {
                run_id: spec.run_id.clone(),
                iteration_no: spec.iteration_no,
                workspace: spec.workspace.clone(),
            },
            observers: vec![Box::new(filesystem)],
            project_revision: "rev-proof-01".to_owned(),
            environment_fingerprint: fingerprint.clone(),
            claims_content,
            clock: proof_clock(),
        };
        ObservationAssembler::new(store.clone())
            .assemble(plan, bound())
            .expect("assembly succeeds")
    };

    let honest = assemble_with_claims(ClaimsFacet::render_content(&claims));
    let swapped = assemble_with_claims(ClaimsFacet::render_content(&[
        "a completely different forensic story".to_owned(),
    ]));
    assert_ne!(
        honest.doc.observation_digest(),
        swapped.doc.observation_digest(),
        "claims ARE covered by the digest"
    );
    assert_eq!(
        VerificationInputs::of_bundle(&honest.doc, honest.bundle_ref.clone()).facets(),
        VerificationInputs::of_bundle(&swapped.doc, swapped.bundle_ref.clone()).facets(),
        "the facet inputs stand still under a claims swap"
    );
}
