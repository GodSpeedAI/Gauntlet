//! The `StateStore` adapter over the embedded database (spec §6.2, §18).
//!
//! Everything durable about a run lives here: its identity (`runs`), its
//! §18.2 checkpoints, its §16.3 settlement rows, and the remaining §18.1
//! record kinds (`records`). All writes are `INSERT` — there is no UPDATE or
//! DELETE anywhere in this crate; idempotency comes from keys, never from
//! rewriting history.
//!
//! `load_run` returns the authoritative `runs` row when there is one. A run
//! known only through the port's own writes (events and checkpoints) gets a
//! documented projection: the port's durable records carry the run id and
//! frozen reference digest but not the full `RunDefinition`, so the fields
//! they never carried come back as explicit placeholders. Recording the run
//! ([`SqliteStateStore::record_run`], the composition root's job) is what
//! makes the record authoritative.
//!
//! One recorded port gap: the domain's `Spent` has no constructor that
//! carries values (spend accrues only through `Budget::charge`), so no code
//! outside the domain can build a non-zero `Spent`. Every `Checkpoint` value
//! that can exist today therefore carries zero spend, and `resume` returns
//! exactly that; the six counters stay durable in the database and the
//! export, and a future non-zero value — which only a domain change can
//! produce — is refused loudly rather than silently zeroed.
//!
//! Port methods carry a `Deadline` (spec §6 preamble); like the event log,
//! this adapter runs single bounded statements with no retry loop and no
//! clock of its own, so the bound is noted and not consulted.

use std::path::Path;
use std::sync::Mutex;

use gauntlet_domain::{
    Budget, DiagnosticAllowance, Digest, EpochSeconds, EventLogOffset, EvidenceRef, Limits,
    MicroUsd, ReferenceId, ReferenceVersion, Run, RunDefinition, RunId, SettlementRecord, Spent,
    UnitId, VerificationRef,
};
use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::json::{self, JsonNumber, JsonValue};
use gauntlet_ports::state_store::{
    Checkpoint, RunEvent, RunEventKind, StateStore, StateStoreError,
};
use rusqlite::Connection;

use crate::encode;
use crate::event_log::{append_to_log, open_connection, read_entries};

/// Maps an [`gauntlet_ports::event_log::EventLogError`] onto the store's
/// vocabulary WITHOUT losing the failure class: `Damaged` (a log row that
/// does not decode into the domain event types) is stored state that does
/// not decode — exactly what [`StateStoreError::Corrupt`] names — with the
/// damage detail preserved; anything else stays a read failure.
fn map_log_error(err: gauntlet_ports::event_log::EventLogError) -> StateStoreError {
    match err {
        gauntlet_ports::event_log::EventLogError::Damaged { detail } => StateStoreError::Corrupt {
            detail: format!("the event log is damaged: {detail}"),
        },
        other => StateStoreError::ReadFailed {
            detail: other.to_string(),
        },
    }
}

/// Placeholder fields for the checkpoint-only projection of [`load_run`]:
/// names the limitation instead of inventing data silently.
const PROJECTION_REFERENCE_ID: &str = "checkpoint-only";
/// The projection's reference version: unknown, so the smallest legal value.
const PROJECTION_REFERENCE_VERSION: u32 = 1;
/// The projection's fingerprint placeholder: a valid digest of nothing.
const PROJECTION_FINGERPRINT_HEX: &str = "0000000000000000";
/// The projection's intent: says what it is, never a fabricated purpose.
const PROJECTION_INTENT: &str = "run identity not yet recorded; projected from a checkpoint";

/// One §18.1 durable record kind carried by the `records` table (everything
/// except `runs`, `settlements`, and the log itself). Typed (AGENTS.md:
/// every fixed-value-set field is a type); the SQL CHECK pins the same list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordKind {
    Iteration,
    RoleInvocation,
    ReferenceVersion,
    Observation,
    Comparison,
    VerificationRecord,
    Residual,
    Payment,
    Failure,
    RecoveryEvent,
    CandidateRevision,
    PromotionDecision,
    Escalation,
}

impl RecordKind {
    /// The wire token stored in `records.record_type`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Iteration => "iteration",
            Self::RoleInvocation => "role_invocation",
            Self::ReferenceVersion => "reference_version",
            Self::Observation => "observation",
            Self::Comparison => "comparison",
            Self::VerificationRecord => "verification_record",
            Self::Residual => "residual",
            Self::Payment => "payment",
            Self::Failure => "failure",
            Self::RecoveryEvent => "recovery_event",
            Self::CandidateRevision => "candidate_revision",
            Self::PromotionDecision => "promotion_decision",
            Self::Escalation => "escalation",
        }
    }
}

impl core::fmt::Display for RecordKind {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The §16.3 settlement commit: the event's fields verbatim
/// (`SettlementEvent{unit_id, evidence_ref, verification_ref, digest, ts}`).
#[derive(Debug, Clone)]
pub struct SettlementCommit {
    pub run_id: RunId,
    pub unit_id: UnitId,
    pub evidence: EvidenceRef,
    pub verification: VerificationRef,
    pub reference_digest: Digest,
    pub at: EpochSeconds,
}

/// A durable run-state store in one embedded database file.
pub struct SqliteStateStore {
    conn: Mutex<Connection>,
}

impl SqliteStateStore {
    /// Opens (creating if needed) the store at `path`, applying any
    /// migrations not yet recorded.
    pub fn open(path: &Path) -> Result<Self, StateStoreError> {
        let conn =
            open_connection(path).map_err(|detail| StateStoreError::WriteFailed { detail })?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// An in-memory store, for tests that never need a second process.
    pub fn open_in_memory() -> Result<Self, StateStoreError> {
        let conn = open_connection(Path::new(":memory:"))
            .map_err(|detail| StateStoreError::WriteFailed { detail })?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Records the authoritative run identity. Idempotent by `run_id`: a
    /// second recording of the SAME identity is a no-op, and a recording
    /// that disagrees with the stored identity on ANY column — the frozen
    /// reference (spec §5.3.3) or any other identity field — is a typed
    /// error naming every differing field and both values.
    pub fn record_run(&self, run: &Run, _deadline: Deadline) -> Result<(), StateStoreError> {
        let conn = self
            .conn
            .lock()
            .expect("the store connection is not poisoned");
        let budget_text = encode::encode_budget_ceiling(run.budget());
        let parent = run.parent_run().map(|id| id.to_string());
        let insert = conn.execute(
            "INSERT INTO runs (
                run_id, created_at, reference_id, reference_version, reference_digest,
                intent, config_fingerprint, environment_fingerprint, parent_run, budget
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            (
                run.id().to_string(),
                run.created_at().get(),
                run.reference_version().reference_id().as_str(),
                u64::from(run.reference_version().version()),
                run.reference_version().digest().to_string(),
                run.intent(),
                run.config_fingerprint().to_string(),
                run.environment_fingerprint().to_string(),
                parent,
                budget_text,
            ),
        );
        match insert {
            Ok(_) => Ok(()),
            Err(rusqlite::Error::SqliteFailure(ffi, _))
                if ffi.code == rusqlite::ErrorCode::ConstraintViolation =>
            {
                // The run id already exists. Idempotency requires the FULL
                // stored identity to agree — not just the digest: a same
                // digest with a different intent/budget/fingerprint is a
                // disagreement that must be surfaced, never silently
                // absorbed as a no-op.
                let stored = conn
                    .query_row(
                        "SELECT created_at, reference_id, reference_version, \
                         reference_digest, intent, config_fingerprint, \
                         environment_fingerprint, parent_run, budget \
                         FROM runs WHERE run_id = ?1",
                        [run.id().to_string()],
                        |row| {
                            Ok((
                                row.get::<_, i64>(0)?,
                                row.get::<_, String>(1)?,
                                row.get::<_, i64>(2)?,
                                row.get::<_, String>(3)?,
                                row.get::<_, String>(4)?,
                                row.get::<_, String>(5)?,
                                row.get::<_, String>(6)?,
                                row.get::<_, Option<String>>(7)?,
                                row.get::<_, String>(8)?,
                            ))
                        },
                    )
                    .map_err(|err| StateStoreError::ReadFailed {
                        detail: format!("reading the stored run identity failed: {err}"),
                    })?;
                let differences = identity_differences(&stored, run)?;
                if differences.is_empty() {
                    Ok(())
                } else {
                    Err(StateStoreError::WriteFailed {
                        detail: format!(
                            "run {} is already recorded and the offered identity disagrees on: \
                             {}; a run never changes its frozen reference or its recorded \
                             identity (spec 5.3.3)",
                            run.id(),
                            differences.join("; ")
                        ),
                    })
                }
            }
            Err(err) => Err(StateStoreError::WriteFailed {
                detail: format!("recording run {} failed: {err}", run.id()),
            }),
        }
    }

    /// The §16.3 atomic commit sequence, in order, no reordering:
    ///
    /// 1. `EventLog.append(SettlementEvent{...})` — durable, fsync-on-append;
    /// 2. the settlement row — idempotent by `(run_id, unit_id,
    ///    verification_ref)`.
    ///
    /// A crash between the two is repaired by
    /// [`SqliteStateStore::complete_pending_settlements`]; a crash before
    /// step 1 leaves the unit retryable, because nothing durable happened.
    pub fn commit_settlement(
        &self,
        commit: SettlementCommit,
        _deadline: Deadline,
    ) -> Result<EventLogOffset, StateStoreError> {
        let conn = self
            .conn
            .lock()
            .expect("the store connection is not poisoned");
        let event = RunEvent {
            run_id: commit.run_id.clone(),
            kind: RunEventKind::SettlementCommitted {
                unit_id: commit.unit_id.clone(),
                evidence: commit.evidence.clone(),
                verification: commit.verification.clone(),
                reference_digest: commit.reference_digest.clone(),
            },
            at: commit.at,
        };
        // Step 1 — durable before anything else happens (§16.3).
        let offset = append_to_log(&conn, &event).map_err(|err| StateStoreError::WriteFailed {
            detail: err.to_string(),
        })?;
        // Step 2 — idempotent keyed insert. If THIS step fails, the event
        // from step 1 is already durable, and the error must say so: a
        // caller reading a bare failure as "nothing happened" would re-run
        // the unit and append a duplicate settlement event. The error names
        // the durable offset, the recovery that completes it, and why
        // re-running is wrong.
        self.apply_settlement_row_with(&conn, &commit, offset)
            .map_err(|err| StateStoreError::WriteFailed {
                detail: format!(
                    "{err}; the settlement event for unit {} of run {} IS durable at offset {} \
                     — complete_pending_settlements will complete this settlement on replay; \
                     re-running the unit is wrong (it would append a duplicate settlement \
                     event)",
                    commit.unit_id,
                    commit.run_id,
                    offset.get()
                ),
            })?;
        Ok(offset)
    }

    /// §16.3 step 2 on its own: writes the settlement row for a settlement
    /// event that is ALREADY durable at `offset`. Idempotent by
    /// `(run_id, unit_id, verification_ref)` — applying the same event twice
    /// adds one row and returns 0 the second time. This is the half the
    /// crash-recovery path replays.
    pub fn apply_settlement_row(
        &self,
        commit: SettlementCommit,
        offset: EventLogOffset,
        _deadline: Deadline,
    ) -> Result<usize, StateStoreError> {
        let conn = self
            .conn
            .lock()
            .expect("the store connection is not poisoned");
        insert_settlement(&conn, &commit, offset)
    }

    /// [`SqliteStateStore::apply_settlement_row`] over a held connection.
    fn apply_settlement_row_with(
        &self,
        conn: &Connection,
        commit: &SettlementCommit,
        offset: EventLogOffset,
    ) -> Result<usize, StateStoreError> {
        insert_settlement(conn, commit, offset)
    }

    /// §18.3 resume step 1: replays the run's settlement events from the log
    /// and completes any half-applied settlement — idempotent by
    /// `(run_id, unit_id, verification_ref)` (§16.3). Returns how many
    /// settlement rows this call added; a second call over the same log adds
    /// none.
    pub fn complete_pending_settlements(
        &self,
        run_id: &RunId,
        _deadline: Deadline,
    ) -> Result<usize, StateStoreError> {
        let conn = self
            .conn
            .lock()
            .expect("the store connection is not poisoned");
        let entries = read_entries(&conn, EventLogOffset::new(0)).map_err(map_log_error)?;
        let mut applied = 0;
        for entry in entries {
            let event = entry.event;
            if event.run_id != *run_id {
                continue;
            }
            let RunEventKind::SettlementCommitted {
                unit_id,
                evidence,
                verification,
                reference_digest,
            } = event.kind
            else {
                continue;
            };
            let added = insert_settlement(
                &conn,
                &SettlementCommit {
                    run_id: event.run_id.clone(),
                    unit_id,
                    evidence,
                    verification,
                    reference_digest,
                    at: event.at,
                },
                entry.offset,
            )?;
            applied += added;
        }
        Ok(applied)
    }

    /// The run's durable settlement rows, ordered by `(unit_id,
    /// verification_ref)` — the order the export pins.
    pub fn settlements(
        &self,
        run_id: &RunId,
        _deadline: Deadline,
    ) -> Result<Vec<SettlementRecord>, StateStoreError> {
        let conn = self
            .conn
            .lock()
            .expect("the store connection is not poisoned");
        settlements_of(&conn, run_id)
    }

    /// Appends one §18.1 durable record. The payload MUST be valid JSON —
    /// model output is untrusted input; it is parsed here, before admission,
    /// and rejected as a typed error otherwise.
    pub fn append_record(
        &self,
        run_id: &RunId,
        kind: RecordKind,
        payload: &str,
        at: EpochSeconds,
        _deadline: Deadline,
    ) -> Result<u64, StateStoreError> {
        let conn = self
            .conn
            .lock()
            .expect("the store connection is not poisoned");
        json::parse(payload).map_err(|err| StateStoreError::WriteFailed {
            detail: format!("the {kind} payload is not valid JSON: {err}"),
        })?;
        conn.execute(
            "INSERT INTO records (run_id, record_type, payload, at) VALUES (?1, ?2, ?3, ?4)",
            (run_id.to_string(), kind.as_str(), payload, at.get()),
        )
        .map_err(|err| StateStoreError::WriteFailed {
            detail: format!("appending a {kind} record for run {run_id} failed: {err}"),
        })?;
        Ok(u64::try_from(conn.last_insert_rowid()).unwrap_or(0))
    }

    /// Loads the authoritative runs row, when there is one.
    fn load_recorded_run(
        conn: &Connection,
        run_id: &RunId,
    ) -> Result<Option<Run>, StateStoreError> {
        let row = conn.query_row(
            "SELECT created_at, reference_id, reference_version, reference_digest, intent, \
             config_fingerprint, environment_fingerprint, parent_run, budget \
             FROM runs WHERE run_id = ?1",
            [run_id.to_string()],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, Option<String>>(7)?,
                    row.get::<_, String>(8)?,
                ))
            },
        );
        let (
            created_at,
            reference_id,
            reference_version,
            reference_digest,
            intent,
            config_fingerprint,
            environment_fingerprint,
            parent_run,
            budget_text,
        ) = match row {
            Ok(stored) => stored,
            Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
            Err(err) => {
                return Err(StateStoreError::ReadFailed {
                    detail: format!("reading run {run_id} failed: {err}"),
                });
            }
        };
        if created_at < 0 {
            return Err(StateStoreError::Corrupt {
                detail: format!(
                    "run {run_id} holds a created_at of {created_at} — negative; this adapter \
                     never writes one, so the row was damaged or tampered out of band"
                ),
            });
        }
        let ceiling = encode::decode_budget_ceiling(&budget_text).map_err(|err| {
            StateStoreError::Corrupt {
                detail: format!("run {run_id} holds an undecodable budget: {err}"),
            }
        })?;
        let budget = Budget::new(
            ceiling.limits,
            ceiling.recovery_reserve_pct,
            ceiling.diagnostic,
        )
        .map_err(|err| StateStoreError::Corrupt {
            detail: format!("run {run_id} holds an unusable budget: {err}"),
        })?;
        let run = Run::new(RunDefinition {
            id: run_id.clone(),
            created_at: EpochSeconds::new(created_at.max(0) as u64),
            reference_version: ReferenceVersion::new(
                ReferenceId::new(&reference_id).map_err(|err| StateStoreError::Corrupt {
                    detail: format!("run {run_id} holds an invalid reference id: {err}"),
                })?,
                u32::try_from(reference_version).unwrap_or(0),
                Digest::parse(&reference_digest).map_err(|err| StateStoreError::Corrupt {
                    detail: format!("run {run_id} holds an invalid digest: {err}"),
                })?,
            ),
            intent,
            config_fingerprint: Digest::parse(&config_fingerprint).map_err(|err| {
                StateStoreError::Corrupt {
                    detail: format!("run {run_id} holds an invalid config fingerprint: {err}"),
                }
            })?,
            environment_fingerprint: Digest::parse(&environment_fingerprint).map_err(|err| {
                StateStoreError::Corrupt {
                    detail: format!("run {run_id} holds an invalid environment fingerprint: {err}"),
                }
            })?,
            budget,
            parent_run: parent_run
                .map(|id| {
                    RunId::new(&id).map_err(|err| StateStoreError::Corrupt {
                        detail: format!("run {run_id} holds an invalid parent run id: {err}"),
                    })
                })
                .transpose()?,
        })
        .map_err(|err| StateStoreError::Corrupt {
            detail: format!("run {run_id} does not reconstruct: {err}"),
        })?;
        Ok(Some(run))
    }

    /// The checkpoint-only projection (see the module docs): true run id and
    /// frozen digest, placeholders for everything the port's durable records
    /// never carried.
    fn project_run_from_checkpoint(
        conn: &Connection,
        run_id: &RunId,
        digest: &Digest,
    ) -> Result<Run, StateStoreError> {
        let created_at: Option<i64> = conn
            .query_row(
                "SELECT MIN(at) FROM event_log WHERE run_id = ?1",
                [run_id.to_string()],
                |row| row.get(0),
            )
            .map_err(|err| StateStoreError::ReadFailed {
                detail: format!("reading the first event of run {run_id} failed: {err}"),
            })?;
        if created_at.is_some_and(|value| value < 0) {
            return Err(StateStoreError::Corrupt {
                detail: format!(
                    "run {run_id} holds an event timestamp of {} — negative; this adapter \
                     never writes one, so the row was damaged or tampered out of band",
                    created_at.unwrap_or_default()
                ),
            });
        }
        Run::new(RunDefinition {
            id: run_id.clone(),
            created_at: EpochSeconds::new(created_at.unwrap_or(0).max(0) as u64),
            reference_version: ReferenceVersion::new(
                ReferenceId::new(PROJECTION_REFERENCE_ID).map_err(|err| {
                    StateStoreError::Corrupt {
                        detail: format!("the projection reference id is invalid: {err}"),
                    }
                })?,
                PROJECTION_REFERENCE_VERSION,
                digest.clone(),
            ),
            intent: PROJECTION_INTENT.to_owned(),
            config_fingerprint: projection_fingerprint()?,
            environment_fingerprint: projection_fingerprint()?,
            // Minimal legal ceilings: a projection cannot spend; the
            // authority is the runs row, recorded by the composition root.
            budget: projection_budget()?,
            parent_run: None,
        })
        .map_err(|err| StateStoreError::Corrupt {
            detail: format!("run {run_id} does not project: {err}"),
        })
    }
}

fn projection_fingerprint() -> Result<Digest, StateStoreError> {
    Digest::new("sha256", PROJECTION_FINGERPRINT_HEX).map_err(|err| StateStoreError::Corrupt {
        detail: format!("the projection fingerprint is invalid: {err}"),
    })
}

fn projection_budget() -> Result<Budget, StateStoreError> {
    let limits =
        Limits::new(1, 1, 1, 1, 1, MicroUsd::new(1)).map_err(|err| StateStoreError::Corrupt {
            detail: format!("the projection budget ceilings are unusable: {err}"),
        })?;
    Budget::new(limits, 1, DiagnosticAllowance::new(1, MicroUsd::new(1))).map_err(|err| {
        StateStoreError::Corrupt {
            detail: format!("the projection budget is unusable: {err}"),
        }
    })
}

/// A zero `Spent` — the only value the domain can hand out without a charge,
/// and therefore the only value any `Checkpoint` can carry today (see the
/// module docs).
fn zero_spent() -> Spent {
    *projection_budget()
        .expect("the minimal legal budget is established above")
        .spent()
}

/// §16.3 step 2: the idempotent keyed insert, returning how many rows it
/// added. Replaying an event twice inserts once — the key is the spec's own.
/// Before the insert, the claimed `offset` is cross-validated against the
/// durable log (see [`verify_settlement_event_at`]): a settlement row may
/// never name an event it did not come from.
fn insert_settlement(
    conn: &Connection,
    commit: &SettlementCommit,
    offset: EventLogOffset,
) -> Result<usize, StateStoreError> {
    verify_settlement_event_at(conn, commit, offset)?;
    conn.execute(
        "INSERT OR IGNORE INTO settlements (
            run_id, unit_id, verification_ref, evidence_ref, reference_digest,
            event_log_offset, settled_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        (
            commit.run_id.to_string(),
            commit.unit_id.as_str(),
            commit.verification.as_str(),
            commit.evidence.as_str(),
            commit.reference_digest.to_string(),
            offset.get(),
            commit.at.get(),
        ),
    )
    .map_err(|err| StateStoreError::WriteFailed {
        detail: format!(
            "writing the settlement row for unit {} of run {} failed: {err}",
            commit.unit_id, commit.run_id
        ),
    })
}

/// Cross-validates a settlement row's claimed offset against the log before
/// the row lands: the event at `offset` must exist, belong to the commit's
/// run, be a `settlement_committed`, and carry exactly the commit's unit,
/// verification, evidence, and reference digest (spec §5.3.6: a
/// `SettlementRecord` "references the event-log offset that made it
/// durable" — its OWN offset, not an offset the caller attests). A missing
/// row or any mismatch is a typed refusal naming both what was claimed and
/// what is there; the replay path passes trivially because it hands over
/// the log's own offsets.
fn verify_settlement_event_at(
    conn: &Connection,
    commit: &SettlementCommit,
    offset: EventLogOffset,
) -> Result<(), StateStoreError> {
    let stored = conn.query_row(
        "SELECT run_id, kind, payload FROM event_log WHERE offset = ?1",
        [offset.get()],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        },
    );
    let (stored_run, stored_kind, stored_payload) = match stored {
        Ok(columns) => columns,
        Err(rusqlite::Error::QueryReturnedNoRows) => {
            return Err(StateStoreError::WriteFailed {
                detail: format!(
                    "the settlement row for unit {} of run {} claims offset {}, but the event \
                     log has no row there — a settlement row must name the durable settlement \
                     event that made it real (spec 5.3.6)",
                    commit.unit_id,
                    commit.run_id,
                    offset.get()
                ),
            });
        }
        Err(err) => {
            return Err(StateStoreError::WriteFailed {
                detail: format!(
                    "reading the event at offset {} to validate the settlement row for unit {} \
                     of run {} failed: {err}",
                    offset.get(),
                    commit.unit_id,
                    commit.run_id
                ),
            });
        }
    };
    let event = match encode::decode_event(
        RunId::new(&stored_run).map_err(|err| StateStoreError::WriteFailed {
            detail: format!(
                "the event at offset {} holds a run id that does not decode ({stored_run:?}): \
                 {err}",
                offset.get()
            ),
        })?,
        commit.at,
        &stored_kind,
        &stored_payload,
    ) {
        Ok(event) => event,
        Err(err) => {
            return Err(StateStoreError::WriteFailed {
                detail: format!(
                    "the settlement row for unit {} of run {} claims offset {}, but the event \
                     there does not decode: {err}",
                    commit.unit_id,
                    commit.run_id,
                    offset.get()
                ),
            });
        }
    };
    let RunEventKind::SettlementCommitted {
        unit_id,
        evidence,
        verification,
        reference_digest,
    } = event.kind
    else {
        return Err(StateStoreError::WriteFailed {
            detail: format!(
                "the settlement row for unit {} of run {} claims offset {}, but the event log \
                 row there is a different event (kind {stored_kind:?}, run {stored_run}) — a \
                 settlement row must name its own durable settlement event (spec 5.3.6)",
                commit.unit_id,
                commit.run_id,
                offset.get()
            ),
        });
    };
    if event.run_id != commit.run_id
        || unit_id != commit.unit_id
        || evidence != commit.evidence
        || verification != commit.verification
        || reference_digest != commit.reference_digest
    {
        return Err(StateStoreError::WriteFailed {
            detail: format!(
                "the settlement row for unit {} of run {} claims offset {}, but the event there \
                 is the settlement of unit {unit_id} of run {} (verification {}, evidence {}) — \
                 the claimed fields and the durable event disagree (spec 5.3.6)",
                commit.unit_id,
                commit.run_id,
                offset.get(),
                event.run_id,
                verification.as_str(),
                evidence.as_str()
            ),
        });
    }
    Ok(())
}

/// The settlement rows of one run, ordered by `(unit_id, verification_ref)`.
fn settlements_of(
    conn: &Connection,
    run_id: &RunId,
) -> Result<Vec<SettlementRecord>, StateStoreError> {
    let mut statement = conn
        .prepare(
            "SELECT unit_id, verification_ref, evidence_ref, event_log_offset \
             FROM settlements WHERE run_id = ?1 ORDER BY unit_id ASC, verification_ref ASC",
        )
        .map_err(|err| StateStoreError::ReadFailed {
            detail: format!("preparing the settlement query failed: {err}"),
        })?;
    let rows = statement
        .query_map([run_id.to_string()], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })
        .map_err(|err| StateStoreError::ReadFailed {
            detail: format!("reading settlements failed: {err}"),
        })?;
    let mut out = Vec::new();
    for row in rows {
        let (unit_id, verification, evidence, offset) =
            row.map_err(|err| StateStoreError::Corrupt {
                detail: format!("a settlement row does not decode: {err}"),
            })?;
        out.push(SettlementRecord::new(
            run_id.clone(),
            UnitId::new(&unit_id).map_err(|err| StateStoreError::Corrupt {
                detail: format!("a stored unit id does not decode: {err}"),
            })?,
            EvidenceRef::new(&evidence).map_err(|err| StateStoreError::Corrupt {
                detail: format!("a stored evidence ref does not decode: {err}"),
            })?,
            VerificationRef::new(&verification).map_err(|err| StateStoreError::Corrupt {
                detail: format!("a stored verification ref does not decode: {err}"),
            })?,
            EventLogOffset::new(u64::try_from(offset).unwrap_or(0)),
        ));
    }
    Ok(out)
}

fn uint(value: u64) -> JsonValue {
    JsonValue::Number(JsonNumber::Uint(value))
}

/// Every identity column on which a stored `runs` row and a re-recorded
/// [`Run`] disagree, as "field (stored X, offered Y)" entries — empty when
/// the recording is idempotent. The budget is compared field-by-field after
/// decoding (an undecodable stored budget is `Corrupt`, not a silent
/// mismatch).
fn identity_differences(
    stored: &(
        i64,
        String,
        i64,
        String,
        String,
        String,
        String,
        Option<String>,
        String,
    ),
    run: &Run,
) -> Result<Vec<String>, StateStoreError> {
    let (
        created_at,
        reference_id,
        reference_version,
        reference_digest,
        intent,
        config_fingerprint,
        environment_fingerprint,
        parent_run,
        budget_text,
    ) = stored;
    let mut differences: Vec<String> = Vec::new();
    if u64::try_from(*created_at) != Ok(run.created_at().get()) {
        differences.push(format!(
            "created_at (stored {created_at}, offered {})",
            run.created_at().get()
        ));
    }
    if reference_id != run.reference_version().reference_id().as_str() {
        differences.push(format!(
            "reference_id (stored {reference_id:?}, offered {:?})",
            run.reference_version().reference_id().as_str()
        ));
    }
    if *reference_version != i64::from(run.reference_version().version()) {
        differences.push(format!(
            "reference_version (stored {reference_version}, offered {})",
            run.reference_version().version()
        ));
    }
    if reference_digest != &run.reference_version().digest().to_string() {
        differences.push(format!(
            "reference_digest (stored {reference_digest:?}, offered {:?})",
            run.reference_version().digest().to_string()
        ));
    }
    if intent != run.intent() {
        differences.push(format!(
            "intent (stored {intent:?}, offered {:?})",
            run.intent()
        ));
    }
    if config_fingerprint != &run.config_fingerprint().to_string() {
        differences.push(format!(
            "config_fingerprint (stored {config_fingerprint:?}, offered {:?})",
            run.config_fingerprint().to_string()
        ));
    }
    if environment_fingerprint != &run.environment_fingerprint().to_string() {
        differences.push(format!(
            "environment_fingerprint (stored {environment_fingerprint:?}, offered {:?})",
            run.environment_fingerprint().to_string()
        ));
    }
    let offered_parent = run.parent_run().map(|id| id.to_string());
    if parent_run != &offered_parent {
        differences.push(format!(
            "parent_run (stored {parent_run:?}, offered {offered_parent:?})"
        ));
    }
    let stored_budget =
        encode::decode_budget_ceiling(budget_text).map_err(|err| StateStoreError::Corrupt {
            detail: format!("run {} holds an undecodable budget: {err}", run.id()),
        })?;
    let offered = run.budget();
    let stored_limits = &stored_budget.limits;
    let offered_limits = offered.limits();
    let budget_fields = [
        (
            "recovery_reserve_pct",
            u64::from(stored_budget.recovery_reserve_pct),
            u64::from(offered.recovery_reserve_pct()),
        ),
        (
            "rounds",
            u64::from(stored_limits.max_rounds()),
            u64::from(offered_limits.max_rounds()),
        ),
        (
            "parallel",
            u64::from(stored_limits.max_parallel()),
            u64::from(offered_limits.max_parallel()),
        ),
        (
            "depth",
            u64::from(stored_limits.max_depth()),
            u64::from(offered_limits.max_depth()),
        ),
        (
            "wall_seconds",
            stored_limits.max_wall_seconds(),
            offered_limits.max_wall_seconds(),
        ),
        (
            "tokens",
            stored_limits.max_tokens(),
            offered_limits.max_tokens(),
        ),
        (
            "cost_usd",
            stored_limits.max_cost_usd().get(),
            offered_limits.max_cost_usd().get(),
        ),
        (
            "diagnostic.tokens",
            stored_budget.diagnostic.tokens(),
            offered.diagnostic_allowance().tokens(),
        ),
        (
            "diagnostic.cost_usd",
            stored_budget.diagnostic.cost_usd().get(),
            offered.diagnostic_allowance().cost_usd().get(),
        ),
    ];
    for (field, was, now) in budget_fields {
        if was != now {
            differences.push(format!("budget.{field} (stored {was}, offered {now})"));
        }
    }
    Ok(differences)
}

impl StateStore for SqliteStateStore {
    fn append_event(
        &self,
        event: RunEvent,
        _deadline: Deadline,
    ) -> Result<EventLogOffset, StateStoreError> {
        let conn = self
            .conn
            .lock()
            .expect("the store connection is not poisoned");
        append_to_log(&conn, &event).map_err(|err| StateStoreError::WriteFailed {
            detail: err.to_string(),
        })
    }

    fn load_run(
        &self,
        run_id: &RunId,
        _deadline: Deadline,
    ) -> Result<Option<Run>, StateStoreError> {
        let conn = self
            .conn
            .lock()
            .expect("the store connection is not poisoned");
        if let Some(run) = Self::load_recorded_run(&conn, run_id)? {
            return Ok(Some(run));
        }
        let latest: Option<(String, i64)> = match conn.query_row(
            "SELECT reference_digest, seq FROM checkpoints WHERE run_id = ?1 \
             ORDER BY iteration_no DESC, seq DESC LIMIT 1",
            [run_id.to_string()],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        ) {
            Ok(pair) => Some(pair),
            Err(rusqlite::Error::QueryReturnedNoRows) => None,
            Err(err) => {
                return Err(StateStoreError::ReadFailed {
                    detail: format!("reading the latest checkpoint of run {run_id} failed: {err}"),
                });
            }
        };
        match latest {
            Some((digest_text, _)) => {
                let digest =
                    Digest::parse(&digest_text).map_err(|err| StateStoreError::Corrupt {
                        detail: format!(
                            "run {run_id} holds a checkpoint digest that does not decode: {err}"
                        ),
                    })?;
                Ok(Some(Self::project_run_from_checkpoint(
                    &conn, run_id, &digest,
                )?))
            }
            None => Ok(None),
        }
    }

    fn checkpoint(
        &self,
        checkpoint: Checkpoint,
        _deadline: Deadline,
    ) -> Result<(), StateStoreError> {
        let conn = self
            .conn
            .lock()
            .expect("the store connection is not poisoned");
        conn.execute(
            "INSERT INTO checkpoints (
                run_id, iteration_no, reference_digest, unit_states, budget_spent,
                in_flight, event_log_offset
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            (
                checkpoint.run_id.to_string(),
                u64::from(checkpoint.iteration_no.get()),
                checkpoint.reference_digest.to_string(),
                encode::encode_unit_states(&checkpoint.unit_states),
                encode::encode_spent(&checkpoint.budget_spent),
                encode::encode_in_flight(&checkpoint.in_flight),
                checkpoint.event_log_offset.get(),
            ),
        )
        .map_err(|err| StateStoreError::WriteFailed {
            detail: format!(
                "writing the iteration-{} checkpoint of run {} failed: {err}",
                checkpoint.iteration_no, checkpoint.run_id
            ),
        })?;
        Ok(())
    }

    fn resume(
        &self,
        run_id: &RunId,
        _deadline: Deadline,
    ) -> Result<Option<Checkpoint>, StateStoreError> {
        let conn = self
            .conn
            .lock()
            .expect("the store connection is not poisoned");
        let stored = match conn.query_row(
            "SELECT seq, iteration_no, reference_digest, unit_states, budget_spent, in_flight, \
             event_log_offset FROM checkpoints WHERE run_id = ?1 \
             ORDER BY iteration_no DESC, seq DESC LIMIT 1",
            [run_id.to_string()],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, i64>(6)?,
                ))
            },
        ) {
            Ok(row) => row,
            Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
            Err(err) => {
                return Err(StateStoreError::ReadFailed {
                    detail: format!("resuming run {run_id} failed: {err}"),
                });
            }
        };
        let (seq, iteration_no, digest_text, unit_states, budget_spent, in_flight, offset) = stored;
        let unit_states =
            encode::decode_unit_states(&unit_states).map_err(|err| StateStoreError::Corrupt {
                detail: format!("run {run_id} checkpoint {seq} holds bad unit states: {err}"),
            })?;
        let spent =
            encode::decode_spent(&budget_spent).map_err(|err| StateStoreError::Corrupt {
                detail: format!("run {run_id} checkpoint {seq} holds a bad budget: {err}"),
            })?;
        let in_flight =
            encode::decode_in_flight(&in_flight).map_err(|err| StateStoreError::Corrupt {
                detail: format!("run {run_id} checkpoint {seq} holds bad in-flight work: {err}"),
            })?;
        let budget_spent = spent_or_refuse(spent, run_id, seq)?;
        Ok(Some(Checkpoint {
            run_id: run_id.clone(),
            reference_digest: Digest::parse(&digest_text).map_err(|err| {
                StateStoreError::Corrupt {
                    detail: format!("run {run_id} checkpoint {seq} holds a bad digest: {err}"),
                }
            })?,
            iteration_no: gauntlet_domain::IterationNo::new(
                u32::try_from(iteration_no).unwrap_or(0),
            )
            .map_err(|err| StateStoreError::Corrupt {
                detail: format!("run {run_id} checkpoint {seq} holds a bad iteration: {err}"),
            })?,
            unit_states,
            budget_spent,
            in_flight,
            event_log_offset: EventLogOffset::new(u64::try_from(offset).unwrap_or(0)),
        }))
    }

    /// Deterministic export (spec §6.2: "Export is deterministic"): stable
    /// segments and explicit `ORDER BY` everywhere — the output is a pure
    /// function of durable state, never of query timing. Timestamps are the
    /// stored ones; none is generated here.
    fn export_jsonl(&self, run_id: &RunId, _deadline: Deadline) -> Result<String, StateStoreError> {
        let conn = self
            .conn
            .lock()
            .expect("the store connection is not poisoned");
        let mut lines: Vec<String> = Vec::new();
        // Segment 1 — the run identity, when recorded.
        if let Some(run) = Self::load_recorded_run(&conn, run_id)? {
            let budget =
                json::parse(&encode::encode_budget_ceiling(run.budget())).map_err(|err| {
                    StateStoreError::ReadFailed {
                        detail: format!("the run's budget does not re-parse: {err}"),
                    }
                })?;
            lines.push(
                JsonValue::Object(vec![
                    ("record".to_owned(), JsonValue::Str("run".to_owned())),
                    ("run_id".to_owned(), JsonValue::Str(run.id().to_string())),
                    ("created_at".to_owned(), uint(run.created_at().get())),
                    (
                        "reference_id".to_owned(),
                        JsonValue::Str(run.reference_version().reference_id().as_str().to_owned()),
                    ),
                    (
                        "reference_version".to_owned(),
                        uint(u64::from(run.reference_version().version())),
                    ),
                    (
                        "reference_digest".to_owned(),
                        JsonValue::Str(run.reference_version().digest().to_string()),
                    ),
                    ("intent".to_owned(), JsonValue::Str(run.intent().to_owned())),
                    (
                        "config_fingerprint".to_owned(),
                        JsonValue::Str(run.config_fingerprint().to_string()),
                    ),
                    (
                        "environment_fingerprint".to_owned(),
                        JsonValue::Str(run.environment_fingerprint().to_string()),
                    ),
                    (
                        "parent_run".to_owned(),
                        match run.parent_run() {
                            Some(parent) => JsonValue::Str(parent.to_string()),
                            None => JsonValue::Null,
                        },
                    ),
                    ("budget".to_owned(), budget),
                ])
                .to_json_text(),
            );
        }
        // Segment 2 — events, in offset order (the append order).
        let entries = read_entries(&conn, EventLogOffset::new(0)).map_err(map_log_error)?;
        for entry in entries {
            if entry.event.run_id != *run_id {
                continue;
            }
            let (kind, payload) = encode::encode_event(&entry.event.kind);
            let body = json::parse(&payload).map_err(|err| StateStoreError::ReadFailed {
                detail: format!("an event payload does not re-parse: {err}"),
            })?;
            lines.push(
                JsonValue::Object(vec![
                    ("record".to_owned(), JsonValue::Str("event".to_owned())),
                    ("offset".to_owned(), uint(entry.offset.get())),
                    ("at".to_owned(), uint(entry.event.at.get())),
                    ("kind".to_owned(), JsonValue::Str(kind)),
                    ("body".to_owned(), body),
                ])
                .to_json_text(),
            );
        }
        // Segment 3 — checkpoints, ordered by (iteration, write order).
        let mut statement = conn
            .prepare(
                "SELECT seq, iteration_no, reference_digest, unit_states, budget_spent, \
                 in_flight, event_log_offset FROM checkpoints WHERE run_id = ?1 \
                 ORDER BY iteration_no ASC, seq ASC",
            )
            .map_err(|err| StateStoreError::ReadFailed {
                detail: format!("preparing the checkpoint export failed: {err}"),
            })?;
        let checkpoint_rows = statement
            .query_map([run_id.to_string()], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, i64>(6)?,
                ))
            })
            .map_err(|err| StateStoreError::ReadFailed {
                detail: format!("reading checkpoints for export failed: {err}"),
            })?;
        for row in checkpoint_rows {
            let (seq, iteration_no, digest, unit_states, budget_spent, in_flight, offset) = row
                .map_err(|err| StateStoreError::ReadFailed {
                    detail: format!("a checkpoint row does not read: {err}"),
                })?;
            let unit_body =
                json::parse(&unit_states).map_err(|err| StateStoreError::ReadFailed {
                    detail: format!("checkpoint {seq} unit states do not re-parse: {err}"),
                })?;
            let spent_body =
                json::parse(&budget_spent).map_err(|err| StateStoreError::ReadFailed {
                    detail: format!("checkpoint {seq} budget does not re-parse: {err}"),
                })?;
            let flight_body =
                json::parse(&in_flight).map_err(|err| StateStoreError::ReadFailed {
                    detail: format!("checkpoint {seq} in-flight list does not re-parse: {err}"),
                })?;
            lines.push(
                JsonValue::Object(vec![
                    ("record".to_owned(), JsonValue::Str("checkpoint".to_owned())),
                    ("seq".to_owned(), uint(u64::try_from(seq).unwrap_or(0))),
                    (
                        "iteration_no".to_owned(),
                        uint(u64::try_from(iteration_no).unwrap_or(0)),
                    ),
                    (
                        "event_log_offset".to_owned(),
                        uint(u64::try_from(offset).unwrap_or(0)),
                    ),
                    ("reference_digest".to_owned(), JsonValue::Str(digest)),
                    ("unit_states".to_owned(), unit_body),
                    ("budget_spent".to_owned(), spent_body),
                    ("in_flight".to_owned(), flight_body),
                ])
                .to_json_text(),
            );
        }
        // Segment 4 — settlements, ordered by (unit, verification).
        for record in settlements_of(&conn, run_id)? {
            lines.push(
                JsonValue::Object(vec![
                    ("record".to_owned(), JsonValue::Str("settlement".to_owned())),
                    (
                        "unit_id".to_owned(),
                        JsonValue::Str(record.unit_id().to_string()),
                    ),
                    (
                        "verification".to_owned(),
                        JsonValue::Str(record.verification().to_string()),
                    ),
                    (
                        "evidence".to_owned(),
                        JsonValue::Str(record.evidence().to_string()),
                    ),
                    (
                        "event_log_offset".to_owned(),
                        uint(record.event_log_offset().get()),
                    ),
                ])
                .to_json_text(),
            );
        }
        // Segment 5 — the remaining §18.1 record kinds, ordered by
        // (kind, seq).
        let mut statement = conn
            .prepare(
                "SELECT seq, record_type, payload, at FROM records WHERE run_id = ?1 \
                 ORDER BY record_type ASC, seq ASC",
            )
            .map_err(|err| StateStoreError::ReadFailed {
                detail: format!("preparing the records export failed: {err}"),
            })?;
        let record_rows = statement
            .query_map([run_id.to_string()], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            })
            .map_err(|err| StateStoreError::ReadFailed {
                detail: format!("reading records for export failed: {err}"),
            })?;
        for row in record_rows {
            let (seq, record_type, payload, at) =
                row.map_err(|err| StateStoreError::ReadFailed {
                    detail: format!("a record row does not read: {err}"),
                })?;
            if at < 0 {
                return Err(StateStoreError::Corrupt {
                    detail: format!(
                        "record seq {seq} of run {run_id} holds an at of {at} — negative; this \
                         adapter never writes one, so the row was damaged or tampered out of band"
                    ),
                });
            }
            let body = json::parse(&payload).map_err(|err| StateStoreError::ReadFailed {
                detail: format!("record {seq} does not re-parse: {err}"),
            })?;
            lines.push(
                JsonValue::Object(vec![
                    ("record".to_owned(), JsonValue::Str(record_type)),
                    ("seq".to_owned(), uint(u64::try_from(seq).unwrap_or(0))),
                    ("at".to_owned(), uint(at as u64)),
                    ("payload".to_owned(), body),
                ])
                .to_json_text(),
            );
        }
        Ok(lines.join("\n"))
    }
}

/// Resume fidelity for `budget_spent`: the six counters are durable, but the
/// domain's `Spent` cannot carry values back (no constructor outside
/// `Budget::charge`). Zero round-trips exactly; anything else is refused
/// loudly — never silently zeroed.
fn spent_or_refuse(
    numbers: encode::SpentNumbers,
    run_id: &RunId,
    seq: i64,
) -> Result<Spent, StateStoreError> {
    let zero = numbers.rounds == 0
        && numbers.wall_seconds == 0
        && numbers.tokens == 0
        && numbers.cost_usd == 0
        && numbers.diagnostic_tokens == 0
        && numbers.diagnostic_cost_usd == 0;
    if zero {
        Ok(zero_spent())
    } else {
        Err(StateStoreError::Corrupt {
            detail: format!(
                "run {run_id} checkpoint {seq} stores non-zero spend (tokens {}, micro-usd {}) \
                 that the domain Spent type cannot carry back — the port type needs a \
                 reconstruction path before non-zero spend can resume; refusing rather than \
                 silently zeroing",
                numbers.tokens, numbers.cost_usd
            ),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gauntlet_domain::{
        DiagnosticAllowance, InvocationId, IterationNo, ResultContractPath, UnitStatus,
    };
    use gauntlet_ports::state_store::Checkpoint;

    const RUN: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
    const INVOCATION: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAW";
    const DIGEST_HEX: &str = "c4d3e2f1a0b1c2d3";

    fn run_id() -> RunId {
        RunId::new(RUN).expect("valid run id")
    }

    fn bound() -> Deadline {
        Deadline::at_epoch(EpochSeconds::new(1_700_000_060))
    }

    fn digest() -> Digest {
        Digest::new("sha256", DIGEST_HEX).expect("valid digest")
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
            environment_fingerprint: Digest::new("sha256", "e5f6a7b8c4d3e2f1")
                .expect("valid digest"),
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

    fn checkpoint(iteration: u32, offset: u64, status: UnitStatus, in_flight: bool) -> Checkpoint {
        Checkpoint {
            run_id: run_id(),
            reference_digest: digest(),
            iteration_no: IterationNo::new(iteration).expect("1-based"),
            unit_states: vec![(UnitId::new("AK-900").expect("valid unit id"), status)],
            budget_spent: *run().budget().spent(),
            in_flight: if in_flight {
                vec![(
                    InvocationId::new(INVOCATION).expect("valid invocation id"),
                    ResultContractPath::new("result-contract.json").expect("valid path"),
                )]
            } else {
                Vec::new()
            },
            event_log_offset: EventLogOffset::new(offset),
        }
    }

    /// The golden export: the exact bytes a known history exports as, pinned
    /// so any ordering or shape drift is a diff, not a vibe. Note the two
    /// checkpoints are written OUT of iteration order — the export must
    /// order by iteration, not by write.
    #[test]
    fn export_is_the_pinned_golden_and_byte_identical() {
        let store = SqliteStateStore::open_in_memory().expect("store opens");
        store.record_run(&run(), bound()).expect("run records");
        let admitted = RunEvent {
            run_id: run_id(),
            kind: RunEventKind::InvocationAdmitted {
                invocation_id: InvocationId::new(INVOCATION).expect("valid invocation id"),
            },
            at: EpochSeconds::new(1_700_000_100),
        };
        store
            .append_event(admitted, bound())
            .expect("event appends");
        let settled = RunEvent {
            run_id: run_id(),
            kind: RunEventKind::SettlementCommitted {
                unit_id: UnitId::new("AK-900").expect("valid unit id"),
                evidence: EvidenceRef::new("evd-000900").expect("valid ref"),
                verification: VerificationRef::new("vrf-000900").expect("valid ref"),
                reference_digest: digest(),
            },
            at: EpochSeconds::new(1_700_000_101),
        };
        store.append_event(settled, bound()).expect("event appends");
        store
            .complete_pending_settlements(&run_id(), bound())
            .expect("replay applies the settlement");
        store
            .checkpoint(checkpoint(2, 2, UnitStatus::InProgress, true), bound())
            .expect("checkpoint writes");
        store
            .checkpoint(checkpoint(1, 1, UnitStatus::Open, false), bound())
            .expect("checkpoint writes");
        store
            .append_record(
                &run_id(),
                RecordKind::Observation,
                r#"{"facet":"filesystem"}"#,
                EpochSeconds::new(1_700_000_103),
                bound(),
            )
            .expect("record appends");

        let export = store.export_jsonl(&run_id(), bound()).expect("exports");
        let again = store
            .export_jsonl(&run_id(), bound())
            .expect("exports again");
        assert_eq!(export, again, "two exports must be byte-identical");

        let expected = [
            r#"{"record":"run","run_id":"01ARZ3NDEKTSV4RRFFQ69G5FAV","created_at":1700000000,"reference_id":"ref-task6","reference_version":3,"reference_digest":"sha256:c4d3e2f1a0b1c2d3","intent":"settle the task","config_fingerprint":"sha256:a1b2c3d4e5f6a7b8","environment_fingerprint":"sha256:e5f6a7b8c4d3e2f1","parent_run":null,"budget":{"recovery_reserve_pct":20,"limits":{"rounds":8,"parallel":2,"depth":2,"wall_seconds":3600,"tokens":1000,"cost_usd":500},"diagnostic":{"tokens":100,"cost_usd":50}}}"#,
            r#"{"record":"event","offset":1,"at":1700000100,"kind":"invocation_admitted","body":{"kind":"invocation_admitted","invocation_id":"01ARZ3NDEKTSV4RRFFQ69G5FAW"}}"#,
            r#"{"record":"event","offset":2,"at":1700000101,"kind":"settlement_committed","body":{"kind":"settlement_committed","unit_id":"AK-900","evidence":"evd-000900","verification":"vrf-000900","reference_digest":"sha256:c4d3e2f1a0b1c2d3"}}"#,
            r#"{"record":"checkpoint","seq":2,"iteration_no":1,"event_log_offset":1,"reference_digest":"sha256:c4d3e2f1a0b1c2d3","unit_states":[["AK-900","open"]],"budget_spent":{"rounds":0,"wall_seconds":0,"tokens":0,"cost_usd":0,"diagnostic_tokens":0,"diagnostic_cost_usd":0},"in_flight":[]}"#,
            r#"{"record":"checkpoint","seq":1,"iteration_no":2,"event_log_offset":2,"reference_digest":"sha256:c4d3e2f1a0b1c2d3","unit_states":[["AK-900","in_progress"]],"budget_spent":{"rounds":0,"wall_seconds":0,"tokens":0,"cost_usd":0,"diagnostic_tokens":0,"diagnostic_cost_usd":0},"in_flight":[["01ARZ3NDEKTSV4RRFFQ69G5FAW","result-contract.json"]]}"#,
            r#"{"record":"settlement","unit_id":"AK-900","verification":"vrf-000900","evidence":"evd-000900","event_log_offset":2}"#,
            r#"{"record":"observation","seq":1,"at":1700000103,"payload":{"facet":"filesystem"}}"#,
        ]
        .join("\n");
        assert_eq!(export, expected, "the export must be the pinned golden");
    }

    /// A run recorded twice with the same identity is a no-op; with a
    /// different frozen reference it is a typed refusal (spec §5.3.3).
    #[test]
    fn recording_a_moved_reference_is_refused() {
        let store = SqliteStateStore::open_in_memory().expect("store opens");
        store.record_run(&run(), bound()).expect("run records");
        store
            .record_run(&run(), bound())
            .expect("same identity is idempotent");
        let mut moved = RunDefinition {
            id: run_id(),
            created_at: EpochSeconds::new(1_700_000_000),
            reference_version: ReferenceVersion::new(
                ReferenceId::new("ref-task6").expect("valid reference id"),
                4,
                Digest::new("sha256", "0000000000000000").expect("valid digest"),
            ),
            intent: "settle the task".to_owned(),
            config_fingerprint: Digest::new("sha256", "a1b2c3d4e5f6a7b8").expect("valid digest"),
            environment_fingerprint: Digest::new("sha256", "e5f6a7b8c4d3e2f1")
                .expect("valid digest"),
            budget: run().budget().clone(),
            parent_run: None,
        };
        moved.intent = "settle the task differently".to_owned();
        let moved = Run::new(moved).expect("legal run");
        let error = store
            .record_run(&moved, bound())
            .expect_err("a run never moves its frozen reference");
        assert!(
            error.to_string().contains("frozen reference"),
            "the refusal names the invariant: {error}"
        );
    }

    /// Resume returns the LATEST checkpoint exactly (spec §18), and a fresh
    /// run resumes `None`.
    #[test]
    fn resume_returns_the_latest_checkpoint_exactly() {
        let store = SqliteStateStore::open_in_memory().expect("store opens");
        assert_eq!(
            store.resume(&run_id(), bound()).expect("resume works"),
            None,
            "a fresh run has no checkpoint"
        );
        let first = checkpoint(1, 1, UnitStatus::Open, true);
        let second = checkpoint(2, 4, UnitStatus::InProgress, false);
        store.checkpoint(first, bound()).expect("writes");
        store.checkpoint(second.clone(), bound()).expect("writes");
        assert_eq!(
            store.resume(&run_id(), bound()).expect("resume works"),
            Some(second),
            "the latest checkpoint comes back exactly"
        );
    }

    /// A payload that is not JSON is refused at the door (§21.2: model
    /// output is untrusted input — parse, validate, then admit).
    #[test]
    fn non_json_record_payloads_are_refused() {
        let store = SqliteStateStore::open_in_memory().expect("store opens");
        let error = store
            .append_record(
                &run_id(),
                RecordKind::Observation,
                "not json at all",
                EpochSeconds::new(1),
                bound(),
            )
            .expect_err("garbage must not be admitted");
        assert!(
            error.to_string().contains("not valid JSON"),
            "the refusal names why: {error}"
        );
    }

    /// The runs row is authoritative when present; a checkpoint-only run
    /// loads with its true id and frozen digest (the projection).
    #[test]
    fn load_run_prefers_the_recorded_identity() {
        let store = SqliteStateStore::open_in_memory().expect("store opens");
        store
            .checkpoint(checkpoint(1, 0, UnitStatus::Open, false), bound())
            .expect("writes");
        let projected = store
            .load_run(&run_id(), bound())
            .expect("loads")
            .expect("a checkpointed run loads");
        assert_eq!(projected.id(), &run_id());
        assert_eq!(projected.reference_version().digest(), &digest());
        assert_eq!(
            projected.intent(),
            PROJECTION_INTENT,
            "the projection names itself instead of inventing an intent"
        );
        store.record_run(&run(), bound()).expect("records");
        let recorded = store
            .load_run(&run_id(), bound())
            .expect("loads")
            .expect("the recorded run loads");
        assert_eq!(recorded.intent(), "settle the task");
        assert_eq!(recorded.reference_version().version(), 3);
    }

    /// The PACKAGED §16.3 primitive, called directly (the correction round's
    /// reopening finding: this method had zero callers; the crash tests
    /// exercised only the helper's two-step re-implementation). Pins the
    /// whole contract at the API that ships: the returned offset is the
    /// settlement row's offset; the event at that offset is exactly the
    /// commit's own settlement event; replay afterwards adds nothing; and a
    /// second commit of the same key appends a second event (the log is
    /// append-only) while the keyed row count stays one.
    #[test]
    fn commit_settlement_is_the_spec_sequence_and_idempotent_by_key() {
        use crate::event_log::read_entries;
        let store = SqliteStateStore::open_in_memory().expect("store opens");
        let commit = SettlementCommit {
            run_id: run_id(),
            unit_id: UnitId::new("AK-900").expect("valid unit id"),
            evidence: EvidenceRef::new("evd-000900").expect("valid ref"),
            verification: VerificationRef::new("vrf-000900").expect("valid ref"),
            reference_digest: digest(),
            at: EpochSeconds::new(1_700_000_101),
        };
        let offset = store
            .commit_settlement(commit.clone(), bound())
            .expect("the packaged commit sequence runs");

        // The settlement row references exactly the returned offset.
        let rows = store.settlements(&run_id(), bound()).expect("reads");
        assert_eq!(rows.len(), 1, "exactly one settlement row");
        assert_eq!(
            rows[0].event_log_offset(),
            offset,
            "the row names the offset the commit returned"
        );

        // The event at that offset decodes to SettlementCommitted with
        // exactly the commit's fields.
        {
            let conn = store
                .conn
                .lock()
                .expect("the store connection is not poisoned");
            let entries = read_entries(&conn, EventLogOffset::new(0)).expect("the log replays");
            assert_eq!(entries.len(), 1, "one event, at the returned offset");
            assert_eq!(entries[0].offset, offset);
            assert_eq!(entries[0].event.run_id, commit.run_id);
            assert_eq!(entries[0].event.at, commit.at);
            match &entries[0].event.kind {
                RunEventKind::SettlementCommitted {
                    unit_id,
                    evidence,
                    verification,
                    reference_digest,
                } => {
                    assert_eq!(unit_id, &commit.unit_id);
                    assert_eq!(evidence, &commit.evidence);
                    assert_eq!(verification, &commit.verification);
                    assert_eq!(reference_digest, &commit.reference_digest);
                }
                other => panic!("the event must be a settlement, got {other:?}"),
            }
        }

        // Nothing is half-applied: replay adds zero.
        assert_eq!(
            store
                .complete_pending_settlements(&run_id(), bound())
                .expect("replay works"),
            0,
            "a completed commit needs no replay"
        );

        // A second commit of the same key appends a second event (the log is
        // append-only and never rewrites) but still one keyed row, and
        // replay still adds zero.
        let second = store
            .commit_settlement(commit, bound())
            .expect("the second commit runs");
        assert!(
            second.get() > offset.get(),
            "the append-only log grows; offsets never rewind"
        );
        assert_eq!(
            store.settlements(&run_id(), bound()).expect("reads").len(),
            1,
            "the (run, unit, verification) key still names one row"
        );
        assert_eq!(
            store
                .complete_pending_settlements(&run_id(), bound())
                .expect("replay works"),
            0,
            "replay after the duplicate-key commit adds nothing"
        );
    }
}
