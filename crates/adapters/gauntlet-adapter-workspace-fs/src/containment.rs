//! Detective containment baseline (W6 work-product guard; plan §W6).
//!
//! A stat-only — size, mtime, kind — manifest of the project root,
//! captured at dispatch and diffed at completion. No byte is ever read:
//! a builder that writes outside its workspace changes size, mtime, or
//! kind (or adds/removes a path), and all four show in the diff. Reads
//! never trip it (atime is not recorded), and renames show as a
//! remove-plus-add pair, which is exactly what they are.
//!
//! Symlinks are recorded by target (never followed — a symlink escape is
//! data, not a traversal), directories by kind only (a child's
//! add/remove already reports itself; dir-mtime churn would double-report
//! it), and everything else (sockets, pipes, devices) by kind only.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use gauntlet_ports::workspace_provider::WorkspaceProviderError;

use crate::seed::is_excluded;

/// Refuses a containment walk above this many entries rather than
/// stat-ing unboundedly (a stat walk is cheap, but never infinite).
const MAX_CONTAINMENT_ENTRIES: usize = 200_000;

/// What the baseline records about one path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainmentEntry {
    pub size: u64,
    pub mtime_nanos: u128,
    pub kind: ContainmentKind,
}

/// The shape of one path: anything a builder-side write can change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContainmentKind {
    File,
    Dir,
    Symlink,
    Other,
}

/// Relative path → dispatch-time entry.
pub type ContainmentManifest = std::collections::BTreeMap<String, ContainmentEntry>;

fn io_err(context: &str, path: &Path, error: io::Error) -> WorkspaceProviderError {
    WorkspaceProviderError::ContainmentCheckFailed {
        detail: format!("{context} {}: {error}", path.display()),
    }
}

fn mtime_nanos(path: &Path, modified: SystemTime) -> Result<u128, WorkspaceProviderError> {
    modified
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|since| since.as_nanos())
        .map_err(|error| WorkspaceProviderError::ContainmentCheckFailed {
            detail: format!("clock skew reading {}: {error}", path.display()),
        })
}

/// Stats one path without following a final symlink.
fn stat_entry(path: &Path) -> Result<Option<ContainmentEntry>, WorkspaceProviderError> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(io_err("stat-ing", path, error)),
    };
    if metadata.file_type().is_symlink() {
        let target = fs::read_link(path).map_err(|error| io_err("reading", path, error))?;
        return Ok(Some(ContainmentEntry {
            size: target.as_os_str().len() as u64,
            mtime_nanos: mtime_nanos(
                path,
                metadata
                    .modified()
                    .map_err(|e| io_err("stat-ing", path, e))?,
            )?,
            kind: ContainmentKind::Symlink,
        }));
    }
    if metadata.is_dir() {
        return Ok(Some(ContainmentEntry {
            size: 0,
            mtime_nanos: 0,
            kind: ContainmentKind::Dir,
        }));
    }
    if metadata.is_file() {
        return Ok(Some(ContainmentEntry {
            size: metadata.len(),
            mtime_nanos: mtime_nanos(
                path,
                metadata
                    .modified()
                    .map_err(|e| io_err("stat-ing", path, e))?,
            )?,
            kind: ContainmentKind::File,
        }));
    }
    Ok(Some(ContainmentEntry {
        size: 0,
        mtime_nanos: 0,
        kind: ContainmentKind::Other,
    }))
}

/// Captures the dispatch-time manifest: every non-excluded path under
/// `root`, stat-only.
pub fn capture_manifest(
    root: &Path,
    excludes: &[String],
) -> Result<ContainmentManifest, WorkspaceProviderError> {
    let mut manifest = ContainmentManifest::new();
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
            if manifest.len() >= MAX_CONTAINMENT_ENTRIES {
                return Err(WorkspaceProviderError::ContainmentCheckFailed {
                    detail: format!(
                        "the project tree at {} exceeds the bounded maximum ({} entries)",
                        root.display(),
                        MAX_CONTAINMENT_ENTRIES
                    ),
                });
            }
            let disk_path = root.join(&rel_path);
            if let Some(stat) = stat_entry(&disk_path)? {
                if stat.kind == ContainmentKind::Dir {
                    stack.push(rel_path.clone());
                }
                manifest.insert(rel_path.to_string_lossy().into_owned(), stat);
            }
        }
    }
    Ok(manifest)
}

/// Diffs `current` against `baseline` into `(changed, added, removed)` —
/// every list sorted, so evidence and residuals are deterministic.
pub fn diff_manifests(
    baseline: &ContainmentManifest,
    current: &ContainmentManifest,
) -> (Vec<String>, Vec<String>, Vec<String>) {
    let mut changed = Vec::new();
    let mut added = Vec::new();
    let mut removed = Vec::new();
    for (path, current_entry) in current {
        match baseline.get(path) {
            None => added.push(path.clone()),
            Some(baseline_entry) if baseline_entry != current_entry => {
                changed.push(path.clone());
            }
            Some(_) => {}
        }
    }
    for path in baseline.keys() {
        if !current.contains_key(path) {
            removed.push(path.clone());
        }
    }
    changed.sort();
    added.sort();
    removed.sort();
    (changed, added, removed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(tag: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("gauntlet-containment-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("the scratch root is creatable");
        root
    }

    #[test]
    fn an_outside_write_is_a_diff() {
        let root = scratch("diff");
        std::fs::write(root.join("kept.txt"), b"same").expect("writes");
        std::fs::write(root.join("edited.txt"), b"before").expect("writes");
        std::fs::write(root.join("deleted.txt"), b"gone").expect("writes");
        let baseline = capture_manifest(&root, &[]).expect("captures");
        std::fs::write(root.join("edited.txt"), b"after, longer").expect("rewrites");
        std::fs::write(root.join("added.txt"), b"new").expect("adds");
        std::fs::remove_file(root.join("deleted.txt")).expect("removes");
        let current = capture_manifest(&root, &[]).expect("re-captures");
        let (changed, added, removed) = diff_manifests(&baseline, &current);
        assert_eq!(changed, vec!["edited.txt".to_owned()]);
        assert_eq!(added, vec!["added.txt".to_owned()]);
        assert_eq!(removed, vec!["deleted.txt".to_owned()]);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_same_size_rewrite_still_flags_mtime() {
        let root = scratch("mtime");
        std::fs::write(root.join("same.txt"), b"12345678").expect("writes");
        let baseline = capture_manifest(&root, &[]).expect("captures");
        std::thread::sleep(std::time::Duration::from_millis(50));
        std::fs::write(root.join("same.txt"), b"12345678").expect("rewrites identically");
        let current = capture_manifest(&root, &[]).expect("re-captures");
        let (changed, _, _) = diff_manifests(&baseline, &current);
        assert_eq!(
            changed,
            vec!["same.txt".to_owned()],
            "identical bytes still advance mtime — the guard is not size-only"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn excluded_subtrees_are_invisible() {
        let root = scratch("excludes");
        std::fs::create_dir_all(root.join(".git")).expect("mkdir");
        std::fs::write(root.join(".git").join("HEAD"), b"ref").expect("writes");
        std::fs::write(root.join("src.txt"), b"v1").expect("writes");
        let excludes = vec![".git".to_owned()];
        let baseline = capture_manifest(&root, &excludes).expect("captures");
        assert!(!baseline.contains_key(".git/HEAD"), "excluded at capture");
        std::fs::write(root.join(".git").join("HEAD"), b"changed-ref").expect("rewrites");
        std::fs::write(root.join(".git").join("new-obj"), b"new").expect("adds");
        // src.txt untouched: the ONLY writes are excluded ones.
        let current = capture_manifest(&root, &excludes).expect("re-captures");
        let (changed, added, _) = diff_manifests(&baseline, &current);
        assert!(
            changed.is_empty() && added.is_empty(),
            "excluded writes never diff"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[cfg(unix)]
    #[test]
    fn a_repointed_symlink_is_a_change() {
        let root = scratch("symlink");
        std::fs::write(root.join("real-a.txt"), b"a").expect("writes");
        std::fs::write(root.join("real-b.txt"), b"b").expect("writes");
        std::os::unix::fs::symlink("real-a.txt", root.join("link.txt")).expect("links");
        let baseline = capture_manifest(&root, &[]).expect("captures");
        std::fs::remove_file(root.join("link.txt")).expect("unlinks");
        std::os::unix::fs::symlink("real-b.txt", root.join("link.txt")).expect("re-links");
        let current = capture_manifest(&root, &[]).expect("re-captures");
        let (changed, _, _) = diff_manifests(&baseline, &current);
        assert_eq!(changed, vec!["link.txt".to_owned()]);
        let _ = std::fs::remove_dir_all(&root);
    }
}
