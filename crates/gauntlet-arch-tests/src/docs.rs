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
//!   Matching (both the tokens and the AK grammar) is case-insensitive:
//!   `Delivered — ak-060` and `IMPLEMENTED — AK-060` fail closed exactly
//!   like their lowercase spellings (COR-B3, falsifier F8), and the
//!   common Cyrillic/Latin homoglyphs are folded to their ASCII
//!   counterparts inside the matchers (`Dеlivered — AK-060` with a
//!   Cyrillic е fails like the ASCII spelling; COR-B3-2, falsifier
//!   round-2 R2-F5).
//!
//! # Honest residual — the confusable table is NOT exhaustive
//!
//! [`ASCII_CONFUSABLES`] covers the common Cyrillic look-alikes only.
//! Exotic scripts and look-alike classes beyond it (Greek, fullwidth,
//! mathematical alphanumerics, digit look-alikes like 0/O and 1/l — the
//! latter would corrupt the AK digit grammar) stay outside these legs'
//! reach. The residual threat is repo-authored prose (weak — a human
//! reviewer sees what the falsifier's reader saw), with review as the
//! second net; an exhaustive Unicode-confusable pass would be a
//! dependency or a data table out of proportion to it.
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
//! - This leg is line-anchored: a NON-canonical status-like token split
//!   across lines from its id (token on one line, `AK-NNN` on the next,
//!   with no six-word vocabulary token anywhere near) is outside its
//!   reach — the wave-2 cross-check's window deliberately binds only the
//!   six vocabulary WORDS, never bare status-like tokens, so ordinary
//!   prose ("proven vs provisional" headings) cannot false-positive.
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

/// The common Cyrillic/Latin homoglyphs (falsifier round-2 R2-F5),
/// each folded to its ASCII counterpart INSIDE the token matchers
/// only — the documents' bytes are never rewritten and the canonical
/// emission is never altered. A plain static table, no dependency.
///
/// NOT exhaustive by design: scripts beyond this table (Greek, fullwidth,
/// mathematical alphanumerics, digit look-alikes like 0/O and 1/l — the
/// latter would corrupt the AK digit grammar) stay outside the legs'
/// reach. The residual class is repo-authored prose — a weak threat with
/// human review as the second net; recorded in the module scope notes.
pub(crate) const ASCII_CONFUSABLES: &[(char, char)] = &[
    ('\u{0410}', 'A'), // Cyrillic А -> A
    ('\u{0412}', 'B'), // Cyrillic В -> B
    ('\u{0415}', 'E'), // Cyrillic Е -> E
    ('\u{041a}', 'K'), // Cyrillic К -> K
    ('\u{041c}', 'M'), // Cyrillic М -> M
    ('\u{041d}', 'H'), // Cyrillic Н -> H
    ('\u{041e}', 'O'), // Cyrillic О -> O
    ('\u{0420}', 'P'), // Cyrillic Р -> P
    ('\u{0421}', 'C'), // Cyrillic С -> C
    ('\u{0422}', 'T'), // Cyrillic Т -> T
    ('\u{0423}', 'Y'), // Cyrillic У -> Y
    ('\u{0425}', 'X'), // Cyrillic Х -> X
    ('\u{0430}', 'a'), // Cyrillic а -> a
    ('\u{0435}', 'e'), // Cyrillic е -> e
    ('\u{043e}', 'o'), // Cyrillic о -> o
    ('\u{0440}', 'p'), // Cyrillic р -> p
    ('\u{0441}', 'c'), // Cyrillic с -> c
    ('\u{0443}', 'y'), // Cyrillic у -> y
    ('\u{0445}', 'x'), // Cyrillic х -> x
];

/// Folds the table's homoglyphs to their ASCII counterparts. Callers
/// apply it to the NORMALIZED scan input only — this is matching help,
/// never a rewrite of the document.
pub(crate) fn fold_confusables(text: &str) -> String {
    text.chars()
        .map(|c| {
            ASCII_CONFUSABLES
                .iter()
                .find(|(from, _)| *from == c)
                .map_or(c, |&(_, to)| to)
        })
        .collect()
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
/// Callers normalize (lowercase) first for case-insensitive matching —
/// the §26 vocabulary and the AK grammar are case-insensitive in prose
/// even though the canonical emission is lowercase (COR-B3, falsifier
/// F8's classes 1/2/5).
pub(crate) fn contains_word(haystack: &str, needle: &str) -> bool {
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
/// after the hyphen) does not count. Case-insensitive (COR-B3): the id
/// grammar accepts any casing — `ak-053` names a unit exactly like
/// `AK-053` — so the line is normalized before scanning.
fn names_a_unit(line: &str) -> bool {
    let lowered = fold_confusables(&line.to_lowercase());
    let bytes = lowered.as_bytes();
    let mut i = 0;
    while let Some(found) = lowered[i..].find("ak-") {
        let at = i + found;
        let mut end = at + 3;
        while end < bytes.len() && bytes[end].is_ascii_digit() {
            end += 1;
        }
        let has_digits = end > at + 3;
        let boundary_before =
            at == 0 || !lowered[..at].chars().next_back().is_some_and(is_word_char);
        let boundary_after = !lowered[end..].chars().next().is_some_and(is_word_char);
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
/// find canonically-spelled, machine-checkable mentions only. Matching is
/// CASE-INSENSITIVE (COR-B3, falsifier F8): `Delivered — ak-060` and
/// `IMPLEMENTED — AK-060` are violations exactly like their lowercase
/// spellings — the line is normalized before matching, and the AK-id
/// grammar accepts any casing.
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
            let lowered = fold_confusables(&line.to_lowercase());
            if !names_a_unit(&lowered) {
                continue;
            }
            let status_like = STATUS_LIKE_TOKENS
                .iter()
                .filter(|token| contains_word(&lowered, token))
                .copied()
                .collect::<Vec<_>>();
            if status_like.is_empty() {
                continue;
            }
            let canonical = STATUS_WORDS
                .iter()
                .any(|word| contains_word(&lowered, word));
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

    /// COR-B3-2: the fold table is well-formed — every entry folds a
    /// NON-ASCII char to a DIFFERENT ASCII char, no duplicate sources —
    /// and it actually folds the falsifier's two executed code points.
    #[test]
    fn the_confusable_table_is_ascii_and_deduplicated() {
        let mut sources = std::collections::BTreeSet::new();
        for &(from, to) in ASCII_CONFUSABLES {
            assert!(!from.is_ascii(), "{from:?} is already ASCII");
            assert!(to.is_ascii(), "{to:?} is not ASCII");
            assert_ne!(from, to);
            assert!(sources.insert(from), "{from:?} appears twice");
        }
        assert_eq!(fold_confusables("\u{0410}"), "A");
        assert_eq!(fold_confusables("\u{0435}"), "e");
        assert_eq!(
            fold_confusables("plain ASCII is untouched — éü stay"),
            "plain ASCII is untouched — éü stay"
        );
    }

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

    /// COR-B3 (falsifier F8 classes 1/2/5 at the vocabulary leg):
    /// case variants — leading capital, ALL CAPS, lowercase unit id —
    /// of status-like tokens beside an id are violations exactly like
    /// their canonical-casing spellings; a case variant of a CANONICAL
    /// word satisfies this leg (its ahead-of-evidence adjudication is
    /// the cross-check leg's job).
    #[test]
    fn capitalized_status_tokens_are_violations_and_capitalized_canonical_words_are_not() {
        let tmp = std::env::temp_dir().join("gauntlet-arch-tests-docs-cased");
        let docs = tmp.join("docs");
        for name in DOC_SET {
            std::fs::create_dir_all(&docs).expect("create tmp docs dir");
            std::fs::write(docs.join(name), STATUS_SECTION).expect("write doc");
        }
        std::fs::write(
            docs.join("SECURITY.md"),
            format!(
                "{STATUS_SECTION}\n- Delivered — AK-060: capitalized bogus claim\n\
                 - IMPLEMENTED — ak-060: all-caps bogus claim on a lowercase id\n"
            ),
        )
        .expect("write doc");
        let violations = check_status_claim_vocabulary(&tmp);
        assert!(
            violations
                .iter()
                .any(|v| v.contains("SECURITY.md") && v.contains("delivered")),
            "the capitalized bogus token must be a violation: {violations:?}"
        );
        assert!(
            violations
                .iter()
                .any(|v| v.contains("SECURITY.md") && v.contains("implemented")),
            "the all-caps bogus token beside a lowercase id must be a violation: {violations:?}"
        );
        // A case variant of a canonical word is canonical HERE (the
        // vocabulary leg is casing-insensitive); whether it is AHEAD of
        // evidence is the cross-check leg's question.
        std::fs::write(
            docs.join("SECURITY.md"),
            format!("{STATUS_SECTION}\n- IMPLEMENTED_AND_PROVEN — ak-060: cased canonical\n"),
        )
        .expect("write doc");
        assert!(
            check_status_claim_vocabulary(&tmp).is_empty(),
            "a cased canonical word satisfies the vocabulary leg"
        );
        let _ = std::fs::remove_dir_all(&tmp);
    }

    /// COR-B3-2 (falsifier round-2 R2-F5): a Cyrillic homoglyph inside a
    /// status-like token beside a unit id is a violation exactly like
    /// its ASCII spelling — the vocabulary leg folds the common
    /// Cyrillic/Latin look-alikes before matching. \u{0435} is Cyrillic е.
    #[test]
    fn a_homoglyph_status_token_beside_a_unit_id_is_a_violation() {
        let tmp = std::env::temp_dir().join("gauntlet-arch-tests-docs-homoglyph");
        let docs = tmp.join("docs");
        for name in DOC_SET {
            std::fs::create_dir_all(&docs).expect("create tmp docs dir");
            std::fs::write(docs.join(name), STATUS_SECTION).expect("write doc");
        }
        std::fs::write(
            docs.join("FAILURE-RECOVERY.md"),
            format!("{STATUS_SECTION}\n- D\u{0435}livered — AK-042: homoglyph claim\n"),
        )
        .expect("write doc");
        let violations = check_status_claim_vocabulary(&tmp);
        assert!(
            violations
                .iter()
                .any(|v| v.contains("FAILURE-RECOVERY.md") && v.contains("delivered")),
            "the homoglyph spelling of a status-like token must be a violation: {violations:?}"
        );
        // An ASCII-clean canonical line stays clean through the fold.
        std::fs::write(
            docs.join("FAILURE-RECOVERY.md"),
            format!("{STATUS_SECTION}\n- implemented_and_proven — AK-042: plain ASCII\n"),
        )
        .expect("write doc");
        assert!(check_status_claim_vocabulary(&tmp).is_empty());
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
