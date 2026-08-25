//! Configuration — spec §8: precedence, the GAUNTLET.md contract, the
//! environment layer, and effective-config resolution.
//!
//! Spec §8.1, quoted where it shapes this module tree:
//!
//! - "Nothing hardcoded. No path, model, provider, command, port number,
//!   timeout, tier policy, or tool identity is a literal in domain or
//!   application code. Conservative defaults live in one declared defaults
//!   table (§8.6) and are overridable."
//! - "Three layers, one order. `built-in defaults` < `GAUNTLET.md` front
//!   matter < `.env` / process environment < CLI flags. Later wins. The
//!   effective config is digested into `Run.config_fingerprint`."
//! - "Secrets by indirection only. Config values may reference `$VAR`; the
//!   runtime resolves them, validates presence, and never logs the value.
//!   Generated files never contain credentials."
//!
//! Module map: [`defaults`] is the §8.6 table; [`gauntlet_md`] parses the
//! repository policy contract's front matter under a strict, documented
//! grammar; [`env_layer`] folds `.env` under the process environment;
//! [`resolve`] merges the four layers through one known-key table;
//! [`canonical`] is the deterministic serialization; [`fingerprint`] digests
//! it; [`error`] carries the typed failures.

pub mod canonical;
pub mod defaults;
pub mod env_layer;
pub mod error;
pub mod fingerprint;
pub mod gauntlet_md;
pub mod resolve;

pub use canonical::{CanonValue, parse_canonical, write_canonical};
pub use env_layer::{EnvLayer, EnvSource, UnknownEnv};
pub use error::ConfigError;
pub use fingerprint::{ALGO as DIGEST_ALGO, config_fingerprint, digest_canonical};
pub use gauntlet_md::{FmEntry, FmScalar, FmValue, FrontMatter, GauntletMdError};
pub use resolve::{
    BudgetSettings, ConfigKey, ControlSettings, EffectiveConfig, FlagOverrides, FlagValue,
    ReferenceSettings, ResolveInput, Resolved, ResolvedBudget, ResolvedConfig, ResolvedReference,
    RoleRunner, UnknownKey, ValueKind, discover_config_path, discover_project_root, resolve,
};

/// Locates a data-only fixture under the repository's shared fixture tree
/// for inline tests. Test scaffolding only.
#[cfg(test)]
pub(crate) fn fixture_path(name: &str) -> std::path::PathBuf {
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    // crates/<crate> -> crates -> repository root.
    let repo_root = manifest
        .ancestors()
        .nth(2)
        .expect("the manifest dir is two levels below the repository root");
    repo_root
        .join("tests")
        .join("fixtures")
        .join("config")
        .join(name)
}
