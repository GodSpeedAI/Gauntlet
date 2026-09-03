# Task 22 — COR-2 correction brief (builder)

Tree: /home/sprime01/projects/gauntlet; task22 work UNCOMMITTED (do not
commit). Falsifier round 2 returned CONFIRM (0 blocking / 0 major /
3 minor / 3 informational) at
`.agents/reports/task22/falsifier-round2.md` — read findings F2-1 and
F2-2 (F2-3 is the ORCHESTRATOR's adjudication, not yours: it goes to
reference-candidates as a CandidateRevision; do not touch it).

Two narrow corrections:

## C6 (F2-2) — a zero-reason refusal must not compose

`TransitionReason::Refused(vec![])` composes and applies today as a
Provisional→Provisional re-record, durably writing a refusal with no
reasons — a record the honest `PromotionService` can never produce.
Fix (binding): composing a `Refused` reason with an EMPTY refusal vec is
a typed composition error (extend the existing reason↔target mismatch
mechanism or add a variant — builder's choice, keep it typed and named);
the suite grows a case on all legs (green stub + sqlite adapter); a
module test asserts the service path is unaffected. RED-first: write the
test, watch it fail against the current tree, then fix.

## C7 (F2-1) — the chronology doc claim is softened to the truth

The promotion module doc says windows decide "never on import order" —
overshoot: under an `occurred_at` TIE the seq tiebreak is append order,
and the falsifier executed a tie whose verdict flips Promote vs Refuse
with import order. Fix (binding): docs-only — state the tie rule
honestly: occurrences order by `(occurred_at, ledger seq)`; the seq
tiebreak is deterministic but is append order, so ties are
order-dependent by construction (recorded residual). Do NOT change the
tiebreak behavior; do NOT touch assertions.

## Discipline

Same as COR-1: flock build protocol; package-scoped gates first; plan
gates (`--test p10_no_false_learning -p gauntlet-cli`, `-p gauntlet-app
memory::`), ports suite legs, sqlite contract_suite, `just boundaries`,
`just lint`, `just ruler` (start + end, PASS both) green before you
report. Never touch the ruler files, the settlement path, or
.agents/reports/reference-candidates.md (the orchestrator owns F2-3's
entry). Nothing staged or committed; no sub-agents; zero new deps.
Checkpoint to cor2-progress.md; final cor2-report.md (files + sha256 +
gate tails + deviations).
