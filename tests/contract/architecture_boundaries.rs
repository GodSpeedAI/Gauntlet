//! Evidence locator `tests/contract/architecture_boundaries` (AK-001) and
//! `tests/contract/architecture_boundaries::denylist` (AK-002).
//!
//! Declared as a cargo test target of `gauntlet-arch-tests` so the frozen
//! answer key's locator resolves to a real, runnable target with the name it
//! names (recritique R13/R16/R28). The checking logic lives in the crate; this
//! file is the address.

use gauntlet_arch_tests as arch;

/// AK-001 — the domain has zero compile-time dependencies on ports,
/// adapters, or infrastructure, over a member set discovered from the
/// filesystem rather than trusted from `[workspace].members`.
#[test]
fn dependency_direction() {
    let violations = arch::check_dependency_direction(&arch::repo_root());
    assert!(
        violations.is_empty(),
        "{}",
        arch::summarize("dependency-direction", &violations)
    );
}

/// AK-001 — what gets scanned must equal what compiles: no symlinks, no
/// unreadable/oversized/non-UTF-8 files, no `#[path]`/`include!` target that
/// escapes its crate directory.
#[test]
fn scope_integrity() {
    let violations = arch::check_scope_integrity(&arch::repo_root());
    assert!(
        violations.is_empty(),
        "{}",
        arch::summarize("scope-integrity", &violations)
    );
}

/// AK-002 — no provider, harness, database, browser, sandbox, or adapter
/// identifier appears in domain or application source.
#[test]
fn denylist() {
    let violations = arch::check_denylist(&arch::repo_root());
    assert!(
        violations.is_empty(),
        "{}",
        arch::summarize("denied-identifier", &violations)
    );
}
