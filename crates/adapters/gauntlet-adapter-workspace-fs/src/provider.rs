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

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use gauntlet_domain::{RunId, WorkspaceRef};
use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::workspace_provider::{
    PromotedFile, Workspace, WorkspaceHook, WorkspacePromotion, WorkspaceProvider,
    WorkspaceProviderError, WorkspaceSubject,
};

use crate::seed::{self, Manifest, WorkspaceSeed};

/// Distinguishes workspaces created for the same (run, subject) pair — a
/// re-dispatched unit gets a FRESH workspace (spec §10.4: `create` is per
/// unit-or-slice), never a reuse of a prior attempt's directory.
static WORKSPACE_SEQ: AtomicU64 = AtomicU64::new(0);

/// A filesystem-backed workspace provider: every workspace is a directory
/// under the root this instance was opened with.
#[derive(Debug, Clone)]
pub struct FsWorkspaceProvider {
    root: PathBuf,
    seed: Option<WorkspaceSeed>,
    /// The seed manifest and run-integration-tree path recorded for each
    /// live seeded workspace (keyed by the workspace's sanitized key),
    /// consulted by `promote` to diff the current workspace content
    /// against what it started from. Shared across clones so composition
    /// can hand out one provider by value.
    seed_manifests: Arc<Mutex<HashMap<String, (PathBuf, Manifest)>>>,
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
            seed: None,
            seed_manifests: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    /// Turns every `create` into a seeded copy of a run-scoped integration
    /// tree (W5 work-product carry-forward) instead of an empty directory.
    /// Without this call, behaviour is unchanged.
    pub fn with_seed(mut self, seed: WorkspaceSeed) -> Self {
        self.seed = Some(seed);
        self
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
        if let Some(seed) = &self.seed {
            let tree = seed::ensure_integration_snapshot(seed, run_id)?;
            let outcome = seed::copy_tree(&tree, &child, &seed.excludes, false)?;
            self.seed_manifests
                .lock()
                .expect("seed manifest lock is never poisoned by a panicking holder")
                .insert(key.clone(), (tree, outcome.manifest));
        }
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
        self.seed_manifests
            .lock()
            .expect("seed manifest lock is never poisoned by a panicking holder")
            .remove(workspace.as_str());
        Ok(())
    }

    /// No workspace hooks exist in this tree yet: no configuration surface
    /// resolves any (the §6.2 `hooks` consumer is uncomposed), and an
    /// adapter that INVENTED one would be fabricating configuration. Empty
    /// is the honest answer until the config schema grows the field.
    fn hooks(&self) -> Vec<WorkspaceHook> {
        Vec::new()
    }

    fn promote(
        &self,
        workspace: &WorkspaceRef,
        _deadline: Deadline,
    ) -> Result<WorkspacePromotion, WorkspaceProviderError> {
        let Some(seed) = &self.seed else {
            return Ok(WorkspacePromotion::default());
        };
        if !is_sanitized(workspace.as_str()) {
            return Err(WorkspaceProviderError::ContainmentViolated {
                path: workspace.as_str().to_owned(),
            });
        }
        let workspace_root = self.contained_child(workspace.as_str())?;
        let (integration_tree, seed_manifest) = self
            .seed_manifests
            .lock()
            .expect("seed manifest lock is never poisoned by a panicking holder")
            .get(workspace.as_str())
            .cloned()
            .ok_or_else(|| WorkspaceProviderError::PromotionFailed {
                detail: format!(
                    "promote: {} is not a known, still-live seeded workspace",
                    workspace.as_str()
                ),
            })?;
        let current = seed::compute_manifest(&workspace_root, &seed.excludes)?;

        let mut changed = Vec::new();
        let mut deleted = Vec::new();
        let mut conflicts = Vec::new();

        for (path, current_digest) in &current {
            let seed_digest = seed_manifest.get(path);
            let is_changed = match seed_digest {
                Some(seed_digest) => seed_digest.sha256 != current_digest.sha256,
                None => true,
            };
            if !is_changed {
                continue;
            }
            let integration_path = integration_tree.join(path);
            let integration_digest = seed::digest_file(&integration_path)?;
            let unchanged_since_seed = match (&integration_digest, seed_digest) {
                (None, None) => true,
                (Some(integration_digest), Some(seed_digest)) => {
                    integration_digest.sha256 == seed_digest.sha256
                }
                _ => false,
            };
            if !unchanged_since_seed {
                conflicts.push(path.clone());
                continue;
            }
            let bytes = std::fs::read(workspace_root.join(path)).map_err(|error| {
                WorkspaceProviderError::PromotionFailed {
                    detail: format!("promote: reading {path} from the workspace failed: {error}"),
                }
            })?;
            seed::write_file_atomic(&integration_path, &bytes, current_digest.executable).map_err(
                |error| WorkspaceProviderError::PromotionFailed {
                    detail: format!(
                        "promote: writing {path} into the integration tree failed: {error}"
                    ),
                },
            )?;
            changed.push(PromotedFile {
                path: path.clone(),
                sha256: current_digest.sha256.clone(),
            });
        }

        for (path, seed_digest) in &seed_manifest {
            if current.contains_key(path) {
                continue;
            }
            let integration_path = integration_tree.join(path);
            let integration_digest = seed::digest_file(&integration_path)?;
            let unchanged_since_seed = integration_digest
                .as_ref()
                .is_some_and(|digest| digest.sha256 == seed_digest.sha256);
            if !unchanged_since_seed {
                conflicts.push(path.clone());
                continue;
            }
            std::fs::remove_file(&integration_path).map_err(|error| {
                WorkspaceProviderError::PromotionFailed {
                    detail: format!(
                        "promote: removing {path} from the integration tree failed: {error}"
                    ),
                }
            })?;
            deleted.push(path.clone());
        }

        Ok(WorkspacePromotion {
            supported: true,
            changed,
            deleted,
            conflicts,
        })
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

    fn seeded_provider(tag: &str) -> (FsWorkspaceProvider, PathBuf, PathBuf) {
        let root = scratch(&format!("seed-{tag}"));
        let project_root = scratch(&format!("seed-{tag}-project"));
        let integration_root = scratch(&format!("seed-{tag}-integration"));
        let provider = FsWorkspaceProvider::open(&root)
            .expect("opens")
            .with_seed(WorkspaceSeed {
                project_root: project_root.clone(),
                integration_root: integration_root.clone(),
                excludes: vec![".git".to_owned()],
            });
        (provider, project_root, integration_root)
    }

    #[test]
    fn a_seeded_workspace_starts_from_the_project_root_snapshot() {
        let (provider, project_root, _integration_root) = seeded_provider("basic");
        std::fs::write(project_root.join("app.txt"), b"hello").expect("writes");
        std::fs::create_dir_all(project_root.join(".git")).expect("mkdir");
        std::fs::write(project_root.join(".git").join("HEAD"), b"ref: x").expect("writes");
        let workspace = provider
            .create(&run(), WorkspaceSubject::Unit(unit("AK-014")), bound())
            .expect("creates");
        assert_eq!(
            std::fs::read(workspace.root.join("app.txt")).expect("reads"),
            b"hello"
        );
        assert!(
            !workspace.root.join(".git").exists(),
            "the exclude kept .git out of the seeded workspace"
        );
    }

    #[test]
    fn a_settled_units_promotion_carries_into_the_next_workspaces_seed() {
        let (provider, project_root, _integration_root) = seeded_provider("carry");
        std::fs::write(project_root.join("seed.txt"), b"seed").expect("writes");
        let first = provider
            .create(&run(), WorkspaceSubject::Unit(unit("AK-014")), bound())
            .expect("creates first");
        // The builder in the first workspace writes new settled work.
        std::fs::write(first.root.join("app.html"), b"<html>v1</html>").expect("writes");
        let promotion = provider
            .promote(&first.reference, bound())
            .expect("promotes");
        assert!(promotion.supported, "a seeded provider supports promotion");
        assert_eq!(promotion.changed.len(), 1);
        assert_eq!(promotion.changed[0].path, "app.html");
        assert!(promotion.deleted.is_empty());
        assert!(promotion.conflicts.is_empty());

        // A second unit's workspace is seeded with the FIRST unit's
        // promoted work — the whole point of carrying it forward.
        let second = provider
            .create(&run(), WorkspaceSubject::Unit(unit("AK-015")), bound())
            .expect("creates second");
        assert_eq!(
            std::fs::read(second.root.join("app.html")).expect("reads"),
            b"<html>v1</html>",
            "the second unit's workspace starts from the first unit's settled work"
        );
        assert_eq!(
            std::fs::read(second.root.join("seed.txt")).expect("reads"),
            b"seed"
        );
    }

    #[test]
    fn promote_reports_deletions_and_leaves_conflicts_untouched() {
        let (provider, project_root, integration_root) = seeded_provider("conflict");
        std::fs::write(project_root.join("keep.txt"), b"keep").expect("writes");
        std::fs::write(project_root.join("remove.txt"), b"gone-soon").expect("writes");
        let workspace = provider
            .create(&run(), WorkspaceSubject::Unit(unit("AK-014")), bound())
            .expect("creates");
        std::fs::remove_file(workspace.root.join("remove.txt")).expect("removes");

        // Simulate a concurrent change to the integration tree's copy of
        // "keep.txt" AFTER this workspace was seeded but BEFORE promote —
        // a conflict, never silently overwritten.
        let run_id = run();
        let tree = integration_root.join(run_id.as_str()).join("tree");
        std::fs::write(tree.join("keep.txt"), b"someone-else-changed-this").expect("writes");
        std::fs::write(
            workspace.root.join("keep.txt"),
            b"this-workspace-changed-it-too",
        )
        .expect("writes");

        let promotion = provider
            .promote(&workspace.reference, bound())
            .expect("promotes");
        assert_eq!(promotion.deleted, vec!["remove.txt".to_owned()]);
        assert_eq!(promotion.conflicts, vec!["keep.txt".to_owned()]);
        assert!(
            promotion.changed.is_empty(),
            "the conflicting path was not promoted: {:?}",
            promotion.changed
        );
        assert_eq!(
            std::fs::read(tree.join("keep.txt")).expect("reads"),
            b"someone-else-changed-this",
            "the integration tree's conflicting content is left untouched"
        );
    }

    #[test]
    fn promoting_an_unknown_workspace_is_a_typed_refusal() {
        let (provider, _project_root, _integration_root) = seeded_provider("unknown");
        let reference = WorkspaceRef::new("never-created").expect("valid ref shape");
        let result = provider.promote(&reference, bound());
        assert!(
            matches!(result, Err(WorkspaceProviderError::PromotionFailed { .. })),
            "an unknown workspace is a typed promotion refusal: {result:?}"
        );
    }

    #[test]
    fn promote_without_a_seed_is_unsupported() {
        let root = scratch("unseeded-promote");
        let provider = FsWorkspaceProvider::open(&root).expect("opens");
        let workspace = provider
            .create(&run(), WorkspaceSubject::Unit(unit("AK-014")), bound())
            .expect("creates");
        let promotion = provider
            .promote(&workspace.reference, bound())
            .expect("the default promotion never errors");
        assert!(!promotion.supported);
        assert!(promotion.changed.is_empty());
        assert!(promotion.deleted.is_empty());
        assert!(promotion.conflicts.is_empty());
    }
}
