//! `Verifier` — spec §6.2: "`verify(claim: VerificationRequest) ->
//! VerificationRecord` · `is_deterministic()` — Deterministic verifiers are
//! tried first (§14.2)."
//!
//! The request reuses the domain's [`EvidenceRequirement`] (§5.3.2: the
//! unit's declared `evidence_required` — kind, locator, producer); the
//! record is the domain's [`VerificationRecord`], which binds one claim to
//! one evidence ref and one outcome by a named instrument.
//!
//! ## Layered error/outcome reconciliation (plan Task 11; recorded decision)
//!
//! Two spec statements govern missing evidence at two DIFFERENT layers, and
//! they are reconciled by layering, not by choosing one:
//!
//! - **Instrument layer (this port).** An adapter that cannot find the
//!   evidence at its locator reports the typed
//!   [`VerifierError::EvidenceMissing`] — the instrument's honest "I could
//!   not decide". This is the behavior the frozen Task-4 shared
//!   `verifier_suite` case `missing_evidence_is_an_error_not_silence`
//!   pins, and it is unchanged.
//! - **Consequential layer (the application's earned verification chain,
//!   spec §14.3).** Binding failures are INCONCLUSIVE *outcomes with typed
//!   classes* ([`crate::verification_record::BindingClass`]), never
//!   `VerifierError`s and never PASS. The chain converts an instrument's
//!   `EvidenceMissing` into an `Outcome::Inconclusive` record carrying
//!   `evidence_unresolvable`, and decides producer/locator/recency from
//!   durable state, not from any request field (Task 10's root invariant:
//!   no binding check compares two values the same caller supplied).
//!
//! Nothing was weakened on either side: the instrument may not pretend to
//! have decided, and the loop may not treat absence as silence or success.

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

    /// This instrument's identity — name and version (spec §14.1: the
    /// record carries "the instrument identity"). The ladder records THIS
    /// value on every `verification_record@v1` it mints for the rung, so a
    /// record's instrument is what the instrument itself reports, never
    /// what a caller pairs with it. Pure — no deadline.
    fn instrument(&self) -> crate::observer::InstrumentIdentity;

    /// Whether this verifier can CONCLUSIVELY evaluate the requirement's
    /// evidence kind (spec §14.2: the ladder selects "the cheapest verifier
    /// that can conclusively decide it"). This is the rung's declaration of
    /// what it decides — an adapter claiming every kind while evaluating
    /// none is a configuration defect the ladder surfaces as an instrument
    /// error, never as a fabricated outcome.
    ///
    /// Default `true` preserves the Task-4 stubs' contract (a local
    /// verifier that decides whatever it is handed); real adapters declare
    /// their kinds. The model rung also answers `true` — it is the rung of
    /// last resort — but `is_deterministic` distinguishes it, and every
    /// record it mints is flagged (B.5 `instrument.deterministic: false`).
    fn supports(&self, _requirement: &EvidenceRequirement) -> bool {
        true
    }
}
