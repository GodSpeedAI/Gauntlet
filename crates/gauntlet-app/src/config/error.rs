//! Configuration failures — typed, carrying the deciding key or variable
//! name, never a secret value (spec §8.1: "validates presence, and never
//! logs the value").

use core::fmt;

use crate::config::gauntlet_md::GauntletMdError;

/// Failures resolving the effective configuration (spec §8.1/§8.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigError {
    /// The GAUNTLET.md front matter violated the grammar. The parse error
    /// carries line and column.
    FrontMatter(GauntletMdError),
    /// A `$VAR` / `${VAR}` indirection in a string value names a variable
    /// the environment does not carry. The VARIABLE NAME and the key are
    /// carried; the value is never echoed (spec §8.1).
    UnresolvedIndirection { var: String, key: String },
    /// A front-matter value's shape does not match the known-key table
    /// (e.g. a quoted string where the table wants a whole number). Kinds
    /// only — the value could be a credential.
    KindMismatch {
        key: String,
        expected: &'static str,
        found: &'static str,
    },
    /// An environment override for a numeric setting is not a whole number.
    /// The variable is named; the raw text is not echoed.
    EnvNotANumber { name: String, key: String },
    /// An environment override for a boolean setting is neither of the two
    /// accepted spellings.
    EnvNotABoolean { name: String, key: String },
    /// A CLI flag carried a value of the wrong kind for its key — a
    /// programming error at the call site, surfaced as data.
    FlagKind { key: String },
    /// A setting landed outside its value domain during typed conversion
    /// (e.g. a budget ceiling that does not fit its integer type).
    ValueOutOfRange { key: String },
    /// The config contract exists at its discovered path but could not be
    /// READ — permission denied, invalid encoding, or another io failure
    /// class. NOT-FOUND is never this error: a missing file is the honest
    /// "no contract" state; an unreadable one is a failure whose locus is
    /// preserved instead of silently dropping the config layer. Carries the
    /// file leaf and the io error KIND — never file contents.
    ContractUnreadable {
        leaf: String,
        kind: std::io::ErrorKind,
    },
    /// The operator explicitly named the contract with `--config <path>`
    /// and that path does not exist. A missing DISCOVERED contract is the
    /// honest "no contract" state; a missing EXPLICITLY REQUESTED one is a
    /// usage-class failure naming the path — the operator asked for that
    /// file, not for discovery, so its absence must never fall through to
    /// the defaults. The path is operator-supplied text, not a secret.
    ExplicitConfigMissing { path: String },
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FrontMatter(inner) => write!(f, "configuration: {inner}"),
            Self::UnresolvedIndirection { var, key } => write!(
                f,
                "configuration: '{key}' references indirection '{var}' that the environment does \
                 not carry — presence is validated, the value is never printed (spec 8.1)"
            ),
            Self::KindMismatch {
                key,
                expected,
                found,
            } => write!(
                f,
                "configuration: '{key}' expects {expected} but the front matter carried {found} \
                 (spec 8.2 known-key table)"
            ),
            Self::EnvNotANumber { name, key } => write!(
                f,
                "configuration: {name} (for '{key}') is not a whole number (spec 8.3)"
            ),
            Self::EnvNotABoolean { name, key } => write!(
                f,
                "configuration: {name} (for '{key}') is neither true nor false (spec 8.3)"
            ),
            Self::FlagKind { key } => write!(
                f,
                "configuration: a CLI flag for '{key}' carried the wrong value kind (spec 8.1 \
                 flag layer)"
            ),
            Self::ValueOutOfRange { key } => write!(
                f,
                "configuration: '{key}' carries a value outside its setting's domain (spec 8.2)"
            ),
            Self::ContractUnreadable { leaf, kind } => write!(
                f,
                "configuration: contract '{leaf}' found but unreadable: {} — the config layer is \
                 not silently dropped; only a missing file is an honest absence (spec 8.2)",
                io_failure_class(*kind)
            ),
            Self::ExplicitConfigMissing { path } => write!(
                f,
                "configuration: explicitly requested config '{path}' does not exist — an \
                 operator-named contract is never treated as 'no contract'; only a missing \
                 discovered contract is an honest absence (spec 8.2)"
            ),
        }
    }
}

/// The failure-class word for an unreadable contract: the io kind's own
/// spelling, with the encoding case named for what it means here.
fn io_failure_class(kind: std::io::ErrorKind) -> String {
    match kind {
        std::io::ErrorKind::PermissionDenied => "permission denied".to_owned(),
        std::io::ErrorKind::InvalidData => "invalid data (not valid UTF-8)".to_owned(),
        other => other.to_string(),
    }
}

impl std::error::Error for ConfigError {}

impl From<GauntletMdError> for ConfigError {
    fn from(inner: GauntletMdError) -> Self {
        Self::FrontMatter(inner)
    }
}
