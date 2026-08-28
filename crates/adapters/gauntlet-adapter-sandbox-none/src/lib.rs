//! The no-backend `Sandbox` adapter (spec §6.2, §8.6, §17.3; plan Task 14):
//! the §8.6 default `backend: none`. An identity wrap is legal ONLY at the
//! `contained` tier — "backend `none`, tier `contained` only; any higher
//! tier without a backend refuses to dispatch" (spec §8.6) — so this
//! backend refuses every policy above contained, refuses an allowlist
//! posture it cannot enforce, refuses a policy naming another backend
//! (cross-wiring defense), and reports `isolation: none` honestly (spec
//! §22.2: a harness that is not itself wrapped by a sandbox reports none).
//!
//! There is no fallback anywhere in this adapter: a policy this backend
//! cannot serve is a validation failure that escalates (spec §21.3),
//! never a quiet unsandboxed wrap.

use gauntlet_domain::BlastRadius;
use gauntlet_ports::agent_runner::Isolation;
use gauntlet_ports::contract::sandbox::SandboxFactory;
use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::sandbox::{
    NetworkPosture, PolicyValidation, Sandbox, SandboxBackendRef, SandboxError, SandboxPolicy,
    SandboxedCommand,
};

/// The §8.6 default backend token. The shape rules of
/// [`SandboxBackendRef::new`] are pure syntax over this constant
/// (non-empty, short, not path-rooted, whitespace-free) — pinned by test
/// below, so the one `expect` in [`Sandbox::backend_identity`] guards an
/// established invariant, not a guess.
const BACKEND_TOKEN: &str = "none";

/// The no-backend sandbox: validation is policy arithmetic over spec
/// §17.3's tier posture; wrapping is the identity wrap.
#[derive(Debug, Clone, Default)]
pub struct NoneSandbox;

impl NoneSandbox {
    pub fn new() -> Self {
        Self
    }

    /// Every rule this backend refuses a policy on, as findings (spec
    /// §6.2: a validation with findings is a failure — all applicable
    /// findings are collected so the escalation names every offense, not
    /// just the first).
    fn policy_findings(policy: &SandboxPolicy) -> Vec<String> {
        let mut findings = Vec::new();
        if policy.tier != BlastRadius::Contained {
            findings.push(format!(
                "backend none is permitted only at the contained tier (spec 17.3) — this policy \
                 names the {} tier",
                policy.tier
            ));
        }
        if let NetworkPosture::Allowlist { hosts } = &policy.network {
            findings.push(format!(
                "backend none cannot enforce a host allowlist ({} host(s) named) — a posture \
                 this backend cannot enforce is a finding, never a silent downgrade",
                hosts.len()
            ));
        }
        if policy.backend.as_str() != BACKEND_TOKEN {
            findings.push(format!(
                "the policy names backend {}; this backend is '{BACKEND_TOKEN}' — a policy \
                 naming another backend must not be validated here",
                policy.backend
            ));
        }
        if policy.writable_roots.is_empty() {
            findings.push(
                "writable_roots is empty — a policy must state where writes are permitted"
                    .to_owned(),
            );
        }
        findings
    }
}

impl Sandbox for NoneSandbox {
    fn validate(
        &self,
        policy: &SandboxPolicy,
        _deadline: Deadline,
    ) -> Result<PolicyValidation, SandboxError> {
        let findings = Self::policy_findings(policy);
        if !findings.is_empty() {
            return Err(SandboxError::ValidationFailed {
                policy: policy.reference.clone(),
                findings,
            });
        }
        Ok(PolicyValidation {
            policy: policy.reference.clone(),
            backend: policy.backend.clone(),
            isolation: Isolation::None,
            findings: Vec::new(),
        })
    }

    fn wrap(
        &self,
        command: &str,
        policy: &SandboxPolicy,
        validation: &PolicyValidation,
        _deadline: Deadline,
    ) -> Result<SandboxedCommand, SandboxError> {
        if validation.policy != policy.reference {
            return Err(SandboxError::WrapFailed {
                detail: format!(
                    "validation {} does not cover policy {} — wrap requires the validation its \
                     own policy earned",
                    validation.policy.as_str(),
                    policy.reference.as_str()
                ),
            });
        }
        if !validation.is_clean() {
            return Err(SandboxError::WrapFailed {
                detail: format!(
                    "validation {} carries {} finding(s) — a failed validation authorizes \
                     nothing (spec 21.3: escalate, never fall back)",
                    validation.policy.as_str(),
                    validation.findings.len()
                ),
            });
        }
        if validation.isolation != Isolation::None {
            return Err(SandboxError::WrapFailed {
                detail: format!(
                    "validation claims the {} rung; this backend achieves none — a foreign \
                     validation claiming more is a cross-wiring",
                    validation.isolation.token()
                ),
            });
        }
        // A hand-forged clean validation must not bootstrap what `validate`
        // would refuse: the wrap re-derives the policy's eligibility here.
        let findings = Self::policy_findings(policy);
        if !findings.is_empty() {
            return Err(SandboxError::WrapFailed {
                detail: format!(
                    "the policy is not eligible at this backend: {}",
                    findings.join("; ")
                ),
            });
        }
        // The identity wrap: legal only at contained (the eligibility
        // re-derivation above is what keeps that "only" true).
        Ok(SandboxedCommand {
            command: command.to_owned(),
            policy: policy.reference.clone(),
            validation: validation.clone(),
        })
    }

    fn backend_identity(&self) -> SandboxBackendRef {
        // The constant's shape is an established invariant (pinned by
        // the identity-token test), so this expect cannot fire.
        SandboxBackendRef::new(BACKEND_TOKEN).expect("the fixed none token is always valid")
    }
}

/// Suite scaffolding for the shared `Sandbox` contract suite (spec §7.3):
/// the no-backend adapter runs the same suite every `Sandbox` adapter
/// must pass.
pub struct NoneSandboxFactory;

impl SandboxFactory for NoneSandboxFactory {
    fn label(&self) -> &'static str {
        "none-backend"
    }

    fn build(&self) -> Result<Box<dyn Sandbox>, String> {
        Ok(Box::new(NoneSandbox::new()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gauntlet_domain::SandboxPolicyRef;
    use gauntlet_ports::contract::sandbox::sandbox_suite;
    use gauntlet_ports::deadline::Deadline;

    /// Suite-bound span and base instant, each on its own line (no
    /// duration-shaped identifier shares a line with a numeric literal).
    const BASE_INSTANT: u64 = 1_700_000_000;
    const BOUND_SPAN: u64 = 60;

    fn bound() -> Deadline {
        Deadline::at_epoch(gauntlet_domain::EpochSeconds::new(
            BASE_INSTANT + BOUND_SPAN,
        ))
    }

    fn contained_policy() -> SandboxPolicy {
        SandboxPolicy {
            reference: SandboxPolicyRef::new("tier-contained-test").expect("test ref"),
            tier: BlastRadius::Contained,
            backend: SandboxBackendRef::new(BACKEND_TOKEN).expect("test backend ref"),
            network: NetworkPosture::DenyByDefault,
            writable_roots: vec!["/suite-writable-root".to_owned()],
            denied_paths: vec!["/suite-denied-path".to_owned()],
        }
    }

    fn policy_for_tier(tier: BlastRadius) -> SandboxPolicy {
        let mut policy = contained_policy();
        policy.tier = tier;
        policy
    }

    fn clean_validation(policy: &SandboxPolicy) -> PolicyValidation {
        NoneSandbox::new()
            .validate(policy, bound())
            .expect("the well-formed contained policy validates clean")
    }

    #[test]
    fn the_identity_token_is_a_valid_backend_ref() {
        // Pins the invariant behind backend_identity's expect.
        assert!(SandboxBackendRef::new(BACKEND_TOKEN).is_ok());
    }

    #[test]
    fn a_well_formed_contained_policy_validates_clean_at_the_none_rung() {
        let sandbox = NoneSandbox::new();
        let policy = contained_policy();
        let validation = sandbox
            .validate(&policy, bound())
            .expect("a well-formed contained policy validates");
        assert!(validation.is_clean());
        assert_eq!(validation.isolation, Isolation::None);
        assert_eq!(validation.policy, policy.reference);
        assert_eq!(validation.backend, policy.backend);
    }

    #[test]
    fn every_tier_above_contained_is_refused_at_validate() {
        let sandbox = NoneSandbox::new();
        for tier in [
            BlastRadius::Local,
            BlastRadius::CrossCutting,
            BlastRadius::Irreversible,
        ] {
            match sandbox.validate(&policy_for_tier(tier), bound()) {
                Err(SandboxError::ValidationFailed { findings, .. }) => {
                    assert!(
                        findings.iter().any(|finding| finding.contains("spec 17.3")),
                        "the refusal names the tier rule for {tier}: {findings:?}"
                    );
                }
                other => panic!("the {tier} tier must be refused, got {other:?}"),
            }
        }
    }

    #[test]
    fn an_allowlist_posture_is_an_honest_finding_not_a_downgrade() {
        let sandbox = NoneSandbox::new();
        let mut policy = contained_policy();
        policy.network = NetworkPosture::Allowlist {
            hosts: vec!["example.test".to_owned()],
        };
        match sandbox.validate(&policy, bound()) {
            Err(SandboxError::ValidationFailed { findings, .. }) => {
                assert!(
                    findings
                        .iter()
                        .any(|finding| finding.contains("cannot enforce a host allowlist")),
                    "the refusal names the unenforceable posture: {findings:?}"
                );
            }
            other => panic!("an allowlist must be refused, got {other:?}"),
        }
    }

    #[test]
    fn a_policy_naming_another_backend_is_refused() {
        let sandbox = NoneSandbox::new();
        let mut policy = contained_policy();
        policy.backend = SandboxBackendRef::new("some-other-backend").expect("test backend ref");
        match sandbox.validate(&policy, bound()) {
            Err(SandboxError::ValidationFailed { findings, .. }) => {
                assert!(
                    findings
                        .iter()
                        .any(|finding| finding.contains("another backend")),
                    "the refusal names the cross-wiring: {findings:?}"
                );
            }
            other => panic!("a foreign-backend policy must be refused, got {other:?}"),
        }
    }

    #[test]
    fn empty_writable_roots_is_refused() {
        let sandbox = NoneSandbox::new();
        let mut policy = contained_policy();
        policy.writable_roots = Vec::new();
        match sandbox.validate(&policy, bound()) {
            Err(SandboxError::ValidationFailed { findings, .. }) => {
                assert!(
                    findings
                        .iter()
                        .any(|finding| finding.contains("writable_roots is empty")),
                    "the refusal names the empty roots: {findings:?}"
                );
            }
            other => panic!("empty writable roots must be refused, got {other:?}"),
        }
    }

    #[test]
    fn the_identity_wrap_carries_the_command_policy_and_validation() {
        let sandbox = NoneSandbox::new();
        let policy = contained_policy();
        let validation = clean_validation(&policy);
        let wrapped = sandbox
            .wrap("run the probe", &policy, &validation, bound())
            .expect("a clean validation of an eligible policy wraps");
        assert_eq!(wrapped.command, "run the probe");
        assert_eq!(wrapped.policy, policy.reference);
        assert_eq!(wrapped.validation, validation);
    }

    #[test]
    fn wrap_refuses_a_foreign_validation() {
        let sandbox = NoneSandbox::new();
        let one = contained_policy();
        let mut two = contained_policy();
        two.reference = SandboxPolicyRef::new("tier-contained-other").expect("test ref");
        let validation = clean_validation(&one);
        match sandbox.wrap("run the probe", &two, &validation, bound()) {
            Err(SandboxError::WrapFailed { detail }) => {
                assert!(
                    detail.contains("does not cover policy"),
                    "the refusal names the cross-wired validation: {detail}"
                );
            }
            other => panic!("a foreign validation must be refused, got {other:?}"),
        }
    }

    #[test]
    fn wrap_refuses_a_findings_carrying_validation() {
        let sandbox = NoneSandbox::new();
        let policy = contained_policy();
        let mut dirty = clean_validation(&policy);
        dirty.findings.push("forged finding".to_owned());
        match sandbox.wrap("run the probe", &policy, &dirty, bound()) {
            Err(SandboxError::WrapFailed { detail }) => {
                assert!(
                    detail.contains("authorizes nothing"),
                    "the refusal names the escalation rule: {detail}"
                );
            }
            other => panic!("a findings-carrying validation must be refused, got {other:?}"),
        }
    }

    #[test]
    fn wrap_refuses_a_validation_claiming_a_higher_rung() {
        let sandbox = NoneSandbox::new();
        let policy = contained_policy();
        let mut foreign = clean_validation(&policy);
        foreign.isolation = Isolation::Process;
        match sandbox.wrap("run the probe", &policy, &foreign, bound()) {
            Err(SandboxError::WrapFailed { detail }) => {
                assert!(
                    detail.contains("cross-wiring"),
                    "the refusal names the cross-wiring: {detail}"
                );
            }
            other => panic!("a higher-rung validation must be refused, got {other:?}"),
        }
    }

    #[test]
    fn wrap_rederives_eligibility_a_forged_clean_validation_authorizes_nothing() {
        // A forged CLEAN validation for a local-tier policy must not
        // bootstrap an identity wrap above contained.
        let sandbox = NoneSandbox::new();
        let policy = policy_for_tier(BlastRadius::Local);
        let forged = PolicyValidation {
            policy: policy.reference.clone(),
            backend: policy.backend.clone(),
            isolation: Isolation::None,
            findings: Vec::new(),
        };
        match sandbox.wrap("run the probe", &policy, &forged, bound()) {
            Err(SandboxError::WrapFailed { detail }) => {
                assert!(
                    detail.contains("not eligible"),
                    "the refusal names the re-derived eligibility: {detail}"
                );
            }
            other => panic!("a forged clean validation must be refused, got {other:?}"),
        }
    }

    #[test]
    fn the_shared_contract_suite_is_clean() {
        let findings = sandbox_suite(&NoneSandboxFactory);
        assert!(
            findings.is_empty(),
            "the none backend must run the shared sandbox suite clean:\n{}",
            findings
                .iter()
                .map(|finding| finding.to_string())
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
}
