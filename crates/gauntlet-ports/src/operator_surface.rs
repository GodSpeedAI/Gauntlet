//! `OperatorSurface` — spec §6.2: "`snapshot() -> RuntimeSnapshot` ·
//! `actions()` — CLI is the reference surface. JSON API / TUI / dashboard
//! are additional adapters."
//!
//! The snapshot sections follow §20.2's priority order: "escalation queue
//! (the operator's actionable inbox) · in-flight invocations · answer-key
//! completion grouped by tier · budget burn · sandbox denials · recent
//! residuals." Actions are the §20.2 action endpoints (refresh, resolve,
//! retry), which "perform the *same* atomic commit sequence as automatic
//! paths; they never bypass it."

use core::fmt;

use gauntlet_domain::{BlastRadius, InvocationId, ResidualId, RunId, UnitId, UnitStatus};

use crate::budget_meter::Remaining;
use crate::deadline::Deadline;
use crate::human_escalation::TicketId;

/// `OperatorSurface` failures — typed (spec §21.2). Per §21.3, a surface
/// failure never crashes the run — the error is reported and the run
/// continues.
#[derive(Debug, Clone, PartialEq)]
pub enum OperatorSurfaceError {
    /// Assembling the snapshot failed.
    SnapshotFailed { detail: String },
}

impl fmt::Display for OperatorSurfaceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SnapshotFailed { detail } => {
                write!(f, "assembling the runtime snapshot failed: {detail}")
            }
        }
    }
}

impl std::error::Error for OperatorSurfaceError {}

/// One unit's projection for the answer-key completion view (§20.2:
/// completion "grouped by tier").
#[derive(Debug, Clone, PartialEq)]
pub struct UnitSnapshot {
    pub id: UnitId,
    pub status: UnitStatus,
    pub tier: BlastRadius,
}

/// The operator's view of the runtime (spec §6.2 `RuntimeSnapshot`),
/// in §20.2's priority order.
#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeSnapshot {
    /// The active run, if any.
    pub run: Option<RunId>,
    /// The escalation queue — the operator's actionable inbox.
    pub escalated: Vec<TicketId>,
    /// In-flight invocations.
    pub in_flight: Vec<InvocationId>,
    /// Answer-key completion, grouped by tier (§20.2).
    pub units: Vec<UnitSnapshot>,
    /// Budget burn, as remaining headroom per dimension.
    pub budget: Remaining,
    /// Recorded sandbox denials (§21.3: sandbox failures escalate; they are
    /// visible here).
    pub sandbox_denials: Vec<String>,
    /// Recent residuals.
    pub open_residuals: Vec<ResidualId>,
}

/// An action the operator can take (spec §20.2 action endpoints).
#[derive(Debug, Clone, PartialEq)]
pub enum OperatorAction {
    /// `POST /api/v1/refresh` — refresh the surface.
    Refresh,
    /// `units/{id}/resolve` — operator resolution of a unit.
    ResolveUnit { unit: UnitId },
    /// `units/{id}/retry` — operator-initiated retry.
    RetryUnit { unit: UnitId },
}

/// The operator port (spec §6.2). `snapshot` reads state (deadline
/// required); `actions` is a pure enumeration.
pub trait OperatorSurface {
    /// The current runtime snapshot (spec §6.2).
    fn snapshot(&self, deadline: Deadline) -> Result<RuntimeSnapshot, OperatorSurfaceError>;

    /// The actions this surface offers (spec §6.2).
    fn actions(&self) -> Vec<OperatorAction>;
}
