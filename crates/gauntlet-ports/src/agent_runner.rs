//! `AgentRunner` — the admission-based agent port (spec §6.1, decision D1).
//!
//! Spec §6.1, quoted where a rule decides a signature:
//!
//! - "`invoke(spec: RoleInvocationSpec) -> Result<InvocationHandle, AgentError>`"
//!   and "`invoke` MUST return promptly after admission. It MUST NOT block on
//!   completion."
//! - "`poll(handle: &InvocationHandle) -> Result<InvocationStatus, AgentError>`"
//!   — a non-blocking peek; the deadline rule in the §6 preamble ("Every port
//!   that can block has a deadline parameter") does not apply because poll
//!   MUST NOT block.
//! - "`collect(handle: &InvocationHandle) -> Result<RoleResult, AgentError>`  #
//!   after Completed" — reading the result document is IO, so per the §6
//!   preamble `collect` takes a deadline parameter in addition to the §6.1
//!   shape.
//! - "`cancel(handle: &InvocationHandle, reason)`" — cancellation talks to a
//!   possibly-remote process, so it also carries a deadline.
//! - "`capabilities() -> RunnerCapabilities`".
//!
//! No method returns a role result from `invoke`: results arrive out-of-band
//! through the declared result contract (§10.3), which is what makes
//! fan-in uniform across harnesses (D1, AK-009).

use core::fmt;

use gauntlet_domain::{
    Budget, Digest, EpochSeconds, InvocationId, InvocationState, IterationNo, ModelIdentity,
    Payment, PermissionSetRef, ResultContractPath, RoleId, RunId, SandboxPolicyRef, SessionRef,
    WorkspaceRef,
};

use crate::deadline::Deadline;
use crate::role_result::RoleResult;

/// `AgentRunner` failures — typed, carrying the invocation locus (spec
/// §21.2: "Typed errors carrying the failure locus; never a bare string,
/// never a swallowed error").
#[derive(Debug, Clone, PartialEq)]
pub enum AgentError {
    /// Admission itself failed (runner not configured, spec unsupported).
    AdmissionRefused { detail: String },
    /// The handle was never admitted by this runner, or is no longer known
    /// to it.
    UnknownHandle { invocation_id: InvocationId },
    /// Communication with the runner failed. The invocation's fate may be
    /// unknown — recovery re-probes result contracts (§18), never assumes.
    Transport { detail: String },
    /// `collect` was called on an invocation that is not `Completed`
    /// (§6.1: collect is "# after Completed").
    ResultNotReady {
        invocation_id: InvocationId,
        state: InvocationState,
    },
    /// The result document existed but could not be read or returned.
    CollectionFailed {
        invocation_id: InvocationId,
        detail: String,
    },
    /// The invocation is already in a terminal state; the requested
    /// operation has no effect. A typed no-op, not a failure of the caller
    /// (idempotent cancellation returns this on the second call when the
    /// adapter chooses not to return `Ok`).
    AlreadyTerminal {
        invocation_id: InvocationId,
        state: InvocationState,
    },
    /// Cancellation could not be performed (not the idempotent case — that
    /// is [`AgentError::AlreadyTerminal`]).
    CancelFailed {
        invocation_id: InvocationId,
        detail: String,
    },
}

impl fmt::Display for AgentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AdmissionRefused { detail } => {
                write!(f, "admission refused: {detail}")
            }
            Self::UnknownHandle { invocation_id } => {
                write!(f, "no invocation {invocation_id} is known to this runner")
            }
            Self::Transport { detail } => write!(f, "runner transport failed: {detail}"),
            Self::ResultNotReady {
                invocation_id,
                state,
            } => write!(
                f,
                "cannot collect {invocation_id} in state {state} — collect is after completed"
            ),
            Self::CollectionFailed {
                invocation_id,
                detail,
            } => write!(
                f,
                "collecting the result of {invocation_id} failed: {detail}"
            ),
            Self::AlreadyTerminal {
                invocation_id,
                state,
            } => write!(
                f,
                "{invocation_id} is already terminal ({state}); the operation is a no-op"
            ),
            Self::CancelFailed {
                invocation_id,
                detail,
            } => write!(f, "cancelling {invocation_id} failed: {detail}"),
        }
    }
}

impl std::error::Error for AgentError {}

/// The `Failed{class}` vocabulary of `InvocationStatus` (spec §6.1) — the
/// failure classes the spec names for invocations:
///
/// - `malformed_result` and `contract_mismatch` — §10.3 rules 4 and 6;
/// - `orphaned_by_restart` — §18 recovery: "anything else is marked
///   `Failed{class: orphaned_by_restart}` and re-planned";
/// - `Execution` — the §21.1 `agent_execution` locus without a finer class;
/// - `missing_result` — plan Task 16 (the §4.1 command-template runner): the child
///   process EXITED without writing a valid result contract. A dead child
///   can never write, so the verdict is reachable BEFORE the deadline —
///   this is a different never-write shape from §10.3 rule 5's `TimedOut`
///   (the child still RUNNING, silent, at its deadline). Exit status is
///   liveness evidence only (§6.1, §10.3): a zero exit with no valid
///   contract is still `missing_result`, and a nonzero exit WITH a valid,
///   matching contract is still `Completed`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvocationFailure {
    MalformedResult,
    ContractMismatch,
    OrphanedByRestart,
    Execution,
    MissingResult,
}

impl fmt::Display for InvocationFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let token = match self {
            Self::MalformedResult => "malformed_result",
            Self::ContractMismatch => "contract_mismatch",
            Self::OrphanedByRestart => "orphaned_by_restart",
            Self::Execution => "agent_execution",
            Self::MissingResult => "missing_result",
        };
        f.write_str(token)
    }
}

/// Spec §6.1: `InvocationStatus := Admitted | Running{last_event_at,
/// payment_so_far} | Completed{result_ref} | Failed{class} | TimedOut |
/// Cancelled`.
///
/// `result_ref` names the result document at the invocation's declared
/// result-contract path (§10.3): completion is a result-contract fact, never
/// a mailbox signal.
#[derive(Debug, Clone, PartialEq)]
pub enum InvocationStatus {
    Admitted,
    Running {
        last_event_at: EpochSeconds,
        payment_so_far: Payment,
    },
    Completed {
        result_ref: ResultContractPath,
    },
    Failed {
        class: InvocationFailure,
    },
    TimedOut,
    Cancelled,
}

impl InvocationStatus {
    /// Whether this status is one of the four terminal states. Once
    /// terminal, an invocation reports the SAME status forever (pinned
    /// post-terminal semantics; see the [`AgentRunner::poll`] contract).
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Completed { .. } | Self::Failed { .. } | Self::TimedOut | Self::Cancelled
        )
    }
}

/// Spec §6.1 `RunnerCapabilities`:
///
/// ```text
/// supports_native_subagents, supports_streaming_events,
/// supports_model_selection, isolation: none|process|container|vm,
/// result_delivery: file|mailbox|return_value, max_depth: u8
/// ```
///
/// "`capabilities()` drives scheduling decisions. A runner reporting
/// `isolation: none` MUST NOT be selected for a tier requiring containment
/// (§17.3)" — that check is the scheduler's; the capabilities type is what
/// it runs on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunnerCapabilities {
    /// Can it fan out inside itself?
    pub supports_native_subagents: bool,
    pub supports_streaming_events: bool,
    pub supports_model_selection: bool,
    pub isolation: Isolation,
    pub result_delivery: ResultDelivery,
    pub max_depth: u8,
}

/// The containment ladder (spec §6.1 `isolation: none | process | container
/// | vm`). Declaration order is the ladder: each rung contains at least what
/// the rung below it does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Isolation {
    None,
    Process,
    Container,
    Vm,
}

/// How a runner's results are delivered (spec §6.1 `result_delivery: file |
/// mailbox | return_value`). Whatever the delivery, the settlement-bearing
/// artifact is the result contract file (§10.3); mailbox and return values
/// are signals.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResultDelivery {
    File,
    Mailbox,
    ReturnValue,
}

/// The context envelope handed to a role: the runtime-built allowlisted
/// context (I4) plus the digest of exactly what was sent. The runner passes
/// the body to the child and never computes the digest — hashing is an
/// instrument's job, and the digest is what the result contract must echo
/// back (§10.3 rule 6).
#[derive(Debug, Clone, PartialEq)]
pub struct ContextEnvelope {
    body: String,
    digest: Digest,
}

impl ContextEnvelope {
    /// Adopts the serialized envelope and the digest computed over it.
    pub fn new(body: String, digest: Digest) -> Self {
        Self { body, digest }
    }

    pub fn body(&self) -> &str {
        &self.body
    }

    pub fn digest(&self) -> &Digest {
        &self.digest
    }
}

/// Spec §6.1 `RoleInvocationSpec` — the ten §6.1 fields (`role, prompt,
/// context_envelope, workspace, permissions, result_contract_path, deadline,
/// model_hint, sandbox_policy, budget_slice`) plus the dispatch identity
/// (`invocation_id`, `run_id`, `iteration_no`) every result document must
/// echo: without them the §10.3 rule 6 mismatch tripwire would be
/// uncheckable at the runner.
#[derive(Debug, Clone)]
pub struct RoleInvocationSpec {
    pub invocation_id: InvocationId,
    pub run_id: RunId,
    pub iteration_no: IterationNo,
    pub role: RoleId,
    pub prompt: String,
    pub context_envelope: ContextEnvelope,
    pub workspace: WorkspaceRef,
    pub permissions: PermissionSetRef,
    pub result_contract_path: ResultContractPath,
    pub deadline: Deadline,
    pub model_hint: Option<ModelIdentity>,
    pub sandbox_policy: Option<SandboxPolicyRef>,
    /// The slice of the run budget this invocation may consume — the run's
    /// whole budget travels on the invocation and the meter derives slices
    /// from it at approval; the runner's spend must stay within the slice.
    pub budget_slice: Budget,
}

/// An admission handle (spec §6.1). Carries the invocation's identity and —
/// when the harness provides one — the session reference, which is forensic
/// evidence only, never control state (I11, Annex C.2: "Record
/// `session_dir` as `session_ref` only").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvocationHandle {
    invocation_id: InvocationId,
    session_ref: Option<SessionRef>,
}

impl InvocationHandle {
    pub fn new(invocation_id: InvocationId, session_ref: Option<SessionRef>) -> Self {
        Self {
            invocation_id,
            session_ref,
        }
    }

    pub fn invocation_id(&self) -> &InvocationId {
        &self.invocation_id
    }

    pub fn session_ref(&self) -> Option<&SessionRef> {
        self.session_ref.as_ref()
    }
}

/// Why an invocation is being cancelled. The spec names cancellation causes
/// across §10.3/§11/§18 but fixes no closed vocabulary, so the port carries
/// the reason as an opaque non-empty token — adapters log it; nothing
/// branches on it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CancelReason {
    reason: String,
}

/// A cancellation reason must be present to be forensic evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CancelReasonInvalid {
    pub input: String,
}

impl fmt::Display for CancelReasonInvalid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "a cancellation reason must not be empty; {:?} carries no forensic value",
            self.input
        )
    }
}

impl std::error::Error for CancelReasonInvalid {}

impl CancelReason {
    pub fn new(reason: &str) -> Result<Self, CancelReasonInvalid> {
        if reason.trim().is_empty() {
            return Err(CancelReasonInvalid {
                input: reason.to_owned(),
            });
        }
        Ok(Self {
            reason: reason.to_owned(),
        })
    }

    pub fn as_str(&self) -> &str {
        &self.reason
    }
}

impl Isolation {
    /// The §6.1 wire token.
    pub fn token(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Process => "process",
            Self::Container => "container",
            Self::Vm => "vm",
        }
    }

    /// Exact-match parse; unknown tokens are typed errors, never coerced.
    pub fn parse(input: &str) -> Result<Self, crate::tokens::UnknownToken> {
        match input {
            "none" => Ok(Self::None),
            "process" => Ok(Self::Process),
            "container" => Ok(Self::Container),
            "vm" => Ok(Self::Vm),
            other => Err(crate::tokens::UnknownToken {
                vocabulary: "Isolation",
                value: other.to_owned(),
            }),
        }
    }
}

impl fmt::Display for Isolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.token())
    }
}

impl ResultDelivery {
    /// The §6.1 wire token.
    pub fn token(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Mailbox => "mailbox",
            Self::ReturnValue => "return_value",
        }
    }

    /// Exact-match parse; unknown tokens are typed errors, never coerced.
    pub fn parse(input: &str) -> Result<Self, crate::tokens::UnknownToken> {
        match input {
            "file" => Ok(Self::File),
            "mailbox" => Ok(Self::Mailbox),
            "return_value" => Ok(Self::ReturnValue),
            other => Err(crate::tokens::UnknownToken {
                vocabulary: "ResultDelivery",
                value: other.to_owned(),
            }),
        }
    }
}

impl fmt::Display for ResultDelivery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.token())
    }
}

/// The admission-based agent port (spec §6.1). See the module docs for the
/// signature decisions and their spec citations.
///
/// Pinned semantics (the shared contract suite enforces them):
///
/// - `invoke` returns after admission; the first `poll` on work that is
///   still running MUST NOT report a terminal state.
/// - A terminal status is reported stably: once `poll` reports a terminal
///   status for a handle, every later `poll` returns the SAME terminal
///   status — a second transition (to any state, including back to
///   `Running`) is a contract violation. "Exactly once" is thereby
///   observable: poll may be called any number of times, but it never
///   reports a second terminal transition.
/// - `cancel` is idempotent: the first call performs the cancellation;
///   later calls return `Ok(())` or [`AgentError::AlreadyTerminal`] and
///   never apply a second effect or change the reported state.
/// - Completion requires a schema-valid result document at the declared
///   path (§10.3 rule 3): no document by the deadline is `TimedOut` (rule
///   5), a malformed one is `Failed{malformed_result}` (rule 4), a
///   mismatched `invocation_id`/`context_envelope_digest` is
///   `Failed{contract_mismatch}` (rule 6).
pub trait AgentRunner {
    /// Admits an invocation. Returns promptly after admission; never blocks
    /// on completion and never returns the role's result (spec §6.1).
    fn invoke(&self, spec: RoleInvocationSpec) -> Result<InvocationHandle, AgentError>;

    /// A non-blocking status peek (spec §6.1). MUST NOT block; a blocking
    /// wait belongs to the caller, bounded by the spec's deadline.
    fn poll(&self, handle: &InvocationHandle) -> Result<InvocationStatus, AgentError>;

    /// Returns the validated result document, only after `Completed`
    /// (spec §6.1). The deadline parameter satisfies the §6 preamble for
    /// this blocking read.
    fn collect(
        &self,
        handle: &InvocationHandle,
        deadline: Deadline,
    ) -> Result<RoleResult, AgentError>;

    /// Cancels the invocation (spec §6.1). Idempotent — see the trait docs.
    /// The deadline parameter bounds this blocking control operation.
    fn cancel(
        &self,
        handle: &InvocationHandle,
        reason: CancelReason,
        deadline: Deadline,
    ) -> Result<(), AgentError>;

    /// The runner's capabilities (spec §6.1). Drives scheduling decisions;
    /// cannot block.
    fn capabilities(&self) -> RunnerCapabilities;
}
