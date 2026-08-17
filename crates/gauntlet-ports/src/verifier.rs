//! `Verifier` — spec §6.2: "`verify(claim: VerificationRequest) ->
//! VerificationRecord` · `is_deterministic()` — Deterministic verifiers are
//! tried first (§14.2)."
//!
//! The request reuses the domain's [`EvidenceRequirement`] (§5.3.2: the
//! unit's declared `evidence_required` — kind, locator, producer); the
//! record is the domain's [`VerificationRecord`], which binds one claim to
//! one evidence ref and one outcome by a named instrument.

use core::fmt;

use gauntlet_domain::{
    EvidenceLocator, EvidenceRequirement, RunId, UnitId, VerificationRecord, VerificationRef,
};

use crate::deadline::Deadline;

/// `Verifier` failures — typed (spec §21.2). Note the distinct state space:
/// an instrument that RAN and produced `Outcome::Fail` or
/// `Outcome::Inconclusive` is a successful verification with a negative or
/// uncertain result, NOT an error; errors are for instruments that could
/// not decide (P12: missing evidence yields INCONCLUSIVE, not silence).
#[derive(Debug, Clone, PartialEq)]
pub enum VerifierError {
    /// The declared evidence does not exist at its locator.
    EvidenceMissing { locator: EvidenceLocator },
    /// The instrument ran and could not produce a decision.
    InstrumentFailed { detail: String },
    /// The requirement is not one this verifier can evaluate.
    RequirementUnsupported { detail: String },
}

impl fmt::Display for VerifierError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EvidenceMissing { locator } => {
                write!(f, "the required evidence is missing at {locator}")
            }
            Self::InstrumentFailed { detail } => {
                write!(f, "the verifying instrument failed: {detail}")
            }
            Self::RequirementUnsupported { detail } => {
                write!(f, "the verification requirement is unsupported: {detail}")
            }
        }
    }
}

impl std::error::Error for VerifierError {}

/// One claim to verify (spec §14): the unit's claim, its declared evidence
/// requirement, and where the evidence lives.
#[derive(Debug, Clone)]
pub struct VerificationRequest {
    pub id: VerificationRef,
    pub run_id: RunId,
    pub unit_id: UnitId,
    pub claim: String,
    pub requirement: EvidenceRequirement,
    pub evidence: EvidenceLocator,
}

/// The verification port (spec §6.2).
pub trait Verifier {
    /// Verifies one claim against its declared evidence requirement.
    /// Blocking (instruments run), so a deadline is required (§6 preamble).
    fn verify(
        &self,
        claim: &VerificationRequest,
        deadline: Deadline,
    ) -> Result<VerificationRecord, VerifierError>;

    /// Whether this verifier is deterministic (spec §6.2: deterministic
    /// verifiers are tried first, §14.2). Pure — no deadline.
    fn is_deterministic(&self) -> bool;
}
