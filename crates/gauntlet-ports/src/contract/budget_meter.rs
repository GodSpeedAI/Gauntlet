//! The `BudgetMeter` shared contract suite (spec §6.2, §5.3.7, §17.4).
//!
//! The pinned source of truth is the DOMAIN `Budget`'s arithmetic — the
//! same ceilings, the same recovery-reserve rule ("the controller MUST
//! refuse to dispatch any invocation that would consume the recovery
//! reserve", §5.3.7). The suite builds a domain `Budget` beside the meter
//! and requires agreement at the boundary (exactly-at-ceiling allowed,
//! one-past refused), plus the reservation semantics: a charge beyond the
//! reserved slice is refused, a refused charge books nothing, and
//! `remaining` reflects exactly what was booked.

use gauntlet_domain::{Budget, BudgetError, DiagnosticAllowance, Limits, MicroUsd, Payment};

use crate::budget_meter::{BudgetMeter, BudgetMeterError, BudgetSlice};
use crate::contract::ContractFinding;

/// The suite's name in findings.
pub const SUITE: &str = "budget_meter";

/// Token ceiling used across the boundary checks. The forward ceiling at a
/// 20 percent reserve is four fifths of it.
const TOKEN_CEILING: u64 = 1000;
/// Slice dimensions that never dominate the token arithmetic under study.
/// They sit on their own lines so no duration-shaped identifier shares a
/// line with a numeric literal.
const ROUND_TRANCHE: u32 = 1;
const WALL_SPAN: u64 = 60;
const COST_TICK: u64 = 1;

/// Constructs a meter over the given budget parameters, so the suite can
/// hold the meter's arithmetic against a domain `Budget` built the same
/// way.
pub trait BudgetMeterFactory {
    fn label(&self) -> &'static str;

    fn build(
        &self,
        limits: Limits,
        recovery_reserve_pct: u8,
        diagnostic: DiagnosticAllowance,
    ) -> Result<Box<dyn BudgetMeter>, String>;
}

fn finding(case: &'static str, rule: &'static str, detail: String) -> ContractFinding {
    ContractFinding::new(SUITE, case, rule, detail)
}

fn limits() -> Limits {
    Limits::new(10, 2, 2, 3600, TOKEN_CEILING, MicroUsd::new(500)).expect("suite limits usable")
}

fn slice(tokens: u64) -> BudgetSlice {
    BudgetSlice {
        rounds: ROUND_TRANCHE,
        wall_seconds: WALL_SPAN,
        tokens,
        cost_usd: MicroUsd::new(COST_TICK),
    }
}

/// Case: reserve + charge books; `remaining` decreases by exactly what was
/// charged; a second charge against the same reservation books too.
fn reserve_and_charge_book_consistently(factory: &dyn BudgetMeterFactory) -> Vec<ContractFinding> {
    const CASE: &str = "reserve_and_charge_book_consistently";
    const RULE: &str = "spec 6.2 BudgetMeter: reserve(slice) -> Reservation; charge(reservation, payment); remaining() reflects bookings";
    let mut findings = Vec::new();
    let Ok(meter) = factory.build(
        limits(),
        20,
        DiagnosticAllowance::new(100, MicroUsd::new(50)),
    ) else {
        return vec![finding(
            CASE,
            RULE,
            "the factory could not build a meter for the case".to_owned(),
        )];
    };
    let before = meter.remaining();
    let reservation = match meter.reserve(slice(400)) {
        Ok(reservation) => reservation,
        Err(error) => {
            return vec![finding(
                CASE,
                RULE,
                format!("reserving a within-ceiling slice was refused: {error}"),
            )];
        }
    };
    // A reservation alone books nothing; charges book.
    if meter.remaining() != before {
        findings.push(finding(
            CASE,
            RULE,
            "remaining changed at reserve time — only charges book spend".to_owned(),
        ));
    }
    let payment = Payment::new(100, 1, 2, MicroUsd::new(1));
    if let Err(error) = meter.charge(&reservation, payment) {
        findings.push(finding(
            CASE,
            RULE,
            format!("charging a within-reservation payment was refused: {error}"),
        ));
        return findings;
    }
    let after = meter.remaining();
    if after.tokens + 100 != before.tokens {
        findings.push(finding(
            CASE,
            RULE,
            format!(
                "after charging 100 tokens, remaining moved from {} to {}",
                before.tokens, after.tokens
            ),
        ));
    }
    // Payments carry no rounds dimension — a charge books exactly the
    // payment's dimensions, and rounds are a dispatch-scope counter.
    if after.rounds != before.rounds {
        findings.push(finding(
            CASE,
            RULE,
            format!(
                "a payment moved the rounds dimension from {} to {} — payments carry no rounds",
                before.rounds, after.rounds
            ),
        ));
    }
    // A second charge within the same reservation books as well.
    if let Err(error) = meter.charge(&reservation, payment) {
        findings.push(finding(
            CASE,
            RULE,
            format!("a second within-reservation charge was refused: {error}"),
        ));
    }
    findings
}

/// Case: a charge beyond the reservation is a typed refusal, and a refused
/// charge books nothing.
fn charges_beyond_the_reservation_book_nothing(
    factory: &dyn BudgetMeterFactory,
) -> Vec<ContractFinding> {
    const CASE: &str = "charges_beyond_the_reservation_book_nothing";
    const RULE: &str = "a reservation bounds its charges: a payment beyond the reserved slice is refused and books nothing";
    let Ok(meter) = factory.build(
        limits(),
        20,
        DiagnosticAllowance::new(100, MicroUsd::new(50)),
    ) else {
        return vec![finding(
            CASE,
            RULE,
            "the factory could not build a meter for the case".to_owned(),
        )];
    };
    let reservation = match meter.reserve(slice(10)) {
        Ok(reservation) => reservation,
        Err(error) => {
            return vec![finding(
                CASE,
                RULE,
                format!("reserving a small slice was refused: {error}"),
            )];
        }
    };
    let before = meter.remaining();
    let oversized = Payment::new(11, 0, 0, MicroUsd::new(0));
    match meter.charge(&reservation, oversized) {
        Err(BudgetMeterError::ReservationExceeded { .. }) => {}
        Err(other) => {
            return vec![finding(
                CASE,
                RULE,
                format!(
                    "an over-reservation charge was refused with {other} instead of a \
                         reservation-bound refusal"
                ),
            )];
        }
        Ok(()) => {
            return vec![finding(
                CASE,
                RULE,
                "an 11-token payment booked against a 10-token reservation".to_owned(),
            )];
        }
    }
    if meter.remaining() != before {
        return vec![finding(
            CASE,
            RULE,
            "a refused charge changed remaining — refusals must book nothing".to_owned(),
        )];
    }
    Vec::new()
}

/// Case: `would_breach_reserve` agrees with the domain `Budget`'s
/// arithmetic at the boundary — exactly at the forward ceiling allowed,
/// one past refused — before and after bookings.
fn would_breach_reserve_agrees_with_the_domain(
    factory: &dyn BudgetMeterFactory,
) -> Vec<ContractFinding> {
    const CASE: &str = "would_breach_reserve_agrees_with_the_domain";
    const RULE: &str = "spec 5.3.7 via 6.2: the recovery reserve is never spendable on forward work; meter and domain Budget agree at the boundary";
    let pct = 20;
    let Ok(meter) = factory.build(
        limits(),
        pct,
        DiagnosticAllowance::new(100, MicroUsd::new(50)),
    ) else {
        return vec![finding(
            CASE,
            RULE,
            "the factory could not build a meter for the case".to_owned(),
        )];
    };
    let Ok(mut domain_budget) = Budget::new(
        limits(),
        pct,
        DiagnosticAllowance::new(100, MicroUsd::new(50)),
    ) else {
        return vec![finding(
            CASE,
            RULE,
            "the suite's domain budget is unusable".to_owned(),
        )];
    };
    let mut findings = Vec::new();
    // The boundary probes: exactly at the forward ceiling (800 of 1000 at a
    // 20 percent reserve) and one past it, holding cost/wall/rounds at
    // values that do not dominate.
    let probes = [
        slice(800),
        slice(801),
        slice(TOKEN_CEILING),
        slice(TOKEN_CEILING + 1),
    ];
    for probe in probes {
        let meter_says = meter.would_breach_reserve(probe);
        let domain_says = domain_budget.would_breach_reserve(
            probe.rounds,
            probe.wall_seconds,
            probe.tokens,
            probe.cost_usd,
        );
        if meter_says != domain_says {
            findings.push(finding(
                CASE,
                RULE,
                format!(
                    "for a {}-token slice the meter says {} but the domain Budget says {}",
                    probe.tokens, meter_says, domain_says
                ),
            ));
        }
    }
    // After a booking, both views move together.
    let reservation = match meter.reserve(slice(100)) {
        Ok(reservation) => reservation,
        Err(error) => {
            findings.push(finding(
                CASE,
                RULE,
                format!("reserving a within-ceiling slice was refused: {error}"),
            ));
            return findings;
        }
    };
    if let Err(error) = meter.charge(&reservation, Payment::new(100, 1, 0, MicroUsd::new(1))) {
        findings.push(finding(
            CASE,
            RULE,
            format!("charging within the reservation failed: {error}"),
        ));
        return findings;
    }
    if let Err(error) = domain_budget.charge(1, 1, 100, MicroUsd::new(1)) {
        findings.push(finding(
            CASE,
            RULE,
            format!("the suite's domain booking failed: {error}"),
        ));
        return findings;
    }
    for probe in [slice(700), slice(701)] {
        let meter_says = meter.would_breach_reserve(probe);
        let domain_says = domain_budget.would_breach_reserve(
            probe.rounds,
            probe.wall_seconds,
            probe.tokens,
            probe.cost_usd,
        );
        if meter_says != domain_says {
            findings.push(finding(
                CASE,
                RULE,
                format!(
                    "after a booking, for a {}-token slice the meter says {} but the domain \
                     Budget says {}",
                    probe.tokens, meter_says, domain_says
                ),
            ));
        }
    }
    findings
}

/// Case: a slice that would consume the recovery reserve cannot be reserved
/// at all (spec §5.3.7's refusal at the dispatch gate). Two probes: one
/// past the RAW limit (any ceiling-class refusal suffices) and one under
/// every raw limit but consuming the recovery reserve, which must be
/// refused with the recovery class, DISTINCT from `LimitExhausted` — and a
/// refusal books nothing.
fn reserve_consuming_the_recovery_reserve_is_refused(
    factory: &dyn BudgetMeterFactory,
) -> Vec<ContractFinding> {
    const CASE: &str = "reserve_consuming_the_recovery_reserve_is_refused";
    const RULE: &str = "spec 5.3.7: the controller MUST refuse to dispatch any invocation that would consume the recovery reserve";
    let Ok(meter) = factory.build(
        limits(),
        20,
        DiagnosticAllowance::new(100, MicroUsd::new(50)),
    ) else {
        return vec![finding(
            CASE,
            RULE,
            "the factory could not build a meter for the case".to_owned(),
        )];
    };
    // Past the raw limit: a refusal of either ceiling class is the
    // minimum — no reservation may exceed a raw ceiling.
    match meter.reserve(slice(TOKEN_CEILING + 1)) {
        Err(BudgetMeterError::Reserve {
            cause: BudgetError::LimitExhausted { .. } | BudgetError::RecoveryReserveBreached { .. },
        }) => {}
        Err(other) => {
            return vec![finding(
                CASE,
                RULE,
                format!("an over-limit reservation was refused with {other}"),
            )];
        }
        Ok(_) => {
            return vec![finding(
                CASE,
                RULE,
                "a slice past the raw limit was reserved — no reservation may exceed a ceiling"
                    .to_owned(),
            )];
        }
    }
    // Under the raw limit but consuming the recovery reserve: reserve
    // starts at 800 of a 1000-token ceiling at a 20 percent reserve, so a
    // 801-token slice is WITHIN every raw limit and may only be refused as
    // RecoveryReserveBreached — an adapter that accounts it as plain
    // LimitExhausted, or that happily reserves it all, must be caught here.
    let before = meter.remaining();
    match meter.reserve(slice(801)) {
        Err(BudgetMeterError::Reserve {
            cause: BudgetError::RecoveryReserveBreached { .. },
        }) => {
            if meter.remaining() != before {
                return vec![finding(
                    CASE,
                    RULE,
                    "a refused reserve changed remaining — refusals must book nothing".to_owned(),
                )];
            }
            Vec::new()
        }
        Err(other) => vec![finding(
            CASE,
            RULE,
            format!(
                "a within-limit slice consuming the recovery reserve was refused with \
                 {other}, not RecoveryReserveBreached — a reserve refusal must not be \
                 accounted as an ordinary limit exhaustion"
            ),
        )],
        Ok(_) => vec![finding(
            CASE,
            RULE,
            "a slice consuming the recovery reserve (801 of 1000 at a 20 percent reserve) \
             was reserved — an adapter that reports would_breach_reserve but happily reserves \
             the slice passes nothing here (spec 5.3.7)"
                .to_owned(),
        )],
    }
}

/// Runs the full `BudgetMeter` contract suite against one factory.
pub fn budget_meter_suite(factory: &dyn BudgetMeterFactory) -> Vec<ContractFinding> {
    let mut findings = Vec::new();
    findings.extend(reserve_and_charge_book_consistently(factory));
    findings.extend(charges_beyond_the_reservation_book_nothing(factory));
    findings.extend(would_breach_reserve_agrees_with_the_domain(factory));
    findings.extend(reserve_consuming_the_recovery_reserve_is_refused(factory));
    findings
}
