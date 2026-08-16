//! `CapabilityLedger` and `CandidateRevision` — spec §5.3.8.
//!
//! Developmental memory, in two shapes:
//!
//! - A [`Capability`] accumulates the settlement evidence that a behavior is
//!   real, plus the variation it was demonstrated under and the recovery
//!   events it survived. Spec §5.3.8: `status: observed | provisional |
//!   promoted | revoked`.
//! - A [`CandidateRevision`] is a proposed change to the loop itself. Spec
//!   §5.3.8: `decision: pending | promoted | rejected | provisional`. The
//!   promotion gate here is structural — a revision cannot be promoted
//!   without evaluation runs and all three held-out comparison results —
//!   because "current → candidate → isolated evaluation → held-out/regression
//!   comparison → promote or rollback" is the whole point of the type. The
//!   numeric promotion thresholds for capabilities (AK-055) are policy and
//!   arrive as arguments from configuration, never as literals here.

use core::fmt;

use crate::values::{CapabilityId, EvidenceRef, RevisionId, RunId};
use crate::wire::wire_enum;

wire_enum! {
    /// Spec §5.3.8 `capability.status`: `observed | provisional | promoted |
    /// revoked`.
    pub enum CapabilityStatus {
        Observed, Provisional, Promoted, Revoked,
    }
}

wire_enum! {
    /// Spec §5.3.8 `candidate_revision.decision`: `pending | promoted |
    /// rejected | provisional`.
    pub enum RevisionDecision {
        Pending, Promoted, Rejected, Provisional,
    }
}

/// `CandidateRevision` invariant failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CandidateRevisionError {
    /// A decision was already recorded; the decision field is write-once.
    DecisionAlreadyMade { current: RevisionDecision },
    /// Promotion without isolated evaluation runs (spec §5.3.8
    /// `evaluation_run_ids`).
    PromotionRequiresEvaluation { evaluation_run_count: usize },
    /// Promotion without the held-out comparison result.
    PromotionRequiresHeldOutResult,
    /// Promotion without the regression comparison result.
    PromotionRequiresRegressionResult,
    /// Promotion without the protected-capabilities comparison result.
    PromotionRequiresProtectedCapabilitiesResult,
    /// Provisional status without any evaluation evidence at all.
    ProvisionalRequiresEvaluation { evaluation_run_count: usize },
    /// Rejection without a rollback reference — a rejected revision must be
    /// rollable back.
    RejectionRequiresRollbackRef,
}

impl fmt::Display for CandidateRevisionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DecisionAlreadyMade { current } => write!(
                f,
                "candidate revision decision is already {current}; decisions are write-once \
                 (spec 5.3.8)"
            ),
            Self::PromotionRequiresEvaluation {
                evaluation_run_count,
            } => write!(
                f,
                "cannot promote a revision with {evaluation_run_count} evaluation runs: promotion \
                 requires isolated evaluation first (spec 5.3.8)"
            ),
            Self::PromotionRequiresHeldOutResult => write!(
                f,
                "cannot promote without the held-out comparison result (spec 5.3.8)"
            ),
            Self::PromotionRequiresRegressionResult => write!(
                f,
                "cannot promote without the regression comparison result (spec 5.3.8)"
            ),
            Self::PromotionRequiresProtectedCapabilitiesResult => write!(
                f,
                "cannot promote without the protected-capabilities comparison result (spec 5.3.8)"
            ),
            Self::ProvisionalRequiresEvaluation {
                evaluation_run_count,
            } => write!(
                f,
                "cannot mark a revision provisional with {evaluation_run_count} evaluation runs: \
                 even provisional status rests on evaluation evidence (spec 5.3.8)"
            ),
            Self::RejectionRequiresRollbackRef => write!(
                f,
                "cannot reject without a rollback reference: a rejected revision must be rollable \
                 back (spec 5.3.8)"
            ),
        }
    }
}

impl std::error::Error for CandidateRevisionError {}

/// `Capability` invariant failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapabilityError {
    /// A status change the capability's state does not permit.
    StatusChangeInvalid {
        from: CapabilityStatus,
        to: CapabilityStatus,
    },
    /// A settlement was recorded for a run already on the settlement list
    /// (AK-055 / spec §19.2: the configured minimum counts settlements —
    /// distinct runs — under recorded variation; one run recorded N times is
    /// one settlement, not N).
    DuplicateSettlement { run: RunId },
    /// Few settlements than the configured minimum (AK-055: one successful
    /// run does not promote a capability).
    PromotionRequiresSettlements { have: usize, need: usize },
    /// Less recorded variation than the configured minimum (AK-055:
    /// settlements under recorded variation).
    PromotionRequiresVariation { axes: usize, need: usize },
    /// No recovery event on record (AK-055: plus a recovery event).
    PromotionRequiresRecoveryEvent,
}

impl fmt::Display for CapabilityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StatusChangeInvalid { from, to } => {
                write!(
                    f,
                    "capability status cannot move {from} to {to} (spec 5.3.8)"
                )
            }
            Self::DuplicateSettlement { run } => write!(
                f,
                "run {run} is already among this capability's settlements: each run counts once, \
                 so replaying a settlement cannot inflate the promotion count (AK-055)"
            ),
            Self::PromotionRequiresSettlements { have, need } => write!(
                f,
                "cannot promote with {have} settlements where {need} are required: one successful \
                 run does not promote a capability (AK-055)"
            ),
            Self::PromotionRequiresVariation { axes, need } => write!(
                f,
                "cannot promote with {axes} recorded variation axes where {need} are required: \
                 promotion requires settlements under recorded variation (AK-055)"
            ),
            Self::PromotionRequiresRecoveryEvent => write!(
                f,
                "cannot promote without a recorded recovery event (AK-055)"
            ),
        }
    }
}

impl std::error::Error for CapabilityError {}

/// One developmental-memory entry (spec §5.3.8 `capability`). Settlements,
/// variation axes, and recovery events accumulate from real runs;
/// `orchestration_burden` is a trend ("trend, not a single number"), so it is
/// a recorded series.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Capability {
    id: CapabilityId,
    description: String,
    status: CapabilityStatus,
    settlements: Vec<RunId>,
    variation_axes: Vec<String>,
    recovery_events: Vec<String>,
    orchestration_burden: Vec<u64>,
}

impl Capability {
    /// A capability enters the ledger as `observed` — an observation, not yet
    /// a commitment.
    pub fn new(id: CapabilityId, description: String) -> Self {
        Self {
            id,
            description,
            status: CapabilityStatus::Observed,
            settlements: Vec::new(),
            variation_axes: Vec::new(),
            recovery_events: Vec::new(),
            orchestration_burden: Vec::new(),
        }
    }

    pub fn id(&self) -> &CapabilityId {
        &self.id
    }

    pub fn description(&self) -> &str {
        &self.description
    }

    pub fn status(&self) -> CapabilityStatus {
        self.status
    }

    /// Run refs demonstrating settlement (spec §5.3.8 `settlements`). Each
    /// run appears at most once: the list is a set of distinct runs, which
    /// is what the promotion count counts (AK-055).
    pub fn settlements(&self) -> &[RunId] {
        &self.settlements
    }

    /// Records a settlement under a run (spec §5.3.8 `settlements`).
    /// Recording the same run twice is a typed error: settlements are keyed
    /// by run, so a replayed recording cannot inflate the promotion count.
    /// (Replay idempotency at the durable layer is §16.3's event-log rule,
    /// plan Task 13 — here, at the ledger, a duplicate is surfaced, not
    /// silently absorbed.)
    pub fn record_settlement(&mut self, run: RunId) -> Result<(), CapabilityError> {
        if self.settlements.iter().any(|recorded| recorded == &run) {
            return Err(CapabilityError::DuplicateSettlement { run });
        }
        self.settlements.push(run);
        Ok(())
    }

    /// What differed across those settlements (spec §5.3.8 `variation_axes`).
    pub fn variation_axes(&self) -> &[String] {
        &self.variation_axes
    }

    pub fn record_variation_axis(&mut self, axis: String) {
        self.variation_axes.push(axis);
    }

    pub fn recovery_events(&self) -> &[String] {
        &self.recovery_events
    }

    pub fn record_recovery_event(&mut self, event: String) {
        self.recovery_events.push(event);
    }

    /// The orchestration-burden trend (spec §5.3.8: "trend, not a single
    /// number").
    pub fn orchestration_burden(&self) -> &[u64] {
        &self.orchestration_burden
    }

    pub fn record_orchestration_burden(&mut self, burden: u64) {
        self.orchestration_burden.push(burden);
    }

    /// Marks the capability provisional. From `observed` only.
    pub fn mark_provisional(&mut self) -> Result<(), CapabilityError> {
        self.restatus(CapabilityStatus::Provisional)
    }

    /// Promotes the capability. The thresholds are arguments because they are
    /// configuration (§8.6), not domain constants; the structural floor is
    /// enforced regardless of the arguments: at least one settlement, at
    /// least one variation axis, and at least one recovery event (AK-055).
    /// The settlement count is a count of DISTINCT runs —
    /// [`Capability::record_settlement`] refuses duplicates, so no replayed
    /// recording can satisfy a minimum.
    pub fn promote(
        &mut self,
        min_settlements: usize,
        min_variation_axes: usize,
    ) -> Result<(), CapabilityError> {
        let need = min_settlements.max(1);
        if self.settlements.len() < need {
            return Err(CapabilityError::PromotionRequiresSettlements {
                have: self.settlements.len(),
                need,
            });
        }
        let need_axes = min_variation_axes.max(1);
        if self.variation_axes.len() < need_axes {
            return Err(CapabilityError::PromotionRequiresVariation {
                axes: self.variation_axes.len(),
                need: need_axes,
            });
        }
        if self.recovery_events.is_empty() {
            return Err(CapabilityError::PromotionRequiresRecoveryEvent);
        }
        self.restatus(CapabilityStatus::Promoted)
    }

    /// Revokes the capability. From `provisional` or `promoted`.
    pub fn revoke(&mut self) -> Result<(), CapabilityError> {
        self.restatus(CapabilityStatus::Revoked)
    }

    fn restatus(&mut self, to: CapabilityStatus) -> Result<(), CapabilityError> {
        let from = self.status;
        let allowed = matches!(
            (from, to),
            (CapabilityStatus::Observed, CapabilityStatus::Provisional)
                | (CapabilityStatus::Observed, CapabilityStatus::Promoted)
                | (CapabilityStatus::Provisional, CapabilityStatus::Promoted)
                | (CapabilityStatus::Provisional, CapabilityStatus::Revoked)
                | (CapabilityStatus::Promoted, CapabilityStatus::Revoked)
        );
        if !allowed {
            return Err(CapabilityError::StatusChangeInvalid { from, to });
        }
        self.status = to;
        Ok(())
    }
}

/// A proposed change to the loop itself (spec §5.3.8 `candidate_revision`).
/// Enters the world `pending`; the decision field is write-once, and the
/// promoting transitions carry their evidence requirements with them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateRevision {
    id: RevisionId,
    target: String,
    claimed_problem: String,
    supporting_evidence: Vec<EvidenceRef>,
    predicted_improvement: String,
    predicted_regression_risk: Vec<String>,
    diff_ref: EvidenceRef,
    evaluation_run_ids: Vec<RunId>,
    held_out_result: Option<String>,
    regression_result: Option<String>,
    protected_capabilities_result: Option<String>,
    decision: RevisionDecision,
    rollback_ref: Option<EvidenceRef>,
}

impl CandidateRevision {
    /// A pending revision. The claimed problem and the diff are required up
    /// front: a revision without a stated problem and a concrete diff is not
    /// evaluable.
    pub fn new(
        id: RevisionId,
        target: String,
        claimed_problem: String,
        supporting_evidence: Vec<EvidenceRef>,
        predicted_improvement: String,
        predicted_regression_risk: Vec<String>,
        diff_ref: EvidenceRef,
    ) -> Self {
        Self {
            id,
            target,
            claimed_problem,
            supporting_evidence,
            predicted_improvement,
            predicted_regression_risk,
            diff_ref,
            evaluation_run_ids: Vec::new(),
            held_out_result: None,
            regression_result: None,
            protected_capabilities_result: None,
            decision: RevisionDecision::Pending,
            rollback_ref: None,
        }
    }

    pub fn id(&self) -> &RevisionId {
        &self.id
    }

    /// What the revision targets: prompt, role, skill, routing, topology,
    /// verifier, … (spec §5.3.8 `target` — an open vocabulary).
    pub fn target(&self) -> &str {
        &self.target
    }

    pub fn claimed_problem(&self) -> &str {
        &self.claimed_problem
    }

    pub fn supporting_evidence(&self) -> &[EvidenceRef] {
        &self.supporting_evidence
    }

    pub fn predicted_improvement(&self) -> &str {
        &self.predicted_improvement
    }

    pub fn predicted_regression_risk(&self) -> &[String] {
        &self.predicted_regression_risk
    }

    pub fn diff_ref(&self) -> &EvidenceRef {
        &self.diff_ref
    }

    pub fn evaluation_run_ids(&self) -> &[RunId] {
        &self.evaluation_run_ids
    }

    /// Records an isolated evaluation run of this candidate (spec §5.3.8
    /// `evaluation_run_ids`).
    pub fn record_evaluation_run(&mut self, run: RunId) {
        self.evaluation_run_ids.push(run);
    }

    pub fn held_out_result(&self) -> Option<&str> {
        self.held_out_result.as_deref()
    }

    pub fn record_held_out_result(&mut self, result: String) {
        self.held_out_result = Some(result);
    }

    pub fn regression_result(&self) -> Option<&str> {
        self.regression_result.as_deref()
    }

    pub fn record_regression_result(&mut self, result: String) {
        self.regression_result = Some(result);
    }

    pub fn protected_capabilities_result(&self) -> Option<&str> {
        self.protected_capabilities_result.as_deref()
    }

    pub fn record_protected_capabilities_result(&mut self, result: String) {
        self.protected_capabilities_result = Some(result);
    }

    pub fn decision(&self) -> RevisionDecision {
        self.decision
    }

    pub fn rollback_ref(&self) -> Option<&EvidenceRef> {
        self.rollback_ref.as_ref()
    }

    /// Marks the revision provisional. Requires at least one evaluation run:
    /// even a provisional decision rests on evaluation evidence.
    pub fn mark_provisional(&mut self) -> Result<(), CandidateRevisionError> {
        self.assert_not_decided()?;
        if self.evaluation_run_ids.is_empty() {
            return Err(CandidateRevisionError::ProvisionalRequiresEvaluation {
                evaluation_run_count: self.evaluation_run_ids.len(),
            });
        }
        self.decision = RevisionDecision::Provisional;
        Ok(())
    }

    /// Promotes the revision. Requires evaluation runs and all three
    /// comparison results; the optimizer's own claim is never among them.
    pub fn promote(&mut self) -> Result<(), CandidateRevisionError> {
        self.assert_not_decided()?;
        if self.evaluation_run_ids.is_empty() {
            return Err(CandidateRevisionError::PromotionRequiresEvaluation {
                evaluation_run_count: self.evaluation_run_ids.len(),
            });
        }
        if self.held_out_result.is_none() {
            return Err(CandidateRevisionError::PromotionRequiresHeldOutResult);
        }
        if self.regression_result.is_none() {
            return Err(CandidateRevisionError::PromotionRequiresRegressionResult);
        }
        if self.protected_capabilities_result.is_none() {
            return Err(CandidateRevisionError::PromotionRequiresProtectedCapabilitiesResult);
        }
        self.decision = RevisionDecision::Promoted;
        Ok(())
    }

    /// Rejects the revision, recording the rollback reference that makes the
    /// rejection safe.
    pub fn reject(&mut self, rollback_ref: EvidenceRef) -> Result<(), CandidateRevisionError> {
        self.assert_not_decided()?;
        self.rollback_ref = Some(rollback_ref);
        self.decision = RevisionDecision::Rejected;
        Ok(())
    }

    fn assert_not_decided(&self) -> Result<(), CandidateRevisionError> {
        if self.decision != RevisionDecision::Pending
            && self.decision != RevisionDecision::Provisional
        {
            return Err(CandidateRevisionError::DecisionAlreadyMade {
                current: self.decision,
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn revision() -> CandidateRevision {
        CandidateRevision::new(
            RevisionId::new("rev-0001").expect("valid id"),
            "verifier".to_owned(),
            "settlement accepts stale evidence".to_owned(),
            vec![EvidenceRef::new("evd-000042").expect("valid ref")],
            "stale evidence is rejected".to_owned(),
            vec!["runs may slow down".to_owned()],
            EvidenceRef::new("evd-diff-01").expect("valid ref"),
        )
    }

    fn evaluation_run() -> RunId {
        RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("valid run id")
    }

    #[test]
    fn a_revision_enters_the_world_pending() {
        assert_eq!(revision().decision(), RevisionDecision::Pending);
    }

    #[test]
    fn promotion_requires_evaluation_runs_and_all_three_results() {
        // Spec §5.3.8 + AGENTS.md self-improvement: candidate → isolated
        // evaluation → held-out/regression comparison → promote or rollback.
        let mut revision = revision();
        assert!(matches!(
            revision.promote(),
            Err(CandidateRevisionError::PromotionRequiresEvaluation {
                evaluation_run_count: 0
            })
        ));
        revision.record_evaluation_run(evaluation_run());
        assert!(matches!(
            revision.promote(),
            Err(CandidateRevisionError::PromotionRequiresHeldOutResult)
        ));
        revision.record_held_out_result("held-out wins".to_owned());
        assert!(matches!(
            revision.promote(),
            Err(CandidateRevisionError::PromotionRequiresRegressionResult)
        ));
        revision.record_regression_result("no regressions".to_owned());
        assert!(matches!(
            revision.promote(),
            Err(CandidateRevisionError::PromotionRequiresProtectedCapabilitiesResult)
        ));
        revision.record_protected_capabilities_result("protected set intact".to_owned());
        revision.promote().expect("full evidence promotes");
        assert_eq!(revision.decision(), RevisionDecision::Promoted);
        // The decision is write-once.
        assert!(matches!(
            revision.promote(),
            Err(CandidateRevisionError::DecisionAlreadyMade { .. })
        ));
    }

    #[test]
    fn provisional_rests_on_evidence_and_rejection_on_a_rollback() {
        let mut revision = revision();
        assert!(matches!(
            revision.mark_provisional(),
            Err(CandidateRevisionError::ProvisionalRequiresEvaluation {
                evaluation_run_count: 0
            })
        ));
        revision.record_evaluation_run(evaluation_run());
        revision
            .mark_provisional()
            .expect("evaluated, provisionally");
        // Provisional revisions can still promote or reject.
        revision.record_held_out_result("held-out loses".to_owned());
        revision.record_regression_result("regressions found".to_owned());
        revision.record_protected_capabilities_result("protected set intact".to_owned());
        let rollback = EvidenceRef::new("evd-rollback").expect("valid ref");
        revision
            .reject(rollback.clone())
            .expect("rejection records");
        assert_eq!(revision.decision(), RevisionDecision::Rejected);
        assert_eq!(revision.rollback_ref(), Some(&rollback));
    }

    #[test]
    fn one_successful_run_does_not_promote_a_capability() {
        // AK-055: promotion requires the configured minimum settlements
        // under recorded variation plus a recovery event. The thresholds are
        // arguments — policy belongs to configuration, not this crate.
        let id = CapabilityId::new("cap-0001").expect("valid id");
        let mut capability = Capability::new(id, "recovers from tool failure".to_owned());
        assert_eq!(capability.status(), CapabilityStatus::Observed);
        capability
            .record_settlement(evaluation_run())
            .expect("first settlement");
        assert!(matches!(
            capability.promote(3, 2),
            Err(CapabilityError::PromotionRequiresSettlements { have: 1, need: 3 })
        ));
        capability
            .record_settlement(RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAZ").expect("valid"))
            .expect("second settlement");
        capability
            .record_settlement(RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FB0").expect("valid"))
            .expect("third settlement");
        assert!(matches!(
            capability.promote(3, 2),
            Err(CapabilityError::PromotionRequiresVariation { axes: 0, need: 2 })
        ));
        capability.record_variation_axis("runner".to_owned());
        capability.record_variation_axis("reference".to_owned());
        assert!(matches!(
            capability.promote(3, 2),
            Err(CapabilityError::PromotionRequiresRecoveryEvent)
        ));
        capability.record_recovery_event("recovered from a timed-out critic".to_owned());
        capability
            .promote(3, 2)
            .expect("evidence-complete promotion");
        assert_eq!(capability.status(), CapabilityStatus::Promoted);
        // Promotion is the end of the line; revocation follows.
        assert!(matches!(
            capability.mark_provisional(),
            Err(CapabilityError::StatusChangeInvalid { .. })
        ));
        capability
            .revoke()
            .expect("promoted capabilities can be revoked");
        assert_eq!(capability.status(), CapabilityStatus::Revoked);
    }

    #[test]
    fn the_structural_floor_survives_lenient_arguments() {
        // Even a caller passing zero minimums cannot promote without any
        // settlement, variation, or recovery evidence.
        let mut capability = Capability::new(
            CapabilityId::new("cap-0002").expect("valid id"),
            "x".to_owned(),
        );
        assert!(matches!(
            capability.promote(0, 0),
            Err(CapabilityError::PromotionRequiresSettlements { need: 1, .. })
        ));
        capability
            .record_settlement(evaluation_run())
            .expect("one settlement");
        assert!(matches!(
            capability.promote(0, 0),
            Err(CapabilityError::PromotionRequiresVariation { need: 1, .. })
        ));
        capability.record_variation_axis("runner".to_owned());
        assert!(matches!(
            capability.promote(0, 0),
            Err(CapabilityError::PromotionRequiresRecoveryEvent)
        ));
        capability.record_orchestration_burden(4);
        assert_eq!(capability.orchestration_burden(), &[4]);
    }

    #[test]
    fn replaying_one_run_three_times_is_not_three_settlements() {
        // AK-055 / spec §19.2: "Promote a capability only with ≥ …
        // settlements across recorded variation axes" — settlements are run
        // refs, and one run recorded N times is one settlement. The replay
        // path the critic traced: a recovery-retry loop re-records the same
        // settlement on each replay; it must not inflate the count.
        let mut capability = Capability::new(
            CapabilityId::new("cap-0003").expect("valid id"),
            "survives replays".to_owned(),
        );
        let run = evaluation_run();
        capability
            .record_settlement(run.clone())
            .expect("recorded once");
        for _ in 0..2 {
            assert!(
                matches!(
                    capability.record_settlement(run.clone()),
                    Err(CapabilityError::DuplicateSettlement { .. })
                ),
                "re-recording the same run is refused"
            );
        }
        assert_eq!(capability.settlements().len(), 1);
        // Three records of one run still leave have = 1 against need = 3.
        capability.record_variation_axis("runner".to_owned());
        capability.record_recovery_event("recovered from a replay".to_owned());
        assert!(matches!(
            capability.promote(3, 1),
            Err(CapabilityError::PromotionRequiresSettlements { have: 1, need: 3 })
        ));
        // Three distinct runs satisfy it.
        capability
            .record_settlement(RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAZ").expect("valid"))
            .expect("second run");
        capability
            .record_settlement(RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FB0").expect("valid"))
            .expect("third run");
        capability
            .promote(3, 1)
            .expect("three distinct runs promote");
        assert_eq!(capability.status(), CapabilityStatus::Promoted);
    }
}
