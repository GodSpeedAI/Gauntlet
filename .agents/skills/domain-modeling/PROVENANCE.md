# Provenance — `domain-modeling`

- **Upstream repository:** https://github.com/mattpocock/skills
- **Upstream path:** `skills/engineering/domain-modeling`
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
   and read `skills/engineering/domain-modeling/SKILL.md`,
   `CONTEXT-FORMAT.md`, and `ADR-FORMAT.md`.
3. Recorded the commit SHA and licence (above).
4. Installed the port (delta below) to
   `.agents/skills/domain-modeling/SKILL.md`.
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

- **Front matter:** unchanged. The upstream `name` (`domain-modeling`) and
  `description` already conform to this repo's skill schema — no
  harmonization edits were needed beyond confirming the match.
- **Body:** the "active discipline" framing, the file-structure guidance
  (single `CONTEXT.md` vs. multi-context `CONTEXT-MAP.md`), the five
  during-session techniques (challenge against the glossary, sharpen fuzzy
  language, discuss concrete scenarios, cross-reference with code, update
  `CONTEXT.md` inline), and the three-part "offer ADRs sparingly" test are
  all preserved in substance. Reworded from "the user" to "the requester"
  for voice consistency, and a short header paragraph identifies this as a
  port and names the Gauntlet-specific role it plays (`wayfinder-gauntlet`'s
  companion for formation vocabulary, Core-spec Annex D.4).
- **Folded inline rather than ported as separate files:** upstream's
  `CONTEXT-FORMAT.md` and `ADR-FORMAT.md` gave the CONTEXT.md entry shape
  and the ADR numbering/template rules their own files with a `SKILL.md`
  link to each; this install's scope (per Task 19) is `SKILL.md` +
  `PROVENANCE.md` only, so their load-bearing rules (one/two-sentence
  definitions with an `Avoid` list; sequential `NNNN-slug.md` ADR numbering
  under `docs/adr/`; the same three-part hard-to-reverse/surprising/real-
  trade-off ADR test) were condensed directly into `SKILL.md`'s body instead
  of linking to files that would not exist in this installation. This also
  avoided reproducing upstream's illustrative directory trees, which open
  with a bare `/` root line — rephrased as prose to keep the body clear of
  anything an absolute-path scan could flag, per
  `.agents/reports/task19/IN-FLIGHT.md`'s Slice B section.
- **Not ported:** `agents/openai.yaml` (harness-specific configuration for
  a different agent product; out of scope).
- **No control logic, no secret-shaped literals** were present in the
  upstream text or introduced in the port; verified by inspection against
  Core-spec §8.1 and the denylists in
  `.agents/reports/task19/IN-FLIGHT.md`'s Slice B section.
