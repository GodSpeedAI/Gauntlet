//! Annex E test-hook selection (spec §8.3 / Annex E: "Test hooks |
//! `GAUNTLET_CLOCK` `GAUNTLET_ID_SEED` (deterministic clock/ids for tests
//! only)").
//!
//! These helpers read the hooks and decide WHICH time/id source a test
//! composition uses:
//!
//! - `GAUNTLET_CLOCK` — unset or empty keeps the production clock (the
//!   selector returns `None` and the caller's default stands); the exact
//!   token `deterministic` selects [`DeterministicClock`], resting at the
//!   virtual epoch; ANY other value is the typed
//!   [`EnvHookError::UnknownClockToken`] — a silent fallback to either
//!   clock would make a typo'd hook look like a pass, which is precisely
//!   the fail-open shape the boundary suite exists to forbid.
//! - `GAUNTLET_ID_SEED` — unset or empty keeps the production id source
//!   (`None`); any non-empty string seeds [`SeededIdGenerator`]. There is
//!   no error vocabulary for this hook: the only refusal a seed can meet
//!   is emptiness, and an empty hook never reaches construction.
//!
//! The selection logic is exposed as the pure [`select_clock`] /
//! [`select_id_generator`] functions over the raw hook value; the
//! `*_from_env` wrappers only read the environment and delegate. That
//! split is deliberate: environment mutation is `unsafe` under the 2024
//! edition (racy with concurrent readers) and this workspace forbids
//! `unsafe`, so tests exercise the pure selectors with arbitrary values
//! instead of mutating process state. Compositions that DO set the hooks
//! (test harnesses, not this crate) may do so freely — cargo nextest runs
//! every test in its own process, so process-global environment mutation
//! cannot leak between tests; do not "fix" that by portifying these
//! helpers into a shared crate.
//!
//! These stay ADAPTER-local by design: they are test doubles for the
//! composition root to consult, not port semantics (the ports carry the
//! traits only).

use core::fmt;

use gauntlet_domain::EpochSeconds;

use crate::deterministic_clock::DeterministicClock;
use crate::seeded_ids::SeededIdGenerator;

/// The exact `GAUNTLET_CLOCK` token that selects the deterministic clock.
pub const CLOCK_TOKEN: &str = "deterministic";

/// A test hook carried a value outside its vocabulary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnvHookError {
    /// `GAUNTLET_CLOCK` was set to something that names no clock. Never a
    /// silent fallback — the run stops and the operator fixes the hook.
    UnknownClockToken { value: String },
}

impl fmt::Display for EnvHookError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownClockToken { value } => write!(
                f,
                "GAUNTLET_CLOCK={value:?} names no clock; the only supported token is \
                 {CLOCK_TOKEN:?} (unset it to keep the production clock)"
            ),
        }
    }
}

impl std::error::Error for EnvHookError {}

/// Reads one hook: absent or empty both read as "no hook" — an empty
/// value carries no selection, exactly like an unset one.
fn hook_value(var: &str) -> Option<String> {
    std::env::var(var).ok().filter(|value| !value.is_empty())
}

/// Selects the clock from the raw `GAUNTLET_CLOCK` value. `Ok(None)`
/// (absent, empty, or `None` passed directly) keeps the production clock;
/// [`CLOCK_TOKEN`] selects the deterministic clock resting at the virtual
/// epoch (`0`) — compositions wanting a different origin construct
/// [`DeterministicClock`] directly. Any other value is a typed error.
pub fn select_clock(raw: Option<&str>) -> Result<Option<DeterministicClock>, EnvHookError> {
    let Some(value) = raw.filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    if value == CLOCK_TOKEN {
        return Ok(Some(DeterministicClock::start_at(EpochSeconds::new(0))));
    }
    Err(EnvHookError::UnknownClockToken {
        value: value.to_owned(),
    })
}

/// Reads `GAUNTLET_CLOCK` from the environment and selects the clock
/// (see [`select_clock`]).
pub fn clock_from_env() -> Result<Option<DeterministicClock>, EnvHookError> {
    select_clock(hook_value("GAUNTLET_CLOCK").as_deref())
}

/// Selects the id generator from the raw `GAUNTLET_ID_SEED` value:
/// absent or empty keeps the production id source (`None`); a non-empty
/// seed selects the seeded generator. This hook cannot fail — the only
/// refusal a seed can meet is emptiness, and an empty value is `None`
/// before construction is ever attempted.
pub fn select_id_generator(raw: Option<&str>) -> Option<SeededIdGenerator> {
    let value = raw.filter(|value| !value.is_empty())?;
    // Construction cannot refuse a non-empty seed (the sole refusal is the
    // empty one, excluded above); `.ok()` discards that impossibility
    // rather than inventing an error vocabulary no caller can ever see.
    SeededIdGenerator::new(value).ok()
}

/// Reads `GAUNTLET_ID_SEED` from the environment and selects the id
/// generator (see [`select_id_generator`]).
pub fn id_generator_from_env() -> Option<SeededIdGenerator> {
    select_id_generator(hook_value("GAUNTLET_ID_SEED").as_deref())
}

#[cfg(test)]
mod tests {
    use super::{CLOCK_TOKEN, EnvHookError, select_clock, select_id_generator};
    use crate::deterministic_clock::DeterministicClock;
    use crate::seeded_ids::SeededIdGenerator;
    use gauntlet_domain::EpochSeconds;
    use gauntlet_ports::clock::Clock;
    use gauntlet_ports::id_generator::IdGenerator;

    #[test]
    fn an_absent_or_empty_clock_hook_keeps_the_production_clock() {
        for raw in [None, Some(""), Some("   ")] {
            // NB: `Some("   ")` is whitespace, not empty — it must be a
            // typed error, so only None and Some("") read as "no hook"
            // here; the whitespace case is asserted separately below.
            if raw == Some("   ") {
                continue;
            }
            assert!(
                select_clock(raw)
                    .expect("absent or empty is not an error")
                    .is_none(),
                "raw {raw:?} must keep the production clock"
            );
        }
    }

    #[test]
    fn the_deterministic_token_selects_the_virtual_clock() {
        let clock = select_clock(Some(CLOCK_TOKEN))
            .expect("the documented token selects")
            .expect("the token selects a clock");
        assert_eq!(
            clock.now().get(),
            0,
            "the selected clock rests at the epoch"
        );
        clock
            .sleep_until(EpochSeconds::new(12))
            .expect("virtual sleep never fails");
        assert_eq!(clock.now().get(), 12);
    }

    #[test]
    fn unknown_clock_tokens_are_typed_errors_never_fallbacks() {
        for token in [
            "Deterministic",
            "deterministic ",
            " system",
            "system",
            "mock",
            "   ",
            "deterministic\n",
        ] {
            match select_clock(Some(token)) {
                Err(EnvHookError::UnknownClockToken { value }) => assert_eq!(value, token),
                other => panic!("token {token:?} must be a typed error, got {other:?}"),
            }
        }
    }

    #[test]
    fn a_seed_selects_the_seeded_generator_and_it_replays() {
        let first = select_id_generator(Some("hook-seed")).expect("a non-empty seed selects");
        let second = SeededIdGenerator::new("hook-seed").expect("same seed direct");
        assert_eq!(first.run_id(), second.run_id());
        assert_eq!(first.invocation_id(), second.invocation_id());
    }

    #[test]
    fn an_absent_or_empty_seed_keeps_the_production_source() {
        assert!(select_id_generator(None).is_none());
        assert!(select_id_generator(Some("")).is_none());
    }

    /// A direct construction path stays available for compositions that
    /// want a chosen origin rather than the hook's epoch zero.
    #[test]
    fn the_clock_type_remains_directly_constructible() {
        let clock = DeterministicClock::start_at(EpochSeconds::new(5));
        assert_eq!(clock.now().get(), 5);
    }
}
