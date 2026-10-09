//! TEST INSTRUMENTATION (slice E1d-1; hidden from the documented API surface).
//!
//! These functions are the out-of-band instruments the end-to-end tests use to
//! damage, or read back, the durable state of a composed run. They are the
//! PostgreSQL port of the SQLite crate's `test_support` module: the function
//! names and signatures are identical, so the composition-root tests change
//! only their crate path.
//!
//! Each function takes `db_path`, the state-instance PATH (the same locator the
//! composition root uses), maps it to its namespace via
//! [`crate::namespace_for_path`], and runs its SQL on a raw client whose
//! `search_path` is that namespace. Nothing in the runtime calls into this
//! module; it is never a production surface.

use std::path::Path;
use std::time::{Duration, Instant};

use gauntlet_ports::json::{self, JsonValue};

use crate::db::{PostgresConfig, application_name, connect};

/// One durable `settlement_committed` event's reader shape:
/// `(event-log offset, unit id, verification ref, evidence ref,
/// stored timestamp)` — see [`settlement_committed_events`].
pub type SettlementEventFacts = (u64, String, String, String, i64);
/// One durable settlement row's reader shape: `(unit id, verification
/// ref, evidence ref, event-log offset, settled_at)` — see
/// [`settlement_rows`].
pub type SettlementRowFacts = (String, String, String, i64, i64);
/// One durable evidence-production row's reader shape, iteration
/// included: `(evidence ref, locator, iteration_no, produced_at,
/// event-log offset)` — see [`evidence_production_rows_with_iteration`].
pub type EvidenceProductionIterationFacts = (String, String, i64, i64, i64);

/// Opens a raw client on the namespace of the state instance at `db_path`.
/// The schema is created if absent (the same as the store's own connect), so
/// the instrument never fails merely because the store was not yet opened.
fn client_at(db_path: &Path) -> Result<postgres::Client, String> {
    connect(&PostgresConfig::for_state_path(db_path))
}

fn text(error: impl std::fmt::Display) -> String {
    error.to_string()
}

/// QUIESCENCE BARRIER: waits until no connection of ANOTHER process carries
/// the namespace of the state instance at `state_locator`
/// (`gauntlet:<ns>:<pid>` in `pg_stat_activity`, `pid` != this process),
/// polling every 20ms for at most `deadline`. This process's own connections
/// (its open stores) never block the barrier.
///
/// A killed process's last COMMIT may still be completing on the server
/// after the client is dead; its backend exits only once that commit is
/// done. Reading durable state only after this returns `Ok` is therefore
/// race-free. Never proceeds silently: a timeout is an `Err` naming the
/// stuck count.
pub fn quiesce_writers(state_locator: &Path, deadline: Duration) -> Result<(), String> {
    quiesce_namespace(&PostgresConfig::for_state_path(state_locator), deadline)
}

/// [`quiesce_writers`] addressed by an explicit namespace configuration, for
/// instances whose namespace is not derived from a state path.
pub fn quiesce_namespace(config: &PostgresConfig, deadline: Duration) -> Result<(), String> {
    // A plain connection: it must not carry a tag it would count, and it
    // must not create the schema as a side effect.
    let mut observer = postgres::Client::connect(&config.url, postgres::NoTls)
        .map_err(|e| format!("quiesce: failed to connect to database: {}", e))?;
    // `_` is a LIKE wildcard and namespaces contain it: escape it.
    let escaped_namespace = config.namespace.replace('_', "\\_");
    let pattern = format!("gauntlet:{}:%", escaped_namespace);
    let own = application_name(&config.namespace, std::process::id());
    let started = Instant::now();
    loop {
        let stuck: i64 = observer
            .query_one(
                "SELECT count(*) FROM pg_stat_activity \
                 WHERE application_name LIKE $1 ESCAPE '\\' AND application_name <> $2",
                &[&pattern, &own],
            )
            .map_err(text)?
            .get(0);
        if stuck == 0 {
            return Ok(());
        }
        if started.elapsed() >= deadline {
            return Err(format!(
                "quiesce: {stuck} connection(s) of namespace {} held by other processes \
                 still live after {deadline:?}",
                config.namespace
            ));
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Deletes every `evidence_productions` row of the state instance at
/// `db_path`, or only the rows at `locator` when one is given. Returns how
/// many rows the statement removed, so a test can assert the damage touched
/// exactly the expected rows.
pub fn delete_evidence_production_rows(
    db_path: &Path,
    locator: Option<&str>,
) -> Result<usize, String> {
    let mut client = client_at(db_path)?;
    let removed = match locator {
        None => client
            .execute("DELETE FROM evidence_productions", &[])
            .map_err(text)?,
        Some(locator) => client
            .execute(
                "DELETE FROM evidence_productions WHERE locator = $1",
                &[&locator],
            )
            .map_err(text)?,
    };
    usize::try_from(removed).map_err(text)
}

/// REGRESSES the LATEST checkpoint's (the exact row `resume` selects —
/// `iteration_no DESC, seq DESC` per run) `event_log_offset` to
/// `new_offset`, out of band. Returns the PREVIOUS offset, so a test can
/// assert the surgery actually regressed an anchor that stood above it.
pub fn regress_latest_checkpoint_offset(
    db_path: &Path,
    run_id: &str,
    new_offset: u64,
) -> Result<u64, String> {
    let mut client = client_at(db_path)?;
    let previous: i64 = client
        .query_one(
            "SELECT event_log_offset FROM checkpoints WHERE run_id = $1 \
             ORDER BY iteration_no DESC, seq DESC LIMIT 1",
            &[&run_id],
        )
        .map_err(text)?
        .get(0);
    let new_offset = i64::try_from(new_offset).map_err(text)?;
    let changed = client
        .execute(
            "UPDATE checkpoints SET event_log_offset = $2 WHERE run_id = $1 AND seq = ( \
             SELECT seq FROM checkpoints WHERE run_id = $1 \
             ORDER BY iteration_no DESC, seq DESC LIMIT 1)",
            &[&run_id, &new_offset],
        )
        .map_err(text)?;
    if changed != 1 {
        return Err(format!(
            "the anchor surgery touched {changed} checkpoint rows of run {run_id}"
        ));
    }
    u64::try_from(previous).map_err(text)
}

/// Reads every durable `evidence_produced` event as
/// `(event-log offset, evidence ref, locator, stored timestamp)`.
pub fn evidence_produced_events(db_path: &Path) -> Result<Vec<(u64, String, String, i64)>, String> {
    let mut client = client_at(db_path)?;
    let rows = client
        .query(
            "SELECT \"offset\", payload, at FROM event_log \
             WHERE kind = 'evidence_produced' ORDER BY \"offset\" ASC",
            &[],
        )
        .map_err(text)?;
    let mut events = Vec::new();
    for row in rows {
        let offset: i64 = row.get(0);
        let payload: String = row.get(1);
        let at: i64 = row.get(2);
        let parsed = json::parse(&payload).map_err(text)?;
        let field = |name: &str| -> Result<String, String> {
            match parsed.get(name) {
                Some(JsonValue::Str(text)) => Ok(text.clone()),
                other => Err(format!(
                    "an evidence_produced payload carries no {name} string: {other:?}"
                )),
            }
        };
        events.push((
            u64::try_from(offset).map_err(text)?,
            field("evidence")?,
            field("locator")?,
            at,
        ));
    }
    Ok(events)
}

/// Reads every durable evidence-production row as
/// `(evidence ref, locator, produced_at, event-log offset)`.
pub fn evidence_production_rows(db_path: &Path) -> Result<Vec<(String, String, i64, i64)>, String> {
    let mut client = client_at(db_path)?;
    let rows = client
        .query(
            "SELECT evidence_ref, locator, produced_at, event_log_offset \
             FROM evidence_productions ORDER BY event_log_offset ASC",
            &[],
        )
        .map_err(text)?;
    Ok(rows
        .iter()
        .map(|row| (row.get(0), row.get(1), row.get(2), row.get(3)))
        .collect())
}

/// Reads every durable evidence-production row as
/// `(evidence ref, locator, iteration_no, produced_at, event-log offset)`.
pub fn evidence_production_rows_with_iteration(
    db_path: &Path,
) -> Result<Vec<EvidenceProductionIterationFacts>, String> {
    let mut client = client_at(db_path)?;
    let rows = client
        .query(
            "SELECT evidence_ref, locator, iteration_no, produced_at, event_log_offset \
             FROM evidence_productions ORDER BY event_log_offset ASC",
            &[],
        )
        .map_err(text)?;
    Ok(rows
        .iter()
        .map(|row| (row.get(0), row.get(1), row.get(2), row.get(3), row.get(4)))
        .collect())
}

/// DAMAGE INJECTION: deletes the durable `settlements` rows of the state
/// instance at `db_path`, or only the rows of `unit` when one is given.
/// Returns how many rows the statement removed.
pub fn delete_settlement_rows(db_path: &Path, unit: Option<&str>) -> Result<usize, String> {
    let mut client = client_at(db_path)?;
    let removed = match unit {
        None => client
            .execute("DELETE FROM settlements", &[])
            .map_err(text)?,
        Some(unit) => client
            .execute("DELETE FROM settlements WHERE unit_id = $1", &[&unit])
            .map_err(text)?,
    };
    usize::try_from(removed).map_err(text)
}

/// Reads every durable `settlement_committed` event as
/// `(event-log offset, unit id, verification ref, evidence ref, stored
/// timestamp)`.
pub fn settlement_committed_events(db_path: &Path) -> Result<Vec<SettlementEventFacts>, String> {
    let mut client = client_at(db_path)?;
    let rows = client
        .query(
            "SELECT \"offset\", payload, at FROM event_log \
             WHERE kind = 'settlement_committed' ORDER BY \"offset\" ASC",
            &[],
        )
        .map_err(text)?;
    let mut events = Vec::new();
    for row in rows {
        let offset: i64 = row.get(0);
        let payload: String = row.get(1);
        let at: i64 = row.get(2);
        let parsed = json::parse(&payload).map_err(text)?;
        let field = |name: &str| -> Result<String, String> {
            match parsed.get(name) {
                Some(JsonValue::Str(text)) => Ok(text.clone()),
                other => Err(format!(
                    "a settlement_committed payload carries no {name} string: {other:?}"
                )),
            }
        };
        events.push((
            u64::try_from(offset).map_err(text)?,
            field("unit_id")?,
            field("verification")?,
            field("evidence")?,
            at,
        ));
    }
    Ok(events)
}

/// Reads every durable settlement row as
/// `(unit id, verification ref, evidence ref, event-log offset, settled_at)`.
pub fn settlement_rows(db_path: &Path) -> Result<Vec<SettlementRowFacts>, String> {
    let mut client = client_at(db_path)?;
    let rows = client
        .query(
            "SELECT unit_id, verification_ref, evidence_ref, event_log_offset, settled_at \
             FROM settlements ORDER BY event_log_offset ASC",
            &[],
        )
        .map_err(text)?;
    Ok(rows
        .iter()
        .map(|row| (row.get(0), row.get(1), row.get(2), row.get(3), row.get(4)))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PostgresStateStore;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    /// A fresh state path per test (the file is never created; the path is
    /// the identity of the instance).
    fn fresh_path(tag: &str) -> PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir()
            .join(format!(
                "gauntlet-testsupport-{tag}-{}-{counter}-{nanos:x}",
                std::process::id()
            ))
            .join("gauntlet-state.db")
    }

    /// A store created through `open_at`, plus a raw client on the same
    /// namespace for fixture writes.
    fn store_with_fixtures(tag: &str) -> (PathBuf, postgres::Client) {
        let path = fresh_path(tag);
        let _store = PostgresStateStore::open_at(&path).expect("open_at creates the instance");
        let client = client_at(&path).expect("raw client on the instance");
        (path, client)
    }

    fn insert_event(
        client: &mut postgres::Client,
        offset: i64,
        kind: &str,
        payload: &str,
        at: i64,
    ) {
        client
            .execute(
                "INSERT INTO event_log (\"offset\", run_id, kind, payload, at) \
                 VALUES ($1, 'run-t', $2, $3, $4)",
                &[&offset, &kind, &payload, &at],
            )
            .expect("fixture event row");
    }

    fn insert_production(
        client: &mut postgres::Client,
        evidence: &str,
        locator: &str,
        iteration: i64,
        at: i64,
        offset: i64,
    ) {
        client
            .execute(
                "INSERT INTO evidence_productions (run_id, evidence_ref, producer_name, producer_version, \
                 locator, iteration_no, produced_at, event_log_offset) \
                 VALUES ('run-t', $1, 'p', '1', $2, $3, $4, $5)",
                &[&evidence, &locator, &iteration, &at, &offset],
            )
            .expect("fixture evidence production row");
    }

    fn insert_settlement(client: &mut postgres::Client, unit: &str, offset: i64, settled_at: i64) {
        client
            .execute(
                "INSERT INTO settlements (run_id, unit_id, verification_ref, evidence_ref, \
                 reference_digest, event_log_offset, settled_at) \
                 VALUES ('run-t', $1, 'v-1', 'e-1', 'd', $2, $3)",
                &[&unit, &offset, &settled_at],
            )
            .expect("fixture settlement row");
    }

    fn insert_checkpoint(client: &mut postgres::Client, iteration: i64, offset: i64) {
        client
            .execute(
                "INSERT INTO checkpoints (run_id, iteration_no, reference_digest, unit_states, \
                 budget_spent, in_flight, event_log_offset) \
                 VALUES ('run-t', $1, 'd', '[]', '{}', '[]', $2)",
                &[&iteration, &offset],
            )
            .expect("fixture checkpoint row");
    }

    #[test]
    fn evidence_production_rows_and_deletion() {
        let (path, mut client) = store_with_fixtures("evidence-rows");
        insert_production(&mut client, "ev-a", "loc-1", 1, 100, 10);
        insert_production(&mut client, "ev-b", "loc-2", 1, 101, 11);
        let rows = evidence_production_rows(&path).expect("read rows");
        assert_eq!(
            rows,
            vec![
                ("ev-a".to_owned(), "loc-1".to_owned(), 100, 10),
                ("ev-b".to_owned(), "loc-2".to_owned(), 101, 11),
            ]
        );
        let with_iteration = evidence_production_rows_with_iteration(&path).expect("read rows");
        assert_eq!(
            with_iteration,
            vec![
                ("ev-a".to_owned(), "loc-1".to_owned(), 1, 100, 10),
                ("ev-b".to_owned(), "loc-2".to_owned(), 1, 101, 11),
            ]
        );
        assert_eq!(
            delete_evidence_production_rows(&path, Some("loc-1")).expect("delete"),
            1
        );
        assert_eq!(evidence_production_rows(&path).expect("read rows").len(), 1);
        assert_eq!(
            delete_evidence_production_rows(&path, None).expect("delete"),
            1
        );
        assert!(
            evidence_production_rows(&path)
                .expect("read rows")
                .is_empty()
        );
    }

    #[test]
    fn evidence_produced_events_read_the_payload_fields() {
        let (path, mut client) = store_with_fixtures("evidence-events");
        insert_event(
            &mut client,
            1,
            "evidence_produced",
            r#"{"evidence":"ev-a","locator":"loc-1"}"#,
            500,
        );
        insert_event(&mut client, 2, "run_started", "{}", 501);
        let events = evidence_produced_events(&path).expect("read events");
        assert_eq!(
            events,
            vec![(1, "ev-a".to_owned(), "loc-1".to_owned(), 500)]
        );
    }

    #[test]
    fn regress_latest_checkpoint_offset_moves_only_the_latest_anchor() {
        let (path, mut client) = store_with_fixtures("checkpoint");
        insert_checkpoint(&mut client, 1, 7);
        insert_checkpoint(&mut client, 2, 9);
        let previous = regress_latest_checkpoint_offset(&path, "run-t", 4).expect("regress");
        assert_eq!(previous, 9);
        let offsets: Vec<i64> = client
            .query(
                "SELECT event_log_offset FROM checkpoints ORDER BY iteration_no ASC",
                &[],
            )
            .expect("read checkpoints")
            .iter()
            .map(|row| row.get(0))
            .collect();
        assert_eq!(offsets, vec![7, 4]);
    }

    #[test]
    fn quiesce_ignores_this_process_but_waits_out_another_process() {
        let path = fresh_path("quiesce");
        let config = PostgresConfig::for_state_path(&path);
        // This process's own open store must not block the barrier.
        let store = PostgresStateStore::open_at(&path).expect("open_at creates the instance");
        quiesce_writers(&path, Duration::from_secs(10))
            .expect("this process's own store is not a foreign writer");
        drop(store);
        // A connection tagged with a different pid is a foreign writer.
        let mut foreign_config: postgres::Config = config.url.parse().expect("the url parses");
        foreign_config.application_name(&application_name(&config.namespace, 99_999_999));
        let foreign = foreign_config
            .connect(postgres::NoTls)
            .expect("the foreign connection opens");
        let started = Instant::now();
        let err = quiesce_writers(&path, Duration::from_millis(200))
            .expect_err("a foreign-pid connection is a writer the barrier must wait for");
        assert!(
            err.contains("still live"),
            "the timeout names the stuck count: {err}"
        );
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "the short deadline bounds the wait"
        );
        drop(foreign);
        quiesce_writers(&path, Duration::from_secs(10))
            .expect("with the foreign connection closed the instance is quiescent");
    }

    #[test]
    fn settlement_rows_and_events_and_deletion() {
        let (path, mut client) = store_with_fixtures("settlement");
        insert_event(
            &mut client,
            3,
            "settlement_committed",
            r#"{"unit_id":"u-1","verification":"v-1","evidence":"e-1"}"#,
            600,
        );
        insert_settlement(&mut client, "u-1", 3, 600);
        insert_settlement(&mut client, "u-2", 5, 601);
        let events = settlement_committed_events(&path).expect("read events");
        assert_eq!(
            events,
            vec![(3, "u-1".to_owned(), "v-1".to_owned(), "e-1".to_owned(), 600)]
        );
        let rows = settlement_rows(&path).expect("read rows");
        assert_eq!(
            rows,
            vec![
                ("u-1".to_owned(), "v-1".to_owned(), "e-1".to_owned(), 3, 600),
                ("u-2".to_owned(), "v-1".to_owned(), "e-1".to_owned(), 5, 601),
            ]
        );
        assert_eq!(
            delete_settlement_rows(&path, Some("u-1")).expect("delete"),
            1
        );
        assert_eq!(settlement_rows(&path).expect("read rows").len(), 1);
        assert_eq!(delete_settlement_rows(&path, None).expect("delete"), 1);
        assert!(settlement_rows(&path).expect("read rows").is_empty());
    }
}
