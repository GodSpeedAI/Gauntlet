//! The settlement-event append gate (Task 13 correction round 3 —
//! confirmation round 1, F-1): `RunEventKind::SettlementCommitted` is
//! EARNED, never appended. The kind reaches the durable log only through
//! `StateStore::commit_settlement`, which consumes the authority-minted
//! `AdmittedSettlement` (`SettlementCommitAuthority::admit` validates the
//! §16.1 conditions against durable state before minting). Both PUBLIC
//! append surfaces refuse the kind for every caller — the generic
//! `StateStore::append_event` and the `EventLog` port's `append` over the
//! same database file.
//!
//! Why this gate exists (the falsifier's executed bypass, mirrored here as
//! the fail-first pair): before the correction, an in-process module
//! holding a store handle could append a forged settlement event directly;
//! the §18.3 replay (`complete_pending_settlements`, driven by honest
//! `resume_run`) then minted the durable settlement row from the forged
//! event alone — zero verification records, no admission, no authority
//! check, no digest consult — poisoning the resume projection and
//! permanently refusing honest settlement of the unit
//! (`UnitAlreadySettled`). The same attacker shape was already gated twice
//! on the sibling surfaces (COR-1's `record_verification` mint,
//! COR2-4's `append_record` kind gate); this file pins the third.
//!
//! Every test here refuses the forged append AND asserts the durable
//! substrate stayed clean: no event row at a new offset, no settlement
//! row, zero verification records — asserted on the store's own export
//! and reads.

use std::path::PathBuf;

use gauntlet_adapter_state_sqlite::{SqliteEventLog, SqliteStateStore};
use gauntlet_domain::{
    Digest, EpochSeconds, EventLogOffset, EvidenceRef, RunId, UnitId, VerificationRef,
};
use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::event_log::{EventLog, EventLogError};
use gauntlet_ports::state_store::{RunEvent, RunEventKind, StateStore};

const RUN: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
/// The falsifier's invented unit: nothing in any ruler names it.
const FORGED_UNIT: &str = "AK-990";
const FORGED_EVIDENCE: &str = "evd-forged-evidence";
const FORGED_VERIFICATION: &str = "vrf-forged-0001";

static SCRATCH_SEQ: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

struct Scratch {
    path: PathBuf,
}

impl Scratch {
    fn new(case: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "gauntlet-settlement-gate-{case}-{}-{}",
            std::process::id(),
            SCRATCH_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::remove_dir_all(&path).ok();
        std::fs::create_dir_all(&path).expect("the scratch parent is creatable");
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

/// The forged event: invented facts, publicly constructible — exactly the
/// falsifier's payload. If this append ever succeeds again, F-1 is back.
fn forged_event() -> RunEvent {
    RunEvent {
        run_id: run_id(),
        kind: RunEventKind::SettlementCommitted {
            unit_id: UnitId::new(FORGED_UNIT).expect("valid unit id"),
            evidence: EvidenceRef::new(FORGED_EVIDENCE).expect("valid ref"),
            verification: VerificationRef::new(FORGED_VERIFICATION).expect("valid ref"),
            reference_digest: Digest::new("sha256", "c4d3e2f1a0b1c2d3").expect("valid digest"),
        },
        at: EpochSeconds::new(1_700_000_050),
    }
}

/// Asserts the durable substrate is EMPTY of every settlement fact the
/// forged append would have minted: no settlement event in the log (the
/// export names none), no settlement row, zero verification records.
fn assert_zero_durable_settlement_writes(store: &SqliteStateStore, stage: &str) {
    let export = store
        .export_jsonl(&run_id(), bound())
        .expect("the export reads");
    assert!(
        !export.contains("settlement_committed"),
        "{stage}: no settlement event may exist in the durable log: {export}"
    );
    assert!(
        !export.contains(FORGED_UNIT) && !export.contains(FORGED_VERIFICATION),
        "{stage}: no trace of the forged facts may exist durably: {export}"
    );
    assert!(
        store
            .settlements(&run_id(), bound())
            .expect("settlement rows read")
            .is_empty(),
        "{stage}: no settlement row may mint from a refused append"
    );
    assert_eq!(
        store
            .verification_records(&run_id(), bound())
            .expect("verification records read")
            .len(),
        0,
        "{stage}: the forged settlement mints nothing to validate against — zero PASS records"
    );
}

/// Leg A — the falsifier's `[A]` sequence: the forged event through the
/// generic `StateStore::append_event` trait method, then the §18.3 replay
/// (the honest `resume_run` driver of `complete_pending_settlements`).
/// BEFORE the correction this appended at an offset and minted a durable
/// settlement row with zero verification records; the refusal must be the
/// typed error naming the earned path, with zero durable writes.
#[test]
fn the_state_store_append_surface_refuses_the_settlement_kind() {
    let scratch = Scratch::new("leg-a");
    let db = scratch.db();
    let store = SqliteStateStore::open(&db).expect("the store opens");
    match store.append_event(forged_event(), bound()) {
        // RED-stage arm: pre-correction this arm is unreachable (the append
        // succeeds); post-correction it is tightened to the typed
        // `SettlementEventNotEarned` variant match.
        Err(error) => {
            let detail = error.to_string();
            assert!(
                detail.contains("commit_settlement"),
                "the refusal names the earned path: {detail}"
            );
        }
        Ok(offset) => panic!(
            "the forged settlement event must be refused by the generic append surface, but it \
             appended durably at offset {} — the 18.3 replay will mint a settlement row with \
             zero verification records (F-1)",
            offset.get()
        ),
    }
    // The replay that honest resume would drive mints nothing from the
    // refused append.
    assert_eq!(
        store
            .complete_pending_settlements(&run_id(), EventLogOffset::new(0), bound())
            .expect("the replay runs"),
        0,
        "no settlement row may mint from the refused append"
    );
    assert_zero_durable_settlement_writes(&store, "after the refused append");
    // And the replay on a REOPENED store (the crash-recovery reader's view)
    // still mints nothing: the refusal left nothing behind.
    drop(store);
    let reopened = SqliteStateStore::open(&db).expect("the store reopens");
    assert_zero_durable_settlement_writes(&reopened, "after reopen");
}

/// Leg B — the falsifier's `[B]` sequence: the same forged event through
/// the SEPARATE `EventLog` port (`SqliteEventLog`) over the same database
/// file. The log is the durability spine both surfaces share; refusing the
/// kind on the store's append alone would leave the log's own append as
/// the bypass. The refusal is the EventLog port's typed error naming the
/// earned path, and the durable substrate stays clean.
#[test]
fn the_event_log_append_surface_refuses_the_settlement_kind() {
    let scratch = Scratch::new("leg-b");
    let db = scratch.db();
    let log = SqliteEventLog::open(&db).expect("the log opens");
    match log.append(forged_event(), bound()) {
        Err(EventLogError::AppendFailed { detail }) => panic!(
            "the refusal must be the settlement-kind gate's typed variant, got the generic \
             AppendFailed: {detail}"
        ),
        Err(error) => {
            let detail = error.to_string();
            assert!(
                detail.contains("commit_settlement"),
                "the refusal names the earned path: {detail}"
            );
        }
        Ok(offset) => panic!(
            "the forged settlement event must be refused by the EventLog append surface, but it \
             appended durably at offset {} — the 18.3 replay will mint a settlement row with \
             zero verification records (F-1)",
            offset.get()
        ),
    }
    // The log itself gained nothing.
    assert_eq!(
        log.last_offset(bound()).expect("the last offset reads"),
        None,
        "no event row may exist at any new offset"
    );
    assert!(
        log.read_from(EventLogOffset::new(0), bound())
            .expect("the log replays")
            .is_empty(),
        "the log must be empty — the refused append wrote no row"
    );
    drop(log);
    // And the store over the same file replays nothing into existence.
    let store = SqliteStateStore::open(&db).expect("the store opens");
    assert_eq!(
        store
            .complete_pending_settlements(&run_id(), EventLogOffset::new(0), bound())
            .expect("the replay runs"),
        0,
        "no settlement row may mint from the refused append"
    );
    assert_zero_durable_settlement_writes(&store, "after the refused log append");
}
