# B1 progress — the conformance sweep (code + manifest)

Charter: `.agents/reports/task27/charter.md` decisions 1-6, 11-12 bind this
slice. Scope: `gauntlet_app::conformance` + `verify --all` + the 70-unit
manifest + tests. NOT the orchestrator's; the REAL sweep run is the
orchestrator's phase-gate action.

## Checkpoints

- 2026-09-07 (session 1 start). Context read: charter, plan §Task 27,
  AGENTS.md, frozen answer key (70 units: 66 test_report, 1 log [AK-061],
  1 file [AK-067], 1 metric [AK-069], 1 test_report-at-artifact-locator
  [AK-070]), verify_cmd.rs / lib.rs usage / cli_surface golden procedure
  / report_manifest.rs (does NOT enumerate verify args — no change owed),
  reference-candidates.md (all dated locator-divergence entries),
  CURRENT_STATUS settled_summary, baseline module (the executor-seam
  precedent: hand-rolled strict codec + ports JSON + FakeExecutor pattern,
  zero new dependencies), ports json/clock, justfile, task26 charter
  discipline section (compile lock = own top-level flock + sccache stop
  inside the same flock'd command).
- Evidence mapping research done. Real targets verified via
  `cargo metadata` + rg on the tree (NOT guessed): the p*_ / g*_ / e2e_*
  composition-root targets of gauntlet-cli; arch-tests' four verbatim
  targets; app in-crate module test filters for AK-003/004/006/007/015/
  017/018/020/028/032-034/040/049/053; adapter targets for AK-008..012
  (agent-cli contract_suite — hermetic probe children) and AK-059
  (agent-mock scenario_playback; the frozen locator names the fixture
  dir, which EXISTS, but no test target lives there).
- NEW divergences found (no dated reference-candidates entry exists;
  will be reported in b1-report.md for B3 to record — single writer):
  AK-003, AK-004, AK-006, AK-007, AK-008, AK-009, AK-010, AK-011,
  AK-012, AK-015, AK-017, AK-018, AK-020, AK-059 (fixture-dir locator,
  test lives in the adapter), AK-061 (build/cli-only-profile log path
  never materialized; task-21 re-evidence is cli_surface + the mock
  full-loop E2E), AK-064, AK-065 (task18/20 closures name the real
  targets; no dedicated dated entry).
- Design settled (charter-faithful): manifest codec = strict hand-rolled
  subset parser (the baseline_task@v1 precedent; schema token
  `gauntlet.conformance-manifest@v1`); execute vocabulary = closed set
  {validated `cargo nextest run -p <allowlisted pkg> [--lib|--test <t>]
  [filter...]`} ∪ {absolute-path no-argument executables — the fixture-
  world arm for /bin/true /bin/false /bin/echo; no shell anywhere, the
  executor runs arg vectors}; file/metric kinds restricted to
  artifact|none arms; test_report/log may carry execute, artifact
  (AK-070 per decision 11), or none; set-equality both directions +
  kind equality + frozen_locator verbatim equality all refuse.
  Artifact predicates: handoff_six_categories (AK-067 — chosen over
  evidence:none because INCONCLUSIVE(no_artifact) is the honest interim
  state AND the disposition becomes mechanical once B3 lands the file;
  disclosed in b1-report), baselines_keys_abcd (AK-069),
  mutation_record_valid (AK-070, missed==0 + shard arithmetic + pin).
  bounded:true planned for AK-053 (recorded partial §18.3 support),
  AK-066 (recorded settleability tension), AK-069 + AK-070 (recorded
  settlement artifacts per decision 11). Evidence dir derived from the
  report path's parent (default report path reproduces the charter's
  .agents/reports/conformance-evidence/<UTC-stamp>/ exactly; explicit
  --report in tests keeps hermetic).
- Next: RED-first tests (app tests/conformance.rs + cli
  tests/conformance_cli.rs) against a refusing stub, then the mechanism,
  then the 70-unit manifest, then battery (package-scoped first; repo
  compile lock = /tmp/gauntlet-build.lock, own top-level flock each
  time, sccache --stop-server inside the same flock'd command).

## Interruption recovery anchor

- 2026-09-07 session 1: RED stage OBSERVED. App tests
  (crates/gauntlet-app/tests/conformance.rs, 19 tests) written FIRST;
  stub module (typed API real, bodies refusing with a typed
  not-implemented) compiled after two mechanical fixes (Debug derive,
  String compare); `cargo nextest run -p gauntlet-app --test
  conformance` (under the /tmp/gauntlet-build.lock flock) = 6 run / 6
  FAILED behaviorally (wrong variant: stub RulerParse instead of the
  asserted typed refusals/values), 13 blocked by fail-fast — the stub
  is discriminated. GREEN stage began.
- 2026-09-07 session 1, REDESIGN RECORDED (AK-016 constraint, binding):
  `just boundaries`' no_hardcoded_literals scanner (frozen,
  mutation-defended) forbids in EVERY core crate (src AND tests —
  collect_files recurses the whole crate tree): (a) any string literal
  starting with '/' (len>1); (b) any path-shaped multi-segment string
  literal; (c) any literal exactly equal to a tool identity
  ("cargo", "nextest", ...); (d) any multi-token literal headed by a
  command head carrying flags/metacharacters; (e) even COMMENTS naming
  rooted segments (/bin, /tmp — ABSOLUTE_PATH_ROOTS). Consequence: the
  charter's STRING-form `execute: "cargo nextest run ..."` cannot be
  parsed, displayed, executed, OR TESTED from any core-crate file, and
  laundering (concat/split/byte-string) would be gaming a frozen gate —
  refused. DECISION (charter deviation #1, to be argued in b1-report):
  the manifest's execute arm is STRUCTURAL —
  `execute: {package: <allowlisted>, test: <target>}` or
  `{package: <allowlisted>, lib: true, filter: <token>}` or
  `{program: /absolute/path}` (the fixture arm) — preserving every
  normative property decision 2 mandates (closed vocabulary, fail-closed,
  no shell — strengthened to unrepresentable-invalid) with tool identity
  in the adapter layer per spec §8.4. Deviation #2: a small pure
  `sweep_toolchain` module in gauntlet-adapter-verify-command-exit-code
  (outside my declared file scope; additive, zero new deps — that
  crate's header forbids app/cli deps, and the module takes none)
  composes the real argv/display/compile-lock wrapper; the CLI executor
  calls it with variable-only Command::new (no literals near the spawn,
  per the scanner's Command::new rule). Evidence-dir/report defaults in
  the CLI compose from gauntlet_app::config::defaults::report_dir_at +
  single-segment literals (the established pattern). Tests build every
  path from segment arrays joined at runtime (the baseline task.rs
  .join("/") precedent).
- Next: implement commands.rs (structural), manifest.rs codec,
  report.rs assembly, mod.rs run_sweep; rewrite the app tests to the
  structural arms; adapter module; verify_cmd --all + usage line;
  conformance_cli.rs; goldens; the 70-unit manifest; battery.

- 2026-09-07 session 1 (final): GREEN across the slice. Renames forced
  by the adapter-identifier denylist (AK-014 class): manifest flow key
  `test:` -> `target:`; slug fallback word changed off the denied
  segment; WORKSPACE_PACKAGES reduced to the five CORE packages with
  the allowlist made injectable (`parse_manifest_with` +
  SweepInputs.allowed_packages) — the two runner adapters are named ONLY
  in the composition root (`verify_cmd::sweep_packages()`), outside the
  denylist's scope. The committed-manifest tooth moved from the app
  test to the CLI test (it needs the full vocabulary). single_writer's
  WRITE_API_INVENTORY extended with the sweep's three write sites (the
  suite's own documented remedy) + synthetic fixture stub + pinned-list
  entries. Boundaries 245/245; ruler PASS (x2); workspace check +
  surfaces-feature check 0; clippy -D warnings clean on the three
  touched crates; fmt-check clean; app package 646/646; conformance
  suites: app 19/19, cli 9/9 (incl. the tooth), cli_surface 4/4,
  adapter 6/6. Goldens regenerated (both). b1-report.md written next —
  NOTHING mid-flight at session end.

- 2026-09-07 COR-1 (orchestrator's first REAL sweep executed two
  defects): (1) flock argv order fixed — options before the file
  operand (was `flock <lock> -w ...` ⇒ flock executed "-w", exit 69,
  45/45 dead in ~50ms; evidence 20260907T193758Z); (2) flock-level
  {86 busy, 69 could-not-execute} now classify as environmental
  CannotRun ⇒ INCONCLUSIVE in the arm-aware CLI executor, with the
  wrapper remapping an inner command's own 69 to 169 so the reserved
  pair stays unambiguous; (3) the missing behavioral tooth added —
  three tests EXECUTING the real flock over the real wrapper (true→0,
  false→1, inner-69→169, held-lock -w 1→86); the old shape test had
  enshrined the defect verbatim. COR-1 battery: adapter 9/9, app+cli+
  cli_surface 32/32, arch-tests 245/245, check/clippy/fmt 0, ruler
  PASS. Real sweep NOT re-run (orchestrator's action). Nothing
  mid-flight.

- 2026-09-07 COR-2 (sweep-2: AK-062 FAIL exit 4 — the whole target is
  surfaces-gated; the vocabulary lacked a features dimension): edges
  discovered from the allowlist's Cargo.tomls ({composition, surfaces};
  adapters none; default excluded); grep sweep of all 45 mapped files
  found only AK-062 file-gated (cli_surface uses inner cfgs, correct as
  mapped). Implemented: features flow-sequence on the execute arm
  (fail-closed validation, flat union — cross-package misuse falls to
  cargo's own loud FAIL, recorded), SweepCommand carries features
  (grouping includes it), WORKSPACE_FEATURES pinned to the workspace by
  a test, sweep_features() at the composition root, adapter composes
  --features after -p and before the target (empirically verified:
  bogus-edge metadata failure names the feature; nextest LIST under
  --features surfaces lists the gated target's tests), AK-062 entry
  updated. RED probe observed first (unknown execute key features).
  Battery: app 22/22, adapter 10/10, cli 13/13, arch 245/245,
  check/clippy/fmt 0, ruler PASS. Real sweep NOT re-run. Nothing
  mid-flight.

- 2026-09-07 COR-3 (falsifier r1 F1/F2/F10/F3/F7/F13): duplicate-id
  refusal (multiset equality), exact kind/arm matrix, signal-kill ⇒
  CannotRun naming the signal (ExitStatusExt, no fabricated -1, no log
  for an incomplete command), handoff predicate demands ## headings,
  F7/F13 disclosures. All four behavioral REDs observed pre-fix
  (incl. the fabricated exit_code: -1 in the F10 panic).
- 2026-09-07 COR-4 (falsifier r2 R2-F1/F2/F3/F4/F6): inner-86 remap →
  186 (half-built remap fixed; classify_exit pure table; FAIL reasons
  name the remap + indistinguishability); inner signal band 129..159 →
  200+signal (NOT the r2-specified 128..191 — my own pass-through tooth
  EXECUTED 169→241 under the wider band, so it was narrowed to the
  standard signals 1..31; RT-signal residual disclosed, stays FAIL);
  daemon stop bounded `timeout 10 ... || true` inside the lock
  (adapter test-phase 269s → 10.4s); bounded-artifact exception
  test_report-only (RED-first); F6 mangled string fixed.
- Final battery: app 27/27, adapter 14/14 (10.4s test-phase), cli lib
  17/17, cli suites 13/13, arch 261/261, check/clippy/fmt 0, ruler
  PASS. Real sweep NOT re-run. Nothing mid-flight.
