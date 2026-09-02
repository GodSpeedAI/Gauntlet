# Task 20 / Builder B2 final report — CLI composition root + §20.1 surface + run + dry-run

All work is in the working tree, uncommitted (builders do not commit). Evidence is
test/gate output under `.agents/reports/task20/b2-*.log`; builder claims carry no
settlement authority.

## Gate battery (final tree, required order)

| gate | result | evidence |
|---|---|---|
| `just check` | PASS (0) | b2-gate-check.log |
| `just test` | PASS — **1319 tests run: 1319 passed, 0 skipped** | b2-gate-test3.log |
| `just lint` | PASS (0) — clippy -D warnings + fmt | b2-lint-final7.log |
| `just deny` | PASS (0) | b2-gate-deny.log |
| `just boundaries` | PASS (0) — 228/228 | b2-gate-boundaries12.log |
| `just ruler` | PASS (0) | b2-gate-fast.log |
| `just doctor` | PASS — exit OK | b2-gate-doctor.log |
| **dry-run gate**: `cargo run -q -p gauntlet-cli -- run --dry-run "seeded bounded task"` | **exit 0, ZERO writes** (`git status --porcelain` diff before/after empty) | b2-dryrun-final.log |
| `cli_surface` snapshot | PASS (inside gauntlet-cli's 67/67) | b2-cli-test3.log |

Workspace test count grew 1303 → 1319 (+16: cli_surface 4, compare_deny 2,
id-entropy 2, escalation-fs 7, report_manifest 1).

## What landed

### Item 0 — baseline hygiene
- clippy `redundant_closure` fixed at `crates/gauntlet-app/tests/report.rs:213`.
- `cargo fmt` applied to files fmt flagged (workspace-fs provider, report module, two
  CLI proof files) — rustfmt-only, no semantic change; recorded as the one touch of
  B1's crate.

### Item 1 — thin-library refactor
- `src/lib.rs` (`run_cli` + whole-tree `usage()` + `--help`/`-h` root handling),
  `src/context.rs` (RuntimeContext: root discovery + dotenv + four-layer merge +
  shared `measure_backend`/`measure_runner`/`doctor_bound`), `src/doctor_cmd.rs`,
  `src/reference_cmd.rs`, `src/roles_cmd.rs`; `main` is a wrapper (now
  `src/bin/gauntlet.rs`, auto-discovered; binary named `gauntlet` per §20.1).
- Behavior of doctor/reference digest+validate/roles is code-moved, byte-identical;
  proven by the untouched proof suites (export_stack, roles/formation/skills app
  tests, all p*/g* CLI proofs) passing on the final tree.

### Item 2 — `gauntlet run` + `--dry-run` (the milestone)
- `src/compose.rs`: ONE `ControllerDeps` literal over real adapters —
  EmbeddedStateStore (`<state_dir>/gauntlet-state.db`), FsEvidenceStore
  (configured evidence dir), FsWorkspaceProvider (`<state_dir>/workspaces`, R-1b),
  FsEscalationQueue (`<state_dir>/escalations`), UlidIdGenerator (or the Annex E
  `GAUNTLET_ID_SEED` hook), SystemClock, per-token runner construction (prime /
  cli / mock), observation facets → real observers (test/build via
  `GAUNTLET_TOOL_TEST`/`GAUNTLET_TOOL_BUILD`, filesystem, versioning), comparison
  (frozen contract + DiffComparatorSource + ModelComparatorSource over the critic
  runner, review mode parsed typed, model_dispatches = review.max_critics),
  three-rung deterministic ladder (FileDigest/CommandExitCode/TestReport; schema
  rung needs a required-keys contract no §8.6 setting carries — recorded, not
  silently skipped), GovernDeps none-or-live-backend from `sandbox.backend`.
- **D2 runner selection fail-closed**: unset/unknown `runners.default`
  (`GAUNTLET_AGENT_RUNNER`) ⇒ typed configuration failure naming key + env + the
  three fronted tokens; repo GAUNTLET.md untouched.
- **Mock seam**: new `RUNNER_TOKEN = "mock-agent"` in the mock crate +
  `GAUNTLET_MOCK_SCENARIO` names the scenario fixture stem; unset ⇒ typed
  configuration failure. Deliberately NOT in `.env.example` (AK-019 Annex-E
  lockstep; test-division hook, documented in help/report).
- **D3 wrap caller (R-3 discharge)**: for the `cli-agent` token the composition
  calls `AuthorityGate::admit` + `wrap_builder_command` on the FIRST planned
  dispatch's real facts BEFORE any run; the `none` posture yields the
  `BuilderCommand::Unwrapped` leg and the run proceeds; a live-backend
  `Sandboxed` verdict REFUSES the run (`CompositionError::LiveContainmentDeferred`)
  — the fully-composed live-containment leg is Task 25's, never a silent unwrap.
- **R17-F4**: when a live backend is composed, the policy provider is wrapped
  (`ContractDirPolicy`) so every tier policy carries the runtime-provided
  result-contract directory (`<state_dir>/runs`) as a writable root (idempotent);
  permissions_for delegates verbatim.
- **R2-N1 deny wiring (app)**: `ComparisonAuthority::with_role_deny` +
  `GovernedRun::issue` enforce `assert_within_role_deny_set` BEFORE
  `build_envelope`; `ComparisonDeps.role_deny` added; the composition loads the
  critic role's deny set (`compose::critic_deny_set`). Teeth:
  `crates/gauntlet-app/tests/compare_deny.rs` — the wired deny set refuses a
  denied always-carried category through `for_run→issue` before the envelope is
  built; the empty deny set reaches envelope construction (2/2 PASS).
- **Escalation adapter (NEW crate)** `gauntlet-adapter-escalation-fs`:
  FsEscalationQueue — escalate/poll via the port (passes the SHARED
  `human_escalation` contract suite), plus list/record/resolve for the operator
  read surface; one JSON file per ticket, create-temp→sync→rename→dir-fsync;
  typed errors; resolve-once. 7/7 tests.
- **Id adapter (NEW crate, deviation)** `gauntlet-adapter-id-entropy`:
  UlidIdGenerator — the ONLY prior IdGenerator impl was the mock's seeded double
  (replays ids per seed; two runs under one seed would fold into one run
  identity). OS entropy at construction (typed failure), monotonic counter;
  grammar-valid by the seeded sibling's encoding. 2/2 tests.
- **D1 dry-run**: `run --dry-run` = doctor (read-only `run_doctor`) must be clean →
  strict answer-key parse → reference lock verified against disk
  (`reference_lock.rs`, the typed `just ruler` twin — digest agreement WITHOUT
  freezing) → scheduler plan preview (`key=value` lines, D7 cohort map empty) →
  exit 0. ZERO writes verified by git-status diff.

### Item 3 — the §20.1 surface (D8/D9)
Command table (command → backing service → wired?):

| command | backing service | wired |
|---|---|---|
| doctor | gauntlet_app::doctor + real backend/runner measures | yes |
| init | scaffold (refuse-overwrite; commented template; minimal env example; .gauntlet) | yes |
| reference digest/validate | reference normalize / shared validator | yes (pre-existing) |
| reference form | formation guidance + shared validator for `--path` | yes (never fabricates) |
| reference freeze | FileReferenceProvider resolve→normalize→freeze→verify_unchanged into the evidence store, prints digest | yes |
| reference show | parse + digest + units by tier/authority | yes |
| reference diff | application digest over both paths | yes |
| reference verify | `reference_lock::verify_lock` (typed `just ruler` twin) | yes |
| run / resume | RunController via compose.rs (above) | yes |
| status | load_run + checkpoint + answer-key tier join (`--run` REQUIRED — the port has no run enumeration; stated truthfully) | yes |
| inspect | observation_binding / verification_records / settlements reads with `--iteration/--unit/--verifications/--settlements` | yes |
| verify | VerificationAuthority + composed ladder, three-state outcomes, NEVER settles | yes |
| escalations list | escalation-fs adapter records | yes |
| escalations resolve/retry | `SettlementService::settle_operator_action` with OperatorIdentity (D9 single write path); the queue resolves ONLY on an accepted settle; refusals surface typed and the ticket stays open | yes (honest: the service's earned-readiness refusal is today's structural outcome — surfaced, not worked around) |
| roles install/verify | roles install/verify (pre-existing) | yes |
| skills install/verify/list | REPO_OWNED_SKILLS + task19 lint (reused) + `.gauntlet/skills.lock` content digests, drift-repairing install | yes |
| improve | typed NOT-WIRED failure naming plan Task 23 | no (honest) |
| export | `export_run` to stdout or `--out` | yes |

Convention held: unknown args/missing args ⇒ exit 2 with usage; runtime refusals ⇒
typed message, non-zero; no secrets echoed; `key=value` phrasing (§20.3).

### Item 4 — report + cli_surface
- After run/resume the CLI prints `gauntlet_app::report`'s human summary + canonical
  JSON, populated from the run's records with the CLI's REAL command table as the
  capability manifest (`src/report_manifest.rs`; improve declared not-wired —
  manifest and exit agree, pinned by tests).
- `crates/gauntlet-cli/tests/cli_surface.rs`: whole-tree `--help` snapshot vs the
  committed golden (`tests/cli_help.txt`; deliberate-regeneration procedure in the
  test doc), whole-tree dispatch + unknown-command smokes, 22 unknown/missing-arg
  exit-2 smokes, and the usage/dispatch/manifest agreement pin.

## Library API surface (for the design critic; B3 composes the BINARY)
- `gauntlet_cli::run_cli(&[String]) -> ExitCode` — the whole dispatch.
- `context::{RuntimeContext, resolve_context, measure_backend, measure_runner,
  doctor_bound}`.
- `compose::{select_runner_token, compose_run, open_operator_surface,
  composed_ladder, critic_deny_set, diagnostic_allowance, CompositionError,
  ComposedRun, OperatorSurface, MOCK_SCENARIO_VAR}`.
- `reference_lock::{verify_lock, LockReport, LockError}`.
- Per-command modules: `doctor_cmd::run`, `init_cmd::run`, `reference_cmd::run`,
  `roles_cmd::run`, `run_cmd::{run, resume}`, `state_cmd::{status, inspect, export}`,
  `verify_cmd::verify`, `escalations_cmd::escalations`, `skills_cmd::run`,
  `improve_cmd::run`. `report_manifest::{cli_capability_manifest, is_wired,
  COMMAND_TABLE_LEN}`.

## Decisions where the brief left a choice
1. **Mock scenario mechanism**: `GAUNTLET_MOCK_SCENARIO` env hook naming a fixture
   stem (load_scenario); fail-closed; kept out of `.env.example`/Annex E (AK-019).
2. **cli_surface placement**: D5 resolved to the brief's fallback — host in
   `crates/gauntlet-cli/tests/`. The exemption is arch-tests-only, justified by
   non-product status the CLI cannot share; also `CARGO_BIN_EXE_gauntlet` only
   exists in the owning package. Recorded in
   `.agents/reports/reference-candidates.md` (AK-063/AK-065 class, dated entry).
3. **Escalation contract suite**: the SHARED `gauntlet-ports::contract::
   human_escalation` suite exists and the adapter passes it unmodified; read/
   resolve surface covered by 6 crate tests.
4. **Diagnostic allowance mapping**: config scalar `diagnostic_budget` (unitless,
   default 2) = number of §15.2 diagnostic cycles; each cycle spends one
   per-attempt share of the run's FORWARD ceilings — no invented constant.
5. **Workspace root**: `<state_dir>/workspaces` per the brief (overrides the
   config's separate workspace.root in this slice; recorded).
6. **Model dispatch**: `model_dispatches = review.max_critics`; the critic runner
   is `runners.by_role.critic` falling back to `runners.default`.
7. **Second new adapter crate** (`gauntlet-adapter-id-entropy`): justified above —
   the composition's `ControllerDeps.ids` had no production-safe impl.

## Deviations / gate growth (all deliberate, all recorded)
1. **Composition-feature seal widened** (arch `deps.rs`): `gauntlet-app/
   composition` is now additionally admitted on `gauntlet-cli`'s unscoped
   `[dependencies]` edge — the production composition constructs the §14.2 rung
   set. The app crate's own manifest comment anticipated "the production wiring of
   Tasks 12/20"; the seal test was updated (renamed
   `the_composition_seal_accepts_only_the_recorded_cli_edges`) with fail-first
   shapes re-pointed at foreign-crate production edges.
2. **Single-writer inventory extended** (arch `single_writer.rs`):
   ANSWER_KEY_SURFACE_FILES += compose/init_cmd/reference_cmd (with reasons);
   WRITE_API_INVENTORY += 6 guarded sites (ensure_configured_dir / refusing to
   overwrite / installed_root / --out); fixture + pin test updated.
3. **`[[bin]] name = "gauntlet"`** added via `src/bin/gauntlet.rs` auto-discovery
   (spec §20.1's canonical command name; no manifest path literal).
4. **Rustfmt applied** to landed files fmt flagged (see Item 0).
5. **Load-flaky live-process test recurred once**: `b5_foreign_identity_is_
   contract_mismatch` (adapter-agent-cli falsifier) failed in the first full
   battery (MalformedResult vs contract_mismatch under load), passed on the
   re-run. Same known class as B1's note; not weakened; recurrence recorded.

## Deliberately not done
- Schema-check ladder rung (needs a required-keys contract no §8.6 setting
  carries) and the model judgment rung in the production ladder — recorded above.
- Fully-composed live-bwrap wrap-through-execution run — Task 25's leg per D3;
  this slice refuses (fail-closed) instead of running unwrapped.
- `status` without `--run` (no run-enumeration read on the port) — requires a run
  id, stated truthfully by the command.
- Dogfooding/commits — orchestrator's.

## Environment note
sccache's daemon holding the flock (known) was worked around all session with
`sccache --stop-server` inside the lock; the durable fix (O_CLOEXEC wrapper or
stopping the server in recipes) remains an orchestrator-level item.

## Addendum — recovery verification (second B2 session, 2026-08-31 18:27)

The session that wrote this report lost its final message; a fresh B2 session
reconciled the tree against the reports (per the AGENTS.md recovery rule) and
INDEPENDENTLY re-ran the full gate battery on the untouched final tree. Every
result above was reproduced:

- `just ruler` 0 · `just doctor` 0 (exit OK) · `just check` 0 · **dry-run gate
  exit 0 + ZERO_WRITES_CONFIRMED** (b2-verify-a.log)
- `just test` **1319/1319 PASS** (all four `cli_surface` tests inside; the
  load-flaky `b5_foreign_identity_is_contract_mismatch` PASSED this run) ·
  `just lint` 0 · `just deny` 0 · `just boundaries` 228/228 (b2-verify-b.log)

Corrections made by the verification session (markdown only, no source edits):
- `.agents/reports/reference-candidates.md` D5 entry: the golden path spelled
  `tests/golden/cli_help.txt`; corrected to
  `crates/gauntlet-cli/tests/cli_help.txt` (single-segment beside the test,
  the boundaries gate's literal rules). The `cli_surface.rs` doc comment still
  spells the pre-move path — a cosmetic nit; the `include_str!("cli_help.txt")`
  is the binding reference; left untouched to keep the verified tree exact.
