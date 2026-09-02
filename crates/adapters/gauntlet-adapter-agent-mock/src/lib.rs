//! Mock `AgentRunner` adapter (spec §4.1, §6.1, §6.3; plan Task 8): a
//! scripted, deterministic runner that replays success, malformed results,
//! silence, timeouts, contract mismatch, lying builders, and
//! reference-mutation attempts, so loop verification never depends on a
//! live model to be exercisable.
//!
//! Spec §6.3 makes this adapter REQUIRED, not optional: "Without it the
//! orchestration layer cannot be tested without paying a frontier model,
//! and every test becomes flaky and expensive — which reliably ends in the
//! tests being skipped."
//!
//! The same crate carries the Annex E test doubles ([`DeterministicClock`],
//! [`SeededIdGenerator`] with their env-hook selectors) — a recorded design
//! decision: test doubles live together; creating new adapter crates for
//! each would churn the workspace's pinned inventories for no architectural
//! gain (none of them names a vendor or leaks a port boundary).

pub mod deterministic_clock;
pub mod hooks;
pub mod runner;
pub mod scenario;
pub mod seeded_ids;

pub use deterministic_clock::DeterministicClock;
pub use hooks::{CLOCK_TOKEN, EnvHookError, clock_from_env, id_generator_from_env};
pub use runner::{MockAgentRunner, MockRunnerFactory};
pub use scenario::{
    ChildBehavior, Derailment, Scenario, ScenarioError, WorkspaceEffect, load_scenario,
    parse_scenario, scenario_fixtures_dir,
};
pub use seeded_ids::{EmptySeed, SeededIdGenerator};

/// The runner token this adapter's composition fronts (plan Task 20,
/// charter D2; the same convention as the prime and generic-CLI
/// siblings): `runners.default = mock-agent` selects the scripted
/// runner — explicitly, by operator configuration (the E2E fixture
/// selects it; a production run never defaults to it).
pub const RUNNER_TOKEN: &str = "mock-agent";

/// The test hook that names the scenario the mock token replays (a
/// fixture stem under the scenario fixtures dir). Deliberately NOT in
/// `.env.example`: the AK-019 instrument pins that file to Annex E
/// lockstep, and this is a test-division hook, not operator
/// configuration — the composition refuses a mock selection without it.
pub const MOCK_SCENARIO_VAR: &str = "GAUNTLET_MOCK_SCENARIO";
