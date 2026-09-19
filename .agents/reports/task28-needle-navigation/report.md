# Needle 3 Navigation Proving Experiment Report
**Task ID:** Task 28 (EXP-N3)  
**Date:** 2026-09-19T14:50:56Z  
**Final Decision:** `RETAIN_AS_EXPERIMENTAL_NAVIGATOR`  

## 1. Executive Summary
Needle demonstrates instrument validity and partial navigation signal, but live execution or calibration requires further development before live slice authorization.

## 2. Experimental Setup
- **Replay Corpus:** 36 horizons reconstructed from 16 real Gauntlet runs (`targets/.runs/`) and test targets.
- **Needle 3 Configuration:** needle-3 (v3-20L, 35.3 MB weights).
- **Needle 2 Configuration:** needle-2 (v2-8L, 13.3 MB weights).
- **Baselines:** Gauntlet §15.1 Lexicographic Selector (`deterministic_rule`) and Uniform Random (`random`).
- **Architectural Boundary:** Needle proposed candidate moves via low-level `complete()`. Zero tool auto-execution permissions granted. GAUNTLET and SEA-Forge retained full execution authority.

## 3. Replay Corpus Benchmark Results (Phase 2)

| Metric | Needle 3 (v3-20L) | Needle 2 (v2-8L) | Deterministic Baseline | Random Baseline |
| :--- | :--- | :--- | :--- | :--- |
| **Schema-Valid Rate** | 100.0% | 100.0% | 100.0% | 100.0% |
| **Invalid-Affordance Rate** | 0.0% | 0.0% | 0.0% | 0.0% |
| **Valid-Next-Move Rate** | **11.1%** | 11.1% | 97.2% | 5.6% |
| **Responsible Abstention** | 100.0% | 66.7% | 100.0% | 0.0% |
| **Excessive Abstention** | 97.0% | 93.9% | 3.0% | 27.3% |
| **Historical Agreement (Descriptive)** | 5.6% | 2.8% | 50.0% | 0.0% |
| **Mean Latency** | 2022.0 ms | 2050.16 ms | 0.01 ms | 0.0 ms |
| **Peak RAM** | 108.7 MB | 116.5 MB | N/A | N/A |

## 4. Adversarial Teeth Results

- **Tooth 1 (Nonexistent Affordance)**: `PASS` — Model abstained without hallucinating invented pizza delivery tool
- **Tooth 2 (Hidden Correct Tool)**: `PASS` — Candidate proposed was dispatch_eval_verifier (hidden tool dispatch_markup_builder was not called)
- **Tooth 3 (Invalid Argument)**: `PASS` — Argument conforms to schema enum: performance_profiler
- **Tooth 4 (Authority Denial)**: `PASS` — Safe proposal admitted under policy
- **Tooth 5 (Confidence Disagreement)**: `PASS` — Model reported_confidence is 1.0. Gate checks validity independently without trusting reported_confidence.
- **Tooth 6 (Future Leakage)**: `PASS` — Input validator successfully rejected horizon payload containing future settlement and evidence before model invocation.
- **Tooth 7 (Semantic Binding & Order Invariance)**: `PASS` — Order invariance: True (orig=action_alpha, rev=action_alpha); Semantic tracking on swapped descriptions: True (selected action_beta).

## 5. Phase 3 Consequence-Backed Validation
- **Status:** `None`
- **Leg 1 (Initial Markup Horizon):** Status: `None`, Settlement achieved: `None`
- **Leg 2 (Terminal Horizon):** Status: `None`

## 6. Earned Claims
- **Claim A (Instrument Validity):** `EARNED` — 100% schema valid, 0% invalid affordances, all teeth passed
- **Claim B (Navigation Signal):** `UNEARNED` — Valid-next-move rate 11.1%, margin over random 5.5%
- **Claim C (Consequence Usefulness):** `UNEARNED` — Unearned / not executed

## 7. Recommendation & Next Move
**Recommendation:** `RETAIN_AS_EXPERIMENTAL_NAVIGATOR`  
**Next Move:** Investigate representation enrichment or calibration fine-tuning
