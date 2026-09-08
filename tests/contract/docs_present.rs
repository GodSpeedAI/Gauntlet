//! Evidence locator `tests/contract/docs_present` (AK-068, spec §25).
//!
//! Declared as a cargo test target of `gauntlet-arch-tests` so the frozen
//! answer key's locator resolves to a real, runnable target with the name
//! it names (the AK-001/AK-049-precedent class: locator materialized as a
//! named target at the repo-root `tests/contract/` directory). The
//! checking logic lives in the crate (`gauntlet_arch_tests::docs`); this
//! file is the address.
//!
//! Scope (task-27 charter slices B2 + B3): legs (a)–(e) are B2's
//! structure checks — presence, the status section in each file, handoff
//! pointer integrity, CONFORMANCE.md's binding content, and the six-word
//! §26 status vocabulary on unit-level claims. Legs (f)–(g) are B3's
//! wave-2 consistency checks: the canonical handoff's category sections
//! equal conformance.json's grouping (counts included), and no doc claim
//! line runs ahead of the conformance report. The consistency logic
//! lives in `gauntlet_arch_tests::docs_consistency`; both its legs read
//! the committed locator by default and honor the fixture env overrides
//! (`GAUNTLET_CONFORMANCE_JSON_FIXTURE`, `GAUNTLET_HANDOFF_FIXTURE`)
//! documented there, which is how the RED teeth run against tampered
//! COPIES of the real artifacts without mutating them.

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

/// AK-068 leg (f) — wave-2 (B3): the canonical handoff's six §26
/// sections group EXACTLY the units `.agents/reports/conformance.json`
/// groups, counts included, every unit exactly once. Charter tooth T4:
/// moving a unit between sections in the handoff makes this RED.
#[test]
fn handoff_matches_conformance_grouping() {
    let violations = arch::docs_consistency::check_handoff_matches_conformance(&arch::repo_root());
    assert!(
        violations.is_empty(),
        "{}",
        arch::summarize("handoff-consistency", &violations)
    );
}

/// AK-068 leg (g) — wave-2 (B3, hardened by COR-B3/falsifier F8):
/// every status claim in the eight docs stays at or behind the
/// conformance report's adjudication — `implemented_and_proven` only
/// where the json says proven, `implemented_but_provisional` where it
/// says proven or provisional, the other four categories only on exact
/// agreement. Claims bind through the canonical
/// `- <status_word> — AK-NNN:` line shape AND a case-insensitive
/// ±1-line window fallback (any casing, separator variants, split
/// lines — the laundering shapes are claims too).
#[test]
fn docs_status_claims_never_ahead_of_conformance() {
    let violations =
        arch::docs_consistency::check_docs_claims_not_ahead_of_conformance(&arch::repo_root());
    assert!(
        violations.is_empty(),
        "{}",
        arch::summarize("docs-claims", &violations)
    );
}
