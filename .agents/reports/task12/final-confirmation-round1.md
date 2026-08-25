# Task 12 - fresh blind confirmation, round 1

Date: 2026-08-25. Critic: fresh context; no builder reports, prior critic
findings, attempt counts, or preregistrations were read. Git history was not
inspected.

## What was confirmed

Task 12 ("Controller: selection, budget, diagnosis") claiming P4 (corrective
loop), P6 (budget control), and answer-key Units AK-037, AK-038, AK-039,
AK-051, AK-052, through the composed production path
(gauntlet-app::control::controller over injected ports), per
.agents/specs/Core-spec.md (normative), .wayfinder/answer-key.md (frozen
ruler, read verbatim), .agents/plans/gauntlet-impl.md Task 12 section, and
AGENTS.md ("Existence is not composition").

## Executed evidence

- cargo nextest run --test p4_corrective_loop --test p6_budget_control:
  6/6 PASS (nextest run ID 46aba52a, 2026-08-25).
- cargo nextest run -p gauntlet-app control:: : 73/73 PASS.
- cargo nextest run -p gauntlet-adapter-meter-memory: 7/7 PASS (includes the
  shared budget-meter contract suite, in-crate and integration runs).

## Findings (severity order)

### 1. MAJOR - AK-052: the meter is NOT consulted before EVERY dispatch; the critic-fan-out leg is count-bounded only, and its meter wiring was deferred by the artifact itself to Task 12, which did not compose it

- Claim broken: AK-052 ("The budget meter is consulted before every dispatch
  and refuses any dispatch that would consume the recovery reserve"), plan
  Task 12 step 3 ("would_breach_reserve consulted before every dispatch"),
  spec 10.4 ("Critic fan-out is bounded by review.max_critics AND by the
  budget meter; a fan-out that would breach the recovery reserve is
  truncated"), 17.4 ("consulted before every dispatch, not after"), 5.3.7
  ("refuse to dispatch ANY invocation").
- Where:
  - crates/gauntlet-app/src/compare/review.rs:32-37 - the deferral record in
    the artifact itself: "this module counts model dispatches against a
    plain typed budget; the full BudgetMeter port (reserve, charge,
    would_breach_reserve, spec 5.3.7/6.2) is NOT wired here - its
    composition is plan Task 12 ledgered trigger." Task 12 is the task under
    confirmation; the trigger was not closed.
  - crates/gauntlet-app/src/control/controller.rs:2106-2130 - the composed
    loop calls governed.issue(...).review with ModelDispatchBudget::new(
    self.comparison.model_dispatches). ModelDispatchBudget is a COUNT of
    dispatches (compare/review.rs:101-148), not a meter consult.
    ComparisonDeps (controller.rs:200-207) carries no meter field, so no
    composition root could wire the run meter into review dispatch without
    changing the controller.
  - crates/adapters/gauntlet-adapter-compare-model/src/lib.rs:87 and
    146-168 - ModelComparator holds its own private budget_slice: Budget; a
    critic invocation payment books there, never against the run budget
    (state.budget) or the meter. The 18.2 checkpoint budget_spent therefore
    under-reports orchestration spend, contrary to 17.4 ("Payment includes
    verification cost and orchestration burden, not just builder tokens").
- Concrete failure scenario (inputs/state to wrong outcome): adaptive review
  (the default) with model_dispatches > 0 and a CLEAN deterministic probe -
  review.rs:16-19 makes this the guaranteed shape for any conforming
  dimension ("a clean probe settles nothing... the dimension stays undecided
  and fans out to a model critic"; it is exercised-up-to-truncation on every
  clean iteration of the p4 proof). The model critic is admitted under the
  count lease only; with forward tokens already at the ceiling the critic
  still dispatches - the recovery reserve can be consumed by critic tokens
  with no refusal, and the run durable budget_spent never sees the spend.
  The builder-dispatch leg of AK-052 is genuinely binding (controller.rs:1455
  reserves before invoke; 2034-2071 a refused charge is a terminal
  budget_exhausted, never a crash); this gap is the critic leg only.
- Executed evidence: reading-based plus indirect. The p4 proof itself sets
  model_dispatches: 0 (p4_corrective_loop.rs:1036) and its header documents
  the clean iteration fan-out as a "budget-truncation gap" - the composed
  path reaches the truncation arm of exactly this seam on every clean
  iteration and passes honestly; one configuration value (> 0) moves it from
  truncation to unmetered dispatch. No test in the tree exercises a metered
  critic dispatch (grep over crates/ finds no BudgetMeter use in compare/
  or the model adapter).
- Mitigations (why MAJOR, not CRITICAL): the shipped compositions set the
  allowance to zero so nothing unmetered dispatches today; the count-budget
  truncation is honestly recorded as unavailable_evidence (I8); no CLI
  composition root exists yet (crates/gauntlet-cli/src/ is main.rs plus
  spec_vocab.rs; Task 20 pending). The capability claim, however, is "every
  dispatch", and the spec binds critic fan-out to the meter; through the
  shipped controller this leg is structurally un-meterable.

### 2. MINOR - BudgetMeter::mint is a public trait method, so the documented "cannot forge" capability claim is not a type-level guarantee

- Claim broken: the port authority claim at
  crates/gauntlet-ports/src/budget_meter.rs:74-112 ("Constructible only
  through BudgetMeter::mint ... arbitrary code cannot forge the
  parallelism-accounting token", correction C9). mint has a public default
  body; Reservation fields are private, but any holder of a dyn BudgetMeter
  can call meter.mint(any_slice) and then meter.charge with the forged
  reservation.
- Concrete failure scenario: a downstream caller forges a reservation and
  charges it: MeteredBudget::charge
  (crates/adapters/gauntlet-adapter-meter-memory/src/lib.rs:194-225)
  saturating-decrements outstanding for a slot never reserved (lib.rs:222),
  so later reserve calls see capacity that does not exist and real
  parallelism can exceed max_parallel. The recovery reserve is NOT
  bypassable by this route - the charge still passes the domain
  Budget::charge gate - so this is an accounting-integrity hole, not a
  reserve hole.
- Executed evidence: reading (the trait method and its default body are
  public API); grep confirms no production caller invokes mint outside
  MeteredBudget::reserve and test stubs, so no shipped path exploits it.

### 3. MINOR - 21.3 degradation: non-integrity failures mid-run end the run with a bare Err, no terminal state, and no close-out checkpoint

- Claim broken: spec 21.3 ("Config/reference invalid: stop new dispatch;
  keep the process alive; keep reconciliation running"; "Agent session
  failure: retry or escalate per 11.5"), 11.5 row 2 (transient: backoff
  retry), 11.6 (every run ends in a terminal state with a report).
- Where / scenario:
  - An operator saves the answer key into an unparseable state (or the file
    becomes unreadable) while a run is in flight: reread_answer_key
    (controller.rs:1392-1438, called at 1145 and 1733) goes through
    strict_units (2810-2819), which maps parse/read failure to
    ControlError::Reference(ResolveFailed | Unavailable); that propagates
    out of drive - the run ends in Err with no terminal, no close-out
    checkpoint, no report. Note the asymmetry: the reference PROVIDER
    Unavailable at the boundary IS non-fatal (1079-1101); the answer-key
    source failure is fatal.
  - A runner transport error during a poll (controller.rs:1698) returns
    Err(ControlError::Agent(..)) rather than an 11.5 row-2 disposition; the
    in-crate test a_crash_after_a_settlement_never_re_runs_settled_work
    (controller.rs:4726-4734) pins this as intended crash semantics with
    resume-based recovery.
- Executed evidence: reading-based (constructing a mid-run corruption probe
  would require authoring a new test binary, outside this round write
  bound). Mitigations verified by passing in-crate tests: the
  top-of-iteration checkpoint (controller.rs:1020) makes state durable
  before the failure window, and resume_run (817-915) recovers from it, so
  no false settlement and no lost state results.

## What held up (what I tried that did NOT break)

- AK-051 (P6) composed: the p6 proof passes against the REAL controller,
  REAL sqlite state store, REAL fs evidence store, REAL command observer,
  REAL diff comparator, REAL metered adapter, and the REAL mock runner
  replaying the REQUIRED lying-builder fixture: terminal budget_exhausted,
  exactly 5 forward rounds of 6 booked (reserve round unspent), all four
  forward dimensions under the ceiling in the durable checkpoint, rows ==
  reservations (the meter own counter), one checkpoint per boundary plus the
  close-out. The reserve arithmetic (Budget::forward_ceiling, charge,
  would_breach_reserve, restore) is exact at every boundary and pinned by
  domain, adapter, and contract-suite tests (all green).
- AK-052 builder leg binding: every builder dispatch on the composed path
  reserves before invoke (controller.rs:1455); the summed-pending
  re-consult precedes each dispatch (1270-1280); a refused charge is a
  terminal with a pinned forensic record - never a crash and never a silent
  drop (2034-2071, record_uncharged_result); in-crate tests for both charge
  loci pass.
- AK-037 composed and non-vacuous: both p4 iterations record 15.1 selections
  durably with rejected alternatives carrying typed reasons; the domain
  aggregate refuses a record without rejected alternatives
  (correction.rs:455-462), so the vacuity class is unrepresentable; the
  numbers in the loop-path records come from the live meter and real loop
  state (selection_facts, controller.rs:3149-3161); the fuller
  three-candidate space (authority-blocked, chosen, ranked-out) is proven
  through the same selector, meter, and store at the proof level.
- AK-038 routed, not predetermined: the corrective re-dispatch is a full
  second plan / meter-reserve / invoke cycle driven by the disposition table
  from the REAL comparator residual (prompt carries residual text only -
  asserted; no critic vocabulary); exposure is by REAL observation over the
  same workspace the builder wrote; closure is a governed-chain verification
  PASS that settles the unit in the final checkpoint.
- AK-039 bounded and non-vacuous: recurrence counts under the semantic 13.5
  key (not fresh ids; different observed locators never merge); the composed
  p4 diagnosis test drives three REAL comparator recurrences into ONE
  bounded five-phase cycle ending INCONCLUSIVE with quarantined pinned
  evidence and an escalation; diagnostic-budget exhaustion leading to
  INCONCLUSIVE is pinned at the engine (where the mechanism lives);
  apparatus hypotheses outrank implementation defect by construction;
  per-phase spend is the allowance divided by the phase count, so one phase
  cannot consume the whole allowance.
- 18.2 checkpoints: written at every iteration boundary with all required
  fields; the offset is a real store-returned water-mark threaded from
  appends (pinned by the in-crate real-offset test); budget spent restores
  only through Budget::restore and its unreachability gate.
- 11.4 step 3: the in-memory snapshot is never authoritative - in-flight
  edits cancel (composed p4 test with teeth: the edit lands inside invoke);
  the queued-unit status leg is honestly recorded as having NO shipped input
  surface, pinned by the_answer_key_cannot_express_a_unit_status_on_any_
  shipped_surface (the parser is closed and typed-refusing; every
  well-formed block parses Open).
- 21.3 abort discipline (the half checkable by outcome): aborted terminals
  arise only from reference mutation and contract mismatch (integrity
  violations); governance-only blockage ends blocked with a ticket, never
  budget_exhausted (C1, test passes); budget breach ends budget_exhausted
  without escalating (the reserve covers the close-out). Finding 3 is the
  residue: bare Err paths that bypass terminal classification entirely.
- 23.1 proof honesty: both proofs run the real controller at the
  composition root with real adapters for state, evidence, reference,
  observation, comparison, and metering; deviations (scripted builder,
  facet-exit rung, recording escalation) are documented in the proof headers
  with reasons; no proof asserts both sides of a decision it also supplies -
  the p6 rows-vs-reservations count reads the meter own counter, not a
  proof-side tally.

## Declared search bound

- Read: the confirmation brief; Core-spec (esp. 5.3.2, 5.3.7, 6.2, 10.3,
  10.4, 11.1-11.6, 12.3, 13.5, 14, 15, 16.1, 17.1, 17.4, 18.2-18.4, 21.2,
  21.3, 23.1); answer-key AK-037/038/039/051/052 verbatim; plan Task 12
  section; AGENTS.md; the artifact: controller.rs, diagnosis.rs,
  disposition.rs, scheduler.rs, select.rs (production logic plus test
  inventories), budget.rs, budget_meter.rs, the meter-memory adapter,
  recording.rs, review.rs (header plus dispatch/lease logic), both proofs in
  full, correction.rs invariants, and spot reads of the sqlite state store
  (offset/checkpoint handling) and the model comparator budget fields.
- Executed: the three nextest runs listed above, one at a time,
  package-scoped, default parallelism. No custom probe binaries could be
  constructed (read-only except this file), so Finding 3 and the mint surface
  of Finding 2 rest on code reading, not execution.
- Threat model: in-process composition correctness against the spec - not
  adversarial builders/critics (anti-gaming is Task 24), not
  crash-injection recovery (Task 13 / P7), not concurrent or multi-host
  stress (single-threaded loop, in-process ports).
- Not examined: git history (prohibited); any report, status,
  preregistration, lesson, or skill file (prohibited); the prime-agent and
  cli-agent adapters (Tasks 15/16); sandbox/policy surfaces (Task 14);
  settlement commit sequencing beyond the loop-side precondition assert
  (Task 13); the full sqlite migration/encode surface; observe-* adapter
  internals beyond their composed use; the CLI binary (not yet composed).

## Verdict

**NOT CONFIRM.**

Within the declared bound, AK-037, AK-038, AK-039, and AK-051 hold through
the composed production path, and AK-052 holds on the builder-dispatch leg -
the meter is binding there, and the proofs are honest compositions, not
fixtures grading themselves. But the AK-052 "every dispatch" conjunct fails
on the composed critic-fan-out leg (Finding 1, MAJOR): the review path that
the controller itself drives every iteration performs model-comparator
dispatches under a count budget with no meter consult and books their
payments outside the run ledger, and that path documentation names Task 12 -
the task being confirmed - as the deferred-to owner of the wiring it did not
perform. The other two findings are MINOR. A MAJOR above the MINOR bar means
the confirmation cannot be granted this round.
