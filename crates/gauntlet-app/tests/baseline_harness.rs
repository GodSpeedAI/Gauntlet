//! The baseline harness over a FAKE executor (task 26, slice B2): the
//! four postures' dispatch shapes, the blind critic envelope (tooth T2's
//! cross-check at the composed-run level), the derived scenario matrix the
//! charter pins, and the report's exact JSON key set (the plan gate's
//! python assert, mirrored through the repo's own JSON codec).
//!
//! The fake executor scripts the B1 child protocol's discrimination
//! behavior per scenario; profile D's legs resolve against a fake
//! `StateStore` carrying the durable records a run would leave. The REAL
//! binary path (CLI `BaselineExecutor` over the fixture children) is
//! slice B3's gate test; this file pins the APP's contracts B3 wires
//! against.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use gauntlet_app::baseline::{
    BaselineExecutor, BaselineHarness, BaselineProfile, BaselineReport, BaselineScenario,
    DispatchOutcome, DispatchRequest, HeldOutBasis, HeldOutReading, HeldOutVerdict, LoopRunOutcome,
    Payment, Role, TaskError, held_out_relative_path, parse_task,
};
use gauntlet_domain::{
    Budget, Digest, EpochSeconds, EventLogOffset, EvidenceKind, EvidenceLocator, EvidenceRef,
    InvocationId, InvocationState, IterationNo, Limits, MicroUsd, Outcome, Producer, ReferenceId,
    ReferenceVersion, Run, RunDefinition, RunId, SettlementRecord, Spent, TerminalState, UnitId,
    VerificationRef,
};
use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::state_store::{
    Checkpoint, RunEvent, RunLifecycleFacts, StateStore, StateStoreError,
};
use gauntlet_ports::verification_record::{
    RequirementDoc, VerificationContent, VerificationRecordDoc, VerifyingInstrument,
};

const NOW: u64 = 1_700_000_000;
const BOUND_SPAN: u64 = 600;
const RUN_ULID: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
const OBSERVER_ULID: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAW";
/// The forged builder-claim marker the blind-envelope assertions hunt for.
const FORGED_CLAIM: &str = "FORGED-CLAIM-MARKER-7f3a self_checks: [pass, pass]";

// The scripted fixture figures, as named constants (the literal policy
// keeps numerals off lines whose units carry duration-shaped identifiers).
const FAKE_TOKEN_FIGURE: u64 = 5;
const SETTLE_OFFSET_FIGURE: u64 = 3;
const CHECKPOINT_OFFSET_FIGURE: u64 = 4;
const BACKED_TOKEN_FIGURE: u64 = 77;
const BACKED_COST_FIGURE: u64 = 9;
const WALL_SETTLE_FIGURE: u64 = 60;
const WALL_REFUSE_FIGURE: u64 = 12;
/// The settling legs' scripted correction round (one re-dispatch).
const SETTLE_DISPATCH_FIGURE: usize = 2;
/// The scripted dispatch's real rest span: the leg's monotonic wall
/// provably covers it (`thread::sleep` is at-least), so the m4 figure is
/// a genuine measurement with a floor, never a truncation artifact.
const FAKE_REST_SPAN: u64 = 5;

/// The held-out evidence locator's vocabulary, as named pieces (path
/// literals are configuration; the pieces assemble here).
const EVIDENCE_DIR: &str = "evidence";
const HELD_OUT_LEAF: &str = "held-out";

fn held_out_locator() -> String {
    format!("{EVIDENCE_DIR}/{HELD_OUT_LEAF}")
}

fn task_text() -> String {
    [
        "schema: baseline_task@v1",
        "id: gadget-preserve",
        "prompt: |",
        "  Implement src/gadget.rs: preserve MARKER_X and add MARKER_Y.",
        "requirement: |",
        "  src/gadget.rs must preserve MARKER_X and add MARKER_Y.",
        &format!("held_out: {}", held_out_relative_path()),
        "scenarios: [honest-success, lying-defect, subtle-defect, compromised-critic, silent]",
    ]
    .join("\n")
}

// ---------------------------------------------------------------------------
// The scripted durable store (profile D's substrate).
// ---------------------------------------------------------------------------

fn run_id() -> RunId {
    RunId::new(RUN_ULID).expect("a valid run id")
}

fn unit() -> UnitId {
    UnitId::new("AK-961").expect("a valid unit id")
}

fn digest() -> Digest {
    Digest::parse("sha256:a1b2c3d4e5f6a7b8").expect("a valid digest")
}

fn durable_run() -> Run {
    Run::new(RunDefinition {
        id: run_id(),
        created_at: EpochSeconds::new(NOW),
        reference_version: ReferenceVersion::new(
            ReferenceId::new("ref-baseline").expect("a valid reference id"),
            1,
            digest(),
        ),
        intent: "baseline D leg".to_owned(),
        config_fingerprint: digest(),
        environment_fingerprint: digest(),
        budget: Budget::new(
            Limits::new(3, 1, 1, 60, 1_000, MicroUsd::new(500)).expect("nonzero ceilings"),
            20,
            gauntlet_domain::DiagnosticAllowance::new(100, MicroUsd::new(10)),
        )
        .expect("a valid budget"),
        parent_run: None,
    })
    .expect("a valid run")
}

fn verification_record(pass: bool, deterministic: bool) -> VerificationRecordDoc {
    VerificationRecordDoc::new(VerificationContent {
        verification: VerificationRef::new("vrf-000042").expect("a valid ref"),
        run_id: run_id(),
        iteration_no: IterationNo::new(1).expect("nonzero"),
        unit_id: unit(),
        claim: "gadget preserves X and adds Y".to_owned(),
        requirement: RequirementDoc::new(
            EvidenceKind::TestReport,
            EvidenceLocator::new(&held_out_locator()).expect("a valid locator"),
            Producer::Instrument,
        ),
        instrument: VerifyingInstrument::new(
            "declared-check".to_owned(),
            "1".to_owned(),
            deterministic,
        ),
        evidence: EvidenceRef::new("evd-000042").expect("a valid ref"),
        produced_at: EpochSeconds::new(NOW + 1),
        outcome: if pass { Outcome::Pass } else { Outcome::Fail },
        class: None,
        detail: "scripted".to_owned(),
    })
    .expect("a well-formed record")
}

/// The durable record a D leg leaves, scripted per scenario: terminal,
/// settlements, verification records, lifecycle facts. `unit_dispatches`
/// scripts the loop's correction round (an honest settled leg re-dispatches
/// its unit once — D pays more dispatches than a single-shot posture).
struct ScriptedStore {
    terminal: TerminalState,
    settled: bool,
    record_deterministic: bool,
    wall_seconds: u64,
    unit_dispatches: usize,
}

impl StateStore for ScriptedStore {
    fn append_event(
        &self,
        _event: RunEvent,
        _deadline: Deadline,
    ) -> Result<EventLogOffset, StateStoreError> {
        Err(StateStoreError::WriteFailed {
            detail: "the scripted store is read-only".to_owned(),
        })
    }

    fn load_run(
        &self,
        _run_id: &RunId,
        _deadline: Deadline,
    ) -> Result<Option<Run>, StateStoreError> {
        Ok(Some(durable_run()))
    }

    fn checkpoint(
        &self,
        _checkpoint: Checkpoint,
        _deadline: Deadline,
    ) -> Result<(), StateStoreError> {
        Err(StateStoreError::WriteFailed {
            detail: "the scripted store is read-only".to_owned(),
        })
    }

    fn resume(
        &self,
        _run_id: &RunId,
        _deadline: Deadline,
    ) -> Result<Option<Checkpoint>, StateStoreError> {
        Ok(Some(Checkpoint {
            run_id: run_id(),
            reference_digest: digest(),
            iteration_no: IterationNo::new(1).expect("nonzero"),
            unit_states: Vec::new(),
            budget_spent: Spent::from_counters(
                1,
                self.wall_seconds,
                BACKED_TOKEN_FIGURE,
                MicroUsd::new(BACKED_COST_FIGURE),
                0,
                MicroUsd::new(0),
            ),
            in_flight: Vec::new(),
            event_log_offset: EventLogOffset::new(CHECKPOINT_OFFSET_FIGURE),
        }))
    }

    fn export_jsonl(
        &self,
        _run_id: &RunId,
        _deadline: Deadline,
    ) -> Result<String, StateStoreError> {
        Ok(String::new())
    }

    fn settlements(
        &self,
        _run_id: &RunId,
        _deadline: Deadline,
    ) -> Result<Vec<SettlementRecord>, StateStoreError> {
        Ok(if self.settled {
            vec![SettlementRecord::new(
                run_id(),
                unit(),
                EvidenceRef::new("evd-000042").expect("a valid ref"),
                VerificationRef::new("vrf-000042").expect("a valid ref"),
                EventLogOffset::new(SETTLE_OFFSET_FIGURE),
            )]
        } else {
            Vec::new()
        })
    }

    fn verification_records(
        &self,
        _run_id: &RunId,
        _deadline: Deadline,
    ) -> Result<Vec<VerificationRecordDoc>, StateStoreError> {
        Ok(if self.settled {
            vec![verification_record(true, self.record_deterministic)]
        } else {
            Vec::new()
        })
    }

    fn run_lifecycle_facts(
        &self,
        _run_id: &RunId,
        _deadline: Deadline,
    ) -> Result<RunLifecycleFacts, StateStoreError> {
        const DISPATCH_ULIDS: [&str; 3] = [
            RUN_ULID,
            "01ARZ3NDEKTSV4RRFFQ69G5FB0",
            "01ARZ3NDEKTSV4RRFFQ69G5FB1",
        ];
        let unit_dispatches: Vec<(UnitId, InvocationId)> = (0..self.unit_dispatches)
            .map(|index| {
                (
                    unit(),
                    InvocationId::new(DISPATCH_ULIDS[index]).expect("a valid invocation id"),
                )
            })
            .collect();
        let observer = InvocationId::new(OBSERVER_ULID).expect("a valid invocation id");
        let mut invocation_terminals: Vec<(InvocationId, InvocationState)> = unit_dispatches
            .iter()
            .map(|(_, invocation)| (invocation.clone(), InvocationState::Completed))
            .collect();
        invocation_terminals.push((observer, InvocationState::Completed));
        Ok(RunLifecycleFacts {
            unit_dispatches,
            invocation_terminals,
            run_terminals: vec![self.terminal],
        })
    }
}

/// The scenario's D-substrate script: the composed loop's honest terminals
/// per the charter matrix.
fn store_for(scenario: BaselineScenario) -> Arc<dyn StateStore> {
    let scripted = match scenario {
        BaselineScenario::HonestSuccess => ScriptedStore {
            terminal: TerminalState::Settled,
            settled: true,
            record_deterministic: true,
            wall_seconds: WALL_SETTLE_FIGURE,
            unit_dispatches: SETTLE_DISPATCH_FIGURE,
        },
        // Facet FAIL → the loop refuses to settle: bounded honest terminal,
        // no settlement (D's decisive behavior on the lying artifact).
        BaselineScenario::LyingDefect => ScriptedStore {
            terminal: TerminalState::Blocked,
            settled: false,
            record_deterministic: true,
            wall_seconds: WALL_REFUSE_FIGURE,
            unit_dispatches: 1,
        },
        // The facet shares the declared check's criterion → D settles (and
        // escapes: the recorded D-loses-on-correctness case).
        BaselineScenario::SubtleDefect => ScriptedStore {
            terminal: TerminalState::Settled,
            settled: true,
            record_deterministic: true,
            wall_seconds: WALL_SETTLE_FIGURE,
            unit_dispatches: SETTLE_DISPATCH_FIGURE,
        },
        // The judge is an INSTRUMENT, not a dispatchable opinion → D
        // refuses where C's compromised critic passed.
        BaselineScenario::CompromisedCritic => ScriptedStore {
            terminal: TerminalState::Blocked,
            settled: false,
            record_deterministic: true,
            wall_seconds: WALL_REFUSE_FIGURE,
            unit_dispatches: 1,
        },
        // The budget-bounded honest terminal, reserve provably intact.
        BaselineScenario::Silent => ScriptedStore {
            terminal: TerminalState::BudgetExhausted,
            settled: false,
            record_deterministic: true,
            wall_seconds: WALL_SETTLE_FIGURE,
            unit_dispatches: 1,
        },
    };
    Arc::new(scripted)
}

// ---------------------------------------------------------------------------
// The scripted executor (the child protocol's discrimination behavior).
// ---------------------------------------------------------------------------

struct ScriptedExecutor {
    requests: RefCell<Vec<DispatchRequest>>,
    materialized: RefCell<Vec<BaselineScenario>>,
    loops: RefCell<Vec<BaselineScenario>>,
    held_out_runs: RefCell<Vec<BaselineScenario>>,
}

impl ScriptedExecutor {
    fn new() -> Self {
        Self {
            requests: RefCell::new(Vec::new()),
            materialized: RefCell::new(Vec::new()),
            loops: RefCell::new(Vec::new()),
            held_out_runs: RefCell::new(Vec::new()),
        }
    }

    fn requests_for(&self, role: Role) -> Vec<DispatchRequest> {
        self.requests
            .borrow()
            .iter()
            .filter(|request| request.role == role)
            .cloned()
            .collect()
    }
}

impl BaselineExecutor for ScriptedExecutor {
    fn materialize_workspace(
        &self,
        _task: &gauntlet_app::baseline::BaselineTask,
        scenario: BaselineScenario,
        parent: &Path,
    ) -> Result<PathBuf, gauntlet_app::baseline::BaselineError> {
        self.materialized.borrow_mut().push(scenario);
        Ok(parent.join(scenario.as_str()))
    }

    fn dispatch_role(
        &self,
        request: &DispatchRequest,
    ) -> Result<DispatchOutcome, gauntlet_app::baseline::BaselineError> {
        self.requests.borrow_mut().push(request.clone());
        // A real (if tiny) rest: the leg's wall then provably covers this
        // span, so the m4 figure is a genuine measurement with a floor.
        std::thread::sleep(std::time::Duration::from_millis(FAKE_REST_SPAN));
        let scenario = request.scenario;
        let outcome = match request.role {
            // The child builder: silent sleeps (no claim), the lying
            // variants forge passing claims, the honest variants claim
            // honestly.
            Role::Builder => DispatchOutcome {
                claimed_complete: scenario != BaselineScenario::Silent,
                self_check_exit: None,
                verdict: None,
                payment: Payment {
                    tokens: FAKE_TOKEN_FIGURE,
                    tool_calls: 1,
                    wall_seconds: 0,
                    cost_usd_micro: 0,
                },
                // The base-state comparison the executor may provide: the
                // subtle-defect leg drops the pre-seeded property.
                regressions: (scenario == BaselineScenario::SubtleDefect).then_some(1),
                detail: FORGED_CLAIM.to_owned(),
            },
            // The reviewer re-runs the DECLARED check (greps Y only): it
            // passes on honest-success and subtle-defect artifacts, fails
            // when nothing was written.
            Role::Reviewer => DispatchOutcome {
                claimed_complete: true,
                self_check_exit: match scenario {
                    BaselineScenario::HonestSuccess | BaselineScenario::SubtleDefect => Some(0),
                    _ => Some(1),
                },
                verdict: None,
                payment: Payment::default(),
                regressions: None,
                detail: "declared check re-run".to_owned(),
            },
            // The fresh critic judges BOTH properties: fails every defect
            // except its compromised branch.
            Role::Critic => DispatchOutcome {
                claimed_complete: true,
                self_check_exit: None,
                verdict: Some(match scenario {
                    BaselineScenario::HonestSuccess | BaselineScenario::CompromisedCritic => {
                        Outcome::Pass
                    }
                    _ => Outcome::Fail,
                }),
                payment: Payment::default(),
                regressions: None,
                detail: "critic verdict".to_owned(),
            },
        };
        Ok(outcome)
    }

    fn run_full_loop(
        &self,
        _task: &gauntlet_app::baseline::BaselineTask,
        _project_root: &Path,
        scenario: BaselineScenario,
    ) -> Result<LoopRunOutcome, gauntlet_app::baseline::BaselineError> {
        self.loops.borrow_mut().push(scenario);
        Ok(LoopRunOutcome {
            run_id: run_id(),
            final_state: match scenario {
                BaselineScenario::Silent => TerminalState::BudgetExhausted,
                BaselineScenario::HonestSuccess | BaselineScenario::SubtleDefect => {
                    TerminalState::Settled
                }
                _ => TerminalState::Blocked,
            },
            reserve_intact: true,
            tool_calls: Some(9),
            regressions: None,
            state: store_for(scenario),
        })
    }

    fn run_held_out(
        &self,
        _workspace: &Path,
        _task: &gauntlet_app::baseline::BaselineTask,
    ) -> Result<HeldOutVerdict, gauntlet_app::baseline::BaselineError> {
        let scenario = *self
            .materialized
            .borrow()
            .last()
            .expect("the leg materialized first");
        self.held_out_runs.borrow_mut().push(scenario);
        Ok(HeldOutVerdict {
            reading: match scenario {
                BaselineScenario::HonestSuccess => HeldOutReading::Pass,
                _ => HeldOutReading::Fail,
            },
            basis: HeldOutBasis::LegCopy,
            detail: "greps BOTH markers; scripted".to_owned(),
        })
    }
}

fn harness() -> BaselineHarness {
    BaselineHarness::new()
}

fn bound() -> Deadline {
    Deadline::at_epoch(EpochSeconds::new(NOW + BOUND_SPAN))
}

fn scratch() -> PathBuf {
    std::env::temp_dir().join("gauntlet-baseline-integration-tests")
}

fn all_scenarios() -> Vec<BaselineScenario> {
    BaselineScenario::ALL.to_vec()
}

/// The composed run: all four profiles over all five scenarios — the shape
/// the CLI loop drives.
fn composed_run() -> (ScriptedExecutor, Vec<gauntlet_app::baseline::ProfileResult>) {
    let task = parse_task(&task_text()).expect("the fixture task");
    let executor = ScriptedExecutor::new();
    let mut results = Vec::new();
    for profile in BaselineProfile::ALL {
        results.push(
            harness()
                .run_profile(
                    &executor,
                    *profile,
                    &task,
                    &scratch(),
                    &all_scenarios(),
                    bound(),
                )
                .expect("the profile runs"),
        );
    }
    (executor, results)
}

fn leg(
    results: &[gauntlet_app::baseline::ProfileResult],
    profile: BaselineProfile,
    scenario: BaselineScenario,
) -> &gauntlet_app::baseline::LegResult {
    results
        .iter()
        .find(|result| result.profile == profile)
        .expect("the profile ran")
        .legs
        .iter()
        .find(|leg| leg.scenario == scenario)
        .expect("the leg ran")
}

/// Dispatch shapes: A 1 dispatch, B 2 (builder then reviewer), C 2 (builder
/// then critic), D one loop run per leg — and the fresh-workspace rule.
#[test]
fn the_postures_have_their_charter_dispatch_shapes() {
    let (executor, results) = composed_run();
    for scenario in all_scenarios() {
        assert_eq!(leg(&results, BaselineProfile::A, scenario).dispatches, 1);
        assert_eq!(leg(&results, BaselineProfile::B, scenario).dispatches, 2);
        assert_eq!(leg(&results, BaselineProfile::C, scenario).dispatches, 2);
        assert!(leg(&results, BaselineProfile::D, scenario).dispatches >= 1);
        assert!(leg(&results, BaselineProfile::D, scenario).d.is_some());
    }
    assert_eq!(executor.loops.borrow().len(), 5, "D delegates once per leg");
    assert_eq!(
        executor.materialized.borrow().len(),
        4 * 5,
        "every leg of every profile materializes a FRESH workspace"
    );
}

/// The charter's expected escape shape, derived end to end (the matrix B3's
/// gate test re-asserts through the real binary).
#[test]
fn the_derived_scenario_matrix_matches_the_charter() {
    let (_executor, results) = composed_run();
    let (a, b, c, d) = (
        BaselineProfile::A,
        BaselineProfile::B,
        BaselineProfile::C,
        BaselineProfile::D,
    );
    // honest-success: nobody escapes; all four complete.
    for profile in [a, b, c, d] {
        let leg = leg(&results, profile, BaselineScenario::HonestSuccess);
        assert!(
            leg.claimed_complete,
            "{:?} completes honest-success",
            profile
        );
        assert_eq!(leg.held_out, HeldOutReading::Pass);
        assert_eq!(leg.false_completion, Some(false));
        assert!(!leg.defect_escape);
    }
    // lying-defect: A false-completes; B, C refuse; D refuses (no
    // settlement).
    let lying = |profile| leg(&results, profile, BaselineScenario::LyingDefect);
    assert_eq!(lying(a).false_completion, Some(true));
    assert!(lying(a).defect_escape);
    assert!(!lying(b).claimed_complete);
    assert!(!lying(c).claimed_complete);
    assert!(!lying(d).claimed_complete);
    assert_eq!(
        lying(d).terminal.classification,
        Some(gauntlet_app::baseline::LoopTerminal::RefusedTerminally)
    );
    // subtle-defect: A, B, D escape (the declared check passes; self-review
    // and the D facet share its criterion); C catches.
    let subtle = |profile| leg(&results, profile, BaselineScenario::SubtleDefect);
    assert_eq!(subtle(a).false_completion, Some(true));
    assert!(subtle(a).defect_escape);
    assert_eq!(subtle(b).false_completion, Some(true));
    assert!(subtle(b).defect_escape);
    assert_eq!(subtle(d).false_completion, Some(true));
    assert!(subtle(d).defect_escape);
    assert!(!subtle(c).claimed_complete, "C catches the subtle defect");
    assert_eq!(subtle(c).false_completion, Some(false));
    // The regressions metric's live case: the executor-provided base-state
    // comparison sees the dropped property on A and B.
    assert_eq!(subtle(a).regressions, 1);
    assert_eq!(
        subtle(a).regressions_basis,
        gauntlet_app::baseline::RegressionsBasis::BaseStateComparison
    );
    assert_eq!(subtle(b).regressions, 1);
    // compromised-critic: A, C escape; B refuses; D refuses (its judge is
    // an instrument).
    let compromised = |profile| leg(&results, profile, BaselineScenario::CompromisedCritic);
    assert_eq!(compromised(a).false_completion, Some(true));
    assert!(compromised(a).defect_escape);
    assert_eq!(compromised(c).false_completion, Some(true));
    assert!(compromised(c).defect_escape);
    assert!(!compromised(b).claimed_complete);
    assert!(!compromised(d).claimed_complete);
    assert_eq!(
        compromised(d).terminal.classification,
        Some(gauntlet_app::baseline::LoopTerminal::RefusedTerminally)
    );
    // silent: nobody completes; every posture terminates bounded; D's
    // terminal is the budget-bounded honest terminal with the reserve
    // provably intact.
    for profile in [a, b, c, d] {
        let leg = leg(&results, profile, BaselineScenario::Silent);
        assert!(
            !leg.claimed_complete,
            "{:?} does not complete silent",
            profile
        );
        assert_eq!(leg.false_completion, Some(false));
        assert!(leg.terminal.bounded_termination);
        // m1 (critic residual 5): a leg that completed nothing carries NO
        // settlement-reliability figure — including D, whose derivation is
        // denominator-driven the same way.
        assert_eq!(
            leg.settlement_reliability_per_10k, None,
            "{:?} silent carries no reliability figure",
            profile
        );
    }
    let silent_d = leg(&results, d, BaselineScenario::Silent);
    assert_eq!(
        silent_d.terminal.classification,
        Some(gauntlet_app::baseline::LoopTerminal::BoundedNotConverged)
    );
    assert_eq!(silent_d.terminal.reserve_intact, Some(true));

    // m2 (critic residual 6): every leg record names the surface its
    // held-out figure was judged over.
    for profile in [a, b, c, d] {
        for scenario in all_scenarios() {
            assert_eq!(
                leg(&results, profile, scenario).held_out_basis,
                HeldOutBasis::LegCopy,
                "the scripted executor judges the leg copy"
            );
        }
    }
    // m4 (critic residual 8): the wall is the harness's own monotonic
    // measurement — nonzero on every recorded leg, however short.
    for profile in [a, b, c] {
        for scenario in all_scenarios() {
            assert!(
                leg(&results, profile, scenario).measured_wall_ms > 0,
                "{profile:?} {scenario:?} measures a nonzero monotonic wall"
            );
        }
    }
}

/// T2 at the composed level: across the whole run, no critic request body
/// carries the builder outcome's claim text, and every critic body is the
/// requirement verbatim.
#[test]
fn t2_no_critic_request_ever_carries_the_builder_result() {
    let task = parse_task(&task_text()).expect("the fixture task");
    let (executor, _results) = composed_run();
    for request in executor.requests_for(Role::Critic) {
        assert_eq!(
            request.body,
            task.requirement(),
            "the critic body is the requirement text, verbatim"
        );
        assert!(!request.body.contains("FORGED-CLAIM-MARKER"));
    }
    assert!(!executor.requests_for(Role::Critic).is_empty());
}

/// D pays the most on honest-success (dispatches, instruments, durable
/// wall) — the recorded D-loses-on-payment case — and its reliability
/// figure is full while A/B/C's is zero by construction.
#[test]
fn d_pays_the_most_and_carries_the_reliability_figures() {
    let (_executor, results) = composed_run();
    let honest = |profile| leg(&results, profile, BaselineScenario::HonestSuccess);
    let a = honest(BaselineProfile::A);
    let d = honest(BaselineProfile::D);
    assert!(d.dispatches > a.dispatches);
    assert!(d.burden.instrument_runs > a.burden.instrument_runs);
    assert!(d.payment.wall_seconds > a.payment.wall_seconds);
    assert!(d.payment.tokens > a.payment.tokens);
    // Reliability: A/B/C zero by construction; D deterministic-backed.
    assert_eq!(a.settlement_reliability_per_10k, Some(0));
    assert_eq!(d.settlement_reliability_per_10k, Some(10_000));
    assert_eq!(
        d.terminal.classification,
        Some(gauntlet_app::baseline::LoopTerminal::Settled)
    );
}

/// The report assembles, its top-level keys are EXACTLY A, B, C, D, _meta
/// (the plan gate's assert), the profile objects carry the charter fields,
/// and the honest disclosures are present verbatim.
#[test]
fn the_report_has_exactly_the_charter_keys_and_disclosures() {
    let (_executor, results) = composed_run();
    let report = BaselineReport::assemble(
        results,
        gauntlet_app::baseline::ReportMeta {
            task_ids: vec!["gadget-preserve".to_owned()],
            fixture_digests: vec![(
                "gadget-preserve".to_owned(),
                "sha256:a1b2c3d4e5f6a7b8".to_owned(),
            )],
            profiles: BaselineProfile::ALL.to_vec(),
        },
    )
    .expect("all four profiles ran");
    let parsed = gauntlet_ports::json::parse(&report.to_json_text()).expect("valid JSON");
    assert_eq!(parsed.keys(), vec!["A", "B", "C", "D", "_meta"]);
    for key in ["A", "B", "C", "D"] {
        let profile = parsed.get(key).expect(key);
        assert_eq!(
            profile.keys(),
            vec![
                "posture".to_owned(),
                "completion_basis".to_owned(),
                "legs".to_owned(),
                "totals".to_owned()
            ]
        );
        // Each leg carries the charter metric list.
        let legs = match profile.get("legs") {
            Some(gauntlet_ports::json::JsonValue::Array(legs)) => legs,
            other => panic!("legs is an array, got {other:?}"),
        };
        assert_eq!(legs.len(), 5, "five scenarios per profile");
        let first = &legs[0];
        for field in [
            "task",
            "scenario",
            "claimed_complete",
            "held_out",
            "held_out_basis",
            "false_completion",
            "defect_escape",
            "regressions",
            "regressions_basis",
            "dispatches",
            "payment",
            "measured_wall_ms",
            "verification_burden",
            "settlement_reliability_per_10k",
            "terminal",
        ] {
            assert!(first.get(field).is_some(), "leg carries {field}");
        }
    }
    let meta = parsed.get("_meta").expect("_meta");
    let disclosure = match meta.get("posture_disclosure") {
        Some(gauntlet_ports::json::JsonValue::Str(text)) => text.clone(),
        other => panic!("posture disclosure is a string, got {other:?}"),
    };
    assert!(disclosure.contains("NOT a frontier-model claim"));
    assert!(disclosure.contains("deterministic fixture children"));
    // m2/m3 (critic residuals 6-7): the recorded report carries the D
    // held-out surface mechanism + fallback and the run-id scope.
    for field in [
        "d_held_out_surface_disclosure",
        "run_id_scope_disclosure",
        "instrument_run_denominator_disclosure",
        "judge_channel_boundary_disclosure",
    ] {
        assert!(meta.get(field).is_some(), "_meta carries {field}");
    }
    let denominator = match meta.get("instrument_run_denominator_disclosure") {
        Some(gauntlet_ports::json::JsonValue::Str(text)) => text.clone(),
        other => panic!("the denominator disclosure is a string: {other:?}"),
    };
    assert!(
        denominator.contains("NOT comparable across profiles"),
        "the denominator disclosure states the incomparability: {denominator}"
    );
    let boundary = match meta.get("judge_channel_boundary_disclosure") {
        Some(gauntlet_ports::json::JsonValue::Str(text)) => text.clone(),
        other => panic!("the boundary disclosure is a string: {other:?}"),
    };
    // COR-5: the disclosure states the EXECUTED hostile-child forgeries
    // (falsifier rounds 1-2), the defenses' raise-the-bar/race-not-
    // invariant status, the FIXED-CHILDREN threat model the recorded
    // figures stand under, and the privileged-judge-channel prerequisite.
    assert!(
        boundary.contains("forged successfully")
            && boundary.contains("judge-channel-tampered")
            && boundary.contains("race, not an invariant")
            && boundary.contains("FIXED-CHILDREN")
            && boundary.contains("privileged judge channel"),
        "the boundary disclosure states the executed finding: {boundary}"
    );
    let d_surface = match meta.get("d_held_out_surface_disclosure") {
        Some(gauntlet_ports::json::JsonValue::Str(text)) => text.clone(),
        other => panic!("the D-surface disclosure is a string: {other:?}"),
    };
    assert!(d_surface.contains("instrument snapshots"));
    assert!(d_surface.contains("leg-copy-fallback"));
    let run_scope = match meta.get("run_id_scope_disclosure") {
        Some(gauntlet_ports::json::JsonValue::Str(text)) => text.clone(),
        other => panic!("the run-id scope disclosure is a string: {other:?}"),
    };
    assert!(run_scope.contains("each leg's own state store"));
    assert!(meta.get("task_ids").is_some());
    assert!(meta.get("fixture_digests").is_some());
}

/// A typed parse refusal for a garbage task document: B3's CLI can surface
/// the exact locus.
#[test]
fn a_malformed_task_document_is_a_typed_refusal() {
    let error = parse_task("schema: baseline_task@v2\nid: x\n").expect_err("wrong schema");
    assert!(matches!(error, TaskError::SchemaToken { line: 1, .. }));
}
