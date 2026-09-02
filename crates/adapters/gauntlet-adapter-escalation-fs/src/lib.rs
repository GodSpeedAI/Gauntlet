//! Filesystem escalation queue adapter (spec §4.1 `HumanEscalation`,
//! §6.2: "File-based default adapter (`escalations/` + operator CLI);
//! no dependency on a chat product"; plan Task 20): a durable queue of
//! typed escalation records under the state dir, one file per ticket.
//!
//! - `escalate(item) -> Ticket` appends a typed record (create-temp,
//!   sync, rename — the crash window never leaves a half ticket);
//! - `poll(ticket)` reads one ticket's state;
//! - [`FsEscalationQueue::list`] is the read side `gauntlet escalations
//!   list` composes from — the escalation READ lives with the escalation
//!   adapter, never with the StateStore (B2a's inventory finding);
//! - [`FsEscalationQueue::resolve`] records the operator action on a
//!   ticket. It is a QUEUE-STATE write only: settling the escalated
//!   work goes through the settlement service's operator path
//!   (§20.2/§16.3, charter D9) — this adapter is never a second writer
//!   of run state.
//!
//! Records are canonical JSON (the ports JSON value, fixed field order)
//! so a ticket file is diffable and hand-inspectable. All failures are
//! the port's typed [`HumanEscalationError`] plus the adapter's own
//! [`EscalationQueueError`] for the read/resolve surface — never a
//! panic, never a swallowed error.

pub mod queue;

pub use queue::{
    ESCALATIONS_DIR_NAME, EscalationQueueError, EscalationRecord, FsEscalationQueue,
    OperatorAction, QuarantineClass, QuarantinedTicket, QueueListing, TICKET_ID_PREFIX,
    TICKET_SCHEMA,
};

/// Capability-named façade for composition roots (the same posture as
/// the process-isolation backend's re-export).
pub use FsEscalationQueue as FileEscalationQueue;
