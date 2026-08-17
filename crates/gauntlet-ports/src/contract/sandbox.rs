//! The `Sandbox` shared contract suite (spec §6.2; §17.3, §21.3).
//!
//! Pinned semantics: validation precedes every use — a validated policy
//! wraps, and the wrapped command carries its policy and its validation;
//! a wrap that hands over a validation earned by a DIFFERENT policy is a
//! typed refusal (the structural chain: `wrap` requires the
//! [`PolicyValidation`] that `validate` produced for the SAME policy).
//! Validation failures escalate; they never fall back to unsandboxed
//! execution (§21.3).

use gauntlet_domain::{BlastRadius, SandboxPolicyRef};

use crate::contract::ContractFinding;
use crate::deadline::Deadline;
use crate::sandbox::{NetworkPosture, Sandbox, SandboxBackendRef, SandboxPolicy};

/// The suite's name in findings.
pub const SUITE: &str = "sandbox";

const BASE_INSTANT: u64 = 1_700_000_000;
/// The suite's bound span, on its own line (numeric literals never share a
/// line with a duration-shaped identifier).
const BOUND_SPAN: u64 = 60;

/// Constructs the sandbox under test. `build` must return an ISOLATED
/// instance: cases assume a clean slate.
pub trait SandboxFactory {
    fn label(&self) -> &'static str;

    fn build(&self) -> Result<Box<dyn Sandbox>, String>;
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

fn policy(reference: &str) -> SandboxPolicy {
    SandboxPolicy {
        reference: SandboxPolicyRef::new(reference).expect("suite policy ref"),
        tier: BlastRadius::Contained,
        backend: SandboxBackendRef::new("local-rung").expect("suite backend ref"),
        network: NetworkPosture::DenyByDefault,
        writable_roots: vec!["ws-suite".to_owned()],
        denied_paths: vec!["ws-forbidden".to_owned()],
    }
}

/// Case: `validate` then `wrap` — the clean validation authorizes the wrap;
/// the wrapped command carries the same policy reference, the same
/// validation, and the exact invocation text (spec §6.2 `wrap(command,
/// policy)` with the validation threaded through).
fn validation_precedes_every_use(factory: &dyn SandboxFactory) -> Vec<ContractFinding> {
    const CASE: &str = "validation_precedes_every_use";
    const RULE: &str = "spec 6.2 Sandbox: validate(policy) precedes wrap(command, policy); the wrapped command carries its validation";
    let Ok(sandbox) = factory.build() else {
        return vec![finding(
            CASE,
            RULE,
            "the factory could not build a sandbox for the case".to_owned(),
        )];
    };
    let policy = policy("policy-contained-01");
    let validation = match sandbox.validate(&policy, bound()) {
        Ok(validation) => validation,
        Err(error) => {
            return vec![finding(
                CASE,
                RULE,
                format!("validating a well-formed policy failed: {error}"),
            )];
        }
    };
    let mut findings = Vec::new();
    if !validation.is_clean() {
        findings.push(finding(
            CASE,
            RULE,
            format!(
                "the validation of a well-formed policy carries {} finding(s)",
                validation.findings.len()
            ),
        ));
    }
    if validation.policy != policy.reference || validation.backend != policy.backend {
        findings.push(finding(
            CASE,
            RULE,
            "the validation does not name the policy and backend it validated".to_owned(),
        ));
    }
    let invocation = "run the probe inside the workspace";
    match sandbox.wrap(invocation, &policy, &validation, bound()) {
        Ok(wrapped) => {
            if wrapped.command != invocation {
                findings.push(finding(
                    CASE,
                    RULE,
                    format!(
                        "the wrapped invocation text differs from the one requested: {:?}",
                        wrapped.command
                    ),
                ));
            }
            if wrapped.policy != policy.reference {
                findings.push(finding(
                    CASE,
                    RULE,
                    "the wrapped command names a different policy than the one that was validated"
                        .to_owned(),
                ));
            }
            if wrapped.validation != validation {
                findings.push(finding(
                    CASE,
                    RULE,
                    "the wrapped command does not carry the validation that authorized the wrap"
                        .to_owned(),
                ));
            }
        }
        Err(error) => findings.push(finding(
            CASE,
            RULE,
            format!("wrapping under a validated policy failed: {error}"),
        )),
    }
    findings
}

/// Case: a wrap that hands over a validation earned by a DIFFERENT policy
/// is a typed refusal — "validation precedes every use" is structural, so a
/// stale or cross-wired validation cannot authorize a wrap (§17.3's
/// posture; the trait contract).
fn wrap_demands_its_own_validation(factory: &dyn SandboxFactory) -> Vec<ContractFinding> {
    const CASE: &str = "wrap_demands_its_own_validation";
    const RULE: &str = "port contract + spec 17.3 posture: wrap requires the validation its own policy earned; a foreign validation is a typed refusal";
    let Ok(sandbox) = factory.build() else {
        return vec![finding(
            CASE,
            RULE,
            "the factory could not build a sandbox for the case".to_owned(),
        )];
    };
    let one = policy("policy-contained-01");
    let two = policy("policy-contained-02");
    let validation = match sandbox.validate(&one, bound()) {
        Ok(validation) => validation,
        Err(error) => {
            return vec![finding(
                CASE,
                RULE,
                format!("validating the first policy failed: {error}"),
            )];
        }
    };
    match sandbox.wrap(
        "run the probe inside the workspace",
        &two,
        &validation,
        bound(),
    ) {
        Err(_) => Vec::new(),
        Ok(wrapped) => vec![finding(
            CASE,
            RULE,
            format!(
                "a wrap under policy {} was authorized by a validation for {} — the wrap must \
                 be refused",
                wrapped.policy, validation.policy
            ),
        )],
    }
}

/// Runs the full `Sandbox` contract suite against one factory.
pub fn sandbox_suite(factory: &dyn SandboxFactory) -> Vec<ContractFinding> {
    let mut findings = Vec::new();
    findings.extend(validation_precedes_every_use(factory));
    findings.extend(wrap_demands_its_own_validation(factory));
    findings
}
