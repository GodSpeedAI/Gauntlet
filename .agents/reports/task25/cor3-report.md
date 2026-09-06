# Task 25 — COR-3 report (corrections round 3, small)

Builder: COR-3, the final small corrections round. Binding brief:
`.agents/reports/task25/cor3-brief.md`; findings source:
`.agents/reports/task25/falsifier-round2.md` (VERDICT CONFIRM; its mutant
recipes and probe shapes were reused exactly). Date: 2026-09-05/06.

**`just ruler` PASS at session start, after EVERY phase, and at the end**
(`RULER_EXIT=0` each time; final explicit run after the battery).

## Dispositions executed

| Finding | Disposition | Outcome |
|---|---|---|
| F2-1 (MAJOR) | FIX, tests-only | LANDED — RED-first under the exact mutant; restored byte-identically; green |
| F2-4 (MAJOR) | FIX, tests-only | LANDED — RED-first under the exact mutant; restored byte-identically; green |
| F2-7 (MAJOR) | PIN + RECORD, no flow build | LANDED — pin test green (the refusal IS the shipped behavior); residual paragraph in `e2e-real.md` |
| F2-2 (MINOR+) | FIX (producer + regression) | LANDED — RED-first under the shipped producer; unit-bearing fix; 567/567 gauntlet-app |
| F2-3 (MINOR) | FIX (mechanical after all) | LANDED — mutate-path lock; stress RED with FOUR duplicate tickets pre-fix; green post-fix |
| F2-5 (observation) | RECORD, no change | Recorded in the F2-4 test's doc comment and liveness helper comment |
| F2-6 (ops hazard) | RECORD, no product change | Recorded below (and practiced: hygiene-kill before every failure assert) |

## F2-1 — the dedup's per-run conjunct has teeth (tests-only)

Test added (crate-local, `gauntlet-adapter-escalation-fs/tests/contract_suite.rs`):
`the_same_summary_under_two_different_runs_is_two_facts_and_mints_two_tickets` —
ONE fixed summary (`escalation_item`), two run ids (`…G5FAV`, `…G5FAW`) must
mint TWO tickets and the listing must show both, each carrying its own run.

RED-first, the falsifier's EXACT one-clause deletion applied to `queue.rs`
(`record.run_id == item.run_id &&` dropped from the dedup predicate), executed
verbatim:

```
test the_same_summary_under_two_different_runs_is_two_facts_and_mints_two_tickets ... FAILED
thread ... panicked at crates/adapters/gauntlet-adapter-escalation-fs/tests/contract_suite.rs:645:5:
assertion `left != right` failed: the same summary under a DIFFERENT run is a new fact — its own
ticket, never a re-affirmation of another run's open ticket
  left: TicketId { ticket: "esc-6a9d3308-0" }
 right: TicketId { ticket: "esc-6a9d3308-0" }
Summary: 16 tests run: 15 passed, 1 failed, 0 skipped
```

The gap, verbatim: ALL 15 delivered tests green under the mutant; the new test
is the sole discriminator. Restore: reversed edit;
`sha256(queue.rs)` after restore `493a16069c753889b1073d495fec9ecd7a48c9a67aaa7fada94acafc29a8f181`
= baseline (identical; mutant sha not recorded — the mutant was a live edit
reversed in place, both sides of the PAIR proven by the restore equality).
Green on the real tree: 16/16 → 18/18 after the round's other additions.

## F2-4 — the KILL leg's group addressing has teeth (tests-only)

New file `crates/adapters/gauntlet-adapter-agent-cli/tests/cor3_group_kill_teeth.rs`,
one test `the_stop_kill_leg_retires_the_whole_group_a_term_immune_tree_survives`:
a `ProcessChildTransport`-spawned TERM-ignoring driver (`trap '' TERM`) with a
TERM-ignoring grandchild (within the documented bounded claim — setsid escape
stays beyond it); the grandchild is observed BEFORE the stop (so the mutant
cannot pass by killing the driver pre-fork); stop bound 2 s forces the
escalation leg (the graceful leg retires nothing); survivors asserted by
bounded `/proc/<pid>/stat` liveness (state != `Z`, pgrp == child pid).
F2-5 recorded in the test's own docs: `kill -0 -- -PGID` counts unreaped
corpses (reaper-latency-sensitive; the falsifier measured >5 s on this host),
so the /proc stat read is the authority and the polls bound only the dying,
never the reaping. HYGIENE: the whole group is killed and observed empty
BEFORE the failure assert fires — a RED run names its survivors and leaks
nothing (F2-6 discipline).

Sanity green FIRST on the delivered tree (2.7 s). Then the falsifier's exact
A3-M3 mutant (KILL leg argv target `&pid` → `&inner.id().to_string()`; TERM
leg untouched), executed verbatim:

```
FAIL [  17.401s] gauntlet-adapter-agent-cli::cor3_group_kill_teeth
     the_stop_kill_leg_retires_the_whole_group_a_term_immune_tree_survives
thread ... panicked at crates/adapters/gauntlet-adapter-agent-cli/tests/cor3_group_kill_teeth.rs:196:5:
the escalation leg's group SIGKILL must retire the WHOLE tree — live members survived:
[1286924, 1287276] (after the hygiene group-kill: []) — the KILL leg addressed something
smaller than the group
Summary: 1 test run: 0 passed, 1 failed, 0 skipped
```

Live survivors NAMED; hygiene kill emptied the group before the assert
(`after the hygiene group-kill: []`) — `ps` sweep clean afterwards (the one
`sleep 1` in the first sweep belongs to the host's pre-existing tunnel
watchdog, pgid 714, started Sep 2 — unrelated). Restore: reversed edit;
`sha256(transport.rs)` after restore
`9f7197e42f0edd9df6df0fb270fe6f51093c7b75367f7b9e5e0bb7d9b55de8e7` = baseline
(identical). Green on the real tree: 1/1 (2.2 s).

## F2-2 — every escalation summary is UNIT-BEARING

RED-first (test written BEFORE the fix) in `govern/gate.rs`'s test module:
`a_lattice_refusal_summary_is_unit_bearing_two_units_same_rule_mint_two_facts`
drives the controller's exact mint expression (`format!("dispatch refused by
the governance gate: {}", refusal.summary())`) over two units refused by the
SAME lattice rule. Under the shipped producer, executed verbatim:

```
thread ... panicked at crates/gauntlet-app/src/govern/gate.rs:1121:9:
assertion `left != right` failed: two units refused by the same rule mint TWO facts —
distinct ticket summaries, never one shared fact colliding into one ticket:
"dispatch refused by the governance gate: lattice movement refused: the dispatch facts
may not rank below the recorded facts (AK-046)"
  left: <the same string>
 right: <the same string>
Summary: 1 test run: 0 passed, 1 failed, 505 skipped
```

Production fix (the brief's one content change, at the falsifier's named
producer locus): `GateRefusal::summary()`'s `LatticeMovementRefused` arm now
renders `lattice movement refused: {rule} — unit {unit}` (the arm's payload
always carried the unit). Audit: every other `GateRefusal` arm, all six
`EscalationCause` Display arms, the scheduler `Ineligible` Display, and the
drain/correction-blocked summaries already embed their unit(s) — this was the
only unit-free producer. After the fix: govern:: 15/15; FULL `gauntlet-app`
package 567/567.

Queue-level companion leg (same file as F2-1's test):
`two_unit_bearing_summaries_in_one_run_mint_two_tickets` — two summaries
shaped exactly like the fixed producer's mint, same run → TWO tickets listed.
Green by construction against the delivered queue (the defect was the
producer, not the dedup); disclosed as a companion regression, not RED-first.

DISCLOSED DECOMPOSITION: "two units, same refusal class, same run → TWO
tickets" is executed as its two real halves because gauntlet-app cannot
depend on the escalation adapter (dependencies point inward): producer
unit-bearingness RED-first (gauntlet-app) + two-tickets-for-distinct-
summaries against the real dedup (escalation-fs). The falsifier's A4-P1
probe executed the collision mechanism itself.

## F2-3 — the queue's mutate path is serialized (mechanical after all)

RED-first (stress test written BEFORE the lock):
`concurrent_escalations_of_one_fact_mint_one_ticket` (8 threads, one fact,
fresh queue per round × 12 rounds). Under the shipped scan-then-mint queue,
executed verbatim — stronger than the falsifier's P2 (which saw 2):

```
assertion `left == right` failed: round 0: every concurrent escalator of ONE fact gets THE
one ticket: [esc-6a9d3be5-1, esc-6a9d3be5-3, esc-6a9d3be5-0, esc-6a9d3be5-2, esc-…-0 ×4]
  left: 4
 right: 1
Summary: 1 test run: 0 passed, 1 failed, 17 skipped
```

FOUR distinct tickets minted for one fact. Fix: `FsEscalationQueue` gains
`mutate_lock: Mutex<()>`, acquired TOP-LEVEL as the first statement of BOTH
mutate methods (`escalate`, `resolve`) and NEVER nested — neither method
calls the other, and the read helpers they use (`list`, `record`) take no
lock (the task-23 lesson, restated in the field's doc comment). A poisoned
lock refuses TYPED (`SubmitFailed` / `WriteFailed`), never silently unlocked.

PATTERN DISCLOSURE (the brief allowed "fix if mechanical"): this is the
repository writer-lock PATTERN — one holder, fail-closed — at the queue's
grain, NOT a second lock FILE beside the queue. Cross-process exclusion is
already the state-dir writer lock's job (spec §20.1's single writer), and a
lock file there would add a stale-lock wedge with manual operator recovery
inside a path every run hits automatically — a new failure class for zero
additional exclusivity. Recorded in the field's doc comment.

Post-fix: escalation-fs package 18/18 (including the stress); no
struct-literal constructions of the queue outside `open()` (grep proof).

## F2-7 — the honest boundary, PINNED and RECORDED (no flow build)

PIN: new test `resolving_a_governance_routed_ticket_is_refused_typed_no_pass_record_exists`
in `crates/gauntlet-cli/tests/e2e_budget.rs` — the composed run (real binary)
ends `escalated` with the AK-953 irreversible never-dispatched ticket open;
`gauntlet escalations resolve --ticket <id> --unit AK-953 --operator …` is
REFUSED typed: non-zero exit, stderr carries `verification_not_pass` AND
`no PASS verification record exists for unit AK-953`; the ticket stands open
after the refusal; zero settlement events. The test's doc comment names the
residual (both governance-routed classes are unresolvable through the
designed surface; the run can never reach `settled` in this world) and its
owner (ruler-owner / CandidateRevision material; the surface is task 21's).
Green against shipped behavior — the refusal IS the shipped behavior:
e2e_budget 3/3.

RECORD: the F2-7 residual paragraph appended to
`.agents/reports/e2e-real.md` ("Corrections round 3 residual — F2-7"): the
dead end, the falsifier's twice-executed reproduction, and the ruler-owner
question — how does an operator-routed Unit ever acquire the §16.1 PASS
evidence its closure requires? The orchestrator adds the reference-candidates
entry at settlement.

## F2-6 — recorded (ops hazard, no product change)

Spawned children inherit the compile.lock fd across the
flock→nextest→test→transport chain (no CLOEXEC on that fd path); the shipped
defense is the group retirement itself, which is why the F2-4 mutant is
dangerous beyond its own leg — it recreates the leak shape that wedges the
whole repository's compile loop (the falsifier reproduced the 28-minute
self-deadlock live and released it by killing the orphans). Practiced this
round: every probe tree is group-killed and observed empty BEFORE any
failure assert; zero survivors on the final sweep. The orchestrator
reinforces the standing warning at settlement: never nest flock, reap every
test child.

## Deviations and disclosures (all minor, none silent)

1. F2-3's lock is in-process (Mutex), not a lock file — rationale above; the
   brief's "if mechanical" branch was taken with the pattern's invariants
   (single holder, top-level, never nested, fail-closed) preserved.
2. F2-2's regression is decomposed across the two crates that own the two
   halves (dependency direction); disclosed above.
3. `cargo fmt` (workspace-wide, via the lint gate's fix path) rewrote
   formatting in EXACTLY two files — my new `cor3_group_kill_teeth.rs` and
   `queue.rs` (whitespace/line-wrapping only, semantics identical; both
   packages re-run green after). Proof: pre-session mtimes on every other
   file, and all nine falsifier-baseline files re-hashed byte-identical
   after fmt (transport.rs `9f7197e4…`, runner.rs `b20960b5…`, cor2 file
   `f30fcc84…`, controller.rs `fddf3f5e…`, report/mod.rs `0ea3ae3d…`,
   sqlite state_store.rs `94b79743…`, encode.rs `88a84f3a…`, ports
   state_store.rs `075363ab…`, gauntlet-app tests/report.rs `6d0e6106…`).
4. `just lint` failed ONCE mid-round (rustfmt diffs on my own files only);
   fixed by formatting my files; clippy itself passed at every invocation.

## Battery (final tree, all green; every invocation `flock .agents/reports/task25/compile.lock -c '…'` with `sccache --stop-server` inside, default threading)

```
just ruler                                              PASS (start, each phase, end)
just check-package gauntlet-cli                         Finished (clean)
just check-package gauntlet-app                         Finished (clean)
cargo nextest run -p gauntlet-adapter-escalation-fs     18/18 (was 15; +F2-1, +F2-2 companion, +F2-3 stress)
cargo nextest run -p gauntlet-adapter-agent-cli         63/63 (+1 cor3 transport teeth)
cargo nextest run --test e2e_full_loop_real_runner      2/2
cargo nextest run --test e2e_recovery                   3/3
cargo nextest run --test e2e_budget                     3/3  (+1 F2-7 pin)
cargo nextest run --test e2e_full_loop_mock             12/12
cargo nextest run --test escalations_cli --test operator_actions_no_bypass   3/3
cargo nextest run --test p5_hard_gate                   4/4
cargo nextest run --test p7_recovery --test p6_budget_control                7/7
(plus, after the production fix: cargo nextest run -p gauntlet-app → 567/567)
just lint                                               PASS (clippy -D warnings + fmt --check)
just boundaries                                         237/237
```

## Tree integrity and probe hygiene

- Repo `.gauntlet/policies/` `ls` proof: exactly `contained.json`,
  `cross_cutting.json`, `local.json` (+ `.gitkeep`) — nothing planted, all
  mtimes pre-session (Aug 28). Every probe ran in `/tmp` scratch worlds.
- Zero leaked probe processes (final `ps` sweep; the F2-4 mutant run's
  survivors were group-killed by the test itself before its assert fired).
- No-touch list untouched and PROVEN by sha256: `transport.rs`, `runner.rs`,
  `cor2_seam_and_recovery_attacks.rs`, `controller.rs`, `report/mod.rs`,
  sqlite `state_store.rs`/`encode.rs`, ports `state_store.rs`, gauntlet-app
  `tests/report.rs` — all byte-identical to the falsifier's baseline digests.
  `.agents/CURRENT_STATUS.yml` (pre-session mtime), repo-root `.wayfinder/**`,
  repo `.gauntlet/**` (beyond the policies `ls` proof), `GAUNTLET.md`, and
  all settled `p*.rs`/`g*.rs`/`antigaming.rs`/`e2e_full_loop_mock.rs` never
  opened for edit.
- Files this round CHANGED (final digests in
  `.agents/reports/task25/cor3-final-sha256.txt`; baselines in
  `cor3-baseline-sha256.txt`):
  - `crates/adapters/gauntlet-adapter-escalation-fs/src/queue.rs`
    (production: F2-3 lock only) `493a1606…` → `1b53670e…`
  - `crates/gauntlet-app/src/govern/gate.rs`
    (production: F2-2 summary arm only + its test) `99e4e208…` → `b672a0de…`
  - `crates/adapters/gauntlet-adapter-escalation-fs/tests/contract_suite.rs`
    (tests: F2-1, F2-2 companion, F2-3 stress) `40a4a575…` → `b92bdbb3…`
  - `crates/gauntlet-cli/tests/e2e_budget.rs` (test: F2-7 pin)
    `726d04c7…` → `b9047a1f…`
  - `crates/adapters/gauntlet-adapter-agent-cli/tests/cor3_group_kill_teeth.rs`
    (NEW: F2-4 teeth) `bb52f5ff…`
  - Records: `.agents/reports/task25/cor3-progress.md`,
    `cor3-baseline-sha256.txt`, `cor3-final-sha256.txt`, this report;
    `.agents/reports/e2e-real.md` (+F2-7 paragraph).
- Nothing staged, nothing committed (`git diff --cached` empty; HEAD still
  the task-24 settlement commit).

## Honest residue

- The composed-loop reachability of `LatticeMovementRefused` remains
  unproven (the falsifier's caveat, unchanged): plan facts and recorded facts
  agree by construction, so the refusal is defense-in-depth. The fix makes
  the producer safe IF it ever fires; the falsifier's A4-P1 probe executed
  the collision mechanism the fix removes.
- The stress test's pre-fix RED is probabilistic by nature (a race); this
  round's executed RED showed the failure at round 0 with 4 duplicate
  tickets. Post-fix it is deterministic green (18/18 across repeated runs in
  the battery).
- The F2-7 dead end is pinned and recorded, not fixed — by disposition
  (ruler-owner question; task-21 surface).
