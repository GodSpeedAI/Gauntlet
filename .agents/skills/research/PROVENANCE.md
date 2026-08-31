# Provenance — `research`

- **Upstream repository:** https://github.com/mattpocock/skills
- **Upstream path:** `skills/engineering/research`
- **Upstream commit SHA:** `6654f6b60cd9d5be8b54c6fafe44346dabeb3b76`
- **Retrieval date:** 2026-08-30
- **Licence:** MIT (`LICENSE` at the upstream repository root; Copyright (c)
  2026 Matt Pocock)
- **Upstream:** `available` — the clone reached the network; this
  installation is transformed from the real fetched `SKILL.md`, not derived
  purely from spec text alone. Pinned to the same commit as
  `wayfinder-gauntlet` (task18) for provenance consistency.

## Retrieval procedure followed

Per Core-spec Annex D.2 (adapted for a companion skill rather than the
derived formation skill itself):

1. Cloned upstream (`git clone --depth 50`) into a temporary directory
   outside the project tree, at `../projects/tmp-skills-clone-task19`
   (sibling of this repo, per `.agents/plans/gauntlet-impl.md` Task 19 and
   `.agents/reports/task19/IN-FLIGHT.md`).
2. Checked out the pinned commit `6654f6b60cd9d5be8b54c6fafe44346dabeb3b76`
   and read `skills/engineering/research/SKILL.md`.
3. Recorded the commit SHA and licence (above).
4. Installed the port (delta below) to
   `.agents/skills/research/SKILL.md`.
5. Deleted the temporary clone (`rm -rf`) after all four companion skills
   were ported; confirmed removed — see
   `.agents/reports/task19/b1-content-progress.md`. Nothing from the clone
   was committed into this repository.
6. Conformance: this skill is exercised as one of Slice B's
   `REPO_OWNED_SKILLS` under `skills::lint`.

## Delta summary

This is a **direct port**, not a semantic redesign like `wayfinder-gauntlet`
— it is a generic engineering skill, not Gauntlet-specific, so the upstream
technique is preserved as-is. The upstream skill is short (a three-item
job list for a background research agent): it is reproduced essentially
verbatim, with "the user" reworded to "the requester" for voice
consistency, and a short header paragraph identifying this as a port and
naming the Gauntlet-specific role it plays (`wayfinder-gauntlet`'s companion
for AFK resolution of a fact a decision waits on, Core-spec Annex D.4).

- **Front matter:** unchanged. The upstream `name` (`research`) and
  `description` already conform to this repo's skill schema — no
  harmonization edits were needed beyond confirming the match.
- **Not ported:** `agents/openai.yaml` (harness-specific configuration for
  a different agent product; out of scope).
- **No control logic, no absolute paths, no secret-shaped literals** were
  present in the upstream text or introduced in the port; verified by
  inspection against Core-spec §8.1 and the denylists in
  `.agents/reports/task19/IN-FLIGHT.md`'s Slice B section.
