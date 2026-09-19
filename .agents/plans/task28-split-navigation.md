# Task 28-B: Split-Contract Navigation Experiment (EXP-N3-SPLIT)

## Objective
Determine whether the failure locus of Needle 3 navigation on raw Gauntlet state summaries (Task 28 EXP-N3: 11.1% valid-next-move, 97.0% excessive abstention) is:
- **Locus A**: Affordance selection failure, or
- **Locus B**: Argument grounding/binding failure.

The experiment tests a decoupled interface where Needle 3 performs **affordance selection only** (`affordance_id`), while a neutral deterministic rule-based binder resolves typed arguments from represented pre-cutoff state.

## Pre-Execution Freeze Matrix
- **Corpus**: 36 frozen horizons from `dataset_manifest.json` (SHA-256: `0de55f35552f1736e64ec49bbb2f8bed1f4c21a8f952f020d84f6ce762494760`).
- **Model**: Cactus Needle 3 (`v3-20L`, 35.3 MB weights), reset via `agent.reset()` before each horizon evaluation.
- **Candidate Surface**: Parameterless tool schemas (`parameters: {"type": "object", "properties": {}}`). Descriptions provide generic stable semantics, applicability/preconditions, expected effects, and constraints without case identifiers or imperative phrasing. Description hash recorded.
- **Binder Neutrality Invariant**: Resolves arguments *only* for the provided `affordance_id`; cannot compare, rank, or substitute affordances.
- **Metric Separation**:
  - `selection_validity`: `VALID`, `INVALID`, `COUNTERFACTUAL_UNKNOWN`.
  - `historical_move_agreement`: purely descriptive.
  - `binding_success`, `failed_missing_argument`, `failed_ambiguous`.
  - Incremental value matrix: `both_valid`, `needle_only_valid`, `rule_only_valid`, `neither_valid`.
- **Phase-3 Authorization Gate**:
  - Needle Selection Validity >= 70%
  - Invalid Affordance Rate = 0%
  - Adversarial Teeth T1–T7 Pass
  - Candidate moves for Phase 3 must be deterministically bindable (`binding_success == True`).
- **Allowed Claims**:
  - Claim A: Instrument validity (earned if schema-valid rate == 100%, invalid-affordance == 0%, teeth pass).
  - Claim B: Navigation usefulness (earned if selection validity >= 70% and margin over random >= 25%).
  - Claim C: Consequence usefulness (withheld unless Phase 3 is authorized and live execution settles).

## Experimental Arms
- **Arm A (Coupled Baseline)**: Settled EXP-N3 results (valid-next-move: 11.1%, excessive abstention: 97.0%).
- **Arm B (Primary Arm)**: Selection-only Needle 3 + Neutral Deterministic Argument Binder.
- **Arm C (Deterministic Baseline)**: Gauntlet §15.1 Lexicographic Selector over identical surface.
- **Arm D (Random Baseline)**: Uniform Random Selector over identical surface + Neutral Deterministic Argument Binder.

## Adversarial Teeth (T1–T7)
- Tooth T1: Nonexistent Affordance (abstains cleanly).
- Tooth T2: Hidden Correct Tool (cannot select unexposed tool).
- Tooth T3: Affordance Immutability (binder cannot change selected affordance).
- Tooth T4: No Fabrication (binder fails typed rather than inventing values).
- Tooth T5: Attribution Preservation (correct selection stays VALID regardless of binding failure; incorrect cannot be rescued).
- Tooth T6: Temporal Cutoff Integrity (no post-cutoff leakage).
- Tooth T7: Binder Neutrality (binder cannot act as hidden navigator/ranker).

## Discriminating Outcomes (O1–O4)
- **Outcome O1**: Selection Strong (>= 70%) -> Locus B (Argument Grounding failure).
- **Outcome O2**: Selection Weak / Excessive Abstention (< 70%) -> Locus A (Affordance Selection failure).
- **Outcome O3**: Representation Enrichment Candidate (Weak on raw summary, systematic pattern observed).
- **Outcome O4**: Deterministic Rule Superior (§15.1 superior, needle_only_valid == 0).

## Experimental Result & Final Terminus
- **Observed Outcome:** **Outcome O2 (Selection Weak: 22.2%) + Outcome O4 (Deterministic Rule Superior: 100.0%, `needle_only_valid = 0`)**.
- **Locus:** Locus A (Affordance Selection Failure).
- **Subsequent Arc:** Task 28-C (EXP-N3-PRUNE) tested the remaining candidate-pruning role; post-execution audit revealed Tooth T3 order-sensitivity failure, lack of statistical separation from random, and economic dominance by cheap baselines.
- **Final Disposition:** **`REMOVE_NEEDLE`**. The Needle navigation experimental branch is closed and decommissioned. All runtime integration removed. Evidence preserved.
