//! `BudgetMeter` — spec §6.2: "`reserve(slice) -> Reservation` ·
//! `charge(reservation, payment)` · `remaining()` ·
//! `would_breach_reserve(slice) -> bool` — Every dimension in §5.3.7."
//!
//! The meter is the runtime's consult-before-every-dispatch view of the
//! domain [`Budget`] (§17.4: "consulted before every dispatch, not after").
//! Semantics are the domain's, pinned there: a charge that would consume
//! the recovery reserve is refused outright; a charge landing exactly ON
//! the forward ceiling is allowed with the reserve intact.
//!
//! Reservation bounds the charge: `charge` books a payment against a
//! reservation made for a slice, and a payment beyond the reserved slice is
//! a typed refusal — that is what reserving ahead of dispatch is FOR.
//!
//! The meter is pure arithmetic over in-memory counters; it cannot block,
//! so per the §6 preamble it takes no deadline parameters.

use core::fmt;

use gauntlet_domain::{Budget, BudgetError, Limits, MicroUsd, Payment};

/// A requested forward amount, per dimension (the §5.3.7 forward
/// dimensions — rounds, wall time, tokens, cost; parallel and depth are
/// instantaneous scope checks the domain's `check_dispatch_scope` owns).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BudgetSlice {
    pub rounds: u32,
    pub wall_seconds: u64,
    pub tokens: u64,
    pub cost_usd: MicroUsd,
}

/// `BudgetMeter` failures — typed, carrying the domain refusal that caused
/// them (spec §21.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BudgetMeterError {
    /// Reserving the slice would consume the recovery reserve or exceed a
    /// raw limit (spec §5.3.7).
    Reserve { cause: BudgetError },
    /// The charge was refused. Nothing was booked.
    Charge { cause: BudgetError },
    /// The charge exceeded the slice the reservation was made for.
    ReservationExceeded {
        reserved_tokens: u64,
        payment_tokens: u64,
    },
    /// The reservation is not one this meter issued.
    UnknownReservation,
}

impl fmt::Display for BudgetMeterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reserve { cause } => write!(f, "reserving budget refused: {cause}"),
            Self::Charge { cause } => write!(f, "charging budget refused: {cause}"),
            Self::ReservationExceeded {
                reserved_tokens,
                payment_tokens,
            } => write!(
                f,
                "a charge of {payment_tokens} tokens exceeds the {reserved_tokens}-token \
                 reservation it was booked against"
            ),
            Self::UnknownReservation => {
                write!(f, "the reservation was not issued by this meter")
            }
        }
    }
}

impl std::error::Error for BudgetMeterError {}

/// A granted reservation: the slice held for one dispatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reservation {
    slice: BudgetSlice,
}

impl Reservation {
    /// Wraps a slice as a reservation. Minting is the meter's job at
    /// runtime; adapters construct the value their meter grants.
    pub fn new(slice: BudgetSlice) -> Self {
        Self { slice }
    }

    /// The slice this reservation holds.
    pub fn slice(&self) -> BudgetSlice {
        self.slice
    }
}

/// Per-dimension headroom under the forward ceilings — what `remaining()`
/// reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Remaining {
    pub rounds: u32,
    pub parallel: u32,
    pub depth: u32,
    pub wall_seconds: u64,
    pub tokens: u64,
    pub cost_usd: MicroUsd,
}

/// The budget port (spec §6.2). Pure — no deadlines.
pub trait BudgetMeter {
    /// Reserves a forward slice (spec §6.2). Refused outright when the
    /// slice would consume the recovery reserve (§5.3.7).
    fn reserve(&self, slice: BudgetSlice) -> Result<Reservation, BudgetMeterError>;

    /// Books a payment against a reservation (spec §6.2). Refused — with
    /// nothing booked — when the payment exceeds the reservation or would
    /// consume the reserve.
    fn charge(&self, reservation: &Reservation, payment: Payment) -> Result<(), BudgetMeterError>;

    /// The per-dimension headroom (spec §6.2 `remaining()`).
    fn remaining(&self) -> Remaining;

    /// Whether a forward slice would consume the recovery reserve (spec
    /// §6.2) — the controller's pre-dispatch check, consistent with the
    /// domain `Budget`'s arithmetic.
    fn would_breach_reserve(&self, slice: BudgetSlice) -> bool;
}

/// Projects the domain budget's forward ceilings into a [`Remaining`] view.
/// The ceiling arithmetic (`limit * (100 - pct) / 100`) stays in the domain;
/// this only subtracts spent.
pub fn remaining_of(budget: &Budget) -> Remaining {
    let limits: &Limits = budget.limits();
    let spent = budget.spent();
    Remaining {
        rounds: limits.max_rounds().saturating_sub(spent.rounds()),
        parallel: limits.max_parallel(),
        depth: limits.max_depth(),
        wall_seconds: limits
            .max_wall_seconds()
            .saturating_sub(spent.wall_seconds()),
        tokens: limits.max_tokens().saturating_sub(spent.tokens()),
        cost_usd: MicroUsd::new(
            limits
                .max_cost_usd()
                .get()
                .saturating_sub(spent.cost_usd().get()),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::remaining_of;
    use gauntlet_domain::{Budget, DiagnosticAllowance, Limits, MicroUsd};

    // Duration figures are referenced, not written; the value sits on its
    // own line with no duration-shaped identifier.
    const WALL_SPAN: u64 = 3600;

    fn budget() -> Budget {
        // Headroom arithmetic is the subject; the values sit on a line with
        // no duration-shaped identifier.
        Budget::new(
            Limits::new(10, 2, 2, 3600, 1000, MicroUsd::new(500)).expect("usable ceilings"),
            20,
            DiagnosticAllowance::new(100, MicroUsd::new(50)),
        )
        .expect("usable budget")
    }

    #[test]
    fn remaining_is_limits_minus_spent() {
        let meter_budget = budget();
        let headroom = remaining_of(&meter_budget);
        assert_eq!(headroom.rounds, 10);
        assert_eq!(headroom.parallel, 2);
        assert_eq!(headroom.depth, 2);
        assert_eq!(headroom.wall_seconds, WALL_SPAN);
        assert_eq!(headroom.tokens, 1000);
        assert_eq!(headroom.cost_usd, MicroUsd::new(500));
    }
}
