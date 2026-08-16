//! `Run` — spec §5.3.3.
//!
//! Spec §5.3.3 invariant, quoted: "a `Run` has exactly one frozen
//! `reference_version` for its whole life. Changing the reference requires a
//! new run with `parent_run` set."
//!
//! The invariant is structural here: `reference_version` is a private field
//! set exactly once, at construction, and the only accessor returns an
//! immutable reference. There is no method that takes a `ReferenceVersion`
//! after `new`, so no public path can move a run onto a different ruler
//! mid-flight (G2, AK-025). `units_state` is deliberately absent from this
//! type: the spec calls it "a projection, rebuilt from events", and a domain
//! struct carrying a second writable copy of Unit status would be exactly the
//! drift the event log exists to prevent. It lands with the state store
//! (plan Task 6).

use core::fmt;

use crate::budget::Budget;
use crate::iteration::Iteration;
use crate::reference::ReferenceVersion;
use crate::values::{Digest, EpochSeconds, RunId, TerminalState};

/// `Run` invariant failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunError {
    /// The operator's requested transformation is absent (spec §5.3.3
    /// `intent` — "the operator's requested transformation, verbatim").
    IntentMissing,
    /// An iteration from a different run was attached.
    IterationFromAnotherRun { run: RunId, iteration_run: RunId },
    /// A terminal state was recorded on a run that already ended. A run ends
    /// exactly once; rewriting the ending is rewriting history.
    TerminalAlreadySet {
        current: TerminalState,
        attempted: TerminalState,
    },
}

impl fmt::Display for RunError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IntentMissing => {
                write!(
                    f,
                    "a run requires the operator's intent, verbatim (spec 5.3.3)"
                )
            }
            Self::IterationFromAnotherRun { run, iteration_run } => write!(
                f,
                "iteration belongs to run {iteration_run}, not {run} (spec 5.3.3)"
            ),
            Self::TerminalAlreadySet { current, attempted } => write!(
                f,
                "run already ended in {current}; a terminal state is recorded once, and {attempted} \
                 would rewrite history (spec 5.3.3)"
            ),
        }
    }
}

impl std::error::Error for RunError {}

/// Definition data for a [`Run`] (spec §5.3.3 shape). Plain pub fields
/// awaiting validation.
#[derive(Debug, Clone)]
pub struct RunDefinition {
    pub id: RunId,
    pub created_at: EpochSeconds,
    pub reference_version: ReferenceVersion,
    pub intent: String,
    pub config_fingerprint: Digest,
    pub environment_fingerprint: Digest,
    pub budget: Budget,
    /// Set for candidate-revision evaluation runs (spec §5.3.3 `parent_run`).
    pub parent_run: Option<RunId>,
}

/// One execution of the loop against one frozen reference (spec §5.3.3).
#[derive(Debug, Clone)]
pub struct Run {
    id: RunId,
    created_at: EpochSeconds,
    reference_version: ReferenceVersion,
    intent: String,
    config_fingerprint: Digest,
    environment_fingerprint: Digest,
    budget: Budget,
    iterations: Vec<Iteration>,
    terminal_state: Option<TerminalState>,
    parent_run: Option<RunId>,
}

impl Run {
    /// Opens a run. The reference version is adopted once, here, and is
    /// frozen for the run's whole life (spec §5.3.3).
    pub fn new(def: RunDefinition) -> Result<Self, RunError> {
        if def.intent.trim().is_empty() {
            return Err(RunError::IntentMissing);
        }
        Ok(Self {
            id: def.id,
            created_at: def.created_at,
            reference_version: def.reference_version,
            intent: def.intent,
            config_fingerprint: def.config_fingerprint,
            environment_fingerprint: def.environment_fingerprint,
            budget: def.budget,
            iterations: Vec::new(),
            terminal_state: None,
            parent_run: def.parent_run,
        })
    }

    pub fn id(&self) -> &RunId {
        &self.id
    }

    pub fn created_at(&self) -> EpochSeconds {
        self.created_at
    }

    /// The frozen reference identity. Spec §5.3.3: "a `Run` has exactly one
    /// frozen `reference_version` for its whole life." Read-only for the
    /// run's whole life; changing the reference requires a new run.
    pub fn reference_version(&self) -> &ReferenceVersion {
        &self.reference_version
    }

    pub fn intent(&self) -> &str {
        &self.intent
    }

    pub fn config_fingerprint(&self) -> &Digest {
        &self.config_fingerprint
    }

    pub fn environment_fingerprint(&self) -> &Digest {
        &self.environment_fingerprint
    }

    pub fn budget(&self) -> &Budget {
        &self.budget
    }

    /// The run this run evaluates a candidate revision for, when applicable.
    pub fn parent_run(&self) -> Option<&RunId> {
        self.parent_run.as_ref()
    }

    pub fn iterations(&self) -> &[Iteration] {
        &self.iterations
    }

    /// Attaches an iteration, refusing iterations that belong to another run.
    pub fn attach_iteration(&mut self, iteration: Iteration) -> Result<(), RunError> {
        if iteration.run_id() != &self.id {
            return Err(RunError::IterationFromAnotherRun {
                run: self.id.clone(),
                iteration_run: iteration.run_id().clone(),
            });
        }
        self.iterations.push(iteration);
        Ok(())
    }

    pub fn terminal_state(&self) -> Option<TerminalState> {
        self.terminal_state
    }

    /// Records the run's terminal state, exactly once (spec §5.3.3
    /// `terminal_state: null` until the run ends).
    pub fn terminate(&mut self, state: TerminalState) -> Result<(), RunError> {
        if let Some(current) = self.terminal_state {
            return Err(RunError::TerminalAlreadySet {
                current,
                attempted: state,
            });
        }
        self.terminal_state = Some(state);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::budget::{DiagnosticAllowance, Limits, MicroUsd};
    use crate::iteration::Iteration;
    use crate::values::{DimensionId, EpochSeconds, ReferenceId};

    fn run_id() -> RunId {
        RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("valid run id")
    }

    fn fingerprint(hex_tail: &str) -> Digest {
        Digest::new("sha256", hex_tail).expect("valid digest parts")
    }

    fn budget() -> Budget {
        Budget::new(
            Limits::new(10, 2, 2, 3600, 1000, MicroUsd::new(500)).expect("ceilings"),
            20,
            DiagnosticAllowance::new(100, MicroUsd::new(50)),
        )
        .expect("budget")
    }

    fn frozen_reference() -> ReferenceVersion {
        ReferenceVersion::new(
            ReferenceId::new("ref-main").expect("valid id"),
            4,
            fingerprint("a1b2c3d4"),
        )
    }

    fn definition() -> RunDefinition {
        RunDefinition {
            id: run_id(),
            created_at: EpochSeconds::new(1_700_000_000),
            reference_version: frozen_reference(),
            intent: "Make the loop settle honestly".to_owned(),
            config_fingerprint: fingerprint("e5f6a7b8"),
            environment_fingerprint: fingerprint("0f1e2d3c"),
            budget: budget(),
            parent_run: None,
        }
    }

    #[test]
    fn a_run_requires_the_operators_intent() {
        let mut def = definition();
        def.intent = "  ".to_owned();
        assert!(matches!(Run::new(def), Err(RunError::IntentMissing)));
    }

    #[test]
    fn the_frozen_reference_version_is_adopted_once_and_never_moved() {
        // Spec §5.3.3: "a `Run` has exactly one frozen `reference_version`
        // for its whole life. Changing the reference requires a new run with
        // `parent_run` set."
        let mut run = Run::new(definition()).expect("valid run");
        let adopted = run.reference_version().clone();
        assert_eq!(run.reference_version(), &adopted);
        // Every state the run can reach leaves the frozen version alone.
        run.attach_iteration(Iteration::new(
            run_id(),
            crate::values::IterationNo::new(1).expect("first"),
            None,
        ))
        .expect("own iteration attaches");
        run.terminate(TerminalState::Settled)
            .expect("runs end once");
        assert_eq!(run.reference_version(), &adopted);
        assert_eq!(run.terminal_state(), Some(TerminalState::Settled));
        // The identity carries id, version, and digest together.
        assert_eq!(run.reference_version().version(), 4);
        assert_eq!(run.reference_version().digest(), adopted.digest());
    }

    #[test]
    fn no_public_surface_can_supply_a_reference_version_after_construction() {
        // The structural half of the §5.3.3 rule: run.rs may mention
        // `reference_version` in exactly one method signature — the
        // by-reference getter — and never assigns the field after `new`.
        let (_, text) = crate::test_sources::read_all()
            .into_iter()
            .find(|(name, _)| name == "run.rs")
            .expect("run.rs is among the sources");
        // Production surface only: the test module below legitimately names
        // the field in its own functions and assertions.
        let production = text
            .split("#[cfg(test)]")
            .next()
            .expect("split always yields a first element")
            .to_owned();
        for line in production.lines() {
            let mentions = line.contains("reference_version");
            if mentions && line.contains("fn ") {
                assert!(
                    line.contains("pub fn reference_version(&self)"),
                    "a new method touched the frozen version: {line}"
                );
            }
            assert!(
                !line.trim_start().starts_with("self.reference_version ="),
                "the frozen version is assigned after construction: {line}"
            );
        }
    }

    #[test]
    fn a_terminal_state_is_recorded_exactly_once() {
        let mut run = Run::new(definition()).expect("valid run");
        assert_eq!(run.terminal_state(), None);
        run.terminate(TerminalState::BudgetExhausted)
            .expect("the first terminal state is recorded");
        assert!(matches!(
            run.terminate(TerminalState::Settled),
            Err(RunError::TerminalAlreadySet {
                current: TerminalState::BudgetExhausted,
                attempted: TerminalState::Settled,
            })
        ));
        assert_eq!(run.terminal_state(), Some(TerminalState::BudgetExhausted));
    }

    #[test]
    fn iterations_from_other_runs_are_refused() {
        let mut run = Run::new(definition()).expect("valid run");
        let foreign_id = RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAZ").expect("valid but other");
        let foreign = Iteration::new(
            foreign_id,
            crate::values::IterationNo::new(1).expect("first"),
            None,
        );
        assert!(matches!(
            run.attach_iteration(foreign),
            Err(RunError::IterationFromAnotherRun { .. })
        ));
        assert!(run.iterations().is_empty());
        let own = Iteration::new(
            run_id(),
            crate::values::IterationNo::new(1).expect("first"),
            None,
        );
        run.attach_iteration(own).expect("own iteration attaches");
        assert_eq!(run.iterations().len(), 1);
    }

    #[test]
    fn a_candidate_evaluation_run_names_its_parent() {
        // §5.3.3 `parent_run`: the shape a reference change requires.
        let mut def = definition();
        def.parent_run = Some(run_id());
        let run = Run::new(def).expect("valid run");
        assert_eq!(run.parent_run(), Some(&run_id()));
    }

    #[test]
    fn definition_fingerprints_are_carried() {
        // Prevent an accidental unused-import lint and pin the §5.3.3
        // fingerprint fields on the aggregate.
        let run = Run::new(definition()).expect("valid run");
        assert_eq!(run.config_fingerprint(), &fingerprint("e5f6a7b8"));
        assert_eq!(run.environment_fingerprint(), &fingerprint("0f1e2d3c"));
        assert_eq!(run.created_at().get(), 1_700_000_000);
        let _ = DimensionId::new("dim-01").expect("valid id");
    }
}
