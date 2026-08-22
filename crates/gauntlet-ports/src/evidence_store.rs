//! `EvidenceStore` — spec §6.2: "`put(bytes, kind) -> EvidenceRef`
//! (content-addressed) · `get` · `pin(run_id, ref)` — Immutable. No
//! overwrite. Pinned evidence survives cleanup."
//!
//! The API shape IS the immutability invariant: there is no update, no
//! delete, and no way to choose a ref — content addressing assigns it, so
//! "same bytes ⇒ same ref" is the only reachable outcome and an overwrite
//! is unrepresentable. Pinning marks evidence to survive later cleanup.

use core::fmt;

use gauntlet_domain::{EvidenceKind, EvidenceRef, RunId};

use crate::deadline::Deadline;

/// `EvidenceStore` failures — typed (spec §21.2).
#[derive(Debug, Clone, PartialEq)]
pub enum EvidenceStoreError {
    /// Writing the evidence failed.
    WriteFailed { detail: String },
    /// Reading evidence failed for a reason other than absence.
    ReadFailed { detail: String },
    /// The ref does not exist in the store.
    NotFound { reference: EvidenceRef },
    /// The pin operation failed.
    PinFailed {
        reference: EvidenceRef,
        detail: String,
    },
}

impl fmt::Display for EvidenceStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WriteFailed { detail } => write!(f, "evidence write failed: {detail}"),
            Self::ReadFailed { detail } => write!(f, "evidence read failed: {detail}"),
            Self::NotFound { reference } => {
                write!(f, "evidence {reference} is not in the store")
            }
            Self::PinFailed { reference, detail } => {
                write!(f, "pinning evidence {reference} failed: {detail}")
            }
        }
    }
}

impl std::error::Error for EvidenceStoreError {}

/// The immutable evidence port (spec §6.2). Every method performs IO, so
/// every method carries a deadline (§6 preamble).
pub trait EvidenceStore {
    /// Stores bytes content-addressed and returns the ref the content
    /// assigned. Putting the same bytes again returns the same ref; there
    /// is no overwrite because there is no other way to address a write.
    fn put(
        &self,
        bytes: &[u8],
        kind: EvidenceKind,
        deadline: Deadline,
    ) -> Result<EvidenceRef, EvidenceStoreError>;

    /// Returns the bytes at a ref, or `None` when the ref was never put.
    fn get(
        &self,
        reference: &EvidenceRef,
        deadline: Deadline,
    ) -> Result<Option<Vec<u8>>, EvidenceStoreError>;

    /// Pins evidence to a run: it survives cleanup. Idempotent; pinning an
    /// unknown ref is [`EvidenceStoreError::NotFound`].
    fn pin(
        &self,
        run_id: &RunId,
        reference: &EvidenceRef,
        deadline: Deadline,
    ) -> Result<(), EvidenceStoreError>;

    /// Whether the run pinned this ref (§16.1 condition 2: settlement
    /// requires evidence that "resolves AND IS PINNED" — a read, never the
    /// idempotent write). FAILS CLOSED by default: a store that cannot
    /// answer reports an error, and a caller enforcing the precondition
    /// must refuse, never treat unknown as pinned or unpinned (I8).
    ///
    /// Plan Task 11 addition; this also closes Task 10's recorded residual
    /// ("the EvidenceStore port has no pin-membership query").
    fn is_pinned(
        &self,
        _run_id: &RunId,
        _reference: &EvidenceRef,
        _deadline: Deadline,
    ) -> Result<bool, EvidenceStoreError> {
        Err(EvidenceStoreError::ReadFailed {
            detail: "this evidence store does not implement pin-membership queries".to_owned(),
        })
    }
}
