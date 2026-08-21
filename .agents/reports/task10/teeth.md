# Task 10 teeth verification

Date: 2026-08-17

## Method and scope

Each mutation below was applied alone, tested once, then replaced with the
byte-identical pre-mutation content retained in memory before the next
mutation. The named focused test was immediately rerun after each revert.
No frozen reference, comparison criterion, acceptance gate, status document,
or prior evidence was changed. The working tree was already dirty before this
exercise; restoration below means the four touched source files exactly equal
their pre-mutation snapshots, not that unrelated concurrent work was reset.

## 1. Observation digest recomputation

- **Mutation:** In `crates/gauntlet-app/src/compare/envelope.rs`, replaced the
  complete `build_envelope` consumption-boundary block
  `let recomputed = observation_digest(inputs.bundle); if &recomputed !=
  inputs.bundle.observation_digest() { return Err(EnvelopeError::ObservationDigestMismatch { ... }); }`
  with a no-op comment. This bypassed the declared-versus-recomputed digest
  refusal.
- **Fail command/evidence:** `cargo nextest run --test p2_context_isolation the_envelope_is_structurally_blind_and_carries_instrument_reality` exited 100;
  the sole selected test panicked at `p2_context_isolation.rs:406:5`:
  `a tampered facet cannot flow into a comparison`.
- **Restoration:** Restored the exact block, including the recomputation,
  comparison, and typed `ObservationDigestMismatch` return.
- **Post-revert green:** the same command exited 0: 1 passed, 1 skipped.

## 2. Blindness assertion

- **Mutation:** In `crates/gauntlet-app/src/compare/envelope.rs`, replaced
  `assert_blind`'s deny-list loop and `BlindnessViolation` return with
  `let _ = bytes; Ok(())`, accepting poisoned bytes.
- **Fail command/evidence:** `cargo nextest run --test p2_context_isolation teeth`
  exited 100; `teeth` panicked at `p2_context_isolation.rs:498:10`:
  `the blindness proof must FAIL on the poisoned envelope: ()`.
- **Restoration:** Restored the exact loop checking every
  `DENY_LISTED_FIELD_NAMES` spelling and returning `BlindnessViolation`.
- **Post-revert green:** the same command exited 0: 1 passed, 1 skipped.

## 3. Aggregation input order

- **Mutation:** In `crates/gauntlet-app/src/compare/aggregate.rs`, changed
  the aggregate `ComparatorIdentity` model field from `String::new()` to
  `anchor.identity().runner().to_owned()`. Since `anchor` is `docs.first()`,
  the serialized aggregate became input-order dependent.
- **Fail command/evidence:** `cargo nextest run -p gauntlet-app compare::aggregate::tests::deterministic_aggregation`
  exited 100; the selected test panicked at `aggregate.rs:606:13` with
  `input order must never change the aggregate bytes`. The displayed records
  differed at `comparator_identity.model` (`critic-b` versus `critic-a`).
- **Restoration:** Restored the identity model field to `String::new()`.
- **Post-revert green:** the same command exited 0: 1 passed, 159 filtered.

## 4. Budget-truncation recording

- **Mutation:** In `crates/gauntlet-app/src/compare/review.rs`, changed the
  `ReviewOutcome` construction from `unavailable_evidence: unavailable` to
  `unavailable_evidence: Vec::new()`, discarding every accumulated truncation
  entry at the observable boundary.
- **Fail command/evidence:** `cargo nextest run -p gauntlet-app compare::review::tests::truncation_is_recorded`
  exited 100; the selected test panicked at `review.rs:619:9`:
  `the dropped dimension is recorded, never omitted` (left: 0, right: 1).
- **Restoration:** Restored `unavailable_evidence: unavailable` exactly.
- **Post-revert green:** the same command exited 0: 1 passed, 159 filtered.

## 5. `ComparisonRecord` / envelope-digest coupling

- **Mutation:** In the deterministic comparator's `ComparisonRecord::new`
  call in `crates/adapters/gauntlet-adapter-compare-diff/src/lib.rs`, replaced
  `envelope.envelope_digest.clone()` with
  `envelope.observation.observation_digest().clone()`. This made the record
  carry a valid digest of the wrong object.
- **Fail command/evidence:** `cargo nextest run --test p2_context_isolation the_envelope_is_structurally_blind_and_carries_instrument_reality`
  exited 100; the selected P2 coupling test panicked at
  `p2_context_isolation.rs:461:5` because `record.envelope_digest()` differed
  from `built.envelope_digest()`.
- **Restoration:** Restored `envelope.envelope_digest.clone()` exactly.
- **Post-revert green:** the same command exited 0: 1 passed, 1 skipped.

## 6. Deterministic-diff exit-status residual derivation

- **Mutation:** In `crates/adapters/gauntlet-adapter-compare-diff/src/lib.rs`,
  inserted `if status != 0 { continue; }` before the existing zero-status
  skip. Thus both zero and non-zero exit statuses bypassed the residual
  construction; a failing instrument could no longer produce its blocking
  residual.
- **Fail command/evidence:** `cargo nextest run --test p2_context_isolation the_envelope_is_structurally_blind_and_carries_instrument_reality`
  exited 100; the selected P2 instrument-residual path panicked at
  `p2_context_isolation.rs:430:5`, receiving `Pass` where its failing
  exit-status instrument requires `Fail`.
- **Restoration:** Removed the inserted non-zero skip and restored the exact
  original `if status == 0 { continue; }` derivation guard.
- **Post-revert green:** the same command exited 0: 1 passed, 1 skipped.

## Restoration confirmation

After the sixth revert, byte-for-byte comparisons against the snapshots made
immediately before this exercise passed for:

- `crates/gauntlet-app/src/compare/envelope.rs`
- `crates/gauntlet-app/src/compare/aggregate.rs`
- `crates/gauntlet-app/src/compare/review.rs`
- `crates/adapters/gauntlet-adapter-compare-diff/src/lib.rs`

The report is the sole artifact written by this teeth exercise. These are
bounded mutation results; they establish that the six named focused tests
reject the specified regressions, not a universal absence-of-regressions
claim.
