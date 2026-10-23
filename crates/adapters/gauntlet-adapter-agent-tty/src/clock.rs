//! The production [`Clock`](gauntlet_ports::Clock) — the wall clock (spec
//! §6.2), the prime adapter's posture mirrored: the RUNNER never reads the
//! system time directly, so tests bind a deterministic clock (the shared
//! suite's pinned deadlines live in the past; only an injected clock can
//! drive them deterministically). Production composition injects this.

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
        EpochSeconds::new(wall_seconds())
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

fn wall_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_system_clock_reads_epoch_seconds() {
        let clock = SystemClock::new();
        let now = clock.now();
        assert!(now.get() > 1_600_000_000, "the wall clock reads: {now:?}");
        clock
            .sleep_until(EpochSeconds::new(1))
            .expect("a reached instant does not interrupt");
    }
}
