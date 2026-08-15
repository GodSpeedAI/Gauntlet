//! Persistence adapter (spec §4.1, §18; plan Task 6): `StateStore` and
//! `EventLog` over an embedded SQL (sqlite) database — append-only event log
//! with fsync-per-append, migrations in-repo, deterministic export. Database
//! specifics live here; the application layer never sees them.
