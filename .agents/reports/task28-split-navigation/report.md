# EXP-N3-SPLIT: Split-Contract Navigation Proving Experiment Report
**Date:** 2026-09-19T15:32:10Z  
**Corpus Hash:** `0de55f35552f1736e64ec49bbb2f8bed1f4c21a8f952f020d84f6ce762494760`  
**Descriptions Hash:** `148ef4106db34f543360b66cfe3837fb3e9d8e01a079e20c7f23b48c5e47f6f6`  
**Final Decision:** `RETAIN_AS_EXPERIMENTAL_NAVIGATOR`  
**Adjudicated Locus:** **Locus A (Affordance Selection Failure)**  

## 1. Executive Summary
Needle 3 selection validity remained low (22.2%) even with argument obligations removed. Needle 3 cannot reliably discern the appropriate affordance from raw Gauntlet state summaries. Furthermore, the deterministic §15.1 rule baseline achieved near-perfect accuracy and Needle provided 0 needle-only valid moves.

## 2. Experimental Arms Comparison

### A. Affordance Selection Metrics
| Metric | Arm A (Coupled Needle 3) | Arm B (Split Needle 3) | Arm C (Rule Baseline) | Arm D (Random Baseline) |
| :--- | :--- | :--- | :--- | :--- |
| **Selection Validity Rate** | N/A (coupled) | **22.2%** | 100.0% | 27.8% |
| **Schema-Valid Selection** | 100.0% | 100.0% | 100.0% | 100.0% |
| **Invalid-Affordance Rate** | 0.0% | 0.0% | 0.0% | 0.0% |
| **Responsible Abstention** | 100.0% | 100.0% | 100.0% | 100.0% |
| **Excessive Abstention** | 97.0% | 0.0% | 0.0% | 34.6% |
| **Historical Agreement (Descriptive)** | 11.1% | 0.0% | 33.3% | 8.3% |
| **Mean Latency** | 18.2 ms | 1058.46 ms | 0.01 ms | 0.0 ms |
| **Peak RAM** | 98.4 MB | 100.8 MB | N/A | N/A |

### B. Combined System Metrics (Selection + Deterministic Binder)
| Metric | Arm A (Coupled) | Arm B (Split Needle 3) | Arm C (Rule) | Arm D (Random) |
| :--- | :--- | :--- | :--- | :--- |
| **Conditional Binding Success** | N/A | 100.0% | 100.0% | 100.0% |
| **Validation Pass Rate** | 100.0% | 88.9% | 100.0% | 83.3% |
| **Authority Denial Rate** | 0.0% | 2.8% | 0.0% | 0.0% |
| **Full Valid Next Move** | 11.1% | **22.2%** | 100.0% | 27.8% |

## 3. Incremental Value Over Deterministic Rule Baseline
| Quadrant | Count | Percentage | Meaning |
| :--- | :--- | :--- | :--- |
| `both_valid` | 8 | 22.2% | Both Needle 3 and Rule found valid move |
| `needle_only_valid` | **0** | **0.0%** | Needle found valid move where Rule failed (Incremental Value) |
| `rule_only_valid` | 28 | 77.8% | Rule found valid move where Needle failed |
| `neither_valid` | 0 | 0.0% | Both failed |

## 4. Adversarial Teeth Battery (T1–T7)
- **Tooth T1 (Nonexistent Affordance)**: `PASS` — Abstained: True, Affordance: None, Rationale: No tool available for ordering or delivery.
- **Tooth T2 (Hidden Correct Tool)**: `PASS` — Selected: dispatch_eval_verifier (markup builder was unexposed and was NOT selected)
- **Tooth T3 (Affordance Immutability)**: `PASS` — Input: dispatch_check_runner -> Output affordance: dispatch_check_runner, Status: FAILED_PREREQUISITE
- **Tooth T4 (No Fabrication)**: `PASS` — Status: FAILED_PREREQUISITE, Error: Unit AK-9011 is not currently open in represented state, Arguments: {}
- **Tooth T5 (Attribution Preservation)**: `PASS` — Separation verified: binding failure does not taint valid selection; binding success does not rescue invalid selection.
- **Tooth T6 (Temporal Cutoff Integrity)**: `PASS` — Future leakage guard verified: detected forbidden keys ['open_units', 'post_cutoff_diff']
- **Tooth T7 (Binder Neutrality)**: `PASS` — Binder neutrality confirmed: binder does not rank or substitute affordances, and strictly operates on supplied affordance_id.

## 5. Phase-3 Authorization Gate & Consequence Validation
- **Authorized:** `False`
- **Status:** `WITHHELD_DUE_TO_PHASE_2_GATE`
- **Reason:** Needle selection validity (22.2%) or margin (-5.6%) did not satisfy authorization threshold (>=70% and >=25%).

## 6. Earned Claims
- **Claim A (Instrument Validity):** `EARNED` — Schema-valid 100%, invalid affordance 0%, all 7 teeth passed.
- **Claim B (Navigation Usefulness):** `UNEARNED` — Selection validity: 22.2% (threshold >=70%), margin over random: -5.6%.
- **Claim C (Consequence Usefulness):** `UNEARNED` — Withheld per frozen Phase-3 gate.

## 7. Next Affordable Move
**Next Move:** Record Representation Enrichment as future candidate hypothesis (do not implement in this experiment)
