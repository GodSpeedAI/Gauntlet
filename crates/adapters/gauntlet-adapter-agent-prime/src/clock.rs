//! The production [`Clock`](gauntlet_ports::Clock) — the wall clock (spec
//! §6.2: "Production adapters read the system clock; deterministic test
//! adapters advance under the test's control").
//!
//! The RUNNER never reads the system time directly: every `now` flows
//! through this port so tests bind a deterministic clock (the shared
//! contract suite's deadlines are pinned in the past — only an injected
//! clock can drive them deterministically).

use gauntlet_domain::EpochSeconds;
use gauntlet_ports::clock::{Clock, ClockError};

/// The system wall clock.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl SystemClock {
    pub fn new() -> Self {
        Self
    }
}

impl Clock for SystemClock {
    fn now(&self) -> EpochSeconds {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|since| since.as_secs())
            .unwrap_or(0);
        EpochSeconds::new(secs)
    }

    fn sleep_until(&self, at: EpochSeconds) -> Result<(), ClockError> {
        loop {
            let now = self.now();
            if now >= at {
                return Ok(());
            }
            let remaining = at.get().saturating_sub(now.get());
            let step = remaining.min(SLEEP_STEP_SECONDS);
            std::thread::sleep(std::time::Duration::from_secs(step));
        }
    }
}

/// The longest single sleep the clock takes, in seconds, so an external
/// clock move (or process signal) is noticed promptly.
const SLEEP_STEP_SECONDS: u64 = 1;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_system_clock_reads_epoch_seconds() {
        let clock = SystemClock::new();
        let now = clock.now();
        assert!(now.get() > 1_600_000_000, "the wall clock reads: {now:?}");
        // Sleeping to the (already reached) past returns immediately.
        clock
            .sleep_until(EpochSeconds::new(1))
            .expect("a reached instant does not interrupt");
    }
}
