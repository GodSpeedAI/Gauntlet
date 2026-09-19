# EXP-N3-PRUNE: Post-Execution Scientific Audit & Correction Report

**Date:** 2026-09-19T12:25:00Z  
**Audit Target:** EXP-N3-PRUNE ([report.json](file:///home/sprime01/projects/gauntlet/.agents/reports/task28-pruning-experiment/report.json))  
**Auditor:** Gauntlet Cognitive Navigator / Scientific Verifier  
**Governing Action:** Halt Phase 2; Re-evaluate all claims under exact probabilistic and economic standards; Reconcile frozen gate vs scientific evidence.

---

## 1. Audit Finding 1: Tooth T3 Substantive Order-Sensitivity Failure

### Flaw Locus
Tooth T3 was designed to enforce that:
> Needle's substantive candidate preference cannot be dictated by arbitrary candidate presentation order.

However, the verification assertion in `harness/needle_nav/pruning_teeth.py` used a disjunctive condition:
```python
passed = "probe_eval" in rank_fwd.ranked_candidate_ids or "probe_eval" in rank_rev.ranked_candidate_ids
```
This allowed the test to report `PASS` if the model selected `probe_eval` in *either* permutation.

### Observed Evidence
When presented with the identical candidate set in forward order vs reverse order:
- **Forward Order Input:** `[c1: probe_eval (ADVANCES), c2: probe_lint (DEAD_END), c3: probe_dom (DEAD_END)]`
  - **Needle Selected Top-2:** `['probe_lint', 'probe_dom']`
  - **Outcome:** Needle **omitted/pruned** `probe_eval` (the sole advancing candidate).
- **Reverse Order Input:** `[c3: probe_dom (DEAD_END), c2: probe_lint (DEAD_END), c1: probe_eval (ADVANCES)]`
  - **Needle Selected Top-2:** `['probe_eval', 'probe_lint']`
  - **Outcome:** Needle **retained** `probe_eval`.

### Substantive Verdict
The selected candidate set was `{probe_lint, probe_dom}` in forward order and `{probe_eval, probe_lint}` in reverse order.
Changing *only* candidate presentation order reversed the substantive selection of the advancing candidate.
- **Original Classification:** `PASS` (flawed disjunctive assertion).
- **Audited Classification:** **`FAIL` (Substantive Order Sensitivity)**.
- **Gate Consequence:** Under the frozen experimental contract, failure of any adversarial tooth **automatically revokes Phase-2 authorization**.

---

## 2. Audit Finding 2: Reassessment of Random Comparison

The primary operational question for a candidate pruner is:
> Does top-$k$ preserve at least one consequence-advancing continuation?

Therefore, `horizon_success@k` is the decisive operational metric. Aggregate `useful_recall@k` measures candidate volume.

### Empirical Breakdown

| Metric | Needle 3 Pruner | Exact Random Expectation | Difference | Std Dev ($\sigma$) | $Z$-Score | Exact $P(\text{Random} \ge \text{Needle})$ |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **Useful Recall @ 2** | **53.12%** | 47.50% | +5.62% | 10.51% | +0.53 | **35.03% ($p = 0.3503$)** |
| **Horizon Success @ 2** | **56.25%** | **61.67%** (reported) / 49.58% (advances) | -5.42% / +6.67% | 12.26% | -0.44 / +0.54 | **38.60% ($p = 0.3860$)** |

### Probabilistic Interpretation
1. **Recall Advantage is Finite-Sample Noise:**
   The exact convolution of all $\binom{N_i}{2}$ combinations across the 16 horizons yields an expected random recall of $47.50\% \pm 10.51\%$. Needle's observed recall of $53.12\%$ sits at the **65th percentile** ($Z = +0.53$). The probability of an unguided random selection achieving $\ge 53.12\%$ recall is **35.03%**. In more than 1 out of 3 random runs, random selection matches or beats Needle.
2. **Needle Loses to Random on Operational Success:**
   On the reported primary operational metric (`horizon_success@2`), Needle achieved 56.25% (9/16 horizons), while the exact expectation of random selection is 61.67% (9.87/16 horizons). Needle actively preserved fewer successful continuations than random expectation.
3. **Claim P1 Status:** **NOT EARNED / INCONCLUSIVE**.

---

## 3. Audit Finding 3: Paired Statistical Comparison vs §15.1 Prioritizer

The unadjusted horizon comparison yielded:
- `both = 2`
- `needle_only = 7`
- `deterministic_only = 5`
- `neither = 2`
- **Total Discordant Horizons ($N_{\text{disc}}$):** 12

### Exact Paired Statistical Test (McNemar / Exact Binomial)
Testing the null hypothesis $H_0: p = 0.5$ (Needle and §15.1 Deterministic Prioritizer are equally likely to win discordant horizons):
- Under $H_0$, $X \sim \text{Binomial}(12, 0.5)$.
- $P(X \ge 7) = \sum_{x=7}^{12} \binom{12}{x} (0.5)^{12} = \frac{1586}{4096} = \mathbf{0.3872}$.
- Two-sided $p$-value: $\mathbf{0.7744}$.
- Observed paired difference in success rate: $+12.5\%$ (Standard Error: $21.42\%$).
- **95% Confidence Interval:** $[-29.48\%, +54.48\%]$.

### Verdict
The count comparison $7 > 5$ is numerically favorable but statistically indistinguishable from a coin flip ($p = 0.3872$). The 95% confidence interval spans widely across zero (from $-29.5\%$ to $+54.5\%$).
- **Claim P3 Status:** **WEAK / NOT EARNED**. The result mechanically satisfied the uncalibrated gate (`needle_only > det_only`), but provides zero rigorous evidence of repeatable incremental capability.

---

## 4. Audit Finding 4: Economic Value per Success & Pareto Dominance

### Strategy Cost Ledger

| Strategy | Total Probes | Probe Wall-Clock | Strategy Latency | Total Strategy Wall-Clock | Successful Horizons | Wall-Clock per Success | Probes per Success |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **Exhaustive (Arm A)** | 68 | 50,940.0 ms | 0.0 ms | 50,940.0 ms | 16.0 (100%) | **3,183.8 ms** | 4.25 |
| **Random Expectation (Arm B)** | 32 | 23,428.0 ms | 0.0 ms | 23,428.0 ms | 9.87 (61.7%) | **2,373.7 ms** | **3.24** |
| **§15.1 Deterministic (Arm C)** | 32 | 15,880.0 ms | 0.1 ms | 15,880.1 ms | 7.0 (43.8%) | **2,268.6 ms** | 4.57 |
| **Needle 3 Pruner (Arm D)** | 32 | 22,990.0 ms | 22,836.3 ms | 45,826.3 ms | 9.0 (56.2%) | **5,091.8 ms** | 3.56 |

### Incremental Economic Analysis
1. **Needle vs Random Expectation:**
   - Incremental Successes: $9.0 - 9.87 = \mathbf{-0.87}$ horizons (Needle delivers *fewer* successes).
   - Incremental Wall-Clock: $45,826.3 - 23,428.0 = \mathbf{+22,398.3\text{ ms}}$.
   - **Finding:** Needle pays 22.4 seconds of inference latency to produce **worse operational success** than pure chance.
2. **Needle vs §15.1 Deterministic Prioritizer:**
   - Incremental Successes: $9.0 - 7.0 = \mathbf{+2.0}$ horizons.
   - Incremental Wall-Clock: $45,826.3 - 15,880.1 = \mathbf{+29,946.2\text{ ms}}$.
   - **Cost per Additional Success:** $\mathbf{14,973.1\text{ ms}}$ per incremental success.
   - **Finding:** Exhaustive probing achieves 100% success at 3,183.8 ms/success. Needle's marginal cost to rescue two horizons is **4.7x higher** than simply probing exhaustively.

### Pareto Dominance Analysis
- **Random strictly dominates Needle on operational pruning:**
  - Random has **higher horizon success** (61.7% vs 56.2%).
  - Random has **48.9% lower total wall-clock** (23,428 ms vs 45,826 ms).
  - Random has **zero inference overhead** (0 ms vs 22,836 ms).
  - Random has **lower cost per success** (2,374 ms vs 5,092 ms).
- **Needle occupies no non-dominated operational point:**
  - If the objective is maximum success: Exhaustive dominates Needle (100% success at lower cost per success).
  - If the objective is lowest payment: Deterministic dominates Needle (15,880 ms vs 45,826 ms).
  - If the objective is cheap probabilistic pruning: Random dominates Needle (higher success, lower cost).

---

## 5. Audit Finding 5: Re-evaluation of Claim P2 (Search-Payment Reduction)

There are two distinct definitions of Search-Payment Reduction:
- **Meaning A (Relative to Exhaustive Probing):** Did Needle reduce wall-clock relative to unpruned search?
  - Yes: $50,940\text{ ms} - 45,826\text{ ms} = +5,113.7\text{ ms}$ saved.
  - Needle mechanically satisfies Meaning A.
- **Meaning B (Relative to Available Cheaper Pruning Alternatives):** Is Needle economically justifiable against existing cheap baselines?
  - **No.** Needle costs 5,091.8 ms per successful horizon—worse than Exhaustive (3,183.8 ms), worse than Random (2,373.7 ms), and worse than Deterministic (2,268.6 ms).
  - It is dominated by Random and yields an exorbitant marginal cost ($14,973$ ms) relative to Deterministic.
- **Verdict:** Claim P2 is **PARTIALLY EARNED under Meaning A**, but **FAILS under Meaning B**. Architectural promotion requires Meaning B.

---

## 6. Authorization Reconciliation

- **`ORIGINAL_FROZEN_GATE_RESULT`:** `PASS` (Mechanically fired due to developmental defects in the gate).
- **`EVIDENCE_AUDIT_RESULT`:** **`NOT_JUSTIFIED / REVOKED`**.

### Developmental Defects in the Original Gate
1. **Tooth T3 Assertion Bug:** Allowed disjunctive passage (`or`), masking a severe substantive order-sensitivity defect.
2. **Unbalanced P1 Metric:** Measured aggregate `useful_recall` without checking whether `horizon_success` beat random expectation.
3. **Uncalibrated P3 Condition:** Used an uncalibrated count comparison ($7 > 5$) without requiring statistical significance on discordant pairs.

---

## 7. Final Decision & Action

Per the user's explicit decision rule:
> Use **`REMOVE_NEEDLE`** if:
> - T3 substantively fails;
> - Needle does not clearly beat random;
> - and Needle is economically dominated by cheap alternatives.

All three conditions are confirmed by the audited evidence:
1. **Tooth T3 substantively failed** (presentation order reversal inverted selection).
2. **Needle does not clearly beat random** ($p = 0.35$ on recall; loses to random on horizon success).
3. **Needle is economically dominated by cheaper alternatives** (Random achieves higher success at 49% less wall-clock; Needle costs 2.14x more per success).

### Final Disposition: **`REMOVE_NEEDLE`**

**Next Actions:**
1. Roll back the Needle integration and runtime dependencies from Gauntlet.
2. Retain all three experimental evidence bundles ([EXP-N3](file:///home/sprime01/projects/gauntlet/.agents/reports/task28-needle-navigation/), [EXP-N3-SPLIT](file:///home/sprime01/projects/gauntlet/.agents/reports/task28-split-navigation/), [EXP-N3-PRUNE](file:///home/sprime01/projects/gauntlet/.agents/reports/task28-pruning-experiment/)) as permanent developmental memory.
3. Update `.agents/CURRENT_STATUS.yml` with the audited final settlement.
4. Proceed with native consequence-guided route discovery in Gauntlet without Needle.
