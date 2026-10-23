//! The `EventLog` adapter over PostgreSQL (spec §6.2: "Append-only,
//! fsync-on-append, the durability spine of §16.3").
//!
//! One table plus a counter table for gapless offsets:
//! - Offsets are strictly increasing and never reused
//! - Each append is one transaction: atomically increment the counter and insert the row
//! - Durability: PostgreSQL's default fsync on commit ensures the append is durable
//!
//! Damage is loud: a row whose payload does not decode into the domain event
//! vocabulary or a hole between consecutive offsets is [`EventLogError::Damaged`] —
//! recovery halts, it never skips (spec §18.3).

use std::sync::Mutex;

use crate::sql::Conn;
use gauntlet_domain::EventLogOffset;
use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::event_log::{EventLog, EventLogError, LogEntry};
use gauntlet_ports::state_store::{RunEvent, RunEventKind};
use postgres::Client;

use crate::db::PostgresConfig;
use crate::encode;
use crate::migrations;

/// An append-only event log in PostgreSQL with gapless offsets via a counter table.
pub struct PostgresEventLog {
    client: Mutex<Client>,
}

impl PostgresEventLog {
    /// Opens the log of the state instance at `path` (see [`crate::locator`]):
    /// the namespace is derived from the path, the URL from the environment.
    pub fn open_at(path: &std::path::Path) -> Result<Self, EventLogError> {
        Self::open(&PostgresConfig::for_state_path(path))
    }

    /// Opens (creating if needed) the log by connecting and applying any migrations
    /// not yet recorded. Open failures surface as `AppendFailed`.
    pub fn open(config: &PostgresConfig) -> Result<Self, EventLogError> {
        let mut client =
            crate::db::connect(config).map_err(|detail| EventLogError::AppendFailed { detail })?;
        migrations::apply(&mut client).map_err(|err| EventLogError::AppendFailed {
            detail: err.to_string(),
        })?;
        Ok(Self {
            client: Mutex::new(client),
        })
    }
}

/// Appends one durable event to the log using a state store connection.
/// This is a helper function used by the state store.
pub(crate) fn append_to_log(
    conn: &Conn,
    event: &RunEvent,
) -> Result<gauntlet_domain::EventLogOffset, gauntlet_ports::event_log::EventLogError> {
    let (kind, payload) = encode::encode_event(&event.kind);
    let run_id_str = event.run_id.to_string();
    let at = event.at.get();

    // Access the client through RefCell and work within that borrow scope
    let offset: i64 = {
        let mut client_ref = conn.client_mut();
        let client = &mut *client_ref;

        // Start a transaction to ensure atomicity of counter increment and insert
        let mut transaction = client.transaction().map_err(|err| {
            gauntlet_ports::event_log::EventLogError::AppendFailed {
                detail: format!("failed to start transaction: {err}"),
            }
        })?;

        // Atomically increment the counter and get the new offset
        let offset: i64 = transaction
            .query_one(
                "UPDATE event_log_counter SET next_offset = next_offset + 1 \
                 WHERE id = 1 RETURNING next_offset",
                &[],
            )
            .map_err(|err| gauntlet_ports::event_log::EventLogError::AppendFailed {
                detail: format!(
                    "appending a {kind} event for run {run_id_str} failed: {err} — the event was NOT recorded"
                ),
            })?
            .get(0);

        // Insert the event with the allocated offset
        transaction
            .execute(
                "INSERT INTO event_log (\"offset\", run_id, kind, payload, at) VALUES ($1, $2, $3, $4, $5)",
                &[&offset, &run_id_str, &kind, &payload, &(at as i64)],
            )
            .map_err(|err| gauntlet_ports::event_log::EventLogError::AppendFailed {
                detail: format!(
                    "appending a {kind} event for run {run_id_str} failed: {err} — the event was NOT recorded"
                ),
            })?;

        // Commit the transaction
        transaction.commit().map_err(|err| {
            gauntlet_ports::event_log::EventLogError::AppendFailed {
                detail: format!(
                    "committing the append for {kind} event for run {run_id_str} failed: {err}"
                ),
            }
        })?;

        offset
    };

    Ok(gauntlet_domain::EventLogOffset::new(
        u64::try_from(offset).unwrap_or(0),
    ))
}

/// Reads entries from the event log starting at the given offset.
/// This is a helper function used by the state store.
pub(crate) fn read_entries(
    conn: &Conn,
    offset: EventLogOffset,
) -> Result<Vec<gauntlet_ports::event_log::LogEntry>, gauntlet_ports::event_log::EventLogError> {
    let rows = conn
        .client_mut()
        .query(
            "SELECT \"offset\", run_id, kind, payload, at FROM event_log WHERE \"offset\" >= $1 \
             ORDER BY \"offset\" ASC",
            &[&(offset.get() as i64)],
        )
        .map_err(|err| gauntlet_ports::event_log::EventLogError::ReadFailed {
            detail: format!("reading the log failed: {err}"),
        })?;

    let mut entries = Vec::new();
    let mut next_expected: Option<i64> = None;

    for row in rows {
        let raw_offset: i64 = row.get(0);
        let run_id_str: String = row.get(1);
        let kind: String = row.get(2);
        let payload: String = row.get(3);
        let raw_at: i64 = row.get(4);

        // Check for holes in the offset sequence
        if let Some(expected) = next_expected
            && raw_offset != expected
        {
            return Err(gauntlet_ports::event_log::EventLogError::Damaged {
                detail: format!(
                    "expected offset {expected} but found {raw_offset} — rows were removed \
                         out of band; an append-only log has no holes"
                ),
            });
        }
        next_expected = Some(raw_offset.saturating_add(1));

        // Check for negative timestamp (tamper evidence)
        if raw_at < 0 {
            return Err(gauntlet_ports::event_log::EventLogError::Damaged {
                detail: format!(
                    "offset {raw_offset}: the stored timestamp {raw_at} is negative — this \
                     adapter never writes one, so the row was damaged or tampered out of band"
                ),
            });
        }

        // Decode the event
        let run_id = gauntlet_domain::RunId::new(&run_id_str).map_err(|err| {
            gauntlet_ports::event_log::EventLogError::Damaged {
                detail: format!("offset {raw_offset}: the stored run id is not a ULID: {err}"),
            }
        })?;

        let decoded = encode::decode_event(
            run_id,
            gauntlet_domain::EpochSeconds::new(raw_at as u64),
            &kind,
            &payload,
        )
        .map_err(|err| gauntlet_ports::event_log::EventLogError::Damaged {
            detail: format!("offset {raw_offset}: {err}"),
        })?;

        entries.push(gauntlet_ports::event_log::LogEntry {
            offset: EventLogOffset::new(u64::try_from(raw_offset).unwrap_or(0)),
            event: decoded,
        });
    }

    Ok(entries)
}

impl EventLog for PostgresEventLog {
    fn append(
        &self,
        event: RunEvent,
        _deadline: Deadline,
    ) -> Result<EventLogOffset, EventLogError> {
        // COR3 (confirmation round 1, F-1): the settlement kind is authority-minted —
        // it reaches this log only through the `StateStore`'s earned `commit_settlement`
        // (consuming an `AdmittedSettlement`). This log is the same durability spine the
        // §18.3 replay mints settlement rows from, so an ungated append here forged the
        // durable settlement substrate exactly as the store's own generic append did.
        if let RunEventKind::SettlementCommitted { unit_id, .. } = &event.kind {
            return Err(EventLogError::SettlementEventNotEarned {
                run_id: event.run_id.clone(),
                unit_id: unit_id.clone(),
            });
        }

        let (kind, payload) = encode::encode_event(&event.kind);
        let run_id_str = event.run_id.to_string();
        let at = event.at.get();

        let mut client = self
            .client
            .lock()
            .expect("the log connection is not poisoned");

        // Start a transaction to ensure atomicity of counter increment and insert
        let mut transaction = client
            .transaction()
            .map_err(|err| EventLogError::AppendFailed {
                detail: format!("failed to start transaction: {err}"),
            })?;

        // Atomically increment the counter and get the new offset
        let offset: i64 = transaction
            .query_one(
                "UPDATE event_log_counter SET next_offset = next_offset + 1 \
                 WHERE id = 1 RETURNING next_offset",
                &[],
            )
            .map_err(|err| EventLogError::AppendFailed {
                detail: format!(
                    "appending a {kind} event for run {run_id_str} failed: {err} — \
                     the event was NOT recorded"
                ),
            })?
            .get(0);

        // Insert the event with the allocated offset
        transaction
            .execute(
                "INSERT INTO event_log (\"offset\", run_id, kind, payload, at) VALUES ($1, $2, $3, $4, $5)",
                &[&offset, &run_id_str, &kind, &payload, &(at as i64)],
            )
            .map_err(|err| EventLogError::AppendFailed {
                detail: format!(
                    "appending a {kind} event for run {run_id_str} failed: {err} — \
                     the event was NOT recorded"
                ),
            })?;

        // Commit the transaction
        transaction
            .commit()
            .map_err(|err| EventLogError::AppendFailed {
                detail: format!(
                    "committing the append for {kind} event for run {run_id_str} failed: {err}"
                ),
            })?;

        Ok(EventLogOffset::new(u64::try_from(offset).unwrap_or(0)))
    }

    fn read_from(
        &self,
        offset: EventLogOffset,
        _deadline: Deadline,
    ) -> Result<Vec<LogEntry>, EventLogError> {
        let mut client = self
            .client
            .lock()
            .expect("the log connection is not poisoned");

        let rows = client
            .query(
                "SELECT \"offset\", run_id, kind, payload, at FROM event_log WHERE \"offset\" >= $1 \
                 ORDER BY \"offset\" ASC",
                &[&(offset.get() as i64)],
            )
            .map_err(|err| EventLogError::ReadFailed {
                detail: format!("reading the log failed: {err}"),
            })?;

        let mut entries = Vec::new();
        let mut next_expected: Option<i64> = None;

        for row in rows {
            let raw_offset: i64 = row.get(0);
            let run_id_str: String = row.get(1);
            let kind: String = row.get(2);
            let payload: String = row.get(3);
            let raw_at: i64 = row.get(4);

            // Check for holes in the offset sequence
            if let Some(expected) = next_expected
                && raw_offset != expected
            {
                return Err(EventLogError::Damaged {
                    detail: format!(
                        "expected offset {expected} but found {raw_offset} — rows were removed \
                             out of band; an append-only log has no holes"
                    ),
                });
            }
            next_expected = Some(raw_offset.saturating_add(1));

            // Check for negative timestamp (tamper evidence)
            if raw_at < 0 {
                return Err(EventLogError::Damaged {
                    detail: format!(
                        "offset {raw_offset}: the stored timestamp {raw_at} is negative — this \
                         adapter never writes one, so the row was damaged or tampered out of band"
                    ),
                });
            }

            // Decode the event
            let run_id =
                gauntlet_domain::RunId::new(&run_id_str).map_err(|err| EventLogError::Damaged {
                    detail: format!("offset {raw_offset}: the stored run id is not a ULID: {err}"),
                })?;

            let decoded = encode::decode_event(
                run_id,
                gauntlet_domain::EpochSeconds::new(raw_at as u64),
                &kind,
                &payload,
            )
            .map_err(|err| EventLogError::Damaged {
                detail: format!("offset {raw_offset}: {err}"),
            })?;

            entries.push(LogEntry {
                offset: EventLogOffset::new(u64::try_from(raw_offset).unwrap_or(0)),
                event: decoded,
            });
        }

        Ok(entries)
    }

    fn last_offset(&self, _deadline: Deadline) -> Result<Option<EventLogOffset>, EventLogError> {
        let mut client = self
            .client
            .lock()
            .expect("the log connection is not poisoned");

        let row = client
            .query_opt("SELECT MAX(\"offset\") FROM event_log", &[])
            .map_err(|err| EventLogError::ReadFailed {
                detail: format!("reading the last offset failed: {err}"),
            })?;

        let offset: Option<Option<i64>> = row
            .map(|r| {
                r.try_get::<_, Option<i64>>(0)
                    .map_err(|err| EventLogError::ReadFailed {
                        detail: format!("reading the last offset value failed: {err}"),
                    })
            })
            .transpose()?;

        Ok(offset
            .flatten()
            .map(|value| EventLogOffset::new(u64::try_from(value).unwrap_or(0))))
    }
}
