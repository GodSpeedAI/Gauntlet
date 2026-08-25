//! BudgetMeter adapter (spec §6.2, §5.3.7, §17.4; plan Task 12 step 3):
//! the runtime's consult-before-every-dispatch view of a run's
//! [`Budget`], with reservation bookkeeping beside it.
//!
//! ## The meter/rounds split (Task 12 correction C2)
//!
//! The PORT expresses PAYMENTS (§5.3.4: tokens, wall seconds, tool calls,
//! cost) — payments carry NO rounds dimension, and this adapter books
//! none. ROUNDS are ITERATION FACTS (§5.3.7 `max_rounds` counts
//! iterations per run): the run controller books one round per iteration
//! on the run `Budget` at the iteration boundary, and that run budget is
//! the SINGLE rounds authority (the rounds gates read its domain
//! arithmetic, never this meter's payment counters). No shipped
//! composition books rounds through the meter, by design.
//!
//! Semantics are the DOMAIN's, and this adapter owns none of the
//! arithmetic: `would_breach_reserve`, the forward ceilings, and every
//! refusal cause come from the domain `Budget` (`reserve` refuses exactly
//! what the domain's charge would refuse, so there is no second
//! implementation to drift), and `remaining` is the port's own projection
//! of the same counters. A charge books through `Budget::charge`; a charge
//! landing exactly ON the forward ceiling books with the reserve intact,
//! one past it is refused — the domain's boundary, not a local copy.
//!
//! Reservation bookkeeping (§17.4 dispatch scope): each outstanding
//! reservation holds one `max_parallel` slot; `reserve` refuses a further
//! slot at the ceiling with the domain's own scope refusal. The release
//! path is the CHARGE: a successful `charge` — including the documented
//! zero-payment charge (`Payment::zero()` books nothing and releases the
//! slot) for cancelled or failed invocations — returns the slot. A refused
//! charge holds it (the dispatch still owns the reservation it was made
//! for). A repeated charge against one reservation therefore books both
//! payments (the port's reservation bounds charges; it does not expire on
//! first use) but only ever released one slot — the counter saturates at
//! zero rather than underflowing.
//!
//! Every admitted reservation is COUNTED ([`Self::admitted_reservations_total`])
//! — AK-052's rows-vs-reservations discipline reads the meter's own
//! truth, not a caller-side tally.
//!
//! The port's `Reservation` is constructible only through the
//! [`BudgetMeter::mint`](gauntlet_ports::budget_meter::BudgetMeter::mint)
//! capability (correction C9: minting requires a meter; arbitrary code
//! cannot forge the parallelism-accounting token), so
//! `UnknownReservation` is not detectable here: a reservation this meter
//! did not issue is indistinguishable from one it did. The bound that
//! matters — nothing books without passing the domain's charge — does not
//! depend on reservation identity.
//!
//! §17.4 also names tool-invocation and retry dimensions. The domain
//! `Limits` (§5.3.7) carries no tool-call or retry ceilings and the port's
//! `Remaining` shape has no fields for them, so this adapter TRACKS both
//! as adapter-level counters ([`Self::tool_calls_total`],
//! [`Self::note_retry`]/[`Self::retries_total`]) and enforces nothing
//! there — the missing ceilings are a recorded residual for the ports
//! revision (plan Task 12 report), not something a meter adapter may
//! invent locally.
//!
//! Thread safety is `Arc<Mutex<…>>` over the inner state; the port takes
//! `&self`, every method locks once, and there is no reentrancy (the
//! domain calls are plain arithmetic on the guarded budget).

use std::sync::{Arc, Mutex};

use gauntlet_domain::{Budget, BudgetError, Payment};
use gauntlet_ports::budget_meter::{
    BudgetMeter, BudgetMeterError, BudgetSlice, Remaining, Reservation, remaining_of,
};

/// The metered run budget: the domain [`Budget`] plus reservation
/// bookkeeping. Constructed at the composition root from the run's budget
/// (the §8.6 defaults table is configuration, not this adapter's concern).
#[derive(Debug)]
pub struct MeteredBudget {
    inner: Arc<Mutex<Inner>>,
}

#[derive(Debug)]
struct Inner {
    budget: Budget,
    /// Outstanding reservations — each holds one `max_parallel` slot
    /// (§17.4). Saturating release; see the module doc.
    outstanding: u32,
    /// Total reservations this meter ADMITTED (AK-052's rows-vs-
    /// reservations truth: every recorded invocation row must have one).
    admitted_reservations: u32,
    /// Adapter-level §17.4 counters with no domain ceiling and no port
    /// field: tool invocations booked through charges, and retries noted
    /// by the controller. Tracked, never enforced (recorded residual).
    tool_calls_total: u64,
    retries_total: u64,
}

impl MeteredBudget {
    /// Wraps an assembled domain budget. The budget's own constructor has
    /// already refused zero ceilings and out-of-range reserves; nothing is
    /// re-validated here because there is no second arithmetic to trust.
    pub fn new(budget: Budget) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner {
                budget,
                outstanding: 0,
                admitted_reservations: 0,
                tool_calls_total: 0,
                retries_total: 0,
            })),
        }
    }

    /// Total reservations this meter admitted (AK-052): the meter's own
    /// rows-vs-reservations truth — every durable invocation row a
    /// composition records must be matched by one admitted reservation.
    pub fn admitted_reservations_total(&self) -> u32 {
        self.lock().admitted_reservations
    }

    /// The cumulative tool-invocation total booked through charges (§17.4).
    /// No ceiling exists to enforce — see the module doc.
    pub fn tool_calls_total(&self) -> u64 {
        self.lock().tool_calls_total
    }

    /// Records one retry (§17.4) — the controller's note; the meter only
    /// counts. No ceiling exists to enforce — see the module doc.
    pub fn note_retry(&self) {
        self.lock().retries_total += 1;
    }

    /// The retry total recorded so far (§17.4).
    pub fn retries_total(&self) -> u64 {
        self.lock().retries_total
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().expect("the meter is not poisoned")
    }

    /// Derives the typed refusal cause for a slice the domain says would
    /// breach: a dry charge over a CLONE of the budget (never the guarded
    /// one — a probe must not book). `would_breach_reserve` is exactly the
    /// charge predicate over the four forward dimensions, so the clone's
    /// refusal is the cause to surface; the unreachable `Ok` arm stays a
    /// typed refusal rather than an unwrap.
    fn breach_cause(budget: &Budget, slice: BudgetSlice) -> BudgetMeterError {
        let mut probe = budget.clone();
        match probe.charge(
            slice.rounds,
            slice.wall_seconds,
            slice.tokens,
            slice.cost_usd,
        ) {
            Err(cause) => BudgetMeterError::Reserve { cause },
            Ok(()) => BudgetMeterError::Reserve {
                cause: BudgetError::LimitExhausted {
                    dimension: gauntlet_domain::BudgetDimension::Tokens,
                    limit: budget.limits().max_tokens(),
                    spent: budget.spent().tokens(),
                    requested: slice.tokens,
                },
            },
        }
    }
}

impl BudgetMeter for MeteredBudget {
    fn reserve(&self, slice: BudgetSlice) -> Result<Reservation, BudgetMeterError> {
        let mut inner = self.lock();
        // The §5.3.7 gate first: a slice that would consume the recovery
        // reserve (or exceed a raw ceiling) is refused with the DOMAIN's
        // typed cause — the same formula the charge path enforces.
        if inner.budget.would_breach_reserve(
            slice.rounds,
            slice.wall_seconds,
            slice.tokens,
            slice.cost_usd,
        ) {
            return Err(Self::breach_cause(&inner.budget, slice));
        }
        // Dispatch scope second: each outstanding reservation holds one
        // parallel slot (§17.4); the domain's own scope check refuses the
        // ceiling with its own error, never a local re-implementation.
        // Depth stays 1 here — a reservation carries no depth; depth is
        // the controller's dispatch-shape decision against `max_depth`.
        let parallel = inner.outstanding.saturating_add(1);
        inner
            .budget
            .check_dispatch_scope(parallel, 1)
            .map_err(|cause| BudgetMeterError::Reserve { cause })?;
        inner.outstanding = parallel;
        inner.admitted_reservations = inner.admitted_reservations.saturating_add(1);
        Ok(self.mint(slice))
    }

    fn charge(&self, reservation: &Reservation, payment: Payment) -> Result<(), BudgetMeterError> {
        let mut inner = self.lock();
        // The reservation bounds its charges where the port can express the
        // bound: the token dimension (the port's refusal carries token
        // fields). Wall/cost ride the domain's cumulative arithmetic — a
        // payment there books only as far as the domain's charge admits.
        let slice = reservation.slice();
        if payment.tokens() > slice.tokens {
            return Err(BudgetMeterError::ReservationExceeded {
                reserved_tokens: slice.tokens,
                payment_tokens: payment.tokens(),
            });
        }
        // Payments carry no rounds dimension (§5.3.4 payment is tokens,
        // wall seconds, tool calls, cost): the round counter moves only at
        // iteration boundaries, never per charge.
        inner
            .budget
            .charge(
                0,
                payment.wall_seconds(),
                payment.tokens(),
                payment.cost_usd(),
            )
            .map_err(|cause| BudgetMeterError::Charge { cause })?;
        // Booked (or a zero payment, which books nothing): release the
        // slot. A refused charge above returned before this line, so the
        // dispatch keeps its reservation.
        inner.outstanding = inner.outstanding.saturating_sub(1);
        inner.tool_calls_total += payment.tool_calls();
        Ok(())
    }

    fn remaining(&self) -> Remaining {
        let inner = self.lock();
        // The port's projection verbatim: limits minus spent. The shared
        // suite pins that a RESERVE changes nothing here ("only charges
        // book spend"), so outstanding-slot availability is NOT expressed
        // through `remaining` — it is observable exactly where it bites,
        // in `reserve`'s scope refusal at the `max_parallel` ceiling.
        // Depth has no meter-level signal either; the ceiling stands (the
        // controller checks its own dispatch depth).
        remaining_of(&inner.budget)
    }

    fn would_breach_reserve(&self, slice: BudgetSlice) -> bool {
        let inner = self.lock();
        inner.budget.would_breach_reserve(
            slice.rounds,
            slice.wall_seconds,
            slice.tokens,
            slice.cost_usd,
        )
    }
}

#[cfg(test)]
mod tests {
    use gauntlet_domain::{Budget, DiagnosticAllowance, Limits, MicroUsd};

    use super::MeteredBudget;
    use gauntlet_ports::budget_meter::{BudgetMeter, BudgetMeterError, BudgetSlice};
    use gauntlet_ports::contract::budget_meter::{BudgetMeterFactory, budget_meter_suite};

    fn limits() -> Limits {
        Limits::new(10, 2, 2, 3600, 1000, MicroUsd::new(500)).expect("usable ceilings")
    }

    fn meter() -> MeteredBudget {
        MeteredBudget::new(
            Budget::new(
                limits(),
                20,
                DiagnosticAllowance::new(100, MicroUsd::new(50)),
            )
            .expect("usable budget"),
        )
    }

    fn slice(tokens: u64) -> BudgetSlice {
        BudgetSlice {
            rounds: 1,
            wall_seconds: 60,
            tokens,
            cost_usd: MicroUsd::new(1),
        }
    }

    /// The suite runs through the same factory the adapter's contract test
    /// uses, so the in-crate run and the contract run cannot drift.
    struct SuiteFactory;

    impl BudgetMeterFactory for SuiteFactory {
        fn label(&self) -> &'static str {
            "metered-budget"
        }

        fn build(
            &self,
            limits: Limits,
            recovery_reserve_pct: u8,
            diagnostic: DiagnosticAllowance,
        ) -> Result<Box<dyn BudgetMeter>, String> {
            let budget =
                Budget::new(limits, recovery_reserve_pct, diagnostic).map_err(|e| e.to_string())?;
            Ok(Box::new(MeteredBudget::new(budget)))
        }
    }

    #[test]
    fn the_metered_budget_passes_the_shared_contract_suite() {
        let findings = budget_meter_suite(&SuiteFactory);
        assert!(
            findings.is_empty(),
            "the metered budget must pass the shared suite:\n{}",
            findings
                .iter()
                .map(|finding| finding.to_string())
                .collect::<Vec<_>>()
                .join("\n")
        );
    }

    /// Reserve↔charge agreement pin: for any slice — boundary-exact values
    /// included (== ceiling, == limit) — `would_breach_reserve(slice)`
    /// agrees with `reserve(slice)`'s refusal, before AND after identical
    /// bookings. Each probe uses a FRESH pair of identically-constructed
    /// meters, so a successful reserve never leaves parallel slots held
    /// that would turn a scope refusal into a false disagreement.
    #[test]
    fn reserve_agrees_with_would_breach_reserve_at_every_boundary() {
        // At a 20 percent reserve: rounds ceiling 8 (of 10), wall 2880
        // (of 3600), tokens 800 (of 1000), cost 400 (of 500). Every probe
        // holds the OTHER dimensions at zero so the boundary under test is
        // the only one that can decide.
        let boundary = |rounds, wall, tokens, cost| BudgetSlice {
            rounds,
            wall_seconds: wall,
            tokens,
            cost_usd: MicroUsd::new(cost),
        };
        let probes = [
            boundary(0, 0, 0, 0),
            boundary(8, 0, 0, 0),
            boundary(9, 0, 0, 0),
            boundary(10, 0, 0, 0),
            boundary(0, 2880, 0, 0),
            boundary(0, 2881, 0, 0),
            boundary(0, 0, 799, 0),
            boundary(0, 0, 800, 0),
            boundary(0, 0, 801, 0),
            boundary(0, 0, 1000, 0),
            boundary(0, 0, 1001, 0),
            boundary(0, 0, 0, 400),
            boundary(0, 0, 0, 401),
        ];
        // Before any booking.
        for probe in probes {
            let asking = meter();
            let reserving = meter();
            assert_eq!(
                asking.would_breach_reserve(probe),
                matches!(
                    reserving.reserve(probe),
                    Err(BudgetMeterError::Reserve { .. })
                ),
                "before bookings, for {probe:?} the two views of the recovery reserve disagree"
            );
        }
        // After the SAME booking on both meters (booked through the port:
        // reserve then charge, so the reservation bookkeeping rides along).
        let book = |probe_meter: &MeteredBudget| {
            let reservation = probe_meter
                .reserve(boundary(0, 60, 100, 1))
                .expect("the pin's booking slice is legal");
            probe_meter
                .charge(
                    &reservation,
                    gauntlet_domain::Payment::new(100, 60, 0, MicroUsd::new(1)),
                )
                .expect("the pin's booking payment is within the reservation");
        };
        let moved: [BudgetSlice; 4] = [
            boundary(8, 2820, 700, 399),
            boundary(8, 2820, 701, 399),
            boundary(0, 0, 800, 0),
            boundary(0, 0, 801, 0),
        ];
        for probe in moved {
            let asking = meter();
            let reserving = meter();
            book(&asking);
            book(&reserving);
            assert_eq!(
                asking.would_breach_reserve(probe),
                matches!(
                    reserving.reserve(probe),
                    Err(BudgetMeterError::Reserve { .. })
                ),
                "after bookings, for {probe:?} the two views of the recovery reserve disagree"
            );
        }
    }

    #[test]
    fn reservations_hold_parallel_slots_until_charged() {
        let meter = meter();
        // max_parallel is 2: two reservations stand, the third is refused
        // with the domain's own scope error, and nothing was booked.
        let first = meter.reserve(slice(10)).expect("the first slot");
        // Holds the second slot for the rest of the test; never charged.
        let _second = meter.reserve(slice(10)).expect("the second slot");
        assert!(matches!(
            meter.reserve(slice(10)),
            Err(BudgetMeterError::Reserve { .. })
        ));
        // `remaining` stays the limits-minus-spent projection (the suite
        // pins that a reserve moves nothing there), so slot availability is
        // asserted where it bites: the third reserve above was the refusal.
        assert_eq!(
            meter.remaining().parallel,
            2,
            "remaining is the projection, not the slot count"
        );
        // A zero-payment charge books nothing and releases the slot — the
        // documented release path for cancelled or failed invocations.
        // (N5: `remaining` is the FORWARD ceiling minus spent, so the
        // nothing-booked invariant is before/after equality, not a
        // raw-limit constant.)
        let unspent = meter.remaining().tokens;
        meter
            .charge(&first, gauntlet_domain::Payment::zero())
            .expect("the zero payment releases the slot");
        assert_eq!(meter.remaining().tokens, unspent, "nothing was booked");
        // The released slot is reservable again — and that reservation is
        // the one the refused-charge probe below holds.
        let refilled = meter
            .reserve(slice(10))
            .expect("the released slot is reservable again");
        assert!(matches!(
            meter.reserve(slice(10)),
            Err(BudgetMeterError::Reserve { .. })
        ));
        let oversized = gauntlet_domain::Payment::new(11, 0, 0, MicroUsd::new(0));
        assert!(matches!(
            meter.charge(&refilled, oversized),
            Err(BudgetMeterError::ReservationExceeded { .. })
        ));
        assert!(
            matches!(
                meter.reserve(slice(10)),
                Err(BudgetMeterError::Reserve { .. })
            ),
            "the refused charge held the slot"
        );
        // A real charge books and releases.
        meter
            .charge(
                &refilled,
                gauntlet_domain::Payment::new(10, 1, 3, MicroUsd::new(1)),
            )
            .expect("the within-reservation payment books");
        assert_eq!(
            meter.remaining().tokens,
            unspent - 10,
            "the within-reservation payment booked its tokens"
        );
        assert!(
            meter.reserve(slice(10)).is_ok(),
            "the charged slot is reservable again"
        );
    }

    #[test]
    fn a_reservation_serves_more_than_one_within_bounds_charge() {
        // The port's reservation bounds charges, it does not expire on
        // first use (the shared suite pins the double charge); the slot
        // release saturates rather than underflowing.
        let meter = meter();
        let reservation = meter.reserve(slice(400)).expect("a legal slice");
        let unspent = meter.remaining().tokens;
        meter
            .charge(
                &reservation,
                gauntlet_domain::Payment::new(100, 1, 0, MicroUsd::new(1)),
            )
            .expect("the first payment books");
        meter
            .charge(
                &reservation,
                gauntlet_domain::Payment::new(100, 1, 0, MicroUsd::new(1)),
            )
            .expect("the second payment books against the same reservation");
        assert_eq!(
            meter.remaining().tokens,
            unspent - 200,
            "both within-reservation payments booked their tokens"
        );
        // Saturated release: outstanding never underflows, and the meter
        // still refuses a slot past max_parallel.
        let held = meter.reserve(slice(10)).expect("a slot");
        let held_too = meter.reserve(slice(10)).expect("another slot");
        assert!(matches!(
            meter.reserve(slice(10)),
            Err(BudgetMeterError::Reserve { .. })
        ));
        let _ = (held, held_too);
    }

    /// §17.4's tool-invocation and retry dimensions: tracked (exposed on
    /// the adapter), enforced nowhere — the domain Limits and the port
    /// Remaining have no fields for them. The recorded residual lives in
    /// the Task-12 report; this test pins the tracking that exists.
    #[test]
    fn tool_calls_and_retries_are_tracked_as_adapter_counters() {
        let meter = meter();
        assert_eq!(meter.tool_calls_total(), 0);
        assert_eq!(meter.retries_total(), 0);
        let reservation = meter.reserve(slice(100)).expect("a legal slice");
        meter
            .charge(
                &reservation,
                gauntlet_domain::Payment::new(10, 0, 4, MicroUsd::new(0)),
            )
            .expect("the payment books");
        meter
            .charge(
                &reservation,
                gauntlet_domain::Payment::new(10, 0, 5, MicroUsd::new(0)),
            )
            .expect("the second payment books");
        assert_eq!(meter.tool_calls_total(), 9);
        meter.note_retry();
        meter.note_retry();
        assert_eq!(meter.retries_total(), 2);
    }

    #[test]
    fn refused_charges_book_nothing() {
        let meter = meter();
        let reservation = meter.reserve(slice(10)).expect("a legal slice");
        let before = meter.remaining();
        assert!(matches!(
            meter.charge(
                &reservation,
                gauntlet_domain::Payment::new(11, 0, 0, MicroUsd::new(0))
            ),
            Err(BudgetMeterError::ReservationExceeded { .. })
        ));
        assert_eq!(meter.remaining().tokens, before.tokens);
        // A payment that passes the reservation bound but breaches the
        // domain's reserve (the cumulative arithmetic) is refused with the
        // domain's typed cause and books nothing either.
        let wide = meter.reserve(slice(800)).expect("the ceiling slice");
        let _ = wide;
        assert!(matches!(
            meter.charge(
                &reservation,
                gauntlet_domain::Payment::new(0, 0, 0, MicroUsd::new(500))
            ),
            Err(BudgetMeterError::Charge { .. })
        ));
        assert_eq!(meter.remaining().tokens, before.tokens);
        assert_eq!(
            meter.remaining().cost_usd,
            before.cost_usd,
            "the refused cost payment booked nothing"
        );
    }
}
