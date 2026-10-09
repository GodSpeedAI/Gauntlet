//! Delegation (E4a): the harness-neutral DelegationBroker's DELEGATE path.
//!
//! A delegation builds a fresh-context envelope from an allowlisted set of
//! kinds ([`context`]), selects a runner through the fabric, admits the node in
//! the Gauntlet-owned invocation graph, and invokes the runner exactly once per
//! admission key ([`broker`]). Lineage, depth, child, and budget limits are
//! enforced by the graph; harness topology is never consulted.

pub mod agents;
pub mod agents_wire;
pub mod broker;
pub mod context;
pub mod reconcile;

#[cfg(test)]
#[allow(clippy::arc_with_non_send_sync)]
// test doubles are single-threaded (RefCell); the broker holds them behind Arc like production runners
mod agents_tests;
#[cfg(test)]
#[allow(clippy::arc_with_non_send_sync)]
// test doubles are single-threaded (RefCell); the broker holds them behind Arc like production runners
mod agents_wire_tests;
#[cfg(test)]
#[allow(clippy::arc_with_non_send_sync)] // single-threaded test doubles behind Arc
pub(crate) mod test_support;
#[cfg(test)]
#[allow(clippy::arc_with_non_send_sync)]
// test doubles are single-threaded (RefCell); the broker holds them behind Arc like production runners
mod tests;

pub use agents::{
    AgentView, AgentsCaller, AgentsPolicy, AgentsRefusal, AgentsService, CancelView, CollectView,
    RESULT_CONTRACT_NAME, RoleGrant, SpawnArgs, SpawnOutcome, WorkspacePolicy,
    derive_invocation_id,
};
pub use agents_wire::{AgentsSurface, CallClock};
pub use broker::{
    Delegated, DelegationBroker, DelegationOutcome, DelegationRefusal, DelegationRequest,
    RunnerSet, RunnerSetError,
};
pub use context::{
    ContextItem, ContextItemKind, ContextPolicy, ContextRefusal, MAX_ENVELOPE_BYTES, build_envelope,
};
pub use reconcile::{
    BrokerError, CancelReport, CollectOutcome, DelegationStatus, RecoveryAction, RecoveryReport,
    Unreconciled,
};
