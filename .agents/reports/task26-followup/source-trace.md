# Task 26 Follow-Up Source Trace: Implementation & Spec Adjudication

This document establishes the exact source code paths, data structures, and specification contracts governing the D+critic arrangement and the findings from independent reviews.

---

## 1. Why Task 26 D Produced `review_dispatches = 0`
- **Location**: `tests/fixtures/baseline/gadget-task/GAUNTLET.md:15`
- **Fact**: `review.max_critics` was set to `0`.
- **Effect**: During baseline execution, the controller resolved `review_max_critics = 0`, bypassing the model comparator dispatch loop entirely.

---

## 2. How `review.max_critics` Governs Critic Dispatch
- **Location**: [crates/gauntlet-app/src/control/controller.rs](file:///home/sprime01/projects/gauntlet/crates/gauntlet-app/src/control/controller.rs) & [crates/gauntlet-cli/src/compose.rs](file:///home/sprime01/projects/gauntlet/crates/gauntlet-cli/src/compose.rs#L970-L974)
- **Mechanism**: `ComparisonDeps::model_dispatches` carries `context.resolved.review_max_critics`. The controller review stage evaluates whether the review budget permits dispatching model comparators. When `model_dispatches > 0`, `ReviewService` admits up to `max_critics` comparator instances per review cycle.

---

## 3. What Context a D Critic Receives
- **Location**: [crates/gauntlet-ports/src/comparator.rs](file:///home/sprime01/projects/gauntlet/crates/gauntlet-ports/src/comparator.rs) & [crates/adapters/gauntlet-adapter-compare-model/src/lib.rs](file:///home/sprime01/projects/gauntlet/crates/adapters/gauntlet-adapter-compare-model/src/lib.rs#L240-L245)
- **Envelope Structure**: The critic receives a strict `ComparisonEnvelope` containing:
  - `run_id`, `iteration_no`
  - `reference_digest` and reference contract
  - `observation_digest` and `ObservationView` (summary of observation facets: files, test exit status, evidence refs)
  - `envelope_digest`
- **Delivery**: Serialized as `ContextEnvelope` to `<state>/runs/<run_id>/inv/<inv_id>/<inv_id>.envelope.json` and referenced via `ENVELOPE-PATH` in the prompt.

---

## 4. Isolation: What the Critic Is Structurally Excluded From
- **Excluded**:
  - Builder prompt, trajectory, reasoning, chain-of-thought, or intermediate tool interactions.
  - Builder identity or persona.
  - Builder claimed exit code or self-check opinions.
  - Prior verdicts or reviews.
- **Spec Anchor**: Core-spec §13.2 ("Fresh Context Comparator Envelope"). The comparator receives only the declared observation projection and the reference contract.

---

## 5. Object Produced by the Critic
- **Contract**: `role_result@v1` wrapping `payload_schema: comparison_record@v1`.
- **Fields** ([crates/gauntlet-ports/src/comparison_record.rs](file:///home/sprime01/projects/gauntlet/crates/gauntlet-ports/src/comparison_record.rs#L38-L50)):
  - `run_id`, `iteration_no`, `reference_digest`, `observation_digest`, `envelope_digest`, `comparison_contract_version`
  - `comparator_identity`: `{ "runner": "...", "model": "...", "dimension": "..." }`
  - `residuals`: list of `ResidualDoc` (`{ "id", "dimension", "unit_refs", "expected", "observed", "evidence_refs", "severity", "confidence", "disputed", "diagnosis", "cheapest_discriminating_test", "candidate_corrections" }`)
  - `hard_gate_results`: list of hard gate outcomes
  - `verdict`: `{ "status": "pass" | "fail", "rationale": "..." }`

---

## 6. How Residuals Enter the Controller
- **Location**: [crates/adapters/gauntlet-adapter-compare-model/src/lib.rs](file:///home/sprime01/projects/gauntlet/crates/adapters/gauntlet-adapter-compare-model/src/lib.rs#L457-L463) & `ComparisonAuthority::accept_comparison`
- **Path**:
  1. `ModelComparator::collect` decodes and validates binding (`validate_document_binding`).
  2. `ComparisonAuthority` records the comparison document in `StateStore::accept_comparison`.
  3. Admitted residuals are inserted into the active iteration residual ledger.

---

## 7. How Blocking Residuals Prevent Settlement
- **Location**: [crates/gauntlet-app/src/settle/mod.rs](file:///home/sprime01/projects/gauntlet/crates/gauntlet-app/src/settle/mod.rs)
- **Precondition**: `SettlementService::evaluate_unit_settlement` checks whether any open `Severity::Blocking` residuals attach to the unit or reference.
- **Invariant**: If any blocking residual is unresolved, the unit CANNOT transition to `Settled`.

---

## 8. Why Critic PASS Cannot Settle Without Verifier Evidence
- **Location**: [crates/gauntlet-app/src/settle/mod.rs](file:///home/sprime01/projects/gauntlet/crates/gauntlet-app/src/settle/mod.rs)
- **Precondition**: Settlement requires all declared unit acceptance criteria to be satisfied with passing `VerificationRecord` documents produced by instruments (`evidence_required`).
- **Core Invariant I3 / I5**: A critic's comparison record is an observation/comparator record; it does NOT constitute verifier evidence and has zero authority to satisfy an instrument evidence requirement.

---

## 9. What Deterministic Verifier Decides
- **Location**: [crates/gauntlet-app/src/verify/mod.rs](file:///home/sprime01/projects/gauntlet/crates/gauntlet-app/src/verify/mod.rs)
- **Action**: Runs the configured verification ladder rungs (e.g. `check.sh`, regex matchers, build/test exit codes) against the workspace and generates immutable `VerificationRecordDoc` instances stored in `EvidenceStore`.

---

## 10. Actor Recorded as Exercising Settlement Authority
- **Finding**: Settlement records store `authority: critic` (or `verifier`), with `exercised_by: runtime`.
- **Spec Meaning**:
  - `authority`: The required governance authority tier specified in the Answer Key / Unit definition.
  - `exercised_by`: The executing substrate actor (the Gauntlet runtime controller executing under the verified admission rules).

---

## 11. Core-Spec §16/§17 Intent
- `authority` specifies the policy threshold required to admit settlement.
- A runtime exercising settlement does so strictly as an instrument of the verified evidence and policy rules, never by arbitrary discretion.

---

## 12. Core-Spec §14.3 `producer: instrument` Semantics
- When a builder writes a report file and an independent test runner executes `scripts/check.sh` verifying that file and the workspace, the verification record generated by the instrument verification rung is an **instrument-produced** verification artifact (`evidence_required.producer: instrument`).
