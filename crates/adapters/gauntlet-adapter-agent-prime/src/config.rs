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
//!   wait for the parent print run to land the admission handle — a
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

/// The rendered cell's default in-cell linger, in seconds, AFTER the
/// admission-handle write (COR-D2 divergence #4, load-bearing, verified
/// by execution): a child of a print-mode parent materializes ONLY while
/// the parent session stays alive past admission — with the cell ending
/// right after the handle write the admitted child NEVER appeared; with
/// a linger it completed its work. The linger is the PARENT's cell;
/// `invoke` still returns the moment the handle parses (D1 untouched).
pub const DEFAULT_ADMISSION_LINGER_SECONDS: u64 = 30;

/// The prime-agent adapter's typed configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrimeAdapterConfig {
    /// The harness binary, resolved through `PATH` when relative.
    pub binary: PathBuf,
    /// Recursion depth this runner reports and permits (Annex C.2).
    pub max_depth: u8,
    /// The production transport's admission bound, in seconds.
    pub admission_timeout_seconds: u64,
    /// The rendered cell's bounded in-cell linger after the handle write,
    /// in seconds (see [`DEFAULT_ADMISSION_LINGER_SECONDS`]).
    pub admission_linger_seconds: u64,
}

impl Default for PrimeAdapterConfig {
    fn default() -> Self {
        Self {
            binary: PathBuf::from(DEFAULT_BINARY_NAME),
            max_depth: DEFAULT_MAX_DEPTH,
            admission_timeout_seconds: DEFAULT_ADMISSION_TIMEOUT_SECONDS,
            admission_linger_seconds: DEFAULT_ADMISSION_LINGER_SECONDS,
        }
    }
}

impl PrimeAdapterConfig {
    /// The admission bound as a [`std::time::Duration`].
    pub fn admission_timeout(&self) -> std::time::Duration {
        std::time::Duration::from_secs(self.admission_timeout_seconds)
    }

    /// The in-cell linger as a [`std::time::Duration`].
    pub fn admission_linger(&self) -> std::time::Duration {
        std::time::Duration::from_secs(self.admission_linger_seconds)
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
        // COR-D2: the linger defaults to the evidence-cited 30 seconds.
        assert_eq!(
            config.admission_linger(),
            std::time::Duration::from_secs(DEFAULT_ADMISSION_LINGER_SECONDS)
        );
        assert_eq!(config.admission_linger_seconds, 30);
    }
}
