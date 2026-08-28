# Task 13 / Builder B3 result contract — single-writer arch rule + doctor/containment pins

Status: COMPLETE. All gates green. Ruler untouched (`just ruler` PASS before and
after). No sub-agents, no parallelism changes. `.wayfinder/`, reference lock,
deny.toml, nextest config, AGENTS.md, `.jolli/`, `.tmp/`, `.agents/prompts/`
untouched. The pre-existing uncommitted modifications in
`crates/gauntlet-app/src/control/controller.rs` and `settle/mod.rs` are B2's;
my tooth edits to those files were cmp-verified byte-identical reverts.

## Files changed

| file | change |
|---|---|
| `crates/gauntlet-arch-tests/src/single_writer.rs` | NEW — the rule module (inventories, production-text derivation, check, pins, P5 battery) |
| `crates/gauntlet-arch-tests/src/lib.rs` | `pub mod single_writer`; entry `check_answer_key_single_writer(root) -> Vec<String>`; doc-table and mechanisms rows |
| `tests/contract/architecture_boundaries.rs` | `answer_key_single_writer` test on the EXISTING structural-rule target (no new repo-root target; the four-locator manifest pin is untouched) |
| `crates/gauntlet-app/src/doctor/mod.rs` | NEW test `an_answer_key_inside_the_workspace_only_canonically_still_fails_check_seven` (Item 3; test-only change) |

## Item 1 — the single-writer / no-writer arch rule (spec §16.4, AK-043 leg a)

Three rules, all pinned by inventories with reasons (suite conventions:
central declarations, exact-equality pin tests, anti-vacuity through the
shared `scan_core_tree` guard, fail-closed parsing):

- **A — surface inventory.** Production modules naming the answer-key
  surface (tokens: `wayfinder`, `AnswerKeySource`, `ANSWER_KEY_NAME`,
  case-insensitive) are pinned to exactly the six `gauntlet-app` files the
  orchestrator's recon named (`config/defaults.rs`, `control/controller.rs`,
  `reference/{provider,manifest,parse,mod}.rs`) — verified against the tree
  before pinning. A new surface file fails; a stale entry fails.
- **B — no unguarded writer at the surface.** DEVIATION from the brief's
  literal shape, forced by the tree: `controller.rs` is BOTH a surface holder
  (`AnswerKeySource`) and a guarded writer (facet-locator materialization at
  :2369/:2388, behind `contained_locator_path`). The recon's "exactly ONE
  site family (doctor)" is stale — the controller sites cite critic-B
  A12/N4. An absolute Rule B would be red on the current tree. Implemented
  as: a write token in a surface module must be pinned in
  `WRITE_API_INVENTORY` WITH a containment guard present in the same file's
  production text (a lexical rule cannot see WHICH path a write targets; the
  guard is the warrant). The reference side (`provider/parse/manifest/mod`,
  `defaults`) is write-free outright. New unguarded writes at the surface —
  including in `provider.rs` — fail (T1 proves it).
- **C — write inventory.** EVERY production write-API token across the core
  crates is pinned: 7 tokens in 3 site families (controller x2 guarded by
  `fn contained_locator_path`; doctor report writer x2 guarded by
  `fn write_report`; doctor check-7 writable probe x3 guarded by
  `fn probe_writable`). Token set (documented, ident-boundaries both sides):
  `fs::{write, remove_file, remove_dir, remove_dir_all, rename, copy,
  create_dir, create_dir_all, hard_link, soft_link, symlink, link}`,
  `File::create`, `OpenOptions` (the type name — any construction is
  inventory material; reads have `File::open`).

**Production text** = `.rs` under a core crate's `src/`, minus
`#[cfg(test)]`-gated module regions (blanked with regions-lexed brace
balancing, newlines preserved so line numbers stay true), minus
`#[cfg(test)]`-gated functions (blanked through the body — discovered LIVE
by the fail-closed clause on `config/mod.rs::fixture_path`, not by my
survey), minus cross-file semicolon-gated modules (`mod NAME;` →
`<dir>/NAME.rs` excluded — the `ports/src/contract` shape). Integration
tests (`<crate>/tests/**`) are outside `src/` by cargo's own target model.
Anything unclassifiable is a violation, never a guess (I8): wrongly
blanking production text would be an evasion hole.

**Teeth (run personally; each break→RED→revert):**
- T1 `std::fs::write` appended to provider.rs production → RED:
  `provider.rs:667 uses write-API token 'fs::write' in an answer-key surface
  module … no unguarded write at the surface` + Rule C extra. Reverted,
  `cmp` byte-identical, green.
- T2 new `reference/t2_tooth.rs` naming `AnswerKeySource` → RED: Rule A
  "names the answer-key surface … not in the pinned inventory". Removed
  (untracked ⇒ removal is byte-identity), green.
- T3 production write in `config/resolve.rs` → RED: Rule C
  `resolve.rs:2036 new production write-API token 'fs::write'`. Reverted,
  `cmp` byte-identical, green.
- Guard tooth (extra, Item 4): `contained_locator_path` fully renamed in
  controller.rs → RED with 5 violations (2 × guard-absent, 3 ×
  unwarranted-surface-write). Reverted, `cmp` byte-identical, green. Note:
  a first weak tooth (`…_renamed`) stayed green because the needle is a
  substring — the recorded tooth uses a fully different name.

**P5 masked-spelling battery (honesty, recorded in module docs and asserted
in tests):**
- env-injected path (`env::var → PathBuf → fs::write`): path provenance
  invisible; write token still fires Rule C. CAUGHT AT THE TOKEN.
- `format!`-constructed surface mention: Rule A does not see the file
  (GENUINE RULE A ESCAPE, pinned as observed); any write in it still fires
  Rule C.
- generic `impl AsRef<Path>` helper funnel: surface file stays
  token-free; helper's token fires Rule C wherever it lives in a core
  crate. CAUGHT AT THE TOKEN.
- `dyn Writer` with the impl in an ADAPTER: no core token at all — GENUINE
  ESCAPE; authority/containment layers and B4's g10 proof own that channel.
- Also recorded: aliased imports (`use std::fs::write as w`), macro-internal
  token concatenation, and the `GAUNTLET_ANSWER_KEY_PATH` env-var name
  (cli `spec_vocab.rs`) being deliberately outside the surface tokens (it
  names an Annex E variable, not an owning module; carries no write API).

Boundary-suite growth: 197 → **215** (pinned; +18 = 1 contract-target test
+ 17 in-crate: pins, blanker discrimination, battery, teeth-equivalents).

## Item 2 — re-litigation record

Scope-split 2026-08-26 honored: `arch-relitigation.md` was ABSENT at close,
so per the brief I ignored Item 2 entirely.

## Item 3 — doctor check 7 pin (N4)

- Verified REAL and non-vacuous: `check_answer_key_outside`
  (`doctor/checks.rs:1054`) does a component-wise lexical prefix check AND a
  canonicalized-filesystem prefix check, and treats canonicalization
  FAILURE as a finding (Fail), never a pass.
- Existing coverage pinned the lexical-inside refusal
  (`an_answer_key_inside_the_workspace_fails_check_seven`). The
  canonicalized-inside leg was UNPINNED → ADDED the discriminating test
  (symlink alias: key lexically outside `real-ws`, canonically inside).
  Mutation-proven: `canonical_inside` forced `false` ⇒ test RED
  (`left: Pass, right: Fail`); reverted cmp-identical; doctor module 29/29.
- Clause-vs-spec note (recorded, not widened): the spec sentence says "not
  inside any builder-writable workspace"; the implementation checks THE
  configured workspace root, with the same check's writable probes covering
  the state/evidence/workspace dirs. No behavior change made.

## Item 4 — AK-043 prevention substrate pin

- ADDED (arch-level, in scope): the Item-1 guard clause — the controller's
  two inventoried writes carry guard needle `fn contained_locator_path`;
  losing/renaming the containment surface fails `just boundaries`
  (real-repo tooth above). This is the structural pin that the loop's
  builder-side write cannot detach from its containment warrant.
- CITED (verified green, not duplicated): controller test
  `an_escaping_evidence_locator_is_refused_at_the_write_boundary`
  (1/1 — typed refusal, `..`-locator, zero bytes written outside);
  ports contract suite `workspace_provider_suite` escape case via
  `suite_runs` (2/2 — Task-9: an allocated root escaping the configured
  root fails the shared suite); agent-mock
  `effect_paths_are_contained_at_parse_time` (1/1) plus the runner's
  at-use re-check (`runner.rs:254-266`, two independent guards — Task-8).
- The mock runner's at-use escape branch is unreachable through the public
  parser (the parser guard fires first) — defense-in-depth by design; noted
  rather than force-testing a private API.

## Gates (verbatim tails)

```
just ruler      -> verify-ruler.sh: PASS — answer key and manifest digests match the frozen lock (/home/sprime01/projects/gauntlet/.gauntlet/reference.lock)
just test-package gauntlet-arch-tests -> Summary [ 24.547s] 215 tests run: 215 passed, 0 skipped
just boundaries -> Summary [ 24.421s] 215 tests run: 215 passed, 0 skipped   (new count pinned: 215)
just check      -> Finished `dev` profile [target(s)] in 15.15s
just lint       -> PASS (cargo clippy --workspace --all-targets -- -D warnings; cargo fmt --check)
```

## Honest scope of claims

The rule is a lexical pin of modules and write-API tokens over the core
crates' production text — NOT a data-flow analysis; the residuals above are
recorded in the module docs, not hidden. "No issue found" means none within
this rule's declared scope (core crates, production text, the documented
token set) under the bounded battery run. Doctor and containment behavior
were pinned, not changed.
