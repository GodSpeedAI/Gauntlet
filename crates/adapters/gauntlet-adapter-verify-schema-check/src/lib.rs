//! Schema-check verifier adapter (spec §4.1, §14; plan Task 11): the §14.2
//! ladder's SECOND rung — "schema check". A claim whose evidence
//! requirement is a structured document (kind `scan`) at a locator is
//! decided by parsing the document and checking the configured required
//! keys are present.
//!
//! The required-keys set is a CONSTRUCTOR parameter — the schema contract
//! is configuration resolved by the composition root (spec §7.1), never a
//! literal policy inside the instrument.
//!
//! PASS iff the artifact parses as a JSON object carrying every required
//! key. FAIL iff it parses but a required key is absent (present-but-wrong
//! evidence is a decision). Absent evidence is the typed
//! [`VerifierError::EvidenceMissing`] (P12 at the instrument layer);
//! unparseable evidence is [`VerifierError::InstrumentFailed`] — the
//! instrument could not decide, and it says so.

use std::path::{Path, PathBuf};

use gauntlet_domain::{EvidenceKind, EvidenceRequirement, InstrumentName, Outcome};
use gauntlet_ports::json::JsonValue;
use gauntlet_ports::observer::InstrumentIdentity;
use gauntlet_ports::verifier::{VerificationRequest, Verifier, VerifierError};

/// The wrapper instrument's name — the capability is the §14.2 schema rung.
pub const INSTRUMENT_NAME: &str = "gauntlet-verify-schema-check";

/// The evidence kinds this instrument conclusively decides.
pub const DECIDES: &[EvidenceKind] = &[EvidenceKind::Scan];

/// The schema verifier: one workspace root plus the required-keys contract.
pub struct SchemaCheckVerifier {
    root: PathBuf,
    required_keys: Vec<String>,
}

impl SchemaCheckVerifier {
    /// Binds the verifier to a workspace root and a required-keys
    /// contract (configuration; at least one key — a schema with no
    /// required keys checks nothing and is a wiring error).
    pub fn new(root: PathBuf, required_keys: Vec<String>) -> Result<Self, VerifierError> {
        if required_keys.is_empty() {
            return Err(VerifierError::RequirementUnsupported {
                detail: "a schema check with no required keys checks nothing".to_owned(),
            });
        }
        let canonical =
            std::fs::canonicalize(&root).map_err(|error| VerifierError::InstrumentFailed {
                detail: format!(
                    "the schema verifier's root {:?} does not resolve: {error}",
                    root.display()
                ),
            })?;
        Ok(Self {
            root: canonical,
            required_keys,
        })
    }

    /// The well-formed PASSING artifact for a required-keys contract: a
    /// JSON object carrying every required key.
    pub fn passing_artifact(required_keys: &[String]) -> Vec<u8> {
        let fields: Vec<String> = required_keys
            .iter()
            .map(|key| format!("\"{key}\":\"present\""))
            .collect();
        format!("{{{}}}", fields.join(",")).into_bytes()
    }

    fn root(&self) -> &Path {
        &self.root
    }
}

impl Verifier for SchemaCheckVerifier {
    fn verify(
        &self,
        claim: &VerificationRequest,
        _deadline: gauntlet_ports::deadline::Deadline,
    ) -> Result<gauntlet_domain::VerificationRecord, VerifierError> {
        let path = contained_path(self.root(), claim.evidence.as_str())?;
        let raw = std::fs::read(&path).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                VerifierError::EvidenceMissing {
                    locator: claim.evidence.clone(),
                }
            } else {
                VerifierError::InstrumentFailed {
                    detail: format!("reading the schema evidence failed: {error}"),
                }
            }
        })?;
        let text = std::str::from_utf8(&raw).map_err(|_| VerifierError::InstrumentFailed {
            detail: "the schema evidence is not UTF-8".to_owned(),
        })?;
        let document =
            gauntlet_ports::json::parse(text).map_err(|error| VerifierError::InstrumentFailed {
                detail: format!("the schema evidence does not parse as JSON: {error}"),
            })?;
        let JsonValue::Object(_) = document else {
            return Err(VerifierError::InstrumentFailed {
                detail: "the schema evidence is not a JSON object".to_owned(),
            });
        };
        let outcome = if self
            .required_keys
            .iter()
            .all(|key| document.get(key).is_some())
        {
            Outcome::Pass
        } else {
            Outcome::Fail
        };
        Ok(gauntlet_domain::VerificationRecord::new(
            claim.id.clone(),
            claim.run_id.clone(),
            claim.unit_id.clone(),
            gauntlet_domain::EvidenceRef::new(claim.evidence.as_str())
                .expect("a locator string is a valid ref string"),
            outcome,
            InstrumentName::new(INSTRUMENT_NAME).expect("the adapter's own name is a plain token"),
        ))
    }

    fn is_deterministic(&self) -> bool {
        true
    }

    fn instrument(&self) -> InstrumentIdentity {
        InstrumentIdentity::new(
            InstrumentName::new(INSTRUMENT_NAME).expect("plain token"),
            env!("CARGO_PKG_VERSION").to_owned(),
        )
    }

    fn supports(&self, requirement: &EvidenceRequirement) -> bool {
        DECIDES.contains(&requirement.kind())
    }
}

/// Resolves a locator under the root, refusing escapes.
fn contained_path(root: &Path, locator: &str) -> Result<PathBuf, VerifierError> {
    if locator.starts_with('/') || locator.split('/').any(|segment| segment == "..") {
        return Err(VerifierError::InstrumentFailed {
            detail: format!("locator {locator:?} escapes the verifier's root"),
        });
    }
    Ok(root.join(locator))
}

/// Builds isolated verifiers over per-build scratch roots (the observe-test
/// factory pattern).
#[derive(Default)]
pub struct SchemaCheckVerifierFactory {
    roots: std::cell::RefCell<Vec<PathBuf>>,
    next: std::cell::Cell<u64>,
}

impl SchemaCheckVerifierFactory {
    pub fn new() -> Self {
        Self::default()
    }

    /// The required-keys contract the suite's verifiers run under: the
    /// single key every scanned document in the suite fixtures carries.
    pub const SUITE_REQUIRED_KEYS: &'static [&str] = &["findings"];
}

impl Drop for SchemaCheckVerifierFactory {
    fn drop(&mut self) {
        for root in self.roots.borrow().iter() {
            let _ = std::fs::remove_dir_all(root);
        }
    }
}

impl gauntlet_ports::contract::verifier::VerifierFactory for SchemaCheckVerifierFactory {
    fn label(&self) -> &'static str {
        INSTRUMENT_NAME
    }

    fn build(
        &self,
        present: Vec<gauntlet_domain::EvidenceLocator>,
    ) -> Result<Box<dyn Verifier>, String> {
        let seq = self.next.get();
        self.next.set(seq.wrapping_add(1));
        let root = std::env::temp_dir().join(format!(
            "gauntlet-verify-schema-{}-{seq}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root)
            .map_err(|err| format!("creating the suite scratch root failed: {err}"))?;
        let keys: Vec<String> = Self::SUITE_REQUIRED_KEYS
            .iter()
            .map(|key| (*key).to_owned())
            .collect();
        for locator in &present {
            if let Some(parent) = contained_path(&root, locator.as_str())
                .map_err(|err| err.to_string())?
                .parent()
                .map(Path::to_path_buf)
            {
                std::fs::create_dir_all(&parent)
                    .map_err(|err| format!("materializing the locator parent failed: {err}"))?;
            }
            std::fs::write(
                contained_path(&root, locator.as_str()).map_err(|err| err.to_string())?,
                SchemaCheckVerifier::passing_artifact(&keys),
            )
            .map_err(|err| format!("writing the passing artifact failed: {err}"))?;
        }
        self.roots.borrow_mut().push(root.clone());
        Ok(Box::new(
            SchemaCheckVerifier::new(root, keys).map_err(|err| err.to_string())?,
        ))
    }

    fn deterministic(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gauntlet_domain::{RunId, UnitId};

    const ULID: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";

    struct Scratch(PathBuf);

    impl Scratch {
        fn new(tag: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "gauntlet-verify-schema-test-{tag}-{}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).expect("scratch root");
            Self(path)
        }

        fn root(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn request(locator: &str) -> VerificationRequest {
        VerificationRequest {
            id: gauntlet_domain::VerificationRef::new("vrf-000042").expect("ref"),
            run_id: RunId::new(ULID).expect("run"),
            unit_id: UnitId::new("AK-0005").expect("unit"),
            claim: "the document conforms".to_owned(),
            requirement: EvidenceRequirement::new(
                EvidenceKind::Scan,
                gauntlet_domain::EvidenceLocator::new(locator).expect("locator"),
                gauntlet_domain::Producer::Instrument,
            ),
            evidence: gauntlet_domain::EvidenceLocator::new(locator).expect("locator"),
        }
    }

    fn bound() -> gauntlet_ports::deadline::Deadline {
        gauntlet_ports::deadline::Deadline::at_epoch(gauntlet_domain::EpochSeconds::new(
            1_700_000_060,
        ))
    }

    #[test]
    fn a_conforming_document_passes_and_a_missing_key_fails() {
        let scratch = Scratch::new("conform");
        let keys = vec!["findings".to_owned(), "scanner".to_owned()];
        std::fs::write(
            scratch.root().join("report.json"),
            SchemaCheckVerifier::passing_artifact(&keys),
        )
        .expect("writes");
        std::fs::write(
            scratch.root().join("incomplete.json"),
            b"{\"findings\":\"present\"}",
        )
        .expect("writes");
        let verifier = SchemaCheckVerifier::new(scratch.root().to_path_buf(), keys).expect("binds");
        assert_eq!(
            verifier
                .verify(&request("report.json"), bound())
                .expect("decides")
                .outcome(),
            Outcome::Pass
        );
        assert_eq!(
            verifier
                .verify(&request("incomplete.json"), bound())
                .expect("decides")
                .outcome(),
            Outcome::Fail
        );
    }

    #[test]
    fn a_document_that_is_not_json_is_an_instrument_failure() {
        let scratch = Scratch::new("notjson");
        std::fs::write(scratch.root().join("doc"), b"{not json").expect("writes");
        let verifier =
            SchemaCheckVerifier::new(scratch.root().to_path_buf(), vec!["findings".to_owned()])
                .expect("binds");
        assert!(matches!(
            verifier.verify(&request("doc"), bound()),
            Err(VerifierError::InstrumentFailed { .. })
        ));
    }

    #[test]
    fn an_empty_required_keys_contract_is_refused_at_wiring() {
        let scratch = Scratch::new("emptykeys");
        assert!(matches!(
            SchemaCheckVerifier::new(scratch.root().to_path_buf(), Vec::new()),
            Err(VerifierError::RequirementUnsupported { .. })
        ));
    }
}
