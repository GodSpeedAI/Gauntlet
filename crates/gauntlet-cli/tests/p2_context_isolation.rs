//! P2 proof (plan Task 10; spec §23.1 P2; AK-026 with AK-027's teeth
//! variant): context isolation asserted over the SERIALIZED envelope, at
//! the composition root — the one crate allowed to name adapters (spec
//! §4.1).
//!
//! Location rationale (recorded per the task brief): the plan spells the
//! proof `tests/proofs/p2_context_isolation`, but the architecture
//! boundary suite's cargo-target-path rule (exemption list exactly
//! `crates/gauntlet-arch-tests`) forbids a core crate from declaring a
//! repository-root test target, so that spelling cannot be materialized —
//! an additional instance of the locator-materialization class first
//! recorded 2026-08-17 against AK-049. The proof therefore lives as an
//! integration test of the composition root with the target NAME the
//! plan's gate uses (`cargo nextest run --test p2_context_isolation`;
//! `p1_reference_integrity.rs` precedent).
//!
//! The scenario (Task 8's REQUIRED `lying-builder` fixture): the child
//! writes an artifact whose content FAILS its own declared self-check
//! while its result document claims the check passed. The proof composes
//! the whole Task-9/10 chain over that reality: real observers → real
//! bundle → the §13.2 allowlist envelope (built with the
//! consumption-boundary digest recompute) → the deterministic diff
//! comparator, whose residual is derived from the INSTRUMENT failure —
//! and the serialized envelope contains NONE of the builder-side canaries
//! the builder's own world is full of.
//!
//! The `teeth` test (AK-027's declared locator suffix
//! `tests/proofs/p2_context_isolation::teeth`) is the designed-to-fail
//! pair: a deny-listed field injected into the envelope serialization
//! must FAIL the blindness proof.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use gauntlet_adapter_agent_mock::{DeterministicClock, MockAgentRunner, load_scenario};
use gauntlet_adapter_compare_diff::DiffComparator;
use gauntlet_adapter_compare_model::{
    ModelAdversarialSource, ModelComparator, ModelComparatorSource,
};
use gauntlet_adapter_evidence_fs::FsEvidenceStore;
use gauntlet_adapter_observe_command::{CommandObserver, CommandSpec};
use gauntlet_adapter_observe_filesystem::FilesystemObserver;
use gauntlet_adapter_state_sqlite::EmbeddedStateStore;
use gauntlet_app::compare::RecordingAgentRunner;
use gauntlet_app::compare::authority::{ComparisonAuthority, IssuedComparison};
use gauntlet_app::compare::contract::ComparisonContract;
use gauntlet_app::compare::{
    AdmissionError, EnvelopeError, ModelDispatchBudget, ReviewDeps, assert_blind,
};
use gauntlet_app::observation::assembly::{ObservationAssembler, ObservationPlan};
use gauntlet_app::observation::bundle::ClaimsFacet;
use gauntlet_app::observation::input::VerificationInputs;
use gauntlet_domain::{
    Budget, DiagnosticAllowance, Digest, EpochSeconds, EvidenceKind, IterationNo, Limits, MicroUsd,
    PermissionSetRef, ReferenceId, ReferenceVersion, Residual, ResidualId, ResultContractPath,
    ReviewMode, RoleId, Run, RunDefinition, RunId, RunnerIdentity, Severity, WorkspaceRef,
};
use gauntlet_ports::agent_runner::{
    AgentError, AgentRunner, ContextEnvelope, InvocationHandle, InvocationStatus, Isolation,
    ResultDelivery, RoleInvocationSpec, RunnerCapabilities,
};
use gauntlet_ports::comparator::{
    AdversarialComparatorSource, Comparator, ComparatorError, ComparatorSource, ComparisonEnvelope,
};
use gauntlet_ports::comparison_record::{
    ComparatorIdentity, ComparisonRecordDoc, RecordContent, RecordVerdict,
};
use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::evidence_store::{EvidenceStore, EvidenceStoreError};
use gauntlet_ports::id_generator::IdGenerator;
use gauntlet_ports::observer::{FacetKind, ObservationRequest, Observer};
use gauntlet_ports::state_store::{Checkpoint, RunEvent, StateStore, StateStoreError};

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

/// The canaries: builder-side material that MUST be present in the
/// builder's world and ABSENT from the envelope bytes (spec §13.2's
/// deny-list, both as field spellings and as content). Each canary rides
/// in real builder-side source material in this proof — the claims story
/// in the builder's result document (and the bundle's claims facet), and
/// the rest in the builder-side context record the runtime holds about
/// the dispatch (conversation, reasoning, identity, attempt count, prior
/// verdicts, operator chat) that the envelope builder is structurally
/// unable to accept.
const CANARY_CLAIMS: &str = "the artifact passes its own declared self-check, says the builder";
const CANARY_REASONING: &str =
    "builder reasoning canary: I retried the flaky path until it looked green";
const CANARY_IDENTITY: &str = "builder-identity-canary-session-42";
const CANARY_ATTEMPTS: &str = "attempt-count-canary: 7";
const CANARY_PRIOR_VERDICTS: &str = "prior-verdicts-canary: last time the critic said pass";
const CANARY_OPERATOR_CHAT: &str = "operator-chat-canary: just ship it, nobody will notice";
const CANARIES: [&str; 6] = [
    CANARY_CLAIMS,
    CANARY_REASONING,
    CANARY_IDENTITY,
    CANARY_ATTEMPTS,
    CANARY_PRIOR_VERDICTS,
    CANARY_OPERATOR_CHAT,
];

static SCRATCH_SEQ: AtomicU32 = AtomicU32::new(0);

/// The repository root, reached through the manifest dir for read-only
/// test inputs. Layout leaves come from the declared defaults table.
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("the cli crate sits two levels under the repository root")
        .to_path_buf()
}

fn frozen_answer_key_path() -> PathBuf {
    use gauntlet_app::config::defaults::{ANSWER_KEY_NAME, WAYFINDER_DIR_NAME};

    repo_root().join(WAYFINDER_DIR_NAME).join(ANSWER_KEY_NAME)
}

fn comparison_contract_path() -> PathBuf {
    use gauntlet_app::config::defaults::{COMPARISON_CONTRACT_NAME, RUNTIME_DIR_NAME};

    repo_root()
        .join(RUNTIME_DIR_NAME)
        .join(COMPARISON_CONTRACT_NAME)
}

/// Isolated scratch directory per test; removed on drop (the repo's
/// pid-tagged Scratch idiom).
struct Scratch {
    path: PathBuf,
}

impl Scratch {
    fn new(tag: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "gauntlet-p2-isolation-{tag}-{}-{}",
            std::process::id(),
            SCRATCH_SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::remove_dir_all(&path).ok();
        std::fs::create_dir_all(&path).expect("the p2 scratch parent is creatable");
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

// Meter figures for the permissive stub, referenced not written (the
// literal-scan idiom: no duration-shaped identifier shares a line with a
// value-position number).
const STUB_ROUND_HEADROOM: u32 = 10;
const STUB_PARALLEL_HEADROOM: u32 = 4;
const STUB_DEPTH_HEADROOM: u32 = 2;
const STUB_WALL_HEADROOM: u64 = 3600;
const STUB_TOKEN_HEADROOM: u64 = 1000;
const STUB_COST_HEADROOM: u64 = 500;
const STUB_CRITIC_ROUNDS: u32 = 1;
const STUB_CRITIC_WALL: u64 = 60;
const STUB_CRITIC_TOKENS: u64 = 100;
const STUB_CRITIC_COST: u64 = 10;

/// F4 (Task-12 correction): review dispatch consults the budget meter
/// before every model comparator admission (spec 10.4/17.4), so
/// `ReviewDeps` requires one. P2's surface is context isolation, not
/// budgeting, so its review fixtures stub the meter's permissive
/// extreme (test-only): full headroom, refuses nothing, books nothing.
struct PermissiveMeter;

impl gauntlet_ports::budget_meter::BudgetMeter for PermissiveMeter {
    fn reserve(
        &self,
        slice: gauntlet_ports::budget_meter::BudgetSlice,
    ) -> Result<
        gauntlet_ports::budget_meter::Reservation,
        gauntlet_ports::budget_meter::BudgetMeterError,
    > {
        Ok(self.mint(slice))
    }
    fn charge(
        &self,
        _: &gauntlet_ports::budget_meter::Reservation,
        _: gauntlet_domain::Payment,
    ) -> Result<(), gauntlet_ports::budget_meter::BudgetMeterError> {
        Ok(())
    }
    fn remaining(&self) -> gauntlet_ports::budget_meter::Remaining {
        gauntlet_ports::budget_meter::Remaining {
            rounds: STUB_ROUND_HEADROOM,
            parallel: STUB_PARALLEL_HEADROOM,
            depth: STUB_DEPTH_HEADROOM,
            wall_seconds: STUB_WALL_HEADROOM,
            tokens: STUB_TOKEN_HEADROOM,
            cost_usd: MicroUsd::new(STUB_COST_HEADROOM),
        }
    }
    fn would_breach_reserve(&self, _: gauntlet_ports::budget_meter::BudgetSlice) -> bool {
        false
    }
}

/// The one shared permissive-meter instance the review fixtures borrow.
static PERMISSIVE_METER: PermissiveMeter = PermissiveMeter;

/// The critic slice p2's review fixtures reserve: a non-zero forward
/// share (a zero slice would make the §5.3.7 consult vacuous).
fn critic_slice() -> gauntlet_ports::budget_meter::BudgetSlice {
    gauntlet_ports::budget_meter::BudgetSlice {
        rounds: STUB_CRITIC_ROUNDS,
        wall_seconds: STUB_CRITIC_WALL,
        tokens: STUB_CRITIC_TOKENS,
        cost_usd: MicroUsd::new(STUB_CRITIC_COST),
    }
}

fn proof_env() -> Vec<(String, String)> {
    vec![(
        "GAUNTLET_PROOF_ENV".to_owned(),
        "p2-context-isolation".to_owned(),
    )]
}

/// The builder-side context record the runtime holds about the dispatch:
/// everything §13.2 deny-lists, in one place, with every canary present.
/// This is the material a "helpful" envelope must never carry.
fn builder_side_context() -> String {
    format!(
        "{{\"builder_reasoning\":\"{CANARY_REASONING}\",\
         \"builder_identity\":\"{CANARY_IDENTITY}\",\
         \"attempt_count\":\"{CANARY_ATTEMPTS}\",\
         \"prior_verdicts\":\"{CANARY_PRIOR_VERDICTS}\",\
         \"operator_chat\":\"{CANARY_OPERATOR_CHAT}\"}}"
    )
}

/// The frozen reference the run compares against: the repository's real
/// answer key, read-only, digested through the production normalize path
/// (the same digest the ruler's own gate computes).
fn frozen_reference_digest() -> Digest {
    let raw = std::fs::read(frozen_answer_key_path()).expect("the frozen answer key is readable");
    gauntlet_app::reference::normalize::digest_of(&raw).expect("the frozen key digests")
}

/// Stores and pins the frozen answer key, returning the `ReferenceVersion` the
/// DURABLE RUN will carry. Nothing downstream accepts a reference version from
/// a caller: the governed chain reads it back out of the recorded run.
fn frozen_reference_version(store: &dyn EvidenceStore, run_id: &RunId) -> ReferenceVersion {
    let raw = std::fs::read(frozen_answer_key_path()).expect("the frozen answer key is readable");
    let normalized =
        gauntlet_app::reference::normalize::normalize_bytes(&raw).expect("the key normalizes");
    let digest = gauntlet_app::reference::normalize::digest_normalized(&normalized);
    let evidence = store
        .put(&normalized, EvidenceKind::File, bound())
        .expect("frozen evidence stores");
    store
        .pin(run_id, &evidence, bound())
        .expect("frozen evidence pins");
    ReferenceVersion::with_evidence(
        ReferenceId::new("gauntlet-reference").expect("reference id"),
        1,
        digest,
        evidence,
    )
}

/// The §13.2 open-residual allowlist input the composed proof uses. Shared so
/// every chain built in this file issues the SAME envelope, and a digest
/// mismatch in a test means a real crosswire rather than a fixture drift.
fn proof_residuals() -> Vec<Residual> {
    vec![Residual::new(
        ResidualId::new("res-000001").expect("valid residual id"),
        Severity::Minor,
        "a previously recorded, still-relevant difference".to_owned(),
        Some("functional behavior".to_owned()),
    )]
}

/// Everything the composed proof produces: the single governed dispatch
/// capability plus the durable substrate it was earned from.
struct Composed {
    issued: IssuedComparison,
    #[allow(dead_code)]
    result: gauntlet_ports::role_result::RoleResult,
    #[allow(dead_code)]
    assembled: gauntlet_app::observation::assembly::AssembledBundle,
    state: Arc<EmbeddedStateStore>,
    evidence: Arc<FsEvidenceStore>,
    /// The fs evidence store's root — the absent-object tooth removes a
    /// stored object through the adapter's documented content-addressed
    /// layout (`objects/<b0-2>/<b2-4>/<64-hex>`, refs `sha256:<hex>`).
    #[allow(dead_code)]
    evidence_root: std::path::PathBuf,
    #[allow(dead_code)]
    reference_version: ReferenceVersion,
    /// The self-check command string the BUILDER declared in its own
    /// `builder_result@v1` document — builder-authored text that the command
    /// instrument then executed verbatim.
    declared_command: String,
}

impl Composed {
    /// Removes one stored object through the fs adapter's DOCUMENTED layout,
    /// constructing the bytes-vanish state honestly (the object was real
    /// when it was bound; then it is gone).
    fn remove_stored_object(&self, evidence: &gauntlet_domain::EvidenceRef) {
        let hex = evidence
            .as_str()
            .strip_prefix("sha256:")
            .expect("fs store refs are sha256:<hex>");
        assert_eq!(hex.len(), 64, "a SHA-256 ref names 64 hex characters");
        let object = self
            .evidence_root
            .join("objects")
            .join(&hex[0..2])
            .join(&hex[2..4])
            .join(hex);
        std::fs::remove_file(&object)
            .expect("the stored object exists and is removed for the scenario");
    }
}

/// Drives the lying builder, observes the SAME workspace with the real
/// Task-9 observers, assembles the bundle, and builds the §13.2 envelope
/// through the application builder. Returns everything the assertions
/// need.
fn composed_envelope(scratch: &Scratch) -> Composed {
    let workspaces_root = scratch.path.join("workspaces");
    let workspace_dir = workspaces_root.join(WORKSPACE_KEY);
    std::fs::create_dir_all(&workspace_dir).expect("the lying builder's workspace is creatable");

    // 1-2. Drive the scripted child to completion over that workspace.
    // COR-D3 mechanical adaptation: the mock resolves effects under
    // <root>/<spec.workspace>/, so it is constructed with the CANONICAL
    // workspaces root — the effects land inside
    // workspaces_root/WORKSPACE_KEY, exactly the tree the observers read.
    let runner = MockAgentRunner::new(
        load_scenario("lying-builder").expect("the REQUIRED fixture parses"),
        workspaces_root.clone(),
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
    let gauntlet_ports::role_result::RolePayload::BuilderResult(payload) = result.payload() else {
        panic!("the lying builder writes the builder payload")
    };
    let claims: Vec<String> = payload.claims().to_vec();
    assert!(!claims.is_empty(), "the claims carry the success story");
    // The claims canary rides in the builder's own document (the story the
    // fixture's claims tell).
    assert!(
        claims.iter().any(|claim| claim.contains("self-check")),
        "the fixture's claims tell the self-check story"
    );
    let declared = &payload.self_checks()[0];
    assert_eq!(
        declared.exit_code(),
        0,
        "the document claims the check passed"
    );
    let declared_command = declared.command().to_owned();

    // 3. Observe the SAME workspace: the filesystem manifest and the
    // fixture's own declared self-check, re-run as a command instrument.
    let evidence_root = scratch.path.join("evidence");
    let store = Arc::new(FsEvidenceStore::open(&evidence_root).expect("the evidence store opens"));
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
        workspaces_root,
        clock.clone(),
        store.clone(),
        &env,
    );

    // 4. Assemble with the collected claims as the claims-facet content —
    // the claims CANARY included, so its absence from the envelope is a
    // real exclusion, not an artifact of never writing it down.
    let claims_content = format!("{}{}", ClaimsFacet::render_content(&claims), CANARY_CLAIMS);
    let observers: Vec<Box<dyn Observer>> = vec![Box::new(filesystem), Box::new(command)];
    let plan = ObservationPlan {
        request: ObservationRequest {
            run_id: spec.run_id.clone(),
            iteration_no: spec.iteration_no,
            workspace: spec.workspace.clone(),
        },
        observers,
        project_revision: "rev-proof-01".to_owned(),
        environment_fingerprint: fingerprint,
        claims_content,
        clock,
    };
    let assembled = ObservationAssembler::new(store.clone())
        .assemble(plan, bound())
        .expect("the composed assembly succeeds");

    // 5. The frozen comparison contract + the GOVERNED admission chain (D2).
    //    The run is made durable FIRST; the chain then reads its frozen
    //    reference back out of the store. No step here supplies a reference
    //    version, a reference view, or bundle CONTENT — only a bundle ref.
    let contract = ComparisonContract::load(&comparison_contract_path())
        .expect("the repository's frozen comparison contract loads");
    let reference_version = frozen_reference_version(store.as_ref(), &spec.run_id);
    let state = Arc::new(
        EmbeddedStateStore::open(&scratch.path.join("state.db")).expect("real state store opens"),
    );
    let durable_run = Run::new(RunDefinition {
        id: spec.run_id.clone(),
        created_at: EpochSeconds::new(BASE_INSTANT),
        reference_version: reference_version.clone(),
        intent: "p2 composed proof".to_owned(),
        config_fingerprint: Digest::new("sha256", &"ab".repeat(32)).expect("digest"),
        environment_fingerprint: Digest::new("sha256", &"cd".repeat(32)).expect("digest"),
        budget: spec.budget_slice.clone(),
        parent_run: None,
    })
    .expect("durable run");
    state
        .record_run(&durable_run, bound())
        .expect("the run is durable before any comparison authority exists");
    // The observation owner binds what it observed. Since Task 12's
    // capability gate this write is EARNED through the ports authority: the
    // bundle ref is the only argument, the §12.4 digest is re-derived from
    // the bytes the store serves, and the declared iteration is validated
    // against the run's durable anchor. This is the write that makes one
    // pinned bundle THIS run's observation at THIS iteration; the comparison
    // chain resolves it from here and takes no bundle argument.
    gauntlet_ports::producer_authority::ObservationBindingAuthority::new(
        state.clone(),
        store.clone(),
    )
    .bind(
        &spec.run_id,
        spec.iteration_no,
        &assembled.bundle_ref,
        bound(),
    )
    .expect("the observed bundle binds durably");
    let issued = ComparisonAuthority::new(state.clone(), store.clone())
        .for_run(&spec.run_id, spec.iteration_no, bound())
        .expect("the durable run yields comparison authority")
        .issue(&contract, Vec::new(), proof_residuals(), bound())
        .expect("the envelope is issued at the composition root");
    Composed {
        issued,
        result,
        assembled,
        state,
        evidence: store,
        evidence_root,
        reference_version,
        declared_command,
    }
}

/// The deterministic diff comparator as a review source. Review dispatch is
/// the only way to reach a comparator now, so the composition test supplies
/// one instead of holding a dispatch capability itself.
struct DiffSource;

impl ComparatorSource for DiffSource {
    fn across_dimensions(&self) -> Result<Box<dyn Comparator>, String> {
        Ok(Box::new(DiffComparator::across_dimensions()))
    }
    fn for_dimension(&self, dimension: &str) -> Result<Box<dyn Comparator>, String> {
        Ok(Box::new(DiffComparator::for_dimension(dimension)))
    }
}

/// A deterministic completed critic used only to exercise the production
/// adapter/application composition: it returns a strict role-result document
/// bound to the exact spec it received.
struct CompletedCritic {
    envelope: ComparisonEnvelope,
    cross_wire: bool,
    spec: std::cell::RefCell<Option<RoleInvocationSpec>>,
}

impl CompletedCritic {
    fn new(envelope: ComparisonEnvelope, cross_wire: bool) -> Self {
        Self {
            envelope,
            cross_wire,
            spec: std::cell::RefCell::new(None),
        }
    }
}

impl AgentRunner for CompletedCritic {
    fn invoke(&self, spec: RoleInvocationSpec) -> Result<InvocationHandle, AgentError> {
        *self.spec.borrow_mut() = Some(spec.clone());
        Ok(InvocationHandle::new(spec.invocation_id, None))
    }

    fn poll(&self, handle: &InvocationHandle) -> Result<InvocationStatus, AgentError> {
        let spec = self.spec.borrow();
        let spec = spec.as_ref().ok_or_else(|| AgentError::UnknownHandle {
            invocation_id: handle.invocation_id().clone(),
        })?;
        Ok(InvocationStatus::Completed {
            result_ref: spec.result_contract_path.clone(),
        })
    }

    fn collect(
        &self,
        handle: &InvocationHandle,
        _: Deadline,
    ) -> Result<gauntlet_ports::role_result::RoleResult, AgentError> {
        let spec = self.spec.borrow();
        let spec = spec.as_ref().ok_or_else(|| AgentError::UnknownHandle {
            invocation_id: handle.invocation_id().clone(),
        })?;
        let document = ComparisonRecordDoc::new(RecordContent {
            run_id: spec.run_id.clone(),
            iteration_no: spec.iteration_no,
            reference_digest: self.envelope.reference_digest().clone(),
            observation_digest: self.envelope.observation().observation_digest().clone(),
            envelope_digest: if self.cross_wire {
                Digest::new("sha256", "ff00").expect("valid test digest")
            } else {
                spec.context_envelope.digest().clone()
            },
            contract_version: "1".to_owned(),
            identity: ComparatorIdentity::new(
                "completed-critic".to_owned(),
                "fixed-model".to_owned(),
                String::new(),
            ),
            residuals: Vec::new(),
            hard_gate_results: Vec::new(),
            verdict: RecordVerdict::new(
                gauntlet_domain::Verdict::Pass,
                "scripted valid result".to_owned(),
            ),
        });
        let raw = format!(
            "{{\"schema\":\"role_result@v1\",\"invocation_id\":\"{}\",\"run_id\":\"{}\",\"iteration_no\":{},\"role\":\"critic\",\"context_envelope_digest\":\"{}\",\"state\":\"completed\",\"payload_schema\":\"comparison_record@v1\",\"payload\":{},\"payment\":{{\"input_tokens\":0,\"output_tokens\":0,\"tool_calls\":0,\"wall_seconds\":0,\"cost_usd\":0.0}},\"unavailable_evidence\":[],\"errors\":[]}}",
            spec.invocation_id,
            spec.run_id,
            spec.iteration_no.get(),
            spec.context_envelope.digest(),
            document.to_payload_json(),
        );
        gauntlet_ports::role_result::RoleResult::decode(&raw).map_err(|error| {
            AgentError::CollectionFailed {
                invocation_id: handle.invocation_id().clone(),
                detail: error.to_string(),
            }
        })
    }

    fn cancel(
        &self,
        _: &InvocationHandle,
        _: gauntlet_ports::CancelReason,
        _: Deadline,
    ) -> Result<(), AgentError> {
        Ok(())
    }

    fn capabilities(&self) -> RunnerCapabilities {
        RunnerCapabilities {
            supports_native_subagents: false,
            supports_streaming_events: false,
            supports_model_selection: false,
            isolation: Isolation::Process,
            result_delivery: ResultDelivery::ReturnValue,
            max_depth: 1,
        }
    }
}

/// Test-only state facade: admission is delegated to the real durable store,
/// but the final acceptance write fails after evidence has already been pinned.
/// It exposes the deliberately ordered, non-atomic cross-store boundary.
struct RejectingAcceptanceStore {
    inner: Arc<EmbeddedStateStore>,
    acceptance_attempts: AtomicU32,
}

impl StateStore for RejectingAcceptanceStore {
    fn append_event(
        &self,
        event: RunEvent,
        deadline: Deadline,
    ) -> Result<gauntlet_domain::EventLogOffset, StateStoreError> {
        self.inner.append_event(event, deadline)
    }
    fn load_run(&self, run_id: &RunId, deadline: Deadline) -> Result<Option<Run>, StateStoreError> {
        self.inner.load_run(run_id, deadline)
    }
    fn checkpoint(
        &self,
        checkpoint: Checkpoint,
        deadline: Deadline,
    ) -> Result<(), StateStoreError> {
        self.inner.checkpoint(checkpoint, deadline)
    }
    fn resume(
        &self,
        run_id: &RunId,
        deadline: Deadline,
    ) -> Result<Option<Checkpoint>, StateStoreError> {
        self.inner.resume(run_id, deadline)
    }
    fn export_jsonl(&self, run_id: &RunId, deadline: Deadline) -> Result<String, StateStoreError> {
        self.inner.export_jsonl(run_id, deadline)
    }
    fn record_role_invocation(
        &self,
        invocation: &gauntlet_domain::RoleInvocation,
        deadline: Deadline,
    ) -> Result<(), StateStoreError> {
        self.inner.record_role_invocation(invocation, deadline)
    }
    fn observation_binding(
        &self,
        run_id: &RunId,
        iteration_no: IterationNo,
        deadline: Deadline,
    ) -> Result<Option<gauntlet_ports::state_store::ObservationBinding>, StateStoreError> {
        self.inner
            .observation_binding(run_id, iteration_no, deadline)
    }
    fn bind_observation_bundle(
        &self,
        run_id: &RunId,
        iteration_no: IterationNo,
        binding: &gauntlet_ports::producer_authority::AdmittedObservationBinding,
        deadline: Deadline,
    ) -> Result<(), StateStoreError> {
        self.inner
            .bind_observation_bundle(run_id, iteration_no, binding, deadline)
    }
    fn accept_comparison(
        &self,
        _: &gauntlet_ports::comparator::AdmittedComparison,
        _: Deadline,
    ) -> Result<(), StateStoreError> {
        self.acceptance_attempts.fetch_add(1, Ordering::SeqCst);
        Err(StateStoreError::WriteFailed {
            detail: "scripted accepted-comparison write refusal".to_owned(),
        })
    }
}

/// Evidence facade which records that the ordered pin completed. A pinned
/// orphan is evidence-storage residue, never an accepted comparison: acceptance
/// returns an error and the state store writes no comparison row.
struct PinTrackingEvidence {
    inner: Arc<FsEvidenceStore>,
    pins: AtomicU32,
}

impl EvidenceStore for PinTrackingEvidence {
    fn put(
        &self,
        bytes: &[u8],
        kind: EvidenceKind,
        deadline: Deadline,
    ) -> Result<gauntlet_domain::EvidenceRef, EvidenceStoreError> {
        self.inner.put(bytes, kind, deadline)
    }
    fn get(
        &self,
        reference: &gauntlet_domain::EvidenceRef,
        deadline: Deadline,
    ) -> Result<Option<Vec<u8>>, EvidenceStoreError> {
        self.inner.get(reference, deadline)
    }
    fn pin(
        &self,
        run_id: &RunId,
        reference: &gauntlet_domain::EvidenceRef,
        deadline: Deadline,
    ) -> Result<(), EvidenceStoreError> {
        self.inner.pin(run_id, reference, deadline)?;
        self.pins.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

struct FixedIds;
impl IdGenerator for FixedIds {
    fn run_id(&self) -> RunId {
        RunId::new(ULID).expect("valid proof id")
    }
    fn invocation_id(&self) -> gauntlet_domain::InvocationId {
        gauntlet_domain::InvocationId::new(ULID).expect("valid proof id")
    }
}

/// The proof: the serialized envelope is structurally blind (no
/// builder-side canary, no deny-listed field) while facet-level
/// instrument reality IS present, the digest recompute refuses tampering,
/// and the diff comparator's residual derives from the INSTRUMENT failure
/// with all three digests coupled.
#[test]
fn the_envelope_is_structurally_blind_and_carries_instrument_reality() {
    let scratch = Scratch::new("main");
    let composed = composed_envelope(&scratch);
    let assembled = &composed.assembled;
    let bytes = composed.issued.canonical_bytes();

    // AK-026, part one: every canary is present in the builder-side
    // source material (the claims facet carries the claims canary; the
    // runtime's builder-side context record carries the rest)...
    let builder_world = format!(
        "{}{}",
        assembled.doc.claims_facet().content(),
        builder_side_context()
    );
    for canary in CANARIES {
        assert!(
            builder_world.contains(canary),
            "the canary must exist in the builder-side material first: {canary:?}"
        );
        // ...and ABSENT from the envelope bytes.
        assert!(
            !bytes.contains(canary),
            "the serialized envelope carries deny-listed content: {canary:?}"
        );
    }
    // The structural assertion itself: no deny-listed field spelling.
    assert_blind(bytes).expect("the honest envelope is structurally blind (P2)");

    // Facet-level reality IS present (the P3 comparison half, composed):
    // the failing exit status and its evidence ref ride in the bytes.
    assert!(
        bytes.contains("\"exit_status\":1"),
        "the failing instrument's exit status must be visible to a comparator"
    );
    let failing = assembled
        .doc
        .facets()
        .iter()
        .find(|facet| facet.kind() == "test")
        .expect("the command facet exists");
    assert_eq!(failing.exit_status(), Some(1));
    assert!(
        bytes.contains(failing.evidence_ref()),
        "the failing facet's evidence ref rides in the bytes"
    );

    // Builder-ORIGINATED text is not contraband. The exact executed command is
    // instrument provenance (AK-028): its string is the BUILDER's own declared
    // self-check, re-run by the command observer. It rides in the envelope, and
    // nothing rejects it for having come from a builder. What stays out is the
    // claims STORY — excluded structurally (Task 9), not by inspecting text.
    let executed = failing
        .command()
        .expect("the re-run instrument records the exact command it executed");
    assert!(
        bytes.contains(executed),
        "the exact executed command must reach the comparator even though a \
         builder authored its text: {executed:?}"
    );
    assert_eq!(
        executed, composed.declared_command,
        "and that text really is the builder's own declared self-check, so this \
         assertion is about provenance rather than a coincidence"
    );

    // Digest-validation tooth material: tampering the bundle after
    // digesting is REFUSED at the consumption boundary (B-3c).
    let contract = ComparisonContract::load(&comparison_contract_path())
        .expect("the frozen comparison contract loads");
    let mut tampered = assembled.doc.content();
    tampered.facets[0] = gauntlet_app::observation::bundle::FacetRecord::from_facet(
        &(gauntlet_ports::observer::ObservationFacet::new(
            FacetKind::Test,
            gauntlet_ports::observer::InstrumentIdentity::new(
                gauntlet_domain::InstrumentName::new("command").expect("valid instrument name"),
                "1.0.0".to_owned(),
            ),
            gauntlet_domain::EvidenceRef::new(failing.evidence_ref()).expect("valid evidence ref"),
        )
        .with_invocation(Some("re-run the declared self-check".to_owned()), Some(0))),
    );
    let tampered = gauntlet_app::observation::bundle::ObservationBundleDoc::new(tampered);
    // The governed chain takes a bundle REF, never bundle content, so the
    // tamper must be written into the evidence store to be presentable at all.
    // Its declared observation digest then no longer covers its contents, and
    // the consumption boundary refuses it before any dispatch (B-3c).
    let tampered_ref = composed
        .evidence
        .put(
            tampered.to_json().as_bytes(),
            EvidenceKind::Transcript,
            bound(),
        )
        .expect("the tampered bundle stores");
    // The chain takes NO bundle argument, so the tamper can only be presented
    // by becoming some run's durably bound observation. Bind it to a second
    // durable run — this run's own binding is write-once and already taken.
    let tampered_run = RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAX").expect("second run id");
    let second = Run::new(RunDefinition {
        id: tampered_run.clone(),
        created_at: EpochSeconds::new(BASE_INSTANT),
        reference_version: composed.reference_version.clone(),
        intent: "tampered-observation probe".to_owned(),
        config_fingerprint: Digest::new("sha256", &"ab".repeat(32)).expect("digest"),
        environment_fingerprint: Digest::new("sha256", &"cd".repeat(32)).expect("digest"),
        budget: dispatch_spec().budget_slice,
        parent_run: None,
    })
    .expect("the second run is valid");
    composed
        .state
        .record_run(&second, bound())
        .expect("the second run is durable");
    // Bound through the EARNED authority (Task 12): the ref is the argument,
    // the §12.4 digest is derived from the bytes the store serves, and the
    // tampered bundle's self-declared digest never enters the binding.
    gauntlet_ports::producer_authority::ObservationBindingAuthority::new(
        composed.state.clone(),
        composed.evidence.clone(),
    )
    .bind(
        &tampered_run,
        IterationNo::new(1).expect("iterations are 1-based"),
        &tampered_ref,
        bound(),
    )
    .expect("the tampered bundle is bound to the second run");
    let refusal = ComparisonAuthority::new(composed.state.clone(), composed.evidence.clone())
        .for_run(
            &tampered_run,
            IterationNo::new(1).expect("iterations are 1-based"),
            bound(),
        )
        .expect("the second durable run yields authority")
        .issue(&contract, Vec::new(), Vec::new(), bound());
    assert!(
        matches!(
            refusal,
            Err(AdmissionError::EnvelopeRefused(
                EnvelopeError::ObservationDigestMismatch { .. }
            ))
        ),
        "a tampered facet cannot flow into a comparison, even as a run's own \
         durably bound observation"
    );

    // Comparator consumption: the deterministic comparator runs INSIDE the
    // governed review — there is no accessor handing its dispatch capability
    // out — and its residual is derived from the INSTRUMENT failure, not from
    // claims (the claims story appears nowhere in the record).
    let envelope = composed.issued.envelope().clone();
    let probes = DiffSource;
    let deps = ReviewDeps {
        deterministic: &probes,
        model: &probes,
        adversarial: None,
        meter: &PERMISSIVE_METER,
        critic_slice: critic_slice(),
    };
    let accepted = composed
        .issued
        .review(
            &deps,
            ReviewMode::Adaptive,
            ModelDispatchBudget::new(10),
            bound(),
            EpochSeconds::new(BASE_INSTANT),
        )
        .expect("the governed review completes")
        .accept(bound())
        .expect("the review is accepted");
    let record = accepted.record().expect("an aggregate record exists");
    assert_eq!(record.verdict().status(), gauntlet_domain::Verdict::Fail);
    let residual = record
        .residuals()
        .iter()
        .find(|residual| residual.dimension() == "functional behavior")
        .expect("the instrument failure is a functional-behavior residual");
    assert_eq!(residual.severity(), Severity::Blocking);
    assert!(
        residual.observed().contains(failing.evidence_ref()),
        "the residual cites the instrument's evidence: {}",
        residual.observed()
    );
    assert!(
        residual.observed().contains("exited 1"),
        "the residual derives from the exit status: {}",
        residual.observed()
    );
    for canary in CANARIES {
        assert!(
            !residual.observed().contains(canary),
            "the residual must not quote builder-side material: {canary:?}"
        );
    }
    // All three digests couple: the record against the envelope, and the
    // envelope against the run's frozen reference.
    assert_eq!(record.reference_digest(), envelope.reference_digest());
    assert_eq!(
        record.observation_digest(),
        envelope.observation().observation_digest()
    );
    assert_eq!(record.envelope_digest(), envelope.envelope_digest());
    assert_eq!(
        record.reference_digest(),
        &frozen_reference_digest(),
        "the run's frozen reference digest is the one compared against"
    );
    assert_eq!(
        record.observation_digest(),
        assembled.doc.observation_digest()
    );
    // The verification inputs (the claims-free view the envelope's
    // observation side IS) see the same reality.
    let inputs = VerificationInputs::of_bundle(&assembled.doc, assembled.bundle_ref.clone());
    assert!(inputs.to_json().contains("\"exit_status\":1"));
}

/// The P2 composition test also proves the real deterministic adapter
/// refuses bytes changed after construction but still labeled with the old
/// dispatch digest. This is the adapter-side half of §13.2 coupling.
/// What this proves, stated exactly: from outside the ports crate a STALE
/// envelope cannot be built at all. Every `ComparisonEnvelope` field is
/// private and the digest is computed inside `assemble`, so there is no
/// sequence of public calls that produces one whose digest labels other
/// bytes — which is why this test asserts reachability, not a refusal.
///
/// The refusal branch of `verify_envelope_digest` is exercised where a stale
/// value CAN be built: `gauntlet_ports::comparator::canonical_tests::
/// an_envelope_whose_digest_does_not_label_its_bytes_is_refused`. An earlier
/// version of this test was named for a refusal it never reached.
#[test]
fn a_stale_envelope_digest_is_unrepresentable_from_outside_the_port() {
    let scratch = Scratch::new("stale-digest");
    let composed = composed_envelope(&scratch);
    let envelope = composed.issued.envelope();
    assert!(gauntlet_ports::comparator::verify_envelope_digest(envelope).is_ok());
    // The only mutation a caller could attempt — change `reference_view`,
    // recompute, re-admit — is a compile-time rejection at the port boundary.
}

/// AK-027's teeth: a deny-listed field injected into the envelope
/// serialization makes the blindness proof FAIL. This is the
/// designed-to-fail assertion pair — the proof function must reject the
/// poisoned envelope.
#[test]
fn durable_critic_invocation_and_accepted_document_survive_reopen_and_reject_crosswire() {
    let scratch = Scratch::new("durable-binding");
    let composed = composed_envelope(&scratch);
    let envelope = composed.issued.envelope().clone();
    let state_path = scratch.path.join("state.db");
    // The run was already made durable by the governed chain — the authority
    // could not have been opened otherwise.
    let state = composed.state.clone();
    let evidence = Arc::new(
        FsEvidenceStore::open(&scratch.path.join("critic-evidence"))
            .expect("real evidence store opens"),
    );
    #[allow(clippy::arc_with_non_send_sync)]
    let raw = Arc::new(CompletedCritic::new(envelope.clone(), false));
    #[allow(clippy::arc_with_non_send_sync)]
    let recording = Arc::new(RecordingAgentRunner::new(
        raw,
        state.clone(),
        evidence,
        RunnerIdentity::new("recording-critic-runner").expect("runner identity"),
        EpochSeconds::new(BASE_INSTANT),
    ));
    let budget = dispatch_spec().budget_slice;
    let comparator = ModelComparator::with_clock(
        recording.clone(),
        Arc::new(FixedIds),
        None,
        budget.clone(),
        Arc::new(DeterministicClock::start_at(EpochSeconds::new(
            BASE_INSTANT,
        ))),
    );
    // Application review depends only on semantic ports. The regular source
    // cannot occupy the adversarial field: that field requires the distinct
    // `AdversarialComparatorSource` capability, supplied here only by the
    // model adapter factory which calls `ModelComparator::adversarial()`.
    let normal_source = ModelComparatorSource::new(comparator.clone());
    let adversarial_source = ModelAdversarialSource::from_model(comparator.clone());
    fn requires_adversarial(_: &dyn AdversarialComparatorSource) {}
    requires_adversarial(&adversarial_source);
    let _regular_port: &dyn ComparatorSource = &normal_source;
    // There is intentionally no conversion from `ComparatorSource` to
    // `AdversarialComparatorSource`; a normal source or a string label cannot
    // fill `_review_deps.adversarial`.

    // Dispatch and durable acceptance both happen INSIDE the governed review.
    // The adapter no longer accepts its own output, and no accessor hands its
    // dispatch capability out, so this is the only route to a durable record.
    let deps = ReviewDeps {
        deterministic: &normal_source,
        model: &normal_source,
        adversarial: Some(&adversarial_source),
        meter: &PERMISSIVE_METER,
        critic_slice: critic_slice(),
    };
    let accepted = composed
        .issued
        .review(
            &deps,
            ReviewMode::Single,
            ModelDispatchBudget::new(4),
            bound(),
            EpochSeconds::new(BASE_INSTANT),
        )
        .expect("the governed review completes")
        .accept(bound())
        .expect("the review is accepted durably");
    let document = accepted
        .contributor_records()
        .first()
        .expect("the critic contributed a record");
    assert_eq!(document.envelope_digest(), envelope.envelope_digest());

    // A durable checkpoint carries the exact in-flight recovery locator; close
    // all original handles before opening a fresh adapter instance below.
    state
        .checkpoint(
            Checkpoint {
                run_id: envelope.run_id().clone(),
                reference_digest: envelope.reference_digest().clone(),
                iteration_no: envelope.iteration_no(),
                unit_states: Vec::new(),
                budget_spent: *budget.spent(),
                in_flight: vec![(
                    gauntlet_domain::InvocationId::new(ULID).expect("proof invocation"),
                    ResultContractPath::new(&format!("critic-comparison-{ULID}"))
                        .expect("proof path"),
                )],
                event_log_offset: gauntlet_domain::EventLogOffset::new(0),
            },
            bound(),
        )
        .expect("checkpoint persists");
    drop(comparator);
    drop(recording);
    drop(state);
    drop(composed.state);

    let reopened = EmbeddedStateStore::open(&state_path).expect("state reopens after process loss");
    let exported = reopened
        .export_jsonl(envelope.run_id(), bound())
        .expect("durable binding exports after reopen");
    assert!(exported.contains("\"record\":\"role_invocation\""));
    assert!(exported.contains("\"record\":\"comparison\""));
    assert!(exported.contains("context_envelope_digest"));
    assert!(exported.contains(&envelope.envelope_digest().to_string()));
    assert!(exported.contains("critic-comparison-01ARZ3NDEKTSV4RRFFQ69G5FAV"));

    // task26-dcritic-apparatus-repair B3/B7's discriminating tooth: an
    // accepted critic comparison must leave a durable InvocationTerminal
    // fact, not just an InvocationAdmitted one — otherwise the invocation
    // is durably admitted but invisible to every reader that reconstructs
    // "which invocations actually ran" from the lifecycle facts alone
    // (gauntlet_app::baseline::metrics::derive_loop_metrics's
    // review_dispatches among them). Independently confirmed via raw
    // durable-store inspection of a real `improve baseline --profiles D` run
    // before this fix: `invocation_admitted` events existed for both the
    // builder and the critic, but ONLY the builder ever received a
    // matching terminal event — the critic's `review_dispatches`
    // contribution silently read back 0 despite the critic genuinely
    // running and its comparison record being durably accepted above.
    let lifecycle = reopened
        .run_lifecycle_facts(envelope.run_id(), bound())
        .expect("the reopened store answers the lifecycle-facts read");
    assert!(
        lifecycle
            .invocation_terminals
            .iter()
            .any(|(_, state)| *state == gauntlet_domain::InvocationState::Completed),
        "an accepted critic comparison must leave a durable InvocationTerminal(Completed) \
         fact, exactly like the builder dispatch path already does — found: {:?}",
        lifecycle.invocation_terminals
    );
    let resumed = reopened
        .resume(envelope.run_id(), bound())
        .expect("resume reads")
        .expect("checkpoint exists");
    assert_eq!(
        resumed.in_flight[0].1.as_str(),
        "critic-comparison-01ARZ3NDEKTSV4RRFFQ69G5FAV"
    );

    // Tooth: the adapter detects a nested cross-wire before the recorder can
    // pin or persist an accepted comparison record.
    let rejected_path = scratch.path.join("crosswire.db");
    let rejected = Arc::new(EmbeddedStateStore::open(&rejected_path).expect("second state opens"));
    let rejected_evidence = Arc::new(
        FsEvidenceStore::open(&scratch.path.join("crosswire-evidence"))
            .expect("second evidence opens"),
    );
    #[allow(clippy::arc_with_non_send_sync)]
    let rejected_recording = Arc::new(RecordingAgentRunner::new(
        {
            #[allow(clippy::arc_with_non_send_sync)]
            let raw = Arc::new(CompletedCritic::new(envelope.clone(), true));
            raw
        },
        rejected.clone(),
        rejected_evidence,
        RunnerIdentity::new("recording-critic-runner").expect("runner identity"),
        EpochSeconds::new(BASE_INSTANT),
    ));
    let rejected_comparator = ModelComparator::with_clock(
        rejected_recording.clone(),
        Arc::new(FixedIds),
        None,
        budget,
        Arc::new(DeterministicClock::start_at(EpochSeconds::new(
            BASE_INSTANT,
        ))),
    );
    // The cross-wired critic is dispatched through its own governed chain over
    // this second durable store: the run is recorded, the observation bound,
    // and the SAME envelope issued — so the refusal below is the crosswire,
    // not a fixture difference.
    let durable = Run::new(RunDefinition {
        id: envelope.run_id().clone(),
        created_at: EpochSeconds::new(BASE_INSTANT),
        reference_version: composed.reference_version.clone(),
        intent: "p2 composed proof".to_owned(),
        config_fingerprint: Digest::new("sha256", &"ab".repeat(32)).expect("digest"),
        environment_fingerprint: Digest::new("sha256", &"cd".repeat(32)).expect("digest"),
        budget: dispatch_spec().budget_slice,
        parent_run: None,
    })
    .expect("durable run");
    rejected
        .record_run(&durable, bound())
        .expect("the run is durable in the second store");
    gauntlet_ports::producer_authority::ObservationBindingAuthority::new(
        rejected.clone(),
        composed.evidence.clone(),
    )
    .bind(
        envelope.run_id(),
        envelope.iteration_no(),
        &composed.assembled.bundle_ref,
        bound(),
    )
    .expect("the same observation binds in the second store");
    let contract = ComparisonContract::load(&comparison_contract_path())
        .expect("the frozen comparison contract loads");
    let rejected_issued = ComparisonAuthority::new(rejected.clone(), composed.evidence.clone())
        .for_run(envelope.run_id(), envelope.iteration_no(), bound())
        .expect("the second store yields authority")
        .issue(&contract, Vec::new(), proof_residuals(), bound())
        .expect("the same envelope is issued");
    assert_eq!(
        rejected_issued.envelope_digest(),
        envelope.envelope_digest(),
        "the second chain issues the identical envelope"
    );
    let rejected_source = ModelComparatorSource::new(rejected_comparator);
    let rejected_deps = ReviewDeps {
        deterministic: &rejected_source,
        model: &rejected_source,
        adversarial: None,
        meter: &PERMISSIVE_METER,
        critic_slice: critic_slice(),
    };
    assert!(matches!(
        rejected_issued.review(
            &rejected_deps,
            ReviewMode::Single,
            ModelDispatchBudget::new(4),
            bound(),
            EpochSeconds::new(BASE_INSTANT)
        ),
        Err(AdmissionError::ReviewRefused(_))
    ));
    let rejected_export = rejected
        .export_jsonl(envelope.run_id(), bound())
        .expect("rejected export");
    assert!(rejected_export.contains("\"record\":\"role_invocation\""));
    assert!(
        !rejected_export.contains("\"record\":\"comparison\""),
        "a cross-wired document is never accepted durably"
    );
}

#[test]
fn mandatory_acceptance_failure_is_non_success_and_never_writes_a_comparison() {
    let scratch = Scratch::new("acceptance-refusal");
    let composed = composed_envelope(&scratch);
    let envelope = composed.issued.envelope().clone();
    let inner = composed.state.clone();
    let state = Arc::new(RejectingAcceptanceStore {
        inner: inner.clone(),
        acceptance_attempts: AtomicU32::new(0),
    });
    // The chain reads the frozen reference and the bound bundle from the SAME
    // evidence store the observation owner wrote to; the facade only counts
    // the ordered pin.
    let evidence = Arc::new(PinTrackingEvidence {
        inner: composed.evidence.clone(),
        pins: AtomicU32::new(0),
    });
    #[allow(clippy::arc_with_non_send_sync)]
    let raw = Arc::new(CompletedCritic::new(envelope.clone(), false));
    #[allow(clippy::arc_with_non_send_sync)]
    let recording = Arc::new(RecordingAgentRunner::new(
        raw,
        state.clone(),
        evidence.clone(),
        RunnerIdentity::new("refusing-acceptance-runner").expect("runner"),
        EpochSeconds::new(BASE_INSTANT),
    ));
    let comparator = ModelComparator::with_clock(
        recording.clone(),
        Arc::new(FixedIds),
        None,
        dispatch_spec().budget_slice,
        Arc::new(DeterministicClock::start_at(EpochSeconds::new(
            BASE_INSTANT,
        ))),
    );
    // The review issues its capability from the REFUSING store, so acceptance
    // during fan-in is the step that fails. Uncertainty is not converted into
    // a success: the review itself is refused.
    let contract = ComparisonContract::load(&comparison_contract_path())
        .expect("the frozen comparison contract loads");
    let issued = ComparisonAuthority::new(state.clone(), evidence.clone())
        .for_run(envelope.run_id(), envelope.iteration_no(), bound())
        .expect("the durable run yields authority through the facade")
        .issue(&contract, Vec::new(), proof_residuals(), bound())
        .expect("the same envelope is issued");
    let source = ModelComparatorSource::new(comparator);
    let deps = ReviewDeps {
        deterministic: &source,
        model: &source,
        adversarial: None,
        meter: &PERMISSIVE_METER,
        critic_slice: critic_slice(),
    };
    assert!(matches!(
        issued.review(
            &deps,
            ReviewMode::Single,
            ModelDispatchBudget::new(4),
            bound(),
            EpochSeconds::new(BASE_INSTANT)
        ),
        Err(AdmissionError::ReviewRefused(_))
    ));
    assert_eq!(state.acceptance_attempts.load(Ordering::SeqCst), 1);
    assert_eq!(
        evidence.pins.load(Ordering::SeqCst),
        1,
        "evidence is pinned before state acceptance"
    );
    let exported = inner
        .export_jsonl(envelope.run_id(), bound())
        .expect("export");
    assert!(exported.contains("\"record\":\"role_invocation\""));
    assert!(
        !exported.contains("\"record\":\"comparison\""),
        "ordered evidence orphan is non-success: no accepted comparison was recorded"
    );
}

#[test]
fn teeth() {
    let scratch = Scratch::new("teeth");
    let composed = composed_envelope(&scratch);
    let bytes = composed.issued.canonical_bytes();
    assert_blind(bytes).expect("the honest envelope passes first");
    // The injection: a "helpful" builder_reasoning field carrying the
    // reasoning canary.
    let poisoned = bytes.replace(
        "\"reference_view\":",
        &format!("\"builder_reasoning\":\"{CANARY_REASONING}\",\"reference_view\":"),
    );
    assert!(
        poisoned.contains(CANARY_REASONING),
        "the canary must ride in the poisoned bytes for the tooth to bite"
    );
    let violation = assert_blind(&poisoned)
        .expect_err("the blindness proof must FAIL on the poisoned envelope");
    assert_eq!(violation.field, "builder_reasoning");
    // And every deny-listed field name, one at a time, fails the same way.
    for field in gauntlet_app::compare::DENY_LISTED_FIELD_NAMES {
        let poisoned = bytes.replace(
            "\"reference_view\":",
            &format!("\"{field}\":\"x\",\"reference_view\":"),
        );
        assert!(
            assert_blind(&poisoned).is_err(),
            "the proof must reject the injected field {field:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// Teeth for the governed-admission invariant (see
// `.agents/reports/task10/architecture-diagnosis.md`).
//
// The compile-time half of the closure is not expressible as a runtime
// assertion and is not claimed as one:
//
//   * `RunComparisonAuthority::open` is the only constructor of comparison
//     authority, and it performs `StateStore::load_run` AND resolves the
//     iteration's durable observation binding;
//   * `ComparisonEnvelope::assemble`, `ObservationView::new` and
//     `TrustedComparisonContext::from_frozen_projection` are crate-private in
//     the port; `TrustedComparisonContext::issue` is the single public minting
//     path, and it reads the bound bundle, projects the view from those bytes,
//     computes the digest and runs the P2 blindness assertion itself;
//   * `compare::{review, aggregate, envelope}` are crate-private and
//     `AcceptedComparison` has no constructor outside the admission service;
//   * `StateStore::accept_comparison` takes an `AdmittedComparison`, which only
//     `MandatoryComparisonAcceptance::accept` mints, and that in turn takes a
//     `ComparisonProduct`, which only a dispatch through a
//     `TrustedComparisonContext` mints — so a hand-built
//     `ComparisonRecordDoc` has nothing to travel on;
//   * the ungoverned `StateStore::record_comparison` append is gone, and no
//     accessor hands the dispatch capability out of `IssuedComparison`.
//
// Every one of those is enforced by the type system — the whole workspace only
// compiles because each former direct call site was migrated onto the governed
// chain. What follows are the runtime refusals that a local patch could not
// produce without the same structure.
// ---------------------------------------------------------------------------

/// Tooth: authority is EARNED from durable state. An empty state store yields
/// no authority, so no envelope, no dispatch, and no comparison — the chain has
/// no entry point that skips this step.
#[test]
fn comparison_authority_requires_a_durable_run() {
    let scratch = Scratch::new("authority-requires-durable-run");
    let state = Arc::new(
        EmbeddedStateStore::open(&scratch.path.join("empty-state.db")).expect("state opens"),
    );
    let evidence = Arc::new(
        FsEvidenceStore::open(&scratch.path.join("empty-evidence")).expect("evidence opens"),
    );
    let refusal = ComparisonAuthority::new(state, evidence).for_run(
        &RunId::new(ULID).expect("valid run id"),
        IterationNo::new(1).expect("iterations are 1-based"),
        bound(),
    );
    // The refusal must name the MISSING DURABLE RUN specifically. Asserting
    // only "some error" would also be satisfied by a downstream evidence
    // failure, which would let the run-existence check be deleted unnoticed.
    let detail = match refusal {
        Err(AdmissionError::AuthorityRefused(error)) => error.to_string(),
        Err(other) => panic!("an undurable run must refuse authority, got {other}"),
        Ok(_) => panic!("an undurable run must not yield comparison authority"),
    };
    assert!(
        detail.contains("is not durable"),
        "the refusal must be the absent durable run, not a later failure: {detail}"
    );
}

/// Tooth: the reference is not a caller input. The issued envelope carries the
/// digest recorded on the durable run — there is no parameter through which a
/// different one could be presented.
#[test]
fn the_issued_envelope_carries_the_durable_runs_own_frozen_digest() {
    let scratch = Scratch::new("reference-not-selectable");
    let composed = composed_envelope(&scratch);
    let durable = composed
        .state
        .load_run(&RunId::new(ULID).expect("valid run id"), bound())
        .expect("the run loads")
        .expect("the run is durable");
    assert_eq!(
        composed.issued.envelope().reference_digest(),
        durable.reference_version().digest(),
        "the envelope compares against the RUN's frozen reference, not a caller's"
    );
    assert_eq!(
        composed.issued.envelope().reference_digest(),
        &frozen_reference_digest()
    );
}

/// Tooth: a bundle belonging to another run cannot be smuggled in by ref. The
/// admission service reopens the bundle from evidence and refuses one whose own
/// declared identity is not the governed run/iteration.
#[test]
fn an_unrecorded_substitute_observation_cannot_enter_comparison() {
    let scratch = Scratch::new("substitute-bundle");
    let composed = composed_envelope(&scratch);
    let contract = ComparisonContract::load(&comparison_contract_path())
        .expect("the frozen comparison contract loads");
    let run_id = RunId::new(ULID).expect("valid run id");
    let iteration_no = IterationNo::new(1).expect("iterations are 1-based");

    // A SECOND observation bundle for the same run and iteration: not
    // malformed, not stale, not foreign — a fully valid document whose facet
    // simply says the self-check passed. Its digest is recomputed so the
    // document is internally consistent, and it is stored and pinned exactly
    // like the real one. The only thing it lacks is the durable record.
    let mut content = composed.assembled.doc.content();
    content.facets = vec![gauntlet_app::observation::bundle::FacetRecord::from_facet(
        &gauntlet_ports::observer::ObservationFacet::new(
            FacetKind::Test,
            gauntlet_ports::observer::InstrumentIdentity::new(
                gauntlet_domain::InstrumentName::new("command").expect("valid instrument name"),
                "1.0.0".to_owned(),
            ),
            gauntlet_domain::EvidenceRef::new("evd-substitute-clean-0001").expect("valid ref"),
        )
        .with_invocation(Some("re-run the declared self-check".to_owned()), Some(0)),
    )];
    let substitute = gauntlet_app::observation::bundle::ObservationBundleDoc::new(content.clone());
    content.observation_digest = gauntlet_app::observation::digest::observation_digest(&substitute);
    let substitute = gauntlet_app::observation::bundle::ObservationBundleDoc::new(content);
    assert_eq!(
        gauntlet_app::observation::digest::observation_digest(&substitute),
        *substitute.observation_digest(),
        "the substitute is a genuinely valid bundle, not a tampered one"
    );
    assert_ne!(
        substitute.observation_digest(),
        composed.assembled.doc.observation_digest(),
        "and it is a different observation from the recorded one"
    );
    let substitute_ref = composed
        .evidence
        .put(
            substitute.to_json().as_bytes(),
            EvidenceKind::Transcript,
            bound(),
        )
        .expect("the substitute stores");
    composed
        .evidence
        .pin(&run_id, &substitute_ref, bound())
        .expect("the substitute pins");

    // 1. It cannot become the binding: the durable binding is write-once.
    //    The re-bind is EARNED through the authority (the substitute's bytes
    //    are real and stored, so the mint succeeds) — and the STORE still
    //    refuses the substitution, surfacing verbatim through the authority.
    let rebind = gauntlet_ports::producer_authority::ObservationBindingAuthority::new(
        composed.state.clone(),
        composed.evidence.clone(),
    )
    .bind(&run_id, iteration_no, &substitute_ref, bound());
    let detail = match rebind {
        Err(gauntlet_ports::producer_authority::ProducerAuthorityError::State(
            StateStoreError::WriteFailed { detail },
        )) => detail,
        other => panic!("rebinding an observed iteration must be refused, got {other:?}"),
    };
    assert!(
        detail.contains(composed.assembled.bundle_ref.as_str()),
        "the refusal names the binding that already stands: {detail}"
    );

    // 2. And it cannot be presented around the binding, because `issue` takes
    //    NO observation argument at all: the chain resolves the bundle from
    //    durable run state. The re-issued envelope is the RECORDED observation,
    //    and the substitute appears nowhere in the bytes the critic sees.
    let issued = ComparisonAuthority::new(composed.state.clone(), composed.evidence.clone())
        .for_run(&run_id, iteration_no, bound())
        .expect("authority reopens for the same durable run")
        .issue(&contract, Vec::new(), proof_residuals(), bound())
        .expect("the recorded observation issues");
    assert_eq!(
        issued.envelope().observation().bundle_ref(),
        &composed.assembled.bundle_ref
    );
    assert_eq!(
        issued.envelope().observation().observation_digest(),
        composed.assembled.doc.observation_digest()
    );
    let bytes = issued.canonical_bytes();
    assert!(
        !bytes.contains(substitute_ref.as_str())
            && !bytes.contains(&substitute.observation_digest().to_string())
            && !bytes.contains("evd-substitute-clean-0001"),
        "the unrecorded substitute reaches no part of the comparison"
    );
}

/// An evidence store that serves DIFFERENT bytes at one ref. Content-addressed
/// stores do not behave this way; the point is that the comparison chain must
/// not have to trust that they don't.
struct SwappingEvidence {
    inner: Arc<FsEvidenceStore>,
    swapped: gauntlet_domain::EvidenceRef,
    bytes: Vec<u8>,
}

impl gauntlet_ports::evidence_store::EvidenceStore for SwappingEvidence {
    fn put(
        &self,
        bytes: &[u8],
        kind: EvidenceKind,
        deadline: Deadline,
    ) -> Result<gauntlet_domain::EvidenceRef, gauntlet_ports::evidence_store::EvidenceStoreError>
    {
        self.inner.put(bytes, kind, deadline)
    }

    fn get(
        &self,
        reference: &gauntlet_domain::EvidenceRef,
        deadline: Deadline,
    ) -> Result<Option<Vec<u8>>, gauntlet_ports::evidence_store::EvidenceStoreError> {
        if reference == &self.swapped {
            return Ok(Some(self.bytes.clone()));
        }
        self.inner.get(reference, deadline)
    }

    fn pin(
        &self,
        run_id: &RunId,
        reference: &gauntlet_domain::EvidenceRef,
        deadline: Deadline,
    ) -> Result<(), gauntlet_ports::evidence_store::EvidenceStoreError> {
        self.inner.pin(run_id, reference, deadline)
    }
}

/// Tooth: the durable binding records the observation's CONTENT, not just a
/// pointer to it. An evidence store that serves other bytes at the bound ref —
/// a hostile adapter, a corrupted store, a store that resolves refs loosely —
/// cannot change what the critic compares. Without the bound digest, the
/// substituted facets would ride into the envelope with the real run's
/// identity and the real frozen reference.
#[test]
fn bytes_that_are_not_the_bound_observation_cannot_be_served_into_a_comparison() {
    let scratch = Scratch::new("swapped-observation-bytes");
    let composed = composed_envelope(&scratch);
    let contract = ComparisonContract::load(&comparison_contract_path())
        .expect("the frozen comparison contract loads");
    let run_id = RunId::new(ULID).expect("valid run id");
    let iteration_no = IterationNo::new(1).expect("iterations are 1-based");

    // A fully valid, internally consistent bundle for the same run/iteration —
    // simply not the one the run recorded.
    let mut content = composed.assembled.doc.content();
    content.facets = vec![gauntlet_app::observation::bundle::FacetRecord::from_facet(
        &gauntlet_ports::observer::ObservationFacet::new(
            FacetKind::Test,
            gauntlet_ports::observer::InstrumentIdentity::new(
                gauntlet_domain::InstrumentName::new("command").expect("valid instrument name"),
                "1.0.0".to_owned(),
            ),
            gauntlet_domain::EvidenceRef::new("evd-swapped-clean-0001").expect("valid ref"),
        )
        .with_invocation(Some("re-run the declared self-check".to_owned()), Some(0)),
    )];
    let swapped = gauntlet_app::observation::bundle::ObservationBundleDoc::new(content.clone());
    content.observation_digest = gauntlet_app::observation::digest::observation_digest(&swapped);
    let swapped = gauntlet_app::observation::bundle::ObservationBundleDoc::new(content);

    let lying = Arc::new(SwappingEvidence {
        inner: composed.evidence.clone(),
        swapped: composed.assembled.bundle_ref.clone(),
        bytes: swapped.to_json().into_bytes(),
    });
    let refusal = ComparisonAuthority::new(composed.state.clone(), lying)
        .for_run(&run_id, iteration_no, bound())
        .expect("authority still opens: the durable binding is intact")
        .issue(&contract, Vec::new(), proof_residuals(), bound());
    let detail = match refusal {
        Err(error) => error.to_string(),
        Ok(_) => panic!("swapped observation bytes must be refused"),
    };
    assert!(
        detail.contains("durably recorded observation"),
        "the refusal names the durable record as the authority: {detail}"
    );
    assert!(
        detail.contains(&composed.assembled.doc.observation_digest().to_string()),
        "and names the observation the run actually recorded: {detail}"
    );
}

/// Tooth: an observation bundle's own declared `observation_digest` is
/// re-derived from its bytes, never believed. The reference side of the
/// envelope has always been verified this way; an unverified observation
/// digest would put an attacker-chosen value into the comparison record and
/// into durable state.
#[test]
fn an_observation_bundle_that_misdeclares_its_own_digest_is_refused() {
    let scratch = Scratch::new("misdeclared-observation-digest");
    let composed = composed_envelope(&scratch);
    let contract = ComparisonContract::load(&comparison_contract_path())
        .expect("the frozen comparison contract loads");
    let run_id = RunId::new(ULID).expect("valid run id");
    let iteration_no = IterationNo::new(1).expect("iterations are 1-based");

    // The recorded bundle's bytes with ONLY its self-declared digest replaced.
    let mut content = composed.assembled.doc.content();
    let lie = Digest::new("sha256", &"ee".repeat(32)).expect("digest");
    content.observation_digest = lie.clone();
    let misdeclared = gauntlet_app::observation::bundle::ObservationBundleDoc::new(content);
    let lying = Arc::new(SwappingEvidence {
        inner: composed.evidence.clone(),
        swapped: composed.assembled.bundle_ref.clone(),
        bytes: misdeclared.to_json().into_bytes(),
    });
    let refusal = ComparisonAuthority::new(composed.state.clone(), lying)
        .for_run(&run_id, iteration_no, bound())
        .expect("authority still opens")
        .issue(&contract, Vec::new(), proof_residuals(), bound());
    let detail = match refusal {
        Err(error) => error.to_string(),
        Ok(_) => panic!("a misdeclared observation digest must be refused"),
    };
    assert!(
        detail.contains(&lie.to_string()),
        "the refusal names the digest the document declared about itself: {detail}"
    );
}

/// Tooth: an iteration the run never observed has no comparison authority at
/// all — the chain refuses to open rather than compare against nothing (I8).
#[test]
fn an_iteration_with_no_durable_observation_binding_has_no_authority() {
    let scratch = Scratch::new("unbound-iteration");
    let composed = composed_envelope(&scratch);
    let refusal = ComparisonAuthority::new(composed.state.clone(), composed.evidence.clone())
        .for_run(
            &RunId::new(ULID).expect("valid run id"),
            IterationNo::new(2).expect("iterations are 1-based"),
            bound(),
        );
    let detail = match refusal {
        Err(AdmissionError::AuthorityRefused(ComparatorError::EnvelopeInvalid { detail })) => {
            detail
        }
        Err(other) => panic!(
            "an unobserved iteration must be refused as an authority \
                              failure, got {other}"
        ),
        Ok(_) => panic!("an unobserved iteration must not yield comparison authority"),
    };
    assert!(
        detail.contains("no durably bound observation bundle"),
        "the refusal names the missing durable binding: {detail}"
    );
}

/// Tooth: a durable binding whose bytes are absent is a typed refusal, never
/// an empty observation silently compared (I8).
#[test]
fn a_bound_observation_whose_bytes_are_absent_is_refused_never_treated_as_empty() {
    let scratch = Scratch::new("absent-bundle");
    let composed = composed_envelope(&scratch);
    let contract = ComparisonContract::load(&comparison_contract_path())
        .expect("the frozen comparison contract loads");
    let orphan_run = RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAY").expect("orphan run id");
    let durable = Run::new(RunDefinition {
        id: orphan_run.clone(),
        created_at: EpochSeconds::new(BASE_INSTANT),
        reference_version: composed.reference_version.clone(),
        intent: "absent-observation probe".to_owned(),
        config_fingerprint: Digest::new("sha256", &"ab".repeat(32)).expect("digest"),
        environment_fingerprint: Digest::new("sha256", &"cd".repeat(32)).expect("digest"),
        budget: dispatch_spec().budget_slice,
        parent_run: None,
    })
    .expect("durable run");
    composed
        .state
        .record_run(&durable, bound())
        .expect("the orphan run is durable");
    // The binding is EARNED over real bytes (Task 12's gate: a binding for
    // bytes that never existed is unrepresentable), and THEN the stored
    // object vanishes — the bytes-absent state constructed the way it
    // actually happens.
    let orphan_bundle = composed
        .evidence
        .put(
            br#"{"schema":"observation_bundle@v1","facets":[]}"#,
            EvidenceKind::Transcript,
            bound(),
        )
        .expect("the orphan run's bundle stores");
    gauntlet_ports::producer_authority::ObservationBindingAuthority::new(
        composed.state.clone(),
        composed.evidence.clone(),
    )
    .bind(
        &orphan_run,
        IterationNo::new(1).expect("iterations are 1-based"),
        &orphan_bundle,
        bound(),
    )
    .expect("the binding is recorded over real bytes");
    composed.remove_stored_object(&orphan_bundle);
    let refusal = ComparisonAuthority::new(composed.state.clone(), composed.evidence.clone())
        .for_run(
            &orphan_run,
            IterationNo::new(1).expect("iterations are 1-based"),
            bound(),
        )
        .expect("authority opens: the binding exists")
        .issue(&contract, Vec::new(), Vec::new(), bound());
    assert!(matches!(
        refusal,
        Err(AdmissionError::ObservationRefused { .. })
    ));
}

/// Tooth: frozen evidence that does not normalize to the durable run's pinned
/// digest yields no authority-bearing projection — so the run cannot be
/// compared against a reference that is not its own (I1).
#[test]
fn frozen_evidence_that_disagrees_with_the_durable_digest_yields_no_projection() {
    let scratch = Scratch::new("frozen-digest-disagrees");
    let composed = composed_envelope(&scratch);
    // A fresh, EMPTY evidence store alongside the same durable run: the pinned
    // frozen object is absent, so the projection cannot be established.
    let other_evidence = Arc::new(
        FsEvidenceStore::open(&scratch.path.join("other-evidence")).expect("evidence opens"),
    );
    let refusal = ComparisonAuthority::new(composed.state.clone(), other_evidence).for_run(
        &RunId::new(ULID).expect("valid run id"),
        IterationNo::new(1).expect("iterations are 1-based"),
        bound(),
    );
    assert!(matches!(refusal, Err(AdmissionError::AuthorityRefused(_))));
}
