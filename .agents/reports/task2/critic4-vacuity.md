# Critic 4 — Vacuity, teeth, and self-exemption review of the architecture-boundary suite

VERDICT: residuals-found (0 critical / 8 major / 2 minor)

Artifact reviewed as it stands: `tests/contract/{architecture_boundaries,adapter_registry,no_hardcoded_literals,env_example_complete}.rs`, `crates/gauntlet-arch-tests/src/{lib,walk,text,denylist,deps,escapes,literals,envfile}.rs`, `crates/gauntlet-arch-tests/Cargo.toml`, root `Cargo.toml`, `justfile`, against `.wayfinder/answer-key.md` AK-001/AK-002/AK-014/AK-016/AK-019 and Core-spec §4.1, §8.1, §24.3, Annex E.

No criticals: I could not construct a single natural edit that defeats any named Unit **wholesale** with every locator green — the layered guards (predicate pins, per-crate floors, member-set floors, derivation vacuity checks) genuinely bite at more than one depth. What remains are clauses that can be silently weakened or wiring whose deletion goes unpunished, each with a concrete trace below.

---

## Major findings

### M1 — The anti-vacuity guard asserts against the walker, not the real tree; a walker-level carve-out is invisible to every guard

**Where:** `crates/gauntlet-arch-tests/src/lib.rs:240-247` (`assert_scan_is_not_vacuous` computes `policed` from a fresh `walk::core_tree(root)` — the same walker that produced `report.scanned`); `crates/gauntlet-arch-tests/src/walk.rs:103-153` (`collect_files`, which decides what exists).

**Summary:** The guard's universe (`tree.files`) and the scan's universe are the same function. The lib.rs doc (lines 68-71, 221-224) promises the post-filter set is asserted to "cover every file under `crate_dirs`", but coverage is computed relative to the walker's output, never against an independent enumeration of the filesystem. Any exclusion added to the *walker* shrinks both sides in lockstep and nothing fails. The predicate-level guard (R30/V13/V15) is real, but it polices the last filter in a chain whose first filter is unpoliced.

**Demonstration:** Edit `walk.rs:collect_files` to add, after the symlink branch:

```rust
if file_type.is_dir() && entry.file_name() == "generated" { continue; }
```

Then add `crates/gauntlet-app/src/generated/wiring.rs` containing `pub const STORE: &str = "sqlite";` and `pub const ROOT: &str = "/tmp/gauntlet/state";`.

Trace — every test that should fail, failing to fail:
- `architecture_boundaries::denylist` — `core_tree` never lists `wiring.rs`, so `scan_core_tree` never reads it; no `denied-identifier` violation.
- `no_hardcoded_literals::core_crates_contain_no_hardcoded_literals` — same: the file is outside both the scan and `assert_scan_is_not_vacuous`'s `policed` set, so neither the count check (lib.rs:249) nor the "no subtree may be carved out" check (lib.rs:258-266) sees it, because `policed` is filtered from the same shrunken `tree.files`.
- The per-crate floor (lib.rs:269-285) still passes: `crates/gauntlet-app/Cargo.toml` is scanned and `src/lib.rs` satisfies "at least one Rust source".
- `walk::scope_tests::scope_decisions_match_the_declared_exclusions` (walk.rs:272-277) still passes: it pins the *predicate* `in_denylist_scope("crates/gauntlet-app/src/generated/wiring.rs") == true`, and the predicate is unchanged — the carve is in the walker.
- The evasion tests (lib.rs:586-876) build their own fixtures and never exercise the real repo walk.

Today's core crates contain no subdirectories (`find crates/gauntlet-{domain,ports,app,cli}` shows only `Cargo.toml` + one `src` file each), which is why this hole has never fired — and why it is purely latent teeth: the first real submodule tree can be carved out invisibly. An independent enumeration (e.g. `std::fs::read_dir` recursion in the guard, or asserting `tree.files` against a second walker with no exclusion logic) would close it.

### M2 — The literal-scan exemption is warranted for exactly one file by name-mention only; additional exemptions are warrant-free once `defaults.rs` exists

**Where:** `crates/gauntlet-arch-tests/src/lib.rs:423-431` (`check_no_hardcoded_literals` passes `&[walk::DEFAULTS_MODULE]` as the trusted `declared_exemptions`), `lib.rs:438-475` (`check_defaults_exemption` warrants only `DEFAULTS_MODULE`, and only by `lowered.contains(setting)` for six setting names — lib.rs:449-459), `walk.rs:53, 211-213` (`in_literal_scope`'s exemption set).

**Summary:** Two weaknesses in one mechanism.

(a) *The warrant is a mentions-test, not a table-test.* `check_defaults_exemption`'s existence branch verifies only that the file's lowercased text contains the substrings `max_rounds`, `max_parallel`, `max_depth`, `max_wall_seconds`, `recovery_reserve_pct`, `diagnostic_budget`. The doc comment (lib.rs:435-437, walk.rs:50-52) says the file must "be the spec §8.6 defaults table"; the code cannot tell a defaults table from a comment that names one. This is demonstrable **today**: create `crates/gauntlet-app/src/config/defaults.rs` whose entire content is

```rust
//! max_rounds max_parallel max_depth max_wall_seconds recovery_reserve_pct diagnostic_budget
pub const API_BIND: &str = "localhost:8080";
pub const BUILDER_CMD: &str = "cargo nextest run";
pub const STALL_MS: u64 = 900_000;
```

`check_defaults_exemption` passes (all six names mentioned); `in_literal_scope` excludes the file, so `check_no_hardcoded_literals` never reads it; the denylist still applies but none of these are vendor terms. `no_hardcoded_literals::core_crates_contain_no_hardcoded_literals` and `defaults_exemption_is_warranted_or_inactive` both stay green while three AK-016 classes live in a file that is not a defaults table.

(b) *The exemption list is unguarded past its first entry.* While `defaults.rs` does **not** exist (today's state), the inactive-branch cross-check (lib.rs:464-473) catches any existing file that differs between the two scopes — good. But the moment `defaults.rs` exists (plan Task 5), that branch is dead: the existence branch returns at lib.rs:460 after checking only `DEFAULTS_MODULE`. The natural-looking edit at that point —

- `walk.rs:212`: `in_denylist_scope(rel) && rel != DEFAULTS_MODULE && rel != "crates/gauntlet-app/src/config/tables.rs"`
- `lib.rs:429`: `&[walk::DEFAULTS_MODULE, "crates/gauntlet-app/src/config/tables.rs"]`

— exempts `tables.rs` from the literal scan with **no warrant check at all**: `check_defaults_exemption` never examines it, the vacuity guard accepts it because it is now a "declared exemption" (lib.rs:259), and the walk unit test pins `config/parser.rs` but not `tables.rs` (walk.rs:289-292). AK-016 silently weakened, every locator green.

### M3 — The curated term lists can shrink in lockstep with their own iterating tests; the "do not delete the term" instruction has no mechanical teeth

**Where:** `crates/gauntlet-arch-tests/src/denylist.rs:82-143` (`DENIED_TERMS`), `literals.rs:72-95, 135-231` (`MODEL_FAMILIES`, `COMMAND_HEADS`, `TOOL_IDENTITIES`, `WELL_KNOWN_PORTS`, …), and the tests that iterate them: `denylist.rs:502-517` (`every_denied_term_is_flagged_in_source`), `literals.rs:959+`.

**Summary:** `every_denied_term_is_flagged_in_source` is the only test that touches most of `DENIED_TERMS`, and it iterates the list itself — delete a term and the test shrinks with it and stays green. The module doc anticipates exactly this edit and forbids it in prose ("do not delete the term", denylist.rs:76-77), which is a comment, not a gate. I verified by exhaustive grep that at least these DENIED_TERMS entries appear in **no fixture anywhere** in the crate: `mongodb`, `sqlx`, `sled`, `diesel`, `postgres`, `mysql`, `ureq`, `hyper`, `podman`, `bwrap`, `bubblewrap`, `firejail`, `nsjail`, `chatgpt`, `gemini`, `copilot`, `mistral`, `grok`, `groq`, `moonshot`, `cohere`, `openrouter`, `ollama`, `bedrock`, all three `vertex*` spellings, `firefox`, `webkit`, `gecko`, `playwright`, `selenium`, `puppeteer`, `safari`, `openobserve`, `qwen`, `llama`. (Six are pinned by the local list in `spec_named_backends`, denylist.rs:871; the rest are pinned only by their presence in the array the iterating test walks.)

**Demonstration:** Delete `"mistral",` from `DENIED_TERMS` (denylist.rs:118). Trace: `every_denied_term_is_flagged_in_source` iterates the smaller list and passes; no fixture anywhere contains `mistral`; `architecture_boundaries::denylist` passes. Then `pub const CRITIC_PROVIDER: &str = "mistral";` in `crates/gauntlet-app/src/lib.rs`, or `mistral = "1"` as a dependency key in `crates/gauntlet-domain/Cargo.toml`, is clean under AK-002's vendor clause. The same lockstep applies to the unpinned majority of `COMMAND_HEADS`/`TOOL_IDENTITIES` (`kubectl`, `terraform`, `ssh`, `curl`, `docker`, …) against AK-016's command and tool-identity classes. A membership pin (a test asserting the list equals a frozen expected vector, or a count floor with a recorded change procedure) is what the denylist module's own philosophy demands and does not have.

The derived half of the denylist (adapter identifiers from workspace members) is **not** subject to this — it is derived, and `check_denylist`/`check_adapter_selection` fail loudly if derivation yields nothing (lib.rs:309-315, 357-363). The erosion surface is the curated vendor half only.

### M4 — `RUNTIME_INJECTED` is a growable, unpinned exclusion list

**Where:** `crates/gauntlet-arch-tests/src/envfile.rs:41-45`, consumed at `envfile.rs:235-243` (floor filter) and `envfile.rs:271-287, 316-317`.

**Summary:** The `GAUNTLET_RESULT_PATH` exclusion is named, justified, and enforced against staleness in both directions (test at envfile.rs:463-485) — that part is sound. But nothing pins the list to that one entry. Growing it converts a spec-recognized variable into an unlisted one with the AK-019 locator green.

**Demonstration:** Edit `envfile.rs:41-45` to add `("GAUNTLET_MAX_WALL_SECONDS", "spec §11.2: recomputed per iteration by the orchestrator")`; delete the `GAUNTLET_MAX_WALL_SECONDS` token from Annex E (spec line ~2094) and its line from `.env.example`. Trace: `spec_gauntlet_variables` still finds the name (§8.2 YAML example, §8.3 group list), but `expected` filters it out via `RUNTIME_INJECTED` (envfile.rs:235-242), so `missing_from_annex` does not fire; the annex-staleness loop (envfile.rs:271-277) does not fire because the name is no longer in the annex; the entry loop's `is_known` (envfile.rs:316-317) accepts its absence from `.env.example`. `env_example_complete::env_example_matches_annex_e` passes while a variable the spec still names in its canonical §8.3 set is absent from `.env.example` — a direct AK-019 violation ("listing every recognized GAUNTLET_ variable") with all locators green. The fix shape is the same as M3: pin the list (assert it equals the documented singleton, and assert every entry is absent from `spec_wide`).

### M5 — Two wiring calls inside `check_dependency_direction` have no entry-point test; deleting either kills an AK-001 clause with the suite green

**Where:** `crates/gauntlet-arch-tests/src/lib.rs:125` (`violations.extend(deps::undeclared_packages(&discovered.dirs, &members))`) and `lib.rs:136-139` (`violations.extend(deps::check_workspace_dependency_paths(&root_table, &members))`).

**Summary:** The evasion tests were built to pin wiring (v9 pins `check_patch_and_replace` through the real entry point), but these two clauses were left with only direct unit tests (`deps.rs:824-841`, `deps.rs:844-855`), which call the functions in isolation and keep passing after the calls are deleted.

**Demonstration:** Delete lib.rs:125. Trace: create `crates/gauntlet-infra/Cargo.toml` (a package directory on disk, absent from `[workspace].members`). `dependency_direction` — the locator test `architecture_boundaries::dependency_direction` — no longer reports the undeclared package; `deps.rs`'s `undeclared_package_directory_is_flagged` still passes because it tests `undeclared_packages` directly; no evasion fixture ever created an undeclared directory (`Repo::clean` writes only declared members). Cargo auto-includes the in-tree path dependency, code compiles into `gauntlet-domain` from outside every scan scope — the exact AK-001 class A1 evasion the module documents (deps.rs:27-40) — with every locator green. Deleting lib.rs:136-139 likewise kills the root-`[workspace.dependencies]`-path clause with only the direct unit test remaining.

### M6 — The anti-vacuity guard invocations are themselves unguarded wiring

**Where:** the `assert_scan_is_not_vacuous` calls at `lib.rs:331-338` (denylist), `lib.rs:391-398` (adapter selection), `lib.rs:423-430` (literals); the v15 test at `lib.rs:740-775`.

**Summary:** v15 verifies the guard *function* bites by calling `assert_scan_is_not_vacuous` directly with a hand-built narrowed predicate. No test verifies that `check_adapter_selection` (or the other two checks) actually applies the guard to **its own** predicate. The guard call is therefore deletable wiring.

**Demonstration:** Delete lib.rs:391-398 and narrow the scan at its call site (lib.rs:371):

```rust
let report = scan_core_tree(root, |f| walk::in_adapter_identifier_scope(f) && !f.contains("/registry/"), |file, text, _| { ... });
```

Trace: `adapter_registry::selection_is_config_driven` scans the narrowed set with no guard — green on the real repo (which currently has no `registry/` files: `crates/gauntlet-app/src/` holds only `lib.rs`); `walk`'s `adapter_identifier_scope_is_the_inner_layers_only` (walk.rs:300-320) still passes because its asserted paths contain no `/registry/`; the role test `every_scanned_file_gets_the_role_its_layer_requires` (lib.rs:888) still passes because role assignment inside `scan_core_tree` uses the unchanged `walk::in_adapter_identifier_scope` (lib.rs:198); v15 still passes because it calls the helper directly and never goes through `check_adapter_selection`. AK-014 is then disabled for any future `crates/gauntlet-app/src/registry/**` subtree with every locator green. (For `check_denylist`/`check_no_hardcoded_literals` the same deletion is only partially masked — the walk unit test pins `/generated/`, `/vendor/`, `/fixtures/`, `/target/` carve-outs specifically, walk.rs:272-279 — so a differently-named carve plus guard deletion is equally green.) An entry-point test per check ("narrow this check's predicate by one exclusion and assert the check fails") is the missing shape; v15 is one line away from being it and stops short.

### M7 — The evidence-locator declarations are unpinned; a one-block manifest edit rots an answer-key locator with the suite green

**Where:** `crates/gauntlet-arch-tests/Cargo.toml:22-36` (the four `[[test]]` blocks that give `tests/contract/*` compiled existence); nothing anywhere reads or verifies this manifest's test-target table.

**Summary:** The repo root is a virtual manifest, and `tests/contract/` sits outside the `gauntlet-arch-tests` package directory, so those four files compile **only** through the explicit `[[test]] path` declarations. Delete one block — a plausible manifest cleanup, since the files "look" auto-discovered — and cargo silently stops compiling that locator's file.

**Demonstration:** Delete the `[[test]] name = "env_example_complete"` block. Trace: `cargo nextest run -p gauntlet-arch-tests` (the `just boundaries` gate) stays green; the answer-key locator `tests/contract/env_example_complete` now resolves to a file that never runs — dead code that still carries the doc comment claiming to be the instrument. Enforcement survives only because `envfile.rs:350-358` duplicates the check as a lib test; that duplication is itself unpinned (deleting the duplicated `mod repo` test plus the `[[test]]` block leaves AK-019 wholly unenforced with everything green — `.env.example` could then lose variables or gain a credential with no gate at all). The same rot applies to the other three locators, each rescued only by an unpinned duplicate in the corresponding `mod repo`. A test asserting the manifest declares exactly the four named targets at the expected paths would pin the evidence contract the answer key depends on.

### M8 — AK-001's "infrastructure" clause is enforced only by name enumeration; an unlisted external crate compiles into the domain with all locators green

**Where:** `crates/gauntlet-arch-tests/src/deps.rs:203-248` (`declared_deps` tracks only workspace members and `path` dependencies), `denylist.rs:82-143` (the only net for external names).

**Summary:** AK-001 forbids domain compile-time dependencies on "ports, adapters, or infrastructure." Ports and adapters are workspace members and are fully covered (including renames, workspace inheritance, `[patch]`/`[replace]`, target-scoped sections). "Infrastructure" from crates.io is covered only insofar as the crate's name contains a `DENIED_TERMS` entry. The deps module's documentation records no such residual — its doc table implies the layer rules are the whole check.

**Demonstration:** Add `zbus = "5"` (or `tokio`, `systemd`, `udev`, `notify`) to `crates/gauntlet-domain/Cargo.toml` `[dependencies]`. Trace: `check_dependency_direction` — `declared_deps` sees a non-member without a `path` key, so it is neither internal nor an unresolved path; `check_member` reports nothing. `check_denylist`'s `scan_manifest` finds no denied substring. `scope_integrity` parses the manifest fine. All five locators green while the domain compiles against D-Bus (or an async runtime). No suite edit is even required — this passes as the code stands. Either an allowlist of permitted external crates per layer, or a recorded residual in deps.rs naming this gap explicitly, is needed for the clause to be honest.

---

## Minor findings

### m1 — `workspace_deps_of` fails open on an unparseable root manifest (masked today by call ordering)

`lib.rs:298-303`: `.parse::<toml::Table>().map(...).unwrap_or_default()` — a parse failure yields an empty alias map, and workspace-inherited renames would silently resolve to nothing inside `scan_manifest`. Today this is unreachable in practice because `check_denylist` calls `members_of(root)` first (lib.rs:308), which panics — fails closed — on an unparseable root manifest. It is a latent fail-open kept shut only by call order; nothing pins that order.

### m2 — Annex-only variables can leave the floor in lockstep (requires editing the spec, i.e. moving the ruler)

`envfile.rs:117-128`: the derived floor counts `GAUNTLET_` tokens anywhere in the spec. Variables named **only** in Annex E (verified for the Promotion group `GAUNTLET_PROMOTION_MARGIN` and Test hooks `GAUNTLET_CLOCK`/`GAUNTLET_ID_SEED` — their only spec occurrences are Annex E lines 2097-2098) shrink `spec_gauntlet_variables` too when deleted from the annex, so an annex+`.env.example` lockstep deletion is green. Since the spec is this instrument's declared source of truth, that is a ruler move rather than a suite defect — but the module docs present the derived floor as lockstep-proof without noting it protects only §8.3-named variables.

---

## Probed and found SOUND (with evidence)

- **Predicate-exclusion guard bites.** Narrowing `in_denylist_scope`/`in_literal_scope`/`in_adapter_identifier_scope` for any file present in the walked tree fails the corresponding locator via `assert_scan_is_not_vacuous`'s "no subtree may be carved out" check (lib.rs:258-266) and, for the pinned paths, the walk unit tests (walk.rs:272-293 pin `/generated/`, `/vendor/`, `/fixtures/`, `/target/`; walk.rs:300-320 pin the inner-vs-composition split). V15 (lib.rs:740-775) demonstrates the guard turning a narrowed predicate's silence into a failure.
- **Floors are derived, not today's count.** Per-crate floor requires each policed crate's manifest and ≥1 Rust source in the scanned set (lib.rs:269-285); member floor requires the five named crates plus ≥1 adapter (lib.rs:102-120); env floor derives from spec-wide tokens minus named exclusions (envfile.rs:226-258), with the lockstep-deletion regression test at envfile.rs:539-564.
- **No silent skips anywhere in the read path.** Unreadable, non-UTF-8, and oversized files are violations (walk.rs:165-192; test at 338-370); unreadable directories and unclassifiable entries are violations (walk.rs:103-127; deps.rs:378-407); `skipped != 0` is itself a violation (lib.rs:286-292). Symlinks in core crates are reported, never followed (walk.rs:129-139; test at 373-411).
- **Scope-escape wiring is pinned through the real entry points.** v1 (`[lib] path`), v2 (all rustc spellings of `#[path]`, structural matcher at escapes.rs:162-232), v3 (`OUT_DIR` unresolvable-is-a-violation, escapes.rs:249-258; `build.rs` by file and by manifest key, escapes.rs:385-409) all drive `check_scope_integrity` on synthetic workspaces (lib.rs:606-685).
- **Denylist laundering is closed.** The composition-root carve-out masks rather than skips, with a space replacement that cannot splice terms (denylist.rs:252-260; tests 647-699); the root `[workspace.dependencies]` table is text-scanned at true line numbers (denylist.rs:429-445; lib.rs:344-349; pinned by v4's second assertion, lib.rs:708-714); workspace-inherited renames resolve through the root table in both spellings (denylist.rs:378-384; deps.rs:233-237); manifests are additionally scanned as raw text so `[features]`/`[package.metadata]` cannot hide names (denylist.rs:982-1010).
- **Role wiring is pinned twice.** `every_scanned_file_gets_the_role_its_layer_requires` (lib.rs:888-918) walks the real repo and asserts the role mapping file-by-file (inverting the branches fails it); `the_two_roles_enforce_different_term_sets` (lib.rs:925-938) proves the roles are not interchangeable.
- **Adapter-identifier derivation cannot go vacuous silently.** `deny.adapter.is_empty()` is a violation in both consumers (lib.rs:309-315, 357-363); derivation covers full names, both spellings, reversed §4.1 segment order, and whole-literal distinguishing segments (denylist.rs:176-223), with the segment vocabulary deliberately excluding spec capability words (test at 815-820).
- **Self-exemptions that are narrow, justified, and guarded.** `TARGET_PATH_EXEMPT_CRATES` is pinned to exactly `["crates/gauntlet-arch-tests"]` with the rule proven to apply to all four core crates (escapes.rs:678-690); the arch-tests crate lives outside `CORE_CRATES` by construction (walk.rs:20-25) and is classified with zero allowed internal deps (deps.rs:81-98, 110); the composition-root exclusion from the adapter-identifier rule is compensated by the requirement that every cli file stay inside the other two scopes (lib.rs:402-413).
- **Malformed manifests fail, never pass.** `scan_manifest` (denylist.rs:357-362), `scan_cargo_target_paths` (escapes.rs:343-349), `check_member`/`declared_deps` (deps.rs:209, 269-270), `read_members` (deps.rs:313-330) all report or error on parse failure; `members_of` panics closed on an unparseable root (lib.rs:85-88).
- **Normalization defeats the textual evasions it claims to.** Four-form scan units — line continuation, `concat!`/raw-string/comment-interleaved fragment joins, rustfmt statement joins, `\x`/`\u{}` decode — each pinned by tests (text.rs:367-520), and consumed by both scanners and the escape scanner.
- **`check_env_example` fails closed on missing evidence.** Unreadable spec or unreadable `.env.example` are violations (lib.rs:481-498); empty parses on either side are violations (envfile.rs:227-234, 262-269); credentials are scanned over raw text including comments before any early return (envfile.rs:94-107, 218); the annex parser is bounded (envfile.rs:131-154; test 397-406) and an annex heading rename fails loudly rather than emptying the expectation (every §8.3-named variable would land in `missing_from_annex`).
- **All five locators are currently wired to substantive checks.** `tests/contract/architecture_boundaries.rs` → `check_dependency_direction` + `check_scope_integrity`, with a test function literally named `denylist` in that target (matching the `::denylist` locator); `adapter_registry.rs::selection_is_config_driven`, `no_hardcoded_literals.rs`, `env_example_complete.rs` → their `check_*` entry points. The rot vector is M7, not any current emptiness.

## What I did not file

- The timeout class's `≥ 2` threshold (a 1-unit duration literal passes), the tier class's `const`/`static`/`default`-line restriction, the model-id family enumeration, and the absolute-path class's enumerated comment reach are all recorded residuals in literals.rs's module table (lines 8-61) with stated false-positive postures — documented incompleteness is not vacuity.
- "Delete the whole suite / remove the member from the workspace" is not filed: no test suite can guard its own deletion, and `just ci` runs `boundaries` between `test` and `ruler` (justfile:43-45).
