# Critic 5 — Vacuity, teeth, and self-exemption review of the architecture-boundary suite

Scope reviewed as it stands: `tests/contract/{architecture_boundaries,adapter_registry,no_hardcoded_literals,env_example_complete}.rs`, `crates/gauntlet-arch-tests/src/{lib,walk,deps,denylist,literals,envfile,escapes,text}.rs`, `crates/gauntlet-arch-tests/Cargo.toml`, and the on-disk state those checks read (`.env.example`, `.agents/specs/Core-spec.md`, core crate trees). No build commands were run; all traces are by hand from source.

VERDICT: residuals-found (0 critical / 3 major / 3 minor)

---

## Major findings

### M-1. The repo-level scans trust their call-site scope predicate and exemption list; nothing pins them

**Severity:** major (one clause of AK-002/AK-014/AK-016 silently weakened by a natural edit; a bulk variant approaches wholesale defeat of AK-016/AK-002)

**Where:** `crates/gauntlet-arch-tests/src/lib.rs:192-235` (`scan_core_tree` signature), `lib.rs:369-388` (`check_denylist` passes `&[]` exemptions), `lib.rs:425-444` (`check_adapter_selection` passes `&[]`), `lib.rs:471-481` (`check_no_hardcoded_literals` passes `&[walk::DEFAULTS_MODULE]`), guard at `lib.rs:295-303`.

**Summary.** The anti-vacuity apparatus pins three layers hard: the walker (guarded against `collect_files` carve-outs by an independent listing, `walk.rs:168-195`, `lib.rs:269-282`), the walk-module predicates (`in_denylist_scope`/`in_literal_scope`/`in_adapter_identifier_scope` are pinned by `walk.rs:286-362` unit tests and by `check_defaults_exemption`'s predicate cross-comparison at `lib.rs:512-528`), and the check bodies (driven adversarially through the entry points by `evasion_tests`, `lib.rs:656-1304`). The fourth layer — the arguments each `check_*` function in `lib.rs` hands to `scan_core_tree` — is trusted input with no warrant check and no pin. The guard's "no subtree may be carved out" clause (`lib.rs:296`) explicitly forgives any file listed in `declared_exemptions`, and only `DEFAULTS_MODULE`'s exemption has a warrant (via `check_defaults_exemption`, which compares the real walk predicates, not the call-site arguments). Adding an exemption entry is the established idiom of this file; the edit looks exactly like the sanctioned pattern.

**Demonstration.** Edit `check_no_hardcoded_literals` (`lib.rs:471-481`) to:

```rust
fn narrowed_literals(rel: &str) -> bool {
    walk::in_literal_scope(rel) && rel != "crates/gauntlet-app/src/generated/wiring.rs"
}
// ... in check_no_hardcoded_literals:
    scan_core_tree(
        root,
        "hardcoded-literal",
        narrowed_literals,
        walk::CORE_CRATES,
        &["crates/gauntlet-app/src/generated/wiring.rs"],   // the "declared exemption"
        |file, text, _| literals::scan_literals(file, text),
    )
```

Then `crates/gauntlet-app/src/generated/wiring.rs` may hardcode any path/model/provider/command/timeout/tier literal AK-016 names. Trace of every test that should catch this, failing to fail:

- `assert_scan_is_not_vacuous` (`lib.rs:254-331`): clause 1 (listing vs walker) unaffected; clause 2 (`expected.len() != scanned.len()`) — both sides use the narrowed predicate, counts agree; clause 3 (`lib.rs:296`) — the file IS a declared exemption; per-crate floors (`lib.rs:306-322`) — manifests still scanned, at least one `.rs` per crate still scanned; `skipped == 0`. Silent.
- `no_hardcoded_literals::core_crates_contain_no_hardcoded_literals` and `defaults_exemption_is_warranted_or_inactive` (`tests/contract/no_hardcoded_literals.rs:8-27`), plus their in-crate duplicates (`literals.rs:1076-1094`) — call the edited entry point; green.
- `check_defaults_exemption` (`lib.rs:500-545`) — compares the real `in_denylist_scope`/`in_literal_scope` predicates, which still agree everywhere except `DEFAULTS_MODULE`; green.
- `evasion_tests` (`lib.rs:804-1029`) — drive `scan_core_tree` with their own predicates and `&[]`/their own exemptions, and their fixture files live in `src/lib.rs`, which stays in scope; green.
- `role_tests` (`lib.rs:1392-1429`) — pass their own arguments; green.

The same edit shape silences `check_denylist` (vendor terms in the exempted file pass AK-002's source scan; nothing else scans core sources for vendor terms) and `check_adapter_selection`. Because clause 3 forgives any enumerated path, a bulk variant — narrow the predicate to manifests plus one clean `.rs` per crate and enumerate the rest as exemptions (the core crates hold only 2 files each today) — disables the literal or denylist scan wholesale with all locators green. A per-file exemption is the natural-looking size; the fix is either a pin on the three call sites' exemption arguments (as `denylist.rs:536` pins `DENIED_TERMS`) or a warrant check that only `DEFAULTS_MODULE` may ever appear there.

### M-2. `check_env_example`'s root-based wrapper is the one `check_*` entry point with no adversarial fixture through it

**Severity:** major (AK-019's entire instrument can be silenced by a one-line edit with every test green; the suite's own documented standard — `lib.rs:572-580`, "driven through the real `check_*` entry points" — is met by the other five checks and missed by this one)

**Where:** `crates/gauntlet-arch-tests/src/lib.rs:548-570` (wrapper), callers: `tests/contract/env_example_complete.rs:10` and `envfile.rs:407`; all adversarial env tests use the two-argument `envfile::check_env_example(spec, env)` directly (`envfile.rs:465-677`).

**Summary.** Every other check has synthetic-workspace tests that call the root-based entry point and assert a violation appears, so deleting or neutering a clause inside it fails a test (e.g. deleting `check_patch_and_replace` fails `m5_patch_git_and_cargo_config_fail_the_gate`; deleting the root-table denylist scan fails `v4`). The env check has no such test: the only two callers of the wrapper are the locator and the in-crate repo assertion, both of which assert emptiness on the real repo. The wrapper — read spec, read `.env.example`, call the scanner — is exactly the wiring layer where the other checks' round-3 criticals lived.

**Demonstration.** Edit `lib.rs:569` from `envfile::check_env_example(&spec, &env)` to `let _ = envfile::check_env_example(&spec, &env); Vec::new()`. Tests failing to fail:

- `env_example_complete::env_example_matches_annex_e` (`tests/contract/env_example_complete.rs:9-16`) — calls the neutered wrapper, gets `[]`, asserts empty: passes.
- `envfile::repo::env_example_matches_annex_e` (`envfile.rs:406-413`) — same wrapper: passes.
- All `envfile::scanner_tests` (`envfile.rs:451-677`) — call the two-argument scanner, which is untouched: pass.
- `the_manifest_declares_exactly_the_four_answer_key_locators` (`lib.rs:1321-1380`) — the locator file still contains `#[test]` and the substring `check_`: passes.

After this edit, AK-019 is unenforced in fact: `.env.example` can lose variables or carry credential-shaped values with the AK-019 locator green. A regression test that builds a synthetic spec/env pair on disk and calls `arch`-level `check_env_example(&root)` — the exact pattern `Repo::clean` provides for the other checks — closes it.

### M-3. `check_cargo_config` closes the rustflags channel only in its `[build]` spelling; the doc claims the channel

**Severity:** major (teeth honesty: `deps.rs:791-797` documents "source replacement and `[build].rustflags` are violations ... rustflags apply to every core-crate compile" — but target-scoped rustflags and rustc wrappers apply to exactly the same compiles and are invisible; no test edit is required for the evasion, only a workspace config file)

**Where:** `crates/gauntlet-arch-tests/src/deps.rs:798-835` (`check_cargo_config` — reads only `table.get("source")` and `table.get("build").rustflags`); test `m5_patch_git_and_cargo_config_fail_the_gate` (`lib.rs:1128-1143`) and `cargo_config_source_replacement_and_rustflags_are_flagged` (`deps.rs:1421-1471`) both use only the `[build]` spelling.

**Summary.** A `.cargo/config.toml` containing

```toml
[target.x86_64-unknown-linux-gnu]
rustflags = ["--extern", "infra=vendor/libinfra.rlib", "--cfg", "injected"]
```

or `[build] rustc-wrapper = "..."` / `[build] rustc-workspace-wrapper = "..."`, redirects every core-crate compile exactly as `[build].rustflags` does. `check_cargo_config` finds no `[source]` table and no `[build].rustflags` key and reports nothing; no other code in the repository reads `.cargo/` (verified: only `deps.rs` and `lib.rs` reference it).

**Demonstration (evasion, no instrument edit needed).** Add the `[target...]` block above to `.cargo/config.toml`, put `pub use infra::Bridge;` in `crates/gauntlet-domain/src/lib.rs`. Trace: `check_dependency_direction` → `check_cargo_config(root)` parses the file, `table.get("source")` is `None`, `table.get("build")` is `None` → no violation (`deps.rs:815-832` only looks at those two tables). `check_denylist`: `infra`/`Bridge` are not denied terms; the domain `Cargo.toml` gained no dependency entry, so `check_member` sees nothing. `scope_integrity`, literal scan (the config lives outside `CORE_CRATES`), denylist: green. Every locator green while infrastructure code compiles into the domain — the exact channel class critic-4 M5 closed for two spellings. (Environment-injected `RUSTFLAGS` is inherently out of reach for a file-based suite; these spellings are file-visible and the check already claims the channel, which is why this is a defect rather than an accepted residual.) Fix: flag any `rustflags`/`rustc-wrapper` key anywhere in the parsed config, or record the narrowing in the module's residual list instead of the current unconditional wording.

---

## Minor findings

### m-1. `DEFAULTS_TABLE_SETTINGS` is the one warrant inventory without a pin

`walk.rs:57-64` defines the six §8.6 settings; `lib.rs:533` is its only consumer, and no test pins its contents (contrast: `DENIED_TERMS` pinned at `denylist.rs:536-602`, all literal inventories pinned at `literals.rs:1709-1953`, allowlists at `deps.rs:1237-1258`). Demonstration: delete `"diagnostic_budget"` from the list — `check_defaults_exemption` then warrants a defaults module declaring only five settings; when `crates/gauntlet-app/src/config/defaults.rs` lands (it does not exist today), it may omit that declaration and carry additional unscanned content (the file is exempt from the literal scan by `walk.rs:53`, and the denylist does not scan URLs or timeouts). No test fails: the real file does not exist, and the evasion fixtures at `lib.rs:1181-1234` declare all six regardless of the list. Impact is bounded — AK-016's own wording exempts the defaults table — so: minor.

### m-2. `CREDENTIAL_SHAPES` is behavior-tested only for a subset of its entries

`envfile.rs:64-80` lists 14 shapes; the tests exercise `sk-`, `ghp_`, `bearer `, `-----begin`, and `eyj` (`envfile.rs:483-500, 553-573, 594-612`). Deleting `glpat-`, `npm_`, `pypi-`, `aiza`, or `akia` leaves every test green. Residual channel: those prefixes on a comment line that is not shaped like an entry (e.g. `# glpat-xxxx` as stray prose) — `raw_credentials` is the only catcher for non-entry-shaped comment material, and `commented_entry_values` requires an `A-Z_` key shape. A pin in the style of `runtime_injected_is_exactly_the_documented_singleton` (`envfile.rs:637-649`) closes it. Minor: entry-shaped and parsed-value credentials remain caught by other clauses.

### m-3. `string_literals` mis-pairs quotes across `'"'` char literals, hiding whole-literal matches on that line

`literals.rs:932-960` documents "Every double-quoted string literal on a line"; the extractor pairs quote characters blindly. Demonstration: `scan_literals("f.rs", "let c = '\"'; let tool = \"git\";")` — the quote inside the char literal pairs with `"git"`'s opening quote, producing literals `"; let tool = "` and `";"`; the text `git` falls between two extracted spans and is never captured. Consequence: the whole-literal classes (`hardcoded-tool-identity`, `hardcoded-provider`, `hardcoded-tier-policy`, and the denylist's adapter-segment rule at `denylist.rs:330-344`) miss terms on any line that also contains a quote-bearing char literal, while `pub const SHELL: &str = "bash";` alone is caught (`literals.rs:1641-1674`). No test fails because no fixture combines both constructs. Minor: contrived shape, substring-based classes (denylist vendor terms, paths, URLs) are unaffected.

---

## Probed and found SOUND (with evidence)

- **The walker is guarded by a genuinely independent listing.** `walk::unfiltered_listing` (`walk.rs:168-195`) shares no code with `collect_files` (`walk.rs:103-153`) and has no exclusion logic to shrink; the guard compares them (`lib.rs:269-282`) and a walker-level carve-out fails every repo-level check through the shared path — pinned by `a_walker_level_carve_out_fails_the_gate` (`lib.rs:888-928`) and `the_independent_listing_agrees_with_the_walker_on_the_real_repo` (`walk.rs:380-415`). Unreadable directories panic in the listing (`walk.rs:181`) — fail-closed.
- **Walk-module predicate narrowing fails the gate.** Guard clause at `lib.rs:295-303`; pinned by `scope_decisions_match_the_declared_exclusions` (`walk.rs:286-339`), `v15_a_narrowed_adapter_identifier_scope_fails_the_gate` (`lib.rs:810-845`), and `a_narrowed_denylist_or_literal_scope_fails_through_the_shared_scan_path` (`lib.rs:851-881`). The guard lives inside `scan_core_tree` (`lib.rs:226-233`), so deleting it means deleting it for every check at once — the design response to the old per-check deletable guard.
- **`CORE_CRATES` membership cannot silently shrink.** Domain/app/cli are pinned directly (`walk.rs:288, 307, 290`); `gauntlet-ports` is pinned transitively — removing it from `CORE_CRATES` makes the walker stop listing it while `check_adapter_selection`'s per-crate floor over `INNER_CRATES` (`lib.rs:306-313`) still demands `crates/gauntlet-ports/Cargo.toml` be scanned, firing "adapter-selection: scope is missing ..."; removing it from `INNER_CRATES` as well fails `walk.rs:346`. I verified this trace before abandoning the candidate finding.
- **Fail-closed reads everywhere probed.** `read_scannable` errors are violations, never skips (`walk.rs:207-234`, pinned `walk.rs:418-450`); `skipped != 0` is itself a violation (`lib.rs:323-329`); unreadable dirs in the walker and in package discovery are violations (`walk.rs:104-116`, `deps.rs:691-704`); malformed manifests are violations in all three consumers (`denylist.rs:367-372`, `escapes.rs:380-386`, `deps.rs:553, 626-633`); unreadable spec/`.env.example` are violations (`lib.rs:551-568`); malformed `.cargo/config` is a violation (`deps.rs:805-813`); the old `unwrap_or_default` alias map now fails closed (`lib.rs:338-352`); the one remaining `unwrap_or_default` (`lib.rs:531`, defaults-module text) fail-closes because empty text fails every `const <setting>` warrant check.
- **Check-body wiring is adversarially pinned through the real entry points** (the round-3/critic-4 standard): deleting `check_patch_and_replace` → `m5a` (`lib.rs:1115-1127`); `check_cargo_config` → `m5b` (`lib.rs:1128-1143`); `undeclared_packages` → `v5a` (`lib.rs:1150-1163`); root-table entries → `v5b`/`c1c`/`c1d` (`lib.rs:1163-1175, 1073-1109`); registry/git deps in members → `c1a`/`c1b` (`lib.rs:1037-1072`); the root `[workspace.dependencies]` text scan → `v4` (`lib.rs:758-785`); scope escapes → `v1`/`v2`/`v3` (`lib.rs:676-755`); literal classes → `v16`/`v17`/`v6`/`v19`/`critic4_literal_evasions` (`lib.rs:931-1029, 1239-1304`); the defaults warrant → `v2` fixtures (`lib.rs:1181-1234`).
- **Self-exemptions are narrow, justified, and guarded.** The arch-tests crate is outside `CORE_CRATES` by construction (stated at `lib.rs:38-40`, `walk.rs:249-252`) and cannot silently join a core scope (adding it would fail its own denylist terms). The composition-root carve-out is a mask, not a skip, with splice protection (`denylist.rs:262-270`, pinned `denylist.rs:732-784`), justified against §4.1, and cross-checked so cli can never leave the vendor/literal scopes (`lib.rs:455-466`). `TARGET_PATH_EXEMPT_CRATES` is exactly the boundary suite and the rule provably applies to all four core crates (`escapes.rs:745-757`). `RUNTIME_INJECTED` is a justified singleton pinned both directions (`envfile.rs:55-59, 326-342, 637-649`). The empty external allowlists are themselves pinned (`deps.rs:1237-1258`).
- **Derived inventories cannot go vacuous.** Empty adapter derivation fails both denylist checks (`lib.rs:358-364, 408-414`) and the adapter-presence interlock in `check_dependency_direction` (`lib.rs:115-120`) stops the workspace from shedding adapters wholesale; empty `[workspace.dependencies]` is flagged (`lib.rs:131-135`); the env floor is derived from spec-wide tokens with lockstep-deletion coverage (`envfile.rs:281-313, 652-677`) and the annex parser fails closed if the heading spelling drifts (empty annex → every expected variable "missing from the annex").
- **Pinned inventories make lockstep edits visible, not silent.** `DENIED_TERMS` (`denylist.rs:536-602`), every literal term list (`literals.rs:1709-1953`), the allowlists (`deps.rs:1237-1258`), `RUNTIME_INJECTED` (`envfile.rs:637-649`), `TARGET_PATH_EXEMPT_CRATES` (`escapes.rs:755-756`), and the `[[test]]` manifest blocks (`lib.rs:1321-1380`) are adjacent `assert_eq` pins — a single-commit lockstep edit remains possible (governance), but it is visible in diff review, which is the mechanical standard.
- **Locator redundancy.** Each of the four `tests/contract/` locators is duplicated by an in-crate `repo` test over the real tree (`denylist.rs:460-479`, `deps.rs:884-893`, `literals.rs:1076-1094`, `envfile.rs:405-413`, `escapes.rs:775-784`), so weakening a locator file alone does not silence enforcement; the manifest pin prevents locator rot (deleting a `[[test]]` block fails `lib.rs:1321`). The manifest pin's `contains("check_")` is weak on its own (a comment satisfies it) but is backed by that redundancy.
- **Normalization and role wiring.** `text.rs` four-form scanning is heavily pinned including the split-before-`=` and whitespace-attribute shapes; the `MAX_STATEMENT_LINES = 24` ceiling is disclosed at `text.rs:118-120` (physical lines remain scanned). Role assignment inversion is pinned twice (`lib.rs:1392-1429, 1436-1449`).
- **`Cargo.toml` channels.** Renames, workspace inheritance, target-scoped sections, `[patch]`/`[replace]`, and source replacement are all resolved and fail closed (driven by `c1a-d`, `m5a/b`, `v9`).

## Probe list (everything exercised, including negatives)

Walker-vs-listing independence (both functions read in full; no shared filter); predicate narrowing at walk-module level and at call-site level (the latter is M-1); `CORE_CRATES`/`INNER_CRATES` membership edits (all four crates, both constants — ports caught transitively, others directly); `DEFAULTS_MODULE` exemption width, warrant shape, and sibling files; composition-root role and mask; `.cargo/config.toml` spellings (M-3 plus `[registries]`, `[env]`, patch `directory =` — the latter two fail closed or out of repo scope); every `unwrap_or_default`/`unwrap_or`/`continue`-on-error in the crate (one benign, listed under sound); env parser heading drift, empty parses, lockstep deletion, runtime-injected growth; inventory pins (found the two unpinned ones, m-1/m-2); evidence-locator manifest and file pins; check-entry-point neutering for all six `check_*` functions (five caught by evasion tests, one not — M-2); char-literal quote mispairing in `string_literals` (m-3); symlink handling in both walks; oversized/non-UTF-8/unreadable files; statement-join ceiling.
