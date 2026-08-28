//! The `EventLog` adapter over the embedded database (spec §6.2: "Append-only,
//! fsync-on-append, the durability spine of §16.3").
//!
//! One table, and no statement this crate issues against it is anything but
//! `INSERT` or `SELECT`: there is no UPDATE, no DELETE, and no path that
//! rewrites an offset. Offsets come from `INTEGER PRIMARY KEY AUTOINCREMENT`,
//! so they are strictly increasing and never reused.
//!
//! Durability: the connection runs with `synchronous=FULL`, which makes every
//! commit fsync before it returns — that is the fsync-on-append the port
//! contract names, and it is why §16.3 step 1 is a real ordering step and not
//! a buffering detail.
//!
//! Damage is loud: a row whose payload does not decode into the domain event
//! vocabulary, a database the reader can certify is corrupt, a negative
//! stored timestamp (impossible through this adapter, so always tamper
//! evidence — never clamped to epoch zero), or a hole between consecutive
//! offsets (rows removed out of band) is [`EventLogError::Damaged`] —
//! recovery halts, it never skips (spec §18.3).

use std::path::Path;
use std::sync::Mutex;

use gauntlet_domain::{EventLogOffset, RunId};
use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::event_log::{EventLog, EventLogError, LogEntry};
use gauntlet_ports::state_store::{RunEvent, RunEventKind};
use rusqlite::Connection;

use crate::encode;

/// An append-only event log in one embedded database file.
pub struct SqliteEventLog {
    conn: Mutex<Connection>,
}

impl SqliteEventLog {
    /// Opens (creating if needed) the log at `path`, applying any migrations
    /// not yet recorded. Open failures surface as `AppendFailed` — the
    /// closest frozen variant; nothing was being appended yet, but a log
    /// that cannot open can record no event, which is that variant's
    /// contract.
    pub fn open(path: &Path) -> Result<Self, EventLogError> {
        let conn =
            open_connection(path).map_err(|detail| EventLogError::AppendFailed { detail })?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// An in-memory log, for tests that never need a second process.
    pub fn open_in_memory() -> Result<Self, EventLogError> {
        let conn = open_connection(Path::new(":memory:"))
            .map_err(|detail| EventLogError::AppendFailed { detail })?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }
}

/// Opens a connection with the durability pragmas this adapter's contracts
/// depend on: a single-file rollback journal and `synchronous=FULL`, so a
/// committed append is on disk before the call returns.
pub(crate) fn open_connection(path: &Path) -> Result<Connection, String> {
    let conn = Connection::open(path).map_err(|err| format!("opening failed: {err}"))?;
    conn.pragma_update(None, "journal_mode", "DELETE")
        .map_err(|err| format!("setting the journal mode failed: {err}"))?;
    conn.pragma_update(None, "synchronous", "FULL")
        .map_err(|err| format!("setting synchronous durability failed: {err}"))?;
    conn.busy_timeout(std::time::Duration::from_millis(5_000))
        .map_err(|err| format!("setting the busy timeout failed: {err}"))?;
    crate::migrations::apply(&conn).map_err(|err| err.to_string())?;
    Ok(conn)
}

/// Reads one durable append: `INSERT` in its own transaction. With
/// `synchronous=FULL` the commit is the fsync.
pub(crate) fn append_to_log(
    conn: &Connection,
    event: &RunEvent,
) -> Result<EventLogOffset, EventLogError> {
    let (kind, payload) = encode::encode_event(&event.kind);
    let insert_kind = kind.clone();
    let append = || {
        conn.execute(
            "INSERT INTO event_log (run_id, kind, payload, at) VALUES (?1, ?2, ?3, ?4)",
            (
                event.run_id.to_string(),
                insert_kind,
                payload,
                event.at.get(),
            ),
        )
    };
    append().map_err(|err| EventLogError::AppendFailed {
        detail: format!(
            "appending a {kind} event for run {} failed: {err} — the event was NOT recorded",
            event.run_id
        ),
    })?;
    let offset = conn.last_insert_rowid();
    Ok(EventLogOffset::new(u64::try_from(offset).unwrap_or(0)))
}

/// Reads every entry at or after `offset`, in append order. Damaged rows are
/// [`EventLogError::Damaged`]; an offset past the end replays empty. Damage
/// includes a negative stored timestamp (this adapter can never write one,
/// so any negative read is tamper evidence, never a value to clamp to epoch
/// zero) and a hole between consecutive offsets (AUTOINCREMENT never reuses
/// ids, so a gap means rows were removed out of band); contiguity is checked
/// from the first returned row forward, so replaying from a point at or
/// after historical damage still sees a contiguous tail.
pub(crate) fn read_entries(
    conn: &Connection,
    offset: EventLogOffset,
) -> Result<Vec<LogEntry>, EventLogError> {
    let mut statement = map_read_error(
        conn.prepare(
            "SELECT offset, run_id, kind, payload, at FROM event_log WHERE offset >= ?1 \
             ORDER BY offset ASC",
        ),
        "preparing the replay query",
    )?;
    let rows = map_read_error(statement.query([offset.get()]), "starting the replay")?;
    let mut entries = Vec::new();
    let mut rows = rows;
    let mut next_expected: Option<u64> = None;
    while let Some(row) = map_read_error(rows.next(), "stepping the replay")? {
        let raw_offset: i64 = row.get(0).map_err(read_failed_at("reading an offset"))?;
        let run_id: String = row.get(1).map_err(read_failed_at("reading a run id"))?;
        let kind: String = row.get(2).map_err(read_failed_at("reading a kind"))?;
        let payload: String = row.get(3).map_err(read_failed_at("reading a payload"))?;
        let raw_at: i64 = row.get(4).map_err(read_failed_at("reading a timestamp"))?;
        let entry_offset = u64::try_from(raw_offset).unwrap_or(0);
        if let Some(expected) = next_expected
            && entry_offset != expected
        {
            return Err(EventLogError::Damaged {
                detail: format!(
                    "expected offset {expected} but found {entry_offset} — rows were removed \
                     out of band; an append-only log has no holes"
                ),
            });
        }
        next_expected = Some(entry_offset.saturating_add(1));
        if raw_at < 0 {
            return Err(EventLogError::Damaged {
                detail: format!(
                    "offset {raw_offset}: the stored timestamp {raw_at} is negative — this \
                     adapter never writes one, so the row was damaged or tampered out of band"
                ),
            });
        }
        let run = RunId::new(&run_id).map_err(|err| EventLogError::Damaged {
            detail: format!("offset {raw_offset}: the stored run id is not a ULID: {err}"),
        })?;
        let decoded = encode::decode_event(
            run,
            gauntlet_domain::EpochSeconds::new(raw_at as u64),
            &kind,
            &payload,
        )
        .map_err(|err| EventLogError::Damaged {
            detail: format!("offset {raw_offset}: {err}"),
        })?;
        entries.push(LogEntry {
            offset: EventLogOffset::new(entry_offset),
            event: decoded,
        });
    }
    Ok(entries)
}

/// `MAX(offset)`, `None` on an empty log.
pub(crate) fn last_offset_of(conn: &Connection) -> Result<Option<EventLogOffset>, EventLogError> {
    let offset: Option<i64> = map_read_error(
        conn.query_row("SELECT MAX(offset) FROM event_log", [], |row| row.get(0)),
        "reading the last offset",
    )?;
    Ok(offset.map(|value| EventLogOffset::new(u64::try_from(value).unwrap_or(0))))
}

fn read_failed_at(step: &'static str) -> impl Fn(rusqlite::Error) -> EventLogError {
    move |err| EventLogError::ReadFailed {
        detail: format!("{step} failed: {err}"),
    }
}

/// A read-step failure is `Damaged` when the engine certifies corruption
/// (corrupt database, not-a-database); anything else is `ReadFailed`.
fn map_read_error<T>(
    outcome: Result<T, rusqlite::Error>,
    step: &'static str,
) -> Result<T, EventLogError> {
    outcome.map_err(|err| match &err {
        rusqlite::Error::SqliteFailure(ffi, _)
            if ffi.code == rusqlite::ErrorCode::DatabaseCorrupt
                || ffi.code == rusqlite::ErrorCode::NotADatabase =>
        {
            EventLogError::Damaged {
                detail: format!("{step} failed: the engine reports a damaged database: {err}"),
            }
        }
        _ => EventLogError::ReadFailed {
            detail: format!("{step} failed: {err}"),
        },
    })
}

impl EventLog for SqliteEventLog {
    fn append(
        &self,
        event: RunEvent,
        _deadline: Deadline,
    ) -> Result<EventLogOffset, EventLogError> {
        // COR3 (confirmation round 1, F-1): the settlement kind is
        // authority-minted — it reaches this log only through the
        // `StateStore`'s earned `commit_settlement` (consuming an
        // `AdmittedSettlement`). This log is the same durability spine the
        // §18.3 replay mints settlement rows from, so an ungated append
        // here forged the durable settlement substrate exactly as the
        // store's own generic append did.
        if let RunEventKind::SettlementCommitted { unit_id, .. } = &event.kind {
            return Err(EventLogError::SettlementEventNotEarned {
                run_id: event.run_id.clone(),
                unit_id: unit_id.clone(),
            });
        }
        let conn = self
            .conn
            .lock()
            .expect("the log connection is not poisoned");
        append_to_log(&conn, &event)
    }

    fn read_from(
        &self,
        offset: EventLogOffset,
        _deadline: Deadline,
    ) -> Result<Vec<LogEntry>, EventLogError> {
        let conn = self
            .conn
            .lock()
            .expect("the log connection is not poisoned");
        read_entries(&conn, offset)
    }

    fn last_offset(&self, _deadline: Deadline) -> Result<Option<EventLogOffset>, EventLogError> {
        let conn = self
            .conn
            .lock()
            .expect("the log connection is not poisoned");
        last_offset_of(&conn)
    }
}
