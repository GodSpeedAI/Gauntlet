# Task 18 — Correction round 2

Second correction to `crates/gauntlet-app/src/formation/mod.rs`'s `check_reference_invariants`
(AK-065 fog/out-of-scope invariant enforcer). A fresh, independent falsifier round 2
(`.agents/reports/task18/falsifier-round2.md`) re-attacked the round-1-corrected code and
found the **same underlying bug class survives**, via a heading spelling round 1 didn't
enumerate: a trailing colon (`## Not yet specified:`, `### Out of scope:`).

## Diagnosis (falsifier's, treated as correct)

Round 1 widened `section_of`/`heading_text` to recognize more heading *spellings*
(case-insensitive, any depth, ASCII or Unicode arrow) — but it left the underlying *default*
in place: an unmatched heading meant "leave the current section unchanged," not "unknown
section." A trailing colon wasn't in round 1's allowlist, so `## Not yet specified:` still
didn't match, `current` stayed at whatever section preceded it (typically `Decisions`), and
the fog/out-of-scope bullet under it was silently re-filed as an accepted decision. Growing
the allowlist (handling `:` today) just produces round 3 against the next unenumerated
variant (a parenthetical, bold markdown, an em-dash, ...). **The permissive default itself was
the bug**, not the specific spellings missing from the allowlist.

## What changed

File touched: `crates/gauntlet-app/src/formation/mod.rs` only (no other file touched, per
scope — same as round 1).

### 1. `section_of`: also strips a trailing colon before comparing

`section_of` now normalizes with `heading.trim().trim_end_matches(':').trim_end()` instead of
just `heading.trim()`, so `"Not yet specified:"` and `"Not yet specified :"` both match
`"Not yet specified"`. This closes the two literal repro cases from the falsifier's report by
itself, but does **not** close the deeper bug (see below) — it's the narrower, allowlist-style
half of the fix, kept because it's a real, cheap, non-adversarial spelling to accept.

### 2. The core policy flip: `Section` gains an explicit `Unrecognized` state; the default for
   "no match" is now always `Unrecognized`, never "unchanged"

- `Section` gained a fourth variant, `Unrecognized`, doc-commented as *not* a "leave it as
  whatever it was" fallback but a real, distinct state a heading switches **into**, the same
  way it switches into any of the other three.
- `parse_sections`'s parser state changed from `current: Option<Section>` (with `None` as the
  unset/dropped state) to `current: Section` (starting at `Section::Unrecognized`).
- The heading branch changed from:
  ```rust
  if let Some(section) = section_of(heading) { current = Some(section); }
  ```
  (leaves `current` alone on no match) to:
  ```rust
  current = section_of(heading).unwrap_or(Section::Unrecognized);
  ```
  (always reassigns; no match explicitly closes whatever section was open).
- `Sections` gained an `unrecognized: Vec<Bullet>` bucket, populated the same way
  `decisions`/`not_yet_specified`/`out_of_scope` are, instead of round 1's `None => {}`
  (silently dropping) arm.
- `check_reference_invariants` gained a fourth check loop, parallel to the existing
  fog/out-of-scope loops: any bullet in `bullets.unrecognized` carrying a non-empty `ids` list
  is refused with the new `FormationError::AmbiguousSectionReference { marker, ids }` variant
  (added to the enum + its `Display` impl). A bullet with no reference, or the literal
  `context-only` marker, in `Unrecognized` state is fine — only a bullet that actually carries
  an `AK-` reference while section identity is ambiguous is refused.

This removes the permissive default entirely: every heading-shaped line now switches `current`
to *something* — one of the three canonical sections, or `Unrecognized` — and every bullet that
carries an `AK-` reference is checked against whichever of those four states is current. There
is no remaining code path where an unrecognized heading can silently preserve a stale section
identity for a bullet that follows it.

### 3. Regression tests

Added four tests to the existing `#[cfg(test)] mod tests` block in the same file (matching
round 1's and the file's own convention — no `tests/formation.rs` needed, since
`crates/gauntlet-app/tests/formation.rs` only carries the fixture-file integration test
`fog_not_in_answer_key`, unrelated to this module's own unit tests):

- `fog_leak_under_colon_suffixed_heading_is_refused` — `## Not yet specified:` with a fog
  bullet referencing `AK-101` → `Err(FogPromoted { marker: "[F1]", ids: ["AK-101"] })`.
- `out_of_scope_leak_under_colon_suffixed_deeper_heading_is_refused` — `### Out of scope:`
  (colon + depth combined, falsifier's attack #4) with an out-of-scope bullet referencing
  `AK-101` → `Err(OutOfScopePromoted { marker: "[S1]", ids: ["AK-101"] })`.
- `ak_reference_before_any_recognized_heading_is_refused` — a bullet carrying a real `AK-`
  reference appears before any recognized section heading (still in the initial
  `Unrecognized` state) → `Err(AmbiguousSectionReference { marker: "[X1]", ids: ["AK-101"] })`.
- `unrecognized_heading_with_no_ak_reference_is_not_over_rejected` — an unrelated `## Notes`
  section with plain prose (no bullets at all) between `Decisions so far` and `Not yet
  specified` → `Ok(())`, confirming the redesign doesn't over-reject documents that simply
  carry extra untracked content with nothing referencing the answer key in it.

Round 1's three regressions (`fog_leak_via_ascii_arrow_is_refused`,
`fog_leak_under_case_variant_heading_is_refused`,
`fog_leak_under_deeper_heading_after_decisions_is_refused`) were re-run unchanged and still
pass — see verification below.

## Does the policy flip change behavior for the shipped fixtures / existing tests?

No. Checked deliberately, per the task brief's explicit instruction to verify this rather than
assume it:

- `tests/fixtures/formation/map.md` and `map-fog-leak.md` both open directly with
  `## Decisions so far` as their first line — there is no title line or any other content
  before the first recognized heading, so no bullet is ever collected while `current` is
  `Unrecognized` in either fixture. Confirmed via `just test-package gauntlet-app`: the
  fixture-driven integration test `gauntlet-app::formation fog_not_in_answer_key` still passes
  (see below).
- Every existing inline unit test's `map` string (`CLEAN_MAP`, and every ad hoc `map` literal
  in `a_decision_without_a_unit_or_context_only_is_refused`,
  `fog_carrying_an_ak_reference_is_refused`, `out_of_scope_carrying_an_ak_reference_is_refused`,
  `a_dangling_decision_reference_is_refused`, `an_invalid_answer_key_is_refused`, and all three
  round-1 regressions) opens directly with `## Decisions so far` as well — same reasoning, no
  behavior change.
- No existing test's expected outcome needed to change. The policy flip is additive: it only
  produces a *new* refusal (`AmbiguousSectionReference`) for a bullet class (AK- reference
  under an unrecognized/initial section) that every existing fixture and test simply never
  exercises.

## RED → GREEN proof

Per the round-1 precedent, the fix was temporarily reverted in place (keeping the four new
tests and the new `FormationError::AmbiguousSectionReference` variant so the file still
compiles), to confirm the new tests fail for the expected reason before restoring the fix.

Reverted for the RED probe (marked `TEMP-REVERT(round2-RED-proof)` inline, never committed):

- `section_of`: dropped the `.trim_end_matches(':').trim_end()` normalization (back to plain
  `.trim()`).
- `parse_sections`: back to `current: Option<Section>`, the heading branch only reassigning
  `current` on a match (leaves it unchanged otherwise), and the match arm collapsing
  `Some(Section::Unrecognized) | None => {}` (dropped, not tracked).
- `check_reference_invariants`: the `bullets.unrecognized` check loop commented out.

RED (pre-fix code, full `formation::` test run, `--no-fail-fast`):

```
$ cargo nextest run -p gauntlet-app formation:: --no-fail-fast
...
        PASS [   0.007s] ( 4/13) gauntlet-app formation::tests::a_dangling_decision_reference_is_refused
        PASS [   0.008s] ( 5/13) gauntlet-app formation::tests::fog_carrying_an_ak_reference_is_refused
        FAIL [   0.009s] ( 6/13) gauntlet-app formation::tests::ak_reference_before_any_recognized_heading_is_refused
    thread '...' panicked at crates/gauntlet-app/src/formation/mod.rs:529:14:
    an AK- reference outside any recognized section fails: ()
        PASS [   0.006s] ( 7/13) gauntlet-app formation::tests::fog_leak_under_case_variant_heading_is_refused
        PASS [   0.005s] ( 8/13) gauntlet-app formation::tests::fog_leak_via_ascii_arrow_is_refused
        FAIL [   0.006s] ( 9/13) gauntlet-app formation::tests::fog_leak_under_colon_suffixed_heading_is_refused
    thread '...' panicked at crates/gauntlet-app/src/formation/mod.rs:493:14:
    a colon-suffixed heading fog leak fails: ()
        PASS [   0.006s] (10/13) gauntlet-app formation::tests::out_of_scope_carrying_an_ak_reference_is_refused
        PASS [   0.006s] (11/13) gauntlet-app formation::tests::fog_leak_under_deeper_heading_after_decisions_is_refused
        FAIL [   0.005s] (12/13) gauntlet-app formation::tests::out_of_scope_leak_under_colon_suffixed_deeper_heading_is_refused
    thread '...' panicked at crates/gauntlet-app/src/formation/mod.rs:508:14:
    a colon-suffixed deeper out-of-scope heading leak fails: ()
        PASS [   0.004s] (13/13) gauntlet-app formation::tests::unrecognized_heading_with_no_ak_reference_is_not_over_rejected
────────────
     Summary [   0.016s] 13 tests run: 10 passed, 3 failed, 396 skipped
```

Exactly the three tests whose scenario depends on the new fix failed, each for the expected
reason (`.expect_err(...)` panicking on `()` because `check_reference_invariants` returned
`Ok(())`): the two colon-suffixed-heading tests, and the before-any-heading
`AmbiguousSectionReference` test. All ten other tests — round 1's three regressions, the
pre-existing five, and the fourth new "don't over-reject" test — passed unchanged, confirming
the revert was scoped correctly and the new tests aren't vacuous.

GREEN (fix restored from a byte-for-byte backup taken before the revert; `diff` against the
backup was empty, confirmed restore was exact):

```
$ cargo nextest run -p gauntlet-app formation::
...
        PASS [   0.005s] ( 1/13) gauntlet-app formation::tests::a_clean_pair_satisfies_every_invariant
        PASS [   0.005s] ( 2/13) gauntlet-app formation::tests::an_invalid_answer_key_is_refused
        PASS [   0.005s] ( 3/13) gauntlet-app formation::tests::a_decision_without_a_unit_or_context_only_is_refused
        PASS [   0.006s] ( 4/13) gauntlet-app formation::tests::ak_reference_before_any_recognized_heading_is_refused
        PASS [   0.006s] ( 5/13) gauntlet-app formation::tests::fog_carrying_an_ak_reference_is_refused
        PASS [   0.008s] ( 6/13) gauntlet-app formation::tests::a_dangling_decision_reference_is_refused
        PASS [   0.005s] ( 7/13) gauntlet-app formation::tests::fog_leak_under_colon_suffixed_heading_is_refused
        PASS [   0.006s] ( 8/13) gauntlet-app formation::tests::fog_leak_under_case_variant_heading_is_refused
        PASS [   0.006s] ( 9/13) gauntlet-app formation::tests::fog_leak_under_deeper_heading_after_decisions_is_refused
        PASS [   0.005s] (10/13) gauntlet-app formation::tests::out_of_scope_carrying_an_ak_reference_is_refused
        PASS [   0.005s] (11/13) gauntlet-app formation::tests::fog_leak_via_ascii_arrow_is_refused
        PASS [   0.005s] (12/13) gauntlet-app formation::tests::out_of_scope_leak_under_colon_suffixed_deeper_heading_is_refused
        PASS [   0.004s] (13/13) gauntlet-app formation::tests::unrecognized_heading_with_no_ak_reference_is_not_over_rejected
────────────
     Summary [   0.015s] 13 tests run: 13 passed, 396 skipped
```

## Full verification (final, post-restore)

```
$ just check-package gauntlet-app
cargo check -p gauntlet-app --all-targets
    Checking gauntlet-app v0.1.0 (/home/sprime01/projects/gauntlet/crates/gauntlet-app)
    Finished `dev` profile [unoptimized] target(s) in 2.73s

$ just test-package gauntlet-app
...
Summary [0.474s] 409 tests run: 409 passed, 0 skipped
(405 round-1 baseline + 4 new round-2 tests)

$ just lint
cargo clippy --workspace --all-targets -- -D warnings
    Checking gauntlet-app v0.1.0 (...)
    Checking gauntlet-cli v0.1.0 (...)
    Finished `dev` profile [unoptimized] target(s) in 5.64s
cargo fmt --check
(clean — no output)

$ just boundaries
...
Summary [19.351s] 228 tests run: 228 passed, 0 skipped

$ just ruler
./scripts/verify-ruler.sh
verify-ruler.sh: PASS — answer key and manifest digests match the frozen lock
(/home/sprime01/projects/gauntlet/.gauntlet/reference.lock)
```

Targeted fixture-integration re-run also confirmed:

```
$ cargo nextest run -p gauntlet-app fog_not_in_answer_key
PASS gauntlet-app::formation fog_not_in_answer_key
```

## Frozen paths — untouched (confirmed)

```
$ git status --porcelain -- .wayfinder/answer-key.md .gauntlet/reference.lock
(empty)
```

Both paths show no changes across the whole session. `just ruler` independently confirms the
frozen lock still matches.

## Out of scope, untouched (confirmed)

- The bullet-marker whitespace-strictness gap (a separate, already-recorded residual) — not
  touched; `line.trim_start().strip_prefix("- ")` in `parse_sections` is unmodified.
- No fixture file touched (`tests/fixtures/formation/**` unchanged).
- No file other than `crates/gauntlet-app/src/formation/mod.rs` was modified.
- The falsifier's "bonus finding" (leading whitespace before `#`, e.g. `" ## Not yet
  specified"`) was explicitly out of the assigned repro list and the falsifier itself weighted
  it out of the NOT_CONFIRM verdict (the module's own doc comment scopes it to "fixture-grade
  markdown, not CommonMark," and every shipped fixture and skill template is flush-left) — not
  addressed here; left as a candidate for a future round if it recurs against real input.

## Summary

Replaced the permissive "unrecognized heading → leave current section unchanged" default in
`formation::mod.rs`'s `parse_sections` with an explicit fourth `Section::Unrecognized` state
that every non-matching heading switches into, and added a parallel check
(`FormationError::AmbiguousSectionReference`) refusing any bullet that carries an `AK-`
reference while that state is current — closing the whole bug class the falsifier identified,
not just the specific trailing-colon spelling it found, while also keeping the narrower
colon-stripping normalization in `section_of` as a legitimate accepted spelling. Added four
regression tests (RED→GREEN proven against a byte-for-byte-restored file), confirmed round 1's
three regressions and every pre-existing test still pass unchanged, verified the policy flip
does not alter behavior for the shipped canonical fixtures or any existing test (all open
directly with `## Decisions so far`, so none are ever in the initial `Unrecognized` state when
a bullet appears), and confirmed all required gates (`check-package`, `test-package`, `lint`,
`boundaries`, `ruler`) green with no other file touched and both frozen paths
(`.wayfinder/answer-key.md`, `.gauntlet/reference.lock`) unchanged.
