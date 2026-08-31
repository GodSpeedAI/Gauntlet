# Task 19 Slice A — content progress (companions + gauntlet-run)

Builder: sub-agent, dispatched 2026-08-30. Scope: exactly "Slice A (content,
no Rust)" in `.agents/reports/task19/IN-FLIGHT.md`. No Rust/Cargo touched.

## Status: DONE (Slice A content complete; awaiting Slice B lint + orchestrator gate)

## Final summary (result contract)

**Files created (full paths, all new/untracked — nothing modified outside
`.agents/skills/` and `.agents/reports/task19/`):**

- `/home/sprime01/projects/gauntlet/.agents/skills/grilling/SKILL.md`
- `/home/sprime01/projects/gauntlet/.agents/skills/grilling/PROVENANCE.md`
- `/home/sprime01/projects/gauntlet/.agents/skills/domain-modeling/SKILL.md`
- `/home/sprime01/projects/gauntlet/.agents/skills/domain-modeling/PROVENANCE.md`
- `/home/sprime01/projects/gauntlet/.agents/skills/research/SKILL.md`
- `/home/sprime01/projects/gauntlet/.agents/skills/research/PROVENANCE.md`
- `/home/sprime01/projects/gauntlet/.agents/skills/prototype/SKILL.md`
- `/home/sprime01/projects/gauntlet/.agents/skills/prototype/PROVENANCE.md`
- `/home/sprime01/projects/gauntlet/.agents/skills/gauntlet-run/SKILL.md`
  (repo-owned; deliberately **no** `PROVENANCE.md`, per IN-FLIGHT.md — the
  file's first line under its front matter states "Repo-owned, no upstream
  derivation.")
- `/home/sprime01/projects/gauntlet/.agents/reports/task19/b1-content-progress.md`
  (this file)

**Network clone:** SUCCEEDED. `git clone --depth 50
https://github.com/mattpocock/skills` into
`/home/sprime01/projects/tmp-skills-clone-task19` (sibling of the gauntlet
repo, outside the project tree) reached the network. The pinned commit
`6654f6b60cd9d5be8b54c6fafe44346dabeb3b76` (same commit task18 used) was
already HEAD of the default branch at clone time and was checked out
explicitly for certainty. The Annex D.1 `upstream: unavailable` fallback
was **not** needed. All four companion skills are ports of the real fetched
upstream `SKILL.md` text (condensed/reworded per each PROVENANCE.md's delta
summary — auxiliary upstream files (`CONTEXT-FORMAT.md`, `ADR-FORMAT.md`,
`UI.md`, `LOGIC.md`, `agents/openai.yaml`) were not in this task's install
scope (`SKILL.md` + `PROVENANCE.md` only) and were folded inline or
dropped, as recorded per-skill).

**Temp clone deletion:** CONFIRMED REMOVED. `rm -rf
/home/sprime01/projects/tmp-skills-clone-task19` ran after all four
companion skills were ported; `ls /home/sprime01/projects/` no longer lists
it (verified directly, see log below). Nothing from the clone was
committed into this repository.

**Lint self-check:** before finishing, grep-scanned all five new
`SKILL.md` bodies against Slice B's three denylist classes described in
IN-FLIGHT.md (control-logic identifiers/phrases, secret-shaped vendor-token
substrings, absolute/Windows path patterns). One false-positive risk was
found and fixed: `gauntlet-run/SKILL.md` originally read "task-level and
run-level reports", whose substring `...ask-l...` contains `sk-`
case-insensitively; reworded to "per-task and per-run reports". Final scan
across all five files: zero matches on all four checks (secret-substrings,
control-logic, leading-`/` absolute path, Windows path). Front-matter
blocks verified: all five have exactly `name` (matching the directory) and
`description` (non-empty, single line), no other keys — schema-conformant
per IN-FLIGHT.md decision 3.

**Scope discipline:** no file under `crates/gauntlet-app/` (including
`src/skills/`, `tests/skills.rs`, `src/lib.rs`), `.wayfinder/answer-key.md`,
or `.gauntlet/reference.lock` was read for editing or modified. `git
status --porcelain` for `.agents/skills`, `.agents/reports/task19`, and
`crates/gauntlet-app` confirms only the new untracked skill directories and
this report — nothing in Slice B's territory touched. No `cargo` command
was run. Nothing was committed (orchestrator commits after both slices
land).

---

## Status: IN PROGRESS (historical log below, superseded by summary above)

## Log

- 2026-08-30: Read AGENTS.md, builder.agent.md, IN-FLIGHT.md (both slices, to
  avoid tripping Slice B's lint), Core-spec.md §8.1, §9.2, §20.1, Annex D
  (D.1-D.4), wayfinder-gauntlet/SKILL.md + PROVENANCE.md (precedent),
  gauntlet-impl.md Task 19. Created this progress file before starting any
  clone or write, per checkpoint discipline.
- Plan: clone upstream https://github.com/mattpocock/skills shallow into
  `../projects/tmp-skills-clone-task19` (sibling of gauntlet repo), prefer
  commit 6654f6b60cd9d5be8b54c6fafe44346dabeb3b76 (same as task18, for
  provenance consistency across companion skills referenced by
  wayfinder-gauntlet). Fall back to `upstream: unavailable` (spec Annex D.1)
  if network unreachable.
- Clone succeeded: `git clone --depth 50 https://github.com/mattpocock/skills
  ../projects/tmp-skills-clone-task19`. Target commit
  6654f6b60cd9d5be8b54c6fafe44346dabeb3b76 was already HEAD of the default
  branch at clone time (reachable, no extra fetch needed); checked it out
  explicitly. Confirmed all four companion dirs exist at that commit:
  `skills/productivity/grilling`, `skills/engineering/domain-modeling`,
  `skills/engineering/research`, `skills/engineering/prototype`. Licence:
  MIT, root `LICENSE`, Copyright (c) 2026 Matt Pocock.
- Read all four upstream `SKILL.md` files in full, plus domain-modeling's
  `CONTEXT-FORMAT.md`/`ADR-FORMAT.md` and prototype's `UI.md`/`LOGIC.md`
  (auxiliary files not in this task's install scope — SKILL.md +
  PROVENANCE.md only — so their load-bearing rules were condensed inline
  into the ported SKILL.md body rather than linked as files that would not
  exist in the installation).
- Confirmed all four upstream front-matter blocks already conform to the
  IN-FLIGHT.md decision-3 schema (`name` + `description` only, `name`
  matching the directory) — no front-matter edits were needed, only the
  body port.
- Wrote and installed:
  - `.agents/skills/grilling/SKILL.md` + `PROVENANCE.md`
  - `.agents/skills/domain-modeling/SKILL.md` + `PROVENANCE.md`
  - `.agents/skills/research/SKILL.md` + `PROVENANCE.md`
  - `.agents/skills/prototype/SKILL.md` + `PROVENANCE.md`
  For domain-modeling and prototype, rephrased upstream's illustrative
  directory-tree diagrams and route-path examples (which used a bare
  leading `/`) into prose, to stay clear of Slice B's absolute-path scan
  per IN-FLIGHT.md's Slice B section.
- Self-checked all four new SKILL.md bodies with grep against Slice B's
  three denylists (control-logic identifiers/phrases, secret-shaped
  literals/prefixes, absolute-path patterns) before moving on — all clean,
  zero matches on all three scans.
- Next: author `.agents/skills/gauntlet-run/SKILL.md` (repo-owned, no
  PROVENANCE.md), then delete the temp clone and confirm removal.
- Authored `.agents/skills/gauntlet-run/SKILL.md`: first line under front
  matter is "Repo-owned, no upstream derivation." per IN-FLIGHT.md
  convention. Body covers doctor → reference → run → supervise escalations
  → inspect evidence per spec §20.1's command surface and IN-FLIGHT.md's
  Slice A spec. Named `gauntlet run "<desired transformation>"` per §20.1
  but stated plainly it may not be wired yet (Task 20 not landed at this
  task's gate time) rather than claiming it works. Every settlement/
  authority/evidence statement phrased descriptively ("the runtime settles
  a Unit when...", never an imperative instruction to settle/grant/approve)
  to stay clear of Slice B's control-logic denylist while still describing
  the real procedure.
- Ran grep-based self-checks (control-logic identifiers/phrases,
  secret-shaped vendor-token substrings including raw `sk-`/`ghp_`/`AKIA`/
  `xoxb-`/`xoxp-`/`xoxa-`/`-----BEGIN` and `key:`/`token:`/`password:`/
  `secret:` assignments, absolute/Windows path patterns) against all five
  new SKILL.md files. Found and fixed one false-positive risk in
  gauntlet-run/SKILL.md ("task-level" contains "sk-" as a raw substring);
  reworded to "per-task". Re-ran all checks clean after the fix. Verified
  all five front-matter blocks have exactly `name`+`description`, no extra
  keys.
- Deleted the temp clone: `rm -rf ../projects/tmp-skills-clone-task19`;
  confirmed via `ls /home/sprime01/projects/` that it no longer appears.
- Confirmed via `git status --porcelain` that only new untracked files
  under `.agents/skills/{grilling,domain-modeling,research,prototype,
  gauntlet-run}/` and `.agents/reports/task19/` exist — nothing under
  `crates/gauntlet-app/`, `.wayfinder/`, or `.gauntlet/` was touched. No
  `cargo` command was run at any point. Nothing committed.
- Slice A content work is complete. See "Final summary (result contract)"
  at the top of this file for the authoritative done-state the orchestrator
  should read.
