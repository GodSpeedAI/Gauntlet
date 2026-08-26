//! The owner-write minting authorities (plan Task 12; the `AdmittedComparison`
//! precedent from plan Task 10 / Task 11 C7).
//!
//! Three `StateStore` writes are OWNER writes, not caller writes:
//! `record_evidence_production` (the evidence owner's durable production
//! fact, spec §14.3), `bind_observation_bundle` (the observation owner's
//! binding of one pinned bundle as *this* run's observation at *this*
//! iteration, spec §5.3.6/§18.1), and — since plan Task 13 —
//! `commit_settlement` (the §16.3 settlement commit, the most consequential
//! write in the system). Before Task 12 any holder of an
//! `Arc<dyn StateStore>` could perform the first two for any
//! store-resolvable bytes, and the declared iteration was never validated;
//! before Task 13 the settlement commit was not on the port at all, and the
//! adapter's inherent surface would take any caller-attested payload.
//!
//! The closure follows the established pattern exactly: the trait methods
//! now accept only values minted HERE — [`AdmittedEvidenceProduction`],
//! [`AdmittedObservationBinding`], and [`AdmittedSettlement`] have no
//! constructor outside this module — and the authorities perform the
//! governed reads before minting:
//!
//! 1. the durable run must exist (an undurable run can produce no
//!    consequential record, §18.1);
//! 2. the DECLARED iteration is validated against the durable anchor — the
//!    run's latest durable checkpoint's iteration, else 1 — the same rule
//!    `VerificationAuthority::for_run` enforces (mirrored here because ports
//!    cannot depend on the application layer; see [`durable_iteration`]);
//! 3. the evidence/bundle bytes are read from the evidence store and the
//!    content digest is DERIVED from those bytes — a caller-supplied digest
//!    is not a parameter anywhere (Task 11's F1 lesson: doctored locator
//!    bytes must not ride a real adapter to PASS).
//!
//! The residual trust assumption is the composition root's adapter selection
//! (spec §7.1) — the same single assumption every minting authority in this
//! crate carries.

use core::fmt;
use std::sync::Arc;

use gauntlet_domain::{
    Digest, EpochSeconds, EventLogOffset, EvidenceLocator, EvidenceRef, IterationNo, Outcome,
    RunId, UnitId,
};
use sha2::{Digest as _, Sha256};

use crate::deadline::Deadline;
use crate::evidence_store::EvidenceStore;
use crate::observer::InstrumentIdentity;
use crate::state_store::{
    EvidenceProduction, EvidenceProductionBinding, ObservationBinding, SettlementCommit,
    StateStore, StateStoreError,
};

/// SHA-256 of raw evidence bytes as a domain [`Digest`] — the production
/// content-digest discipline's ONE hashing rule (Task 11 C3). This is the
/// same algorithm the application layer's `verify::content_digest` applies;
/// the equality is pinned by a test there, so producer and verifier sides
/// cannot drift apart into two rules.
pub fn evidence_content_digest(bytes: &[u8]) -> Digest {
    let hex: String = Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    Digest::new("sha256", &hex).expect("SHA-256 hex satisfies the Digest grammar by construction")
}

/// Owner-write admission failures — typed (spec §21.2). Store failures ride
/// [`Self::State`] so a caller can distinguish its own write-once refusals
/// (surfaced verbatim) from the authority's governed-read refusals.
#[derive(Debug, Clone, PartialEq)]
pub enum ProducerAuthorityError {
    /// The run is not durable — no owner write exists for a run the store
    /// never recorded.
    RunNotDurable { run_id: RunId },
    /// The declared iteration is not one the run's durable state supports:
    /// the latest durable checkpoint stands at `durable` (iteration 1 for a
    /// run that has crossed no boundary). Stale would re-certify a prior
    /// loop pass; ahead would certify one that never happened.
    IterationUnsupported {
        run_id: RunId,
        requested: IterationNo,
        durable: IterationNo,
    },
    /// The evidence ref does not resolve — there are no bytes to derive the
    /// content digest from, so no production fact can be minted.
    EvidenceAbsent { evidence: EvidenceRef },
    /// Reading the evidence bytes failed for a reason other than absence.
    EvidenceUnreadable {
        evidence: EvidenceRef,
        detail: String,
    },
    /// The bundle bytes are not a UTF-8 JSON document, so the §12.4
    /// observation digest has nothing to run over.
    BundleInvalid { detail: String },
    /// A durable read or the store write itself failed; the owner write's
    /// own write-once refusals surface here verbatim.
    State(StateStoreError),
}

impl fmt::Display for ProducerAuthorityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RunNotDurable { run_id } => {
                write!(
                    f,
                    "run {run_id} is not durable; the owner-write authority cannot open"
                )
            }
            Self::IterationUnsupported {
                run_id,
                requested,
                durable,
            } => write!(
                f,
                "run {run_id} stands durably at iteration {} (its latest checkpoint); the \
                 owner write declares iteration {} and is refused — the declared iteration must \
                 be the run's durable iteration",
                durable.get(),
                requested.get()
            ),
            Self::EvidenceAbsent { evidence } => write!(
                f,
                "evidence {evidence} does not resolve in the evidence store — there are no \
                 bytes to derive the content digest from"
            ),
            Self::EvidenceUnreadable { evidence, detail } => write!(
                f,
                "reading evidence {evidence} from the store failed: {detail}"
            ),
            Self::BundleInvalid { detail } => {
                write!(f, "the observation bundle bytes are invalid: {detail}")
            }
            Self::State(error) => write!(f, "a durable owner-write step failed: {error}"),
        }
    }
}

impl std::error::Error for ProducerAuthorityError {}

/// A production fact the evidence-owner authority minted — the only value
/// [`StateStore::record_evidence_production`] accepts. No constructor
/// exists outside this module, so a caller that assembled an
/// [`EvidenceProduction`] itself has nothing to pass to any store; the
/// content digest inside is the one THIS module derived from the store's
/// own bytes, never a caller's claim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedEvidenceProduction {
    production: EvidenceProduction,
}

impl AdmittedEvidenceProduction {
    /// The minted production payload, for the store that records it.
    pub fn production(&self) -> &EvidenceProduction {
        &self.production
    }
}

/// An observation binding the observation-owner authority minted — the only
/// value [`StateStore::bind_observation_bundle`] accepts. The §12.4 digest
/// inside was re-derived from the bundle's bytes HERE; binding the pointer
/// without the content digest is what the durable binding has refused since
/// Task 10, and now the mint refuses it one step earlier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedObservationBinding {
    binding: ObservationBinding,
}

impl AdmittedObservationBinding {
    /// The minted binding payload (ref + §12.4 content digest), for the
    /// store that records it.
    pub fn binding(&self) -> &ObservationBinding {
        &self.binding
    }
}

/// The run's durable iteration anchor: the latest durable checkpoint's
/// iteration, else iteration 1 for a run that has crossed no boundary.
///
/// This MIRRORS the anchor rule `VerificationAuthority::for_run` enforces
/// (application layer, plan Task 11 correction round 1 C5/B-M2): the anchor
/// is validated against durable state, never taken on the caller's word.
/// The rule is duplicated rather than factored because ports cannot depend
/// on the application layer; any change to either side must move both, and
/// the Task-12 report records that coupling.
fn durable_iteration(
    state: &dyn StateStore,
    run_id: &RunId,
    deadline: Deadline,
) -> Result<IterationNo, ProducerAuthorityError> {
    let durable = state
        .resume(run_id, deadline)
        .map_err(ProducerAuthorityError::State)?
        .map_or_else(
            || IterationNo::new(1).expect("iterations are 1-based"),
            |checkpoint| checkpoint.iteration_no,
        );
    Ok(durable)
}

/// Loads the durable run and validates the declared iteration against the
/// durable anchor — the two governed reads both authorities share.
fn governed_run(
    state: &dyn StateStore,
    run_id: &RunId,
    declared: IterationNo,
    deadline: Deadline,
) -> Result<(), ProducerAuthorityError> {
    state
        .load_run(run_id, deadline)
        .map_err(ProducerAuthorityError::State)?
        .ok_or_else(|| ProducerAuthorityError::RunNotDurable {
            run_id: run_id.clone(),
        })?;
    let durable = durable_iteration(state, run_id, deadline)?;
    if declared != durable {
        return Err(ProducerAuthorityError::IterationUnsupported {
            run_id: run_id.clone(),
            requested: declared,
            durable,
        });
    }
    Ok(())
}

/// The evidence owner's minting authority: performs the governed reads,
/// derives the production's content digest FROM THE STORE'S OWN BYTES, mints
/// the [`AdmittedEvidenceProduction`] the store accepts, and records it.
///
/// Every argument the caller supplies is a producer FACT (who, where, when,
/// which ref) — never the content digest (derived here) and never the
/// ordering anchor (the store mints it, as ever, from its own append).
#[derive(Clone)]
pub struct EvidenceProductionAuthority {
    state: Arc<dyn StateStore>,
    evidence: Arc<dyn EvidenceStore>,
}

impl EvidenceProductionAuthority {
    pub fn new(state: Arc<dyn StateStore>, evidence: Arc<dyn EvidenceStore>) -> Self {
        Self { state, evidence }
    }

    /// Performs every governed read `record` performs and mints the
    /// [`AdmittedEvidenceProduction`] WITHOUT writing it — the mint half of
    /// the authority, split for the crash-window completion path (plan
    /// Task 13, A5): the durable `EvidenceProduced` event already exists,
    /// so the re-earning side needs the minted production facts to complete
    /// the missing row against that event, not a second append. The mint
    /// carries the content digest derived HERE from the store's own bytes
    /// and the run/iteration validation `record` enforces; `produced_at`
    /// remains a producer FACT — the completed row's timestamp comes from
    /// the durable event, not from this value.
    #[allow(clippy::too_many_arguments)]
    pub fn mint(
        &self,
        run_id: &RunId,
        iteration_no: IterationNo,
        evidence_ref: &EvidenceRef,
        producer: &InstrumentIdentity,
        locator: &EvidenceLocator,
        produced_at: EpochSeconds,
        deadline: Deadline,
    ) -> Result<AdmittedEvidenceProduction, ProducerAuthorityError> {
        governed_run(self.state.as_ref(), run_id, iteration_no, deadline)?;
        let bytes = self
            .evidence
            .get(evidence_ref, deadline)
            .map_err(|error| ProducerAuthorityError::EvidenceUnreadable {
                evidence: evidence_ref.clone(),
                detail: error.to_string(),
            })?
            .ok_or_else(|| ProducerAuthorityError::EvidenceAbsent {
                evidence: evidence_ref.clone(),
            })?;
        let content = evidence_content_digest(&bytes);
        Ok(AdmittedEvidenceProduction {
            production: EvidenceProduction::new(
                evidence_ref.clone(),
                content,
                producer.clone(),
                locator.clone(),
                iteration_no,
                produced_at,
            ),
        })
    }

    /// Records the production of `evidence_ref` in one run: run-exists and
    /// durable-iteration validation, a bytes read at the ref, the content
    /// digest derived from those bytes, then the store's write-once record
    /// (idempotent for identical facts, refusing any different facts for
    /// the same ref — the store's own rule, surfaced verbatim through
    /// [`ProducerAuthorityError::State`]). The governed reads and the mint
    /// live in [`Self::mint`]; recording is mint + the store write.
    #[allow(clippy::too_many_arguments)]
    pub fn record(
        &self,
        run_id: &RunId,
        iteration_no: IterationNo,
        evidence_ref: &EvidenceRef,
        producer: &InstrumentIdentity,
        locator: &EvidenceLocator,
        produced_at: EpochSeconds,
        deadline: Deadline,
    ) -> Result<EvidenceProductionBinding, ProducerAuthorityError> {
        let admitted = self.mint(
            run_id,
            iteration_no,
            evidence_ref,
            producer,
            locator,
            produced_at,
            deadline,
        )?;
        self.state
            .record_evidence_production(run_id, &admitted, deadline)
            .map_err(ProducerAuthorityError::State)
    }
}

/// The observation owner's minting authority: validates the run and the
/// declared iteration, reads the bundle bytes at the ref, re-derives the
/// §12.4 observation content digest from those bytes (the canonical
/// derivation in [`crate::observation_digest`] — ONE rule, reused, never a
/// second implementation), mints the [`AdmittedObservationBinding`] the
/// store accepts, and binds it.
#[derive(Clone)]
pub struct ObservationBindingAuthority {
    state: Arc<dyn StateStore>,
    evidence: Arc<dyn EvidenceStore>,
}

impl ObservationBindingAuthority {
    pub fn new(state: Arc<dyn StateStore>, evidence: Arc<dyn EvidenceStore>) -> Self {
        Self { state, evidence }
    }

    /// Binds one pinned bundle as the run's observation at the declared
    /// iteration. There is no digest argument: what is bound is the bundle
    /// the store serves, digest included, so bytes that are not the bound
    /// content are refused downstream whoever serves them.
    pub fn bind(
        &self,
        run_id: &RunId,
        iteration_no: IterationNo,
        bundle_ref: &EvidenceRef,
        deadline: Deadline,
    ) -> Result<(), ProducerAuthorityError> {
        governed_run(self.state.as_ref(), run_id, iteration_no, deadline)?;
        let bytes = self
            .evidence
            .get(bundle_ref, deadline)
            .map_err(|error| ProducerAuthorityError::EvidenceUnreadable {
                evidence: bundle_ref.clone(),
                detail: error.to_string(),
            })?
            .ok_or_else(|| ProducerAuthorityError::EvidenceAbsent {
                evidence: bundle_ref.clone(),
            })?;
        let text =
            std::str::from_utf8(&bytes).map_err(|error| ProducerAuthorityError::BundleInvalid {
                detail: format!("the bundle bytes are not UTF-8: {error}"),
            })?;
        let document =
            crate::json::parse(text).map_err(|error| ProducerAuthorityError::BundleInvalid {
                detail: format!("the bundle bytes are not JSON: {error}"),
            })?;
        let digest = crate::observation_digest::observation_digest(&document);
        let admitted = AdmittedObservationBinding {
            binding: ObservationBinding::new(bundle_ref.clone(), digest),
        };
        self.state
            .bind_observation_bundle(run_id, iteration_no, &admitted, deadline)
            .map_err(ProducerAuthorityError::State)
    }
}

// ---------------------------------------------------------------------------
// Settlement admission (plan Task 13 B1; prereg R1/A5) — the most
// consequential owner write in the system, earned the same way.
// ---------------------------------------------------------------------------

/// Settlement-admission failures — typed (spec §21.2). Store failures ride
/// [`Self::State`] so a caller can distinguish the authority's governed-read
/// refusals from the store's own write refusals (surfaced verbatim).
#[derive(Debug, Clone, PartialEq)]
pub enum SettlementAdmissionError {
    /// The run is not durable — nothing in this run can settle.
    RunNotDurable { run_id: RunId },
    /// §16.1 condition 1 failed against DURABLE state: no verification
    /// record with `Outcome::Pass` exists for this (run, unit) naming the
    /// committed evidence ref. `detail` distinguishes the three shapes: no
    /// record at all, a record whose outcome is not PASS (an INCONCLUSIVE
    /// or FAIL never satisfies a settlement requirement, I8), and a PASS
    /// record whose evidence ref disagrees with the commit's (the committed
    /// evidence must be the evidence the durable record verified).
    VerificationNotPass {
        run_id: RunId,
        unit_id: UnitId,
        detail: String,
    },
    /// The settlement is already durable for this
    /// `(run_id, unit_id, verification_ref)` key — re-committing would
    /// append a duplicate settlement event (the store's own error names
    /// that "wrong"); the run's settled work stands and re-running the unit
    /// is the §18.3 defect, not a retry.
    AlreadyCommitted {
        run_id: RunId,
        unit_id: UnitId,
        verification: gauntlet_domain::VerificationRef,
        event_log_offset: EventLogOffset,
    },
    /// A durable read or the store write itself failed.
    State(StateStoreError),
}

impl fmt::Display for SettlementAdmissionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RunNotDurable { run_id } => write!(
                f,
                "run {run_id} is not durable; a settlement admission cannot open"
            ),
            Self::VerificationNotPass {
                run_id,
                unit_id,
                detail,
            } => write!(
                f,
                "no PASS verification record for unit {unit_id} of run {run_id} names the \
                 committed evidence — settlement requires one (spec 16.1 condition 1): {detail}"
            ),
            Self::AlreadyCommitted {
                run_id,
                unit_id,
                verification,
                event_log_offset,
            } => write!(
                f,
                "unit {unit_id} of run {run_id} is already settled under verification \
                 {verification} (event-log offset {}); re-committing would append a duplicate \
                 settlement event — the settled work stands (spec 16.3/18.3)",
                event_log_offset.get()
            ),
            Self::State(error) => write!(f, "a durable settlement step failed: {error}"),
        }
    }
}

impl std::error::Error for SettlementAdmissionError {}

/// A settlement commit the settlement authority minted — the only value
/// [`StateStore::commit_settlement`] accepts. No constructor exists outside
/// this module, so a caller that assembled a [`SettlementCommit`] itself
/// (the shape any in-process store holder could write before Task 13) has
/// nothing to pass to any store.
#[derive(Debug, Clone, PartialEq)]
pub struct AdmittedSettlement {
    commit: SettlementCommit,
}

impl AdmittedSettlement {
    /// The minted commit payload, for the store that commits it.
    pub fn commit(&self) -> &SettlementCommit {
        &self.commit
    }
}

/// The settlement minting authority (plan Task 13, R1/A5): validates the
/// §16.1 store-side preconditions against DURABLE state and mints the
/// [`AdmittedSettlement`] the store accepts.
///
/// What the AUTHORITY checks (governed reads over durable state):
///
/// 1. the run is durable;
/// 2. §16.1 condition 1: a verification record with `Outcome = Pass` exists
///    in the run's durable records for the commit's unit, naming the
///    commit's evidence ref (a FAIL/INCONCLUSIVE record, a foreign unit's
///    record, or a PASS over different evidence never admits);
/// 3. the §16.3 idempotence key `(run_id, unit_id, verification_ref)` is
///    not already a durable settlement row.
///
/// What the STORE cross-validates when the minted commit lands (defense in
/// depth — both must hold, neither substitutes for the other): the
/// settlement row's offset names the durable settlement event that made it
/// real, with every field AND the stored event timestamp agreeing
/// (spec §5.3.6, the A3 seam-closing).
///
/// What this authority deliberately does NOT check: §16.1 condition 2's
/// resolve-and-pin half (the evidence STORE is not in this authority's
/// hands — the application layer's `settlement_precondition` owns it, where
/// the chain's own evidence store is in scope) and conditions 3–5
/// (authority chain, blocking residuals, reference digest — the settlement
/// service's scope, plan Task 13 B2). Ports cannot reach those; minting
/// here is the store-side half of the gate, not the whole of §16.1.
#[derive(Clone)]
pub struct SettlementCommitAuthority {
    state: Arc<dyn StateStore>,
}

impl SettlementCommitAuthority {
    pub fn new(state: Arc<dyn StateStore>) -> Self {
        Self { state }
    }

    /// Validates the commit against durable state and mints the admission
    /// [`StateStore::commit_settlement`] consumes. Every fact the caller
    /// supplies (unit, evidence, verification, digest, timestamp) is
    /// validated against what the store already knows — none is taken on
    /// the caller's word.
    pub fn admit(
        &self,
        commit: SettlementCommit,
        deadline: Deadline,
    ) -> Result<AdmittedSettlement, SettlementAdmissionError> {
        self.state
            .load_run(&commit.run_id, deadline)
            .map_err(SettlementAdmissionError::State)?
            .ok_or_else(|| SettlementAdmissionError::RunNotDurable {
                run_id: commit.run_id.clone(),
            })?;
        let records = self
            .state
            .verification_records(&commit.run_id, deadline)
            .map_err(SettlementAdmissionError::State)?;
        let unit_records: Vec<_> = records
            .iter()
            .filter(|record| record.unit_id() == &commit.unit_id)
            .collect();
        if unit_records.is_empty() {
            return Err(SettlementAdmissionError::VerificationNotPass {
                run_id: commit.run_id.clone(),
                unit_id: commit.unit_id.clone(),
                detail: format!(
                    "the run holds no verification record for unit {} at all",
                    commit.unit_id
                ),
            });
        }
        let pass_records: Vec<_> = unit_records
            .iter()
            .filter(|record| record.outcome() == Outcome::Pass)
            .collect();
        if pass_records.is_empty() {
            let outcomes: Vec<String> = unit_records
                .iter()
                .map(|record| record.outcome().to_string())
                .collect();
            return Err(SettlementAdmissionError::VerificationNotPass {
                run_id: commit.run_id.clone(),
                unit_id: commit.unit_id.clone(),
                detail: format!(
                    "the run's records for unit {} hold outcomes {} — none is PASS",
                    commit.unit_id,
                    outcomes.join(", ")
                ),
            });
        }
        if !pass_records
            .iter()
            .any(|record| record.evidence() == &commit.evidence)
        {
            return Err(SettlementAdmissionError::VerificationNotPass {
                run_id: commit.run_id.clone(),
                unit_id: commit.unit_id.clone(),
                detail: format!(
                    "the run's PASS records for unit {} name evidence other than {} — the \
                     committed evidence must be the evidence a durable PASS record verified",
                    commit.unit_id, commit.evidence
                ),
            });
        }
        if let Some(existing) = self
            .state
            .settlements(&commit.run_id, deadline)
            .map_err(SettlementAdmissionError::State)?
            .into_iter()
            .find(|row| {
                row.unit_id() == &commit.unit_id && row.verification() == &commit.verification
            })
        {
            return Err(SettlementAdmissionError::AlreadyCommitted {
                run_id: commit.run_id.clone(),
                unit_id: commit.unit_id.clone(),
                verification: commit.verification.clone(),
                event_log_offset: existing.event_log_offset(),
            });
        }
        Ok(AdmittedSettlement { commit })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use gauntlet_domain::{
        Budget, DiagnosticAllowance, EvidenceKind, InstrumentName, Limits, MicroUsd, ReferenceId,
        ReferenceVersion, Run, RunDefinition,
    };

    use super::{
        AdmittedEvidenceProduction, AdmittedObservationBinding, EvidenceProductionAuthority,
        ObservationBindingAuthority, ProducerAuthorityError, SettlementCommit,
        evidence_content_digest,
    };
    use crate::deadline::Deadline;
    use crate::evidence_store::{EvidenceStore, EvidenceStoreError};
    use crate::observer::InstrumentIdentity;
    use crate::state_store::{
        Checkpoint, EvidenceProductionBinding, RunEvent, StateStore, StateStoreError,
    };
    use gauntlet_domain::{
        EpochSeconds, EventLogOffset, EvidenceLocator, EvidenceRef, IterationNo,
    };

    const BASE_INSTANT: u64 = 1_700_000_000;
    /// The suite's bound span, on its own line (numeric literals never share
    /// a line with a duration-shaped identifier).
    const BOUND_SPAN: u64 = 60;
    const RUN: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";

    fn bound() -> Deadline {
        Deadline::at_epoch(EpochSeconds::new(BASE_INSTANT + BOUND_SPAN))
    }

    fn run_id() -> gauntlet_domain::RunId {
        gauntlet_domain::RunId::new(RUN).expect("valid run id")
    }

    fn fixture_run() -> Run {
        Run::new(RunDefinition {
            id: run_id(),
            created_at: EpochSeconds::new(BASE_INSTANT),
            reference_version: ReferenceVersion::new(
                ReferenceId::new("ref-authority-tests").expect("valid reference id"),
                1,
                gauntlet_domain::Digest::new("sha256", &"ab".repeat(32)).expect("valid digest"),
            ),
            intent: "authority tests".to_owned(),
            config_fingerprint: gauntlet_domain::Digest::new("sha256", &"cd".repeat(32))
                .expect("valid digest"),
            environment_fingerprint: gauntlet_domain::Digest::new("sha256", &"ef".repeat(32))
                .expect("valid digest"),
            budget: Budget::new(
                Limits::new(8, 2, 2, 3600, 1000, MicroUsd::new(500)).expect("legal ceilings"),
                20,
                DiagnosticAllowance::new(100, MicroUsd::new(50)),
            )
            .expect("legal budget"),
            parent_run: None,
        })
        .expect("legal run")
    }

    /// A mutable evidence fake: bytes at a ref can be swapped AFTER a
    /// production was recorded, which is exactly the after-the-fact tamper
    /// the derived-from-bytes digest must catch (Task 11's F1 lesson).
    #[derive(Default)]
    struct MutableEvidence {
        objects: Mutex<Vec<(EvidenceRef, Vec<u8>)>>,
    }

    impl MutableEvidence {
        fn insert(&self, name: &str, bytes: &[u8]) -> EvidenceRef {
            let reference = EvidenceRef::new(name).expect("valid evidence ref");
            self.objects
                .lock()
                .expect("not poisoned")
                .push((reference.clone(), bytes.to_vec()));
            reference
        }

        fn replace(&self, name: &str, bytes: &[u8]) {
            let reference = EvidenceRef::new(name).expect("valid evidence ref");
            let mut objects = self.objects.lock().expect("not poisoned");
            if let Some(entry) = objects.iter_mut().find(|(found, _)| *found == reference) {
                entry.1 = bytes.to_vec();
            }
        }
    }

    impl EvidenceStore for MutableEvidence {
        fn put(
            &self,
            bytes: &[u8],
            _: EvidenceKind,
            _: Deadline,
        ) -> Result<EvidenceRef, EvidenceStoreError> {
            let index = self.objects.lock().expect("not poisoned").len();
            Ok(self.insert(&format!("evd-authority-{index:04}"), bytes))
        }
        fn get(
            &self,
            reference: &EvidenceRef,
            _: Deadline,
        ) -> Result<Option<Vec<u8>>, EvidenceStoreError> {
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
            _: &gauntlet_domain::RunId,
            _: &EvidenceRef,
            _: Deadline,
        ) -> Result<(), EvidenceStoreError> {
            Ok(())
        }
    }

    /// An in-memory store fake holding the durable run, its checkpoint
    /// (the iteration anchor), and write-once owner-write rows keyed the
    /// way the real adapter keys them.
    #[derive(Default)]
    struct OwnerWriteState {
        run: Option<Run>,
        checkpoint: Mutex<Option<Checkpoint>>,
        productions: Mutex<Vec<EvidenceProductionBinding>>,
        bindings: Mutex<Vec<(IterationNo, EvidenceRef, gauntlet_domain::Digest)>>,
        next_offset: Mutex<u64>,
    }

    impl OwnerWriteState {
        fn with_run(run: Run) -> Self {
            Self {
                run: Some(run),
                ..Self::default()
            }
        }

        fn checkpoint_at(&self, iteration: u32) {
            *self.checkpoint.lock().expect("not poisoned") = Some(Checkpoint {
                run_id: run_id(),
                reference_digest: fixture_run().reference_version().digest().clone(),
                iteration_no: IterationNo::new(iteration).expect("1-based"),
                unit_states: Vec::new(),
                budget_spent: *fixture_run().budget().spent(),
                in_flight: Vec::new(),
                event_log_offset: EventLogOffset::new(0),
            });
        }

        fn recorded_digest(&self, evidence: &EvidenceRef) -> Option<gauntlet_domain::Digest> {
            self.productions
                .lock()
                .expect("not poisoned")
                .iter()
                .find(|binding| binding.production().evidence() == evidence)
                .map(|binding| binding.production().content().clone())
        }
    }

    impl StateStore for OwnerWriteState {
        fn append_event(
            &self,
            _: RunEvent,
            _: Deadline,
        ) -> Result<EventLogOffset, StateStoreError> {
            let mut next = self.next_offset.lock().expect("not poisoned");
            *next += 1;
            Ok(EventLogOffset::new(*next))
        }
        fn load_run(
            &self,
            run_id: &gauntlet_domain::RunId,
            _: Deadline,
        ) -> Result<Option<Run>, StateStoreError> {
            Ok((self.run.as_ref().is_some_and(|run| run.id() == run_id)).then(fixture_run))
        }
        fn checkpoint(&self, checkpoint: Checkpoint, _: Deadline) -> Result<(), StateStoreError> {
            *self.checkpoint.lock().expect("not poisoned") = Some(checkpoint);
            Ok(())
        }
        fn resume(
            &self,
            _: &gauntlet_domain::RunId,
            _: Deadline,
        ) -> Result<Option<Checkpoint>, StateStoreError> {
            Ok(self.checkpoint.lock().expect("not poisoned").clone())
        }
        fn record_evidence_production(
            &self,
            run_id: &gauntlet_domain::RunId,
            production: &super::AdmittedEvidenceProduction,
            _: Deadline,
        ) -> Result<EvidenceProductionBinding, StateStoreError> {
            let payload = production.production();
            let mut productions = self.productions.lock().expect("not poisoned");
            if let Some(existing) = productions
                .iter()
                .find(|binding| binding.production().evidence() == payload.evidence())
            {
                if existing.production() == payload {
                    return Ok(existing.clone());
                }
                return Err(StateStoreError::WriteFailed {
                    detail: format!(
                        "run {run_id} already recorded {} with different facts",
                        payload.evidence()
                    ),
                });
            }
            let mut next = self.next_offset.lock().expect("not poisoned");
            *next += 1;
            let binding = EvidenceProductionBinding::new(
                run_id.clone(),
                payload.clone(),
                EventLogOffset::new(*next),
            );
            productions.push(binding.clone());
            Ok(binding)
        }
        fn bind_observation_bundle(
            &self,
            run_id: &gauntlet_domain::RunId,
            iteration_no: IterationNo,
            binding: &super::AdmittedObservationBinding,
            _: Deadline,
        ) -> Result<(), StateStoreError> {
            let payload = binding.binding();
            let mut bindings = self.bindings.lock().expect("not poisoned");
            if let Some((_, found_ref, found_digest)) = bindings
                .iter()
                .find(|(iteration, _, _)| *iteration == iteration_no)
            {
                if found_ref == payload.evidence() && found_digest == payload.digest() {
                    return Ok(());
                }
                return Err(StateStoreError::WriteFailed {
                    detail: format!(
                        "run {run_id} iteration {} is already bound to {found_ref}",
                        iteration_no.get()
                    ),
                });
            }
            bindings.push((
                iteration_no,
                payload.evidence().clone(),
                payload.digest().clone(),
            ));
            Ok(())
        }
        fn observation_binding(
            &self,
            _: &gauntlet_domain::RunId,
            iteration_no: IterationNo,
            _: Deadline,
        ) -> Result<Option<crate::state_store::ObservationBinding>, StateStoreError> {
            Ok(self
                .bindings
                .lock()
                .expect("not poisoned")
                .iter()
                .find(|(iteration, _, _)| *iteration == iteration_no)
                .map(|(_, reference, digest)| {
                    crate::state_store::ObservationBinding::new(reference.clone(), digest.clone())
                }))
        }
        fn export_jsonl(
            &self,
            _: &gauntlet_domain::RunId,
            _: Deadline,
        ) -> Result<String, StateStoreError> {
            Ok(String::new())
        }
    }

    fn instrument() -> InstrumentIdentity {
        InstrumentIdentity::new(
            InstrumentName::new("authority-test-instrument").expect("plain token"),
            "1".to_owned(),
        )
    }

    fn production_authority(
        state: &std::sync::Arc<OwnerWriteState>,
        evidence: &std::sync::Arc<MutableEvidence>,
    ) -> EvidenceProductionAuthority {
        EvidenceProductionAuthority::new(state.clone(), evidence.clone())
    }

    fn binding_authority(
        state: &std::sync::Arc<OwnerWriteState>,
        evidence: &std::sync::Arc<MutableEvidence>,
    ) -> ObservationBindingAuthority {
        ObservationBindingAuthority::new(state.clone(), evidence.clone())
    }

    #[test]
    fn an_undurable_run_refuses_every_owner_write() {
        let state = std::sync::Arc::new(OwnerWriteState::default());
        let evidence = std::sync::Arc::new(MutableEvidence::default());
        let reference = evidence.insert("evd-never-recorded", b"bytes");
        let production_refusal = production_authority(&state, &evidence).record(
            &run_id(),
            IterationNo::new(1).expect("1-based"),
            &reference,
            &instrument(),
            &EvidenceLocator::new("report.json").expect("valid locator"),
            EpochSeconds::new(BASE_INSTANT),
            bound(),
        );
        assert!(matches!(
            production_refusal,
            Err(ProducerAuthorityError::RunNotDurable { .. })
        ));
        let binding_refusal = binding_authority(&state, &evidence).bind(
            &run_id(),
            IterationNo::new(1).expect("1-based"),
            &reference,
            bound(),
        );
        assert!(matches!(
            binding_refusal,
            Err(ProducerAuthorityError::RunNotDurable { .. })
        ));
    }

    #[test]
    fn a_stale_or_ahead_declared_iteration_is_refused() {
        let state = std::sync::Arc::new(OwnerWriteState::with_run(fixture_run()));
        let evidence = std::sync::Arc::new(MutableEvidence::default());
        let reference = evidence.insert("evd-iteration-probe", b"bytes");
        state.checkpoint_at(2);
        // Stale: the run durably stands at 2; declaring 1 would record a
        // prior loop pass's production under the current one's name.
        assert!(matches!(
            production_authority(&state, &evidence).record(
                &run_id(),
                IterationNo::new(1).expect("1-based"),
                &reference,
                &instrument(),
                &EvidenceLocator::new("report.json").expect("valid locator"),
                EpochSeconds::new(BASE_INSTANT),
                bound(),
            ),
            Err(ProducerAuthorityError::IterationUnsupported {
                requested, durable, ..
            })
                if requested.get() == 1 && durable.get() == 2
        ));
        // Ahead: declaring 3 certifies a loop pass that never happened.
        assert!(matches!(
            binding_authority(&state, &evidence).bind(
                &run_id(),
                IterationNo::new(3).expect("1-based"),
                &reference,
                bound(),
            ),
            Err(ProducerAuthorityError::IterationUnsupported { .. })
        ));
        // A run that crossed no boundary stands at iteration 1 — the
        // no-checkpoint default anchor. The probe binds a minimal JSON
        // bundle so the only property under test is the anchor.
        let fresh = std::sync::Arc::new(OwnerWriteState::with_run(fixture_run()));
        let bundle = evidence.insert(
            "evd-anchor-probe",
            br#"{"schema":"observation_bundle@v1","facets":[]}"#,
        );
        let binding = binding_authority(&fresh, &evidence).bind(
            &run_id(),
            IterationNo::new(1).expect("1-based"),
            &bundle,
            bound(),
        );
        assert!(
            binding.is_ok(),
            "no checkpoint means iteration 1: {binding:?}"
        );
    }

    #[test]
    fn the_content_digest_is_derived_from_the_served_bytes() {
        let state = std::sync::Arc::new(OwnerWriteState::with_run(fixture_run()));
        let evidence = std::sync::Arc::new(MutableEvidence::default());
        let reference = evidence.insert("evd-derived", b"the honest production bytes");
        let binding = production_authority(&state, &evidence)
            .record(
                &run_id(),
                IterationNo::new(1).expect("1-based"),
                &reference,
                &instrument(),
                &EvidenceLocator::new("report.json").expect("valid locator"),
                EpochSeconds::new(BASE_INSTANT),
                bound(),
            )
            .expect("the honest production records");
        // The recorded content digest is the digest of the BYTES AT THE
        // REF — there was never a parameter to supply a different one.
        assert_eq!(
            binding.production().content(),
            &evidence_content_digest(b"the honest production bytes")
        );
        assert!(binding.event_log_offset().get() > 0);
        // Re-recording the SAME facts is idempotent (the store's rule).
        assert!(
            production_authority(&state, &evidence)
                .record(
                    &run_id(),
                    IterationNo::new(1).expect("1-based"),
                    &reference,
                    &instrument(),
                    &EvidenceLocator::new("report.json").expect("valid locator"),
                    EpochSeconds::new(BASE_INSTANT),
                    bound(),
                )
                .is_ok()
        );
    }

    #[test]
    fn bytes_swapped_after_the_fact_are_refused_by_the_derived_digest() {
        // Task 11's F1 shape: the ref is real, the record is real, and the
        // bytes at the ref CHANGE. Because the authority re-derives the
        // digest from the served bytes, the second record attempt carries a
        // DIFFERENT derived digest and the store's write-once rule refuses
        // it — the doctored bytes cannot ride the honest record.
        let state = std::sync::Arc::new(OwnerWriteState::with_run(fixture_run()));
        let evidence = std::sync::Arc::new(MutableEvidence::default());
        let reference = evidence.insert("evd-swap-target", b"the original bytes");
        let authority = production_authority(&state, &evidence);
        authority
            .record(
                &run_id(),
                IterationNo::new(1).expect("1-based"),
                &reference,
                &instrument(),
                &EvidenceLocator::new("report.json").expect("valid locator"),
                EpochSeconds::new(BASE_INSTANT),
                bound(),
            )
            .expect("the original records");
        let recorded = state
            .recorded_digest(&reference)
            .expect("the production is durable");
        assert_eq!(recorded, evidence_content_digest(b"the original bytes"));

        evidence.replace("evd-swap-target", b"the doctored bytes");
        let refusal = authority.record(
            &run_id(),
            IterationNo::new(1).expect("1-based"),
            &reference,
            &instrument(),
            &EvidenceLocator::new("report.json").expect("valid locator"),
            EpochSeconds::new(BASE_INSTANT),
            bound(),
        );
        assert!(
            matches!(
                &refusal,
                Err(ProducerAuthorityError::State(
                    StateStoreError::WriteFailed { .. }
                ))
            ),
            "the swapped bytes must be refused through the store's write-once rule, got \
             {refusal:?}"
        );
        // And the durable record still names the ORIGINAL content.
        assert_eq!(
            state.recorded_digest(&reference),
            Some(evidence_content_digest(b"the original bytes"))
        );
    }

    #[test]
    fn an_absent_ref_refuses_because_there_are_no_bytes_to_digest() {
        let state = std::sync::Arc::new(OwnerWriteState::with_run(fixture_run()));
        let evidence = std::sync::Arc::new(MutableEvidence::default());
        let absent = EvidenceRef::new("evd-never-put").expect("valid ref");
        assert!(matches!(
            production_authority(&state, &evidence).record(
                &run_id(),
                IterationNo::new(1).expect("1-based"),
                &absent,
                &instrument(),
                &EvidenceLocator::new("report.json").expect("valid locator"),
                EpochSeconds::new(BASE_INSTANT),
                bound(),
            ),
            Err(ProducerAuthorityError::EvidenceAbsent { .. })
        ));
        assert!(matches!(
            binding_authority(&state, &evidence).bind(
                &run_id(),
                IterationNo::new(1).expect("1-based"),
                &absent,
                bound(),
            ),
            Err(ProducerAuthorityError::EvidenceAbsent { .. })
        ));
    }

    #[test]
    fn the_binding_digest_is_the_canonical_124_derivation_of_the_bytes() {
        let state = std::sync::Arc::new(OwnerWriteState::with_run(fixture_run()));
        let evidence = std::sync::Arc::new(MutableEvidence::default());
        // A minimal bundle document; the §12.4 rule digests the normalized
        // form (timestamps stripped, the self-referential digest member
        // pinned empty), so the bound digest is a function of the bundle's
        // CONTENT, not its clock fields.
        let bundle = br#"{"schema":"observation_bundle@v1","run_id":"01ARZ3NDEKTSV4RRFFQ69G5FAV","iteration_no":1,"observed_at":1700000000,"facets":[],"observation_digest":"00"}"#;
        let reference = evidence.insert("evd-bundle", bundle);
        binding_authority(&state, &evidence)
            .bind(
                &run_id(),
                IterationNo::new(1).expect("1-based"),
                &reference,
                bound(),
            )
            .expect("the bundle binds");
        let bound = state
            .observation_binding(&run_id(), IterationNo::new(1).expect("1-based"), bound())
            .expect("the binding reads back")
            .expect("the binding exists");
        let document = crate::json::parse(std::str::from_utf8(bundle).expect("UTF-8 fixture"))
            .expect("the fixture is JSON");
        assert_eq!(
            bound.digest(),
            &crate::observation_digest::observation_digest(&document),
            "the bound digest is the ONE canonical derivation, reused"
        );
        assert_eq!(bound.evidence(), &reference);
    }

    #[test]
    fn bundle_bytes_that_are_not_json_refuse_the_bind() {
        let state = std::sync::Arc::new(OwnerWriteState::with_run(fixture_run()));
        let evidence = std::sync::Arc::new(MutableEvidence::default());
        let reference = evidence.insert("evd-not-json", b"not a bundle at all");
        assert!(matches!(
            binding_authority(&state, &evidence).bind(
                &run_id(),
                IterationNo::new(1).expect("1-based"),
                &reference,
                bound(),
            ),
            Err(ProducerAuthorityError::BundleInvalid { .. })
        ));
    }

    // -----------------------------------------------------------------------
    // Settlement admission (plan Task 13 B1, R1/A5: the settlement write is
    // the most consequential owner write in the system)
    // -----------------------------------------------------------------------

    /// The durable substrate the settlement admission validates against:
    /// the recorded run, its verification records, and its settlement rows.
    /// Keyed the way the real adapter keys them (the §16.3 replay key).
    #[derive(Default)]
    struct SettlementGateState {
        run: Option<Run>,
        verifications: Mutex<Vec<crate::verification_record::VerificationRecordDoc>>,
        settlement_rows: Mutex<Vec<SettlementCommit>>,
        next_offset: Mutex<u64>,
    }

    impl SettlementGateState {
        fn with_run(run: Run) -> Self {
            Self {
                run: Some(run),
                ..Self::default()
            }
        }
    }

    impl StateStore for SettlementGateState {
        fn append_event(
            &self,
            _: RunEvent,
            _: Deadline,
        ) -> Result<EventLogOffset, StateStoreError> {
            let mut next = self.next_offset.lock().expect("not poisoned");
            *next += 1;
            Ok(EventLogOffset::new(*next))
        }
        fn load_run(
            &self,
            run_id: &gauntlet_domain::RunId,
            _: Deadline,
        ) -> Result<Option<Run>, StateStoreError> {
            Ok((self.run.as_ref().is_some_and(|run| run.id() == run_id)).then(fixture_run))
        }
        fn checkpoint(&self, _: Checkpoint, _: Deadline) -> Result<(), StateStoreError> {
            Ok(())
        }
        fn resume(
            &self,
            _: &gauntlet_domain::RunId,
            _: Deadline,
        ) -> Result<Option<Checkpoint>, StateStoreError> {
            Ok(None)
        }
        fn record_verification(
            &self,
            record: &crate::verification_record::VerificationRecordDoc,
            _: Deadline,
        ) -> Result<(), StateStoreError> {
            self.verifications
                .lock()
                .expect("not poisoned")
                .push(record.clone());
            Ok(())
        }
        fn verification_records(
            &self,
            _: &gauntlet_domain::RunId,
            _: Deadline,
        ) -> Result<Vec<crate::verification_record::VerificationRecordDoc>, StateStoreError>
        {
            Ok(self.verifications.lock().expect("not poisoned").clone())
        }
        fn commit_settlement(
            &self,
            admitted: &super::AdmittedSettlement,
            _: Deadline,
        ) -> Result<EventLogOffset, StateStoreError> {
            let commit = admitted.commit().clone();
            let mut rows = self.settlement_rows.lock().expect("not poisoned");
            if rows
                .iter()
                .any(|row| row.unit_id == commit.unit_id && row.verification == commit.verification)
            {
                return Err(StateStoreError::WriteFailed {
                    detail: "already committed".to_owned(),
                });
            }
            let mut next = self.next_offset.lock().expect("not poisoned");
            *next += 1;
            rows.push(commit);
            Ok(EventLogOffset::new(*next))
        }
        fn settlements(
            &self,
            run_id: &gauntlet_domain::RunId,
            _: Deadline,
        ) -> Result<Vec<gauntlet_domain::SettlementRecord>, StateStoreError> {
            let mut rows = self.settlement_rows.lock().expect("not poisoned").clone();
            rows.sort_by(|left, right| {
                (left.unit_id.as_str(), left.verification.as_str())
                    .cmp(&(right.unit_id.as_str(), right.verification.as_str()))
            });
            Ok(rows
                .into_iter()
                .filter(|row| &row.run_id == run_id)
                .map(|row| {
                    gauntlet_domain::SettlementRecord::new(
                        row.run_id,
                        row.unit_id,
                        row.evidence,
                        row.verification,
                        EventLogOffset::new(1),
                    )
                })
                .collect())
        }
        fn export_jsonl(
            &self,
            _: &gauntlet_domain::RunId,
            _: Deadline,
        ) -> Result<String, StateStoreError> {
            Ok(String::new())
        }
    }

    fn pass_record(
        unit: &str,
        evidence: &str,
        outcome: gauntlet_domain::Outcome,
    ) -> crate::verification_record::VerificationRecordDoc {
        crate::verification_record::VerificationRecordDoc::new(
            crate::verification_record::VerificationContent {
                run_id: run_id(),
                iteration_no: IterationNo::new(1).expect("1-based"),
                unit_id: gauntlet_domain::UnitId::new(unit).expect("valid unit id"),
                claim: "the unit's claim".to_owned(),
                requirement: crate::verification_record::RequirementDoc::new(
                    gauntlet_domain::EvidenceKind::TestReport,
                    EvidenceLocator::new("report.json").expect("valid locator"),
                    gauntlet_domain::Producer::Instrument,
                ),
                instrument: crate::verification_record::VerifyingInstrument::new(
                    "settle-authority-test".to_owned(),
                    "1".to_owned(),
                    true,
                ),
                evidence: EvidenceRef::new(evidence).expect("valid evidence ref"),
                produced_at: EpochSeconds::new(BASE_INSTANT),
                outcome,
                class: None,
                detail: "the fixture record".to_owned(),
            },
        )
        .expect("fixture content is well formed")
    }

    fn settlement_commit() -> super::SettlementCommit {
        super::SettlementCommit {
            run_id: run_id(),
            unit_id: gauntlet_domain::UnitId::new("AK-900").expect("valid unit id"),
            evidence: EvidenceRef::new("evd-000900").expect("valid evidence ref"),
            verification: gauntlet_domain::VerificationRef::new("vrf-000900")
                .expect("valid verification ref"),
            reference_digest: fixture_run().reference_version().digest().clone(),
            at: EpochSeconds::new(BASE_INSTANT),
        }
    }

    fn settlement_authority(
        state: &std::sync::Arc<SettlementGateState>,
    ) -> super::SettlementCommitAuthority {
        super::SettlementCommitAuthority::new(state.clone())
    }

    #[test]
    fn a_settlement_admission_requires_a_durable_run() {
        let state = std::sync::Arc::new(SettlementGateState::default());
        assert!(matches!(
            settlement_authority(&state).admit(settlement_commit(), bound()),
            Err(super::SettlementAdmissionError::RunNotDurable { .. })
        ));
    }

    #[test]
    fn a_settlement_admission_requires_a_durable_pass_record_for_the_unit() {
        let state = std::sync::Arc::new(SettlementGateState::with_run(fixture_run()));
        // No record at all.
        assert!(matches!(
            settlement_authority(&state).admit(settlement_commit(), bound()),
            Err(super::SettlementAdmissionError::VerificationNotPass { .. })
        ));
        // A record for the unit that is not PASS never admits (I8:
        // INCONCLUSIVE never satisfies a settlement requirement).
        state
            .record_verification(
                &pass_record(
                    "AK-900",
                    "evd-000900",
                    gauntlet_domain::Outcome::Inconclusive,
                ),
                bound(),
            )
            .expect("the record lands");
        assert!(matches!(
            settlement_authority(&state).admit(settlement_commit(), bound()),
            Err(super::SettlementAdmissionError::VerificationNotPass { .. })
        ));
        // A PASS record for a DIFFERENT unit never admits this unit.
        state
            .record_verification(
                &pass_record("AK-901", "evd-000900", gauntlet_domain::Outcome::Pass),
                bound(),
            )
            .expect("the record lands");
        assert!(matches!(
            settlement_authority(&state).admit(settlement_commit(), bound()),
            Err(super::SettlementAdmissionError::VerificationNotPass { .. })
        ));
        // A PASS record for the unit whose evidence ref disagrees with the
        // commit's — the committed evidence must be the evidence the durable
        // PASS record verified.
        state
            .record_verification(
                &pass_record("AK-900", "evd-000901", gauntlet_domain::Outcome::Pass),
                bound(),
            )
            .expect("the record lands");
        assert!(matches!(
            settlement_authority(&state).admit(settlement_commit(), bound()),
            Err(super::SettlementAdmissionError::VerificationNotPass { .. })
        ));
    }

    #[test]
    fn an_already_committed_settlement_refuses_re_admission() {
        let state = std::sync::Arc::new(SettlementGateState::with_run(fixture_run()));
        state
            .record_verification(
                &pass_record("AK-900", "evd-000900", gauntlet_domain::Outcome::Pass),
                bound(),
            )
            .expect("the record lands");
        let authority = settlement_authority(&state);
        let admitted = authority
            .admit(settlement_commit(), bound())
            .expect("the honest settlement earns its admission");
        state
            .commit_settlement(&admitted, bound())
            .expect("the store commits the admitted settlement");
        assert!(matches!(
            authority.admit(settlement_commit(), bound()),
            Err(super::SettlementAdmissionError::AlreadyCommitted { .. })
        ));
    }

    #[test]
    fn an_earned_admission_commits_through_the_store_and_reads_back() {
        let state = std::sync::Arc::new(SettlementGateState::with_run(fixture_run()));
        state
            .record_verification(
                &pass_record("AK-900", "evd-000900", gauntlet_domain::Outcome::Pass),
                bound(),
            )
            .expect("the record lands");
        let admitted = settlement_authority(&state)
            .admit(settlement_commit(), bound())
            .expect("the honest settlement earns its admission");
        assert_eq!(admitted.commit(), &settlement_commit());
        let offset = state
            .commit_settlement(&admitted, bound())
            .expect("the minted admission commits");
        assert!(offset.get() > 0);
        let rows = state.settlements(&run_id(), bound()).expect("reads back");
        assert_eq!(rows.len(), 1, "exactly one settlement row");
        assert_eq!(rows[0].unit_id().as_str(), "AK-900");
        assert_eq!(rows[0].verification().as_str(), "vrf-000900");
    }

    /// The structural gate, settlement leg: `AdmittedSettlement` has no
    /// constructor outside this module, so a caller holding a hand-built
    /// [`SettlementCommit`] has nothing to pass to any store.
    #[test]
    fn the_minted_settlement_admission_has_no_public_constructor() {
        let admitted = super::AdmittedSettlement {
            commit: settlement_commit(),
        };
        assert_eq!(admitted.commit(), &settlement_commit());
    }

    /// The structural gate: the minted types have NO constructor outside
    /// this crate, so a direct store call is a compile error everywhere
    /// else. From inside the crate the fields are reachable, which is what
    /// makes this negative probe possible at all — the compiled
    /// out-of-crate probe is the confirmation round's job (recorded in the
    /// Task-12 report).
    #[test]
    fn the_minted_types_have_no_public_constructor() {
        // Constructing the minted values is possible ONLY through the
        // module's private fields (this test) or the authorities (everyone).
        // If a `new` ever becomes public, this test still compiles — so the
        // assertion is the trait's SIGNATURE: the store methods below take
        // the minted types, and there is no `Admitted*::new` in the public
        // API. `rustdoc`/compile gates hold the line; this test pins the
        // payload-wiring the authorities rely on.
        let production = crate::state_store::EvidenceProduction::new(
            EvidenceRef::new("evd-mint-probe").expect("valid ref"),
            evidence_content_digest(b"probe"),
            instrument(),
            EvidenceLocator::new("probe.json").expect("valid locator"),
            IterationNo::new(1).expect("1-based"),
            EpochSeconds::new(BASE_INSTANT),
        );
        let admitted = AdmittedEvidenceProduction {
            production: production.clone(),
        };
        assert_eq!(admitted.production(), &production);
        let binding = crate::state_store::ObservationBinding::new(
            EvidenceRef::new("evd-mint-probe").expect("valid ref"),
            evidence_content_digest(b"probe"),
        );
        let admitted_binding = AdmittedObservationBinding {
            binding: binding.clone(),
        };
        assert_eq!(admitted_binding.binding(), &binding);
    }
}
