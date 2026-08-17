//! The `ToolProvider` shared contract suite (spec §6.2; §8.4).
//!
//! Pinned semantics: the availability report is honest and inside the
//! fixed §8.4 vocabulary — `available()` reports exactly the set the
//! provider can currently resolve, resolve satisfies that declaration
//! (each available capability resolves to a binding OF that capability),
//! and a capability outside the resolvable set is the typed
//! [`ToolProviderError::Unresolved`] refusal (spec §8.4: "when a required
//! capability cannot be resolved, the run does not start").

use crate::contract::ContractFinding;
use crate::deadline::Deadline;
use crate::tool_provider::{SemanticCapability, ToolProvider, ToolProviderError};

/// The suite's name in findings.
pub const SUITE: &str = "tool_provider";

const BASE_INSTANT: u64 = 1_700_000_000;
/// The suite's bound span, on its own line (numeric literals never share a
/// line with a duration-shaped identifier).
const BOUND_SPAN: u64 = 60;

/// Constructs the provider under test. `resolvable` is the exact set of
/// §8.4 capabilities the adapter can currently resolve.
pub trait ToolProviderFactory {
    fn label(&self) -> &'static str;

    fn build(&self, resolvable: Vec<SemanticCapability>) -> Result<Box<dyn ToolProvider>, String>;
}

fn finding(case: &'static str, rule: &'static str, detail: String) -> ContractFinding {
    ContractFinding {
        suite: SUITE,
        case,
        rule,
        detail,
    }
}

fn bound() -> Deadline {
    Deadline::at_epoch(gauntlet_domain::EpochSeconds::new(
        BASE_INSTANT + BOUND_SPAN,
    ))
}

/// Case: `available()` reports exactly the resolvable set — nothing more,
/// nothing less (spec §6.2 `available() -> [SemanticCapability]`).
fn available_reports_exactly_the_resolvable_set(
    factory: &dyn ToolProviderFactory,
) -> Vec<ContractFinding> {
    const CASE: &str = "available_reports_exactly_the_resolvable_set";
    const RULE: &str =
        "spec 6.2 ToolProvider: available() is every capability the provider can currently resolve";
    let resolvable = vec![
        SemanticCapability::CodeNav,
        SemanticCapability::Versioning,
        SemanticCapability::Schema,
    ];
    let Ok(provider) = factory.build(resolvable.clone()) else {
        return vec![finding(
            CASE,
            RULE,
            "the factory could not build a provider for the case".to_owned(),
        )];
    };
    let available = provider.available();
    let mut findings = Vec::new();
    for capability in &resolvable {
        if !available.contains(capability) {
            findings.push(finding(
                CASE,
                RULE,
                format!("{capability} is resolvable but absent from available()"),
            ));
        }
    }
    for capability in &available {
        if !resolvable.contains(capability) {
            findings.push(finding(
                CASE,
                RULE,
                format!("{capability} appears in available() but was declared unresolvable"),
            ));
        }
    }
    findings
}

/// Case: every available capability resolves, and the binding names the
/// capability it serves (tools bind by capability, never by product, spec
/// §8.4).
fn resolve_satisfies_the_declared_availability(
    factory: &dyn ToolProviderFactory,
) -> Vec<ContractFinding> {
    const CASE: &str = "resolve_satisfies_the_declared_availability";
    const RULE: &str =
        "spec 8.4: tools bind by capability; a binding names the capability it serves";
    let resolvable = vec![
        SemanticCapability::CodeNav,
        SemanticCapability::Versioning,
        SemanticCapability::Schema,
    ];
    let Ok(provider) = factory.build(resolvable) else {
        return vec![finding(
            CASE,
            RULE,
            "the factory could not build a provider for the case".to_owned(),
        )];
    };
    let mut findings = Vec::new();
    for capability in provider.available() {
        match provider.resolve(capability, bound()) {
            Ok(binding) if binding.capability == capability => {}
            Ok(binding) => findings.push(finding(
                CASE,
                RULE,
                format!(
                    "resolving {capability} returned a binding for {}",
                    binding.capability
                ),
            )),
            Err(error) => findings.push(finding(
                CASE,
                RULE,
                format!("resolving the available capability {capability} failed: {error}"),
            )),
        }
    }
    findings
}

/// Case: a capability outside the resolvable set is the typed `Unresolved`
/// refusal (spec §8.4: a required capability that cannot be resolved stops
/// the run — it is never silently bound to something else).
fn unavailable_capabilities_are_typed_refusals(
    factory: &dyn ToolProviderFactory,
) -> Vec<ContractFinding> {
    const CASE: &str = "unavailable_capabilities_are_typed_refusals";
    const RULE: &str = "spec 8.4 + port contract: a capability with no binding is Unresolved, never a substitute binding";
    let Ok(provider) = factory.build(vec![SemanticCapability::CodeNav]) else {
        return vec![finding(
            CASE,
            RULE,
            "the factory could not build a provider for the case".to_owned(),
        )];
    };
    let absent = SemanticCapability::Browser;
    match provider.resolve(absent, bound()) {
        Err(ToolProviderError::Unresolved { capability }) if capability == absent => Vec::new(),
        Err(other) => vec![finding(
            CASE,
            RULE,
            format!("an unresolvable capability was refused with {other} instead of Unresolved"),
        )],
        Ok(binding) => vec![finding(
            CASE,
            RULE,
            format!(
                "an unresolvable capability resolved to {} — no substitutes for an unbound \
                 capability",
                binding.capability
            ),
        )],
    }
}

/// Runs the full `ToolProvider` contract suite against one factory.
pub fn tool_provider_suite(factory: &dyn ToolProviderFactory) -> Vec<ContractFinding> {
    let mut findings = Vec::new();
    findings.extend(available_reports_exactly_the_resolvable_set(factory));
    findings.extend(resolve_satisfies_the_declared_availability(factory));
    findings.extend(unavailable_capabilities_are_typed_refusals(factory));
    findings
}
