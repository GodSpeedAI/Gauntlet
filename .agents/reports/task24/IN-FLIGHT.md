# IN-FLIGHT — Task 24: anti-gaming suite G1–G16 + mutation teeth

**Dispatched:** 2026-09-04, orchestrator session (task24 start).
**Plan:** `.agents/plans/gauntlet-impl.md` §Task 24. **Ruler units:**
AK-058 + AK-070. **Charter:** `charter.md` (this directory).

## Slices

- **B1** — suite + manifest + E-1 scan tokens + G16 i1 fix (REAL
  defect, RED-first) + scoped mutation run. State: COMPLETE 2026-09-04
  (report: b1-report.md; battery green; tree reconciled by
  orchestrator).
- **Blind design-as-built critic r1** — NOT_CONFIRM (0 BLOCKING /
  1 MAJOR / 3 MINOR / 5 INFORMATIONAL; critic-design.md). MAJOR F-1:
  G7 mutant exclusion wider than true equivalence.
- **COR-1 (B2)** — C1 shrink G7 exclusion to the two `<=` equivalents
  + operator authority-decrease leg (RED-first); C2 manifest/suite
  header honesty (plan-gate filter divergence); C3 canonical mutation
  re-run on the final tree; C4 widened G16/G1 mutation spans; C5
  earlier_record_failures early-return restructure. State: COMPLETE
  (cor1-report.md; mutation re-certified 73 examined / 63 caught /
  10 unviable / missed 0).
- **Executing falsifier r1 (fresh, blind)** — CONFIRM (0 CRITICAL /
  0 MAJOR / 1 MINOR / 2 INFORMATIONAL; falsifier-round1.md). 16/16
  strategies attacked via 17 neutralizations, every named defense
  load-bearing (named test RED under neutralization, byte-identical
  restores proven). MINOR F-2: manifest validator bindings are
  prefix/bounds, not identity (hardening candidate recorded, not
  blocking).
- **COR-2 (B3)** — E-2 trigger discharge (task23 residual: by-unit
  instrument join first-row order sensitivity): any-weaker join in
  `instrument_series_verdict` + RED-first order-independence test
  (suite 18 → 19). State: COMPLETE (cor2-report.md).
- **Orchestrator (closure)** — cor2's edit shifted gate.rs lines +18
  BELOW the edit, desyncing two manifest entries (G5 :780→:798,
  G16 :837→:855) and two script spans (781:800→799:818,
  842:912→860:930) — re-pinned by the orchestrator and the mutation
  pipeline re-run on the final tree (closure-mutation-run.log), then
  the personal closure battery + Done-when tooth drill.

## Contract

- B1 writes `b1-progress.md` at every checkpoint and `b1-report.md`
  on completion (with sha256 digests of every file it touched).
- Completion = schema-valid report file, never a mailbox message.
- Tree baseline before dispatch: `git status` clean except
  `.jolli/**` (operator tooling, never staged) and this directory.

## Recovery protocol (orchestrator)

Before treating any interruption as a death: message the agent and
WAIT for observable progress (file mtime or process change) —
a "dead" agent may revive (task23 lesson). On confirmed death:
reconcile `git status` + `b1-progress.md` against this ledger, then
dispatch a continuation builder that reconciles orphaned state FIRST.
One builder on the tree at a time. After ANY interleave suspicion:
orchestrator runs the FULL battery, not slice-scoped suites.
