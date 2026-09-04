//! The evaluation harness (plan Task 23, charter D4's `evaluate` piece;
//! spec §19.4: "current → candidate → isolated evaluation"): per task
//! set, one CONTROL run (current configuration) and one TREATMENT run
//! (the candidate configuration delta applied, `parent_run` set — the
//! §5.3.3 isolated-evaluation shape), both through the INJECTED executor
//! seam, both against the SAME frozen reference.
//!
//! The executor seam is an APP-OWNED trait — the loop is application, not
//! infrastructure, so the seam deliberately does not live in
//! gauntlet-ports — implemented at the CLI composition root over the real
//! composed loop ([`compose_run`](crates/gauntlet-cli's composition)); the
//! app consumes it injected, so the harness composes with the REAL loop in
//! the p9 proof and with doubles in unit tests.
//!
//! After each arm's run reaches a terminal state, the harness derives the
//! arm's typed metrics from that run's DURABLE facts (the same function
//! both arms), imports the run's memory through the §19.1 developmental
//! import under every declared protected capability (the import runs only
//! after the run is terminal — the evolving-run re-import wedge RES-03
//! stays closed by construction), and appends the durable
//! [`EvaluationRecord`]. A run that FAILED to execute is a typed error up
//! to the operator — insufficient evidence, never a negative result; a
//! run that executed but did not SETTLE is recorded as it happened, and
//! the gate refuses promotion on the evidence gap.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use gauntlet_domain::RevisionId;
use gauntlet_domain::{CapabilityId, Digest, RunId, TerminalState};
use gauntlet_ports::capability_ledger::CapabilityLedger;
use gauntlet_ports::clock::Clock;
use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::revision_ledger::{
    EvaluationAppend, EvaluationRecord, RevisionLedger, RevisionLedgerError, RunMetrics,
};
use gauntlet_ports::state_store::StateStore;

use crate::memory::developmental::{DevelopmentalImport, ImportError};
use crate::promote::delta::DeltaDocument;
use crate::promote::metrics::{MetricsError, derive_run_metrics};

/// One arm's execution request: WHERE the task-set project lives, the
/// operator's intent, the candidate delta (treatment only), and — for the
/// treatment arm — the control run this evaluation is isolated under
/// (`RunDefinition.parent_run`, the §5.3.3 shape).
#[derive(Debug, Clone)]
pub struct ExecutionRequest {
    pub project_root: PathBuf,
    pub intent: String,
    pub delta: Option<DeltaDocument>,
    pub parent_run: Option<RunId>,
}

impl ExecutionRequest {
    /// The control arm: current configuration, no parent.
    pub fn control(project_root: PathBuf, intent: &str) -> Self {
        Self {
            project_root,
            intent: intent.to_owned(),
            delta: None,
            parent_run: None,
        }
    }

    /// The treatment arm: the candidate delta applied, isolated under the
    /// control run.
    pub fn treatment(
        project_root: PathBuf,
        intent: &str,
        delta: DeltaDocument,
        parent: &RunId,
    ) -> Self {
        Self {
            project_root,
            intent: intent.to_owned(),
            delta: Some(delta),
            parent_run: Some(parent.clone()),
        }
    }
}

/// One arm's execution result: the run's id, its terminal state, the
/// frozen reference digest the run settled against, and the run's OWN
/// state-store handle (the metrics derive from THAT store — each
/// evaluation run composes its own durable substrate). The handle is a
/// trait object and carries no Debug; outcomes are printed through their
/// fields, never through a derived dump.
#[derive(Clone)]
pub struct ExecutionOutcome {
    pub run_id: RunId,
    pub terminal: TerminalState,
    pub reference_digest: Digest,
    pub state: Arc<dyn StateStore>,
}

/// Why the executor seam refused — typed.
#[derive(Debug, Clone, PartialEq)]
pub enum ExecutionError {
    /// The executor's composition or run failed, with the locus the
    /// executor reported.
    Failed { detail: String },
}

impl core::fmt::Display for ExecutionError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Failed { detail } => write!(f, "the evaluation run failed to execute: {detail}"),
        }
    }
}

impl std::error::Error for ExecutionError {}

/// The executor seam (charter D4): the harness composes CONTROL and
/// TREATMENT runs through THIS trait, implemented at the CLI composition
/// root over the real composed loop.
pub trait EvaluationExecutor {
    fn execute(
        &self,
        request: &ExecutionRequest,
        deadline: Deadline,
    ) -> Result<ExecutionOutcome, ExecutionError>;

    /// COR-1 (critic C7/m3): the composition declares WHICH of the
    /// candidate delta's entries it can provably apply, BEFORE any arm
    /// runs. An evaluated delta whose declared change never reached the
    /// treatment arm would adjudicate the WRONG candidate — so every
    /// entry must be provably applied, or nothing is evaluated. The
    /// refusal names EVERY unappliable entry.
    fn validate_delta(&self, delta: &DeltaDocument) -> Result<(), ExecutionError>;
}

/// Why an evaluation refused — typed, with the locus.
#[derive(Debug, Clone, PartialEq)]
pub enum EvaluationError {
    /// The revision has no admitted proposal — nothing can be evaluated.
    UnknownRevision { revision: RevisionId },
    /// The revision ledger refused a read or the record append.
    Ledger(RevisionLedgerError),
    /// An arm failed to execute. NOTHING was recorded: a failed evaluation
    /// run is insufficient evidence, never a negative result.
    Execution { arm: &'static str, detail: String },
    /// The treatment arm's ISOLATION could not be verified: its durable
    /// run record is missing or does not carry the declared `parent_run`
    /// (COR-1, critic M2). NOTHING is recorded — the evaluation evidence
    /// would be invalid, and invalid evidence rests at Provisional
    /// (re-run properly), never a promotion.
    IsolationUnverifiable {
        arm: &'static str,
        run: RunId,
        detail: String,
    },
    /// The candidate delta carries entries this composition cannot
    /// provably apply (COR-1, critic C7). NOTHING runs and NOTHING is
    /// recorded: evaluating would adjudicate a candidate whose declared
    /// change never reached the treatment arm.
    DeltaUnappliable { detail: String },
    /// An arm's metrics could not be derived from its durable facts.
    Metrics { run: RunId, detail: String },
    /// A substrate read behind the developmental import failed.
    Import { detail: String },
    /// An evaluation arm's project resolved a frozen reference digest
    /// DIFFERENT from the promoting project's verified lock (COR-2, R1):
    /// the evaluation would adjudicate under a moved ruler. NOTHING is
    /// recorded — a moved-ruler evaluation is not evidence, and the gate
    /// never sees it. The digests are boxed to keep the typed error small
    /// enough to travel in a `Result` (the repo clippy gate rejects a
    /// large error payload); the types are exactly what they were.
    ReferenceMoved {
        arm: &'static str,
        run: RunId,
        carried: Box<Digest>,
        current: Box<Digest>,
    },
}

impl core::fmt::Display for EvaluationError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UnknownRevision { revision } => write!(
                f,
                "revision {revision} has no admitted proposal; nothing can be evaluated"
            ),
            Self::Ledger(error) => write!(f, "the evaluation's ledger write failed: {error}"),
            Self::Execution { arm, detail } => {
                write!(f, "the {arm} arm failed to execute: {detail}")
            }
            Self::IsolationUnverifiable { arm, run, detail } => write!(
                f,
                "the {arm} arm's isolation is unverifiable (run {run}): {detail} — nothing \
                 was recorded; re-run the evaluation so the treatment carries the declared \
                 parent"
            ),
            Self::DeltaUnappliable { detail } => write!(
                f,
                "the candidate delta could not be applied in full: {detail} — every entry \
                 of an evaluated delta must be provably applied, or nothing is evaluated"
            ),
            Self::Metrics { run, detail } => {
                write!(f, "run {run}'s metrics are underivable: {detail}")
            }
            Self::Import { detail } => {
                write!(f, "the evaluation run's memory import failed: {detail}")
            }
            Self::ReferenceMoved {
                arm,
                run,
                carried,
                current,
            } => write!(
                f,
                "the {arm} arm (run {run}) settled against frozen reference {carried}, but the \
                 promoting project's verified lock carries {current} — a moved ruler is \
                 refused at evaluation time and nothing was recorded (spec 19.4, condition 2)"
            ),
        }
    }
}

impl std::error::Error for EvaluationError {}

/// One evaluation's operator declaration: the revision, the candidate
/// delta (parsed from the revision's `candidate_change` bytes by the
/// caller that can resolve the evidence store), the task-set project
/// roots (operator-NAMED — the harness never invents task sets from the
/// active reference alone), and the protected capabilities the §19.4
/// non-inferiority condition reads.
#[derive(Debug, Clone)]
pub struct EvaluationRequest {
    pub revision: RevisionId,
    /// The declared target metric (§19.4 condition 7's vocabulary) —
    /// recorded with the durable evaluation, and exactly what the gate
    /// judges.
    pub metric: crate::promote::metrics::TargetMetric,
    /// The promoting project's VERIFIED frozen reference digest (COR-2,
    /// R1) — the CLI derives it from the verified reference lock. The
    /// harness refuses any arm whose project resolved a DIFFERENT frozen
    /// digest at evaluate time, before anything is recorded: the
    /// regression set is an operator-named project with its own
    /// frozen reference, and a moved regression ruler never becomes durable
    /// evidence.
    pub current_reference_digest: Digest,
    pub delta: DeltaDocument,
    pub held_out_root: PathBuf,
    pub regression_root: PathBuf,
    pub protected: Vec<CapabilityId>,
    pub intent: String,
}

/// What one evaluation appended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvaluationSummary {
    pub record: EvaluationRecord,
    pub idempotent: bool,
    /// The developmental imports' appended-row counts, per protected
    /// capability (a run already imported is the ledger's idempotent
    /// no-op).
    pub imported: usize,
}

/// The §19.4 evaluation harness.
#[derive(Clone)]
pub struct EvaluationHarness {
    ledger: Arc<dyn RevisionLedger>,
    capability_ledger: Arc<dyn CapabilityLedger>,
    clock: Arc<dyn Clock>,
}

impl EvaluationHarness {
    pub fn new(
        ledger: Arc<dyn RevisionLedger>,
        capability_ledger: Arc<dyn CapabilityLedger>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            ledger,
            capability_ledger,
            clock,
        }
    }

    /// Composes the four REAL runs (control + treatment over the held-out
    /// set AND the regression set), derives both arms' metrics from their
    /// durable facts, imports each finished run's memory, and appends the
    /// durable evaluation record.
    pub fn evaluate(
        &self,
        request: &EvaluationRequest,
        executor: &dyn EvaluationExecutor,
        deadline: Deadline,
    ) -> Result<EvaluationSummary, EvaluationError> {
        self.ledger
            .proposal(&request.revision, deadline)
            .map_err(EvaluationError::Ledger)?
            .ok_or_else(|| EvaluationError::UnknownRevision {
                revision: request.revision.clone(),
            })?;
        // COR-1 (critic C7): the composition must be able to apply EVERY
        // declared delta entry before any arm runs — an evaluation over a
        // partially applied delta would adjudicate the wrong candidate.
        // The refusal is typed and names every unappliable entry; no run
        // is charged and nothing is recorded.
        executor.validate_delta(&request.delta).map_err(|error| {
            EvaluationError::DeltaUnappliable {
                detail: error.to_string(),
            }
        })?;
        // HELD-OUT set: control first, then the treatment isolated under
        // it (parent_run — the §5.3.3 isolated-evaluation shape).
        let held_out_control = self.run_arm(
            "control",
            request,
            &request.held_out_root,
            None,
            executor,
            deadline,
        )?;
        let held_out_treatment = self.run_arm(
            "treatment",
            request,
            &request.held_out_root,
            Some(&held_out_control.run_id),
            executor,
            deadline,
        )?;
        // REGRESSION set: same shape.
        let regression_control = self.run_arm(
            "control",
            request,
            &request.regression_root,
            None,
            executor,
            deadline,
        )?;
        let regression_treatment = self.run_arm(
            "treatment",
            request,
            &request.regression_root,
            Some(&regression_control.run_id),
            executor,
            deadline,
        )?;
        // The durable metrics: the SAME function derives both arms.
        let control_held_out_metrics = self.metrics(&held_out_control, deadline)?;
        let treatment_held_out_metrics = self.metrics(&held_out_treatment, deadline)?;
        let control_regression_metrics = self.metrics(&regression_control, deadline)?;
        let treatment_regression_metrics = self.metrics(&regression_treatment, deadline)?;
        // Each finished evaluation run's developmental memory — the import
        // runs only after the run reached its terminal state (the executor
        // returns terminal outcomes), under every declared protected
        // capability.
        let mut imported = 0usize;
        for outcome in [
            &held_out_control,
            &held_out_treatment,
            &regression_control,
            &regression_treatment,
        ] {
            for capability in &request.protected {
                let import = DevelopmentalImport::new(
                    outcome.state.clone(),
                    self.capability_ledger.clone(),
                    self.clock.clone(),
                );
                let summary = import
                    .import_run(capability, &outcome.run_id, deadline)
                    .map_err(|error| match error {
                        ImportError::State(state) => EvaluationError::Import {
                            detail: state.to_string(),
                        },
                        ImportError::Ledger(ledger) => EvaluationError::Import {
                            detail: ledger.to_string(),
                        },
                        other @ (ImportError::RunNotDurable { .. }
                        | ImportError::RecoveryNotEarned { .. }) => EvaluationError::Import {
                            detail: other.to_string(),
                        },
                    })?;
                imported += summary.appended;
            }
        }
        let record = EvaluationRecord {
            revision: request.revision.clone(),
            metric: request.metric.token().to_owned(),
            protected: request
                .protected
                .iter()
                .map(|capability| capability.as_str().to_owned())
                .collect(),
            held_out_control: held_out_control.run_id.clone(),
            held_out_treatment: held_out_treatment.run_id,
            regression_control: regression_control.run_id.clone(),
            regression_treatment: regression_treatment.run_id,
            // The treatment arms' parent facts. The VERIFICATION against
            // each run's durable record happens in `run_arm` before this
            // constructor can run (COR-1, critic M2) — what is recorded
            // here is the verified fact, not the request's intent.
            held_out_treatment_parent: held_out_control.run_id,
            regression_treatment_parent: regression_control.run_id,
            control_reference_digest: held_out_control.reference_digest,
            treatment_reference_digest: held_out_treatment.reference_digest,
            // The REGRESSION arms' rulers are durable too (COR-2, R1) —
            // derived from the regression runs' own outcomes, exactly as
            // the held-out arms' are.
            regression_control_reference_digest: regression_control.reference_digest,
            regression_treatment_reference_digest: regression_treatment.reference_digest,
            control_held_out_metrics,
            treatment_held_out_metrics,
            control_regression_metrics,
            treatment_regression_metrics,
            at: self.clock.now(),
        };
        let EvaluationAppend { idempotent, .. } = self
            .ledger
            .append_evaluation(record.clone(), deadline)
            .map_err(EvaluationError::Ledger)?;
        Ok(EvaluationSummary {
            record,
            idempotent,
            imported,
        })
    }

    fn run_arm(
        &self,
        arm: &'static str,
        request: &EvaluationRequest,
        root: &Path,
        parent: Option<&RunId>,
        executor: &dyn EvaluationExecutor,
        deadline: Deadline,
    ) -> Result<ExecutionOutcome, EvaluationError> {
        let execution = match parent {
            None => ExecutionRequest::control(root.to_path_buf(), &request.intent),
            Some(control) => ExecutionRequest::treatment(
                root.to_path_buf(),
                &request.intent,
                request.delta.clone(),
                control,
            ),
        };
        let outcome =
            executor
                .execute(&execution, deadline)
                .map_err(|error| EvaluationError::Execution {
                    arm,
                    detail: error.to_string(),
                })?;
        // COR-2 (R1): the moved-ruler world is refused AT THE SOURCE. The
        // arm's project may freeze its own reference (the regression set
        // is an operator-named project); if it resolved a frozen reference
        // digest different from the promoting project's verified lock, the
        // evaluation would adjudicate under a moved ruler. Typed refusal,
        // BEFORE anything is recorded — a moved-ruler evaluation is not
        // evidence, and the gate never sees one.
        if outcome.reference_digest != request.current_reference_digest {
            return Err(EvaluationError::ReferenceMoved {
                arm,
                run: outcome.run_id.clone(),
                carried: Box::new(outcome.reference_digest.clone()),
                current: Box::new(request.current_reference_digest.clone()),
            });
        }
        // COR-1 (critic M2): the isolated-evaluation fact is VERIFIED, not
        // conventional. The harness holds the outcome's own state handle —
        // it reads the treatment run's DURABLE record and checks that the
        // declared parent is what the record carries, BEFORE anything is
        // appended. A missing record or a missing/mismatched parent is an
        // `IsolationUnverifiable` refusal: nothing is recorded, so the
        // gate's missing-evidence floor rests the revision at Provisional
        // (re-run properly), never a promotion on unverified isolation.
        if let Some(declared_parent) = parent {
            let run_record = outcome
                .state
                .load_run(&outcome.run_id, deadline)
                .map_err(|error| EvaluationError::IsolationUnverifiable {
                    arm,
                    run: outcome.run_id.clone(),
                    detail: format!("the durable run record could not be read: {error}"),
                })?
                .ok_or_else(|| EvaluationError::IsolationUnverifiable {
                    arm,
                    run: outcome.run_id.clone(),
                    detail: "the treatment run left no durable record".to_owned(),
                })?;
            let recorded_parent = run_record.parent_run();
            if recorded_parent != Some(declared_parent) {
                return Err(EvaluationError::IsolationUnverifiable {
                    arm,
                    run: outcome.run_id.clone(),
                    detail: format!(
                        "the declared isolated-evaluation parent is {declared_parent}, but the \
                         durable run record carries {}",
                        recorded_parent
                            .map(|parent| parent.as_str().to_owned())
                            .unwrap_or_else(|| "no parent".to_owned())
                    ),
                });
            }
        }
        Ok(outcome)
    }

    fn metrics(
        &self,
        outcome: &ExecutionOutcome,
        deadline: Deadline,
    ) -> Result<RunMetrics, EvaluationError> {
        derive_run_metrics(outcome.state.as_ref(), &outcome.run_id, deadline).map_err(|error| {
            match error {
                MetricsError::State(state) => EvaluationError::Metrics {
                    run: outcome.run_id.clone(),
                    detail: state.to_string(),
                },
                MetricsError::NotDurable { .. } | MetricsError::Undecidable { .. } => {
                    EvaluationError::Metrics {
                        run: outcome.run_id.clone(),
                        detail: error.to_string(),
                    }
                }
            }
        })
    }
}
