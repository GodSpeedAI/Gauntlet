# Task 21 — IN-FLIGHT ledger

Status: **SETTLED 2026-09-03** (orchestrator). Plan Task 21 — operator
surfaces (feature-gated). Charter: charter.md (binding, D1–D12).
Closure: closure.md. The task22 line is where the settling session
stops, per operator instruction.

## Slices

| Slice | Scope | Agent | State | Result contract |
|---|---|---|---|---|
| B1 | Adapter crate + CLI feature work + no-bypass test (charter steps 1–6) | builder (dispatched 2026-09-02) | COMPLETE (b1-report.md; landed in a71eba9) | .agents/reports/task21/b1-report.md |
| ORCH | Personal gate battery + D11 teeth | orchestrator | COMPLETE (orchestrator-teeth.md) | .agents/reports/task21/orchestrator-teeth.md |
| CRITIC | Blind design-as-built critic (fresh context) | critic | COMPLETE — CONFIRM 0 blocking / 0 major / 5 minor / 6 informational | .agents/reports/task21/critic-design.md |
| FALSIFIER r1 | Blind executing falsifier (fresh instance) | falsifier | COMPLETE — NOT_CONFIRM 0 blocking / 0 major / 3 minor / 4 informational (every forgery-ladder rung refused; minors narrow) | .agents/reports/task21/falsifier-round1.md |
| ADJUDICATION | Both rounds adjudicated | orchestrator | COMPLETE — 13 items corrected via COR-1, 5 trigger-recorded | design-adjudication.md, falsifier-adjudication-r1.md |
| COR-1 | Correction slice C1–C10 (tests-first) | builder | COMPLETE — all ten items landed; gates green per report; tree left uncommitted | .agents/reports/task21/cor1-report.md |
| ORCH post-COR1 | Personal re-verification of COR-1 tree | orchestrator | COMPLETE — plan gate 8/8, adapter 10/10, settle 52/52, no-default exit 0, E2E 12/12 both configs, cli_surface 7/7 | .agents/reports/task21/orch-post-cor1-battery.log |
| FALSIFIER r2 | Focused re-attack of COR-1 + regression sweep | falsifier | COMPLETE — CONFIRM 0 blocking / 0 major / 3 minor / 2 informational; all ten corrections HELD under executed attack | .agents/reports/task21/falsifier-round2.md |
| ADJUDICATION r2 | r2 findings adjudicated | orchestrator | COMPLETE — 3 minors → COR-2 (same defect classes, cheap fixes); 2 info recorded; staging-claim discrepancy recorded | .agents/reports/task21/falsifier-adjudication-r2.md |
| COR-2 | Micro-slice D1–D3 + D1-addendum (--unit + unknown-arg escape; same-function sweep) | builder | COMPLETE — all items failing-first; gates 12/12, 11/11, lint/boundaries/ruler green | .agents/reports/task21/cor2-report.md |
| ORCH post-COR2 | Personal battery on the COR-2 tree | orchestrator | IN FLIGHT | .agents/reports/task21/orch-post-cor2-battery.log |
| FALSIFIER r3 | Focused: re-attack D1–D3+addendum, adjudicate the two recorded raw-arg loci, no-bypass regression leg | falsifier | COMPLETE — CONFIRM 0/0/3m/0i; all COR-2 fixes HELD; regression leg HELD; backoff proven (19.9 lines/s vs ~1M/s) | .agents/reports/task21/falsifier-round3.md |
| ADJUDICATION r3 | r3 findings adjudicated | orchestrator | COMPLETE — class shown non-convergent (new locus every round); COR-3 class-closing sweep commissioned (FINAL round); scope hard-bounded to the escalations/action family | .agents/reports/task21/falsifier-adjudication-r3.md |
| COR-3 | Class-closing sweep: one escape discipline + enumeration-free invariant teeth (hostile records via queue APIs + argv; no raw control bytes; §20.3 line-count invariant) | builder | COMPLETE — log_quote sweep, per-interpolation audit table, invariant RED pre-fix (11 violations) → GREEN; gates 13/13, 7/7, 11/11, no-default clean, lint/boundaries/ruler green | .agents/reports/task21/cor3-report.md |
| ORCH post-COR3 | Personal battery on the COR-3 tree | orchestrator | COMPLETE — 13/13, 7/7, 11/11, no-default 0 errors, E2E 12/12 both configs | .agents/reports/task21/orch-post-cor3-battery.log |
| FALSIFIER r4 | FINAL focused: r3's three attack families + invariant-teeth attack | falsifier | COMPLETE — CONFIRM 0/0/1m/1i; all families held; minor = invariant test avoided (not declared) the recorded human-line boundary | .agents/reports/task21/falsifier-round4.md |
| ADJUDICATION r4 | r4 adjudicated; tests-only addendum dispatched | orchestrator | COMPLETE | .agents/reports/task21/falsifier-adjudication-r4.md |
| r4 ADDENDUM | tests-only: three hostile-id read-failure shapes driven; boundary pinned BY CONSTRUCTION; carve-out declared | builder | COMPLETE — RED-first (4 violations) → GREEN; gates 13/13, 7/7, lint clean | cor3-report.md (r4 ADDENDUM section) |
| CLOSURE | Closure battery + closure.md + status rewrite + commits | orchestrator | COMPLETE — battery 1392/1392, boundaries 228/228, lint/deny clean, ruler PASS, plan gate 13/13, no-default 0 errors, E2E 12/12 both, cli_surface 7/7 both | .agents/reports/task21/closure.md |

## Provenance resolution (2026-09-02, operator)

The operator CONFIRMED the `docs/` tree in a71eba9 is their own
intentional documentation commit ("i committed some documentation fyi").
The commit bundled that documentation with the task21 implementation,
reports, and .jolli churn under a tutorials-titled message — the
bundling remains a recorded deviation from one-task-one-commit (B1's
report had warned against staging docs with the task21 work), but the
actor and intent are now known: operator documentation, no untangling
needed. The "confirm provenance" open question is CLOSED.

## Phase log (newest last)

- 2026-09-02 orchestrator: state reconciled (git vs CURRENT_STATUS vs
  task20 records; tree dirty only with .jolli/ — unstaged, untouched).
  Context read; charter written; CURRENT_STATUS.yml set to
  active=task21/dispatched. B1 dispatched.
- 2026-09-02 orchestrator: session resumed after interruption. Reconciled:
  B1 complete (b1-report.md); the whole task21 slice + reports was
  committed by an out-of-band actor as a71eba9 under a misleading
  "tutorials" message BUNDLED with an out-of-scope docs/ tree (~36 md
  files) and .jolli churn — B1's report had explicitly warned not to
  stage those with the task21 commit. Deviation recorded; history not
  rewritten (tree outranks record). Gate battery re-run personally:
  ALL GREEN (plan gate 5/5; no-default build exit 0; mock E2E 12/12 in
  BOTH configs; cli_surface+escalations 7/7; ruler PASS).
  D11 teeth verified personally: mutation 1 (service-side gate deleted)
  → in-crate sensor RED; mutation 2 (composition bypasses both gates)
  → E2E leg 1 RED; restores byte-identical (sha256 proven); GREEN
  re-run 5/5 + 1/1. Builder teeth-note inaccuracy recorded (which layer
  catches which mutation). Blind design-as-built critic + blind
  executing falsifier r1 dispatched in parallel (fresh contexts,
  mutually blind).
- 2026-09-02 orchestrator: critic CONFIRM (0/0/5m/6i); falsifier r1
  NOT_CONFIRM (0/0/3m/4i — F-1 re-entry message locus, F-4 ANSI log
  injection, F-5 non-durable-run unit read; forgery ladder refused at
  every executed stage). Both adjudicated: 13 items → COR-1 builder
  slice (dispatched, tests-first), 5 trigger-recorded/recorded at
  settlement. Focused falsifier r2 (corrections + regression sweep) is
  the remaining confirmation step before settlement.

## Recovery notes

- If B1 dies: reconcile `git status` + `.agents/reports/task21/b1-progress.md`
  (declare-first STARTED/DONE lines) against landed code before
  re-dispatch; tree outranks record. Do not treat partially-landed work
  as unstarted.
- One compiler at a time (flock + sccache --stop-server inside the same
  command); background+poll cadence for any call over ~3 minutes.
