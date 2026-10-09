//! The semantic tool runtime (G): admits every tool call from a
//! `ToolCatalog` before its handler runs. See [`runtime`] for the admission
//! order and [`schema`] for the JSON Schema subset.

pub mod program;
pub mod runtime;
pub mod schema;

#[cfg(test)]
mod program_tests;
#[cfg(test)]
mod tests;

pub use program::{AdmissionError, ProgramDriver, admitted_subset};
pub use runtime::{
    EFFECT_NOT_ADMITTED, EffectPolicy, MAX_RESULT_BYTES, NO_HANDLER, RESULT_TOO_LARGE,
    RuntimeError, SemanticToolRuntime, ToolHandler,
};
