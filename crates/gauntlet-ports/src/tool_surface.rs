//! The protocol-neutral tool surface: a named set of model-facing operations,
//! each described by a JSON Schema text and called with a parsed JSON value.
//!
//! A projection (a tool-protocol server, a binding, a UI) lists the tools and
//! forwards calls. The surface owns the strict decoding and encoding of its
//! arguments and results; the projection owns only transport.
//!
//! The outcome separates two kinds of "no":
//!
//! - [`ToolOutcome::Refused`] is a governed refusal (authority, limits,
//!   ownership). It is a NORMAL result the model can read and act on.
//! - [`ToolOutcome::Invalid`] means the arguments were malformed. Nothing ran.

use crate::json::{JsonValue, parse};

/// One tool as a projection lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolListing {
    pub name: String,
    pub description: String,
    /// The JSON Schema of the arguments, as text.
    pub input_schema: String,
}

/// The result of one call.
#[derive(Debug, Clone, PartialEq)]
pub enum ToolOutcome {
    /// The call succeeded; the value matches the tool's output schema.
    Ok(JsonValue),
    /// A governed refusal, with a stable machine `code` and a human `detail`.
    Refused { code: String, detail: String },
    /// The arguments were malformed; `detail` names the offending path.
    Invalid { detail: String },
}

/// A set of tools callable by name. Single-threaded, like the sibling ports.
pub trait ToolSurface {
    /// Every tool this surface offers, in presentation order.
    fn tools(&self) -> Vec<ToolListing>;

    /// Calls one tool with already-parsed arguments.
    fn call(&self, name: &str, arguments: &JsonValue) -> ToolOutcome;

    /// Calls one tool with raw JSON text. Text that is not admissible JSON,
    /// including a document with a duplicate key, is `Invalid`.
    fn call_text(&self, name: &str, arguments: &str) -> ToolOutcome {
        match parse(arguments) {
            Ok(value) => self.call(name, &value),
            Err(error) => ToolOutcome::Invalid {
                detail: format!("arguments: {error}"),
            },
        }
    }
}
