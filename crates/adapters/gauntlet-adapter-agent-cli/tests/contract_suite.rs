//! The shared `AgentRunner` contract suite (spec §7.3) run against this
//! adapter — "Adding an adapter means implementing the trait and running
//! the existing suite, not writing a bespoke one." The suite lives,
//! UNMODIFIED, in `gauntlet-ports::contract::agent_runner`; this file only
//! supplies the factory and asserts the findings are empty.
//!
//! The process doubles are what the suite stands in for (spec §6.3: "a
//! real adapter maps it to a probe child instructed to behave that way";
//! R-1b: no real builder process runs in the battery): the adapter's REAL
//! logic — template rendering, prompt-file writing, anchored files, the
//! total-order classification, idempotent cancellation — is what the
//! suite exercises, through real files and the real strict contract
//! codec.
//!
//! Plan Task 16 adds the sibling `exit_without_result_suite` (never-write
//! shape 2), which this adapter is one of the two designed satisfiers of
//! (the other being the mock runner): a child that has EXITED without a
//! valid result document is `Failed{missing_result}`, decided before any
//! deadline — never `Completed` (exit status is liveness evidence) and
//! never `TimedOut`.

use gauntlet_adapter_agent_cli::testing::CliRunnerFactory;
use gauntlet_ports::contract::agent_runner::{
    ProbeBudget, agent_runner_suite, exit_without_result_suite,
};

/// The unmodified shared `AgentRunner` contract suite against this
/// adapter with the in-repo instant probe budget. Empty findings = pass.
#[test]
fn the_cli_agent_runner_passes_the_unmodified_shared_contract_suite() {
    // AK-009..AK-012 on the command-template adapter: admission before
    // completion, terminal-once stability, the TimedOut /
    // malformed_result / contract_mismatch classifications, idempotent
    // cancellation, and stable typed capabilities.
    let findings = agent_runner_suite(&CliRunnerFactory, ProbeBudget::INSTANT);
    assert!(
        findings.is_empty(),
        "the cli-agent runner must pass the shared suite:\n{}",
        findings
            .iter()
            .map(|finding| finding.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// The plan-Task-16 shape-2 suite: an exited child with no valid result
/// document is `Failed{missing_result}` on the FIRST probe — never
/// Completed (the contract is the completion fact), never TimedOut (no
/// deadline was reached) — with the document uncollectible and the
/// terminal stable.
#[test]
fn the_cli_agent_runner_passes_the_exit_without_result_suite() {
    let findings = exit_without_result_suite(&CliRunnerFactory);
    assert!(
        findings.is_empty(),
        "the cli-agent runner must pass the exit-without-result suite:\n{}",
        findings
            .iter()
            .map(|finding| finding.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    );
}
