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

use core::sync::atomic::{AtomicU64, Ordering};

use gauntlet_domain::EpochSeconds;
use gauntlet_ports::clock::{Clock, ClockError};

/// A virtual clock whose instant moves only when something sleeps.
///
/// The interior is one atomic word, so the clock is `Send + Sync` and
/// shareable as `Arc<dyn Clock>` — the shape every port consumer wires
/// (the Task-9 composition root hands the same clock to several observers
/// and the assembler). The shared timeline contract is the atomic itself:
/// monotonic max under [`Clock::sleep_until`], from any thread.
#[derive(Debug)]
pub struct DeterministicClock {
    now: AtomicU64,
}

impl DeterministicClock {
    /// A clock resting at the given instant.
    pub fn start_at(instant: EpochSeconds) -> Self {
        Self {
            now: AtomicU64::new(instant.get()),
        }
    }

    /// The current virtual instant — the port's `now`, exposed for tests
    /// that want to assert the time line directly.
    pub fn instant(&self) -> EpochSeconds {
        EpochSeconds::new(self.now.load(Ordering::Relaxed))
    }
}

impl Clone for DeterministicClock {
    /// Snapshots the current instant into an independent clock (a clone of
    /// a clock is a new clock reading the same time, not a shared handle —
    /// sharing is what `Arc` is for).
    fn clone(&self) -> Self {
        Self {
            now: AtomicU64::new(self.now.load(Ordering::Relaxed)),
        }
    }
}

impl Clock for DeterministicClock {
    fn now(&self) -> EpochSeconds {
        EpochSeconds::new(self.now.load(Ordering::Relaxed))
    }

    fn sleep_until(&self, at: EpochSeconds) -> Result<(), ClockError> {
        // Monotonic max: advance only forward, via compare-exchange so a
        // concurrent sleep can never lose the maximum target. The ports'
        // trait doc pins the deterministic-adapter reading — "a sleep to
        // `at` moves the clock to `at`" — and a backwards move is silently
        // swallowed rather than an error because real `sleep_until(past)`
        // also returns instantly without moving the clock; matching that
        // observable is the honest emulation, and callers that need to
        // notice can compare `now()`.
        let target = at.get();
        let mut current = self.now.load(Ordering::Relaxed);
        while target > current {
            match self.now.compare_exchange_weak(
                current,
                target,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(observed) => current = observed,
            }
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

    /// The composition-root shape this clock must support (Task 9's p3
    /// proof hands one clock to several observers under `Arc<dyn Clock>`):
    /// concurrent sleeps from several threads settle on the maximum
    /// target, never a lost update. A `Cell` interior cannot even compile
    /// this test — the atomic interior is what makes the clock shareable.
    #[test]
    fn concurrent_sleeps_settle_on_the_maximum_target_under_sharing() {
        use std::sync::Arc;
        let clock = Arc::new(DeterministicClock::start_at(EpochSeconds::new(EARLY)));
        let shared: Arc<dyn Clock + Send + Sync> = clock.clone();
        let mut threads = Vec::new();
        for offset in [10u64, 90, 30] {
            let shared = shared.clone();
            threads.push(std::thread::spawn(move || {
                shared
                    .sleep_until(EpochSeconds::new(EARLY + offset))
                    .expect("virtual sleep never fails");
            }));
        }
        for thread in threads {
            thread.join().expect("the sleeper threads must not panic");
        }
        assert_eq!(
            shared.now().get(),
            LATE,
            "the maximum target wins and no concurrent sleep is lost"
        );
    }
}
