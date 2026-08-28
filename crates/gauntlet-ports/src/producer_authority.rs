//! The owner-write minting authorities (plan Task 12; the `AdmittedComparison`
//! precedent from plan Task 10 / Task 11 C7).
//!
//! Four `StateStore` writes are OWNER writes, not caller writes:
//! `record_evidence_production` (the evidence owner's durable production
//! fact, spec §14.3), `bind_observation_bundle` (the observation owner's
//! binding of one pinned bundle as *this* run's observation at *this*
//! iteration, spec §5.3.6/§18.1), `commit_settlement` (the §16.3 settlement
//! commit, the most consequential write in the system), and — since the
//! Task-13 correction round — `record_verification` (the §5.3.6/§16.1
//! durable PASS substrate the settlement admission itself validates
//! against: before the correction any holder of an `Arc<dyn StateStore>`
//! could build a PASS document with the public `VerificationRecordDoc::new`
//! and write it raw, forging the very fact §16.1 condition 1 reads). Before
//! Task 12 any holder of an `Arc<dyn StateStore>` could perform the first
//! two for any store-resolvable bytes, and the declared iteration was never
//! validated; before Task 13 the settlement commit was not on the port at
//! all, and the adapter's inherent surface would take any caller-attested
//! payload.
//!
//! The closure follows the established pattern exactly: the trait methods
//! now accept only values minted HERE — [`AdmittedEvidenceProduction`],
//! [`AdmittedObservationBinding`], [`AdmittedSettlement`], and
//! [`AdmittedVerificationRecord`] have no constructor outside this module —
//! and the authorities perform the governed reads before minting:
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
// Verification-record admission (Task 13 correction round 1, COR-1; early
// probe C1) — the durable §16.1-condition-1 substrate is owner-written.
// ---------------------------------------------------------------------------

/// A verification record the recording authority minted — the only value
/// [`StateStore::record_verification`] accepts. No constructor exists
/// outside this module, so a caller that assembled a
/// [`VerificationRecordDoc`](crate::verification_record::VerificationRecordDoc)
/// itself (the shape any in-process store holder could write before the
/// correction) has nothing to pass to any store.
///
/// The mint validates the document's OWN grounding facts — the run it names
/// is durable and the iteration it declares is the run's durable anchor —
/// because this row is what the settlement admission later reads as "a PASS
/// verification record exists in this run": a record a caller minted over
/// an undurable run or a stale/ahead iteration would forge that fact. What
/// the mint deliberately does NOT re-derive is the verification itself: the
/// document is the governed verification chain's output (the application
/// layer's `VerifiedClaim` consumer), and the chain's own §14 binding
/// checks are what make the record's facts earned, not this authority's.
#[derive(Debug, Clone, PartialEq)]
pub struct AdmittedVerificationRecord {
    record: crate::verification_record::VerificationRecordDoc,
}

impl AdmittedVerificationRecord {
    /// The minted document, for the store that records it.
    pub fn record(&self) -> &crate::verification_record::VerificationRecordDoc {
        &self.record
    }
}

/// The verification-record owner's minting authority (Task 13 correction
/// round 1, COR-1): performs the governed reads and mints the
/// [`AdmittedVerificationRecord`] the store accepts. The caller is the
/// verified chain itself — the application layer's `VerifiedClaim` consumer
/// path — never an arbitrary store holder.
#[derive(Clone)]
pub struct VerificationRecordAuthority {
    state: Arc<dyn StateStore>,
}

impl VerificationRecordAuthority {
    pub fn new(state: Arc<dyn StateStore>) -> Self {
        Self { state }
    }

    /// Validates the document's grounding against durable state — the run
    /// it names is durable, and the iteration it declares is the run's
    /// durable anchor (the same rule every owner write validates; a stale
    /// iteration would certify a prior loop pass, an ahead one a pass that
    /// never happened) — then mints and durably records the document.
    ///
    /// The store's own write remains append-only per document; this
    /// authority adds the earning, it does not relax the store's rules.
    pub fn record(
        &self,
        record: &crate::verification_record::VerificationRecordDoc,
        deadline: Deadline,
    ) -> Result<(), ProducerAuthorityError> {
        governed_run(
            self.state.as_ref(),
            record.run_id(),
            record.iteration_no(),
            deadline,
        )?;
        let admitted = AdmittedVerificationRecord {
            record: record.clone(),
        };
        self.state
            .record_verification(&admitted, deadline)
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
    /// The §16.3 key is already durable, and the STORED settlement row
    /// disagrees with the offered commit on a field outside the key (the
    /// evidence ref — the key fixes run, unit, and verification). The same
    /// key has already settled DIFFERENT evidence: idempotent re-entry is
    /// for the SAME settlement, so this is an integrity refusal, never a
    /// success riding the existing offset (Task 13 correction round 1,
    /// COR-4; early probe M2). Boxed: the ids plus both evidence refs
    /// would otherwise inflate every `Result<_, SettlementAdmissionError>`
    /// return past the lint's large-`Err` bar (the `ReferenceMutated`
    /// precedent).
    AlreadyCommittedDivergence(Box<AlreadyCommittedDivergenceRefusal>),
    /// One settlement per (run, unit) — the unit already holds a durable
    /// settlement row under a DIFFERENT §16.3 key (a different verification
    /// ref). A fresh honest PASS mints a fresh key, and before COR2-1(a)
    /// (critic round 1, F-1) that fresh key admitted: the second commit
    /// appended a second settlement event and row for work that had already
    /// settled — AK-042's "no double commit" guarantee broken at substance
    /// level while every §16.1-condition-1 check held. The same-key shape
    /// keeps its own idempotent refusal ([`Self::AlreadyCommitted`]) and
    /// integrity refusal ([`Self::AlreadyCommittedDivergence`]). Boxed per
    /// the `AlreadyCommittedDivergence` large-`Err` precedent.
    UnitAlreadySettled(Box<UnitAlreadySettledRefusal>),
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
            Self::AlreadyCommittedDivergence(refusal) => {
                let AlreadyCommittedDivergenceRefusal {
                    run_id,
                    unit_id,
                    verification,
                    committed_evidence,
                    offered_evidence,
                    event_log_offset,
                } = refusal.as_ref();
                write!(
                    f,
                    "unit {unit_id} of run {run_id} is already settled under verification \
                     {verification} over evidence {committed_evidence} (event-log offset {}), but \
                     this commit offers evidence {offered_evidence} — the same key cannot settle \
                     different evidence; the stored row stands (spec 16.3; COR-4)",
                    event_log_offset.get()
                )
            }
            Self::UnitAlreadySettled(refusal) => {
                let UnitAlreadySettledRefusal {
                    run_id,
                    unit_id,
                    settled_verification,
                    offered_verification,
                    event_log_offset,
                } = refusal.as_ref();
                write!(
                    f,
                    "unit {unit_id} of run {run_id} already holds a settlement under verification \
                     {settled_verification} (event-log offset {}); this commit offers a different \
                     key ({offered_verification}) — one settlement per unit per run, however \
                     honest the fresh evidence behind the new key (spec 16.3; COR2-1)",
                    event_log_offset.get()
                )
            }
            Self::State(error) => write!(f, "a durable settlement step failed: {error}"),
        }
    }
}

impl std::error::Error for SettlementAdmissionError {}

/// The boxed payload of
/// [`SettlementAdmissionError::AlreadyCommittedDivergence`]: the key plus
/// the stored row's evidence, the offered commit's evidence, and the
/// stored row's own event-log offset.
#[derive(Debug, Clone, PartialEq)]
pub struct AlreadyCommittedDivergenceRefusal {
    pub run_id: RunId,
    pub unit_id: UnitId,
    pub verification: gauntlet_domain::VerificationRef,
    pub committed_evidence: EvidenceRef,
    pub offered_evidence: EvidenceRef,
    pub event_log_offset: EventLogOffset,
}

/// The boxed payload of
/// [`SettlementAdmissionError::UnitAlreadySettled`]: the key's ids, the
/// STORED settlement's verification ref, the offered commit's ref, and the
/// stored row's own event-log offset.
#[derive(Debug, Clone, PartialEq)]
pub struct UnitAlreadySettledRefusal {
    pub run_id: RunId,
    pub unit_id: UnitId,
    pub settled_verification: gauntlet_domain::VerificationRef,
    pub offered_verification: gauntlet_domain::VerificationRef,
    pub event_log_offset: EventLogOffset,
}

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
            // COR-5 (early probe M5): admission does not trust the store's
            // run scoping — the record's OWN run must be the commit's run.
            // A conforming-but-sloppy store that feeds foreign-run records
            // to this filter finds nothing here, never a PASS.
            .filter(|record| {
                record.unit_id() == &commit.unit_id && record.run_id() == &commit.run_id
            })
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
        // COR-3 (early probe M1): the §16.3 key's third component is
        // durable-moored — the durable PASS record's verification ref must
        // EQUAL the commit's. Before the correction the record carried no
        // verification ref, so one honest PASS admitted N distinct-key
        // commits under N caller-invented refs, each appending its own
        // settlement event.
        if !pass_records
            .iter()
            .any(|record| record.verification() == &commit.verification)
        {
            let refs: Vec<&str> = pass_records
                .iter()
                .map(|record| record.verification().as_str())
                .collect();
            return Err(SettlementAdmissionError::VerificationNotPass {
                run_id: commit.run_id.clone(),
                unit_id: commit.unit_id.clone(),
                detail: format!(
                    "the run's PASS records for unit {} name verification refs [{}] — none is \
                     the commit's {} — the 16.3 key's verification component is moored in the \
                     durable record, never invented",
                    commit.unit_id,
                    refs.join(", "),
                    commit.verification
                ),
            });
        }
        let existing_rows = self
            .state
            .settlements(&commit.run_id, deadline)
            .map_err(SettlementAdmissionError::State)?;
        if let Some(existing) = existing_rows.iter().find(|row| {
            row.unit_id() == &commit.unit_id && row.verification() == &commit.verification
        }) {
            // COR-4 (early probe M2): the STORED row is what it is — the
            // key hit must AGREE on the row's remaining field (the evidence
            // ref) for re-entry to be idempotent. A divergent evidence ref
            // over the same key is a typed integrity refusal, never a
            // success riding the existing offset.
            if existing.evidence() != &commit.evidence {
                return Err(SettlementAdmissionError::AlreadyCommittedDivergence(
                    Box::new(AlreadyCommittedDivergenceRefusal {
                        run_id: commit.run_id.clone(),
                        unit_id: commit.unit_id.clone(),
                        verification: commit.verification.clone(),
                        committed_evidence: existing.evidence().clone(),
                        offered_evidence: commit.evidence.clone(),
                        event_log_offset: existing.event_log_offset(),
                    }),
                ));
            }
            return Err(SettlementAdmissionError::AlreadyCommitted {
                run_id: commit.run_id.clone(),
                unit_id: commit.unit_id.clone(),
                verification: commit.verification.clone(),
                event_log_offset: existing.event_log_offset(),
            });
        }
        // COR2-1(a) (critic round 1, F-1's admission half): the exact key
        // is not durable, but a DIFFERENT key for the SAME (run, unit) is.
        // A fresh honest PASS mints a fresh verification ref — every
        // condition-1 check above holds for it — and before this guard the
        // fresh key admitted over the already-settled unit, appending a
        // second settlement event and row. One settlement per unit per
        // run; re-running settled work is §18.3's defect, not a retry.
        if let Some(settled) = existing_rows
            .iter()
            .find(|row| row.unit_id() == &commit.unit_id)
        {
            return Err(SettlementAdmissionError::UnitAlreadySettled(Box::new(
                UnitAlreadySettledRefusal {
                    run_id: commit.run_id.clone(),
                    unit_id: commit.unit_id.clone(),
                    settled_verification: settled.verification().clone(),
                    offered_verification: commit.verification.clone(),
                    event_log_offset: settled.event_log_offset(),
                },
            )));
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
            record: &super::AdmittedVerificationRecord,
            _: Deadline,
        ) -> Result<(), StateStoreError> {
            self.verifications
                .lock()
                .expect("not poisoned")
                .push(record.record().clone());
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

    /// COR2-1a's fixture: the same honest PASS shape under a DIFFERENT
    /// verification ref (a fresh re-verification of the same unit+evidence
    /// mints a fresh ref — the falsifier's F-1 second leg).
    fn pass_record_with_ref(
        unit: &str,
        evidence: &str,
        verification: &str,
    ) -> crate::verification_record::VerificationRecordDoc {
        verification_doc(unit, evidence, gauntlet_domain::Outcome::Pass, verification)
    }

    fn pass_record(
        unit: &str,
        evidence: &str,
        outcome: gauntlet_domain::Outcome,
    ) -> crate::verification_record::VerificationRecordDoc {
        // The default fixture's ref — deliberately the honest commit's own
        // (COR-3: admit compares the durable record's ref to the commit's).
        verification_doc(unit, evidence, outcome, "vrf-000900")
    }

    fn verification_doc(
        unit: &str,
        evidence: &str,
        outcome: gauntlet_domain::Outcome,
        verification: &str,
    ) -> crate::verification_record::VerificationRecordDoc {
        crate::verification_record::VerificationRecordDoc::new(
            crate::verification_record::VerificationContent {
                verification: gauntlet_domain::VerificationRef::new(verification)
                    .expect("valid verification ref"),
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

    /// The fixture record under an explicit verification ref and evidence —
    /// the fresh-honest-PASS shape the COR2-1(a) sequence mints (a NEW ref
    /// over NEW evidence: every admission check holds for it, which is
    /// exactly why the one-settlement-per-unit guard is its own rule).
    fn pass_record_under(
        unit: &str,
        evidence: &str,
        verification: &str,
        outcome: gauntlet_domain::Outcome,
    ) -> crate::verification_record::VerificationRecordDoc {
        let mut content = pass_record(unit, evidence, gauntlet_domain::Outcome::Pass).content();
        content.verification =
            gauntlet_domain::VerificationRef::new(verification).expect("valid verification ref");
        content.outcome = outcome;
        crate::verification_record::VerificationRecordDoc::new(content)
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

    /// Earns a verification record into the gate state through the
    /// recording authority (COR-1: the raw trait write takes the minted
    /// type, so even the fixture goes through the earning path).
    fn earn_record(
        state: &std::sync::Arc<SettlementGateState>,
        record: &crate::verification_record::VerificationRecordDoc,
    ) -> Result<(), super::ProducerAuthorityError> {
        super::VerificationRecordAuthority::new(state.clone()).record(record, bound())
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
        earn_record(
            &state,
            &pass_record(
                "AK-900",
                "evd-000900",
                gauntlet_domain::Outcome::Inconclusive,
            ),
        )
        .expect("the record lands");
        assert!(matches!(
            settlement_authority(&state).admit(settlement_commit(), bound()),
            Err(super::SettlementAdmissionError::VerificationNotPass { .. })
        ));
        // A PASS record for a DIFFERENT unit never admits this unit.
        earn_record(
            &state,
            &pass_record("AK-901", "evd-000900", gauntlet_domain::Outcome::Pass),
        )
        .expect("the record lands");
        assert!(matches!(
            settlement_authority(&state).admit(settlement_commit(), bound()),
            Err(super::SettlementAdmissionError::VerificationNotPass { .. })
        ));
        // A PASS record for the unit whose evidence ref disagrees with the
        // commit's — the committed evidence must be the evidence the durable
        // PASS record verified.
        earn_record(
            &state,
            &pass_record("AK-900", "evd-000901", gauntlet_domain::Outcome::Pass),
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
        earn_record(
            &state,
            &pass_record("AK-900", "evd-000900", gauntlet_domain::Outcome::Pass),
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

    /// COR2-1a (critic round 1, F-1): one settlement per (run, unit). A
    /// fresh honest PASS with a FRESH verification ref is a fresh §16.3
    /// key — every condition-1 check holds for it — but the unit is
    /// already durably settled, and re-settling it would append a second
    /// settlement event and row ("re-running settled work is a defect",
    /// §18.3).
    #[test]
    fn a_fresh_verification_key_cannot_re_settle_a_settled_unit() {
        let state = std::sync::Arc::new(SettlementGateState::with_run(fixture_run()));
        earn_record(
            &state,
            &pass_record("AK-900", "evd-000900", gauntlet_domain::Outcome::Pass),
        )
        .expect("the first record lands");
        let authority = settlement_authority(&state);
        let admitted = authority
            .admit(settlement_commit(), bound())
            .expect("the first honest settlement earns its admission");
        state
            .commit_settlement(&admitted, bound())
            .expect("the store commits the first settlement");
        // The second honest PASS: same unit, same evidence, FRESH ref.
        earn_record(
            &state,
            &pass_record_with_ref("AK-900", "evd-000900", "vrf-000901"),
        )
        .expect("the second record lands");
        let mut second = settlement_commit();
        second.verification =
            gauntlet_domain::VerificationRef::new("vrf-000901").expect("valid verification ref");
        assert!(matches!(
            authority.admit(second, bound()),
            Err(super::SettlementAdmissionError::UnitAlreadySettled { .. })
        ));
    }

    #[test]
    fn an_earned_admission_commits_through_the_store_and_reads_back() {
        let state = std::sync::Arc::new(SettlementGateState::with_run(fixture_run()));
        earn_record(
            &state,
            &pass_record("AK-900", "evd-000900", gauntlet_domain::Outcome::Pass),
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

    /// COR-1 (early probe C1): the C1 attack — hand-built PASS document,
    /// raw trait write, admit, commit — is closed at the TYPE level: the
    /// trait's `record_verification` takes the minted
    /// [`AdmittedVerificationRecord`], so a caller holding a document it
    /// built itself has nothing to pass to any store (fail-first evidence:
    /// this test's pre-fix form asserted the raw write must be refused and
    /// FAILED with "an ungated VerificationRecordDoc write must be refused"
    /// while the raw write existed). What remains runtime-checkable is the
    /// authority's own grounding validation, below and in the sibling tests.
    #[test]
    fn a_hand_built_pass_document_cannot_become_the_admission_substrate() {
        let state = std::sync::Arc::new(SettlementGateState::default());
        // The recording authority refuses the document outright when the run
        // it names is not durable — the forged substrate never lands, so the
        // settlement admission that reads it never opens.
        assert!(matches!(
            earn_record(
                &state,
                &pass_record("AK-900", "evd-000900", gauntlet_domain::Outcome::Pass)
            ),
            Err(super::ProducerAuthorityError::RunNotDurable { .. })
        ));
        assert!(
            state
                .verification_records(&run_id(), bound())
                .expect("reads")
                .is_empty(),
            "nothing landed"
        );
        assert!(matches!(
            settlement_authority(&state).admit(settlement_commit(), bound()),
            Err(super::SettlementAdmissionError::RunNotDurable { .. })
        ));
    }

    /// COR-1: the recording authority validates the document's declared
    /// iteration against the run's durable anchor — a stale or ahead
    /// iteration would certify a loop pass that is not the current one.
    #[test]
    fn an_unsupported_iteration_refuses_the_verification_record_write() {
        let state = std::sync::Arc::new(SettlementGateState::with_run(fixture_run()));
        let mut stale =
            pass_record("AK-900", "evd-000900", gauntlet_domain::Outcome::Pass).content();
        stale.iteration_no = IterationNo::new(2).expect("1-based");
        let doc = crate::verification_record::VerificationRecordDoc::new(stale)
            .expect("altered content is well formed");
        assert!(matches!(
            earn_record(&state, &doc),
            Err(super::ProducerAuthorityError::IterationUnsupported {
                requested, durable, ..
            })
                if requested.get() == 2 && durable.get() == 1
        ));
        assert!(
            state
                .verification_records(&run_id(), bound())
                .expect("reads")
                .is_empty(),
            "the stale-iteration document never landed"
        );
    }

    /// COR-1: the honest document earns its record through the authority
    /// and reads back — the earning path the verified chain composes.
    #[test]
    fn an_earned_verification_record_reads_back() {
        let state = std::sync::Arc::new(SettlementGateState::with_run(fixture_run()));
        let doc = pass_record("AK-900", "evd-000900", gauntlet_domain::Outcome::Pass);
        earn_record(&state, &doc).expect("the grounded document records");
        let stored = state
            .verification_records(&run_id(), bound())
            .expect("reads");
        assert_eq!(stored.as_slice(), &[doc]);
    }

    /// COR-5 fail-first (early probe M5): admission does not trust the
    /// store's run scoping. `SettlementGateState` is a CONFORMING but
    /// run-blind store — its `verification_records` returns every record it
    /// holds — so a durable PASS naming a FOREIGN run reaches the filter.
    /// Before the correction `admit` never compared `record.run_id()` to
    /// the commit's run, and the foreign PASS admitted the commit.
    #[test]
    fn a_foreign_run_pass_record_never_admits_even_under_a_run_blind_store() {
        let state = std::sync::Arc::new(SettlementGateState::with_run(fixture_run()));
        // The foreign-run PASS: identical to the honest fixture except its
        // run. It sits in the store directly — the sloppy-store shape; the
        // recording authority itself would refuse it (COR-1), which is
        // precisely why admission needs its own defense.
        let mut foreign =
            pass_record("AK-900", "evd-000900", gauntlet_domain::Outcome::Pass).content();
        foreign.run_id =
            gauntlet_domain::RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAB").expect("foreign run id");
        let foreign =
            crate::verification_record::VerificationRecordDoc::new(foreign).expect("well formed");
        state
            .record_verification(
                &super::AdmittedVerificationRecord {
                    record: foreign.clone(),
                },
                bound(),
            )
            .expect("the run-blind store holds it");
        // The store hands the foreign record to admission (run-blind).
        assert!(
            state
                .verification_records(&run_id(), bound())
                .expect("reads")
                .iter()
                .any(|record| record == &foreign),
            "the fixture store is the discriminating run-blind instrument"
        );
        let refused = settlement_authority(&state).admit(settlement_commit(), bound());
        assert!(
            matches!(
                refused,
                Err(super::SettlementAdmissionError::VerificationNotPass { .. })
            ),
            "a PASS of another run never admits this run's commit: {refused:?}"
        );
    }

    /// COR-4 (early probe M2): a §16.3 key hit must AGREE with the stored
    /// row — the same key over DIFFERENT evidence is the typed
    /// `AlreadyCommittedDivergence` integrity refusal, and only an agreeing
    /// re-entry is the idempotent `AlreadyCommitted` shape.
    #[test]
    fn a_key_hit_diverging_on_evidence_is_a_typed_refusal() {
        let state = std::sync::Arc::new(SettlementGateState::with_run(fixture_run()));
        earn_record(
            &state,
            &pass_record("AK-900", "evd-000900", gauntlet_domain::Outcome::Pass),
        )
        .expect("the record lands");
        let authority = settlement_authority(&state);
        let admitted = authority
            .admit(settlement_commit(), bound())
            .expect("the honest settlement earns its admission");
        state
            .commit_settlement(&admitted, bound())
            .expect("the store commits the admitted settlement");
        // A second durable PASS for the SAME ref over DIFFERENT evidence —
        // the admission's evidence and ref checks both hold for it.
        let divergent = pass_record("AK-900", "evd-000901", gauntlet_domain::Outcome::Pass);
        earn_record(&state, &divergent).expect("the divergent PASS lands");
        let mut offered = settlement_commit();
        offered.evidence = EvidenceRef::new("evd-000901").expect("valid evidence ref");
        match authority.admit(offered, bound()) {
            Err(super::SettlementAdmissionError::AlreadyCommittedDivergence(refusal)) => {
                assert_eq!(refusal.committed_evidence.as_str(), "evd-000900");
                assert_eq!(refusal.offered_evidence.as_str(), "evd-000901");
            }
            other => panic!("the divergent key hit refuses by type, got {other:?}"),
        }
        // The AGREEING re-entry stays the idempotent shape.
        assert!(matches!(
            authority.admit(settlement_commit(), bound()),
            Err(super::SettlementAdmissionError::AlreadyCommitted { .. })
        ));
    }

    /// COR-3 fail-first (early probe M1): the N-distinct-refs attack. One
    /// honest durable PASS record; the first commit settles under the §16.3
    /// key carrying the record's verification ref. The attack presents
    /// FURTHER commits over the SAME unit and evidence under DIFFERENT,
    /// caller-invented verification refs — each a distinct §16.3 key, each
    /// appending its own durable settlement event. The admission must moor
    /// key component three in the durable record: a commit whose
    /// verification ref no durable PASS record names never admits.
    #[test]
    fn one_pass_record_cannot_settle_n_distinct_verification_refs() {
        let state = std::sync::Arc::new(SettlementGateState::with_run(fixture_run()));
        earn_record(
            &state,
            &pass_record("AK-900", "evd-000900", gauntlet_domain::Outcome::Pass),
        )
        .expect("the record lands");
        let authority = settlement_authority(&state);
        let admitted = authority
            .admit(settlement_commit(), bound())
            .expect("the honest settlement earns its admission");
        state
            .commit_settlement(&admitted, bound())
            .expect("the store commits the admitted settlement");
        // The attack: two further commits, same unit and evidence,
        // DIFFERENT invented verification refs.
        for invented in ["vrf-forged-a", "vrf-forged-b"] {
            let mut forged = settlement_commit();
            forged.verification =
                gauntlet_domain::VerificationRef::new(invented).expect("valid ref shape");
            let refused = authority.admit(forged, bound());
            assert!(
                refused.is_err(),
                "a commit under invented ref {invented} must not admit over one PASS: \
                 {refused:?}"
            );
            // And therefore no second durable settlement can land.
            assert_eq!(
                state.settlements(&run_id(), bound()).expect("reads").len(),
                1,
                "exactly one settlement row, however many refs are invented"
            );
        }
    }

    /// COR2-1(a) fail-first (critic round 1, F-1's admission half): the
    /// falsifier's sequence — a settled unit re-attempted through a FRESH
    /// honest PASS (a new verification ref over new evidence), which mints a
    /// fresh §16.3 key. Before the correction `admit` only checked whether
    /// the OFFERED key was already durable, so the fresh key admitted and
    /// the commit appended a second settlement event and row for a unit
    /// that had already settled. After: a typed refusal when ANY settlement
    /// row exists for the (run, unit) under a different key; the
    /// same-key-same-fields shape keeps its idempotent `AlreadyCommitted`
    /// refusal (pinned by the sibling tests).
    #[test]
    fn a_settled_unit_re_attempted_under_a_fresh_honest_pass_refuses() {
        let state = std::sync::Arc::new(SettlementGateState::with_run(fixture_run()));
        // The honest first settlement: PASS under vrf-000900 over
        // evd-000900, admitted and committed — one row.
        earn_record(
            &state,
            &pass_record("AK-900", "evd-000900", gauntlet_domain::Outcome::Pass),
        )
        .expect("the record lands");
        let authority = settlement_authority(&state);
        let admitted = authority
            .admit(settlement_commit(), bound())
            .expect("the honest settlement earns its admission");
        state
            .commit_settlement(&admitted, bound())
            .expect("the store commits the admitted settlement");
        assert_eq!(
            state.settlements(&run_id(), bound()).expect("reads").len(),
            1
        );
        // The fresh honest PASS: a NEW verification ref over NEW evidence —
        // every §16.1-condition-1 check the admission performs holds for it
        // (durable PASS, this run, this unit, this evidence, this ref).
        let fresh = pass_record_under(
            "AK-900",
            "evd-000901",
            "vrf-000901",
            gauntlet_domain::Outcome::Pass,
        );
        earn_record(&state, &fresh).expect("the fresh honest PASS lands");
        let mut re_attempt = settlement_commit();
        re_attempt.evidence = EvidenceRef::new("evd-000901").expect("valid evidence ref");
        re_attempt.verification =
            gauntlet_domain::VerificationRef::new("vrf-000901").expect("valid ref shape");
        match authority.admit(re_attempt, bound()) {
            Err(super::SettlementAdmissionError::UnitAlreadySettled(refusal)) => {
                let boxed = *refusal;
                assert_eq!(boxed.settled_verification.as_str(), "vrf-000900");
                assert_eq!(boxed.offered_verification.as_str(), "vrf-000901");
            }
            other => panic!(
                "a settled unit never admits a second settlement under a fresh key, got {other:?}"
            ),
        }
        // And therefore nothing further can be committed: exactly one row
        // stands, however honest the re-attempt's own evidence was.
        assert_eq!(
            state.settlements(&run_id(), bound()).expect("reads").len(),
            1,
            "one settlement per (run, unit), however many fresh PASSes follow"
        );
    }

    /// The structural gate, verification-record leg (COR-1):
    /// `AdmittedVerificationRecord` has no constructor outside this module,
    /// so a caller holding a hand-built document has nothing to pass to any
    /// store — the raw trait signature takes the minted type.
    #[test]
    fn the_minted_verification_record_has_no_public_constructor() {
        let doc = pass_record("AK-900", "evd-000900", gauntlet_domain::Outcome::Pass);
        let admitted = super::AdmittedVerificationRecord {
            record: doc.clone(),
        };
        assert_eq!(admitted.record(), &doc);
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
