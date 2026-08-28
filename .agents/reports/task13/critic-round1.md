# Task 13 — blind critic round 1 (landed surfaces B1+B2+B3+COR-1, pre-B4)

Two parallel read-only agents, fresh context, no task13/** reports access
(blind). Full returns preserved below, abridged only where they duplicate.
Dispatched by the orchestrator 2026-08-27 ~11:00 EDT; the installed role
agents (critic/falsifier) are not invocable from this session's delegation
surface (tool: "Agent type not found; Available agents: general-purpose,
Explore") — the roles were emulated in the prompts.

## Spec critic verdict: NOT CONFIRM (narrow, enumerated)
- M-1: §16.1 run-level `settlement.minimum_reliability` conjunct parsed
  (domain reference.rs:306-344) but consumed NOWHERE in control:: —
  run-settled decided at controller.rs:2993-3005 on all-units-settled +
  boundary re-verify only. Mitigating: the frozen reference's own field is
  §19.2 promotion prose, not a machine-consumable count.
- M-2: AK-062 positive half unimplemented (settle_operator_action always
  refuses; no CLI operator surface) — documented Task-21 deferral.
- M-3: AK-031/AK-043 frozen proof locators empty — B4 Items 2/3 in flight
  at read time (p7_recovery.rs existed; p11/g10 not yet).
- Verified-good (selected): §16.1 unit-level all five conditions; §16.3
  order + idempotence key + all three crash windows (real kills);
  §16.4 core-side; §21.3 legs; doctor check 7. Per-artifact CONFIRM table
  in the return; overall NOT CONFIRM turns ONLY on M-1/M-2/M-3.
- Minors: m-1 resume step order (documented deviation), m-2 anchor > vs
  >=, m-3 actor-table superset (Operator{identity:None} accepted for
  critic units), m-4 ForeignRun dead on settle() path, m-5 operator-action
  run-blindness, m-6 verification-record disagreement iteration-sensitivity,
  m-7 check-7 single root, m-8 run-settled stricter than "blocking units",
  m-9 Orchestrator-rationale route unused in-loop, m-10 stale precondition
  doc, m-11 adapter cross-validation claim bounded-unread.

## Falsifier verdict: 7 sequences (1 CRITICAL, 3 MAJOR, 3 MINOR); no frozen claim broken AS WRITTEN
- **F-1 CRITICAL (reachable, no store non-conformance)**: builder-declared
  `attempted` list (controller.rs:3374-3386 trusts payload.attempted())
  can name an ALREADY-SETTLED unit A during a flight on B: the completion
  loop (2356-2504) never checks the unit is the flight's dispatched unit or
  not-Settled; the service's durable condition-4 checks only Blocked; a
  fresh honest PASS at A's locator mints a fresh verification ref = a
  FRESH §16.3 key → second SettlementCommitted event + second row.
  Substance-level break of AK-042's "no double commit" guarantee (the
  literal kill-resume path is sound), plus a scheduler bypass (a
  never-dispatched unit can be settled). Also the single-element-list
  exploit shape (two distinct locators die loudly at write-once).
- F-2 MAJOR = M-2 (AK-062 affirmative conjunct; operator_surface port has
  snapshot/actions but NO execute; settle_operator_action always errs).
- F-3 MAJOR (narrow): admit→commit TOCTOU duplicate event, requires
  concurrent settlers (none in-repo; single-threaded drive).
- F-4 MAJOR/MINOR (narrow): transient edit+revert of a NON-in-flight unit
  between boundaries undetected (reread cancels in-flight only; boundary
  digest matches after revert) — planning reads the mutated projection;
  verification/settlement stay frozen-grounded (no settlement impact).
- F-5 MINOR (gate: concrete adapter type in-process): sqlite inherent pub
  `append_record` accepts RecordKind::VerificationRecord with any JSON —
  bypasses the COR-1 trait-level mint.
- F-6 MINOR: replay path INSERT OR IGNORE swallows same-key
  divergent-evidence events silently (no COR-4 divergence check on
  replay).
- F-7 MINOR = early-probe m3 (orphan completion binds current bytes;
  content-addressed adapter makes it sound).
- 15 held seams verified (foreign-run, non-PASS, evidence substitution,
  invented refs, readiness portability, raw PASS docs, caller-attested
  offsets/timestamps, in-memory-before-durable, kill-resume literal,
  stale-iteration PASS, degraded-iteration settling, transient loops,
  comparator settling, re-opening settled, scheduler re-dispatch).
