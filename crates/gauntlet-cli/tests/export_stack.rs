//! Export-stack composition test (Task 6 correction round): the application
//! layer's canonical export ([`gauntlet_app::state::export_run`]) run over
//! the REAL embedded-SQL state adapter's output. The composition root is the
//! one crate that may name adapters (spec §4.1) — through the
//! capability-named export, since a vendor-flavored type name cannot be
//! spelled here (the boundary denylist masks the crate name only) — so this
//! is where the seam lives: the adapter's emitted record shapes and the app
//! layer's canonical field lists are two hand-maintained lists, and drift
//! between them surfaces here as a typed error or a byte diff — never only
//! at runtime in the field. Seeded history: a recorded run, two events, two
//! checkpoints written OUT of iteration order, a settlement through the
//! packaged `commit_settlement` primitive, and one generic record.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use gauntlet_adapter_state_sqlite::{EmbeddedStateStore, RecordKind, SettlementCommit};
use gauntlet_app::state::export_run;
use gauntlet_domain::{
    Budget, DiagnosticAllowance, Digest, EpochSeconds, EventLogOffset, EvidenceRef, IterationNo,
    Limits, MicroUsd, ReferenceId, ReferenceVersion, Run, RunDefinition, RunId, Spent, UnitId,
    UnitStatus, VerificationRef,
};
use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::json::{self, JsonValue};
use gauntlet_ports::state_store::{Checkpoint, RunEvent, RunEventKind, StateStore};

const RUN: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";

static SCRATCH_SEQ: AtomicU32 = AtomicU32::new(0);

struct Scratch {
    path: PathBuf,
}

impl Scratch {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "gauntlet-export-stack-{}-{}",
            std::process::id(),
            SCRATCH_SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::remove_dir_all(&path).ok();
        std::fs::create_dir_all(&path).expect("export-stack scratch parent is creatable");
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

// Fixed points far from any clock (the export must never read one). Consts,
// because this crate is a core crate whose literals scanner refuses a bare
// clock number on any line that also names the bound.
const RUN_CREATED_AT: u64 = 1_700_000_000;
const ADMITTED_AT: u64 = 1_700_000_100;
const SETTLED_AT: u64 = 1_700_000_101;
const OBSERVED_AT: u64 = 1_700_000_103;
const BOUND_AT: u64 = 1_700_000_060;

fn bound() -> Deadline {
    Deadline::at_epoch(EpochSeconds::new(BOUND_AT))
}

fn digest() -> Digest {
    Digest::new("sha256", "c4d3e2f1a0b1c2d3").expect("valid digest")
}

fn run() -> Run {
    Run::new(RunDefinition {
        id: run_id(),
        created_at: EpochSeconds::new(RUN_CREATED_AT),
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

fn zero_spent() -> Spent {
    *Budget::new(
        Limits::new(1, 1, 1, 1, 1, MicroUsd::new(1)).expect("minimal legal ceilings"),
        1,
        DiagnosticAllowance::new(1, MicroUsd::new(1)),
    )
    .expect("minimal legal budget")
    .spent()
}

fn checkpoint(iteration: u32, offset: u64, status: UnitStatus) -> Checkpoint {
    Checkpoint {
        run_id: run_id(),
        reference_digest: digest(),
        iteration_no: IterationNo::new(iteration).expect("1-based"),
        unit_states: vec![(UnitId::new("AK-900").expect("valid unit id"), status)],
        budget_spent: zero_spent(),
        in_flight: Vec::new(),
        event_log_offset: EventLogOffset::new(offset),
    }
}

fn string_of(line: &str, field: &str) -> String {
    match json::parse(line)
        .expect("every export line is JSON")
        .get(field)
    {
        Some(JsonValue::Str(value)) => value.clone(),
        other => panic!("field {field:?} of an export line is not a string: {other:?}"),
    }
}

fn uint_of(line: &str, field: &str) -> u64 {
    match json::parse(line)
        .expect("every export line is JSON")
        .get(field)
    {
        Some(JsonValue::Number(gauntlet_ports::json::JsonNumber::Uint(value))) => *value,
        other => panic!("field {field:?} of an export line is not a uint: {other:?}"),
    }
}

/// The stack: real adapter export → app-layer canonical re-export. Two
/// exports of the same history are byte-identical, the output is the
/// canonical total order (not the write order — the checkpoints were
/// written 2 then 1 and must export 1 then 2), and the canonical bytes are
/// exactly the adapter's own for this history, pinning the two shape lists
/// against each other.
#[test]
fn the_real_adapter_export_stack_is_deterministic_and_canonical() {
    let scratch = Scratch::new();
    let db: PathBuf = scratch.db();
    let store = EmbeddedStateStore::open(Path::new(&db)).expect("the store opens");

    // A representative history, in the order a run would write it.
    store.record_run(&run(), bound()).expect("the run records");
    store
        .append_event(
            RunEvent {
                run_id: run_id(),
                kind: RunEventKind::InvocationAdmitted {
                    invocation_id: gauntlet_domain::InvocationId::new("01ARZ3NDEKTSV4RRFFQ69G5FAW")
                        .expect("valid invocation id"),
                },
                at: EpochSeconds::new(ADMITTED_AT),
            },
            bound(),
        )
        .expect("the first event appends");
    let settled_at = store
        .commit_settlement(
            SettlementCommit {
                run_id: run_id(),
                unit_id: UnitId::new("AK-900").expect("valid unit id"),
                evidence: EvidenceRef::new("evd-000900").expect("valid ref"),
                verification: VerificationRef::new("vrf-000900").expect("valid ref"),
                reference_digest: digest(),
                at: EpochSeconds::new(SETTLED_AT),
            },
            bound(),
        )
        .expect("the packaged settlement commit runs");
    assert_eq!(settled_at.get(), 2, "the settlement event is the second");
    // Two checkpoints, deliberately written OUT of iteration order: the
    // export must order by iteration, not by write.
    store
        .checkpoint(checkpoint(2, 2, UnitStatus::InProgress), bound())
        .expect("the iteration-2 checkpoint writes first");
    store
        .checkpoint(checkpoint(1, 1, UnitStatus::Open), bound())
        .expect("the iteration-1 checkpoint writes second");
    store
        .append_record(
            &run_id(),
            RecordKind::Observation,
            r#"{"facet":"filesystem"}"#,
            EpochSeconds::new(OBSERVED_AT),
            bound(),
        )
        .expect("the generic record appends");

    // The stack, twice: byte-identical whatever the store does between calls.
    let first = export_run(&store, &run_id(), bound()).expect("the stack exports");
    let second = export_run(&store, &run_id(), bound()).expect("the stack exports again");
    assert_eq!(
        first, second,
        "two exports of the same history must be byte-identical through the whole stack"
    );

    let lines: Vec<&str> = first.split('\n').collect();
    assert_eq!(
        lines.len(),
        7,
        "run + 2 events + 2 checkpoints + settlement + record"
    );
    let tags: Vec<String> = lines.iter().map(|line| string_of(line, "record")).collect();
    assert_eq!(
        tags,
        [
            "run",
            "event",
            "event",
            "checkpoint",
            "checkpoint",
            "settlement",
            "observation"
        ],
        "the canonical total order: run, events by offset, checkpoints by \
         iteration, settlement, then the generic records"
    );
    assert_eq!(string_of(lines[0], "run_id"), RUN, "the run record leads");
    assert_eq!(uint_of(lines[1], "offset"), 1, "events in offset order");
    assert_eq!(uint_of(lines[2], "offset"), 2, "events in offset order");
    assert_eq!(
        (
            uint_of(lines[3], "iteration_no"),
            uint_of(lines[4], "iteration_no")
        ),
        (1, 2),
        "checkpoints export by iteration — 1 before 2 — though 2 was written first"
    );
    assert_eq!(
        uint_of(lines[5], "event_log_offset"),
        2,
        "the settlement names its own durable event"
    );

    // The seam pin: for this history the canonical bytes are exactly the
    // adapter's own export — the two hand-maintained shape lists agree.
    let adapter_export = store
        .export_jsonl(&run_id(), bound())
        .expect("the adapter exports");
    assert_eq!(
        first, adapter_export,
        "the app layer's canonical re-export and the adapter's export must agree byte for byte"
    );
}
