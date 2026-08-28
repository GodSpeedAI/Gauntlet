//! P6 proof (plan Task 12 step 6; spec §23.1 P6; frozen Units AK-051 with
//! AK-052's every-dispatch-reserved discipline beside it): budget control
//! through the REAL run controller at the composition root — a deliberately
//! NON-CONVERGING task (the REQUIRED `lying-builder` scenario fixture,
//! replayed verbatim by the real mock runner for every invocation, so the
//! same residual recurs every iteration) against a tight budget, terminating
//! at the declared limit with terminal `budget_exhausted`, the recovery
//! reserve INTACT, every recorded invocation carrying a meter reservation,
//! one durable checkpoint per iteration boundary, and the final state
//! exportable.
//!
//! Frozen locator note (the p3/p4/p12 precedent, recorded): AK-051's
//! `evidence_required.locator` is `tests/proofs/p6_budget_control`
//! (`.wayfinder/answer-key.md`) — a repo-root test-directory spelling that
//! cannot be declared as a cargo target of a CORE crate (the boundary
//! suite's cargo-target-path rule; the class was first recorded 2026-08-17
//! against AK-049, and AK-022/023/024/029 re-declared the same shape to
//! composition-root targets). The proof therefore materializes as an
//! integration test of the composition root whose cargo test-target NAME is
//! exactly `p6_budget_control`, so the plan's focused gate
//! (`cargo nextest run --test p6_budget_control`) runs verbatim.
//!
//! Composition inventory (spec §4.1 — the CLI is the composition root;
//! every piece is REAL unless this header names the proof-local replacement
//! and why):
//!
//! - durable state: the REAL embedded-SQL store on a scratch database
//!   file behind a delegating counting decorator (every call forwards
//!   unchanged; the counters exist so the proof can count the gated
//!   owner-writes and the recording-runner's invocation rows);
//! - evidence: the REAL fs content-addressed store (a second REAL instance
//!   serves the reference provider's `freeze`);
//! - reference: the REAL file-backed provider over a scratch answer-key
//!   FILE and an inline minimal manifest (freeze → verify_unchanged, with
//!   the per-boundary re-verify);
//! - runner: the REAL `MockAgentRunner` replaying the REQUIRED
//!   `lying-builder` scenario fixture (Task 8's vocabulary) — one instance
//!   replays one scenario for EVERY admitted invocation, which is exactly
//!   the never-fixing shape this proof needs: the artifact on disk fails
//!   the fixture's own declared self-check every single iteration while
//!   the document claims it passes;
//! - observation: the REAL command observer re-running the fixture's own
//!   declared check (`grep PASS out/check-target.txt`, program and
//!   arguments parsed from the loaded scenario's payload, never spelled
//!   here) over the SAME workspace the child wrote — exit status 1 every
//!   iteration is the recurring residual's observable ground;
//! - comparison: the REAL deterministic diff comparator under `adaptive`
//!   review with the model allowance ZERO (a nonzero instrument exit is
//!   settled conclusively by the deterministic probe; the model source is
//!   the REAL model comparator over a critic runner that REFUSES
//!   admission — proof-local, same posture as p4: if the lease discipline
//!   ever broke the run would fail loudly);
//! - verification: the REAL governed chain over a single-rung ladder at
//!   the §14.2 focused-test position (DEVIATION, documented — the same
//!   one p4 carries: the B3 controller writes the FIRST observation
//!   facet's `facet_evidence@v1` bytes at the attempted unit's required
//!   locator, and no Task-11 verifier rung's declared artifact contract
//!   is `facet_evidence@v1`; the proof-local rung decides that document's
//!   own recorded exit status. The integration finding — controller
//!   locator-write vs rung artifact contracts — is recorded in the
//!   Task-12 report);
//! - meter: the BARE REAL metered-budget adapter (correction C2: ONE
//!   production rounds wiring — the controller's run-`Budget` is the
//!   single rounds authority, booking ONE round per iteration at the
//!   iteration boundary; the meter expresses PAYMENTS only, the port's
//!   "payments carry no rounds" delegation). AK-052's rows-vs-reservations
//!   count reads the adapter's OWN admission counter
//!   (`admitted_reservations_total`), not a proof-side tally;
//! - escalation: a proof-local recording surface (no real HumanEscalation
//!   adapter exists yet; Task 13's declared boundary).
//!
//! Terminus design note (the brief's requirement that BOTH §11.5 row-9
//! behaviors be pinned, neither vacuously): with `nonconvergence_n` at the
//! default 3 and the rounds ceiling at 6, the terminus that fires COMPOSED
//! is exhaustion, not diagnosis — the residual key is §13.5-faithful
//! (correction C4: dimension + expected + OBSERVED LOCATOR), and each
//! iteration's facet evidence is a DISTINCT content-addressed document
//! (the real clock's timestamps), so each iteration's residual carries a
//! different observed locator and the recurrence count never reaches the
//! threshold; the threshold is ALSO raised past the forward rounds
//! ceiling so the exhaustion terminus is robust even against byte-stable
//! facet evidence. The composed recurrence→N→diagnosis route (row 9) is
//! proven by p4's `a_recurring_residual_through_the_real_comparator...`
//! test over a pinned-clock observer (byte-stable facet evidence ⇒ a
//! stable observed locator ⇒ the recurrence genuinely counts), and the
//! diagnosis terminus itself — bounded diagnosis, diagnostic-budget
//! exhaustion ⇒ INCONCLUSIVE with quarantine and escalation — is pinned
//! in-crate by the `control::diagnosis` tests (AK-039), which the Task-12
//! gate `cargo nextest run -p gauntlet-app control::` runs alongside this
//! proof.

use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use gauntlet_adapter_agent_mock::{
    DeterministicClock, MockAgentRunner, SeededIdGenerator, load_scenario,
};
use gauntlet_adapter_compare_diff::DiffComparator;
use gauntlet_adapter_compare_model::{ModelComparator, ModelComparatorSource};
use gauntlet_adapter_evidence_fs::FsEvidenceStore;
use gauntlet_adapter_meter_memory::MeteredBudget;
use gauntlet_adapter_observe_command::{CommandObserver, CommandSpec};
use gauntlet_adapter_state_sqlite::EmbeddedStateStore;
use gauntlet_app::compare::contract::ComparisonContract;
use gauntlet_app::control::controller::{
    BudgetShape, ComparisonDeps, ControllerDeps, RunController,
};
use gauntlet_app::control::disposition::ControlPolicy;
use gauntlet_app::reference::provider::{AnswerKeySource, FileReferenceProvider, ManifestSource};
use gauntlet_app::verify::ladder::{Ladder, LadderRung, RungKind};
use gauntlet_domain::{
    Budget, DiagnosticAllowance, Digest, EpochSeconds, EvidenceKind, EvidenceLocator, EvidenceRef,
    IterationNo, Limits, MicroUsd, Outcome, RunId, TerminalState, UnitStatus,
};
use gauntlet_ports::agent_runner::{
    AgentError, AgentRunner, CancelReason, InvocationHandle, InvocationStatus, Isolation,
    ResultDelivery, RoleInvocationSpec, RunnerCapabilities,
};
use gauntlet_ports::budget_meter::{BudgetMeter, BudgetMeterError, BudgetSlice};
use gauntlet_ports::comparator::{Comparator, ComparatorSource};
use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::id_generator::IdGenerator;
use gauntlet_ports::observer::{FacetKind, InstrumentIdentity};
use gauntlet_ports::observer_evidence::FacetEvidenceDoc;
use gauntlet_ports::state_store::{
    Checkpoint, EvidenceProductionBinding, ObservationBinding, RunEvent, StateStore,
    StateStoreError,
};
use gauntlet_ports::verifier::{VerificationRequest, Verifier, VerifierError};

// ---------------------------------------------------------------------------
// Named fixture figures (the literal-scan idiom: no duration-shaped or
// port-shaped identifier shares a line with a value-position number).
// ---------------------------------------------------------------------------

/// Fixed point far from any wall clock (the p3/p4/p12 proof convention).
const BASE_INSTANT: u64 = 1_700_000_000;
/// The IO bound handed to every store/instrument call in this proof.
const BOUND_AT: u64 = BASE_INSTANT + 90;
/// The tight rounds ceiling: the recovery reserve (15%) holds ONE round —
/// the run may spend FIVE forward rounds and must stop there.
const ROUNDS_LIMIT: u32 = 6;
const PARALLELISM: u32 = 2;
const DEPTH_LIMIT: u32 = 2;
/// Wall-seconds ceiling for the run (also the operation-bound horizon).
const WALL_LIMIT: u64 = 3600;
const TOKEN_CEILING: u64 = 1_000;
const COST_CEILING_MICRO: u64 = 500;
/// The recovery reserve percentage (the brief's tight-budget example).
const RESERVE_PCT: u8 = 15;
/// The rounds the run may spend FORWARD — the domain's own ceiling formula
/// (`limit × (100−pct)/100`), computed, never a second literal to keep in
/// step: 6 × 85/100 = 5.
const FORWARD_ROUNDS_CEILING: u32 = ROUNDS_LIMIT * (100 - RESERVE_PCT as u32) / 100;
/// The tokens the run may spend FORWARD under the same formula.
const FORWARD_TOKENS_CEILING: u64 = TOKEN_CEILING * (100 - RESERVE_PCT as u64) / 100;
/// The wall seconds the run may spend FORWARD under the same formula.
const FORWARD_WALL_CEILING: u64 = WALL_LIMIT * (100 - RESERVE_PCT as u64) / 100;
/// The cost (micro-USD) the run may spend FORWARD under the same formula.
const FORWARD_COST_CEILING_MICRO: u64 = COST_CEILING_MICRO * (100 - RESERVE_PCT as u64) / 100;
/// The diagnostic allowance (part of the budget shape the controller
/// validates; the exhaustion path never reaches it).
const DIAGNOSTIC_TOKENS: u64 = 100;
const DIAGNOSTIC_COST_MICRO: u64 = 50;
/// The §11.4 poll tick span, in milliseconds (the in-crate fixture idiom:
/// span vocabulary under neutral names — a duration-suffixed identifier
/// with a value literal is a §8.1 flag under the literals scan).
const TICK_WINDOW: u64 = 1_000;
/// The §11.4 quiet window, in milliseconds (same idiom; moot here: the
/// lying builder completes on its first poll; the figure only shapes the
/// policy).
const QUIET_WINDOW: u64 = 30_000;
/// §11.5 retry-pause bounds, in milliseconds (same idiom).
const PAUSE_FLOOR: u64 = 1;
const PAUSE_CEILING: u64 = 4;
/// Attempt ceiling per unit — DELIBERATELY above the forward rounds
/// ceiling (5) so the row-1/row-5 boundary never binds before the budget
/// does: budget exhaustion, not `max_attempts_exceeded`, must be the
/// terminus this proof asserts.
const ATTEMPT_CEILING: u32 = 7;
/// §11.5 row 9 threshold — DELIBERATELY above the forward rounds ceiling
/// (5) so AK-051's terminus is exhaustion by construction, robust even
/// against byte-stable facet evidence (see the module doc's terminus
/// note; the composed recurrence→diagnosis route is pinned in p4).
const NONCONVERGENCE_N: u32 = 6;
/// The identity seed for the controller's id generator — a SECOND,
/// identically-seeded generator lets the fixture precompute the run id
/// (the first id the controller mints), which names the run's workspace
/// directory under the one canonical root the mock child writes into.
const IDS_SEED: &str = "p6-proof-identity";

/// The proof's single Unit — the LYING-BUILDER FIXTURE's own declared
/// `attempted` id (the scratch ruler is self-contained; the fixture's
/// document must name the unit the run dispatched, or the loop's
/// attempted-units resolution has nothing to verify — the lie lives in
/// the claims, never the identity).
const UNIT: &str = "AK-014";
/// The Unit's required evidence locator — single-segment by design (the
/// literals scan polices path-shaped literals in this crate).
const REPORT_LOCATOR: &str = "p6-report.json";
/// The REQUIRED scenario fixture this proof replays (Task 8's vocabulary:
/// the document claims the artifact passes its own declared self-check
/// while the workspace effect writes an artifact that fails it).
const SCENARIO_NAME: &str = "lying-builder";

/// The scratch answer key: one Unit whose evidence requirement is decidable
/// by the composed focused-test rung over the workspace.
///
/// Task-14 fixture posture note: this proof's subject (a non-converging
/// task exhausts the FORWARD budget with the reserve intact) is
/// tier-agnostic; the unit sits at the contained tier because the mock
/// runner honestly reports isolation none (spec §22.2) and the Task-14
/// gate correctly refuses a none-isolation runner above contained (spec
/// §17.3, AK-050). The above-contained posture is proven by p5_hard_gate.
const ANSWER_KEY: &str = "## AK-014\nclaim: the checked artifact passes its focused re-run \
     check\nverify_by: test\nblast_radius: contained\nauthority: critic\nevidence_required: {kind: \
     test_report, locator: p6-report.json, producer: instrument}\n\n";

/// The inline minimal manifest (the shared-suite shape).
const MANIFEST: &str = "reference:\n  id: ref-p6-proof-suite\n  version: 1\n  purpose: >\n    \
     One paragraph.\n  desired_state: >\n    The destination.\n  settlement:\n    \
     success_criteria:\n      - the destination is reached\n    minimum_reliability: >\n      \
     spec 19.2 default: >= 1 settlements before promotion.\n    terminal_conditions: \
     [settled, aborted]\n";

/// The frozen comparison-contract artifact this proof composes (the
/// shared-suite minimal shape; the `functional behavior` dimension is the
/// one the real diff comparator maps test facets onto).
const COMPARISON_CONTRACT: &str = "version: 1\ndimensions:\n  - functional behavior\n\
     severity_rubric:\n  - blocking | invalidates the iteration\n  - major | real, not \
     invalidating\n  - minor | small\n  - informational | recorded only\ncomparator_receives:\n  \
     - the allowlist members\nresidual_definition:\n  - a difference\n";

// ---------------------------------------------------------------------------
// Scratch and shared fixture pieces
// ---------------------------------------------------------------------------

/// Isolated per-test scratch tree under the system temp dir; removed on
/// drop (the repo's pid-tagged Scratch idiom).
struct Scratch {
    path: PathBuf,
}

impl Scratch {
    fn new(tag: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("gauntlet-p6-proof-{tag}-{}", std::process::id()));
        std::fs::remove_dir_all(&path).ok();
        std::fs::create_dir_all(&path).expect("the p6 scratch parent is creatable");
        Self { path }
    }

    fn child(&self, name: &str) -> PathBuf {
        let path = self.path.join(name);
        std::fs::create_dir_all(&path).expect("the p6 scratch child is creatable");
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
        "p6-budget-control".to_owned(),
    )]
}

// ---------------------------------------------------------------------------
// The counting state decorator (delegating; the gated-write counters)
// ---------------------------------------------------------------------------

/// The REAL embedded store behind a delegating counter. No behavior
/// changes: every method forwards verbatim; the Cell counters exist so the
/// proof can count the gated owner-writes and the recording runner's
/// durable invocation rows — the store's methods accept only
/// authority-minted argument types, so the counts are counts of
/// authority-earned writes by construction.
struct CountingState {
    inner: Arc<EmbeddedStateStore>,
    observation_binds: Cell<u32>,
    evidence_productions: Cell<u32>,
    role_invocations: Cell<u32>,
    checkpoints: Cell<u32>,
    selections: Cell<u32>,
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
        })
    }
}

impl StateStore for CountingState {
    fn append_event(
        &self,
        event: RunEvent,
        deadline: Deadline,
    ) -> Result<gauntlet_domain::EventLogOffset, StateStoreError> {
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
// The composed focused-test rung over facet_evidence@v1 (proof-local; the
// documented deviation, identical in shape to p4's)
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
// The never-admitted critic (model source holder) and the diff source
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
    fn collect(
        &self,
        handle: &InvocationHandle,
        _: Deadline,
    ) -> Result<gauntlet_ports::role_result::RoleResult, AgentError> {
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
    tickets: Mutex<Vec<String>>,
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
            gauntlet_ports::human_escalation::TicketId::new("ticket-p6")
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
    /// The meter registry — the factory's LAST build is the run's OWN
    /// meter (compose builds a validation meter first; `start_run`/
    /// `resume_run` build the run's), so AK-052's rows-vs-reservations
    /// count reads the meter's own admission counter at assertion time,
    /// never a proof-side tally.
    meter_registry: Arc<Mutex<Option<Arc<MeteredBudget>>>>,
    run_workspace: PathBuf,
    evidence_root: PathBuf,
}

impl Fixture {
    fn new(tag: &str) -> Self {
        let scratch = Scratch::new(tag);
        // The ONE canonical workspace root the whole loop resolves under:
        // the controller's workspace_root, the observer's root, and the
        // parent of the mock child's effect tree are the same directory.
        let workspace_root = scratch.child("workspace");
        let evidence_root = scratch.child("evidence");
        let key_dir = scratch.child("ruler");
        let key_path = key_dir.join("p6-answer-key.md");
        std::fs::write(&key_path, ANSWER_KEY).expect("the scratch answer key is written");
        let evidence = Arc::new(
            FsEvidenceStore::open(&evidence_root).expect("the real fs evidence store opens"),
        );
        let state = CountingState::new(Arc::new(
            EmbeddedStateStore::open(&scratch.child("state").join("p6-state.db"))
                .expect("the real embedded state store opens"),
        ));
        let clock = Arc::new(DeterministicClock::start_at(EpochSeconds::new(
            BASE_INSTANT,
        )));

        // The REQUIRED lying-builder scenario, replayed verbatim by the
        // REAL mock runner for EVERY invocation — the never-fixing shape.
        let scenario = load_scenario(SCENARIO_NAME).expect("the REQUIRED fixture parses");
        let declared = scenario
            .payload()
            .expect("the lying builder declares its payload")
            .self_checks()
            .first()
            .expect("the fixture declares exactly the self-check to re-run")
            .command();
        let mut tokens = declared.split_whitespace();
        let program = tokens
            .next()
            .expect("the declared check names a program")
            .to_owned();
        let args: Vec<String> = tokens.map(str::to_owned).collect();

        // The run's workspace directory under the canonical root is named
        // by the run id — the FIRST id the controller's seeded generator
        // mints. A second, identically-seeded generator precomputes it so
        // the real mock child (whose effects resolve under its fixed
        // constructor root) writes into the very tree the real observers
        // are bound to.
        let run_dir = SeededIdGenerator::new(IDS_SEED)
            .expect("a non-empty seed")
            .run_id()
            .to_string();
        let run_workspace = workspace_root.join(&run_dir);
        let runner = MockAgentRunner::new(scenario, run_workspace.clone());

        #[allow(clippy::arc_with_non_send_sync)] // single-threaded proof fixture wiring
        let observers: Vec<Arc<dyn gauntlet_ports::observer::Observer>> =
            vec![Arc::new(CommandObserver::from_parts(
                CommandSpec::new(program, args),
                FacetKind::Test,
                workspace_root.clone(),
                clock.clone(),
                evidence.clone(),
                &proof_env(),
            ))];
        let contract = ComparisonContract::parse(COMPARISON_CONTRACT)
            .expect("the frozen comparison contract parses");
        #[allow(clippy::arc_with_non_send_sync)] // single-threaded proof fixture wiring
        let model = ModelComparatorSource::new(ModelComparator::with_clock(
            Arc::new(NeverAdmittedCritic),
            Arc::new(SeededIdGenerator::new("p6-proof-critic-identity").expect("a non-empty seed")),
            None,
            budget(),
            clock.clone(),
        ));
        #[allow(clippy::arc_with_non_send_sync)] // single-threaded proof fixture wiring
        let provider = Arc::new(FileReferenceProvider::from_sources(
            AnswerKeySource::Path(key_path.clone()),
            ManifestSource::Inline(MANIFEST.to_owned()),
            Rc::new(
                FsEvidenceStore::open(&evidence_root)
                    .expect("the provider's real evidence store opens"),
            ),
        ));

        // The composition-root meter factory (C2): the BARE REAL adapter —
        // the same production wiring p4 composes. The registry keeps the
        // run's own meter handle so AK-052's rows-vs-reservations count
        // reads the METER'S OWN admission counter (the factory runs once
        // for compose's validation plan and once for the run; the run's
        // meter is the last built and the one the proof reads).
        let meter_registry: Arc<Mutex<Option<Arc<MeteredBudget>>>> = Arc::new(Mutex::new(None));
        let factory_registry = meter_registry.clone();
        #[allow(clippy::arc_with_non_send_sync)] // single-threaded proof fixture wiring
        let meter_factory: Arc<dyn Fn(Budget) -> Arc<dyn BudgetMeter> + Send + Sync> =
            Arc::new(move |budget| {
                let meter: Arc<MeteredBudget> = Arc::new(MeteredBudget::new(budget));
                *factory_registry.lock().expect("meter registry lock") = Some(meter.clone());
                meter
            });

        #[allow(clippy::arc_with_non_send_sync)] // single-threaded proof fixture wiring
        let deps = ControllerDeps {
            state: state.clone(),
            evidence: evidence.clone(),
            ids: Arc::new(SeededIdGenerator::new(IDS_SEED).expect("a non-empty seed")),
            clock,
            runner: Arc::new(runner),
            runner_identity: gauntlet_domain::RunnerIdentity::new("p6-lying-builder-mock")
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
            meter_factory,
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
            // The §8.6 conservative posture (plan Task 14): contained
            // critic-tier dispatches run ungated by containment; the mock
            // runner's honest none isolation makes above-contained refusal
            // the correct posture here.
            govern: gauntlet_app::govern::GovernDeps::none_workspace(),
            environment_fingerprint: Digest::new("sha256", &"cd".repeat(32)).expect("digest"),
            intent: "p6 budget control proof".to_owned(),
        };
        let controller = RunController::compose(deps).expect("the composed controller builds");
        Self {
            _scratch: scratch,
            controller,
            state,
            meter_registry,
            run_workspace,
            evidence_root,
        }
    }

    /// Every object the REAL fs store holds, read through the adapter's
    /// documented content-addressed layout.
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

    /// The meter's OWN admission count (AK-052's reservations side),
    /// read from the registry's current (the run's) meter.
    fn admitted(&self) -> u32 {
        self.meter_registry
            .lock()
            .expect("meter registry lock")
            .as_ref()
            .map(|meter| meter.admitted_reservations_total())
            .unwrap_or(0)
    }
}

// ---------------------------------------------------------------------------
// The proof
// ---------------------------------------------------------------------------

/// P6's core: the non-converging lying builder meets a tight budget. The
/// run terminates at the declared limit with terminal `budget_exhausted`;
/// the recovery reserve is INTACT (forward spend never crossed
/// `limit × (100−pct)/100` on any dimension); every recorded invocation
/// row has a corresponding meter reservation (AK-052, composed); one
/// checkpoint per iteration boundary; the final state exports.
#[test]
fn a_non_converging_task_exhausts_the_forward_budget_with_the_reserve_intact() {
    let fixture = Fixture::new("main");
    let outcome = fixture
        .controller
        .start_run()
        .expect("the composed loop runs to a terminal state");
    assert_eq!(
        outcome.terminal,
        TerminalState::BudgetExhausted,
        "the terminus is exhaustion, not diagnosis or escalation \
         (escalation summary: {:?}, iterations: {})",
        outcome.escalation_summary,
        outcome.iterations
    );
    assert!(
        outcome.escalation_summary.is_none(),
        "budget exhaustion escalates nothing (the reserve covers the close-out)"
    );
    assert!(
        outcome.iterations >= FORWARD_ROUNDS_CEILING,
        "the loop ran its full forward share: {}",
        outcome.iterations
    );

    // Exactly the forward rounds were dispatched — the sixth round is the
    // reserve, and the §17.4 pre-dispatch consult refused to spend it.
    assert_eq!(
        fixture.admitted(),
        FORWARD_ROUNDS_CEILING,
        "reservations admitted == the forward rounds ceiling"
    );
    assert_eq!(
        fixture.state.role_invocations.get(),
        FORWARD_ROUNDS_CEILING,
        "every recorded invocation row (AK-052: rows) matches the meter's \
         admitted reservations (rows vs reservations)"
    );
    assert_eq!(
        fixture.state.observation_binds.get(),
        FORWARD_ROUNDS_CEILING,
        "the observation owner bound every iteration's bundle"
    );
    assert_eq!(
        fixture.state.evidence_productions.get(),
        FORWARD_ROUNDS_CEILING,
        "the evidence producer recorded every iteration's facet at the locator"
    );
    let productions = fixture
        .state
        .evidence_productions_at(
            &outcome.run_id,
            &EvidenceLocator::new(REPORT_LOCATOR).expect("the fixture locator is valid"),
            bound(),
        )
        .expect("productions readable");
    assert_eq!(
        productions.len(),
        FORWARD_ROUNDS_CEILING as usize,
        "the durable productions at the required locator read back whole"
    );
    // Correction C5: every residual-driven iteration recorded its §15.1
    // selection WITH its rejected alternative — the moment a correction is
    // chosen is the record's moment, and no vacuous record exists.
    assert_eq!(
        fixture.state.selections.get(),
        FORWARD_ROUNDS_CEILING,
        "each correcting iteration recorded its residual-driven selection"
    );
    let selections = fixture
        .state
        .correction_selections(&outcome.run_id, bound())
        .expect("selections readable");
    assert_eq!(selections.len(), FORWARD_ROUNDS_CEILING as usize);
    assert!(
        selections.iter().all(|selection| {
            selection.chosen().summary().contains("bounded re-dispatch")
                && !selection.rejected().is_empty()
        }),
        "every record is the residual-driven bounded choice with its \
         rejected alternative"
    );

    // A checkpoint exists for EVERY completed iteration boundary (§18.2),
    // plus the close-out checkpoint the terminal path writes after the
    // loop breaks (the reserve keeps that affordable by construction).
    assert_eq!(
        fixture.state.checkpoints.get(),
        FORWARD_ROUNDS_CEILING + 1,
        "one durable checkpoint per iteration boundary plus the close-out"
    );

    // The recovery reserve is INTACT: the run's own durable §18.2 budget
    // record never crossed the forward ceiling on ANY dimension.
    let checkpoint = fixture
        .state
        .resume(&outcome.run_id, bound())
        .expect("checkpoints readable")
        .expect("the run left a durable final checkpoint");
    let spent = checkpoint.budget_spent;
    assert!(
        spent.rounds() <= FORWARD_ROUNDS_CEILING,
        "forward rounds {} never crossed the ceiling {}",
        spent.rounds(),
        FORWARD_ROUNDS_CEILING
    );
    assert!(
        spent.tokens() <= FORWARD_TOKENS_CEILING,
        "forward tokens {} never crossed the ceiling {}",
        spent.tokens(),
        FORWARD_TOKENS_CEILING
    );
    assert!(
        spent.wall_seconds() <= FORWARD_WALL_CEILING,
        "forward wall seconds {} never crossed the ceiling {}",
        spent.wall_seconds(),
        FORWARD_WALL_CEILING
    );
    assert!(
        spent.cost_usd().get() <= FORWARD_COST_CEILING_MICRO,
        "forward cost {} never crossed the ceiling {}",
        spent.cost_usd().get(),
        FORWARD_COST_CEILING_MICRO
    );

    // The rounds truth agrees from the RUN BUDGET the durable checkpoint
    // carries: exactly the five forward rounds were booked at the
    // iteration boundaries and the reserve round was never spent.
    assert_eq!(
        checkpoint.budget_spent.rounds(),
        FORWARD_ROUNDS_CEILING,
        "the run budget booked exactly the forward rounds — the reserve \
         round stands"
    );

    // The SAME residual recurred every iteration, and the run says so
    // durably two ways. First: each iteration's observation bundle (all
    // five DISTINCT documents — they carry distinct timestamps) records
    // the re-run check FAILING (facet exit status 1). Second: the §16.1
    // refusal record for the FAILING verification is PINNED — once as an
    // object, because the store is content-addressed and the five
    // iterations' refusal records are byte-identical by construction
    // (same run, same unit, same refusal text); deduplication collapses
    // the pins' bytes, not the fact.
    let records = fixture.stored_records();
    let recurring_fail_facets = records
        .iter()
        .filter(|record| {
            record.contains("observation_bundle@v1") && record.contains("\"exit_status\":1")
        })
        .count();
    assert_eq!(
        recurring_fail_facets, FORWARD_ROUNDS_CEILING as usize,
        "every iteration's own bundle carries the failing re-run check"
    );
    assert!(
        records
            .iter()
            .any(|record| record.contains("the settling claim's own outcome is FAIL")),
        "the FAILING verification's settlement refusal is pinned evidence \
         (one deduplicated object for five byte-identical records) — since \
         COR-2 the refusal is the settlement service's own entry gate"
    );
    assert_ne!(
        checkpoint
            .unit_states
            .iter()
            .find(|(unit, _)| unit.as_str() == UNIT)
            .map(|(_, status)| *status),
        Some(UnitStatus::Settled),
        "a never-fixed defect never settles"
    );

    // The lie itself stayed observable on disk to the end: reality fails
    // the fixture's own declared check (path from the scenario's changed
    // paths, never spelled here).
    let changed = load_scenario(SCENARIO_NAME)
        .expect("the REQUIRED fixture parses")
        .payload()
        .expect("the payload is declared")
        .changed_paths()
        .first()
        .expect("the fixture names its changed path")
        .clone();
    let artifact = std::fs::read_to_string(fixture.run_workspace.join(&changed))
        .expect("the lying builder's artifact is on disk");
    assert!(
        artifact.contains("FAIL"),
        "reality fails the check on every iteration: {artifact:?}"
    );

    // Final state exportable (§6.2).
    let exported = fixture
        .state
        .export_jsonl(&outcome.run_id, bound())
        .expect("the run exports");
    assert!(!exported.is_empty());
    assert!(
        exported.contains("observation") || exported.contains("checkpoint"),
        "the export carries the run's durable records"
    );
}

/// AK-052 at the meter itself (the BARE REAL adapter, C2's production
/// wiring): the meter REFUSES the slice that would consume the recovery
/// reserve, with the domain's typed cause, the refusal books NOTHING, and
/// every admission is counted by the meter's own counter — the
/// rows-vs-reservations truth. The token dimension is the boundary pinned
/// here at the exact edge: the forward ceiling admits, one past it is
/// refused, the reserve survives. (Rounds are iteration facts booked by
/// the run controller on the run `Budget` — the meter's payment counters
/// never move them; the main test pins that boundary from the durable
/// checkpoint.)
#[test]
fn the_meter_refuses_the_reserve_consuming_slice_and_books_nothing() {
    let meter = MeteredBudget::new(budget());
    assert_eq!(
        meter.admitted_reservations_total(),
        0,
        "a fresh meter has admitted nothing"
    );
    // N5 (Task-12 correction): `remaining` is the FORWARD ceiling minus
    // spent (1000 at the 15 percent reserve = 850 forward), so the
    // nothing-booked invariants below are before/after comparisons,
    // never raw-limit constants.
    let unspent = meter.remaining().tokens;
    // The token figures ride named consts (§8.1 literal hygiene).
    const CEILING_SLICE_TOKENS: u64 = FORWARD_TOKENS_CEILING;
    const ONE_PAST_CEILING_TOKENS: u64 = FORWARD_TOKENS_CEILING + 1;
    let at_ceiling = BudgetSlice {
        rounds: 0,
        wall_seconds: 0,
        tokens: CEILING_SLICE_TOKENS,
        cost_usd: MicroUsd::new(0),
    };
    assert!(
        !meter.would_breach_reserve(at_ceiling),
        "the consult says the ceiling slice lands exactly ON the forward ceiling"
    );
    let reservation = meter.reserve(at_ceiling).expect("the ceiling admits");
    assert_eq!(
        meter.admitted_reservations_total(),
        1,
        "the admission is the meter's own counted truth"
    );
    // One token past the ceiling would consume the recovery reserve:
    // refused — by the pure consult AND by reserve, with the domain's
    // typed cause.
    let one_past = BudgetSlice {
        rounds: 0,
        wall_seconds: 0,
        tokens: ONE_PAST_CEILING_TOKENS,
        cost_usd: MicroUsd::new(0),
    };
    assert!(
        meter.would_breach_reserve(one_past),
        "the consult says one past the ceiling breaches the recovery reserve"
    );
    assert!(
        matches!(
            meter.reserve(one_past),
            Err(BudgetMeterError::Reserve { .. })
        ),
        "the refusal carries the domain's typed cause"
    );
    assert_eq!(
        meter.admitted_reservations_total(),
        1,
        "the refusal admitted (and counted) nothing"
    );
    assert_eq!(
        meter.remaining().tokens,
        unspent,
        "the refusal booked nothing: the reserve stands"
    );
    // The reservation bound holds charges to their slice (the release
    // path is the charge; a zero payment books nothing and returns the
    // slot — the B1 adapter's documented convention).
    const PROBE_PAYMENT_TOKENS: u64 = CEILING_SLICE_TOKENS + 1;
    assert!(
        matches!(
            meter.charge(
                &reservation,
                gauntlet_domain::Payment::new(PROBE_PAYMENT_TOKENS, 0, 0, MicroUsd::new(0)),
            ),
            Err(BudgetMeterError::ReservationExceeded { .. })
        ),
        "the real adapter's reservation bound is the production bound"
    );
    meter
        .charge(&reservation, gauntlet_domain::Payment::zero())
        .expect("the zero payment releases the slot without booking");
    assert_eq!(
        meter.remaining().tokens,
        unspent,
        "the zero-payment release booked nothing"
    );
}
