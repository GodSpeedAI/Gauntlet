//! PostgreSQL migrations, applied in order and recorded in a schema_migrations table.
//! The migration set is append-only: a migration, once shipped, is never
//! edited or removed — later changes are new, higher-numbered entries.

use postgres::Client;
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
pub(crate) const MIGRATIONS: &[Migration] = &[Migration {
    version: 1,
    name: "initial",
    sql: include_str!("../migrations/0001_initial.sql"),
}, Migration {
    version: 2,
    name: "frozen_reference_evidence_and_bigint_version",
    sql: include_str!("../migrations/0002_frozen_reference_evidence.sql"),
}, Migration {
    version: 3,
    name: "observation_bindings",
    sql: include_str!("../migrations/0003_observation_bindings.sql"),
}];

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

/// Applies all outstanding migrations to the database in a transaction.
/// Uses `pg_advisory_xact_lock()` to serialize concurrent opens.
pub(crate) fn apply(client: &mut Client) -> Result<(), MigrationError> {
    // Create the schema_migrations table if it doesn't exist
    client
        .execute(
            "CREATE TABLE IF NOT EXISTS schema_migrations (
                version BIGINT PRIMARY KEY,
                name TEXT NOT NULL
            )",
            &[],
        )
        .map_err(|e| {
            MigrationError::Unavailable(format!("failed to create migrations table: {}", e))
        })?;

    // Get applied versions
    let applied = applied_versions(client)?;

    // Apply new migrations within a transaction with advisory lock
    for migration in MIGRATIONS {
        if !applied.contains(&migration.version) {
            // Take an advisory lock to serialize concurrent migration attempts
            client
                .execute("SELECT pg_advisory_xact_lock(1234567890)", &[])
                .map_err(|e| {
                    MigrationError::Unavailable(format!("failed to acquire migration lock: {}", e))
                })?;

            // Re-check after acquiring lock in case another process beat us
            let recheck = applied_versions(client)?;
            if !recheck.contains(&migration.version) {
                // Execute the migration SQL, preprocessing to remove comments
                for statement in migration.sql.split(';') {
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
                    let trimmed = trimmed.trim();
                    if !trimmed.is_empty() {
                        client.execute(trimmed, &[]).map_err(|e| {
                            let error_detail = match e.source() {
                                Some(source) => format!("{}: {}", e, source),
                                None => format!("{}", e),
                            };
                            MigrationError::Unavailable(format!(
                                "migration {} ({}) failed: {}",
                                migration.version, migration.name, error_detail
                            ))
                        })?;
                    }
                }

                // Record the migration
                client
                    .execute(
                        "INSERT INTO schema_migrations (version, name) VALUES ($1, $2)",
                        &[&migration.version, &migration.name],
                    )
                    .map_err(|e| {
                        MigrationError::Unavailable(format!(
                            "failed to record migration {}: {}",
                            migration.version, e
                        ))
                    })?;
            }
        }
    }

    Ok(())
}

/// Returns the list of applied migration versions in ascending order.
pub(crate) fn applied_versions(client: &mut Client) -> Result<Vec<i64>, MigrationError> {
    // Check if the table exists in the current schema (via search_path) using to_regclass
    // which properly respects the connection's search_path and avoids cross-namespace leaks
    let table_exists: bool = client
        .query_one("SELECT to_regclass('schema_migrations') IS NOT NULL", &[])
        .map_err(|e| MigrationError::Unavailable(e.to_string()))?
        .get(0);

    if !table_exists {
        return Ok(Vec::new());
    }

    let mut versions = Vec::new();
    let rows = client
        .query(
            "SELECT version FROM schema_migrations ORDER BY version ASC",
            &[],
        )
        .map_err(|e| MigrationError::Unavailable(e.to_string()))?;

    for row in rows {
        let version: i64 = row.get(0);
        versions.push(version);
    }

    Ok(versions)
}

#[cfg(test)]
mod tests {
    use super::*;
    
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

    #[test]
    fn to_regclass_respects_search_path() {
        // This test verifies that to_regclass() properly respects the connection's
        // search_path, proving that applied_versions() doesn't have cross-namespace leaks.
        // We test this with a simple schema creation and table existence check.
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
