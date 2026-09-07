# Failure and recovery

Spec §18 (persistence, state, recovery) and §21 (failure model) are
normative. Per-unit conformance truth lives in
`.agents/reports/conformance.json` and `.agents/reports/handoff.md`.

## Failure classes

Every failure carries one of the sixteen typed classes of spec §21.1:
`configuration · reference · formation · workspace · agent_execution ·
tool_execution · observation · comparison · verification · settlement ·
persistence · environment · budget · governance · recovery ·
inconclusive_evidence`. Typed errors preserve the locus; bare strings and
swallowed errors are banned; `PASS`, `FAIL`, and `INCONCLUSIVE` remain
three states through every layer.

## Degradation rules (spec §21.3)

| Failure | Rule |
|---|---|
| Config/reference invalid | stop new dispatch; keep the process alive; keep reconciliation running |
| Agent session failure | retry or escalate per §11.5; never settle the unit |
| Sandbox failure | escalate; **never** fall back to unsandboxed execution |
| Observation failure | record unavailable evidence; dependent verification becomes `INCONCLUSIVE` |
| Surface/log failure | never crash the run |
| Integrity violation | abort the run; preserve everything |

## Substrate

A single-file embedded database (SQLite via rusqlite) is the default
`StateStore` adapter choice — not an architectural commitment (spec
§18.1). The event log is append-only and crash-safe; the evidence store is
content-addressed with no overwrite path; deterministic export to JSONL is
required regardless of backend. `RunId`s are collision-resistant and
lexicographically sortable.

## Checkpoints and resume

Every iteration boundary writes a durable checkpoint: run id, reference
digest, iteration number, unit-state projection, budget spent, in-flight
invocation ids with their result-contract paths, and the event-log offset
(spec §18.2). **Critical run state is never reconstructed from
conversational memory.**

`gauntlet resume <run_id>` (spec §18.3):

1. Replay the event log from the last checkpoint offset; complete any
   half-applied settlement idempotently.
2. Re-freeze-verify the reference digest; mismatch →
   `aborted{reference_mutated}`.
3. Re-probe in-flight invocations: a valid result contract on disk is
   collected; anything else is marked `Failed{class: orphaned_by_restart}`
   and re-planned.
4. Sweep workspaces for settled/out-of-scope units.
5. Re-run preflight and continue.

**Shipped semantics — re-production, not adoption.** A crash landing after
a production row is durable leads the resume to re-dispatch and re-produce:
§14.3 staleness makes the pre-crash iteration's evidence unusable, so the
locator holds one production row per iteration. This is a recorded design
decision with a measured payment (F-C4-1: the first re-dispatch inherits
the crashed attempt's workspace and is one wasted attempt before the
settling one; with an attempts ceiling of 2 the produce-window crash class
escalates instead of settling) — reference-candidates 2026-09-06 entry 4.
The recovery legs pin these spec-honest semantics tests-only after the
task-25 closure battery executed the produce-window defect live.

## Budget as governance

`BudgetMeter` is consulted before every dispatch, never after (spec
§17.4). Dimensions: rounds, parallelism, depth, wall time, tokens, cost,
tool invocations, retries. Breach stops dispatch, drives the run to a
terminal state, and reports. The recovery reserve is never spendable on
forward work. A non-converging run terminates at the declared limit with
the reserve intact — through the real binary in the task-25 E2E.

## Proven vs provisional

Status categories are spec §26's six; unit-level claims defer to
`.agents/reports/conformance.json`.

- implemented_but_provisional — AK-041: the atomic commit sequence appends
  the durable event before any status write and before any in-memory
  advance (`crates/gauntlet-cli/tests/p7_recovery.rs`, task 13).
- implemented_but_provisional — AK-042: killing the process mid-settlement
  and resuming settles exactly once — no double commit, no lost evidence
  (p7; the real-binary recovery legs at task 25: SIGKILL at observed
  durable boundaries, exactly-once resume).
- implemented_but_provisional — AK-051: a non-converging task terminates
  at the declared budget limit with the recovery reserve unspent
  (`p6_budget_control.rs` + `e2e_budget.rs` through the real binary).
- implemented_but_provisional — AK-053: every iteration boundary writes a
  durable checkpoint sufficient to resume without conversational context
  (`crates/gauntlet-app/src/state/`, the state-store contract suite, and
  the composed resume path; task 6, composed at task 20).
- implemented_but_provisional — AK-054: run records export
  deterministically to JSONL — exporting the same run twice is
  byte-identical (`crates/gauntlet-app/src/state/` export service;
  `crates/gauntlet-cli/tests/export_stack.rs`).
