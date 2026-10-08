//! The `CapabilityLedger` adapter over PostgreSQL (spec §19.1) — the
//! developmental-memory observation log and the capability register's
//! transition history in the same embedded database as the run state.
//!
//! Cross-run by design (charter D7): the ledger deliberately outlives
//! runs — it is the substrate promotion evidence accumulates in across the
//! runs a per-run `StateStore` names no enumeration of.
//!
//! - [`PostgresCapabilityLedger`] — INSERT-only over migration 0009's two
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

use std::sync::Mutex;

use gauntlet_domain::{CapabilityId, CapabilityStatus, EpochSeconds};
use gauntlet_ports::capability_ledger::{
    CapabilityEntry, CapabilityLedger, CapabilityLedgerError, ObservationAppend, ObservationRecord,
    StatusTransition, TransitionReason, promotion_witness_is_rooted,
};
use gauntlet_ports::deadline::Deadline;
use postgres::Client;

use crate::db::PostgresConfig;
use crate::migrations;

/// The capability ledger over PostgreSQL (spec §19.1).
pub struct PostgresCapabilityLedger {
    client: Mutex<Client>,
}

impl PostgresCapabilityLedger {
    /// Opens (creating if needed) the ledger by connecting and applying any
    /// migrations not yet recorded.
    pub fn open(config: &PostgresConfig) -> Result<Self, CapabilityLedgerError> {
        let mut client = crate::db::connect(config)
            .map_err(|detail| CapabilityLedgerError::AppendFailed { detail })?;
        migrations::apply(&mut client).map_err(|err| CapabilityLedgerError::AppendFailed {
            detail: err.to_string(),
        })?;
        Ok(Self {
            client: Mutex::new(client),
        })
    }

    /// The capability's derived register entry: registered_at from the
    /// first observation, the current status from the latest transition.
    fn entry_of(
        client: &mut Client,
        capability: &CapabilityId,
    ) -> Result<Option<CapabilityEntry>, CapabilityLedgerError> {
        // Get the first observation's timestamp (registered_at)
        let first_row = client
            .query_opt(
                "SELECT at FROM capability_observations WHERE capability = $1 \
                 ORDER BY seq ASC LIMIT 1",
                &[&capability.as_str()],
            )
            .map_err(|err| read_failed("the first observation instant", capability, err))?;

        let Some(first) = first_row else {
            return Ok(None);
        };

        let registered_at: i64 = first.get(0);

        // Get the latest transition's payload
        let latest_row = client
            .query_opt(
                "SELECT payload FROM capability_transitions WHERE capability = $1 \
                 ORDER BY seq DESC LIMIT 1",
                &[&capability.as_str()],
            )
            .map_err(|err| read_failed("the latest transition", capability, err))?;

        let (status, last_transition_at) = match latest_row {
            Some(row) => {
                let payload: String = row.get(0);
                let transition = StatusTransition::decode(&payload).map_err(|error| {
                    CapabilityLedgerError::Corrupt {
                        detail: format!(
                            "the latest transition of capability {capability} does not decode: {error}"
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

fn read_failed(
    what: &str,
    capability: &CapabilityId,
    err: postgres::Error,
) -> CapabilityLedgerError {
    CapabilityLedgerError::ReadFailed {
        detail: format!("reading {what} of capability {capability} failed: {err}"),
    }
}

impl CapabilityLedger for PostgresCapabilityLedger {
    /// Appends one observation. Ungated (spec §19.1: "Raw. Immediate,
    /// append-only"); idempotent through the identity UNIQUE key.
    fn append_observation(
        &self,
        observation: ObservationRecord,
        _deadline: Deadline,
    ) -> Result<ObservationAppend, CapabilityLedgerError> {
        let identity = observation.identity().clone();
        let payload = observation.to_json();

        let mut client = self
            .client
            .lock()
            .expect("the ledger connection is not poisoned");

        // Check if the observation already exists with the same identity
        let existing_row = client
            .query_opt(
                "SELECT seq, payload FROM capability_observations \
                 WHERE capability = $1 AND class = $2 AND run_id = $3 AND fact = $4",
                &[
                    &identity.capability().as_str(),
                    &identity.class().to_string(),
                    &identity.run().as_str(),
                    &identity.fact(),
                ],
            )
            .map_err(|err| read_failed("the stored observation", identity.capability(), err))?;

        if let Some(existing) = existing_row {
            let seq: i64 = existing.get(0);
            let stored_payload: String = existing.get(1);
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

        // Insert the new observation
        let insert_result = client.execute(
            "INSERT INTO capability_observations \
             (capability, class, run_id, fact, payload, at) VALUES ($1, $2, $3, $4, $5, $6)",
            &[
                &identity.capability().as_str(),
                &identity.class().to_string(),
                &identity.run().as_str(),
                &identity.fact(),
                &payload,
                &(observation.at().get() as i64),
            ],
        );

        match insert_result {
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

        // Get the sequence number of the inserted row
        let seq_row = client
            .query_one(
                "SELECT currval(pg_get_serial_sequence('capability_observations', 'seq'))",
                &[],
            )
            .map_err(|err| CapabilityLedgerError::AppendFailed {
                detail: format!("retrieving the inserted observation sequence failed: {err}"),
            })?;

        let sequence: i64 = seq_row.get(0);
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
        let mut client = self
            .client
            .lock()
            .expect("the ledger connection is not poisoned");

        let rows = client
            .query(
                "SELECT payload FROM capability_observations WHERE capability = $1 \
                 ORDER BY seq ASC",
                &[&capability.as_str()],
            )
            .map_err(|err| read_failed("the observation log", capability, err))?;

        let mut observations = Vec::new();
        for row in rows {
            let payload: String = row.get(0);
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
        let mut client = self
            .client
            .lock()
            .expect("the ledger connection is not poisoned");
        Self::entry_of(&mut client, capability)
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

        let mut client = self
            .client
            .lock()
            .expect("the ledger connection is not poisoned");

        let mut transaction =
            client
                .transaction()
                .map_err(|err| CapabilityLedgerError::AppendFailed {
                    detail: format!("beginning the transition transaction failed: {err}"),
                })?;

        // Validate: capability must exist (have observations)
        let Some(entry) = entry_of_from_transaction(&mut transaction, &capability)? else {
            return Err(CapabilityLedgerError::UnknownCapability {
                capability: capability.clone(),
            });
        };

        // Validate: transition's from state must match current status
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
            let log = observation_log_from_transaction(&mut transaction, &capability)?;
            for witness in &evidence.settlements {
                if !promotion_witness_is_rooted(&log, &capability, witness) {
                    return Err(CapabilityLedgerError::UnrootedPromotion {
                        capability: capability.clone(),
                        witness: witness.clone(),
                    });
                }
            }
        }

        // Insert the transition
        transaction
            .execute(
                "INSERT INTO capability_transitions (capability, payload) VALUES ($1, $2)",
                &[&capability.as_str(), &payload],
            )
            .map_err(|err| CapabilityLedgerError::AppendFailed {
                detail: format!(
                    "applying a transition for capability {capability} failed: {err} — \
                     nothing was recorded"
                ),
            })?;

        // Retrieve the updated entry
        let new_entry =
            entry_of_from_transaction(&mut transaction, &capability)?.ok_or_else(|| {
                CapabilityLedgerError::ReadFailed {
                    detail: format!("the register entry of {capability} vanished mid-transition"),
                }
            })?;

        // Commit the transaction
        transaction
            .commit()
            .map_err(|err| CapabilityLedgerError::AppendFailed {
                detail: format!("committing the transition failed: {err}"),
            })?;

        Ok(new_entry)
    }

    fn transitions(
        &self,
        capability: &CapabilityId,
        _deadline: Deadline,
    ) -> Result<Vec<StatusTransition>, CapabilityLedgerError> {
        let mut client = self
            .client
            .lock()
            .expect("the ledger connection is not poisoned");

        let rows = client
            .query(
                "SELECT payload FROM capability_transitions WHERE capability = $1 \
                 ORDER BY seq ASC",
                &[&capability.as_str()],
            )
            .map_err(|err| read_failed("the transition history", capability, err))?;

        let mut history = Vec::new();
        for row in rows {
            let payload: String = row.get(0);
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

/// The capability's derived register entry from a transaction — registered_at from the
/// first observation, the current status from the latest transition.
fn entry_of_from_transaction(
    transaction: &mut postgres::Transaction,
    capability: &CapabilityId,
) -> Result<Option<CapabilityEntry>, CapabilityLedgerError> {
    // Get the first observation's timestamp (registered_at)
    let first_row = transaction
        .query_opt(
            "SELECT at FROM capability_observations WHERE capability = $1 \
             ORDER BY seq ASC LIMIT 1",
            &[&capability.as_str()],
        )
        .map_err(|err| read_failed("the first observation instant", capability, err))?;

    let Some(first) = first_row else {
        return Ok(None);
    };

    let registered_at: i64 = first.get(0);

    // Get the latest transition's payload
    let latest_row = transaction
        .query_opt(
            "SELECT payload FROM capability_transitions WHERE capability = $1 \
             ORDER BY seq DESC LIMIT 1",
            &[&capability.as_str()],
        )
        .map_err(|err| read_failed("the latest transition", capability, err))?;

    let (status, last_transition_at) = match latest_row {
        Some(row) => {
            let payload: String = row.get(0);
            let transition = StatusTransition::decode(&payload).map_err(|error| {
                CapabilityLedgerError::Corrupt {
                    detail: format!(
                        "the latest transition of capability {capability} does not decode: {error}"
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

/// The capability's observation log, decoded in append order — the log a
/// promotion's witnesses must resolve against (COR-1 C3).
fn observation_log_from_transaction(
    transaction: &mut postgres::Transaction,
    capability: &CapabilityId,
) -> Result<Vec<ObservationRecord>, CapabilityLedgerError> {
    let rows = transaction
        .query(
            "SELECT payload FROM capability_observations WHERE capability = $1 \
             ORDER BY seq ASC",
            &[&capability.as_str()],
        )
        .map_err(|err| read_failed("the observation log", capability, err))?;

    let mut observations = Vec::new();
    for row in rows {
        let payload: String = row.get(0);
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
