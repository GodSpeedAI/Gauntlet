//! Run-state services (plan Task 6): thin orchestration over the
//! `StateStore` port. Spec §18.1: "Deterministic export to portable
//! JSON/JSONL is REQUIRED regardless of backend" — this module is where
//! that requirement lives for the application layer, so no consumer of the
//! export depends on whichever adapter happens to be wired underneath.

pub mod export;

pub use export::{ExportError, export_run};
