//! `Clock` — spec §6.2: "`now()` · `sleep_until` — Injectable; tests use a
//! deterministic clock."
//!
//! The only source of "now" in the runtime. The domain's
//! [`EpochSeconds`](gauntlet_domain::EpochSeconds) is the value; this port
//! is where it comes from. Production adapters read the system clock;
//! deterministic test adapters advance under the test's control.

use core::fmt;

use gauntlet_domain::EpochSeconds;

/// `Clock` failures — typed (spec §21.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClockError {
    /// The wait was interrupted. The clock did not reach the target
    /// instant; the caller decides whether to retry.
    Interrupted { detail: String },
}

impl fmt::Display for ClockError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Interrupted { detail } => {
                write!(f, "the clock's wait was interrupted: {detail}")
            }
        }
    }
}

impl std::error::Error for ClockError {}

/// The time port (spec §6.2). `now` is pure; `sleep_until` blocks until the
/// target instant BY DESIGN — its parameter is the bound itself, so no
/// separate deadline is needed (§6 preamble).
pub trait Clock {
    /// The current instant.
    fn now(&self) -> EpochSeconds;

    /// Blocks until the given instant (or as close as the platform
    /// permits). Deterministic adapters use this as their advance
    /// mechanism: a sleep to `at` moves the clock to `at`.
    fn sleep_until(&self, at: EpochSeconds) -> Result<(), ClockError>;
}
