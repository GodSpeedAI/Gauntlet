//! `Deadline` — the ports-layer bound on blocking work (spec §6: "Every port
//! that can block has a deadline parameter").
//!
//! A deadline is a POINT, not a span: the latest instant by which the
//! operation must have returned. It wraps the domain's
//! [`EpochSeconds`](gauntlet_domain::EpochSeconds) so ports introduce no time
//! concept of their own — the domain stays the single vocabulary, and the
//! `Clock` port remains the only source of "now".
//!
//! No default exists: an unbounded blocking call is exactly what this type
//! exists to make unrepresentable at the call site (spec §22.7: hooks run
//! with a REQUIRED timeout; AGENTS.md: "Absence of a configured budget is
//! never permission for unlimited recursion" — same posture for time).

use core::fmt;

use gauntlet_domain::EpochSeconds;

/// The latest instant by which a blocking port call must return.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Deadline {
    at: EpochSeconds,
}

impl Deadline {
    /// Adopts an instant as the bound. The value comes from a `Clock` plus
    /// configured policy — never from a literal here.
    pub fn at_epoch(at: EpochSeconds) -> Self {
        Self { at }
    }

    /// The instant this deadline names.
    pub fn instant(self) -> EpochSeconds {
        self.at
    }

    /// Whether `now` has passed this deadline.
    pub fn exceeded_at(self, now: EpochSeconds) -> bool {
        now > self.at
    }
}

impl fmt::Display for Deadline {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "by {}", self.at)
    }
}

#[cfg(test)]
mod tests {
    use super::Deadline;
    use gauntlet_domain::EpochSeconds;

    const BASE: u64 = 1_700_000_000;
    const SPAN: u64 = 30;

    #[test]
    fn a_deadline_is_a_point_and_expiry_is_exact() {
        let bound = Deadline::at_epoch(EpochSeconds::new(BASE + SPAN));
        assert_eq!(bound.instant().get(), BASE + SPAN);
        assert!(!bound.exceeded_at(EpochSeconds::new(BASE)));
        assert!(!bound.exceeded_at(EpochSeconds::new(BASE + SPAN)));
        assert!(bound.exceeded_at(EpochSeconds::new(BASE + SPAN + 1)));
        assert!(Deadline::at_epoch(EpochSeconds::new(BASE)) < bound);
    }
}
