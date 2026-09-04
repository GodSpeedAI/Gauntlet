//! The `RevisionLedger` adapter (plan Task 23; spec §19.3/§19.4) — the
//! durable candidate-revision store in the same embedded database as the
//! run state and the capability ledger.
//!
//! Cross-run by design (charter D3): revisions are evaluated and
//! adjudicated across runs, so the ledger deliberately outlives them.
//!
//! - [`SqliteRevisionLedger`] — INSERT-only over migration 0010's four
//!   tables. Proposal admissions are idempotent through the `revision`
//!   UNIQUE key (a same-id re-admission whose facts agree is a no-op; a
//!   disagreeing one is the typed `ProposalConflict` with the stored row
//!   standing); evaluation appends are idempotent on the four-run
//!   identity. The current decision is DERIVED from the latest decision
//!   record's verdict — there is no separately mutable decision column
//!   anywhere — and the rolled-back fact is the EXISTENCE of a restore
//!   row, so a promoted-and-restored revision reads
//!   promoted-but-rolled-back, never silently active.
//! - Stored payloads are the typed records' canonical JSON
//!   ([`gauntlet_ports::revision_ledger`] codecs); a stored payload that
//!   does not decode is a typed `Corrupt` error, never a degraded record.
//!
//! Port methods carry a `Deadline` (spec §6 preamble); like the state
//! store, this adapter runs single bounded statements with no retry loop
//! and no clock of its own, so the bound is noted and not consulted.

use std::path::Path;
use std::sync::Mutex;

use gauntlet_domain::RevisionId;
use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::revision_ledger::{
    DecisionKind, EvaluationAppend, EvaluationRecord, ProposalAdmission, RestoreRecord,
    RevisionDecisionRecord, RevisionEntry, RevisionLedger, RevisionLedgerError, RevisionProposal,
};
use rusqlite::Connection;

use crate::event_log::open_connection;

/// The revision ledger over the embedded database (spec §19.3/§19.4).
pub struct SqliteRevisionLedger {
    conn: Mutex<Connection>,
}

impl SqliteRevisionLedger {
    /// Opens (creating if needed) the ledger at `path`, applying any
    /// migrations not yet recorded.
    pub fn open(path: &Path) -> Result<Self, RevisionLedgerError> {
        let conn =
            open_connection(path).map_err(|detail| RevisionLedgerError::AppendFailed { detail })?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// An in-memory ledger, for tests that never need a second process.
    pub fn open_in_memory() -> Result<Self, RevisionLedgerError> {
        let conn = open_connection(Path::new(":memory:"))
            .map_err(|detail| RevisionLedgerError::AppendFailed { detail })?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    fn require_known(&self, conn: &Connection, revision: &RevisionId) -> Query<()> {
        let known: i64 = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM revision_proposals WHERE revision = ?1)",
                [revision.as_str()],
                |row| row.get(0),
            )
            .map_err(|err| read_failed("the stored proposal", revision, err))?;
        if known == 0 {
            return Err(RevisionLedgerError::UnknownRevision {
                revision: revision.clone(),
            });
        }
        Ok(())
    }

    /// The revision's derived entry: the decision from the LATEST decision
    /// record's verdict, the rolled-back fact from a restore row's
    /// existence — both derived, neither separately mutable.
    fn entry_of(conn: &Connection, revision: &RevisionId) -> Query<Option<RevisionEntry>> {
        let payload: Option<String> = match conn.query_row(
            "SELECT payload FROM revision_proposals WHERE revision = ?1",
            [revision.as_str()],
            |row| row.get::<_, String>(0),
        ) {
            Ok(found) => Some(found),
            Err(rusqlite::Error::QueryReturnedNoRows) => None,
            Err(err) => {
                return Err(read_failed("the stored proposal", revision, err));
            }
        };
        let Some(payload) = payload else {
            return Ok(None);
        };
        let proposal =
            RevisionProposal::decode(&payload).map_err(|error| RevisionLedgerError::Corrupt {
                detail: format!(
                    "the stored proposal of revision {revision} does not decode: {error}"
                ),
            })?;
        let admitted_at = i64::try_from(proposal.admitted_at().get()).map_err(|_| {
            RevisionLedgerError::Corrupt {
                detail: "a negative admission instant is tamper evidence".to_owned(),
            }
        })?;
        let latest: Option<String> = match conn.query_row(
            "SELECT payload FROM revision_decisions WHERE revision = ?1 \
             ORDER BY seq DESC LIMIT 1",
            [revision.as_str()],
            |row| row.get(0),
        ) {
            Ok(payload) => Some(payload),
            Err(rusqlite::Error::QueryReturnedNoRows) => None,
            Err(err) => {
                return Err(read_failed("the latest decision record", revision, err));
            }
        };
        let (decision, last_decision_at) = match latest {
            Some(payload) => {
                let record = RevisionDecisionRecord::decode(&payload).map_err(|error| {
                    RevisionLedgerError::Corrupt {
                        detail: format!(
                            "the latest decision of revision {revision} does not decode: \
                                 {error}"
                        ),
                    }
                })?;
                (Some(record.writes()), Some(record.at()))
            }
            // A proposal without any decision: the ledger's floor state —
            // pending, never adjudicated.
            None => (None, None),
        };
        let evaluations = conn
            .query_row(
                "SELECT COUNT(*) FROM revision_evaluations WHERE revision = ?1",
                [revision.as_str()],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|err| read_failed("the evaluation count", revision, err))?;
        let rolled_back = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM revision_restores WHERE revision = ?1)",
                [revision.as_str()],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|err| read_failed("the restore history", revision, err))?
            != 0;
        Ok(Some(RevisionEntry {
            revision: revision.clone(),
            decision,
            rolled_back,
            evaluations: usize::try_from(evaluations).map_err(|_| {
                RevisionLedgerError::Corrupt {
                    detail: "a negative evaluation count is tamper evidence".to_owned(),
                }
            })?,
            admitted_at: gauntlet_domain::EpochSeconds::new(u64::try_from(admitted_at).map_err(
                |_| RevisionLedgerError::Corrupt {
                    detail: "a negative admission instant is tamper evidence".to_owned(),
                },
            )?),
            last_decision_at,
        }))
    }

    /// The shared pre-write checks of a decision or restore record: the
    /// revision must be known and the record's claimed state must agree
    /// with the derived state. Called inside the write's transaction so
    /// the derived state the claim is checked against is the same snapshot
    /// the record lands in.
    fn validate_claim(
        conn: &Connection,
        revision: &RevisionId,
        claimed: DecisionKind,
        rolled_back_ok: bool,
    ) -> Result<(), RevisionLedgerError> {
        let Some(entry) = Self::entry_of(conn, revision)? else {
            return Err(RevisionLedgerError::UnknownRevision {
                revision: revision.clone(),
            });
        };
        let current = entry.decision.unwrap_or(DecisionKind::Pending);
        if rolled_back_ok {
            // Restore records: the claim must be the promoted, unrestored
            // state itself.
            if claimed != gauntlet_ports::revision_ledger::DecisionKind::Promoted
                || entry.rolled_back
                || current != gauntlet_ports::revision_ledger::DecisionKind::Promoted
            {
                return Err(RevisionLedgerError::StaleRestore {
                    revision: revision.clone(),
                    current,
                    rolled_back: entry.rolled_back,
                });
            }
        } else if claimed != current {
            return Err(RevisionLedgerError::StaleDecision {
                revision: revision.clone(),
                current,
            });
        }
        Ok(())
    }
}

type Query<T> = Result<T, RevisionLedgerError>;

fn read_failed(what: &str, revision: &RevisionId, err: rusqlite::Error) -> RevisionLedgerError {
    RevisionLedgerError::ReadFailed {
        detail: format!("reading {what} of revision {revision} failed: {err}"),
    }
}

impl RevisionLedger for SqliteRevisionLedger {
    /// Admits one proposal. Idempotent through the revision UNIQUE key.
    fn admit_proposal(
        &self,
        proposal: RevisionProposal,
        _deadline: Deadline,
    ) -> Result<ProposalAdmission, RevisionLedgerError> {
        let revision = proposal.revision().clone();
        let payload = proposal.to_json();
        let conn = self
            .conn
            .lock()
            .expect("the ledger connection is not poisoned");
        let existing: Option<String> = match conn.query_row(
            "SELECT payload FROM revision_proposals WHERE revision = ?1",
            [revision.as_str()],
            |row| row.get(0),
        ) {
            Ok(found) => Some(found),
            Err(rusqlite::Error::QueryReturnedNoRows) => None,
            Err(err) => {
                return Err(read_failed("the stored proposal", &revision, err));
            }
        };
        if let Some(stored_payload) = existing {
            let stored = RevisionProposal::decode(&stored_payload)?;
            if stored.facts_agree(&proposal) {
                return Ok(ProposalAdmission { idempotent: true });
            }
            return Err(RevisionLedgerError::ProposalConflict { revision });
        }
        conn.execute(
            "INSERT INTO revision_proposals (revision, payload) VALUES (?1, ?2)",
            (revision.as_str(), payload),
        )
        .map_err(|err| RevisionLedgerError::AppendFailed {
            detail: format!("admitting revision {revision} failed: {err} — nothing was recorded"),
        })?;
        Ok(ProposalAdmission { idempotent: false })
    }

    fn proposal(
        &self,
        revision: &RevisionId,
        _deadline: Deadline,
    ) -> Result<Option<RevisionProposal>, RevisionLedgerError> {
        let conn = self
            .conn
            .lock()
            .expect("the ledger connection is not poisoned");
        let payload: Option<String> = match conn.query_row(
            "SELECT payload FROM revision_proposals WHERE revision = ?1",
            [revision.as_str()],
            |row| row.get(0),
        ) {
            Ok(found) => Some(found),
            Err(rusqlite::Error::QueryReturnedNoRows) => None,
            Err(err) => {
                return Err(read_failed("the stored proposal", revision, err));
            }
        };
        payload
            .map(|stored| {
                RevisionProposal::decode(&stored).map_err(|error| RevisionLedgerError::Corrupt {
                    detail: format!(
                        "the stored proposal of revision {revision} does not decode: {error}"
                    ),
                })
            })
            .transpose()
    }

    /// Appends one evaluation record. Idempotent on the four-run identity.
    fn append_evaluation(
        &self,
        record: EvaluationRecord,
        _deadline: Deadline,
    ) -> Result<EvaluationAppend, RevisionLedgerError> {
        let revision = record.revision.clone();
        let identity = record.identity();
        let payload = record.to_json();
        let conn = self
            .conn
            .lock()
            .expect("the ledger connection is not poisoned");
        self.require_known(&conn, &revision)?;
        let existing: Option<String> = match conn.query_row(
            "SELECT payload FROM revision_evaluations WHERE revision = ?1 \
             AND held_out_control = ?2 AND held_out_treatment = ?3 \
             AND regression_control = ?4 AND regression_treatment = ?5",
            (
                revision.as_str(),
                identity.0.as_str(),
                identity.1.as_str(),
                identity.2.as_str(),
                identity.3.as_str(),
            ),
            |row| row.get(0),
        ) {
            Ok(found) => Some(found),
            Err(rusqlite::Error::QueryReturnedNoRows) => None,
            Err(err) => {
                return Err(read_failed("the stored evaluation", &revision, err));
            }
        };
        let sequence: i64 = conn
            .query_row(
                "SELECT COALESCE(MAX(seq), 0) FROM revision_evaluations",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|err| RevisionLedgerError::ReadFailed {
                detail: format!("reading the evaluation sequence failed: {err}"),
            })?;
        let sequence = u64::try_from(sequence).map_err(|_| RevisionLedgerError::Corrupt {
            detail: "a negative evaluation sequence is tamper evidence".to_owned(),
        })?;
        if let Some(stored_payload) = existing {
            let stored = EvaluationRecord::decode(&stored_payload)?;
            if stored.facts_agree(&record) {
                return Ok(EvaluationAppend {
                    sequence,
                    idempotent: true,
                });
            }
            return Err(RevisionLedgerError::EvaluationConflict { revision });
        }
        conn.execute(
            "INSERT INTO revision_evaluations (revision, held_out_control, held_out_treatment, \
             regression_control, regression_treatment, payload) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            (
                revision.as_str(),
                identity.0.as_str(),
                identity.1.as_str(),
                identity.2.as_str(),
                identity.3.as_str(),
                payload,
            ),
        )
        .map_err(|err| RevisionLedgerError::AppendFailed {
            detail: format!(
                "appending an evaluation of {revision} failed: {err} — nothing was \
                             recorded"
            ),
        })?;
        Ok(EvaluationAppend {
            sequence,
            idempotent: false,
        })
    }

    fn evaluations(
        &self,
        revision: &RevisionId,
        _deadline: Deadline,
    ) -> Result<Vec<EvaluationRecord>, RevisionLedgerError> {
        let conn = self
            .conn
            .lock()
            .expect("the ledger connection is not poisoned");
        let mut statement = conn
            .prepare(
                "SELECT payload FROM revision_evaluations WHERE revision = ?1 ORDER BY seq ASC",
            )
            .map_err(|err| read_failed("the evaluation records", revision, err))?;
        let rows = statement
            .query_map([revision.as_str()], |row| row.get::<_, String>(0))
            .map_err(|err| read_failed("the evaluation records", revision, err))?;
        let mut records = Vec::new();
        for row in rows {
            let payload = row.map_err(|err| read_failed("an evaluation payload", revision, err))?;
            records.push(EvaluationRecord::decode(&payload).map_err(|error| {
                RevisionLedgerError::Corrupt {
                    detail: format!(
                        "a stored evaluation of revision {revision} does not decode: {error}"
                    ),
                }
            })?);
        }
        Ok(records)
    }

    /// Applies one decision record — the ledger's ONLY decision writer.
    /// One transaction: the staleness check reads the same snapshot the
    /// history insert lands in.
    fn apply_decision(
        &self,
        record: RevisionDecisionRecord,
        _deadline: Deadline,
    ) -> Result<RevisionEntry, RevisionLedgerError> {
        let revision = record.revision().clone();
        let payload = record.to_json();
        let conn = self
            .conn
            .lock()
            .expect("the ledger connection is not poisoned");
        conn.execute_batch("BEGIN IMMEDIATE;").map_err(|err| {
            RevisionLedgerError::AppendFailed {
                detail: format!("beginning the decision transaction failed: {err}"),
            }
        })?;
        let outcome = (|| {
            Self::validate_claim(&conn, &revision, record.claimed(), false)?;
            conn.execute(
                "INSERT INTO revision_decisions (revision, payload) VALUES (?1, ?2)",
                (revision.as_str(), payload),
            )
            .map_err(|err| RevisionLedgerError::AppendFailed {
                detail: format!(
                    "applying a decision for revision {revision} failed: {err} — nothing was \
                     recorded"
                ),
            })?;
            Self::entry_of(&conn, &revision)?.ok_or_else(|| RevisionLedgerError::ReadFailed {
                detail: format!("the derived entry of {revision} vanished mid-decision"),
            })
        })();
        match outcome {
            Ok(entry) => {
                conn.execute_batch("COMMIT;")
                    .map_err(|err| RevisionLedgerError::AppendFailed {
                        detail: format!("committing the decision failed: {err}"),
                    })?;
                Ok(entry)
            }
            Err(error) => {
                conn.execute_batch("ROLLBACK;").map_err(|rollback| {
                    RevisionLedgerError::AppendFailed {
                        detail: format!("{error}; rolling back failed too: {rollback}"),
                    }
                })?;
                Err(error)
            }
        }
    }

    fn decisions(
        &self,
        revision: &RevisionId,
        _deadline: Deadline,
    ) -> Result<Vec<RevisionDecisionRecord>, RevisionLedgerError> {
        let conn = self
            .conn
            .lock()
            .expect("the ledger connection is not poisoned");
        let mut statement = conn
            .prepare("SELECT payload FROM revision_decisions WHERE revision = ?1 ORDER BY seq ASC")
            .map_err(|err| read_failed("the decision history", revision, err))?;
        let rows = statement
            .query_map([revision.as_str()], |row| row.get::<_, String>(0))
            .map_err(|err| read_failed("the decision history", revision, err))?;
        let mut history = Vec::new();
        for row in rows {
            let payload = row.map_err(|err| read_failed("a decision payload", revision, err))?;
            history.push(RevisionDecisionRecord::decode(&payload).map_err(|error| {
                RevisionLedgerError::Corrupt {
                    detail: format!(
                        "a stored decision of revision {revision} does not decode: {error}"
                    ),
                }
            })?);
        }
        Ok(history)
    }

    /// Records one restore transition (append-only). One transaction, the
    /// decision shape: the promoted-and-unrestored claim is validated
    /// against the same snapshot the row lands in.
    fn record_restore(
        &self,
        record: RestoreRecord,
        _deadline: Deadline,
    ) -> Result<RevisionEntry, RevisionLedgerError> {
        let revision = record.revision().clone();
        let payload = record.to_json();
        let conn = self
            .conn
            .lock()
            .expect("the ledger connection is not poisoned");
        conn.execute_batch("BEGIN IMMEDIATE;").map_err(|err| {
            RevisionLedgerError::AppendFailed {
                detail: format!("beginning the restore transaction failed: {err}"),
            }
        })?;
        let outcome = (|| {
            Self::validate_claim(&conn, &revision, record.claimed(), true)?;
            conn.execute(
                "INSERT INTO revision_restores (revision, payload) VALUES (?1, ?2)",
                (revision.as_str(), payload),
            )
            .map_err(|err| RevisionLedgerError::AppendFailed {
                detail: format!(
                    "recording the restore of revision {revision} failed: {err} — nothing was \
                     recorded"
                ),
            })?;
            Self::entry_of(&conn, &revision)?.ok_or_else(|| RevisionLedgerError::ReadFailed {
                detail: format!("the derived entry of {revision} vanished mid-restore"),
            })
        })();
        match outcome {
            Ok(entry) => {
                conn.execute_batch("COMMIT;")
                    .map_err(|err| RevisionLedgerError::AppendFailed {
                        detail: format!("committing the restore failed: {err}"),
                    })?;
                Ok(entry)
            }
            Err(error) => {
                conn.execute_batch("ROLLBACK;").map_err(|rollback| {
                    RevisionLedgerError::AppendFailed {
                        detail: format!("{error}; rolling back failed too: {rollback}"),
                    }
                })?;
                Err(error)
            }
        }
    }

    fn restores(
        &self,
        revision: &RevisionId,
        _deadline: Deadline,
    ) -> Result<Vec<RestoreRecord>, RevisionLedgerError> {
        let conn = self
            .conn
            .lock()
            .expect("the ledger connection is not poisoned");
        let mut statement = conn
            .prepare("SELECT payload FROM revision_restores WHERE revision = ?1 ORDER BY seq ASC")
            .map_err(|err| read_failed("the restore history", revision, err))?;
        let rows = statement
            .query_map([revision.as_str()], |row| row.get::<_, String>(0))
            .map_err(|err| read_failed("the restore history", revision, err))?;
        let mut history = Vec::new();
        for row in rows {
            let payload = row.map_err(|err| read_failed("a restore payload", revision, err))?;
            history.push(RestoreRecord::decode(&payload).map_err(|error| {
                RevisionLedgerError::Corrupt {
                    detail: format!(
                        "a stored restore of revision {revision} does not decode: {error}"
                    ),
                }
            })?);
        }
        Ok(history)
    }

    fn revision(
        &self,
        revision: &RevisionId,
        _deadline: Deadline,
    ) -> Result<Option<RevisionEntry>, RevisionLedgerError> {
        let conn = self
            .conn
            .lock()
            .expect("the ledger connection is not poisoned");
        Self::entry_of(&conn, revision)
    }
}
