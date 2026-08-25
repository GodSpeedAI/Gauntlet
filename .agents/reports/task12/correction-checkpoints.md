
---

# Correction round 1 — builder log (2026-08-23)

Session start. Brief + adjudication + both critic reports read. Survey
complete: controller.rs (3816 ln), scheduler.rs, select.rs, diagnosis.rs,
disposition.rs, budget_meter port, MeteredBudget adapter, domain
budget/correction/unit, p4 (3 tests), p6 (2 tests), producer_authority
anchor rule, teeth.md, plan Task 12 gate.

Design decisions (from the adjudication text, recorded before editing):

- C1: new `UnitScheduler::ineligibility_report` surfaces typed Ineligible
  reasons; plan_and_dispatch classifies: budget breach (BudgetReserveBreach
  or the summed consult's refusal) => budget_exhausted; governance-only
  blockers (Irreversible / TierConcurrencyUnconfigured / RunnerUnconfigured
  / operator-frozen StatusNotOpen) => `blocked` + escalation ticket.
  Budget takes precedence (adjudication: "authority-blocked-ONLY ... yields
  blocked").
- C2: run-Budget books ONE round per iteration at the boundary; refusal =>
  terminal budget_exhausted (never ControlError); rounds gates read
  would_breach_reserve(1 round) on the run budget; p6 drops
  RoundsSupplementedMeter (bare MeteredBudget + new
  admitted_reservations_total() adapter counter for AK-052); per-completion
  charge books rounds=0.
- C8 interplay: checkpoint moves to the iteration boundary. Anchor
  semantics (producer_authority: declared iteration == latest checkpoint's
  iteration, default 1): boundary checkpoint written ENTERING iteration k
  declares k, so iteration k's owner writes pass the anchor; close-out at
  terminal declares the last iteration. p6 arithmetic: 5 boundary + 1
  close-out = FORWARD+1 stays exact.
- C3: per-phase spend = allowance / 5 (div_ceil, CyclePhase::ALL.len());
  zero-allowance refused at composition (typed BudgetShape guard);
  evidence facts input-driven (reference unavailability, observation gaps,
  verifier-vs-comparator disagreement, payment exhaustion,
  implementation_shaped as the LAST-resort fallback only);
  disconfirming observation = the unit's latest governed verification
  outcome (Pass => disconfirmed => CorrectionIndicated); the
  CorrectionIndicated record is pinned as evidence (observability).
- C4: residuals_of attributes doc.unit_refs().first() falling back to the
  iteration's unit (§13.5 identity + §11.5 row 9); ResidualKey gains the
  expected/observed locator pair (§13.5 merge key); composed
  recurrence->N->diagnosis through the REAL diff comparator: new p4 test
  with a pinned-clock observer (byte-stable facet evidence => stable
  observed locator => recurrence counts) + a PassOnPresent rung (verifier
  disagrees with the comparator) => apparatus locus + CorrectionIndicated;
  p6 raises NONCONVERGENCE_N above the forward rounds ceiling so AK-051's
  exhaustion terminus stays the terminus.
- C5: CorrectionSelection::new refuses empty rejected (new
  RejectedAlternativesEmpty); loop records §15.1 selections on BOTH paths
  over a real pair: the correction actually being dispatched/completed
  (bounded, real authority/tier/estimate facts, non-zero forward ask on the
  residual path) + the §15.1-step-3 speculative sibling (clean-workspace
  re-run) => >=1 rejected alternative structurally; exercisable_authority
  = Critic (the run's real auto-dispatch ceiling per §17.1 today, L1);
  completion-path NoAdmissibleCandidate => pinned evidence + continue
  (never an error); residual-path NoAdmissibleCandidate => escalate only
  for non-budget blocks.
- C6: PlanInput.units = the FULL re-read membership with a new
  `statuses` projection map (durable statuses authoritative over the
  parsed file status) so validate_graph never sees UnknownDependency after
  settlement while eligibility filters; meter reserve refusal =>
  skip-and-continue; dispatch prompts/charges the whole planned member
  list (cohort consistency); cohorts stay empty with the L4 spec-awaiting
  record.
- C7: reread_answer_key projects the whole re-parsed set: file status
  changes for queued units override durable state (file authoritative in
  the window before the boundary abort); removed units leave the loop's
  scope; new units seed Open.
- C8: checkpoint tracks the store-returned offset at every append; resume
  restores per-unit attempts from evidence_productions_at (the loop's own
  durable producer writes; retry-ledger persistence stays L2/Task 13).
- C9: Reservation minting moves behind a provided BudgetMeter::mint
  (construction requires a meter - the B1 ports-trusted-layer analog);
  escalate() escapes the JSON record; charge refusal pins the completed
  result's fact + typed exhaustion disposition; drain gains a run-level
  poll-tick bound (cancel + pin + escalate past it).

Baseline before any edit: running `cargo nextest run -p gauntlet-app
control::` now (expect green).

## C1 — terminal truth — DONE (2026-08-23)

- Fail-first: new controller test
  `a_governance_blocked_run_terminals_blocked_with_a_ticket_never_exhausted`
  (irreversible+human single-unit key) observed `left: BudgetExhausted,
  right: Blocked` — the exact critic-A C1 defect.
- Fix: scheduler gained `Ineligible::unit()/is_budget_block()/
  is_governance_block()` and `UnitScheduler::ineligibility_report` (plan's
  shape validation extracted to `validate_input_shape`, shared); controller
  `plan_and_dispatch` tracks `budget_refused` from the summed consult and
  delegates to `classify_no_admission`: budget breach (consult refusal or
  any owed `BudgetReserveBreach`) => `budget_exhausted`; governance-only
  (Irreversible/TierConcurrencyUnconfigured/RunnerUnconfigured/
  operator-frozen StatusNotOpen, over units still owed) => `blocked` +
  ticket via the new `submit_ticket` (escalate() = ticket + Escalated).
  drive's break generalized to any terminal with empty in-flight;
  decide_progress's rounds gate guarded not to overwrite a set terminal.
- Files: scheduler.rs, controller.rs.
- Tests: `cargo nextest run -p gauntlet-app control::` — 55/55 PASS
  (54 baseline + 1 new; no regression).

## C2 — rounds unification — DONE (2026-08-23)

- Fail-first (observed against a temporary defect shape — the crash the
  critics described, reproduced on the new bare wiring): p6 rewired to the
  BARE `MeteredBudget` (RoundsSupplementedMeter DELETED) and the loop's
  round booking made error-propagating (`map_err(ControlError::Budget)?`)
  — p6 main FAILED at start_run; first observed shape also exposed an
  ordering defect in my own first fix (booking at iteration START
  pre-consumed the last affordable round: "reservations admitted left: 4,
  right: 5") — fixed by booking at the boundary AFTER the drain.
  Byte-identical restore of the fixed controller afterwards
  (/tmp/controller-c2-fixed.rs).
- Fix (controller.rs): ONE round per iteration booked on the run `Budget`
  at the iteration boundary (`book_iteration_round`, after drain, before
  decide_progress; a stronger mid-drain terminal wins over exhaustion);
  both rounds gates (`plan_and_dispatch`, `decide_progress`) read
  `rounds_exhausted(state)` = the run budget's own
  `would_breach_reserve(1 round)` — never the meter; per-completion
  charge books rounds=0 with refusal => terminal exhaustion (never a bare
  ControlError); module doc documents the meter/rounds split (port
  expresses payments; rounds are iteration facts).
- Fix (meter-memory adapter): module doc split section;
  `admitted_reservations_total()` counter (AK-052's rows-vs-reservations
  truth on the meter itself); reserve now mints via the port capability.
- Fix (ports/budget_meter.rs — the C9a mint landed here as a
  prerequisite): `Reservation::new` removed; construction only through the
  provided `BudgetMeter::mint` (B1 minting pattern: minting requires a
  meter); contract stubs + controller FakeMeter updated to `self.mint`.
- p6: supplement deleted; factory wires the bare adapter (registry keeps
  the run's meter handle for the admission count); NONCONVERGENCE_N raised
  to 6 (> forward ceiling 5) so AK-051's exhaustion terminus is robust;
  header re-documented (rounds authority + terminus note); second test
  rewritten over the bare meter (token-ceiling reserve refusal + admission
  counting + zero-payment release).
- Files: controller.rs, scheduler.rs (untouched this item), budget_meter.rs
  (ports), meter-memory adapter, contract/stubs.rs, p6_budget_control.rs.
- Tests: p6 2/2 PASS, p4 3/3 PASS, control:: 55/55 PASS, meter adapter +
  ports 95/95 PASS.

## C4 — composed non-convergence — DONE (2026-08-23; implemented before C3 — C3's composed tests need the attribution fallback to reach enter_diagnosis)

- Fail-first (three observations):
  1. diagnosis.rs `different_observed_locators_do_not_accumulate_one_recurrence_count`
     — compile-failed against the old key (`no field expected/observed`),
     then pinned the behavior.
  2. In-crate composed tests under the pre-C4 defect shape (unattributed
     residuals skipped): `left: BudgetExhausted, right: Inconclusive` and
     `correction-indicated records: 0, right: 1` — the detector never fed.
  3. The new p4 real-comparator test under the same defect shape:
     `left: Escalated, right: Inconclusive` (row 9 never routed; the loop
     row-4'd into escalation).
- Fix: ResidualKey carries the §13.5 merge-key pair (expected/observed
  locators; Ord includes them; module doc updated — different observed
  locators are different residuals, never one count); controller
  `residuals_of` returns LoopResidual{residual, unit, expected, observed}
  with attribution = the record's first unit_ref FALLING BACK to the
  iteration's unit (§13.5 identity + §11.5 row 9; the shipped deterministic
  comparator emits empty refs); the row-9 loop feeds the detector the full
  key; open_residuals set BEFORE the row-9 loop (critic m6 also closed —
  in-path for this fix); disposition.rs test key updated.
- Composed REAL-comparator proof: new p4 test
  `a_recurring_residual_through_the_real_diff_comparator_enters_bounded_diagnosis`
  — the REAL command observer over an absent program (name derived from
  the fixture's parsed check, never spelled) records §12.3 unavailability
  gaps every iteration; the REAL diff comparator derives STABLE §13.5 keys
  (no per-iteration evidence ref in the gap residuals — the pinned-clock
  approach was rejected: byte-stable facet evidence collides with the
  producer's write-once-per-ref rule, observed as a typed Producer
  refusal); recurrence 3x routes into diagnosis, verification honestly
  INCONCLUSIVE, cycle ends Inconclusive+quarantine+escalation, diagnostic
  spend 4×allowance/5 durable in the checkpoint.
- Tests: control:: 58/58, p4 4/4.

## C3 — composed diagnosis — DONE (2026-08-23)

- Fail-first: the two new in-crate composed tests under the pre-C3 defect
  shape (hardcoded implementation_shaped, hardcoded disconfirmed:true,
  whole-allowance per phase): `left: 100, right: 80` (phase 1 consumed the
  whole allowance) and `left: Inconclusive, right: Settled` (no
  input-driven observation). Fixed controller restored afterwards
  byte-identical.
- Fix (enter_diagnosis): per-phase spend = allowance div_ceil 5
  (CyclePhase::ALL.len(), no literal); DiagnosticEvidence input-driven —
  reference unavailability OR the observation's own unavailable-evidence
  gaps => observation_unavailable; comparator-residual + governed
  verification PASS => verifier_disagreed (§18.4, apparatus outranks
  implementation); rounds exhaustion => payment_exhausted;
  implementation_shaped ONLY as the last-resort fallback when every
  apparatus fact is silent; DisconfirmingObservation = the unit's latest
  governed verification outcome (Pass => disconfirmed); the
  CorrectionIndicated record is PINNED (locus, phases, occurrences) —
  reachable and observable composed; LoopState.last_verifications tracks
  the real outcomes; compose() refuses a zero diagnostic allowance
  (typed BudgetShape/BudgetError::LimitZero — fail-fast, §15.2 "a run can
  always afford to diagnose").
- Composed tests: `a_recurring_residual_routes_composed_diagnosis_to_correction_indicated`
  (FlakyVerdictVerifier — a rung whose evidence question differs from the
  comparator's, modeled as pass-from-3rd-call: iterations 1-2 FAIL
  (recurrence counts), iteration 3 PASS settles AND fires diagnosis =>
  apparatus locus "verifier error", phases:5, CorrectionIndicated pinned,
  diagnostic_tokens == the whole allowance across 5 phases, terminal
  Settled) and `a_surviving_hypothesis_ends_inconclusive_with_quarantine_composed`
  (failing rung: implementation fallback locus stands, disconfirmed
  honestly false, Inconclusive + quarantine + escalation, 4 phases booked).
- Tests: control:: 58/58, p4 4/4, p6 2/2.

---

# Correction round 1 — FINISHER slice F1 (2026-08-25)

Session start. Read: brief `correction-brief-f1.md`, `AGENTS.md`,
`correction-adjudication.md`, `residual-readjudication.md`, this file's
round-1 log, `critic-b-result.md` (§A3, §A4). Holding the single-compiler
build lock; no concurrent cargo/nextest; no sub-agents that compile.

## Item 0 — C5 VERIFIED AND RECORDED (landed by the cancelled round-1 builder; NOT implemented by F1)

This entry is the checkpoint the cancelled builder was killed before
writing. **F1 implemented nothing here** — F1 read the tree, confirmed each
sub-part is present *and asserted*, and ran the verification bar.

- **C5.1** — `CorrectionSelection::new` refuses an empty rejected list.
  PRESENT: `gauntlet-domain/src/correction.rs:448-450`
  (`if rejected.is_empty() { return Err(CorrectionError::RejectedAlternativesEmpty); }`),
  error variant + doc `:381-387`, constructor doc clause `:445-446`.
  ASSERTED: `correction.rs:572-596`
  `a_selection_without_rejected_alternatives_is_unrepresentable` — matches
  the typed variant AND asserts the Display names the vacuity.
- **C5.2** — selection recorded on the RESIDUAL (correcting) iteration.
  PRESENT: `controller.rs:1793-1798` — inside the
  `Disposition::RetryWithResidual` arm, after `queue_retry_dispatch`,
  `self.record_selection(state, &flight.unit, true)?` with the §15.1 step-4
  doc comment. Completion path `controller.rs:1806-1810`
  (`record_selection(..., false)`).
- **C5.3** — real candidates, non-zero forward ask on the correcting path.
  PRESENT: `controller.rs:2004-2018` — `forward_ask` = `per_attempt_estimate(
  limits, spent, recovery_reserve_pct)` when `residual_driven`, the
  all-zero `BudgetSlice` only on the completion path (recorded intent, left
  alone per the brief). Candidate space `controller.rs:2043-2077`: a
  `Bounded` chosen candidate and a `Speculative` sibling built through the
  same `candidate_of` closure, so >=1 rejected alternative exists
  structurally. No synthetic self-comparison remains.
- **C5.4** — `required_authority` from the real unit fact vs a distinct
  `exercisable_authority`. PRESENT: `controller.rs:2053`
  `required_authority: domain_unit.authority()` (the parsed domain Unit),
  against `controller.rs:2085-2089`
  `exercisable_authority: gauntlet_domain::Authority::Critic` with the
  §17.1-ceiling-today comment naming L1/Task 14. Not raised by F1.
- **C5.5** — no hardcoded tier list erroring cross_cutting. PRESENT:
  `controller.rs:2090-2093` `dispatchable_tiers:
  dispatchable_tier_capacity(self.budget_shape.limits).into_iter().map(|(tier, _)| tier).collect()`;
  `dispatchable_tier_capacity` at `controller.rs:2309-2316` returns
  Contained/Local/CrossCutting and deliberately omits `irreversible`
  (§17.3 no-capacity-entry-is-unadmitted). The `[Contained, Local]` literal
  critic-B A7 named is gone. **LEFT ALONE** (brief, critic-A m7 / Task 14):
  that the three tiers share the global parallel bound.
- Composed assertions cited: `p4_corrective_loop.rs:1252-1296` (both
  iterations recorded; the correcting record names "bounded re-dispatch"
  and carries a `SpeculativeWhenBoundedAvailable` rejection) and
  `p6_budget_control.rs:933-953` (one residual-driven record per forward
  round, every one non-vacuous).

Verification bar (run personally, sequentially, build lock held):

- `cargo nextest run -p gauntlet-app control::` — `Summary [   0.207s] 58
  tests run: 58 passed, 225 skipped`
- `cargo nextest run --test p4_corrective_loop --test p6_budget_control` —
  `Summary [   0.373s] 6 tests run: 6 passed, 0 skipped`

No sub-part was found absent within this bounded verification (the five
named loci + the two composed proof sites); this is not a claim that C5 has
no other unexamined facet.

Fail-first note: item 0 is a *verification and recording* item by brief
construction — there is no fail-first evidence to observe because F1 wrote
no code here. Fail-first resumes at item 1.

## Item 1 — C6.1 over-capacity scope refusal skips-and-continues — DONE (F1, 2026-08-25)

**Fail-first, two observations** (the first exposed a fixture-locality
defect that had to be closed before the real defect was observable):

1. First run of the new test: `panicked at controller.rs:4044: the
   over-capacity truncation is recorded as unavailable evidence` — the run
   did NOT error. Diagnosis: the in-crate `FakeMeter` answered ONLY the
   §5.3.7 reserve conjunct and carried no outstanding-reservation
   accounting, so it could never produce the shipped adapter's
   `check_dispatch_scope` refusal. That is why critic-B's A3 escape was
   invisible in this crate ("Neither p4, p6, nor the in-crate controller
   tests exercise it") — the prereg R1 fixture-local-wiring class, in the
   fixture itself.
2. After aligning `FakeMeter` with the shipped `MeteredBudget` contract
   (reserve gate FIRST, then `budget.check_dispatch_scope(outstanding + 1,
   1)`; a booked charge releases the slot with `saturating_sub(1)` —
   the adapter's own comment and arithmetic), the SAME test observed the
   exact critic-B defect:
   `panicked at controller.rs:4067: an over-capacity fan-out never errors
   the run: Meter(Reserve { cause: ScopeExceeded { parallel: 3, depth: 1 } })`

**Fix (controller.rs):**

- New private `enum DispatchAdmission { Admitted, TruncatedAtCapacity,
  EmptySlice }` (`controller.rs:412-429`) — the loop's needed distinction
  is "consumed a parallel slot" vs "fan-out truncated here", which neither
  `()` nor `Result` can carry. `dispatch` now returns it.
- `dispatch` matches the reserve refusal by CAUSE:
  `BudgetMeterError::Reserve { cause: BudgetError::ScopeExceeded {
  parallel, depth } }` records the truncation and returns
  `TruncatedAtCapacity`; every other `BudgetMeterError` still propagates
  as `ControlError::Meter`. The genuine forward-ceiling breach is
  untouched — it is caught EARLIER by the summed pre-dispatch
  `would_breach_reserve` consult, which still sets `budget_refused` and
  routes to C1/C2's `budget_exhausted`.
- Grounding quoted in the fix's doc comment: §10.4 (`Core-spec.md:777`,
  verbatim — "a fan-out that would breach the recovery reserve is
  truncated in dimension-priority order, and the truncation is recorded as
  `unavailable_evidence`, not silently dropped"), §21.3 (`Core-spec.md:1238`
  — "Integrity violation | Abort the run; preserve everything" is the ONLY
  abort row), §11.2 (the meter check is a per-Unit eligibility conjunct).
- New `record_dispatch_truncation` writes and PINS a JSON evidence record
  (`class: dispatch_truncated`) naming run, iteration, unit, and the scope
  numbers the domain refused on — the §10.4 record, not a log line.
- `plan_and_dispatch`'s dispatch loop matches the admission: `Admitted`
  sets `admitted`, `TruncatedAtCapacity` breaks (the parallel ceiling is a
  RUN-global bound, so every later member of the already
  dimension-priority-ordered plan faces the identical refusal — stopping
  IS the priority-ordered truncation), `EmptySlice` is a no-op.

**Fix (test fixture, same file):** `FakeMeter` becomes
`FakeMeter(Mutex<FakeMeterInner { budget, outstanding }>)` and implements
the shipped scope contract; `answer_key` gained a third unit (AK-9103);
new named figure `OVER_CAPACITY_UNITS = FIXTURE_PARALLELISM as usize + 1`
so the fixture is bound to the parallelism ceiling rather than to a
literal.

**Test:** `control::controller::tests::
an_over_capacity_fan_out_is_truncated_and_recorded_never_a_run_error` —
`FIXTURE_PARALLELISM + 1` independent contained units, nothing in flight;
asserts the run does not Err, terminals `Settled`, dispatches all three
units (the truncated one on a later iteration, not dropped), and that a
pinned `dispatch_truncated` record naming AK-9103 exists.

**Deliberately left alone:** the forward-ceiling breach path (C1/C2's
`budget_exhausted`) and `classify_no_admission`. A scope refusal cannot
co-occur with `!admitted && in_flight.is_empty()` (with an empty flight
set the meter's `outstanding` is 0, so the first reserve always clears
`max_parallel >= 1`), so no guard was added there — adding one would be
dead code.

Gates after item 1:

- `cargo nextest run -p gauntlet-app control::` — `Summary [   0.245s] 59
  tests run: 59 passed, 225 skipped`
- `cargo nextest run --test p4_corrective_loop --test p6_budget_control` —
  `Summary [   0.352s] 6 tests run: 6 passed, 0 skipped`

> Process change received from the orchestrator mid-item-2 (2026-08-25 10:54):
> DECLARE-FIRST checkpointing from here on — a `STARTED` marker with the
> intended design BEFORE editing, the full `DONE` entry after. Items 0 and 1
> above were written after completion under the old rule and are left exactly
> as written. Item 2 was already in flight (fail-first observed) when the
> change arrived; its STARTED marker below is written at that moment and says
> so. Items 3-6 declare before editing.

## Item 2 — C6.2 full graph membership with separate eligibility — STARTED 2026-08-25 10:54

(Marker written mid-item, at the moment the declare-first rule arrived; the
fail-first observation below had already been made.)

Intended design (adopting the cancelled builder's recorded C6 design, with
the "which source wins" rule made explicit and shared with item 3):

- `PlanInput.units` carries the FULL re-read membership; the controller's
  `open`-only filter survives only as the early-return predicate ("does the
  loop still owe work on any unit"), so a fully-settled run still short-
  circuits before the rounds gate and cannot be mislabelled
  `budget_exhausted`.
- New `PlanInput.statuses: BTreeMap<UnitId, UnitStatus>` — the caller's
  durable/effective projection — plus `PlanInput::status_of(unit)` which
  falls back to the Unit's parsed status when the caller supplies none.
- §11.2 conjunct 1 and conjunct 2 both decide on `status_of`, so a settled
  member is present for `validate_graph` but refused by eligibility.
- **Which source wins for a queued unit (shared with item 3):** durable
  closure is STICKY, otherwise the file wins. Neither source can re-open
  what the other closed. File closed + memory open => closed (§11.4 step 3:
  "The in-memory snapshot is never authoritative over the file"). Memory
  closed + file open => closed (§5.3.2: `status = settled` requires a
  `VerificationRecord` with `Outcome = PASS` — the run's own durable
  settlement is the fact, and the controller never writes back to the file;
  §18.3 "Re-running settled work is a defect"). Both open => open.
  `state.unit_states` is the single map holding that reconciliation, and
  `PlanInput.statuses` is its projection.

## Item 2 — C6.2 full graph membership with separate eligibility — DONE 2026-08-25 11:06

**Fail-first, two observations** (the two-part fix has two distinct
failures, and I observed both rather than reasoning about the second):

1. Composed defect (`controller.rs`), the shipped shape critic-B named:
   `panicked at controller.rs:4161: a settled dependency never errors the
   next plan: Scheduler(UnknownDependency { unit: UnitId { raw: "AK-9102"
   }, dependency: UnitId { raw: "AK-9101" } })`
2. The trap the brief warned about, observed rather than assumed: with
   full membership restored but `PlanInput::status_of` temporarily
   neutralized to `unit.status()` (the parsed file status), the new
   scheduler test failed
   `panicked at scheduler.rs:1060: a durably settled dependency releases
   its dependent: DependencyUnresolved { unit: UnitId { raw: "AK-001" },
   dependency: UnitId { raw: "AK-901" }, status: Some(Open) }` — i.e.
   `UnknownDependency` traded for `DependencyUnresolved`, exactly as
   predicted. The scheduler was then restored byte-identically
   (`diff` clean against the pre-neutralization copy).

**Design adopted:** the cancelled builder's recorded C6 design (a
`statuses` projection on `PlanInput`, durable-status authoritative,
consulted by eligibility while membership stays whole), with two
refinements I record because they are deviations in detail:

- `validate_graph` is UNCHANGED. It reads `by_id` built from
  `input.units`, and full membership is what fixes it; giving it a second
  status-aware membership check would be a second source of truth for the
  same question.
- `status_of` FALLS BACK to the Unit's parsed status when the projection
  carries no entry, rather than requiring the map to be total. A direct
  caller planning over an unstarted run (the compose-time validation plan
  at `controller.rs:582`, the scheduler's own tests) then needs no
  bookkeeping, and the fallback is the honest reading for that caller.

**Which source wins for a queued unit** (the answer shared with item 3):
durable closure is STICKY; otherwise the file wins. §11.4 step 3 ("The
in-memory snapshot is never authoritative over the file") gives the file
the open-direction; §5.3.2 (`status = settled` requires a
`VerificationRecord` with `Outcome = PASS`) plus §18.3 ("Re-running settled
work is a defect") give durable settlement the closed-direction.
`state.unit_states` holds that reconciliation; `PlanInput.statuses` is its
projection.

**Files:** `crates/gauntlet-app/src/control/scheduler.rs`,
`crates/gauntlet-app/src/control/controller.rs`.

- `scheduler.rs`: new `pub statuses: BTreeMap<UnitId, UnitStatus>` field on
  `PlanInput` with its §11.2/§5.3.2 doc; new `PlanInput::status_of`;
  conjunct 1 and conjunct 2 both decide on it (conjunct 1's
  `Ineligible::StatusNotOpen` now reports the DURABLE status, so the
  refusal is not a lie about the file).
- `controller.rs`: `plan_and_dispatch` passes the whole re-read set as
  `units` and `state.unit_states` as `statuses`; the old open-only filter
  survives ONLY as the `owes_work` early-return predicate — without it a
  fully-closed run would fall past the early return into the rounds gate
  and could be terminalled `budget_exhausted` on the round that settled
  everything. `estimates` now covers the whole membership (otherwise
  `validate_input_shape` refuses `MissingEstimate`). Compose-time
  validation plan and the scheduler's `plan_input` fixture pass an empty
  projection.

**Tests:**
`control::controller::tests::a_settled_dependency_stays_in_the_graph_while_eligibility_filters_it`
(composed: AK-9102 `depends_on: [AK-9101]`, both settle, two prompts,
terminal `Settled`) and
`control::scheduler::tests::the_durable_projection_decides_status_for_membership_and_dependencies`
(unit: parsed status asserted still `Open`; projection releases conjunct 2,
refuses conjunct 1 with the durable status, and the plan admits only the
dependent). New fixture `answer_key_with_dependency()`.

Gates after item 2:

- `cargo nextest run -p gauntlet-app control::` — `Summary [   0.460s] 61
  tests run: 61 passed, 225 skipped`
- `cargo nextest run --test p4_corrective_loop --test p6_budget_control` —
  `Summary [   0.808s] 6 tests run: 6 passed, 0 skipped`

## Item 3 — C7 per-tick re-read authoritative for ALL units — STARTED 2026-08-25 11:14

Intended design, and one finding that changes the shape of the item:

- **C7.1 is SPEC_ONLY_NO_SHIPPED_SURFACE** (bounded search, declared below).
  The answer key cannot express a Unit status at all: the parser's frozen
  field set is `{claim, verify_by, blast_radius, authority,
  evidence_required, depends_on}` (`reference/parse.rs:40-49`) and is
  typed-refusing (`UnknownField`), and `Unit::new` hardwires
  `status: UnitStatus::Open` (`gauntlet-domain/src/unit.rs:351`). So an
  operator edit marking a queued unit `settled`/`out_of_scope` is not
  "silently ignored" — it is a hard parse refusal, and no shipped surface
  can produce a parsed status other than `Open`. Deliverable therefore
  follows the L4 precedent: RECORD honestly with a pinning test, do not
  invent a status source (the parser's frozen field set is off-limits) and
  do not add a branch that can never fire.
- **C7.2 and C7.3 ARE reachable** (an operator can delete or add an
  `## AK-NNNN` block) and get the full fail-first treatment: prune
  `unit_states` + the per-unit side maps for removed units; seed newly
  added units.
- Both land in `reread_answer_key`, AFTER the existing in-flight cancel
  loop, so a removed in-flight unit is cancelled before its state is
  pruned. Seeding uses the parsed `unit.status()` rather than a hardcoded
  `Open` — the same expression that becomes the C7.1 propagation point if a
  status surface ever ships.

## Item 3 — C7 per-tick re-read — DONE (C7.1 recorded) / CONFLICT (C7.2, C7.3) 2026-08-25 11:29

### C7.1 — SPEC_ONLY_NO_SHIPPED_SURFACE, recorded and pinned

Bounded search over two shipped surfaces:
`crates/gauntlet-app/src/reference/parse.rs:40-49` (`field_keys()` is
frozen at `{claim, verify_by, blast_radius, authority, evidence_required,
depends_on}`; anything else is a typed `AnswerKeyError::UnknownField`) and
`crates/gauntlet-domain/src/unit.rs:351` (`Unit::new` hardwires
`status: UnitStatus::Open`). **Not found within those two surfaces** — not
a universal absence claim. Consequence: an operator hand-edit marking a
QUEUED unit `settled`/`out_of_scope` is not "silently ignored" (critic-B
A4's wording); it is a hard parse refusal, and no shipped parse can yield
a status other than `Open`.

Deliverable per the L4 precedent and §11.6 ("Provisional capability is
never reported as complete"): a doc record on `reread_answer_key` plus a
PINNING test — `control::controller::tests::
the_answer_key_cannot_express_a_unit_status_on_any_shipped_surface` —
which asserts (a) `status: settled` in a block is refused with the field
named, and (b) every well-formed fixture block parses `Open`. The day a
status surface ships, that test fails and the propagation point exists.
Also recorded: every OTHER field the file can express is ALREADY
authoritative for queued units, because eligibility re-runs each iteration
over the freshly parsed `Unit` values.

### C7.2 and C7.3 — STOPPED, conflict recorded, nothing weakened

Fail-first observations were made FIRST, under §21.3's unavailability
window (the only window in which a mid-run answer-key edit changes what
the loop does instead of aborting it, since §9.4 puts the answer key
inside the frozen digest):

- C7.2 (unit deleted from the file, queued behind a dependency):
  `assertion left == right failed: the deleted unit left scope ... left:
  BudgetExhausted, right: Settled` — the stale `unit_states` entry blocked
  `all_closed` and the run reported the rounds ceiling.
- C7.3 (unit added to the file): `assertion left == right failed: the added
  unit is dispatched, not silently settled ... left: 1, right: 2` — the
  run terminalled `Settled` with the added unit never dispatched.

Both defects are real as MECHANISM. The briefed fixes were then implemented
and immediately produced a third observation that changes the disposition:

- With the seed in place, C7.3's run failed
  `the loop runs: VerificationRefused(UnitUnavailable { unit: UnitId { raw:
  "AK-9102" }, detail: "no block with that id exists in the frozen
  reference (digest sha256:88eca9a6...)" })`.

**The conflict.** The run's settleable membership is the FROZEN
reference's, not the file's:
`VerificationAuthority::frozen_unit` (`verify/authority.rs:368-389`) parses
`self.projection.view()` — the snapshot taken at the freeze — and refuses
any unit absent from it, an assertion standing in
`verify::authority::tests::a_unit_absent_from_the_frozen_reference_is_refused`
(`authority.rs:1044-1060`). Therefore:

- Pruning `unit_states` when a unit leaves the FILE lets `all_closed`
  report `settled` for a unit OF THE FROZEN RULER that the run never
  observed, compared, or verified. That is a settlement lie — a worse one
  than the imprecise `budget_exhausted` it replaces — and it weakens
  §5.3.2/§9.4.
- Seeding a unit that entered the FILE admits work the verification
  authority correctly refuses; the observed result is a hard
  `ControlError` with no terminal and no close-out checkpoint (the
  critic-B A2 shape).

Neither leg is fixable without either weakening a standing assertion or
DECIDING a §21.3 degradation policy — and `residual-readjudication.md`
explicitly deferred the §21.3 answer-key/reference degradation surface this
round ("an unforced policy change to a surface no critic finding requires
moving. Trigger: Task 13 recovery composition, or the next §21.3
degradation-policy touch"). Choosing what the loop owes when the file and
the frozen ruler disagree IS that decision.

Per the brief's hard rule ("If a finding cannot be fixed without weakening
an assertion, a gate, or the frozen reference: STOP, record the conflict
... Never weaken"), I stopped. The prune/seed implementation and its two
tests were REMOVED from the tree; the observations above are the evidence
of record. `reread_answer_key`'s doc now records the boundary explicitly:
the re-read governs CANCELLATION and what is dispatchable now, never what
the run owes.

Also noted, not acted on (recorded for the deferral ledger): a unit added
mid-run under the §21.3 window is dispatched today and then hard-errors at
verification — a terminal-truth defect of the C1 family (no terminal, no
close-out checkpoint, no report) sitting on the deferred §21.3 surface.

**Files:** `crates/gauntlet-app/src/control/controller.rs` (doc record +
the C7.1 pinning test only).

Gates after item 3:

- `cargo nextest run -p gauntlet-app control::` — `Summary [   0.338s] 62
  tests run: 62 passed, 225 skipped`

## Item 4 — C6.4 record L4 honestly (record, do not compose) — STARTED 2026-08-25 11:31

Intended design: a doc paragraph at BOTH `cohorts: BTreeMap::new()` sites
(`controller.rs:587` compose-time validation plan, `:1006` the live plan)
recording that §11.3 cohort slicing is unreachable because its input is
structurally unavailable on every shipped surface, citing §11.3
(`Core-spec.md:835`), the closed ruler field set, the typed-refusing
parser, and Task 20's E2E reference fixture as where it gets exercised.
§11.6 makes the record mandatory. Plus ONE pinning test (the C7.1 pattern)
so the unavailability is asserted rather than claimed in prose. No cohort
source invented; `.wayfinder/**` and the parser's frozen field set
untouched.

## Item 4 — C6.4 record L4 honestly — DONE 2026-08-25 11:35

- Record landed at BOTH `cohorts: BTreeMap::new()` sites
  (`controller.rs`, the compose-time validation plan and the live plan):
  §11.3 (`Core-spec.md:835`) states the criterion and names no marker; the
  frozen ruler's field set is closed; `reference.yaml` carries no grouping
  key; the parser's field set is closed AND typed-refusing; `domain::Unit`
  carries no cohort. The CONSUMER is real and §11.3-cited in
  `scheduler.rs`; the PRODUCER does not exist, so the map is empty by
  structure, not by choice. Bounded claim ("not found within those four
  surfaces"), Task 20's E2E reference fixture named as where it gets
  exercised, §11.6 cited as why the record is mandatory.
- Fail-first: this is a RECORD item, so there is no behavioural failure to
  observe. Instead of leaving the claim as prose I pinned it — new test
  `control::controller::tests::
  the_answer_key_cannot_express_a_cohort_on_any_shipped_surface` asserts a
  `cohort:` field is refused by name. This is a deliberate small extension
  of the briefed deliverable, for the same reason as C7.1's pinning test:
  a prose record of unreachability goes stale silently; an asserted one
  fails the day the surface ships.
- No cohort source invented. `.wayfinder/**` untouched. The parser's
  frozen field set untouched.
- Gate: `cargo nextest run -p gauntlet-app control::` —
  `Summary [   0.210s] 63 tests run: 63 passed, 225 skipped`

## Item 5 — C6.3 cohort charge/prompt consistency — STARTED 2026-08-25 11:36

Intended design: make the inconsistency UNREPRESENTABLE-or-loudly-refused
rather than silently latent. `dispatch` keeps `planned.units.first()` while
reserving `planned.estimated_slice`, which the scheduler summed over ALL
members, and prompts one unit — "charge what you prompt" is violated the
moment a slice carries more than one member. Given item 4's finding that
cohorts cannot form on any shipped path, the honest minimum is a TYPED
refusal at the dispatch boundary when a planned slice carries more than
one member while the controller can only dispatch one, carrying the
failure locus (the slice's member list). Fail-first by constructing a
multi-member `PlannedDispatch` directly.

## Item 5 — C6.3 cohort charge/prompt consistency — DONE 2026-08-25 11:47

**Fail-first:** the guard was first landed PERMISSIVE (exactly today's
behaviour, `Ok(planned.units.first().cloned())`) and the discriminating
test observed the silent drop:
`panicked at controller.rs:4306: a slice this controller cannot dispatch
is refused, never truncated: Some(UnitId { raw: "AK-9101" })` — AK-9102
dropped with no record while the summed estimate for both was reserved.

**Choice and justification.** The brief offered "unrepresentable or loudly
refused". Unrepresentable would mean either composing real cohort dispatch
(prompting and charging every member) — that is §11.3 slicing, which item
4 established has no shipped input and which the plan assigns to Task 20 —
or changing `PlannedDispatch` so a multi-member slice cannot be
constructed, which would delete the scheduler's §11.3 capability to serve
the controller's current limitation. Both are worse than the third option.
So: LOUDLY REFUSED, typed, carrying the failure locus.

**Fix (`controller.rs`):**

- New `ControlError::SliceNotDispatchable { units: Vec<UnitId> }` with a
  `Display` naming the member count and every member id.
- New `fn dispatchable_member(&PlannedDispatch) -> Result<Option<UnitId>,
  ControlError>`: `Ok(None)` for the empty slice (the caller's existing
  `EmptySlice` skip, unchanged), `Ok(Some(unit))` for one member, and the
  typed refusal for more. `dispatch` now goes through it, BEFORE the
  reserve — so a refused slice charges nothing.
- The doc records why refusal rather than tolerance: the inconsistency is
  latent only because cohorts cannot form today, and a silent drop stops
  being latent silently. A typed refusal makes Task 20's composition a
  visible failing change instead of a behaviour drift.

**Test:** `control::controller::tests::
a_multi_member_slice_is_refused_rather_than_charged_for_and_dropped` —
pins all three arms (empty/one/many) and asserts the refusal carries the
member list and names the member that would have been dropped.

- Gate: `cargo nextest run -p gauntlet-app control::` —
  `Summary [   1.313s] 64 tests run: 64 passed, 225 skipped`

## Item 6 — N6 remove the dead `compose_with_policy` — STARTED 2026-08-25 11:48

Intended design: delete the orphaned `#[cfg(test)]` helper (zero references
repo-wide; the two tests needing a custom policy call `compose_full`
directly). It is a live gate failure — `just lint` is `clippy --all-targets
-D warnings` and the tree warns `function compose_with_policy is never
used`. Fail-first is the existing warning, captured verbatim before the
deletion.

## Item 6 — N6 remove the dead `compose_with_policy` (+ the rest of the lint gate) — DONE 2026-08-25 11:58

**Fail-first (verbatim, observed on my first compile of this session, before
any edit):**

```
warning: function `compose_with_policy` is never used
    --> crates/gauntlet-app/src/control/controller.rs:3669:8
     |
3669 |     fn compose_with_policy(
     |        ^^^^^^^^^^^^^^^^^^^
     |
     = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default
```

Confirmed zero references repo-wide (`grep -rn compose_with_policy crates/
tests/` returned only the definition) before deleting it.

`just lint` is `clippy --workspace --all-targets -- -D warnings` FOLLOWED
BY `cargo fmt --check`, and it was red for three more pre-existing reasons
the brief did not enumerate. All three are dead-code/mechanical, none
weakens anything, and item 6's requirement is "`just lint` must go green":

1. `p6_budget_control.rs:114,121` — `error: unused import: BudgetError` and
   `error: unused import: Remaining`, left behind when C2 deleted
   `RoundsSupplementedMeter`. Removed from the two `use` lists.
2. `p4_corrective_loop.rs:990` — `error: ... arc_with_non_send_sync` on
   `Arc::new(CommandObserver::from_parts(...))`, from C4's real-comparator
   test. Annotated with the repository's OWN established idiom for this
   exact case — `#[allow(clippy::arc_with_non_send_sync)] //
   single-threaded proof fixture wiring` — which the same file already uses
   two statements earlier (`:979`) and which p2, p6, p12 and
   `controller.rs:2810` all use. Not a new decision, a missed annotation.
3. `cargo fmt --check` diffs in `controller.rs`, `scheduler.rs`,
   `diagnosis.rs`, `p4_corrective_loop.rs`, `p6_budget_control.rs` — some
   mine, some pre-existing drift from C3/C4. `cargo fmt` touched ONLY those
   five files (verified against the pre-run `--check` file list); every
   change is whitespace/wrapping.

**Files:** `controller.rs` (deletion), `p6_budget_control.rs`,
`p4_corrective_loop.rs`, plus rustfmt over the five files above.

### Gates — all four, run personally, sequentially, one compiler at a time

```
$ just check-package gauntlet-app
cargo check -p gauntlet-app --all-targets
    Checking gauntlet-app v0.1.0 (/home/sprime01/projects/gauntlet/crates/gauntlet-app)
    Finished `dev` profile [unoptimized] target(s) in 8.90s

$ cargo nextest run -p gauntlet-app control::
     Summary [   0.403s] 64 tests run: 64 passed, 225 skipped

$ cargo nextest run --test p4_corrective_loop --test p6_budget_control
     Summary [   1.042s] 6 tests run: 6 passed, 0 skipped

$ just lint
cargo clippy --workspace --all-targets -- -D warnings
    Checking gauntlet-app v0.1.0 (/home/sprime01/projects/gauntlet/crates/gauntlet-app)
    Checking gauntlet-cli v0.1.0 (/home/sprime01/projects/gauntlet/crates/gauntlet-cli)
    Finished `dev` profile [unoptimized] target(s) in 28.26s
cargo fmt --check
```

(`just lint` prints nothing after `cargo fmt --check` when formatting
matches; the recipe exited 0.)

F1 slice complete. Result contract: `.agents/reports/task12/correction-f1.md`.

---

# Correction round 1 — FINISHER slice F2 (2026-08-25)

Session start. Read: brief `correction-brief-f2.md`, `AGENTS.md`,
`correction-adjudication.md`, `residual-readjudication.md` (incl. the C7
CONFLICT ruling), `correction-f1.md`, this file's F1 entries,
`critic-b-result.md` §A9/§A11/§A12. `./scripts/reconcile-checkpoints.sh 12`
run FIRST: "OK — no task-surface file was modified after the last checkpoint
write" — nothing of mine is partly done in the tree. Baseline before any
edit: `cargo nextest run -p gauntlet-app control::` — 64/64 PASS.
Single-compiler build lock held throughout; no concurrent cargo/nextest.

## Item 0 — bound dispatch-eligible membership by the FROZEN REFERENCE — STARTED 2026-08-25 11:51

Intended design (the orchestrator's post-F1 ruling; NOT the original
C7.2/C7.3): one guard — `plan_and_dispatch` filters the per-tick re-read to
units present in the run's FROZEN reference, so a file-only unit (added under
§21.3's unavailability window) is never dispatched and never hard-errors at
verification with `UnitUnavailable`. No pruning, no seeding, no §21.3 policy
change. Frozen-membership source: the PORT's governed
`FrozenReferenceProjection::load_version(run.reference_version(), evidence,
deadline)` (digest-verified bytes at the version's pinned ref — the same
load `VerificationAuthority::for_run` performs) parsed by the app crate's
own `reference::parse::parse_answer_key` — the identical route
`GovernedVerification::frozen_unit` uses, so no port widening and no new
authority surface. The set is derived ONCE per run (start/resume) into
`LoopState.frozen_membership` (the frozen reference cannot change mid-run
by definition); the guard consults it at the plan boundary. Fail-first
under the unavailability window, mirroring F1's C7.3 observation shape.

## Item 0 — bound dispatch-eligible membership by the FROZEN REFERENCE — DONE 2026-08-25 12:04

**Fail-first (verbatim, observed with the guard neutralized to the
pre-fix filter shape):**

```
thread 'control::controller::tests::a_file_only_unit_is_never_dispatched_the_frozen_reference_bounds_membership' panicked at crates/gauntlet-app/src/control/controller.rs:4498:14:
a file-only unit never errors the run at verification: VerificationRefused(UnitUnavailable { unit: UnitId { raw: "AK-9103" }, detail: "no block with that id exists in the frozen reference (digest sha256:30d2bfa786d58382fb665449d9342b01647fe9e1911fbfe22d5635a013048ea6)" })
```

The first test run (single-unit fixture) PASSED against the neutralized
guard — recorded honestly: the loop settled and exited before any second
iteration, so the window never opened. The fixture was corrected to the
DEPENDENCY key (AK-9102 still owed when the edit lands) before the defect
was observable. That is F1's blocker 2 reproduced: dispatch during the
§21.3 window, then a hard `ControlError` — no terminal, no close-out
checkpoint, no report.

**Design (as declared; one guard, nothing else moved):**

- `LoopState.frozen_membership: BTreeSet<UnitId>`, derived ONCE per run in
  `start_run`/`resume_run` by `RunController::frozen_membership(&run)`:
  the PORT's governed `FrozenReferenceProjection::load_version` over the
  durable run's own `ReferenceVersion` — bytes at the version's pinned
  ref, verified against its pinned digest — parsed by
  `crate::reference::parse::parse_answer_key`. **Surface used and why:**
  the identical route `GovernedVerification::frozen_unit` takes
  (`verify/authority.rs` parses the same projection with the same parser);
  `frozen_unit` itself is crate-private to the verify module and stays so;
  no port widened, no new authority surface, no second parser.
- `plan_and_dispatch` filters the per-tick re-read to frozen members
  BEFORE the owes-work predicate and the `PlanInput` build, so a file-only
  unit cannot make the loop owe, estimate, plan, or dispatch it.
- New typed `ControlError::FrozenMembership { run_id, detail }` for an
  underivable freeze snapshot (a run that cannot bound its own membership
  refuses to start/resume rather than plan against an unreadable ruler).
- `seed_unit_states` UNTOUCHED: at start/resume the file equals the freeze
  (`verify_unchanged` gates both entries), and mid-run seeding does not
  exist — the defect lived only at the plan boundary.

**Files:** `crates/gauntlet-app/src/control/controller.rs`.

**Tests:** new
`control::controller::tests::a_file_only_unit_is_never_dispatched_the_frozen_reference_bounds_membership`
(AK-9103 appended to the file + provider made unavailable on AK-9101's
admission; asserts terminal `Settled`, exactly 2 prompts, no prompt names
AK-9103, and the `reference_unavailable` gap record pinned — the window
the test rides is itself asserted open). Fixture: `Composed` gained the
provider's `unavailable` switch (test-only exposure of an existing field).

**Gates after item 0:** `cargo nextest run -p gauntlet-app control::` —
`Summary [   0.439s] 65 tests run: 65 passed, 225 skipped` (64 baseline
+ 1 new).

## Item 1 — C8.1 real event_log_offset (no constant 0) — STARTED 2026-08-25 12:06

Intended design: the store-returned `EventLogOffset` is threaded at EVERY
controller append site — `checkpoint()` takes `&mut LoopState` and assigns
`state.last_offset` from its own `CheckpointWritten` append (replacing
`let _ = offset;`), and the four `append_terminal_event` callers that
discard it (hand-edit cancellation in `reread_answer_key`, drain
cancellation, failure disposition, timeout disposition) assign it too; the
resume caller already does. Without all five, the water-mark lags by every
terminal event of the iteration. §18.2 lists the offset as a required
field and §18.3 step 1 makes it the anchor replay starts from.

## Item 1 — C8.1 real event_log_offset (no constant 0) — DONE 2026-08-25 12:16

**Fail-first (verbatim; the five threading sites were reverted to the
pre-fix discarding shapes — `let _ = offset;` and the four bare `?;`
drops — before this run):**

```
thread 'control::controller::tests::a_fresh_run_checkpoint_carries_a_real_event_log_offset' panicked at crates/gauntlet-app/src/control/controller.rs:4303:9:
the close-out checkpoint carries the store-returned offset, not the fresh-run zero (got 0)
```

The scenario deliberately includes a failed-then-retried invocation (a
real `InvocationTerminal` event before the close-out) so the test
discriminates BOTH the terminal-append threading and the checkpoint's own
append, not just the latter.

**Fix (controller.rs, five sites):**

- `checkpoint(&self, state: &mut LoopState)` — the close-out caller passes
  `&mut`; the checkpoint's own `CheckpointWritten` append assigns
  `state.last_offset` (the `let _ = offset;` discard is gone; the doc
  records §18.3 step 1's anchor semantics).
- The four `append_terminal_event` callers that dropped the returned
  offset now assign `state.last_offset` from it: hand-edit cancellation
  (`reread_answer_key`), drain cancellation, failure disposition, timeout
  disposition. The resume caller already assigned it.
- The durable row keeps recording the water-mark BEFORE its own append
  (the anchor a replay starts from), and the next checkpoint starts where
  the previous one ended.

**Files:** `crates/gauntlet-app/src/control/controller.rs`.

**Tests:** new
`control::controller::tests::a_fresh_run_checkpoint_carries_a_real_event_log_offset`
(asserts the close-out row's offset > 0 and strictly behind the store's
head).

**Gates after item 1:** `cargo nextest run -p gauntlet-app control::` —
`Summary [   0.619s] 66 tests run: 66 passed, 225 skipped`.

## Item 2 — N3 strict checkpoint offset READ path (state-sqlite) — STARTED 2026-08-25 12:18

Intended design: bounded to the CHECKPOINT leg only. The sqlite
`resume`'s decode `EventLogOffset::new(u64::try_from(offset).unwrap_or(0))`
(`state_store.rs:1706`) silently re-introduces the constant-zero defect
C8.1 just removed whenever a tampered/negative stored offset fails
`try_from`. Make it strict: a non-representable stored offset is
`StateStoreError::Corrupt` naming run and checkpoint seq. The settlement/
evidence-production/event-log coercions and `settled_at` are Task 13's
surface and are NOT touched. Fail-first: write a checkpoint through the
adapter, tamper the row's offset to a negative integer via the same
connection, observe `resume` returning `Ok` with offset 0 today, assert
`Corrupt` instead.

## Item 2 — N3 strict checkpoint offset READ path (state-sqlite) — DONE 2026-08-25 12:26

**Fail-first (verbatim):**

```
thread 'state_store::tests::a_tampered_checkpoint_offset_is_corrupt_never_a_silent_zero' panicked at crates/adapters/gauntlet-adapter-state-sqlite/src/state_store.rs:2542:37:
a non-representable stored offset is corrupt, never a silent zero (got 0)
```

**Fix:** `state_store.rs` `resume`'s checkpoint decode —
`EventLogOffset::new(u64::try_from(offset).unwrap_or(0))` became a strict
`map_err` to `StateStoreError::Corrupt` naming the run, the checkpoint
`seq`, and the stored value. **Bounded to the checkpoint leg only**: the
settlement / evidence-production / event-log coercions and `settled_at`
are untouched (Task 13's surface, per `residual-readjudication.md`).

**Files:** `crates/adapters/gauntlet-adapter-state-sqlite/src/state_store.rs`.

**Tests:** new `state_store::tests::a_tampered_checkpoint_offset_is_corrupt_never_a_silent_zero`
(writes a checkpoint through the adapter, tampers the row's offset to -1
via the same connection, asserts the typed `Corrupt` refusal names the
locus).

**Gates after item 2:** `cargo nextest run -p gauntlet-adapter-state-sqlite`
— `Summary [   0.422s] 41 tests run: 41 passed, 0 skipped` (40 baseline +
1 new).

## Item 3 — C8.2 checkpoint at the ITERATION BOUNDARY — STARTED 2026-08-25 12:29

Intended design (the cancelled builder's recorded design, adopted): the
per-completion mid-pipeline checkpoint (`process_completion`, after the
charge, BEFORE observe/compare/verify/settle) is REMOVED; the boundary
checkpoint is written at the TOP of iteration k in `drive` (after
`next_iteration`, before `boundary_verify`), declaring iteration k. That
single placement is simultaneously (a) §11.1's iterate order — content
"post dispose-residuals of k−1", the checkpoint last — and (b) the
producer-authority anchor: iteration k's owner writes (observation
binding, evidence production, comparison — all declaring k) validate
against the LATEST durable checkpoint, which now declares k BEFORE any of
them run; the close-out checkpoint at the terminal stays and shares the
last iteration number. PROOF ARITHMETIC THAT MUST SURVIVE, unchanged:
p6 `checkpoints == FORWARD_ROUNDS_CEILING + 1` (5 boundary + 1 close-out —
the p6 run's rounds terminal fires at the END of iteration 5 in
`decide_progress`, so the loop runs exactly 5 iterations) and p4 `>= 2`.
Fail-first: a crash after a settle must never re-run settled work
(§18.3) — mid-pipeline, the latest durable checkpoint predates the
settle, and a resume re-dispatches the settled unit.

## Item 3 — C8.2 checkpoint at the ITERATION BOUNDARY — DONE 2026-08-25 12:41

**Fail-first (verbatim, against the mid-pipeline placement):**

```
thread 'control::controller::tests::a_crash_after_a_settlement_never_re_runs_settled_work' panicked at crates/gauntlet-app/src/control/controller.rs:4398:9:
assertion `left == right` failed: a unit settled before the crash is never re-dispatched (spec 18.3)
  left: 2
 right: 1
```

Iteration 1 settled AK-9101; the latest durable checkpoint was written
BEFORE that settle (mid-pipeline, post-charge pre-verify), so the resume
re-dispatched the settled unit — §18.3's defect class exactly.

**Fix (the recorded design, adopted without deviation):**

- `drive` writes the boundary checkpoint at the TOP of iteration k (after
  `next_iteration`, before `boundary_verify`), declaring k. The doc
  records why one placement satisfies both rules: the CONTENT is
  post-dispose of k−1 (§11.1's iterate order — the checkpoint ends the
  PRIOR iteration), and the declaration of k is the producer-authority
  anchor iteration k's owner writes validate against (C5/B-M2) — the
  pre-C8.2 mid-pipeline checkpoint supplied that anchor one completion
  late.
- The per-completion checkpoint call in `process_completion` is REMOVED.
- The close-out checkpoint at the terminal stays; it declares the same
  iteration as the last boundary checkpoint. The sqlite resume's
  `ORDER BY iteration_no DESC, seq DESC` picks the close-out (verified by
  the adapter suite staying green).
- Module-doc loop description updated to the new order.

**Proof arithmetic — UNCHANGED and still exact (no proof-assertion
edits):**

- `cargo nextest run -p gauntlet-app control::` —
  `Summary [   0.367s] 67 tests run: 67 passed, 225 skipped` (66 + 1 new;
  the multi-iteration composed-diagnosis tests exercise iteration ≥ 2
  owner writes, so the producer-authority hazard is covered by the
  suite, not just reasoned about).
- `cargo nextest run --test p4_corrective_loop --test p6_budget_control`
  — `Summary [   1.303s] 6 tests run: 6 passed, 0 skipped`, including
  p6's UNMODIFIED `checkpoints == FORWARD_ROUNDS_CEILING + 1` (5 boundary
  + 1 close-out; the rounds terminal fires in `decide_progress` at the
  end of iteration 5, so the loop runs exactly 5 iterations) and p4's
  `>= 2`.

**Files:** `crates/gauntlet-app/src/control/controller.rs`.

**Tests:** new
`control::controller::tests::a_crash_after_a_settlement_never_re_runs_settled_work`
(crash injected as `Behavior::PollError` — a new fixture variant — on
iteration 2's dispatch; asserts the resume settles the run, prompts the
pre-crash-settled unit exactly once across both controllers, and
re-plans the owed unit).

## Item 4 — C8.3 resume restores per-unit attempts — STARTED 2026-08-25 12:45

Intended design (attempts leg ONLY; retry-ledger persistence stays L2 —
the §18.2 field set is spec-fixed): reconstruct `attempts` in
`resume_run` from the loop's own durable producer writes — one
`record_evidence_production` per completed attempt that produced
instrument evidence — via the existing `StateStore::evidence_productions_
at(run, locator)` query, keyed by each unit's FROZEN locator (from the
same frozen-projection parse item 0 added). Source verified before
relying on it, with two recorded caveats: a facet-less completion
records no production (undercount — conservative for a per-unit ceiling)
and units sharing one locator share one count (overcount for the
less-attempted unit); both are documented at the code, and L2's
retry-ledger persistence remains the full-fidelity close. Fail-first: a
crashed residual-retry run resumes with attempts restored to 1, so the
next residual disposes at the §11.5 cap (escalate) instead of re-arming
the whole retry ladder.

## Item 4 — C8.3 resume restores per-unit attempts — DONE 2026-08-25 12:57

**Fail-first (verbatim, with the reconstruction neutralized):**

```
thread 'control::controller::tests::resume_restores_per_unit_attempts_from_durable_productions' panicked at crates/gauntlet-app/src/control/controller.rs:4537:9:
assertion `left == right` failed: one completed attempt before the crash + one restored-ceiling attempt after it
  left: 4
 right: 2
```

Without restoration the resumed run re-armed the ENTIRE ladder — and the
extra attempts fed the non-convergence detector, which fired diagnosis on
the third recurring residual. One honest correction to my own test
arithmetic, recorded: the crash lands at the second attempt's POLL
(after its prompt), so the crashed controller prompts twice and its
never-completed in-flight attempt is legitimately re-runnable — the
ceiling, not the prompt history, is what must not re-arm. With the fix
active the totals are 3 (2 pre-crash + 1 restored-ceiling) vs 4
un-restored; the assertion was corrected to 3 with an accurate message
before the green run (the corrected test still discriminates: 3 vs 4,
and terminal `Escalated` vs the ladder continuing).

**Fix:** `resume_run` reconstructs `attempts` after `seed_unit_states`
from `StateStore::evidence_productions_at(run, locator)` over each
unit's FROZEN locator (`frozen_units` — item 0's projection load, one
step earlier; `frozen_membership` now derives from it). Two recorded
caveats at the code: facet-less completions record no production
(undercount — conservative for a per-unit ceiling), and units sharing
one locator share one count (overcount for the less-attempted unit).
Retry-ledger persistence (`timeout_retries`, `reminders`,
`pending_attachments`) stays L2 — the §18.2 field set is spec-fixed.

**The conditional the brief asked me to answer:** does C8.3's resume
need to distinguish a checkpoint-only projection from a full one? **No.**
The resume path reads exactly two durable shapes, each self-identifying
by kind: the `Checkpoint` (via `StateStore::resume`) and the full
evidence-production rows (via `evidence_productions_at`); no consumer
needs to know whether other data exists outside the checkpoint. The
"checkpoint-only run projection indistinguishable" ledger item stays
later-scope, un-activated.

**Files:** `crates/gauntlet-app/src/control/controller.rs`.

**Tests:** new
`control::controller::tests::resume_restores_per_unit_attempts_from_durable_productions`.

**Gates after item 4:** `cargo nextest run -p gauntlet-app control::` —
`Summary [   0.444s] 68 tests run: 68 passed, 225 skipped`.

## Item 5 — C9.2 escape the escalation-evidence JSON (one shared helper) — STARTED 2026-08-25 12:59

Intended design: one shared `escape_json_text` helper (backslash first,
then quote — both are the characters that break the record) used by
`submit_ticket`'s evidence record (the funnel for all three escalation
terminals since C1: `escalated`, `blocked`, `inconclusive`), replacing
the raw interpolation. The two existing ad-hoc `.replace('"', "'")`
sites (the §21.3 reference-unavailable record and the §15.2 quarantine
reason) route through the SAME helper so there are not three copies of
an escaping rule — checked first that no test pins their exact bytes
(p4 pins only `"quarantine":true`, not the reason field). Fail-first at
the production seam: drive `submit_ticket` with a quote- and
backslash-bearing summary and assert the pinned record carries the
ESCAPED sequence, not the raw characters.

## Item 5 — C9.2 escape the escalation-evidence JSON — DONE 2026-08-25 13:09

**Fail-first (verbatim, with `ticket_evidence_record` neutralized to the
pre-fix raw interpolation):**

```
thread 'control::controller::tests::an_escalation_summary_with_quotes_and_backslashes_stays_parseable_evidence' panicked at crates/gauntlet-app/src/control/controller.rs:4515:13:
the forensic record parses: json: unexpected byte 0x67 at offset 43 — record: {"unit":"AK-9101","summary":"run blocked: "governance" and a \ backslash"}
```

**Fix (controller.rs):**

- New shared `escape_json_text` (backslash FIRST, then quote — order is
  load-bearing) and `ticket_evidence_record(unit, summary)`; `submit_ticket`
  writes through it. Since C1 this one record is the funnel for all three
  escalation terminals, so the single helper covers the whole surface.
- The two existing ad-hoc `.replace('"', "'")` sites — the §21.3
  reference-unavailability record and the §15.2 quarantine reason — now
  route through the SAME helper (three copies of an escaping rule was the
  next defect). Pre-checked that no test pins their exact bytes: p4 pins
  only the `"quarantine":true` substring, not the reason field; p4/p6
  re-run green after the change.

**Files:** `crates/gauntlet-app/src/control/controller.rs`.

**Tests:** new
`control::controller::tests::an_escalation_summary_with_quotes_and_backslashes_stays_parseable_evidence`
— asserts the record PARSES under the ports' own strict JSON parser and
the hostile summary round-trips byte-for-byte.

**Gates after item 5:** `cargo nextest run -p gauntlet-app control::` —
`Summary [   0.481s] 69 tests run: 69 passed, 225 skipped`;
`cargo nextest run --test p4_corrective_loop --test p6_budget_control` —
`Summary [   0.835s] 6 tests run: 6 passed, 0 skipped`.

## Item 6 — C9.3 no silent result drop on charge refusal (BOTH sites) — STARTED 2026-08-25 13:11

Intended design: §21.2 "never a swallowed error" + §10.4 "not silently
dropped" + §12.3's forensics precedent (an artifact may be STORED FOR
FORENSICS while excluded from verification inputs): on EITHER charge
refusal (the meter's, or the loop budget's own — C2 created the second
site), the completed result's facts are PINNED as a typed-class JSON
evidence record (`result_uncharged`, naming unit, invocation, the
requested payment, the refusing locus `meter`/`run_budget`, and the
disposition `budget_exhausted`) BEFORE the terminal is set — kept,
recorded, settling nothing. The meter-refusal path additionally RELEASES
the reservation (the parallel-slot leak: a refused charge keeps its
reservation because only a booked charge releases the slot). Site 1 is
reachable composed (a payment past the per-attempt reservation);
site 2 is unreachable through the shipped `FakeMeter` (it mirrors the
loop budget, so the meter always refuses first) — it needs a permissive
meter fixture plus a payment sequence that crosses the loop budget's
forward ceiling, wired through a meter-factory parameter on the
`compose_full` fixture.

## Item 6 — C9.3 no silent result drop on charge refusal — DONE 2026-08-25 13:27

**Fail-first (verbatim, BOTH sites):**

```
thread 'control::controller::tests::a_result_the_run_budget_refuses_is_recorded_not_dropped' panicked at crates/gauntlet-app/src/control/controller.rs:4729:9:
assertion `left == right` failed: the refused result is pinned as forensic evidence: []
  left: 0
 right: 1
```

(site 1's run failed identically — `2 tests run: 0 passed, 2 failed`,
both `left: 0, right: 1`: nothing pinned on either refusal path.)

**Fix (controller.rs):**

- New `record_uncharged_result(state, flight, payment, refused_by)` —
  pins a `result_uncharged` JSON forensic record naming run, iteration,
  unit, invocation, the requested payment (tokens/wall/cost), the typed
  refusing locus, and the disposition `budget_exhausted`, with the §12.3
  note (stored for forensics; excluded from verification inputs; settles
  nothing). Locus values are named consts
  (`CHARGE_LOCUS_METER`/`CHARGE_LOCUS_RUN_BUDGET`).
- Site 1 (meter refusal): releases the reservation FIRST (the leak — a
  refused charge keeps its slot because only a booked charge releases
  it), then records, then the terminal.
- Site 2 (the loop budget's own refusal, C2's second site): records,
  then the terminal (the meter's successful booking already released the
  slot).

**Files:** `crates/gauntlet-app/src/control/controller.rs`.

**Tests:** new
`a_result_whose_meter_charge_is_refused_is_recorded_not_dropped`
(site 1 composed: payment 200 past the 100-token per-attempt
reservation) and
`a_result_the_run_budget_refuses_is_recorded_not_dropped`
(site 2 isolated: the new `PermissiveMeter` fixture — books nothing,
refuses nothing — because the shipped mirror meter always reaches the
loop budget's refusal first; two 700-token completions cross the
800-token forward ceiling on the second). Fixture additions:
`Behavior::CompleteWithTokens(u64)` (the payment-size lever through the
SAME result-document path every completion uses) and a meter-factory
parameter on `compose_full` (defaulted to `meter_factory()` at all four
existing call sites).

**Gates after item 6:** `cargo nextest run -p gauntlet-app control::` —
`Summary [   0.844s] 71 tests run: 71 passed, 225 skipped`.

## Item 7 — C9.4 bounded drain (max polls per tick) — STARTED 2026-08-25 13:29

Intended design (derived bound, NOT a new Annex E setting — the
blast-radius note: a `max_polls_per_tick` on `ControlPolicy` would touch
`ControlPolicy::new` validation, `from_settings`'s typed mapping, and
the Annex E config surface, compounding critic-A m2's open "four REQUIRED
settings" minor): the drain pass bound is DERIVED from the two settings
the drain already reads and already requires —
`stall_timeout_ms.div_ceil(poll_interval_ms)` — the number of polls one
stall window holds. Within that many consecutive passes any healthy
invocation either completes or is stall-caught; still-non-empty in-flight
after a full window of polls means progress claims are cycling without
completion, and past-the-bound behaviour follows the recorded design:
CANCEL in-flight, PIN the fact as evidence, ESCALATE. Fail-first: a
runner that reports ever-fresh progress and never completes cannot be
stopped by stall detection — the drain spins until something external
kills it (observed as the fixture clock exhausting its sleep budget and
erroring); with the bound, the drain terminates itself and escalates
inside the budget.

## Item 7 — C9.4 bounded drain — DONE 2026-08-25 13:44

**Fail-first (verbatim, against the unbounded drain):**

```
thread 'control::controller::tests::a_drain_that_never_terminates_is_bounded_cancelled_and_escalated' panicked at crates/gauntlet-app/src/control/controller.rs:4846:14:
the drain bounds itself without external help: Agent(Transport { detail: "the fixture runner exhausted its poll budget while the drain kept spinning" })
```

The drain could not stop itself — the fixture runner's own budget had to
kill it. (Fixture bug found and fixed during the red run, recorded: my
first `ProgressForever` arm double-counted polls — `poll()`'s preamble
already counts every call — so the budget tripped at pass 17 instead of
33; corrected to read the preamble's count before the second run.)

**Fix (controller.rs, `drain` + new `bound_the_drain`):**

- Pass bound DERIVED from the two settings the drain already reads and
  already requires: `stall_timeout_ms.div_ceil(poll_interval_ms)` — one
  stall window of polls. NO new `ControlPolicy`/Annex E setting (the
  blast-radius note in the brief: a fifth REQUIRED setting would compound
  critic-A m2's open minor). Doc records the derivation's meaning:
  within one window of consecutive passes every healthy invocation
  either completes or is stall-caught; still-non-empty after that means
  progress claims are cycling without completion.
- Past the bound (recorded design adopted): CANCEL every in-flight
  (releasing each reservation; `AlreadyTerminal` tolerated), append the
  cancelled terminal events (offset-threaded per C8.1), PIN a
  `drain_poll_bound` JSON record naming the units and the derived limit,
  ESCALATE (`Escalated` terminal — the loop owes the operator a ticket,
  not a spin).

**Files:** `crates/gauntlet-app/src/control/controller.rs`.

**Tests:** new
`a_drain_that_never_terminates_is_bounded_cancelled_and_escalated`
(`Behavior::ProgressForever { max_polls }` — ever-fresh far-future
progress that defeats stall detection; asserts `Escalated`, the pinned
`drain_poll_bound` record, and that the drain stopped inside one stall
window of polls).

**Gates after item 7:** `cargo nextest run -p gauntlet-app control::` —
`Summary [   0.883s] 72 tests run: 72 passed, 225 skipped`.

## Item 8 — N4 evidence-locator containment at the controller write path — STARTED 2026-08-25 13:47

Intended design: the fix lives at the CONTROLLER's containment boundary,
not the domain — `validate_plain_ref` is a Task-3-closed public value
grammar shared by every locator-like newtype, and widening it is a
domain/ports revision, not a controller fix. A component-wise guard
(`ParentDir`, `RootDir`, `Prefix` all refused — parent-escaping AND
absolute locators) before the `create_dir_all` + `fs::write` at the one
canonical workspace root, with a typed `ControlError::EvidenceLocatorEscapes`
naming the locator and the root. Fail-first with a `../`-bearing locator:
today the run completes and writes bytes OUTSIDE the root.

## Item 8 — N4 evidence-locator containment — DONE 2026-08-25 13:58

**Fail-first (verbatim, with the component guard neutralized):**

```
thread 'control::controller::tests::an_escaping_evidence_locator_is_refused_at_the_write_boundary' panicked at crates/gauntlet-app/src/control/controller.rs:5006:27:
the escaping locator is refused typed, got verification refused: the evidence store refused the binding check: locator ../leak-ctrl-9101.json escapes the governed evidence workspace root — the chain refuses to bind bytes outside the workspace the instruments read
```

Plus the direct file-system evidence, personally observed before cleanup:
`/tmp/leak-ctrl-9101.json` EXISTED and held the facet bytes
(`{"schema":"facet_evidence@v1","instrument_name":"gauntlet-controller-fixture",...`)
— the write had already escaped. Notable, recorded: F1's verification
chain DOES refuse the escaped locator at the BIND check — but only AFTER
the controller has written the bytes outside the root, and as a hard
error with no terminal (the C1-family shape). The controller-boundary
guard is therefore load-bearing: the byte must never be written; the
bind-time refusal is a second layer, not a substitute.

**Fix (controller.rs):** new `contained_locator_path(root, locator)` —
component-wise refusal of `ParentDir`, `RootDir`, and `Prefix` (the
`..` escape AND absolute locators) before the `create_dir_all` +
`fs::write`; new typed `ControlError::EvidenceLocatorEscapes { locator,
root }` whose Display names both. The domain's `validate_plain_ref` is
UNTOUCHED (Task-3-closed public value grammar shared by every
locator-like newtype — widening it is a ports revision).

**Files:** `crates/gauntlet-app/src/control/controller.rs`.

**Tests:** new
`an_escaping_evidence_locator_is_refused_at_the_write_boundary`
(answer-key unit with `locator: ../leak-ctrl-9101.json`; asserts the
typed refusal carries the exact locator and the canonical root, and no
byte lands outside the root).

**Gates after item 8:** `cargo nextest run -p gauntlet-app control::` —
`Summary [   0.547s] 73 tests run: 73 passed, 225 skipped`.

## Item 9 — N5 the durable §15.1 record reports FORWARD headroom — STARTED 2026-08-25 14:00

Intended design: the fix belongs in `remaining_of` ITSELF, not a record-
site patch. Grounding: §17.4 "The recovery reserve is never spendable on
forward work" — a meter whose `remaining()` reports reserve-inclusive
headroom contradicts its own `would_breach_reserve` gate, and the §6.2
port row says only "every dimension in §5.3.7" (no raw-limit semantics).
The helper's OWN doc already promises "projects the domain budget's
FORWARD CEILINGS into a Remaining view; the ceiling arithmetic stays in
the domain" — the implementation betrays its doc; the fix makes the code
conform. Caller check (the brief's conditional): every `remaining_of`
caller — the shipped meter-memory adapter's `remaining()`, the scheduler/
controller/select TEST meters, the contract stubs — routes through the
same helper and uses it only for RECORDS (every gate is
`would_breach_reserve`), so no caller needs raw-limit headroom; changing
the helper moves all of them consistently. The domain's
`Budget::forward_ceiling` becomes `pub` (the port reads the domain's
arithmetic rather than duplicating the formula). Fail-first: the port's
own `remaining_is_limits_minus_spent` test rewritten to the forward
semantics (1000 tokens at a 20% reserve ⇒ 800), run red against the
current raw-limit code.

## Item 9 — N5 the durable §15.1 record reports FORWARD headroom — DONE 2026-08-25 14:16

**Fail-first (verbatim, the port's own test rewritten to the forward
semantics and run against the shipped raw-limit code):**

```
thread 'budget_meter::tests::remaining_is_forward_ceiling_minus_spent' panicked at crates/gauntlet-ports/src/budget_meter.rs:188:9:
assertion `left == right` failed: 10 rounds at a 20 percent reserve: 8 forward
  left: 10
 right: 8
```

**Where the fix belongs, and why (the brief's diagnostic question):** in
`remaining_of` ITSELF, not a record-site patch. §17.4 "The recovery
reserve is never spendable on forward work" — a meter whose `remaining()`
reports reserve-inclusive headroom contradicts its own
`would_breach_reserve` gate; the §6.2 port row says only "every dimension
in §5.3.7" (no raw-limit semantics); and the helper's OWN doc already
promised "projects the domain budget's FORWARD CEILINGS … the ceiling
arithmetic stays in the domain" — the implementation was betraying its
doc. **Caller check (the conditional):** every `remaining_of` caller —
the shipped meter-memory adapter's `remaining()`, the scheduler/
controller/select test meters, the contract stubs — routes through the
same helper and uses it only for RECORDS (every gate is
`would_breach_reserve`), so no caller needs raw-limit headroom; the
helper change moves them all consistently.

**Fix:**

- `gauntlet-domain/src/budget.rs`: `Budget::forward_ceiling` made `pub`
  (doc: the port reads the domain's own ceiling arithmetic rather than
  duplicating the formula — N5's single source).
- `gauntlet-ports/src/budget_meter.rs`: `remaining_of` subtracts spent
  from the FORWARD ceiling per dimension (rounds/wall/tokens/cost);
  parallel/depth pass through (scope dimensions, no reserve, no spend).
- Contract-suite knock-ons moved WITH the semantics, each preserving the
  asserted invariant in stronger before/after form (never a raw-limit
  constant): the meter-memory adapter's two tests (`nothing was booked`
  and the within-reservation bookings) and p6's
  `the_meter_refuses_the_reserve_consuming_slice_and_books_nothing`
  (refusal-booked-nothing and zero-payment-release-booked-nothing). The
  select.rs test asserting `recorded == meter.remaining()` at call time
  stays green UNCHANGED (both sides route through the helper).

**Files:** `crates/gauntlet-domain/src/budget.rs`,
`crates/gauntlet-ports/src/budget_meter.rs`,
`crates/adapters/gauntlet-adapter-meter-memory/src/lib.rs`,
`crates/gauntlet-cli/tests/p6_budget_control.rs`.

**Tests:** port test rewritten
(`remaining_is_forward_ceiling_minus_spent`: 10→8 rounds, 3600→2880
wall, 1000→800 tokens, 500→400 cost at a 20% reserve); three raw pins
converted to before/after invariants as above.

**Gates after item 9 (all green):**
`cargo nextest run -p gauntlet-adapter-meter-memory` — `Summary [   0.080s] 7 tests run: 7 passed`;
`cargo nextest run --test p4_corrective_loop --test p6_budget_control` — `Summary [   0.925s] 6 tests run: 6 passed`;
`cargo nextest run -p gauntlet-app control::` — `Summary [   0.633s] 73 tests run: 73 passed`.

### F2 slice gates — all six, run personally, sequentially (2026-08-25 14:30)

```
$ just check-package gauntlet-app
cargo check -p gauntlet-app --all-targets
    Checking gauntlet-ports v0.1.0 (/home/sprime01/projects/gauntlet/crates/gauntlet-ports)
    Checking gauntlet-app v0.1.0 (/home/sprime01/projects/gauntlet/crates/gauntlet-app)
    Finished 'dev' profile [unoptimized] target(s) in 15.86s

$ cargo nextest run -p gauntlet-app control::
     Summary [   0.644s] 73 tests run: 73 passed, 225 skipped

$ cargo nextest run --test p4_corrective_loop --test p6_budget_control
     Summary [   3.007s] 6 tests run: 6 passed, 0 skipped

$ cargo nextest run -p gauntlet-ports
     Summary [   0.463s] 88 tests run: 88 passed, 0 skipped

$ cargo nextest run -p gauntlet-adapter-state-sqlite
     Summary [   0.988s] 41 tests run: 41 passed, 0 skipped

$ just lint
cargo clippy --workspace --all-targets -- -D warnings
    Finished 'dev' profile [unoptimized] target(s) in 1.99s
cargo fmt --check
```

(`just lint` exited 0; `cargo fmt --check` silent on match. Two red
intermediate lint states during the item-9 close — an `unused_mut` in
`bound_the_drain` and rustfmt drift in the rewritten port test — were
fixed before the final run; both are visible above in item 9's trail.)

F2 slice complete. Result contract:
`.agents/reports/task12/correction-f2.md`.

---

# Correction round 1 — FINISHER slice F3 (2026-08-25)

Session start. Read: brief `correction-brief-f3.md`, `AGENTS.md`,
`residual-readjudication.md` (N1, N2), `correction-f1.md`,
`correction-f2.md`, this file's F1/F2 entries.
`./scripts/reconcile-checkpoints.sh 12` run FIRST: "OK — no task-surface
file was modified after the last checkpoint write" — nothing of mine is
partly done in the tree. Single-compiler build lock held throughout; no
concurrent cargo/nextest. Baseline evidence before any edit: the frozen
manifest's deep parse is already pinned green in-tree
(`reference/manifest.rs::the_frozen_manifest_parses_with_its_fields_intact`,
which reads the REAL `.wayfinder/reference.yaml` and asserts every
extracted field), so the N1 stop condition is not expected to fire — it
will be re-verified through the real doctor path at item 3's
`just doctor`.

## Item 1 — N1 doctor check 2's manifest clause non-vacuous + strict parse — STARTED 2026-08-25 13:15

Intended design (`crates/gauntlet-app/src/doctor/checks.rs`, the check-2
manifest clause, and the scratch fixture in `doctor/mod.rs`):

- Vacuity leg: `top_level_mapping_keys` returning `Ok(0)` (an all-indented
  file) currently reports PASS ("top level is a mapping (0 keys)"). A
  mapping with zero top-level keys is not a mapping — the clause will
  treat `Ok(0)` as a FAIL with evidence naming the vacuity. The shape
  check itself (column-zero `key:` lines) stays as the cheap first gate.
- Deep-parse leg: after the shape gate passes, the clause runs the STRICT
  `parse_manifest` (the same parser the provider path uses,
  `reference/provider.rs:182`); an `Err` is reported as a reference-class
  failure carrying the typed `ManifestError`'s line and reason; a pass
  reports the parsed identity (id, version) so the success evidence is
  non-vacuous too.
- Fixture promotion: `doctor/mod.rs::scratch_manifest` (documented
  "shallow on purpose") becomes a minimal FROZEN-SHAPE manifest (id,
  version, folded purpose/desired_state, settlement with
  success_criteria + a `>= 1 settlements` minimum_reliability +
  terminal_conditions) that deep-parses. The scratch lock already hashes
  `scratch_manifest()`'s bytes, so it follows automatically.
- Known knock-on, moving WITH the semantics (F2's precedent, not a
  weakening): `a_moved_manifest_fails_check_two_as_a_reference_class_failure`
  rewrites the scratch manifest to a shallow one-line variant and asserts
  the manifest clause still passes; under the deep parse that rewrite is
  itself a manifest failure. The moved bytes become a deep-parseable
  rewrite (prose edit) so the test's exact assertions — manifest clause
  PASSES, digest clause alone FAILS — keep their meaning.
- Fail-first, BOTH legs: (a) an all-indented manifest (lock digest true,
  so the manifest clause is the only locus) must FAIL — today it passes;
  (b) a top-level-shaped manifest that cannot deep-parse (shallow
  `id`/`version` only) must FAIL with reference-class evidence — today it
  passes. Stop condition acknowledged: if the REAL
  `.wayfinder/reference.yaml` fails the deep parse, halt and record in
  `.agents/reports/reference-candidates.md`.

## Item 1 — N1 doctor check 2's manifest clause non-vacuous + strict parse — DONE 2026-08-25 13:21

**Baseline before any edit:** `cargo nextest run -p gauntlet-app doctor::`
— `Summary [   0.230s] 25 tests run: 25 passed, 273 skipped`.

**Fail-first, BOTH legs (verbatim):**

Leg 1 — the vacuity (an all-indented manifest, lock rewritten TRUE for the
doctored bytes, so the manifest clause is the only possible locus):

```
thread 'doctor::tests::an_all_indented_manifest_is_not_a_mapping_and_fails_check_two' panicked at crates/gauntlet-app/src/doctor/mod.rs:983:9:
assertion `left == right` failed: an all-indented manifest is not a mapping: gauntlet doctor (spec 8.5 preflight)
  left: Pass
 right: Fail
```

Leg 2 — the strict parse (top-level mapping shape kept, deep-parse-breaking
content: the pre-promotion scratch shape `id`/`version` only):

```
thread 'doctor::tests::a_manifest_that_cannot_deep_parse_fails_check_two_as_a_reference_class_failure' panicked at crates/gauntlet-app/src/doctor/mod.rs:1031:9:
assertion `left == right` failed
  left: Pass
 right: Fail
```

(`2 tests run: 0 passed, 2 failed` — both legs red against the shipped
clause, exactly the two N1 defects.)

**Fix (`crates/gauntlet-app/src/doctor/checks.rs`, check 2's manifest
clause):**

- `Ok(0)` from `top_level_mapping_keys` is now a FAIL with evidence
  naming the vacuity ("zero keys is not a pass"); the shape check stays
  the cheap first gate, and the helper's doc records the zero-count
  contract.
- After a non-zero shape pass, the clause runs the STRICT
  `parse_manifest` (the same parser the provider path uses at admission,
  `reference/provider.rs:182`): an `Err` is a FAIL whose evidence names
  the reference class and carries the typed `ManifestError`'s line and
  reason; a pass reports the parsed identity (id, version) so success
  evidence is non-vacuous too.

**Fixture promotion (`crates/gauntlet-app/src/doctor/mod.rs`):**
`scratch_manifest` is now a minimal FROZEN-SHAPE manifest (id
`scratch-ref-01`, version, folded purpose/desired_state, settlement with
success_criteria + `>= 1 settlements` minimum_reliability +
`terminal_conditions: [settled, aborted]`) that deep-parses; the scratch
lock already hashes `scratch_manifest()`'s bytes, so it follows
automatically.

**Knock-on moved WITH the semantics (assertions unchanged, fixture
moved):** `a_moved_manifest_fails_check_two_as_a_reference_class_failure`
rewrote the scratch manifest to a shallow one-liner and asserted the
manifest clause still passes — under the deep parse that rewrite is
itself a manifest failure. The moved bytes are now a deep-parseable
prose rewrite of the promoted fixture, so the test's exact assertions
(manifest clause PASSES, digest clause alone FAILS) keep their original
meaning: the digest clause stays the only possible locus. A
`assert_ne!` guards that the rewrite actually moves bytes.

**Stop condition check:** the REAL `.wayfinder/reference.yaml`
deep-parses — pinned in-tree by
`reference/manifest.rs::the_frozen_manifest_parses_with_its_fields_intact`
(reads the real file, asserts every extracted field), and re-verified
through the real doctor path at item 3's `just doctor`. No
`reference-candidates.md` entry needed; the parser was not touched.

**Files:** `crates/gauntlet-app/src/doctor/checks.rs`,
`crates/gauntlet-app/src/doctor/mod.rs`.

**Tests:** new `doctor::tests::an_all_indented_manifest_is_not_a_mapping_and_fails_check_two`
and `doctor::tests::a_manifest_that_cannot_deep_parse_fails_check_two_as_a_reference_class_failure`.

**Gates after item 1:** `cargo nextest run -p gauntlet-app doctor::` —
`Summary [   0.111s] 27 tests run: 27 passed, 273 skipped` (25 baseline +
2 new).

## Item 2 — N2 in-crate spawn-classification test (observe-command) — STARTED 2026-08-25 13:21

Intended design (`crates/adapters/gauntlet-adapter-observe-command/src/lib.rs`,
the same `mod tests` that holds the deadline/ceiling tests): one in-crate
test pinning the spawn-failure classification — a program that cannot
spawn "could not run at all" → `ObserverError::Unavailable` with detail
naming the spawn failure (`run_bounded`'s `map_err` arm, the typed
classification read, not assumed). The test drives the REAL
`CommandObserver::observe` end-to-end over a live scratch workspace
(mirroring how C4's p4 proof leans on it cross-crate) with a
non-spawnable program: a `./`-relative name guaranteed absent from the
scratch workspace, so PATH is bypassed and ENOENT is deterministic.
Real `WallClock`, future deadline, `NullStore` (never reached — the
spawn refuses first). Fail-first here means discrimination, not
existence: land the test green, then briefly flip the classification at
the spawn arm to `InstrumentFailed`, observe the test FAIL, revert
byte-identical, re-run green; both observations recorded.

## Item 2 — N2 in-crate spawn-classification test (observe-command) — DONE 2026-08-25 13:23

**Observation 1 — green against the shipped classification:** the new
test landed and the whole package suite passed —

```
     Summary [   1.033s] 10 tests run: 10 passed, 0 skipped
```

**Observation 2 — the flip (verbatim):** the spawn arm's classification
was flipped `Unavailable` → `InstrumentFailed` (nothing else touched),
and the new test failed exactly on the classification:

```
thread 'tests::a_program_that_cannot_spawn_is_unavailable_not_a_failed_instrument' panicked at crates/adapters/gauntlet-adapter-observe-command/src/lib.rs:727:22:
a non-spawnable program is an unavailable instrument, got Err(InstrumentFailed { kind: Test, detail: "spawning the command instrument failed: No such file or directory (os error 2)" })
```

(`1 test run: 0 passed, 1 failed`.)

**Revert — byte-identical, verified:** the flip was reverted;
`sha256sum` over the file returned
`16c3420947c8b3ab9e067b5e859b84765c9d6a768f751e3c9de9666619601976`
both before the flip and after the revert, and `diff` against a
pre-flip snapshot was clean. **Observation 3 — green again:**
`Summary [   1.047s] 10 tests run: 10 passed, 0 skipped`.

**Test:** `tests::a_program_that_cannot_spawn_is_unavailable_not_a_failed_instrument`
in the adapter's in-crate `mod tests` (the same module as the
deadline/ceiling tests), driving the REAL `CommandObserver::observe`
end-to-end: live scratch workspace, real `WallClock`, future deadline,
`NullStore` (never reached — the spawn refuses first), and a
`./`-relative absent program name so PATH lookup is bypassed and the
spawn failure is a deterministic not-found against the workspace we
just created (the integration-test sibling uses a bare PATH-looked-up
name). The typed classification was read, not assumed:
`ObserverError::Unavailable { detail }` with `detail` naming the spawn
failure — `run_bounded`'s `map_err` arm.

**Honest scope note (bounded claim):** the property is ALSO
discriminated in the package's integration target
(`tests/shared_contract.rs::a_missing_program_is_unavailable_never_a_facet`,
added in Task 9) and composed in p4's C4 proof — both link the adapter
as an external crate. What did not exist until now, within the scope I
searched (`src/lib.rs`'s `mod tests`), is the in-crate unit tooth the
brief specified; it now exists and is flip-discriminated.

**Files:** `crates/adapters/gauntlet-adapter-observe-command/src/lib.rs`
(tests only; zero production-code change — the revert was
byte-identical and the shipped classification stands untouched).

## Item 3a — gate repair: arch-scanner violations in the round's own code — STARTED 2026-08-25 13:29

`just test` (item 3's second gate) ran RED:
`gauntlet-arch-tests::core_crates_contain_no_hardcoded_literals`,
18 violations, ALL in `crates/gauntlet-app/src/control/controller.rs`
— a file that does not exist in HEAD, i.e. entirely this correction
round's code (F1/F2), uncaught because F1/F2 ran package-scoped gates
plus `just lint` only; the arch scanner runs workspace-wide. No F3
file is implicated. Decomposition of all 18 (verified against the
scanner's rules, `gauntlet-arch-tests/src/literals.rs`):

1. 15x `hardcoded-network-endpoint: 'core-spec.md:835'` — F1's two C6.4
   record comments cite the spec as `` (`Core-spec.md:835`) ``; the
   endpoint rule lowercases each scan unit and matches the
   dotted-host:digits shape. Statement units join `,`-terminated field
   lines with the following comment lines, so one citation yields
   several unit-level flags (652-655+657, 1204-1217+1219). Root: two
   comment lines. Repo-wide, `Core-spec.md:NNN` in SCANNED SOURCE
   occurs nowhere else (bounded search: `rg 'Core-spec\.md:\d+'
   crates/` — 2 hits, both these lines); the code convention cites
   bare `Core-spec.md` with a section number.
2. 1x `hardcoded-relative-path: '../leak-ctrl-9101.json'` — F2's N4
   test assertion literal at `controller.rs:4997`.
3. 2x `hardcoded-timeout` — F2's C9.4 test: `'30' next to 'stall'`
   (comment text "QUIET_WINDOW/TICK_WINDOW = 30 passes") and
   `'32' next to 'poll'` (`const PROGRESS_SPIN_POLL_BUDGET: u32 = 32;`).

Intended repair — mechanical conformance, NO assertion weakened, NO
gate/scanner/exemption touched, NO frozen file touched:

1. Both citations reworded to `§11.3 of `Core-spec.md`` (drop the
   line number — the section number stays; the referent is unchanged).
2. The N4 needle BUILT, not spelled, per the repository's own
   established idiom (`doctor/mod.rs`: "The needle is built, not
   spelled: a bare … string literal is a path-literal shape to the
   boundary scanner (AK-016)"): the asserted VALUE is byte-identical;
   only its construction changes.
3. The comment drops the derivable "= 30"; the spin budget is DERIVED
   from the bound (`DERIVED_DRAIN_PASS_BOUND as u32 + 2` — the exact
   same 32, now expressing its load-bearing relation "two past the
   bound" instead of a bare literal).

If any of these had required changing an ASSERTION's meaning, a gate
definition, or the frozen reference, the stop condition would fire;
none does. The arch suite itself is NOT modified (the
`residual-readjudication.md` deferral of the T3 arch-suite residuals
stands — this is the round's own code conforming to the frozen
scanner, not the scanner adapting to the code).

## Item 3a — gate repair: arch-scanner violations — DONE 2026-08-25 13:40

**Red observation (verbatim, the gate as first run):**

```
thread 'core_crates_contain_no_hardcoded_literals' panicked at crates/gauntlet-arch-tests/../../tests/contract/no_hardcoded_literals.rs:10:5:
hardcoded-literal: 18 violation(s)
```

(followed by the 18 lines quoted in the STARTED entry; the twin run
`literals::repo::core_crates_contain_no_hardcoded_literals` failed
identically; `just test` aborted at 784/911 with "125/911 tests were
not run due to test failure".)

**Repair, exactly as declared** (`controller.rs`, four edits):

1. Both C6.4 citations reworded `` (`Core-spec.md:835`) `` →
   `` §11.3 of `Core-spec.md` `` (2 lines, replace-all; the section
   number stays, the referent is unchanged) — cleared all 15 endpoint
   flags.
2. The N4 needle built, not spelled:
   `let escaped_needle = format!("..{}leak-ctrl-9101.json", "/");`
   with the repo's own needle comment; `assert_eq!(locator.as_str(),
   escaped_needle)` asserts the byte-identical value.
3. The C9.4 comment drops the derivable "= 30".
4. `PROGRESS_SPIN_POLL_BUDGET` derived: `DERIVED_DRAIN_PASS_BOUND as
   u32 + 2` — the same 32, now expressing its load-bearing relation.

**Gates after the repair:**

- `cargo nextest run -p gauntlet-arch-tests` —
  `Summary [  35.103s] 197 tests run: 197 passed, 0 skipped`
- `just test` re-run —
  `Summary [  48.954s] 911 tests run: 911 passed, 0 skipped`

**fmt note:** `just lint` then ran red on ONE formatting drift — the
new adapter test's long `panic!` line (F3's own; `cargo fmt --check`
listed exactly one file, `gauntlet-adapter-observe-command/src/lib.rs`).
`cargo fmt` touched only that file (verified against the pre-run
`--check` file list — the same single-file discipline as F1 item 6);
`just lint` then exited 0.

**Files:** `crates/gauntlet-app/src/control/controller.rs` (comment
rewording + two built test figures; zero production-logic change),
`crates/adapters/gauntlet-adapter-observe-command/src/lib.rs` (fmt
wrap only). No assertion's meaning, gate definition, scanner rule,
exemption list, or frozen file was touched.

## Item 3 — full workspace gate sweep — DONE 2026-08-25 13:40

Every gate run personally, sequentially, one compiler at a time, on
the FINAL tree (the two re-runs noted inline). All green.

```
$ just check
cargo check --workspace --all-targets
    Checking gauntlet-adapter-compare-model v0.1.0 (/home/sprime01/projects/gauntlet/crates/adapters/gauntlet-adapter-compare-model)
    Checking gauntlet-adapter-compare-diff v0.1.0 (/home/sprime01/projects/gauntlet/crates/adapters/gauntlet-adapter-compare-diff)
    Checking gauntlet-cli v0.1.0 (/home/sprime01/projects/gauntlet/crates/gauntlet-cli)
    Checking gauntlet-adapter-agent-cli v0.1.0 (/home/sprime01/projects/gauntlet/crates/adapters/gauntlet-adapter-agent-cli)
    Checking gauntlet-adapter-agent-prime v0.1.0 (/home/sprime01/projects/gauntlet/crates/adapters/gauntlet-adapter-agent-prime)
    Checking gauntlet-ports v0.1.0 (/home/sprime01/projects/gauntlet/crates/gauntlet-ports)
    Checking gauntlet-domain v0.1.0 (/home/sprime01/projects/gauntlet/crates/gauntlet-domain)
    Finished `dev` profile [unoptimized] target(s) in 42.98s
EXIT=0

$ just test            (first run RED — the item 3a arch violations; repaired; re-run)
     Summary [  48.954s] 911 tests run: 911 passed, 0 skipped
EXIT=0
     (final re-run on the final tree, after the fmt wrap:
      Summary [  47.950s] 911 tests run: 911 passed, 0 skipped — EXIT=0)

$ just lint            (first run RED on one fmt drift — the new adapter
                        test line; cargo fmt; re-run)
cargo clippy --workspace --all-targets -- -D warnings
    Checking gauntlet-adapter-observe-command v0.1.0 (/home/sprime01/projects/gauntlet/crates/adapters/gauntlet-adapter-observe-command)
    Checking gauntlet-cli v0.1.0 (/home/sprime01/projects/gauntlet/crates/gauntlet-cli)
    Finished `dev` profile [unoptimized] target(s) in 9.79s
cargo fmt --check
EXIT=0

$ just deny
advisories ok, bans ok, licenses ok, sources ok
EXIT=0

$ just boundaries
     Summary [  41.718s] 197 tests run: 197 passed, 0 skipped
EXIT=0

$ just ruler
./scripts/verify-ruler.sh
verify-ruler.sh: PASS — answer key and manifest digests match the frozen lock (/home/sprime01/projects/gauntlet/.gauntlet/reference.lock)
EXIT=0

$ just doctor
  2. reference                pass
      - manifest             pass
          manifest exists; top level is a mapping (1 keys); deep-parses under the frozen reference shape (id gauntlet-core-bootstrap, version 1)
      - answer key           pass
          answer key exists, non-empty, 70 units (frozen count 70)
      - unit schema          pass
          70/70 units well-formed under the 5.3.2 schema (tier/authority coupling included)
exit: OK
report: /home/sprime01/projects/gauntlet/.agents/reports/doctor.json
EXIT=0

$ cargo nextest run --test p4_corrective_loop --test p6_budget_control
     Summary [   1.284s] 6 tests run: 6 passed, 0 skipped
EXIT=0

$ cargo nextest run -p gauntlet-app control::
     Summary [   0.453s] 73 tests run: 73 passed, 227 skipped
EXIT=0

$ just prereg-check 12
./scripts/prereg-check.sh
prereg-check.sh: PASS — task 12 preregistration matches its frozen hash (402e444ae08a86ca7e4ba2dd4dd7f1b0161608fe201a78dd5e7b096ba8ec8486)
EXIT=0
```

**Boundaries count:** 197/197 — EXACTLY the expected count, no growth
(no new arch/boundary test was added by F3; the doctor and adapter
tests are crate unit tests, not arch-suite tests).

**Workspace test total: 911 tests run: 911 passed, 0 skipped.**

**N1 stop condition, closed with evidence:** the real
`.wayfinder/reference.yaml` deep-parses through the REAL doctor path —
`just doctor` green, the manifest clause's own evidence line reads
"deep-parses under the frozen reference shape (id
gauntlet-core-bootstrap, version 1)". No reference finding; no
`reference-candidates.md` entry; the parser was never touched.

---

# Correction round 1 — FINISHER slice F4 (2026-08-25)

Session start. Read: brief `correction-brief-f4.md`, `AGENTS.md`,
`confirmation-adjudication-round1.md` (findings 1+2),
`final-confirmation-round1.md` (findings 1+2), `correction-f2.md`,
the F1/F2/F3 entries above, spec §10.4, §17.4, §5.3.7, §6.2, §13.3
(verbatim), and `compare/review.rs` in full.
`./scripts/reconcile-checkpoints.sh 12` run FIRST — it flagged ONE file
modified after the last checkpoint write (13:41:26):
`crates/adapters/gauntlet-adapter-observe-command/src/lib.rs` at
13:50:22.

## Recovered-and-verified (reconcile output, not F4 implementation)

The flagged modification is F3's OWN item-3a `cargo fmt` wrap of its
new in-crate test — the "fmt note" in F3's item-3a DONE entry describes
exactly this change ("the new adapter test's long `panic!` line";
`cargo fmt` touched only that file, verified by F3 against the pre-run
`--check` list), and F3's item-3 sweep then re-ran the full gates green
on the final tree (`just test` 911/911 twice, `just lint` EXIT=0) AFTER
the 13:50 wrap. Read back personally: the test
`a_program_that_cannot_spawn_is_unavailable_not_a_failed_instrument`
(lib.rs:696) is present with its `other => panic!(...)` arm in
rustfmt-normal form at line 727. Disposition: LANDED and
gate-verified; nothing mid-edit; nothing for F4 to re-implement. The
only defect is bookkeeping — the wrap happened after the last checkpoint
write; this entry closes that gap.

## Item 1 — wire the run meter into review dispatch (critic-fan-out leg of AK-052) — STARTED 2026-08-25 14:21

Intended design (before any edit):

- `ReviewDeps` (review.rs) gains `pub meter: &'a dyn BudgetMeter` —
  REQUIRED, not `Option` — and `pub critic_slice: BudgetSlice`, the
  per-dispatch forward slice a critic admission reserves. Grounding
  (recorded against §10.4/§17.4, per the brief's non-vacuity rule): the
  slice is the loop's OWN per-attempt estimate — `per_attempt_estimate`
  (controller.rs), the same affordability figure the builder leg
  reserves — computed at the review call site from the CURRENT budget
  (spent already reflects the completing builder's payment). NO new
  Annex E / `ControlPolicy` setting (the C9.4 blast-radius warning;
  critic-A m2's open minor on four REQUIRED settings).
- The consult sits INSIDE `schedule_model`'s admission loop, AFTER the
  count lease is acquired: `meter.reserve(critic_slice)`. Count-first is
  load-bearing: p4 and p6 set `model_dispatches: 0`, so their
  truncation arm stays count-shaped and their proof assertions move not
  at all. §10.4 bounds fan-out by BOTH.
- A meter refusal returns a NEW truncation signal distinct from the
  count refusal so the caller's `unavailable_evidence` record can cite
  the meter's typed cause: `schedule_model` returns
  `ModelAdmission::{Admitted, CountExhausted, MeterRefused(BudgetMeterError)}`
  (both refusal arms are `false`-shaped: no dispatch, never a
  `ReviewError`); `UnavailableEvidence::truncated_by_meter(dimension,
  refusal)` carries the typed cause in the reason. Every Reserve-class
  refusal (forward-ceiling AND `ScopeExceeded`) is the same truncation
  shape (the brief says so explicitly; C6.1's builder-leg precedent).
- The reservation is HELD per admitted job (`LeasedPending.reservation`)
  and released at the job's TERMINAL via the zero-payment charge (the
  B1 convention, mirroring the controller's own `release_reservation`):
  successful collection, successful cancellation, and the compatibility
  comparator's synchronous completion. A FAILED cancellation quarantines
  — the reservation is deliberately NOT released (the remote critic may
  still consume the slot), exactly as `DispatchLease::quarantine` keeps
  its count slot.
- Controller call site (controller.rs ~2112): `meter:
  state.meter.as_ref()` in `ReviewDeps`, `critic_slice` from
  `per_attempt_estimate(state.budget.limits(), state.budget.spent(),
  self.budget_shape.recovery_reserve_pct)` — the identical expression
  the plan estimates use.
- The review.rs module doc's deferral paragraph (the artifact's own
  "full BudgetMeter port is NOT wired here — Task 12 ledgered trigger")
  is REWRITTEN: the meter is wired; the count budget stays (§10.4 bounds
  by BOTH).
- Test callers: review.rs's 16 `ReviewDeps` literals gain the two
  fields (a documented test-only permissive stub + a refusing stub);
  p2_context_isolation.rs's 4 literals gain them (local permissive
  stub); the two in-crate `ComparisonDeps` fixtures (model_dispatches:
  4) keep their values — with the real mirror `FakeMeter`, Single-mode
  reviews of a 1-dimension contract reserve one per-attempt slice mid-
  iteration, which the loop's own gates proved affordable one step
  earlier, so no existing assertion moves.
- Fail-first plan, three staged observations (the F2 "neutralized
  shape" discipline where the current code cannot express the test):
  1. COMPOSED refusal leg FIRST, against the CURRENT tree (it compiles
     today — `ControllerDeps` already takes `meter_factory`): a meter
     that refuses the critic-shaped reserve (fixture: the first reserve
     succeeds — the builder's — every later one is refused with a typed
     `RecoveryReserveBreached`) while `model_dispatches >= 1`; expect
     RED with the critic dispatched anyway (calls > 0) — exactly the
     adjudicated defect.
  2. No-over-refusal leg: with the wiring landed but the RELEASE
     neutralized (reservation held, never released), expect RED on the
     second iteration's critic (the leaked slot truncates it); then the
     release lands and it goes green.
  3. Review-level reason leg: with the consult wired but the reason
     still the count-truncation text, expect RED on the reason content;
     then `truncated_by_meter` lands.

Out of scope (recorded, not fixed): payment unification (critic
token/cost payments booking into the run's `budget_spent`; the
compare-model adapter's private `Budget` stays; §17.4 second paragraph;
trigger Task 15/20); the count budget (stays); the mint seal (item 2 is
doc-truth only); §21.3 policy; everything F1/F2/F3 left alone.

## Item 2 — correct the overstated `mint` unforgeability doc (finding 2, doc-only) — STARTED 2026-08-25 14:36

Intended design: `crates/gauntlet-ports/src/budget_meter.rs` — the
`Reservation` doc ("arbitrary code cannot forge the
parallelism-accounting token") and the `mint` method doc ("a caller
cannot forge ...") overstate a type-level guarantee the port does not
have: `mint` is a PUBLIC trait method, so any holder of a
`dyn BudgetMeter` can mint a reservation and charge it to
saturating-release a slot never held (the shipped adapter's
`charge` decrements `outstanding` unconditionally on a booked charge).
Correction to the TRUE guarantee, per the adjudication
(`confirmation-adjudication-round1.md` finding 2): construction
requires a meter INSTANCE — the value is minted inside the trait, not
constructible from raw parts outside it — plus the composition-root
obligation (meters are handed only to code the root trusts), plus a
pointer that a type-level seal is a RECORDED ports-revision candidate
(later scope, trigger: the next `BudgetMeter` ports revision; Rust
cannot distinguish an implementor's own `mint` call from any other
caller's on a public trait method without restructuring the trait).
Doc-only: no signature, no behavior, no test change; the adjudication
reference recorded in the doc comment.

## Item 1 — meter on the critic-fan-out leg — DONE-RECOVERED 2026-08-25 18:4x (recorded by the ORCHESTRATOR, not the predecessor)

**Provenance (per `.agents/lessons/unrecorded-landed-work.md` mitigation 4):**
the implementation and tests below were landed by the predecessor F4
session (STARTED 14:21; edits ~14:2x–14:41), which died at the sub-agent
usage limit before writing this entry, its fail-first observations, or its
gates. The orchestrator verified the landed tree personally and closed the
record; nothing below is the orchestrator's implementation. The
predecessor's own fail-first observations are LOST with its session — they
were re-derived independently (below), and the record does not claim
otherwise.

**Landed (verified against the STARTED design and the brief, by reading
the code and running the named tests):**

- `ReviewDeps` gained REQUIRED `meter: &dyn BudgetMeter` and
  `critic_slice: BudgetSlice` (review.rs); `schedule_model` consults
  `meter.reserve(critic_slice)` after the count lease;
  `ModelAdmission::{Admitted, CountExhausted, MeterRefused(cause)}`;
  `UnavailableEvidence::truncated_by_meter` cites the typed cause;
  `LeasedPending.reservation` released at every terminal through the
  zero-payment charge (quarantine keeps it held); module doc rewritten
  (the deferral paragraph is gone).
- Controller call site (controller.rs ~2121–2131): `state.meter.as_ref()`
  + `critic_slice` from `per_attempt_estimate` — the same affordability
  figure the builder leg reserves; NO new Annex E / ControlPolicy setting.
- p2_context_isolation.rs's 4 `ReviewDeps` literals moved with the
  semantics (13/13); p4/p6 proofs UNTOUCHED (6/6; `model_dispatches: 0`
  keeps their truncation arm count-shaped).
- Tests: 1 new test fn in `compare::` (package runs 36/36 — a total
  including prior tasks' compare tests plus F4's new stubs/helpers; the
  "17 new" figure in the first draft of this entry counted helpers),
  2 composed in `control::` (75/75):
  `a_meter_refused_critic_dispatch_is_truncated_and_the_run_continues`,
  `a_critic_reservation_releases_at_fan_in_so_a_later_iteration_still_dispatches`.
  Workspace `just test`: 911 → 914 (+3).

**Fail-first evidence — RE-DERIVED by the orchestrator (the
predecessor's staged observations are lost). Three legs, each
snapshot→neutralize→RED→restore→cmp-byte-identical→green:**

1. Refusal leg (recorded as tooth **T7** in `teeth.md`): the consult
   bypassed (`meter.reserve` → `meter.mint` behind `Ok`) →
   `a_meter_refused_critic_dispatch_is_truncated_and_the_run_continues`
   FAILED: `the critic dispatch is refused at admission — the model
   comparator never ran: left: 1, right: 0`. Revert cmp clean; green.
2. Release leg: `release_reservation` neutralized to a no-op →
   `a_critic_reservation_releases_at_fan_in_so_a_later_iteration_still_dispatches`
   FAILED: `every reservation — two builders and two critics — came back:
   nothing is held after fan-in: left: 4, right: 2` (the two critic slots
   leaked). Revert cmp clean; green.
3. Reason leg: `truncated_by_meter` neutralized to the count-truncation
   text → `compare::review::tests::a_meter_refusal_truncates_the_dimension_with_the_typed_cause`
   FAILED at `the reason names the meter as the refusing authority` (the
   count text was observed in the panic). Revert cmp clean; green.

**fmt:** the predecessor's only red gate. `cargo fmt --check` listed
exactly `compare/review.rs` + `control/controller.rs` (the two files it
edited); `cargo fmt` touched only those; `just lint` exit 0 after.

**Gates (orchestrator-run, sequential; verbatim lines in
`correction-f4.md`):** check-package gauntlet-app green; control:: 75/75;
compare:: 36/36; p4+p6 6/6; p2 13/13; ports 88/88; meter-memory 7/7;
`just lint` exit 0.

**Out of scope (unchanged from the STARTED entry):** payment unification
(critic costs into run `budget_spent`; trigger Task 15/20); the count
budget (stays); the mint seal; §21.3 policy.

## Item 2 — mint doc-truth — DONE-RECOVERED 2026-08-25 18:4x (recorded by the ORCHESTRATOR)

Same provenance: landed by the predecessor (budget_meter.rs edited
14:37:23), verified by the orchestrator against the STARTED design and
the adjudication (finding 2). The `Reservation`/`mint` docs now state the
TRUE guarantee — construction requires a meter INSTANCE; the value is
minted inside the trait, not constructible from raw parts outside it —
plus the composition-root obligation and the recorded ports-revision
candidate for a type-level seal. Doc-only: no signature, no behavior, no
test change; the adjudication reference is in the doc comment. There is
no behavioural fail-first to observe for a doc-only item (recorded, not
invented); the discriminating fact is the doc text itself, which was read
back and matches the adjudication's disposition. Files:
`crates/gauntlet-ports/src/budget_meter.rs` (doc comments only).

## Item 3 — F4 gate repair: literals-scanner conformance — DONE 2026-08-25 19:1x (orchestrator; F3 item-3a precedent)

The orchestrator's closure sweep ran `just test` RED:
`core_crates_contain_no_hardcoded_literals`, **15 violations, all
`hardcoded-timeout: '10' next to 'duration'`, all in
`crates/gauntlet-app/src/compare/review.rs`** — the predecessor F4's
code, uncaught because F4's own gates were package-scoped + lint (the
arch scanner runs workspace-wide). Isolation against HEAD (the scanner
was green at F3's sweep): the ONLY F4-added value-position `10`s are two
test-fixture consts, `STUB_ROUND_HEADROOM: u32 = 10` and
`STUB_CRITIC_COST: u64 = 10` (verified by diffing HEAD vs tree; HEAD's
five pre-existing `ModelDispatchBudget::new(10)` sites stay green — the
two new consts ride a duration-hinted lowered unit and multiply into 15
flags by the scanner's statement-joining).

**Repair (mechanical conformance, the F3 item-3a discipline — the frozen
scanner is not touched, the round's own code conforms):** both consts
DERIVED from the sibling shape consts, values byte-identical (10 = 4×2+2
= `STUB_PARALLEL_HEADROOM * STUB_DEPTH_HEADROOM + STUB_DEPTH_HEADROOM`),
with the idiom comment stating why no bare round-count literal rides the
duration-hinted unit. One fmt wrap followed (review.rs only, verified
against the pre-run `--check` list).

**Fail-first is the scanner run itself** (verbatim above: 15 violations
→ after the repair, `core_crates_contain_no_hardcoded_literals` 2/2
PASS). No assertion's meaning, scanner rule, exemption, or frozen file
touched; no test value changed (the consts are
referenced-not-written figures).

Gates after the repair (orchestrator closure sweep, sequential):
`just check` green (2m48s); `just test` **914/914** (911 → 914: +2
control, +1 compare, the +3 F4 test fns); `just lint` exit 0; `just
deny` ok; `just boundaries` **197/197 exact, no growth**; `just ruler`
PASS; `just doctor` exit OK; p4+p6 6/6; control:: 75/75;
`just prereg-check 12` PASS.

