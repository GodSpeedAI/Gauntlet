//! The application-owned comparison admission service (plan Task 10; spec
//! §13.2, §13.5, §18.1).
//!
//! This module is the **only** route from a durable run to a consequential
//! comparison. It exists because Task 10's earlier shape modelled provenance as
//! value validation at each boundary — every check compared two values the
//! caller already held — so a caller could assemble an envelope, mint a
//! "trusted" context, run a comparator, and persist a record without ever
//! touching durable run state. See
//! `.agents/reports/task10/architecture-diagnosis.md`.
//!
//! The chain here is a consuming typestate. Each step is the operation that
//! earns the next state, and no state has a public constructor:
//!
//! ```text
//! ComparisonAuthority::for_run  -> GovernedRun        (durable StateStore::load_run)
//! GovernedRun::issue            -> IssuedComparison   (bundle reopened from evidence)
//! IssuedComparison::review      -> CompletedReview    (the only dispatch)
//! CompletedReview::accept       -> AcceptedComparison (the only consequence)
//! ```
//!
//! `AcceptedComparison` is the only value downstream services may treat as a
//! comparison that happened. It cannot be constructed anywhere else, and
//! `compare::review` / `compare::aggregate` / `compare::envelope::build_envelope`
//! are crate-private so no alternate route to it can be written.
//!
//! **Residual (recorded, not claimed closed):** the `EvidenceStore` port has no
//! pin-membership query, so `issue` binds the reopened bundle by its
//! content-addressed ref plus its declared run/iteration rather than by proving
//! pin membership. Adding such a query is a port change outside Task 10's step
//! list.

use std::sync::Arc;

use gauntlet_domain::{EvidenceKind, ReviewMode};
use gauntlet_domain::{IterationNo, ReferenceVersion, Residual, RunId};
use gauntlet_ports::comparator::{
    ApplicableConstraint, ComparatorError, ComparisonEnvelope, FrozenReferenceProjection,
    MandatoryComparisonAcceptance, RunComparisonAuthority, TrustedComparisonContext,
};
use gauntlet_ports::comparison_record::ComparisonRecordDoc;
use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::evidence_store::EvidenceStore;
use gauntlet_ports::state_store::StateStore;

use crate::observation::bundle::ObservationBundleDoc;

use super::contract::ComparisonContract;
use super::envelope::{BuiltEnvelope, EnvelopeError, EnvelopeInputs, build_envelope};
use super::review::{ModelDispatchBudget, ReviewDeps, ReviewError, ReviewOutcome, review};

/// Admission failures for the governed chain (spec §21.2: typed, never a
/// silently degraded success).
#[derive(Debug)]
pub enum AdmissionError {
    /// The run is not durable, or its frozen reference could not be loaded and
    /// verified against the run's own pinned digest.
    AuthorityRefused(ComparatorError),
    /// The named observation evidence is absent, unreadable, or is not this
    /// run's bundle at this iteration.
    ObservationRefused { detail: String },
    /// Envelope construction refused the inputs (digest recompute, structural
    /// blindness, trusted dispatch).
    EnvelopeRefused(EnvelopeError),
    /// The review itself failed. Uncertainty is never converted to success.
    ReviewRefused(ReviewError),
    /// Durable acceptance refused or failed.
    AcceptanceRefused(ComparatorError),
}

impl core::fmt::Display for AdmissionError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::AuthorityRefused(error) => {
                write!(f, "comparison authority refused: {error}")
            }
            Self::ObservationRefused { detail } => {
                write!(f, "the observation bundle was refused: {detail}")
            }
            Self::EnvelopeRefused(error) => write!(f, "envelope construction refused: {error}"),
            Self::ReviewRefused(error) => write!(f, "the governed review failed: {error}"),
            Self::AcceptanceRefused(error) => {
                write!(f, "durable comparison acceptance refused: {error}")
            }
        }
    }
}

impl std::error::Error for AdmissionError {}

/// The composition-root-injected substrate. Substituting these adapters is the
/// one remaining way to influence what the chain trusts — and that is spec
/// §7.1 adapter selection at an audited seam, not a public bypass.
pub struct ComparisonAuthority {
    state: Arc<dyn StateStore>,
    evidence: Arc<dyn EvidenceStore>,
}

impl ComparisonAuthority {
    pub fn new(state: Arc<dyn StateStore>, evidence: Arc<dyn EvidenceStore>) -> Self {
        Self { state, evidence }
    }

    /// Opens the governed chain for one durable run at one iteration. The run
    /// must already be durable, and its own frozen `ReferenceVersion` — never a
    /// caller-supplied one — names the reference evidence that is loaded and
    /// verified here.
    pub fn for_run(
        &self,
        run_id: &RunId,
        iteration_no: IterationNo,
        deadline: Deadline,
    ) -> Result<GovernedRun, AdmissionError> {
        let authority =
            RunComparisonAuthority::open(self.state.as_ref(), run_id, iteration_no, deadline)
                .map_err(AdmissionError::AuthorityRefused)?;
        let projection =
            FrozenReferenceProjection::load(&authority, self.evidence.as_ref(), deadline)
                .map_err(AdmissionError::AuthorityRefused)?;
        Ok(GovernedRun {
            state: self.state.clone(),
            evidence: self.evidence.clone(),
            authority,
            projection,
        })
    }
}

/// A run whose durable identity and frozen reference are established. Not
/// `Clone`: authority is earned per (run, iteration), not copied around.
pub struct GovernedRun {
    state: Arc<dyn StateStore>,
    evidence: Arc<dyn EvidenceStore>,
    authority: RunComparisonAuthority,
    projection: FrozenReferenceProjection,
}

impl GovernedRun {
    /// The run's frozen reference version, as recorded durably.
    pub fn reference(&self) -> &ReferenceVersion {
        self.authority.reference()
    }

    /// Mints the single dispatch capability for this run/iteration.
    ///
    /// **There is no observation argument.** The bundle compared over is the
    /// one the run durably bound at this iteration
    /// ([`StateStore::observation_binding`], resolved when the authority was
    /// opened). A second bundle that is individually valid, correctly hashed,
    /// pinned, and even self-declares this run and iteration has no parameter
    /// to travel through: substitution is unrepresentable rather than
    /// detected. Reopening those bound bytes here also means the caller never
    /// supplies observation CONTENT, only the run it is comparing.
    pub fn issue(
        self,
        contract: &ComparisonContract,
        constraints: Vec<ApplicableConstraint>,
        open_residuals: Vec<Residual>,
        deadline: Deadline,
    ) -> Result<IssuedComparison, AdmissionError> {
        let bundle_ref = self.authority.observation_ref().clone();
        let raw = self
            .evidence
            .get(&bundle_ref, deadline)
            .map_err(|error| AdmissionError::ObservationRefused {
                detail: format!("reading observation evidence {bundle_ref} failed: {error}"),
            })?
            .ok_or_else(|| AdmissionError::ObservationRefused {
                detail: format!("the durably bound observation evidence {bundle_ref} is absent"),
            })?;
        let text =
            std::str::from_utf8(&raw).map_err(|error| AdmissionError::ObservationRefused {
                detail: format!(
                    "observation evidence is not UTF-8 at {}",
                    error.valid_up_to()
                ),
            })?;
        let bundle = ObservationBundleDoc::decode(text).map_err(|error| {
            AdmissionError::ObservationRefused {
                detail: format!("observation evidence does not decode as a bundle: {error}"),
            }
        })?;
        let built = build_envelope(EnvelopeInputs {
            authority: &self.authority,
            frozen_reference: &self.projection,
            bundle: &bundle,
            evidence: self.evidence.as_ref(),
            deadline,
            contract,
            constraints,
            open_residuals,
        })
        .map_err(AdmissionError::EnvelopeRefused)?;
        Ok(IssuedComparison {
            state: self.state,
            evidence: self.evidence,
            authority: self.authority,
            built,
        })
    }
}

/// A minted, single-use dispatch capability. `review` consumes it, so one
/// issued comparison can be dispatched exactly once; and the value is not
/// `Clone`, so it cannot be replayed at a later iteration.
pub struct IssuedComparison {
    state: Arc<dyn StateStore>,
    evidence: Arc<dyn EvidenceStore>,
    authority: RunComparisonAuthority,
    built: BuiltEnvelope,
}

impl IssuedComparison {
    /// The envelope digest recorded on both the `RoleInvocation` and every
    /// `ComparisonRecord` (spec §13.2).
    pub fn envelope_digest(&self) -> &gauntlet_domain::Digest {
        self.built.envelope_digest()
    }

    /// The canonical envelope bytes the digest covers — P2's structural
    /// assertion runs over exactly these.
    pub fn canonical_bytes(&self) -> &str {
        self.built.canonical_bytes()
    }

    /// The observational envelope shape. Read-only: it is not dispatchable.
    pub fn envelope(&self) -> &ComparisonEnvelope {
        self.built.envelope()
    }

    fn context(&self) -> &TrustedComparisonContext {
        self.built.trusted_context()
    }

    /// The only comparison dispatch. Consumes the capability and retains the
    /// exact contributor set that ran — acceptance later works over that
    /// retained set, never over a caller-supplied slice, so presenting a
    /// friendly subset of a high-assurance review is unrepresentable.
    pub fn review(
        self,
        deps: &ReviewDeps<'_>,
        mode: ReviewMode,
        budget: ModelDispatchBudget,
        deadline: Deadline,
    ) -> Result<CompletedReview, AdmissionError> {
        // The acceptance boundary is the service's own, built from the same
        // injected substrate the authority was opened against. A caller cannot
        // pass a different one in `deps`.
        let acceptance =
            MandatoryComparisonAcceptance::new(self.state.clone(), self.evidence.clone());
        let outcome = review(self.context(), deps, &acceptance, mode, budget, deadline)
            .map_err(AdmissionError::ReviewRefused)?;
        Ok(CompletedReview {
            evidence: self.evidence,
            authority: self.authority,
            outcome,
        })
    }
}

/// A dispatched review whose contributor set is retained. Not consequential
/// yet: nothing durable has been written for the comparison.
pub struct CompletedReview {
    evidence: Arc<dyn EvidenceStore>,
    authority: RunComparisonAuthority,
    outcome: ReviewOutcome,
}

impl CompletedReview {
    /// The in-process outcome, for inspection before acceptance.
    pub fn outcome(&self) -> &ReviewOutcome {
        &self.outcome
    }

    /// Makes the review consequential (spec §18.1: comparisons are minimum
    /// durable records).
    ///
    /// Model contributors were already admitted individually through
    /// [`MandatoryComparisonAcceptance`] during collection — that path re-checks
    /// the governed authority carried by the dispatch capability, the admitted
    /// invocation tuple, and the durable run's frozen reference digest. This
    /// step makes the merged aggregate durable: pinned evidence plus a run
    /// event, over the contributor set this review RETAINED. There is no
    /// parameter for a contributor slice, so accepting a friendly subset of a
    /// high-assurance review is unrepresentable.
    ///
    /// A fully truncated review has no aggregate; it is accepted as its
    /// recorded gaps and nothing else (I8 — never a fabricated record).
    ///
    /// **Residual (recorded):** deterministic-probe contributors have no
    /// `RoleInvocation` to bind to, so they reach durability only through the
    /// aggregate's pinned evidence, not through the invocation-bound tuple.
    /// The aggregate itself is durable as pinned, content-addressed evidence;
    /// a dedicated `RunEventKind` for it would be a ports/export change beyond
    /// this correction's surface.
    pub fn accept(self, deadline: Deadline) -> Result<AcceptedComparison, AdmissionError> {
        if let Some(aggregate) = self.outcome.record() {
            let evidence = self
                .evidence
                .put(
                    aggregate.to_json().as_bytes(),
                    EvidenceKind::Transcript,
                    deadline,
                )
                .map_err(|error| {
                    AdmissionError::AcceptanceRefused(ComparatorError::ComparisonFailed {
                        detail: format!("persisting the accepted aggregate failed: {error}"),
                    })
                })?;
            self.evidence
                .pin(self.authority.run_id(), &evidence, deadline)
                .map_err(|error| {
                    AdmissionError::AcceptanceRefused(ComparatorError::ComparisonFailed {
                        detail: format!("pinning the accepted aggregate failed: {error}"),
                    })
                })?;
        }
        Ok(AcceptedComparison {
            outcome: self.outcome,
        })
    }
}

/// The only value in the system that means "a comparison happened and counts".
/// It has no constructor outside this module, so no verification, controller,
/// or settlement consumer can be handed a fabricated one.
pub struct AcceptedComparison {
    outcome: ReviewOutcome,
}

impl AcceptedComparison {
    /// The accepted aggregate record; `None` exactly when every dimension was
    /// truncated (the outcome is then its recorded gaps alone).
    pub fn record(&self) -> Option<&ComparisonRecordDoc> {
        self.outcome.record()
    }

    /// The accepted contributor documents, in the order they were dispatched.
    pub fn contributor_records(&self) -> &[ComparisonRecordDoc] {
        self.outcome.contributor_records()
    }

    /// Dimensions dropped by budget truncation (recorded, never omitted).
    pub fn unavailable_evidence(&self) -> &[super::review::UnavailableEvidence] {
        self.outcome.unavailable_evidence()
    }
}
