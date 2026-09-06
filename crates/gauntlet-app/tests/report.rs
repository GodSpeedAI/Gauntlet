//! Report-generator tests (plan Task 20 step 3, spec §26): the six status
//! categories populate FROM DURABLE RECORDS, never from claims. The teeth:
//! change the records and the report moves; identical records render
//! byte-identically; an all-empty store populates nothing.

use gauntlet_app::report::{CapabilityManifestEntry, StatusCategory, build_report};
use gauntlet_domain::{
    Budget, DiagnosticAllowance, Digest, EpochSeconds, EventLogOffset, EvidenceKind,
    EvidenceLocator, EvidenceRef, IterationNo, Limits, MicroUsd, Outcome, Producer, ReferenceId,
    ReferenceVersion, Run, RunDefinition, RunId, SettlementRecord, UnitId, UnitStatus,
    VerificationRef,
};
use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::json;
use gauntlet_ports::state_store::{
    Checkpoint, RunEvent, RunLifecycleFacts, StateStore, StateStoreError,
};
use gauntlet_ports::verification_record::{
    RequirementDoc, VerificationContent, VerificationRecordDoc, VerifyingInstrument,
};

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

const BASE_INSTANT: u64 = 1_700_000_000;

fn run_id() -> RunId {
    RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("valid run id")
}

fn other_run_id() -> RunId {
    RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAW").expect("valid run id")
}

fn bound() -> Deadline {
    Deadline::at_epoch(EpochSeconds::new(BASE_INSTANT + 60))
}

fn unit(id: &str) -> UnitId {
    UnitId::new(id).expect("valid unit id")
}

fn digest() -> Digest {
    Digest::new("sha256", "a1b2c3d4e5f6a7b8").expect("valid digest")
}

fn budget() -> Budget {
    Budget::new(
        Limits::new(10, 2, 2, 3600, 1000, MicroUsd::new(500)).expect("ceilings"),
        20,
        DiagnosticAllowance::new(100, MicroUsd::new(50)),
    )
    .expect("budget")
}

fn terminated_run() -> Run {
    let definition = RunDefinition {
        id: run_id(),
        created_at: EpochSeconds::new(BASE_INSTANT),
        reference_version: ReferenceVersion::new(
            ReferenceId::new("ref-main").expect("valid id"),
            4,
            digest(),
        ),
        intent: "Prove the loop end to end".to_owned(),
        config_fingerprint: digest(),
        environment_fingerprint: digest(),
        budget: budget(),
        parent_run: None,
    };
    let mut run = Run::new(definition).expect("valid run");
    run.terminate(gauntlet_domain::TerminalState::Settled)
        .expect("runs end once");
    run
}

fn checkpoint(units: Vec<(UnitId, UnitStatus)>) -> Checkpoint {
    Checkpoint {
        run_id: run_id(),
        reference_digest: digest(),
        iteration_no: IterationNo::new(1).expect("iterations are 1-based"),
        unit_states: units,
        budget_spent: gauntlet_domain::Spent::from_counters(
            0,
            0,
            0,
            MicroUsd::new(0),
            0,
            MicroUsd::new(0),
        ),
        in_flight: Vec::new(),
        event_log_offset: EventLogOffset::new(7),
    }
}

fn verification(ref_token: &str, unit_token: &str, outcome: Outcome) -> VerificationRecordDoc {
    VerificationRecordDoc::new(VerificationContent {
        verification: VerificationRef::new(ref_token).expect("valid verification ref"),
        run_id: run_id(),
        iteration_no: IterationNo::new(1).expect("iterations are 1-based"),
        unit_id: unit(unit_token),
        claim: "the unit behaves as its reference requires".to_owned(),
        requirement: RequirementDoc::new(
            EvidenceKind::TestReport,
            EvidenceLocator::new("locator-report-fixture").expect("valid locator"),
            Producer::Instrument,
        ),
        instrument: VerifyingInstrument::new(
            "gauntlet-report-fixture-verifier".to_owned(),
            "1".to_owned(),
            true,
        ),
        evidence: EvidenceRef::new("evd-report-fixture").expect("valid evidence ref"),
        produced_at: EpochSeconds::new(BASE_INSTANT + 42),
        outcome,
        class: None,
        detail: "the instrument decided".to_owned(),
    })
    .expect("well-formed verification record")
}

fn settlement(unit_token: &str, vrf: &str, evd: &str) -> SettlementRecord {
    SettlementRecord::new(
        run_id(),
        unit(unit_token),
        EvidenceRef::new(evd).expect("valid evidence ref"),
        VerificationRef::new(vrf).expect("valid verification ref"),
        EventLogOffset::new(9),
    )
}

/// A read-only scripted store. When `reversed` is set, the settlement and
/// verification rows are handed over in reverse order — adapters differ;
/// the report must not. `facts` scripts the run's lifecycle records
/// (corrections round 2, F-R4): the unit-attributed dispatches, the
/// invocation terminals, and the run's own recorded terminals the
/// attempt-honest classification and the real terminal derive from.
#[derive(Clone)]
struct ScriptedStore {
    run: Option<Run>,
    checkpoint: Option<Checkpoint>,
    settlements: Vec<SettlementRecord>,
    verifications: Vec<VerificationRecordDoc>,
    facts: RunLifecycleFacts,
    reversed: bool,
}

impl StateStore for ScriptedStore {
    fn append_event(
        &self,
        _event: RunEvent,
        _deadline: Deadline,
    ) -> Result<gauntlet_domain::EventLogOffset, StateStoreError> {
        Err(StateStoreError::WriteFailed {
            detail: "the report's store stub is read-only".to_owned(),
        })
    }

    fn load_run(
        &self,
        _run_id: &RunId,
        _deadline: Deadline,
    ) -> Result<Option<Run>, StateStoreError> {
        Ok(self.run.clone())
    }

    fn checkpoint(
        &self,
        _checkpoint: Checkpoint,
        _deadline: Deadline,
    ) -> Result<(), StateStoreError> {
        Err(StateStoreError::WriteFailed {
            detail: "the report's store stub is read-only".to_owned(),
        })
    }

    fn resume(
        &self,
        _run_id: &RunId,
        _deadline: Deadline,
    ) -> Result<Option<Checkpoint>, StateStoreError> {
        Ok(self.checkpoint.clone())
    }

    fn settlements(
        &self,
        _run_id: &RunId,
        _deadline: Deadline,
    ) -> Result<Vec<SettlementRecord>, StateStoreError> {
        let mut rows = self.settlements.clone();
        if self.reversed {
            rows.reverse();
        }
        Ok(rows)
    }

    fn verification_records(
        &self,
        _run_id: &RunId,
        _deadline: Deadline,
    ) -> Result<Vec<VerificationRecordDoc>, StateStoreError> {
        let mut rows = self.verifications.clone();
        if self.reversed {
            rows.reverse();
        }
        Ok(rows)
    }

    fn run_lifecycle_facts(
        &self,
        _run_id: &RunId,
        _deadline: Deadline,
    ) -> Result<RunLifecycleFacts, StateStoreError> {
        Ok(self.facts.clone())
    }

    fn export_jsonl(
        &self,
        _run_id: &RunId,
        _deadline: Deadline,
    ) -> Result<String, StateStoreError> {
        Ok(String::new())
    }
}

fn manifest_entry(key: &str, wired: bool, evidence: Option<&str>) -> CapabilityManifestEntry {
    CapabilityManifestEntry::new(key.to_owned(), wired, evidence.map(unit))
        .expect("valid manifest entry")
}

// ---------------------------------------------------------------------------
// The record-driven population tests
// ---------------------------------------------------------------------------

/// A run with settled+verified units populates implemented-and-proven from
/// records; every other populated category names its record too. The
/// out-of-scope, never-attempted unit is surfaced separately, not invented
/// into a category.
#[test]
fn categories_populate_from_records() {
    let store = ScriptedStore {
        run: None,
        checkpoint: Some(checkpoint(vec![
            (unit("AK-001"), UnitStatus::Settled),
            (unit("AK-002"), UnitStatus::InProgress),
            (unit("AK-003"), UnitStatus::Open),
            (unit("AK-004"), UnitStatus::Blocked),
            (unit("AK-005"), UnitStatus::OutOfScope),
        ])),
        settlements: vec![settlement("AK-001", "vrf-000001", "evd-000001")],
        verifications: vec![
            verification("vrf-000001", "AK-001", Outcome::Pass),
            verification("vrf-000002", "AK-002", Outcome::Inconclusive),
        ],
        reversed: false,
        facts: RunLifecycleFacts::default(),
    };
    let report = build_report(&store, &run_id(), &[], bound()).expect("the report builds");
    assert_eq!(report.count(StatusCategory::ImplementedAndProven), 1);
    assert_eq!(report.count(StatusCategory::Provisional), 1);
    assert_eq!(report.count(StatusCategory::ConfiguredButUntested), 1);
    assert_eq!(report.count(StatusCategory::Blocked), 1);
    assert_eq!(report.count(StatusCategory::Proposed), 0);
    assert_eq!(report.count(StatusCategory::Unknown), 0);
    let proven = report
        .units
        .iter()
        .find(|row| row.unit == unit("AK-001"))
        .expect("AK-001 is classified");
    assert_eq!(proven.category, StatusCategory::ImplementedAndProven);
    assert!(
        proven.basis.contains("vrf-000001") && proven.basis.contains("PASS"),
        "the basis cites the deciding records: {}",
        proven.basis
    );
    assert_eq!(
        report.out_of_scope,
        vec![unit("AK-005")],
        "the never-attempted out-of-scope unit is surfaced, not classified"
    );
    assert!(!report.run_present);
}

/// TEETH: removing the settlement record moves the unit OUT of
/// implemented-and-proven even though its PASS verification record remains —
/// proof is the pair, read from records, not the intention.
#[test]
fn removing_the_settlement_record_moves_the_unit_out_of_implemented_and_proven() {
    let store = ScriptedStore {
        run: None,
        checkpoint: Some(checkpoint(vec![(unit("AK-001"), UnitStatus::Settled)])),
        settlements: Vec::new(),
        verifications: vec![verification("vrf-000001", "AK-001", Outcome::Pass)],
        reversed: false,
        facts: RunLifecycleFacts::default(),
    };
    let report = build_report(&store, &run_id(), &[], bound()).expect("the report builds");
    assert_eq!(
        report.count(StatusCategory::ImplementedAndProven),
        0,
        "a PASS record without its settlement row is not proof"
    );
    assert_eq!(report.count(StatusCategory::Provisional), 1);
    let row = &report.units[0];
    assert_eq!(row.unit, unit("AK-001"));
    assert!(
        row.basis.contains("no settlement record"),
        "the basis names the missing half: {}",
        row.basis
    );
}

/// Blocked is the record's decision: with the projection the unit is
/// blocked, without it the category is empty. Unknown is the caller asking
/// about a unit NO durable record names.
#[test]
fn blocked_and_unknown_are_record_driven() {
    let blocked_projection = ScriptedStore {
        run: None,
        checkpoint: Some(checkpoint(vec![
            (unit("AK-004"), UnitStatus::Blocked),
            (unit("AK-003"), UnitStatus::Open),
        ])),
        settlements: Vec::new(),
        verifications: Vec::new(),
        reversed: false,
        facts: RunLifecycleFacts::default(),
    };
    let report =
        build_report(&blocked_projection, &run_id(), &[], bound()).expect("the report builds");
    assert_eq!(report.count(StatusCategory::Blocked), 1);
    assert_eq!(report.count(StatusCategory::ConfiguredButUntested), 1);

    // The same store without the blocked projection: the category empties.
    let without = ScriptedStore {
        checkpoint: Some(checkpoint(vec![(unit("AK-003"), UnitStatus::Open)])),
        ..blocked_projection.clone()
    };
    let report = build_report(&without, &run_id(), &[], bound()).expect("the report builds");
    assert_eq!(report.count(StatusCategory::Blocked), 0);

    // A manifest entry bound to a unit no record names: unknown, and the
    // units section does not invent the unit.
    let manifest = vec![manifest_entry("inspect", true, Some("AK-099"))];
    let report = build_report(&blocked_projection, &run_id(), &manifest, bound()).expect("builds");
    assert_eq!(
        report.count(StatusCategory::Unknown),
        0,
        "run-scope units are record-named only"
    );
    assert!(report.units.iter().all(|row| row.unit != unit("AK-099")));
    let capability = report
        .capabilities
        .iter()
        .find(|row| row.key == "inspect")
        .expect("the manifest row is reported");
    assert_eq!(capability.category, StatusCategory::Unknown);
    assert!(
        capability.basis.contains("no durable record"),
        "the basis says why nothing is known: {}",
        capability.basis
    );
}

/// The machine rendering is deterministic for identical records: built twice
/// it is byte-identical, and two stores holding the same records hand the
/// rows over in different orders without moving a byte.
#[test]
fn the_machine_rendering_is_deterministic_and_order_independent() {
    let forward = ScriptedStore {
        run: Some(terminated_run()),
        checkpoint: Some(checkpoint(vec![
            (unit("AK-001"), UnitStatus::Settled),
            (unit("AK-002"), UnitStatus::Open),
        ])),
        settlements: vec![settlement("AK-001", "vrf-000001", "evd-000001")],
        verifications: vec![verification("vrf-000001", "AK-001", Outcome::Pass)],
        reversed: false,
        facts: RunLifecycleFacts::default(),
    };
    let reversed = ScriptedStore {
        run: forward.run.clone(),
        checkpoint: forward.checkpoint.clone(),
        // The same rows, handed over backwards.
        settlements: {
            let mut rows = forward.settlements.clone();
            rows.reverse();
            rows
        },
        verifications: {
            let mut rows = forward.verifications.clone();
            rows.reverse();
            rows
        },
        reversed: false,
        facts: RunLifecycleFacts::default(),
    };
    let first = build_report(&forward, &run_id(), &[], bound()).expect("builds");
    let second = build_report(&forward, &run_id(), &[], bound()).expect("builds again");
    assert_eq!(
        first.to_json(),
        second.to_json(),
        "the same records render byte-identically every time"
    );
    let from_reversed_store = build_report(&reversed, &run_id(), &[], bound()).expect("builds");
    assert_eq!(
        first.to_json(),
        from_reversed_store.to_json(),
        "the report is a function of the records, not the hand-over order"
    );
    assert_eq!(first.human_summary(), second.human_summary());
    assert_eq!(first, from_reversed_store);
    // The run's own recorded facts move through, never minted.
    assert!(first.run_present);
    assert_eq!(
        first.terminal_state,
        Some(gauntlet_domain::TerminalState::Settled)
    );
    assert!(
        first.to_json().contains("\"terminal_state\":\"settled\""),
        "{}",
        first.to_json()
    );
}

/// No category is populated without a corresponding record: an all-empty
/// store with an empty manifest populates nothing, and the report is still
/// exit-neutral — it renders, naming every category as empty.
#[test]
fn an_all_empty_store_populates_nothing() {
    let store = ScriptedStore {
        run: None,
        checkpoint: None,
        settlements: Vec::new(),
        verifications: Vec::new(),
        reversed: false,
        facts: RunLifecycleFacts::default(),
    };
    let report = build_report(&store, &run_id(), &[], bound()).expect("the report builds");
    for category in StatusCategory::ALL {
        assert_eq!(
            report.count(*category),
            0,
            "{} is populated without a record",
            category.token()
        );
    }
    assert!(report.units.is_empty() && report.capabilities.is_empty());
    assert!(report.out_of_scope.is_empty());
    assert!(!report.run_present);
    assert_eq!(report.terminal_state, None);
    // The machine rendering is still valid JSON carrying all six tokens,
    // each mapping to an empty array.
    let parsed = json::parse(&report.to_json()).expect("the empty report is valid JSON");
    let categories = parsed
        .get("categories")
        .expect("the rendering always carries the categories object");
    for category in StatusCategory::ALL {
        let bucket = categories
            .get(category.token())
            .unwrap_or_else(|| panic!("the rendering always distinguishes {}", category.token()));
        let empty = match bucket {
            json::JsonValue::Array(items) => items.is_empty(),
            _ => false,
        };
        assert!(
            empty,
            "{} maps to an empty array over an all-empty store",
            category.token()
        );
    }
    for category in StatusCategory::ALL {
        assert!(
            report
                .human_summary()
                .contains(&format!("{} 0", category.token())),
            "the summary counts {category} as empty: {}",
            report.human_summary()
        );
    }
}

/// A settlement row whose verification ref has no durable PASS record is
/// never proven — missing evidence is not passing evidence.
#[test]
fn a_settlement_without_a_durable_pass_record_is_never_proven() {
    let store = ScriptedStore {
        run: None,
        checkpoint: Some(checkpoint(vec![(unit("AK-001"), UnitStatus::Settled)])),
        settlements: vec![settlement("AK-001", "vrf-000001", "evd-000001")],
        verifications: Vec::new(),
        reversed: false,
        facts: RunLifecycleFacts::default(),
    };
    let report = build_report(&store, &run_id(), &[], bound()).expect("the report builds");
    assert_eq!(report.count(StatusCategory::ImplementedAndProven), 0);
    assert_eq!(report.count(StatusCategory::Provisional), 1);
    assert!(
        report.units[0].basis.contains("no durable PASS record"),
        "{}",
        report.units[0].basis
    );
}

/// An INCONCLUSIVE verification record never proves or settles: with the run
/// still working the unit, the honest category is provisional.
#[test]
fn an_inconclusive_record_is_provisional_never_proven() {
    let store = ScriptedStore {
        run: None,
        checkpoint: Some(checkpoint(vec![(unit("AK-002"), UnitStatus::Open)])),
        settlements: Vec::new(),
        verifications: vec![verification("vrf-000002", "AK-002", Outcome::Inconclusive)],
        reversed: false,
        facts: RunLifecycleFacts::default(),
    };
    let report = build_report(&store, &run_id(), &[], bound()).expect("the report builds");
    assert_eq!(report.count(StatusCategory::ImplementedAndProven), 0);
    assert_eq!(report.count(StatusCategory::Provisional), 1);
    assert!(
        report.units[0].basis.contains("INCONCLUSIVE"),
        "{}",
        report.units[0].basis
    );
}

/// Capability rows come from the caller-supplied manifest, crossed with the
/// bound unit's records: wired+proven is proven, wired+unbound is configured-
/// but-untested, not-wired is proposed even when records exist, and a
/// settlement scope (wrong-run records) proves nothing.
#[test]
fn capability_rows_follow_the_manifest_and_the_records() {
    let store = ScriptedStore {
        run: None,
        checkpoint: Some(checkpoint(vec![(unit("AK-001"), UnitStatus::Settled)])),
        settlements: vec![settlement("AK-001", "vrf-000001", "evd-000001")],
        verifications: vec![verification("vrf-000001", "AK-001", Outcome::Pass)],
        reversed: false,
        facts: RunLifecycleFacts::default(),
    };
    let manifest = vec![
        manifest_entry("run", true, Some("AK-001")),
        manifest_entry("status", true, None),
        manifest_entry("improve", false, None),
    ];
    let report = build_report(&store, &run_id(), &manifest, bound()).expect("the report builds");
    let category_of = |key: &str| {
        report
            .capabilities
            .iter()
            .find(|row| row.key == key)
            .map(|row| row.category)
            .unwrap_or_else(|| panic!("{key} is reported"))
    };
    assert_eq!(category_of("run"), StatusCategory::ImplementedAndProven);
    assert_eq!(category_of("status"), StatusCategory::ConfiguredButUntested);
    assert_eq!(
        category_of("improve"),
        StatusCategory::Proposed,
        "a not-wired declaration is never upgraded by records"
    );
    // Rows sort by key, whatever order the caller listed them in.
    let keys: Vec<&str> = report
        .capabilities
        .iter()
        .map(|row| row.key.as_str())
        .collect();
    assert_eq!(keys, vec!["improve", "run", "status"]);

    // Records of a DIFFERENT run are not this run's proof.
    let foreign = ScriptedStore {
        run: None,
        checkpoint: None,
        settlements: vec![SettlementRecord::new(
            other_run_id(),
            unit("AK-001"),
            EvidenceRef::new("evd-000001").expect("valid ref"),
            VerificationRef::new("vrf-000001").expect("valid ref"),
            EventLogOffset::new(9),
        )],
        verifications: Vec::new(),
        reversed: false,
        facts: RunLifecycleFacts::default(),
    };
    let report = build_report(&foreign, &run_id(), &[], bound()).expect("the report builds");
    assert_eq!(
        report.count(StatusCategory::ImplementedAndProven),
        0,
        "another run's settlement row is not this run's proof"
    );
    assert!(
        report.units.is_empty(),
        "foreign records name no unit of this run"
    );
}

/// Duplicate manifest keys are a typed refusal — an ambiguous report is not
/// a report.
#[test]
fn duplicate_manifest_keys_are_a_typed_error() {
    let store = ScriptedStore {
        run: None,
        checkpoint: None,
        settlements: Vec::new(),
        verifications: Vec::new(),
        reversed: false,
        facts: RunLifecycleFacts::default(),
    };
    let manifest = vec![
        manifest_entry("run", true, None),
        manifest_entry("run", false, None),
    ];
    let error = build_report(&store, &run_id(), &manifest, bound()).expect_err("refused");
    assert!(
        matches!(
            error,
            gauntlet_app::report::ReportError::DuplicateManifestKey { ref key } if key.as_str() == "run"
        ),
        "{error}"
    );
}

/// F-R4 (corrections round 2): an ATTEMPTED unit is provisional with its
/// record-derived attempt basis — never configured-but-untested, never
/// "never attempted" — even though the checkpoint still projects it open;
/// a unit with NO dispatch on record says exactly that; and the report's
/// terminal is the run's RECORDED terminal (the latest `RunTerminal`
/// event), not a null.
#[test]
fn an_attempted_unit_is_provisional_with_the_record_derived_attempt_basis() {
    let invocation =
        |token: &str| gauntlet_domain::InvocationId::new(token).expect("valid invocation id");
    let store = ScriptedStore {
        run: None,
        checkpoint: Some(checkpoint(vec![
            (unit("AK-001"), UnitStatus::Open),
            (unit("AK-003"), UnitStatus::Open),
        ])),
        settlements: Vec::new(),
        verifications: Vec::new(),
        facts: RunLifecycleFacts {
            unit_dispatches: vec![
                (unit("AK-001"), invocation("01ARZ3NDEKTSV4RRFFQ69G5FB1")),
                (unit("AK-001"), invocation("01ARZ3NDEKTSV4RRFFQ69G5FB2")),
            ],
            invocation_terminals: vec![
                (
                    invocation("01ARZ3NDEKTSV4RRFFQ69G5FB1"),
                    gauntlet_domain::InvocationState::TimedOut,
                ),
                (
                    invocation("01ARZ3NDEKTSV4RRFFQ69G5FB2"),
                    gauntlet_domain::InvocationState::TimedOut,
                ),
            ],
            run_terminals: vec![gauntlet_domain::TerminalState::Escalated],
        },
        reversed: false,
    };
    let report = build_report(&store, &run_id(), &[], bound()).expect("the report builds");
    assert_eq!(report.count(StatusCategory::Provisional), 1);
    assert_eq!(report.count(StatusCategory::ConfiguredButUntested), 1);
    let attempted = report
        .units
        .iter()
        .find(|row| row.unit == unit("AK-001"))
        .expect("the attempted unit is classified");
    assert_eq!(
        attempted.category,
        StatusCategory::Provisional,
        "recorded dispatches without a settlement are provisional, never \
         configured-but-untested"
    );
    assert!(
        attempted.basis.contains("attempted-and-timed-out")
            && attempted.basis.contains("timed_out"),
        "the basis names the records' attempt disposition: {}",
        attempted.basis
    );
    assert!(
        !attempted.basis.contains("never attempted"),
        "an attempted unit is never described as never-attempted: {}",
        attempted.basis
    );
    let unattempted = report
        .units
        .iter()
        .find(|row| row.unit == unit("AK-003"))
        .expect("the never-dispatched unit is classified");
    assert_eq!(unattempted.category, StatusCategory::ConfiguredButUntested);
    assert!(
        unattempted.basis.contains("no dispatch attempt"),
        "the basis states the unit's own record truth: {}",
        unattempted.basis
    );
    // The run's REAL terminal: the latest recorded RunTerminal — a run
    // whose pass ended escalated reports escalated, never a null.
    assert_eq!(
        report.terminal_state,
        Some(gauntlet_domain::TerminalState::Escalated)
    );
}
