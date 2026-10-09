pub use gauntlet_domain::{HarnessId, RunnerProfileId};
use gauntlet_ports::agent_runner::Isolation;
use std::collections::BTreeMap;

/// Support level — a capability ladder from none to native agents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SupportLevel {
    Process = 0,
    Interactive = 1,
    Session = 2,
    Structured = 3,
    Native = 4,
}

impl std::fmt::Display for SupportLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Process => write!(f, "Process"),
            Self::Interactive => write!(f, "Interactive"),
            Self::Session => write!(f, "Session"),
            Self::Structured => write!(f, "Structured"),
            Self::Native => write!(f, "Native"),
        }
    }
}

/// Lifecycle source — where availability is determined.
/// Lifecycle is liveness/forensics only, never completion semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LifecycleSource {
    Structured,
    Process,
    Screen,
    Unavailable,
}

impl std::fmt::Display for LifecycleSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Structured => write!(f, "Structured"),
            Self::Process => write!(f, "Process"),
            Self::Screen => write!(f, "Screen"),
            Self::Unavailable => write!(f, "Unavailable"),
        }
    }
}

/// Fabric capabilities — what a runner instance supports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FabricCapabilities {
    pub support_level: SupportLevel,
    pub lifecycle_source: LifecycleSource,
    pub resumable: bool,
    pub model_selection: bool,
    pub native_subagents: bool,
    pub streaming_events: bool,
    pub structured_lifecycle: bool,
    pub fresh_context: bool,
    pub workspace_mutation: bool,
    pub isolation: Isolation,
    pub max_depth: u8,
}

/// Model facts — provider and model identification (None means UNKNOWN).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelFacts {
    pub provider: Option<String>,
    pub model: Option<String>,
}

/// Runner profile — identity, harness, model, label.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunnerProfile {
    pub id: RunnerProfileId,
    pub harness: HarnessId,
    pub model: ModelFacts,
    pub label: String,
}

/// Availability status of a runner instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Availability {
    Available,
    Unavailable { reason: String },
}

/// Runner instance — profile + availability + capabilities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunnerInstance {
    pub profile: RunnerProfile,
    pub availability: Availability,
    pub capabilities: FabricCapabilities,
}

/// Registry errors — typed failures for deterministic construction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistryError {
    /// Duplicate profile ID in the registry.
    DuplicateProfileId { id: String },
    /// Profile references an unknown harness.
    UnknownHarness { harness: String },
}

impl std::fmt::Display for RegistryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateProfileId { id } => {
                write!(f, "duplicate runner profile id: '{id}'")
            }
            Self::UnknownHarness { harness } => {
                write!(f, "profile references unknown harness: '{harness}'")
            }
        }
    }
}

impl std::error::Error for RegistryError {}

/// Runner registry — deterministic, ordered collection of runner instances.
#[derive(Debug, Clone)]
pub struct RunnerRegistry {
    instances: BTreeMap<RunnerProfileId, RunnerInstance>,
}

impl RunnerRegistry {
    /// Create a new runner registry from instances.
    /// Rejects duplicate profile IDs.
    pub fn new(instances: Vec<RunnerInstance>) -> Result<Self, RegistryError> {
        let mut map = BTreeMap::new();
        for instance in instances {
            let id_str = instance.profile.id.to_string();
            if map.insert(instance.profile.id.clone(), instance).is_some() {
                return Err(RegistryError::DuplicateProfileId { id: id_str });
            }
        }
        Ok(RunnerRegistry { instances: map })
    }

    /// Get a runner instance by profile ID.
    pub fn get(&self, id: &RunnerProfileId) -> Option<&RunnerInstance> {
        self.instances.get(id)
    }

    /// Return all instances in deterministic order (sorted by profile id).
    pub fn instances(&self) -> Vec<&RunnerInstance> {
        self.instances.values().collect()
    }

    /// Check registry against a set of known harnesses.
    /// Returns UnknownHarness error if any profile references a harness outside the set.
    /// Returns Ok with the same registry if all harnesses are known.
    pub fn with_known_harnesses(
        &self,
        known: std::collections::HashSet<HarnessId>,
    ) -> Result<&Self, RegistryError> {
        for instance in self.instances.values() {
            if !known.contains(&instance.profile.harness) {
                return Err(RegistryError::UnknownHarness {
                    harness: instance.profile.harness.to_string(),
                });
            }
        }
        Ok(self)
    }
}
