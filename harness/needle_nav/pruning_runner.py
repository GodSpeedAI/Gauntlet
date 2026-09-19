"""
Candidate Pruning & Search Efficiency Experiment Orchestrator (EXP-N3-PRUNE).

Evaluates whether Needle 3 can serve as a cheap candidate-pruning instrument
when deterministic hard gates leave N in [4, 5] candidate affordances unresolved
and probing every candidate has meaningful payment.

Compares 4 Arms:
- Arm A: Exhaustive Probe Reference (N probes)
- Arm B: Exact Random Expectation (analytical enumeration of all C(N, k) combinations)
- Arm C: Gauntlet §15.1 Real Deterministic Prioritizer
- Arm D: Needle 3 Top-K Pruner (k = 2)
"""

from __future__ import annotations

import argparse
import json
import os
import sys
import time
from typing import Any, Dict, List, Tuple

from .pruning_adapter import NeedlePruningAdapter
from .pruning_contract import (
    CandidateAffordance,
    ConsequenceClassification,
    HorizonPruningEvaluation,
    PruningHorizon,
    PruningRanking,
    compute_exact_random_expectation,
)
from .pruning_dataset import compute_dataset_sha256, make_pruning_horizons
from .pruning_deterministic import GauntletSection151Prioritizer, PureLexicalOrderBaseline
from .pruning_teeth import run_all_pruning_teeth


def evaluate_ranking_on_horizon(
    horizon: PruningHorizon,
    ranking: PruningRanking,
) -> HorizonPruningEvaluation:
    """Evaluates a top-k candidate ranking against frozen consequence truth."""
    selected_ids = ranking.ranked_candidate_ids
    k = horizon.probe_budget_k
    n = len(horizon.candidates)

    useful_set = horizon.useful_candidate_ids
    advances_set = horizon.advances_candidate_ids

    total_useful = len(useful_set)
    total_advances = len(advances_set)

    # Retained useful & advances in top-k
    retained_useful = len(set(selected_ids) & useful_set)
    retained_advances = len(set(selected_ids) & advances_set)

    useful_recall = (retained_useful / total_useful) if total_useful > 0 else 0.0
    horizon_success = retained_advances > 0
    diversity_ratio = (retained_advances / total_advances) if total_advances > 0 else 0.0

    # Payment calculations
    # Probes avoided = N - len(selected_ids)
    probes_avoided = n - len(selected_ids)
    all_probe_time = sum(c.probe_wall_clock_ms for c in horizon.candidates)
    selected_probe_time = sum(
        c.probe_wall_clock_ms for c in horizon.candidates if c.affordance_id in selected_ids
    )
    probe_wall_clock_avoided = all_probe_time - selected_probe_time
    net_wall_clock_saved = probe_wall_clock_avoided - ranking.strategy_latency_ms

    return HorizonPruningEvaluation(
        horizon_id=horizon.horizon_id,
        candidate_count_n=n,
        probe_budget_k=k,
        selected_candidate_ids=selected_ids,
        useful_candidates_count=total_useful,
        retained_useful_count=retained_useful,
        useful_recall_at_k=round(useful_recall, 4),
        horizon_success_at_k=horizon_success,
        retained_advances_count=retained_advances,
        total_advances_count=total_advances,
        diversity_retention_ratio=round(diversity_ratio, 4),
        probes_avoided=probes_avoided,
        probe_wall_clock_avoided_ms=round(probe_wall_clock_avoided, 2),
        strategy_latency_ms=ranking.strategy_latency_ms,
        net_wall_clock_saved_ms=round(net_wall_clock_saved, 2),
    )


def run_pruning_experiment(repo_root: str) -> Dict[str, Any]:
    """Runs the full EXP-N3-PRUNE experiment."""
    print("\n" + "=" * 70)
    print("TASK 28-C: CANDIDATE-PRUNING & SEARCH-EFFICIENCY EXPERIMENT (EXP-N3-PRUNE)")
    print("=" * 70)

    # 1. Dataset Provenance & Leakage Audit
    dataset_sha256 = compute_dataset_sha256()
    print(f"Dataset SHA-256:       {dataset_sha256}")
    horizons = make_pruning_horizons()
    total_h = len(horizons)
    print(f"Total Ambiguous Horizons: {total_h}")
    assert total_h == 16, "Expected 16 horizons"

    reports_dir = os.path.join(repo_root, ".agents", "reports", "task28-pruning-experiment")
    obs_dir = os.path.join(reports_dir, "observations")
    os.makedirs(obs_dir, exist_ok=True)

    # 2. Adversarial Teeth Battery (T1–T8)
    print("\n" + "-" * 70)
    print("ADVERSARIAL TEETH BATTERY (T1–T8)")
    print("-" * 70)
    needle_pruner = NeedlePruningAdapter(generation=3)
    teeth_results = run_all_pruning_teeth(needle_pruner)
    teeth_all_passed = all(t["passed"] for t in teeth_results)
    for t in teeth_results:
        st = "PASS" if t["passed"] else "FAIL"
        print(f"  [{st}] {t['tooth']}: {t['evidence']}")
    print(f"Teeth Battery Passed: {teeth_all_passed}")

    # 3. Experimental Arms Execution
    print("\n" + "-" * 70)
    print("EXECUTING EXPERIMENTAL ARMS (k = 2)")
    print("-" * 70)

    # Arm A: Exhaustive Probe Reference
    exhaustive_probes_count = sum(len(h.candidates) for h in horizons)
    exhaustive_wall_clock_ms = sum(sum(c.probe_wall_clock_ms for c in h.candidates) for h in horizons)
    print(f"Arm A (Exhaustive Reference): {exhaustive_probes_count} probes, {exhaustive_wall_clock_ms:.1f} ms wall-clock.")

    # Arm B: Exact Random Expectation
    random_recalls = []
    random_successes = []
    random_probe_times = []
    for h in horizons:
        exp_rec, exp_suc, _ = compute_exact_random_expectation(h)
        random_recalls.append(exp_rec)
        random_successes.append(exp_suc)
        # Expected probe time is (k / n) * sum(times)
        p_time = (h.probe_budget_k / len(h.candidates)) * sum(c.probe_wall_clock_ms for c in h.candidates)
        random_probe_times.append(p_time)

    arm_b_metrics = {
        "expected_useful_recall_at_k": round(sum(random_recalls) / total_h, 4),
        "expected_horizon_success_at_k": round(sum(random_successes) / total_h, 4),
        "total_probes_conducted": total_h * 2,
        "total_probes_avoided": exhaustive_probes_count - (total_h * 2),
        "total_probe_wall_clock_ms": round(sum(random_probe_times), 2),
        "net_wall_clock_saved_ms": round(exhaustive_wall_clock_ms - sum(random_probe_times), 2),
    }
    print(f"Arm B (Exact Random Expectation): Recall@2 = {arm_b_metrics['expected_useful_recall_at_k']*100:.1f}%, Success@2 = {arm_b_metrics['expected_horizon_success_at_k']*100:.1f}%")

    # Arm C: Gauntlet §15.1 Deterministic Prioritizer
    det_prioritizer = GauntletSection151Prioritizer()
    det_evals: List[HorizonPruningEvaluation] = []
    for h in horizons:
        ranking = det_prioritizer.prune(h)
        ev = evaluate_ranking_on_horizon(h, ranking)
        det_evals.append(ev)

    arm_c_useful_recalls = [e.useful_recall_at_k for e in det_evals]
    arm_c_successes = [1.0 if e.horizon_success_at_k else 0.0 for e in det_evals]
    arm_c_probe_time = sum(sum(c.probe_wall_clock_ms for c in h.candidates if c.affordance_id in e.selected_candidate_ids) for h, e in zip(horizons, det_evals))
    arm_c_latency = sum(e.strategy_latency_ms for e in det_evals)

    arm_c_metrics = {
        "useful_recall_at_k": round(sum(arm_c_useful_recalls) / total_h, 4),
        "horizon_success_at_k": round(sum(arm_c_successes) / total_h, 4),
        "total_probes_conducted": sum(len(e.selected_candidate_ids) for e in det_evals),
        "total_probes_avoided": sum(e.probes_avoided for e in det_evals),
        "total_probe_wall_clock_ms": round(arm_c_probe_time, 2),
        "total_strategy_latency_ms": round(arm_c_latency, 3),
        "net_wall_clock_saved_ms": round(exhaustive_wall_clock_ms - (arm_c_probe_time + arm_c_latency), 2),
        "diversity_retention_ratio": round(sum(e.diversity_retention_ratio for e in det_evals) / total_h, 4),
    }
    print(f"Arm C (Gauntlet §15.1 Prioritizer): Recall@2 = {arm_c_metrics['useful_recall_at_k']*100:.1f}%, Success@2 = {arm_c_metrics['horizon_success_at_k']*100:.1f}%")

    # Arm D: Needle 3 Top-K Pruner
    print("Executing Arm D (Needle 3 Top-K Pruner)...")
    needle_evals: List[HorizonPruningEvaluation] = []
    needle_latencies = []
    needle_peak_rams = []

    for h in horizons:
        ranking = needle_pruner.prune(h)
        ev = evaluate_ranking_on_horizon(h, ranking)
        needle_evals.append(ev)
        needle_latencies.append(ranking.strategy_latency_ms)
        if ranking.telemetry.get("peak_ram_mb"):
            needle_peak_rams.append(ranking.telemetry["peak_ram_mb"])

        # Persist observation
        obs_file = os.path.join(obs_dir, f"{h.horizon_id}_needle3.json")
        with open(obs_file, "w", encoding="utf-8") as f:
            json.dump({
                "horizon_id": h.horizon_id,
                "family": h.family,
                "state_summary": h.state_summary,
                "candidates": [
                    {
                        "affordance_id": c.affordance_id,
                        "description": c.description,
                        "consequence": c.consequence.value,
                        "probe_wall_clock_ms": c.probe_wall_clock_ms,
                    }
                    for c in h.candidates
                ],
                "ranking": {
                    "ranked_candidate_ids": ranking.ranked_candidate_ids,
                    "abstained": ranking.abstained,
                    "reported_confidence": ranking.reported_confidence,
                    "strategy_latency_ms": ranking.strategy_latency_ms,
                    "telemetry": ranking.telemetry,
                },
                "evaluation": {
                    "selected_ids": ev.selected_candidate_ids,
                    "useful_recall_at_k": ev.useful_recall_at_k,
                    "horizon_success_at_k": ev.horizon_success_at_k,
                    "retained_advances_count": ev.retained_advances_count,
                    "probes_avoided": ev.probes_avoided,
                    "probe_wall_clock_avoided_ms": ev.probe_wall_clock_avoided_ms,
                    "net_wall_clock_saved_ms": ev.net_wall_clock_saved_ms,
                },
            }, f, indent=2, sort_keys=True)

    arm_d_useful_recalls = [e.useful_recall_at_k for e in needle_evals]
    arm_d_successes = [1.0 if e.horizon_success_at_k else 0.0 for e in needle_evals]
    arm_d_probe_time = sum(sum(c.probe_wall_clock_ms for c in h.candidates if c.affordance_id in e.selected_candidate_ids) for h, e in zip(horizons, needle_evals))
    arm_d_latency = sum(needle_latencies)
    arm_d_net_saved = exhaustive_wall_clock_ms - (arm_d_probe_time + arm_d_latency)

    arm_d_metrics = {
        "useful_recall_at_k": round(sum(arm_d_useful_recalls) / total_h, 4),
        "horizon_success_at_k": round(sum(arm_d_successes) / total_h, 4),
        "total_probes_conducted": sum(len(e.selected_candidate_ids) for e in needle_evals),
        "total_probes_avoided": sum(e.probes_avoided for e in needle_evals),
        "total_probe_wall_clock_ms": round(arm_d_probe_time, 2),
        "total_strategy_latency_ms": round(arm_d_latency, 2),
        "mean_inference_latency_ms": round(arm_d_latency / total_h, 2),
        "peak_ram_mb": round(max(needle_peak_rams), 1) if needle_peak_rams else None,
        "net_wall_clock_saved_ms": round(arm_d_net_saved, 2),
        "diversity_retention_ratio": round(sum(e.diversity_retention_ratio for e in needle_evals) / total_h, 4),
    }
    print(f"Arm D (Needle 3 Top-K Pruner): Recall@2 = {arm_d_metrics['useful_recall_at_k']*100:.1f}%, Success@2 = {arm_d_metrics['horizon_success_at_k']*100:.1f}%")

    # 4. Paired Horizon-Level Incremental Value Matrix (Correction #6)
    both_count = 0
    needle_only_count = 0
    det_only_count = 0
    neither_count = 0

    for n_ev, d_ev in zip(needle_evals, det_evals):
        n_suc = n_ev.horizon_success_at_k
        d_suc = d_ev.horizon_success_at_k

        if n_suc and d_suc:
            both_count += 1
        elif n_suc and not d_suc:
            needle_only_count += 1
        elif not n_suc and d_suc:
            det_only_count += 1
        else:
            neither_count += 1

    incremental_matrix = {
        "both": both_count,
        "needle_only": needle_only_count,
        "deterministic_only": det_only_count,
        "neither": neither_count,
    }
    print("\nPaired Incremental Value Matrix (Horizon Success):")
    print(f"  both:               {both_count}")
    print(f"  needle_only:        {needle_only_count}")
    print(f"  deterministic_only: {det_only_count}")
    print(f"  neither:            {neither_count}")

    # 5. Claims Evaluation
    claim_p1_earned = arm_d_metrics["useful_recall_at_k"] > arm_b_metrics["expected_useful_recall_at_k"]
    claim_p2_earned = arm_d_metrics["net_wall_clock_saved_ms"] > 0 and arm_d_metrics["total_probes_avoided"] > 0
    claim_p3_earned = needle_only_count > det_only_count
    claim_p4_earned = False  # Phase 2 live consequence usefulness

    # Phase 2 Authorization Gate
    phase_2_authorized = (
        claim_p1_earned
        and claim_p2_earned
        and claim_p3_earned
        and teeth_all_passed
    )
    print(f"\nPhase-2 Live Route-Discovery Gate Authorized: {phase_2_authorized}")

    # Final Recommendation
    if phase_2_authorized:
        final_recommendation = "USE_IN_ROUTE_DISCOVERY_EXPERIMENTS"
        next_action = "Authorize bounded live route-discovery experiment slice."
    elif claim_p1_earned and not claim_p3_earned:
        final_recommendation = "REMOVE_NEEDLE"
        next_action = "Roll back Needle integration; Needle shows pruning signal but is strictly inferior to §15.1 deterministic prioritizer."
    else:
        final_recommendation = "REMOVE_NEEDLE"
        next_action = "Roll back Needle integration; no search-payment or incremental value demonstrated."

    report = {
        "experiment_id": "EXP-N3-PRUNE",
        "timestamp": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "provenance": {
            "dataset_sha256": dataset_sha256,
            "total_horizons": total_h,
            "probe_budget_k": 2,
            "needle_model": "Cactus Needle 3 (v3-20L, 35.3 MB)",
        },
        "arms": {
            "arm_a_exhaustive": {
                "total_probes_conducted": exhaustive_probes_count,
                "total_probe_wall_clock_ms": round(exhaustive_wall_clock_ms, 2),
                "useful_recall_at_k": 1.0,
                "horizon_success_at_k": 1.0,
            },
            "arm_b_random_expectation": arm_b_metrics,
            "arm_c_deterministic_prioritizer": arm_c_metrics,
            "arm_d_needle3_pruner": arm_d_metrics,
        },
        "incremental_matrix": incremental_matrix,
        "adversarial_teeth": teeth_results,
        "claims": {
            "claim_p1_pruning_signal": {
                "earned": claim_p1_earned,
                "evidence": f"Needle recall {arm_d_metrics['useful_recall_at_k']*100:.1f}% vs random expectation {arm_b_metrics['expected_useful_recall_at_k']*100:.1f}%",
            },
            "claim_p2_search_payment_reduction": {
                "earned": claim_p2_earned,
                "evidence": f"Net wall-clock saved: {arm_d_metrics['net_wall_clock_saved_ms']:.1f} ms, probes avoided: {arm_d_metrics['total_probes_avoided']}",
            },
            "claim_p3_incremental_value": {
                "earned": claim_p3_earned,
                "evidence": f"Paired comparison: needle_only = {needle_only_count} vs deterministic_only = {det_only_count}",
            },
            "claim_p4_live_consequence_usefulness": {
                "earned": claim_p4_earned,
                "evidence": "Withheld per frozen Phase-2 authorization gate.",
            },
        },
        "phase_2_gate": {
            "authorized": phase_2_authorized,
            "reasons": {
                "p1_pruning_signal": claim_p1_earned,
                "p2_payment_reduction": claim_p2_earned,
                "p3_incremental_value": claim_p3_earned,
                "teeth_passed": teeth_all_passed,
            },
        },
        "final_recommendation": {
            "recommendation": final_recommendation,
            "next_action": next_action,
        },
    }

    report_json_path = os.path.join(reports_dir, "report.json")
    with open(report_json_path, "w", encoding="utf-8") as f:
        json.dump(report, f, indent=2, sort_keys=True)
    print(f"\nSaved structured report: {report_json_path}")

    report_md_path = os.path.join(reports_dir, "report.md")
    write_pruning_markdown_report(report, report_md_path)
    print(f"Saved markdown report:   {report_md_path}")

    return report


def write_pruning_markdown_report(report: Dict[str, Any], filepath: str) -> None:
    """Writes human-readable markdown report for EXP-N3-PRUNE."""
    arms = report["arms"]
    a = arms["arm_a_exhaustive"]
    b = arms["arm_b_random_expectation"]
    c = arms["arm_c_deterministic_prioritizer"]
    d = arms["arm_d_needle3_pruner"]

    inc = report["incremental_matrix"]
    claims = report["claims"]
    rec = report["final_recommendation"]

    lines = [
        "# EXP-N3-PRUNE: Candidate-Pruning and Search-Efficiency Proving Report",
        f"**Date:** {report['timestamp']}  ",
        f"**Dataset SHA-256:** `{report['provenance']['dataset_sha256']}`  ",
        f"**Final Recommendation:** `{rec['recommendation']}`  \n",
        "## 1. Executive Summary",
        "This experiment evaluated whether Cactus Needle 3 can serve as a cheap candidate-pruning instrument ",
        "when deterministic hard gates leave N in [4, 5] candidate affordances unresolved and probing every candidate ",
        "incurs real execution payment. The comparison evaluated Needle against exhaustive probing, analytical random expectation, ",
        "and Gauntlet's real §15.1 deterministic prioritizer.",
        "",
        "## 2. Experimental Arms Comparison (Probe Budget k = 2)",
        "",
        "| Metric | Arm A (Exhaustive) | Arm B (Random Expectation) | Arm C (§15.1 Deterministic) | Arm D (Needle 3 Pruner) |",
        "| :--- | :--- | :--- | :--- | :--- |",
        f"| **Probes Conducted** | {a['total_probes_conducted']} | {b['total_probes_conducted']} | {c['total_probes_conducted']} | {d['total_probes_conducted']} |",
        f"| **Probes Avoided** | 0 | {b['total_probes_avoided']} | {c['total_probes_avoided']} | {d['total_probes_avoided']} |",
        f"| **Useful Recall @ 2** | 100.0% | {b['expected_useful_recall_at_k']*100:.1f}% | {c['useful_recall_at_k']*100:.1f}% | **{d['useful_recall_at_k']*100:.1f}%** |",
        f"| **Horizon Success @ 2** | 100.0% | {b['expected_horizon_success_at_k']*100:.1f}% | {c['horizon_success_at_k']*100:.1f}% | **{d['horizon_success_at_k']*100:.1f}%** |",
        f"| **Probe Wall-Clock** | {a['total_probe_wall_clock_ms']:.1f} ms | {b['total_probe_wall_clock_ms']:.1f} ms | {c['total_probe_wall_clock_ms']:.1f} ms | {d['total_probe_wall_clock_ms']:.1f} ms |",
        f"| **Inference Overhead** | 0.0 ms | 0.0 ms | {c['total_strategy_latency_ms']:.1f} ms | {d['total_strategy_latency_ms']:.1f} ms |",
        f"| **Net Wall-Clock Saved** | 0.0 ms | {b['net_wall_clock_saved_ms']:.1f} ms | {c['net_wall_clock_saved_ms']:.1f} ms | **{d['net_wall_clock_saved_ms']:.1f} ms** |",
        f"| **Diversity Retention** | 100.0% | N/A | {c['diversity_retention_ratio']*100:.1f}% | {d['diversity_retention_ratio']*100:.1f}% |",
        "",
        "## 3. Paired Incremental Value Matrix (Horizon Success)",
        "| Quadrant | Count | Percentage | Interpretation |",
        "| :--- | :--- | :--- | :--- |",
        f"| `both` | {inc['both']} | {inc['both']/16*100:.1f}% | Both Needle 3 and §15.1 Prioritizer retained an advancing candidate |",
        f"| `needle_only` | **{inc['needle_only']}** | **{inc['needle_only']/16*100:.1f}%** | Needle succeeded where §15.1 Prioritizer failed (True Incremental Value) |",
        f"| `deterministic_only` | {inc['deterministic_only']} | {inc['deterministic_only']/16*100:.1f}% | §15.1 Prioritizer succeeded where Needle failed |",
        f"| `neither` | {inc['neither']} | {inc['neither']/16*100:.1f}% | Neither strategy retained an advancing candidate |",
        "",
        "## 4. Adversarial Teeth Battery (T1–T8)",
    ]
    for t in report["adversarial_teeth"]:
        st = "PASS" if t["passed"] else "FAIL"
        lines.append(f"- **{t['tooth']}**: `{st}` — {t['evidence']}")

    lines.extend([
        "",
        "## 5. Claims Evaluation",
        f"- **Claim P1 (Pruning Signal):** `{'EARNED' if claims['claim_p1_pruning_signal']['earned'] else 'UNEARNED'}` — {claims['claim_p1_pruning_signal']['evidence']}",
        f"- **Claim P2 (Search-Payment Reduction):** `{'EARNED' if claims['claim_p2_search_payment_reduction']['earned'] else 'UNEARNED'}` — {claims['claim_p2_search_payment_reduction']['evidence']}",
        f"- **Claim P3 (Incremental Value):** `{'EARNED' if claims['claim_p3_incremental_value']['earned'] else 'UNEARNED'}` — {claims['claim_p3_incremental_value']['evidence']}",
        f"- **Claim P4 (Live Consequence Usefulness):** `{'EARNED' if claims['claim_p4_live_consequence_usefulness']['earned'] else 'UNEARNED'}` — {claims['claim_p4_live_consequence_usefulness']['evidence']}",
        "",
        "## 6. Final Recommendation",
        f"**Recommendation:** `{rec['recommendation']}`  ",
        f"**Next Action:** {rec['next_action']}",
    ])

    with open(filepath, "w", encoding="utf-8") as f:
        f.write("\n".join(lines) + "\n")


if __name__ == "__main__":
    repo_root = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
    run_pruning_experiment(repo_root)
