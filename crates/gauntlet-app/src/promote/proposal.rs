//! The optimizer surface (plan Task 23, charter D4's `proposal` piece;
//! spec §19.3): admission of a `candidate_revision@v1` result document as
//! a durable PENDING revision, and the `propose` path that dispatches the
//! REAL optimizer role invocation through the `AgentRunner` port.
//!
//! **The service struct holds ONLY the ledger and the clock** (charter
//! D4/D10) — structurally no configuration-write and no roles-install
//! path. The arch suite's containment scan polices the module's source;
//! the type system polices the handle: there is no field a config writer
//! or a roles installer could ride in on.
//!
//! **Admission** (`admit_document`): strict envelope decode (the codec's
//! registry validates the payload; a payload that claims a decision or
//! evaluation evidence was already refused at decode) → the role must be
//! the optimizer's → the declared state must be `completed` → the
//! revision id is DERIVED from the declared content (a content-addressed
//! id: the same document admitted twice is the same revision, so the
//! ledger's identity idempotency makes a replayed admission a no-op, and
//! a colliding id with disagreeing facts is the typed conflict) → the
//! durable PENDING proposal.
//!
//! **Dispatch** (`propose`): a SINGLE role invocation through the ports —
//! `invoke` returns an admission handle, never an answer (spec §6.1;
//! AGENTS.md delegation); the caller supplies the allowlisted context
//! envelope (role body + the run-history projection — nothing from the
//! role's `context.deny`); the service builds the spec, invokes, polls
//! under a bounded probe cap with the clock's tick between probes,
//! collects at the declared result-contract path only after `Completed`,
//! validates the §10.3 rule-6 identity, and admits the payload through
//! the SAME admission path an operator-supplied document takes.

use std::sync::Arc;

use gauntlet_domain::{
    Budget, InvocationId, IterationNo, ModelIdentity, PermissionSetRef, ResultContractPath,
    RevisionId, RoleId, RunId, SandboxPolicyRef, WorkspaceRef,
};
use gauntlet_ports::agent_runner::{
    AgentError, AgentRunner, ContextEnvelope, InvocationFailure, InvocationStatus,
    RoleInvocationSpec,
};
use gauntlet_ports::clock::Clock;
use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::id_generator::IdGenerator;
use gauntlet_ports::revision_ledger::{RevisionLedger, RevisionLedgerError, RevisionProposal};
use gauntlet_ports::role_result::{
    CandidateRevisionPayload, RolePayload, RoleResult, RoleResultError, RoleResultState,
};
use sha2::{Digest as _, Sha256};

/// The §19.3 gate over one revision ledger.
#[derive(Clone)]
pub struct ProposalService {
    ledger: Arc<dyn RevisionLedger>,
    clock: Arc<dyn Clock>,
}

/// One admission: the revision's content-derived id, and whether the
/// ledger already carried it (the idempotent no-op).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Admission {
    pub revision: RevisionId,
    pub idempotent: bool,
}

/// Why the optimizer surface refused — typed, with the locus (spec §21.2).
#[derive(Debug, Clone, PartialEq)]
pub enum ProposalError {
    /// The result document failed the strict decode. Nothing was admitted.
    Codec(RoleResultError),
    /// The document's declared state is not `completed` — a failed or
    /// runtime-terminated invocation produced no proposal.
    NotCompleted,
    /// The document does not carry the optimizer's payload schema.
    NotACandidateRevision { found: &'static str },
    /// The document speaks as a role other than the optimizer. The
    /// candidate-revision channel is the optimizer's only output.
    NotTheOptimizer,
    /// The ledger refused the admission or a later read.
    Ledger(RevisionLedgerError),
    /// The runner refused the admission of the optimizer invocation.
    Admission(AgentError),
    /// The invocation reached a non-completed terminal state.
    InvocationTerminal { detail: String },
    /// The invocation was still non-terminal at the bounded probe cap.
    Unresponsive {
        invocation: InvocationId,
        probes: u32,
    },
    /// A value the dispatch derives from established invariants (the
    /// 1-based first iteration, the hex revision id) refused to compose.
    /// Unreachable while the invariants hold; typed rather than panicked.
    Internal { detail: String },
}

impl core::fmt::Display for ProposalError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Codec(error) => write!(f, "the optimizer's result document is invalid: {error}"),
            Self::NotCompleted => write!(
                f,
                "the result document does not declare a completed state — a failed \
                 invocation produced no proposal"
            ),
            Self::NotACandidateRevision { found } => write!(
                f,
                "the result document carries the {found:?} payload, not \
                 candidate_revision@v1 — the optimizer proposes, it does not settle"
            ),
            Self::NotTheOptimizer => write!(
                f,
                "the result document does not speak as the optimizer role — the \
                 candidate-revision channel is the optimizer's only output"
            ),
            Self::Ledger(error) => write!(f, "the proposal admission failed: {error}"),
            Self::Admission(error) => {
                write!(f, "the optimizer invocation was not admitted: {error}")
            }
            Self::InvocationTerminal { detail } => {
                write!(
                    f,
                    "the optimizer invocation ended without a proposal: {detail}"
                )
            }
            Self::Unresponsive { invocation, probes } => write!(
                f,
                "invocation {invocation} was still in flight after {probes} bounded probes — \
                 the dispatch never awaits an answer; re-propose or raise the deadline"
            ),
            Self::Internal { detail } => {
                write!(
                    f,
                    "the proposal dispatch refused an impossible shape: {detail}"
                )
            }
        }
    }
}

impl std::error::Error for ProposalError {}

impl From<RoleResultError> for ProposalError {
    fn from(error: RoleResultError) -> Self {
        Self::Codec(error)
    }
}

/// The bounded probe cap: the dispatch NEVER awaits an answer (spec §6.1);
/// a non-terminal invocation at the cap is the typed `Unresponsive` — the
/// honest failure of a deadline-bounded poll loop, never a hang.
const PROBE_CAP: u32 = 64;
/// The poll tick's span, on its own line (numeric literals never share a
/// line with a duration-shaped identifier).
const POLL_TICK_SPAN: u64 = 1;

/// The dispatch plan the caller supplies: the allowlisted envelope (built
/// OUTSIDE this module — the role body and the run-history projection are
/// the composition's to assemble from the durable substrate, never from
/// the role's `context.deny`), the provenance run, and the dispatch
/// plumbing.
#[derive(Clone)]
pub struct ProposePlan {
    /// The run whose history seeds the proposal — the admission's
    /// provenance.
    pub run_id: RunId,
    pub envelope: ContextEnvelope,
    pub workspace: WorkspaceRef,
    pub permissions: PermissionSetRef,
    /// The slice of budget the optimizer invocation may consume.
    pub budget_slice: Budget,
    pub ids: Arc<dyn IdGenerator>,
    pub model_hint: Option<ModelIdentity>,
    pub sandbox_policy: Option<SandboxPolicyRef>,
}

impl ProposalService {
    pub fn new(ledger: Arc<dyn RevisionLedger>, clock: Arc<dyn Clock>) -> Self {
        Self { ledger, clock }
    }

    /// Admits an operator-supplied result document (`improve propose
    /// --from-result`): the SAME strict decode, never a bypass of
    /// validation — just of the model call (charter D6).
    pub fn admit_document(
        &self,
        document: &str,
        deadline: Deadline,
    ) -> Result<Admission, ProposalError> {
        let result = RoleResult::decode(document).map_err(ProposalError::Codec)?;
        self.admit_result(&result, None, deadline)
    }

    /// Dispatches the REAL optimizer role invocation and admits its result
    /// document (charter D4). Invoke → handle → bounded poll → collect at
    /// the declared result-contract path; the port's poll never blocks and
    /// the loop's bound is the probe cap — the dispatch never awaits an
    /// answer.
    pub fn propose(
        &self,
        runner: &dyn AgentRunner,
        plan: ProposePlan,
        deadline: Deadline,
    ) -> Result<Admission, ProposalError> {
        let invocation = plan.ids.invocation_id();
        let iteration = IterationNo::new(1).map_err(|_| ProposalError::Internal {
            detail: "the first iteration is 1-based; 1 always composes".to_owned(),
        })?;
        let contract =
            ResultContractPath::new(&format!("results/{invocation}/candidate_revision.json"))
                .map_err(|_| ProposalError::Internal {
                    detail: "the result-contract path composes from the invocation id".to_owned(),
                })?;
        let spec = RoleInvocationSpec {
            invocation_id: invocation.clone(),
            run_id: plan.run_id.clone(),
            iteration_no: iteration,
            role: RoleId::Optimizer,
            prompt: "read the run evidence; produce a candidate_revision@v1 proposal; you may \
                     not modify the active loop"
                .to_owned(),
            context_envelope: plan.envelope.clone(),
            workspace: plan.workspace,
            permissions: plan.permissions,
            result_contract_path: contract,
            deadline,
            model_hint: plan.model_hint,
            sandbox_policy: plan.sandbox_policy,
            budget_slice: plan.budget_slice,
        };
        let handle = runner.invoke(spec).map_err(ProposalError::Admission)?;
        let mut probes = 0u32;
        loop {
            let status = runner.poll(&handle).map_err(ProposalError::Admission)?;
            match status {
                InvocationStatus::Admitted | InvocationStatus::Running { .. } => {
                    probes += 1;
                    if probes >= PROBE_CAP {
                        return Err(ProposalError::Unresponsive { invocation, probes });
                    }
                    let now = self.clock.now();
                    self.clock
                        .sleep_until(gauntlet_domain::EpochSeconds::new(
                            now.get().saturating_add(POLL_TICK_SPAN),
                        ))
                        .map_err(|error| ProposalError::InvocationTerminal {
                            detail: format!("the poll tick failed: {error}"),
                        })?;
                }
                InvocationStatus::Completed { .. } => {
                    let result = runner
                        .collect(&handle, deadline)
                        .map_err(ProposalError::Admission)?;
                    // §10.3 rule 6: the collected document must carry THIS
                    // dispatch's identity and envelope digest.
                    result
                        .validate_against(&invocation, plan.envelope.digest())
                        .map_err(ProposalError::Codec)?;
                    return self.admit_result(&result, Some(plan.run_id), deadline);
                }
                InvocationStatus::Failed { class } => {
                    return Err(ProposalError::InvocationTerminal {
                        detail: format!("Failed{{class: {}}}", class_token(class)),
                    });
                }
                InvocationStatus::TimedOut => {
                    return Err(ProposalError::InvocationTerminal {
                        detail: "TimedOut — no schema-valid result document by the deadline"
                            .to_owned(),
                    });
                }
                InvocationStatus::Cancelled => {
                    return Err(ProposalError::InvocationTerminal {
                        detail: "Cancelled".to_owned(),
                    });
                }
            }
        }
    }

    /// The one admission path every document takes: strict-decoded
    /// envelope → the optimizer speaking as the optimizer → completed →
    /// the candidate payload → the content-derived revision id → the
    /// durable PENDING proposal.
    fn admit_result(
        &self,
        result: &RoleResult,
        source_run: Option<RunId>,
        deadline: Deadline,
    ) -> Result<Admission, ProposalError> {
        if result.state() != RoleResultState::Completed {
            return Err(ProposalError::NotCompleted);
        }
        if result.role() != &RoleId::Optimizer {
            return Err(ProposalError::NotTheOptimizer);
        }
        let payload = match result.payload() {
            RolePayload::CandidateRevision(payload) => payload,
            other => {
                return Err(ProposalError::NotACandidateRevision {
                    found: other.schema_token(),
                });
            }
        };
        let revision = revision_id_of(payload)?;
        let proposal = RevisionProposal::new(
            revision.clone(),
            payload.affected_component().to_owned(),
            payload.claimed_problem().to_owned(),
            payload.supporting_evidence().to_vec(),
            payload.predicted_improvement().to_owned(),
            payload.potential_regression().to_vec(),
            payload.candidate_change().clone(),
            payload.evaluation_plan().to_owned(),
            payload.held_out_evaluation().to_owned(),
            payload.rollback().clone(),
            self.clock.now(),
            source_run,
            format!("claimed problem: {}", payload.claimed_problem()),
        );
        let receipt = self
            .ledger
            .admit_proposal(proposal, deadline)
            .map_err(ProposalError::Ledger)?;
        Ok(Admission {
            revision,
            idempotent: receipt.idempotent,
        })
    }
}

/// The content-addressed revision id: `rev-` plus the first 16 hex digits
/// of SHA-256 over the payload's canonical declaration bytes. The same
/// document is the same revision (the ledger's idempotency makes a
/// replayed admission a no-op); a different document under a colliding id
/// is the ledger's typed `ProposalConflict`, never a silent overwrite.
fn revision_id_of(payload: &CandidateRevisionPayload) -> Result<RevisionId, ProposalError> {
    let mut hasher = Sha256::new();
    hasher.update(payload.affected_component().as_bytes());
    hasher.update([0]);
    hasher.update(payload.claimed_problem().as_bytes());
    hasher.update([0]);
    for reference in payload.supporting_evidence() {
        hasher.update(reference.as_str().as_bytes());
        hasher.update([0]);
    }
    hasher.update(payload.predicted_improvement().as_bytes());
    hasher.update([0]);
    for risk in payload.potential_regression() {
        hasher.update(risk.as_bytes());
        hasher.update([0]);
    }
    hasher.update(payload.candidate_change().as_str().as_bytes());
    hasher.update([0]);
    hasher.update(payload.evaluation_plan().as_bytes());
    hasher.update([0]);
    hasher.update(payload.held_out_evaluation().as_bytes());
    hasher.update([0]);
    hasher.update(payload.rollback().as_str().as_bytes());
    let digest_bytes = hasher.finalize();
    let mut hex = String::with_capacity(16);
    for byte in digest_bytes.iter().take(8) {
        hex.push_str(&format!("{byte:02x}"));
    }
    // A "rev-" prefix plus 16 hex digits fits the RevisionId grammar by
    // construction; the typed refusal exists so an impossible digest
    // shape is an error, never a panic.
    RevisionId::new(&format!("rev-{hex}")).map_err(|error| ProposalError::Internal {
        detail: format!("the content-derived revision id refused to compose: {error}"),
    })
}

fn class_token(class: InvocationFailure) -> &'static str {
    match class {
        InvocationFailure::MalformedResult => "malformed_result",
        InvocationFailure::ContractMismatch => "contract_mismatch",
        InvocationFailure::OrphanedByRestart => "orphaned_by_restart",
        InvocationFailure::Execution => "agent_execution",
        InvocationFailure::MissingResult => "missing_result",
    }
}
