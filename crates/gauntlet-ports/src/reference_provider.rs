//! `ReferenceProvider` — spec §6.2: "`resolve(intent) -> ReferenceDraft` ·
//! `normalize` · `digest` · `freeze(run_id) -> ReferenceVersion` ·
//! `verify_unchanged(version)` — `verify_unchanged` is the I1 tripwire;
//! called before settlement and at every iteration boundary."
//!
//! The pipeline: the operator's intent resolves to a draft of the reference
//! (repo artifacts loaded, not yet validated); normalization builds the
//! domain [`Reference`] with its invariants; the digest covers the
//! normalized form; freeze binds one version to a run; `verify_unchanged`
//! re-derives the digest and fails loudly if the active system has moved.
//! "The builder never modifies the standard its work is evaluated against.
//! Freeze, digest, re-verify" (AGENTS.md, Architecture).

use core::fmt;

use gauntlet_domain::{
    Digest, Reference, ReferenceDefinition, ReferenceError, ReferenceVersion, RunId,
};

use crate::deadline::Deadline;

/// `ReferenceProvider` failures — typed, with the pipeline stage as locus.
#[derive(Debug, Clone, PartialEq)]
pub enum ReferenceProviderError {
    /// The intent could not be resolved to any reference draft (missing
    /// artifacts, unreadable sources).
    ResolveFailed { detail: String },
    /// The draft failed normalization into a domain `Reference`.
    NormalizeFailed { cause: ReferenceError },
    /// The normalized form could not be digested.
    DigestFailed { detail: String },
    /// Freezing a version for the run failed.
    FreezeFailed { detail: String },
    /// I1 tripwire: the active reference's digest no longer matches the
    /// frozen version. The run is invalid; never retried silently (§11.5:
    /// "Reference digest mismatch | Abort the run; do not retry").
    ReferenceMutated { expected: Digest, actual: Digest },
    /// The provider could not reach its backing artifacts.
    Unavailable { detail: String },
}

impl fmt::Display for ReferenceProviderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ResolveFailed { detail } => {
                write!(f, "resolving the reference failed: {detail}")
            }
            Self::NormalizeFailed { cause } => {
                write!(f, "the reference draft does not normalize: {cause}")
            }
            Self::DigestFailed { detail } => {
                write!(f, "digesting the normalized reference failed: {detail}")
            }
            Self::FreezeFailed { detail } => {
                write!(f, "freezing the reference version failed: {detail}")
            }
            Self::ReferenceMutated { expected, actual } => write!(
                f,
                "the reference has moved: frozen digest {expected}, current digest {actual} — \
                 abort the run, do not retry (I1)"
            ),
            Self::Unavailable { detail } => {
                write!(f, "reference artifacts are unavailable: {detail}")
            }
        }
    }
}

impl std::error::Error for ReferenceProviderError {}

/// A resolved-but-unnormalized reference (spec §6.2 `ReferenceDraft`): the
/// artifacts as loaded, before the domain's constructor-level invariants
/// have been enforced by `normalize`.
#[derive(Debug, Clone)]
pub struct ReferenceDraft {
    sources: Vec<String>,
    definition: ReferenceDefinition,
}

impl ReferenceDraft {
    pub fn new(sources: Vec<String>, definition: ReferenceDefinition) -> Self {
        Self {
            sources,
            definition,
        }
    }

    /// The artifact locators the draft was resolved from (§5.3.1
    /// `source_refs`).
    pub fn sources(&self) -> &[String] {
        &self.sources
    }

    pub fn definition(&self) -> &ReferenceDefinition {
        &self.definition
    }

    pub fn into_definition(self) -> ReferenceDefinition {
        self.definition
    }
}

/// The reference port (spec §6.2). Every method performs artifact IO, so
/// every method carries a deadline (§6 preamble).
pub trait ReferenceProvider {
    /// Resolves the operator's intent (verbatim, §5.3.3) to a reference
    /// draft.
    fn resolve(
        &self,
        intent: &str,
        deadline: Deadline,
    ) -> Result<ReferenceDraft, ReferenceProviderError>;

    /// Normalizes a draft into a domain `Reference`, enforcing the §5.3.1
    /// invariants.
    fn normalize(
        &self,
        draft: ReferenceDraft,
        deadline: Deadline,
    ) -> Result<Reference, ReferenceProviderError>;

    /// Digests the normalized reference. The algorithm is configuration;
    /// the digest carries it.
    fn digest(
        &self,
        reference: &Reference,
        deadline: Deadline,
    ) -> Result<Digest, ReferenceProviderError>;

    /// Freezes the reference as the run's one version for its whole life
    /// (spec §5.3.3: "a `Run` has exactly one frozen `reference_version`
    /// for its whole life").
    fn freeze(
        &self,
        run_id: &RunId,
        reference: &Reference,
        deadline: Deadline,
    ) -> Result<ReferenceVersion, ReferenceProviderError>;

    /// The I1 tripwire (spec §6.2): re-derives the current digest and
    /// compares. Ok means unchanged; [`ReferenceProviderError::ReferenceMutated`]
    /// invalidates the run.
    fn verify_unchanged(
        &self,
        version: &ReferenceVersion,
        deadline: Deadline,
    ) -> Result<(), ReferenceProviderError>;
}
