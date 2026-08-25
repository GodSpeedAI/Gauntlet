//! `CorrectionSelector` — spec §15.1, the controller's selection of the
//! next affordable correction.
//!
//! Spec §15.1, quoted where it decides behavior:
//!
//! - "For each candidate correction, estimate: impact · dependency order ·
//!   confidence · payment · reversibility · risk · governance status ·
//!   settlement accessibility · recovery cost · expected information gain."
//! - "1. Reject governance-blocked paths first (authority, tier, budget,
//!   irreversibility)."
//! - "2. Among allowed corrections, select the **next affordable
//!   correction** that most usefully reduces error while preserving
//!   recovery capacity."
//! - "3. Prefer `bounded correction → observable consequence → useful
//!   settlement` over large speculative rewrites."
//! - "4. Record the selection **and the rejected alternatives with
//!   reasons** on the iteration. A controller whose choices are unexplained
//!   cannot be improved."
//! - "It does not simply select the largest residual."
//!
//! ## Explainable by construction
//!
//! There is no numeric blend. Candidates are compared by an explicit
//! LEXICOGRAPHIC priority over the ten §15.1 dimensions — first difference
//! decides, the deciding dimension is recorded in the trace, and every
//! dimension is named at least once for the chosen candidate. The priority
//! (documented on [`CorrectionSelector::select`]) follows the §15.1
//! procedure's own structure: the step-3 bounded-over-speculative shape
//! preference leads (a speculative rewrite is the riskier shape — that is
//! the dimension it is recorded under), then "most usefully reduces error"
//! (impact), then the supporting dimensions, with payment late so a
//! marginally cheaper correction never beats a usefully more corrective
//! one, and deterministic tie-breaks last.
//!
//! ## Purity contract
//!
//! The selector holds no state and consults no clock or store. Its one
//! live consultation is the meter: affordability is decided by asking the
//! injected [`BudgetMeter`] `would_breach_reserve` at call time (§17.4),
//! and a refused candidate's rejection carries the meter's own numbers —
//! "preserving recovery capacity" is a computed fact, not prose. Durable
//! recording of the resulting [`CorrectionSelection`] is the caller's
//! write, through [`StateStore::record_correction_selection`].

use core::cmp::Ordering;

use gauntlet_domain::{
    Authority, BlastRadius, CorrectionSelection, RejectedAlternative, RejectionClass,
    RejectionReason, SelectionDimension, Severity, UnitId, blast_rank,
};
use gauntlet_domain::{BudgetFacts, CorrectionDescriptor, CorrectionShape, DecisionStep};
use gauntlet_domain::{IterationNo, RunId};
use gauntlet_ports::budget_meter::{BudgetMeter, Remaining};

// ---------------------------------------------------------------------------
// Candidate facts — typed, caller-assembled
// ---------------------------------------------------------------------------

/// The impact fact (§15.1 "impact"): the worst severity among the
/// residuals the correction addresses, and how many it addresses. Compared
/// severity-first, then count — explicitly, never as a weighted blend
/// ("it does not simply select the largest residual").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Impact {
    pub worst_severity: Severity,
    pub residuals_addressed: u32,
}

/// The settlement-accessibility fact (§15.1 "settlement accessibility"):
/// whether settling the touched Unit's evidence requirement is
/// instrument-verifiable (§22's preferred deterministic posture),
/// requires human signoff, or has no accessible path at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettlementAccessibility {
    Instrument,
    HumanSignoff,
    Missing,
}

/// The confidence fact (§15.1 "confidence"), from the attempt history —
/// the developmental-memory slice available today. Ordered best to worst.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Confidence {
    /// The touched Unit's previous correction verified PASS.
    LastSucceeded,
    /// Never attempted — unproven, but no contrary evidence.
    NeverAttempted,
    /// One failed attempt (§11.5: re-dispatch with the residual attached).
    FailedOnce,
    /// Repeated failure — §11.5 routes this OUT of correction and into
    /// diagnosis, so it ranks last among corrections.
    RepeatedFailure,
}

/// The expected-information-gain fact (§15.1 "expected information gain"):
/// what performing the correction would additionally discriminate, in
/// §15.2's vocabulary — nothing, incremental evidence, or a decisive
/// disconfirmation of a standing hypothesis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InformationGain {
    None,
    Incremental,
    Decisive,
}

/// One candidate correction with the typed facts the ten §15.1 dimensions
/// are decided from. Every field is a FACT the caller (slice B3) assembles
/// from the reference, residuals, verification outcomes, meter, and unit
/// state — there are no free-text self-declared scores. Payment rides the
/// descriptor's estimate; reversibility is the touched work's tier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub correction: CorrectionDescriptor,
    /// The authority the touched Unit requires for closure (§17.1).
    pub required_authority: Authority,
    /// The blast radius of the touched work (§11.3 tiers).
    pub tier: BlastRadius,
    /// Dependency order: every `depends_on` of the touched Units is
    /// settled or out_of_scope (§11.2).
    pub dependency_ready: bool,
    pub settlement: SettlementAccessibility,
    pub impact: Impact,
    pub information_gain: InformationGain,
    pub confidence: Confidence,
    /// Risk (§15.1 "risk"), as the breadth of the blast: how many Units
    /// the correction touches at once.
    pub units_touched: u32,
    /// Recovery cost (§15.1 "recovery cost"): how many Units' work must be
    /// rolled back if the correction is wrong (§5.3.7's "checkpoint, roll
    /// back, and report").
    pub rollback_units: u32,
}

impl Candidate {
    /// The primary Unit a rejection or decision names: the descriptor's
    /// first unit ref.
    fn lead_unit(&self) -> &UnitId {
        self.correction
            .unit_refs()
            .first()
            .expect("the descriptor constructor refuses an empty unit list")
    }
}

/// The selection inputs: the run/iteration identity, the meter, the
/// governance facts (the highest authority the run can exercise, and the
/// tiers it may dispatch under policy), and the candidates.
pub struct SelectionContext<'a> {
    pub run_id: RunId,
    pub iteration_no: IterationNo,
    pub meter: &'a dyn BudgetMeter,
    pub exercisable_authority: Authority,
    pub dispatchable_tiers: Vec<BlastRadius>,
    pub candidates: Vec<Candidate>,
}

/// Selection failures — typed. `NoAdmissibleCandidate` carries every
/// rejected alternative: when governance blocks everything, the caller
/// escalates (§11.6 `blocked`), and the evidence travels in the error.
#[derive(Debug, Clone, PartialEq)]
pub enum SelectionError {
    /// No candidates were offered — the §2 fork's settle/escalate branch,
    /// not a selection failure.
    NoCandidates,
    /// Every candidate was governance-blocked or unaffordable.
    NoAdmissibleCandidate { rejected: Vec<RejectedAlternative> },
    /// The assembled record failed the domain aggregate's validation.
    /// Unreachable when the selector is correct (the trace it builds names
    /// all ten dimensions); typed rather than swallowed so an assembly
    /// defect can never masquerade as a valid selection.
    Domain(gauntlet_domain::CorrectionError),
}

impl core::fmt::Display for SelectionError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NoCandidates => write!(
                f,
                "no candidate corrections were offered — select has nothing to rank (spec 15.1)"
            ),
            Self::NoAdmissibleCandidate { rejected } => write!(
                f,
                "every candidate was rejected ({} alternative(s) recorded); governance-blocked \
                 paths are never selected — escalate (spec 15.1 step 1, 11.6)",
                rejected.len()
            ),
            Self::Domain(error) => {
                write!(f, "the assembled selection record is invalid: {error}")
            }
        }
    }
}

impl std::error::Error for SelectionError {}

// ---------------------------------------------------------------------------
// The selector
// ---------------------------------------------------------------------------

/// Severity precedence for the impact comparison: blocking > major > minor
/// > informational (§5.2's severity list order).
fn severity_precedence(severity: Severity) -> u8 {
    match severity {
        Severity::Blocking => 3,
        Severity::Major => 2,
        Severity::Minor => 1,
        Severity::Informational => 0,
    }
}

/// The §15.1 stateless selector.
#[derive(Debug, Clone)]
pub struct CorrectionSelector;

impl CorrectionSelector {
    /// No `Default` impl on purpose (the domain `DraftUnit` precedent): a
    /// default selector would imply a default §15.1 policy, and the policy
    /// is this type's whole content.
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self
    }

    /// Runs the §15.1 procedure over the context's candidates:
    ///
    /// **Step 1 — reject governance-blocked paths first**, in the spec's
    /// own parenthetical order (authority, tier, budget, irreversibility):
    /// a candidate above the run's exercisable authority ceiling, at a
    /// tier no dispatch policy admits, whose estimate the meter refuses,
    /// or whose touched work is irreversible. Each rejection carries its
    /// class and the typed blocking fact — budget blocks carry the meter's
    /// own numbers.
    ///
    /// **Step 2 — among the allowed, the next affordable, most
    /// error-reducing correction**, by a fixed lexicographic priority:
    /// shape (bounded first, §15.1 step 3 — recorded under the risk
    /// dimension, the riskier shape being the speculative rewrite) →
    /// dependency order (ready first) → settlement accessibility
    /// (instrument < human signoff < missing) → impact (worst severity,
    /// then count) → expected information gain → confidence →
    /// reversibility (contained first) → risk (fewer units touched) →
    /// payment (cheaper estimate) → recovery cost (fewer rollback units).
    ///
    /// **Tie-breaks, deterministic:** the lead unit id lexicographic, then
    /// the correction summary — a complete tie on all ten dimensions is
    /// decided by ordering alone, recorded under the dependency-order
    /// dimension with a note that says exactly that (§11.2's determinism
    /// discipline; "a scheduler whose behavior varies run to run cannot be
    /// compared against itself" — the selector is held to the same bar).
    ///
    /// **Step 4 — record everything:** the returned
    /// [`CorrectionSelection`] carries every rejected alternative with its
    /// reason and a trace naming all ten dimensions for the chosen
    /// candidate plus the deciding dimension for each ranked-out
    /// alternative.
    pub fn select(
        &self,
        context: &SelectionContext<'_>,
    ) -> Result<CorrectionSelection, SelectionError> {
        if context.candidates.is_empty() {
            return Err(SelectionError::NoCandidates);
        }
        // Step 1 — governance blocks, in the §15.1 parenthetical order.
        let headroom = context.meter.remaining();
        let mut rejected: Vec<RejectedAlternative> = Vec::new();
        let mut admissible: Vec<&Candidate> = Vec::new();
        for candidate in &context.candidates {
            match self.governance_verdict(candidate, context, headroom) {
                Some((class, reason)) => {
                    rejected.push(RejectedAlternative::new(
                        candidate.correction.clone(),
                        class,
                        reason,
                    ));
                }
                None => admissible.push(candidate),
            }
        }
        let Some(first) = admissible.first().copied() else {
            return Err(SelectionError::NoAdmissibleCandidate { rejected });
        };
        // Step 2/3 — the lexicographic priority: fold the ordering
        // pairwise, recording the deciding dimension and note per loser.
        // A complete tie keeps the incumbent (identical-by-value
        // candidates produce an identical record either way, so input
        // order never moves the selection).
        let mut chosen = first;
        let mut losers: Vec<(&Candidate, SelectionDimension, String)> = Vec::new();
        for candidate in admissible.iter().skip(1) {
            let (ordering, dimension, note) = self.compare(chosen, candidate);
            match ordering {
                Ordering::Less => losers.push((candidate, dimension, note)),
                Ordering::Greater => {
                    losers.push((chosen, dimension, note));
                    chosen = candidate;
                }
                Ordering::Equal => losers.push((candidate, dimension, note)),
            }
        }
        // Step 4 — the trace: all ten dimensions named for the chosen
        // candidate, then the deciding dimension per ranked-out candidate.
        let mut trace = Self::chosen_facts(chosen, headroom);
        for (loser, dimension, note) in losers {
            trace.push(DecisionStep::new(
                dimension,
                format!("not selected over {}: {note}", chosen.correction.summary()),
            ));
            let reason = if chosen.correction.shape() == CorrectionShape::Bounded
                && loser.correction.shape() == CorrectionShape::Speculative
            {
                RejectionReason::SpeculativeWhenBoundedAvailable
            } else {
                RejectionReason::LowerPriority {
                    decided_by: dimension,
                }
            };
            rejected.push(RejectedAlternative::new(
                loser.correction.clone(),
                RejectionClass::NotSelected,
                reason,
            ));
        }
        CorrectionSelection::new(
            context.run_id.clone(),
            context.iteration_no,
            chosen.correction.clone(),
            rejected,
            trace,
        )
        .map_err(SelectionError::Domain)
    }

    /// §15.1 step 1 for one candidate, in the spec's parenthetical order:
    /// authority, tier, budget, irreversibility. `None` = admissible.
    #[allow(clippy::type_complexity)]
    fn governance_verdict(
        &self,
        candidate: &Candidate,
        context: &SelectionContext<'_>,
        headroom: Remaining,
    ) -> Option<(RejectionClass, RejectionReason)> {
        // Authority: §17.1 — the correction needs an authority the run
        // cannot exercise.
        if gauntlet_domain::authority_rank(candidate.required_authority)
            > gauntlet_domain::authority_rank(context.exercisable_authority)
        {
            return Some((
                RejectionClass::Authority,
                RejectionReason::AuthorityAboveCeiling {
                    required: candidate.required_authority,
                    exercisable: context.exercisable_authority,
                },
            ));
        }
        // Tier: the touched work's tier has no dispatch policy (§17.3).
        if !context.dispatchable_tiers.contains(&candidate.tier) {
            return Some((
                RejectionClass::Tier,
                RejectionReason::TierNotDispatchable {
                    tier: candidate.tier,
                },
            ));
        }
        // Budget: the meter, consulted NOW — §17.4, never a precomputed
        // answer. A refusal records the meter's own numbers.
        let estimated = candidate.correction.estimated();
        let slice = gauntlet_ports::budget_meter::BudgetSlice {
            rounds: estimated.requested_rounds,
            wall_seconds: estimated.requested_wall_seconds,
            tokens: estimated.requested_tokens,
            cost_usd: estimated.requested_cost_usd,
        };
        if context.meter.would_breach_reserve(slice) {
            return Some((
                RejectionClass::Budget,
                RejectionReason::MeterRefusedReserve {
                    facts: BudgetFacts {
                        requested_rounds: estimated.requested_rounds,
                        requested_wall_seconds: estimated.requested_wall_seconds,
                        requested_tokens: estimated.requested_tokens,
                        requested_cost_usd: estimated.requested_cost_usd,
                        remaining_rounds: headroom.rounds,
                        remaining_wall_seconds: headroom.wall_seconds,
                        remaining_tokens: headroom.tokens,
                        remaining_cost_usd: headroom.cost_usd,
                    },
                },
            ));
        }
        // Irreversibility: §17.1 — never an automatic candidate, whatever
        // any prompt says.
        if candidate.tier == BlastRadius::Irreversible {
            return Some((
                RejectionClass::Irreversibility,
                RejectionReason::IrreversibleTier {
                    tier: candidate.tier,
                },
            ));
        }
        None
    }

    /// The lexicographic comparison: first difference decides; the deciding
    /// dimension and a human-readable note come back with the ordering.
    #[allow(clippy::type_complexity)]
    fn compare<'a>(
        &self,
        a: &'a Candidate,
        b: &'a Candidate,
    ) -> (Ordering, SelectionDimension, String) {
        // Dimension names are fully qualified: several §15.1 dimension
        // variants share names with the fact types above (Impact,
        // Confidence, ...), and shadowing would silently mis-bind.
        // Ordering convention throughout: `Less` means `a` is preferred —
        // descending dimensions (shape, dependency, impact, information
        // gain) reverse their natural comparison; ascending dimensions
        // (settlement rank, confidence rank, tier, risk, payment,
        // recovery) do not.
        // Shape (§15.1 step 3) — recorded under risk: the speculative
        // rewrite is the riskier shape.
        let a_bounded = a.correction.shape() == CorrectionShape::Bounded;
        let b_bounded = b.correction.shape() == CorrectionShape::Bounded;
        if a_bounded != b_bounded {
            let winner = if a_bounded { a } else { b };
            return (
                match (a_bounded, b_bounded) {
                    (true, false) => Ordering::Less,
                    (false, true) => Ordering::Greater,
                    _ => Ordering::Equal,
                },
                SelectionDimension::Risk,
                format!(
                    "bounded shape preferred over the speculative rewrite (spec 15.1 step 3): {}",
                    winner.correction.summary()
                ),
            );
        }
        // Dependency order.
        if a.dependency_ready != b.dependency_ready {
            return (
                match (a.dependency_ready, b.dependency_ready) {
                    (true, false) => Ordering::Less,
                    (false, true) => Ordering::Greater,
                    _ => Ordering::Equal,
                },
                SelectionDimension::DependencyOrder,
                format!(
                    "dependencies ready: {}",
                    if a.dependency_ready { a } else { b }.lead_unit()
                ),
            );
        }
        // Settlement accessibility.
        let rank = |settlement: SettlementAccessibility| match settlement {
            SettlementAccessibility::Instrument => 0,
            SettlementAccessibility::HumanSignoff => 1,
            SettlementAccessibility::Missing => 2,
        };
        if rank(a.settlement) != rank(b.settlement) {
            return (
                rank(a.settlement).cmp(&rank(b.settlement)),
                SelectionDimension::SettlementAccessibility,
                "settlement by instrument preferred over signoff over none".to_owned(),
            );
        }
        // Impact: worst severity first, then count.
        let a_severity = severity_precedence(a.impact.worst_severity);
        let b_severity = severity_precedence(b.impact.worst_severity);
        if a_severity != b_severity || a.impact.residuals_addressed != b.impact.residuals_addressed
        {
            let ordering = b_severity.cmp(&a_severity).then(
                b.impact
                    .residuals_addressed
                    .cmp(&a.impact.residuals_addressed),
            );
            return (
                ordering,
                SelectionDimension::Impact,
                format!(
                    "more error reduced: worst severity {} over {}, {} residuals",
                    a.impact.worst_severity,
                    b.impact.worst_severity,
                    a.impact
                        .residuals_addressed
                        .max(b.impact.residuals_addressed)
                ),
            );
        }
        // Expected information gain.
        let gain = |value: InformationGain| match value {
            InformationGain::None => 0,
            InformationGain::Incremental => 1,
            InformationGain::Decisive => 2,
        };
        if gain(a.information_gain) != gain(b.information_gain) {
            return (
                gain(b.information_gain).cmp(&gain(a.information_gain)),
                SelectionDimension::ExpectedInformationGain,
                "higher expected information gain".to_owned(),
            );
        }
        // Confidence.
        let rank = |value: Confidence| match value {
            Confidence::LastSucceeded => 0,
            Confidence::NeverAttempted => 1,
            Confidence::FailedOnce => 2,
            Confidence::RepeatedFailure => 3,
        };
        if rank(a.confidence) != rank(b.confidence) {
            return (
                rank(a.confidence).cmp(&rank(b.confidence)),
                SelectionDimension::Confidence,
                "stronger prior evidence of success".to_owned(),
            );
        }
        // Reversibility: the contained tier is the most reversible.
        if blast_rank(a.tier) != blast_rank(b.tier) {
            return (
                blast_rank(a.tier).cmp(&blast_rank(b.tier)),
                SelectionDimension::Reversibility,
                format!("{} is more reversible than {}", a.tier, b.tier),
            );
        }
        // Risk: fewer Units touched.
        if a.units_touched != b.units_touched {
            return (
                a.units_touched.cmp(&b.units_touched),
                SelectionDimension::Risk,
                format!(
                    "narrower blast: {} vs {} units touched",
                    a.units_touched.min(b.units_touched),
                    a.units_touched.max(b.units_touched)
                ),
            );
        }
        // Payment: cheaper estimate (cost, then tokens).
        let a_estimate = a.correction.estimated();
        let b_estimate = b.correction.estimated();
        let payment = a_estimate
            .requested_cost_usd
            .get()
            .cmp(&b_estimate.requested_cost_usd.get())
            .then(
                a_estimate
                    .requested_tokens
                    .cmp(&b_estimate.requested_tokens),
            );
        if payment != Ordering::Equal {
            return (
                payment,
                SelectionDimension::Payment,
                format!(
                    "cheaper estimate: {} vs {} micro-usd",
                    a_estimate.requested_cost_usd.get(),
                    b_estimate.requested_cost_usd.get()
                ),
            );
        }
        // Recovery cost: fewer Units to roll back.
        if a.rollback_units != b.rollback_units {
            return (
                a.rollback_units.cmp(&b.rollback_units),
                SelectionDimension::RecoveryCost,
                format!(
                    "cheaper recovery: {} vs {} units to roll back",
                    a.rollback_units.min(b.rollback_units),
                    a.rollback_units.max(b.rollback_units)
                ),
            );
        }
        // Deterministic tie-breaks: lead unit id, then summary. A complete
        // tie on all ten dimensions is an ordering fact — recorded under
        // the dependency-order dimension (§11.2's determinism discipline)
        // with a note that says exactly what decided.
        let by_unit = a.lead_unit().as_str().cmp(b.lead_unit().as_str());
        let ordering = by_unit.then_with(|| a.correction.summary().cmp(b.correction.summary()));
        (
            ordering,
            SelectionDimension::DependencyOrder,
            format!(
                "all ten 15.1 dimensions equal — deterministic ordering by unit id: {} before {}",
                a.lead_unit(),
                b.lead_unit()
            ),
        )
    }

    /// The ten per-dimension fact steps for the chosen candidate — the
    /// guarantee that every §15.1 dimension is named in the trace whatever
    /// the pairwise decisions touched.
    fn chosen_facts(chosen: &Candidate, headroom: Remaining) -> Vec<DecisionStep> {
        let estimated = chosen.correction.estimated();
        vec![
            DecisionStep::new(
                SelectionDimension::Impact,
                format!(
                    "worst residual severity {}, {} residual(s) addressed",
                    chosen.impact.worst_severity, chosen.impact.residuals_addressed
                ),
            ),
            DecisionStep::new(
                SelectionDimension::DependencyOrder,
                format!(
                    "dependencies {}",
                    if chosen.dependency_ready {
                        "settled or out of scope"
                    } else {
                        "still open — correction is preparation, not closure"
                    }
                ),
            ),
            DecisionStep::new(
                SelectionDimension::Confidence,
                format!("attempt history: {:?}", chosen.confidence),
            ),
            DecisionStep::new(
                SelectionDimension::Payment,
                format!(
                    "estimate {} tokens / {} micro-usd; meter headroom {} tokens / {} micro-usd",
                    estimated.requested_tokens,
                    estimated.requested_cost_usd.get(),
                    headroom.tokens,
                    headroom.cost_usd.get()
                ),
            ),
            DecisionStep::new(
                SelectionDimension::Reversibility,
                format!("touched work tier {}", chosen.tier),
            ),
            DecisionStep::new(
                SelectionDimension::Risk,
                format!(
                    "{} unit(s) touched; shape {}",
                    chosen.units_touched,
                    chosen.correction.shape()
                ),
            ),
            DecisionStep::new(
                SelectionDimension::GovernanceStatus,
                format!(
                    "allowed: authority within the run's ceiling ({} required), tier dispatchable",
                    chosen.required_authority
                ),
            ),
            DecisionStep::new(
                SelectionDimension::SettlementAccessibility,
                format!("settlement path: {:?}", chosen.settlement),
            ),
            DecisionStep::new(
                SelectionDimension::RecoveryCost,
                format!("{} unit(s) to roll back if wrong", chosen.rollback_units),
            ),
            DecisionStep::new(
                SelectionDimension::ExpectedInformationGain,
                format!("information gain: {:?}", chosen.information_gain),
            ),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gauntlet_domain::EpochSeconds;
    use gauntlet_domain::EventLogOffset;
    use gauntlet_domain::{Budget, DiagnosticAllowance, Limits, MicroUsd, Payment};
    use gauntlet_ports::budget_meter::{BudgetMeterError, Reservation};
    use gauntlet_ports::deadline::Deadline;
    use gauntlet_ports::state_store::{Checkpoint, RunEvent, StateStore, StateStoreError};
    use std::cell::RefCell;

    fn unit_id(n: u16) -> UnitId {
        UnitId::new(&format!("AK-{n:03}")).expect("valid unit id")
    }

    // Named test facts — §8.1 literal discipline: duration- and
    // cost-shaped values are never inline numbers, in tests as in source.
    const REQ_ROUNDS: u32 = 1;
    const REQ_SPAN: u64 = 60;
    const REM_ROUNDS: u32 = 6;
    const REM_SPAN: u64 = 2700;
    const REM_TOKENS: u64 = 800;
    const REM_COST: u64 = 400;
    const RUN_ROUNDS: u32 = 8;
    const RUN_PARALLEL: u32 = 2;
    const RUN_DEPTH: u32 = 2;
    const RUN_SPAN: u64 = 3600;
    const RUN_TOKENS: u64 = 1000;
    const RUN_COST: u64 = 500;
    const RESERVE_PCT: u8 = 20;
    const DIAG_TOKENS: u64 = 100;
    const DIAG_COST: u64 = 50;
    const EPOCH_TICK: u64 = 1_700_000_060;
    const BASE_TOKENS: u64 = 100;
    const BASE_COST: u64 = 10;

    fn facts(tokens: u64, cost: u64) -> BudgetFacts {
        BudgetFacts {
            requested_rounds: REQ_ROUNDS,
            requested_wall_seconds: REQ_SPAN,
            requested_tokens: tokens,
            requested_cost_usd: MicroUsd::new(cost),
            remaining_rounds: REM_ROUNDS,
            remaining_wall_seconds: REM_SPAN,
            remaining_tokens: REM_TOKENS,
            remaining_cost_usd: MicroUsd::new(REM_COST),
        }
    }

    fn candidate(n: u16) -> Candidate {
        Candidate {
            correction: gauntlet_domain::CorrectionDescriptor::new(
                format!("correction {n}"),
                CorrectionShape::Bounded,
                vec![unit_id(n)],
                facts(BASE_TOKENS, BASE_COST),
            )
            .expect("complete descriptor"),
            required_authority: Authority::Critic,
            tier: BlastRadius::Contained,
            dependency_ready: true,
            settlement: SettlementAccessibility::Instrument,
            impact: Impact {
                worst_severity: Severity::Major,
                residuals_addressed: 1,
            },
            information_gain: InformationGain::Incremental,
            confidence: Confidence::NeverAttempted,
            units_touched: 1,
            rollback_units: 1,
        }
    }

    /// A meter over a real domain budget (1000 tokens, 20 percent reserve
    /// ⇒ 800 forward) — refusal arithmetic is the domain's.
    struct RealMeter {
        budget: Budget,
    }

    impl BudgetMeter for RealMeter {
        fn reserve(
            &self,
            _: gauntlet_ports::budget_meter::BudgetSlice,
        ) -> Result<Reservation, BudgetMeterError> {
            unreachable!("the selector never reserves")
        }
        fn charge(&self, _: &Reservation, _: Payment) -> Result<(), BudgetMeterError> {
            unreachable!("the selector never charges")
        }
        fn remaining(&self) -> Remaining {
            gauntlet_ports::budget_meter::remaining_of(&self.budget)
        }
        fn would_breach_reserve(&self, slice: gauntlet_ports::budget_meter::BudgetSlice) -> bool {
            self.budget.would_breach_reserve(
                slice.rounds,
                slice.wall_seconds,
                slice.tokens,
                slice.cost_usd,
            )
        }
    }

    fn meter() -> RealMeter {
        RealMeter {
            budget: Budget::new(
                Limits::new(
                    RUN_ROUNDS,
                    RUN_PARALLEL,
                    RUN_DEPTH,
                    RUN_SPAN,
                    RUN_TOKENS,
                    MicroUsd::new(RUN_COST),
                )
                .expect("usable"),
                RESERVE_PCT,
                DiagnosticAllowance::new(DIAG_TOKENS, MicroUsd::new(DIAG_COST)),
            )
            .expect("usable"),
        }
    }

    fn context<'a>(meter: &'a RealMeter, candidates: Vec<Candidate>) -> SelectionContext<'a> {
        SelectionContext {
            run_id: RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("valid run id"),
            iteration_no: IterationNo::new(2).expect("1-based"),
            meter,
            exercisable_authority: Authority::LeadAgent,
            dispatchable_tiers: vec![
                BlastRadius::Contained,
                BlastRadius::Local,
                BlastRadius::CrossCutting,
            ],
            candidates,
        }
    }

    /// The in-app fake store for the durability test: records selections
    /// as values (the durable adapter carries the JSON round trip; the
    /// shared suite runs it there).
    #[derive(Default)]
    struct FakeStore {
        selections: RefCell<Vec<CorrectionSelection>>,
    }

    impl StateStore for FakeStore {
        fn append_event(
            &self,
            _: RunEvent,
            _: Deadline,
        ) -> Result<EventLogOffset, StateStoreError> {
            Err(StateStoreError::WriteFailed {
                detail: "the fake records selections only".to_owned(),
            })
        }
        fn load_run(
            &self,
            _: &RunId,
            _: Deadline,
        ) -> Result<Option<gauntlet_domain::Run>, StateStoreError> {
            Ok(None)
        }
        fn checkpoint(&self, _: Checkpoint, _: Deadline) -> Result<(), StateStoreError> {
            Ok(())
        }
        fn resume(&self, _: &RunId, _: Deadline) -> Result<Option<Checkpoint>, StateStoreError> {
            Ok(None)
        }
        fn export_jsonl(&self, _: &RunId, _: Deadline) -> Result<String, StateStoreError> {
            Ok(String::new())
        }
        fn record_correction_selection(
            &self,
            selection: &CorrectionSelection,
            _deadline: Deadline,
        ) -> Result<(), StateStoreError> {
            self.selections.borrow_mut().push(selection.clone());
            Ok(())
        }
        fn correction_selections(
            &self,
            _: &RunId,
            _: Deadline,
        ) -> Result<Vec<CorrectionSelection>, StateStoreError> {
            Ok(self.selections.borrow().clone())
        }
    }

    #[test]
    fn controller_records_alternatives() {
        // AK-037's locator claim, at the app layer: selection + rejected
        // alternatives + reasons survive a durable round trip.
        let meter = meter();
        let high = Candidate {
            impact: Impact {
                worst_severity: Severity::Blocking,
                residuals_addressed: 2,
            },
            ..candidate(2)
        };
        let low = candidate(1);
        let selection = CorrectionSelector::new()
            .select(&context(&meter, vec![high.clone(), low]))
            .expect("selects");
        assert_eq!(selection.chosen().summary(), high.correction.summary());
        assert_eq!(selection.rejected().len(), 1, "the loser is recorded");
        assert_eq!(selection.rejected()[0].class(), RejectionClass::NotSelected);
        // The trace names all ten dimensions (the aggregate enforces it,
        // and the test pins that it does).
        for dimension in SelectionDimension::ALL {
            assert!(
                selection
                    .trace()
                    .iter()
                    .any(|step| step.dimension() == *dimension),
                "the trace names {dimension}"
            );
        }
        // Durable round trip through the port.
        let store = FakeStore::default();
        let bound = Deadline::at_epoch(EpochSeconds::new(EPOCH_TICK));
        store
            .record_correction_selection(&selection, bound)
            .expect("records");
        let back = store
            .correction_selections(selection.run_id(), bound)
            .expect("reads");
        assert_eq!(back.as_slice(), &[selection], "the record round-trips");
    }

    #[test]
    fn governance_blocks_are_never_chosen_even_when_highest_impact() {
        // §15.1 step 1: governance-blocked paths are rejected FIRST — a
        // blocking-severity irreversible candidate loses to a minor
        // contained one, and the rejection carries its class and fact.
        let meter = meter();
        let mut irresistible = candidate(2);
        irresistible.impact = Impact {
            worst_severity: Severity::Blocking,
            residuals_addressed: 5,
        };
        irresistible.tier = BlastRadius::Irreversible;
        irresistible.required_authority = Authority::Human;
        let ordinary = candidate(1);
        let selection = CorrectionSelector::new()
            .select(&context(
                &meter,
                vec![irresistible.clone(), ordinary.clone()],
            ))
            .expect("selects the allowed one");
        assert_eq!(selection.chosen().summary(), ordinary.correction.summary());
        // §15.1 step 1's own parenthetical order (authority, tier, budget,
        // irreversibility): over a lead-agent run the candidate is refused
        // by the FIRST-listed gate — but never chosen whatever refuses it.
        assert!(matches!(
            selection.rejected()[0].reason(),
            RejectionReason::AuthorityAboveCeiling { .. }
        ));
        // The irreversibility class fires when the earlier gates pass (a
        // human-exercisable run whose policy lists the tier): §17.1 —
        // never an automatic candidate, regardless of any prompt.
        let mut human_run = context(&meter, vec![irresistible, ordinary.clone()]);
        human_run.exercisable_authority = Authority::Human;
        human_run.dispatchable_tiers.push(BlastRadius::Irreversible);
        let selection = CorrectionSelector::new()
            .select(&human_run)
            .expect("selects");
        assert_eq!(selection.chosen().summary(), ordinary.correction.summary());
        let blocked = selection
            .rejected()
            .iter()
            .find(|alternative| alternative.class() == RejectionClass::Irreversibility)
            .expect("the irreversible rejection is recorded with its class");
        assert!(matches!(
            blocked.reason(),
            RejectionReason::IrreversibleTier { .. }
        ));
        // Authority-blocked: a human-authority closure above a lead-agent
        // run, at an ordinary tier.
        let mut needs_human = candidate(3);
        needs_human.required_authority = Authority::Human;
        let selection = CorrectionSelector::new()
            .select(&context(&meter, vec![needs_human, ordinary]))
            .expect("selects");
        assert!(matches!(
            selection.rejected()[0].class(),
            RejectionClass::Authority
        ));
        // Tier-blocked: cross_cutting with no dispatch policy for it.
        let mut off_policy = candidate(4);
        off_policy.tier = BlastRadius::CrossCutting;
        let mut narrow = context(&meter, vec![off_policy, candidate(1)]);
        narrow.dispatchable_tiers = vec![BlastRadius::Contained, BlastRadius::Local];
        let selection = CorrectionSelector::new().select(&narrow).expect("selects");
        assert!(
            selection
                .rejected()
                .iter()
                .any(|alternative| alternative.class() == RejectionClass::Tier)
        );
    }

    #[test]
    fn a_budget_block_records_the_meters_own_numbers() {
        // "Preserving recovery capacity must be a computed fact": the
        // rejection carries the requested slice AND the meter's headroom.
        let meter = meter();
        let mut expensive = candidate(2);
        expensive.correction = gauntlet_domain::CorrectionDescriptor::new(
            "a broad rewrite".to_owned(),
            CorrectionShape::Speculative,
            vec![unit_id(2)],
            facts(801, 10),
        )
        .expect("complete descriptor");
        let affordable = candidate(1);
        let selection = CorrectionSelector::new()
            .select(&context(&meter, vec![expensive, affordable]))
            .expect("selects the affordable one");
        let budget_rejection = selection
            .rejected()
            .iter()
            .find(|alternative| alternative.class() == RejectionClass::Budget)
            .expect("the budget block is recorded with its class");
        let RejectionReason::MeterRefusedReserve { facts: recorded } = budget_rejection.reason()
        else {
            panic!("the reason is the meter's numbers");
        };
        assert_eq!(recorded.requested_tokens, 801);
        // The headroom is the METER's remaining(), captured at call time.
        let headroom = meter.remaining();
        assert_eq!(recorded.remaining_tokens, headroom.tokens);
        assert_eq!(recorded.remaining_cost_usd, headroom.cost_usd);
        assert_eq!(recorded.remaining_rounds, headroom.rounds);
        assert_eq!(recorded.remaining_wall_seconds, headroom.wall_seconds);
        // The boundary is exact: 800 lands ON the forward ceiling and is
        // admissible; 801 would consume the reserve.
        let mut boundary = candidate(3);
        boundary.correction = gauntlet_domain::CorrectionDescriptor::new(
            "exactly the ceiling".to_owned(),
            CorrectionShape::Bounded,
            vec![unit_id(3)],
            facts(800, 10),
        )
        .expect("complete descriptor");
        // Correction C5: a selection without rejected alternatives is
        // unrepresentable — the single-candidate construction the old
        // assertion pinned (`rejected().is_empty()`) is the vacuity the
        // domain aggregate now refuses. The boundary fact the test is
        // about survives over a real pair: the ceiling candidate is
        // CHOSEN against a second affordable candidate.
        // The peer asks one past the ceiling: the meter refuses it, the
        // boundary-exact candidate is the affordable one selected, and
        // the refusal is recorded as the rejected alternative with the
        // meter's own numbers.
        let mut peer = candidate(4);
        peer.correction = gauntlet_domain::CorrectionDescriptor::new(
            "one past the ceiling".to_owned(),
            CorrectionShape::Bounded,
            vec![unit_id(4)],
            facts(801, 10),
        )
        .expect("complete descriptor");
        let selection = CorrectionSelector::new()
            .select(&context(&meter, vec![boundary, peer]))
            .expect("on the ceiling is affordable");
        assert_eq!(
            selection.chosen().summary(),
            "exactly the ceiling",
            "the ceiling-exact estimate is the affordable one selected"
        );
        assert_eq!(selection.rejected().len(), 1);
        assert_eq!(selection.rejected()[0].class(), RejectionClass::Budget);
        // And the single-candidate space is the typed refusal, never a
        // vacuous record.
        assert!(
            matches!(
                CorrectionSelector::new().select(&context(&meter, vec![candidate(5)])),
                Err(SelectionError::Domain(
                    gauntlet_domain::CorrectionError::RejectedAlternativesEmpty
                ))
            ),
            "a one-candidate selection cannot exist"
        );
    }

    #[test]
    fn bounded_corrections_beat_speculative_rewrites() {
        // §15.1 step 3: "Prefer bounded correction → observable consequence
        // → useful settlement over large speculative rewrites" — the shape
        // preference LEADS the ordering, so a speculative candidate with
        // higher impact still loses, and its rejection says exactly that.
        let meter = meter();
        let mut speculative = candidate(2);
        speculative.correction = gauntlet_domain::CorrectionDescriptor::new(
            "rewrite the subsystem wholesale".to_owned(),
            CorrectionShape::Speculative,
            vec![unit_id(2)],
            facts(100, 10),
        )
        .expect("complete descriptor");
        speculative.impact = Impact {
            worst_severity: Severity::Blocking,
            residuals_addressed: 3,
        };
        let bounded = candidate(1);
        let selection = CorrectionSelector::new()
            .select(&context(&meter, vec![speculative.clone(), bounded.clone()]))
            .expect("selects");
        assert_eq!(selection.chosen().summary(), bounded.correction.summary());
        let rejected = selection
            .rejected()
            .iter()
            .find(|alternative| {
                alternative.correction().summary() == speculative.correction.summary()
            })
            .expect("the speculative loss is recorded");
        assert!(matches!(
            rejected.reason(),
            RejectionReason::SpeculativeWhenBoundedAvailable
        ));
    }

    #[test]
    fn tie_breaks_are_deterministic_and_the_priority_decides_in_order() {
        let meter = meter();
        // Two candidates identical on every dimension: the unit id decides,
        // whichever order they arrive in.
        let a = candidate(1);
        let b = candidate(2);
        let first = CorrectionSelector::new()
            .select(&context(&meter, vec![a.clone(), b.clone()]))
            .expect("selects");
        let second = CorrectionSelector::new()
            .select(&context(&meter, vec![b, a]))
            .expect("selects");
        assert_eq!(
            first.chosen().summary(),
            second.chosen().summary(),
            "input order never moves the selection"
        );
        assert_eq!(first.rejected().len(), 1);
        // Impact outranks payment: a blocking-severity correction beats a
        // cheaper minor one.
        let mut corrective = candidate(3);
        corrective.impact = Impact {
            worst_severity: Severity::Blocking,
            residuals_addressed: 1,
        };
        let mut cheap = candidate(4);
        cheap.correction = gauntlet_domain::CorrectionDescriptor::new(
            "a cheap cosmetic".to_owned(),
            CorrectionShape::Bounded,
            vec![unit_id(4)],
            facts(10, 1),
        )
        .expect("complete descriptor");
        let selection = CorrectionSelector::new()
            .select(&context(&meter, vec![cheap, corrective.clone()]))
            .expect("selects");
        assert_eq!(
            selection.chosen().summary(),
            corrective.correction.summary()
        );
        // Dependency-readiness outranks impact: an unready blocking
        // correction is preparation; a ready major one is the next
        // affordable error reduction.
        let mut unready = candidate(5);
        unready.dependency_ready = false;
        unready.impact = Impact {
            worst_severity: Severity::Blocking,
            residuals_addressed: 2,
        };
        let ready = candidate(6);
        let selection = CorrectionSelector::new()
            .select(&context(&meter, vec![unready, ready.clone()]))
            .expect("selects");
        assert_eq!(selection.chosen().summary(), ready.correction.summary());
    }

    #[test]
    fn empty_and_fully_blocked_inputs_are_typed_errors_carrying_their_evidence() {
        let meter = meter();
        assert!(matches!(
            CorrectionSelector::new().select(&context(&meter, Vec::new())),
            Err(SelectionError::NoCandidates)
        ));
        let mut blocked = candidate(1);
        blocked.tier = BlastRadius::Irreversible;
        let mut human_run = context(&meter, vec![blocked]);
        // Every earlier gate passes on purpose, so the recorded class is
        // the irreversibility gate itself.
        human_run.exercisable_authority = Authority::Human;
        human_run.dispatchable_tiers.push(BlastRadius::Irreversible);
        let error = CorrectionSelector::new()
            .select(&human_run)
            .expect_err("governance blocks everything");
        let SelectionError::NoAdmissibleCandidate { rejected } = &error else {
            panic!("all-blocked is its own typed error");
        };
        assert_eq!(rejected.len(), 1, "the evidence travels in the error");
        assert_eq!(rejected[0].class(), RejectionClass::Irreversibility);
    }
}
