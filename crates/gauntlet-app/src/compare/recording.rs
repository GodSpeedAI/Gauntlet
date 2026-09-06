//! Application-owned durable boundary for model comparison dispatch.
//!
//! The model adapter remains responsible for parsing and binding untrusted
//! result contracts. This service owns the consequential lifecycle: it records
//! the exact domain invocation after runner admission and accepts a comparison
//! only after the adapter has completed those checks.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::sync::Arc;

use gauntlet_domain::{
    BlastRadius, EpochSeconds, InvocationId, RoleInvocation, RoleInvocationDefinition,
    RunnerIdentity,
};
use gauntlet_ports::agent_runner::{
    AgentError, AgentRunner, CancelReason, InvocationHandle, InvocationStatus, RoleInvocationSpec,
    RunnerCapabilities,
};
use gauntlet_ports::comparator::MandatoryComparisonAcceptance;
use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::evidence_store::EvidenceStore;
use gauntlet_ports::state_store::{RunEvent, RunEventKind, StateStore};

/// The runner injected into a model comparator at the application composition
/// boundary. It never permits polling of a child whose admitted aggregate was
/// not made durable.
///
/// The tier channel (corrections round 1, F3): the durable role_invocation
/// record's `tier` field must state the dispatch's REAL tier, never a
/// hardcoded default. The dispatch facts live with the caller that knows the
/// unit's tier at admission time (the gate admission), so the channel is a
/// typed per-invocation registration consumed exactly once, at the durable
/// record's construction. The [`AgentRunner`] port itself is NOT widened:
/// the port's specs carry no tier and stay that way. Single-threaded loop
/// posture (`RefCell` interior, the composed controller's own recorded
/// posture).
#[derive(Clone)]
pub struct RecordingAgentRunner {
    inner: Arc<dyn AgentRunner>,
    state: Arc<dyn StateStore>,
    evidence: Arc<dyn EvidenceStore>,
    runner_identity: RunnerIdentity,
    admitted_at: EpochSeconds,
    dispatch_tiers: RefCell<BTreeMap<InvocationId, BlastRadius>>,
}

impl RecordingAgentRunner {
    pub fn new(
        inner: Arc<dyn AgentRunner>,
        state: Arc<dyn StateStore>,
        evidence: Arc<dyn EvidenceStore>,
        runner_identity: RunnerIdentity,
        admitted_at: EpochSeconds,
    ) -> Self {
        Self {
            inner,
            state,
            evidence,
            runner_identity,
            admitted_at,
            dispatch_tiers: RefCell::new(BTreeMap::new()),
        }
    }

    /// Records the dispatch's tier fact — the tier the gate admitted the
    /// unit at — for the invocation about to be dispatched. Consumed by
    /// the durable record's construction ([`Self::admitted`]); a fact the
    /// caller knows at admission time, typed end to end.
    pub fn record_dispatch_tier(&self, invocation_id: InvocationId, tier: BlastRadius) {
        self.dispatch_tiers.borrow_mut().insert(invocation_id, tier);
    }

    /// The acceptance port to inject into `ModelComparator` alongside this
    /// runner. Both halves share the same durable stores but preserve the
    /// adapter/application dependency direction.
    pub fn mandatory_comparison_acceptance(&self) -> MandatoryComparisonAcceptance {
        MandatoryComparisonAcceptance::new(self.state.clone(), self.evidence.clone())
    }

    fn admitted(
        &self,
        spec: &RoleInvocationSpec,
        handle: &InvocationHandle,
    ) -> Result<RoleInvocation, AgentError> {
        if handle.invocation_id() != &spec.invocation_id {
            return Err(AgentError::AdmissionRefused {
                detail: format!(
                    "runner returned handle {} for admitted spec {}",
                    handle.invocation_id(),
                    spec.invocation_id
                ),
            });
        }
        // The dispatch's REAL tier (corrections round 1, F3): the fact the
        // admitting composition recorded, consumed exactly once here. A
        // no-policy dispatch is the §5.3.5-legal shape ONLY at the
        // contained tier under the conservative no-backend posture, so an
        // absent tier fact there is that dispatch and no other. A
        // policy-carrying dispatch whose tier fact never arrived is a
        // composition defect: the record must not guess — the admission
        // is refused and the child is cancelled for recovery by the
        // invoke path's existing persistence-failure arm.
        let tier = match self.dispatch_tiers.borrow_mut().remove(&spec.invocation_id) {
            Some(tier) => tier,
            None if spec.sandbox_policy.is_none() => BlastRadius::Contained,
            None => {
                return Err(AgentError::AdmissionRefused {
                    detail: format!(
                        "no dispatch tier fact was recorded for policy-carrying invocation {} — \
                         the admitting composition must record the gate-admitted tier before \
                         the dispatch (the durable record must not guess a tier)",
                        spec.invocation_id
                    ),
                });
            }
        };
        RoleInvocation::admit(RoleInvocationDefinition {
            id: spec.invocation_id.clone(),
            run_id: spec.run_id.clone(),
            iteration_no: spec.iteration_no,
            role: spec.role.clone(),
            runner: self.runner_identity.clone(),
            model: spec.model_hint.clone(),
            permissions: spec.permissions.clone(),
            context_envelope_digest: spec.context_envelope.digest().clone(),
            workspace_ref: spec.workspace.clone(),
            tier,
            sandbox_policy_ref: spec.sandbox_policy.clone(),
            result_contract_path: spec.result_contract_path.clone(),
            admitted_at: Some(self.admitted_at),
            session_ref: handle.session_ref().cloned(),
        })
        .map_err(|error| AgentError::AdmissionRefused {
            detail: format!("constructing admitted role invocation failed: {error}"),
        })
    }

    fn cancel_after_persistence_failure(
        &self,
        handle: &InvocationHandle,
        deadline: Deadline,
        locus: &str,
    ) -> AgentError {
        let reason = CancelReason::new("durable invocation recording failed before polling")
            .expect("fixed cancellation reason");
        let cancellation = self.inner.cancel(handle, reason, deadline);
        AgentError::AdmissionRefused {
            detail: match cancellation {
                Ok(()) | Err(AgentError::AlreadyTerminal { .. }) => format!(
                    "{locus}; admitted child {} was cancelled for recovery",
                    handle.invocation_id()
                ),
                Err(error) => format!(
                    "{locus}; admitted child {} could not be cancelled and must be re-probed: {error}",
                    handle.invocation_id()
                ),
            },
        }
    }
}

impl AgentRunner for RecordingAgentRunner {
    fn invoke(&self, spec: RoleInvocationSpec) -> Result<InvocationHandle, AgentError> {
        let handle = self.inner.invoke(spec.clone())?;
        let invocation = match self.admitted(&spec, &handle) {
            Ok(invocation) => invocation,
            Err(error) => {
                return Err(self.cancel_after_persistence_failure(
                    &handle,
                    spec.deadline,
                    &error.to_string(),
                ));
            }
        };
        if let Err(error) = self
            .state
            .record_role_invocation(&invocation, spec.deadline)
        {
            return Err(self.cancel_after_persistence_failure(
                &handle,
                spec.deadline,
                &format!(
                    "recording admitted invocation {} failed: {error}",
                    invocation.id()
                ),
            ));
        }
        if let Err(error) = self.state.append_event(
            RunEvent {
                run_id: spec.run_id.clone(),
                kind: RunEventKind::InvocationAdmitted {
                    invocation_id: spec.invocation_id.clone(),
                },
                at: self.admitted_at,
            },
            spec.deadline,
        ) {
            return Err(self.cancel_after_persistence_failure(
                &handle,
                spec.deadline,
                &format!(
                    "writing admission event for {} failed: {error}",
                    invocation.id()
                ),
            ));
        }
        Ok(handle)
    }

    fn poll(&self, handle: &InvocationHandle) -> Result<InvocationStatus, AgentError> {
        self.inner.poll(handle)
    }

    fn collect(
        &self,
        handle: &InvocationHandle,
        deadline: Deadline,
    ) -> Result<gauntlet_ports::role_result::RoleResult, AgentError> {
        self.inner.collect(handle, deadline)
    }

    fn cancel(
        &self,
        handle: &InvocationHandle,
        reason: CancelReason,
        deadline: Deadline,
    ) -> Result<(), AgentError> {
        self.inner.cancel(handle, reason, deadline)
    }

    fn capabilities(&self) -> RunnerCapabilities {
        self.inner.capabilities()
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::sync::Arc;

    use super::RecordingAgentRunner;
    use gauntlet_domain::{
        Budget, DiagnosticAllowance, Digest, EpochSeconds, EvidenceKind, IterationNo, Limits,
        MicroUsd, PermissionSetRef, ResultContractPath, RoleId, RunId, RunnerIdentity,
        WorkspaceRef,
    };
    use gauntlet_ports::agent_runner::{
        AgentError, AgentRunner, ContextEnvelope, InvocationHandle, InvocationStatus, Isolation,
        ResultDelivery, RoleInvocationSpec, RunnerCapabilities,
    };
    use gauntlet_ports::deadline::Deadline;
    use gauntlet_ports::evidence_store::{EvidenceStore, EvidenceStoreError};
    use gauntlet_ports::state_store::{Checkpoint, StateStore, StateStoreError};

    const ID: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
    const TEST_MAX_ROUNDS: u32 = 2;
    const TEST_MAX_PARALLEL: u32 = 1;
    const TEST_MAX_DEPTH: u32 = 1;
    const TEST_MAX_TOKENS: u64 = 100;
    const TEST_MAX_WALL_SECONDS: u64 = TEST_MAX_TOKENS;
    const TEST_MAX_COST: u64 = 100;
    const TEST_RECOVERY_RESERVE: u8 = 20;
    const TEST_DIAGNOSTIC_TOKENS: u64 = 10;
    const TEST_DIAGNOSTIC_COST: u64 = 10;
    const TEST_DEADLINE_AT: u64 = TEST_MAX_TOKENS;

    struct CancellingRunner(Rc<RefCell<u32>>);
    impl AgentRunner for CancellingRunner {
        fn invoke(&self, spec: RoleInvocationSpec) -> Result<InvocationHandle, AgentError> {
            Ok(InvocationHandle::new(spec.invocation_id, None))
        }
        fn poll(&self, _: &InvocationHandle) -> Result<InvocationStatus, AgentError> {
            Ok(InvocationStatus::Admitted)
        }
        fn collect(
            &self,
            _: &InvocationHandle,
            _: Deadline,
        ) -> Result<gauntlet_ports::role_result::RoleResult, AgentError> {
            unreachable!("persistence refusal prevents collection")
        }
        fn cancel(
            &self,
            _: &InvocationHandle,
            _: gauntlet_ports::CancelReason,
            _: Deadline,
        ) -> Result<(), AgentError> {
            *self.0.borrow_mut() += 1;
            Ok(())
        }
        fn capabilities(&self) -> RunnerCapabilities {
            RunnerCapabilities {
                supports_native_subagents: false,
                supports_streaming_events: false,
                supports_model_selection: false,
                isolation: Isolation::Process,
                result_delivery: ResultDelivery::File,
                max_depth: 1,
            }
        }
    }

    struct FailingState;
    impl StateStore for FailingState {
        fn append_event(
            &self,
            _: gauntlet_ports::RunEvent,
            _: Deadline,
        ) -> Result<gauntlet_domain::EventLogOffset, StateStoreError> {
            Err(StateStoreError::WriteFailed {
                detail: "unused".to_owned(),
            })
        }
        fn load_run(
            &self,
            _: &RunId,
            _: Deadline,
        ) -> Result<Option<gauntlet_domain::Run>, StateStoreError> {
            Err(StateStoreError::ReadFailed {
                detail: "unused".to_owned(),
            })
        }
        fn checkpoint(&self, _: Checkpoint, _: Deadline) -> Result<(), StateStoreError> {
            Err(StateStoreError::WriteFailed {
                detail: "unused".to_owned(),
            })
        }
        fn resume(&self, _: &RunId, _: Deadline) -> Result<Option<Checkpoint>, StateStoreError> {
            Err(StateStoreError::ReadFailed {
                detail: "unused".to_owned(),
            })
        }
        fn export_jsonl(&self, _: &RunId, _: Deadline) -> Result<String, StateStoreError> {
            Err(StateStoreError::ReadFailed {
                detail: "unused".to_owned(),
            })
        }
    }

    struct UnusedEvidence;
    impl EvidenceStore for UnusedEvidence {
        fn put(
            &self,
            _: &[u8],
            _: EvidenceKind,
            _: Deadline,
        ) -> Result<gauntlet_domain::EvidenceRef, EvidenceStoreError> {
            Err(EvidenceStoreError::WriteFailed {
                detail: "unused".to_owned(),
            })
        }
        fn get(
            &self,
            _: &gauntlet_domain::EvidenceRef,
            _: Deadline,
        ) -> Result<Option<Vec<u8>>, EvidenceStoreError> {
            Err(EvidenceStoreError::ReadFailed {
                detail: "unused".to_owned(),
            })
        }
        fn pin(
            &self,
            _: &RunId,
            _: &gauntlet_domain::EvidenceRef,
            _: Deadline,
        ) -> Result<(), EvidenceStoreError> {
            Err(EvidenceStoreError::PinFailed {
                reference: gauntlet_domain::EvidenceRef::new("evd-unused").expect("valid ref"),
                detail: "unused".to_owned(),
            })
        }
    }

    fn spec() -> RoleInvocationSpec {
        let budget = Budget::new(
            Limits::new(
                TEST_MAX_ROUNDS,
                TEST_MAX_PARALLEL,
                TEST_MAX_DEPTH,
                TEST_MAX_WALL_SECONDS,
                TEST_MAX_TOKENS,
                MicroUsd::new(TEST_MAX_COST),
            )
            .expect("limits"),
            TEST_RECOVERY_RESERVE,
            DiagnosticAllowance::new(TEST_DIAGNOSTIC_TOKENS, MicroUsd::new(TEST_DIAGNOSTIC_COST)),
        )
        .expect("budget");
        RoleInvocationSpec {
            invocation_id: gauntlet_domain::InvocationId::new(ID).expect("id"),
            run_id: RunId::new(ID).expect("id"),
            iteration_no: IterationNo::new(1).expect("iteration"),
            role: RoleId::Critic,
            prompt: "critic".to_owned(),
            context_envelope: ContextEnvelope::new(
                "{}".to_owned(),
                Digest::new("sha256", "aa").expect("digest"),
            ),
            workspace: WorkspaceRef::new("critic-ws").expect("workspace"),
            permissions: PermissionSetRef::new("envelope-only").expect("permissions"),
            result_contract_path: ResultContractPath::new("critic-result").expect("path"),
            deadline: Deadline::at_epoch(EpochSeconds::new(TEST_DEADLINE_AT)),
            model_hint: None,
            sandbox_policy: None,
            budget_slice: budget,
        }
    }

    #[test]
    fn persistence_failure_cancels_the_already_admitted_child() {
        let cancellations = Rc::new(RefCell::new(0));
        #[allow(clippy::arc_with_non_send_sync)]
        let runner: Arc<dyn AgentRunner> = Arc::new(CancellingRunner(cancellations.clone()));
        let recording = RecordingAgentRunner::new(
            runner,
            Arc::new(FailingState),
            Arc::new(UnusedEvidence),
            RunnerIdentity::new("recording-runner").expect("runner"),
            EpochSeconds::new(1),
        );
        assert!(matches!(
            recording.invoke(spec()),
            Err(AgentError::AdmissionRefused { .. })
        ));
        assert_eq!(
            *cancellations.borrow(),
            1,
            "unrecorded admission is cancelled, never polled"
        );
    }
}
