//! Deterministic `Clock` adapter (spec §6.2: "`now()` · `sleep_until` —
//! Injectable; tests use a deterministic clock"; Annex E test hook
//! `GAUNTLET_CLOCK`).
//!
//! Virtual time, advanced only by explicit [`Clock::sleep_until`] calls:
//! sleeping moves the clock FORWARD to the target instant and never sleeps
//! for real, so a test that "waits" is instantaneous while the observable
//! time line still moves. `now()` is pure. Sleeping backwards is a no-op
//! (monotonic max semantics — a virtual clock that could move backwards
//! would let deadlines un-expire, which no real clock can do and which
//! `Deadline::exceeded_at` consumers rely on never seeing).

use core::cell::Cell;

use gauntlet_domain::EpochSeconds;
use gauntlet_ports::clock::{Clock, ClockError};

/// A virtual clock whose instant moves only when something sleeps.
///
/// Single-threaded by design (a `Cell` interior): the mock runner family is
/// a test double, and cross-thread virtual time would need a shared
/// timeline contract the spec does not define. Tests needing one timeline
/// share one `&DeterministicClock`.
#[derive(Debug, Clone)]
pub struct DeterministicClock {
    now: Cell<EpochSeconds>,
}

impl DeterministicClock {
    /// A clock resting at the given instant.
    pub fn start_at(instant: EpochSeconds) -> Self {
        Self {
            now: Cell::new(instant),
        }
    }

    /// The current virtual instant — the port's `now`, exposed for tests
    /// that want to assert the time line directly.
    pub fn instant(&self) -> EpochSeconds {
        self.now.get()
    }
}

impl Clock for DeterministicClock {
    fn now(&self) -> EpochSeconds {
        self.now.get()
    }

    fn sleep_until(&self, at: EpochSeconds) -> Result<(), ClockError> {
        // Monotonic max: advance only forward. The ports' trait doc pins the
        // deterministic-adapter reading — "a sleep to `at` moves the clock
        // to `at`" — and a backwards move is silently swallowed rather than
        // an error because real `sleep_until(past)` also returns instantly
        // without moving the clock; matching that observable is the honest
        // emulation, and callers that need to notice can compare `now()`.
        if at > self.now.get() {
            self.now.set(at);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::DeterministicClock;
    use gauntlet_domain::EpochSeconds;
    use gauntlet_ports::clock::Clock;

    const EARLY: u64 = 1_700_000_000;
    const LATE: u64 = EARLY + 90;

    #[test]
    fn now_is_pure_until_something_sleeps() {
        let clock = DeterministicClock::start_at(EpochSeconds::new(EARLY));
        assert_eq!(clock.now().get(), EARLY);
        assert_eq!(clock.now().get(), EARLY, "reading does not advance time");
    }

    #[test]
    fn sleeping_advances_to_the_target_instant() {
        let clock = DeterministicClock::start_at(EpochSeconds::new(EARLY));
        clock
            .sleep_until(EpochSeconds::new(LATE))
            .expect("virtual sleep never fails");
        assert_eq!(clock.now().get(), LATE);
    }

    #[test]
    fn sleeping_never_moves_backwards() {
        let clock = DeterministicClock::start_at(EpochSeconds::new(LATE));
        clock
            .sleep_until(EpochSeconds::new(EARLY))
            .expect("virtual sleep never fails");
        assert_eq!(
            clock.now().get(),
            LATE,
            "a backwards sleep leaves the clock where it is — deadlines cannot un-expire"
        );
    }

    #[test]
    fn repeated_sleeps_settle_on_the_maximum_target() {
        let clock = DeterministicClock::start_at(EpochSeconds::new(EARLY));
        clock
            .sleep_until(EpochSeconds::new(EARLY + 5))
            .expect("virtual sleep never fails");
        clock
            .sleep_until(EpochSeconds::new(EARLY + 2))
            .expect("virtual sleep never fails");
        clock
            .sleep_until(EpochSeconds::new(LATE))
            .expect("virtual sleep never fails");
        assert_eq!(clock.now().get(), LATE);
    }
}
