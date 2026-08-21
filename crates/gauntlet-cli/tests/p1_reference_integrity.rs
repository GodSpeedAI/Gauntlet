//! P1 proof (plan Task 7 step 5): reference integrity through the COMPOSED
//! path — the production `FileReferenceProvider`, the real content-addressed
//! filesystem evidence store, the real embedded state store — at the
//! composition root, the one crate allowed to name adapters (spec §4.1).
//!
//! Location rationale (recorded per the task brief): the plan spells the
//! proof `tests/proofs/p1_reference_integrity`, but the architecture
//! boundary suite's cargo-target-path rule (exemption list exactly
//! `crates/gauntlet-arch-tests`) forbids a core crate from declaring a
//! repository-root test target, so that spelling cannot be materialized —
//! an additional instance of the locator-materialization class first
//! recorded 2026-08-17 against AK-049. The proof therefore lives as an
//! integration test of the composition root with the target NAME the plan's
//! gate uses (`cargo nextest run --test p1_reference_integrity`).
//!
//! The frozen ruler itself is never pointed at: the test COPIES
//! `.wayfinder/answer-key.md` and `.wayfinder/reference.yaml` (read-only)
//! into a scratch directory and works on the copies — the copy stands in
//! for the configured answer key of a real deployment, so a "builder
//! mutation" writes to the copy, never to `.wayfinder/`.
//!
//! Honest scope (do not overclaim): iteration boundaries and settlement
//! checks are wired into the real controller at plan Tasks 12–13. P1 here
//! proves the provider/evidence/state composition and the tripwire
//! primitive at the composition root; AK-023/AK-024 claims remain
//! substrate-level until the controller callsites exist.

use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use gauntlet_adapter_evidence_fs::FsEvidenceStore;
use gauntlet_adapter_state_sqlite::EmbeddedStateStore;
use gauntlet_app::reference::normalize::{digest_of, normalize_bytes};
use gauntlet_app::reference::parse::parse_answer_key;
use gauntlet_app::reference::provider::FileReferenceProvider;
use gauntlet_domain::{
    Budget, DiagnosticAllowance, Digest, EpochSeconds, EventLogOffset, EvidenceRef, IterationNo,
    Limits, MicroUsd, Reference, ReferenceVersion, Run, RunDefinition, RunId,
};
use gauntlet_ports::comparator::ComparatorError;
use gauntlet_ports::comparator::{FrozenReferenceProjection, RunComparisonAuthority};
use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::evidence_store::EvidenceStore;
use gauntlet_ports::reference_provider::{ReferenceProvider, ReferenceProviderError};
use gauntlet_ports::state_store::{Checkpoint, StateStore};

const RUN: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
/// Fixed points far from any clock (the pipeline under test must never read
/// one). Named consts because a core crate's literals scanner refuses a
/// bare clock number on any line that also names the bound.
const BOUND_AT: u64 = 1_700_000_060;
const RUN_CREATED_AT: u64 = 1_700_000_000;

/// The builder fixture's mid-run edit of the active copy (AK-023's
/// mechanism): a plausible new block written into the key between
/// boundaries.
const MID_RUN_EDIT: &str = "## AK-900\nclaim: a builder's mid-run edit of the ruler.\n";

static SCRATCH_SEQ: AtomicU32 = AtomicU32::new(0);

/// Isolated scratch directory per test; removed on drop. The pid +
/// atomic-counter Scratch idiom used across the repo's test suites.
struct Scratch {
    path: PathBuf,
}

impl Scratch {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "gauntlet-p1-reference-{}-{}",
            std::process::id(),
            SCRATCH_SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::remove_dir_all(&path).ok();
        std::fs::create_dir_all(&path).expect("p1 scratch parent is creatable");
        Self { path }
    }

    /// The frozen ruler's answer key and manifest, copied in read-only, as
    /// this run's ACTIVE artifacts. Nothing in this file ever points at
    /// `.wayfinder/` itself — the ruler is never an active artifact.
    fn seed_frozen_copy(&self) -> (PathBuf, PathBuf) {
        let root = repo_root();
        let key = self.path.join("answer-key.md");
        let manifest = self.path.join("reference.yaml");
        let frozen_key = std::fs::read(root.join(".wayfinder").join("answer-key.md"))
            .expect("the frozen answer key is readable");
        let frozen_manifest =
            std::fs::read_to_string(root.join(".wayfinder").join("reference.yaml"))
                .expect("the frozen manifest is readable");
        std::fs::write(&key, frozen_key).expect("seeds the scratch key copy");
        std::fs::write(&manifest, frozen_manifest).expect("seeds the scratch manifest copy");
        (key, manifest)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.path).ok();
    }
}

/// The repository root, reached through the manifest dir — read-only access
/// to the frozen ruler for seeding copies.
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("the cli crate sits two levels under the repository root")
        .to_path_buf()
}

fn run_id() -> RunId {
    RunId::new(RUN).expect("valid run id")
}

fn bound() -> Deadline {
    Deadline::at_epoch(EpochSeconds::new(BOUND_AT))
}

/// The composed pipeline through the production provider: resolve, then
/// normalize, then freeze. Each step's failure fails the test calling this.
fn freeze_active_copy(provider: &FileReferenceProvider) -> (Reference, ReferenceVersion) {
    let draft = provider
        .resolve("settle plan task 7", bound())
        .expect("resolves the seeded copy");
    let reference = provider.normalize(draft, bound()).expect("normalizes");
    let version = provider
        .freeze(&run_id(), &reference, bound())
        .expect("freezes the run's reference version");
    (reference, version)
}

/// The iteration-boundary tripwire call: what the Task-12 controller will
/// call at every boundary and immediately before settlement (spec §6.2).
/// A named helper so the teeth edit — deleting a boundary call site — is
/// surgical (plan Task 7 done-when).
fn boundary_verify(
    provider: &FileReferenceProvider,
    version: &ReferenceVersion,
) -> Result<(), ReferenceProviderError> {
    provider.verify_unchanged(version, bound())
}

/// AK-022: the frozen reference is stored in the evidence store OUTSIDE
/// every builder-writable path, holds exactly the normalized key, and is
/// pinned to the run.
#[test]
fn frozen_copy_outside_workspace() {
    let scratch = Scratch::new();
    let (key, manifest) = scratch.seed_frozen_copy();
    // Two builder workspaces under the scratch root, and the evidence store
    // rooted OUTSIDE both — the grader's frozen copy must not live where
    // the graded can write.
    let workspace_a = scratch.path.join("workspace-a");
    let workspace_b = scratch.path.join("workspace-b");
    std::fs::create_dir_all(&workspace_a).expect("workspace a creates");
    std::fs::create_dir_all(&workspace_b).expect("workspace b creates");
    let store_root = scratch.path.join("evidence");
    let store = FsEvidenceStore::open(&store_root).expect("the evidence store opens");
    let provider = FileReferenceProvider::from_paths(key.clone(), manifest, Rc::new(store.clone()));

    // Strict admission over the seeded copy (one of the provider module
    // header's three strict-admission callsites): the copy is the frozen
    // key verbatim, so it must parse 70/70 well-formed with no failures.
    let seeded = std::fs::read_to_string(&key).expect("the seeded copy reads");
    let parsed = parse_answer_key(&seeded);
    assert!(
        parsed.failures.is_empty(),
        "the frozen key copy parses clean: {:?}",
        parsed.failures
    );
    assert_eq!(
        parsed.units.len(),
        70,
        "the frozen key carries seventy units"
    );

    let (_reference, version) = freeze_active_copy(&provider);

    // The frozen object's path (the store's documented fanout layout
    // `objects/<b0-2>/<b2-4>/<hex>`; the API exposes no path query, so the
    // layout is asserted through its documented shape) sits outside every
    // workspace root.
    let hex = version.digest().hex();
    let object = store_root
        .join("objects")
        .join(&hex[..2])
        .join(&hex[2..4])
        .join(hex);
    // The SUT-owned placement, asserted directly (critic-B F17): the frozen
    // object lives under the EVIDENCE STORE's root — the one authority that
    // owns frozen copies — and neither that root nor the object sits inside
    // any builder workspace.
    assert!(
        object.starts_with(&store_root),
        "the frozen object {object:?} must live under the evidence store root {store_root:?}"
    );
    for workspace in [&workspace_a, &workspace_b] {
        assert!(
            !store_root.starts_with(workspace),
            "the evidence store root {store_root:?} must sit outside the workspace {workspace:?}"
        );
        assert!(
            !object.starts_with(workspace),
            "the frozen object {object:?} must sit outside the workspace {workspace:?}"
        );
    }
    assert!(
        object.is_file(),
        "the frozen object exists at its content address"
    );

    // The stored bytes are exactly the normalized key, and the evidence
    // address is the version's own digest — both are the same function of
    // the same bytes.
    let key_bytes = std::fs::read(&key).expect("the active copy reads");
    let normalized = normalize_bytes(&key_bytes).expect("the key is valid UTF-8");
    assert_eq!(
        &digest_of(&key_bytes).expect("the active copy digests"),
        version.digest(),
        "the frozen version digest is the digest of the normalized key"
    );
    let evidence_ref = EvidenceRef::new(&version.digest().to_string())
        .expect("the frozen digest is a valid evidence ref");
    assert_eq!(
        store.get(&evidence_ref, bound()).expect("the store reads"),
        Some(normalized),
        "the frozen object holds exactly the normalized key bytes"
    );

    // The ref is pinned to the run: the store's documented pin layout is a
    // marker per (run, ref) at `pins/<run-id>/<evidence-ref>`.
    let pin = store_root
        .join("pins")
        .join(run_id().to_string())
        .join(evidence_ref.to_string());
    assert!(pin.is_file(), "the frozen ref is pinned to the run");
}

/// AK-023's mechanism: a builder writing to the active reference copy
/// mid-run is detected — `verify_unchanged` aborts with
/// `ReferenceMutated { expected, actual }` (expected = frozen digest,
/// actual = re-derived digest) — and the frozen evidence-store copy is
/// byte-unchanged by the mutation.
#[test]
fn builder_mutation_of_the_active_reference_aborts_with_reference_mutated() {
    let scratch = Scratch::new();
    let (key, manifest) = scratch.seed_frozen_copy();
    let store_root = scratch.path.join("evidence");
    let store = FsEvidenceStore::open(&store_root).expect("the evidence store opens");
    let provider = FileReferenceProvider::from_paths(key.clone(), manifest, Rc::new(store.clone()));
    let (_reference, version) = freeze_active_copy(&provider);
    let evidence_ref = EvidenceRef::new(&version.digest().to_string())
        .expect("the frozen digest is a valid evidence ref");
    let frozen_bytes = store.get(&evidence_ref, bound()).expect("the store reads");

    // The builder fixture overwrites the ACTIVE copy (the scratch key
    // standing in for the configured answer key — never `.wayfinder/`).
    std::fs::write(&key, MID_RUN_EDIT).expect("the builder mutation lands");

    let rederived = digest_of(MID_RUN_EDIT.as_bytes()).expect("the mutation digests");
    match provider.verify_unchanged(&version, bound()) {
        Err(ReferenceProviderError::ReferenceMutated { expected, actual }) => {
            assert_eq!(expected, *version.digest(), "expected is the frozen digest");
            assert_eq!(actual, rederived, "actual is the re-derived digest");
        }
        other => panic!("a moved ruler must abort with ReferenceMutated, got {other:?}"),
    }

    // The frozen copy is immutable evidence: byte-unchanged by the mutation.
    assert_eq!(
        store.get(&evidence_ref, bound()).expect("the store reads"),
        frozen_bytes,
        "the frozen evidence-store copy is unchanged by the active-copy mutation"
    );
}

/// AK-024's mechanism/substrate: the digest is re-verified at every
/// iteration boundary and immediately before settlement. The mutation lands
/// between boundary 1 and boundary 2; the tripwire must fire at the very
/// next boundary — detection that waited for settlement only would mean the
/// boundary calls were decorative. The loop's `boundary_verify` call site
/// is the tooth: delete it and `detected_at` stays `None` and this test
/// fails (plan Task 7 done-when).
#[test]
fn digest_rechecked() {
    let scratch = Scratch::new();
    let (key, manifest) = scratch.seed_frozen_copy();
    let store_root = scratch.path.join("evidence");
    let store = FsEvidenceStore::open(&store_root).expect("the evidence store opens");
    let provider = FileReferenceProvider::from_paths(key.clone(), manifest, Rc::new(store));
    let (_reference, version) = freeze_active_copy(&provider);

    // Boundary 1, on the intact ruler: the tripwire must be QUIET when
    // nothing moved — a tripwire that fires on an unmoved ruler is as
    // broken as one that never fires.
    boundary_verify(&provider, &version).expect("boundary 1 sees the intact ruler");

    // The builder writes to the active copy mid-run, between boundary 1 and
    // boundary 2.
    std::fs::write(&key, MID_RUN_EDIT).expect("the mid-run mutation lands");

    // Boundaries 2 and 3 (a harness standing in for the controller's
    // iteration loop): the tripwire must fire at the FIRST boundary after
    // the edit.
    let mut detected_at: Option<u32> = None;
    for boundary in 2u32..=3 {
        if let Err(ReferenceProviderError::ReferenceMutated { expected, actual }) =
            boundary_verify(&provider, &version)
        {
            assert_eq!(expected, *version.digest());
            assert_ne!(actual, *version.digest());
            detected_at = Some(boundary);
            // The controller aborts at the boundary that fires (spec §11.5);
            // the loop's later boundaries never run.
            break;
        }
    }
    assert_eq!(
        detected_at,
        Some(2),
        "the tripwire must fire at the very next boundary after the mutation — \
         never at settlement only"
    );

    // The settlement pre-check — the same primitive, immediately before
    // settlement (spec §6.2) — fires as well: a second line of defense,
    // never the first.
    assert!(matches!(
        boundary_verify(&provider, &version),
        Err(ReferenceProviderError::ReferenceMutated { .. })
    ));
}

/// Freeze binds to run identity durably: a domain `Run` constructed over
/// the frozen `ReferenceVersion`, recorded through the real embedded state
/// store, loads back carrying the identical frozen version.
#[test]
fn the_run_carries_the_frozen_version() {
    let scratch = Scratch::new();
    let (key, manifest) = scratch.seed_frozen_copy();
    let store_root = scratch.path.join("evidence");
    let store = FsEvidenceStore::open(&store_root).expect("the evidence store opens");
    let provider = FileReferenceProvider::from_paths(key, manifest, Rc::new(store));
    let (_reference, version) = freeze_active_copy(&provider);

    let db = scratch.path.join("state.db");
    let state = EmbeddedStateStore::open(&db).expect("the state store opens");
    let run = Run::new(RunDefinition {
        id: run_id(),
        created_at: EpochSeconds::new(RUN_CREATED_AT),
        reference_version: version.clone(),
        intent: "settle the task".to_owned(),
        config_fingerprint: Digest::new("sha256", &"ab".repeat(32)).expect("valid digest"),
        environment_fingerprint: Digest::new("sha256", &"cd".repeat(32)).expect("valid digest"),
        budget: Budget::new(
            Limits::new(8, 2, 2, 3600, 1000, MicroUsd::new(500)).expect("legal ceilings"),
            20,
            DiagnosticAllowance::new(100, MicroUsd::new(50)),
        )
        .expect("legal budget"),
        parent_run: None,
    })
    .expect("legal run");
    state.record_run(&run, bound()).expect("the run records");
    state
        .checkpoint(
            Checkpoint {
                run_id: run_id(),
                reference_digest: version.digest().clone(),
                iteration_no: IterationNo::new(1).expect("first iteration"),
                unit_states: Vec::new(),
                budget_spent: *run.budget().spent(),
                in_flight: Vec::new(),
                event_log_offset: EventLogOffset::new(0),
            },
            bound(),
        )
        .expect("the frozen run checkpoints");
    // Comparison authority also requires the iteration's durable observation
    // binding. This test is about the REFERENCE half, so it binds a placeholder
    // ref: the authority resolves the ref, and only the comparison chain reads
    // its bytes.
    state
        .bind_observation_bundle(
            &run_id(),
            IterationNo::new(1).expect("first iteration"),
            &gauntlet_ports::state_store::ObservationBinding::new(
                gauntlet_domain::EvidenceRef::new("evd-observation-bundle-p1").expect("valid ref"),
                gauntlet_domain::Digest::new("sha256", &"ab".repeat(32)).expect("digest"),
            ),
            bound(),
        )
        .expect("the observation binds durably");
    drop(state);

    let reopened = EmbeddedStateStore::open(&db).expect("the state store reopens");
    let loaded = reopened
        .load_run(&run_id(), bound())
        .expect("the run loads")
        .expect("the recorded run is durable");
    assert_eq!(
        loaded.reference_version(),
        &version,
        "the loaded run carries the identical frozen reference version"
    );
    assert_eq!(loaded.id(), &run_id(), "the loaded run is the same run");

    let evidence_after_reopen = FsEvidenceStore::open(&store_root).expect("evidence reopens");
    // The projection is loaded through the EARNED authority: the reference
    // version comes out of the durable run, never from this test.
    let authority = RunComparisonAuthority::open(
        &reopened,
        &run_id(),
        gauntlet_domain::IterationNo::new(1).expect("iteration"),
        bound(),
    )
    .expect("the durable run yields comparison authority");
    assert_eq!(authority.reference(), &version);
    let projection = FrozenReferenceProjection::load(&authority, &evidence_after_reopen, bound())
        .expect("the rehydrated version still projects its frozen evidence");
    assert_eq!(projection.version(), &version);

    // Tooth: a rehydrated version never treats an absent evidence object as a
    // binding. The typed unavailable outcome is required instead.
    let missing_evidence = FsEvidenceStore::open(&scratch.path.join("missing-evidence"))
        .expect("empty evidence store opens");
    assert!(matches!(
        FrozenReferenceProjection::load(&authority, &missing_evidence, bound()),
        Err(ComparatorError::EnvelopeInvalid { .. })
    ));
}
