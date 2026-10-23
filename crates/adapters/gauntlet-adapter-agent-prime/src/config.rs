//! The prime-agent adapter's typed configuration (spec §8.1: configuration
//! is never a literal in logic; Annex C: adapter mechanics are
//! version-guarded and adapter-local).
//!
//! Every field is a plain typed value with a conservative [`Default`]:
//!
//! - the binary is a bare command NAME resolved through `PATH` at use —
//!   never an absolute path (a machine's layout is configuration, not
//!   code);
//! - `max_depth` maps the harness's recursion configuration onto
//!   [`RunnerCapabilities::max_depth`] (Annex C.2: "Map depth configuration
//!   to `max_depth` and report it") and defaults to 1 — the §8.6
//!   conservative default: "recursion is raised deliberately, never by
//!   default";
//! - `admission_timeout_seconds` bounds the production kernel transport's
//!   wait for the rpc-mode parent process to land the admission handle — a
//!   model-backed, non-deterministic round trip (recon §8), so the default
//!   is generous but bounded; absence of a configured budget is never
//!   permission for unbounded waiting (AGENTS.md).
//!
//! The supported harness version range is deliberately NOT configuration
//! here: it is a typed [`crate::probe::VersionRange`] the composition
//! root injects from the application's defaults module (correction
//! RES-D8) — a config that could move its own compatibility ruler would
//! be self-defeating (probe mismatches are `configuration` failures
//! surfaced by `gauntlet doctor`, plan Task 15 step 4).

use std::path::PathBuf;

use crate::mcp_bridge::McpBridge;

/// The Python child-spawn call dialect: version-gated support for prime-agent
/// harness evolution. Callable08 uses `await rlm(...)` (0.8 style); Spawn09
/// uses `await rlm.spawn(...)` (0.9+ style).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SpawnDialect {
    /// Prime-agent 0.8: callable model, `await rlm(prompt, name=..., model=...)`
    Callable08,
    /// Prime-agent 0.9+: spawn method, `await rlm.spawn(prompt, name=..., model=...)`
    #[default]
    Spawn09,
}

impl SpawnDialect {
    /// The call expression prefix for this dialect.
    pub fn call_expr(self) -> &'static str {
        match self {
            Self::Callable08 => "rlm(",
            Self::Spawn09 => "rlm.spawn(",
        }
    }

    /// Human-readable label for this dialect (vendor-specific, safe for adapter layer).
    pub fn label(self) -> &'static str {
        match self {
            Self::Callable08 => "Callable08",
            Self::Spawn09 => "Spawn09",
        }
    }
}

/// The configured binary's default NAME (resolved through `PATH` at use,
/// mirroring the process-isolation backend's `locate`). A name, not a
/// path: this crate contains no absolute-path literal.
pub const DEFAULT_BINARY_NAME: &str = "prime-agent";

/// The §8.6 conservative recursion default: a root may create children;
/// grandchildren require deliberate configuration.
pub const DEFAULT_MAX_DEPTH: u8 = 1;

/// The production kernel transport's default admission bound, in seconds:
/// admission is one parent print run (model-backed, non-deterministic),
/// so the default is generous — but bounded.
pub const DEFAULT_ADMISSION_TIMEOUT_SECONDS: u64 = 600;

/// The harness's agent-state directory override. Every invocation with a
/// provisioned MCP bridge is launched with this variable pointing at its own
/// per-invocation agent directory; the harness's own control calls get the
/// same value so they address the same state.
pub const AGENT_DIR_ENV: &str = "PRIME_AGENT_CODING_AGENT_DIR";

/// The prime-agent adapter's typed configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrimeAdapterConfig {
    /// The harness binary, resolved through `PATH` when relative.
    pub binary: PathBuf,
    /// Recursion depth this runner reports and permits (Annex C.2).
    pub max_depth: u8,
    /// The production transport's admission bound, in seconds.
    pub admission_timeout_seconds: u64,
    /// The Python child-spawn call dialect (version-gated support).
    pub spawn_dialect: SpawnDialect,
    /// The per-invocation MCP bridge, when this runner provisions one. `None`
    /// (the default) launches the harness exactly as before.
    pub mcp_bridge: Option<McpBridge>,
}

impl Default for PrimeAdapterConfig {
    fn default() -> Self {
        Self {
            binary: PathBuf::from(DEFAULT_BINARY_NAME),
            max_depth: DEFAULT_MAX_DEPTH,
            admission_timeout_seconds: DEFAULT_ADMISSION_TIMEOUT_SECONDS,
            spawn_dialect: SpawnDialect::default(),
            mcp_bridge: None,
        }
    }
}

impl PrimeAdapterConfig {
    /// The admission bound as a [`std::time::Duration`].
    pub fn admission_timeout(&self) -> std::time::Duration {
        std::time::Duration::from_secs(self.admission_timeout_seconds)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_conservative_and_typed() {
        let config = PrimeAdapterConfig::default();
        assert_eq!(config.binary, PathBuf::from(DEFAULT_BINARY_NAME));
        assert_eq!(config.max_depth, DEFAULT_MAX_DEPTH);
        assert_eq!(
            config.admission_timeout(),
            std::time::Duration::from_secs(DEFAULT_ADMISSION_TIMEOUT_SECONDS)
        );
        assert_eq!(config.spawn_dialect, SpawnDialect::Spawn09);
        assert!(config.mcp_bridge.is_none(), "the bridge is off by default");
    }
}
