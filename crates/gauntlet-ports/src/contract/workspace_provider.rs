//! The `WorkspaceProvider` shared contract suite (spec §6.2; §10.4, §17.5).
//!
//! Pinned semantics: `create` is per run and per unit-or-slice, and units
//! dispatched in parallel never share a workspace (spec §10.4 — distinct
//! subjects get distinct workspaces, so a shared workspace is not
//! reachable through this API); every workspace lies under the configured
//! root with a §17.5.3-sanitized key; cleanup releases the workspace so the
//! provider remains usable.

use std::path::PathBuf;

use gauntlet_domain::{RunId, UnitId};

use crate::contract::ContractFinding;
use crate::deadline::Deadline;
use crate::workspace_provider::{WorkspaceProvider, WorkspaceSubject};

/// The suite's name in findings.
pub const SUITE: &str = "workspace_provider";

const BASE_INSTANT: u64 = 1_700_000_000;
/// The suite's bound span, on its own line (numeric literals never share a
/// line with a duration-shaped identifier).
const BOUND_SPAN: u64 = 60;

/// Constructs the provider under test. The provider allocates under
/// [`WorkspaceProviderFactory::root`]; `build` must return an ISOLATED
/// instance.
pub trait WorkspaceProviderFactory {
    fn label(&self) -> &'static str;

    fn root(&self) -> PathBuf;

    fn build(&self) -> Result<Box<dyn WorkspaceProvider>, String>;
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

fn run() -> RunId {
    RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("suite run id is a ULID")
}

/// Case: distinct subjects get distinct workspaces, each under the
/// configured root with a sanitized key (§10.4: units dispatched in
/// parallel never share a workspace; §17.5.2/§17.5.3 containment and key
/// sanitation).
fn create_isolates_parallel_subjects(
    factory: &dyn WorkspaceProviderFactory,
) -> Vec<ContractFinding> {
    const CASE: &str = "create_isolates_parallel_subjects";
    const RULE: &str = "spec 10.4 + 17.5: create is per unit-or-slice under the configured root; parallel subjects never share a workspace; keys sanitize to [A-Za-z0-9._-]";
    let Ok(provider) = factory.build() else {
        return vec![finding(
            CASE,
            RULE,
            "the factory could not build a provider for the case".to_owned(),
        )];
    };
    let root = factory.root();
    let one = UnitId::new("AK-014").expect("suite unit id");
    let two = UnitId::new("AK-015").expect("suite unit id");
    let first = match provider.create(&run(), WorkspaceSubject::Unit(one), bound()) {
        Ok(workspace) => workspace,
        Err(error) => {
            return vec![finding(
                CASE,
                RULE,
                format!("creating the first workspace failed: {error}"),
            )];
        }
    };
    let second = match provider.create(&run(), WorkspaceSubject::Unit(two), bound()) {
        Ok(workspace) => workspace,
        Err(error) => {
            return vec![finding(
                CASE,
                RULE,
                format!("creating the second workspace failed: {error}"),
            )];
        }
    };
    let mut findings = Vec::new();
    if first.reference == second.reference || first.root == second.root {
        findings.push(finding(
            CASE,
            RULE,
            "two distinct units were handed the same workspace — parallel subjects never share"
                .to_owned(),
        ));
    }
    for (label, workspace) in [("the first", &first), ("the second", &second)] {
        if !workspace.root.starts_with(&root) {
            findings.push(finding(
                CASE,
                RULE,
                format!(
                    "{label} workspace root {:?} escapes the configured root {:?} (spec 17.5.2)",
                    workspace.root, root
                ),
            ));
        }
        if !workspace.key_is_sanitized() {
            findings.push(finding(
                CASE,
                RULE,
                format!(
                    "{label} workspace key {:?} is outside [A-Za-z0-9._-]",
                    workspace.key
                ),
            ));
        }
    }
    findings
}

/// Case: cleanup releases a workspace and the provider stays usable —
/// cleanup succeeds on an allocated workspace, and the provider still
/// allocates afterwards (spec §6.2 `cleanup`; §21.2: failures are typed,
/// and a typed error here is fine; what is not fine is a workspace that
/// cannot be released).
fn cleanup_releases_the_workspace(factory: &dyn WorkspaceProviderFactory) -> Vec<ContractFinding> {
    const CASE: &str = "cleanup_releases_the_workspace";
    const RULE: &str =
        "spec 6.2 WorkspaceProvider: cleanup removes a workspace; the provider remains usable";
    let Ok(provider) = factory.build() else {
        return vec![finding(
            CASE,
            RULE,
            "the factory could not build a provider for the case".to_owned(),
        )];
    };
    let unit = UnitId::new("AK-014").expect("suite unit id");
    let workspace = match provider.create(&run(), WorkspaceSubject::Unit(unit.clone()), bound()) {
        Ok(workspace) => workspace,
        Err(error) => {
            return vec![finding(
                CASE,
                RULE,
                format!("creating the workspace failed: {error}"),
            )];
        }
    };
    let mut findings = Vec::new();
    if let Err(error) = provider.cleanup(&workspace.reference, bound()) {
        findings.push(finding(
            CASE,
            RULE,
            format!("cleaning up an allocated workspace failed: {error}"),
        ));
        return findings;
    }
    // The provider must still be able to allocate after a cleanup.
    if let Err(error) = provider.create(&run(), WorkspaceSubject::Unit(unit), bound()) {
        findings.push(finding(
            CASE,
            RULE,
            format!("creating a workspace after cleanup failed: {error}"),
        ));
    }
    findings
}

/// Runs the full `WorkspaceProvider` contract suite against one factory.
pub fn workspace_provider_suite(factory: &dyn WorkspaceProviderFactory) -> Vec<ContractFinding> {
    let mut findings = Vec::new();
    findings.extend(create_isolates_parallel_subjects(factory));
    findings.extend(cleanup_releases_the_workspace(factory));
    findings
}
