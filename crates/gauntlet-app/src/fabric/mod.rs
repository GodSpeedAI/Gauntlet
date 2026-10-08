//! Fabric — runner profile registry and deterministic selection logic.
//!
//! The fabric layer maps application requirements to concrete runners
//! (harness × model × profile combinations). It enforces requirements closure:
//! a selection either meets ALL requirements or fails with the explicit
//! UnmetRequirement reasons, never silently degraded.
//!
//! Determinism invariants:
//! - Instances ordered by profile id (BTreeMap)
//! - Ties broken by support_level desc, then profile id asc
//! - Selection failures list ALL reasons, not just the first
//! - Explicit pin: honored or explicitly refused (no fallback)
//! - Availability and capability mismatch both contribute to rejection
//!
//! Lifecycle is liveness/forensics only: captured at snapshot time,
//! never used for completion semantics.

pub mod registry;
pub mod selection;

pub use registry::{
    Availability, FabricCapabilities, HarnessId, LifecycleSource, ModelFacts, RegistryError,
    RunnerInstance, RunnerProfile, RunnerProfileId, RunnerRegistry, SupportLevel,
};

pub use selection::{
    DiverseSelection, DiversityKnown, DiversityRequest, Requirements, Selected, SelectionRefusal,
    UnmetRequirement, select, select_diverse,
};
