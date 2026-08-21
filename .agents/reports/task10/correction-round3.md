# Task 10 correction round 3 — final bounded status

**Status: direct Task-10 A/B blockers implemented; not a claim that a production
CLI loop or Task 20 is complete.** No frozen ruler, `CURRENT_STATUS.md`, or
`.agents/reports/reference-candidates.md` was changed by this invocation.

## A. Durable critic invocation/comparison binding

- Added typed `StateStore::record_role_invocation` and
  `StateStore::record_comparison` operations. The embedded-state adapter records
  canonical immutable `role_invocation` and `comparison` rows; migration 0003
  adds the recovery lookup indexes.
- Added application-owned `RecordingAgentRunner`. It invokes the child, builds
  the admitted domain `RoleInvocation` from the exact `RoleInvocationSpec` plus
  returned session reference, durably records it and its admission event before
  returning the handle to any poller. A failed durable write cancels the admitted
  child and reports the recovery locus.
- `ModelComparator` now requires an `AcceptedComparisonRecorder`; after all
  outer and nested binding validation it writes/pins the accepted document via
  the app recorder before returning it. Thus the composition cannot report an
  unrecorded accepted comparison.
- Added a real composition proof in `p2_context_isolation`: application
  envelope builder + model adapter + recording runner + filesystem evidence +
  embedded state store. It reopens the store, checks invocation/document digest
  binding and checkpoint result path, and has a cross-wired nested-document tooth
  that yields `contract_mismatch` with no durable comparison. A recording-store
  failure tooth proves cancellation of the already admitted child.

## B. Fanout leases and adversarial source selection

- Replaced the count-only dispatch precheck with mutex-protected admission
  leases in `ModelDispatchBudget`; total allowance and active cap are independently
  enforced. A lease starts before admission and releases on collection,
  cancellation, or admission error.
- At cap, canonical fan-in collects one pending job before admitting the next;
  when permits suffice, all jobs remain admitted before first collection. A
  collection failure cancels all uncollected siblings.
- `ComparatorSourceKind` makes deterministic/model/adversarial selection typed;
  high assurance rejects a non-adversarial source.
- Deterministic teeth: the existing three-job barrier still proves all admission
  precedes first collection at capacity three; new no-wallclock proof proves the
  third job waits for a two-lease release; sibling-failure proof observes both
  cancellations.

## Focused gates

PASS:

- `cargo test -p gauntlet-app compare::recording` (1)
- `cargo test -p gauntlet-app compare::review` (10)
- `cargo test -p gauntlet-adapter-compare-model` (5)
- `cargo test -p gauntlet-adapter-state-sqlite`
- `cargo test -p gauntlet-cli --test p2_context_isolation` (4)
- `cargo check --workspace --all-targets`
- `cargo clippy -p gauntlet-app -p gauntlet-adapter-compare-model -p gauntlet-cli --all-targets -- -D warnings`
- `cargo fmt --check`
- `just boundaries` (194)
- `just ruler`
- `just deny` (warnings only; exit 0)

`cargo nextest run --workspace` is **not green** because the pre-existing r3
provenance residual fails `p1_reference_integrity::the_run_carries_the_frozen_version`:
SQLite re-open still reconstructs the legacy unbound `ReferenceVersion.evidence_ref`.
That is the explicitly out-of-scope provenance residual recorded before this bounded
A/B correction; it was not weakened, hidden, or changed here.


## C. Frozen `ReferenceVersion` evidence persistence correction (post-gate residual)

### Scope and migration

The red workspace gate was reproduced before this correction:
`cargo nextest run -p gauntlet-cli --test p1_reference_integrity the_run_carries_the_frozen_version`
exited **100**. SQLite had rehydrated the stored run using the legacy
`ReferenceVersion::new` marker (`unbound-frozen-reference-evidence`) rather
than the evidence ref returned by the real freeze.

Migration **0004_frozen_reference_evidence** adds nullable
`runs.reference_evidence_ref`. New authoritative run writes persist the exact
`ReferenceVersion.evidence_ref`; reopen parses it and reconstructs with
`ReferenceVersion::with_evidence`. It participates in idempotent identity
comparison and deterministic export. The application export's strict canonical
run shape was extended with that field, so the full export stack remains
reachable and canonical.

Existing rows retain `NULL`. `load_run` returns typed
`StateStoreError::Unavailable` explaining that the original frozen evidence
must be migrated/recreated and that no binding is synthesized. This is
intentional: a digest cannot prove which evidence object was pinned.

### Reopen and negative evidence

`p1_reference_integrity::the_run_carries_the_frozen_version` now records a
checkpoint, drops the state adapter, reopens it, checks complete
`ReferenceVersion` equality, then constructs a `FrozenReferenceProjection`
from the rehydrated version and reopened filesystem evidence. Its missing
object tooth requires `EnvelopeError::FrozenReferenceUnavailable`.

SQLite tests additionally prove (1) a legacy row with no evidence column value
is `StateStoreError::Unavailable`, and (2) an otherwise identical re-recording
with a different frozen evidence ref is a typed write refusal. These are the
wrong/missing evidence teeth; neither path fabricates a binding.

### Evidence

Focused green after the migration:

- `cargo nextest run -p gauntlet-adapter-state-sqlite` — **33/33 PASS**.
- `cargo nextest run -p gauntlet-cli --test p1_reference_integrity` — **4/4 PASS**.
- `cargo nextest run -p gauntlet-cli --test p2_context_isolation` — **4/4 PASS**.
- `cargo nextest run --workspace` — **677/677 PASS**.
- `just check && just lint && just deny && just boundaries && just ruler` —
  **PASS**; boundaries **194/194**; deny emitted only its pre-existing
  informational warnings.

The first full workspace attempt exposed `export_stack` rejecting the new,
strictly emitted `reference_evidence_ref` as an unknown canonical run field.
The canonical application export shape was updated, its composed export test
passed, and the final workspace run above was green.

This removes the specific red P1 persistence residual only. **Task 10 remains
IN-FLIGHT/BLOCKED and is not closed or settled by this correction.**


## Round 4 implementation checkpoint (in progress)

Started direct correction of the fresh-confirmation residuals: opaque trusted
dispatch provenance, mandatory durable acceptance, cleanup/quarantine of pending
critic work, and unforgeable high-assurance source selection. No frozen material,
`CURRENT_STATUS.md`, or reference-candidate file was modified by this slice.
This is a checkpoint only; no gate or closure claim is made.


## Round 4 direct-correction update — **not closed; material residuals remain**

### Implemented and focused evidence

- Added `ComparatorError::RecoveryRequired`. The model pending collector now
  cancels on bounded-probe/poll nonterminal errors. If cancellation cannot
  confirm termination it returns this typed recovery locus instead of
  converting it into an ordinary timeout/error.
- Reworked review admission leases: a failure in collection quarantines the
  primary lease; every admitted sibling is cancelled; cancellation failures
  quarantine their leases and surface `RecoveryRequired`. `ModelDispatchBudget`
  exposes active/quarantined work for recovery observability. The new
  `cancellation_error_quarantines_every_unconfirmed_sibling` tooth proves the
  primary plus a cancellation-refusing sibling stay counted rather than release
  capacity.
- Replaced public `ComparatorSourceKind` self-labeling with a sealed
  `AdversarialComparatorSource` slot and crate-owned `AdversarialSource`
  wrapper. A normal external `ComparatorSource` cannot type-check in
  `ReviewDeps.adversarial` merely by returning a public enum/tag.
- Added a `FrozenReferenceProjection` and `TrustedComparisonContext` in ports;
  construction validates normalized evidence bytes against the frozen
  `ReferenceVersion` and then validates the envelope's view/digest against that
  projection. `BuiltEnvelope` now retains this trusted context, so the
  provenance-bearing dispatch value exists and the application builder uses it.

Focused green:

- `cargo test -p gauntlet-app compare::review` — 11/11 PASS.
- `cargo test -p gauntlet-adapter-compare-model` — 5/5 PASS.
- `cargo test -p gauntlet-adapter-compare-diff` — 6/6 PASS.
- `cargo test --workspace --no-run` compiled successfully before the final
  focused test-only import cleanup; no full gate/closure claim is made here.

### Remaining direct blockers (do **not** treat this as confirmation)

1. The `Comparator` port still consumes public `ComparisonEnvelope`, rather
   than `TrustedComparisonContext`. Changing that signature correctly requires
   migrating every shared comparator contract fixture and adapter test through
   evidence-backed trusted construction. Therefore a direct recomputed-digest
   serializable-envelope dispatch remains possible at this checkpoint.
2. Accepted-comparison composition is still based on the public recorder trait;
   it has not yet been replaced by a mandatory application-owned durable
   acceptance boundary with complete run/invocation/reference tuple validation.
3. The sealed adversarial wrapper prevents external enum/tag forgery, but the
   production composition factory that admits only a concrete adversarial model
   source still needs wiring; no claim is made that Task 10 is fully closed.
4. Source/admission/aggregate error paths still need the same explicit cleanup
   guard used for collection paths, and the forever-running-primary tooth has
   not yet been added.

No frozen ruler, `CURRENT_STATUS.md`, or reference-candidate material was
modified by this correction slice.
