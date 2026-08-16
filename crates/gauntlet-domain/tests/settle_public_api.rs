//! Exercises `Unit::settle` from OUTSIDE the crate, through the public API
//! only (spec §5.3.2, §16.1: settlement requires a `VerificationRecord` with
//! `Outcome = PASS` "for this unit id **in this run**").
//!
//! This file is also the teeth for the crate-root re-export of
//! `VerificationRecord`: it is the parameter type of the public settle path,
//! so if the export is removed this file stops compiling — the app layer
//! could not name the type at all.

use gauntlet_domain::{
    Authority, BlastRadius, EvidenceKind, EvidenceLocator, EvidenceRef, EvidenceRequirement,
    InstrumentName, Outcome, Producer, RunId, Unit, UnitError, UnitId, UnitStatus,
    VerificationRecord, VerificationRef, VerifyBy,
};

fn unit() -> Unit {
    Unit::new(
        UnitId::new("AK-014").expect("valid unit id"),
        "The settle path is callable from outside the crate.".to_owned(),
        VerifyBy::Test,
        BlastRadius::Contained,
        Authority::LeadAgent,
        EvidenceRequirement::new(
            EvidenceKind::TestReport,
            EvidenceLocator::new("locator-0001").expect("valid locator"),
            Producer::Instrument,
        ),
        Vec::new(),
    )
    .expect("legal unit")
}

fn record(run: RunId, outcome: Outcome) -> VerificationRecord {
    VerificationRecord::new(
        VerificationRef::new("vrf-000042").expect("valid verification ref"),
        run,
        UnitId::new("AK-014").expect("valid unit id"),
        EvidenceRef::new("evd-000042").expect("valid evidence ref"),
        outcome,
        InstrumentName::new("verifier-01").expect("valid instrument name"),
    )
}

#[test]
fn settle_refuses_a_foreign_run_pass_record_and_accepts_a_same_run_one() {
    let this_run = RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("valid run id");
    let foreign_run = RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAZ").expect("valid run id");
    let mut unit = unit();
    let evidence = EvidenceRef::new("evd-000042").expect("valid evidence ref");

    // A PASS record replayed from a different run is stale evidence (§16.1):
    // refused with the typed failure locus, and the Unit stays open.
    let stale = record(foreign_run.clone(), Outcome::Pass);
    assert_eq!(
        unit.settle(this_run.clone(), evidence.clone(), &stale),
        Err(UnitError::VerificationFromDifferentRun {
            expected: this_run.clone(),
            actual: foreign_run,
        })
    );
    assert_eq!(unit.status(), UnitStatus::Open);

    // A PASS record from this run settles the Unit.
    let live = record(this_run.clone(), Outcome::Pass);
    unit.settle(this_run, evidence, &live)
        .expect("a same-run PASS record settles");
    assert_eq!(unit.status(), UnitStatus::Settled);
}
