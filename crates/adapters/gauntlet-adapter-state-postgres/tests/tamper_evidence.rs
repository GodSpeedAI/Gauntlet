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

use std::sync::atomic::{AtomicU64, Ordering};

use gauntlet_adapter_state_postgres::{
    LOCAL_DEFAULT_URL, PostgresConfig, PostgresEventLog, PostgresStateStore,
};
use gauntlet_domain::{
    Budget, DiagnosticAllowance, Digest, EpochSeconds, EventLogOffset, IterationNo, Limits,
    MicroUsd, ReferenceId, ReferenceVersion, Run, RunDefinition, RunId, Spent, UnitId, UnitStatus,
    VerificationRef,
};
use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::event_log::{EventLog, EventLogError};
use gauntlet_ports::producer_authority::SettlementCommitAuthority;
use gauntlet_ports::state_store::{
    Checkpoint, RunEvent, RunEventKind, SettlementCommit, StateStore, StateStoreError,
};
use postgres::{Client, NoTls};

const RUN: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
const UNIT: &str = "AK-900";
const DIGEST_HEX: &str = "c4d3e2f1a0b1c2d3";

static SCRATCH_SEQ: AtomicU64 = AtomicU64::new(0);

/// A fresh PostgreSQL namespace per case, dropped on scope exit.
struct Scratch {
    config: PostgresConfig,
}

impl Scratch {
    fn new(case: &str) -> Self {
        let _ = case;
        let pid = std::process::id();
        let counter = SCRATCH_SEQ.fetch_add(1, Ordering::Relaxed);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .ok()
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let url = std::env::var("GAUNTLET_DATABASE_URL")
            .unwrap_or_else(|_| LOCAL_DEFAULT_URL.to_string());
        let namespace = format!("gauntlet_t_{}_{:x}_{:x}", pid, counter, nanos);
        Self {
            config: PostgresConfig { url, namespace },
        }
    }

    fn config(&self) -> &PostgresConfig {
        &self.config
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        if let Ok(mut client) = Client::connect(&self.config.url, NoTls) {
            let _ = client.batch_execute(&format!(
                "DROP SCHEMA IF EXISTS \"{}\" CASCADE",
                self.config.namespace
            ));
        }
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

/// The PASS verification document the Task-13 settlement admission
/// validates against: this run, the commit's unit, the commit's evidence.
fn pass_record() -> gauntlet_ports::verification_record::VerificationRecordDoc {
    gauntlet_ports::verification_record::VerificationRecordDoc::new(
        gauntlet_ports::verification_record::VerificationContent {
            // Deliberately the committed settlement's own ref (COR-3: the
            // admission requires the durable record's ref to equal the
            // commit's).
            verification: VerificationRef::new("vrf-000900").expect("valid ref"),
            run_id: run_id(),
            iteration_no: IterationNo::new(1).expect("1-based"),
            unit_id: UnitId::new(UNIT).expect("valid unit id"),
            claim: "the tampered unit's claim".to_owned(),
            requirement: gauntlet_ports::verification_record::RequirementDoc::new(
                gauntlet_domain::EvidenceKind::TestReport,
                gauntlet_domain::EvidenceLocator::new("tamper-report.json").expect("locator"),
                gauntlet_domain::Producer::Instrument,
            ),
            instrument: gauntlet_ports::verification_record::VerifyingInstrument::new(
                "tamper-verifier".to_owned(),
                "1".to_owned(),
                true,
            ),
            evidence: gauntlet_domain::EvidenceRef::new("evd-000900").expect("valid ref"),
            produced_at: EpochSeconds::new(1_700_000_100),
            outcome: gauntlet_domain::Outcome::Pass,
            class: None,
            detail: "the fixture PASS record".to_owned(),
        },
    )
    .expect("fixture content is well formed")
}

/// Makes the durable substrate the admission validates against and mints
/// the admission itself (the governed path, plan Task 13).
fn earn_settlement(
    store: &std::sync::Arc<PostgresStateStore>,
    commit: SettlementCommit,
) -> gauntlet_ports::producer_authority::AdmittedSettlement {
    store.record_run(&run(), bound()).expect("the run records");
    gauntlet_ports::VerificationRecordAuthority::new(store.clone())
        .record(&pass_record(), bound())
        .expect("the PASS record lands");
    SettlementCommitAuthority::new(store.clone())
        .admit(commit, bound())
        .expect("the honest settlement earns its admission")
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

/// A raw second connection to the same namespace — the damage instrument.
/// Pins `search_path` to the namespace exactly as the adapter does.
fn raw(config: &PostgresConfig) -> Client {
    let mut client =
        Client::connect(&config.url, NoTls).expect("the raw instrument connection opens");
    client
        .batch_execute(&format!("SET search_path TO \"{}\"", config.namespace))
        .expect("the raw instrument pins its namespace");
    client
}

/// Appends one out-of-band event_log row, allocating its offset the way the
/// adapter does (the gapless counter), so the raw row is a real next offset.
fn raw_event(config: &PostgresConfig, run: &str, kind: &str, payload: &str, at: i64) {
    let mut client = raw(config);
    let offset: i64 = client
        .query_one(
            "UPDATE event_log_counter SET next_offset = next_offset + 1 \
             WHERE id = 1 RETURNING next_offset",
            &[],
        )
        .expect("the counter advances")
        .get(0);
    client
        .execute(
            "INSERT INTO event_log (\"offset\", run_id, kind, payload, at) \
             VALUES ($1, $2, $3, $4, $5)",
            &[&offset, &run, &kind, &payload, &at],
        )
        .expect("the tamper row lands");
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
    let config = scratch.config();
    let store = PostgresStateStore::open(config).expect("the store opens");
    store
        .append_event(admitted_event(1_700_000_000), bound())
        .expect("the valid event appends");
    drop(store);
    raw_event(config, RUN, ADMITTED_KIND, ADMITTED_PAYLOAD, -5);
    let log = PostgresEventLog::open(config).expect("the log reopens");
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
    let config = scratch.config();
    let store = PostgresStateStore::open(config).expect("the store opens");
    store.record_run(&run(), bound()).expect("run records");
    drop(store);
    raw(config)
        .execute(
            "INSERT INTO runs (
                run_id, created_at, reference_id, reference_version, reference_digest,
                intent, config_fingerprint, environment_fingerprint, parent_run, budget
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
            &[
                &"01ARZ3NDEKTSV4RRFFQ69G5FAZ",
                &-1_i64,
                &"ref-task6",
                &3_i64,
                &format!("sha256:{DIGEST_HEX}"),
                &"settle the task",
                &"sha256:a1b2c3d4e5f6a7b8",
                &"sha256:e5f6a7b8c4d3e2f1",
                &Option::<String>::None,
                &"{\"recovery_reserve_pct\":20,\"limits\":{\"rounds\":8,\"parallel\":2,\"depth\":2,\"\
                 \"wall_seconds\":3600,\"tokens\":1000,\"cost_usd\":500},\"diagnostic\":{\"tokens\":\
                 100,\"cost_usd\":50}}",
            ],
        )
        .expect("the tamper row lands");
    let store = PostgresStateStore::open(config).expect("the store reopens");
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
    let config = scratch.config();
    let store = PostgresStateStore::open(config).expect("the store opens");
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
    raw_event(config, RUN, ADMITTED_KIND, ADMITTED_PAYLOAD, -9);
    let store = PostgresStateStore::open(config).expect("the store reopens");
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
    let config = scratch.config();
    let store = PostgresStateStore::open(config).expect("the store opens");
    store.record_run(&run(), bound()).expect("run records");
    drop(store);
    raw(config)
        .execute(
            "INSERT INTO records (run_id, record_type, payload, at) VALUES ($1, $2, $3, $4)",
            &[&RUN, &"observation", &"{\"facet\":\"filesystem\"}", &-7_i64],
        )
        .expect("the tamper row lands");
    let store = PostgresStateStore::open(config).expect("the store reopens");
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

// The three `apply_settlement_row` cross-validation legs (foreign offset /
// past-log offset / own-offset happy path) moved IN-CRATE (state_store.rs
// tests) with the Task-13 correction round: the raw row-apply primitive is
// `#[cfg(test)]` since COR-1 closed the early probe's C1 aggravator, and
// out-of-crate integration tests cannot see cfg(test) items. Assertions
// are verbatim in the new home.

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
    let config = scratch.config();
    let store = PostgresStateStore::open(config).expect("the store opens");
    drop(store);
    let mut injector = raw(config);
    injector
        .batch_execute(
            "CREATE FUNCTION refuse_settlement_rows_fn() RETURNS trigger
             LANGUAGE plpgsql AS $$
             BEGIN
                 RAISE EXCEPTION 'gauntlet injection: settlement row refused';
             END;
             $$;
             CREATE TRIGGER refuse_settlement_rows AFTER INSERT ON settlements
             FOR EACH ROW EXECUTE FUNCTION refuse_settlement_rows_fn();",
        )
        .expect("the failure-injection trigger installs");
    drop(injector);

    let store = std::sync::Arc::new(PostgresStateStore::open(config).expect("the store reopens"));
    let admitted = earn_settlement(&store, commit());
    match store.commit_settlement(&admitted, bound()) {
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
    let log = PostgresEventLog::open(config).expect("the log opens");
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
    raw(config)
        .batch_execute(
            "DROP TRIGGER refuse_settlement_rows ON settlements;
             DROP FUNCTION refuse_settlement_rows_fn();",
        )
        .expect("the injection is removed");
    let store = PostgresStateStore::open(config).expect("the store reopens");
    assert_eq!(
        store
            .complete_pending_settlements(&run_id(), EventLogOffset::new(0), bound())
            .expect("replay works"),
        1,
        "replay completes the half-settled unit exactly as the error said it would"
    );
}

/// A3's decode leg (plan Task 13): a settlement row holding an offset no
/// u64 can represent is `Corrupt` naming the row — never silently coerced
/// to offset 0 (the Task-12 N3 checkpoint discipline, mirrored). This
/// adapter can never write such a row, so any negative read is tamper
/// evidence.
#[test]
fn a_tampered_settlement_offset_is_corrupt_never_a_silent_zero() {
    let scratch = Scratch::new("negative-settlement-offset");
    let config = scratch.config();
    // The settlement substrate lands the EARNED way (COR3: the generic
    // append surfaces refuse the settlement kind); the tamper below then
    // corrupts the row out-of-band — this suite's own damage model.
    let store = std::sync::Arc::new(PostgresStateStore::open(config).expect("the store opens"));
    let admitted = earn_settlement(&store, commit());
    store
        .commit_settlement(&admitted, bound())
        .expect("the honest settlement commits");
    assert_eq!(
        store
            .settlements(&run_id(), bound())
            .expect("the row lands")
            .len(),
        1
    );
    drop(store);
    raw(config)
        .execute("UPDATE settlements SET event_log_offset = -1", &[])
        .expect("the tamper applies");
    let store = PostgresStateStore::open(config).expect("the store reopens");
    match store.settlements(&run_id(), bound()) {
        Err(StateStoreError::Corrupt { detail }) => {
            assert!(
                detail.contains("event-log offset") && detail.contains("-1"),
                "the refusal names the locus and the stored value: {detail}"
            );
        }
        Ok(rows) => panic!(
            "a non-representable stored settlement offset is corrupt, never a silent zero (got \
             {} rows reading offset {:?})",
            rows.len(),
            rows.first().map(|row| row.event_log_offset().get())
        ),
        Err(other) => panic!("the refusal is Corrupt, got {other}"),
    }
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
    let config = scratch.config();
    let store = PostgresStateStore::open(config).expect("the store opens");
    store.record_run(&run(), bound()).expect("run records");
    drop(store);
    raw_event(
        config,
        RUN,
        "settlement_committed",
        "{\"kind\":\"settlement_committed\",\"unit_id\":",
        1_700_000_001,
    );
    let store = PostgresStateStore::open(config).expect("the store reopens");
    match store.export_jsonl(&run_id(), bound()) {
        Err(StateStoreError::Corrupt { detail }) => {
            assert!(
                detail.contains("damaged") && detail.contains("offset 1"),
                "the damage class and locus survive the mapping: {detail}"
            );
        }
        other => panic!("log damage on export must be Corrupt, got {other:?}"),
    }
    match store.complete_pending_settlements(&run_id(), EventLogOffset::new(0), bound()) {
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
    let config = scratch.config();
    let store = PostgresStateStore::open(config).expect("the store opens");
    for sequence in 0..3u64 {
        store
            .append_event(admitted_event(1_700_000_000 + sequence), bound())
            .expect("appends");
    }
    drop(store);
    let log = PostgresEventLog::open(config).expect("the log opens");
    assert_eq!(
        log.read_from(EventLogOffset::new(0), bound())
            .expect("a contiguous log replays")
            .len(),
        3,
        "the pre-tamper log is contiguous and green"
    );
    drop(log);
    raw(config)
        .execute("DELETE FROM event_log WHERE \"offset\" = 2", &[])
        .expect("the middle row is removed out of band");
    let log = PostgresEventLog::open(config).expect("the log reopens");
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
    let config = scratch.config();
    let store = PostgresStateStore::open(config).expect("the store opens");
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

// ---------------------------------------------------------------------------
// COR2-5 — a same-key settlement event with DIVERGENT facts is loud on
// replay, never a silently ignored INSERT (critic round 1, F-6)
// ---------------------------------------------------------------------------

/// The durable row for (run, unit, verification) holds evidence evd-000900;
/// the log is then handed a second, well-formed settlement event for the
/// SAME key whose evidence is evd-000901. Replay must surface a typed
/// integrity refusal naming both sides — before the correction the keyed
/// INSERT OR IGNORE swallowed the divergence and reported Ok(0).
#[test]
fn a_same_key_divergent_event_is_loud_on_replay() {
    let scratch = Scratch::new("cor25-divergent-replay");
    let config = scratch.config();
    let store = std::sync::Arc::new(PostgresStateStore::open(config).expect("the store opens"));
    let admitted = earn_settlement(&store, commit());
    store
        .commit_settlement(&admitted, bound())
        .expect("the honest settlement commits");
    drop(store);
    let divergent_payload = format!(
        "{{\"kind\":\"settlement_committed\",\"unit_id\":\"{UNIT}\",\"evidence\":\"evd-000901\",\
         \"verification\":\"vrf-000900\",\"reference_digest\":\"sha256:{DIGEST_HEX}\"}}"
    );
    raw_event(
        config,
        RUN,
        "settlement_committed",
        divergent_payload.as_str(),
        1_700_000_200,
    );
    let store = PostgresStateStore::open(config).expect("the store reopens");
    match store.complete_pending_settlements(&run_id(), EventLogOffset::new(0), bound()) {
        Err(StateStoreError::Corrupt { detail }) => {
            assert!(
                detail.contains("DIVERGENT")
                    && detail.contains("evd-000900")
                    && detail.contains("evd-000901"),
                "the refusal names both sides of the divergence: {detail}"
            );
        }
        other => panic!(
            "a same-key divergent settlement event must be a typed integrity refusal on \
             replay, got {other:?}"
        ),
    }
}

/// The same-facts duplicate stays the idempotent no-op it always was
/// (the mirror leg of the divergence test, on its own fixture - a divergent
/// event anywhere in the log correctly fails every replay over it).
#[test]
fn a_same_facts_duplicate_event_stays_the_idempotent_no_op() {
    let scratch = Scratch::new("cor25-same-facts");
    let config = scratch.config();
    let store = std::sync::Arc::new(PostgresStateStore::open(config).expect("the store opens"));
    let admitted = earn_settlement(&store, commit());
    store
        .commit_settlement(&admitted, bound())
        .expect("the honest settlement commits");
    drop(store);
    let same_facts_payload = format!(
        "{{\"kind\":\"settlement_committed\",\"unit_id\":\"{UNIT}\",\"evidence\":\
         \"evd-000900\",\"verification\":\"vrf-000900\",\"reference_digest\":\"sha256:{DIGEST_HEX}\"}}"
    );
    raw_event(
        config,
        RUN,
        "settlement_committed",
        same_facts_payload.as_str(),
        1_700_000_300,
    );
    let store = PostgresStateStore::open(config).expect("the store reopens");
    match store.complete_pending_settlements(&run_id(), EventLogOffset::new(0), bound()) {
        Ok(0) => {}
        other => panic!("a same-facts duplicate stays the idempotent no-op, got {other:?}"),
    }
}
