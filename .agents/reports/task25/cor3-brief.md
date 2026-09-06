# Task 25 — COR-3 brief (corrections round 3, small)

You are the corrections builder for task 25's third corrections round in
/home/sprime01/projects/gauntlet. The focused falsifier round 2 returned
CONFIRM (the round-1/2 corrections held under execution) with 3 MAJOR /
3 minor / observations, every one executed
(`.agents/reports/task25/falsifier-round2.md` — read it FIRST; its
mutant recipes and probe shapes are your specifications). Orchestrator
dispositions below. The teeth fixes are TESTS-ONLY; the summary fix is
one small content change; nothing else in production moves.

## Dispositions (orchestrator-owned, binding)

- **F2-1 (MAJOR — FIX, tests-only).** The escalation dedup's per-run
  conjunct has zero executed teeth: deleting
  `record.run_id == item.run_id` leaves the ENTIRE battery green.
  Fix: a contract-suite test in `gauntlet-adapter-escalation-fs` — the
  SAME summary escalated under TWO different run ids must mint TWO
  tickets (the listing shows both). RED-FIRST: apply the falsifier's
  exact one-clause deletion, show the new test RED (all delivered
  tests stay green — the gap, verbatim), restore byte-identically
  (sha256 both sides), green on the real tree.
- **F2-4 (MAJOR — FIX, tests-only).** The stop's KILL leg can silently
  revert to direct-pid addressing (recreating the round-1 orphaned-tree
  leak for TERM-ignoring wrapped trees) with the whole battery green;
  only a /proc-liveness probe discriminates. Fix: a transport-level
  teeth test whose shape makes the group-KILL leg the ONLY retire path
  — a TERM-ignoring child WITH a TERM-ignoring grandchild (within the
  documented bounded claim), survivors asserted by bounded /proc
  liveness. RED-FIRST: apply the falsifier's exact mutant (KILL leg
  argv `-<pid>` → direct pid), show the new test RED with live
  survivors named, restore byte-identically, green on the real tree.
  Mind F2-5 (zombie reaping latency): poll past reaper latency or wait
  on the corpses explicitly so the assertion is not
  reaper-latency-flaky.
- **F2-7 (MAJOR — RECORD + PIN the honest posture; NO flow build).**
  The operator resolve/retry surface demands a durable-PASS
  verification record that operator-routed tickets (§11.5-row-6
  lead_agent closure; irreversible never-dispatched) can never carry —
  every resolve is refused typed and the run cannot reach `settled` in
  this world. Building operator-closure semantics is OUT OF SCOPE for
  task25 (task21 surface; §16.1's evidence requirement for human
  closure makes this a ruler-owner question). Disposition:
  1. PIN the posture: a small assertion (in the escalation/budget leg
     you find natural) that resolving the governance-routed ticket is
     REFUSED TYPED with the recorded reason — the honest boundary,
     documented in a test comment naming the residual and its owner
     (ruler owner / task21-surface follow-up).
  2. RECORD: `e2e-real.md` gains the F2-7 residual paragraph (the
     dead-end, the executed reproduction, the ruler-owner question:
     how does an operator-routed Unit ever acquire the §16.1 PASS
     evidence? — CandidateRevision material). The orchestrator adds
     the reference-candidates entry at settlement.
- **F2-2 (MINOR+ — FIX).** The shipped `LatticeMovementRefused`
  escalation summary can be byte-identical for DIFFERENT units (the
  unit id is dropped), so a genuinely new hard residual re-affirms
  another unit's ticket and is dropped (G8-adjacent). Fix: every
  escalation summary minted by the composed loop is UNIT-BEARING
  (include the unit id) — the falsifier named the producer locus; fix
  at the mint site(s), not the dedup semantics. Regression test: two
  units, same refusal class, same run → TWO tickets (this also
  hardens the dedup's fact-carrier contract). RED-first under the
  shipped producer.
- **F2-3 (MINOR — FIX if mechanical, else RECORD).** Concurrent
  escalate of one fact mints 2 tickets (scan-then-mint race; direction
  of failure is noise, not suppression). Fix preferred: serialize the
  queue's mutate path with the repository's existing writer-lock
  pattern (a lock beside the queue, top-level, never nested), plus a
  concurrency stress test if cheap (bounded). If you hit a mechanical
  obstacle (port shape, contract-suite conflicts), RECORD the race
  with trigger `next escalation-schema touch` and say so plainly.
- **F2-5 (observation — RECORD, no change).** Group-liveness idioms
  are reaper-latency-sensitive; note in the F2-4 test's comment.
- **F2-6 (ops hazard — RECORD, no product change).** Spawned children
  inherit the compile.lock fd across the flock→nextest→test→transport
  chain; the group retirement is the shipped defense; the wedge is
  the recorded task23/20 class. Record in cor3-report.md; the
  orchestrator reinforces the standing warning at settlement.

## Constraints (unchanged, binding)

- Never touch: repo-root `.wayfinder/**`, repo `.gauntlet/**`,
  repo-root `GAUNTLET.md`, settled `p*.rs`/`g*.rs`/`antigaming.rs`/
  `e2e_full_loop_mock.rs`, `.agents/CURRENT_STATUS.yml`. Production
  changes are LIMITED to the F2-2 summary mint site(s) and (if you
  take it) the F2-3 queue lock — anything else is a deviation to
  disclose.
- No new dependencies; no network; bounded waits (mind F2-5).
- RED-first evidence verbatim for F2-1, F2-4, F2-2 (and F2-3 if fixed).
- Checkpoint: `cor3-progress.md`. Contract: `cor3-report.md`. Final
  message is a signal only. Nothing staged or committed.

## Battery (all green before reporting)

```bash
just ruler
just check-package gauntlet-cli
just check-package gauntlet-app
cargo nextest run -p gauntlet-adapter-escalation-fs
cargo nextest run -p gauntlet-adapter-agent-cli
cargo nextest run --test e2e_full_loop_real_runner
cargo nextest run --test e2e_recovery
cargo nextest run --test e2e_budget
cargo nextest run --test e2e_full_loop_mock
cargo nextest run --test escalations_cli --test operator_actions_no_bypass
cargo nextest run --test p5_hard_gate
cargo nextest run --test p7_recovery --test p6_budget_control
just lint
just boundaries
```

## Operational addenda (binding — identical to prior rounds)

- ONE compile at a time: EVERY cargo/just invocation is
  `flock .agents/reports/task25/compile.lock -c '<command>'`, flock
  its OWN top-level command, never nested; `sccache --stop-server`
  inside the same flock'd command. Default threading only.
- `just ruler` before and after every phase; STOP if it fails.
- Probe hygiene: nothing planted under repo `.gauntlet/policies/`;
  `ls` proof. Reap every test child (F2-6's wedge is one leak away).
- API-outage discipline: checkpoint after every landed fix; retry
  connection errors; on persistent outage write resume state and stop.
