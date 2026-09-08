//! The anti-gaming suite (plan Task 24, phase 6; ruler AK-058 + AK-070;
//! spec §23.2 G1–G16 + §23.1 teeth rule): one test per gaming strategy,
//! each naming the ONE mechanism whose mutation turns it RED, over REAL
//! adapters at the composition root.
//!
//! §23.1 (verbatim): "Each proof MUST have a **teeth check**: disabling
//! the defense makes the proof fail. A proof that passes with its
//! mechanism removed is testing nothing." The machine-readable teeth map
//! is `.agents/reports/defended-mechanisms.txt` (test_id | G-ID |
//! mechanism | source_location), parsed and enforced by
//! `the_defended_mechanisms_manifest_covers_all_sixteen_strategies` at
//! the bottom of this file; `scripts/run-mutation.sh` scopes
//! cargo-mutants to the manifest's files and fails on ANY survivor.
//!
//! Per-strategy depth notes (the manifest and each test's header carry
//! the details):
//!
//! - G6, G7, G10 have DEEP composed proofs already
//!   (`g6_hide_failed_observations.rs`, `g7_tier_downgrade.rs`,
//!   `g10_builder_writes_answer_key.rs` — settled work, cited not
//!   repeated). This suite's tests bind to the same mechanisms at the
//!   layer the manifest names, thinner by design.
//! - G9 is ADVISORY (the honest-boundary rule): the runtime has NO
//!   mechanical write-scope enforcement for a critic-context process —
//!   the role file's `filesystem: read-only` is a recorded, parsed
//!   posture (`roles::definition::FilesystemMode`), and the roles lock
//!   records `enforcement: advisory` when no writable role surface
//!   exists (`roles::install::Enforcement::Advisory` — "the lock records
//!   the weakness instead of faking enforcement"). The g9 test asserts
//!   exactly that advisory surface and says so in its header; the
//!   manifest entry carries `advisory:` and NO mutation scope.
//! - G16 exercises the promotion gate's re-adjudication discipline
//!   (spec §23.2 "Re-ask a model until it returns `pass`", at the
//!   §19.4 layer): a FAILED condition recorded by an EARLIER evaluation
//!   is durable evidence a re-run cannot launder.
//!
//! Composition inventory (spec §4.1 — everything REAL unless named):
//! embedded-SQL state/revision/capability stores on scratch files, the
//! real fs content-addressed evidence store, the real file reference
//! provider, the real comparison-contract parser, the real §19.4
//! promotion harness/gate/proposal services, the real deterministic
//! aggregation and observation-digest modules, the ports' settlement/
//! verification authorities, the governed comparison chain
//! (ComparisonAuthority -> issue -> review -> accept, with a proof-local
//! Comparator in the review source seam for G8 — the seam a model
//! adapter occupies, flowing the same acceptance/aggregation path), and
//! the mock adapter's deterministic clock. No leg dispatches an AGENT:
//! the claims are about what the durable substrate, the services, and
//! the comparison chain prove (the p10 precedent); the deep proofs that
//! DO dispatch through the composed loop are cited above.
//!
//! No network, no model, no new dependencies. Whole-target wall clock
//! stays minutes, not the ~10-minute ceiling.

use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};

use gauntlet_adapter_agent_mock::DeterministicClock;
use gauntlet_adapter_evidence_fs::FsEvidenceStore;
use gauntlet_adapter_meter_memory::MeteredBudget;
use gauntlet_adapter_state_sqlite::{
    EmbeddedCapabilityLedger, EmbeddedRevisionLedger, EmbeddedStateStore,
};
use gauntlet_app::compare::authority::ComparisonAuthority;
use gauntlet_app::compare::contract::{ComparisonContract, ContractError};
use gauntlet_app::compare::{DENY_LISTED_FIELD_NAMES, assert_blind};
use gauntlet_app::compare::{ModelDispatchBudget, ReviewDeps};
use gauntlet_app::govern::{ChangeOrigin, LatticeMovement, review_lattice_movement};
use gauntlet_app::observation::bundle::{
    BundleContent, ClaimsFacet, FacetRecord, ObservationBundleDoc, ObservationFailureRecord,
};
use gauntlet_app::observation::digest::{normalized_bytes, observation_digest};
use gauntlet_app::promote::delta::DeltaDocument;
use gauntlet_app::promote::evaluate::{
    EvaluationExecutor, EvaluationHarness, EvaluationRequest, ExecutionError, ExecutionOutcome,
    ExecutionRequest,
};
use gauntlet_app::promote::gate::{GateInput, GateVerdict, PromotionGate};
use gauntlet_app::promote::metrics::TargetMetric;
use gauntlet_app::promote::proposal::ProposalService;
use gauntlet_app::reference::normalize::{digest_normalized, normalize_bytes};
use gauntlet_app::reference::provider::{AnswerKeySource, FileReferenceProvider, ManifestSource};
use gauntlet_domain::{
    Authority, BlastRadius, CapabilityId, Digest, EvidenceRef, InstrumentName, IterationNo,
    ResidualId, RevisionId, RoleId, RunId, Severity, UnitId, Verdict,
};
use gauntlet_ports::clock::Clock;
use gauntlet_ports::comparison_record::{
    ComparatorIdentity, ComparisonRecordDoc, RecordContent, RecordVerdict, ResidualDoc,
};
use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::evidence_store::EvidenceStore;
use gauntlet_ports::observer::{FacetKind, InstrumentIdentity, ObservationFacet};
use gauntlet_ports::producer_authority::{SettlementCommitAuthority, VerificationRecordAuthority};
use gauntlet_ports::reference_provider::{ReferenceProvider, ReferenceProviderError};
use gauntlet_ports::revision_ledger::{
    EvaluationRecord, RevisionLedger, RunMetrics, SettlementFact,
};
use gauntlet_ports::state_store::{Checkpoint, SettlementCommit, StateStore};
use gauntlet_ports::verification_record::{
    RequirementDoc, VerificationContent, VerificationRecordDoc, VerifyingInstrument,
};

// ---------------------------------------------------------------------------
// Named fixture facts (§8.1 literal discipline — never a bare numeral on
// a line that names a port-shaped identifier).
// ---------------------------------------------------------------------------

const BASE_INSTANT: u64 = 1_700_000_000;
const BOUND_SPAN: u64 = 600;
const CHECKPOINT_ITERATION: u32 = 3;
const SPENT_WALL_SPAN: u64 = 30;
const UNIT_TOKENS: u64 = 300;
/// The authored per-invocation economy: the baseline runner books the
/// expensive cost per settled unit; the candidate's runner books the
/// economy cost (the p9 authored-economy pattern). Three units per arm.
const BASELINE_RUNNER: &str = "mock-agent";
const CANDIDATE_RUNNER: &str = "antigaming-candidate-runner";
const EXPENSIVE_UNIT_COST_MICROS: u64 = 3 * 1_000;
const ECONOMY_UNIT_COST_MICROS: u64 = 2_000;
/// The control arm's payment-per-settlement figure (fixed-point per
/// 10 000): 9 000 micro across 3 settlements.
const EXPECTED_CONTROL_FIGURE: u64 = 30_000_000;
/// The treatment arm's figure with the economy cost: 6 000 micro across
/// 3 settlements.
const EXPECTED_TREATMENT_FIGURE: u64 = 20_000_000;

const REFERENCE_ID: &str = "ref-antigaming-gate";
const REFERENCE_DIGEST: &str = "sha256:1c2d3e4f5a6b7c8d";
const INTENT: &str = "the antigaming gate-fixture evaluation";
const PROTECTED_CAPABILITY: &str = "cap-antigaming-gate";
const GATE_UNITS: [&str; 3] = ["AK-9901", "AK-9902", "AK-9903"];
/// The two task-set roots the harness hands the executor; the executor
/// never touches the filesystem, it only needs to DISTINGUISH the sets.
const HELD_OUT_ROOT: &str = "task-set-held-out";
const REGRESSION_ROOT: &str = "task-set-regression";

const EVIDENCE_LOCATOR: &str = "antigaming-fixture-report.json";
const INSTRUMENT_NAME: &str = "gauntlet-antigaming-fixture-instrument";
const CLAIM: &str = "the unit's behavioral claim holds";

fn bound() -> Deadline {
    Deadline::at_epoch(gauntlet_domain::EpochSeconds::new(
        BASE_INSTANT + BOUND_SPAN,
    ))
}

fn clock() -> Arc<DeterministicClock> {
    Arc::new(DeterministicClock::start_at(
        gauntlet_domain::EpochSeconds::new(BASE_INSTANT),
    ))
}

fn gate_reference_digest() -> Digest {
    Digest::parse(REFERENCE_DIGEST).expect("the fixture digest is well formed")
}

// ---------------------------------------------------------------------------
// The gate world: the p9 composition — REAL proposal service, REAL
// evaluation harness, REAL promotion gate, REAL embedded-SQL ledgers, and
// an executor seam authored over the REAL earned authorities (durable run
// identity, checkpoints with `budget_spent`, authority-minted verification
// records, authority-admitted + committed §16.3 settlements — never
// hand-built rows).
// ---------------------------------------------------------------------------

/// The gate-fixture scratch database (state store + revision ledger +
/// capability ledger share ONE file, as in production).
struct GateScratch {
    path: PathBuf,
}

impl GateScratch {
    fn new(tag: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "gauntlet-antigaming-gate-{tag}-{}",
            std::process::id()
        ));
        std::fs::remove_file(&path).ok();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("the gate-fixture parent is creatable");
        }
        Self { path }
    }

    fn open_state(&self) -> Arc<EmbeddedStateStore> {
        Arc::new(EmbeddedStateStore::open(&self.path).expect("the gate-fixture state store opens"))
    }

    fn open_revisions(&self) -> Arc<EmbeddedRevisionLedger> {
        Arc::new(
            EmbeddedRevisionLedger::open(&self.path)
                .expect("the gate-fixture revision ledger opens"),
        )
    }

    fn open_capabilities(&self) -> Arc<EmbeddedCapabilityLedger> {
        Arc::new(
            EmbeddedCapabilityLedger::open(&self.path)
                .expect("the gate-fixture capability ledger opens"),
        )
    }
}

impl Drop for GateScratch {
    fn drop(&mut self) {
        std::fs::remove_file(&self.path).ok();
    }
}

/// The gate-fixture world: everything REAL, one scratch, plus the
/// evidence store the delta/rollback pins resolve through.
struct GateWorld {
    scratch: GateScratch,
    evidence_dir: PathBuf,
    clock: Arc<DeterministicClock>,
    /// The world's run-id counter, shared by every executor the legs
    /// compose (run identity is unique per world, never per executor).
    runs: Arc<AtomicU64>,
}

impl GateWorld {
    fn new(tag: &str) -> Self {
        let evidence_dir = std::env::temp_dir().join(format!(
            "gauntlet-antigaming-gate-{tag}-evidence-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&evidence_dir);
        std::fs::create_dir_all(&evidence_dir).expect("the gate evidence dir is creatable");
        Self {
            scratch: GateScratch::new(tag),
            evidence_dir,
            clock: clock(),
            runs: Arc::new(AtomicU64::new(0)),
        }
    }

    fn open_evidence(&self) -> Arc<gauntlet_adapter_evidence_fs::FsEvidenceStore> {
        Arc::new(
            gauntlet_adapter_evidence_fs::FsEvidenceStore::open(&self.evidence_dir)
                .expect("the gate evidence store opens"),
        )
    }

    /// Pins bytes and returns the content-addressed ref.
    fn pin(&self, bytes: &[u8]) -> String {
        self.open_evidence()
            .put(
                bytes,
                gauntlet_domain::EvidenceKind::File,
                Deadline::at_epoch(gauntlet_domain::EpochSeconds::new(
                    BASE_INSTANT + BOUND_SPAN,
                )),
            )
            .expect("the gate-fixture bytes pin")
            .as_str()
            .to_owned()
    }

    /// Admits the candidate through the REAL proposal service.
    fn propose(&self, document: &str) -> RevisionId {
        let service = ProposalService::new(self.scratch.open_revisions(), self.clock.clone());
        service
            .admit_document(document, bound())
            .expect("the gate-fixture candidate admits")
            .revision
    }

    /// The gate input, resolved EXACTLY as the CLI resolves it: the delta
    /// bytes and the rollback resolution both read through the REAL
    /// evidence store, via the DURABLE proposal's own refs.
    fn gate_input(&self, revision: &RevisionId) -> GateInput {
        let evidence = self.open_evidence();
        let ledger = self.scratch.open_revisions();
        let proposal = ledger
            .proposal(revision, bound())
            .expect("the proposal reads")
            .expect("the proposal is durable");
        let bytes = evidence
            .get(proposal.diff_ref(), bound())
            .expect("the delta bytes read")
            .expect("the delta bytes are pinned");
        let text = String::from_utf8(bytes).expect("the delta bytes are UTF-8");
        GateInput {
            current_reference_digest: gate_reference_digest(),
            delta: DeltaDocument::parse(&text).expect("the pinned delta parses"),
            rollback_resolvable: Some(true),
        }
    }
}

/// What the executor's authored world looks like for the TREATMENT arm —
/// the honest seam for "re-run the evaluation and hope": the candidate
/// delta (the runner switch) is always genuinely applied; what varies
/// between evaluations is the candidate runner's AUTHORED economy, the
/// same way a real loop's re-run can land on different durable facts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Economy {
    /// The candidate runner is genuinely cheaper (the p9 honest road).
    Improves,
    /// The candidate runner costs exactly what the baseline costs — the
    /// switch applies but the metric does NOT improve (the failing
    /// evaluation a re-ask attacker hopes to outrun).
    Flat,
    /// The candidate regresses the REGRESSION task set: its treatment arm
    /// there settles every unit but the last (a real, durable, earned
    /// regression shape).
    RegressesLastRegressionUnit,
}

struct EconomyExecutor {
    state: Arc<EmbeddedStateStore>,
    clock: Arc<DeterministicClock>,
    /// Shared with the WORLD (not the executor instance): two evaluations
    /// on one revision compose two executors over ONE durable store, and
    /// run identity must never collide across them.
    runs: Arc<AtomicU64>,
    economy: AtomicU8,
}

impl EconomyExecutor {
    fn new(
        state: Arc<EmbeddedStateStore>,
        clock: Arc<DeterministicClock>,
        economy: Economy,
        runs: Arc<AtomicU64>,
    ) -> Self {
        let encoded = match economy {
            Economy::Improves => 0,
            Economy::Flat => 1,
            Economy::RegressesLastRegressionUnit => 2,
        };
        Self {
            state,
            clock,
            runs,
            economy: AtomicU8::new(encoded),
        }
    }

    fn economy(&self) -> Economy {
        match self.economy.load(Ordering::Relaxed) {
            1 => Economy::Flat,
            2 => Economy::RegressesLastRegressionUnit,
            _ => Economy::Improves,
        }
    }

    fn next_run_id(&self) -> gauntlet_domain::RunId {
        // Unique per arm across the WHOLE fixture process: two
        // evaluations on one revision must never collide on run identity.
        let sequence = self.runs.fetch_add(1, Ordering::Relaxed) + 1;
        let stem = format!("01ARZ3NDEKTSV4RRFFQ69G5F{sequence:02X}");
        gauntlet_domain::RunId::new(&stem).expect("the fixture run id is a ULID")
    }

    fn unit_cost_micros(&self, treatment: bool) -> u64 {
        match self.economy() {
            Economy::Improves => {
                if treatment {
                    ECONOMY_UNIT_COST_MICROS
                } else {
                    EXPENSIVE_UNIT_COST_MICROS
                }
            }
            Economy::Flat => EXPENSIVE_UNIT_COST_MICROS,
            // The regression shape still improves the held-out economy —
            // the candidate's ONLY defect is the skipped regression unit.
            Economy::RegressesLastRegressionUnit => {
                if treatment {
                    ECONOMY_UNIT_COST_MICROS
                } else {
                    EXPENSIVE_UNIT_COST_MICROS
                }
            }
        }
    }
}

impl EvaluationExecutor for EconomyExecutor {
    /// The composition applies exactly the `runners.default` entry — any
    /// other key is one this seam cannot provably apply and is refused
    /// (the p9 seam's honest rule).
    fn validate_delta(&self, delta: &DeltaDocument) -> Result<(), ExecutionError> {
        let mut refused: Vec<String> = Vec::new();
        for entry in delta.entries() {
            let appliable = entry.key == "runners.default"
                && (entry.value == BASELINE_RUNNER || entry.value == CANDIDATE_RUNNER);
            if !appliable {
                refused.push(entry.key.clone());
            }
        }
        if refused.is_empty() {
            Ok(())
        } else {
            Err(ExecutionError::Failed {
                detail: format!(
                    "entries {refused:?} are outside this composition's authored manifest"
                ),
            })
        }
    }

    fn execute(
        &self,
        request: &ExecutionRequest,
        deadline: Deadline,
    ) -> Result<ExecutionOutcome, ExecutionError> {
        // Apply the declared delta: the runner switch (validated above) —
        // the only authored entry, and only to the candidate runner.
        if let Some(delta) = &request.delta {
            for entry in delta.entries() {
                if entry.key == "runners.default" && entry.value == CANDIDATE_RUNNER {
                    continue;
                }
                return Err(ExecutionError::Failed {
                    detail: format!("entry {} cannot be applied here", entry.key),
                });
            }
        }
        let treatment = request.delta.is_some();
        let on_regression_set = request.project_root == std::path::Path::new(REGRESSION_ROOT);
        let units: &[&str] = if treatment
            && on_regression_set
            && self.economy() == Economy::RegressesLastRegressionUnit
        {
            // The authored regression: every unit but the last settles.
            &GATE_UNITS[..GATE_UNITS.len() - 1]
        } else {
            &GATE_UNITS
        };
        let run_id = self.next_run_id();
        let run = gauntlet_domain::Run::new(gauntlet_domain::RunDefinition {
            id: run_id.clone(),
            created_at: gauntlet_domain::EpochSeconds::new(BASE_INSTANT),
            reference_version: gauntlet_domain::ReferenceVersion::new(
                gauntlet_domain::ReferenceId::new(REFERENCE_ID).expect("reference id"),
                1,
                gate_reference_digest(),
            ),
            intent: INTENT.to_owned(),
            config_fingerprint: Digest::new("sha256", "5a6b7c8d9e0f1a2b").expect("digest"),
            environment_fingerprint: Digest::new("sha256", "b1a2c3d4e5f6a7b8").expect("digest"),
            budget: gauntlet_domain::Budget::new(
                gauntlet_domain::Limits::new(
                    8,
                    2,
                    1,
                    3_600,
                    2_000_000,
                    gauntlet_domain::MicroUsd::new(500_000),
                )
                .expect("the fixture limits are usable"),
                15,
                gauntlet_domain::DiagnosticAllowance::new(100, gauntlet_domain::MicroUsd::new(50)),
            )
            .expect("the fixture budget is usable"),
            parent_run: request.parent_run.clone(),
        })
        .expect("the fixture run is valid");
        self.state
            .record_run(&run, deadline)
            .expect("the run identity records");
        // The arm's work, earned in honest iteration order: each unit's
        // verification record is MINTED under ITS OWN durable boundary
        // checkpoint, and every settlement is ADMITTED through the
        // authority and COMMITTED through the §16.3 path.
        let unit_cost = self.unit_cost_micros(treatment);
        let mut settled: Vec<(gauntlet_domain::UnitId, gauntlet_domain::UnitStatus)> = Vec::new();
        let mut booked_rounds: u32 = 0;
        let mut total_cost_micros: u64 = 0;
        let mut total_tokens: u64 = 0;
        for (index, unit) in units.iter().enumerate() {
            let iteration = (index + 1) as u32;
            self.state
                .checkpoint(
                    Checkpoint {
                        run_id: run_id.clone(),
                        reference_digest: gate_reference_digest(),
                        iteration_no: gauntlet_domain::IterationNo::new(iteration)
                            .expect("iterations are 1-based"),
                        unit_states: settled.clone(),
                        budget_spent: gauntlet_domain::Spent::from_counters(
                            booked_rounds,
                            SPENT_WALL_SPAN,
                            total_tokens,
                            gauntlet_domain::MicroUsd::new(total_cost_micros),
                            0,
                            gauntlet_domain::MicroUsd::new(0),
                        ),
                        in_flight: Vec::new(),
                        event_log_offset: gauntlet_domain::EventLogOffset::new(0),
                    },
                    deadline,
                )
                .expect("the boundary checkpoint lands");
            let sequence = index + 1;
            let verification = format!("vrf-{run_id}-{sequence}");
            let record = VerificationRecordDoc::new(VerificationContent {
                verification: gauntlet_domain::VerificationRef::new(&verification)
                    .expect("verification ref"),
                run_id: run_id.clone(),
                iteration_no: gauntlet_domain::IterationNo::new(iteration)
                    .expect("iterations are 1-based"),
                unit_id: gauntlet_domain::UnitId::new(unit).expect("unit id"),
                claim: CLAIM.to_owned(),
                requirement: RequirementDoc::new(
                    gauntlet_domain::EvidenceKind::TestReport,
                    gauntlet_domain::EvidenceLocator::new(EVIDENCE_LOCATOR).expect("locator"),
                    gauntlet_domain::Producer::Instrument,
                ),
                instrument: VerifyingInstrument::new(
                    INSTRUMENT_NAME.to_owned(),
                    "1".to_owned(),
                    true,
                ),
                evidence: gauntlet_domain::EvidenceRef::new(&format!("evd-{run_id}-{sequence}"))
                    .expect("evidence ref"),
                produced_at: gauntlet_domain::EpochSeconds::new(BASE_INSTANT + 10),
                outcome: gauntlet_domain::Outcome::Pass,
                class: None,
                detail: "the deterministic instrument decided PASS".to_owned(),
            })
            .expect("the fixture record is well formed");
            VerificationRecordAuthority::new(self.state.clone())
                .record(&record, deadline)
                .expect("the PASS record is earned and durable");
            let commit = SettlementCommit {
                run_id: run_id.clone(),
                unit_id: gauntlet_domain::UnitId::new(unit).expect("unit id"),
                evidence: record.evidence().clone(),
                verification: record.verification().clone(),
                reference_digest: gate_reference_digest(),
                at: self.clock.now(),
            };
            let admitted = SettlementCommitAuthority::new(self.state.clone())
                .admit(commit, deadline)
                .expect("the settlement admits");
            self.state
                .commit_settlement(&admitted, deadline)
                .expect("the settlement commits");
            settled.push((
                gauntlet_domain::UnitId::new(unit).expect("unit id"),
                gauntlet_domain::UnitStatus::Settled,
            ));
            booked_rounds = iteration;
            total_cost_micros += unit_cost;
            total_tokens += UNIT_TOKENS;
        }
        // The closing checkpoint: every (authored) unit SETTLED, the
        // arm's authored economy in `budget_spent` — the durable facts
        // the harness's metric derivation reads.
        self.state
            .checkpoint(
                Checkpoint {
                    run_id: run_id.clone(),
                    reference_digest: gate_reference_digest(),
                    iteration_no: gauntlet_domain::IterationNo::new(CHECKPOINT_ITERATION)
                        .expect("iterations are 1-based"),
                    unit_states: settled,
                    budget_spent: gauntlet_domain::Spent::from_counters(
                        booked_rounds,
                        SPENT_WALL_SPAN,
                        total_tokens,
                        gauntlet_domain::MicroUsd::new(total_cost_micros),
                        0,
                        gauntlet_domain::MicroUsd::new(0),
                    ),
                    in_flight: Vec::new(),
                    event_log_offset: gauntlet_domain::EventLogOffset::new(0),
                },
                deadline,
            )
            .expect("the closing checkpoint lands");
        Ok(ExecutionOutcome {
            run_id,
            terminal: gauntlet_domain::TerminalState::Settled,
            reference_digest: gate_reference_digest(),
            state: self.state.clone(),
        })
    }
}

/// Assembles the optimizer's result document over the given refs — the
/// strict `role_result@v1` envelope carrying the §19.3 nine-field payload
/// (the p9 shape).
fn candidate_document(
    target: &str,
    invocation: &str,
    provenance_run: &str,
    delta_ref: &str,
    rollback_ref: &str,
) -> String {
    use gauntlet_ports::json::{JsonNumber, JsonValue};
    let payload = JsonValue::Object(vec![
        (
            "claimed_problem".to_owned(),
            JsonValue::Str("the fixture's runner books expensive dispatch overhead".to_owned()),
        ),
        (
            "supporting_evidence".to_owned(),
            JsonValue::Array(vec![JsonValue::Str("sha256:feed0000face1111".to_owned())]),
        ),
        (
            "affected_component".to_owned(),
            JsonValue::Str(target.to_owned()),
        ),
        (
            "predicted_improvement".to_owned(),
            JsonValue::Str("the candidate's declared improvement".to_owned()),
        ),
        (
            "potential_regression".to_owned(),
            JsonValue::Array(vec![JsonValue::Str(
                "a candidate runner may regress the regression set".to_owned(),
            )]),
        ),
        (
            "candidate_change".to_owned(),
            JsonValue::Str(delta_ref.to_owned()),
        ),
        (
            "evaluation_plan".to_owned(),
            JsonValue::Str("held-out set: the fixture task sets".to_owned()),
        ),
        (
            "held_out_evaluation".to_owned(),
            JsonValue::Str("regression set: every unit must re-settle".to_owned()),
        ),
        (
            "rollback".to_owned(),
            JsonValue::Str(rollback_ref.to_owned()),
        ),
    ]);
    JsonValue::Object(vec![
        (
            "schema".to_owned(),
            JsonValue::Str(gauntlet_ports::role_result::ROLE_RESULT_SCHEMA.to_owned()),
        ),
        (
            "invocation_id".to_owned(),
            JsonValue::Str(invocation.to_owned()),
        ),
        (
            "run_id".to_owned(),
            JsonValue::Str(provenance_run.to_owned()),
        ),
        (
            "iteration_no".to_owned(),
            JsonValue::Number(JsonNumber::Uint(1)),
        ),
        ("role".to_owned(), JsonValue::Str("optimizer".to_owned())),
        (
            "context_envelope_digest".to_owned(),
            JsonValue::Str("sha256:a1b2c3d4e5f6a7b8".to_owned()),
        ),
        ("state".to_owned(), JsonValue::Str("completed".to_owned())),
        (
            "payload_schema".to_owned(),
            JsonValue::Str(gauntlet_ports::role_result::CANDIDATE_REVISION_SCHEMA.to_owned()),
        ),
        ("payload".to_owned(), payload),
        (
            "payment".to_owned(),
            JsonValue::Object(vec![
                (
                    "input_tokens".to_owned(),
                    JsonValue::Number(JsonNumber::Uint(0)),
                ),
                (
                    "output_tokens".to_owned(),
                    JsonValue::Number(JsonNumber::Uint(0)),
                ),
                (
                    "tool_calls".to_owned(),
                    JsonValue::Number(JsonNumber::Uint(0)),
                ),
                (
                    "wall_seconds".to_owned(),
                    JsonValue::Number(JsonNumber::Uint(0)),
                ),
                (
                    "cost_usd".to_owned(),
                    JsonValue::Number(JsonNumber::Float(0.0)),
                ),
            ]),
        ),
        ("unavailable_evidence".to_owned(), JsonValue::Array(vec![])),
        ("errors".to_owned(), JsonValue::Array(vec![])),
    ])
    .to_json_text()
}

fn delta_document(entries: &[(&str, &str)]) -> String {
    use gauntlet_ports::json::JsonValue;
    JsonValue::Array(
        entries
            .iter()
            .map(|(key, value)| {
                JsonValue::Object(vec![
                    ("key".to_owned(), JsonValue::Str((*key).to_owned())),
                    ("value".to_owned(), JsonValue::Str((*value).to_owned())),
                ])
            })
            .collect(),
    )
    .to_json_text()
}

/// Pins the runner-switch delta + rollback and admits the candidate.
/// The runner switch is the ONE inert key the seam can provably apply;
/// every authored-world difference lives in the executor's economy.
fn propose_runner_candidate(world: &GateWorld, tag: &str) -> (RevisionId, String) {
    let delta = delta_document(&[("runners.default", CANDIDATE_RUNNER)]);
    let rollback = delta_document(&[("runners.default", BASELINE_RUNNER)]);
    let delta_ref = world.pin(delta.as_bytes());
    let rollback_ref = world.pin(rollback.as_bytes());
    let revision = world.propose(&candidate_document(
        "runners.default",
        &format!("01ARZ3NDEKTSV4RRFFQ69G5FP{tag}"),
        &format!("01ARZ3NDEKTSV4RRFFQ69G5FA{tag}"),
        &delta_ref,
        &rollback_ref,
    ));
    (revision, delta_ref)
}

/// Runs ONE four-arm evaluation through the REAL harness.
fn evaluate_runner_candidate(
    world: &GateWorld,
    revision: &RevisionId,
    executor: &EconomyExecutor,
    protected: &[&str],
) {
    let harness = EvaluationHarness::new(
        world.scratch.open_revisions(),
        world.scratch.open_capabilities(),
        world.clock.clone(),
    );
    let request = EvaluationRequest {
        revision: revision.clone(),
        metric: TargetMetric::PaymentPerSettlement,
        current_reference_digest: gate_reference_digest(),
        delta: {
            let evidence = world.open_evidence();
            let ledger = world.scratch.open_revisions();
            let proposal = ledger
                .proposal(revision, bound())
                .expect("the proposal reads")
                .expect("durable");
            let bytes = evidence
                .get(proposal.diff_ref(), bound())
                .expect("the delta bytes read")
                .expect("pinned");
            DeltaDocument::parse(&String::from_utf8(bytes).expect("utf-8")).expect("parses")
        },
        held_out_root: PathBuf::from(HELD_OUT_ROOT),
        regression_root: PathBuf::from(REGRESSION_ROOT),
        protected: protected
            .iter()
            .map(|token| CapabilityId::new(token).expect("capability id"))
            .collect(),
        intent: INTENT.to_owned(),
    };
    harness
        .evaluate(&request, executor, bound())
        .expect("the evaluation records");
}

fn gate_for(world: &GateWorld) -> PromotionGate {
    PromotionGate::new(
        world.scratch.open_revisions(),
        world.scratch.open_capabilities(),
        world.clock.clone(),
        0,
    )
    .expect("the declared margin is inside the 10 000 bps domain")
}

// ---------------------------------------------------------------------------
// G16 — re-ask a model until it returns `pass`, at the §19.4 gate layer.
// ---------------------------------------------------------------------------

/// The i1 lead (charter recon fact 6): the gate adjudicated
/// `evaluations.last()` — a failing evaluation followed by a cleaner
/// re-run would adjudicate on the re-run alone. THE TEETH: the earlier
/// evaluation's FAILED conditions are durable evidence; a re-run cannot
/// launder them. Deep-proof citation: the real-binary weakening shape is
/// p9 leg 1; this is the re-adjudication half at the gate level.
#[test]
fn g16_a_failing_evaluation_cannot_be_laundered_by_a_rerun() {
    let world = GateWorld::new("g16-launder");
    let (revision, _delta_ref) = propose_runner_candidate(&world, "4");

    // Evaluation 1: the candidate's authored economy is FLAT — the runner
    // switch applies but the metric does not improve. Adjudicating NOW
    // would REJECT (margin_not_met) and the rejection is write-once; the
    // re-ask attacker instead re-rolls WITHOUT adjudicating.
    let flat = EconomyExecutor::new(
        world.scratch.open_state(),
        world.clock.clone(),
        Economy::Flat,
        world.runs.clone(),
    );
    evaluate_runner_candidate(&world, &revision, &flat, &[PROTECTED_CAPABILITY]);

    // Evaluation 2: the re-rolled world where the candidate genuinely
    // improves. This is the record `evaluations.last()` would adjudicate.
    let improves = EconomyExecutor::new(
        world.scratch.open_state(),
        world.clock.clone(),
        Economy::Improves,
        world.runs.clone(),
    );
    evaluate_runner_candidate(&world, &revision, &improves, &[PROTECTED_CAPABILITY]);

    // THE GATE: the failing first evaluation must not be laundered by the
    // passing second one.
    let input = world.gate_input(&revision);
    let verdict = gate_for(&world)
        .adjudicate(&revision, &input, bound())
        .expect("the adjudication completes");
    match &verdict {
        GateVerdict::Rejected { reasons, .. } => {
            assert!(
                reasons
                    .iter()
                    .any(|refusal| refusal.token() == "margin_not_met"),
                "the earlier evaluation's failed margin must be recorded: {reasons:?}"
            );
            assert_eq!(
                reasons.len(),
                1,
                "exactly the earlier evaluation's failure is recorded once — the sweep must \
                 not re-adjudicate the last record against itself: {reasons:?}"
            );
        }
        GateVerdict::Promoted(_) => panic!(
            "a failing evaluation was laundered by a passing re-run — the re-ask-until-pass \
             shape adjudicated PROMOTED: {verdict:?}"
        ),
        other => panic!("expected a rejection, got {other:?}"),
    }
}

/// The honest road the G16 correction must keep open: a PROVISIONAL
/// (evidence GAP — here, no protected set declared) followed by a
/// complete re-evaluation adjudicates on the COMPLETE record. Gaps are
/// superseded by fresher evidence; only FAILED conditions are durable
/// contrary evidence. Also pins the write-once floor: past a final state
/// the gate refuses re-adjudication entirely (AlreadyFinal).
#[test]
fn g16_the_honest_road_a_gap_then_a_complete_evaluation_still_promotes() {
    let world = GateWorld::new("g16-honest");
    let (revision, _delta_ref) = propose_runner_candidate(&world, "5");

    // Evaluation 1: everything holds but NO protected capability was
    // declared — condition 6 is UNDECIDABLE, an evidence gap.
    let improves = EconomyExecutor::new(
        world.scratch.open_state(),
        world.clock.clone(),
        Economy::Improves,
        world.runs.clone(),
    );
    evaluate_runner_candidate(&world, &revision, &improves, &[]);

    // Adjudicated now: PROVISIONAL (never promoted, never rejected), and
    // the provisional label is durably written (an evaluation exists).
    let input = world.gate_input(&revision);
    let verdict = gate_for(&world)
        .adjudicate(&revision, &input, bound())
        .expect("the adjudication completes");
    match &verdict {
        GateVerdict::Provisional { reasons, written } => {
            assert!(
                reasons
                    .iter()
                    .any(|refusal| refusal.token() == "protected_capability_inferior"),
                "the missing protected set is the recorded gap: {reasons:?}"
            );
            assert!(*written, "a provisional with an evaluation is durable");
        }
        other => panic!("expected a provisional, got {other:?}"),
    }

    // Evaluation 2: the complete declaration. The re-adjudication (legal
    // from provisional — the state machine's own rule) concludes — and
    // the promotion's figures are the AUTHORED economy derived from the
    // durable checkpoints: the margin arithmetic discriminates.
    evaluate_runner_candidate(&world, &revision, &improves, &[PROTECTED_CAPABILITY]);
    let verdict = gate_for(&world)
        .adjudicate(&revision, &input, bound())
        .expect("the re-adjudication completes");
    let GateVerdict::Promoted(evidence) = &verdict else {
        panic!(
            "the honest road stays open — a gap superseded by a complete evaluation promotes: \
             {verdict:?}"
        );
    };
    assert_eq!(
        evidence.metric, "payment_per_settlement",
        "the declared metric is what the gate judged"
    );
    assert_eq!(
        evidence.control_figure, EXPECTED_CONTROL_FIGURE,
        "the control arm's durable payment figure"
    );
    assert_eq!(
        evidence.treatment_figure, EXPECTED_TREATMENT_FIGURE,
        "the treatment arm's durable payment figure"
    );
    assert!(
        evidence.control_figure > evidence.treatment_figure,
        "the improvement is genuine, not vacuous"
    );

    // Write-once past the final state: a THIRD adjudication attempt is
    // the typed AlreadyFinal refusal — the gate's re-ask floor.
    let error = gate_for(&world)
        .adjudicate(&revision, &input, bound())
        .expect_err("write-once past a final state");
    assert!(
        matches!(
            error,
            gauntlet_app::promote::gate::GateError::AlreadyFinal { .. }
        ),
        "the promoted decision is sticky: {error:?}"
    );
}

// ---------------------------------------------------------------------------
// G5 — overfit the held-out set, at the §19.4 gate: the candidate that
// regresses the REGRESSION set is rejected even while its held-out
// metric genuinely improves.
// ---------------------------------------------------------------------------

/// THE TEETH: the gate's regression-suite condition (`regression_verdict`
/// in `crates/gauntlet-app/src/promote/gate.rs`) — every unit control
/// settled, treatment settles too. Remove it and the skipped regression
/// unit is invisible: the same fixture promotes (every other condition is
/// authored to hold), and this test's rejection assertion is RED.
#[test]
fn g5_a_regression_set_regression_is_rejected_while_the_metric_improves() {
    let world = GateWorld::new("g5-regression");
    let (revision, _delta_ref) = propose_runner_candidate(&world, "6");

    let executor = EconomyExecutor::new(
        world.scratch.open_state(),
        world.clock.clone(),
        Economy::RegressesLastRegressionUnit,
        world.runs.clone(),
    );
    evaluate_runner_candidate(&world, &revision, &executor, &[PROTECTED_CAPABILITY]);

    let input = world.gate_input(&revision);
    let verdict = gate_for(&world)
        .adjudicate(&revision, &input, bound())
        .expect("the adjudication completes");
    match &verdict {
        GateVerdict::Rejected { reasons, .. } => {
            assert_eq!(
                reasons
                    .iter()
                    .filter(|refusal| refusal.token() == "regression_suite_failed")
                    .count(),
                1,
                "exactly the regression condition failed: {reasons:?}"
            );
            assert!(
                !reasons
                    .iter()
                    .any(|refusal| refusal.token() == "margin_not_met"),
                "the held-out metric genuinely improved — the rejection is the regression \
                 condition's alone: {reasons:?}"
            );
        }
        other => panic!(
            "a candidate that skips a control-settled regression unit must be REJECTED, got \
             {other:?}"
        ),
    }
}

// ---------------------------------------------------------------------------
// G13 — optimize the score instead of settlement, at the §19.4 gate: a
// delta that weakens settlement access is REJECTED even when the target
// metric genuinely improves. Loop-economics note (recon fact 7): this
// pins the PRECEDENCE (settlement access before the margin) at the layer
// where it lives — no per-settlement-economics claim is made.
//
// Seam note (recorded): the evaluation record is appended through the
// REAL ledger port over the REAL proposal, with all record facts authored
// to the complete-arms improving shape — because the classification under
// test is the GATE's read of the DECLARED delta, and the harness seam
// refuses (honestly) to claim it can APPLY a verification-bearing key.
// The full-loop shape — a real binary rejecting a verification-weakening
// candidate that declares a gain — is p9 leg 1's settled proof, cited.
// ---------------------------------------------------------------------------

/// THE TEETH: `DeltaDocument::settlement_access_verdict` (AK-056's
/// refusal, `crates/gauntlet-app/src/promote/delta.rs`). Neutralize it
/// and the weakening delta classifies harmless: the same fixture
/// promotes (the metric improves, the arms are complete), and this
/// test's rejection assertion is RED.
#[test]
fn g13_a_metric_gain_never_outranks_weakened_settlement_access() {
    let world = GateWorld::new("g13-score");
    // The candidate's delta: the verification-bearing review mode — a
    // key whose change weakens settlement access (any value but the
    // §22 posture's preserve-value refuses).
    let delta = delta_document(&[("review.mode", "adaptive")]);
    let rollback = delta_document(&[("review.mode", "single")]);
    let delta_ref = world.pin(delta.as_bytes());
    let rollback_ref = world.pin(rollback.as_bytes());
    let revision = world.propose(&candidate_document(
        "review.mode",
        "01ARZ3NDEKTSV4RRFFQ69G5FP7",
        "01ARZ3NDEKTSV4RRFFQ69G5FA7",
        &delta_ref,
        &rollback_ref,
    ));

    // The evaluation record: complete, improving, all-deterministic arms
    // — every OTHER condition authored to hold, appended through the REAL
    // ledger port (append-only; keyed by the run-set identity).
    let record = improving_evaluation_record(&revision);
    world
        .scratch
        .open_revisions()
        .append_evaluation(record, bound())
        .expect("the evaluation record appends");

    // THE GATE: REJECTED with AK-056's reason — despite the margin.
    let input = world.gate_input(&revision);
    let verdict = gate_for(&world)
        .adjudicate(&revision, &input, bound())
        .expect("the adjudication completes");
    match &verdict {
        GateVerdict::Rejected { reasons, .. } => {
            assert_eq!(
                reasons.len(),
                1,
                "exactly the weakening refused: {reasons:?}"
            );
            assert_eq!(reasons[0].token(), "settlement_access_weakened");
        }
        other => panic!(
            "a settlement-access-weakening delta with an IMPROVED metric promoted — score \
             outranked settlement: {other:?}"
        ),
    }
}

/// The complete, improving, all-deterministic evaluation record (the
/// facts a real four-arm evaluation over the authored economy would
/// carry), for gate-level tests whose seam appends through the REAL
/// ledger port. The four run ids are distinct fixture ULIDs.
fn improving_evaluation_record(revision: &RevisionId) -> EvaluationRecord {
    const RUN_CONTROL: &str = "01ARZ3NDEKTSV4RRFFQ69G5E01";
    const RUN_TREATMENT: &str = "01ARZ3NDEKTSV4RRFFQ69G5E02";
    const RUN_REGRESSION_CONTROL: &str = "01ARZ3NDEKTSV4RRFFQ69G5E03";
    const RUN_REGRESSION_TREATMENT: &str = "01ARZ3NDEKTSV4RRFFQ69G5E04";
    let run = |stem: &str| gauntlet_domain::RunId::new(stem).expect("the fixture run id is a ULID");
    let metrics = |per_unit_micros: u64| RunMetrics {
        iterations: u64::from(CHECKPOINT_ITERATION),
        payment_tokens: UNIT_TOKENS * GATE_UNITS.len() as u64,
        payment_micro_usd: per_unit_micros * GATE_UNITS.len() as u64,
        settlements: GATE_UNITS
            .iter()
            .map(|unit| SettlementFact {
                unit: (*unit).to_owned(),
                instrument_deterministic: Some(true),
                occurred_at: Some(gauntlet_domain::EpochSeconds::new(BASE_INSTANT + 10)),
            })
            .collect(),
        dispatched_units: GATE_UNITS.iter().map(|unit| (*unit).to_owned()).collect(),
        settled_terminal: true,
    };
    EvaluationRecord {
        revision: revision.clone(),
        metric: TargetMetric::PaymentPerSettlement.token().to_owned(),
        protected: Vec::new(),
        held_out_control: run(RUN_CONTROL),
        held_out_treatment: run(RUN_TREATMENT),
        regression_control: run(RUN_REGRESSION_CONTROL),
        regression_treatment: run(RUN_REGRESSION_TREATMENT),
        held_out_treatment_parent: run(RUN_CONTROL),
        regression_treatment_parent: run(RUN_REGRESSION_CONTROL),
        control_reference_digest: gate_reference_digest(),
        treatment_reference_digest: gate_reference_digest(),
        regression_control_reference_digest: gate_reference_digest(),
        regression_treatment_reference_digest: gate_reference_digest(),
        control_held_out_metrics: metrics(EXPENSIVE_UNIT_COST_MICROS),
        treatment_held_out_metrics: metrics(ECONOMY_UNIT_COST_MICROS),
        control_regression_metrics: metrics(EXPENSIVE_UNIT_COST_MICROS),
        treatment_regression_metrics: metrics(ECONOMY_UNIT_COST_MICROS),
        at: gauntlet_domain::EpochSeconds::new(BASE_INSTANT + 20),
    }
}

/// Residual E-2 (task23 closure; trigger: next gate touch — task24
/// touched the gate): the by-unit instrument join used to take the FIRST
/// treatment settlement row per unit — order-sensitive under duplicated
/// unit rows (whichever row sorted first decided, so an ordering-
/// controlled row could shield a weakened counterpart).
///
/// THE TEETH: `instrument_series_verdict`
/// (`crates/gauntlet-app/src/promote/gate.rs`), the any-weaker join.
/// A treatment arm carrying TWO rows for one control-settled unit — one
/// weakened, one deterministic — must adjudicate `Weakened` (condition 3
/// failed) in BOTH orders. Neutralize the any-weaker join back to
/// first-match and the weakened-ROW-LAST ordering goes RED: the gate
/// adjudicates on the deterministic row alone and the weakened
/// counterpart is shielded by its position.
#[test]
fn the_by_unit_instrument_join_is_order_independent_under_duplicated_rows() {
    for (label, weakened_first) in [("weakened-first", true), ("weakened-last", false)] {
        let world = GateWorld::new(&format!("e2-join-{label}"));
        let (revision, _delta_ref) = propose_runner_candidate(&world, "8");

        // The otherwise-complete, improving, all-deterministic record;
        // the treatment held-out arm's FIRST unit gains a SECOND
        // settlement row — the duplicated unit carries one weakened and
        // one deterministic instrument, in this ordering's arrangement.
        let mut record = improving_evaluation_record(&revision);
        let weakened_row = SettlementFact {
            unit: GATE_UNITS[0].to_owned(),
            instrument_deterministic: Some(false),
            occurred_at: Some(gauntlet_domain::EpochSeconds::new(BASE_INSTANT + 11)),
        };
        let deterministic_row = SettlementFact {
            unit: GATE_UNITS[0].to_owned(),
            instrument_deterministic: Some(true),
            occurred_at: Some(gauntlet_domain::EpochSeconds::new(BASE_INSTANT + 12)),
        };
        let duplicated = if weakened_first {
            vec![weakened_row, deterministic_row]
        } else {
            vec![deterministic_row, weakened_row]
        };
        let mut settlements = record.treatment_held_out_metrics.settlements.clone();
        let tail = settlements.split_off(1);
        record.treatment_held_out_metrics.settlements = [duplicated, tail].concat();

        world
            .scratch
            .open_revisions()
            .append_evaluation(record, bound())
            .expect("the evaluation record appends");

        // THE GATE: the weakened counterpart is never shielded by
        // position — exactly the weakened series refused, in both
        // orders.
        let input = world.gate_input(&revision);
        let verdict = gate_for(&world)
            .adjudicate(&revision, &input, bound())
            .expect("the adjudication completes");
        match &verdict {
            GateVerdict::Rejected { reasons, .. } => {
                assert_eq!(
                    reasons.len(),
                    1,
                    "exactly the weakened instrument series is refused ({label}): {reasons:?}"
                );
                assert_eq!(
                    reasons[0].token(),
                    "settlement_access_weakened",
                    "the duplicated unit's weakened row decides ({label})"
                );
            }
            other => panic!(
                "a duplicated-unit treatment row shielded its weakened counterpart by \
                 position ({label}) — the by-unit instrument join adjudicated {other:?}"
            ),
        }
    }
}

// ---------------------------------------------------------------------------
// Shared fixture for the provider-level tests (G1's contract parser, G2 /
// G10's reference provider, G6's bundle digest, G8's aggregation, G11's
// blindness): small, real, per-test scratch.
// ---------------------------------------------------------------------------

struct Scratch {
    path: PathBuf,
}

impl Scratch {
    fn new(tag: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("gauntlet-antigaming-{tag}-{}", std::process::id()));
        std::fs::remove_dir_all(&path).ok();
        std::fs::create_dir_all(&path).expect("the scratch parent is creatable");
        Self { path }
    }

    fn child(&self, name: &str) -> PathBuf {
        let path = self.path.join(name);
        std::fs::create_dir_all(&path).expect("the scratch child is creatable");
        path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

const FIXTURE_RUN_ID: &str = "01ARZ3NDEKTSV4RRFFQ69G5EB1";
const G1_VALID_CONTRACT: &str = concat!(
    "version: 1\n",
    "dimensions:\n",
    "  - functional behavior\n",
    "severity_rubric:\n",
    "  - blocking | invalidates the iteration\n",
    "  - major | real, not invalidating\n",
    "  - minor | small\n",
    "  - informational | recorded only\n",
    "comparator_receives:\n",
    "  - the allowlist members\n",
    "residual_definition:\n",
    "  - a difference\n",
);
const KEY_ORIGINAL: &str = "claim: the unit's behavioral claim holds.\nverify_by: static_check\n";
const KEY_MUTATED: &str = "claim: the WEAKENED claim holds.\nverify_by: none\n";
const MANIFEST_TEXT: &str = "reference:\n  id: ref-antigaming-suite\n  version: 1\n  \
     purpose: >\n    One paragraph.\n  desired_state: >\n    The destination.\n  settlement:\n    \
     success_criteria:\n      - the destination is reached\n    minimum_reliability: >\n      \
     spec 19.2 default: >= 1 settlements before promotion.\n    terminal_conditions: \
     [settled, aborted]\n";

// ---------------------------------------------------------------------------
// G1 — weaken the acceptance criteria: the comparison-contract parser
// refuses a contract that cannot grade.
// ---------------------------------------------------------------------------

/// THE TEETH: the rubric-coverage and structure validation in
/// `ComparisonContract::parse` (`crates/gauntlet-app/src/compare/
/// contract.rs`). A contract missing a severity can never assign that
/// severity to a residual; one that compares nothing can never compare.
/// Neutralize the validation and every refused shape below parses — the
/// weakened standard enters the loop, and this test is RED.
#[test]
fn g1_a_weakened_comparison_contract_is_refused_at_the_boundary() {
    let contract =
        ComparisonContract::parse(G1_VALID_CONTRACT).expect("the complete contract parses");
    assert_eq!(contract.dimensions(), &["functional behavior".to_owned()]);

    // Dropping a severity from the rubric weakens the standard: a
    // residual graded `major` would be unassignable.
    let no_major = G1_VALID_CONTRACT.replace("  - major | real, not invalidating\n", "");
    let error = ComparisonContract::parse(&no_major)
        .expect_err("a rubric without every severity is refused");
    assert!(
        matches!(error, ContractError::RubricInvalid { .. }),
        "the missing severity is a typed refusal: {error:?}"
    );

    // A contract that instantiates no dimension compares nothing.
    let no_dimensions = G1_VALID_CONTRACT.replace("  - functional behavior\n", "");
    let error = ComparisonContract::parse(&no_dimensions)
        .expect_err("a contract that compares nothing is refused");
    assert!(
        matches!(error, ContractError::RubricInvalid { .. }),
        "the empty dimension set is a typed refusal: {error:?}"
    );

    // An unsupported version is never partially adopted.
    let future = G1_VALID_CONTRACT.replace("version: 1", "version: 2");
    let error = ComparisonContract::parse(&future).expect_err("an unknown version is refused");
    assert!(
        matches!(error, ContractError::VersionUnsupported { .. }),
        "the unsupported version is a typed refusal: {error:?}"
    );
}

// ---------------------------------------------------------------------------
// G2 — change the reference mid-run: the provider's I1 tripwire fires
// with BOTH digests when the frozen artifact moves after the freeze.
// (The composed mid-run shape — resume re-freeze-verify aborting the run
// — is g7 leg 3's settled proof; p1 pins the builder-mutation leg.)
// ---------------------------------------------------------------------------

/// THE TEETH: `FileReferenceProvider::verify_unchanged`'s digest
/// comparison (`crates/gauntlet-app/src/reference/provider.rs`). Make it
/// always `Ok` and the moved ruler below goes unnoticed — this test is
/// RED.
#[test]
fn g2_a_reference_changed_after_the_freeze_fires_the_mutation_tripwire() {
    let scratch = Scratch::new("g2-tripwire");
    let key_path = scratch.child("ruler").join("answer-key.md");
    std::fs::write(&key_path, KEY_ORIGINAL).expect("the key is written");
    let provider = FileReferenceProvider::from_sources(
        AnswerKeySource::Path(key_path.clone()),
        ManifestSource::Inline(MANIFEST_TEXT.to_owned()),
        Rc::new(
            FsEvidenceStore::open(&scratch.child("evidence"))
                .expect("the real evidence store opens"),
        ),
    );
    let run_id = RunId::new(FIXTURE_RUN_ID).expect("the fixture run id is a ULID");
    let draft = provider.resolve("the g2 proof", bound()).expect("resolves");
    let reference = provider.normalize(draft, bound()).expect("normalizes");
    let version = provider
        .freeze(&run_id, &reference, bound())
        .expect("freezes");
    provider
        .verify_unchanged(&version, bound())
        .expect("an unmoved artifact is unchanged");

    // THE ATTACK: the reference changes MID-RUN, after the freeze.
    std::fs::write(&key_path, KEY_MUTATED).expect("the attacker edit lands");
    match provider.verify_unchanged(&version, bound()) {
        Err(ReferenceProviderError::ReferenceMutated { expected, actual }) => {
            assert_eq!(
                &expected,
                version.digest(),
                "the expected digest is the frozen one"
            );
            assert_ne!(
                &actual,
                version.digest(),
                "the actual digest is the moved one"
            );
        }
        other => panic!("a moved ruler must fire the I1 tripwire, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// G10 — let the builder write its own answer key (detection half): the
// builder's raw write lands through the same filesystem, and the runtime
// refuses to FREEZE the tampered standard — the same file-derived digest
// consult the settle path performs (§9.4). The composed prevention leg
// (the effect-surface locator refusal) and the settle-time detection leg
// are g10_builder_writes_answer_key.rs's settled proof, cited.
// ---------------------------------------------------------------------------

/// THE TEETH: the freeze-boundary digest guard in
/// `FileReferenceProvider::freeze` (`crates/gauntlet-app/src/reference/
/// provider.rs`) — the mutation-between-normalize-and-freeze tripwire.
/// Remove it and the tampered key freezes as the run's standard: this
/// test is RED.
#[test]
fn g10_a_builder_rewrite_of_the_answer_key_is_refused_at_freeze() {
    let scratch = Scratch::new("g10-freeze");
    let key_path = scratch.child("ruler").join("answer-key.md");
    std::fs::write(&key_path, KEY_ORIGINAL).expect("the key is written");
    let provider = FileReferenceProvider::from_sources(
        AnswerKeySource::Path(key_path.clone()),
        ManifestSource::Inline(MANIFEST_TEXT.to_owned()),
        Rc::new(
            FsEvidenceStore::open(&scratch.child("evidence"))
                .expect("the real evidence store opens"),
        ),
    );
    let run_id = RunId::new(FIXTURE_RUN_ID).expect("the fixture run id is a ULID");
    let draft = provider
        .resolve("the g10 proof", bound())
        .expect("resolves");
    let reference = provider.normalize(draft, bound()).expect("normalizes");

    // THE ATTACK: the builder-context raw write — the exact shape every
    // builder-context process could perform — lands BEFORE the freeze.
    std::fs::write(&key_path, KEY_MUTATED).expect("the builder write lands");
    match provider.freeze(&run_id, &reference, bound()) {
        Err(ReferenceProviderError::ReferenceMutated { expected, actual }) => {
            assert_eq!(&expected, reference.digest());
            assert_ne!(&actual, reference.digest());
        }
        other => panic!("a tampered standard must never freeze as the run's ruler, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// G6 — hide failed observations: the observation digest covers BOTH gap
// arrays, so hiding a failure changes the digest (tamper-evidence).
// (The composed assembler proof — gap arrays recorded by the real
// observer chain — is g6_hide_failed_observations.rs, cited.)
// ---------------------------------------------------------------------------

/// THE TEETH: the digest normalization's coverage rule
/// (`crates/gauntlet-ports/src/observation_digest.rs`,
/// `normalized_bytes`) — everything but timestamps is covered. Exempt
/// the gap arrays from the normalization and the two digests below
/// coincide: this test is RED.
#[test]
fn g6_a_hidden_failed_observation_changes_the_observation_digest() {
    let healthy = g6_bundle(Vec::new());
    let failing = g6_bundle(vec![ObservationFailureRecord::new(
        FacetKind::Test.token().to_owned(),
        "instrument_failed".to_owned(),
        "the instrument exited without a report".to_owned(),
    )]);

    // The digest is timestamp-free (the normalization rule pinned) —
    // BOTH the document instant and each facet's window.
    let mut shifted = healthy.content();
    shifted.observed_at = BASE_INSTANT + 4_000;
    shifted.facets = vec![g6_facet_record_at(
        BASE_INSTANT + 4_000,
        BASE_INSTANT + 4_003,
    )];
    assert_eq!(
        observation_digest(&ObservationBundleDoc::new(shifted)),
        observation_digest(&healthy),
        "the same observation re-taken later digests identically"
    );

    // THE ATTACK: the failure is hidden — the gap array emptied. THE
    // DIGEST MUST MOVE: the tamper is detectable everywhere the digest
    // is re-derived (the comparison envelope refuses a bundle whose
    // declared digest does not match its contents).
    assert_ne!(
        observation_digest(&failing),
        observation_digest(&healthy),
        "a hidden failure changes the digest — tamper-evidence"
    );
    assert!(
        normalized_bytes(&failing).contains("instrument_failed"),
        "the gap arrays are inside the normalized (digested) bytes"
    );
    assert!(
        normalized_bytes(&healthy).contains("gauntlet-antigaming-probe"),
        "facet content survives the normalization (it is digested, not dropped)"
    );
}

fn g6_bundle(failures: Vec<ObservationFailureRecord>) -> ObservationBundleDoc {
    let facet = g6_facet_record_at(BASE_INSTANT, BASE_INSTANT + 3);
    ObservationBundleDoc::new(BundleContent {
        run_id: RunId::new(FIXTURE_RUN_ID).expect("the fixture run id is a ULID"),
        iteration_no: IterationNo::new(1).expect("iterations are 1-based"),
        observed_at: BASE_INSTANT,
        project_revision: "rev-antigaming-01".to_owned(),
        environment_fingerprint: "sha256:0123abcd".to_owned(),
        observation_digest: Digest::new("sha256", &"ab".repeat(32)).expect("digest"),
        facets: vec![facet],
        claims_facet: ClaimsFacet::new("the artifact passes its own self-check".to_owned()),
        unavailable_evidence: Vec::new(),
        observation_failures: failures,
    })
}

/// One observed facet whose window sits at the given instants.
fn g6_facet_record_at(started: u64, ended: u64) -> FacetRecord {
    let instrument = InstrumentIdentity::new(
        InstrumentName::new("gauntlet-antigaming-probe").expect("the instrument name is valid"),
        "1".to_owned(),
    );
    let evidence = EvidenceRef::new("evd-antigaming-g6").expect("the evidence ref is valid");
    FacetRecord::from_facet(
        &(ObservationFacet::new(FacetKind::Test, instrument, evidence)
            .with_invocation(Some("probe".to_owned()), Some(1))
            .with_window(
                Some(gauntlet_domain::EpochSeconds::new(started)),
                Some(gauntlet_domain::EpochSeconds::new(ended)),
            )
            .with_summary(Some("one probe, observed".to_owned()))),
    )
}

// ---------------------------------------------------------------------------
// G7 — downgrade a unit's blast radius or authority to unblock: the
// lattice review refuses ANY agent-originated movement and every
// operator decrease. (The dispatch-gate conjunct and the composed
// frozen-ruler leg are g7_tier_downgrade.rs, cited.)
// ---------------------------------------------------------------------------

/// THE TEETH: `review_lattice_movement` (`crates/gauntlet-app/src/
/// govern/posture.rs`). Make it permissive and the agent downgrade below
/// is ALLOWED — this test is RED.
#[test]
fn g7_an_agent_role_cannot_downgrade_a_units_tier() {
    let builder = ChangeOrigin::AgentRole(RoleId::Builder);
    let critic = ChangeOrigin::AgentRole(RoleId::Critic);
    let operator = ChangeOrigin::Operator;
    let recorded = (BlastRadius::Local, Authority::Critic);

    // The containment-escape direction: an agent DECREASING the tier.
    let refused = review_lattice_movement(
        UnitId::new("AK-9501").expect("unit id"),
        recorded,
        (BlastRadius::Contained, Authority::Critic),
        &builder,
    );
    match &refused {
        LatticeMovement::Refused {
            blast_movement,
            rule,
            ..
        } => {
            assert_eq!(
                blast_movement,
                &Some((BlastRadius::Local, BlastRadius::Contained)),
                "the refusal carries the recorded and proposed tiers"
            );
            assert!(
                rule.contains("agent role") && rule.contains("AK-046"),
                "the recorded rule is the claim itself: {rule}"
            );
        }
        other => panic!("an agent tier downgrade must be refused, got {other:?}"),
    }

    // An agent RAISE is refused too (raises are operator-only); a critic
    // proposing no movement is always allowed.
    assert!(
        !review_lattice_movement(
            UnitId::new("AK-9502").expect("unit id"),
            recorded,
            (BlastRadius::CrossCutting, Authority::LeadAgent),
            &builder,
        )
        .is_allowed(),
        "only an operator can raise (AK-046's second clause)"
    );
    assert!(
        review_lattice_movement(
            UnitId::new("AK-9503").expect("unit id"),
            recorded,
            recorded,
            &critic,
        )
        .is_allowed(),
        "no movement is always allowed"
    );

    // The operator may raise either axis and may NEVER lower: the
    // runtime has no downward path at all.
    assert!(
        review_lattice_movement(
            UnitId::new("AK-9504").expect("unit id"),
            recorded,
            (BlastRadius::CrossCutting, Authority::LeadAgent),
            &operator,
        )
        .is_allowed()
    );
    assert!(
        !review_lattice_movement(
            UnitId::new("AK-9505").expect("unit id"),
            recorded,
            (BlastRadius::Contained, Authority::Critic),
            &operator,
        )
        .is_allowed(),
        "an operator decrease is refused too — the runtime never lowers"
    );

    // The authority axis is under the same never-lowers rule: an operator
    // decrease confined to the authority axis (lead_agent -> critic, the
    // blast radius untouched) is refused as well.
    assert!(
        !review_lattice_movement(
            UnitId::new("AK-9506").expect("unit id"),
            (BlastRadius::Local, Authority::LeadAgent),
            (BlastRadius::Local, Authority::Critic),
            &operator,
        )
        .is_allowed(),
        "an operator authority decrease is refused too — the runtime never lowers"
    );
}

// ---------------------------------------------------------------------------
// G8 — drop hard residuals: §13.5's deterministic aggregation — a
// blocking residual forces FAIL, whatever the contributor's verdict intent
// says, and NO model sits in the merge path. Exercised through the only
// public route to the merger — the governed comparison chain
// (`ComparisonAuthority` → `GovernedRun::issue` → `review`), whose fan-in
// calls `compare::aggregate` (crate-private on purpose; the deep
// in-module teeth are the aggregate unit tests).
// ---------------------------------------------------------------------------

/// THE TEETH: the verdict rule in `compare::aggregate` (`crates/
/// gauntlet-app/src/compare/aggregate.rs` — `blocking || any_failed_gate →
/// Fail`). Drop `blocking` from the rule and the blocked contributor below
/// aggregates to a PASS (its verdict intent, one undisputed residual) —
/// this test's Fail assertion is RED.
#[test]
fn g8_a_blocking_residual_forces_the_aggregate_verdict_to_fail() {
    // The blocked world: one scripted critic whose document carries a
    // BLOCKING residual under a PASS verdict intent — the exact shape a
    // score-optimizing critic would produce while hiding a hard failure.
    let blocked = g8_review(G8ResidualShape::BlockingUnderPassIntent);
    let record = blocked.outcome.record().expect("the review aggregates");
    assert_eq!(
        record.verdict().status(),
        Verdict::Fail,
        "one blocking residual forces fail regardless of the verdict intent"
    );
    assert_eq!(
        record.residuals()[0].severity(),
        Severity::Blocking,
        "the blocking severity survives the merge (never averaged away)"
    );
    assert_eq!(
        record.identity().model(),
        "",
        "no model in the merge path (§13.5's deterministic rule)"
    );

    // The positive control: the same composed world with a clean critic
    // passes — the refusal above is the blocking residual's work alone.
    let clean = g8_review(G8ResidualShape::CleanPass);
    let record = clean.outcome.record().expect("the review aggregates");
    assert_eq!(record.verdict().status(), Verdict::Pass);
}

/// What the scripted critic's contributor document carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum G8ResidualShape {
    /// A BLOCKING residual under a PASS verdict intent.
    BlockingUnderPassIntent,
    /// No residuals, PASS — the positive control.
    CleanPass,
}

/// The composed review world: a durable run over the real ruler, a real
/// bound observation bundle, the real governed comparison chain, and one
/// scripted critic contributor (a proof-local `Comparator` — the review
/// source seam, the same insertion point a model adapter occupies; its
/// document flows through the SAME mandatory-acceptance/aggregation path
/// a dispatched critic's does).
struct G8ReviewWorld {
    _scratch: Scratch,
    outcome: gauntlet_app::compare::authority::AcceptedComparison,
}

fn g8_review(shape: G8ResidualShape) -> G8ReviewWorld {
    use gauntlet_ports::budget_meter::BudgetSlice;
    use gauntlet_ports::comparator::{
        Comparator, ComparatorError, ComparatorSource, TrustedComparisonContext,
    };
    use gauntlet_ports::producer_authority::ObservationBindingAuthority;

    const ITERATION_ONE: u32 = 1;
    const CRITIC_ROUNDS: u32 = 1;
    const CRITIC_WALL: u64 = 30;
    const CRITIC_TOKENS: u64 = 200;
    const CRITIC_COST_MICROS: u64 = 50;
    const METER_ROUNDS: u32 = 4;
    const METER_PARALLEL: u32 = 2;
    const METER_DEPTH: u32 = 2;
    const METER_WALL: u64 = 3_600;
    const METER_TOKENS: u64 = 2_000_000;
    const METER_COST_MICROS: u64 = 25 * 1_000_000;
    const METER_RESERVE_PCT: u8 = 15;

    let scratch = Scratch::new("g8-review");
    let store = Arc::new(FsEvidenceStore::open(&scratch.child("evidence")).expect("opens"));
    let state =
        Arc::new(EmbeddedStateStore::open(&scratch.child("state").join("g8.db")).expect("opens"));

    // The bundle: a real, self-digest-consistent observation document,
    // stored and durably bound as the run's observation at this iteration.
    let bundle_content = g6_bundle(Vec::new()).content();
    let mut consistent = bundle_content.clone();
    consistent.observation_digest = observation_digest(&ObservationBundleDoc::new(bundle_content));
    let bundle = ObservationBundleDoc::new(consistent);
    let bundle_digest = bundle.observation_digest().clone();
    let bundle_ref = store
        .put(
            bundle.to_json().as_bytes(),
            gauntlet_domain::EvidenceKind::File,
            bound(),
        )
        .expect("the bundle bytes store");
    let run_id = RunId::new(FIXTURE_RUN_ID).expect("the fixture run id is a ULID");
    let iteration = IterationNo::new(ITERATION_ONE).expect("iterations are 1-based");

    // The frozen ruler: the repository's real answer key, read-only,
    // normalized through the production path and pinned to the run (the
    // p2 composition).
    let raw = std::fs::read(g8_answer_key_path()).expect("the frozen answer key is readable");
    let normalized = normalize_bytes(&raw).expect("the key normalizes");
    let digest = digest_normalized(&normalized);
    let reference_evidence = store
        .put(&normalized, gauntlet_domain::EvidenceKind::File, bound())
        .expect("frozen evidence stores");
    store
        .pin(&run_id, &reference_evidence, bound())
        .expect("frozen evidence pins");
    let reference_version = gauntlet_domain::ReferenceVersion::with_evidence(
        gauntlet_domain::ReferenceId::new(REFERENCE_ID).expect("reference id"),
        1,
        digest,
        reference_evidence,
    );
    let durable_run = gauntlet_domain::Run::new(gauntlet_domain::RunDefinition {
        id: run_id.clone(),
        created_at: gauntlet_domain::EpochSeconds::new(BASE_INSTANT),
        reference_version,
        intent: INTENT.to_owned(),
        config_fingerprint: Digest::new("sha256", &"ab".repeat(32)).expect("digest"),
        environment_fingerprint: Digest::new("sha256", &"cd".repeat(32)).expect("digest"),
        budget: gauntlet_domain::Budget::new(
            gauntlet_domain::Limits::new(
                METER_ROUNDS,
                METER_PARALLEL,
                METER_DEPTH,
                METER_WALL,
                METER_TOKENS,
                gauntlet_domain::MicroUsd::new(METER_COST_MICROS),
            )
            .expect("limits"),
            METER_RESERVE_PCT,
            gauntlet_domain::DiagnosticAllowance::new(
                CRITIC_TOKENS,
                gauntlet_domain::MicroUsd::new(CRITIC_COST_MICROS),
            ),
        )
        .expect("budget"),
        parent_run: None,
    })
    .expect("the durable run is valid");
    state
        .record_run(&durable_run, bound())
        .expect("the run is durable before any comparison authority exists");
    ObservationBindingAuthority::new(state.clone(), store.clone())
        .bind(&run_id, iteration, &bundle_ref, bound())
        .expect("the observed bundle binds durably");

    let contract = ComparisonContract::parse(G1_VALID_CONTRACT).expect("the contract parses");
    let issued = ComparisonAuthority::new(state.clone(), store.clone())
        .for_run(&run_id, iteration, bound())
        .expect("the durable run yields comparison authority")
        .issue(&contract, Vec::new(), Vec::new(), bound())
        .expect("the envelope is issued at the composition root");

    // The scripted critic contributor: its document rides the SAME
    // acceptance/aggregation path a dispatched critic's does.
    #[derive(Clone)]
    struct ScriptedCritic {
        shape: G8ResidualShape,
        bundle_digest: Digest,
    }
    impl Comparator for ScriptedCritic {
        fn compare(
            &self,
            _: &TrustedComparisonContext,
            _: Deadline,
        ) -> Result<gauntlet_domain::ComparisonRecord, ComparatorError> {
            Err(ComparatorError::ComparisonFailed {
                detail: "the g8 fixture's critic documents natively; the summary path is \
                         never reached"
                    .to_owned(),
            })
        }
        fn comparison_document(
            &self,
            context: &TrustedComparisonContext,
            _: Deadline,
        ) -> Result<Option<ComparisonRecordDoc>, ComparatorError> {
            let envelope = context.envelope();
            let residuals = match self.shape {
                G8ResidualShape::BlockingUnderPassIntent => {
                    vec![g8_residual("res-g8-blocking", Severity::Blocking)]
                }
                G8ResidualShape::CleanPass => Vec::new(),
            };
            let verdict = match self.shape {
                G8ResidualShape::BlockingUnderPassIntent => Verdict::Pass,
                G8ResidualShape::CleanPass => Verdict::Pass,
            };
            Ok(Some(ComparisonRecordDoc::new(RecordContent {
                run_id: envelope.run_id().clone(),
                iteration_no: envelope.iteration_no(),
                reference_digest: envelope.reference_digest().clone(),
                observation_digest: self.bundle_digest.clone(),
                envelope_digest: envelope.envelope_digest().clone(),
                contract_version: envelope.contract().version().to_owned(),
                identity: ComparatorIdentity::new(
                    "g8-scripted-critic".to_owned(),
                    String::new(),
                    "all".to_owned(),
                ),
                residuals,
                hard_gate_results: Vec::new(),
                verdict: RecordVerdict::new(
                    verdict,
                    "the g8 fixture's contributor verdict".to_owned(),
                ),
            })))
        }
    }
    struct ScriptedSource {
        critic: Arc<ScriptedCritic>,
    }
    impl ComparatorSource for ScriptedSource {
        fn across_dimensions(&self) -> Result<Box<dyn Comparator>, String> {
            Ok(Box::new((*self.critic).clone()))
        }
        fn for_dimension(&self, _: &str) -> Result<Box<dyn Comparator>, String> {
            Ok(Box::new((*self.critic).clone()))
        }
    }
    let source = ScriptedSource {
        critic: Arc::new(ScriptedCritic {
            shape,
            bundle_digest: bundle_digest.clone(),
        }),
    };

    // The never-reached deterministic source (Single mode dispatches only
    // the model); a reached probe fails loudly (the g7 UnusedDiff idiom).
    struct UnusedDeterministic;
    impl ComparatorSource for UnusedDeterministic {
        fn across_dimensions(&self) -> Result<Box<dyn Comparator>, String> {
            Err("the g8 fixture's single-mode review never probes deterministically".to_owned())
        }
        fn for_dimension(&self, _: &str) -> Result<Box<dyn Comparator>, String> {
            Err("the g8 fixture's single-mode review never probes deterministically".to_owned())
        }
    }

    let meter_budget = gauntlet_domain::Budget::new(
        gauntlet_domain::Limits::new(
            METER_ROUNDS,
            METER_PARALLEL,
            METER_DEPTH,
            METER_WALL,
            METER_TOKENS,
            gauntlet_domain::MicroUsd::new(METER_COST_MICROS),
        )
        .expect("limits"),
        METER_RESERVE_PCT,
        gauntlet_domain::DiagnosticAllowance::new(
            CRITIC_TOKENS,
            gauntlet_domain::MicroUsd::new(CRITIC_COST_MICROS),
        ),
    )
    .expect("budget");
    let meter = MeteredBudget::new(meter_budget);
    let deps = ReviewDeps {
        deterministic: &UnusedDeterministic,
        model: &source,
        adversarial: None,
        meter: &meter,
        critic_slice: BudgetSlice {
            rounds: CRITIC_ROUNDS,
            wall_seconds: CRITIC_WALL,
            tokens: CRITIC_TOKENS,
            cost_usd: gauntlet_domain::MicroUsd::new(CRITIC_COST_MICROS),
        },
    };
    let outcome = issued
        .review(
            &deps,
            gauntlet_domain::ReviewMode::Single,
            ModelDispatchBudget::new(1),
            bound(),
            gauntlet_domain::EpochSeconds::new(BASE_INSTANT),
        )
        .expect("the single-critic review completes")
        .accept(bound())
        .expect("the accepted comparison");
    G8ReviewWorld {
        _scratch: scratch,
        outcome,
    }
}

fn g8_answer_key_path() -> PathBuf {
    use gauntlet_app::config::defaults::{ANSWER_KEY_NAME, WAYFINDER_DIR_NAME};
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("the cli crate sits two levels under the repository root")
        .join(WAYFINDER_DIR_NAME)
        .join(ANSWER_KEY_NAME)
}

fn g8_residual(id: &str, severity: Severity) -> ResidualDoc {
    ResidualDoc::new(
        ResidualId::new(id).expect("the residual id is valid"),
        "functional behavior".to_owned(),
        vec![],
        "tests pass".to_owned(),
        "facet:test exit 1".to_owned(),
        vec![EvidenceRef::new("evd-antigaming-g8").expect("the evidence ref is valid")],
        severity,
        0.5,
        false,
        String::new(),
        String::new(),
        vec![],
    )
}

// ---------------------------------------------------------------------------
// G11 — share context between builder and critic: the serialized
// comparison envelope is STRUCTURALLY blind — every deny-listed builder
// field is refused over the bytes themselves. (The composed envelope
// proof — the built envelope's bytes against a real builder-side canary —
// is p2_context_isolation.rs, cited.)
// ---------------------------------------------------------------------------

/// THE TEETH: `assert_blind` over `DENY_LISTED_FIELD_NAMES`
/// (`crates/gauntlet-ports/src/comparator.rs`). Empty the deny list and
/// every refusal below disappears — this test is RED.
#[test]
fn g11_the_comparison_envelope_is_structurally_blind_to_builder_context() {
    // Every deny-listed field, injected as an object field into
    // otherwise-blind bytes, is refused — the injected field is never
    // legitimate (the frozen reference may QUOTE a prohibited word; an
    // object FIELD may not exist).
    for field in DENY_LISTED_FIELD_NAMES {
        let poisoned = format!(r#"{{"reference_view":"the frozen standard","{field}":1}}"#);
        let violation =
            assert_blind(&poisoned).expect_err("an injected builder field must refuse the bytes");
        assert_eq!(violation.field, *field, "the violation names the field");
    }
    // A deny-listed field nested inside ARRAYS is found too.
    let violation = assert_blind(r#"[[{"builder_reasoning":"canary"}]]"#)
        .expect_err("a deny-listed field inside an array must refuse the bytes");
    assert_eq!(violation.field, "builder_reasoning");
    // The allowlist shape passes.
    assert_blind(r#"{"reference_view":"the frozen standard"}"#)
        .expect("a blind envelope's bytes carry no deny-listed field");
    // Text that merely ORIGINATES with a builder, inside an
    // instrument-produced VALUE, is not contraband — the check parses
    // structure, never prose.
    assert_blind(r#"{"reference_view":"quoted: builder_reasoning was a word here"}"#)
        .expect("a quoted word inside a value is not an injected field");
}

// ---------------------------------------------------------------------------
// Stage-3 shared substrate: the earned run (p10 pattern — identity,
// checkpoint, authority-minted PASS record, authority-admitted §16.3
// settlement commit), the developmental import, and the promotion
// service over the REAL embedded-SQL ledgers.
// ---------------------------------------------------------------------------

const LEDGER_BASE_INSTANT: u64 = 1_700_000_000;
const LEDGER_CHECKPOINT_ITERATION: u32 = 1;
const LEDGER_SPENT_ROUNDS: u32 = 1;
const LEDGER_SPENT_WALL_SPAN: u64 = 30;
const LEDGER_SPENT_TOKENS: u64 = 400;
const LEDGER_SPENT_COST_MICROS: u64 = 5;
const LEDGER_BUDGET_ROUNDS: u32 = 8;
const LEDGER_BUDGET_PARALLEL: u32 = 2;
const LEDGER_BUDGET_DEPTH: u32 = 1;
const LEDGER_BUDGET_WALL_SPAN: u64 = 3_600;
const LEDGER_BUDGET_TOKENS: u64 = 2_000_000;
const LEDGER_BUDGET_COST_MICROS: u64 = 500_000;
const LEDGER_RESERVE_PCT: u8 = 15;
const LEDGER_DIAGNOSTIC_TOKENS: u64 = 100;
const LEDGER_DIAGNOSTIC_COST_MICROS: u64 = 50;
const LEDGER_REFERENCE_DIGEST: &str = "sha256:1a2b3c4d5e6f1a2b";
const LEDGER_INSTRUMENT: &str = "gauntlet-antigaming-ledger-instrument";

fn ledger_reference_digest() -> Digest {
    Digest::parse(LEDGER_REFERENCE_DIGEST).expect("the fixture digest is well formed")
}

/// A durable run identity, recorded through the store's own door.
fn ledger_durable_run(state: &Arc<EmbeddedStateStore>, run_id: &RunId) {
    let run = gauntlet_domain::Run::new(gauntlet_domain::RunDefinition {
        id: run_id.clone(),
        created_at: gauntlet_domain::EpochSeconds::new(LEDGER_BASE_INSTANT),
        reference_version: gauntlet_domain::ReferenceVersion::new(
            gauntlet_domain::ReferenceId::new(REFERENCE_ID).expect("reference id"),
            1,
            ledger_reference_digest(),
        ),
        intent: INTENT.to_owned(),
        config_fingerprint: Digest::new("sha256", "0f1e2d3c4b5a6978").expect("digest"),
        environment_fingerprint: Digest::new("sha256", "78695a4b3c2d1e0f").expect("digest"),
        budget: gauntlet_domain::Budget::new(
            gauntlet_domain::Limits::new(
                LEDGER_BUDGET_ROUNDS,
                LEDGER_BUDGET_PARALLEL,
                LEDGER_BUDGET_DEPTH,
                LEDGER_BUDGET_WALL_SPAN,
                LEDGER_BUDGET_TOKENS,
                gauntlet_domain::MicroUsd::new(LEDGER_BUDGET_COST_MICROS),
            )
            .expect("the fixture limits are usable"),
            LEDGER_RESERVE_PCT,
            gauntlet_domain::DiagnosticAllowance::new(
                LEDGER_DIAGNOSTIC_TOKENS,
                gauntlet_domain::MicroUsd::new(LEDGER_DIAGNOSTIC_COST_MICROS),
            ),
        )
        .expect("the fixture budget is usable"),
        parent_run: None,
    })
    .expect("the fixture run is valid");
    state
        .record_run(&run, bound())
        .expect("the run identity records");
}

fn ledger_pass_record(run_id: &RunId, unit: &str, sequence: usize) -> VerificationRecordDoc {
    let verification = format!("vrf-antigaming-{sequence}");
    VerificationRecordDoc::new(VerificationContent {
        verification: gauntlet_domain::VerificationRef::new(&verification).expect("ref"),
        run_id: run_id.clone(),
        iteration_no: gauntlet_domain::IterationNo::new(LEDGER_CHECKPOINT_ITERATION)
            .expect("iterations are 1-based"),
        unit_id: UnitId::new(unit).expect("unit id"),
        claim: CLAIM.to_owned(),
        requirement: RequirementDoc::new(
            gauntlet_domain::EvidenceKind::TestReport,
            gauntlet_domain::EvidenceLocator::new(EVIDENCE_LOCATOR).expect("locator"),
            gauntlet_domain::Producer::Instrument,
        ),
        instrument: VerifyingInstrument::new(LEDGER_INSTRUMENT.to_owned(), "1".to_owned(), true),
        evidence: gauntlet_domain::EvidenceRef::new(&format!("evd-antigaming-{sequence}"))
            .expect("evidence ref"),
        produced_at: gauntlet_domain::EpochSeconds::new(LEDGER_BASE_INSTANT + 10),
        outcome: gauntlet_domain::Outcome::Pass,
        class: None,
        detail: "the deterministic instrument decided PASS".to_owned(),
    })
    .expect("the fixture record is well formed")
}

/// Lands ONE run's full successful substrate through the earned path:
/// identity, boundary checkpoint, the authority-minted PASS record, and
/// the authority-admitted §16.3 settlement commit.
fn ledger_settle_one_run(
    state: &Arc<EmbeddedStateStore>,
    clock: &DeterministicClock,
    run_id: &RunId,
    unit: &str,
    sequence: usize,
) {
    ledger_durable_run(state, run_id);
    state
        .checkpoint(
            Checkpoint {
                run_id: run_id.clone(),
                reference_digest: ledger_reference_digest(),
                iteration_no: gauntlet_domain::IterationNo::new(LEDGER_CHECKPOINT_ITERATION)
                    .expect("iterations are 1-based"),
                unit_states: Vec::new(),
                budget_spent: gauntlet_domain::Spent::from_counters(
                    LEDGER_SPENT_ROUNDS,
                    LEDGER_SPENT_WALL_SPAN,
                    LEDGER_SPENT_TOKENS,
                    gauntlet_domain::MicroUsd::new(LEDGER_SPENT_COST_MICROS),
                    0,
                    gauntlet_domain::MicroUsd::new(0),
                ),
                in_flight: Vec::new(),
                event_log_offset: gauntlet_domain::EventLogOffset::new(0),
            },
            bound(),
        )
        .expect("the checkpoint lands");
    let record = ledger_pass_record(run_id, unit, sequence);
    VerificationRecordAuthority::new(state.clone())
        .record(&record, bound())
        .expect("the PASS record is earned and durable");
    let commit = SettlementCommit {
        run_id: run_id.clone(),
        unit_id: UnitId::new(unit).expect("unit id"),
        evidence: record.evidence().clone(),
        verification: record.verification().clone(),
        reference_digest: ledger_reference_digest(),
        at: clock.now(),
    };
    let admitted = SettlementCommitAuthority::new(state.clone())
        .admit(commit, bound())
        .expect("the settlement admits");
    state
        .commit_settlement(&admitted, bound())
        .expect("the settlement commits");
}

// ---------------------------------------------------------------------------
// G3 — skip expensive verification: a settlement commit for a unit with
// NO verification record at all is refused by the store-side §16.1 gate.
// (The P11 proof — a pass VERDICT without a verification record never
// settles through the composed chain — is p11_critic_is_not_verifier.rs,
// cited.)
// ---------------------------------------------------------------------------

/// THE TEETH: the no-records arm of `SettlementCommitAuthority::admit`
/// (`crates/gauntlet-ports/src/producer_authority.rs`). Neutralize it and
/// the refusal below changes shape — this test's typed-error AND exact
/// detail assertions are RED.
#[test]
fn g3_a_settlement_with_no_verification_record_is_refused() {
    let scratch = GateScratch::new("g3-skip");
    let state = scratch.open_state();
    let clock = clock();
    let run_id = RunId::new("01ARZ3NDEKTSV4RRFFQ69G5D01").expect("the fixture run id is a ULID");
    ledger_durable_run(&state, &run_id);
    // The run DID verify its first unit: unit A's PASS record is durably
    // earned. Unit B (the attack below) has NO record of its own — and
    // the admission must not let the same run's FOREIGN record stand in
    // for B's verification.
    VerificationRecordAuthority::new(state.clone())
        .record(&ledger_pass_record(&run_id, "AK-9904", 1), bound())
        .expect("unit A's PASS record is earned and durable");

    // The skip: settle unit B WITHOUT any verification work — no record
    // exists for IT in the run.
    let commit = SettlementCommit {
        run_id: run_id.clone(),
        unit_id: UnitId::new("AK-9905").expect("unit id"),
        evidence: gauntlet_domain::EvidenceRef::new("evd-antigaming-g3").expect("evidence ref"),
        verification: gauntlet_domain::VerificationRef::new("vrf-antigaming-g3")
            .expect("verification ref"),
        reference_digest: ledger_reference_digest(),
        at: clock.now(),
    };
    let error = SettlementCommitAuthority::new(state.clone())
        .admit(commit, bound())
        .expect_err("a settlement with no verification record is refused");
    match &error {
        SettlementAdmissionError::VerificationNotPass { detail, .. } => {
            assert!(
                detail.contains("no verification record") && detail.contains("at all"),
                "the refusal names the absent verification work: {detail}"
            );
        }
        other => panic!("expected VerificationNotPass, got {other:?}"),
    }
}

use gauntlet_ports::producer_authority::SettlementAdmissionError;

// ---------------------------------------------------------------------------
// G12 — claim missing evidence as pass: records exist, NONE is PASS —
// the three-state floor at the store-side admission (a FAIL or
// INCONCLUSIVE outcome never settles). The operator-action parity and
// the every-layer INCONCLUSIVE proof are p12_missing_is_not_passing.rs,
// cited.
// ---------------------------------------------------------------------------

/// THE TEETH: the no-PASS arm of `SettlementCommitAuthority::admit`
/// (`crates/gauntlet-ports/src/producer_authority.rs`). Neutralize the
/// PASS filter and the FAIL record below admits — this test is RED.
#[test]
fn g12_a_non_pass_outcome_never_admits_a_settlement() {
    let scratch = GateScratch::new("g12-missing");
    let state = scratch.open_state();
    let clock = clock();
    let run_id = RunId::new("01ARZ3NDEKTSV4RRFFQ69G5D02").expect("the fixture run id is a ULID");
    ledger_durable_run(&state, &run_id);

    // The chain legitimately mints a FAIL record (a real instrument said
    // no). The attacker then tries to settle on it — the claim that
    // missing (non-passing) evidence is a pass.
    let failing = VerificationRecordDoc::new(VerificationContent {
        verification: gauntlet_domain::VerificationRef::new("vrf-antigaming-g12").expect("ref"),
        run_id: run_id.clone(),
        iteration_no: gauntlet_domain::IterationNo::new(LEDGER_CHECKPOINT_ITERATION)
            .expect("iterations are 1-based"),
        unit_id: UnitId::new("AK-9906").expect("unit id"),
        claim: CLAIM.to_owned(),
        requirement: RequirementDoc::new(
            gauntlet_domain::EvidenceKind::TestReport,
            gauntlet_domain::EvidenceLocator::new(EVIDENCE_LOCATOR).expect("locator"),
            gauntlet_domain::Producer::Instrument,
        ),
        instrument: VerifyingInstrument::new(LEDGER_INSTRUMENT.to_owned(), "1".to_owned(), true),
        evidence: gauntlet_domain::EvidenceRef::new("evd-antigaming-g12").expect("evidence ref"),
        produced_at: gauntlet_domain::EpochSeconds::new(LEDGER_BASE_INSTANT + 10),
        outcome: gauntlet_domain::Outcome::Fail,
        class: None,
        detail: "the deterministic instrument decided FAIL".to_owned(),
    })
    .expect("the fixture record is well formed");
    VerificationRecordAuthority::new(state.clone())
        .record(&failing, bound())
        .expect("the FAIL record is earned and durable");

    let commit = SettlementCommit {
        run_id: run_id.clone(),
        unit_id: UnitId::new("AK-9906").expect("unit id"),
        evidence: failing.evidence().clone(),
        verification: failing.verification().clone(),
        reference_digest: ledger_reference_digest(),
        at: clock.now(),
    };
    let error = SettlementCommitAuthority::new(state.clone())
        .admit(commit, bound())
        .expect_err("a FAIL outcome never settles");
    match &error {
        SettlementAdmissionError::VerificationNotPass { detail, .. } => {
            assert!(
                detail.contains("none is PASS") && detail.contains("FAIL"),
                "the refusal names the recorded outcomes: {detail}"
            );
        }
        other => panic!("expected VerificationNotPass, got {other:?}"),
    }

    // The §16.3 discipline the same authority enforces around the PASS
    // floor (the committed key's own teeth, exercised over an honestly
    // earned settlement): the key is moored, and re-entry is idempotent.
    const G12B_RUN_ID: &str = "01ARZ3NDEKTSV4RRFFQ69G5D03";
    const G12B_UNIT: &str = "AK-9906";
    let ledger_state = scratch.open_state();
    ledger_settle_one_run(
        &ledger_state,
        &clock,
        &RunId::new(G12B_RUN_ID).expect("the fixture run id is a ULID"),
        G12B_UNIT,
        21,
    );
    let records = ledger_state
        .verification_records(
            &RunId::new(G12B_RUN_ID).expect("the fixture run id is a ULID"),
            bound(),
        )
        .expect("the records read");
    let pass = records
        .iter()
        .find(|record| record.outcome() == gauntlet_domain::Outcome::Pass)
        .expect("the earned PASS record");
    let authority = SettlementCommitAuthority::new(ledger_state.clone());

    // (a) the commit's EVIDENCE must be the evidence a durable PASS
    // record verified: a commit naming the PASS verification with
    // FOREIGN evidence is refused.
    let foreign_evidence = SettlementCommit {
        run_id: RunId::new(G12B_RUN_ID).expect("the fixture run id is a ULID"),
        unit_id: UnitId::new(G12B_UNIT).expect("unit id"),
        evidence: gauntlet_domain::EvidenceRef::new("evd-antigaming-g12-foreign")
            .expect("evidence ref"),
        verification: pass.verification().clone(),
        reference_digest: ledger_reference_digest(),
        at: clock.now(),
    };
    let error = authority
        .admit(foreign_evidence, bound())
        .expect_err("a commit with foreign evidence is refused");
    assert!(
        matches!(error, SettlementAdmissionError::VerificationNotPass { .. })
            && error.to_string().contains("name evidence other than"),
        "the evidence-mooring refusal fires: {error:?}"
    );

    // (b) the §16.3 key's verification component is MOORED in the durable
    // record: a commit inventing a verification ref is refused.
    let invented_ref = SettlementCommit {
        run_id: RunId::new(G12B_RUN_ID).expect("the fixture run id is a ULID"),
        unit_id: UnitId::new(G12B_UNIT).expect("unit id"),
        evidence: pass.evidence().clone(),
        verification: gauntlet_domain::VerificationRef::new("vrf-antigaming-g12-invented")
            .expect("verification ref"),
        reference_digest: ledger_reference_digest(),
        at: clock.now(),
    };
    let error = authority
        .admit(invented_ref, bound())
        .expect_err("a commit with an invented verification ref is refused");
    assert!(
        error.to_string().contains("moored in the durable record")
            || error.to_string().contains("none is the commit's"),
        "the ref-mooring refusal fires: {error:?}"
    );

    // (c) re-entry of the SAME key is the idempotent no-op: exactly ONE
    // durable settlement row exists after the re-admit + re-commit.
    let replay = SettlementCommit {
        run_id: RunId::new(G12B_RUN_ID).expect("the fixture run id is a ULID"),
        unit_id: UnitId::new(G12B_UNIT).expect("unit id"),
        evidence: pass.evidence().clone(),
        verification: pass.verification().clone(),
        reference_digest: ledger_reference_digest(),
        at: clock.now(),
    };
    let error = authority
        .admit(replay, bound())
        .expect_err("re-entry of the same key is the typed AlreadyCommitted no-op");
    assert!(
        matches!(error, SettlementAdmissionError::AlreadyCommitted { .. }),
        "the replay is the typed idempotent no-op: {error:?}"
    );
    let rows = ledger_state
        .settlements(
            &RunId::new(G12B_RUN_ID).expect("the fixture run id is a ULID"),
            bound(),
        )
        .expect("the rows read");
    assert_eq!(
        rows.len(),
        1,
        "exactly one durable settlement row: replaying twice is a no-op"
    );

    // (d) a unit settles ONCE per run: a second, fully EARNED key for the
    // already-settled unit (its own new PASS record) is still refused —
    // exactly-once, with the refusal naming BOTH verifications (the one
    // that settled and the one offered). The existing-row probe matches
    // on the FULL key (unit AND verification); permissive matching here
    // would misreport this refusal as an idempotent replay or a
    // divergence instead.
    let second = ledger_pass_record(
        &RunId::new(G12B_RUN_ID).expect("the fixture run id is a ULID"),
        G12B_UNIT,
        22,
    );
    VerificationRecordAuthority::new(ledger_state.clone())
        .record(&second, bound())
        .expect("the second PASS record is earned and durable");
    let second_commit = SettlementCommit {
        run_id: RunId::new(G12B_RUN_ID).expect("the fixture run id is a ULID"),
        unit_id: UnitId::new(G12B_UNIT).expect("unit id"),
        evidence: second.evidence().clone(),
        verification: second.verification().clone(),
        reference_digest: ledger_reference_digest(),
        at: clock.now(),
    };
    let error = authority
        .admit(second_commit, bound())
        .expect_err("a unit settles exactly once per run");
    match &error {
        SettlementAdmissionError::UnitAlreadySettled(refusal) => {
            assert_eq!(refusal.settled_verification.as_str(), "vrf-antigaming-21");
            assert_eq!(refusal.offered_verification.as_str(), "vrf-antigaming-22");
        }
        other => panic!(
            "expected UnitAlreadySettled, got {other:?} — the exactly-once floor must not \
             decay into a replay/divergence classification"
        ),
    }
}

// ---------------------------------------------------------------------------
// G4 — reduce variation in evaluation: the §19.2 recorded-variation
// condition — N settlements over IDENTICAL content axes are refused;
// run ids and timestamps are not variation. Loop-economics note (recon
// fact 7): this pins the promotion CONDITION at the layer where it lives
// (PromotionService); no per-settlement-economics claim is made. (The
// full-conjunction proof is p10 leg 3, cited.)
// ---------------------------------------------------------------------------

/// THE TEETH: `settled_with_real_variation` (`crates/gauntlet-app/src/
/// memory/promotion.rs`) — the content-axis comparison. Make it accept
/// anything and the identical-window refusal below disappears — this
/// test's named-reason assertion is RED.
#[test]
fn g4_identical_content_axes_are_refused_as_insufficient_variation() {
    use gauntlet_app::memory::developmental::DevelopmentalImport;
    use gauntlet_app::memory::promotion::{PromotionDecision, PromotionService};

    const MIN_SETTLEMENTS: u32 = 3;
    const G4_UNIT: &str = "AK-9907";
    const G4_CAPABILITY: &str = "cap-antigaming-g4";

    let scratch = GateScratch::new("g4-variation");
    let state = scratch.open_state();
    let ledger = scratch.open_capabilities();
    let clock = clock();
    let capability = CapabilityId::new(G4_CAPABILITY).expect("capability id");

    // Three honest, fully settled runs over THE SAME unit, THE SAME
    // frozen reference, THE SAME evidence class. Only run ids and
    // timestamps differ — which are deliberately NOT variation axes.
    for sequence in 1..=3 {
        let run_id = RunId::new(&format!("01ARZ3NDEKTSV4RRFFQ69G5D{sequence:02X}"))
            .expect("the fixture run id is a ULID");
        ledger_settle_one_run(&state, &clock, &run_id, G4_UNIT, sequence);
        let summary = DevelopmentalImport::new(state.clone(), ledger.clone(), clock.clone())
            .import_run(&capability, &run_id, bound())
            .expect("the import projects the run");
        assert!(
            summary.appended >= 2,
            "each run's memory lands: {summary:?}"
        );
    }

    let service = PromotionService::new(ledger.clone(), MIN_SETTLEMENTS, clock.clone())
        .expect("the minimum is valid");
    let decision = service.evaluate(&capability, bound()).expect("evaluates");
    match decision {
        PromotionDecision::Refused { refusals, entry } => {
            assert!(
                refusals
                    .iter()
                    .any(|refusal| refusal.rule() == "insufficient_variation"),
                "identical content axes are recorded as insufficient variation: {refusals:?}"
            );
            assert_eq!(
                entry.status,
                gauntlet_domain::CapabilityStatus::Provisional,
                "the refusal is durable: the capability sits provisional"
            );
        }
        PromotionDecision::Promoted { .. } => panic!(
            "three settlements under identical content axes promoted — the run-id trap \
             (variation reduced to re-rolls)"
        ),
    }
}

// ---------------------------------------------------------------------------
// G14 — remove recovery checks: recovery credit that completed NOTHING
// is refused (§18.3's completed-count floor at the memory import), and
// recovery credit for a run that exists nowhere is refused. (The
// full recovery proof — crash, resume, exactly-once — is p7_recovery.rs;
// the RecoveryNotEarned import floor is task22 C4, cited.)
// ---------------------------------------------------------------------------

/// THE TEETH: the `completed == 0` guard in
/// `DevelopmentalImport::record_recovery` (`crates/gauntlet-app/src/
/// memory/developmental.rs`). Remove it and a zero-completed "recovery"
/// below is accepted as durable recovery credit — this test is RED.
#[test]
fn g14_recovery_credit_with_no_completed_recovery_is_refused() {
    use gauntlet_app::memory::developmental::DevelopmentalImport;

    const G4_CAPABILITY: &str = "cap-antigaming-g14";
    const G14_UNIT: &str = "AK-9908";

    let scratch = GateScratch::new("g14-recovery");
    let state = scratch.open_state();
    let ledger = scratch.open_capabilities();
    let clock = clock();
    let capability = CapabilityId::new(G4_CAPABILITY).expect("capability id");
    let run_id = RunId::new("01ARZ3NDEKTSV4RRFFQ69G5D10").expect("the fixture run id is a ULID");

    // The import's substrate: one honestly settled run (the capability
    // registers; the run is durable).
    ledger_settle_one_run(&state, &clock, &run_id, G14_UNIT, 1);
    DevelopmentalImport::new(state.clone(), ledger.clone(), clock.clone())
        .import_run(&capability, &run_id, bound())
        .expect("the import projects the run");

    // THE ATTACK ONE: claim recovery credit with a completed count of
    // ZERO — §18.3 completed nothing, so there is no handled recovery.
    let error = DevelopmentalImport::new(state.clone(), ledger.clone(), clock.clone())
        .record_recovery(
            &capability,
            &run_id,
            "the claimed handled recovery".to_owned(),
            0,
            "zero completions".to_owned(),
            bound(),
        )
        .expect_err("unearned recovery credit is refused");
    assert!(
        matches!(
            error,
            gauntlet_app::memory::developmental::ImportError::RecoveryNotEarned { completed: 0 }
        ),
        "the zero-completed recovery is refused by name: {error:?}"
    );

    // THE ATTACK TWO: recovery credit for a run that exists NOWHERE.
    let phantom = RunId::new("01ARZ3NDEKTSV4RRFFQ69G5D11").expect("the fixture run id is a ULID");
    let error = DevelopmentalImport::new(state.clone(), ledger.clone(), clock.clone())
        .record_recovery(
            &capability,
            &phantom,
            "the phantom run's recovery".to_owned(),
            1,
            "one completion, no run".to_owned(),
            bound(),
        )
        .expect_err("recovery credit for a nonexistent run is refused");
    assert!(
        matches!(
            error,
            gauntlet_app::memory::developmental::ImportError::RunNotDurable { .. }
        ),
        "the phantom-run recovery is refused by name: {error:?}"
    );
}

// ---------------------------------------------------------------------------
// G15 — end early to save tokens while reporting settlement: the report's
// categories derive from DURABLE RECORDS — a unit whose work was recorded
// but never settled (the run ended early) is reported provisional, never
// implemented-and-proven; the proven category demands the settlement row
// AND the durable PASS record it names. (The operator-surface proof is
// the report task's own suite; the durable-derivation shape is p7/p12
// adjacent, cited.)
// ---------------------------------------------------------------------------

/// THE TEETH: `classify_unit`'s proven-pair check (`crates/gauntlet-app/
/// src/report/mod.rs` — settlement row AND a durable PASS record naming
/// it). Break the pair (require only one half, or flip the outcome test)
/// and one of the two worlds below reports implemented-and-proven
/// without its proof — this test is RED.
#[test]
fn g15_a_budget_ended_run_is_never_reported_implemented_and_proven() {
    use gauntlet_app::report::{StatusCategory, build_report};

    const G15_SETTLED_UNIT: &str = "AK-9909";
    const G15_UNSETTLED_UNIT: &str = "AK-9910";
    const G15_RUN_ID: &str = "01ARZ3NDEKTSV4RRFFQ69G5D12";

    let scratch = GateScratch::new("g15-report");
    let state = scratch.open_state();
    let clock = clock();
    let run_id = RunId::new(G15_RUN_ID).expect("the fixture run id is a ULID");

    // Unit A: the honest proof pair — earned settlement + PASS record.
    ledger_settle_one_run(&state, &clock, &run_id, G15_SETTLED_UNIT, 1);
    // Unit B: recorded attempt, NO settlement — the run ended early
    // before B's verification ever passed. Its FAIL record is durable.
    ledger_durable_run(&state, &run_id);
    let failing = VerificationRecordDoc::new(VerificationContent {
        verification: gauntlet_domain::VerificationRef::new("vrf-antigaming-g15").expect("ref"),
        run_id: run_id.clone(),
        iteration_no: gauntlet_domain::IterationNo::new(LEDGER_CHECKPOINT_ITERATION)
            .expect("iterations are 1-based"),
        unit_id: UnitId::new(G15_UNSETTLED_UNIT).expect("unit id"),
        claim: CLAIM.to_owned(),
        requirement: RequirementDoc::new(
            gauntlet_domain::EvidenceKind::TestReport,
            gauntlet_domain::EvidenceLocator::new(EVIDENCE_LOCATOR).expect("locator"),
            gauntlet_domain::Producer::Instrument,
        ),
        instrument: VerifyingInstrument::new(LEDGER_INSTRUMENT.to_owned(), "1".to_owned(), true),
        evidence: gauntlet_domain::EvidenceRef::new("evd-antigaming-g15").expect("evidence ref"),
        produced_at: gauntlet_domain::EpochSeconds::new(LEDGER_BASE_INSTANT + 10),
        outcome: gauntlet_domain::Outcome::Fail,
        class: None,
        detail: "the deterministic instrument decided FAIL".to_owned(),
    })
    .expect("the fixture record is well formed");
    VerificationRecordAuthority::new(state.clone())
        .record(&failing, bound())
        .expect("the FAIL record is earned and durable");

    let report = build_report(state.as_ref(), &run_id, &[], bound()).expect("the report builds");
    let row_of = |unit: &str| {
        report
            .units
            .iter()
            .find(|row| row.unit.as_str() == unit)
            .unwrap_or_else(|| panic!("the report carries unit {unit}"))
    };
    // The positive control: the proven pair reads proven.
    assert_eq!(
        row_of(G15_SETTLED_UNIT).category,
        StatusCategory::ImplementedAndProven,
        "the honest settlement reads implemented-and-proven"
    );
    // THE CLAIM UNDER TEST: the ended-early unit — recorded work, no
    // settlement — is NEVER reported proven.
    let unsettled = row_of(G15_UNSETTLED_UNIT);
    assert_eq!(
        unsettled.category,
        StatusCategory::Provisional,
        "recorded work without recorded proof is provisional: {}",
        unsettled.basis
    );
    assert!(
        unsettled
            .basis
            .contains("an unproven outcome never settles"),
        "the basis names why the FAIL outcome cannot settle: {}",
        unsettled.basis
    );
}

// ---------------------------------------------------------------------------
// G9 — let the critic edit implementation: ADVISORY (the honest-boundary
// rule). The runtime has NO mechanical write-scope enforcement for a
// critic-context process: the role file's `filesystem: read-only` is a
// RECORDED, TYPED posture (closed vocabulary; an unknown mode word is
// refused, never silently widened), and the roles lock's
// `Enforcement::Advisory` exists precisely to record the weakness
// instead of faking enforcement. This test asserts that advisory surface
// and NOTHING MORE — the manifest entry carries advisory: and no
// mutation scope. Reference-candidate note (for the closure): a runtime
// write-scope enforcement for role contexts is the missing mechanical
// defense.
// ---------------------------------------------------------------------------

/// ADVISORY TEST — no mutation scope. The assertions below are the
/// posture that exists (recorded, typed, refusal-to-widen), not a
/// mechanical containment proof.
#[test]
fn g9_the_critic_write_surface_posture_is_advisory_and_recorded() {
    use gauntlet_app::roles::{FilesystemMode, NetworkMode, RoleDefinition};

    const CRITIC_ROLE: &str = concat!(
        "---\n",
        "role: critic\n",
        "description: Compares an observation bundle against the frozen reference and ",
        "emits residuals\n",
        "permissions:\n",
        "  filesystem: read-only\n",
        "  write_paths: []\n",
        "  network: deny\n",
        "  tools: [code_nav]\n",
        "context:\n",
        "  allow: [reference, observation_bundle, comparison_contract]\n",
        "  deny: []\n",
        "result_contract: comparison_record@v1\n",
        "---\n",
        "# Critic role contract\n",
        "\n",
        "You compare; you never edit implementation.\n",
    );
    const WIDENED_MODE: &str = concat!(
        "---\n",
        "role: critic\n",
        "description: Compares an observation bundle\n",
        "permissions:\n",
        "  filesystem: run-state\n",
        "  write_paths: []\n",
        "  network: deny\n",
        "  tools: [code_nav]\n",
        "context:\n",
        "  allow: [reference]\n",
        "  deny: []\n",
        "result_contract: comparison_record@v1\n",
        "---\n",
        "# Widened\n",
    );

    let env = gauntlet_app::config::EnvLayer::from_parts("", &[], &[]);
    let tool_vocabulary = ["code_nav"];

    // The recorded posture: the critic's filesystem mode parses to the
    // closed vocabulary's read-only word, the network to deny.
    let definition = RoleDefinition::load("critic", CRITIC_ROLE, &env, &tool_vocabulary)
        .expect("the critic role definition loads");
    assert_eq!(
        definition.permissions().filesystem,
        FilesystemMode::ReadOnly,
        "the critic's recorded filesystem posture is read-only"
    );
    assert_eq!(
        definition.permissions().network,
        NetworkMode::Deny,
        "the critic's recorded network posture is deny"
    );

    // The vocabulary is CLOSED: a made-up mode word is refused — the
    // recorded posture can never silently widen.
    let error = RoleDefinition::load("critic", WIDENED_MODE, &env, &tool_vocabulary)
        .expect_err("a mode word outside the vocabulary is refused");
    assert!(
        matches!(
            error,
            gauntlet_app::roles::RoleError::UnknownFilesystemMode { .. }
        ),
        "the unknown filesystem mode is a typed refusal: {error:?}"
    );
}

// ---------------------------------------------------------------------------
// The manifest validator — the machine-readable teeth map
// (.agents/reports/defended-mechanisms.txt) is parsed and enforced:
// format, all-sixteen coverage, location existence, and the
// non-advisory test_id ↔ antigaming-target binding.
// ---------------------------------------------------------------------------

#[test]
fn the_defended_mechanisms_manifest_covers_all_sixteen_strategies() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("the cli crate sits two levels under the repository root")
        .to_path_buf();
    // The manifest file NAME is a leaf literal (no separators); the
    // directory layout comes from the declared defaults table.
    use gauntlet_app::config::defaults::{AGENTS_DIR_NAME, REPORTS_DIR_NAME};
    let manifest_path = repo_root
        .join(AGENTS_DIR_NAME)
        .join(REPORTS_DIR_NAME)
        .join("defended-mechanisms.txt");
    let text = std::fs::read_to_string(&manifest_path)
        .expect("the defended-mechanisms manifest exists and reads");

    let mut covered: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let mut entries = 0usize;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fields: Vec<&str> = line.split('|').map(str::trim).collect();
        assert_eq!(
            fields.len(),
            4,
            "every manifest line is `test_id | G-ID | mechanism | source_location`: {line}"
        );
        let (test_id, gid, mechanism, location) = (
            fields[0].to_owned(),
            fields[1].to_owned(),
            fields[2].to_owned(),
            fields[3].to_owned(),
        );
        entries += 1;

        // The G-ID is exactly the strategy vocabulary (G1..G16).
        let gid_number = gid
            .strip_prefix('G')
            .and_then(|tail| tail.parse::<u8>().ok());
        assert!(
            gid_number.is_some_and(|number| (1..=16).contains(&number)),
            "the G-ID is G1..G16: {gid}"
        );
        covered.insert(gid.clone());

        // A non-advisory entry's test must live in THIS target (ids
        // shaped `antigaming::<fn>`). Recorded divergence: the plan
        // gate's literal `antigaming::` positional matches zero tests on
        // this nextest (package-selector parse, exit 4 — fails closed);
        // the working suite gate is `--test antigaming`.
        if !mechanism.starts_with("advisory:") {
            assert!(
                test_id.starts_with("antigaming::"),
                "a non-advisory entry's test_id must be in the antigaming target: {test_id}"
            );
        } else {
            // Advisory entries assert the advisory surface that exists —
            // their reason must be spelled out.
            assert!(
                mechanism.len() > "advisory:".len(),
                "an advisory entry spells out its reason: {mechanism}"
            );
        }

        // The source location exists: the file reads and the named line
        // is inside it.
        let (path, line_no) = location
            .rsplit_once(':')
            .expect("the location is `path:line`");
        let absolute = repo_root.join(path);
        let source = std::fs::read_to_string(&absolute).unwrap_or_else(|error| {
            panic!("the manifest's source location {location} must exist: {error}")
        });
        let line: usize = line_no.parse().expect("the location line is a number");
        assert!(
            line >= 1 && line <= source.lines().count(),
            "the manifest's line {line_no} is inside {path}"
        );
    }

    assert!(
        entries >= 16,
        "the manifest carries at least one entry per strategy: {entries}"
    );
    for index in 1..=16u8 {
        let gid = format!("G{index}");
        assert!(
            covered.contains(&gid),
            "strategy {gid} has a manifest entry (all sixteen are covered)"
        );
    }
}
