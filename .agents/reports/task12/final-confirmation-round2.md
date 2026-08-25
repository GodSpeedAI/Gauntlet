# Task 12 — fresh blind confirmation, round 2

Critic: fresh context, round 2. Read nothing under .agents/reports/ other than
the confirmation brief; no git history was consulted; no builder reasoning,
attempt counts, prior findings, or correction adjudications were read. (Some
artifact source files self-describe prior corrections in comments; those
comments were read as part of the working tree and were not chased further.)

## What was confirmed

Task 12 of .agents/plans/gauntlet-impl.md (P4 corrective loop, P6 budget
control; AK-037, AK-038, AK-039, AK-051, AK-052) against Core-spec sections
5.3.2/5.3.7, 10.3/10.4, 11.1-11.6, 12.3, 13.5, 15.1/15.2, 17.1/17.4,
18.2-18.4, 21.2/21.3, 23.1 and the frozen answer-key units, through the
composed production path (RunController over the real metered-budget adapter,
real embedded-SQL state store, real fs evidence store, real file reference
provider, real command observer, real diff comparator, governed
compare/verify chains) as materialized by the p4/p6 proofs at the composition
root.

## Executed evidence (one cargo command at a time, package-scoped)

- cargo nextest run -p gauntlet-cli --test p4_corrective_loop --test p6_budget_control
  -> 6 tests run: 6 passed, 0 failed.
- cargo nextest run -p gauntlet-app control:: -> 75 tests run: 75 passed,
  0 failed (nextest also reported 228 skipped under the same filter; I could
  not fully attribute that count within budget and did not chase it; 0 failed
  is the gate-relevant fact).
- cargo nextest run -p gauntlet-adapter-meter-memory -> 7 tests run: 7 passed
  (includes the shared budget-meter contract suite).

## Findings (severity-ranked; nothing above MINOR)

### MINOR-1 — a model rung's runner dispatch bypasses the budget meter (latent composition seam)
- Claim touched: AK-052 'consulted before every dispatch', under a future
  composition that injects a model-judgment rung.
- Where: crates/gauntlet-app/src/verify/ladder.rs:509-514 (ModelRung::verify
  invokes its runner with no meter.reserve / would_breach_reserve consult;
  the spec's budget_slice is data handed to the runner, not a gate the
  controller enforces). Contrast the two metered legs: builder dispatch at
  controller.rs:1455 (reserve after the summed consult at 1272-1281) and
  critic fan-out at compare/review.rs:630.
- Failure scenario: a Task-20 composition root builds a ladder containing a
  ModelJudgment rung and injects it via ControllerDeps.ladder; a claim that
  falls through the deterministic rungs then dispatches a verifier-role model
  invocation whose token spend never passes the recovery-reserve gate.
- Why MINOR, not higher: not reachable in the shipped tree — Ladder::new and
  ModelRung::new are feature-gated to test/composition (ladder.rs:449, 475),
  no production code constructs a ladder (grep over non-test code shows the
  only construction sites are tests/proofs), and crates/gauntlet-cli/src/main.rs
  composes no controller yet. Within the Task-12 composed path (builder +
  critic dispatches), every dispatch consults the meter, bindingly. Flag for
  Task 20's composition review.

### MINOR-2 — AK-052's frozen evidence locator names no literal test
- Claim touched: evidence-locator fidelity only (the capability itself is
  proven).
- Where: .wayfinder/answer-key.md:367 locator tests/contract/budget::reserve_protected;
  grep over the repo finds that string only in the ruler and Core-spec. The
  capability is proven by the shared suite (crates/gauntlet-ports/src/contract/budget_meter.rs,
  incl. reserve_consuming_the_recovery_reserve_is_refused, run by the
  gauntlet-adapter-meter-memory contract tests) and by p6's
  the_meter_refuses_the_reserve_consuming_slice_and_books_nothing.
- Scenario: an auditor following the frozen locator verbatim finds no such
  test path. Same frozen-locator vs cargo-target class the proofs' headers
  already document (recorded against AK-049, 2026-08-17). Same applies to
  AK-039's tests/contract/diagnosis::nonconvergence (in-crate
  control::diagnosis tests) and AK-037's
  tests/contract/controller::records_alternatives
  (control::select::tests::controller_records_alternatives).

### MINOR-3 — the loop-path 15.1 candidate space is fixed at two shapes
- Claim touched: AK-037 breadth (the record is true and non-vacuous; the
  space considered is constant).
- Where: controller.rs:2575-2606 (record_selection always offers the chosen
  bounded re-dispatch/completion versus the speculative clean-workspace
  sibling for the same unit).
- Scenario: every loop-path selection record has the same two-candidate
  shape regardless of project state; the alternatives are genuinely ranked
  by the real selector with typed reasons (the domain refuses an empty
  rejected list, correction.rs:455), and the numbers recorded are the
  numbers the decision used (selection_facts reads meter.remaining() at
  decision time), so the record is not false — but it is also never richer
  than two. The proof supplements with a three-candidate composed selector
  test (p4_corrective_loop.rs:1462). Documented in the proof itself.

### MINOR-4 — a mid-run answer-key read failure errors the run instead of degrading
- Claim touched: 21.3 row 1 (config/reference invalid: stop new dispatch,
  keep the process alive, keep reconciliation running) on the 11.4 step-3
  re-read path.
- Where: controller.rs:2826-2834 (strict_units maps a read failure to
  Reference Unavailable) which propagates as a typed Err out of drive(),
  e.g. via drain step 3 at controller.rs:1733 — unlike the boundary_verify
  Unavailable arm (controller.rs:1079-1101) which pins a gap and continues.
- Scenario: the answer-key file is unreadable for one tick (transient IO);
  the run returns Err (no terminal RunOutcome) rather than recording the gap
  and continuing reconciliation. Typed error, nothing converted into
  success, durable state preserved — a degradation-path deviation only.

### MINOR-5 — p6's module doc mischaracterizes p4's diagnosis-test mechanism
- Claim touched: documentation accuracy only (the composed claim holds).
- Where: p6_budget_control.rs:82-85 says p4's recurrence test works 'over a
  pinned-clock observer (byte-stable facet evidence ...)'; the actual test
  (p4_corrective_loop.rs:1376-1390) passes absent_program=true — recurrence
  counts through STABLE unavailable-evidence gap keys
  (gauntlet-adapter-compare-diff: observed 'unavailable:<kind>'), while a
  failing-but-present facet embeds the per-iteration content-addressed
  evidence_ref in its observed string and therefore never counts.
- Corollary worth recording: a never-fixing builder whose observation facets
  are time-varying terminates by budget exhaustion, not diagnosis. That is
  faithful to 13.5's residual identity (dimension, expected, observed
  locator) and pinned by the diagnosis test
  different_observed_locators_do_not_accumulate_one_recurrence_count.

### MINOR-6 — evidence production binds the FIRST facet's bytes to every attempted unit
- Claim touched: producer-binding granularity on multi-attempt payloads.
- Where: controller.rs:2162-2220 (assembled.doc.facets().first() is written
  at each attempted unit's required locator).
- Scenario: a builder result naming multiple attempted units gets the same
  first-facet bytes at every unit's locator; verification then decides on
  those bytes honestly (fail/inconclusive if wrong — no false pass), but the
  per-unit evidence binding is coarse. No shipped proof exercises a
  multi-unit attempted list.

## What was checked and held (with the load-bearing reasons)

- AK-052 bindingness: the only Reservation source on the composed path is
  meter.reserve (dispatch is private; InFlight requires a reservation);
  three layers consult the meter — scheduler eligibility conjunct 6
  (scheduler.rs:562-575, at call time), the summed pre-dispatch consult
  before EVERY dispatch including later members of one plan
  (controller.rs:1272-1281), and reserve() itself (adapter lib.rs:166-192,
  domain arithmetic). The accounting quantity cannot be produced outside the
  meter on this path: charges go only through meter.charge / budget.charge,
  and the zero-payment release convention is the adapter's documented one.
  mint() is a public trait method (any meter holder can mint a slot token) —
  acknowledged in the port's own doc; the recovery reserve is NOT consumable
  that way (charges still pass the domain gate), so AK-052's substance
  stands.
- AK-051: terminal budget_exhausted, not a crash — rounds are booked at the
  iteration boundary on the run Budget (single rounds authority), the
  explicit rounds gate precedes any dispatch (controller.rs:1254-1259), and
  the domain charge path is a second, independent reserve guard. p6 asserts
  all four dimensions at or under the forward ceilings from the durable
  checkpoint, escalation_summary None, rows == reservations == 5.
- AK-037: records exist on BOTH loop paths (residual-driven and completion),
  durably, through the sqlite store's round-trip (state_store.rs:1744-1786
  plus its round-trip test); non-vacuity is a type invariant
  (correction.rs:455); recorded numbers equal decision numbers.
- AK-038: the corrective iteration is ROUTED — disposition row 1 decides
  retry-with-residual-text (disposition.rs:400-423), the re-dispatch prompt
  carries residual text only (asserted on the real prompt), and the second
  iteration's PASS settles the unit through the governed chain. The proof's
  scripted runner scripts the BUILDER's behavior, not the controller's
  decision.
- AK-039: recurrence accumulates through the real comparator on stable keys;
  diagnosis is bounded per cycle (five phases, allowance divided per phase)
  and across cycles (re-entry spends the finite diagnostic allowance until
  charge_diagnostic refuses -> INCONCLUSIVE); INCONCLUSIVE, quarantine-as-
  pinned-evidence, and escalation are all composed (p4 third test) and
  pinned in-crate (diagnosis tests incl.
  budget_exhaustion_yields_inconclusive_quarantine_and_escalation).
- 18.2/18.3: the Checkpoint struct carries every spec-listed field with real
  values (event-log offset threaded from actual appends — pinned by an
  in-crate test; budget_spent from the run Budget; in-flight from live
  state); placement at the top of iteration k carrying k-1's outcomes
  satisfies 11.1's end-of-iteration checkpoint under the documented C8.2
  reading, and p6 counts exactly forward-rounds + 1 checkpoints.
- 11.4 step 3 for QUEUED units: every field the file can express is
  authoritative each tick (fresh strict re-parse feeds eligibility); the
  status field is inexpressible in the shipped grammar, which is a bounded
  claim pinned by test (the_answer_key_cannot_express_a_unit_status_on_any_shipped_surface),
  not an assertion of universal absence.
- 21.3 abort discipline: the only aborts are reference mutation and contract
  mismatch — both 11.6 integrity rows. Reference unavailability at the
  boundary is a pinned gap, not an abort.
- Proof honesty (23.1): assertions are substantive (durable read-backs,
  counts at the real store, prompt content, reserve arithmetic from the
  durable checkpoint, deduplication-aware record counting); the mid-run-edit
  test fails if the re-read is removed (the stall path would re-dispatch
  instead of cancelling). Teeth re-runs are the orchestrator's, out of my
  scope.

## Search bound

I read, in full or at the named granularity: the confirmation brief; the
listed Core-spec sections; the five AK entries; the plan's Task 12 section;
AGENTS.md; control/{controller,scheduler,select,diagnosis,disposition}.rs
production code in full; domain correction.rs and budget.rs production code
in full; ports budget_meter.rs and state_store.rs in full;
ports/contract/budget_meter.rs at the reserve-case level; adapters
meter-memory (full), state-sqlite (checkpoint/correction-selection surface);
adapter-compare-diff (residual-key derivation); verify/ladder.rs and
verify/authority.rs and compare/review.rs at the dispatch/verify-claim
regions; both proof tests in full. I did NOT examine: the mock/prime/cli
agent runner internals, the remaining observers beyond the command observer,
the remaining verify/compare module bodies, gauntlet-arch-tests, the CLI
main, any other task's reports, CURRENT_STATUS, preregistrations, lessons,
roles, skills, .tmp, .jolli, or any git history. All claims above are
bounded to that set. Threat model: composition-level bypass hunting by
reading and gate execution; no mutation/teeth runs (orchestrator's scope),
no fuzzing.

## Verdict

CONFIRM. Within the declared bound, the five AK claims and P4/P6 hold through
the composed production path — the mechanisms are reachable and load-bearing
on the path the shipped controller actually takes, verified by reading the
composition and by executing the Task-12 gates green (6/6 proofs, 75/75
control tests, 7/7 meter adapter tests). Nothing I found rises above MINOR.
