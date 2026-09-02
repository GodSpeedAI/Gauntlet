//! The filesystem [`WorkspaceProvider`] (spec §6.2, §17.5; plan Task 20,
//! R-1b part a).
//!
//! Port methods carry a [`Deadline`] (spec §6 preamble); this adapter
//! performs single filesystem operations with no retry loop and holds no
//! clock of its own (the `Clock` port is the runtime's only "now"), so the
//! bound is noted and not consulted — there is no waiting code here to
//! bound. An adapter that adds retries must consult it.
//!
//! ## Containment discipline (§17.5)
//!
//! Three layers, cheapest first:
//!
//! 1. DERIVE: the workspace key is built only from the run id, the subject's
//!    unit ids, and a process-local sequence, each sanitized to
//!    `[A-Za-z0-9._-]` — separators and `..` segments are unrepresentable in
//!    a key, so `root.join(key)` cannot traverse by construction.
//! 2. LEXICAL CHECK: the join is still verified with the roles installer's
//!    `contained_join` discipline (task17) — the child must be relative and
//!    stay inside the root by component inspection. Defense in depth costs
//!    one line; the derive layer makes the check pass, the check makes a
//!    future derive regression a refusal instead of an escape.
//! 3. RESOLVED CHECK (§17.5.2 "symlink escape is checked, not assumed"):
//!    after the directory exists, BOTH the workspace and the root are
//!    canonicalized and the prefix re-verified — a symlink planted over a
//!    workspace path pointing outside the root is a typed
//!    `ContainmentViolated`, never a workspace.
//!
//! `cleanup` treats its [`WorkspaceRef`] argument as UNTRUSTED external
//! input (AGENTS.md): the same derive→lexical→resolved order applies, with
//! the ref validated against the key class before it may join the root.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use gauntlet_domain::{RunId, WorkspaceRef};
use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::workspace_provider::{
    Workspace, WorkspaceHook, WorkspaceProvider, WorkspaceProviderError, WorkspaceSubject,
};

/// Distinguishes workspaces created for the same (run, subject) pair — a
/// re-dispatched unit gets a FRESH workspace (spec §10.4: `create` is per
/// unit-or-slice), never a reuse of a prior attempt's directory.
static WORKSPACE_SEQ: AtomicU64 = AtomicU64::new(0);

/// A filesystem-backed workspace provider: every workspace is a directory
/// under the root this instance was opened with.
#[derive(Debug, Clone)]
pub struct FsWorkspaceProvider {
    root: PathBuf,
}

impl FsWorkspaceProvider {
    /// Opens (and creates, if needed) the workspace root. The composition
    /// passes the configured workspaces directory (e.g.
    /// `<state_dir>/workspaces` via the app defaults'
    /// `WORKSPACES_DIR_NAME`) — never a literal.
    pub fn open(root: &Path) -> Result<Self, WorkspaceProviderError> {
        std::fs::create_dir_all(root).map_err(|error| WorkspaceProviderError::CreateFailed {
            detail: format!(
                "the workspace root {} could not be created: {error}",
                root.display()
            ),
        })?;
        Ok(Self {
            root: root.to_path_buf(),
        })
    }

    /// The configured root this provider materializes workspaces under.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The sanitized workspace KEY for one subject (§17.5.3): every byte
    /// outside `[A-Za-z0-9._-]` becomes `_`, so the key is one path
    /// component — no separator, no traversal segment, ever.
    fn key_for(run_id: &RunId, subject: &WorkspaceSubject) -> String {
        let subject_key = match subject {
            WorkspaceSubject::Unit(unit) => unit.to_string(),
            WorkspaceSubject::Slice(units) => {
                let joined = units
                    .iter()
                    .map(|unit| unit.to_string())
                    .collect::<Vec<_>>()
                    .join("-");
                format!("slice-{joined}")
            }
        };
        let sequence = WORKSPACE_SEQ.fetch_add(1, Ordering::Relaxed);
        sanitize(&format!("{run_id}.{subject_key}.{sequence:06}"))
    }

    /// Joins a VALIDATED key class onto the root and checks containment
    /// lexically (the `contained_join` discipline): the child must stay
    /// inside the root by component inspection. The key class guarantees
    /// single-component keys, so a failure here names a bug in the derive
    /// layer, not a caller-input problem — still a typed refusal, never a
    /// guessed-at path.
    fn contained_child(&self, key: &str) -> Result<PathBuf, WorkspaceProviderError> {
        let child = self.root.join(key);
        if !is_within_root(&child, &self.root) {
            return Err(WorkspaceProviderError::ContainmentViolated {
                path: child.to_string_lossy().into_owned(),
            });
        }
        Ok(child)
    }

    /// The RESOLVED containment check (§17.5.2): canonicalize the child and
    /// the root, then require the prefix. Runs AFTER the directory exists.
    fn resolved_containment(&self, child: &Path) -> Result<(), WorkspaceProviderError> {
        let resolved_root = std::fs::canonicalize(&self.root).map_err(|error| {
            WorkspaceProviderError::CreateFailed {
                detail: format!(
                    "the workspace root {} could not be resolved: {error}",
                    self.root.display()
                ),
            }
        })?;
        let resolved_child =
            std::fs::canonicalize(child).map_err(|error| WorkspaceProviderError::CreateFailed {
                detail: format!(
                    "the workspace {} could not be resolved: {error}",
                    child.display()
                ),
            })?;
        if !resolved_child.starts_with(&resolved_root) {
            return Err(WorkspaceProviderError::ContainmentViolated {
                path: resolved_child.to_string_lossy().into_owned(),
            });
        }
        Ok(())
    }
}

impl WorkspaceProvider for FsWorkspaceProvider {
    fn create(
        &self,
        run_id: &RunId,
        subject: WorkspaceSubject,
        _deadline: Deadline,
    ) -> Result<Workspace, WorkspaceProviderError> {
        let key = Self::key_for(run_id, &subject);
        let child = self.contained_child(&key)?;
        std::fs::create_dir_all(&child).map_err(|error| WorkspaceProviderError::CreateFailed {
            detail: format!(
                "the workspace directory {} could not be created: {error}",
                child.display()
            ),
        })?;
        // §17.5.2: the escape is CHECKED after the fact, on the resolved
        // path — creation through a symlinked component is a refusal, and
        // the created directory is left for `cleanup` rather than run in.
        self.resolved_containment(&child)?;
        let reference =
            WorkspaceRef::new(&key).map_err(|error| WorkspaceProviderError::CreateFailed {
                detail: format!("the derived key {key:?} is not a valid workspace ref: {error}"),
            })?;
        Ok(Workspace {
            reference,
            root: child,
            key,
        })
    }

    fn cleanup(
        &self,
        workspace: &WorkspaceRef,
        _deadline: Deadline,
    ) -> Result<(), WorkspaceProviderError> {
        // The ref is untrusted external input: it must be of THIS
        // provider's key class (sanitized, single component) before it may
        // join the root. Anything else — separators, `..`, absolute
        // shapes — is a containment refusal, not a best-effort miss.
        if !is_sanitized(workspace.as_str()) {
            return Err(WorkspaceProviderError::ContainmentViolated {
                path: workspace.as_str().to_owned(),
            });
        }
        let child = self.contained_child(workspace.as_str())?;
        match std::fs::remove_dir_all(&child) {
            Ok(()) => {}
            // Already released: cleanup is idempotent — a caller terminalizing
            // twice (retry ledger, recovery re-probe) must not error.
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(WorkspaceProviderError::CleanupFailed {
                    detail: format!(
                        "the workspace {} could not be removed: {error}",
                        child.display()
                    ),
                });
            }
        }
        Ok(())
    }

    /// No workspace hooks exist in this tree yet: no configuration surface
    /// resolves any (the §6.2 `hooks` consumer is uncomposed), and an
    /// adapter that INVENTED one would be fabricating configuration. Empty
    /// is the honest answer until the config schema grows the field.
    fn hooks(&self) -> Vec<WorkspaceHook> {
        Vec::new()
    }
}

/// Whether `child` sits inside `root` by component inspection — the roles
/// installer's lexical rule (task17 `contained_join`/`ensure_within_root`),
/// duplicated here because adapters depend on domain and ports only, never
/// on gauntlet-app where that helper lives.
fn is_within_root(child: &Path, root: &Path) -> bool {
    if !child.is_absolute() || !root.is_absolute() {
        return false;
    }
    if child
        .components()
        .any(|component| component == std::path::Component::ParentDir)
    {
        return false;
    }
    child.starts_with(root)
}

/// The §17.5.3 key class: `[A-Za-z0-9._-]`, non-empty. Every other byte
/// becomes `_`.
fn sanitize(raw: &str) -> String {
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

/// Whether a string is already inside the §17.5.3 key class.
fn is_sanitized(raw: &str) -> bool {
    !raw.is_empty()
        && raw.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || byte == b'.' || byte == b'_' || byte == b'-'
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use gauntlet_domain::{EpochSeconds, UnitId};

    fn bound() -> Deadline {
        Deadline::at_epoch(EpochSeconds::new(1_700_000_060))
    }

    fn run() -> RunId {
        RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("suite run id is a ULID")
    }

    fn scratch(tag: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "gauntlet-workspace-fs-unit-{tag}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("scratch is creatable");
        path
    }

    fn unit(id: &str) -> UnitId {
        UnitId::new(id).expect("unit id")
    }

    #[test]
    fn keys_sanitize_everything_outside_the_key_class() {
        assert_eq!(sanitize("run./../etc"), "run._.._etc");
        assert_eq!(sanitize("a b\tc"), "a_b_c");
        assert_eq!(sanitize(""), "_");
        assert!(is_sanitized(&sanitize("anything / goes ..")));
        assert!(!is_sanitized("../escape"));
        assert!(!is_sanitized("a/b"));
        assert!(!is_sanitized(""));
    }

    #[test]
    fn every_created_key_is_one_sanitized_component_under_the_root() {
        // The domain grammars (UnitId, RunId) are already narrow, but the
        // adapter's own derive layer is what the §17.5.3 class is enforced
        // by — checked here on every create, not assumed from upstream.
        let root = scratch("keys");
        let provider = FsWorkspaceProvider::open(&root).expect("opens");
        let workspace = provider
            .create(&run(), WorkspaceSubject::Unit(unit("AK-014")), bound())
            .expect("creates");
        assert!(is_sanitized(&workspace.key), "the key: {workspace:?}");
        assert_eq!(
            workspace.root.components().count(),
            root.components().count() + 1,
            "the key is ONE component under the root"
        );
        assert!(workspace.root.starts_with(&root));
    }

    #[test]
    fn cleanup_refuses_a_ref_outside_the_key_class() {
        let root = scratch("cleanup-refusal");
        let provider = FsWorkspaceProvider::open(&root).expect("opens");
        for hostile in ["../escape", "a/b", "", ".hidden/.."] {
            if let Ok(reference) = WorkspaceRef::new(hostile) {
                let result = provider.cleanup(&reference, bound());
                assert!(
                    matches!(
                        result,
                        Err(WorkspaceProviderError::ContainmentViolated { .. })
                    ),
                    "a ref outside the key class must be refused, got {result:?} for {hostile:?}"
                );
            }
        }
        assert!(
            std::fs::read_dir(&root).expect("root reads").count() == 0,
            "nothing outside the key class was ever joined, so nothing was removed"
        );
    }

    #[test]
    fn cleanup_is_idempotent_over_a_released_workspace() {
        let root = scratch("cleanup-idempotent");
        let provider = FsWorkspaceProvider::open(&root).expect("opens");
        let workspace = provider
            .create(&run(), WorkspaceSubject::Unit(unit("AK-014")), bound())
            .expect("creates");
        provider
            .cleanup(&workspace.reference, bound())
            .expect("the first cleanup succeeds");
        provider
            .cleanup(&workspace.reference, bound())
            .expect("a second cleanup of the same ref is a no-op, not an error");
    }

    #[test]
    fn a_symlinked_workspace_escape_is_a_typed_refusal() {
        // §17.5.2: the escape is checked on the RESOLVED path. A symlink
        // planted at a path the provider would hand out, pointing OUTSIDE
        // the root, must be refused — never a workspace.
        let root = scratch("symlink-escape");
        let outside = scratch("symlink-outside");
        let provider = FsWorkspaceProvider::open(&root).expect("opens");
        if !cfg!(unix) {
            // Platform-honest: without symlink support this platform cannot
            // plant the escape, and the case has no subject.
            return;
        }
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&outside, root.join("planted-escape"))
                .expect("plants the symlink");
            let check = provider.resolved_containment(&root.join("planted-escape"));
            assert!(
                matches!(
                    check,
                    Err(WorkspaceProviderError::ContainmentViolated { .. })
                ),
                "a symlink pointing outside the root is a typed containment refusal: {check:?}"
            );
        }
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&outside);
    }
}
