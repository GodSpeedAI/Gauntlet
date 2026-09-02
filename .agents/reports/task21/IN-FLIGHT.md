# Task 21 — IN-FLIGHT ledger

Status: DISPATCHED 2026-09-02 (orchestrator). Plan Task 21 — operator
surfaces (feature-gated). Charter: charter.md (binding, D1–D12).

## Slices

| Slice | Scope | Agent | State | Result contract |
|---|---|---|---|---|
| B1 | Adapter crate + CLI feature work + no-bypass test (charter steps 1–6) | builder (dispatched 2026-09-02) | IN FLIGHT | .agents/reports/task21/b1-report.md |

## Phase log (newest last)

- 2026-09-02 orchestrator: state reconciled (git vs CURRENT_STATUS vs
  task20 records; tree dirty only with .jolli/ — unstaged, untouched).
  Context read; charter written; CURRENT_STATUS.yml set to
  active=task21/dispatched. B1 dispatched.

## Recovery notes

- If B1 dies: reconcile `git status` + `.agents/reports/task21/b1-progress.md`
  (declare-first STARTED/DONE lines) against landed code before
  re-dispatch; tree outranks record. Do not treat partially-landed work
  as unstarted.
- One compiler at a time (flock + sccache --stop-server inside the same
  command); background+poll cadence for any call over ~3 minutes.
