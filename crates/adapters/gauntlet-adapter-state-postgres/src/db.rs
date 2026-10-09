//! PostgreSQL database configuration and connection management.
//!
//! This module provides database configuration, connection setup with schema
//! isolation via PostgreSQL namespaces, and utilities for placeholder
//! rewriting (SQLite ? → PostgreSQL $1, $2, ...) to keep ported SQL close
//! to the original.

use postgres::{Client, NoTls};

/// PostgreSQL configuration for a Gauntlet instance.
#[derive(Clone, Debug)]
pub struct PostgresConfig {
    /// Connection URL (e.g., "postgresql://user:pass@host/dbname")
    pub url: String,
    /// Namespace (PostgreSQL schema name) for this instance.
    /// Isolation allows multiple independent Gauntlet instances per server.
    pub namespace: String,
}

/// Default PostgreSQL connection URL for local development.
/// Assumes PostgreSQL 18 running at 127.0.0.1:54329.
pub const LOCAL_DEFAULT_URL: &str = "postgresql://postgres:postgres@127.0.0.1:54329/postgres";

/// Generates a stable FNV-1a 64-bit hash of a canonicalized path.
/// Used to derive namespace names from directory paths.
/// Returns a lowercase hex string (16 characters).
#[cfg(test)]
pub fn fnv1a_hash(input: &[u8]) -> String {
    const FNV_PRIME: u64 = 1099511628211;
    const FNV_OFFSET: u64 = 14695981039346656037;

    let mut hash = FNV_OFFSET;
    for &byte in input {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }

    format!("{:x}", hash)
}

/// Validates a PostgreSQL identifier (schema/table name).
/// Rejects injection attempts like `x"; DROP SCHEMA public;--`.
/// Valid identifiers: start with a-z or _, contain only a-z0-9_,
/// max 63 characters (PostgreSQL limit).
pub fn validate_identifier(name: &str) -> Result<(), String> {
    if name.is_empty() || name.len() > 63 {
        return Err(format!("identifier length must be 1-63 chars: {}", name));
    }
    if !name.chars().next().unwrap_or('x').is_ascii_lowercase() && !name.starts_with('_') {
        return Err(format!("identifier must start with [a-z_]: {}", name));
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    {
        return Err(format!("identifier must contain only [a-z0-9_]: {}", name));
    }
    Ok(())
}

/// The `application_name` a connection of the namespace `namespace` opened by
/// the process `pid` carries: `gauntlet:<namespace>:<pid>`. The pid makes a
/// process's own connections distinguishable from another process's.
pub fn application_name(namespace: &str, pid: u32) -> String {
    format!("gauntlet:{}:{}", namespace, pid)
}

/// Establishes a PostgreSQL connection and initializes the namespace (schema).
///
/// Steps:
/// 1. Connect using the provided URL with NoTls.
/// 2. Validate the namespace identifier.
/// 3. Create the schema if it doesn't exist.
/// 4. Set search_path to use the namespace.
///
/// # Errors
/// Returns an error if connection fails, identifier validation fails,
/// or schema creation fails.
pub fn connect(config: &PostgresConfig) -> Result<Client, String> {
    validate_identifier(&config.namespace)?;

    // Tag every backend with its namespace so a state instance's live
    // connections are identifiable in `pg_stat_activity` (see
    // `test_support::quiesce_writers`).
    let mut pg_config: postgres::Config = config
        .url
        .parse()
        .map_err(|e| format!("failed to connect to database: {}", e))?;
    pg_config.application_name(&application_name(&config.namespace, std::process::id()));
    let mut client = pg_config
        .connect(NoTls)
        .map_err(|e| format!("failed to connect to database: {}", e))?;

    // Create schema if not exists. `CREATE SCHEMA IF NOT EXISTS` is not safe
    // under concurrency (two sessions can both pass the existence check and one
    // raises unique_violation), so it runs under the same advisory lock that
    // serializes migrations. The transaction ends before `apply` starts, so the
    // two never nest.
    let create_schema_sql = format!("CREATE SCHEMA IF NOT EXISTS \"{}\"", config.namespace);
    let mut tx = client
        .transaction()
        .map_err(|e| format!("failed to create schema: {}", e))?;
    tx.execute(
        "SELECT pg_advisory_xact_lock($1)",
        &[&crate::migrations::MIGRATION_LOCK_KEY],
    )
    .map_err(|e| format!("failed to create schema: {}", e))?;
    tx.execute(&create_schema_sql, &[])
        .map_err(|e| format!("failed to create schema: {}", e))?;
    tx.commit()
        .map_err(|e| format!("failed to create schema: {}", e))?;

    // Set search_path to the namespace
    let set_path_sql = format!("SET search_path TO \"{}\"", config.namespace);
    client
        .execute(&set_path_sql, &[])
        .map_err(|e| format!("failed to set search_path: {}", e))?;

    Ok(client)
}

/// Rewrites SQLite-style placeholders (? or ?1, ?2, ...) to PostgreSQL style ($1, $2, ...).
///
/// This allows ported SQL from SQLite to work in PostgreSQL with minimal changes.
///
/// Rules:
/// - ?N (numbered) → $N
/// - ? (positional) → $1, $2, ... (in order)
/// - Does NOT rewrite ? inside quoted string literals
///
/// # Examples
/// - "SELECT * FROM t WHERE id = ?" → "SELECT * FROM t WHERE id = $1"
/// - "SELECT * FROM t WHERE id = ?1 AND name = ?2" → unchanged (already correct)
#[cfg(test)]
pub fn rewrite_placeholders(sql: &str) -> Result<String, String> {
    let mut result = String::new();
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut positional_count = 0;
    let chars: Vec<char> = sql.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let ch = chars[i];

        // Track quote state
        match ch {
            '\'' if !in_double_quote => {
                in_single_quote = !in_single_quote;
                result.push(ch);
                i += 1;
            }
            '"' if !in_single_quote => {
                in_double_quote = !in_double_quote;
                result.push(ch);
                i += 1;
            }
            '?' if !in_single_quote && !in_double_quote => {
                // Check if it's ?N format
                if i + 1 < chars.len() && chars[i + 1].is_ascii_digit() {
                    // ?N format: already in PostgreSQL style, just replace ? with $
                    result.push('$');
                    i += 1;
                } else {
                    // Positional ?: replace with $N
                    positional_count += 1;
                    result.push_str(&format!("${}", positional_count));
                    i += 1;
                }
            }
            _ => {
                result.push(ch);
                i += 1;
            }
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fnv1a_is_stable() {
        let path = "/home/user/gauntlet";
        let hash1 = fnv1a_hash(path.as_bytes());
        let hash2 = fnv1a_hash(path.as_bytes());
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn placeholder_rewriter_positional() {
        assert_eq!(
            rewrite_placeholders("SELECT * WHERE id = ?").unwrap(),
            "SELECT * WHERE id = $1"
        );
        assert_eq!(
            rewrite_placeholders("SELECT * WHERE id = ? AND name = ?").unwrap(),
            "SELECT * WHERE id = $1 AND name = $2"
        );
    }

    #[test]
    fn placeholder_rewriter_numbered() {
        assert_eq!(
            rewrite_placeholders("SELECT * WHERE id = ?1").unwrap(),
            "SELECT * WHERE id = $1"
        );
        assert_eq!(
            rewrite_placeholders("SELECT * WHERE id = ?1 AND name = ?2").unwrap(),
            "SELECT * WHERE id = $1 AND name = $2"
        );
    }

    #[test]
    fn placeholder_rewriter_respects_quotes() {
        assert_eq!(
            rewrite_placeholders("SELECT '?' WHERE id = ?").unwrap(),
            "SELECT '?' WHERE id = $1"
        );
        assert_eq!(
            rewrite_placeholders("SELECT '?' || id FROM t WHERE x = ?").unwrap(),
            "SELECT '?' || id FROM t WHERE x = $1"
        );
    }

    #[test]
    fn identifier_validation() {
        assert!(validate_identifier("gauntlet_123").is_ok());
        assert!(validate_identifier("_private").is_ok());
        assert!(validate_identifier("event_log").is_ok());

        // Rejects uppercase
        assert!(validate_identifier("MyTable").is_err());
        // Rejects special chars
        assert!(validate_identifier("table-name").is_err());
        // Rejects injection
        assert!(validate_identifier("x\"; DROP SCHEMA").is_err());
        // Rejects empty or too long
        assert!(validate_identifier("").is_err());
        assert!(validate_identifier(&"a".repeat(64)).is_err());
    }
}
