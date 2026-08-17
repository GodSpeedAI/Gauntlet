//! The `Clock` shared contract suite (spec §6.2: "Injectable; tests use a
//! deterministic clock").
//!
//! Pinned semantics: `now` never moves backwards (real or deterministic);
//! after `sleep_until(at)` with `at > now`, `now` is at least `at`; when
//! the FACTORY declares the adapter deterministic, two instances agree at
//! every step of an identical sequence.

use gauntlet_domain::EpochSeconds;

use crate::clock::Clock;
use crate::contract::ContractFinding;

/// The suite's name in findings.
pub const SUITE: &str = "clock";

/// Constructs the clock under test and declares whether it is
/// deterministic.
pub trait ClockFactory {
    fn label(&self) -> &'static str;

    fn build(&self) -> Box<dyn Clock>;

    /// Whether every instance built by this factory, driven by identical
    /// sequences, reports identical instants (spec §6.2: "tests use a
    /// deterministic clock").
    fn deterministic(&self) -> bool;
}

fn finding(case: &'static str, rule: &'static str, detail: String) -> ContractFinding {
    ContractFinding {
        suite: SUITE,
        case,
        rule,
        detail,
    }
}

/// Case: `now` never decreases across reads and sleeps.
fn now_never_moves_backwards(factory: &dyn ClockFactory) -> Vec<ContractFinding> {
    const CASE: &str = "now_never_moves_backwards";
    const RULE: &str = "a clock is monotonic across reads and sleeps";
    let clock = factory.build();
    let mut findings = Vec::new();
    let mut previous = clock.now();
    for _ in 0..4 {
        let current = clock.now();
        if current < previous {
            findings.push(finding(
                CASE,
                RULE,
                format!("now moved backwards from {previous} to {current}"),
            ));
            return findings;
        }
        previous = current;
        // A sleep to a strictly later instant must land at or past it.
        let target = EpochSeconds::new(previous.get() + 1);
        if let Err(error) = clock.sleep_until(target) {
            findings.push(finding(
                CASE,
                RULE,
                format!("sleep_until({target}) errored: {error}"),
            ));
            return findings;
        }
        let after = clock.now();
        if after < target {
            findings.push(finding(
                CASE,
                RULE,
                format!("after sleeping until {target}, now is only {after}"),
            ));
            return findings;
        }
        previous = after;
    }
    findings
}

/// Case: when the factory declares determinism, two instances agree at
/// every step of an identical sequence. (A wall-clock adapter declares
/// false and skips nothing — this case simply does not apply to it.)
fn deterministic_clocks_agree(factory: &dyn ClockFactory) -> Vec<ContractFinding> {
    const CASE: &str = "deterministic_clocks_agree";
    const RULE: &str = "spec 6.2: a deterministic clock replays identically per instance";
    if !factory.deterministic() {
        return Vec::new();
    }
    let one = factory.build();
    let two = factory.build();
    let mut findings = Vec::new();
    for _ in 0..3 {
        if one.now() != two.now() {
            findings.push(finding(
                CASE,
                RULE,
                format!(
                    "two deterministic instances report {} and {} at the same step",
                    one.now(),
                    two.now()
                ),
            ));
            return findings;
        }
        let advance_to = EpochSeconds::new(one.now().get() + 5);
        let (a, b) = (one.sleep_until(advance_to), two.sleep_until(advance_to));
        if let (Ok(()), Ok(())) = (a, b) {
            continue;
        }
        findings.push(finding(
            CASE,
            RULE,
            "an identical sleep failed on one instance but not the other".to_owned(),
        ));
        return findings;
    }
    findings
}

/// Runs the full `Clock` contract suite against one factory.
pub fn clock_suite(factory: &dyn ClockFactory) -> Vec<ContractFinding> {
    let mut findings = Vec::new();
    findings.extend(now_never_moves_backwards(factory));
    findings.extend(deterministic_clocks_agree(factory));
    findings
}
