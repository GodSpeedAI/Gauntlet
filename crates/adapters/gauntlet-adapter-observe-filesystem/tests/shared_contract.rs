//! The shared `Observer` contract suite (spec §7.3) run against this
//! adapter — UNMODIFIED, from `gauntlet_ports::contract::observer` — plus
//! the per-adapter provenance test (AK-028's substance: every observation
//! facet records instrument identity, exact command, environment
//! fingerprint, exit status, and a content-addressed evidence ref whose
//! stored bytes strict-decode as `facet_evidence@v1`).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use gauntlet_adapter_observe_filesystem::{
    FilesystemObserver, FilesystemObserverFactory, INSTRUMENT_NAME,
};
use gauntlet_domain::{EpochSeconds, EvidenceKind, EvidenceRef, IterationNo, RunId, WorkspaceRef};
use gauntlet_ports::clock::Clock;
use gauntlet_ports::contract::observer::observer_suite;
use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::evidence_store::{EvidenceStore, EvidenceStoreError};
use gauntlet_ports::observer::{FacetKind, ObservationRequest, Observer};
use gauntlet_ports::observer_evidence::{FACET_EVIDENCE_SCHEMA, FacetEvidenceDoc};

const ULID: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
const BASE_INSTANT: u64 = 1_700_000_000;

fn bound() -> Deadline {
    Deadline::at_epoch(EpochSeconds::new(BASE_INSTANT + 60))
}

/// A fixed clock: one declared instant, sleeps move nothing (the adapters
/// only read `now`).
struct FixedClock {
    now: EpochSeconds,
}

impl Clock for FixedClock {
    fn now(&self) -> EpochSeconds {
        self.now
    }

    fn sleep_until(&self, _at: EpochSeconds) -> Result<(), gauntlet_ports::clock::ClockError> {
        Ok(())
    }
}

/// In-test scratch evidence store (the ports crate's stubs are
/// `pub(crate)`-gated, and adapter-to-adapter dev-dependencies are
/// forbidden): ref-keyed bytes, monotonically assigned on put.
/// Mutex-protected so the Arc the observer holds is `Send` + `Sync`
/// (clippy keeps the store honest about that).
#[derive(Default)]
struct ScratchStore {
    blobs: Mutex<HashMap<EvidenceRef, Vec<u8>>>,
    next: AtomicU64,
}

impl EvidenceStore for ScratchStore {
    fn put(
        &self,
        bytes: &[u8],
        _kind: EvidenceKind,
        _deadline: Deadline,
    ) -> Result<EvidenceRef, EvidenceStoreError> {
        let n = self.next.fetch_add(1, Ordering::Relaxed);
        let reference = EvidenceRef::new(&format!("evd-observe-{n:06}")).map_err(|err| {
            EvidenceStoreError::WriteFailed {
                detail: err.to_string(),
            }
        })?;
        self.blobs
            .lock()
            .map_err(|_| poisoned_store("put"))?
            .insert(reference.clone(), bytes.to_vec());
        Ok(reference)
    }

    fn get(
        &self,
        reference: &EvidenceRef,
        _deadline: Deadline,
    ) -> Result<Option<Vec<u8>>, EvidenceStoreError> {
        Ok(self
            .blobs
            .lock()
            .map_err(|_| EvidenceStoreError::ReadFailed {
                detail: "the scratch store's lock was poisoned during get".to_owned(),
            })?
            .get(reference)
            .cloned())
    }

    fn pin(
        &self,
        _run_id: &RunId,
        _reference: &EvidenceRef,
        _deadline: Deadline,
    ) -> Result<(), EvidenceStoreError> {
        Ok(())
    }
}

/// Lock poisoning only follows a panic while held; carried as a typed
/// error rather than a second panic.
fn poisoned_store(op: &str) -> EvidenceStoreError {
    EvidenceStoreError::WriteFailed {
        detail: format!("the scratch store's lock was poisoned during {op}"),
    }
}

/// Isolated scratch workspace root per test; removed on drop (the
/// repo's hand-rolled Scratch idiom).
struct Scratch {
    path: PathBuf,
}

impl Scratch {
    fn new(tag: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "gauntlet-observe-filesystem-contract-{tag}-{}",
            std::process::id()
        ));
        std::fs::remove_dir_all(&path).ok();
        std::fs::create_dir_all(&path).expect("the contract scratch root is creatable");
        Self { path }
    }

    /// A workspace with real content: two files, one nested directory, one
    /// symlink pointing at the nested directory (listed, never followed).
    fn materialize(&self, key: &str) -> WorkspaceRef {
        let ws = self.path.join(key);
        let nested = ws.join("nested");
        std::fs::create_dir_all(&nested).expect("the nested directory is creatable");
        std::fs::write(ws.join("probe-file.txt"), b"filesystem probe content\n")
            .expect("the probe file is seedable");
        std::fs::write(nested.join("deep-file.txt"), b"nested probe content\n")
            .expect("the nested file is seedable");
        #[cfg(unix)]
        std::os::unix::fs::symlink("nested", ws.join("link-to-nested"))
            .expect("the probe symlink is creatable");
        WorkspaceRef::new(key).expect("the workspace ref is well formed")
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.path).ok();
    }
}

fn request(workspace: WorkspaceRef) -> ObservationRequest {
    ObservationRequest {
        run_id: RunId::new(ULID).expect("the suite run id is a ULID"),
        iteration_no: IterationNo::new(1).expect("iterations are 1-based"),
        workspace,
    }
}

fn observer_over(root: &Path, store: Arc<ScratchStore>) -> FilesystemObserver {
    FilesystemObserver::from_parts(
        root.to_path_buf(),
        Arc::new(FixedClock {
            now: EpochSeconds::new(BASE_INSTANT),
        }),
        store,
        &[(
            "GAUNTLET_PROBE_ENV".to_owned(),
            "filesystem-provenance".to_owned(),
        )],
    )
}

/// The unmodified Task-4 `Observer` contract suite against this adapter.
/// Empty findings = pass.
#[test]
fn the_filesystem_observer_passes_the_unmodified_shared_contract_suite() {
    let clock = Arc::new(FixedClock {
        now: EpochSeconds::new(BASE_INSTANT),
    });
    let store = Arc::new(ScratchStore::default());
    let factory = FilesystemObserverFactory::new(clock, store);
    let findings = observer_suite(&factory);
    assert!(
        findings.is_empty(),
        "the filesystem observer must pass the shared suite:\n{}",
        findings
            .iter()
            .map(|finding| finding.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// AK-028's substance, per facet: instrument identity (name + version),
/// exact command, environment fingerprint, recorded exit status, and an
/// evidence ref whose stored bytes strict-decode as `facet_evidence@v1`
/// with the exit status matching the facet.
#[test]
fn a_facet_carries_full_provenance_and_matching_evidence() {
    let scratch = Scratch::new("provenance");
    let workspace = scratch.materialize("ws-provenance");
    let store = Arc::new(ScratchStore::default());
    let observer = observer_over(&scratch.path, store.clone());

    let facet = observer
        .observe(&request(workspace), bound())
        .expect("the live workspace observes");
    assert_eq!(facet.kind(), FacetKind::Filesystem);
    assert_eq!(facet.instrument().name().as_str(), INSTRUMENT_NAME);
    assert!(!facet.instrument().version().is_empty());
    assert!(
        facet.command().is_some_and(|command| !command.is_empty()),
        "the walk contract is the recorded command"
    );
    assert_eq!(facet.exit_status(), Some(0));

    let bytes = store
        .get(facet.evidence_ref(), bound())
        .expect("the store read succeeds")
        .expect("the facet's evidence is present in the store");
    // The evidence document is UTF-8 by construction (the codec encodes a
    // String); lossy conversion is only a guard against store corruption.
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.contains(FACET_EVIDENCE_SCHEMA));
    let doc = FacetEvidenceDoc::decode(&text).expect("the evidence strict-decodes");
    assert_eq!(doc.instrument_name(), INSTRUMENT_NAME);
    assert!(!doc.instrument_version().is_empty());
    assert!(!doc.command().is_empty());
    assert!(!doc.env_fingerprint().is_empty());
    assert_eq!(
        doc.exit_status(),
        facet.exit_status().unwrap_or(i64::MAX),
        "the evidence document's exit status matches the facet's"
    );
    // The manifest is the walk's record: both files listed, relative paths.
    assert!(doc.stdout().contains("\"path\":\"probe-file.txt\""));
    assert!(doc.stdout().contains("\"path\":\"nested/deep-file.txt\""));
}

/// The walk is deterministic (same workspace, same bytes → same manifest)
/// and symlinks are listed without being followed: nothing under the link's
/// target path spelling appears in the manifest.
#[test]
fn the_manifest_is_deterministic_and_never_follows_symlinks() {
    let scratch = Scratch::new("determinism");
    let workspace = scratch.materialize("ws-determinism");
    let store = Arc::new(ScratchStore::default());
    let observer = observer_over(&scratch.path, store.clone());

    let first = observer
        .observe(&request(workspace.clone()), bound())
        .expect("the first walk observes");
    let second = observer
        .observe(&request(workspace), bound())
        .expect("the second walk observes");
    let first_doc = evidence_of(&store, &first);
    let second_doc = evidence_of(&store, &second);
    assert_eq!(
        first_doc.stdout(),
        second_doc.stdout(),
        "the same workspace bytes walk to the same manifest"
    );
    assert!(first_doc.stdout().contains("\"kind\":\"symlink\""));
    assert!(
        !first_doc.stdout().contains("link-to-nested/"),
        "the symlink is listed, never descended through"
    );
    // The nested file appears exactly once (through its real parent).
    let occurrences = first_doc.stdout().matches("nested/deep-file.txt").count();
    assert_eq!(occurrences, 1);
}

fn evidence_of(
    store: &ScratchStore,
    facet: &gauntlet_ports::observer::ObservationFacet,
) -> FacetEvidenceDoc {
    let bytes = store
        .get(facet.evidence_ref(), bound())
        .expect("the store read succeeds")
        .expect("the facet's evidence is present");
    FacetEvidenceDoc::decode(&String::from_utf8_lossy(&bytes)).expect("strict decode")
}
