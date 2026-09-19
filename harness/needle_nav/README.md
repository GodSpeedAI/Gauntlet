# Archived Research Harness: Needle Navigation & Candidate Pruning (EXP-N3)

**STATUS:** `ARCHIVED / DECOMMISSIONED`  
**FINAL EVIDENCE-EARNED DISPOSITION:** `REMOVE_NEEDLE`  
**DATE:** 2026-09-19

## Purpose
This directory contains the frozen, decoupled Python research harness used to conduct the Task 28 proving experiments in Gauntlet:
1. **EXP-N3** (Coupled Affordance Selection + Argument Grounding)
2. **EXP-N3-SPLIT** (Decomposition into Selection-Only + Deterministic Rule Binding)
3. **EXP-N3-PRUNE** (Candidate Pruning and Search Efficiency over Unresolved Horizons)

## Experimental Conclusions
Across all three experimental configurations:
- **Coupled Contract (EXP-N3):** 100% schema validity, but 97.0% excessive abstention on raw state summaries due to strict parameter-grounding heads (11.1% valid-next-move rate).
- **Split Decomposition (EXP-N3-SPLIT):** Removing argument obligations eliminated excessive abstention (0.0%), but affordance selection validity reached only 22.2% (8/36), trailing uniform random (27.8%) and Gauntlet §15.1 deterministic rule (100.0%).
- **Candidate Pruning (EXP-N3-PRUNE):** Re-evaluation of candidate pruning under exact probabilistic standards showed:
  - Tooth T3 (Ranking Permutation) suffered a substantive order-sensitivity failure (inverted candidate selection when presentation order was reversed), which was masked in the runner by an erroneous disjunctive assertion.
  - Useful recall advantage (+5.6%) is within ordinary finite-sample random variation ($Z = +0.53, p = 0.3503$).
  - Needle achieved 56.25% operational horizon success, losing to exact analytical random expectation (61.67%).
  - Paired discordance over §15.1 deterministic prioritizer (7 vs 5) is not statistically significant ($p = 0.3872$).
  - Random expectation strictly Pareto-dominates Needle on operational success and total payment (higher success, 48.9% less wall-clock, zero latency overhead, 2.14x lower cost per success).

## Final Disposition: REMOVE_NEEDLE
- Phase-2 live route-discovery authorization was **REVOKED**.
- No Needle-based live navigation slice is authorized.
- No durable navigator promotion is authorized.
- Needle is **not** part of any normal operational or runtime path in Gauntlet.
- Gauntlet's native deterministic and consequence-guided route discovery remains authoritative.

## Preserved Evidence & Reports
- [EXP-N3 Coupled Report](../../.agents/reports/task28-needle-navigation/report.md)
- [EXP-N3-SPLIT Decomposition Report](../../.agents/reports/task28-split-navigation/report.md)
- [EXP-N3-PRUNE Pruning Report](../../.agents/reports/task28-pruning-experiment/report.md)
- [EXP-N3-PRUNE Post-Execution Scientific Audit](../../.agents/reports/task28-pruning-experiment/audit_report.md)

*This code is preserved strictly for research auditability, provenance, and historical reproducibility.*
