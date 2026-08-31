# Task 19 Slice B — code progress (`gauntlet_app::skills` lint module)

Builder: sub-agent, dispatched 2026-08-30. Scope: exactly "Slice B (code)" in
`.agents/reports/task19/IN-FLIGHT.md`. Touches only Rust/Cargo files plus one
new test fixture; never touches Slice A's `.agents/skills/*` content files or
their `PROVENANCE.md`.

## Status: DONE — all gates green, including `lint_passes_every_installed_skill`

## Final summary (result contract)

### Files created

- `crates/gauntlet-app/src/skills/mod.rs` — the lint module:
  `SkillFrontMatter`, `SkillLintError` (typed, each variant carrying its
  failure locus; two deliberate completions beyond `IN-FLIGHT.md`'s
  illustrative enum are documented in the module doc comment —
  `FrontMatterGrammar` wraps the shared grammar's own typed error instead
  of collapsing every grammar failure into one case, and `InvalidFieldKind`
  / `Io` cover a wrong-kind field value and an unreadable file), the
  control-logic denylist (`CONTROL_LOGIC_IDENTIFIERS` /
  `CONTROL_LOGIC_PHRASES`, exactly the set IN-FLIGHT.md's Slice B section
  names), the secret-like scanner, the absolute-path scanner,
  `REPO_OWNED_SKILLS` (the closed six-entry list), `lint_skill_text`, and
  `lint_installed_skills`. Includes 16 unit tests in `#[cfg(test)] mod
  tests`.
- `tests/fixtures/skills/control-logic-violation.md` — the required teeth
  fixture: valid front matter, body naming
  `SettlementService::settle_unit` and the phrase "mark the unit as
  settled when tests pass".

### Files modified

- `crates/gauntlet-app/src/lib.rs` — added `pub mod skills;` in the
  correct alphabetical slot, between `settle` and `state`.
- `crates/gauntlet-app/tests/skills.rs` — added five tests
  (`lint_passes_every_installed_skill`, `lint_rejects_control_logic_fixture`,
  `lint_rejects_unknown_front_matter_key`, `lint_rejects_absolute_path`,
  `lint_rejects_secret_like_value`), nested inside a new `mod skills { ...
  }` block. **Why the nesting**: `cargo nextest`'s positional filter is a
  "test name filter" (`cargo nextest run --help`), matching a test's own
  qualified Rust path within its binary — NOT `<binary-id>::<test-name>`.
  Empirically confirmed: with these five functions at the file's top
  level (as first written), `cargo nextest run -p gauntlet-app
  skills::lint` returned "0 tests run... no tests to run", because a
  top-level integration-test function's Rust name carries no `skills::`
  prefix (that prefix only appears in the binary id nextest prints for
  *display*, e.g. `gauntlet-app::skills lint_rejects_absolute_path` — a
  space, not the `::` IN-FLIGHT.md's gate command assumes). Wrapping the
  five gate-covered tests in `mod skills { use super::*; ... }` makes
  their real qualified name `skills::lint_*`, so the literal gate command
  matches as a prefix — verified: `cargo nextest run -p gauntlet-app
  skills::lint` now runs exactly those 5 tests, all passing.
  `wayfinder_gauntlet_conformance` (task18's test) stays untouched at the
  file's top level, unaffected by the nesting, and still passes.
- `crates/gauntlet-arch-tests/src/single_writer.rs` — added
  `crates/gauntlet-app/src/skills/mod.rs` to `ANSWER_KEY_SURFACE_FILES`
  (with reason), updated the corresponding pin test
  (`the_pinned_inventories_are_pinned`), and added a matching synthetic
  entry to the `Repo::clean` test fixture used by
  `the_clean_fixture_passes_the_check`. **Why**: `REPO_OWNED_SKILLS` must
  contain the literal, exact skill-directory name `wayfinder-gauntlet`
  for `lint_installed_skills`'s filesystem lookups to be correct — no
  rephrasing removes that requirement. `just boundaries`'s
  `answer_key_single_writer` check (spec §16.4, AK-043) lexically scans
  every core-crate production `.rs` file for the substring `wayfinder`
  (among other tokens) as a proxy for "names the answer-key surface," and
  flags any match not in its pinned inventory. This is a real, honest
  lexical match (not a bug in the scanner) on a string that does NOT
  actually reference `.wayfinder/answer-key.md` — `skills/mod.rs` never
  reads or writes it, has zero write-API tokens, and so cannot trip Rule
  B/C of that same check. The scanner's own violation message names the
  sanctioned remedy for exactly this situation ("extend
  ANSWER_KEY_SURFACE_FILES with a reason or remove the mention"); removal
  was not viable (the literal string is load-bearing), so I used the
  extension path, with an honest reason stating there is no ownership or
  derivation of the reference path. This is the only file outside
  `gauntlet-app` and the fixture I touched, and it was necessary to
  satisfy this task's explicit requirement to run `just boundaries`
  before declaring done.
  Two other cosmetic changes in `crates/gauntlet-app/src/skills/mod.rs`
  and `crates/gauntlet-app/tests/skills.rs` were needed for the SAME
  `just boundaries` gate (the separate `core_crates_contain_no_hardcoded_literals`
  check, AK-016): a doc comment describing URL-token exclusion originally
  spelled `http://`/`https://` literally (the "hardcoded-url" class flags
  any `scheme://` in any string literal or comment) — reworded to
  describe the shape in prose instead; and three test fixture strings
  used real path-root names from the scanner's own `ABSOLUTE_PATH_ROOTS`
  list (`/etc`, `/usr`) or a literal URL scheme — replaced with a
  synthetic root name (`/gauntlet-fixture-root/...`, not one of the
  scanner's recognized roots) and a rewritten test that proves the same
  underlying property (a token not starting with `/` is never flagged)
  without a literal URL. This is the same class of "the boundary suite
  polices test text like any other source" note already left in
  `config/gauntlet_md.rs`'s own test module — not new to this task.

### Verification run (all green, in this order, per AGENTS.md's
fast-feedback workflow)

1. `cargo check -p gauntlet-app --all-targets` — PASS.
2. `cargo nextest run -p gauntlet-app` (full package) — 430/430 passed, 0
   failed, 0 skipped.
3. `cargo nextest run -p gauntlet-app skills::lint` (the exact literal
   gate command from IN-FLIGHT.md) — 5/5 passed:
   `skills::lint_rejects_absolute_path`,
   `skills::lint_rejects_control_logic_fixture`,
   `skills::lint_rejects_secret_like_value`,
   `skills::lint_rejects_unknown_front_matter_key`,
   `skills::lint_passes_every_installed_skill`.
4. `just boundaries` (`cargo nextest run -p gauntlet-arch-tests`) —
   228/228 passed, 0 failed. Includes `answer_key_single_writer` and
   `core_crates_contain_no_hardcoded_literals`, both green.
5. `cargo fmt --check -p gauntlet-app -p gauntlet-arch-tests` — clean
   (ran `cargo fmt` once to apply four mechanical rustfmt reflows in
   `skills/mod.rs`/`tests/skills.rs`, then re-verified clean; no
   semantic change).
6. `cargo clippy -p gauntlet-app -p gauntlet-arch-tests --all-targets --
   -D warnings` — 0 warnings.

Not run (out of my task's stated scope — orchestrator's closure gates,
run after both slices land): `just deny`, `just ruler`, `just doctor`,
the design-as-built critic and falsifier rounds. `just check` / `just
test` (workspace-wide) were not run since this task's scope permits
package-scoped verification and the workspace-wide equivalents
(`check-package`/`test-package` for the two touched crates, plus
`boundaries`) all passed; the orchestrator's stated closure sequence
covers the workspace-wide gates.

### `lint_passes_every_installed_skill` status: PASSES (real, not
aspirational)

Slice A completed and landed all six `REPO_OWNED_SKILLS` directories
under `.agents/skills/` (confirmed via `ls .agents/skills/` and
`.agents/reports/task19/b1-content-progress.md`'s own "Slice A content
work is complete" line) before this test was last run. The final
verification run above (`cargo nextest run -p gauntlet-app
skills::lint`) exercised `lint_installed_skills` against the real
repository root and it reported zero failures across all six skills.

### Deviations from IN-FLIGHT.md's illustrative text, each recorded here
rather than silently made

1. `SkillLintError` has 11 variants, not the 9 named illustratively in
   IN-FLIGHT.md's Slice B section — see "Files created" above. Every
   variant IN-FLIGHT.md named is present (under the same or a very close
   name); the two additions are typed-error completeness, not scope
   drift, and IN-FLIGHT.md's own text explicitly leaves this boundary to
   "builder's call."
2. The stated gate command works only because the five gate-covered
   tests are nested in `mod skills { ... }` inside `tests/skills.rs` —
   see "Files modified" above for the full empirical reasoning. This
   follows the SAME "locator-class divergence, recorded rather than
   silently patched or silently ignored" precedent this very file's own
   top doc comment already establishes for the AK-064 locator (task18).
3. `crates/gauntlet-arch-tests/src/single_writer.rs` was touched (see
   above) — the only file outside `gauntlet-app` I modified, and only to
   satisfy this task's own explicit "run `just boundaries` before
   declaring done" requirement via that suite's own documented,
   sanctioned extension mechanism for a lexical false positive on a
   load-bearing literal.

## Log

- 2026-08-30: Read AGENTS.md, builder.agent.md, IN-FLIGHT.md (both slices),
  `crates/gauntlet-app/src/config/gauntlet_md.rs` (front-matter parser to
  reuse), `crates/gauntlet-app/src/roles/definition.rs` (reuse precedent),
  `crates/gauntlet-app/tests/skills.rs` (existing task18 test, must keep
  passing), `crates/gauntlet-app/src/formation/mod.rs` (deny-by-default
  validation module precedent, teeth-test style), `crates/gauntlet-app/src/
  lib.rs` (pub mod list — `skills` slots between `settle` and `state`).
  Confirmed via `tail` of `.agents/reports/task19/b1-content-progress.md`
  and `ls .agents/skills/` that Slice A is still IN PROGRESS (only
  `wayfinder-gauntlet/` and unrelated `jolli*/` dirs exist on disk right
  now) — this is the expected parallel-timing state per IN-FLIGHT.md, not a
  bug. Confirmed `.agents/skills/wayfinder-gauntlet/SKILL.md` front matter
  shape (`name`, `description`, `disable-model-invocation: true`) matches
  IN-FLIGHT.md decision 3 exactly.
  Checked `crates/gauntlet-app/src/config/mod.rs` for the `fixture_path`
  test helper (crate-local fixtures) — not used here; skills fixtures follow
  the workspace-root `tests/fixtures/skills/` convention like
  `tests/fixtures/formation/`, matching how `tests/skills.rs`'s `repo_root()`
  + `fixture()` helper already works.
  Confirmed `std::fs` usage in the app layer is an established pattern
  (`doctor/checks.rs`, `roles/install.rs`, `config/gauntlet_md.rs` test) —
  no boundary concern reading `SKILL.md` files directly in this module.
- Plan: new `crates/gauntlet-app/src/skills/mod.rs` with `SkillFrontMatter`,
  `SkillLintError`, `lint_skill_text`, `REPO_OWNED_SKILLS`,
  `lint_installed_skills`; add `pub mod skills;` to `lib.rs`; add tests to
  the existing `crates/gauntlet-app/tests/skills.rs` plus a new fixture
  `tests/fixtures/skills/control-logic-violation.md`.
- Wrote `crates/gauntlet-app/src/skills/mod.rs` (schema validation,
  control-logic/secret/absolute-path scanners, `REPO_OWNED_SKILLS`,
  `lint_installed_skills`; a documented, narrow key-normalization bridge
  for the external SKILL.md convention's hyphenated
  `disable-model-invocation` key, since `gauntlet_md`'s bare-identifier
  key grammar has no hyphen — see the module doc comment's "Hyphenated
  front-matter keys" section), added `pub mod skills;` to `lib.rs`, wrote
  `tests/fixtures/skills/control-logic-violation.md`, and added the five
  tests to `tests/skills.rs`.
- `cargo check -p gauntlet-app --all-targets` — clean on first try.
- `cargo nextest run -p gauntlet-app` — 430/430 passed, including
  `wayfinder_gauntlet_conformance` and every new `skills::lint_*`/`skills::
  tests::*` test. Confirmed by this point that Slice A had already landed
  all six `.agents/skills/*` directories (checked `ls .agents/skills/`
  again), so `lint_passes_every_installed_skill` exercised real content,
  not just the earlier partial state.
- Discovered the exact gate command `cargo nextest run -p gauntlet-app
  skills::lint` returned "no tests to run" against the tests as first
  written (flat, top-level in `tests/skills.rs`). Investigated: `cargo
  nextest run --help` documents the positional filter as a "test name
  filter"; empirically confirmed (via `cargo nextest list`, and trying
  `-E 'test(/lint_/)'` / `-E 'binary(skills)'`) that the filter matches
  only a test's own qualified Rust name, never the binary id nextest
  shows for display. Fixed by nesting the five gate-covered tests in `mod
  skills { ... }` inside `tests/skills.rs` — re-verified the exact
  literal gate command now runs exactly those 5 tests, all passing, and
  `wayfinder_gauntlet_conformance` (untouched, still top-level) still
  passes on its own name.
- Ran `just boundaries`: found 4 hardcoded-literal violations
  (`hardcoded-url`/`hardcoded-absolute-path`, from doc-comment/test-text
  mentions of `http://`/`https://` and `/etc`, `/usr`) and 1
  `answer_key_single_writer` violation (`skills/mod.rs` names the literal
  substring `wayfinder`, via `REPO_OWNED_SKILLS`'s required
  `"wayfinder-gauntlet"` entry, and was not in the pinned
  `ANSWER_KEY_SURFACE_FILES` inventory). Investigated
  `crates/gauntlet-arch-tests/src/literals.rs` and
  `crates/gauntlet-arch-tests/src/single_writer.rs` directly (via
  `ctx_execute_file`, to keep the ~3000/~1300-line files themselves out of
  context) to find the exact matched-token lists
  (`ABSOLUTE_PATH_ROOTS`, `ANSWER_KEY_SURFACE_TOKENS`,
  `ANSWER_KEY_SURFACE_FILES`) and each check's documented false-positive
  posture and sanctioned remedy. Fixed the 4 literal violations by
  rewording the affected doc comment and swapping the two path-shaped
  test fixtures for a synthetic root name outside `ABSOLUTE_PATH_ROOTS`,
  and one URL-shaped test for a rewritten non-URL equivalent proving the
  same property. Fixed the single-writer violation by adding
  `skills/mod.rs` to `ANSWER_KEY_SURFACE_FILES` with an honest reason
  (see "Files modified" above for the full reasoning on why this was the
  correct remedy, not scope creep) — which required updating that file's
  own pin test (`the_pinned_inventories_are_pinned`) and its
  `Repo::clean` synthetic fixture (`the_clean_fixture_passes_the_check`)
  to stay internally consistent. Re-ran `just boundaries`: 228/228 green.
- `cargo fmt --check` found 8 mechanical reflow diffs in the two touched
  `gauntlet-app` files (long single-line calls rustfmt prefers wrapped, or
  vice versa) — ran `cargo fmt` once (formatting only, no semantic edits)
  and re-verified clean.
- `cargo clippy -p gauntlet-app -p gauntlet-arch-tests --all-targets -- -D
  warnings` — 0 warnings.
- Final combined re-run (`test-package gauntlet-app` → `boundaries` →
  the exact gate command) after the `cargo fmt` pass, to make sure
  formatting introduced no regression: all green, as recorded in the
  summary above. Wrote this final summary section at the top of this
  file per the task's result-contract instruction.
