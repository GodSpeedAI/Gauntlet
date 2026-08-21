# Task 10 final blind confirmation — round 2

## Verdict: NOT CONFIRM

### Material Task-10 residual: frozen-reference provenance is bypassable at the comparator boundary

`FrozenReferenceProjection` in `crates/gauntlet-app/src/compare/envelope.rs` is a useful opaque, evidence-checked input to `build_envelope`: its only production constructor loads `ReferenceVersion.evidence_ref`, normalizes the bytes, and compares their digest to the version digest.  That closes the arbitrary-view pairing *at that builder API*.

It does not close the declared requirement across the actual comparator-dispatch boundary.  `gauntlet_ports::comparator::ComparisonEnvelope` has public `reference_digest` and public `reference_view` fields, and `calculate_envelope_digest` is public.  A caller can therefore construct an envelope containing a genuine frozen reference digest and arbitrary reference text, recompute its envelope digest, and submit it directly to either comparator.

The model adapter (`gauntlet-adapter-compare-model/src/lib.rs`, `ModelComparator::admit_comparison`) verifies only that the envelope digest covers those caller-supplied bytes and that the contract/isolation are acceptable.  It does not receive an evidence store or `ReferenceVersion`, so it cannot verify that `reference_view` was rendered from the frozen evidence.  The deterministic adapter has the same digest-only consumption check.  The review entry point also accepts this public envelope and performs only `verify_envelope_digest`.

Thus the exact attack named in this confirmation request—genuine digest paired with arbitrary view—can still reach a critic and produce a persistently bound comparison record.  Existing tests prove stale-digest refusal and the builder-only opaque path, but do not exercise or refuse this recomputed-digest provenance-smuggling path.  This is a Task-10 boundary/invariant issue, not Task-20 CLI/full-loop scope.

## Confirmed portions (static inspection; no commands run)

- The frozen builder path allowlists its inputs, validates frozen evidence bytes/digest, rechecks observation digest, canonicalizes and digests the envelope, and runs the serialized-field blindness assertion.  The P2 test includes the deny-list tooth.
- `review` has a two-phase pending-comparison path. With sufficient configured concurrency it admits all fanout/high-assurance jobs before first collection; leases are acquired before admission, retained through collection/cancellation, and active capacity is bounded. Collection failure attempts sibling cancellation. High assurance requires a separately tagged `ComparatorSourceKind::Adversarial`; `ModelComparator::adversarial()` supplies the stronger prompt.
- The application `RecordingAgentRunner` records an admitted domain `RoleInvocation` after runner admission and before returning its handle; durable-record failure attempts cancellation. Its accepted-comparison recorder pins evidence and records only a document bound to the admitted run/iteration/envelope digest. The P2 composition test reopens SQLite and checks durable invocation/comparison exports and a cross-wire rejection.
- Aggregate behavior, adaptive probe skipping, and budget-truncation gaps are implemented in Task-10 code. Lack of a Task-20 CLI composition root/full end-to-end loop is not counted as a residual here.

## Required correction before confirmation

Make the dispatchable comparison context provenance-carrying/opaque through the comparator boundary (or make comparator dispatch accept only a builder-produced opaque type), then have every consuming adapter verify the frozen-evidence/reference-version binding rather than only the envelope self-digest. Add a tooth that supplies an arbitrary `reference_view` with a genuine frozen digest and a recomputed envelope digest; dispatch must refuse it before model admission and before comparison persistence.
