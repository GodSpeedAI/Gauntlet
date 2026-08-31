# Provenance — `grilling`

- **Upstream repository:** https://github.com/mattpocock/skills
- **Upstream path:** `skills/productivity/grilling`
- **Upstream commit SHA:** `6654f6b60cd9d5be8b54c6fafe44346dabeb3b76`
- **Retrieval date:** 2026-08-30
- **Licence:** MIT (`LICENSE` at the upstream repository root; Copyright (c)
  2026 Matt Pocock)
- **Upstream:** `available` — the clone reached the network; this
  installation is transformed from the real fetched `SKILL.md`, not derived
  purely from spec text alone (Core-spec Annex D.1 sanctions an `upstream:
  unavailable` fallback; that path was not needed here). Pinned to the same
  commit as `wayfinder-gauntlet` (task18) for provenance consistency across
  the companion skills it references.

## Retrieval procedure followed

Per Core-spec Annex D.2 (adapted for a companion skill rather than the
derived formation skill itself):

1. Cloned upstream (`git clone --depth 50`) into a temporary directory
   outside the project tree, at `../projects/tmp-skills-clone-task19`
   (sibling of this repo, per `.agents/plans/gauntlet-impl.md` Task 19 and
   `.agents/reports/task19/IN-FLIGHT.md`).
2. Checked out the pinned commit `6654f6b60cd9d5be8b54c6fafe44346dabeb3b76`
   (it was already reachable — HEAD of the default branch at clone time)
   and read `skills/productivity/grilling/SKILL.md`.
3. Recorded the commit SHA and licence (above).
4. Installed the port (delta below) to
   `.agents/skills/grilling/SKILL.md`.
5. Deleted the temporary clone (`rm -rf`) after all four companion skills
   were ported; confirmed removed — see
   `.agents/reports/task19/b1-content-progress.md`. Nothing from the clone
   was committed into this repository.
6. Conformance: this skill is exercised as one of Slice B's
   `REPO_OWNED_SKILLS` under `skills::lint`.

## Delta summary

This is a **direct port**, not a semantic redesign like `wayfinder-gauntlet`
— it is a generic productivity skill, not Gauntlet-specific, so the upstream
technique is preserved as-is.

- **Front matter:** unchanged. The upstream `name` (`grilling`) and
  `description` already conform to this repo's skill schema (`name` +
  `description` only, no extra keys) — no harmonization edits were needed
  beyond confirming the match.
- **Body:** reworded from "the user" to "the requester" throughout for
  voice consistency with this repo's other skills, and a short header
  paragraph was added identifying this as a port and naming the
  Gauntlet-specific role it plays (`wayfinder-gauntlet`'s companion for
  formation questioning, Core-spec Annex D.4). The design-tree / frontier /
  rounds technique, the question-format template, the fact-finding-is-the-
  agent's-job rule, and the "frontier empty" completion condition are all
  preserved verbatim in substance.
- **Not ported:** `agents/openai.yaml` (harness-specific configuration for
  a different agent product; out of scope — this repo installs skill
  content, not a harness-specific agent registration).
- **No control logic, no absolute paths, no secret-shaped literals** were
  present in the upstream text or introduced in the port; verified by
  inspection against Core-spec §8.1 and the denylists in
  `.agents/reports/task19/IN-FLIGHT.md`'s Slice B section.
