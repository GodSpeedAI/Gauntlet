//! Integration test for the formation invariant checker's teeth (plan Task
//! 18 step 5; AK-065). The AK-065 locator names
//! `tests/contract/formation::fog_not_in_answer_key`; this file delivers
//! that test as `formation::fog_not_in_answer_key` — the same recorded
//! locator-divergence class as task17's `roles.rs` and this task's own
//! `skills.rs`, never a reason to touch the ruler.
//!
//! Proves the actual teeth (plan Done-when): a clean map/key pair passes
//! every §9.2 invariant, and the fog-leak pair is refused with the SPECIFIC
//! `FogPromoted` variant — matched by pattern, not merely `is_err()` — so
//! this proves the right invariant fired, not just that something failed.

use std::fs;
use std::path::PathBuf;

use gauntlet_app::formation::{FormationError, check_reference_invariants};

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
fn fog_not_in_answer_key() {
    // Positive: the clean map.md / answer-key.md pair passes every §9.2
    // invariant.
    let clean_map = fixture("map.md");
    let clean_key = fixture("answer-key.md");
    check_reference_invariants(&clean_map, &clean_key)
        .expect("the clean fixture pair satisfies every formation invariant");

    // Negative: map-fog-leak.md's [F1] carries an AK-103 reference, and
    // answer-key-fog-leak.md carries a structurally valid AK-103 block —
    // schema validity alone must NOT catch this; only the formation
    // cross-check does, and it must name the SPECIFIC invariant that
    // fired.
    let fog_leak_map = fixture("map-fog-leak.md");
    let fog_leak_key = fixture("answer-key-fog-leak.md");
    let error = check_reference_invariants(&fog_leak_map, &fog_leak_key)
        .expect_err("a fog bullet carrying an AK- reference is refused");
    match error {
        FormationError::FogPromoted { marker, ids } => {
            assert_eq!(marker, "[F1]", "the offending bullet is named");
            assert_eq!(
                ids,
                vec!["AK-103".to_owned()],
                "the leaked reference is named"
            );
        }
        other => panic!("expected FormationError::FogPromoted, got {other:?}"),
    }
}
