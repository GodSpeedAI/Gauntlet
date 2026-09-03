//! The `CapabilityLedger` adapter (plan Task 22; spec §19.1) — the
//! developmental-memory observation log and the capability register's
//! transition history in the same embedded database as the run state.
//!
//! Cross-run by design (charter D7): the ledger deliberately outlives
//! runs — it is the substrate promotion evidence accumulates in across the
//! runs a per-run `StateStore` names no enumeration of.
//!
//! - [`SqliteCapabilityLedger`] — INSERT-only over migration 0009's two
//!   tables. Observation appends are idempotent through the
//!   `(capability, class, run_id, fact)` UNIQUE key: a same-identity
//!   re-append whose facts agree is a no-op; a disagreeing re-append is
//!   the typed `ObservationConflict` with the stored row standing. The
//!   register's current status is DERIVED from the latest transition's
//!   target — there is no separately mutable status column anywhere, so a
//!   bare set-status shape has no substrate to write through.
//! - Stored payloads are the typed records' canonical JSON
//!   ([`gauntlet_ports::capability_ledger`] codecs); a stored payload that
//!   does not decode is a typed `Corrupt` error, never a degraded record.
//!
//! Port methods carry a `Deadline` (spec §6 preamble); like the state
//! store, this adapter runs single bounded statements with no retry loop
//! and no clock of its own, so the bound is noted and not consulted.

use std::path::Path;
use std::sync::Mutex;

use gauntlet_domain::{CapabilityId, CapabilityStatus, EpochSeconds};
use gauntlet_ports::capability_ledger::{
    CapabilityEntry, CapabilityLedger, CapabilityLedgerError, ObservationAppend, ObservationRecord,
    StatusTransition, TransitionReason, promotion_witness_is_rooted,
};
use gauntlet_ports::deadline::Deadline;
use rusqlite::Connection;

use crate::event_log::open_connection;

/// The capability ledger over the embedded database (spec §19.1).
pub struct SqliteCapabilityLedger {
    conn: Mutex<Connection>,
}

impl SqliteCapabilityLedger {
    /// Opens (creating if needed) the ledger at `path`, applying any
    /// migrations not yet recorded.
    pub fn open(path: &Path) -> Result<Self, CapabilityLedgerError> {
        let conn = open_connection(path)
            .map_err(|detail| CapabilityLedgerError::AppendFailed { detail })?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// An in-memory ledger, for tests that never need a second process.
    pub fn open_in_memory() -> Result<Self, CapabilityLedgerError> {
        let conn = open_connection(Path::new(":memory:"))
            .map_err(|detail| CapabilityLedgerError::AppendFailed { detail })?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// The capability's derived register entry: registered_at from the
    /// first observation, the current status from the latest transition.
    fn entry_of(conn: &Connection, capability: &CapabilityId) -> Query<Option<CapabilityEntry>> {
        let first: Option<i64> = match conn.query_row(
            "SELECT at FROM capability_observations WHERE capability = ?1 \
             ORDER BY seq ASC LIMIT 1",
            [capability.as_str()],
            |row| row.get(0),
        ) {
            Ok(at) => Some(at),
            Err(rusqlite::Error::QueryReturnedNoRows) => None,
            Err(err) => {
                return Err(read_failed(
                    "the first observation instant",
                    capability,
                    err,
                ));
            }
        };
        let Some(registered_at) = first else {
            return Ok(None);
        };
        let latest: Option<String> = match conn.query_row(
            "SELECT payload FROM capability_transitions WHERE capability = ?1 \
             ORDER BY seq DESC LIMIT 1",
            [capability.as_str()],
            |row| row.get(0),
        ) {
            Ok(payload) => Some(payload),
            Err(rusqlite::Error::QueryReturnedNoRows) => None,
            Err(err) => {
                return Err(read_failed("the latest transition", capability, err));
            }
        };
        let (status, last_transition_at) = match latest {
            Some(payload) => {
                let transition = StatusTransition::decode(&payload).map_err(|error| {
                    CapabilityLedgerError::Corrupt {
                        detail: format!(
                            "the latest transition of capability {capability} does not decode: \
                             {error}"
                        ),
                    }
                })?;
                (transition.to(), Some(transition.at()))
            }
            // Observations without any transition: the register's
            // entry state — Observed, never evaluated.
            None => (CapabilityStatus::Observed, None),
        };
        Ok(Some(CapabilityEntry {
            capability: capability.clone(),
            status,
            registered_at: EpochSeconds::new(u64::try_from(registered_at).map_err(|_| {
                CapabilityLedgerError::Corrupt {
                    detail: "a negative observation instant is tamper evidence".to_owned(),
                }
            })?),
            last_transition_at,
        }))
    }
}

type Query<T> = Result<T, CapabilityLedgerError>;

/// The capability's observation log, decoded in append order — the log a
/// promotion's witnesses must resolve against (COR-1 C3).
fn observation_log(conn: &Connection, capability: &CapabilityId) -> Query<Vec<ObservationRecord>> {
    let mut statement = conn
        .prepare(
            "SELECT payload FROM capability_observations WHERE capability = ?1 \
             ORDER BY seq ASC",
        )
        .map_err(|err| read_failed("the observation log", capability, err))?;
    let rows = statement
        .query_map([capability.as_str()], |row| row.get::<_, String>(0))
        .map_err(|err| read_failed("the observation log", capability, err))?;
    let mut observations = Vec::new();
    for row in rows {
        let payload = row.map_err(|err| read_failed("an observation payload", capability, err))?;
        observations.push(ObservationRecord::decode(&payload).map_err(|error| {
            CapabilityLedgerError::Corrupt {
                detail: format!(
                    "a stored observation of capability {capability} does not decode: {error}"
                ),
            }
        })?);
    }
    Ok(observations)
}

fn read_failed(
    what: &str,
    capability: &CapabilityId,
    err: rusqlite::Error,
) -> CapabilityLedgerError {
    CapabilityLedgerError::ReadFailed {
        detail: format!("reading {what} of capability {capability} failed: {err}"),
    }
}

impl CapabilityLedger for SqliteCapabilityLedger {
    /// Appends one observation. Ungated (spec §19.1: "Raw. Immediate,
    /// append-only"); idempotent through the identity UNIQUE key.
    fn append_observation(
        &self,
        observation: ObservationRecord,
        _deadline: Deadline,
    ) -> Result<ObservationAppend, CapabilityLedgerError> {
        let identity = observation.identity().clone();
        let payload = observation.to_json();
        let conn = self
            .conn
            .lock()
            .expect("the ledger connection is not poisoned");
        let existing: Option<(i64, String)> = match conn.query_row(
            "SELECT seq, payload FROM capability_observations \
             WHERE capability = ?1 AND class = ?2 AND run_id = ?3 AND fact = ?4",
            (
                identity.capability().as_str(),
                identity.class().to_string(),
                identity.run().as_str(),
                identity.fact(),
            ),
            |row| Ok((row.get(0)?, row.get(1)?)),
        ) {
            Ok(found) => Some(found),
            Err(rusqlite::Error::QueryReturnedNoRows) => None,
            Err(err) => {
                return Err(read_failed(
                    "the stored observation",
                    identity.capability(),
                    err,
                ));
            }
        };
        if let Some((seq, stored_payload)) = existing {
            let stored = ObservationRecord::decode(&stored_payload)?;
            if stored.facts_agree(&observation) {
                return Ok(ObservationAppend {
                    sequence: u64::try_from(seq).map_err(|_| CapabilityLedgerError::Corrupt {
                        detail: "a negative observation sequence is tamper evidence".to_owned(),
                    })?,
                    idempotent: true,
                });
            }
            return Err(CapabilityLedgerError::ObservationConflict { identity });
        }
        let insert = conn.execute(
            "INSERT INTO capability_observations \
             (capability, class, run_id, fact, payload, at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            (
                identity.capability().as_str(),
                identity.class().to_string(),
                identity.run().as_str(),
                identity.fact(),
                payload,
                observation.at().get(),
            ),
        );
        match insert {
            Ok(_) => {}
            Err(err) => {
                return Err(CapabilityLedgerError::AppendFailed {
                    detail: format!(
                        "appending a {} observation of run {} for capability {} failed: {err} — \
                         nothing was recorded",
                        identity.class(),
                        identity.run(),
                        identity.capability()
                    ),
                });
            }
        }
        let sequence = conn.last_insert_rowid();
        Ok(ObservationAppend {
            sequence: u64::try_from(sequence).map_err(|_| CapabilityLedgerError::Corrupt {
                detail: "a negative observation sequence is tamper evidence".to_owned(),
            })?,
            idempotent: false,
        })
    }

    fn observations(
        &self,
        capability: &CapabilityId,
        _deadline: Deadline,
    ) -> Result<Vec<ObservationRecord>, CapabilityLedgerError> {
        let conn = self
            .conn
            .lock()
            .expect("the ledger connection is not poisoned");
        let mut statement = conn
            .prepare(
                "SELECT payload FROM capability_observations WHERE capability = ?1 \
                 ORDER BY seq ASC",
            )
            .map_err(|err| read_failed("the observation log", capability, err))?;
        let rows = statement
            .query_map([capability.as_str()], |row| row.get::<_, String>(0))
            .map_err(|err| read_failed("the observation log", capability, err))?;
        let mut observations = Vec::new();
        for row in rows {
            let payload =
                row.map_err(|err| read_failed("an observation payload", capability, err))?;
            observations.push(ObservationRecord::decode(&payload).map_err(|error| {
                CapabilityLedgerError::Corrupt {
                    detail: format!(
                        "a stored observation of capability {capability} does not decode: {error}"
                    ),
                }
            })?);
        }
        Ok(observations)
    }

    fn capability(
        &self,
        capability: &CapabilityId,
        _deadline: Deadline,
    ) -> Result<Option<CapabilityEntry>, CapabilityLedgerError> {
        let conn = self
            .conn
            .lock()
            .expect("the ledger connection is not poisoned");
        Self::entry_of(&conn, capability)
    }

    /// Applies one status transition — the register's ONLY status writer.
    /// One transaction: the legality and staleness checks read the same
    /// snapshot the history insert lands in.
    fn apply_transition(
        &self,
        transition: StatusTransition,
        _deadline: Deadline,
    ) -> Result<CapabilityEntry, CapabilityLedgerError> {
        let capability = transition.capability().clone();
        let payload = transition.to_json();
        let conn = self
            .conn
            .lock()
            .expect("the ledger connection is not poisoned");
        conn.execute_batch("BEGIN IMMEDIATE;").map_err(|err| {
            CapabilityLedgerError::AppendFailed {
                detail: format!("beginning the transition transaction failed: {err}"),
            }
        })?;
        let outcome = (|| {
            let Some(entry) = Self::entry_of(&conn, &capability)? else {
                return Err(CapabilityLedgerError::UnknownCapability {
                    capability: capability.clone(),
                });
            };
            if transition.from() != entry.status {
                return Err(CapabilityLedgerError::StaleTransition {
                    capability: capability.clone(),
                    current: entry.status,
                });
            }
            // Log-rooting (COR-1 C3): an elevating transition is an earned
            // write — every witness in its evidence must resolve, in THIS
            // ledger's own observation log, to a Settlement observation of
            // the same capability with matching (run, unit, verification,
            // evidence) facts. Pure provenance, not §19.2 logic. Checked
            // inside the transaction so the log the witnesses resolve
            // against is the same snapshot the history insert lands in.
            if let TransitionReason::Promoted(evidence) = transition.reason() {
                let log = observation_log(&conn, &capability)?;
                for witness in &evidence.settlements {
                    if !promotion_witness_is_rooted(&log, &capability, witness) {
                        return Err(CapabilityLedgerError::UnrootedPromotion {
                            capability: capability.clone(),
                            witness: witness.clone(),
                        });
                    }
                }
            }
            conn.execute(
                "INSERT INTO capability_transitions (capability, payload) VALUES (?1, ?2)",
                (capability.as_str(), payload),
            )
            .map_err(|err| CapabilityLedgerError::AppendFailed {
                detail: format!(
                    "applying a transition for capability {capability} failed: {err} — \
                     nothing was recorded"
                ),
            })?;
            Self::entry_of(&conn, &capability)?.ok_or_else(|| CapabilityLedgerError::ReadFailed {
                detail: format!("the register entry of {capability} vanished mid-transition"),
            })
        })();
        match outcome {
            Ok(entry) => {
                conn.execute_batch("COMMIT;").map_err(|err| {
                    CapabilityLedgerError::AppendFailed {
                        detail: format!("committing the transition failed: {err}"),
                    }
                })?;
                Ok(entry)
            }
            Err(error) => {
                conn.execute_batch("ROLLBACK;").map_err(|rollback| {
                    CapabilityLedgerError::AppendFailed {
                        detail: format!("{error}; rolling back failed too: {rollback}"),
                    }
                })?;
                Err(error)
            }
        }
    }

    fn transitions(
        &self,
        capability: &CapabilityId,
        _deadline: Deadline,
    ) -> Result<Vec<StatusTransition>, CapabilityLedgerError> {
        let conn = self
            .conn
            .lock()
            .expect("the ledger connection is not poisoned");
        let mut statement = conn
            .prepare(
                "SELECT payload FROM capability_transitions WHERE capability = ?1 \
                 ORDER BY seq ASC",
            )
            .map_err(|err| read_failed("the transition history", capability, err))?;
        let rows = statement
            .query_map([capability.as_str()], |row| row.get::<_, String>(0))
            .map_err(|err| read_failed("the transition history", capability, err))?;
        let mut history = Vec::new();
        for row in rows {
            let payload =
                row.map_err(|err| read_failed("a transition payload", capability, err))?;
            history.push(StatusTransition::decode(&payload).map_err(|error| {
                CapabilityLedgerError::Corrupt {
                    detail: format!(
                        "a stored transition of capability {capability} does not decode: {error}"
                    ),
                }
            })?);
        }
        Ok(history)
    }
}
