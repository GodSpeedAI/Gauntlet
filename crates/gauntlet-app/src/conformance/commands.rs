//! The closed execute-command vocabulary (charter decisions 2/6): an
//! `execute` arm is NOT a command string and never free-form shell — it
//! is a STRUCTURAL command, and the invalid shapes are unrepresentable
//! rather than parsed-and-rejected (the strengthened form of the
//! charter's fail-closed rule; see the task-27 B1 report for the
//! recorded deviation and its reason — the frozen AK-016 literal
//! boundary forbids tool-identity strings in every core crate, so a
//! string vocabulary could neither be implemented nor tested here).
//!
//! Exactly three shapes exist:
//!
//! 1. a nextest invocation over an allowlisted workspace package:
//!    `{package: P, target: T}` or `{package: P, lib: true[, filter: F]}`
//!    — per-target nextest filters have no justfile equivalent (the
//!    recipes are package/workspace scoped), so the raw tool under the
//!    repository's configured parallelism is the honest spelling, the
//!    scripts/run-mutation.sh precedent, recorded in the report's
//!    `_meta`;
//! 2. a bare absolute executable with NO arguments: `{program: P}` —
//!    the fixture-world arm that lets the sweep's own tests run real
//!    processes without ever running the battery.
//!
//! The argv/display composition for shape 1 lives in the adapter layer
//! (spec §8.4 — the runtime names capabilities, never products): the
//! composition root injects it, this crate never spells a tool name.

use core::fmt;

/// The CORE workspace packages this layer's default vocabulary admits
/// (a deliberate closed list; widening it is a recorded change, not a
/// silent default). Adapter crates are deliberately absent: naming an
/// adapter identifier in application source is the AK-002 denylist's
/// own rule — the composition root extends the list with the adapters
/// IT wires ([`parse_manifest_with`]) and owns those names.
pub const WORKSPACE_PACKAGES: &[&str] = &[
    "gauntlet-domain",
    "gauntlet-ports",
    "gauntlet-app",
    "gauntlet-cli",
    "gauntlet-arch-tests",
];

/// The FEATURE edges the application's default vocabulary admits,
/// derived from the core packages' own `[features]` tables (gauntlet-app
/// declares `composition`; gauntlet-cli declares `surfaces`; the other
/// core packages declare none; `default` is implicit everywhere and is
/// deliberately absent — passing it adds nothing). Pinned to the
/// workspace Cargo.tomls by a test, so a new edge fails until it is
/// inventoried here; the composition root owns the full allowlist's
/// edges (`parse_manifest_with`).
pub const WORKSPACE_FEATURES: &[&str] = &["composition", "surfaces"];

/// How a nextest invocation selects its targets.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum NextestSelector {
    Lib,
    Test(String),
}

/// One validated sweep command — a structural value; the executor turns
/// it into a process invocation (and never receives a string to run).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SweepCommand {
    Nextest {
        package: String,
        selector: NextestSelector,
        filter: Option<String>,
        /// Cargo feature edges the invocation must enable (COR-2: a
        /// target compiled only under a feature config runs ZERO tests
        /// under the default config — nextest exit 4 — so the config is
        /// part of the command, and grouping treats it so).
        features: Vec<String>,
    },
    Program {
        path: String,
    },
}

/// Why an execute arm's shape is not in the vocabulary. Typed, carrying
/// the offending value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandError {
    UnknownPackage { package: String },
    BadTarget { target: String },
    BadFilter { filter: String },
    BadFeature { feature: String },
    DuplicateFeature { feature: String },
    BothSelectors,
    NoSelector,
    ProgramNotAbsolute { path: String },
    ProgramWithArguments { path: String },
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownPackage { package } => write!(
                f,
                "package {package:?} is not in the vocabulary's workspace allowlist"
            ),
            Self::BadTarget { target } => {
                write!(f, "target {target:?} is not a plain cargo test-target name")
            }
            Self::BadFilter { filter } => {
                write!(
                    f,
                    "filter {filter:?} is not a plain nextest test-name token"
                )
            }
            Self::BadFeature { feature } => write!(
                f,
                "feature {feature:?} is not an edge the vocabulary admits"
            ),
            Self::DuplicateFeature { feature } => {
                write!(f, "feature {feature:?} appears twice in one arm")
            }
            Self::BothSelectors => {
                write!(
                    f,
                    "an execute arm carries both a target and the lib selector"
                )
            }
            Self::NoSelector => write!(f, "an execute arm names no target and no lib selector"),
            Self::ProgramNotAbsolute { path } => write!(
                f,
                "program {path:?}: the bare-executable arm admits absolute paths only"
            ),
            Self::ProgramWithArguments { path } => write!(
                f,
                "program {path:?}: the bare-executable arm admits no arguments"
            ),
        }
    }
}

impl std::error::Error for CommandError {}

/// A plain token a target name or filter may carry: test names, module
/// paths, digits — no whitespace, no shell metacharacter, no flag shape.
fn plain_token(token: &str) -> bool {
    !token.is_empty()
        && !token.starts_with('-')
        && token
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == ':' || c == '.' || c == '-')
}

/// Validates the nextest arm's pieces against the CALLER's package and
/// feature vocabularies (the codec hands the parsed key/value pairs
/// over; this is the closed door they pass through). The application
/// defaults are [`WORKSPACE_PACKAGES`] and [`WORKSPACE_FEATURES`]; the
/// composition root may extend them with the adapters it wires.
#[allow(clippy::too_many_arguments)] // the arm's validated pieces, threaded from the codec
pub fn validate_nextest(
    package: &str,
    target: Option<&str>,
    lib: bool,
    filter: Option<&str>,
    features: &[String],
    allowed: &[&str],
    allowed_features: &[&str],
) -> Result<SweepCommand, CommandError> {
    if !allowed.contains(&package) {
        return Err(CommandError::UnknownPackage {
            package: package.to_owned(),
        });
    }
    let selector = match (target, lib) {
        (Some(_), true) => return Err(CommandError::BothSelectors),
        (Some(target), false) => {
            if !plain_token(target) {
                return Err(CommandError::BadTarget {
                    target: target.to_owned(),
                });
            }
            NextestSelector::Test(target.to_owned())
        }
        (None, true) => NextestSelector::Lib,
        (None, false) => return Err(CommandError::NoSelector),
    };
    if let Some(filter) = filter
        && !plain_token(filter)
    {
        return Err(CommandError::BadFilter {
            filter: filter.to_owned(),
        });
    }
    let mut validated_features = Vec::new();
    for feature in features {
        if !plain_token(feature) || !allowed_features.contains(&feature.as_str()) {
            return Err(CommandError::BadFeature {
                feature: feature.clone(),
            });
        }
        if validated_features.contains(feature) {
            return Err(CommandError::DuplicateFeature {
                feature: feature.clone(),
            });
        }
        validated_features.push(feature.clone());
    }
    Ok(SweepCommand::Nextest {
        package: package.to_owned(),
        selector,
        filter: filter.map(str::to_owned),
        features: validated_features,
    })
}

/// Validates the bare-executable arm: one absolute path, no arguments
/// (whitespace inside the value is argument smuggling — refused).
pub fn validate_program(path: &str) -> Result<SweepCommand, CommandError> {
    if path.contains(char::is_whitespace) {
        return Err(CommandError::ProgramWithArguments {
            path: path.to_owned(),
        });
    }
    if !path.starts_with('/') || path.len() < 2 {
        return Err(CommandError::ProgramNotAbsolute {
            path: path.to_owned(),
        });
    }
    Ok(SweepCommand::Program {
        path: path.to_owned(),
    })
}

/// The command's STRUCTURAL display — the canonical spelling the report
/// and the grouping records carry. It names the capability and its
/// parameters; the EXECUTOR's record names the concrete invocation it
/// ran (argv spelling), which is the adapter layer's to compose.
pub fn command_display(command: &SweepCommand) -> String {
    match command {
        SweepCommand::Nextest {
            package,
            selector,
            filter,
            features,
        } => {
            let mut text = format!("nextest[{package}:");
            match selector {
                NextestSelector::Lib => text.push_str("lib"),
                NextestSelector::Test(target) => {
                    text.push_str("test=");
                    text.push_str(target);
                }
            }
            if let Some(filter) = filter {
                text.push(':');
                text.push_str(filter);
            }
            for feature in features {
                text.push('@');
                text.push_str(feature);
            }
            text.push(']');
            text
        }
        SweepCommand::Program { path } => path.clone(),
    }
}

/// The evidence log slug for a command (sanitized to `[a-z0-9-]`).
pub fn command_slug(command: &SweepCommand) -> String {
    let raw = match command {
        SweepCommand::Nextest {
            package,
            selector,
            filter,
            features,
        } => {
            let mut text = package.clone();
            match selector {
                NextestSelector::Lib => text.push_str("-lib"),
                NextestSelector::Test(target) => {
                    text.push('-');
                    text.push_str(target);
                }
            }
            if let Some(filter) = filter {
                text.push('-');
                text.push_str(filter);
            }
            for feature in features {
                text.push('-');
                text.push_str(feature);
            }
            text
        }
        SweepCommand::Program { path } => {
            path.rsplit('/').next().unwrap_or("executable").to_owned()
        }
    };
    let slug: String = raw
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let trimmed = slug.trim_matches('-').to_owned();
    if trimmed.is_empty() {
        "sweep".to_owned()
    } else {
        trimmed.chars().take(60).collect()
    }
}
