//! The semantic tool runtime (G2): the one place a tool call is admitted before
//! a handler runs.
//!
//! For every call, in this order:
//!
//! 1. The name must be an operation of the catalog. An unknown name is
//!    `Invalid`, and nothing runs.
//! 2. The arguments are bounded (depth, then size) and validated against the
//!    operation's schema subset. A violation is `Invalid` with the offending
//!    path, and the handler does not run.
//! 3. The operation's [`EffectClass`] must be admitted by the [`EffectPolicy`]
//!    of the host. A refusal is `Refused` with the stable code
//!    [`EFFECT_NOT_ADMITTED`].
//! 4. The registered handler runs with the invocation. The caller is the
//!    host's identity, carried on the invocation; no argument can change it.
//! 5. A successful result is capped at [`MAX_RESULT_BYTES`]. A larger result
//!    is replaced by a `Refused` with [`RESULT_TOO_LARGE`], never truncated,
//!    because a truncated JSON document is not a valid result.
//!
//! Authority is not decided here. The runtime does not duplicate or bypass a
//! consequential operation's canonical admission. An operation that changes the
//! control plane (for example `agents.spawn`) still passes through its own
//! service, which performs its authority, budget, and lineage admission. The
//! runtime only decides whether the host's policy admits the effect class at
//! all, and whether the arguments are well formed.

use std::collections::BTreeMap;

use gauntlet_domain::EffectClass;
use gauntlet_ports::json::JsonValue;
use gauntlet_ports::tool_catalog::{
    ToolCatalog, ToolInvocation, ToolInvoker, ToolName, ToolOperation,
};
use gauntlet_ports::tool_surface::ToolOutcome;

use super::schema::{self, SchemaError};

/// The stable refusal code for an effect class the policy does not admit.
pub const EFFECT_NOT_ADMITTED: &str = "effect_not_admitted";

/// The stable refusal code for a successful result over the size cap.
pub const RESULT_TOO_LARGE: &str = "result_too_large";

/// The stable refusal code for an operation with no registered handler.
pub const NO_HANDLER: &str = "no_handler";

/// The largest result document the runtime passes through, in compact bytes.
pub const MAX_RESULT_BYTES: usize = 256 * 1024;

/// The effect classes one host admits. Least privilege: a host lists only the
/// classes its operations need.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectPolicy {
    allowed: Vec<EffectClass>,
}

impl EffectPolicy {
    pub fn new(allowed: &[EffectClass]) -> Self {
        Self {
            allowed: allowed.to_vec(),
        }
    }

    /// A caller that may read state and nothing else.
    pub fn read_only() -> Self {
        Self::new(&[EffectClass::Read])
    }

    pub fn admits(&self, effect: EffectClass) -> bool {
        self.allowed.contains(&effect)
    }
}

/// A handler for one operation. Closures implement it.
pub trait ToolHandler {
    fn handle(&self, invocation: &ToolInvocation) -> ToolOutcome;
}

impl<F: Fn(&ToolInvocation) -> ToolOutcome> ToolHandler for F {
    fn handle(&self, invocation: &ToolInvocation) -> ToolOutcome {
        self(invocation)
    }
}

/// Why the runtime could not be built from a catalog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeError {
    /// An operation's schema is outside the subset the runtime enforces.
    Schema {
        operation: String,
        error: SchemaError,
    },
    /// The catalog offers the same name twice.
    DuplicateOperation(String),
    /// A handler is registered for a name the catalog does not offer.
    UnknownOperation(String),
    /// A handler is registered twice for one name.
    DuplicateHandler(String),
}

impl std::fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Schema { operation, error } => write!(f, "operation {operation}: {error}"),
            Self::DuplicateOperation(name) => write!(f, "operation {name} is offered twice"),
            Self::UnknownOperation(name) => write!(f, "no operation named {name}"),
            Self::DuplicateHandler(name) => write!(f, "operation {name} has two handlers"),
        }
    }
}

impl std::error::Error for RuntimeError {}

/// Admits and dispatches calls to one catalog's operations.
pub struct SemanticToolRuntime<'a> {
    operations: BTreeMap<ToolName, ToolOperation>,
    handlers: BTreeMap<ToolName, Box<dyn ToolHandler + 'a>>,
    policy: EffectPolicy,
}

impl<'a> SemanticToolRuntime<'a> {
    /// Snapshots the catalog and checks every schema against the subset. A
    /// schema outside the subset fails the build, so no operation is ever
    /// listed with a schema the runtime cannot enforce.
    pub fn new(catalog: &dyn ToolCatalog, policy: EffectPolicy) -> Result<Self, RuntimeError> {
        let mut operations = BTreeMap::new();
        for operation in catalog.operations() {
            schema::check_subset(&operation.input_schema).map_err(|error| {
                RuntimeError::Schema {
                    operation: operation.name.as_str().to_string(),
                    error,
                }
            })?;
            let name = operation.name.clone();
            if operations.insert(name.clone(), operation).is_some() {
                return Err(RuntimeError::DuplicateOperation(name.as_str().to_string()));
            }
        }
        Ok(Self {
            operations,
            handlers: BTreeMap::new(),
            policy,
        })
    }

    /// Registers the handler for one operation of the catalog.
    pub fn register<H: ToolHandler + 'a>(
        &mut self,
        name: &ToolName,
        handler: H,
    ) -> Result<(), RuntimeError> {
        if !self.operations.contains_key(name) {
            return Err(RuntimeError::UnknownOperation(name.as_str().to_string()));
        }
        if self.handlers.contains_key(name) {
            return Err(RuntimeError::DuplicateHandler(name.as_str().to_string()));
        }
        self.handlers.insert(name.clone(), Box::new(handler));
        Ok(())
    }

    /// The operations this runtime admits, in catalog order.
    pub fn operations(&self) -> impl Iterator<Item = &ToolOperation> {
        self.operations.values()
    }

    fn admit(
        &self,
        operation: &ToolOperation,
        invocation: &ToolInvocation,
    ) -> Result<(), ToolOutcome> {
        if let Err(detail) = schema::check_argument_bounds(&invocation.args) {
            return Err(ToolOutcome::Invalid { detail });
        }
        if let Err(detail) = schema::validate(&operation.input_schema, &invocation.args) {
            return Err(ToolOutcome::Invalid { detail });
        }
        if !self.policy.admits(operation.effect) {
            return Err(ToolOutcome::Refused {
                code: EFFECT_NOT_ADMITTED.to_string(),
                detail: format!(
                    "operation {} has effect {:?}, which this caller does not admit",
                    operation.name.as_str(),
                    operation.effect
                ),
            });
        }
        Ok(())
    }
}

impl ToolInvoker for SemanticToolRuntime<'_> {
    fn invoke(&self, invocation: &ToolInvocation) -> ToolOutcome {
        let Some(operation) = self.operations.get(&invocation.name) else {
            return ToolOutcome::Invalid {
                detail: format!("unknown tool: {}", invocation.name.as_str()),
            };
        };
        if let Err(outcome) = self.admit(operation, invocation) {
            return outcome;
        }
        let Some(handler) = self.handlers.get(&invocation.name) else {
            return ToolOutcome::Refused {
                code: NO_HANDLER.to_string(),
                detail: format!("operation {} has no handler", invocation.name.as_str()),
            };
        };
        match handler.handle(invocation) {
            ToolOutcome::Ok(value) => cap_result(value),
            other => other,
        }
    }
}

fn cap_result(value: JsonValue) -> ToolOutcome {
    if value.to_json_text().len() > MAX_RESULT_BYTES {
        return ToolOutcome::Refused {
            code: RESULT_TOO_LARGE.to_string(),
            detail: format!("the result exceeds {MAX_RESULT_BYTES} bytes"),
        };
    }
    ToolOutcome::Ok(value)
}
