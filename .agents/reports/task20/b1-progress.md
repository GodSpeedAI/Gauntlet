# B1 progress — task20 (containment & durability seams)

Builder B1. Protocol: STARTED before each item, DONE after, with evidence.
This file is the contract if the final message is lost.

STARTED item-1 (production WorkspaceProvider adapter, R-1b part a) at 2026-08-31

DONE item-1 at 2026-08-31.
- Files: crates/adapters/gauntlet-adapter-workspace-fs/{Cargo.toml, src/lib.rs,
  src/provider.rs, tests/contract_suite.rs}; registered in root Cargo.toml
  (members + workspace.dependencies).
- Implementation: FsWorkspaceProvider::open(root); create() derives a
  sanitized single-component key `run.subject.seq`, checks containment
  lexically (contained_join discipline), creates the dir, re-checks
  containment on the CANONICALIZED path (§17.5.2 symlink escape checked),
  typed WorkspaceProviderError throughout, no unwrap/expect in production
  paths; cleanup() treats the ref as untrusted input (key-class validation,
  idempotent over NotFound); hooks() returns empty (no config surface exists
  — documented in-code).
- Evidence:
  - `flock .agents/reports/task20/compile.lock -c 'just check-package
    gauntlet-adapter-workspace-fs'` → Finished, 0 errors.
  - `flock .agents/reports/task20/compile.lock -c 'just test-package
    gauntlet-adapter-workspace-fs'` → `6 tests run: 6 passed, 0 skipped`
    (nextest): the shared `workspace_provider_suite` green
    (create_isolates_parallel_subjects, cleanup_releases_the_workspace) +
    in-crate tests (key sanitation, single-component keys, cleanup ref
    refusal for refs outside the key class, cleanup idempotence, symlink
    escape typed refusal).
  - Two intermediate failures were TEST-side and fixed (factory root/build
    root mismatch; a miscounted expected string in the sanitize test).
    Implementation unchanged by those fixes.

STARTED item-2 (fresh per-unit workspace through gate admission; cleanup) at
2026-08-31

DONE item-2 core wiring at 2026-08-31 (teeth red-run recorded below).
- gate.rs: GateFacts gains `workspace: WorkspaceRef` (per-dispatch fact);
  GovernDeps loses its lifetime `workspace` field (no longer the operative
  source of truth — doc comment records why); `none_backend()` loses its
  param; `none_workspace()` removed; `with_backend()` loses its workspace
  param; admit() anchors conjunct-6 policy resolution at
  `facts.workspace`. New test `the_gate_anchors_policy_resolution_at_the_dispatch_workspace`
  (StaticPolicies now LEDGERS the workspaces it is asked for; assertion
  requires the two facts' distinct anchors, in order).
- controller.rs: ControllerDeps gains `pub workspaces:
  Option<Arc<dyn WorkspaceProvider>>`; dispatch() allocates a FRESH
  workspace from the provider BEFORE admission (typed
  ControlError::Workspace on refusal — never a silent fallback when a
  provider is wired; provider-less composition keeps the run-derived ref),
  passes it via GateFacts AND the RoleInvocationSpec; the observation
  request targets the flight's OWN spec workspace; ControlError gains the
  Workspace arm + From impl + Display.
- Cleanup wired: `release_flight` (reservation + workspace) at ALL 11
  invocation-terminalization sites (reread cancel, drain Cancelled,
  bound_the_drain, dispose_failure x3, dispose_transient, dispose_timeout
  x2, charge-refused, and the drain's Completed arm releases AFTER
  process_completion returns — §16 ordering: cleanup can neither precede
  nor preempt settlement); `release_workspace` is best-effort, records a
  `workspace_cleanup_failed` pinned record on failure, never errors the run.
- Mechanical constructor updates: controller.rs test fixtures (compose_full
  gained a `workspaces` param, all 7 call sites updated; hand-built deps
  literals + `none_backend(...)` call sites updated), 7
  crates/gauntlet-cli/tests files (`workspaces: None` in every
  ControllerDeps literal; `none_workspace()` → `none_backend()`).
- NEW tests: `two_units_of_one_run_are_admitted_into_distinct_workspaces`
  (TEETH, in controller.rs tests) +
  `without_a_provider_the_dispatch_falls_back_to_the_run_derived_ref`.

STARTED item-2-teeth (red→green proof) at 2026-08-31 — resume: GREEN run first

TEETH PROOF (completed 2026-08-31 by B1-finisher; logs referenced below):

1. GREEN (wired mechanism, tree as landed) —
   `flock .agents/reports/task20/compile.lock -c 'cargo nextest run -p
   gauntlet-app two_units_of_one_run_are_admitted_into_distinct_workspaces
   without_a_provider_the_dispatch_falls_back_to_the_run_derived_ref'`
   → finish-teeth-green.log:
   `2 tests run: 2 passed, 456 skipped` EXIT=0
   `PASS gauntlet-app control::controller::tests::two_units_of_one_run_
   are_admitted_into_distinct_workspaces`

2. RED (minimal bypass: the controller's dispatch match made both arms
   return the run-derived ref, provider never consulted — the doc-comment's
   "old single-lifetime path restored") —
   `cargo nextest run -p gauntlet-app two_units_of_one_run_are_admitted_
   into_distinct_workspaces` → finish-teeth-red.log:
   `1 test run: 0 passed, 1 failed` EXIT=100
   `assertion left == right failed: two units of one run must hold
   DISTINCT workspace refs at admission, got {"01ARZ3NDEKTSV4RRFFQ69G5FAV"}
   — a shared ref is the R-1b defect this tooth exists to refuse`
   (panicked at controller.rs:7841 — exactly the distinctness assertion
   the test doc-comment predicts).

3. RESTORE byte-identical: controller.rs copied back from /tmp backup;
   sha256 before bypass AND after restore =
   e93a67a753c7c866600779da081c8cfa5dc728adc3e4bba0fe5d418e73e4f6a8.
   Tree NEVER left in the reverted state beyond the single red run.

4. GREEN again (post-restore) — finish-teeth-green2.log:
   `Finished test profile ... in 2m 22s`
   `2 tests run: 2 passed, 456 skipped` EXIT=0
   `PASS gauntlet-app control::controller::tests::two_units_of_one_run_
   are_admitted_into_distinct_workspaces` and `PASS
   ...without_a_provider_the_dispatch_falls_back_to_the_run_derived_ref`

DONE item-2-teeth at 2026-08-31. GREEN→RED→GREEN proven; restore verified
by sha256; tree never left reverted.

STARTED gate battery (check-package → test-package → check → test → lint →
boundaries → ruler, in order, each under the compile lock) at 2026-08-31

BATTERY RESULT (log: finish-gate-battery.log):
- EXIT1 check-package gauntlet-adapter-workspace-fs = 0
- EXIT2 test-package gauntlet-adapter-workspace-fs = 0 (contract suite green)
- EXIT3 just check = 0
- EXIT4 just test = 100 — RED: arch literal gate flags '/tmp' literals in
  B1's OWN teeth fixture (controller.rs:7777-7778)
- EXIT5 just lint = 101 — RED: clippy needless_return at controller.rs:1120
  (B1's release_workspace tail)
- EXIT6 just boundaries = 100 — RED: same literal gate + single-writer gate
  on install.rs (B1's item-3 tooth used #[cfg(test)] shapes the blanker
  refuses to guess at: a static + a statement; I8 fail-closed)
- EXIT7 just ruler = 0
ALL THREE REDS ARE B1-SCOPE (caused by B1's landed code). No out-of-scope
red observed in this battery.

STARTED battery-red fixes (all inside B1 files; no gate/assertion edits) at
2026-08-31:
- fix-A: controller.rs teeth fixture root: std::env::temp_dir() instead of
  the hardcoded '/tmp/ws-fixture-{n}' literal (refs/assertions unchanged).
- fix-B: drop the needless tail `return;` in release_workspace (control
  flow identical).
- fix-C: install.rs tooth seam reshaped into cfg-disjoint twin MODULES
  (production no-op / test counting sensor) — the blanker classifies gated
  modules and functions only; the shared gate file stays byte-identical.

PRE-CHECK after fixes A/B/C (finish-fix-precheck.log + /tmp/b1-*-full.log):
- clippy -p gauntlet-app --all-targets: B1's needless_return is FIXED
  (lib compiles), but the NEW failure is in the `report` TEST TARGET
  (crates/gauntlet-app/tests/report.rs — B2's untracked in-flight file):
  clippy::redundant_closure, -D warnings. OUT OF B1 SCOPE — reported to
  the orchestrator, NOT fixed by B1-finisher.
- cargo fmt -p gauntlet-app --check: B1's controller.rs has an import
  placement diff (teeth tests' `use super::run_workspace_ref;`) — B1-scope,
  fixing via targeted rustfmt on B1's two files only (NOT the whole
  package, to avoid reformatting B2's in-flight files).

BATTERY-2 after fixes A/B/C + targeted rustfmt (finish-battery2.log):
- EXIT0 targeted (teeth x2 + write_atomic tooth) = 0 (3 passed)
- EXIT3 just check = 0
- EXIT4 just test = 100 — literal gate FIXED (0 literal violations).
  Remaining: (a) single-writer flags B1's `OpenOptions` construction in
  install.rs:1177; (b) gauntlet-adapter-agent-cli
  `b1_contract_wins_over_exit7_and_process_is_never_consulted` FAILED once
  (expected Completed, got MalformedResult) — NOT a B1 file, PASSED in
  battery-1's ordering, PASSED in isolation (finish-iso-b1test.log,
  ISO_EXIT=0): load-flaky live-process attack test, reported, NOT fixed
  (outside B1 scope).
- EXIT5 just lint = 101 — B1's files clean; red is B2's tests/report.rs
  (clippy redundant_closure) + B2's report/mod.rs fmt diffs. Out of scope.
- EXIT6 just boundaries = 100 — only the single-writer OpenOptions item.
- EXIT7 just ruler = 0

FIX-D (single-writer): write_atomic's directory fsync switched from
`OpenOptions::new().read(true).open(parent)` to `std::fs::File::open(parent)`
— semantically the SAME read-only open (the gate's own documented blessed
spelling: "read-only opens have File::open"), consistent with the file-fsync
half three lines above. Shared gate files stay byte-untouched.

STARTED battery-3 (boundaries + test + lint + ruler) at 2026-08-31

(battery-3 aborted mid-run: the single-writer gate matched the token
'OpenOptions' in MY OWN fix-D comment — the scanner scans comments and
strings too. Comment rephrased without the token; stale run stopped.)

BATTERY-4 FINAL (finish-battery4.log, finish-lint4-full.log) at 2026-08-31:
- just boundaries = 0  (arch-tests 228/228 — literal gate + single-writer
  gate both green on B1's code)
- just test = 0  (workspace: `1303 tests run: 1303 passed, 0 skipped` —
  includes the previously flaky adapter falsifier test, confirming the
  single battery-2 failure was load flakiness, and all CLI/gate/teeth/
  tooth tests)
- just lint = 101 — the ONLY error is clippy redundant_closure at
  crates/gauntlet-app/tests/report.rs:213, B2's untracked IN-FLIGHT file;
  zero lint or fmt findings in any B1 file (controller.rs, gate.rs,
  install.rs, adapter crate — each verified rustfmt-clean individually).
  OUT OF B1 SCOPE — reported, not fixed.
- just ruler = 0
- (check-package / test-package gauntlet-adapter-workspace-fs and
  just check were green in batteries 1-2 and the touched crates are
  covered by battery-4's workspace runs; no regression possible on the
  untouched adapter crate.)

DONE gate battery at 2026-08-31. All B1-scope gates green; the only red
anywhere is B2's in-flight report slice (clippy), which B1-finisher did
not touch.

ENVIRONMENT NOTE for the orchestrator: a sccache server forked from
inside a `flock compile.lock` session inherits the locked fd and keeps
the lock held after the flock wrapper exits (observed twice today; fixed
both times by `sccache --stop-server`). Consider a failure-log entry.

DONE item-2-teeth (final): GREEN→RED→GREEN proof recorded above; teeth +
fallback + write_atomic tooth all green in battery-4's workspace run.

DONE finish-fix (remaining compile error, p5_hard_gate.rs:701
ControllerDeps literal) at 2026-08-31 — by B1-finisher.
- `workspaces: None,` inserted after the `govern,` field in the
  ControllerDeps literal at p5_hard_gate.rs:701 (mirrors the placement in
  g7/p4/p6/p7/p11/g10 and the file's own second literal at :1375). No
  assertion touched.
- Evidence: `flock .agents/reports/task20/compile.lock -c 'just
  check-package gauntlet-cli'` → `Finished dev profile ... in 2.75s`,
  EXIT=0 (log: finish-check-cli.log).

---

SECOND FINISHER PASS (2026-08-31 ~14:42–15:05). Dispatched as "B1 finisher"
under a stale belief the first finisher had died; found its processes still
running battery-4. Stood off all source edits until its battery shell
exited (B4_DONE=0), then resumed at the genuinely missing items. Coordinator
alerted mid-flight. Prior pass's report and logs verified, not redone.

TOOTH-2 PROOF — install.rs `write_atomic_fsyncs_the_directory_entry`
(the dir-entry fsync mutation tooth; the prior pass had GREEN-only evidence):

1. GREEN (tree as landed) — finish-tooth2-green1.log:
   `1 test run: 1 passed, 457 skipped`, GREEN1_EXIT=0.
2. BYPASS (minimal, temporary): the parent-directory fsync half
   (`File::open(parent)` + `sync_all`) AND its `record()` call removed —
   the pre-R17-note-1 commit path the tooth exists to refuse. Edit-reversed
   afterwards.
3. RED — finish-tooth2-red.log: `1 test run: 0 passed, 1 failed`,
   RED_EXIT=100, panicked at install.rs:1241: `assertion left == right
   failed: exactly one directory-entry fsync per committed write …`
   left: 0, right: 1 — exactly the predicted sensor observation.
4. RESTORE byte-identical: sha256 before bypass AND after restore =
   c0002ef0e6a3dc9a91bc1ebafd721a25ce96fc5925629272f2d429c3c6496fd9
   (plus `diff -q` against the /tmp backup: identical). Tree never left
   reverted beyond the single red run.
5. GREEN again — finish-tooth2-green2.log: `1 test run: 1 passed,
   457 skipped`, GREEN2_EXIT=0.

DONE tooth-2 three-state proof.

BATTERY-5 (re-verification on the FINAL tree — the prior pass had reasoned
check-package/test-package/check green "no regression possible" without
re-running them after its last edits): finish-battery5-rest.log —
- `just check-package gauntlet-adapter-workspace-fs` = 0
- `just test-package gauntlet-adapter-workspace-fs` = 0 (6/6)
- `just check` = 0 (Finished dev profile in 2m 20s)
(`just test` / `just boundaries` / `just lint` / `just ruler` results from
battery-4 carry over: install.rs restored byte-identically, controller.rs
untouched this pass.)

DONE gate battery — every battery item green on the final tree except
`just lint`, whose sole error on the current tree is B2a's in-flight
tests/report.rs:213 (clippy redundant_closure). Out of B1 scope; reported,
not fixed.
