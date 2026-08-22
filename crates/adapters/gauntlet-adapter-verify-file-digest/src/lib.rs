//! File-digest verifier adapter (spec §4.1, §14; plan Task 11): the §14.2
//! ladder's FIRST rung — "file/digest check". A claim whose evidence
//! requirement is a `file` at a locator is decided by re-deriving the
//! content digest and comparing it against the digest the evidence names.
//!
//! The artifact contract (the wrapper IS the instrument, the observe-
//! adapter convention): the evidence at `evidence_required.locator` is a
//! two-line digest manifest —
//!
//! ```text
//! sha256:<hex-of-content>
//! <relative path of the content, under the verifier's root>
//! ```
//!
//! PASS iff SHA-256 of the named content equals the declared digest.
//! FAIL iff it does not (the evidence is present and WRONG — that is a
//! decision, not an error). Absent evidence is the typed
//! [`VerifierError::EvidenceMissing`] (P12 at the instrument layer; the
//! application's verification chain converts it to
//! INCONCLUSIVE/`evidence_unresolvable`). A manifest that is not a
//! two-line digest manifest is [`VerifierError::InstrumentFailed`] — the
//! instrument could not decide, and it says so.

use std::path::{Path, PathBuf};

use gauntlet_domain::{EvidenceKind, EvidenceRequirement, InstrumentName, Outcome};
use gauntlet_ports::observer::InstrumentIdentity;
use gauntlet_ports::verifier::{VerificationRequest, Verifier, VerifierError};
use sha2::{Digest as _, Sha256};

/// The wrapper instrument's name — the capability is the §14.2 file/digest
/// rung; this names the adapter that verifies.
pub const INSTRUMENT_NAME: &str = "gauntlet-verify-file-digest";

/// The evidence kinds this instrument conclusively decides (§14.2: the
/// ladder dispatches the cheapest rung that `supports` the requirement).
pub const DECIDES: &[EvidenceKind] = &[EvidenceKind::File];

/// The digest verifier, bound to one workspace ROOT at construction
/// (canonicalized then; locators resolve under it and never escape it).
pub struct FileDigestVerifier {
    root: PathBuf,
}

impl FileDigestVerifier {
    /// Binds the verifier to a workspace root; the root must exist when
    /// the instrument is wired (it is the same root the observation side
    /// wrote its artifacts under).
    pub fn new(root: PathBuf) -> Result<Self, VerifierError> {
        let canonical =
            std::fs::canonicalize(&root).map_err(|error| VerifierError::InstrumentFailed {
                detail: format!(
                    "the file-digest verifier's root {:?} does not resolve: {error}",
                    root.display()
                ),
            })?;
        Ok(Self { root: canonical })
    }

    /// Writes the well-formed PASSING artifact pair for the suite factory
    /// and tests: the content at `content_relative`, its digest manifest
    /// at `locator`.
    pub fn write_passing_artifact(
        root: &Path,
        locator: &str,
        content_relative: &str,
        bytes: &[u8],
    ) -> Result<(), VerifierError> {
        let content_path = contained_path(root, content_relative)?;
        if let Some(parent) = content_path.parent() {
            std::fs::create_dir_all(parent).map_err(write_failed)?;
        }
        std::fs::write(&content_path, bytes).map_err(write_failed)?;
        let manifest = format!(
            "sha256:{}\n{content_relative}\n",
            sha256_hex(bytes).expect("SHA-256 output is valid hex")
        );
        let manifest_path = contained_path(root, locator)?;
        if let Some(parent) = manifest_path.parent() {
            std::fs::create_dir_all(parent).map_err(write_failed)?;
        }
        std::fs::write(&manifest_path, manifest).map_err(write_failed)?;
        Ok(())
    }
}

impl Verifier for FileDigestVerifier {
    fn verify(
        &self,
        claim: &VerificationRequest,
        _deadline: gauntlet_ports::deadline::Deadline,
    ) -> Result<gauntlet_domain::VerificationRecord, VerifierError> {
        let manifest_path = contained_path(&self.root, claim.evidence.as_str())?;
        let manifest = std::fs::read_to_string(&manifest_path).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                VerifierError::EvidenceMissing {
                    locator: claim.evidence.clone(),
                }
            } else {
                read_failed(error)
            }
        })?;
        let mut lines = manifest.lines();
        let declared = lines.next().unwrap_or_default();
        let content_relative = lines.next().unwrap_or_default();
        if lines.next().is_some()
            || !declared.starts_with("sha256:")
            || declared.len() != "sha256:".len() + 64
            || content_relative.is_empty()
        {
            return Err(VerifierError::InstrumentFailed {
                detail: format!(
                    "the digest manifest at {} is not the two-line sha256 manifest the \
                     instrument's contract declares",
                    claim.evidence
                ),
            });
        }
        let content_path = contained_path(&self.root, content_relative)?;
        let content = std::fs::read(&content_path).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                VerifierError::EvidenceMissing {
                    locator: gauntlet_domain::EvidenceLocator::new(content_relative)
                        .expect("a contained relative path is a valid locator"),
                }
            } else {
                read_failed(error)
            }
        })?;
        let actual = sha256_hex(&content).expect("SHA-256 output is valid hex");
        let outcome = if format!("sha256:{actual}") == declared {
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

/// Resolves a locator under the root, refusing escapes (the observe
/// adapters' containment discipline): absolute locators and `..` segments
/// never resolve.
fn contained_path(root: &Path, locator: &str) -> Result<PathBuf, VerifierError> {
    if locator.starts_with('/') || locator.split('/').any(|segment| segment == "..") {
        return Err(VerifierError::InstrumentFailed {
            detail: format!("locator {locator:?} escapes the verifier's root"),
        });
    }
    Ok(root.join(locator))
}

fn sha256_hex(bytes: &[u8]) -> Option<String> {
    let hex: String = Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    (!hex.is_empty()).then_some(hex)
}

fn read_failed(error: std::io::Error) -> VerifierError {
    VerifierError::InstrumentFailed {
        detail: format!("reading the digest evidence failed: {error}"),
    }
}

fn write_failed(error: std::io::Error) -> VerifierError {
    VerifierError::InstrumentFailed {
        detail: format!("writing the digest fixture failed: {error}"),
    }
}

// ---------------------------------------------------------------------------
// The shared-suite factory (spec §7.3: every adapter runs the UNMODIFIED
// suite; the factory materializes the `present` locators with this
// instrument's well-formed passing artifacts).
// ---------------------------------------------------------------------------

/// Builds isolated verifiers over per-build scratch roots (the
/// `observe-test` factory pattern: single-threaded interior mutability,
/// roots removed on drop).
#[derive(Default)]
pub struct FileDigestVerifierFactory {
    roots: std::cell::RefCell<Vec<PathBuf>>,
    next: std::cell::Cell<u64>,
}

impl FileDigestVerifierFactory {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Drop for FileDigestVerifierFactory {
    fn drop(&mut self) {
        for root in self.roots.borrow().iter() {
            let _ = std::fs::remove_dir_all(root);
        }
    }
}

impl gauntlet_ports::contract::verifier::VerifierFactory for FileDigestVerifierFactory {
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
            "gauntlet-verify-file-digest-{}-{seq}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root)
            .map_err(|err| format!("creating the suite scratch root failed: {err}"))?;
        for locator in &present {
            // A SIBLING file, not a nested path: the manifest itself sits at
            // the locator, so the content it names must not live under it.
            let content = format!("{}.content", locator.as_str());
            FileDigestVerifier::write_passing_artifact(
                &root,
                locator.as_str(),
                &content,
                b"the suite's passing content",
            )
            .map_err(|err| err.to_string())?;
        }
        self.roots.borrow_mut().push(root.clone());
        Ok(Box::new(
            FileDigestVerifier::new(root).map_err(|err| err.to_string())?,
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
                "gauntlet-verify-file-digest-test-{tag}-{}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).expect("scratch root");
            Self(path)
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
            claim: "the digest matches".to_owned(),
            requirement: EvidenceRequirement::new(
                EvidenceKind::File,
                gauntlet_domain::EvidenceLocator::new(locator).expect("locator"),
                gauntlet_domain::Producer::Instrument,
            ),
            evidence: gauntlet_domain::EvidenceLocator::new(locator).expect("locator"),
        }
    }

    #[test]
    fn a_matching_digest_passes_and_a_mismatch_fails() {
        let scratch = Scratch::new("match");
        FileDigestVerifier::write_passing_artifact(
            &scratch.0,
            "evidence/manifest",
            "evidence/content.bin",
            b"the content",
        )
        .expect("fixture writes");
        let verifier = FileDigestVerifier::new(scratch.0.clone()).expect("verifier binds");
        let pass = verifier
            .verify(&request("evidence/manifest"), bound())
            .expect("the instrument decides");
        assert_eq!(pass.outcome(), Outcome::Pass);
        // Corrupt the content: present-but-wrong evidence is a FAIL
        // decision, not an error.
        std::fs::write(scratch.0.join("evidence/content.bin"), b"tampered").expect("corrupt");
        let fail = verifier
            .verify(&request("evidence/manifest"), bound())
            .expect("the instrument decides");
        assert_eq!(fail.outcome(), Outcome::Fail);
    }

    #[test]
    fn a_malformed_manifest_is_an_instrument_failure_not_a_fake_outcome() {
        let scratch = Scratch::new("malformed");
        std::fs::write(scratch.0.join("manifest"), "not a manifest").expect("writes");
        let verifier = FileDigestVerifier::new(scratch.0.clone()).expect("binds");
        assert!(matches!(
            verifier.verify(&request("manifest"), bound()),
            Err(VerifierError::InstrumentFailed { .. })
        ));
    }

    #[test]
    fn a_locator_that_escapes_the_root_never_resolves() {
        let scratch = Scratch::new("escape");
        let verifier = FileDigestVerifier::new(scratch.0.clone()).expect("binds");
        assert!(matches!(
            verifier.verify(&request("../outside"), bound()),
            Err(VerifierError::InstrumentFailed { detail, .. }) if detail.contains("escapes")
        ));
    }

    fn bound() -> gauntlet_ports::deadline::Deadline {
        gauntlet_ports::deadline::Deadline::at_epoch(gauntlet_domain::EpochSeconds::new(
            1_700_000_060,
        ))
    }
}
