# B3 report — task 26: CLI wiring + gate test (completion contract)

Existence of this file with the slice battery results below is the
completion claim. It carries NO settlement authority (charter §Battery).
The orchestrator owns integration (the plan gate) and closure.

**HEADLINE: the plan gate CANNOT pass against the fixture as shipped —
two independent fixture/contract defects stop profile D, and with it the
whole report (B2's `assemble` requires all four profiles). The gate test
is deliberately RED with exactly the first defect's error. Both defects
are outside B3's scope; exact repros below.** Everything else in the
slice landed and is green.

## What landed (files)

- `crates/gauntlet-cli/src/improve_cmd.rs` — the `improve baseline`
  subcommand (verbatim charter spelling; house-style arg loops, unknown
  args → usage + exit 2) and `BaselineCliExecutor`, the CLI
  composition root's [`gauntlet_app::baseline::BaselineExecutor`] impl:
  fresh scratch world per leg, A/B/C dispatches through the REAL
  `cli-agent` adapter, profile D through the REAL composed loop
  (`compose_run` → `start_run`, the `ComposedExecutor` precedent), the
  held-out instrument as ground truth, sha2 fixture digests for `_meta`,
  atomic report write (the roles module's `write_atomic` pattern mirrored
  inline: stage → fsync → rename → dir-entry fsync), per-profile summary
  line on stdout, typed exit mapping (Configuration → 2, Execution/
  Inconclusive → 1).
- `crates/gauntlet-cli/src/lib.rs` — the root `usage()` line + module
  doc (surface consistency; routing needed no change).
- `crates/gauntlet-cli/tests/cli_help.txt` — REGENERATED golden
  snapshot: the ONLY diff is the two new baseline usage lines (verified
  by diff before committing). Deliberate, recorded update per the
  cli_surface.rs header procedure.
- `crates/gauntlet-cli/tests/improve_baseline.rs` — NEW: drives the REAL
  binary (`CARGO_BIN_EXE_gauntlet`) through the plan gate's exact command
  shape and asserts the charter matrix, the payment determinism, and the
  T2/T3 composition-half teeth (inventory below).
- `.agents/reports/task26/b3-progress.md` / this file.
- `report_manifest.rs` RECONCILED, no change: rows are per top-level
  command; `improve` is already wired (RES-15 satisfied by construction).
- `cli_surface.rs` needed NO change: `improve` is in the command tables
  already; only the golden text moved.

## The exact command surface

```
gauntlet improve baseline --profiles <A,B,C,D> --tasks <dir> \
                    [--out <path>] [--config <path>]
```

(in both `improve_cmd::usage()` and the root `gauntlet --help`).
`--profiles` is required and parsed by B2's `parse_profile_list`
(fail-closed, exit 2 naming token + vocabulary); `--tasks` is required,
must be an existing directory, subdirectories with a `task.yaml` are
enumerated deterministically sorted and strictly parsed (`baseline_task@v1`;
refusals exit 2 naming the line/mechanism); `--out` defaults to
`.agents/reports/baselines.json` resolved against the CURRENT WORKING
DIRECTORY (the plan gate's literal path); `--config` overrides each leg's
own config discovery. A partial `--profiles` selection RUNS only its
legs, then B2's `assemble` refuses the missing profile by name (exit 2,
no report written) — observed and asserted.

## The executor seam (composition decisions, all recorded in source)

- **Scratch worlds**: per leg `<legs-root>/<task>.<scenario>-<n>/world/`,
  the task project copied to `<leg>/world/workspaces/<scenario>` with the
  state dir at `<leg>/world` — so the adapter's execution anchor IS the
  project copy: A/B/C children run in the real project (base source +
  declared check present), nothing ever writes into `tests/fixtures/`.
  `<n>` is a per-run materialization counter: every leg owns its
  directory so a later leg's fresh-world wipe can never destroy an
  earlier leg's retained records.
- **The per-leg scenario channel** (design fact this workspace imposes):
  `unsafe_code = "forbid"` (workspace lints) makes edition-2024
  `std::env::set_var` unusable, so the brief's "set `GAUNTLET_BASELINE_MODE`
  per leg" cannot be implemented as env writes. Channels used instead,
  all documented in source:
  1. ONE static child command template (machine-independent, no caller
     data) wrapping the dispatch in `sh -c`: it reads the scenario and
     the child path from per-leg FILES (`<leg>/baseline-mode`,
     `<leg>/baseline-child`) via `$2/../../../` — every dispatched
     workspace (A/B/C's project copy AND D's fresh per-unit workspace)
     sits at `<leg>/world/workspaces/<key>`, so the leg directory is
     always three levels up. `$`/parens are data to the template parser
     (shell-word rules; placeholders render as single argv words, no
     shell re-splitting).
  2. A one-time safe re-exec (`Command::env`, `current_exe`, stdio
     inherited, exit propagated; recursion-guarded by template
     equality) establishing the three STATIC env reads the composed
     loop makes: `GAUNTLET_CLI_AGENT_CMD` (the static template),
     `GAUNTLET_TOOL_TEST` (the task project's own declared check —
     leg-independent because the check examines the dispatched
     workspace through its cwd), `GAUNTLET_ID_SEED` (one shared seed;
     every leg has its OWN state DB, so there is no cross-leg
     collision and the runs are reproducible).
  3. Per-leg state/artifact roots and the D legs' control settings
     (`GAUNTLET_STALL_TIMEOUT_MS`=5000, `GAUNTLET_POLL_INTERVAL_MS`=1000,
     retry 100/2000, attempts 3) injected INTO the config `EnvLayer`
     (last occurrence wins) — never into the process environment.
- **Deviation from the brief's wiring note (documented)**: for the
  `cli-agent` token the A/B/C runner is constructed as the SAME
  production `CliAgentRunner` `runner_for` builds, but with
  `CliAgentConfig::new(<static template>)` instead of `from_env()` — the
  template cannot ride the environment (see above). Runner SELECTION
  still goes through `select_runner_token` over the leg's own resolved
  config (fixture-level delegation); any non-`cli-agent` token keeps the
  standard `runner_for` path. No containment seam (charter decision 3).
- **Dispatch mechanics**: envelope = body + `content_digest` per B2's
  request builders; invoke → bounded poll (25 ms tick, 800-probe
  backstop) → collect. The runner's own 10 s spec deadline pins
  `TimedOut` (the transport's whole-group stop retires the child — the
  task-25 F6 lesson); a belt-and-suspenders `cancel` follows, tolerating
  the typed `AlreadyTerminal` no-op. Timed-out/refusing children are
  OUTCOMES (`claimed_complete: false`), never errors. The critic's
  verdict maps from its self-check exit (0 → Pass, nonzero → Fail,
  missing → Inconclusive — never a passing verdict).
- **Executor-provided figures**: A/B/C builder dispatches carry a
  base-state regression comparison (declared source-level consts present
  at materialization and missing after; basis `base-state-comparison`).
  D's `tool_calls`/`regressions` are honestly `None` (no counter at this
  seam; the loop's instruments cannot see the preserved property — no
  metric invented into existence). `reserve_intact` = the durable
  checkpoint's `budget_spent` against the leg's OWN effective config via
  the domain's `would_breach_reserve` (no checkpoint ⇒ false — missing
  evidence is not passing evidence).

## Test inventory (`crates/gauntlet-cli/tests/improve_baseline.rs`)

All `--out` paths live in the test's scratch world; the repo default is
never written by this suite. Charter teeth marked.

1. `the_plan_gate_shape_writes_the_charter_report_and_the_matrix_holds`
   — THE gate command (verbatim shape, repo-root cwd, report into the
   scratch): exit 0; report keys exactly `["A","B","C","D","_meta"]`
   (the plan gate's python assert mirrored); honest-posture disclosure
   present; the full derived scenario matrix per profile; D pays the
   most on the DETERMINISTIC dimensions (totals dispatches +
   instrument_runs > A/B/C's — never wall-clock ordering); T3 durable
   half (D lying-defect: zero settlement rows in the leg's OWN store,
   every store holding the run); T2 composition half (the critic
   envelope body is the requirement VERBATIM — equality, parsed through
   the app codec — with the forged claim needle and result vocabulary
   absent). **RED — see the blocker below.**
2. `a_partial_profile_selection_runs_only_its_legs_and_is_refused_at_assembly`
   — `--profiles A,B` runs only its legs, assemble refuses naming
   profile C (exit 2), no report written. GREEN.
3. `unknown_profile_tokens_are_refused_fail_closed` — `A,X` → exit 2
   naming the token (T4 at the CLI seam). GREEN.
4. `a_missing_tasks_directory_is_refused` — missing flag and
   nonexistent dir → exit 2 naming the mechanism. GREEN.
5. `an_unknown_scenario_in_a_fixture_task_is_refused` — a scratch task-set
   copy with `sneaky-mode` → exit 2 naming the token (the fixture itself
   is never modified). GREEN.

## Slice battery results

- `just check-package gauntlet-cli` — exit 0, NO warnings (flock'd
  top-level; `sccache --stop-server` inside the same flock; no nested
  flock; no workspace-wide runs).
- `just test-package gauntlet-cli` — exit 100 (the expected red):
  **139 tests run, 138 passed, 1 failed — the failure is exactly
  `the_plan_gate_shape_writes_the_charter_report_and_the_matrix_holds`,
  blocked by defect D1 below** (no other test regressed; the
  `b1_contract_wins_over_exit7` load-flake watch item did not fire).

## Observation runs beyond the battery (diagnostic, fixture untouched)

To observe the D half while D1/D2 block the pristine fixture, two
SCRATCH-COPY diagnostic runs were made (copies under /tmp; the task set
amended only there — manifest prose fix + lock refreeze, then a
two-scenario variant; the repo fixture was never written):

- A/B/C rows verified against the charter EXACTLY (per-profile summaries
  over the full five-scenario set): A — 4 completions incl. the lying
  false completion, 3 escapes (lying, subtle, compromised), silent
  bounded; B — 2 completions (honest + subtle shared-criterion escape),
  10 dispatches, 5 instrument runs; C — 2 completions (honest +
  compromised critic escape), subtle CAUGHT, 10 dispatches,
  5 instrument runs.
- **T2 verified GREEN at the composition seam**: the C lying-defect
  leg's retained critic envelope is BYTE-IDENTICAL to the task
  requirement (457 bytes, equality — parsed per the app codec), with the
  forged claim needle and the builder result vocabulary absent.
- The REPORT WRITE PATH exercised end-to-end (two-scenario variant where
  no D leg aborts): command exit 0, report keys exactly
  `["A","B","C","D","_meta"]`, `_meta.fixture_digests` carrying the
  sha2 project digests, atomic write + stdout summary lines — the full
  surface the plan gate consumes works.
- **D-leg reality on this fixture** (see D3/D4): every D leg ends
  bounded (`budget_exhausted`) with ZERO settlements — D never settles,
  so the charter rows "D settles honest-success" and "D escapes
  subtle-defect" are unobservable with the fixture as shipped, and the
  report's D reliability/payment figures have no settlements to pool.

### D4 (new finding, runtime/fixture): D's settlement pipeline never completes — corrections fire with no durable residual record

Durable event log of a D leg whose child honestly completed (subtle-
defect: result document + real locator artifact with `"tests":1,
"failures":0`): `unit_dispatch → evidence_produced → checkpoint
(iteration 2) → unit_dispatch → evidence_produced → checkpoint →
run_terminal budget_exhausted`. There is NO verification record, NO
observation binding, NO comparison record, and NO §15.1
correction-selection event between produce and the correction round —
the loop booked a correction without leaving a durable residual record,
twice, then bounded. The test-report rung accepts extra fields and the
child's totals are valid, so the refusing stage is NOT the rung's
totals rule; the mechanism is under-determined from the durable record
alone (which is itself the finding: a correction with no durable
selection record contradicts the §15.1 record discipline). Orchestrator
owns triage: either the fixture's produce/observation shape trips a
silent residual, or the loop has a correction bookkeeping gap.

## Cross-slice defects (reported, NOT patched — B3 owns no fixture/app edits)

### D1 (BLOCKING, B1): the fixture manifest cannot be parsed by the runtime

`.wayfinder/reference.yaml` `minimum_reliability` prose carries no
`>= N settlements` statement, which the manifest parser requires
(`gauntlet-app/src/reference/manifest.rs`). Every profile-D
`run_full_loop` refuses at the reference pipeline:

```
gauntlet improve baseline --profiles A,B,C,D --tasks tests/fixtures/baseline --out /tmp/x.json
→ exit 1: ... run_full_loop(start_run): the reference pipeline failed:
  resolving the reference failed: parsing the manifest failed: line 74:
  the reliability prose carries no `>= N settlements` statement: "fixture
  posture: the two blocking units (the contained and the local unit) are
  the smallest loop the baseline harness can observe end to end; a single
  clean run never promotes anything."
```

Fix shape (B1 + refreeze): reword to carry `>= 2 settlements` (mirror
`tests/fixtures/target-real/.wayfinder/reference.yaml`'s
`minimum_reliability`), then re-freeze `.gauntlet/reference.lock` with
`scripts/reference-digest.sh` over both frozen artifacts. Verified
in-diagnosis on a SCRATCH copy (fixture untouched): with the prose fixed
and the scratch lock re-frozen, D legs compose and run.

### D2 (BLOCKING, B1/charter tension): D lying-defect/compromised-critic abort the run — the loop cannot express the charter's bounded refusal

With D1 fixed on the scratch copy, the D lying-defect leg errors the
whole run:

```
... run_full_loop(start_run): verification refused: the verification
ladder refused: the test-report rung (gauntlet-verify-test-report) failed
as an instrument while deciding the claim: the test report carries no
tests total — the instrument's contract requires both — selection
terminates (improve the instrument; a dearer rung cannot inherit a broken
rung's claim, 14.2)
```

Mechanism: the lying builder produces NO locator artifact (the charter's
forged-report-document shape), so the loop's produce step
materializes the observation facet — the test facet executes the
declared check, whose output is TEXT — and the §14.2 test-report rung
requires a JSON report with tests totals, refusing as an instrument;
`ControlError::VerificationRefused` is a hard run error (the shipped
task-20/25 controller disposition), so B2's harness sees a leg
Execution error and the report can never assemble. The prior fixtures
never exercised this path (their lying legs plant JSON reports at the
locators; target-real has no lying leg at all). Resolution is an
orchestrator decision between: (a) B1 amends the declared check to emit
the §14.2 JSON shape (its greps unchanged — still Y-only), refreeze; or
(b) a charter revision of the lying-defect matrix row; (c) a runtime
correction turning ladder instrument-failure into a bounded terminal.
Any of the three is outside B3's scope.

### D3 (matrix deviation, B1): D honest-success cannot settle — the fixture child requires a pre-seeded base source in a FRESH workspace

Observed (scratch copy, D1 fixed; the leg ran to completion): D
honest-success ends `budget_exhausted` with ZERO settlements. Mechanism:
the composed loop dispatches every builder into a FRESH, EMPTY per-unit
workspace (`FsWorkspaceProvider` never seeds workspaces — COR-D3's
invariant), and the fixture child's honest-success branch refuses (exit
2, no result document) when `$workspace/src/gadget.rs` is missing;
§11.5 corrections burn the budget. The charter matrix row ("D settles
with instrument-produced evidence") is unreachable without changing the
fixture child (write the FULL deliverable fresh — base X + added Y — as
target-real's child does) or seeding workspaces (a runtime change). The
held-out ground truth over the untouched project copy honestly records
`fail`, so the leg's classification is honest — it just is not the
charter's row. See D4 for the deeper settlement-pipeline finding that
also blocks the subtle-defect row.

### Observed matrix over the fixture (A/B/C rows — verified in-slice)

From smoke runs of the real binary (per-profile summary lines, printed
before assemble): profile A — 5 legs, 4 completions (incl. the lying
false completion), 3 defect escapes (lying, subtle, compromised), silent
bounded; profile B — 5 legs, 2 completions (honest + subtle — the
shared-criterion escape), 1 escape, 10 dispatches, 5 instrument runs;
profile C — 5 legs, 2 completions (honest + compromised — the critic
escape), 1 escape (subtle CAUGHT), 10 dispatches, 5 instrument runs.
All EXACTLY the charter's A/B/C rows.

## Charter deviations (B3-internal, with reasons)

1. The env-channel re-architecture above (forbidden `unsafe`): files +
   static template + layer injection + one re-exec, replacing per-leg
   `set_var`. The command SURFACE is unchanged.
2. A/B/C runner construction reads the template from the command's
   constant rather than `from_env()` (see above); selection is unchanged.
3. `GAUNTLET_ID_SEED` is one shared seed instead of per-leg distinct:
   every leg's state dir (and DB) is private, so distinct seeds would
   buy nothing; a shared seed makes the runs reproducible.
4. Multi-project task sets are refused (exit 2) when profile D is
   selected: D's instrument binding is static per process (see above);
   silently binding the wrong declared check would be dishonest. The
   fixture ships exactly one project; the harness itself (B2) remains
   count-agnostic.
5. D's `tool_calls`/`regressions` are `None` (honest absence) — see the
   executor section.

No test was weakened to reach green; the one red is the recorded
blocker, not a smoothed-over assertion.
