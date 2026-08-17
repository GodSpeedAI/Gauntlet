//! The `Verifier` shared contract suite (spec §6.2; §14.2, P12).
//!
//! Pinned semantics: a verification record is bound one-to-one to its
//! request — id, run, unit, and the evidence that was actually verified
//! (spec §5.3.2: "… for this unit id in this run"); missing evidence is an
//! error, never silence and never a fabricated pass (P12: "missing
//! evidence yields INCONCLUSIVE, not silence" — the missing-evidence case
//! is the typed [`VerifierError::EvidenceMissing`], the instrument's way
//! of saying it could not decide); and a deterministic verifier does not
//! flap — the same request verifies to the same record.

use gauntlet_domain::{
    EpochSeconds, EvidenceKind, EvidenceLocator, EvidenceRequirement, Outcome, Producer, RunId,
    UnitId, VerificationRef,
};

use crate::contract::ContractFinding;
use crate::deadline::Deadline;
use crate::verifier::{VerificationRequest, Verifier, VerifierError};

/// The suite's name in findings.
pub const SUITE: &str = "verifier";

const BASE_INSTANT: u64 = 1_700_000_000;
/// The suite's bound span, on its own line (numeric literals never share a
/// line with a duration-shaped identifier).
const BOUND_SPAN: u64 = 60;

/// Constructs the verifier under test. `present` names the evidence
/// locators the adapter can actually evaluate; anything else is missing
/// (P12).
pub trait VerifierFactory {
    fn label(&self) -> &'static str;

    fn build(&self, present: Vec<EvidenceLocator>) -> Result<Box<dyn Verifier>, String>;

    /// Whether this verifier is deterministic (spec §6.2: deterministic
    /// verifiers are tried first, §14.2). Adapters that are not declare
    /// false; the flap case does not apply to them.
    fn deterministic(&self) -> bool;
}

fn finding(case: &'static str, rule: &'static str, detail: String) -> ContractFinding {
    ContractFinding {
        suite: SUITE,
        case,
        rule,
        detail,
    }
}

fn bound() -> Deadline {
    Deadline::at_epoch(EpochSeconds::new(BASE_INSTANT + BOUND_SPAN))
}

fn request(locator: EvidenceLocator) -> VerificationRequest {
    VerificationRequest {
        id: VerificationRef::new("vrf-000042").expect("suite verification ref"),
        run_id: RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("suite run id is a ULID"),
        unit_id: UnitId::new("AK-014").expect("suite unit id"),
        claim: "the slice is complete per the frozen standard".to_owned(),
        requirement: EvidenceRequirement::new(
            EvidenceKind::Log,
            locator.clone(),
            Producer::Instrument,
        ),
        evidence: locator,
    }
}

/// Case: the record is bound to its request — id, run, unit, and the exact
/// evidence ref verified (§5.3.2's one-to-one binding; settlement checks
/// these later).
fn verified_claims_are_bound_to_their_request(
    factory: &dyn VerifierFactory,
) -> Vec<ContractFinding> {
    const CASE: &str = "verified_claims_are_bound_to_their_request";
    const RULE: &str =
        "spec 5.3.2/6.2: the verification record binds the request's id, run, unit, and evidence";
    let locator = EvidenceLocator::new("locator-suite-01").expect("suite locator");
    let Ok(verifier) = factory.build(vec![locator.clone()]) else {
        return vec![finding(
            CASE,
            RULE,
            "the factory could not build a verifier for the case".to_owned(),
        )];
    };
    let request = request(locator.clone());
    let record = match verifier.verify(&request, bound()) {
        Ok(record) => record,
        Err(error) => {
            return vec![finding(
                CASE,
                RULE,
                format!("verifying a claim over present evidence failed: {error}"),
            )];
        }
    };
    let mut findings = Vec::new();
    if record.id() != &request.id {
        findings.push(finding(
            CASE,
            RULE,
            format!(
                "the record carries id {} instead of {}",
                record.id(),
                request.id
            ),
        ));
    }
    if record.run_id() != &request.run_id {
        findings.push(finding(
            CASE,
            RULE,
            format!(
                "the record names run {} instead of {}",
                record.run_id(),
                request.run_id
            ),
        ));
    }
    if record.unit_id() != &request.unit_id {
        findings.push(finding(
            CASE,
            RULE,
            format!(
                "the record names unit {} instead of {}",
                record.unit_id(),
                request.unit_id
            ),
        ));
    }
    if record.evidence().as_str() != request.evidence.as_str() {
        findings.push(finding(
            CASE,
            RULE,
            format!(
                "the record names evidence {} instead of {}",
                record.evidence(),
                request.evidence
            ),
        ));
    }
    if !matches!(
        record.outcome(),
        Outcome::Pass | Outcome::Fail | Outcome::Inconclusive
    ) {
        findings.push(finding(
            CASE,
            RULE,
            "the record carries no three-state outcome".to_owned(),
        ));
    }
    findings
}

/// Case: evidence the adapter cannot find is the typed `EvidenceMissing`
/// error — never a pass, never silence (P12; spec §6.2's error space:
/// instruments that could not decide error, they do not pretend).
fn missing_evidence_is_an_error_not_silence(factory: &dyn VerifierFactory) -> Vec<ContractFinding> {
    const CASE: &str = "missing_evidence_is_an_error_not_silence";
    const RULE: &str = "P12: missing evidence yields INCONCLUSIVE, not silence — the adapter reports EvidenceMissing instead of a result";
    let Ok(verifier) = factory.build(vec![
        EvidenceLocator::new("locator-suite-01").expect("suite locator"),
    ]) else {
        return vec![finding(
            CASE,
            RULE,
            "the factory could not build a verifier for the case".to_owned(),
        )];
    };
    let absent = EvidenceLocator::new("locator-suite-absent-99").expect("suite locator");
    let request = request(absent.clone());
    match verifier.verify(&request, bound()) {
        Err(VerifierError::EvidenceMissing { locator }) if locator == absent => Vec::new(),
        Err(other) => vec![finding(
            CASE,
            RULE,
            format!("missing evidence was refused with {other} instead of EvidenceMissing"),
        )],
        Ok(record) => vec![finding(
            CASE,
            RULE,
            format!(
                "verifying against absent evidence produced a record with outcome {} — \
                 missing evidence is not passing evidence",
                record.outcome()
            ),
        )],
    }
}

/// Case: when the factory declares the verifier deterministic, the same
/// request verifies to the same record — no flapping (spec §6.2: the
/// scheduling value of deterministic verifiers presumes stability).
fn deterministic_verifiers_do_not_flap(factory: &dyn VerifierFactory) -> Vec<ContractFinding> {
    const CASE: &str = "deterministic_verifiers_do_not_flap";
    const RULE: &str =
        "spec 6.2/14.2: a deterministic verifier returns the same record for the same request";
    if !factory.deterministic() {
        return Vec::new();
    }
    let locator = EvidenceLocator::new("locator-suite-01").expect("suite locator");
    let Ok(verifier) = factory.build(vec![locator.clone()]) else {
        return vec![finding(
            CASE,
            RULE,
            "the factory could not build a verifier for the case".to_owned(),
        )];
    };
    let request = request(locator);
    let one = verifier.verify(&request, bound());
    let two = verifier.verify(&request, bound());
    match (one, two) {
        (Ok(first), Ok(second)) if first == second => Vec::new(),
        (Ok(first), Ok(second)) => vec![finding(
            CASE,
            RULE,
            format!(
                "the same request produced the records {} and {} — a deterministic verifier \
                 does not flap",
                first.outcome(),
                second.outcome()
            ),
        )],
        (Err(error), _) | (_, Err(error)) => vec![finding(
            CASE,
            RULE,
            format!("a deterministic verification errored: {error}"),
        )],
    }
}

/// Runs the full `Verifier` contract suite against one factory.
pub fn verifier_suite(factory: &dyn VerifierFactory) -> Vec<ContractFinding> {
    let mut findings = Vec::new();
    findings.extend(verified_claims_are_bound_to_their_request(factory));
    findings.extend(missing_evidence_is_an_error_not_silence(factory));
    findings.extend(deterministic_verifiers_do_not_flap(factory));
    findings
}
