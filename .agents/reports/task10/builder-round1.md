# Task 10 — Builder round 1 report (recovery documentation)

**Date:** 2026-08-20  
**Scope:** documentation-only recovery checkpoint and report. This report does
not confer settlement authority. It modifies neither implementation nor tests,
ruler/reference material, comparison contract, Cargo files, reference
candidates, or `CURRENT_STATUS.md`.

## Evidence convention

“Recorded” means a prior builder checkpoint says it happened. “Recovery
reported green” means the recovery task supplied that focused outcome, but no
verbatim command transcript is stored in this report directory. “Inspected”
means this finisher read the indicated working-tree file. Neither is a proof
of the total system settling.

## Per-slice account

| Slice | What landed / files | Test evidence and decisions | Limits, deviations, residuals, recorded acts |
|---|---|---|---|
| D1 | **Recorded** in `builder-checkpoints.md`: domain `comparison.rs`; ports `comparator.rs`, new `comparison_record.rs`, `role_result.rs`, comparator contract/stubs; arch inventory/pins; compare-model member skeleton. Reported sizes include ~950 lines for the codec. | Recorded counts: domain+ports 146/146 and arch 194/194. Recovery reported the D1-focused gate green. Recorded decision: §13.2 requires `ComparisonRecord.envelope_digest`; envelope observation is the claims-free facet view; strict B.4 decoding was selected. | No independent line recount or raw recovery output. Recorded acts: new workspace member; parser inventory/pin growth; no external allowlist growth. Boundary vocabulary decision is recorded in the D1 checkpoint, not re-adjudicated here. |
| D2 | **Recorded**: `.gauntlet/comparison-contract.yaml`; app `compare/{contract,envelope,mod}.rs`; app wiring (reported ~430/~530 lines). | Recorded app count 152/152 (compare 11/11); recovery reported focused green. Recorded decision: exact allowlist inputs, recompute observation digest at the consumption boundary, canonical envelope digest, and structural claims exclusion. | No independently retained stdout. The checkpoint relies on Core-spec §§9.5, 13.2, and 5.3.5; its quoted normative basis is “stored on both the RoleInvocation and the ComparisonRecord.” |
| D3 | **Recorded**: `crates/gauntlet-app/src/compare/aggregate.rs` (reported ~640 lines). | Recorded app count 157/157 (compare 16/16); recovery reported focused green. Recorded decision: BTreeMap canonical merge; blocking residual forces fail; missing/empty input is typed rather than a pass. | No raw transcript or independent byte-order experiment is attached here. No extra recorded act beyond the new app module. |
| D4 | **Recorded**: `crates/gauntlet-app/src/compare/review.rs` (reported ~700 lines). | Recorded app count 160/160 (compare 19/19); recovery reported focused green. Recorded decision: adaptive probes settle only instrument-positive dimensions; budget truncation becomes `UnavailableEvidence`; full `BudgetMeter` wiring remains Task 12. | No raw transcript. The reported scope is Core-spec §§13.3–13.5 / AK-033–AK-034, not a proof of controller or settlement behavior. |
| D5 | **Recorded**: diff adapter `src/lib.rs` and new model adapter `src/lib.rs` (reported ~470/~780 lines). | Recorded counts: diff 5/5, model 4/4, ports 79/79; recovery reported focused green. Recorded decision: in-crate scripted critic rather than modifying the pinned mock scenario dialect; no external adapter allowlist growth. | No raw transcript. Adapter conformance is not a claim that every production composition path or future runner settles. |
| D6 | **Inspected**: untracked `crates/gauntlet-cli/tests/p2_context_isolation.rs`, currently 511 lines, containing exactly `the_envelope_is_structurally_blind_and_carries_instrument_reality` and `teeth`. The initial static inspection was 491 lines; a concurrent boundary-gate correction subsequently replaced hard-coded relative paths. The dated AK-026/027 and AK-032/033/034 locator-materialization entries are present in `.agents/reports/reference-candidates.md`; this finisher did not edit them. | Recovery reported `cargo nextest run -p gauntlet-cli --test p2_context_isolation` green. The first test composes real named Task-9 observation adapters, bundle assembly, envelope building, and `DiffComparator`; it asserts six builder-side canaries are absent from serialized bytes, instrument failure is present, tampering yields `ObservationDigestMismatch`, the residual derives from exit status/evidence, and all three digests couple. `teeth` injects `builder_reasoning` and every deny-listed field spelling and expects `assert_blind` to reject them. | The test establishes these assertions only for its constructed lying-builder scenario and its configured deny list. It does not by itself prove all fields, all observers/comparators, model-comparator behavior, broad gate success, or settlement. Raw nextest stdout/count was not retained in Task-10 reports. |

## Spec basis and deviations

The recorded implementation cites Core-spec §§5.3.5, 6.2–6.3, 9.5, 12.3–12.4,
13.2–13.5, Annex B.1/B.4, and AK-026, AK-027, AK-032, AK-033, AK-034. The
D6 test directly embodies the brief’s required serialized-envelope blindness,
facet-reality, digest-refusal, comparison-consumption, and AK-027 tooth
assertions.

Deviation from the brief: interruption prevented the builder’s contemporaneous
D6 checkpoint and final report. This recovery report supplies them after the
fact, preserving the distinction between recorded claims and available
execution evidence. It does not convert an unavailable transcript into one.

## Gate record (output placeholders where no transcript is available)

| Required command | Available evidence / status |
|---|---|
| `just ruler` (before/after) | Recovery audit recorded **PASS** before focused recovery. Final rerun output: **not captured / not claimed**. |
| `cargo nextest run -p gauntlet-domain` | Recovery reported **green** (parent output showed exit 0); full stdout retained here: **not captured**. |
| `cargo nextest run -p gauntlet-ports` | Recovery reported **green**; full stdout/count: **not captured**. |
| `cargo nextest run -p gauntlet-app compare::` | Recovery reported **green**; full stdout/count: **not captured**. |
| `cargo nextest run -p gauntlet-adapter-compare-diff` | Recovery reported **green**; full stdout/count: **not captured**. |
| `cargo nextest run -p gauntlet-adapter-compare-model` | Recovery reported **green**; full stdout/count: **not captured**. |
| `cargo nextest run -p gauntlet-cli --test p2_context_isolation` | Recovery reported **green**; static source has two tests; verbatim stdout/count: **not captured**. |
| `just check && just lint && just deny && just boundaries` | **Not PASS.** Parent recovery reported the chain exited 100 at `just boundaries`, with eight hardcoded-relative-path findings (including P2 paths). Correction was in progress when this report was written. No replacement green output is claimed. |
| `cargo nextest run --workspace` | **Not captured / not claimed.** |

`du -sh target` at report inspection: **1.8G**.

## Teeth readiness (make-and-revert candidates)

An orchestrator can demonstrate that the intended focused proofs discriminate by
making each bounded edit, observing the named failure, and reverting it:

1. Remove envelope observation-digest recomputation; the P2 tamper refusal
   should fail.
2. Remove the D6 serialized deny-list assertion or weaken `assert_blind`; the
   poisoned `teeth` case should fail to reject `builder_reasoning`.
3. Make D3 aggregate ordering input-dependent; `deterministic_aggregation`
   should fail byte-identical output checks.
4. Silence D4 budget-truncation gaps; `truncation_is_recorded` should fail.
5. Drop `envelope_digest` from a comparison record/coupling; shared
   comparator/P2 digest assertions should fail.
6. Remove the diff residual’s facet exit-status derivation; the P2 blocking
   residual/evidence assertions should fail.

These are readiness instructions, not mutations performed by this finisher.

## Remaining bounded residuals

- Required final gates are incomplete/red as recorded above; a green focused
  P2 result is not a green final gate.
- The evidence directory has no verbatim focused-gate logs, so this report
  deliberately does not claim their exact current counts or timing.
- D1–D5 remain implementation plus recorded/recovery-focused evidence pending
  the orchestrator’s independent comparison, correction, verification, and
  settlement process.
