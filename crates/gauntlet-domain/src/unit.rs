//! `Unit` — the answer-key entry (spec §5.3.2).
//!
//! Spec §5.3.2 aggregate invariants, quoted where they decide the shape:
//!
//! - "`evidence_required.producer` MUST be `instrument` unless `verify_by =
//!   human_signoff`. A Unit whose only possible producer is a model is **not
//!   a Unit**; it is a quality dimension."
//! - "`blast_radius = irreversible` ⟹ `authority = human`. Any parse
//!   producing another combination is an error." (AK-006)
//! - "`blast_radius ∈ {cross_cutting, irreversible}` ⟹ authority MUST NOT be
//!   `critic`, regardless of `verify_by`." (AK-007)
//! - "`status = settled` requires a non-null `evidence` ref **and** a
//!   `VerificationRecord` with `Outcome = PASS` for this unit id in this
//!   run."
//! - "A Unit's tier MAY be raised by an operator. It MUST NOT be lowered by
//!   any agent role. Tier downgrade is the canonical gaming move (§23.2,
//!   G7)."
//!
//! Every constructor path funnels through [`validate_coupling`]; the settled
//! state is reachable only through [`Unit::settle`], which inspects the
//! `VerificationRecord` itself.

use core::fmt;

use crate::values::{
    Authority, BlastRadius, EvidenceKind, EvidenceLocator, EvidenceRef, Outcome, ResidualId,
    RoleId, RunId, UnitId, UnitStatus, VerificationRef, VerifyBy,
};
use crate::verification::VerificationRecord;
use crate::wire::wire_enum;

wire_enum! {
    /// Spec §5.3.2 `evidence_required.producer`:
    /// `instrument | human   (never: model assertion)`. Two variants only —
    /// a model can never be a producer because the type has no such state.
    pub enum Producer {
        Instrument, Human,
    }
}

/// Who is attempting a governance-relevant change to a Unit. Spec §5.3.2
/// distinguishes "an operator" (may raise) from "any agent role" (may not
/// lower).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Actor {
    Operator,
    Agent(RoleId),
}

/// Which tier axis a refused change tried to move.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TierAxis {
    BlastRadius,
    Authority,
}

impl fmt::Display for TierAxis {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::BlastRadius => "blast radius",
            Self::Authority => "authority",
        };
        f.write_str(name)
    }
}

/// A field [`DraftUnit`] was missing when conversion was attempted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DraftField {
    Id,
    Claim,
    VerifyBy,
    BlastRadius,
    Authority,
    EvidenceRequirement,
}

impl fmt::Display for DraftField {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Id => "id",
            Self::Claim => "claim",
            Self::VerifyBy => "verify_by",
            Self::BlastRadius => "blast_radius",
            Self::Authority => "authority",
            Self::EvidenceRequirement => "evidence_required",
        };
        f.write_str(name)
    }
}

/// `Unit` invariant failures — typed, carrying the offending values so the
/// caller can diagnose and recover (AGENTS.md: typed errors with failure
/// locus; never a panic, never a bool).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnitError {
    /// AK-006: irreversible work requires human authority.
    IrreversibleRequiresHuman {
        blast_radius: BlastRadius,
        authority: Authority,
    },
    /// AK-007: critic authority is forbidden for cross-cutting or
    /// irreversible blast radius, regardless of `verify_by`.
    CriticForbiddenForBlastRadius { blast_radius: BlastRadius },
    /// The producer rule: instrument unless `verify_by = human_signoff`.
    ProducerMustBeInstrument {
        verify_by: VerifyBy,
        producer: Producer,
    },
    /// `status = settled` without a non-null evidence ref (spec §5.3.2).
    SettledWithoutEvidence { unit_id: UnitId },
    /// Settling against a verification record whose outcome is not PASS —
    /// INCONCLUSIVE never satisfies a settlement requirement (AK-005).
    SettledWithoutPassVerification { unit_id: UnitId, outcome: Outcome },
    /// The verification record belongs to a different Unit.
    VerificationForDifferentUnit { expected: UnitId, actual: UnitId },
    /// The verification record was produced in a different run. Spec §5.3.2:
    /// settlement requires a PASS record "for this unit id **in this run**";
    /// §16.1: "A `VerificationRecord` with `Outcome = PASS` exists for it in
    /// this run." A record with the right unit id and the right outcome,
    /// replayed from a foreign run, is stale evidence and does not settle.
    VerificationFromDifferentRun { expected: RunId, actual: RunId },
    /// A status change the Unit's state does not permit.
    StatusTransitionInvalid { from: UnitStatus, to: UnitStatus },
    /// A draft was converted while a required field was absent.
    DraftIncomplete { field: DraftField },
    /// Settled state cannot be rehydrated from a draft: it is produced only
    /// by [`Unit::settle`] against a live `VerificationRecord`.
    SettledRequiresLiveSettlement { unit_id: UnitId },
    /// An agent role attempted any tier change; only operators retier.
    AgentMayNotChangeTier { actor: RoleId },
    /// Somebody attempted to lower a tier axis — forbidden for everyone
    /// (spec §5.3.2, §23.2 G7).
    TierLoweringForbidden {
        axis: TierAxis,
        blast_from: BlastRadius,
        blast_to: BlastRadius,
        authority_from: Authority,
        authority_to: Authority,
    },
}

impl fmt::Display for UnitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IrreversibleRequiresHuman {
                blast_radius,
                authority,
            } => write!(
                f,
                "Unit tier/authority coupling violated: {blast_radius} requires human authority, \
                 got {authority} (spec 5.3.2, AK-006)"
            ),
            Self::CriticForbiddenForBlastRadius { blast_radius } => write!(
                f,
                "Unit tier/authority coupling violated: {blast_radius} forbids critic authority, \
                 regardless of verify_by (spec 5.3.2, AK-007)"
            ),
            Self::ProducerMustBeInstrument {
                verify_by,
                producer,
            } => write!(
                f,
                "Unit evidence producer must be instrument unless verify_by is human signoff; \
                 got producer {producer} with verify_by {verify_by} (spec 5.3.2)"
            ),
            Self::SettledWithoutEvidence { unit_id } => write!(
                f,
                "Unit {unit_id} cannot be settled without a non-null evidence ref (spec 5.3.2)"
            ),
            Self::SettledWithoutPassVerification { unit_id, outcome } => write!(
                f,
                "Unit {unit_id} cannot settle on outcome {outcome}: only PASS settles, and \
                 INCONCLUSIVE never satisfies a settlement requirement (spec 5.3.2, AK-005)"
            ),
            Self::VerificationForDifferentUnit { expected, actual } => write!(
                f,
                "verification record is for Unit {actual}, not {expected} (spec 5.3.2)"
            ),
            Self::VerificationFromDifferentRun { expected, actual } => write!(
                f,
                "verification record is from run {actual}, not {expected}: a PASS record settles \
                 only its own run's Unit, never a replayed or foreign one (spec 5.3.2, 16.1)"
            ),
            Self::StatusTransitionInvalid { from, to } => {
                write!(f, "Unit status cannot move {from} to {to}")
            }
            Self::DraftIncomplete { field } => {
                write!(f, "draft Unit is missing required field {field}")
            }
            Self::SettledRequiresLiveSettlement { unit_id } => write!(
                f,
                "Unit {unit_id}: settled state is produced only by settling against a live \
                 verification record, never rehydrated from a draft (spec 5.3.2)"
            ),
            Self::AgentMayNotChangeTier { actor } => write!(
                f,
                "agent role {actor} may not change a Unit's tier or authority; only an operator \
                 may raise them (spec 5.3.2, 23.2 G7)"
            ),
            Self::TierLoweringForbidden {
                axis,
                blast_from,
                blast_to,
                authority_from,
                authority_to,
            } => write!(
                f,
                "tier lowering refused on the {axis} axis ({blast_from}/{authority_from} to \
                 {blast_to}/{authority_to}): tiers are raised, never lowered (spec 5.3.2, 23.2 \
                 G7)"
            ),
        }
    }
}

impl std::error::Error for UnitError {}

/// What would prove a Unit's claim (spec §5.3.2 `evidence_required`):
/// REQUIRED — a Unit without evidence requirements does not exist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceRequirement {
    kind: EvidenceKind,
    locator: EvidenceLocator,
    producer: Producer,
}

impl EvidenceRequirement {
    /// Evidence of `kind` at `locator`, produced by `producer`.
    pub fn new(kind: EvidenceKind, locator: EvidenceLocator, producer: Producer) -> Self {
        Self {
            kind,
            locator,
            producer,
        }
    }

    pub fn kind(&self) -> EvidenceKind {
        self.kind
    }

    pub fn locator(&self) -> &EvidenceLocator {
        &self.locator
    }

    pub fn producer(&self) -> Producer {
        self.producer
    }
}

/// Raises-only ordering on the tier axis (spec §5.3.2). Declaration order in
/// [`BlastRadius`] is the severity order: contained < local < cross_cutting <
/// irreversible.
pub(crate) fn blast_rank(blast: BlastRadius) -> u8 {
    match blast {
        BlastRadius::Contained => 0,
        BlastRadius::Local => 1,
        BlastRadius::CrossCutting => 2,
        BlastRadius::Irreversible => 3,
    }
}

/// Raises-only ordering on the authority axis (spec §5.3.2): critic <
/// lead_agent < human.
pub(crate) fn authority_rank(authority: Authority) -> u8 {
    match authority {
        Authority::Critic => 0,
        Authority::LeadAgent => 1,
        Authority::Human => 2,
    }
}

/// The §5.3.2 tier/authority coupling, checked on every path that can set
/// these fields. Quoted rules:
///
/// - "`blast_radius = irreversible` ⟹ `authority = human`. Any parse
///   producing another combination is an error."
/// - "`blast_radius ∈ {cross_cutting, irreversible}` ⟹ authority MUST NOT be
///   `critic`, regardless of `verify_by`."
pub(crate) fn validate_coupling(
    blast_radius: BlastRadius,
    authority: Authority,
    verify_by: VerifyBy,
    producer: Producer,
) -> Result<(), UnitError> {
    if blast_radius == BlastRadius::Irreversible && authority != Authority::Human {
        return Err(UnitError::IrreversibleRequiresHuman {
            blast_radius,
            authority,
        });
    }
    if matches!(
        blast_radius,
        BlastRadius::CrossCutting | BlastRadius::Irreversible
    ) && authority == Authority::Critic
    {
        return Err(UnitError::CriticForbiddenForBlastRadius { blast_radius });
    }
    if producer != Producer::Instrument && verify_by != VerifyBy::HumanSignoff {
        return Err(UnitError::ProducerMustBeInstrument {
            verify_by,
            producer,
        });
    }
    Ok(())
}

/// A Unit under construction (plan Task 3 redesign trigger): fields arrive
/// incrementally — from a parser or a projection — and conversion into a
/// [`Unit`] runs every invariant. The draft itself enforces nothing; the
/// aggregate does. No `Default`: default values are §8.6 configuration, not
/// domain state.
#[derive(Debug, Clone)]
pub struct DraftUnit {
    pub id: Option<UnitId>,
    pub claim: Option<String>,
    pub verify_by: Option<VerifyBy>,
    pub blast_radius: Option<BlastRadius>,
    pub authority: Option<Authority>,
    pub evidence_required: Option<EvidenceRequirement>,
    pub depends_on: Vec<UnitId>,
    pub evidence: Option<EvidenceRef>,
    pub status: UnitStatus,
    pub attempt: u32,
    pub last_residual_ref: Option<ResidualId>,
    pub notes: Option<String>,
}

impl DraftUnit {
    /// An empty draft in the `open` status. No `Default` impl on purpose:
    /// default values are §8.6 configuration, not domain state, and an
    /// implicit draft would hide that a projection chose every field.
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            id: None,
            claim: None,
            verify_by: None,
            blast_radius: None,
            authority: None,
            evidence_required: None,
            depends_on: Vec::new(),
            evidence: None,
            status: UnitStatus::Open,
            attempt: 0,
            last_residual_ref: None,
            notes: None,
        }
    }
    /// Converts the draft into a validated Unit. Missing required fields,
    /// coupling violations, and the producer rule are typed errors. A draft
    /// carrying `status = settled` is refused: settled state is produced only
    /// by [`Unit::settle`] against a live `VerificationRecord` with
    /// `Outcome = PASS` (spec §5.3.2).
    pub fn try_into_unit(self) -> Result<Unit, UnitError> {
        let id = self.id.clone().ok_or(UnitError::DraftIncomplete {
            field: DraftField::Id,
        })?;
        let claim = self.claim.clone().ok_or(UnitError::DraftIncomplete {
            field: DraftField::Claim,
        })?;
        let verify_by = self.verify_by.ok_or(UnitError::DraftIncomplete {
            field: DraftField::VerifyBy,
        })?;
        let blast_radius = self.blast_radius.ok_or(UnitError::DraftIncomplete {
            field: DraftField::BlastRadius,
        })?;
        let authority = self.authority.ok_or(UnitError::DraftIncomplete {
            field: DraftField::Authority,
        })?;
        let evidence_required =
            self.evidence_required
                .clone()
                .ok_or(UnitError::DraftIncomplete {
                    field: DraftField::EvidenceRequirement,
                })?;
        if self.status == UnitStatus::Settled {
            return Err(UnitError::SettledRequiresLiveSettlement { unit_id: id });
        }
        let mut unit = Unit::new(
            id,
            claim,
            verify_by,
            blast_radius,
            authority,
            evidence_required,
            self.depends_on,
        )?;
        unit.status = self.status;
        unit.attempt = self.attempt;
        unit.last_residual_ref = self.last_residual_ref;
        unit.notes = self.notes;
        Ok(unit)
    }
}

/// A checkable claim from the answer key (spec §5.3.2). Invariants hold at
/// construction and at every transition method; there is no public path to an
/// invalid state.
#[derive(Debug, Clone)]
pub struct Unit {
    id: UnitId,
    claim: String,
    verify_by: VerifyBy,
    blast_radius: BlastRadius,
    authority: Authority,
    evidence_required: EvidenceRequirement,
    evidence: Option<EvidenceRef>,
    verification: Option<VerificationRef>,
    status: UnitStatus,
    depends_on: Vec<UnitId>,
    attempt: u32,
    last_residual_ref: Option<ResidualId>,
    notes: Option<String>,
}

impl Unit {
    /// Creates a Unit in the `open` status with no evidence. Runs the full
    /// §5.3.2 coupling and producer validation; invalid combinations are a
    /// typed error, never a silent default (AK-004, AK-006, AK-007).
    pub fn new(
        id: UnitId,
        claim: String,
        verify_by: VerifyBy,
        blast_radius: BlastRadius,
        authority: Authority,
        evidence_required: EvidenceRequirement,
        depends_on: Vec<UnitId>,
    ) -> Result<Self, UnitError> {
        if claim.trim().is_empty() {
            return Err(UnitError::DraftIncomplete {
                field: DraftField::Claim,
            });
        }
        validate_coupling(
            blast_radius,
            authority,
            verify_by,
            evidence_required.producer(),
        )?;
        Ok(Self {
            id,
            claim,
            verify_by,
            blast_radius,
            authority,
            evidence_required,
            evidence: None,
            verification: None,
            status: UnitStatus::Open,
            depends_on,
            attempt: 0,
            last_residual_ref: None,
            notes: None,
        })
    }

    pub fn id(&self) -> &UnitId {
        &self.id
    }

    pub fn claim(&self) -> &str {
        &self.claim
    }

    pub fn verify_by(&self) -> VerifyBy {
        self.verify_by
    }

    pub fn blast_radius(&self) -> BlastRadius {
        self.blast_radius
    }

    pub fn authority(&self) -> Authority {
        self.authority
    }

    pub fn evidence_required(&self) -> &EvidenceRequirement {
        &self.evidence_required
    }

    /// The pinned evidence ref, present only when settled (spec §5.3.2
    /// `evidence: null # filled at settlement with an EvidenceRef`).
    pub fn evidence(&self) -> Option<&EvidenceRef> {
        self.evidence.as_ref()
    }

    /// The verification record that settled this Unit, if settled.
    pub fn verification(&self) -> Option<&VerificationRef> {
        self.verification.as_ref()
    }

    pub fn status(&self) -> UnitStatus {
        self.status
    }

    pub fn depends_on(&self) -> &[UnitId] {
        &self.depends_on
    }

    pub fn attempt(&self) -> u32 {
        self.attempt
    }

    pub fn last_residual_ref(&self) -> Option<&ResidualId> {
        self.last_residual_ref.as_ref()
    }

    pub fn notes(&self) -> Option<&str> {
        self.notes.as_deref()
    }

    /// Records one more attempt at this Unit (spec §5.3.2 `attempt`).
    pub fn record_attempt(&mut self) {
        self.attempt += 1;
    }

    /// Points at the residual currently open against this Unit.
    pub fn note_residual(&mut self, residual: ResidualId) {
        self.last_residual_ref = Some(residual);
    }

    /// Sets an operator note. Free-form, never load-bearing.
    pub fn set_notes(&mut self, notes: Option<String>) {
        self.notes = notes;
    }

    /// Whether this Unit is a hard gate (spec §5.3.1: "every hard-gate Unit
    /// MUST be individually representable in the answer key; a weighted
    /// score never absorbs a hard requirement"). Hard gates are the
    /// irreversible-tier Units.
    pub fn is_hard_gate(&self) -> bool {
        self.blast_radius == BlastRadius::Irreversible
    }

    /// Applies a status change that does not require evidence. Moves into
    /// `settled` are refused — a Unit settles only through [`Unit::settle`]
    /// against a live `VerificationRecord`; a bare transition attempt
    /// without pinned evidence is reported as exactly that (spec §5.3.2).
    /// Moves out of `settled` are refused — settlement is append-only in
    /// effect.
    pub fn transition(&mut self, to: UnitStatus) -> Result<(), UnitError> {
        use UnitStatus::*;
        let from = self.status;
        if to == Settled {
            if self.evidence.is_none() {
                return Err(UnitError::SettledWithoutEvidence {
                    unit_id: self.id.clone(),
                });
            }
            // Evidence present but no live PASS record was presented; only
            // settle() can close that gap.
            return Err(UnitError::StatusTransitionInvalid { from, to });
        }
        let allowed = matches!(
            (from, to),
            (Open, InProgress)
                | (Open, Blocked)
                | (Open, OutOfScope)
                | (InProgress, Blocked)
                | (InProgress, OutOfScope)
                | (InProgress, Open)
                | (Blocked, InProgress)
                | (Blocked, OutOfScope)
        );
        if !allowed {
            return Err(UnitError::StatusTransitionInvalid { from, to });
        }
        self.status = to;
        Ok(())
    }

    /// Settles this Unit: spec §5.3.2 — "`status = settled` requires a
    /// non-null `evidence` ref **and** a `VerificationRecord` with
    /// `Outcome = PASS` for this unit id in this run."
    ///
    /// Checked here, exactly: the record's `unit_id` must match this Unit,
    /// the record's `run_id` must equal `run` (the run context is a
    /// parameter — a Unit does not know its run), and its outcome must be
    /// `PASS`; `FAIL` and `INCONCLUSIVE` both refuse, and INCONCLUSIVE is
    /// not a soft PASS (AK-005). The `evidence` ref is pinned by this call.
    /// Not checked here: whether the ref resolves and is pinned in the
    /// evidence store — that is the durable layer's conjunct (spec §16.1;
    /// plan Task 13).
    pub fn settle(
        &mut self,
        run: RunId,
        evidence: EvidenceRef,
        verification: &VerificationRecord,
    ) -> Result<(), UnitError> {
        if verification.unit_id() != &self.id {
            return Err(UnitError::VerificationForDifferentUnit {
                expected: self.id.clone(),
                actual: verification.unit_id().clone(),
            });
        }
        if verification.run_id() != &run {
            return Err(UnitError::VerificationFromDifferentRun {
                expected: run,
                actual: verification.run_id().clone(),
            });
        }
        if verification.outcome() != Outcome::Pass {
            return Err(UnitError::SettledWithoutPassVerification {
                unit_id: self.id.clone(),
                outcome: verification.outcome(),
            });
        }
        if self.status == UnitStatus::Settled {
            // Settling twice is a status transition out of nothing; the
            // durable layer owns replay idempotency (plan Task 13).
            return Err(UnitError::StatusTransitionInvalid {
                from: UnitStatus::Settled,
                to: UnitStatus::Settled,
            });
        }
        self.evidence = Some(evidence);
        self.verification = Some(verification.id().clone());
        self.status = UnitStatus::Settled;
        Ok(())
    }

    /// Retiers a Unit. Spec §5.3.2: "A Unit's tier MAY be raised by an
    /// operator. It MUST NOT be lowered by any agent role."
    ///
    /// - An agent actor is refused for ANY change (raising included) — tier
    ///   changes are an operator act.
    /// - Lowering either axis is refused for everyone.
    /// - The resulting pair must still satisfy the §5.3.2 coupling.
    pub fn retier(
        &mut self,
        actor: Actor,
        blast_radius: BlastRadius,
        authority: Authority,
    ) -> Result<(), UnitError> {
        if let Actor::Agent(role) = actor {
            return Err(UnitError::AgentMayNotChangeTier { actor: role });
        }
        if blast_rank(blast_radius) < blast_rank(self.blast_radius)
            || authority_rank(authority) < authority_rank(self.authority)
        {
            return Err(UnitError::TierLoweringForbidden {
                axis: if blast_rank(blast_radius) < blast_rank(self.blast_radius) {
                    TierAxis::BlastRadius
                } else {
                    TierAxis::Authority
                },
                blast_from: self.blast_radius,
                blast_to: blast_radius,
                authority_from: self.authority,
                authority_to: authority,
            });
        }
        validate_coupling(
            blast_radius,
            authority,
            self.verify_by,
            self.evidence_required.producer(),
        )?;
        self.blast_radius = blast_radius;
        self.authority = authority;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::values::{EvidenceLocator, EvidenceRef, InstrumentName, VerificationRef};
    use crate::verification::VerificationRecord;

    fn requirement(producer: Producer) -> EvidenceRequirement {
        EvidenceRequirement::new(
            EvidenceKind::TestReport,
            EvidenceLocator::new("locator-0001").expect("valid locator"),
            producer,
        )
    }

    fn unit_id() -> UnitId {
        UnitId::new("AK-014").expect("spec example id")
    }

    fn run_id() -> RunId {
        RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("valid run id")
    }

    fn foreign_run_id() -> RunId {
        RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAZ").expect("valid but other")
    }

    fn claim() -> String {
        "The domain rejects invalid tier and authority pairings".to_owned()
    }

    fn build(
        blast_radius: BlastRadius,
        authority: Authority,
        verify_by: VerifyBy,
        producer: Producer,
    ) -> Result<Unit, UnitError> {
        Unit::new(
            unit_id(),
            claim(),
            verify_by,
            blast_radius,
            authority,
            requirement(producer),
            Vec::new(),
        )
    }

    fn build_draft(
        blast_radius: BlastRadius,
        authority: Authority,
        verify_by: VerifyBy,
        producer: Producer,
    ) -> Result<Unit, UnitError> {
        let mut draft = DraftUnit::new();
        draft.id = Some(unit_id());
        draft.claim = Some(claim());
        draft.verify_by = Some(verify_by);
        draft.blast_radius = Some(blast_radius);
        draft.authority = Some(authority);
        draft.evidence_required = Some(requirement(producer));
        draft.try_into_unit()
    }

    fn pass_record_for(run: &RunId, id: &UnitId, outcome: Outcome) -> VerificationRecord {
        VerificationRecord::new(
            VerificationRef::new("vrf-000042").expect("valid ref"),
            run.clone(),
            id.clone(),
            EvidenceRef::new("evd-000042").expect("valid ref"),
            outcome,
            InstrumentName::new("verifier-01").expect("valid name"),
        )
    }

    fn evidence() -> EvidenceRef {
        EvidenceRef::new("evd-000042").expect("valid ref")
    }

    #[test]
    fn exhaustive_tier_authority_coupling_holds_on_every_construction_path() {
        // Spec §5.3.2:
        // - "`blast_radius = irreversible` ⟹ `authority = human`. Any parse
        //    producing another combination is an error." (AK-006)
        // - "`blast_radius ∈ {cross_cutting, irreversible}` ⟹ authority MUST
        //    NOT be `critic`, regardless of `verify_by`." (AK-007)
        //
        // EXHAUSTIVE over BlastRadius × Authority × VerifyBy, through BOTH
        // public construction paths, with the expected error kind decided
        // independently from the spec text — never from the implementation.
        for blast_radius in BlastRadius::ALL.iter().copied() {
            for authority in Authority::ALL.iter().copied() {
                for verify_by in VerifyBy::ALL.iter().copied() {
                    let irreversibly_unmanned =
                        blast_radius == BlastRadius::Irreversible && authority != Authority::Human;
                    let critic_forbidden = matches!(
                        blast_radius,
                        BlastRadius::CrossCutting | BlastRadius::Irreversible
                    ) && authority == Authority::Critic;

                    for built in [
                        build(blast_radius, authority, verify_by, Producer::Instrument),
                        build_draft(blast_radius, authority, verify_by, Producer::Instrument),
                    ] {
                        match (built, irreversibly_unmanned, critic_forbidden) {
                            (Err(UnitError::IrreversibleRequiresHuman { .. }), true, _) => {}
                            (Err(UnitError::CriticForbiddenForBlastRadius { .. }), false, true) => {
                            }
                            (Ok(_), false, false) => {}
                            (outcome, _, _) => panic!(
                                "blast {blast_radius} authority {authority} verify {verify_by}: \
                                 wrong outcome {outcome:?}"
                            ),
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn irreversible_work_requires_human_authority_on_every_path() {
        // AK-006: the canonical gaming move, refused wherever it is tried.
        for authority in [Authority::Critic, Authority::LeadAgent] {
            assert!(matches!(
                build(
                    BlastRadius::Irreversible,
                    authority,
                    VerifyBy::StaticCheck,
                    Producer::Instrument
                ),
                Err(UnitError::IrreversibleRequiresHuman { .. })
            ));
            assert!(matches!(
                build_draft(
                    BlastRadius::Irreversible,
                    authority,
                    VerifyBy::StaticCheck,
                    Producer::Instrument
                ),
                Err(UnitError::IrreversibleRequiresHuman { .. })
            ));
            // Retiering a valid Unit up to irreversible without human
            // authority is the same violation later in the lifecycle. The
            // authority axis stays put, so only the coupling can refuse.
            let mut unit = build(
                BlastRadius::CrossCutting,
                Authority::LeadAgent,
                VerifyBy::StaticCheck,
                Producer::Instrument,
            )
            .expect("cc/lead is valid");
            assert!(matches!(
                unit.retier(
                    Actor::Operator,
                    BlastRadius::Irreversible,
                    Authority::LeadAgent
                ),
                Err(UnitError::IrreversibleRequiresHuman { .. })
            ));
            let mut unit = build(
                BlastRadius::Local,
                Authority::Critic,
                VerifyBy::StaticCheck,
                Producer::Instrument,
            )
            .expect("local/critic is valid");
            assert!(matches!(
                unit.retier(
                    Actor::Operator,
                    BlastRadius::Irreversible,
                    Authority::Critic
                ),
                Err(UnitError::IrreversibleRequiresHuman { .. })
            ));
        }
        // The one legal shape at that tier.
        let _ = build(
            BlastRadius::Irreversible,
            Authority::Human,
            VerifyBy::StaticCheck,
            Producer::Instrument,
        )
        .expect("irreversible with human authority constructs");
    }

    #[test]
    fn evidence_producer_must_be_instrument_unless_human_signoff() {
        // Spec §5.3.2: "`evidence_required.producer` MUST be `instrument`
        // unless `verify_by = human_signoff`. A Unit whose only possible
        // producer is a model is **not a Unit**; it is a quality dimension."
        for verify_by in VerifyBy::ALL {
            let built = build(
                BlastRadius::Local,
                Authority::Critic,
                *verify_by,
                Producer::Human,
            );
            if *verify_by == VerifyBy::HumanSignoff {
                let _ = built.expect("human producer with human signoff");
            } else {
                assert!(
                    matches!(built, Err(UnitError::ProducerMustBeInstrument { .. })),
                    "human producer with {verify_by} must be refused"
                );
            }
        }
        // Instrument producer is legal with every verify_by.
        for verify_by in VerifyBy::ALL {
            let _ = build(
                BlastRadius::Local,
                Authority::Critic,
                *verify_by,
                Producer::Instrument,
            )
            .expect("instrument producer is legal with every verify_by");
        }
    }

    #[test]
    fn settling_requires_evidence_and_a_pass_verification_record() {
        // Spec §5.3.2: "`status = settled` requires a non-null `evidence`
        // ref **and** a `VerificationRecord` with `Outcome = PASS` for this
        // unit id in this run."
        let mut unit = build(
            BlastRadius::Local,
            Authority::Critic,
            VerifyBy::StaticCheck,
            Producer::Instrument,
        )
        .expect("valid unit");
        // No evidence, no verification: a bare transition is refused.
        assert!(matches!(
            unit.transition(UnitStatus::Settled),
            Err(UnitError::SettledWithoutEvidence { .. })
        ));
        assert_eq!(unit.status(), UnitStatus::Open);

        // FAIL does not settle.
        let record = pass_record_for(&run_id(), unit.id(), Outcome::Fail);
        assert!(matches!(
            unit.settle(run_id(), evidence(), &record),
            Err(UnitError::SettledWithoutPassVerification {
                outcome: Outcome::Fail,
                ..
            })
        ));
        // INCONCLUSIVE never satisfies a settlement requirement (AK-005).
        let record = pass_record_for(&run_id(), unit.id(), Outcome::Inconclusive);
        assert!(matches!(
            unit.settle(run_id(), evidence(), &record),
            Err(UnitError::SettledWithoutPassVerification {
                outcome: Outcome::Inconclusive,
                ..
            })
        ));
        // A record for a different Unit does not settle this one.
        let other_id = UnitId::new("AK-015").expect("valid id");
        let record = pass_record_for(&run_id(), &other_id, Outcome::Pass);
        assert!(matches!(
            unit.settle(run_id(), evidence(), &record),
            Err(UnitError::VerificationForDifferentUnit { .. })
        ));
        assert_eq!(unit.status(), UnitStatus::Open);

        // PASS with evidence settles.
        let record = pass_record_for(&run_id(), unit.id(), Outcome::Pass);
        unit.settle(run_id(), evidence(), &record)
            .expect("PASS settles");
        assert_eq!(unit.status(), UnitStatus::Settled);
        assert_eq!(unit.evidence(), Some(&evidence()));
        assert_eq!(
            unit.verification(),
            Some(&VerificationRef::new("vrf-000042").expect("valid ref"))
        );

        // Settling twice is refused: settlement is append-only in effect.
        let record = pass_record_for(&run_id(), unit.id(), Outcome::Pass);
        assert!(matches!(
            unit.settle(run_id(), evidence(), &record),
            Err(UnitError::StatusTransitionInvalid { .. })
        ));
        // And settled Units do not transition out.
        assert!(matches!(
            unit.transition(UnitStatus::Open),
            Err(UnitError::StatusTransitionInvalid { .. })
        ));
    }

    #[test]
    fn a_pass_record_from_a_foreign_run_does_not_settle() {
        // Spec §5.3.2 line 297: "… a `VerificationRecord` with `Outcome =
        // PASS` for this unit id **in this run**." Restated by §16.1: "A
        // `VerificationRecord` with `Outcome = PASS` exists for it in this
        // run." The stale-evidence class §16 exists to exclude: a record
        // replayed from a previous run has the right unit id and the right
        // outcome — and still must not settle.
        let mut unit = build(
            BlastRadius::Local,
            Authority::Critic,
            VerifyBy::StaticCheck,
            Producer::Instrument,
        )
        .expect("valid unit");
        let stale_record = pass_record_for(&foreign_run_id(), unit.id(), Outcome::Pass);
        assert!(matches!(
            unit.settle(run_id(), evidence(), &stale_record),
            Err(UnitError::VerificationFromDifferentRun { .. })
        ));
        assert_eq!(unit.status(), UnitStatus::Open);
        assert_eq!(unit.evidence(), None);
        // The same record settles when the run context matches its own.
        let own_record = pass_record_for(&foreign_run_id(), unit.id(), Outcome::Pass);
        unit.settle(foreign_run_id(), evidence(), &own_record)
            .expect("a PASS record settles the Unit in its own run");
        assert_eq!(unit.status(), UnitStatus::Settled);
    }

    #[test]
    fn a_draft_cannot_rehydrate_a_settled_unit() {
        // Settled state is produced only by settling against a live PASS
        // VerificationRecord — never parsed, drafted, or defaulted in.
        let mut draft = DraftUnit::new();
        draft.id = Some(unit_id());
        draft.claim = Some(claim());
        draft.verify_by = Some(VerifyBy::StaticCheck);
        draft.blast_radius = Some(BlastRadius::Local);
        draft.authority = Some(Authority::Critic);
        draft.evidence_required = Some(requirement(Producer::Instrument));
        draft.status = UnitStatus::Settled;
        assert!(matches!(
            draft.try_into_unit(),
            Err(UnitError::SettledRequiresLiveSettlement { .. })
        ));
    }

    #[test]
    fn drafts_report_which_required_field_is_missing() {
        for (field, stripper) in [
            (DraftField::Id, 0usize),
            (DraftField::Claim, 1),
            (DraftField::VerifyBy, 2),
            (DraftField::BlastRadius, 3),
            (DraftField::Authority, 4),
            (DraftField::EvidenceRequirement, 5),
        ] {
            let mut draft = DraftUnit::new();
            draft.id = Some(unit_id());
            draft.claim = Some(claim());
            draft.verify_by = Some(VerifyBy::StaticCheck);
            draft.blast_radius = Some(BlastRadius::Local);
            draft.authority = Some(Authority::Critic);
            draft.evidence_required = Some(requirement(Producer::Instrument));
            match stripper {
                0 => draft.id = None,
                1 => draft.claim = None,
                2 => draft.verify_by = None,
                3 => draft.blast_radius = None,
                4 => draft.authority = None,
                _ => draft.evidence_required = None,
            }
            assert!(
                matches!(draft.try_into_unit(), Err(UnitError::DraftIncomplete { field: f }) if f == field),
                "expected DraftIncomplete::{field} for slot {stripper}"
            );
        }
        // An empty claim is a missing claim.
        let mut draft = DraftUnit::new();
        draft.id = Some(unit_id());
        draft.claim = Some("   ".to_owned());
        draft.verify_by = Some(VerifyBy::StaticCheck);
        draft.blast_radius = Some(BlastRadius::Local);
        draft.authority = Some(Authority::Critic);
        draft.evidence_required = Some(requirement(Producer::Instrument));
        assert!(matches!(
            draft.try_into_unit(),
            Err(UnitError::DraftIncomplete { .. })
        ));
    }

    #[test]
    fn non_settling_status_transitions_follow_the_table() {
        let mut unit = build(
            BlastRadius::Local,
            Authority::Critic,
            VerifyBy::StaticCheck,
            Producer::Instrument,
        )
        .expect("valid unit");
        unit.transition(UnitStatus::InProgress)
            .expect("open to in progress");
        unit.transition(UnitStatus::Open)
            .expect("in progress back to open");
        unit.transition(UnitStatus::Blocked)
            .expect("open to blocked");
        unit.transition(UnitStatus::InProgress)
            .expect("blocked resumes");
        unit.transition(UnitStatus::OutOfScope)
            .expect("in progress to out of scope");
        // Reopening from out_of_scope is not in the table.
        assert!(matches!(
            unit.transition(UnitStatus::Open),
            Err(UnitError::StatusTransitionInvalid { .. })
        ));
    }

    #[test]
    fn an_operator_may_raise_but_nobody_may_lower() {
        // Spec §5.3.2: "A Unit's tier MAY be raised by an operator. It MUST
        // NOT be lowered by any agent role. Tier downgrade is the canonical
        // gaming move (§23.2, G7)."
        let mut unit = build(
            BlastRadius::Local,
            Authority::Critic,
            VerifyBy::StaticCheck,
            Producer::Instrument,
        )
        .expect("valid unit");
        // Raise: blast and authority both up, coupling preserved.
        unit.retier(
            Actor::Operator,
            BlastRadius::CrossCutting,
            Authority::LeadAgent,
        )
        .expect("operator raises both axes");
        assert_eq!(unit.blast_radius(), BlastRadius::CrossCutting);
        assert_eq!(unit.authority(), Authority::LeadAgent);
        // Nothing changes on a refused lowering (lead agent to critic is a
        // lowering, refused before the coupling is even consulted).
        assert!(matches!(
            unit.retier(
                Actor::Operator,
                BlastRadius::CrossCutting,
                Authority::Critic
            ),
            Err(UnitError::TierLoweringForbidden { .. })
        ));
        assert_eq!(unit.authority(), Authority::LeadAgent);

        // A genuine raise that lands on a forbidden pairing is also refused:
        // raising the tier cannot buy back critic authority (AK-007).
        let mut unit = build(
            BlastRadius::Local,
            Authority::Critic,
            VerifyBy::StaticCheck,
            Producer::Instrument,
        )
        .expect("local/critic is valid");
        assert!(matches!(
            unit.retier(
                Actor::Operator,
                BlastRadius::CrossCutting,
                Authority::Critic
            ),
            Err(UnitError::CriticForbiddenForBlastRadius { .. })
        ));
        assert_eq!(unit.blast_radius(), BlastRadius::Local);
    }

    #[test]
    fn every_rank_lowering_move_is_refused_for_every_actor() {
        // AK-046 at the domain level, exhaustively: for every valid starting
        // pairing, every move that lowers either axis is refused; agents are
        // refused for EVERY tier change, raise included.
        let valid_starts: Vec<(BlastRadius, Authority)> = BlastRadius::ALL
            .iter()
            .flat_map(|blast| {
                Authority::ALL
                    .iter()
                    .map(move |authority| (*blast, *authority))
            })
            .filter(|(blast, authority)| {
                build(
                    *blast,
                    *authority,
                    VerifyBy::StaticCheck,
                    Producer::Instrument,
                )
                .is_ok()
            })
            .collect();
        // 3 (contained) + 3 (local) + 2 (cross-cutting) + 1 (irreversible)
        // legal pairings, decided by the spec rules alone.
        assert_eq!(valid_starts.len(), 9);

        for (blast_from, authority_from) in valid_starts {
            for blast_to in BlastRadius::ALL {
                for authority_to in Authority::ALL {
                    let lowers = blast_rank(*blast_to) < blast_rank(blast_from)
                        || authority_rank(*authority_to) < authority_rank(authority_from);
                    // Agents never retier, whatever the direction.
                    let mut unit = build(
                        blast_from,
                        authority_from,
                        VerifyBy::StaticCheck,
                        Producer::Instrument,
                    )
                    .expect("valid start");
                    assert!(
                        matches!(
                            unit.retier(Actor::Agent(RoleId::Critic), *blast_to, *authority_to),
                            Err(UnitError::AgentMayNotChangeTier { .. })
                        ),
                        "agent retier {blast_from} to {blast_to} must be refused"
                    );
                    assert_eq!(unit.blast_radius(), blast_from, "refusal changed nothing");

                    let mut unit = build(
                        blast_from,
                        authority_from,
                        VerifyBy::StaticCheck,
                        Producer::Instrument,
                    )
                    .expect("valid start");
                    let outcome = unit.retier(Actor::Operator, *blast_to, *authority_to);
                    if lowers {
                        assert!(
                            matches!(outcome, Err(UnitError::TierLoweringForbidden { .. })),
                            "lowering {blast_from}/{authority_from} to {blast_to}/{authority_to} \
                             must be refused"
                        );
                        assert_eq!(unit.blast_radius(), blast_from, "refusal changed nothing");
                        assert_eq!(unit.authority(), authority_from, "refusal changed nothing");
                    }
                }
            }
        }
    }

    #[test]
    fn tier_lowering_errors_carry_the_axis_and_both_ends() {
        let mut unit = build(
            BlastRadius::CrossCutting,
            Authority::Human,
            VerifyBy::StaticCheck,
            Producer::Instrument,
        )
        .expect("cc/human is valid");
        assert!(matches!(
            unit.retier(Actor::Operator, BlastRadius::Local, Authority::Human),
            Err(UnitError::TierLoweringForbidden {
                axis: TierAxis::BlastRadius,
                ..
            })
        ));
        let mut unit = build(
            BlastRadius::Local,
            Authority::Human,
            VerifyBy::StaticCheck,
            Producer::Instrument,
        )
        .expect("local/human is valid");
        assert!(matches!(
            unit.retier(Actor::Operator, BlastRadius::Local, Authority::LeadAgent),
            Err(UnitError::TierLoweringForbidden {
                axis: TierAxis::Authority,
                ..
            })
        ));
    }

    #[test]
    fn hard_gates_are_the_irreversible_tier() {
        // Spec §5.3.1: hard-gate Units are individually representable and a
        // weighted score never absorbs them; here, is_hard_gate names them.
        let mut unit = build(
            BlastRadius::Local,
            Authority::Critic,
            VerifyBy::StaticCheck,
            Producer::Instrument,
        )
        .expect("valid unit");
        assert!(!unit.is_hard_gate());
        unit.retier(Actor::Operator, BlastRadius::Irreversible, Authority::Human)
            .expect("operator raises to the hard tier");
        assert!(unit.is_hard_gate());
    }

    #[test]
    fn attempts_and_residuals_are_recorded_on_the_unit() {
        let mut unit = build(
            BlastRadius::Local,
            Authority::Critic,
            VerifyBy::StaticCheck,
            Producer::Instrument,
        )
        .expect("valid unit");
        assert_eq!(unit.attempt(), 0);
        unit.record_attempt();
        unit.record_attempt();
        assert_eq!(unit.attempt(), 2);
        assert_eq!(unit.last_residual_ref(), None);
        let residual = crate::values::ResidualId::new("res-000007").expect("valid id");
        unit.note_residual(residual.clone());
        assert_eq!(unit.last_residual_ref(), Some(&residual));
        unit.set_notes(Some("operator context".to_owned()));
        assert_eq!(unit.notes(), Some("operator context"));
    }
}
