# Task 10 static audit — D4/D5

**Scope and method.** Read-only reconciliation of the D4/D5 checkpoint claims with the
Task 10 brief/plan, Core spec, answer key, and source/tests. No files besides this report
were changed; no build or test command was run. “Pass/green” statements in the checkpoint
are therefore not independently verified.

## Findings requiring correction or an explicit deferral

### F1 — Critical: `high_assurance` can silently omit the required adversarial pass

The spec requires high assurance to fan out over every dimension **plus** an adversarial
comparator (Core spec §13.3, lines 926–935); the D4 brief says the same (builder brief
lines 166–168), and the checkpoint claims it landed (builder-checkpoints.md lines 197–198).
But `ReviewDeps.adversarial` is optional (`crates/gauntlet-app/src/compare/review.rs:80–84`).
The dispatch only acts when it is `Some` (`:226–240`); `None` yields a normal successful
outcome with neither an error nor unavailable-evidence entry. The broad mode test
intentionally supplies `None` for *every* mode, including `HighAssurance`, and expects
success (`review.rs:699–734`). This is an implementation/test contradiction of the claimed
mode semantics, not merely missing coverage.

### F2 — Critical: model comparator neither polls/collects to the supplied deadline nor cancels on timeout

The D5 brief requires `poll`/**`collect` within the `Deadline`** (`builder-brief.md:190–197`).
Spec §10.3 requires no result by deadline to be `TimedOut` and the runtime to cancel the
child (`Core-spec.md:746–755`). `ModelComparator::comparison_document` passes a deadline
into the invocation spec but then calls `drive_to_terminal(&handle)` without it
(`compare-model/src/lib.rs:162–185`). The latter only makes 24 immediate polls
(`:255–273`), never calls `collect`, never consults the deadline, and no `cancel` call
exists in the adapter. Thus the checkpoint statement that “collect carries the real
deadline” (builder-checkpoints.md:259–261) is false for this code. A probe-count convention
may be a temporary boundedness mechanism, but it is not the specified deadline/cancellation
behavior and is not recorded as a D5 deferral.

### F3 — Critical: untrusted critic payload is parsed but not bound to the dispatched envelope/run

The outer `role_result` is checked against invocation ID and context-envelope digest
(`compare-model/src/lib.rs:224–238`), but the nested `ComparisonRecordDoc` is immediately
accepted (`:239–252`). There is no comparison of its `run_id`, `iteration_no`,
`reference_digest`, `observation_digest`, `envelope_digest`, or contract version to the
invocation/envelope. `to_domain_record` then copies the payload’s three digests verbatim
(`:293–308`). This permits a well-formed, cross-wired/stale comparison payload despite the
checkpoint’s “all three digests coupled” claim (builder-checkpoints.md:248–250) and its
claim of result validation (lines 261–268). The derailment test covers foreign *outer*
invocation and wrong payload type only (`:740–787`), not a mismatched nested comparison
record. Add a negative test for every coupling field and refuse it as `ContractMismatch`.

### F4 — High: D4 reboxing discards B.4 residual/gate provenance before aggregation

Review calls the `Comparator` trait’s domain-returning method and reconstructs a new wire
document in `to_doc` (`review.rs:204, 223, 254, 274, 297–339`). That reconstruction sets
`unit_refs`, `expected`, `evidence_refs`, diagnostics/tests/corrections, and hard gates to
empty values (`:301–338`), replaces comparator identity with `review-dispatch` (`:328–332`),
and substitutes the domain detail for `observed` (`:308–315`). Section 13.5 requires merge
keys `(dimension, expected, observed_locator)` and unioned evidence refs (Core spec
§13.5, lines 941–947). Consequently distinct adapter/critic B.4 records can be collapsed
on the wrong key and their evidence/provenance lost. The adapters’ public
`comparison_document` methods do produce full records (diff `lib.rs:107–130`; model
`lib.rs:140–253), but review does not use them. The D4 tests only use deliberately
minimal domain records (`review.rs:409–477`), so they do not establish preservation.

### F5 — High: fanout is serial, and adaptive does not run all probes before model dispatch

Spec §13.3 says fanout is “in parallel” and adaptive is: run probes, determine undecided
dimensions, then fan out (Core spec lines 926–933). Review invokes each comparator directly
inside ordinary loops: fanout/high-assurance at `review.rs:211–225`, and adaptive invokes a
model immediately after each clean probe at `:242–277`, before probing later dimensions.
There is no concurrency primitive or two-phase undecided collection. This contradicts the
brief’s “deterministic probes first” wording (`builder-brief.md:162–165`) and leaves the
checkpoint’s mode claim (lines 192–196) overstated. The named adaptive test observes only
which model dimensions were logged (`review.rs:516–589`), not ordering or parallelism.

### F6 — Medium: truncation is only an in-memory side channel, not part of the comparison record

`ReviewOutcome` stores gaps only in `Vec<UnavailableEvidence>` (`review.rs:116–137`) and
returns them separately (`:281–289`). `comparison_record@v1` has no such field
(`comparison_record.rs:576–587, 784–835`), and D4 neither wraps the outcome in a role
result nor persists it. Thus the AK-034 requirement to *record* missing dimensions
(answer key lines 235–240) is demonstrated only for an immediate caller; durable/e2e
recording is unproven. The D4 test only inspects the returned vector (`review.rs:591–692`).
This may be a later composition responsibility, but it needs an explicit interface and
ledgered deferral rather than the checkpoint’s unconditional “recorded” claim.

### F7 — Medium: model fanout’s per-dimension boundary is prompt-only and unvalidated

The model adapter adds the requested dimension solely to prose (`compare-model/lib.rs:121–137`).
After parsing, it does not require `comparator_identity.dimension` or each residual dimension
to match that requested dimension (acceptance path `:239–253`). A critic can return another
or uninstantiated dimension and be admitted. This weakens the D4 claim that one comparator
judges each applicable dimension and leaves no adversarial test for scope escape.

## Scope/review notes

* The confidence-number codec change is outside the D4/D5 adapter/review slices
  (`comparison_record.rs:380–395`; checkpoint lines 286–289). It appears causally related
to adapter round-tripping, so it is not necessarily inappropriate, but it is a cross-port
change with no cited regression test for integer-form confidence. Record it as a bounded
dependency fix rather than “in passing.”
* The count-only `ModelDispatchBudget` is explicitly allowed by the D4 brief’s Task-12
deferral (`builder-brief.md:169–175`) and is not scope creep. It must not be represented
as full `BudgetMeter` compliance.

## Positive static confirmations

The implementation does sort dimensions before budget selection (`review.rs:186–192`),
records budget-dropped dimensions in the returned outcome (`:206–208, 213–215, 264–266`),
uses a deterministic diff mapping for all eight current `FacetKind` variants
(`compare-diff/lib.rs:59–67`), and both adapter crates invoke the shared comparator suite
in their tests (diff `:545–565`; model `:795–820`). These do not resolve F1–F7.

## Recommended disposition

Do not treat D4/D5 as settled from the checkpoint. Correct F1–F4 before relying on review
modes or the model comparator for a production comparison path; either correct F5–F7 or
record their explicit scope/dependency deferrals with tests that prevent their silent
regression.
