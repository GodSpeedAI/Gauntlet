//! `observation_digest` (spec §12.4): the ONE rule for what an observation
//! bundle's digest covers, owned by the port that consumes bundles rather
//! than by the application that produces them.
//!
//! The digest covers the normalized bundle MINUS TIMESTAMPS — `observed_at`
//! and each facet's `started_at`/`ended_at` — so the same observation
//! re-taken at a different wall-clock time digests identically. Everything
//! else is covered: facet content, claims content, and BOTH gap arrays, so a
//! hidden failure changes the digest (tamper-evidence). That is a different
//! concern from claims being excluded from VERIFICATION inputs (I2): the
//! digest covers the claims, the comparison envelope never carries them.
//!
//! The document's own `observation_digest` member is self-referential, so
//! the normalized form pins it to the empty string before hashing.
//!
//! This lives in ports because the comparison chain must be able to
//! RE-DERIVE the digest from the bound bundle's bytes. A digest the bundle
//! merely declares about itself is not evidence of anything; the reference
//! side of the envelope has always been verified this way, and this makes
//! the observation side symmetric.

use gauntlet_domain::Digest;
use sha2::{Digest as _, Sha256};

use crate::json::JsonValue;

/// The normalized canonical bytes the digest is taken over. Exposed so
/// producers and tests can pin the normalization directly.
pub fn normalized_bytes(document: &JsonValue) -> String {
    let JsonValue::Object(entries) = document else {
        return document.to_json_text();
    };
    let normalized: Vec<(String, JsonValue)> = entries
        .iter()
        .filter_map(|(key, value)| match key.as_str() {
            "observed_at" => None,
            "observation_digest" => Some((key.clone(), JsonValue::Str(String::new()))),
            "facets" => Some((key.clone(), strip_facet_timestamps(value))),
            _ => Some((key.clone(), value.clone())),
        })
        .collect();
    JsonValue::Object(normalized).to_json_text()
}

/// Removes `started_at`/`ended_at` from every facet object, in place of the
/// full array.
fn strip_facet_timestamps(facets: &JsonValue) -> JsonValue {
    let JsonValue::Array(items) = facets else {
        return facets.clone();
    };
    let stripped: Vec<JsonValue> = items
        .iter()
        .map(|facet| {
            let JsonValue::Object(entries) = facet else {
                return facet.clone();
            };
            JsonValue::Object(
                entries
                    .iter()
                    .filter(|(key, _)| key != "started_at" && key != "ended_at")
                    .cloned()
                    .collect(),
            )
        })
        .collect();
    JsonValue::Array(stripped)
}

/// The §12.4 digest: SHA-256 over [`normalized_bytes`]. Infallible by
/// construction — SHA-256 output is 64 lowercase hex characters, exactly the
/// domain's hex grammar.
pub fn observation_digest(document: &JsonValue) -> Digest {
    let hex: String = Sha256::digest(normalized_bytes(document).as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    Digest::new("sha256", &hex).expect("SHA-256 hex satisfies the Digest grammar by construction")
}
