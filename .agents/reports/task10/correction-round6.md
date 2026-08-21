# Task 10 correction round 6 — provenance API hardening checkpoint

## Scope changed

This slice removes public writable fields from the serializable
`ComparisonEnvelope`. The comparator adapters continue to accept only
`TrustedComparisonContext`; consumers now receive read-only envelope accessors.
The application issuer is the only production callsite that assembles an
envelope and sets its digest. Adapter and contract fixtures were migrated to
the read-only surface. The P2 stale-envelope test now documents the important
compile-time boundary: a caller cannot replace raw `reference_view` or an
observation field and recompute through the former public-field route.

## Evidence

- `cargo test -p gauntlet-cli --test p2_context_isolation`: 5 passed.
- `cargo test -p gauntlet-adapter-compare-diff -p gauntlet-adapter-compare-model -p gauntlet-app --lib`: passed (175 app tests plus adapter suites).
- `just test`: 685/685 passed.
- `just check`, `just lint`, `just deny`, `just boundaries` (194/194), and
  `just ruler`: passed.

## Required honest residuals — no closure claim

This is **not** a complete resolution of final-confirmation-round3 defect A.
`ComparisonEnvelope::assemble`, `FrozenReferenceProjection::load`, and
`TrustedComparisonContext::from_frozen_projection` remain public port APIs.
Although fields are now opaque and the comparator accepts only the trusted
type, those public construction APIs still permit a caller who controls an
evidence store to construct/re-admit a context. The required application-owned
issuer that loads the durable run, verifies its frozen `ReferenceVersion`,
reopens the stored observation bundle from the observation owner, and alone
mints the comparator capability has **not** landed. Therefore raw recomputed
reference-view and instrument-text smuggling are not yet conclusively refused
before dispatch; the requested fail-first teeth cannot honestly be claimed.

The separate accepted-record/durable tuple and adversarial-source findings from
final confirmation round 3 remain open and were not altered by this slice.
Task 10 remains IN-FLIGHT/BLOCKED and is neither closed nor settled. No frozen
reference, `CURRENT_STATUS.md`, or reference-candidate material was changed.
