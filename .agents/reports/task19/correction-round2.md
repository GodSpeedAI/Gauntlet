# Task 19 correction round 2 (COR3 + COR4)

**Trigger:** fresh blind EXECUTING falsifier round 1 NOT_CONFIRM (1 major,
1 minor) — see `.agents/reports/task19/falsifier-round1.md`. Full findings
FALS-01 (major) and FALS-02 (minor) there; read them in full before editing.

## COR3 (major, FALS-01 Part B) — control-logic phrase match needs a
## runtime-exemption, not another phrase addition

**Root cause (do not paper over this — fix the mechanism):**
`CONTROL_LOGIC_PHRASES` is matched as a bare, subject-blind, case-insensitive
substring (`scan_control_logic` in `crates/gauntlet-app/src/skills/mod.rs`).
This means ANY phrase in the list — not just COR1's three new additions —
can in principle also match legitimate third-person descriptive prose about
what *the runtime* does, which is the module's own sanctioned voice (see the
passing precedent test `a_bare_control_word_in_ordinary_prose_is_not_over_rejected`
and `gauntlet-run/SKILL.md`'s entire required style per IN-FLIGHT.md: "every
such decision is a sentence like 'the runtime settles a Unit when X'...
never 'settle the unit when X'"). The falsifier demonstrated this concretely
for three of COR1's additions; treat it as a property of the mechanism, not
of those three strings specifically.

**Fix:** add a runtime-exemption to `scan_control_logic` for
`CONTROL_LOGIC_PHRASES` matches (NOT for `CONTROL_LOGIC_IDENTIFIERS` —
those are exact Rust API names, a much narrower true-positive signal, leave
them matched unconditionally). When a phrase match is found on a line,
check whether the substring `"the runtime"` (case-insensitive) appears
earlier on that SAME line, before the match's start position. If it does,
treat the match as sanctioned descriptive prose and do not flag it (continue
scanning — other patterns on the same line, or the same pattern elsewhere,
are still checked normally). Implement this by finding the match's byte
position (not just `.contains()`), not by any regex crate — stay within the
existing hand-rolled scanner style (no new dependency).

This is a bounded, documented exemption — it will not catch a deliberately
adversarial sentence like "This skill, not the runtime, settles the unit"
(that remains the already-disclosed RES-01/"what this lint is not"
residual; do not chase it further, do not add subject-tracking NLP). Update
the "## What this lint is not" module-doc section to mention the runtime-
exemption explicitly: it is a per-line textual proximity check, not
grammatical subject parsing, and can itself be defeated by a sentence that
mentions "the runtime" elsewhere for unrelated reasons while still making a
skill-authored decision — record this honestly rather than overclaiming.

**Required regression tests** (add to `skills/mod.rs`'s own `#[cfg(test)]`
block, next to COR1's tests):
1. Each of the falsifier's three FALS-01 Part B sentences (copy verbatim
   from `falsifier-round1.md`) must now lint CLEAN (`Ok(...)`):
   - `"The runtime settles the unit once every blocking residual is resolved and evidence is verified."`
   - `"When the recovery reserve is untouched, the runtime authorizes additional budget for the next round automatically."`
   - `"During dispatch the runtime grants the unit authority consistent with its declared blast radius."`
2. Confirm ALL existing tests still pass, especially the three COR1
   regression tests added in round 1 (`a_skill_claiming_to_settle_the_unit_itself_is_refused`,
   `a_skill_instructing_budget_authorization_is_refused`,
   `a_skill_instructing_authority_elevation_is_refused`) and the original
   `a_bare_control_word_in_ordinary_prose_is_not_over_rejected` — these
   must still correctly REJECT/lint-clean exactly as before; the exemption
   must not accidentally widen coverage back to nothing. Add one more
   explicit test that a violation sentence WITHOUT "the runtime" nearby
   (e.g. re-use the critic's original `"This skill settles the Unit once
   its evidence passes."`) still correctly rejects even after the
   exemption is added — this is the key discriminator proving the fix is
   the mechanism, not a regression of COR1's closure.

## COR4 (minor, FALS-02) — secret-label compound-word boundary bypass

**Locus:** `label_assignment_is_secret` in `crates/gauntlet-app/src/skills/mod.rs`.

**Root cause:** the left-boundary check currently treats `_` the same as an
alphanumeric character (both block a match), which correctly avoids
`"monkey:"` matching label `"key"` but ALSO blocks the common compound
convention `api_key:` / `secret_key:` / `auth_token:` / `client_secret:`
from ever matching any of the four `SECRET_LABELS`.

**Fix:** change the left-boundary condition so only an alphanumeric
character immediately before the label counts as a boundary violation;
`_` no longer counts as one. Concretely: change
`!(bytes[start - 1].is_ascii_alphanumeric() || bytes[start - 1] == b'_')`
to `!bytes[start - 1].is_ascii_alphanumeric()`. Verify by hand this still
rejects `"monkey:"` as a `"key"` match (character before "key" is `n`,
alphanumeric — still correctly excluded) while now accepting `"api_key:"`
(character before "key" is `_` — now correctly a hit).

**Required regression tests:** the four FALS-02 reproduction cases from
`falsifier-round1.md` (`api_key:`, `secret_key:`, `auth_token:`,
`client_secret:`, each with a non-`$`-prefixed value) must now all lint as
`Err(SkillLintError::SecretLikeValueDetected { .. })`. Also keep/re-confirm
the existing `"monkey:"`-style non-match test (if one exists; add it if not)
so the fix doesn't regress the original word-boundary purpose.

## Boundary

Only `crates/gauntlet-app/src/skills/mod.rs` (code + its own test module)
should change. Do not touch `.wayfinder/answer-key.md`,
`.gauntlet/reference.lock`, the five real `SKILL.md` files, any
`PROVENANCE.md`, or `crates/gauntlet-arch-tests/src/single_writer.rs`
(already correct and confirmed live-fire by the falsifier).

After landing: `just check-package gauntlet-app`, `just test-package
gauntlet-app` (confirm ALL tests pass, old and new), `cargo nextest run -p
gauntlet-app skills::lint`, `just boundaries`, `just ruler`. Write progress
to `.agents/reports/task19/cor2-progress.md` as you go.
