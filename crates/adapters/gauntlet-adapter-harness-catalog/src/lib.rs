//! Harness catalog: data model and validation for runtime harness definitions.
//!
//! This crate provides:
//! - A strict TOML-subset parser for configuration files
//! - A typed HarnessDefinition model with comprehensive validation
//! - Error types for diagnostics
//!
//! # Note
//!
//! This crate is data + validation only; it does not spawn processes or interact with harnesses.
//!
//! # Example
//!
//! ```ignore
//! use gauntlet_adapter_harness_catalog::{HarnessDefinition, Origin};
//!
//! let toml = r#"
//! schema = 1
//! id = "test"
//! label = "Test Harness"
//! executables = ["test"]
//! # ... rest of configuration
//! "#;
//!
//! let definition = HarnessDefinition::from_toml_str(toml, Origin::BuiltIn)?;
//! println!("{} ({})", definition.label, definition.id);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

pub mod catalog;
pub mod definition;
pub mod provenance;
pub mod subset_toml;

pub use catalog::{CatalogError, builtin, load_custom_dir};
pub use definition::{
    Capabilities, Control, DefinitionError, DefinitionResult, HarnessDefinition, InterruptKind,
    Launch, Lifecycle, LifecycleSource, Origin, PromptMode, Provenance, Session, SessionIdentity,
    SupportLevel,
};
pub use provenance::{
    HERDR_LICENSE, HERDR_PINNED_COMMIT, HERDR_SOURCE, ProvenanceRef, RefField, SyncReport,
    check_against_donor, parse_ref,
};
pub use subset_toml::{ParseError, ParseErrorKind, Parser, Table, Value};
