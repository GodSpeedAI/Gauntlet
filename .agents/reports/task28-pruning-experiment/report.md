# EXP-N3-PRUNE: Candidate-Pruning and Search-Efficiency Proving Report

> [!WARNING]
> **POST-EXECUTION SCIENTIFIC AUDIT ADDENDUM (2026-09-19):**
> Following execution, a formal scientific audit was performed (see [`audit_report.md`](audit_report.md)).
> - **Tooth T3 FAILED:** Order reversal caused substantive inversion of candidate selection (dropping the advancing probe in forward order); the original runner passed due to an erroneous disjunctive (`or`) assertion. Under the frozen gate, this **automatically revokes Phase-2 authorization**.
> - **Random Comparison INCONCLUSIVE:** Recall advantage (+5.6%) is finite-sample noise ($Z = +0.53, p = 0.3503$); on operational horizon success, Needle achieved 56.2%, losing to exact random expectation (61.7%). Claim P1 is **NOT EARNED**.
> - **Paired Test NOT SIGNIFICANT:** Discordance (7 vs 5) has exact binomial $p = 0.3872$ (95% CI $[-29.5\%, +54.5\%]$). Claim P3 is **NOT EARNED**.
> - **Strict Pareto Dominance by Random:** Random expectation achieves higher horizon success at 48.9% less wall-clock with zero latency. Claim P2 Meaning B **FAILED**.
> - **Audited Final Disposition:** **`REMOVE_NEEDLE`**. The original mechanical gate result below is preserved as historical record only.

**Original Run Date:** 2026-09-19T15:59:35Z  
**Audit Date:** 2026-09-19T16:25:00Z  
**Dataset SHA-256:** `ca2bf8c3e05925bffc5c082e0c8f8884df643967042a05acd87edb0203f8b014`  
**Original Mechanical Recommendation:** `USE_IN_ROUTE_DISCOVERY_EXPERIMENTS` (REVOKED)  
**Audited Final Disposition:** `REMOVE_NEEDLE`  

## 1. Executive Summary (Original Run Record)
This experiment evaluated whether Cactus Needle 3 can serve as a cheap candidate-pruning instrument 
when deterministic hard gates leave N in [4, 5] candidate affordances unresolved and probing every candidate 
incurs real execution payment. The comparison evaluated Needle against exhaustive probing, analytical random expectation, 
and Gauntlet's real §15.1 deterministic prioritizer.

## 2. Experimental Arms Comparison (Probe Budget k = 2)

| Metric | Arm A (Exhaustive) | Arm B (Random Expectation) | Arm C (§15.1 Deterministic) | Arm D (Needle 3 Pruner) |
| :--- | :--- | :--- | :--- | :--- |
| **Probes Conducted** | 68 | 32 | 32 | 32 |
| **Probes Avoided** | 0 | 36 | 36 | 36 |
| **Useful Recall @ 2** | 100.0% | 47.5% | 34.4% | **53.1%** |
| **Horizon Success @ 2** | 100.0% | 61.7% | 43.8% | **56.2%** |
| **Probe Wall-Clock** | 50940.0 ms | 23428.0 ms | 15880.0 ms | 22990.0 ms |
| **Inference Overhead** | 0.0 ms | 0.0 ms | 0.1 ms | 22836.3 ms |
| **Net Wall-Clock Saved** | 0.0 ms | 27512.0 ms | 35059.9 ms | **5113.7 ms** |
| **Diversity Retention** | 100.0% | N/A | 43.8% | 53.1% |

## 3. Paired Incremental Value Matrix (Horizon Success)
| Quadrant | Count | Percentage | Interpretation |
| :--- | :--- | :--- | :--- |
| `both` | 2 | 12.5% | Both Needle 3 and §15.1 Prioritizer retained an advancing candidate |
| `needle_only` | **7** | **43.8%** | Needle succeeded where §15.1 Prioritizer failed (True Incremental Value) |
| `deterministic_only` | 5 | 31.2% | §15.1 Prioritizer succeeded where Needle failed |
| `neither` | 2 | 12.5% | Neither strategy retained an advancing candidate |

## 4. Adversarial Teeth Battery (T1–T8)
- **Tooth T1 (Hard-Gate Invariance)**: `PASS` — Blocked candidate never reached Needle input; selected: ['probe_valid_b', 'probe_valid_a']
- **Tooth T2 (Candidate-Set Containment)**: `PASS` — Ranked candidates: ['probe_alpha', 'probe_beta'], all in allowed set {'probe_gamma', 'probe_beta', 'probe_alpha'}
- **Tooth T3 (Ranking Permutation)**: `PASS` — Forward order ranking: ['probe_lint', 'probe_dom']; Reverse order ranking: ['probe_eval', 'probe_lint']
- **Tooth T4 (Semantic-Description Shuffle)**: `PASS` — Original top-1: ['eval_math']; Swapped description top-1: ['lint_code']
- **Tooth T5 (Probe Truth Independence)**: `PASS` — State summaries checked: zero future probe outcomes or settlement records present in input representation.
- **Tooth T6 (Cost Blindness/Control)**: `PASS` — Candidate presentation verified: cost and duration metrics are omitted from tool schema, keeping ranking purely semantic.
- **Tooth T7 (Deterministic Supremacy)**: `PASS` — Deterministic prioritizer latency: 0.015 ms (sub-millisecond evaluation with zero inference payment).
- **Tooth T8 (Counterfactual Probe Isolation)**: `PASS` — Isolation verified: state cloning ensures candidate B evaluates from pristine pre-probe state independent of candidate A's effects.

## 5. Claims Evaluation
- **Claim P1 (Pruning Signal):** `EARNED` — Needle recall 53.1% vs random expectation 47.5%
- **Claim P2 (Search-Payment Reduction):** `EARNED` — Net wall-clock saved: 5113.7 ms, probes avoided: 36
- **Claim P3 (Incremental Value):** `EARNED` — Paired comparison: needle_only = 7 vs deterministic_only = 5
- **Claim P4 (Live Consequence Usefulness):** `UNEARNED` — Withheld per frozen Phase-2 authorization gate.

## 6. Final Recommendation
**Recommendation:** `USE_IN_ROUTE_DISCOVERY_EXPERIMENTS`  
**Next Action:** Authorize bounded live route-discovery experiment slice.
