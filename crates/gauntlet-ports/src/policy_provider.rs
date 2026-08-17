//! `PolicyProvider` — spec §6.2: "`policy_for(tier, workspace) ->
//! SandboxPolicy` · `permissions_for(role, unit) -> PermissionSet` —
//! Resolves repo-owned policy artifacts; no policy is compiled in."
//!
//! `PermissionSet` carries the §17.2 fields verbatim: "filesystem
//! read/write path sets, network posture, tool allowlist, credential scope,
//! destructive-operation flag", plus the enforcement record — "Prompt-only
//! enforcement MUST be recorded as `enforcement: advisory`".

use core::fmt;

use gauntlet_domain::{BlastRadius, PermissionSetRef, RoleId, UnitId, WorkspaceRef};

use crate::deadline::Deadline;
use crate::sandbox::{NetworkPosture, SandboxPolicy};
use crate::tool_provider::SemanticCapability;

/// `PolicyProvider` failures — typed (spec §21.2).
#[derive(Debug, Clone, PartialEq)]
pub enum PolicyProviderError {
    /// No policy exists for the tier. For `irreversible` this is the
    /// REQUIREMENT (spec §17.3: "**No policy exists.** The action never
    /// reaches the sandbox port unattended."); for other tiers it means the
    /// repo's policy artifacts are incomplete.
    NoPolicyForTier { tier: BlastRadius },
    /// The policy artifacts exist but do not decode.
    Invalid { detail: String },
    /// The artifacts could not be read.
    Unavailable { detail: String },
}

impl fmt::Display for PolicyProviderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoPolicyForTier { tier } => write!(
                f,
                "no sandbox policy exists for the {tier} tier — for irreversible that is the \
                 requirement (spec 17.3); for any other tier the policy artifacts are incomplete"
            ),
            Self::Invalid { detail } => write!(f, "a policy artifact is invalid: {detail}"),
            Self::Unavailable { detail } => {
                write!(f, "policy artifacts are unavailable: {detail}")
            }
        }
    }
}

impl std::error::Error for PolicyProviderError {}

/// A resolved permission set (spec §17.2), field for field.
#[derive(Debug, Clone, PartialEq)]
pub struct PermissionSet {
    pub reference: PermissionSetRef,
    /// Filesystem read path set.
    pub read_paths: Vec<String>,
    /// Filesystem write path set.
    pub write_paths: Vec<String>,
    /// Network posture.
    pub network: NetworkPosture,
    /// Tool allowlist — semantic capabilities (§8.4), never products.
    pub tools: Vec<SemanticCapability>,
    /// Credential scope.
    pub credential_scope: CredentialScope,
    /// Destructive-operation flag (§22.8: destructive operations need more
    /// than this flag alone).
    pub destructive: bool,
    /// How this set will actually be enforced — the §17.2 mechanism ladder.
    pub enforcement: Enforcement,
}

/// Credential scope granted to the invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CredentialScope {
    /// No credentials.
    None,
    /// Short-lived, read-only, scoped credentials (§22.6).
    BoundedReadonly { scope: String },
    /// Ambient worker credentials — the harness-is-not-a-sandbox posture
    /// (§22.2); requires containment to be safe.
    Ambient,
}

/// The §17.2 enforcement ladder: "The runtime applies it through the
/// strongest available mechanism: sandbox policy > process user/mount
/// options > runner-native permission API > prompt." Prompt-only
/// enforcement is recorded as `Advisory` "so its weakness is visible in
/// evidence".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Enforcement {
    SandboxPolicy,
    ProcessOptions,
    RunnerNative,
    Advisory,
}

impl Enforcement {
    /// The wire token.
    pub fn token(self) -> &'static str {
        match self {
            Self::SandboxPolicy => "sandbox_policy",
            Self::ProcessOptions => "process_options",
            Self::RunnerNative => "runner_native",
            Self::Advisory => "advisory",
        }
    }
}

impl fmt::Display for Enforcement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.token())
    }
}

/// The repo-owned policy port (spec §6.2). Resolving artifacts is IO, so
/// both methods carry deadlines (§6 preamble).
pub trait PolicyProvider {
    /// Resolves the sandbox policy for a tier in a workspace (spec §6.2).
    /// Refuses the `irreversible` tier structurally (§17.3).
    fn policy_for(
        &self,
        tier: BlastRadius,
        workspace: &WorkspaceRef,
        deadline: Deadline,
    ) -> Result<SandboxPolicy, PolicyProviderError>;

    /// Resolves the permission set for a role dispatching a unit (spec
    /// §17.2: `permissions_for(role, unit)`).
    fn permissions_for(
        &self,
        role: &RoleId,
        unit: &UnitId,
        deadline: Deadline,
    ) -> Result<PermissionSet, PolicyProviderError>;
}
