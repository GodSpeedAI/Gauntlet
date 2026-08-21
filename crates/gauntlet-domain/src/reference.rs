//! `Reference` — spec §5.3.1.
//!
//! Spec §5.3.1 aggregate invariants, quoted:
//!
//! - "A `Reference` is immutable once frozen into a `ReferenceVersion`.
//!   Mutation produces a new version with a new digest."
//! - "Every `blocking: true` quality dimension and every hard-gate Unit MUST
//!   be individually representable in the answer key; **a weighted score
//!   never absorbs a hard requirement.** 97/100 does not compensate for a
//!   violated invariant."
//! - "`unknowns` MUST NOT appear in `units` as if settled."
//!
//! Immutability is by absent mutation paths: neither [`Reference`] nor
//! [`ReferenceVersion`] exposes a setter or a `&mut` accessor. Hard
//! requirements are decided individually by [`hard_gate_aggregate`], which
//! has no weights in it at all.

use core::fmt;

use crate::unit::{EvidenceRequirement, Unit};
use crate::values::{
    Digest, DimensionId, EvidenceKind, EvidenceRef, ReferenceId, TerminalState, UnitId, Verdict,
};

/// Which §5.3.1 narrative field was missing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NarrativeField {
    Purpose,
    DesiredState,
}

impl fmt::Display for NarrativeField {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Purpose => "purpose",
            Self::DesiredState => "desired_state",
        };
        f.write_str(name)
    }
}

/// `Reference` invariant failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReferenceError {
    /// A required narrative field was empty (spec §5.3.1 `purpose`,
    /// `desired_state`).
    NarrativeMissing { field: NarrativeField },
    /// Two Units share an id.
    DuplicateUnit { id: UnitId },
    /// The settlement criteria name a hard-gate Unit that is not in the
    /// answer key — "every hard-gate Unit MUST be individually
    /// representable".
    HardUnitMissing { id: UnitId },
    /// Two quality dimensions share an id.
    DuplicateDimension { id: DimensionId },
    /// A `blocking: true` dimension with no evidence requirement cannot be
    /// individually represented, so it could only be averaged away.
    BlockingDimensionWithoutEvidence { id: DimensionId },
    /// An unknown's name collides with a Unit id: "unknowns MUST NOT appear
    /// in units as if settled."
    UnknownPresentAsUnit { name: String },
    /// An unknown arrived without a name — unknowns are "named, not silently
    /// resolved".
    UnknownUnnamed,
    /// Minimum reliability below one verification pass is not a reliability
    /// statement at all.
    MinimumReliabilityInvalid { passes: u32 },
}

impl fmt::Display for ReferenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NarrativeMissing { field } => {
                write!(f, "reference {field} is required and empty (spec 5.3.1)")
            }
            Self::DuplicateUnit { id } => write!(f, "two Units share id {id} (spec 5.3.1)"),
            Self::HardUnitMissing { id } => write!(
                f,
                "settlement names hard-gate Unit {id} that is not in the answer key — every \
                 hard-gate Unit MUST be individually representable (spec 5.3.1)"
            ),
            Self::DuplicateDimension { id } => {
                write!(f, "two quality dimensions share id {id} (spec 5.3.1)")
            }
            Self::BlockingDimensionWithoutEvidence { id } => write!(
                f,
                "blocking dimension {id} has no evidence requirement: a hard requirement that \
                 cannot be checked individually can only be averaged away (spec 5.3.1)"
            ),
            Self::UnknownPresentAsUnit { name } => write!(
                f,
                "unknown {name:?} also appears as a Unit id — unknowns MUST NOT appear in units \
                 as if settled (spec 5.3.1)"
            ),
            Self::UnknownUnnamed => {
                write!(
                    f,
                    "an unknown is unnamed — unknowns are named, not silently resolved"
                )
            }
            Self::MinimumReliabilityInvalid { passes } => write!(
                f,
                "minimum reliability of {passes} consecutive clean passes is below one (spec \
                 5.3.1)"
            ),
        }
    }
}

impl std::error::Error for ReferenceError {}

/// The frozen identity of a reference version (spec §5.3.1: `id`, `version`,
/// `digest` — "id + digest, frozen", §5.3.3). Immutable once constructed:
/// no mutation path exists. A changed manifest constructs a new
/// `ReferenceVersion` with a new digest.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ReferenceVersion {
    reference_id: ReferenceId,
    version: u32,
    digest: Digest,
    /// Immutable evidence holding the normalized frozen reference bytes.
    evidence_ref: EvidenceRef,
}

impl ReferenceVersion {
    /// Legacy constructor retained for old persisted/test projections. New
    /// freezes MUST use [`Self::with_evidence`]; an unbound version cannot
    /// construct a `FrozenReferenceProjection`.
    pub fn new(reference_id: ReferenceId, version: u32, digest: Digest) -> Self {
        Self::with_evidence(
            reference_id,
            version,
            digest,
            EvidenceRef::new("unbound-frozen-reference-evidence")
                .expect("the fixed legacy evidence marker is valid"),
        )
    }

    /// Constructs the complete §9.4 frozen identity, including the evidence
    /// object from which downstream projections must be rendered.
    pub fn with_evidence(
        reference_id: ReferenceId,
        version: u32,
        digest: Digest,
        evidence_ref: EvidenceRef,
    ) -> Self {
        Self {
            reference_id,
            version,
            digest,
            evidence_ref,
        }
    }

    pub fn reference_id(&self) -> &ReferenceId {
        &self.reference_id
    }

    pub fn version(&self) -> u32 {
        self.version
    }

    pub fn digest(&self) -> &Digest {
        &self.digest
    }

    /// The immutable evidence object containing the normalized frozen bytes.
    pub fn evidence_ref(&self) -> &EvidenceRef {
        &self.evidence_ref
    }
}

/// A named open question (spec §5.3.1 `unknowns` — "named, not silently
/// resolved"). An `Unknown` has no conversion into a [`Unit`]; the only
/// thing a runtime may do with one is resolve it into a new reference
/// version, never into a settled Unit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unknown {
    name: String,
    statement: String,
}

impl Unknown {
    pub fn new(name: String, statement: String) -> Self {
        Self { name, statement }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn statement(&self) -> &str {
        &self.statement
    }
}

/// A recorded assumption (spec §5.3.1 `assumptions` — "recorded,
/// revisitable").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Assumption {
    statement: String,
}

impl Assumption {
    pub fn new(statement: String) -> Self {
        Self { statement }
    }

    pub fn statement(&self) -> &str {
        &self.statement
    }
}

/// The definition data for a quality dimension (spec §5.3.1). Plain pub
/// fields: this is input awaiting validation by [`QualityDimension::new`].
#[derive(Debug, Clone)]
pub struct QualityDimensionDef {
    pub id: DimensionId,
    pub description: String,
    pub desired_condition: String,
    pub evidence_required: Vec<EvidenceRequirement>,
    pub comparison_method: String,
    pub threshold: Option<String>,
    pub weight: Option<u32>,
    pub blocking: bool,
}

/// One judgeable quality axis (spec §5.3.1 `quality_dimensions`). A
/// `blocking` dimension MUST carry its own evidence requirements — that is
/// what makes it "individually representable" rather than absorbable by a
/// weighted score.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QualityDimension {
    id: DimensionId,
    description: String,
    desired_condition: String,
    evidence_required: Vec<EvidenceRequirement>,
    comparison_method: String,
    threshold: Option<String>,
    weight: Option<u32>,
    blocking: bool,
}

impl QualityDimension {
    /// Validates the §5.3.1 blocking invariant: "a blocking dimension cannot
    /// be averaged away" — it must be individually checkable.
    pub fn new(def: QualityDimensionDef) -> Result<Self, ReferenceError> {
        if def.blocking && def.evidence_required.is_empty() {
            return Err(ReferenceError::BlockingDimensionWithoutEvidence { id: def.id });
        }
        Ok(Self {
            id: def.id,
            description: def.description,
            desired_condition: def.desired_condition,
            evidence_required: def.evidence_required,
            comparison_method: def.comparison_method,
            threshold: def.threshold,
            weight: def.weight,
            blocking: def.blocking,
        })
    }

    pub fn id(&self) -> &DimensionId {
        &self.id
    }

    pub fn description(&self) -> &str {
        &self.description
    }

    pub fn desired_condition(&self) -> &str {
        &self.desired_condition
    }

    pub fn evidence_required(&self) -> &[EvidenceRequirement] {
        &self.evidence_required
    }

    pub fn comparison_method(&self) -> &str {
        &self.comparison_method
    }

    pub fn threshold(&self) -> Option<&str> {
        self.threshold.as_deref()
    }

    /// The weight, for weighted aggregation of NON-blocking dimensions
    /// only. Blocking dimensions never enter a weighted score.
    pub fn weight(&self) -> Option<u32> {
        self.weight
    }

    pub fn is_blocking(&self) -> bool {
        self.blocking
    }
}

/// Settlement criteria (spec §5.3.1 `settlement`): success criteria, the
/// evidence kinds settlement depends on, minimum reliability ("e.g. 3
/// consecutive clean verification passes"), terminal conditions, and the
/// hard-gate Units that MUST be individually represented.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettlementCriteria {
    success_criteria: Vec<String>,
    required_evidence: Vec<EvidenceKind>,
    minimum_reliability_passes: u32,
    terminal_conditions: Vec<TerminalState>,
    hard_units: Vec<UnitId>,
}

impl SettlementCriteria {
    /// `minimum_reliability_passes` below one is refused: reliability below
    /// one clean pass is not a settlement bar.
    pub fn new(
        success_criteria: Vec<String>,
        required_evidence: Vec<EvidenceKind>,
        minimum_reliability_passes: u32,
        terminal_conditions: Vec<TerminalState>,
        hard_units: Vec<UnitId>,
    ) -> Result<Self, ReferenceError> {
        if minimum_reliability_passes == 0 {
            return Err(ReferenceError::MinimumReliabilityInvalid {
                passes: minimum_reliability_passes,
            });
        }
        Ok(Self {
            success_criteria,
            required_evidence,
            minimum_reliability_passes,
            terminal_conditions,
            hard_units,
        })
    }

    pub fn success_criteria(&self) -> &[String] {
        &self.success_criteria
    }

    pub fn required_evidence(&self) -> &[EvidenceKind] {
        &self.required_evidence
    }

    pub fn minimum_reliability_passes(&self) -> u32 {
        self.minimum_reliability_passes
    }

    pub fn terminal_conditions(&self) -> &[TerminalState] {
        &self.terminal_conditions
    }

    /// The hard-gate Units: individually required, never weighted.
    pub fn hard_units(&self) -> &[UnitId] {
        &self.hard_units
    }
}

/// Deterministic aggregation of unit and dimension verdicts under the §5.3.1
/// rule: "**a weighted score never absorbs a hard requirement.** 97/100 does
/// not compensate for a violated invariant."
///
/// There are no weights in this function — that is the mechanism. Any `Fail`
/// (blocking or not) forces `Fail`; with no fails, any `Inconclusive` yields
/// `Inconclusive` (missing evidence is not passing evidence, I8); only
/// uniform passes pass.
pub fn hard_gate_aggregate(
    unit_verdicts: &[(UnitId, Verdict)],
    dimension_verdicts: &[(DimensionId, Verdict)],
) -> Verdict {
    let all = unit_verdicts
        .iter()
        .map(|(_, verdict)| *verdict)
        .chain(dimension_verdicts.iter().map(|(_, verdict)| *verdict));
    let mut saw_inconclusive = false;
    for verdict in all {
        match verdict {
            Verdict::Fail => return Verdict::Fail,
            Verdict::Inconclusive => saw_inconclusive = true,
            Verdict::Pass => {}
        }
    }
    if saw_inconclusive {
        Verdict::Inconclusive
    } else {
        Verdict::Pass
    }
}

/// The definition data for a [`Reference`] (spec §5.3.1 manifest shape).
/// Plain pub fields awaiting validation.
#[derive(Debug, Clone)]
pub struct ReferenceDefinition {
    pub id: ReferenceId,
    pub version: u32,
    pub digest: Digest,
    pub source_refs: Vec<String>,
    pub purpose: String,
    pub desired_state: String,
    pub invariants: Vec<String>,
    pub constraints: Vec<String>,
    pub anti_goals: Vec<String>,
    pub units: Vec<Unit>,
    pub quality_dimensions: Vec<QualityDimension>,
    pub settlement: SettlementCriteria,
    pub unknowns: Vec<Unknown>,
    pub assumptions: Vec<Assumption>,
}

/// The frozen destination (spec §5.3.1). Immutable after construction — no
/// mutation paths — and convertible into its [`ReferenceVersion`] identity
/// on demand.
#[derive(Debug, Clone)]
pub struct Reference {
    id: ReferenceId,
    version: u32,
    digest: Digest,
    source_refs: Vec<String>,
    purpose: String,
    desired_state: String,
    invariants: Vec<String>,
    constraints: Vec<String>,
    anti_goals: Vec<String>,
    units: Vec<Unit>,
    quality_dimensions: Vec<QualityDimension>,
    settlement: SettlementCriteria,
    unknowns: Vec<Unknown>,
    assumptions: Vec<Assumption>,
}

impl Reference {
    /// Validates and adopts a manifest. Enforced here:
    ///
    /// - `purpose` and `desired_state` are present ("one paragraph: what
    ///   reaching the destination means");
    /// - Unit ids are unique;
    /// - every hard-gate Unit named by the settlement criteria exists among
    ///   the units;
    /// - unknowns are named and do not collide with any Unit id
    ///   ("`unknowns` MUST NOT appear in `units` as if settled");
    /// - quality dimension ids are unique.
    pub fn new(def: ReferenceDefinition) -> Result<Self, ReferenceError> {
        if def.purpose.trim().is_empty() {
            return Err(ReferenceError::NarrativeMissing {
                field: NarrativeField::Purpose,
            });
        }
        if def.desired_state.trim().is_empty() {
            return Err(ReferenceError::NarrativeMissing {
                field: NarrativeField::DesiredState,
            });
        }
        let mut seen_units = Vec::with_capacity(def.units.len());
        for unit in &def.units {
            let id = unit.id().clone();
            if seen_units.contains(&id) {
                return Err(ReferenceError::DuplicateUnit { id });
            }
            seen_units.push(id);
        }
        for hard in def.settlement.hard_units() {
            if !seen_units.contains(hard) {
                return Err(ReferenceError::HardUnitMissing { id: hard.clone() });
            }
        }
        for unknown in &def.unknowns {
            if unknown.name.trim().is_empty() {
                return Err(ReferenceError::UnknownUnnamed);
            }
            if let Some(id) = seen_units.iter().find(|id| id.as_str() == unknown.name) {
                return Err(ReferenceError::UnknownPresentAsUnit {
                    name: id.as_str().to_owned(),
                });
            }
        }
        let mut seen_dimensions = Vec::with_capacity(def.quality_dimensions.len());
        for dimension in &def.quality_dimensions {
            let id = dimension.id().clone();
            if seen_dimensions.contains(&id) {
                return Err(ReferenceError::DuplicateDimension { id });
            }
            seen_dimensions.push(id);
        }
        Ok(Self {
            id: def.id,
            version: def.version,
            digest: def.digest,
            source_refs: def.source_refs,
            purpose: def.purpose,
            desired_state: def.desired_state,
            invariants: def.invariants,
            constraints: def.constraints,
            anti_goals: def.anti_goals,
            units: def.units,
            quality_dimensions: def.quality_dimensions,
            settlement: def.settlement,
            unknowns: def.unknowns,
            assumptions: def.assumptions,
        })
    }

    /// The frozen identity of this version (spec §5.3.1: a Reference is
    /// "immutable once frozen into a ReferenceVersion").
    pub fn freeze(&self) -> ReferenceVersion {
        ReferenceVersion::new(self.id.clone(), self.version, self.digest.clone())
    }

    pub fn id(&self) -> &ReferenceId {
        &self.id
    }

    pub fn version(&self) -> u32 {
        self.version
    }

    pub fn digest(&self) -> &Digest {
        &self.digest
    }

    pub fn source_refs(&self) -> &[String] {
        &self.source_refs
    }

    pub fn purpose(&self) -> &str {
        &self.purpose
    }

    pub fn desired_state(&self) -> &str {
        &self.desired_state
    }

    pub fn invariants(&self) -> &[String] {
        &self.invariants
    }

    pub fn constraints(&self) -> &[String] {
        &self.constraints
    }

    pub fn anti_goals(&self) -> &[String] {
        &self.anti_goals
    }

    pub fn units(&self) -> &[Unit] {
        &self.units
    }

    pub fn quality_dimensions(&self) -> &[QualityDimension] {
        &self.quality_dimensions
    }

    pub fn settlement(&self) -> &SettlementCriteria {
        &self.settlement
    }

    pub fn unknowns(&self) -> &[Unknown] {
        &self.unknowns
    }

    pub fn assumptions(&self) -> &[Assumption] {
        &self.assumptions
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::unit::{EvidenceRequirement, Producer};
    use crate::values::{BlastRadius, EvidenceLocator, UnitId};

    fn digest(hex_tail: &str) -> Digest {
        Digest::new("sha256", hex_tail).expect("valid digest parts")
    }

    fn requirement() -> EvidenceRequirement {
        EvidenceRequirement::new(
            EvidenceKind::TestReport,
            EvidenceLocator::new("locator-0001").expect("valid locator"),
            Producer::Instrument,
        )
    }

    fn unit(id: &str) -> Unit {
        Unit::new(
            UnitId::new(id).expect("valid id"),
            "A checkable claim".to_owned(),
            crate::values::VerifyBy::StaticCheck,
            BlastRadius::Local,
            crate::values::Authority::Critic,
            requirement(),
            Vec::new(),
        )
        .expect("valid unit")
    }

    fn dimension(id: &str, blocking: bool) -> QualityDimension {
        QualityDimension::new(QualityDimensionDef {
            id: DimensionId::new(id).expect("valid dimension id"),
            description: "one axis".to_owned(),
            desired_condition: "it holds".to_owned(),
            evidence_required: if blocking {
                vec![requirement()]
            } else {
                Vec::new()
            },
            comparison_method: "count".to_owned(),
            threshold: None,
            weight: None,
            blocking,
        })
        .expect("valid dimension")
    }

    fn criteria(hard_units: Vec<UnitId>) -> SettlementCriteria {
        SettlementCriteria::new(
            vec!["the destination is reached".to_owned()],
            vec![EvidenceKind::TestReport],
            3,
            vec![TerminalState::Settled],
            hard_units,
        )
        .expect("valid criteria")
    }

    fn definition(units: Vec<Unit>) -> ReferenceDefinition {
        ReferenceDefinition {
            id: ReferenceId::new("ref-main").expect("valid id"),
            version: 4,
            digest: digest("a1b2c3d4"),
            source_refs: Vec::new(),
            purpose: "What reaching the destination means".to_owned(),
            desired_state: "The destination, narrated".to_owned(),
            invariants: Vec::new(),
            constraints: Vec::new(),
            anti_goals: Vec::new(),
            units,
            quality_dimensions: Vec::new(),
            settlement: criteria(Vec::new()),
            unknowns: Vec::new(),
            assumptions: Vec::new(),
        }
    }

    #[test]
    fn a_reference_requires_its_narrative_fields() {
        let mut def = definition(vec![unit("AK-014")]);
        def.purpose = "   ".to_owned();
        assert!(matches!(
            Reference::new(def),
            Err(ReferenceError::NarrativeMissing { .. })
        ));
        let mut def = definition(vec![unit("AK-014")]);
        def.desired_state = String::new();
        assert!(matches!(
            Reference::new(def),
            Err(ReferenceError::NarrativeMissing { .. })
        ));
    }

    #[test]
    fn unit_ids_are_unique_within_a_reference() {
        let def = definition(vec![unit("AK-014"), unit("AB-123"), unit("AK-014")]);
        assert!(matches!(
            Reference::new(def),
            Err(ReferenceError::DuplicateUnit { .. })
        ));
    }

    #[test]
    fn every_hard_gate_unit_must_exist_in_the_answer_key() {
        // Spec §5.3.1: "Every `blocking: true` quality dimension and every
        // hard-gate Unit MUST be individually representable in the answer
        // key; **a weighted score never absorbs a hard requirement.**"
        let mut def = definition(vec![unit("AK-014")]);
        def.settlement = criteria(vec![UnitId::new("AK-999").expect("valid id")]);
        assert!(matches!(
            Reference::new(def),
            Err(ReferenceError::HardUnitMissing { .. })
        ));
        let mut def = definition(vec![unit("AK-014")]);
        def.settlement = criteria(vec![UnitId::new("AK-014").expect("valid id")]);
        let _ = Reference::new(def).expect("present hard units construct");
    }

    #[test]
    fn unknowns_are_named_and_never_settled_as_units() {
        // Spec §5.3.1: "`unknowns` MUST NOT appear in `units` as if
        // settled."
        let mut def = definition(vec![unit("AK-014")]);
        def.unknowns = vec![Unknown::new(
            String::new(),
            "an unnamed question".to_owned(),
        )];
        assert!(matches!(
            Reference::new(def),
            Err(ReferenceError::UnknownUnnamed)
        ));
        let mut def = definition(vec![unit("AK-014")]);
        def.unknowns = vec![Unknown::new(
            "AK-014".to_owned(),
            "already a unit".to_owned(),
        )];
        assert!(matches!(
            Reference::new(def),
            Err(ReferenceError::UnknownPresentAsUnit { .. })
        ));
    }

    #[test]
    fn dimension_ids_are_unique() {
        let mut def = definition(Vec::new());
        def.quality_dimensions = vec![dimension("dim-01", false), dimension("dim-01", true)];
        assert!(matches!(
            Reference::new(def),
            Err(ReferenceError::DuplicateDimension { .. })
        ));
    }

    #[test]
    fn a_blocking_dimension_must_be_individually_checkable() {
        // "a blocking dimension cannot be averaged away" — without its own
        // evidence requirement it could only be absorbed by a weight.
        assert!(matches!(
            QualityDimension::new(QualityDimensionDef {
                id: DimensionId::new("dim-02").expect("valid id"),
                description: "uncheckable".to_owned(),
                desired_condition: "it holds".to_owned(),
                evidence_required: Vec::new(),
                comparison_method: "count".to_owned(),
                threshold: None,
                weight: Some(10),
                blocking: true,
            }),
            Err(ReferenceError::BlockingDimensionWithoutEvidence { .. })
        ));
    }

    #[test]
    fn minimum_reliability_below_one_pass_is_refused() {
        assert!(matches!(
            SettlementCriteria::new(Vec::new(), Vec::new(), 0, Vec::new(), Vec::new()),
            Err(ReferenceError::MinimumReliabilityInvalid { passes: 0 })
        ));
    }

    #[test]
    fn freezing_carries_the_version_identity() {
        let reference = Reference::new(definition(vec![unit("AK-014")])).expect("valid reference");
        let version = reference.freeze();
        assert_eq!(version.reference_id(), reference.id());
        assert_eq!(version.version(), 4);
        assert_eq!(version.digest(), reference.digest());
    }

    #[test]
    fn a_weighted_score_never_absorbs_a_hard_requirement() {
        // Spec §5.3.1 aggregation rule: any Fail forces Fail however many
        // passes surround it; with no fails, missing evidence is not
        // passing evidence (I8): Inconclusive dominates plain passes.
        let ak = |n: &str, verdict| (UnitId::new(n).expect("id"), verdict);
        let dim = |n: &str, verdict| (DimensionId::new(n).expect("id"), verdict);
        assert_eq!(
            hard_gate_aggregate(
                &[ak("AK-001", Verdict::Pass), ak("AK-002", Verdict::Pass)],
                &[dim("dim-01", Verdict::Fail)],
            ),
            Verdict::Fail
        );
        assert_eq!(
            hard_gate_aggregate(
                &[ak("AK-001", Verdict::Pass), ak("AK-002", Verdict::Fail)],
                &[dim("dim-01", Verdict::Pass)],
            ),
            Verdict::Fail
        );
        assert_eq!(
            hard_gate_aggregate(
                &[
                    ak("AK-001", Verdict::Pass),
                    ak("AK-002", Verdict::Inconclusive)
                ],
                &[dim("dim-01", Verdict::Pass)],
            ),
            Verdict::Inconclusive
        );
        assert_eq!(
            hard_gate_aggregate(
                &[ak("AK-001", Verdict::Pass), ak("AK-002", Verdict::Pass)],
                &[dim("dim-01", Verdict::Pass)],
            ),
            Verdict::Pass
        );
    }
}
