# Task 18 — Correction round 1

Targeted fix to `crates/gauntlet-app/src/formation/mod.rs`'s `check_reference_invariants`
(AK-065 fog/out-of-scope invariant enforcer), addressing the single defect class both the
design-as-built critic (RES-02) and the round-1 falsifier (Attack 2) independently converged
on: the hand-rolled `map.md` line parser failed OPEN on non-canonical but plausible input.

## What changed

File touched: `crates/gauntlet-app/src/formation/mod.rs` only (no other file touched, per
scope).

### 1. Section heading recognition (was: exact `"## "` prefix + exact-case string match)

- Added `heading_text(line: &str) -> Option<&str>`: strips any number (>=1) of leading `#`
  characters followed by whitespace, returning the trimmed remainder. A line with no leading
  `#` returns `None` (not heading-shaped at all).
- Rewrote `section_of` to compare the heading text against the three canonical titles
  (`"Decisions so far"`, `"Not yet specified"`, `"Out of scope"`) with
  `str::eq_ignore_ascii_case` instead of an exact `match`.
- Rewrote the heading branch in `parse_sections`: it now calls `heading_text`, and only
  reassigns `current` when `section_of` returns `Some(_)`. A heading-shaped line that matches
  none of the three (e.g. the document's own `# Wayfinder Map — <title>` top title) now
  **leaves `current` unchanged** instead of resetting it to `None` — this was itself a latent
  bug in the old code (any unrecognized `"## "` heading silently closed whatever section was
  open), not something the task asked to fix directly, but it falls directly out of doing the
  heading match correctly and was called for by "leave the current section unchanged exactly
  as before" in the task brief.

### 2. Reference-arrow recognition (was: only the Unicode `→`)

- Added `find_arrow(after_marker: &str) -> Option<(usize, usize)>`: returns the byte range of
  whichever of `→` (U+2192) or the ASCII digraph `->` occurs first in the string (the two are
  not expected to co-occur in one bullet, but the earliest match wins if they do).
- `parse_bullet` now calls `find_arrow` instead of `after_marker.split_once('→')`; the trailer
  slice starts right after the matched arrow's byte range. The id-extraction/`context-only`
  logic downstream is byte-for-byte unchanged — only what counts as "the arrow" changed.

### 3. Regression tests

Added three tests to the existing `#[cfg(test)] mod tests` block in the same file (this
crate's convention: every other formation test already lives inline in that module, e.g.
`fog_carrying_an_ak_reference_is_refused`; no `tests/formation.rs` exists or was needed):

- `fog_leak_via_ascii_arrow_is_refused`
- `fog_leak_under_case_variant_heading_is_refused`
- `fog_leak_under_deeper_heading_after_decisions_is_refused`

Each asserts on the specific `FormationError::FogPromoted { marker, ids }` variant (marker
`"[F1]"`, ids `["AK-101"]`), the same style as the pre-existing
`fog_carrying_an_ak_reference_is_refused`, not a bare `is_err()`.

## Exact before/after behavior per scenario

All three repro inputs below reference a real `AK-101` id from `CLEAN_ANSWER_KEY` from a
`Not yet specified` (fog) bullet — the invariant says this must be `Err(FogPromoted)`.

1. **ASCII arrow (`fog_leak_via_ascii_arrow_is_refused`)**
   Input: `"## Not yet specified\n- [F1] Leaked. -> AK-101\n"`.
   - Before: `parse_bullet` only recognized `→`; `after_marker.split_once('→')` returned
     `None`, so the bullet parsed with `ids: []`. The fog-check loop found nothing to flag.
     `check_reference_invariants` returned `Ok(())` — the leak went completely undetected.
   - After: `find_arrow` matches the `->` digraph; the bullet parses with
     `ids: ["AK-101"]`. `check_reference_invariants` returns
     `Err(FormationError::FogPromoted { marker: "[F1]", ids: ["AK-101"] })`.

2. **Case-variant heading (`fog_leak_under_case_variant_heading_is_refused`)**
   Input: `"## Decisions so far\n...\n\n## Not Yet Specified\n- [F1] Leaked. → AK-101\n"`.
   - Before: `section_of` matched only the exact-case string `"Not yet specified"`. The
     title-case heading matched none of the three arms, so `section_of` returned `None`, and
     the old `current = section_of(heading)` assignment reset `current` to `None`. The `[F1]`
     bullet was then collected into neither `not_yet_specified` nor `decisions` (the `None`
     arm in the `match current` is a no-op), so it was dropped from every check entirely.
     `Ok(())`.
   - After: `section_of` does a case-insensitive compare, matches `"Not Yet Specified"`
     against `NotYetSpecified`, and `current` is updated. The bullet is collected into
     `not_yet_specified`, and the fog-check loop returns
     `Err(FormationError::FogPromoted { marker: "[F1]", ids: ["AK-101"] })`.

3. **Deeper heading after a Decisions section (`fog_leak_under_deeper_heading_after_decisions_is_refused`)**
   — the falsifier's most severe finding.
   Input: `"## Decisions so far\n...\n\n### Not yet specified\n- [F1] Leaked. → AK-101\n"`.
   - Before: the heading branch matched only the literal `"## "` (exactly two hashes).
     `"### Not yet specified"` does not start with `"## "`, so the branch was skipped
     entirely; the line fell through to the bullet-parse check, which also didn't match (it
     doesn't start with `"- "`), so it was silently ignored as untracked content. `current`
     stayed `Some(Decisions)` from the prior section. The `[F1]` bullet that followed was
     then collected into `decisions`, not `not_yet_specified` — it passed the
     `DecisionWithoutUnit` check (it *does* carry an `AK-` id) and the dangling-reference
     check (`AK-101` is real), so the tool reported it as a **validated decision** instead of
     flagging a live fog leak. `Ok(())`.
   - After: `heading_text` strips any number of leading `#`s (three, here) plus the
     following whitespace, yielding `"Not yet specified"`; `section_of` matches it
     regardless of depth, so `current` switches to `NotYetSpecified` before the `[F1]` bullet
     is parsed. The bullet is correctly collected into `not_yet_specified`, and
     `check_reference_invariants` returns
     `Err(FormationError::FogPromoted { marker: "[F1]", ids: ["AK-101"] })`.

## RED → GREEN proof (not just narrative)

To make sure the three new tests are meaningful (not vacuously passing), the two fix hunks
were temporarily reverted in place (keeping the new tests), the suite was re-run to observe
RED, then the fix was restored from a scratch backup and re-verified GREEN.

RED (pre-fix code, new tests only):

```
cargo nextest run -p gauntlet-app formation::tests::fog_leak
...
FAIL [0.006s] (1/3) gauntlet-app formation::tests::fog_leak_under_case_variant_heading_is_refused
FAIL [0.005s] (2/3) gauntlet-app formation::tests::fog_leak_under_deeper_heading_after_decisions_is_refused
FAIL [0.006s] (3/3) gauntlet-app formation::tests::fog_leak_via_ascii_arrow_is_refused
Summary [0.010s] 3 tests run: 0 passed, 3 failed, 402 skipped
```

Each failure was the expected one: `.expect_err(...)` panicking because
`check_reference_invariants` returned `Ok(())` (e.g.
`panicked at .../formation/mod.rs:341:14: a case-variant heading fog leak fails: ()`).

GREEN (fix restored):

```
cargo nextest run -p gauntlet-app formation::
...
PASS [0.006s] (1/9) gauntlet-app formation::tests::fog_carrying_an_ak_reference_is_refused
PASS [0.006s] (2/9) gauntlet-app formation::tests::a_dangling_decision_reference_is_refused
PASS [0.006s] (3/9) gauntlet-app formation::tests::an_invalid_answer_key_is_refused
PASS [0.006s] (4/9) gauntlet-app formation::tests::fog_leak_under_case_variant_heading_is_refused
PASS [0.006s] (5/9) gauntlet-app formation::tests::a_clean_pair_satisfies_every_invariant
PASS [0.006s] (6/9) gauntlet-app formation::tests::a_decision_without_a_unit_or_context_only_is_refused
PASS [0.004s] (7/9) gauntlet-app formation::tests::out_of_scope_carrying_an_ak_reference_is_refused
PASS [0.005s] (8/9) gauntlet-app formation::tests::fog_leak_under_deeper_heading_after_decisions_is_refused
PASS [0.005s] (9/9) gauntlet-app formation::tests::fog_leak_via_ascii_arrow_is_refused
Summary [0.012s] 9 tests run: 9 passed, 396 skipped
```

## Full verification (final, post-restore)

```
$ just check-package gauntlet-app
cargo check -p gauntlet-app --all-targets
    Checking gauntlet-app v0.1.0 (/home/sprime01/projects/gauntlet/crates/gauntlet-app)
    Finished `dev` profile [unoptimized] target(s) in 2.65s

$ just test-package gauntlet-app
...
Summary [0.562s] 405 tests run: 405 passed, 0 skipped

$ just lint
cargo clippy --workspace --all-targets -- -D warnings
    Checking gauntlet-app v0.1.0 (...)
    Checking gauntlet-cli v0.1.0 (...)
    Finished `dev` profile [unoptimized] target(s) in 5.71s
cargo fmt --check
(clean — no output)

$ just boundaries
...
Summary [18.778s] 228 tests run: 228 passed, 0 skipped

$ just ruler
./scripts/verify-ruler.sh
verify-ruler.sh: PASS — answer key and manifest digests match the frozen lock
(/home/sprime01/projects/gauntlet/.gauntlet/reference.lock)
```

Targeted re-runs also confirmed:

```
$ cargo nextest run -p gauntlet-app fog_not_in_answer_key
PASS [0.003s] (1/1) gauntlet-app::formation fog_not_in_answer_key
Summary [0.004s] 1 test run: 1 passed, 404 skipped

$ cargo nextest run -p gauntlet-app formation::tests::fog_leak
(after restoring the fix — see GREEN block above; all 3 pass)
```

## Out of scope, untouched (confirmed)

- The bullet-marker whitespace-strictness gap (critic RES-02) — not touched; only the arrow
  widening inside `parse_bullet` was changed, the `- ` bullet-marker detection in
  `parse_sections` (`line.trim_start().strip_prefix("- ")`) is unmodified.
- Nextest filter-syntax convention — not touched, not relevant to this file.
- `.wayfinder/answer-key.md`, `.gauntlet/reference.lock`, `tests/fixtures/formation/**` — not
  touched. `just ruler` confirms the frozen lock still matches.
- No file other than `crates/gauntlet-app/src/formation/mod.rs` was modified.

## Summary

Widened `formation::mod.rs`'s section-heading and reference-arrow recognition to be
depth/case-robust and ASCII/Unicode-arrow-equivalent, added three regression tests
reproducing the falsifier's and critic's exact repro transcripts (each independently proven
RED against the pre-fix code and GREEN after), and confirmed all required gates
(`check-package`, `test-package`, `lint`, `boundaries`, `ruler`) green with no other file
touched.
