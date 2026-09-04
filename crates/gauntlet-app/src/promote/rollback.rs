//! The rollback/restore surface (plan Task 23, charter D4's `rollback`
//! piece): records the restore transition append-only; the derived state
//! flips to promoted-but-rolled-back (never silently active). A rollback
//! of a never-promoted revision — or of an already-restored one — is a
//! typed refusal: there is nothing to restore.

use std::sync::Arc;

use gauntlet_domain::RevisionId;
use gauntlet_ports::clock::Clock;
use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::revision_ledger::{
    DecisionKind, RestoreRecord, RevisionEntry, RevisionLedger, RevisionLedgerError,
};

/// The restore surface over one revision ledger.
#[derive(Clone)]
pub struct RollbackService {
    ledger: Arc<dyn RevisionLedger>,
    clock: Arc<dyn Clock>,
}

/// Why a restore refused — typed.
#[derive(Debug, Clone, PartialEq)]
pub enum RollbackError {
    /// The revision has no admitted proposal.
    UnknownRevision { revision: RevisionId },
    /// Nothing to restore: the revision is not a promoted, unrestored
    /// revision.
    NothingToRestore {
        revision: RevisionId,
        current: DecisionKind,
        rolled_back: bool,
    },
    /// The ledger refused the append or a read.
    Ledger(RevisionLedgerError),
}

impl core::fmt::Display for RollbackError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UnknownRevision { revision } => {
                write!(
                    f,
                    "revision {revision} has no admitted proposal; nothing to restore"
                )
            }
            Self::NothingToRestore {
                revision,
                current,
                rolled_back,
            } => write!(
                f,
                "revision {revision} is durably {current} (rolled_back={rolled_back}); only a \
                 promoted, unrestored revision restores"
            ),
            Self::Ledger(error) => write!(f, "the restore's ledger access failed: {error}"),
        }
    }
}

impl std::error::Error for RollbackError {}

impl RollbackService {
    pub fn new(ledger: Arc<dyn RevisionLedger>, clock: Arc<dyn Clock>) -> Self {
        Self { ledger, clock }
    }

    /// Records the restore transition (append-only). The derived state
    /// flips to promoted-but-rolled-back; the restore is one-way — a
    /// second restore refuses, and the promotion's decision history is
    /// never edited.
    pub fn restore(
        &self,
        revision: &RevisionId,
        detail: String,
        deadline: Deadline,
    ) -> Result<RevisionEntry, RollbackError> {
        let entry = self
            .ledger
            .revision(revision, deadline)
            .map_err(RollbackError::Ledger)?
            .ok_or_else(|| RollbackError::UnknownRevision {
                revision: revision.clone(),
            })?;
        let current = entry.decision.unwrap_or(DecisionKind::Pending);
        if current != DecisionKind::Promoted || entry.rolled_back {
            return Err(RollbackError::NothingToRestore {
                revision: revision.clone(),
                current,
                rolled_back: entry.rolled_back,
            });
        }
        let record = RestoreRecord::new(
            revision.clone(),
            DecisionKind::Promoted,
            detail,
            self.clock.now(),
        );
        self.ledger
            .record_restore(record, deadline)
            .map_err(RollbackError::Ledger)
    }
}
