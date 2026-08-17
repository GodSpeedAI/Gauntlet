# Task 5 recon — workspace state

**Provenance:** extracted 2026-08-16 by a read-only explore agent; persisted by the
orchestrator. file:line references valid at commit `5317347`.

## 1. `crates/gauntlet-app/` — current state

- `Cargo.toml` (18 lines); deps: `gauntlet-domain.workspace = true`,
  `gauntlet-ports.workspace = true` only. Lines 14–17 comment: "`src/config/` is the
  declared defaults module (plan Task 5), the one place environment literals are
  permitted."
- `src/lib.rs` (10 lines) — doc-comment-only stub listing future services
  (`RunController`, `IterationEngine`, `UnitScheduler`, `CorrectionSelector`,
  `SettlementService`, `PromotionService`, `ReferenceFreezer`, `EnvelopeBuilder`,
  `DiagnosticEngine`) and the layer rule. No modules, no tests, no `config/` dir yet.

## 2. `crates/gauntlet-cli/` — current state

- `Cargo.toml` — deps: `gauntlet-domain`, `gauntlet-ports`, `gauntlet-app` (all
  `.workspace = true`); comment names it the composition root; adapter deps land with
  their tasks.
- `src/main.rs` (9 lines) — stub printing `gauntlet <version>` + "command surface
  lands in Task 20". No arg parsing, no commands, no tests.

## 3. `justfile` — all recipes

```
ruler:          ./scripts/verify-ruler.sh
check:          cargo check --workspace --all-targets
test:           cargo nextest run --workspace
lint:           cargo clippy --workspace --all-targets -- -D warnings ; cargo fmt --check
deny:           cargo deny check
boundaries:     cargo nextest run -p gauntlet-arch-tests
ci: check test lint deny boundaries ruler   (+ @echo "ci: all gates green")
doctor:         @echo "doctor: not implemented — lands in plan Task 5 (spec §8.5 preflight checks)" >&2; exit 2
schemas:        @echo "schemas: not implemented …" >&2; exit 2
roles-verify:   @echo "roles-verify: not implemented — lands in Task 17 …" >&2; exit 2
```

Current `doctor` (justfile:45–48) fails loudly with exit 2. Header (lines 1–6): "Do
not remove `ruler` or change what it calls"; ruler runs last in `ci`.

## 4. `crates/gauntlet-arch-tests/` — the gates Task 5 must satisfy

### walk.rs — DEFAULTS_MODULE + DEFAULTS_TABLE_SETTINGS

- Keyed by exact file path (walk.rs:62):
  `pub const DEFAULTS_MODULE: &str = "crates/gauntlet-app/src/config/defaults.rs";`
  Doc (46–61): since round 6 the exemption is NOT file-scoped — the file is inside
  the literal scan; only declarations of `DEFAULTS_TABLE_SETTINGS` settings are
  exempt; since round 6.1 it is DECLARATION-granular (each setting's own declaration,
  head to its terminating `;`, from the one lexing pass, blanked before matching).
  The vendor denylist applies to the whole file regardless.
- Keyed by setting-name list (walk.rs:89–100), pinned by test at 483–497:
```rust
pub const DEFAULTS_TABLE_SETTINGS: &[&str] = &[
    "max_rounds", "max_parallel", "max_depth", "max_wall_seconds",
    "recovery_reserve_pct", "diagnostic_budget", "review_mode",
    "sandbox_backend", "interactive", "git_strategy",
];
```
- `DECLARED_SCAN_EXEMPTIONS` (line 78) is `&[]` — EMPTY, pinned (475–480).
- `CORE_CRATES` (20–25) = domain, ports, app, cli; `INNER_CRATES` (31–35) = domain,
  ports, app (denylist scope); `COMPOSITION_ROOT = "crates/gauntlet-cli"` (44).
- `in_literal_scope` (382–384) == `in_denylist_scope` (369–371) for every file.

### literals.rs — exemption execution

- `scan_literals` (499): `defaults_scope = path == DEFAULTS_MODULE` (506);
  `mask_setting_declarations(&unit.text)` (533) blanks each setting declaration with
  spaces (length/line numbers preserved).
- `is_defaults_setting_declaration` (689–718): after `pub`/`pub(...)`/`async`/
  `unsafe`/`extern` qualifiers, requires `const`/`static` (case-insensitive), then an
  identifier that IS a §8.6 setting (case-insensitive, trimmed of non-`[A-Za-z0-9_]`),
  whole-identifier match (`MAX_ROUNDS_LOG` does not qualify).
- `setting_declaration_spans` (757–780): splits at CODE semicolons only.

### lib.rs — `check_defaults_exemption` (anti-vacuity, 637–703)

If `crates/gauntlet-app/src/config/defaults.rs` exists it must: (a) stay in denylist
scope; (b) literal scope == denylist scope for every walked file; (c) DECLARE each of
the ten settings (`const <SETTING>` per the same declaration-shape test,
line-filtered); (d) be readable. Violations prefixed `vacuous-exemption:`.

**Consequence:** defaults.rs declares the ten settings as `pub const <NAME>` (case
insensitive, whole identifier); values inside those declarations are literal-exempt;
the vendor denylist (`DENIED_TERMS` includes `"docker"`, `"bubblewrap"`) still applies
to the entire file — defaults must not name vendors.

### deps.rs — external-dependency allowlists (fail CLOSED)

- `pub type ExternalAllowance = (&'static str, &'static str);` (crate name, recorded
  reason); reason enforced (112, `allowlist_entries_state_a_reason`).
- `DOMAIN_EXTERNAL_ALLOWLIST = &[]`, `PORTS_EXTERNAL_ALLOWLIST = &[]`,
  `APPLICATION_EXTERNAL_ALLOWLIST = &[]` (EMPTY by design, AK-002),
  `CLI_EXTERNAL_ALLOWLIST = &[]`, `ADAPTER_EXTERNAL_ALLOWLIST = &[]`;
  `BOUNDARY_TESTS_EXTERNAL_ALLOWLIST` = only non-empty:
  `("toml", "manifest parsing — the one external dependency plan Task 2 permits …")`.
- Doctrine (93–106): "THE EMPTY LIST IS THE DESIGN … adding an entry must be a
  recorded, justified act — a comment naming the spec section or plan task that
  permits it, beside the entry — not a diff buried in a refactor."
- Allowlist admits (crate, SOURCE) pairs, default registry only; git sources,
  alternate registries, patch/replace, `.cargo/config*` compiler keys forbidden.
- Root `[workspace.dependencies]` entries must be member paths or on
  `external_allowlist_union()` (383–444).
- To add a crate: (1) allowlist entry with justification; (2) root
  `[workspace.dependencies]` plain version requirement; (3) member `.workspace =
  true`; (4) `cargo deny check` passes (MIT/Apache-2.0/BSD-3-Clause/ISC/Zlib/
  Unicode-3.0 allowed; unknown-registry/git = deny).
- Precedent: `gauntlet-ports/src/json.rs` — hand-rolled JSON reader (zero-dep wire
  contract parsing).

## 5. `.env.example`

92 `GAUNTLET_*` variables, all empty. Header (1–10): precedence "defaults <
GAUNTLET.md < env < flags (spec §8); the defaults table lands with plan Task 5."
`GAUNTLET_RESULT_PATH` exclusion enforced via `RUNTIME_INJECTED`
(arch-tests `envfile.rs:24-37`, 55–59): absent from Annex E and `.env.example`.
Versioning pair: `GAUNTLET_TOOL_VERSIONING` (line 58), `GAUNTLET_VERSIONING_STRATEGY`
(line 98).

## 6. Domain/ports surface relevant to config/doctor

- `PolicyProvider` (ports policy_provider.rs:116–134):
  `policy_for(&self, tier: BlastRadius, workspace: &WorkspaceRef, deadline) -> Result<SandboxPolicy, PolicyProviderError>`;
  `permissions_for(&self, role: &RoleId, unit: &UnitId, deadline) -> Result<PermissionSet, PolicyProviderError>`.
  `PolicyProviderError` (19–30): `NoPolicyForTier { tier }` / `Invalid { detail }` /
  `Unavailable { detail }`.
- `Clock` (clock.rs:36–44): `now() -> EpochSeconds`; `sleep_until(at)`.
- `BudgetMeter` (budget_meter.rs:105–122): `reserve/charge/remaining/would_breach_reserve`.
- `IdGenerator` (id_generator.rs:12–19): `run_id()` / `invocation_id()` (infallible).
- `OperatorSurface` (operator_surface.rs:84–90): `snapshot(deadline)`, `actions()`.
- `WorkspaceProvider` (workspace_provider.rs:96–115): `create(run_id, subject,
  deadline) -> Result<Workspace, _>` / `cleanup` / `hooks`; `Workspace { reference,
  root: PathBuf, key }`.
- `Sandbox` (sandbox.rs): `SandboxBackendRef::new(&str)` opaque ≤64 bytes;
  `SandboxPolicy { reference, tier, backend, network, writable_roots, denied_paths }`;
  `PolicyValidation::is_clean()`; `SandboxedCommand` carries validation.
- Domain `Budget` (gauntlet-domain/src/budget.rs): `MicroUsd` newtype,
  `BudgetDimension`, `BudgetError` (LimitExhausted / RecoveryReserveBreached /
  ReserveOutOfRange{pct} / LimitZero / ScopeExceeded), `Limits`
  (max_rounds/max_parallel/max_depth/max_wall_seconds/max_tokens/max_cost_usd) —
  "a Budget without limits is not constructible; the values come from configuration
  (plan Task 5)" (140–146, header 11).
- `ReviewMode` domain enum (values.rs:268–270): `Single, Fanout, Adaptive,
  HighAssurance` via `wire_enum!`.
- Typed-error house pattern: plain enums, doc'd variants with deciding values,
  hand-written `Display` with spec anchors ("spec 17.3"), empty
  `impl std::error::Error`. Module headers: `//! Name — spec §x.y`.

## 7. GAUNTLET.md / policies / reports today

- No `GAUNTLET.md` anywhere in the repo.
- `.gauntlet/policies/` exists with only `.gitkeep`.
- `.gauntlet/reference.lock` (942 bytes): flat JSON — answer_key_path
  (".wayfinder/answer-key.md"), answer_key_digest ("sha256:ccc7…859"),
  manifest_path, manifest_digest, algo ("sha256"), frozen_at, spec_source,
  spec_annex, unit_count (70), digest_tool ("scripts/reference-digest.sh"), note
  forbidding edits.
- Reports: `.agents/reports/` top-level files + `task2/`, `task3/`, `task4/` dirs.

## 8. Test conventions

- Unit tests: inline `#[cfg(test)] mod tests` at file bottoms.
- Contract suites: `crates/gauntlet-ports/src/contract/` — plain functions over an
  adapter FACTORY returning `Vec<ContractFinding>`
  (`{suite, case, rule (spec anchor), detail}`); `#[cfg(test)]`-gated machinery;
  teeth tests run deliberately-broken stubs.
- Repo-root `tests/contract/*.rs` declared as `[[test]]` targets in
  gauntlet-arch-tests/Cargo.toml (22–36) with `path = "../../tests/contract/…"`,
  thin files calling `arch::check_*`. Other dirs: `tests/antigaming/`, `tests/e2e/`,
  `tests/fixtures/`, `tests/proofs/`.
- Domain: `crates/gauntlet-domain/tests/settle_public_api.rs` integration test;
  `#[cfg(test)] mod test_sources` inside src.
- Plan Task 5 gate: `cargo nextest run -p gauntlet-app config:: doctor:: && just doctor`.

## 9. Re-export style

- gauntlet-domain lib.rs: private `mod x;` + granular `pub use module::{…}`.
- gauntlet-ports lib.rs: `pub mod x;` + `pub use x::{…}` pairs (31–98) — the natural
  pattern for gauntlet-app.
- Arch gates key on the exact path `crates/gauntlet-app/src/config/defaults.rs`.

## 10. Workspace root

Edition 2024, rust-version 1.92, resolver 3, `unsafe_code = "forbid"`;
`[workspace.dependencies]` = member paths + `toml = "0.9"`; `gauntlet-cli` is a member
with no dependents.
