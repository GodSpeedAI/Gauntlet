//! The earned verification chain (plan Task 11; spec §14.1–§14.3).
//!
//! [`VerificationAuthority::for_run`] performs the governed open (durable
//! run + digest-verified frozen reference projection, parsed into the answer
//! key); [`GovernedVerification::verify_claim`] re-derives every binding
//! fact from durable state, runs the §14.3 checks, dispatches the §14.2
//! ladder, and mints the only consequential [`VerifiedClaim`]. Nothing in
//! this module compares two values the same caller supplied — see the
//! module doc in [`super`].

use std::path::{Path, PathBuf};
use std::sync::Arc;

use gauntlet_domain::{
    EpochSeconds, EvidenceLocator, EvidenceRef, InstrumentName, IterationNo, Outcome, Producer,
    Run, RunId, Unit, UnitId, VerificationRecord,
};
use gauntlet_ports::comparator::{ComparatorError, FrozenReferenceProjection};
use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::evidence_store::EvidenceStore;
use gauntlet_ports::id_generator::IdGenerator;
use gauntlet_ports::observer::InstrumentIdentity;
use gauntlet_ports::state_store::{
    EvidenceProduction, EvidenceProductionBinding, StateStore, StateStoreError,
};
use gauntlet_ports::verification_record::{
    BindingClass, RequirementDoc, VerificationContent, VerificationRecordDoc, VerifyingInstrument,
};
use gauntlet_ports::verifier::VerificationRequest;

use super::ladder::{Ladder, LadderDecision};

/// Admission failures for the governed chain (spec §21.2: typed, never a
/// silently degraded success).
#[derive(Debug)]
pub enum VerificationAdmissionError {
    /// The run is not durable — no consequential verification exists for a
    /// run the store never recorded.
    RunNotDurable { run_id: RunId },
    /// The requested iteration anchor is not one the run's durable state
    /// supports (correction round 1, C5/B-M2 first half): the latest
    /// durable checkpoint stands at `durable` (iteration 1 for a run that
    /// has crossed no boundary), so anchoring at `requested` would either
    /// re-fresh prior-iteration evidence (an old anchor) or certify a loop
    /// pass that never happened (a future anchor).
    IterationUnsupported {
        run_id: RunId,
        requested: IterationNo,
        durable: IterationNo,
    },
    /// A durable read failed.
    State(StateStoreError),
    /// The evidence workspace root cannot be resolved (correction round 2,
    /// F1): the chain re-reads locator bytes under a CANONICALIZED root —
    /// the same construction discipline the verifier adapters bind with —
    /// and a root that does not resolve refuses the governed open. Nothing
    /// downstream may read evidence through a root the open never pinned.
    WorkspaceRootRefused { detail: String },
    /// The frozen reference could not be loaded or its pinned digest did
    /// not cover the served bytes (the I1 tripwire fires here too).
    ReferenceRefused(ComparatorError),
}

impl core::fmt::Display for VerificationAdmissionError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::RunNotDurable { run_id } => {
                write!(
                    f,
                    "run {run_id} is not durable; verification authority cannot open"
                )
            }
            Self::IterationUnsupported {
                run_id,
                requested,
                durable,
            } => write!(
                f,
                "run {run_id} stands durably at iteration {} (its latest checkpoint); \
                 anchoring verification at iteration {} is refused — the anchor must be the \
                 run's durable iteration",
                durable.get(),
                requested.get()
            ),
            Self::State(error) => write!(f, "a durable verification read failed: {error}"),
            Self::WorkspaceRootRefused { detail } => {
                write!(f, "the evidence workspace root was refused: {detail}")
            }
            Self::ReferenceRefused(error) => {
                write!(f, "the frozen reference was refused: {error}")
            }
        }
    }
}

impl std::error::Error for VerificationAdmissionError {}

/// Verification refusals — everything that is NOT a three-state outcome
/// (spec §21.2). Binding failures are NOT here: they are
/// `Outcome::Inconclusive` records with a [`BindingClass`], minted as
/// [`VerifiedClaim`]s like any other outcome.
#[derive(Debug)]
pub enum VerificationRefusal {
    /// The request names a run other than the governed one.
    RunMismatch { governed: RunId, request: RunId },
    /// The frozen reference does not contain the unit — either no block
    /// with that id exists, or its block failed answer-key parsing. A unit
    /// without a frozen requirement cannot be verified (I8: a missing
    /// REQUIREMENT is not a satisfied one).
    UnitUnavailable { unit: UnitId, detail: String },
    /// A durable read failed during the binding checks.
    State(StateStoreError),
    /// The evidence store failed during the binding checks.
    Evidence { detail: String },
    /// The instrument returned a record that contradicts its dispatch
    /// (another instrument's name, or another evidence pointer).
    InstrumentContradictsDispatch {
        record_instrument: String,
        rung_instrument: String,
        detail: String,
    },
    /// The evidence store served bytes that are not the content the
    /// durable production record names (correction round 1, C3/B-M4: the
    /// `ObservationBinding` content-digest discipline, mirrored on the
    /// production surface). The examined content and the pinned ref must
    /// be one fact — a mismatch is tampering or corruption, refused
    /// whoever serves the bytes, never converted into an outcome.
    EvidenceContentMismatch {
        evidence: EvidenceRef,
        recorded: gauntlet_domain::Digest,
        served: gauntlet_domain::Digest,
    },
    /// The bytes at the evidence LOCATOR — the mutable workspace surface
    /// the dispatched instrument examines — are not the content the
    /// durable production record names (correction round 2, F1: the C3
    /// store-side binding was one-directional, and a doctored workspace
    /// file rode through the real adapter to a PASS over pinned FAILING
    /// bytes). Before the instrument is dispatched, the chain re-reads the
    /// locator under the governed workspace root — the same containment
    /// discipline the adapters apply — and requires the three-way
    /// equality `digest(locator bytes) == production.content() ==
    /// digest(store bytes)`. A mismatch is tampering or corruption:
    /// refused BEFORE dispatch, never converted into an outcome, never
    /// settled over. Residual (bounded, recorded): the chain's read and
    /// the instrument's read are two reads of a mutable file microseconds
    /// apart in one process — spec §22.2 records that the harness is not a
    /// sandbox; closing the race entirely means feeding instruments
    /// store-served bytes (a Verifier port revision), not chosen here.
    LocatorContentMismatch {
        locator: EvidenceLocator,
        recorded: gauntlet_domain::Digest,
        examined: gauntlet_domain::Digest,
    },
    /// The ladder refused the dispatch (empty, mis-ordered, or no rung can
    /// decide the requirement).
    Ladder(super::ladder::LadderError),
}

impl core::fmt::Display for VerificationRefusal {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::RunMismatch { governed, request } => write!(
                f,
                "the request names run {request} but the governed verification run is {governed}"
            ),
            Self::UnitUnavailable { unit, detail } => {
                write!(
                    f,
                    "the frozen reference has no verifiable unit {unit}: {detail}"
                )
            }
            Self::State(error) => write!(f, "a durable verification read failed: {error}"),
            Self::Evidence { detail } => {
                write!(f, "the evidence store refused the binding check: {detail}")
            }
            Self::EvidenceContentMismatch {
                evidence,
                recorded,
                served,
            } => write!(
                f,
                "the evidence store served {served} at {evidence}, but the durable production \
                 record names {recorded} as the ingested content — the examined content and the \
                 pinned ref are not one fact (refused, whoever serves the bytes)"
            ),
            Self::LocatorContentMismatch {
                locator,
                recorded,
                examined,
            } => write!(
                f,
                "the workspace file at locator {locator} hashes to {examined}, but the durable \
                 production record names {recorded} as the ingested content — the bytes the \
                 instrument would examine are not the pinned bytes (refused before dispatch, \
                 whoever wrote the file)"
            ),
            Self::InstrumentContradictsDispatch {
                record_instrument,
                rung_instrument,
                detail,
            } => write!(
                f,
                "the instrument returned a record contradicting its dispatch (record names \
                 {record_instrument:?}, the rung is {rung_instrument:?}): {detail}"
            ),
            Self::Ladder(error) => write!(f, "the verification ladder refused: {error}"),
        }
    }
}

impl std::error::Error for VerificationRefusal {}

/// Why one candidate failed the §14.3 binding validation: a binding CLASS
/// (minted on the INCONCLUSIVE record) or a typed REFUSAL (infrastructure —
/// never an outcome; §21 loci). Module-private: only the earned chain's
/// candidate walk produces or consumes it. The refusal is boxed to keep
/// the walk's error small (clippy `result_large_err`); it is unboxed
/// exactly once, at the walk's single `return Err`.
enum CandidateRejection {
    Class(BindingClass, String),
    Refused(Box<VerificationRefusal>),
}

/// The composition-root-injected substrate. Substituting these adapters is
/// the one remaining way to influence what the chain trusts — spec §7.1
/// adapter selection at an audited seam, not a public bypass.
pub struct VerificationAuthority {
    state: Arc<dyn StateStore>,
    evidence: Arc<dyn EvidenceStore>,
    ids: Arc<dyn IdGenerator>,
}

impl VerificationAuthority {
    pub fn new(
        state: Arc<dyn StateStore>,
        evidence: Arc<dyn EvidenceStore>,
        ids: Arc<dyn IdGenerator>,
    ) -> Self {
        Self {
            state,
            evidence,
            ids,
        }
    }

    /// Opens the governed chain for one durable run at one iteration, over
    /// the rung set the composition root composed (correction round 1,
    /// C1/B-M1: the ladder is EARNED AUTHORITY, injected HERE — at the
    /// governed open — never a per-claim argument of
    /// [`GovernedVerification::verify_claim`]. A hand-built rung has no
    /// parameter to travel through; the `AdmittedComparison` precedent —
    /// no public ladder constructor exists in a normal build of this
    /// crate). The evidence WORKSPACE ROOT travels with it under the same
    /// earned-authority discipline (correction round 2, F1): the root the
    /// composition root also binds its file-reading verifier adapters with
    /// — the chain re-reads locator bytes under the CANONICALIZED root so
    /// the bytes it binds are the bytes the instruments examine. Three
    /// governed reads happen here and nowhere else: the durable run (which
    /// carries its own frozen
    /// [`gauntlet_domain::ReferenceVersion`]), the run's latest durable
    /// CHECKPOINT (whose iteration must support the requested anchor,
    /// C5/B-M2), and the frozen projection's bytes, loaded at that
    /// version's pinned ref and verified against its pinned digest.
    /// Neither the reference identity nor its bytes are parameters.
    pub fn for_run(
        &self,
        run_id: &RunId,
        iteration_no: IterationNo,
        ladder: Ladder,
        workspace_root: PathBuf,
        deadline: Deadline,
    ) -> Result<GovernedVerification, VerificationAdmissionError> {
        let run = self
            .state
            .load_run(run_id, deadline)
            .map_err(VerificationAdmissionError::State)?
            .ok_or_else(|| VerificationAdmissionError::RunNotDurable {
                run_id: run_id.clone(),
            })?;
        // F1: canonicalize the evidence workspace root NOW — the same
        // construction discipline the verifier adapters apply — so every
        // later locator read resolves under one pinned root, never a path
        // the caller can swap between the open and the read.
        let workspace = std::fs::canonicalize(&workspace_root).map_err(|error| {
            VerificationAdmissionError::WorkspaceRootRefused {
                detail: format!(
                    "the evidence workspace root {} does not resolve: {error}",
                    workspace_root.display()
                ),
            }
        })?;
        // C5: the anchor must be the run's durable iteration. The latest
        // checkpoint names where the run durably stands; a run that has
        // crossed no boundary stands at iteration 1. An older anchor would
        // re-fresh prior-iteration evidence; a newer one certifies a pass
        // that never happened. Both are refused — the anchor is validated
        // against durable state, never taken on the caller's word.
        let durable_iteration = self
            .state
            .resume(run_id, deadline)
            .map_err(VerificationAdmissionError::State)?
            .map_or_else(
                || IterationNo::new(1).expect("iterations are 1-based"),
                |checkpoint| checkpoint.iteration_no,
            );
        if iteration_no != durable_iteration {
            return Err(VerificationAdmissionError::IterationUnsupported {
                run_id: run_id.clone(),
                requested: iteration_no,
                durable: durable_iteration,
            });
        }
        let projection = FrozenReferenceProjection::load_version(
            run.reference_version(),
            self.evidence.as_ref(),
            deadline,
        )
        .map_err(VerificationAdmissionError::ReferenceRefused)?;
        Ok(GovernedVerification {
            state: self.state.clone(),
            evidence: self.evidence.clone(),
            ids: self.ids.clone(),
            run,
            projection,
            iteration_no,
            ladder,
            workspace,
        })
    }
}

/// A run whose durable identity, frozen reference, iteration anchor, rung
/// set, and evidence workspace root are established. Not `Clone`: authority
/// is earned per (run, iteration), not copied around.
pub struct GovernedVerification {
    state: Arc<dyn StateStore>,
    evidence: Arc<dyn EvidenceStore>,
    ids: Arc<dyn IdGenerator>,
    run: Run,
    projection: FrozenReferenceProjection,
    iteration_no: IterationNo,
    ladder: Ladder,
    /// The canonicalized evidence workspace root (F1): locators resolve
    /// under it with the adapters' own containment discipline, so the
    /// bytes the chain binds are the bytes the instruments examine.
    workspace: PathBuf,
}

impl GovernedVerification {
    /// The run's frozen reference version, as recorded durably.
    pub fn reference_version(&self) -> &gauntlet_domain::ReferenceVersion {
        self.run.reference_version()
    }

    /// The iteration this verification is anchored at (the loop pass whose
    /// transform the evidence must certify). This is the COORDINATE of the
    /// question, in the same trust position as
    /// `RunComparisonAuthority::open`'s iteration argument — the recency
    /// FACT (when the evidence was produced) is read from the store inside
    /// [`Self::verify_claim`], never from any argument.
    pub fn iteration_no(&self) -> IterationNo {
        self.iteration_no
    }

    /// The unit's frozen form from the parsed answer key. Crate-private:
    /// this is the requirement side of every binding check, and it is not
    /// handable around as a value to pair with caller data.
    fn frozen_unit(&self, unit_id: &UnitId) -> Result<Unit, VerificationRefusal> {
        let parsed = crate::reference::parse::parse_answer_key(self.projection.view());
        if let Some(unit) = parsed.units.iter().find(|unit| unit.id() == unit_id) {
            return Ok(unit.clone());
        }
        let detail = match parsed
            .failures
            .iter()
            .find(|failure| failure.block == Some(unit_id.clone()))
        {
            Some(failure) => {
                format!("the unit's answer-key block failed to parse: {failure}")
            }
            None => format!(
                "no block with that id exists in the frozen reference (digest {})",
                self.projection.digest()
            ),
        };
        Err(VerificationRefusal::UnitUnavailable {
            unit: unit_id.clone(),
            detail,
        })
    }

    /// The only consequential verification. Takes the caller's
    /// [`VerificationRequest`] as the CLAIM POINTER (run, unit) and derives
    /// everything else from durable state — INCLUDING the instruments: the
    /// §14.2 ladder is the one this governed verification was OPENED with
    /// (C1); no per-claim caller chooses a rung. Derived:
    ///
    /// - requirement ← the frozen unit's `evidence_required`;
    /// - evidence candidates ← the durable production bindings recorded at
    ///   that locator in this run;
    /// - resolvability AND content ← the evidence store AND the workspace
    ///   locator (the store-served bytes and the locator file must EACH
    ///   hash to the production's recorded content digest — C3's store
    ///   half, F1's locator half: the examined bytes and the pinned bytes
    ///   are one fact before any instrument is dispatched);
    /// - recency ← each production's ordering anchor (iteration + store-
    ///   minted event-log offset) versus the governed iteration.
    ///
    /// The request's `requirement`, `evidence`, `claim`, and `id` fields
    /// are NOT read for any binding decision — they are the caller's
    /// say-so, and a lying request has no parameter to lie through. The
    /// P12 proof demonstrates this directly.
    pub fn verify_claim(
        &self,
        request: &VerificationRequest,
        deadline: Deadline,
    ) -> Result<VerifiedClaim, VerificationRefusal> {
        if &request.run_id != self.run.id() {
            return Err(VerificationRefusal::RunMismatch {
                governed: self.run.id().clone(),
                request: request.run_id.clone(),
            });
        }
        let unit = self.frozen_unit(&request.unit_id)?;
        let requirement = unit.evidence_required().clone();

        // The evidence side: durable production records at the FROZEN
        // locator. There is no caller-supplied evidence ref.
        let productions = self
            .state
            .evidence_productions_at(self.run.id(), requirement.locator(), deadline)
            .map_err(VerificationRefusal::State)?;

        // §14.3 binding validation, freshest anchor first. The first
        // candidate that passes every check is the one verified; if none
        // passes, the FRESHEST candidate's class is the actionable one
        // (documented tie-break: the most recent production is the evidence
        // the owner last certified at this locator). Infrastructure
        // failures (store read errors, content-digest mismatches) are
        // typed REFUSALS, never candidate classes — the unanswerable stays
        // unanswerable (§21 loci; correction round 1, C6.1/C3).
        let mut freshest_failure: Option<(BindingClass, String, EvidenceRef)> = None;
        let mut admitted: Option<EvidenceProductionBinding> = None;
        for binding in productions.iter().rev() {
            match self.binding_class(binding, &requirement, deadline) {
                Ok(()) => {
                    admitted = Some(binding.clone());
                    break;
                }
                Err(CandidateRejection::Class(class, detail)) => {
                    if freshest_failure.is_none() {
                        freshest_failure =
                            Some((class, detail, binding.production().evidence().clone()));
                    }
                }
                Err(CandidateRejection::Refused(refusal)) => return Err(*refusal),
            }
        }

        let Some(binding) = admitted else {
            // No candidate passed the binding checks — or there were no
            // candidates at all. Either way: INCONCLUSIVE with the typed
            // class, never PASS, never silence (P12, §14.3/§14.4). When no
            // production exists at all, the record points its evidence_ref
            // at the required locator — the suite's locator-as-ref
            // convention for evidence that is not there.
            let (class, detail, evidence) = freshest_failure.unwrap_or_else(|| {
                (
                    BindingClass::EvidenceUnresolvable,
                    format!(
                        "no durable evidence-production record exists at locator {} in run {} — \
                         the requirement's evidence does not resolve there",
                        requirement.locator(),
                        self.run.id()
                    ),
                    EvidenceRef::new(requirement.locator().as_str())
                        .expect("a locator string is a valid ref string"),
                )
            });
            return Ok(self.mint_binding_failure(&unit, &requirement, &evidence, class, detail));
        };
        let production = binding.production().clone();
        let evidence_ref = production.evidence().clone();

        // §14.2 ladder dispatch over a CHAIN-CONSTRUCTED request: id minted
        // here, requirement from the frozen unit, claim from the frozen
        // unit, evidence locator from the durable production record. The
        // caller's request fields never reach an instrument.
        let dispatched = VerificationRequest {
            id: self.mint_verification_ref(),
            run_id: self.run.id().clone(),
            unit_id: unit.id().clone(),
            claim: unit.claim().to_owned(),
            requirement: requirement.clone(),
            evidence: production.locator().clone(),
        };
        match self
            .ladder
            .select_and_verify(&dispatched, deadline)
            .map_err(VerificationRefusal::Ladder)?
        {
            LadderDecision::Decided { rung, record } => {
                // The instrument's record must name the rung's own identity
                // and the evidence pointer it was dispatched with — a
                // verifier cannot return another instrument's work.
                let rung_identity = rung.identity();
                if record.instrument().as_str() != rung_identity.name().as_str() {
                    return Err(VerificationRefusal::InstrumentContradictsDispatch {
                        record_instrument: record.instrument().as_str().to_owned(),
                        rung_instrument: rung_identity.name().as_str().to_owned(),
                        detail: "the returned record names an instrument other than the rung's"
                            .to_owned(),
                    });
                }
                if record.evidence().as_str() != dispatched.evidence.as_str() {
                    return Err(VerificationRefusal::InstrumentContradictsDispatch {
                        record_instrument: record.instrument().as_str().to_owned(),
                        rung_instrument: rung_identity.name().as_str().to_owned(),
                        detail: format!(
                            "the returned record certifies evidence {} but was dispatched at \
                             locator {}",
                            record.evidence(),
                            dispatched.evidence
                        ),
                    });
                }
                let instrument = VerifyingInstrument::new(
                    rung_identity.name().as_str().to_owned(),
                    rung_identity.version().to_owned(),
                    rung.verifier().is_deterministic(),
                );
                let detail = format!(
                    "decided by the {} rung ({}) at 14.2 ladder position {}",
                    rung.kind(),
                    rung_identity.name(),
                    rung.kind().position()
                );
                // An instrument-returned INCONCLUSIVE carries NO §14.3
                // class: the instrument could not decide the content, which
                // is a different uncertainty than a binding failure (the
                // codec documents this distinction).
                Ok(VerifiedClaim {
                    document: self.mint(
                        &unit,
                        &requirement,
                        &evidence_ref,
                        &production,
                        instrument,
                        record.id(),
                        record.outcome(),
                        None,
                        detail,
                    ),
                    domain_record: record,
                    evidence: self.evidence.clone(),
                })
            }
            LadderDecision::EvidenceAbsent { locator, rung } => {
                // The layered reconciliation (spec §12.3, §21.3, P12): the
                // instrument could not find the evidence at its locator;
                // verification becomes INCONCLUSIVE for the dependent
                // claim, classed `evidence_unresolvable`. The instrument
                // that reported the absence is named on the record.
                let rung_identity = rung.identity();
                let detail = format!(
                    "the {} rung ({}) found no evidence at locator {locator} — P12: missing \
                     evidence is INCONCLUSIVE, never PASS",
                    rung.kind(),
                    rung_identity.name()
                );
                let instrument = VerifyingInstrument::new(
                    rung_identity.name().as_str().to_owned(),
                    rung_identity.version().to_owned(),
                    rung.verifier().is_deterministic(),
                );
                Ok(VerifiedClaim {
                    document: self.mint(
                        &unit,
                        &requirement,
                        &evidence_ref,
                        &production,
                        instrument,
                        &dispatched.id,
                        Outcome::Inconclusive,
                        Some(BindingClass::EvidenceUnresolvable),
                        detail,
                    ),
                    domain_record: VerificationRecord::new(
                        dispatched.id,
                        self.run.id().clone(),
                        unit.id().clone(),
                        evidence_ref.clone(),
                        Outcome::Inconclusive,
                        InstrumentName::new(rung_identity.name().as_str())
                            .expect("the rung's name is a validated instrument name"),
                    ),
                    evidence: self.evidence.clone(),
                })
            }
        }
    }

    fn mint_verification_ref(&self) -> gauntlet_domain::VerificationRef {
        gauntlet_domain::VerificationRef::new(&format!("vrf-{}", self.ids.invocation_id().as_str()))
            .expect("a vrf- prefixed ULID is a valid verification ref")
    }

    /// One candidate's §14.3 binding validation. Every input except the
    /// requirement comes from durable state. `Ok(())` = every condition
    /// holds; `Err(Class(..))` = the binding failure (a §14.3 class on the
    /// minted record); `Err(Refused(..))` = an infrastructure failure that
    /// must stay a typed refusal, never an outcome (§21 loci; correction
    /// round 1, C6.1/C3).
    fn binding_class(
        &self,
        binding: &EvidenceProductionBinding,
        requirement: &gauntlet_domain::EvidenceRequirement,
        deadline: Deadline,
    ) -> Result<(), CandidateRejection> {
        let production = binding.production();
        // Condition 4 — locator match (exact for deterministic kinds). The
        // lookup was BY locator so equality holds structurally for a
        // conforming store; the check is the app-layer defense against a
        // locator-filter regression in a store adapter (the C6.2 tooth
        // demonstrates it biting a contract-violating store), and it stands
        // for the recorded mapping (mismatch ⇒ the requirement's evidence
        // does not resolve at its locator).
        if production.locator() != requirement.locator() {
            return Err(CandidateRejection::Class(
                BindingClass::EvidenceUnresolvable,
                format!(
                    "the evidence was produced at locator {}, but the requirement's locator is \
                     {} — evidence at another locator is not the required evidence (the \
                     recorded mapping for the fourth 14.3 condition)",
                    production.locator(),
                    requirement.locator()
                ),
            ));
        }
        // Condition 1 — the evidence ref resolves in the evidence store.
        // A read FAILURE is not a §14.3 class: it is the §21 persistence
        // locus, a typed refusal (C6.1) — the unanswerable never degrades
        // into an answerable class on a record.
        let resolved = self
            .evidence
            .get(production.evidence(), deadline)
            .map_err(|error| {
                CandidateRejection::Refused(Box::new(VerificationRefusal::Evidence {
                    detail: format!(
                        "reading evidence {} from the store failed: {error}",
                        production.evidence()
                    ),
                }))
            })?;
        let Some(bytes) = resolved else {
            return Err(CandidateRejection::Class(
                BindingClass::EvidenceUnresolvable,
                format!(
                    "evidence ref {} does not resolve in the evidence store",
                    production.evidence()
                ),
            ));
        };
        // Content binding (correction round 1, C3/B-M4): the served bytes
        // must BE the recorded content — the digest of what the store
        // serves is re-derived and compared against the digest the
        // producer recorded for the bytes ingested at this locator. The
        // examined content and the pinned ref are one fact or the mismatch
        // is refused, whoever serves the bytes.
        let served = super::content_digest(&bytes);
        if &served != production.content() {
            return Err(CandidateRejection::Refused(Box::new(
                VerificationRefusal::EvidenceContentMismatch {
                    evidence: production.evidence().clone(),
                    recorded: production.content().clone(),
                    served,
                },
            )));
        }
        // Condition 2 — the producing instrument matches
        // `evidence_required.producer`. The domain's Producer vocabulary is
        // a CLASS (`instrument | human`; spec §5.3.2 — "never: model
        // assertion"): an instrument production record satisfies
        // `instrument`; a `human` requirement (human sign-off evidence)
        // cannot be satisfied by instrument-produced evidence. Recorded
        // decision — the only faithful reading of a two-value vocabulary.
        match requirement.producer() {
            Producer::Instrument => {}
            Producer::Human => {
                return Err(CandidateRejection::Class(
                    BindingClass::EvidenceProducerMismatch,
                    format!(
                        "the requirement's producer is `human` (human sign-off evidence), but \
                         the durable production record names the instrument {} — an instrument \
                         cannot produce human evidence",
                        production.producer().name()
                    ),
                ));
            }
        }
        // Condition 3 — produced in this run at or after the transform it
        // certifies. The production records ARE run-keyed (a foreign run's
        // records are invisible to this lookup), so "in this run" holds
        // structurally; the recency FACT is the production's anchor
        // iteration versus the governed iteration — both durable, neither
        // supplied by any verification caller.
        if production.iteration_no().get() < self.iteration_no.get() {
            return Err(CandidateRejection::Class(
                BindingClass::EvidenceStale,
                format!(
                    "the evidence was produced at iteration {} (anchor offset {}), before the \
                     transform it must certify at iteration {} — no stale evidence",
                    production.iteration_no().get(),
                    binding.event_log_offset().get(),
                    self.iteration_no.get()
                ),
            ));
        }
        // Content binding, locator half — the LAST check before admission
        // (correction round 2, F1): the store-side check above binds the
        // durable record's own bytes, but the dispatched instrument
        // examines the LOCATOR — a mutable workspace file (spec §22.2) —
        // and nothing else bound those bytes. The binding is load-bearing
        // exactly when this candidate is about to dispatch, so it stands at
        // the gate: re-read the locator under the governed workspace root
        // (the adapters' own containment discipline) and require the
        // three-way equality `digest(locator bytes) ==
        // production.content() == digest(store bytes)`. An honest store
        // record plus a doctored workspace file — the confirmation critic's
        // executed attack — is refused HERE, as tampering, before any
        // instrument is dispatched. Candidates excluded by a class (stale,
        // producer mismatch) never dispatch, so they never reach this
        // read. Bounded residual: this read and the instrument's read are
        // microseconds apart in one process; §22.2 records that the
        // harness is not a sandbox (the race-free closure is a Verifier
        // port revision, recorded not chosen).
        if let Some(examined_bytes) = self.read_locator_content(production.locator())? {
            let examined = super::content_digest(&examined_bytes);
            if &examined != production.content() {
                return Err(CandidateRejection::Refused(Box::new(
                    VerificationRefusal::LocatorContentMismatch {
                        locator: production.locator().clone(),
                        recorded: production.content().clone(),
                        examined,
                    },
                )));
            }
        }
        Ok(())
    }

    /// Reads the bytes at one evidence locator under the governed
    /// workspace root — the SAME containment discipline the verifier
    /// adapters apply (correction round 2, F1): an absolute path or any
    /// `..` segment never resolves under the root, so the chain never
    /// binds bytes from outside the workspace the instruments read.
    ///
    /// `Ok(None)` is an ABSENT artifact, and the chain does not class the
    /// absence itself: the dispatched instrument reports it
    /// (`VerifierError::EvidenceMissing`) and the layered P12
    /// reconciliation mints the INCONCLUSIVE record naming the rung —
    /// absent evidence stays the instrument's finding, never a
    /// chain-minted PASS and never silence. Any other read failure is a
    /// typed refusal (the §21 persistence locus, the C6.1 discipline):
    /// the unanswerable never degrades into an outcome.
    fn read_locator_content(
        &self,
        locator: &EvidenceLocator,
    ) -> Result<Option<Vec<u8>>, CandidateRejection> {
        if escapes_the_workspace_root(locator.as_str()) {
            return Err(CandidateRejection::Refused(Box::new(
                VerificationRefusal::Evidence {
                    detail: format!(
                        "locator {locator} escapes the governed evidence workspace root — the \
                         chain refuses to bind bytes outside the workspace the instruments read"
                    ),
                },
            )));
        }
        match std::fs::read(self.workspace.join(locator.as_str())) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(CandidateRejection::Refused(Box::new(
                VerificationRefusal::Evidence {
                    detail: format!(
                        "reading the evidence locator {locator} under the governed workspace \
                         root failed: {error}"
                    ),
                },
            ))),
        }
    }

    /// Mints the §14.3 binding-failure record: INCONCLUSIVE, typed class,
    /// the binding layer as its author. There is no durable production to
    /// cite, so `produced_at` is epoch zero, the evidence_ref names where
    /// the requirement's evidence should have resolved, and the content
    /// digest is the all-zero placeholder (never compared — no store bytes
    /// are read on this path).
    fn mint_binding_failure(
        &self,
        unit: &Unit,
        requirement: &gauntlet_domain::EvidenceRequirement,
        evidence: &EvidenceRef,
        class: BindingClass,
        detail: String,
    ) -> VerifiedClaim {
        let instrument = VerifyingInstrument::new(
            "verification-chain".to_owned(),
            env!("CARGO_PKG_VERSION").to_owned(),
            true,
        );
        let placeholder = EvidenceProduction::new(
            evidence.clone(),
            gauntlet_domain::Digest::new("sha256", &"0".repeat(64))
                .expect("the zero placeholder is valid hex"),
            InstrumentIdentity::new(
                InstrumentName::new("verification-chain").expect("plain token"),
                String::new(),
            ),
            requirement.locator().clone(),
            IterationNo::new(1).expect("iterations are 1-based"),
            EpochSeconds::new(0),
        );
        // ONE ref for both records: the durable document names the
        // verification the domain record IS (COR-3).
        let verification = self.mint_verification_ref();
        VerifiedClaim {
            document: self.mint(
                unit,
                requirement,
                evidence,
                &placeholder,
                instrument,
                &verification,
                Outcome::Inconclusive,
                Some(class),
                detail,
            ),
            domain_record: VerificationRecord::new(
                verification,
                self.run.id().clone(),
                unit.id().clone(),
                evidence.clone(),
                Outcome::Inconclusive,
                InstrumentName::new("verification-chain").expect("plain token"),
            ),
            evidence: self.evidence.clone(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn mint(
        &self,
        unit: &Unit,
        requirement: &gauntlet_domain::EvidenceRequirement,
        evidence: &EvidenceRef,
        production: &EvidenceProduction,
        instrument: VerifyingInstrument,
        verification: &gauntlet_domain::VerificationRef,
        outcome: Outcome,
        class: Option<BindingClass>,
        detail: String,
    ) -> VerificationRecordDoc {
        VerificationRecordDoc::new(VerificationContent {
            // COR-3: the durable document names the verification it IS —
            // the same ref the domain record carries, so the §16.3 key's
            // third component is moored in the durable record the
            // settlement admission reads.
            verification: verification.clone(),
            run_id: self.run.id().clone(),
            iteration_no: self.iteration_no,
            unit_id: unit.id().clone(),
            claim: unit.claim().to_owned(),
            requirement: RequirementDoc::new(
                requirement.kind(),
                requirement.locator().clone(),
                requirement.producer(),
            ),
            instrument,
            evidence: evidence.clone(),
            produced_at: production.produced_at(),
            outcome,
            class,
            detail,
        })
        .expect("the chain only mints class-consistent content")
    }
}

/// Whether a locator can never resolve under a workspace root: an
/// absolute path (a root component, or a Windows drive prefix) or any
/// parent-directory segment escapes it. This is the adapters' own
/// containment discipline (`contained_path` in the verify adapters),
/// expressed through std's semantic path components — a strictly STRONGER
/// refusal (a drive-prefixed locator is refused here too), so the chain
/// never binds bytes from outside the workspace the instruments read.
fn escapes_the_workspace_root(raw: &str) -> bool {
    use std::path::Component;
    Path::new(raw).components().any(|component| {
        matches!(
            component,
            Component::RootDir | Component::ParentDir | Component::Prefix(_)
        )
    })
}

/// A verification that happened and counts — the only value downstream
/// services (the settlement precondition, later the controller) may treat
/// as a verification result. It has no constructor outside this module.
/// The production binding it was decided from is NOT carried: every fact in
/// it is durable and re-derivable from the store, which is the property
/// that makes the chain's decisions auditable after the fact.
///
/// The claim DOES carry the evidence store of the chain that minted it
/// (correction round 1, C4/B-M5): the settlement precondition re-derives
/// its evidence facts from THAT store — the claim's own — so a caller
/// cannot satisfy §16.1 condition 2 by presenting a stooge store. The
/// handle is private; no precondition parameter accepts a different one.
pub struct VerifiedClaim {
    document: VerificationRecordDoc,
    domain_record: VerificationRecord,
    evidence: Arc<dyn EvidenceStore>,
}

impl VerifiedClaim {
    /// The B.5 wire record (outcome, typed class when INCONCLUSIVE,
    /// instrument identity with its §14.2 flag).
    pub fn record(&self) -> &VerificationRecordDoc {
        &self.document
    }

    /// The lossy §5.3.6 domain record.
    pub fn domain_record(&self) -> &VerificationRecord {
        &self.domain_record
    }

    /// The evidence store of the chain that minted this claim — the only
    /// store the settlement precondition consults (crate-private).
    pub(crate) fn chain_evidence(&self) -> &Arc<dyn EvidenceStore> {
        &self.evidence
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::super::ladder::{Ladder, LadderRung, RungKind};
    use super::super::testing::*;
    use super::{VerificationAdmissionError, VerificationAuthority, VerificationRefusal};
    use gauntlet_domain::{
        EvidenceKind, EvidenceLocator, EvidenceRef, EvidenceRequirement, IterationNo, Outcome,
        Producer, RunId, UnitId,
    };
    use gauntlet_ports::state_store::StateStore;
    use gauntlet_ports::verification_record::BindingClass;
    use gauntlet_ports::verifier::VerificationRequest;

    fn unit_request(unit: &str) -> VerificationRequest {
        // The caller's say-so — deliberately used in the lying-request
        // tests to prove none of these fields reaches a binding decision.
        VerificationRequest {
            id: gauntlet_domain::VerificationRef::new("vrf-caller-0001").expect("valid ref"),
            run_id: RunId::new(RUN_ULID).expect("fixture run"),
            unit_id: UnitId::new(unit).expect("fixture unit"),
            claim: "whatever the caller claims".to_owned(),
            requirement: EvidenceRequirement::new(
                EvidenceKind::TestReport,
                EvidenceLocator::new("fixture-report-locator").expect("locator"),
                Producer::Instrument,
            ),
            evidence: EvidenceLocator::new("fixture-report-locator").expect("locator"),
        }
    }

    fn ladder_with_test_report_rung(passing: &[&str]) -> Ladder {
        Ladder::new(vec![LadderRung::new(
            RungKind::TestReport,
            Arc::new(FakeDeterministicVerifier {
                name: "fixture-verifier",
                kinds: &[EvidenceKind::TestReport],
                passing: passing.iter().map(|locator| locator.to_string()).collect(),
            }),
        )])
        .expect("a single-rung ladder is well formed")
    }

    fn chain() -> (VerificationAuthority, Arc<VerifyingState>) {
        let (state, evidence) = authority_parts();
        let authority =
            VerificationAuthority::new(state.clone(), evidence, Arc::new(SeededIds::new()));
        (authority, state)
    }

    #[test]
    fn a_run_that_is_not_durable_refuses_authority() {
        let (authority, state) = chain();
        let absent = RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAB").expect("other run");
        assert!(matches!(
            authority.for_run(
                &absent,
                IterationNo::new(1).expect("1-based"),
                ladder_with_test_report_rung(&[]),
                state.workspace_root().to_owned(),
                bound()
            ),
            Err(VerificationAdmissionError::RunNotDurable { .. })
        ));
    }

    #[test]
    fn a_tampered_frozen_reference_refuses_authority() {
        // Serve DIFFERENT bytes at the pinned reference evidence ref: the
        // projection's digest check (I1 tripwire) refuses the open.
        let evidence = Arc::new(PinningEvidence::default());
        let frozen = evidence.insert(
            "evd-fixture-frozen-reference",
            b"## AK-9001\nclaim: a tampered replacement\n",
        );
        let state = Arc::new(VerifyingState::new(fixture_run(frozen)));
        let authority =
            VerificationAuthority::new(state.clone(), evidence, Arc::new(SeededIds::new()));
        let run = RunId::new(RUN_ULID).expect("fixture run");
        assert!(matches!(
            authority.for_run(
                &run,
                IterationNo::new(1).expect("1-based"),
                ladder_with_test_report_rung(&[]),
                state.workspace_root().to_owned(),
                bound()
            ),
            Err(VerificationAdmissionError::ReferenceRefused(_))
        ));
    }

    #[test]
    fn a_request_naming_another_run_is_refused() {
        let (authority, state) = chain();
        let governed = authority
            .for_run(
                &RunId::new(RUN_ULID).expect("fixture run"),
                IterationNo::new(1).expect("1-based"),
                ladder_with_test_report_rung(&[]),
                state.workspace_root().to_owned(),
                bound(),
            )
            .expect("the fixture run opens");
        let mut request = unit_request("AK-9001");
        request.run_id = RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAB").expect("other run");
        assert!(matches!(
            governed.verify_claim(&request, bound()),
            Err(VerificationRefusal::RunMismatch { .. })
        ));
    }

    #[test]
    fn a_unit_absent_from_the_frozen_reference_is_refused() {
        let (authority, state) = chain();
        let governed = authority
            .for_run(
                &RunId::new(RUN_ULID).expect("fixture run"),
                IterationNo::new(1).expect("1-based"),
                ladder_with_test_report_rung(&[]),
                state.workspace_root().to_owned(),
                bound(),
            )
            .expect("the fixture run opens");
        assert!(matches!(
            governed.verify_claim(&unit_request("AK-9999"), bound()),
            Err(VerificationRefusal::UnitUnavailable { .. })
        ));
    }

    #[test]
    fn no_production_record_at_the_locator_is_inconclusive_unresolvable() {
        let (authority, state) = chain();
        let governed = authority
            .for_run(
                &RunId::new(RUN_ULID).expect("fixture run"),
                IterationNo::new(1).expect("1-based"),
                ladder_with_test_report_rung(&[]),
                state.workspace_root().to_owned(),
                bound(),
            )
            .expect("the fixture run opens");
        let claim = governed
            .verify_claim(&unit_request("AK-9001"), bound())
            .expect("the chain mints a record, not an error");
        assert_eq!(claim.record().outcome(), Outcome::Inconclusive);
        assert_eq!(
            claim.record().class(),
            Some(BindingClass::EvidenceUnresolvable)
        );
        // And INCONCLUSIVE never satisfies the settlement precondition —
        // which consults the claim's own chain store (C4).
        let readiness = super::super::precondition::settlement_precondition(
            &claim,
            &RunId::new(RUN_ULID).expect("fixture run"),
            bound(),
        );
        assert!(matches!(
            readiness,
            Err(super::super::precondition::PreconditionFailure::NotPass {
                outcome: Outcome::Inconclusive
            })
        ));
    }

    #[test]
    fn an_unresolvable_evidence_ref_is_inconclusive_unresolvable() {
        let (state, evidence) = authority_parts();
        // The production record exists... but its ref never landed in the
        // evidence store (the bytes are absent — the "absent evidence
        // file" case at the binding layer).
        state.record(
            "evd-never-stored",
            "fixture-report-verifier",
            "fixture-report-locator",
            1,
            b"never stored bytes",
        );
        let authority =
            VerificationAuthority::new(state.clone(), evidence, Arc::new(SeededIds::new()));
        let governed = authority
            .for_run(
                &RunId::new(RUN_ULID).expect("fixture run"),
                IterationNo::new(1).expect("1-based"),
                ladder_with_test_report_rung(&[]),
                state.workspace_root().to_owned(),
                bound(),
            )
            .expect("the fixture run opens");
        let claim = governed
            .verify_claim(&unit_request("AK-9001"), bound())
            .expect("the chain mints a record");
        assert_eq!(claim.record().outcome(), Outcome::Inconclusive);
        assert_eq!(
            claim.record().class(),
            Some(BindingClass::EvidenceUnresolvable)
        );
        assert!(
            claim.record().detail().contains("does not resolve"),
            "the detail names the failure locus: {}",
            claim.record().detail()
        );
    }

    #[test]
    fn evidence_from_a_prior_iteration_is_inconclusive_stale() {
        let (state, evidence) = authority_parts();
        // Produced at iteration 1; the governed verification stands at
        // iteration 2 — the evidence predates the transform it must
        // certify. The run's durable checkpoint stands at 2 (C5).
        state.record(
            "evd-fresh-once",
            "fixture-report-verifier",
            "fixture-report-locator",
            1,
            b"test report bytes",
        );
        evidence.insert("evd-fresh-once", b"test report bytes");
        state.checkpoint_at(2);
        let authority =
            VerificationAuthority::new(state.clone(), evidence, Arc::new(SeededIds::new()));
        let governed = authority
            .for_run(
                &RunId::new(RUN_ULID).expect("fixture run"),
                IterationNo::new(2).expect("1-based"),
                ladder_with_test_report_rung(&[]),
                state.workspace_root().to_owned(),
                bound(),
            )
            .expect("the fixture run opens");
        let claim = governed
            .verify_claim(&unit_request("AK-9001"), bound())
            .expect("the chain mints a record");
        assert_eq!(claim.record().outcome(), Outcome::Inconclusive);
        assert_eq!(claim.record().class(), Some(BindingClass::EvidenceStale));
    }

    #[test]
    fn a_lying_request_cannot_make_stale_evidence_fresh() {
        // P1's disconfirming observation: the request LIES (its requirement
        // claims a different producer, its evidence locator names the
        // freshest artifact, its claim text asserts success) — the decision
        // still reads durable state, so the record is still
        // INCONCLUSIVE/evidence_stale.
        let (state, evidence) = authority_parts();
        state.record(
            "evd-old",
            "fixture-report-verifier",
            "fixture-report-locator",
            1,
            b"test report bytes",
        );
        evidence.insert("evd-old", b"test report bytes");
        state.checkpoint_at(3);
        let authority =
            VerificationAuthority::new(state.clone(), evidence, Arc::new(SeededIds::new()));
        let governed = authority
            .for_run(
                &RunId::new(RUN_ULID).expect("fixture run"),
                IterationNo::new(3).expect("1-based"),
                ladder_with_test_report_rung(&[]),
                state.workspace_root().to_owned(),
                bound(),
            )
            .expect("the fixture run opens");
        let mut lying = unit_request("AK-9001");
        lying.requirement = EvidenceRequirement::new(
            EvidenceKind::TestReport,
            EvidenceLocator::new("fixture-report-locator").expect("locator"),
            Producer::Human, // the lie: a human never produced this
        );
        lying.claim = "the work is done and fresh".to_owned();
        let claim = governed
            .verify_claim(&lying, bound())
            .expect("the chain mints a record");
        assert_eq!(claim.record().outcome(), Outcome::Inconclusive);
        // The FROZEN requirement says producer=instrument; the stale
        // production is the actionable failure — staleness, not the lie.
        assert_eq!(claim.record().class(), Some(BindingClass::EvidenceStale));
    }

    #[test]
    fn verifying_at_a_later_iteration_never_advances_recency_on_its_own() {
        // task26's security property: a production's recency ADVANCES
        // only through a real, governed re-production
        // (`EvidenceProductionAuthority::record`, reached only from a
        // completion pipeline that re-read bytes from THIS iteration's
        // OWN workspace and validated the declared iteration against the
        // durable checkpoint) — never merely by VERIFYING, and never on a
        // request's say-so. `verify_claim` is a read: it must decide
        // INCONCLUSIVE/stale exactly like the tests above, and it must
        // never itself move the durable row's recorded iteration, however
        // many times (or at whatever governed iteration) it is called.
        let (state, evidence) = authority_parts();
        state.record(
            "evd-no-reproduction",
            "fixture-report-verifier",
            "fixture-report-locator",
            1,
            b"test report bytes",
        );
        evidence.insert("evd-no-reproduction", b"test report bytes");
        state.checkpoint_at(2);
        let authority =
            VerificationAuthority::new(state.clone(), evidence, Arc::new(SeededIds::new()));
        let governed = authority
            .for_run(
                &RunId::new(RUN_ULID).expect("fixture run"),
                IterationNo::new(2).expect("1-based"),
                ladder_with_test_report_rung(&[]),
                state.workspace_root().to_owned(),
                bound(),
            )
            .expect("the fixture run opens");
        // Verify TWICE at the later governed iteration: if verifying ever
        // mutated recency, the second call would see a fresher row than
        // the first.
        let first = governed
            .verify_claim(&unit_request("AK-9001"), bound())
            .expect("the chain mints a record");
        let second = governed
            .verify_claim(&unit_request("AK-9001"), bound())
            .expect("the chain mints a record");
        for claim in [&first, &second] {
            assert_eq!(claim.record().outcome(), Outcome::Inconclusive);
            assert_eq!(claim.record().class(), Some(BindingClass::EvidenceStale));
        }
        // The durable production row itself never moved: still iteration
        // 1, exactly as recorded — no read path may advance it.
        let production = state
            .evidence_production(
                &RunId::new(RUN_ULID).expect("fixture run"),
                &EvidenceRef::new("evd-no-reproduction").expect("fixture ref"),
                bound(),
            )
            .expect("readable")
            .expect("the row exists");
        assert_eq!(
            production.production().iteration_no().get(),
            1,
            "verifying alone must never advance a production's recorded iteration"
        );
    }

    #[test]
    fn instrument_produced_evidence_never_satisfies_a_human_requirement() {
        let (state, evidence) = authority_parts();
        state.record(
            "evd-signoff",
            "fixture-report-verifier",
            "fixture-signoff-locator",
            1,
            b"an instrument's attestation",
        );
        evidence.insert("evd-signoff", b"an instrument's attestation");
        let authority =
            VerificationAuthority::new(state.clone(), evidence, Arc::new(SeededIds::new()));
        let governed = authority
            .for_run(
                &RunId::new(RUN_ULID).expect("fixture run"),
                IterationNo::new(1).expect("1-based"),
                ladder_with_test_report_rung(&[]),
                state.workspace_root().to_owned(),
                bound(),
            )
            .expect("the fixture run opens");
        let claim = governed
            .verify_claim(&unit_request("AK-9002"), bound())
            .expect("the chain mints a record");
        assert_eq!(claim.record().outcome(), Outcome::Inconclusive);
        assert_eq!(
            claim.record().class(),
            Some(BindingClass::EvidenceProducerMismatch)
        );
        assert_eq!(claim.record().requirement().producer(), Producer::Human);
    }

    #[test]
    fn a_missing_instrument_artifact_is_inconclusive_unresolvable_p12() {
        // The binding checks PASS (a fresh, resolvable production record
        // exists), but the INSTRUMENT finds no evidence at the locator —
        // the layered reconciliation: Err(EvidenceMissing) becomes an
        // INCONCLUSIVE record with the §14.3 class, never silence and
        // never PASS.
        let (state, evidence) = authority_parts();
        state.record(
            "evd-present",
            "fixture-report-verifier",
            "fixture-report-locator",
            1,
            b"test report bytes",
        );
        evidence.insert("evd-present", b"test report bytes");
        let authority =
            VerificationAuthority::new(state.clone(), evidence, Arc::new(SeededIds::new()));
        let governed = authority
            .for_run(
                &RunId::new(RUN_ULID).expect("fixture run"),
                IterationNo::new(1).expect("1-based"),
                // The ladder's rung's passing set is EMPTY: the instrument
                // reports the artifact absent at the locator.
                ladder_with_test_report_rung(&[]),
                state.workspace_root().to_owned(),
                bound(),
            )
            .expect("the fixture run opens");
        let claim = governed
            .verify_claim(&unit_request("AK-9001"), bound())
            .expect("the chain mints a record");
        assert_eq!(claim.record().outcome(), Outcome::Inconclusive);
        assert_eq!(
            claim.record().class(),
            Some(BindingClass::EvidenceUnresolvable)
        );
        assert!(
            claim.record().detail().contains("P12"),
            "the record cites the P12 rule: {}",
            claim.record().detail()
        );
        // The instrument that reported the absence is NAMED on the record.
        assert_eq!(claim.record().instrument().name(), "fixture-verifier");
    }

    #[test]
    fn fresh_resolvable_evidence_passes_and_carries_the_instrument_identity() {
        let (state, evidence) = authority_parts();
        state.record(
            "evd-good",
            "fixture-report-verifier",
            "fixture-report-locator",
            2,
            b"test report bytes",
        );
        evidence.insert("evd-good", b"test report bytes");
        state.checkpoint_at(2);
        let authority =
            VerificationAuthority::new(state.clone(), evidence, Arc::new(SeededIds::new()));
        let governed = authority
            .for_run(
                &RunId::new(RUN_ULID).expect("fixture run"),
                IterationNo::new(2).expect("1-based"),
                ladder_with_test_report_rung(&["fixture-report-locator"]),
                state.workspace_root().to_owned(),
                bound(),
            )
            .expect("the fixture run opens");
        let claim = governed
            .verify_claim(&unit_request("AK-9001"), bound())
            .expect("the chain mints a record");
        assert_eq!(claim.record().outcome(), Outcome::Pass);
        assert_eq!(claim.record().class(), None);
        assert_eq!(claim.record().evidence().as_str(), "evd-good");
        assert_eq!(claim.record().instrument().name(), "fixture-verifier");
        assert!(claim.record().instrument().deterministic());
        // The record's requirement is the FROZEN one, not the caller's.
        assert_eq!(
            claim.record().requirement().kind(),
            EvidenceKind::TestReport
        );
        assert_eq!(
            claim.record().requirement().producer(),
            Producer::Instrument
        );
        // The claim text is the frozen claim, not the caller's.
        assert_eq!(
            claim.record().claim(),
            "the fixture claim is verified by a test report"
        );
    }

    #[test]
    fn the_freshest_passing_candidate_wins_and_ties_break_to_the_fresh_class() {
        let (state, evidence) = authority_parts();
        let older = state.record(
            "evd-older",
            "fixture-report-verifier",
            "fixture-report-locator",
            1,
            b"older bytes",
        );
        let newer = state.record(
            "evd-newer",
            "fixture-report-verifier",
            "fixture-report-locator",
            2,
            b"newer bytes",
        );
        assert!(newer.event_log_offset().get() > older.event_log_offset().get());
        evidence.insert("evd-older", b"older bytes");
        evidence.insert("evd-newer", b"newer bytes");
        state.checkpoint_at(2);
        let authority =
            VerificationAuthority::new(state.clone(), evidence, Arc::new(SeededIds::new()));
        let governed = authority
            .for_run(
                &RunId::new(RUN_ULID).expect("fixture run"),
                IterationNo::new(2).expect("1-based"),
                ladder_with_test_report_rung(&["fixture-report-locator"]),
                state.workspace_root().to_owned(),
                bound(),
            )
            .expect("the fixture run opens");
        let claim = governed
            .verify_claim(&unit_request("AK-9001"), bound())
            .expect("the chain mints a record");
        // Both candidates pass the binding checks (both are at/after the
        // governed iteration 2? older is iteration 1 — stale). The FRESHEST
        // (evd-newer) is verified.
        assert_eq!(claim.record().outcome(), Outcome::Pass);
        assert_eq!(claim.record().evidence().as_str(), "evd-newer");
    }

    #[test]
    fn when_every_candidate_fails_the_freshest_class_is_recorded() {
        let (state, evidence) = authority_parts();
        state.record(
            "evd-stale-a",
            "fixture-report-verifier",
            "fixture-report-locator",
            1,
            b"older bytes",
        );
        state.record(
            "evd-stale-b",
            "fixture-report-verifier",
            "fixture-report-locator",
            2,
            b"newer bytes",
        );
        // Both resolve — both are simply stale for a verification anchored
        // at iteration 3.
        evidence.insert("evd-stale-a", b"older bytes");
        evidence.insert("evd-stale-b", b"newer bytes");
        state.checkpoint_at(3);
        let authority =
            VerificationAuthority::new(state.clone(), evidence, Arc::new(SeededIds::new()));
        let governed = authority
            .for_run(
                &RunId::new(RUN_ULID).expect("fixture run"),
                IterationNo::new(3).expect("1-based"),
                ladder_with_test_report_rung(&[]),
                state.workspace_root().to_owned(),
                bound(),
            )
            .expect("the fixture run opens");
        let claim = governed
            .verify_claim(&unit_request("AK-9001"), bound())
            .expect("the chain mints a record");
        assert_eq!(claim.record().class(), Some(BindingClass::EvidenceStale));
        // The FRESHEST failing candidate's ref is the one named.
        assert_eq!(claim.record().evidence().as_str(), "evd-stale-b");
    }

    #[test]
    fn an_instrument_returning_another_identity_is_refused() {
        let (state, evidence) = authority_parts();
        state.record(
            "evd-good",
            "fixture-report-verifier",
            "fixture-report-locator",
            1,
            b"bytes",
        );
        evidence.insert("evd-good", b"bytes");
        // The rung reports one identity but the verifier mints records
        // under a DIFFERENT name — the chain refuses the contradiction.
        struct RenegadeVerifier;
        impl gauntlet_ports::verifier::Verifier for RenegadeVerifier {
            fn verify(
                &self,
                claim: &VerificationRequest,
                _: gauntlet_ports::deadline::Deadline,
            ) -> Result<gauntlet_domain::VerificationRecord, gauntlet_ports::verifier::VerifierError>
            {
                Ok(gauntlet_domain::VerificationRecord::new(
                    claim.id.clone(),
                    claim.run_id.clone(),
                    claim.unit_id.clone(),
                    gauntlet_domain::EvidenceRef::new(claim.evidence.as_str())
                        .expect("locator as ref"),
                    Outcome::Pass,
                    gauntlet_domain::InstrumentName::new("someone-else").expect("plain token"),
                ))
            }
            fn is_deterministic(&self) -> bool {
                true
            }
            fn instrument(&self) -> gauntlet_ports::observer::InstrumentIdentity {
                gauntlet_ports::observer::InstrumentIdentity::new(
                    gauntlet_domain::InstrumentName::new("the-rung").expect("plain token"),
                    "1".to_owned(),
                )
            }
        }
        let ladder = Ladder::new(vec![LadderRung::new(
            RungKind::TestReport,
            Arc::new(RenegadeVerifier),
        )])
        .expect("ladder");
        let authority =
            VerificationAuthority::new(state.clone(), evidence, Arc::new(SeededIds::new()));
        let governed = authority
            .for_run(
                &RunId::new(RUN_ULID).expect("fixture run"),
                IterationNo::new(1).expect("1-based"),
                ladder,
                state.workspace_root().to_owned(),
                bound(),
            )
            .expect("the fixture run opens");
        assert!(matches!(
            governed.verify_claim(&unit_request("AK-9001"), bound()),
            Err(VerificationRefusal::InstrumentContradictsDispatch { .. })
        ));
    }

    // ------------------------------------------------------------------
    // Correction round 1 — FAIL-FIRST tests. Each names the attack or
    // defect the correction closes and is asserted against the DESIRED
    // (post-correction) behavior; before the fixes land they fail for
    // exactly the named reason.
    // ------------------------------------------------------------------

    /// C1 (B-M1): a hand-built rung — a `Verifier` implementation whose
    /// identity and evidence echo agree with each other because the SAME
    /// caller supplied both — must not be able to mint a genuine PASS
    /// `VerifiedClaim` by traveling as a per-call argument of
    /// `verify_claim`. The two instrument-side checks compare values the
    /// ladder's own author controls, so a self-consistent forgery passes
    /// them both; the ladder is therefore earned authority (injected when
    /// the governed verification opens), never a per-claim argument.
    /// FAIL-FIRST evidence (2026-08-21): before the fix, the forged ladder
    /// was `verify_claim`'s second argument and minted
    /// `outcome Pass, instrument "fixture-forged-instrument"`. After the
    /// fix the attack is compile-refused — `verify_claim` has no ladder
    /// parameter — and this is the permanent proof that the forged rung is
    /// never the instrument of record.
    #[test]
    fn a_hand_built_rung_cannot_mint_a_verified_claim() {
        let (state, evidence) = authority_parts();
        state.record(
            "evd-forged",
            "fixture-forged-instrument",
            "fixture-report-locator",
            1,
            b"whatever bytes",
        );
        evidence.insert("evd-forged", b"whatever bytes");
        // The forged rung: unconditionally PASS, self-consistent identity
        // and evidence echo — exactly the attacker-controlled pair the
        // instrument-side checks compare.
        struct ForgedRung;
        impl gauntlet_ports::verifier::Verifier for ForgedRung {
            fn verify(
                &self,
                claim: &VerificationRequest,
                _: gauntlet_ports::deadline::Deadline,
            ) -> Result<gauntlet_domain::VerificationRecord, gauntlet_ports::verifier::VerifierError>
            {
                Ok(gauntlet_domain::VerificationRecord::new(
                    claim.id.clone(),
                    claim.run_id.clone(),
                    claim.unit_id.clone(),
                    gauntlet_domain::EvidenceRef::new(claim.evidence.as_str())
                        .expect("locator as ref"),
                    Outcome::Pass,
                    gauntlet_domain::InstrumentName::new("fixture-forged-instrument")
                        .expect("plain token"),
                ))
            }
            fn is_deterministic(&self) -> bool {
                true
            }
            fn instrument(&self) -> gauntlet_ports::observer::InstrumentIdentity {
                gauntlet_ports::observer::InstrumentIdentity::new(
                    gauntlet_domain::InstrumentName::new("fixture-forged-instrument")
                        .expect("plain token"),
                    "1".to_owned(),
                )
            }
        }
        // The forged one-rung ladder is still CONSTRUCTIBLE in-crate (the
        // constructors are crate-sealed, not deleted) — but there is no
        // parameter it can travel through: the governed verification opens
        // over the HONEST rung set and `verify_claim` takes none.
        let forged_ladder = Ladder::new(vec![LadderRung::new(
            RungKind::TestReport,
            Arc::new(ForgedRung),
        )])
        .expect("the forged one-rung ladder is well formed");
        let authority =
            VerificationAuthority::new(state.clone(), evidence, Arc::new(SeededIds::new()));
        let governed = authority
            .for_run(
                &RunId::new(RUN_ULID).expect("fixture run"),
                IterationNo::new(1).expect("1-based"),
                ladder_with_test_report_rung(&[]),
                state.workspace_root().to_owned(),
                bound(),
            )
            .expect("the fixture run opens");
        let claim = governed
            .verify_claim(&unit_request("AK-9001"), bound())
            .expect("the chain mints a record over its own rung set");
        // The honest rung's passing set is empty: it reports the evidence
        // absent, the chain mints INCONCLUSIVE — and the forged instrument
        // is nowhere on the record.
        assert_eq!(claim.record().outcome(), Outcome::Inconclusive);
        assert_ne!(
            claim.record().instrument().name(),
            "fixture-forged-instrument",
            "the forged rung was never dispatched: there is no ladder parameter on the mint"
        );
        let _ = forged_ladder; // the attack value exists and is inert
    }

    /// C3 (B-M4): the instrument examines the LOCATOR bytes while the
    /// record and the pin name the STORE ref — nothing binds them. Store
    /// bytes that are not the content the production recorded must be
    /// refused, whoever serves them (the `ObservationBinding` content-digest
    /// discipline, mirrored on the production surface). FAIL-FIRST
    /// evidence (2026-08-21): before the fix, the swapped bytes certified a
    /// PASS over the production's ref.
    #[test]
    fn store_bytes_that_are_not_the_recorded_content_are_refused() {
        let (state, evidence) = authority_parts();
        // The producer's artifact at the locator (what the instrument
        // examines): a passing report, whose digest the production records.
        // The store ref, however, holds DIFFERENT bytes — the swapped-bytes
        // attack.
        let artifact = b"{\"tests\":7,\"failures\":0}";
        let swapped = b"{\"tests\":0,\"failures\":99}";
        state.record(
            "evd-swapped",
            "fixture-report-verifier",
            "fixture-report-locator",
            1,
            artifact,
        );
        evidence.insert("evd-swapped", swapped);
        // The instrument-side rung sees the LOCATOR in its passing set, so
        // without the content binding the swapped bytes certify a PASS.
        let authority =
            VerificationAuthority::new(state.clone(), evidence, Arc::new(SeededIds::new()));
        let governed = authority
            .for_run(
                &RunId::new(RUN_ULID).expect("fixture run"),
                IterationNo::new(1).expect("1-based"),
                ladder_with_test_report_rung(&["fixture-report-locator"]),
                state.workspace_root().to_owned(),
                bound(),
            )
            .expect("the fixture run opens");
        match governed.verify_claim(&unit_request("AK-9001"), bound()) {
            Err(VerificationRefusal::EvidenceContentMismatch {
                evidence,
                recorded,
                served,
            }) => {
                assert_eq!(evidence.as_str(), "evd-swapped");
                assert_eq!(recorded, sha256_digest(artifact));
                assert_eq!(served, sha256_digest(swapped));
            }
            other => panic!(
                "swapped store bytes must be a typed refusal naming the digests, got {:?}",
                other.as_ref().map(|claim| claim.record().outcome())
            ),
        }
    }

    /// F1 (MAJOR, confirmation round 1): the C3 content binding covered
    /// only the STORE side — the dispatched instrument examines the
    /// LOCATOR, a mutable workspace file, and nothing bound those bytes.
    /// The executed attack: an honest producer records and stores FAILING
    /// bytes (store digest == recorded digest, so C3 is satisfied), then a
    /// builder-class write (§22.2) doctors the workspace file at the
    /// locator into PASSING bytes — pre-correction the doctored file rode
    /// through to a PASS over pinned FAILING bytes. The chain now re-reads
    /// the locator under the governed workspace root BEFORE dispatch and
    /// requires `digest(locator bytes) == production.content() ==
    /// digest(store bytes)`; a mismatch is the typed
    /// `LocatorContentMismatch` refusal, never an outcome. FAIL-FIRST
    /// evidence (2026-08-21, pristine HEAD, the REAL adapter — captured in
    /// correction-round2.md): the attack minted `Pass` and the settlement
    /// precondition GRANTED readiness over the pinned FAILING bytes.
    #[test]
    fn a_doctored_workspace_file_cannot_ride_the_locator_to_a_pass() {
        let (state, evidence) = authority_parts();
        // The honest producer: failing bytes ingested at the locator,
        // stored, and recorded (record writes the artifact too — one fact
        // in all three places).
        let failing = b"{\"tests\":1,\"failures\":1}";
        let doctored = b"{\"tests\":7,\"failures\":0}";
        state.record(
            "evd-locator-doctored",
            "fixture-report-verifier",
            "fixture-report-locator",
            1,
            failing,
        );
        evidence.insert("evd-locator-doctored", failing);
        // The builder-class write: the mutable workspace file at the
        // locator is doctored into a passing report. The store side stays
        // perfectly honest — every durable column tells the truth.
        std::fs::write(
            state.workspace_root().join("fixture-report-locator"),
            doctored,
        )
        .expect("the doctored workspace file is written");
        // The instrument's passing set names the locator: without the
        // locator half of the binding, the doctored file certifies a PASS.
        let authority =
            VerificationAuthority::new(state.clone(), evidence, Arc::new(SeededIds::new()));
        let governed = authority
            .for_run(
                &RunId::new(RUN_ULID).expect("fixture run"),
                IterationNo::new(1).expect("1-based"),
                ladder_with_test_report_rung(&["fixture-report-locator"]),
                state.workspace_root().to_owned(),
                bound(),
            )
            .expect("the fixture run opens");
        match governed.verify_claim(&unit_request("AK-9001"), bound()) {
            Err(VerificationRefusal::LocatorContentMismatch {
                locator,
                recorded,
                examined,
            }) => {
                assert_eq!(locator.as_str(), "fixture-report-locator");
                assert_eq!(recorded, sha256_digest(failing));
                assert_eq!(examined, sha256_digest(doctored));
            }
            other => panic!(
                "a doctored workspace file must be a typed locator-content refusal naming the \
                 digests BEFORE dispatch, got {:?}",
                other.as_ref().map(|claim| claim.record().outcome())
            ),
        }
    }

    /// C5 (B-M2, first half): the governed iteration is the loop pass whose
    /// transform the evidence must certify. A caller anchoring at an
    /// iteration the run's durable checkpoint contradicts (an old anchor
    /// re-freshens prior-iteration evidence; a future anchor certifies a
    /// pass that never happened) must be a typed refusal — the anchor has
    /// to be supported by durable state. FAIL-FIRST evidence
    /// (2026-08-21): before the fix, both anchors opened.
    #[test]
    fn an_iteration_no_durable_checkpoint_supports_is_refused() {
        let (state, evidence) = authority_parts();
        state.record(
            "evd-old-anchor",
            "fixture-report-verifier",
            "fixture-report-locator",
            1,
            b"prior-iteration bytes",
        );
        evidence.insert("evd-old-anchor", b"prior-iteration bytes");
        // The run's durable state has crossed into iteration 4.
        state.checkpoint_at(4);
        let authority =
            VerificationAuthority::new(state.clone(), evidence, Arc::new(SeededIds::new()));
        // The attack: anchor at iteration 1 so the iteration-1 production
        // is "fresh" again. Durable state says the run stands at iteration 4.
        assert!(matches!(
            authority.for_run(
                &RunId::new(RUN_ULID).expect("fixture run"),
                IterationNo::new(1).expect("1-based"),
                ladder_with_test_report_rung(&[]),
                state.workspace_root().to_owned(),
                bound()
            ),
            Err(VerificationAdmissionError::IterationUnsupported {
                durable,
                requested,
                ..
            }) if durable.get() == 4 && requested.get() == 1
        ));
        // A future anchor is equally unsupported.
        assert!(matches!(
            authority.for_run(
                &RunId::new(RUN_ULID).expect("fixture run"),
                IterationNo::new(5).expect("1-based"),
                ladder_with_test_report_rung(&[]),
                state.workspace_root().to_owned(),
                bound()
            ),
            Err(VerificationAdmissionError::IterationUnsupported {
                durable,
                requested,
                ..
            }) if durable.get() == 4 && requested.get() == 5
        ));
        // The durable anchor itself opens (the coordinate is validated,
        // not forbidden).
        assert!(
            authority
                .for_run(
                    &RunId::new(RUN_ULID).expect("fixture run"),
                    IterationNo::new(4).expect("1-based"),
                    ladder_with_test_report_rung(&[]),
                    state.workspace_root().to_owned(),
                    bound()
                )
                .is_ok()
        );
    }

    /// C6.1 (A-m5/B-m4, convergent): an evidence-store READ FAILURE during
    /// the resolvability check is an infrastructure failure (§21
    /// persistence locus), not a §14.3 binding class on a record — the
    /// unanswerable must stay unanswerable (typed refusal), never silently
    /// degrade into an answerable class. FAIL-FIRST evidence
    /// (2026-08-21): before the fix, the read failure minted
    /// INCONCLUSIVE/`evidence_unresolvable`.
    #[test]
    fn an_evidence_store_read_failure_is_a_typed_refusal_not_a_class() {
        let (state, evidence) = authority_parts();
        state.record(
            "evd-unreadable",
            "fixture-report-verifier",
            "fixture-report-locator",
            1,
            b"present but unservable bytes",
        );
        evidence.insert("evd-unreadable", b"present but unservable bytes");
        let authority =
            VerificationAuthority::new(state.clone(), evidence.clone(), Arc::new(SeededIds::new()));
        let governed = authority
            .for_run(
                &RunId::new(RUN_ULID).expect("fixture run"),
                IterationNo::new(1).expect("1-based"),
                ladder_with_test_report_rung(&[]),
                state.workspace_root().to_owned(),
                bound(),
            )
            .expect("the fixture run opens");
        // The store becomes unservable AFTER the governed open (its frozen
        // reference already loaded) — the binding check's read is what fails.
        evidence.fail_reads();
        match governed.verify_claim(&unit_request("AK-9001"), bound()) {
            Err(VerificationRefusal::Evidence { detail }) => {
                assert!(
                    detail.contains("evd-unreadable"),
                    "the refusal names the evidence: {detail}"
                );
            }
            Ok(claim) => panic!(
                "a store read failure must be a typed refusal, not an outcome; got {:?} with \
                 class {:?}",
                claim.record().outcome(),
                claim.record().class()
            ),
            Err(other) => panic!("expected the evidence refusal, got {other:?}"),
        }
    }

    /// C6.2 (A-m3/B-m1, convergent): the §14.3 locator check is the
    /// app-layer defense against a store whose by-locator query leaks
    /// productions recorded at OTHER locators (an adapter regression would
    /// otherwise defeat the condition with no failing check here). The
    /// tooth: a contract-violating store surfaces off-locator productions
    /// and the check bites — INCONCLUSIVE/unresolvable, never PASS.
    #[test]
    fn the_locator_check_bites_a_store_that_leaks_off_locator_productions() {
        let (state, evidence) = authority_parts();
        // A production recorded at ANOTHER locator (honest stores never
        // return it for this lookup; the leaking fixture does).
        state.record(
            "evd-elsewhere",
            "fixture-report-verifier",
            "fixture-elsewhere-locator",
            1,
            b"produced elsewhere",
        );
        evidence.insert("evd-elsewhere", b"produced elsewhere");
        state.leak_off_locator();
        let authority =
            VerificationAuthority::new(state.clone(), evidence, Arc::new(SeededIds::new()));
        let governed = authority
            .for_run(
                &RunId::new(RUN_ULID).expect("fixture run"),
                IterationNo::new(1).expect("1-based"),
                ladder_with_test_report_rung(&["fixture-elsewhere-locator"]),
                state.workspace_root().to_owned(),
                bound(),
            )
            .expect("the fixture run opens");
        let claim = governed
            .verify_claim(&unit_request("AK-9001"), bound())
            .expect("the chain mints a record");
        assert_eq!(
            claim.record().outcome(),
            Outcome::Inconclusive,
            "evidence at another locator is not the required evidence"
        );
        assert_eq!(
            claim.record().class(),
            Some(BindingClass::EvidenceUnresolvable)
        );
        assert!(
            claim.record().detail().contains("another locator"),
            "the detail names the locator mapping: {}",
            claim.record().detail()
        );
    }
}
