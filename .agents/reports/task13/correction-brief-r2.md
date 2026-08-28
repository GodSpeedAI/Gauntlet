# Task 13 / Correction brief round 2 — COR2-1..COR2-6 (critic round 1)

You are the CORRECTION BUILDER. Read `AGENTS.md`, then
`critic-round1.md` (findings) and `critic-round1-adjudication.md` (YOUR
SCOPE — COR2-1..COR2-6 fix-now; the later-scope list is NOT yours).
Checkpoint discipline: STARTED/DONE per item appended to
`correction-checkpoints.md`, never batched; fail-first per behavioral
change (the critic's attack sequence is the RED test). Result contract:
`correction-round2.md`.

- **COR2-1(a)** admission-level one-settlement-per-(run,unit): `admit`
  refuses a DIFFERENT-key commit when ANY settlement row exists for the
  (run, unit); same-key stays idempotent success. RED = the falsifier's
  F-1 sequence end-to-end (settled A re-attempted via a fresh honest PASS
  → today: second event + row; after: typed refusal, zero durable
  writes). **COR2-1(b)** completion-path guard: the per-completion loop
  processes only attempted units intersecting the flight's DISPATCHED
  set; anything else becomes a pinned typed governance record, never
  verified/settled. RED = attempted=["A"] during a flight on B.
- **COR2-2** Operator closures require an identity regardless of unit
  authority (§20.2). RED = Operator{identity:None} on a critic unit
  (today accepted).
- **COR2-3** `settle_operator_action` filters records by run (the COR-5
  mirror). RED = foreign-run PASS via the run-blind ports fake.
- **COR2-4** close the sqlite inherent `append_record`
  RecordKind::VerificationRecord bypass (test-gate/privatize; same idiom
  as `apply_settlement_row`). RED = the concrete-type forgery (today
  writes).
- **COR2-5** anchored-replay divergence detection: a same-key event with
  fields diverging from the existing row → typed refusal, never silent
  OR-IGNORE; same-key-same-fields stays a no-op. RED = raw-injected
  divergent duplicate event (today silently ignored).
- **COR2-6** precondition.rs stale doc trued (doc-only).

Gates (verbatim tails): ruler before/after; check+test gauntlet-ports,
gauntlet-app, gauntlet-adapter-state-sqlite; the four proof filters (p7
too once B4 has landed — coordinate order with IN-FLIGHT); lint;
boundaries. All standing constraints (forbidden paths, no migration, no
weakened assertions, no sub-agents, single build lock) unchanged.
