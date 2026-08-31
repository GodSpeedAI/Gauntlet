# Task 19 correction round 1 (COR1 + COR2)

**Trigger:** fresh blind design-as-built critic CONFIRM (0 blocking / 1 major
/ 1 minor) — see `.agents/reports/task19/critic-design.md`.

## COR1 (major, RES-01) — widen the control-logic phrase denylist

The critic traced three concrete sentences that plainly violate spec Annex
D.4 (a skill deciding settlement, authorizing budget, or elevating
authority) but evade `CONTROL_LOGIC_PHRASES` in
`crates/gauntlet-app/src/skills/mod.rs`:

```
"This skill settles the Unit once its evidence passes."
"The agent should authorize additional budget for this unit."
"Elevate the Unit's authority to cross_cutting before dispatching."
```

Add phrases that close exactly these three (plus natural singular/plural
variants), chosen so they do NOT overlap with the existing legal-prose
regression test `a_bare_control_word_in_ordinary_prose_is_not_over_rejected`
(`"The runtime settles a Unit when its evidence is verified, under its own
authority model."` must keep linting clean — note it says "settles **a**
Unit", indefinite article; the new phrase below uses "settles **the**
unit", definite article, which does not appear as a substring of the legal
sentence — verify this non-overlap yourself before landing):

Add to `CONTROL_LOGIC_PHRASES`:
- `"settles the unit"`
- `"authorize additional budget"`
- `"authorize the budget"` (if not already effectively covered — check
  against existing `"approve the budget"`/`"deny the budget"`; add if
  genuinely distinct)
- `"authorizes additional budget"`
- `"elevate the unit's authority"`
- `"elevates the unit's authority"`
- `"grants the unit authority"`

Do not add bare words (`"settle"`, `"authority"`, `"budget"` alone) — that
regresses the over-rejection guard. Run
`cargo test-package gauntlet-app` (or `just test-package gauntlet-app`)
after the edit and confirm
`a_bare_control_word_in_ordinary_prose_is_not_over_rejected` still passes.

Add regression tests in the same `#[cfg(test)] mod tests` block in
`skills/mod.rs` (or `tests/skills.rs`, match the existing convention)
proving each of the three critic sentences above is now
`Err(SkillLintError::ControlLogicDetected { .. })`.

## COR2 (minor, RES-02) — disclose the lint's limitation

Add a short "## What this lint is not" section to the module doc comment
at the top of `crates/gauntlet-app/src/skills/mod.rs` (mirror the posture
of `crates/gauntlet-arch-tests/src/single_writer.rs`'s "THREAT MODEL — what
this rule is not" section, read it for the tone/shape to match — a
sibling file in the very same diff already sets this precedent). State
plainly: this is a closed lexical exact-phrase/identifier scanner, not
semantic understanding; a control-logic decision phrased in yet another
way than the pinned identifiers/phrases can still evade it (cross-
reference RES-01 in `.agents/reports/task19/critic-design.md`); a clean
`skills::lint` run is evidence the shipped content doesn't match a KNOWN
decisive pattern, never proof the text contains no control logic in any
phrasing — human/critic review remains the backstop for skill content,
same as elsewhere in this codebase.

## Boundary

Do not touch `.wayfinder/answer-key.md`, `.gauntlet/reference.lock`, or any
file outside `crates/gauntlet-app/src/skills/mod.rs` (and its test module,
wherever the existing tests already live — check both `skills/mod.rs`'s own
`#[cfg(test)]` block and `crates/gauntlet-app/tests/skills.rs` before
picking where to add the new regression tests; follow whichever convention
the existing three teeth tests for this lint already use). Do not touch
Slice A's skill content files — the five real `SKILL.md` files (`grilling`,
`domain-modeling`, `research`, `prototype`, `gauntlet-run`) already lint
clean per the critic's independent run; leave them exactly as they are.

After landing: `just check-package gauntlet-app`, `just test-package
gauntlet-app`, `cargo nextest run -p gauntlet-app skills::lint`, `just
boundaries`, `just ruler`. Write progress to
`.agents/reports/task19/cor1-progress.md` as you go.
