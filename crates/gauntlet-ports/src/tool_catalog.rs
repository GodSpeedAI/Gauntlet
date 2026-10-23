//! The tool catalog and invoker ports (G1): one list of named operations that
//! every projection (a tool-protocol server, a Python or JavaScript binding, a
//! UI) reads, and one invoker that every call goes through.
//!
//! - [`ToolCatalog`] lists [`ToolOperation`]s in a deterministic order. Each
//!   operation carries its JSON Schema subset and its [`EffectClass`]. The
//!   catalog never grants authority; the runtime that dispatches a call
//!   decides admission.
//! - [`ToolInvoker`] runs one [`ToolInvocation`]. The caller is supplied by the
//!   host that built the invocation, never by the arguments.
//! - [`CatalogBuilder`] merges several catalogs and rejects a duplicate name
//!   fail-closed.
//! - [`CatalogSurface`] derives the protocol-neutral [`ToolSurface`] from a
//!   catalog plus an invoker, so the names and schemas a projection lists are
//!   exactly the catalog's.
//!
//! This module names capabilities only. It carries no vendor identifiers.

use gauntlet_domain::{EffectClass, InvocationId};

use crate::json::JsonValue;
use crate::tool_surface::{ToolListing, ToolOutcome, ToolSurface};

const NAME_MAX_LEN: usize = 64;

/// A validated operation name: `[a-z][a-z0-9_.]{0,63}`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ToolName(String);

/// Why a candidate name was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolNameError {
    Empty,
    TooLong { len: usize },
    BadFirstCharacter,
    BadCharacter { index: usize },
}

impl std::fmt::Display for ToolNameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => write!(f, "a tool name is empty"),
            Self::TooLong { len } => {
                write!(f, "a tool name is {len} bytes; the limit is {NAME_MAX_LEN}")
            }
            Self::BadFirstCharacter => write!(f, "a tool name starts with a lowercase letter"),
            Self::BadCharacter { index } => {
                write!(f, "a tool name has a disallowed character at byte {index}")
            }
        }
    }
}

impl std::error::Error for ToolNameError {}

impl ToolName {
    /// Validates a candidate name. Accepts `[a-z][a-z0-9_.]{0,63}` only.
    pub fn new(text: &str) -> Result<Self, ToolNameError> {
        if text.is_empty() {
            return Err(ToolNameError::Empty);
        }
        if text.len() > NAME_MAX_LEN {
            return Err(ToolNameError::TooLong { len: text.len() });
        }
        for (index, byte) in text.bytes().enumerate() {
            let ok = match index {
                0 => byte.is_ascii_lowercase(),
                _ => {
                    byte.is_ascii_lowercase()
                        || byte.is_ascii_digit()
                        || byte == b'_'
                        || byte == b'.'
                }
            };
            if !ok {
                return Err(if index == 0 {
                    ToolNameError::BadFirstCharacter
                } else {
                    ToolNameError::BadCharacter { index }
                });
            }
        }
        Ok(Self(text.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// One model-facing operation. `input_schema` is a JSON Schema document in the
/// bounded subset the runtime validates; the subset is defined by the app
/// layer, not here.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolOperation {
    pub name: ToolName,
    pub description: String,
    pub input_schema: JsonValue,
    pub effect: EffectClass,
}

/// A deterministic, named set of operations.
pub trait ToolCatalog {
    /// Every operation, in a deterministic order a projection presents.
    fn operations(&self) -> Vec<ToolOperation>;

    /// The operation with this name, if the catalog holds it.
    fn get(&self, name: &ToolName) -> Option<ToolOperation>;
}

/// One call to run. The caller is the host's identity for the calling
/// invocation; no argument can name or change it.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolInvocation {
    pub caller: InvocationId,
    pub name: ToolName,
    pub args: JsonValue,
}

/// Runs one invocation and reports its outcome. Single-threaded, like the
/// sibling ports.
pub trait ToolInvoker {
    fn invoke(&self, invocation: &ToolInvocation) -> ToolOutcome;
}

/// Why a catalog could not be built.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CatalogError {
    /// Two catalogs (or one catalog twice) offer the same name.
    DuplicateName(ToolName),
}

impl std::fmt::Display for CatalogError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateName(name) => write!(f, "tool name {} is offered twice", name.as_str()),
        }
    }
}

impl std::error::Error for CatalogError {}

/// Merges catalogs in the order they are added. A duplicate name is an error:
/// the composite never shadows an operation.
#[derive(Debug, Default)]
pub struct CatalogBuilder {
    operations: Vec<ToolOperation>,
}

impl CatalogBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with(mut self, catalog: &dyn ToolCatalog) -> Result<Self, CatalogError> {
        for operation in catalog.operations() {
            if self
                .operations
                .iter()
                .any(|known| known.name == operation.name)
            {
                return Err(CatalogError::DuplicateName(operation.name));
            }
            self.operations.push(operation);
        }
        Ok(self)
    }

    pub fn build(self) -> Composite {
        Composite {
            operations: self.operations,
        }
    }
}

/// The merged catalog that [`CatalogBuilder`] produces.
#[derive(Debug, Clone, PartialEq)]
pub struct Composite {
    operations: Vec<ToolOperation>,
}

impl ToolCatalog for Composite {
    fn operations(&self) -> Vec<ToolOperation> {
        self.operations.clone()
    }

    fn get(&self, name: &ToolName) -> Option<ToolOperation> {
        self.operations.iter().find(|op| &op.name == name).cloned()
    }
}

/// Derives a protocol-neutral [`ToolSurface`] from a catalog and an invoker.
/// The host supplies the caller; every call is stamped with it.
pub struct CatalogSurface<'a> {
    catalog: &'a dyn ToolCatalog,
    invoker: &'a dyn ToolInvoker,
    caller: InvocationId,
}

impl<'a> CatalogSurface<'a> {
    pub fn new(
        catalog: &'a dyn ToolCatalog,
        invoker: &'a dyn ToolInvoker,
        caller: InvocationId,
    ) -> Self {
        Self {
            catalog,
            invoker,
            caller,
        }
    }
}

impl ToolSurface for CatalogSurface<'_> {
    fn tools(&self) -> Vec<ToolListing> {
        self.catalog
            .operations()
            .into_iter()
            .map(|operation| ToolListing {
                name: operation.name.as_str().to_string(),
                description: operation.description,
                input_schema: operation.input_schema.to_json_text(),
            })
            .collect()
    }

    fn call(&self, name: &str, arguments: &JsonValue) -> ToolOutcome {
        match ToolName::new(name) {
            Ok(name) => self.invoker.invoke(&ToolInvocation {
                caller: self.caller.clone(),
                name,
                args: arguments.clone(),
            }),
            Err(_) => ToolOutcome::Invalid {
                detail: format!("unknown tool: {name}"),
            },
        }
    }
}
