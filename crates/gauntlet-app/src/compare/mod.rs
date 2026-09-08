//! Comparison services (spec §9.5, §13; plan Task 10): the comparison
//! contract artifact ([`contract`]) and the §13.2 allowlist envelope
//! builder with the consumption-boundary digest recompute ([`envelope`]).
//! Later Task-10 slices add deterministic §13.5 aggregation and §13.3
//! review-mode dispatch under this module.
//!
//! Operates on the domain through ports only. Deterministic by
//! construction: no adapter, no clock, and no randomness sit in any merge
//! path (spec §13.5 — "Aggregation MUST NOT be performed by a model").

pub mod authority;
pub mod contract;
pub mod recording;

// Crate-private on purpose. `aggregate`, `envelope` and `review` are the three
// steps that confer authority or consequence; exposing any of them publicly
// reopens the bypass class the admission service exists to close (see
// `.agents/reports/task10/architecture-diagnosis.md`). The governed chain in
// [`authority`] is the only public route.
pub(crate) mod aggregate;
pub(crate) mod envelope;
pub(crate) mod review;

pub use aggregate::{AGGREGATE_RUNNER, AggregateError};
pub use authority::{
    AcceptedComparison, AdmissionError, ComparisonAuthority, CompletedReview, GovernedRun,
    IssuedComparison,
};
pub use contract::{ComparisonContract, ContractError, SUPPORTED_CONTRACT_VERSION};
pub use envelope::{BlindnessViolation, DENY_LISTED_FIELD_NAMES, EnvelopeError, assert_blind};
pub use recording::RecordingAgentRunner;
pub use review::{
    ModelDispatchBudget, ReviewDeps, ReviewError, ReviewOutcome, UnavailableEvidence,
};

/// Test-only substrate for the governed chain.
///
/// Deliberately `#[cfg(test)]`: there is no public or feature-gated way to mint
/// a [`gauntlet_ports::comparator::RunComparisonAuthority`] outside a durable
/// `StateStore` read, so unit tests stand up a real (in-memory) store and a
/// real recorded run rather than being handed a forged authority.
#[cfg(test)]
pub(crate) mod testing {
    use std::sync::{Arc, Mutex};

    use gauntlet_domain::EventLogOffset;
    use gauntlet_domain::{
        Budget, DiagnosticAllowance, Digest, EpochSeconds, EvidenceKind, EvidenceRef, IterationNo,
        Limits, MicroUsd, ReferenceId, ReferenceVersion, Run, RunDefinition, RunId,
    };
    use gauntlet_ports::comparator::{
        FrozenReferenceProjection, MandatoryComparisonAcceptance, RunComparisonAuthority,
    };
    use gauntlet_ports::deadline::Deadline;
    use gauntlet_ports::evidence_store::{EvidenceStore, EvidenceStoreError};
    use gauntlet_ports::state_store::{Checkpoint, RunEvent, StateStore, StateStoreError};

    /// A state store holding exactly one recorded run — the durable anchor the
    /// authority is earned from.
    pub(crate) struct OneRunStore(
        pub(crate) Run,
        pub(crate) Vec<(RunId, IterationNo, ObservationBinding)>,
    );

    use gauntlet_ports::state_store::ObservationBinding;

    impl StateStore for OneRunStore {
        fn observation_binding(
            &self,
            run_id: &RunId,
            iteration_no: IterationNo,
            _: Deadline,
        ) -> Result<Option<ObservationBinding>, StateStoreError> {
            Ok(self
                .1
                .iter()
                .find(|(run, iteration, _)| run == run_id && *iteration == iteration_no)
                .map(|(_, _, binding)| binding.clone()))
        }

        fn append_event(
            &self,
            _: RunEvent,
            _: Deadline,
        ) -> Result<EventLogOffset, StateStoreError> {
            Ok(EventLogOffset::new(0))
        }
        fn load_run(&self, run_id: &RunId, _: Deadline) -> Result<Option<Run>, StateStoreError> {
            Ok((run_id == self.0.id()).then(|| self.0.clone()))
        }
        fn checkpoint(&self, _: Checkpoint, _: Deadline) -> Result<(), StateStoreError> {
            Ok(())
        }
        fn resume(&self, _: &RunId, _: Deadline) -> Result<Option<Checkpoint>, StateStoreError> {
            Ok(None)
        }
        fn export_jsonl(&self, _: &RunId, _: Deadline) -> Result<String, StateStoreError> {
            Ok(String::new())
        }
    }

    /// Content-addressed enough for the chain: `get` returns what `put` stored.
    #[derive(Default)]
    pub(crate) struct MemoryEvidence(Mutex<Vec<(EvidenceRef, Vec<u8>)>>);

    impl MemoryEvidence {
        pub(crate) fn insert(&self, name: &str, bytes: &[u8]) -> EvidenceRef {
            let reference = EvidenceRef::new(name).expect("valid evidence ref");
            self.0
                .lock()
                .expect("not poisoned")
                .push((reference.clone(), bytes.to_vec()));
            reference
        }
    }

    impl EvidenceStore for MemoryEvidence {
        fn put(
            &self,
            bytes: &[u8],
            _: EvidenceKind,
            _: Deadline,
        ) -> Result<EvidenceRef, EvidenceStoreError> {
            let index = self.0.lock().expect("not poisoned").len();
            Ok(self.insert(&format!("evd-memory-{index:06}"), bytes))
        }
        fn get(
            &self,
            reference: &EvidenceRef,
            _: Deadline,
        ) -> Result<Option<Vec<u8>>, EvidenceStoreError> {
            Ok(self
                .0
                .lock()
                .expect("not poisoned")
                .iter()
                .find(|(stored, _)| stored == reference)
                .map(|(_, bytes)| bytes.clone()))
        }
        fn pin(&self, _: &RunId, _: &EvidenceRef, _: Deadline) -> Result<(), EvidenceStoreError> {
            Ok(())
        }
    }

    /// The fixtures' fixed instant; the deadline is derived from it so no
    /// duration literal sits at a deadline construction site (spec §8.1).
    const BASE_INSTANT: u64 = 1_700_000_000;
    /// A fixed instant far enough ahead of [`BASE_INSTANT`] that no fixture
    /// call expires; derived, so no duration literal sits at the deadline site.
    const FIXTURE_HORIZON: u64 = BASE_INSTANT * 2;

    pub(crate) fn bound() -> Deadline {
        Deadline::at_epoch(EpochSeconds::new(FIXTURE_HORIZON))
    }

    fn digest_of(bytes: &[u8]) -> Digest {
        use sha2::{Digest as _, Sha256};
        let hex: String = Sha256::digest(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        Digest::new("sha256", &hex).expect("sha-256 output is a valid digest")
    }

    /// Records a run whose frozen reference IS `view`, then earns the authority
    /// and loads the projection through the same governed calls production uses.
    pub(crate) fn governed(
        run_id: &RunId,
        iteration_no: IterationNo,
        view: &str,
        bundle: &[u8],
    ) -> Governed {
        let deadline = bound();
        let evidence = MemoryEvidence::default();
        let reference = evidence.insert("evd-frozen-reference-0001", view.as_bytes());
        let version = ReferenceVersion::with_evidence(
            ReferenceId::new("governed-fixture").expect("reference id"),
            1,
            digest_of(view.as_bytes()),
            reference,
        );
        let run = Run::new(RunDefinition {
            id: run_id.clone(),
            created_at: EpochSeconds::new(BASE_INSTANT),
            reference_version: version,
            intent: "governed test fixture".to_owned(),
            config_fingerprint: digest_of(b"config"),
            environment_fingerprint: digest_of(b"environment"),
            budget: Budget::new(
                Limits::new(8, 4, 4, 600, 1_000_000, MicroUsd::new(1_000_000))
                    .expect("the fixture limits are non-zero"),
                10,
                DiagnosticAllowance::new(1_000, MicroUsd::new(1_000)),
            )
            .expect("the fixture budget is valid"),
            parent_run: None,
        })
        .expect("the fixture run is valid");
        let bundle_ref = evidence.insert("evd-observation-bundle-0001", bundle);
        let store = Arc::new(OneRunStore(
            run,
            vec![(
                run_id.clone(),
                iteration_no,
                ObservationBinding::new(
                    bundle_ref.clone(),
                    gauntlet_ports::observation_digest::observation_digest(
                        &gauntlet_ports::json::parse(
                            core::str::from_utf8(bundle).expect("the fixture bundle is UTF-8"),
                        )
                        .expect("the fixture bundle is JSON"),
                    ),
                ),
            )],
        ));
        let authority =
            RunComparisonAuthority::open(store.as_ref(), run_id, iteration_no, deadline)
                .expect("the durable run yields authority");
        let evidence = Arc::new(evidence);
        let projection = FrozenReferenceProjection::load(&authority, evidence.as_ref(), deadline)
            .expect("the pinned frozen evidence projects");
        Governed {
            authority,
            projection,
            evidence,
            state: store,
            bundle_ref,
        }
    }

    /// Everything the governed chain needs, as the fixture earned it.
    pub(crate) struct Governed {
        pub(crate) authority: RunComparisonAuthority,
        pub(crate) projection: FrozenReferenceProjection,
        pub(crate) evidence: Arc<MemoryEvidence>,
        pub(crate) state: Arc<OneRunStore>,
        #[allow(dead_code)]
        pub(crate) bundle_ref: EvidenceRef,
    }

    /// The acceptance boundary over the fixture's own substrate.
    pub(crate) fn acceptance(governed: &Governed) -> MandatoryComparisonAcceptance {
        MandatoryComparisonAcceptance::new(
            governed.state.clone(),
            governed.evidence.clone(),
            EpochSeconds::new(BASE_INSTANT),
        )
    }
}
