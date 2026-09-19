---
type: Lesson
title: Model footprint is not economic payment; total payment per successful consequential outcome governs learned navigation
description: For bounded affordance navigation, tiny model parameter size does not imply economic efficiency; total payment per successful consequential outcome governs, and deterministic/consequence-guided mechanisms dominate where available.
resource: .agents/reports/task28-pruning-experiment/audit_report.md
tags: [navigation, needle, economics, route-discovery, affordance-selection]
scope: repo-wide
last_modified: 2026-09-19
---

# 1. Context
Across three formal proving experiments (Task 28 EXP-N3, EXP-N3-SPLIT, EXP-N3-PRUNE), Cactus Needle 3 (a 35.3 MB parameterless tool selector) was evaluated as a System-1 navigator and candidate pruner in Gauntlet. The post-execution scientific audit revealed that despite low memory footprint and strict schema compliance, Needle failed to provide meaningful navigation usefulness or search-payment reduction over deterministic and random baselines.

# 2. The lesson
For the tested Gauntlet affordance surfaces, tiny learned navigation did not earn its payment. Deterministic and consequence-guided mechanisms remain preferable where available. Learned models may still participate in candidate generation or semantic search when that role independently earns its cost, but they do not acquire authority, settlement, or route-retention responsibility from tool-selection fluency alone.

Model footprint is not the relevant economic unit. Total payment per successful consequential outcome is. Do not universalize this into "tiny models are useless" or "learned navigation can never work"; the claim is strictly bounded to the tested Needle/Gauntlet affordance surfaces and evidence.

# 3. Why it matters
A small model (e.g. 35 MB) can produce an illusion of cheapness while running up significant inference latency (1.4s per horizon) that overwhelms the execution payment of the probes it was meant to prune. In EXP-N3-PRUNE, Needle cost 5,091.8 ms per successful horizon—worse than exhaustive probing (3,183.8 ms) and more than double deterministic (2,268.6 ms) or random expectation (2,373.7 ms). Furthermore, learned tool selection exhibited severe substantive order sensitivity (Tooth T3 FAIL), dropping viable candidates upon presentation permutation.

# 4. Scope
Applies to:
- Gauntlet System-1 navigation, affordance selection, and candidate pruning.
- Evaluations of learned tool-calling models for agent action selection where deterministic hard gates and consequence probes exist.

Does NOT apply to:
- Unbounded semantic search or broad candidate generation where deterministic rules cannot define candidate sets.
- Offline model evaluation where execution wall-clock payment is irrelevant.

# 5. Evidence
1. **EXP-N3 Coupled Report:** [report.md](file:///home/sprime01/projects/gauntlet/.agents/reports/task28-needle-navigation/report.md) — 97.0% excessive abstention on raw state summaries.
2. **EXP-N3-SPLIT Report:** [report.md](file:///home/sprime01/projects/gauntlet/.agents/reports/task28-split-navigation/report.md) — Selection validity 22.2% vs §15.1 deterministic rule 100.0% (`needle_only_valid = 0`).
3. **EXP-N3-PRUNE Post-Execution Scientific Audit:** [audit_report.md](file:///home/sprime01/projects/gauntlet/.agents/reports/task28-pruning-experiment/audit_report.md) — Tooth T3 substantive order-sensitivity fail; random expectation strictly Pareto-dominates Needle (higher horizon success at 48.9% lower wall-clock); final disposition `REMOVE_NEEDLE`.
