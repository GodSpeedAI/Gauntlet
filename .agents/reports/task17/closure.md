# Task 17 — Closure: role artifacts + harness installer

**SETTLED — CONFIRM.** Rounds: build (orchestrator-completed after builder window death, COR-O1..6) → design-as-built critic NOT_CONFIRM (14 residuals; RES-D01..D14 adjudicated in `design-adjudication.md`) → B2 landed COR-D01..D14 → blind falsifier round 1 NOT_CONFIRM (2 major / 3 minor → `correction-round1.md`) → B3 landed COR-R1-1..4 (6 teeth) → fresh blind falsifier round 2 **CONFIRM** (0 blocking / 0 major). AK-063 locator divergence recorded, never the ruler.

## 1. What landed (HEAD a116046 + falsifier-round2)

- **Repository role artifacts (10 files)** — `.agents/roles/{orchestrator,builder,critic,verifier,observer,optimizer}.md` plus four per-dimension critic stems `critic-{functional,architecture,security,evidence-completeness}.md`: spec §10.1 front matter (`role`, `context.allow` / `context.deny`, `permissions {filesystem, write_paths, network, tools}`, `result_contract`, `deadline_seconds`); bodies describe the contract and cite the spec, no governance logic in prose (skill-lint class); critic per-dimension bodies for fan-out.
- **Loader** — `crates/gauntlet-app/src/roles/{definition,category,mod}.rs`: validating constructor `RoleDefinition::load` reusing task 5's front-matter + `$VAR` machinery (`config::gauntlet_md`, `EnvLayer`); closed vocabularies `FilesystemMode`/`NetworkMode` plus `tool_vocabulary` and `ContextCategory` (`allow` narrow, never grant beyond builder permits; `OverGrant` refusal); intersection semantics (`allow` minus `deny` shadows → `DenyShadowsAllow`); `DeclaredPath { raw, resolved }` with manual `Debug` raw-only (COR-R1-2, spec §8.1); `ToolError` Displays name token + locus, never resolved VALUE.
- **`gauntlet roles install --harness <id>`** — semantic harness registry (`crates/gauntlet-app/src/roles/harness.rs`): `prime-agent` `WritableRoleDir` vs `generic-cli` `AdvisoryPromptArtifacts`; install writes verbatim bytes into the repo-configured state root (`state-root/roles/<surface>/<harness>/`), never operator dirs, never outside workspace/state roots (`SourceOutsideProject` / `WritePathEscape` with `ensure_within_root`); records `Enforcement` (`Writable` vs `Advisory`) and per-entry `source`/`installed` relative paths + `sha256:` digests in `.gauntlet/roles.lock` (`RolesLock` canonical serialization + `lock_digest` self-digest — COR-O1 closure); `repaired` flag drift-relative via installed-bytes before/after delta (COR-R1-4); fail-closed on unknown/unspecified harness (typed `HarnessUnspecified`/`UnknownHarness`).
- **`gauntlet roles verify`** — content-digest drift detection: out-of-band installed edit/source edit/missing file → typed `Drift` with `DriftKind`; lock tamper → typed `LockTampered` (hand-edited lock, re-forged lock with recomputed self-digest, non-canonical whitespace via canonical re-serialization equality, duplicate-entry lock); derived-shape tamper (COR-R1-1: registered harness check, enforcement vs surface, per-entry derived `source`/`installed` paths — digest-valid but shape-invalid → tamper before digest compare); set-membership drift COR-D04 (installed-extra / source-extra `.md` the lock does not record → drift by name); `contained_join` traversal/absolute escape refusal (COR-D13); protected anchor refusal (§17.5.5: ruler/answer-key/policies/lock); anchored, lexical containment (sandbox job for symlink).
- **CLI surface** — `crates/gauntlet-cli/src/main.rs` `roles install/verify` + `roles_context()` anchoring (`GAUNTLET_PROJECT_ROOT`/`GAUNTLET_STATE_DIR`/`GAUNTLET_ROLES_DIR`/`RESULT_CONTRACT_DIR` via `resolve_against`); `roles verify --harness` unknown-arg refusal (exit 2, COR-D12); `just roles-verify` wired to `cargo run -q -p gauntlet-cli -- roles verify` (COR-D14).
- **Teeth (22 tests in `crates/gauntlet-app/tests/roles.rs`)** — idempotence (byte-identical lock), installed/source/missing drift, hand-edited lock, full lock reforge with recomputed self-digest (installed drift cannot be blessed by forging digest), non-canonical whitespace reforge (COR-O1), over-grant/unknown-key/shadow refusal, indirection missing, `resolved_secrets_never_reach_the_lock_or_bytes_or_debug`, write-path escape, advisory fallback + registry sweep, fail-closed harness, `verify_without_install`, envelope grant/deny-set data, dimension-stem load, COR-D01 empty lock, COR-D04 set-membership (2), COR-D13 traversal/absolute, COR-R1-1 source-redirect + cross-harness relocation (2 variants), COR-R1-3 duplicate-entry, COR-R1-4 repair flag (drift-relative), COR-R1-2 canary pair (mode + write-path) — each with RED before GREEN proven at the owning round, retained in the suite.
- **Arch pins** — `crates/gauntlet-arch-tests/src/single_writer.rs` extended to the roles lock write discipline; `just boundaries` `dependency_direction` + `no_hardcoded_literals` + `denylist` all green on the new surface.

## 2. Closure battery (orchestrator, personally, on the closure tree a116046)

- `just ruler` PASS at start and at end (`sha256:ccc7c99b / 95331d3e` — frozen lock).
- `just check-package gauntlet-app` clean (0.30s).
- `just test-package gauntlet-app` **392/392 PASS** (386 base + 6 COR-R1 teeth, 0.560s); `just boundaries` **228/228 PASS** (21.255s); `just lint` 0 warnings, `fmt --check` clean (clippy `-D warnings`).
- `just deny` advisories/bans/licenses/sources ok (three unmatched-license-allowance warnings pre-existing, not a policy violation).
- `just doctor` exit OK (config 1–5, reference PASS matching frozen lock, sandbox PASS, writable paths PASS, budget PASS; report at `.agents/reports/doctor.json`).
- `just prereg-check 17` PASS (`b3616216df3763407b58c624fb2f46b45f3cbe70771ed6cb7b7ede698bfd571a`, never edited).

**Teeth re-verified personally (RED → byte-identical restore, sha256 → GREEN):** (1) `FilesystemMode` vocab gutted → `unknown_filesystem_mode_is_refused` RED with raw token; (2) `DeclaredPath` derived Debug echoing resolved → canary pair RED with `contains(CANARY)==true`; (3) `duplicate_entry_lock_is_tamper` guard removed → accepted `Ok(roles:3)` RED; (4) `repaired` comparing lock bytes → `reinstall_over_out_of_band_installed_drift_reports_the_repair` RED (`false`); (5) `EmptyRoleSet` removed → `forged_empty_lock_with_recomputed_digest_is_tamper` RED (`Ok(roles:0)`). Pristine digests `c3300e62…` (`definition.rs`) / `18a8c581…` (`install.rs`) / `b827277b…` (`envelope.rs`) matched the builders' recorded values at every step.

## 3. Prereg adjudication (honest; frozen prereg b3616216…)

- **P1 (deny parsed-but-unconsumed / union) — SUPPORTED (sharper form).** RES-D07 fired at critique — deny-set typed but unwired. B2 landed `envelope.rs:269 assert_within_role_deny_set` consume-side; composition wiring stays at task 20 with R-1b hard-trigger (honestly scoped).
- **P2 (idempotence corner) — FALSIFIED.** Second install is byte-identical; canonical lock and self-digest make the property structural.
- **P3 (drift gap) — SUPPORTED (sharper).** Falsifier's FIND-1/FIND-2 majors were derived-shape forgery and secret leakage — the content-digest family exactly as predicted, closed by B3.
- **P4 ($VAR divergence) — SUPPORTED (sharper).** Reuse held, but VALUES leaked into refusal Displays — second-kind divergence, fixed by DeclaredPath raw-only audit.
- **P5 (harness registry representational gap) — SUPPORTED.** Registry as capability map, advisory fallback, fail-closed — each recorded as D3/D4 in `design-adjudication.md`.

### Blindspots

- Real harness role surfaces differ in mutability (agent-mutable per Annex C.4) — design held installed state under the repo-configured root and the advisory fallback, not every native harness surface (recorded, not hidden).
- Role BODY prose cannot be proven to change live sub-agent behavior — claim is installed-and-verified (AK-063), never live enforcement (R-1b explicitly re-stated for task 20).
- Session-mutable harness-state drift flake — `verify`'s subject is pinned to content-digest + set-membership, not mtime/harness liveness.

### Transfer tests

- L1 replay: SUPPORTED — task 5 front-matter + `$VAR` machinery reused, one parse site, `just test-package` green throughout.
- L2 parametric: SUPPORTED — install + verify over both registered harnesses, same path, per-harness enforcement recorded.
- L3 structural: SUPPORTED — novel over-grant / unknown-key / shadow / escape shapes refused at construction without a payload list.

### Explicit nonclaims (all restated open)

R-1b (no real builder executed — first composed dispatch still hard-triggers task 20), R-2 (template vs wrapped command — D3 fog, trigger task 20), R-3 (wrap_builder_command gains no caller), UNK-003 (runners.default — narrowed: doctor measures the harness, the choice stays the operator's), no live dispatch proof (AK-063), no vendor harness proven.

## 4. Residuals opened (triggers recorded)

- **R17-note-1** (minor, durability seam): `write_atomic` fsyncs the staged FILE before rename, but not the parent DIRECTORY — the lock directory entry is not crash-durable. Recorded in `reference-candidates` at closure; trigger: next `roles install` durability touch or task 20 composition (deliberately out of scope here, per COR-D05).
- **R2-N1** (informational, wiring): `assert_within_role_deny_set` is typed and tested in `gauntlet-app` but has no production caller until composition wires the envelope builder to the typed deny-set per role invocation (R-1b). Trigger: first real builder execution (task 20).
- **R17-F4** (CR-D deferred): repository artifacts' `write_paths: ["$RESULT_CONTRACT_DIR"]` is runtime-provided; the absolute-host-path shape is an intentional lexical seam — a resolved symlink check is the sandbox's job, not the installer's (documented at `install.rs:1054`). Trigger: task 20 CLI surface.
- Round-2 not-attacked list (next round's benefit): full `cargo-mutants` sweep (task 24), symlink-escape, TOCTOU, case-insensitive, unicode stems, dotenv injection variants, doctor checks over `roles.lock`, scheduler/authority gating, recovery/resume.

## 5. Deviations (all recorded at the time)

- D-dev-1 (recorded in `design-adjudication.md` D3): `roles verify --harness` unknown-arg refusal — the plan's literal gate `roles verify` carries no flag; the documented gate divergence uses an explicit harness id in the harness-level test `every_registered_harness_installs_and_verifies`, never the CLI literal.
- D-dev-2 (B2 `b2-progress.md` Pre-work finding): `GAUNTLET_ROLES_DIR` out-of-root override correctly refused by `SourceOutsideProject` — the smoke's empty-dir leg re-used an in-project override dir.
- D-dev-3 (`falsifier-round2.md` §Not attacked): full mutant sweep not run on WSL2 per `AGENTS.md` Expensive evidence (settlement-boundary evidence, not per-round).
- Healing: milestone 390ccf3 was orchestrator-completed after the builder's usage-window death (degraded independence recorded in `IN-FLIGHT.md` + `b3-builder.md` §7); both compensation rounds were fresh-context and firewalled; falsifier round 1's 22-file diff was reconciled per tree-outranks-record (zero work lost).

## 6. Answer-key bookkeeping

AK-063 (roles installed + drift detection) now carries test evidence: provisional through Task 17. Its frozen locator (`tests/contract/roles::install_and_verify`) names a repo-root `tests/contract/` target that `gauntlet-arch-tests` (test-only, depends on no internal crate) cannot host without violating the `dependency_direction` boundary suit — the locator-class divergence recorded in `reference-candidates.md` (2026-08-30 entry), never by editing the ruler. R-1b/R-2/R-3 triggers re-recorded for task 20; UNK-003 narrowed (doctor measures the harness).

## 7. Settlement statement

Settlement gates: independent blind confirmation (falsifier round 2, **CONFIRM**, fresh context, charter verbatim, live legs against `std::env::temp_dir()` roots, byte-identical mutations with sha256 proofs) ✓; proven teeth (22 teeth, 6 new COR-R1 teeth + 5 re-verified personally; round-1 + round-2 batteries retained) ✓; prereg recorded with outcome file (`task-17.outcome.yaml`, frozen `b3616216…` never edited) ✓; full battery green on the closure tree (392/392 + 228/228 + ruler/prereg PASS) ✓; ruler PASS before and after ✓. The orchestrator wrote no shipped code this task beyond adjudication and the standing heal+continue completion (recorded, firewalled) — B2, B3, critic, both falsifiers were delegated; the orchestrator's hands-on work was the battery re-run, teeth re-verification, and the five targeted mutants.
