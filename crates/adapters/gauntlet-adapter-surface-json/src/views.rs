//! The run-scoped and unit-scoped read seam (charter D6): the §20.2
//! `GET /api/v1/runs/{id}` and `GET /api/v1/units/{id}` views, assembled
//! by the composition root from the EXISTING durable reads only —
//! `load_run`, `resume`, `verification_records`, `settlements`,
//! `queue.list()`, and the frozen-key tier facts. No new read port is
//! invented here; every field names a durable read.

use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::json::JsonValue;
use gauntlet_ports::operator_surface::OperatorSurfaceError;

use crate::unit_json;

/// One durable verification record's projection (the record facts the
/// CLI's `inspect --verifications` prints, as fields).
#[derive(Debug, Clone, PartialEq)]
pub struct VerificationFact {
    /// The unit the record verifies.
    pub unit_id: String,
    /// The three-state outcome (`pass` / `fail` / `inconclusive`) —
    /// never collapsed.
    pub outcome: String,
    /// The record's evidence ref.
    pub evidence: String,
    /// The record's verification ref.
    pub verification: String,
}

impl VerificationFact {
    fn to_json(&self) -> JsonValue {
        JsonValue::Object(vec![
            ("unit_id".to_owned(), JsonValue::Str(self.unit_id.clone())),
            ("outcome".to_owned(), JsonValue::Str(self.outcome.clone())),
            ("evidence".to_owned(), JsonValue::Str(self.evidence.clone())),
            (
                "verification".to_owned(),
                JsonValue::Str(self.verification.clone()),
            ),
        ])
    }
}

/// One durable settlement row's projection (the §16.3 commit facts).
#[derive(Debug, Clone, PartialEq)]
pub struct SettlementFact {
    pub unit_id: String,
    pub verification: String,
    pub evidence: String,
}

impl SettlementFact {
    fn to_json(&self) -> JsonValue {
        JsonValue::Object(vec![
            ("unit_id".to_owned(), JsonValue::Str(self.unit_id.clone())),
            (
                "verification".to_owned(),
                JsonValue::Str(self.verification.clone()),
            ),
            ("evidence".to_owned(), JsonValue::Str(self.evidence.clone())),
        ])
    }
}

/// One run's view (`GET /api/v1/runs/{id}`): the run-anchored durable
/// facts. Every field traces to a named durable read; absence is honest
/// (`None` / empty), never a guess.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RunView {
    /// The run's latest durable checkpoint iteration (`None` = the run
    /// never crossed a boundary — the status command's own honesty).
    pub iteration: Option<u32>,
    /// Units by frozen tier with their durable statuses (frozen-key tier
    /// facts + the checkpoint projection). Empty when the key is
    /// unreadable through the same reads `status` uses.
    pub units: Vec<gauntlet_ports::operator_surface::UnitSnapshot>,
    /// The run's open escalation tickets (the operator's actionable
    /// inbox for this run).
    pub open_tickets: Vec<String>,
    /// The run's durable verification records.
    pub verifications: Vec<VerificationFact>,
    /// The run's durable settlement rows.
    pub settlements: Vec<SettlementFact>,
    /// Budget spent at the latest checkpoint (µUSD as the integer).
    pub budget_spent: Option<BudgetSpentFacts>,
}

/// The checkpoint's `budget_spent` facts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BudgetSpentFacts {
    pub rounds: u32,
    pub wall_seconds: u64,
    pub tokens: u64,
    pub cost_usd_micro: u64,
}

impl RunView {
    /// The `GET /api/v1/runs/{id}` response body.
    pub fn to_json(&self, run: &gauntlet_domain::RunId) -> JsonValue {
        JsonValue::Object(vec![
            ("run_id".to_owned(), JsonValue::Str(run.to_string())),
            (
                "iteration".to_owned(),
                match self.iteration {
                    Some(iteration) => {
                        JsonValue::Number(gauntlet_ports::json::JsonNumber::Uint(iteration as u64))
                    }
                    None => JsonValue::Null,
                },
            ),
            ("units".to_owned(), crate::units_json(&self.units)),
            (
                "open_tickets".to_owned(),
                JsonValue::Array(
                    self.open_tickets
                        .iter()
                        .map(|ticket| JsonValue::Str(ticket.clone()))
                        .collect(),
                ),
            ),
            (
                "verifications".to_owned(),
                JsonValue::Array(
                    self.verifications
                        .iter()
                        .map(|fact| fact.to_json())
                        .collect(),
                ),
            ),
            (
                "settlements".to_owned(),
                JsonValue::Array(self.settlements.iter().map(|fact| fact.to_json()).collect()),
            ),
            (
                "budget_spent".to_owned(),
                match self.budget_spent {
                    Some(spent) => JsonValue::Object(vec![
                        (
                            "rounds".to_owned(),
                            JsonValue::Number(gauntlet_ports::json::JsonNumber::Uint(
                                spent.rounds as u64,
                            )),
                        ),
                        (
                            "wall_seconds".to_owned(),
                            JsonValue::Number(gauntlet_ports::json::JsonNumber::Uint(
                                spent.wall_seconds,
                            )),
                        ),
                        (
                            "tokens".to_owned(),
                            JsonValue::Number(gauntlet_ports::json::JsonNumber::Uint(spent.tokens)),
                        ),
                        (
                            "cost_usd_micro".to_owned(),
                            JsonValue::Number(gauntlet_ports::json::JsonNumber::Uint(
                                spent.cost_usd_micro,
                            )),
                        ),
                    ]),
                    None => JsonValue::Null,
                },
            ),
        ])
    }
}

/// One unit's view (`GET /api/v1/units/{id}`): the unit-scoped durable
/// facts across the run-anchored reads.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct UnitView {
    /// The unit's frozen tier + durable status.
    pub unit: Option<gauntlet_ports::operator_surface::UnitSnapshot>,
    /// The unit's durable verification records.
    pub verifications: Vec<VerificationFact>,
    /// Whether a durable settlement row exists for the unit.
    pub settled: bool,
}

impl UnitView {
    /// The `GET /api/v1/units/{id}` response body.
    pub fn to_json(&self, unit: &gauntlet_domain::UnitId) -> JsonValue {
        JsonValue::Object(vec![
            ("unit_id".to_owned(), JsonValue::Str(unit.to_string())),
            (
                "unit".to_owned(),
                match &self.unit {
                    Some(snapshot) => unit_json(snapshot),
                    None => JsonValue::Null,
                },
            ),
            (
                "verifications".to_owned(),
                JsonValue::Array(
                    self.verifications
                        .iter()
                        .map(|fact| fact.to_json())
                        .collect(),
                ),
            ),
            ("settled".to_owned(), JsonValue::Bool(self.settled)),
        ])
    }
}

/// The run-scoped reads (charter D6). Implemented by the composition
/// root over the same durable reads the status/inspect commands use.
pub trait RunScopedReader {
    /// The run-scoped view; `NotFound` is a typed 404 at the HTTP layer.
    fn run_view(
        &self,
        run: &gauntlet_domain::RunId,
        deadline: Deadline,
    ) -> Result<RunView, RunScopedError>;

    /// The unit-scoped view, resolved across the given (derived or
    /// `?run=`-given) run.
    fn unit_view(
        &self,
        run: &gauntlet_domain::RunId,
        unit: &gauntlet_domain::UnitId,
        deadline: Deadline,
    ) -> Result<UnitView, RunScopedError>;
}

/// Why a run-scoped read failed — the HTTP layer maps these to status
/// codes: `NotFound` → 404, `ReadFailed` → 500.
#[derive(Debug, Clone, PartialEq)]
pub enum RunScopedError {
    /// The run (or the unit within it) is not durable in this store.
    NotFound { detail: String },
    /// A durable read failed.
    ReadFailed { detail: String },
}

impl From<OperatorSurfaceError> for RunScopedError {
    fn from(error: OperatorSurfaceError) -> Self {
        Self::ReadFailed {
            detail: error.to_string(),
        }
    }
}

impl core::fmt::Display for RunScopedError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NotFound { detail } => write!(f, "not found in the durable reads: {detail}"),
            Self::ReadFailed { detail } => write!(f, "the durable read failed: {detail}"),
        }
    }
}

impl std::error::Error for RunScopedError {}
