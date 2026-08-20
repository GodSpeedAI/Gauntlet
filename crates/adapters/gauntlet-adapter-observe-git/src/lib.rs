//! Versioning observer adapter (spec §4.1, §12; plan Task 9): the
//! versioning facet (`FacetKind::Versioning` — the operator's capability
//! rename, absorbed; this crate's product-named path stays by the recorded
//! §24.3 decision) — repository state observed as instrument-produced
//! evidence with recorded provenance and a content-addressed evidence ref.
//!
//! The wrapper IS the instrument (plan Task 9 redesign trigger): the facet
//! names `gauntlet-observe-git`, the adapter, and records the exact wrapped
//! command line. Git's own version is deliberately NOT probed (uniformity
//! across the five observe adapters — every facet's instrument is its
//! wrapper; a recorded limitation for the critic).
//!
//! Exit-status semantics — the ONE Task-9 exception, documented: a command
//! that runs and exits nonzero is normally a facet carrying that status,
//! but git exiting nonzero means there is NO usable repository state, so
//! this adapter maps it to [`ObserverError::InstrumentFailed`] with the
//! exit code and captured stderr in the detail. A spawn failure (the git
//! binary missing) is [`ObserverError::Unavailable`]. A git killed by a
//! signal produced no exit status at all — also `InstrumentFailed`.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use gauntlet_domain::{EvidenceKind, InstrumentName, WorkspaceRef};
use gauntlet_ports::clock::Clock;
use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::evidence_store::EvidenceStore;
use gauntlet_ports::observer::{
    FacetKind, InstrumentIdentity, ObservationFacet, ObservationRequest, Observer, ObserverError,
};
use gauntlet_ports::observer_evidence::{FacetEvidenceDoc, ObservedRun, command_line};
use sha2::{Digest as _, Sha256};

/// The wrapper instrument's name — the capability is `versioning`
/// (`FacetKind::Versioning`); this names the adapter that observed.
pub const INSTRUMENT_NAME: &str = "gauntlet-observe-git";

/// The versioning query this adapter wraps: branch + worktree state in the
/// machine-parsable v1 porcelain format, with the branch header line.
const STATUS_ARGS: &[&str] = &["status", "--porcelain=v1", "-b"];

/// The environment fingerprint of an explicit pair list: the sorted
/// `KEY=VALUE` lines, SHA-256, rendered `sha256:<hex>` (duplicated per
/// adapter — adapter-to-adapter dependencies are forbidden).
pub fn env_fingerprint(pairs: &[(String, String)]) -> String {
    let mut lines: Vec<String> = pairs
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect();
    lines.sort();
    let mut hasher = Sha256::new();
    for line in &lines {
        hasher.update(line.as_bytes());
        hasher.update(b"\n");
    }
    let hex: String = hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    format!("sha256:{hex}")
}

/// The versioning observer: bound to a workspace ROOT at construction
/// (canonicalized then, C1); a request resolves to a path under that
/// canonical root, refused if it would escape it.
pub struct GitObserver {
    root: PathBuf,
    clock: Arc<dyn Clock>,
    store: Arc<dyn EvidenceStore>,
    env_fingerprint: String,
}

impl GitObserver {
    /// Binds the observer to a workspace root, fingerprinting the observing
    /// process's environment.
    pub fn new(root: PathBuf, clock: Arc<dyn Clock>, store: Arc<dyn EvidenceStore>) -> Self {
        let pairs: Vec<(String, String)> = std::env::vars().collect();
        Self {
            root: canonical_root(root),
            clock,
            store,
            env_fingerprint: env_fingerprint(&pairs),
        }
    }

    /// Test/composition-root constructor over an explicit environment.
    pub fn from_parts(
        root: PathBuf,
        clock: Arc<dyn Clock>,
        store: Arc<dyn EvidenceStore>,
        env_pairs: &[(String, String)],
    ) -> Self {
        Self {
            root: canonical_root(root),
            clock,
            store,
            env_fingerprint: env_fingerprint(env_pairs),
        }
    }
}

impl Observer for GitObserver {
    fn observe(
        &self,
        request: &ObservationRequest,
        deadline: Deadline,
    ) -> Result<ObservationFacet, ObserverError> {
        let kind = FacetKind::Versioning;
        let workspace = resolve_workspace(&self.root, request, kind)?;
        let args: Vec<String> = STATUS_ARGS.iter().map(|arg| (*arg).to_owned()).collect();
        let query = command_line("git", &args);
        let started = self.clock.now();
        let output = Command::new("git")
            .args(STATUS_ARGS)
            .current_dir(&workspace)
            .output()
            .map_err(|err| ObserverError::Unavailable {
                kind,
                detail: format!("spawning the versioning instrument failed: {err}"),
            })?;
        let ended = self.clock.now();
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        // The documented exception: git's own nonzero exit means no usable
        // repository state — an instrument failure, never a facet.
        let exit = match output.status.code() {
            Some(code) => i64::from(code),
            None => {
                return Err(ObserverError::InstrumentFailed {
                    kind,
                    detail: format!(
                        "the versioning instrument terminated by signal with no exit status; \
                         stderr so far: {stderr}"
                    ),
                });
            }
        };
        if exit != 0 {
            return Err(ObserverError::InstrumentFailed {
                kind,
                detail: format!("exit status {exit}; stderr: {stderr}"),
            });
        }
        let summary = summarize(&stdout);
        let doc = FacetEvidenceDoc::new(
            INSTRUMENT_NAME.to_owned(),
            env!("CARGO_PKG_VERSION").to_owned(),
            ObservedRun {
                command: query.clone(),
                env_fingerprint: self.env_fingerprint.clone(),
                exit_status: exit,
                started_at: started.get(),
                ended_at: ended.get(),
                stdout,
                stderr,
            },
        )
        .map_err(|err| ObserverError::InstrumentFailed {
            kind,
            detail: format!("building the facet evidence document failed: {err}"),
        })?;
        let evidence_ref = self
            .store
            .put(doc.encode().as_bytes(), EvidenceKind::Log, deadline)
            .map_err(|err| ObserverError::Unavailable {
                kind,
                detail: format!("storing the facet evidence document failed: {err}"),
            })?;
        let name =
            InstrumentName::new(INSTRUMENT_NAME).map_err(|err| ObserverError::Unavailable {
                kind,
                detail: format!("the instrument name is not a valid InstrumentName: {err}"),
            })?;
        let instrument = InstrumentIdentity::new(name, env!("CARGO_PKG_VERSION").to_owned());
        Ok(ObservationFacet::new(kind, instrument, evidence_ref)
            .with_invocation(Some(query), Some(exit))
            .with_window(Some(started), Some(ended))
            .with_summary(Some(summary)))
    }

    fn facet_kind(&self) -> FacetKind {
        FacetKind::Versioning
    }
}

/// One branch/state line of the porcelain output, verbatim.
fn summarize(stdout: &str) -> String {
    let first = stdout.lines().next().unwrap_or("");
    format!("porcelain status: {first}")
}

/// The canonical workspace root, computed once at construction (C1): the
/// root itself may legitimately be a symlink (operator configuration), so
/// canonicalizing THROUGH it is correct — what containment requires is
/// that no WORKSPACE resolves outside the canonical root. A root that
/// does not exist yet is stored unresolved; any observe of a workspace
/// under it then fails the containment prefix check fail-closed.
fn canonical_root(root: PathBuf) -> PathBuf {
    root.canonicalize().unwrap_or(root)
}

/// Resolves a request's workspace under the observer's CANONICAL root,
/// refusing every ref that could resolve outside it (correction C1):
/// `.`/`..` components are refused outright, and the resolved path must
/// canonicalize back under the root — a symlinked workspace pointing
/// outside the root is refused, never followed. Existence on disk is the
/// truth (no membership list): a ref whose path does not exist is
/// `Unavailable`.
fn resolve_workspace(
    root: &Path,
    request: &ObservationRequest,
    kind: FacetKind,
) -> Result<PathBuf, ObserverError> {
    let reference = request.workspace.as_str();
    if let Some(part) = reference
        .split('/')
        .find(|part| *part == ".." || *part == ".")
    {
        return Err(ObserverError::Unavailable {
            kind,
            detail: format!(
                "workspace {reference} is refused: the '{part}' path component is a containment \
                 violation — a workspace must resolve inside the observer root"
            ),
        });
    }
    let resolved = match root.join(reference).canonicalize() {
        Ok(resolved) => resolved,
        Err(_) => {
            return Err(ObserverError::Unavailable {
                kind,
                detail: format!("workspace {reference} does not exist under the observer root"),
            });
        }
    };
    if !resolved.starts_with(root) {
        return Err(ObserverError::Unavailable {
            kind,
            detail: format!(
                "workspace {reference} resolves to {}, outside the canonical observer root {} — \
                 a containment violation (a symlinked workspace pointing outside the root is \
                 refused, never followed)",
                resolved.display(),
                root.display()
            ),
        });
    }
    Ok(resolved)
}

/// The deterministic environment the suite factory fingerprints.
fn suite_env() -> Vec<(String, String)> {
    vec![(
        "GAUNTLET_OBSERVER_SUITE".to_owned(),
        "versioning".to_owned(),
    )]
}

/// Suite scaffolding for the shared `Observer` contract suite (spec §7.3):
/// `build` materializes each declared live workspace as a REAL freshly
/// initialized repository under a per-build scratch root (the suite's
/// success case must observe a genuine versioning state; the git binary is
/// required, and a missing binary is an honest factory error the suite
/// reports). Production wiring composes this adapter through the plan Task
/// 12/15 controller. Scratch roots are removed on drop.
pub struct GitObserverFactory {
    clock: Arc<dyn Clock>,
    store: Arc<dyn EvidenceStore>,
    roots: std::cell::RefCell<Vec<PathBuf>>,
    next: std::cell::Cell<u32>,
}

impl GitObserverFactory {
    pub fn new(clock: Arc<dyn Clock>, store: Arc<dyn EvidenceStore>) -> Self {
        Self {
            clock,
            store,
            roots: std::cell::RefCell::new(Vec::new()),
            next: std::cell::Cell::new(0),
        }
    }
}

impl Drop for GitObserverFactory {
    fn drop(&mut self) {
        for root in self.roots.borrow().iter() {
            let _ = std::fs::remove_dir_all(root);
        }
    }
}

impl gauntlet_ports::contract::observer::ObserverFactory for GitObserverFactory {
    fn label(&self) -> &'static str {
        "gauntlet-observe-git"
    }

    fn kind(&self) -> FacetKind {
        FacetKind::Versioning
    }

    fn build(&self, live: Vec<WorkspaceRef>) -> Result<Box<dyn Observer>, String> {
        let seq = self.next.get();
        self.next.set(seq.wrapping_add(1));
        let root =
            std::env::temp_dir().join(format!("gauntlet-observe-git-{}-{seq}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root)
            .map_err(|err| format!("creating the suite scratch root failed: {err}"))?;
        for workspace in &live {
            let dir = root.join(workspace.as_str());
            let output = Command::new("git")
                .arg("init")
                .arg(&dir)
                .output()
                .map_err(|err| {
                    format!("spawning the versioning tool to materialize a suite workspace failed: {err}")
                })?;
            if !output.status.success() {
                return Err(format!(
                    "initializing a suite repository failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                ));
            }
        }
        self.roots.borrow_mut().push(root.clone());
        Ok(Box::new(GitObserver::from_parts(
            root,
            self.clock.clone(),
            self.store.clone(),
            &suite_env(),
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gauntlet_domain::{IterationNo, RunId};

    /// Per-test scratch tree; removed on drop.
    struct Scratch {
        path: PathBuf,
    }

    impl Scratch {
        fn new(tag: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "gauntlet-observe-git-containment-{tag}-{}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).expect("the containment scratch parent is creatable");
            Self { path }
        }

        fn dir(&self, name: &str) -> PathBuf {
            let dir = self.path.join(name);
            std::fs::create_dir_all(&dir).expect("the containment scratch dir is creatable");
            dir
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    fn containment_request(workspace: &str) -> ObservationRequest {
        ObservationRequest {
            run_id: RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("valid run id"),
            iteration_no: IterationNo::new(1).expect("iterations are 1-based"),
            workspace: WorkspaceRef::new(workspace).expect("valid workspace ref"),
        }
    }

    /// Containment (correction C1): a workspace ref can never resolve the
    /// instrument's working directory outside the observer's canonical
    /// root — `.`/`..` components are refused outright, a symlinked
    /// workspace pointing outside the root is refused (never followed),
    /// while a plain ref and a workspace under a symlinked ROOT (operator
    /// config, canonicalized through at construction) both still resolve.
    #[test]
    fn workspace_resolution_is_contained_inside_the_observer_root() {
        let scratch = Scratch::new("root");
        let root = scratch.dir("root");
        std::fs::create_dir_all(root.join("ws")).expect("the live workspace is creatable");
        scratch.dir("sibling");
        let outside = scratch.dir("outside");
        std::os::unix::fs::symlink(&outside, root.join("ws-out"))
            .expect("the outside-pointing symlink is creatable");
        let canonical_root = root.canonicalize().expect("the scratch root canonicalizes");

        for escape in ["../sibling", "./ws"] {
            match resolve_workspace(
                &canonical_root,
                &containment_request(escape),
                FacetKind::Versioning,
            ) {
                Err(ObserverError::Unavailable { detail, .. }) => {
                    assert!(
                        detail.contains("containment"),
                        "the refusal names containment for {escape}: {detail}"
                    );
                }
                other => panic!("ref {escape} must be refused, got {other:?}"),
            }
        }
        match resolve_workspace(
            &canonical_root,
            &containment_request("ws-out"),
            FacetKind::Versioning,
        ) {
            Err(ObserverError::Unavailable { detail, .. }) => {
                assert!(
                    detail.contains("containment") && detail.contains("symlink"),
                    "the refusal names the symlinked-workspace containment violation: {detail}"
                );
            }
            other => panic!("a symlink outside the root must be refused, got {other:?}"),
        }

        assert!(
            resolve_workspace(
                &canonical_root,
                &containment_request("ws"),
                FacetKind::Versioning
            )
            .is_ok()
        );
        let linked_root = scratch.path.join("root-link");
        std::os::unix::fs::symlink(&root, &linked_root).expect("the root symlink is creatable");
        let through_link = linked_root.canonicalize().expect("the link canonicalizes");
        assert_eq!(through_link, canonical_root);
        assert!(
            resolve_workspace(
                &through_link,
                &containment_request("ws"),
                FacetKind::Versioning
            )
            .is_ok()
        );
    }

    #[test]
    fn the_env_fingerprint_is_sorted_pairs_over_sha256() {
        let pairs = vec![
            ("B".to_owned(), "2".to_owned()),
            ("A".to_owned(), "x=y".to_owned()),
        ];
        let mut hasher = Sha256::new();
        hasher.update(b"A=x=y\n");
        hasher.update(b"B=2\n");
        let hex: String = hasher
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        let expected = format!("sha256:{hex}");
        assert_eq!(env_fingerprint(&pairs), expected);
        let mut reversed = pairs.clone();
        reversed.reverse();
        assert_eq!(env_fingerprint(&reversed), expected);
        let other = vec![
            ("B".to_owned(), "2".to_owned()),
            ("A".to_owned(), "z".to_owned()),
        ];
        assert_ne!(env_fingerprint(&other), expected);
    }

    #[test]
    fn the_wrapped_command_line_is_the_recorded_query() {
        let args: Vec<String> = STATUS_ARGS.iter().map(|arg| (*arg).to_owned()).collect();
        assert_eq!(command_line("git", &args), "git status --porcelain=v1 -b");
    }
}
