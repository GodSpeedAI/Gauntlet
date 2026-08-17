//! `Comparator` — spec §6.2: "`compare(envelope: ComparisonEnvelope) ->
//! ComparisonRecord` — Envelope is built by the runtime (I4). Comparator
//! implementations MAY be model-backed or deterministic."
//!
//! I4 (envelope construction): the runtime builds the comparison envelope
//! from an allowlist — the frozen reference's digest, the runtime-rendered
//! reference view the critic may see, and the observation bundle. No
//! builder trajectory, reasoning, identity, or attempt count is present
//! (P2 asserts this structurally). The envelope type below is exactly that
//! allowlist; adding a field to it is a boundary decision, not a refactor.

use core::fmt;

use gauntlet_domain::{ComparisonRecord, Digest, IterationNo, ObservationBundle, RunId};

use crate::deadline::Deadline;

/// `Comparator` failures — typed (spec §21.2).
#[derive(Debug, Clone, PartialEq)]
pub enum ComparatorError {
    /// The envelope is not a shape this comparator can operate on.
    EnvelopeInvalid { detail: String },
    /// The comparison itself failed (model unreachable, contract
    /// unparseable). Uncertainty is never converted into success (§21.2):
    /// a failed comparison is an error, not a pass.
    ComparisonFailed { detail: String },
}

impl fmt::Display for ComparatorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EnvelopeInvalid { detail } => {
                write!(f, "the comparison envelope is invalid: {detail}")
            }
            Self::ComparisonFailed { detail } => {
                write!(f, "the comparison failed: {detail}")
            }
        }
    }
}

impl std::error::Error for ComparatorError {}

/// The runtime-built comparison envelope (spec §6.2, invariant I4):
/// everything a fresh-context critic is allowed to see, and nothing else.
#[derive(Debug, Clone)]
pub struct ComparisonEnvelope {
    pub run_id: RunId,
    pub iteration_no: IterationNo,
    /// The run's frozen reference digest — carried so the I1 cross-check is
    /// possible at every later boundary.
    pub reference_digest: Digest,
    /// The runtime-rendered, allowlisted view of the reference the critic
    /// compares against.
    pub reference_view: String,
    /// What was actually observed (its digest couples the record, §5.3.6).
    pub observation: ObservationBundle,
    /// The comparison contract version in force (Annex B.4
    /// `comparison_contract_version`); adapters record what they ran under.
    pub contract_version: String,
}

/// The comparison port (spec §6.2). Comparators produce residuals only —
/// a comparator verdict alone never settles a Unit (AK-031, enforced at
/// settlement).
pub trait Comparator {
    /// Compares the observation against the reference view under the
    /// envelope's contract. Blocking (model-backed adapters exist), so a
    /// deadline is required (§6 preamble).
    fn compare(
        &self,
        envelope: &ComparisonEnvelope,
        deadline: Deadline,
    ) -> Result<ComparisonRecord, ComparatorError>;
}
