//! `EventLog` — spec §6.2: "`append(event) -> Offset` · `read_from(offset)` ·
//! `last_offset()` — Append-only, fsync-on-append, the durability spine of
//! §16.3."
//!
//! Offsets are monotonic: every append returns an offset strictly greater
//! than every offset returned before it. `read_from` replays entries at or
//! after the given offset in append order; an offset past the last entry
//! replays nothing (an empty replay, not an error). The logged type is the
//! shared [`RunEvent`] vocabulary from [`crate::state_store`].

use core::fmt;

use gauntlet_domain::EventLogOffset;

use crate::deadline::Deadline;
use crate::state_store::RunEvent;

/// `EventLog` failures — typed (spec §21.2).
#[derive(Debug, Clone, PartialEq)]
pub enum EventLogError {
    /// A durable append failed. The event was NOT recorded — the caller
    /// must not proceed as if it were (§16.3 ordering).
    AppendFailed { detail: String },
    /// A replay read failed.
    ReadFailed { detail: String },
    /// The log exists but a span of it does not decode; recovery halts
    /// rather than silently skipping entries.
    Damaged { detail: String },
}

impl fmt::Display for EventLogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AppendFailed { detail } => {
                write!(f, "event log append failed: {detail}")
            }
            Self::ReadFailed { detail } => {
                write!(f, "event log read failed: {detail}")
            }
            Self::Damaged { detail } => {
                write!(f, "the event log is damaged: {detail}")
            }
        }
    }
}

impl std::error::Error for EventLogError {}

/// One replayed log entry: the offset it was appended at, plus the event.
#[derive(Debug, Clone, PartialEq)]
pub struct LogEntry {
    pub offset: EventLogOffset,
    pub event: RunEvent,
}

/// The durability-spine port (spec §6.2). Appends are durable before they
/// return (fsync-on-append is the adapter's duty); every method carries a
/// deadline (§6 preamble).
pub trait EventLog {
    /// Appends one event durably and returns its monotonic offset.
    fn append(&self, event: RunEvent, deadline: Deadline) -> Result<EventLogOffset, EventLogError>;

    /// Replays every entry at or after `offset`, in append order.
    fn read_from(
        &self,
        offset: EventLogOffset,
        deadline: Deadline,
    ) -> Result<Vec<LogEntry>, EventLogError>;

    /// The offset of the last append; `None` when the log is empty.
    fn last_offset(&self, deadline: Deadline) -> Result<Option<EventLogOffset>, EventLogError>;
}
