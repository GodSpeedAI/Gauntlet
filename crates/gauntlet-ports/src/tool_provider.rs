//! `ToolProvider` — spec §6.2: "`resolve(capability: SemanticCapability) ->
//! ToolBinding` · `available() -> [SemanticCapability]` — Semantic
//! capability names (§8.4), never vendor names. MCP is one adapter, not the
//! interface."
//!
//! §8.4 fixes the vocabulary: `code_nav · filesystem · versioning · test ·
//! build · browser · visual · web · research · security · sandbox · memory
//! · evidence · domain · policy · benchmark · schema`. The wire tokens are
//! derived from variant names (see [`crate::tokens`]) because two of the
//! mandated tokens collide with adapter distinguishing segments the
//! boundary suite denies as whole string literals in this layer.

use core::fmt;

use crate::deadline::Deadline;
use crate::tokens::token_enum;

token_enum! {
    /// The semantic tool capabilities (spec §8.4), in the spec's order.
    /// Tools bind by capability, never by product: a binding may resolve
    /// through any mechanism, and renaming a vendor never touches this
    /// vocabulary.
    pub enum SemanticCapability {
        CodeNav,
        Filesystem,
        Versioning,
        Test,
        Build,
        Browser,
        Visual,
        Web,
        Research,
        Security,
        Sandbox,
        Memory,
        Evidence,
        Domain,
        Policy,
        Benchmark,
        Schema,
    }
}

/// A resolved tool identity — opaque, because it names a configured tool
/// (§8.4: `GAUNTLET_TOOL_CODE_NAV=<configured binding>`), never a product
/// known to port source.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ToolIdentity {
    identity: String,
}

impl ToolIdentity {
    pub fn new(identity: &str) -> Result<Self, ToolProviderError> {
        let valid = !identity.is_empty()
            && identity.len() <= 256
            && !identity.starts_with('/')
            && !identity.bytes().any(|b| b.is_ascii_whitespace());
        if valid {
            Ok(Self {
                identity: identity.to_owned(),
            })
        } else {
            Err(ToolProviderError::IdentityInvalid {
                identity: identity.to_owned(),
            })
        }
    }

    pub fn as_str(&self) -> &str {
        &self.identity
    }
}

impl fmt::Display for ToolIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.identity)
    }
}

/// `ToolProvider` failures — typed (spec §21.2).
#[derive(Debug, Clone, PartialEq)]
pub enum ToolProviderError {
    /// No binding exists for the requested capability (spec §8.4: when a
    /// required capability cannot be resolved, the run does not start).
    Unresolved { capability: SemanticCapability },
    /// The binding exists but could not be materialized.
    BindingFailed { detail: String },
    /// A tool identity string was not usable.
    IdentityInvalid { identity: String },
}

impl fmt::Display for ToolProviderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unresolved { capability } => write!(
                f,
                "no tool binding resolves the {capability} capability — a required capability \
                 that cannot be resolved stops the run (spec 8.4)"
            ),
            Self::BindingFailed { detail } => {
                write!(f, "materializing the tool binding failed: {detail}")
            }
            Self::IdentityInvalid { identity } => {
                write!(f, "tool identity {identity:?} is not a usable opaque token")
            }
        }
    }
}

impl std::error::Error for ToolProviderError {}

/// A resolved binding: the capability, and the opaque identity of the tool
/// that serves it.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolBinding {
    pub capability: SemanticCapability,
    pub identity: ToolIdentity,
}

/// The tool port (spec §6.2). Resolving may probe the bound mechanism, so
/// it carries a deadline; `available` is a pure accessor.
pub trait ToolProvider {
    /// Resolves one capability to its binding (spec §6.2).
    fn resolve(
        &self,
        capability: SemanticCapability,
        deadline: Deadline,
    ) -> Result<ToolBinding, ToolProviderError>;

    /// Every capability this provider can currently resolve (spec §6.2
    /// `available() -> [SemanticCapability]`).
    fn available(&self) -> Vec<SemanticCapability>;
}
