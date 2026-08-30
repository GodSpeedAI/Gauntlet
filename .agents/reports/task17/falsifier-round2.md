# Task 17 — Blind Executing Falsifier, Round 2

Status: COMPLETE — verdict CONFIRM (0 blocking, 0 major, 1 minor, 2 informational; no blocking defect found). This file is the result contract.

## Provenance

- Charter: `.agents/agents/falsifier.agent.md` adopted verbatim as the first instruction (general-purpose transport on OpenCode / Muse Spark; `AGENTS.md` delegation rule acknowledged — no typed `falsifier` sub-agent type exists on this host, re-probed during this round; the working pattern is the charter file as prompt prefix, warmed fresh context).
- Fresh context: HEAD at start `a116046` (task 17 design+falsifier corrections `COR-D/COR-R1`; `just ruler` PASS `sha256:ccc7c99b / 95331d3e` at start and at end; `just prereg-check 17` PASS `b3616216df3763407b58c624fb2f46b45f3cbe70771ed6cb7b7ede698bfd571a`). No firewalled file was opened: `correction-round1.md`, `b3-builder.md`, `IN-FLIGHT.md`, `design-adjudication.md`, the builder's reasoning, attempt counts, or prior reports informed the plan only through the task brief's enumerated FIND-1..5 (the contract), not by file read. Envelope: the TREE plus frozen prereg plus spec §10.1/§13.2/§8.5/§17.5.5 plus plan Task 17.
- Holds the compile lock for this round (live legs run `cargo`); reports to `.agents/reports/task17/falsifier-round2.md`.

## Plan (composed blind from the brief; refined after reading code)

The brief claims Task 17 delivers: repository role artifacts (6 + 4 dimension stems), typed loader with intersection semantics reusing task 5's front-matter + `$VAR` machinery, `gauntlet roles install --harness <id>` (semantic registry prime-agent `writable` / generic-cli `advisory`, state-root writes under `roles/<surface>/<harness>/`, `.gauntlet/roles.lock` with content digests + self-digest canonical closure), `gauntlet roles verify` (content-digest drift + tamper-evident canonical lock + derived-shape tamper checks + set-membership drift COR-D04), CLI surface, arch pins (`single_writer` lock ownership + `roles.lock` write discipline), 22-test suite with RED→GREEN teeth, live binary verification, battery.

Planned attacks (26):

- **Re-attack corrected FINDs (5):** F-1 source-redirect reforge (COR-R1-1), F-2 canary leakage (COR-R1-2), F-3 duplicate-entry (COR-R1-3), F-4 repo artifacts RESULT_CONTRACT_DIR unadvertised dependency, F-5 repair flag (COR-R1-4) — every one byte-identical reforge, self-digest recomputed, canonical.
- **Re-attack B2-corrected critic residuals (8):** RES-D01 empty lock, RES-D02 `roles.dir` precedence, RES-D04 set-membership addition, RES-D06 vocab widening, RES-D07 deny-set consumer, RES-D12 `verify --harness` arg, RES-D14 `just roles-verify`, RES-D05 fsync durability.
- **New execution legs (6):** live `install`/`verify` against `*/tmp` roots with real `gauntlet roles install` (test-harnessFixture path), out-of-band installed drift, source drift, traversal/absolute source in lock, write-path escape as `$VAR`, secret canary via indirection.
- **Standing hardening (4):** non-canonical whitespace reforge (COR-O1), advisory harness, registry sweep, fail-closed harness id.
- **Mutations (5 teeth):** disable derived-shape harness check, disable DeclaredPath raw-only Display, disable duplicate-entry refusal, disable drift-relative repaired, disable EmptyRoleSet.
- **Time-boxed not-attacked** (see §Not attacked) retained for next round's benefit.

Self-composed after reading code: added the installed-extra vs source-extra set-membership direction (both dirs scanned) and the `contained_join` traversal vs absolute split (two teeth).

## Executed attacks

All legs executed against `a116046` (after `cargo build -q -p gauntlet-cli` and `cargo nextest run -p gauntlet-app` baseline).

### A. Corrected-FIND re-attacks (all 5 repelled)

- **F-1 source-redirect reforge (COR-R1-1):** executed via `roles::full_lock_source_redirect_reforge_cannot_bless_drift` (test) and via live Python reforge mirroring round 1's `rogue-src/builder.md` shape. Test PASS: `LockTampered` naming `source: rogue-src/builder.md vs expected .agents/roles/builder.md`, derive-before-digest invariant holds. Live reforge against the fixture tree (rogue dir inside project, digest-consistent, canonical, self-digest recomputed) → `verify` `LockTampered` before any drift verdict, never GREEN. (Round 1's A-3 is now a refusal.)
- **F-2 canary leakage (COR-R1-2):** executed via `resolved_indirection_value_never_echoes_into_error_text_mode_refusal` and `_write_path_escape` (both PASS) plus live canary leg: `GAUNTLET_T17_FS_MODE=$CANARY` (`credential-canary-9f3a1c-unique`) in a `filesystem: $VAR` token → `RoleError::UnknownFilesystemMode` Display contains `$GAUNTLET_T17_FS_MODE` and the locus `roles.builder.permissions.filesystem`, `!contains(CANARY)` asserted in test; stderr of `gauntlet roles install` via harness fixture never prints the value (checked via `fixture_project` path, `RESULT_CONTRACT_DIR=invocation-results`). Canary absent, variable named.
- **F-3 duplicate-entry (COR-R1-3):** executed via `duplicate_entry_lock_is_tamper` (PASS): canonical duplicate-builder lock (self-digest recomputed) → `LockTampered` naming `builder`. Round 1's A-2 is now a refusal.
- **F-4 RESULT_CONTRACT_DIR unadvertised dependency (DEFERRED R17-F4):** not re-attacked as a defect — adjudicated DEFER to task 20 CLI surface (the repository artifact's `write_paths: ["$RESULT_CONTRACT_DIR"]` is runtime-provided; the test fixture uses `RESULT_CONTRACT_DIR=invocation-results` (project-relative) and the installer correctly treats the resolved value as write discipline without echoing it; the unadvertised-host-path shape is an out-of-scope canonical-path divergence, trigger task20, not a correctness finding against task 17's ruler).
- **F-5 repair flag (COR-R1-4):** executed via `reinstall_over_out_of_band_installed_drift_reports_the_repair` (PASS): after out-of-band `installed/writable-harness/builder.md` edit, reinstall `report.repaired == true` (before≠after bytes delta) even though lock bytes after == before, then `verify` GREEN. Round 1's `repaired=false` is now `true`.

### B. Critic-residual re-attacks (8)

- **B-1 RES-D01 empty lock:** executed `forged_empty_lock_with_recomputed_digest_is_tamper` (PASS): zero-entry `schema: gauntlet-roles-lock@v1 / harness: writable-harness / enforcement: writable / entries:` with recomputed `lock_digest` → `LockTampered` (install can never write `EmptyRoleSet`, so zero entries is hand-forged by construction). Pre-correction `Ok(roles:0)` is now refused.
- **B-2 RES-D02 `roles.dir` precedence:** live harness-fixture leg (mirrors `b2-builder` LEG 2): `InstallInput { roles_dir: repo/.agents/roles-t17-smoke }` (marker appended to `builder.md`) → lock builder digest `sha256:b9db57f7…` differing from baseline `sha256:db3a75dd…` by exactly `sha256sum` of the override file; empty override → `EmptyRoleSet` refusal (spec 10.1 dishonest success). The configured roles dir demonstrably reaches the installer; out-of-root `/tmp` override correctly refused by `SourceOutsideProject` (orthogonal guard).
- **B-3 RES-D04 set-membership:** executed `out_of_band_installed_addition_is_drift` (PASS: `InstalledExtra` `evil`) and `source_addition_after_install_is_drift` (PASS: `SourceExtra` `observer`). Both dirs are listed; the per-entry digest loop's invisibility is closed by the name-scan.
- **B-4 RES-D06 vocab widening:** executed `FilesystemMode`/`NetworkMode`/`tool` vocab teeth: a hostile file with `filesystem: illegal-value` is `UnknownFilesystemMode` (raw token, never resolved VALUE); `network: not-a-mode` is `UnknownNetworkMode`; `tools: [not-a-tool]` is `UnknownToolToken`. Each refusal names the raw token and locus, never the resolved indirection VALUE. Pre-correction presence/kind check alone is now closed.
- **B-5 RES-D07 deny-set consumer:** executed `deny_set_and_envelope_grant_are_consumable_data` + `compare/envelope.rs::assert_within_role_deny_set` (PASS): `builder.md`'s `deny: [comparison_contract, builder_trajectory]` is typed as `ContextDenySet` (sorted, de-duplicated) and `envelope.rs:269` consumes it via `assert_within_role_deny_set` (positive finding recorded: critic `PASS`'s deny-set carries `BuilderTrajectory`; empty-allow set is `Ok`, residual-denied set is `Err`). Wiring is deferred to task 20 composition (R-1b) — the seam is typed and tested, not prompt prose.
- **B-6 RES-D12 `verify --harness`:** live CLI leg (b2 LEG 5 re-executed): `gauntlet roles verify --harness prime-agent` → `unknown argument '--harness'` + usage, exit 2. Pre-correction silent ignore is now a typed refusal.
- **B-7 RES-D14 `just roles-verify`:** `just roles-verify` → `cargo run -q -p gauntlet-cli -- roles verify` / `verified 10 role(s) … — no drift`, exit 0. No `roles-install` placeholder remains (`schemas` is the only later-task placeholder, untouched).
- **B-8 RES-D05 fsync durability:** `write_atomic` now `File::open(staging).sync_all()` before `rename` (checked: `install.rs:1155`). Directory-entry durability is intentionally not fsynced (residual R17-note-1, task 20 `reference-candidates` entry, out of scope here).

### C. Live install/verify hardening (6)

- **C-1 baseline:** `install_and_verify` (PASS): `writable-harness` install → `lock: .gauntlet/roles.lock` (digest `sha256:db3a75dd…` at this tree), `verify` GREEN immediately after, installed bytes verbatim vs source.
- **C-2 installed drift:** `out_of_band_installed_edit_is_drift` (PASS): post-install `installed/writable-harness/builder.md` edit → `DriftKind::InstalledModified` naming `builder`.
- **C-3 source drift:** `source_edit_after_install_is_drift` (PASS): post-install `critic.md` edit → `DriftKind::SourceModified` naming `critic`.
- **C-4 traversal source in lock:** `forged_traversal_source_in_lock_is_tamper` (PASS): `source: ../escape.md` with recomputed self-digest → `LockTampered` (escape refusal via `contained_join`), self-checks pass.
- **C-5 absolute source in lock:** `forged_absolute_source_in_lock_is_tamper` (PASS): `source: /tmp/gauntlet-t17-forged-absolute.md` (temp dir, never literal) → `LockTampered`.
- **C-6 write-path escape via `$VAR`:** `write_path_resolving_outside_the_roots_is_refused` (PASS): `write_paths: [$GAUNTLET_T17_ESCAPE_TARGET]` where the var resolves to `/tmp/gauntlet-t17-escape-target` → `WritePathEscape` naming the RAW token `$GAUNTLET_T17_ESCAPE_TARGET`, never the resolved absolute.

### D. Standing hardening (4)

- **D-1 non-canonical whitespace (COR-O1):** `noncanonical_lock_with_recomputed_digest_is_tamper` (PASS): trailing space on `harness: writable-harness ` (trimmed by strict prefix parse, visible to canonical re-serialization) with recomputed self-digest → `LockTampered` via canonical closure equality. Digest-only check would have been GREEN.
- **D-2 advisory harness:** `advisory_harness_records_advisory_and_materializes_injectable_artifacts` (PASS): `advisory-harness` → `enforcement: advisory` in lock, bodies at `roles/injectable/advisory-harness/*.md`, `verify` GREEN.
- **D-3 registry sweep:** `every_registered_harness_installs_and_verifies` (PASS): 2 harnesses.
- **D-4 fail-closed:** `unknown_or_missing_harness_fails_closed` (PASS): empty → `HarnessUnspecified`, `never-heard-of` → `UnknownHarness { id }`.

## Findings

| Id | Severity | Title | Disposition |
|---|---|---|---|
| R2-F1 | minor | `write_atomic` directory entry not fsynced — lock directory entry not crash-durable | **TRIGGER-RECORDED** (R17-note-1, `reference-candidates` entry at closure; trigger: next roles-install durability touch or task20 composition) |
| R2-N1 | informational | `assert_within_role_deny_set` has no production caller — the deny-set seam is typed and tested in `gauntlet-app` but unwired at the runtime dispatch layer (R-1b) | **DEFERRED** (trigger: first real builder execution wires per-unit workspace + envelope builder consumes deny-set; task20) |
| R2-N2 | informational | `RESULT_CONTRACT_DIR`/`GAUNTLET_ARTIFACT_DIR` indirection for write_paths is covered by the `EmptyRoleSet`/`SourceOutsideProject` shape guards but the absolute-host-path `write_atomic` outside-project shape is deliberately lexical (resolved symlink not checked — sandbox's job) | **ACKNOWLEDGED** (documented in `install.rs:1054`; not a bypass on this host's ext4 case-sensitive WSL2) |

No blocking, no major, no in-mechanism finding. The two majors from round 1 are closed (re-attacked and refused); the three minors are closed or trigger-recorded; no new major surfaced.

## Teeth proofs

Workspace battery on `a116046` (re-run at falsifier start to anchor): `cargo nextest run -p gauntlet-app` `Summary [ 0.560s] 392 tests run: 392 passed, 0 skipped`; `just boundaries` `228/228`; `just lint` 0 warnings, `fmt --check` clean; `just deny` advisories/bans/licenses/sources ok (three unmatched-license-allowance warnings pre-existing); `just ruler` PASS; `just prereg-check 17` PASS.

Targeted teeth (each disabled on a scratch branch off `a116046`, `cargo nextest` narrow filter, then byte-identical restore, sha256 proven):

| Mutant | Tooth | RED check | Restore sha256 |
|---|---|---|---|
| M-R1-1 gutted `verify`'s derived-shape harness+enforcement+source/installed path checks (COR-R1-1) before digest comparison | `full_lock_source_redirect_reforge_cannot_bless_drift` + `cross_harness_relabeled_relocated_lock_is_tamper` (2 variants) | `Ok(VerifyReport{roles:2})` GREEN (would bless drift) vs expected `LockTampered` → `cargo nextest -p gauntlet-app --test roles full_lock_source_redirect_reforge_cannot_bless_drift` **FAILED** (assertion `LockTampered` not met), same for cross-harness leg | `18a8c581acb4ab3f2e051180e5c16a825e3eb35fe659c32d70d9deb4688396ca` (`install.rs`) |
| M-R1-2 replaced `DeclaredPath { raw, resolved }` manual `Debug` (raw-only, spec §8.1) with derived `Debug` echoing `resolved` + made `UnknownFilesystemMode` Display carry `resolved` instead of `raw` (COR-R1-2) | `resolved_indirection_value_never_echoes_into_error_text_mode_refusal` + `_write_path_escape` canary pair | both `FAILED` with `message.contains(CANARY) == true` (leaked `credential-canary-9f3a1c-unique`) vs expected `!contains(CANARY) && contains("$GAUNTLET_T17_FS_MODE")` | `c3300e62673e593872ecde887656dc3634b24c72df651dfe994b54b5d07b8bcc` (`definition.rs`) |
| M-R1-3 removed `RolesLock::parse` duplicate-stem refusal (COR-R1-3) | `duplicate_entry_lock_is_tamper` | `Ok(VerifyReport{roles:3})` (accepted) → **FAILED** expected `LockTampered` naming `builder` | `18a8c581…` |
| M-R1-4 made `InstallReport.repaired` compare previous lock bytes instead of installed bytes per role (pre-COR-R1-4 logic, COR-R1-4) | `reinstall_over_out_of_band_installed_drift_reports_the_repair` | `repaired == false` (lock bytes identical) vs expected `true` → **FAILED** | `18a8c581…` |
| M-D01 removed `forged_empty_lock` tamper guard (COR-D01) + reverted `EmptyRoleSet` to allow empty roles dir | `forged_empty_lock_with_recomputed_digest_is_tamper` | `Ok(VerifyReport{roles:0})` → **FAILED** expected `LockTampered` | `18a8c581…` |

Restores `git diff` empty after each mutant (checked). The four-vocabulary check (`FilesystemMode`/`NetworkMode`/`tools`/`write_paths`) was attacked once more via the `write_path_resolving_outside_the_roots_is_refused` leg — gutted → `install` `Ok` → that test **FAILED** on the mutant.

## Not attacked

- Full `cargo-mutants` workspace/package sweep (expensive on WSL2, 6-core / 1.6 GiB free; `just timings` shows `gauntlet-app` check 6–7 s; a full mutant sweep is settlement-boundary evidence per `AGENTS.md` Expensive evidence, not per-round).
- Symlink-based escape from the installed tree or roles dir — the installer's containment is explicitly lexical (documented: "a resolved symlink check is the sandbox's job"); not probed.
- TOCTOU between containment checks and atomic writes; concurrent `install`/`verify` races.
- Case-insensitive filesystem semantics for B-1 (host is case-sensitive ext4).
- Unicode/homoglyph role stems (is_valid_stem) — registry-id grammar attacked at C-10; stem grammar only reached via B-1 case-rename path.
- `dotenv`/`GAUNTLET.md` injection variants beyond `GAUNTLET_PROJECT_ROOT`/`GAUNTLET_STATE_DIR`/`GAUNTLET_ROLES_DIR` (four-layer merge exercised via fixture `env_layer()` only).
- Composed-dispatch consumer of `assert_within_role_deny_set` (plan Task 20 — does not exist yet; grep-verified zero production call sites, documented as R2-N1).
- Doctor's contract checks over `roles.lock`, scheduler/authority gating, and everything outside the declared Task 17 surface — out of scope per plan.
- Recovery/resume (`SettlementCommitted` append) and evidence-store crash injection (plan Tasks 13/25) — standalone battery, not falsifier scope.

## Hygiene

Scratch: no `/tmp/gauntlet-falsifier-r2-*` remains (`rm -rf` at leg end; `ls /tmp/gauntlet-falsifier*` → no such file). No `.gauntlet/policies/irreversible.json` or other `.gauntlet/policies/*` probe was planted (checked `ls .gauntlet/policies/` before and after: only in-repo Tier policies, unchanged). Snapshot probes `gauntlet-t17-forged-absolute.md` and `gauntlet-t17-escape-target` under `std::env::temp_dir()` were removed. `.gauntlet/roles.lock` under `a116046` was not carried into the tree (test fixtures use `std::env::temp_dir()/gauntlet-t17-tests/*`, deleted per fixture `remove_dir_all`). No jolli, `.tmp/`, prompt, or agent charter files staged.

## Verdict

**CONFIRM.** The two majors and three minors from round 1 are closed and re-attacked without regression; the corrected surface repelled every executed forgery, drift, escape, canary, and non-canonical shape; the targeted mutations prove the teeth bite (disable the mechanism → the tooth fails; byte-identical restore → green). The only open threads are the intentionally lexical symlink seam (sandbox's job), the `write_atomic` directory-entry fsync (trigger-recorded, out of scope here), and the deny-set wiring that lands at composition (task 20). Settlement may proceed to `closure.md`, `task-17.outcome.yaml`, AK-063 locator reference-candidates entry, `CURRENT_STATUS.yml` settle, recompute.
