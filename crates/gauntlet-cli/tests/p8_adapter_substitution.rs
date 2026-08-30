//! P8 / AK-013 — the adapter-substitution proof (spec §23.1 P8/I12; plan
//! Task 16): "The full conformance suite passes against >=2 `AgentRunner`
//! adapters, one of them not the mock."
//!
//! ONE test body runs the FULL suite — the unmodified shared
//! `agent_runner_suite` PLUS the plan-Task-16 sibling
//! `exit_without_result_suite` (never-write shape 2) — over EVERY
//! selected factory in a loop. There are zero per-adapter test bodies:
//! substituting adapters is data (the factory list), exactly the
//! substitution property P8 exists to prove. The mock runner is
//! `gauntlet-adapter-agent-mock`'s; the non-mock adapter is the generic
//! command-template CLI runner (`gauntlet-adapter-agent-cli`).
//!
//! Adapter selection: the plan's literal `--adapter` flag has no
//! precedent in this repository's nextest usage (no test in the repo
//! parses CLI arguments; the recorded deviation is D10 in the task's
//! report), so selection is the environment variable
//! `GAUNTLET_P8_ADAPTER`:
//!
//! - unset or `both` (the default): run BOTH factories — the gate line
//!   `cargo nextest run --test p8_adapter_substitution` works unchanged;
//! - `mock` / `cli`: run that single factory (debugging convenience;
//!   the two-factory assertion below only binds the default run).
//!
//! Any other value FAILS the test — an unknown selection is a defect,
//! never a silently narrowed battery. The factories' own command
//! templates are constructor-injected literals (RES-D10): this test never
//! reads the host's `GAUNTLET_CLI_AGENT_CMD`, so the battery is not
//! environment-sensitive beyond the explicit selection variable.

use gauntlet_adapter_agent_cli::testing::CliRunnerFactory;
use gauntlet_adapter_agent_mock::MockRunnerFactory;
use gauntlet_ports::contract::agent_runner::{
    AgentRunnerFactory, ProbeBudget, agent_runner_suite, exit_without_result_suite,
};

const MOCK_LABEL: &str = "mock-agent-runner";
const CLI_LABEL: &str = "cli-agent-adapter";

/// The selection variable: `mock | cli | both` (unset = `both`).
const SELECTION_VAR: &str = "GAUNTLET_P8_ADAPTER";

/// The selected factories, resolved per call (the factories are unit
/// structs — resolution is free). An unknown value is a failure, not a
/// silently narrowed battery.
fn selected_factories() -> Vec<(&'static str, &'static dyn AgentRunnerFactory)> {
    match std::env::var(SELECTION_VAR).as_deref() {
        Ok("") | Ok("both") | Err(_) => {
            vec![
                (MOCK_LABEL, &MockRunnerFactory),
                (CLI_LABEL, &CliRunnerFactory),
            ]
        }
        Ok("mock") => vec![(MOCK_LABEL, &MockRunnerFactory)],
        Ok("cli") => vec![(CLI_LABEL, &CliRunnerFactory)],
        Ok(other) => panic!(
            "{SELECTION_VAR}={other:?} is unknown — expected mock | cli | both; an unknown \
             selection must fail loudly, never narrow the battery silently"
        ),
    }
}

/// The proof: the SAME loop body runs every selected adapter through the
/// FULL conformance surface. Empty findings from both = P8 holds.
#[test]
fn the_full_conformance_suite_passes_against_every_selected_adapter_unchanged() {
    let selected = selected_factories();
    let mut report = String::new();
    for (label, factory) in &selected {
        let shared = agent_runner_suite(*factory, ProbeBudget::INSTANT);
        let shape_two = exit_without_result_suite(*factory);
        report.push_str(&format!(
            "adapter {label}: shared-suite findings = {}, shape-2 findings = {}\n",
            shared.len(),
            shape_two.len()
        ));
        assert!(
            shared.is_empty(),
            "adapter {label} must pass the shared agent_runner suite:\n{}",
            shared
                .iter()
                .map(|finding| finding.to_string())
                .collect::<Vec<_>>()
                .join("\n")
        );
        assert!(
            shape_two.is_empty(),
            "adapter {label} must pass the exit-without-result suite:\n{}",
            shape_two
                .iter()
                .map(|finding| finding.to_string())
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
    // The default (gate-line) run IS the substitution proof: at least
    // two adapters, one of them NOT the mock, through identical code.
    if std::env::var(SELECTION_VAR).as_deref() != Ok("mock")
        && std::env::var(SELECTION_VAR).as_deref() != Ok("cli")
    {
        assert!(
            selected.len() >= 2,
            "the default battery selects both adapters, got {}",
            selected.len()
        );
        assert!(
            selected.iter().any(|(label, _)| *label == CLI_LABEL),
            "one adapter must not be the mock: {report}"
        );
        assert!(
            selected.iter().any(|(label, _)| *label == MOCK_LABEL),
            "the mock anchors the comparison: {report}"
        );
    }
}
