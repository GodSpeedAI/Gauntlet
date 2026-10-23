//! State-instance locators (slice E1d-1).
//!
//! The composition root still names a state instance by a PATH under the
//! state directory (`<state_dir>/gauntlet-state.db`). No file is created at
//! that path: the path is an identity. This module maps it to a PostgreSQL
//! namespace (schema) via a stable hash of the lexically normalized absolute
//! path, and takes the server URL from `GAUNTLET_DATABASE_URL` (default
//! [`LOCAL_DEFAULT_URL`]).

use std::path::{Component, Path, PathBuf};

use postgres::{Client, NoTls};

use crate::db::{LOCAL_DEFAULT_URL, PostgresConfig, validate_identifier};
use gauntlet_ports::state_store::StateStoreError;

/// The environment variable naming the PostgreSQL server URL.
pub const DATABASE_URL_ENV: &str = "GAUNTLET_DATABASE_URL";

/// Prefix of every namespace this module derives from a state path.
const NAMESPACE_PREFIX: &str = "gauntlet_";

/// FNV-1a 64-bit hash: stable across processes, platforms and Rust versions
/// (unlike `DefaultHasher`), which the namespace identity requires.
fn fnv1a_64(bytes: &[u8]) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut hash = FNV_OFFSET;
    for &byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

/// Lexically normalizes an absolute path: `.` components vanish, `..`
/// components pop the previous normal component. Nothing touches the disk.
fn lexically_normalized(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => match out.components().next_back() {
                Some(Component::Normal(_)) => {
                    out.pop();
                }
                // `/..` is `/`; a relative leftover `..` is kept as written.
                Some(Component::RootDir | Component::Prefix(_)) => {}
                _ => out.push(".."),
            },
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// The identity of a state path, made absolute and normalized WITHOUT
/// requiring the path to exist. When the parent directory exists it is
/// canonicalized (resolving symlinks in the directory part) and the file name
/// re-joined; otherwise the lexically normalized absolute path is used as is.
fn identity_path(path: &Path) -> PathBuf {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        match std::env::current_dir() {
            Ok(cwd) => cwd.join(path),
            Err(_) => path.to_path_buf(),
        }
    };
    let normalized = lexically_normalized(&absolute);
    if let (Some(parent), Some(file_name)) = (normalized.parent(), normalized.file_name())
        && let Ok(canonical_parent) = parent.canonicalize()
    {
        return canonical_parent.join(file_name);
    }
    normalized
}

/// The PostgreSQL namespace (schema) of the state instance at `path`:
/// `gauntlet_` followed by 16 lowercase hex digits of the FNV-1a-64 hash of
/// the identity path. Always a valid identifier (see `validate_identifier`).
pub fn namespace_for_path(path: &Path) -> String {
    let identity = identity_path(path);
    let bytes = identity.as_os_str().as_encoded_bytes();
    format!("{NAMESPACE_PREFIX}{:016x}", fnv1a_64(bytes))
}

/// The server URL: `GAUNTLET_DATABASE_URL` when set and non-empty, else
/// [`LOCAL_DEFAULT_URL`].
fn database_url() -> String {
    match std::env::var(DATABASE_URL_ENV) {
        Ok(url) if !url.trim().is_empty() => url,
        _ => LOCAL_DEFAULT_URL.to_owned(),
    }
}

impl PostgresConfig {
    /// The configuration of the state instance at `path`: the server URL from
    /// the environment (or the local default) and the path's namespace.
    pub fn for_state_path(path: &Path) -> PostgresConfig {
        PostgresConfig {
            url: database_url(),
            namespace: namespace_for_path(path),
        }
    }
}

/// Whether the state instance at `path` exists: its namespace schema exists
/// AND has at least one applied migration. Never creates anything.
///
/// # Errors
/// [`StateStoreError::ReadFailed`] when the database cannot be reached or the
/// catalog cannot be read. An unavailable database is NOT reported as `false`.
pub fn state_exists(path: &Path) -> Result<bool, StateStoreError> {
    let config = PostgresConfig::for_state_path(path);
    let read_failed = |detail: String| StateStoreError::ReadFailed { detail };
    validate_identifier(&config.namespace).map_err(read_failed)?;
    let mut client = Client::connect(&config.url, NoTls)
        .map_err(|error| read_failed(format!("failed to connect to PostgreSQL: {error}")))?;
    schema_has_migrations(&mut client, &config.namespace).map_err(read_failed)
}

/// Catalog check on an open client: the schema exists, holds a
/// `schema_migrations` table, and that table records at least one version.
fn schema_has_migrations(client: &mut Client, namespace: &str) -> Result<bool, String> {
    let schema_exists: bool = client
        .query_one(
            "SELECT EXISTS (SELECT 1 FROM pg_namespace WHERE nspname = $1)",
            &[&namespace],
        )
        .map_err(|error| format!("failed to read the schema catalog: {error}"))?
        .get(0);
    if !schema_exists {
        return Ok(false);
    }
    let table_exists: bool = client
        .query_one(
            "SELECT EXISTS (SELECT 1 FROM pg_class c \
             JOIN pg_namespace n ON n.oid = c.relnamespace \
             WHERE n.nspname = $1 AND c.relname = 'schema_migrations' AND c.relkind = 'r')",
            &[&namespace],
        )
        .map_err(|error| format!("failed to read the table catalog: {error}"))?
        .get(0);
    if !table_exists {
        return Ok(false);
    }
    // The namespace is a validated identifier (checked by the caller), so it
    // is safe to spell as a quoted identifier.
    let applied: i64 = client
        .query_one(
            &format!("SELECT COUNT(*) FROM \"{namespace}\".schema_migrations"),
            &[],
        )
        .map_err(|error| format!("failed to count applied migrations: {error}"))?
        .get(0);
    Ok(applied > 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        PostgresCapabilityLedger, PostgresEventLog, PostgresRevisionLedger, PostgresStateStore,
    };
    use std::sync::atomic::{AtomicU64, Ordering};

    /// A state path that does not exist, unique per call so that tests do not
    /// share a namespace across runs.
    fn unique_state_path(tag: &str) -> PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir()
            .join(format!(
                "gauntlet-locator-{tag}-{}-{counter}-{nanos:x}",
                std::process::id()
            ))
            .join("gauntlet-state.db")
    }

    #[test]
    fn namespace_is_stable_and_valid() {
        let path = PathBuf::from("/home/user/gauntlet/state/gauntlet-state.db");
        let first = namespace_for_path(&path);
        let second = namespace_for_path(&path);
        assert_eq!(first, second);
        assert_eq!(first.len(), "gauntlet_".len() + 16);
        assert!(first.starts_with("gauntlet_"));
        assert!(validate_identifier(&first).is_ok());
        let hex = &first["gauntlet_".len()..];
        assert!(
            hex.chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        );
    }

    #[test]
    fn namespace_normalizes_dot_segments() {
        let plain = PathBuf::from("/home/user/state/gauntlet-state.db");
        let dotted = PathBuf::from("/home/user/./x/../state/gauntlet-state.db");
        assert_eq!(namespace_for_path(&plain), namespace_for_path(&dotted));
    }

    #[test]
    fn distinct_paths_get_distinct_namespaces() {
        let a = PathBuf::from("/home/user/state-a/gauntlet-state.db");
        let b = PathBuf::from("/home/user/state-b/gauntlet-state.db");
        assert_ne!(namespace_for_path(&a), namespace_for_path(&b));
    }

    #[test]
    fn nonexistent_parent_is_accepted() {
        let path = PathBuf::from("/definitely/not/here/gauntlet-state.db");
        assert!(validate_identifier(&namespace_for_path(&path)).is_ok());
    }

    #[test]
    fn state_exists_is_false_before_open_and_true_after() {
        let path = unique_state_path("exists");
        assert!(!state_exists(&path).expect("the database is reachable"));
        let store = PostgresStateStore::open_at(&path).expect("open_at creates the instance");
        drop(store);
        assert!(state_exists(&path).expect("the database is reachable"));
        assert!(!path.exists(), "no file is created at the state path");
    }

    #[test]
    fn every_store_kind_opens_the_same_instance_at_a_path() {
        let path = unique_state_path("kinds");
        let _log = PostgresEventLog::open_at(&path).expect("event log opens");
        let _capabilities =
            PostgresCapabilityLedger::open_at(&path).expect("capability ledger opens");
        let _revisions = PostgresRevisionLedger::open_at(&path).expect("revision ledger opens");
        assert!(state_exists(&path).expect("the database is reachable"));
    }
}
