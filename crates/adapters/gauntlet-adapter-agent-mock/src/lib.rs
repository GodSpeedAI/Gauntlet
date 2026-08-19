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
