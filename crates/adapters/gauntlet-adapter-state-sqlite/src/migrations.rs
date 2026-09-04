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
        assert_eq!(
            applied_versions(&conn).unwrap(),
            vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10]
        );
        apply(&conn).expect("an up-to-date database is a no-op");
        assert_eq!(
            applied_versions(&conn).unwrap(),
            vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10]
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
}
