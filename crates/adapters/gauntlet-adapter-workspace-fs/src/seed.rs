//! Seeded workspaces over a run-scoped integration tree (W5 work-product
//! carry-forward; plan §W5).
//!
//! Without [`WorkspaceSeed`] configured, [`super::FsWorkspaceProvider`]
//! behaves exactly as before: `create` makes an empty directory. With one
//! configured, `create` first ensures a filtered, run-scoped snapshot of
//! the project root exists (built once, atomically, and reused for the
//! life of the run — this is what lets settled work carry forward across
//! units), then copies the CURRENT integration tree into the new
//! workspace and records that copy's manifest as the workspace's seed, for
//! `promote` to diff against later.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use sha2::{Digest as _, Sha256};

use gauntlet_domain::RunId;
use gauntlet_ports::workspace_provider::WorkspaceProviderError;

/// Refuses a project-root snapshot above this many files rather than
/// copying unboundedly.
const MAX_SNAPSHOT_FILES: usize = 50_000;
/// Refuses a project-root snapshot above this many total bytes.
const MAX_SNAPSHOT_TOTAL_BYTES: u64 = 1_000_000_000;
/// Skips (recording as skipped) any single file above this many bytes.
const MAX_SNAPSHOT_FILE_BYTES: u64 = 50_000_000;

static TEMP_SEQ: AtomicU64 = AtomicU64::new(0);

/// Turns every `create` into a seeded copy of a run-scoped integration
/// tree instead of an empty directory (builder-style: `.with_seed(...)`
/// on [`super::FsWorkspaceProvider`]).
#[derive(Debug, Clone)]
pub struct WorkspaceSeed {
    /// The project root a run's integration tree is snapshotted from.
    pub project_root: PathBuf,
    /// Where per-run integration trees and baselines live —
    /// `<integration_root>/<run_id>/tree/` and `.../baseline.json`.
    pub integration_root: PathBuf,
    /// Relative paths excluded from both the snapshot and later promotion,
    /// matched as a path-component prefix (`.git` excludes `.git/**`).
    pub excludes: Vec<String>,
    /// Relative paths excluded from the W6 detective containment baseline
    /// — the same VCS/build/state surfaces as `excludes`, but WITHOUT the
    /// answer-key evidence locators: a locator path under the project root
    /// is instrument-owned, and a builder touching it is itself a breach
    /// worth catching, never seed noise to hide.
    pub containment_excludes: Vec<String>,
}

/// One file's recorded content digest, size, and executable bit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDigest {
    pub sha256: String,
    pub size: u64,
    pub executable: bool,
}

/// Relative path → recorded digest, for a snapshot, an integration tree, or
/// a workspace seed.
pub type Manifest = BTreeMap<String, FileDigest>;

/// Whether `rel` is excluded: an exclude entry matches as a path-component
/// prefix of `rel` (so `.git` excludes `.git` itself and everything under
/// it; `evidence/check-report.json` excludes only that exact file).
pub fn is_excluded(rel: &Path, excludes: &[String]) -> bool {
    excludes.iter().any(|exclude| {
        let mut exclude_components = Path::new(exclude).components();
        let mut rel_components = rel.components();
        loop {
            match exclude_components.next() {
                None => return true,
                Some(expected) => match rel_components.next() {
                    None => return false,
                    Some(actual) if actual == expected => continue,
                    Some(_) => return false,
                },
            }
        }
    })
}

fn io_err(context: &str, path: &Path, error: io::Error) -> WorkspaceProviderError {
    WorkspaceProviderError::CreateFailed {
        detail: format!("{context} {}: {error}", path.display()),
    }
}

#[cfg(unix)]
fn is_executable(metadata: &fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt as _;
    const EXECUTABLE_MODE_BITS: u32 = 0o111;
    metadata.permissions().mode() & EXECUTABLE_MODE_BITS != 0
}

#[cfg(not(unix))]
fn is_executable(_metadata: &fs::Metadata) -> bool {
    false
}

#[cfg(unix)]
fn set_executable(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt as _;
    const EXECUTABLE_MODE: u32 = 0o755;
    let mut permissions = fs::metadata(path)?.permissions();
    permissions.set_mode(EXECUTABLE_MODE);
    fs::set_permissions(path, permissions)
}

#[cfg(not(unix))]
fn set_executable(_path: &Path) -> io::Result<()> {
    Ok(())
}

fn hex_digest(bytes: &[u8]) -> String {
    let hashed = Sha256::digest(bytes);
    let mut out = String::with_capacity(hashed.len() * 2);
    for byte in hashed {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

/// A copy of `src` onto `dst` (both directories, `dst` freshly created),
/// filtering `excludes`, skipping symlinks and special files, and refusing
/// (bounded) or skipping (per-file) oversized content. Returns the
/// manifest of everything copied.
pub struct CopyOutcome {
    pub manifest: Manifest,
    pub skipped: Vec<String>,
}

pub fn copy_tree(
    src: &Path,
    dst: &Path,
    excludes: &[String],
    enforce_bounds: bool,
) -> Result<CopyOutcome, WorkspaceProviderError> {
    let mut manifest = Manifest::new();
    let mut skipped = Vec::new();
    let mut total_bytes: u64 = 0;
    let mut total_files: usize = 0;
    let mut stack = vec![PathBuf::new()];
    while let Some(rel_dir) = stack.pop() {
        let src_dir = src.join(&rel_dir);
        let entries = fs::read_dir(&src_dir).map_err(|error| io_err("reading", &src_dir, error))?;
        for entry in entries {
            let entry = entry.map_err(|error| io_err("reading an entry under", &src_dir, error))?;
            let rel_path = rel_dir.join(entry.file_name());
            if is_excluded(&rel_path, excludes) {
                continue;
            }
            let metadata = entry
                .metadata()
                .map_err(|error| io_err("stat-ing", &entry.path(), error))?;
            let symlink_metadata = fs::symlink_metadata(entry.path())
                .map_err(|error| io_err("stat-ing", &entry.path(), error))?;
            if symlink_metadata.file_type().is_symlink() {
                skipped.push(rel_path.to_string_lossy().into_owned());
                continue;
            }
            if metadata.is_dir() {
                fs::create_dir_all(dst.join(&rel_path))
                    .map_err(|error| io_err("creating", &dst.join(&rel_path), error))?;
                stack.push(rel_path);
                continue;
            }
            if !metadata.is_file() {
                skipped.push(rel_path.to_string_lossy().into_owned());
                continue;
            }
            let size = metadata.len();
            if enforce_bounds && size > MAX_SNAPSHOT_FILE_BYTES {
                skipped.push(rel_path.to_string_lossy().into_owned());
                continue;
            }
            total_files += 1;
            total_bytes += size;
            if enforce_bounds
                && (total_files > MAX_SNAPSHOT_FILES || total_bytes > MAX_SNAPSHOT_TOTAL_BYTES)
            {
                return Err(WorkspaceProviderError::CreateFailed {
                    detail: format!(
                        "the project snapshot at {} exceeds the bounded maximum ({} files / {} bytes)",
                        src.display(),
                        MAX_SNAPSHOT_FILES,
                        MAX_SNAPSHOT_TOTAL_BYTES
                    ),
                });
            }
            let bytes =
                fs::read(entry.path()).map_err(|error| io_err("reading", &entry.path(), error))?;
            let sha256 = hex_digest(&bytes);
            let executable = is_executable(&metadata);
            let dst_path = dst.join(&rel_path);
            if let Some(parent) = dst_path.parent() {
                fs::create_dir_all(parent).map_err(|error| io_err("creating", parent, error))?;
            }
            fs::write(&dst_path, &bytes).map_err(|error| io_err("writing", &dst_path, error))?;
            if executable {
                set_executable(&dst_path).map_err(|error| io_err("chmod-ing", &dst_path, error))?;
            }
            manifest.insert(
                rel_path.to_string_lossy().into_owned(),
                FileDigest {
                    sha256,
                    size,
                    executable,
                },
            );
        }
    }
    Ok(CopyOutcome { manifest, skipped })
}

/// Walks an already-on-disk tree (a workspace, post-copy) and digests every
/// non-excluded regular file, without writing anything. Used by `promote`
/// to see what a builder changed since the workspace was seeded.
pub fn compute_manifest(
    root: &Path,
    excludes: &[String],
) -> Result<Manifest, WorkspaceProviderError> {
    let mut manifest = Manifest::new();
    let mut stack = vec![PathBuf::new()];
    while let Some(rel_dir) = stack.pop() {
        let dir = root.join(&rel_dir);
        let entries = fs::read_dir(&dir).map_err(|error| io_err("reading", &dir, error))?;
        for entry in entries {
            let entry = entry.map_err(|error| io_err("reading an entry under", &dir, error))?;
            let rel_path = rel_dir.join(entry.file_name());
            if is_excluded(&rel_path, excludes) {
                continue;
            }
            let symlink_metadata = fs::symlink_metadata(entry.path())
                .map_err(|error| io_err("stat-ing", &entry.path(), error))?;
            if symlink_metadata.file_type().is_symlink() {
                continue;
            }
            if symlink_metadata.is_dir() {
                stack.push(rel_path);
                continue;
            }
            if !symlink_metadata.is_file() {
                continue;
            }
            if let Some(digest) = digest_file(&entry.path())? {
                manifest.insert(rel_path.to_string_lossy().into_owned(), digest);
            }
        }
    }
    Ok(manifest)
}

/// Digests one already-on-disk file, or `None` if it does not exist (as a
/// regular file).
pub fn digest_file(path: &Path) -> Result<Option<FileDigest>, WorkspaceProviderError> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(io_err("stat-ing", path, error)),
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Ok(None);
    }
    let bytes = fs::read(path).map_err(|error| io_err("reading", path, error))?;
    let full_metadata = fs::metadata(path).map_err(|error| io_err("stat-ing", path, error))?;
    Ok(Some(FileDigest {
        sha256: hex_digest(&bytes),
        size: bytes.len() as u64,
        executable: is_executable(&full_metadata),
    }))
}

/// Writes `bytes` to `dest` atomically (temp file in the same directory,
/// then rename), creating parent directories, preserving the executable
/// bit.
pub fn write_file_atomic(dest: &Path, bytes: &[u8], executable: bool) -> io::Result<()> {
    let parent = dest.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let sequence = TEMP_SEQ.fetch_add(1, Ordering::Relaxed);
    let temp = parent.join(format!(
        ".tmp-{}-{}-{sequence:06}",
        std::process::id(),
        dest.file_name().and_then(|n| n.to_str()).unwrap_or("file")
    ));
    fs::write(&temp, bytes)?;
    if executable {
        set_executable(&temp)?;
    }
    fs::rename(&temp, dest)
}

/// The run's integration tree root, ensuring the snapshot exists (built in
/// a sibling temp dir, then renamed into place; a concurrent builder that
/// wins the race is simply reused).
pub fn ensure_integration_snapshot(
    seed: &WorkspaceSeed,
    run_id: &RunId,
) -> Result<PathBuf, WorkspaceProviderError> {
    let run_root = seed
        .integration_root
        .join(sanitize_component(run_id.as_str()));
    let tree = run_root.join("tree");
    let baseline = run_root.join("baseline.json");
    if tree.is_dir() && baseline.is_file() {
        return Ok(tree);
    }
    fs::create_dir_all(&seed.integration_root)
        .map_err(|error| io_err("creating", &seed.integration_root, error))?;
    let sequence = TEMP_SEQ.fetch_add(1, Ordering::Relaxed);
    let temp_root = seed.integration_root.join(format!(
        ".tmp-snapshot-{}-{}-{sequence:06}",
        std::process::id(),
        sanitize_component(run_id.as_str())
    ));
    let _ = fs::remove_dir_all(&temp_root);
    let temp_tree = temp_root.join("tree");
    fs::create_dir_all(&temp_tree).map_err(|error| io_err("creating", &temp_tree, error))?;
    let outcome = copy_tree(&seed.project_root, &temp_tree, &seed.excludes, true)?;
    let baseline_json = manifest_to_json(&outcome.manifest);
    let temp_baseline = temp_root.join("baseline.json");
    fs::write(&temp_baseline, baseline_json)
        .map_err(|error| io_err("writing", &temp_baseline, error))?;
    match fs::rename(&temp_root, &run_root) {
        Ok(()) => Ok(tree),
        Err(_) if tree.is_dir() && baseline.is_file() => {
            // Lost the race to another process/thread building the same
            // run's snapshot concurrently — the existing snapshot is the
            // one that counts; drop ours.
            let _ = fs::remove_dir_all(&temp_root);
            Ok(tree)
        }
        Err(error) => Err(io_err("renaming", &temp_root, error)),
    }
}

/// The §17.5.3-style key-class sanitization applied to path components
/// this module derives (the run id is already narrow, but the derive
/// layer is what the guarantee is enforced by, not the caller).
fn sanitize_component(raw: &str) -> String {
    let sanitized: String = raw
        .bytes()
        .map(|byte| {
            if byte.is_ascii_alphanumeric() || byte == b'.' || byte == b'_' || byte == b'-' {
                byte as char
            } else {
                '_'
            }
        })
        .collect();
    if sanitized.is_empty() {
        "_".to_owned()
    } else {
        sanitized
    }
}

/// A minimal, dependency-free JSON encoding for a [`Manifest`] (baseline
/// and delivery manifests are read back by both this adapter and the CLI's
/// `deliver` command via [`parse_manifest_json`]).
pub fn manifest_to_json(manifest: &Manifest) -> String {
    let mut out = String::from("{\n");
    let mut first = true;
    for (path, digest) in manifest {
        if !first {
            out.push_str(",\n");
        }
        first = false;
        out.push_str(&format!(
            "  {}: {{\"sha256\": {}, \"size\": {}, \"executable\": {}}}",
            json_string(path),
            json_string(&digest.sha256),
            digest.size,
            digest.executable
        ));
    }
    out.push_str("\n}\n");
    out
}

fn json_string(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len() + 2);
    out.push('"');
    for ch in raw.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            control if (control as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", control as u32));
            }
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exclude_prefix_matching() {
        assert!(is_excluded(Path::new(".git"), &[".git".to_owned()]));
        assert!(is_excluded(Path::new(".git/HEAD"), &[".git".to_owned()]));
        assert!(!is_excluded(Path::new(".github"), &[".git".to_owned()]));
        assert!(is_excluded(
            Path::new("evidence/check-report.json"),
            &["evidence/check-report.json".to_owned()]
        ));
        assert!(!is_excluded(
            Path::new("evidence/other.json"),
            &["evidence/check-report.json".to_owned()]
        ));
    }

    #[test]
    fn manifest_json_round_trips_through_parse() {
        let mut manifest = Manifest::new();
        manifest.insert(
            "a/b.txt".to_owned(),
            FileDigest {
                sha256: "deadbeef".to_owned(),
                size: 3,
                executable: false,
            },
        );
        let json = manifest_to_json(&manifest);
        let parsed = crate::json::parse_manifest_json(&json).expect("parses");
        assert_eq!(parsed, manifest);
    }
}
