# Task 19 IN-FLIGHT — companion + repo-owned skills

**Dispatched:** 2026-08-30, orchestrator (this session).
**Status:** build phase, two parallel builder slices dispatched.
**Plan reference:** `.agents/plans/gauntlet-impl.md` Task 19. Deps: Task 18
(settled, 827dd46). Parallel-safe per the plan.

## Orchestrator decisions made before dispatch

1. **Per-role skills (`gauntlet-role-<role>`, Annex D.4):** NOT authored this
   task. Audited all 10 files in `.agents/roles/` (42–55 lines each, front
   matter + a short body, every one self-contained per task17's closure).
   None exceeds a normal role-file body or needs procedure content that
   would duplicate AGENTS.md/the plan (which AGENTS.md forbids duplicating).
   Decision recorded here per "ask before you assume" — no operator
   ambiguity, this is a bounded editorial judgment, not an architecture
   fork. If a future role file grows a genuinely long operating procedure,
   extract it then.
2. **Lint scope boundary:** `.agents/skills/` also contains `jolli*/` and
   other directories that are **not tracked in this repo** (excluded via
   `.git/info/exclude`; confirmed with `git ls-files .agents/skills/`,
   which currently returns only `wayfinder-gauntlet/{SKILL.md,PROVENANCE.md}`).
   Those are another tool's local runtime artifacts, not Gauntlet-authored
   skills, and must never be scanned, linted, or asserted about. The lint
   validates an explicit, closed list of repo-owned skill directory names —
   never a directory glob over `.agents/skills/` — so a foreign local skill
   can never accidentally pass *or* fail this task's gate.
3. **Front-matter schema (binding on both slices):** required keys `name`
   (must equal the skill's directory name) and `description` (non-empty,
   single line); the only recognized optional key is
   `disable-model-invocation` (bool) — already used by
   `wayfinder-gauntlet/SKILL.md`, which must keep passing. Any other
   front-matter key is a lint failure (deny-by-default — same lesson task18
   COR2 already paid for: an allowlist that silently under-rejects is worse
   than an explicit closed set).
4. **Reuse, do not reimplement:** front-matter parsing is
   `gauntlet_app::config::gauntlet_md::parse` (already used identically by
   `roles/definition.rs`). Do not write a second front-matter parser.

Two slices, disjoint files, no compile conflict (Slice A touches no
Rust/Cargo file) — same pattern as task18.

---

## Slice A (content, no Rust) — companions + gauntlet-run

Clone upstream (`git clone --depth 1 https://github.com/mattpocock/skills`)
into a temporary directory **outside the project tree**, e.g.
`../projects/tmp-skills-clone-task19` (sibling of this repo). Prefer pinning
the same commit task18 recorded (`6654f6b60cd9d5be8b54c6fafe44346dabeb3b76`)
if reachable, for provenance consistency; if network is unreachable, follow
spec Annex D.1's sanctioned fallback (`upstream: unavailable`) and author
each companion skill's body directly from its one-line role description in
spec Annex D.4 plus its generic subject matter (grilling technique,
domain-modeling, research, prototyping) — say so plainly in PROVENANCE.md,
do not pretend a transform occurred that didn't.

Install, each under `.agents/skills/<name>/`:

- `grilling/` — upstream `skills/productivity/grilling`
- `domain-modeling/` — upstream `skills/engineering/domain-modeling`
- `research/` — upstream `skills/engineering/research`
- `prototype/` — upstream `skills/engineering/prototype`

For each: `SKILL.md` with front matter conforming to the schema in decision
3 above (harmonizing the upstream file's front matter into this shape IS
the transform for these skills — record it as such, even if small, in that
skill's `PROVENANCE.md`, mirroring `wayfinder-gauntlet/PROVENANCE.md`'s
shape: upstream repo, upstream path, commit SHA, retrieval date, licence,
delta summary). Body must:
- preserve the upstream skill's actual technique/procedure (it is a direct
  port, not a semantic redesign like `wayfinder-gauntlet` — these are
  generic engineering/productivity skills, not Gauntlet-specific);
- contain **no control logic** (no settlement, authority, or budget
  decision — Annex D.4's closing rule) and no absolute paths and no
  secret-shaped literals (Slice B's lint will check this mechanically —
  coordinate: if the real upstream text trips the denylist, rephrase the
  port rather than defeating the lint);
- reference `$VAR`-style indirection only, never a literal secret value,
  consistent with spec §8.1.

Then author `.agents/skills/gauntlet-run/SKILL.md` — **repo-owned, no
PROVENANCE.md** (state "repo-owned, no upstream derivation" as the file's
first line under the front matter, same convention as noting
`upstream: unavailable` would, so a reader never has to guess). Content:
the operator procedure `doctor → reference → run → supervise escalations →
inspect evidence`, i.e. describe *when and how* to invoke:
`gauntlet doctor`, `gauntlet reference validate --path <path>` (Task 18),
`gauntlet run ...` (once Task 20 lands — note it may not exist yet at this
task's gate time; describe the intended command from spec §20.1 and say
plainly if it isn't wired yet, do not claim a command works before Task 20
proves it), how to find and act on escalations, and where evidence lives
(`.agents/reports/`, `.gauntlet/`). This skill **describes procedure only**
— it must never itself decide settlement/authority/budget; every such
decision is a sentence like "the runtime settles a Unit when X" (describing
what the runtime does), never "settle the unit when X" (an imperative
skill-authored decision). This distinction is exactly what Slice B's lint
enforces — read its control-logic denylist before writing this file.

Delete the temporary clone when done; confirm removed (`ls` the parent, as
task18's PROVENANCE.md procedure step 6 did).

Write progress to `.agents/reports/task19/b1-content-progress.md` as you go
(checkpoint discipline, AGENTS.md failure-log rule).

---

## Slice B (code) — `gauntlet_app::skills` lint module + tests

New module `crates/gauntlet-app/src/skills/mod.rs`; add `pub mod skills;`
to `crates/gauntlet-app/src/lib.rs` (alphabetical, between `settle` and
`state` — actually `skills` sorts before `state`, after `settle`; check the
existing list and slot it correctly).

- `pub struct SkillFrontMatter { pub name: String, pub description: String,
  pub disable_model_invocation: Option<bool> }` (or reuse
  `gauntlet_md::FrontMatter` directly if adding a typed wrapper is not
  pulling its weight — builder's call, but the four fields above are the
  ones the lint actually inspects).
- `pub enum SkillLintError { MissingFrontMatter, UnterminatedFrontMatter,
  MissingField { field: &'static str }, EmptyField { field: &'static str },
  NameMismatch { expected: String, found: String },
  UnknownFrontMatterKey { key: String }, ControlLogicDetected { pattern:
  String, line: usize }, AbsolutePathDetected { line: usize },
  SecretLikeValueDetected { line: usize } }` — typed, each carrying its
  failure locus (line number where useful). **Never** include the matched
  secret substring itself in the error (spec §8.1 — secrets never printed,
  even redacted ones might leak in a diff/log); the line number is enough
  to locate it.
- `pub fn lint_skill_text(dir_name: &str, text: &str) ->
  Result<SkillFrontMatter, SkillLintError>` — parse front matter via
  `crate::config::gauntlet_md::parse`, validate schema (decision 3), then
  scan the body.
- Control-logic denylist: closed, explicit, case-insensitive substring set
  anchored on the runtime's own control vocabulary so it cannot false-
  positive on ordinary prose. Include at minimum the exact identifiers
  `SettlementService`, `settle_unit`, `AuthorityGate`, `GateAdmission`,
  `BudgetMeter`, `Outcome::Pass`, `Outcome::Fail`, plus decisive multi-word
  phrases such as "mark the unit as settled", "grant authority", "approve
  the budget", "deny the budget", "the unit is now settled". Do **not**
  denylist a bare word like "settle" or "authority" alone — that will
  false-positive on this very skill's own procedural prose ("the runtime
  settles a Unit when..."). If real installed-skill text trips a false
  positive, fix the *phrasing* of the skill (coordinate with Slice A),
  never widen the lint into uselessness.
- Secret-like scan: `-----BEGIN`, common vendor token prefixes (`sk-`,
  `ghp_`, `AKIA`, `xoxb-`/`xoxp-`/`xoxa-`), and a literal
  `key:`/`token:`/`password:`/`secret:` assignment (front matter or body)
  whose value does not start with `$`.
- Absolute-path scan: a path token starting with `/` that is not part of an
  `http://`/`https://` URL, and Windows-style `[A-Za-z]:\`. Do not flag
  relative paths like `.wayfinder/...` or `crates/gauntlet-app/...`.
- `pub const REPO_OWNED_SKILLS: &[&str] = &["wayfinder-gauntlet",
  "grilling", "domain-modeling", "research", "prototype", "gauntlet-run"];`
  — the explicit closed list from decision 2. `pub fn
  lint_installed_skills(skills_root: &Path) ->
  Vec<(&'static str, Result<SkillFrontMatter, SkillLintError>)>` reads only
  these named subdirectories' `SKILL.md`, never globs the directory.

Tests — **add to the existing** `crates/gauntlet-app/tests/skills.rs`
(same nextest binary as task18's `wayfinder_gauntlet_conformance`, so the
gate's substring filter `skills::lint` resolves against test names
prefixed `lint_`):
- `fn lint_passes_every_installed_skill()` — runs
  `lint_installed_skills` against the real repo root (same
  `repo_root()` helper already in this file) and asserts every one of the
  six `REPO_OWNED_SKILLS` is `Ok`.
- `fn lint_rejects_control_logic_fixture()` — the required teeth. Add a
  fixture file `tests/fixtures/skills/control-logic-violation.md`
  (workspace-root-relative, same convention as `tests/fixtures/formation/`)
  containing valid front matter plus a body sentence that both names
  `SettlementService::settle_unit` and says "mark the unit as settled
  when tests pass" — assert `lint_skill_text` returns
  `Err(SkillLintError::ControlLogicDetected { .. })`.
- `fn lint_rejects_unknown_front_matter_key()` and
  `fn lint_rejects_absolute_path()` / `fn
  lint_rejects_secret_like_value()` as additional teeth if cheap — not
  strictly required by the plan's stated Done-when, but each denylist class
  should have at least one proof it actually fires, per this project's
  standing "a suite no adapter/mechanism can fail is not a suite" rule.

Write progress to `.agents/reports/task19/b1-code-progress.md` as you go.

---

## Gate (task19, run by orchestrator after both slices land)

```bash
cargo nextest run -p gauntlet-app skills::lint
```

Plus, before declaring settlement: `just check`, `just test`, `just lint`,
`just deny`, `just boundaries`, `just ruler`, `just doctor`. Then the
established Gauntlet loop: design-as-built critic (fresh, blind) →
executing falsifier round(s) → corrections as needed → fresh falsifier
CONFIRM before settlement. Never settle on builder say-so.

## Recovery note

If this session dies mid-flight: `git status` for landed slice work,
re-read this file for the contract, check
`.agents/reports/task19/*-progress.md` for slice status, do not re-dispatch
a slice whose files already exist and compile/pass — finish the remainder
per the Finisher procedure
(`.agents/lessons/session-interruption-recovery.md`).
