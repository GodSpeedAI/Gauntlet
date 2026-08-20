//! Bundle assembly (spec §5.3.6, §12, §21.3; plan Task 9): run the plan's
//! observers in order, record every facet, turn every observer failure
//! into BOTH required gap entries at exactly ONE site, cross-check each
//! facet's provenance against its stored evidence, digest, and store the
//! finished document content-addressed.
//!
//! Deadlines follow the port-wide recorded convention: noted, passed to
//! store writes, not enforced as process timeouts (Task-8 deferral
//! class; plan Task 12 composes real deadline behavior).
//!
//! `project_revision` is caller-supplied (the runtime knows the revision
//! it dispatched against; the versioning facet records repository state,
//! not the revision contract — a plan Task 12 wiring point).

use std::sync::Arc;

use gauntlet_domain::{Digest, EvidenceKind, EvidenceRef};
use gauntlet_ports::clock::Clock;
use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::evidence_store::EvidenceStore;
use gauntlet_ports::observer::{ObservationRequest, Observer, ObserverError};
use gauntlet_ports::observer_evidence::FacetEvidenceDoc;

use super::bundle::{
    BundleContent, ClaimsFacet, FacetRecord, ObservationBundleDoc, ObservationFailureRecord,
    UnavailableEvidenceRecord,
};
use super::digest::observation_digest;

/// The digest every freshly assembled document carries until the real one
/// is computed over it. The normalized form pins the `observation_digest`
/// field to `""` before hashing (see `digest`), so this placeholder can
/// never influence the computed digest — it exists only because the
/// content bag requires a value.
const DIGEST_PLACEHOLDER_HEX: &str =
    "0000000000000000000000000000000000000000000000000000000000000000";

/// Assembly failures — typed, with the failing step (spec §21.2). A
/// malformed PLAN is refused before anything runs (C4): an empty observer
/// list would assemble an "all-healthy" attestation over zero probes, and
/// identity fields the wire contract requires non-empty are checked at the
/// construction path instead of failing decode later. Beyond that, only a
/// failure to DURABLY RECORD the bundle aborts assembly; observer failures
/// and provenance disagreements become recorded gap entries (honest data,
/// never a crash).
#[derive(Debug, Clone, PartialEq)]
pub enum AssemblyError {
    /// Writing the assembled bundle document to the evidence store
    /// failed — without the bundle's own evidence ref there is no bundle.
    BundleStoreWrite { detail: String },
    /// The plan declares no observers (C4): zero probes is not a positive
    /// attestation that nothing was unavailable.
    EmptyObserverList,
    /// The plan's `environment_fingerprint` is not `sha256:` followed by
    /// exactly 64 lowercase hex characters (C4).
    BadFingerprintShape { fingerprint: String },
    /// The plan's `project_revision` is empty (C4) — the bundle's own wire
    /// contract requires it non-empty.
    EmptyProjectRevision,
}

impl core::fmt::Display for AssemblyError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::BundleStoreWrite { detail } => write!(
                f,
                "storing the assembled observation bundle failed: {detail}"
            ),
            Self::EmptyObserverList => write!(
                f,
                "the plan declares no observers: an empty observation is not a positive \
                 attestation"
            ),
            Self::BadFingerprintShape { fingerprint } => write!(
                f,
                "the plan's environment fingerprint is not sha256: followed by exactly 64 \
                 lowercase hex characters: {fingerprint}"
            ),
            Self::EmptyProjectRevision => {
                write!(f, "the plan's project revision is empty")
            }
        }
    }
}

impl std::error::Error for AssemblyError {}

/// One assembly request: what to observe, with which observers, under
/// which recorded identity.
pub struct ObservationPlan {
    pub request: ObservationRequest,
    pub observers: Vec<Box<dyn Observer>>,
    pub project_revision: String,
    pub environment_fingerprint: String,
    pub claims_content: String,
    pub clock: Arc<dyn Clock>,
}

/// The assembled bundle plus its content-addressed evidence ref.
#[derive(Debug)]
pub struct AssembledBundle {
    pub doc: ObservationBundleDoc,
    pub bundle_ref: EvidenceRef,
}

/// Assembles observation bundle documents (plan Task 9).
pub struct ObservationAssembler {
    store: Arc<dyn EvidenceStore>,
}

impl ObservationAssembler {
    pub fn new(store: Arc<dyn EvidenceStore>) -> Self {
        Self { store }
    }

    /// Runs the plan: observe each observer in plan order, populate gaps
    /// at the single site, cross-check facet provenance against the
    /// stored evidence, digest, and put the finished document.
    pub fn assemble(
        &self,
        plan: ObservationPlan,
        deadline: Deadline,
    ) -> Result<AssembledBundle, AssemblyError> {
        // C4: refuse malformed plans before anything runs. An empty
        // observer list would assemble an "all-healthy" attestation over
        // zero probes; the identity fields the bundle's wire contract
        // requires non-empty are checked here so an in-memory caller
        // cannot build a bundle its own decode would reject.
        if plan.observers.is_empty() {
            return Err(AssemblyError::EmptyObserverList);
        }
        if plan.project_revision.is_empty() {
            return Err(AssemblyError::EmptyProjectRevision);
        }
        if !is_sha256_fingerprint(&plan.environment_fingerprint) {
            return Err(AssemblyError::BadFingerprintShape {
                fingerprint: plan.environment_fingerprint.clone(),
            });
        }
        let observed_at = plan.clock.now().get();
        let mut facets: Vec<FacetRecord> = Vec::new();
        let mut unavailable: Vec<UnavailableEvidenceRecord> = Vec::new();
        let mut failures: Vec<ObservationFailureRecord> = Vec::new();

        for observer in &plan.observers {
            match observer.observe(&plan.request, deadline) {
                Ok(facet) => facets.push(FacetRecord::from_facet(&facet)),
                Err(error) => {
                    let (missing, failed) = gap_entries_for(&error);
                    unavailable.push(missing);
                    failures.push(failed);
                }
            }
        }

        // Provenance cross-check: each facet's evidence must strict-decode
        // as a facet_evidence@v1 document that AGREES with the facet record
        // on instrument identity, exit status, command, and the timing
        // window. A disagreement is recorded as data in BOTH gap arrays
        // (C2, §21.3 — the facet stays; the failure entry names the
        // mismatch) — an honest gap, not a crash.
        for facet in &facets {
            self.cross_check_facet(facet, deadline, &mut failures, &mut unavailable);
        }

        let placeholder = Digest::new("sha256", DIGEST_PLACEHOLDER_HEX).map_err(|error| {
            AssemblyError::BundleStoreWrite {
                detail: format!("the digest placeholder is not a valid Digest: {error}"),
            }
        })?;
        let content = BundleContent {
            run_id: plan.request.run_id.clone(),
            iteration_no: plan.request.iteration_no,
            observed_at,
            project_revision: plan.project_revision,
            environment_fingerprint: plan.environment_fingerprint,
            observation_digest: placeholder,
            facets,
            claims_facet: ClaimsFacet::new(plan.claims_content),
            unavailable_evidence: unavailable,
            observation_failures: failures,
        };
        // The real digest is computed over the normalized form, which pins
        // the digest field away — the placeholder cannot leak in.
        let draft = ObservationBundleDoc::new(content);
        let digest = observation_digest(&draft);
        let mut content = draft.content();
        content.observation_digest = digest;
        let doc = ObservationBundleDoc::new(content);

        let bundle_ref = self
            .store
            .put(doc.to_json().as_bytes(), EvidenceKind::Log, deadline)
            .map_err(|error| AssemblyError::BundleStoreWrite {
                detail: error.to_string(),
            })?;
        Ok(AssembledBundle { doc, bundle_ref })
    }

    fn cross_check_facet(
        &self,
        record: &FacetRecord,
        deadline: Deadline,
        failures: &mut Vec<ObservationFailureRecord>,
        unavailable: &mut Vec<UnavailableEvidenceRecord>,
    ) {
        // C2: the cross-check's ONE failure site populates BOTH arrays —
        // a facet whose evidence disagrees with it is recorded unavailable
        // evidence too (§21.3), so a consumer reading either array sees
        // the kind.
        let mut mismatch = |detail: String| {
            failures.push(ObservationFailureRecord::new(
                record.kind().to_owned(),
                "provenance_mismatch".to_owned(),
                detail.clone(),
            ));
            unavailable.push(UnavailableEvidenceRecord::new(
                record.kind().to_owned(),
                format!("the facet's evidence is unavailable: {detail}"),
            ));
        };
        let reference = match EvidenceRef::new(record.evidence_ref()) {
            Ok(reference) => reference,
            Err(error) => {
                mismatch(format!("the evidence ref is not well formed: {error}"));
                return;
            }
        };
        let bytes = match self.store.get(&reference, deadline) {
            Ok(Some(bytes)) => bytes,
            Ok(None) => {
                mismatch("the facet's evidence is absent from the store".to_owned());
                return;
            }
            Err(error) => {
                mismatch(format!("reading the facet's evidence failed: {error}"));
                return;
            }
        };
        let evidence = match FacetEvidenceDoc::decode(&String::from_utf8_lossy(&bytes)) {
            Ok(evidence) => evidence,
            Err(error) => {
                mismatch(format!(
                    "the facet's evidence does not strict-decode as facet evidence: {error}"
                ));
                return;
            }
        };
        // C3: agreement on instrument identity, exit status, command, and
        // the timing window. Only what the facet record actually CARRIES is
        // compared (the Option fields) — with one deliberate exception: a
        // facet with no exit status against evidence that has one is itself
        // a mismatch (the None-bypass is closed).
        if record.instrument_name() != evidence.instrument_name() {
            mismatch(format!(
                "the facet names instrument {} but its evidence names {}",
                record.instrument_name(),
                evidence.instrument_name()
            ));
            return;
        }
        if record.instrument_version() != evidence.instrument_version() {
            mismatch(format!(
                "the facet names instrument version {} but its evidence names {}",
                record.instrument_version(),
                evidence.instrument_version()
            ));
            return;
        }
        match record.exit_status() {
            Some(exit) if exit == evidence.exit_status() => {}
            Some(exit) => {
                mismatch(format!(
                    "the facet records exit status {exit} but its evidence records {}",
                    evidence.exit_status()
                ));
                return;
            }
            None => {
                mismatch(format!(
                    "the facet carries no exit status while its evidence records {}",
                    evidence.exit_status()
                ));
                return;
            }
        }
        if let Some(command) = record.command()
            && command != evidence.command()
        {
            mismatch(format!(
                "the facet records command {command:?} but its evidence records {:?}",
                evidence.command()
            ));
            return;
        }
        if let Some(started) = record.started_at()
            && started != evidence.started_at()
        {
            mismatch(format!(
                "the facet records started_at {started} but its evidence records {}",
                evidence.started_at()
            ));
            return;
        }
        if let Some(ended) = record.ended_at()
            && ended != evidence.ended_at()
        {
            mismatch(format!(
                "the facet records ended_at {ended} but its evidence records {}",
                evidence.ended_at()
            ));
        }
    }
}

/// The declared plan-fingerprint shape (C4): `sha256:` followed by
/// exactly 64 lowercase hexadecimal characters — the same spelling the
/// adapters' `env_fingerprint` functions produce.
fn is_sha256_fingerprint(fingerprint: &str) -> bool {
    let Some(hex) = fingerprint.strip_prefix("sha256:") else {
        return false;
    };
    hex.len() == 64
        && hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// THE single population site for observer failures (the plan's done-when
/// tooth lives here): one function turns an [`ObserverError`] into BOTH
/// §12.3 entries — an `observation_failures` record AND, per §21.3 (an
/// observation failure is recorded unavailable evidence), an
/// `unavailable_evidence` record. The error carries the facet kind but no
/// instrument name; the kind token is the recorded probe identity (a
/// documented limitation — see the bundle's domain-view mapping).
fn gap_entries_for(error: &ObserverError) -> (UnavailableEvidenceRecord, ObservationFailureRecord) {
    match error {
        ObserverError::InstrumentFailed { kind, detail } => (
            UnavailableEvidenceRecord::new(
                kind.token(),
                format!("the instrument failed: {detail}"),
            ),
            ObservationFailureRecord::new(
                kind.token(),
                "instrument_failed".to_owned(),
                detail.clone(),
            ),
        ),
        ObserverError::Unavailable { kind, detail } => (
            UnavailableEvidenceRecord::new(
                kind.token(),
                format!("the instrument is unavailable: {detail}"),
            ),
            ObservationFailureRecord::new(kind.token(), "unavailable".to_owned(), detail.clone()),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::{AssemblyError, ObservationAssembler, ObservationPlan};
    use crate::observation::bundle::ObservationBundleDoc;
    use crate::observation::digest::observation_digest;
    use crate::observation::input::VerificationInputs;
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::{Arc, Mutex};

    use gauntlet_domain::InstrumentName;
    use gauntlet_domain::{
        EpochSeconds, EvidenceKind, EvidenceRef, IterationNo, RunId, WorkspaceRef,
    };
    use gauntlet_ports::clock::Clock;
    use gauntlet_ports::deadline::Deadline;
    use gauntlet_ports::evidence_store::{EvidenceStore, EvidenceStoreError};
    use gauntlet_ports::observer::{
        FacetKind, InstrumentIdentity, ObservationFacet, ObservationRequest, Observer,
        ObserverError,
    };
    use gauntlet_ports::observer_evidence::{FacetEvidenceDoc, ObservedRun};

    const ULID: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
    const BASE_INSTANT: u64 = 1_700_000_000;
    const BOUND_SPAN: u64 = 60;
    const CANARY_CLAIM: &str = "the artifact passes its own declared self-check";
    /// A plan fingerprint that satisfies the C4 shape (`sha256:` + exactly
    /// 64 lowercase hex characters).
    const VALID_FINGERPRINT: &str =
        "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    fn bound() -> Deadline {
        Deadline::at_epoch(EpochSeconds::new(BASE_INSTANT + BOUND_SPAN))
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

    /// In-test scratch evidence store: ref-keyed bytes, monotonically
    /// assigned. Optionally fails every put (the g6 store-failure shape).
    struct ScratchStore {
        blobs: Mutex<HashMap<EvidenceRef, Vec<u8>>>,
        next: AtomicU64,
        fail_puts: bool,
    }

    impl ScratchStore {
        fn new() -> Self {
            Self {
                blobs: Mutex::new(HashMap::new()),
                next: AtomicU64::new(0),
                fail_puts: false,
            }
        }

        fn failing() -> Self {
            Self {
                fail_puts: true,
                ..Self::new()
            }
        }
    }

    impl EvidenceStore for ScratchStore {
        fn put(
            &self,
            bytes: &[u8],
            _kind: EvidenceKind,
            _deadline: Deadline,
        ) -> Result<EvidenceRef, EvidenceStoreError> {
            if self.fail_puts {
                return Err(EvidenceStoreError::WriteFailed {
                    detail: "the scratch store is configured to fail writes".to_owned(),
                });
            }
            let n = self.next.fetch_add(1, Ordering::Relaxed);
            let reference = EvidenceRef::new(&format!("evd-assembly-{n:06}")).map_err(|err| {
                EvidenceStoreError::WriteFailed {
                    detail: err.to_string(),
                }
            })?;
            self.blobs
                .lock()
                .map_err(|_| EvidenceStoreError::WriteFailed {
                    detail: poisoned("put"),
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
                    detail: poisoned("get"),
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

    fn poisoned(op: &str) -> String {
        format!("the scratch store's lock was poisoned during {op}")
    }

    /// What one stubbed observer does when observed.
    enum StubScript {
        /// Stores honest evidence with the given exit and returns a facet
        /// carrying the same exit.
        Healthy(i64),
        /// Returns the given observer error (never a facet).
        Failing(ObserverError),
        /// Stores evidence whose exit DISAGREES with the facet it returns
        /// (the provenance-mismatch probe).
        Disagreeing { facet_exit: i64, evidence_exit: i64 },
        /// Stores bytes that do not strict-decode as facet evidence at all,
        /// then returns a facet citing them (the C2 garbage-evidence probe).
        Garbage,
        /// Stores genuine evidence from a DIFFERENT run — another
        /// instrument, another command, another window — with an exit that
        /// AGREES with the facet it returns (the C3 cross-wiring probe:
        /// facet A wearing facet B's evidence).
        CrossWired { exit: i64 },
        /// Stores honest evidence carrying an exit but returns a facet with
        /// NO exit status (the C3 None-bypass probe).
        Exitless { evidence_exit: i64 },
    }

    struct StubObserver {
        kind: FacetKind,
        script: StubScript,
        store: Arc<ScratchStore>,
    }

    impl Observer for StubObserver {
        fn observe(
            &self,
            _request: &ObservationRequest,
            deadline: Deadline,
        ) -> Result<ObservationFacet, ObserverError> {
            let instrument = InstrumentIdentity::new(
                InstrumentName::new("suite-probe").expect("valid instrument name"),
                "1.2.3".to_owned(),
            );
            match &self.script {
                StubScript::Healthy(exit) => {
                    let facet_exit = *exit;
                    self.facet_for(instrument, Some(facet_exit), facet_exit, deadline)
                }
                StubScript::Disagreeing {
                    facet_exit,
                    evidence_exit,
                } => self.facet_for(instrument, Some(*facet_exit), *evidence_exit, deadline),
                StubScript::Garbage => {
                    let evidence_ref = self
                        .store
                        .put(
                            b"this is not a facet_evidence@v1 document",
                            EvidenceKind::Log,
                            deadline,
                        )
                        .map_err(|err| ObserverError::Unavailable {
                            kind: self.kind,
                            detail: format!("storing the garbage evidence failed: {err}"),
                        })?;
                    Ok(self.plain_facet(instrument, evidence_ref, 0))
                }
                StubScript::CrossWired { exit } => {
                    let doc = FacetEvidenceDoc::new(
                        "other-probe".to_owned(),
                        "9.9.9".to_owned(),
                        ObservedRun {
                            command: "other-command".to_owned(),
                            env_fingerprint: "sha256:0123abcd".to_owned(),
                            exit_status: *exit,
                            started_at: BASE_INSTANT + 100,
                            ended_at: BASE_INSTANT + 103,
                            stdout: "captured output of a different run".to_owned(),
                            stderr: String::new(),
                        },
                    )
                    .map_err(|err| ObserverError::InstrumentFailed {
                        kind: self.kind,
                        detail: format!("building the cross-wired evidence failed: {err}"),
                    })?;
                    let evidence_ref = self
                        .store
                        .put(doc.encode().as_bytes(), EvidenceKind::Log, deadline)
                        .map_err(|err| ObserverError::Unavailable {
                            kind: self.kind,
                            detail: format!("storing the cross-wired evidence failed: {err}"),
                        })?;
                    Ok(self.plain_facet(instrument, evidence_ref, *exit))
                }
                StubScript::Exitless { evidence_exit } => {
                    self.facet_for(instrument, None, *evidence_exit, deadline)
                }
                StubScript::Failing(error) => Err(error.clone()),
            }
        }

        fn facet_kind(&self) -> FacetKind {
            self.kind
        }
    }

    impl StubObserver {
        /// A facet with this stub's standard shape: command `probe`, the
        /// standard window, and the given exit status.
        fn plain_facet(
            &self,
            instrument: InstrumentIdentity,
            evidence_ref: EvidenceRef,
            exit: i64,
        ) -> ObservationFacet {
            ObservationFacet::new(self.kind, instrument, evidence_ref)
                .with_invocation(Some("probe".to_owned()), Some(exit))
                .with_window(
                    Some(EpochSeconds::new(BASE_INSTANT)),
                    Some(EpochSeconds::new(BASE_INSTANT + 3)),
                )
                .with_summary(Some("one probe, observed".to_owned()))
        }

        /// Stores a facet_evidence@v1 document carrying `evidence_exit`,
        /// then builds the facet carrying `facet_exit`.
        fn facet_for(
            &self,
            instrument: InstrumentIdentity,
            facet_exit: Option<i64>,
            evidence_exit: i64,
            deadline: Deadline,
        ) -> Result<ObservationFacet, ObserverError> {
            let doc = FacetEvidenceDoc::new(
                "suite-probe".to_owned(),
                "1.2.3".to_owned(),
                ObservedRun {
                    command: "probe".to_owned(),
                    env_fingerprint: "sha256:0123abcd".to_owned(),
                    exit_status: evidence_exit,
                    started_at: BASE_INSTANT,
                    ended_at: BASE_INSTANT + 3,
                    stdout: "captured probe output".to_owned(),
                    stderr: String::new(),
                },
            )
            .map_err(|err| ObserverError::InstrumentFailed {
                kind: self.kind,
                detail: format!("building the facet evidence document failed: {err}"),
            })?;
            let evidence_ref = self
                .store
                .put(doc.encode().as_bytes(), EvidenceKind::Log, deadline)
                .map_err(|err| ObserverError::Unavailable {
                    kind: self.kind,
                    detail: format!("storing the facet evidence document failed: {err}"),
                })?;
            Ok(ObservationFacet::new(self.kind, instrument, evidence_ref)
                .with_invocation(Some("probe".to_owned()), facet_exit)
                .with_window(
                    Some(EpochSeconds::new(BASE_INSTANT)),
                    Some(EpochSeconds::new(BASE_INSTANT + 3)),
                )
                .with_summary(Some("one probe, observed".to_owned())))
        }
    }

    fn request() -> ObservationRequest {
        ObservationRequest {
            run_id: RunId::new(ULID).expect("valid run id"),
            iteration_no: IterationNo::new(1).expect("iterations are 1-based"),
            workspace: WorkspaceRef::new("ws-assembly").expect("valid workspace ref"),
        }
    }

    fn plan_with(
        observers: Vec<Box<dyn Observer>>,
        claims: &str,
        clock: Arc<FixedClock>,
    ) -> ObservationPlan {
        ObservationPlan {
            request: request(),
            observers,
            project_revision: "rev-suite-01".to_owned(),
            environment_fingerprint: VALID_FINGERPRINT.to_owned(),
            claims_content: claims.to_owned(),
            clock,
        }
    }

    fn clock() -> Arc<FixedClock> {
        Arc::new(FixedClock {
            now: EpochSeconds::new(BASE_INSTANT),
        })
    }

    fn healthy(store: Arc<ScratchStore>, kind: FacetKind) -> Box<StubObserver> {
        Box::new(StubObserver {
            kind,
            script: StubScript::Healthy(0),
            store,
        })
    }

    fn failing(kind: FacetKind, error: ObserverError) -> Box<StubObserver> {
        Box::new(StubObserver {
            kind,
            script: StubScript::Failing(error),
            store: Arc::new(ScratchStore::new()),
        })
    }

    #[test]
    fn observer_failures_populate_both_gap_arrays_exactly_once() {
        let store = Arc::new(ScratchStore::new());
        let observers: Vec<Box<dyn Observer>> = vec![
            healthy(store.clone(), FacetKind::Test),
            failing(
                FacetKind::Browser,
                ObserverError::InstrumentFailed {
                    kind: FacetKind::Browser,
                    detail: "the instrument exited without a report".to_owned(),
                },
            ),
        ];
        let assembler = ObservationAssembler::new(store.clone());
        let assembled = assembler
            .assemble(plan_with(observers, CANARY_CLAIM, clock()), bound())
            .expect("one failed probe never fails the whole assembly");
        let doc = &assembled.doc;
        assert_eq!(doc.facets().len(), 1, "the healthy observer's facet stays");
        assert_eq!(
            doc.unavailable_evidence().len(),
            1,
            "the failure is recorded unavailable evidence too (§21.3)"
        );
        assert_eq!(doc.unavailable_evidence()[0].kind(), "browser");
        assert_eq!(doc.observation_failures().len(), 1);
        assert_eq!(doc.observation_failures()[0].kind(), "browser");
        assert_eq!(
            doc.observation_failures()[0].error_class(),
            "instrument_failed"
        );
        assert!(doc.as_domain_view().has_gaps());
    }

    #[test]
    fn all_healthy_observers_leave_both_arrays_explicitly_empty() {
        let store = Arc::new(ScratchStore::new());
        let observers: Vec<Box<dyn Observer>> = vec![
            healthy(store.clone(), FacetKind::Test),
            healthy(store.clone(), FacetKind::Filesystem),
        ];
        let assembler = ObservationAssembler::new(store);
        let assembled = assembler
            .assemble(plan_with(observers, CANARY_CLAIM, clock()), bound())
            .expect("all-healthy assembly succeeds");
        let doc = &assembled.doc;
        assert_eq!(doc.facets().len(), 2);
        // Typed, not Optional: empty is a positive statement (§12.3, I8).
        assert_eq!(doc.unavailable_evidence(), &[]);
        assert_eq!(doc.observation_failures(), &[]);
        assert!(!doc.as_domain_view().has_gaps());
    }

    #[test]
    fn a_provenance_disagreement_is_recorded_not_fatal() {
        let store = Arc::new(ScratchStore::new());
        let disagreeing = Box::new(StubObserver {
            kind: FacetKind::Test,
            script: StubScript::Disagreeing {
                facet_exit: 0,
                evidence_exit: 1,
            },
            store: store.clone(),
        });
        let observers: Vec<Box<dyn Observer>> = vec![disagreeing];
        let assembler = ObservationAssembler::new(store);
        let assembled = assembler
            .assemble(plan_with(observers, CANARY_CLAIM, clock()), bound())
            .expect("a disagreement is data, never a crash");
        let doc = &assembled.doc;
        assert_eq!(doc.facets().len(), 1, "the facet stays");
        assert_eq!(doc.observation_failures().len(), 1);
        assert_eq!(
            doc.observation_failures()[0].error_class(),
            "provenance_mismatch"
        );
        assert!(
            doc.observation_failures()[0]
                .detail()
                .contains("records exit status 0 but its evidence records 1"),
            "the mismatch is named: {}",
            doc.observation_failures()[0].detail()
        );
        assert!(doc.as_domain_view().has_gaps());
    }

    /// C2 (§21.3): a facet whose evidence is garbage — the cross-check's
    /// failure site populates BOTH arrays, so a consumer reading only
    /// `unavailable_evidence` still sees the kind.
    #[test]
    fn garbage_evidence_populates_both_gap_arrays() {
        let store = Arc::new(ScratchStore::new());
        let garbage = Box::new(StubObserver {
            kind: FacetKind::Test,
            script: StubScript::Garbage,
            store: store.clone(),
        });
        let observers: Vec<Box<dyn Observer>> = vec![garbage];
        let assembler = ObservationAssembler::new(store);
        let assembled = assembler
            .assemble(plan_with(observers, CANARY_CLAIM, clock()), bound())
            .expect("undecodable evidence is data, never a crash");
        let doc = &assembled.doc;
        assert_eq!(doc.facets().len(), 1, "the facet stays");
        assert_eq!(doc.observation_failures().len(), 1);
        assert_eq!(
            doc.observation_failures()[0].error_class(),
            "provenance_mismatch"
        );
        assert_eq!(
            doc.observation_failures()[0].kind(),
            FacetKind::Test.token()
        );
        assert_eq!(
            doc.unavailable_evidence().len(),
            1,
            "a facet with undecodable evidence is recorded unavailable evidence too (C2)"
        );
        assert_eq!(
            doc.unavailable_evidence()[0].kind(),
            FacetKind::Test.token()
        );
        assert!(doc.as_domain_view().has_gaps());
    }

    /// C3: cross-wired provenance — facet A citing facet B's evidence with
    /// EQUAL exits must still mismatch: instrument, version, command, and
    /// the timing window are all agreement checks now, and every
    /// disagreement flows through the same BOTH-array failure site (C2).
    #[test]
    fn cross_wired_evidence_with_equal_exits_is_a_provenance_mismatch() {
        let store = Arc::new(ScratchStore::new());
        let cross_wired = Box::new(StubObserver {
            kind: FacetKind::Test,
            script: StubScript::CrossWired { exit: 0 },
            store: store.clone(),
        });
        let observers: Vec<Box<dyn Observer>> = vec![cross_wired];
        let assembler = ObservationAssembler::new(store);
        let assembled = assembler
            .assemble(plan_with(observers, CANARY_CLAIM, clock()), bound())
            .expect("cross-wiring is data, never a crash");
        let doc = &assembled.doc;
        assert_eq!(doc.facets().len(), 1, "the facet stays");
        assert_eq!(
            doc.observation_failures().len(),
            1,
            "the cross-wiring is recorded: {:#?}",
            doc.observation_failures()
        );
        assert_eq!(
            doc.observation_failures()[0].error_class(),
            "provenance_mismatch"
        );
        assert!(
            doc.observation_failures()[0]
                .detail()
                .contains("instrument"),
            "the disagreement is named: {}",
            doc.observation_failures()[0].detail()
        );
        assert_eq!(
            doc.unavailable_evidence().len(),
            1,
            "the mismatch is recorded unavailable evidence too (C2)"
        );
        assert_eq!(
            doc.unavailable_evidence()[0].kind(),
            FacetKind::Test.token()
        );
        assert!(doc.as_domain_view().has_gaps());
    }

    /// C3, the None-bypass: a facet that carries NO exit status while its
    /// evidence carries one is a mismatch, not a free ride.
    #[test]
    fn an_exitless_facet_against_exited_evidence_is_a_provenance_mismatch() {
        let store = Arc::new(ScratchStore::new());
        let exitless = Box::new(StubObserver {
            kind: FacetKind::Test,
            script: StubScript::Exitless { evidence_exit: 0 },
            store: store.clone(),
        });
        let observers: Vec<Box<dyn Observer>> = vec![exitless];
        let assembler = ObservationAssembler::new(store);
        let assembled = assembler
            .assemble(plan_with(observers, CANARY_CLAIM, clock()), bound())
            .expect("an exitless facet is data, never a crash");
        let doc = &assembled.doc;
        assert_eq!(doc.observation_failures().len(), 1);
        assert_eq!(
            doc.observation_failures()[0].error_class(),
            "provenance_mismatch"
        );
        assert!(
            doc.observation_failures()[0]
                .detail()
                .contains("no exit status"),
            "the None-bypass is named: {}",
            doc.observation_failures()[0].detail()
        );
        assert_eq!(doc.unavailable_evidence().len(), 1);
        assert!(doc.as_domain_view().has_gaps());
    }

    #[test]
    fn an_observer_store_write_failure_surfaces_as_gap_entries() {
        let store = Arc::new(ScratchStore::new());
        let observers: Vec<Box<dyn Observer>> = vec![
            healthy(store.clone(), FacetKind::Filesystem),
            failing(
                FacetKind::Build,
                ObserverError::Unavailable {
                    kind: FacetKind::Build,
                    detail: "storing the facet evidence document failed: the scratch store is \
                             configured to fail writes"
                        .to_owned(),
                },
            ),
        ];
        let assembler = ObservationAssembler::new(store);
        let assembled = assembler
            .assemble(plan_with(observers, CANARY_CLAIM, clock()), bound())
            .expect("the failing OBSERVER is a gap, not an assembly failure");
        assert_eq!(assembled.doc.facets().len(), 1);
        assert_eq!(
            assembled.doc.unavailable_evidence()[0].kind(),
            FacetKind::Build.token()
        );
        assert_eq!(
            assembled.doc.observation_failures()[0].error_class(),
            "unavailable"
        );
    }

    #[test]
    fn a_failing_bundle_store_write_is_a_typed_assembly_error() {
        let failing_store = Arc::new(ScratchStore::failing());
        let healthy_store = Arc::new(ScratchStore::new());
        let observers: Vec<Box<dyn Observer>> = vec![healthy(healthy_store, FacetKind::Test)];
        let assembler = ObservationAssembler::new(failing_store);
        match assembler.assemble(plan_with(observers, CANARY_CLAIM, clock()), bound()) {
            Err(AssemblyError::BundleStoreWrite { detail }) => {
                assert!(
                    detail.contains("fail writes"),
                    "the detail names the failure: {detail}"
                );
            }
            other => panic!("a bundle store failure must be a typed error, got {other:?}"),
        }
    }

    /// C4: `assemble` refuses malformed plans with typed errors — an empty
    /// observer list would assemble an "all-healthy" attestation over zero
    /// probes.
    #[test]
    fn an_empty_observer_list_is_refused() {
        let store = Arc::new(ScratchStore::new());
        let assembler = ObservationAssembler::new(store);
        match assembler.assemble(plan_with(Vec::new(), CANARY_CLAIM, clock()), bound()) {
            Err(AssemblyError::EmptyObserverList) => {}
            other => panic!("an empty plan must be refused, got {other:?}"),
        }
    }

    /// C4: the plan's `environment_fingerprint` must be `sha256:` + exactly
    /// 64 lowercase hex characters — anything else is a typed refusal.
    #[test]
    fn a_badly_shaped_environment_fingerprint_is_refused() {
        let badly_shaped = [
            String::new(),
            "not-a-fingerprint".to_owned(),
            // Right algorithm, wrong lengths.
            "sha256:".to_owned(),
            "sha256:0123abcd".to_owned(),
            format!("sha256:{}", "a".repeat(63)),
            format!("sha256:{}", "a".repeat(65)),
            // Right length, wrong alphabet or algorithm.
            format!("sha256:{}", "A".repeat(64)),
            format!("sha256:{}", "g".repeat(64)),
            format!("blake2b:{}", "a".repeat(64)),
        ];
        for fingerprint in badly_shaped {
            let store = Arc::new(ScratchStore::new());
            let observers: Vec<Box<dyn Observer>> = vec![healthy(store.clone(), FacetKind::Test)];
            let plan = ObservationPlan {
                environment_fingerprint: fingerprint.clone(),
                ..plan_with(observers, CANARY_CLAIM, clock())
            };
            let assembler = ObservationAssembler::new(store);
            match assembler.assemble(plan, bound()) {
                Err(AssemblyError::BadFingerprintShape { fingerprint: seen }) => {
                    assert_eq!(seen, fingerprint, "the refusal names the fingerprint");
                }
                other => panic!("fingerprint {fingerprint} must be refused, got {other:?}"),
            }
        }
    }

    /// C4: an empty `project_revision` is a typed refusal — the bundle's
    /// wire contract itself requires it non-empty.
    #[test]
    fn an_empty_project_revision_is_refused() {
        let store = Arc::new(ScratchStore::new());
        let observers: Vec<Box<dyn Observer>> = vec![healthy(store.clone(), FacetKind::Test)];
        let plan = ObservationPlan {
            project_revision: String::new(),
            ..plan_with(observers, CANARY_CLAIM, clock())
        };
        let assembler = ObservationAssembler::new(store);
        match assembler.assemble(plan, bound()) {
            Err(AssemblyError::EmptyProjectRevision) => {}
            other => panic!("an empty revision must be refused, got {other:?}"),
        }
    }

    #[test]
    fn assembly_is_deterministic_under_the_same_clock_and_environment() {
        let first_store = Arc::new(ScratchStore::new());
        let first_observers: Vec<Box<dyn Observer>> =
            vec![healthy(first_store.clone(), FacetKind::Test)];
        let first = ObservationAssembler::new(first_store.clone())
            .assemble(plan_with(first_observers, CANARY_CLAIM, clock()), bound())
            .expect("first assembly");

        let second_store = Arc::new(ScratchStore::new());
        let second_observers: Vec<Box<dyn Observer>> =
            vec![healthy(second_store.clone(), FacetKind::Test)];
        let second = ObservationAssembler::new(second_store)
            .assemble(plan_with(second_observers, CANARY_CLAIM, clock()), bound())
            .expect("second assembly");

        assert_eq!(
            first.doc.to_json(),
            second.doc.to_json(),
            "same observers, clock, and environment assemble byte-identical documents"
        );
        assert_eq!(
            first.doc.observation_digest(),
            second.doc.observation_digest()
        );
        // The stored field always equals a recomputation over the document.
        assert_eq!(
            first.doc.observation_digest(),
            &observation_digest(&first.doc),
            "the digest field is the recomputed digest"
        );
    }

    #[test]
    fn the_assembled_document_round_trips_and_feeds_verification_inputs() {
        let store = Arc::new(ScratchStore::new());
        let observers: Vec<Box<dyn Observer>> = vec![healthy(store.clone(), FacetKind::Test)];
        let assembler = ObservationAssembler::new(store);
        let assembled = assembler
            .assemble(plan_with(observers, CANARY_CLAIM, clock()), bound())
            .expect("assembly");
        let text = assembled.doc.to_json();
        let back = ObservationBundleDoc::decode(&text).expect("the stored form decodes strictly");
        assert_eq!(back, assembled.doc);
        let inputs = VerificationInputs::of_bundle(&back, assembled.bundle_ref.clone());
        assert_eq!(inputs.facets().len(), 1);
        assert!(!inputs.to_json().contains(CANARY_CLAIM));
    }
}
