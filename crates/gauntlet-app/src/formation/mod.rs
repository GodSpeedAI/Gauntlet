//! Formation-time invariant checks over the project's `map.md` convention
//! (plan Task 18 step 5; spec §9.2): "every entry in Decisions so far
//! either produced ≥1 Unit or is recorded as context-only; nothing in Not
//! yet specified appears in the answer key; nothing in Out of scope
//! appears in either."
//!
//! This module reads no path and owns no file: [`check_reference_invariants`]
//! takes `map_text` and `answer_key_text` as plain strings, so callers
//! decide where those bytes come from. `map_text` is parsed as
//! fixture-grade markdown (not CommonMark: a `#`-heading whose text
//! matches one of the three section titles, at any heading depth,
//! case-insensitively, and tolerating a trailing colon, switches the
//! current section; a heading that matches none of them switches
//! explicitly to a fourth `Unrecognized` state rather than leaving the
//! previous section current — see [`Section`] — then `- [marker] text`
//! bullet lines belong to whatever section is current); `answer_key_text`
//! goes through the shared
//! [`super::reference::validate`] wrapper over the Task 7 parser — reused,
//! not reimplemented. The answer key is not a decorative parameter:
//! beyond the three §9.2 checks over `map_text` alone, every `AK-` id a
//! Decisions bullet references must actually resolve to a parsed Unit, or
//! the reference is dangling. A bullet carrying an `AK-` reference while
//! `Unrecognized` is current is also refused (see
//! [`FormationError::AmbiguousSectionReference`]): correction round 2
//! (falsifier round 2) found that "unrecognized heading → leave the
//! current section unchanged" was itself the bug, since it silently
//! re-files a fog/out-of-scope bullet into whatever section preceded an
//! unrecognized heading (e.g. `## Not yet specified:` with a trailing
//! colon).

use std::fmt;

use super::reference::parse::BlockFailure;
use super::reference::validate::validate_answer_key;

/// The map.md section a bullet line currently falls under. The three
/// canonical §9.2 sections are matched by heading text, case-insensitively
/// and at any `#`-heading depth; `Unrecognized` is the fourth, explicit
/// state for everything else — the document's own top title, prose
/// between sections, or a heading whose text doesn't match one of the
/// three canonical titles.
///
/// `Unrecognized` is not a "leave it as whatever it was" fallback: it is
/// a real, distinct state a heading switches *into*, the same way it
/// switches into any of the other three. Correction round 1 fixed a set
/// of specific heading spellings the recognizer failed to match (ASCII
/// arrow, case variance, deeper heading depth); a fresh falsifier then
/// found the same underlying bug survives via a spelling round 1 didn't
/// enumerate (a trailing colon), because the *default* for "no match" was
/// "keep the previous section," which silently re-files a fog/out-of-scope
/// bullet as if it were still under the last recognized section. Growing
/// the allowlist of recognized spellings only produces the next
/// unenumerated variant; the fix here removes the permissive default
/// itself: no match now always means `Unrecognized`, never "unchanged."
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Section {
    Decisions,
    NotYetSpecified,
    OutOfScope,
    /// No recognized section heading has (yet, or currently) opened —
    /// includes text before the first heading and any heading that isn't
    /// one of the three canonical titles. A bullet carrying an `AK-`
    /// reference while in this state is refused (see
    /// [`FormationError::AmbiguousSectionReference`]): its section
    /// identity can't be trusted, so it must not be treated as an
    /// implicitly validated decision.
    Unrecognized,
}

/// Matches `heading` (already `.trim()`-ed by [`heading_text`]) against the
/// three canonical §9.2 section titles, case-insensitively. Before
/// comparing, also strips one trailing `:` (and any whitespace before or
/// after it) so `"Not yet specified:"` and `"Not yet specified :"` both
/// match `"Not yet specified"` — an LLM-authored `map.md` writing a
/// heading as if it introduced a list is a plausible, non-adversarial
/// spelling, not just the exact three canonical strings.
fn section_of(heading: &str) -> Option<Section> {
    let normalized = heading.trim().trim_end_matches(':').trim_end();
    if normalized.eq_ignore_ascii_case("Decisions so far") {
        Some(Section::Decisions)
    } else if normalized.eq_ignore_ascii_case("Not yet specified") {
        Some(Section::NotYetSpecified)
    } else if normalized.eq_ignore_ascii_case("Out of scope") {
        Some(Section::OutOfScope)
    } else {
        None
    }
}

/// Returns the trimmed text of `line` when it is a Markdown heading (one or
/// more leading `#` characters followed by whitespace), regardless of
/// depth. `None` when `line` is not heading-shaped at all.
fn heading_text(line: &str) -> Option<&str> {
    let after_hashes = line.trim_start_matches('#');
    if after_hashes.len() == line.len() {
        // No leading '#': not a heading line.
        return None;
    }
    let after_marker = after_hashes.strip_prefix(char::is_whitespace)?;
    Some(after_marker.trim())
}

/// One parsed bullet: its marker as written (`[D1]`, `[F1]`, `[S1]`, ...),
/// the `AK-` ids it references (empty when it carries none), and whether
/// it carried the literal `context-only` marker instead.
#[derive(Debug, Clone)]
struct Bullet {
    marker: String,
    ids: Vec<String>,
    context_only: bool,
}

/// §9.2 invariant violations, each carrying the offending bullet's marker
/// as the failure locus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormationError {
    /// A `Decisions so far` bullet carries neither an `AK-` reference nor
    /// the `context-only` marker.
    DecisionWithoutUnit { marker: String },
    /// A `Not yet specified` bullet carries an `AK-` reference: fog must
    /// never graduate into the answer key without first becoming a
    /// decision.
    FogPromoted { marker: String, ids: Vec<String> },
    /// An `Out of scope` bullet carries an `AK-` reference.
    OutOfScopePromoted { marker: String, ids: Vec<String> },
    /// A bullet carries an `AK-` reference while no recognized §9.2
    /// section is current — either nothing has opened a tracked section
    /// yet (text before the first heading, or the document's own title),
    /// or the nearest preceding heading didn't match one of the three
    /// canonical titles. The bullet's section identity is ambiguous, so
    /// the reference cannot be trusted as a validated decision; refusing
    /// it is the restrictive counterpart of `FogPromoted` /
    /// `OutOfScopePromoted` for the "we don't actually know which section
    /// this is" case.
    AmbiguousSectionReference { marker: String, ids: Vec<String> },
    /// A `Decisions so far` bullet references an `AK-` id the answer key
    /// does not actually contain — a dangling reference the schema-valid
    /// parse alone cannot catch.
    DanglingReference { marker: String, id: String },
    /// `answer_key_text` itself failed the Task 7 strict parse.
    AnswerKeyInvalid { cause: BlockFailure },
}

impl fmt::Display for FormationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DecisionWithoutUnit { marker } => write!(
                f,
                "{marker}: a Decisions so far bullet must carry >=1 AK- reference or the \
                 literal context-only marker"
            ),
            Self::FogPromoted { marker, ids } => write!(
                f,
                "{marker}: a Not yet specified bullet carries AK- reference(s) {ids:?}; fog \
                 must never appear in the answer key"
            ),
            Self::OutOfScopePromoted { marker, ids } => write!(
                f,
                "{marker}: an Out of scope bullet carries AK- reference(s) {ids:?}; out-of-scope \
                 items must never appear in the answer key"
            ),
            Self::AmbiguousSectionReference { marker, ids } => write!(
                f,
                "{marker}: carries AK- reference(s) {ids:?} outside any recognized §9.2 \
                 section; its section identity is ambiguous and cannot be trusted as a \
                 validated decision"
            ),
            Self::DanglingReference { marker, id } => write!(
                f,
                "{marker}: references {id}, which the answer key does not contain"
            ),
            Self::AnswerKeyInvalid { cause } => {
                write!(f, "the answer key does not validate: {cause}")
            }
        }
    }
}

impl std::error::Error for FormationError {}

/// Checks the three §9.2 formation invariants over a `map.md`/answer-key
/// pair. `answer_key_text` is validated with the shared Task 7 parser
/// wrapper and its parsed Unit ids back the dangling-reference check.
pub fn check_reference_invariants(
    map_text: &str,
    answer_key_text: &str,
) -> Result<(), FormationError> {
    let units = validate_answer_key(answer_key_text)
        .map_err(|cause| FormationError::AnswerKeyInvalid { cause })?;
    let known_ids: Vec<&str> = units.iter().map(|unit| unit.id().as_str()).collect();

    let bullets = parse_sections(map_text);

    for bullet in &bullets.not_yet_specified {
        if !bullet.ids.is_empty() {
            return Err(FormationError::FogPromoted {
                marker: bullet.marker.clone(),
                ids: bullet.ids.clone(),
            });
        }
    }
    for bullet in &bullets.out_of_scope {
        if !bullet.ids.is_empty() {
            return Err(FormationError::OutOfScopePromoted {
                marker: bullet.marker.clone(),
                ids: bullet.ids.clone(),
            });
        }
    }
    for bullet in &bullets.unrecognized {
        if !bullet.ids.is_empty() {
            return Err(FormationError::AmbiguousSectionReference {
                marker: bullet.marker.clone(),
                ids: bullet.ids.clone(),
            });
        }
    }
    for bullet in &bullets.decisions {
        if !bullet.context_only && bullet.ids.is_empty() {
            return Err(FormationError::DecisionWithoutUnit {
                marker: bullet.marker.clone(),
            });
        }
        for id in &bullet.ids {
            if !known_ids.contains(&id.as_str()) {
                return Err(FormationError::DanglingReference {
                    marker: bullet.marker.clone(),
                    id: id.clone(),
                });
            }
        }
    }
    Ok(())
}

/// The bullets collected per section, including the `unrecognized` bucket
/// for bullets whose current section is ambiguous.
#[derive(Debug, Default)]
struct Sections {
    decisions: Vec<Bullet>,
    not_yet_specified: Vec<Bullet>,
    out_of_scope: Vec<Bullet>,
    /// Bullets seen while no recognized section was current: before any
    /// heading has matched, or after a heading that didn't match one of
    /// the three canonical titles. Checked in
    /// [`check_reference_invariants`] the same way `not_yet_specified` and
    /// `out_of_scope` are — any `AK-` reference here is refused.
    unrecognized: Vec<Bullet>,
}

/// Splits `map_text` into its sections' bullets. A heading line (any depth
/// of leading `#`s, matched case-insensitively, tolerating a trailing
/// colon) switches the current section: to one of the three canonical
/// sections when its text matches, or explicitly to `Unrecognized`
/// otherwise — an unmatched heading is never treated as "leave the
/// current section as it was," since that permissive default is exactly
/// what let a fog/out-of-scope bullet get silently misfiled into whatever
/// section preceded an unrecognized heading. The parser starts in
/// `Unrecognized` before any heading has been seen. A `- [marker] ...`
/// line belongs to whatever section is current, tracked or not.
fn parse_sections(map_text: &str) -> Sections {
    let mut sections = Sections::default();
    let mut current = Section::Unrecognized;
    for raw_line in map_text.split('\n') {
        let line = raw_line.strip_suffix('\r').unwrap_or(raw_line);
        if let Some(heading) = heading_text(line) {
            current = section_of(heading).unwrap_or(Section::Unrecognized);
            continue;
        }
        let Some(trimmed) = line.trim_start().strip_prefix("- ") else {
            continue;
        };
        let Some(bullet) = parse_bullet(trimmed) else {
            continue;
        };
        match current {
            Section::Decisions => sections.decisions.push(bullet),
            Section::NotYetSpecified => sections.not_yet_specified.push(bullet),
            Section::OutOfScope => sections.out_of_scope.push(bullet),
            Section::Unrecognized => sections.unrecognized.push(bullet),
        }
    }
    sections
}

/// Locates the reference-arrow marker in `after_marker`, accepting either
/// the Unicode arrow `→` (U+2192) or the ASCII digraph `->` as equivalent.
/// Returns the byte range `[start, end)` of whichever arrow occurs first
/// (the two are not expected to co-occur in one bullet, but the earliest
/// match wins if they do).
fn find_arrow(after_marker: &str) -> Option<(usize, usize)> {
    let unicode = after_marker.find('→').map(|i| (i, i + '→'.len_utf8()));
    let ascii = after_marker.find("->").map(|i| (i, i + 2));
    match (unicode, ascii) {
        (Some(u), Some(a)) => Some(if u.0 <= a.0 { u } else { a }),
        (Some(u), None) => Some(u),
        (None, Some(a)) => Some(a),
        (None, None) => None,
    }
}

/// Parses one bullet body (the text after `"- "`): `[marker] text` with an
/// optional `→ AK-nnn[, AK-nnn...]` / `-> AK-nnn[, AK-nnn...]` or
/// `→ context-only` / `-> context-only` trailer — the Unicode and ASCII
/// arrow spellings are equivalent. Returns `None` when the line does not
/// open with a bracketed marker (a non-conforming line is simply not a
/// tracked bullet, not a parse failure — this is fixture-grade markdown,
/// not a strict format).
fn parse_bullet(body: &str) -> Option<Bullet> {
    let rest = body.strip_prefix('[')?;
    let (marker_inner, after_marker) = rest.split_once(']')?;
    let marker = format!("[{marker_inner}]");
    let (ids, context_only) = match find_arrow(after_marker) {
        Some((_start, end)) => {
            let trailer = after_marker[end..].trim();
            if trailer == "context-only" {
                (Vec::new(), true)
            } else {
                let ids = trailer
                    .split(',')
                    .map(|entry| entry.trim().to_owned())
                    .filter(|entry| !entry.is_empty())
                    .collect();
                (ids, false)
            }
        }
        None => (Vec::new(), false),
    };
    Some(Bullet {
        marker,
        ids,
        context_only,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const CLEAN_ANSWER_KEY: &str = "## AK-101\nclaim: A checkable claim.\nverify_by: test\n\
        blast_radius: local\nauthority: critic\nevidence_required: {kind: test_report, \
        locator: suite-check-0001, producer: instrument}\n\n\
        ## AK-102\nclaim: Another checkable claim.\nverify_by: test\nblast_radius: local\n\
        authority: critic\nevidence_required: {kind: test_report, locator: \
        suite-check-0002, producer: instrument}\n";

    const CLEAN_MAP: &str = "## Decisions so far\n- [D1] A decision. → AK-101, AK-102\n\
        - [D2] A context-only decision. → context-only\n\n\
        ## Not yet specified\n- [F1] An open question.\n\n\
        ## Out of scope\n- [S1] Something excluded.\n";

    #[test]
    fn a_clean_pair_satisfies_every_invariant() {
        check_reference_invariants(CLEAN_MAP, CLEAN_ANSWER_KEY).expect("clean pair passes");
    }

    #[test]
    fn a_decision_without_a_unit_or_context_only_is_refused() {
        let map = "## Decisions so far\n- [D1] An unresolved decision.\n";
        let error = check_reference_invariants(map, CLEAN_ANSWER_KEY)
            .expect_err("a bare decision bullet fails");
        assert!(matches!(
            error,
            FormationError::DecisionWithoutUnit { marker } if marker == "[D1]"
        ));
    }

    #[test]
    fn fog_carrying_an_ak_reference_is_refused() {
        let map = "## Decisions so far\n- [D1] A decision. → AK-101, AK-102\n\n\
                    ## Not yet specified\n- [F1] Leaked. → AK-101\n";
        let error = check_reference_invariants(map, CLEAN_ANSWER_KEY).expect_err("fog leak fails");
        assert!(matches!(
            error,
            FormationError::FogPromoted { marker, ids } if marker == "[F1]" && ids == vec!["AK-101".to_owned()]
        ));
    }

    #[test]
    fn out_of_scope_carrying_an_ak_reference_is_refused() {
        let map = "## Decisions so far\n- [D1] A decision. → AK-101, AK-102\n\n\
                    ## Out of scope\n- [S1] Leaked. → AK-101\n";
        let error =
            check_reference_invariants(map, CLEAN_ANSWER_KEY).expect_err("out-of-scope leak fails");
        assert!(matches!(
            error,
            FormationError::OutOfScopePromoted { marker, ids } if marker == "[S1]" && ids == vec!["AK-101".to_owned()]
        ));
    }

    #[test]
    fn a_dangling_decision_reference_is_refused() {
        let map = "## Decisions so far\n- [D1] A decision. → AK-999\n";
        let error = check_reference_invariants(map, CLEAN_ANSWER_KEY)
            .expect_err("a reference to a nonexistent unit fails");
        assert!(matches!(
            error,
            FormationError::DanglingReference { marker, id } if marker == "[D1]" && id == "AK-999"
        ));
    }

    #[test]
    fn an_invalid_answer_key_is_refused() {
        let map = "## Decisions so far\n- [D1] A decision. → context-only\n";
        let error = check_reference_invariants(map, "## AK-101\nverify_by: vibes\n")
            .expect_err("an invalid answer key fails");
        assert!(matches!(error, FormationError::AnswerKeyInvalid { .. }));
    }

    // -- Correction round 1 regressions (falsifier round 1, Attack 2 / critic RES-02) --
    //
    // Both a fresh-context design critic and a fresh-context falsifier
    // independently found that the hand-rolled line parser above failed
    // OPEN on non-canonical but plausible `map.md` input: an ASCII `->`
    // arrow went unrecognized entirely, and a case-variant or
    // wrong-depth section heading silently misfiled a fog bullet as
    // belonging to the previous (Decisions) section. Each of the three
    // tests below reproduces one of those exact repro transcripts and
    // asserts on the specific `FormationError::FogPromoted` variant, the
    // same way `fog_carrying_an_ak_reference_is_refused` above does —
    // not just `is_err()`.

    #[test]
    fn fog_leak_via_ascii_arrow_is_refused() {
        // Same fog leak as `fog_carrying_an_ak_reference_is_refused`, but
        // spelled with the ASCII digraph `->` instead of the Unicode `→`.
        // Before the fix, `parse_bullet` only recognized `→`, so this
        // bullet parsed with zero ids and the leak went undetected
        // (`Ok(())`).
        let map = "## Decisions so far\n- [D1] A decision. → AK-101, AK-102\n\n\
                    ## Not yet specified\n- [F1] Leaked. -> AK-101\n";
        let error = check_reference_invariants(map, CLEAN_ANSWER_KEY)
            .expect_err("an ASCII-arrow fog leak fails");
        assert!(matches!(
            error,
            FormationError::FogPromoted { marker, ids } if marker == "[F1]" && ids == vec!["AK-101".to_owned()]
        ));
    }

    #[test]
    fn fog_leak_under_case_variant_heading_is_refused() {
        // The "Not yet specified" heading is written as
        // "## Not Yet Specified" (title case). Before the fix,
        // `section_of` matched only the exact-case string, so this
        // heading was unrecognized, `current` was reset to `None`, and
        // the fog bullet under it was collected into no section at all.
        let map = "## Decisions so far\n- [D1] A decision. → AK-101, AK-102\n\n\
                    ## Not Yet Specified\n- [F1] Leaked. → AK-101\n";
        let error = check_reference_invariants(map, CLEAN_ANSWER_KEY)
            .expect_err("a case-variant heading fog leak fails");
        assert!(matches!(
            error,
            FormationError::FogPromoted { marker, ids } if marker == "[F1]" && ids == vec!["AK-101".to_owned()]
        ));
    }

    #[test]
    fn fog_leak_under_deeper_heading_after_decisions_is_refused() {
        // `### Not yet specified` (three `#`s) immediately follows a
        // `## Decisions so far` section. Before the fix, the parser only
        // recognized exactly `## ` (two hashes), so this heading was not
        // treated as a section switch at all: the fog bullet under it
        // stayed filed under the still-current `Decisions` section and
        // was reported as a validated decision instead of a fog leak —
        // the falsifier's most severe finding.
        let map = "## Decisions so far\n- [D1] A decision. → AK-101, AK-102\n\n\
                    ### Not yet specified\n- [F1] Leaked. → AK-101\n";
        let error = check_reference_invariants(map, CLEAN_ANSWER_KEY)
            .expect_err("a deeper-heading fog leak fails");
        assert!(matches!(
            error,
            FormationError::FogPromoted { marker, ids } if marker == "[F1]" && ids == vec!["AK-101".to_owned()]
        ));
    }

    // -- Correction round 2 regressions (falsifier round 2) --
    //
    // A fresh, independent falsifier re-attacked the round-1-corrected
    // code and found the SAME underlying bug class survives via a
    // spelling round 1 didn't enumerate: a heading with a trailing colon
    // (`## Not yet specified:`, `### Out of scope:`). `section_of`
    // rejected the colon, `parse_sections` treated the unmatched heading
    // as "leave the current section unchanged," and the fog/out-of-scope
    // bullet under it got silently misfiled into whatever section
    // preceded it (typically Decisions), where a real `AK-` reference
    // passed as a validated decision. The round-2 fix removes the
    // permissive default itself: `section_of` now strips a trailing colon
    // before comparing, AND any heading that still doesn't match one of
    // the three canonical titles switches explicitly to a new
    // `Unrecognized` state (never "unchanged"), and a bullet carrying an
    // `AK-` reference while `Unrecognized` is refused as
    // `AmbiguousSectionReference`.

    #[test]
    fn fog_leak_under_colon_suffixed_heading_is_refused() {
        // "## Not yet specified:" (trailing colon). Before the round-2
        // fix, `section_of` rejected the colon, so this heading didn't
        // match, `current` stayed at `Decisions` (the round-1 "unchanged"
        // default), and the fog bullet under it was reported as a
        // validated decision instead of a fog leak. The round-2 fix
        // strips the trailing colon before comparing, so the heading is
        // recognized as `NotYetSpecified` directly — this test would
        // pass even without the `Unrecognized`-state redesign, purely
        // from the colon-stripping half of the fix.
        let map = "## Decisions so far\n- [D1] A decision. → AK-101, AK-102\n\n\
                    ## Not yet specified:\n- [F1] Leaked. → AK-101\n";
        let error = check_reference_invariants(map, CLEAN_ANSWER_KEY)
            .expect_err("a colon-suffixed heading fog leak fails");
        assert!(matches!(
            error,
            FormationError::FogPromoted { marker, ids } if marker == "[F1]" && ids == vec!["AK-101".to_owned()]
        ));
    }

    #[test]
    fn out_of_scope_leak_under_colon_suffixed_deeper_heading_is_refused() {
        // "### Out of scope:" combines the round-1 depth variant with the
        // round-2 trailing-colon variant, over the Out of scope section
        // specifically (falsifier round 2's attack #4).
        let map = "## Decisions so far\n- [D1] A decision. → AK-101, AK-102\n\n\
                    ### Out of scope:\n- [S1] Leaked. → AK-101\n";
        let error = check_reference_invariants(map, CLEAN_ANSWER_KEY)
            .expect_err("a colon-suffixed deeper out-of-scope heading leak fails");
        assert!(matches!(
            error,
            FormationError::OutOfScopePromoted { marker, ids } if marker == "[S1]" && ids == vec!["AK-101".to_owned()]
        ));
    }

    #[test]
    fn ak_reference_before_any_recognized_heading_is_refused() {
        // A bullet carrying a real `AK-` reference appears before any
        // recognized section heading has opened — the parser's initial
        // state is `Unrecognized` (round 1's `None` start state behaved
        // the same for this exact case: bullets under `None` were simply
        // dropped, i.e. also NOT accepted as validated decisions — but
        // silently dropping the reference is a different failure mode
        // than the misfiling gap this correction targets, so this test
        // pins the round-2 behavior explicitly: refuse it, with the
        // dedicated variant, don't just drop it).
        let map = "- [X1] A stray bullet before any heading. → AK-101\n\n\
                    ## Decisions so far\n- [D1] A decision. → AK-102\n";
        let error = check_reference_invariants(map, CLEAN_ANSWER_KEY)
            .expect_err("an AK- reference outside any recognized section fails");
        assert!(matches!(
            error,
            FormationError::AmbiguousSectionReference { marker, ids }
                if marker == "[X1]" && ids == vec!["AK-101".to_owned()]
        ));
    }

    #[test]
    fn unrecognized_heading_with_no_ak_reference_is_not_over_rejected() {
        // An unrelated "## Notes" section with plain prose (no `- [marker]`
        // bullets at all) between two recognized sections. The
        // `Unrecognized` state must not reject documents that simply have
        // extra, untracked content with nothing referencing the answer
        // key in it — only a bullet that actually carries an `AK-`
        // reference while section identity is ambiguous is the risk this
        // correction closes.
        let map = "## Decisions so far\n- [D1] A decision. → AK-101, AK-102\n\n\
                    ## Notes\nJust some prose here, not a tracked bullet at all.\n\n\
                    ## Not yet specified\n- [F1] An open question.\n\n\
                    ## Out of scope\n- [S1] Something excluded.\n";
        check_reference_invariants(map, CLEAN_ANSWER_KEY)
            .expect("an unrecognized heading with no AK- reference underneath is not rejected");
    }
}
