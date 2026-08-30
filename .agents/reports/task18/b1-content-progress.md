# Task 18 — Slice A (B1-content) progress

Builder: content-only slice (SKILL.md, PROVENANCE.md, fixtures). No Rust/Cargo files touched.

Started: 2026-08-30.

## Step 0 — setup
- Created `.agents/skills/wayfinder-gauntlet/` and `tests/fixtures/formation/` directories.
- Read AGENTS.md, `.agents/reports/task18/IN-FLIGHT.md`, plan Task 18 section
  (`.agents/plans/gauntlet-impl.md` ~L575-599), spec §9.2 (~L614-646), spec
  §5.3.2 Unit schema (~L273-290), spec Annex D (~L2041-2079), and the real
  `.wayfinder/answer-key.md` AK-063..AK-065 entries for exact field syntax.

## Step 1 — upstream clone attempt
- Ran `git clone --depth 1 https://github.com/mattpocock/skills /home/sprime01/projects/tmp-wayfinder-clone-task18` (outside project tree, per plan). SUCCEEDED — network was reachable.
- Commit SHA: `6654f6b60cd9d5be8b54c6fafe44346dabeb3b76`.
- Read `skills/engineering/wayfinder/SKILL.md` (issue-tracker-based wayfinder skill: map as tracker issue, tickets as child issues, HITL/AFK ticket types, fog-of-war, out-of-scope, one-ticket-per-session).
- Licence: MIT (`LICENSE` at repo root, Copyright (c) 2026 Matt Pocock).
- Deleted the temporary clone with `rm -rf` immediately after extraction — confirmed removed, nothing from it committed into this repo.
- Result: `upstream: available` (not the `unavailable` fallback path) — SKILL.md below is derived from the real upstream text, cross-checked against spec §9.2 / Annex D.

## Step 2 — `.agents/skills/wayfinder-gauntlet/SKILL.md`
- Written. Preserves (own words, cross-checked against upstream text and
  spec §9.2/Annex D.3): destination-first charting; grilling loop
  (breadth-first frontier pass, then one-decision-at-a-time resolution,
  with the research-ticket exception carried over from upstream); fog of
  war with the exact precise-question test ("whether you can state the
  question precisely now, not whether you can answer it"); out-of-scope as
  a distinct, never-graduating section; HITL vs AFK including the verbatim
  guardrail that an agent answering its own grilling questions has broken
  the skill, and that unattended formation records unresolved questions as
  fog rather than inventing answers.
- Replaces tracker wiring with the two local files (`.wayfinder/map.md`,
  `.wayfinder/answer-key.md`), the typed Unit schema (id via heading, claim,
  verify_by, blast_radius, authority, evidence_required, depends_on) in
  place of a bare ticket body, `depends_on` in place of native tracker
  blocking edges, and `map.md`'s Decisions-so-far linking Unit ids instead
  of tracker links.
- Adds the `blast_radius: local` default (raised only for shared
  state/money/auth/security/hard-to-reverse decisions) and documents, as
  the explicit last step of the formation procedure, running
  `gauntlet reference validate --path .wayfinder/answer-key.md`.
- Contains no settlement/authority/budget decision logic (explicitly noted
  in the skill body itself, per Annex D.4 — skills are instructions, not
  control logic).

## Step 3 — `.agents/skills/wayfinder-gauntlet/PROVENANCE.md`
- Written. Records: upstream repo URL, upstream path
  (`skills/engineering/wayfinder`), commit SHA
  `6654f6b60cd9d5be8b54c6fafe44346dabeb3b76`, retrieval date 2026-08-30,
  MIT licence, and a paraphrased delta summary (pointing at Core-spec §9.2's
  table rather than duplicating it verbatim).

## Step 4 — fixtures under `tests/fixtures/formation/`
- `grilling-transcript.md` — fictional ~24-line transcript for a small
  decision ("should the CLI support a `--dry-run` flag on `run`"),
  including a HITL-appropriate hedge left open (irreversible-tier
  rendering) and an explicit out-of-scope aside (`roles install`
  dry-run), so the transcript itself motivates map.md's D1/D2/F1/S1.
- `map.md` — exact contract structure: 2 Decisions (D1 → AK-101, AK-102;
  D2 → context-only), 1 fog item (F1, no `→ AK-` arrow), 1 out-of-scope
  item (S1, no arrow). Verified by grep: no arrow present on F1/S1 in this
  file.
- `answer-key.md` — exactly `## AK-101` and `## AK-102`, using the field
  shape/values specified in the task (verify_by: test, blast_radius:
  local, authority: critic, evidence_required: {kind: test_report,
  locator: tests/fixtures/formation::fixture_check, producer: instrument}),
  matching the real `.wayfinder/answer-key.md` AK-063..065 literal syntax
  I read directly for this. AK-102 additionally carries `depends_on:
  [AK-101]` (optional field, exercised because the real key uses it
  elsewhere and D1's flag-then-plan decision is naturally two ordered
  claims) — this does not change the required field combination for
  either entry.
- `map-fog-leak.md` — identical to `map.md` except F1's line gains
  ` → AK-103` at the end. Verified by grep.
- `answer-key-fog-leak.md` — `answer-key.md` plus one additional
  well-formed `## AK-103` block (same field shape as AK-101/102), so it is
  schema-valid on its own. Verified: contains AK-101, AK-102, AK-103
  headings.

## Notes / ambiguities encountered
- None requiring escalation. The one judgment call: whether to include
  `depends_on` on AK-102 in the fixture answer keys. Left it in since the
  real answer key uses that field and the Task 7 parser must already
  support it; if Slice B's parser turns out not to accept `depends_on` for
  any reason, that would show up as a fixture-parsing test failure and
  should be flagged back here rather than silently reworked.
- No product/domain decisions were invented. The `--dry-run` scenario is
  explicitly a stand-in fixture per the task's own instruction ("this
  doesn't need to be realistic Gauntlet content") — it is not a
  Gauntlet-scope decision that will be built.

## Summary
All four Slice A (B1-content) deliverables are written: SKILL.md,
PROVENANCE.md, and the five fixture files under tests/fixtures/formation/.
Upstream clone succeeded (SHA 6654f6b60cd9d5be8b54c6fafe44346dabeb3b76, MIT
licence) and the temporary clone was deleted immediately after extraction.
No `.rs`/`Cargo.toml`/`Cargo.lock`/`.wayfinder/`/`.gauntlet/` files were
touched (confirmed via `git status`). No `git add`/`git commit` run — left
for the orchestrator. Slice A is complete and ready for the Slice B
conformance tests to consume.
