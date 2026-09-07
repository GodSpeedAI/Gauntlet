//! Evidence locator `tests/contract/docs_present` (AK-068, spec §25).
//!
//! Declared as a cargo test target of `gauntlet-arch-tests` so the frozen
//! answer key's locator resolves to a real, runnable target with the name
//! it names (the AK-001/AK-049-precedent class: locator materialized as a
//! named target at the repo-root `tests/contract/` directory). The
//! checking logic lives in the crate (`gauntlet_arch_tests::docs`); this
//! file is the address.
//!
//! Scope (task-27 charter slice B2): structure only — presence, the
//! status section in each file, handoff pointer integrity, CONFORMANCE.md's
//! binding content, and the six-word §26 status vocabulary on unit-level
//! claims. The conformance.json cross-check leg is deliberately NOT here;
//! it belongs to the wave-2 consistency test (charter slice B3).

use gauntlet_arch_tests as arch;

/// AK-068 leg (a) — all eight §25 files exist under `docs/`, non-empty.
/// Deleting any one of them makes this RED.
#[test]
fn docs_set_present() {
    let violations = arch::docs::check_docs_files_present(&arch::repo_root());
    assert!(
        violations.is_empty(),
        "{}",
        arch::summarize("docs-set-present", &violations)
    );
}

/// AK-068 leg (b) — each of the eight carries the `## Proven vs
/// provisional` section: no document states a capability without stating
/// whether it is proven or provisional (spec §26). Stripping the section
/// from any one of them makes this RED.
#[test]
fn status_section_in_every_doc() {
    let violations = arch::docs::check_status_section_present(&arch::repo_root());
    assert!(
        violations.is_empty(),
        "{}",
        arch::summarize("status-section", &violations)
    );
}

/// AK-068 leg (c) — `docs/HANDOFF.md` is a thin pointer: it names the
/// canonical handoff locator (`.agents/reports/handoff.md`, AK-067's
/// frozen locator) and all six §26 categories, so per-unit truth stays
/// single-sourced and the pointer cannot drift into a second handoff.
#[test]
fn handoff_pointer_names_canonical_locator() {
    let violations = arch::docs::check_handoff_pointer(&arch::repo_root());
    assert!(
        violations.is_empty(),
        "{}",
        arch::summarize("handoff-pointer", &violations)
    );
}

/// AK-068 leg (d) — `docs/CONFORMANCE.md` carries its binding content
/// list (task-27 charter decision 8): the trust-posture, sandbox/tier,
/// profile-adjudication, and advisory-only-enforcement sections, and the
/// UNK-001 terminal disposition under its stable phrase.
#[test]
fn conformance_binding_content_present() {
    let violations = arch::docs::check_conformance_binding_content(&arch::repo_root());
    assert!(
        violations.is_empty(),
        "{}",
        arch::summarize("conformance-content", &violations)
    );
}

/// AK-068 leg (e) — unit-level status claims use only the six §26 status
/// words. Any other status token on a line that names a ruler unit
/// (`AK-NNN`) fails: the wave-2 cross-check parses exactly this shape, so
/// a claim like `proven — AK-060` or a mixed spelling beside an id would
/// be invisible to it.
#[test]
fn status_claims_use_canonical_vocabulary() {
    let violations = arch::docs::check_status_claim_vocabulary(&arch::repo_root());
    assert!(
        violations.is_empty(),
        "{}",
        arch::summarize("status-vocabulary", &violations)
    );
}
