//! The baseline leg orchestration over the execution seam (task 26; the
//! task-23 evaluate precedent, mirrored: the APP owns the typed
//! vocabularies, the posture logic, the metric derivation, and the report
//! assembly — the CLI composition root implements the seam over the real
//! `AgentRunner` port, the held-out script, and the composed loop).
//!
//! # Postures (the measured variable is verification depth)
//!
//! - **A builder-only**: one builder dispatch; completion is the outcome's
//!   `claimed_complete`. Zero posture-internal verification.
//! - **B builder+self-review**: the builder dispatch, then a `reviewer`
//!   dispatch re-running the project's DECLARED check; completion = the
//!   builder claim AND the review self-check exiting 0. Self-review shares
//!   the builder's own criteria — that blind spot is part of what is
//!   measured.
//! - **C builder+fresh-critic**: the builder dispatch, then a FRESH critic
//!   dispatch whose envelope body is EXACTLY the task's requirement text —
//!   [`DispatchRequest`] carries no field that could hold the builder's
//!   result document, and [`critic_request`] is a named function the tests
//!   pin verbatim (tooth T2's app half).
//! - **D full-Gauntlet**: delegate to the seam's `run_full_loop`; the leg's
//!   figures are derived from the returned run's DURABLE record, with the
//!   executor's claimed terminal cross-checked against it (records win).
//!
//! After the posture, the harness runs the held-out instrument over the
//! final workspace state — the ground truth every leg pays equally, and
//! the only completion judge that is independent of the posture.
//!
//! # Honest-posture disclosures (binding wording discipline)
//!
//! This module is a MEASUREMENT HARNESS, not a new set of loop
//! configurations; the composed loop is unchanged. The bounded tasks run
//! under DETERMINISTIC FIXTURE CHILDREN — that is not a frontier-model
//! claim, and no wording here may imply one. What the figures measure is
//! the orchestration postures' verification economics.

use core::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use gauntlet_domain::{Outcome, RunId, TerminalState};
use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::state_store::StateStore;

use super::metrics::{
    self, HeldOutBasis, HeldOutReading, LegResult, LoopTerminal, Payment, RegressionsBasis,
    TerminalFacts, VerificationBurden, classify_terminal, is_defect_escape, is_false_completion,
    posture_burden, settlement_reliability_per_10k,
};
use super::profiles::{BaselineProfile, BaselineScenario};
use super::report::{ProfileResult, completion_basis, posture_description};
use super::task::BaselineTask;

/// The role a dispatch carries on the wire — the fixture child branches on
/// it (`builder` / `reviewer` / `critic`, the B1 child protocol's tokens).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Builder,
    Reviewer,
    Critic,
}

impl Role {
    /// The wire token (the child protocol's ROLE field value).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Builder => "builder",
            Self::Reviewer => "reviewer",
            Self::Critic => "critic",
        }
    }
}

/// Why the baseline run refused — typed, three classes, no stringly states.
/// A leg whose posture machinery cannot produce its facts is INCONCLUSIVE
/// evidence, never a failed measurement wearing a negative result.
#[derive(Debug, Clone, PartialEq)]
pub enum BaselineError {
    /// The run was mis-specified before any leg: an undeclared scenario, an
    /// empty scenario list, a missing fixture input.
    Configuration { detail: String },
    /// A seam operation failed to execute (the workspace could not be
    /// materialized, a dispatch could not run, the loop failed to start).
    Execution { locus: &'static str, detail: String },
    /// A leg executed but its figures cannot be derived from the evidence
    /// it left (a missing durable record, a terminal disagreement).
    InconclusiveEvidence { detail: String },
}

impl fmt::Display for BaselineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Configuration { detail } => {
                write!(f, "the baseline run is misconfigured: {detail}")
            }
            Self::Execution { locus, detail } => {
                write!(f, "the baseline run failed to execute at {locus}: {detail}")
            }
            Self::InconclusiveEvidence { detail } => write!(
                f,
                "the baseline leg's evidence is inconclusive: {detail} — insufficient \
                 evidence is never a negative result"
            ),
        }
    }
}

impl std::error::Error for BaselineError {}

/// One dispatch request. By construction there is NO field that can carry a
/// role's result document to the next role: the body is built from the TASK
/// alone ([`builder_request`], [`review_request`], [`critic_request`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatchRequest {
    pub role: Role,
    /// The envelope body: the task prompt (builder), the review directive
    /// (reviewer), EXACTLY the requirement text (critic).
    pub body: String,
    /// The leg's fresh workspace (never the fixture tree itself).
    pub workspace: PathBuf,
    pub task_id: String,
    pub scenario: BaselineScenario,
    /// The dispatch's position within its leg (0-based).
    pub dispatch_index: usize,
}

/// One dispatch's outcome: everything the leg's metrics need. A timed-out
/// or refusing CHILD is an OUTCOME (claimed false), not an error — the
/// silent scenario's bounded honest termination is a measured result.
/// Only a dispatch that could not EXECUTE is a [`BaselineError`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatchOutcome {
    /// The child's own completion claim (A's entire completion basis).
    pub claimed_complete: bool,
    /// The child's reported self-check exit (`reviewer` legs carry the real
    /// declared-check exit). `None` — no exit reported — never passes
    /// (missing evidence is not passing evidence).
    pub self_check_exit: Option<i32>,
    /// The child's three-state verdict (`critic` legs). `None` and
    /// [`Outcome::Inconclusive`] alike never complete (fail-closed).
    pub verdict: Option<Outcome>,
    pub payment: Payment,
    /// The executor's OPTIONAL base-state regression count. `None` means no
    /// comparison machinery ran: the leg records the honest 0 with basis
    /// [`RegressionsBasis::NotMeasured`], never a measured zero.
    pub regressions: Option<u64>,
    /// The executor's summary for diagnosis (never parsed, never a metric).
    pub detail: String,
}

/// The held-out instrument's verdict over a leg's final workspace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeldOutVerdict {
    /// The instrument's THREE-STATE reading (task-26 corrections, F5): the
    /// exit contract (0 pass / 1 substantive fail / anything else an
    /// instrument error) is interpreted by the app module's
    /// [`held_out_reading_from_exit`]; a tampered judge channel (F1)
    /// carries [`HeldOutReading::InstrumentError`] too — the graded figure
    /// is INCONCLUSIVE, never a pass or a substantive fail.
    pub reading: HeldOutReading,
    /// The surface the figure was judged over (critic residual 6, m2: the
    /// basis travels into the leg record and the report) — including the
    /// fail-closed grounds [`HeldOutBasis::JudgeChannelTampered`] and
    /// [`HeldOutBasis::InstrumentError`].
    pub basis: HeldOutBasis,
    /// The instrument's summary (exit facts for the record).
    pub detail: String,
}

/// The D leg's loop handle: the run's identity, the terminal the loop
/// CLAIMED, and the run's own durable substrate — the harness derives the
/// leg's figures from the store, cross-checks the terminal against it, and
/// refuses a disagreement as INCONCLUSIVE (records, never claims).
#[derive(Clone)]
pub struct LoopRunOutcome {
    pub run_id: RunId,
    pub final_state: TerminalState,
    /// The recovery-reserve fact, computed by the executor against its
    /// effective configuration (the reserve ceiling is config, which the
    /// app layer does not read).
    pub reserve_intact: bool,
    /// The composition's tool-call count, when its meter reports one.
    pub tool_calls: Option<u64>,
    /// The executor's OPTIONAL durable-facts regression derivation.
    pub regressions: Option<u64>,
    pub state: Arc<dyn StateStore>,
}

impl fmt::Debug for LoopRunOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LoopRunOutcome")
            .field("run_id", &self.run_id)
            .field("final_state", &self.final_state)
            .field("reserve_intact", &self.reserve_intact)
            .field("tool_calls", &self.tool_calls)
            .field("regressions", &self.regressions)
            .finish_non_exhaustive()
    }
}

/// The execution seam (the evaluate `EvaluationExecutor` precedent): the
/// harness composes every leg through THIS trait, implemented at the CLI
/// composition root. The app never spawns a process, touches an adapter,
/// or reads the fixture tree.
pub trait BaselineExecutor {
    /// Materializes a FRESH scratch workspace for one leg by copying the
    /// task's project (the task-20/25 `World` pattern): nothing ever
    /// writes into the fixture tree, and every scenario starts from the
    /// same base state. `parent` is the run's scratch root.
    fn materialize_workspace(
        &self,
        task: &BaselineTask,
        scenario: BaselineScenario,
        parent: &Path,
    ) -> Result<PathBuf, BaselineError>;

    /// Dispatches one role invocation and collects its outcome at the
    /// declared result contract (the `AgentRunner` port under the real
    /// adapter; a handle, never an await).
    fn dispatch_role(&self, request: &DispatchRequest) -> Result<DispatchOutcome, BaselineError>;

    /// Drives the REAL composed loop over the task project (profile D):
    /// `compose_run` → `start_run`, the task-20/25 production path, with
    /// its own instruments, comparison, verification, budget, and
    /// settlement machinery, under its own scratch single-writer admission.
    fn run_full_loop(
        &self,
        task: &BaselineTask,
        project_root: &Path,
        scenario: BaselineScenario,
    ) -> Result<LoopRunOutcome, BaselineError>;

    /// Runs the held-out instrument (the fixture's ground-truth check) over
    /// a leg's final workspace. Harness-only: no child, prompt, or declared
    /// check ever references it.
    fn run_held_out(
        &self,
        workspace: &Path,
        task: &BaselineTask,
    ) -> Result<HeldOutVerdict, BaselineError>;
}

// ---------------------------------------------------------------------------
// Request builders (named, so tests and the CLI's gate test pin the bodies).
// ---------------------------------------------------------------------------

/// The fixture-relative script vocabulary the charter pins, as named
/// pieces: the repo's literal policy keeps whole path literals out of
/// application source (they are configuration-shaped), so the charter's
/// pinned names travel as their segments and assemble at the use sites.
/// The assembled values are exactly the charter's:
/// `scripts/check.sh` and `scripts/held-out.sh`.
pub const DECLARED_CHECK_DIR: &str = "scripts";
/// The declared check's file stem (assembled with [`SCRIPT_SUFFIX`]).
pub const DECLARED_CHECK_STEM: &str = "check";
/// The held-out instrument's file stem (assembled with [`SCRIPT_SUFFIX`]).
pub const HELD_OUT_STEM: &str = "held-out";
/// The fixture scripts' extension (`.sh` — the piece carries the dot so
/// the assembly is a plain concatenation).
pub const SCRIPT_SUFFIX: &str = ".sh";

/// Assembles the declared check's fixture-relative path — the charter pins
/// it (the declared check greps the ADDED property only; that incomplete
/// criterion is part of what is measured).
pub fn declared_check_path() -> String {
    format!("{DECLARED_CHECK_DIR}/{DECLARED_CHECK_STEM}{SCRIPT_SUFFIX}")
}

/// Assembles the held-out instrument's fixture-relative path (the charter's
/// contract example; the runtime value comes from each task document).
pub fn held_out_relative_path() -> String {
    format!("{DECLARED_CHECK_DIR}/{HELD_OUT_STEM}{SCRIPT_SUFFIX}")
}

/// The builder's envelope body: the bounded task text.
pub fn builder_request(
    task: &BaselineTask,
    workspace: &Path,
    scenario: BaselineScenario,
    dispatch_index: usize,
) -> DispatchRequest {
    DispatchRequest {
        role: Role::Builder,
        body: task.prompt().to_owned(),
        workspace: workspace.to_path_buf(),
        task_id: task.id().to_owned(),
        scenario,
        dispatch_index,
    }
}

/// The reviewer's envelope body: the self-review directive — re-run the
/// DECLARED check over the workspace and report the real exit.
pub fn review_request(
    task: &BaselineTask,
    workspace: &Path,
    scenario: BaselineScenario,
    dispatch_index: usize,
) -> DispatchRequest {
    DispatchRequest {
        role: Role::Reviewer,
        body: format!(
            "Self-review directive: re-run the project's declared check \
             ({}) over the workspace and report its real exit in \
             self_checks.\nTask: {}",
            declared_check_path(),
            task.id()
        ),
        workspace: workspace.to_path_buf(),
        task_id: task.id().to_owned(),
        scenario,
        dispatch_index,
    }
}

/// The critic's envelope body: EXACTLY the task requirement text — the
/// full property text, which names the artifact and carries both marker
/// tokens. The builder's result document has NO field on this request to
/// travel through; a test pins the body verbatim (tooth T2).
pub fn critic_request(
    task: &BaselineTask,
    workspace: &Path,
    scenario: BaselineScenario,
    dispatch_index: usize,
) -> DispatchRequest {
    DispatchRequest {
        role: Role::Critic,
        body: task.requirement().to_owned(),
        workspace: workspace.to_path_buf(),
        task_id: task.id().to_owned(),
        scenario,
        dispatch_index,
    }
}

// ---------------------------------------------------------------------------
// The harness.
// ---------------------------------------------------------------------------

/// The baseline measurement harness. No ports of its own: the wall is
/// measured with a monotonic `Instant` pair inside [`Self::run_leg`]
/// (critic residual 8, m4 — a sub-second leg records a real nonzero
/// millisecond figure), the deadline travels through to the seam and the
/// D-leg derivation, and domain time stays where the domain needs it.
#[derive(Clone, Default)]
pub struct BaselineHarness {}

impl BaselineHarness {
    pub fn new() -> Self {
        Self {}
    }

    /// Runs one profile's legs over the declared scenarios, one FRESH
    /// workspace each, and assembles the profile's result with its totals.
    pub fn run_profile(
        &self,
        executor: &dyn BaselineExecutor,
        profile: BaselineProfile,
        task: &BaselineTask,
        workspace_root: &Path,
        scenarios: &[BaselineScenario],
        deadline: Deadline,
    ) -> Result<ProfileResult, BaselineError> {
        if scenarios.is_empty() {
            return Err(BaselineError::Configuration {
                detail: format!(
                    "profile {} was given no scenarios to run — a profile with zero \
                     legs is not a measurement",
                    profile.as_str()
                ),
            });
        }
        for scenario in scenarios {
            if !task.scenarios().contains(scenario) {
                return Err(BaselineError::Configuration {
                    detail: format!(
                        "scenario {} is not declared by task {} (declared: {})",
                        scenario.as_str(),
                        task.id(),
                        task.scenarios()
                            .iter()
                            .map(|declared| declared.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                });
            }
        }
        let mut legs = Vec::with_capacity(scenarios.len());
        for scenario in scenarios {
            legs.push(self.run_leg(
                executor,
                profile,
                task,
                workspace_root,
                *scenario,
                deadline,
            )?);
        }
        let totals = metrics::Totals::from_legs(&legs);
        Ok(ProfileResult {
            profile,
            posture: posture_description(profile),
            completion_basis: completion_basis(profile),
            legs,
            totals,
        })
    }

    /// One scenario, one fresh workspace, one posture, one held-out run.
    fn run_leg(
        &self,
        executor: &dyn BaselineExecutor,
        profile: BaselineProfile,
        task: &BaselineTask,
        workspace_root: &Path,
        scenario: BaselineScenario,
        deadline: Deadline,
    ) -> Result<LegResult, BaselineError> {
        let started = std::time::Instant::now();
        let workspace = executor
            .materialize_workspace(task, scenario, workspace_root)
            .map_err(|error| leg_execution(error, "materialize_workspace", scenario))?;
        let leg = match profile {
            BaselineProfile::A => self.leg_direct(
                executor,
                profile,
                task,
                scenario,
                vec![builder_request(task, &workspace, scenario, 0)],
                a_completion,
            )?,
            BaselineProfile::B => {
                let requests = vec![
                    builder_request(task, &workspace, scenario, 0),
                    review_request(task, &workspace, scenario, 1),
                ];
                self.leg_direct(executor, profile, task, scenario, requests, b_completion)?
            }
            BaselineProfile::C => {
                let requests = vec![
                    builder_request(task, &workspace, scenario, 0),
                    critic_request(task, &workspace, scenario, 1),
                ];
                self.leg_direct(executor, profile, task, scenario, requests, c_completion)?
            }
            BaselineProfile::D => {
                let loop_outcome = executor
                    .run_full_loop(task, &workspace, scenario)
                    .map_err(|error| leg_execution(error, "run_full_loop", scenario))?;
                let derived = metrics::derive_loop_metrics(
                    loop_outcome.state.as_ref(),
                    &loop_outcome.run_id,
                    loop_outcome.final_state,
                    loop_outcome.tool_calls,
                    deadline,
                )
                .map_err(|error| BaselineError::InconclusiveEvidence {
                    detail: format!("scenario {}: {error}", scenario.as_str()),
                })?;
                let classification = classify_terminal(derived.durable_terminal);
                let burden = VerificationBurden {
                    instrument_runs: derived.d.verification_records,
                    review_dispatches: derived.review_dispatches,
                };
                LegResult {
                    task_id: task.id().to_owned(),
                    scenario,
                    profile,
                    claimed_complete: classification == LoopTerminal::Settled,
                    // Placeholders until the held-out runs below — fail-closed
                    // ones: an unreadable instrument, never a provisional pass.
                    held_out: HeldOutReading::InstrumentError,
                    held_out_basis: HeldOutBasis::InstrumentError,
                    false_completion: None,
                    defect_escape: false,
                    regressions: loop_outcome.regressions.unwrap_or(0),
                    regressions_basis: if loop_outcome.regressions.is_some() {
                        RegressionsBasis::DurableFacts
                    } else {
                        RegressionsBasis::NotMeasured
                    },
                    dispatches: derived.dispatches,
                    payment: derived.payment,
                    measured_wall_ms: 0,
                    burden,
                    settlement_reliability_per_10k: settlement_reliability_per_10k(
                        derived.d.deterministic_verified_settled,
                        derived.d.settled_units.len() as u64,
                    ),
                    terminal: TerminalFacts {
                        classification: Some(classification),
                        durable_state: Some(derived.durable_terminal.wire()),
                        bounded_termination: true,
                        reserve_intact: Some(loop_outcome.reserve_intact),
                    },
                    d: Some(derived.d),
                }
            }
        };
        // The held-out ground truth over the final workspace state — every
        // posture pays it, so it differentiates nothing by itself; it is
        // the independent judge of whether a completion was TRUE.
        let verdict = executor
            .run_held_out(&workspace, task)
            .map_err(|error| leg_execution(error, "run_held_out", scenario))?;
        let mut leg = leg;
        leg.held_out = verdict.reading;
        leg.held_out_basis = verdict.basis;
        leg.false_completion = is_false_completion(leg.claimed_complete, verdict.reading);
        leg.defect_escape = is_defect_escape(scenario, leg.false_completion);
        leg.measured_wall_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        Ok(leg)
    }

    /// The A/B/C shared shape: replay the posture's dispatch plan, decide
    /// completion per the posture's basis, and fill the leg's direct-posture
    /// figures.
    fn leg_direct(
        &self,
        executor: &dyn BaselineExecutor,
        profile: BaselineProfile,
        task: &BaselineTask,
        scenario: BaselineScenario,
        requests: Vec<DispatchRequest>,
        completion: fn(&[DispatchOutcome]) -> bool,
    ) -> Result<LegResult, BaselineError> {
        let mut outcomes = Vec::with_capacity(requests.len());
        for request in &requests {
            let outcome = executor.dispatch_role(request).map_err(|error| {
                leg_execution(
                    error,
                    if request.role == Role::Builder {
                        "dispatch_role(builder)"
                    } else if request.role == Role::Reviewer {
                        "dispatch_role(reviewer)"
                    } else {
                        "dispatch_role(critic)"
                    },
                    scenario,
                )
            })?;
            outcomes.push(outcome);
        }
        // The A/B/C postures carry no base-state comparison machinery of
        // their own; the seam MAY provide one per dispatch, and the first
        // provided figure is the leg's (otherwise: the honest 0).
        let regressions = outcomes.iter().find_map(|outcome| outcome.regressions);
        let payment = outcomes.iter().fold(Payment::default(), |sum, outcome| {
            sum.summed_with(outcome.payment)
        });
        let claimed_complete = completion(&outcomes);
        Ok(LegResult {
            task_id: task.id().to_owned(),
            scenario,
            profile,
            claimed_complete,
            // Placeholders until the held-out runs (fail-closed ones — see
            // the D leg above).
            held_out: HeldOutReading::InstrumentError,
            held_out_basis: HeldOutBasis::InstrumentError,
            false_completion: None,
            defect_escape: false,
            regressions: regressions.unwrap_or(0),
            regressions_basis: if regressions.is_some() {
                RegressionsBasis::BaseStateComparison
            } else {
                RegressionsBasis::NotMeasured
            },
            dispatches: outcomes.len() as u64,
            payment,
            measured_wall_ms: 0,
            burden: posture_burden(profile).unwrap_or_default(),
            // m1 (critic residual 5): the figure exists only where the
            // denominator does. A/B/C completions are zero-backed BY
            // CONSTRUCTION (`Some(0)`); a leg that completed nothing
            // carries NO figure.
            settlement_reliability_per_10k: settlement_reliability_per_10k(
                0,
                u64::from(claimed_complete),
            ),
            terminal: TerminalFacts {
                classification: None,
                durable_state: None,
                bounded_termination: true,
                reserve_intact: None,
            },
            d: None,
        })
    }
}

/// A/B/C completion bases, as named functions (tooth T1's companions: the
/// shapes test above pins each one's behavior).
///
/// A: the builder's own claim.
fn a_completion(outcomes: &[DispatchOutcome]) -> bool {
    outcomes
        .first()
        .is_some_and(|outcome| outcome.claimed_complete)
}

/// B: the builder claim AND the review self-check exiting 0.
fn b_completion(outcomes: &[DispatchOutcome]) -> bool {
    a_completion(outcomes) && outcomes.len() >= 2 && outcomes[1].self_check_exit == Some(0)
}

/// C: the builder claim AND the fresh critic's PASS verdict. `None` and
/// INCONCLUSIVE never complete.
fn c_completion(outcomes: &[DispatchOutcome]) -> bool {
    a_completion(outcomes) && outcomes.len() >= 2 && outcomes[1].verdict == Some(Outcome::Pass)
}

/// Wraps a seam failure with its leg and locus (the failure locus travels).
fn leg_execution(
    error: BaselineError,
    locus: &'static str,
    scenario: BaselineScenario,
) -> BaselineError {
    BaselineError::Execution {
        locus,
        detail: format!("scenario {}: {error}", scenario.as_str()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::cell::RefCell;
    use std::path::PathBuf;

    use super::super::task::parse_task;

    /// A fixed instant, named (no numeral shares a line with a duration).
    const NOW: u64 = 1_700_000_000;
    /// The distinctive claim text the fake builder carries — the marker the
    /// blind-envelope test proves never reaches the critic request.
    const FORGED_CLAIM: &str = "FORGED-CLAIM-MARKER-7f3a self_checks all pass";
    /// The fake builder's token figure (named: the figure never shares a
    /// unit with a duration-shaped identifier).
    const FAKE_TOKEN_FIGURE: u64 = 5;
    /// The fake dispatch's real rest span: the leg's monotonic wall must
    /// cover it (`thread::sleep` is at-least), so the sub-second figure is
    /// deterministically nonzero milliseconds.
    const FAKE_REST_SPAN: u64 = 5;

    fn fixture_task() -> BaselineTask {
        let text = [
            "schema: baseline_task@v1",
            "id: gadget-preserve",
            "prompt: |",
            "  Implement src/gadget.rs; preserve MARKER_X and add MARKER_Y.",
            "requirement: |",
            "  src/gadget.rs must preserve MARKER_X and add MARKER_Y.",
            &format!("held_out: {}", held_out_relative_path()),
            "scenarios: [honest-success, lying-defect, subtle-defect, compromised-critic, silent]",
        ]
        .join("\n");
        parse_task(&text).expect("fixture task")
    }

    /// The scripted executor: records every request, answers per role.
    struct FakeExecutor {
        requests: RefCell<Vec<DispatchRequest>>,
        materialized: RefCell<Vec<BaselineScenario>>,
        builder_claim: bool,
        review_exit: Option<i32>,
        critic_verdict: Option<Outcome>,
        held_out_reading: HeldOutReading,
        held_out_basis: HeldOutBasis,
        /// D-only scripting.
        full_loops: RefCell<Vec<BaselineScenario>>,
        loop_state: Option<Arc<dyn StateStore>>,
        loop_terminal: TerminalState,
        reserve_intact: bool,
    }

    impl Default for FakeExecutor {
        fn default() -> Self {
            Self {
                requests: RefCell::new(Vec::new()),
                materialized: RefCell::new(Vec::new()),
                builder_claim: true,
                review_exit: Some(0),
                critic_verdict: Some(Outcome::Pass),
                held_out_reading: HeldOutReading::Pass,
                held_out_basis: HeldOutBasis::LegCopy,
                full_loops: RefCell::new(Vec::new()),
                loop_state: None,
                loop_terminal: TerminalState::Settled,
                reserve_intact: true,
            }
        }
    }

    impl FakeExecutor {
        fn requests_for(&self, role: Role) -> Vec<DispatchRequest> {
            self.requests
                .borrow()
                .iter()
                .filter(|request| request.role == role)
                .cloned()
                .collect()
        }
    }

    impl BaselineExecutor for FakeExecutor {
        fn materialize_workspace(
            &self,
            _task: &BaselineTask,
            scenario: BaselineScenario,
            parent: &Path,
        ) -> Result<PathBuf, BaselineError> {
            self.materialized.borrow_mut().push(scenario);
            Ok(parent.join(scenario.as_str()))
        }

        fn dispatch_role(
            &self,
            request: &DispatchRequest,
        ) -> Result<DispatchOutcome, BaselineError> {
            self.requests.borrow_mut().push(request.clone());
            // A real (if tiny) rest: the leg's wall then provably covers
            // this span, so the monotonic figure is a genuine measurement
            // with a floor, never a truncation artifact.
            std::thread::sleep(std::time::Duration::from_millis(FAKE_REST_SPAN));
            let outcome = match request.role {
                Role::Builder => DispatchOutcome {
                    claimed_complete: self.builder_claim,
                    self_check_exit: None,
                    verdict: None,
                    payment: Payment {
                        tokens: FAKE_TOKEN_FIGURE,
                        tool_calls: 1,
                        wall_seconds: 0,
                        cost_usd_micro: 0,
                    },
                    regressions: None,
                    // The forged claim: the builder result document's text —
                    // which must NEVER reach the critic request.
                    detail: FORGED_CLAIM.to_owned(),
                },
                Role::Reviewer => DispatchOutcome {
                    claimed_complete: true,
                    self_check_exit: self.review_exit,
                    verdict: None,
                    payment: Payment::default(),
                    regressions: None,
                    detail: "declared check re-run".to_owned(),
                },
                Role::Critic => DispatchOutcome {
                    claimed_complete: true,
                    self_check_exit: None,
                    verdict: self.critic_verdict,
                    payment: Payment::default(),
                    regressions: None,
                    detail: "critic verdict".to_owned(),
                },
            };
            Ok(outcome)
        }

        fn run_full_loop(
            &self,
            _task: &BaselineTask,
            _project_root: &Path,
            scenario: BaselineScenario,
        ) -> Result<LoopRunOutcome, BaselineError> {
            self.full_loops.borrow_mut().push(scenario);
            let state = self.loop_state.clone().expect("D tests install a store");
            Ok(LoopRunOutcome {
                run_id: run_id(),
                final_state: self.loop_terminal,
                reserve_intact: self.reserve_intact,
                tool_calls: Some(9),
                regressions: None,
                state,
            })
        }

        fn run_held_out(
            &self,
            _workspace: &Path,
            _task: &BaselineTask,
        ) -> Result<HeldOutVerdict, BaselineError> {
            Ok(HeldOutVerdict {
                reading: self.held_out_reading,
                basis: self.held_out_basis,
                detail: "held-out exit 0".to_owned(),
            })
        }
    }

    fn run_id() -> RunId {
        RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("a valid run id")
    }

    fn harness() -> BaselineHarness {
        BaselineHarness::new()
    }

    fn bound() -> Deadline {
        Deadline::at_epoch(gauntlet_domain::EpochSeconds::new(NOW + 600))
    }

    fn scratch() -> PathBuf {
        std::env::temp_dir().join("gauntlet-baseline-harness-tests")
    }

    // -- A posture ---------------------------------------------------------

    /// A makes exactly ONE dispatch (the builder), completes on the
    /// builder's own claim, pays that dispatch's payment, and its burden is
    /// zero: no posture-internal verification at all.
    #[test]
    fn posture_a_makes_one_dispatch_and_trusts_it() {
        let task = fixture_task();
        let executor = FakeExecutor {
            held_out_reading: HeldOutReading::Fail,
            ..FakeExecutor::default()
        };
        let result = harness()
            .run_profile(
                &executor,
                BaselineProfile::A,
                &task,
                &scratch(),
                &[BaselineScenario::LyingDefect],
                bound(),
            )
            .expect("profile A runs");
        assert_eq!(result.legs.len(), 1);
        let leg = &result.legs[0];
        assert_eq!(leg.dispatches, 1);
        assert_eq!(executor.requests_for(Role::Builder).len(), 1);
        assert_eq!(executor.requests_for(Role::Reviewer).len(), 0);
        assert_eq!(executor.requests_for(Role::Critic).len(), 0);
        assert!(leg.claimed_complete);
        // Held-out substantively failed: the claim was false — the FALSE
        // COMPLETION shape.
        assert_eq!(leg.held_out, HeldOutReading::Fail);
        assert_eq!(leg.false_completion, Some(true));
        assert!(leg.defect_escape);
        assert_eq!(leg.payment.tokens, 5);
        assert_eq!(leg.burden.instrument_runs, 0);
        assert_eq!(leg.burden.review_dispatches, 0);
        // A's completion basis is the builder's own claim.
        assert_eq!(
            result.completion_basis,
            completion_basis(BaselineProfile::A)
        );
        assert_eq!(result.totals.false_completions, 1);
    }

    // -- B posture ---------------------------------------------------------

    /// B makes TWO dispatches (builder, then reviewer), and completion
    /// requires the claim AND the review exit 0.
    #[test]
    fn posture_b_reviews_with_the_declared_check() {
        let task = fixture_task();
        // Builder claims, but the declared check fails: B does NOT complete.
        let failing_check = FakeExecutor {
            review_exit: Some(1),
            ..FakeExecutor::default()
        };
        let result = harness()
            .run_profile(
                &failing_check,
                BaselineProfile::B,
                &task,
                &scratch(),
                &[BaselineScenario::LyingDefect],
                bound(),
            )
            .expect("profile B runs");
        let leg = &result.legs[0];
        assert_eq!(leg.dispatches, 2);
        assert!(!leg.claimed_complete);
        assert_eq!(leg.false_completion, Some(false));
        assert_eq!(leg.burden.instrument_runs, 1);
        assert_eq!(leg.burden.review_dispatches, 1);

        // Builder claims and the check exits 0: B completes.
        let passing_check = FakeExecutor::default();
        let result = harness()
            .run_profile(
                &passing_check,
                BaselineProfile::B,
                &task,
                &scratch(),
                &[BaselineScenario::SubtleDefect],
                bound(),
            )
            .expect("profile B runs");
        let leg = &result.legs[0];
        assert!(leg.claimed_complete);
        assert_eq!(leg.dispatches, 2);
        // The reviewer's body is the review directive, not the task prompt.
        let reviews = passing_check.requests_for(Role::Reviewer);
        assert_eq!(reviews.len(), 1);
        assert!(reviews[0].body.contains(&declared_check_path()));
        assert_ne!(reviews[0].body, task.prompt());
    }

    // -- C posture + T2 blind envelope -------------------------------------

    /// T2 (app half): the critic request body is EXACTLY the requirement
    /// text — verbatim — and the builder outcome's claim text (the forged
    /// marker) appears NOWHERE in it. The reviewer role is never used.
    #[test]
    fn t2_the_critic_envelope_is_blind_to_the_builder_result() {
        let task = fixture_task();
        let executor = FakeExecutor::default();
        harness()
            .run_profile(
                &executor,
                BaselineProfile::C,
                &task,
                &scratch(),
                &[BaselineScenario::LyingDefect],
                bound(),
            )
            .expect("profile C runs");
        let critics = executor.requests_for(Role::Critic);
        assert_eq!(critics.len(), 1);
        let critic = &critics[0];
        // Verbatim: the body IS the requirement text.
        assert_eq!(critic.body, task.requirement());
        // And the builder's forged claim text is structurally absent.
        assert!(!critic.body.contains("FORGED-CLAIM-MARKER"));
        assert!(!critic.body.contains(FORGED_CLAIM));
        // Dispatch indexes are posture-internal positions.
        assert_eq!(critic.dispatch_index, 1);
    }

    /// C completes only when the builder claims AND the critic passes; a
    /// compromised critic's false pass completes the leg (the escape the
    /// matrix records).
    #[test]
    fn posture_c_completes_on_claim_and_critic_pass() {
        let task = fixture_task();
        let passing = FakeExecutor::default();
        let result = harness()
            .run_profile(
                &passing,
                BaselineProfile::C,
                &task,
                &scratch(),
                &[BaselineScenario::SubtleDefect],
                bound(),
            )
            .expect("profile C runs");
        assert!(result.legs[0].claimed_complete);
        assert_eq!(result.legs[0].dispatches, 2);

        let refusing = FakeExecutor {
            critic_verdict: Some(Outcome::Fail),
            ..FakeExecutor::default()
        };
        let result = harness()
            .run_profile(
                &refusing,
                BaselineProfile::C,
                &task,
                &scratch(),
                &[BaselineScenario::LyingDefect],
                bound(),
            )
            .expect("profile C runs");
        assert!(!result.legs[0].claimed_complete);
        assert_eq!(result.legs[0].false_completion, Some(false));
    }

    // -- Harness mechanics -------------------------------------------------

    /// A leg that claims with a PASSING held-out run is NOT a false
    /// completion; the held-out verdict is the independent judge.
    #[test]
    fn honest_completion_is_not_a_false_completion() {
        let task = fixture_task();
        let executor = FakeExecutor::default();
        let result = harness()
            .run_profile(
                &executor,
                BaselineProfile::A,
                &task,
                &scratch(),
                &[BaselineScenario::HonestSuccess],
                bound(),
            )
            .expect("profile A runs");
        let leg = &result.legs[0];
        assert!(leg.claimed_complete);
        assert_eq!(leg.held_out, HeldOutReading::Pass);
        assert_eq!(leg.false_completion, Some(false));
        assert!(!leg.defect_escape);
    }

    /// Every leg materializes a FRESH workspace named for its scenario —
    /// nothing shares state across legs.
    #[test]
    fn every_leg_gets_a_fresh_workspace() {
        let task = fixture_task();
        let executor = FakeExecutor::default();
        harness()
            .run_profile(
                &executor,
                BaselineProfile::A,
                &task,
                &scratch(),
                &[BaselineScenario::HonestSuccess, BaselineScenario::Silent],
                bound(),
            )
            .expect("profile A runs");
        assert_eq!(
            executor.materialized.borrow().clone(),
            vec![BaselineScenario::HonestSuccess, BaselineScenario::Silent]
        );
        let builders = executor.requests_for(Role::Builder);
        assert_eq!(builders[0].workspace, scratch().join("honest-success"));
        assert_eq!(builders[1].workspace, scratch().join("silent"));
    }

    /// m2 (critic residual 6): the verdict's judged basis travels into the
    /// leg record, and the wall is the harness's own monotonic measurement
    /// (see `a_sub_second_leg_records_a_nonzero_monotonic_wall`) — the
    /// Clock port is no longer on the wall's path.
    #[test]
    fn the_leg_record_carries_the_verdicts_basis() {
        let task = fixture_task();
        let executor = FakeExecutor::default();
        let result = harness()
            .run_profile(
                &executor,
                BaselineProfile::A,
                &task,
                &scratch(),
                &[BaselineScenario::LyingDefect],
                bound(),
            )
            .expect("profile A runs");
        assert_eq!(result.legs[0].held_out_basis, HeldOutBasis::LegCopy);
    }

    /// F1 (task-26 corrections): a TAMPERED judge channel reaches the leg
    /// record as an INCONCLUSIVE reading over the named tamper basis —
    /// never a pass, never a substantive fail, never a derived
    /// false-completion figure, and never an escape. The claimed-complete
    /// figure stays whatever the posture's own basis derived (the durable
    /// record governs it); the inconclusive held-out column is the
    /// disclosure.
    #[test]
    fn a_tampered_judge_channel_is_inconclusive_never_a_verdict() {
        let task = fixture_task();
        let executor = FakeExecutor {
            // A claiming builder: the completion figure is the posture's own
            // basis, independent of what the held-out could read.
            builder_claim: true,
            held_out_reading: HeldOutReading::InstrumentError,
            held_out_basis: HeldOutBasis::JudgeChannelTampered,
            ..FakeExecutor::default()
        };
        let result = harness()
            .run_profile(
                &executor,
                BaselineProfile::A,
                &task,
                &scratch(),
                &[BaselineScenario::HonestSuccess],
                bound(),
            )
            .expect("profile A runs");
        let leg = &result.legs[0];
        assert_eq!(leg.held_out, HeldOutReading::InstrumentError);
        assert_eq!(leg.held_out_basis, HeldOutBasis::JudgeChannelTampered);
        assert_eq!(leg.false_completion, None);
        assert!(!leg.defect_escape);
        assert!(
            leg.claimed_complete,
            "the durable record still governs the completion figure; the tamper \
             lives in the held-out columns"
        );
    }

    /// F5: a BROKEN instrument (an exit outside the contract's pass/fail
    /// bands, or a spawn failure) is an INCONCLUSIVE reading over the
    /// instrument-error basis — the leg never records a substantive fail
    /// and never derives a false completion from it.
    #[test]
    fn a_broken_instrument_is_inconclusive_never_a_substantive_fail() {
        let task = fixture_task();
        let executor = FakeExecutor {
            builder_claim: true,
            held_out_reading: HeldOutReading::InstrumentError,
            held_out_basis: HeldOutBasis::InstrumentError,
            ..FakeExecutor::default()
        };
        let result = harness()
            .run_profile(
                &executor,
                BaselineProfile::A,
                &task,
                &scratch(),
                &[BaselineScenario::HonestSuccess],
                bound(),
            )
            .expect("profile A runs");
        let leg = &result.legs[0];
        assert_eq!(leg.held_out, HeldOutReading::InstrumentError);
        assert_eq!(leg.held_out_basis, HeldOutBasis::InstrumentError);
        assert_eq!(leg.false_completion, None);
        assert!(!leg.defect_escape);
    }

    /// m1 (critic residual 5): a leg that completed NOTHING carries no
    /// settlement-reliability figure — no denominator, no figure (the
    /// metrics module's own rule). The pre-correction behavior recorded
    /// `Some(0)` for such legs; this test was RED first.
    #[test]
    fn a_zero_completion_leg_carries_no_reliability_figure() {
        let task = fixture_task();
        let executor = FakeExecutor {
            builder_claim: false,
            held_out_reading: HeldOutReading::Fail,
            ..FakeExecutor::default()
        };
        let result = harness()
            .run_profile(
                &executor,
                BaselineProfile::A,
                &task,
                &scratch(),
                &[BaselineScenario::Silent],
                bound(),
            )
            .expect("profile A runs");
        let leg = &result.legs[0];
        assert!(!leg.claimed_complete, "the silent leg completes nothing");
        assert_eq!(
            leg.settlement_reliability_per_10k, None,
            "a leg with zero completions carries no figure"
        );
    }

    /// m4 (critic residual 8): the leg wall is measured with a monotonic
    /// instant pair inside the harness, so a SUB-SECOND leg records a
    /// nonzero millisecond figure — the wall dimension stays real where
    /// the postures differ least. The pre-correction behavior derived the
    /// figure from the second-resolution Clock and recorded 0; this test
    /// was RED first.
    #[test]
    fn a_sub_second_leg_records_a_nonzero_monotonic_wall() {
        let task = fixture_task();
        let executor = FakeExecutor::default();
        let result = harness()
            .run_profile(
                &executor,
                BaselineProfile::A,
                &task,
                &scratch(),
                &[BaselineScenario::HonestSuccess],
                bound(),
            )
            .expect("profile A runs");
        assert!(
            result.legs[0].measured_wall_ms >= FAKE_REST_SPAN,
            "a sub-second leg still measures a real, nonzero wall in milliseconds \
             (the leg's floor is the dispatch rest span)"
        );
    }

    /// An empty scenario list is refused before anything runs (a profile
    /// with zero legs is not a measurement). The undeclared-scenario
    /// refusal is `an_undeclared_scenario_is_refused`.
    #[test]
    fn an_empty_scenario_list_is_refused() {
        let task = fixture_task();
        let executor = FakeExecutor::default();
        let error = harness()
            .run_profile(
                &executor,
                BaselineProfile::A,
                &task,
                &scratch(),
                &[],
                bound(),
            )
            .expect_err("no scenarios");
        assert!(matches!(error, BaselineError::Configuration { .. }));
        // Nothing ran.
        assert!(executor.requests.borrow().is_empty());
        assert!(executor.materialized.borrow().is_empty());
    }

    /// The fake task declares all five scenarios; to exercise the refusal,
    /// a task declaring fewer must refuse an undeclared run.
    #[test]
    fn an_undeclared_scenario_is_refused() {
        let text = [
            "schema: baseline_task@v1",
            "id: narrow",
            "prompt: |",
            "  the text",
            "requirement: |",
            "  the requirement",
            &format!("held_out: {}", held_out_relative_path()),
            "scenarios: [honest-success]",
        ]
        .join("\n");
        let task = parse_task(&text).expect("narrow task");
        let executor = FakeExecutor::default();
        let error = harness()
            .run_profile(
                &executor,
                BaselineProfile::A,
                &task,
                &scratch(),
                &[BaselineScenario::Silent],
                bound(),
            )
            .expect_err("silent is not declared");
        assert!(
            matches!(error, BaselineError::Configuration { .. }),
            "{error}"
        );
        // Nothing ran.
        assert!(executor.requests.borrow().is_empty());
    }

    /// A dispatch that cannot execute is a typed EXECUTION error naming the
    /// locus — never a leg record with invented facts.
    #[test]
    fn a_failing_dispatch_is_a_typed_execution_error() {
        struct BrokenExecutor;
        impl BaselineExecutor for BrokenExecutor {
            fn materialize_workspace(
                &self,
                _: &BaselineTask,
                _: BaselineScenario,
                _: &Path,
            ) -> Result<PathBuf, BaselineError> {
                Ok(std::env::temp_dir().join("gauntlet-broken"))
            }

            fn dispatch_role(&self, _: &DispatchRequest) -> Result<DispatchOutcome, BaselineError> {
                Err(BaselineError::Execution {
                    locus: "dispatch_role",
                    detail: "the child could not start".to_owned(),
                })
            }

            fn run_full_loop(
                &self,
                _: &BaselineTask,
                _: &Path,
                _: BaselineScenario,
            ) -> Result<LoopRunOutcome, BaselineError> {
                unreachable!("A never runs the loop")
            }

            fn run_held_out(
                &self,
                _: &Path,
                _: &BaselineTask,
            ) -> Result<HeldOutVerdict, BaselineError> {
                unreachable!("the leg already failed")
            }
        }
        let task = fixture_task();
        let error = harness()
            .run_profile(
                &BrokenExecutor,
                BaselineProfile::A,
                &task,
                &scratch(),
                &[BaselineScenario::HonestSuccess],
                bound(),
            )
            .expect_err("the dispatch cannot execute");
        match error {
            BaselineError::Execution { locus, detail } => {
                assert_eq!(locus, "dispatch_role(builder)");
                assert!(detail.contains("honest-success"));
                assert!(detail.contains("the child could not start"));
            }
            other => panic!("expected an execution error, got {other:?}"),
        }
    }
}
