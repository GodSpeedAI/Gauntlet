//! Crash-injection instrument (plan Task 6 step 5): performs a scripted
//! sequence against a real database file and can `std::process::abort()` at
//! a named step. Integration tests spawn this as a child process, let the
//! abort kill it for real, and then REOPEN the file — nothing here simulates
//! a crash from inside the process that owns the connection.
//!
//! ```text
//! crash_helper append-events <db> <run-id> <count> [abort-after]
//!   Appends <count> invocation events; with [abort-after], aborts right
//!   after that many appends — a crash mid-sequence.
//!
//! crash_helper settle <db> <run-id> <unit-id> <abort-point>
//!   The §16.3 sequence for one unit, as the two separate port steps:
//!     1. append the SettlementCommitted event (durable, fsync);
//!     2. write the settlement row (idempotent by the §16.3 key).
//!   <abort-point> is `before-append` (nothing durable happened — the unit
//!   must stay retryable), `after-append` (half-settled: the event is
//!   durable, the row is not), `after-apply`, or `none`.
//!
//! crash_helper settle-via-commit <db> <run-id> <unit-id> <abort-point>
//!   The PACKAGED §16.3 primitive — SqliteStateStore::commit_settlement, one
//!   call, both durable steps — rather than the two steps issued separately.
//!   The abort points sit at the call's edges (a single call cannot be
//!   interrupted in the middle without in-process injection): `before`
//!   aborts before the call (nothing durable — the unit must stay
//!   retryable), `after` aborts once the call returned (fully settled), and
//!   `none` exits cleanly.
//!
//! crash_helper corrupt-tail <db>
//!   Appends one valid event, then a raw row whose payload cannot decode —
//!   damage injection for the Damaged-read tests.
//! ```

use std::path::Path;
use std::sync::Arc;

use gauntlet_adapter_state_sqlite::SqliteStateStore;
use gauntlet_domain::{
    Budget, DiagnosticAllowance, Digest, EpochSeconds, EvidenceKind, EvidenceLocator, EvidenceRef,
    InstrumentName, IterationNo, Limits, MicroUsd, Producer, ReferenceId, ReferenceVersion, Run,
    RunDefinition, RunId, UnitId, VerificationRef,
};
use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::producer_authority::SettlementCommitAuthority;
use gauntlet_ports::state_store::{RunEvent, RunEventKind, SettlementCommit, StateStore};
use gauntlet_ports::verification_record::{
    RequirementDoc, VerificationContent, VerificationRecordDoc, VerifyingInstrument,
};

/// A far-future bound for the helper's own calls; the helper is the
/// instrument, not the system under observation.
fn bound() -> Deadline {
    Deadline::at_epoch(EpochSeconds::new(4_000_000_000))
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let script = args.get(1).map(String::as_str).unwrap_or_default();
    let outcome = match script {
        "append-events" => append_events(&args[2..]),
        "settle" => settle(&args[2..]),
        "settle-via-commit" => settle_via_commit(&args[2..]),
        "corrupt-tail" => corrupt_tail(&args[2..]),
        other => {
            eprintln!("crash_helper: unknown script {other:?}; see the module docs");
            std::process::exit(2);
        }
    };
    if let Err(detail) = outcome {
        eprintln!("crash_helper: {detail}");
        std::process::exit(2);
    }
}

fn append_events(args: &[String]) -> Result<(), String> {
    let [db, run_id, count, abort_after @ ..] = args else {
        return Err("usage: append-events <db> <run-id> <count> [abort-after]".to_owned());
    };
    let run = RunId::new(run_id).map_err(|err| format!("run id: {err}"))?;
    let count: u32 = count
        .parse()
        .map_err(|_| format!("count {count:?} is not a number"))?;
    let abort_after: Option<u32> = match abort_after {
        [] => None,
        [value] => Some(
            value
                .parse()
                .map_err(|_| format!("abort-after {value:?} is not a number"))?,
        ),
        _ => return Err("append-events takes at most one abort-after value".to_owned()),
    };
    let store = SqliteStateStore::open(Path::new(db)).map_err(|err| err.to_string())?;
    for sequence in 0..count {
        let event = RunEvent {
            run_id: run.clone(),
            kind: RunEventKind::InvocationAdmitted {
                invocation_id: gauntlet_domain::InvocationId::new("01ARZ3NDEKTSV4RRFFQ69G5FAW")
                    .map_err(|err| err.to_string())?,
            },
            at: EpochSeconds::new(1_700_000_000 + u64::from(sequence)),
        };
        store
            .append_event(event, bound())
            .map_err(|err| err.to_string())?;
        println!("appended {sequence}");
        if abort_after == Some(sequence + 1) {
            println!("aborting after append {sequence}");
            std::process::abort();
        }
    }
    Ok(())
}

fn settle(args: &[String]) -> Result<(), String> {
    let [db, run_id, unit_id, abort_point] = args else {
        return Err("usage: settle <db> <run-id> <unit-id> <abort-point>".to_owned());
    };
    let run = RunId::new(run_id).map_err(|err| format!("run id: {err}"))?;
    let unit = UnitId::new(unit_id).map_err(|err| format!("unit id: {err}"))?;
    let store = SqliteStateStore::open(Path::new(db)).map_err(|err| err.to_string())?;
    let commit = SettlementCommit {
        run_id: run.clone(),
        unit_id: unit.clone(),
        evidence: EvidenceRef::new("evd-crash-0001").map_err(|err| err.to_string())?,
        verification: VerificationRef::new("vrf-crash-0001").map_err(|err| err.to_string())?,
        reference_digest: Digest::new("sha256", "c4d3e2f1a0b1c2d3")
            .map_err(|err| err.to_string())?,
        at: EpochSeconds::new(1_700_000_050),
    };
    if abort_point == "before-append" {
        println!("aborting before the event append — nothing durable");
        std::process::abort();
    }
    // §16.3 step 1: the durable event.
    let offset = store
        .append_event(
            RunEvent {
                run_id: run.clone(),
                kind: RunEventKind::SettlementCommitted {
                    unit_id: commit.unit_id.clone(),
                    evidence: commit.evidence.clone(),
                    verification: commit.verification.clone(),
                    reference_digest: commit.reference_digest.clone(),
                },
                at: commit.at,
            },
            bound(),
        )
        .map_err(|err| err.to_string())?;
    println!(
        "settlement event appended for {unit} at offset {}",
        offset.get()
    );
    if abort_point == "after-append" {
        println!("aborting after the event append — half-settled");
        std::process::abort();
    }
    // §16.3 step 2: the idempotent keyed row, for the event just appended.
    let applied = store
        .apply_settlement_row(commit, offset, bound())
        .map_err(|err| err.to_string())?;
    println!("settlement row applied for {unit} (added {applied})");
    if abort_point == "after-apply" {
        println!("aborting after the settlement row — fully settled, killed anyway");
        std::process::abort();
    }
    if abort_point != "none" {
        return Err(format!("unknown abort point {abort_point:?}"));
    }
    Ok(())
}

/// The PACKAGED §16.3 primitive, `commit_settlement` — one call, both
/// durable steps (correction round: the packaged primitive previously had
/// zero callers; the crash tests exercised only the two steps issued
/// separately through `settle`). Abort points sit at the call's edges.
///
/// Since plan Task 13 the packaged primitive consumes an EARNED admission
/// (the ports `SettlementCommitAuthority`): the helper first makes the run
/// durable and records the PASS verification document the admission
/// validates against — the same governed path production composes — then
/// mints and commits.
fn settle_via_commit(args: &[String]) -> Result<(), String> {
    let [db, run_id, unit_id, abort_point] = args else {
        return Err("usage: settle-via-commit <db> <run-id> <unit-id> <abort-point>".to_owned());
    };
    let run = RunId::new(run_id).map_err(|err| format!("run id: {err}"))?;
    let unit = UnitId::new(unit_id).map_err(|err| format!("unit id: {err}"))?;
    let store = Arc::new(SqliteStateStore::open(Path::new(db)).map_err(|err| err.to_string())?);
    // The durable substrate the admission validates against (plan Task 13).
    store
        .record_run(&crash_fixture_run(&run), bound())
        .map_err(|err| err.to_string())?;
    store
        .record_verification(&pass_record(&run, &unit), bound())
        .map_err(|err| err.to_string())?;
    let commit = SettlementCommit {
        run_id: run.clone(),
        unit_id: unit.clone(),
        evidence: EvidenceRef::new("evd-crash-0001").map_err(|err| err.to_string())?,
        verification: VerificationRef::new("vrf-crash-0001").map_err(|err| err.to_string())?,
        reference_digest: Digest::new("sha256", "c4d3e2f1a0b1c2d3")
            .map_err(|err| err.to_string())?,
        at: EpochSeconds::new(1_700_000_050),
    };
    if abort_point == "before" {
        println!("aborting before commit_settlement — no settlement is durable");
        std::process::abort();
    }
    if abort_point != "after" && abort_point != "none" {
        return Err(format!("unknown abort point {abort_point:?}"));
    }
    let admitted = SettlementCommitAuthority::new(store.clone())
        .admit(commit, bound())
        .map_err(|err| err.to_string())?;
    let offset = store
        .commit_settlement(&admitted, bound())
        .map_err(|err| err.to_string())?;
    println!("commit_settlement returned offset {}", offset.get());
    if abort_point == "after" {
        println!("aborting after commit_settlement — fully settled, killed anyway");
        std::process::abort();
    }
    Ok(())
}

fn corrupt_tail(args: &[String]) -> Result<(), String> {
    let [db] = args else {
        return Err("usage: corrupt-tail <db>".to_owned());
    };
    let store = SqliteStateStore::open(Path::new(db)).map_err(|err| err.to_string())?;
    let run = RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").map_err(|err| err.to_string())?;
    store
        .append_event(
            RunEvent {
                run_id: run,
                kind: RunEventKind::InvocationAdmitted {
                    invocation_id: gauntlet_domain::InvocationId::new("01ARZ3NDEKTSV4RRFFQ69G5FAW")
                        .map_err(|err| err.to_string())?,
                },
                at: EpochSeconds::new(1_700_000_000),
            },
            bound(),
        )
        .map_err(|err| err.to_string())?;
    // Deliberate damage: a row whose payload cannot decode. This is the one
    // place in the crate that writes around the encoder, and it exists only
    // in this test instrument.
    let conn = rusqlite::Connection::open(db).map_err(|err| err.to_string())?;
    conn.execute(
        "INSERT INTO event_log (run_id, kind, payload, at) VALUES (?1, ?2, ?3, ?4)",
        (
            "01ARZ3NDEKTSV4RRFFQ69G5FAV",
            "settlement_committed",
            "{\"kind\":\"settlement_committed\",\"unit_id\":",
            1_700_000_001_i64,
        ),
    )
    .map_err(|err| err.to_string())?;
    println!("damaged tail written");
    Ok(())
}

/// The durable run identity the Task-13 settlement admission opens over:
/// the frozen digest the settlement commits name (`sha256:c4d3e2f1a0b1c2d3`).
fn crash_fixture_run(run: &RunId) -> Run {
    Run::new(RunDefinition {
        id: run.clone(),
        created_at: EpochSeconds::new(1_700_000_000),
        reference_version: ReferenceVersion::new(
            ReferenceId::new("ref-crash-helper").expect("valid reference id"),
            1,
            Digest::new("sha256", "c4d3e2f1a0b1c2d3").expect("valid digest"),
        ),
        intent: "crash-injection settlement".to_owned(),
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

/// The PASS verification document the settlement admission validates
/// against: this run, this unit, the evidence the commit names.
fn pass_record(run: &RunId, unit: &UnitId) -> VerificationRecordDoc {
    VerificationRecordDoc::new(VerificationContent {
        run_id: run.clone(),
        iteration_no: IterationNo::new(1).expect("1-based"),
        unit_id: unit.clone(),
        claim: "the crash-helper unit's claim".to_owned(),
        requirement: RequirementDoc::new(
            EvidenceKind::TestReport,
            EvidenceLocator::new("crash-helper-report.json").expect("valid locator"),
            Producer::Instrument,
        ),
        instrument: VerifyingInstrument::new(
            InstrumentName::new("crash-helper-verifier")
                .expect("valid instrument name")
                .as_str()
                .to_owned(),
            "1".to_owned(),
            true,
        ),
        evidence: EvidenceRef::new("evd-crash-0001").expect("valid evidence ref"),
        produced_at: EpochSeconds::new(1_700_000_040),
        outcome: gauntlet_domain::Outcome::Pass,
        class: None,
        detail: "the crash helper's fixture PASS record".to_owned(),
    })
    .expect("fixture content is well formed")
}
