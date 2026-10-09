//! The authority class of one model-facing operation. The class says which gate
//! governs a call; no class grants authority by itself.

/// The authority class of one affordance or tool operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectClass {
    /// Reads state only. Governed by permission and budget.
    Read,
    /// Mutates a workspace. Governed by permission and containment.
    WorkspaceMutation,
    /// Reaches outside the workspace. Governed by the authority policy.
    ExternalEffect,
    /// Changes the control plane. Governed by canonical admission.
    ControlPlane,
}
