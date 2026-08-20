//! The shared `Observer` contract suite (spec §7.3) run against this
//! adapter — UNMODIFIED, from `gauntlet_ports::contract::observer` — plus
//! the per-adapter provenance test (AK-028's substance) and the pinned
//! P3 core: a command that RUNS and exits nonzero is a facet carrying
//! that status, never an error. The generic adapter also pins that the
//! facet kind is exactly the caller's binding.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use gauntlet_adapter_observe_command::{
    CommandObserver, CommandObserverFactory, CommandSpec, INSTRUMENT_NAME,
};
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
            "gauntlet-observe-command-contract-{tag}-{}",
            std::process::id()
        ));
        std::fs::remove_dir_all(&path).ok();
        std::fs::create_dir_all(&path).expect("the contract scratch root is creatable");
        Self { path }
    }

    fn materialize(&self, key: &str) -> WorkspaceRef {
        std::fs::create_dir_all(self.path.join(key)).expect("the live workspace is creatable");
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

/// The unmodified Task-4 `Observer` contract suite against this adapter
/// (factory binding: the `test` facet class, the same binding the p3
/// proof gives the re-run self-check).
#[test]
fn the_command_observer_passes_the_unmodified_shared_contract_suite() {
    let factory = CommandObserverFactory::new(fixed_clock(), Arc::new(ScratchStore::default()));
    let findings = observer_suite(&factory);
    assert!(
        findings.is_empty(),
        "the command observer must pass the shared suite:\n{}",
        findings
            .iter()
            .map(|finding| finding.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// AK-028's substance, per facet — and P3's core in the same breath: a
/// command that runs and exits nonzero is a facet carrying exit 1 whose
/// evidence strict-decodes with the same status; the facet's kind is the
/// caller's binding, verbatim.
#[test]
fn a_facet_carries_full_provenance_and_a_failing_exit_is_still_a_facet() {
    let scratch = Scratch::new("provenance");
    let workspace = scratch.materialize("ws-provenance");
    let store = Arc::new(ScratchStore::default());
    let observer = CommandObserver::from_parts(
        CommandSpec::new("false", Vec::new()),
        FacetKind::Test,
        scratch.path.clone(),
        fixed_clock(),
        store.clone(),
        &[(
            "GAUNTLET_PROBE_ENV".to_owned(),
            "command-provenance".to_owned(),
        )],
    );

    let facet = observer
        .observe(&request(workspace), bound())
        .expect("a failing command is a successful observation");
    assert_eq!(
        facet.kind(),
        FacetKind::Test,
        "the facet carries the caller's binding"
    );
    assert_eq!(facet.instrument().name().as_str(), INSTRUMENT_NAME);
    assert!(!facet.instrument().version().is_empty());
    assert_eq!(facet.command(), Some("false"));
    assert_eq!(
        facet.exit_status(),
        Some(1),
        "P3: the observed failure IS the facet"
    );

    let bytes = store
        .get(facet.evidence_ref(), bound())
        .expect("the store read succeeds")
        .expect("the facet's evidence is present in the store");
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.contains(FACET_EVIDENCE_SCHEMA));
    let doc = FacetEvidenceDoc::decode(&text).expect("the evidence strict-decodes");
    assert_eq!(doc.instrument_name(), INSTRUMENT_NAME);
    assert_eq!(doc.command(), "false");
    assert!(!doc.env_fingerprint().is_empty());
    assert_eq!(
        doc.exit_status(),
        facet.exit_status().unwrap_or(i64::MAX),
        "the evidence document's exit status matches the facet's"
    );
}

/// A command whose program does not exist never ran: spawn failure is an
/// `ObserverError::Unavailable` (correction C8, Task-9 adjudication: a
/// binary that fails to spawn "could not run at all" — the same class the
/// versioning adapter uses; this test used to pin the drifted
/// `InstrumentFailed` spelling), never a fabricated facet.
#[test]
fn a_missing_program_is_unavailable_never_a_facet() {
    let scratch = Scratch::new("spawn-failure");
    let workspace = scratch.materialize("ws-spawn-failure");
    let observer = CommandObserver::from_parts(
        CommandSpec::new("no-such-gauntlet-probe-binary", Vec::new()),
        FacetKind::Test,
        scratch.path.clone(),
        fixed_clock(),
        Arc::new(ScratchStore::default()),
        &[("GAUNTLET_PROBE_ENV".to_owned(), "spawn-failure".to_owned())],
    );
    match observer.observe(&request(workspace), bound()) {
        Err(ObserverError::Unavailable { kind, detail }) => {
            assert_eq!(kind, FacetKind::Test);
            assert!(
                detail.contains("spawn"),
                "the detail names the failing step: {detail}"
            );
        }
        other => panic!("a spawn failure must be an error, got {other:?}"),
    }
}
