# Task 23 — COR-1 CONTINUATION brief (fresh builder; predecessor died)

You are the COR-1 continuation builder. The predecessor landed C1
(verified) and C3 (code + RED test in tree, verification run was
interrupted), then died mid-C3 with a self-deadlocked compile command
(operational note below). Your job: verify the landed work, then finish
the remaining items of `.agents/reports/task23/cor1-brief.md` exactly.

READ FIRST, in full:
1. `.agents/reports/task23/cor1-brief.md` — the work order (C1–C11 +
   records-not-code).
2. `.agents/reports/task23/IN-FLIGHT.md` — esp. the 2026-09-04 C2
   DECISION entry (legs 2/3 promote at the gate level over REAL
   StateStore earned authorities, p10 composition pattern; leg 1 stays
   on the REAL composed loop; no C1 relaxation; the loop-economics
   finding gets recorded in reference-candidates + your report).
3. `.agents/reports/task23/critic-design.md` — the findings being
   corrected (B1 blocking, M1–M4, m1–m6, i1–i6).
4. `AGENTS.md`; `.agents/reports/task23/charter.md` (what must not
   regress).

STATE ON THE TREE (reconcile, do not redo):
- **C1 DONE** (promote/gate.rs + metrics.rs + tests.rs; sentinel
  deleted; underivable arms ⇒ EvaluationIncomplete gaps; 4 RED-first
  regression tests; 34/34 green at the time). Verify, don't redo.
- **C3 LANDED, verification owed**: `facts_agree` no longer compares
  `admitted_at` (crates/gauntlet-ports/src/revision_ledger.rs:174) and
  the stepping-clock test exists
  (crates/gauntlet-app/src/promote/tests.rs:874). Run its tests green
  (`cargo nextest run -p gauntlet-app -E "test(admission_is_idempotent)
  or test(a_disagreeing_re_admission)"` + the ports suite), fix
  whatever the interruption left half-done, then mark C3 done.
- **C2 partial**: the predecessor was continuing fixture-side work per
  the decision. Reconcile
  `crates/gauntlet-cli/tests/p9_self_improvement_containment.rs`
  against the C2 decision before continuing it.
- **C4–C11 + records-not-code**: NOT STARTED.

OPERATIONAL DISCIPLINE — THE PREDECESSOR DIED OF THIS, BINDING ON YOU:
- ONE compiler at a time, and **NEVER nest a `flock` inside another
  `flock`'s -c string** — the predecessor wrapped a compound containing
  a second flock in one outer flock and self-deadlocked the repository
  lock. Every compile/test call is its OWN top-level command:
  `flock /home/sprime01/projects/gauntlet/.agents/reports/task23/compile.lock -c 'cargo nextest run … 2>&1 | tail -10; sccache --stop-server'`
  (the `sccache --stop-server` INSIDE the same flock'd command, AFTER
  the build call). Before editing, check for lingering cargo/flock
  processes (`pgrep -fa "cargo|flock|nextest"`).
- Run long builds with run_in_background and poll every 30–60 s (you
  die on a 10-minute inactivity timeout — keep tool-call cadence high;
  never one synchronous call over ~3 minutes). WSL2 ~1.6 GiB free —
  never raise parallelism.
- **Checkpoint cadence is mandatory**: append STARTED/DONE lines with
  sha256 of edited files to `.agents/reports/task23/cor1-progress.md`
  AFTER EVERY ITEM (append a CONTINUATION header noting you are the
  second builder). The predecessor's last checkpoint was C1 — do not
  let work land uncheckpointed.
- Finish with `.agents/reports/task23/cor1-report.md`: per-item RED →
  fix → GREEN with real numbers (mark C1 as predecessor's, C3 as
  landed-and-verified), the C2 deviation + loop-economics finding
  section, the records-not-code entries (i1–i4 etc.), and the full
  acceptance battery results.
- Every remaining item is RED-first per the brief. Never weaken an
  assertion; never edit the ruler files; no new dependencies; do not
  stage or commit.

ACCEPTANCE before reporting done (all green in your own runs):
`cargo nextest run --test p9_self_improvement_containment`;
`cargo nextest run -p gauntlet-app promote::`;
`cargo nextest run -p gauntlet-app --test promotion candidate_only`;
`cargo nextest run -p gauntlet-ports -E "test(revision_ledger)"`;
`cargo nextest run -p gauntlet-adapter-state-sqlite --test contract_suite`;
`cargo nextest run -p gauntlet-arch-tests -E "test(optimizer)"`;
`just check`.
