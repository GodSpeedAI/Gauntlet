//! The promotion service (plan Task 22, charter D4/D5; spec §19.2) — the
//! ONLY composer of elevating capability-status transitions.
//!
//! > Tool access is not capability. One successful run is not capability.
//! > Capability is repeated reliable settlement under relevant variation,
//! > with recovery and decreasing orchestration burden.
//! >
//! > Promote a capability only with: ≥ `GAUNTLET_PROMOTION_MIN_SETTLEMENTS`
//! > (default 3) settlements across recorded variation axes; ≥1 recovery
//! > event handled; non-increasing failure rate; **no gate erosion** (the
//! > settlements did not become easier because verification became
//! > weaker); stable or decreasing orchestration burden.
//! >
//! > **A faster workflow that became faster by skipping verification is a
//! > regression, not learning.**
//!
//! ## The typed rules, mechanically (charter D4)
//!
//! All conditions are evaluated; every failed condition is recorded as its
//! own typed refusal. The decision over the REAL ledger and the REAL
//! substrate is what AK-055 proves.
//!
//! **Windows decide on substrate chronology — with an honest residual at
//! the tie.** The outcome window and the settlement window are ordered by
//! `(occurred_at, ledger seq)` before any window logic — `occurred_at` is
//! the fact's own occurrence instant in the substrate (a verification
//! record's `produced_at`). The seq tiebreak is deterministic, but the
//! ledger seq IS append order: when observations inside the decisive pair
//! share an `occurred_at` — realistic at EpochSeconds granularity across
//! parallel runs — the verdict is order-dependent by construction. That
//! is a recorded residual (falsifier round-2 F2-1), not a property this
//! ordering retires; the ordering removes only the coarser dependency,
//! that a whole-window verdict flips with the IMPORT order of facts whose
//! occurrence instants differ. `at` (the recording instant) is never a
//! decision input. A decided window containing any observation without
//! `occurred_at` is UNDECIDABLE and refuses (`InsufficientOutcomeEvidence`
//! / `InsufficientSettlementEvidence`) — insufficient evidence reads as a
//! refusal, never as a pass.
//!
//! 1. **Minimum settlements** — at least the configured number of
//!    settlement-class observations back the capability
//!    (`InsufficientSettlements`). The observations were appended
//!    idempotently by the developmental import, so a replayed import
//!    cannot inflate the count.
//! 2. **Recorded variation** — the settlement window must differ on at
//!    least one CONTENT axis ([`VariationAxis`], computed by the rows
//!    from their own populated facts: unit, frozen reference, evidence
//!    kind and locator class). `run_id` and timestamps are deliberately
//!    not axes — they differ across every run, which is exactly the
//!    false-variation trap (AK-055). Identical axes across the window is
//!    `InsufficientVariation`, whatever the settlement count. A row
//!    missing ANY decision fact (unit, reference, kind, locator,
//!    instrument, burden, verification ref, evidence ref, occurrence
//!    instant) makes the whole settlement window undecidable — ONE typed
//!    refusal naming the missing classes
//!    (`InsufficientSettlementEvidence`): sparse rows never pass and
//!    never credit variation, because absence is not variation.
//! 3. **Recovery handled** — at least one recovery-class observation on
//!    record (`NoRecoveryObserved`). The observation is appended, ungated,
//!    by the recovery-completion call site after
//!    `complete_pending_settlements` returns a count greater than zero —
//!    never inside the settlement machinery.
//! 4. **Non-increasing failure rate** — over the capability's outcome
//!    window (attempt/failure observations, in substrate-chronology
//!    order), the later half's failure rate must not exceed the earlier
//!    half's. A failure is a FAIL outcome; only decided outcomes
//!    (PASS/FAIL) count. A window (or a half) with no decided attempts —
//!    or a decided row with no occurrence instant — is
//!    `InsufficientOutcomeEvidence` — insufficient evidence reads as a
//!    refusal, NEVER as a pass. This is a typed rule over durable
//!    outcome facts, not a semantic proof; the figures are recorded in
//!    the decision.
//! 5. **No gate erosion, stable or decreasing orchestration burden** — a
//!    settlement verified by a deterministic instrument followed by one
//!    verified only by a non-deterministic (model) rung is `GateErosion`:
//!    the settlements became easier because verification became weaker.
//!    Per-settlement orchestration cost — the typed proxy of the run's
//!    correction-selection count at settlement time — must not increase
//!    across the window (`OrchestrationBurdenIncreased`). A settlement
//!    missing its instrument or burden fact leaves the condition
//!    undecidable, which refuses (`InsufficientInstrumentEvidence` /
//!    `InsufficientBurdenEvidence`). Both are typed proxies over recorded
//!    instruments, honestly not semantic proofs.
//!
//! ## Decisions and statuses (charter D5)
//!
//! - `Promoted` — every condition held: the transition carries the
//!   durable [`PromotionEvidence`] decision record naming the settlements
//!   and their evidence.
//! - `Refused` — any condition failed: the transition writes `Provisional`
//!   with the per-condition reasons durable (§19.4's "insufficient
//!   evidence ⟹ provisional ⟹ not promoted"). A capability never
//!   evaluated stays `Observed`.
//! - A `Promoted` capability is not re-evaluated (its status stands;
//!   demotion is not a legal move); a `Revoked` capability is NEVER
//!   re-elevated — revoke is sticky, there is no flip-back path, and
//!   re-promotion would require the FULL evaluation against a
//!   non-revoked status the register cannot present.

use std::sync::Arc;

use gauntlet_domain::{CapabilityId, CapabilityStatus, Outcome};
use gauntlet_ports::capability_ledger::{
    CapabilityEntry, CapabilityLedger, CapabilityLedgerError, ObservationClass, PromotionEvidence,
    PromotionRefusal, SettlementWitness, StatusTransition, StatusTransitionError, TransitionReason,
    VariationAxis,
};
use gauntlet_ports::clock::Clock;
use gauntlet_ports::deadline::Deadline;

/// The §19.2 gate over one capability ledger.
#[derive(Clone)]
pub struct PromotionService {
    ledger: Arc<dyn CapabilityLedger>,
    min_settlements: u32,
    clock: Arc<dyn Clock>,
}

/// A durable promotion decision: the capability was promoted with its
/// evidence record, or refused with its per-condition reasons (both
/// durable as transition records; the entry carries the resulting
/// status).
#[derive(Debug, Clone, PartialEq)]
pub enum PromotionDecision {
    Promoted {
        evidence: PromotionEvidence,
        entry: CapabilityEntry,
    },
    Refused {
        refusals: Vec<PromotionRefusal>,
        entry: CapabilityEntry,
    },
}

/// Promotion/revoke service failures — typed (spec §21.2).
#[derive(Debug, Clone, PartialEq)]
pub enum PromotionServiceError {
    /// The ledger read or write failed.
    Ledger(CapabilityLedgerError),
    /// The capability has no observations — nothing to evaluate.
    UnknownCapability { capability: CapabilityId },
    /// The capability is revoked, and revoke is sticky: evaluation
    /// refuses without writing anything.
    RevokedIsSticky { capability: CapabilityId },
    /// The capability is already promoted; its status stands.
    AlreadyPromoted { capability: CapabilityId },
    /// A configured minimum of 0 — the §19.2 floor cannot hold below 1.
    InvalidMinimum { min_settlements: u32 },
    /// The transition record refused to compose (a revoke from a status
    /// the state machine does not permit leaving).
    Illegal(StatusTransitionError),
}

impl core::fmt::Display for PromotionServiceError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Ledger(error) => write!(f, "the promotion evaluation failed: {error}"),
            Self::UnknownCapability { capability } => write!(
                f,
                "capability {capability} has no observations on record; nothing can be \
                 evaluated"
            ),
            Self::RevokedIsSticky { capability } => write!(
                f,
                "capability {capability} is revoked and revoke is sticky: it is never \
                 re-elevated, only re-earned through a full evaluation of a capability that \
                 is not revoked (spec 19.2)"
            ),
            Self::AlreadyPromoted { capability } => write!(
                f,
                "capability {capability} is already promoted; its status stands (a refused \
                 re-evaluation would demote, which is not a legal move)"
            ),
            Self::InvalidMinimum { min_settlements } => write!(
                f,
                "a promotion minimum of {min_settlements} cannot hold the 19.2 floor that \
                 one successful run is not capability"
            ),
            Self::Illegal(error) => write!(f, "the transition refused to compose: {error}"),
        }
    }
}

impl std::error::Error for PromotionServiceError {}

impl PromotionService {
    /// Composes the service. A configured minimum of 0 is refused at
    /// construction (the resolution layer refuses it too — defense in
    /// depth on the same typed rule).
    pub fn new(
        ledger: Arc<dyn CapabilityLedger>,
        min_settlements: u32,
        clock: Arc<dyn Clock>,
    ) -> Result<Self, PromotionServiceError> {
        if min_settlements == 0 {
            return Err(PromotionServiceError::InvalidMinimum { min_settlements });
        }
        Ok(Self {
            ledger,
            min_settlements,
            clock,
        })
    }

    /// The configured minimum this service enforces.
    pub fn min_settlements(&self) -> u32 {
        self.min_settlements
    }

    /// Evaluates the capability against §19.2 and writes the durable
    /// decision: `Promoted` with the evidence record, or `Refused` writing
    /// `Provisional` with the per-condition reasons. A capability never
    /// evaluated stays `Observed`; a revoked capability is never
    /// re-evaluated (sticky); a promoted capability's status stands.
    pub fn evaluate(
        &self,
        capability: &CapabilityId,
        deadline: Deadline,
    ) -> Result<PromotionDecision, PromotionServiceError> {
        let entry = self.register_entry(capability, deadline)?;
        if entry.status == CapabilityStatus::Revoked {
            return Err(PromotionServiceError::RevokedIsSticky {
                capability: capability.clone(),
            });
        }
        if entry.status == CapabilityStatus::Promoted {
            return Err(PromotionServiceError::AlreadyPromoted {
                capability: capability.clone(),
            });
        }
        let observations = self
            .ledger
            .observations(capability, deadline)
            .map_err(PromotionServiceError::Ledger)?;
        let settlement_window: Vec<_> = observations
            .iter()
            .filter(|observation| observation.identity().class() == ObservationClass::Settlement)
            .collect();
        let outcome_window: Vec<_> = observations
            .iter()
            .filter(|observation| {
                matches!(
                    observation.identity().class(),
                    ObservationClass::Attempt | ObservationClass::Failure
                )
            })
            .collect();
        let recovery_count = observations
            .iter()
            .filter(|observation| observation.identity().class() == ObservationClass::Recovery)
            .count();

        // Substrate chronology BEFORE any window logic: both decided
        // windows order by `(occurred_at, ledger seq)`. The seq tiebreak
        // is deterministic but IS append order — an `occurred_at` tie
        // inside the decisive pair is order-dependent by construction
        // (the recorded residual, COR-2 C7); facts with distinct
        // occurrence instants decide on chronology, not on the import
        // call order (runs complete and import out of order under
        // parallelism). A decided window whose rows do not name their
        // occurrence instants is UNDECIDABLE: it refuses, never passes,
        // and decides no condition.
        let (settlement_window, settlement_evidence_refusal) =
            match settlement_window_facts(settlement_window) {
                Ok(window) => (window, None),
                Err(missing) => (
                    Vec::new(),
                    Some(PromotionRefusal::InsufficientSettlementEvidence { missing }),
                ),
            };
        let (outcome_window, outcome_evidence_refusal) = match ordered_by_occurrence(outcome_window)
        {
            Some(window) => (window, None),
            None => (
                Vec::new(),
                Some(PromotionRefusal::InsufficientOutcomeEvidence),
            ),
        };

        let mut refusals = Vec::new();
        // Condition 1 — the settlement count.
        let have = settlement_window.len();
        let need = self.min_settlements as usize;
        if have < need {
            refusals.push(PromotionRefusal::InsufficientSettlements {
                have,
                need: self.min_settlements,
            });
        }
        // Condition 2 — recorded variation on a CONTENT axis: identical
        // axes across the window refuse, whatever the count. A sparse
        // window is undecidable and refuses by name — it never passes,
        // and it never credits variation (absence is not variation).
        if let Some(refusal) = settlement_evidence_refusal.as_ref() {
            refusals.push(refusal.clone());
        } else if !settled_with_real_variation(&settlement_window) {
            refusals.push(PromotionRefusal::InsufficientVariation);
        }
        // Condition 3 — a recovery handled.
        if recovery_count == 0 {
            refusals.push(PromotionRefusal::NoRecoveryObserved);
        }
        // Condition 4 — the non-increasing failure rate, over the
        // chronology-ordered outcome window.
        if let Some(refusal) = outcome_evidence_refusal {
            refusals.push(refusal);
        } else if let Some(rate_refusal) = failure_rate_refusal(&outcome_window) {
            refusals.push(rate_refusal);
        }
        // Condition 5 — no gate erosion, non-increasing burden, over the
        // chronology-ordered settlement window.
        if settlement_evidence_refusal.is_none() {
            if let Some(erosion_refusal) = gate_erosion_refusal(&settlement_window) {
                refusals.push(erosion_refusal);
            }
            if let Some(burden_refusal) = burden_refusal(&settlement_window) {
                refusals.push(burden_refusal);
            }
        }

        if !refusals.is_empty() {
            let transition = StatusTransition::new(
                capability.clone(),
                entry.status,
                CapabilityStatus::Provisional,
                TransitionReason::Refused(refusals.clone()),
                Vec::new(),
                self.clock.now(),
            )
            .map_err(PromotionServiceError::Illegal)?;
            let entry = self
                .ledger
                .apply_transition(transition, deadline)
                .map_err(PromotionServiceError::Ledger)?;
            return Ok(PromotionDecision::Refused { refusals, entry });
        }

        let evidence = promotion_evidence(
            self.min_settlements,
            &settlement_window,
            recovery_count,
            &outcome_window,
        );
        let evidence_refs = evidence
            .settlements
            .iter()
            .map(|w| w.evidence.clone())
            .collect();
        let transition = StatusTransition::new(
            capability.clone(),
            entry.status,
            CapabilityStatus::Promoted,
            TransitionReason::Promoted(evidence.clone()),
            evidence_refs,
            self.clock.now(),
        )
        .map_err(PromotionServiceError::Illegal)?;
        let entry = self
            .ledger
            .apply_transition(transition, deadline)
            .map_err(PromotionServiceError::Ledger)?;
        Ok(PromotionDecision::Promoted { evidence, entry })
    }

    /// Revokes the capability (charter D5: the operator/optimizer-facing
    /// service method). From `Provisional` or `Promoted`; an unevaluated
    /// capability has nothing to revoke, and an already-revoked capability
    /// refuses — revoke is sticky.
    pub fn revoke(
        &self,
        capability: &CapabilityId,
        detail: String,
        deadline: Deadline,
    ) -> Result<CapabilityEntry, PromotionServiceError> {
        let entry = self.register_entry(capability, deadline)?;
        if entry.status == CapabilityStatus::Revoked {
            return Err(PromotionServiceError::RevokedIsSticky {
                capability: capability.clone(),
            });
        }
        let transition = StatusTransition::new(
            capability.clone(),
            entry.status,
            CapabilityStatus::Revoked,
            TransitionReason::Revoked(detail),
            Vec::new(),
            self.clock.now(),
        )
        .map_err(PromotionServiceError::Illegal)?;
        self.ledger
            .apply_transition(transition, deadline)
            .map_err(PromotionServiceError::Ledger)
    }

    fn register_entry(
        &self,
        capability: &CapabilityId,
        deadline: Deadline,
    ) -> Result<CapabilityEntry, PromotionServiceError> {
        self.ledger
            .capability(capability, deadline)
            .map_err(PromotionServiceError::Ledger)?
            .ok_or_else(|| PromotionServiceError::UnknownCapability {
                capability: capability.clone(),
            })
    }
}

/// Substrate chronology (spec §19.2 via the correction round): every
/// decided window is ordered by `(occurred_at, ledger seq)` BEFORE any
/// window logic — the ledger read-back is append (seq) order, so the
/// stable sort keeps the seq as the tiebreak. The tiebreak is
/// deterministic but IS append order: an `occurred_at` tie is
/// order-dependent by construction (the recorded residual, COR-2 C7).
/// `None` when any row in the window names no occurrence instant: the
/// window is UNDECIDABLE — the caller refuses, it never passes.
fn ordered_by_occurrence(
    mut window: Vec<&gauntlet_ports::capability_ledger::ObservationRecord>,
) -> Option<Vec<&gauntlet_ports::capability_ledger::ObservationRecord>> {
    if window
        .iter()
        .any(|observation| observation.occurred_at().is_none())
    {
        return None;
    }
    window.sort_by_key(|observation| {
        observation
            .occurred_at()
            .map(gauntlet_domain::EpochSeconds::get)
    });
    Some(window)
}

/// The settlement window's decision facts, validated and
/// chronology-ordered. EVERY settlement observation must carry the FULL
/// fact set — the unit, the frozen reference, the evidence kind and
/// locator, the verifying instrument, the orchestration cost, the
/// verification ref, the pinned evidence, and the substrate occurrence
/// instant. Any sparse row makes the window UNDECIDABLE (`Err` naming
/// the missing classes, sorted and deduplicated): a sparse window never
/// passes, and it never credits variation — absence is not variation.
/// On success the window is ordered by `(occurred_at, ledger seq)` —
/// substrate chronology, with the append-order seq tiebreak (the recorded
/// tie residual, COR-2 C7).
fn settlement_window_facts(
    window: Vec<&gauntlet_ports::capability_ledger::ObservationRecord>,
) -> Result<Vec<&gauntlet_ports::capability_ledger::ObservationRecord>, Vec<String>> {
    let mut missing: Vec<&'static str> = Vec::new();
    for observation in &window {
        for (class, present) in [
            ("unit", observation.unit().is_some()),
            ("reference_digest", observation.reference_digest().is_some()),
            ("evidence_kind", observation.evidence_kind().is_some()),
            ("evidence_locator", observation.evidence_locator().is_some()),
            (
                "instrument_deterministic",
                observation.instrument_deterministic().is_some(),
            ),
            (
                "orchestration_cost",
                observation.orchestration_cost().is_some(),
            ),
            ("verification", observation.verification().is_some()),
            ("evidence_ref", observation.evidence().is_some()),
            ("occurred_at", observation.occurred_at().is_some()),
        ] {
            if !present {
                missing.push(class);
            }
        }
    }
    if !missing.is_empty() {
        missing.sort_unstable();
        missing.dedup();
        return Err(missing.into_iter().map(str::to_owned).collect());
    }
    ordered_by_occurrence(window).ok_or_else(|| {
        // Unreachable: the validation above established every row carries
        // an occurrence instant.
        vec!["occurred_at".to_owned()]
    })
}

/// Condition 2: the settlement window carries real content variation — at
/// least one window row's axis set differs from the first row's.
fn settled_with_real_variation(
    window: &[&gauntlet_ports::capability_ledger::ObservationRecord],
) -> bool {
    let Some(first) = window.first() else {
        // No settlements at all: the count condition already refuses, and
        // there is no variation to speak of — refuse here too.
        return false;
    };
    let first_axes = first.axes();
    window
        .iter()
        .any(|observation| observation.axes() != first_axes)
}

/// Condition 4's typed rule: (failures, decided) per window half, then a
/// cross-multiplied strict-increase check (no floating point). A window
/// with no decided attempts in either half is insufficient evidence.
fn failure_rate_refusal(
    window: &[&gauntlet_ports::capability_ledger::ObservationRecord],
) -> Option<PromotionRefusal> {
    if window.is_empty() {
        return Some(PromotionRefusal::InsufficientOutcomeEvidence);
    }
    let split = window.len() / 2;
    let (earlier, later) = window.split_at(split);
    let earlier_rate = rate_of(earlier);
    let later_rate = rate_of(later);
    let Some((earlier_failures, earlier_decided)) = earlier_rate else {
        return Some(PromotionRefusal::InsufficientOutcomeEvidence);
    };
    let Some((later_failures, later_decided)) = later_rate else {
        return Some(PromotionRefusal::InsufficientOutcomeEvidence);
    };
    // failures/decided strictly increased ⟺ f2·d1 > f1·d2 (decided > 0).
    if later_failures * earlier_decided > earlier_failures * later_decided {
        return Some(PromotionRefusal::FailureRateIncreased {
            earlier_failures,
            earlier_decided,
            later_failures,
            later_decided,
        });
    }
    None
}

/// (failures, decided) over one half; `None` when the half holds no
/// decided outcome (an INCONCLUSIVE-only half cannot decide a rate).
fn rate_of(half: &[&gauntlet_ports::capability_ledger::ObservationRecord]) -> Option<(u64, u64)> {
    let mut failures = 0;
    let mut decided = 0;
    for observation in half {
        match observation.outcome() {
            Some(Outcome::Fail) => {
                failures += 1;
                decided += 1;
            }
            Some(Outcome::Pass) => decided += 1,
            Some(Outcome::Inconclusive) | None => {}
        }
    }
    if decided == 0 {
        None
    } else {
        Some((failures, decided))
    }
}

/// Condition 5's erosion half: a later settlement verified by a strictly
/// weaker instrument class than an earlier one — the deterministic rung
/// is the stronger class; once seen, a non-deterministic verification
/// behind it is erosion.
fn gate_erosion_refusal(
    window: &[&gauntlet_ports::capability_ledger::ObservationRecord],
) -> Option<PromotionRefusal> {
    let mut deterministic_seen = false;
    for observation in window {
        let Some(deterministic) = observation.instrument_deterministic() else {
            return Some(PromotionRefusal::InsufficientInstrumentEvidence);
        };
        if deterministic {
            deterministic_seen = true;
        } else if deterministic_seen {
            return Some(PromotionRefusal::GateErosion);
        }
    }
    None
}

/// Condition 5's burden half: per-settlement orchestration cost must not
/// increase across the window.
fn burden_refusal(
    window: &[&gauntlet_ports::capability_ledger::ObservationRecord],
) -> Option<PromotionRefusal> {
    let mut previous: Option<u64> = None;
    for observation in window {
        let Some(cost) = observation.orchestration_cost() else {
            return Some(PromotionRefusal::InsufficientBurdenEvidence);
        };
        if let Some(earlier) = previous
            && cost > earlier
        {
            return Some(PromotionRefusal::OrchestrationBurdenIncreased {
                earlier,
                later: cost,
            });
        }
        previous = Some(cost);
    }
    None
}

/// The §19.2 decision record's figures, from the windows the conditions
/// decided over. Called ONLY on the promote path, after
/// [`settlement_window_facts`] validated the settlement window: every row
/// carries every fact, so every settlement observation yields a witness —
/// no silent drops — and `settlements_observed == witnesses.len()` is a
/// structural invariant of the record.
fn promotion_evidence(
    min_settlements: u32,
    settlement_window: &[&gauntlet_ports::capability_ledger::ObservationRecord],
    recovery_count: usize,
    outcome_window: &[&gauntlet_ports::capability_ledger::ObservationRecord],
) -> PromotionEvidence {
    let mut axes: Vec<VariationAxis> = settlement_window
        .iter()
        .flat_map(|observation| observation.axes())
        .collect();
    axes.sort_by_key(|axis| axis.sort_key());
    axes.dedup_by_key(|axis| axis.sort_key());
    let (earlier, later) = halves(outcome_window);
    let instrument_series = settlement_window
        .iter()
        .map(|observation| {
            observation
                .instrument_deterministic()
                .expect("the validated settlement window carries every instrument fact")
        })
        .collect();
    let orchestration_costs = settlement_window
        .iter()
        .map(|observation| {
            observation
                .orchestration_cost()
                .expect("the validated settlement window carries every burden fact")
        })
        .collect();
    let settlements = settlement_window
        .iter()
        .map(|observation| SettlementWitness {
            run: observation.identity().run().clone(),
            unit: observation
                .unit()
                .expect("the validated settlement window carries every witness fact: unit"),
            verification: observation
                .verification()
                .expect("the validated settlement window carries every witness fact: verification")
                .clone(),
            evidence: observation
                .evidence()
                .expect("the validated settlement window carries every witness fact: evidence")
                .clone(),
        })
        .collect();
    PromotionEvidence {
        min_settlements_configured: min_settlements,
        settlements_observed: settlement_window.len(),
        variation_axes: axes,
        recovery_observations: recovery_count,
        failure_rate_earlier: earlier.unwrap_or((0, 0)),
        failure_rate_later: later.unwrap_or((0, 0)),
        instrument_series,
        orchestration_costs,
        settlements,
    }
}

/// One window half's (failures, decided) rate, or `None` when the half
/// holds no decided outcome.
type HalfRate = Option<(u64, u64)>;

fn halves(
    window: &[&gauntlet_ports::capability_ledger::ObservationRecord],
) -> (HalfRate, HalfRate) {
    let split = window.len() / 2;
    let (earlier, later) = window.split_at(split);
    (rate_of(earlier), rate_of(later))
}

/// The typed import error vocabulary surfaces through the promotion path
/// too (an evaluation over a ledger that fails reads refuses).
impl From<CapabilityLedgerError> for PromotionServiceError {
    fn from(error: CapabilityLedgerError) -> Self {
        Self::Ledger(error)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use gauntlet_domain::{
        CapabilityId, CapabilityStatus, Digest, EpochSeconds, EvidenceKind, EvidenceLocator,
        EvidenceRef, Outcome, RunId, UnitId, VerificationRef,
    };
    use gauntlet_ports::capability_ledger::{
        CapabilityLedger, ObservationClass, ObservationIdentity, ObservationRecord,
        StatusTransition, StatusTransitionError, TransitionReason,
    };

    use super::{PromotionDecision, PromotionRefusal, PromotionService, PromotionServiceError};
    use crate::memory::testing::{FixedClock, MemoryLedger, bound};

    const CAP: &str = "cap-promotion-tests";
    /// The fixture locator: a single-segment locator name (path-shaped
    /// literals are environment coupling wherever they are rooted).
    const FIXTURE_LOCATOR: &str = "p10-fixture-locator";
    const REF: &str = "sha256:ab00000000000000000000000000000000000000000000000000000000000000";
    const BASE: u64 = 1_700_000_000;

    fn capability() -> CapabilityId {
        CapabilityId::new(CAP).expect("test capability id")
    }

    fn run(sequence: usize) -> RunId {
        // 26-char Crockford form; the two-char suffix varies over the tests' runs
        // (hex digits are Crockford characters).
        RunId::new(&format!("01ARZ3NDEKTSV4RRFFQ69G5F{sequence:02X}")).expect("test run id")
    }

    fn settlement_observation(
        sequence: usize,
        unit: &str,
        deterministic: bool,
        cost: u64,
    ) -> ObservationRecord {
        let fact = format!("settlement:{unit}:vrf-set-{sequence}");
        ObservationRecord::new(
            ObservationIdentity::new(
                capability(),
                ObservationClass::Settlement,
                run(sequence),
                fact,
            )
            .expect("settlement identity"),
            EpochSeconds::new(BASE),
            format!("unit {unit} settled"),
        )
        .with_unit(UnitId::new(unit).expect("test unit"))
        .with_outcome(Outcome::Pass)
        .with_instrument_deterministic(deterministic)
        .with_evidence_kind(EvidenceKind::TestReport)
        .with_evidence_locator(EvidenceLocator::new(FIXTURE_LOCATOR).expect("test locator"))
        .with_reference_digest(Digest::parse(REF).expect("test digest"))
        .with_verification(VerificationRef::new(&format!("vrf-set-{sequence}")).expect("ref"))
        .with_evidence(EvidenceRef::new(&format!("evd-set-{sequence}")).expect("evidence"))
        .with_orchestration_cost(cost)
        // The substrate occurrence fact the import projects from the
        // matched verification record's `produced_at`.
        .with_occurred_at(EpochSeconds::new(BASE + sequence as u64))
    }

    fn attempt_observation(sequence: usize, outcome: Outcome) -> ObservationRecord {
        attempt_observation_at(sequence, outcome, EpochSeconds::new(BASE + sequence as u64))
    }

    /// Same row with an explicit substrate occurrence instant (the chronology
    /// the windows decide on).
    fn attempt_observation_at(
        sequence: usize,
        outcome: Outcome,
        occurred_at: EpochSeconds,
    ) -> ObservationRecord {
        let fact = format!("verification:vrf-att-{sequence}");
        ObservationRecord::new(
            ObservationIdentity::new(
                capability(),
                match outcome {
                    Outcome::Fail => ObservationClass::Failure,
                    Outcome::Pass | Outcome::Inconclusive => ObservationClass::Attempt,
                },
                run(10 + sequence),
                fact,
            )
            .expect("attempt identity"),
            EpochSeconds::new(BASE),
            "an outcome fact".to_owned(),
        )
        .with_outcome(outcome)
        .with_occurred_at(occurred_at)
    }

    fn recovery_observation() -> ObservationRecord {
        ObservationRecord::new(
            ObservationIdentity::new(
                capability(),
                ObservationClass::Recovery,
                run(20),
                "recovery:settlement-replay".to_owned(),
            )
            .expect("recovery identity"),
            EpochSeconds::new(BASE),
            "replay completion completed one settlement".to_owned(),
        )
    }

    /// The full-legal window: three settlements under real variation
    /// (distinct units), every verification deterministic, flat burden,
    /// three PASS outcomes, one recovery. Everything §19.2 asks for.
    fn seed_full(ledger: &MemoryLedger) {
        for (sequence, unit) in ["AK-9401", "AK-9402", "AK-9403"].iter().enumerate() {
            ledger
                .append_observation(settlement_observation(sequence, unit, true, 0), bound())
                .expect("settlement seeds");
        }
        for sequence in 0..3 {
            ledger
                .append_observation(attempt_observation(sequence, Outcome::Pass), bound())
                .expect("attempt seeds");
        }
        ledger
            .append_observation(recovery_observation(), bound())
            .expect("recovery seeds");
    }

    fn service(ledger: &Arc<MemoryLedger>) -> PromotionService {
        PromotionService::new(ledger.clone(), 3, Arc::new(FixedClock))
            .expect("the default minimum is valid")
    }

    fn refusal_rules(decision: &PromotionDecision) -> Vec<&'static str> {
        match decision {
            PromotionDecision::Promoted { .. } => Vec::new(),
            PromotionDecision::Refused { refusals, .. } => {
                refusals.iter().map(PromotionRefusal::rule).collect()
            }
        }
    }

    #[test]
    fn the_full_satisfaction_path_is_the_only_road_to_promoted() {
        let ledger = Arc::new(MemoryLedger::new());
        seed_full(&ledger);
        let decision = service(&ledger)
            .evaluate(&capability(), bound())
            .expect("the full evidence promotes");
        let (evidence, entry) = match decision {
            PromotionDecision::Promoted { evidence, entry } => (evidence, entry),
            PromotionDecision::Refused { refusals, .. } => {
                panic!("the full evidence was refused: {refusals:?}")
            }
        };
        assert_eq!(entry.status, CapabilityStatus::Promoted);
        assert_eq!(evidence.settlements_observed, 3);
        assert_eq!(evidence.min_settlements_configured, 3);
        assert_eq!(evidence.recovery_observations, 1);
        // Structural invariant (no silent drops): every settlement
        // observation in the window yields a witness — the decision
        // record never carries fewer witnesses than settlements observed.
        assert_eq!(evidence.settlements.len(), evidence.settlements_observed);
        // The recorded variation is real: the union across the window
        // holds three distinct unit axes (plus the shared reference,
        // kind, and locator axes the settlements agree on).
        let unit_axes = evidence
            .variation_axes
            .iter()
            .filter(|axis| axis.sort_key().starts_with("unit:"))
            .count();
        assert_eq!(unit_axes, 3);
        // The decision record is durable: the transition history carries it
        // with the settlements' evidence refs.
        let history = ledger.transitions(&capability(), bound()).expect("history");
        assert_eq!(history.len(), 1);
        let TransitionReason::Promoted(recorded) = history[0].reason() else {
            panic!("the applied transition is not a promotion");
        };
        assert_eq!(recorded.settlements.len(), 3);
        assert_eq!(
            history[0].evidence().len(),
            3,
            "each settlement's evidence is named"
        );
    }

    #[test]
    fn fewer_settlements_than_the_minimum_refuse_and_write_provisional() {
        // Roll the window back to two settlements: no promotion can rest
        // on two.
        let ledger = Arc::new(MemoryLedger::new());
        ledger
            .append_observation(settlement_observation(0, "AK-9401", true, 0), bound())
            .expect("seed");
        ledger
            .append_observation(settlement_observation(1, "AK-9402", true, 0), bound())
            .expect("seed");
        for sequence in 0..3 {
            ledger
                .append_observation(attempt_observation(sequence, Outcome::Pass), bound())
                .expect("seed");
        }
        ledger
            .append_observation(recovery_observation(), bound())
            .expect("seed");
        let decision = service(&ledger)
            .evaluate(&capability(), bound())
            .expect("the evaluation runs");
        assert!(refusal_rules(&decision).contains(&"insufficient_settlements"));
        match &decision {
            PromotionDecision::Refused { entry, .. } => {
                assert_eq!(entry.status, CapabilityStatus::Provisional)
            }
            PromotionDecision::Promoted { .. } => panic!("two settlements promoted"),
        }
    }

    #[test]
    fn identical_content_axes_refuse_insufficient_variation() {
        // AK-055's teeth: three settlements whose content axes are
        // identical are NOT recorded variation — only the runs differ.
        let ledger = Arc::new(MemoryLedger::new());
        for sequence in 0..3 {
            ledger
                .append_observation(
                    settlement_observation(sequence, "AK-9401", true, 0),
                    bound(),
                )
                .expect("seed");
        }
        for sequence in 0..3 {
            ledger
                .append_observation(attempt_observation(sequence, Outcome::Pass), bound())
                .expect("seed");
        }
        ledger
            .append_observation(recovery_observation(), bound())
            .expect("seed");
        let decision = service(&ledger)
            .evaluate(&capability(), bound())
            .expect("the evaluation runs");
        assert!(refusal_rules(&decision).contains(&"insufficient_variation"));
        match &decision {
            PromotionDecision::Refused { entry, .. } => {
                assert_eq!(entry.status, CapabilityStatus::Provisional)
            }
            PromotionDecision::Promoted { .. } => {
                panic!("identical axes promoted — the false-variation trap is open")
            }
        }
    }

    #[test]
    fn no_recovery_event_refuses() {
        // The full window minus the recovery observation.
        let ledger = Arc::new(MemoryLedger::new());
        for (sequence, unit) in ["AK-9401", "AK-9402", "AK-9403"].iter().enumerate() {
            ledger
                .append_observation(settlement_observation(sequence, unit, true, 0), bound())
                .expect("seed");
        }
        for sequence in 0..3 {
            ledger
                .append_observation(attempt_observation(sequence, Outcome::Pass), bound())
                .expect("seed");
        }
        let decision = service(&ledger)
            .evaluate(&capability(), bound())
            .expect("the evaluation runs");
        assert!(refusal_rules(&decision).contains(&"no_recovery_observed"));
    }

    #[test]
    fn a_rising_failure_rate_refuses() {
        let ledger = Arc::new(MemoryLedger::new());
        seed_full(&ledger);
        // A FAIL outcome after two passes: the later half's rate rises.
        ledger
            .append_observation(attempt_observation(3, Outcome::Fail), bound())
            .expect("seed");
        let decision = service(&ledger)
            .evaluate(&capability(), bound())
            .expect("the evaluation runs");
        assert!(refusal_rules(&decision).contains(&"failure_rate_increased"));
    }

    #[test]
    fn windows_decide_on_substrate_chronology_never_append_order() {
        // The B3b mechanism at the window level: the SAME honest facts,
        // appended newest-first (import order 4,3,2,1), must read as the
        // same trajectory. The FAIL is the NEWEST fact (its occurrence
        // instant is the latest) but lands FIRST in the ledger; an
        // append-ordered window inverts the halves and certifies a rising
        // rate as improving. The window orders by (occurred_at, ledger
        // seq) before any window logic, so the verdict is a function of
        // the recorded facts, never of the import call order.
        let ledger = Arc::new(MemoryLedger::new());
        // Import order 4,3,2,1: the newest fact lands first.
        ledger
            .append_observation(
                attempt_observation_at(3, Outcome::Fail, EpochSeconds::new(BASE + 30)),
                bound(),
            )
            .expect("seed the newest fact first");
        for sequence in 0..3 {
            ledger
                .append_observation(attempt_observation(sequence, Outcome::Pass), bound())
                .expect("seed");
        }
        for (sequence, unit) in ["AK-9401", "AK-9402", "AK-9403"].iter().enumerate() {
            ledger
                .append_observation(settlement_observation(sequence, unit, true, 0), bound())
                .expect("seed");
        }
        ledger
            .append_observation(recovery_observation(), bound())
            .expect("seed");
        let decision = service(&ledger)
            .evaluate(&capability(), bound())
            .expect("the evaluation runs");
        match &decision {
            PromotionDecision::Refused { refusals, entry } => {
                assert_eq!(
                    refusals,
                    &vec![PromotionRefusal::FailureRateIncreased {
                        earlier_failures: 0,
                        earlier_decided: 2,
                        later_failures: 1,
                        later_decided: 2,
                    }],
                    "the occurrence-ordered halves are (0,2) earlier, (1,2) later — the \
                     recorded figures must follow substrate chronology, not import order"
                );
                assert_eq!(entry.status, CapabilityStatus::Provisional);
            }
            PromotionDecision::Promoted { .. } => panic!(
                "the newest-first import PROMOTED a genuinely rising failure rate — the \
                 window decided on import order, not on substrate chronology (B3b)"
            ),
        }
    }

    #[test]
    fn an_outcome_window_missing_occurrence_facts_is_undecidable() {
        // Chronology is a decision input: a decided outcome window whose
        // rows do not name when their facts occurred cannot order the
        // halves, so it refuses — insufficient evidence reads as a
        // refusal, never as a pass (fail-closed).
        let ledger = Arc::new(MemoryLedger::new());
        for (sequence, unit) in ["AK-9401", "AK-9402", "AK-9403"].iter().enumerate() {
            ledger
                .append_observation(settlement_observation(sequence, unit, true, 0), bound())
                .expect("seed");
        }
        // Attempts with NO occurred_at: no substrate chronology to order by.
        for sequence in 0..3 {
            let identity = ObservationIdentity::new(
                capability(),
                ObservationClass::Attempt,
                run(10 + sequence),
                format!("verification:vrf-att-{sequence}"),
            )
            .expect("attempt identity");
            ledger
                .append_observation(
                    ObservationRecord::new(
                        identity,
                        EpochSeconds::new(BASE),
                        "an outcome fact with no occurrence instant".to_owned(),
                    )
                    .with_outcome(Outcome::Pass),
                    bound(),
                )
                .expect("seed");
        }
        ledger
            .append_observation(recovery_observation(), bound())
            .expect("seed");
        let decision = service(&ledger)
            .evaluate(&capability(), bound())
            .expect("the evaluation runs");
        assert!(
            refusal_rules(&decision).contains(&"insufficient_outcome_evidence"),
            "a window without occurrence facts is undecidable: {decision:?}"
        );
        match &decision {
            PromotionDecision::Refused { .. } => {}
            PromotionDecision::Promoted { .. } => {
                panic!("an unordered window promoted — undecidable evidence passed")
            }
        }
    }

    #[test]
    fn sparse_settlement_rows_never_promote_and_never_credit_variation_b2a() {
        // The falsifier's B2a, verbatim: one FULL settlement row plus two
        // SPARSE rows (no unit, no reference, no evidence kind, no
        // locator; instrument, burden, verification, evidence, outcome
        // present). Sparse rows made the axis SETS differ, so variation
        // was credited to ABSENCE and the window promoted. A settlement
        // window whose rows do not carry the full fact set is undecidable:
        // it refuses by name, never passes, and never credits variation.
        let ledger = Arc::new(MemoryLedger::new());
        ledger
            .append_observation(settlement_observation(0, "AK-9401", true, 0), bound())
            .expect("seed the full row");
        for sequence in 1..=2 {
            let identity = ObservationIdentity::new(
                capability(),
                ObservationClass::Settlement,
                run(sequence),
                format!("settlement:sparse:vrf-set-{sequence}"),
            )
            .expect("sparse identity");
            ledger
                .append_observation(
                    ObservationRecord::new(
                        identity,
                        EpochSeconds::new(BASE),
                        "a sparse settlement row".to_owned(),
                    )
                    .with_outcome(Outcome::Pass)
                    .with_instrument_deterministic(true)
                    .with_verification(
                        VerificationRef::new(&format!("vrf-set-{sequence}")).expect("ref"),
                    )
                    .with_evidence(EvidenceRef::new(&format!("evd-set-{sequence}")).expect("evd"))
                    .with_orchestration_cost(0)
                    .with_occurred_at(EpochSeconds::new(BASE + sequence as u64)),
                    bound(),
                )
                .expect("seed the sparse rows");
        }
        for sequence in 0..3 {
            ledger
                .append_observation(attempt_observation(sequence, Outcome::Pass), bound())
                .expect("seed");
        }
        ledger
            .append_observation(recovery_observation(), bound())
            .expect("seed");
        let decision = service(&ledger)
            .evaluate(&capability(), bound())
            .expect("the evaluation runs");
        match &decision {
            PromotionDecision::Refused { refusals, entry } => {
                assert!(
                    refusals
                        .iter()
                        .any(|refusal| refusal.rule() == "insufficient_settlement_evidence"),
                    "the sparse window refuses by name: {refusals:?}"
                );
                let missing = match refusals
                    .iter()
                    .find(|refusal| refusal.rule() == "insufficient_settlement_evidence")
                {
                    Some(PromotionRefusal::InsufficientSettlementEvidence { missing }) => {
                        missing.clone()
                    }
                    other => panic!("the refusal names the missing classes: {other:?}"),
                };
                for class in [
                    "unit",
                    "reference_digest",
                    "evidence_kind",
                    "evidence_locator",
                ] {
                    assert!(
                        missing.iter().any(|named| named == class),
                        "the refusal names the missing {class} class: {missing:?}"
                    );
                }
                assert_eq!(entry.status, CapabilityStatus::Provisional);
            }
            PromotionDecision::Promoted { evidence, .. } => panic!(
                "a window with sparse settlement rows PROMOTED — variation was credited to \
                 absence (variation_axes: {:?})",
                evidence.variation_axes
            ),
        }
    }

    #[test]
    fn all_sparse_identical_settlements_still_refuse_b2b() {
        // The falsifier's B2b: three all-sparse rows with identical EMPTY
        // axes still refuse — either the insufficient-evidence rule or the
        // variation rule fires (both are acceptable refusals); the window
        // never promotes on empty-axis "variation".
        let ledger = Arc::new(MemoryLedger::new());
        for sequence in 0..3 {
            let identity = ObservationIdentity::new(
                capability(),
                ObservationClass::Settlement,
                run(sequence),
                format!("settlement:sparse:vrf-set-{sequence}"),
            )
            .expect("sparse identity");
            ledger
                .append_observation(
                    ObservationRecord::new(
                        identity,
                        EpochSeconds::new(BASE),
                        "a sparse settlement row".to_owned(),
                    )
                    .with_outcome(Outcome::Pass)
                    .with_instrument_deterministic(true)
                    .with_verification(
                        VerificationRef::new(&format!("vrf-set-{sequence}")).expect("ref"),
                    )
                    .with_evidence(EvidenceRef::new(&format!("evd-set-{sequence}")).expect("evd"))
                    .with_orchestration_cost(0)
                    .with_occurred_at(EpochSeconds::new(BASE + sequence as u64)),
                    bound(),
                )
                .expect("seed");
        }
        for sequence in 0..3 {
            ledger
                .append_observation(attempt_observation(sequence, Outcome::Pass), bound())
                .expect("seed");
        }
        ledger
            .append_observation(recovery_observation(), bound())
            .expect("seed");
        let decision = service(&ledger)
            .evaluate(&capability(), bound())
            .expect("the evaluation runs");
        match &decision {
            PromotionDecision::Refused { refusals, entry } => {
                assert!(
                    refusals.iter().any(|refusal| {
                        refusal.rule() == "insufficient_variation"
                            || refusal.rule() == "insufficient_settlement_evidence"
                    }),
                    "identical EMPTY axes refuse as insufficient evidence or insufficient \
                     variation: {refusals:?}"
                );
                assert_eq!(entry.status, CapabilityStatus::Provisional);
            }
            PromotionDecision::Promoted { .. } => {
                panic!("all-sparse identical settlements promoted")
            }
        }
    }

    #[test]
    fn an_undecidable_outcome_window_refuses_never_passes() {
        // Only INCONCLUSIVE attempts: the failure rate cannot be decided,
        // and insufficient evidence reads as a refusal, never a pass.
        let ledger = Arc::new(MemoryLedger::new());
        for (sequence, unit) in ["AK-9401", "AK-9402", "AK-9403"].iter().enumerate() {
            ledger
                .append_observation(settlement_observation(sequence, unit, true, 0), bound())
                .expect("seed");
        }
        for sequence in 0..3 {
            ledger
                .append_observation(
                    attempt_observation(sequence, Outcome::Inconclusive),
                    bound(),
                )
                .expect("seed");
        }
        ledger
            .append_observation(recovery_observation(), bound())
            .expect("seed");
        let decision = service(&ledger)
            .evaluate(&capability(), bound())
            .expect("the evaluation runs");
        assert!(refusal_rules(&decision).contains(&"insufficient_outcome_evidence"));
    }

    #[test]
    fn gate_erosion_refuses_the_weaker_later_instrument() {
        let ledger = Arc::new(MemoryLedger::new());
        for (sequence, unit) in ["AK-9401", "AK-9402", "AK-9403"].iter().enumerate() {
            let deterministic = sequence < 2;
            ledger
                .append_observation(
                    settlement_observation(sequence, unit, deterministic, 0),
                    bound(),
                )
                .expect("seed");
        }
        for sequence in 0..3 {
            ledger
                .append_observation(attempt_observation(sequence, Outcome::Pass), bound())
                .expect("seed");
        }
        ledger
            .append_observation(recovery_observation(), bound())
            .expect("seed");
        let decision = service(&ledger)
            .evaluate(&capability(), bound())
            .expect("the evaluation runs");
        assert!(refusal_rules(&decision).contains(&"gate_erosion"));
    }

    #[test]
    fn an_undecidable_erosion_window_refuses() {
        // The third settlement names no instrument — erosion cannot be
        // decided, so it refuses. Since the correction round the
        // settlement window is pre-validated as a whole: a row missing
        // its instrument fact (here: also kind, locator, and evidence) is
        // the typed insufficient-evidence refusal naming the missing
        // classes — fail-closed, never a pass, and erosion is never
        // decided over a sparse window.
        let anonymous = ObservationRecord::new(
            ObservationIdentity::new(
                capability(),
                ObservationClass::Settlement,
                run(3),
                "settlement:AK-9403:vrf-set-3".to_owned(),
            )
            .expect("settlement identity"),
            EpochSeconds::new(BASE),
            "unit AK-9403 settled".to_owned(),
        )
        .with_unit(UnitId::new("AK-9403").expect("test unit"))
        .with_outcome(Outcome::Pass)
        .with_reference_digest(Digest::parse(REF).expect("test digest"))
        .with_verification(VerificationRef::new("vrf-set-3").expect("ref"))
        .with_orchestration_cost(0)
        .with_occurred_at(EpochSeconds::new(BASE + 3));
        let ledger = Arc::new(MemoryLedger::new());
        ledger
            .append_observation(settlement_observation(0, "AK-9401", true, 0), bound())
            .expect("seed");
        ledger
            .append_observation(settlement_observation(1, "AK-9402", true, 0), bound())
            .expect("seed");
        ledger.append_observation(anonymous, bound()).expect("seed");
        for sequence in 0..3 {
            ledger
                .append_observation(attempt_observation(sequence, Outcome::Pass), bound())
                .expect("seed");
        }
        ledger
            .append_observation(recovery_observation(), bound())
            .expect("seed");
        let decision = service(&ledger)
            .evaluate(&capability(), bound())
            .expect("the evaluation runs");
        let refusals = match &decision {
            PromotionDecision::Refused { refusals, .. } => refusals,
            PromotionDecision::Promoted { .. } => {
                panic!("a window with a fact-bare settlement promoted")
            }
        };
        assert!(
            refusals
                .iter()
                .any(|refusal| refusal.rule() == "insufficient_settlement_evidence"),
            "the sparse window refuses by name: {refusals:?}"
        );
        match refusals
            .iter()
            .find(|refusal| refusal.rule() == "insufficient_settlement_evidence")
        {
            Some(PromotionRefusal::InsufficientSettlementEvidence { missing }) => {
                assert!(
                    missing
                        .iter()
                        .any(|named| named == "instrument_deterministic"),
                    "the refusal names the missing instrument class: {missing:?}"
                );
            }
            other => panic!("the refusal names the missing classes: {other:?}"),
        }
    }

    #[test]
    fn a_rising_orchestration_burden_refuses() {
        let ledger = Arc::new(MemoryLedger::new());
        for (sequence, unit) in ["AK-9401", "AK-9402", "AK-9403"].iter().enumerate() {
            let cost = if sequence == 0 { 0 } else { 1 };
            ledger
                .append_observation(settlement_observation(sequence, unit, true, cost), bound())
                .expect("seed");
        }
        for sequence in 0..3 {
            ledger
                .append_observation(attempt_observation(sequence, Outcome::Pass), bound())
                .expect("seed");
        }
        ledger
            .append_observation(recovery_observation(), bound())
            .expect("seed");
        let decision = service(&ledger)
            .evaluate(&capability(), bound())
            .expect("the evaluation runs");
        assert!(refusal_rules(&decision).contains(&"orchestration_burden_increased"));
    }

    #[test]
    fn an_undecidable_burden_window_refuses() {
        // The second settlement carries no burden fact — the burden
        // condition cannot be decided, so the window refuses. Since the
        // correction round the settlement window is pre-validated as a
        // whole: the sparse row is the typed insufficient-evidence
        // refusal naming the missing classes (orchestration_cost among
        // them) — fail-closed, never a pass.
        let ledger = Arc::new(MemoryLedger::new());
        ledger
            .append_observation(settlement_observation(0, "AK-9401", true, 0), bound())
            .expect("seed");
        // A settlement with no burden fact: the burden condition cannot be
        // decided, so it refuses.
        let anonymous = ObservationRecord::new(
            ObservationIdentity::new(
                capability(),
                ObservationClass::Settlement,
                run(1),
                "settlement:AK-9402:vrf-set-1".to_owned(),
            )
            .expect("settlement identity"),
            EpochSeconds::new(BASE),
            "unit AK-9402 settled".to_owned(),
        )
        .with_unit(UnitId::new("AK-9402").expect("test unit"))
        .with_outcome(Outcome::Pass)
        .with_instrument_deterministic(true)
        .with_reference_digest(Digest::parse(REF).expect("test digest"))
        .with_verification(VerificationRef::new("vrf-set-1").expect("ref"))
        .with_occurred_at(EpochSeconds::new(BASE + 1));
        ledger.append_observation(anonymous, bound()).expect("seed");
        ledger
            .append_observation(settlement_observation(2, "AK-9403", true, 0), bound())
            .expect("seed");
        for sequence in 0..3 {
            ledger
                .append_observation(attempt_observation(sequence, Outcome::Pass), bound())
                .expect("seed");
        }
        ledger
            .append_observation(recovery_observation(), bound())
            .expect("seed");
        let decision = service(&ledger)
            .evaluate(&capability(), bound())
            .expect("the evaluation runs");
        let refusals = match &decision {
            PromotionDecision::Refused { refusals, .. } => refusals,
            PromotionDecision::Promoted { .. } => {
                panic!("a window with a fact-bare settlement promoted")
            }
        };
        assert!(
            refusals
                .iter()
                .any(|refusal| refusal.rule() == "insufficient_settlement_evidence"),
            "the sparse window refuses by name: {refusals:?}"
        );
        match refusals
            .iter()
            .find(|refusal| refusal.rule() == "insufficient_settlement_evidence")
        {
            Some(PromotionRefusal::InsufficientSettlementEvidence { missing }) => {
                assert!(
                    missing.iter().any(|named| named == "orchestration_cost"),
                    "the refusal names the missing burden class: {missing:?}"
                );
            }
            other => panic!("the refusal names the missing classes: {other:?}"),
        }
    }

    #[test]
    fn a_refused_evaluation_is_durable_and_a_second_refusal_re_records() {
        // Two settlements instead of three: refused, Provisional, reasons
        // durable.
        let starved = Arc::new(MemoryLedger::new());
        starved
            .append_observation(settlement_observation(0, "AK-9401", true, 0), bound())
            .expect("seed");
        starved
            .append_observation(settlement_observation(0, "AK-9401", true, 0), bound())
            .expect("seed");
        starved
            .append_observation(settlement_observation(1, "AK-9402", true, 0), bound())
            .expect("seed");
        for sequence in 0..3 {
            starved
                .append_observation(attempt_observation(sequence, Outcome::Pass), bound())
                .expect("seed");
        }
        starved
            .append_observation(recovery_observation(), bound())
            .expect("seed");
        let service = service(&starved);
        let first = service.evaluate(&capability(), bound()).expect("evaluates");
        match &first {
            PromotionDecision::Refused { entry, .. } => {
                assert_eq!(entry.status, CapabilityStatus::Provisional)
            }
            PromotionDecision::Promoted { .. } => panic!("two settlements promoted"),
        }
        // A second refused evaluation re-records its reasons: the status
        // stays Provisional (a non-elevating re-record), the history grows.
        let second = service.evaluate(&capability(), bound()).expect("evaluates");
        match &second {
            PromotionDecision::Refused { entry, .. } => {
                assert_eq!(entry.status, CapabilityStatus::Provisional)
            }
            PromotionDecision::Promoted { .. } => panic!("the re-evaluation promoted"),
        }
        let history = starved
            .transitions(&capability(), bound())
            .expect("history");
        assert_eq!(history.len(), 2);
        for transition in &history {
            assert!(matches!(transition.reason(), TransitionReason::Refused(_)));
        }
    }

    #[test]
    fn the_honest_service_path_is_unaffected_by_the_zero_reason_refusal_gate() {
        // COR-2 C6: the zero-reason composition gate is a floor under the
        // durable record, not a brake on the honest service. The honest
        // refusal — one settlement short of the minimum — still evaluates,
        // still writes Provisional, and its reasons stay durable and whole.
        let starved = Arc::new(MemoryLedger::new());
        starved
            .append_observation(settlement_observation(0, "AK-9401", true, 0), bound())
            .expect("seed");
        starved
            .append_observation(settlement_observation(1, "AK-9402", true, 0), bound())
            .expect("seed");
        for sequence in 0..3 {
            starved
                .append_observation(attempt_observation(sequence, Outcome::Pass), bound())
                .expect("seed");
        }
        starved
            .append_observation(recovery_observation(), bound())
            .expect("seed");
        let starved_service = service(&starved);
        let decision = starved_service
            .evaluate(&capability(), bound())
            .expect("evaluates");
        let refusals = match &decision {
            PromotionDecision::Refused { refusals, .. } => refusals.clone(),
            PromotionDecision::Promoted { .. } => panic!("two settlements promoted"),
        };
        assert!(
            !refusals.is_empty(),
            "the honest refusal carries its per-condition reasons"
        );
        let history = starved
            .transitions(&capability(), bound())
            .expect("history");
        assert_eq!(history.len(), 1, "exactly the honest refusal is durable");
        match history[0].reason() {
            TransitionReason::Refused(recorded) => assert_eq!(
                recorded, &refusals,
                "the durable record is the honest refusal with its reasons whole"
            ),
            other => panic!("the honest record is a refusal, got {other:?}"),
        }
        // The gate itself: the zero-reason shape composes nowhere — not
        // even the F2-2 re-record against the Provisional status the
        // honest service just wrote — and a refused composition writes
        // nothing.
        match StatusTransition::new(
            capability(),
            CapabilityStatus::Provisional,
            CapabilityStatus::Provisional,
            TransitionReason::Refused(Vec::new()),
            Vec::new(),
            EpochSeconds::new(BASE),
        ) {
            Err(StatusTransitionError::EmptyRefusal) => {}
            _ => panic!("a zero-reason refusal composed"),
        }
        assert_eq!(
            starved
                .transitions(&capability(), bound())
                .expect("history")
                .len(),
            1,
            "the refused composition wrote nothing"
        );
        // The other half of the honest path: full satisfaction still
        // promotes with its decision record.
        let full = Arc::new(MemoryLedger::new());
        seed_full(&full);
        let full_service = service(&full);
        match full_service
            .evaluate(&capability(), bound())
            .expect("evaluates")
        {
            PromotionDecision::Promoted { evidence, .. } => {
                assert!(
                    !evidence.settlements.is_empty(),
                    "the promotion carries its settlement witnesses"
                );
            }
            PromotionDecision::Refused { .. } => panic!("the full window refused"),
        }
    }

    #[test]
    fn an_already_promoted_capability_is_not_re_evaluated() {
        let ledger = Arc::new(MemoryLedger::new());
        seed_full(&ledger);
        let service = service(&ledger);
        service.evaluate(&capability(), bound()).expect("promotes");
        assert!(matches!(
            service.evaluate(&capability(), bound()),
            Err(PromotionServiceError::AlreadyPromoted { .. })
        ));
    }

    #[test]
    fn revoke_is_sticky_against_evaluation_and_again() {
        let ledger = Arc::new(MemoryLedger::new());
        seed_full(&ledger);
        let service = service(&ledger);
        service.evaluate(&capability(), bound()).expect("promotes");
        let entry = service
            .revoke(
                &capability(),
                "the operator revoked the route".to_owned(),
                bound(),
            )
            .expect("revokes");
        assert_eq!(entry.status, CapabilityStatus::Revoked);
        // Evaluation refuses without writing, and never re-promotes.
        assert!(matches!(
            service.evaluate(&capability(), bound()),
            Err(PromotionServiceError::RevokedIsSticky { .. })
        ));
        assert_eq!(
            ledger
                .capability(&capability(), bound())
                .expect("reads")
                .expect("present")
                .status,
            CapabilityStatus::Revoked,
            "the evaluation moved nothing"
        );
        // A second revoke is the same sticky refusal.
        assert!(matches!(
            service.revoke(&capability(), "again".to_owned(), bound()),
            Err(PromotionServiceError::RevokedIsSticky { .. })
        ));
        // The history ends at the one revoke.
        let history = ledger.transitions(&capability(), bound()).expect("history");
        assert_eq!(history.len(), 2);
        assert!(matches!(history[1].reason(), TransitionReason::Revoked(_)));
    }

    #[test]
    fn an_observed_capability_cannot_be_revoked_directly() {
        let ledger = Arc::new(MemoryLedger::new());
        seed_full(&ledger);
        // Observed has never been evaluated; the state machine leaves it
        // only toward Provisional or Promoted.
        let service = service(&ledger);
        assert!(matches!(
            service.revoke(&capability(), "premature".to_owned(), bound()),
            Err(PromotionServiceError::Illegal(_))
        ));
    }

    #[test]
    fn an_unknown_capability_is_a_typed_refusal_not_an_evaluation() {
        let ledger = Arc::new(MemoryLedger::new());
        let service = service(&ledger);
        assert!(matches!(
            service.evaluate(&capability(), bound()),
            Err(PromotionServiceError::UnknownCapability { .. })
        ));
        assert!(matches!(
            service.revoke(&capability(), "nothing".to_owned(), bound()),
            Err(PromotionServiceError::UnknownCapability { .. })
        ));
    }

    #[test]
    fn a_zero_minimum_is_refused_at_construction() {
        let ledger = Arc::new(MemoryLedger::new());
        assert!(matches!(
            PromotionService::new(ledger as Arc<_>, 0, Arc::new(FixedClock)),
            Err(PromotionServiceError::InvalidMinimum { min_settlements: 0 })
        ));
    }

    #[test]
    fn observations_alone_never_move_the_status() {
        // The no-bare-set-status shape: seeding memory registers Observed
        // and nothing else; only a legal typed transition record moves the
        // register, and the records that would skip evaluation refuse to
        // compose at all.
        let ledger = Arc::new(MemoryLedger::new());
        seed_full(&ledger);
        let entry = ledger
            .capability(&capability(), bound())
            .expect("reads")
            .expect("registered");
        assert_eq!(entry.status, CapabilityStatus::Observed);
        assert!(
            ledger
                .transitions(&capability(), bound())
                .expect("history")
                .is_empty()
        );
    }
}
