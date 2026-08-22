//! P12 proof (plan Task 11; spec §14.3–§14.4, §16.1 conditions 1–2, §23.1
//! P12; frozen Unit AK-005): **missing evidence is INCONCLUSIVE at every
//! layer of the composed verification path — never PASS, never silence,
//! never an error escape hatch** — and a request that LIES about recency,
//! producer, or locator cannot change that, because every binding decision
//! reads durable state (frozen answer key + write-once production records +
//! the evidence store), never the request.
//!
//! Frozen locator note (the p1/p3 precedent, recorded): AK-005's
//! `evidence_required.locator` is `tests/proofs/p12_missing_is_not_passing`
//! (`.wayfinder/answer-key.md`). That repo-root spelling cannot be declared
//! as a cargo target of a CORE crate: the boundary suite's cargo-target-path
//! rule (`gauntlet-arch-tests` exemption list, exactly
//! `crates/gauntlet-arch-tests`) reports a `[[test]]` path outside the crate
//! directory, and the literals scan reports the up-up path literal itself
//! (demonstrated 2026-08-21 before this file landed: 4 boundary failures,
//! `scope-escape` + `hardcoded-relative-path`). The proof therefore
//! materializes as an integration test of the composition root whose cargo
//! test-target NAME is exactly `p12_missing_is_not_passing`, so the plan's
//! focused gate (`cargo nextest run --test p12_missing_is_not_passing`)
//! runs verbatim. Locator-materialization class first recorded 2026-08-17
//! against AK-049; same class as the p3 header note.
//!
//! Composition (spec §4.1 — the CLI is the composition root, the one crate
//! allowed to name adapters; every instrument and store below is REAL):
//!
//! - durable state: the REAL embedded-SQL state store on a temp-dir database
//!   file (the run, and the write-once evidence-production records whose
//!   ordering anchor the store mints server-side);
//! - evidence: the REAL content-addressed fs evidence store (put/get/pin);
//! - the frozen reference: the fixture answer key below, normalized,
//!   digested, and stored through the real store — the run's
//!   `ReferenceVersion` pins both, and the chain re-verifies the digest on
//!   open;
//! - instruments: the four REAL Task-11 verifier adapters plus the
//!   application's AgentRunner-backed model rung (a scripted runner in this
//!   test — model output is untrusted input, so the rung's strict
//!   `role_result@v1` / `verification_record@v1` parsing is on the composed
//!   path);
//! - the application layer's earned `verify::` chain (`VerificationAuthority
//!   -> GovernedVerification::verify_claim -> VerifiedClaim`) and the §16.1
//!   settlement precondition.
//!
//! Correction round 1 note (2026-08-21): the proof wiring composes through
//! the corrected chain — the rung set travels to `for_run` (the earned
//! authority, C1; this test IS the composition root, so it builds the
//! ladder through the application layer's audited `composition` feature),
//! the iteration anchor is validated against the durable checkpoint the
//! fixture writes (C5), the production records carry the content digest of
//! the bytes they ingested (C3), and the settlement precondition consults
//! the claim's own chain store (C4 — no store argument exists anymore).
//!
//! Correction round 2 note (2026-08-21): the governed open also receives
//! the fixture's WORKSPACE ROOT (F1 — the same root the real adapters are
//! bound with), the chain re-reads locator bytes under it before dispatch
//! (`digest(locator) == recorded == digest(store)`), and the proof's
//! fixtures keep the locator file, the stored object, and the recorded
//! digest ONE fact (`PASSING_REPORT`/`FAILING_REPORT` are exactly the
//! adapter-written bytes). The doctored-workspace attack — honest FAILING
//! store record plus a doctored PASSING file at the locator — is test 12,
//! a permanent refusal proof.
//!
//! AK-035's deterministic-first property is proved here too: when a model
//! rung and a deterministic rung could BOTH decide one claim, the
//! deterministic instrument's identity is the one recorded.

use std::cell::RefCell;
use std::path::PathBuf;
use std::sync::Arc;

use gauntlet_adapter_agent_mock::SeededIdGenerator;
use gauntlet_adapter_evidence_fs::FsEvidenceStore;
use gauntlet_adapter_state_sqlite::EmbeddedStateStore;
use gauntlet_adapter_verify_command_exit_code::CommandExitVerifier;
use gauntlet_adapter_verify_file_digest::FileDigestVerifier;
use gauntlet_adapter_verify_schema_check::SchemaCheckVerifier;
use gauntlet_adapter_verify_test_report::TestReportVerifier;
use gauntlet_app::reference::normalize::{digest_of, normalize_bytes};
use gauntlet_app::verify::ladder::MODEL_RUNG_NAME;
use gauntlet_app::verify::{
    Ladder, LadderRung, ModelRung, PreconditionFailure, RungKind, VerificationAuthority,
    content_digest, settlement_precondition,
};
use gauntlet_domain::{
    Budget, DiagnosticAllowance, EpochSeconds, EvidenceKind, EvidenceLocator, EvidenceRef,
    IterationNo, Limits, MicroUsd, Outcome, Producer, ReferenceId, ReferenceVersion, Run,
    RunDefinition, RunId, UnitId, VerificationRef,
};
use gauntlet_ports::agent_runner::{
    AgentRunner, InvocationHandle, InvocationStatus, RoleInvocationSpec,
};
use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::evidence_store::EvidenceStore;
use gauntlet_ports::observer::InstrumentIdentity;
use gauntlet_ports::role_result::RoleResult;
use gauntlet_ports::state_store::{Checkpoint, EvidenceProduction, StateStore};
use gauntlet_ports::verification_record::{
    BindingClass, RequirementDoc, VerificationContent, VerificationRecordDoc, VerifyingInstrument,
};
use gauntlet_ports::verifier::{VerificationRequest, Verifier, VerifierError};

// ---------------------------------------------------------------------------
// Fixture constants
// ---------------------------------------------------------------------------

/// Fixed points far from any wall clock (the p3 proof convention): the
/// pipeline under test must never read a clock beyond the injected one.
const BASE_INSTANT: u64 = 1_700_000_000;
const BOUND_AT: u64 = BASE_INSTANT + 90;
/// The durable fixture run (a proven-valid ULID; unique per test only by the
/// store file, which lives under the per-test scratch tree).
const RUN_ULID: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
const FIXTURE_REFERENCE: &str = "p12-proof-reference";

/// The bytes of the fixture's PASSING report — EXACTLY what the real
/// adapter's own writer produces for `write_report(7, 0,
/// "the-p12-fixture-suite")` (correction round 2, F1): the locator file,
/// the stored object, and the recorded content digest are ONE fact now,
/// and these constants keep every `produce` call on the honest side of
/// that identity. Drift is caught by the proof itself — any divergence
/// between these bytes and the adapter-written artifact is the F1
/// three-way mismatch, refused.
const PASSING_REPORT: &[u8] = b"{\"tests\":7,\"failures\":0,\"command\":\"the-p12-fixture-suite\"}";
/// The honest FAILING report: tests RAN and some failed (present-and-wrong
/// evidence is a decision, not an absence).
const FAILING_REPORT: &[u8] = b"{\"tests\":1,\"failures\":1,\"command\":\"the-p12-fixture-suite\"}";

/// AK-9101 — the focused-test unit every binding scenario centres on.
const REPORT_UNIT: &str = "AK-9101";
const REPORT_LOCATOR: &str = "p12-report.json";
/// AK-9102 — a human-producer requirement instrument evidence can never
/// satisfy (§14.3 condition 2).
const SIGNOFF_UNIT: &str = "AK-9102";
const SIGNOFF_LOCATOR: &str = "p12-signoff.json";
/// AK-9103 — its requirement's locator; evidence produced ELSEWHERE must not
/// satisfy it (§14.3 condition 4, the recorded locator mapping).
const REQUIRED_UNIT: &str = "AK-9103";
const REQUIRED_LOCATOR: &str = "p12-required.json";
const ELSEWHERE_LOCATOR: &str = "p12-elsewhere.json";
/// AK-9104 — screenshot evidence: no deterministic rung declares it, so only
/// the model rung can decide (§14.2's rung of last resort).
const SURFACE_UNIT: &str = "AK-9104";
const SURFACE_LOCATOR: &str = "p12-surface.bin";
const SURFACE_CLAIM: &str = "the rendered surface matches the frozen design intent";

/// The frozen answer key this proof freezes through the REAL store. Single
/// locators by design: the literals scan polices path-shaped literals in
/// this crate.
const ANSWER_KEY: &str = "## AK-9101\nclaim: the focused proof suite passes and its report is \
     bound to this run\nverify_by: test\nblast_radius: local\nauthority: critic\n\
     evidence_required: {kind: test_report, locator: p12-report.json, producer: instrument}\n\n\
     ## AK-9102\nclaim: the operator signed the release note\nverify_by: human_signoff\n\
     blast_radius: local\nauthority: human\nevidence_required: {kind: test_report, locator: \
     p12-signoff.json, producer: human}\n\n## AK-9103\nclaim: the required artifact lives at the \
     frozen locator\nverify_by: test\nblast_radius: local\nauthority: critic\nevidence_required: \
     {kind: test_report, locator: p12-required.json, producer: instrument}\n\n## AK-9104\nclaim: \
     the rendered surface matches the frozen design intent\nverify_by: visual_diff\nblast_radius: \
     local\nauthority: critic\nevidence_required: {kind: screenshot, locator: p12-surface.bin, \
     producer: instrument}\n";

// ---------------------------------------------------------------------------
// Scratch and fixture
// ---------------------------------------------------------------------------

/// Isolated per-test scratch tree under the system temp dir; removed on
/// drop (the repo's pid-tagged Scratch idiom).
struct Scratch {
    path: PathBuf,
}

impl Scratch {
    fn new(tag: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("gauntlet-p12-proof-{tag}-{}", std::process::id()));
        std::fs::remove_dir_all(&path).ok();
        std::fs::create_dir_all(&path).expect("the p12 scratch parent is creatable");
        Self { path }
    }

    fn child(&self, name: &str) -> PathBuf {
        let path = self.path.join(name);
        std::fs::create_dir_all(&path).expect("the p12 scratch child is creatable");
        path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.path).ok();
    }
}

fn bound() -> Deadline {
    Deadline::at_epoch(EpochSeconds::new(BOUND_AT))
}

fn budget() -> Budget {
    Budget::new(
        Limits::new(8, 2, 2, 3600, 1000, MicroUsd::new(500)).expect("proof limits usable"),
        20,
        DiagnosticAllowance::new(100, MicroUsd::new(50)),
    )
    .expect("proof budget usable")
}

// The seeded generator's interior state is deliberately single-threaded
// (the ports' object traits carry no Send + Sync supertrait bound) — the
// same recorded allow the ladder's own rung holders carry.
#[allow(clippy::arc_with_non_send_sync)]
fn seeded_ids() -> Arc<SeededIdGenerator> {
    Arc::new(SeededIdGenerator::new("p12-proof-identity").expect("a non-empty seed"))
}

fn locator(name: &str) -> EvidenceLocator {
    EvidenceLocator::new(name).expect("the fixture locators are single-segment and valid")
}

fn reference(name: &str) -> EvidenceRef {
    EvidenceRef::new(name).expect("the fixture refs are valid")
}

fn run_id() -> RunId {
    RunId::new(RUN_ULID).expect("the fixture run id is a valid ULID")
}

/// The composed substrate: the REAL embedded state store on a temp-dir
/// database file, the REAL fs evidence store, and the frozen fixture
/// reference recorded durably as the run's own `ReferenceVersion`.
struct Fixture {
    _scratch: Scratch,
    state: Arc<EmbeddedStateStore>,
    evidence: Arc<FsEvidenceStore>,
    workspace: PathBuf,
    reference_digest: gauntlet_domain::Digest,
}

impl Fixture {
    fn new(tag: &str) -> Self {
        let scratch = Scratch::new(tag);
        let workspace = scratch.child("workspace");
        let evidence = Arc::new(
            FsEvidenceStore::open(&scratch.child("evidence"))
                .expect("the real fs evidence store opens"),
        );
        let state = Arc::new(
            EmbeddedStateStore::open(&scratch.child("state").join("p12-state.db"))
                .expect("the real embedded state store opens"),
        );
        // Freeze the reference through the real store: normalized bytes in,
        // their content address back, the digest pinned in the version.
        let normalized = normalize_bytes(ANSWER_KEY.as_bytes())
            .expect("the fixture key is UTF-8 and normalizes");
        let frozen = evidence
            .put(&normalized, EvidenceKind::File, bound())
            .expect("the frozen reference is stored");
        let version = ReferenceVersion::with_evidence(
            ReferenceId::new(FIXTURE_REFERENCE).expect("the fixture reference id is valid"),
            1,
            digest_of(ANSWER_KEY.as_bytes()).expect("the fixture key digests"),
            frozen,
        );
        let run = Run::new(RunDefinition {
            id: run_id(),
            created_at: EpochSeconds::new(BASE_INSTANT),
            reference_version: version,
            intent: "p12 proof fixture run".to_owned(),
            config_fingerprint: gauntlet_domain::Digest::new("sha256", &"aa".repeat(32))
                .expect("a hex digest is valid"),
            environment_fingerprint: gauntlet_domain::Digest::new("sha256", &"bb".repeat(32))
                .expect("a hex digest is valid"),
            budget: budget(),
            parent_run: None,
        })
        .expect("the fixture run is a valid run");
        state
            .record_run(&run, bound())
            .expect("the fixture run is durable");
        let reference_digest = run.reference_version().digest().clone();
        Self {
            _scratch: scratch,
            state,
            evidence,
            workspace,
            reference_digest,
        }
    }

    fn run(&self) -> RunId {
        run_id()
    }

    /// The producer-side write the way the evidence owner performs it: the
    /// bytes land in the store, the durable production record names ref,
    /// CONTENT DIGEST (C3 — the digest of the bytes ingested at the
    /// locator, computed through the application layer's own rule),
    /// producer, locator, and iteration — and the ordering anchor is MINTED
    /// by the store, never supplied here.
    fn produce(
        &self,
        producer: &InstrumentIdentity,
        at: &str,
        iteration: u32,
        payload: &[u8],
        kind: EvidenceKind,
    ) -> EvidenceRef {
        let evidence = self
            .evidence
            .put(payload, kind, bound())
            .expect("the production bytes are stored");
        let production = EvidenceProduction::new(
            evidence.clone(),
            content_digest(payload),
            producer.clone(),
            locator(at),
            IterationNo::new(iteration).expect("iterations are 1-based"),
            EpochSeconds::new(BASE_INSTANT + u64::from(iteration) * 100),
        );
        let binding = self
            .state
            .record_evidence_production(&self.run(), &production, bound())
            .expect("the write-once production record lands");
        assert!(
            binding.event_log_offset().get() > 0,
            "the ordering anchor is minted server-side"
        );
        evidence
    }

    /// Opens the governed verification at one iteration over one rung set
    /// (C1: the ladder is earned authority injected HERE, at the open). The
    /// fixture first writes the run's durable CHECKPOINT at that iteration
    /// the way the loop's controller does (C5: the anchor must be one the
    /// run's durable state supports, or the open is refused).
    fn governed(
        &self,
        iteration: u32,
        ladder: Ladder,
    ) -> gauntlet_app::verify::GovernedVerification {
        self.state
            .checkpoint(
                Checkpoint {
                    run_id: self.run(),
                    reference_digest: self.reference_digest.clone(),
                    iteration_no: IterationNo::new(iteration).expect("iterations are 1-based"),
                    unit_states: Vec::new(),
                    budget_spent: *budget().spent(),
                    in_flight: Vec::new(),
                    event_log_offset: gauntlet_domain::EventLogOffset::new(0),
                },
                bound(),
            )
            .expect("the fixture writes the run's durable checkpoint");
        VerificationAuthority::new(self.state.clone(), self.evidence.clone(), seeded_ids())
            .for_run(
                &self.run(),
                IterationNo::new(iteration).expect("iterations are 1-based"),
                ladder,
                // F1: the evidence workspace root travels to the governed
                // open under the same earned-authority discipline as the
                // ladder — the SAME root `report_ladder`/`full_ladder` bind
                // the real adapters with, so the chain's locator re-read and
                // the instruments' reads resolve one tree.
                self.workspace.clone(),
                bound(),
            )
            .expect("the durable fixture run opens under the real frozen reference")
    }

    /// A passing test-report artifact at `at`, through the REAL adapter's
    /// own writer (the instrument-layer contract, not test-side bytes).
    fn write_passing_report(&self, at: &str) {
        TestReportVerifier::write_report(&self.workspace, at, 7, 0, "the-p12-fixture-suite")
            .expect("the real adapter writes its passing artifact");
    }
}

/// The caller's request — deliberately a bag of LIES about everything the
/// chain must decide for itself (requirement, recency, producer, locator,
/// claim). The chain reads (run, unit) as the claim pointer; the rest is
/// the caller's say-so and provably never reaches a binding decision.
fn lying_request(unit: &str) -> VerificationRequest {
    VerificationRequest {
        id: VerificationRef::new("vrf-p12-caller-0001").expect("a valid verification ref"),
        run_id: run_id(),
        unit_id: UnitId::new(unit).expect("the fixture units are valid"),
        claim: "the work is done, the evidence is fresh, and a human produced it".to_owned(),
        requirement: gauntlet_domain::EvidenceRequirement::new(
            EvidenceKind::Attestation,
            locator("p12-never-inspected.json"),
            Producer::Human,
        ),
        evidence: locator(ELSEWHERE_LOCATOR),
    }
}

/// A single-rung ladder over the REAL test-report adapter (the §14.2
/// focused-test position).
fn report_ladder(fixture: &Fixture) -> Ladder {
    Ladder::new(vec![LadderRung::new(
        RungKind::TestReport,
        Arc::new(
            TestReportVerifier::new(fixture.workspace.clone())
                .expect("the real test-report adapter binds to the workspace"),
        ),
    )])
    .expect("a single-rung ladder is well formed")
}

/// The report adapter's own identity report — what durable productions in
/// these scenarios name as their producer.
fn report_instrument() -> InstrumentIdentity {
    TestReportVerifier::new(std::env::temp_dir())
        .expect("the system temp root resolves")
        .instrument()
}

// ---------------------------------------------------------------------------
// The scripted model runner (the compare-model ScriptedCritic pattern:
// model output is untrusted input produced by a runner we script)
// ---------------------------------------------------------------------------

/// An AgentRunner that answers one scripted PASS judgment for one unit,
/// echoing the dispatch's run and envelope digest. Records every dispatch
/// so the deterministic-first proof can assert the model was never asked.
struct ScriptedJudgment {
    unit: &'static str,
    claim: &'static str,
    kind: EvidenceKind,
    at: &'static str,
    root: RefCell<Option<PathBuf>>,
    specs: RefCell<Vec<RoleInvocationSpec>>,
}

impl ScriptedJudgment {
    fn new(unit: &'static str, claim: &'static str, kind: EvidenceKind, at: &'static str) -> Self {
        Self {
            unit,
            claim,
            kind,
            at,
            root: RefCell::new(None),
            specs: RefCell::new(Vec::new()),
        }
    }

    fn dispatches(&self) -> usize {
        self.specs.borrow().len()
    }

    fn document_for(&self, spec: &RoleInvocationSpec) -> String {
        let payload = VerificationRecordDoc::new(VerificationContent {
            run_id: spec.run_id.clone(),
            // The judgment echoes the GOVERNED iteration of its dispatch
            // (C6.3: a result at another iteration is refused by the rung).
            iteration_no: spec.iteration_no,
            unit_id: UnitId::new(self.unit).expect("the fixture unit is valid"),
            claim: self.claim.to_owned(),
            requirement: RequirementDoc::new(self.kind, locator(self.at), Producer::Instrument),
            instrument: VerifyingInstrument::new(MODEL_RUNG_NAME.to_owned(), "1".to_owned(), false),
            evidence: reference(self.at),
            produced_at: EpochSeconds::new(BASE_INSTANT + 9),
            outcome: Outcome::Pass,
            class: None,
            detail: "the scripted judgment for the p12 proof".to_owned(),
        })
        .expect("the scripted payload is a well formed record");
        format!(
            "{{\"schema\":\"role_result@v1\",\"invocation_id\":\"{}\",\"run_id\":\"{}\",\"\
             iteration_no\":{},\"role\":\"verifier\",\"context_envelope_digest\":\"{}\",\"state\":\"\
             completed\",\"payload_schema\":\"verification_record@v1\",\"payload\":{},\"payment\":{{\
             \"input_tokens\":1,\"output_tokens\":1,\"tool_calls\":0,\"wall_seconds\":0,\
             \"cost_usd\":0.0}},\"unavailable_evidence\":[],\"errors\":[]}}",
            spec.invocation_id.as_str(),
            spec.run_id.as_str(),
            u64::from(spec.iteration_no.get()),
            spec.context_envelope.digest(),
            payload.to_payload_json(),
        )
    }
}

impl AgentRunner for ScriptedJudgment {
    fn invoke(
        &self,
        spec: RoleInvocationSpec,
    ) -> Result<InvocationHandle, gauntlet_ports::agent_runner::AgentError> {
        self.specs.borrow_mut().push(spec.clone());
        let root = std::env::temp_dir().join(format!(
            "gauntlet-p12-judgment-{}-{}",
            std::process::id(),
            self.unit
        ));
        std::fs::remove_dir_all(&root).ok();
        std::fs::create_dir_all(&root).expect("the scripted judgment root is creatable");
        std::fs::write(
            root.join(spec.result_contract_path.as_str()),
            self.document_for(&spec),
        )
        .expect("the scripted judgment writes its contract");
        *self.root.borrow_mut() = Some(root);
        Ok(InvocationHandle::new(spec.invocation_id, None))
    }

    fn poll(
        &self,
        _handle: &InvocationHandle,
    ) -> Result<InvocationStatus, gauntlet_ports::agent_runner::AgentError> {
        let spec = self
            .specs
            .borrow()
            .last()
            .cloned()
            .expect("invoke precedes poll");
        Ok(InvocationStatus::Completed {
            result_ref: spec.result_contract_path,
        })
    }

    fn collect(
        &self,
        handle: &InvocationHandle,
        _: Deadline,
    ) -> Result<RoleResult, gauntlet_ports::agent_runner::AgentError> {
        let root = self.root.borrow().clone().expect("invoke precedes collect");
        let spec = self
            .specs
            .borrow()
            .last()
            .cloned()
            .expect("invoke precedes collect");
        let raw = std::fs::read_to_string(root.join(spec.result_contract_path.as_str())).map_err(
            |error| gauntlet_ports::agent_runner::AgentError::CollectionFailed {
                invocation_id: handle.invocation_id().clone(),
                detail: error.to_string(),
            },
        )?;
        RoleResult::decode(&raw).map_err(|error| {
            gauntlet_ports::agent_runner::AgentError::CollectionFailed {
                invocation_id: handle.invocation_id().clone(),
                detail: error.to_string(),
            }
        })
    }

    fn cancel(
        &self,
        _handle: &InvocationHandle,
        _reason: gauntlet_ports::agent_runner::CancelReason,
        _deadline: Deadline,
    ) -> Result<(), gauntlet_ports::agent_runner::AgentError> {
        Ok(())
    }

    fn capabilities(&self) -> gauntlet_ports::agent_runner::RunnerCapabilities {
        gauntlet_ports::agent_runner::RunnerCapabilities {
            supports_native_subagents: false,
            supports_streaming_events: false,
            supports_model_selection: false,
            isolation: gauntlet_ports::agent_runner::Isolation::None,
            result_delivery: gauntlet_ports::agent_runner::ResultDelivery::File,
            max_depth: 1,
        }
    }
}

/// The model rung over the scripted judgment (the app-layer rung of last
/// resort, §14.2 position 10).
/// The scripted judgment is single-threaded test state behind the rung
/// handle — the recorded allow the ladder's rung wrapping carries.
#[allow(clippy::arc_with_non_send_sync)]
fn judgment_rung(
    unit: &'static str,
    claim: &'static str,
    kind: EvidenceKind,
    at: &'static str,
    iteration: u32,
) -> (Arc<ScriptedJudgment>, LadderRung) {
    let runner = Arc::new(ScriptedJudgment::new(unit, claim, kind, at));
    let rung = ModelRung::new(
        runner.clone(),
        seeded_ids(),
        None,
        budget(),
        IterationNo::new(iteration).expect("iterations are 1-based"),
    )
    .rung();
    (runner, rung)
}

/// The full five-rung ladder: the four REAL deterministic adapters plus the
/// model rung, in §14.2 position order by construction.
fn full_ladder(fixture: &Fixture, model: LadderRung) -> Ladder {
    Ladder::new(vec![
        LadderRung::new(
            RungKind::FileDigest,
            Arc::new(
                FileDigestVerifier::new(fixture.workspace.clone())
                    .expect("the real file-digest adapter binds"),
            ),
        ),
        LadderRung::new(
            RungKind::SchemaCheck,
            Arc::new(
                SchemaCheckVerifier::new(fixture.workspace.clone(), vec!["tests".to_owned()])
                    .expect("the real schema-check adapter binds"),
            ),
        ),
        LadderRung::new(
            RungKind::CommandExitCode,
            Arc::new(
                CommandExitVerifier::new(fixture.workspace.clone())
                    .expect("the real command-exit adapter binds"),
            ),
        ),
        LadderRung::new(
            RungKind::TestReport,
            Arc::new(
                TestReportVerifier::new(fixture.workspace.clone())
                    .expect("the real test-report adapter binds"),
            ),
        ),
        model,
    ])
    .expect("the five rungs occupy five distinct 14.2 positions")
}

// ---------------------------------------------------------------------------
// 1 — the instrument layer: absent evidence is a typed error, not silence
// ---------------------------------------------------------------------------

#[test]
fn the_four_real_adapters_report_absent_evidence_as_a_typed_error() {
    let fixture = Fixture::new("instruments");
    // file/digest rung: a passing artifact pair, then an absent locator.
    FileDigestVerifier::write_passing_artifact(
        &fixture.workspace,
        "p12-manifest.list",
        "p12-content.bin",
        b"the digest-checked content",
    )
    .expect("the real adapter writes its passing artifact");
    let digest = FileDigestVerifier::new(fixture.workspace.clone())
        .expect("the real file-digest adapter binds");
    // schema rung.
    let keys = vec!["tests".to_owned(), "failures".to_owned()];
    std::fs::write(
        fixture.workspace.join("p12-schema.json"),
        SchemaCheckVerifier::passing_artifact(&keys),
    )
    .expect("the schema artifact is written");
    let schema = SchemaCheckVerifier::new(fixture.workspace.clone(), keys)
        .expect("the real schema-check adapter binds");
    // compiler/command rung.
    gauntlet_adapter_verify_command_exit_code::write_transcript(
        &fixture.workspace,
        "p12-transcript.list",
        "the-fixture-build",
        0,
    )
    .expect("the real adapter writes its passing transcript");
    let command = CommandExitVerifier::new(fixture.workspace.clone())
        .expect("the real command-exit adapter binds");
    // focused-test rung.
    fixture.write_passing_report(REPORT_LOCATOR);
    let report = TestReportVerifier::new(fixture.workspace.clone())
        .expect("the real test-report adapter binds");

    let request = |kind: EvidenceKind, at: &str| VerificationRequest {
        id: VerificationRef::new("vrf-p12-instrument").expect("a valid ref"),
        run_id: run_id(),
        unit_id: UnitId::new(REPORT_UNIT).expect("the fixture unit is valid"),
        claim: "the instrument decides the evidence content".to_owned(),
        requirement: gauntlet_domain::EvidenceRequirement::new(
            kind,
            locator(at),
            Producer::Instrument,
        ),
        evidence: locator(at),
    };

    // Present-and-passing evidence is a PASS decision for every instrument.
    assert_eq!(
        digest
            .verify(&request(EvidenceKind::File, "p12-manifest.list"), bound())
            .expect("the digest rung decides")
            .outcome(),
        Outcome::Pass
    );
    assert_eq!(
        schema
            .verify(&request(EvidenceKind::Scan, "p12-schema.json"), bound())
            .expect("the schema rung decides")
            .outcome(),
        Outcome::Pass
    );
    assert_eq!(
        command
            .verify(&request(EvidenceKind::Log, "p12-transcript.list"), bound())
            .expect("the command rung decides")
            .outcome(),
        Outcome::Pass
    );
    assert_eq!(
        report
            .verify(&request(EvidenceKind::TestReport, REPORT_LOCATOR), bound())
            .expect("the test-report rung decides")
            .outcome(),
        Outcome::Pass
    );

    // Absent evidence is the typed instrument error — one per rung, and
    // never a PASS and never silence (the shared suite pins this; here it
    // is on the composed path with the real adapters).
    assert!(matches!(
        digest.verify(
            &request(EvidenceKind::File, "p12-absent-manifest.list"),
            bound()
        ),
        Err(VerifierError::EvidenceMissing { .. })
    ));
    assert!(matches!(
        schema.verify(
            &request(EvidenceKind::Scan, "p12-absent-schema.json"),
            bound()
        ),
        Err(VerifierError::EvidenceMissing { .. })
    ));
    assert!(matches!(
        command.verify(
            &request(EvidenceKind::Log, "p12-absent-transcript.list"),
            bound()
        ),
        Err(VerifierError::EvidenceMissing { .. })
    ));
    assert!(matches!(
        report.verify(
            &request(EvidenceKind::TestReport, "p12-absent-report.json"),
            bound()
        ),
        Err(VerifierError::EvidenceMissing { .. })
    ));
}

// ---------------------------------------------------------------------------
// 2 — the binding layer: absent STORE evidence, with the artifact present
//     and the request lying — INCONCLUSIVE, never PASS, at every layer
// ---------------------------------------------------------------------------

#[test]
fn absent_store_evidence_is_inconclusive_at_every_layer_never_pass() {
    let fixture = Fixture::new("absent-store");
    // The artifact EXISTS at the locator and would PASS; the production
    // record is fresh and by the right instrument class — but its evidence
    // ref was never stored (the bytes are absent from the evidence store).
    fixture.write_passing_report(REPORT_LOCATOR);
    let producer = report_instrument();
    let never_stored = reference("p12-never-stored");
    let production = EvidenceProduction::new(
        never_stored.clone(),
        // The producer-side content digest (C3) of the bytes it says it
        // ingested — the resolve check fires before any digest comparison
        // because those bytes never landed in the store.
        content_digest(b"the bytes the producer claims to have ingested"),
        producer,
        locator(REPORT_LOCATOR),
        IterationNo::new(2).expect("iterations are 1-based"),
        EpochSeconds::new(BASE_INSTANT + 200),
    );
    fixture
        .state
        .record_evidence_production(&fixture.run(), &production, bound())
        .expect("the production record lands");

    // The request lies about everything decidable. The decision still reads
    // durable state: the requirement's evidence does not resolve there.
    let claim = fixture
        .governed(2, report_ladder(&fixture))
        .verify_claim(&lying_request(REPORT_UNIT), bound())
        .expect("the chain mints a record, not an error");
    assert_eq!(claim.record().outcome(), Outcome::Inconclusive);
    assert_eq!(
        claim.record().class(),
        Some(BindingClass::EvidenceUnresolvable)
    );
    assert!(
        claim.record().detail().contains("does not resolve"),
        "the detail names the failure locus: {}",
        claim.record().detail()
    );
    assert_ne!(claim.record().outcome(), Outcome::Pass);

    // And the settlement precondition refuses it: INCONCLUSIVE never
    // satisfies a settlement requirement (§16.1 condition 1, AK-005).
    let refused = settlement_precondition(&claim, &fixture.run(), bound());
    assert!(matches!(
        refused,
        Err(PreconditionFailure::NotPass {
            outcome: Outcome::Inconclusive
        })
    ));
}

// ---------------------------------------------------------------------------
// 3 — the layered P12 reconciliation: binding checks pass, the INSTRUMENT
//     finds no evidence — Err(EvidenceMissing) becomes INCONCLUSIVE
// ---------------------------------------------------------------------------

#[test]
fn an_absent_artifact_at_a_resolving_production_is_inconclusive_p12() {
    let fixture = Fixture::new("absent-artifact");
    // Fresh production whose ref RESOLVES — but no artifact file at the
    // locator: the real adapter reports EvidenceMissing, and the chain
    // converts it to INCONCLUSIVE/evidence_unresolvable, naming the rung.
    fixture.produce(
        &report_instrument(),
        REPORT_LOCATOR,
        2,
        PASSING_REPORT,
        EvidenceKind::TestReport,
    );
    let claim = fixture
        .governed(2, report_ladder(&fixture))
        .verify_claim(&lying_request(REPORT_UNIT), bound())
        .expect("the chain mints a record");
    assert_eq!(claim.record().outcome(), Outcome::Inconclusive);
    assert_eq!(
        claim.record().class(),
        Some(BindingClass::EvidenceUnresolvable)
    );
    assert!(
        claim.record().detail().contains("P12"),
        "the record cites the P12 rule: {}",
        claim.record().detail()
    );
    assert_eq!(
        claim.record().instrument().name(),
        "gauntlet-verify-test-report",
        "the instrument that reported the absence is named on the record"
    );
    let refused = settlement_precondition(&claim, &fixture.run(), bound());
    assert!(matches!(
        refused,
        Err(PreconditionFailure::NotPass {
            outcome: Outcome::Inconclusive
        })
    ));
}

// ---------------------------------------------------------------------------
// 4 — stale evidence from a prior iteration (§14.3 condition 3)
// ---------------------------------------------------------------------------

#[test]
fn stale_evidence_from_a_prior_iteration_is_inconclusive_stale() {
    let fixture = Fixture::new("stale");
    // The evidence is real, present, and passing — but it was produced at
    // iteration 1, and the governed verification stands at iteration 2:
    // the evidence predates the transform it must certify.
    fixture.write_passing_report(REPORT_LOCATOR);
    fixture.produce(
        &report_instrument(),
        REPORT_LOCATOR,
        1,
        PASSING_REPORT,
        EvidenceKind::TestReport,
    );
    let claim = fixture
        .governed(2, report_ladder(&fixture))
        .verify_claim(&lying_request(REPORT_UNIT), bound())
        .expect("the chain mints a record");
    assert_eq!(claim.record().outcome(), Outcome::Inconclusive);
    assert_eq!(claim.record().class(), Some(BindingClass::EvidenceStale));
    let refused = settlement_precondition(&claim, &fixture.run(), bound());
    assert!(matches!(
        refused,
        Err(PreconditionFailure::NotPass {
            outcome: Outcome::Inconclusive
        })
    ));
}

// ---------------------------------------------------------------------------
// 5 — a request lying about recency cannot make stale evidence fresh
// ---------------------------------------------------------------------------

#[test]
fn a_request_lying_about_recency_producer_and_locator_cannot_make_stale_fresh() {
    let fixture = Fixture::new("lying-stale");
    fixture.write_passing_report(REPORT_LOCATOR);
    let fresh_once = fixture.produce(
        &report_instrument(),
        REPORT_LOCATOR,
        1,
        PASSING_REPORT,
        EvidenceKind::TestReport,
    );
    // The lie: the request's claim asserts freshness, its requirement names
    // a human producer at another locator, its evidence pointer names a
    // different artifact. The frozen requirement and the durable production
    // record are what decide — and they say iteration 1 < governed 3.
    let mut lying = lying_request(REPORT_UNIT);
    lying.claim = "this evidence was produced just now in this iteration".to_owned();
    let claim = fixture
        .governed(3, report_ladder(&fixture))
        .verify_claim(&lying, bound())
        .expect("the chain mints a record");
    assert_eq!(claim.record().outcome(), Outcome::Inconclusive);
    assert_eq!(claim.record().class(), Some(BindingClass::EvidenceStale));
    // The record's requirement is the FROZEN one, not the lying one.
    assert_eq!(
        claim.record().requirement().kind(),
        EvidenceKind::TestReport
    );
    assert_eq!(
        claim.record().requirement().producer(),
        Producer::Instrument
    );
    assert_eq!(
        claim.record().requirement().locator().as_str(),
        REPORT_LOCATOR
    );
    // The stale evidence the durable record names is the one on the record.
    assert_eq!(claim.record().evidence().as_str(), fresh_once.as_str());
}

// ---------------------------------------------------------------------------
// 6 — producer mismatch (§14.3 condition 2): instrument evidence never
//     satisfies a human-producer requirement
// ---------------------------------------------------------------------------

#[test]
fn instrument_produced_evidence_never_satisfies_a_human_producer() {
    let fixture = Fixture::new("producer");
    // The signoff requirement's producer is HUMAN (frozen); the durable
    // production names an instrument. The artifact is present, fresh, and
    // would pass if dispatched — the producer check refuses it first.
    fixture.write_passing_report(SIGNOFF_LOCATOR);
    fixture.produce(
        &report_instrument(),
        SIGNOFF_LOCATOR,
        2,
        PASSING_REPORT,
        EvidenceKind::TestReport,
    );
    let claim = fixture
        .governed(2, report_ladder(&fixture))
        .verify_claim(&lying_request(SIGNOFF_UNIT), bound())
        .expect("the chain mints a record");
    assert_eq!(claim.record().outcome(), Outcome::Inconclusive);
    assert_eq!(
        claim.record().class(),
        Some(BindingClass::EvidenceProducerMismatch)
    );
    // The requirement on the record is the frozen human-producer one.
    assert_eq!(claim.record().requirement().producer(), Producer::Human);
}

// ---------------------------------------------------------------------------
// 7 — locator mismatch (§14.3 condition 4, the recorded mapping): evidence
//     produced at another locator is not the required evidence
// ---------------------------------------------------------------------------

#[test]
fn evidence_at_another_locator_is_unresolvable_not_the_required_evidence() {
    let fixture = Fixture::new("locator");
    // Fresh, resolving, passing evidence — produced at a DIFFERENT locator
    // than the frozen requirement's. The requirement's locator has no
    // production at all: evidence at another locator is not the required
    // evidence (the recorded mapping maps §14.3's fourth condition onto
    // `evidence_unresolvable`).
    fixture.write_passing_report(ELSEWHERE_LOCATOR);
    fixture.produce(
        &report_instrument(),
        ELSEWHERE_LOCATOR,
        2,
        PASSING_REPORT,
        EvidenceKind::TestReport,
    );
    let claim = fixture
        .governed(2, report_ladder(&fixture))
        .verify_claim(&lying_request(REQUIRED_UNIT), bound())
        .expect("the chain mints a record");
    assert_eq!(claim.record().outcome(), Outcome::Inconclusive);
    assert_ne!(claim.record().outcome(), Outcome::Pass);
    assert_eq!(
        claim.record().class(),
        Some(BindingClass::EvidenceUnresolvable)
    );
    assert!(
        claim.record().detail().contains(REQUIRED_LOCATOR),
        "the detail names the locator whose evidence does not resolve: {}",
        claim.record().detail()
    );
    let refused = settlement_precondition(&claim, &fixture.run(), bound());
    assert!(matches!(
        refused,
        Err(PreconditionFailure::NotPass {
            outcome: Outcome::Inconclusive
        })
    ));
}

// ---------------------------------------------------------------------------
// 8 — the happy anchor: fresh, resolving, pinned evidence PASSES and
//     approaches settlement (§16.1 conditions 1–2 positive)
// ---------------------------------------------------------------------------

#[test]
fn fresh_resolving_pinned_evidence_passes_and_approaches_settlement() {
    let fixture = Fixture::new("happy");
    fixture.write_passing_report(REPORT_LOCATOR);
    let evidence = fixture.produce(
        &report_instrument(),
        REPORT_LOCATOR,
        2,
        PASSING_REPORT,
        EvidenceKind::TestReport,
    );
    fixture
        .evidence
        .pin(&fixture.run(), &evidence, bound())
        .expect("the run pins its evidence");
    let claim = fixture
        .governed(2, report_ladder(&fixture))
        .verify_claim(&lying_request(REPORT_UNIT), bound())
        .expect("the chain mints a record");
    assert_eq!(claim.record().outcome(), Outcome::Pass);
    assert_eq!(claim.record().class(), None);
    assert_eq!(claim.record().evidence().as_str(), evidence.as_str());
    assert_eq!(
        claim.record().instrument().name(),
        "gauntlet-verify-test-report"
    );
    assert!(claim.record().instrument().deterministic());
    // The settlement precondition re-derives resolution and pinning from
    // the store and grants readiness.
    let readiness = settlement_precondition(&claim, &fixture.run(), bound())
        .expect("PASS + resolves + pinned = ready");
    assert_eq!(readiness.evidence().as_str(), evidence.as_str());
}

// ---------------------------------------------------------------------------
// 9 — deterministic-first (AK-035): when a model rung and a deterministic
//     rung could BOTH decide, the deterministic instrument decides
// ---------------------------------------------------------------------------

#[test]
fn the_deterministic_rung_decides_when_the_model_rung_could_too() {
    let fixture = Fixture::new("both-rungs");
    fixture.write_passing_report(REPORT_LOCATOR);
    fixture.produce(
        &report_instrument(),
        REPORT_LOCATOR,
        2,
        PASSING_REPORT,
        EvidenceKind::TestReport,
    );
    let (judgment, model) = judgment_rung(
        REPORT_UNIT,
        "the focused proof suite passes",
        EvidenceKind::TestReport,
        REPORT_LOCATOR,
        2,
    );
    let deterministic = LadderRung::new(
        RungKind::TestReport,
        Arc::new(
            TestReportVerifier::new(fixture.workspace.clone())
                .expect("the real test-report adapter binds"),
        ),
    );
    // Construction order deliberately puts the MODEL first: dispatch order
    // must still be ascending §14.2 position.
    let ladder = Ladder::new(vec![model, deterministic]).expect("the both-rung ladder is valid");
    assert_eq!(ladder.rungs()[0].kind(), RungKind::TestReport);
    assert_eq!(ladder.rungs()[1].kind(), RungKind::ModelJudgment);

    let claim = fixture
        .governed(2, ladder)
        .verify_claim(&lying_request(REPORT_UNIT), bound())
        .expect("the chain mints a record");
    // The deterministic instrument's identity is the one recorded.
    assert_eq!(claim.record().outcome(), Outcome::Pass);
    assert_eq!(
        claim.record().instrument().name(),
        "gauntlet-verify-test-report"
    );
    assert!(claim.record().instrument().deterministic());
    // And the model was never even dispatched — never ask a model to judge
    // what a deterministic instrument establishes (§14.2, AK-035).
    assert_eq!(
        judgment.dispatches(),
        0,
        "the model rung must not be dispatched when a deterministic rung decides"
    );
}

// ---------------------------------------------------------------------------
// 10 — every model-rung record is flagged non-deterministic (B.5)
// ---------------------------------------------------------------------------

#[test]
fn every_model_rung_record_is_flagged_non_deterministic() {
    let fixture = Fixture::new("model-flag");
    // Screenshot evidence: none of the four real deterministic rungs
    // declares it; the model rung (position 10) is the rung of last resort.
    fixture.produce(
        &report_instrument(),
        SURFACE_LOCATOR,
        2,
        b"a rendered surface capture",
        EvidenceKind::Screenshot,
    );
    std::fs::write(
        fixture.workspace.join(SURFACE_LOCATOR),
        b"a rendered surface capture",
    )
    .expect("the surface artifact is written");
    let (judgment, model) = judgment_rung(
        SURFACE_UNIT,
        SURFACE_CLAIM,
        EvidenceKind::Screenshot,
        SURFACE_LOCATOR,
        2,
    );
    let ladder = full_ladder(&fixture, model);
    let claim = fixture
        .governed(2, ladder)
        .verify_claim(&lying_request(SURFACE_UNIT), bound())
        .expect("the chain mints a record");
    assert_eq!(claim.record().outcome(), Outcome::Pass);
    assert_eq!(claim.record().instrument().name(), MODEL_RUNG_NAME);
    assert!(
        !claim.record().instrument().deterministic(),
        "every model-produced record is flagged non-deterministic (B.5)"
    );
    assert_eq!(
        judgment.dispatches(),
        1,
        "the model rung decided exactly once"
    );
}

// ---------------------------------------------------------------------------
// 12 — F1 (correction round 2): a doctored workspace file cannot ride the
//      locator to a verdict over pinned bytes it is not
// ---------------------------------------------------------------------------

/// F1 (MAJOR, confirmation round 1): the C3 content binding covered only
/// the STORE side; the confirmation critic EXECUTED the swapped-bytes
/// attack in the LOCATOR direction through this composed path — an honest
/// producer stores a FAILING report with its honest digest, the run pins
/// it, a builder-class write (spec §22.2) doctors the mutable workspace
/// file at the locator into a passing one, and the real adapter read the
/// doctored bytes. FAIL-FIRST evidence (2026-08-21, pristine HEAD —
/// captured verbatim in correction-round2.md): the attack minted `Pass`
/// and the settlement precondition GRANTED readiness over the pinned
/// FAILING bytes. The chain now re-reads the locator under the governed
/// workspace root BEFORE dispatch and requires the three-way
/// `digest(locator bytes) == production.content() == digest(store bytes)`;
/// the doctored write is a typed refusal, and no claim exists to settle.
#[test]
fn a_doctored_workspace_file_cannot_ride_the_locator_to_a_pass() {
    let fixture = Fixture::new("doctored-locator");
    // The honest producer: a FAILING report ingested at the locator,
    // stored through the REAL content-addressed store, recorded with its
    // honest digest, and pinned by the run.
    let evidence = fixture.produce(
        &report_instrument(),
        REPORT_LOCATOR,
        2,
        FAILING_REPORT,
        EvidenceKind::TestReport,
    );
    fixture
        .evidence
        .pin(&fixture.run(), &evidence, bound())
        .expect("the run pins its evidence");
    // The builder-class write: the mutable workspace file at the locator is
    // doctored into a PASSING report. Every durable column stays honest —
    // the store, the record, the digest, the pin — the lie lives only in
    // the examination surface.
    std::fs::write(fixture.workspace.join(REPORT_LOCATOR), PASSING_REPORT)
        .expect("the doctored workspace file is written");
    match fixture
        .governed(2, report_ladder(&fixture))
        .verify_claim(&lying_request(REPORT_UNIT), bound())
    {
        Err(gauntlet_app::verify::VerificationRefusal::LocatorContentMismatch {
            locator,
            recorded,
            examined,
        }) => {
            assert_eq!(locator.as_str(), REPORT_LOCATOR);
            assert_eq!(recorded, content_digest(FAILING_REPORT));
            assert_eq!(examined, content_digest(PASSING_REPORT));
        }
        Ok(claim) => panic!(
            "a doctored workspace file must be refused before dispatch, never mint an outcome; \
             got {:?} (settlement readiness granted: {})",
            claim.record().outcome(),
            settlement_precondition(&claim, &fixture.run(), bound()).is_ok()
        ),
        Err(other) => panic!("expected the typed locator-content refusal, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// 11 — the three §14.3 classes are reachable and distinct (no collapsed
//      catch-all)
// ---------------------------------------------------------------------------

#[test]
fn the_three_binding_classes_are_reachable_and_distinct() {
    // Three independent scenarios, one per class, through the composed
    // chain. Distinctness is asserted over the minted records' classes.
    let unresolvable = {
        let fixture = Fixture::new("class-unresolvable");
        fixture
            .governed(2, report_ladder(&fixture))
            .verify_claim(&lying_request(REPORT_UNIT), bound())
            .expect("the chain mints a record")
            .record()
            .class()
    };
    let stale = {
        let fixture = Fixture::new("class-stale");
        fixture.write_passing_report(REPORT_LOCATOR);
        fixture.produce(
            &report_instrument(),
            REPORT_LOCATOR,
            1,
            PASSING_REPORT,
            EvidenceKind::TestReport,
        );
        fixture
            .governed(2, report_ladder(&fixture))
            .verify_claim(&lying_request(REPORT_UNIT), bound())
            .expect("the chain mints a record")
            .record()
            .class()
    };
    let mismatch = {
        let fixture = Fixture::new("class-producer");
        fixture.write_passing_report(SIGNOFF_LOCATOR);
        fixture.produce(
            &report_instrument(),
            SIGNOFF_LOCATOR,
            2,
            PASSING_REPORT,
            EvidenceKind::TestReport,
        );
        fixture
            .governed(2, report_ladder(&fixture))
            .verify_claim(&lying_request(SIGNOFF_UNIT), bound())
            .expect("the chain mints a record")
            .record()
            .class()
    };
    let unresolvable =
        unresolvable.expect("no production at the locator yields the unresolvable class");
    let stale = stale.expect("a prior-iteration production yields the stale class");
    let mismatch = mismatch.expect(
        "an instrument production against a human requirement yields \
         the producer-mismatch class",
    );
    assert_ne!(unresolvable, stale);
    assert_ne!(stale, mismatch);
    assert_ne!(unresolvable, mismatch);
}
