"""
Metrics Evaluation for Gauntlet Needle Navigation Experiment.

Computes:
1. Schema-valid proposal rate
2. Invalid-affordance rate
3. Valid-next-move rate (against ground-truth pre-cutoff state)
4. Argument accuracy
5. Responsible and excessive abstention metrics
6. Historical move agreement (DESCRIPTIVE ONLY)
7. Confidence calibration summary
8. Resource payment (latency, peak RAM, TPS)
"""

from __future__ import annotations

from typing import Any, Dict, List, Optional
from .port import NavigationProposal, NavigationRequest


def evaluate_horizon_batch(
    results: List[Dict[str, Any]]
) -> Dict[str, Any]:
    """Evaluates a batch of (request, proposal, ground_truth) evaluation tuples.
    
    Each item in results contains:
    - 'request': NavigationRequest
    - 'proposal': NavigationProposal
    - 'ground_truth': Dict[str, Any] (valid_affordances, historical_next_move, etc.)
    """
    total = len(results)
    if total == 0:
        return {}

    schema_valid_count = 0
    invalid_affordance_count = 0
    valid_next_move_count = 0
    argument_correct_count = 0
    argument_applicable_count = 0

    responsible_abstention_ops = 0
    responsible_abstentions = 0
    actionable_ops = 0
    excessive_abstentions = 0

    historical_agreements = 0

    latencies = []
    confidences_valid = []
    confidences_invalid = []
    confidences_abstained = []
    peak_rams = []
    decode_tps_list = []

    for item in results:
        req: NavigationRequest = item["request"]
        prop: NavigationProposal = item["proposal"]
        gt: Dict[str, Any] = item["ground_truth"]

        latencies.append(prop.provider_provenance.latency_ms)
        if "peak_ram_mb" in prop.provider_provenance.telemetry and prop.provider_provenance.telemetry["peak_ram_mb"]:
            peak_rams.append(float(prop.provider_provenance.telemetry["peak_ram_mb"]))
        if "decode_tps" in prop.provider_provenance.telemetry and prop.provider_provenance.telemetry["decode_tps"]:
            decode_tps_list.append(float(prop.provider_provenance.telemetry["decode_tps"]))

        candidate_names = req.candidate_names()
        valid_moves = gt.get("valid_affordances", [])

        # 1. Schema valid & invalid affordance
        if prop.abstained or (prop.affordance_id is None):
            schema_valid_count += 1
        elif prop.affordance_id in candidate_names:
            schema_valid_count += 1
        else:
            invalid_affordance_count += 1

        # 2. Abstention analysis
        is_abstention_ground_truth = "__ABSTAIN__" in valid_moves
        if is_abstention_ground_truth:
            responsible_abstention_ops += 1
            if prop.abstained or prop.affordance_id is None:
                responsible_abstentions += 1
        else:
            actionable_ops += 1
            if prop.abstained or prop.affordance_id is None:
                excessive_abstentions += 1

        # 3. Valid next move
        is_valid_move = False
        if is_abstention_ground_truth:
            if prop.abstained or prop.affordance_id is None:
                is_valid_move = True
        else:
            if prop.affordance_id in valid_moves:
                is_valid_move = True

        if is_valid_move:
            valid_next_move_count += 1
            if prop.reported_confidence is not None:
                confidences_valid.append(prop.reported_confidence)
        else:
            if prop.reported_confidence is not None:
                confidences_invalid.append(prop.reported_confidence)

        if (prop.abstained or prop.affordance_id is None) and prop.reported_confidence is not None:
            confidences_abstained.append(prop.reported_confidence)

        # 4. Argument accuracy (if applicable)
        target_unit = gt.get("historical_unit")
        if target_unit and not prop.abstained and prop.arguments:
            argument_applicable_count += 1
            if prop.arguments.get("unit_id") == target_unit:
                argument_correct_count += 1

        # 5. Historical agreement (descriptive only!)
        hist_move = gt.get("historical_next_move")
        if hist_move:
            # Check if proposed affordance corresponds to historical action
            if not prop.abstained and prop.affordance_id:
                if prop.arguments.get("unit_id") and target_unit:
                    if prop.arguments.get("unit_id") == target_unit:
                        historical_agreements += 1
                elif prop.affordance_id in valid_moves and target_unit in valid_moves:
                    historical_agreements += 1
            elif prop.abstained and ("none" in hist_move.lower() or "run_terminal" in hist_move.lower()):
                historical_agreements += 1

    return {
        "total_horizons": total,
        "schema_valid_rate": round(schema_valid_count / total, 4),
        "invalid_affordance_rate": round(invalid_affordance_count / total, 4),
        "valid_next_move_rate": round(valid_next_move_count / total, 4),
        "argument_accuracy": round(argument_correct_count / max(argument_applicable_count, 1), 4),
        "abstention_metrics": {
            "responsible_abstention_rate": round(responsible_abstentions / max(responsible_abstention_ops, 1), 4),
            "responsible_abstentions_count": responsible_abstentions,
            "responsible_abstention_opportunities": responsible_abstention_ops,
            "excessive_abstention_rate": round(excessive_abstentions / max(actionable_ops, 1), 4),
            "excessive_abstentions_count": excessive_abstentions,
            "actionable_opportunities": actionable_ops,
        },
        "descriptive_metrics": {
            "historical_move_agreement_rate": round(historical_agreements / total, 4),
            "historical_agreements_count": historical_agreements,
            "note": "Descriptive only; historical moves may be non-optimal or non-unique.",
        },
        "confidence_calibration": {
            "mean_reported_confidence_valid": round(sum(confidences_valid) / max(len(confidences_valid), 1), 4) if confidences_valid else None,
            "mean_reported_confidence_invalid": round(sum(confidences_invalid) / max(len(confidences_invalid), 1), 4) if confidences_invalid else None,
            "mean_reported_confidence_abstained": round(sum(confidences_abstained) / max(len(confidences_abstained), 1), 4) if confidences_abstained else None,
            "total_confidence_samples": len(confidences_valid) + len(confidences_invalid),
        },
        "payment": {
            "mean_latency_ms": round(sum(latencies) / total, 2),
            "min_latency_ms": round(min(latencies), 2),
            "max_latency_ms": round(max(latencies), 2),
            "peak_ram_mb": round(max(peak_rams), 2) if peak_rams else None,
            "mean_decode_tps": round(sum(decode_tps_list) / len(decode_tps_list), 2) if decode_tps_list else None,
        },
    }
