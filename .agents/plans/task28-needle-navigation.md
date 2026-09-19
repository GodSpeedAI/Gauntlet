# Task 28: Needle 3 Navigation Proving Experiment (EXP-N3)

## Objective
Evaluate whether Cactus Needle 3 (a tiny specialized on-device tool-calling model) can navigate a correctly bounded GodSpeed affordance surface cheaply and reliably enough to serve as a System-1 navigator, while Gauntlet and SEA-Forge retain control of execution, authority, evidence, and settlement.

## Architectural Boundaries & Invariants
1. **Proposal-Only Navigation**: Needle only proposes moves via low-level `complete()`. No tool execution permissions or callbacks are granted.
2. **Provider-Neutral Port**: Implemented behind `NavigatorPort` (`harness/needle_nav/port.py`). The core does not know `cactus-needle` or Python object types.
3. **Decoupled Python Harness**: Runs beside Gauntlet in `harness/needle_nav/` with zero modifications to Gauntlet Rust crates or SEA-Forge kernels.
4. **Pre-Scoring Persistence**: Every model response is persisted to `.agents/reports/task28-needle-navigation/observations/` prior to scoring.
5. **Frozen Pre-Cutoff Replay**: Evaluated over 36 horizons reconstructed from 16 real Gauntlet runs (`targets/.runs/`) and test targets, with strict exclusion of post-cutoff evidence.

## Tasks & Phases
- **T28-A: Navigation Port, Needle Adapter & Phase 1 Mechanical Smoke**
  - Implement `port.py`, `needle_adapter.py`, `baselines.py`, and `persistence.py`.
  - Mechanical smoke test: passed on both Needle 3 (35.3MB) and Needle 2 (13.3MB).
- **T28-B: Frozen Historical Replay Experiment & Baselines (Phase 2)**
  - Reconstructed 36 frozen horizons across initial, intermediate, late, terminal, and multi-tier states.
  - Benchmarked Needle 3, Needle 2, Gauntlet §15.1 Lexicographic Deterministic Rule, and Random baseline.
- **T28-C: Adversarial Teeth Battery (Teeth 1–7)**
  - Tooth 1 (Nonexistent Affordance): Passed (clean abstention).
  - Tooth 2 (Hidden Correct Tool): Passed (cannot call unexposed tools).
  - Tooth 3 (Invalid Argument): Passed (typed enum adherence).
  - Tooth 4 (Authority Denial): Passed (gate denies unauthorized moves).
  - Tooth 5 (Confidence Disagreement): Passed (confidence does not bypass gate).
  - Tooth 6 (Future Leakage): Passed (input validator rejects post-cutoff data).
  - Tooth 7 (Semantic Binding & Order Invariance): Passed (order-invariant and tracks semantic descriptions).
- **T28-D: Phase 3 Consequence-Backed Validation & Claims Adjudication**
  - Predeclared Gate: Claim A passed (100% schema valid, 0% invalid affordances, all teeth passed).
  - Claim B gate failed: Valid-next-move rate was 11.1% (threshold >= 70%) due to 97.0% excessive abstention on raw state summaries; Needle's grounding head suppresses calls when argument values are not verbatim present in the prompt string.
  - Phase 3 consequence validation was honestly withheld per predeclared gate.

## Final Recommendation (At Task Settlement)
`RETAIN_AS_EXPERIMENTAL_NAVIGATOR`
- Instrument validity earned (Claim A).
- Live execution slice unearned (Claims B & C unearned).

## Subsequent Arc & Final Terminus
- **Task 28-B (EXP-N3-SPLIT):** Disproved that argument grounding was the sole failure locus; selection-only validity reached only 22.2%, trailing §15.1 deterministic rule (100.0%).
- **Task 28-C (EXP-N3-PRUNE) & Audit:** Re-evaluation of candidate pruning revealed Tooth T3 substantive order-sensitivity failure, lack of statistical separation from random, and economic dominance by cheap alternatives.
- **Final Disposition:** **`REMOVE_NEEDLE`**. The Needle navigation experimental branch is closed and decommissioned. All runtime integration removed. Evidence preserved.
