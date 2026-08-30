//! The generic CLI agent adapter's typed configuration (spec §8.1:
//! configuration is never a literal in logic; plan Task 16).
//!
//! The adapter is driven ENTIRELY by a configured command template: the
//! operator names, in `GAUNTLET_CLI_AGENT_CMD`, how any headless coding
//! agent is invoked as a subprocess (spec §4.1 `cli-agent`). There is no
//! default template on purpose — a default would be a silently selected
//! product, and selection is configuration, not code. A missing template
//! is a typed [`ConfigError::MissingCommand`], never a fallback.
//!
//! - `command_template` is shell-word PARSED into argv at admission
//!   (never shell-interpolated; see the `template` module).
//! - `max_depth` maps onto [`RunnerCapabilities::max_depth`] and defaults
//!   to the §8.6 conservative value: delegation is raised deliberately,
//!   never by default. This adapter spawns NO children of its own, so a
//!   depth of 0 would also be honest; the conservative §8.6 default is
//!   kept so the value is raised, not lowered, by configuration.
//! - `cancel_span_seconds` bounds one cancellation's blocking stop
//!   operation past the caller's bound (§6 preamble: bounded blocking
//!   control operations, never unbounded ones).

use std::fmt;

use crate::template::{Template, TemplateError, parse_template};

/// The environment variable carrying the command template (spec §4.1).
pub use crate::COMMAND_ENV_VAR;

/// The §8.6 conservative recursion default.
pub const DEFAULT_MAX_DEPTH: u8 = 1;

/// The bounded span, in seconds, a cancellation's own stop operation adds
/// past the caller's bound.
pub const DEFAULT_CANCEL_SPAN_SECONDS: u64 = 60;

/// Configuration failures — typed, carrying the locus (spec §21.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigError {
    /// `GAUNTLET_CLI_AGENT_CMD` is not set. There is no default template:
    /// selection is configuration, not code.
    MissingCommand { var: &'static str },
    /// The variable is set but cannot be read as Unicode.
    InvalidCommand { var: &'static str, detail: String },
    /// The template does not parse (see [`TemplateError`] for the loci).
    Template(TemplateError),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingCommand { var } => write!(
                f,
                "{var} is not set — the generic CLI adapter is driven entirely by its \
                 configured command template and has no default"
            ),
            Self::InvalidCommand { var, detail } => {
                write!(f, "{var} is not readable as Unicode: {detail}")
            }
            Self::Template(error) => write!(f, "the command template is invalid: {error}"),
        }
    }
}

impl std::error::Error for ConfigError {}

impl From<TemplateError> for ConfigError {
    fn from(error: TemplateError) -> Self {
        Self::Template(error)
    }
}

/// The generic CLI adapter's typed configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliAgentConfig {
    /// The raw command template (shell-word parsed at admission).
    pub command_template: String,
    /// Recursion depth this runner reports (§6.1 `max_depth`).
    pub max_depth: u8,
    /// The bounded span, in seconds, one cancellation's stop adds past
    /// the caller's bound.
    pub cancel_span_seconds: u64,
}

impl CliAgentConfig {
    /// A configuration over the given raw template, with conservative
    /// defaults for the depth and cancel span.
    pub fn new(command_template: impl Into<String>) -> Self {
        Self {
            command_template: command_template.into(),
            max_depth: DEFAULT_MAX_DEPTH,
            cancel_span_seconds: DEFAULT_CANCEL_SPAN_SECONDS,
        }
    }

    /// Reads the configuration from the process environment.
    pub fn from_env() -> Result<Self, ConfigError> {
        let template = match std::env::var(crate::COMMAND_ENV_VAR) {
            Ok(template) => template,
            Err(std::env::VarError::NotPresent) => {
                return Err(ConfigError::MissingCommand {
                    var: crate::COMMAND_ENV_VAR,
                });
            }
            Err(error) => {
                return Err(ConfigError::InvalidCommand {
                    var: crate::COMMAND_ENV_VAR,
                    detail: error.to_string(),
                });
            }
        };
        Ok(Self::new(template))
    }

    /// The parsed template — the typed configuration probe (plan Task 16,
    /// doctor §8.5 check 3): a template that parses carries only known
    /// placeholders, whole-word each, with balanced quoting. The doctor's
    /// configuration-kind probe is exactly this parse; it renders no
    /// values and touches no binary.
    pub fn parsed_template(&self) -> Result<Template, TemplateError> {
        parse_template(&self.command_template)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_env_var_is_a_typed_missing_command() {
        // No default exists: an unset variable is the typed refusal.
        let error = std::env::var("GAUNTLET_CLI_AGENT_TEST_UNSET_VARIABLE").unwrap_err();
        assert!(matches!(error, std::env::VarError::NotPresent));
        let config = ConfigError::MissingCommand {
            var: "GAUNTLET_CLI_AGENT_CMD",
        };
        assert!(config.to_string().contains("no default"));
    }

    #[test]
    fn defaults_are_conservative_and_the_probe_is_the_parse() {
        let config = CliAgentConfig::new("agent {prompt_file} {result_path}");
        assert_eq!(config.max_depth, DEFAULT_MAX_DEPTH);
        assert_eq!(config.cancel_span_seconds, DEFAULT_CANCEL_SPAN_SECONDS);
        // The configuration probe (doctor §8.5 check 3, RES-D6) IS the
        // typed parse: known placeholders, whole-word, balanced quotes.
        assert!(config.parsed_template().is_ok());
        let hostile = CliAgentConfig::new("agent {fabricated}");
        assert!(hostile.parsed_template().is_err());
    }
}
