# Task 19 correction round 1 — progress

Status: DONE — all 5 required gates PASS

## Final summary (read this first)

Applied COR1 and COR2 from `.agents/reports/task19/correction-round1.md`
to exactly one file: `crates/gauntlet-app/src/skills/mod.rs`. No other
file was touched.

**COR1** — widened `CONTROL_LOGIC_PHRASES` (crates/gauntlet-app/src/skills/mod.rs)
from 5 to 12 entries by appending exactly the 7 phrases the correction
contract specified:
```
"settles the unit"
"authorize additional budget"
"authorize the budget"
"authorizes additional budget"
"elevate the unit's authority"
"elevates the unit's authority"
"grants the unit authority"
```
No bare words were added. Non-overlap with the legal-prose regression
sentence ("The runtime settles a Unit when its evidence is verified,
under its own authority model.") was verified by hand before landing (see
"Non-overlap verification" below) — none of the 7 new phrases are a
substring of that sentence, lowercased.

Added 3 new regression tests to `crates/gauntlet-app/src/skills/mod.rs`'s
existing `#[cfg(test)] mod tests` block (placed immediately after
`a_decisive_control_phrase_is_refused`, the existing control-logic-
specific test group in that file — chosen over `tests/skills.rs` because
that file's own three control-logic teeth tests
(`a_bare_control_word_in_ordinary_prose_is_not_over_rejected`,
`an_exact_control_identifier_is_refused`, `a_decisive_control_phrase_is_refused`)
already live in `mod.rs`, not in `tests/skills.rs`):
- `a_skill_claiming_to_settle_the_unit_itself_is_refused` — reproduces
  critic sentence 1 ("This skill settles the Unit once its evidence
  passes."), asserts `Err(SkillLintError::ControlLogicDetected { .. })`.
- `a_skill_instructing_budget_authorization_is_refused` — reproduces
  critic sentence 2 ("The agent should authorize additional budget for
  this unit."), same assertion.
- `a_skill_instructing_authority_elevation_is_refused` — reproduces
  critic sentence 3 ("Elevate the Unit's authority to cross_cutting
  before dispatching."), same assertion.

**COR2** — added a "## What this lint is not" section to the module doc
comment at the top of `crates/gauntlet-app/src/skills/mod.rs` (placed
after the existing "Never echo a secret" section, before the `use`
statements), mirroring the tone/shape of
`crates/gauntlet-arch-tests/src/single_writer.rs`'s "THREAT MODEL — what
this rule is not" section. States plainly that the control-logic scan is
a closed lexical exact-phrase/identifier match, not semantic
understanding; that a control-logic decision phrased another way still
evades it (cross-references RES-01 in critic-design.md); that a clean
`skills::lint` run is evidence of no KNOWN-pattern match, never proof of
no control logic in any phrasing; and that human/critic review remains
the backstop, cross-referencing `single_writer.rs`'s own section.

## Pass/fail status of every required command

| Command | Result |
|---|---|
| `just check-package gauntlet-app` | PASS (clean compile, 2m20s) |
| `just test-package gauntlet-app` | PASS — 433 tests run, 433 passed, 0 skipped, 0 failed. Includes `skills::tests::a_bare_control_word_in_ordinary_prose_is_not_over_rejected` PASS and all 3 new regression tests PASS. |
| `cargo nextest run -p gauntlet-app skills::lint` | PASS — 5 tests run, 5 passed (the same 5 the critic ran: `lint_rejects_control_logic_fixture`, `lint_rejects_secret_like_value`, `lint_rejects_unknown_front_matter_key`, `lint_rejects_absolute_path`, `lint_passes_every_installed_skill`) |
| `just boundaries` | PASS — 228 tests run, 228 passed, 0 skipped (includes `answer_key_single_writer` PASS) |
| `just ruler` | PASS — "answer key and manifest digests match the frozen lock" |

Also separately re-confirmed `a_bare_control_word_in_ordinary_prose_is_not_over_rejected`
still passes in isolation (`cargo nextest run -p gauntlet-app
a_bare_control_word_in_ordinary_prose_is_not_over_rejected` → 1 test run,
1 passed).

## Scope confirmation

`git status --porcelain -- crates/gauntlet-app/src/skills/mod.rs` shows
only that file modified (on top of its pre-existing staged Slice B
addition); `git diff --stat` for it shows +68/-0 lines, all additions
(the widened const, its new doc comment, the new module-doc section, and
the 3 new tests). No other file was touched by this correction round —
confirmed against the pre-existing `git status` (the five real `SKILL.md`
files, their `PROVENANCE.md` files, `lib.rs`, `tests/skills.rs`,
`single_writer.rs`, and the control-logic fixture were already
staged/modified from Task 19 Slice A/B before this correction round
began, and remain byte-identical to that prior state).

## Plan

- COR1: widen `CONTROL_LOGIC_PHRASES` in `crates/gauntlet-app/src/skills/mod.rs`
  with the 7 phrases listed in `correction-round1.md`, verify non-overlap
  with `a_bare_control_word_in_ordinary_prose_is_not_over_rejected`'s legal
  sentence, add 3 regression tests reproducing the critic's exact
  counterexample sentences.
- COR2: add "## What this lint is not" section to the module doc comment,
  mirroring `single_writer.rs`'s "THREAT MODEL" section.
- Run the 5 required gate commands in order.

## Edits made

1. `crates/gauntlet-app/src/skills/mod.rs` — widened `CONTROL_LOGIC_PHRASES`
   from 5 to 12 entries, added a doc comment on the const explaining the
   COR1 widening and its rationale (non-overlap with the legal-prose test).
2. `crates/gauntlet-app/src/skills/mod.rs` — added "## What this lint is
   not" section to the module doc comment (after "Never echo a secret",
   before the `use` statements).
3. `crates/gauntlet-app/src/skills/mod.rs` — added 3 regression tests to
   the existing `#[cfg(test)] mod tests` block, immediately after
   `a_decisive_control_phrase_is_refused` (the existing control-logic-
   specific test group in this file):
   - `a_skill_claiming_to_settle_the_unit_itself_is_refused`
   - `a_skill_instructing_budget_authorization_is_refused`
   - `a_skill_instructing_authority_elevation_is_refused`

No file outside `crates/gauntlet-app/src/skills/mod.rs` was touched.
`crates/gauntlet-app/tests/skills.rs` was read (to check the existing test
convention per the task contract) but not edited — the file's own
`#[cfg(test)]` block already held the three control-logic-specific teeth
tests (`a_bare_control_word_in_ordinary_prose_is_not_over_rejected`,
`an_exact_control_identifier_is_refused`, `a_decisive_control_phrase_is_refused`),
so the new regression tests follow that same in-file convention rather than
`tests/skills.rs`'s broader-lint-surface convention.

## Non-overlap verification (manual, before landing)

Legal-prose sentence (must stay clean):
"The runtime settles a Unit when its evidence is verified, under its own
authority model." → lowercased: "the runtime settles a unit when its
evidence is verified, under its own authority model."

Checked each of the 7 new phrases (lowercased) as a substring of the
lowercased legal sentence — none match:
- "settles the unit" — legal sentence has "settles a unit" (indefinite
  article), not "settles the unit". NO MATCH.
- "authorize additional budget" — NO MATCH (sentence has no "budget").
- "authorize the budget" — NO MATCH.
- "authorizes additional budget" — NO MATCH.
- "elevate the unit's authority" — NO MATCH.
- "elevates the unit's authority" — NO MATCH.
- "grants the unit authority" — NO MATCH.

Mapping from the three critic sentences to the new phrase that now catches
each (verified by substring inspection):
- "This skill settles the Unit once its evidence passes." → contains
  "settles the unit" (case-insensitive).
- "The agent should authorize additional budget for this unit." →
  contains "authorize additional budget".
- "Elevate the Unit's authority to cross_cutting before dispatching." →
  contains "elevate the unit's authority".

## Gate results

See "Pass/fail status of every required command" at the top of this file.
