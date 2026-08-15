//! Evidence locator `tests/contract/no_hardcoded_literals` (AK-016).

use gauntlet_arch_tests as arch;

/// AK-016 — no path, model, provider, command, timeout, tier policy, or tool
/// identity is a hardcoded literal outside the declared defaults table.
#[test]
fn core_crates_contain_no_hardcoded_literals() {
    let violations = arch::check_no_hardcoded_literals(&arch::repo_root());
    assert!(
        violations.is_empty(),
        "{}",
        arch::summarize("hardcoded-literal", &violations)
    );
}

/// AK-016's "declared defaults table" must be a verified path, not a
/// self-declared one: the exemption is warranted or it is inactive.
#[test]
fn defaults_exemption_is_warranted_or_inactive() {
    let violations = arch::check_defaults_exemption(&arch::repo_root());
    assert!(
        violations.is_empty(),
        "{}",
        arch::summarize("vacuous-exemption", &violations)
    );
}
