//! `HumanEscalation` — spec §6.2: "`escalate(item) -> EscalationTicket` ·
//! `poll(ticket) -> EscalationState` — File-based default adapter
//! (`escalations/` + operator CLI); no dependency on a chat product."
//!
//! Escalation is a first-class outcome, not an error path (§11.6: `blocked`
//! and `escalated` terminal states require tickets to exist). The default
//! adapter is a later task; the port carries the vocabulary.

use core::fmt;
use core::str::FromStr;

use gauntlet_domain::{EpochSeconds, EvidenceRef, RunId};

use crate::deadline::Deadline;

/// An escalation ticket id — opaque (the default adapter's naming is its
/// own; the port does not constrain it beyond usability).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TicketId {
    ticket: String,
}

impl TicketId {
    pub fn new(ticket: &str) -> Result<Self, HumanEscalationError> {
        // COR-F3 (falsifier round 1, F-3): the id is an OPAQUE token, never
        // a path fragment. Any path-separator payload is refused outright,
        // and the surviving grammar is pinned to plain name components —
        // `..` (and `.`) cannot ride a ticket id into any adapter's path
        // join. Validity: non-empty, ≤128 bytes, no leading `/`, no
        // whitespace, no path separator, and every path component a plain
        // name.
        let valid = !ticket.is_empty()
            && ticket.len() <= 128
            && !ticket.starts_with('/')
            && !ticket.bytes().any(|b| b.is_ascii_whitespace())
            && !ticket.chars().any(std::path::is_separator)
            && std::path::Path::new(ticket)
                .components()
                .all(|component| matches!(component, std::path::Component::Normal(_)));
        if valid {
            Ok(Self {
                ticket: ticket.to_owned(),
            })
        } else {
            Err(HumanEscalationError::IdentityInvalid {
                identity: ticket.to_owned(),
            })
        }
    }

    pub fn as_str(&self) -> &str {
        &self.ticket
    }
}

impl fmt::Display for TicketId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.ticket)
    }
}

impl FromStr for TicketId {
    type Err = HumanEscalationError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        Self::new(input)
    }
}

/// `HumanEscalation` failures — typed (spec §21.2).
#[derive(Debug, Clone, PartialEq)]
pub enum HumanEscalationError {
    /// The ticket could not be submitted.
    SubmitFailed { detail: String },
    /// The ticket is not known to this adapter.
    TicketNotFound { ticket: TicketId },
    /// Reading the ticket's state failed.
    StateReadFailed { detail: String },
    /// A ticket id string was not usable.
    IdentityInvalid { identity: String },
}

impl fmt::Display for HumanEscalationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SubmitFailed { detail } => {
                write!(f, "submitting the escalation failed: {detail}")
            }
            Self::TicketNotFound { ticket } => {
                write!(f, "escalation ticket {ticket} is not known")
            }
            Self::StateReadFailed { detail } => {
                write!(f, "reading the escalation state failed: {detail}")
            }
            Self::IdentityInvalid { identity } => {
                write!(f, "ticket id {identity:?} is not a usable opaque token")
            }
        }
    }
}

impl std::error::Error for HumanEscalationError {}

/// What is being escalated: the run, a one-line summary for the operator's
/// queue, and the evidence carrying the full context.
#[derive(Debug, Clone)]
pub struct EscalationItem {
    pub run_id: RunId,
    pub summary: String,
    pub context: EvidenceRef,
}

/// An open ticket (spec §6.2 `EscalationTicket`).
#[derive(Debug, Clone, PartialEq)]
pub struct EscalationTicket {
    id: TicketId,
    created_at: EpochSeconds,
}

impl EscalationTicket {
    pub fn new(id: TicketId, created_at: EpochSeconds) -> Self {
        Self { id, created_at }
    }

    pub fn id(&self) -> &TicketId {
        &self.id
    }

    pub fn created_at(&self) -> EpochSeconds {
        self.created_at
    }
}

/// A ticket's state (spec §6.2 `EscalationState`): waiting on the operator,
/// or resolved by one — with the operator's identity recorded, as §20.2
/// requires for every action.
#[derive(Debug, Clone, PartialEq)]
pub enum EscalationState {
    Open,
    Resolved { operator: String, note: String },
}

impl EscalationState {
    /// Whether an operator has acted.
    pub fn is_resolved(&self) -> bool {
        matches!(self, Self::Resolved { .. })
    }
}

/// The escalation port (spec §6.2). Submitting and polling perform IO, so
/// both carry deadlines (§6 preamble).
pub trait HumanEscalation {
    /// Records an escalation and returns its ticket (spec §6.2).
    fn escalate(
        &self,
        item: EscalationItem,
        deadline: Deadline,
    ) -> Result<EscalationTicket, HumanEscalationError>;

    /// Reads a ticket's state (spec §6.2).
    fn poll(
        &self,
        ticket: &EscalationTicket,
        deadline: Deadline,
    ) -> Result<EscalationState, HumanEscalationError>;
}
