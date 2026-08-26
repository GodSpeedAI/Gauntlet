//! Persistence adapter (spec §4.1, §18; plan Task 6): `StateStore` and
//! `EventLog` over an embedded SQL database — append-only event log with
//! fsync-per-append, migrations in-repo, deterministic export. Database
//! specifics live here; the application layer never sees them.
//!
//! - [`SqliteEventLog`] — the §6.2 durability spine: one table, INSERT and
//!   SELECT only, `synchronous=FULL` so every append is durable before it
//!   returns, damaged rows are a typed `Damaged` error, never a skip.
//! - [`SqliteStateStore`] — run identity, §18.2 checkpoints, §16.3
//!   settlement rows, the remaining §18.1 record kinds, and the
//!   deterministic JSONL export.
//! - Migrations are embedded SQL files applied in ascending order and
//!   recorded in a `schema_migrations` table; the set is append-only.
//!
//! The `crash_helper` binary beside this crate is the crash-injection
//! instrument for the §16.3/§18.3 recovery tests: it performs scripted
//! sequences and `std::process::abort()`s at a named step, so the tests kill
//! a real process and reopen the file, rather than dropping a connection.
//!
//! The capability-named re-exports ([`EmbeddedStateStore`],
//! [`EmbeddedEventLog`]) are the handles the composition root may spell:
//! the boundary denylist masks this crate's NAME (vendor names belong in
//! adapters, AGENTS.md Naming), and a vendor-flavored type name would leave
//! a bare vendor substring after masking (spec §8.4: "the runtime names
//! capabilities, never products"; spec §18.1: "a single-file embedded
//! database"). The vendor-named paths above remain canonical inside the
//! adapter layer, where vendor names belong.

mod encode;
mod event_log;
mod migrations;
mod state_store;

/// The capability name of [`SqliteEventLog`] for the composition root —
/// see the module docs.
pub use event_log::SqliteEventLog as EmbeddedEventLog;
/// The capability name of [`SqliteStateStore`] for the composition root —
/// see the module docs.
pub use state_store::SqliteStateStore as EmbeddedStateStore;

pub use event_log::SqliteEventLog;
pub use state_store::{RecordKind, SqliteStateStore};
