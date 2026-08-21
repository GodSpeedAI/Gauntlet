# Task 10 — Round 4 builder checkpoint

Started direct repair of the four requested remaining blockers in the existing dirty worktree. Scope is limited to comparator trusted-dispatch provenance, mandatory durable acceptance, sealed adversarial composition, and admitted-job cleanup/quarantine. No frozen reference/ruler, `CURRENT_STATUS.md`, or reference-candidate material will be modified. This is only a start checkpoint; no verification or closure claim.

## Final implementation report (no closure claim)

### Landed

- `Comparator`, `comparison_document`, and two-phase `admit` now accept only opaque `TrustedComparisonContext`; raw `ComparisonEnvelope` remains serializable/auditable but no longer type-checks as dispatch input. Diff/model/review and shared fixtures were migrated. Adapter teeth construct a raw envelope with the real frozen digest, an arbitrary view, and a recomputed self-digest; trusted projection construction refuses it before adapter admission.
- Replaced the public callback-shaped accepted-comparison recorder with concrete `MandatoryComparisonAcceptance`. `ModelComparator::new` requires it; it pins evidence then calls `StateStore::accept_comparison`. SQLite acceptance refuses absent durable runs, mismatched frozen reference digest, and absent/ambiguous durable invocation binding before writing a comparison. P2 now creates a durable run before critic dispatch and proves reopen persistence.
- Retained sealed `AdversarialComparatorSource`/crate-owned `AdversarialSource` high-assurance slot and its normal-source type separation.
- Added a scheduling error cleanup boundary: all source/probe/admission errors after any pending admission cancel remaining pending work; cancellation failures quarantine leases and return recovery-required. Added source-failure-after-admission tooth. Existing collection/cancellation and bounded model timeout paths remain covered.

### Verification

- focused ports/app/diff/model/sqlite/P2 suite: PASS
- `cargo clippy --workspace --all-targets -- -D warnings`: PASS
- `cargo nextest run --workspace`: 679/679 PASS
- `just check && just lint && just deny && just boundaries && just ruler`: PASS (boundaries 194/194).

### Honest remaining limitations

1. The sealed adversarial wrapper is application-owned but this repository still lacks a production review composition factory that constructs it specifically from `ModelComparator::adversarial()`; its tests use an internal scripted source. Thus the exact requested production wiring is not established.
2. `MandatoryComparisonAcceptance` prevents the old public no-op-recorder constructor path and SQLite checks durable run/invocation/reference bindings. Its ports are still implementable by an alternate state adapter, and evidence pin plus state record cannot be one cross-store atomic transaction; ordered pin-then-validated-record is implemented, with a pinned orphan possible if state record fails.
3. Cleanup covers review scheduling failures and model nonterminal poll/probe failures. There is no added dedicated forever-running-primary tooth beyond the existing bounded-hang typed timeout test, and aggregate failures occur only after fan-in has emptied pending work.

No frozen ruler/Core material, CURRENT_STATUS, or reference-candidate document was edited by this slice. Pre-existing dirty changes to CURRENT_STATUS/reference-candidates remain in the worktree and were not touched. This report makes no Task-10 closure or settlement claim.


---

## Round 4 residual-evidence completion

### Landed

1. `PendingComparison::collect` now retains the admitted handle on failure.
   Review actively cancels the failing primary and all siblings. A successful
   cancellation releases its lease; cancellation failure quarantines it and
   returns `ComparatorError::RecoveryRequired`. Deterministic teeth cover a
   forever-running primary for both outcomes. The model adapter also has a
   bounded-running-primary tooth that verifies direct collection actively calls
   cancellation.
2. Comparator factories are semantic ports. The model adapter provides a normal
   `ModelComparatorSource` and a separate `ModelAdversarialSource`; the latter
   is constructed only by `ModelComparator::adversarial()`. Application review
   accepts the separate adversarial capability, not a normal source or label.
   The P2 composition test constructs `ReviewDeps` with the real adapter source
   through ports only. This is a construction proof, not a Task-20 claim that
   the CLI production root exists.
3. The P2 acceptance-refusal tooth uses a real durable state/evidence setup.
   It proves a post-pin `accept_comparison` failure returns an error and leaves
   no durable comparison record. The ordered evidence pin can leave an orphan;
   the test names that residue and proves it is not success.

### Verification

- focused `gauntlet-app` review: 14 passed
- `gauntlet-adapter-compare-model`: 7 passed
- P2 composition: 5 passed
- `cargo clippy --workspace --all-targets -- -D warnings`: passed
- `cargo nextest run --workspace`: 684/684 passed
- `just check`, `just lint`, `just deny` (pre-existing informational warnings),
  `just boundaries` (194/194), and `just ruler`: passed

No frozen reference/ruler, `CURRENT_STATUS.md`, or reference-candidate material
was edited by this round. This report is evidence only and makes no Task-10
closure or settlement claim.
