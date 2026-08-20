//! The shared `Observer` contract suite (spec §7.3) run against this
//! adapter — UNMODIFIED, from `gauntlet_ports::contract::observer` — plus
//! the per-adapter provenance test (AK-028's substance) and the pinned
//! nonzero-exit exception: git failing means no usable repository state.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use gauntlet_adapter_observe_git::{GitObserver, GitObserverFactory, INSTRUMENT_NAME};
use gauntlet_domain::{EpochSeconds, EvidenceKind, EvidenceRef, IterationNo, RunId, WorkspaceRef};
use gauntlet_ports::clock::Clock;
use gauntlet_ports::contract::observer::observer_suite;
use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::evidence_store::{EvidenceStore, EvidenceStoreError};
use gauntlet_ports::observer::{FacetKind, ObservationRequest, Observer, ObserverError};
use gauntlet_ports::observer_evidence::{FACET_EVIDENCE_SCHEMA, FacetEvidenceDoc};

const ULID: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
const BASE_INSTANT: u64 = 1_700_000_000;

fn bound() -> Deadline {
    Deadline::at_epoch(EpochSeconds::new(BASE_INSTANT + 60))
}

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

/// In-test scratch evidence store (adapter-to-adapter dev-dependencies are
/// forbidden; the ports stubs are `pub(crate)`): ref-keyed bytes,
/// monotonically assigned on put. Mutex-protected so the Arc the observer
/// holds is `Send` + `Sync` (clippy keeps the store honest about that).
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

/// Isolated scratch workspace root per test; removed on drop.
struct Scratch {
    path: PathBuf,
}

impl Scratch {
    fn new(tag: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "gauntlet-observe-git-contract-{tag}-{}",
            std::process::id()
        ));
        std::fs::remove_dir_all(&path).ok();
        std::fs::create_dir_all(&path).expect("the contract scratch root is creatable");
        Self { path }
    }

    /// A workspace that IS a freshly initialized repository (a real
    /// versioning state for the facet to observe).
    fn materialize_repo(&self, key: &str) -> WorkspaceRef {
        let dir = self.path.join(key);
        let output = Command::new("git").arg("init").arg(&dir).output().expect(
            "the versioning tool is required to materialize the suite repository (missing binary \
             is an environment failure, not a pass)",
        );
        assert!(
            output.status.success(),
            "materializing the suite repository failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        WorkspaceRef::new(key).expect("the workspace ref is well formed")
    }

    /// A workspace that is NOT a repository (for the exception test).
    fn materialize_plain(&self, key: &str) -> WorkspaceRef {
        let dir = self.path.join(key);
        std::fs::create_dir_all(&dir).expect("the plain workspace is creatable");
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

fn fixed_clock() -> Arc<FixedClock> {
    Arc::new(FixedClock {
        now: EpochSeconds::new(BASE_INSTANT),
    })
}

/// The unmodified Task-4 `Observer` contract suite against this adapter.
#[test]
fn the_versioning_observer_passes_the_unmodified_shared_contract_suite() {
    let factory = GitObserverFactory::new(fixed_clock(), Arc::new(ScratchStore::default()));
    let findings = observer_suite(&factory);
    assert!(
        findings.is_empty(),
        "the versioning observer must pass the shared suite:\n{}",
        findings
            .iter()
            .map(|finding| finding.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// AK-028's substance, per facet: instrument identity, exact command,
/// environment fingerprint, exit status, and a content-addressed evidence
/// ref whose stored bytes strict-decode as `facet_evidence@v1` with the
/// exit status matching the facet.
#[test]
fn a_facet_carries_full_provenance_and_matching_evidence() {
    let scratch = Scratch::new("provenance");
    let workspace = scratch.materialize_repo("ws-provenance");
    let store = Arc::new(ScratchStore::default());
    let observer = GitObserver::from_parts(
        scratch.path.clone(),
        fixed_clock(),
        store.clone(),
        &[(
            "GAUNTLET_PROBE_ENV".to_owned(),
            "versioning-provenance".to_owned(),
        )],
    );

    let facet = observer
        .observe(&request(workspace), bound())
        .expect("the live repository observes");
    assert_eq!(facet.kind(), FacetKind::Versioning);
    assert_eq!(facet.instrument().name().as_str(), INSTRUMENT_NAME);
    assert!(!facet.instrument().version().is_empty());
    assert_eq!(facet.command(), Some("git status --porcelain=v1 -b"));
    assert_eq!(facet.exit_status(), Some(0));

    let bytes = store
        .get(facet.evidence_ref(), bound())
        .expect("the store read succeeds")
        .expect("the facet's evidence is present in the store");
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.contains(FACET_EVIDENCE_SCHEMA));
    let doc = FacetEvidenceDoc::decode(&text).expect("the evidence strict-decodes");
    assert_eq!(doc.instrument_name(), INSTRUMENT_NAME);
    assert_eq!(doc.command(), "git status --porcelain=v1 -b");
    assert!(!doc.env_fingerprint().is_empty());
    assert_eq!(
        doc.exit_status(),
        facet.exit_status().unwrap_or(i64::MAX),
        "the evidence document's exit status matches the facet's"
    );
    assert!(
        doc.stdout().starts_with("## "),
        "the porcelain branch header is the evidence's first line: {:?}",
        doc.stdout().lines().next()
    );
}

/// The documented exception: git exiting nonzero over a non-repository is
/// `InstrumentFailed` carrying the exit code and stderr — never a facet
/// pretending a repository state exists.
#[test]
fn a_non_repository_maps_to_instrument_failed_with_the_exit_code() {
    let scratch = Scratch::new("exception");
    let workspace = scratch.materialize_plain("ws-not-a-repo");
    let observer = GitObserver::from_parts(
        scratch.path.clone(),
        fixed_clock(),
        Arc::new(ScratchStore::default()),
        &[(("GAUNTLET_PROBE_ENV".to_owned()), "exception".to_owned())],
    );
    match observer.observe(&request(workspace), bound()) {
        Err(ObserverError::InstrumentFailed { kind, detail }) => {
            assert_eq!(kind, FacetKind::Versioning);
            assert!(
                detail.contains("128") || detail.contains("exit status"),
                "the detail carries the exit code: {detail}"
            );
        }
        other => panic!("a non-repository must fail the instrument, got {other:?}"),
    }
}
