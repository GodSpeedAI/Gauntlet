# COR-R5 progress (checkpoint trail — declare-first protocol)

Builder: COR-R5. Work order: falsifier-adjudication-r5.md (COR-R5-1 MAJOR,
COR-R5-2 MINOR). Constraint set: charter.md + operational addenda.

## Items

ITEM-0 preflight
STARTED 2026-08-31: git status reconciled (tracked modifications = the
orchestrator's landed COR-R4 state, per AGENTS.md recovery rule); no live
compile.lock holder; no running cargo/sccache. Reading binding docs,
running `just ruler` before edits.
DONE 2026-08-31: `just ruler` PASS (rc=0, digests match the frozen lock;
cor-r5-ruler-before.log). Docs read: charter (+addenda), falsifier-round5,
adjudication-r5, cor-r4-report. Mechanism located:
- COR-R5-1 locus: `resume_run` settled no-op predicate
  (crates/gauntlet-app/src/control/controller.rs ~L1215) — the
  `completion.settlements_completed == 0` conjunct routes the
  settlement-completed all-settled run into `drive`, where the refused
  round booking sets BudgetExhausted before `decide_progress` runs.
  Fix shape: drop that conjunct (COR-R4-1 precedent — a completed
  settlement row re-inserted from the durable event is durable by
  itself, appends no event, moves no checkpoint-recorded field), so the
  all-settled projection closes as the settled no-op with ZERO booked
  rounds — before/regardless of booking. `drive`/`book_iteration_round`/
  `decide_progress` untouched (p6 pins mid-run budget semantics; the
  commission is the resume seam).
- COR-R5-2 locus: same branch — cross-check the event-ordered settled
  projection against the store's keyed settlement rows; per missing row,
  pin one honest forensic record via a new SettlementService method
  (pin_limitation style, distinct class token). Runs ONLY on the no-op
  branch: full-path behavior byte-identical; no damage ⇒ no pins.

ITEM-1 COR-R5-1 predicate + comment (controller.rs)
STARTED 2026-08-31: editing resume_run.
DONE 2026-08-31: conjunct removed; comment rewritten (COR-R5-1 + COR-R5-2
documented in the predicate's own bullets); `settlements()` read lifted to
a reusable binding; `_completion` discard documents that the completion's
VALUE no longer gates the no-op while its side effects always run.
`just check-package gauntlet-app` PASS (cor-r5-check-app1.log).

ITEM-2 COR-R5-2 service pin (settle/mod.rs)
STARTED 2026-08-31: adding SettlementService::pin_missing_settlement_rows.
DONE 2026-08-31: per-row pin, class `settlement_row_unrecoverable`, naming
run + missing row's unit + anchor-bounded replay fact + the
checked-against-the-store posture; no byte-presence or event-position
claims beyond the anchor fact. Controller calls it inside the no-op branch
only, mapping SettleError like the completion block.

ITEM-3 controller tests (FakeState helper + 2 new tests)
STARTED 2026-08-31.
DONE 2026-08-31: `drop_settlement_row` fixture damage helper; test
`a_recovery_completed_settlement_on_a_settled_run_resumes_as_the_settled_noop`
(settlement-arm compound reproduction: row deleted + anchor regressed just
below the event → replay re-inserts at the event's own offset, settled
no-op, zero re-dispatch, no pins, second resume clean); test
`a_settled_noop_over_below_anchor_settlement_damage_pins_the_missing_row`
(below-anchor damage → exactly one pin naming the damaged unit, sibling
never named, row honestly stays missing, per-resume re-pin honesty).
Fixture-fact discovery, recorded: the close-out checkpoint's anchor sits
ABOVE the settlement events in this fixture (the loop's water-mark tracks
appends after the settle), so the controller tests move the anchor to the
reproduction posture — the same GIVEN-damage class as the shipped COR-R4-1
leg; the natural below-anchor shape is the E2E leg's (real store
geometry). First run failed on the wrong geometry premise (rows stayed 0);
fixed with the anchor move + a precondition assert that fails loudly if
the geometry claim is wrong. All 3 controller tests green incl. the
shipped COR-R4-1 twin (cor-r5-test-ctrl2.log).

ITEM-4 E2E legs 7c/7d + sqlite test_support instruments
STARTED 2026-08-31: `delete_settlement_rows`,
`settlement_committed_events`, `settlement_rows` in the adapter's
doc-hidden test_support; E2E leg 7c (the falsifier's exact reproduction
through the real binary: exit 0, settled, zero booked rounds, zero
re-dispatch, row restored at the event's identity, export delta = exactly
the one settlement record, event/checkpoint/role_invocation segments
byte-identical, second resume whole-scratch byte-identical) and leg 7d
(the natural below-anchor D2 shape: exactly one settlement forensic pin,
sibling silent, row honestly missing, export byte-identical).
DONE 2026-08-31: both legs green (cor-r5-e2e1.log: 7c PASS; 7d first run
FAILED on an over-strict report assertion — discovery: with the keyed row
honestly missing, the resume's report prints `implemented-and-proven 2,
provisional 1`, the honest keyed-surface read, NOT the falsifier's noted
"still prints 3"; assertion corrected to pin the honest shape, reason in
the leg's comment; 7d PASS in cor-r5-e2e2.log; 7c re-verified green).

ITEM-5 lint clean
STARTED 2026-08-31: `just lint`.
DONE 2026-08-31: two clippy rounds fixed in MY new code only —
type_complexity (5-tuple readers -> SettlementEventFacts/SettlementRowFacts
type aliases in test_support), op_ref (my controller-test closures);
rustfmt applied to the touched packages. `just lint` PASS
(cor-r5-lint4.log, rc=0). No gate edited; no shipped assertion touched.

ITEM-6 shipped-test adaptations (settlement-arm full-path pins)
STARTED 2026-08-31: package runs surfaced exactly two shipped tests
pinning the OLD settlement-arm behavior (the reconciliation iteration's
checkpoint) that COR-R5-1 commissioned away:
- a_resume_completes_half_applied_settlements_and_reconciles_the_projection
  (controller.rs): adapted to pin the §5.3.3 event-ordered projection
  (checkpoint word + durable rows above the anchor — what every later
  resume actually derives) carries the owed unit Settled, PLUS a new
  zero-booked-rounds pin (outcome.iterations == first's). Teeth kept:
  exactly-once keyed-row completion, zero re-dispatch.
- p7 a_crash_between_event_and_row_completes_exactly_once_through_resume:
  adapted the same way — the restart writes NO checkpoint (new
  zero-write pin), the §5.3.3 projection carries both units Settled.
  Teeth kept: exactly-once (2 rows), zero re-dispatch,
  assert_recovery_invariants untouched.
Both green (cor-r5-test-ctrl3.log, cor-r5-test-p7.log). No assertion
weakened; the inverted leg is the commissioned behavior change itself,
reasoned at both sites.

ITEM-7 full battery
STARTED 2026-08-31.
DONE 2026-08-31 (all evidence in .agents/reports/task20/cor-r5-*.log):
- just ruler BEFORE edits: PASS (cor-r5-ruler-before.log)
- just check: PASS (cor-r5-bat-check.log)
- just test (workspace): 1375/1375 passed, 0 skipped (cor-r5-bat-test.log;
  +4 vs COR-R4's 1371 = my 2 controller tests + 2 E2E legs)
- just lint: PASS (cor-r5-lint4.log)
- just deny: PASS rc=0 (cor-r5-bat-deny.log)
- just boundaries: 228/228 (cor-r5-bat-boundaries.log)
- just doctor: PASS rc=0 (cor-r5-bat-doctor.log)
- just ruler AFTER edits: PASS, digests match the frozen lock
  (cor-r5-bat-ruler2.log)
- packages: sqlite 50/50, app 474/474, cli 97/97
  (cor-r5-test-{sqlite2,app2,cli2}.log)
- e2e_full_loop_mock + cli_surface: 16/16 (cor-r5-bat-e2e.log)
- repo-root dry-run: exit 0, zero-write (porcelain diff 0 bytes;
  cor-r5-bat-dryrun.log)

COR-R5 COMPLETE. Writing the final report.
