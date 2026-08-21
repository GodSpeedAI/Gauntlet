# Task 10 adjudication: durable critic `RoleInvocation` binding

## Decision: **A — required work remains**

The implementation proves a *transient* digest chain, not the requested durable
`RoleInvocation` binding.  It therefore does not meet the criterion that the
actual critic invocation and the accepted comparison remain auditable after
process loss.

## Normative basis

* Core spec §13.2 requires the serialized envelope digest to be stored on
  **both** the `RoleInvocation` and the `ComparisonRecord`; it defines the P2
  proof as an assertion over those serialized bytes.
* Core spec §5.3.5 makes `context_envelope_digest` a `RoleInvocation` field,
  and §18.1 lists `role invocations` and `comparisons` among the minimum
  durable records.  §18.2 further requires in-flight invocation ids and their
  result-contract paths in each durable checkpoint; §18.3 requires a restart
  to re-probe them rather than rely on memory.
* Plan Task 10 step 2 says: “Serialize canonically, digest, and record the
  digest on both the invocation and the comparison record.”  Its stated goal
  is runtime construction, not a test-only association.

## What is present

* `crates/gauntlet-domain/src/role_invocation.rs:108-210` has the required
  `RoleInvocationDefinition.context_envelope_digest`, retains it in the
  aggregate, and exposes it.  `crates/gauntlet-domain/src/comparison.rs:100-156`
  similarly requires and exposes `ComparisonRecord.envelope_digest`.
* The model adapter correctly puts the canonical envelope and its digest in a
  `RoleInvocationSpec` before `AgentRunner::invoke`
  (`crates/adapters/gauntlet-adapter-compare-model/src/lib.rs:156-190`), and
  rejects a result whose outer role-result or nested comparison document does
  not match that digest (lines 246-273 and 335-384).
* The app has only a local construction assertion:
  `crates/gauntlet-app/src/compare/envelope.rs:475-512` creates a
  `RoleInvocation` in a unit test and compares its digest.  It neither
  dispatches it nor writes it durably.  The P2 integration test likewise
  dispatches only a `RoleInvocationSpec` (`crates/gauntlet-cli/tests/p2_context_isolation.rs:147-172`).

## Binding gap

There is no production path that constructs the domain `RoleInvocation` for
this model critic, attaches it to an iteration, or persists it.  The only
production `RoleInvocation::admit` use is the app test above; the model adapter
uses the port transport value `RoleInvocationSpec`, not the domain aggregate.

The current persistence interfaces cannot retain the binding:

* `StateStore` has only `append_event`, run/checkpoint/resume/export
  (`crates/gauntlet-ports/src/state_store.rs:61-126`).  Its
  `InvocationAdmitted` event contains only `invocation_id` (lines 68-74), and
  a checkpoint's in-flight tuple contains only id/path (lines 88-96), neither
  an envelope digest nor a serialised invocation.
* `EventLog` simply appends that `RunEvent` vocabulary
  (`crates/gauntlet-ports/src/event_log.rs:55-78`).
* `EvidenceStore` can immutably put and pin bytes
  (`crates/gauntlet-ports/src/evidence_store.rs:44-76`), but the model
  comparator imports/uses neither it nor a state recorder.
* SQLite names `RecordKind::RoleInvocation` and `Comparison`
  (`crates/adapters/gauntlet-adapter-state-sqlite/src/state_store.rs:73-110`),
  but its `append_record` is adapter-specific raw-JSON API (lines 384-410),
  not a `StateStore` port method, and no comparison-dispatch path calls it.

Thus a successful in-memory comparison can disappear across a restart, and a
replayed invocation id cannot be bound to the record's envelope digest.  The
existing adapter validation is valuable admission-time validation, but it is
not durable evidence.

## Correct owner and smallest coherent change

The owner is an **application-layer comparison-dispatch/iteration service**:
it owns lifecycle and durability and depends only on domain + ports, as Core
spec §4.1 requires.  The CLI composition root should only inject concrete
runner/store/evidence adapters; neither the model comparator nor SQLite should
be made the authority for run lifecycle.

Smallest coherent change:

1. Add a typed `StateStore` operation for a durably admitted
   `RoleInvocation` (including its digest and result-contract path), and a
   typed durable comparison-record operation/reference.  Have the SQLite
   adapter implement canonical serialization; do not expose its raw
   `append_record` to application code.
2. Add an application `RecordingAgentRunner`/comparison-dispatch wrapper.
   It receives the runtime-built `RoleInvocationSpec`, invokes the selected
   runner, uses the returned handle/session ref to construct the domain
   `RoleInvocation`, records it **before returning control to polling**, and
   writes the admitted event/checkpoint.  It must also persist the accepted
   `comparison_record@v1` (and pin its immutable bytes through
   `EvidenceStore`) only after the model adapter's binding checks succeed.
3. Make the wrapper the runner injected into `ModelComparator`.  This retains
   the adapter's fresh-context/result-schema validation while ensuring the
   exact id and digest seen by the runner are the values persisted by the
   application.  A durable-write failure must be explicit (and cancel/recover
   the already-admitted child); it must not continue as an unrecorded pass.

This is smaller and safer than moving model protocol parsing into the CLI or
having the model adapter call SQLite directly; those alternatives either break
§4.1 substitution or leave lifecycle authority vendor-specific.

## Fail-first proof

Add a CLI composition integration test (for example
`crates/gauntlet-cli/tests/p2_invocation_binding.rs`) before the change.  Use
the real app envelope builder, model comparator over the mock `AgentRunner`,
SQLite `StateStore`/`EventLog`, and filesystem `EvidenceStore`.

The test must dispatch one critic, reopen the stores (not reuse memory), and
assert all of the following are equal: canonical-envelope SHA-256, durable
`RoleInvocation.context_envelope_digest`, role-result digest, and durable
`comparison_record@v1.envelope_digest`; also assert the durable invocation id
is the result's id and its result-contract path survives checkpoint/resume.
A wrong-digest fixture must yield `contract_mismatch` and no accepted durable
comparison.  It fails on HEAD because no production composition path emits a
durable invocation or comparison binding, and it proves the §18 restart case
rather than merely the current in-process equality test.
