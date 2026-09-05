//! The §19.4 self-improvement gate (plan Task 23, charter D4's `gate`
//! piece) — ALL EIGHT conditions, typed per-condition refusals:
//!
//! 1. **Governance preserved** — the treatment arm's dispatched-unit set
//!    over the held-out set contains every unit the control arm dispatched
//!    (a SUPERSET passes; a skipped control unit is cherry-picking —
//!    durable facts; tier and authority are per-unit properties frozen in
//!    the reference both arms share, so unit-set containment preserves the
//!    tier/authority profile), and the declared target is not classified
//!    governance-bearing. The implemented direction is the conservative
//!    one: cherry-picking is caught; extra treatment dispatch is not an
//!    improvement vector (COR-1, critic m2).
//! 2. **Reference integrity preserved** — ALL FOUR evaluation arms (the
//!    held-out and regression control/treatment runs) carry the CURRENT
//!    frozen reference digest (never a moved ruler; the regression set is
//!    an operator-named project whose own frozen reference is checked, never
//!    assumed — COR-2, R1). The harness refuses a moved-ruler arm at
//!    evaluate time too, before anything is recorded.
//! 3. **Settlement access preserved** — the candidate's configuration
//!    delta is classified by the typed TOTAL function over the config key
//!    vocabulary ([`crate::promote::delta`]); any delta that removes,
//!    disables, or downgrades a verification-bearing key is
//!    `SettlementAccessWeakened` — **AK-056's refusal**. Unknown keys
//!    classify as protected: fail-closed, no default-allow. Additionally
//!    the treatment settlements' instrument series must be non-weaker
//!    than control's, the arms JOINED BY UNIT (never by list position —
//!    an extra settlement cannot shield a weakened unit; COR-2, R2).
//! 4. **Rollback valid** — the proposal declares a rollback ref, and it
//!    resolves where the evidence store is at hand (the caller supplies
//!    the resolution; `None` records "store not at hand" and decides on
//!    presence alone).
//! 5. **Regression suite passes** — on the regression task set, every unit
//!    control settled, treatment settles too, with non-weaker instruments.
//! 6. **Protected capabilities non-inferior** — every declared protected
//!    capability reads non-inferior in the treatment arm vs control (its
//!    settlement memories from the treatment runs at least match the
//!    control runs', in the shared §19.1 ledger the evaluation imports
//!    into). An UNDECLARED set is an evidence gap, never a pass.
//! 7. **Target metric improves by the declared margin** — the target
//!    metric is from the typed vocabulary
//!    ([`crate::promote::metrics::TargetMetric`], lower-is-better), and
//!    treatment's figure must be strictly better than control's AND cover
//!    the configured margin in basis points relative to control. Margin 0
//!    keeps the floor at strict improvement. An arm whose figure is
//!    UNDERIVABLE (it did not reach a settled terminal, or settled
//!    nothing) is an evidence gap — never a failed condition, never a
//!    placeholder figure (COR-1).
//! 8. **Payment remains acceptable** — treatment payment per settlement
//!    ≤ control's (stable-or-decreasing, the §19.2 shape), with the same
//!    underivable-figure rule as condition 7.
//!
//! Verdicts: all eight hold → **Promoted** (the write-once decision via
//! the ledger's decision state machine + the durable eight-condition
//! evidence record); any condition FAILED → **Rejected** (EVERY failed
//! condition's reason recorded + the rollback ref); evidence MISSING or
//! undecidable → **Provisional** — NEVER promoted, and never rejected.
//! Every condition is evaluated: a FAILED condition never hides behind an
//! evidence gap elsewhere, and an evidence gap never upgrades to a
//! failure.
//!
//! The re-ask floor (task24 i1; spec §23.2 G16): the adjudication covers
//! EVERY evaluation record's FAILED conditions, not only the newest
//! record's. A failing evaluation followed by a passing re-run
//! adjudicates REJECTED — a re-run supersedes an earlier record's
//! evidence GAPS (the honest road stays open) and never launders its
//! failures ([`earlier_record_failures`]).

use std::sync::Arc;

use gauntlet_domain::{CapabilityId, Digest, EvidenceRef, RevisionId, RunId};
use gauntlet_ports::capability_ledger::{CapabilityLedger, ObservationClass};
use gauntlet_ports::clock::Clock;
use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::revision_ledger::{
    DecisionKind, EvaluationRecord, GateDecision, GateRefusal, RevisionDecisionRecord,
    RevisionEntry, RevisionLedger, RevisionLedgerError, RevisionPromotionEvidence, RunMetrics,
};

use crate::promote::delta::DeltaDocument;
use crate::promote::metrics::TargetMetric;

/// The §19.4 gate.
#[derive(Clone)]
pub struct PromotionGate {
    ledger: Arc<dyn RevisionLedger>,
    capability_ledger: Arc<dyn CapabilityLedger>,
    clock: Arc<dyn Clock>,
    margin_bps: u64,
}

/// The adjudication's input beyond the ledger: the CURRENT frozen
/// reference digest (condition 2), the declared target metric (condition
/// 7), the parsed candidate delta (condition 3), the protected
/// capabilities (condition 6), and the rollback resolution (condition 4 —
/// `None` when the evidence store is not at hand).
#[derive(Debug, Clone)]
pub struct GateInput {
    pub current_reference_digest: Digest,
    pub delta: DeltaDocument,
    /// Whether the proposal's rollback ref resolves in the evidence store;
    /// `None` when the store is not at hand (the condition then decides on
    /// presence alone, as documented).
    pub rollback_resolvable: Option<bool>,
}

/// The gate's verdict. `Promoted` carries the eight-condition evidence;
/// `Rejected` carries EVERY failed condition's reason plus the rollback
/// ref; `Provisional` carries the evidence gaps' reasons — `written` is
/// false only for the ZERO-evidence shape (the revision stays PENDING: the
/// domain floor refuses a provisional label with no evaluation runs at
/// all, so the verdict is returned to the operator and nothing is durably
/// written).
#[derive(Debug, Clone, PartialEq)]
pub enum GateVerdict {
    Promoted(RevisionPromotionEvidence),
    Rejected {
        reasons: Vec<GateRefusal>,
        rollback_ref: EvidenceRef,
    },
    Provisional {
        reasons: Vec<GateRefusal>,
        written: bool,
    },
}

/// Why the gate refused to adjudicate at all — distinct from the verdicts.
#[derive(Debug, Clone, PartialEq)]
pub enum GateError {
    /// The revision has no admitted proposal.
    UnknownRevision { revision: RevisionId },
    /// The revision's decision is write-once past a final state.
    AlreadyFinal {
        revision: RevisionId,
        current: DecisionKind,
    },
    /// The ledger refused a read or write.
    Ledger(RevisionLedgerError),
    /// A §19.1 capability-ledger read failed (COR-1, critic M4): an
    /// infrastructure failure is NOT evidence of an inferior capability —
    /// the gate aborts typed (retryable), writes nothing, and the
    /// revision is NOT rejected.
    CapabilityLedger(gauntlet_ports::capability_ledger::CapabilityLedgerError),
}

impl core::fmt::Display for GateError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UnknownRevision { revision } => write!(
                f,
                "revision {revision} has no admitted proposal; nothing to adjudicate"
            ),
            Self::AlreadyFinal { revision, current } => write!(
                f,
                "revision {revision} is durably {current}; the decision is write-once past a \
                 final state and cannot be re-adjudicated"
            ),
            Self::Ledger(error) => write!(f, "the gate's ledger access failed: {error}"),
            Self::CapabilityLedger(error) => write!(
                f,
                "the gate's capability-ledger access failed: {error} — an infrastructure \
                 failure is not evidence of inferiority; the adjudication aborted and may be \
                 retried"
            ),
        }
    }
}

impl std::error::Error for GateError {}

/// Why the gate refused to compose — the declared bar enters the gate
/// here, and a bar that cannot be an improvement requirement is refused
/// where it enters, never silently reinterpreted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GateConfigError {
    /// A margin above 10 000 basis points is above 100%: mathematically
    /// unsatisfiable for positive figures, so the former silent saturation
    /// clamp enforced a declared 150% margin as EXACTLY 100% (the
    /// falsifier's probe B promoted a 100% improvement through a declared
    /// 150% margin). Refused at the constructor AND at config resolution
    /// (COR-2, R3).
    MarginOutOfRange { margin_bps: u64 },
}

impl core::fmt::Display for GateConfigError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::MarginOutOfRange { margin_bps } => write!(
                f,
                "the declared promotion margin {margin_bps}bps is above the 10 000 bps \
                 ceiling — a required improvement above 100% cannot be satisfied by any \
                 positive figure, so it is refused, never clamped (spec 19.4, condition 7)"
            ),
        }
    }
}

impl std::error::Error for GateConfigError {}

impl PromotionGate {
    pub fn new(
        ledger: Arc<dyn RevisionLedger>,
        capability_ledger: Arc<dyn CapabilityLedger>,
        clock: Arc<dyn Clock>,
        margin_bps: u64,
    ) -> Result<Self, GateConfigError> {
        // COR-2 (R3): the declared bar is refused where it enters. A
        // margin above 10 000 bps is above 100%; the former silent
        // `.min(10_000)` clamp turned a declared 150% requirement into a
        // 100% one and promoted on it.
        if margin_bps > 10_000 {
            return Err(GateConfigError::MarginOutOfRange { margin_bps });
        }
        Ok(Self {
            ledger,
            capability_ledger,
            clock,
            margin_bps,
        })
    }

    /// The configured margin this gate enforces (basis points relative to
    /// control).
    pub fn margin_bps(&self) -> u64 {
        self.margin_bps
    }

    /// Adjudicates one revision against the FULL §19.4 conjunction and
    /// writes the durable decision.
    pub fn adjudicate(
        &self,
        revision: &RevisionId,
        input: &GateInput,
        deadline: Deadline,
    ) -> Result<GateVerdict, GateError> {
        let proposal = self
            .ledger
            .proposal(revision, deadline)
            .map_err(GateError::Ledger)?
            .ok_or_else(|| GateError::UnknownRevision {
                revision: revision.clone(),
            })?;
        let entry = self
            .ledger
            .revision(revision, deadline)
            .map_err(GateError::Ledger)?
            .ok_or_else(|| GateError::UnknownRevision {
                revision: revision.clone(),
            })?;
        if let Some(current @ (DecisionKind::Promoted | DecisionKind::Rejected)) = entry.decision {
            return Err(GateError::AlreadyFinal {
                revision: revision.clone(),
                current,
            });
        }
        let evaluations = self
            .ledger
            .evaluations(revision, deadline)
            .map_err(GateError::Ledger)?;
        // The ZERO-evidence shape: the §19.4 floor adjudicates Provisional
        // and NEVER promotes — and the domain floor refuses a provisional
        // LABEL with no evaluation runs, so the revision stays PENDING
        // with the verdict returned to the operator, nothing written.
        let Some(evaluation) = evaluations.last().cloned() else {
            return Ok(GateVerdict::Provisional {
                reasons: vec![GateRefusal::MissingEvaluation],
                written: false,
            });
        };

        // Evaluate ALL conditions; separate FAILED conditions from
        // evidence gaps. Order is the conditions' own, so the recorded
        // reasons read in the spec's order.
        let mut failed: Vec<GateRefusal> = Vec::new();
        let mut gaps: Vec<GateRefusal> = Vec::new();
        let mut series_non_weaker = true;

        // The re-ask floor (task24 i1; spec §23.2 G16 "Re-ask a model
        // until it returns `pass`"): EVERY evaluation record's FAILED
        // conditions are durable contrary evidence. A later, cleaner
        // re-run supersedes an earlier record's evidence GAPS (the honest
        // road — an incomplete arm, re-evaluated — stays open below), but
        // it can never LAUNDER a failure: a failing evaluation followed
        // by a passing one adjudicates REJECTED, never promoted. The
        // gate used to read `evaluations.last()` alone, which adjudicated
        // exactly the re-rolled record.
        for earlier in evaluations.iter().take(evaluations.len() - 1) {
            failed.extend(earlier_record_failures(
                earlier,
                proposal.target(),
                self.margin_bps,
            ));
        }

        // Condition 1 — governance preserved.
        if let Err(refusal) = governance_verdict(
            &evaluation.control_held_out_metrics,
            &evaluation.treatment_held_out_metrics,
            proposal.target(),
        ) {
            failed.push(refusal);
        }
        // Condition 2 — reference integrity preserved over ALL FOUR arms
        // (COR-2, R1): the record carries both held-out AND both
        // regression arms' frozen reference digests; ANY arm whose digest
        // differs from the current frozen reference is a moved ruler,
        // refused with one typed `ReferenceMoved` naming the offending
        // arm(s). The regression set is an operator-named project with its
        // own frozen reference — its ruler is checked, never assumed.
        let arm_digests = [
            ("held-out control", &evaluation.control_reference_digest),
            ("held-out treatment", &evaluation.treatment_reference_digest),
            (
                "regression control",
                &evaluation.regression_control_reference_digest,
            ),
            (
                "regression treatment",
                &evaluation.regression_treatment_reference_digest,
            ),
        ];
        let moved_arms: Vec<&str> = arm_digests
            .iter()
            .filter(|(_, digest)| **digest != input.current_reference_digest)
            .map(|(arm, _)| *arm)
            .collect();
        if !moved_arms.is_empty() {
            failed.push(GateRefusal::ReferenceMoved {
                detail: format!(
                    "the offending arm(s) [{}] carry a frozen reference other than the \
                     current one ({}) — the ruler has moved, or the arms disagree; a moved \
                     ruler never adjudicates",
                    moved_arms.join(", "),
                    input.current_reference_digest
                ),
            });
        }
        // Condition 3 — settlement access preserved: the delta
        // classification first (AK-056's refusal), then the instrument
        // series.
        if let Err(refusal) = input.delta.settlement_access_verdict() {
            failed.push(refusal);
        }
        match instrument_series_verdict(
            &evaluation.control_held_out_metrics,
            &evaluation.treatment_held_out_metrics,
        ) {
            SeriesVerdict::NonWeaker => {}
            SeriesVerdict::Weakened => {
                series_non_weaker = false;
                failed.push(GateRefusal::SettlementAccessWeakened {
                    detail: "the treatment settlements' instrument series is weaker than \
                             control's — verification got weaker exactly where the candidate \
                             claims improvement"
                        .to_owned(),
                });
            }
            SeriesVerdict::Undecidable { detail } => {
                gaps.push(GateRefusal::EvaluationIncomplete { detail });
            }
        }
        // Condition 4 — rollback valid.
        if proposal.rollback_ref().as_str().is_empty() {
            failed.push(GateRefusal::RollbackInvalid {
                detail: "the proposal declares no rollback".to_owned(),
            });
        } else if input.rollback_resolvable == Some(false) {
            failed.push(GateRefusal::RollbackInvalid {
                detail: format!(
                    "the declared rollback {} does not resolve in the evidence store",
                    proposal.rollback_ref()
                ),
            });
        }
        // Condition 5 — the regression suite passes.
        if let Err(refusal) = regression_verdict(
            &evaluation.control_regression_metrics,
            &evaluation.treatment_regression_metrics,
        ) {
            failed.push(refusal);
        }
        // Condition 6 — protected capabilities non-inferior, over the set
        // the EVALUATION declared (recorded at evaluation time, so the
        // gate adjudicates exactly what the operator named when the arms
        // ran).
        if evaluation.protected.is_empty() {
            gaps.push(GateRefusal::ProtectedCapabilityInferior {
                capability: "(none declared)".to_owned(),
                detail: "no protected capability set was declared at evaluation — the \
                         non-inferiority condition is undecidable, and an undecidable \
                         condition refuses"
                    .to_owned(),
            });
        } else {
            for token in &evaluation.protected {
                let capability = CapabilityId::new(token);
                let Ok(capability) = capability else {
                    gaps.push(GateRefusal::ProtectedCapabilityInferior {
                        capability: token.clone(),
                        detail: "the declared protected capability is outside the id grammar"
                            .to_owned(),
                    });
                    continue;
                };
                match self.protected_verdict(&capability, &evaluation, deadline) {
                    Ok(GateOutcome::Holds) => {}
                    Ok(GateOutcome::Refused(refusal)) => failed.push(refusal),
                    // An infrastructure failure aborts the WHOLE
                    // adjudication — never a verdict, nothing written.
                    Err(error) => return Err(error),
                }
            }
        }
        // Condition 7 — the target metric improves by the declared
        // margin. The metric is the one the EVALUATION declared (the
        // operator named it at evaluation time; the gate judges exactly
        // that), parsed through the typed vocabulary. An arm whose figure
        // is underivable (it did not reach a settled terminal, or settled
        // nothing) is an EVIDENCE GAP — the condition is undecidable, and
        // an undecidable condition never composes a failed one from
        // placeholder figures (COR-1, critic B1).
        let declared_metric = TargetMetric::parse(&evaluation.metric)
            .map_err(|_| underivable("the evaluation declared an unknown metric token"))?;
        let control_figure_opt = declared_metric.figure(&evaluation.control_held_out_metrics);
        let treatment_figure_opt = declared_metric.figure(&evaluation.treatment_held_out_metrics);
        let (control_figure, treatment_figure) = match (control_figure_opt, treatment_figure_opt) {
            (Some(control), Some(treatment)) => {
                if !margin_holds(control, treatment, self.margin_bps) {
                    failed.push(GateRefusal::MarginNotMet {
                        metric: declared_metric.token().to_owned(),
                        control_figure: control,
                        treatment_figure: treatment,
                        margin_bps: self.margin_bps,
                    });
                }
                (control, treatment)
            }
            (control, treatment) => {
                gaps.push(GateRefusal::EvaluationIncomplete {
                    detail: format!(
                        "the {} figure is underivable for {} — an arm that did not reach \
                             a settled terminal (or settled nothing) has no comparable figure; \
                             an undecidable margin is an evidence gap, never a failure",
                        declared_metric.token(),
                        underivable_arms(control.is_some(), treatment.is_some())
                    ),
                });
                (0, 0)
            }
        };
        // Condition 8 — payment remains acceptable. The same rule: an
        // underivable per-settlement payment figure is an evidence gap,
        // never a FAILED condition composed from placeholders.
        let control_payment_opt =
            TargetMetric::PaymentPerSettlement.figure(&evaluation.control_held_out_metrics);
        let treatment_payment_opt =
            TargetMetric::PaymentPerSettlement.figure(&evaluation.treatment_held_out_metrics);
        let (control_payment, treatment_payment) =
            match (control_payment_opt, treatment_payment_opt) {
                (Some(control), Some(treatment)) => {
                    if treatment > control {
                        failed.push(GateRefusal::PaymentIncreased {
                            control_figure: control,
                            treatment_figure: treatment,
                        });
                    }
                    (control, treatment)
                }
                (control, treatment) => {
                    gaps.push(GateRefusal::EvaluationIncomplete {
                        detail: format!(
                            "the payment-per-settlement figure is underivable for {} — an arm \
                             that did not reach a settled terminal (or settled nothing) has no \
                             comparable figure; an undecidable payment condition is an \
                             evidence gap, never a failure",
                            underivable_arms(control.is_some(), treatment.is_some())
                        ),
                    });
                    (0, 0)
                }
            };

        let rollback_ref = proposal.rollback_ref().clone();
        if !failed.is_empty() {
            // REJECTED: every failed condition's reason recorded, with the
            // rollback ref the domain floor requires.
            let decision = GateDecision::rejected(failed.clone(), rollback_ref.clone())
                .map_err(|_| underivable("the gate composed an empty rejection"))?;
            self.write_decision(&entry, revision, decision, deadline)?;
            return Ok(GateVerdict::Rejected {
                reasons: failed,
                rollback_ref,
            });
        }
        if !gaps.is_empty() {
            // PROVISIONAL: insufficient evidence is never a negative
            // result, and never a promotion.
            let decision = GateDecision::provisional(gaps.clone())
                .map_err(|_| underivable("the gate composed an empty provisional"))?;
            self.write_decision(&entry, revision, decision, deadline)?;
            return Ok(GateVerdict::Provisional {
                reasons: gaps,
                written: true,
            });
        }

        // ALL EIGHT hold: the durable decision record carries the
        // eight-condition evidence.
        let evidence = RevisionPromotionEvidence {
            metric: declared_metric.token().to_owned(),
            control_figure,
            treatment_figure,
            margin_bps: self.margin_bps,
            control_payment_figure: control_payment,
            treatment_payment_figure: treatment_payment,
            unit_mix_preserved: evaluation
                .control_held_out_metrics
                .dispatched_units
                .iter()
                .all(|unit| {
                    evaluation
                        .treatment_held_out_metrics
                        .dispatched_units
                        .contains(unit)
                }),
            instrument_series_non_weaker: series_non_weaker,
            control_reference_digest: evaluation.control_reference_digest.to_string(),
            treatment_reference_digest: evaluation.treatment_reference_digest.to_string(),
            regression_units_control_settled: evaluation
                .control_regression_metrics
                .settlements
                .len(),
            regression_units_treatment_settled: evaluation
                .treatment_regression_metrics
                .settlements
                .len(),
            protected_capabilities: evaluation.protected.clone(),
        };
        let decision = GateDecision::promoted(evidence.clone());
        self.write_decision(&entry, revision, decision, deadline)?;
        Ok(GateVerdict::Promoted(evidence))
    }

    /// Composes and applies the decision record: the claim is the DERIVED
    /// current state; the applying ledger validates the claim against the
    /// same snapshot the record lands in.
    fn write_decision(
        &self,
        entry: &RevisionEntry,
        revision: &RevisionId,
        decision: GateDecision,
        deadline: Deadline,
    ) -> Result<(), GateError> {
        let claimed = entry.decision.unwrap_or(DecisionKind::Pending);
        let record =
            RevisionDecisionRecord::new(revision.clone(), claimed, decision, self.clock.now())
                .map_err(|_| GateError::AlreadyFinal {
                    revision: revision.clone(),
                    current: claimed,
                })?;
        self.ledger
            .apply_decision(record, deadline)
            .map_err(GateError::Ledger)?;
        Ok(())
    }

    /// Condition 6 over ONE protected capability: the capability's
    /// settlement memories rooted in the treatment runs must at least
    /// match the control runs', in the shared §19.1 ledger the evaluation
    /// imported into. A capability with NO memories from either arm is an
    /// evidence gap (undecidable refuses), never a pass. The OUTER result
    /// is the abort channel (an infrastructure failure — COR-1); the
    /// INNER one is the condition's own refusal.
    fn protected_verdict(
        &self,
        capability: &CapabilityId,
        evaluation: &EvaluationRecord,
        deadline: Deadline,
    ) -> Result<GateOutcome, GateError> {
        let observations = self
            .capability_ledger
            .observations(capability, deadline)
            // COR-1 (critic M4): an infrastructure READ FAILURE is not
            // evidence of inferiority — it aborts the adjudication typed
            // (retryable, nothing written), it never becomes a FAILED
            // condition and never a durable Rejection.
            .map_err(GateError::CapabilityLedger)?;
        let settlement_count_for = |run: &RunId| {
            observations
                .iter()
                .filter(|observation| {
                    observation.identity().run() == run
                        && observation.identity().class() == ObservationClass::Settlement
                })
                .count()
        };
        let control = settlement_count_for(&evaluation.held_out_control)
            .max(settlement_count_for(&evaluation.regression_control));
        let treatment = settlement_count_for(&evaluation.held_out_treatment)
            .max(settlement_count_for(&evaluation.regression_treatment));
        if control == 0 && treatment == 0 {
            return Ok(GateOutcome::Refused(
                GateRefusal::ProtectedCapabilityInferior {
                    capability: capability.as_str().to_owned(),
                    detail: "the capability carries no settlement memories from either arm — \
                             non-inferiority is undecidable and refuses"
                        .to_owned(),
                },
            ));
        }
        if treatment < control {
            return Ok(GateOutcome::Refused(
                GateRefusal::ProtectedCapabilityInferior {
                    capability: capability.as_str().to_owned(),
                    detail: format!(
                        "{control} control settlements vs {treatment} treatment settlements"
                    ),
                },
            ));
        }
        Ok(GateOutcome::Holds)
    }
}

/// The typed corruption shape for a record the gate itself can only ever
/// compose with reasons — a defense-in-depth locus, never a production
/// path.
fn underivable(detail: &str) -> GateError {
    GateError::Ledger(RevisionLedgerError::Corrupt {
        detail: detail.to_owned(),
    })
}

/// One condition's outcome, kept distinct from an abort: the condition
/// either HOLDS or REFUSES with its typed reason — an infrastructure
/// failure is neither, and travels as a `GateError`.
enum GateOutcome {
    Holds,
    Refused(GateRefusal),
}

/// Names the arm(s) whose figure is underivable — the recorded gap names
/// its locus, never a silent placeholder.
fn underivable_arms(control_derivable: bool, treatment_derivable: bool) -> &'static str {
    match (control_derivable, treatment_derivable) {
        (true, false) => "the treatment arm",
        (false, true) => "the control arm",
        (false, false) => "both arms",
        (true, true) => "no arm",
    }
}

/// Condition 1's typed rule: the treatment dispatched a SUPERSET of the
/// control arm's units AND the target is not governance-bearing. The
/// implemented direction is the conservative one — a treatment that
/// SKIPPED a unit control dispatched is cherry-picking and alters the
/// governance shape; extra treatment dispatch passes (it is not an
/// improvement vector), and the docs say exactly that (COR-1, critic m2).
fn governance_verdict(
    control: &RunMetrics,
    treatment: &RunMetrics,
    target: &str,
) -> Result<(), GateRefusal> {
    if target_is_governance_bearing(target) {
        return Err(GateRefusal::GovernanceAltered {
            detail: format!(
                "the declared target {target:?} is classified governance-bearing — the loop's \
                 authority, settlement, scheduler, and reference surfaces are not a \
                 candidate's to change"
            ),
        });
    }
    // The tier/authority profile over dispatched units is preserved when
    // the treatment dispatched a SUPERSET of the control arm's units (the
    // same frozen plan's governance shape, budget breadth permitting) —
    // and is altered when the treatment SKIPPED a unit control dispatched
    // (cherry-picking). Extra treatment dispatch passes: the implemented
    // direction is the conservative one, and this comment claims only what
    // the check below enforces (COR-1, critic m2).
    let preserved = control
        .dispatched_units
        .iter()
        .all(|unit| treatment.dispatched_units.contains(unit));
    if !preserved {
        return Err(GateRefusal::GovernanceAltered {
            detail: format!(
                "the dispatched-unit mixes diverge (control {:?}, treatment {:?}) — the \
                 treatment arm must not skip or substitute a unit the control arm \
                 dispatched",
                control.dispatched_units, treatment.dispatched_units
            ),
        });
    }
    Ok(())
}

/// The target classification (typed, total, documented): a target naming
/// the loop's authority, settlement, scheduler, reference, or governance
/// surfaces is governance-bearing — the active system never moves its own
/// ruler. Every other target (the §5.3.8 open vocabulary) adjudicates on
/// the durable facts. The match is CASE-NORMALIZED (COR-2, R5): the
/// target's lowercased form is what the prefixes are matched against, so
/// "Reference-pipeline" and "Settlement-machinery" classify exactly as
/// their lower-case spellings do — capitalization is not an escape hatch
/// from governance.
pub fn target_is_governance_bearing(target: &str) -> bool {
    const GOVERNANCE_PREFIXES: &[&str] = &[
        "governance",
        "settlement",
        "authority",
        "scheduler",
        "reference",
    ];
    let normalized = target.to_lowercase();
    GOVERNANCE_PREFIXES
        .iter()
        .any(|prefix| normalized.starts_with(prefix))
}

/// Condition 3's instrument-series half, typed (COR-2, R2). The arms are
/// JOINED BY UNIT — the same join the regression condition uses — never by
/// list position: for every unit the CONTROL arm settled, EVERY treatment
/// settlement row of the SAME unit must carry an instrument at least as
/// strong (the deterministic rung is the strong one), so ANY weaker
/// counterpart row weakens the series at whichever position it sits. The
/// any-weaker rule is what makes the verdict ORDER-INDEPENDENT under
/// duplicated unit rows (residual E-2, task23 closure): an arm may carry
/// SEVERAL rows for one unit — distinct instruments over re-settlements —
/// and the earlier first-match join let whichever row sorted first decide,
/// an ordering-controlled row shielding a weakened counterpart. It is
/// strictly fail-safe: an extra treatment row can only ever hurt the
/// treatment arm, never shield it, and the honest single-row road is
/// unchanged. A counterpart row that names no instrument is an evidence
/// gap, but a DEMONSTRATED weakness outranks the gap — a failed condition
/// is never demoted to undecidable by another row's position. A
/// treatment-settled unit with no control counterpart is not compared —
/// the series runs over the shared unit set. Treatment settling a
/// control-settled unit NOT AT ALL, or either side of a pair missing its
/// instrument fact, is an evidence gap (undecidable refuses), never a
/// strength.
enum SeriesVerdict {
    NonWeaker,
    Weakened,
    Undecidable { detail: String },
}

fn instrument_series_verdict(control: &RunMetrics, treatment: &RunMetrics) -> SeriesVerdict {
    for control_fact in &control.settlements {
        let counterparts: Vec<_> = treatment
            .settlements
            .iter()
            .filter(|fact| fact.unit == control_fact.unit)
            .collect();
        if counterparts.is_empty() {
            return SeriesVerdict::Undecidable {
                detail: format!(
                    "unit {} settled under control but the treatment arm carries no \
                     settlement for the SAME unit — the instrument series is joined by \
                     unit and cannot decide over a missing counterpart",
                    control_fact.unit
                ),
            };
        }
        let Some(control_deterministic) = control_fact.instrument_deterministic else {
            return SeriesVerdict::Undecidable {
                detail: "a control settlement names no verifying instrument — the series \
                         cannot decide"
                    .to_owned(),
            };
        };
        // The any-weaker join (E-2): EVERY counterpart row must be at
        // least as strong. Weakness is looked for over ALL rows before
        // the missing-instrument gap is reported, so the verdict never
        // depends on which row comes first.
        let mut missing_instrument = false;
        for treatment_fact in counterparts {
            let Some(treatment_deterministic) = treatment_fact.instrument_deterministic else {
                missing_instrument = true;
                continue;
            };
            if control_deterministic && !treatment_deterministic {
                return SeriesVerdict::Weakened;
            }
        }
        if missing_instrument {
            return SeriesVerdict::Undecidable {
                detail: "a treatment settlement names no verifying instrument — the series \
                         cannot decide"
                    .to_owned(),
            };
        }
    }
    SeriesVerdict::NonWeaker
}

/// Condition 5's typed rule: every unit control settled, treatment settles
/// too, with non-weaker instruments. A missing instrument fact anywhere is
/// an evidence gap.
fn regression_verdict(control: &RunMetrics, treatment: &RunMetrics) -> Result<(), GateRefusal> {
    for control_fact in &control.settlements {
        let treatment_fact = treatment
            .settlements
            .iter()
            .find(|fact| fact.unit == control_fact.unit)
            .ok_or_else(|| GateRefusal::RegressionSuiteFailed {
                detail: format!(
                    "unit {} settled under control but not under treatment",
                    control_fact.unit
                ),
            })?;
        match (
            control_fact.instrument_deterministic,
            treatment_fact.instrument_deterministic,
        ) {
            (Some(control_instrument), Some(treatment_instrument)) => {
                if control_instrument && !treatment_instrument {
                    return Err(GateRefusal::RegressionSuiteFailed {
                        detail: format!(
                            "unit {} settled under a deterministic instrument under control \
                             but a weaker one under treatment",
                            control_fact.unit
                        ),
                    });
                }
            }
            (None, _) | (_, None) => {
                return Err(GateRefusal::EvaluationIncomplete {
                    detail: format!(
                        "unit {}'s instrument fact is missing from an arm — the regression \
                         condition is undecidable and refuses",
                        control_fact.unit
                    ),
                });
            }
        }
    }
    Ok(())
}

/// One EARLIER evaluation record's record-derived FAILED conditions (the
/// re-ask floor's sweep, task24 i1): governance (condition 1), the
/// instrument series (condition 3's record half), the regression suite
/// (condition 5), the target margin (condition 7), and payment
/// (condition 8) — each derived ONLY from the record's own durable arm
/// facts. Evidence gaps are deliberately NOT collected here
/// (`EvaluationIncomplete`, the series' `Undecidable`, the regression
/// condition's incomplete arm): a gap is undecidability, and the freshest
/// record's evidence supersedes it — that is the honest road the floor
/// must keep open. Input- and ledger-derived conditions are not swept
/// either: condition 2 compares against the CURRENT frozen digest and
/// condition 4 against the current input, and condition 6 reads the
/// capability ledger as it stands NOW — re-derived below over the
/// freshest record, which is the only honest comparison they admit. A
/// failure, by contrast, is a derived comparison over derivable figures —
/// durable evidence, never superseded by a re-roll.
fn earlier_record_failures(
    evaluation: &EvaluationRecord,
    target: &str,
    margin_bps: u64,
) -> Vec<GateRefusal> {
    let named = |detail: String| {
        format!(
            "an earlier evaluation (held-out control run {}, held-out treatment run {}) \
             recorded a failure a later re-run does not launder: {detail}",
            evaluation.held_out_control, evaluation.held_out_treatment
        )
    };
    let mut failed = Vec::new();
    if let Err(refusal) = governance_verdict(
        &evaluation.control_held_out_metrics,
        &evaluation.treatment_held_out_metrics,
        target,
    ) {
        failed.push(match refusal {
            GateRefusal::GovernanceAltered { detail } => GateRefusal::GovernanceAltered {
                detail: named(detail),
            },
            other => other,
        });
    }
    if let SeriesVerdict::Weakened = instrument_series_verdict(
        &evaluation.control_held_out_metrics,
        &evaluation.treatment_held_out_metrics,
    ) {
        failed.push(GateRefusal::SettlementAccessWeakened {
            detail: named(
                "the earlier evaluation's instrument series is weaker than its control's — \
                 verification got weaker exactly where the candidate claims improvement"
                    .to_owned(),
            ),
        });
    }
    if let Err(GateRefusal::RegressionSuiteFailed { detail }) = regression_verdict(
        &evaluation.control_regression_metrics,
        &evaluation.treatment_regression_metrics,
    ) {
        failed.push(GateRefusal::RegressionSuiteFailed {
            detail: named(detail),
        });
    }
    // Condition 7's margin arithmetic needs the record's DECLARED metric;
    // condition 8's payment floor does not. The checks derive
    // independently: an unparseable declaration leaves the margin
    // undecidable from this record and skips only the margin check — it
    // must not skip the payment floor below.
    if let Ok(declared) = TargetMetric::parse(&evaluation.metric)
        && let (Some(control), Some(treatment)) = (
            declared.figure(&evaluation.control_held_out_metrics),
            declared.figure(&evaluation.treatment_held_out_metrics),
        )
        && !margin_holds(control, treatment, margin_bps)
    {
        failed.push(GateRefusal::MarginNotMet {
            metric: declared.token().to_owned(),
            control_figure: control,
            treatment_figure: treatment,
            margin_bps,
        });
    }
    if let (Some(control), Some(treatment)) = (
        TargetMetric::PaymentPerSettlement.figure(&evaluation.control_held_out_metrics),
        TargetMetric::PaymentPerSettlement.figure(&evaluation.treatment_held_out_metrics),
    ) && treatment > control
    {
        failed.push(GateRefusal::PaymentIncreased {
            control_figure: control,
            treatment_figure: treatment,
        });
    }
    failed
}

/// Condition 7's margin arithmetic, in fixed-point per-10 000 figures
/// (cross-multiplied integer math, u128 intermediates — no floating point
/// reaches a §19.4 decision). Holds iff treatment is STRICTLY better than
/// control AND the improvement covers the margin: treatment × 10 000 ≤
/// control × (10 000 − margin_bps). Margin 0 keeps the floor at strict
/// improvement; the 10 000 ceiling (exactly 100%) demands the treatment
/// figure be 0. A margin ABOVE 10 000 bps never reaches this arithmetic —
/// it is refused typed at the gate constructor and at config resolution
/// (COR-2, R3); the saturating subtraction is the ceiling case's own
/// arithmetic, not a clamp.
fn margin_holds(control_figure: u64, treatment_figure: u64, margin_bps: u64) -> bool {
    debug_assert!(
        margin_bps <= 10_000,
        "a margin above the 10 000 bps ceiling is refused at the constructor"
    );
    if treatment_figure >= control_figure {
        return false;
    }
    let allowed = 10_000u64.saturating_sub(margin_bps);
    u128::from(treatment_figure) * 10_000 <= u128::from(control_figure) * u128::from(allowed)
}

#[cfg(test)]
pub(crate) fn margin_holds_for_tests(
    control_figure: u64,
    treatment_figure: u64,
    margin_bps: u64,
) -> bool {
    margin_holds(control_figure, treatment_figure, margin_bps)
}
