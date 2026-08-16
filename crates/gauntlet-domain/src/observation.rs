//! `ObservationBundle` — spec §5.3.6.
//!
//! Spec §5.3.6 domain invariant: "`ObservationBundle` MUST record
//! `unavailable_evidence` and `observation_failures` explicitly. A bundle
//! that silently omits a failed probe is invalid (I8)."
//!
//! Both arrays are REQUIRED constructor parameters — populated `Vec` types,
//! never `Option`. An empty vec is a positive statement that nothing failed;
//! a missing field cannot swallow a failure because there is no missing
//! field. Facets themselves (provenance, commands, exit statuses) are the
//! observer adapters' concern (plan Task 9) and the Annex B schema.

use crate::values::{Digest, EvidenceLocator, InstrumentName};

/// Evidence a probe wanted and could not get (spec §5.3.6
/// `unavailable_evidence`). Absent evidence is recorded, never dropped —
/// "Missing evidence is not passing evidence" (I8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnavailableEvidence {
    locator: EvidenceLocator,
    reason: String,
}

impl UnavailableEvidence {
    pub fn new(locator: EvidenceLocator, reason: String) -> Self {
        Self { locator, reason }
    }

    pub fn locator(&self) -> &EvidenceLocator {
        &self.locator
    }

    pub fn reason(&self) -> &str {
        &self.reason
    }
}

/// A probe that ran and failed (spec §5.3.6 `observation_failures`). A bundle
/// that silently omits one is invalid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservationFailure {
    instrument: InstrumentName,
    probe: String,
    failure: String,
}

impl ObservationFailure {
    pub fn new(instrument: InstrumentName, probe: String, failure: String) -> Self {
        Self {
            instrument,
            probe,
            failure,
        }
    }

    pub fn instrument(&self) -> &InstrumentName {
        &self.instrument
    }

    pub fn probe(&self) -> &str {
        &self.probe
    }

    pub fn failure(&self) -> &str {
        &self.failure
    }
}

/// What a run's observation step actually saw (spec §5.1: the Observation
/// context communicates via "bundle digest + evidence refs").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservationBundle {
    observation_digest: Digest,
    unavailable_evidence: Vec<UnavailableEvidence>,
    observation_failures: Vec<ObservationFailure>,
}

impl ObservationBundle {
    /// Assembles a bundle. The two gap arrays are mandatory arguments: the
    /// caller states explicitly that a list is empty rather than the field
    /// existing optionally.
    pub fn new(
        observation_digest: Digest,
        unavailable_evidence: Vec<UnavailableEvidence>,
        observation_failures: Vec<ObservationFailure>,
    ) -> Self {
        Self {
            observation_digest,
            unavailable_evidence,
            observation_failures,
        }
    }

    /// Digest over the normalized bundle (computed by the observer side,
    /// spec §5.3.6 comparison coupling).
    pub fn observation_digest(&self) -> &Digest {
        &self.observation_digest
    }

    pub fn unavailable_evidence(&self) -> &[UnavailableEvidence] {
        &self.unavailable_evidence
    }

    pub fn observation_failures(&self) -> &[ObservationFailure] {
        &self.observation_failures
    }

    /// Whether anything was missing or failed. Not a pass/fail of the work —
    /// a statement about the observation itself (I8).
    pub fn has_gaps(&self) -> bool {
        !self.unavailable_evidence.is_empty() || !self.observation_failures.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest() -> Digest {
        Digest::new("sha256", "e5f6a7b8").expect("valid digest parts")
    }

    #[test]
    fn a_bundle_carries_its_gaps_explicitly() {
        // Spec §5.3.6: "`ObservationBundle` MUST record
        // `unavailable_evidence` and `observation_failures` explicitly. A
        // bundle that silently omits a failed probe is invalid (I8)."
        // Both arrays are populated, non-optional types.
        let unavailable = UnavailableEvidence::new(
            EvidenceLocator::new("locator-0002").expect("valid locator"),
            "the probe found nothing at this locator".to_owned(),
        );
        let failure = ObservationFailure::new(
            InstrumentName::new("observer-02").expect("valid name"),
            "facet-digest".to_owned(),
            "the instrument exited without a report".to_owned(),
        );
        let bundle =
            ObservationBundle::new(digest(), vec![unavailable.clone()], vec![failure.clone()]);
        assert_eq!(bundle.observation_digest(), &digest());
        assert_eq!(bundle.unavailable_evidence(), &[unavailable]);
        assert_eq!(bundle.observation_failures(), &[failure]);
        assert!(bundle.has_gaps());
    }

    #[test]
    fn an_empty_gap_list_is_a_positive_statement() {
        // The caller states the lists are empty; there is no missing field
        // to swallow a failure behind.
        let bundle = ObservationBundle::new(digest(), Vec::new(), Vec::new());
        assert_eq!(bundle.unavailable_evidence(), &[]);
        assert_eq!(bundle.observation_failures(), &[]);
        assert!(!bundle.has_gaps());
    }
}
