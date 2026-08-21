# Task 10 — final confirmation round 2 (adversarial)

**Verdict: FAIL — do not CONFIRM.** Static, read-only review found major surviving
residuals. No compilation was run.

## Scope and ruler

Read only the frozen `.agents/specs/Core-spec.md`, `.wayfinder/answer-key.md`,
Task 10 section of `.agents/plans/gauntlet-impl.md`, and current source/tests.
In particular: spec §§5.3.3–5.3.6, 10.3–10.4, 13.2–13.5; AK-026, AK-027,
AK-032, AK-033, and AK-034. I did not read status files, prior Task-10 reports,
git history, or prior reviews.

## Surviving major residuals

### M1 — timed-out/failing fan-out can leave critics live while releasing their permits

**Attack.** Supply a runner whose admitted critic remains `Running`. In
`PendingModelComparison::collect`, the probe ceiling calls
`drive_to_terminal_with` (`crates/adapters/gauntlet-adapter-compare-model/src/lib.rs:253`).
After 24 non-terminal polls, that helper returns `ComparatorError::TimedOut`
(lines 329–347). The `?` returns before `cancel_with` is reached, so the very
critic that timed out is neither cancelled nor durably marked as such. Its
`PendingModelComparison` and its `DispatchLease` are then dropped.

A second concrete path: on a sibling collection failure,
`collect_next` discards every cancellation result (`review.rs:512–533`, notably
`let _ = sibling.pending.cancel(...)` at 529). A runner can return a cancel
error while retaining the remote child. The pending objects then drop and all
leases release.

A third path reaches the same outcome before collection: after one or more
successful admissions, a later `model.for_dimension` or
`adversarial.across_dimensions` source failure is propagated with `?`
(`review.rs:354–360, 374–380, 418–424`) without cancelling already-pending
siblings.

**Impact.** A subsequent review can acquire capacity that the live remote
critics still consume, violating the bounded `max_parallel`/fan-out premise;
those critics can also later write result contracts after the review has failed.
This contradicts §10.3’s cancellation requirement and the source’s own claim
that a failure never leaves an admitted sibling uncollected.

**Required correction.** Own pending work in a cleanup guard that attempts and
records cancellation on *every* post-admission error path, including source,
admission, probe-ceiling timeout, collection, and aggregation failure. Do not
release an active permit until the runner reports terminal/cancelled (or retain
it as quarantined capacity and surface a typed recovery failure). Add teeth for
a forever-running primary and a cancellation-error sibling.

### M2 — high-assurance adversarial status is forgeable by a normal comparator

**Attack.** An external implementation of the public
`ComparatorSource` trait can return `ComparatorSourceKind::Adversarial` while
its `across_dimensions` returns an ordinary `ModelComparator::new(...)` (not
`.adversarial()`), or any comparator with a standard/pass-oriented prompt.
`review` accepts it solely on the enum value (`review.rs:286–321`) and then
calls it at `374–380`. The public enum/trait are not sealed and carry no
adversarial instruction or attestation.

**Impact.** `ReviewMode::HighAssurance` may return a normal fan-out result while
claiming its required adversarial pass ran. This violates §13.3’s requirement
for “an adversarial comparator instructed to find the strongest case” and makes
assurance a caller assertion rather than a typed construction property.

**Required correction.** Make source construction application-owned and
unforgeable (for example a sealed source trait or a distinct adversarial source
newtype constructed only from `ModelComparator::adversarial()`), and add a
negative test showing a standard-model source cannot occupy the adversarial
slot.

### M3 — frozen-reference/run provenance and durable acceptance remain bypassable

**Attack.** Construct `gauntlet_ports::comparator::ComparisonEnvelope` directly:
all members including `reference_digest` and `reference_view` are public
(`comparator.rs:541–570`). Put attacker-selected reference text and a matching
self-computed envelope digest in it. Both model and deterministic adapters only
check that this self-declared digest recomputes (`compare-model/lib.rs:145`;
compare-diff/lib.rs:107); neither can require a
`FrozenReferenceProjection`, its evidence ref, or equality with the durable
Run’s frozen `ReferenceVersion`.

For model success, instantiate the public `ModelComparator::new` with an
`AcceptedComparisonRecorder` implementation that returns `Ok(())`
(`compare-model/lib.rs:87–107`; public recorder trait at
`comparator.rs:695–702`). The adapter returns a successful comparison after
that no-op. Alternatively, even the supplied recorder checks only run id,
iteration, and envelope digest before persistence (`recording.rs:204–220`),
not the durable run identity/reference evidence. The SQLite
`record_comparison` path is an append of opaque payload (`state_store.rs:1222–1234`)
and does not reject a nonexistent run or a reference digest differing from that
run. Reopen/export proves bytes survived, not that they bind to the frozen run.

**Impact.** A fabricated reference can be compared and/or accepted as a success
without a frozen-reference evidence provenance chain. This directly defeats
§5.3.3’s one-frozen-reference-per-run and §5.3.6’s requirement that a record
whose reference digest differs from the run invalidates the iteration. It also
leaves the claimed “no unrecorded success” property dependent on optional
composition rather than enforced at the acceptance boundary.

**Required correction.** Make comparator consumption accept an opaque
`BuiltEnvelope`/trusted dispatch token rather than public forgeable envelope
fields; bind it to the stored Run and frozen evidence at admission and durable
recording. Make durable acceptance mandatory (not an arbitrary public callback)
and enforce the run/reference/observation/envelope/contract tuple transactionally
on record and reopen. Add cross-run, nonexistent-run, forged-reference, and
no-op-recorder teeth.

## Attacks that did not leave a separate residual

- The canonical envelope build rechecks the observation digest and uses
  `FrozenReferenceProjection::load`; its normal construction path excludes the
  claims facet and checks deny-listed JSON field names.
- Model result handling validates outer invocation/run/iteration/role/envelope
  binding and nested run/iteration/reference/observation/envelope/contract
  fields before calling its recorder.
- Aggregation is deterministic over supplied documents, retains inherited and
  newly detected disputes, unions evidence, and makes any supplied blocking
  residual fail. It treats a gate missing from a *contributor* as incomplete.
  I found no independent Task-10 residual there; the current contract exposes
  no authoritative expected-gate set from which “missing from every
  contributor” could be distinguished from “not applicable.”
- Recording failures returned by the supplied recorder do not themselves become
  a successful `ModelComparator` return; M3 is the remaining composition
  bypass, not a claim that that particular error branch returns `Ok`.
