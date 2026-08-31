# Critic design-as-built review — Task 19 (companion + repo-owned skills)

**Verdict: CONFIRM**

Blocking: 0 · Major: 1 · Minor: 1

Fresh-context review. Allowlisted envelope: `.agents/agents/critic.agent.md`,
`AGENTS.md`, `.agents/plans/gauntlet-impl.md` (Task 19), `.agents/specs/Core-spec.md`
(§9.2, Annex D, §8.1), `.agents/reports/task19/IN-FLIGHT.md`, and
`.agents/skills/wayfinder-gauntlet/{SKILL.md,PROVENANCE.md}` (precedent). No
`*-progress.md` file under `.agents/reports/task19/` was opened. Independently
inspected the staged diff and ran the gate commands myself (not taken on
report/say-so):

```
cargo nextest run -p gauntlet-app skills::lint   → 5 tests run: 5 passed
just boundaries                                   → 228 tests run: 228 passed
just ruler                                        → PASS (digests match .gauntlet/reference.lock)
```

## Summary of what was checked and found sound

- `REPO_OWNED_SKILLS` (`crates/gauntlet-app/src/skills/mod.rs:192-199`) is a
  fixed `&[&str]` literal of exactly six names; `lint_installed_skills` maps
  over that constant only — no `read_dir`/glob over `.agents/skills/`.
  Verified independently: `git ls-files .agents/skills/` returns only the
  six repo-owned directories' files; `jolli*` directories exist on disk
  (`ls .agents/skills/`) but are absent from `git ls-files` and from
  `git status --porcelain` — genuinely untracked, genuinely excluded from
  the lint's reach, matching IN-FLIGHT.md decision 2.
- Front matter of all five new `SKILL.md` files (`grilling`,
  `domain-modeling`, `research`, `prototype`, `gauntlet-run`): each has
  exactly `name` (equal to its own directory name) and `description`
  (non-empty, single line); no extra keys. Confirmed both by direct
  reading and by the passing `lint_passes_every_installed_skill` gate,
  which I ran myself rather than trusting.
- `gauntlet-run/SKILL.md` describes procedure only. It states explicitly
  that it "never itself decides settlement, authority, or budget" and
  phrases every runtime action descriptively ("the runtime performs the
  freeze," "the runtime settles a Unit when..."-style framing), never as
  an imperative the skill itself carries out. It correctly refuses to
  overclaim Task 20: "`gauntlet run` ... **This command may not be wired
  yet** ... do not claim `gauntlet run` works ... if it is not yet
  implemented, say so plainly."
- All four companion `PROVENANCE.md` files (`grilling`, `domain-modeling`,
  `research`, `prototype`) record upstream repository, upstream path,
  commit SHA (`6654f6b60cd9d5be8b54c6fafe44346dabeb3b76`, matching task18's
  pin), retrieval date, licence, and a genuine delta summary (front-matter
  harmonization, voice rewording, and — for `domain-modeling`/`prototype`
  — an honest note that upstream's split-file structure was folded inline
  rather than silently dropped). `gauntlet-run/SKILL.md` states "Repo-owned,
  no upstream derivation" as its first line under the front matter, per
  contract.
- Scope: `git diff --cached --stat` shows exactly the Slice A files (5
  skill directories' `SKILL.md`/`PROVENANCE.md`, `gauntlet-run` with no
  `PROVENANCE.md` as contracted) and Slice B files (`skills/mod.rs`,
  `lib.rs` +1 line, `tests/skills.rs`, `single_writer.rs`, one fixture).
  No per-role `gauntlet-role-*` skills were added, matching IN-FLIGHT.md
  decision 1 (deliberately deferred, recorded, not silently dropped). No
  file outside this footprint was touched.
- `single_writer.rs` diff: `crates/gauntlet-app/src/skills/mod.rs` was
  added to `ANSWER_KEY_SURFACE_FILES` (Rule A — lexical surface-token
  inventory) but **not** to `WRITE_API_INVENTORY` (Rule C — the write-token
  inventory with containment guards). This is correct and honest, not a
  loosened boundary: `REPO_OWNED_SKILLS` contains the literal string
  `"wayfinder-gauntlet"`, which trips the `"wayfinder"` surface token
  (Rule A is a pure lexical scan, case-insensitive substring, over ALL
  core-crate production text). `skills/mod.rs` contains no write-API token
  anywhere (only `std::fs::read_to_string`, never `fs::write` or an
  equivalent) — confirmed by reading the full file — so it has nothing to
  register under Rule C, and none of the module's existing read-only
  behavior changed. The added reason string in the pin ("a skill identity
  string the lint's filesystem lookups must match exactly, not a path this
  module owns or derives; it neither reads nor writes
  `.wayfinder/answer-key.md`") is accurate. `just boundaries` passed
  228/228 including `answer_key_single_writer` with this change live, and
  the module's own docs describe Rule A false positives as an accepted,
  by-design category (`format!`-constructed mentions, etc.) — this is the
  same category, just triggered by a literal instead of a `format!` call.
  **Informational, not a residual: this is a legitimate documented lexical
  false positive, not the boundary being weakened to make code pass.**

## Residuals

### RES-01
- **Dimension:** architecture
- **Severity:** major
- **Locus:** `crates/gauntlet-app/src/skills/mod.rs:216-227`
  (`CONTROL_LOGIC_PHRASES`, five exact multi-word phrases)
- **Contract ref:** plan Task 19 Gate ("a skill containing a settlement
  decision fails it (teeth)"); Core-spec Annex D.4 ("skills... are not a
  place to put control logic: anything that decides settlement, authority,
  or budget belongs in the runtime"); IN-FLIGHT.md Slice B ("Include at
  minimum the exact identifiers... plus decisive multi-word phrases such
  as...").
- **Defect:** The denylist correctly avoids bare-word false positives
  (verified: `"the runtime settles a Unit when its evidence is verified,
  under its own authority model"` lints clean, both by direct trace of
  `scan_control_logic`'s case-insensitive substring match against the
  fixed 12-entry list and by the module's own passing unit test
  `a_bare_control_word_in_ordinary_prose_is_not_over_rejected`). But its
  recall is narrow enough that a settlement/authority/budget decision
  phrased even slightly differently from the five listed idioms, or not
  using one of the seven exact Rust identifiers, evades detection
  entirely — while still being exactly the class of text Annex D.4
  forbids a skill from containing. The required fixture
  (`tests/fixtures/skills/control-logic-violation.md`) is caught only
  because it happens to use two of the exact listed strings
  (`SettlementService::settle_unit` and "mark the unit as settled").
  A skill author phrasing the same decision differently is not caught by
  any mechanism in this task.
- **Reproduction (traced against the closed list at
  `skills/mod.rs:206-227`, no rephrasing needed to falsify — each string
  below is checked for substring containment, case-insensitively, against
  all 12 entries; none match):**
  ```
  lint_skill_text("x", "---\nname: x\ndescription: d\n---\n\
      This skill settles the Unit once its evidence passes.\n")
  // → Ok(...) — a skill CLAIMING to settle a Unit itself passes clean.
  // Neither "settle_unit" (no underscore/verb-noun form present) nor any
  // of the five phrases match "settles the Unit once...".

  lint_skill_text("x", "---\nname: x\ndescription: d\n---\n\
      The agent should authorize additional budget for this unit.\n")
  // → Ok(...) — a budget-authorization instruction to an agent passes
  // clean. "authorize" is not in the list; "approve the budget" /
  // "deny the budget" require those exact words in that exact order.

  lint_skill_text("x", "---\nname: x\ndescription: d\n---\n\
      Elevate the Unit's authority to cross_cutting before dispatching.\n")
  // → Ok(...) — an authority-elevation instruction (exactly the case
  // AGENTS.md's "An agent may never lower a Unit's blast radius or
  // authority" is guarding the inverse of) passes clean. Bare "authority"
  // is deliberately not denylisted (correctly, to avoid over-rejection),
  // but "elevate...authority" is a decisive phrase with no listed
  // synonym.
  ```
  This is a real gap in the stated "teeth" requirement, not a contract
  violation of the letter of Task 19 (the plan's Done-when — one control-
  logic fixture must fail — is satisfied, and IN-FLIGHT.md phrases the
  list as an "at minimum" example set rather than a completeness
  contract). None of the five actually-shipped `SKILL.md` bodies contain
  any of the three example sentences above or an equivalent — read in
  full, all five are genuinely procedural. The gap is in the mechanism's
  durability against future or careless skill authorship, not in the
  content shipped by this task.
- **Suggested correction (not applied — read-only review):** extend
  `CONTROL_LOGIC_PHRASES` with a small number of additional decisive verb
  phrases anchored on the skill/agent being the grammatical subject of a
  settlement/authority/budget verb (e.g. "settles the unit", "authorize
  the budget", "elevate...authority", "grants authority"), while
  continuing to avoid bare nouns — this stays within the explicit design
  constraint ("never widen the lint into uselessness") since it adds
  specific decisive phrases, not bare words.

### RES-02
- **Dimension:** evidence
- **Severity:** minor
- **Locus:** `crates/gauntlet-app/src/skills/mod.rs:1-56` (module doc
  comment)
- **Contract ref:** none directly (not required by Task 19's plan text);
  raised because the sibling file touched in the same diff,
  `crates/gauntlet-arch-tests/src/single_writer.rs:61-105`, sets an
  explicit precedent in this same codebase for disclosing a lexical
  scanner's known escapes in a dedicated "THREAT MODEL" section rather
  than leaving them implicit.
- **Defect:** `skills/mod.rs`'s module doc explains the schema and the
  hyphen-normalization bridge in detail but does not carry an equivalent
  explicit "known limitations" section stating that the control-logic
  scan is a closed exact-phrase/identifier lexical match, not semantic
  detection, and that rephrased control logic is a disclosed, accepted
  residual (the same posture RES-01 describes). The single-writer module
  one file away in the same commit treats exactly this kind of
  disclosure as load-bearing documentation, not decoration.
- **Reproduction:** read `crates/gauntlet-arch-tests/src/single_writer.rs`
  lines 61-105 ("THREAT MODEL — what this rule is not") against
  `crates/gauntlet-app/src/skills/mod.rs` lines 1-56 — the latter has no
  comparable section.
- **Suggested correction (not applied):** add a short "what this lint is
  not" note to the module doc, cross-referencing RES-01's recall
  limitation, so a future reader does not mistake a clean
  `skills::lint` run for semantic proof that a skill body contains no
  control logic.

## Gate reproduction (run independently, not taken from any report)

```
$ cargo nextest run -p gauntlet-app skills::lint
Summary [0.008s] 5 tests run: 5 passed, 425 skipped

$ just boundaries
Summary [17.224s] 228 tests run: 228 passed, 0 skipped
(includes answer_key_single_writer, PASS)

$ just ruler
verify-ruler.sh: PASS — answer key and manifest digests match the frozen
lock (/home/sprime01/projects/gauntlet/.gauntlet/reference.lock)
```
