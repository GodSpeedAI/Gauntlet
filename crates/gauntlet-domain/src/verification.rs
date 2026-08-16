//! `VerificationRecord` — spec §5.3.6.
//!
//! Spec §5.3.6 domain invariant: "`VerificationRecord` binds one claim to one
//! evidence ref and one `Outcome` produced by a named instrument."
//!
//! The binding is structural: every field is a required, validated value
//! object — there is no record shape that carries two claims, two evidence
//! refs, or an anonymous instrument. The record also carries the run it was
//! produced in (spec §5.3.2: settlement requires a PASS record "for this
//! unit id **in this run**"; §16.1: "A `VerificationRecord` with
//! `Outcome = PASS` exists for it in this run"), which is what lets
//! `Unit::settle` refuse a stale record replayed from a different run. The
//! deep Annex B schema lands with the verifier task (plan Task 11); this
//! module fixes the domain shape.

use core::fmt;

use crate::values::{EvidenceRef, InstrumentName, Outcome, RunId, UnitId, VerificationRef};

/// One verification decision: run + claim (by Unit id) + evidence + outcome +
/// instrument. Constructing it is total — the value objects already carry
/// their own validity — so there is no error type here. The run binding is a
/// required field: a record that cannot say which run produced it cannot be
/// checked against "in this run" at settlement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerificationRecord {
    id: VerificationRef,
    run_id: RunId,
    unit_id: UnitId,
    evidence: EvidenceRef,
    outcome: Outcome,
    instrument: InstrumentName,
}

impl VerificationRecord {
    /// Binds the run, claim, evidence, outcome, and instrument. The caller (a
    /// verifier, spec §14) decides the outcome; the domain only guarantees
    /// the binding is one-to-one-to-one and named, and that the producing
    /// run is part of the record's identity.
    pub fn new(
        id: VerificationRef,
        run_id: RunId,
        unit_id: UnitId,
        evidence: EvidenceRef,
        outcome: Outcome,
        instrument: InstrumentName,
    ) -> Self {
        Self {
            id,
            run_id,
            unit_id,
            evidence,
            outcome,
            instrument,
        }
    }

    pub fn id(&self) -> &VerificationRef {
        &self.id
    }

    /// The run this record was produced in (spec §5.3.2: "… for this unit id
    /// in this run"). Settlement checks this against the run settling the
    /// Unit; a PASS record from a foreign run is stale evidence (§16.1).
    pub fn run_id(&self) -> &RunId {
        &self.run_id
    }

    /// The claim's Unit. Settlement checks this against the Unit being
    /// settled (spec §5.3.2).
    pub fn unit_id(&self) -> &UnitId {
        &self.unit_id
    }

    pub fn evidence(&self) -> &EvidenceRef {
        &self.evidence
    }

    pub fn outcome(&self) -> Outcome {
        self.outcome
    }

    /// The named instrument that produced this outcome (spec §5.3.6:
    /// "produced by a named instrument").
    pub fn instrument(&self) -> &InstrumentName {
        &self.instrument
    }
}

impl fmt::Display for VerificationRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "verification {} for {} in run {} by {}: {}",
            self.id, self.unit_id, self.run_id, self.instrument, self.outcome
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_id() -> RunId {
        RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("valid run id")
    }

    fn record(outcome: Outcome) -> VerificationRecord {
        VerificationRecord::new(
            VerificationRef::new("vrf-000042").expect("valid ref"),
            run_id(),
            UnitId::new("AK-014").expect("valid id"),
            EvidenceRef::new("evd-000042").expect("valid ref"),
            outcome,
            InstrumentName::new("verifier-01").expect("valid name"),
        )
    }

    #[test]
    fn a_record_binds_one_claim_to_one_evidence_and_one_outcome() {
        // Spec §5.3.6: "`VerificationRecord` binds one claim to one evidence
        // ref and one `Outcome` produced by a named instrument."
        let record = record(Outcome::Pass);
        assert_eq!(record.run_id(), &run_id());
        assert_eq!(record.unit_id().as_str(), "AK-014");
        assert_eq!(record.evidence().as_str(), "evd-000042");
        assert_eq!(record.outcome(), Outcome::Pass);
        assert_eq!(record.instrument().as_str(), "verifier-01");
    }

    #[test]
    fn the_three_outcomes_are_all_representable_and_distinct() {
        assert_eq!(record(Outcome::Pass).outcome(), Outcome::Pass);
        assert_eq!(record(Outcome::Fail).outcome(), Outcome::Fail);
        assert_eq!(
            record(Outcome::Inconclusive).outcome(),
            Outcome::Inconclusive
        );
        assert_ne!(record(Outcome::Pass), record(Outcome::Fail));
        assert_ne!(record(Outcome::Pass), record(Outcome::Inconclusive));
    }
}
