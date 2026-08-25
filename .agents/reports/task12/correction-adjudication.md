# Task 12 — critic adjudication (orchestrator)

Sources: `critic-a-result.md` (1 CRITICAL C1, 8 MAJOR M1–M8, 8 MINOR),
`critic-b-result.md` (8 ESCAPED areas, 4 BIT, 2 INCONCLUSIVE, minors).
Convergence: M1↔A2, M2↔A8, M3↔A5, M5↔A7, M7↔A11, M8↔A3c — independent
discovery of the same defects validates both critics.

**Prereg R1 FIRED**: multiple seams share the causal structure "the
composition certifies its own admission / passes via fixture-local wiring
while the shipped path behaves differently" (rounds shadow, synthetic
selection candidate, hardcoded diagnosis shape, unattributed residuals).
Predicted intervention class applies: unforgeable/authority-minted values +
durable-state reads at the control layer. Also P1 fired (meter consult and
dispatch as independent, divergent steps), P3 fired (recurrence never counts
composed), P2 partially (queued-unit snapshot authoritative).

## FIX NOW (correction round)

- **C1** (A-C1): terminal selection — governance-blocked/idle ≠ budget
  exhaustion. Use the scheduler's typed Ineligible reasons: only a budget
  breach yields `budget_exhausted`; irreversibly/authority-blocked-only
  termination yields `blocked` (+ escalation item where §17.1 closure is
  needed — the full authority gate stays Task 14, but the TERMINAL must not
  lie).
- **C2** (A-M1, B-A2, A10-fixture-locality): ONE production rounds wiring —
  the controller's run-`Budget` is the single rounds authority (booked at
  the iteration boundary per the B1 module-doc delegation); the exhaustion
  path terminates `budget_exhausted` with close-out checkpoint + reserve
  intact, never a bare `ControlError`; the rounds gate reads the run budget,
  not the meter; **p6 must pass under the bare `MeteredBudget` production
  wiring** — delete the proof-local `RoundsSupplementedMeter`; p4's factory
  aligned; document the meter/rounds split (port expresses payments; rounds
  are iteration-facts) in both module docs.
- **C3** (A-M2, B-A8): composed diagnosis — remove `implementation_shaped`
  and `disconfirmed` hardcodes; per-phase spend bounded (allowance divided,
  not the whole allowance per phase); `CorrectionIndicated` reachable and
  apparatus loci input-driven through the composed path.
- **C4** (A-M3, B-A5): composed non-convergence — residuals from the real
  diff comparator must attribute (iteration-unit fallback when `unit_refs`
  is empty, grounded in §13.5/§11.5); `ResidualKey` carries the
  expected/observed locator pair; a composed test drives recurrence to N
  through the REAL comparator into diagnosis.
- **C5** (A-M5, B-A7): residual-driven selection — when residuals exist,
  enumerate real candidate corrections, score per §15.1, and record the
  selection WITH rejected alternatives; `CorrectionSelection::new` REFUSES
  an empty rejected list (unrepresentable vacuity — the preregistered A5
  class); authority/tier inputs from real unit facts (no self-comparison,
  no hardcoded tier list erroring cross_cutting completions).
- **C6** (B-A3): scheduler wiring — over-capacity meter scope refusal
  skips-and-continues (never a run error); `PlanInput.units` includes
  settled/out_of_scope members for graph validation while eligibility
  filters separately (no `UnknownDependency` after a settlement); cohort
  charge/prompt consistency (charge what you prompt; prompt what you
  charged); §11.3 cohort construction composed if the reference marks
  non-separability, else recorded honestly (see L4).
- **C7** (B-A4): per-tick re-read covers ALL units, not only in-flight —
  the file is authoritative for queued units too (§11.4 step 3).
- **C8** (B-A9, B-A11 partial): real `event_log_offset` in checkpoints
  (capture the store-returned offset, no constant 0); checkpoint written at
  the iteration BOUNDARY (post compare/verify/settle-in-memory), not
  mid-pipeline; resume restores per-unit attempts from durable state.
- **C9** (B minors+INCONCLUSIVE): `Reservation` construction gated to the
  meter (no public `Reservation::new` forging parallelism accounting);
  escape escalation evidence JSON; stop silently dropping results on charge
  refusal (record + disposition); bounded drain (max polls per tick).

## LATER-SCOPE (recorded, with triggers)

- **L1** (A-M4): §17.1 authority-escalation-before-transform and
  capability-vs-tier dispatch are plan-Task-14 step 1 (AuthorityGate);
  `Unit::settle` authority-blindness is the §16.1 SettlementService
  (Task 13). The controller surfaces authority facts in its completion
  record so both have the hook. Trigger: Tasks 13/14.
- **L2** (A-M7, B-A11 half): full §18.3 resume (log replay, workspace
  sweep, preflight, collected-results refeeding) and retry-ledger
  checkpoint persistence (the §18.2 field set is spec-fixed; a shape
  change is a ports revision). Post-C2 the crash-loop re-arm is bounded by
  rounds exhaustion. Trigger: Task 13 P7.
- **L3** (B-A9 crash-window): mid-window crash replay of the settlement
  commit — Task 13's §16.3 discipline. The Task-12 checkpoint ORDER is
  fixed now (C8); the replay machinery is not built here.
- **L4** (A-M8/B-A3c if spec-awaiting): cohort construction requires the
  reference to mark non-separability; if no shipped surface expresses it,
  record as spec-awaiting (Task 20 E2E reference fixture will exercise it).

## REJECTED / SCOPING NOTES

- **A-M6 "no teeth"**: critic-scope artifact — `teeth.md` (T1–T6, all
  bite-proven with byte-identical reverts) was excluded from the blind
  scope as builder output. The orchestrator re-runs teeth personally at
  closure; that re-run is the evidence of record, not this adjudication.
- **B-A6 controller-drain INCONCLUSIVE**: addressed by C9's bounded drain;
  runner-side deadline compliance remains the port-wide convention
  (Task-15 real runner will make it observable).
