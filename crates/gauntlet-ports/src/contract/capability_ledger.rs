//! The `CapabilityLedger` shared contract suite (spec §19.1; plan Task 22,
//! charter D1/D2).
//!
//! Pinned semantics: the observation log is append-only, ungated, and
//! idempotent on the declared observation identity (same identity with
//! agreeing facts is a no-op; a disagreeing re-append is the typed
//! conflict, the stored row standing); the first observation registers the
//! capability `Observed`; the register's ONLY status writer is a
//! first-class transition record whose `from` must agree with the durable
//! status and whose move must be legal (revoked is sticky — no transition
//! leaves it); applied transitions are durable, ordered history.

use gauntlet_domain::{
    CapabilityId, CapabilityStatus, EpochSeconds, EvidenceKind, EvidenceLocator, EvidenceRef,
    Outcome, RunId, UnitId, VerificationRef,
};

use crate::capability_ledger::{
    CapabilityEntry, CapabilityLedger, CapabilityLedgerError, ObservationAppend, ObservationClass,
    ObservationIdentity, ObservationRecord, PromotionRefusal, StatusTransition,
    StatusTransitionError, TransitionReason,
};
use crate::contract::ContractFinding;
use crate::deadline::Deadline;

/// The suite's name in findings.
pub const SUITE: &str = "capability_ledger";

const BASE_INSTANT: u64 = 1_700_000_000;
/// The suite's bound span, on its own line (numeric literals never share a
/// line with a duration-shaped identifier).
const BOUND_SPAN: u64 = 90;

/// Constructs the ledger under test. `build` must return an ISOLATED, EMPTY
/// instance.
pub trait CapabilityLedgerFactory {
    fn label(&self) -> &'static str;

    fn build(&self) -> Result<Box<dyn CapabilityLedger>, String>;
}

fn finding(case: &'static str, rule: &'static str, detail: String) -> ContractFinding {
    ContractFinding::new(SUITE, case, rule, detail)
}

fn bound() -> Deadline {
    Deadline::at_epoch(EpochSeconds::new(BASE_INSTANT + BOUND_SPAN))
}

fn capability() -> CapabilityId {
    CapabilityId::new("cap-suite-0001").expect("suite capability id")
}

fn run(sequence: usize) -> RunId {
    // 26-char Crockford form; the last char varies over the suite's runs
    // (hex digits 0-7 are Crockford characters).
    RunId::new(&format!("01ARZ3NDEKTSV4RRFFQ69G5FA{sequence:X}")).expect("suite run id is a ULID")
}

fn identity(class: ObservationClass, sequence: usize, fact: &str) -> ObservationIdentity {
    ObservationIdentity::new(capability(), class, run(sequence), fact.to_owned())
        .expect("suite identity")
}

/// One settlement-shaped observation with every fact field populated, so
/// the exactness case discriminates across the whole row. The pinned
/// evidence ref (`evd-suite`) is what a rooted promotion witness names —
/// the log-rooting case (COR-1 C3) resolves witnesses against rows like
/// this one.
fn settlement_record(sequence: usize) -> ObservationRecord {
    ObservationRecord::new(
        identity(
            ObservationClass::Settlement,
            sequence,
            "settlement:AK-9001:vrf-suite",
        ),
        EpochSeconds::new(BASE_INSTANT + sequence as u64),
        "the unit settled".to_owned(),
    )
    .with_unit(UnitId::new("AK-9001").expect("suite unit"))
    .with_outcome(Outcome::Pass)
    .with_instrument_deterministic(true)
    .with_evidence_kind(EvidenceKind::TestReport)
    .with_evidence_locator(EvidenceLocator::new("suite-locator").expect("suite locator"))
    .with_reference_digest(suite_digest())
    .with_verification(VerificationRef::new("vrf-suite").expect("suite verification"))
    .with_evidence(EvidenceRef::new("evd-suite").expect("suite evidence"))
    .with_orchestration_cost(2)
}

/// The suite's digest fixture (kept beside the cases that use it).
fn suite_digest() -> gauntlet_domain::Digest {
    gauntlet_domain::Digest::new("sha256", "0011223344556677").expect("suite digest")
}

/// Case: the observation log is append-only and idempotent on the declared
/// identity — a same-identity re-append with agreeing facts is a no-op
/// (nothing duplicated), and a disagreeing re-append is the typed conflict
/// with the stored row standing untouched.
fn observation_appends_are_idempotent_on_identity(
    factory: &dyn CapabilityLedgerFactory,
) -> Vec<ContractFinding> {
    const CASE: &str = "observation_appends_are_idempotent_on_identity";
    const RULE: &str = "spec 19.1 via Task 22 D1: observation appends are idempotent on the \
         declared identity — the same identity re-appended with agreeing facts appends nothing, \
         and a disagreeing re-append is the typed conflict with the stored row standing";
    let mut findings = Vec::new();
    let Ok(ledger) = factory.build() else {
        return vec![finding(
            CASE,
            RULE,
            "the factory could not build a ledger for the case".to_owned(),
        )];
    };
    let record = settlement_record(0);
    let receipt = match ledger.append_observation(record.clone(), bound()) {
        Ok(receipt) => receipt,
        Err(error) => {
            return vec![finding(
                CASE,
                RULE,
                format!("the first append failed: {error}"),
            )];
        }
    };
    if receipt.idempotent {
        findings.push(finding(
            CASE,
            RULE,
            "the FIRST append of a fresh identity reported itself idempotent".to_owned(),
        ));
    }
    // Same identity, agreeing facts: a no-op.
    match ledger.append_observation(record.clone(), bound()) {
        Ok(ObservationAppend {
            idempotent: true, ..
        }) => {}
        Ok(_) => findings.push(finding(
            CASE,
            RULE,
            "re-appending the same identity with agreeing facts appended a second row".to_owned(),
        )),
        Err(error) => findings.push(finding(
            CASE,
            RULE,
            format!("the agreeing re-append was refused: {error}"),
        )),
    }
    let stored = match ledger.observations(&capability(), bound()) {
        Ok(observations) => observations,
        Err(error) => {
            return vec![finding(
                CASE,
                RULE,
                format!("the read-back failed: {error}"),
            )];
        }
    };
    if stored.len() != 1 {
        findings.push(finding(
            CASE,
            RULE,
            format!(
                "the log holds {} rows after one identity was appended twice",
                stored.len()
            ),
        ));
        return findings;
    }
    // Same identity, disagreeing facts: the typed conflict, row standing.
    let disagreeing = record
        .clone()
        .with_detail("a different claim about the same identity".to_owned());
    match ledger.append_observation(disagreeing, bound()) {
        Err(CapabilityLedgerError::ObservationConflict { .. }) => {}
        Err(other) => findings.push(finding(
            CASE,
            RULE,
            format!("the disagreeing re-append failed for the wrong reason: {other}"),
        )),
        Ok(_) => findings.push(finding(
            CASE,
            RULE,
            "a disagreeing re-append of a recorded identity was accepted — a later writer \
             displaced recorded memory"
                .to_owned(),
        )),
    }
    match ledger.observations(&capability(), bound()) {
        Ok(observations) if observations.len() == 1 => {}
        Ok(observations) => findings.push(finding(
            CASE,
            RULE,
            format!(
                "the conflict left {} rows; the stored row must stand",
                observations.len()
            ),
        )),
        Err(error) => findings.push(finding(
            CASE,
            RULE,
            format!("the read-back after the conflict failed: {error}"),
        )),
    }
    findings
}

/// Case: observations read back EXACTLY as appended, in append order, every
/// fact field intact — the register's typed facts survive the round trip.
fn observations_read_back_exactly_as_appended(
    factory: &dyn CapabilityLedgerFactory,
) -> Vec<ContractFinding> {
    const CASE: &str = "observations_read_back_exactly_as_appended";
    const RULE: &str = "spec 19.1 via Task 22 D1: the observation log replays every row exactly \
         as appended, in append order, every typed fact intact";
    let mut findings = Vec::new();
    let Ok(ledger) = factory.build() else {
        return vec![finding(
            CASE,
            RULE,
            "the factory could not build a ledger for the case".to_owned(),
        )];
    };
    let mut expected = Vec::new();
    // Every §19.1 class, including the failure and inconclusive shapes the
    // ungated policy must admit as eagerly as success.
    let fixtures: Vec<ObservationRecord> = vec![
        settlement_record(0),
        ObservationRecord::new(
            identity(ObservationClass::Attempt, 1, "verification:vrf-a"),
            EpochSeconds::new(BASE_INSTANT + 1),
            "an attempt recorded raw".to_owned(),
        )
        .with_outcome(Outcome::Pass)
        .with_instrument_deterministic(true),
        ObservationRecord::new(
            identity(ObservationClass::Failure, 1, "verification:vrf-b"),
            EpochSeconds::new(BASE_INSTANT + 2),
            "a failure recorded raw".to_owned(),
        )
        .with_outcome(Outcome::Fail)
        .with_instrument_deterministic(false),
        ObservationRecord::new(
            identity(ObservationClass::Attempt, 1, "verification:vrf-c"),
            EpochSeconds::new(BASE_INSTANT + 3),
            "an inconclusive attempt recorded raw".to_owned(),
        )
        .with_outcome(Outcome::Inconclusive),
        ObservationRecord::new(
            identity(ObservationClass::Correction, 1, "correction:0"),
            EpochSeconds::new(BASE_INSTANT + 4),
            "the controller chose a retry".to_owned(),
        ),
        ObservationRecord::new(
            identity(ObservationClass::Payment, 1, "checkpoint-spend"),
            EpochSeconds::new(BASE_INSTANT + 5),
            "the checkpoint's durable spend".to_owned(),
        ),
        ObservationRecord::new(
            identity(ObservationClass::Recovery, 1, "recovery:settlement-replay"),
            EpochSeconds::new(BASE_INSTANT + 6),
            "replay completion completed one settlement".to_owned(),
        ),
        ObservationRecord::new(
            identity(ObservationClass::Comparison, 1, "comparison:vrf-a"),
            EpochSeconds::new(BASE_INSTANT + 7),
            "the comparison accepted".to_owned(),
        ),
    ];
    for record in &fixtures {
        match ledger.append_observation(record.clone(), bound()) {
            Ok(_) => expected.push(record.clone()),
            Err(error) => {
                return vec![finding(
                    CASE,
                    RULE,
                    format!(
                        "appending a {} observation failed: {error}",
                        record.identity().class()
                    ),
                )];
            }
        }
    }
    match ledger.observations(&capability(), bound()) {
        Ok(observations) if observations == expected => {}
        Ok(observations) => {
            for (position, (stored, wanted)) in observations.iter().zip(expected.iter()).enumerate()
            {
                if stored != wanted {
                    findings.push(finding(
                        CASE,
                        RULE,
                        format!(
                            "row {position} does not read back as appended: class {} fact {:?} \
                             differs from the appended record",
                            stored.identity().class(),
                            stored.identity().fact()
                        ),
                    ));
                }
            }
            if observations.len() != expected.len() {
                findings.push(finding(
                    CASE,
                    RULE,
                    format!(
                        "the log replays {} rows where {} were appended",
                        observations.len(),
                        expected.len()
                    ),
                ));
            }
            if findings.is_empty() {
                findings.push(finding(
                    CASE,
                    RULE,
                    "the read-back diverged from the appended rows".to_owned(),
                ));
            }
        }
        Err(error) => findings.push(finding(
            CASE,
            RULE,
            format!("the read-back failed: {error}"),
        )),
    }
    findings
}

/// Case: the observation log is UNGATED — failure and inconclusive
/// outcomes append with the same eagerness as passes, and any class
/// appends freely (spec §19.1: a failed attempt is still memory).
fn every_class_appends_ungated(factory: &dyn CapabilityLedgerFactory) -> Vec<ContractFinding> {
    const CASE: &str = "every_class_appends_ungated";
    const RULE: &str = "spec 19.1: developmental memory is immediate and append-only for EVERY \
         class and EVERY outcome — a failed attempt is still memory";
    let mut findings = Vec::new();
    for class in ObservationClass::ALL {
        let Ok(ledger) = factory.build() else {
            return vec![finding(
                CASE,
                RULE,
                "the factory could not build a ledger for the case".to_owned(),
            )];
        };
        let record = ObservationRecord::new(
            identity(*class, 3, "suite-fact"),
            EpochSeconds::new(BASE_INSTANT),
            format!("a {class} observation"),
        )
        .with_outcome(Outcome::Fail);
        match ledger.append_observation(record, bound()) {
            Ok(_) => {}
            Err(error) => findings.push(finding(
                CASE,
                RULE,
                format!("a failing-outcome {class} append was gated: {error}"),
            )),
        }
    }
    findings
}

/// Case: the first observation registers the capability `Observed`; an
/// unevaluated capability reads as `Observed` with no transitions.
fn first_observation_registers_observed(
    factory: &dyn CapabilityLedgerFactory,
) -> Vec<ContractFinding> {
    const CASE: &str = "first_observation_registers_observed";
    const RULE: &str = "spec 5.3.8/19 via Task 22 D5: a capability enters the register Observed \
         when its first observation lands, and an unevaluated capability carries no transitions";
    let mut findings = Vec::new();
    let Ok(ledger) = factory.build() else {
        return vec![finding(
            CASE,
            RULE,
            "the factory could not build a ledger for the case".to_owned(),
        )];
    };
    match ledger.capability(&capability(), bound()) {
        Ok(None) => {}
        Ok(Some(entry)) => findings.push(finding(
            CASE,
            RULE,
            format!(
                "an unknown capability read as {} instead of absent",
                entry.status
            ),
        )),
        Err(error) => findings.push(finding(
            CASE,
            RULE,
            format!("reading an unknown capability failed: {error}"),
        )),
    }
    if let Err(error) = ledger.append_observation(settlement_record(4), bound()) {
        return vec![finding(
            CASE,
            RULE,
            format!("the registering append failed: {error}"),
        )];
    }
    match ledger.capability(&capability(), bound()) {
        Ok(Some(CapabilityEntry {
            status: CapabilityStatus::Observed,
            ..
        })) => {}
        Ok(Some(entry)) => findings.push(finding(
            CASE,
            RULE,
            format!(
                "the first observation registered the capability {}",
                entry.status
            ),
        )),
        Ok(None) => findings.push(finding(
            CASE,
            RULE,
            "the first observation did not register the capability".to_owned(),
        )),
        Err(error) => findings.push(finding(
            CASE,
            RULE,
            format!("reading the registered capability failed: {error}"),
        )),
    }
    match ledger.transitions(&capability(), bound()) {
        Ok(history) if history.is_empty() => {}
        Ok(history) => findings.push(finding(
            CASE,
            RULE,
            format!(
                "an unevaluated capability carries {} transitions",
                history.len()
            ),
        )),
        Err(error) => findings.push(finding(
            CASE,
            RULE,
            format!("reading an unevaluated capability's history failed: {error}"),
        )),
    }
    findings
}

/// Composes and applies Observed→Provisional (refused, reasons durable)
/// then Provisional→Promoted (promoted, decision evidence durable), and
/// asserts the history reads back exactly.
fn transitions_are_legal_typed_and_durable(
    factory: &dyn CapabilityLedgerFactory,
) -> Vec<ContractFinding> {
    const CASE: &str = "transitions_are_legal_typed_and_durable";
    const RULE: &str = "spec 5.3.8/19.4 via Task 22 D4/D5: refusals write Provisional with \
         durable per-condition reasons; promotion writes Promoted with a durable decision \
         record; the transition history is ordered and complete";
    let mut findings = Vec::new();
    let Ok(ledger) = factory.build() else {
        return vec![finding(
            CASE,
            RULE,
            "the factory could not build a ledger for the case".to_owned(),
        )];
    };
    if let Err(error) = ledger.append_observation(settlement_record(5), bound()) {
        return vec![finding(
            CASE,
            RULE,
            format!("the registering append failed: {error}"),
        )];
    }
    let refusal = StatusTransition::new(
        capability(),
        CapabilityStatus::Observed,
        CapabilityStatus::Provisional,
        TransitionReason::Refused(vec![
            PromotionRefusal::InsufficientSettlements { have: 1, need: 3 },
            PromotionRefusal::InsufficientVariation,
        ]),
        vec![],
        EpochSeconds::new(BASE_INSTANT + 10),
    );
    let refusal = match refusal {
        Ok(transition) => transition,
        Err(error) => {
            return vec![finding(
                CASE,
                RULE,
                format!("a legal refusal transition refused to compose: {error}"),
            )];
        }
    };
    if let Err(error) = ledger.apply_transition(refusal.clone(), bound()) {
        return vec![finding(
            CASE,
            RULE,
            format!("the refusal transition failed: {error}"),
        )];
    }
    let evidence_ref = EvidenceRef::new("evd-promotion-decision").expect("suite evidence ref");
    let promotion = StatusTransition::new(
        capability(),
        CapabilityStatus::Provisional,
        CapabilityStatus::Promoted,
        TransitionReason::Promoted(promotion_evidence(5)),
        vec![evidence_ref.clone()],
        EpochSeconds::new(BASE_INSTANT + 11),
    );
    let promotion = match promotion {
        Ok(transition) => transition,
        Err(error) => {
            return vec![finding(
                CASE,
                RULE,
                format!("a legal promotion transition refused to compose: {error}"),
            )];
        }
    };
    if let Err(error) = ledger.apply_transition(promotion.clone(), bound()) {
        return vec![finding(
            CASE,
            RULE,
            format!("the promotion transition failed: {error}"),
        )];
    }
    // The register reads Promoted.
    match ledger.capability(&capability(), bound()) {
        Ok(Some(entry)) if entry.status == CapabilityStatus::Promoted => {}
        Ok(Some(entry)) => findings.push(finding(
            CASE,
            RULE,
            format!(
                "the register reads {} after the promotion transition",
                entry.status
            ),
        )),
        Ok(None) => findings.push(finding(
            CASE,
            RULE,
            "the register lost the capability after two transitions".to_owned(),
        )),
        Err(error) => findings.push(finding(
            CASE,
            RULE,
            format!("reading the register after promotion failed: {error}"),
        )),
    }
    // The history reads back exactly, in order.
    match ledger.transitions(&capability(), bound()) {
        Ok(history) if history == vec![refusal, promotion] => {}
        Ok(history) if history.len() != 2 => findings.push(finding(
            CASE,
            RULE,
            format!(
                "the history carries {} transitions where 2 were applied",
                history.len()
            ),
        )),
        Ok(_) => findings.push(finding(
            CASE,
            RULE,
            "the history does not read back as the transitions applied".to_owned(),
        )),
        Err(error) => findings.push(finding(
            CASE,
            RULE,
            format!("reading the history failed: {error}"),
        )),
    }
    findings
}

/// Case: illegal and stale transitions are refused and write nothing —
/// promoted cannot demote, a wrong `from` claim is stale, an unknown
/// capability cannot transition, and (below) revoked never moves.
fn illegal_and_stale_transitions_are_refused_and_write_nothing(
    factory: &dyn CapabilityLedgerFactory,
) -> Vec<ContractFinding> {
    const CASE: &str = "illegal_and_stale_transitions_are_refused_and_write_nothing";
    const RULE: &str = "spec 5.3.8 via Task 22 D2/D5: no transition the state machine forbids is \
         ever applied, a stale from-claim is refused, and nothing is written by a refusal";
    let mut findings = Vec::new();
    let Ok(ledger) = factory.build() else {
        return vec![finding(
            CASE,
            RULE,
            "the factory could not build a ledger for the case".to_owned(),
        )];
    };
    // The TYPE refuses to compose an illegal move (the no-set-status shape).
    match StatusTransition::new(
        capability(),
        CapabilityStatus::Promoted,
        CapabilityStatus::Provisional,
        TransitionReason::Refused(vec![PromotionRefusal::GateErosion]),
        vec![],
        EpochSeconds::new(BASE_INSTANT),
    ) {
        Err(StatusTransitionError::IllegalMove { .. }) => {}
        Err(other) => findings.push(finding(
            CASE,
            RULE,
            format!("an illegal move composed with the wrong refusal: {other}"),
        )),
        Ok(_) => findings.push(finding(
            CASE,
            RULE,
            "a promoted-to-provisional demotion composed into a transition record".to_owned(),
        )),
    }
    // The reason must carry the target.
    match StatusTransition::new(
        capability(),
        CapabilityStatus::Observed,
        CapabilityStatus::Promoted,
        TransitionReason::Refused(vec![PromotionRefusal::GateErosion]),
        vec![],
        EpochSeconds::new(BASE_INSTANT),
    ) {
        Err(StatusTransitionError::ReasonMismatch { .. }) => {}
        Err(other) => findings.push(finding(
            CASE,
            RULE,
            format!("a mismatched reason composed with the wrong refusal: {other}"),
        )),
        Ok(_) => findings.push(finding(
            CASE,
            RULE,
            "a refusal reason carried a promotion target".to_owned(),
        )),
    }
    // An unknown capability cannot transition.
    let unknown = StatusTransition::new(
        capability(),
        CapabilityStatus::Observed,
        CapabilityStatus::Provisional,
        TransitionReason::Refused(vec![PromotionRefusal::InsufficientSettlements {
            have: 0,
            need: 3,
        }]),
        vec![],
        EpochSeconds::new(BASE_INSTANT),
    )
    .expect("the shape is legal");
    match ledger.apply_transition(unknown, bound()) {
        Err(CapabilityLedgerError::UnknownCapability { .. }) => {}
        Err(other) => findings.push(finding(
            CASE,
            RULE,
            format!("the unknown-capability transition failed for the wrong reason: {other}"),
        )),
        Ok(_) => findings.push(finding(
            CASE,
            RULE,
            "a capability with no observations transitioned".to_owned(),
        )),
    }
    // A stale from-claim is refused, nothing written.
    if let Err(error) = ledger.append_observation(settlement_record(6), bound()) {
        return vec![finding(
            CASE,
            RULE,
            format!("the registering append failed: {error}"),
        )];
    }
    let stale = StatusTransition::new(
        capability(),
        CapabilityStatus::Provisional,
        CapabilityStatus::Promoted,
        TransitionReason::Promoted(promotion_evidence(6)),
        vec![],
        EpochSeconds::new(BASE_INSTANT),
    )
    .expect("the shape is legal");
    match ledger.apply_transition(stale, bound()) {
        Err(CapabilityLedgerError::StaleTransition {
            current: CapabilityStatus::Observed,
            ..
        }) => {}
        Err(other) => findings.push(finding(
            CASE,
            RULE,
            format!("the stale transition failed for the wrong reason: {other}"),
        )),
        Ok(_) => findings.push(finding(
            CASE,
            RULE,
            "a transition with a wrong from-claim was applied".to_owned(),
        )),
    }
    match ledger.transitions(&capability(), bound()) {
        Ok(history) if history.is_empty() => {}
        Ok(history) => findings.push(finding(
            CASE,
            RULE,
            format!(
                "the refused transitions left {} history rows",
                history.len()
            ),
        )),
        Err(error) => findings.push(finding(
            CASE,
            RULE,
            format!("reading the history after refusals failed: {error}"),
        )),
    }
    match ledger.capability(&capability(), bound()) {
        Ok(Some(entry)) if entry.status == CapabilityStatus::Observed => {}
        Ok(Some(entry)) => findings.push(finding(
            CASE,
            RULE,
            format!(
                "the refused transitions moved the status to {}",
                entry.status
            ),
        )),
        Ok(None) => findings.push(finding(
            CASE,
            RULE,
            "the register lost the capability".to_owned(),
        )),
        Err(error) => findings.push(finding(
            CASE,
            RULE,
            format!("reading the register after refusals failed: {error}"),
        )),
    }
    findings
}

/// Case: revoke is sticky — from Promoted the revoke applies, and NO
/// transition ever leaves Revoked (charter D5: re-elevation has no
/// flip-back path; the FULL evaluation against a non-revoked status is the
/// only road, and a revoked capability cannot present one).
fn revoke_is_sticky(factory: &dyn CapabilityLedgerFactory) -> Vec<ContractFinding> {
    const CASE: &str = "revoke_is_sticky";
    const RULE: &str = "spec 5.3.8 via Task 22 D5: revoked is terminal — no transition leaves \
         Revoked, so no flip-back path exists";
    let mut findings = Vec::new();
    let Ok(ledger) = factory.build() else {
        return vec![finding(
            CASE,
            RULE,
            "the factory could not build a ledger for the case".to_owned(),
        )];
    };
    if let Err(error) = ledger.append_observation(settlement_record(7), bound()) {
        return vec![finding(
            CASE,
            RULE,
            format!("the registering append failed: {error}"),
        )];
    }
    let promote = StatusTransition::new(
        capability(),
        CapabilityStatus::Observed,
        CapabilityStatus::Promoted,
        TransitionReason::Promoted(promotion_evidence(7)),
        vec![],
        EpochSeconds::new(BASE_INSTANT + 1),
    )
    .expect("the promotion shape is legal");
    if let Err(error) = ledger.apply_transition(promote, bound()) {
        return vec![finding(
            CASE,
            RULE,
            format!("the promotion failed: {error}"),
        )];
    }
    let revoke = StatusTransition::new(
        capability(),
        CapabilityStatus::Promoted,
        CapabilityStatus::Revoked,
        TransitionReason::Revoked("the operator revoked the capability".to_owned()),
        vec![],
        EpochSeconds::new(BASE_INSTANT + 2),
    )
    .expect("the revoke shape is legal");
    if let Err(error) = ledger.apply_transition(revoke, bound()) {
        return vec![finding(CASE, RULE, format!("the revoke failed: {error}"))];
    }
    match ledger.capability(&capability(), bound()) {
        Ok(Some(entry)) if entry.status == CapabilityStatus::Revoked => {}
        Ok(Some(entry)) => findings.push(finding(
            CASE,
            RULE,
            format!("the register reads {} after the revoke", entry.status),
        )),
        Ok(None) => findings.push(finding(
            CASE,
            RULE,
            "the register lost the capability".to_owned(),
        )),
        Err(error) => findings.push(finding(
            CASE,
            RULE,
            format!("reading after revoke failed: {error}"),
        )),
    }
    // Every move OUT of Revoked must be refused, and nothing may be written.
    for target in [
        CapabilityStatus::Promoted,
        CapabilityStatus::Provisional,
        CapabilityStatus::Observed,
    ] {
        let reason = if target == CapabilityStatus::Promoted {
            TransitionReason::Promoted(promotion_evidence(7))
        } else if target == CapabilityStatus::Provisional {
            TransitionReason::Refused(vec![PromotionRefusal::InsufficientVariation])
        } else {
            TransitionReason::Revoked("back to observed".to_owned())
        };
        match StatusTransition::new(
            capability(),
            CapabilityStatus::Revoked,
            target,
            reason,
            vec![],
            EpochSeconds::new(BASE_INSTANT + 3),
        ) {
            // Either typed refusal proves the point: no transition record
            // toward a non-revoked status can even be composed (an illegal
            // move, or a reason that cannot carry the target).
            Err(_) => {}
            Ok(transition) => match ledger.apply_transition(transition, bound()) {
                Err(_) => {}
                Ok(_) => findings.push(finding(
                    CASE,
                    RULE,
                    format!(
                        "a transition left Revoked toward {target} — the revocation was \
                             not sticky"
                    ),
                )),
            },
        }
    }
    match ledger.capability(&capability(), bound()) {
        Ok(Some(entry)) if entry.status == CapabilityStatus::Revoked => {}
        Ok(Some(entry)) => findings.push(finding(
            CASE,
            RULE,
            format!(
                "the register reads {} after the flip-back attempts",
                entry.status
            ),
        )),
        Ok(None) => findings.push(finding(
            CASE,
            RULE,
            "the register lost the capability".to_owned(),
        )),
        Err(error) => findings.push(finding(
            CASE,
            RULE,
            format!("reading the register after the flip-back attempts failed: {error}"),
        )),
    }
    findings
}

/// The suite's promotion-evidence fixture, ROOTED to the settlement row
/// `settlement_record(sequence)` appends: the witness names that row's
/// (run, unit, verification, evidence) facts exactly, so an honest
/// elevating transition resolves against the ledger's own observation log
/// (COR-1 C3).
fn promotion_evidence(sequence: usize) -> crate::capability_ledger::PromotionEvidence {
    crate::capability_ledger::PromotionEvidence {
        min_settlements_configured: 3,
        settlements_observed: 3,
        variation_axes: vec![
            crate::capability_ledger::VariationAxis::Unit(
                UnitId::new("AK-9001").expect("suite unit"),
            ),
            crate::capability_ledger::VariationAxis::EvidenceKind(EvidenceKind::TestReport),
        ],
        recovery_observations: 1,
        failure_rate_earlier: (0, 2),
        failure_rate_later: (0, 1),
        instrument_series: vec![true, true, true],
        orchestration_costs: vec![2, 2, 1],
        settlements: vec![crate::capability_ledger::SettlementWitness {
            run: run(sequence),
            unit: UnitId::new("AK-9001").expect("suite unit"),
            verification: VerificationRef::new("vrf-suite").expect("suite verification"),
            evidence: EvidenceRef::new("evd-suite").expect("suite evidence"),
        }],
    }
}

/// Case: elevating transitions are LOG-ROOTED earned writes (COR-1 C3) —
/// the B4 reproduction, verbatim: fabricated full-satisfaction evidence
/// composed into a legal Observed→Promoted transition over a
/// one-observation ledger is refused with the typed `UnrootedPromotion`,
/// the register is NOT elevated, and no transition row is appended. The
/// positive control then proves the obligation blocks only unrooted
/// elevations: with the witness rooted to the settlement row the ledger
/// actually carries, the same shape applies.
fn elevating_transitions_are_log_rooted(
    factory: &dyn CapabilityLedgerFactory,
) -> Vec<ContractFinding> {
    const CASE: &str = "elevating_transitions_are_log_rooted";
    const RULE: &str = "spec 19.1/19.2 via Task 22 COR-1 C3: an elevating transition's \
         witnesses must resolve to Settlement observations in the ledger's OWN observation \
         log (same capability, matching run/unit/verification/evidence) — a fabricated \
         promotion evidence is the typed UnrootedPromotion refusal, never an applied \
         transition";
    let mut findings = Vec::new();
    let Ok(ledger) = factory.build() else {
        return vec![finding(
            CASE,
            RULE,
            "the factory could not build a ledger for the case".to_owned(),
        )];
    };
    // One observation registers the capability Observed.
    if let Err(error) = ledger.append_observation(settlement_record(8), bound()) {
        return vec![finding(
            CASE,
            RULE,
            format!("the registering append failed: {error}"),
        )];
    }
    // Fabricated evidence over a one-observation ledger: the witness names
    // a run that appears nowhere in the log.
    let fabricated = StatusTransition::new(
        capability(),
        CapabilityStatus::Observed,
        CapabilityStatus::Promoted,
        TransitionReason::Promoted(promotion_evidence(9)),
        vec![],
        EpochSeconds::new(BASE_INSTANT + 1),
    )
    .expect("the forged shape is legal to compose (the LEDGER is the gate)");
    match ledger.apply_transition(fabricated, bound()) {
        Err(CapabilityLedgerError::UnrootedPromotion { .. }) => {}
        Err(other) => findings.push(finding(
            CASE,
            RULE,
            format!("the fabricated promotion failed for the wrong reason: {other}"),
        )),
        Ok(_) => findings.push(finding(
            CASE,
            RULE,
            "a fabricated promotion evidence APPLIED directly through the port — the \
             forged status stands as if earned"
                .to_owned(),
        )),
    }
    // The register was not elevated and no transition row was appended.
    match ledger.capability(&capability(), bound()) {
        Ok(Some(entry)) if entry.status == CapabilityStatus::Observed => {}
        Ok(Some(entry)) => findings.push(finding(
            CASE,
            RULE,
            format!(
                "the register reads {} after the refused fabrication — it must stay Observed",
                entry.status
            ),
        )),
        Ok(None) => findings.push(finding(
            CASE,
            RULE,
            "the register lost the capability".to_owned(),
        )),
        Err(error) => findings.push(finding(
            CASE,
            RULE,
            format!("reading the register after the refused fabrication failed: {error}"),
        )),
    }
    match ledger.transitions(&capability(), bound()) {
        Ok(history) if history.is_empty() => {}
        Ok(history) => findings.push(finding(
            CASE,
            RULE,
            format!(
                "the refused fabrication left {} transition rows; nothing may be appended",
                history.len()
            ),
        )),
        Err(error) => findings.push(finding(
            CASE,
            RULE,
            format!("reading the history after the refused fabrication failed: {error}"),
        )),
    }
    // Positive control: the SAME transition shape, with the witness rooted
    // to the settlement row the ledger actually carries, applies — the
    // obligation blocks unrooted elevations, not elevations.
    let rooted = StatusTransition::new(
        capability(),
        CapabilityStatus::Observed,
        CapabilityStatus::Promoted,
        TransitionReason::Promoted(promotion_evidence(8)),
        vec![],
        EpochSeconds::new(BASE_INSTANT + 2),
    )
    .expect("the rooted shape is legal");
    if let Err(error) = ledger.apply_transition(rooted, bound()) {
        findings.push(finding(
            CASE,
            RULE,
            format!("the honest rooted promotion was refused: {error}"),
        ));
    }
    findings
}

/// Case: a zero-reason refusal does not compose (COR-2 C6; falsifier
/// round-2 F2-2) — `TransitionReason::Refused` with an EMPTY refusal set
/// is a decision record with no decision in it: the honest promotion
/// service can never produce one (an all-conditions-pass evaluation
/// promotes instead), so the type refuses to compose it and no leg ever
/// writes a zero-reason refusal into the durable decision record. The
/// honest non-empty refusal shape is proven unaffected in the same case.
fn a_zero_reason_refusal_does_not_compose(
    factory: &dyn CapabilityLedgerFactory,
) -> Vec<ContractFinding> {
    const CASE: &str = "a_zero_reason_refusal_does_not_compose";
    const RULE: &str = "spec 19.4 via Task 22 COR-2 C6: a refusal carries the per-condition \
         reasons that failed — an EMPTY refusal set composes no transition record (the typed \
         EmptyRefusal), and the durable decision record never gains a zero-reason refusal";
    let mut findings = Vec::new();
    let Ok(ledger) = factory.build() else {
        return vec![finding(
            CASE,
            RULE,
            "the factory could not build a ledger for the case".to_owned(),
        )];
    };
    if let Err(error) = ledger.append_observation(settlement_record(10), bound()) {
        return vec![finding(
            CASE,
            RULE,
            format!("the registering append failed: {error}"),
        )];
    }
    // The empty refusal must not compose — not from Observed…
    match StatusTransition::new(
        capability(),
        CapabilityStatus::Observed,
        CapabilityStatus::Provisional,
        TransitionReason::Refused(Vec::new()),
        vec![],
        EpochSeconds::new(BASE_INSTANT + 1),
    ) {
        Err(StatusTransitionError::EmptyRefusal) => {}
        Err(other) => findings.push(finding(
            CASE,
            RULE,
            format!("the empty refusal was refused for the wrong reason: {other}"),
        )),
        Ok(transition) => {
            findings.push(finding(
                CASE,
                RULE,
                "an EMPTY refusal set composed into a transition record — a zero-reason \
                 refusal is a record the honest service can never produce"
                    .to_owned(),
            ));
            // …and if it composed, it must not land anywhere: a durable
            // zero-reason refusal is the exact record F2-2 named.
            if ledger.apply_transition(transition, bound()).is_ok() {
                findings.push(finding(
                    CASE,
                    RULE,
                    "the zero-reason refusal APPLIED — the durable decision record gained a \
                     refusal with no reasons"
                        .to_owned(),
                ));
            }
        }
    }
    // The honest refusal path is unaffected: a NON-empty refusal composes
    // and applies, writing Provisional with its reasons durable.
    let honest = StatusTransition::new(
        capability(),
        CapabilityStatus::Observed,
        CapabilityStatus::Provisional,
        TransitionReason::Refused(vec![PromotionRefusal::InsufficientSettlements {
            have: 1,
            need: 3,
        }]),
        vec![],
        EpochSeconds::new(BASE_INSTANT + 2),
    );
    let honest = match honest {
        Ok(transition) => transition,
        Err(error) => {
            findings.push(finding(
                CASE,
                RULE,
                format!("an honest non-empty refusal refused to compose: {error}"),
            ));
            return findings;
        }
    };
    if let Err(error) = ledger.apply_transition(honest, bound()) {
        findings.push(finding(
            CASE,
            RULE,
            format!("the honest non-empty refusal failed to apply: {error}"),
        ));
        return findings;
    }
    // …and the F2-2 shape verbatim — the zero-reason Provisional→Provisional
    // re-record — must not compose either.
    match StatusTransition::new(
        capability(),
        CapabilityStatus::Provisional,
        CapabilityStatus::Provisional,
        TransitionReason::Refused(Vec::new()),
        vec![],
        EpochSeconds::new(BASE_INSTANT + 3),
    ) {
        Err(StatusTransitionError::EmptyRefusal) => {}
        Err(other) => findings.push(finding(
            CASE,
            RULE,
            format!("the empty re-record was refused for the wrong reason: {other}"),
        )),
        Ok(transition) => {
            findings.push(finding(
                CASE,
                RULE,
                "the zero-reason Provisional→Provisional re-record composed — the durable \
                 decision record can be polluted with a refusal that refused nothing"
                    .to_owned(),
            ));
            if ledger.apply_transition(transition, bound()).is_ok() {
                findings.push(finding(
                    CASE,
                    RULE,
                    "the zero-reason re-record APPLIED — the durable decision record gained a \
                     second refusal with no reasons"
                        .to_owned(),
                ));
            }
        }
    }
    // The durable decision record carries exactly the one honest refusal.
    match ledger.transitions(&capability(), bound()) {
        Ok(history) if history.len() == 1 => {}
        Ok(history) => findings.push(finding(
            CASE,
            RULE,
            format!(
                "the decision record carries {} transition rows where exactly the one honest \
                 refusal belongs",
                history.len()
            ),
        )),
        Err(error) => findings.push(finding(
            CASE,
            RULE,
            format!("reading the decision record failed: {error}"),
        )),
    }
    findings
}

/// Runs the full `CapabilityLedger` contract suite against one factory.
pub fn capability_ledger_suite(factory: &dyn CapabilityLedgerFactory) -> Vec<ContractFinding> {
    let mut findings = Vec::new();
    findings.extend(observation_appends_are_idempotent_on_identity(factory));
    findings.extend(observations_read_back_exactly_as_appended(factory));
    findings.extend(every_class_appends_ungated(factory));
    findings.extend(first_observation_registers_observed(factory));
    findings.extend(transitions_are_legal_typed_and_durable(factory));
    findings.extend(illegal_and_stale_transitions_are_refused_and_write_nothing(
        factory,
    ));
    findings.extend(revoke_is_sticky(factory));
    findings.extend(elevating_transitions_are_log_rooted(factory));
    findings.extend(a_zero_reason_refusal_does_not_compose(factory));
    findings
}
