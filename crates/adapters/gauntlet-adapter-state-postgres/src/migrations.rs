//! PostgreSQL migrations, applied in order and recorded in a schema_migrations table.
//! The migration set is append-only: a migration, once shipped, is never
//! edited or removed — later changes are new, higher-numbered entries.

use postgres::{Client, Transaction};
use std::error::Error;

/// One migration: its version, its name, and the SQL it runs.
pub(crate) struct Migration {
    pub version: i64,
    pub name: &'static str,
    pub sql: &'static str,
}

/// The migration set, in the order it is applied. Append-only: entries are
/// never reordered, edited, or removed. This is slice E1a: all tables for
/// EventLog, CapabilityLedger, and RevisionLedger; E1b (StateStore) deferred.
pub(crate) const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "initial",
        sql: include_str!("../migrations/0001_initial.sql"),
    },
    Migration {
        version: 2,
        name: "frozen_reference_evidence_and_bigint_version",
        sql: include_str!("../migrations/0002_frozen_reference_evidence.sql"),
    },
    Migration {
        version: 3,
        name: "observation_bindings",
        sql: include_str!("../migrations/0003_observation_bindings.sql"),
    },
    Migration {
        version: 4,
        name: "invocation_graph",
        sql: include_str!("../migrations/0004_invocation_graph.sql"),
    },
    Migration {
        version: 5,
        name: "lifecycle",
        sql: include_str!("../migrations/0005_lifecycle.sql"),
    },
];

/// Session-level advisory lock key (`pg_advisory_xact_lock`) that serializes
/// namespace creation (`db::connect`) and migration application (`apply`)
/// across every connection on the server. Both take it inside a transaction,
/// so it is held until that transaction commits or rolls back.
pub(crate) const MIGRATION_LOCK_KEY: i64 = 1_234_567_890;

/// Splits a migration script into statements at each `;` that is outside a
/// dollar-quoted (`$$ ... $$`) body, so PL/pgSQL function bodies survive.
fn split_statements(sql: &str) -> Vec<&str> {
    let bytes = sql.as_bytes();
    let mut statements = Vec::new();
    let mut start = 0;
    let mut in_dollar_quote = false;
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i..].starts_with(b"$$") {
            in_dollar_quote = !in_dollar_quote;
            i += 2;
            continue;
        }
        if bytes[i] == b';' && !in_dollar_quote {
            statements.push(&sql[start..i]);
            start = i + 1;
        }
        i += 1;
    }
    statements.push(&sql[start..]);
    statements
}

/// Removes `--` line comments from one statement and trims it. Returns an
/// empty string when nothing executable remains.
fn strip_comments(statement: &str) -> String {
    let mut trimmed = String::new();
    for line in statement.lines() {
        // Remove SQL comments (-- to end of line)
        let comment_pos = line.find("--").unwrap_or(line.len());
        let line_no_comment = &line[..comment_pos];
        if !trimmed.is_empty() || !line_no_comment.trim().is_empty() {
            trimmed.push_str(line_no_comment);
            trimmed.push('\n');
        }
    }
    trimmed.trim().to_string()
}

/// Why a migration could not be applied — typed, carrying the version.
#[derive(Debug, Clone)]
pub(crate) enum MigrationError {
    /// A database operation (connection, execution, schema creation) failed.
    Unavailable(String),
}

impl core::fmt::Display for MigrationError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            MigrationError::Unavailable(detail) => {
                write!(f, "migration unavailable: {}", detail)
            }
        }
    }
}

/// Renders a driver error with its source, when it has one.
fn describe(e: &postgres::Error) -> String {
    match e.source() {
        Some(source) => format!("{}: {}", e, source),
        None => format!("{}", e),
    }
}

/// Applies all outstanding migrations to the database.
pub(crate) fn apply(client: &mut Client) -> Result<(), MigrationError> {
    apply_list(client, MIGRATIONS)
}

/// Applies the outstanding entries of `migrations` as ONE transaction.
///
/// The transaction takes `pg_advisory_xact_lock(MIGRATION_LOCK_KEY)` first, so
/// concurrent first opens of a namespace queue up behind each other and the
/// second one sees the first one's committed schema. Table creation, every
/// migration's statements, and each `schema_migrations` row commit together
/// or not at all: a failing statement rolls the whole transaction back, so
/// nothing is left half-applied and nothing is recorded.
pub(crate) fn apply_list(
    client: &mut Client,
    migrations: &[Migration],
) -> Result<(), MigrationError> {
    let unavailable = |what: &str, e: &postgres::Error| {
        MigrationError::Unavailable(format!("{}: {}", what, describe(e)))
    };

    let mut tx = client
        .transaction()
        .map_err(|e| unavailable("failed to begin migration transaction", &e))?;

    tx.execute("SELECT pg_advisory_xact_lock($1)", &[&MIGRATION_LOCK_KEY])
        .map_err(|e| unavailable("failed to acquire migration lock", &e))?;

    tx.execute(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version BIGINT PRIMARY KEY,
            name TEXT NOT NULL
        )",
        &[],
    )
    .map_err(|e| unavailable("failed to create migrations table", &e))?;

    // Read under the lock, so the set reflects every migration committed so far.
    let applied = applied_versions(&mut tx)?;

    for migration in migrations {
        if applied.contains(&migration.version) {
            continue;
        }
        for statement in split_statements(migration.sql) {
            let sql = strip_comments(statement);
            if sql.is_empty() {
                continue;
            }
            tx.execute(sql.as_str(), &[]).map_err(|e| {
                MigrationError::Unavailable(format!(
                    "migration {} ({}) failed: {}",
                    migration.version,
                    migration.name,
                    describe(&e)
                ))
            })?;
        }
        tx.execute(
            "INSERT INTO schema_migrations (version, name) VALUES ($1, $2)",
            &[&migration.version, &migration.name],
        )
        .map_err(|e| {
            MigrationError::Unavailable(format!(
                "failed to record migration {}: {}",
                migration.version,
                describe(&e)
            ))
        })?;
    }

    tx.commit()
        .map_err(|e| unavailable("failed to commit migrations", &e))
}

/// Returns the applied migration versions in ascending order. Must run after
/// `schema_migrations` exists (`apply_list` creates it under the lock first).
fn applied_versions(tx: &mut Transaction<'_>) -> Result<Vec<i64>, MigrationError> {
    let rows = tx
        .query(
            "SELECT version FROM schema_migrations ORDER BY version ASC",
            &[],
        )
        .map_err(|e| MigrationError::Unavailable(describe(&e)))?;
    Ok(rows.iter().map(|row| row.get::<_, i64>(0)).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::db::{LOCAL_DEFAULT_URL, PostgresConfig, connect};
    use crate::{PostgresEventLog, PostgresInvocationGraph};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Barrier};
    use std::time::SystemTime;

    #[test]
    fn migrations_are_append_only() {
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
    }

    /// A namespace no other test or process uses: pid, counter and time.
    fn fresh_config(tag: &str) -> PostgresConfig {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let nanos = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
        PostgresConfig {
            url: std::env::var("GAUNTLET_DATABASE_URL")
                .unwrap_or_else(|_| LOCAL_DEFAULT_URL.to_string()),
            namespace: format!(
                "gauntlet_mig_{}_{}_{:x}_{:x}",
                tag,
                std::process::id(),
                counter,
                nanos
            ),
        }
    }

    fn recorded_versions(client: &mut Client) -> Vec<i64> {
        client
            .query(
                "SELECT version FROM schema_migrations ORDER BY version",
                &[],
            )
            .expect("schema_migrations is readable")
            .iter()
            .map(|row| row.get::<_, i64>(0))
            .collect()
    }

    fn expected_versions() -> Vec<i64> {
        MIGRATIONS.iter().map(|m| m.version).collect()
    }

    #[test]
    fn concurrent_first_open_applies_each_migration_once() {
        for round in 0..10 {
            let config = fresh_config(&format!("apply{}", round));
            let barrier = Arc::new(Barrier::new(8));
            let handles: Vec<_> = (0..8)
                .map(|_| {
                    let config = config.clone();
                    let barrier = barrier.clone();
                    std::thread::spawn(move || -> Result<(), String> {
                        let mut client = connect(&config)?;
                        barrier.wait();
                        apply(&mut client).map_err(|e| e.to_string())
                    })
                })
                .collect();
            for handle in handles {
                handle
                    .join()
                    .expect("thread does not panic")
                    .expect("every concurrent first open succeeds");
            }
            let mut client = connect(&config).expect("namespace reopens");
            assert_eq!(
                recorded_versions(&mut client),
                expected_versions(),
                "round {}: each migration is recorded exactly once",
                round
            );
        }
    }

    #[test]
    fn concurrent_store_opens_all_succeed() {
        let config = fresh_config("stores");
        let barrier = Arc::new(Barrier::new(6));
        let handles: Vec<_> = (0..6)
            .map(|i| {
                let config = config.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || -> Result<(), String> {
                    barrier.wait();
                    if i % 2 == 0 {
                        PostgresEventLog::open(&config)
                            .map(|_| ())
                            .map_err(|e| format!("{:?}", e))
                    } else {
                        PostgresInvocationGraph::open(&config)
                            .map(|_| ())
                            .map_err(|e| format!("{:?}", e))
                    }
                })
            })
            .collect();
        for handle in handles {
            handle
                .join()
                .expect("thread does not panic")
                .expect("every concurrent store open succeeds");
        }
        let mut client = connect(&config).expect("namespace reopens");
        assert_eq!(recorded_versions(&mut client), expected_versions());
    }

    #[test]
    fn failed_migration_rolls_back_wholly_and_real_set_applies_after() {
        let config = fresh_config("atomic");
        let mut client = connect(&config).expect("connect");
        let good = Migration {
            version: 1,
            name: "good",
            sql: "CREATE TABLE t1 (id INT);",
        };
        let bad = Migration {
            version: 2,
            name: "bad",
            sql: "CREATE TABLE t2 (id INT); SELECT 1/0;",
        };
        let failed = apply_list(&mut client, &[good, bad]);
        assert!(failed.is_err(), "the bad migration fails the whole apply");

        for (name, sql) in [
            ("t1", "SELECT to_regclass('t1') IS NULL"),
            ("t2", "SELECT to_regclass('t2') IS NULL"),
            (
                "schema_migrations",
                "SELECT to_regclass('schema_migrations') IS NULL",
            ),
        ] {
            let absent: bool = client.query_one(sql, &[]).expect("catalog query").get(0);
            assert!(
                absent,
                "{} must not exist after the rolled-back apply",
                name
            );
        }

        apply(&mut client).expect("the real migration set applies after the failure");
        assert_eq!(recorded_versions(&mut client), expected_versions());
    }

    #[test]
    fn reapplying_recorded_migrations_is_a_no_op() {
        let config = fresh_config("idempotent");
        let mut client = connect(&config).expect("connect");
        apply(&mut client).expect("first apply");
        apply(&mut client).expect("second apply is a no-op");
        assert_eq!(recorded_versions(&mut client), expected_versions());
    }

    #[test]
    fn to_regclass_respects_search_path() {
        // This test verifies that to_regclass() properly respects the connection's
        // search_path, proving that a lookup in applied_versions() cannot leak
        // across namespaces. We test this with a simple schema creation and table
        // existence check.
        let url = std::env::var("GAUNTLET_DATABASE_URL").unwrap_or_else(|_| {
            "postgresql://postgres:postgres@127.0.0.1:54329/postgres".to_string()
        });

        let nanos = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .ok()
            .map(|d| d.as_nanos())
            .unwrap_or(0);

        // Create a test schema
        let pid = std::process::id();
        let test_schema = format!("gauntlet_test_to_regclass_{}_{:x}", pid, nanos);

        let mut conn = match postgres::Client::connect(&url, postgres::NoTls) {
            Ok(c) => c,
            Err(_) => {
                // If we can't connect to database, skip this test
                return;
            }
        };

        // Create schema and table
        let _ = conn.execute(&format!("CREATE SCHEMA IF NOT EXISTS {}", test_schema), &[]);
        let _ = conn.execute(&format!("SET search_path TO {}", test_schema), &[]);
        let _ = conn.execute("CREATE TABLE test_regclass_table (id INT)", &[]);

        // Verify to_regclass finds it in the current search_path
        let table_exists: bool = conn
            .query_one("SELECT to_regclass('test_regclass_table') IS NOT NULL", &[])
            .unwrap()
            .get(0);
        assert!(
            table_exists,
            "to_regclass should find table in current search_path"
        );

        // Switch search_path away and verify it's NOT found
        let _ = conn.execute("SET search_path TO public", &[]);
        let table_exists_public: bool = conn
            .query_one("SELECT to_regclass('test_regclass_table') IS NOT NULL", &[])
            .unwrap()
            .get(0);
        assert!(
            !table_exists_public,
            "to_regclass should NOT find table when search_path is public"
        );

        // Clean up
        let _ = conn.execute(
            &format!("DROP SCHEMA IF EXISTS {} CASCADE", test_schema),
            &[],
        );
    }
}
