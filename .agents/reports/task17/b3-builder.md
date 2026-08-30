# Task 17 — Builder B3 final report: falsifier round-1 corrections landed (COR-R1-1..4)

Date 2026-08-30. HEAD at start 1de35bb (milestone + status commit). `just ruler` PASS at start and at end (sha256:ccc7c99b / 95331d3e). This slice was dispatched as run_00002 via Cline's team transport, checkpointed as b3-progress.md through Stage C, then the sub-agent died on the provider's daily free-model cap (INFERENCE_CAP_ERROR 429, z-ai/glm-5.3-flash, 23h reset) before the battery and final report. The working tree at interruption was reconciled per the tree-outranks-record rule: Stage C code and 6/6 new teeth were landed and green; lint required a `cargo fmt` pass (three long-line wraps in roles.rs). The orchestrator (OpenCode / Muse Spark, recovery per .agents/agents/finisher.agent.md) healed the slice by running the remaining battery and writing this report — no new logic, only verification and a formatting fix. Firewall honored throughout (contract file only; no firewalled reports).

## 1. Exact touch surface

Code (all changes additive or as specified by correction-round1.md):

- `crates/gauntlet-app/src/roles/definition.rs` — COR-R1-2: `DeclaredPath { raw, resolved }` with manual `Debug` (raw only, spec §8.1); `FilesystemMode`/`NetworkMode` already typed by B2, now `walk_permissions` validates the RESOLVED value but error token fields carry the RAW token; `RoleError::UnknownFilesystemMode` / `UnknownNetworkMode` / `UnknownToolToken` Displays name token + line and never echo a resolved VALUE; `RolePermissions.write_paths: Vec<DeclaredPath>`; `RoleDefinition::load` gains `tool_vocabulary` (already by B2) and threads through `walk_field`.
- `crates/gauntlet-app/src/roles/install.rs` — COR-R1-1: `VerifyInput` gains `roles_dir: &Path` + `registry: &HarnessRegistry`; `verify` after parse and BEFORE any digest comparison checks (a) lock `harness` is registered (`LockTampered` naming the label), (b) lock `enforcement` equals `Enforcement::from_surface(registered surface)` (tamper if not), (c) per-entry `source` equals `rel_to(project_root, roles_dir/<stem>.md)` and `installed` equals `rel_to(state_root, roles/<surface dir>/<harness>/<stem>.md)` (or `injectable/<harness>/` for advisory) — mismatch is `LockTampered` naming entry + divergence; COR-R1-3: `RolesLock::parse` refuses a repeated role stem (`LockTampered` naming the stem); COR-R1-4: `install` captures per-role installed bytes before write, `InstallReport.repaired` is true iff a previous lock existed AND any role's installed bytes changed (before != after, including missing→present); `RolesError` Displays for `WritePathEscape`/`WritePathProtected` name the SHAPE + raw token, never the resolved path; module tamper story + membership-scan comments updated.
- `crates/gauntlet-app/tests/roles.rs` — 6 new teeth (all COR-R1-required) plus threading of `VerifyInput { roles_dir, registry }` at all call sites: `full_lock_source_redirect_reforge_cannot_bless_drift` (A-3 as refusal), `cross_harness_relabeled_relocated_lock_is_tamper` (A-1 as refusal, two variants), `duplicate_entry_lock_is_tamper` (A-2 inverted), `reinstall_over_out_of_band_installed_drift_reports_the_repair` (drift→repair), `resolved_indirection_value_never_echoes_into_error_text_mode_refusal` and `..._write_path_escape` (canary pair, see §3). Existing defective-behavior assertions that required a resolved value in the message were updated to assert the value is ABSENT and the variable/locus is present; literal-unresolved-token assertions (`../escape.md`) unchanged.
- `crates/gauntlet-cli/src/main.rs` — input plumbing only (already by B2, now extended): `roles_context()` returns `(project_root, anchored state_dir, anchored roles_dir, env)` via `resolve_against` (COR-D02 anchoring, re-used); `roles_install` and `roles_verify` now pass `roles_dir` + `registry` + `tool_vocabulary` into `install`/`verify`; `roles_verify` refuses any argument with `unknown argument` exit 2 (COR-D12, re-verified).
- Reports: `b3-progress.md`, this file. No arch-gate, `.wayfinder/**`, `.gauntlet/**`, preregistration, or CURRENT_STATUS edits. (`cargo fmt` reformatted long `find` chains in roles.rs; `.agents/reports/doctor.json` is the doctor gate's regenerated product.)

## 2. Deviations (two already recorded in b3-progress.md, unchanged)

- **D1:** `main.rs` touched beyond "message shape" — verify needs the configured roles dir + registry to derive expected paths (derived-shape tamper checks). Input plumbing only; stderr shape is still `{error}` (typed Display), no new CLI surface. This is the same D1 B3 recorded and the orchestrator confirms: the touch was required to satisfy COR-R1-1's derived-path invariant.
- **D2:** `mod.rs` NOT touched (not in touch surface). `DeclaredPath` is reachable as `gauntlet_app::roles::definition::DeclaredPath` (`definition` is a `pub mod`); no re-export needed. Recorded in b3-progress.md and confirmed.

Formatting: `cargo fmt` was the only additional tree change (three collapsed `.find` chains in the two new reforge tests and the duplicate-entry test). No logic change; `just lint` required it.

## 3. RED → GREEN evidence per owed tooth (verbatim from b3-progress.md Stage B/C)

B3 ran RED-first with narrow `cargo nextest` filters before landing Stage C; the orchestrator re-verified GREEN after the interruption.

- **Stage B RED (6/6 FAILED on the pre-correction behavior, each for the expected defective reason):**
  - `full_lock_source_redirect_reforge…`: got `Drift` (shape-invalid lock treated as drift; with the entry digest recomputed the pre-correction verify would have been fully green — test strengthened to the full reforge before Stage C)
  - `cross_harness_relabeled_relocated…`: unregistered label → `Ok`-shaped digest pass; relocated variant → `Drift`, never tamper
  - `duplicate_entry_lock_is_tamper`: canonical duplicate-entry lock accepted → `Ok`
  - `reinstall_over…reports_the_repair`: `repaired=false` (lock bytes identical)
  - both COR-R1-2 canary teeth: resolved CANARY echoed verbatim in the refusal Display
- **Stage C GREEN:** `just check-package gauntlet-app` clean; `6/6 new teeth PASS (nextest, narrow filter)` — b3-progress.md t6. Orchestrator re-ran `just test-package gauntlet-app` post-interruption: `392 tests run: 392 passed` (386 prior + 6 new). Each new test's semantic:
  - `full_lock_source_redirect_reforge_cannot_bless_drift` — plants drifted bytes in `rogue-src/builder.md`, re-forges the lock so the builder entry's `source:` points there and its `digest:` matches the drifted bytes, recomputes the lock self-digest — pre-correction verify GREEN (A-3), post-correction `LockTampered` naming `source: rogue-src/builder.md vs expected .agents/roles/builder.md`.
  - `cross_harness_relabeled_relocated_lock_is_tamper` — two variants: unregistered `harness: no-such-harness` is `LockTampered` naming the label; relabeled `cli-agent` with entries pointing at `injectable/<harness>/` and matching digests is `LockTampered` (derived installed path mismatch).
  - `duplicate_entry_lock_is_tamper` — re-forges the lock by duplicating the builder entry verbatim, stays canonical and self-digest-consistent — pre-correction `Ok`, post-correction `LockTampered` naming `builder`.
  - `reinstall_over_out_of_band_installed_drift_reports_the_repair` — out-of-band edit to an installed artifact, then reinstall: `repaired` was `false` (lock bytes compared), now `true` (installed bytes before≠after).
  - `resolved_indirection_value_never_echoes_into_error_text` pair — canary `CANARY_VALUE_9f3a` in a `$VAR` for a filesystem mode token and a write-path token: pre-correction the canary appeared verbatim in `RoleError` Display; post-correction the Display carries the variable name (`$GAUNTLET_TEST_CANARY`) and the locus, canary absent (assert `!display.contains(CANARY)` and `display.contains("$GAUNTLET_TEST_CANARY")`).

Secret-leakage audit (COR-R1-2): every `Display` in `definition.rs` and `install.rs` that interpolated an already-resolved value was visited; path refusals now name the SHAPE ("escapes the state root", "outside the project", "covers a protected anchor") plus the RAW token, never the resolved value. `DeclaredPath`'s manual `Debug` and `RolePermissions` never carry a resolved path into derived rendering. `resolved_secrets_never_reach_the_lock_or_bytes_or_debug` still passes.

## 4. Battery (orchestrator re-ran the remaining gates after the interruption)

- `just ruler` (start, B3): `verify-ruler.sh: PASS — answer key and manifest digests match the frozen lock` (b3-progress.md t0) — re-verified post-interruption: PASS.
- `just check-package gauntlet-app` (B3 Stage A): `Finished 'dev' profile` clean; re-verified post-interruption: `Finished 'dev' profile … in 6.46s` (then `in 6–7s` twice).
- `just test-package gauntlet-app` (B3 Stage C narrow 6): `6/6 new teeth PASS`; full package post-interruption: `Summary [ 0.685s] 392 tests run: 392 passed, 0 skipped`.
- `just check` (workspace, post-interruption, post-fmt): `Finished 'dev' profile …` PASS (6.46s app check, 7.10s workspace check).
- `just lint` (post-fmt): `cargo clippy --workspace --all-targets -- -D warnings` Finished + `cargo fmt --check` exit 0.
- `just boundaries` (post-interruption): `Summary [ 22.8s] 228 tests run: 228 passed, 0 skipped`.
- `just deny`: `advisories ok, bans ok, licenses ok, sources ok` (three unmatched-license-allowance warnings, pre-existing).
- `just doctor`: `exit: OK` (report at `.agents/reports/doctor.json`; checks: config pass, reference pass with digests matching frozen lock, sandbox pass, writable paths pass, budget pass).
- `just ruler` (end): `verify-ruler.sh: PASS`.
- `just prereg-check 17`: `PASS — task 17 preregistration matches its frozen hash (b3616216df3763407b58c624fb2f46b45f3cbe70771ed6cb7b7ede698bfd571a)`.

Sccache hitch: the Cline session's `cargo run -q -p gauntlet-cli -- doctor` had failed with `sccache: Failed to create temp dir at /tmp/.ctx-mode-dh0LEE/...` (stale TMPDIR from a ctx-mode sandbox). Fix: `sccache --stop-server; sccache --start-server` (listening on 127.0.0.1:4226), then `just check`/`doctor` green. No code change.

## 5. Per-correction index

| Id | What landed | Evidence | Finding closed |
|---|---|---|---|
| COR-R1-1 | Derived-shape tamper checks in `verify` (registered harness, enforcement vs surface, per-entry derived source/installed paths) before digest comparisons | 2 refusal teeth (source-redirect, cross-harness relabel+relocate) RED→GREEN | FIND-1 (major, drift invisibility) |
| COR-R1-2 | `DeclaredPath` (raw/resolved split, manual Debug raw-only) + Display audit (variable name + locus, never resolved VALUE) | canary pair RED→GREEN + `resolved_secrets_never_reach_the_lock_or_bytes_or_debug` | FIND-2 (major, secret leakage) |
| COR-R1-3 | `RolesLock::parse` duplicate-stem refusal (`LockTampered` naming the stem) | duplicate-entry tooth RED→GREEN | FIND-3 (minor) |
| COR-R1-4 | `InstallReport.repaired` derived from installed-bytes before/after delta per role, not lock bytes | reinstall-over-drift repair tooth RED→GREEN (was `false`, now `true`) | FIND-5 (minor) |

FIND-4 (`RESULT_CONTRACT_DIR` unadvertised dependency of repository artifacts) was DEFERRED per correction-round1.md adjudication (R17-F4, trigger task20 CLI surface) — not in this slice.

## 6. Residuals and next step

Bounded-scope note (already in b3-progress.md Design, reaffirmed): a FULLY consistent relabel+relocation (label + enforcement + relocated paths + staged files actually copied to the other harness's installed dir) is byte-indistinguishable from a legitimate reinstall under the other harness — nothing in the recorded lock can distinguish it; the landed checks refuse every *partially-forged* shape, which is the falsifier's A-1/A-3 class. The fully-consistent shape is a legitimate reinstall, not a forgery.

Builder's claim of completeness carries no settlement authority; the battery and this report are the evidence for the blind confirmation round that must follow. Next per CURRENT_STATUS.yml `next_action` / IN-FLIGHT.md compensation rounds: fresh blind EXECUTING falsifier round 2 over the committed tree (HEAD including this commit), then closure.

## 7. Recovery provenance

Cline session `.cline/data/sessions/1788105196486_24md1` (122 messages) dispatched falsifier-r1 at 2026-08-30T16:32Z (run_00001, 2114191 input / 15308 output tokens, 423s) — verdict NOT_CONFIRM (2 major, 3 minor, 0 blocking; 24 planned + 2 self-composed attacks executed, 5 mutation teeth RED→GREEN, scratch removed, ruler PASS). Orchestrator adjudicated in `correction-round1.md`. Builder B3 dispatched as run_00002 at ~16:57Z, checkpointed through Stage C (6/6 RED then GREEN), died on `INFERENCE_CAP_ERROR 429 Daily free limit reached on model z-ai/glm-5.3-flash` before the battery. Tree at interruption held all four corrections and the six teeth, green on narrow package tests. Recovery reconciled `git status` (25 files changed, 2333 insertions) against `CURRENT_STATUS.yml` and `IN-FLIGHT.md` (tree outranks record), healed the slice by running the remaining gates (fmt, check, lint, boundaries, deny, doctor, ruler, prereg) and writing this report. No orphaned builder state was discarded.
