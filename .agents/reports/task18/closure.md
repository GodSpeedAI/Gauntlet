# Task 18 closure — `wayfinder-gauntlet` skill derivation

**Settled:** 2026-08-30. AK-064, AK-065 provisionally settled.

## What was built

- `.agents/skills/wayfinder-gauntlet/SKILL.md` + `PROVENANCE.md` — derived from
  the real upstream (`github.com/mattpocock/skills`, commit
  `6654f6b60cd9d5be8b54c6fafe44346dabeb3b76`, MIT), not the `unavailable`
  fallback path. Preserves destination-first, fog-of-war (precise-question
  test), out-of-scope-never-graduates, HITL-never-self-answered,
  one-decision-per-session; replaces tracker wiring with the two-file output
  and the typed Unit schema; documents `gauntlet reference validate` as the
  closing step of formation. No settlement/authority/budget logic embedded
  (Annex D.4).
- `gauntlet_app::reference::validate::validate_answer_key` — thin reuse of
  the Task 7 strict parser (`parse::parse_all_or_fail`), no new parsing
  logic.
- `gauntlet_app::formation` — the §9.2 formation-invariant enforcer
  (`check_reference_invariants(map_text, answer_key_text)`), the mechanism
  behind AK-065. See the correction history below — this module went
  through two hardening rounds after build.
- `gauntlet reference validate --path <path>` CLI subcommand
  (`crates/gauntlet-cli/src/main.rs`), mirroring `reference digest`'s
  arg-parsing style.
- `crates/gauntlet-app/tests/skills.rs::wayfinder_gauntlet_conformance`
  (AK-064) and `crates/gauntlet-app/tests/formation.rs::fog_not_in_answer_key`
  (AK-065) — locator divergence from the answer key's stated
  `tests/contract/...` path is recorded here, same class as task17's
  `roles.rs`, never a reason to touch the ruler.
- Fixtures under `tests/fixtures/formation/`: `grilling-transcript.md`,
  `map.md`, `answer-key.md` (AK-101/AK-102), `map-fog-leak.md`,
  `answer-key-fog-leak.md` (AK-103) — the last pair proves the layering is
  real: `reference validate` alone accepts the fog-leak answer key
  (schema-valid on its own, 3 units), only `formation::
  check_reference_invariants` rejects it.

## Confirmation history

1. **Design-as-built critic** (fresh context) → CONFIRM, 0 blocking / 2 major
   / 3 minor. Confirmed SKILL.md's semantic fidelity (not just keyword
   presence), PROVENANCE.md's Annex D.1 completeness, clean layering, and
   the schema-vs-formation separation by direct execution. Major RES-02:
   the formation parser failed open on non-canonical headings/markers.
   (`critic-design.md`)
2. **Falsifier round 1** (fresh, executing) → CONFIRM, mutation-teeth proven
   (RED→byte-identical restore→GREEN), but found the same class as RES-02
   independently: ASCII `->` arrow, case-variant heading, and (most
   severe) an extra `#` on "Not yet specified" right after a Decisions
   section silently re-filing a fog bullet as a validated decision.
   (`falsifier-round1.md`)
3. **Correction round 1**: widened heading/arrow recognition (case-
   insensitive, any depth, both arrow forms) to close the three
   demonstrated cases. Proved RED→GREEN on 3 new regression tests.
   (`correction-round1.md`)
4. **Falsifier round 2** (fresh, executing, independent of round 1's own
   tests) → **NOT_CONFIRM**. Found the fix was an allowlist over a still-
   permissive default: a trailing-colon heading (`## Not yet specified:`)
   reproduced the identical misfile bug via an unenumerated spelling.
   (`falsifier-round2.md`)
5. **Correction round 2**: architectural fix, not another allowlist entry —
   flipped the default from permissive ("unrecognized heading → keep
   current section") to a `Section::Unrecognized` state where any bullet
   carrying an `AK-` reference is refused (`AmbiguousSectionReference`).
   Only `Decisions` treats a reference as expected; every other state
   (including now `Unrecognized`) refuses one. Proved RED→GREEN; confirmed
   no behavior change on the shipped canonical fixtures.
   (`correction-round2.md`)
6. **Falsifier round 3** (fresh, executing) → **CONFIRM**, 0 blocking.
   Attacked the new deny-by-default architecture itself (substring-title
   headings, re-entrant sections, heading/bullet confusion, pre-existing
   dangling-reference/duplicate-block/decision-without-unit regressions,
   whitespace/empty inputs, malformed id shapes) and re-proved the
   mutation-teeth. Found two **minor, pre-existing, non-blocking**
   over-rejection findings in `parse_bullet`'s unvalidated id-shape
   splitting (biases toward false-positive refusal, never toward letting a
   leak through) — recorded as residuals, not fixed (out of scope for this
   task; trigger: next `formation` module touch). (`falsifier-round3.md`)

Net: two correction rounds were required before an independent, fresh,
executing falsifier could not find a way to slip a fog/out-of-scope
reference past the checker. Both corrections were proportionate
(the second was a genuine redesign of the default, not a widened
allowlist, precisely because the falsifier's round-2 diagnosis said an
allowlist would keep recurring).

## Verification (orchestrator-run, not builder say-so)

`just check-package gauntlet-app`, `just check-package gauntlet-cli`,
`just test-package gauntlet-app` (409/409), `just lint`, `just boundaries`
(228/228), `just ruler` (PASS), full workspace `just test` (1244/1244 after
correction round 1; re-confirmed package-scoped green after round 2) — all
run personally by the orchestrator after each round, not taken on trust
from any subagent's report.

## Residuals (trigger-recorded, not blocking)

- The bullet-marker whitespace-strictness gap (critic RES-02's other half,
  falsifier round 3's A5/A5b/A6): `parse_bullet` doesn't validate `AK-`
  id shape and doesn't recognize a malformed `- [` marker (extra
  space/tab/missing bracket) as a tracked bullet at all — both biases are
  toward over-rejection or silent non-tracking, never toward letting a
  real leak validate cleanly. Not fixed here (out of scope for task18's
  correction rounds; module docs already disclose "fixture-grade markdown,
  not CommonMark" as a deliberate scope choice). Trigger: next `formation`
  module touch, or when a real agent's `map.md` output is consumed at
  task20 composition.
- The plan's literal gate command (`cargo nextest run -p gauntlet-app
  skills::wayfinder`) doesn't match nextest's space-joined filter syntax;
  same pre-existing class as task17's `roles::` gate. Correct invocation:
  the bare test name (`wayfinder_gauntlet_conformance`, `fog_not_in_answer_key`).
  Locator divergence, not a defect; never a reason to touch the ruler.

## Guardrails held

`.wayfinder/answer-key.md` and `.gauntlet/reference.lock` were never
modified at any point across build, three falsifier rounds, or two
correction rounds — verified after every round.
