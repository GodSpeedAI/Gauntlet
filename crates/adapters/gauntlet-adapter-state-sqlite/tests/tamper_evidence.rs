//! Out-of-band damage and tamper-injection tests (Task 6 correction round).
//!
//! Every test here writes to the database file through a second RAW
//! connection — the one instrument in this crate allowed to issue SQL the
//! adapter itself never issues (tamper rows with impossible values, a gap
//! DELETE, a failure-injection trigger). The adapter's production sources
//! stay INSERT/SELECT only; this file plays the same instrument role for
//! tamper evidence that `crash_helper`'s `corrupt-tail` plays for crash
//! tests: the assertions are that damage is a TYPED error naming the
//! damaged row — never a silently normalized value, never a silent gap
//! skip, never a caller-attested settlement row (spec §18.3, §6.2, §5.3.6).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use gauntlet_adapter_state_sqlite::{SettlementCommit, SqliteEventLog, SqliteStateStore};
use gauntlet_domain::{
    Budget, DiagnosticAllowance, Digest, EpochSeconds, EventLogOffset, IterationNo, Limits,
    MicroUsd, ReferenceId, ReferenceVersion, Run, RunDefinition, RunId, Spent, UnitId, UnitStatus,
    VerificationRef,
};
use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::event_log::{EventLog, EventLogError};
use gauntlet_ports::state_store::{
    Checkpoint, RunEvent, RunEventKind, StateStore, StateStoreError,
};

const RUN: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
const UNIT: &str = "AK-900";
const DIGEST_HEX: &str = "c4d3e2f1a0b1c2d3";

static SCRATCH_SEQ: AtomicU32 = AtomicU32::new(0);

struct Scratch {
    path: PathBuf,
}

impl Scratch {
    fn new(case: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "gauntlet-tamper-{case}-{}-{}",
            std::process::id(),
            SCRATCH_SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::remove_dir_all(&path).ok();
        std::fs::create_dir_all(&path).expect("tamper scratch parent is creatable");
        Self { path }
    }

    fn db(&self) -> PathBuf {
        self.path.join("state.db")
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.path).ok();
    }
}

fn run_id() -> RunId {
    RunId::new(RUN).expect("valid run id")
}

fn bound() -> Deadline {
    Deadline::at_epoch(EpochSeconds::new(1_700_000_060))
}

fn digest() -> Digest {
    Digest::new("sha256", DIGEST_HEX).expect("valid digest")
}

fn zero_spent() -> Spent {
    *Budget::new(
        Limits::new(1, 1, 1, 1, 1, MicroUsd::new(1)).expect("minimal legal ceilings"),
        1,
        DiagnosticAllowance::new(1, MicroUsd::new(1)),
    )
    .expect("minimal legal budget")
    .spent()
}

fn run() -> Run {
    Run::new(RunDefinition {
        id: run_id(),
        created_at: EpochSeconds::new(1_700_000_000),
        reference_version: ReferenceVersion::new(
            ReferenceId::new("ref-task6").expect("valid reference id"),
            3,
            digest(),
        ),
        intent: "settle the task".to_owned(),
        config_fingerprint: Digest::new("sha256", "a1b2c3d4e5f6a7b8").expect("valid digest"),
        environment_fingerprint: Digest::new("sha256", "e5f6a7b8c4d3e2f1").expect("valid digest"),
        budget: Budget::new(
            Limits::new(8, 2, 2, 3600, 1000, MicroUsd::new(500)).expect("legal ceilings"),
            20,
            DiagnosticAllowance::new(100, MicroUsd::new(50)),
        )
        .expect("legal budget"),
        parent_run: None,
    })
    .expect("legal run")
}

fn commit() -> SettlementCommit {
    SettlementCommit {
        run_id: run_id(),
        unit_id: UnitId::new(UNIT).expect("valid unit id"),
        evidence: gauntlet_domain::EvidenceRef::new("evd-000900").expect("valid ref"),
        verification: VerificationRef::new("vrf-000900").expect("valid ref"),
        reference_digest: digest(),
        at: EpochSeconds::new(1_700_000_101),
    }
}

fn admitted_event(at: u64) -> RunEvent {
    RunEvent {
        run_id: run_id(),
        kind: RunEventKind::InvocationAdmitted {
            invocation_id: gauntlet_domain::InvocationId::new("01ARZ3NDEKTSV4RRFFQ69G5FAW")
                .expect("valid invocation id"),
        },
        at: EpochSeconds::new(at),
    }
}

fn settlement_event(at: u64) -> RunEvent {
    RunEvent {
        run_id: run_id(),
        kind: RunEventKind::SettlementCommitted {
            unit_id: UnitId::new(UNIT).expect("valid unit id"),
            evidence: gauntlet_domain::EvidenceRef::new("evd-000900").expect("valid ref"),
            verification: VerificationRef::new("vrf-000900").expect("valid ref"),
            reference_digest: digest(),
        },
        at: EpochSeconds::new(at),
    }
}

/// A raw second connection to the same file — the damage instrument.
fn raw(db: &Path) -> rusqlite::Connection {
    rusqlite::Connection::open(db).expect("the raw instrument connection opens")
}

/// One valid event_log row's (kind, payload), for tamper INSERTs that stay
/// decodable so the test isolates exactly one damaged column.
const ADMITTED_KIND: &str = "invocation_admitted";
const ADMITTED_PAYLOAD: &str =
    "{\"kind\":\"invocation_admitted\",\"invocation_id\":\"01ARZ3NDEKTSV4RRFFQ69G5FAW\"}";

// ---------------------------------------------------------------------------
// Item 2 — negative timestamps are damage, not clamped to epoch 0
// ---------------------------------------------------------------------------

/// A negative `at` in the event log is `EventLogError::Damaged` naming the
/// offset — the adapter can never write one, so any negative read is tamper
/// evidence that must not be silently rewritten to epoch 0.
#[test]
fn a_negative_event_timestamp_is_damage_naming_the_offset() {
    let scratch = Scratch::new("negative-at");
    let db = scratch.db();
    let store = SqliteStateStore::open(&db).expect("the store opens");
    store
        .append_event(admitted_event(1_700_000_000), bound())
        .expect("the valid event appends");
    drop(store);
    raw(&db)
        .execute(
            "INSERT INTO event_log (run_id, kind, payload, at) VALUES (?1, ?2, ?3, ?4)",
            (RUN, ADMITTED_KIND, ADMITTED_PAYLOAD, -5_i64),
        )
        .expect("the tamper row lands");
    let log = SqliteEventLog::open(&db).expect("the log reopens");
    match log.read_from(EventLogOffset::new(0), bound()) {
        Err(EventLogError::Damaged { detail }) => {
            assert!(
                detail.contains("offset 2") && detail.contains("negative"),
                "the refusal names where and what: {detail}"
            );
        }
        other => panic!("a negative timestamp must be Damaged, got {other:?}"),
    }
}

/// A negative `created_at` in the runs row is `StateStoreError::Corrupt`
/// naming the run — never a projection of epoch 0.
#[test]
fn a_negative_created_at_is_corrupt_naming_the_run() {
    let scratch = Scratch::new("negative-created");
    let db = scratch.db();
    let store = SqliteStateStore::open(&db).expect("the store opens");
    store.record_run(&run(), bound()).expect("run records");
    drop(store);
    raw(&db)
        .execute(
            "INSERT INTO runs (
                run_id, created_at, reference_id, reference_version, reference_digest,
                intent, config_fingerprint, environment_fingerprint, parent_run, budget
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            (
                "01ARZ3NDEKTSV4RRFFQ69G5FAZ",
                -1_i64,
                "ref-task6",
                3,
                format!("sha256:{DIGEST_HEX}"),
                "settle the task",
                "sha256:a1b2c3d4e5f6a7b8",
                "sha256:e5f6a7b8c4d3e2f1",
                Option::<String>::None,
                "{\"recovery_reserve_pct\":20,\"limits\":{\"rounds\":8,\"parallel\":2,\"depth\":2,\
                 \"wall_seconds\":3600,\"tokens\":1000,\"cost_usd\":500},\"diagnostic\":{\"tokens\":\
                 100,\"cost_usd\":50}}",
            ),
        )
        .expect("the tamper row lands");
    let store = SqliteStateStore::open(&db).expect("the store reopens");
    let tampered = RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAZ").expect("valid run id");
    match store.load_run(&tampered, bound()) {
        Err(StateStoreError::Corrupt { detail }) => {
            assert!(
                detail.contains("negative") && detail.contains("created_at"),
                "the refusal names the row and the damage: {detail}"
            );
        }
        other => panic!("a negative created_at must be Corrupt, got {other:?}"),
    }
}

/// A negative first-event `MIN(at)` in the checkpoint-only projection is
/// `Corrupt` naming the run — the projection refuses damage too.
#[test]
fn a_negative_projection_timestamp_is_corrupt_naming_the_run() {
    let scratch = Scratch::new("negative-min");
    let db = scratch.db();
    let store = SqliteStateStore::open(&db).expect("the store opens");
    store
        .checkpoint(
            Checkpoint {
                run_id: run_id(),
                reference_digest: digest(),
                iteration_no: IterationNo::new(1).expect("1-based"),
                unit_states: vec![(UnitId::new(UNIT).expect("valid unit id"), UnitStatus::Open)],
                budget_spent: zero_spent(),
                in_flight: Vec::new(),
                event_log_offset: EventLogOffset::new(0),
            },
            bound(),
        )
        .expect("the checkpoint writes");
    drop(store);
    raw(&db)
        .execute(
            "INSERT INTO event_log (run_id, kind, payload, at) VALUES (?1, ?2, ?3, ?4)",
            (RUN, ADMITTED_KIND, ADMITTED_PAYLOAD, -9_i64),
        )
        .expect("the tamper row lands");
    let store = SqliteStateStore::open(&db).expect("the store reopens");
    match store.load_run(&run_id(), bound()) {
        Err(StateStoreError::Corrupt { detail }) => {
            assert!(
                detail.contains("negative") && detail.contains(&run_id().to_string()),
                "the refusal names the run and the damage: {detail}"
            );
        }
        other => panic!("a negative MIN(at) must be Corrupt, got {other:?}"),
    }
}

/// A negative `at` on a records row is `Corrupt` naming the row's sequence —
/// the export refuses it rather than exporting epoch 0.
#[test]
fn a_negative_record_timestamp_is_corrupt_naming_the_row() {
    let scratch = Scratch::new("negative-record");
    let db = scratch.db();
    let store = SqliteStateStore::open(&db).expect("the store opens");
    store.record_run(&run(), bound()).expect("run records");
    drop(store);
    raw(&db)
        .execute(
            "INSERT INTO records (run_id, record_type, payload, at) VALUES (?1, ?2, ?3, ?4)",
            (RUN, "observation", "{\"facet\":\"filesystem\"}", -7_i64),
        )
        .expect("the tamper row lands");
    let store = SqliteStateStore::open(&db).expect("the store reopens");
    match store.export_jsonl(&run_id(), bound()) {
        Err(StateStoreError::Corrupt { detail }) => {
            assert!(
                detail.contains("negative") && detail.contains("seq 1"),
                "the refusal names the damaged row: {detail}"
            );
        }
        other => panic!("a negative record timestamp must be Corrupt, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Item 3 — settlement-row offsets are cross-validated against the log
// ---------------------------------------------------------------------------

/// `apply_settlement_row` with an offset that holds a DIFFERENT event is a
/// typed refusal naming what was claimed and what is there; a settlement
/// row must name its own durable settlement event (spec §5.3.6).
#[test]
fn a_settlement_row_pointing_at_a_foreign_event_is_refused() {
    let scratch = Scratch::new("foreign-offset");
    let db = scratch.db();
    let store = SqliteStateStore::open(&db).expect("the store opens");
    let offset = store
        .append_event(admitted_event(1_700_000_000), bound())
        .expect("the admitted event appends at offset 1");
    assert_eq!(offset.get(), 1);
    match store.apply_settlement_row(commit(), offset, bound()) {
        Err(StateStoreError::WriteFailed { detail }) => {
            assert!(
                detail.contains("offset 1") && detail.contains(ADMITTED_KIND),
                "the refusal names the claimed offset and the event actually there: {detail}"
            );
        }
        other => panic!("a foreign offset must be a typed refusal, got {other:?}"),
    }
    // Nothing landed: the row does not exist.
    assert!(
        store
            .settlements(&run_id(), bound())
            .expect("reads")
            .is_empty(),
        "the refused settlement row must not exist"
    );
}

/// `apply_settlement_row` with an offset BEYOND the log is a typed refusal
/// naming the missing row.
#[test]
fn a_settlement_row_pointing_past_the_log_is_refused() {
    let scratch = Scratch::new("past-log");
    let db = scratch.db();
    let store = SqliteStateStore::open(&db).expect("the store opens");
    match store.apply_settlement_row(commit(), EventLogOffset::new(99), bound()) {
        Err(StateStoreError::WriteFailed { detail }) => {
            assert!(
                detail.contains("99") && detail.contains("no row"),
                "the refusal names the missing offset: {detail}"
            );
        }
        other => panic!("a missing offset must be a typed refusal, got {other:?}"),
    }
    assert!(
        store
            .settlements(&run_id(), bound())
            .expect("reads")
            .is_empty(),
        "the refused settlement row must not exist"
    );
}

/// The happy path still works: applying a row at the offset of ITS OWN
/// settlement event lands exactly once.
#[test]
fn a_settlement_row_at_its_own_event_offset_lands() {
    let scratch = Scratch::new("own-offset");
    let db = scratch.db();
    let store = SqliteStateStore::open(&db).expect("the store opens");
    let offset = store
        .append_event(settlement_event(1_700_000_101), bound())
        .expect("the settlement event appends");
    let added = store
        .apply_settlement_row(commit(), offset, bound())
        .expect("the row applies at its own event's offset");
    assert_eq!(added, 1);
    let rows = store.settlements(&run_id(), bound()).expect("reads");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].event_log_offset(), offset);
}

// ---------------------------------------------------------------------------
// Item 4 — the half-settled error names the durable state
// ---------------------------------------------------------------------------

/// When `commit_settlement`'s step 2 fails after step 1 is durable, the
/// error detail must state the offset of the durable settlement event, that
/// replay will complete it, and that re-running the unit is wrong. The
/// step-2 failure is injected with a trigger that makes every settlements
/// insert fail (a missing table in the body is a hard error INSERT OR
/// IGNORE does not swallow), then removed so replay can complete.
#[test]
fn a_half_settled_failure_names_the_durable_offset_and_the_recovery() {
    let scratch = Scratch::new("half-settled-wording");
    let db = scratch.db();
    let store = SqliteStateStore::open(&db).expect("the store opens");
    drop(store);
    let injector = raw(&db);
    injector
        .execute_batch(
            "CREATE TRIGGER refuse_settlement_rows AFTER INSERT ON settlements
             BEGIN
                 INSERT INTO gauntlet_injection_no_such_table VALUES (1);
             END;",
        )
        .expect("the failure-injection trigger installs");
    drop(injector);

    let store = SqliteStateStore::open(&db).expect("the store reopens");
    match store.commit_settlement(commit(), bound()) {
        Err(StateStoreError::WriteFailed { detail }) => {
            assert!(
                detail.contains("durable at offset 1"),
                "the error names where the settlement event IS durable: {detail}"
            );
            assert!(
                detail.contains("complete_pending_settlements"),
                "the error names the recovery: {detail}"
            );
            assert!(
                detail.contains("duplicate"),
                "the error warns against re-running the unit: {detail}"
            );
        }
        other => panic!("an injected step-2 failure must be WriteFailed, got {other:?}"),
    }
    // Step 1's promise held: the settlement event IS durable.
    let log = SqliteEventLog::open(&db).expect("the log opens");
    assert_eq!(
        log.last_offset(bound()).expect("reads"),
        Some(EventLogOffset::new(1)),
        "the settlement event survived the step-2 failure"
    );
    assert!(
        store
            .settlements(&run_id(), bound())
            .expect("reads")
            .is_empty(),
        "the settlement row did not land"
    );
    // Remove the injection and replay: the guidance in the error is real.
    drop(store);
    raw(&db)
        .execute_batch("DROP TRIGGER refuse_settlement_rows;")
        .expect("the injection is removed");
    let store = SqliteStateStore::open(&db).expect("the store reopens");
    assert_eq!(
        store
            .complete_pending_settlements(&run_id(), bound())
            .expect("replay works"),
        1,
        "replay completes the half-settled unit exactly as the error said it would"
    );
}

// ---------------------------------------------------------------------------
// Item 5 — log damage surfaces as Corrupt through the store surface
// ---------------------------------------------------------------------------

/// A damaged log row is `StateStoreError::Corrupt` (stored state that does
/// not decode) through both store methods that read the log, with the
/// damage detail preserved — not `ReadFailed`, which loses the class.
#[test]
fn log_damage_surfaces_as_corrupt_through_the_store() {
    let scratch = Scratch::new("damage-class");
    let db = scratch.db();
    let store = SqliteStateStore::open(&db).expect("the store opens");
    store.record_run(&run(), bound()).expect("run records");
    drop(store);
    raw(&db)
        .execute(
            "INSERT INTO event_log (run_id, kind, payload, at) VALUES (?1, ?2, ?3, ?4)",
            (
                RUN,
                "settlement_committed",
                "{\"kind\":\"settlement_committed\",\"unit_id\":",
                1_700_000_001_i64,
            ),
        )
        .expect("the damaged row lands");
    let store = SqliteStateStore::open(&db).expect("the store reopens");
    match store.export_jsonl(&run_id(), bound()) {
        Err(StateStoreError::Corrupt { detail }) => {
            assert!(
                detail.contains("damaged") && detail.contains("offset 1"),
                "the damage class and locus survive the mapping: {detail}"
            );
        }
        other => panic!("log damage on export must be Corrupt, got {other:?}"),
    }
    match store.complete_pending_settlements(&run_id(), bound()) {
        Err(StateStoreError::Corrupt { detail }) => {
            assert!(
                detail.contains("damaged"),
                "the damage class survives the mapping: {detail}"
            );
        }
        other => panic!("log damage on replay must be Corrupt, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Item 6 — offset gaps on replay are damage
// ---------------------------------------------------------------------------

/// A deleted middle row (impossible through the adapter, possible by
/// tamper) is a typed `Damaged` error naming the gap — contiguity is
/// checked from the first returned row forward, so replaying from a point
/// at or after the gap stays green, and a contiguous log replays green.
#[test]
fn an_offset_gap_is_damage_naming_the_hole() {
    let scratch = Scratch::new("offset-gap");
    let db = scratch.db();
    let store = SqliteStateStore::open(&db).expect("the store opens");
    for sequence in 0..3u64 {
        store
            .append_event(admitted_event(1_700_000_000 + sequence), bound())
            .expect("appends");
    }
    drop(store);
    let log = SqliteEventLog::open(&db).expect("the log opens");
    assert_eq!(
        log.read_from(EventLogOffset::new(0), bound())
            .expect("a contiguous log replays")
            .len(),
        3,
        "the pre-tamper log is contiguous and green"
    );
    drop(log);
    raw(&db)
        .execute("DELETE FROM event_log WHERE offset = 2", [])
        .expect("the middle row is removed out of band");
    let log = SqliteEventLog::open(&db).expect("the log reopens");
    match log.read_from(EventLogOffset::new(0), bound()) {
        Err(EventLogError::Damaged { detail }) => {
            assert!(
                detail.contains("expected offset 2") && detail.contains("found 3"),
                "the refusal names the hole: {detail}"
            );
        }
        other => panic!("a gap must be Damaged, got {other:?}"),
    }
    // Contiguity is checked from the FIRST RETURNED row forward: a replay
    // starting at or after the gap sees a contiguous tail and stays green.
    let tail = log
        .read_from(EventLogOffset::new(3), bound())
        .expect("the tail past the gap is contiguous");
    assert_eq!(tail.len(), 1);
    assert_eq!(tail[0].offset.get(), 3);
}

// ---------------------------------------------------------------------------
// Item 7 — record_run checks the full identity, not just the digest
// ---------------------------------------------------------------------------

/// Re-recording with the SAME digest but a DIFFERENT intent is a typed
/// refusal naming the differing field and both values — a recorded run's
/// identity never changes, and a silent no-op would mask the disagreement.
#[test]
fn record_run_refuses_a_changed_intent_on_the_same_digest() {
    let scratch = Scratch::new("identity-intent");
    let db = scratch.db();
    let store = SqliteStateStore::open(&db).expect("the store opens");
    store.record_run(&run(), bound()).expect("run records");
    let moved = RunDefinition {
        id: run_id(),
        created_at: EpochSeconds::new(1_700_000_000),
        reference_version: ReferenceVersion::new(
            ReferenceId::new("ref-task6").expect("valid reference id"),
            3,
            digest(),
        ),
        intent: "settle the task differently".to_owned(),
        config_fingerprint: Digest::new("sha256", "a1b2c3d4e5f6a7b8").expect("valid digest"),
        environment_fingerprint: Digest::new("sha256", "e5f6a7b8c4d3e2f1").expect("valid digest"),
        budget: run().budget().clone(),
        parent_run: None,
    };
    let moved = Run::new(moved).expect("legal run");
    match store.record_run(&moved, bound()) {
        Err(StateStoreError::WriteFailed { detail }) => {
            assert!(
                detail.contains("intent"),
                "the refusal names the differing field: {detail}"
            );
            assert!(
                detail.contains("settle the task")
                    && detail.contains("settle the task differently"),
                "the refusal carries both values: {detail}"
            );
        }
        other => panic!("an identity disagreement must be a typed refusal, got {other:?}"),
    }
}
