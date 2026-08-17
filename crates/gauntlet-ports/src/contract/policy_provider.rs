//! The `PolicyProvider` shared contract suite (spec §6.2; §17.2, §17.3).
//!
//! Pinned semantics: `policy_for` refuses the `irreversible` tier
//! STRUCTURALLY — "**No policy exists.** The action never reaches the
//! sandbox port unattended" (spec §17.3) holds even when a policy artifact
//! exists for it; seeded policies and permission sets resolve verbatim;
//! an unseeded tier is the typed [`PolicyProviderError::NoPolicyForTier`].

use gauntlet_domain::{
    BlastRadius, PermissionSetRef, RoleId, SandboxPolicyRef, UnitId, WorkspaceRef,
};

use crate::contract::ContractFinding;
use crate::deadline::Deadline;
use crate::policy_provider::{
    CredentialScope, Enforcement, PermissionSet, PolicyProvider, PolicyProviderError,
};
use crate::sandbox::{NetworkPosture, SandboxBackendRef, SandboxPolicy};
use crate::tool_provider::SemanticCapability;

/// The suite's name in findings.
pub const SUITE: &str = "policy_provider";

const BASE_INSTANT: u64 = 1_700_000_000;
/// The suite's bound span, on its own line (numeric literals never share a
/// line with a duration-shaped identifier).
const BOUND_SPAN: u64 = 60;

/// Constructs the provider under test over the given policy artifacts and
/// permission sets (a clean slate per case).
pub trait PolicyProviderFactory {
    fn label(&self) -> &'static str;

    fn build(
        &self,
        policies: Vec<(BlastRadius, SandboxPolicy)>,
        permission_sets: Vec<(RoleId, PermissionSet)>,
    ) -> Result<Box<dyn PolicyProvider>, String>;
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

fn workspace() -> WorkspaceRef {
    WorkspaceRef::new("ws-suite-01").expect("suite workspace ref")
}

fn policy(reference: &str, tier: BlastRadius) -> SandboxPolicy {
    SandboxPolicy {
        reference: SandboxPolicyRef::new(reference).expect("suite policy ref"),
        tier,
        backend: SandboxBackendRef::new("local-rung").expect("suite backend ref"),
        network: NetworkPosture::DenyByDefault,
        writable_roots: vec!["ws-suite".to_owned()],
        denied_paths: Vec::new(),
    }
}

fn permission_set(reference: &str) -> PermissionSet {
    PermissionSet {
        reference: PermissionSetRef::new(reference).expect("suite permission ref"),
        read_paths: vec!["ws-suite".to_owned()],
        write_paths: vec!["ws-suite".to_owned()],
        network: NetworkPosture::DenyByDefault,
        tools: vec![SemanticCapability::CodeNav, SemanticCapability::Schema],
        credential_scope: CredentialScope::None,
        destructive: false,
        enforcement: Enforcement::Advisory,
    }
}

/// Runs the full `PolicyProvider` contract suite against one factory.
pub fn policy_provider_suite(factory: &dyn PolicyProviderFactory) -> Vec<ContractFinding> {
    let mut findings = Vec::new();
    findings.extend(irreversible_is_refused_structurally(factory));
    findings.extend(seeded_policies_resolve_verbatim(factory));
    findings.extend(seeded_permission_sets_resolve_verbatim(factory));
    findings.extend(unseeded_tiers_are_typed_refusals(factory));
    findings
}

/// Case: `policy_for(irreversible, ...)` is refused even when an artifact
/// exists for it — the refusal is the REQUIREMENT (spec §17.3), not an
/// absence.
fn irreversible_is_refused_structurally(
    factory: &dyn PolicyProviderFactory,
) -> Vec<ContractFinding> {
    const CASE: &str = "irreversible_is_refused_structurally";
    const RULE: &str = "spec 17.3: no policy exists for the irreversible tier — the action never reaches the sandbox port unattended; this holds even when an artifact exists";
    let seeded = policy("policy-irreversible-03", BlastRadius::Irreversible);
    let Ok(provider) = factory.build(vec![(BlastRadius::Irreversible, seeded)], Vec::new()) else {
        return vec![finding(
            CASE,
            RULE,
            "the factory could not build a provider for the case".to_owned(),
        )];
    };
    match provider.policy_for(BlastRadius::Irreversible, &workspace(), bound()) {
        Err(PolicyProviderError::NoPolicyForTier {
            tier: BlastRadius::Irreversible,
        }) => Vec::new(),
        Err(other) => vec![finding(
            CASE,
            RULE,
            format!("irreversible was refused with {other} instead of NoPolicyForTier"),
        )],
        Ok(artifact) => vec![finding(
            CASE,
            RULE,
            format!(
                "a policy for the irreversible tier was resolved ({artifact:?}) — the refusal \
                 is a structural requirement, not a missing artifact"
            ),
        )],
    }
}

/// Case: seeded policies resolve verbatim — the artifact the runtime
/// resolves is the artifact the repo owns (§6.2: "Resolves repo-owned
/// policy artifacts").
fn seeded_policies_resolve_verbatim(factory: &dyn PolicyProviderFactory) -> Vec<ContractFinding> {
    const CASE: &str = "seeded_policies_resolve_verbatim";
    const RULE: &str = "spec 6.2 PolicyProvider: policy_for(tier, workspace) resolves the repo-owned artifact, verbatim";
    let contained = policy("policy-contained-01", BlastRadius::Contained);
    let local = policy("policy-local-02", BlastRadius::Local);
    let Ok(provider) = factory.build(
        vec![
            (BlastRadius::Contained, contained.clone()),
            (BlastRadius::Local, local.clone()),
        ],
        Vec::new(),
    ) else {
        return vec![finding(
            CASE,
            RULE,
            "the factory could not build a provider for the case".to_owned(),
        )];
    };
    let mut findings = Vec::new();
    for (tier, expected) in [
        (BlastRadius::Contained, contained),
        (BlastRadius::Local, local),
    ] {
        match provider.policy_for(tier, &workspace(), bound()) {
            Ok(artifact) if artifact == expected => {}
            Ok(artifact) => findings.push(finding(
                CASE,
                RULE,
                format!("for tier {tier} the provider resolved {artifact:?} instead of the seeded artifact"),
            )),
            Err(error) => findings.push(finding(
                CASE,
                RULE,
                format!("for tier {tier} a seeded policy failed to resolve: {error}"),
            )),
        }
    }
    findings
}

/// Case: a seeded permission set resolves verbatim (§6.2
/// `permissions_for(role, unit)`; §17.2 fields untouched in transit).
fn seeded_permission_sets_resolve_verbatim(
    factory: &dyn PolicyProviderFactory,
) -> Vec<ContractFinding> {
    const CASE: &str = "seeded_permission_sets_resolve_verbatim";
    const RULE: &str = "spec 6.2 PolicyProvider: permissions_for(role, unit) resolves the repo-owned set, verbatim";
    let set = permission_set("perm-suite-builder");
    let Ok(provider) = factory.build(Vec::new(), vec![(RoleId::Builder, set.clone())]) else {
        return vec![finding(
            CASE,
            RULE,
            "the factory could not build a provider for the case".to_owned(),
        )];
    };
    let unit = UnitId::new("AK-014").expect("suite unit id");
    match provider.permissions_for(&RoleId::Builder, &unit, bound()) {
        Ok(resolved) if resolved == set => Vec::new(),
        Ok(resolved) => vec![finding(
            CASE,
            RULE,
            format!("the resolved set {resolved:?} differs from the seeded set {set:?}"),
        )],
        Err(error) => vec![finding(
            CASE,
            RULE,
            format!("a seeded permission set failed to resolve: {error}"),
        )],
    }
}

/// Case: an unseeded tier is the typed `NoPolicyForTier` refusal (for any
/// tier other than irreversible this means the artifacts are incomplete,
/// spec §17.3's sibling clause).
fn unseeded_tiers_are_typed_refusals(factory: &dyn PolicyProviderFactory) -> Vec<ContractFinding> {
    const CASE: &str = "unseeded_tiers_are_typed_refusals";
    const RULE: &str =
        "spec 6.2 + port contract: a tier with no policy artifact is NoPolicyForTier{tier}";
    let Ok(provider) = factory.build(
        vec![(
            BlastRadius::Contained,
            policy("policy-contained-01", BlastRadius::Contained),
        )],
        Vec::new(),
    ) else {
        return vec![finding(
            CASE,
            RULE,
            "the factory could not build a provider for the case".to_owned(),
        )];
    };
    match provider.policy_for(BlastRadius::Local, &workspace(), bound()) {
        Err(PolicyProviderError::NoPolicyForTier {
            tier: BlastRadius::Local,
        }) => Vec::new(),
        Err(other) => vec![finding(
            CASE,
            RULE,
            format!("an unseeded tier was refused with {other} instead of NoPolicyForTier"),
        )],
        Ok(artifact) => vec![finding(
            CASE,
            RULE,
            format!("an unseeded tier resolved {artifact:?} — artifacts must exist to resolve"),
        )],
    }
}
