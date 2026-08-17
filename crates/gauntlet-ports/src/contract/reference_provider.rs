//! The `ReferenceProvider` shared contract suite (spec §6.2; §5.3.3, I1).
//!
//! Pinned semantics: the pipeline round-trips — `resolve` → `normalize` →
//! `digest` → `freeze` produce a version whose identity and digest are the
//! reference's own, and `verify_unchanged` right after freezing is `Ok`;
//! and `verify_unchanged` is the I1 tripwire — once the backed artifact
//! changes, it fails with [`ReferenceProviderError::ReferenceMutated`]
//! carrying the frozen and current digests. "Freeze, digest, re-verify"
//! (AGENTS.md, Architecture).

use std::cell::RefCell;
use std::rc::Rc;

use gauntlet_domain::{ReferenceVersion, RunId};

use crate::contract::ContractFinding;
use crate::deadline::Deadline;
use crate::reference_provider::{ReferenceProvider, ReferenceProviderError};

/// The suite's name in findings.
pub const SUITE: &str = "reference_provider";

const BASE_INSTANT: u64 = 1_700_000_000;
/// The suite's bound span, on its own line (numeric literals never share a
/// line with a duration-shaped identifier).
const BOUND_SPAN: u64 = 60;

/// Constructs the provider under test over a single artifact whose content
/// the suite drives through `artifact` — the factory's adapter resolves
/// from that content, so the suite can rewrite the standard mid-run to
/// trip the I1 wire.
pub trait ReferenceProviderFactory {
    fn label(&self) -> &'static str;

    fn build(&self, artifact: Rc<RefCell<String>>) -> Result<Box<dyn ReferenceProvider>, String>;
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

fn freeze_and_verify(
    provider: &dyn ReferenceProvider,
    case: &'static str,
    rule: &'static str,
) -> Result<ReferenceVersion, ContractFinding> {
    let draft = provider
        .resolve("keep the standard fixed", bound())
        .map_err(|error| finding(case, rule, format!("resolve failed: {error}")))?;
    let reference = provider
        .normalize(draft, bound())
        .map_err(|error| finding(case, rule, format!("normalize failed: {error}")))?;
    let version = provider
        .freeze(&run(), &reference, bound())
        .map_err(|error| finding(case, rule, format!("freeze failed: {error}")))?;
    if version.reference_id() != reference.id() || version.digest() != reference.digest() {
        return Err(finding(
            case,
            rule,
            format!(
                "the frozen version ({}, digest {}) does not carry the reference's own \
                 identity ({}, digest {})",
                version.reference_id(),
                version.digest(),
                reference.id(),
                reference.digest()
            ),
        ));
    }
    provider
        .verify_unchanged(&version, bound())
        .map_err(|error| {
            finding(
                case,
                rule,
                format!("verify_unchanged right after freezing failed: {error}"),
            )
        })?;
    Ok(version)
}

/// Case: the whole pipeline round-trips against a stable artifact —
/// resolve, normalize, digest, freeze; the version carries the reference's
/// identity; verify_unchanged confirms the standard has not moved.
fn pipeline_round_trips(factory: &dyn ReferenceProviderFactory) -> Vec<ContractFinding> {
    const CASE: &str = "pipeline_round_trips";
    const RULE: &str = "spec 6.2 ReferenceProvider: resolve -> normalize -> digest -> freeze; verify_unchanged on an untouched artifact is Ok";
    let artifact = Rc::new(RefCell::new("the standard text".to_owned()));
    let Ok(provider) = factory.build(artifact) else {
        return vec![finding(
            CASE,
            RULE,
            "the factory could not build a provider for the case".to_owned(),
        )];
    };
    match freeze_and_verify(provider.as_ref(), CASE, RULE) {
        Ok(_) => Vec::new(),
        Err(violation) => vec![violation],
    }
}

/// Case: the I1 tripwire — once the artifact changes, `verify_unchanged`
/// fails with `ReferenceMutated` carrying the frozen and the current
/// digests; the failure is loud and typed (spec §11.5: "Reference digest
/// mismatch | Abort the run; do not retry").
fn the_tripwire_watches_the_artifact(
    factory: &dyn ReferenceProviderFactory,
) -> Vec<ContractFinding> {
    const CASE: &str = "the_tripwire_watches_the_artifact";
    const RULE: &str = "spec 6.2 (I1): verify_unchanged re-derives the digest and fails with ReferenceMutated when the artifact moved";
    let artifact = Rc::new(RefCell::new("the standard text".to_owned()));
    let Ok(provider) = factory.build(artifact.clone()) else {
        return vec![finding(
            CASE,
            RULE,
            "the factory could not build a provider for the case".to_owned(),
        )];
    };
    let version = match freeze_and_verify(provider.as_ref(), CASE, RULE) {
        Ok(version) => version,
        Err(violation) => return vec![violation],
    };
    let frozen = version.digest().clone();
    *artifact.borrow_mut() = "the operator edited the standard".to_owned();
    match provider.verify_unchanged(&version, bound()) {
        Err(ReferenceProviderError::ReferenceMutated { expected, actual }) => {
            if expected == frozen && actual != frozen {
                Vec::new()
            } else {
                vec![finding(
                    CASE,
                    RULE,
                    format!(
                        "the tripwire fired with expected {expected} and actual {actual}; the \
                         frozen digest is {frozen} — the cross-check must name both sides"
                    ),
                )]
            }
        }
        Err(other) => vec![finding(
            CASE,
            RULE,
            format!("a moved artifact was refused with {other} instead of ReferenceMutated"),
        )],
        Ok(()) => vec![finding(
            CASE,
            RULE,
            "verify_unchanged stayed Ok after the artifact changed — the I1 tripwire is absent"
                .to_owned(),
        )],
    }
}

/// Runs the full `ReferenceProvider` contract suite against one factory.
pub fn reference_provider_suite(factory: &dyn ReferenceProviderFactory) -> Vec<ContractFinding> {
    let mut findings = Vec::new();
    findings.extend(pipeline_round_trips(factory));
    findings.extend(the_tripwire_watches_the_artifact(factory));
    findings
}
