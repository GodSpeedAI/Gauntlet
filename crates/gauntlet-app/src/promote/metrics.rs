//! The typed target-metric vocabulary and the run-metrics derivation
//! (plan Task 23, charter D4's `evaluate` piece): both evaluation arms'
//! figures derive from each run's DURABLE facts with the SAME function —
//! settled units, iterations, payment, the per-settlement verifying
//! instruments, and the dispatched-unit mix — read through the
//! `StateStore`'s typed reads. Nothing is claimed by the candidate or the
//! harness; every figure is a store read.

use core::fmt;

use gauntlet_domain::RunId;
use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::revision_ledger::{RunMetrics, SettlementFact};
use gauntlet_ports::state_store::StateStore;

/// The target metric's typed vocabulary (charter D4 condition 7): the
/// figures the §19.4 margin compares. Both are LOWER-IS-BETTER and both
/// derive from the same durable facts the control arm reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetMetric {
    /// Booked iterations per settlement.
    IterationsPerSettlement,
    /// Checkpoint payment (micro-dollars) per settlement.
    PaymentPerSettlement,
}

impl TargetMetric {
    /// Every metric, for iteration and validation.
    pub const ALL: &'static [Self] = &[Self::IterationsPerSettlement, Self::PaymentPerSettlement];

    /// Exact-match parse of the CLI's `--metric` token; unknown tokens are
    /// typed errors, never coerced defaults.
    pub fn parse(token: &str) -> Result<Self, UnknownMetric> {
        match token {
            "iterations_per_settlement" => Ok(Self::IterationsPerSettlement),
            "payment_per_settlement" => Ok(Self::PaymentPerSettlement),
            other => Err(UnknownMetric {
                input: other.to_owned(),
            }),
        }
    }

    /// The wire token.
    pub fn token(self) -> &'static str {
        match self {
            Self::IterationsPerSettlement => "iterations_per_settlement",
            Self::PaymentPerSettlement => "payment_per_settlement",
        }
    }

    /// The metric's figure over one arm's metrics, FIXED-POINT PER 10 000
    /// (cross-multiplied integer math — no floating point reaches a §19.4
    /// decision). `None` when the arm settled nothing: the figure is
    /// UNDECIDABLE, and the gate refuses on the evidence gap, never on a
    /// guessed value.
    ///
    /// `None` too when the arm did not reach a SETTLED terminal (COR-1,
    /// critic B1): an incomplete arm is an EVIDENCE GAP, never a figure —
    /// there is no sentinel value, because any magic figure (worst or
    /// best) fabricates evidence the run never produced. The gap is the
    /// gate's to adjudicate: Provisional, never promoted, never rejected.
    /// The completion difference itself is additionally governed by the
    /// regression-suite condition over the durable settlements.
    pub fn figure(self, metrics: &RunMetrics) -> Option<u64> {
        if !metrics.settled_terminal {
            return None;
        }
        let settlements = metrics.settlements.len() as u64;
        if settlements == 0 {
            return None;
        }
        // The numerators are bounded by configured run ceilings, so the
        // u128 intermediate cannot overflow a u64 per-10 000 figure for
        // any run the configuration can produce; a genuinely overflowing
        // figure saturates at u64::MAX rather than wrapping.
        let numerator = match self {
            Self::IterationsPerSettlement => u128::from(metrics.iterations),
            Self::PaymentPerSettlement => u128::from(metrics.payment_micro_usd),
        };
        let figure = numerator * 10_000 / u128::from(settlements);
        Some(u64::try_from(figure).unwrap_or(u64::MAX))
    }
}

/// An unknown `--metric` token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownMetric {
    pub input: String,
}

impl fmt::Display for UnknownMetric {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "metric token {:?} is outside the typed vocabulary ({})",
            self.input,
            TargetMetric::ALL
                .iter()
                .map(|metric| metric.token())
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

impl std::error::Error for UnknownMetric {}

/// Why a run's metrics could not be derived — insufficient evidence, never
/// a negative result.
#[derive(Debug, Clone, PartialEq)]
pub enum MetricsError {
    /// The run has no durable identity — there is nothing to derive from.
    NotDurable { run: RunId },
    /// A substrate read failed.
    State(gauntlet_ports::state_store::StateStoreError),
    /// The run is durable but the figures the gate compares are absent (a
    /// run that never booked a checkpoint carries no iterations or
    /// payment facts).
    Undecidable { run: RunId, detail: String },
}

impl fmt::Display for MetricsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotDurable { run } => {
                write!(
                    f,
                    "run {run} is not durable; there is nothing to derive from"
                )
            }
            Self::State(error) => write!(f, "the metrics derivation's read failed: {error}"),
            Self::Undecidable { run, detail } => {
                write!(f, "run {run}'s metrics are underivable: {detail}")
            }
        }
    }
}

impl std::error::Error for MetricsError {}

/// Derives one run's typed metrics from its durable facts. The same
/// function derives BOTH arms' figures (charter D4).
pub fn derive_run_metrics(
    state: &dyn StateStore,
    run: &RunId,
    deadline: Deadline,
) -> Result<RunMetrics, MetricsError> {
    if state
        .load_run(run, deadline)
        .map_err(MetricsError::State)?
        .is_none()
    {
        return Err(MetricsError::NotDurable { run: run.clone() });
    }
    // The terminal fact, DERIVED from durable records (spec 5.3.3's
    // terminal is the loop's own close decision; the store carries the
    // plan and the settlements, so the derived predicate is: every unit
    // of the latest checkpoint's plan is settled). A run that left plan
    // units unsettled has not reached its settled terminal — the figures
    // these facts would feed are UNDERIVABLE (an evidence gap the gate
    // records), never a placeholder value.
    let checkpoint = state
        .resume(run, deadline)
        .map_err(MetricsError::State)?
        .ok_or_else(|| MetricsError::Undecidable {
            run: run.clone(),
            detail: "no durable checkpoint — no iteration or payment facts exist".to_owned(),
        })?;
    let settlements_rows = state
        .settlements(run, deadline)
        .map_err(MetricsError::State)?;
    let settled_units: Vec<String> = settlements_rows
        .iter()
        .map(|settlement| settlement.unit_id().as_str().to_owned())
        .collect();
    let settled_terminal = checkpoint
        .unit_states
        .iter()
        .all(|(unit, _)| settled_units.contains(&unit.as_str().to_owned()))
        && !checkpoint.unit_states.is_empty();
    // The settlement facts: each committed settlement joined with its
    // deciding verification record (the instrument's §14.2 model-rung flag
    // and the substrate occurrence instant), in store order — the same
    // projection the §19.1 developmental import performs.
    let verifications = state
        .verification_records(run, deadline)
        .map_err(MetricsError::State)?;
    let settlements = settlements_rows
        .iter()
        .map(|settlement| {
            let decided = verifications
                .iter()
                .find(|record| record.verification() == settlement.verification());
            SettlementFact {
                unit: settlement.unit_id().as_str().to_owned(),
                instrument_deterministic: decided.map(|record| record.instrument().deterministic()),
                occurred_at: decided.map(|record| record.produced_at()),
            }
        })
        .collect();
    // The dispatched-unit mix: every unit the run dispatched (attempted),
    // sorted and deduplicated — the governance condition's durable figure.
    let mut dispatched: Vec<String> = verifications
        .iter()
        .map(|record| record.unit_id().as_str().to_owned())
        .collect();
    dispatched.sort();
    dispatched.dedup();
    Ok(RunMetrics {
        iterations: u64::from(checkpoint.iteration_no.get()),
        payment_tokens: checkpoint.budget_spent.tokens(),
        payment_micro_usd: checkpoint.budget_spent.cost_usd().get(),
        settlements,
        dispatched_units: dispatched,
        settled_terminal,
    })
}
