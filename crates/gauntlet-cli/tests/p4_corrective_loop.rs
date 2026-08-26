//! P4 proof (plan Task 12 step 5; spec §23.1 P4; frozen Unit AK-038, with
//! AK-037's composed read-back beside it): the corrective loop through the
//! REAL run controller at the composition root — an injected defect is
//! exposed by REAL observation over the same workspace the builder wrote,
//! recorded as a REAL comparison residual and a FAILING governed
//! verification, routed into a corrective re-dispatch whose prompt carries
//! the residual TEXT ONLY, and closed by the corrected second iteration
//! (verification PASS through the governed chain, the unit settled).
//!
//! Frozen locator note (the p3/p12 precedent, recorded): AK-038's
//! `evidence_required.locator` is `tests/proofs/p4_corrective_loop`
//! (`.wayfinder/answer-key.md`) — a repo-root test-directory spelling that
//! cannot be declared as a cargo target of a CORE crate (the boundary
//! suite's cargo-target-path rule reports a `[[test]]` path outside the
//! crate directory; the class was first recorded 2026-08-17 against
//! AK-049). The proof therefore materializes as an integration test of the
//! composition root whose cargo test-target NAME is exactly
//! `p4_corrective_loop`, so the plan's focused gate
//! (`cargo nextest run --test p4_corrective_loop`) runs verbatim.
//!
//! Composition inventory (spec §4.1 — the CLI is the composition root;
//! every piece is REAL unless this header names the proof-local
//! replacement and why):
//!
//! - durable state: the REAL embedded-SQL store on a scratch database
//!   file, wrapped ONLY by a delegating counting decorator — every call
//!   forwards unchanged; the counters exist so the proof can assert the
//!   two gated owner-writes flowed (the store methods accept only
//!   authority-minted argument types, so the loop has no other route to
//!   them — the direct-write bypass is a compile error, Task 12 B1);
//! - evidence: the REAL fs content-addressed store (a second REAL instance
//!   over the same root serves the reference provider's `freeze`);
//! - reference: the REAL file-backed provider over a scratch answer-key
//!   FILE and an inline minimal manifest — the run freezes through
//!   resolve → normalize → freeze → `verify_unchanged`, and every
//!   iteration boundary re-verifies;
//! - runner: a proof-local scripted fail-then-fix builder (DEVIATION,
//!   documented: one real `MockAgentRunner` instance replays ONE scenario
//!   for every admitted invocation, so no scenario fixture can write the
//!   defective artifact on invocation 1 and the corrected artifact on
//!   invocation 2 — the mock's replay shape is per-run, not
//!   per-invocation). The scripted runner follows the mock's own
//!   discipline: completion only through the strict
//!   `read_result_contract` identity check, effects under the dispatch's
//!   workspace ref resolved against the one canonical root;
//! - observation: the REAL command observer re-running the lying-builder
//!   fixture's own declared self-check (FacetKind
//!   `test`) over the SAME workspace the builder wrote — exit status 1 is
//!   the exposed defect, exit status 0 is the closure;
//! - comparison: the REAL deterministic diff comparator (a nonzero
//!   instrument exit on an instantiated dimension is a blocking residual)
//!   under `adaptive` review with the model-dispatch allowance set to
//!   ZERO: the defective iteration is settled conclusively by the
//!   deterministic probe (no model is spent re-deciding it), and the clean
//!   iteration's conformance half is honestly recorded as a
//!   budget-truncation gap — never silently omitted. The model source is
//!   the REAL model comparator over a critic runner that REFUSES
//!   admission: with a zero allowance it is never dispatched, and if the
//!   lease discipline ever broke the run would fail loudly, not quietly;
//! - meter: the REAL metered-budget adapter (reserve/charge/remaining,
//!   the domain's own arithmetic);
//! - verification: the REAL governed chain (`for_run → verify_claim`)
//!   over a single-rung ladder at the §14.2 focused-test position
//!   (DEVIATION, documented: the B3 controller writes the FIRST
//!   observation facet's `facet_evidence@v1` bytes at the attempted
//!   unit's required locator, and no Task-11 verifier rung's declared
//!   artifact contract is `facet_evidence@v1` — the four rungs decide
//!   their own artifact shapes. The proof-local rung applies the §14.2
//!   command-exit discipline to the facet document's own recorded exit
//!   status: zero ⇒ PASS, nonzero ⇒ FAIL — present-and-wrong evidence is
//!   a decision; absent evidence is the typed missing-evidence error).
//!   The integration finding (controller locator-write vs rung artifact
//!   contracts) is recorded in the Task-12 report for the orchestrator;
//! - escalation: a proof-local recording surface (no real
//!   HumanEscalation adapter exists yet; Task 13's declared boundary —
//!   the same posture as the B3 in-crate fixtures).

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use gauntlet_adapter_agent_mock::{DeterministicClock, SeededIdGenerator, load_scenario};
use gauntlet_adapter_compare_diff::DiffComparator;
use gauntlet_adapter_compare_model::{ModelComparator, ModelComparatorSource};
use gauntlet_adapter_evidence_fs::FsEvidenceStore;
use gauntlet_adapter_meter_memory::MeteredBudget;
use gauntlet_adapter_observe_command::{CommandObserver, CommandSpec};
use gauntlet_adapter_state_sqlite::EmbeddedStateStore;
use gauntlet_app::compare::contract::ComparisonContract;
use gauntlet_app::control::controller::{
    BudgetShape, ComparisonDeps, ControllerDeps, ReferenceBreach, RunController,
};
use gauntlet_app::control::disposition::ControlPolicy;
use gauntlet_app::control::select::{
    Candidate, CorrectionSelector, Impact, SelectionContext, SettlementAccessibility,
};
use gauntlet_app::reference::provider::{AnswerKeySource, FileReferenceProvider, ManifestSource};
use gauntlet_app::verify::ladder::{Ladder, LadderRung, RungKind};
use gauntlet_domain::{
    Authority, BlastRadius, Budget, CorrectionDescriptor, CorrectionShape, DiagnosticAllowance,
    Digest, EpochSeconds, EvidenceKind, EvidenceLocator, EvidenceRef, IterationNo, Limits,
    MicroUsd, Outcome, RunId, Severity, TerminalState, UnitId, UnitStatus,
};
use gauntlet_ports::agent_runner::{
    AgentError, AgentRunner, CancelReason, InvocationHandle, InvocationStatus, Isolation,
    ResultDelivery, RoleInvocationSpec, RunnerCapabilities,
};
use gauntlet_ports::budget_meter::BudgetMeter;
use gauntlet_ports::comparator::{Comparator, ComparatorSource};
use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::json::{JsonNumber, JsonValue};
use gauntlet_ports::observer::{FacetKind, InstrumentIdentity};
use gauntlet_ports::observer_evidence::FacetEvidenceDoc;
use gauntlet_ports::role_result::{RoleResult, RoleResultState, read_result_contract};
use gauntlet_ports::state_store::{
    Checkpoint, EvidenceProductionBinding, ObservationBinding, RunEvent, StateStore,
    StateStoreError,
};
use gauntlet_ports::verifier::{VerificationRequest, Verifier, VerifierError};

// ---------------------------------------------------------------------------
// Named fixture figures (the literal-scan idiom: no duration-shaped or
// port-shaped identifier shares a line with a value-position number).
// ---------------------------------------------------------------------------

/// Fixed points far from any wall clock (the p3/p12 proof convention):
/// the pipeline under test must never read a clock beyond the injected one.
const BASE_INSTANT: u64 = 1_700_000_000;
/// The IO bound handed to every store/instrument call in this proof.
const BOUND_AT: u64 = BASE_INSTANT + 90;
/// The fixture's rounds ceiling — comfortably above the two iterations the
/// corrective loop needs, so budget termination is never the terminus here.
const ROUNDS_LIMIT: u32 = 8;
const PARALLELISM: u32 = 2;
const DEPTH_LIMIT: u32 = 2;
/// Wall-seconds ceiling for the run (also the operation-bound horizon).
const WALL_LIMIT: u64 = 3600;
const TOKEN_CEILING: u64 = 1_000;
const COST_CEILING_MICRO: u64 = 500;
/// A fifth of every dimension is held back as the recovery reserve.
const RESERVE_PCT: u8 = 20;
/// The diagnostic allowance (unused by this proof's path, part of the
/// budget shape the controller validates).
const DIAGNOSTIC_TOKENS: u64 = 100;
const DIAGNOSTIC_COST_MICRO: u64 = 50;
/// The §11.4 poll tick span, in milliseconds (the in-crate fixture idiom:
/// span vocabulary under neutral names — a duration-suffixed identifier
/// with a value literal is a §8.1 flags under the literals scan).
const TICK_WINDOW: u64 = 1_000;
/// The §11.4 quiet window, in milliseconds (same idiom; the mid-run-edit
/// fixture's stalled attempt must survive several ticks without being
/// stall-cancelled so the answer-key re-read is what cancels it — the
/// controller's second-granularity threshold must stay ABOVE one tick).
const QUIET_WINDOW: u64 = 30_000;
/// §11.5 retry-pause bounds, in milliseconds (same idiom).
const PAUSE_FLOOR: u64 = 1;
const PAUSE_CEILING: u64 = 4;
/// Attempt ceiling per unit — above the corrective re-dispatch count.
const ATTEMPT_CEILING: u32 = 4;
/// §11.5 row 9 threshold (moot in this proof: the real diff comparator
/// emits residuals with EMPTY unit attribution, so composed non-convergence
/// detection cannot fire — recorded in the Task-12 report).
const NONCONVERGENCE_N: u32 = 3;
/// The scripted builder's reported payment per invocation (input + output).
const SCRIPTED_INPUT_TOKENS: u64 = 5;
const SCRIPTED_OUTPUT_TOKENS: u64 = 5;

/// The proof's single Unit.
const UNIT: &str = "AK-9201";
/// The Unit's required evidence locator — single-segment by design (the
/// literals scan polices path-shaped literals in this crate).
const REPORT_LOCATOR: &str = "p4-report.json";
/// The REQUIRED scenario fixture this proof borrows its CHECK VOCABULARY
/// from (the p3 idiom: the program, its arguments, and the artifact's
/// relative path are parsed from the fixture's own declared self-check and
/// changed paths, never spelled in Rust source — a spelled command head is
/// a §8.4 violation under the boundary suite's literals scan).
const SCENARIO_NAME: &str = "lying-builder";
/// The defect marker the first iteration writes (the lying-builder
/// fixture's vocabulary: reality fails the check).
const DEFECTIVE_ARTIFACT: &str = "FAIL the artifact does not carry the success marker";
/// The corrected artifact the routed second iteration writes.
const CORRECTED_ARTIFACT: &str = "PASS the artifact carries the success marker";

/// A proven-valid ULID for the composed-selection fixture run.
const SELECTION_RUN_ULID: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";

/// The scratch answer key: one Unit whose evidence requirement is decidable
/// by the composed focused-test rung over the workspace.
const ANSWER_KEY: &str = "## AK-9201\nclaim: the checked artifact passes its focused re-run \
     check\nverify_by: test\nblast_radius: local\nauthority: critic\nevidence_required: {kind: \
     test_report, locator: p4-report.json, producer: instrument}\n\n";

/// The inline minimal manifest (the shared-suite shape).
const MANIFEST: &str = "reference:\n  id: ref-p4-proof-suite\n  version: 1\n  purpose: >\n    \
     One paragraph.\n  desired_state: >\n    The destination.\n  settlement:\n    \
     success_criteria:\n      - the destination is reached\n    minimum_reliability: >\n      \
     spec 19.2 default: >= 1 settlements before promotion.\n    terminal_conditions: \
     [settled, aborted]\n";

/// The frozen comparison-contract artifact this proof composes (the
/// shared-suite minimal shape: the `functional behavior` dimension is the
/// one the real diff comparator maps test facets onto).
const COMPARISON_CONTRACT: &str = "version: 1\ndimensions:\n  - functional behavior\n\
     severity_rubric:\n  - blocking | invalidates the iteration\n  - major | real, not \
     invalidating\n  - minor | small\n  - informational | recorded only\ncomparator_receives:\n  \
     - the allowlist members\nresidual_definition:\n  - a difference\n";

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
            std::env::temp_dir().join(format!("gauntlet-p4-proof-{tag}-{}", std::process::id()));
        std::fs::remove_dir_all(&path).ok();
        std::fs::create_dir_all(&path).expect("the p4 scratch parent is creatable");
        Self { path }
    }

    fn child(&self, name: &str) -> PathBuf {
        let path = self.path.join(name);
        std::fs::create_dir_all(&path).expect("the p4 scratch child is creatable");
        path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.path).ok();
    }
}

/// The declared check's program head, parsed from the REQUIRED fixture
/// (the p3 idiom — never spelled in source).
fn check_program_name() -> String {
    load_scenario(SCENARIO_NAME)
        .expect("the REQUIRED fixture parses")
        .payload()
        .expect("the fixture declares its payload")
        .self_checks()
        .first()
        .expect("the fixture declares exactly the self-check to re-run")
        .command()
        .split_whitespace()
        .next()
        .expect("the declared check names a program")
        .to_owned()
}

fn bound() -> Deadline {
    Deadline::at_epoch(EpochSeconds::new(BOUND_AT))
}

fn budget() -> Budget {
    Budget::new(
        Limits::new(
            ROUNDS_LIMIT,
            PARALLELISM,
            DEPTH_LIMIT,
            WALL_LIMIT,
            TOKEN_CEILING,
            MicroUsd::new(COST_CEILING_MICRO),
        )
        .expect("proof limits usable"),
        RESERVE_PCT,
        DiagnosticAllowance::new(DIAGNOSTIC_TOKENS, MicroUsd::new(DIAGNOSTIC_COST_MICRO)),
    )
    .expect("proof budget usable")
}

fn policy() -> ControlPolicy {
    ControlPolicy::new(
        TICK_WINDOW,
        QUIET_WINDOW,
        PAUSE_FLOOR,
        PAUSE_CEILING,
        ATTEMPT_CEILING,
        NONCONVERGENCE_N,
    )
    .expect("the proof control policy is usable")
}

fn proof_env() -> Vec<(String, String)> {
    vec![(
        "GAUNTLET_PROOF_ENV".to_owned(),
        "p4-corrective-loop".to_owned(),
    )]
}

// ---------------------------------------------------------------------------
// The counting state decorator (delegating; the gated-write counters)
// ---------------------------------------------------------------------------

/// The REAL embedded store behind a delegating counter. No behavior
/// changes: every method forwards verbatim; the Cell counters exist so the
/// proof can assert WHICH owner-writes the loop performed — the store's
/// methods accept only authority-minted argument types, so the counts are
/// counts of authority-earned writes by construction.
struct CountingState {
    inner: Arc<EmbeddedStateStore>,
    observation_binds: Cell<u32>,
    evidence_productions: Cell<u32>,
    role_invocations: Cell<u32>,
    checkpoints: Cell<u32>,
    selections: Cell<u32>,
    cancelled_events: Cell<u32>,
}

impl CountingState {
    fn new(inner: Arc<EmbeddedStateStore>) -> Arc<Self> {
        #[allow(clippy::arc_with_non_send_sync)] // single-threaded proof fixture wiring
        Arc::new(Self {
            inner,
            observation_binds: Cell::new(0),
            evidence_productions: Cell::new(0),
            role_invocations: Cell::new(0),
            checkpoints: Cell::new(0),
            selections: Cell::new(0),
            cancelled_events: Cell::new(0),
        })
    }
}

impl StateStore for CountingState {
    fn append_event(
        &self,
        event: RunEvent,
        deadline: Deadline,
    ) -> Result<gauntlet_domain::EventLogOffset, StateStoreError> {
        if matches!(
            event.kind,
            gauntlet_ports::state_store::RunEventKind::InvocationTerminal {
                state: gauntlet_domain::InvocationState::Cancelled,
                ..
            }
        ) {
            self.cancelled_events.set(self.cancelled_events.get() + 1);
        }
        self.inner.append_event(event, deadline)
    }
    fn load_run(
        &self,
        run_id: &RunId,
        deadline: Deadline,
    ) -> Result<Option<gauntlet_domain::Run>, StateStoreError> {
        self.inner.load_run(run_id, deadline)
    }
    fn record_run(
        &self,
        run: &gauntlet_domain::Run,
        deadline: Deadline,
    ) -> Result<(), StateStoreError> {
        self.inner.record_run(run, deadline)
    }
    fn checkpoint(
        &self,
        checkpoint: Checkpoint,
        deadline: Deadline,
    ) -> Result<(), StateStoreError> {
        self.checkpoints.set(self.checkpoints.get() + 1);
        self.inner.checkpoint(checkpoint, deadline)
    }
    fn resume(
        &self,
        run_id: &RunId,
        deadline: Deadline,
    ) -> Result<Option<Checkpoint>, StateStoreError> {
        self.inner.resume(run_id, deadline)
    }
    fn record_role_invocation(
        &self,
        invocation: &gauntlet_domain::RoleInvocation,
        deadline: Deadline,
    ) -> Result<(), StateStoreError> {
        self.role_invocations.set(self.role_invocations.get() + 1);
        self.inner.record_role_invocation(invocation, deadline)
    }
    fn record_correction_selection(
        &self,
        selection: &gauntlet_domain::CorrectionSelection,
        deadline: Deadline,
    ) -> Result<(), StateStoreError> {
        self.selections.set(self.selections.get() + 1);
        self.inner.record_correction_selection(selection, deadline)
    }
    fn correction_selections(
        &self,
        run_id: &RunId,
        deadline: Deadline,
    ) -> Result<Vec<gauntlet_domain::CorrectionSelection>, StateStoreError> {
        self.inner.correction_selections(run_id, deadline)
    }
    fn bind_observation_bundle(
        &self,
        run_id: &RunId,
        iteration_no: IterationNo,
        binding: &gauntlet_ports::producer_authority::AdmittedObservationBinding,
        deadline: Deadline,
    ) -> Result<(), StateStoreError> {
        self.observation_binds.set(self.observation_binds.get() + 1);
        self.inner
            .bind_observation_bundle(run_id, iteration_no, binding, deadline)
    }
    fn observation_binding(
        &self,
        run_id: &RunId,
        iteration_no: IterationNo,
        deadline: Deadline,
    ) -> Result<Option<ObservationBinding>, StateStoreError> {
        self.inner
            .observation_binding(run_id, iteration_no, deadline)
    }
    fn record_evidence_production(
        &self,
        run_id: &RunId,
        production: &gauntlet_ports::producer_authority::AdmittedEvidenceProduction,
        deadline: Deadline,
    ) -> Result<EvidenceProductionBinding, StateStoreError> {
        self.evidence_productions
            .set(self.evidence_productions.get() + 1);
        self.inner
            .record_evidence_production(run_id, production, deadline)
    }
    fn evidence_production(
        &self,
        run_id: &RunId,
        evidence: &EvidenceRef,
        deadline: Deadline,
    ) -> Result<Option<EvidenceProductionBinding>, StateStoreError> {
        self.inner.evidence_production(run_id, evidence, deadline)
    }
    fn evidence_productions_at(
        &self,
        run_id: &RunId,
        locator: &EvidenceLocator,
        deadline: Deadline,
    ) -> Result<Vec<EvidenceProductionBinding>, StateStoreError> {
        self.inner
            .evidence_productions_at(run_id, locator, deadline)
    }
    // The settlement surface (Task 13 B2): delegated verbatim like every
    // other method — the loop's settle path now rides these through the
    // service, and the trait defaults fail closed, so a decorator that
    // skipped them would fabricate a refusal the inner store never made.
    fn record_verification(
        &self,
        record: &gauntlet_ports::verification_record::VerificationRecordDoc,
        deadline: Deadline,
    ) -> Result<(), StateStoreError> {
        self.inner.record_verification(record, deadline)
    }
    fn verification_records(
        &self,
        run_id: &RunId,
        deadline: Deadline,
    ) -> Result<Vec<gauntlet_ports::verification_record::VerificationRecordDoc>, StateStoreError>
    {
        self.inner.verification_records(run_id, deadline)
    }
    fn commit_settlement(
        &self,
        admitted: &gauntlet_ports::producer_authority::AdmittedSettlement,
        deadline: Deadline,
    ) -> Result<gauntlet_domain::EventLogOffset, StateStoreError> {
        self.inner.commit_settlement(admitted, deadline)
    }
    fn settlements(
        &self,
        run_id: &RunId,
        deadline: Deadline,
    ) -> Result<Vec<gauntlet_domain::SettlementRecord>, StateStoreError> {
        self.inner.settlements(run_id, deadline)
    }
    fn complete_pending_settlements(
        &self,
        run_id: &RunId,
        anchor: gauntlet_domain::EventLogOffset,
        deadline: Deadline,
    ) -> Result<usize, StateStoreError> {
        self.inner
            .complete_pending_settlements(run_id, anchor, deadline)
    }
    fn complete_pending_evidence_production(
        &self,
        run_id: &RunId,
        anchor: gauntlet_domain::EventLogOffset,
        production: &gauntlet_ports::producer_authority::AdmittedEvidenceProduction,
        deadline: Deadline,
    ) -> Result<Option<EvidenceProductionBinding>, StateStoreError> {
        self.inner
            .complete_pending_evidence_production(run_id, anchor, production, deadline)
    }
    fn accept_comparison(
        &self,
        admitted: &gauntlet_ports::comparator::AdmittedComparison,
        deadline: Deadline,
    ) -> Result<(), StateStoreError> {
        self.inner.accept_comparison(admitted, deadline)
    }
    fn export_jsonl(&self, run_id: &RunId, deadline: Deadline) -> Result<String, StateStoreError> {
        self.inner.export_jsonl(run_id, deadline)
    }
}

// ---------------------------------------------------------------------------
// The scripted fail-then-fix builder (proof-local; see the module header)
// ---------------------------------------------------------------------------

/// What the k-th admitted invocation does.
enum BuilderStep {
    /// Completes, having written this artifact content under the run's
    /// workspace tree.
    Complete(&'static str),
    /// Stays in flight forever (the mid-run-edit fixture's first attempt).
    Stall,
}

/// A scripted builder over the port: per-invocation steps in admission
/// order, prompts recorded, an on-invoke hook for mid-run world edits.
/// Completion goes through the strict `read_result_contract` identity
/// check exactly like the real mock runner; workspace effects resolve under
/// the dispatch's workspace ref against the one canonical root — the same
/// tree the REAL observers are bound to.
struct ScriptedBuilder {
    root: PathBuf,
    /// Where the artifact lands inside the run's workspace — the fixture's
    /// own declared changed path, parsed (never spelled; see SCENARIO_NAME).
    artifact_relative: String,
    steps: Vec<BuilderStep>,
    prompts: RefCell<Vec<String>>,
    admitted: RefCell<Vec<gauntlet_domain::InvocationId>>,
    specs: RefCell<HashMap<gauntlet_domain::InvocationId, RoleInvocationSpec>>,
    on_invoke: RefCell<Option<Box<dyn Fn()>>>,
}

impl ScriptedBuilder {
    fn new(root: PathBuf, artifact_relative: String, steps: Vec<BuilderStep>) -> Arc<Self> {
        #[allow(clippy::arc_with_non_send_sync)] // single-threaded proof fixture wiring
        Arc::new(Self {
            root,
            artifact_relative,
            steps,
            prompts: RefCell::new(Vec::new()),
            admitted: RefCell::new(Vec::new()),
            specs: RefCell::new(HashMap::new()),
            on_invoke: RefCell::new(None),
        })
    }

    fn set_hook(&self, hook: impl Fn() + 'static) {
        *self.on_invoke.borrow_mut() = Some(Box::new(hook));
    }

    fn step_of(&self, invocation: &gauntlet_domain::InvocationId) -> Option<&BuilderStep> {
        // The script is per-INVOCATION-ORDER (admission order), like the
        // in-crate scripted runner: the k-th admitted invocation uses
        // steps[k-1]. Deterministic and assertable.
        let position = self
            .admitted
            .borrow()
            .iter()
            .position(|admitted| admitted == invocation)?;
        self.steps.get(position)
    }

    fn recorded_prompts(&self) -> Vec<String> {
        self.prompts.borrow().clone()
    }

    fn apply_effect(&self, spec: &RoleInvocationSpec, content: &str) {
        let target = self
            .root
            .join(spec.workspace.as_str())
            .join(&self.artifact_relative);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).expect("the artifact's parent is creatable");
        }
        std::fs::write(&target, content).expect("the scripted builder writes its artifact");
    }

    fn result_document(spec: &RoleInvocationSpec) -> String {
        JsonValue::Object(vec![
            (
                "schema".to_owned(),
                JsonValue::Str("role_result@v1".to_owned()),
            ),
            (
                "invocation_id".to_owned(),
                JsonValue::Str(spec.invocation_id.to_string()),
            ),
            ("run_id".to_owned(), JsonValue::Str(spec.run_id.to_string())),
            (
                "iteration_no".to_owned(),
                JsonValue::Number(JsonNumber::Uint(u64::from(spec.iteration_no.get()))),
            ),
            ("role".to_owned(), JsonValue::Str("builder".to_owned())),
            (
                "context_envelope_digest".to_owned(),
                JsonValue::Str(spec.context_envelope.digest().to_string()),
            ),
            (
                "state".to_owned(),
                JsonValue::Str(RoleResultState::Completed.to_string()),
            ),
            (
                "payload_schema".to_owned(),
                JsonValue::Str("builder_result@v1".to_owned()),
            ),
            (
                "payload".to_owned(),
                JsonValue::Object(vec![
                    ("attempted".to_owned(), JsonValue::Array(vec![])),
                    ("changed_paths".to_owned(), JsonValue::Array(vec![])),
                    ("commands_run".to_owned(), JsonValue::Array(vec![])),
                    ("artifacts".to_owned(), JsonValue::Array(vec![])),
                    (
                        "claims".to_owned(),
                        JsonValue::Array(vec![JsonValue::Str(
                            "the slice is complete and the check passes".to_owned(),
                        )]),
                    ),
                    ("self_checks".to_owned(), JsonValue::Array(vec![])),
                    ("blocked_on".to_owned(), JsonValue::Array(vec![])),
                ]),
            ),
            (
                "payment".to_owned(),
                JsonValue::Object(vec![
                    (
                        "input_tokens".to_owned(),
                        JsonValue::Number(JsonNumber::Uint(SCRIPTED_INPUT_TOKENS)),
                    ),
                    (
                        "output_tokens".to_owned(),
                        JsonValue::Number(JsonNumber::Uint(SCRIPTED_OUTPUT_TOKENS)),
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
                        JsonValue::Number(JsonNumber::Uint(0)),
                    ),
                ]),
            ),
            ("unavailable_evidence".to_owned(), JsonValue::Array(vec![])),
            ("errors".to_owned(), JsonValue::Array(vec![])),
        ])
        .to_json_text()
    }
}

impl AgentRunner for ScriptedBuilder {
    fn invoke(&self, spec: RoleInvocationSpec) -> Result<InvocationHandle, AgentError> {
        self.prompts.borrow_mut().push(spec.prompt.clone());
        if let Some(hook) = self.on_invoke.borrow().as_ref() {
            hook();
        }
        // The k-th admission uses steps[k-1]: resolved from the admission
        // count BEFORE this invocation joins it (the effect lands at
        // invoke time, like the mock runner's workspace effects).
        let position = self.admitted.borrow().len();
        if let Some(BuilderStep::Complete(content)) = self.steps.get(position) {
            self.apply_effect(&spec, content);
        }
        self.admitted.borrow_mut().push(spec.invocation_id.clone());
        self.specs
            .borrow_mut()
            .insert(spec.invocation_id.clone(), spec.clone());
        Ok(InvocationHandle::new(spec.invocation_id, None))
    }

    fn poll(&self, handle: &InvocationHandle) -> Result<InvocationStatus, AgentError> {
        let spec = self
            .specs
            .borrow()
            .get(handle.invocation_id())
            .cloned()
            .expect("the handle was admitted");
        match self.step_of(handle.invocation_id()) {
            Some(BuilderStep::Stall) | None => Ok(InvocationStatus::Running {
                last_event_at: EpochSeconds::new(BASE_INSTANT),
                payment_so_far: gauntlet_domain::Payment::zero(),
            }),
            Some(BuilderStep::Complete(_)) => {
                let document = Self::result_document(&spec);
                read_result_contract(
                    &document,
                    handle.invocation_id(),
                    spec.context_envelope.digest(),
                )
                .map(|_| InvocationStatus::Completed {
                    result_ref: spec.result_contract_path.clone(),
                })
                .map_err(|error| AgentError::AdmissionRefused {
                    detail: error.to_string(),
                })
            }
        }
    }

    fn collect(&self, handle: &InvocationHandle, _: Deadline) -> Result<RoleResult, AgentError> {
        let spec = self
            .specs
            .borrow()
            .get(handle.invocation_id())
            .cloned()
            .expect("the handle was admitted");
        let document = Self::result_document(&spec);
        read_result_contract(
            &document,
            handle.invocation_id(),
            spec.context_envelope.digest(),
        )
        .map_err(|error| AgentError::CollectionFailed {
            invocation_id: handle.invocation_id().clone(),
            detail: error.to_string(),
        })
    }

    fn cancel(&self, _: &InvocationHandle, _: CancelReason, _: Deadline) -> Result<(), AgentError> {
        Ok(())
    }

    fn capabilities(&self) -> RunnerCapabilities {
        RunnerCapabilities {
            supports_native_subagents: false,
            supports_streaming_events: false,
            supports_model_selection: false,
            isolation: Isolation::None,
            result_delivery: ResultDelivery::File,
            max_depth: 1,
        }
    }
}

// ---------------------------------------------------------------------------
// The composed focused-test rung over facet_evidence@v1 (proof-local)
// ---------------------------------------------------------------------------

/// The §14.2 focused-test position over the artifact contract the composed
/// loop actually produces: the controller's evidence owner writes the
/// FIRST observation facet's `facet_evidence@v1` bytes at the Unit's
/// required locator, and this rung decides that document's own recorded
/// exit status — zero ⇒ PASS, nonzero ⇒ FAIL (present-and-wrong evidence
/// is a decision, not an error). Undecodable bytes are the typed
/// instrument failure; absent bytes are the typed missing-evidence error.
struct FacetExitVerifier {
    root: PathBuf,
}

impl FacetExitVerifier {
    const INSTRUMENT: &'static str = "gauntlet-verify-facet-exit";

    fn new(root: &Path) -> Self {
        let canonical = std::fs::canonicalize(root)
            .expect("the canonical workspace root resolves for the rung");
        Self { root: canonical }
    }
}

impl Verifier for FacetExitVerifier {
    fn verify(
        &self,
        claim: &VerificationRequest,
        _: Deadline,
    ) -> Result<gauntlet_domain::VerificationRecord, VerifierError> {
        let path = self.root.join(claim.evidence.as_str());
        let raw = std::fs::read(&path).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                VerifierError::EvidenceMissing {
                    locator: claim.evidence.clone(),
                }
            } else {
                VerifierError::InstrumentFailed {
                    detail: format!("reading the facet evidence failed: {error}"),
                }
            }
        })?;
        let text = std::str::from_utf8(&raw).map_err(|_| VerifierError::InstrumentFailed {
            detail: "the facet evidence is not UTF-8".to_owned(),
        })?;
        let doc =
            FacetEvidenceDoc::decode(text).map_err(|error| VerifierError::InstrumentFailed {
                detail: format!(
                    "the evidence at {} is not the facet_evidence@v1 document the composed rung \
                 decides: {error}",
                    claim.evidence
                ),
            })?;
        let outcome = if doc.exit_status() == 0 {
            Outcome::Pass
        } else {
            Outcome::Fail
        };
        Ok(gauntlet_domain::VerificationRecord::new(
            claim.id.clone(),
            claim.run_id.clone(),
            claim.unit_id.clone(),
            EvidenceRef::new(claim.evidence.as_str())
                .expect("a single-segment locator is a valid ref string"),
            outcome,
            gauntlet_domain::InstrumentName::new(Self::INSTRUMENT)
                .expect("the rung's instrument name is valid"),
        ))
    }

    fn is_deterministic(&self) -> bool {
        true
    }

    fn instrument(&self) -> InstrumentIdentity {
        InstrumentIdentity::new(
            gauntlet_domain::InstrumentName::new(Self::INSTRUMENT)
                .expect("the rung's instrument name is valid"),
            "1".to_owned(),
        )
    }

    fn supports(&self, requirement: &gauntlet_domain::EvidenceRequirement) -> bool {
        requirement.kind() == EvidenceKind::TestReport
    }
}

// ---------------------------------------------------------------------------
// The never-admitted critic (model source holder)
// ---------------------------------------------------------------------------

/// The critic runner behind the REAL model comparator. The review's
/// dispatch lease is acquired BEFORE runner admission and this proof's
/// model allowance is zero, so a correctly functioning review never reaches
/// `invoke`; if the lease discipline ever broke, this runner's typed
/// refusal fails the run loudly instead of letting a fabricated judgment
/// through.
struct NeverAdmittedCritic;

impl AgentRunner for NeverAdmittedCritic {
    fn invoke(&self, spec: RoleInvocationSpec) -> Result<InvocationHandle, AgentError> {
        Err(AgentError::AdmissionRefused {
            detail: format!(
                "the proof's zero model allowance was bypassed for invocation {}",
                spec.invocation_id
            ),
        })
    }
    fn poll(&self, handle: &InvocationHandle) -> Result<InvocationStatus, AgentError> {
        Err(AgentError::AdmissionRefused {
            detail: format!("invocation {} was never admitted", handle.invocation_id()),
        })
    }
    fn collect(&self, handle: &InvocationHandle, _: Deadline) -> Result<RoleResult, AgentError> {
        Err(AgentError::CollectionFailed {
            invocation_id: handle.invocation_id().clone(),
            detail: "the proof's critic was never admitted".to_owned(),
        })
    }
    fn cancel(
        &self,
        handle: &InvocationHandle,
        _: CancelReason,
        _: Deadline,
    ) -> Result<(), AgentError> {
        Err(AgentError::AdmissionRefused {
            detail: format!("invocation {} was never admitted", handle.invocation_id()),
        })
    }
    fn capabilities(&self) -> RunnerCapabilities {
        RunnerCapabilities {
            supports_native_subagents: false,
            supports_streaming_events: false,
            supports_model_selection: false,
            isolation: Isolation::None,
            result_delivery: ResultDelivery::File,
            max_depth: 1,
        }
    }
}

/// The deterministic diff comparator as a review source (the p2
/// composition seam: review dispatch is the only way to reach a
/// comparator).
struct DiffSource;

impl ComparatorSource for DiffSource {
    fn across_dimensions(&self) -> Result<Box<dyn Comparator>, String> {
        Ok(Box::new(DiffComparator::across_dimensions()))
    }
    fn for_dimension(&self, dimension: &str) -> Result<Box<dyn Comparator>, String> {
        Ok(Box::new(DiffComparator::for_dimension(dimension)))
    }
}

/// The recording escalation surface (§11.6's ticket requirement) —
/// proof-local: no real HumanEscalation adapter exists (Task 13).
#[derive(Default)]
struct RecordingEscalation {
    tickets: std::sync::Mutex<Vec<String>>,
}

impl gauntlet_ports::human_escalation::HumanEscalation for RecordingEscalation {
    fn escalate(
        &self,
        item: gauntlet_ports::human_escalation::EscalationItem,
        _: Deadline,
    ) -> Result<
        gauntlet_ports::human_escalation::EscalationTicket,
        gauntlet_ports::human_escalation::HumanEscalationError,
    > {
        self.tickets
            .lock()
            .expect("escalation lock")
            .push(item.summary);
        Ok(gauntlet_ports::human_escalation::EscalationTicket::new(
            gauntlet_ports::human_escalation::TicketId::new("ticket-p4")
                .expect("the fixture ticket id is valid"),
            EpochSeconds::new(BASE_INSTANT),
        ))
    }
    fn poll(
        &self,
        _: &gauntlet_ports::human_escalation::EscalationTicket,
        _: Deadline,
    ) -> Result<
        gauntlet_ports::human_escalation::EscalationState,
        gauntlet_ports::human_escalation::HumanEscalationError,
    > {
        Ok(gauntlet_ports::human_escalation::EscalationState::Open)
    }
}

// ---------------------------------------------------------------------------
// The composed fixture
// ---------------------------------------------------------------------------

struct Fixture {
    _scratch: Scratch,
    controller: RunController,
    state: Arc<CountingState>,
    runner: Arc<ScriptedBuilder>,
    evidence_root: PathBuf,
    workspace_root: PathBuf,
    key_path: PathBuf,
}

/// The meter factory over the REAL adapter (composition-root wiring: the
/// controller constructs its meter through this factory only).
fn meter_factory() -> Arc<dyn Fn(Budget) -> Arc<dyn BudgetMeter> + Send + Sync> {
    Arc::new(|budget| Arc::new(MeteredBudget::new(budget)))
}

impl Fixture {
    fn new(tag: &str, steps: Vec<BuilderStep>, hook: Option<Box<dyn Fn()>>) -> Self {
        Self::new_full(tag, steps, hook, None, false)
    }

    /// `observer_clock` overrides ONLY the observer's stamping clock;
    /// `absent_program` names the observer's program after the fixture's
    /// parsed check with a suffix no real program carries — the REAL
    /// command observer then cannot spawn and records the §12.3
    /// unavailability the assembler turns into unavailable-evidence gaps.
    /// The REAL diff comparator derives STABLE §13.5 residual keys from
    /// those gaps (same dimension, same expected text, same observed
    /// locator — no per-iteration evidence ref in them), so the recurrence
    /// genuinely counts across iterations (correction C4's composed leg).
    fn new_full(
        tag: &str,
        steps: Vec<BuilderStep>,
        hook: Option<Box<dyn Fn()>>,
        observer_clock: Option<Arc<DeterministicClock>>,
        absent_program: bool,
    ) -> Self {
        let scratch = Scratch::new(tag);
        let workspace_root = scratch.child("workspace");
        let evidence_root = scratch.child("evidence");
        let key_dir = scratch.child("ruler");
        let key_path = key_dir.join("p4-answer-key.md");
        std::fs::write(&key_path, ANSWER_KEY).expect("the scratch answer key is written");
        let evidence = Arc::new(
            FsEvidenceStore::open(&evidence_root).expect("the real fs evidence store opens"),
        );
        let state = CountingState::new(Arc::new(
            EmbeddedStateStore::open(&scratch.child("state").join("p4-state.db"))
                .expect("the real embedded state store opens"),
        ));
        let clock = Arc::new(DeterministicClock::start_at(EpochSeconds::new(
            BASE_INSTANT,
        )));
        // The check vocabulary (the p3 idiom): the program, its arguments,
        // and the artifact's relative path come from the REQUIRED fixture's
        // own declared self-check and changed paths — parsed, never spelled.
        let scenario = load_scenario(SCENARIO_NAME).expect("the REQUIRED fixture parses");
        let declared = scenario
            .payload()
            .expect("the fixture declares its payload")
            .self_checks()
            .first()
            .expect("the fixture declares exactly the self-check to re-run")
            .command();
        let mut tokens = declared.split_whitespace();
        let check_program = tokens
            .next()
            .expect("the declared check names a program")
            .to_owned();
        let check_args: Vec<String> = tokens.map(str::to_owned).collect();
        let artifact_relative = scenario
            .payload()
            .expect("the payload is declared")
            .changed_paths()
            .first()
            .expect("the fixture names its changed path")
            .clone();
        let runner = ScriptedBuilder::new(workspace_root.clone(), artifact_relative, steps);
        if let Some(hook) = hook {
            runner.set_hook(hook);
        }
        #[allow(clippy::arc_with_non_send_sync)] // single-threaded proof fixture wiring
        let observer_clock: Arc<dyn gauntlet_ports::clock::Clock> =
            observer_clock.unwrap_or_else(|| clock.clone());
        let probe_program = if absent_program {
            // Derived from the fixture's parsed program (never spelled);
            // no real program carries this suffix, so spawning fails.
            format!("{check_program}-gauntlet-absent")
        } else {
            check_program.clone()
        };
        #[allow(clippy::arc_with_non_send_sync)] // single-threaded proof fixture wiring
        let observers: Vec<Arc<dyn gauntlet_ports::observer::Observer>> =
            vec![Arc::new(CommandObserver::from_parts(
                CommandSpec::new(probe_program, check_args),
                FacetKind::Test,
                workspace_root.clone(),
                observer_clock,
                evidence.clone(),
                &proof_env(),
            ))];
        let contract = ComparisonContract::parse(COMPARISON_CONTRACT)
            .expect("the frozen comparison contract parses");
        #[allow(clippy::arc_with_non_send_sync)] // single-threaded proof fixture wiring
        let model = ModelComparatorSource::new(ModelComparator::with_clock(
            Arc::new(NeverAdmittedCritic),
            Arc::new(SeededIdGenerator::new("p4-proof-critic-identity").expect("a non-empty seed")),
            None,
            budget(),
            clock.clone(),
        ));
        #[allow(clippy::arc_with_non_send_sync)] // single-threaded proof fixture wiring
        let provider = Arc::new(FileReferenceProvider::from_sources(
            AnswerKeySource::Path(key_path.clone()),
            ManifestSource::Inline(MANIFEST.to_owned()),
            // The provider's freeze writes through its own REAL store
            // handle over the SAME content-addressed root.
            Rc::new(
                FsEvidenceStore::open(&evidence_root)
                    .expect("the provider's real evidence store opens"),
            ),
        ));
        #[allow(clippy::arc_with_non_send_sync)] // single-threaded proof fixture wiring
        let deps = ControllerDeps {
            state: state.clone(),
            evidence: evidence.clone(),
            ids: Arc::new(SeededIdGenerator::new("p4-proof-identity").expect("a non-empty seed")),
            clock,
            runner: runner.clone(),
            runner_identity: gauntlet_domain::RunnerIdentity::new("p4-scripted-builder")
                .expect("runner identity"),
            observers,
            comparison: ComparisonDeps {
                contract,
                deterministic: Arc::new(DiffSource),
                model: Arc::new(model),
                adversarial: None,
                review_mode: gauntlet_domain::ReviewMode::Adaptive,
                model_dispatches: 0,
            },
            ladder: Ladder::new(vec![LadderRung::new(
                RungKind::TestReport,
                Arc::new(FacetExitVerifier::new(&workspace_root)),
            )])
            .expect("the single-rung focused-test ladder composes"),
            reference: provider,
            answer_key: AnswerKeySource::Path(key_path.clone()),
            escalation: Arc::new(RecordingEscalation::default()),
            meter_factory: meter_factory(),
            budget: BudgetShape {
                limits: Limits::new(
                    ROUNDS_LIMIT,
                    PARALLELISM,
                    DEPTH_LIMIT,
                    WALL_LIMIT,
                    TOKEN_CEILING,
                    MicroUsd::new(COST_CEILING_MICRO),
                )
                .expect("proof limits usable"),
                recovery_reserve_pct: RESERVE_PCT,
                diagnostic: DiagnosticAllowance::new(
                    DIAGNOSTIC_TOKENS,
                    MicroUsd::new(DIAGNOSTIC_COST_MICRO),
                ),
            },
            policy: policy(),
            workspace_root: workspace_root.clone(),
            config_fingerprint: Digest::new("sha256", &"ab".repeat(32)).expect("digest"),
            environment_fingerprint: Digest::new("sha256", &"cd".repeat(32)).expect("digest"),
            intent: "p4 corrective loop proof".to_owned(),
        };
        let controller = RunController::compose(deps).expect("the composed controller builds");
        Self {
            _scratch: scratch,
            controller,
            state,
            runner,
            evidence_root,
            workspace_root,
            key_path,
        }
    }

    /// Every object the REAL fs store holds, read through the adapter's
    /// documented content-addressed layout (the pinned-record assertions
    /// walk these).
    fn stored_records(&self) -> Vec<String> {
        let objects = self.evidence_root.join("objects");
        let mut records = Vec::new();
        let mut stack = vec![objects];
        while let Some(dir) = stack.pop() {
            let entries = std::fs::read_dir(&dir).expect("the objects tree is walkable");
            for entry in entries {
                let entry = entry.expect("the objects entry is readable");
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if let Ok(bytes) = std::fs::read(&path) {
                    records.push(String::from_utf8_lossy(&bytes).into_owned());
                }
            }
        }
        records
    }
}

// ---------------------------------------------------------------------------
// The proof
// ---------------------------------------------------------------------------

/// P4's core: the injected defect is exposed by REAL observation, recorded
/// by REAL comparison and verification, routed as a corrective iteration
/// whose prompt carries the residual text only, and closed by the
/// corrected second iteration through the governed verification chain.
#[test]
fn an_injected_defect_is_exposed_compared_and_closed_by_a_routed_correction() {
    let fixture = Fixture::new(
        "main",
        vec![
            BuilderStep::Complete(DEFECTIVE_ARTIFACT),
            BuilderStep::Complete(CORRECTED_ARTIFACT),
        ],
        None,
    );
    let outcome = fixture
        .controller
        .start_run()
        .expect("the composed corrective loop runs to a terminal state");
    assert_eq!(
        outcome.terminal,
        TerminalState::Settled,
        "escalation summary: {:?}, iterations: {}",
        outcome.escalation_summary,
        outcome.iterations
    );
    assert_eq!(
        outcome.iterations, 2,
        "one defective pass, one routed correction"
    );

    // The routed correction: exactly two dispatches, and the SECOND prompt
    // carries the residual's expected/observed TEXT ONLY — no critic
    // trajectory rides it (§11.5 row 1).
    let prompts = fixture.runner.recorded_prompts();
    assert_eq!(prompts.len(), 2, "exactly one corrective re-dispatch");
    assert!(
        prompts[1].contains("expected: the test facet's instrument exits 0"),
        "the re-dispatch carries the residual text: {}",
        prompts[1]
    );
    assert!(
        !prompts[1].contains("critic"),
        "no critic vocabulary rides the re-dispatch: {}",
        prompts[1]
    );

    // REAL observation exposed the defect and then saw it closed: the
    // durable observation bindings exist for BOTH iterations (the gated
    // observation owner's write — counted at the store, which accepts only
    // authority-minted bindings).
    assert_eq!(fixture.state.observation_binds.get(), 2);
    assert!(
        fixture
            .state
            .observation_binding(
                &outcome.run_id,
                IterationNo::new(1).expect("iteration"),
                bound()
            )
            .expect("bindings readable")
            .is_some(),
        "iteration 1's bundle is durably bound"
    );
    assert!(
        fixture
            .state
            .observation_binding(
                &outcome.run_id,
                IterationNo::new(2).expect("iteration"),
                bound()
            )
            .expect("bindings readable")
            .is_some(),
        "iteration 2's bundle is durably bound"
    );

    // The gated evidence-producer write flowed at the required locator for
    // both iterations (the freshest is the closing one).
    assert_eq!(fixture.state.evidence_productions.get(), 2);
    let productions = fixture
        .state
        .evidence_productions_at(
            &outcome.run_id,
            &EvidenceLocator::new(REPORT_LOCATOR).expect("the fixture locator is valid"),
            bound(),
        )
        .expect("productions readable");
    assert_eq!(productions.len(), 2, "one production per iteration");
    assert_eq!(
        productions
            .last()
            .expect("ordered by anchor")
            .production()
            .iteration_no()
            .get(),
        2,
        "the freshest production anchors the closing iteration"
    );

    // The admitted invocations were durably recorded before polling (the
    // recording-runner discipline), one per dispatch.
    assert_eq!(fixture.state.role_invocations.get(), 2);

    // Verification FAILed and recorded it in iteration 1, PASSed in
    // iteration 2 — both through the governed chain, both leaving pinned
    // durable evidence. Before Task 13 B2, iteration 2's PASS honestly
    // recorded the settlement-precondition gap ("is not pinned to the
    // settling run") because the loop never performed the condition-2 pin;
    // B2's settle wiring pins the chain-minted PASS evidence and the
    // settlement SERVICE commits — so the honest state is now the DURABLE
    // settlement: a keyed settlement row naming the unit, plus the pinned
    // B.6 authority-decision record.
    let records = fixture.stored_records();
    assert!(
        records
            .iter()
            .any(|record| record.contains("the verification outcome is FAIL")),
        "iteration 1's FAILING verification is pinned evidence"
    );
    assert!(
        !records.is_empty(),
        "the composed loop pinned durable evidence"
    );
    let settlements = fixture
        .state
        .settlements(&outcome.run_id, bound())
        .expect("settlement rows readable");
    assert_eq!(
        settlements.len(),
        1,
        "iteration 2's PASS settles DURABLY through the service: {settlements:?}"
    );
    assert_eq!(settlements[0].unit_id().as_str(), UNIT);
    assert!(
        settlements[0].event_log_offset().get() > 0,
        "the settlement names the durable event that made it real"
    );
    assert!(
        records
            .iter()
            .any(|record| record.contains("settlement_authority_decision")),
        "the B.6 authority decision is pinned accompanying evidence"
    );
    // And the PASS is decisional: the unit SETTLED in the final checkpoint
    // — only a governed-chain PASS settles it.
    let checkpoint = fixture
        .state
        .resume(&outcome.run_id, bound())
        .expect("checkpoints readable")
        .expect("the run left a durable checkpoint");
    assert_eq!(
        checkpoint
            .unit_states
            .iter()
            .find(|(unit, _)| unit.as_str() == UNIT)
            .map(|(_, status)| *status),
        Some(UnitStatus::Settled),
        "the corrected iteration's verification PASS settled the unit"
    );
    // The locator holds the CLOSING facet's bytes (exit status 0), under
    // the one canonical workspace root.
    let locator_bytes = std::fs::read_to_string(fixture.workspace_root.join(REPORT_LOCATOR))
        .expect("the producer wrote the locator bytes");
    assert!(locator_bytes.contains("\"exit_status\":0"));
    assert!(locator_bytes.contains(&check_program_name()));

    // AK-037 at the composed level, loop path (correction C5): BOTH
    // iterations recorded their §15.1 selections durably — the residual-
    // driven correction on the defective iteration AND the completion on
    // the closing one — each WITH its rejected alternative (the §15.1
    // step-3 speculative sibling; a record without alternatives is
    // unrepresentable).
    let selections = fixture
        .state
        .correction_selections(&outcome.run_id, bound())
        .expect("selections readable");
    assert_eq!(
        selections.len(),
        2,
        "the correcting iteration and the closing iteration each recorded"
    );
    assert_eq!(selections[0].iteration_no().get(), 1);
    assert!(
        selections[0]
            .chosen()
            .summary()
            .contains("bounded re-dispatch"),
        "the residual-driven correction is the chosen record: {}",
        selections[0].chosen().summary()
    );
    assert!(
        selections[0].rejected().iter().any(|alternative| matches!(
            alternative.reason(),
            gauntlet_domain::RejectionReason::SpeculativeWhenBoundedAvailable
        )),
        "the correcting iteration recorded its rejected speculative alternative"
    );
    assert!(
        selections[1].chosen().summary().contains(UNIT),
        "the chosen completion names the completed unit: {}",
        selections[1].chosen().summary()
    );
    assert_eq!(selections[1].iteration_no().get(), 2);
    assert!(!selections[1].trace().is_empty());
    assert!(
        !selections[1].rejected().is_empty(),
        "the completion record carries its alternative"
    );

    // A checkpoint exists for every iteration boundary (§18.2).
    assert!(fixture.state.checkpoints.get() >= 2);

    // Final state exportable (§6.2).
    let exported = fixture
        .state
        .export_jsonl(&outcome.run_id, bound())
        .expect("the run exports");
    assert!(!exported.is_empty());
}

/// §11.4 step 3 at the composition root: an operator hand-edit to the
/// answer key while an attempt is IN FLIGHT cancels that attempt (the
/// per-tick strict re-read — the in-memory snapshot is never
/// authoritative) and the next boundary aborts the run with BOTH digests
/// as pinned evidence. The first attempt stalls so the edit lands while it
/// is genuinely in flight; without the re-read the stalled attempt would
/// ride into the stall-timeout path instead (re-dispatch, escalation) and
/// this test fails — the teeth (T3) break this exact coupling.
#[test]
fn a_mid_run_answer_key_edit_cancels_the_in_flight_attempt_and_aborts() {
    let fixture = Fixture::new(
        "mid-run-edit",
        vec![BuilderStep::Stall, BuilderStep::Stall],
        None,
    );
    // The edit lands at the first dispatch (inside invoke): the in-flight
    // attempt's unit projection changes under it.
    let key_path = fixture.key_path.clone();
    fixture.runner.set_hook(move || {
        let edited = ANSWER_KEY.replace(
            "the checked artifact passes its focused re-run check",
            "the OPERATOR-EDITED claim",
        );
        std::fs::write(&key_path, edited).expect("the operator edit lands");
    });
    let outcome = fixture
        .controller
        .start_run()
        .expect("the loop reaches a terminal state");
    assert_eq!(outcome.terminal, TerminalState::Aborted);
    let ReferenceBreach::Mutated { expected, actual } =
        outcome.reference_breach.expect("the breach is classified")
    else {
        panic!("a moved ruler is a mutation, never unavailability");
    };
    assert_ne!(expected, actual);
    // BOTH digests are pinned durable evidence.
    let records = fixture.stored_records();
    let mutation: Vec<_> = records
        .iter()
        .filter(|record| record.contains("reference_mutated"))
        .collect();
    assert_eq!(mutation.len(), 1, "exactly one mutation record is pinned");
    assert!(mutation[0].contains(&expected.to_string()));
    assert!(mutation[0].contains(&actual.to_string()));
    // The §11.4 step-3 cancellation fired on the IN-FLIGHT attempt (the
    // durable Cancelled terminal event), not a stall timeout.
    assert_eq!(
        fixture.state.cancelled_events.get(),
        1,
        "the re-read cancelled the edited in-flight attempt"
    );
    // No retry: exactly one dispatch happened.
    assert_eq!(fixture.runner.recorded_prompts().len(), 1);
}

/// §11.5 row 9 through the REAL deterministic diff comparator (correction
/// C4): the observer's instrument CANNOT SPAWN (the REAL command observer
/// over a program no real installation carries), so every iteration's
/// observation records the §12.3 unavailability as evidence gaps and the
/// REAL comparator derives the SAME residual — same dimension, same
/// expected text, same §13.5 observed locator — three times. The
/// recurrence routes the composed loop into the bounded §15.2 diagnostic
/// cycle; the gaps are the input-driven apparatus fact (observation
/// failure), the verification is honestly INCONCLUSIVE (no evidence was
/// ever produced), the disconfirming observation is honestly false, and
/// the cycle ends INCONCLUSIVE with the evidence quarantined and the
/// escalation submitted — the run never re-patches past the threshold.
#[test]
fn a_recurring_residual_through_the_real_diff_comparator_enters_bounded_diagnosis() {
    let fixture = Fixture::new_full(
        "diagnosis",
        // Enough steps for the three recurrence iterations plus slack.
        vec![
            BuilderStep::Complete(DEFECTIVE_ARTIFACT),
            BuilderStep::Complete(DEFECTIVE_ARTIFACT),
            BuilderStep::Complete(DEFECTIVE_ARTIFACT),
            BuilderStep::Complete(DEFECTIVE_ARTIFACT),
        ],
        None,
        None,
        true,
    );
    let outcome = fixture
        .controller
        .start_run()
        .expect("the composed loop runs to a terminal state");
    assert_eq!(
        outcome.terminal,
        TerminalState::Inconclusive,
        "the recurrence routes to diagnosis, not exhaustion or escalation \
         (escalation: {:?}, iterations: {})",
        outcome.escalation_summary,
        outcome.iterations
    );
    assert!(
        outcome
            .escalation_summary
            .as_deref()
            .is_some_and(|summary| summary.contains("inconclusive")),
        "§15.2's inconclusive terminal escalates: {:?}",
        outcome.escalation_summary
    );
    let records = fixture.stored_records();
    assert!(
        records
            .iter()
            .any(|record| record.contains("\"quarantine\":true")),
        "the quarantined evidence is pinned durable evidence"
    );
    // The recurrence was REAL: three iterations recorded the unavailable
    // test probe in their own bundles.
    let unavailable = records
        .iter()
        .filter(|record| record.contains("observation_bundle@v1") && record.contains("unavailable"))
        .count();
    assert_eq!(
        unavailable, 3,
        "three iterations observed the same unavailable instrument"
    );
    // The cycle's honest spend: four phases (the surviving hypothesis
    // refused the fifth) at the divided per-phase allowance.
    const DIVIDED_PHASES_BOOKED: u64 = 4;
    const DIVISION_PHASE_COUNT: u64 = 5;
    let checkpoint = fixture
        .state
        .resume(&outcome.run_id, bound())
        .expect("checkpoints readable")
        .expect("the run left a durable checkpoint");
    assert_eq!(
        checkpoint.budget_spent.diagnostic_tokens(),
        DIVIDED_PHASES_BOOKED * DIAGNOSTIC_TOKENS / DIVISION_PHASE_COUNT,
        "the diagnostic allowance was divided per phase, not consumed whole"
    );
    // The unit never settled: no evidence was ever produced to settle on.
    assert_ne!(
        checkpoint
            .unit_states
            .iter()
            .find(|(unit, _)| unit.as_str() == UNIT)
            .map(|(_, status)| *status),
        Some(UnitStatus::Settled),
        "an inconclusive verification never settles"
    );
}

/// AK-037 at the composed level, full candidate space: the REAL selector
/// over the REAL meter and the REAL durable store records the chosen
/// correction together with rejected alternatives and their typed reasons
/// — the same durable path the controller's loop record travels. (The
/// controller's own loop-path candidate set is fixed by the B3 slice —
/// one candidate, the completed unit — so the alternatives-bearing
/// selection is composed here, at the proof, through the same selector,
/// meter, and store the loop uses.)
#[test]
fn correction_selection_durably_carries_rejected_alternatives_with_reasons() {
    let scratch = Scratch::new("selection");
    let state = Arc::new(
        EmbeddedStateStore::open(&scratch.child("state").join("p4-selection.db"))
            .expect("the real embedded state store opens"),
    );
    let run_id = RunId::new(SELECTION_RUN_ULID).expect("the fixture run id is a valid ULID");
    let reference_digest = Digest::new("sha256", &"ee".repeat(32)).expect("a hex digest is valid");
    let version = gauntlet_domain::ReferenceVersion::with_evidence(
        gauntlet_domain::ReferenceId::new("p4-selection-reference")
            .expect("the fixture reference id is valid"),
        1,
        reference_digest.clone(),
        EvidenceRef::new(&format!("sha256:{}", "0".repeat(64))).expect("the fixture ref is valid"),
    );
    let run = gauntlet_domain::Run::new(gauntlet_domain::RunDefinition {
        id: run_id.clone(),
        created_at: EpochSeconds::new(BASE_INSTANT),
        reference_version: version,
        intent: "p4 composed selection fixture".to_owned(),
        config_fingerprint: Digest::new("sha256", &"ab".repeat(32)).expect("digest"),
        environment_fingerprint: Digest::new("sha256", &"cd".repeat(32)).expect("digest"),
        budget: budget(),
        parent_run: None,
    })
    .expect("the fixture run is valid");
    state
        .record_run(&run, bound())
        .expect("the fixture run is durable");

    let meter = MeteredBudget::new(budget());
    let unit = UnitId::new(UNIT).expect("the fixture unit is valid");
    let facts = |requested_tokens: u64| gauntlet_domain::BudgetFacts {
        requested_rounds: 1,
        requested_wall_seconds: 1,
        requested_tokens,
        requested_cost_usd: MicroUsd::new(1),
        remaining_rounds: meter.remaining().rounds,
        remaining_wall_seconds: meter.remaining().wall_seconds,
        remaining_tokens: meter.remaining().tokens,
        remaining_cost_usd: meter.remaining().cost_usd,
    };
    // Three candidates: one governance-blocked on authority (§17.1 — the
    // correction needs an authority the run cannot exercise), one
    // affordable bounded correction, one cheaper-shaped bounded
    // alternative that loses the lexicographic priority.
    let blocked = Candidate {
        correction: CorrectionDescriptor::new(
            "have the operator re-freeze the ruler under human authority".to_owned(),
            CorrectionShape::Speculative,
            vec![unit.clone()],
            facts(0),
        )
        .expect("the blocked descriptor is complete"),
        required_authority: Authority::Human,
        tier: BlastRadius::Local,
        dependency_ready: true,
        settlement: SettlementAccessibility::HumanSignoff,
        impact: Impact {
            worst_severity: Severity::Major,
            residuals_addressed: 1,
        },
        information_gain: gauntlet_app::control::select::InformationGain::Incremental,
        confidence: gauntlet_app::control::select::Confidence::NeverAttempted,
        units_touched: 1,
        rollback_units: 1,
    };
    let chosen = Candidate {
        correction: CorrectionDescriptor::new(
            format!("complete {unit} under the run's own authority"),
            CorrectionShape::Bounded,
            vec![unit.clone()],
            facts(10),
        )
        .expect("the chosen descriptor is complete"),
        required_authority: Authority::Critic,
        tier: BlastRadius::Contained,
        dependency_ready: true,
        settlement: SettlementAccessibility::Instrument,
        impact: Impact {
            worst_severity: Severity::Major,
            residuals_addressed: 1,
        },
        information_gain: gauntlet_app::control::select::InformationGain::Incremental,
        confidence: gauntlet_app::control::select::Confidence::LastSucceeded,
        units_touched: 1,
        rollback_units: 1,
    };
    let runner_up = Candidate {
        correction: CorrectionDescriptor::new(
            "retry the whole unit from a clean workspace".to_owned(),
            CorrectionShape::Bounded,
            vec![unit.clone()],
            facts(20),
        )
        .expect("the runner-up descriptor is complete"),
        required_authority: Authority::Critic,
        tier: BlastRadius::Local,
        dependency_ready: true,
        settlement: SettlementAccessibility::Instrument,
        impact: Impact {
            worst_severity: Severity::Major,
            residuals_addressed: 1,
        },
        information_gain: gauntlet_app::control::select::InformationGain::Incremental,
        confidence: gauntlet_app::control::select::Confidence::NeverAttempted,
        units_touched: 1,
        rollback_units: 1,
    };
    let selection = CorrectionSelector::new()
        .select(&SelectionContext {
            run_id: run_id.clone(),
            iteration_no: IterationNo::new(1).expect("iterations are 1-based"),
            meter: &meter,
            exercisable_authority: Authority::Critic,
            dispatchable_tiers: vec![BlastRadius::Contained, BlastRadius::Local],
            candidates: vec![blocked, chosen, runner_up],
        })
        .expect("the composed selector decides");
    assert_eq!(
        selection.chosen().summary(),
        format!("complete {unit} under the run's own authority")
    );
    let rejected = selection.rejected();
    assert!(
        rejected.len() >= 2,
        "both the governance-blocked and the ranked-out alternative are recorded"
    );
    assert!(
        rejected.iter().any(|alternative| matches!(
            alternative.class(),
            gauntlet_domain::RejectionClass::Authority
        )),
        "the authority-blocked alternative carries its class"
    );
    assert!(
        rejected
            .iter()
            .all(|alternative| !format!("{:?}", alternative.reason()).is_empty()),
        "every rejected alternative carries its typed reason"
    );

    // The record is durable through the SAME store path the loop's record
    // travels, and reads back whole: chosen, alternatives, and trace.
    state
        .record_correction_selection(&selection, bound())
        .expect("the selection records durably");
    let read_back = state
        .correction_selections(&run_id, bound())
        .expect("selections readable");
    assert_eq!(read_back.len(), 1);
    assert_eq!(read_back[0], selection);
}
