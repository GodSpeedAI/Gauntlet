//! G6 anti-gaming proof (plan Task 9; AK-030): a failed observation can
//! never be hidden — unavailable evidence and observation failures are
//! recorded explicitly, never dropped, never defaulted to success.
//!
//! Location rationale (recorded per the task brief): the plan spells the
//! proof `tests/antigaming/g6_hide_failed_observations`, but the
//! architecture boundary suite's cargo-target-path rule (exemption list
//! exactly `crates/gauntlet-arch-tests`) forbids a core crate from
//! declaring a repository-root test target — the same
//! locator-materialization class first recorded 2026-08-17 against
//! AK-049. The proof therefore lives as an integration test of the
//! composition root with the target NAME the plan's gate uses
//! (`cargo nextest run --test g6_hide_failed_observations`;
//! `p1_reference_integrity.rs` precedent).
//!
//! The three faces of G6, composed through the real assembler and a real
//! content-addressed store:
//!
//! 1. a probe whose program does not exist (spawn failure) plus a healthy
//!    probe → one facet, BOTH gap arrays naming exactly the failed kind,
//!    `has_gaps()` true, and the gap arrays are covered by the digest;
//! 2. a store that fails the probe's evidence write → the OBSERVER
//!    surfaces an error → the same double gap recording;
//! 3. an all-healthy bundle whose gap arrays are EMPTY, explicitly, as
//!    populated types (the positive statement — absence of gaps is
//!    attested, not assumed).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use gauntlet_adapter_evidence_fs::FsEvidenceStore;
use gauntlet_adapter_observe_command::{CommandObserver, CommandSpec};
use gauntlet_adapter_observe_filesystem::FilesystemObserver;
use gauntlet_app::observation::assembly::{ObservationAssembler, ObservationPlan};
use gauntlet_app::observation::bundle::ClaimsFacet;
use gauntlet_app::observation::digest::observation_digest;
use gauntlet_app::observation::input::VerificationInputs;
use gauntlet_domain::{EpochSeconds, EvidenceKind, EvidenceRef, IterationNo, RunId, WorkspaceRef};
use gauntlet_ports::clock::Clock;
use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::evidence_store::{EvidenceStore, EvidenceStoreError};
use gauntlet_ports::observer::{FacetKind, ObservationRequest};

const ULID: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
const BASE_INSTANT: u64 = 1_700_000_000;
const BOUND_SPAN: u64 = 60;
const WORKSPACE_KEY: &str = "ws-g6";
/// A program name that cannot exist on this host (single token; resolved
/// via PATH, so spawn fails).
const MISSING_PROGRAM: &str = "no-such-gauntlet-probe-binary";

/// Isolated scratch directory per test; removed on drop.
struct Scratch {
    path: PathBuf,
}

impl Scratch {
    fn new(tag: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "gauntlet-g6-observation-{tag}-{}",
            std::process::id()
        ));
        std::fs::remove_dir_all(&path).ok();
        std::fs::create_dir_all(&path).expect("the g6 scratch parent is creatable");
        Self { path }
    }

    fn workspaces_root(&self) -> PathBuf {
        let root = self.path.join("workspaces");
        std::fs::create_dir_all(root.join(WORKSPACE_KEY)).expect("the g6 workspace is creatable");
        root
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.path).ok();
    }
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

fn clock() -> Arc<FixedClock> {
    Arc::new(FixedClock {
        now: EpochSeconds::new(BASE_INSTANT),
    })
}

fn bound() -> Deadline {
    Deadline::at_epoch(EpochSeconds::new(BASE_INSTANT + BOUND_SPAN))
}

fn proof_env() -> Vec<(String, String)> {
    vec![(
        "GAUNTLET_PROOF_ENV".to_owned(),
        "g6-hide-failed-observations".to_owned(),
    )]
}

fn request() -> ObservationRequest {
    ObservationRequest {
        run_id: RunId::new(ULID).expect("valid run id"),
        iteration_no: IterationNo::new(1).expect("iterations are 1-based"),
        workspace: WorkspaceRef::new(WORKSPACE_KEY).expect("valid workspace ref"),
    }
}

/// A store whose `put` always fails (the g6 store-failure shape); `get`
/// answers from an empty map.
#[derive(Default)]
struct FailingStore {
    puts: AtomicU64,
}

impl EvidenceStore for FailingStore {
    fn put(
        &self,
        _bytes: &[u8],
        _kind: EvidenceKind,
        _deadline: Deadline,
    ) -> Result<EvidenceRef, EvidenceStoreError> {
        self.puts.fetch_add(1, Ordering::Relaxed);
        Err(EvidenceStoreError::WriteFailed {
            detail: "the failing store refuses every write".to_owned(),
        })
    }

    fn get(
        &self,
        _reference: &EvidenceRef,
        _deadline: Deadline,
    ) -> Result<Option<Vec<u8>>, EvidenceStoreError> {
        Ok(None)
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

/// An in-memory store for the assembler in the store-failure case (the
/// assembler must still be able to record the BUNDLE; only the probe's
/// store fails).
#[derive(Default)]
struct MapStore {
    blobs: Mutex<HashMap<EvidenceRef, Vec<u8>>>,
    next: AtomicU64,
}

impl EvidenceStore for MapStore {
    fn put(
        &self,
        bytes: &[u8],
        _kind: EvidenceKind,
        _deadline: Deadline,
    ) -> Result<EvidenceRef, EvidenceStoreError> {
        let n = self.next.fetch_add(1, Ordering::Relaxed);
        let reference = EvidenceRef::new(&format!("evd-g6-{n:06}")).map_err(|err| {
            EvidenceStoreError::WriteFailed {
                detail: err.to_string(),
            }
        })?;
        self.blobs
            .lock()
            .map_err(|_| EvidenceStoreError::WriteFailed {
                detail: "the map store's lock was poisoned during put".to_owned(),
            })?
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
                detail: "the map store's lock was poisoned during get".to_owned(),
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

/// Face 1: a probe whose program does not exist plus a healthy probe. The
/// failed probe appears in BOTH required arrays exactly once, the healthy
/// facet survives, the domain view reports gaps, and the gap arrays are
/// covered by the digest (a rebuilt document with a mutated gap array
/// digests differently — the gap records cannot be silently edited).
#[test]
fn a_failed_probe_is_recorded_in_both_arrays_and_covered_by_the_digest() {
    let scratch = Scratch::new("missing-program");
    let workspaces_root = scratch.workspaces_root();
    let store = Arc::new(
        FsEvidenceStore::open(&scratch.path.join("evidence")).expect("the evidence store opens"),
    );
    let env = proof_env();
    let fingerprint = gauntlet_adapter_observe_command::env_fingerprint(&env);

    let healthy =
        FilesystemObserver::from_parts(workspaces_root.clone(), clock(), store.clone(), &env);
    let missing = CommandObserver::from_parts(
        CommandSpec::new(MISSING_PROGRAM, Vec::new()),
        FacetKind::Test,
        workspaces_root,
        clock(),
        store.clone(),
        &env,
    );
    let plan = ObservationPlan {
        request: request(),
        observers: vec![Box::new(healthy), Box::new(missing)],
        project_revision: "rev-proof-01".to_owned(),
        environment_fingerprint: fingerprint,
        claims_content: ClaimsFacet::render_content(&["the work is done".to_owned()]),
        clock: clock(),
    };
    let assembled = ObservationAssembler::new(store)
        .assemble(plan, bound())
        .expect("one failed probe never fails the whole assembly");
    let doc = &assembled.doc;

    assert_eq!(
        doc.facets().len(),
        1,
        "only the healthy probe produced a facet"
    );
    assert_eq!(doc.facets()[0].kind(), "filesystem");
    assert_eq!(
        doc.unavailable_evidence().len(),
        1,
        "the failed probe is recorded unavailable evidence (§21.3)"
    );
    assert_eq!(doc.unavailable_evidence()[0].kind(), "test");
    assert_eq!(doc.observation_failures().len(), 1);
    assert_eq!(doc.observation_failures()[0].kind(), "test");
    assert!(!doc.observation_failures()[0].error_class().is_empty());
    // Correction C8 (Task-9 adjudication, from critic-B BIT-7b + critic-A
    // m1): a binary that fails to SPAWN "could not run at all" — the
    // port's own `Unavailable` class, the one the versioning adapter
    // already used. This proof used to pin the drifted `instrument_failed`
    // spelling; that drift WAS the defect. BOTH gap arrays still populate
    // (asserted above) — only the recorded error class changed.
    assert!(
        doc.observation_failures()[0].error_class() == "unavailable",
        "a spawn failure classifies as unavailable"
    );
    assert!(doc.as_domain_view().has_gaps());

    // The gap entries are verification inputs too (I8 downstream).
    let inputs = VerificationInputs::of_bundle(doc, assembled.bundle_ref.clone());
    assert_eq!(inputs.unavailable_evidence().len(), 1);
    assert_eq!(inputs.observation_failures().len(), 1);

    // Gap coverage by the digest: rebuild the document with one more gap
    // entry and the digest MUST change — hidden edits to the gap arrays
    // are tamper-evident.
    let base_digest = observation_digest(doc);
    let mut mutated = doc.content();
    mutated.observation_failures.push(
        gauntlet_app::observation::bundle::ObservationFailureRecord::new(
            "browser".to_owned(),
            "instrument_failed".to_owned(),
            "a hypothetically hidden failure".to_owned(),
        ),
    );
    let mutated_doc = gauntlet_app::observation::bundle::ObservationBundleDoc::new(mutated);
    assert_ne!(
        observation_digest(&mutated_doc),
        base_digest,
        "the gap arrays are inside the digest's coverage"
    );
}

/// Face 2: the probe's evidence write fails — the OBSERVER surfaces the
/// error, and the assembler records it as the same double gap. The
/// assembler's own store still works, so the BUNDLE (carrying the gap
/// records) is durably written.
#[test]
fn a_probe_store_write_failure_surfaces_as_gap_entries() {
    let scratch = Scratch::new("store-failure");
    let workspaces_root = scratch.workspaces_root();
    let failing = Arc::new(FailingStore::default());
    let working = Arc::new(MapStore::default());
    let env = proof_env();

    let healthy =
        FilesystemObserver::from_parts(workspaces_root.clone(), clock(), working.clone(), &env);
    // The starved probe runs a command that EXISTS (`true`) against the
    // failing store: the instrument runs, exits 0, and the evidence WRITE
    // is what fails — a different failure locus from the spawn failure of
    // face 1, and the one the store-failure case is about.
    let starved = CommandObserver::from_parts(
        CommandSpec::new("true", Vec::new()),
        FacetKind::Test,
        workspaces_root,
        clock(),
        failing.clone(),
        &env,
    );
    let plan = ObservationPlan {
        request: request(),
        observers: vec![Box::new(healthy), Box::new(starved)],
        project_revision: "rev-proof-01".to_owned(),
        environment_fingerprint: gauntlet_adapter_observe_command::env_fingerprint(&env),
        claims_content: ClaimsFacet::render_content(&["the work is done".to_owned()]),
        clock: clock(),
    };
    let assembled = ObservationAssembler::new(working)
        .assemble(plan, bound())
        .expect("the failing OBSERVER is a gap, not an assembly failure");
    assert!(
        failing.puts.load(Ordering::Relaxed) > 0,
        "the starved probe actually attempted a store write"
    );
    let doc = &assembled.doc;
    assert_eq!(doc.facets().len(), 1);
    assert_eq!(doc.unavailable_evidence()[0].kind(), "test");
    assert_eq!(doc.observation_failures()[0].kind(), "test");
    assert!(doc.as_domain_view().has_gaps());
    // The bundle itself WAS durably written (its bytes resolve).
    assert!(!assembled.bundle_ref.as_str().is_empty());
}

/// Face 3 (the positive statement): an all-healthy bundle carries EMPTY
/// gap arrays as populated types — absence attested, never assumed
/// (typed, not `Option`).
#[test]
fn an_all_healthy_bundle_attests_empty_gap_arrays_explicitly() {
    let scratch = Scratch::new("all-healthy");
    let workspaces_root = scratch.workspaces_root();
    let store = Arc::new(
        FsEvidenceStore::open(&scratch.path.join("evidence")).expect("the evidence store opens"),
    );
    let env = proof_env();
    let filesystem =
        FilesystemObserver::from_parts(workspaces_root.clone(), clock(), store.clone(), &env);
    let command = CommandObserver::from_parts(
        CommandSpec::new("true", Vec::new()),
        FacetKind::Test,
        workspaces_root,
        clock(),
        store.clone(),
        &env,
    );
    let plan = ObservationPlan {
        request: request(),
        observers: vec![Box::new(filesystem), Box::new(command)],
        project_revision: "rev-proof-01".to_owned(),
        environment_fingerprint: gauntlet_adapter_observe_command::env_fingerprint(&env),
        claims_content: ClaimsFacet::render_content(&["the work is done".to_owned()]),
        clock: clock(),
    };
    let assembled = ObservationAssembler::new(store)
        .assemble(plan, bound())
        .expect("an all-healthy assembly succeeds");
    let doc = &assembled.doc;
    assert_eq!(doc.facets().len(), 2);
    // Typed, not Optional: emptiness is a positive statement (§12.3, I8).
    assert_eq!(doc.unavailable_evidence(), &[]);
    assert_eq!(doc.observation_failures(), &[]);
    assert!(!doc.as_domain_view().has_gaps());
    assert!(!doc.facets().is_empty());
}
