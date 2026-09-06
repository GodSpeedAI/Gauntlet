# Task 25 — COR-3 progress checkpoint (corrections round 3)

Builder: COR-3. Started 2026-09-05. Brief: `.agents/reports/task25/cor3-brief.md`
(findings source: `.agents/reports/task25/falsifier-round2.md`, VERDICT CONFIRM).

## Phase 0 — orientation (DONE)

- `just ruler` at session start: **PASS** (`verify-ruler.sh: PASS — answer key
  and manifest digests match the frozen lock`), RULER_EXIT=0.
- All target loci read and identified:
  - F2-1: dedup scan conjunct at `crates/adapters/gauntlet-adapter-escalation-fs/src/queue.rs`
    (`escalate`, `.find(|record| record.is_open() && record.run_id == item.run_id && record.summary == item.summary)`).
    Test locus: `crates/adapters/gauntlet-adapter-escalation-fs/tests/contract_suite.rs`
    (crate-local additions only; the SHARED suite lives in `gauntlet_ports::contract`
    and is not edited).
  - F2-4: `crates/adapters/gauntlet-adapter-agent-cli/src/transport.rs` stop legs —
    the shared `pid = format!("-{}", inner.id())` binding serves TERM (line ~268/273)
    and KILL (line ~305); the falsifier's exact A3-M3 mutant re-points ONLY the KILL
    leg's argv target to the direct pid. Test locus: NEW file
    `crates/adapters/gauntlet-adapter-agent-cli/tests/cor3_group_kill_teeth.rs`
    (transport-level, TERM-ignoring driver + TERM-ignoring grandchild, /proc-stat
    liveness excluding Z, hygiene-kill BEFORE the failure assert — F2-6 leak-proof).
  - F2-2: producer `crates/gauntlet-app/src/govern/gate.rs` `GateRefusal::summary()`
    `LatticeMovementRefused` arm (only unit-free variant; the payload's
    `LatticeMovement::Refused` DOES carry `unit`). Mint site controller.rs:2412
    wraps `refusal.summary()` verbatim. Every other producer embeds the unit
    (audited: all other `GateRefusal` arms; all six `EscalationCause` Display arms;
    scheduler `Ineligible`; the drain and correction-blocked summaries carry unit
    lists). Regression: gate-level two-units-same-rule summaries DISTINCT+unit-bearing
    (RED-first), plus a queue-level companion leg (unit-bearing-shaped summaries,
    same run → two tickets) in the escalation contract suite.
  - F2-3: `FsEscalationQueue` scan-then-mint race. Plan: serialize the mutate path
    (escalate + resolve) with an in-process lock acquired TOP-LEVEL at the head of
    each method, never nested — the repository writer-lock PATTERN (single holder,
    fail-closed), not a second lock FILE: cross-process exclusivity is already
    guaranteed by the state-dir writer lock (single writer per state dir, spec
    §20.1), and a second lock FILE beside the queue would add a stale-lock wedge
    with manual recovery inside a path every run hits automatically. Disclosure to
    cor3-report.md.
  - F2-7: pin leg = NEW test in `crates/gauntlet-cli/tests/e2e_budget.rs` (the
    composed budget leg already mints the real governance-routed tickets via the
    real binary): resolve the AK-953 (irreversible, never-dispatched) ticket →
    refused typed `verification_not_pass`, ticket stays open, zero settlements.
    Comment names the residual + ruler-owner. Record: F2-7 paragraph appended to
    `.agents/reports/e2e-real.md`.
- Baseline sha256 of every file this round may touch:
  `.agents/reports/task25/cor3-baseline-sha256.txt` (queue.rs, contract_suite.rs,
  transport.rs, gate.rs, e2e_budget.rs — the four shared files match the
  falsifier's baseline list exactly).

## Phase F2-1 (DONE — landed)

- Test added: `the_same_summary_under_two_different_runs_is_two_facts_and_mints_two_tickets`
  in `contract_suite.rs` (crate-local; one fixed summary, two run ids).
- RED-first (falsifier's EXACT one-clause deletion applied to queue.rs,
  `record.run_id == item.run_id &&` dropped), executed verbatim:
  `the_same_summary_under_two_different_runs_is_two_facts_and_mints_two_tickets ... FAILED`
  — `assertion left != right failed` with BOTH sides
  `TicketId { ticket: "esc-6a9d3308-0" }` (run B's fact re-affirmed run A's
  ticket); Summary: 16 tests run: 15 passed, 1 failed — ALL delivered tests
  green under the mutant: the executed gap.
- Restore: queue.rs reversed edit; sha256 after restore
  `493a16069c753889b1073d495fec9ecd7a48c9a67aaa7fada94acafc29a8f181` =
  baseline (identical, byte-for-byte).
- Green on the real tree: `cargo nextest run -p gauntlet-adapter-escalation-fs`
  → 16 tests run: 16 passed.
- `just ruler` after phase: PASS (RULER_EXIT=0).

## Phase F2-4 (in progress)

- New transport-level teeth file: `crates/adapters/gauntlet-adapter-agent-cli/tests/cor3_group_kill_teeth.rs`.
  Shape: ProcessChildTransport spawns a TERM-ignoring driver (`trap '' TERM`,
  waits on a background TERM-ignoring grandchild shell that ignores TERM and
  loops sleeps) → group liveness via /proc/<pid>/stat scan (state != Z, pgrp ==
  child pid; F2-5: `kill -0 -- -PGID` counts unreaped corpses, so the /proc
  stat read is the authority and polls stay bounded past reaper latency) →
  `stop` (2 s bound so the TERM leg retires nothing and the escalation leg is
  the only retire path) → poll for live group members → HYGIENE KILL the group
  BEFORE the failure assert (no leaked child ever, F2-6) → assert no live
  member survived, naming captured pids.

## Phase F2-4 (DONE — landed)

- New file: `crates/adapters/gauntlet-adapter-agent-cli/tests/cor3_group_kill_teeth.rs`,
  one test `the_stop_kill_leg_retires_the_whole_group_a_term_immune_tree_survives`.
- Sanity green FIRST on the delivered tree (2.7 s) — then the falsifier's
  exact A3-M3 mutant applied (KILL leg argv target `&pid` → `&inner.id().to_string()`,
  TERM leg untouched):
  RED verbatim:
  `the escalation leg's group SIGKILL must retire the WHOLE tree — live members
  survived: [1286924, 1287276] (after the hygiene group-kill: []) — the KILL
  leg addressed something smaller than the group`
  (Summary: 1 test run: 0 passed, 1 failed, 17.4 s). Hygiene kill ran BEFORE
  the assert: zero leaked children (F2-6 discipline; `ps` sweep clean — the
  one `sleep 1` in the sweep belongs to the host's pre-existing tunnel
  watchdog, pgid 714, unrelated).
- Restore: transport.rs reversed edit; sha256 after restore
  `9f7197e42f0edd9df6df0fb270fe6f51093c7b75367f7b9e5e0bb7d9b55de8e7` =
  baseline (identical, byte-for-byte).
- Green on the real tree: 1 test run: 1 passed (2.2 s).
- `just ruler` after phase: PASS (RULER_EXIT=0).
- F2-5 recorded in the test's own doc comment (the `/proc` stat authority vs
  the reaper-latency-sensitive `kill -0 -- -PGID` idiom) and in the liveness
  helper's comment.

## Phase F2-2 (DONE — landed)

- RED-first (test added to `govern/gate.rs` tests BEFORE the fix), under the
  shipped producer, executed verbatim:
  `a_lattice_refusal_summary_is_unit_bearing_two_units_same_rule_mint_two_facts ... FAILED`
  — `assertion left != right failed` with left == right ==
  `"dispatch refused by the governance gate: lattice movement refused: the
  dispatch facts may not rank below the recorded facts (AK-046)"`
  (byte-identical summaries for units AK-001 and AK-002 — the falsifier's
  A4-P1 collision at the real producer; Summary: 1 run, 0 passed, 1 failed).
- Production fix (the only one this round beyond F2-3's lock, per brief):
  `govern/gate.rs` `GateRefusal::summary()` `LatticeMovementRefused` arm now
  renders `lattice movement refused: {rule} — unit {unit}` (the arm's payload
  always carried the unit; every other arm already embeds it — audited all
  `GateRefusal` arms, all six `EscalationCause` Display arms, the scheduler
  `Ineligible` Display, the drain and correction-blocked summaries).
- Green after fix: govern:: 15/15; full `gauntlet-app` package 567/567.
- Queue-level companion leg added to the escalation contract suite:
  `two_unit_bearing_summaries_in_one_run_mint_two_tickets` (summaries shaped
  exactly like the fixed producer's mint, same run → two tickets listed).
  Green by construction against the delivered queue (the defect was the
  producer, not the dedup) — disclosed as a companion regression, not a
  RED-first: `cargo nextest run -p gauntlet-adapter-escalation-fs` → 17/17.
- Disclosed decomposition: gauntlet-app cannot depend on the escalation
  adapter (dependencies point inward), so "two units → two tickets" is
  executed as its two real halves: producer unit-bearingness (RED-first,
  gauntlet-app) + queue two-tickets-for-distinct-summaries (escalation-fs).
- `just ruler` after phase: PASS (RULER_EXIT=0).

## Phase F2-3 (DONE — landed, mechanical after all)

- RED-first (stress test written BEFORE the lock), under the shipped
  scan-then-mint queue, executed verbatim — stronger than the falsifier's
  P2 (which saw 2 tickets): round 0 minted FOUR tickets for one fact
  (`left: 4, right: 1`, distinct ids `esc-6a9d3be5-{0,1,2,3}` across 8
  threads; Summary: 1 run, 0 passed, 1 failed).
- Fix: `FsEscalationQueue` gains a `mutate_lock: Mutex<()>`; acquired
  TOP-LEVEL as the first statement of BOTH mutate methods (`escalate`,
  `resolve`), NEVER nested (neither calls the other; `list`/`record` take
  no lock — the task-23 lesson). Poisoned lock refuses typed
  (SubmitFailed / WriteFailed), never silently unlocked. Pattern
  disclosure: this is the repository writer-lock PATTERN (single holder,
  fail-closed) at the queue's grain — NOT a second lock FILE, because
  cross-process exclusion is already the state-dir writer lock's job
  (spec §20.1 single writer), and a lock file beside the queue would add
  a stale-lock wedge with manual recovery inside a path every run hits
  automatically. Recorded in the field's doc comment.
- Bounded stress test `concurrent_escalations_of_one_fact_mint_one_ticket`
  (8 threads × 12 rounds, fresh queue per round, no unbounded waits):
  GREEN after the fix.
- `cargo nextest run -p gauntlet-adapter-escalation-fs` → 18/18. No
  struct-literal constructions of the queue outside `open()` (grep proof).
- `just ruler` after phase: PASS (RULER_EXIT=0).

## Phase F2-7 (DONE — pin + record, no flow build)

- Pin: new test `resolving_a_governance_routed_ticket_is_refused_typed_no_pass_record_exists`
  in `crates/gauntlet-cli/tests/e2e_budget.rs` — composed run (real binary,
  escalated terminal), the AK-953 irreversible never-dispatched ticket is
  resolved → REFUSED typed: non-zero exit, stderr carries
  `verification_not_pass` AND `no PASS verification record exists for unit
  AK-953`; ticket stands open after the refusal; zero settlement events.
  The test's doc comment names the residual (the designed operator flow
  dead-ends for both governance-routed classes; the run can never reach
  `settled` in this world) and its owner (ruler-owner /
  CandidateRevision material; surface is task 21's).
- Green: `cargo nextest run --test e2e_budget` → 3 tests run: 3 passed.
- Record: F2-7 residual paragraph appended to `.agents/reports/e2e-real.md`
  (the dead end, the executed reproductions, the ruler-owner question).
- `just ruler` after phase: PASS (RULER_EXIT=0).

## Next: full battery (brief's list, every invocation flock'd)
