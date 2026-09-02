//! The action seam (charter D3): ONE operator-action path, implemented
//! by the composition root over the settlement service. The adapter
//! knows only the command shape and the typed outcome — NO settlement
//! type crosses this seam, so the HTTP layer cannot become a second
//! settlement writer by construction.

use std::fmt;

use gauntlet_ports::deadline::Deadline;

/// The action kind an endpoint carries (`resolve` / `retry`; §20.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionKind {
    /// `POST /api/v1/units/{id}/resolve`.
    Resolve,
    /// `POST /api/v1/units/{id}/retry`.
    Retry,
}

impl ActionKind {
    /// The wire token (and the §20.3 log line's `action=` value).
    pub fn token(self) -> &'static str {
        match self {
            Self::Resolve => "resolve",
            Self::Retry => "retry",
        }
    }
}

impl fmt::Display for ActionKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.token())
    }
}

/// One operator action request: the action kind, the unit, the recorded
/// operator identity, and the operator's note (charter D3's input
/// vocabulary) plus the OPTIONAL `?run=` anchor the endpoint accepts
/// (charter D6's run-anchoring, extended to actions so a replay can
/// address its run after its ticket has resolved — a RECORDED deviation
/// from D3's exact field list, never a silent one). The identity arrives
/// as the raw header value; the composition root validates it through
/// the settlement service's typed `OperatorIdentity` constructor (D7:
/// missing/blank is a typed 4xx).
#[derive(Debug, Clone)]
pub struct ActionCommand {
    pub kind: ActionKind,
    pub unit: String,
    pub operator: String,
    pub note: String,
    pub run: Option<String>,
}

/// An accepted action: what the ONE shared path did, in durable facts.
#[derive(Debug, Clone, PartialEq)]
pub struct ActionAccepted {
    /// The run the action settled against.
    pub run_id: String,
    /// The unit that settled.
    pub unit_id: String,
    /// The escalation tickets resolved AFTER the service accepted (D10:
    /// the queue write happens only after acceptance; the tickets are
    /// named, never silent).
    pub tickets_resolved: Vec<String>,
    /// Whether THIS call performed the durable §16.3 commit (`false` =
    /// the settlement key was already durable — the idempotent re-entry
    /// shape; the re-POST of an accepted action never commits twice).
    pub committed_now: bool,
}

/// A typed refusal: the service (or the ticket lookup) refused the
/// action; nothing was written. `code` is a stable token from the
/// settlement failure vocabulary; `message` is the typed error's own
/// Display — the SAME text the CLI path surfaces (refusal parity).
#[derive(Debug, Clone, PartialEq)]
pub struct ActionRefusal {
    pub code: String,
    pub message: String,
}

/// An infrastructure failure (a store/queue/environment read failed):
/// distinct from a refusal — the action was not decided, so nothing is
/// claimed either way (missing evidence is not passing evidence).
#[derive(Debug, Clone, PartialEq)]
pub struct ActionFailure {
    pub code: String,
    pub message: String,
}

/// The outcome of one operator action through the shared path.
#[derive(Debug, Clone, PartialEq)]
pub enum ActionOutcome {
    /// The service accepted; the durable commit sequence ran.
    Accepted(ActionAccepted),
    /// The action was refused typed; the ticket stays open.
    Refused(ActionRefusal),
    /// The action could not be decided (infrastructure); nothing written.
    Failed(ActionFailure),
}

impl ActionOutcome {
    /// The §20.3 log line's `outcome=` token.
    pub fn outcome_token(&self) -> &'static str {
        match self {
            Self::Accepted(_) => "accepted",
            Self::Refused(_) => "refused",
            Self::Failed(_) => "failed",
        }
    }

    /// The refusal/failure code, for the log line and the error body.
    pub fn code(&self) -> Option<&str> {
        match self {
            Self::Accepted(_) => None,
            Self::Refused(refusal) => Some(&refusal.code),
            Self::Failed(failure) => Some(&failure.code),
        }
    }

    /// The typed message, for the log line and the error body.
    pub fn message(&self) -> Option<&str> {
        match self {
            Self::Accepted(_) => None,
            Self::Refused(refusal) => Some(&refusal.message),
            Self::Failed(failure) => Some(&failure.message),
        }
    }
}

/// The action handler: implemented ONCE, in the composition root, over
/// the settlement service's operator gate + governed verification chain
/// (charter D3/D4). The HTTP layer calls it; it never learns how the
/// settlement works.
pub trait OperatorActionHandler {
    fn perform(&self, command: ActionCommand, deadline: Deadline) -> ActionOutcome;
}

/// The D6 disclosure: how the active run was derived, served with every
/// snapshot response (`run_derivation`). The derivation is never a
/// silent guess — the surface names its rule and its sources.
pub trait SnapshotDisclosure {
    fn run_derivation(&self) -> String;
}
