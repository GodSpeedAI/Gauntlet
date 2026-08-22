//! Command-exit-code verifier adapter (spec §4.1, §14; plan Task 11): the
//! §14.2 ladder's THIRD rung — "compiler/type check" (a build/command
//! exit). A claim whose evidence requirement is a command transcript (kind
//! `log`) at a locator is decided by the recorded exit status.
//!
//! The wrapper IS the instrument, and it decides over the RECORD, not a
//! re-run: the transcript at `evidence_required.locator` is a two-line
//! artifact —
//!
//! ```text
//! <the exact command that ran>
//! <its exit status, an integer>
//! ```
//!
//! PASS iff the recorded exit status is zero. FAIL iff it is nonzero (the
//! command ran and failed — present-and-wrong evidence is a decision, not
//! an error, the P3 discipline). Absent evidence is the typed
//! [`VerifierError::EvidenceMissing`] (P12 at the instrument layer); a
//! malformed transcript is [`VerifierError::InstrumentFailed`].

use std::path::{Path, PathBuf};

use gauntlet_domain::{EvidenceKind, EvidenceRequirement, InstrumentName, Outcome};
use gauntlet_ports::observer::InstrumentIdentity;
use gauntlet_ports::verifier::{VerificationRequest, Verifier, VerifierError};

/// The wrapper instrument's name — the capability is the §14.2
/// command-exit rung.
pub const INSTRUMENT_NAME: &str = "gauntlet-verify-command-exit-code";

/// The evidence kinds this instrument conclusively decides.
pub const DECIDES: &[EvidenceKind] = &[EvidenceKind::Log];

/// The command-exit verifier, bound to one workspace ROOT at construction.
pub struct CommandExitVerifier {
    root: PathBuf,
}

impl CommandExitVerifier {
    /// Binds the verifier to a workspace root (canonicalized then; locators
    /// resolve under it and never escape it).
    pub fn new(root: PathBuf) -> Result<Self, VerifierError> {
        let canonical =
            std::fs::canonicalize(&root).map_err(|error| VerifierError::InstrumentFailed {
                detail: format!(
                    "the command-exit verifier's root {:?} does not resolve: {error}",
                    root.display()
                ),
            })?;
        Ok(Self { root: canonical })
    }

    /// Writes the well-formed PASSING artifact: a transcript of a command
    /// that exited zero.
    pub fn write_passing_artifact(
        root: &Path,
        locator: &str,
        command: &str,
    ) -> Result<(), VerifierError> {
        write_transcript(root, locator, command, 0)
    }
}

/// Writes a two-line transcript under the root at the locator.
pub fn write_transcript(
    root: &Path,
    locator: &str,
    command: &str,
    exit: i64,
) -> Result<(), VerifierError> {
    let path = contained_path(root, locator)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| VerifierError::InstrumentFailed {
            detail: format!("materializing the transcript parent failed: {error}"),
        })?;
    }
    std::fs::write(&path, format!("{command}\n{exit}\n")).map_err(|error| {
        VerifierError::InstrumentFailed {
            detail: format!("writing the transcript failed: {error}"),
        }
    })
}

impl Verifier for CommandExitVerifier {
    fn verify(
        &self,
        claim: &VerificationRequest,
        _deadline: gauntlet_ports::deadline::Deadline,
    ) -> Result<gauntlet_domain::VerificationRecord, VerifierError> {
        let path = contained_path(&self.root, claim.evidence.as_str())?;
        let transcript = std::fs::read_to_string(&path).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                VerifierError::EvidenceMissing {
                    locator: claim.evidence.clone(),
                }
            } else {
                VerifierError::InstrumentFailed {
                    detail: format!("reading the command transcript failed: {error}"),
                }
            }
        })?;
        let mut lines = transcript.lines();
        let _command = lines.next();
        let status = lines.next();
        if lines.next().is_some() || status.is_none() {
            return Err(VerifierError::InstrumentFailed {
                detail: format!(
                    "the transcript at {} is not the two-line command/exit artifact the \
                     instrument's contract declares",
                    claim.evidence
                ),
            });
        }
        // The recorded exit status is an integer by contract; anything else
        // is a transcript the instrument cannot decide over.
        let exit: i64 =
            status
                .unwrap_or_default()
                .parse()
                .map_err(|_| VerifierError::InstrumentFailed {
                    detail: format!(
                        "the transcript at {} records a non-integer exit status",
                        claim.evidence
                    ),
                })?;
        let outcome = if exit == 0 {
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
pub struct CommandExitVerifierFactory {
    roots: std::cell::RefCell<Vec<PathBuf>>,
    next: std::cell::Cell<u64>,
}

impl CommandExitVerifierFactory {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Drop for CommandExitVerifierFactory {
    fn drop(&mut self) {
        for root in self.roots.borrow().iter() {
            let _ = std::fs::remove_dir_all(root);
        }
    }
}

impl gauntlet_ports::contract::verifier::VerifierFactory for CommandExitVerifierFactory {
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
            "gauntlet-verify-command-{}-{seq}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root)
            .map_err(|err| format!("creating the suite scratch root failed: {err}"))?;
        for locator in &present {
            write_transcript(&root, locator.as_str(), "cargo nextest run", 0)
                .map_err(|err| err.to_string())?;
        }
        self.roots.borrow_mut().push(root.clone());
        Ok(Box::new(
            CommandExitVerifier::new(root).map_err(|err| err.to_string())?,
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
                "gauntlet-verify-command-test-{tag}-{}",
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
            claim: "the command exited zero".to_owned(),
            requirement: EvidenceRequirement::new(
                EvidenceKind::Log,
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
    fn a_zero_exit_passes_and_a_nonzero_exit_fails() {
        let scratch = Scratch::new("exit");
        write_transcript(scratch.root(), "ok.log", "cargo build", 0).expect("writes");
        write_transcript(scratch.root(), "broken.log", "cargo build", 1).expect("writes");
        let verifier = CommandExitVerifier::new(scratch.root().to_path_buf()).expect("binds");
        assert_eq!(
            verifier
                .verify(&request("ok.log"), bound())
                .expect("decides")
                .outcome(),
            Outcome::Pass
        );
        // A command that RAN and failed is a recorded FAIL, not an error —
        // the P3 discipline at the verification layer.
        assert_eq!(
            verifier
                .verify(&request("broken.log"), bound())
                .expect("decides")
                .outcome(),
            Outcome::Fail
        );
    }

    #[test]
    fn a_transcript_without_an_exit_status_is_an_instrument_failure() {
        let scratch = Scratch::new("malformed");
        std::fs::write(scratch.root().join("only-command.log"), "cargo build\n").expect("writes");
        let verifier = CommandExitVerifier::new(scratch.root().to_path_buf()).expect("binds");
        assert!(matches!(
            verifier.verify(&request("only-command.log"), bound()),
            Err(VerifierError::InstrumentFailed { .. })
        ));
    }
}
