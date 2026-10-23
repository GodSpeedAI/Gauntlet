//! In-memory reference `InvocationGraph` (E2a). Single-threaded, like the
//! sibling reference doubles: a `RefCell` over plain vectors and maps.
//!
//! Semantics are honest all-or-nothing: every admission computes its
//! placement and budget checks against the current graph first, and only a
//! fully accepted node is pushed. A refusal or error never touches state.
//! The shared suite in [`super::invocation_graph`] runs against this type,
//! so the suite is proven satisfiable by a real implementation.

use core::cell::RefCell;
use std::collections::BTreeMap;

use gauntlet_domain::{
    AdmissionKey, BudgetDimension, EpochSeconds, InvocationId, RunId, SessionRef,
};

use crate::agent_runner::InvocationFailure;
use crate::budget_meter::BudgetSlice;
use crate::contract::invocation_graph::InvocationGraphFactory;
use crate::deadline::Deadline;
use crate::invocation_graph::{
    AdmissionOutcome, AdmissionRefusal, AdmissionRequest, GraphError, InvocationGraph,
    InvocationNode, NativeSessionBinding, NodeStatus, ResultBinding,
};
use crate::lifecycle::{
    HarnessObservation, LifecycleEntry, LifecycleView, ObservationOutcome, ObservationRecord,
    ObservationSequence, ObservationSource, SequenceOrigin,
};

/// The epoch of a node with no native session attached.
const NO_SESSION_EPOCH: u32 = 0;

/// Node storage. A node's index in `nodes` is its admission sequence, so
/// index order is admission order everywhere.
#[derive(Debug, Default)]
struct State {
    nodes: Vec<InvocationNode>,
    by_id: BTreeMap<InvocationId, usize>,
    by_key: BTreeMap<(RunId, AdmissionKey), usize>,
    /// The latest applied observation per (invocation, epoch, source).
    /// Superseded epochs keep their entries; only the current epoch is viewed.
    lifecycle: BTreeMap<(InvocationId, u32, ObservationSource), LifecycleEntry>,
    /// Every received observation, in receipt order.
    observations: Vec<ObservationRecord>,
}

/// The epoch an observation or view is judged under for a binding.
fn epoch_of(binding: Option<&NativeSessionBinding>) -> u32 {
    binding.map_or(NO_SESSION_EPOCH, |bound| bound.epoch)
}

fn origin_of(sequence: ObservationSequence) -> SequenceOrigin {
    match sequence {
        ObservationSequence::SourceProvided(_) => SequenceOrigin::SourceProvided,
        ObservationSequence::GauntletAssigned => SequenceOrigin::GauntletAssigned,
    }
}

/// The reference graph. Construct with `Default` or via
/// [`InMemoryInvocationGraphFactory`].
#[derive(Debug, Default)]
pub struct InMemoryInvocationGraph {
    state: RefCell<State>,
}

/// Builds fresh in-memory graphs for the shared suite.
#[derive(Debug, Clone, Copy, Default)]
pub struct InMemoryInvocationGraphFactory;

impl InvocationGraphFactory for InMemoryInvocationGraphFactory {
    fn label(&self) -> &'static str {
        "in-memory-reference"
    }

    fn build(&self) -> Result<Box<dyn InvocationGraph>, String> {
        Ok(Box::new(InMemoryInvocationGraph::default()))
    }
}

/// The immutable facts of a stored node match a request.
fn same_facts(existing: &InvocationNode, request: &AdmissionRequest) -> bool {
    existing.invocation_id == request.invocation_id
        && existing.run_id == request.run_id
        && existing.parent == request.parent
        && existing.role == request.role
        && existing.runner_profile == request.runner_profile
        && existing.harness == request.harness
        && existing.workspace == request.workspace
        && existing.context_digest == request.context_digest
        && existing.result_contract == request.result_contract
        && existing.admission_key == request.admission_key
        && existing.budget == request.budget
}

/// Componentwise budget rule: the live children's reserved slices plus the
/// requested slice must fit the parent's budget in every dimension. Sums
/// saturate, which can only over-reject, never over-admit.
fn fits(
    parent: &BudgetSlice,
    live: &[&InvocationNode],
    slice: &BudgetSlice,
) -> Result<(), BudgetDimension> {
    let total = |pick: fn(&BudgetSlice) -> u64| {
        live.iter()
            .fold(0_u64, |acc, node| acc.saturating_add(pick(&node.budget)))
    };
    let checks = [
        (
            BudgetDimension::Rounds,
            u64::from(parent.rounds),
            total(|b| u64::from(b.rounds)).saturating_add(u64::from(slice.rounds)),
        ),
        (
            BudgetDimension::WallSeconds,
            parent.wall_seconds,
            total(|b| b.wall_seconds).saturating_add(slice.wall_seconds),
        ),
        (
            BudgetDimension::Tokens,
            parent.tokens,
            total(|b| b.tokens).saturating_add(slice.tokens),
        ),
        (
            BudgetDimension::CostUsd,
            parent.cost_usd.get(),
            total(|b| b.cost_usd.get()).saturating_add(slice.cost_usd.get()),
        ),
    ];
    for (dimension, cap, wanted) in checks {
        if wanted > cap {
            return Err(dimension);
        }
    }
    Ok(())
}

impl State {
    fn index(&self, id: &InvocationId) -> Result<usize, GraphError> {
        self.by_id
            .get(id)
            .copied()
            .ok_or_else(|| GraphError::UnknownInvocation { id: id.clone() })
    }

    /// Direct children, in admission order.
    fn child_indices(&self, parent: &InvocationId) -> Vec<usize> {
        (0..self.nodes.len())
            .filter(|&i| self.nodes[i].parent.as_ref() == Some(parent))
            .collect()
    }

    /// Every descendant, sorted by depth then admission order.
    fn descendant_indices(&self, id: &InvocationId) -> Vec<usize> {
        let mut out = Vec::new();
        let mut frontier = vec![id.clone()];
        while let Some(current) = frontier.pop() {
            for child in self.child_indices(&current) {
                frontier.push(self.nodes[child].invocation_id.clone());
                out.push(child);
            }
        }
        out.sort_by_key(|&i| (self.nodes[i].depth, i));
        out
    }

    /// Parent first, root last.
    fn ancestor_indices(&self, id: &InvocationId) -> Result<Vec<usize>, GraphError> {
        let mut out = Vec::new();
        let mut current = self.index(id)?;
        while let Some(parent) = self.nodes[current].parent.as_ref() {
            current = self.index(parent)?;
            out.push(current);
        }
        Ok(out)
    }

    /// Computes placement and budget for a request without touching state.
    fn place(&self, request: &AdmissionRequest) -> Result<(InvocationId, u32), AdmissionRefusal> {
        let Some(parent_id) = request.parent.as_ref() else {
            let root_exists = self
                .nodes
                .iter()
                .any(|n| n.run_id == request.run_id && n.parent.is_none());
            if root_exists {
                return Err(AdmissionRefusal::RootAlreadyExists);
            }
            return Ok((request.invocation_id.clone(), 0));
        };
        let &parent_index = self
            .by_id
            .get(parent_id)
            .ok_or(AdmissionRefusal::UnknownParent)?;
        let parent = &self.nodes[parent_index];
        if parent.run_id != request.run_id {
            return Err(AdmissionRefusal::WrongRun);
        }
        if parent.status.is_terminal() {
            return Err(AdmissionRefusal::ParentTerminal);
        }
        let would_be = parent.depth.saturating_add(1);
        if would_be > request.limits.max_depth {
            return Err(AdmissionRefusal::DepthLimit {
                max: request.limits.max_depth,
                would_be,
            });
        }
        let live: Vec<&InvocationNode> = self
            .child_indices(parent_id)
            .into_iter()
            .map(|i| &self.nodes[i])
            .filter(|n| !n.status.is_terminal())
            .collect();
        if live.len()
            >= usize::try_from(request.limits.max_children_per_parent).unwrap_or(usize::MAX)
        {
            return Err(AdmissionRefusal::ChildLimit {
                max: request.limits.max_children_per_parent,
            });
        }
        fits(&parent.budget, &live, &request.budget)
            .map_err(|dimension| AdmissionRefusal::BudgetExceeded { dimension })?;
        Ok((parent.root.clone(), would_be))
    }

    fn admit(&mut self, request: AdmissionRequest) -> AdmissionOutcome {
        let key = (request.run_id.clone(), request.admission_key.clone());
        if let Some(&index) = self.by_key.get(&key) {
            let existing = &self.nodes[index];
            return if same_facts(existing, &request) {
                AdmissionOutcome::AlreadyAdmitted(existing.clone())
            } else {
                AdmissionOutcome::Refused(AdmissionRefusal::KeyReusedWithDifferentFacts)
            };
        }
        if self.by_id.contains_key(&request.invocation_id) {
            return AdmissionOutcome::Refused(AdmissionRefusal::InvocationIdReused);
        }
        let (root, depth) = match self.place(&request) {
            Ok(placement) => placement,
            Err(refusal) => return AdmissionOutcome::Refused(refusal),
        };
        let node = InvocationNode {
            invocation_id: request.invocation_id.clone(),
            run_id: request.run_id,
            parent: request.parent,
            root,
            depth,
            role: request.role,
            runner_profile: request.runner_profile,
            harness: request.harness,
            workspace: request.workspace,
            context_digest: request.context_digest,
            result_contract: request.result_contract,
            admission_key: request.admission_key,
            budget: request.budget,
            status: NodeStatus::Admitted,
            native_session: None,
            result_digest: None,
            admitted_at: request.at,
            terminal_at: None,
        };
        let index = self.nodes.len();
        self.by_id.insert(node.invocation_id.clone(), index);
        self.by_key.insert(key, index);
        self.nodes.push(node.clone());
        AdmissionOutcome::Admitted(node)
    }

    /// Moves a non-terminal node to a terminal (or running) status.
    fn settle(
        &mut self,
        id: &InvocationId,
        to: NodeStatus,
        at: Option<EpochSeconds>,
    ) -> Result<InvocationNode, GraphError> {
        let index = self.index(id)?;
        let node = &mut self.nodes[index];
        if node.status.is_terminal() {
            return Err(GraphError::IllegalTransition {
                id: id.clone(),
                from: node.status,
                to,
            });
        }
        node.status = to;
        node.terminal_at = at;
        Ok(node.clone())
    }

    /// Decides one observation for a non-terminal node whose binding matches.
    /// Returns the outcome and the observation as it is recorded. Mutates the
    /// lifecycle map only when the observation applies.
    fn apply_observation(
        &mut self,
        observation: &HarnessObservation,
        epoch: u32,
    ) -> Result<(ObservationOutcome, HarnessObservation), GraphError> {
        let key = (observation.invocation_id.clone(), epoch, observation.source);
        let previous = self.lifecycle.get(&key).cloned();
        let last_applied = previous.as_ref().map_or(0, |entry| entry.sequence);
        let sequence = match observation.sequence {
            ObservationSequence::SourceProvided(number) if number <= last_applied => {
                return Ok((
                    ObservationOutcome::IgnoredStale { last_applied },
                    observation.clone(),
                ));
            }
            ObservationSequence::SourceProvided(number) => number,
            ObservationSequence::GauntletAssigned => {
                last_applied
                    .checked_add(1)
                    .ok_or_else(|| GraphError::WriteFailed {
                        detail: "lifecycle sequence exhausted".to_owned(),
                    })?
            }
        };
        self.lifecycle.insert(
            key,
            LifecycleEntry {
                state: observation.state,
                source: observation.source,
                sequence,
                origin: origin_of(observation.sequence),
                observed_at: observation.observed_at,
                epoch,
            },
        );
        let mut recorded = observation.clone();
        recorded.sequence = ObservationSequence::SourceProvided(sequence);
        Ok((
            ObservationOutcome::Applied {
                sequence,
                previous: previous.map(|entry| entry.state),
            },
            recorded,
        ))
    }

    fn record_observation(
        &mut self,
        observation: HarnessObservation,
        received_at: EpochSeconds,
    ) -> Result<ObservationOutcome, GraphError> {
        let index = self.index(&observation.invocation_id)?;
        let terminal = self.nodes[index].status.is_terminal();
        let current = self.nodes[index].native_session.clone();
        let (outcome, recorded) = if terminal {
            (ObservationOutcome::IgnoredTerminal, observation.clone())
        } else if observation.native_session != current {
            (
                ObservationOutcome::RejectedStaleSession {
                    current: current.clone(),
                },
                observation.clone(),
            )
        } else {
            self.apply_observation(&observation, epoch_of(current.as_ref()))?
        };
        self.observations.push(ObservationRecord {
            origin: origin_of(observation.sequence),
            observation: recorded,
            outcome: outcome.clone(),
            received_at,
        });
        Ok(outcome)
    }

    /// The current view: entries of the node's current epoch, highest trust
    /// first. The effective entry is the first one.
    fn lifecycle_view(&self, index: usize) -> LifecycleView {
        let node = &self.nodes[index];
        let epoch = epoch_of(node.native_session.as_ref());
        let mut per_source: Vec<LifecycleEntry> = self
            .lifecycle
            .iter()
            .filter(|(key, _)| key.0 == node.invocation_id && key.1 == epoch)
            .map(|(_, entry)| entry.clone())
            .collect();
        per_source.sort_by_key(|entry| core::cmp::Reverse(entry.source));
        LifecycleView {
            effective: per_source.first().cloned(),
            per_source,
        }
    }
}

impl InvocationGraph for InMemoryInvocationGraph {
    fn admit(
        &self,
        request: AdmissionRequest,
        _deadline: Deadline,
    ) -> Result<AdmissionOutcome, GraphError> {
        Ok(self.state.borrow_mut().admit(request))
    }

    fn attach_native_session(
        &self,
        id: &InvocationId,
        session: SessionRef,
        _deadline: Deadline,
    ) -> Result<NativeSessionBinding, GraphError> {
        let state = &mut *self.state.borrow_mut();
        let index = state.index(id)?;
        let node = &mut state.nodes[index];
        if node.status.is_terminal() {
            return Err(GraphError::IllegalTransition {
                id: id.clone(),
                from: node.status,
                to: node.status,
            });
        }
        let epoch = match node.native_session.as_ref() {
            None => 1,
            Some(bound) => bound
                .epoch
                .checked_add(1)
                .ok_or_else(|| GraphError::WriteFailed {
                    detail: "native session epoch exhausted".to_owned(),
                })?,
        };
        let binding = NativeSessionBinding { session, epoch };
        node.native_session = Some(binding.clone());
        Ok(binding)
    }

    fn mark_running(
        &self,
        id: &InvocationId,
        _deadline: Deadline,
    ) -> Result<InvocationNode, GraphError> {
        let state = &mut *self.state.borrow_mut();
        let index = state.index(id)?;
        let from = state.nodes[index].status;
        if from != NodeStatus::Admitted {
            return Err(GraphError::IllegalTransition {
                id: id.clone(),
                from,
                to: NodeStatus::Running,
            });
        }
        state.nodes[index].status = NodeStatus::Running;
        Ok(state.nodes[index].clone())
    }

    fn complete(
        &self,
        id: &InvocationId,
        binding: ResultBinding,
        at: EpochSeconds,
        _deadline: Deadline,
    ) -> Result<InvocationNode, GraphError> {
        let state = &mut *self.state.borrow_mut();
        let index = state.index(id)?;
        let node = &state.nodes[index];
        if node.status.is_terminal() {
            return Err(GraphError::IllegalTransition {
                id: id.clone(),
                from: node.status,
                to: NodeStatus::Completed,
            });
        }
        if binding.contract_path != node.result_contract
            || binding.context_digest != node.context_digest
        {
            return Err(GraphError::ContractMismatch { id: id.clone() });
        }
        let result_digest = binding.result_digest;
        let mut completed = state.settle(id, NodeStatus::Completed, Some(at))?;
        completed.result_digest = Some(result_digest.clone());
        state.nodes[index].result_digest = Some(result_digest);
        Ok(completed)
    }

    fn fail(
        &self,
        id: &InvocationId,
        class: InvocationFailure,
        at: EpochSeconds,
        _deadline: Deadline,
    ) -> Result<InvocationNode, GraphError> {
        self.state
            .borrow_mut()
            .settle(id, NodeStatus::Failed(class), Some(at))
    }

    fn time_out(
        &self,
        id: &InvocationId,
        at: EpochSeconds,
        _deadline: Deadline,
    ) -> Result<InvocationNode, GraphError> {
        self.state
            .borrow_mut()
            .settle(id, NodeStatus::TimedOut, Some(at))
    }

    fn cancel_subtree(
        &self,
        id: &InvocationId,
        at: EpochSeconds,
        _deadline: Deadline,
    ) -> Result<Vec<InvocationId>, GraphError> {
        let state = &mut *self.state.borrow_mut();
        let root = state.index(id)?;
        let mut targets = vec![root];
        targets.extend(state.descendant_indices(id));
        let newly: Vec<usize> = targets
            .into_iter()
            .filter(|&i| !state.nodes[i].status.is_terminal())
            .collect();
        let mut cancelled = Vec::with_capacity(newly.len());
        for index in newly {
            let node = &mut state.nodes[index];
            node.status = NodeStatus::Cancelled;
            node.terminal_at = Some(at);
            cancelled.push(node.invocation_id.clone());
        }
        Ok(cancelled)
    }

    fn node(
        &self,
        id: &InvocationId,
        _deadline: Deadline,
    ) -> Result<Option<InvocationNode>, GraphError> {
        let state = self.state.borrow();
        Ok(state.by_id.get(id).map(|&i| state.nodes[i].clone()))
    }

    fn children(
        &self,
        id: &InvocationId,
        _deadline: Deadline,
    ) -> Result<Vec<InvocationNode>, GraphError> {
        let state = self.state.borrow();
        state.index(id)?;
        Ok(state
            .child_indices(id)
            .into_iter()
            .map(|i| state.nodes[i].clone())
            .collect())
    }

    fn descendants(
        &self,
        id: &InvocationId,
        _deadline: Deadline,
    ) -> Result<Vec<InvocationNode>, GraphError> {
        let state = self.state.borrow();
        state.index(id)?;
        Ok(state
            .descendant_indices(id)
            .into_iter()
            .map(|i| state.nodes[i].clone())
            .collect())
    }

    fn ancestors(
        &self,
        id: &InvocationId,
        _deadline: Deadline,
    ) -> Result<Vec<InvocationNode>, GraphError> {
        let state = self.state.borrow();
        Ok(state
            .ancestor_indices(id)?
            .into_iter()
            .map(|i| state.nodes[i].clone())
            .collect())
    }

    fn active_nodes(
        &self,
        run_id: &RunId,
        _deadline: Deadline,
    ) -> Result<Vec<InvocationNode>, GraphError> {
        let state = self.state.borrow();
        Ok(state
            .nodes
            .iter()
            .filter(|n| &n.run_id == run_id && !n.status.is_terminal())
            .cloned()
            .collect())
    }

    fn record_observation(
        &self,
        observation: HarnessObservation,
        received_at: EpochSeconds,
        _deadline: Deadline,
    ) -> Result<ObservationOutcome, GraphError> {
        self.state
            .borrow_mut()
            .record_observation(observation, received_at)
    }

    fn lifecycle(
        &self,
        id: &InvocationId,
        _deadline: Deadline,
    ) -> Result<LifecycleView, GraphError> {
        let state = self.state.borrow();
        let index = state.index(id)?;
        Ok(state.lifecycle_view(index))
    }

    fn observation_history(
        &self,
        id: &InvocationId,
        _deadline: Deadline,
    ) -> Result<Vec<ObservationRecord>, GraphError> {
        let state = self.state.borrow();
        state.index(id)?;
        Ok(state
            .observations
            .iter()
            .filter(|record| &record.observation.invocation_id == id)
            .cloned()
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::InMemoryInvocationGraphFactory;
    use crate::contract::invocation_graph::invocation_graph_suite;

    #[test]
    fn invocation_graph_reference_passes_the_suite() {
        let findings = invocation_graph_suite(&InMemoryInvocationGraphFactory);
        assert!(
            findings.is_empty(),
            "the reference graph must run the suite clean, but reported:\n{}",
            findings
                .iter()
                .map(|f| f.to_string())
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
}
