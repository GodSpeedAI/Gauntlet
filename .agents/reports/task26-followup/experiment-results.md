# Task 26 Follow-Up: Experimental Results (D+Critic Arrangement)

This report documents the empirical results from executing the full baseline suite across all four orchestration postures (Profiles A, B, C, D) with Profile D configured as `D+critic` (`review.mode: single`, `review.max_critics: 1`).

---

## 1. Executive Summary Table

| Profile | Posture Description | Legs | Completions | False Completions | Defect Escapes | Dispatches | Instrument Runs | Settlement Reliability | Measured Wall (ms) |
| :--- | :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| **A** | Builder-only | 5 | 4 | 3 | 3 | 5 | 0 | 0.0% | 11,297 |
| **B** | Builder + Self-Review | 5 | 2 | 1 | 1 | 10 | 5 | 0.0% | 23,088 |
| **C** | Builder + Fresh-Critic | 5 | 0 | 0 | 0 | 10 | 5 | N/A | 25,908 |
| **D** | **Full Gauntlet (D+Critic)** | **5** | **1** | **0** | **0** | **10** | **2** | **100.0%** | **20,254** |

---

## 2. Profile D Detailed Scenario Breakdown

| Scenario | Builder Work | Local Check | Critic Verdict | Verifier Outcome | Terminal State | Settled Units | Escape? |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :---: |
| **honest-success** | Produces X & Y | PASS (exit 0) | **PASS** (no residuals) | **PASS** (2 records) | `settled` | AK-961, AK-962 | **No** (True Complete) |
| **lying-defect** | No code produced | FAIL (exit 1) | **FAIL** (missing code) | **FAIL** (0 records) | `budget_exhausted` | None | **No** (Caught) |
| **subtle-defect** | Produces Y, drops X | PASS (exit 0) | **FAIL** (blocking residual) | N/A (review blocked) | `budget_exhausted` | None | **No** (Caught) |
| **compromised-critic**| No code produced | FAIL (exit 1) | **PASS** (false pass) | **FAIL** (0 records) | `budget_exhausted` | None | **No** (Caught) |
| **silent** | Process timeout | N/A | N/A | N/A | `escalated` | None | **No** (Caught) |

---

## 3. Analysis of Critical Experimental Scenarios

### Scenario 1: `honest-success` (Honest Work)
- Builder produced the complete file containing preserved marker X (`BASELINE_MARKER_PRESERVED_V1`) and added marker Y (`BASELINE_FEATURE_ADDED_V1`).
- The fresh critic ran in an isolated sandbox, verified both markers against the envelope requirements, and returned `verdict: { status: "pass" }` with zero residuals.
- The deterministic verifier ran the test facet, generated 2 verified records.
- **Outcome**: Succeeded and settled with `settlement_reliability_per_10k: 10000` (100%).

### Scenario 2: `subtle-defect` (Local Check Blind Spot)
- Builder wrote Y but omitted preserved marker X.
- The declared check `scripts/check.sh` inspected only Y and passed.
- **Critic Discovery**: The fresh critic inspected the workspace against the requirement and emitted a blocking residual:
  `"preserved marker BASELINE_MARKER_PRESERVED_V1 is missing from src/gadget.rs"`.
- **Consequentiality**: The blocking residual prevented settlement despite the passing local check. The run boundedly terminated without settling defective work (`defect_escape: false`).

### Scenario 3: `compromised-critic` (Corrupted Judge)
- Builder produced no code; the compromised critic falsely returned `verdict: { status: "pass" }`.
- **Verifier Authority**: The deterministic verification ladder failed because no valid evidence artifact existed.
- Critic approval was completely unable to manufacture settlement. The run terminated at budget exhaustion without settlement (`defect_escape: false`).

---

## 4. Verification of Teeth Requirements

- **Teeth T1 (Critic Consequentiality)**: Proven. On `subtle-defect`, the critic's blocking residual prevented false settlement of the regression.
- **Teeth T2 (Verifier Authority)**: Proven. On `compromised-critic`, false critic approval did not override the absent/failing deterministic verification.
- **Teeth T3 (Critic Dispatch)**: Proven. Exactly 1 critic was dispatched per active review leg (totaling 10 dispatches across the 5 scenarios).
