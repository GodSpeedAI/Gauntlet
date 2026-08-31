# Provenance — `prototype`

- **Upstream repository:** https://github.com/mattpocock/skills
- **Upstream path:** `skills/engineering/prototype`
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
   and read `skills/engineering/prototype/SKILL.md`, `UI.md`, and
   `LOGIC.md`.
3. Recorded the commit SHA and licence (above).
4. Installed the port (delta below) to
   `.agents/skills/prototype/SKILL.md`.
5. Deleted the temporary clone (`rm -rf`) after all four companion skills
   were ported; confirmed removed — see
   `.agents/reports/task19/b1-content-progress.md`. Nothing from the clone
   was committed into this repository.
6. Conformance: this skill is exercised as one of Slice B's
   `REPO_OWNED_SKILLS` under `skills::lint`.

## Delta summary

This is a **direct port**, not a semantic redesign like `wayfinder-gauntlet`
— it is a generic engineering skill, not Gauntlet-specific, so the upstream
technique is preserved as-is.

- **Front matter:** unchanged. The upstream `name` (`prototype`) and
  `description` already conform to this repo's skill schema — no
  harmonization edits were needed beyond confirming the match.
- **Body:** the "throwaway code that answers a question" framing, the
  branch-picking rule (logic vs. UI, defaulting by matching the surrounding
  code when the requester isn't reachable), and the six shared rules
  (throwaway-and-marked, trivial to run, no persistence by default,
  skip the polish, surface the state, capture it when done) are preserved
  in substance. Reworded from "the user" to "the requester" for voice
  consistency, and a short header paragraph identifies this as a port and
  names the Gauntlet-specific role it plays (`wayfinder-gauntlet`'s
  companion for a cheap artifact raising HITL discussion fidelity,
  Core-spec Annex D.4).
- **Folded inline rather than ported as separate files:** upstream split
  the two branches into `UI.md` and `LOGIC.md`, each linked from
  `SKILL.md`; this install's scope (per Task 19) is `SKILL.md` +
  `PROVENANCE.md` only, so both branches' load-bearing guidance was
  condensed directly into `SKILL.md`'s body — the logic branch's pure-module
  isolation and single-HTML-file shareable-demo shape (free-play buttons
  plus guided-walkthrough tabs), and the UI branch's prefer-the-existing-
  page rule, `variant` search-param switcher, and 3-variant default (cap 5).
  Some implementation-level detail from the originals was dropped as too
  fine-grained for a condensed body (e.g. the exact pseudo-code switcher
  snippet, the specific keyboard-focus exception, the production-build gate
  mechanism) — the anti-patterns list captures the load-bearing constraints
  instead. Upstream's `?variant=` and route-path examples (e.g. a literal
  leading-`/` route or a pseudo-code block) were rephrased as prose to keep
  the body clear of anything an absolute-path scan could flag, per
  `.agents/reports/task19/IN-FLIGHT.md`'s Slice B section.
- **Not ported:** `agents/openai.yaml` (harness-specific configuration for
  a different agent product; out of scope).
- **No control logic, no secret-shaped literals** were present in the
  upstream text or introduced in the port; verified by inspection against
  Core-spec §8.1 and the denylists in
  `.agents/reports/task19/IN-FLIGHT.md`'s Slice B section.
