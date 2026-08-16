//! `SettlementRecord` — spec §5.3.6.
//!
//! Spec §5.3.6 domain invariant: "`SettlementRecord` is append-only and
//! references the event-log offset that made it durable."
//!
//! Append-only-shaped: every field is fixed at construction, there are no
//! mutation methods, and the event-log offset is a required argument — a
//! settlement that cannot name the durable event that made it real does not
//! exist. The atomic commit sequence around it is plan Task 13.

use crate::values::{EvidenceRef, RunId, UnitId, VerificationRef};

/// A position in the append-only event log. Monotonicity is the log's
/// invariant; the type just prevents an offset from being a bare integer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EventLogOffset {
    offset: u64,
}

impl EventLogOffset {
    pub fn new(offset: u64) -> Self {
        Self { offset }
    }

    pub fn get(self) -> u64 {
        self.offset
    }
}

/// The durable fact that a Unit settled (spec §5.3.1 settlement, §16
/// preconditions). Constructed only after those preconditions held; carries
/// the evidence and verification that satisfied them plus the event-log
/// offset that made the settlement durable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettlementRecord {
    run_id: RunId,
    unit_id: UnitId,
    evidence: EvidenceRef,
    verification: VerificationRef,
    event_log_offset: EventLogOffset,
}

impl SettlementRecord {
    pub fn new(
        run_id: RunId,
        unit_id: UnitId,
        evidence: EvidenceRef,
        verification: VerificationRef,
        event_log_offset: EventLogOffset,
    ) -> Self {
        Self {
            run_id,
            unit_id,
            evidence,
            verification,
            event_log_offset,
        }
    }

    pub fn run_id(&self) -> &RunId {
        &self.run_id
    }

    pub fn unit_id(&self) -> &UnitId {
        &self.unit_id
    }

    pub fn evidence(&self) -> &EvidenceRef {
        &self.evidence
    }

    pub fn verification(&self) -> &VerificationRef {
        &self.verification
    }

    /// The event-log offset that made this settlement durable (spec §5.3.6).
    pub fn event_log_offset(&self) -> EventLogOffset {
        self.event_log_offset
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_settlement_record_names_the_event_that_made_it_durable() {
        // Spec §5.3.6: "`SettlementRecord` is append-only and references the
        // event-log offset that made it durable." The offset is a required
        // constructor argument — a settlement that cannot name its durable
        // event does not exist.
        let record = SettlementRecord::new(
            RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("valid run id"),
            UnitId::new("AK-014").expect("valid id"),
            EvidenceRef::new("evd-000042").expect("valid ref"),
            VerificationRef::new("vrf-000042").expect("valid ref"),
            EventLogOffset::new(4321),
        );
        assert_eq!(record.event_log_offset().get(), 4321);
        assert_eq!(record.unit_id().as_str(), "AK-014");
        assert_eq!(record.evidence().as_str(), "evd-000042");
        assert_eq!(record.verification().as_str(), "vrf-000042");
        // Offsets order: the log's monotonicity is what makes settlement
        // replay idempotent at the durable layer.
        assert!(EventLogOffset::new(4322) > record.event_log_offset());
    }
}
