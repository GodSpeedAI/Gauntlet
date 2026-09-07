# B3 progress — task 26 (CLI wiring + gate test)

Checkpoint file, updated as the slice goes. Completion contract lands in
`b3-report.md`.

## Exploration settled (2026-09-06)

- B2's public API read in full (`baseline/{harness,metrics,report,
  profiles,task}.rs`); the executor seam, request builders, report
  assembly, and assemble()'s refuse-missing rule are as documented in
  b2-report.md.
- Child-protocol mechanics confirmed: `ProcessChildTransport` inherits
  the PARENT environment verbatim + `GAUNTLET_RESULT_PATH` (RES-D3), so
  per-leg `GAUNTLET_BASELINE_MODE` / `GAUNTLET_CLI_AGENT_CMD` must be in
  the CLI process env per leg (edition-2024 `unsafe set_var`, documented,
  single-threaded command loop).
- `CliAgentRunner` anchors each dispatch's execution at
  `<state_dir>/workspaces/<workspace-ref>` (single-component key class)
  and the `{workspace}` template placeholder carries THAT path to the
  child. B3's scratch layout exploits this honestly: per leg
  `<legs-root>/<task>.<scenario>/world/workspaces/<scenario>` IS the
  copied task project, with `GAUNTLET_STATE_DIR =
  <legs-root>/<task>.<scenario>/world` — so A/B/C children run in the
  real project copy (base source + declared check present) while nothing
  ever writes into `tests/fixtures/`.
- Retained invocation dirs (`runs/<run>/inv/<inv>/*.envelope.json` +
  `*.prompt.txt`, RES-D12) are the composition-half seam for the T2
  blind-envelope assertion from a black-box test; the D leg's
  `gauntlet-state.db` is the durable seam for T3.
- Runner poll pins `TimedOut` itself and stops the child via the
  transport's group stop; `cancel` after the pin is the typed no-op
  (`AlreadyTerminal`) — belt-and-suspenders call kept, documented.
- Reserve-intact: domain `Budget::forward_ceiling` /
  `would_breach_reserve` over the durable checkpoint's `budget_spent`,
  against the leg's OWN effective config.
- report_manifest.rs reconciled: rows are per top-level command;
  `improve` is already wired — no row change needed or made (RES-15).
- Expected finding (to be verified by the run): profile D's honest-
  success leg dispatches the fixture child into a FRESH, EMPTY per-unit
  workspace (the production loop's FsWorkspaceProvider never seeds
  workspaces), and B1's child refuses (exit 2, no result document) when
  `src/gadget.rs` is missing — so D honest-success is predicted NOT to
  settle, deviating from the charter matrix row. Unfixable at the
  composition root without breaking the §17.5.1
  instrument-examines-the-dispatched-workspace invariant. Will be
  asserted as the charter says (RED = the finding) and reported.

## Findings so far (2026-09-06)

1. **BLOCKING B1 fixture defect — profile D cannot run against the
   pristine fixture.** The fixture's `.wayfinder/reference.yaml`
   `minimum_reliability` prose carries no `>= N settlements` statement,
   which the app manifest parser requires
   (`gauntlet-app/src/reference/manifest.rs`; error: "line 74: the
   reliability prose carries no `>= N settlements` statement"). Every
   `run_full_loop` refuses at the reference pipeline, so NO report can be
   assembled at all (B2's `assemble` requires D). Exact repro:
   `gauntlet improve baseline --profiles A,B,C,D --tasks
   tests/fixtures/baseline --out <tmp>` → exit 1 with that error after
   the A/B/C legs complete. Fix shape (B1 + lock refreeze, NOT done
   here): reword the prose to carry `>= 2 settlements` (mirroring
   `tests/fixtures/target-real/.wayfinder/reference.yaml`'s
   `minimum_reliability`), then re-freeze `.gauntlet/reference.lock` with
   `scripts/reference-digest.sh` over the two frozen artifacts. The gate
   test (`the_plan_gate_shape_writes_the_charter_report_and_the_matrix_holds`)
   is deliberately RED with exactly this error until the fixture is
   fixed — a red relevant test is a stopped world, not a nuance.
2. **Design fact this workspace imposes:** `unsafe_code = "forbid"`
   (workspace lints) makes edition-2024 `std::env::set_var` unusable, so
   the per-leg scenario cannot travel through the process environment as
   the brief sketched. Channels used instead (documented in
   improve_cmd.rs): (a) ONE static shell-wrapper child template that
   reads the scenario + child path from per-leg FILES
   (`<leg>/baseline-mode`, `<leg>/baseline-child`) via
   `$2/../../../`; (b) a one-time safe re-exec
   (`Command::env`, `current_exe`) establishing the three STATIC env
   reads the composed loop makes (`GAUNTLET_CLI_AGENT_CMD`,
   `GAUNTLET_TOOL_TEST`, `GAUNTLET_ID_SEED`); (c) per-leg state/artifact
   roots + D control settings injected INTO the config `EnvLayer` (last
   occurrence wins), never into the process env.
3. Early template bug (fixed in slice): the wrapper's `$2/../../`
   resolved to `<leg>/world`, not `<leg>` — three levels up is correct.
4. Profile A's observed row over the fixture (smoke run): 5 legs,
   4 completions (incl. the lying false completion), 3 escapes (lying,
   subtle, compromised), silent bounded — EXACTLY the charter's A row.

## Complete (2026-09-06)

Slice landed; see `b3-report.md` (the completion contract). Battery:
check-package clean; test-package 139 run / 138 passed / 1 expected red
(the gate test, blocked by fixture defect D1). T2 verified green at the
composition seam by hand (byte-identical requirement envelope); the
report write path exercised end-to-end on a scratch diagnostic task set
(exit 0, exact keys, _meta digests). Temp diagnostic trees removed.
