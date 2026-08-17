//! `config_fingerprint` — the digest of the effective configuration (spec
//! §8.1: "The effective config is digested into `Run.config_fingerprint`";
//! §5.3.3 Run contract).
//!
//! The digest runs over the RAW-TOKEN canonical serialization (plan Task 5
//! decision D4): indirection references stay `$VAR` text, so a machine's
//! secret values never enter the digested bytes. The domain `Digest` type
//! carries the algorithm (`sha256:`-prefixed hex); hashing is this module's
//! instrument job — the domain never computes one.

use gauntlet_domain::Digest;
use sha2::{Digest as _, Sha256};

use crate::config::canonical;
use crate::config::resolve::EffectiveConfig;

/// The algorithm identifier recorded with every config fingerprint.
pub const ALGO: &str = "sha256";

/// Digests the canonical serialization of `config`. Identical effective
/// configurations — however their layers were composed — yield identical
/// fingerprints; any changed value moves it.
pub fn config_fingerprint(config: &EffectiveConfig) -> Digest {
    let canonical_text = config.to_canonical();
    let hashed = Sha256::digest(canonical_text.as_bytes());
    let hex: String = hashed.iter().map(|byte| format!("{byte:02x}")).collect();
    // Established invariant, not a hope: the algorithm token is the const
    // above and the hex is 64 lowercase characters produced here, both of
    // which the domain Digest constructor accepts by construction.
    Digest::new(ALGO, &hex).unwrap_or_else(|err| {
        unreachable!("sha256 plus 64 lowercase hex characters is always a valid digest: {err}")
    })
}

/// Digests arbitrary canonical text (the same rule, for callers that already
/// hold the serialization).
pub fn digest_canonical(text: &str) -> Digest {
    let hashed = Sha256::digest(text.as_bytes());
    let hex: String = hashed.iter().map(|byte| format!("{byte:02x}")).collect();
    Digest::new(ALGO, &hex).unwrap_or_else(|err| {
        unreachable!("sha256 plus 64 lowercase hex characters is always a valid digest: {err}")
    })
}

/// Re-exported for callers building digested documents with the same writer.
pub use canonical::write_canonical;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::env_layer::EnvLayer;
    use crate::config::resolve::{ConfigKey, FlagOverrides, FlagValue, ResolveInput, resolve};
    use std::path::Path;

    const VOCAB: &[&str] = &["GAUNTLET_MAX_ROUNDS"];

    fn config_from(doc: Option<&str>, flags: FlagOverrides) -> EffectiveConfig {
        let env = EnvLayer::from_parts("", &[], VOCAB);
        let front_matter = doc.and_then(|text| crate::config::gauntlet_md::parse(text).ok());
        resolve(&ResolveInput {
            project_root: Path::new("proj-root").to_path_buf(),
            front_matter,
            env: &env,
            flags,
        })
        .expect("resolves")
        .effective
    }

    #[test]
    fn identical_configurations_digest_identically_however_built() {
        let doc = "---\nbudget:\n  max_rounds: 5\n---\n";
        let a = config_from(Some(doc), FlagOverrides::new());
        let mut flags = FlagOverrides::new();
        flags.set(ConfigKey::BudgetMaxRounds, FlagValue::WholeNumber(5));
        let b = config_from(None, flags);
        assert_eq!(config_fingerprint(&a), config_fingerprint(&b));
        assert_eq!(
            config_fingerprint(&a).to_string(),
            digest_canonical(&a.to_canonical()).to_string()
        );
    }

    #[test]
    fn any_changed_value_moves_the_fingerprint() {
        let base = config_from(None, FlagOverrides::new());
        let mut flags = FlagOverrides::new();
        flags.set(ConfigKey::BudgetMaxRounds, FlagValue::WholeNumber(6));
        let changed = config_from(None, flags);
        assert_ne!(config_fingerprint(&base), config_fingerprint(&changed));
    }

    #[test]
    fn the_digest_is_the_domain_shape_and_carries_no_secret_values() {
        let env = EnvLayer::from_parts(
            "",
            &[("TASK5_CANARY".to_owned(), "canary-value-xyz".to_owned())],
            VOCAB,
        );
        let doc = "---\nreference:\n  answer_key: key-$TASK5_CANARY.md\n---\n";
        let fm = crate::config::gauntlet_md::parse(doc).ok();
        let config = resolve(&ResolveInput {
            project_root: Path::new("proj-root").to_path_buf(),
            front_matter: fm,
            env: &env,
            flags: FlagOverrides::new(),
        })
        .expect("resolves")
        .effective;
        let digest = config_fingerprint(&config);
        let text = digest.to_string();
        assert!(text.starts_with("sha256:"));
        assert_eq!(text.len(), "sha256:".len() + 64);
        assert!(!text.contains("canary"));
        assert!(!text.contains("value"));
    }
}
