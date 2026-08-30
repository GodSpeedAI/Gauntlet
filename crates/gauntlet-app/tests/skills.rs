//! Integration test for the wayfinder-gauntlet skill's conformance to spec
//! §9.2 (plan Task 18; AK-064). The AK-064 locator names
//! `tests/contract/skills::wayfinder_gauntlet_conformance`; this file
//! delivers that test as `skills::wayfinder_gauntlet_conformance` — the
//! directory divergence is a recorded locator-class matter for closure,
//! same class as task17's `roles.rs`, never a reason to touch the ruler.
//!
//! Loads the fixtures a parallel content-only slice produced under
//! `tests/fixtures/formation/` (workspace-root-relative, per the shared
//! Task 18 design contract in `.agents/reports/task18/IN-FLIGHT.md`) and
//! checks: the fixture answer key validates cleanly into exactly AK-101 and
//! AK-102; the clean map/key pair satisfies every §9.2 formation invariant;
//! the skill document exists and preserves its four semantics markers
//! (destination-first, fog-of-war, out-of-scope-never-graduates,
//! HITL-never-self-answered); its provenance record exists and is
//! non-empty.

use std::fs;
use std::path::PathBuf;

use gauntlet_app::formation::check_reference_invariants;
use gauntlet_app::reference::validate::validate_answer_key;

/// The repository root, derived from `CARGO_MANIFEST_DIR` the same way
/// `reference::parse`'s frozen-key test and `roles.rs` do: this crate sits
/// two levels below the workspace root.
fn repo_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .ancestors()
        .nth(2)
        .expect("the app crate sits two levels under the repository root")
        .to_path_buf()
}

fn fixture(name: &str) -> String {
    let path = repo_root()
        .join("tests")
        .join("fixtures")
        .join("formation")
        .join(name);
    fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("fixture {} reads: {err}", path.display()))
}

#[test]
fn wayfinder_gauntlet_conformance() {
    // (a) the fixture answer key validates cleanly and yields exactly the
    // expected Units.
    let answer_key_text = fixture("answer-key.md");
    let units = validate_answer_key(&answer_key_text).expect("the fixture answer key validates");
    let ids: Vec<&str> = units.iter().map(|unit| unit.id().as_str()).collect();
    assert_eq!(
        ids,
        vec!["AK-101", "AK-102"],
        "the fixture answer key yields exactly AK-101 and AK-102"
    );

    // (b) the clean map/key pair satisfies every §9.2 formation invariant.
    let map_text = fixture("map.md");
    check_reference_invariants(&map_text, &answer_key_text)
        .expect("the clean map/key pair satisfies every formation invariant");

    // (c) the skill document exists and preserves its four semantics
    // markers, case-insensitive substring check (a light content lint, no
    // NLP).
    let skill_path = repo_root()
        .join(".agents")
        .join("skills")
        .join("wayfinder-gauntlet")
        .join("SKILL.md");
    let skill_text = fs::read_to_string(&skill_path)
        .unwrap_or_else(|err| panic!("{} reads: {err}", skill_path.display()));
    let lowered = skill_text.to_lowercase();
    for marker in ["destination", "fog", "out of scope", "hitl"] {
        assert!(
            lowered.contains(marker),
            "SKILL.md must preserve the {marker:?} semantics marker"
        );
    }

    // (d) the provenance record exists and is non-empty.
    let provenance_path = repo_root()
        .join(".agents")
        .join("skills")
        .join("wayfinder-gauntlet")
        .join("PROVENANCE.md");
    let provenance_text = fs::read_to_string(&provenance_path)
        .unwrap_or_else(|err| panic!("{} reads: {err}", provenance_path.display()));
    assert!(
        !provenance_text.trim().is_empty(),
        "PROVENANCE.md must be non-empty"
    );
}
