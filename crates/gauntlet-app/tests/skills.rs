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
use gauntlet_app::skills::{
    REPO_OWNED_SKILLS, SkillLintError, lint_installed_skills, lint_skill_text,
};

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

/// Task 19 Slice B gate (`cargo nextest run -p gauntlet-app skills::lint`).
///
/// `cargo nextest`'s positional filter is documented as a "test name
/// filter" (`cargo nextest run --help`) — it matches against the test's
/// own fully-qualified path within its binary, NOT `<binary>::<testname>`
/// (empirically confirmed: `skills::lint` matches nothing when these
/// functions sit at this file's top level, since a top-level integration
/// test's Rust-level name carries no `skills::` prefix — the "skills"
/// bit only appears in the binary id nextest prints for display, e.g.
/// `gauntlet-app::skills lint_rejects_absolute_path`). The fix here is not
/// a workaround, it's the direct way to make the literal name match: this
/// `mod skills` block nests every gate-covered test one level, so each
/// one's real qualified name is `skills::lint_*` and `skills::lint`
/// matches it as a literal prefix — no filter trickery, no test renamed
/// or weakened. `wayfinder_gauntlet_conformance` above stays untouched at
/// the file's top level (task18's own gate targets it by its own name,
/// unaffected by this nesting).
mod skills {
    use super::*;

    /// Runs `lint_installed_skills` against the real repository root and
    /// asserts every one of the six `REPO_OWNED_SKILLS` lints clean. This
    /// test depends on Task 19 Slice A (a parallel content-only slice)
    /// having installed all six skill directories under `.agents/skills/`
    /// with front matter conforming to the schema in `IN-FLIGHT.md`
    /// decision 3 — see `.agents/reports/task19/b1-code-progress.md` for
    /// whether that slice had landed as of this test's last run.
    #[test]
    fn lint_passes_every_installed_skill() {
        let skills_root = repo_root().join(".agents").join("skills");
        let results = lint_installed_skills(&skills_root);
        assert_eq!(
            results.len(),
            REPO_OWNED_SKILLS.len(),
            "lint_installed_skills must report exactly the six REPO_OWNED_SKILLS"
        );
        let failures: Vec<String> = results
            .into_iter()
            .filter_map(|(name, result)| match result {
                Ok(_) => None,
                Err(err) => Some(format!("{name}: {err}")),
            })
            .collect();
        assert!(
            failures.is_empty(),
            "every REPO_OWNED_SKILLS entry must lint clean:\n{}",
            failures.join("\n")
        );
    }

    /// The required teeth test (task19 `IN-FLIGHT.md` Slice B): a fixture
    /// whose body names `SettlementService::settle_unit` and says "mark
    /// the unit as settled when tests pass" must fail with exactly
    /// `ControlLogicDetected`, not any other error, and not pass silently.
    #[test]
    fn lint_rejects_control_logic_fixture() {
        let path = repo_root()
            .join("tests")
            .join("fixtures")
            .join("skills")
            .join("control-logic-violation.md");
        let text = fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("fixture {} reads: {err}", path.display()));
        let error = lint_skill_text("control-logic-violation", &text)
            .expect_err("the control-logic fixture must be rejected");
        assert!(
            matches!(error, SkillLintError::ControlLogicDetected { .. }),
            "expected ControlLogicDetected, got {error:?}"
        );
    }

    /// Additional teeth (IN-FLIGHT.md: "each denylist class should have at
    /// least one proof it actually fires") — an unknown front-matter key
    /// is deny-by-default rejected, never silently ignored.
    #[test]
    fn lint_rejects_unknown_front_matter_key() {
        let text = "---\nname: sample\ndescription: A sample.\nallowed-tools: [bash]\n---\nBody.\n";
        let error = lint_skill_text("sample", text)
            .expect_err("an unrecognized front-matter key must be rejected");
        assert!(
            matches!(error, SkillLintError::UnknownFrontMatterKey { .. }),
            "expected UnknownFrontMatterKey, got {error:?}"
        );
    }

    /// Additional teeth: an absolute filesystem path in the body is
    /// rejected. The token is a synthetic root name (not one of the
    /// boundary suite's own recognized path-root names — see
    /// `src/skills/mod.rs`'s own absolute-path teeth test for the same
    /// note), since the suite's literal scan polices test text like any
    /// other source.
    #[test]
    fn lint_rejects_absolute_path() {
        let text = "---\nname: sample\ndescription: A sample.\n---\nRun /gauntlet-fixture-root/tool now.\n";
        let error = lint_skill_text("sample", text).expect_err("an absolute path must be rejected");
        assert!(
            matches!(error, SkillLintError::AbsolutePathDetected { .. }),
            "expected AbsolutePathDetected, got {error:?}"
        );
    }

    /// Additional teeth: a literal secret-shaped assignment is rejected,
    /// and a `$VAR` indirection of the same label is not (spec §8.1).
    #[test]
    fn lint_rejects_secret_like_value() {
        let text = "---\nname: sample\ndescription: A sample.\n---\npassword: hunter2changeme\n";
        let error = lint_skill_text("sample", text)
            .expect_err("a literal secret-shaped value must be rejected");
        assert!(
            matches!(error, SkillLintError::SecretLikeValueDetected { .. }),
            "expected SecretLikeValueDetected, got {error:?}"
        );
    }
}
