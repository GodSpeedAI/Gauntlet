//! `IdGenerator` — spec §6.2: "`run_id()` · `invocation_id()` — Injectable;
//! tests use a seeded generator."
//!
//! The domain never generates ids — it validates them; this port is where
//! ids come from. Production adapters mint ULIDs (sortable,
//! collision-resistant, spec §5.2); seeded test adapters replay fixed
//! sequences deterministically.

use gauntlet_domain::{InvocationId, RunId};

/// The identity port (spec §6.2). Pure — no deadlines.
pub trait IdGenerator {
    /// A fresh run id (spec §5.2 ULID grammar).
    fn run_id(&self) -> RunId;

    /// A fresh invocation id (same grammar, distinct type — an invocation
    /// id cannot stand in for a run id).
    fn invocation_id(&self) -> InvocationId;
}
