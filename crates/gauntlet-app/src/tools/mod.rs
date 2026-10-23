//! The semantic tool runtime (G): admits every tool call from a
//! `ToolCatalog` before its handler runs. See [`runtime`] for the admission
//! order and [`schema`] for the JSON Schema subset. [`program_catalog`] exposes
//! one bounded tool program as the `program.run` operation.

pub mod program;
pub mod program_catalog;
pub mod runtime;
pub mod schema;

#[cfg(test)]
mod program_tests;
#[cfg(test)]
mod tests;

pub use program::{AdmissionError, ProgramDriver, admitted_subset};
pub use program_catalog::{
    CeilingError, DEFAULT_PROGRAM_MAX_PARALLEL_CALLS, DEFAULT_PROGRAM_MAX_RESULT_BYTES,
    DEFAULT_PROGRAM_MAX_TOOL_CALLS, PROGRAM_RUN_TOOL, PROGRAM_SOURCE_LIMIT_BYTES, ProgramCatalog,
    ProgramCeilings, outcome_json, program_runtime,
};
pub use runtime::{
    EFFECT_NOT_ADMITTED, EffectPolicy, MAX_RESULT_BYTES, NO_HANDLER, RESULT_TOO_LARGE,
    RuntimeError, SemanticToolRuntime, ToolHandler,
};
