//! Test-report verifier adapter (spec §4.1, §14; plan Task 11): the §14.2
//! ladder's FOURTH rung — "focused test". A claim whose evidence
//! requirement is a `test_report` at a locator is decided by the report's
//! own totals.
//!
//! The artifact contract: the report at `evidence_required.locator` is a
//! JSON object carrying `tests` (how many ran) and `failures` (how many
//! failed) —
//!
//! ```json
//! { "tests": 3, "failures": 0, "command": "cargo nextest run ..." }
//! ```
//!
//! PASS iff `tests > 0` and `failures == 0`. FAIL iff `tests > 0` and
//! `failures > 0` (tests RAN and some failed — present-and-wrong evidence
//! is a decision). A report with ZERO tests is
//! [`VerifierError::InstrumentFailed`] — an empty report is no evidence at
//! all, and missing evidence is not passing evidence (I8): the instrument
//! declines to decide over it rather than blessing the emptiness. Absent
//! evidence is the typed [`VerifierError::EvidenceMissing`] (P12 at the
//! instrument layer).

use std::path::{Path, PathBuf};

use gauntlet_domain::{EvidenceKind, EvidenceRequirement, InstrumentName, Outcome};
use gauntlet_ports::json::{JsonNumber, JsonValue};
use gauntlet_ports::observer::InstrumentIdentity;
use gauntlet_ports::verifier::{VerificationRequest, Verifier, VerifierError};

/// The wrapper instrument's name — the capability is the §14.2 focused-test
/// rung.
pub const INSTRUMENT_NAME: &str = "gauntlet-verify-test-report";

/// The evidence kinds this instrument conclusively decides.
pub const DECIDES: &[EvidenceKind] = &[EvidenceKind::TestReport];

/// The test-report verifier, bound to one workspace ROOT at construction.
pub struct TestReportVerifier {
    root: PathBuf,
}

impl TestReportVerifier {
    /// Binds the verifier to a workspace root (canonicalized then; locators
    /// resolve under it and never escape it).
    pub fn new(root: PathBuf) -> Result<Self, VerifierError> {
        let canonical =
            std::fs::canonicalize(&root).map_err(|error| VerifierError::InstrumentFailed {
                detail: format!(
                    "the test-report verifier's root {:?} does not resolve: {error}",
                    root.display()
                ),
            })?;
        Ok(Self { root: canonical })
    }

    /// Writes a well-formed report artifact: `tests` ran, `failures` failed,
    /// under the recorded `command`.
    pub fn write_report(
        root: &Path,
        locator: &str,
        tests: u64,
        failures: u64,
        command: &str,
    ) -> Result<(), VerifierError> {
        let path = contained_path(root, locator)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| VerifierError::InstrumentFailed {
                detail: format!("materializing the report parent failed: {error}"),
            })?;
        }
        let report =
            format!("{{\"tests\":{tests},\"failures\":{failures},\"command\":\"{command}\"}}");
        std::fs::write(&path, report).map_err(|error| VerifierError::InstrumentFailed {
            detail: format!("writing the test report failed: {error}"),
        })
    }
}

impl Verifier for TestReportVerifier {
    fn verify(
        &self,
        claim: &VerificationRequest,
        _deadline: gauntlet_ports::deadline::Deadline,
    ) -> Result<gauntlet_domain::VerificationRecord, VerifierError> {
        let path = contained_path(&self.root, claim.evidence.as_str())?;
        let raw = std::fs::read(&path).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                VerifierError::EvidenceMissing {
                    locator: claim.evidence.clone(),
                }
            } else {
                VerifierError::InstrumentFailed {
                    detail: format!("reading the test report failed: {error}"),
                }
            }
        })?;
        let text = std::str::from_utf8(&raw).map_err(|_| VerifierError::InstrumentFailed {
            detail: "the test report is not UTF-8".to_owned(),
        })?;
        let report =
            gauntlet_ports::json::parse(text).map_err(|error| VerifierError::InstrumentFailed {
                detail: format!("the test report does not parse as JSON: {error}"),
            })?;
        let count = |key: &str| -> Result<u64, VerifierError> {
            match report.get(key) {
                Some(JsonValue::Number(JsonNumber::Uint(value))) => Ok(*value),
                Some(_) => Err(VerifierError::InstrumentFailed {
                    detail: format!("the test report's {key} is not a whole number"),
                }),
                None => Err(VerifierError::InstrumentFailed {
                    detail: format!(
                        "the test report carries no {key} total — the instrument's contract \
                         requires both"
                    ),
                }),
            }
        };
        let tests = count("tests")?;
        let failures = count("failures")?;
        if tests == 0 {
            return Err(VerifierError::InstrumentFailed {
                detail: format!(
                    "the test report at {} records ZERO tests run — an empty report is no \
                     evidence, not a pass (I8)",
                    claim.evidence
                ),
            });
        }
        let outcome = if failures == 0 {
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
pub struct TestReportVerifierFactory {
    roots: std::cell::RefCell<Vec<PathBuf>>,
    next: std::cell::Cell<u64>,
}

impl TestReportVerifierFactory {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Drop for TestReportVerifierFactory {
    fn drop(&mut self) {
        for root in self.roots.borrow().iter() {
            let _ = std::fs::remove_dir_all(root);
        }
    }
}

impl gauntlet_ports::contract::verifier::VerifierFactory for TestReportVerifierFactory {
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
            "gauntlet-verify-test-report-{}-{seq}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root)
            .map_err(|err| format!("creating the suite scratch root failed: {err}"))?;
        for locator in &present {
            TestReportVerifier::write_report(
                &root,
                locator.as_str(),
                3,
                0,
                "cargo nextest run --test the-suite",
            )
            .map_err(|err| err.to_string())?;
        }
        self.roots.borrow_mut().push(root.clone());
        Ok(Box::new(
            TestReportVerifier::new(root).map_err(|err| err.to_string())?,
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
                "gauntlet-verify-test-report-test-{tag}-{}",
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
            claim: "the tests pass".to_owned(),
            requirement: EvidenceRequirement::new(
                EvidenceKind::TestReport,
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
    fn a_clean_report_passes_and_a_failing_report_fails() {
        let scratch = Scratch::new("totals");
        TestReportVerifier::write_report(scratch.root(), "clean.json", 12, 0, "nextest")
            .expect("w");
        TestReportVerifier::write_report(scratch.root(), "dirty.json", 12, 2, "nextest")
            .expect("w");
        let verifier = TestReportVerifier::new(scratch.root().to_path_buf()).expect("binds");
        assert_eq!(
            verifier
                .verify(&request("clean.json"), bound())
                .expect("decides")
                .outcome(),
            Outcome::Pass
        );
        assert_eq!(
            verifier
                .verify(&request("dirty.json"), bound())
                .expect("decides")
                .outcome(),
            Outcome::Fail
        );
    }

    #[test]
    fn an_empty_report_is_not_a_pass_and_not_even_a_decision() {
        let scratch = Scratch::new("empty");
        TestReportVerifier::write_report(scratch.root(), "empty.json", 0, 0, "nextest").expect("w");
        let verifier = TestReportVerifier::new(scratch.root().to_path_buf()).expect("binds");
        // Zero tests: the instrument refuses to bless emptiness (I8).
        assert!(matches!(
            verifier.verify(&request("empty.json"), bound()),
            Err(VerifierError::InstrumentFailed { detail, .. }) if detail.contains("ZERO tests")
        ));
    }

    #[test]
    fn a_report_missing_a_total_is_an_instrument_failure() {
        let scratch = Scratch::new("nototals");
        std::fs::write(scratch.root().join("half.json"), b"{\"tests\":3}").expect("writes");
        let verifier = TestReportVerifier::new(scratch.root().to_path_buf()).expect("binds");
        assert!(matches!(
            verifier.verify(&request("half.json"), bound()),
            Err(VerifierError::InstrumentFailed { .. })
        ));
    }
}
