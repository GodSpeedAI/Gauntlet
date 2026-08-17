//! Crash-injection recovery tests (plan Task 6 step 5; spec §16.3, §18.3):
//! a REAL child process performs the scripted sequence and is killed by
//! `std::process::abort()` at a named step — then this test reopens the file
//! and asserts what survived.
//!
//! - the log is readable and truncation-free after a crash mid-append;
//! - a half-settled run (§16.3 crash between steps 1 and 2) completes the
//!   settlement exactly once on replay, idempotent by
//!   `(run_id, unit_id, verification_ref)`;
//! - a crash BEFORE the event append leaves the unit retryable — nothing
//!   durable happened;
//! - a damaged tail is a typed `Damaged` error, never a silent skip.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};

use gauntlet_adapter_state_sqlite::{SqliteEventLog, SqliteStateStore};
use gauntlet_domain::{
    Budget, DiagnosticAllowance, EpochSeconds, EventLogOffset, IterationNo, Limits, MicroUsd,
    RunId, Spent, UnitId, UnitStatus,
};
use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::event_log::{EventLog, EventLogError};
use gauntlet_ports::state_store::{Checkpoint, RunEventKind, StateStore};

const HELPER: &str = env!("CARGO_BIN_EXE_crash_helper");
const RUN: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
const UNIT: &str = "AK-900";

static SCRATCH_SEQ: AtomicU32 = AtomicU32::new(0);

struct Scratch {
    path: PathBuf,
}

impl Scratch {
    fn new(case: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "gauntlet-crash-{case}-{}-{}",
            std::process::id(),
            SCRATCH_SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::remove_dir_all(&path).ok();
        std::fs::create_dir_all(&path).expect("crash scratch parent is creatable");
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

fn helper(script: &str, db: &Path, rest: &[&str]) -> std::process::ExitStatus {
    let db_text = db.to_str().expect("the scratch path is UTF-8");
    let mut command = Command::new(HELPER);
    command.arg(script).arg(db_text).args(rest);
    command.status().expect("the crash helper binary runs")
}

/// Reopens the log over the crashed file — a fresh connection, exactly what
/// a restarting process does.
fn log(db: &Path) -> SqliteEventLog {
    SqliteEventLog::open(db).expect("the log reopens")
}

/// A zero `Spent` source for seeding checkpoints: the domain has no other
/// constructor, and every representable checkpoint today carries zero spend.
fn zero_spent() -> Spent {
    *Budget::new(
        Limits::new(1, 1, 1, 1, 1, MicroUsd::new(1)).expect("minimal legal ceilings"),
        1,
        DiagnosticAllowance::new(1, MicroUsd::new(1)),
    )
    .expect("minimal legal budget")
    .spent()
}

/// A crash in the MIDDLE of an append sequence: the log reopens readable,
/// holds exactly the appends that returned, and replays them in order with
/// no truncation and no phantom tail.
#[test]
fn a_mid_sequence_crash_leaves_the_log_readable_and_truncation_free() {
    let scratch = Scratch::new("mid-append");
    let db = scratch.db();
    let status = helper("append-events", &db, &[RUN, "6", "4"]);
    assert!(
        !status.success(),
        "the helper must die at the named step, not exit cleanly"
    );
    let store = SqliteStateStore::open(&db).expect("the database reopens");
    assert_eq!(
        store.resume(&run_id(), bound()).expect("resume works"),
        None,
        "the crash left no checkpoint, and the store still answers"
    );
    assert_eq!(
        log(&db).last_offset(bound()).expect("the log is readable"),
        Some(EventLogOffset::new(4)),
        "exactly the four appends that returned are durable"
    );
    let replay = log(&db)
        .read_from(EventLogOffset::new(0), bound())
        .expect("the full log replays");
    assert_eq!(replay.len(), 4, "no truncation, no phantom tail");
    for (index, entry) in replay.iter().enumerate() {
        assert_eq!(entry.offset.get(), u64::try_from(index).unwrap_or(0) + 1);
        assert!(matches!(
            entry.event.kind,
            RunEventKind::InvocationAdmitted { .. }
        ));
    }
}

/// §16.3's own scenario: crash AFTER the event append, BEFORE the settlement
/// row. Replay completes the settlement exactly once; replaying again is a
/// no-op on the same key.
#[test]
fn a_half_settled_run_completes_exactly_once_on_replay() {
    let scratch = Scratch::new("half-settle");
    let db = scratch.db();
    let status = helper("settle", &db, &[RUN, UNIT, "after-append"]);
    assert!(
        !status.success(),
        "the helper must die between the two §16.3 steps"
    );
    let store = SqliteStateStore::open(&db).expect("the database reopens");
    // The event IS durable — that is what step 1 promised.
    assert_eq!(
        log(&db).last_offset(bound()).unwrap(),
        Some(EventLogOffset::new(1)),
        "the settlement event survived the crash"
    );
    // The settlement row is NOT there yet — the crash was between steps.
    assert!(
        store.settlements(&run_id(), bound()).unwrap().is_empty(),
        "the half-applied settlement is not yet a row"
    );
    // §18.3 resume step 1: replay completes it, exactly once.
    let applied = store
        .complete_pending_settlements(&run_id(), bound())
        .expect("replay works");
    assert_eq!(applied, 1, "the first replay applies the settlement");
    let rows = store.settlements(&run_id(), bound()).expect("reads");
    assert_eq!(rows.len(), 1, "exactly one settlement row for the key");
    assert_eq!(rows[0].unit_id().as_str(), UNIT);
    assert_eq!(rows[0].verification().as_str(), "vrf-crash-0001");
    assert_eq!(rows[0].event_log_offset(), EventLogOffset::new(1));
    // Replaying twice is a no-op (§16.3: keyed idempotency).
    let again = store
        .complete_pending_settlements(&run_id(), bound())
        .expect("replay works");
    assert_eq!(again, 0, "the second replay applies nothing");
    assert_eq!(
        store.settlements(&run_id(), bound()).unwrap().len(),
        1,
        "the (run, unit, verification) key still names one row"
    );
}

/// A crash BEFORE the event append: nothing durable happened, so the unit is
/// retryable — no settlement event, no settlement row, and the checkpoint's
/// unit projection still shows the unit open.
#[test]
fn a_crash_before_the_event_append_leaves_the_unit_retryable() {
    let scratch = Scratch::new("before-append");
    let db = scratch.db();
    // Seed durable state the crash must not disturb: a checkpoint with the
    // unit open.
    let seed = SqliteStateStore::open(&db).expect("the seed store opens");
    seed.checkpoint(
        Checkpoint {
            run_id: run_id(),
            reference_digest: gauntlet_domain::Digest::new("sha256", "c4d3e2f1a0b1c2d3")
                .expect("valid digest"),
            iteration_no: IterationNo::new(1).expect("1-based"),
            unit_states: vec![(UnitId::new(UNIT).expect("valid unit id"), UnitStatus::Open)],
            budget_spent: zero_spent(),
            in_flight: Vec::new(),
            event_log_offset: EventLogOffset::new(0),
        },
        bound(),
    )
    .expect("the seed checkpoint writes");
    drop(seed);

    let status = helper("settle", &db, &[RUN, UNIT, "before-append"]);
    assert!(
        !status.success(),
        "the helper must die before anything durable"
    );
    let store = SqliteStateStore::open(&db).expect("the database reopens");
    assert!(
        store.settlements(&run_id(), bound()).unwrap().is_empty(),
        "no settlement row exists — the unit is retryable"
    );
    assert_eq!(
        log(&db).last_offset(bound()).unwrap(),
        None,
        "no settlement event was appended"
    );
    let resumed = store
        .resume(&run_id(), bound())
        .expect("resume works")
        .expect("the seed checkpoint is there");
    assert_eq!(
        resumed.unit_states,
        vec![(UnitId::new(UNIT).expect("valid unit id"), UnitStatus::Open)],
        "the unit projection still shows the unit open"
    );
}

/// The positive control: an uninterrupted settle run needs no replay, and
/// replaying it afterwards adds nothing.
#[test]
fn an_uninterrupted_settlement_is_complete_without_replay() {
    let scratch = Scratch::new("settle-none");
    let db = scratch.db();
    let status = helper("settle", &db, &[RUN, UNIT, "none"]);
    assert!(status.success(), "the helper runs the full sequence");
    let store = SqliteStateStore::open(&db).expect("the database reopens");
    assert_eq!(store.settlements(&run_id(), bound()).unwrap().len(), 1);
    assert_eq!(
        store
            .complete_pending_settlements(&run_id(), bound())
            .expect("replay works"),
        0,
        "nothing was half-applied, so replay applies nothing"
    );
}

/// A damaged tail is a typed `Damaged` error on replay — recovery halts
/// rather than silently skipping the bad row (spec §18.3, §6.2).
#[test]
fn a_damaged_tail_is_a_typed_damaged_error() {
    let scratch = Scratch::new("damaged-tail");
    let db = scratch.db();
    let status = helper("corrupt-tail", &db, &[]);
    assert!(status.success(), "the damage script completes");
    // The store still opens; it is the REPLAY that must refuse the damage.
    SqliteStateStore::open(&db).expect("the database reopens");
    let replay = log(&db).read_from(EventLogOffset::new(0), bound());
    match replay {
        Err(EventLogError::Damaged { detail }) => {
            assert!(
                detail.contains("offset 2"),
                "the refusal names where: {detail}"
            );
        }
        other => panic!("a damaged tail must be Damaged, got {other:?}"),
    }
}

/// The PACKAGED §16.3 primitive (`commit_settlement`), killed after the
/// call returned: the correction round's reopening finding — the packaged
/// primitive previously had zero callers, so the sequence was proven only
/// through the helper's two-step re-implementation. Here the child calls
/// the primitive itself and aborts AFTER it: exactly one settlement event
/// and one settlement row must be durable, and replay must add nothing.
#[test]
fn the_packaged_commit_primitive_settles_completely_when_killed_after() {
    let scratch = Scratch::new("commit-after");
    let db = scratch.db();
    let status = helper("settle-via-commit", &db, &[RUN, UNIT, "after"]);
    assert!(
        !status.success(),
        "the helper must die after the commit call, not exit cleanly"
    );
    let store = SqliteStateStore::open(&db).expect("the database reopens");
    assert_eq!(
        log(&db).last_offset(bound()).unwrap(),
        Some(EventLogOffset::new(1)),
        "exactly one settlement event is durable"
    );
    let rows = store.settlements(&run_id(), bound()).expect("reads");
    assert_eq!(rows.len(), 1, "exactly one settlement row");
    assert_eq!(rows[0].unit_id().as_str(), UNIT);
    assert_eq!(rows[0].event_log_offset(), EventLogOffset::new(1));
    assert_eq!(
        store
            .complete_pending_settlements(&run_id(), bound())
            .expect("replay works"),
        0,
        "nothing was half-applied, so replay adds nothing"
    );
}

/// The packaged primitive killed BEFORE the call: nothing durable happened
/// at all — no settlement event, no settlement row, and replay has nothing
/// to complete. The unit is retryable.
#[test]
fn a_crash_before_the_packaged_commit_leaves_the_unit_retryable() {
    let scratch = Scratch::new("commit-before");
    let db = scratch.db();
    let status = helper("settle-via-commit", &db, &[RUN, UNIT, "before"]);
    assert!(
        !status.success(),
        "the helper must die before the commit call"
    );
    let store = SqliteStateStore::open(&db).expect("the database reopens");
    assert!(
        store.settlements(&run_id(), bound()).unwrap().is_empty(),
        "no settlement row exists — the unit is retryable"
    );
    assert_eq!(
        log(&db).last_offset(bound()).unwrap(),
        None,
        "no settlement event was appended"
    );
    assert_eq!(
        store
            .complete_pending_settlements(&run_id(), bound())
            .expect("replay works"),
        0,
        "nothing durable means nothing to replay"
    );
}
