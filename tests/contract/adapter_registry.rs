//! Evidence locator `tests/contract/adapter_registry::selection_is_config_driven`
//! (AK-014).
//!
//! The registry itself lands with plan Task 20. What is checkable today — and
//! what AK-014's second clause states — is that no adapter is chosen by a
//! literal in domain or application code. The adapter identifiers are derived
//! from the workspace members, so this check widens automatically as adapters
//! are added.

use gauntlet_arch_tests as arch;

/// AK-014 — adapter selection resolves from configuration only.
#[test]
fn selection_is_config_driven() {
    let violations = arch::check_adapter_selection(&arch::repo_root());
    assert!(
        violations.is_empty(),
        "{}",
        arch::summarize("adapter-selection", &violations)
    );
}
