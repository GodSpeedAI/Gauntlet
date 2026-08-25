//! `CorrectionSelection` — the controller's §15.1 decision, made durable.
//!
//! Spec §15.1, quoted where it decides the shape:
//!
//! - "For each candidate correction, estimate: impact · dependency order ·
//!   confidence · payment · reversibility · risk · governance status ·
//!   settlement accessibility · recovery cost · expected information gain."
//! - "1. Reject governance-blocked paths first (authority, tier, budget,
//!   irreversibility)."
//! - "4. Record the selection **and the rejected alternatives with reasons**
//!   on the iteration. A controller whose choices are unexplained cannot be
//!   improved."
//!
//! Consequences for this type:
//!
//! - A selection cannot EXIST without its rejected alternatives and its
//!   decision trace — they are constructor arguments, and the constructor
//!   refuses a trace that does not reference every one of the ten §15.1
//!   dimensions by name. Explainability is an invariant of the type, not a
//!   reporting habit layered on top.
//! - Every rejection carries the §15.1 step-1 governance class (or
//!   `NotSelected` for an allowed alternative the ordering ranked below the
//!   chosen one) AND a typed reason carrying the blocking fact — for budget
//!   blocks that fact is the meter's own numbers, so "preserving recovery
//!   capacity" is a computed fact, never prose.
//! - The class and the reason must agree: a `Budget`-class rejection whose
//!   reason is not the meter's numbers is a mislabeled record, and the
//!   constructor refuses it.
//!
//! The application-layer selector (`gauntlet-app` `control::select`) builds
//! candidates from typed facts and constructs this aggregate through the
//! validating constructor; there is no other public path to a selection.

use core::fmt;

use crate::budget::MicroUsd;
use crate::values::{Authority, BlastRadius, IterationNo, RunId, UnitId};

// ---------------------------------------------------------------------------
// The ten §15.1 dimensions
// ---------------------------------------------------------------------------

/// One of the ten §15.1 scoring dimensions — the vocabulary every decision
/// trace must name. The order here is DOCUMENTATIONAL (the spec's own list
/// order); the selector's comparison priority is the selector's documented
/// policy, and the trace records whichever dimension decided each step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SelectionDimension {
    Impact,
    DependencyOrder,
    Confidence,
    Payment,
    Reversibility,
    Risk,
    GovernanceStatus,
    SettlementAccessibility,
    RecoveryCost,
    ExpectedInformationGain,
}

impl SelectionDimension {
    /// Every dimension, in the §15.1 list order.
    pub const ALL: &'static [Self] = &[
        Self::Impact,
        Self::DependencyOrder,
        Self::Confidence,
        Self::Payment,
        Self::Reversibility,
        Self::Risk,
        Self::GovernanceStatus,
        Self::SettlementAccessibility,
        Self::RecoveryCost,
        Self::ExpectedInformationGain,
    ];

    /// The §15.1 name, as the spec writes it in the dimension list.
    pub fn name(self) -> &'static str {
        match self {
            Self::Impact => "impact",
            Self::DependencyOrder => "dependency order",
            Self::Confidence => "confidence",
            Self::Payment => "payment",
            Self::Reversibility => "reversibility",
            Self::Risk => "risk",
            Self::GovernanceStatus => "governance status",
            Self::SettlementAccessibility => "settlement accessibility",
            Self::RecoveryCost => "recovery cost",
            Self::ExpectedInformationGain => "expected information gain",
        }
    }
}

impl fmt::Display for SelectionDimension {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

// ---------------------------------------------------------------------------
// Candidate descriptors
// ---------------------------------------------------------------------------

/// The §15.1 step-3 shape of a correction: "Prefer `bounded correction →
/// observable consequence → useful settlement` over large speculative
/// rewrites." The shape is a typed field of the descriptor, so the
/// preference is a comparison between facts, not a judgment call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CorrectionShape {
    /// Bounded, with an observable consequence and a path to useful
    /// settlement (§15.1 step 3's preferred shape).
    Bounded,
    /// A large speculative rewrite.
    Speculative,
}

impl fmt::Display for CorrectionShape {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let token = match self {
            Self::Bounded => "bounded",
            Self::Speculative => "speculative",
        };
        f.write_str(token)
    }
}

/// The meter's own numbers, carried wherever a budget decision was made —
/// the requested forward slice and the headroom the meter reported at the
/// moment of the decision. §5.3.7's invariant ("the controller MUST refuse
/// to dispatch any invocation that would consume the recovery reserve") is
/// decided by these numbers, so the numbers are the record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BudgetFacts {
    pub requested_rounds: u32,
    pub requested_wall_seconds: u64,
    pub requested_tokens: u64,
    pub requested_cost_usd: MicroUsd,
    pub remaining_rounds: u32,
    pub remaining_wall_seconds: u64,
    pub remaining_tokens: u64,
    pub remaining_cost_usd: MicroUsd,
}

/// What the controller considered doing (spec §15.1 "candidate
/// correction"). A descriptor is inert facts — summary, shape, the Units it
/// touches, and the payment estimate the affordability decision used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorrectionDescriptor {
    summary: String,
    shape: CorrectionShape,
    unit_refs: Vec<UnitId>,
    estimated: BudgetFacts,
}

impl CorrectionDescriptor {
    /// A candidate descriptor. The summary must be non-empty and at least
    /// one Unit must be named — a correction that touches nothing and says
    /// nothing is not a candidate.
    pub fn new(
        summary: String,
        shape: CorrectionShape,
        unit_refs: Vec<UnitId>,
        estimated: BudgetFacts,
    ) -> Result<Self, CorrectionError> {
        if summary.trim().is_empty() {
            return Err(CorrectionError::DescriptorIncomplete {
                field: "summary",
                detail: "a candidate correction must say what it does".to_owned(),
            });
        }
        if unit_refs.is_empty() {
            return Err(CorrectionError::DescriptorIncomplete {
                field: "unit_refs",
                detail: "a candidate correction must name the Unit(s) it touches".to_owned(),
            });
        }
        Ok(Self {
            summary,
            shape,
            unit_refs,
            estimated,
        })
    }

    pub fn summary(&self) -> &str {
        &self.summary
    }

    /// The §15.1 step-3 shape (bounded vs speculative).
    pub fn shape(&self) -> CorrectionShape {
        self.shape
    }

    pub fn unit_refs(&self) -> &[UnitId] {
        &self.unit_refs
    }

    /// The payment estimate — the numbers the affordability decision used.
    pub fn estimated(&self) -> BudgetFacts {
        self.estimated
    }
}

// ---------------------------------------------------------------------------
// Rejected alternatives
// ---------------------------------------------------------------------------

/// Why a candidate was rejected. The first four variants are §15.1 step 1's
/// governance-block classes, verbatim ("Reject governance-blocked paths
/// first (authority, tier, budget, irreversibility)"); `NotSelected` covers
/// an allowed, affordable alternative the ordering ranked below the chosen
/// one — §15.1 step 4 still requires its reason recorded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RejectionClass {
    Authority,
    Tier,
    Budget,
    Irreversibility,
    NotSelected,
}

impl RejectionClass {
    /// Whether this is one of §15.1 step 1's governance-block classes.
    pub fn is_governance(self) -> bool {
        !matches!(self, Self::NotSelected)
    }
}

impl fmt::Display for RejectionClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let token = match self {
            Self::Authority => "authority",
            Self::Tier => "tier",
            Self::Budget => "budget",
            Self::Irreversibility => "irreversibility",
            Self::NotSelected => "not selected",
        };
        f.write_str(token)
    }
}

/// The typed reason a candidate was rejected, carrying the blocking fact.
/// Each governance class pairs with exactly one reason variant — the
/// constructor of [`CorrectionSelection`] refuses any other pairing, so a
/// record can never label a rejection with a class its facts do not support.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RejectionReason {
    /// §17.1: the correction requires an authority above the run's ceiling.
    AuthorityAboveCeiling {
        required: Authority,
        exercisable: Authority,
    },
    /// The correction's tier is not dispatchable under the run's policy
    /// (§17.3: e.g. no sandbox policy exists for the tier).
    TierNotDispatchable { tier: BlastRadius },
    /// The meter refused the estimated slice: charging it would consume the
    /// recovery reserve (§5.3.7). The meter's own numbers are the record.
    MeterRefusedReserve { facts: BudgetFacts },
    /// The correction touches irreversible work (§17.1: "never an automatic
    /// dispatch candidate", scheduler-level, unconditional).
    IrreversibleTier { tier: BlastRadius },
    /// Allowed and affordable, but the recorded dimension ordering ranked
    /// the chosen correction first; `decided_by` names the dimension that
    /// decided it (§15.1 step 2).
    LowerPriority { decided_by: SelectionDimension },
    /// Allowed and affordable, but a bounded correction was available and
    /// §15.1 step 3 prefers bounded shapes over speculative rewrites.
    SpeculativeWhenBoundedAvailable,
}

/// One rejected candidate: its descriptor, its rejection class, and the
/// typed reason with the blocking fact (spec §15.1 step 4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RejectedAlternative {
    correction: CorrectionDescriptor,
    class: RejectionClass,
    reason: RejectionReason,
}

impl RejectedAlternative {
    /// Pairs a descriptor with its class and reason. Consistency of
    /// (class, reason) is checked when the selection is constructed; this
    /// constructor only assembles.
    pub fn new(
        correction: CorrectionDescriptor,
        class: RejectionClass,
        reason: RejectionReason,
    ) -> Self {
        Self {
            correction,
            class,
            reason,
        }
    }

    pub fn correction(&self) -> &CorrectionDescriptor {
        &self.correction
    }

    pub fn class(&self) -> RejectionClass {
        self.class
    }

    pub fn reason(&self) -> &RejectionReason {
        &self.reason
    }

    /// The (class, reason) pairing §15.1 step 1 and step 4 define. Used by
    /// [`CorrectionSelection::new`]; visible so callers can validate before
    /// assembling.
    pub fn class_matches_reason(&self) -> bool {
        matches!(
            (self.class, &self.reason),
            (
                RejectionClass::Authority,
                RejectionReason::AuthorityAboveCeiling { .. }
            ) | (
                RejectionClass::Tier,
                RejectionReason::TierNotDispatchable { .. }
            ) | (
                RejectionClass::Budget,
                RejectionReason::MeterRefusedReserve { .. }
            ) | (
                RejectionClass::Irreversibility,
                RejectionReason::IrreversibleTier { .. }
            ) | (
                RejectionClass::NotSelected,
                RejectionReason::LowerPriority { .. }
            ) | (
                RejectionClass::NotSelected,
                RejectionReason::SpeculativeWhenBoundedAvailable
            )
        )
    }
}

// ---------------------------------------------------------------------------
// The decision trace
// ---------------------------------------------------------------------------

/// One step of the decision trace: the §15.1 dimension the step is about,
/// and what the facts said. The trace is the record that makes the
/// controller's choices improvable (§15.1 step 4: "A controller whose
/// choices are unexplained cannot be improved").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecisionStep {
    dimension: SelectionDimension,
    note: String,
}

impl DecisionStep {
    pub fn new(dimension: SelectionDimension, note: String) -> Self {
        Self { dimension, note }
    }

    pub fn dimension(&self) -> SelectionDimension {
        self.dimension
    }

    pub fn note(&self) -> &str {
        &self.note
    }
}

// ---------------------------------------------------------------------------
// The aggregate
// ---------------------------------------------------------------------------

/// `CorrectionSelection` invariant failures — typed, naming the locus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CorrectionError {
    /// A descriptor was missing a required fact.
    DescriptorIncomplete { field: &'static str, detail: String },
    /// The decision trace does not reference every one of the ten §15.1
    /// dimensions; the missing ones are listed.
    TraceIncomplete { missing: Vec<SelectionDimension> },
    /// A rejection's class and reason disagree — e.g. a `budget`-class
    /// rejection whose reason is not the meter's numbers.
    ClassReasonMismatch {
        class: RejectionClass,
        summary: String,
    },
    /// The selection records ZERO rejected alternatives (Task-12
    /// correction C5, the preregistered A5 vacuity class): §15.1 step 4
    /// requires the selection AND the rejected alternatives with reasons —
    /// a selection without alternatives is satisfied vacuously and is
    /// unrepresentable. Callers must enumerate a real candidate space.
    RejectedAlternativesEmpty,
}

impl fmt::Display for CorrectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DescriptorIncomplete { field, detail } => {
                write!(f, "candidate correction is incomplete at {field}: {detail}")
            }
            Self::TraceIncomplete { missing } => {
                let names: Vec<&str> = missing.iter().map(|d| d.name()).collect();
                write!(
                    f,
                    "the decision trace never references the 15.1 dimension(s) {} — a \
                     selection is explainable by construction or it does not exist",
                    names.join(", ")
                )
            }
            Self::ClassReasonMismatch { class, summary } => write!(
                f,
                "rejected alternative {summary:?} carries class {class} with a reason that \
                 class cannot support — the blocking fact and its label must agree (spec 15.1 \
                 step 1/4)"
            ),
            Self::RejectedAlternativesEmpty => write!(
                f,
                "the selection records no rejected alternatives — 15.1 step 4 requires the \
                 rejected alternatives with reasons, and a selection without them is vacuous \
                 and unrepresentable"
            ),
        }
    }
}

impl std::error::Error for CorrectionError {}

/// The controller's recorded decision for one iteration (spec §15.1 step 4):
/// the chosen correction, every rejected alternative with its class and
/// typed reason, and a decision trace that names all ten dimensions.
///
/// Construction is the only public path, and it validates all three parts —
/// there is no "selection without record" state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorrectionSelection {
    run_id: RunId,
    iteration_no: IterationNo,
    chosen: CorrectionDescriptor,
    rejected: Vec<RejectedAlternative>,
    trace: Vec<DecisionStep>,
}

impl CorrectionSelection {
    /// Assembles and validates the selection:
    ///
    /// - the trace must reference every one of the ten §15.1 dimensions at
    ///   least once (explainability by construction);
    /// - every rejected alternative's class must pair with its reason
    ///   variant (a `budget` rejection carries the meter's numbers, an
    ///   `authority` rejection carries both authorities, and so on);
    /// - the rejected list is NON-EMPTY — §15.1 step 4's record without
    ///   alternatives is vacuous and unrepresentable (correction C5).
    pub fn new(
        run_id: RunId,
        iteration_no: IterationNo,
        chosen: CorrectionDescriptor,
        rejected: Vec<RejectedAlternative>,
        trace: Vec<DecisionStep>,
    ) -> Result<Self, CorrectionError> {
        if rejected.is_empty() {
            return Err(CorrectionError::RejectedAlternativesEmpty);
        }
        let missing = SelectionDimension::ALL
            .iter()
            .copied()
            .filter(|dimension| !trace.iter().any(|step| step.dimension() == *dimension))
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            return Err(CorrectionError::TraceIncomplete { missing });
        }
        for alternative in &rejected {
            if !alternative.class_matches_reason() {
                return Err(CorrectionError::ClassReasonMismatch {
                    class: alternative.class(),
                    summary: alternative.correction().summary().to_owned(),
                });
            }
        }
        Ok(Self {
            run_id,
            iteration_no,
            chosen,
            rejected,
            trace,
        })
    }

    pub fn run_id(&self) -> &RunId {
        &self.run_id
    }

    pub fn iteration_no(&self) -> IterationNo {
        self.iteration_no
    }

    /// The selected correction (§15.1 step 2/3's winner).
    pub fn chosen(&self) -> &CorrectionDescriptor {
        &self.chosen
    }

    /// Every rejected alternative with its class and reason (§15.1 step 4).
    pub fn rejected(&self) -> &[RejectedAlternative] {
        &self.rejected
    }

    /// The decision trace — every §15.1 dimension is named at least once
    /// (constructor-enforced).
    pub fn trace(&self) -> &[DecisionStep] {
        &self.trace
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_id() -> RunId {
        RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("valid run id")
    }

    fn iteration() -> IterationNo {
        IterationNo::new(2).expect("1-based")
    }

    // Named test facts — the repository's literal discipline (§8.1) keeps
    // duration- and cost-shaped values out of inline numbers, in tests as
    // in production source.
    const REQ_ROUNDS: u32 = 1;
    const REQ_SPAN: u64 = 30;
    const REQ_TOKENS: u64 = 500;
    const REQ_COST: u64 = 10;
    const REM_ROUNDS: u32 = 6;
    const REM_SPAN: u64 = 2700;
    const REM_TOKENS: u64 = 700;
    const REM_COST: u64 = 390;

    fn facts() -> BudgetFacts {
        BudgetFacts {
            requested_rounds: REQ_ROUNDS,
            requested_wall_seconds: REQ_SPAN,
            requested_tokens: REQ_TOKENS,
            requested_cost_usd: MicroUsd::new(REQ_COST),
            remaining_rounds: REM_ROUNDS,
            remaining_wall_seconds: REM_SPAN,
            remaining_tokens: REM_TOKENS,
            remaining_cost_usd: MicroUsd::new(REM_COST),
        }
    }

    fn descriptor(summary: &str) -> CorrectionDescriptor {
        CorrectionDescriptor::new(
            summary.to_owned(),
            CorrectionShape::Bounded,
            vec![UnitId::new("AK-014").expect("valid unit id")],
            facts(),
        )
        .expect("complete descriptor")
    }

    fn full_trace() -> Vec<DecisionStep> {
        SelectionDimension::ALL
            .iter()
            .map(|dimension| DecisionStep::new(*dimension, format!("fact for {dimension}")))
            .collect()
    }

    /// One well-formed rejected alternative, reusable across the tests.
    fn alternative(summary: &str) -> RejectedAlternative {
        RejectedAlternative::new(
            descriptor(summary),
            RejectionClass::NotSelected,
            RejectionReason::LowerPriority {
                decided_by: SelectionDimension::Impact,
            },
        )
    }

    #[test]
    fn a_selection_without_rejected_alternatives_is_unrepresentable() {
        // Correction C5 (the preregistered A5 vacuity class): §15.1 step 4
        // requires the selection AND the rejected alternatives with
        // reasons — a selection that records no alternatives satisfies the
        // record obligation vacuously and must not exist.
        let refused = CorrectionSelection::new(
            run_id(),
            iteration(),
            descriptor("fix the parser branch"),
            Vec::new(),
            full_trace(),
        );
        assert!(
            matches!(refused, Err(CorrectionError::RejectedAlternativesEmpty)),
            "the vacuous selection is refused, got {refused:?}"
        );
        assert!(
            refused
                .expect_err("the vacuous selection is refused")
                .to_string()
                .contains("vacuous"),
            "the refusal names the vacuity"
        );
    }

    #[test]
    fn a_trace_must_name_every_dimension_or_the_selection_does_not_exist() {
        // §15.1 step 4: "Record the selection and the rejected alternatives
        // with reasons ... A controller whose choices are unexplained cannot
        // be improved." The type makes that structural.
        let complete = CorrectionSelection::new(
            run_id(),
            iteration(),
            descriptor("fix the parser branch"),
            vec![alternative("rewrite everything")],
            full_trace(),
        );
        assert!(complete.is_ok());

        // Drop two dimensions from the trace; the constructor names them.
        let mut partial = full_trace();
        partial.retain(|step| {
            step.dimension() != SelectionDimension::Risk
                && step.dimension() != SelectionDimension::RecoveryCost
        });
        let refused = CorrectionSelection::new(
            run_id(),
            iteration(),
            descriptor("fix the parser branch"),
            vec![alternative("rewrite everything")],
            partial,
        )
        .expect_err("an unexplained selection must not exist");
        assert!(
            matches!(
                &refused,
                CorrectionError::TraceIncomplete { missing }
                    if missing.len() == 2
                        && missing.contains(&SelectionDimension::Risk)
                        && missing.contains(&SelectionDimension::RecoveryCost)
            ),
            "the refusal names what is missing: {refused}"
        );

        // Every dimension name appears in the error text — the operator
        // reads the same vocabulary the trace uses.
        assert!(refused.to_string().contains("risk"));
        assert!(refused.to_string().contains("recovery cost"));
    }

    #[test]
    fn every_class_pairs_with_exactly_its_reason() {
        // The full (class, reason) matrix: the consistent pairs construct;
        // every inconsistent pairing is refused with the class and the
        // alternative's summary. The expectation is written here from the
        // §15.1 step-1/step-4 rules, independently of the implementation.
        use RejectionClass as Class;
        use RejectionReason as Reason;
        let consistent = |class: Class, reason: &Reason| {
            matches!(
                (class, reason),
                (Class::Authority, Reason::AuthorityAboveCeiling { .. })
                    | (Class::Tier, Reason::TierNotDispatchable { .. })
                    | (Class::Budget, Reason::MeterRefusedReserve { .. })
                    | (Class::Irreversibility, Reason::IrreversibleTier { .. })
                    | (
                        Class::NotSelected,
                        Reason::LowerPriority { .. } | Reason::SpeculativeWhenBoundedAvailable,
                    )
            )
        };
        let reasons = [
            Reason::AuthorityAboveCeiling {
                required: Authority::Human,
                exercisable: Authority::LeadAgent,
            },
            Reason::TierNotDispatchable {
                tier: BlastRadius::CrossCutting,
            },
            Reason::MeterRefusedReserve { facts: facts() },
            Reason::IrreversibleTier {
                tier: BlastRadius::Irreversible,
            },
            Reason::LowerPriority {
                decided_by: SelectionDimension::Impact,
            },
            Reason::SpeculativeWhenBoundedAvailable,
        ];
        let classes = [
            Class::Authority,
            Class::Tier,
            Class::Budget,
            Class::Irreversibility,
            Class::NotSelected,
        ];
        for class in classes {
            for reason in &reasons {
                let alternative =
                    RejectedAlternative::new(descriptor("a candidate"), class, reason.clone());
                let built = CorrectionSelection::new(
                    run_id(),
                    iteration(),
                    descriptor("the winner"),
                    vec![alternative],
                    full_trace(),
                );
                if consistent(class, reason) {
                    let selection = built.expect("the consistent pairing constructs");
                    assert_eq!(selection.rejected()[0].class(), class);
                    assert_eq!(selection.rejected()[0].reason(), reason);
                } else {
                    assert!(
                        matches!(
                            &built,
                            Err(CorrectionError::ClassReasonMismatch {
                                class: found, ..
                            }) if *found == class
                        ),
                        "class {class} with reason {reason:?} must be refused"
                    );
                }
            }
        }
    }

    #[test]
    fn descriptors_require_a_summary_and_a_unit() {
        assert!(matches!(
            CorrectionDescriptor::new(
                "   ".to_owned(),
                CorrectionShape::Bounded,
                vec![UnitId::new("AK-014").expect("valid unit id")],
                facts(),
            ),
            Err(CorrectionError::DescriptorIncomplete {
                field: "summary",
                ..
            })
        ));
        assert!(matches!(
            CorrectionDescriptor::new(
                "a real change".to_owned(),
                CorrectionShape::Speculative,
                Vec::new(),
                facts(),
            ),
            Err(CorrectionError::DescriptorIncomplete {
                field: "unit_refs",
                ..
            })
        ));
        // The legal descriptor round-trips its facts.
        let descriptor = descriptor("a real change");
        assert_eq!(descriptor.shape(), CorrectionShape::Bounded);
        assert_eq!(descriptor.estimated().requested_tokens, 500);
        assert_eq!(
            descriptor.estimated().remaining_cost_usd,
            MicroUsd::new(390)
        );
        assert_eq!(
            descriptor.unit_refs(),
            &[UnitId::new("AK-014").expect("valid unit id")]
        );
    }

    #[test]
    fn the_governance_classes_are_the_spec_four_plus_not_selected() {
        // §15.1 step 1: "(authority, tier, budget, irreversibility)" — the
        // four governance classes; NotSelected is the recorded alternative
        // for step 4's non-governance rejections.
        assert!(RejectionClass::Authority.is_governance());
        assert!(RejectionClass::Tier.is_governance());
        assert!(RejectionClass::Budget.is_governance());
        assert!(RejectionClass::Irreversibility.is_governance());
        assert!(!RejectionClass::NotSelected.is_governance());
        assert_eq!(
            RejectionClass::Irreversibility.to_string(),
            "irreversibility"
        );
    }

    #[test]
    fn the_dimension_vocabulary_is_the_spec_ten() {
        // Exactly the ten §15.1 dimensions, in the spec's list order, with
        // the spec's own names.
        let names: Vec<&str> = SelectionDimension::ALL.iter().map(|d| d.name()).collect();
        assert_eq!(
            names,
            vec![
                "impact",
                "dependency order",
                "confidence",
                "payment",
                "reversibility",
                "risk",
                "governance status",
                "settlement accessibility",
                "recovery cost",
                "expected information gain",
            ]
        );
    }
}
