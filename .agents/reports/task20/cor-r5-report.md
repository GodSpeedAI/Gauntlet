# Task 20 — COR-R5 correction report (falsifier round 5 → corrections)

Builder: COR-R5. Work order: `falsifier-adjudication-r5.md` (authoritative).
Inputs: `falsifier-round5.md` (executed attacks + loci), `charter.md`
(+ operational addenda), `cor-r4-report.md` (the predicate adjusted),
AGENTS.md. Checkpoint trail: `cor-r5-progress.md`. `just ruler` PASS
before the first edit and after the last (digests match the frozen lock
both times).

VERDICT (builder claim, no settlement authority): both adjudicated
corrections implemented and pinned. The settlement arm of the
recovery-completed shape now closes an otherwise-all-settled run as the
honest settled no-op — terminal `settled`, exit 0, ZERO booked rounds —
before (and regardless of) iteration booking, so the budget-refused
round can never short-circuit the settled close, through the falsifier's
exact real-binary reproduction (shipped E2E leg 7c) and a controller
twin; below-anchor settlement-row damage on a certified settled no-op is
pinned per row, checked against the store, symmetric with the production
arm (E2E leg 7d + controller twin), and a no-damage resume pins nothing
new. Full battery green (below). No assertion weakened; the two shipped
tests that pinned the removed reconciliation-checkpoint behavior received
mechanical adaptations with recorded reasons (strength preserved, one
zero-write pin ADDED at each site); every other shipped proof, E2E leg,
and gate is green UNMODIFIED.

---

## COR-R5-1 (MAJOR) — a recovery-COMPLETED settlement on an otherwise-all-settled run closes as the settled no-op before/regardless of iteration booking

### Mechanism

One conjunct dropped from the settled no-op predicate (`resume_run`,
`crates/gauntlet-app/src/control/controller.rs`):
`completion.settlements_completed == 0`. With that conjunct, a §18.3
step-1 SETTLEMENT completion (the anchored replay re-inserting a keyed
row from the durable `SettlementCommitted` event — reachable from an
ordinary §16.3/A5 crash, no forgery) routed the all-settled run into
`drive`, where the fresh iteration's round booking was refused by the
recovery-reserve rule, the refused booking set
`terminal=BudgetExhausted`, and `decide_progress`'s all-closed settled
close was skipped (`if state.terminal.is_none()`) — the falsifier's
exit-1 `budget_exhausted iterations=4` over a run whose every unit is
settled; with rounds headroom, the same shape booked a burned
zero-dispatch round. The re-inserted row is durable BY ITSELF —
cross-validated against the durable event (the store's own
`verify_settlement_event_at`: the row's offset and `settled_at` are the
EVENT's own), appending no event, moving no field a checkpoint records —
the same class COR-R4-1 established for the production mint; its
durability derives from the event, not from a reconciliation checkpoint,
and the replay is idempotent on every later resume. So the all-settled
projection now closes at the checkpoint's OWN iteration with zero booked
rounds: the close fires BEFORE (and regardless of) iteration booking.
`drive`, `book_iteration_round`, and `decide_progress` are UNTOUCHED —
the mid-run budget semantics p6 pins (a round booked over an iteration
that did real work) are out of the commission's scope and unchanged. The
`_completion` binding documents that the completion's VALUE no longer
gates the no-op while its side effects always run. The negative stays
pinned: a genuinely unfinished run (a checkpoint legitimately projecting
a unit Open; in-flight work; empty membership) still fails the predicate
and takes the full path — p7's crash-before-event re-dispatch leg and
the attempts-ladder resume leg are green as shipped.

### Tests

- Controller
  `a_recovery_completed_settlement_on_a_settled_run_resumes_as_the_settled_noop`:
  the falsifier's compound reproduction on the fixture — settlement ROW
  deleted, durable checkpoint anchor regressed to just below the event
  (the fixture's close-out anchor sits ABOVE the settlement events; see
  deviations) → resume ⇒ terminal=settled at the checkpoint's OWN
  iteration, zero booked rounds, zero re-dispatch (prompts and
  role_invocations unchanged), the row re-inserted at the EVENT'S OWN
  offset, the settlement unduplicated, ZERO forensic pins of either
  class, no `budget_exhausted` anywhere, and the second resume the same
  no-op (the replay idempotent over the restored row).
- E2E `the_resume_of_a_settled_run_with_a_recovery_completed_settlement_is_the_settled_noop`
  (leg 7c, `crates/gauntlet-cli/tests/e2e_full_loop_mock.rs`): the
  falsifier's exact compound reproduction through the real composed
  binary — one keyed settlement row deleted (the first-settled unit's)
  AND the close-out anchor regressed strictly below that unit's
  settlement event, under the shipped budget posture (4 rounds, 15%
  reserve) whose refused booking produced the dishonest terminal. Pins:
  exit 0, `terminal=settled`, `iterations=` the run's own, zero
  `budget_exhausted`, `implemented-and-proven 3`, zero forensic pins of
  either class, the re-completed row bound to the event's own
  identity byte-level (refs, offset, stored timestamp via the new
  sqlite readers), the export stream gaining EXACTLY the one settlement
  record (event/checkpoint/role_invocation segments byte-identical;
  nothing removed — zero booked rounds, zero re-dispatch, no
  reconciliation checkpoint), and the second resume a byte-identical
  whole-scratch zero-write no-op.

## COR-R5-2 (MINOR) — below-anchor settlement-row damage on a certified settled no-op is pinned per row, symmetric with the production arm

### Mechanism

New `SettlementService::pin_missing_settlement_rows`
(`crates/gauntlet-app/src/settle/mod.rs`), called from inside the no-op
branch only (full-path behavior byte-identical): the controller
cross-checks the event-ordered settled projection against the store's
own keyed settlement rows (the same `settlements()` read that builds the
projection) and pins ONE forensic record per settled-projecting unit
whose keyed row is missing — class `settlement_row_unrecoverable`,
naming the run, the missing row's unit, and the anchor-bounded replay
fact, with the checked-against-the-store posture; it asserts NO byte
presence and NO event position beyond the anchor fact (the durable event
stays the truth the row derives from; the pin records the divergence,
never a repair). `pin_limitation`'s posture mirrored: honest per-row
forensics, no fabrication, no silence. No damage ⇒ no pins: the cross-
check writes nothing when every settled unit's row stands (the shipped
first-resume no-op leg and leg 7c's second resume pin the zero-write
shape; the shipped production-damage legs' pin counts are unchanged —
the production needles and the settlement needle are disjoint).

### Tests

- Controller
  `a_settled_noop_over_below_anchor_settlement_damage_pins_the_missing_row`:
  two-unit run, first-settled unit's row deleted with the anchor above
  its event (given damage mirroring the real store's natural
  multi-iteration posture; the fixture fact is documented at the test) →
  resume ⇒ terminal=settled at the run's own iteration, zero
  re-dispatch, EXACTLY one settlement forensic record naming the damaged
  unit (the intact sibling never named), the anchor-bounded fact and the
  checked-against-the-store posture asserted, the row honestly stays
  missing (never fabricated), the production arm pins nothing, and the
  second resume re-observes the persistent damage and re-pins it
  (per-resume honesty, the production arm's own posture) while staying
  the no-op.
- E2E `the_settled_noop_over_below_anchor_settlement_damage_pins_the_missing_row`
  (leg 7d): the falsifier's NATURAL cell-D2 shape through the real
  binary — one keyed row deleted, close-out anchor INTACT (the damaged
  unit's event sits below it naturally) → exit 0 settled no-op at the
  run's own iteration, exactly one `settlement_row_unrecoverable` pin
  naming the damaged unit (siblings silent, anchor-bounded fact, store
  posture), the row honestly missing (2 of 3 rows remain), the report
  honestly reflecting the keyed divergence (`implemented-and-proven 2,
  provisional 1` — see deviations), zero production pins, and the
  durable run record byte-identical (the forensics live in the evidence
  store, never in the run's own record).

---

## Files touched

- `crates/gauntlet-app/src/control/controller.rs` — predicate conjunct
  dropped + comment rewritten (COR-R5-1 + COR-R5-2 documented in the
  predicate's own bullets), `settlements()` read lifted to a reusable
  binding, the no-op-branch cross-check, the `_completion` discard,
  FakeState `drop_settlement_row` helper, 2 new controller tests,
  1 mechanical adaptation (below).
- `crates/gauntlet-app/src/settle/mod.rs` —
  `pin_missing_settlement_rows`.
- `crates/adapters/gauntlet-adapter-state-sqlite/src/lib.rs` — doc-hidden
  test_support: `delete_settlement_rows` (damage injection),
  `settlement_committed_events` / `settlement_rows` (readers) with the
  `SettlementEventFacts` / `SettlementRowFacts` type aliases (the clippy
  type-complexity fix; the aliases ARE the documented reader shapes).
- `crates/gauntlet-cli/tests/e2e_full_loop_mock.rs` — 3 thin wrappers,
  E2E legs 7c and 7d.
- `crates/gauntlet-cli/tests/p7_recovery.rs` — 1 mechanical adaptation
  (below).
- `.agents/reports/task20/cor-r5-progress.md`, this report, battery logs
  (`cor-r5-*.log`).

## Assertion changes, with reasons (the only two shipped-test edits)

1. `a_resume_completes_half_applied_settlements_and_reconciles_the_projection`
   (controller.rs) — its last block pinned the RAW durable checkpoint
   word carrying the owed unit Settled, i.e. the reconciliation
   checkpoint the old full path wrote. COR-R5-1 commissioned that close
   away (zero booked rounds; the falsifier's reproduction burned exactly
   that round into `budget_exhausted`). Adapted: the settled-ness is now
   pinned in the §5.3.3 event-ordered projection (the checkpoint's word
   upgraded by the durable settlement rows above the anchor — the
   projection every later resume actually derives), and a NEW
   zero-booked-rounds pin (`outcome.iterations == first_outcome.
   iterations`) is added. Teeth preserved: exactly-once keyed-row
   completion, zero re-dispatch — untouched.
2. p7 `a_crash_between_event_and_row_completes_exactly_once_through_resume`
   — same shape: the restart's OWN reconciliation checkpoint no longer
   exists. Adapted: a NEW pin that the restart writes NO checkpoint
   (zero booked rounds), and the both-units-Settled assertion moved to
   the §5.3.3 event-ordered rebuild over the pre-crash checkpoint.
   Teeth preserved and untouched: zero re-dispatch,
   `assert_recovery_invariants`, exactly-once (2 rows).
   No assertion was weakened; nothing was skipped, disabled, or
   excluded. All other shipped tests are UNMODIFIED and green.

## Battery (evidence = what the runs printed; logs in .agents/reports/task20/)

| Gate | Result | Log |
|---|---|---|
| `just ruler` (before edits) | PASS — digests match the frozen lock | cor-r5-ruler-before.log |
| `just check` | PASS (exit 0) | cor-r5-bat-check.log |
| `just test` (workspace) | PASS — **1375 tests run: 1375 passed, 0 skipped** (+4 = the 2 controller tests + 2 E2E legs) | cor-r5-bat-test.log |
| `just lint` | PASS (exit 0; clippy type_complexity → type aliases and op_ref fixes in my new code only; rustfmt on the touched packages — formatting only) | cor-r5-lint4.log |
| `just deny` | PASS (exit 0) | cor-r5-bat-deny.log |
| `just boundaries` | PASS — **228 tests run: 228 passed** | cor-r5-bat-boundaries.log |
| `just doctor` | PASS (exit 0) | cor-r5-bat-doctor.log |
| `just ruler` (after edits) | PASS (exit 0; digests match) | cor-r5-bat-ruler2.log |
| Package tests | sqlite **50/50**, app **474/474**, cli **97/97** | cor-r5-test-{sqlite2,app2,cli2}.log |
| `cargo nextest run -p gauntlet-cli --test e2e_full_loop_mock --test cli_surface` | **16/16** (10 shipped E2E legs UNMODIFIED + legs 7c/7d + 4 cli_surface pins) | cor-r5-bat-e2e.log |
| repo-root dry-run | exit 0 — "dry-run complete — no freeze, no state rows, no evidence pins, no dispatch"; before/after `git status --porcelain` diff **0 bytes** | cor-r5-bat-dryrun.log |

Over-rejection leg clean: the shipped no-op proofs (both worlds), the
pins-don't-defeat-the-no-op legs, the production-damage legs' exact pin
counts, the negative no-false-no-op legs (p7 crash-before-event
re-dispatch, the attempts-ladder resume), and the whole workspace are
green — honest builders settle in both produce worlds; genuinely
unfinished runs still take the full path.

## Deviations / findings for the record

1. **Fixture-fact discovery (controller tests)**: the fixture's
   close-out checkpoint anchor sits ABOVE the settlement events (the
   loop's water-mark threads appends after the settle), so the
   settlement-arm compound reproduction needs the anchor regressed just
   below the event — the SAME given-damage posture the shipped COR-R4-1
   leg uses, documented at the test with a precondition assert that
   fails loudly if the geometry claim is wrong. The natural below-anchor
   shape (the falsifier's D2 cell) needs no surgery in the real store
   and is leg 7d's. My first controller test run failed on the wrong
   geometry premise (the row stayed un-re-inserted); the fix and the
   recorded fact are in ITEM-3 of the progress trail.
2. **A discovery pinned, not papered over (leg 7d)**: with a keyed
   settlement row honestly missing below the anchor, the resume's report
   reads the keyed surface and prints `implemented-and-proven 2,
   provisional 1` — NOT the falsifier's noted "report still prints 3".
   The terminal decision reads the event-ordered projection (settled),
   the report honestly reflects the keyed divergence, and the new pin
   names it — the leg pins exactly that honest shape, with the leg 7c
   contrast (recovered keyed damage reports 3 proven) recorded in the
   comment.
3. **The settlement arm's idempotence is anchor-stable**: the no-op
   writes no checkpoint, so the resume anchor stays where it was and the
   §5.3.3 row-above-anchor upgrade re-derives the same all-settled
   projection on every later resume — the settled no-op is a fixpoint
   (pinned by the second-resume assertions in both worlds).
4. **Gate adaptations in NEW code only**: clippy type_complexity (my
   5-tuple readers → documented type aliases), clippy op_ref (my test
   closures), rustfmt on the touched packages. No gate edited or
   weakened; `just boundaries` green with no inventory change (the
   test_support additions mirror the existing damage-injection shape).
5. `sccache --stop-server` errored twice after short commands (no server
   was running — nothing had compiled / formatting only); benign, the
   protocol's purpose was served trivially.
6. The known load-flaky live-process family
   (`gauntlet-adapter-agent-cli` `falsifier_round1_live_attacks.rs`)
   passed in this round's full battery — no recurrence to note.
