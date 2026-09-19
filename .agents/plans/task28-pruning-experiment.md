# Task 28-C: Candidate-Pruning and Search-Efficiency Proving Experiment (EXP-N3-PRUNE)

## Objective
Evaluate whether Cactus Needle 3 can serve as a cheap candidate-pruning and prioritization instrument when deterministic hard gates (visibility, reachability, payment, governance, prerequisites) leave multiple legitimate affordances unresolved ($N \ge 3$) and probing every candidate has meaningful payment.

## Core Distinction
- Settled: Needle cannot replace Gauntlet §15.1 deterministic navigation (where deterministic navigation achieves 100% validity).
- Under Test: Can Needle reduce the candidate set ($k < N$) while preserving the affordances that consequence later shows were useful, saving net search payment over exhaustive probing, random selection, and cheap deterministic heuristics?

## Pre-Execution Freeze Matrix
- **Corpus**: 16 ambiguous route-search horizons across 3 representative families:
  1. Diagnostic / Verification Probes (5 horizons)
  2. Remediation / Builder Strategy Probes (5 horizons)
  3. Native Consequence-Guided Route Discovery (6 horizons, T21 catalog)
- **Probe Budget**: Frozen at $k = 2$ ($N \in [4, 5]$).
- **Consequence Truth**:
  - `ADVANCES`: Produces necessary verification evidence or state transition required for route settlement.
  - `REDUNDANT`: Valid execution but produces no new state/evidence.
  - `DEAD_END`: Valid execution that fails to advance the route.
  - `COSTLY_ADVANCE`: Advances but with materially higher payment.
- **Economic Formula**:
  $$\text{Net Payment Saved} = \text{Exhaustive Probe Cost} - (\text{Needle Inference Overhead} + \text{Top-}k\text{ Probe Cost})$$
- **Phase-2 Live Route-Discovery Authorization Gate**:
  - `useful_recall@k > Random useful_recall@k` (Claim P1: Pruning Signal)
  - `Net Payment Saved > 0` (Claim P2: Search-Payment Reduction)
  - `needle_only > 0` (Claim P3: Incremental Value)
  - Adversarial Teeth T1–T7 Pass (100%)

## Experimental Arms
- **Arm A (Exhaustive Reference)**: Probe all $N$ candidates. (Maximal discovery, maximal payment).
- **Arm B (Random Top-K)**: Uniform random sample of $k=2$ candidates.
- **Arm C (Deterministic Heuristic Top-K)**: Static / registration order top-$k=2$.
- **Arm D (Needle Top-K)**: Needle ranks candidates, probes top-$k=2$.

## Adversarial Teeth (T1–T7)
- Tooth T1: Hard-Gate Invariance (ineligible candidates never reach Needle).
- Tooth T2: Candidate-Set Containment (cannot select unexposed tools).
- Tooth T3: Ranking Permutation (presentation order does not dictate ranking).
- Tooth T4: Semantic-Description Shuffle (ranking degrades/changes when descriptions are swapped).
- Tooth T5: Probe Truth Independence (no future probe outcomes in input).
- Tooth T6: Cost Blindness/Control (Needle is cost-blind unless payment is explicitly represented).
- Tooth T7: Deterministic Supremacy (if cheap deterministic heuristic achieves equal recall at lower cost, Needle does not earn a role).

## Decision Rule & Final Disposition
- **Status:** `CLOSED (Settled)`
- **Execution Date:** 2026-09-19
- **Audit Date:** 2026-09-19
- **Final Disposition:** **`REMOVE_NEEDLE`**

### Chronology of Evidence:
1. **Mechanical Execution:**
   - Completed across 16 horizons.
   - Initial report recorded: P1 earned (+5.6% recall over random), P2 earned (+5,113.7 ms net saved), P3 earned (needle_only 7 > det_only 5), all teeth passed.
   - Original Gate mechanically fired: `AUTHORIZED`.
2. **Post-Execution Scientific Audit:**
   - **Tooth T3 FAILED:** The test runner disjunctive assertion (`or`) masked a substantive failure: Needle dropped the advancing probe in forward order and selected it in reverse order.
   - **Claim P1 INCONCLUSIVE:** Recall advantage was finite-sample noise ($Z=+0.53, p=0.3503$); on operational horizon success, Needle lost to random expectation (56.25% vs 61.67%).
   - **Claim P3 UNPROVEN:** Discordant difference (7 vs 5) has exact binomial $p = 0.3872$ (95% CI $[-29.5\%, +54.5\%]$).
   - **Economic Dominance:** Random expectation strictly Pareto-dominates Needle on operational pruning (higher success, 48.9% less wall-clock, 2.14x lower cost per success).
   - **Gate Outcome:** Original authorization **REVOKED**.
3. **Disposition:**
   - Needle is completely removed from active integration and runtime paths.
   - All experimental code in `harness/needle_nav/` is archived.
   - All reports and observations in `.agents/reports/task28-pruning-experiment/` are preserved as immutable developmental memory.
   - Native consequence-guided route discovery proceeds in Gauntlet without Needle.
