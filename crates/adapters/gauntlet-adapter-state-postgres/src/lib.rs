//! Persistence adapter (spec §4.1, §18; slice E1a/E1b): `EventLog`,
//! `CapabilityLedger`, `RevisionLedger`, and `StateStore` over PostgreSQL
//! (canonical durable store) — append-only event log, migrations in-repo.
//! Database specifics live here; the application layer never sees them.
//!
//! - [`PostgresEventLog`] — the §6.2 durability spine: append-only log,
//!   INSERT and SELECT only, offsets via a counter table with serialization,
//!   damaged rows are a typed error, never a skip.
//! - [`PostgresCapabilityLedger`] — §19.1 developmental-memory and
//!   capability-register store.
//! - [`PostgresRevisionLedger`] — §19.3/§19.4 candidate-revision store.
//! - [`PostgresStateStore`] — §18 run identity, checkpoints, records (core E1b-1).
//! - Migrations are PostgreSQL dialect SQL applied in ascending order and
//!   recorded in a `schema_migrations` table; the set is append-only.
//! - Slice E1b-2/3 (settlement and evidence groups, recovery tests) deferred.
//!
//! The capability-named re-exports ([`EmbeddedEventLog`],
//! [`EmbeddedCapabilityLedger`], [`EmbeddedRevisionLedger`], [`EmbeddedStateStore`]) are the
//! handles the composition root may spell: the boundary denylist masks
//! this crate's NAME (vendor names belong in adapters, AGENTS.md Naming),
//! and a vendor-flavored type name would leave a bare vendor substring
//! after masking (spec §8.4: "the runtime names capabilities, never products").
//! The vendor-named paths remain canonical inside the adapter layer.

mod capability_ledger;
mod db;
mod encode;
mod event_log;
mod invocation_graph;
mod locator;
mod migrations;
mod revision_ledger;
mod sql;
mod state_store;

/// TEST INSTRUMENTATION: out-of-band damage injection and read-back over a
/// state instance's durable rows (see the module docs). Never a production surface.
#[doc(hidden)]
pub mod test_support;

/// The configuration token that selects this persistence adapter
/// (`state.backend` / `GAUNTLET_STATE_BACKEND`). The token is the
/// adapter's to spell (vendor names belong in adapters, AGENTS.md
/// Naming): the composition root and its tests import this constant
/// rather than carrying the vendor substring themselves — the boundary
/// denylist masks this crate's NAME in imports, never a bare vendor
/// literal outside it.
pub const STATE_BACKEND_TOKEN: &str = "postgres";

/// The capability name of [`PostgresCapabilityLedger`] for the composition
/// root — the §19.1 developmental-memory and capability-register store.
pub use capability_ledger::PostgresCapabilityLedger as EmbeddedCapabilityLedger;
/// The capability name of [`PostgresEventLog`] for the composition root —
/// see the module docs.
pub use event_log::PostgresEventLog as EmbeddedEventLog;
/// The capability name of [`PostgresRevisionLedger`] for the composition
/// root — the §19.3/§19.4 candidate-revision store.
pub use revision_ledger::PostgresRevisionLedger as EmbeddedRevisionLedger;
/// The capability name of [`PostgresStateStore`] for the composition root —
/// the §18 run state store (core group E1b-1).
pub use state_store::PostgresStateStore as EmbeddedStateStore;

pub use capability_ledger::PostgresCapabilityLedger;
/// The capability name of [`PostgresConfig`] for the composition root and its
/// tests: the state instance's connection configuration (see [`locator`]).
pub use db::PostgresConfig as StateInstanceConfig;
pub use db::{LOCAL_DEFAULT_URL, PostgresConfig};
pub use event_log::PostgresEventLog;
pub use invocation_graph::PostgresInvocationGraph;
pub use locator::{DATABASE_URL_ENV, namespace_for_path, state_exists};
pub use revision_ledger::PostgresRevisionLedger;
pub use state_store::{PostgresStateStore, RecordKind};
