//! Mock `AgentRunner` adapter (spec §4.1, §6.1; plan Task 8): a scripted,
//! deterministic runner that replays success, malformed results, silence,
//! timeouts, contract mismatch, lying builders, and reference-mutation
//! attempts, so loop verification never depends on a live model to be
//! exercisable.
