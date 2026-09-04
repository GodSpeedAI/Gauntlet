//! Shared contract suites (spec §7.3): "Each port has a **shared contract
//! test suite** that every adapter for that port MUST pass. Adding an
//! adapter means implementing the trait and running the existing suite, not
//! writing a bespoke one."
//!
//! Each suite is a plain function parameterized over an adapter FACTORY —
//! a trait that constructs the adapter under test together with the
//! fixtures a case needs. Suites return their findings instead of
//! panicking, so the same function serves three roles:
//!
//! 1. an adapter's own test runs the suite and asserts it is empty;
//! 2. this crate's suite-run tests pin the suites green against reference
//!    in-memory stubs;
//! 3. this crate's teeth tests run deliberately-broken stubs and assert the
//!    suite REPORTS the violated case — "a suite no adapter can fail is not
//!    a suite" (plan Task 4).
//!
//! Findings name the suite, the case, and the rule (with its spec anchor)
//! so a failing adapter sees exactly which contract it broke.

use core::fmt;

/// One contract violation a suite found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractFinding {
    /// The suite that found it, e.g. `agent_runner`.
    pub suite: &'static str,
    /// The named case that found it, e.g.
    /// `admission_returns_before_completion`.
    pub case: &'static str,
    /// The rule that was violated, with its spec anchor.
    pub rule: &'static str,
    /// What was observed, for diagnosis.
    pub detail: String,
}

impl fmt::Display for ContractFinding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{}::{}] {} — {}",
            self.suite, self.case, self.rule, self.detail
        )
    }
}

impl ContractFinding {
    /// Whether this finding is from the named case.
    pub fn is_case(&self, case: &'static str) -> bool {
        self.case == case
    }

    /// Builds a suite finding for a case.
    pub fn new(
        suite: &'static str,
        case: &'static str,
        rule: &'static str,
        detail: String,
    ) -> Self {
        Self {
            suite,
            case,
            rule,
            detail,
        }
    }
}

/// A factory could not build the adapter under test with the fixtures a
/// case needs. This is a suite-harness failure, not a contract finding:
/// the case could not run at all, which adapters must fix before anything
/// else can be said.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuiteSetupError {
    pub case: &'static str,
    pub detail: String,
}

impl fmt::Display for SuiteSetupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "setup for case {} failed: {}", self.case, self.detail)
    }
}

impl std::error::Error for SuiteSetupError {}

pub mod agent_runner;
pub mod budget_meter;
pub mod capability_ledger;
pub mod clock;
pub mod comparator;
pub mod event_log;
pub mod evidence_store;
pub mod human_escalation;
pub mod id_generator;
pub mod observer;
pub mod operator_surface;
pub mod policy_provider;
pub mod reference_provider;
pub mod revision_ledger;
pub mod sandbox;
pub mod state_store;
pub mod tool_provider;
pub mod verifier;
pub mod workspace_provider;

#[cfg(test)]
pub(crate) mod stubs;

#[cfg(test)]
mod suite_runs;
