//! The shared `AgentRunner` contract suite (spec §7.3) run against this
//! adapter — "Adding an adapter means implementing the trait and running
//! the existing suite, not writing a bespoke one." The suite lives,
//! UNMODIFIED, in `gauntlet-ports::contract::agent_runner`; this file only
//! supplies the factory and asserts the findings are empty.
//!
//! AK-059's core: "The mock AgentRunner can replay scripted results
//! including malformed output, timeout, and a reference-mutation attempt"
//! — replaying is proven by the fixture playback tests
//! (`tests/scenario_playback.rs`); passing the unmodified Task-4 suite is
//! proven here.

use gauntlet_adapter_agent_mock::MockRunnerFactory;
use gauntlet_ports::contract::agent_runner::{ProbeBudget, agent_runner_suite};

/// The unmodified Task-4 `AgentRunner` contract suite against the mock
/// adapter with the in-repo instant probe budget. Empty findings = pass.
#[test]
fn the_mock_runner_passes_the_unmodified_shared_contract_suite() {
    // AK-059: the mock must satisfy every pinned runner semantic —
    // admission-before-completion, terminal-once stability, the TimedOut /
    // malformed_result / contract_mismatch classifications, idempotent
    // cancellation, and stable typed capabilities.
    let findings = agent_runner_suite(&MockRunnerFactory, ProbeBudget::INSTANT);
    assert!(
        findings.is_empty(),
        "the mock runner must pass the shared suite:\n{}",
        findings
            .iter()
            .map(|finding| finding.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    );
}
