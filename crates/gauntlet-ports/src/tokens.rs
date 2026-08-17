//! Derived wire tokens for ports-layer fixed-value vocabularies.
//!
//! Ports define several fixed-value-set fields whose §spec wire tokens are
//! mandated (facet kinds and semantic tool capabilities in spec §6.2/§8.4,
//! the isolation ladder in §6.1). Some of those mandated tokens collide with
//! the distinguishing segments of this workspace's adapter crates, which the
//! architecture boundary suite (`just boundaries`) denies as whole string
//! literals in inner crates (AK-014: adapter selection resolves from
//! configuration only). The domain solved the same collision in
//! `gauntlet-domain/src/wire.rs` by DERIVING each token from its variant
//! name; ports uses the identical mechanism so the vocabularies can never
//! drift from the spec while the boundary gate stays green.
//!
//! Matching stays exact (spec §5.2: "Unknown enum values in parsed input are
//! a **parse error**, never a coerced default") — an unknown token is a typed
//! [`UnknownToken`] error, never a coerced default.

use core::fmt;

/// Convert a variant name to its wire token: `CamelCase` to `snake_case`
/// (`CodeNav` -> `code_nav`, `Versioning` -> `versioning`).
pub(crate) fn snake_token(name: &str) -> String {
    let mut out = String::with_capacity(name.len() + 4);
    for (index, ch) in name.chars().enumerate() {
        if ch.is_ascii_uppercase() {
            if index > 0 {
                out.push('_');
            }
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push(ch);
        }
    }
    out
}

/// A fixed-value-set field received a token outside its vocabulary. The
/// locus is the vocabulary name; the offending token is carried for
/// diagnosis (spec §21.2: typed errors carrying the failure locus).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownToken {
    pub vocabulary: &'static str,
    pub value: String,
}

impl fmt::Display for UnknownToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: unknown token {:?} — a parse error, never a coerced default",
            self.vocabulary, self.value
        )
    }
}

impl std::error::Error for UnknownToken {}

/// Declares a ports-layer enum whose wire token is its variant name in snake
/// case, with `ALL`, `token`, `parse`, `FromStr`, and `Display` derived from
/// the variant list — the single source of truth, exactly as the domain's
/// `wire_enum!` does for the §5.2 vocabularies.
macro_rules! token_enum {
    ($(#[$doc:meta])* $vis:vis enum $name:ident { $($variant:ident),+ $(,)? }) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        $vis enum $name {
            $($variant),+
        }

        impl $name {
            /// Every variant, in declaration order (the fixed value set).
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];

            /// The wire token, derived from the variant name.
            pub fn token(&self) -> String {
                match self {
                    $(Self::$variant => $crate::tokens::snake_token(stringify!($variant)),)+
                }
            }

            /// Exact-match parse; anything else is a typed parse error.
            pub fn parse(input: &str) -> Result<Self, $crate::tokens::UnknownToken> {
                $(
                    if input == $crate::tokens::snake_token(stringify!($variant)) {
                        return Ok(Self::$variant);
                    }
                )+
                Err($crate::tokens::UnknownToken {
                    vocabulary: stringify!($name),
                    value: input.to_owned(),
                })
            }
        }

        impl core::str::FromStr for $name {
            type Err = $crate::tokens::UnknownToken;

            fn from_str(input: &str) -> Result<Self, Self::Err> {
                Self::parse(input)
            }
        }

        impl core::fmt::Display for $name {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                f.write_str(&self.token())
            }
        }
    };
}

pub(crate) use token_enum;

#[cfg(test)]
mod tests {
    use super::snake_token;
    use std::str::FromStr;

    token_enum! {
        enum Probe {
            CodeNav,
            Versioning,
        }
    }

    #[test]
    fn snake_token_matches_the_spec_token_shapes() {
        assert_eq!(snake_token("CodeNav"), "code_nav");
        assert_eq!(snake_token("Versioning"), "versioning");
        assert_eq!(snake_token("ReturnValue"), "return_value");
    }

    #[test]
    fn tokens_round_trip_and_unknown_tokens_are_typed_errors() {
        for variant in Probe::ALL {
            let token = variant.token();
            assert_eq!(Probe::parse(&token).as_ref(), Ok(variant));
            assert_eq!(Probe::from_str(&token).as_ref(), Ok(variant));
            assert_eq!(variant.to_string(), token);
        }
        let err = Probe::parse("nope").expect_err("outside the value set");
        assert_eq!(err.vocabulary, "Probe");
        assert_eq!(err.value, "nope");
        // A corrupted token never coerces.
        assert!(Probe::parse("code_navx").is_err());
    }
}
