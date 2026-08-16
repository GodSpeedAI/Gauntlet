//! `RoleInvocation` — spec §5.3.5.
//!
//! Key field notes (spec §5.3.5): `runner` is "adapter identity
//! (configuration, not architecture)"; `sandbox_policy_ref` is "null only
//! when tier=contained and backend=none"; `result_contract_path` is "where
//! the child MUST write its result (§10.3)". The backend half of the sandbox
//! rule is a configuration fact — enforced where the backend is knowable
//! (plan Task 14) — so the domain enforces the tier half: no sandbox policy
//! means contained tier, always.

use core::fmt;

use crate::budget::MicroUsd;
use crate::values::{
    BlastRadius, Digest, EpochSeconds, InvocationId, InvocationState, IterationNo, ModelIdentity,
    PermissionSetRef, ResultContractPath, RoleId, RunId, RunnerIdentity, SandboxPolicyRef,
    SessionRef, WorkspaceRef,
};

/// Payment for one invocation or iteration (spec §5.3.4 `payment`:
/// "tokens, wall seconds, tool calls, cost").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Payment {
    tokens: u64,
    wall_seconds: u64,
    tool_calls: u64,
    cost_usd: MicroUsd,
}

impl Payment {
    pub fn new(tokens: u64, wall_seconds: u64, tool_calls: u64, cost_usd: MicroUsd) -> Self {
        Self {
            tokens,
            wall_seconds,
            tool_calls,
            cost_usd,
        }
    }

    /// The additive identity — an iteration starts unbilled, stated
    /// explicitly rather than defaulted.
    pub fn zero() -> Self {
        Self::new(0, 0, 0, MicroUsd::new(0))
    }

    /// Sums two payments (mirrored counters accumulate).
    pub fn plus(&self, other: &Payment) -> Payment {
        Self::new(
            self.tokens + other.tokens,
            self.wall_seconds + other.wall_seconds,
            self.tool_calls + other.tool_calls,
            MicroUsd::new(self.cost_usd.get() + other.cost_usd.get()),
        )
    }

    pub fn tokens(&self) -> u64 {
        self.tokens
    }

    pub fn wall_seconds(&self) -> u64 {
        self.wall_seconds
    }

    pub fn tool_calls(&self) -> u64 {
        self.tool_calls
    }

    pub fn cost_usd(&self) -> MicroUsd {
        self.cost_usd
    }
}

/// `RoleInvocation` failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoleInvocationError {
    /// A sandbox policy ref is absent for a tier above contained (spec
    /// §5.3.5: "null only when tier=contained and backend=none").
    SandboxPolicyRequiredForTier { tier: BlastRadius },
    /// A state change the invocation's state does not permit.
    TransitionInvalid {
        from: InvocationState,
        to: InvocationState,
    },
    /// Payment was recorded on an invocation that is already terminal.
    PaymentOnTerminalInvocation { state: InvocationState },
}

impl fmt::Display for RoleInvocationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SandboxPolicyRequiredForTier { tier } => write!(
                f,
                "a {tier} invocation requires a sandbox policy ref; it may be absent only for \
                 the contained tier (spec 5.3.5)"
            ),
            Self::TransitionInvalid { from, to } => {
                write!(f, "invocation cannot move {from} to {to}")
            }
            Self::PaymentOnTerminalInvocation { state } => {
                write!(f, "payment cannot be recorded on a {state} invocation")
            }
        }
    }
}

impl std::error::Error for RoleInvocationError {}

/// Definition data for admitting a [`RoleInvocation`] (spec §5.3.5 shape).
#[derive(Debug, Clone)]
pub struct RoleInvocationDefinition {
    pub id: InvocationId,
    pub run_id: RunId,
    pub iteration_no: IterationNo,
    pub role: RoleId,
    pub runner: RunnerIdentity,
    pub model: Option<ModelIdentity>,
    pub permissions: PermissionSetRef,
    pub context_envelope_digest: Digest,
    pub workspace_ref: WorkspaceRef,
    pub tier: BlastRadius,
    pub sandbox_policy_ref: Option<SandboxPolicyRef>,
    pub result_contract_path: ResultContractPath,
    pub admitted_at: Option<EpochSeconds>,
    pub session_ref: Option<SessionRef>,
}

/// One admitted delegated role (spec §5.3.5). Enters the world in the
/// `admitted` state and moves through a validated transition table; there is
/// no method that skips a state or resurrects a terminal one.
#[derive(Debug, Clone)]
pub struct RoleInvocation {
    id: InvocationId,
    run_id: RunId,
    iteration_no: IterationNo,
    role: RoleId,
    runner: RunnerIdentity,
    model: Option<ModelIdentity>,
    permissions: PermissionSetRef,
    context_envelope_digest: Digest,
    workspace_ref: WorkspaceRef,
    tier: BlastRadius,
    sandbox_policy_ref: Option<SandboxPolicyRef>,
    result_contract_path: ResultContractPath,
    state: InvocationState,
    admitted_at: Option<EpochSeconds>,
    settled_at: Option<EpochSeconds>,
    payment: Option<Payment>,
    session_ref: Option<SessionRef>,
}

impl RoleInvocation {
    /// Admits an invocation (spec §5.3.5 `state: admitted`). Enforces the
    /// sandbox/tier coupling: above the contained tier, a sandbox policy ref
    /// is required.
    pub fn admit(def: RoleInvocationDefinition) -> Result<Self, RoleInvocationError> {
        if def.sandbox_policy_ref.is_none() && def.tier != BlastRadius::Contained {
            return Err(RoleInvocationError::SandboxPolicyRequiredForTier { tier: def.tier });
        }
        Ok(Self {
            id: def.id,
            run_id: def.run_id,
            iteration_no: def.iteration_no,
            role: def.role,
            runner: def.runner,
            model: def.model,
            permissions: def.permissions,
            context_envelope_digest: def.context_envelope_digest,
            workspace_ref: def.workspace_ref,
            tier: def.tier,
            sandbox_policy_ref: def.sandbox_policy_ref,
            result_contract_path: def.result_contract_path,
            state: InvocationState::Admitted,
            admitted_at: def.admitted_at,
            settled_at: None,
            payment: None,
            session_ref: def.session_ref,
        })
    }

    pub fn id(&self) -> &InvocationId {
        &self.id
    }

    pub fn run_id(&self) -> &RunId {
        &self.run_id
    }

    pub fn iteration_no(&self) -> IterationNo {
        self.iteration_no
    }

    pub fn role(&self) -> &RoleId {
        &self.role
    }

    pub fn runner(&self) -> &RunnerIdentity {
        &self.runner
    }

    pub fn model(&self) -> Option<&ModelIdentity> {
        self.model.as_ref()
    }

    pub fn permissions(&self) -> &PermissionSetRef {
        &self.permissions
    }

    /// Digest of exactly what was sent to the role (I4 evidence).
    pub fn context_envelope_digest(&self) -> &Digest {
        &self.context_envelope_digest
    }

    pub fn workspace_ref(&self) -> &WorkspaceRef {
        &self.workspace_ref
    }

    /// The containment tier this invocation was dispatched under.
    pub fn tier(&self) -> BlastRadius {
        self.tier
    }

    pub fn sandbox_policy_ref(&self) -> Option<&SandboxPolicyRef> {
        self.sandbox_policy_ref.as_ref()
    }

    pub fn result_contract_path(&self) -> &ResultContractPath {
        &self.result_contract_path
    }

    pub fn state(&self) -> InvocationState {
        self.state
    }

    pub fn admitted_at(&self) -> Option<EpochSeconds> {
        self.admitted_at
    }

    pub fn settled_at(&self) -> Option<EpochSeconds> {
        self.settled_at
    }

    pub fn payment(&self) -> Option<&Payment> {
        self.payment.as_ref()
    }

    pub fn session_ref(&self) -> Option<&SessionRef> {
        self.session_ref.as_ref()
    }

    fn transition(&mut self, to: InvocationState) -> Result<(), RoleInvocationError> {
        use InvocationState::*;
        let from = self.state;
        let allowed = matches!(
            (from, to),
            (Admitted, Running)
                | (Admitted, Cancelled)
                | (Running, Completed)
                | (Running, Failed)
                | (Running, TimedOut)
                | (Running, Cancelled)
        );
        if !allowed {
            return Err(RoleInvocationError::TransitionInvalid { from, to });
        }
        self.state = to;
        Ok(())
    }

    /// Moves the invocation into `running`.
    pub fn mark_running(&mut self) -> Result<(), RoleInvocationError> {
        self.transition(InvocationState::Running)
    }

    /// Completes the invocation, recording its payment and settlement time.
    /// Completion of the CALL is a transport fact; completion of the WORK is
    /// a result-contract fact decided elsewhere (AK-009, plan Task 4).
    pub fn complete(
        &mut self,
        payment: Payment,
        settled_at: EpochSeconds,
    ) -> Result<(), RoleInvocationError> {
        self.transition(InvocationState::Completed)?;
        self.payment = Some(payment);
        self.settled_at = Some(settled_at);
        Ok(())
    }

    /// Fails the invocation, recording its payment and settlement time.
    pub fn fail(
        &mut self,
        payment: Payment,
        settled_at: EpochSeconds,
    ) -> Result<(), RoleInvocationError> {
        self.transition(InvocationState::Failed)?;
        self.payment = Some(payment);
        self.settled_at = Some(settled_at);
        Ok(())
    }

    /// Times the invocation out, recording its payment and settlement time.
    pub fn time_out(
        &mut self,
        payment: Payment,
        settled_at: EpochSeconds,
    ) -> Result<(), RoleInvocationError> {
        self.transition(InvocationState::TimedOut)?;
        self.payment = Some(payment);
        self.settled_at = Some(settled_at);
        Ok(())
    }

    /// Cancels the invocation.
    pub fn cancel(&mut self) -> Result<(), RoleInvocationError> {
        self.transition(InvocationState::Cancelled)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_id() -> RunId {
        RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("valid run id")
    }

    fn invocation_id() -> InvocationId {
        InvocationId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("valid invocation id")
    }

    fn envelope_digest() -> Digest {
        Digest::new("sha256", "a1b2c3d4").expect("valid parts")
    }

    fn definition(
        tier: BlastRadius,
        sandbox: Option<SandboxPolicyRef>,
    ) -> RoleInvocationDefinition {
        RoleInvocationDefinition {
            id: invocation_id(),
            run_id: run_id(),
            iteration_no: IterationNo::new(1).expect("first iteration"),
            role: RoleId::Builder,
            runner: RunnerIdentity::new("runner-07").expect("valid identity"),
            model: Some(ModelIdentity::new("model-large-02").expect("valid identity")),
            permissions: PermissionSetRef::new("perm-readonly").expect("valid ref"),
            context_envelope_digest: envelope_digest(),
            workspace_ref: WorkspaceRef::new("workspace-03").expect("valid ref"),
            tier,
            sandbox_policy_ref: sandbox,
            result_contract_path: ResultContractPath::new("result-contract").expect("valid path"),
            admitted_at: Some(EpochSeconds::new(1_700_000_000)),
            session_ref: Some(SessionRef::new("session-0a").expect("valid ref")),
        }
    }

    fn policy() -> SandboxPolicyRef {
        SandboxPolicyRef::new("sandbox-strict").expect("valid ref")
    }

    #[test]
    fn above_contained_tier_a_sandbox_policy_is_required() {
        // Spec §5.3.5: `sandbox_policy_ref` is "null only when
        // tier=contained and backend=none". The domain enforces the tier
        // half; the backend half is configuration (plan Task 14). Every tier
        // above contained is probed, irreversible included — the tier where
        // an unsandboxed dispatch is most dangerous is not the one to leave
        // untested.
        for tier in [
            BlastRadius::Local,
            BlastRadius::CrossCutting,
            BlastRadius::Irreversible,
        ] {
            assert!(
                matches!(
                    RoleInvocation::admit(definition(tier, None)),
                    Err(RoleInvocationError::SandboxPolicyRequiredForTier { .. })
                ),
                "policy-less {tier} admission must be refused"
            );
        }
        let _ = RoleInvocation::admit(definition(BlastRadius::Contained, None)).expect("contained");
        let _ = RoleInvocation::admit(definition(BlastRadius::Local, Some(policy())))
            .expect("sandboxed local");
        // The legal shape at the irreversible tier: policy present, admitted.
        let irreversible =
            RoleInvocation::admit(definition(BlastRadius::Irreversible, Some(policy())))
                .expect("a sandboxed irreversible invocation is admittable");
        assert_eq!(irreversible.tier(), BlastRadius::Irreversible);
    }

    #[test]
    fn admission_records_the_dispatch_facts() {
        let invocation = RoleInvocation::admit(definition(BlastRadius::Local, Some(policy())))
            .expect("admitted");
        assert_eq!(invocation.state(), InvocationState::Admitted);
        assert_eq!(invocation.run_id(), &run_id());
        assert_eq!(invocation.iteration_no().get(), 1);
        assert_eq!(invocation.role(), &RoleId::Builder);
        assert_eq!(invocation.context_envelope_digest(), &envelope_digest());
        assert_eq!(
            invocation.result_contract_path().as_str(),
            "result-contract"
        );
        assert_eq!(invocation.tier(), BlastRadius::Local);
        assert_eq!(invocation.payment(), None);
    }

    #[test]
    fn the_state_machine_has_no_shortcuts_or_resurrections() {
        let payment = Payment::new(10, 1, 2, MicroUsd::new(3));
        let at = EpochSeconds::new(1_700_000_060);

        // No skipping states.
        let mut invocation =
            RoleInvocation::admit(definition(BlastRadius::Contained, None)).expect("admitted");
        assert!(matches!(
            invocation.complete(payment, at),
            Err(RoleInvocationError::TransitionInvalid { .. })
        ));

        // Happy path: admitted → running → completed.
        let mut invocation =
            RoleInvocation::admit(definition(BlastRadius::Contained, None)).expect("admitted");
        invocation.mark_running().expect("starts");
        invocation.complete(payment, at).expect("completes");
        assert_eq!(invocation.state(), InvocationState::Completed);
        assert_eq!(invocation.payment(), Some(&payment));
        assert_eq!(invocation.settled_at(), Some(at));

        // Terminal states are terminal.
        assert!(matches!(
            invocation.mark_running(),
            Err(RoleInvocationError::TransitionInvalid { .. })
        ));
        assert!(matches!(
            invocation.fail(payment, at),
            Err(RoleInvocationError::TransitionInvalid { .. })
        ));

        // Fail and time-out paths from running.
        let mut invocation =
            RoleInvocation::admit(definition(BlastRadius::Contained, None)).expect("admitted");
        invocation.mark_running().expect("starts");
        invocation.fail(payment, at).expect("fails");
        assert_eq!(invocation.state(), InvocationState::Failed);

        let mut invocation =
            RoleInvocation::admit(definition(BlastRadius::Contained, None)).expect("admitted");
        invocation.mark_running().expect("starts");
        invocation.time_out(payment, at).expect("times out");
        assert_eq!(invocation.state(), InvocationState::TimedOut);

        // Cancellation from admitted and from running.
        let mut invocation =
            RoleInvocation::admit(definition(BlastRadius::Contained, None)).expect("admitted");
        invocation.cancel().expect("admitted work may be cancelled");
        assert_eq!(invocation.state(), InvocationState::Cancelled);
        let mut invocation =
            RoleInvocation::admit(definition(BlastRadius::Contained, None)).expect("admitted");
        invocation.mark_running().expect("starts");
        invocation.cancel().expect("running work may be cancelled");
    }

    #[test]
    fn payments_add_up() {
        let a = Payment::new(10, 1, 2, MicroUsd::new(3));
        let b = Payment::new(5, 1, 1, MicroUsd::new(4));
        let sum = a.plus(&b);
        assert_eq!(sum, Payment::new(15, 2, 3, MicroUsd::new(7)));
        assert_eq!(Payment::zero().plus(&a), a);
    }
}
