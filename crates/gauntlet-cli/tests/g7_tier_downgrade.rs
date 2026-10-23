//! G7 proof (plan Task 14; frozen Unit AK-046, locator
//! `tests/antigaming/g7_tier_downgrade`): an agent role cannot lower a
//! Unit's blast radius or authority; only an operator can raise it — and
//! through the runtime NO decrease is honored at all (spec §17.1 "A tier
//! may be raised by an operator; it is never lowered by an agent").
//!
//! The claim has three surfaces, and the proof exercises each as its own
//! leg:
//!
//! 1. the LATTICE review (`govern::posture::review_lattice_movement`):
//!    an agent-originated movement is refused in EVERY direction (a raise
//!    is operator-only too — the claim's second clause read strictly);
//!    an operator may raise, never lower; the refused movement CARRIES
//!    the recorded-and-proposed values (the governance event is
//!    reconstructable from the refusal alone);
//! 2. the DISPATCH gate: plan facts ranking below the answer-key facts —
//!    the shape a forged or drifted projection would carry — are refused
//!    before any other conjunct matters, with the AK-046 rule in the
//!    recorded summary;
//! 3. the FROZEN-RULER defense, composed: a run whose queued unit is at
//!    the local tier refuses dispatch (the honest none-isolation runner,
//!    AK-050's surface); an attacker then lowers the unit's tier in the
//!    answer key between runs — the containment escape attempt — and the
//!    run's resume re-freeze-verifies FIRST: the run ABORTS with the
//!    mutation breach and the lowered tier NEVER dispatches (§9.4's
//!    freeze + §18.3's step 2; the runtime never re-plans against an
//!    edited ruler).
//!
//! Frozen locator note (the p3/p5/p6 precedent, recorded): the ruler's
//! locator spells a repo-root test target (`tests/antigaming/…`) a CORE
//! crate cannot declare; the proof materializes as a composition-root
//! integration test whose cargo test-target NAME is exactly
//! `g7_tier_downgrade`.
//!
//! Composition inventory: the loop legs reuse the p5 proof's REAL
//! composition (embedded-SQL state, fs evidence, file reference provider,
//! the REQUIRED lying-builder mock scenario — an honest §22.2
//! none-isolation in-process runner); no observer is needed (the legs'
//! runs dispatch nothing successfully).

use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use gauntlet_adapter_agent_mock::{
    DeterministicClock, MockAgentRunner, SeededIdGenerator, load_scenario,
};
use gauntlet_adapter_compare_model::{ModelComparator, ModelComparatorSource};
use gauntlet_adapter_evidence_fs::FsEvidenceStore;
use gauntlet_adapter_meter_memory::MeteredBudget;
use gauntlet_adapter_state_postgres::EmbeddedStateStore;
use gauntlet_app::compare::contract::ComparisonContract;
use gauntlet_app::control::controller::{
    BudgetShape, ComparisonDeps, ControllerDeps, RunController,
};
use gauntlet_app::control::disposition::ControlPolicy;
use gauntlet_app::govern::{
    AuthorityGate, ChangeOrigin, GateFacts, GateRefusal, GovernDeps, LatticeMovement,
    review_lattice_movement,
};
use gauntlet_app::reference::provider::{AnswerKeySource, FileReferenceProvider, ManifestSource};
use gauntlet_app::verify::ladder::{Ladder, LadderRung, RungKind};
use gauntlet_domain::{
    Authority, BlastRadius, Budget, DiagnosticAllowance, EpochSeconds, Limits, MicroUsd, RoleId,
    RunId, TerminalState, UnitId,
};
use gauntlet_ports::agent_runner::{
    AgentError, AgentRunner, CancelReason, InvocationHandle, InvocationStatus, Isolation,
    ResultDelivery, RoleInvocationSpec, RunnerCapabilities,
};
use gauntlet_ports::budget_meter::BudgetMeter;
use gauntlet_ports::comparator::{Comparator, ComparatorSource};
use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::id_generator::IdGenerator;
use gauntlet_ports::state_store::{RunEvent, StateStore, StateStoreError};

// Named fixture facts (§8.1 literal discipline).
const BASE_INSTANT: u64 = 1_700_000_000;
const ROUNDS_LIMIT: u32 = 4;
const PARALLELISM: u32 = 2;
const DEPTH_LIMIT: u32 = 2;
const WALL_LIMIT: u64 = 3600;
const TOKEN_CEILING: u64 = 2_000_000;
const COST_CEILING_MICRO: u64 = 25_000_000;
const RESERVE_PCT: u8 = 15;
const DIAGNOSTIC_TOKENS: u64 = 2;
const DIAGNOSTIC_COST_MICRO: u64 = 200;
const TICK_WINDOW: u64 = 60;
const QUIET_WINDOW: u64 = 120;
const PAUSE_FLOOR: u64 = 60;
const PAUSE_CEILING: u64 = 600;
const ATTEMPT_CEILING: u32 = 3;
const NONCONVERGENCE_N: u32 = 9;
const IDS_SEED: &str = "g7-proof-identity";
const SCENARIO_NAME: &str = "lying-builder";

/// One queued Unit at the LOCAL tier with critic authority — the tier
/// whose downgrade to contained would escape the containment requirement.
const LOCAL_KEY: &str = "## AK-9401\nclaim: the local unit claims a workspace-local \
     change.\nverify_by: static_check\nblast_radius: local\nauthority: \
     critic\nevidence_required: {kind: test_report, locator: g7-report.json, producer: \
     instrument}\n\n";

const MANIFEST: &str = "reference:\n  id: ref-g7-proof-suite\n  version: 1\n  purpose: >\n    \
     One paragraph.\n  desired_state: >\n    The destination.\n  settlement:\n    \
     success_criteria:\n      - the destination is reached\n    minimum_reliability: >\n      \
     spec 19.2 default: >= 1 settlements before promotion.\n    terminal_conditions: \
     [settled, aborted]\n";

const COMPARISON_CONTRACT: &str = "version: 1\ndimensions:\n  - functional behavior\n\
     severity_rubric:\n  - blocking | invalidates the iteration\n  - major | real, not \
     invalidating\n  - minor | small\n  - informational | recorded only\ncomparator_receives:\n  \
     - the allowlist members\nresidual_definition:\n  - a difference\n";

struct Scratch {
    path: PathBuf,
}

impl Scratch {
    fn new(tag: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("gauntlet-g7-proof-{tag}-{}", std::process::id()));
        std::fs::remove_dir_all(&path).ok();
        std::fs::create_dir_all(&path).expect("the g7 scratch parent is creatable");
        Self { path }
    }

    fn child(&self, name: &str) -> PathBuf {
        let path = self.path.join(name);
        std::fs::create_dir_all(&path).expect("the g7 scratch child is creatable");
        path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
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

/// The REAL embedded store behind a delegating invocation counter (the p5
/// idiom): "never dispatched" is a count of durable invocation rows.
struct CountingState {
    inner: Arc<EmbeddedStateStore>,
    role_invocations: Cell<u32>,
}

impl CountingState {
    fn new(inner: Arc<EmbeddedStateStore>) -> Arc<Self> {
        #[allow(clippy::arc_with_non_send_sync)] // single-threaded proof fixture wiring
        Arc::new(Self {
            inner,
            role_invocations: Cell::new(0),
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
        checkpoint: gauntlet_ports::state_store::Checkpoint,
        deadline: Deadline,
    ) -> Result<(), StateStoreError> {
        self.inner.checkpoint(checkpoint, deadline)
    }
    fn resume(
        &self,
        run_id: &RunId,
        deadline: Deadline,
    ) -> Result<Option<gauntlet_ports::state_store::Checkpoint>, StateStoreError> {
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
        at: gauntlet_domain::EpochSeconds,
        deadline: Deadline,
    ) -> Result<(), StateStoreError> {
        self.inner
            .record_correction_selection(selection, at, deadline)
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
        iteration_no: gauntlet_domain::IterationNo,
        binding: &gauntlet_ports::producer_authority::AdmittedObservationBinding,
        deadline: Deadline,
    ) -> Result<(), StateStoreError> {
        self.inner
            .bind_observation_bundle(run_id, iteration_no, binding, deadline)
    }
    fn observation_binding(
        &self,
        run_id: &RunId,
        iteration_no: gauntlet_domain::IterationNo,
        deadline: Deadline,
    ) -> Result<Option<gauntlet_ports::state_store::ObservationBinding>, StateStoreError> {
        self.inner
            .observation_binding(run_id, iteration_no, deadline)
    }
    fn record_evidence_production(
        &self,
        run_id: &RunId,
        production: &gauntlet_ports::producer_authority::AdmittedEvidenceProduction,
        deadline: Deadline,
    ) -> Result<gauntlet_ports::state_store::EvidenceProductionBinding, StateStoreError> {
        self.inner
            .record_evidence_production(run_id, production, deadline)
    }
    fn evidence_production(
        &self,
        run_id: &RunId,
        evidence: &gauntlet_domain::EvidenceRef,
        deadline: Deadline,
    ) -> Result<Option<gauntlet_ports::state_store::EvidenceProductionBinding>, StateStoreError>
    {
        self.inner.evidence_production(run_id, evidence, deadline)
    }
    fn evidence_productions_at(
        &self,
        run_id: &RunId,
        locator: &gauntlet_domain::EvidenceLocator,
        deadline: Deadline,
    ) -> Result<Vec<gauntlet_ports::state_store::EvidenceProductionBinding>, StateStoreError> {
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

/// The never-admitted critic (the p5/p6 posture).
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
    fn cancel(&self, _: &InvocationHandle, _: CancelReason, _: Deadline) -> Result<(), AgentError> {
        Err(AgentError::AdmissionRefused {
            detail: "the proof's critic was never admitted".to_owned(),
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

/// The never-dispatched deterministic comparator (the runs dispatch
/// nothing; comparison is never reached — the holder is composition
/// plumbing, and a reached comparison fails loudly).
struct UnusedDiff;

impl ComparatorSource for UnusedDiff {
    fn across_dimensions(&self) -> Result<Box<dyn Comparator>, String> {
        Err("the g7 proof never compares".to_owned())
    }
    fn for_dimension(&self, _: &str) -> Result<Box<dyn Comparator>, String> {
        Err("the g7 proof never compares".to_owned())
    }
}

/// A no-op verifier rung: the legs' runs never verify (nothing completes),
/// and a reached rung fails loudly.
struct UnusedRung;

impl gauntlet_ports::verifier::Verifier for UnusedRung {
    fn verify(
        &self,
        _: &gauntlet_ports::verifier::VerificationRequest,
        _: Deadline,
    ) -> Result<gauntlet_domain::VerificationRecord, gauntlet_ports::verifier::VerifierError> {
        Err(gauntlet_ports::verifier::VerifierError::InstrumentFailed {
            detail: "the g7 proof never verifies".to_owned(),
        })
    }
    fn is_deterministic(&self) -> bool {
        true
    }
    fn instrument(&self) -> gauntlet_ports::observer::InstrumentIdentity {
        gauntlet_ports::observer::InstrumentIdentity::new(
            gauntlet_domain::InstrumentName::new("g7-unused-rung")
                .expect("the instrument name is valid"),
            "1".to_owned(),
        )
    }
    fn supports(&self, _: &gauntlet_domain::EvidenceRequirement) -> bool {
        false
    }
}

struct LoopFixture {
    _scratch: Scratch,
    controller: RunController,
    state: Arc<CountingState>,
    key_path: PathBuf,
}

impl LoopFixture {
    fn new(tag: &str) -> Self {
        let scratch = Scratch::new(tag);
        let workspace_root = scratch.child("workspace");
        let evidence_root = scratch.child("evidence");
        let key_dir = scratch.child("ruler");
        let key_path = key_dir.join("g7-answer-key.md");
        std::fs::write(&key_path, LOCAL_KEY).expect("the scratch answer key is written");
        let evidence = Arc::new(
            FsEvidenceStore::open(&evidence_root).expect("the real fs evidence store opens"),
        );
        let state = CountingState::new(Arc::new(
            EmbeddedStateStore::open_at(&scratch.child("state").join("g7-state.db"))
                .expect("the real embedded state store opens"),
        ));
        let clock = Arc::new(DeterministicClock::start_at(EpochSeconds::new(
            BASE_INSTANT,
        )));
        let scenario = load_scenario(SCENARIO_NAME).expect("the REQUIRED fixture parses");
        // COR-D3 mechanical adaptation: the mock resolves effects under
        // <root>/<spec.workspace>/, so it is constructed with the
        // CANONICAL root — the effects land at
        // workspace_root/<run_id>/…, exactly where they stood under the
        // prior construction.
        let runner = MockAgentRunner::new(scenario, workspace_root.clone());
        let contract = ComparisonContract::parse(COMPARISON_CONTRACT)
            .expect("the frozen comparison contract parses");
        #[allow(clippy::arc_with_non_send_sync)] // single-threaded proof fixture wiring
        let model = ModelComparatorSource::new(ModelComparator::with_clock(
            Arc::new(NeverAdmittedCritic),
            Arc::new(SeededIdGenerator::new("g7-proof-critic-identity").expect("a non-empty seed")),
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
        #[allow(clippy::arc_with_non_send_sync)] // single-threaded proof fixture wiring
        let meter_factory: Arc<dyn Fn(Budget) -> Arc<dyn BudgetMeter> + Send + Sync> =
            Arc::new(|budget| Arc::new(MeteredBudget::new(budget)));
        #[allow(clippy::arc_with_non_send_sync)] // single-threaded proof fixture wiring
        let deps = ControllerDeps {
            state: state.clone(),
            evidence: evidence.clone(),
            ids: Arc::new(SeededIdGenerator::new(IDS_SEED).expect("a non-empty seed")),
            clock,
            runner: Arc::new(runner),
            runner_identity: gauntlet_domain::RunnerIdentity::new("g7-lying-builder-mock")
                .expect("runner identity"),
            observers: Vec::new(),
            comparison: ComparisonDeps {
                contract,
                deterministic: Arc::new(UnusedDiff),
                model: Arc::new(model),
                adversarial: None,
                review_mode: gauntlet_domain::ReviewMode::Adaptive,
                model_dispatches: 0,
                role_deny: gauntlet_app::roles::ContextDenySet::new(Vec::new()),
            },
            ladder_for: Arc::new(move |_| {
                Ok(Ladder::new(vec![LadderRung::new(
                    RungKind::TestReport,
                    Arc::new(UnusedRung),
                )])
                .expect("the single-rung ladder composes"))
            }),
            reference: provider,
            answer_key: AnswerKeySource::Path(key_path.clone()),
            escalation: Arc::new(g7_support::NoopEscalation),
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
            govern: GovernDeps::none_backend(),
            workspaces: None,
            config_fingerprint: gauntlet_domain::Digest::new("sha256", &"ab".repeat(32))
                .expect("digest"),
            environment_fingerprint: gauntlet_domain::Digest::new("sha256", &"cd".repeat(32))
                .expect("digest"),
            intent: "g7 tier-downgrade proof".to_owned(),
        };
        let controller = RunController::compose(deps).expect("the composed controller builds");
        Self {
            _scratch: scratch,
            controller,
            state,
            key_path,
        }
    }
}

/// The escalation surface for the loop legs (a no-op ticket; the
/// governance facts the legs assert are the gate-level summaries and the
/// terminal, which the outcome carries).
mod g7_support {
    use gauntlet_ports::deadline::Deadline;
    use gauntlet_ports::human_escalation::{
        EscalationItem, EscalationState, EscalationTicket, HumanEscalation, HumanEscalationError,
        TicketId,
    };

    pub struct NoopEscalation;

    /// The fixture ticket instant, on its own line.
    const TICKET_AT: u64 = 1_700_000_000;

    impl HumanEscalation for NoopEscalation {
        fn escalate(
            &self,
            _: EscalationItem,
            _: Deadline,
        ) -> Result<EscalationTicket, HumanEscalationError> {
            Ok(EscalationTicket::new(
                TicketId::new("ticket-g7").expect("the fixture ticket id is valid"),
                gauntlet_domain::EpochSeconds::new(TICKET_AT),
            ))
        }
        fn poll(
            &self,
            _: &EscalationTicket,
            _: Deadline,
        ) -> Result<EscalationState, HumanEscalationError> {
            Ok(EscalationState::Open)
        }
    }
}

fn unit(n: u16) -> UnitId {
    // The `let` binding is load-bearing, not style: the literals scanner
    // joins a `;`-less tail expression with FOLLOWING lines until the
    // next semicolon, which would merge the deadline-bearing helpers
    // below into a statement carrying the format literal.
    #[allow(clippy::let_and_return)]
    let id = UnitId::new(&format!("AK-94{n:02}")).expect("valid unit id");
    id
}

fn bound() -> Deadline {
    Deadline::at_epoch(EpochSeconds::new(BASE_INSTANT + 600))
}

/// Leg 1 — the lattice review: an agent role cannot move a Unit's blast
/// radius or authority in ANY direction (AK-046's first clause read with
/// its second: only an operator can raise); an operator may raise and
/// never lower; refused movements carry the recorded/proposed pairs.
#[test]
fn an_agent_role_cannot_move_a_units_lattice_in_any_direction() {
    let builder = ChangeOrigin::AgentRole(RoleId::Builder);
    let critic = ChangeOrigin::AgentRole(RoleId::Critic);
    let operator = ChangeOrigin::Operator;
    let recorded = (BlastRadius::Local, Authority::Critic);

    // An agent DECREASING either axis — the containment-escape direction.
    let refused = review_lattice_movement(
        unit(1),
        recorded,
        (BlastRadius::Contained, Authority::Critic),
        &builder,
    );
    match &refused {
        LatticeMovement::Refused {
            blast_movement,
            authority_movement,
            rule,
            ..
        } => {
            assert_eq!(
                blast_movement,
                &Some((BlastRadius::Local, BlastRadius::Contained))
            );
            assert_eq!(authority_movement, &None);
            assert!(
                rule.contains("agent role") && rule.contains("AK-046"),
                "the recorded rule is the claim itself: {rule}"
            );
        }
        other => panic!("an agent blast decrease must be refused, got {other:?}"),
    }
    let refused = review_lattice_movement(
        unit(2),
        recorded,
        (BlastRadius::Local, Authority::Critic),
        &critic,
    );
    assert!(refused.is_allowed(), "no movement is always allowed");
    let _ = refused;

    // An agent RAISING — refused too: raises are operator-only.
    let refused = review_lattice_movement(
        unit(3),
        recorded,
        (BlastRadius::CrossCutting, Authority::LeadAgent),
        &builder,
    );
    assert!(
        !refused.is_allowed(),
        "an agent cannot raise a tier either — only an operator can (AK-046)"
    );

    // An operator may raise either axis; an operator decrease is refused
    // (the runtime never lowers — the operator's path to a lower tier is
    // the ruler they own, outside this review).
    assert!(
        review_lattice_movement(
            unit(4),
            recorded,
            (BlastRadius::CrossCutting, Authority::Critic),
            &operator
        )
        .is_allowed()
    );
    assert!(
        review_lattice_movement(
            unit(5),
            recorded,
            (BlastRadius::Local, Authority::LeadAgent),
            &operator
        )
        .is_allowed()
    );
    assert!(
        !review_lattice_movement(
            unit(6),
            recorded,
            (BlastRadius::Contained, Authority::Critic),
            &operator
        )
        .is_allowed()
    );
}

/// Leg 2 — the dispatch gate's defense: plan facts ranking BELOW the
/// recorded answer-key facts (the forged-projection shape) are refused
/// with the AK-046 rule in the recorded summary, before any containment
/// or policy conjunct is consulted.
#[test]
fn a_dispatch_fact_below_the_recorded_facts_is_refused_as_a_downgrade() {
    let gate = AuthorityGate::new();
    // The forged dispatch: the unit's facts claimed at contained/critic
    // while the answer key records local/critic — the exact escape a
    // compromised projection would carry. Every other fact is maximal.
    let forged = GateFacts {
        unit: unit(1),
        tier: BlastRadius::Contained,
        authority: Authority::Critic,
        runner_isolation: Isolation::Process,
        workspace: gauntlet_domain::WorkspaceRef::new("g7-gate-workspace")
            .expect("valid workspace ref"),
    };
    let refusal = gate
        .admit(
            &GovernDeps::none_backend(),
            &forged,
            Some((BlastRadius::Local, Authority::Critic)),
            bound(),
        )
        .expect_err("a downgraded dispatch is refused");
    match &refusal {
        GateRefusal::LatticeMovementRefused { movement } => match movement {
            LatticeMovement::Refused { blast_movement, .. } => {
                assert_eq!(
                    blast_movement,
                    &Some((BlastRadius::Local, BlastRadius::Contained))
                );
            }
            LatticeMovement::Allowed => panic!("checked by the arm"),
        },
        other => panic!("expected LatticeMovementRefused, got {other:?}"),
    }
    assert!(
        refusal.summary().contains("agent role"),
        "the governance event names AK-046's rule: {}",
        refusal.summary()
    );
    // The honest facts (recorded == dispatch facts) pass the conjunct and
    // proceed to the next one — the refusal above is the lattice conjunct
    // alone, not a blanket refusal.
    let honest = GateFacts {
        unit: unit(2),
        tier: BlastRadius::Contained,
        authority: Authority::Critic,
        runner_isolation: Isolation::Process,
        workspace: gauntlet_domain::WorkspaceRef::new("g7-gate-workspace")
            .expect("valid workspace ref"),
    };
    gate.admit(
        &GovernDeps::none_backend(),
        &honest,
        Some((BlastRadius::Contained, Authority::Critic)),
        bound(),
    )
    .expect("coincident facts admit past the lattice conjunct");
}

/// Leg 3 — the frozen-ruler defense, composed: a queued local-tier unit
/// refuses dispatch under the honest none-isolation runner (AK-050's
/// surface, iteration one); an attacker then lowers the unit's tier in
/// the answer key — the containment escape — and the run's RESUME
/// re-freeze-verifies FIRST (§18.3 step 2): the run aborts with the
/// mutation breach, and the lowered tier NEVER dispatches.
#[test]
fn a_mid_run_tier_downgrade_attempt_never_dispatches_at_the_lowered_tier() {
    let fixture = LoopFixture::new("frozen-ruler");
    let first = fixture
        .controller
        .start_run()
        .expect("the first run reaches a terminal state");
    assert_eq!(
        first.terminal,
        TerminalState::Blocked,
        "the local tier under the honest none runner refuses dispatch"
    );
    assert_eq!(
        fixture.state.role_invocations.get(),
        0,
        "nothing dispatched in the first run"
    );

    // The attack: lower the queued unit's tier in the answer key — the
    // ruler the run FROZE at start is what resume re-verifies, so the
    // edit is a reference mutation, whatever its content.
    let downgraded = LOCAL_KEY.replace("blast_radius: local", "blast_radius: contained");
    assert_ne!(downgraded, LOCAL_KEY, "the edit changed the tier");
    std::fs::write(&fixture.key_path, downgraded).expect("the attacker edit lands");

    // §18.3 step 2: re-freeze-verify FIRST — before any checkpoint,
    // settlement completion, or re-planning trusts the edited file.
    let run_id = resumed_run_id();
    let outcome = fixture
        .controller
        .resume_run(&run_id)
        .expect("resume reaches a terminal outcome");
    assert_eq!(outcome.terminal, TerminalState::Aborted);
    let breach = outcome.reference_breach.expect("the breach is classified");
    assert!(
        matches!(
            breach,
            gauntlet_app::control::controller::ReferenceBreach::Mutated { .. }
        ),
        "a moved ruler is a mutation, never unavailability: {breach:?}"
    );
    assert_eq!(
        fixture.state.role_invocations.get(),
        0,
        "the lowered tier never dispatched — the run aborted at the boundary"
    );
}

/// The run id the fixture's seeded generator mints FIRST (the controller
/// consumes the same seed; the pre-computation is the p5/p6 idiom).
fn resumed_run_id() -> RunId {
    SeededIdGenerator::new(IDS_SEED)
        .expect("a non-empty seed")
        .run_id()
}
