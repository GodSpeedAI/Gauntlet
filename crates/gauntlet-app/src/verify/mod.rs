//! The application-owned verification service (plan Task 11; spec §14,
//! §16.1 conditions 1–2, P12).
//!
//! This module is the **only** route from a durable run to a consequential
//! verification record, and it is shaped by the invariant Task 10 learned
//! the hard way and the Task-11 preregistration predicts will be violated
//! (P1): **no binding check may compare two values the same caller already
//! supplied.** Concretely, for every §14.3 check:
//!
//! - the **requirement** side (kind, locator, producer) comes from the
//!   frozen reference projection — the durable run's own
//!   [`ReferenceVersion`](gauntlet_domain::ReferenceVersion), loaded and
//!   digest-verified here, parsed into the answer key, the unit's
//!   `evidence_required` read from it. The `VerificationRequest`'s own
//!   `requirement` field is the caller's say-so and is never read for any
//!   binding decision.
//! - the **evidence** side (ref, producer identity, locator, iteration,
//!   ordering anchor) comes from the durable write-once
//!   [`EvidenceProductionBinding`](gauntlet_ports::EvidenceProductionBinding)
//!   rows the evidence owner recorded, plus the `EvidenceStore` for
//!   resolvability. The request's `evidence` locator field is never read
//!   for any binding decision.
//! - the **recency** decision reads the production record's ordering anchor
//!   (iteration + event-log offset minted by the state store's append) — a
//!   value no caller of `verify` supplies. A request that lies about
//!   recency, producer, or locator changes nothing: there is no parameter
//!   those lies can travel through. The ITERATION ANCHOR itself is
//!   validated against the run's latest durable checkpoint (correction
//!   round 1, C5/B-M2): an open at an iteration no durable state supports
//!   — older (re-freshening prior-iteration evidence) or newer (certifying
//!   a pass that never happened) — is a typed admission refusal.
//!
//! The chain mirrors `compare::authority` (the Task-10 typestate): earned
//! governed state, `pub(crate)` sealing, unforgeable products:
//!
//! ```text
//! VerificationAuthority::for_run(run, iteration, LADDER, WORKSPACE_ROOT) -> GovernedVerification
//!   (durable run + durable-iteration anchor + verified frozen projection
//!    + the composition root's rung set — C1: the ladder is earned
//!    authority injected at the open, and no public ladder constructor
//!    exists in a normal build of this crate — and the canonicalized
//!    evidence workspace root — F1: the same root the adapters bind, under
//!    which the chain re-reads locator bytes before dispatch)
//! GovernedVerification::verify_claim -> VerifiedClaim            (the only verification mint)
//! settlement_precondition(&VerifiedClaim) -> SettlementReadiness  (§16.1 conditions 1–2 only;
//!    C4 — evidence facts re-derived from the claim's OWN chain store,
//!    never a caller-presented one)
//! ```
//!
//! Layered error/outcome reconciliation (recorded decision): an instrument
//! that cannot find the evidence reports the typed
//! [`VerifierError::EvidenceMissing`] (the Task-4 suite pins that
//! instrument-level behavior); THIS chain converts that error — and every
//! §14.3 binding failure — into an `Outcome::Inconclusive` record carrying
//! the typed [`BindingClass`](gauntlet_ports::BindingClass). Missing
//! evidence is never silence, never an error escape hatch, and never PASS
//! (P12).
//!
//! The model rung ([`ladder::ModelRung`]) is `is_deterministic() == false`
//! and every record minted over it carries B.5 `instrument.deterministic:
//! false`; §14.2's cheapest-conclusive-first ordering keeps it behind every
//! deterministic rung that can decide the requirement.

pub mod authority;
pub mod ladder;
pub mod precondition;

pub use authority::{
    GovernedVerification, VerificationAdmissionError, VerificationAuthority, VerificationRefusal,
    VerifiedClaim,
};
pub use ladder::{Ladder, LadderError, LadderRung, ModelRung, RungKind, VERIFIER_PROMPT};
pub use precondition::{PreconditionFailure, SettlementReadiness, settlement_precondition};

/// SHA-256 of raw bytes as a domain [`gauntlet_domain::Digest`] — the
/// production content-digest discipline's one hashing rule (C3). Public so
/// the composition root's producer-side wiring records the SAME digest the
/// chain re-derives from the store's bytes (an honest producer computes it
/// through the application layer's own rule, never a second
/// implementation).
pub fn content_digest(bytes: &[u8]) -> gauntlet_domain::Digest {
    use sha2::{Digest as _, Sha256};
    let hex: String = Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    gauntlet_domain::Digest::new("sha256", &hex).expect("SHA-256 hex is a valid digest")
}

/// Test substrate for the chain's unit tests (the `compare::testing`
/// pattern): the earned chain is opened over REAL governed reads — a state
/// fake that actually holds the run and its production bindings, and an
/// evidence fake that actually pins — never over a forged authority.
#[cfg(test)]
pub(crate) mod testing {
    use std::sync::{Arc, Mutex};

    use gauntlet_domain::{
        Budget, DiagnosticAllowance, Digest, EpochSeconds, EventLogOffset, EvidenceKind,
        EvidenceLocator, EvidenceRef, InstrumentName, IterationNo, Limits, MicroUsd, ReferenceId,
        ReferenceVersion, Run, RunDefinition, RunId,
    };
    use gauntlet_ports::deadline::Deadline;
    use gauntlet_ports::evidence_store::{EvidenceStore, EvidenceStoreError};
    use gauntlet_ports::observer::InstrumentIdentity;
    use gauntlet_ports::state_store::{
        Checkpoint, EvidenceProduction, EvidenceProductionBinding, RunEvent, StateStore,
        StateStoreError,
    };
    use sha2::{Digest as _, Sha256};

    pub(crate) const BASE_INSTANT: u64 = 1_700_000_000;
    pub(crate) const RUN_ULID: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";

    /// A minimal well-formed answer key whose AK-9001 unit requires a
    /// `test_report` at the fixture locator, produced by an instrument, and
    /// whose AK-9002 unit requires `human` sign-off evidence.
    pub(crate) const ANSWER_KEY: &str = "## AK-9001\nclaim: the fixture claim is verified by a \
         test report\nverify_by: test\nblast_radius: local\nauthority: critic\nevidence_required: \
         {kind: test_report, locator: fixture-report-locator, producer: instrument}\n\n## \
         AK-9002\nclaim: the fixture claim needs human sign-off\nverify_by: \
         human_signoff\nblast_radius: local\nauthority: human\nevidence_required: {kind: \
         attestation, locator: fixture-signoff-locator, producer: human}\n";

    /// The digest of the normalized fixture view — what the run's frozen
    /// `ReferenceVersion` pins. Normalization mirrors the projection's
    /// rules (CRLF folded, exactly one trailing newline).
    pub(crate) fn answer_key_digest() -> Digest {
        let normalized = normalize(ANSWER_KEY);
        let hex: String = Sha256::digest(normalized.as_bytes())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        Digest::new("sha256", &hex).expect("SHA-256 hex is a valid digest")
    }

    fn normalize(text: &str) -> String {
        let folded = text.replace("\r\n", "\n").replace('\r', "\n");
        let mut lines: Vec<&str> = folded.split('\n').collect();
        while matches!(lines.last(), Some(last) if last.is_empty()) {
            lines.pop();
        }
        if lines.is_empty() {
            String::new()
        } else {
            format!("{}\n", lines.join("\n"))
        }
    }

    pub(crate) fn fixture_run(reference_evidence: EvidenceRef) -> Run {
        Run::new(RunDefinition {
            id: RunId::new(RUN_ULID).expect("fixture run id"),
            created_at: EpochSeconds::new(BASE_INSTANT),
            reference_version: ReferenceVersion::with_evidence(
                ReferenceId::new("fixture-reference").expect("valid reference id"),
                1,
                answer_key_digest(),
                reference_evidence,
            ),
            intent: "verify-chain fixture".to_owned(),
            config_fingerprint: Digest::new("sha256", &"aa".repeat(32)).expect("digest"),
            environment_fingerprint: Digest::new("sha256", &"bb".repeat(32)).expect("digest"),
            budget: Budget::new(
                Limits::new(8, 2, 2, 3600, 1000, MicroUsd::new(500)).expect("limits"),
                20,
                DiagnosticAllowance::new(100, MicroUsd::new(50)),
            )
            .expect("budget"),
            parent_run: None,
        })
        .expect("the fixture run is valid")
    }

    /// SHA-256 of raw bytes as a domain [`Digest`] (delegates to the
    /// module's production helper).
    pub(crate) fn sha256_digest(bytes: &[u8]) -> Digest {
        super::content_digest(bytes)
    }

    fn zero_spent() -> gauntlet_domain::Spent {
        *Budget::new(
            Limits::new(1, 1, 1, 1, 1, MicroUsd::new(1)).expect("minimal legal ceilings"),
            1,
            DiagnosticAllowance::new(1, MicroUsd::new(1)),
        )
        .expect("minimal legal budget")
        .spent()
    }

    /// A production-record-holding state store: one durable run, its
    /// production bindings, monotonically minted event offsets (the
    /// store-side anchor, never a caller value), and a durable checkpoint
    /// the fixture can advance (the C5 anchor-validation substrate). It
    /// also owns a REAL scratch workspace root (correction round 2, F1):
    /// the producer-side `record` writes its artifact at the locator under
    /// that root, so the chain's locator re-read — and any doctored
    /// overwrite a test performs — operate on actual files, the way the
    /// composed path does.
    pub(crate) struct VerifyingState {
        run: Run,
        workspace: std::path::PathBuf,
        productions: Mutex<Vec<EvidenceProductionBinding>>,
        next_offset: Mutex<u64>,
        checkpoint: Mutex<Option<Checkpoint>>,
        leak_off_locator: Mutex<bool>,
    }

    impl Drop for VerifyingState {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.workspace);
        }
    }

    impl VerifyingState {
        pub(crate) fn new(run: Run) -> Self {
            static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let seq = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let workspace = std::env::temp_dir().join(format!(
                "gauntlet-verify-authority-{}-{seq}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&workspace);
            std::fs::create_dir_all(&workspace).expect("the fixture workspace root is creatable");
            Self {
                run,
                workspace,
                productions: Mutex::new(Vec::new()),
                next_offset: Mutex::new(0),
                checkpoint: Mutex::new(None),
                leak_off_locator: Mutex::new(false),
            }
        }

        /// The fixture's real workspace root — what a governed open pins
        /// as its evidence workspace (F1). The state store hands it out so
        /// the same fixture root travels to `for_run` and to any doctored
        /// write a test performs.
        pub(crate) fn workspace_root(&self) -> &std::path::Path {
            &self.workspace
        }

        /// Makes `evidence_productions_at` ignore its locator filter — a
        /// CONTRACT-VIOLATING store (the C6.2 tooth: the app-layer locator
        /// check must bite a store that leaks off-locator productions).
        pub(crate) fn leak_off_locator(&self) {
            *self.leak_off_locator.lock().expect("not poisoned") = true;
        }

        /// Writes the run's durable checkpoint at `iteration` (the §18
        /// boundary discipline), the way the loop's controller does.
        pub(crate) fn checkpoint_at(&self, iteration: u32) {
            let offset = {
                let mut next = self.next_offset.lock().expect("not poisoned");
                *next += 1;
                *next
            };
            let checkpoint = Checkpoint {
                run_id: self.run.id().clone(),
                reference_digest: self.run.reference_version().digest().clone(),
                iteration_no: IterationNo::new(iteration).expect("1-based"),
                unit_states: Vec::new(),
                budget_spent: zero_spent(),
                in_flight: Vec::new(),
                event_log_offset: EventLogOffset::new(offset),
            };
            *self.checkpoint.lock().expect("not poisoned") = Some(checkpoint);
        }

        /// The producer-side write (the evidence owner's), as the
        /// embedded-SQL adapter performs it: the offset is minted HERE,
        /// inside the store, and the record carries the digest of the
        /// ingested payload (C3's content binding — pass the SAME bytes the
        /// evidence store holds, as an honest producer does). F1: the
        /// write also lands the artifact at the locator under the fixture's
        /// real workspace root — an honest producer's bytes at the locator,
        /// in the store, and in the recorded digest are ONE fact, so the
        /// chain's three-way binding has a truthful substrate to defend.
        pub(crate) fn record(
            &self,
            evidence: &str,
            producer: &str,
            locator: &str,
            iteration: u32,
            payload: &[u8],
        ) -> EvidenceProductionBinding {
            let path = self.workspace.join(locator);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).expect("the fixture artifact parent is creatable");
            }
            std::fs::write(&path, payload).expect("the fixture artifact lands at the locator");
            let production = EvidenceProduction::new(
                EvidenceRef::new(evidence).expect("fixture evidence ref"),
                sha256_digest(payload),
                InstrumentIdentity::new(
                    InstrumentName::new(producer).expect("fixture instrument name"),
                    "1".to_owned(),
                ),
                EvidenceLocator::new(locator).expect("fixture locator"),
                IterationNo::new(iteration).expect("1-based"),
                EpochSeconds::new(BASE_INSTANT + u64::from(iteration) * 100),
            );
            let offset = {
                let mut next = self.next_offset.lock().expect("not poisoned");
                *next += 1;
                EventLogOffset::new(*next)
            };
            let binding = EvidenceProductionBinding::new(self.run.id().clone(), production, offset);
            self.productions
                .lock()
                .expect("not poisoned")
                .push(binding.clone());
            binding
        }
    }

    impl StateStore for VerifyingState {
        fn load_run(&self, run_id: &RunId, _: Deadline) -> Result<Option<Run>, StateStoreError> {
            Ok((run_id == self.run.id()).then(|| self.run.clone()))
        }
        fn append_event(
            &self,
            _: RunEvent,
            _: Deadline,
        ) -> Result<EventLogOffset, StateStoreError> {
            let mut next = self.next_offset.lock().expect("not poisoned");
            *next += 1;
            Ok(EventLogOffset::new(*next))
        }
        fn checkpoint(&self, checkpoint: Checkpoint, _: Deadline) -> Result<(), StateStoreError> {
            *self.checkpoint.lock().expect("not poisoned") = Some(checkpoint);
            Ok(())
        }
        fn resume(&self, _: &RunId, _: Deadline) -> Result<Option<Checkpoint>, StateStoreError> {
            Ok(self.checkpoint.lock().expect("not poisoned").clone())
        }
        fn evidence_production(
            &self,
            run_id: &RunId,
            evidence: &EvidenceRef,
            _: Deadline,
        ) -> Result<Option<EvidenceProductionBinding>, StateStoreError> {
            Ok(self
                .productions
                .lock()
                .expect("not poisoned")
                .iter()
                .find(|binding| {
                    binding.run_id() == run_id && binding.production().evidence() == evidence
                })
                .cloned())
        }
        fn evidence_productions_at(
            &self,
            run_id: &RunId,
            locator: &EvidenceLocator,
            _: Deadline,
        ) -> Result<Vec<EvidenceProductionBinding>, StateStoreError> {
            let leak = *self.leak_off_locator.lock().expect("not poisoned");
            let mut found: Vec<EvidenceProductionBinding> = self
                .productions
                .lock()
                .expect("not poisoned")
                .iter()
                .filter(|binding| {
                    binding.run_id() == run_id
                        && (leak || binding.production().locator() == locator)
                })
                .cloned()
                .collect();
            found.sort_by_key(|binding| binding.event_log_offset().get());
            Ok(found)
        }
        fn export_jsonl(&self, _: &RunId, _: Deadline) -> Result<String, StateStoreError> {
            Ok(String::new())
        }
    }

    /// A memory evidence store with REAL pin semantics (put/get/pin/
    /// is_pinned), so the settlement precondition's condition 2 is exercised
    /// against an honest store.
    #[derive(Default)]
    pub(crate) struct PinningEvidence {
        pub(crate) objects: Mutex<Vec<(EvidenceRef, Vec<u8>)>>,
        pub(crate) pins: Mutex<Vec<(RunId, EvidenceRef)>>,
        /// When set, `is_pinned` fails — the fail-closed unknown state.
        /// Interior-mutable so an `Arc`-shared fixture can flip it.
        pin_query_fails: Mutex<bool>,
        /// When set, `get` fails — the unanswerable-read state (C6.1).
        /// Interior-mutable so an `Arc`-shared fixture can flip it.
        pub(crate) read_fails: Mutex<bool>,
    }

    impl PinningEvidence {
        /// Makes every subsequent `get` fail (the unanswerable-read state).
        pub(crate) fn fail_reads(&self) {
            *self.read_fails.lock().expect("not poisoned") = true;
        }

        /// Makes every subsequent `is_pinned` fail (the unknown state).
        pub(crate) fn fail_pin_queries(&self) {
            *self.pin_query_fails.lock().expect("not poisoned") = true;
        }

        pub(crate) fn insert(&self, name: &str, bytes: &[u8]) -> EvidenceRef {
            let reference = EvidenceRef::new(name).expect("valid evidence ref");
            self.objects
                .lock()
                .expect("not poisoned")
                .push((reference.clone(), bytes.to_vec()));
            reference
        }

        /// Removes one object (the bytes-vanish-between-verify-and-settle
        /// state for the precondition tests).
        pub(crate) fn remove(&self, name: &str) {
            let reference = EvidenceRef::new(name).expect("valid evidence ref");
            self.objects
                .lock()
                .expect("not poisoned")
                .retain(|(found, _)| *found != reference);
        }

        pub(crate) fn pin(&self, run: &RunId, reference: &EvidenceRef) {
            self.pins
                .lock()
                .expect("not poisoned")
                .push((run.clone(), reference.clone()));
        }
    }

    impl EvidenceStore for PinningEvidence {
        fn put(
            &self,
            bytes: &[u8],
            _: EvidenceKind,
            _: Deadline,
        ) -> Result<EvidenceRef, EvidenceStoreError> {
            let index = self.objects.lock().expect("not poisoned").len();
            Ok(self.insert(&format!("evd-verify-{index:06}"), bytes))
        }
        fn get(
            &self,
            reference: &EvidenceRef,
            _: Deadline,
        ) -> Result<Option<Vec<u8>>, EvidenceStoreError> {
            if *self.read_fails.lock().expect("not poisoned") {
                return Err(EvidenceStoreError::ReadFailed {
                    detail: "the fixture store cannot serve reads".to_owned(),
                });
            }
            Ok(self
                .objects
                .lock()
                .expect("not poisoned")
                .iter()
                .find(|(found, _)| found == reference)
                .map(|(_, bytes)| bytes.clone()))
        }
        fn pin(
            &self,
            run_id: &RunId,
            reference: &EvidenceRef,
            _: Deadline,
        ) -> Result<(), EvidenceStoreError> {
            self.pins
                .lock()
                .expect("not poisoned")
                .push((run_id.clone(), reference.clone()));
            Ok(())
        }
        fn is_pinned(
            &self,
            run_id: &RunId,
            reference: &EvidenceRef,
            _: Deadline,
        ) -> Result<bool, EvidenceStoreError> {
            if *self.pin_query_fails.lock().expect("not poisoned") {
                return Err(EvidenceStoreError::ReadFailed {
                    detail: "the fixture store cannot answer pin queries".to_owned(),
                });
            }
            Ok(self
                .pins
                .lock()
                .expect("not poisoned")
                .contains(&(run_id.clone(), reference.clone())))
        }
    }

    /// A deterministic rung-sized verifier for the chain tests: decides by
    /// KIND (the `supports` declaration) and evaluates the evidence content
    /// the fixture placed at the request's locator.
    pub(crate) struct FakeDeterministicVerifier {
        pub(crate) name: &'static str,
        pub(crate) kinds: &'static [EvidenceKind],
        /// locators whose content PASSES; everything else present FAILs.
        pub(crate) passing: Vec<String>,
    }

    impl gauntlet_ports::verifier::Verifier for FakeDeterministicVerifier {
        fn verify(
            &self,
            claim: &gauntlet_ports::verifier::VerificationRequest,
            _: Deadline,
        ) -> Result<gauntlet_domain::VerificationRecord, gauntlet_ports::verifier::VerifierError>
        {
            use gauntlet_domain::VerificationRecord;
            use gauntlet_ports::verifier::VerifierError;
            if !self
                .passing
                .iter()
                .any(|locator| locator == claim.evidence.as_str())
            {
                // Absent from the passing set: the fixture treats it as the
                // instrument finding no evidence there (the P12 instrument
                // behavior the shared suite pins).
                return Err(VerifierError::EvidenceMissing {
                    locator: claim.evidence.clone(),
                });
            }
            Ok(VerificationRecord::new(
                claim.id.clone(),
                claim.run_id.clone(),
                claim.unit_id.clone(),
                gauntlet_domain::EvidenceRef::new(claim.evidence.as_str())
                    .expect("a locator string is a valid ref string"),
                gauntlet_domain::Outcome::Pass,
                gauntlet_domain::InstrumentName::new(self.name)
                    .expect("the fixture name is a plain token"),
            ))
        }
        fn is_deterministic(&self) -> bool {
            true
        }
        fn instrument(&self) -> InstrumentIdentity {
            InstrumentIdentity::new(
                InstrumentName::new(self.name).expect("plain token"),
                "1".to_owned(),
            )
        }
        fn supports(&self, requirement: &gauntlet_domain::EvidenceRequirement) -> bool {
            self.kinds.contains(&requirement.kind())
        }
    }

    pub(crate) fn bound() -> Deadline {
        gauntlet_ports::deadline::Deadline::at_epoch(EpochSeconds::new(BASE_INSTANT + 90))
    }

    /// The seeded id source the chain mints verification refs from.
    pub(crate) struct SeededIds(Mutex<u64>);

    impl SeededIds {
        pub(crate) fn new() -> Self {
            Self(Mutex::new(0))
        }
    }

    impl gauntlet_ports::id_generator::IdGenerator for SeededIds {
        fn run_id(&self) -> RunId {
            RunId::new(RUN_ULID).expect("fixture run id")
        }
        fn invocation_id(&self) -> gauntlet_domain::InvocationId {
            let next = {
                let mut guard = self.0.lock().expect("not poisoned");
                *guard += 1;
                *guard
            };
            gauntlet_domain::InvocationId::new(&format!("01ARZ3NDEKTSV4RRFFQ69G5F{next:02X}"))
                .expect("fixture invocation id")
        }
    }

    pub(crate) fn authority_parts() -> (Arc<VerifyingState>, Arc<PinningEvidence>) {
        let evidence = Arc::new(PinningEvidence::default());
        let frozen = evidence.insert("evd-fixture-frozen-reference", ANSWER_KEY.as_bytes());
        let state = Arc::new(VerifyingState::new(fixture_run(frozen)));
        (state, evidence)
    }
}
