//! In-repo migrations, applied in order and recorded in a migrations table
//! (plan Task 6). The set is append-only: a migration, once shipped, is never
//! edited or removed — later changes are new, higher-numbered files.

use rusqlite::Connection;

/// One migration: its number, its name, and the SQL it runs. The SQL files
/// live beside this crate as ordinary reviewable sources.
pub(crate) struct Migration {
    pub version: u32,
    pub name: &'static str,
    pub sql: &'static str,
}

/// The migration set, in the order it is applied. Append-only: entries are
/// never reordered, edited, or removed.
pub(crate) const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "event_log",
        sql: include_str!("../migrations/0001_event_log.sql"),
    },
    Migration {
        version: 2,
        name: "state_records",
        sql: include_str!("../migrations/0002_state_records.sql"),
    },
    Migration {
        version: 3,
        name: "critic_bindings",
        sql: include_str!("../migrations/0003_critic_bindings.sql"),
    },
    Migration {
        version: 4,
        name: "frozen_reference_evidence",
        sql: include_str!("../migrations/0004_frozen_reference_evidence.sql"),
    },
    Migration {
        version: 5,
        name: "observation_bindings",
        sql: include_str!("../migrations/0005_observation_bindings.sql"),
    },
    Migration {
        version: 6,
        name: "evidence_productions",
        sql: include_str!("../migrations/0006_evidence_productions.sql"),
    },
    Migration {
        version: 7,
        name: "evidence_production_content",
        sql: include_str!("../migrations/0007_evidence_production_content.sql"),
    },
    Migration {
        version: 8,
        name: "correction_selection_records",
        sql: include_str!("../migrations/0008_correction_selection_records.sql"),
    },
    Migration {
        version: 9,
        name: "capability_ledger",
        sql: include_str!("../migrations/0009_capability_ledger.sql"),
    },
    Migration {
        version: 10,
        name: "revision_ledger",
        sql: include_str!("../migrations/0010_revision_ledger.sql"),
    },
    Migration {
        version: 11,
        name: "evidence_productions_per_iteration",
        sql: include_str!("../migrations/0011_evidence_productions_per_iteration.sql"),
    },
];

/// Why a migration could not be applied — typed, carrying the version.
#[derive(Debug)]
pub(crate) enum MigrationError {
    /// Reading or writing the migrations table failed.
    Bookkeeping { version: u32, detail: String },
    /// The migration's SQL failed.
    Apply {
        version: u32,
        name: &'static str,
        detail: String,
    },
}

impl core::fmt::Display for MigrationError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Bookkeeping { version, detail } => {
                write!(f, "migration {version} bookkeeping failed: {detail}")
            }
            Self::Apply {
                version,
                name,
                detail,
            } => {
                write!(f, "migration {version} ({name}) failed: {detail}")
            }
        }
    }
}

/// Applies every migration not yet recorded, in ascending version order, each
/// together with its bookkeeping row in one transaction. Opening an
/// up-to-date database applies nothing.
pub(crate) fn apply(conn: &Connection) -> Result<(), MigrationError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version INTEGER PRIMARY KEY,
            name    TEXT NOT NULL
        );",
    )
    .map_err(|err| MigrationError::Bookkeeping {
        version: 0,
        detail: format!("creating schema_migrations failed: {err}"),
    })?;
    for migration in MIGRATIONS {
        let applied: bool =
            conn.query_row(
                "SELECT COUNT(*) FROM schema_migrations WHERE version = ?1",
                [u64::from(migration.version)],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|err| MigrationError::Bookkeeping {
                version: migration.version,
                detail: format!("reading the migrations table failed: {err}"),
            })? > 0;
        if applied {
            continue;
        }
        conn.execute_batch("BEGIN IMMEDIATE;")
            .map_err(|err| MigrationError::Apply {
                version: migration.version,
                name: migration.name,
                detail: format!("beginning the transaction failed: {err}"),
            })?;
        let outcome = conn.execute_batch(migration.sql).and_then(|_| {
            conn.execute(
                "INSERT INTO schema_migrations (version, name) VALUES (?1, ?2)",
                (u64::from(migration.version), migration.name),
            )
            .map(|_| ())
        });
        match outcome {
            Ok(()) => {
                conn.execute_batch("COMMIT;")
                    .map_err(|err| MigrationError::Apply {
                        version: migration.version,
                        name: migration.name,
                        detail: format!("committing the migration failed: {err}"),
                    })?;
            }
            Err(err) => {
                // Roll back so a failed migration leaves no half-applied set.
                conn.execute_batch("ROLLBACK;")
                    .map_err(|rollback| MigrationError::Apply {
                        version: migration.version,
                        name: migration.name,
                        detail: format!(
                            "{err}; rolling back failed too: {rollback} — the database needs \
                             forensics, not a retry"
                        ),
                    })?;
                return Err(MigrationError::Apply {
                    version: migration.version,
                    name: migration.name,
                    detail: err.to_string(),
                });
            }
        }
    }
    Ok(())
}

/// The versions recorded as applied, ascending — for the migration tests.
#[cfg(test)]
pub(crate) fn applied_versions(conn: &Connection) -> Result<Vec<u32>, MigrationError> {
    let mut statement = conn
        .prepare("SELECT version FROM schema_migrations ORDER BY version")
        .map_err(|err| MigrationError::Bookkeeping {
            version: 0,
            detail: format!("reading the migrations table failed: {err}"),
        })?;
    let versions = statement
        .query_map([], |row| {
            u32::try_from(row.get::<_, i64>(0)?)
                .map_err(|err| rusqlite::Error::ToSqlConversionFailure(Box::new(err)))
        })
        .map_err(|err| MigrationError::Bookkeeping {
            version: 0,
            detail: err.to_string(),
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| MigrationError::Bookkeeping {
            version: 0,
            detail: err.to_string(),
        })?;
    Ok(versions)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrations_apply_in_order_and_reopening_applies_nothing_new() {
        let conn = Connection::open_in_memory().expect("in-memory database opens");
        apply(&conn).expect("the set applies");
        // 0008 (correction-selection records) widened the records CHECK by
        // rebuilding the table — a recorded act, pinned here with the rest.
        // 0009 (capability ledger) added the two §19.1 tables.
        // 0010 (revision ledger) added the four §19.3/§19.4 tables.
        // 0011 (task26) widened evidence_productions' primary key to
        // (run, evidence ref, iteration) by rebuilding the table.
        assert_eq!(
            applied_versions(&conn).unwrap(),
            vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]
        );
        apply(&conn).expect("an up-to-date database is a no-op");
        assert_eq!(
            applied_versions(&conn).unwrap(),
            vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]
        );
        // Every migration in the set is numbered strictly ascending, and the
        // versions recorded are exactly the set's versions — the append-only
        // property pinned mechanically.
        let mut previous = 0;
        for migration in MIGRATIONS {
            assert!(
                migration.version > previous,
                "migration {} is not after {} — the set is append-only and ordered",
                migration.version,
                previous
            );
            previous = migration.version;
        }
        assert_eq!(
            applied_versions(&conn).unwrap(),
            MIGRATIONS.iter().map(|m| m.version).collect::<Vec<_>>()
        );
    }

    #[test]
    fn migration_0011_preserves_pre_existing_production_rows_byte_identical() {
        // A database exactly as it stood before 0011 shipped: migrations
        // 1-10 applied, one production row written under the OLD (run,
        // evidence_ref) primary key.
        let conn = Connection::open_in_memory().expect("in-memory database opens");
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_migrations (
                version INTEGER PRIMARY KEY,
                name    TEXT NOT NULL
            );",
        )
        .expect("the bookkeeping table creates");
        for migration in &MIGRATIONS[..10] {
            conn.execute_batch(migration.sql)
                .expect("a pre-0011 migration applies");
            conn.execute(
                "INSERT INTO schema_migrations (version, name) VALUES (?1, ?2)",
                (u64::from(migration.version), migration.name),
            )
            .expect("bookkeeping row inserts");
        }
        conn.execute(
            "INSERT INTO evidence_productions (
                run_id, evidence_ref, producer_name, producer_version, locator, iteration_no,
                produced_at, event_log_offset, content_digest
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            (
                "01ARZ3NDEKTSV4RRFFQ69G5FAV",
                "evd-pre-0011",
                "suite-instrument",
                "1",
                "suite-locator.json",
                1_i64,
                1_700_000_000_i64,
                1_i64,
                "sha256:deadbeef",
            ),
        )
        .expect("the pre-0011 row inserts under the old (run, ref) primary key");

        // Bring the database up to date: 0011 rebuilds the table.
        apply(&conn).expect("0011 applies over the pre-existing row");
        assert_eq!(
            applied_versions(&conn).unwrap(),
            MIGRATIONS.iter().map(|m| m.version).collect::<Vec<_>>()
        );

        let row: (
            String,
            String,
            String,
            String,
            String,
            i64,
            i64,
            i64,
            String,
        ) = conn
            .query_row(
                "SELECT run_id, evidence_ref, producer_name, producer_version, locator, \
                 iteration_no, produced_at, event_log_offset, content_digest \
                 FROM evidence_productions WHERE evidence_ref = ?1",
                ["evd-pre-0011"],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                        row.get(7)?,
                        row.get(8)?,
                    ))
                },
            )
            .expect("the pre-0011 row reads back after the migration");
        assert_eq!(
            row,
            (
                "01ARZ3NDEKTSV4RRFFQ69G5FAV".to_owned(),
                "evd-pre-0011".to_owned(),
                "suite-instrument".to_owned(),
                "1".to_owned(),
                "suite-locator.json".to_owned(),
                1,
                1_700_000_000,
                1,
                "sha256:deadbeef".to_owned(),
            ),
            "the pre-existing row survives 0011 byte-identical"
        );

        // The widened key now accepts a SECOND row for the SAME ref at a
        // LATER iteration — exactly what 0011 exists to allow.
        conn.execute(
            "INSERT INTO evidence_productions (
                run_id, evidence_ref, producer_name, producer_version, locator, iteration_no,
                produced_at, event_log_offset, content_digest
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            (
                "01ARZ3NDEKTSV4RRFFQ69G5FAV",
                "evd-pre-0011",
                "suite-instrument",
                "1",
                "suite-locator.json",
                2_i64,
                1_700_000_100_i64,
                2_i64,
                "sha256:deadbeef",
            ),
        )
        .expect("a later-iteration row for the same ref inserts under the widened key");
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM evidence_productions WHERE evidence_ref = ?1",
                ["evd-pre-0011"],
                |row| row.get(0),
            )
            .expect("count reads");
        assert_eq!(count, 2, "both iterations' rows stand side by side");
    }
}
