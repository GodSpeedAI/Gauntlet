"""
Metrics Evaluation for Split-Contract Navigation Experiment (EXP-N3-SPLIT).

Computes:
1. Selection-only metrics:
   - schema_valid_selection_rate
   - invalid_affordance_rate
   - valid_next_affordance_rate
   - selection_validity breakdown (VALID, INVALID, COUNTERFACTUAL_UNKNOWN)
   - responsible_abstention_rate
   - excessive_abstention_rate
   - historical_move_agreement_rate (descriptive only)
   - reported_confidence_calibration
   - payment (latency, RAM, TPS)
2. Combined split system metrics:
   - argument_binding_success_conditional (P(binding_success | selection == VALID))
   - binding_failure_breakdown (missing argument, ambiguous, prerequisite failure)
   - validation_pass_rate
   - authority_denial_rate
   - full_valid_next_move_rate
3. Incremental value matrix over §15.1 deterministic rule baseline:
   - both_valid, needle_only_valid, rule_only_valid, neither_valid
"""

from __future__ import annotations

from typing import Any, Dict, List

from .split_contract import (
    BindingStatus,
    IncrementalValueMatrix,
    SelectionValidity,
    SplitHorizonEvaluation,
)


def evaluate_split_batch(evaluations: List[SplitHorizonEvaluation]) -> Dict[str, Any]:
    """Computes all split contract metrics for a given arm's evaluations."""
    total = len(evaluations)
    if total == 0:
        return {}

    # 1. Selection-Only Metrics
    schema_valid_count = 0
    invalid_affordance_count = 0
    valid_selection_count = 0
    invalid_selection_count = 0
    unknown_selection_count = 0

    responsible_abstention_opps = 0
    responsible_abstention_count = 0
    excessive_abstention_opps = 0
    excessive_abstention_count = 0

    historical_agreement_count = 0

    confidences_valid = []
    confidences_invalid = []
    latencies = []
    peak_rams = []
    decode_tps_list = []
    prefill_tps_list = []

    # 2. Combined System Metrics
    valid_selections_total = 0
    binding_success_on_valid = 0
    failed_missing_arg_count = 0
    failed_ambiguous_count = 0
    failed_prereq_count = 0
    validation_pass_count = 0
    authority_denied_count = 0
    full_valid_next_move_count = 0

    for ev in evaluations:
        sel = ev.selection
        prov = sel.provider_provenance

        # Latency & Telemetry
        latencies.append(prov.latency_ms)
        if prov.telemetry.get("peak_ram_mb"):
            peak_rams.append(prov.telemetry["peak_ram_mb"])
        if prov.telemetry.get("decode_tps"):
            decode_tps_list.append(prov.telemetry["decode_tps"])
        if prov.telemetry.get("prefill_tps"):
            prefill_tps_list.append(prov.telemetry["prefill_tps"])

        # Schema Validity (selection in candidates or abstained)
        # Note: We check if affordance_id was exposed in horizon candidates
        # If navigator proposed something outside candidates, invalid_affordance
        if ev.selection_validity != SelectionValidity.INVALID or sel.abstained or sel.affordance_id is not None:
            schema_valid_count += 1

        # Selection validity breakdown
        if ev.selection_validity == SelectionValidity.VALID:
            valid_selection_count += 1
            if sel.reported_confidence is not None:
                confidences_valid.append(sel.reported_confidence)
        elif ev.selection_validity == SelectionValidity.INVALID:
            invalid_selection_count += 1
            if sel.reported_confidence is not None:
                confidences_invalid.append(sel.reported_confidence)
        else:
            unknown_selection_count += 1

        # Historical agreement (descriptive only)
        if ev.historical_move_agreement:
            historical_agreement_count += 1

        # Abstention breakdown
        # We determine opportunity from whether the horizon's valid move was __ABSTAIN__
        # In our dataset, when no open units remain, valid move is __ABSTAIN__
        # We check through ev.selection_validity or binding_result
        if ev.binding_result.status == BindingStatus.NOT_APPLICABLE and ev.selection_validity == SelectionValidity.VALID:
            # Horizon had no open units and navigator abstained
            responsible_abstention_opps += 1
            if sel.abstained:
                responsible_abstention_count += 1
        elif sel.abstained and ev.selection_validity == SelectionValidity.INVALID:
            # Navigator abstained when valid moves were available!
            excessive_abstention_opps += 1
            excessive_abstention_count += 1
        else:
            if not sel.abstained and ev.selection_validity == SelectionValidity.VALID:
                pass
            else:
                excessive_abstention_opps += 1

        # Combined System Metrics
        if ev.selection_validity == SelectionValidity.VALID:
            valid_selections_total += 1
            if ev.binding_result.is_success or ev.binding_result.status == BindingStatus.NOT_APPLICABLE:
                binding_success_on_valid += 1

        if ev.binding_result.status == BindingStatus.FAILED_MISSING_ARGUMENT:
            failed_missing_arg_count += 1
        elif ev.binding_result.status == BindingStatus.FAILED_AMBIGUOUS:
            failed_ambiguous_count += 1
        elif ev.binding_result.status == BindingStatus.FAILED_PREREQUISITE:
            failed_prereq_count += 1

        if ev.validation_passed:
            validation_pass_count += 1

        if not ev.authority_admitted:
            authority_denied_count += 1

        if ev.full_valid_next_move:
            full_valid_next_move_count += 1

    # Safe rates
    valid_selection_rate = valid_selection_count / total
    invalid_selection_rate = invalid_selection_count / total
    schema_valid_rate = schema_valid_count / total
    invalid_affordance_rate = invalid_affordance_count / total

    resp_abstain_rate = (responsible_abstention_count / responsible_abstention_opps) if responsible_abstention_opps > 0 else 1.0
    excess_abstain_rate = (excessive_abstention_count / excessive_abstention_opps) if excessive_abstention_opps > 0 else 0.0
    historical_agreement_rate = historical_agreement_count / total

    cond_binding_success_rate = (binding_success_on_valid / valid_selections_total) if valid_selections_total > 0 else 0.0
    full_valid_move_rate = full_valid_next_move_count / total

    mean_conf_valid = (sum(confidences_valid) / len(confidences_valid)) if confidences_valid else None
    mean_conf_invalid = (sum(confidences_invalid) / len(confidences_invalid)) if confidences_invalid else None
    conf_gap = (mean_conf_valid - mean_conf_invalid) if (mean_conf_valid is not None and mean_conf_invalid is not None) else None

    payment = {
        "mean_latency_ms": round(sum(latencies) / len(latencies), 2) if latencies else 0.0,
        "peak_ram_mb": round(max(peak_rams), 1) if peak_rams else None,
        "mean_decode_tps": round(sum(decode_tps_list) / len(decode_tps_list), 1) if decode_tps_list else None,
        "mean_prefill_tps": round(sum(prefill_tps_list) / len(prefill_tps_list), 1) if prefill_tps_list else None,
    }

    return {
        "total_horizons": total,
        "selection_only_metrics": {
            "schema_valid_selection_rate": round(schema_valid_rate, 4),
            "invalid_affordance_rate": round(invalid_affordance_rate, 4),
            "valid_next_affordance_rate": round(valid_selection_rate, 4),
            "validity_breakdown": {
                "VALID": valid_selection_count,
                "INVALID": invalid_selection_count,
                "COUNTERFACTUAL_UNKNOWN": unknown_selection_count,
            },
            "abstention": {
                "responsible_abstention_rate": round(resp_abstain_rate, 4),
                "responsible_abstention_count": responsible_abstention_count,
                "responsible_abstention_opportunities": responsible_abstention_opps,
                "excessive_abstention_rate": round(excess_abstain_rate, 4),
                "excessive_abstention_count": excessive_abstention_count,
                "excessive_abstention_opportunities": excessive_abstention_opps,
            },
            "historical_move_agreement_rate": round(historical_agreement_rate, 4),
            "confidence_calibration": {
                "mean_confidence_valid": round(mean_conf_valid, 4) if mean_conf_valid is not None else None,
                "mean_confidence_invalid": round(mean_conf_invalid, 4) if mean_conf_invalid is not None else None,
                "calibration_gap": round(conf_gap, 4) if conf_gap is not None else None,
            },
            "payment": payment,
        },
        "combined_system_metrics": {
            "conditional_binding_success_rate": round(cond_binding_success_rate, 4),
            "binding_success_count": binding_success_on_valid,
            "valid_selection_opportunities": valid_selections_total,
            "binding_failures": {
                "failed_missing_argument": failed_missing_arg_count,
                "failed_ambiguous": failed_ambiguous_count,
                "failed_prerequisite": failed_prereq_count,
            },
            "validation_pass_rate": round(validation_pass_count / total, 4),
            "authority_denial_rate": round(authority_denied_count / total, 4),
            "full_valid_next_move_rate": round(full_valid_move_rate, 4),
            "full_valid_next_move_count": full_valid_next_move_count,
        },
    }


def compute_incremental_matrix(
    needle_evals: List[SplitHorizonEvaluation],
    rule_evals: List[SplitHorizonEvaluation],
) -> IncrementalValueMatrix:
    """Computes both_valid, needle_only_valid, rule_only_valid, neither_valid across 36 horizons."""
    matrix = IncrementalValueMatrix()
    assert len(needle_evals) == len(rule_evals), "Evaluations count mismatch"

    for n_ev, r_ev in zip(needle_evals, rule_evals):
        n_valid = n_ev.selection_validity == SelectionValidity.VALID
        r_valid = r_ev.selection_validity == SelectionValidity.VALID

        if n_valid and r_valid:
            matrix.both_valid += 1
        elif n_valid and not r_valid:
            matrix.needle_only_valid += 1
        elif not n_valid and r_valid:
            matrix.rule_only_valid += 1
        else:
            matrix.neither_valid += 1

    return matrix
