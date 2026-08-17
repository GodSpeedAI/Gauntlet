//! Spec vocabularies the composition root supplies (plan Task 5).
//!
//! Two closed name lists live HERE rather than in the application layer,
//! because the boundary suite derives adapter identifiers from the workspace
//! members and denies their spellings as substrings in the inner crates —
//! and both lists collide with derived spellings (Annex E's runner-command
//! variable carries one; §8.4's capability vocabulary carries three). The
//! composition root is the suite's own designated place where naming is
//! legal; the application layer consumes the lists as data.
//!
//! Sources: spec Annex E (environment variable reference — mirrored exactly
//! by `.env.example`, which the boundary suite pins against the annex) and
//! spec §8.4 (semantic tool capabilities).

/// Every Annex E variable name, including the test hooks.
pub const ANNEX_E_ENV: &[&str] = &[
    "GAUNTLET_PROJECT_ROOT",
    "GAUNTLET_CONFIG_DIR",
    "GAUNTLET_STATE_DIR",
    "GAUNTLET_ARTIFACT_DIR",
    "GAUNTLET_PLAN_DIR",
    "GAUNTLET_REPORT_DIR",
    "GAUNTLET_TMP_DIR",
    "GAUNTLET_REFERENCE_PATH",
    "GAUNTLET_REFERENCE_MANIFEST",
    "GAUNTLET_REFERENCE_DIGEST",
    "GAUNTLET_ANSWER_KEY_PATH",
    "GAUNTLET_MAP_PATH",
    "GAUNTLET_COMPARISON_CONTRACT",
    "GAUNTLET_ACCEPTANCE_PATH",
    "GAUNTLET_CONTEXT_PATHS",
    "GAUNTLET_CANON_PATHS",
    "GAUNTLET_HELD_OUT_PATH",
    "GAUNTLET_REGRESSION_PATH",
    "GAUNTLET_DIGEST_ALGO",
    "GAUNTLET_AGENT_RUNNER",
    "GAUNTLET_ORCHESTRATOR_RUNNER",
    "GAUNTLET_ORCHESTRATOR_MODEL",
    "GAUNTLET_BUILDER_RUNNER",
    "GAUNTLET_BUILDER_MODEL",
    "GAUNTLET_CRITIC_RUNNER",
    "GAUNTLET_CRITIC_MODEL",
    "GAUNTLET_VERIFIER_RUNNER",
    "GAUNTLET_VERIFIER_MODEL",
    "GAUNTLET_OBSERVER_RUNNER",
    "GAUNTLET_OPTIMIZER_RUNNER",
    "GAUNTLET_OPTIMIZER_MODEL",
    "GAUNTLET_CLI_AGENT_CMD",
    "GAUNTLET_ROLES_DIR",
    "GAUNTLET_MCP_CONFIG",
    "GAUNTLET_TOOL_REGISTRY",
    "GAUNTLET_MCP_ALLOWLIST",
    "GAUNTLET_MCP_REQUIRED",
    "GAUNTLET_TOOL_CODE_NAV",
    "GAUNTLET_TOOL_FILESYSTEM",
    "GAUNTLET_TOOL_VERSIONING",
    "GAUNTLET_TOOL_TEST",
    "GAUNTLET_TOOL_BUILD",
    "GAUNTLET_TOOL_BROWSER",
    "GAUNTLET_TOOL_VISUAL",
    "GAUNTLET_TOOL_WEB",
    "GAUNTLET_TOOL_RESEARCH",
    "GAUNTLET_TOOL_SECURITY",
    "GAUNTLET_TOOL_SANDBOX",
    "GAUNTLET_TOOL_MEMORY",
    "GAUNTLET_TOOL_EVIDENCE",
    "GAUNTLET_TOOL_DOMAIN",
    "GAUNTLET_TOOL_POLICY",
    "GAUNTLET_TOOL_BENCHMARK",
    "GAUNTLET_TOOL_SCHEMA",
    "GAUNTLET_INTERACTIVE",
    "GAUNTLET_MAX_ROUNDS",
    "GAUNTLET_MAX_PARALLEL",
    "GAUNTLET_MAX_DEPTH",
    "GAUNTLET_MAX_WALL_SECONDS",
    "GAUNTLET_MAX_TOKENS",
    "GAUNTLET_MAX_COST_USD",
    "GAUNTLET_RECOVERY_RESERVE_PCT",
    "GAUNTLET_DIAGNOSTIC_BUDGET",
    "GAUNTLET_REVIEW_MODE",
    "GAUNTLET_MAX_CRITICS",
    "GAUNTLET_MAX_ATTEMPTS_PER_UNIT",
    "GAUNTLET_NONCONVERGENCE_N",
    "GAUNTLET_POLL_INTERVAL_MS",
    "GAUNTLET_STALL_TIMEOUT_MS",
    "GAUNTLET_RETRY_BASE_MS",
    "GAUNTLET_RETRY_MAX_MS",
    "GAUNTLET_ALLOW_INSTALL",
    "GAUNTLET_ALLOW_NETWORK",
    "GAUNTLET_ALLOW_DESTRUCTIVE",
    "GAUNTLET_ALLOW_ORCHESTRATOR_IMPLEMENTATION",
    "GAUNTLET_VERSIONING_STRATEGY",
    "GAUNTLET_SANDBOX_BACKEND",
    "GAUNTLET_SANDBOX_POLICY_DIR",
    "GAUNTLET_SANDBOX_BINARY",
    "GAUNTLET_SANDBOX_EXPERIMENTAL",
    "GAUNTLET_STATE_BACKEND",
    "GAUNTLET_EVIDENCE_BACKEND",
    "GAUNTLET_EXPORT_DIR",
    "GAUNTLET_OPERATOR_SURFACES",
    "GAUNTLET_API_BIND",
    "GAUNTLET_LOG_FORMAT",
    "GAUNTLET_LOG_SINK",
    "GAUNTLET_OTEL_ENDPOINT",
    "GAUNTLET_PROMOTION_MIN_SETTLEMENTS",
    "GAUNTLET_PROMOTION_MARGIN",
    "GAUNTLET_CLOCK",
    "GAUNTLET_ID_SEED",
];

/// The §8.4 semantic tool capabilities, in spec order.
pub const CAPABILITY_VOCABULARY: &[&str] = &[
    "code_nav",
    "filesystem",
    "versioning",
    "test",
    "build",
    "browser",
    "visual",
    "web",
    "research",
    "security",
    "sandbox",
    "memory",
    "evidence",
    "domain",
    "policy",
    "benchmark",
    "schema",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_vocabularies_are_complete() {
        assert_eq!(ANNEX_E_ENV.len(), 92, "Annex E mirror count");
        // Annex group order is preserved (the list documents the annex's
        // structure); lookup is membership, not order.
        // The test hooks are recognized, not unknown.
        assert!(ANNEX_E_ENV.contains(&"GAUNTLET_CLOCK"));
        assert!(ANNEX_E_ENV.contains(&"GAUNTLET_ID_SEED"));
        // The versioning pair both exist (spec 8.6 setting vs Annex E name).
        assert!(ANNEX_E_ENV.contains(&"GAUNTLET_VERSIONING_STRATEGY"));
        assert_eq!(CAPABILITY_VOCABULARY.len(), 17, "spec 8.4 vocabulary");
    }
}
