# Task 26 Follow-Up: Adjudication of Independent Review Findings

This document adjudicates the two core architectural questions raised by the independent verification reports ([opus-task26-independent-verification.md](file:///home/sprime01/projects/gauntlet/.agents/reports/opus-task26-independent-verification.md) and [sol-task26-independent-verification.md](file:///home/sprime01/projects/gauntlet/.agents/reports/sol-task26-independent-verification.md)).

---

## 1. Finding A: `authority: critic` vs `exercised_by: runtime`

### The Observation
The forensic reviews highlighted settlement ledger entries containing:
```text
required: critic
exercised_by: runtime
rationale_recorded: false
```
The reviewers questioned whether a settlement with `authority: critic` was legitimate if `exercised_by` recorded `runtime` rather than a direct critic model invocation.

### Specification Analysis (Core-Spec §5.3.2, §16.1, §17)
1. **Separation of Requirement vs Actor**:
   - `authority` defines the **governance tier** required by the Answer Key for the unit (e.g., `builder`, `critic`, `verifier`, `operator`).
   - `exercised_by` defines the **executing mechanism** that settled the transition. In an automated controller, the runtime itself executes the settlement transaction after verifying all prerequisites.
2. **Authority Is Not Delegation of Settlement**:
   - Core-spec Invariant I3 and I5 explicitly state: *A critic never settles work; opinion never authorizes.*
   - A model critic does NOT have write access to the state store or settlement ledger. The Gauntlet runtime is the sole deterministic execution kernel that transitions units to `Settled`.
3. **Verdict**: **Conformant with Specification**.
   - Recording `exercised_by: runtime` reflects actual architectural reality: the runtime admitted the settlement under the authority criteria required by the unit.

---

## 2. Finding B: `producer: instrument` and Builder-Authored Report Bytes

### The Observation
Reviewers noted that in certain fixtures, the builder script wrote an evidence report JSON file into `$workspace/evidence/`, and the instrument verification rung checked that file's existence and exit status. The reviews asked whether this violates `evidence_required.producer: instrument`.

### Specification Analysis (Core-Spec §14.3, §14.4)
1. **Evidentiary Hierarchy**:
   - Builder-authored self-reports (`builder_result@v1.artifacts`) are untrusted builder claims (§10.3 Rule 6).
   - An instrument verification rung (`VerificationAuthority` running `scripts/check.sh` / test runner) independently inspects the workspace, tests execution exit codes, and creates a `VerificationRecordDoc`.
2. **Corroboration vs Origination**:
   - The *verification record* that satisfies settlement is produced by the *verification instrument* (the verifier adapter), NOT by the builder.
   - Even if locator paths or test outputs originated from child executions, settlement is admitted exclusively on the basis of the independent verifier rung's verified observation.
3. **Verdict**: **Conformant with Terminology Clarification**.
   - The verified evidence used by the settlement engine is strictly instrument-produced. Builder-generated workspace artifacts serve as inputs to the verification instrument.
