//! Lifecycle sequencing (E3): turns raw harness readings into Gauntlet-
//! sequenced, session-bound observations.
//!
//! A runner reports a [`HarnessLifecycleSample`]. It carries no sequence, and
//! none is trusted if a runner supplies one. This supervisor stamps each
//! reading with a Gauntlet sequence at receipt, binds it to the node's CURRENT
//! `(session, epoch)`, and only then records it through
//! [`InvocationGraph::record_observation`].
//!
//! Invariants this module upholds (pinned by the `lifecycle_sequencing_*`
//! acceptance tests in `lifecycle_tests`):
//!
//! - Sequences are per `(invocation, epoch, source)` and strictly increase.
//!   The next number is one above the highest number ANY earlier record of
//!   that key carries, including ignored and rejected records. A number is
//!   therefore never reused, even after a restart.
//! - The high-water mark is read from the persisted observation history, never
//!   from memory. A fresh supervisor after a restart resumes exactly where the
//!   previous one stopped. A supervisor is stateless and holds no counter.
//! - A reading older than the applied view of its key (by `observed_at`) is
//!   stamped with the applied number, so the graph ignores it as stale. A late
//!   reading cannot roll the view back.
//! - A reading naming another session is bound to the current epoch with its
//!   own session ref. The graph rejects it as wrong-session and it only
//!   becomes history.
//! - Nothing here changes node status, completes, or fails an invocation.
//!
//! Receipt-order note: the port's `GauntletAssigned` is numbered at APPLY time
//! inside the graph, so it cannot give receipt order. Stamping here, at
//! receipt, does. Concurrent supervisors on one invocation cannot corrupt the
//! view. The graph's atomic `SourceProvided` comparison turns any race into
//! `IgnoredStale`.

use gauntlet_domain::InvocationId;
use gauntlet_ports::Deadline;
use gauntlet_ports::invocation_graph::{GraphError, InvocationGraph, NativeSessionBinding};
use gauntlet_ports::lifecycle::{
    HarnessLifecycleSample, HarnessObservation, ObservationOutcome, ObservationRecord,
    ObservationSequence, ObservationSource,
};

/// Stamps and records harness lifecycle readings against one graph.
pub struct LifecycleSupervisor<'g> {
    graph: &'g dyn InvocationGraph,
}

impl<'g> LifecycleSupervisor<'g> {
    pub fn new(graph: &'g dyn InvocationGraph) -> Self {
        Self { graph }
    }

    /// Receives one reading for `id`, stamps it, and records it.
    ///
    /// `Unknown invocation` is a typed graph error. Every other outcome,
    /// including stale, wrong-session, and terminal, is returned as `Ok`
    /// because the graph has already kept the reading as history.
    pub fn receive(
        &self,
        id: &InvocationId,
        sample: HarnessLifecycleSample,
        deadline: Deadline,
    ) -> Result<ObservationOutcome, GraphError> {
        let node = self
            .graph
            .node(id, deadline)?
            .ok_or_else(|| GraphError::UnknownInvocation { id: id.clone() })?;
        let current = node.native_session.clone();
        let epoch = epoch_of(current.as_ref());
        // Judged against the node's current binding. A reading naming the
        // bound session gets the exact current binding and is applied. Any
        // other reading gets its own session at the current epoch, which the
        // graph refuses as wrong-session.
        let binding = sample
            .native_session
            .as_ref()
            .map(|session| NativeSessionBinding {
                session: session.clone(),
                epoch,
            });
        let same_binding = binding == current;

        let view = self.graph.lifecycle(id, deadline)?;
        let history = self.graph.observation_history(id, deadline)?;
        let sequence = match view
            .per_source
            .iter()
            .find(|entry| entry.source == sample.source)
        {
            Some(applied) if same_binding && sample.observed_at < applied.observed_at => {
                applied.sequence
            }
            _ => high_water(&history, epoch, sample.source).saturating_add(1),
        };

        self.graph.record_observation(
            HarnessObservation {
                invocation_id: id.clone(),
                native_session: binding,
                state: sample.state,
                source: sample.source,
                observed_at: sample.observed_at,
                sequence: ObservationSequence::SourceProvided(sequence),
            },
            sample.observed_at,
            deadline,
        )
    }
}

/// The epoch a binding names. Epoch 0 means no session was attached.
fn epoch_of(binding: Option<&NativeSessionBinding>) -> u32 {
    binding.map_or(0, |bound| bound.epoch)
}

/// The highest number any recorded observation of this key carries, or 0.
/// Read from history so it survives restarts and includes ignored records.
fn high_water(history: &[ObservationRecord], epoch: u32, source: ObservationSource) -> u64 {
    history
        .iter()
        .filter(|record| {
            record.observation.source == source
                && epoch_of(record.observation.native_session.as_ref()) == epoch
        })
        .filter_map(|record| match record.observation.sequence {
            ObservationSequence::SourceProvided(number) => Some(number),
            ObservationSequence::GauntletAssigned => None,
        })
        .max()
        .unwrap_or(0)
}
