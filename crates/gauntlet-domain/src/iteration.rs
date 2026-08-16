//! `Iteration` — spec §5.3.4.
//!
//! One corrective cycle of a run: the selected correction, the role
//! invocations it dispatched, the observation/comparison/verification
//! artifacts it produced, the residuals it opened and closed, its payment,
//! and its durable checkpoint (I11). The domain type holds the shape and the
//! ownership invariants (an attached invocation belongs to this iteration of
//! this run); durability is the event log's job (plan Task 6/13).

use core::fmt;

use crate::role_invocation::{Payment, RoleInvocation};
use crate::values::{EpochSeconds, EvidenceRef, IterationNo, ResidualId, RunId, VerificationRef};

/// `Iteration` invariant failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IterationError {
    /// An invocation from another run or iteration was attached.
    InvocationMismatch {
        iteration_run: RunId,
        invocation_run: RunId,
        iteration_no: IterationNo,
        invocation_no: IterationNo,
    },
    /// A residual was closed without having been opened here.
    ResidualNotOpen { id: ResidualId },
}

impl fmt::Display for IterationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvocationMismatch {
                iteration_run,
                invocation_run,
                iteration_no,
                invocation_no,
            } => write!(
                f,
                "invocation belongs to run {invocation_run} iteration {invocation_no}, not run \
                 {iteration_run} iteration {iteration_no} (spec 5.3.4)"
            ),
            Self::ResidualNotOpen { id } => {
                write!(
                    f,
                    "residual {id} cannot be closed: it was not opened in this iteration"
                )
            }
        }
    }
}

impl std::error::Error for IterationError {}

/// One iteration of a run (spec §5.3.4).
#[derive(Debug, Clone)]
pub struct Iteration {
    run_id: RunId,
    no: IterationNo,
    selected_correction: Option<String>,
    role_invocations: Vec<RoleInvocation>,
    observation_bundle_ref: Option<EvidenceRef>,
    comparison_record_ref: Option<EvidenceRef>,
    verification_record_refs: Vec<VerificationRef>,
    residuals_opened: Vec<ResidualId>,
    residuals_closed: Vec<ResidualId>,
    payment: Payment,
    checkpoint_at: Option<(EpochSeconds, EvidenceRef)>,
}

impl Iteration {
    /// An iteration begins unbilled, with no invocations and no artifacts;
    /// the selected correction (§15) may still be undecided.
    pub fn new(run_id: RunId, no: IterationNo, selected_correction: Option<String>) -> Self {
        Self {
            run_id,
            no,
            selected_correction,
            role_invocations: Vec::new(),
            observation_bundle_ref: None,
            comparison_record_ref: None,
            verification_record_refs: Vec::new(),
            residuals_opened: Vec::new(),
            residuals_closed: Vec::new(),
            payment: Payment::zero(),
            checkpoint_at: None,
        }
    }

    pub fn run_id(&self) -> &RunId {
        &self.run_id
    }

    pub fn no(&self) -> IterationNo {
        self.no
    }

    /// What this iteration attempted, and why (spec §5.3.4 / §15).
    pub fn selected_correction(&self) -> Option<&str> {
        self.selected_correction.as_deref()
    }

    pub fn role_invocations(&self) -> &[RoleInvocation] {
        &self.role_invocations
    }

    /// Attaches a role invocation, refusing invocations that belong to
    /// another run or iteration (spec §5.3.4: an iteration owns its
    /// invocations).
    pub fn attach_invocation(&mut self, invocation: RoleInvocation) -> Result<(), IterationError> {
        if invocation.run_id() != &self.run_id || invocation.iteration_no() != self.no {
            return Err(IterationError::InvocationMismatch {
                iteration_run: self.run_id.clone(),
                invocation_run: invocation.run_id().clone(),
                iteration_no: self.no,
                invocation_no: invocation.iteration_no(),
            });
        }
        self.role_invocations.push(invocation);
        Ok(())
    }

    pub fn observation_bundle_ref(&self) -> Option<&EvidenceRef> {
        self.observation_bundle_ref.as_ref()
    }

    pub fn note_observation_bundle(&mut self, reference: EvidenceRef) {
        self.observation_bundle_ref = Some(reference);
    }

    pub fn comparison_record_ref(&self) -> Option<&EvidenceRef> {
        self.comparison_record_ref.as_ref()
    }

    pub fn note_comparison_record(&mut self, reference: EvidenceRef) {
        self.comparison_record_ref = Some(reference);
    }

    pub fn verification_record_refs(&self) -> &[VerificationRef] {
        &self.verification_record_refs
    }

    pub fn add_verification_record(&mut self, reference: VerificationRef) {
        self.verification_record_refs.push(reference);
    }

    pub fn residuals_opened(&self) -> &[ResidualId] {
        &self.residuals_opened
    }

    pub fn residuals_closed(&self) -> &[ResidualId] {
        &self.residuals_closed
    }

    /// Opens a residual against this iteration.
    pub fn open_residual(&mut self, id: ResidualId) {
        self.residuals_opened.push(id);
    }

    /// Closes a previously opened residual, moving it to the closed set.
    /// Closing a residual that was never opened here is a typed error: the
    /// two lists are the iteration's ledger, and they cannot disagree.
    pub fn close_residual(&mut self, id: ResidualId) -> Result<(), IterationError> {
        let Some(position) = self.residuals_opened.iter().position(|open| *open == id) else {
            return Err(IterationError::ResidualNotOpen { id });
        };
        self.residuals_opened.remove(position);
        self.residuals_closed.push(id);
        Ok(())
    }

    /// Mirrored payment counters (spec §5.3.4 `payment`), additive.
    pub fn payment(&self) -> &Payment {
        &self.payment
    }

    pub fn record_payment(&mut self, payment: &Payment) {
        self.payment = self.payment.plus(payment);
    }

    /// The durable checkpoint written at this iteration boundary (I11,
    /// AK-053): when it was taken, and where the checkpoint evidence lives.
    pub fn checkpoint_at(&self) -> Option<(EpochSeconds, EvidenceRef)> {
        self.checkpoint_at.clone()
    }

    pub fn checkpoint(&mut self, at: EpochSeconds, reference: EvidenceRef) {
        self.checkpoint_at = Some((at, reference));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::budget::MicroUsd;
    use crate::role_invocation::{Payment, RoleInvocation, RoleInvocationDefinition};
    use crate::values::{
        BlastRadius, Digest, EpochSeconds, EvidenceRef, InvocationId, PermissionSetRef,
        ResultContractPath, RoleId, RunnerIdentity, SessionRef, WorkspaceRef,
    };

    fn run_id() -> RunId {
        RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("valid run id")
    }

    fn other_run_id() -> RunId {
        RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAZ").expect("valid but other")
    }

    fn invocation(run: RunId, no: u32) -> RoleInvocation {
        let def = RoleInvocationDefinition {
            id: InvocationId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("valid id"),
            run_id: run,
            iteration_no: IterationNo::new(no).expect("1-based"),
            role: RoleId::Builder,
            runner: RunnerIdentity::new("runner-07").expect("valid"),
            model: None,
            permissions: PermissionSetRef::new("perm-readonly").expect("valid"),
            context_envelope_digest: Digest::new("sha256", "a1b2c3d4").expect("valid"),
            workspace_ref: WorkspaceRef::new("workspace-03").expect("valid"),
            tier: BlastRadius::Contained,
            sandbox_policy_ref: None,
            result_contract_path: ResultContractPath::new("result-contract").expect("valid"),
            admitted_at: None,
            session_ref: Some(SessionRef::new("session-0a").expect("valid")),
        };
        RoleInvocation::admit(def).expect("admits contained without policy")
    }

    #[test]
    fn invocations_belong_to_their_own_run_and_iteration() {
        // Spec §5.3.4: the iteration's `role_invocations` are ITS entities.
        let mut iteration = Iteration::new(run_id(), IterationNo::new(2).expect("1-based"), None);
        // Wrong run.
        assert!(matches!(
            iteration.attach_invocation(invocation(other_run_id(), 2)),
            Err(IterationError::InvocationMismatch { .. })
        ));
        // Wrong iteration number.
        assert!(matches!(
            iteration.attach_invocation(invocation(run_id(), 3)),
            Err(IterationError::InvocationMismatch { .. })
        ));
        assert!(iteration.role_invocations().is_empty());
        // Matching invocation attaches.
        iteration
            .attach_invocation(invocation(run_id(), 2))
            .expect("own invocation attaches");
        assert_eq!(iteration.role_invocations().len(), 1);
    }

    #[test]
    fn residual_ledgers_cannot_disagree() {
        let mut iteration = Iteration::new(run_id(), IterationNo::new(1).expect("1-based"), None);
        let residual = ResidualId::new("res-000007").expect("valid id");
        // Closing a residual that was never opened here is a typed error:
        // the two lists are a ledger.
        assert!(matches!(
            iteration.close_residual(residual.clone()),
            Err(IterationError::ResidualNotOpen { .. })
        ));
        iteration.open_residual(residual.clone());
        assert_eq!(
            iteration.residuals_opened(),
            std::slice::from_ref(&residual)
        );
        iteration
            .close_residual(residual.clone())
            .expect("opened residuals close");
        assert!(iteration.residuals_opened().is_empty());
        assert_eq!(iteration.residuals_closed(), &[residual]);
        // And never twice.
        assert!(matches!(
            iteration.close_residual(ResidualId::new("res-000007").expect("valid")),
            Err(IterationError::ResidualNotOpen { .. })
        ));
    }

    #[test]
    fn payment_accumulates_and_checkpoints_are_recorded() {
        let mut iteration = Iteration::new(
            run_id(),
            IterationNo::new(1).expect("1-based"),
            Some("why".to_owned()),
        );
        assert_eq!(iteration.selected_correction(), Some("why"));
        assert_eq!(*iteration.payment(), Payment::zero());
        iteration.record_payment(&Payment::new(10, 1, 2, MicroUsd::new(3)));
        iteration.record_payment(&Payment::new(5, 1, 1, MicroUsd::new(4)));
        assert_eq!(iteration.payment().tokens(), 15);
        assert_eq!(iteration.payment().cost_usd(), MicroUsd::new(7));
        assert_eq!(iteration.checkpoint_at(), None);
        let at = EpochSeconds::new(1_700_000_060);
        let reference = EvidenceRef::new("evd-checkpoint").expect("valid ref");
        iteration.checkpoint(at, reference.clone());
        assert_eq!(iteration.checkpoint_at(), Some((at, reference)));
        // Artifact refs are recordable and readable back.
        let bundle = EvidenceRef::new("evd-observation").expect("valid ref");
        iteration.note_observation_bundle(bundle.clone());
        assert_eq!(iteration.observation_bundle_ref(), Some(&bundle));
        let verification = crate::values::VerificationRef::new("vrf-000042").expect("valid");
        iteration.add_verification_record(verification.clone());
        assert_eq!(iteration.verification_record_refs(), &[verification]);
    }
}
