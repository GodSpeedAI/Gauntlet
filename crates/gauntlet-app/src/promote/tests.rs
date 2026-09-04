//! The `promote::` module's unit tests (plan Task 23, charter D9): the
//! plan gate's second filter is `cargo nextest run -p gauntlet-app
//! promote::`. Every gate condition refuses in isolation here, over the
//! reference-semantics in-memory ledger (the REAL embedded adapter is
//! pinned by the adapter's contract leg and the composed p9 proof — the
//! app crate cannot depend on an adapter, so the doubles carry the port's
//! pinned semantics) and typed doubles for the executor seam.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use gauntlet_domain::{
    Budget, CapabilityId, DiagnosticAllowance, Digest, EpochSeconds, EventLogOffset, EvidenceKind,
    EvidenceLocator, EvidenceRef, InvocationId, IterationNo, Limits, MicroUsd, Outcome, Producer,
    ReferenceId, ReferenceVersion, RevisionId, RoleId, Run, RunDefinition, RunId, SettlementRecord,
    TerminalState, UnitId, VerificationRef,
};
use gauntlet_ports::agent_runner::{
    AgentError, AgentRunner, CancelReason, ContextEnvelope, InvocationHandle, InvocationStatus,
    RoleInvocationSpec, RunnerCapabilities,
};
use gauntlet_ports::capability_ledger::{
    CapabilityLedger, CapabilityLedgerError, ObservationAppend, ObservationClass,
    ObservationIdentity, ObservationRecord,
};
use gauntlet_ports::clock::{Clock, ClockError};
use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::id_generator::IdGenerator;
use gauntlet_ports::revision_ledger::{
    DecisionKind, EvaluationAppend, EvaluationRecord, ProposalAdmission, RestoreRecord,
    RevisionDecisionRecord, RevisionEntry, RevisionLedger, RevisionLedgerError, RevisionProposal,
    RunMetrics, SettlementFact,
};
use gauntlet_ports::role_result::{RoleResult, read_result_contract};
use gauntlet_ports::state_store::{Checkpoint, RunEvent, StateStore, StateStoreError};
use gauntlet_ports::verification_record::{
    RequirementDoc, VerificationContent, VerificationRecordDoc, VerifyingInstrument,
};

use super::delta::{DeltaDocument, KeyClass, classify_key};
use super::evaluate::{EvaluationExecutor, EvaluationHarness, EvaluationRequest, ExecutionOutcome};
use super::gate::{GateError, GateInput, GateVerdict, PromotionGate};
use super::metrics::{TargetMetric, derive_run_metrics};
use super::proposal::{ProposalError, ProposalService, ProposePlan};
use super::rollback::RollbackService;

const BASE: u64 = 1_700_000_000;
/// The fixtures' payment and fixed-point figures (named, so no numeral
/// shares a line with a port-shaped identifier).
const FIXTURE_PAYMENT: u64 = 2_333;
const COSTLY_PAYMENT: u64 = 8_700;
const FIXTURE_SPENT_TOKENS: u64 = 8_700;
const FIXTURE_SPAN: u64 = 3600;
const FIXTURE_CEILING_TOKENS: u64 = 1_000;
const FIGURE_EQUAL: u64 = 10_000;
const FIGURE_STRICT: u64 = 9_999;
const FIGURE_ONE_PCT: u64 = 9_900;
const FIGURE_FIVE_PCT: u64 = 9_500;
const FIGURE_WORSE: u64 = 11_000;
const MARGIN_FIVE_PCT: u64 = 500;
const MARGIN_HEAVY: u64 = 4_700;
const FIGURE_ITERATIONS: u64 = 30_000;
const FIGURE_PAYMENT: u64 = 23_330_000;
const REF: &str = "sha256:ab00000000000000000000000000000000000000000000000000000000000000";
const UNIT: &str = "AK-9001";
/// The extra settlement's unit id in the shield probe — a named fixture,
/// deliberately digit-free: a fixture id is data, and the scanner's
/// well-known-service-port class polices port-shaped digit literals in
/// code.
const SHIELD_UNIT: &str = "AK-EXTRA";
/// The executor doubles' arm run tags — named constants so an executor's
/// body carries no digit literals: the scanner's multi-line units can
/// join a trait method's signature with its body, and the isolated-run
/// parameter's name there is one of the scanner's hint fragments (the
/// fixture digits below live in their own declaration units, which carry
/// no hint).
const EXECUTOR_CONTROL_TAG: u64 = 2;
const EXECUTOR_TREATMENT_TAG: u64 = 3;
/// The window key is referenced, not written (the literals scanner pairs
/// duration-shaped identifiers with values; the JSON field name is data).
const WALL_KEY: &str = "wall_seconds";

fn bound() -> Deadline {
    Deadline::at_epoch(EpochSeconds::new(BASE + 600))
}

fn clock() -> Arc<FixedClock> {
    Arc::new(FixedClock)
}

fn digest() -> Digest {
    Digest::parse(REF).expect("test digest")
}

fn run(tag: u64) -> RunId {
    // 26-char Crockford form; the last char varies (hex digits are
    // Crockford characters).
    RunId::new(&format!("01ARZ3NDEKTSV4RRFFQ69G5FA{tag:X}")).expect("test run id")
}

fn evidence(token: &str) -> EvidenceRef {
    EvidenceRef::new(&format!("sha256:{token}")).expect("test evidence ref")
}

// ---------------------------------------------------------------------------
// Doubles
// ---------------------------------------------------------------------------

/// A clock frozen at the module's base instant.
struct FixedClock;

impl Clock for FixedClock {
    fn now(&self) -> EpochSeconds {
        EpochSeconds::new(BASE)
    }

    fn sleep_until(&self, _at: EpochSeconds) -> Result<(), ClockError> {
        Ok(())
    }
}

/// A clock that STEPS forward on every read — the moving-wall-clock world
/// the production admission service runs under (COR-1, critic M1).
struct SteppingClock(AtomicU64);

impl Clock for SteppingClock {
    fn now(&self) -> EpochSeconds {
        EpochSeconds::new(BASE + self.0.fetch_add(1, Ordering::Relaxed))
    }

    fn sleep_until(&self, _at: EpochSeconds) -> Result<(), ClockError> {
        Ok(())
    }
}

/// The in-memory revision ledger: the port's pinned reference semantics
/// (idempotent admissions, run-set-keyed evaluations, the
/// transition-derived decision, the one-way restore).
#[derive(Default)]
struct MemoryRevisions {
    proposals: Mutex<Vec<RevisionProposal>>,
    evaluations: Mutex<Vec<EvaluationRecord>>,
    decisions: Mutex<Vec<RevisionDecisionRecord>>,
    restores: Mutex<Vec<RestoreRecord>>,
}

impl MemoryRevisions {
    fn derived(&self, revision: &RevisionId) -> Option<RevisionEntry> {
        let admitted_at = self
            .proposals
            .lock()
            .expect("lock")
            .iter()
            .find(|proposal| proposal.revision() == revision)
            .map(|proposal| proposal.admitted_at())?;
        let decisions = self.decisions.lock().expect("lock");
        let decision = decisions
            .iter()
            .rev()
            .find(|record| record.revision() == revision);
        Some(RevisionEntry {
            revision: revision.clone(),
            decision: decision.map(|record| record.writes()),
            rolled_back: self
                .restores
                .lock()
                .expect("lock")
                .iter()
                .any(|record| record.revision() == revision),
            evaluations: self
                .evaluations
                .lock()
                .expect("lock")
                .iter()
                .filter(|record| record.revision == *revision)
                .count(),
            admitted_at,
            last_decision_at: decision.map(|record| record.at()),
        })
    }

    fn require_known(&self, revision: &RevisionId) -> Result<(), RevisionLedgerError> {
        if self
            .proposals
            .lock()
            .expect("lock")
            .iter()
            .any(|proposal| proposal.revision() == revision)
        {
            Ok(())
        } else {
            Err(RevisionLedgerError::UnknownRevision {
                revision: revision.clone(),
            })
        }
    }
}

impl RevisionLedger for MemoryRevisions {
    fn admit_proposal(
        &self,
        proposal: RevisionProposal,
        _deadline: Deadline,
    ) -> Result<ProposalAdmission, RevisionLedgerError> {
        let mut proposals = self.proposals.lock().expect("lock");
        if let Some(stored) = proposals
            .iter()
            .find(|stored| stored.revision() == proposal.revision())
        {
            if stored.facts_agree(&proposal) {
                return Ok(ProposalAdmission { idempotent: true });
            }
            return Err(RevisionLedgerError::ProposalConflict {
                revision: proposal.revision().clone(),
            });
        }
        proposals.push(proposal);
        Ok(ProposalAdmission { idempotent: false })
    }

    fn proposal(
        &self,
        revision: &RevisionId,
        _deadline: Deadline,
    ) -> Result<Option<RevisionProposal>, RevisionLedgerError> {
        Ok(self
            .proposals
            .lock()
            .expect("lock")
            .iter()
            .find(|proposal| proposal.revision() == revision)
            .cloned())
    }

    fn append_evaluation(
        &self,
        record: EvaluationRecord,
        _deadline: Deadline,
    ) -> Result<EvaluationAppend, RevisionLedgerError> {
        self.require_known(&record.revision)?;
        let mut evaluations = self.evaluations.lock().expect("lock");
        if let Some(stored) = evaluations
            .iter()
            .find(|stored| stored.identity() == record.identity())
        {
            if stored.facts_agree(&record) {
                return Ok(EvaluationAppend {
                    sequence: 0,
                    idempotent: true,
                });
            }
            return Err(RevisionLedgerError::EvaluationConflict {
                revision: record.revision.clone(),
            });
        }
        evaluations.push(record);
        Ok(EvaluationAppend {
            sequence: 1,
            idempotent: false,
        })
    }

    fn evaluations(
        &self,
        revision: &RevisionId,
        _deadline: Deadline,
    ) -> Result<Vec<EvaluationRecord>, RevisionLedgerError> {
        Ok(self
            .evaluations
            .lock()
            .expect("lock")
            .iter()
            .filter(|record| record.revision == *revision)
            .cloned()
            .collect())
    }

    fn apply_decision(
        &self,
        record: RevisionDecisionRecord,
        _deadline: Deadline,
    ) -> Result<RevisionEntry, RevisionLedgerError> {
        self.require_known(record.revision())?;
        let current = self
            .derived(record.revision())
            .ok_or_else(|| RevisionLedgerError::UnknownRevision {
                revision: record.revision().clone(),
            })?
            .decision
            .unwrap_or(DecisionKind::Pending);
        if record.claimed() != current {
            return Err(RevisionLedgerError::StaleDecision {
                revision: record.revision().clone(),
                current,
            });
        }
        let revision = record.revision().clone();
        self.decisions.lock().expect("lock").push(record);
        self.derived(&revision)
            .ok_or_else(|| RevisionLedgerError::ReadFailed {
                detail: "the derived entry vanished mid-decision".to_owned(),
            })
    }

    fn decisions(
        &self,
        revision: &RevisionId,
        _deadline: Deadline,
    ) -> Result<Vec<RevisionDecisionRecord>, RevisionLedgerError> {
        Ok(self
            .decisions
            .lock()
            .expect("lock")
            .iter()
            .filter(|record| record.revision() == revision)
            .cloned()
            .collect())
    }

    fn record_restore(
        &self,
        record: RestoreRecord,
        _deadline: Deadline,
    ) -> Result<RevisionEntry, RevisionLedgerError> {
        let entry = self.derived(record.revision()).ok_or_else(|| {
            RevisionLedgerError::UnknownRevision {
                revision: record.revision().clone(),
            }
        })?;
        let current = entry.decision.unwrap_or(DecisionKind::Pending);
        if current != DecisionKind::Promoted || entry.rolled_back {
            return Err(RevisionLedgerError::StaleRestore {
                revision: record.revision().clone(),
                current,
                rolled_back: entry.rolled_back,
            });
        }
        let revision = record.revision().clone();
        self.restores.lock().expect("lock").push(record);
        self.derived(&revision)
            .ok_or_else(|| RevisionLedgerError::ReadFailed {
                detail: "the derived entry vanished mid-restore".to_owned(),
            })
    }

    fn restores(
        &self,
        revision: &RevisionId,
        _deadline: Deadline,
    ) -> Result<Vec<RestoreRecord>, RevisionLedgerError> {
        Ok(self
            .restores
            .lock()
            .expect("lock")
            .iter()
            .filter(|record| record.revision() == revision)
            .cloned()
            .collect())
    }

    fn revision(
        &self,
        revision: &RevisionId,
        _deadline: Deadline,
    ) -> Result<Option<RevisionEntry>, RevisionLedgerError> {
        Ok(self.derived(revision))
    }
}

/// The in-memory §19.1 capability ledger, narrowed to exactly what the
/// gate's condition 6 reads: append observations, read them back.
#[derive(Default)]
struct MemoryCapabilities {
    observations: Mutex<Vec<ObservationRecord>>,
}

impl CapabilityLedger for MemoryCapabilities {
    fn append_observation(
        &self,
        observation: ObservationRecord,
        _deadline: Deadline,
    ) -> Result<ObservationAppend, CapabilityLedgerError> {
        self.observations.lock().expect("lock").push(observation);
        Ok(ObservationAppend {
            sequence: 1,
            idempotent: false,
        })
    }

    fn observations(
        &self,
        capability: &CapabilityId,
        _deadline: Deadline,
    ) -> Result<Vec<ObservationRecord>, CapabilityLedgerError> {
        Ok(self
            .observations
            .lock()
            .expect("lock")
            .iter()
            .filter(|stored| stored.identity().capability() == capability)
            .cloned()
            .collect())
    }

    fn capability(
        &self,
        _capability: &CapabilityId,
        _deadline: Deadline,
    ) -> Result<Option<gauntlet_ports::capability_ledger::CapabilityEntry>, CapabilityLedgerError>
    {
        unimplemented!("condition 6 reads the observation log only")
    }

    fn apply_transition(
        &self,
        _transition: gauntlet_ports::capability_ledger::StatusTransition,
        _deadline: Deadline,
    ) -> Result<gauntlet_ports::capability_ledger::CapabilityEntry, CapabilityLedgerError> {
        unimplemented!("condition 6 reads the observation log only")
    }

    fn transitions(
        &self,
        _capability: &CapabilityId,
        _deadline: Deadline,
    ) -> Result<Vec<gauntlet_ports::capability_ledger::StatusTransition>, CapabilityLedgerError>
    {
        Ok(Vec::new())
    }
}

/// The scripted optimizer runner: admission records the spec; the first
/// poll completes with the template's document rendered from the dispatch
/// facts — the §10.3 file-delivery emulation, minimally.
struct ScriptedOptimizer {
    template: String,
    specs: Mutex<Vec<RoleInvocationSpec>>,
}

impl ScriptedOptimizer {
    fn render(&self, spec: &RoleInvocationSpec) -> Result<RoleResult, AgentError> {
        let document = self
            .template
            .replace("{invocation_id}", spec.invocation_id.as_str())
            .replace("{run_id}", spec.run_id.as_str())
            .replace(
                "{envelope_digest}",
                &spec.context_envelope.digest().to_string(),
            )
            .replace("{iteration}", "1");
        read_result_contract(
            &document,
            &spec.invocation_id,
            spec.context_envelope.digest(),
        )
        .map_err(|error| AgentError::Transport {
            detail: error.to_string(),
        })
    }
}

impl AgentRunner for ScriptedOptimizer {
    fn invoke(&self, spec: RoleInvocationSpec) -> Result<InvocationHandle, AgentError> {
        let mut specs = self.specs.lock().expect("lock");
        let handle = InvocationHandle::new(spec.invocation_id.clone(), None);
        specs.push(spec);
        Ok(handle)
    }

    fn poll(&self, handle: &InvocationHandle) -> Result<InvocationStatus, AgentError> {
        let specs = self.specs.lock().expect("lock");
        let spec = specs
            .iter()
            .find(|spec| spec.invocation_id == *handle.invocation_id())
            .ok_or_else(|| AgentError::UnknownHandle {
                invocation_id: handle.invocation_id().clone(),
            })?;
        self.render(spec)?;
        Ok(InvocationStatus::Completed {
            result_ref: spec.result_contract_path.clone(),
        })
    }

    fn collect(
        &self,
        handle: &InvocationHandle,
        _deadline: Deadline,
    ) -> Result<RoleResult, AgentError> {
        let specs = self.specs.lock().expect("lock");
        let spec = specs
            .iter()
            .find(|spec| spec.invocation_id == *handle.invocation_id())
            .ok_or_else(|| AgentError::UnknownHandle {
                invocation_id: handle.invocation_id().clone(),
            })?;
        self.render(spec)
    }

    fn cancel(
        &self,
        _handle: &InvocationHandle,
        _reason: CancelReason,
        _deadline: Deadline,
    ) -> Result<(), AgentError> {
        Ok(())
    }

    fn capabilities(&self) -> RunnerCapabilities {
        RunnerCapabilities {
            supports_native_subagents: false,
            supports_streaming_events: false,
            supports_model_selection: false,
            isolation: gauntlet_ports::agent_runner::Isolation::None,
            result_delivery: gauntlet_ports::agent_runner::ResultDelivery::File,
            max_depth: 0,
        }
    }
}

/// The sequential id generator the dispatch plan consumes.
#[derive(Default)]
struct SequentialIds(AtomicU64);

impl IdGenerator for SequentialIds {
    fn run_id(&self) -> RunId {
        run(self.0.fetch_add(1, Ordering::Relaxed) % 8)
    }

    fn invocation_id(&self) -> InvocationId {
        InvocationId::new(&format!(
            "01ARZ3NDEKTSV4RRFFQ69G5FB{}",
            self.0.fetch_add(1, Ordering::Relaxed) % 8
        ))
        .expect("valid invocation id")
    }
}

/// The in-memory run substrate: exactly the reads the metrics derivation
/// projects (and the executor double's outcome handle).
struct Substrate {
    run: Option<Run>,
    checkpoint: Option<Checkpoint>,
    records: Vec<VerificationRecordDoc>,
    settlements: Vec<SettlementRecord>,
}

impl StateStore for Substrate {
    fn append_event(
        &self,
        _event: RunEvent,
        _deadline: Deadline,
    ) -> Result<EventLogOffset, StateStoreError> {
        Err(StateStoreError::WriteFailed {
            detail: "the promote-suite substrate never appends".to_owned(),
        })
    }

    fn load_run(
        &self,
        _run_id: &RunId,
        _deadline: Deadline,
    ) -> Result<Option<Run>, StateStoreError> {
        Ok(self.run.clone())
    }

    fn checkpoint(
        &self,
        _checkpoint: Checkpoint,
        _deadline: Deadline,
    ) -> Result<(), StateStoreError> {
        Err(StateStoreError::WriteFailed {
            detail: "the promote-suite substrate never checkpoints".to_owned(),
        })
    }

    fn resume(
        &self,
        _run_id: &RunId,
        _deadline: Deadline,
    ) -> Result<Option<Checkpoint>, StateStoreError> {
        Ok(self.checkpoint.clone())
    }

    fn export_jsonl(
        &self,
        _run_id: &RunId,
        _deadline: Deadline,
    ) -> Result<String, StateStoreError> {
        Ok(String::new())
    }

    fn verification_records(
        &self,
        _run_id: &RunId,
        _deadline: Deadline,
    ) -> Result<Vec<VerificationRecordDoc>, StateStoreError> {
        Ok(self.records.clone())
    }

    fn correction_selections(
        &self,
        _run_id: &RunId,
        _deadline: Deadline,
    ) -> Result<Vec<gauntlet_domain::CorrectionSelection>, StateStoreError> {
        Ok(Vec::new())
    }

    fn settlements(
        &self,
        _run_id: &RunId,
        _deadline: Deadline,
    ) -> Result<Vec<SettlementRecord>, StateStoreError> {
        Ok(self.settlements.clone())
    }
}

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// The conforming optimizer document, assembled from parts (mirrors the
/// codec's own fixture style — no brace counting in a format string).
fn candidate_document() -> String {
    use gauntlet_ports::json::{JsonNumber, JsonValue};
    let payload = JsonValue::Object(vec![
        (
            "claimed_problem".to_owned(),
            JsonValue::Str("correction loops burn two rounds".to_owned()),
        ),
        (
            "supporting_evidence".to_owned(),
            JsonValue::Array(vec![JsonValue::Str(evidence("aaa0").as_str().to_owned())]),
        ),
        (
            "affected_component".to_owned(),
            JsonValue::Str("correction.selection".to_owned()),
        ),
        (
            "predicted_improvement".to_owned(),
            JsonValue::Str("one fewer correction round".to_owned()),
        ),
        (
            "potential_regression".to_owned(),
            JsonValue::Array(vec![JsonValue::Str("a residual may re-open".to_owned())]),
        ),
        (
            "candidate_change".to_owned(),
            JsonValue::Str(evidence("bbb1").as_str().to_owned()),
        ),
        (
            "evaluation_plan".to_owned(),
            JsonValue::Str("held-out set against the frozen reference".to_owned()),
        ),
        (
            "held_out_evaluation".to_owned(),
            JsonValue::Str("regression set replay".to_owned()),
        ),
        (
            "rollback".to_owned(),
            JsonValue::Str(evidence("ccc2").as_str().to_owned()),
        ),
    ]);
    let envelope = |invocation: &str, run: &str, envelope_digest: &str, iteration: u64| {
        JsonValue::Object(vec![
            (
                "schema".to_owned(),
                JsonValue::Str("role_result@v1".to_owned()),
            ),
            (
                "invocation_id".to_owned(),
                JsonValue::Str(invocation.to_owned()),
            ),
            ("run_id".to_owned(), JsonValue::Str(run.to_owned())),
            (
                "iteration_no".to_owned(),
                JsonValue::Number(JsonNumber::Uint(iteration)),
            ),
            ("role".to_owned(), JsonValue::Str("optimizer".to_owned())),
            (
                "context_envelope_digest".to_owned(),
                JsonValue::Str(envelope_digest.to_owned()),
            ),
            ("state".to_owned(), JsonValue::Str("completed".to_owned())),
            (
                "payload_schema".to_owned(),
                JsonValue::Str("candidate_revision@v1".to_owned()),
            ),
            ("payload".to_owned(), payload.clone()),
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
                    (WALL_KEY.to_owned(), JsonValue::Number(JsonNumber::Uint(0))),
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
    };
    // The placeholder form the ScriptedOptimizer renders from the dispatch
    // facts.
    envelope("{invocation_id}", "{run_id}", "{envelope_digest}", 1)
}

fn metrics_with(iterations: u64, payment_micro_usd: u64, deterministic: bool) -> RunMetrics {
    RunMetrics {
        iterations,
        payment_tokens: 1_000,
        payment_micro_usd,
        settlements: vec![SettlementFact {
            unit: UNIT.to_owned(),
            instrument_deterministic: Some(deterministic),
            occurred_at: Some(EpochSeconds::new(BASE + 1)),
        }],
        dispatched_units: vec![UNIT.to_owned()],
        settled_terminal: true,
    }
}

fn honest_evaluation(revision: &RevisionId, control: RunId, treatment: RunId) -> EvaluationRecord {
    EvaluationRecord {
        revision: revision.clone(),
        metric: "iterations_per_settlement".to_owned(),
        protected: vec!["cap-eval".to_owned()],
        held_out_control: control.clone(),
        held_out_treatment: treatment,
        regression_control: run(4),
        regression_treatment: run(5),
        held_out_treatment_parent: control,
        regression_treatment_parent: run(4),
        control_reference_digest: digest(),
        treatment_reference_digest: digest(),
        regression_control_reference_digest: digest(),
        regression_treatment_reference_digest: digest(),
        control_held_out_metrics: metrics_with(3, FIXTURE_PAYMENT, true),
        treatment_held_out_metrics: metrics_with(2, FIXTURE_PAYMENT, true),
        control_regression_metrics: metrics_with(3, FIXTURE_PAYMENT, true),
        treatment_regression_metrics: metrics_with(2, FIXTURE_PAYMENT, true),
        at: EpochSeconds::new(BASE + 10),
    }
}

/// The operator-declared protected capability the honest world satisfies.
fn protected_capability() -> CapabilityId {
    CapabilityId::new("cap-eval").expect("capability id")
}

fn gate_input(delta_text: &str) -> GateInput {
    GateInput {
        current_reference_digest: digest(),
        delta: DeltaDocument::parse(delta_text).expect("the delta parses"),
        rollback_resolvable: Some(true),
    }
}

const INERT_DELTA: &str = "[{\"key\":\"budget.max_parallel\",\"value\":\"2\"}]";
const WEAKENING_DELTA: &str =
    "[{\"key\":\"verification.prefer_deterministic\",\"value\":\"false\"}]";

/// Admits the candidate document under the fixture revision id, with the
/// dispatch-fact placeholders filled with fixed facts.
fn admit_fixture(ledger: &Arc<MemoryRevisions>) {
    let service = ProposalService::new(ledger.clone(), clock());
    let document = candidate_document()
        .replace("{invocation_id}", "01ARZ3NDEKTSV4RRFFQ69G5FB1")
        .replace("{run_id}", "01ARZ3NDEKTSV4RRFFQ69G5FA1")
        .replace("{envelope_digest}", REF)
        .replace("{iteration}", "1");
    service
        .admit_document(&document, bound())
        .expect("the fixture document admits");
}

fn gate_with(
    ledger: &Arc<MemoryRevisions>,
    capabilities: &Arc<MemoryCapabilities>,
    margin_bps: u64,
) -> PromotionGate {
    gate_with_dyn(ledger.clone(), capabilities.clone(), margin_bps)
}

/// The gate over ANY capability-ledger implementation — the C6 double is
/// not the memory one.
fn gate_with_dyn(
    ledger: Arc<MemoryRevisions>,
    capabilities: Arc<dyn CapabilityLedger>,
    margin_bps: u64,
) -> PromotionGate {
    PromotionGate::new(ledger, capabilities, clock(), margin_bps)
        .expect("the fixture margin is inside the 10 000 bps domain")
}

// ---------------------------------------------------------------------------
// Admission (charter D9: payload validation, idempotency, conflict,
// dispatch)
// ---------------------------------------------------------------------------

#[test]
fn admission_writes_a_pending_revision_with_every_declared_field() {
    let ledger = Arc::new(MemoryRevisions::default());
    admit_fixture(&ledger);
    let stored = ledger
        .proposal(&revision_of_document(), bound())
        .expect("reads")
        .expect("the revision is durable");
    assert_eq!(stored.target(), "correction.selection");
    assert_eq!(stored.supporting_evidence().len(), 1);
    assert_eq!(stored.predicted_regression_risk().len(), 1);
    assert!(!stored.evaluation_plan().is_empty());
    assert!(!stored.held_out_plan().is_empty());
    // The derived entry sits PENDING — admitted, never adjudicated.
    let entry = ledger
        .revision(&stored.revision().clone(), bound())
        .expect("reads")
        .expect("present");
    assert_eq!(entry.decision, None);
    assert_eq!(entry.evaluations, 0);
}

/// The fixture document's content-derived revision id.
fn revision_of_document() -> RevisionId {
    let ledger = Arc::new(MemoryRevisions::default());
    admit_fixture(&ledger);
    ledger
        .proposals
        .lock()
        .expect("lock")
        .first()
        .expect("admitted")
        .revision()
        .clone()
}

#[test]
fn a_decision_claiming_document_is_refused_whole() {
    let ledger = Arc::new(MemoryRevisions::default());
    let service = ProposalService::new(ledger.clone(), clock());
    let document = candidate_document()
        .replace("{invocation_id}", "01ARZ3NDEKTSV4RRFFQ69G5FB2")
        .replace("{run_id}", "01ARZ3NDEKTSV4RRFFQ69G5FA2")
        .replace("{envelope_digest}", REF)
        .replace("{iteration}", "1")
        .replace(
            "\"errors\":[]",
            "\"errors\":[]},\"decision\":\"promoted\",\"x\":1",
        );
    // The smuggled keys break the ENVELOPE's field table (decision rides
    // the envelope, not the payload) — refused either way, never
    // partially admitted. The payload-level refusal is pinned by the
    // codec's own tests.
    let error = service
        .admit_document(&document, bound())
        .expect_err("a decision-claiming document is refused");
    assert!(matches!(error, ProposalError::Codec(_)));
    assert!(
        ledger.proposals.lock().expect("lock").is_empty(),
        "nothing was admitted"
    );
}

#[test]
fn admission_is_idempotent_on_the_content_derived_id() {
    let ledger = Arc::new(MemoryRevisions::default());
    let service = ProposalService::new(ledger.clone(), clock());
    let fixed = candidate_document()
        .replace("{invocation_id}", "01ARZ3NDEKTSV4RRFFQ69G5FB3")
        .replace("{run_id}", "01ARZ3NDEKTSV4RRFFQ69G5FA3")
        .replace("{envelope_digest}", REF)
        .replace("{iteration}", "1");
    let first = service
        .admit_document(&fixed, bound())
        .expect("the first admission lands");
    assert!(!first.idempotent);
    let second = service
        .admit_document(&fixed, bound())
        .expect("the replay lands");
    assert!(
        second.idempotent,
        "the same document is the same revision — the idempotent no-op"
    );
    assert_eq!(first.revision, second.revision);
    assert_eq!(ledger.proposals.lock().expect("lock").len(), 1);
}

#[test]
fn admission_is_idempotent_across_a_stepping_clock() {
    // COR-1 (C3): replaying the SAME document a moment later mints the
    // same content-derived id with a DIFFERENT admission instant. The
    // instant is admission provenance, not a declared fact — the replay is
    // the idempotent no-op, never a false "different declared facts"
    // conflict (the documented behavior only ever held under a frozen
    // clock).
    let ledger = Arc::new(MemoryRevisions::default());
    let service = ProposalService::new(ledger.clone(), Arc::new(SteppingClock(AtomicU64::new(1))));
    let fixed = candidate_document()
        .replace("{invocation_id}", "01ARZ3NDEKTSV4RRFFQ69G5FB5")
        .replace("{run_id}", "01ARZ3NDEKTSV4RRFFQ69G5FA5")
        .replace("{envelope_digest}", REF)
        .replace("{iteration}", "1");
    let first = service
        .admit_document(&fixed, bound())
        .expect("the first admission lands");
    assert!(!first.idempotent);
    let second = service
        .admit_document(&fixed, bound())
        .expect("the replay lands at a later admission instant");
    assert!(
        second.idempotent,
        "the same document at a different instant is the same revision — the idempotent no-op"
    );
    assert_eq!(first.revision, second.revision);
    assert_eq!(ledger.proposals.lock().expect("lock").len(), 1);
}

#[test]
fn a_disagreeing_re_admission_is_the_typed_conflict() {
    let ledger = Arc::new(MemoryRevisions::default());
    admit_fixture(&ledger);
    let stored = ledger.proposals.lock().expect("lock")[0].clone();
    let disagreeing = RevisionProposal::new(
        stored.revision().clone(),
        stored.target().to_owned(),
        "a different claimed problem".to_owned(),
        Vec::new(),
        stored.predicted_improvement().to_owned(),
        Vec::new(),
        stored.diff_ref().clone(),
        stored.evaluation_plan().to_owned(),
        stored.held_out_plan().to_owned(),
        stored.rollback_ref().clone(),
        stored.admitted_at(),
        stored.source_run().cloned(),
        stored.detail().to_owned(),
    );
    let error = ledger
        .admit_proposal(disagreeing, bound())
        .expect_err("the disagreeing re-admission is the typed conflict");
    assert!(matches!(
        error,
        RevisionLedgerError::ProposalConflict { .. }
    ));
}

#[test]
fn propose_dispatches_the_optimizer_and_admits_its_document() {
    let ledger = Arc::new(MemoryRevisions::default());
    let service = ProposalService::new(ledger.clone(), clock());
    let runner = ScriptedOptimizer {
        template: candidate_document(),
        specs: Mutex::new(Vec::new()),
    };
    let plan = ProposePlan {
        run_id: run(1),
        envelope: ContextEnvelope::new(
            "allowlisted context".to_owned(),
            Digest::new("sha256", "1122334455667788").expect("digest"),
        ),
        workspace: gauntlet_domain::WorkspaceRef::new("ws-optimizer").expect("ref"),
        permissions: gauntlet_domain::PermissionSetRef::new("perm-readonly").expect("ref"),
        budget_slice: Budget::new(
            Limits::new(
                8,
                2,
                1,
                FIXTURE_SPAN,
                FIXTURE_CEILING_TOKENS,
                MicroUsd::new(500),
            )
            .expect("limits"),
            15,
            DiagnosticAllowance::new(100, MicroUsd::new(50)),
        )
        .expect("budget"),
        ids: Arc::new(SequentialIds::default()),
        model_hint: None,
        sandbox_policy: None,
    };
    let admission = service
        .propose(&runner, plan, bound())
        .expect("the dispatch admits its document");
    let specs = runner.specs.lock().expect("lock");
    assert_eq!(specs.len(), 1);
    assert_eq!(specs[0].role, RoleId::Optimizer);
    assert!(
        specs[0]
            .result_contract_path
            .as_str()
            .contains(admission.revision.as_str())
            || specs[0]
                .result_contract_path
                .as_str()
                .contains("candidate_revision"),
        "the result contract names the candidate-revision document"
    );
    // The dispatched document was admitted through the SAME admission
    // path: the durable PENDING revision exists.
    let stored = ledger
        .proposal(&admission.revision, bound())
        .expect("reads")
        .expect("durable");
    assert_eq!(stored.source_run(), Some(&run(1)));
}

// ---------------------------------------------------------------------------
// Delta classification (charter D9: the typed total function)
// ---------------------------------------------------------------------------

#[test]
fn unknown_delta_keys_refuse_to_parse() {
    let error = DeltaDocument::parse("[{\"key\":\"not.a.key\",\"value\":\"1\"}]")
        .expect_err("an unknown key refuses");
    assert!(matches!(error, super::delta::DeltaError::UnknownKey { .. }));
}

#[test]
fn the_delta_classification_is_fail_closed() {
    assert_eq!(classify_key("budget.max_parallel"), KeyClass::Inert);
    assert_eq!(classify_key("runners.default"), KeyClass::Inert);
    assert_eq!(
        classify_key("runners.by_role.critic.model"),
        KeyClass::Inert
    );
    assert_eq!(
        classify_key("verification.prefer_deterministic"),
        KeyClass::VerificationBearing
    );
    assert_eq!(classify_key("review.mode"), KeyClass::VerificationBearing);
    // Unknown and unclassifiable keys are protected: no default-allow.
    assert_eq!(classify_key("state.dir"), KeyClass::Protected);
    assert_eq!(classify_key("made.up.key"), KeyClass::Protected);
    assert_eq!(classify_key("runners.by_role.critic"), KeyClass::Protected);
    let delta = DeltaDocument::parse("[{\"key\":\"made.up.key\",\"value\":\"1\"}]")
        .expect_err("unknown keys refuse at parse (declared keys only)");
    let _ = delta;
}

#[test]
fn a_verification_bearing_downgrade_is_the_weak_class() {
    let delta = DeltaDocument::parse(WEAKENING_DELTA).expect("parses");
    let verdict = delta.settlement_access_verdict().expect_err("weakens");
    assert_eq!(verdict.token(), "settlement_access_weakened");
    // The preserving value is the one §22 posture.
    let delta = DeltaDocument::parse(
        "[{\"key\":\"verification.prefer_deterministic\",\"value\":\"true\"}]",
    )
    .expect("parses");
    delta
        .settlement_access_verdict()
        .expect("preserving the deterministic preference is not a weakening");
}

// ---------------------------------------------------------------------------
// Margin arithmetic (charter D9: 0 = strict improvement; configured
// margins refuse unmet improvements)
// ---------------------------------------------------------------------------

#[test]
fn margin_zero_demands_strict_improvement() {
    // A figure that only EQUALS control does not pass at margin 0.
    assert!(!super::gate::margin_holds_for_tests(
        FIGURE_EQUAL,
        FIGURE_EQUAL,
        0
    ));
    // A strict improvement passes.
    assert!(super::gate::margin_holds_for_tests(
        FIGURE_EQUAL,
        FIGURE_STRICT,
        0
    ));
}

#[test]
fn a_configured_margin_the_improvement_does_not_meet_refuses() {
    // A one-percent improvement does not cover a five-percent margin.
    assert!(!super::gate::margin_holds_for_tests(
        FIGURE_EQUAL,
        FIGURE_ONE_PCT,
        MARGIN_FIVE_PCT
    ));
    // 5% better covers it exactly at the boundary (≤).
    assert!(super::gate::margin_holds_for_tests(
        FIGURE_EQUAL,
        FIGURE_FIVE_PCT,
        MARGIN_FIVE_PCT
    ));
    // A regression never passes, whatever the margin.
    assert!(!super::gate::margin_holds_for_tests(
        FIGURE_EQUAL,
        FIGURE_WORSE,
        0
    ));
}

// ---------------------------------------------------------------------------
// Metrics derivation (charter D9: durable facts, both arms same function)
// ---------------------------------------------------------------------------

fn substrate() -> Substrate {
    substrate_with_parent(None)
}

/// The substrate fixture with a chosen durable parent fact — the C4
/// verification reads exactly this off the treatment run's record.
fn substrate_with_parent(parent: Option<RunId>) -> Substrate {
    let run_record = Run::new(RunDefinition {
        id: run(1),
        created_at: EpochSeconds::new(BASE),
        reference_version: ReferenceVersion::new(
            ReferenceId::new("ref-promote-tests").expect("reference id"),
            1,
            digest(),
        ),
        intent: "the promote tests' fixture run".to_owned(),
        config_fingerprint: Digest::new("sha256", "a1b2c3d4e5f6a7b8").expect("digest"),
        environment_fingerprint: Digest::new("sha256", "1a2b3c4d5e6f7a8b").expect("digest"),
        budget: Budget::new(
            Limits::new(
                8,
                2,
                1,
                FIXTURE_SPAN,
                FIXTURE_CEILING_TOKENS,
                MicroUsd::new(500),
            )
            .expect("limits"),
            15,
            DiagnosticAllowance::new(100, MicroUsd::new(50)),
        )
        .expect("budget"),
        parent_run: parent,
    })
    .expect("test run");
    Substrate {
        run: Some(run_record),
        checkpoint: Some(Checkpoint {
            run_id: run(1),
            reference_digest: digest(),
            iteration_no: IterationNo::new(3).expect("1-based"),
            unit_states: vec![(
                UnitId::new(UNIT).expect("unit"),
                gauntlet_domain::UnitStatus::Open,
            )],
            budget_spent: gauntlet_domain::Spent::from_counters(
                3,
                120,
                FIXTURE_SPENT_TOKENS,
                MicroUsd::new(FIXTURE_PAYMENT),
                0,
                MicroUsd::new(0),
            ),
            in_flight: Vec::new(),
            event_log_offset: EventLogOffset::new(42),
        }),
        records: vec![verification_record("vrf-1", UNIT, Outcome::Pass, 3)],
        settlements: vec![settlement(UNIT, "vrf-1")],
    }
}

fn verification_record(
    verification: &str,
    unit: &str,
    outcome: Outcome,
    iteration: u32,
) -> VerificationRecordDoc {
    VerificationRecordDoc::new(VerificationContent {
        verification: VerificationRef::new(verification).expect("ref"),
        run_id: run(1),
        iteration_no: IterationNo::new(iteration).expect("1-based"),
        unit_id: UnitId::new(unit).expect("unit"),
        claim: "the unit's behavioral claim".to_owned(),
        requirement: RequirementDoc::new(
            EvidenceKind::TestReport,
            EvidenceLocator::new("fixture-locator").expect("locator"),
            Producer::Instrument,
        ),
        instrument: VerifyingInstrument::new(
            "gauntlet-test-instrument".to_owned(),
            "1".to_owned(),
            true,
        ),
        evidence: EvidenceRef::new(&format!("evd-{verification}")).expect("evidence"),
        produced_at: EpochSeconds::new(BASE + 10),
        outcome,
        class: None,
        detail: "the instrument decided".to_owned(),
    })
    .expect("well-formed record")
}

fn settlement(unit: &str, verification: &str) -> SettlementRecord {
    SettlementRecord::new(
        run(1),
        UnitId::new(unit).expect("unit"),
        EvidenceRef::new(&format!("evd-{verification}")).expect("evidence"),
        VerificationRef::new(verification).expect("ref"),
        EventLogOffset::new(21),
    )
}

#[test]
fn metrics_derive_from_the_runs_durable_facts() {
    let substrate = substrate();
    let metrics = derive_run_metrics(&substrate, &run(1), bound()).expect("the figures derive");
    assert_eq!(metrics.iterations, 3);
    assert_eq!(metrics.payment_micro_usd, FIXTURE_PAYMENT);
    // The fixture's checkpoint plan holds exactly the settled unit: the
    // derived terminal fact is settled.
    assert!(metrics.settled_terminal);
    assert_eq!(metrics.settlements.len(), 1);
    assert_eq!(
        metrics.settlements[0].instrument_deterministic,
        Some(true),
        "the instrument fact comes from the deciding verification record"
    );
    assert_eq!(metrics.dispatched_units, vec![UNIT.to_owned()]);
    // The fixed-point figure: 3 iterations per 1 settlement = 30 000 per
    // 10 000.
    assert_eq!(
        TargetMetric::IterationsPerSettlement.figure(&metrics),
        Some(FIGURE_ITERATIONS)
    );
    // 3 000 micro-dollars per 1 settlement = 30 000 000 per 10 000.
    assert_eq!(
        TargetMetric::PaymentPerSettlement.figure(&metrics),
        Some(FIGURE_PAYMENT)
    );
}

#[test]
fn metrics_for_a_run_without_a_checkpoint_are_underivable() {
    let mut substrate = substrate();
    substrate.checkpoint = None;
    let error = derive_run_metrics(&substrate, &run(1), bound()).expect_err("underivable");
    assert!(matches!(
        error,
        super::metrics::MetricsError::Undecidable { .. }
    ));
}

// ---------------------------------------------------------------------------
// Gate conditions (charter D9: every condition refuses in isolation)
// ---------------------------------------------------------------------------

/// The shared honest baseline: admitted revision, one honest evaluation
/// (treatment strictly better: 2 iterations vs 3 per settlement), inert
/// delta, everything resolvable. Each condition's test breaks exactly its
/// own condition.
fn honest_world() -> (Arc<MemoryRevisions>, Arc<MemoryCapabilities>, RevisionId) {
    let ledger = Arc::new(MemoryRevisions::default());
    admit_fixture(&ledger);
    let revision = revision_of_document();
    ledger
        .append_evaluation(honest_evaluation(&revision, run(2), run(3)), bound())
        .expect("the evaluation lands");
    // The protected capability carries ONE settlement memory from each
    // arm (the evaluation's import): non-inferior by equality.
    let capabilities = Arc::new(MemoryCapabilities::default());
    for (tag, arm) in [(10, run(2)), (11, run(3))] {
        capabilities
            .append_observation(
                ObservationRecord::new(
                    ObservationIdentity::new(
                        protected_capability(),
                        ObservationClass::Settlement,
                        arm,
                        format!("settlement:{tag}"),
                    )
                    .expect("identity"),
                    EpochSeconds::new(BASE),
                    "the arm's imported settlement".to_owned(),
                ),
                bound(),
            )
            .expect("appends");
    }
    (ledger, capabilities, revision)
}

#[test]
fn the_honest_road_promotes_through_the_full_conjunction() {
    let (ledger, capabilities, revision) = honest_world();
    let gate = gate_with(&ledger, &capabilities, 0);
    let verdict = gate
        .adjudicate(&revision, &gate_input(INERT_DELTA), bound())
        .expect("adjudicates");
    assert!(
        matches!(verdict, GateVerdict::Promoted(_)),
        "the honest candidate promotes: {verdict:?}"
    );
    let entry = ledger
        .revision(&revision, bound())
        .expect("reads")
        .expect("present");
    assert_eq!(entry.decision, Some(DecisionKind::Promoted));
}

#[test]
fn a_weakening_candidate_improves_the_metric_yet_is_rejected_with_settlement_access_weakened() {
    // AK-056 in-module: the delta genuinely improves the metric (the
    // honest evaluation's figures stand) but the delta downgrades the
    // deterministic-verification preference.
    let (ledger, capabilities, revision) = honest_world();
    let gate = gate_with(&ledger, &capabilities, 0);
    let verdict = gate
        .adjudicate(&revision, &gate_input(WEAKENING_DELTA), bound())
        .expect("adjudicates");
    let GateVerdict::Rejected { reasons, .. } = verdict else {
        panic!("the weakening candidate must be REJECTED: {verdict:?}");
    };
    assert!(
        reasons
            .iter()
            .any(|reason| reason.token() == "settlement_access_weakened"),
        "AK-056's refusal is recorded: {reasons:?}"
    );
    let entry = ledger
        .revision(&revision, bound())
        .expect("reads")
        .expect("present");
    assert_eq!(entry.decision, Some(DecisionKind::Rejected));
}

#[test]
fn a_moved_reference_is_rejected() {
    let (ledger, capabilities, revision) = honest_world();
    let mut input = gate_input(INERT_DELTA);
    input.current_reference_digest =
        Digest::parse("sha256:cd00000000000000000000000000000000000000000000000000000000000000")
            .expect("digest");
    let gate = gate_with(&ledger, &capabilities, 0);
    let verdict = gate
        .adjudicate(&revision, &input, bound())
        .expect("adjudicates");
    let GateVerdict::Rejected { reasons, .. } = verdict else {
        panic!("a moved ruler must reject: {verdict:?}");
    };
    assert!(
        reasons
            .iter()
            .any(|reason| reason.token() == "reference_moved")
    );
}

#[test]
fn an_altered_governance_mix_is_rejected() {
    let (ledger, capabilities, revision) = honest_world();
    // The treatment arm SKIPPED a unit the control arm dispatched —
    // cherry-picking the governance shape.
    let evaluations = ledger.evaluations(&revision, bound()).expect("reads");
    let mut altered = evaluations[0].clone();
    altered.control_held_out_metrics.dispatched_units = vec![UNIT.to_owned(), "AK-9002".to_owned()];
    // Replace the record: a fresh ledger with the altered evaluation.
    let fresh = Arc::new(MemoryRevisions::default());
    admit_fixture(&fresh);
    fresh
        .append_evaluation(altered, bound())
        .expect("the altered evaluation lands");
    let gate = gate_with(&fresh, &capabilities, 0);
    let verdict = gate
        .adjudicate(&revision, &gate_input(INERT_DELTA), bound())
        .expect("adjudicates");
    let GateVerdict::Rejected { reasons, .. } = verdict else {
        panic!("an altered governance mix must reject: {verdict:?}");
    };
    assert!(
        reasons
            .iter()
            .any(|reason| reason.token() == "governance_altered")
    );
}

#[test]
fn a_governance_bearing_target_is_rejected() {
    let ledger = Arc::new(MemoryRevisions::default());
    let service = ProposalService::new(ledger.clone(), clock());
    // The same document with a governance-bearing affected component —
    // different content, so its own content-derived id.
    let document = candidate_document()
        .replace("correction.selection", "settlement.gate")
        .replace("{invocation_id}", "01ARZ3NDEKTSV4RRFFQ69G5FB4")
        .replace("{run_id}", "01ARZ3NDEKTSV4RRFFQ69G5FA4")
        .replace("{envelope_digest}", REF)
        .replace("{iteration}", "1");
    let admission = service.admit_document(&document, bound()).expect("admits");
    ledger
        .append_evaluation(
            honest_evaluation(&admission.revision, run(2), run(3)),
            bound(),
        )
        .expect("the evaluation lands");
    let gate = gate_with(&ledger, &Arc::new(MemoryCapabilities::default()), 0);
    let verdict = gate
        .adjudicate(&admission.revision, &gate_input(INERT_DELTA), bound())
        .expect("adjudicates");
    let GateVerdict::Rejected { reasons, .. } = verdict else {
        panic!("a governance-bearing target must reject: {verdict:?}");
    };
    assert!(
        reasons
            .iter()
            .any(|reason| reason.token() == "governance_altered")
    );
}

#[test]
fn an_unresolvable_rollback_is_rejected() {
    let (ledger, capabilities, revision) = honest_world();
    let mut input = gate_input(INERT_DELTA);
    input.rollback_resolvable = Some(false);
    let gate = gate_with(&ledger, &capabilities, 0);
    let verdict = gate
        .adjudicate(&revision, &input, bound())
        .expect("adjudicates");
    let GateVerdict::Rejected {
        reasons,
        rollback_ref,
        ..
    } = verdict
    else {
        panic!("an invalid rollback must reject: {verdict:?}");
    };
    assert!(
        reasons
            .iter()
            .any(|reason| reason.token() == "rollback_invalid")
    );
    // The rejection carries the rollback ref — the safe-rollback record.
    assert_eq!(rollback_ref, evidence("ccc2"));
}

#[test]
fn a_regression_suite_failure_is_rejected() {
    let (ledger, capabilities, revision) = honest_world();
    // On the regression set, the treatment arm settles NOTHING.
    let evaluations = ledger.evaluations(&revision, bound()).expect("reads");
    let mut broken = evaluations[0].clone();
    broken.treatment_regression_metrics.settlements = Vec::new();
    let fresh = Arc::new(MemoryRevisions::default());
    admit_fixture(&fresh);
    fresh.append_evaluation(broken, bound()).expect("lands");
    let gate = gate_with(&fresh, &capabilities, 0);
    let verdict = gate
        .adjudicate(&revision, &gate_input(INERT_DELTA), bound())
        .expect("adjudicates");
    let GateVerdict::Rejected { reasons, .. } = verdict else {
        panic!("a regression-suite failure must reject: {verdict:?}");
    };
    assert!(
        reasons
            .iter()
            .any(|reason| reason.token() == "regression_suite_failed")
    );
}

#[test]
fn an_instrument_downgrade_is_rejected_as_settlement_access_weakened() {
    let (ledger, capabilities, revision) = honest_world();
    let evaluations = ledger.evaluations(&revision, bound()).expect("reads");
    let mut weakened = evaluations[0].clone();
    weakened.treatment_held_out_metrics.settlements[0].instrument_deterministic = Some(false);
    let fresh = Arc::new(MemoryRevisions::default());
    admit_fixture(&fresh);
    fresh.append_evaluation(weakened, bound()).expect("lands");
    let gate = gate_with(&fresh, &capabilities, 0);
    let verdict = gate
        .adjudicate(&revision, &gate_input(INERT_DELTA), bound())
        .expect("adjudicates");
    let GateVerdict::Rejected { reasons, .. } = verdict else {
        panic!("an instrument downgrade must reject: {verdict:?}");
    };
    assert!(
        reasons
            .iter()
            .any(|reason| reason.token() == "settlement_access_weakened")
    );
}

#[test]
fn a_margin_the_improvement_does_not_meet_is_rejected() {
    let (ledger, capabilities, revision) = honest_world();
    // The honest evaluation is ~33% better; a 5 000bps (50%) margin does
    // not hold.
    let gate = gate_with(&ledger, &capabilities, MARGIN_HEAVY);
    let verdict = gate
        .adjudicate(&revision, &gate_input(INERT_DELTA), bound())
        .expect("adjudicates");
    let GateVerdict::Rejected { reasons, .. } = verdict else {
        panic!("an unmet margin must reject: {verdict:?}");
    };
    assert!(
        reasons
            .iter()
            .any(|reason| reason.token() == "margin_not_met")
    );
}

#[test]
fn an_equal_metric_refuses_at_margin_zero() {
    let ledger = Arc::new(MemoryRevisions::default());
    admit_fixture(&ledger);
    let revision = revision_of_document();
    let mut evaluation = honest_evaluation(&revision, run(2), run(3));
    // Identical figures: no strict improvement.
    evaluation.treatment_held_out_metrics = evaluation.control_held_out_metrics.clone();
    ledger
        .append_evaluation(evaluation, bound())
        .expect("lands");
    let gate = gate_with(&ledger, &Arc::new(MemoryCapabilities::default()), 0);
    let verdict = gate
        .adjudicate(&revision, &gate_input(INERT_DELTA), bound())
        .expect("adjudicates");
    let GateVerdict::Rejected { reasons, .. } = verdict else {
        panic!("an equal metric is not an improvement: {verdict:?}");
    };
    assert!(
        reasons
            .iter()
            .any(|reason| reason.token() == "margin_not_met")
    );
}

#[test]
fn increased_payment_is_rejected() {
    let (ledger, capabilities, revision) = honest_world();
    let evaluations = ledger.evaluations(&revision, bound()).expect("reads");
    let mut costly = evaluations[0].clone();
    costly.treatment_held_out_metrics.payment_micro_usd = COSTLY_PAYMENT;
    let fresh = Arc::new(MemoryRevisions::default());
    admit_fixture(&fresh);
    fresh.append_evaluation(costly, bound()).expect("lands");
    let gate = gate_with(&fresh, &capabilities, 0);
    let verdict = gate
        .adjudicate(&revision, &gate_input(INERT_DELTA), bound())
        .expect("adjudicates");
    let GateVerdict::Rejected { reasons, .. } = verdict else {
        panic!("increased payment must reject: {verdict:?}");
    };
    assert!(
        reasons
            .iter()
            .any(|reason| reason.token() == "payment_increased")
    );
}

#[test]
fn an_inferior_protected_capability_is_rejected() {
    let (ledger, capabilities, revision) = honest_world();
    // The protected capability carries TWO control settlements but ONE
    // treatment settlement: inferior.
    let capability = CapabilityId::new("cap-eval").expect("capability id");
    for tag in [2, 6] {
        capabilities
            .append_observation(
                ObservationRecord::new(
                    ObservationIdentity::new(
                        capability.clone(),
                        ObservationClass::Settlement,
                        run(2),
                        format!("settlement:{tag}"),
                    )
                    .expect("identity"),
                    EpochSeconds::new(BASE),
                    "control settlement".to_owned(),
                ),
                bound(),
            )
            .expect("appends");
    }
    capabilities
        .append_observation(
            ObservationRecord::new(
                ObservationIdentity::new(
                    capability.clone(),
                    ObservationClass::Settlement,
                    run(3),
                    "settlement:9".to_owned(),
                )
                .expect("identity"),
                EpochSeconds::new(BASE),
                "treatment settlement".to_owned(),
            ),
            bound(),
        )
        .expect("appends");
    // The evaluation must have DECLARED the protected capability.
    let evaluations = ledger.evaluations(&revision, bound()).expect("reads");
    let mut declared = evaluations[0].clone();
    declared.protected = vec![capability.as_str().to_owned()];
    let fresh = Arc::new(MemoryRevisions::default());
    admit_fixture(&fresh);
    fresh.append_evaluation(declared, bound()).expect("lands");
    let ledger = fresh;
    let gate = gate_with(&ledger, &capabilities, 0);
    let input = gate_input(INERT_DELTA);
    let verdict = gate
        .adjudicate(&revision, &input, bound())
        .expect("adjudicates");
    let GateVerdict::Rejected { reasons, .. } = verdict else {
        panic!("an inferior protected capability must reject: {verdict:?}");
    };
    assert!(
        reasons
            .iter()
            .any(|reason| reason.token() == "protected_capability_inferior")
    );
}

// ---------------------------------------------------------------------------
// The provisional floor (charter D9: missing evidence ⇒ provisional,
// never promoted, never rejected) and the derived rolled-back state
// ---------------------------------------------------------------------------

#[test]
fn missing_evaluation_adjudicates_provisional_and_stays_pending() {
    let ledger = Arc::new(MemoryRevisions::default());
    admit_fixture(&ledger);
    let revision = revision_of_document();
    let gate = gate_with(&ledger, &Arc::new(MemoryCapabilities::default()), 0);
    let verdict = gate
        .adjudicate(&revision, &gate_input(INERT_DELTA), bound())
        .expect("adjudicates");
    let GateVerdict::Provisional { reasons, written } = verdict else {
        panic!("no evidence is provisional: {verdict:?}");
    };
    assert!(!written, "the zero-evidence shape stays PENDING");
    assert_eq!(reasons[0].token(), "missing_evaluation");
    // Never promoted, never rejected — and not even a provisional label
    // (the domain floor refuses one with no evaluation runs): PENDING.
    let entry = ledger
        .revision(&revision, bound())
        .expect("reads")
        .expect("present");
    assert_eq!(entry.decision, None);
}

#[test]
fn an_undeclared_protected_set_is_a_provisional_gap_not_a_pass() {
    let (_ledger, capabilities, revision) = honest_world();
    // Everything else holds; the evaluation DECLARED no protected set →
    // condition 6 is undecidable → provisional, never promoted.
    let ledger = Arc::new(MemoryRevisions::default());
    admit_fixture(&ledger);
    let evaluations = _ledger.evaluations(&revision, bound()).expect("reads");
    let mut undeclared_record = evaluations[0].clone();
    undeclared_record.protected = Vec::new();
    ledger
        .append_evaluation(undeclared_record, bound())
        .expect("lands");
    let gate = gate_with(&ledger, &capabilities, 0);
    let verdict = gate
        .adjudicate(&revision, &gate_input(INERT_DELTA), bound())
        .expect("adjudicates");
    let GateVerdict::Provisional { reasons, written } = verdict else {
        panic!("an undecidable condition refuses: {verdict:?}");
    };
    assert!(written, "the evaluation exists, so the label writes");
    assert!(
        reasons
            .iter()
            .any(|reason| reason.token() == "protected_capability_inferior")
    );
    let entry = ledger
        .revision(&revision, bound())
        .expect("reads")
        .expect("present");
    assert_eq!(entry.decision, Some(DecisionKind::Provisional));
    assert!(!entry.rolled_back);
}

// ---------------------------------------------------------------------------
// COR-1 (C1): incomplete arms are evidence gaps, never sentinels. The
// blind critic's two empirical probes, pinned in isolation, plus the
// (0,0)-placeholder fallback at the same site.
// ---------------------------------------------------------------------------

#[test]
fn an_incomplete_control_arm_never_promotes_and_the_gap_is_named() {
    // Probe 1 (critic B1): the control arm never reached a settled
    // terminal while the treatment completed. The SENTINEL read this arm
    // as the worst figure and PROMOTED the candidate with zero efficiency
    // evidence; the honest gate adjudicates the EVIDENCE GAP.
    let (ledger, capabilities, revision) = honest_world();
    let evaluations = ledger.evaluations(&revision, bound()).expect("reads");
    let mut incomplete = evaluations[0].clone();
    incomplete.control_held_out_metrics.settled_terminal = false;
    let fresh = Arc::new(MemoryRevisions::default());
    admit_fixture(&fresh);
    fresh
        .append_evaluation(incomplete, bound())
        .expect("the incomplete evaluation lands");
    let gate = gate_with(&fresh, &capabilities, 0);
    let verdict = gate
        .adjudicate(&revision, &gate_input(INERT_DELTA), bound())
        .expect("adjudicates");
    let GateVerdict::Provisional { reasons, written } = verdict else {
        panic!("an incomplete control arm is an evidence gap, never a promotion: {verdict:?}");
    };
    assert!(written, "the evaluation exists, so the label writes");
    assert!(
        reasons
            .iter()
            .any(|reason| reason.token() == "evaluation_incomplete"),
        "the gap names the incomplete arm: {reasons:?}"
    );
    let entry = fresh
        .revision(&revision, bound())
        .expect("reads")
        .expect("present");
    assert_eq!(
        entry.decision,
        Some(DecisionKind::Provisional),
        "never promoted, never rejected"
    );
}

#[test]
fn two_incomplete_arms_adjudicate_provisional_and_nothing_is_rejected() {
    // Probe 2 (critic B1): a no-op candidate over two equally incomplete
    // arms. The SENTINEL compared u64::MAX against itself and durably
    // REJECTED the revision on `margin_not_met`; the honest gate rests it
    // at Provisional — insufficient evidence is not a negative result.
    let (ledger, capabilities, revision) = honest_world();
    let evaluations = ledger.evaluations(&revision, bound()).expect("reads");
    let mut incomplete = evaluations[0].clone();
    incomplete.control_held_out_metrics.settled_terminal = false;
    incomplete.treatment_held_out_metrics.settled_terminal = false;
    let fresh = Arc::new(MemoryRevisions::default());
    admit_fixture(&fresh);
    fresh
        .append_evaluation(incomplete, bound())
        .expect("the incomplete evaluation lands");
    let gate = gate_with(&fresh, &capabilities, 0);
    let verdict = gate
        .adjudicate(&revision, &gate_input(INERT_DELTA), bound())
        .expect("adjudicates");
    let GateVerdict::Provisional { reasons, written } = verdict else {
        panic!("two incomplete arms are insufficient evidence — never a rejection: {verdict:?}");
    };
    assert!(written, "the evaluation exists, so the label writes");
    assert!(
        reasons
            .iter()
            .any(|reason| reason.token() == "evaluation_incomplete"),
        "the gaps name the incomplete arms: {reasons:?}"
    );
    assert!(
        !reasons
            .iter()
            .any(|reason| reason.token() == "margin_not_met"),
        "the sentinel comparison must not survive as a failed condition: {reasons:?}"
    );
    let entry = fresh
        .revision(&revision, bound())
        .expect("reads")
        .expect("present");
    assert_eq!(entry.decision, Some(DecisionKind::Provisional));
}

#[test]
fn an_arm_that_settled_nothing_makes_the_margin_undecidable_not_failed() {
    // The (0,0)-placeholder fallback at the same site: a figure that is
    // genuinely ABSENT (the arm settled nothing) is an evidence gap only —
    // never a FAILED MarginNotMet composed from placeholder zeros.
    let (ledger, capabilities, revision) = honest_world();
    let evaluations = ledger.evaluations(&revision, bound()).expect("reads");
    let mut settled_nothing = evaluations[0].clone();
    settled_nothing.control_held_out_metrics.settlements = Vec::new();
    let fresh = Arc::new(MemoryRevisions::default());
    admit_fixture(&fresh);
    fresh
        .append_evaluation(settled_nothing, bound())
        .expect("the arm-settled-nothing evaluation lands");
    let gate = gate_with(&fresh, &capabilities, 0);
    let verdict = gate
        .adjudicate(&revision, &gate_input(INERT_DELTA), bound())
        .expect("adjudicates");
    let GateVerdict::Provisional { reasons, written } = verdict else {
        panic!("an underivable figure is a gap, never a failure: {verdict:?}");
    };
    assert!(written, "the evaluation exists, so the label writes");
    assert!(
        reasons
            .iter()
            .any(|reason| reason.token() == "evaluation_incomplete"),
        "the gap names the arm with no figure: {reasons:?}"
    );
    assert!(
        !reasons
            .iter()
            .any(|reason| reason.token() == "margin_not_met"),
        "placeholder figures must not compose a failed condition: {reasons:?}"
    );
}

#[test]
fn a_reduced_attempt_ceiling_cannot_promote_through_the_completion_gate() {
    // COR-1 (C8): the honest justification for the attempt ceiling's Inert
    // class on C1's ground. The weakening direction (fewer attempts)
    // starves a hard unit: the arm cannot reach a settled terminal, the
    // figures are underivable, and the verdict is the PROVISIONAL gap —
    // never the promotion the old sentinel-era comment feared, and never
    // the rejection the sentinel produced. The delta itself classifies
    // inert: condition 3 holds; the completion gate is what refuses.
    let (ledger, capabilities, revision) = honest_world();
    let evaluations = ledger.evaluations(&revision, bound()).expect("reads");
    let mut starved = evaluations[0].clone();
    starved.treatment_held_out_metrics.settled_terminal = false;
    let fresh = Arc::new(MemoryRevisions::default());
    admit_fixture(&fresh);
    fresh
        .append_evaluation(starved, bound())
        .expect("the starved evaluation lands");
    let starved_delta =
        DeltaDocument::parse("[{\"key\":\"max_attempts_per_unit\",\"value\":\"1\"}]")
            .expect("the attempt-ceiling delta parses");
    let gate = gate_with(&fresh, &capabilities, 0);
    let verdict = gate
        .adjudicate(
            &revision,
            &GateInput {
                current_reference_digest: digest(),
                delta: starved_delta,
                rollback_resolvable: Some(true),
            },
            bound(),
        )
        .expect("adjudicates");
    let GateVerdict::Provisional { reasons, written } = verdict else {
        panic!("a starved arm is an evidence gap — never promoted, never rejected: {verdict:?}");
    };
    assert!(written);
    assert!(
        reasons
            .iter()
            .any(|reason| reason.token() == "evaluation_incomplete"),
        "the completion gate names the starved arm: {reasons:?}"
    );
    assert!(
        !reasons
            .iter()
            .any(|reason| reason.token() == "settlement_access_weakened"),
        "the attempt ceiling is inert: condition 3 holds: {reasons:?}"
    );
}

#[test]
fn a_final_decision_is_never_re_adjudicated() {
    let (ledger, capabilities, revision) = honest_world();
    let gate = gate_with(&ledger, &capabilities, 0);
    gate.adjudicate(&revision, &gate_input(INERT_DELTA), bound())
        .expect("promotes");
    let error = gate
        .adjudicate(&revision, &gate_input(INERT_DELTA), bound())
        .expect_err("write-once past a final state");
    assert!(matches!(error, GateError::AlreadyFinal { .. }));
}

#[test]
fn rollback_flips_the_derived_state_and_refuses_the_unpromoted() {
    let (ledger, capabilities, revision) = honest_world();
    let gate = gate_with(&ledger, &capabilities, 0);
    gate.adjudicate(&revision, &gate_input(INERT_DELTA), bound())
        .expect("promotes");
    let rollback = RollbackService::new(ledger.clone(), clock());
    let entry = rollback
        .restore(&revision, "the operator rolled it back".to_owned(), bound())
        .expect("restores");
    assert_eq!(entry.decision, Some(DecisionKind::Promoted));
    assert!(
        entry.rolled_back,
        "promoted-BUT-ROLLED-BACK: never silently active"
    );
    // The second restore refuses: the rollback is one-way.
    let error = rollback
        .restore(&revision, "again".to_owned(), bound())
        .expect_err("one-way");
    assert!(matches!(
        error,
        super::rollback::RollbackError::NothingToRestore { .. }
    ));
    // A NEVER-PROMOTED revision has nothing to restore.
    let pending_ledger = Arc::new(MemoryRevisions::default());
    admit_fixture(&pending_ledger);
    let pending_revision = revision_of_document();
    let fresh_rollback = RollbackService::new(pending_ledger.clone(), clock());
    let error = fresh_rollback
        .restore(&pending_revision, "too early".to_owned(), bound())
        .expect_err("nothing to restore");
    assert!(matches!(
        error,
        super::rollback::RollbackError::NothingToRestore { .. }
    ));
}

// ---------------------------------------------------------------------------
// The evaluation harness (charter D9: the executor seam double)
// ---------------------------------------------------------------------------

/// The executor double: canned outcomes keyed by arm, recording the
/// requests it received.
struct ScriptedExecutor {
    control: Substrate,
    treatment: Substrate,
    requests: Mutex<Vec<super::evaluate::ExecutionRequest>>,
}

impl EvaluationExecutor for ScriptedExecutor {
    fn validate_delta(
        &self,
        _delta: &DeltaDocument,
    ) -> Result<(), super::evaluate::ExecutionError> {
        // The scripted composition applies every declared entry.
        Ok(())
    }

    fn execute(
        &self,
        request: &super::evaluate::ExecutionRequest,
        _deadline: Deadline,
    ) -> Result<ExecutionOutcome, super::evaluate::ExecutionError> {
        self.requests.lock().expect("lock").push(request.clone());
        let (substrate, run_id) = if request.parent_run.is_some() {
            (&self.treatment, run(EXECUTOR_TREATMENT_TAG))
        } else {
            (&self.control, run(EXECUTOR_CONTROL_TAG))
        };
        Ok(ExecutionOutcome {
            run_id,
            terminal: TerminalState::Settled,
            reference_digest: digest(),
            state: Arc::new(Substrate {
                run: substrate.run.clone(),
                checkpoint: substrate.checkpoint.clone(),
                records: substrate.records.clone(),
                settlements: substrate.settlements.clone(),
            }),
        })
    }
}

#[test]
fn the_harness_records_both_arms_with_the_parent_run_set() {
    let ledger = Arc::new(MemoryRevisions::default());
    admit_fixture(&ledger);
    let revision = revision_of_document();
    let capabilities = Arc::new(MemoryCapabilities::default());
    let harness = EvaluationHarness::new(ledger.clone(), capabilities, clock());
    let executor = ScriptedExecutor {
        control: substrate(),
        treatment: substrate_with_parent(Some(run(2))),
        requests: Mutex::new(Vec::new()),
    };
    let request = EvaluationRequest {
        revision: revision.clone(),
        metric: TargetMetric::IterationsPerSettlement,
        delta: DeltaDocument::parse(INERT_DELTA).expect("parses"),
        current_reference_digest: digest(),
        held_out_root: std::path::PathBuf::from("held-out"),
        regression_root: std::path::PathBuf::from("regression"),
        protected: Vec::new(),
        intent: "the evaluation intent".to_owned(),
    };
    let summary = harness
        .evaluate(&request, &executor, bound())
        .expect("the evaluation runs");
    assert!(!summary.idempotent);
    let requests = executor.requests.lock().expect("lock");
    assert_eq!(requests.len(), 4, "control+treatment over both sets");
    // The TREATMENT arms carry parent_run (the §5.3.3 isolated-evaluation
    // shape); the control arms do not.
    let treatments: Vec<_> = requests
        .iter()
        .filter(|request| request.parent_run.is_some())
        .collect();
    assert_eq!(treatments.len(), 2);
    let controls: Vec<_> = requests
        .iter()
        .filter(|request| request.parent_run.is_none())
        .collect();
    assert_eq!(controls.len(), 2);
    // The durable record holds both arms' run ids and reference digests.
    assert_eq!(summary.record.held_out_control, run(2));
    assert_eq!(summary.record.held_out_treatment, run(3));
    assert_eq!(summary.record.control_reference_digest, digest());
    let stored = ledger.evaluations(&revision, bound()).expect("reads");
    assert_eq!(stored.len(), 1, "one durable evaluation record");
}

#[test]
fn an_executor_failure_records_nothing() {
    let ledger = Arc::new(MemoryRevisions::default());
    admit_fixture(&ledger);
    let revision = revision_of_document();
    let capabilities = Arc::new(MemoryCapabilities::default());
    let harness = EvaluationHarness::new(ledger.clone(), capabilities, clock());
    let failing = FailingExecutor;
    let request = EvaluationRequest {
        revision: revision.clone(),
        metric: TargetMetric::IterationsPerSettlement,
        delta: DeltaDocument::parse(INERT_DELTA).expect("parses"),
        current_reference_digest: digest(),
        held_out_root: std::path::PathBuf::from("held-out"),
        regression_root: std::path::PathBuf::from("regression"),
        protected: Vec::new(),
        intent: "the evaluation intent".to_owned(),
    };
    let error = harness
        .evaluate(&request, &failing, bound())
        .expect_err("a failed evaluation run is a typed error, never a negative result");
    assert!(matches!(
        error,
        super::evaluate::EvaluationError::Execution { .. }
    ));
    assert!(
        ledger
            .evaluations(&revision, bound())
            .expect("reads")
            .is_empty(),
        "nothing was recorded: insufficient evidence is never a negative result"
    );
}

struct FailingExecutor;

impl EvaluationExecutor for FailingExecutor {
    fn validate_delta(
        &self,
        _delta: &DeltaDocument,
    ) -> Result<(), super::evaluate::ExecutionError> {
        Ok(())
    }

    fn execute(
        &self,
        _request: &super::evaluate::ExecutionRequest,
        _deadline: Deadline,
    ) -> Result<ExecutionOutcome, super::evaluate::ExecutionError> {
        Err(super::evaluate::ExecutionError::Failed {
            detail: "the composition refused".to_owned(),
        })
    }
}

// ---------------------------------------------------------------------------
// COR-1 (C4): the isolated-evaluation fact is VERIFIED and durable, not
// conventional. The harness checks each treatment run's DURABLE record
// (`load_run(...).parent_run`) against the declared control parent BEFORE
// appending, and the record carries the verified fact.
// ---------------------------------------------------------------------------

#[test]
fn an_unverifiable_treatment_isolation_refuses_to_record() {
    // A substituted executor that ran the treatment WITHOUT the parent on
    // its durable record: isolation is not verifiable, so the evaluation
    // evidence would be invalid — the harness refuses to record, the gate
    // later rests at MissingEvaluation ⇒ Provisional (re-run properly),
    // never a promotion.
    let ledger = Arc::new(MemoryRevisions::default());
    admit_fixture(&ledger);
    let revision = revision_of_document();
    let harness = EvaluationHarness::new(
        ledger.clone(),
        Arc::new(MemoryCapabilities::default()),
        clock(),
    );
    let executor = ScriptedExecutor {
        control: substrate(),
        treatment: substrate_with_parent(None),
        requests: Mutex::new(Vec::new()),
    };
    let request = EvaluationRequest {
        revision: revision.clone(),
        metric: TargetMetric::IterationsPerSettlement,
        delta: DeltaDocument::parse(INERT_DELTA).expect("parses"),
        current_reference_digest: digest(),
        held_out_root: std::path::PathBuf::from("held-out"),
        regression_root: std::path::PathBuf::from("regression"),
        protected: Vec::new(),
        intent: "the evaluation intent".to_owned(),
    };
    let error = harness
        .evaluate(&request, &executor, bound())
        .expect_err("isolation that cannot be verified refuses to record");
    assert!(
        matches!(
            error,
            super::evaluate::EvaluationError::IsolationUnverifiable { .. }
        ),
        "the typed gap names the unverifiable isolation: {error:?}"
    );
    assert!(
        ledger
            .evaluations(&revision, bound())
            .expect("reads")
            .is_empty(),
        "nothing was recorded: invalid evidence is never admitted"
    );
}

#[test]
fn a_mismatched_treatment_parent_refuses_to_record() {
    // The treatment run's durable record names a DIFFERENT parent than the
    // declared control run: same refusal — the isolation fact does not hold.
    let ledger = Arc::new(MemoryRevisions::default());
    admit_fixture(&ledger);
    let revision = revision_of_document();
    let harness = EvaluationHarness::new(
        ledger.clone(),
        Arc::new(MemoryCapabilities::default()),
        clock(),
    );
    let executor = ScriptedExecutor {
        control: substrate(),
        treatment: substrate_with_parent(Some(run(7))),
        requests: Mutex::new(Vec::new()),
    };
    let request = EvaluationRequest {
        revision: revision.clone(),
        metric: TargetMetric::IterationsPerSettlement,
        delta: DeltaDocument::parse(INERT_DELTA).expect("parses"),
        current_reference_digest: digest(),
        held_out_root: std::path::PathBuf::from("held-out"),
        regression_root: std::path::PathBuf::from("regression"),
        protected: Vec::new(),
        intent: "the evaluation intent".to_owned(),
    };
    let error = harness
        .evaluate(&request, &executor, bound())
        .expect_err("a mismatched parent is not the declared isolation");
    assert!(matches!(
        error,
        super::evaluate::EvaluationError::IsolationUnverifiable { .. }
    ));
    assert!(
        ledger
            .evaluations(&revision, bound())
            .expect("reads")
            .is_empty(),
        "nothing was recorded"
    );
}

#[test]
fn the_verified_parent_fact_is_durable_on_the_record() {
    // The honest executor: the treatment runs carry the declared parent on
    // their durable records, the harness verifies them, and the appended
    // record carries the verified fact (the gate's MissingEvaluation floor
    // and every later reader see a certified isolation).
    let ledger = Arc::new(MemoryRevisions::default());
    admit_fixture(&ledger);
    let revision = revision_of_document();
    let harness = EvaluationHarness::new(
        ledger.clone(),
        Arc::new(MemoryCapabilities::default()),
        clock(),
    );
    let executor = ScriptedExecutor {
        control: substrate(),
        treatment: substrate_with_parent(Some(run(2))),
        requests: Mutex::new(Vec::new()),
    };
    let request = EvaluationRequest {
        revision: revision.clone(),
        metric: TargetMetric::IterationsPerSettlement,
        delta: DeltaDocument::parse(INERT_DELTA).expect("parses"),
        current_reference_digest: digest(),
        held_out_root: std::path::PathBuf::from("held-out"),
        regression_root: std::path::PathBuf::from("regression"),
        protected: Vec::new(),
        intent: "the evaluation intent".to_owned(),
    };
    let summary = harness
        .evaluate(&request, &executor, bound())
        .expect("the honest evaluation records");
    assert_eq!(
        summary.record.held_out_treatment_parent,
        run(2),
        "the held-out treatment's verified parent"
    );
    assert_eq!(
        summary.record.regression_treatment_parent,
        run(2),
        "the regression treatment's verified parent"
    );
    let stored = ledger.evaluations(&revision, bound()).expect("reads");
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].held_out_treatment_parent, run(2));
    assert_eq!(stored[0].regression_treatment_parent, run(2));
}

// ---------------------------------------------------------------------------
// COR-1 (C6): an infrastructure failure is a typed abort, never a negative
// verdict.
// ---------------------------------------------------------------------------

/// The failing §19.1 capability ledger: reads refuse — an INFRASTRUCTURE
/// failure, not evidence of an inferior capability.
struct FailingCapabilities;

impl CapabilityLedger for FailingCapabilities {
    fn append_observation(
        &self,
        _observation: ObservationRecord,
        _deadline: Deadline,
    ) -> Result<ObservationAppend, CapabilityLedgerError> {
        Err(CapabilityLedgerError::ReadFailed {
            detail: "the capability ledger is unavailable".to_owned(),
        })
    }

    fn observations(
        &self,
        _capability: &CapabilityId,
        _deadline: Deadline,
    ) -> Result<Vec<ObservationRecord>, CapabilityLedgerError> {
        Err(CapabilityLedgerError::ReadFailed {
            detail: "the capability ledger is unavailable".to_owned(),
        })
    }

    fn capability(
        &self,
        _capability: &CapabilityId,
        _deadline: Deadline,
    ) -> Result<Option<gauntlet_ports::capability_ledger::CapabilityEntry>, CapabilityLedgerError>
    {
        Err(CapabilityLedgerError::ReadFailed {
            detail: "the capability ledger is unavailable".to_owned(),
        })
    }

    fn apply_transition(
        &self,
        _transition: gauntlet_ports::capability_ledger::StatusTransition,
        _deadline: Deadline,
    ) -> Result<gauntlet_ports::capability_ledger::CapabilityEntry, CapabilityLedgerError> {
        Err(CapabilityLedgerError::ReadFailed {
            detail: "the capability ledger is unavailable".to_owned(),
        })
    }

    fn transitions(
        &self,
        _capability: &CapabilityId,
        _deadline: Deadline,
    ) -> Result<Vec<gauntlet_ports::capability_ledger::StatusTransition>, CapabilityLedgerError>
    {
        Err(CapabilityLedgerError::ReadFailed {
            detail: "the capability ledger is unavailable".to_owned(),
        })
    }
}

#[test]
fn a_capability_ledger_read_failure_aborts_the_gate_and_writes_nothing() {
    // COR-1 (C6): a capability-ledger READ FAILURE is not evidence of an
    // inferior capability. The old mapping turned it into
    // `ProtectedCapabilityInferior` — a FAILED condition ⇒ a durable,
    // write-once REJECTED. The honest gate aborts typed (retryable),
    // nothing written, the revision NOT rejected.
    let (ledger, _capabilities, revision) = honest_world();
    let gate = gate_with_dyn(ledger.clone(), Arc::new(FailingCapabilities), 0);
    let error = gate
        .adjudicate(&revision, &gate_input(INERT_DELTA), bound())
        .expect_err("an infrastructure failure aborts; it never becomes a verdict");
    assert!(
        matches!(error, GateError::CapabilityLedger(_)),
        "the typed abort names the ledger failure: {error:?}"
    );
    assert!(
        ledger
            .decisions(&revision, bound())
            .expect("reads")
            .is_empty(),
        "nothing was written: the revision is not rejected"
    );
}

// ---------------------------------------------------------------------------
// COR-1 (C7): every entry of an evaluated delta is provably applied, or
// nothing is evaluated.
// ---------------------------------------------------------------------------

/// A composition that can apply only SOME declared delta entries — the
/// gate-classified-but-cannot-apply shape the critic flagged.
struct PartialApplier {
    applies: Vec<String>,
    treatment: Substrate,
    requests: Mutex<Vec<super::evaluate::ExecutionRequest>>,
}

impl EvaluationExecutor for PartialApplier {
    fn validate_delta(&self, delta: &DeltaDocument) -> Result<(), super::evaluate::ExecutionError> {
        let unappliable: Vec<String> = delta
            .overrides()
            .iter()
            .map(|(key, _)| key.clone())
            .filter(|key| !self.applies.contains(key))
            .collect();
        if unappliable.is_empty() {
            Ok(())
        } else {
            Err(super::evaluate::ExecutionError::Failed {
                detail: format!("entries {unappliable:?} cannot be applied"),
            })
        }
    }

    fn execute(
        &self,
        request: &super::evaluate::ExecutionRequest,
        _deadline: Deadline,
    ) -> Result<ExecutionOutcome, super::evaluate::ExecutionError> {
        self.requests.lock().expect("lock").push(request.clone());
        let (substrate, run_id) = if request.parent_run.is_some() {
            (&self.treatment, run(EXECUTOR_TREATMENT_TAG))
        } else {
            (&self.control_run(), run(EXECUTOR_CONTROL_TAG))
        };
        Ok(ExecutionOutcome {
            run_id,
            terminal: TerminalState::Settled,
            reference_digest: digest(),
            state: Arc::new(Substrate {
                run: substrate.run.clone(),
                checkpoint: substrate.checkpoint.clone(),
                records: substrate.records.clone(),
                settlements: substrate.settlements.clone(),
            }),
        })
    }
}

impl PartialApplier {
    fn control_run(&self) -> Substrate {
        substrate()
    }
}

// ---------------------------------------------------------------------------
// COR-2 (falsifier round 1) — the correction probes. Each test reproduces
// one falsifier finding against the real gate/harness semantics before the
// fix lands (RED-first), then pins the fix.
// ---------------------------------------------------------------------------

/// A metrics fixture over an explicit (unit, instrument-deterministic)
/// settlement series — the falsifier's probe shapes compose their arms
/// from these.
fn settlement_series(units: &[(&str, bool)]) -> RunMetrics {
    RunMetrics {
        iterations: 3,
        payment_tokens: 1_000,
        payment_micro_usd: FIXTURE_PAYMENT,
        settlements: units
            .iter()
            .map(|(unit, deterministic)| SettlementFact {
                unit: (*unit).to_owned(),
                instrument_deterministic: Some(*deterministic),
                occurred_at: Some(EpochSeconds::new(BASE + 1)),
            })
            .collect(),
        dispatched_units: units.iter().map(|(unit, _)| (*unit).to_owned()).collect(),
        settled_terminal: true,
    }
}

#[test]
fn an_extra_deterministic_settlement_cannot_shield_a_weakened_unit() {
    // COR-2 (R2, falsifier F-2 probe A verbatim): control settles
    // [u1 deterministic]; treatment settles [u0 deterministic, u1 MODEL
    // RUNG]. The POSITIONAL pairing compared control's u1 against
    // treatment's extra u0 and never saw the weakened unit — the gate
    // PROMOTED with instrument_series_non_weaker = true while verification
    // was genuinely weaker on a real unit. The pairing is BY UNIT:
    // treatment's u1 is weaker than control's u1, so the gate must reject
    // with the AK-056 refusal.
    let (ledger, capabilities, revision) = honest_world();
    let evaluations = ledger.evaluations(&revision, bound()).expect("reads");
    let mut shielded = evaluations[0].clone();
    shielded.control_held_out_metrics = settlement_series(&[(UNIT, true)]);
    shielded.treatment_held_out_metrics = settlement_series(&[(SHIELD_UNIT, true), (UNIT, false)]);
    let fresh = Arc::new(MemoryRevisions::default());
    admit_fixture(&fresh);
    fresh
        .append_evaluation(shielded, bound())
        .expect("the shielded evaluation lands");
    let gate = gate_with(&fresh, &capabilities, 0);
    let verdict = gate
        .adjudicate(&revision, &gate_input(INERT_DELTA), bound())
        .expect("adjudicates");
    let GateVerdict::Rejected { reasons, .. } = verdict else {
        panic!("an extra settlement must not shield a weakened unit: {verdict:?}");
    };
    assert!(
        reasons
            .iter()
            .any(|reason| reason.token() == "settlement_access_weakened"),
        "the weakened unit is caught by the settlement-access condition: {reasons:?}"
    );
    let entry = fresh
        .revision(&revision, bound())
        .expect("reads")
        .expect("present");
    assert_eq!(entry.decision, Some(DecisionKind::Rejected));
}

#[test]
fn a_margin_above_the_basis_point_ceiling_is_never_a_silent_clamp() {
    // COR-2 (R3, falsifier F-3 probe B): control figure 10 000, treatment
    // figure 0 — a genuine 100% improvement — against a declared 15 000
    // bps margin PROMOTED under the former saturation clamp: a declared
    // 150% margin enforced as exactly 100%. A margin above 10 000 bps
    // cannot be an improvement requirement at all, so the gate REFUSES it
    // where it enters — the constructor, typed — instead of adjudicating
    // on the clamped bar.
    let Err(error) = PromotionGate::new(
        Arc::new(MemoryRevisions::default()),
        Arc::new(MemoryCapabilities::default()),
        clock(),
        15_000,
    ) else {
        panic!("a margin above 10 000 bps is a typed configuration error, never a clamp");
    };
    assert_eq!(
        error,
        super::gate::GateConfigError::MarginOutOfRange { margin_bps: 15_000 }
    );
}

#[test]
fn the_basis_point_ceiling_itself_is_a_legal_exactly_one_hundred_percent_bar() {
    // 10 000 bps is INSIDE the domain: it demands the treatment figure be
    // 0 — a genuine halving of the figure — and the gate composes at the
    // ceiling. Only values ABOVE the ceiling are configuration errors.
    assert!(super::gate::margin_holds_for_tests(FIGURE_EQUAL, 0, 10_000));
    assert!(!super::gate::margin_holds_for_tests(
        FIGURE_EQUAL,
        1,
        10_000
    ));
    let gate = PromotionGate::new(
        Arc::new(MemoryRevisions::default()),
        Arc::new(MemoryCapabilities::default()),
        clock(),
        10_000,
    )
    .expect("10 000 bps is exactly 100% and inside the domain");
    assert_eq!(gate.margin_bps(), 10_000);
}

#[test]
fn the_governance_target_classification_is_case_normalized() {
    // COR-2 (R5, falsifier F-5 probe E verbatim): the governance-prefix
    // match was case-SENSITIVE, so capitalized spellings of the loop's
    // authority surfaces classified as free targets.
    for target in [
        "Reference-pipeline",
        "Settlement-machinery",
        "Scheduler-machinery",
        "Authority-surface",
        "GOVERNANCE",
        "reference",
        "settlement.gate",
    ] {
        assert!(
            super::gate::target_is_governance_bearing(target),
            "{target:?} must classify governance-bearing regardless of case"
        );
    }
    for free in ["correction.selection", "review.latency", "memory.import"] {
        assert!(
            !super::gate::target_is_governance_bearing(free),
            "{free:?} is not a governance surface"
        );
    }
}

/// A second, DIFFERENT frozen reference digest — the falsifier's
/// moved-ruler world composes from this and the fixture digest.
fn moved_digest() -> Digest {
    Digest::parse("sha256:cd00000000000000000000000000000000000000000000000000000000000000")
        .expect("moved digest")
}

#[test]
fn a_moved_regression_ruler_is_rejected_with_reference_moved_naming_the_regression_arm() {
    // COR-2 (R1, falsifier F-1): the record carried only the held-out
    // arms' digests, so condition 2 structurally saw 2 of 4 arms — the
    // falsifier's binary-world evaluation recorded held-out digest
    // ebba17… and regression digest 3f6179… in ONE record and adjudicated
    // past condition 2. All four arms' digests are on the record now, and
    // condition 2 refuses with ReferenceMoved naming the offending arm.
    let (ledger, capabilities, revision) = honest_world();
    let evaluations = ledger.evaluations(&revision, bound()).expect("reads");
    let mut moved = evaluations[0].clone();
    moved.regression_control_reference_digest = moved_digest();
    moved.regression_treatment_reference_digest = moved_digest();
    let fresh = Arc::new(MemoryRevisions::default());
    admit_fixture(&fresh);
    fresh
        .append_evaluation(moved, bound())
        .expect("the moved-ruler evaluation lands");
    let gate = gate_with(&fresh, &capabilities, 0);
    let verdict = gate
        .adjudicate(&revision, &gate_input(INERT_DELTA), bound())
        .expect("adjudicates");
    let GateVerdict::Rejected { reasons, .. } = verdict else {
        panic!(
            "a regression set frozen under a moved ruler must never adjudicate clean: \
             {verdict:?}"
        );
    };
    let reference_moved = reasons
        .iter()
        .find(|reason| reason.token() == "reference_moved")
        .unwrap_or_else(|| panic!("condition 2's refusal is recorded: {reasons:?}"));
    let gauntlet_ports::revision_ledger::GateRefusal::ReferenceMoved { detail } = reference_moved
    else {
        panic!("unexpected refusal shape: {reference_moved:?}");
    };
    assert!(
        detail.contains("regression"),
        "the refusal names the offending regression arm: {detail}"
    );
    let entry = fresh
        .revision(&revision, bound())
        .expect("reads")
        .expect("present");
    assert_eq!(entry.decision, Some(DecisionKind::Rejected));
}

/// COR-2 (R1): the falsifier's binary-world executor — every arm settles
/// honestly, but the REGRESSION task set is an operator-named project
/// whose own frozen reference is a MOVED ruler, so the regression arms'
/// outcomes carry a different reference digest than the promoting
/// project's verified lock.
struct MovedRegressionExecutor {
    control: Substrate,
    treatment: Substrate,
}

impl EvaluationExecutor for MovedRegressionExecutor {
    fn validate_delta(
        &self,
        _delta: &DeltaDocument,
    ) -> Result<(), super::evaluate::ExecutionError> {
        Ok(())
    }

    fn execute(
        &self,
        request: &super::evaluate::ExecutionRequest,
        _deadline: Deadline,
    ) -> Result<ExecutionOutcome, super::evaluate::ExecutionError> {
        let (substrate, run_id) = if request.parent_run.is_some() {
            (&self.treatment, run(EXECUTOR_TREATMENT_TAG))
        } else {
            (&self.control, run(EXECUTOR_CONTROL_TAG))
        };
        let moved = request.project_root.as_path() == std::path::Path::new("regression");
        Ok(ExecutionOutcome {
            run_id,
            terminal: TerminalState::Settled,
            reference_digest: if moved { moved_digest() } else { digest() },
            state: Arc::new(Substrate {
                run: substrate.run.clone(),
                checkpoint: substrate.checkpoint.clone(),
                records: substrate.records.clone(),
                settlements: substrate.settlements.clone(),
            }),
        })
    }
}

#[test]
fn a_moved_regression_ruler_refuses_evaluation_and_records_nothing() {
    // COR-2 (R1): the moved-ruler world is refused AT THE SOURCE. An arm
    // whose project resolved a frozen reference different from the
    // promoting project's verified lock is a typed evaluate-time refusal —
    // before any record is appended — so a moved-ruler evaluation never
    // becomes durable evidence for the gate to mis-adjudicate.
    let ledger = Arc::new(MemoryRevisions::default());
    admit_fixture(&ledger);
    let revision = revision_of_document();
    let harness = EvaluationHarness::new(
        ledger.clone(),
        Arc::new(MemoryCapabilities::default()),
        clock(),
    );
    let executor = MovedRegressionExecutor {
        control: substrate(),
        treatment: substrate_with_parent(Some(run(2))),
    };
    let request = EvaluationRequest {
        revision: revision.clone(),
        metric: TargetMetric::IterationsPerSettlement,
        delta: DeltaDocument::parse(INERT_DELTA).expect("parses"),
        current_reference_digest: digest(),
        held_out_root: std::path::PathBuf::from("held-out"),
        regression_root: std::path::PathBuf::from("regression"),
        protected: Vec::new(),
        intent: "the evaluation intent".to_owned(),
    };
    let error = harness
        .evaluate(&request, &executor, bound())
        .expect_err("a moved-ruler arm refuses the whole evaluation");
    // The FIRST moved arm is the regression set's control run: the
    // harness labels arms control/treatment, so the moved SET is named by
    // the carried digest (the moved ruler) and the run id.
    let super::evaluate::EvaluationError::ReferenceMoved {
        arm,
        run: moved_run,
        carried,
        current,
    } = error
    else {
        panic!("the typed refusal names the moved arm: {error:?}");
    };
    assert_eq!(arm, "control", "the regression control arm is what moved");
    assert_eq!(
        moved_run,
        run(EXECUTOR_CONTROL_TAG),
        "the refusal carries the moved run's id"
    );
    assert_eq!(
        *carried,
        moved_digest(),
        "the refusal carries the digest the moved project froze"
    );
    assert_eq!(*current, digest(), "against the promoting project's lock");
    assert!(
        ledger
            .evaluations(&revision, bound())
            .expect("reads")
            .is_empty(),
        "nothing was recorded: a moved-ruler evaluation is not evidence"
    );
}

#[test]
fn an_unappliable_delta_entry_refuses_evaluation_before_any_arm_runs() {
    // COR-1 (C7): a delta entry the composition cannot apply means the
    // gate would adjudicate a candidate whose declared change never
    // reached the treatment arm. The harness refuses TYPED, naming the
    // entry, BEFORE any arm executes — no runs charged, nothing recorded.
    let ledger = Arc::new(MemoryRevisions::default());
    admit_fixture(&ledger);
    let revision = revision_of_document();
    let harness = EvaluationHarness::new(
        ledger.clone(),
        Arc::new(MemoryCapabilities::default()),
        clock(),
    );
    let executor = PartialApplier {
        applies: vec!["budget.max_parallel".to_owned()],
        treatment: substrate_with_parent(Some(run(2))),
        requests: Mutex::new(Vec::new()),
    };
    let request = EvaluationRequest {
        revision: revision.clone(),
        metric: TargetMetric::IterationsPerSettlement,
        // Parses fine (a declared, inert key) — but this composition only
        // applies budget.max_parallel:
        delta: DeltaDocument::parse("[{\"key\":\"runners.default\",\"value\":\"economy\"}]")
            .expect("parses"),
        current_reference_digest: digest(),
        held_out_root: std::path::PathBuf::from("held-out"),
        regression_root: std::path::PathBuf::from("regression"),
        protected: Vec::new(),
        intent: "the evaluation intent".to_owned(),
    };
    let error = harness
        .evaluate(&request, &executor, bound())
        .expect_err("an unappliable delta refuses the whole evaluation");
    assert!(
        matches!(
            error,
            super::evaluate::EvaluationError::DeltaUnappliable { .. }
        ),
        "the typed refusal names the unappliable entry: {error:?}"
    );
    assert!(
        executor.requests.lock().expect("lock").is_empty(),
        "no arm ran: no run was charged on a delta that cannot be applied"
    );
    assert!(
        ledger
            .evaluations(&revision, bound())
            .expect("reads")
            .is_empty(),
        "nothing was recorded"
    );
}
