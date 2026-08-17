//! The `OperatorSurface` shared contract suite (spec §6.2; §20.2).
//!
//! Pinned semantics: the snapshot is the operator's projection of the
//! runtime state — nothing the runtime holds is dropped, nothing absent is
//! invented; and the actions the surface offers cover the §20.2 endpoints
//! (refresh, unit resolve, unit retry) for the units it shows.

use gauntlet_domain::{BlastRadius, InvocationId, MicroUsd, ResidualId, RunId, UnitId, UnitStatus};

use crate::budget_meter::Remaining;
use crate::contract::ContractFinding;
use crate::deadline::Deadline;
use crate::human_escalation::TicketId;
use crate::operator_surface::{OperatorAction, OperatorSurface, RuntimeSnapshot, UnitSnapshot};

/// The suite's name in findings.
pub const SUITE: &str = "operator_surface";

const BASE_INSTANT: u64 = 1_700_000_000;
/// The suite's bound span, on its own line (numeric literals never share a
/// line with a duration-shaped identifier).
const BOUND_SPAN: u64 = 60;

/// Constructs the surface under test over the given runtime state.
/// `seeded` is what the runtime currently holds; the adapter's snapshot
/// must reproduce it exactly (projection, not invention).
pub trait OperatorSurfaceFactory {
    fn label(&self) -> &'static str;

    fn build(&self, seeded: RuntimeSnapshot) -> Result<Box<dyn OperatorSurface>, String>;
}

fn finding(case: &'static str, rule: &'static str, detail: String) -> ContractFinding {
    ContractFinding::new(SUITE, case, rule, detail)
}

fn bound() -> Deadline {
    let instant = gauntlet_domain::EpochSeconds::new(BASE_INSTANT + BOUND_SPAN);
    Deadline::at_epoch(instant)
}

/// Seeded budget figures, referenced not written (numeric literals never
/// share a line with a duration-shaped identifier).
const SEED_ROUNDS: u32 = 4;
const SEED_PARALLEL: u32 = 2;
const SEED_WALL_SPAN: u64 = BOUND_SPAN * 4;
const SEED_TOKENS: u64 = 640;
const SEED_COST_USD: u64 = 200;

/// A canonical runtime state: every §20.2 section populated.
fn seeded() -> RuntimeSnapshot {
    RuntimeSnapshot {
        run: Some(RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("suite run id is a ULID")),
        escalated: vec![
            TicketId::new("suite-ticket-01").expect("suite ticket id"),
            TicketId::new("suite-ticket-02").expect("suite ticket id"),
        ],
        in_flight: vec![
            InvocationId::new("01ARZ3NDEKTSV4RRFFQ69G5FAW").expect("suite invocation id"),
        ],
        units: vec![
            UnitSnapshot {
                id: UnitId::new("AK-001").expect("suite unit id"),
                status: UnitStatus::Open,
                tier: BlastRadius::Contained,
            },
            UnitSnapshot {
                id: UnitId::new("AK-002").expect("suite unit id"),
                status: UnitStatus::Settled,
                tier: BlastRadius::Local,
            },
        ],
        budget: Remaining {
            rounds: SEED_ROUNDS,
            parallel: SEED_PARALLEL,
            depth: 1,
            wall_seconds: SEED_WALL_SPAN,
            tokens: SEED_TOKENS,
            cost_usd: MicroUsd::new(SEED_COST_USD),
        },
        sandbox_denials: vec!["deny recorded for a local write".to_owned()],
        open_residuals: vec![ResidualId::new("res-000042").expect("suite residual id")],
    }
}

/// Case: the snapshot reproduces the runtime state exactly — every seeded
/// escalation, invocation, unit, denial, and residual is visible; nothing
/// is dropped or invented (§20.2: the operator's actionable inbox is the
/// whole point of the surface).
fn snapshot_echoes_the_runtime_state(factory: &dyn OperatorSurfaceFactory) -> Vec<ContractFinding> {
    const CASE: &str = "snapshot_echoes_the_runtime_state";
    const RULE: &str = "spec 20.2: the snapshot is the operator's projection of the runtime state — full echo, no invention";
    let seeded = seeded();
    let Ok(surface) = factory.build(seeded.clone()) else {
        return vec![finding(
            CASE,
            RULE,
            "the factory could not build a surface for the case".to_owned(),
        )];
    };
    match surface.snapshot(bound()) {
        Ok(snapshot) if snapshot == seeded => Vec::new(),
        Ok(snapshot) => vec![finding(
            CASE,
            RULE,
            format!(
                "the snapshot dropped or invented state: {} escalated tickets seeded vs {} shown; {} units seeded vs {} shown",
                seeded.escalated.len(),
                snapshot.escalated.len(),
                seeded.units.len(),
                snapshot.units.len()
            ),
        )],
        Err(error) => vec![finding(
            CASE,
            RULE,
            format!("assembling the snapshot failed: {error}"),
        )],
    }
}

/// Case: the actions offered cover the §20.2 endpoints — refresh, and
/// resolve/retry for a unit the snapshot shows (§20.2: the operator acts
/// through the same atomic commit sequence as the automatic paths).
fn actions_offer_the_2022_endpoints(factory: &dyn OperatorSurfaceFactory) -> Vec<ContractFinding> {
    const CASE: &str = "actions_offer_the_2022_endpoints";
    const RULE: &str =
        "spec 20.2: the surface offers refresh, unit resolve, and unit retry endpoints";
    let seeded = seeded();
    let unit = UnitId::new("AK-001").expect("suite unit id");
    let Ok(surface) = factory.build(seeded) else {
        return vec![finding(
            CASE,
            RULE,
            "the factory could not build a surface for the case".to_owned(),
        )];
    };
    let actions = surface.actions();
    let mut findings = Vec::new();
    if !actions.contains(&OperatorAction::Refresh) {
        findings.push(finding(
            CASE,
            RULE,
            "the refresh endpoint is missing from actions()".to_owned(),
        ));
    }
    if !actions.contains(&OperatorAction::ResolveUnit { unit: unit.clone() }) {
        findings.push(finding(
            CASE,
            RULE,
            format!("resolve is not offered for unit {unit} shown in the snapshot"),
        ));
    }
    if !actions
        .iter()
        .any(|action| matches!(action, OperatorAction::RetryUnit { unit: u } if u == &unit))
    {
        findings.push(finding(
            CASE,
            RULE,
            format!("retry is not offered for unit {unit} shown in the snapshot"),
        ));
    }
    findings
}

/// Runs the full `OperatorSurface` contract suite against one factory.
pub fn operator_surface_suite(factory: &dyn OperatorSurfaceFactory) -> Vec<ContractFinding> {
    let mut findings = Vec::new();
    findings.extend(snapshot_echoes_the_runtime_state(factory));
    findings.extend(actions_offer_the_2022_endpoints(factory));
    findings
}
