# Provenance — `wayfinder-gauntlet`

- **Upstream repository:** https://github.com/mattpocock/skills
- **Upstream path:** `skills/engineering/wayfinder`
- **Upstream commit SHA:** `6654f6b60cd9d5be8b54c6fafe44346dabeb3b76`
- **Retrieval date:** 2026-08-30
- **Licence:** MIT (`LICENSE` at the upstream repository root; Copyright (c)
  2026 Matt Pocock)
- **Upstream:** `available` — the clone reached the network; this derivation
  is transformed from the real fetched `SKILL.md`, not derived purely from
  spec text alone. (Core-spec Annex D.1 sanctions an `upstream: unavailable`
  fallback derived purely from §9.2 when the network can't be reached; that
  path was not needed here.)

## Retrieval procedure followed

Per Core-spec Annex D.2:

1. Cloned upstream (`git clone --depth 1`) into a temporary directory
   outside the project tree, at `../projects/tmp-wayfinder-clone-task18`
   (sibling of this repo, per the implementation plan).
2. Read `skills/engineering/wayfinder/SKILL.md` and its local-markdown
   tracker fallback convention.
3. Recorded the commit SHA and licence (above).
4. Transformed the skill (delta table below) into
   `.agents/skills/wayfinder-gauntlet/SKILL.md`.
5. Companion skills (`grilling`, `domain-modeling`, `research`,
   `prototype`) are referenced by name in the derived skill but are
   installed separately, each with its own provenance entry, under Task 19
   — out of scope for this task.
6. Deleted the temporary clone (`rm -rf`) immediately after step 4;
   confirmed removed; nothing from it was committed into this repository.
7. Conformance test: `tests/contract/skills::wayfinder_gauntlet_conformance`
   (AK-064), authored in the parallel Slice B of this task, exercises this
   derivation against the fixtures in `tests/fixtures/formation/`.

## Delta summary

Full detail is Core-spec §9.2's delta table; paraphrased here:

| Upstream | `wayfinder-gauntlet` | Why |
|---|---|---|
| Map + tickets as issue-tracker child issues | Two local files: `.wayfinder/map.md`, `.wayfinder/answer-key.md` | The Gauntlet's writer-authority model (§16.4) needs a small structured file with a single writer, not a tracker API. |
| Ticket body is a bare question; answer recorded on resolution | Answer-key entry is a typed `Unit` (§5.3.2) | A bare assertion is only gradeable by eyeball. A typed Unit binds the verification method and evidence requirement to the claim. |
| Tracker labels express ticket type | `verify_by` / `blast_radius` / `authority` / `evidence_required` typed fields | Execution-safety fields must be enums, not free-text labels. |
| Blocking via the tracker's native dependency relation | `depends_on` list | Ordering only; dependency never confers authority. |
| Map's Decisions-so-far links to tracker tickets | Map's Decisions-so-far links to Unit ids | Same index semantics, addressed locally instead of via a tracker URL. |

Additions beyond a straight port (also from §9.2 / Annex D.3):

- A default `blast_radius: local`, raised to `cross_cutting` or
  `irreversible` only when the decision touches shared state, money, auth,
  security, or is hard to reverse.
- A mandatory validation pass (`gauntlet reference validate --path
  .wayfinder/answer-key.md`) as the last step of the formation procedure, so
  a non-conforming answer key is rejected at formation time rather than
  discovered later at run time.

Removed relative to upstream: issue-tracker wiring, ticket-type labels, and
multi-session tracker-concurrency assumptions — none of these apply once the
output is two local files with a single writer.

Preserved verbatim in semantics (own words in the derived skill; see
`SKILL.md`): destination-first charting; the grilling/questioning loop;
fog-of-war with the precise-question test; out-of-scope as a distinct,
non-graduating section; HITL versus AFK, including that an agent answering
its own grilling questions has broken the skill and that unattended
formation records unresolved questions as fog rather than inventing answers;
one-decision-at-a-time discipline.

No settlement, authority-elevation, or budget decision logic is included in
the derived skill — per Core-spec Annex D.4, skills are instructions, not
control logic; that logic belongs in the runtime, where it is typed and
tested.
