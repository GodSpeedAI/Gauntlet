//! The `RevisionLedger` shared contract suite (spec §19.3/§19.4; plan
//! Task 23, charter D2).
//!
//! Pinned semantics: proposals are idempotent on the declared revision id
//! (same revision re-admitted with agreeing facts is a no-op; a
//! disagreeing re-admission is the typed conflict, the stored proposal
//! standing); decisions are first-class records whose `claimed` state must
//! agree with the DERIVED decision and whose move past a final state is
//! uncomposable; verdicts refuse to compose with empty reasons; evaluation
//! records are idempotent on the run-set identity; restores are
//! append-only, refuse a never-promoted revision, and flip the derived
//! entry to promoted-but-rolled-back.

use gauntlet_domain::{Digest, EpochSeconds, EvidenceRef, RevisionId, RunId};

use crate::contract::ContractFinding;
use crate::deadline::Deadline;
use crate::revision_ledger::{
    DecisionKind, EvaluationRecord, GateDecision, GateRefusal, RestoreRecord,
    RevisionDecisionRecord, RevisionEntry, RevisionLedger, RevisionLedgerError,
    RevisionPromotionEvidence, RevisionProposal, RunMetrics, SettlementFact,
};

/// The suite's name in findings.
pub const SUITE: &str = "revision_ledger";

const BASE_INSTANT: u64 = 1_700_000_000;
/// The fixture's payment figures (micro-dollars and the fixed-point
/// per-10 000 forms the gate's evidence records carry).
const FIXTURE_TOKENS: u64 = 1_000;
const FIXTURE_PAYMENT: u64 = 2_333;
const FIGURE_CONTROL: u64 = 10_000;
const FIGURE_TREATMENT: u64 = 6_666;
const PAYMENT_CONTROL: u64 = 10_000;
const PAYMENT_TREATMENT: u64 = 10_000;
/// The suite's bound span, on its own line (numeric literals never share a
/// line with a duration-shaped identifier).
const BOUND_SPAN: u64 = 90;

/// Constructs the ledger under test. `build` must return an ISOLATED, EMPTY
/// instance.
pub trait RevisionLedgerFactory {
    fn label(&self) -> &'static str;

    fn build(&self) -> Result<Box<dyn RevisionLedger>, String>;
}

fn finding(case: &'static str, rule: &'static str, detail: String) -> ContractFinding {
    ContractFinding::new(SUITE, case, rule, detail)
}

fn bound() -> Deadline {
    Deadline::at_epoch(EpochSeconds::new(BASE_INSTANT + BOUND_SPAN))
}

fn revision(sequence: usize) -> RevisionId {
    RevisionId::new(&format!("rev-suite-{sequence:04}")).expect("suite revision id")
}

fn run(sequence: usize) -> RunId {
    // 26-char Crockford form; the last char varies over the suite's runs
    // (hex digits 0-7 are Crockford characters).
    RunId::new(&format!("01ARZ3NDEKTSV4RRFFQ69G5FA{sequence:X}")).expect("suite run id is a ULID")
}

fn ref_of(token: &str) -> EvidenceRef {
    EvidenceRef::new(token).expect("suite evidence ref")
}

fn digest() -> Digest {
    Digest::new("sha256", "0011223344556677").expect("suite digest")
}

/// A second, DIFFERENT frozen reference digest — the moved-ruler world the
/// four-arm digest case (COR-2, R1) composes from.
fn moved_digest() -> Digest {
    Digest::new("sha256", "ffeeddccbbaa9988").expect("moved digest")
}

/// The suite's proposal fixture.
fn proposal(sequence: usize) -> RevisionProposal {
    RevisionProposal::new(
        revision(sequence),
        "correction.selection".to_owned(),
        "correction loops burn two rounds".to_owned(),
        vec![ref_of("evd-suite-obs")],
        "one fewer correction round".to_owned(),
        vec!["a residual may re-open".to_owned()],
        ref_of("evd-suite-diff"),
        "held-out set replay".to_owned(),
        "regression set replay".to_owned(),
        ref_of("evd-suite-rollback"),
        EpochSeconds::new(BASE_INSTANT + sequence as u64),
        Some(run(sequence)),
        "the suite's admitted proposal".to_owned(),
    )
}

/// The suite's metrics fixture (two deterministic settlements).
fn metrics() -> RunMetrics {
    RunMetrics {
        iterations: 2,
        payment_tokens: FIXTURE_TOKENS,
        payment_micro_usd: FIXTURE_PAYMENT,
        settlements: vec![
            SettlementFact {
                unit: "AK-9001".to_owned(),
                instrument_deterministic: Some(true),
                occurred_at: Some(EpochSeconds::new(BASE_INSTANT + 1)),
            },
            SettlementFact {
                unit: "AK-9002".to_owned(),
                instrument_deterministic: Some(true),
                occurred_at: Some(EpochSeconds::new(BASE_INSTANT + 2)),
            },
        ],
        dispatched_units: vec!["AK-9001".to_owned(), "AK-9002".to_owned()],
        settled_terminal: true,
    }
}

/// The suite's evaluation fixture: all four arms the same run prefix,
/// discriminated by sequence. The protected set is deliberately NON-empty
/// so the idempotency agreement's protected-set half has something to
/// disagree with (COR-2, R4).
fn evaluation(sequence: usize) -> EvaluationRecord {
    EvaluationRecord {
        revision: revision(sequence),
        metric: "iterations_per_settlement".to_owned(),
        protected: vec!["cap-suite".to_owned()],
        held_out_control: run(sequence),
        held_out_treatment: run(sequence + 1),
        regression_control: run(sequence + 2),
        regression_treatment: run(sequence + 3),
        held_out_treatment_parent: run(sequence),
        regression_treatment_parent: run(sequence + 2),
        control_reference_digest: digest(),
        treatment_reference_digest: digest(),
        regression_control_reference_digest: digest(),
        regression_treatment_reference_digest: digest(),
        control_held_out_metrics: metrics(),
        treatment_held_out_metrics: metrics(),
        control_regression_metrics: metrics(),
        treatment_regression_metrics: metrics(),
        at: EpochSeconds::new(BASE_INSTANT + 10),
    }
}

/// The suite's promotion-evidence fixture — every condition's figure.
fn promotion_evidence() -> RevisionPromotionEvidence {
    RevisionPromotionEvidence {
        metric: "iterations_per_settlement".to_owned(),
        control_figure: FIGURE_CONTROL,
        treatment_figure: FIGURE_TREATMENT,
        margin_bps: 0,
        control_payment_figure: PAYMENT_CONTROL,
        treatment_payment_figure: PAYMENT_TREATMENT,
        unit_mix_preserved: true,
        instrument_series_non_weaker: true,
        control_reference_digest: digest().to_string(),
        treatment_reference_digest: digest().to_string(),
        regression_units_control_settled: 2,
        regression_units_treatment_settled: 2,
        protected_capabilities: Vec::new(),
    }
}

/// Case: proposals are idempotent on the declared revision id — the same
/// revision re-admitted with agreeing facts appends nothing, and a
/// disagreeing re-admission is the typed conflict with the stored proposal
/// standing.
fn proposals_are_idempotent_on_revision(
    factory: &dyn RevisionLedgerFactory,
) -> Vec<ContractFinding> {
    const CASE: &str = "proposals_are_idempotent_on_revision";
    const RULE: &str = "spec 19.3 via Task 23 D2: proposal admissions are idempotent on the \
         declared revision id — the same revision re-admitted with agreeing facts admits \
         nothing, and a disagreeing re-admission is the typed conflict with the stored \
         proposal standing";
    let mut findings = Vec::new();
    let Ok(ledger) = factory.build() else {
        return vec![finding(
            CASE,
            RULE,
            "the factory could not build a ledger for the case".to_owned(),
        )];
    };
    let candidate = proposal(0);
    let receipt = match ledger.admit_proposal(candidate.clone(), bound()) {
        Ok(receipt) => receipt,
        Err(error) => {
            return vec![finding(
                CASE,
                RULE,
                format!("the first admission failed: {error}"),
            )];
        }
    };
    if receipt.idempotent {
        findings.push(finding(
            CASE,
            RULE,
            "the FIRST admission of a fresh revision reported itself idempotent".to_owned(),
        ));
    }
    match ledger.admit_proposal(candidate.clone(), bound()) {
        Ok(admission) if admission.idempotent => {}
        Ok(_) => findings.push(finding(
            CASE,
            RULE,
            "re-admitting the same revision with agreeing facts admitted a second proposal"
                .to_owned(),
        )),
        Err(error) => findings.push(finding(
            CASE,
            RULE,
            format!("the agreeing re-admission was refused: {error}"),
        )),
    }
    // The SAME document re-admitted at a LATER admission instant (COR-1,
    // critic M1): the instant is admission provenance, not a declared
    // fact — the content-derived revision is the same, so this is the
    // idempotent no-op, never the typed conflict. (The moving-clock
    // production admission stamps every admission with the current
    // instant; an idempotency that rides the instant fails on its
    // production path.)
    let later_same_facts = RevisionProposal::new(
        revision(0),
        "correction.selection".to_owned(),
        "correction loops burn two rounds".to_owned(),
        vec![ref_of("evd-suite-obs")],
        "one fewer correction round".to_owned(),
        vec!["a residual may re-open".to_owned()],
        ref_of("evd-suite-diff"),
        "held-out set replay".to_owned(),
        "regression set replay".to_owned(),
        ref_of("evd-suite-rollback"),
        EpochSeconds::new(BASE_INSTANT + 500),
        Some(run(0)),
        "the suite's admitted proposal".to_owned(),
    );
    match ledger.admit_proposal(later_same_facts, bound()) {
        Ok(admission) if admission.idempotent => {}
        Ok(_) => findings.push(finding(
            CASE,
            RULE,
            "the same declared facts at a later admission instant admitted a second proposal"
                .to_owned(),
        )),
        Err(error) => findings.push(finding(
            CASE,
            RULE,
            format!(
                "the same facts at a later admission instant were refused: {error} — the \
                 admission instant is provenance, not a declared fact"
            ),
        )),
    }
    // Same revision, disagreeing facts: the typed conflict, row standing.
    let disagreeing = RevisionProposal::new(
        revision(0),
        "correction.selection".to_owned(),
        "a different claimed problem".to_owned(),
        Vec::new(),
        "a different prediction".to_owned(),
        Vec::new(),
        ref_of("evd-suite-diff-2"),
        "a different test plan".to_owned(),
        "a different held-out plan".to_owned(),
        ref_of("evd-suite-rollback-2"),
        EpochSeconds::new(BASE_INSTANT + 1),
        None,
        "a different detail".to_owned(),
    );
    match ledger.admit_proposal(disagreeing, bound()) {
        Err(RevisionLedgerError::ProposalConflict { .. }) => {}
        Err(other) => findings.push(finding(
            CASE,
            RULE,
            format!("the disagreeing re-admission failed for the wrong reason: {other}"),
        )),
        Ok(_) => findings.push(finding(
            CASE,
            RULE,
            "a disagreeing re-admission of a recorded revision was accepted — a later \
             writer displaced the admitted proposal"
                .to_owned(),
        )),
    }
    match ledger.proposal(&revision(0), bound()) {
        Ok(Some(stored)) if stored.claimed_problem() == "correction loops burn two rounds" => {}
        Ok(Some(_)) => findings.push(finding(
            CASE,
            RULE,
            "the stored proposal was displaced by the disagreeing re-admission".to_owned(),
        )),
        Ok(None) => findings.push(finding(
            CASE,
            RULE,
            "the stored proposal vanished after the conflict".to_owned(),
        )),
        Err(error) => findings.push(finding(
            CASE,
            RULE,
            format!("the read-back after the conflict failed: {error}"),
        )),
    }
    findings
}

/// Case: proposals read back EXACTLY as admitted, every declared field
/// intact — the record the gate adjudicates is the record the optimizer
/// declared.
fn proposals_read_back_exactly_as_admitted(
    factory: &dyn RevisionLedgerFactory,
) -> Vec<ContractFinding> {
    const CASE: &str = "proposals_read_back_exactly_as_admitted";
    const RULE: &str = "spec 19.3 via Task 23 D2: the ledger replays every admitted proposal \
         exactly as admitted, every declared field intact";
    let mut findings = Vec::new();
    let Ok(ledger) = factory.build() else {
        return vec![finding(
            CASE,
            RULE,
            "the factory could not build a ledger for the case".to_owned(),
        )];
    };
    let admitted = proposal(1);
    if let Err(error) = ledger.admit_proposal(admitted.clone(), bound()) {
        return vec![finding(
            CASE,
            RULE,
            format!("the admission failed: {error}"),
        )];
    }
    match ledger.proposal(&revision(1), bound()) {
        Ok(Some(stored)) if stored == admitted.clone() => {}
        Ok(Some(stored)) => findings.push(finding(
            CASE,
            RULE,
            format!(
                "the proposal does not read back as admitted: target {:?} vs {:?}",
                stored.target(),
                admitted.target()
            ),
        )),
        Ok(None) => findings.push(finding(
            CASE,
            RULE,
            "the admitted proposal is not on the ledger".to_owned(),
        )),
        Err(error) => findings.push(finding(
            CASE,
            RULE,
            format!("the read-back failed: {error}"),
        )),
    }
    // An unknown revision reads absent, never an error.
    match ledger.proposal(&revision(9), bound()) {
        Ok(None) => {}
        Ok(Some(_)) => findings.push(finding(
            CASE,
            RULE,
            "an unknown revision read as present".to_owned(),
        )),
        Err(error) => findings.push(finding(
            CASE,
            RULE,
            format!("reading an unknown revision failed: {error}"),
        )),
    }
    findings
}

/// Case: evaluations are idempotent on the run-set identity and read back
/// in order; an unknown revision refuses.
fn evaluations_are_idempotent_and_rooted(
    factory: &dyn RevisionLedgerFactory,
) -> Vec<ContractFinding> {
    const CASE: &str = "evaluations_are_idempotent_and_rooted";
    const RULE: &str = "spec 19.4 via Task 23 D2: evaluation records are idempotent on the \
         run-set identity, refuse an unknown revision, and read back in append order";
    let mut findings = Vec::new();
    let Ok(ledger) = factory.build() else {
        return vec![finding(
            CASE,
            RULE,
            "the factory could not build a ledger for the case".to_owned(),
        )];
    };
    match ledger.append_evaluation(evaluation(2), bound()) {
        Err(RevisionLedgerError::UnknownRevision { .. }) => {}
        Err(other) => findings.push(finding(
            CASE,
            RULE,
            format!("the unknown-revision evaluation failed for the wrong reason: {other}"),
        )),
        Ok(_) => findings.push(finding(
            CASE,
            RULE,
            "an evaluation appended for a revision with no admitted proposal".to_owned(),
        )),
    }
    if let Err(error) = ledger.admit_proposal(proposal(2), bound()) {
        return vec![finding(
            CASE,
            RULE,
            format!("the admission failed: {error}"),
        )];
    }
    match ledger.append_evaluation(evaluation(2), bound()) {
        Ok(appended) if !appended.idempotent => {}
        Ok(_) => findings.push(finding(
            CASE,
            RULE,
            "the FIRST evaluation append reported itself idempotent".to_owned(),
        )),
        Err(error) => {
            return vec![finding(
                CASE,
                RULE,
                format!("the evaluation append failed: {error}"),
            )];
        }
    }
    match ledger.append_evaluation(evaluation(2), bound()) {
        Ok(appended) if appended.idempotent => {}
        Ok(_) => findings.push(finding(
            CASE,
            RULE,
            "re-appending the same run-set with agreeing facts appended a second record".to_owned(),
        )),
        Err(error) => findings.push(finding(
            CASE,
            RULE,
            format!("the agreeing re-append was refused: {error}"),
        )),
    }
    // The same run-set with DISAGREEING figures: the typed conflict.
    let mut disagreeing = evaluation(2);
    disagreeing.control_held_out_metrics.iterations = 99;
    match ledger.append_evaluation(disagreeing, bound()) {
        Err(RevisionLedgerError::EvaluationConflict { .. }) => {}
        Err(other) => findings.push(finding(
            CASE,
            RULE,
            format!("the disagreeing re-append failed for the wrong reason: {other}"),
        )),
        Ok(_) => findings.push(finding(
            CASE,
            RULE,
            "a disagreeing re-append of a recorded run-set was accepted — a later writer \
             displaced recorded evaluation evidence"
                .to_owned(),
        )),
    }
    // COR-2 (R4, falsifier probe C): the evaluation's DECLARED facts — the
    // metric and the protected set — are inside the idempotency
    // agreement. A re-append of the same run-set with a different declared
    // metric is the typed conflict with the stored record standing, never
    // the idempotent no-op that silently keeps the ORIGINAL declaration.
    let mut redeclared_metric = evaluation(2);
    redeclared_metric.metric = "payment_per_settlement".to_owned();
    match ledger.append_evaluation(redeclared_metric, bound()) {
        Err(RevisionLedgerError::EvaluationConflict { .. }) => {}
        Err(other) => findings.push(finding(
            CASE,
            RULE,
            format!("the re-declared-metric re-append failed for the wrong reason: {other}"),
        )),
        Ok(appended) if appended.idempotent => findings.push(finding(
            CASE,
            RULE,
            "a re-append with a DIFFERENT declared metric was swallowed as the idempotent \
             no-op — the evaluation's declared metric sits outside the agreement"
                .to_owned(),
        )),
        Ok(_) => findings.push(finding(
            CASE,
            RULE,
            "a re-append with a different declared metric appended a second record".to_owned(),
        )),
    }
    // An EMPTIER protected set on the same run-set disagrees the same way.
    let mut emptier_protected = evaluation(2);
    emptier_protected.protected = Vec::new();
    match ledger.append_evaluation(emptier_protected, bound()) {
        Err(RevisionLedgerError::EvaluationConflict { .. }) => {}
        Err(other) => findings.push(finding(
            CASE,
            RULE,
            format!("the emptied-protected re-append failed for the wrong reason: {other}"),
        )),
        Ok(appended) if appended.idempotent => findings.push(finding(
            CASE,
            RULE,
            "a re-append with an EMPTIER protected set was swallowed as the idempotent \
             no-op — the declared protected set sits outside the agreement"
                .to_owned(),
        )),
        Ok(_) => findings.push(finding(
            CASE,
            RULE,
            "a re-append with an emptied protected set appended a second record".to_owned(),
        )),
    }
    // The stored record stands: the ORIGINAL declaration survives every
    // refused re-append, and exactly one record exists.
    match ledger.evaluations(&revision(2), bound()) {
        Ok(records) if records.len() == 1 => {
            if records[0].metric != "iterations_per_settlement"
                || records[0].protected != vec!["cap-suite".to_owned()]
            {
                findings.push(finding(
                    CASE,
                    RULE,
                    "the stored evaluation's declared facts were displaced by a refused \
                     re-append"
                        .to_owned(),
                ));
            }
        }
        Ok(records) => findings.push(finding(
            CASE,
            RULE,
            format!(
                "the ledger holds {} evaluation records where one was appended",
                records.len()
            ),
        )),
        Err(error) => findings.push(finding(
            CASE,
            RULE,
            format!("the evaluation read-back failed: {error}"),
        )),
    }
    findings
}

/// Case: decisions are typed, legal, and durable — provisional-with-
/// reasons then promoted-with-evidence; the derived entry and the history
/// read back exactly.
fn decisions_are_typed_legal_and_durable(
    factory: &dyn RevisionLedgerFactory,
) -> Vec<ContractFinding> {
    const CASE: &str = "decisions_are_typed_legal_and_durable";
    const RULE: &str = "spec 19.4 via Task 23 D2: a refusal writes Provisional with durable \
         per-condition reasons, a pass writes Promoted with the eight-condition decision \
         record, and the derived decision is the latest record's kind";
    let mut findings = Vec::new();
    let Ok(ledger) = factory.build() else {
        return vec![finding(
            CASE,
            RULE,
            "the factory could not build a ledger for the case".to_owned(),
        )];
    };
    if let Err(error) = ledger.admit_proposal(proposal(3), bound()) {
        return vec![finding(
            CASE,
            RULE,
            format!("the admission failed: {error}"),
        )];
    }
    let provisional = RevisionDecisionRecord::new(
        revision(3),
        DecisionKind::Pending,
        GateDecision::provisional(vec![GateRefusal::MissingEvaluation])
            .expect("a non-empty refusal composes"),
        EpochSeconds::new(BASE_INSTANT + 1),
    );
    let provisional = match provisional {
        Ok(record) => record,
        Err(error) => {
            return vec![finding(
                CASE,
                RULE,
                format!("a legal provisional record refused to compose: {error}"),
            )];
        }
    };
    if let Err(error) = ledger.apply_decision(provisional.clone(), bound()) {
        return vec![finding(
            CASE,
            RULE,
            format!("the provisional decision failed: {error}"),
        )];
    }
    let promoted = RevisionDecisionRecord::new(
        revision(3),
        DecisionKind::Provisional,
        GateDecision::promoted(promotion_evidence()),
        EpochSeconds::new(BASE_INSTANT + 2),
    );
    let promoted = match promoted {
        Ok(record) => record,
        Err(error) => {
            return vec![finding(
                CASE,
                RULE,
                format!("a legal promotion record refused to compose: {error}"),
            )];
        }
    };
    if let Err(error) = ledger.apply_decision(promoted.clone(), bound()) {
        return vec![finding(
            CASE,
            RULE,
            format!("the promotion decision failed: {error}"),
        )];
    }
    match ledger.revision(&revision(3), bound()) {
        Ok(Some(RevisionEntry {
            decision: Some(DecisionKind::Promoted),
            rolled_back: false,
            ..
        })) => {}
        Ok(Some(entry)) => findings.push(finding(
            CASE,
            RULE,
            format!(
                "the derived entry reads decision={:?} rolled_back={} after the promotion",
                entry.decision, entry.rolled_back
            ),
        )),
        Ok(None) => findings.push(finding(
            CASE,
            RULE,
            "the derived entry vanished after two decisions".to_owned(),
        )),
        Err(error) => findings.push(finding(
            CASE,
            RULE,
            format!("reading the derived entry failed: {error}"),
        )),
    }
    match ledger.decisions(&revision(3), bound()) {
        Ok(history) if history == vec![provisional, promoted] => {}
        Ok(history) if history.len() != 2 => findings.push(finding(
            CASE,
            RULE,
            format!(
                "the history carries {} records where 2 were applied",
                history.len()
            ),
        )),
        Ok(_) => findings.push(finding(
            CASE,
            RULE,
            "the history does not read back as the records applied".to_owned(),
        )),
        Err(error) => findings.push(finding(
            CASE,
            RULE,
            format!("reading the history failed: {error}"),
        )),
    }
    findings
}

/// Case: stale claims, final states, unknown revisions, and zero-reason
/// verdicts are refused and write nothing.
fn illegal_decisions_are_refused_and_write_nothing(
    factory: &dyn RevisionLedgerFactory,
) -> Vec<ContractFinding> {
    const CASE: &str = "illegal_decisions_are_refused_and_write_nothing";
    const RULE: &str = "spec 5.3.8/19.4 via Task 23 D2: a decision past a final state composes \
         nothing, a stale claimed state is refused, an unknown revision cannot be adjudicated, \
         and a zero-reason verdict composes no record";
    let mut findings = Vec::new();
    let Ok(ledger) = factory.build() else {
        return vec![finding(
            CASE,
            RULE,
            "the factory could not build a ledger for the case".to_owned(),
        )];
    };
    // A zero-reason verdict does not even compose — rejected…
    match GateDecision::rejected(Vec::new(), ref_of("evd-suite-rollback")) {
        Ok(_) => findings.push(finding(
            CASE,
            RULE,
            "an EMPTY rejection composed — a decision record with no reasons in it".to_owned(),
        )),
        Err(error) => {
            if !matches!(
                error,
                crate::revision_ledger::GateDecisionError::EmptyReasons
            ) {
                findings.push(finding(
                    CASE,
                    RULE,
                    format!("the empty rejection was refused for the wrong reason: {error}"),
                ));
            }
        }
    }
    // …and provisional: insufficient evidence refuses, it never passes and
    // never records itself without reasons.
    match GateDecision::provisional(Vec::new()) {
        Ok(_) => findings.push(finding(
            CASE,
            RULE,
            "an EMPTY provisional composed — a decision record with no reasons in it".to_owned(),
        )),
        Err(error) => {
            if !matches!(
                error,
                crate::revision_ledger::GateDecisionError::EmptyReasons
            ) {
                findings.push(finding(
                    CASE,
                    RULE,
                    format!("the empty provisional was refused for the wrong reason: {error}"),
                ));
            }
        }
    }
    // A record past a final state refuses to compose.
    match RevisionDecisionRecord::new(
        revision(4),
        DecisionKind::Promoted,
        GateDecision::promoted(promotion_evidence()),
        EpochSeconds::new(BASE_INSTANT),
    ) {
        Err(crate::revision_ledger::DecisionRecordError::FinalState { .. }) => {}
        Ok(_) => findings.push(finding(
            CASE,
            RULE,
            "a record claimed FROM a final state composed — the write-once floor fell".to_owned(),
        )),
    }
    // An unknown revision cannot be adjudicated.
    let unknown = RevisionDecisionRecord::new(
        revision(4),
        DecisionKind::Pending,
        GateDecision::provisional(vec![GateRefusal::MissingEvaluation])
            .expect("a non-empty refusal composes"),
        EpochSeconds::new(BASE_INSTANT),
    )
    .expect("the shape is legal");
    match ledger.apply_decision(unknown, bound()) {
        Err(RevisionLedgerError::UnknownRevision { .. }) => {}
        Err(other) => findings.push(finding(
            CASE,
            RULE,
            format!("the unknown-revision decision failed for the wrong reason: {other}"),
        )),
        Ok(_) => findings.push(finding(
            CASE,
            RULE,
            "a revision with no admitted proposal was adjudicated".to_owned(),
        )),
    }
    // A stale claimed state is refused, nothing written: the derived
    // decision is pending, the record claims provisional.
    if let Err(error) = ledger.admit_proposal(proposal(4), bound()) {
        return vec![finding(
            CASE,
            RULE,
            format!("the admission failed: {error}"),
        )];
    }
    let stale = RevisionDecisionRecord::new(
        revision(4),
        DecisionKind::Provisional,
        GateDecision::promoted(promotion_evidence()),
        EpochSeconds::new(BASE_INSTANT),
    )
    .expect("the shape is legal");
    match ledger.apply_decision(stale, bound()) {
        Err(RevisionLedgerError::StaleDecision {
            current: DecisionKind::Pending,
            ..
        }) => {}
        Err(other) => findings.push(finding(
            CASE,
            RULE,
            format!("the stale decision failed for the wrong reason: {other}"),
        )),
        Ok(_) => findings.push(finding(
            CASE,
            RULE,
            "a decision with a wrong claimed state was applied".to_owned(),
        )),
    }
    match ledger.decisions(&revision(4), bound()) {
        Ok(history) if history.is_empty() => {}
        Ok(history) => findings.push(finding(
            CASE,
            RULE,
            format!("the refused decisions left {} history rows", history.len()),
        )),
        Err(error) => findings.push(finding(
            CASE,
            RULE,
            format!("reading the history after refusals failed: {error}"),
        )),
    }
    findings
}

/// Case: restores are append-only, refuse a never-promoted revision, flip
/// the derived entry to promoted-but-rolled-back, and refuse once rolled
/// back — the restore is a one-way durable fact.
fn restores_are_append_only_and_refuse_the_unpromoted(
    factory: &dyn RevisionLedgerFactory,
) -> Vec<ContractFinding> {
    const CASE: &str = "restores_are_append_only_and_refuse_the_unpromoted";
    const RULE: &str = "spec 19.4 via Task 23 D4: rollback records the restore transition \
         append-only; a never-promoted revision has nothing to restore; a promoted-then- \
         restored revision reads promoted-BUT-ROLLED-BACK, never silently active";
    let mut findings = Vec::new();
    let Ok(ledger) = factory.build() else {
        return vec![finding(
            CASE,
            RULE,
            "the factory could not build a ledger for the case".to_owned(),
        )];
    };
    // A never-promoted revision cannot restore.
    if let Err(error) = ledger.admit_proposal(proposal(5), bound()) {
        return vec![finding(
            CASE,
            RULE,
            format!("the admission failed: {error}"),
        )];
    }
    let unpromoted = RestoreRecord::new(
        revision(5),
        DecisionKind::Pending,
        "rollback before any decision".to_owned(),
        EpochSeconds::new(BASE_INSTANT + 1),
    );
    match ledger.record_restore(unpromoted, bound()) {
        Err(RevisionLedgerError::StaleRestore {
            current: DecisionKind::Pending,
            rolled_back: false,
            ..
        }) => {}
        Err(other) => findings.push(finding(
            CASE,
            RULE,
            format!("the unpromoted restore failed for the wrong reason: {other}"),
        )),
        Ok(_) => findings.push(finding(
            CASE,
            RULE,
            "a never-promoted revision recorded a restore".to_owned(),
        )),
    }
    // The promotion lands, then the restore flips the derived entry.
    let promoted = RevisionDecisionRecord::new(
        revision(5),
        DecisionKind::Pending,
        GateDecision::promoted(promotion_evidence()),
        EpochSeconds::new(BASE_INSTANT + 2),
    )
    .expect("the promotion shape is legal");
    if let Err(error) = ledger.apply_decision(promoted, bound()) {
        return vec![finding(
            CASE,
            RULE,
            format!("the promotion failed: {error}"),
        )];
    }
    let restore = RestoreRecord::new(
        revision(5),
        DecisionKind::Promoted,
        "the operator rolled the candidate back".to_owned(),
        EpochSeconds::new(BASE_INSTANT + 3),
    );
    if let Err(error) = ledger.record_restore(restore, bound()) {
        return vec![finding(CASE, RULE, format!("the restore failed: {error}"))];
    }
    match ledger.revision(&revision(5), bound()) {
        Ok(Some(RevisionEntry {
            decision: Some(DecisionKind::Promoted),
            rolled_back: true,
            ..
        })) => {}
        Ok(Some(entry)) => findings.push(finding(
            CASE,
            RULE,
            format!(
                "the derived entry reads decision={:?} rolled_back={} after the restore — it \
                 must read promoted-but-rolled-back",
                entry.decision, entry.rolled_back
            ),
        )),
        Ok(None) => findings.push(finding(
            CASE,
            RULE,
            "the derived entry vanished after the restore".to_owned(),
        )),
        Err(error) => findings.push(finding(
            CASE,
            RULE,
            format!("reading the derived entry after the restore failed: {error}"),
        )),
    }
    // The restore is one-way: a second restore refuses.
    let again = RestoreRecord::new(
        revision(5),
        DecisionKind::Promoted,
        "a second restore".to_owned(),
        EpochSeconds::new(BASE_INSTANT + 4),
    );
    match ledger.record_restore(again, bound()) {
        Err(RevisionLedgerError::StaleRestore {
            rolled_back: true, ..
        }) => {}
        Err(other) => findings.push(finding(
            CASE,
            RULE,
            format!("the second restore failed for the wrong reason: {other}"),
        )),
        Ok(_) => findings.push(finding(
            CASE,
            RULE,
            "a second restore was recorded — the rollback history is not append-only-one-way"
                .to_owned(),
        )),
    }
    match ledger.restores(&revision(5), bound()) {
        Ok(records) if records.len() == 1 => {}
        Ok(records) => findings.push(finding(
            CASE,
            RULE,
            format!(
                "the restore history carries {} records where one belongs",
                records.len()
            ),
        )),
        Err(error) => findings.push(finding(
            CASE,
            RULE,
            format!("reading the restore history failed: {error}"),
        )),
    }
    findings
}

/// Case: the evaluation record carries ALL FOUR arms' reference digests
/// (COR-2, R1) — the regression arms' rulers are durable, read back
/// exactly, and a re-append whose regression-arm digest disagrees is the
/// typed conflict. The falsifier evaluated through the real binary with
/// the held-out arms freezing one ruler and the regression arms a MOVED
/// one; the record had no field that could even notice.
fn evaluations_carry_all_four_arms_reference_digests(
    factory: &dyn RevisionLedgerFactory,
) -> Vec<ContractFinding> {
    const CASE: &str = "evaluations_carry_all_four_arms_reference_digests";
    const RULE: &str = "spec 19.4 via Task 23 D2/COR-2 R1: an evaluation record durably \
         carries all FOUR arms' reference digests — held-out control/treatment AND \
         regression control/treatment — reads them back exactly, and a re-append whose \
         regression-arm digest disagrees is the typed conflict";
    let mut findings = Vec::new();
    let Ok(ledger) = factory.build() else {
        return vec![finding(
            CASE,
            RULE,
            "the factory could not build a ledger for the case".to_owned(),
        )];
    };
    if let Err(error) = ledger.admit_proposal(proposal(6), bound()) {
        return vec![finding(
            CASE,
            RULE,
            format!("the admission failed: {error}"),
        )];
    }
    if let Err(error) = ledger.append_evaluation(evaluation(6), bound()) {
        return vec![finding(
            CASE,
            RULE,
            format!("the evaluation append failed: {error}"),
        )];
    }
    match ledger.evaluations(&revision(6), bound()) {
        Ok(records) if records.len() == 1 => {
            let stored = &records[0];
            for (arm, carried) in [
                ("held-out control", &stored.control_reference_digest),
                ("held-out treatment", &stored.treatment_reference_digest),
                (
                    "regression control",
                    &stored.regression_control_reference_digest,
                ),
                (
                    "regression treatment",
                    &stored.regression_treatment_reference_digest,
                ),
            ] {
                if *carried != digest() {
                    findings.push(finding(
                        CASE,
                        RULE,
                        format!("the {arm} arm's reference digest did not read back as frozen"),
                    ));
                }
            }
        }
        Ok(records) => findings.push(finding(
            CASE,
            RULE,
            format!(
                "the ledger holds {} evaluation records where one was appended",
                records.len()
            ),
        )),
        Err(error) => findings.push(finding(
            CASE,
            RULE,
            format!("the evaluation read-back failed: {error}"),
        )),
    }
    // A re-append of the same run-set whose REGRESSION arm froze a moved
    // ruler disagrees: the typed conflict, the stored record standing.
    let mut moved_control = evaluation(6);
    moved_control.regression_control_reference_digest = moved_digest();
    match ledger.append_evaluation(moved_control, bound()) {
        Err(RevisionLedgerError::EvaluationConflict { .. }) => {}
        Err(other) => findings.push(finding(
            CASE,
            RULE,
            format!("the moved-regression-control re-append failed for the wrong reason: {other}"),
        )),
        Ok(appended) if appended.idempotent => findings.push(finding(
            CASE,
            RULE,
            "a re-append whose REGRESSION CONTROL arm carries a moved digest was swallowed \
             as the idempotent no-op — the regression ruler sits outside the agreement"
                .to_owned(),
        )),
        Ok(_) => findings.push(finding(
            CASE,
            RULE,
            "a re-append with a moved regression-control digest appended a second record"
                .to_owned(),
        )),
    }
    let mut moved_treatment = evaluation(6);
    moved_treatment.regression_treatment_reference_digest = moved_digest();
    match ledger.append_evaluation(moved_treatment, bound()) {
        Err(RevisionLedgerError::EvaluationConflict { .. }) => {}
        Err(other) => findings.push(finding(
            CASE,
            RULE,
            format!(
                "the moved-regression-treatment re-append failed for the wrong reason: {other}"
            ),
        )),
        Ok(appended) if appended.idempotent => findings.push(finding(
            CASE,
            RULE,
            "a re-append whose REGRESSION TREATMENT arm carries a moved digest was swallowed \
             as the idempotent no-op — the regression ruler sits outside the agreement"
                .to_owned(),
        )),
        Ok(_) => findings.push(finding(
            CASE,
            RULE,
            "a re-append with a moved regression-treatment digest appended a second record"
                .to_owned(),
        )),
    }
    findings
}

/// Runs the full `RevisionLedger` contract suite against one factory.
pub fn revision_ledger_suite(factory: &dyn RevisionLedgerFactory) -> Vec<ContractFinding> {
    let mut findings = Vec::new();
    findings.extend(proposals_are_idempotent_on_revision(factory));
    findings.extend(proposals_read_back_exactly_as_admitted(factory));
    findings.extend(evaluations_are_idempotent_and_rooted(factory));
    findings.extend(evaluations_carry_all_four_arms_reference_digests(factory));
    findings.extend(decisions_are_typed_legal_and_durable(factory));
    findings.extend(illegal_decisions_are_refused_and_write_nothing(factory));
    findings.extend(restores_are_append_only_and_refuse_the_unpromoted(factory));
    findings
}
