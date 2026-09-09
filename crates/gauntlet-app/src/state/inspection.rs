//! Unit inspection and workbench projection services (spec §20.2; T01).
//!
//! Provides the deep inspection read contract for Units across the
//! Reference (answer key), durable verification records, EvidenceStore,
//! and escalation tickets, as well as the lightweight source-referencing
//! workbench projections ([`CaseProjection`], [`WorkItemProjection`],
//! [`AttentionItemProjection`]).
//!
//! Preserves canonical domain ownership: Case/WorkItem/AttentionItem
//! are non-authoritative read projections over RunId, UnitId, and TicketId.

use core::fmt;

use gauntlet_domain::{
    Authority, BlastRadius, EvidenceKind, EvidenceLocator, EvidenceRef, Outcome, RunId, Unit,
    UnitId, UnitStatus, VerifyBy,
};
use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::evidence_store::EvidenceStore;
use gauntlet_ports::human_escalation::TicketId;
use gauntlet_ports::state_store::StateStore;

/// Cap for bounded evidence text excerpts in the inspection view (16 KiB).
pub const BOUNDED_EVIDENCE_BYTE_LIMIT: usize = 16 * 1024;

/// What would prove a Unit's claim — projected for inspection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceRequirementFact {
    pub kind: EvidenceKind,
    pub locator: EvidenceLocator,
    pub producer: String,
}

/// One verification record's inspection fact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerificationInspectionFact {
    pub unit_id: UnitId,
    pub outcome: Outcome,
    pub evidence: EvidenceRef,
    pub verification: String,
}

/// One escalation fact for a unit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EscalationInspectionFact {
    pub ticket_id: TicketId,
    pub run_id: RunId,
    pub unit_id: Option<UnitId>,
    pub reason: String,
    pub is_open: bool,
}

/// Bounded evidence content returned from EvidenceStore.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedEvidenceContent {
    pub reference: EvidenceRef,
    pub byte_count: usize,
    pub text_preview: Option<String>,
    pub is_truncated: bool,
}

/// Complete Unit inspection contract.
#[derive(Debug, Clone, PartialEq)]
pub struct UnitInspection {
    pub unit_id: UnitId,
    pub tier: Option<BlastRadius>,
    pub claim: Option<String>,
    pub authority: Option<Authority>,
    pub blast_radius: Option<BlastRadius>,
    pub verify_by: Option<VerifyBy>,
    pub evidence_required: Option<EvidenceRequirementFact>,
    pub depends_on: Vec<UnitId>,
    pub status: UnitStatus,
    pub settled: bool,
    pub verifications: Vec<VerificationInspectionFact>,
    pub escalation: Option<EscalationInspectionFact>,
    pub evidence_content: Option<BoundedEvidenceContent>,
    pub precondition_explanation: Option<String>,
}

/// Case projection (source-referencing projection over RunId).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseProjection {
    pub case_id: String,
    pub run_id: RunId,
    pub title: String,
    pub total_units: usize,
    pub settled_units: usize,
    pub open_escalations: usize,
    pub failed_units: usize,
}

/// WorkItem projection (source-referencing projection over UnitId).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkItemProjection {
    pub work_item_id: String,
    pub run_id: RunId,
    pub unit_id: UnitId,
    pub tier: BlastRadius,
    pub status: UnitStatus,
    pub claim: String,
    pub blocked_by: Vec<UnitId>,
}

/// AttentionItem projection (source-referencing projection over TicketId or failing verification).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttentionItemProjection {
    pub attention_id: String,
    pub run_id: RunId,
    pub unit_id: Option<UnitId>,
    pub ticket_id: Option<TicketId>,
    pub kind: String,
    pub summary: String,
    pub is_open: bool,
}

/// Why unit inspection failed — typed (spec §21.2).
#[derive(Debug, Clone, PartialEq)]
pub enum UnitInspectionError {
    StateReadFailed { detail: String },
    UnitNotFound { unit_id: UnitId },
}

impl fmt::Display for UnitInspectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StateReadFailed { detail } => {
                write!(f, "state read failed during inspection: {detail}")
            }
            Self::UnitNotFound { unit_id } => {
                write!(f, "unit {unit_id} not found in reference or run state")
            }
        }
    }
}

impl std::error::Error for UnitInspectionError {}

/// Inspects one unit in a run by synthesizing Reference definition,
/// durable checkpoint states, verification records, escalation tickets,
/// and bounded EvidenceStore content.
pub fn inspect_unit(
    run_id: &RunId,
    unit_id: &UnitId,
    answer_key_units: &[Unit],
    state: &dyn StateStore,
    evidence_store: Option<&dyn EvidenceStore>,
    open_tickets: &[(TicketId, Option<UnitId>, String, bool)],
    deadline: Deadline,
) -> Result<UnitInspection, UnitInspectionError> {
    // 1. Locate Unit definition in answer key
    let ref_unit = answer_key_units.iter().find(|u| u.id() == unit_id);

    // 2. Fetch checkpoint to determine status
    let checkpoint =
        state
            .resume(run_id, deadline)
            .map_err(|err| UnitInspectionError::StateReadFailed {
                detail: err.to_string(),
            })?;

    // 3. Fetch verification records
    let records = state
        .verification_records(run_id, deadline)
        .map_err(|err| UnitInspectionError::StateReadFailed {
            detail: err.to_string(),
        })?;

    let verifications: Vec<VerificationInspectionFact> = records
        .iter()
        .filter(|rec| rec.unit_id() == unit_id)
        .map(|rec| VerificationInspectionFact {
            unit_id: rec.unit_id().clone(),
            outcome: rec.outcome(),
            evidence: rec.evidence().clone(),
            verification: rec.verification().to_string(),
        })
        .collect();

    // 4. Fetch settlements
    let settlements = state.settlements(run_id, deadline).map_err(|err| {
        UnitInspectionError::StateReadFailed {
            detail: err.to_string(),
        }
    })?;
    let settled = settlements.iter().any(|s| s.unit_id() == unit_id);

    // 5. Locate matching escalation ticket if any
    let escalation = open_tickets
        .iter()
        .find(|(_, u_id, reason, _)| {
            u_id.as_ref() == Some(unit_id) || reason.contains(unit_id.as_str())
        })
        .map(
            |(ticket_id, u_id, reason, is_open)| EscalationInspectionFact {
                ticket_id: ticket_id.clone(),
                run_id: run_id.clone(),
                unit_id: u_id.clone(),
                reason: reason.clone(),
                is_open: *is_open,
            },
        );

    let status = if settled {
        UnitStatus::Settled
    } else if escalation.as_ref().is_some_and(|e| e.is_open) {
        UnitStatus::Blocked
    } else {
        checkpoint
            .as_ref()
            .and_then(|cp| {
                cp.unit_states
                    .iter()
                    .find(|(id, _)| id == unit_id)
                    .map(|(_, s)| *s)
            })
            .unwrap_or(UnitStatus::Open)
    };

    // 6. Fetch bounded evidence content if evidence_store provided and verification exists
    let evidence_content =
        if let (Some(store), Some(latest_verif)) = (evidence_store, verifications.last()) {
            match store.get(&latest_verif.evidence, deadline) {
                Ok(Some(bytes)) => {
                    let byte_count = bytes.len();
                    let is_truncated = byte_count > BOUNDED_EVIDENCE_BYTE_LIMIT;
                    let slice = if is_truncated {
                        &bytes[..BOUNDED_EVIDENCE_BYTE_LIMIT]
                    } else {
                        &bytes[..]
                    };
                    let text_preview = String::from_utf8(slice.to_vec()).ok();
                    Some(BoundedEvidenceContent {
                        reference: latest_verif.evidence.clone(),
                        byte_count,
                        text_preview,
                        is_truncated,
                    })
                }
                _ => None,
            }
        } else {
            None
        };

    // 7. Compute precondition explanation
    let depends_on = ref_unit
        .map(|u| u.depends_on().to_vec())
        .unwrap_or_default();

    let mut unmet_deps = Vec::new();
    for dep in &depends_on {
        let dep_settled = settlements.iter().any(|s| s.unit_id() == dep);
        if !dep_settled {
            unmet_deps.push(dep.clone());
        }
    }

    let precondition_explanation = if settled {
        Some("Settled: verified PASS and recorded in settlement state".to_owned())
    } else if let Some(esc) = &escalation {
        Some(format!(
            "Escalated: ticket {} open — {}",
            esc.ticket_id, esc.reason
        ))
    } else if !unmet_deps.is_empty() {
        let dep_names: Vec<String> = unmet_deps.iter().map(|d| d.to_string()).collect();
        Some(format!(
            "Blocked: prerequisite units not yet settled: {}",
            dep_names.join(", ")
        ))
    } else if let Some(last_v) = verifications.last() {
        if last_v.outcome == Outcome::Fail {
            Some(format!(
                "Failed: last verification outcome was FAIL with evidence {}",
                last_v.evidence
            ))
        } else if last_v.outcome == Outcome::Inconclusive {
            Some(format!(
                "Inconclusive: last verification outcome was INCONCLUSIVE with evidence {}",
                last_v.evidence
            ))
        } else {
            Some("Ready for settlement: verification PASS recorded".to_owned())
        }
    } else {
        Some("Preconditions satisfied: ready for execution".to_owned())
    };

    if ref_unit.is_none() && checkpoint.is_none() && verifications.is_empty() && !settled {
        return Err(UnitInspectionError::UnitNotFound {
            unit_id: unit_id.clone(),
        });
    }

    Ok(UnitInspection {
        unit_id: unit_id.clone(),
        tier: ref_unit.map(|u| u.blast_radius()),
        claim: ref_unit.map(|u| u.claim().to_owned()),
        authority: ref_unit.map(|u| u.authority()),
        blast_radius: ref_unit.map(|u| u.blast_radius()),
        verify_by: ref_unit.map(|u| u.verify_by()),
        evidence_required: ref_unit.map(|u| {
            let req = u.evidence_required();
            EvidenceRequirementFact {
                kind: req.kind(),
                locator: req.locator().clone(),
                producer: format!("{:?}", req.producer()),
            }
        }),
        depends_on,
        status,
        settled,
        verifications,
        escalation,
        evidence_content,
        precondition_explanation,
    })
}

/// Generates CaseProjection from run state and tickets.
pub fn project_case(
    run_id: &RunId,
    units: &[Unit],
    settled_units_count: usize,
    open_escalations_count: usize,
    failed_units_count: usize,
) -> CaseProjection {
    CaseProjection {
        case_id: format!("case:{run_id}"),
        run_id: run_id.clone(),
        title: format!("Run {run_id}"),
        total_units: units.len(),
        settled_units: settled_units_count,
        open_escalations: open_escalations_count,
        failed_units: failed_units_count,
    }
}

/// Generates WorkItemProjection for a unit.
pub fn project_work_item(
    run_id: &RunId,
    unit: &Unit,
    status: UnitStatus,
    settled_ids: &[UnitId],
) -> WorkItemProjection {
    let blocked_by: Vec<UnitId> = unit
        .depends_on()
        .iter()
        .filter(|dep| !settled_ids.contains(dep))
        .cloned()
        .collect();

    WorkItemProjection {
        work_item_id: format!("work:{run_id}:{}", unit.id()),
        run_id: run_id.clone(),
        unit_id: unit.id().clone(),
        tier: unit.blast_radius(),
        status,
        claim: unit.claim().to_owned(),
        blocked_by,
    }
}

/// Generates AttentionItemProjection for an escalation ticket.
pub fn project_attention_item(
    run_id: &RunId,
    ticket_id: &TicketId,
    unit_id: Option<&UnitId>,
    reason: &str,
    is_open: bool,
) -> AttentionItemProjection {
    AttentionItemProjection {
        attention_id: format!("attention:{ticket_id}"),
        run_id: run_id.clone(),
        unit_id: unit_id.cloned(),
        ticket_id: Some(ticket_id.clone()),
        kind: "escalation".to_owned(),
        summary: reason.to_owned(),
        is_open,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gauntlet_domain::{
        EpochSeconds, EventLogOffset, EvidenceKind, EvidenceLocator, EvidenceRef, IterationNo,
        Outcome, Producer, Run, RunId, SettlementRecord, UnitId, UnitStatus, VerificationRef,
    };
    use gauntlet_ports::deadline::Deadline;
    use gauntlet_ports::human_escalation::TicketId;
    use gauntlet_ports::state_store::{Checkpoint, RunEvent, StateStore, StateStoreError};
    use gauntlet_ports::verification_record::{
        RequirementDoc, VerificationContent, VerificationRecordDoc, VerifyingInstrument,
    };

    // Fixed test instants, named (no numeral shares a unit with a duration
    // hint — the repo-wide literal scanner is load-bearing; see
    // baseline/harness.rs's NOW for the established pattern).
    const LATE_INSTANT: u64 = 2_000_000_000;
    const FIXED_INSTANT: u64 = 1_700_000_000;

    fn test_deadline() -> Deadline {
        Deadline::at_epoch(EpochSeconds::new(LATE_INSTANT))
    }

    fn test_run_id() -> RunId {
        RunId::new("01H7XRP0000000000000000000").expect("valid run id")
    }

    fn test_unit(id_str: &str) -> UnitId {
        UnitId::new(id_str).expect("valid unit id")
    }

    fn test_verification(unit_id: &UnitId, outcome: Outcome) -> VerificationRecordDoc {
        VerificationRecordDoc::new(VerificationContent {
            verification: VerificationRef::new("vrf:01H7XRP0000000000000000001")
                .expect("valid ref"),
            run_id: test_run_id(),
            iteration_no: IterationNo::new(1).expect("iteration 1"),
            unit_id: unit_id.clone(),
            claim: "test claim".to_owned(),
            requirement: RequirementDoc::new(
                EvidenceKind::TestReport,
                EvidenceLocator::new("locator-test").expect("valid locator"),
                Producer::Instrument,
            ),
            instrument: VerifyingInstrument::new(
                "test-verifier".to_owned(),
                "1.0".to_owned(),
                true,
            ),
            evidence: EvidenceRef::new(
                "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            )
            .expect("valid ref"),
            produced_at: EpochSeconds::new(FIXED_INSTANT),
            outcome,
            class: None,
            detail: "test detail".to_owned(),
        })
        .expect("valid verification doc")
    }

    struct MockStore {
        checkpoint: Option<Checkpoint>,
        verifications: Vec<VerificationRecordDoc>,
        settlements: Vec<SettlementRecord>,
    }

    impl StateStore for MockStore {
        fn append_event(
            &self,
            _: RunEvent,
            _: Deadline,
        ) -> Result<EventLogOffset, StateStoreError> {
            Ok(EventLogOffset::new(0))
        }
        fn load_run(&self, _: &RunId, _: Deadline) -> Result<Option<Run>, StateStoreError> {
            Ok(None)
        }
        fn checkpoint(&self, _: Checkpoint, _: Deadline) -> Result<(), StateStoreError> {
            Ok(())
        }
        fn resume(&self, _: &RunId, _: Deadline) -> Result<Option<Checkpoint>, StateStoreError> {
            Ok(self.checkpoint.clone())
        }
        fn export_jsonl(&self, _: &RunId, _: Deadline) -> Result<String, StateStoreError> {
            Ok(String::new())
        }
        fn verification_records(
            &self,
            _: &RunId,
            _: Deadline,
        ) -> Result<Vec<VerificationRecordDoc>, StateStoreError> {
            Ok(self.verifications.clone())
        }
        fn settlements(
            &self,
            _: &RunId,
            _: Deadline,
        ) -> Result<Vec<SettlementRecord>, StateStoreError> {
            Ok(self.settlements.clone())
        }
    }

    #[test]
    fn project_case_builds_expected_fields() {
        let run_id = test_run_id();
        let proj = project_case(&run_id, &[], 5, 2, 1);
        assert_eq!(proj.case_id, format!("case:{run_id}"));
        assert_eq!(proj.run_id, run_id);
        assert_eq!(proj.total_units, 0);
        assert_eq!(proj.settled_units, 5);
        assert_eq!(proj.open_escalations, 2);
        assert_eq!(proj.failed_units, 1);
    }

    #[test]
    fn project_attention_item_builds_expected_fields() {
        let run_id = test_run_id();
        let ticket_id = TicketId::new("TCK-001").expect("valid ticket id");
        let unit_id = test_unit("UT-001");
        let proj = project_attention_item(
            &run_id,
            &ticket_id,
            Some(&unit_id),
            "Needs human review",
            true,
        );

        assert_eq!(proj.attention_id, "attention:TCK-001");
        assert_eq!(proj.unit_id, Some(unit_id));
        assert_eq!(proj.ticket_id, Some(ticket_id));
        assert_eq!(proj.kind, "escalation");
        assert_eq!(proj.summary, "Needs human review");
        assert!(proj.is_open);
    }

    #[test]
    fn inspect_unit_not_found_when_completely_absent() {
        let run_id = test_run_id();
        let unit_id = test_unit("UT-999");
        let store = MockStore {
            checkpoint: None,
            verifications: Vec::new(),
            settlements: Vec::new(),
        };
        let deadline = test_deadline();
        let result = inspect_unit(&run_id, &unit_id, &[], &store, None, &[], deadline);
        assert!(matches!(
            result,
            Err(UnitInspectionError::UnitNotFound { .. })
        ));
    }

    #[test]
    fn inspect_unit_settled_precondition() {
        let run_id = test_run_id();
        let unit_id = test_unit("UT-100");
        let store = MockStore {
            checkpoint: None,
            verifications: Vec::new(),
            settlements: vec![SettlementRecord::new(
                run_id.clone(),
                unit_id.clone(),
                EvidenceRef::new(
                    "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
                )
                .unwrap(),
                VerificationRef::new("vrf:01H7XRP0000000000000000001").unwrap(),
                EventLogOffset::new(1),
            )],
        };
        let deadline = test_deadline();
        let inspection = inspect_unit(&run_id, &unit_id, &[], &store, None, &[], deadline)
            .expect("inspection succeeds");
        assert!(inspection.settled);
        assert_eq!(inspection.status, UnitStatus::Settled);
        assert!(
            inspection
                .precondition_explanation
                .unwrap()
                .contains("Settled: verified PASS")
        );
    }

    #[test]
    fn inspect_unit_escalated_precondition() {
        let run_id = test_run_id();
        let unit_id = test_unit("UT-200");
        let ticket_id = TicketId::new("TCK-002").expect("valid ticket");
        let store = MockStore {
            checkpoint: None,
            verifications: vec![test_verification(&unit_id, Outcome::Pass)],
            settlements: Vec::new(),
        };
        let tickets = vec![(
            ticket_id,
            Some(unit_id.clone()),
            "Escalated for operator decision".to_owned(),
            true,
        )];
        let deadline = test_deadline();
        let inspection = inspect_unit(&run_id, &unit_id, &[], &store, None, &tickets, deadline)
            .expect("inspection succeeds");
        assert_eq!(inspection.status, UnitStatus::Blocked);
        assert!(
            inspection
                .precondition_explanation
                .unwrap()
                .contains("Escalated: ticket TCK-002 open — Escalated for operator decision")
        );
    }

    #[test]
    fn inspect_unit_failed_precondition() {
        let run_id = test_run_id();
        let unit_id = test_unit("UT-300");
        let store = MockStore {
            checkpoint: None,
            verifications: vec![test_verification(&unit_id, Outcome::Fail)],
            settlements: Vec::new(),
        };
        let deadline = test_deadline();
        let inspection = inspect_unit(&run_id, &unit_id, &[], &store, None, &[], deadline)
            .expect("inspection succeeds");
        assert_eq!(inspection.status, UnitStatus::Open);
        assert!(
            inspection
                .precondition_explanation
                .unwrap()
                .contains("Failed: last verification outcome was FAIL")
        );
    }
}
