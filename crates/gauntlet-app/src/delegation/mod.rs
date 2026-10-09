//! Delegation (E4a): the harness-neutral DelegationBroker's DELEGATE path.
//!
//! A delegation builds a fresh-context envelope from an allowlisted set of
//! kinds ([`context`]), selects a runner through the fabric, admits the node in
//! the Gauntlet-owned invocation graph, and invokes the runner exactly once per
//! admission key ([`broker`]). Lineage, depth, child, and budget limits are
//! enforced by the graph; harness topology is never consulted.

pub mod broker;
pub mod context;

#[cfg(test)]
mod tests;

pub use broker::{
    Delegated, DelegationBroker, DelegationOutcome, DelegationRefusal, DelegationRequest,
    RunnerSet, RunnerSetError,
};
pub use context::{
    ContextItem, ContextItemKind, ContextPolicy, ContextRefusal, MAX_ENVELOPE_BYTES, build_envelope,
};
