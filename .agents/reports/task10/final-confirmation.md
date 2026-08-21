# Task 10 final fresh confirmation

## Verdict: NOT CONFIRM

Fresh read-only review against Core spec §§5.3.5, 10.3, 13.2–13.5; Answer Key AK-026, AK-027, AK-032–AK-034; and Task 10 in the implementation plan. No prior Task-10 report, status file, history, or compilation was consulted.

## Blocking findings

1. **BLOCKING — the envelope does not establish that `reference_view` is frozen reference or its valid projection.** `EnvelopeInputs` accepts unrelated caller-supplied `reference_digest: Digest` and `reference_view: String` (`crates/gauntlet-app/src/compare/envelope.rs:107–115`). It recomputes only the observation digest (`:164`) and then includes the arbitrary string in the envelope. Thus a caller can label builder trajectory/reasoning as `reference_view` while supplying a genuine frozen digest; `assert_blind` checks JSON field names (`:237`), not provenance. The source itself calls this a “separate typed-port gap” (`:561`). This violates §13.2’s allowlist (“reference (frozen, or relevant projection)”) and prevents confirmation of AK-026/P2. Replace the detached string/digest inputs with a frozen-reference/projection value whose derivation and digest binding are verified at construction; add the adversarial provenance test.

2. **MAJOR — fan-out is serial, not parallel.** In `review.rs:241–254`, each model comparator is acquired and fully `compare_to_doc`-completed inside one `for` loop before the next begins. Adaptive has the same serial model dispatch (`:276–317`). Core §13.3 explicitly requires one comparator per fan-out dimension “in parallel.” This is an observable behavior/performance contract, not a test-only detail.

3. **MAJOR — actual critic dispatch has no durable `RoleInvocation` binding.** The model adapter creates only a transient `RoleInvocationSpec` and calls `runner.invoke` (`gauntlet-adapter-compare-model/src/lib.rs:156–186`). It neither admits/attaches a domain `RoleInvocation` nor writes it through a state/event port. The isolated unit test at `envelope.rs:476` constructs a separate domain invocation but is not the dispatch path. Therefore the §5.3.5 requirement that the digest be recorded on the invocation, and the plan’s Task-10 Step 2, are not met durably. The returned comparison document is well bound to the envelope (`compare-model:246–273`), but that does not repair missing invocation persistence.

## Other probe results

- **Pass:** consumption-boundary observation-digest mutation is rejected (`envelope.rs:164`); review and both comparator adapters also recheck envelope digest before dispatch/compare.
- **Pass:** adaptive treats an inconclusive deterministic probe as undecided and dispatches a model critic; its explicit test is `review.rs:718`.
- **Pass, bounded:** aggregation fails closed when a gate reported by one contributor is omitted by another (`aggregate.rs:364–417`, test `:849`). It cannot identify a hard gate omitted by *all* contributors because neither `aggregate` nor its input carries the required hard-gate set. Enforcing that set is P5/Task 14 territory; do not silently treat this later-scope boundary as satisfied by Task 10.
- **Concern:** high-assurance may exhaust the same dispatch budget on ordinary dimensions and merely record `adversarial-pass` unavailable (`review.rs:256–268`) rather than run its required adversarial comparator. It remains inconclusive rather than falsely passing, but cannot honestly provide high assurance.

## Scope boundary

No frozen-reference candidate or later-task dependency blocks repairing findings 1–3: they are direct Task-10 requirements. Controller-level full production composition is later (Task 20), but it does not excuse Task 10’s specified envelope provenance, parallel fan-out, or durable invocation-record mechanism.
