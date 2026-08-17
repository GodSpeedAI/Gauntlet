# Task 5 — Builder Brief (round 1)

**You are the builder for Task 5 of `.agents/plans/gauntlet-impl.md`: Configuration,
defaults, and `gauntlet doctor`.** Repo root: `/home/sprime01/projects/gauntlet`.

**Read before writing any code, in this order:**
1. This brief (all of it — the decisions below are settled; do not re-litigate).
2. `AGENTS.md` (root).
3. `.agents/reports/task5/recon-spec.md` — verbatim spec §8 extraction.
4. `.agents/reports/task5/recon-workspace.md` — current workspace state.
5. Spec sections as needed: `.agents/specs/Core-spec.md` §8 (lines 503–615), §17.3,
   §20.1, Annex E. **The spec outranks this brief; if they truly conflict, stop and
   report the conflict instead of picking a side.**

## Mission

Implement, in `gauntlet-app` + `gauntlet-cli`, with tests that have teeth:

1. A `defaults` module carrying the spec §8.6 settings table.
2. A `GAUNTLET.md` front-matter parser (constrained grammar, below) with `$VAR`
   indirection, unknown-key collection, and never-printed values.
3. Four-layer precedence resolution (`built-in defaults < GAUNTLET.md < .env/process
   env < CLI flags`) producing an `EffectiveConfig` with stable canonical
   serialization and `config_fingerprint`.
4. `doctor`: all eight §8.5 checks, machine report at
   `$GAUNTLET_REPORT_DIR/doctor.json` + human summary, non-zero exit on required
   failure.
5. The repo's own `GAUNTLET.md`, and a real `just doctor` recipe.

**Gate (must be green before you report done):**

```bash
cargo nextest run -p gauntlet-app config:: doctor:: && just doctor && \
just check && just lint && just deny && just boundaries && just ruler
```

**Teeth you must demonstrate and record verbatim output for:**
- Place `.gauntlet/policies/irreversible.json` (any bytes) → `just doctor` exits
  non-zero with the AK-049 finding; remove it → green again.
- A canary secret (env var referenced via `$VAR` in a fixture GAUNTLET.md) never
  appears in `doctor.json` bytes or human-summary output (asserted by test, and
  demonstrated once by grep).
- Precedence: a value set at each of the four layers wins over the layer below
  (tested at every adjacent pair).

## Hard constraints

- **Never touch** `.wayfinder/**`, `.agents/specs/**`, `.gauntlet/reference.lock`,
  prior task reports, or `.jolli/**`. Do not edit the frozen spec or answer key.
- **Do not commit.** The orchestrator verifies and commits. Leave work in the tree.
- **Build lock:** you are the only compiling agent. Do not spawn parallel compilers.
  Watch `du -sh target`; do NOT `cargo clean` (memory is tight; incremental state is
  precious).
- **No new dependencies except `sha2`** (decision D2 below). If you believe you need
  another crate, stop and report instead of adding it.
- **Never weaken an arch test, a pinned inventory, or an assertion to reach green.**
  If the boundary suite flags your code, fix your code or report the misalignment as
  a finding — do not touch the suite. (Exception: the deliberate, justified
  `APPLICATION_EXTERNAL_ALLOWLIST` edit for `sha2` per D2 — that is the documented
  procedure, with the reason comment beside the entry.)
- House style: `//!` module headers citing spec lines, granular re-exports (ports
  pattern: `pub mod x;` + `pub use x::{…}`), typed error enums with deciding values
  and hand-written `Display` with spec anchors, empty `impl std::error::Error`, no
  `unwrap`/`expect` outside tests, `unsafe` forbidden by workspace anyway.
- Checkpoint discipline (final messages get lost): after completing EACH numbered
  work item below, append one line to
  `.agents/reports/task5/builder-checkpoints.md` (create it): timestamp, item, state.
  Your final report goes to `.agents/reports/task5/builder-round1.md`.

## Settled decisions (do not re-litigate)

**D1 — std-only front-matter parser.** No serde/serde_yaml. The spec requires front
matter that "MUST decode to a map" (§8.2), not full YAML. Implement a strict,
documented subset grammar (below), modeled on the `gauntlet-ports/src/json.rs`
zero-dep precedent. Strictness is the feature: anything outside the grammar is a
typed parse error surfaced by doctor check 1. If parsing later grows per-key
branches, the plan's redesign trigger (schema-driven deserialization in
`docs/schemas/`) fires — note it, don't pre-build it.

**D2 — one new dependency: `sha2`.** Needed for `config_fingerprint` now and reused
by Task 7's reference digest. Procedure (documented in deps.rs doctrine): (1) add
`("sha2", "config_fingerprint digest (plan Task 5); reused by reference digest
(Task 7) — std has no SHA-256")` to `APPLICATION_EXTERNAL_ALLOWLIST` in
`crates/gauntlet-arch-tests/src/deps.rs`; (2) `sha2 = "0.10"` in root
`[workspace.dependencies]`; (3) `sha2.workspace = true` in gauntlet-app. `just deny`
and `just boundaries` must pass after. Nothing else.

**D3 — naming: setting key `git_strategy`, env var `GAUNTLET_VERSIONING_STRATEGY`.**
The §8.6 table (frozen) and the arch suite's pinned `DEFAULTS_TABLE_SETTINGS` both
say `git_strategy`; Annex E and `.env.example` say `GAUNTLET_VERSIONING_STRATEGY`.
The setting↔env-var mapping is data in the env layer. Do not "fix" either side.

**D4 — canonical serialization never materializes `$VAR` values.** `EffectiveConfig`
keeps raw tokens (with `$VAR`/`${VAR}` refs intact) for serialization; resolution
products live in a separate, non-serializing struct used by doctor/runtime checks.
`config_fingerprint` = SHA-256 over the canonical serialization of the RAW-TOKEN
form. Justification: §8.1 "Generated files never contain credentials"; §22 item 4.
`doctor.json` is a generated file → same rule.

**D5 — doctor check statuses are four-valued:** `pass`, `fail`, `unavailable`
(machinery lands in a later task — honest per I8, never silently pass), `skipped`
(precondition false, e.g. check 5's policy-existence clause when `backend == none`).
All eight checks always appear in the report with evidence strings. Exit non-zero iff
any REQUIRED check is `fail`. `unavailable` never blocks, never counts as pass in
prose. Sub-check granularity allowed within a check, each with its own status.

**D6 — doctor scope honesty for this task** (what is real now vs later):
- Check 1 (config parses/unknown keys/$VAR/secrets): FULLY real.
- Check 2 (reference manifest + answer key): manifest exists + parses as a mapping;
  answer key exists, non-empty, counts exactly `unit_count` (70) `^## AK-` blocks
  (same criterion as the Task-1 ruler gate). Per-Unit §5.3.2 validation (incl.
  tier/authority coupling) is Task 7's parser → `unavailable { reason: unit-schema
  validation lands with the ReferenceProvider parser (plan Task 7) }`. Digest-vs-lock
  verification is Task 7's `verify_unchanged` → `unavailable` likewise. Do NOT shell
  out to `scripts/reference-digest.sh`.
- Check 3 (runner resolves per dispatchable role): validate runner CONFIG structure
  (default well-formed if present; `by_role` entries well-formed; names non-empty
  strings). Runtime resolution against a runner registry → `unavailable { reason:
  runner adapters land with plan Tasks 15–16 }`. If NO runners are configured at all,
  the structural part is `skipped { reason: no runners configured (UNK-003) }`.
- Check 4 (capabilities satisfy tiers): `unavailable { reason: capability-vs-tier
  dispatch check lands with plan Task 14; Isolation ladder exists in ports }` —
  UNLESS config structurally contradicts (e.g. `sandbox.backend != none` while some
  role's runner is the literal `none`)? No — leave that to Task 14; keep check 4
  uniformly `unavailable` this task with the reason.
- Check 5 (policies): FULLY real. `backend != none` ⇒ policy files for `contained`,
  `local`, `cross_cutting` must exist under policy_dir; policy file for `irreversible`
  must NOT exist — its presence is `fail` (AK-049, §17.3) even when backend == none.
- Check 6 (`GAUNTLET_MCP_REQUIRED` resolves): if unset/empty → `pass` with note "no
  required capabilities configured". If set → validate each name against the §8.4
  capability vocabulary (code_nav, filesystem, versioning, test, build, browser,
  visual, web, research, security, sandbox, memory, evidence, domain, policy,
  benchmark, schema): unknown capability name → `fail`. Tool-binding resolution
  (does a tool exist for it) → `unavailable { reason: tool registry lands with plan
  Tasks 15–16 }`.
- Check 7 (dirs writable; answer key outside workspaces): FULLY real. Probe
  writability by creating+removing a temp file in state dir, evidence dir, workspace
  root (create dirs if absent). Answer-key path (resolved) must not be inside the
  resolved workspace root (lexical + canonicalized check; canonicalize failures are
  findings, not crashes).
- Check 8 (budget parses, non-zero, reserve < 100): FULLY real. Build domain
  `Limits` from effective config; domain `BudgetError` variants become check
  findings. This proves domain reuse (budget.rs says values come from this task).

**D7 — repo `GAUNTLET.md` omits `$VAR` references and the runners section.**
Doctor check 1 fails on unresolved `$VAR` indirections, and this repo sets none of
them, so the repo's own GAUNTLET.md must only carry literal-resolvable values:
reference paths (relative, as in the spec example), roles.dir, observation.facets
`[versioning, filesystem, build, test]`, review `{mode: adaptive, max_critics: 4}`,
verification `{prefer_deterministic: true}`, budget (§8.6 defaults: 8/2/1/3600/
2M tokens/25 USD/15%/2), workspace `{use_worktrees: true}` (root omitted → default),
state `{backend: sqlite}` (dir omitted → default), evidence (dir omitted → default),
operator `{surfaces: [cli]}`. Omit `runners` and `sandbox` entirely (UNK-003: the
default runner is an open unknown; sandbox backend defaults to `none`). Add an H2
"## Runners" documentation section (below the front matter) noting UNK-003 and that
runner selection arrives via `$GAUNTLET_AGENT_RUNNER` in later tasks. H2 sections are
documentation per §8.2 — the parser accepts and ignores them (but the doctor
unknown-key report covers front matter only).

**D8 — defaults module shape.** `crates/gauntlet-app/src/config/defaults.rs` MUST
declare the ten §8.6 settings as `pub const <NAME>` where `<NAME>` case-insensitively
equals the setting name, whole identifier (arch rule `is_defaults_setting_declaration`):
`MAX_ROUNDS=8`, `MAX_PARALLEL=2`, `MAX_DEPTH=1`, `MAX_WALL_SECONDS=3600`,
`RECOVERY_RESERVE_PCT=15`, `DIAGNOSTIC_BUDGET=2`, `REVIEW_MODE="adaptive"`,
`SANDBOX_BACKEND="none"`, `INTERACTIVE` (type `InteractiveDefault::AutoDetect` —
auto-detect TTY is a behavior, not a literal), `GIT_STRATEGY="worktree"`. A pinned
test asserts value equality with §8.6. Other conservative defaults (policy dir
`.gauntlet/policies`, report dir `.agents/reports`, state dir, evidence dir, workspace
root default, `GAUNTLET.md` discovery fallback `./GAUNTLET.md`) also live in
defaults.rs as consts — they are literal-scanned (relative paths are not a denied
literal class; no absolute paths, no URLs, no ports, no model ids) — keep them
relative-path strings. If the boundary suite flags any of these, that is a finding to
report, not a hack. Vendor denylist applies to the whole file: name no vendors.

**D9 — env layering.** Layer 3 = `.env` file at project root (simple `KEY=VALUE`
lines, `#` comments, no interpolation, quoted values stripped) UNDER process
environment (process env wins over `.env`). Unknown `GAUNTLET_*` env var names
(names not in Annex E) are collected and reported like unknown front-matter keys —
never silent, never fatal. Test-hook vars (`GAUNTLET_CLOCK`, `GAUNTLET_ID_SEED`) are
recognized, not unknown.

**D10 — CLI surface this task.** `gauntlet doctor [--config <path>]` only. Discovery
per §8.2: `--config` → `$GAUNTLET_CONFIG_DIR/GAUNTLET.md` → `./GAUNTLET.md`.
Exit 0 on healthy, non-zero on required failure; human summary to stdout; report JSON
to `$GAUNTLET_REPORT_DIR/doctor.json` (default `.agents/reports`). No other
subcommands; unknown args → usage error, exit 2. No clap — std `env::args` parsing,
small and boring. The justfile `doctor` recipe becomes
`cargo run -q -p gauntlet-cli -- doctor` (propagate exit). Keep the recipe's comment
style; do not touch `ruler`.

**D11 — module layout (gate filters depend on it).** In gauntlet-app:
`pub mod config;` → `src/config/{mod.rs, defaults.rs, gauntlet_md.rs, env_layer.rs,
resolve.rs, canonical.rs, fingerprint.rs (sha2 wrapper), error.rs}` and
`pub mod doctor;` → `src/doctor/{mod.rs, checks.rs, report.rs}`. Inline
`#[cfg(test)]` tests so nextest sees `config::…` and `doctor::…` names. Granular
re-exports: `EffectiveConfig`, `ConfigError`, `DoctorReport`, `run_doctor`-style
entry (exact names yours; keep them semantic). The CLI stays thin: parse args, call
app, print, exit.

**D12 — front-matter grammar (document it in `gauntlet_md.rs` header).** Accepted:
optional leading `---` fence + optional closing `---` (content between); nested
mappings via 2-space indentation; `key: value`; keys bare identifiers
(`[A-Za-z_][A-Za-z0-9_]*`); scalar values: bare/quoted strings, integers, booleans
(`true`/`false`); inline flow sequences `[a, b, c]`; inline flow mappings
`{k: v, k2: v2}` (needed for `by_role`); `#` comments to end of line (not inside
quoted strings); blank lines. `$VAR` and `${VAR}` tokens allowed inside string
values. Rejected (typed errors with line:col): tabs for indentation, block scalars
(`|`, `>`), anchors/aliases, tags, multi-document, duplicate keys, non-map top level,
unclosed quotes/brackets, empty key, mixed indentation widths on sibling lines.
Empty/no front matter → empty map (valid). Values are kept as raw strings + a tiny
typed value enum; DO NOT convert to final types in the parser — conversion happens
in resolve.rs against the known-key table.

**D13 — known-key table.** One table in resolve.rs mapping every documented §8.2 key
path (e.g. `reference.answer_key`, `budget.max_rounds`, `runners.by_role.<role>.model`)
to: expected value kind + target field on a `RawConfig`/`EffectiveConfig` builder +
the env-var override(s) that address it (Annex E names, e.g. setting `git_strategy` ←
`GAUNTLET_VERSIONING_STRATEGY`). Any front-matter key not in the table → collected
`unknown_key` (path + line). Any env var not mapping into the table → collected
`unknown_env`. This table is the single place key knowledge lives (plan's
schema-driven redesign trigger watches it).

**D14 — canonical serialization.** Deterministic JSON object: keys sorted
(byte-wise, BTreeMap-style), two-space indent, LF endings, no timestamps, `$VAR`
tokens raw (D4). Round-trip stable: parsing the canonical form of an EffectiveConfig
yields an identical EffectiveConfig. `config_fingerprint()` returns a domain `Digest`
(SHA-256, `sha256:`-prefixed hex — reuse the domain `Digest` type; check its
constructor). Serialize via a small hand-rolled writer in canonical.rs (reuse it for
doctor.json emission — same writer, sorted keys; secrets rule keeps values raw).

**D15 — AK-018 contract test (opportunistic locator resolution).** Try adding
`tests/contract/secrets.rs` as a `[[test]]` target of gauntlet-app
(`path = "../../tests/contract/secrets.rs"`) with test `never_leaked` (canary
assertion, D4/teeth). If `just boundaries` objects to a non-arch crate declaring a
repo-level test path, drop the target, keep the assertion as an inline
`config::…::secrets_never_leaked` test, and record the deferral in your report.
Similarly, if straightforward, an integration test matching AK-015's locator
`tests/contract/config::precedence` may be added the same way (same fallback). Do
not fight the arch suite for either.

**D16 — re-check API.** Doctor's checks are exposed as callable functions
(`pub fn check_*` or a `Doctor` struct) so the Task-12 controller can re-run the
drift subset at iteration boundaries (§8.5). Keep the API surface small and typed.

## Work items (checkpoint after each)

1. `defaults.rs` with D8 contents + pinned value tests (module skeleton + lib.rs
   wiring first so `just boundaries` sees the real path immediately).
2. `gauntlet_md.rs` parser per D12 + tests (spec §8.2 example as a fixture,
   verbatim, incl. inline flow maps; error cases each producing the right typed
   error; unknown keys collected).
3. `env_layer.rs` per D9 + tests (process-env-over-.env, unknown env collection,
   test hooks recognized).
4. `resolve.rs` + `error.rs` per D13: precedence merge (all four layers; adjacent-
   pair override tests), `$VAR` presence validation (missing → typed error listing
   var name, never value), `EffectiveConfig` construction; budget → domain `Limits`
   (D6 check-8 prep).
5. `canonical.rs` + `fingerprint.rs` per D14 (+ `sha2` per D2, allowlist edit) +
   tests (determinism under shuffled construction, round-trip, canary-absence).
6. `doctor/` per D5/D6/D16 + tests: every check's pass/fail/unavailable/skipped
   paths exercised; report JSON emission (sorted keys, statuses, evidence strings,
   `generated_at` allowed in report but OUTSIDE any digested content — the report is
   not digested this task, keep the field anyway for humans); exit-code logic.
7. CLI `doctor` command per D10 + justfile recipe replacement.
8. Repo `GAUNTLET.md` per D7 + `.agents/reports/task5/` fixtures as needed (fixtures
   for tests live under `tests/fixtures/config/` — check arch walker treatment of
   that dir; keep them data-only).
9. Optional D15 locator tests.
10. Full gate run + teeth demonstrations (record verbatim output into your report).
11. `cargo fmt`, final `just lint`, write `.agents/reports/task5/builder-round1.md`.

## Report format (builder-round1.md)

- File map (every file touched/created, one line each).
- Gate output verbatim (tail) + teeth transcripts (the irreversible-policy fail, the
  canary grep, precedence summary).
- Deviations from this brief, with reasons.
- Known gaps / deferrals / honest unknowns (what is `unavailable` and why).
- Anything the arch suite flagged and how you resolved it (or why you couldn't).

Remember: your claim of completion carries no settlement authority — the orchestrator
verifies independently and blind critics attack the artifact. Report honestly; a
recorded gap costs nothing, a hidden one costs the run.
