//! `Sandbox` — spec §6.2: "`validate(policy) -> PolicyValidation` (dry run) ·
//! `wrap(command, policy) -> SandboxedCommand` · `backend_identity()` —
//! Validation precedes every use. Failure escalates; it never falls back to
//! unsandboxed."
//!
//! "Validation precedes every use" is enforced structurally: `wrap` requires
//! the [`PolicyValidation`] that `validate` produced for the SAME policy, so
//! a wrapped command cannot exist without a validation in hand. Backend
//! identities are opaque refs resolved from configuration (§8.2) — product
//! spellings belong to adapters and configuration, never to port source
//! (§24.3; the boundary suite denies them here).

use core::fmt;
use core::str::FromStr;

use gauntlet_domain::{BlastRadius, SandboxPolicyRef};

use crate::agent_runner::Isolation;
use crate::deadline::Deadline;

/// A configured sandbox backend identifier — opaque, like the domain's
/// other configuration refs. Constructed from configuration strings only.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SandboxBackendRef {
    backend: String,
}

impl SandboxBackendRef {
    pub fn new(backend: &str) -> Result<Self, SandboxError> {
        let valid = !backend.is_empty()
            && backend.len() <= 64
            && !backend.starts_with('/')
            && !backend.bytes().any(|b| b.is_ascii_whitespace());
        if valid {
            Ok(Self {
                backend: backend.to_owned(),
            })
        } else {
            Err(SandboxError::IdentityInvalid {
                identity: backend.to_owned(),
            })
        }
    }

    pub fn as_str(&self) -> &str {
        &self.backend
    }
}

impl fmt::Display for SandboxBackendRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.backend)
    }
}

impl FromStr for SandboxBackendRef {
    type Err = SandboxError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        Self::new(input)
    }
}

/// Network posture under containment: deny-by-default, or an explicit host
/// allowlist "instead of blanket network only if the claim requires it"
/// (spec §17.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetworkPosture {
    DenyByDefault,
    Allowlist { hosts: Vec<String> },
}

/// A per-tier sandbox policy (spec §17.3). "Write a policy per tier. Never
/// ship a backend's bundled default policy unmodified." There is no policy
/// for the `irreversible` tier at all — that absence is a requirement, not
/// an omission (§17.3; `PolicyProvider` refuses to resolve one).
#[derive(Debug, Clone, PartialEq)]
pub struct SandboxPolicy {
    pub reference: SandboxPolicyRef,
    pub tier: BlastRadius,
    pub backend: SandboxBackendRef,
    pub network: NetworkPosture,
    /// Where inside (or relative to) the workspace writes are permitted.
    pub writable_roots: Vec<String>,
    /// Paths explicitly denied even if otherwise writable — credential
    /// paths and the repository's version-control directory at the `local`
    /// tier (spec §17.3).
    pub denied_paths: Vec<String>,
}

/// The result of a dry-run validation (spec §6.2 `PolicyValidation`): the
/// policy and backend validated, the containment rung the backend actually
/// achieves, and any findings. A validation with findings is a failure —
/// escalation, never a fallback to unsandboxed (§21.3).
#[derive(Debug, Clone, PartialEq)]
pub struct PolicyValidation {
    pub policy: SandboxPolicyRef,
    pub backend: SandboxBackendRef,
    /// The containment rung this backend achieves for this policy — the
    /// fact §17.3's "Runner must report `isolation ≥ process`" checks
    /// against.
    pub isolation: Isolation,
    pub findings: Vec<String>,
}

impl PolicyValidation {
    /// Whether validation succeeded (no findings).
    pub fn is_clean(&self) -> bool {
        self.findings.is_empty()
    }
}

/// A command wrapped for execution under a validated policy.
#[derive(Debug, Clone, PartialEq)]
pub struct SandboxedCommand {
    pub command: String,
    pub policy: SandboxPolicyRef,
    /// The validation that authorized the wrap — carried so the executing
    /// layer can prove validation preceded use.
    pub validation: PolicyValidation,
}

/// `Sandbox` failures — typed (spec §21.2). A validation failure ESCALATES;
/// it never falls back to unsandboxed execution (§6.2, §21.3).
#[derive(Debug, Clone, PartialEq)]
pub enum SandboxError {
    /// Dry-run validation found problems. The policy is not usable; the
    /// finding list says why.
    ValidationFailed {
        policy: SandboxPolicyRef,
        findings: Vec<String>,
    },
    /// The requested backend cannot run here.
    BackendUnavailable {
        backend: SandboxBackendRef,
        detail: String,
    },
    /// Wrapping failed. The command was NOT executed unsandboxed.
    WrapFailed { detail: String },
    /// A backend identity string was not usable.
    IdentityInvalid { identity: String },
}

impl fmt::Display for SandboxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ValidationFailed { policy, findings } => write!(
                f,
                "sandbox policy {policy} failed validation: {} finding(s); escalation, never an \
                 unsandboxed fallback (spec 21.3)",
                findings.len()
            ),
            Self::BackendUnavailable { backend, detail } => {
                write!(f, "sandbox backend {backend} is unavailable: {detail}")
            }
            Self::WrapFailed { detail } => {
                write!(
                    f,
                    "wrapping the command failed: {detail} — not executed unsandboxed"
                )
            }
            Self::IdentityInvalid { identity } => {
                write!(
                    f,
                    "backend identity {identity:?} is not a usable opaque token"
                )
            }
        }
    }
}

impl std::error::Error for SandboxError {}

/// The containment port (spec §6.2). Validation and wrapping block (they
/// talk to backends), so both carry deadlines (§6 preamble);
/// `backend_identity` is a pure accessor.
pub trait Sandbox {
    /// Dry-run validation of a policy (spec §6.2). Precedes every use.
    fn validate(
        &self,
        policy: &SandboxPolicy,
        deadline: Deadline,
    ) -> Result<PolicyValidation, SandboxError>;

    /// Wraps a command under a policy, using the validation that policy
    /// earned (spec §6.2 `wrap(command, policy)`, with the validation
    /// threaded so "validation precedes every use" is structural). A
    /// validation naming a different policy is a typed error.
    fn wrap(
        &self,
        command: &str,
        policy: &SandboxPolicy,
        validation: &PolicyValidation,
        deadline: Deadline,
    ) -> Result<SandboxedCommand, SandboxError>;

    /// The backend this adapter fronts — an opaque configured identity
    /// (§8.2), never a product name in port source.
    fn backend_identity(&self) -> SandboxBackendRef;
}
