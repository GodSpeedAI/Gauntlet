//! Persistence adapter (spec §4.1, §18; plan Task 6): `StateStore` and
//! `EventLog` over an embedded SQL database — append-only event log with
//! fsync-per-append, migrations in-repo, deterministic export. Database
//! specifics live here; the application layer never sees them.
//!
//! - [`SqliteEventLog`] — the §6.2 durability spine: one table, INSERT and
//!   SELECT only, `synchronous=FULL` so every append is durable before it
//!   returns, damaged rows are a typed `Damaged` error, never a skip.
//! - [`SqliteStateStore`] — run identity, §18.2 checkpoints, §16.3
//!   settlement rows, the remaining §18.1 record kinds, and the
//!   deterministic JSONL export.
//! - Migrations are embedded SQL files applied in ascending order and
//!   recorded in a `schema_migrations` table; the set is append-only.
//!
//! The `crash_helper` binary beside this crate is the crash-injection
//! instrument for the §16.3/§18.3 recovery tests: it performs scripted
//! sequences and `std::process::abort()`s at a named step, so the tests kill
//! a real process and reopen the file, rather than dropping a connection.
//!
//! The capability-named re-exports ([`EmbeddedStateStore`],
//! [`EmbeddedEventLog`]) are the handles the composition root may spell:
//! the boundary denylist masks this crate's NAME (vendor names belong in
//! adapters, AGENTS.md Naming), and a vendor-flavored type name would leave
//! a bare vendor substring after masking (spec §8.4: "the runtime names
//! capabilities, never products"; spec §18.1: "a single-file embedded
//! database"). The vendor-named paths above remain canonical inside the
//! adapter layer, where vendor names belong.

mod capability_ledger;
mod encode;
mod event_log;
mod migrations;
mod revision_ledger;
mod state_store;

/// The configuration token that selects this persistence adapter
/// (`state.backend` / `GAUNTLET_STATE_BACKEND`). The token is the
/// adapter's to spell (vendor names belong in adapters, AGENTS.md
/// Naming): the composition root and its tests import this constant
/// rather than carrying the vendor substring themselves — the boundary
/// denylist masks this crate's NAME in imports, never a bare vendor
/// literal outside it.
pub const STATE_BACKEND_TOKEN: &str = "sqlite";

/// The capability name of [`SqliteCapabilityLedger`] for the composition
/// root — the §19.1 developmental-memory and capability-register store
/// (plan Task 22).
pub use capability_ledger::SqliteCapabilityLedger as EmbeddedCapabilityLedger;
/// The capability name of [`SqliteEventLog`] for the composition root —
/// see the module docs.
pub use event_log::SqliteEventLog as EmbeddedEventLog;
/// The capability name of [`SqliteRevisionLedger`] for the composition
/// root — the §19.3/§19.4 candidate-revision store (plan Task 23).
pub use revision_ledger::SqliteRevisionLedger as EmbeddedRevisionLedger;
/// The capability name of [`SqliteStateStore`] for the composition root —
/// see the module docs.
pub use state_store::SqliteStateStore as EmbeddedStateStore;

pub use capability_ledger::SqliteCapabilityLedger;
pub use event_log::SqliteEventLog;
pub use revision_ledger::SqliteRevisionLedger;
pub use state_store::{RecordKind, SqliteStateStore};

/// OUT-OF-BAND DAMAGE INJECTION (COR-R3-1, falsifier round 3; test-support
/// only, hidden from the documented API surface): the falsifier's exact
/// reproduction — production ROWS deleted out of a settled run's database
/// while every durable event and evidence byte stays — must run through the
/// REAL composed binary, and a write-once store has, by design, no
/// production delete. The surgery lives HERE, beside the `crash_helper`
/// binary, where the engine's name belongs (the boundary denylist masks
/// this crate's name, never a bare vendor literal outside adapters). It is
/// never a production surface: nothing in the runtime calls into it.
#[doc(hidden)]
pub mod test_support {
    /// One durable `settlement_committed` event's reader shape:
    /// `(event-log offset, unit id, verification ref, evidence ref,
    /// stored timestamp)` — see [`settlement_committed_events`].
    pub type SettlementEventFacts = (u64, String, String, String, i64);
    /// One durable settlement row's reader shape: `(unit id, verification
    /// ref, evidence ref, event-log offset, settled_at)` — see
    /// [`settlement_rows`].
    pub type SettlementRowFacts = (String, String, String, i64, i64);

    /// Deletes every `evidence_productions` row of the database at
    /// `db_path`, or only the rows at `locator` when one is given. Returns
    /// how many rows the statement removed, so a test can assert the
    /// damage touched exactly the expected rows.
    pub fn delete_evidence_production_rows(
        db_path: &std::path::Path,
        locator: Option<&str>,
    ) -> Result<usize, String> {
        let connection =
            ::rusqlite::Connection::open(db_path).map_err(|error| error.to_string())?;
        match locator {
            None => connection
                .execute("DELETE FROM evidence_productions", ())
                .map_err(|error| error.to_string()),
            Some(locator) => connection
                .execute(
                    "DELETE FROM evidence_productions WHERE locator = ?1",
                    [locator],
                )
                .map_err(|error| error.to_string()),
        }
    }

    /// REGRESSES the LATEST checkpoint's (the exact row `resume` selects —
    /// `iteration_no DESC, seq DESC` per run) `event_log_offset` to
    /// `new_offset`, out of band. Returns the PREVIOUS offset, so a test
    /// can assert the surgery actually regressed an anchor that stood
    /// above it. COR-R4-1 (falsifier round 4, F-R4-3): this is the
    /// reproduction's precondition — an A5-shaped orphan event durable
    /// ABOVE the resume anchor on an otherwise fully settled run. The
    /// real store does not validate a checkpoint's offset against the log
    /// head; that gap is the RECORDED "checkpoint() forged-row projection
    /// class (planning-layer)" deferral, which stays out of task20's
    /// scope. This instrument exists so the correction's own contract
    /// (recovery-completed damage ⇒ settled no-op) is pinned through the
    /// real composed binary against that given damage — never a
    /// production surface; nothing in the runtime calls into it.
    pub fn regress_latest_checkpoint_offset(
        db_path: &std::path::Path,
        run_id: &str,
        new_offset: u64,
    ) -> Result<u64, String> {
        let connection =
            ::rusqlite::Connection::open(db_path).map_err(|error| error.to_string())?;
        let previous: i64 = connection
            .query_row(
                "SELECT event_log_offset FROM checkpoints WHERE run_id = ?1 \
                 ORDER BY iteration_no DESC, seq DESC LIMIT 1",
                [run_id],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        let changed = connection
            .execute(
                "UPDATE checkpoints SET event_log_offset = ?2 WHERE run_id = ?1 AND seq = ( \
                 SELECT seq FROM checkpoints WHERE run_id = ?1 \
                 ORDER BY iteration_no DESC, seq DESC LIMIT 1)",
                ::rusqlite::params![
                    run_id,
                    i64::try_from(new_offset).map_err(|error| error.to_string())?
                ],
            )
            .map_err(|error| error.to_string())?;
        if changed != 1 {
            return Err(format!(
                "the anchor surgery touched {changed} checkpoint rows of run {run_id}"
            ));
        }
        u64::try_from(previous).map_err(|error| error.to_string())
    }

    /// Reads every durable `evidence_produced` event as
    /// `(event-log offset, evidence ref, locator, stored timestamp)` —
    /// the facts a recovery-completed row must be bound to (COR-R4-1's
    /// E2E leg asserts the re-minted row against the EVENT's own
    /// identity, byte-level, the way the falsifier verified it).
    pub fn evidence_produced_events(
        db_path: &std::path::Path,
    ) -> Result<Vec<(u64, String, String, i64)>, String> {
        let connection =
            ::rusqlite::Connection::open(db_path).map_err(|error| error.to_string())?;
        let mut statement = connection
            .prepare(
                "SELECT offset, payload, at FROM event_log WHERE kind = 'evidence_produced' \
                 ORDER BY offset ASC",
            )
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            })
            .map_err(|error| error.to_string())?;
        let mut events = Vec::new();
        for row in rows {
            let (offset, payload, at) = row.map_err(|error| error.to_string())?;
            let parsed =
                gauntlet_ports::json::parse(&payload).map_err(|error| error.to_string())?;
            let field = |name: &str| -> Result<String, String> {
                match parsed.get(name) {
                    Some(gauntlet_ports::json::JsonValue::Str(text)) => Ok(text.clone()),
                    other => Err(format!(
                        "an evidence_produced payload carries no {name} string: {other:?}"
                    )),
                }
            };
            events.push((
                u64::try_from(offset).map_err(|error| error.to_string())?,
                field("evidence")?,
                field("locator")?,
                at,
            ));
        }
        Ok(events)
    }

    /// Reads every durable evidence-production row as
    /// `(evidence ref, locator, produced_at, event-log offset)` — the
    /// reader half of the COR-R4-1 E2E leg's byte-level mint assertion.
    pub fn evidence_production_rows(
        db_path: &std::path::Path,
    ) -> Result<Vec<(String, String, i64, i64)>, String> {
        let connection =
            ::rusqlite::Connection::open(db_path).map_err(|error| error.to_string())?;
        let mut statement = connection
            .prepare(
                "SELECT evidence_ref, locator, produced_at, event_log_offset \
                 FROM evidence_productions ORDER BY event_log_offset ASC",
            )
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            })
            .map_err(|error| error.to_string())?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row.map_err(|error| error.to_string())?);
        }
        Ok(out)
    }

    /// Out-of-band DAMAGE INJECTION (COR-R5, falsifier round 5; test-support
    /// only, hidden from the documented API surface): deletes the durable
    /// `settlements` rows of the database at `db_path`, or only the rows of
    /// `unit` when one is given. The keyed table is write-once by design
    /// (the §16.3 idempotent insert has no delete), so the falsifier's
    /// settlement-arm reproduction surgery — the keyed row gone while the
    /// durable `SettlementCommitted` event stays — goes through HERE,
    /// beside the other damage injections. It is never a production
    /// surface: nothing in the runtime calls into it. Returns how many rows
    /// the statement removed, so a test can assert the damage touched
    /// exactly the expected rows.
    pub fn delete_settlement_rows(
        db_path: &std::path::Path,
        unit: Option<&str>,
    ) -> Result<usize, String> {
        let connection =
            ::rusqlite::Connection::open(db_path).map_err(|error| error.to_string())?;
        match unit {
            None => connection
                .execute("DELETE FROM settlements", ())
                .map_err(|error| error.to_string()),
            Some(unit) => connection
                .execute("DELETE FROM settlements WHERE unit_id = ?1", [unit])
                .map_err(|error| error.to_string()),
        }
    }

    /// Reads every durable `settlement_committed` event as
    /// `(event-log offset, unit id, verification ref, evidence ref, stored
    /// timestamp)` — the EVENT-side facts a recovery-completed settlement
    /// row must be bound to (the COR-R5-1 E2E leg pins the re-inserted row
    /// against the event's own identity, byte-level, the way the falsifier
    /// verified it).
    pub fn settlement_committed_events(
        db_path: &std::path::Path,
    ) -> Result<Vec<SettlementEventFacts>, String> {
        let connection =
            ::rusqlite::Connection::open(db_path).map_err(|error| error.to_string())?;
        let mut statement = connection
            .prepare(
                "SELECT offset, payload, at FROM event_log WHERE kind = 'settlement_committed' \
                 ORDER BY offset ASC",
            )
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            })
            .map_err(|error| error.to_string())?;
        let mut events = Vec::new();
        for row in rows {
            let (offset, payload, at) = row.map_err(|error| error.to_string())?;
            let parsed =
                gauntlet_ports::json::parse(&payload).map_err(|error| error.to_string())?;
            let field = |name: &str| -> Result<String, String> {
                match parsed.get(name) {
                    Some(gauntlet_ports::json::JsonValue::Str(text)) => Ok(text.clone()),
                    other => Err(format!(
                        "a settlement_committed payload carries no {name} string: {other:?}"
                    )),
                }
            };
            events.push((
                u64::try_from(offset).map_err(|error| error.to_string())?,
                field("unit_id")?,
                field("verification")?,
                field("evidence")?,
                at,
            ));
        }
        Ok(events)
    }

    /// Reads every durable settlement row as
    /// `(unit id, verification ref, evidence ref, event-log offset,
    /// settled_at)` — the ROW-side facts the COR-R5-1 E2E leg pins the
    /// re-inserted settlement against (the row must carry the EVENT's own
    /// offset and stored timestamp, never the recovery's clock).
    pub fn settlement_rows(db_path: &std::path::Path) -> Result<Vec<SettlementRowFacts>, String> {
        let connection =
            ::rusqlite::Connection::open(db_path).map_err(|error| error.to_string())?;
        let mut statement = connection
            .prepare(
                "SELECT unit_id, verification_ref, evidence_ref, event_log_offset, settled_at \
                 FROM settlements ORDER BY event_log_offset ASC",
            )
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                ))
            })
            .map_err(|error| error.to_string())?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row.map_err(|error| error.to_string())?);
        }
        Ok(out)
    }
}
