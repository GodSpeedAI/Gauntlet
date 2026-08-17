//! The `Comparator` shared contract suite (spec §6.2, invariant I4 and
//! §5.3.6 coupling).
//!
//! Pinned semantics: the record a comparator returns MUST carry the
//! envelope's own digests — the reference digest it compared against and
//! the observation digest — so the I1 cross-check stays possible at every
//! later boundary ("a record whose `reference_digest` differs from the
//! run's frozen digest invalidates the iteration", spec §5.3.6). An
//! envelope carrying a comparison-contract version the adapter does not
//! know is a typed [`ComparatorError::EnvelopeInvalid`] refusal, never a
//! guess.

use gauntlet_domain::{Digest, EpochSeconds, IterationNo, ObservationBundle, RunId};

use crate::comparator::{Comparator, ComparatorError, ComparisonEnvelope};
use crate::contract::ContractFinding;
use crate::deadline::Deadline;

/// The suite's name in findings.
pub const SUITE: &str = "comparator";

const BASE_INSTANT: u64 = 1_700_000_000;
/// The suite's bound span, on its own line (numeric literals never share a
/// line with a duration-shaped identifier).
const BOUND_SPAN: u64 = 60;

/// Constructs the comparator under test. `build` must return an ISOLATED
/// instance: cases assume a clean slate.
pub trait ComparatorFactory {
    fn label(&self) -> &'static str;

    /// The comparison-contract version this adapter claims to run under
    /// (Annex B.4 `comparison_contract_version`) — the value the record
    /// path runs with.
    fn supported_version(&self) -> String;

    fn build(&self) -> Result<Box<dyn Comparator>, String>;
}

fn finding(case: &'static str, rule: &'static str, detail: String) -> ContractFinding {
    ContractFinding::new(SUITE, case, rule, detail)
}

fn bound() -> Deadline {
    Deadline::at_epoch(EpochSeconds::new(BASE_INSTANT + BOUND_SPAN))
}

/// A canonical envelope: distinct digests per side, an observation bundle
/// without gaps, and the factory's declared contract version.
fn envelope(factory: &dyn ComparatorFactory) -> ComparisonEnvelope {
    ComparisonEnvelope {
        run_id: RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("suite run id is a ULID"),
        iteration_no: IterationNo::new(3).expect("suite iteration is 1-based"),
        reference_digest: Digest::new("sha256", "c4d3e2f1a0b1c2d3").expect("suite digest"),
        reference_view: "the runtime-rendered view of the frozen standard".to_owned(),
        observation: ObservationBundle::new(
            Digest::new("sha256", "e5f6a7b8a1b2c3d4").expect("suite observation digest"),
            Vec::new(),
            Vec::new(),
        ),
        contract_version: factory.supported_version(),
    }
}

/// Case: the returned record couples both of the envelope's digests (spec
/// §5.3.6: the record carries `reference_digest` and `observation_digest`;
/// I1's cross-check is impossible at later boundaries without them).
fn record_digests_couple_the_envelope(factory: &dyn ComparatorFactory) -> Vec<ContractFinding> {
    const CASE: &str = "record_digests_couple_the_envelope";
    const RULE: &str = "spec 5.3.6: the comparison record MUST carry the envelope's reference_digest and observation_digest";
    let Ok(comparator) = factory.build() else {
        return vec![finding(
            CASE,
            RULE,
            "the factory could not build a comparator for the case".to_owned(),
        )];
    };
    let envelope = envelope(factory);
    let observation_digest = envelope.observation.observation_digest().clone();
    let reference_digest = envelope.reference_digest.clone();
    let record = match comparator.compare(&envelope, bound()) {
        Ok(record) => record,
        Err(error) => {
            return vec![finding(
                CASE,
                RULE,
                format!("comparing the canonical envelope failed: {error}"),
            )];
        }
    };
    let mut findings = Vec::new();
    if record.reference_digest() != &reference_digest {
        findings.push(finding(
            CASE,
            RULE,
            format!(
                "the record carries reference_digest {} but the envelope's is {}",
                record.reference_digest(),
                reference_digest
            ),
        ));
    }
    if record.observation_digest() != &observation_digest {
        findings.push(finding(
            CASE,
            RULE,
            format!(
                "the record carries observation_digest {} but the envelope's is {}",
                record.observation_digest(),
                observation_digest
            ),
        ));
    }
    findings
}

/// Case: an envelope under a contract version the adapter does not know is
/// a typed [`ComparatorError::EnvelopeInvalid`], never a comparison run
/// under a guessed contract.
fn unknown_contract_versions_are_rejected(factory: &dyn ComparatorFactory) -> Vec<ContractFinding> {
    const CASE: &str = "unknown_contract_versions_are_rejected";
    const RULE: &str = "spec 6.2 + port contract: a contract version the adapter does not know is EnvelopeInvalid, never a guessed run";
    let Ok(comparator) = factory.build() else {
        return vec![finding(
            CASE,
            RULE,
            "the factory could not build a comparator for the case".to_owned(),
        )];
    };
    let mut unknown = envelope(factory);
    unknown.contract_version = "suite-version-not-known-9".to_owned();
    match comparator.compare(&unknown, bound()) {
        Err(ComparatorError::EnvelopeInvalid { .. }) => Vec::new(),
        Err(other) => vec![finding(
            CASE,
            RULE,
            format!(
                "an unknown contract version was refused with {other} instead of EnvelopeInvalid"
            ),
        )],
        Ok(_) => vec![finding(
            CASE,
            RULE,
            "an unknown contract version was compared instead of refused".to_owned(),
        )],
    }
}

/// Runs the full `Comparator` contract suite against one factory.
pub fn comparator_suite(factory: &dyn ComparatorFactory) -> Vec<ContractFinding> {
    let mut findings = Vec::new();
    findings.extend(record_digests_couple_the_envelope(factory));
    findings.extend(unknown_contract_versions_are_rejected(factory));
    findings
}
