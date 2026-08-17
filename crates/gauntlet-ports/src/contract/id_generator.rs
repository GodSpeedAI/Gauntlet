//! The `IdGenerator` shared contract suite (spec §6.2: "Injectable; tests
//! use a seeded generator").
//!
//! Pinned semantics: ids are distinct within an instance (no collisions on
//! the same generator); the grammar is guaranteed by the types themselves
//! (`RunId`/`InvocationId` validate at construction); when the FACTORY
//! declares the adapter deterministic, two instances replay identical
//! sequences.

use gauntlet_domain::InvocationId;
use gauntlet_domain::RunId;

use crate::contract::ContractFinding;
use crate::id_generator::IdGenerator;

/// The suite's name in findings.
pub const SUITE: &str = "id_generator";

/// How many of each id the suite draws.
const DRAW: u32 = 8;

/// Constructs the generator under test and declares whether it is
/// deterministic.
pub trait IdGeneratorFactory {
    fn label(&self) -> &'static str;

    fn build(&self) -> Box<dyn IdGenerator>;

    /// Whether identical factory configurations replay identical sequences
    /// (spec §6.2: "tests use a seeded generator").
    fn deterministic(&self) -> bool;
}

fn finding(case: &'static str, rule: &'static str, detail: String) -> ContractFinding {
    ContractFinding {
        suite: SUITE,
        case,
        rule,
        detail,
    }
}

/// Case: ids drawn from one generator never collide.
fn ids_never_collide(factory: &dyn IdGeneratorFactory) -> Vec<ContractFinding> {
    const CASE: &str = "ids_never_collide";
    const RULE: &str = "spec 5.2: ids are collision-resistant";
    let generator = factory.build();
    let mut runs: Vec<RunId> = Vec::new();
    let mut invocations: Vec<InvocationId> = Vec::new();
    for _ in 0..DRAW {
        runs.push(generator.run_id());
        invocations.push(generator.invocation_id());
    }
    let mut findings = Vec::new();
    if runs.iter().collect::<std::collections::BTreeSet<_>>().len() != runs.len() {
        findings.push(finding(
            CASE,
            RULE,
            format!("{} run ids drawn, with collisions", runs.len()),
        ));
    }
    if invocations
        .iter()
        .collect::<std::collections::BTreeSet<_>>()
        .len()
        != invocations.len()
    {
        findings.push(finding(
            CASE,
            RULE,
            format!(
                "{} invocation ids drawn, with collisions",
                invocations.len()
            ),
        ));
    }
    findings
}

/// Case: when the factory declares determinism, two instances replay
/// identical sequences. (A ULID-minting adapter declares false; the case
/// does not apply to it.)
fn deterministic_generators_replay(factory: &dyn IdGeneratorFactory) -> Vec<ContractFinding> {
    const CASE: &str = "deterministic_generators_replay";
    const RULE: &str = "spec 6.2: a seeded generator replays identically per instance";
    if !factory.deterministic() {
        return Vec::new();
    }
    let one = factory.build();
    let two = factory.build();
    for _ in 0..DRAW {
        if one.run_id() != two.run_id() || one.invocation_id() != two.invocation_id() {
            return vec![finding(
                CASE,
                RULE,
                "two identically-seeded instances diverged in their sequence".to_owned(),
            )];
        }
    }
    Vec::new()
}

/// Runs the full `IdGenerator` contract suite against one factory.
pub fn id_generator_suite(factory: &dyn IdGeneratorFactory) -> Vec<ContractFinding> {
    let mut findings = Vec::new();
    findings.extend(ids_never_collide(factory));
    findings.extend(deterministic_generators_replay(factory));
    findings
}
