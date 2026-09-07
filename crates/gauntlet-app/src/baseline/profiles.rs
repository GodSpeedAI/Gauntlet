//! The typed profile and scenario vocabularies for the §19.x baseline
//! measurement harness (task 26; plan step "Implement runner profiles").
//!
//! Fail-closed discipline (spec §5.2 rule, AK-004): an unknown token is a
//! typed error naming the token AND the accepted vocabulary — never a
//! coerced default, never a case-insensitive rescue. Lowercase `a` is not
//! profile A.

use core::fmt;

/// The baseline runner profile (charter decision 2): the measured variable
/// is verification DEPTH — how much posture-internal verification stands
/// between the builder's own claim and a completion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BaselineProfile {
    /// Builder-only: one dispatch, completion is the builder's own claim.
    A,
    /// Builder + self-review: a second dispatch re-runs the DECLARED check.
    B,
    /// Builder + fresh critic: a second dispatch sees ONLY the requirement
    /// text, never the builder's result document.
    C,
    /// Full Gauntlet: the real composed loop with its own instruments,
    /// comparison, verification, budget, and settlement machinery.
    D,
}

impl BaselineProfile {
    /// Every profile, in charter order (also the report's column order).
    pub const ALL: &'static [Self] = &[Self::A, Self::B, Self::C, Self::D];

    /// Every profile (named accessor for the CLI's vocabulary rendering).
    pub fn all() -> &'static [Self] {
        Self::ALL
    }

    /// Exact-match parse of the CLI's `--profiles` member token.
    pub fn parse(token: &str) -> Result<Self, UnknownToken> {
        match token {
            "A" => Ok(Self::A),
            "B" => Ok(Self::B),
            "C" => Ok(Self::C),
            "D" => Ok(Self::D),
            other => Err(UnknownToken {
                input: other.to_owned(),
                vocabulary: PROFILE_VOCABULARY,
            }),
        }
    }

    /// The wire token.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::A => "A",
            Self::B => "B",
            Self::C => "C",
            Self::D => "D",
        }
    }
}

const PROFILE_VOCABULARY: &str = "A, B, C, D";

/// The scripted scenario a baseline leg runs (charter scenario matrix):
/// what the fixture child does, selected per leg by `GAUNTLET_BASELINE_MODE`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BaselineScenario {
    /// Keeps the pre-seeded property, adds the asked-for one, honest claims.
    HonestSuccess,
    /// Writes nothing, forges passing claims.
    LyingDefect,
    /// Writes only the new property — drops the pre-seeded one; honest
    /// claims (the declared check genuinely passes).
    SubtleDefect,
    /// Lying-defect artifact; the critic branch always passes.
    CompromisedCritic,
    /// Never writes a result; sleeps past every deadline.
    Silent,
}

impl BaselineScenario {
    /// Every scenario, in charter-matrix order.
    pub const ALL: &'static [Self] = &[
        Self::HonestSuccess,
        Self::LyingDefect,
        Self::SubtleDefect,
        Self::CompromisedCritic,
        Self::Silent,
    ];

    /// Every scenario (named accessor for the CLI's vocabulary rendering).
    pub fn all() -> &'static [Self] {
        Self::ALL
    }

    /// Exact-match parse; unknown tokens are typed errors, never coerced
    /// defaults (the child's `GAUNTLET_BASELINE_MODE` vocabulary).
    pub fn parse(token: &str) -> Result<Self, UnknownToken> {
        match token {
            "honest-success" => Ok(Self::HonestSuccess),
            "lying-defect" => Ok(Self::LyingDefect),
            "subtle-defect" => Ok(Self::SubtleDefect),
            "compromised-critic" => Ok(Self::CompromisedCritic),
            "silent" => Ok(Self::Silent),
            other => Err(UnknownToken {
                input: other.to_owned(),
                vocabulary: SCENARIO_VOCABULARY,
            }),
        }
    }

    /// The wire token (also the fixture's `GAUNTLET_BASELINE_MODE` value).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::HonestSuccess => "honest-success",
            Self::LyingDefect => "lying-defect",
            Self::SubtleDefect => "subtle-defect",
            Self::CompromisedCritic => "compromised-critic",
            Self::Silent => "silent",
        }
    }
}

const SCENARIO_VOCABULARY: &str =
    "honest-success, lying-defect, subtle-defect, compromised-critic, silent";

/// An unknown profile/scenario token — the error names the token AND the
/// accepted vocabulary, so an operator typo is diagnosable from the message
/// alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownToken {
    pub input: String,
    pub vocabulary: &'static str,
}

impl fmt::Display for UnknownToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "token {:?} is outside the typed vocabulary ({}) — exact match \
             required, no coercion",
            self.input, self.vocabulary
        )
    }
}

impl std::error::Error for UnknownToken {}

/// Why a `--profiles` list refused — typed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileListError {
    /// A member token is not in the profile vocabulary.
    Unknown(UnknownToken),
    /// A comma-separated segment is empty (`"A,,C"`, `"A,"`, `""`) — a
    /// silently dropped segment would measure fewer profiles than declared.
    EmptySegment { at: usize },
}

impl fmt::Display for ProfileListError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unknown(unknown) => write!(f, "{unknown}"),
            Self::EmptySegment { at } => write!(
                f,
                "the profile list has an empty segment at byte {at} — every \
                 comma-separated segment must name a profile"
            ),
        }
    }
}

impl std::error::Error for ProfileListError {}

/// Parses the CLI's comma-separated `--profiles` list. Dedup-FREE: the
/// list is honored exactly as written (a duplicated profile would run its
/// legs twice — that is the operator's declared intent, not something this
/// parser silently collapses). Empty segments are refused.
pub fn parse_profile_list(spec: &str) -> Result<Vec<BaselineProfile>, ProfileListError> {
    let mut profiles = Vec::new();
    let mut offset = 0usize;
    for segment in spec.split(',') {
        let trimmed = segment.trim();
        if trimmed.is_empty() {
            return Err(ProfileListError::EmptySegment { at: offset });
        }
        profiles.push(BaselineProfile::parse(trimmed).map_err(ProfileListError::Unknown)?);
        offset += segment.len() + 1;
    }
    Ok(profiles)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// T4 (profile half): the full vocabulary parses by its exact tokens.
    #[test]
    fn every_profile_token_parses() {
        for profile in BaselineProfile::ALL {
            assert_eq!(
                BaselineProfile::parse(profile.as_str()).expect("vocabulary token"),
                *profile
            );
        }
        assert_eq!(BaselineProfile::all().len(), 4);
    }

    /// T4: unknown, lowercase, and padded tokens are refused — a typed
    /// error naming the token and the vocabulary, never a coerced default.
    #[test]
    fn unknown_profile_tokens_are_refused_fail_closed() {
        for bad in ["E", "a", " A", "A ", "builder", ""] {
            let error = BaselineProfile::parse(bad).expect_err(bad);
            assert_eq!(error.input, bad);
            assert_eq!(error.vocabulary, "A, B, C, D");
            assert!(
                error.to_string().contains(bad),
                "the error names the offending token: {error}"
            );
        }
    }

    #[test]
    fn every_scenario_token_parses() {
        for scenario in BaselineScenario::ALL {
            assert_eq!(
                BaselineScenario::parse(scenario.as_str()).expect("vocabulary token"),
                *scenario
            );
        }
        assert_eq!(BaselineScenario::all().len(), 5);
    }

    /// T4 (scenario half): unknown tokens refused, naming the vocabulary.
    #[test]
    fn unknown_scenario_tokens_are_refused() {
        for bad in ["honest", "HONEST-SUCCESS", "lying_defect", "silent ", ""] {
            let error = BaselineScenario::parse(bad).expect_err(bad);
            assert_eq!(error.input, bad);
            assert!(error.to_string().contains("honest-success"));
        }
    }

    #[test]
    fn profile_list_parses_in_order_without_dedup() {
        let parsed = parse_profile_list("A,B,C,D").expect("the plan gate's exact list shape");
        assert_eq!(
            parsed,
            vec![
                BaselineProfile::A,
                BaselineProfile::B,
                BaselineProfile::C,
                BaselineProfile::D
            ]
        );
        // Dedup-free: duplicates are honored as written, not collapsed.
        let duplicated = parse_profile_list("A,A").expect("duplicates are honored");
        assert_eq!(duplicated, vec![BaselineProfile::A, BaselineProfile::A]);
        // Whitespace around a member is not a token.
        assert_eq!(
            parse_profile_list(" B ,D").expect("trimmed members"),
            vec![BaselineProfile::B, BaselineProfile::D]
        );
    }

    #[test]
    fn profile_list_refuses_empty_segments_and_unknown_members() {
        for bad in ["", "A,,C", "A,", ",A", ","] {
            let error = parse_profile_list(bad).expect_err(bad);
            assert!(
                matches!(error, ProfileListError::EmptySegment { .. }),
                "{bad} is an empty-segment refusal, got {error}"
            );
        }
        let error = parse_profile_list("A,Z").expect_err("Z");
        assert!(matches!(
            error,
            ProfileListError::Unknown(UnknownToken { input, .. }) if input == "Z"
        ));
    }
}
