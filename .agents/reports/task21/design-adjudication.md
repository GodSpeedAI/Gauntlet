# Task 21 — design-critic adjudication (orchestrator)

Critic: blind design-as-built, fresh context. Verdict: **CONFIRM (0
blocking / 0 major / 5 minor / 6 informational)**. Report:
`critic-design.md`. Adjudication below; corrections land as one builder
slice (COR-1) after falsifier r1 reports, then a focused re-attack.

## Correction decisions

| Res | Sev | Decision | Route |
|---|---|---|---|
| RES-01 refused/failed log missing run_id+ticket (D7 field parity) | MINOR | **CORRECT** — binding-charter shortfall on the audit surface this task exists to provide; cheap | COR-1 + test assertion |
| RES-03 serve loop dies on transient accept errors vs "serves until interrupted" | MINOR | **CORRECT** — classify accept errors (continue transient / break listener-fatal), keep §21.3 posture | COR-1 + injected-failure unit test |
| RES-04 sandbox_denials matcher narrower than its doc claim | MINOR | **CORRECT** — drift-proof shape preferred (pin to the governance gate's own summary vocabulary); if no shared constant exists cheaply, narrow the doc to the implemented matcher and say which | COR-1 (builder records which shape) |
| RES-05 lib.rs field-order comment inaccurate ("escalated FIRST" vs run-first port order) | MINOR | **CORRECT** — one doc line stating the actual rule + the recorded charter tension | COR-1 |
| RES-10 non-string `note` silently coerced to empty | INFO | **CORRECT** — typed 400 `body_unparsable`; untrusted input is never silently coerced | COR-1 |
| RES-08 escalations_cmd.rs header names retired entry point | INFO | **CORRECT** — one header line | COR-1 |
| RES-11 Cargo.toml "ONLY feature edge" overstatement | INFO | **CORRECT** — reword to "the only edge THIS task adds" | COR-1 |
| RES-02 ticket↔unit pairing unbindable (EscalationRecord has no unit field) | MINOR | **TRIGGER-RECORD** — principled fix is a ports revision (schema change to a pre-existing record); cheaper disclosure fix (typed acceptance line naming settled unit + resolved ticket) folds into COR-1 as a disclosure only; the binding itself waits for the ports revision | deferral + partial disclosure in COR-1 |
| RES-06 open_residuals structurally empty, undisclosed on the wire | INFO | **TRIGGER-RECORD** — rides the already-recorded typed-durable-residual-read ports deferral | deferral |
| RES-07 leg-3 authority premise unreachable for identified operators | INFO | **RECORD AT SETTLEMENT** — closure restates it; manufacturing it would mean changing the service or the ruler, both out of scope | closure |
| RES-09 AK-062 frozen locator divergence | INFO | **RECORD AT SETTLEMENT** — existing reference-candidates entry (2026-08-28) is the record; nothing frozen is edited | closure |

## D11 note (critic vs executed evidence)

The critic's static analysis of the charter's one-line mutation
(its "D11 teeth (structural)" section) predicted E2E leg 1 RED under
the service-side gate deletion. The orchestrator's EXECUTED teeth runs
(orchestrator-teeth.md, sha256-pinned) show leg 1 stays GREEN under
that mutation — the composition root calls the gate itself, so the
composed path still refuses — while the in-crate sensor goes RED; and
leg 1 goes RED under the composition-side bypass of both gates. Each
layer's bypass is caught by its own layer's sensor; the critic's
reasoning missed the composition-layer gate. Executed evidence
outranks static reasoning; no action beyond this record.

## Rule invoked

0 blocking / 0 major ⇒ no re-round owed on the critic's own findings;
minors may be trigger-recorded. RES-01/RES-03 are being corrected
anyway because they are binding-charter shortfalls fixable inside this
task's scope at near-zero risk; the rest follow the task17/task18
precedent (minors trigger-recorded with named triggers).
