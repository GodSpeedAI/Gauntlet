use gauntlet_ports::agent_runner::Isolation;
use std::collections::HashSet;

use crate::fabric::registry::{
    Availability, HarnessId, RunnerInstance, RunnerProfileId, RunnerRegistry, SupportLevel,
};

/// Unmet requirement — why a runner was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnmetRequirement {
    InsufficientSupportLevel {
        required: SupportLevel,
        found: SupportLevel,
    },
    CannotResume,
    NeedsModelSelection,
    NeedsNativeSubagents,
    NeedsStreaming,
    NeedsStructuredLifecycle,
    RequiresFreshContext,
    WorkspaceMutationForbidden,
    InsufficientIsolation {
        required: Isolation,
        found: Isolation,
    },
    InsufficientDepthSupport {
        required: u8,
        found: u8,
    },
}

impl std::fmt::Display for UnmetRequirement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InsufficientSupportLevel { required, found } => {
                write!(
                    f,
                    "insufficient support level: required {required}, found {found}"
                )
            }
            Self::CannotResume => write!(f, "runner does not support resumption"),
            Self::NeedsModelSelection => write!(f, "runner does not support model selection"),
            Self::NeedsNativeSubagents => write!(f, "runner does not support native subagents"),
            Self::NeedsStreaming => write!(f, "runner does not support streaming events"),
            Self::NeedsStructuredLifecycle => {
                write!(f, "runner does not support structured lifecycle")
            }
            Self::RequiresFreshContext => write!(f, "runner does not support fresh context"),
            Self::WorkspaceMutationForbidden => {
                write!(f, "workspace mutation is forbidden but runner requires it")
            }
            Self::InsufficientIsolation { required, found } => {
                write!(
                    f,
                    "insufficient isolation: required {:?}, found {:?}",
                    required, found
                )
            }
            Self::InsufficientDepthSupport { required, found } => {
                write!(
                    f,
                    "insufficient max depth support: required {required}, found {found}"
                )
            }
        }
    }
}

impl std::error::Error for UnmetRequirement {}

/// Workspace mutation requirement — Required, Forbidden, or DontCare.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Needs {
    Required,
    Forbidden,
    DontCare,
}

/// Selection requirements — what a runner must support.
#[derive(Debug, Clone)]
pub struct Requirements {
    pub fresh_context: bool,
    pub workspace_mutation: Needs,
    pub min_support_level: SupportLevel,
    pub needs_resume: bool,
    pub needs_model_selection: bool,
    pub needs_native_subagents: bool,
    pub needs_streaming: bool,
    pub needs_structured_lifecycle: bool,
    pub min_isolation: Isolation,
    pub min_depth_support: u8,
}

impl Default for Requirements {
    fn default() -> Self {
        Self {
            fresh_context: false,
            workspace_mutation: Needs::DontCare,
            min_support_level: SupportLevel::Process,
            needs_resume: false,
            needs_model_selection: false,
            needs_native_subagents: false,
            needs_streaming: false,
            needs_structured_lifecycle: false,
            min_isolation: Isolation::None,
            min_depth_support: 0,
        }
    }
}

/// Selection preference — hints that break ties but never override requirements.
#[derive(Debug, Clone, Default)]
pub struct Preference {
    pub pin: Option<RunnerProfileId>,
    pub preferred_harness: Option<HarnessId>,
    pub preferred_model: Option<String>,
}

/// Reason for selection refusal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectionRefusal {
    /// Explicit pin refers to an unknown profile.
    PinUnknown { pin: String },
    /// Pinned runner is unavailable.
    PinUnavailable { pin: String, reason: String },
    /// Pinned runner violates one or more requirements.
    PinViolatesRequirements {
        pin: String,
        unmet: Vec<UnmetRequirement>,
    },
    /// No eligible runner found; lists why each was rejected.
    NoEligibleRunner {
        rejections: Vec<(RunnerProfileId, Vec<UnmetRequirement>)>,
    },
    /// Insufficient diversity for the requested count and constraints.
    InsufficientDiversity { requested: usize, achieved: usize },
}

impl std::fmt::Display for SelectionRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PinUnknown { pin } => write!(f, "pinned runner '{pin}' is not known"),
            Self::PinUnavailable { pin, reason } => {
                write!(f, "pinned runner '{pin}' is unavailable: {reason}")
            }
            Self::PinViolatesRequirements { pin, unmet } => {
                write!(f, "pinned runner '{pin}' violates requirements:")?;
                for req in unmet {
                    write!(f, " {req};")?;
                }
                Ok(())
            }
            Self::NoEligibleRunner { rejections } => {
                write!(f, "no eligible runner found; rejections:")?;
                for (id, reasons) in rejections {
                    write!(f, " {id}:",)?;
                    for reason in reasons {
                        write!(f, " {reason};")?;
                    }
                }
                Ok(())
            }
            Self::InsufficientDiversity {
                requested,
                achieved,
            } => {
                write!(
                    f,
                    "insufficient diversity: requested {requested}, achieved {achieved}"
                )
            }
        }
    }
}

impl std::error::Error for SelectionRefusal {}

/// Selected runner instance.
#[derive(Debug, Clone)]
pub struct Selected {
    pub instance: RunnerInstance,
}

/// Diverse selection result with diversity facts.
#[derive(Debug, Clone)]
pub struct DiversityKnown {
    pub distinct_harnesses: usize,
    pub distinct_providers_known: usize,
    pub unknown_provider_count: usize,
}

/// Diverse selection with multiple runners and diversity facts.
#[derive(Debug, Clone)]
pub struct DiverseSelection {
    pub instances: Vec<RunnerInstance>,
    pub diversity: DiversityKnown,
}

/// Diversity request constraints.
#[derive(Debug, Clone)]
pub struct DiversityRequest {
    pub require_distinct_harness: bool,
    pub require_distinct_provider: bool,
}

/// Select a single runner that meets all requirements.
/// Returns the first eligible runner (deterministic order).
/// Explicit pin: honored strictly or explicitly refused (never fallback).
pub fn select(
    registry: &RunnerRegistry,
    requirements: &Requirements,
    preference: &Preference,
) -> Result<Selected, SelectionRefusal> {
    let instances = registry.instances();

    // Handle explicit pin.
    if let Some(pin) = &preference.pin {
        let instance = registry
            .get(pin)
            .ok_or_else(|| SelectionRefusal::PinUnknown {
                pin: pin.to_string(),
            })?;

        // Check availability.
        if let Availability::Unavailable {
            reason: reason_text,
        } = &instance.availability
        {
            return Err(SelectionRefusal::PinUnavailable {
                pin: pin.to_string(),
                reason: reason_text.clone(),
            });
        }

        // Check requirements.
        let unmet = check_unmet_requirements(instance, requirements);
        if !unmet.is_empty() {
            return Err(SelectionRefusal::PinViolatesRequirements {
                pin: pin.to_string(),
                unmet,
            });
        }

        return Ok(Selected {
            instance: instance.clone(),
        });
    }

    // Find all eligible runners.
    let mut eligible: Vec<_> = instances
        .iter()
        .filter(|inst| inst.availability == Availability::Available)
        .filter(|inst| check_unmet_requirements(inst, requirements).is_empty())
        .collect();

    if eligible.is_empty() {
        // Gather rejection reasons for all.
        let rejections: Vec<_> = instances
            .iter()
            .map(|inst| {
                let mut reasons = Vec::new();
                if let Availability::Unavailable { reason: _ } = &inst.availability {
                    reasons.push(UnmetRequirement::CannotResume); // placeholder
                }
                reasons.extend(check_unmet_requirements(inst, requirements));
                (inst.profile.id.clone(), reasons)
            })
            .collect();

        return Err(SelectionRefusal::NoEligibleRunner { rejections });
    }

    // Sort by preference and support level (deterministic order).
    eligible.sort_by(|a, b| {
        // Prefer pinned (already handled above).
        // Prefer matching harness.
        let harness_match_a = preference
            .preferred_harness
            .as_ref()
            .map(|h| &a.profile.harness == h)
            .unwrap_or(false);
        let harness_match_b = preference
            .preferred_harness
            .as_ref()
            .map(|h| &b.profile.harness == h)
            .unwrap_or(false);

        if harness_match_a != harness_match_b {
            return if harness_match_a {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Greater
            };
        }

        // Prefer matching model provider.
        let model_match_a = preference
            .preferred_model
            .as_ref()
            .and(a.profile.model.provider.as_ref())
            .map(|p| p == preference.preferred_model.as_ref().unwrap())
            .unwrap_or(false);
        let model_match_b = preference
            .preferred_model
            .as_ref()
            .and(b.profile.model.provider.as_ref())
            .map(|p| p == preference.preferred_model.as_ref().unwrap())
            .unwrap_or(false);

        if model_match_a != model_match_b {
            return if model_match_a {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Greater
            };
        }

        // Break ties: support level (descending), then profile id (ascending).
        match b
            .capabilities
            .support_level
            .cmp(&a.capabilities.support_level)
        {
            std::cmp::Ordering::Equal => a.profile.id.cmp(&b.profile.id),
            other => other,
        }
    });

    Ok(Selected {
        instance: (*eligible[0]).clone(),
    })
}

/// Select multiple runners with diversity constraints.
pub fn select_diverse(
    registry: &RunnerRegistry,
    requirements: &Requirements,
    count: usize,
    diversity: &DiversityRequest,
) -> Result<DiverseSelection, SelectionRefusal> {
    let instances = registry.instances();

    // Find all eligible runners.
    let eligible: Vec<_> = instances
        .iter()
        .filter(|inst| inst.availability == Availability::Available)
        .filter(|inst| check_unmet_requirements(inst, requirements).is_empty())
        .cloned()
        .collect();

    if eligible.is_empty() {
        return Err(SelectionRefusal::InsufficientDiversity {
            requested: count,
            achieved: 0,
        });
    }

    // Greedy selection with diversity tracking.
    let mut selected = Vec::new();
    let mut used_harnesses = HashSet::new();
    let mut used_providers = HashSet::new();
    let mut unknown_provider_count = 0;

    for inst in eligible.iter() {
        if selected.len() >= count {
            break;
        }

        // Check harness distinctness.
        if diversity.require_distinct_harness && used_harnesses.contains(&inst.profile.harness) {
            continue;
        }

        // Check provider distinctness.
        if diversity.require_distinct_provider {
            if let Some(provider) = &inst.profile.model.provider {
                if used_providers.contains(provider) {
                    continue;
                }
            } else {
                // Can't satisfy provider distinctness with unknown provider.
                return Err(SelectionRefusal::InsufficientDiversity {
                    requested: count,
                    achieved: selected.len(),
                });
            }
        }

        used_harnesses.insert(inst.profile.harness.clone());
        if let Some(provider) = &inst.profile.model.provider {
            used_providers.insert(provider.clone());
        } else {
            unknown_provider_count += 1;
        }
        selected.push((*inst).clone());
    }

    if selected.len() < count {
        return Err(SelectionRefusal::InsufficientDiversity {
            requested: count,
            achieved: selected.len(),
        });
    }

    Ok(DiverseSelection {
        instances: selected,
        diversity: DiversityKnown {
            distinct_harnesses: used_harnesses.len(),
            distinct_providers_known: used_providers.len(),
            unknown_provider_count,
        },
    })
}

/// Check which requirements are unmet for a runner instance.
fn check_unmet_requirements(
    instance: &RunnerInstance,
    requirements: &Requirements,
) -> Vec<UnmetRequirement> {
    let mut unmet = Vec::new();

    // Support level.
    if instance.capabilities.support_level < requirements.min_support_level {
        unmet.push(UnmetRequirement::InsufficientSupportLevel {
            required: requirements.min_support_level,
            found: instance.capabilities.support_level,
        });
    }

    // Resume capability.
    if requirements.needs_resume && !instance.capabilities.resumable {
        unmet.push(UnmetRequirement::CannotResume);
    }

    // Model selection.
    if requirements.needs_model_selection && !instance.capabilities.model_selection {
        unmet.push(UnmetRequirement::NeedsModelSelection);
    }

    // Native subagents.
    if requirements.needs_native_subagents && !instance.capabilities.native_subagents {
        unmet.push(UnmetRequirement::NeedsNativeSubagents);
    }

    // Streaming.
    if requirements.needs_streaming && !instance.capabilities.streaming_events {
        unmet.push(UnmetRequirement::NeedsStreaming);
    }

    // Structured lifecycle.
    if requirements.needs_structured_lifecycle && !instance.capabilities.structured_lifecycle {
        unmet.push(UnmetRequirement::NeedsStructuredLifecycle);
    }

    // Fresh context.
    if requirements.fresh_context && !instance.capabilities.fresh_context {
        unmet.push(UnmetRequirement::RequiresFreshContext);
    }

    // Workspace mutation.
    match requirements.workspace_mutation {
        super::selection::Needs::Required => {
            if !instance.capabilities.workspace_mutation {
                unmet.push(UnmetRequirement::WorkspaceMutationForbidden);
            }
        }
        super::selection::Needs::Forbidden => {
            if instance.capabilities.workspace_mutation {
                unmet.push(UnmetRequirement::WorkspaceMutationForbidden);
            }
        }
        super::selection::Needs::DontCare => {}
    }

    // Isolation.
    if instance.capabilities.isolation < requirements.min_isolation {
        unmet.push(UnmetRequirement::InsufficientIsolation {
            required: requirements.min_isolation,
            found: instance.capabilities.isolation,
        });
    }

    // Max depth.
    if instance.capabilities.max_depth < requirements.min_depth_support {
        unmet.push(UnmetRequirement::InsufficientDepthSupport {
            required: requirements.min_depth_support,
            found: instance.capabilities.max_depth,
        });
    }

    unmet
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fabric::registry::{
        Availability, FabricCapabilities, HarnessId, LifecycleSource, ModelFacts, RunnerInstance,
        RunnerProfile, RunnerProfileId,
    };

    fn make_instance(id: &str, harness: &str, support: SupportLevel) -> RunnerInstance {
        RunnerInstance {
            profile: RunnerProfile {
                id: RunnerProfileId::new(id).unwrap(),
                harness: HarnessId::new(harness).unwrap(),
                model: ModelFacts {
                    provider: Some("vendor-a".to_string()),
                    model: Some("model-1".to_string()),
                },
                label: format!("Runner {}", id),
            },
            availability: Availability::Available,
            capabilities: FabricCapabilities {
                support_level: support,
                lifecycle_source: LifecycleSource::Process,
                resumable: true,
                model_selection: true,
                native_subagents: true,
                streaming_events: true,
                structured_lifecycle: true,
                fresh_context: true,
                workspace_mutation: true,
                isolation: Isolation::Process,
                max_depth: 8,
            },
        }
    }

    #[test]
    fn test_multiple_profiles_one_harness() {
        let instances = vec![
            make_instance("profile-1", "harness-a", SupportLevel::Process),
            make_instance("profile-2", "harness-a", SupportLevel::Interactive),
        ];
        let registry = RunnerRegistry::new(instances).unwrap();
        assert_eq!(registry.instances().len(), 2);
    }

    #[test]
    fn test_multiple_harnesses_concurrent() {
        let instances = vec![
            make_instance("profile-1", "harness-a", SupportLevel::Process),
            make_instance("profile-2", "harness-b", SupportLevel::Interactive),
        ];
        let registry = RunnerRegistry::new(instances).unwrap();
        assert_eq!(registry.instances().len(), 2);
    }

    #[test]
    fn test_unknown_harness_fails_closed() {
        let instances = vec![make_instance(
            "profile-1",
            "harness-unknown",
            SupportLevel::Process,
        )];
        let registry = RunnerRegistry::new(instances).unwrap();
        let known = std::collections::HashSet::from([HarnessId::new("harness-a").unwrap()]);
        assert!(registry.with_known_harnesses(known).is_err());
    }

    #[test]
    fn test_duplicate_profile_id_rejected() {
        let instances = vec![
            make_instance("profile-1", "harness-a", SupportLevel::Process),
            make_instance("profile-1", "harness-b", SupportLevel::Interactive),
        ];
        assert!(RunnerRegistry::new(instances).is_err());
    }

    #[test]
    fn test_unavailable_runner_never_selected() {
        let mut inst = make_instance("profile-1", "harness-a", SupportLevel::Process);
        inst.availability = Availability::Unavailable {
            reason: "offline".to_string(),
        };
        let registry = RunnerRegistry::new(vec![inst]).unwrap();
        let result = select(&registry, &Requirements::default(), &Preference::default());
        assert!(matches!(
            result,
            Err(SelectionRefusal::NoEligibleRunner { .. })
        ));
    }

    #[test]
    fn test_capability_mismatch_yields_unmet() {
        let mut inst = make_instance("profile-1", "harness-a", SupportLevel::Process);
        inst.capabilities.support_level = SupportLevel::Interactive;
        let registry = RunnerRegistry::new(vec![inst]).unwrap();

        let req = Requirements {
            min_support_level: SupportLevel::Structured,
            ..Default::default()
        };

        let result = select(&registry, &req, &Preference::default());
        assert!(matches!(
            result,
            Err(SelectionRefusal::NoEligibleRunner { rejections }) if
            rejections.iter().any(|(_, unmet)|
                unmet.iter().any(|u| matches!(u, UnmetRequirement::InsufficientSupportLevel { .. }))
            )
        ));
    }

    #[test]
    fn test_explicit_pin_honored() {
        let inst = make_instance("profile-1", "harness-a", SupportLevel::Process);
        let registry = RunnerRegistry::new(vec![inst.clone()]).unwrap();
        let pref = Preference {
            pin: Some(RunnerProfileId::new("profile-1").unwrap()),
            ..Default::default()
        };
        let result = select(&registry, &Requirements::default(), &pref);
        assert!(result.is_ok());
    }

    #[test]
    fn test_pin_violates_requirements_no_fallback() {
        let inst = make_instance("profile-1", "harness-a", SupportLevel::Interactive);
        let registry = RunnerRegistry::new(vec![inst]).unwrap();

        let req = Requirements {
            min_support_level: SupportLevel::Native,
            ..Default::default()
        };

        let pref = Preference {
            pin: Some(RunnerProfileId::new("profile-1").unwrap()),
            ..Default::default()
        };

        let result = select(&registry, &req, &pref);
        assert!(matches!(
            result,
            Err(SelectionRefusal::PinViolatesRequirements { .. })
        ));
    }

    #[test]
    fn test_pin_unavailable_refuses() {
        let mut inst = make_instance("profile-1", "harness-a", SupportLevel::Process);
        inst.availability = Availability::Unavailable {
            reason: "offline".to_string(),
        };
        let registry = RunnerRegistry::new(vec![inst]).unwrap();
        let pref = Preference {
            pin: Some(RunnerProfileId::new("profile-1").unwrap()),
            ..Default::default()
        };
        let result = select(&registry, &Requirements::default(), &pref);
        assert!(matches!(
            result,
            Err(SelectionRefusal::PinUnavailable { .. })
        ));
    }

    #[test]
    fn test_selection_deterministic_regardless_input_order() {
        let instances1 = vec![
            make_instance("profile-1", "harness-a", SupportLevel::Process),
            make_instance("profile-2", "harness-b", SupportLevel::Interactive),
            make_instance("profile-3", "harness-c", SupportLevel::Session),
        ];
        let instances2 = vec![
            make_instance("profile-3", "harness-c", SupportLevel::Session),
            make_instance("profile-1", "harness-a", SupportLevel::Process),
            make_instance("profile-2", "harness-b", SupportLevel::Interactive),
        ];

        let registry1 = RunnerRegistry::new(instances1).unwrap();
        let registry2 = RunnerRegistry::new(instances2).unwrap();

        let result1 = select(&registry1, &Requirements::default(), &Preference::default()).unwrap();
        let result2 = select(&registry2, &Requirements::default(), &Preference::default()).unwrap();

        assert_eq!(result1.instance.profile.id, result2.instance.profile.id);
    }

    #[test]
    fn test_preference_breaks_ties_not_overrides() {
        let mut inst1 = make_instance("profile-1", "harness-a", SupportLevel::Process);
        inst1.profile.model.provider = Some("vendor-x".to_string());

        let mut inst2 = make_instance("profile-2", "harness-b", SupportLevel::Process);
        inst2.profile.model.provider = Some("vendor-y".to_string());

        let registry = RunnerRegistry::new(vec![inst1, inst2]).unwrap();

        // Preference for harness-b should break the tie.
        let pref = Preference {
            preferred_harness: Some(HarnessId::new("harness-b").unwrap()),
            ..Default::default()
        };

        let result = select(&registry, &Requirements::default(), &pref).unwrap();
        assert_eq!(
            result.instance.profile.harness,
            HarnessId::new("harness-b").unwrap()
        );
    }

    #[test]
    fn test_diversity_distinct_harnesses() {
        let instances = vec![
            make_instance("profile-1", "harness-a", SupportLevel::Process),
            make_instance("profile-2", "harness-b", SupportLevel::Interactive),
            make_instance("profile-3", "harness-c", SupportLevel::Session),
        ];
        let registry = RunnerRegistry::new(instances).unwrap();

        let diversity_req = DiversityRequest {
            require_distinct_harness: true,
            require_distinct_provider: false,
        };

        let result =
            select_diverse(&registry, &Requirements::default(), 3, &diversity_req).unwrap();

        assert_eq!(result.instances.len(), 3);
        assert_eq!(result.diversity.distinct_harnesses, 3);
    }

    #[test]
    fn test_diversity_refused_unknown_providers() {
        let mut inst = make_instance("profile-1", "harness-a", SupportLevel::Process);
        inst.profile.model.provider = None;

        let registry = RunnerRegistry::new(vec![inst]).unwrap();

        let diversity_req = DiversityRequest {
            require_distinct_harness: false,
            require_distinct_provider: true,
        };

        let result = select_diverse(&registry, &Requirements::default(), 1, &diversity_req);

        assert!(matches!(
            result,
            Err(SelectionRefusal::InsufficientDiversity { .. })
        ));
    }

    #[test]
    fn test_diversity_known_counts_correct() {
        let mut inst1 = make_instance("profile-1", "harness-a", SupportLevel::Process);
        inst1.profile.model.provider = Some("vendor-x".to_string());

        let mut inst2 = make_instance("profile-2", "harness-b", SupportLevel::Interactive);
        inst2.profile.model.provider = Some("vendor-y".to_string());

        let mut inst3 = make_instance("profile-3", "harness-c", SupportLevel::Session);
        inst3.profile.model.provider = None;

        let registry = RunnerRegistry::new(vec![inst1, inst2, inst3]).unwrap();

        let diversity_req = DiversityRequest {
            require_distinct_harness: true,
            require_distinct_provider: false,
        };

        let result =
            select_diverse(&registry, &Requirements::default(), 3, &diversity_req).unwrap();

        assert_eq!(result.diversity.distinct_harnesses, 3);
        assert_eq!(result.diversity.distinct_providers_known, 2);
        assert_eq!(result.diversity.unknown_provider_count, 1);
    }
}
