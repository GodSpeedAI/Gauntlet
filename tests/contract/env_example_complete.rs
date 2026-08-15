//! Evidence locator `tests/contract/env_example_complete` (AK-019).

use gauntlet_arch_tests as arch;

/// AK-019 — a `.env.example` exists listing every recognized `GAUNTLET_`
/// variable with empty values and no credentials. Annex E of the spec is the
/// source of truth; this test diffs the file against it.
#[test]
fn env_example_matches_annex_e() {
    let violations = arch::check_env_example(&arch::repo_root());
    assert!(
        violations.is_empty(),
        "{}",
        arch::summarize("env-example", &violations)
    );
}
