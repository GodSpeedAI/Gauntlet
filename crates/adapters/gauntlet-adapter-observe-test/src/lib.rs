//! Test observer adapter (spec §4.1, §12; plan Task 9): the test facet —
//! exit status and captured output from a declared test-runner instrument,
//! recorded with the exact command line and environment fingerprint used.
//!
//! The wrapper IS the instrument (plan Task 9 redesign trigger): the facet
//! names `gauntlet-observe-test`, and the exact wrapped command line is
//! recorded on the facet and in its `facet_evidence@v1` document. The
//! wrapped command is a CONSTRUCTOR parameter ([`TestCommand`]) — the tool
//! binding (which test runner, with which flags) is configuration resolved
//! by plan Tasks 15–16, never a literal here.
//!
//! Exit-status semantics (P3 core): a test run that RUNS and exits nonzero
//! (failing tests) is a successful observation of failing reality — a
//! facet carrying that exit status. Only an instrument that cannot run
//! (spawn failure, missing workspace, store write failure) or that dies by
//! signal (no exit status exists to record) is an [`ObserverError`].

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

/// The wrapper instrument's name — the capability is `test`
/// (`FacetKind::Test`); this names the adapter that observed.
pub const INSTRUMENT_NAME: &str = "gauntlet-observe-test";

/// The declared instrument this adapter wraps (plan Tasks 15–16 bind real
/// tools): the program and its arguments, verbatim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestCommand {
    pub program: String,
    pub args: Vec<String>,
}

impl TestCommand {
    pub fn new(program: impl Into<String>, args: Vec<String>) -> Self {
        Self {
            program: program.into(),
            args,
        }
    }
}

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

/// The test observer: wraps one declared [`TestCommand`], bound to a
/// workspace ROOT at construction (canonicalized then, C1); a request
/// resolves to a path under that canonical root, refused if it would
/// escape it.
pub struct TestObserver {
    command: TestCommand,
    root: PathBuf,
    clock: Arc<dyn Clock>,
    store: Arc<dyn EvidenceStore>,
    env_fingerprint: String,
}

impl TestObserver {
    /// Binds the observer to a declared test command and workspace root,
    /// fingerprinting the observing process's environment.
    pub fn new(
        command: TestCommand,
        root: PathBuf,
        clock: Arc<dyn Clock>,
        store: Arc<dyn EvidenceStore>,
    ) -> Self {
        let pairs: Vec<(String, String)> = std::env::vars().collect();
        Self {
            command,
            root: canonical_root(root),
            clock,
            store,
            env_fingerprint: env_fingerprint(&pairs),
        }
    }

    /// Test/composition-root constructor over an explicit environment.
    pub fn from_parts(
        command: TestCommand,
        root: PathBuf,
        clock: Arc<dyn Clock>,
        store: Arc<dyn EvidenceStore>,
        env_pairs: &[(String, String)],
    ) -> Self {
        Self {
            command,
            root: canonical_root(root),
            clock,
            store,
            env_fingerprint: env_fingerprint(env_pairs),
        }
    }
}

impl Observer for TestObserver {
    fn observe(
        &self,
        request: &ObservationRequest,
        deadline: Deadline,
    ) -> Result<ObservationFacet, ObserverError> {
        let kind = FacetKind::Test;
        let workspace = resolve_workspace(&self.root, request, kind)?;
        let query = command_line(&self.command.program, &self.command.args);
        let started = self.clock.now();
        let output = Command::new(&self.command.program)
            .args(&self.command.args)
            .current_dir(&workspace)
            .output()
            .map_err(|err| ObserverError::Unavailable {
                // C8: a program that cannot spawn "could not run at all" —
                // `Unavailable`, matching the versioning adapter. The
                // instrument never ran, so it cannot have failed.
                kind,
                detail: format!("spawning the test instrument failed: {err}"),
            })?;
        let ended = self.clock.now();
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        // ANY exit status is the observed reality (P3 core: failing tests
        // are the observation, not an observation failure). A signal death
        // produced no exit status — that is an instrument failure.
        let exit = match output.status.code() {
            Some(code) => i64::from(code),
            None => {
                return Err(ObserverError::InstrumentFailed {
                    kind,
                    detail: format!(
                        "the test instrument terminated by signal with no exit status; \
                         stderr so far: {stderr}"
                    ),
                });
            }
        };
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
            .with_summary(Some(format!("exit status {exit}"))))
    }

    fn facet_kind(&self) -> FacetKind {
        FacetKind::Test
    }
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
    vec![("GAUNTLET_OBSERVER_SUITE".to_owned(), "test".to_owned())]
}

/// Suite scaffolding for the shared `Observer` contract suite (spec §7.3):
/// `build` materializes each declared live workspace as a plain directory
/// under a per-build scratch root and returns an observer wrapping a
/// trivially successful command (`true`, resolved via PATH — the honest
/// form; no path is hardcoded). Production wiring resolves the real test
/// tool binding (plan Tasks 15–16). Scratch roots are removed on drop.
pub struct TestObserverFactory {
    clock: Arc<dyn Clock>,
    store: Arc<dyn EvidenceStore>,
    roots: std::cell::RefCell<Vec<PathBuf>>,
    next: std::cell::Cell<u32>,
}

impl TestObserverFactory {
    pub fn new(clock: Arc<dyn Clock>, store: Arc<dyn EvidenceStore>) -> Self {
        Self {
            clock,
            store,
            roots: std::cell::RefCell::new(Vec::new()),
            next: std::cell::Cell::new(0),
        }
    }
}

impl Drop for TestObserverFactory {
    fn drop(&mut self) {
        for root in self.roots.borrow().iter() {
            let _ = std::fs::remove_dir_all(root);
        }
    }
}

impl gauntlet_ports::contract::observer::ObserverFactory for TestObserverFactory {
    fn label(&self) -> &'static str {
        "gauntlet-observe-test"
    }

    fn kind(&self) -> FacetKind {
        FacetKind::Test
    }

    fn build(&self, live: Vec<WorkspaceRef>) -> Result<Box<dyn Observer>, String> {
        let seq = self.next.get();
        self.next.set(seq.wrapping_add(1));
        let root = std::env::temp_dir().join(format!(
            "gauntlet-observe-test-{}-{seq}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root)
            .map_err(|err| format!("creating the suite scratch root failed: {err}"))?;
        for workspace in &live {
            std::fs::create_dir_all(root.join(workspace.as_str()))
                .map_err(|err| format!("materializing a live workspace failed: {err}"))?;
        }
        self.roots.borrow_mut().push(root.clone());
        Ok(Box::new(TestObserver::from_parts(
            TestCommand::new("true", Vec::new()),
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
                "gauntlet-observe-test-containment-{tag}-{}",
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
                FacetKind::Test,
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
            FacetKind::Test,
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
            resolve_workspace(&canonical_root, &containment_request("ws"), FacetKind::Test).is_ok()
        );
        let linked_root = scratch.path.join("root-link");
        std::os::unix::fs::symlink(&root, &linked_root).expect("the root symlink is creatable");
        let through_link = linked_root.canonicalize().expect("the link canonicalizes");
        assert_eq!(through_link, canonical_root);
        assert!(
            resolve_workspace(&through_link, &containment_request("ws"), FacetKind::Test).is_ok()
        );
    }

    /// C8 (Task-9 adjudication): a test instrument whose program cannot
    /// spawn "could not run at all" — the port's `Unavailable` class, the
    /// one the versioning adapter already used — not `InstrumentFailed`
    /// (nothing ran, so nothing failed; that class is reserved for an
    /// instrument that RAN).
    #[test]
    fn a_missing_program_classifies_as_unavailable() {
        let scratch = Scratch::new("spawn");
        let root = scratch.dir("root");
        std::fs::create_dir_all(root.join("ws")).expect("the workspace is creatable");
        let observer = TestObserver::from_parts(
            TestCommand::new("no-such-gauntlet-probe-binary", Vec::new()),
            root,
            Arc::new(NeverClock),
            Arc::new(NeverStore),
            &[],
        );
        match observer.observe(
            &containment_request("ws"),
            Deadline::at_epoch(gauntlet_domain::EpochSeconds::new(0)),
        ) {
            Err(ObserverError::Unavailable { detail, .. }) => {
                assert!(
                    detail.contains("spawning"),
                    "the refusal names the spawn failure: {detail}"
                );
            }
            other => panic!("a spawn failure must be Unavailable, got {other:?}"),
        }
    }

    struct NeverClock;

    impl Clock for NeverClock {
        fn now(&self) -> gauntlet_domain::EpochSeconds {
            gauntlet_domain::EpochSeconds::new(0)
        }

        fn sleep_until(
            &self,
            _at: gauntlet_domain::EpochSeconds,
        ) -> Result<(), gauntlet_ports::clock::ClockError> {
            Ok(())
        }
    }

    struct NeverStore;

    impl EvidenceStore for NeverStore {
        fn put(
            &self,
            _bytes: &[u8],
            _kind: gauntlet_domain::EvidenceKind,
            _deadline: Deadline,
        ) -> Result<gauntlet_domain::EvidenceRef, gauntlet_ports::evidence_store::EvidenceStoreError>
        {
            Err(
                gauntlet_ports::evidence_store::EvidenceStoreError::WriteFailed {
                    detail: "the spawn-class test never stores".to_owned(),
                },
            )
        }

        fn get(
            &self,
            _reference: &gauntlet_domain::EvidenceRef,
            _deadline: Deadline,
        ) -> Result<Option<Vec<u8>>, gauntlet_ports::evidence_store::EvidenceStoreError> {
            Err(
                gauntlet_ports::evidence_store::EvidenceStoreError::ReadFailed {
                    detail: "the spawn-class test never reads".to_owned(),
                },
            )
        }

        fn pin(
            &self,
            _run_id: &gauntlet_domain::RunId,
            _reference: &gauntlet_domain::EvidenceRef,
            _deadline: Deadline,
        ) -> Result<(), gauntlet_ports::evidence_store::EvidenceStoreError> {
            Err(
                gauntlet_ports::evidence_store::EvidenceStoreError::PinFailed {
                    reference: _reference.clone(),
                    detail: "the spawn-class test never pins".to_owned(),
                },
            )
        }
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
}
