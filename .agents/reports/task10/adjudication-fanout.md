# Task 10 adjudication — parallel fanout and high-assurance adversarial pass

**Disposition: A — DIRECTLY IMPLEMENTABLE IN TASK 10.**

This is an implementation/composition defect, not a reference ambiguity or a
later-task-only requirement.  The frozen specification fixes both the required
behaviour and its boundary: `fanout` is one comparator *per applicable
dimension, in parallel*, and `high_assurance` is that fanout plus an adversarial
comparator (§13.3, `Core-spec.md:926-935`).  The Task 10 plan independently
requires all four review modes (`gauntlet-impl.md:378-383`).  It therefore needs
no ruler change, candidate revision, or product decision.

## Finding

The current implementation does **not** perform the required parallel fanout.

* `review` sorts dimensions and enters the `Fanout | HighAssurance` arm
  (`crates/gauntlet-app/src/compare/review.rs:216-224,241-242`), but calls
  `compare_to_doc` inside that loop (`:247-254`).  That call must return before
  the next dimension is selected.  The high-assurance adversarial call is then
  made only after the whole dimension loop (`:256-270`).  Thus it is serial
  dimension review followed by serial adversarial review, not parallel fanout.
* The apparent lower-level admission API cannot make that serial loop concurrent:
  `Comparator::compare` is explicitly blocking (`gauntlet-ports/src/comparator.rs:674-682`).
  `ModelComparator::comparison_document` admits one child and immediately
  drives it to a terminal status (`gauntlet-adapter-compare-model/src/lib.rs:186-197`),
  then collects and validates it (`:235-274`).  Only after that return can
  review construct/admit the next comparator.
* The adversarial *instruction* and source-required guard are correctly present:
  `ModelComparator::adversarial` selects the strong-case prompt
  (`gauntlet-adapter-compare-model/src/lib.rs:69-74,105-127`), and a missing
  high-assurance source is refused (`review.rs:206-214`).  They do not repair
  the serial scheduling.  Moreover, `ComparatorSource` is only an application
  trait (`review.rs:64-86`); its shown concrete implementations are test
  fixtures (`:562-596`).  There is no production source/composition in the
  current tree which can make the high-assurance path reachable.
* Existing review tests establish ordered calls and aggregation/truncation, not
  concurrent admission.  For example the high-assurance test expects the two
  dimension log entries and only asserts that its adversarial pass was budget
  truncated (`review.rs:835-868`); the model test fixture records calls with
  `Rc<RefCell<_>>` (`:532-579`), which cannot exercise cross-thread dispatch.

The spec expressly describes the required shape as “admission × N” followed by
fan-in polling (`Core-spec.md:757-771`), requires the `AgentRunner` admission
call to return promptly (`Core-spec.md:738-755`; `agent_runner.rs:430-446`),
and says critic fanout is bounded by `review.max_critics` and the budget meter
(`Core-spec.md:774-778`).  The current count-only budget is documented by the
code itself as a Task-12 deferral (`review.rs:24-37`).  It is insufficient to
make a concurrent dispatch safe: `BudgetMeter` has no lease/permit operation
(`budget_meter.rs:104-122`), and its `remaining_of` reports the configured
parallel ceiling rather than active-use headroom (`:124-144`).  Spec §17.4
nonetheless requires the meter before **every** dispatch (`Core-spec.md:1080-1084`).

## Smallest correct repair

Keep the existing blocking `Comparator::compare` compatibility API for
single/deterministic comparison.  Add an application-level two-phase model
comparison path instead of attempting to infer parallelism from threads around
that API:

1. Split `ModelComparator::comparison_document` at its existing admission
   boundary into `admit(envelope, deadline) -> PendingComparison` (which
   creates the exact `RoleInvocationSpec` and calls `AgentRunner::invoke`) and
   `collect(pending, deadline) -> ComparisonRecordDoc` (the existing
   poll/timeout/cancel/result-binding logic).  A pending value owns the handle,
   spec, and reserved dispatch lease.
2. Extend the *application* `ComparatorSource`/`ReviewDeps` model and
   adversarial sources with that two-phase capability.  In canonical dimension
   order, review reserves/admits every affordable dimension and, for
   `high_assurance`, the adversarial job too **before any poll or collect**.
   Fan in the pending jobs under the deadline, then pass their documents to the
   existing deterministic `aggregate`.  Preserve the present
   `unavailable_evidence` behaviour for jobs that cannot be admitted.
3. Make the budget composition authoritative at that same admission boundary:
   add an atomic dispatch lease/permit to `BudgetMeter` (or compose review with
   the controller’s equivalent) that checks reserve, depth, and active global
   parallelism and releases on terminal collection/cancellation.  Schedule
   batches no larger than that permit and `review.max_critics`; never replace
   this with a read of `remaining()`.  This is a narrow port/composition
   addition necessitated by the frozen §10.4/§17.4 rules, not a deferral of the
   Task-10 feature.

This preserves freshness and envelope binding: every admitted job still gets a
new invocation/workspace and the same verified envelope digest, as the model
adapter already constructs (`gauntlet-adapter-compare-model/src/lib.rs:138-185`).
It also preserves deterministic output: admission ordering and eventual
aggregation remain canonical; completion order is not used as a merge input
(§13.5, `Core-spec.md:941-947`).

## Deterministic discriminating proof

Add a review/model-composition test with two contract dimensions, sufficient
budget/permits for three jobs, and a scripted `AgentRunner` configured with
`expected_admissions = 3`.  Its `invoke` records every `RoleInvocationSpec` and
returns immediately; its `poll` deliberately returns a typed failure until all
three invocations have been admitted.  It then returns three valid, bound
`comparison_record@v1` results.

Run `high_assurance`.  Assert that it succeeds only after exactly three
admissions occur before the first poll/collect: two distinct dimension-scoped
standard prompts plus one `ADVERSARIAL_PROMPT`; all have distinct invocation
IDs/workspace/result paths and the identical verified envelope digest.  The
current implementation deterministically fails on its first poll, because it
has admitted only its first dimension.  No wall-clock timing or scheduler race
is involved.  A companion cap test with two leases for three jobs asserts no
more than two live handles and that the third is admitted after a lease is
released, proving the required bound rather than merely spawning threads.

## Conclusion

The high-assurance prompt/source check is a partial mechanism, but the required
parallel fanout (including the adversarial pass) is absent and has no production
source composition.  The authority for the repair is already frozen in the
specification and Task 10 plan.  **A** is therefore the only supportable
user-defined disposition.
