# B1 report — the conformance sweep (code + manifest)

Task 27 slice B1 (charter `.agents/reports/task27/charter.md`, decisions
1-6 + 11-12 binding). This report is a claim with NO settlement
authority. The REAL sweep run is the orchestrator's phase-gate action
(charter decision 12).

## What was built (files)

- `crates/gauntlet-app/src/conformance/` (new module; `mod` line added
  in `crates/gauntlet-app/src/lib.rs`):
  - `mod.rs` — the sweep orchestration ([`run_sweep`]): parse the frozen
    ruler (reuses `gauntlet_app::reference::parse`), parse + strictly
    validate the manifest, cross-validate against the ruler (set
    equality BOTH directions, kind equality, frozen-locator verbatim
    equality — each a typed refusal with NO report written), group
    identical execute commands and run each ONCE, adjudicate every unit
    fail-closed, assemble `gauntlet.conformance@v1`. Defines the
    `ConformanceExecutor` seam (the task-23/26 precedent: the app never
    spawns a process), `CommandOutcome { display, exit_code, stdout,
    stderr, duration_ms }`, and the typed `ExecuteError::CannotRun`
    (environmental ⇒ INCONCLUSIVE, never PASS, never FAIL-by-default).
  - `manifest.rs` — the strict `gauntlet.conformance-manifest@v1`
    codec: hand-rolled parser over a documented subset (the
    `baseline_task@v1` precedent; zero new dependencies). Exactly the
    eight entry keys, each exactly once, all required; exactly ONE
    evidence arm per entry; validated `status_hint` and `divergence`
    vocabularies; `file`/`metric` kinds restricted to artifact|none
    arms; artifact paths repo-relative (component-wise N4 guard) with
    NAMED predicates only; tabs/comments/drift each a typed line-anchored
    refusal.
  - `commands.rs` — the closed STRUCTURAL command vocabulary (see
    deviation D1): nextest arms `{package, target}` / `{package, lib,
    filter}` over an injectable package allowlist (core packages here;
    adapters only at the composition root), plus the bare
    absolute-executable fixture arm `{program}`. No shell, no command
    strings, invalid shapes unrepresentable.
  - `predicates.rs` — the named typed artifact predicates:
    `handoff_six_categories`, `baselines_keys_abcd`,
    `mutation_record_valid` (counts reconcile, missed == 0, shards all
    exit 0, 64-hex pin, dated).
  - `report.rs` — the frozen `gauntlet.conformance@v1` assembly: the
    mechanical status_category mapping (PASS+unbounded ⇒
    implemented_and_proven; PASS+bounded ⇒ implemented_but_provisional;
    INCONCLUSIVE ⇒ the entry's status_hint, `none` ⇒ `unknown`
    disclosed; FAIL ⇒ blocked); PASS structurally impossible without a
    non-empty repo-relative ON-DISK evidence_ref (assembly refuses;
    tested); the `_meta` disclosures.
- `crates/gauntlet-app/tests/conformance.rs` (new, 19 tests) — the
  FIXTURE world (tiny fake 4-unit answer key + fixture manifests +
  CountingExecutor over the true/false/echo binaries): every refusal
  class, all disposition semantics, dedupe, bounded mapping, assembly
  refusal, frozen-schema agreement, codec strictness. NEVER the real
  battery.
- `crates/gauntlet-cli/src/verify_cmd.rs` — `verify --all [--report
  <path>] [--manifest <path>]`: mutually exclusive with `--run`
  (usage error, exit 2, both orders; `--unit` likewise); fail-closed
  arg errors; defaults report = `<root>/.agents/reports/conformance.json`,
  manifest = `<root>/.agents/reports/conformance-manifest.yaml`
  (composed via `gauntlet_app::config::defaults::report_dir_at`); exits
  0 iff completed with zero FAIL (INCONCLUSIVE allowed); writes ONLY
  the report + the evidence dir (`<report parent>/conformance-evidence/
  <UTC-stamp>/NN-<slug>.log` + a `MANIFEST` recording command, exit,
  duration, sha256 per log); git head-commit resolution (HEAD → ref →
  packed-refs fallback); caller-resolved RFC3339 + stamp (civil-date
  arithmetic, no dependency). The `--run` mode is byte-identical (its
  arg-error messages and code path untouched; cli_surface's verify
  refusals stay green). `sweep_packages()` — the composition root's
  vocabulary (core + the two runner adapters; adapters named only here,
  outside the adapter-identifier denylist).
- `crates/gauntlet-cli/src/lib.rs` — one usage line added for `--all`.
- `crates/gauntlet-cli/tests/conformance_cli.rs` (new, 9 tests) — the
  REAL binary over a generated 70-entry fixture manifest (ruler shape
  embedded from the frozen key): mutual-exclusion exit 2; every
  refusal class exits non-zero with NO report; a clean sweep exits 0
  with the frozen report shape, 70 units, grouping asserted
  (`_meta.commands_executed == 1`), evidence-only write surface; a FAIL
  leg exits non-zero with the honest report + `exit=1` in the MANIFEST;
  an absent artifact is INCONCLUSIVE with exit 0; and the
  committed-manifest tooth (T1).
- `crates/gauntlet-cli/tests/cli_help.txt` +
  `crates/gauntlet-cli/tests/cli_help_surfaces.txt` — golden help
  regenerated per the documented procedure (surface change recorded
  here). NOTE: the surfaces golden was ALREADY STALE at HEAD (missing
  task-26's `improve baseline` usage lines) — a pre-existing defect
  found and repaired by this regen; the surfaces snapshot test would
  have failed under `--features surfaces` before it.
- `crates/gauntlet-adapter-verify-command-exit-code/src/
  sweep_toolchain.rs` (new module; see deviation D2) — the ONE place
  the sweep spells tool products: the repository's nextest invocation
  composition and the compile-lock wrapper (bounded `-w` wait; `-E
  LOCK_BUSY_EXIT` distinguishes an environmental lock refusal from a
  command failure; sccache stopped INSIDE the same locked command with
  the wrapped exit status preserved; never nested). Pure composition,
  no new dependencies; in-module unit tests (that crate is outside the
  literal scan, by construction).
- `crates/gauntlet-arch-tests/src/single_writer.rs` (see deviation D3)
  — the sweep's three production write sites registered in
  `WRITE_API_INVENTORY` (the suite's own documented remedy: "add the
  site to WRITE_API_INVENTORY with its reason and guard"), plus the
  synthetic fixture stub and the pinned-inventory expectation entries.
- `.agents/reports/conformance-manifest.yaml` — the authored input,
  ALL 70 units (below).

## The manifest — per-unit summary

- 70 entries, set-equal with the parsed frozen ruler both directions
  (the tooth test proves it against the real key).
- Arms: 67 execute / 3 artifact (`AK-067` handoff.md +
  `handoff_six_categories`; `AK-069` baselines.json +
  `baselines_keys_abcd`; `AK-070` mutation.json +
  `mutation_record_valid`) / 0 none. Every mapping mechanically exists
  and runs today (test targets verified against `cargo metadata` and
  the tree — never guessed).
- 45 distinct commands (identical commands group; e.g. the five p5
  units share one invocation, AK-004/AK-006 share `reference::parse`).
- bounded: true — `AK-053` (recorded partial §18.3 resume support,
  RES-A4), `AK-066` (recorded settleability tension, task-25 entry 2),
  `AK-069` + `AK-070` (recorded settlement artifacts per decision 11;
  the basis lines state exactly what ran and when).
- divergence: 44 dated `reference-candidates <date>` citations (every
  recorded locator-divergence entry), 9 `none` (AK-001/002/014/016/019
  verbatim arch targets; AK-067/068/069/070 at-or-achievable-verbatim
  per the charter), 17 `unrecorded` (NEW divergences — the list below;
  B3 is the single writer who owes their dated entries).

### AK-067 choice (dispatch asked; the pick)

Artifact predicate, NOT `evidence: none`: the file does not exist today
(B3, wave 2) so the interim disposition is the SAME honest
INCONCLUSIVE(`no_artifact`), and once B3 lands `handoff.md` the unit
adjudicates mechanically with no manifest edit — the predicate is
minimal and mechanical (non-empty UTF-8 naming all six §26 category
tokens), documented in `predicates.rs` for B3.

## RED-first record (honest)

All 19 app tests were authored BEFORE any mechanism existed (they
failed to compile against a absent module — the first RED). A typed
stub (real API, every body refusing with `RulerParse { not implemented
(RED stage) }`) was then compiled and the suite run:
`cargo nextest run -p gauntlet-app --test conformance` (under the
compile-lock flock) = **6 run / 6 FAILED behaviorally** for the expected
reason (the stub's wrong-variant refusal instead of each test's
asserted typed refusal or values: e.g. `FrozenLocatorMismatch, got
RulerParse { not implemented (RED stage) }`), 13 blocked by fail-fast
in the same run — the stub is uniformly discriminated. A per-test
`--no-fail-fast` RED re-run was not spent (a compile cycle); the GREEN
run then showed all 19 passing against the real mechanism. Genuinely
RED-first with an observed behavioral RED: the six executed (unknown-id
refusal, kind-mismatch refusal, none-arm INCONCLUSIVE, bounded mapping,
failing-command FAIL, frozen-locator refusal). Authored-before-mechanism
but RED observed only via the fail-fast-blocked run: the remaining 13
(codec strictness, vocabulary refusal, dedupe, evidence shape, schema
agreement, assembly refusal, executor-inability, absent/refuting
artifact...). The 9 CLI tests and the adapter's 6 were written against
the landed application surface (their mechanisms — the CLI flag surface
and the toolchain composition — did not exist; RED was the compile
failure; no behavioral RED cycle was spent on them).

## NEW divergences found (frozen locator vs real target; NO
reference-candidates entry exists — B3 owes the dated entries; I did
NOT write reference-candidates.md)

- AK-003 `tests/contract/typed_enums` → in-crate domain suite
  (`gauntlet-domain` lib, `every_fixed_value_set_enum_rejects_unknown_values`).
- AK-004 + AK-006 `tests/contract/answer_key_parse::*` → in-crate app
  suite (`gauntlet-app` lib, `reference::parse`).
- AK-007 same class → `gauntlet-domain` lib,
  `exhaustive_tier_authority_coupling_holds_on_every_construction_path`.
- AK-008..AK-012 `tests/contract/agent_runner::*` → the shared
  agent-runner contract suite lives in `gauntlet-ports` and RUNS as the
  adapters' `contract_suite` targets (mapped to the cli adapter's).
- AK-015 `tests/contract/config::precedence` → `gauntlet-app` lib
  (`precedence`).
- AK-017 `tests/e2e/doctor` → `gauntlet-app` lib (`doctor`).
- AK-018 `tests/contract/secrets::never_leaked` → `gauntlet-app` lib
  (`secrets_never_leaked`).
- AK-020 `tests/e2e/bounded_without_config` → `gauntlet-app` lib
  (`an_empty_world_resolves_to_the_defaults_table`).
- AK-059 `tests/fixtures/mock_runner_scenarios` — the fixture DATA
  exists at the frozen locator, but the test_report is produced by
  `gauntlet-adapter-agent-mock`'s `scenario_playback` target.
- AK-061 `build/cli-only-profile` (kind log) — the log path was never
  materialized; the real evidence is task-21's re-evidence (the mock
  full-loop E2E through the CLI-only default build + cli_surface's
  surface freeze).
- AK-064 + AK-065 `tests/contract/skills|formation::*` → the crate's
  own test targets (`gauntlet-app` `skills`/`formation`), recorded in
  the task-18/20 closures and the AK-063 entry's class, but with NO
  dedicated dated entry of their own.

## Charter deviations (with reasons)

1. **The execute arm is STRUCTURAL, not the charter example's command
   string.** `execute: {package: P, target: T}` / `{package: P, lib:
   true, filter: F}` / `{program: <abs>}` instead of `execute: "cargo
   nextest run ..."`. Reason: the frozen, mutation-defended AK-016
   literal boundary (verified live: it fired on my first draft)
   forbids in EVERY core crate (src and tests — the scanner recurses
   the whole crate tree): any literal equal to a tool identity
   ("cargo", "nextest"), any command-shaped multi-token literal, any
   absolute-path literal, any path-shaped multi-segment literal, and
   even rooted segments in comments. A string vocabulary is therefore
   unimplementable AND untestable in core scope; laundering tokens
   (split/concat/byte-strings) would be gaming a frozen gate and was
   refused. The structural form preserves every normative property
   decision 2 mandates — closed vocabulary, fail-closed, no shell —
   and STRENGTHENS it (invalid commands are unrepresentable; there is
   no string to inject). The report's `_meta` disclosure records the
   deviation and rationale.
2. **A new module in an adapter crate outside my declared file
   list**: `gauntlet-adapter-verify-command-exit-code/src/
   sweep_toolchain.rs` (+ its `pub mod` line). Reason: spec §8.4 keeps
   tool products in the adapter layer; the composition root itself is
   inside the literal scan, so the argv/display/compile-lock
   composition needs an adapter home. The crate's own header forbids
   gauntlet-app/cli dependencies — the module is pure std-only
   composition and adds none.
3. **`crates/gauntlet-arch-tests/src/single_writer.rs` edited** (B2's
   crate this wave, but a different file than B2's named scope):
   three `WRITE_API_INVENTORY` entries + the synthetic fixture stub +
   the pinned-list expectation. Reason: the boundary suite's own
   failure message prescribes exactly this remedy for new production
   write sites; removing the writes is impossible (the sweep must
   write its report + evidence — charter decision 4).
4. **The divergence vocabulary carries `unrecorded`** alongside
   `none | reference-candidates <date>`: a REAL divergence whose dated
   entry is owed (17 units). Claiming `none` would be false; a dated
   citation would be fabricated (the entries do not exist yet — B3 is
   the single writer). Disclosed in `_meta`.
5. **`cli_help_surfaces.txt` also regenerated** (not in my named file
   list): the shared usage() line changed, so the surfaces golden
   needed the identical regen; it additionally repaired a PRE-EXISTING
   staleness from task 26 (missing `improve baseline` usage lines).
6. **Minor**: `status_hint: none` + an INCONCLUSIVE outcome maps to
   the category `unknown` (disclosed in `_meta`) — an unforeseen
   inconclusive state (e.g. environmental cannot_run on an entry
   expecting PASS) must not block the sweep nor invent a hint.
   `SweepInputs` carries its path fields as Strings (mechanical).
   The evidence dir is derived from the report path's parent (the
   DEFAULT report path reproduces the charter's
   `.agents/reports/conformance-evidence/` verbatim; an explicit
   `--report` keeps tests hermetic).

## Slice battery (exact commands; all under the repo compile lock,
each flock its own top-level command with sccache stopped inside)

- RED: `cargo nextest run -p gauntlet-app --test conformance` → 6/6
  executed FAILED (behavioral; expected reason) + 13 fail-fast-blocked;
  non-zero exit.
- `cargo nextest run -p gauntlet-app --test conformance -p gauntlet-cli
  --test conformance_cli --test cli_surface
  -p gauntlet-adapter-verify-command-exit-code --no-fail-fast` →
  **32/32 PASS, 0 failed** (app 19 + cli 9 + cli_surface 4).
- `cargo nextest run -p gauntlet-adapter-verify-command-exit-code` →
  **6/6 PASS**.
- `cargo nextest run -p gauntlet-arch-tests --no-fail-fast` (= just
  boundaries' suite) → **245/245 PASS** (incl. the literal scanner,
  the denylist, single-writer with the extended inventory).
- `cargo nextest run -p gauntlet-app` → **646/646 PASS**.
- `cargo check --workspace --all-targets` → **exit 0**.
- `cargo check -p gauntlet-cli --features surfaces --all-targets` →
  **exit 0** (both feature configs compile; the surfaces golden
  matches its snapshot by construction of the regen).
- `cargo clippy -p gauntlet-app -p gauntlet-cli
  -p gauntlet-adapter-verify-command-exit-code --all-targets --
  -D warnings` → **exit 0**.
- `cargo fmt --check` over the four touched crates → **exit 0**.
- `./scripts/verify-ruler.sh` (`just ruler`) → **PASS**, run at
  mid-slice and at the end; the ruler never moved (nothing under
  `.wayfinder/` or any reference.lock was touched).

## Known defects / limits

- The REAL sweep has never been executed end-to-end (the orchestrator's
  phase-gate action, per the charter and my dispatch). The executor's
  nextest path is covered by the adapter's composition unit tests
  (argv, display, lock wrapper, exit preservation) and the CLI paths
  are proven over the fixture arm; the first real run may still surface
  environment-level facts (a busy compile lock adjudicates honestly to
  INCONCLUSIVE with the typed reason, never a wrong FAIL).
- The sweep imposes no per-command runtime timeout (the lock
  ACQUISITION is bounded at one hour); 45 distinct nextest invocations
  under the repo's configured parallelism is a long but bounded run.
- AK-068 (`docs_present`) FAILS the sweep until B2's target lands —
  fail-closed by design; the IN-FLIGHT ledger sequences the first real
  sweep after wave 1 (B1+B2).
- `conformance.json` is not generated or committed by this slice
  (decision 12: the orchestrator runs and commits it at the phase
  boundary, re-generating at settlement).
- The committed-manifest tooth asserts `pass == 2` over the recorded
  artifacts — deleting `baselines.json` or `mutation.json` (the T2
  tooth) flips it RED via `no_artifact`, which is the intended bite.
- The full `gauntlet-cli` package battery (e2e real-runner legs) was
  not re-run by this slice (the orchestrator's full battery owns it);
  the CLI targets I touched and `cli_surface` are green, the `--run`
  code path is byte-identical, and the workspace type-check passes for
  both feature configurations.

## COR-1 (2026-09-07, after the orchestrator's FIRST REAL SWEEP)

The first real sweep (evidence `.agents/reports/conformance-evidence/
20260907T193758Z/`) executed two defects in my deliverable. Both fixed;
the third item below is the missing test class that let the first
defect through.

### What was wrong (executed evidence)

1. **BLOCKING — the compile-lock wrapper's argv order was wrong.**
   `lock_wrapped` composed `flock <lockfile> -w <secs> -E 86 -c
   <script>`; flock requires OPTIONS BEFORE the file operand, so flock
   treated the token after the file as the command to execute. Every
   one of the 45 commands died in ~50ms with `flock: failed to execute
   -w: No such file or directory`, exit 69 (e.g. `01-gauntlet-arch-
   tests-architecture-boundaries.log`, MANIFEST line `exit=69
   duration=64ms`).
2. **Classification — flock-level failures were adjudicated FAIL**
   across the 67 executed units, violating charter decision 3 (executor
   inability ⇒ INCONCLUSIVE, never FAIL-by-default).

### What changed

- `sweep_toolchain.rs`:
  - `lock_wrapped`/`lock_wrapped_over` now compose `flock -w <secs> -E
    <code> <lockfile> -c <script>` — options before the file operand
    (the empirically proven order).
  - Reserved-exit disambiguation contract, documented and pinned:
    `LOCK_BUSY_EXIT = 86` and `LOCK_EXEC_FAILED_EXIT = 69` are
    FLOCK-LEVEL; the wrapper script REMAPS the inner command's own 69
    to `INNER_EXIT_REMAP = 169` (`if [ "$__sweep_status" -eq 69 ];
    then __sweep_status=169; fi`) before exiting — so {86, 69} mean the
    lock layer and 169 means the inner command exited 69 on its own
    merits.
  - `lock_wrapped_over(lock, wait, inner)` is the explicit seam the
    behavioral tooth needs (temp lock, short bound); the production
    `lock_wrapped` delegates over `COMPILE_LOCK` with the production
    wait — unchanged call site.
- `verify_cmd.rs` (the executor): the flock-level classification is
  ARM-AWARE — only the locked (nextest) arm reinterpretts {86, 69} as
  the typed environmental `CannotRun` (distinct reasons; the 69 reason
  carries the wrapper's first stderr line for diagnosability); the
  bare-program arm owns every code it produces. CannotRun ⇒ INCONCLUSIVE
  was already the app's pinned behavior (tested there).
- `report.rs`: the compile-lock `_meta` disclosure now states the
  environmental classification and the remap contract.

### The missing tooth (why defect 1 survived my battery)

My adapter unit test asserted the argv SHAPE (`args[0] == lockfile`,
`args[1] == "-w"`) — it enshrined the defect verbatim. The composition
was never EXECUTED: the task24/26 recurrence (shape-pinned, never run).
Added three BEHAVIORAL teeth that run the REAL flock over the REAL
wrapper composition in a temp lock:
- `the_wrapper_executes_the_inner_command_under_the_real_flock` —
  inner `true` exits 0 THROUGH the wrapper; inner `false` exits 1
  (preserved across the sccache stop). This leg is RED under the
  defect-1 composition (it would observe 69 + `failed to execute -w`).
- `the_inner_own_exit_69_is_remapped_so_the_reserved_pair_stays_flock_level`
  — a stub executable exiting 69 surfaces as 169; one exiting 7 passes
  through untouched.
- `a_held_lock_reports_the_busy_code_within_the_bounded_wait` — a real
  holder process holds the lock; the wrapper with `-w 1` reports 86
  (environmental busy, not a command failure).
The shape test remains (renamed to `..._puts_options_before_the_file_
operand`, asserting the corrected order) as the fast guard beside the
behavioral legs.

### Honest limit

The CLI executor's {86,69}→CannotRun mapping itself has no core-scope
test: driving the locked arm hermetically would require running cargo
(the real battery — forbidden in tests) or spawning from a scanned core
file (the AK-016 Command::new literal rule). The composition legs are
proven end-to-end in the adapter (where spawning is legitimate), and
CannotRun ⇒ INCONCLUSIVE is proven at the app layer; the six-line
mapping between them is the residual untested seam.

### COR-1 battery (all under the repo compile lock; every flock its own
top-level command, sccache stopped inside)

- `cargo nextest run -p gauntlet-adapter-verify-command-exit-code
  --no-fail-fast` → **9/9 PASS** (6 prior + 3 behavioral teeth; the
  busy leg holds a real lock for ~4s).
- `cargo nextest run -p gauntlet-app --test conformance -p gauntlet-cli
  --test conformance_cli --test cli_surface --no-fail-fast` →
  **32/32 PASS** (app 19 + cli 9 + cli_surface 4).
- `cargo nextest run -p gauntlet-arch-tests --no-fail-fast` →
  **245/245 PASS**.
- `cargo check -p gauntlet-app -p gauntlet-cli
  -p gauntlet-adapter-verify-command-exit-code --all-targets` → exit 0.
- `cargo clippy` over the same three crates `--all-targets --
  -D warnings` → exit 0 (one `useless_format` found and fixed in the
  new classification code).
- `cargo fmt --check` over the same three crates → exit 0.
- `./scripts/verify-ruler.sh` (`just ruler`) → **PASS**; the ruler
  never moved.
- The real sweep was NOT re-run (the orchestrator's phase-gate action).

## COR-2 (2026-09-07, after sweep-2: 68 PASS / 1 INCONCLUSIVE (AK-067,
correct, wave 2) / 1 FAIL — AK-062, `command exited 4`)

### What was wrong (executed evidence)

AK-062's whole test file `crates/gauntlet-cli/tests/
operator_actions_no_bypass.rs` is gated `#![cfg(feature = "surfaces")]`
(line 34). Under the default feature config the binary compiles with
ZERO tests; nextest exits 4 "no tests to run" (log
`.agents/reports/conformance-evidence/20260907T200600Z/
40-gauntlet-cli-operator-actions-no-bypass.log`: compiled clean,
exit 4). The unit's evidence exists and is green under `--features
surfaces` — the manifest's command was incomplete: the vocabulary had
no features dimension. (The exit-4 ⇒ FAIL classification itself is
CORRECT and unchanged — zero tests ran is never PASS; it did its job.)

### Discovery (per the correction's instruction — not hardcoded blindly)

The feature edges of every package in the vocabulary's allowlist, read
from their `[features]` tables: gauntlet-app declares `composition`;
gauntlet-cli declares `surfaces` (plus `default`, implicit everywhere
and deliberately excluded from the vocabulary — passing it adds
nothing); gauntlet-domain, gauntlet-ports, gauntlet-arch-tests, and
both runner adapters declare NONE. So the validated feature vocabulary
is exactly {composition, surfaces}. A swept grep of all 45 mapped test
files for `cfg(feature` found exactly ONE other file:
`cli_surface.rs` uses INNER `#[cfg(feature = "surfaces")]` items only —
its DEFAULT-config tests exist and run (its snapshot is the default
build's own freeze), so AK-060's entry is correct as mapped. AK-062
was the only file-level gate.

### What changed

- Vocabulary (app `commands.rs` + `manifest.rs` + `mod.rs`): the
  execute arm gains an optional `features: [edge, ...]` flow sequence,
  validated fail-closed — unknown edges, duplicates, malformed
  sequences, and features-on-the-program-arm all refuse (typed
  `BadFeature`/`DuplicateFeature`/`BadEvidenceArm`). `SweepCommand::
  Nextest` carries `features: Vec<String>` (grouping treats it as part
  of the command — two entries differing only in features are distinct
  commands); the structural display renders it
  (`nextest[gauntlet-cli:test=x@surfaces]`).
- Vocabulary injection: `parse_manifest_with(text, packages, features)`
  — the application default is `WORKSPACE_FEATURES = [composition,
  surfaces]`, PINNED to the core packages' Cargo.tomls by a test that
  parses their `[features]` tables (a new edge fails until inventoried;
  `default` excluded). The composition root owns the full-allowlist
  edges: `verify_cmd::sweep_features()` — the adapters declare none, so
  it is the same pair, documented with the derivation.
- Adapter composition (`sweep_toolchain.rs`):
  `nextest_invocation(..., features)` emits `--features <joined>`
  directly after the package and BEFORE the target selection. Placement
  verified EMPIRICALLY against the real tool: (a) a bogus edge in this
  position fails at metadata stage naming the feature (`does not
  contain this feature: not-a-real-feature`, exit 101 — proving the
  flag is parsed there); (b) `cargo nextest list -p gauntlet-cli
  --features surfaces --test operator_actions_no_bypass` compiles the
  surfaces config and LISTS the gated target's tests — the exact
  composed placement, end to end (list only; no tests executed).
- Manifest: AK-062's entry now carries `features: [surfaces]`; its
  basis states the config requirement ("the whole target is
  feature-gated; the default config compiles zero of its tests").
- Tests (RED-first where practical): the features field was added as a
  RED PROBE first and observed failing for the expected reason
  (`unknown execute key "features"`) before the mechanism landed; then
  the full set: parse-into-typed-shape + display (app), unknown/
  duplicate/malformed/program-arm refusals (app), the WORKSPACE_FEATURES
  workspace pin (app), the argv-placement composition test (adapter,
  RED until the param existed), and the committed-manifest tooth +
  sweep_features pin (cli — the tooth now proves the updated manifest
  cross-validates with the feature edge admitted).

### Honest residual

The feature vocabulary is a FLAT union across the allowlist, not
per-package: an arm naming an edge a DIFFERENT allowlisted package
declares (e.g. `composition` on gauntlet-domain) parses and is refused
only by cargo itself at execution time — a loud FAIL, never a silent
PASS. Per-package mapping is a widening if the orchestrator wants
refusal-at-parse; recorded rather than built (no current manifest entry
needs it).

### COR-2 battery (same discipline: every flock its own top-level
command, sccache stopped inside)

- `cargo nextest run -p gauntlet-app --test conformance
  --no-fail-fast` → **22/22 PASS** (19 prior + 3 new).
- `cargo nextest run -p gauntlet-adapter-verify-command-exit-code
  --no-fail-fast` → **10/10 PASS** (9 prior + the placement test).
- `cargo nextest run -p gauntlet-cli --test conformance_cli --test
  cli_surface --no-fail-fast` → **13/13 PASS** (9 + 4).
- `cargo nextest run -p gauntlet-arch-tests --no-fail-fast` →
  **245/245 PASS**.
- `cargo check` + `cargo clippy --all-targets -- -D warnings` over the
  three touched crates → exit 0.
- `cargo fmt --check` over the same → exit 0.
- `./scripts/verify-ruler.sh` (`just ruler`) → **PASS**; the ruler
  never moved.
- The real sweep was NOT re-run (the orchestrator's action).

## COR-3 (completed by the ORCHESTRATOR under the heal+continue exception)

B1's agent died on a usage-limit window mid-COR-3 (2026-09-08 ~14:08
harness time; window resets 14:42). The code and tests below were
verified LANDED IN THE TREE before the death (git diff audit per fix);
this section records them from orchestrator-verified evidence, not
builder claim. The scoped battery was re-run by the orchestrator.

- F1 (duplicate ids): `ManifestError::DuplicateId` typed refusal,
  one-entry-per-ruler-unit enforced in `parse_manifest_with`
  (manifest.rs; "COR-3 F1: one entry per ruler unit"). Test:
  `duplicate_manifest_ids_refuse`.
- F2 (kind<->arm matrix exact): test_report|log => execute ONLY;
  file|metric => artifact|none ONLY; other kinds refused at the
  matrix ("kind ... is outside the sweep's kind-arm matrix"). Test:
  `the_kind_arm_matrix_is_exact`. The committed manifest verified
  compliant (no content change needed).
- F10 (signal-killed != FAIL): `code()==None` maps to the typed
  `ExecuteError::CannotRun` naming the signal via ExitStatusExt
  (verify_cmd.rs:123 region; never a fabricated "exit -1"). Test:
  `a_signal_killed_command_is_the_typed_environmental_refusal`
  (self-killing stub, expects CannotRun with "signal" and "9").
- F3 (predicate grammar): `handoff_six_categories` demands a
  `## <category>` heading per category (trailing count suffix fine);
  a bare word-legend refutes ("the handoff carries no '## ...'
  heading"). Test: `the_handoff_predicate_demands_section_headings`.
- F7+F13 (_meta disclosures, no behavior change): the arm-dependent
  evidence_ref anchor rule (execute refs resolve from the report's
  parent; artifact refs from the workspace root) and the lib-filter
  substring semantics + over-determination/fail-closed direction +
  recorded drift risk are both disclosed in the report assembly.
- Battery: the orchestrator's full workspace battery over BOTH
  corrections (COR-3 + B3's COR-B3) — results recorded in the
  settlement/ledger records of this task.

## COR-3 (2026-09-07, falsifier round 1 findings F1/F2/F10/F3/F7/F13 —
each with an executed repro; code verified landed by the orchestrator)

- **F1 (MAJOR, forged PASS)**: duplicate manifest unit ids were
  ACCEPTED, the first entry silently shadowing later ones (executed:
  AK-901 twice, passing arm first → PASS with the forged basis, the
  shadowed entry nowhere). Fix: `ManifestError::DuplicateId { id,
  first_line, line }` raised at entry-open in `parse_manifest_with` —
  set equality is now on multisets. RED: `duplicate_manifest_ids_refuse`
  observed FAILED (`duplicate parses, no refusal`) before the fix.
- **F2 (MAJOR, evidence-kind laundering)**: a test_report unit could
  carry an ARTIFACT arm and PASS implemented_and_proven with ZERO
  commands. Fix: the kind/arm matrix is EXACT — test_report/log ⇒
  execute, file/metric ⇒ artifact|none, kinds outside the matrix refuse
  outright — with the ONE exception charter decision 11 defines
  (bounded recorded artifact; see COR-4 R2-F4 for its final scoping).
  RED: `the_kind_arm_matrix_is_exact` observed FAILED before the fix.
- **F10 (MAJOR, false FAIL on environmental kill)**: a signal-killed
  command was recorded FAIL "command exited -1" (the executor's
  `code().unwrap_or(-1)` fabricated a code; the falsifier kill -9'd a
  lock-waiting wrapper). Fix: `code() == None` ⇒ typed CannotRun naming
  the signal (via `ExitStatusExt`, cfg-scoped); no evidence log is
  written for a command that never completed, so no log can record a
  fabricated exit. The unopenable-lock-file class rides the same path
  (spawn/IO error ⇒ CannotRun). RED: `a_signal_killed_command_is_the_
  typed_environmental_refusal` observed FAILED with the fabricated
  `exit_code: -1` in the panic output.
- **F3 (MINOR, predicate accepts a legend)**: handoff_six_categories
  held on one line of word salad. Fix: the predicate demands the
  section grammar — a `## <category>` heading (first token after the
  marker) for each of the six; suffixes like unit counts stay fine.
  RED: observed `a word-salad legend refutes, got Ok(Holds)`.
- **F7+F13 (MINOR, disclosures only)**: added verbatim to `_meta`:
  the arm-dependent evidence_ref anchoring (execute refs from the
  report's parent; artifact refs from the workspace root; the committed
  report is at the default locator where both resolve) and the
  substring-filter semantics + over-determination direction + the
  recorded drift risk. No manifest content change (F13 adjudicated
  disclose-and-record).

## COR-4 (2026-09-07, falsifier round 2 findings R2-F1/F2/F3/F4/F6 —
each executed against my COR-3-corrected surfaces)

- **R2-F1 (MAJOR, inner-86 misattributed lock-busy)**: the remap was
  half-built — the wrapper remapped inner-69→169 but never inner-86, so
  a command failing with its own 86 was recorded INCONCLUSIVE "lock
  stayed busy" and a single-unit world exited 0. Fix: the wrapper also
  remaps inner-86→186 (`INNER_EXIT_REMAP_86`); the CLI classifies
  through a new pure `classify_exit` table (adapter — testable without
  cargo); 169/186 land as FAIL with the reason naming the remap AND the
  indistinguishability ("a direct exit of this code is indistinguishable
  from the remapped form — disclosed"); the lying `{86,69} unambiguous`
  comment is replaced; the `_meta` disclosure names both remaps. Teeth:
  adapter behavioral `the_inner_own_exit_86_is_remapped...` (real
  wrapper, stub exiting 86 → 186), `the_exit_classification_table_holds`
  (pure table incl. 169/186/200..231/232/199 endpoints).
- **R2-F2 (MAJOR, inner signal death still FAIL-by-default)**: kill -15
  → FAIL "exited 143", kill -9 → "exited 137" — the wrapper's sh
  encoded 128+n and COR-3 covered only a wrapper-level kill. Fix: the
  wrapper remaps the shell signal band to `200+signal` (200..231,
  outside every flock code); the CLI classifies 200..231 as the typed
  CannotRun naming the signal (the realistic OOM victim is the inner
  process). Band-ambiguity caveat disclosed (a genuine inner exit
  200..231 is indistinguishable — it lands INCONCLUSIVE, the side that
  claims nothing). Teeth: `an_inner_signal_death_surfaces_in_the_signal_
  band` (kill -15 → 215, kill -9 → 209, REAL wrapper) and the
  pass-through pin `a_genuine_inner_band_exit_passes_through_disclosed`.
- **R2-F1/F2 correction-of-the-correction (found by MY OWN tooth)**:
  the r2-specified 128..191 band consumed a genuine inner 169 (executed
  in my tooth: 169 → 241). Narrowed the band to 129..159 — the standard
  signals 1..31 are exactly 129..159 — which preserves the remapped
  169/186 AND remaps every standard signal death; a real-time-signal
  death (160..192) remains a plain FAIL (disclosed residual, failure
  direction). The module doc records the executed 169→241 evidence.
- **R2-F3 (MAJOR, payment)**: the unconditional `sccache --stop-server`
  stalled ~2m14s per command whenever no daemon was live — lock-held
  (~100 min/sweep; the falsifier measured 134s floor vs 0.009s bare
  flock). Fix: `timeout 10 sccache --stop-server >/dev/null 2>&1 ||
  true` INSIDE the locked command (the daemon-never-holds-the-lock
  invariant preserved; a >10s stop is killed and its exit swallowed).
  **Timing before/after (adapter suite, nextest test-phase Summary
  wall)**: COR-3 era ~269s per the falsifier's measurement of these
  same wrapper-driven tests → **10.4s** after the fix (the suite grew
  9→14 tests in between; the per-command floor is gone — the remaining
  seconds are the held-lock leg's deliberate sleeps). Total command
  wall times in this session (~140-158s) are compile + contention with
  the parallel builder's locks, not test time.
- **R2-F4 (MINOR, dead latitude)**: log × bounded-artifact was ACCEPTED
  (PASS provisional, zero commands on a text artifact). Fix: the
  bounded recorded-artifact exception is TEST_REPORT only (charter
  decision 11 names exactly one instance, AK-070, kind test_report).
  RED-first: `the_bounded_artifact_exception_is_test_report_only`
  written against the accepting behavior, observed FAILED, then green.
  The committed manifest is unaffected (AK-070 is test_report; the
  tooth re-proves it).
- **R2-F6 (cosmetic)**: the DuplicateId message's mangled 17-space run
  fixed; `the_duplicate_id_refusal_is_clean_prose` asserts no doubled
  spaces and both line loci.

### COR-3 + COR-4 battery (every flock its own top-level command; sccache
stopped inside; B3's parallel gates contended for the lock — wall times
include queuing, test-phase Summaries are the honest figures)

- app conformance: **27/27 PASS** (22 + duplicate-id, matrix, handoff
  grammar, bounded-scope, clean-prose; the COR-3 F1/F2/F3 trio observed
  behavioral RED pre-fix).
- adapter: **14/14 PASS** (test-phase 10.4s; the COR-1 behavioral teeth
  + R2-F1's 86-remap, R2-F2's signal-band legs, the pass-through pin,
  the classification table, and the bounded-daemon-stop shape pin).
- cli lib (incl. the F10 signal tooth): **17/17 PASS**; conformance_cli
  + cli_surface: **13/13 PASS**.
- arch-tests: **261/261 PASS** (the suite grew with B3's landed work;
  the earlier combined-run denylist FAIL was a stale-tree transient —
  green in isolation and in this final full run).
- `cargo check` + `cargo clippy --all-targets -- -D warnings` over the
  three touched crates → exit 0; `cargo fmt --check` → exit 0.
- `./scripts/verify-ruler.sh` (`just ruler`) → **PASS**; the ruler
  never moved.
- The real sweep / settlement re-sweep NOT re-run (the orchestrator's).

## COR-5 (2026-09-07, falsifier round 3 — CONFIRM; 4 residuals, all
disclosure/classification polish; behavior already correct both directions
where the falsifier verified it)

- **R3-F1 (MINOR, disclosure gap)**: a CHOSEN inner exit inside the
  signal band (129..159) is classified as a signal death, and that
  indistinguishability was named nowhere (DISCLOSURES listed only
  169/186/200..231; the module doc asserted the band "was KILLED").
  Fix: the `_meta` disclosure names the chosen-exit band ambiguity; the
  module doc says "is classified as"; the reason is now the toolchain's
  pure `inner_signal_reason` — "classified as killed by signal N (the
  wrapper's shell encoded exit status S — a chosen exit in this band is
  indistinguishable from a signal death)". Toothed at both layers
  (adapter reason tests; app `the_disclosures_name_the_chosen_exit_
  band_ambiguity` pins the sentence).
- **R3-F2 (MINOR, impossible facts)**: inner exit 200 produced "killed
  by signal 0 (encoded as 128)" — doubly impossible. Fix: one boring
  branch in `inner_signal_reason` — observed 200 reads "the inner
  command's exit status 200 falls in the reserved signal band with no
  valid signal encoding — classified environmental (INCONCLUSIVE claims
  nothing)". Toothed: the degenerate entry asserts neither "signal 0"
  nor any encoding.
- **R3-F3 (COSMETIC)**: the kind-arm refusal text advertised the
  bounded recorded-artifact form to log-kind authors. Fix: kind-specific
  prose (log ⇒ execute-only rule; test_report ⇒ the decision-11
  exception). Toothed in the matrix test (log text contains
  "execute arms only", never "recorded-artifact").
- **R3-F4 (MINOR)**: an artifact-only sweep into a NONEXISTENT report
  parent adjudicated fully then died at the report write (os error 2) —
  the evidence loop only creates parents for evidence files, and an
  artifact-only run has none. Fix: `create_dir_all` the report's parent
  up front (real write errors stay fail-explicit). Toothed end-to-end
  through the REAL binary: `an_artifact_only_sweep_creates_a_missing_
  report_parent` (scratch project-root world: 2-unit file/metric
  fixture ruler + detached HEAD, report at out/deep/conformance.json →
  exit 0, report written).

### COR-5 battery (same discipline; B3 contention in the walls, Summaries
are test-phase)

- adapter: **15/15 PASS** (14 + the reason-builder teeth).
- app conformance: **28/28 PASS** (27 + the disclosure pin and the
  kind-specific-prose assertions).
- cli lib + conformance_cli + cli_surface: **31/31 PASS** (30 + the
  artifact-only-world tooth).
- arch-tests: **261/261 PASS**.
- `cargo check` + `cargo clippy --all-targets -- -D warnings` (one
  `useless_format` in the new branch found and fixed) → exit 0;
  `cargo fmt --check` → exit 0.
- `./scripts/verify-ruler.sh` (`just ruler`) → **PASS**; the ruler
  never moved.
- The settlement re-sweep NOT re-run (the orchestrator's).
