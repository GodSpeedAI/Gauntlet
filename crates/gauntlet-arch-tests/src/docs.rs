//! Documentation-set presence and status-vocabulary checks (spec §25, plan
//! Task 27; charter decision 7).
//!
//! # Instrument and evidence locator
//!
//! The frozen answer key addresses this work at
//! `tests/contract/docs_present` (AK-068, kind `test_report`): a cargo test
//! target of this crate whose NAME matches the locator and whose PATH is
//! the repo-root `tests/contract/` directory — the same address-vs-logic
//! precedent as `architecture_boundaries` (AK-001). The checking logic
//! lives here; that file is the address.
//!
//! # What this enforces
//!
//! - the eight §25 files exist under `docs/` and are non-empty;
//! - each carries a `## Proven vs provisional` section (spec §26: the final
//!   report distinguishes proven from provisional; a document that never
//!   states which parts are proven is exactly the defect Task 27 exists to
//!   prevent);
//! - `docs/HANDOFF.md` names the canonical handoff locator
//!   (`.agents/reports/handoff.md`, AK-067's frozen locator) so the thin
//!   pointer cannot drift into a second handoff;
//! - `docs/CONFORMANCE.md` carries its binding content (task-27 charter
//!   decision 8): the trust-posture, sandbox/tier, profile-adjudication,
//!   and advisory-only-enforcement sections, and the UNK-001 terminal
//!   disposition with its stable phrase;
//! - every unit-level status claim in the eight files uses exactly the six
//!   §26 status words: on any line naming a ruler unit (`AK-NNN`), a
//!   status-like token is admissible only when one of the six canonical
//!   words is on that same line. The canonical forms are the underscore
//!   spellings `implemented_and_proven`, `implemented_but_provisional`,
//!   `configured_but_untested`, plus `proposed`, `blocked`, `unknown`.
//!
//! # What this does NOT enforce (deliberately)
//!
//! - It does not read `.agents/reports/conformance.json` and does not
//!   cross-check any status word against the sweep's adjudication. That
//!   leg belongs to the task-27 wave-2 consistency test (charter slice B3);
//!   this module checks structure and vocabulary only.
//! - It does not judge whether a status word is EARNED by evidence — only
//!   the sweep can. A canonically-spelled claim ahead of its evidence is
//!   caught by the wave-2 cross-check, not here.
//! - It polices only the eight UPPERCASE §25 files. The operator's
//!   lowercase `docs/` tree is out of scope by charter (never modified,
//!   never a second ruler).
//!
//! Teeth are structural: delete one of the eight files, strip a status
//! section, or put a non-canonical status word next to an `AK-` id, and
//! this goes RED (proof recorded in `.agents/reports/task27/b2-report.md`).

use std::path::Path;

/// The §25 documentation set, in spec order.
pub const DOC_SET: [&str; 8] = [
    "ARCHITECTURE.md",
    "CONFORMANCE.md",
    "CONFIGURATION.md",
    "SECURITY.md",
    "FAILURE-RECOVERY.md",
    "OPERATOR-GUIDE.md",
    "SELF-IMPROVEMENT.md",
    "HANDOFF.md",
];

/// The section every one of the eight must carry (spec §26 / charter d7).
pub const STATUS_SECTION: &str = "## Proven vs provisional";

/// The canonical handoff report's frozen locator (AK-067).
pub const CANONICAL_HANDOFF_LOCATOR: &str = ".agents/reports/handoff.md";

/// The stable phrase anchoring CONFORMANCE.md's UNK-001 terminal
/// disposition (task-27 charter decision 8; wording seeded by
/// `.agents/reports/task14/structural-review-unk001.md` R1c).
pub const UNK001_STABLE_PHRASE: &str = "cross_cutting containment — a declared substitution";

/// Sections `docs/CONFORMANCE.md` must carry beyond the status section.
pub const CONFORMANCE_SECTIONS: [&str; 5] = [
    "## Trust posture",
    "## Sandbox backends and tier mapping",
    "## Profile adjudication",
    "## Advisory-only enforcement",
    "## UNK-001 — terminal disposition",
];

/// The six §26 status categories, machine spelling (charter decision 5).
pub const STATUS_WORDS: [&str; 6] = [
    "implemented_and_proven",
    "implemented_but_provisional",
    "configured_but_untested",
    "proposed",
    "blocked",
    "unknown",
];

/// Bare tokens that read as a status claim about a unit. None of them
/// occurs inside a canonical underscore form (underscores are word
/// characters), so `implemented_but_provisional` does NOT contain the bare
/// token `provisional`. Mixed-up near-misses of the canonical spellings are
/// listed explicitly so a plausible-but-wrong category word fails closed.
const STATUS_LIKE_TOKENS: [&str; 22] = [
    "proven",
    "provisional",
    "untested",
    "tested",
    "configured",
    "implemented",
    "complete",
    "completed",
    "delivered",
    "done",
    "passing",
    "working",
    "operational",
    "met",
    "partial",
    "absent",
    "ready",
    "finished",
    "implemented_and_provisional",
    "implemented_but_proven",
    "configured_but_tested",
    "configured_but_unproven",
];

fn read_doc(root: &Path, name: &str) -> Result<String, String> {
    std::fs::read_to_string(root.join("docs").join(name))
        .map_err(|err| format!("docs-set: {name} unreadable: {err}"))
}

/// (a) All eight §25 files exist under `docs/` and are non-empty. An
/// unreadable file is a violation, never a skip (I8).
pub fn check_docs_files_present(root: &Path) -> Vec<String> {
    let mut violations = Vec::new();
    for name in DOC_SET {
        match read_doc(root, name) {
            Ok(text) => {
                if text.trim().is_empty() {
                    violations.push(format!("docs-set: docs/{name} is empty"));
                }
            }
            Err(err) => violations.push(err),
        }
    }
    violations
}

/// (b) Every one of the eight carries the status section.
pub fn check_status_section_present(root: &Path) -> Vec<String> {
    let mut violations = Vec::new();
    for name in DOC_SET {
        match read_doc(root, name) {
            Ok(text) => {
                if !text.contains(STATUS_SECTION) {
                    violations.push(format!(
                        "docs-set: docs/{name} lacks the `{STATUS_SECTION}` section"
                    ));
                }
            }
            Err(err) => violations.push(err),
        }
    }
    violations
}

/// (c) `docs/HANDOFF.md` is a thin pointer: it names the canonical handoff
/// locator, so per-unit truth stays single-sourced.
pub fn check_handoff_pointer(root: &Path) -> Vec<String> {
    let mut violations = Vec::new();
    match read_doc(root, "HANDOFF.md") {
        Ok(text) => {
            if !text.contains(CANONICAL_HANDOFF_LOCATOR) {
                violations.push(format!(
                    "docs-set: docs/HANDOFF.md does not name the canonical locator \
                     `{CANONICAL_HANDOFF_LOCATOR}`"
                ));
            }
            for word in STATUS_WORDS {
                if !text.contains(word) {
                    violations.push(format!(
                        "docs-set: docs/HANDOFF.md omits the §26 category `{word}` \
                         from its summary table"
                    ));
                }
            }
        }
        Err(err) => violations.push(err),
    }
    violations
}

/// (d) `docs/CONFORMANCE.md` carries its binding content list (charter
/// decision 8, mechanically spot-checked).
pub fn check_conformance_binding_content(root: &Path) -> Vec<String> {
    let mut violations = Vec::new();
    match read_doc(root, "CONFORMANCE.md") {
        Ok(text) => {
            for section in CONFORMANCE_SECTIONS {
                if !text.contains(section) {
                    violations.push(format!(
                        "docs-set: docs/CONFORMANCE.md lacks the `{section}` section"
                    ));
                }
            }
            if !text.contains(UNK001_STABLE_PHRASE) {
                violations.push(format!(
                    "docs-set: docs/CONFORMANCE.md lacks the UNK-001 disposition phrase \
                     `{UNK001_STABLE_PHRASE}`"
                ));
            }
        }
        Err(err) => violations.push(err),
    }
    violations
}

/// `haystack` contains `needle` as a whole word: the characters around it
/// are not `[A-Za-z0-9_]`. Underscores count as word characters, so the
/// bare token `proven` does not match inside `implemented_but_proven`.
fn contains_word(haystack: &str, needle: &str) -> bool {
    let mut start = 0;
    while let Some(found) = haystack[start..].find(needle) {
        let at = start + found;
        let end = at + needle.len();
        let boundary_before =
            at == 0 || !haystack[..at].chars().next_back().is_some_and(is_word_char);
        let boundary_after = !haystack[end..].chars().next().is_some_and(is_word_char);
        if boundary_before && boundary_after {
            return true;
        }
        start = at + needle.len().max(1);
    }
    false
}

fn is_word_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// The line names a ruler unit: `AK-` followed by at least one digit,
/// delimited as a word on both sides. The prose literal `AK-NNN` (letters
/// after the hyphen) does not count.
fn names_a_unit(line: &str) -> bool {
    let bytes = line.as_bytes();
    let mut i = 0;
    while let Some(found) = line[i..].find("AK-") {
        let at = i + found;
        let mut end = at + 3;
        while end < bytes.len() && bytes[end].is_ascii_digit() {
            end += 1;
        }
        let has_digits = end > at + 3;
        let boundary_before = at == 0 || !line[..at].chars().next_back().is_some_and(is_word_char);
        let boundary_after = !line[end..].chars().next().is_some_and(is_word_char);
        if has_digits && boundary_before && boundary_after {
            return true;
        }
        i = at + 3;
    }
    false
}

/// (e) Status vocabulary: on any line of the eight files that names a ruler
/// unit, a status-like token is admissible only beside one of the six
/// canonical §26 words. A bare `proven`/`delivered`/`tested`/mixed-spelling
/// claim adjacent to an `AK-` id fails closed — the wave-2 cross-check must
/// find canonically-spelled, machine-checkable mentions only.
pub fn check_status_claim_vocabulary(root: &Path) -> Vec<String> {
    let mut violations = Vec::new();
    for name in DOC_SET {
        let text = match read_doc(root, name) {
            Ok(text) => text,
            Err(err) => {
                violations.push(err);
                continue;
            }
        };
        for (index, line) in text.lines().enumerate() {
            if !names_a_unit(line) {
                continue;
            }
            let status_like = STATUS_LIKE_TOKENS
                .iter()
                .filter(|token| contains_word(line, token))
                .copied()
                .collect::<Vec<_>>();
            if status_like.is_empty() {
                continue;
            }
            let canonical = STATUS_WORDS.iter().any(|word| contains_word(line, word));
            if !canonical {
                violations.push(format!(
                    "docs-set: docs/{name}:{} names a ruler unit with a non-canonical \
                     status token ({}) but none of the six §26 words",
                    index + 1,
                    status_like.join(", ")
                ));
            }
        }
    }
    violations
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn word_boundaries_treat_underscores_as_word_characters() {
        assert!(contains_word(
            "implemented_but_provisional — AK-060",
            "AK-060"
        ));
        // The bare token `proven` must NOT match inside the canonical form.
        assert!(!contains_word(
            "implemented_but_provisional — AK-060: prose",
            "proven"
        ));
        assert!(!contains_word(
            "implemented_and_proven — AK-060: prose",
            "provisional"
        ));
        // Hyphens and spaces are boundaries.
        assert!(contains_word("proven-vs-provisional section", "proven"));
        assert!(contains_word("status: proven", "proven"));
    }

    #[test]
    fn unit_detection_requires_digits() {
        assert!(names_a_unit(
            "- implemented_but_provisional — AK-068: prose"
        ));
        assert!(names_a_unit("see AK-001 and AK-002"));
        // Prose literals and near-misses do not count.
        assert!(!names_a_unit("(`AK-NNN`), the status word"));
        assert!(!names_a_unit("the AK- class"));
    }

    #[test]
    fn a_bogus_status_word_beside_a_unit_id_is_a_violation() {
        let tmp = std::env::temp_dir().join("gauntlet-arch-tests-docs-bogus");
        let docs = tmp.join("docs");
        for name in DOC_SET {
            std::fs::create_dir_all(&docs).expect("create tmp docs dir");
            std::fs::write(docs.join(name), STATUS_SECTION).expect("write doc");
        }
        std::fs::write(
            docs.join("OPERATOR-GUIDE.md"),
            format!("{STATUS_SECTION}\n- proven — AK-060: bogus claim\n"),
        )
        .expect("write doc");
        assert!(
            check_status_claim_vocabulary(&tmp)
                .iter()
                .any(|v| v.contains("OPERATOR-GUIDE.md") && v.contains("non-canonical")),
            "the bogus status word beside AK-060 must be a violation"
        );
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
