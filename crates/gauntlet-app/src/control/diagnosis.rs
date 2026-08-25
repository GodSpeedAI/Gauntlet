//! `DiagnosticEngine` — spec §15.2 bounded diagnostic closure, plus the
//! §11.5 non-convergence trigger that routes into it.
//!
//! Spec §15.2, quoted where it decides the shape:
//!
//! - "When expected and observed diverge in a way that repeats, do not
//!   patch. Run a bounded diagnostic cycle:
//!   `diagnosis → prediction → cheapest discriminating test → disconfirming
//!   observation → correction`."
//! - "Spend from `diagnostic_budget`. If nothing resolves within it: mark
//!   `INCONCLUSIVE`, quarantine the evidence, stabilize, escalate or defer.
//!   Do not invent certainty."
//! - "Note that six of those sixteen are defects in the *measuring
//!   apparatus*, not the implementation. A loop that only ever diagnoses
//!   'implementation defect' is not diagnosing."
//!
//! §11.5 trigger row: "Non-converging residual set (same residual id N
//! times, `N = GAUNTLET_NONCONVERGENCE_N`, default 3) | Stop correcting;
//! enter diagnosis (§15.2); if diagnosis is inconclusive, escalate."
//!
//! ## The stable semantic key (preregistered concern, now a requirement)
//!
//! The §11.5 text says "same residual id", but residual ids are minted
//! fresh by every comparison — keying on the raw [`ResidualId`] would never
//! see a recurrence. The trigger keys on a semantic PROJECTION instead:
//! `(unit, dimension, class)` PLUS the §13.5 residual identity's
//! expected/observed locator pair — the Unit the residual names, the
//! dimension it was found in, its severity class, and the merge key that
//! makes one residual one residual. Ids are ignored; the projection is
//! what "the same residual again" means, and two DIFFERENT residuals
//! (different observed locators) never accumulate one count (correction
//! C4: the false-positive direction is guarded by construction).
//!
//! ## Purity contract
//!
//! The engine holds no clock and no store. The cycle's one I/O-adjacent
//! act — escalation on an inconclusive outcome — goes through the injected
//! [`HumanEscalation`] port. The caller (slice B3's controller) supplies
//! the evidence facts, the run's [`Budget`] to spend from, and the
//! observation outcome of the cycle's discriminating test; the quarantined
//! evidence's durable persistence is the composition's write, not the
//! engine's.

use std::collections::BTreeMap;

use gauntlet_domain::{Budget, EvidenceRef, MicroUsd, ResidualId, RunId, Severity, UnitId};
use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::human_escalation::{EscalationItem, HumanEscalation, HumanEscalationError};

// ---------------------------------------------------------------------------
// The sixteen failure loci (§15.2)
// ---------------------------------------------------------------------------

/// The §15.2 candidate failure loci, verbatim and complete. "Candidate
/// failure loci — enumerate before choosing."
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FailureLocus {
    BadReference,
    AmbiguousReference,
    WrongObservation,
    ObservationFailure,
    IncorrectComparison,
    CriticError,
    VerifierError,
    RouteSelectionError,
    ImplementationDefect,
    ToolDefect,
    EnvironmentDrift,
    InsufficientPayment,
    MissingCapability,
    IntegrationFailure,
    ArchitecturalDefect,
    GovernanceConflict,
}

impl FailureLocus {
    /// Every locus, in the §15.2 list order.
    pub const ALL: &'static [Self] = &[
        Self::BadReference,
        Self::AmbiguousReference,
        Self::WrongObservation,
        Self::ObservationFailure,
        Self::IncorrectComparison,
        Self::CriticError,
        Self::VerifierError,
        Self::RouteSelectionError,
        Self::ImplementationDefect,
        Self::ToolDefect,
        Self::EnvironmentDrift,
        Self::InsufficientPayment,
        Self::MissingCapability,
        Self::IntegrationFailure,
        Self::ArchitecturalDefect,
        Self::GovernanceConflict,
    ];

    /// The six loci that are defects in the measuring apparatus, not the
    /// implementation (§15.2: "six of those sixteen"). Derived: the
    /// observe → compare → verify chain plus its routing — every stage
    /// whose defect would make a healthy implementation LOOK wrong.
    pub const APPARATUS: &'static [Self] = &[
        Self::WrongObservation,
        Self::ObservationFailure,
        Self::IncorrectComparison,
        Self::CriticError,
        Self::VerifierError,
        Self::RouteSelectionError,
    ];

    /// Whether this locus belongs to the measuring apparatus.
    pub fn is_apparatus(self) -> bool {
        Self::APPARATUS.contains(&self)
    }
}

impl core::fmt::Display for FailureLocus {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let token = match self {
            Self::BadReference => "bad reference",
            Self::AmbiguousReference => "ambiguous reference",
            Self::WrongObservation => "wrong observation",
            Self::ObservationFailure => "observation failure",
            Self::IncorrectComparison => "incorrect comparison",
            Self::CriticError => "critic error",
            Self::VerifierError => "verifier error",
            Self::RouteSelectionError => "route-selection error",
            Self::ImplementationDefect => "implementation defect",
            Self::ToolDefect => "tool defect",
            Self::EnvironmentDrift => "environment drift",
            Self::InsufficientPayment => "insufficient payment",
            Self::MissingCapability => "missing capability",
            Self::IntegrationFailure => "integration failure",
            Self::ArchitecturalDefect => "architectural defect",
            Self::GovernanceConflict => "governance conflict",
        };
        f.write_str(token)
    }
}

// ---------------------------------------------------------------------------
// The §11.5 trigger
// ---------------------------------------------------------------------------

/// The semantic projection one residual is counted under: the Unit it
/// names, the dimension it was found in, its severity class, and the
/// §13.5 merge key's expected/observed LOCATOR PAIR. This — never the raw
/// [`ResidualId`] — is what "the same residual again" means (see the
/// module doc): free-text wording is NOT collapsed (the pair IS the
/// residual's identity — two different observed locators are two
/// different residuals and never accumulate one count).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResidualKey {
    pub unit: UnitId,
    pub dimension: String,
    pub severity: Severity,
    /// The §13.5 merge key's `expected` component.
    pub expected: String,
    /// The §13.5 merge key's `observed_locator` component.
    pub observed: String,
}

/// The severity ordering the key map needs: blocking > major > minor >
/// informational (§5.2's own severity list order). Local to the key's
/// container ordering — no severity comparison leaks into decisions.
fn severity_rank(severity: Severity) -> u8 {
    match severity {
        Severity::Blocking => 3,
        Severity::Major => 2,
        Severity::Minor => 1,
        Severity::Informational => 0,
    }
}

impl Ord for ResidualKey {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        (
            &self.unit,
            self.dimension.as_str(),
            severity_rank(self.severity),
            self.expected.as_str(),
            self.observed.as_str(),
        )
            .cmp(&(
                &other.unit,
                other.dimension.as_str(),
                severity_rank(other.severity),
                other.expected.as_str(),
                other.observed.as_str(),
            ))
    }
}

impl PartialOrd for ResidualKey {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// One observed residual occurrence: its (ignored) fresh id plus the
/// semantic projection the trigger counts under.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecurringResidual {
    pub id: ResidualId,
    pub key: ResidualKey,
}

/// A residual set that has recurred N times — §11.5's "stop correcting;
/// enter diagnosis" signal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Nonconvergence {
    pub key: ResidualKey,
    pub occurrences: u32,
}

/// Counts residual occurrences under their semantic key and fires when a
/// key reaches the configured threshold (§11.5,
/// `GAUNTLET_NONCONVERGENCE_N`, default 3 — the default is configuration,
/// surfaced by [`crate::config`]).
#[derive(Debug)]
pub struct NonconvergenceDetector {
    threshold: u32,
    counts: BTreeMap<ResidualKey, u32>,
}

impl NonconvergenceDetector {
    /// A detector firing at `threshold` occurrences of the same semantic
    /// key. A threshold of zero is refused: a trigger that fires before any
    /// occurrence has been counted is not a recurrence signal.
    pub fn new(threshold: u32) -> Result<Self, DiagnosisError> {
        if threshold == 0 {
            return Err(DiagnosisError::ThresholdZero);
        }
        Ok(Self {
            threshold,
            counts: BTreeMap::new(),
        })
    }

    /// The configured threshold (`GAUNTLET_NONCONVERGENCE_N`).
    pub fn threshold(&self) -> u32 {
        self.threshold
    }

    /// Records one occurrence and returns the non-convergence signal when
    /// the semantic key has reached the threshold. The fresh [`ResidualId`]
    /// is ignored by design (see the module doc): counting keys on the
    /// `(unit, dimension, class)` projection, so fresh ids under the same
    /// projection are the §11.5 recurrence. Idempotent past the threshold:
    /// further occurrences keep returning the signal, never reset it.
    pub fn observe(&mut self, residual: &RecurringResidual) -> Option<Nonconvergence> {
        let count = self.counts.entry(residual.key.clone()).or_insert(0);
        *count += 1;
        (*count >= self.threshold).then(|| Nonconvergence {
            key: residual.key.clone(),
            occurrences: *count,
        })
    }

    /// How many occurrences a key has been counted under — inspectable, so
    /// the run report can say how close anything is to the trigger.
    pub fn occurrences(&self, key: &ResidualKey) -> u32 {
        self.counts.get(key).copied().unwrap_or(0)
    }
}

// ---------------------------------------------------------------------------
// The diagnostic cycle
// ---------------------------------------------------------------------------

/// The per-phase diagnostic spend, charged from the domain `Budget`'s
/// §15.2 allowance (`Budget::charge_diagnostic`). The mapping from the
/// config `diagnostic_budget` scalar to the domain allowance happens at
/// composition (slice B3) — the engine consumes the DOMAIN unit only, never
/// a second one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiagnosticSpend {
    pub tokens: u64,
    pub cost_usd: MicroUsd,
}

/// The typed evidence facts one diagnosis is decided from. Every field maps
/// to one (or two) §15.2 loci; locus selection is a pure function of these
/// — nothing else. The two reference facts outrank everything (§11.5: a
/// mutated reference aborts the run, it is never diagnosed around), the
/// apparatus facts outrank the implementation (§15.2's note), and
/// `implementation_shaped` is consulted LAST.
#[derive(Debug, Clone, Default)]
pub struct DiagnosticEvidence {
    /// The frozen reference's digest moved (§9.4) — bad reference.
    pub reference_mutated: bool,
    /// The reference does not determine an expected value — ambiguous
    /// reference.
    pub reference_ambiguous: bool,
    /// A facet produced values that do not reflect the system (stale or
    /// mis-bound probe) — wrong observation.
    pub observation_wrong: bool,
    /// An instrument could not run at all (§12.3 `Unavailable`) —
    /// observation failure.
    pub observation_unavailable: bool,
    /// The comparator's record is disputed or internally inconsistent
    /// (§13.5) — incorrect comparison.
    pub comparator_disputed: bool,
    /// The critic's output was malformed or self-contradictory — critic
    /// error.
    pub critic_malformed: bool,
    /// A deterministic verifier disagreed with the comparison (§18.4
    /// "verifier disagrees with comparator") — verifier error.
    pub verifier_disagreed: bool,
    /// The dispatch routed work to the wrong role or target —
    /// route-selection error.
    pub route_selection_failed: bool,
    /// A tool failed (§18.4 "tool fails") — tool defect.
    pub tool_failed: bool,
    /// The environment changed mid-run (§18.4) — environment drift.
    pub environment_drifted: bool,
    /// A needed budget dimension is exhausted — insufficient payment.
    pub payment_exhausted: bool,
    /// A required capability has no binding (§8.4) — missing capability.
    pub capability_missing: bool,
    /// Components disagree at their seams — integration failure.
    pub integration_failed: bool,
    /// The architecture itself cannot satisfy the reference —
    /// architectural defect.
    pub architecture_suspect: bool,
    /// Governance blocked what the correction path assumed — governance
    /// conflict.
    pub governance_conflict: bool,
    /// The residual facts themselves point at the implementation (failing
    /// instrument evidence over an otherwise healthy apparatus) —
    /// implementation defect. Consulted LAST on purpose: "a loop that only
    /// ever diagnoses 'implementation defect' is not diagnosing."
    pub implementation_shaped: bool,
    /// The cheapest discriminating test the residual record carried (Annex
    /// B.4 `cheapest_discriminating_test`), if any — recorded in the report
    /// as the cycle's test.
    pub discriminating_test: Option<String>,
    /// The evidence refs to quarantine when the outcome is INCONCLUSIVE
    /// (§15.2 "quarantine the evidence").
    pub quarantined_evidence: Vec<EvidenceRef>,
}

impl DiagnosticEvidence {
    /// The locus this evidence selects, with the facts that selected it —
    /// a pure, documented precedence:
    ///
    /// 1. reference integrity (§11.5 aborts on it — it outranks diagnosis),
    /// 2. the measuring apparatus (§15.2's note),
    /// 3. tooling, environment, payment, capability, integration,
    ///    architecture, governance,
    /// 4. the implementation, last.
    ///
    /// `None` when no fact is set: the apparatus reported nothing, and the
    /// engine refuses to invent a locus.
    pub fn selected_locus(&self) -> Option<(FailureLocus, Vec<String>)> {
        let candidates: [(FailureLocus, bool, String); 16] = [
            (
                FailureLocus::BadReference,
                self.reference_mutated,
                "the frozen reference digest moved".to_owned(),
            ),
            (
                FailureLocus::AmbiguousReference,
                self.reference_ambiguous,
                "the reference does not determine an expected value".to_owned(),
            ),
            (
                FailureLocus::WrongObservation,
                self.observation_wrong,
                "a facet produced values that do not reflect the system".to_owned(),
            ),
            (
                FailureLocus::ObservationFailure,
                self.observation_unavailable,
                "an instrument could not run at all (unavailable facet)".to_owned(),
            ),
            (
                FailureLocus::IncorrectComparison,
                self.comparator_disputed,
                "the comparison record is disputed or inconsistent".to_owned(),
            ),
            (
                FailureLocus::CriticError,
                self.critic_malformed,
                "the critic's output was malformed".to_owned(),
            ),
            (
                FailureLocus::VerifierError,
                self.verifier_disagreed,
                "a deterministic verifier disagreed with the comparison".to_owned(),
            ),
            (
                FailureLocus::RouteSelectionError,
                self.route_selection_failed,
                "work was routed to the wrong role or target".to_owned(),
            ),
            (
                FailureLocus::ToolDefect,
                self.tool_failed,
                "a tool failed".to_owned(),
            ),
            (
                FailureLocus::EnvironmentDrift,
                self.environment_drifted,
                "the environment changed mid-run".to_owned(),
            ),
            (
                FailureLocus::InsufficientPayment,
                self.payment_exhausted,
                "a needed budget dimension is exhausted".to_owned(),
            ),
            (
                FailureLocus::MissingCapability,
                self.capability_missing,
                "a required capability has no binding".to_owned(),
            ),
            (
                FailureLocus::IntegrationFailure,
                self.integration_failed,
                "components disagree at their seams".to_owned(),
            ),
            (
                FailureLocus::ArchitecturalDefect,
                self.architecture_suspect,
                "the architecture cannot satisfy the reference".to_owned(),
            ),
            (
                FailureLocus::GovernanceConflict,
                self.governance_conflict,
                "governance blocked what the path assumed".to_owned(),
            ),
            // Last, the implementation (§15.2: "a loop that only ever
            // diagnoses 'implementation defect' is not diagnosing") — the
            // array's iteration order IS the precedence, so every other
            // hypothesis wins when its fact is set.
            (
                FailureLocus::ImplementationDefect,
                self.implementation_shaped,
                "the residual facts point at the implementation".to_owned(),
            ),
        ];
        candidates
            .into_iter()
            .find(|(_, set, _)| *set)
            .map(|(locus, _, evidence)| (locus, vec![evidence]))
    }
}

/// The outcome of the cycle's ONE disconfirming observation (§15.2): did
/// the cheapest discriminating test disconfirm the locus hypothesis? The
/// composition supplies this from the real instrument run — the engine
/// never generates an observation itself, and never asks twice (G16).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DisconfirmingObservation {
    pub disconfirmed: bool,
}

/// The typed quarantine marker (§15.2 "quarantine the evidence"): what was
/// quarantined, under which locus hypothesis, and why. This is a real
/// record, not a doc note — the composition (slice B3) persists it beside
/// the escalation it travels with; the engine hands it to the
/// [`HumanEscalation`] port inside every inconclusive outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuarantineRecord {
    pub run_id: RunId,
    pub evidence_refs: Vec<EvidenceRef>,
    pub locus: Option<FailureLocus>,
    pub reason: String,
}

/// The terminal outcome of one diagnostic cycle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiagnosisOutcome {
    /// The disconfirming observation DISCONFIRMED the locus hypothesis:
    /// §15.2's cycle terminates in a correction, which the composition
    /// dispatches next (§15.1 selection over the corrected candidate).
    CorrectionIndicated {
        locus: FailureLocus,
        correction_hint: String,
    },
    /// Nothing resolved within the diagnostic budget (or the hypothesis
    /// survived its one disconfirming test): `INCONCLUSIVE`, evidence
    /// quarantined, escalation submitted (§15.2; §11.6 `inconclusive`).
    /// "Do not invent certainty."
    Inconclusive { quarantine: QuarantineRecord },
}

/// What one run of the cycle produced — the full inspectable trail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosisReport {
    /// The locus the evidence selected, when the diagnosis phase completed.
    pub locus: Option<FailureLocus>,
    /// The facts that selected the locus.
    pub selecting_evidence: Vec<String>,
    /// The falsifiable prediction (the locus hypothesis, stated).
    pub prediction: Option<String>,
    /// The cheapest discriminating test the evidence carried, when any.
    pub discriminating_test: Option<String>,
    /// How many of the five phases completed before the cycle ended.
    pub phases_completed: u32,
    /// What the cycle actually booked from the diagnostic allowance.
    pub spend_booked: DiagnosticSpend,
    pub outcome: DiagnosisOutcome,
}

/// The escalation-side context the cycle needs: which run, which evidence
/// ref carries the full context, and the port deadline.
#[derive(Debug, Clone)]
pub struct EscalationContext {
    pub run_id: RunId,
    pub context_ref: EvidenceRef,
    pub deadline: Deadline,
}

/// Diagnosis failures — typed (spec §21.2).
#[derive(Debug, Clone, PartialEq)]
pub enum DiagnosisError {
    /// A non-convergence threshold of zero was configured: the trigger
    /// counts occurrences, and zero cannot be reached by counting.
    ThresholdZero,
    /// The engine was built with an all-zero per-phase spend: diagnosis
    /// must be bounded, and a free cycle is not (§15.2 "spend from
    /// diagnostic_budget").
    SpendZero,
    /// The outcome was INCONCLUSIVE but the escalation port refused the
    /// ticket. The quarantine record travels in the error — §11.6's
    /// `inconclusive` terminal requires both, and neither is dropped.
    EscalationFailed {
        quarantine: QuarantineRecord,
        cause: HumanEscalationError,
    },
}

impl core::fmt::Display for DiagnosisError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::ThresholdZero => write!(
                f,
                "a non-convergence threshold of zero cannot be reached by counting occurrences \
                 (spec 11.5)"
            ),
            Self::SpendZero => write!(
                f,
                "the diagnostic per-phase spend is zero — a diagnostic cycle must cost from the \
                 diagnostic budget to be bounded (spec 15.2)"
            ),
            Self::EscalationFailed { quarantine, cause } => write!(
                f,
                "an inconclusive diagnosis could not be escalated ({cause}); its quarantined \
                 evidence ({} ref(s)) is carried here and must be persisted by the caller \
                 (spec 15.2, 11.6)",
                quarantine.evidence_refs.len()
            ),
        }
    }
}

impl std::error::Error for DiagnosisError {}

/// The five phases of the §15.2 cycle, in order — the state the engine
/// advances through, at most once each, per cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CyclePhase {
    Diagnosis,
    Prediction,
    CheapestDiscriminatingTest,
    DisconfirmingObservation,
    Correction,
}

impl CyclePhase {
    /// Every phase, in cycle order.
    pub const ALL: &'static [Self] = &[
        Self::Diagnosis,
        Self::Prediction,
        Self::CheapestDiscriminatingTest,
        Self::DisconfirmingObservation,
        Self::Correction,
    ];
}

/// The bounded §15.2 diagnostic cycle engine. Stateless between cycles: one
/// call runs exactly one bounded cycle (five phases at most, one
/// disconfirming observation, no re-asking), so boundedness is by
/// construction — there is no API that loops.
#[derive(Debug, Clone)]
pub struct DiagnosticEngine {
    spend: DiagnosticSpend,
}

impl DiagnosticEngine {
    /// An engine charging `spend` from the diagnostic allowance per phase.
    /// An all-zero spend is refused: a free diagnosis is an unbounded one.
    pub fn new(spend: DiagnosticSpend) -> Result<Self, DiagnosisError> {
        if spend.tokens == 0 && spend.cost_usd.get() == 0 {
            return Err(DiagnosisError::SpendZero);
        }
        Ok(Self { spend })
    }

    /// Runs ONE bounded §15.2 cycle over `evidence`:
    ///
    /// 1. `diagnosis` — select the locus (input-driven; no locus ⇒ nothing
    ///    resolves ⇒ INCONCLUSIVE).
    /// 2. `prediction` — state the falsifiable hypothesis (the locus).
    /// 3. `cheapest discriminating test` — the test the evidence carried.
    /// 4. `disconfirming observation` — the caller's one typed observation.
    /// 5. `correction` — only when the hypothesis was disconfirmed.
    ///
    /// Every phase charges `spend` from `budget`'s diagnostic allowance; a
    /// refused charge ends the cycle INCONCLUSIVE with the evidence
    /// quarantined and an escalation submitted. A hypothesis that survives
    /// its one disconfirming observation is NOT a resolution — the cycle
    /// ends INCONCLUSIVE the same way ("do not invent certainty", G16: no
    /// path re-asks until the answer changes).
    pub fn run_cycle(
        &self,
        evidence: &DiagnosticEvidence,
        budget: &mut Budget,
        observation: &DisconfirmingObservation,
        escalation: &dyn HumanEscalation,
        escalation_ctx: &EscalationContext,
    ) -> Result<DiagnosisReport, DiagnosisError> {
        let mut booked = DiagnosticSpend {
            tokens: 0,
            cost_usd: MicroUsd::new(0),
        };
        let mut phases = 0u32;
        let charge = |budget: &mut Budget, booked: &mut DiagnosticSpend| -> bool {
            if budget
                .charge_diagnostic(self.spend.tokens, self.spend.cost_usd)
                .is_err()
            {
                return false;
            }
            booked.tokens += self.spend.tokens;
            booked.cost_usd = MicroUsd::new(booked.cost_usd.get() + self.spend.cost_usd.get());
            true
        };
        let inconclusive = |locus: Option<FailureLocus>,
                            phases: u32,
                            booked: DiagnosticSpend,
                            reason: String|
         -> Result<DiagnosisReport, DiagnosisError> {
            let quarantine = QuarantineRecord {
                run_id: escalation_ctx.run_id.clone(),
                evidence_refs: evidence.quarantined_evidence.clone(),
                locus,
                reason,
            };
            let item = EscalationItem {
                run_id: escalation_ctx.run_id.clone(),
                summary: format!(
                    "inconclusive diagnosis ({phases} of 5 phases, locus {}): evidence \
                     quarantined, operator decision required",
                    locus.map_or_else(|| "unresolved".to_owned(), |l| l.to_string())
                ),
                context: escalation_ctx.context_ref.clone(),
            };
            match escalation.escalate(item, escalation_ctx.deadline) {
                Ok(_) => Ok(DiagnosisReport {
                    locus,
                    selecting_evidence: Vec::new(),
                    prediction: None,
                    discriminating_test: None,
                    phases_completed: phases,
                    spend_booked: booked,
                    outcome: DiagnosisOutcome::Inconclusive { quarantine },
                }),
                Err(cause) => Err(DiagnosisError::EscalationFailed { quarantine, cause }),
            }
        };

        // Phase 1 — diagnosis.
        if !charge(budget, &mut booked) {
            return inconclusive(
                None,
                phases,
                booked,
                "the diagnostic budget was exhausted before a locus could be selected".to_owned(),
            );
        }
        phases += 1;
        let Some((locus, selecting_evidence)) = evidence.selected_locus() else {
            return inconclusive(
                None,
                phases,
                booked,
                "no locus hypothesis is supported by the evidence — nothing resolves, and the \
                 engine does not invent one"
                    .to_owned(),
            );
        };

        // Phase 2 — prediction.
        if !charge(budget, &mut booked) {
            return inconclusive(
                Some(locus),
                phases,
                booked,
                "the diagnostic budget was exhausted before the hypothesis was stated".to_owned(),
            );
        }
        phases += 1;
        let prediction = format!("the divergence is caused by {locus}");

        // Phase 3 — the cheapest discriminating test.
        if !charge(budget, &mut booked) {
            return inconclusive(
                Some(locus),
                phases,
                booked,
                "the diagnostic budget was exhausted before the discriminating test".to_owned(),
            );
        }
        phases += 1;

        // Phase 4 — the one disconfirming observation.
        if !charge(budget, &mut booked) {
            return inconclusive(
                Some(locus),
                phases,
                booked,
                "the diagnostic budget was exhausted before the disconfirming observation"
                    .to_owned(),
            );
        }
        phases += 1;

        if !observation.disconfirmed {
            // The hypothesis survived its one test: not a resolution.
            return inconclusive(
                Some(locus),
                phases,
                booked,
                format!(
                    "the {locus} hypothesis survived its one disconfirming observation — do not \
                     invent certainty, and do not re-ask"
                ),
            );
        }

        // Phase 5 — correction.
        if !charge(budget, &mut booked) {
            return inconclusive(
                Some(locus),
                phases,
                booked,
                "the diagnostic budget was exhausted before the correction could be indicated"
                    .to_owned(),
            );
        }
        phases += 1;
        Ok(DiagnosisReport {
            locus: Some(locus),
            selecting_evidence,
            prediction: Some(prediction),
            discriminating_test: evidence.discriminating_test.clone(),
            phases_completed: phases,
            spend_booked: booked,
            outcome: DiagnosisOutcome::CorrectionIndicated {
                locus,
                correction_hint: format!("correct the {locus} and re-run the discriminating test"),
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gauntlet_domain::{Budget, DiagnosticAllowance, EpochSeconds, Limits};
    use gauntlet_ports::human_escalation::{EscalationTicket, TicketId};
    use std::sync::{Arc, Mutex};

    fn unit_id() -> UnitId {
        UnitId::new("AK-014").expect("valid unit id")
    }

    fn key(dimension: &str) -> ResidualKey {
        ResidualKey {
            unit: unit_id(),
            dimension: dimension.to_owned(),
            severity: Severity::Blocking,
            expected: "the fixture's expected locator text".to_owned(),
            observed: "the fixture's observed locator".to_owned(),
        }
    }

    fn occurrence(id: &str, key: ResidualKey) -> RecurringResidual {
        RecurringResidual {
            id: ResidualId::new(id).expect("valid residual id"),
            key,
        }
    }

    #[test]
    fn different_observed_locators_do_not_accumulate_one_recurrence_count() {
        // Correction C4 / §13.5: the residual identity is `(dimension,
        // expected, observed_locator)` — two DIFFERENT residuals in the
        // same unit+dimension+severity must not merge into one recurrence
        // count (the false-positive direction has no guard otherwise).
        // Threshold 2: one occurrence of residual A, then one occurrence
        // of residual B (same projection, DIFFERENT observed locator)
        // must fire nothing; the SECOND occurrence of A fires.
        let mut detector = NonconvergenceDetector::new(2).expect("valid threshold");
        let mut a = key("functional");
        a.expected = "the report exists at the locator".to_owned();
        a.observed = "evd-aaa exited 1".to_owned();
        let mut b = key("functional");
        b.expected = a.expected.clone();
        b.observed = "evd-bbb exited 1".to_owned();
        assert!(
            detector
                .observe(&occurrence("res-00000a", a.clone()))
                .is_none(),
            "A's first occurrence fires nothing"
        );
        assert!(
            detector.observe(&occurrence("res-00000b", b)).is_none(),
            "B is a DIFFERENT residual (different observed locator): it must not \
             advance A's recurrence count"
        );
        assert!(
            detector
                .observe(&occurrence("res-00000c", a.clone()))
                .is_some(),
            "A's second occurrence is the recurrence"
        );
    }

    #[test]
    fn nonconvergence_triggers_on_the_nth_occurrence_not_before() {
        // §11.5: "same residual id N times, N = GAUNTLET_NONCONVERGENCE_N,
        // default 3". N-1 occurrences fire nothing; the Nth fires.
        let mut detector = NonconvergenceDetector::new(3).expect("valid threshold");
        assert!(
            detector
                .observe(&occurrence("res-000001", key("functional")))
                .is_none()
        );
        assert!(
            detector
                .observe(&occurrence("res-000002", key("functional")))
                .is_none()
        );
        let fired = detector
            .observe(&occurrence("res-000003", key("functional")))
            .expect("the third occurrence fires");
        assert_eq!(fired.occurrences, 3);
        assert_eq!(fired.key, key("functional"));
        // Past the threshold the signal stays on; it never resets.
        let again = detector
            .observe(&occurrence("res-000004", key("functional")))
            .expect("still non-converging");
        assert_eq!(again.occurrences, 4);
        assert_eq!(detector.threshold(), 3);
    }

    #[test]
    fn recurring_residuals_with_fresh_ids_still_trigger() {
        // The preregistered concern, now the requirement: ids are minted
        // fresh per comparison, so a trigger keyed on the raw ResidualId
        // would NEVER fire. Three FRESH ids under the same semantic key
        // (unit, dimension, class) are the §11.5 "same residual N times".
        let mut detector = NonconvergenceDetector::new(3).expect("valid threshold");
        for n in 1..=3 {
            let fresh = format!("res-{n:06}");
            let observed = detector.observe(&occurrence(&fresh, key("functional")));
            if n < 3 {
                assert!(observed.is_none(), "occurrence {n} must not fire");
            } else {
                assert!(
                    observed.is_some(),
                    "the third fresh id under the same key must fire"
                );
            }
        }
    }

    #[test]
    fn different_dimension_or_class_under_the_same_unit_does_not_accumulate() {
        let mut detector = NonconvergenceDetector::new(3).expect("valid threshold");
        for _ in 0..5 {
            let _ = detector.observe(&occurrence("res-000001", key("functional")));
        }
        // A different dimension is a different residual: two occurrences of
        // the security dimension never fire at a threshold of three, and
        // they do not ride on the functional dimension's count.
        assert!(
            detector
                .observe(&occurrence("res-000002", key("security")))
                .is_none()
        );
        assert!(
            detector
                .observe(&occurrence("res-000003", key("security")))
                .is_none()
        );
        assert_eq!(detector.occurrences(&key("security")), 2);
        // A different severity class is a different residual too.
        let mut relaxed = key("functional");
        relaxed.severity = Severity::Minor;
        assert!(
            detector
                .observe(&occurrence("res-000004", relaxed.clone()))
                .is_none()
        );
        assert!(
            detector
                .observe(&occurrence("res-000005", relaxed.clone()))
                .is_none()
        );
        assert_eq!(detector.occurrences(&relaxed), 2);
    }

    #[test]
    fn a_zero_threshold_is_refused() {
        assert!(matches!(
            NonconvergenceDetector::new(0),
            Err(DiagnosisError::ThresholdZero)
        ));
    }

    // -------------------------------------------------------------------
    // Locus selection
    // -------------------------------------------------------------------

    #[test]
    fn a_measuring_apparatus_input_does_not_route_to_implementation_defect() {
        // §15.2: "six of those sixteen are defects in the measuring
        // apparatus, not the implementation. A loop that only ever
        // diagnoses 'implementation defect' is not diagnosing."
        let evidence = DiagnosticEvidence {
            verifier_disagreed: true,
            ..DiagnosticEvidence::default()
        };
        let (locus, why) = evidence.selected_locus().expect("a locus is selected");
        assert_eq!(locus, FailureLocus::VerifierError);
        assert!(!why.is_empty(), "the selecting evidence is recorded");
        // Even when the facts ALSO look implementation-shaped, the
        // apparatus outranks the implementation.
        let both = DiagnosticEvidence {
            verifier_disagreed: true,
            implementation_shaped: true,
            ..DiagnosticEvidence::default()
        };
        assert_eq!(
            both.selected_locus().expect("locus").0,
            FailureLocus::VerifierError
        );
        // And every apparatus locus beats it.
        for (locus, evidence) in [
            (
                FailureLocus::WrongObservation,
                DiagnosticEvidence {
                    observation_wrong: true,
                    implementation_shaped: true,
                    ..DiagnosticEvidence::default()
                },
            ),
            (
                FailureLocus::ObservationFailure,
                DiagnosticEvidence {
                    observation_unavailable: true,
                    implementation_shaped: true,
                    ..DiagnosticEvidence::default()
                },
            ),
            (
                FailureLocus::IncorrectComparison,
                DiagnosticEvidence {
                    comparator_disputed: true,
                    implementation_shaped: true,
                    ..DiagnosticEvidence::default()
                },
            ),
            (
                FailureLocus::CriticError,
                DiagnosticEvidence {
                    critic_malformed: true,
                    implementation_shaped: true,
                    ..DiagnosticEvidence::default()
                },
            ),
            (
                FailureLocus::RouteSelectionError,
                DiagnosticEvidence {
                    route_selection_failed: true,
                    implementation_shaped: true,
                    ..DiagnosticEvidence::default()
                },
            ),
        ] {
            assert_eq!(
                evidence.selected_locus().expect("locus").0,
                locus,
                "the apparatus locus must outrank implementation defect"
            );
            assert!(locus.is_apparatus());
        }
    }

    #[test]
    fn every_non_implementation_locus_outranks_implementation_defect() {
        // The documented precedence puts the implementation LAST: "a loop
        // that only ever diagnoses 'implementation defect' is not
        // diagnosing" (§15.2) — when the evidence carries ANY other
        // hypothesis, that hypothesis wins. The apparatus six are pinned
        // above; this pins the remaining seven the same way.
        for (locus, evidence) in [
            (
                FailureLocus::ToolDefect,
                DiagnosticEvidence {
                    tool_failed: true,
                    implementation_shaped: true,
                    ..DiagnosticEvidence::default()
                },
            ),
            (
                FailureLocus::EnvironmentDrift,
                DiagnosticEvidence {
                    environment_drifted: true,
                    implementation_shaped: true,
                    ..DiagnosticEvidence::default()
                },
            ),
            (
                FailureLocus::InsufficientPayment,
                DiagnosticEvidence {
                    payment_exhausted: true,
                    implementation_shaped: true,
                    ..DiagnosticEvidence::default()
                },
            ),
            (
                FailureLocus::MissingCapability,
                DiagnosticEvidence {
                    capability_missing: true,
                    implementation_shaped: true,
                    ..DiagnosticEvidence::default()
                },
            ),
            (
                FailureLocus::IntegrationFailure,
                DiagnosticEvidence {
                    integration_failed: true,
                    implementation_shaped: true,
                    ..DiagnosticEvidence::default()
                },
            ),
            (
                FailureLocus::ArchitecturalDefect,
                DiagnosticEvidence {
                    architecture_suspect: true,
                    implementation_shaped: true,
                    ..DiagnosticEvidence::default()
                },
            ),
            (
                FailureLocus::GovernanceConflict,
                DiagnosticEvidence {
                    governance_conflict: true,
                    implementation_shaped: true,
                    ..DiagnosticEvidence::default()
                },
            ),
        ] {
            assert_eq!(
                evidence.selected_locus().expect("locus").0,
                locus,
                "every non-implementation locus outranks implementation defect"
            );
        }
    }

    #[test]
    fn reference_integrity_outranks_every_hypothesis_and_empty_evidence_selects_none() {
        // §11.5's reference-mutation row aborts the run — the bad-reference
        // locus outranks every diagnostic hypothesis, and everything
        // outranks implementation defect.
        let evidence = DiagnosticEvidence {
            reference_mutated: true,
            verifier_disagreed: true,
            implementation_shaped: true,
            ..DiagnosticEvidence::default()
        };
        assert_eq!(
            evidence.selected_locus().expect("locus").0,
            FailureLocus::BadReference
        );
        // No facts set: no locus is invented.
        assert!(DiagnosticEvidence::default().selected_locus().is_none());
        // Alone, implementation-shaped facts DO route there — the honest
        // case still works, it is just never the FIRST explanation.
        let honest = DiagnosticEvidence {
            implementation_shaped: true,
            ..DiagnosticEvidence::default()
        };
        assert_eq!(
            honest.selected_locus().expect("locus").0,
            FailureLocus::ImplementationDefect
        );
    }

    #[test]
    fn the_locus_vocabulary_is_the_spec_sixteen() {
        assert_eq!(FailureLocus::ALL.len(), 16, "sixteen loci, verbatim");
        assert_eq!(
            FailureLocus::APPARATUS.len(),
            6,
            "six of them are apparatus"
        );
        assert!(
            FailureLocus::ALL
                .iter()
                .all(|locus| locus.is_apparatus() || !FailureLocus::APPARATUS.contains(locus))
        );
        assert_eq!(
            FailureLocus::ImplementationDefect.to_string(),
            "implementation defect"
        );
    }

    // -------------------------------------------------------------------
    // The bounded cycle
    // -------------------------------------------------------------------

    // Named test facts — §8.1 literal discipline: duration- and
    // cost-shaped values are never inline numbers, in tests as in source.
    const RUN_ROUNDS: u32 = 8;
    const RUN_PARALLEL: u32 = 2;
    const RUN_DEPTH: u32 = 2;
    const RUN_SPAN: u64 = 3600;
    const RUN_TOKENS: u64 = 1000;
    const RUN_COST: u64 = 500;
    const RESERVE_PCT: u8 = 20;
    const DIAG_COST: u64 = 100;
    const TICK_EPOCH: u64 = 1_700_000_100;
    const CTX_EPOCH: u64 = 1_700_000_060;

    fn spend() -> DiagnosticSpend {
        DiagnosticSpend {
            tokens: 10,
            cost_usd: MicroUsd::new(1),
        }
    }

    fn budget(tokens: u64) -> Budget {
        Budget::new(
            Limits::new(
                RUN_ROUNDS,
                RUN_PARALLEL,
                RUN_DEPTH,
                RUN_SPAN,
                RUN_TOKENS,
                MicroUsd::new(RUN_COST),
            )
            .expect("usable ceilings"),
            RESERVE_PCT,
            DiagnosticAllowance::new(tokens, MicroUsd::new(DIAG_COST)),
        )
        .expect("usable budget")
    }

    /// A recording escalation fake: accepts the item, hands back a ticket.
    #[derive(Default, Clone)]
    struct RecordingEscalation {
        items: Arc<Mutex<Vec<EscalationItem>>>,
        refuse: bool,
    }

    impl HumanEscalation for RecordingEscalation {
        fn escalate(
            &self,
            item: EscalationItem,
            _deadline: Deadline,
        ) -> Result<EscalationTicket, HumanEscalationError> {
            if self.refuse {
                return Err(HumanEscalationError::SubmitFailed {
                    detail: "the fake refuses".to_owned(),
                });
            }
            self.items.lock().expect("not poisoned").push(item);
            Ok(EscalationTicket::new(
                TicketId::new("esc-000001").expect("valid ticket id"),
                EpochSeconds::new(TICK_EPOCH),
            ))
        }

        fn poll(
            &self,
            _ticket: &EscalationTicket,
            _deadline: Deadline,
        ) -> Result<gauntlet_ports::human_escalation::EscalationState, HumanEscalationError>
        {
            Ok(gauntlet_ports::human_escalation::EscalationState::Open)
        }
    }

    fn ctx() -> EscalationContext {
        EscalationContext {
            run_id: RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("valid run id"),
            context_ref: EvidenceRef::new("evd-diagnosis-0001").expect("valid ref"),
            deadline: Deadline::at_epoch(EpochSeconds::new(CTX_EPOCH)),
        }
    }

    #[test]
    fn a_disconfirmed_hypothesis_completes_the_cycle_bounded() {
        let engine = DiagnosticEngine::new(spend()).expect("valid spend");
        let mut run_budget = budget(200);
        let evidence = DiagnosticEvidence {
            verifier_disagreed: true,
            discriminating_test: Some("re-run the deterministic verifier alone".to_owned()),
            quarantined_evidence: vec![EvidenceRef::new("evd-q-1").expect("valid ref")],
            ..DiagnosticEvidence::default()
        };
        let observation = DisconfirmingObservation { disconfirmed: true };
        let escalation = RecordingEscalation::default();
        let report = engine
            .run_cycle(
                &evidence,
                &mut run_budget,
                &observation,
                &escalation,
                &ctx(),
            )
            .expect("the cycle resolves");
        assert_eq!(report.locus, Some(FailureLocus::VerifierError));
        assert_eq!(report.phases_completed, 5);
        assert_eq!(
            report.spend_booked.tokens, 50,
            "five phases, one spend each"
        );
        assert!(matches!(
            report.outcome,
            DiagnosisOutcome::CorrectionIndicated { .. }
        ));
        // Bounded by construction: nothing was escalated, and the booking is
        // exactly phases × spend.
        assert!(escalation.items.lock().expect("not poisoned").is_empty());
        assert_eq!(
            report.spend_booked,
            DiagnosticSpend {
                tokens: 5 * spend().tokens,
                cost_usd: MicroUsd::new(5)
            }
        );
    }

    #[test]
    fn budget_exhaustion_yields_inconclusive_quarantine_and_escalation() {
        // §15.2: "If nothing resolves within it: mark INCONCLUSIVE,
        // quarantine the evidence, stabilize, escalate or defer." Allowance
        // for two phases only.
        let engine = DiagnosticEngine::new(spend()).expect("valid spend");
        let mut run_budget = budget(2 * spend().tokens);
        let evidence = DiagnosticEvidence {
            verifier_disagreed: true,
            quarantined_evidence: vec![
                EvidenceRef::new("evd-q-1").expect("valid ref"),
                EvidenceRef::new("evd-q-2").expect("valid ref"),
            ],
            ..DiagnosticEvidence::default()
        };
        let observation = DisconfirmingObservation { disconfirmed: true };
        let escalation = RecordingEscalation::default();
        let report = engine
            .run_cycle(
                &evidence,
                &mut run_budget,
                &observation,
                &escalation,
                &ctx(),
            )
            .expect("the inconclusive outcome is a result, not an error");
        assert_eq!(report.phases_completed, 2);
        assert_eq!(report.spend_booked.tokens, 20, "only the phases that ran");
        let DiagnosisOutcome::Inconclusive { quarantine } = &report.outcome else {
            panic!("an exhausted diagnostic budget is INCONCLUSIVE, never a correction");
        };
        assert_eq!(
            quarantine.evidence_refs.len(),
            2,
            "the evidence is quarantined"
        );
        assert_eq!(quarantine.locus, Some(FailureLocus::VerifierError));
        assert!(
            quarantine
                .reason
                .contains("diagnostic budget was exhausted")
        );
        // The escalation item exists, names INCONCLUSIVE, and carries the
        // context evidence ref.
        let items = escalation.items.lock().expect("not poisoned");
        let item = items.first().expect("the escalation was submitted");
        assert!(item.summary.contains("inconclusive"));
        assert_eq!(item.context, ctx().context_ref);
        // Forward spend was never touched — diagnosis never spends the
        // forward budget or the reserve.
        assert_eq!(run_budget.spent().tokens(), 0);
    }

    #[test]
    fn a_surviving_hypothesis_is_inconclusive_never_re_asked() {
        // G16 / §15.2 "do not invent certainty": the ONE disconfirming
        // observation did not disconfirm. The cycle ends INCONCLUSIVE —
        // there is no second observation, no re-ask.
        let engine = DiagnosticEngine::new(spend()).expect("valid spend");
        let mut run_budget = budget(200);
        let evidence = DiagnosticEvidence {
            critic_malformed: true,
            quarantined_evidence: vec![EvidenceRef::new("evd-q-1").expect("valid ref")],
            ..DiagnosticEvidence::default()
        };
        let observation = DisconfirmingObservation {
            disconfirmed: false,
        };
        let escalation = RecordingEscalation::default();
        let report = engine
            .run_cycle(
                &evidence,
                &mut run_budget,
                &observation,
                &escalation,
                &ctx(),
            )
            .expect("resolves to an outcome");
        assert_eq!(report.phases_completed, 4, "the correction phase never ran");
        assert!(matches!(
            report.outcome,
            DiagnosisOutcome::Inconclusive { .. }
        ));
        assert_eq!(escalation.items.lock().expect("not poisoned").len(), 1);
    }

    #[test]
    fn evidence_with_no_hypothesis_is_inconclusive_without_inventing_one() {
        let engine = DiagnosticEngine::new(spend()).expect("valid spend");
        let mut run_budget = budget(200);
        let observation = DisconfirmingObservation { disconfirmed: true };
        let escalation = RecordingEscalation::default();
        let report = engine
            .run_cycle(
                &DiagnosticEvidence::default(),
                &mut run_budget,
                &observation,
                &escalation,
                &ctx(),
            )
            .expect("resolves to an outcome");
        assert_eq!(report.phases_completed, 1);
        let DiagnosisOutcome::Inconclusive { quarantine } = &report.outcome else {
            panic!("no evidence means no invented certainty");
        };
        assert!(quarantine.locus.is_none());
    }

    #[test]
    fn an_escalation_refusal_carries_the_quarantine_in_the_error() {
        // §11.6: the inconclusive terminal requires the quarantine AND the
        // ticket; when the port refuses, neither fact is dropped.
        let engine = DiagnosticEngine::new(spend()).expect("valid spend");
        let mut run_budget = budget(200);
        let evidence = DiagnosticEvidence {
            verifier_disagreed: true,
            quarantined_evidence: vec![EvidenceRef::new("evd-q-1").expect("valid ref")],
            ..DiagnosticEvidence::default()
        };
        let observation = DisconfirmingObservation {
            disconfirmed: false,
        };
        let refusing = RecordingEscalation {
            refuse: true,
            ..RecordingEscalation::default()
        };
        let error = engine
            .run_cycle(&evidence, &mut run_budget, &observation, &refusing, &ctx())
            .expect_err("the escalation refusal surfaces");
        assert!(matches!(
            &error,
            DiagnosisError::EscalationFailed { quarantine, .. }
                if quarantine.evidence_refs.len() == 1
        ));
    }

    #[test]
    fn a_zero_spend_engine_is_refused_and_the_cycle_is_forward_only() {
        assert!(matches!(
            DiagnosticEngine::new(DiagnosticSpend {
                tokens: 0,
                cost_usd: MicroUsd::new(0),
            }),
            Err(DiagnosisError::SpendZero)
        ));
        // The phase list is the cycle: five phases, in order, once each —
        // the engine's one-call shape has no loop to unbound.
        assert_eq!(CyclePhase::ALL.len(), 5);
        assert_eq!(CyclePhase::ALL[0], CyclePhase::Diagnosis);
        assert_eq!(CyclePhase::ALL[4], CyclePhase::Correction);
    }
}
