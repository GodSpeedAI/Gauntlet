# Falsifier round 3 — AK-065 `check_reference_invariants` (post correction-round-2)

Target: `crates/gauntlet-app/src/formation/mod.rs::check_reference_invariants`
(spec §9.2, ~line 616-646). Round 1 fixed ASCII arrow / case / depth variants.
Round 2 fixed the fail-open default ("unrecognized heading → keep the
previous section") by introducing `Section::Unrecognized` and
`FormationError::AmbiguousSectionReference`, refusing any `AK-` reference on
a bullet whose current section is not exactly `Decisions`.

This round attacks the round-2 architecture itself: does deny-by-default
actually cover `Decisions` correctly (no over-rejection), does the new state
machine have edge-case leaks, and does the mutation-teeth proof hold.

Baseline captured before probing:
`sha256sum crates/gauntlet-app/src/formation/mod.rs` =
`cb7203711cf394ba5aa883847d9e71bb798163087c2077f9eb93d7fd6289807f`. All
probes below were added as temporary `#[test]` functions inside
`formation::tests`, run, then removed; the file hash after cleanup matched
the baseline exactly (confirmed post-run). `.wayfinder/answer-key.md`
(md5 `06f1e0f90d355dee497b6459566e4e40`) and `.gauntlet/reference.lock`
(md5 `9b4840c611495b8d4d219ade015b918e`) were unchanged throughout.

## Attacks and results

### A1 — False positive check: lowercase + colon `Decisions` heading
Input: `## decisions so far:\n- [D1] A decision. → AK-101\n` against
`CLEAN_ANSWER_KEY`.
Result: `Ok(())`.
Verdict: correct. `section_of`'s trailing-colon-stripping + case-insensitive
match works for `Decisions` exactly as it does for the other two sections;
a legitimately-formed heading is not spuriously refused. No false positive.

### A2 — Substring-match heading: `## Not yet specified in this round`
Input: heading text is a superset of the canonical title (extra trailing
prose), with a real fog leak (`→ AK-101`) underneath.
Result: `Err(AmbiguousSectionReference { marker: "[F1]", ids: ["AK-101"] })`.
Verdict: correct, and importantly *safe in both directions*: `section_of`
uses `eq_ignore_ascii_case` (exact-length comparison), not a substring or
`starts_with` check, so `"Not yet specified in this round"` does NOT match
`"Not yet specified"`. It falls through to `Unrecognized`, and the leak is
still caught (via `AmbiguousSectionReference` rather than `FogPromoted`,
but still refused, not silently accepted, and not misassigned to the wrong
canonical section). No substring-match vulnerability.

### A3 / A3b — Re-entering `Decisions` after `NotYetSpecified`
Input: `Decisions → NotYetSpecified → Decisions` (second block has a fresh
valid decision) — passes (`Ok(())`). Then the same shape with a further
`NotYetSpecified` block after the second `Decisions` block, carrying a real
leak — correctly caught as `FogPromoted`.
Verdict: correct. `current` is a plain `Section` value reassigned on every
heading, not a stack or memoized state; there is no state leak across
repeated headings of the same canonical section.

### A4 — Heading-shaped bullet line: `# - [D2] weird heading-shaped line`
Input: a line starting with `#` whose remainder looks bullet-shaped.
Result: `heading_text` correctly classifies it as a heading first (any line
with a `#`-prefix-then-whitespace wins heading detection before bullet
detection is ever attempted), its text (`"- [D2] weird heading-shaped
line"`) doesn't match any canonical title, so `current` becomes
`Unrecognized`. The following real bullet `[D3] → AK-102` is then correctly
refused as `AmbiguousSectionReference`.
Verdict: correct, no confusion between heading and bullet detection; no
crash, well-defined precedence (heading wins).

### A5 / A5b — Malformed `AK-` id shape under `Decisions`: `AK-abc`, `AK123`
Result: both produce `Err(DanglingReference { marker: "[D1]", id: "AK-abc"
/ "AK123" })` — not a panic, not silent acceptance.
Verdict: **minor gap, pre-existing (not introduced by round 2).**
`parse_bullet` does not validate the `AK-NNN` shape at all — it splits
*anything* after the arrow that isn't literally `context-only` on commas
and treats each piece as an "id," regardless of whether it starts with
`AK-` or matches the Task 7 parser's strict `AK-NNN` grammar (confirmed
separately: `crates/gauntlet-app/src/reference/parse.rs` enforces the
strict shape and rejects `AK-8`, `[ab-008]`, etc. on the answer-key side).
The consequence here is cosmetic, not a security hole under `Decisions`:
a malformed reference still fails as `DanglingReference` (whatever the
"id" text is, it will not be found in `known_ids`), so the document is
still correctly refused — just with a less precise error variant/message
than a "malformed reference" would give. Not blocking.

### A6 — Non-`AK-`-shaped arrow trailer under `NotYetSpecified`: `→ see also F2`
Input: `## Not yet specified\n- [F1] TBD. → see also F2\n` (no real `AK-`
reference anywhere in the trailer, just prose after an arrow).
Result: `Err(FogPromoted { marker: "[F1]", ids: ["see also F2"] })`.
Verdict: **real finding — false-positive / over-rejection class, pre-existing
(not introduced by round 2), severity minor.** The same permissive
`parse_bullet` id-splitting from A5 means *any* arrow trailer other than the
literal string `context-only` is treated as if it were one or more `AK-`
ids, even when it plainly is not (no `AK-` prefix, no digits). A legitimate
`Not yet specified` / `Out of scope` bullet that uses `→` for some other
annotation (e.g. cross-referencing another bullet, or noting "resolved once
X happens") would be spuriously refused as a fog/out-of-scope leak, even
though nothing in the answer key is actually referenced. Checked the real
usage convention: `tests/fixtures/formation/map.md`'s `F1`/`S1` bullets
carry no arrow at all, so this is not exercised by any shipped fixture, and
the practical blast radius looks low — but it is a genuine precision gap in
the invariant enforcement, not merely a hypothetical. Recommend `parse_bullet`
require each split-on-comma entry to look like `AK-` (or better, `AK-\d+`)
before counting it as an id, distinct from the `context-only` sentinel.
This is NOT part of what round 2 touched (round 2 only changed `section_of`
colon-stripping and the `Section`/`Unrecognized` state machine) — flagging
it now because task 18's brief asked to probe it directly, not because it
regressed.

### A7 — Malformed but `AK-`-prefixed id under `NotYetSpecified`: `→ AK-abc`
Result: `Err(FogPromoted { marker: "[F1]", ids: ["AK-abc"] })`.
Verdict: correct / defensible. Unlike A6, this text does look like a
near-reference (`AK-` prefix), so treating it conservatively as a fog leak
is the safe, fail-closed choice, consistent with the deny-by-default spirit
of round 2. Not a bug.

### A8 — Whitespace-only `map_text`
Input: `"   \n\t\n  \n"` with `CLEAN_ANSWER_KEY`.
Result: `Ok(())`. No panic; no bullets/sections found, all per-bullet checks
vacuously pass.
Verdict: correct, sensible behavior.

### A9 / A9b — Empty `map_text` and/or empty `answer_key_text`
- Empty `map_text` + empty `answer_key_text`: `Ok(())` (an empty answer key
  parses to zero Units, which is schema-valid; an empty map has no bullets
  to check).
- Empty `map_text` + valid `CLEAN_ANSWER_KEY`: `Ok(())`.
Verdict: correct, no panic. Whether an empty answer key or empty map should
be *policy*-rejected elsewhere is out of this module's scope (§9.2 is a
per-bullet cross-check, not a coverage/completeness requirement), so this is
not a defect in `check_reference_invariants` itself.

### Re-verification of original (pre-round-2) scenarios
Ran the full existing suite (`just test-package gauntlet-app`, 409/409
green) which includes, unmodified: `a_dangling_decision_reference_is_refused`,
`an_invalid_answer_key_is_refused`, `a_decision_without_a_unit_or_context_only_is_refused`,
and the integration test `formation::fog_not_in_answer_key` (positive clean
pair + negative fog-leak pair via `tests/fixtures/formation/*.md`). Also
confirmed via `reference/parse.rs`'s own test suite (unmodified by round 2)
that duplicate-block detection (`a_duplicate_block_id_fails_the_later_block`)
and strict `AK-NNN` id-shape rejection on the answer-key side are still
intact — round 2 touched only `formation/mod.rs`'s `Section`/`section_of`/
`parse_sections`, not the shared `reference::validate`/`reference::parse`
path the dangling-reference and answer-key-validity checks depend on. No
regression found in any pre-round-2 scenario.

## Mutation-teeth proof

Target: the `AmbiguousSectionReference` construction in
`check_reference_invariants`'s loop over `bullets.unrecognized`.

Mutation applied: `if !bullet.ids.is_empty()` → `if false &&
!bullet.ids.is_empty()` (permanently disables the check).

Result with mutation applied: `cargo test -p gauntlet-app --lib
formation::tests` → **1 failed / 23 passed** — the shipped test
`ak_reference_before_any_recognized_heading_is_refused` went RED
(panicked expecting an `Err`, got `Ok(())`), exactly as expected. As a
side effect, two of this round's own probes (A2's substring-heading leak
and A4's heading-shaped-bullet-line leak) also silently passed under the
mutation, confirming they depend on the same code path — i.e. the shipped
suite's one red test is suficient proof that removing this check is
detected, and this round's new attacks would have gone undetected by the
mutation had they been shipped tests instead of probes (they don't need to
be, since the existing test already proves the code path).

Mutation reverted byte-for-byte (`if !bullet.ids.is_empty()` restored).
`cargo test -p gauntlet-app --lib formation::tests` → 13/13 GREEN
(the original file's own test count, probes already removed by this point).

## Cleanup verification

- All temporary probe `#[test]` functions removed from
  `crates/gauntlet-app/src/formation/mod.rs`.
- `sha256sum` of the file after cleanup matches the pre-probe baseline
  exactly: `cb7203711cf394ba5aa883847d9e71bb798163087c2077f9eb93d7fd6289807f`.
- `.wayfinder/answer-key.md` md5 unchanged: `06f1e0f90d355dee497b6459566e4e40`.
- `.gauntlet/reference.lock` md5 unchanged: `9b4840c611495b8d4d219ade015b918e`.
- No stray files created; `git status` shows only the pre-existing untracked
  `crates/gauntlet-app/src/formation/` directory (unchanged from session
  start), nothing else touched by this round.

## Final checks

- `just check-package gauntlet-app` → PASS (cargo check, all targets, 0
  warnings/errors).
- `just test-package gauntlet-app` → PASS, 409/409 tests.
- `just boundaries` → PASS, 228/228 tests.
- `just ruler` → PASS — answer key and manifest digests match the frozen
  lock.

## Verdict

**CONFIRM.**

9 attack classes run (A1–A9, several with sub-variants), 0 blocking
findings. Round 2's core architectural claim holds under adversarial
re-testing: `Section::Unrecognized` is a real, deny-by-default state (not a
"leave unchanged" fallback), `section_of`'s colon/case normalization does
not spuriously reject legitimate `Decisions so far` variants (no false
positive there), heading-vs-substring matching is exact-length (no
accidental canonical-section coercion via prose-suffixed headings),
heading-vs-bullet detection has a well-defined precedence with no crash
paths, repeated same-section headings do not leak state, and empty/
whitespace-only inputs are handled without panics. The mutation-teeth
re-proof confirms the shipped test suite (specifically
`ak_reference_before_any_recognized_heading_is_refused`) actually detects
removal of the `AmbiguousSectionReference` check, not just that tests exist
near it.

Two **non-blocking, pre-existing** findings surfaced (neither introduced by
round 2, both in code round 2 did not touch — `parse_bullet`'s id
extraction):
1. **Minor** — malformed `AK-`-shaped ids (`AK-abc`, `AK123` with no
   hyphen) under `Decisions` produce `DanglingReference` rather than a more
   precise "malformed reference" signal. Cosmetic; the document is still
   correctly refused.
2. **Minor, false-positive class** — any arrow (`→`/`->`) trailer under
   `NotYetSpecified`/`OutOfScope` that isn't the literal `context-only`
   marker is treated as if it carries `AK-` id(s) and triggers
   `FogPromoted`/`OutOfScopePromoted`, even when the trailer text doesn't
   look like an `AK-` reference at all (e.g. `→ see also F2`). Real fixture
   usage doesn't exercise this (fog/out-of-scope bullets there carry no
   arrow at all), so practical impact looks low, but it is a genuine
   over-rejection risk worth a follow-up correction (require each
   comma-split id candidate to match an `AK-` prefix before treating it as
   a reference, distinct from the `context-only` sentinel).

Neither finding lets a real fog/out-of-scope leak reach `Ok(())` undetected
— both are in the direction of the checker being *too* strict on
non-adversarial input, not too permissive on adversarial input. AK-065's
core invariant (no fog entry appears in the answer key) is not defeated by
any attack in this round.
