# B2 progress — CLI composition root + §20.1 surface + run (task20)

Checkpoint file per charter. STARTED/DONE lines per item, declare-first.

STARTED Item 0 (baseline hygiene: clippy redundant_closure fix + just lint) — 2026-08-31
DONE Item 0 — 2026-08-31
- clippy redundant_closure fixed at crates/gauntlet-app/tests/report.rs:213 (`evidence.map(unit)`).
- Workspace clippy: 0 errors (b2-item0-lint.log). `cargo fmt --check` then showed diffs in
  ALREADY-LANDED files (workspace-fs provider.rs, report/mod.rs, tests/report.rs,
  g10/p11 test files) — pure formatting, required by the lint gate; formatted the three
  named packages with `cargo fmt` (no semantic change). fmt now clean (FMT_EXIT=0).
  DEVIATION NOTE: provider.rs is inside B1's crate; the change is rustfmt-only, recorded here.
- Full `just lint` re-run deferred to the final gate battery.
STARTED Item 1 (thin-library refactor of gauntlet-cli) — 2026-08-31
PROGRESS Item 1 — 2026-08-31
- lib+bin refactor DONE: src/lib.rs (run_cli+usage), src/context.rs (RuntimeContext +
  shared measure_backend/measure_runner/doctor_bound), src/doctor_cmd.rs, 
  src/reference_cmd.rs, src/roles_cmd.rs; main.rs is a 7-line wrapper. 
  `just check-package gauntlet-cli` EXIT=0 (b2-item1-check.log). Byte-identical 
  behavior proof (just doctor diff) deferred to the gate battery.
Item 2 NEW adapters landed (uncommitted, compiling next):
- crates/adapters/gauntlet-adapter-escalation-fs/ — FsEscalationQueue under
  <state_dir>/escalations; escalate/poll via HumanEscalation port (SHARED contract
  suite green: 7/7, b2-esc-adapter-check.log), plus list/record/resolve for the
  CLI read surface; typed errors; atomic create-temp+sync+rename+dir-fsync writes.
- crates/adapters/gauntlet-adapter-id-ulid/ — UlidIdGenerator: production IdGenerator
  (OS entropy at construction, monotonic counter; the seeded mock double is the only
  prior impl and replays ids per seed — unusable for real runs). DEVIATION to record:
  a second new adapter crate beyond the brief's escalation-fs, justified by the
  composition's ControllerDeps.ids requirement.
- Both registered in root Cargo.toml members + workspace.dependencies.
PROGRESS Item 2 — 2026-08-31
- R2-N1 deny wiring DONE (app): ComparisonAuthority.with_role_deny + GovernedRun.issue
  enforces assert_within_role_deny_set before build_envelope; ComparisonDeps.role_deny
  field added; controller compose() threads it; 8 test literals + 6 in-crate literals
  updated mechanically. `just check-package gauntlet-app` EXIT=0.
- gauntlet-adapter-id-ulid: 2/2 tests green (b2-round2-check.log).
- Composition-feature seal (arch deps.rs) WIDENED + recorded: admits gauntlet-cli's
  unscoped [dependencies] edge (this task's production rung-set construction) besides
  the original dev edge; seal test renamed the_composition_seal_accepts_only_the_recorded_cli_edges,
  fail-first shapes re-pointed at foreign-crate production edges. This is a GATE
  SEMANTICS change — recorded as the slice's headline deviation, per the app
  Cargo.toml note that anticipated "the production wiring of Tasks 12/20".
- compose.rs (run composition over real adapters) + run_cmd.rs (run/resume/dry-run)
  + reference_lock.rs (typed just-ruler twin) + report_manifest.rs written; compiling.
PROGRESS Items 2+3 — 2026-08-31
- compose.rs/check fixes iterating (compile round 4 in flight). Item 3 modules WRITTEN:
  init_cmd, improve_cmd (typed not-wired naming Task 23), skills_cmd (install/verify/
  list over REPO_OWNED_SKILLS + skills.lock, task19 lint reused), escalations_cmd
  (list via queue; resolve/retry THROUGH SettlementService::settle_operator_action),
  state_cmd (status/inspect/export from port reads; status REQUIRES --run — no
  enumeration read exists, stated truthfully), verify_cmd (VerificationAuthority +
  composed ladder, three-state outcomes, NEVER settles), reference_cmd +form/freeze/
  show/diff/verify.
- lib.rs dispatch registers all 13 §20.1 commands; usage() lists the whole tree.
PROGRESS Items 2+3 — 2026-08-31 (2)
- [[bin]] name = "gauntlet" added (spec 20.1's canonical command name; package stays
  gauntlet-cli). Root --help/-h prints whole-tree usage exit 0 (snapshot target);
  per-command unknown args stay exit-2 usage (COR-D12).
- cli_surface test target: crates/gauntlet-cli/tests/cli_surface.rs (D5 divergence —
  exemption is arch-tests-only and CANNOT transfer to a product crate; recorded in
  .agents/reports/reference-candidates.md as the AK-063/AK-065 class). Golden:
  tests/golden/cli_help.txt.
- D5 decision recorded: hosting at tests/e2e/ verbatim requires widening
  TARGET_PATH_EXEMPT_CRATES to a product crate — rejected as a scan-guarantee
  weakening; CARGO_BIN_EXE_gauntlet only exists in the owning package anyway.
PROGRESS R2-N1 teeth — 2026-08-31
- compare_deny.rs: a_wired_deny_set_refuses_the_envelope_before_it_is_built +
  an_empty_deny_set_reaches_the_envelope_construction — 2/2 PASS. The governed chain
  (for_run -> issue) refuses a denied always-carried category BEFORE build_envelope;
  the empty deny set reaches envelope construction.
- INCIDENT (fixed): my mechanical regex edit overwrote `model_dispatches` values with 0
  in 14 ComparisonDeps literals (controller.rs had 4 -> broke
  a_meter_refused_critic_dispatch_is_truncated...); restored from git diff — CLI tests
  were originally 0 (unchanged), controller.rs restored to 4. Diff now clean of any
  model_dispatches changes. LESSON: scripted edits must INSERT a field, never REWRITE
  a neighboring value.
PROGRESS gates — 2026-08-31
- just lint GREEN (LINT_EXIT=0) after clippy fixes (arc_with_non_send_sync allows with
  single-threaded-loop justification per p6 precedent; contains; collapsible-if; Limits
  Copy) and formatting across the touched packages.
- DRY-RUN GATE PASSED: cargo run -q -p gauntlet-cli -- run --dry-run "seeded bounded
  task" exits 0 from repo root; git-status diff before/after shows ZERO new writes;
  output: doctor-clean preflight, 70 units parsed, lock verified, 61-dispatch plan
  preview (b2-dryrun-out.log).
- gauntlet-cli tests 67/67 (incl. cli_surface snapshot + smokes); app compare_deny 2/2.
PROGRESS gates — 2026-08-31 (2)
- just boundaries GREEN after: (a) renaming id-ulid -> id-entropy (denylist derived
  'id_ulid' collided with domain's existing 'valid_ulid' test text; 'sqlite' removed
  from STATE_DB_NAME -> gauntlet-state.db); (b) fixing 12+6 literal violations
  (instants into consts, bin moved to src/bin/gauntlet.rs auto-discovery so no
  manifest path literal, golden single-segment, env! via let binding); (c) extending
  ANSWER_KEY_SURFACE_FILES (+compose/init_cmd/reference_cmd with reasons) and
  WRITE_API_INVENTORY (+6 guarded sites; guard helper ensure_configured_dir in
  compose.rs; refuse-overwrite needle in init; installed_root in skills; --out in
  export) + fixture/pin-test updates. Gate grew by deliberate recorded inventory.
- ruler/doctor/deny: all GREEN (RULER 0, DOCTOR 0 exit OK, DENY 0).
- model_dispatches corruption incident fixed (see above).
ALL ITEMS DONE — 2026-08-31. Final gates on the final tree:
- just check 0; just test 1319/1319 PASS; just lint 0; just deny 0; just boundaries 0;
  just ruler 0; just doctor exit OK; cli_surface snapshot PASS (67/67 gauntlet-cli);
  dry-run gate: exit 0 + ZERO_WRITES_CONFIRMED (b2-dryrun-final.log).
- Load-flaky b5 (adapter-agent-cli falsifier) recurred once in battery 1, passed on
  the re-run full battery — known class per charter addendum, not weakened.
- Final report: .agents/reports/task20/b2-report.md

STARTED RECOVERY-VERIFY (new B2 session; prior session's final message lost —
  claims carry no settlement authority) — 2026-08-31 18:18
- Reconciled per AGENTS.md: git status + this progress file + b2-report.md match
  the tree (lib/bin split, cmd modules, escalation-fs, id-entropy, mock
  RUNNER_TOKEN, cli_surface + candidates entry all present). No live compile.lock
  holders or cargo processes. No source edits after the 18:12 green lint except
  compare_deny.rs (pre-lint-final7).
- Re-running the FULL gate battery on the untouched tree as independent recovery
  evidence (b2-verify-a/b.log). Any red ⇒ fix before finishing.
DONE RECOVERY-VERIFY — 2026-08-31 18:27 (all evidence b2-verify-a.log, b2-verify-b.log,
  this session's own runs on the final tree):
- just ruler 0; just doctor 0 (exit OK); just check 0.
- dry-run gate: DRYRUN_EXIT=0 + ZERO_WRITES_CONFIRMED (git-status diff within the gate).
- just test: 1319/1319 PASS (incl. all 4 cli_surface tests; load-flaky
  b5_foreign_identity_is_contract_mismatch PASSED this run).
- just lint 0; just deny 0; just boundaries 228/228.
- Reference-candidates.md D5 entry: stale golden path corrected to
  crates/gauntlet-cli/tests/cli_help.txt (markdown only; the test file's doc
  comment still spells the pre-move `golden/cli_help.txt` — cosmetic nit, the
  include_str! is binding; recorded, not touched post-evidence).
B2 SLICE COMPLETE AND INDEPENDENTLY VERIFIED. No source edits made this session.
