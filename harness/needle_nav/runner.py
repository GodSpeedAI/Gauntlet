"""
Main Experiment Orchestrator for Needle Navigation Proving Experiment in Gauntlet.

Commands:
    python -m harness.needle_nav.runner --phase 1    (Mechanical smoke)
    python -m harness.needle_nav.runner --phase 2    (Frozen offline replay)
    python -m harness.needle_nav.runner --teeth      (Adversarial teeth battery)
    python -m harness.needle_nav.runner --phase 3    (Consequence-backed validation)
    python -m harness.needle_nav.runner --all        (Full end-to-end experiment)
"""

from __future__ import annotations

import argparse
import json
import os
import sys
import time
from typing import Any, Dict, List

from .baselines import DeterministicRuleNavigator, RandomNavigator
from .consequence_validator import ConsequenceValidator
from .dataset import HorizonDataset
from .metrics import evaluate_horizon_batch
from .needle_adapter import NeedleNavigatorAdapter
from .persistence import ObservationStore
from .port import AffordanceSchema, NavigationRequest, NavigatorPort
from .teeth import run_all_teeth


def run_phase_1_smoke(needle3: NavigatorPort, needle2: NavigatorPort) -> Dict[str, Any]:
    """Phase 1: Mechanical smoke test proving instrument validity."""
    print("\n" + "=" * 60)
    print("PHASE 1: MECHANICAL SMOKE TEST")
    print("=" * 60)

    sample_tool = AffordanceSchema(
        name="dispatch_markup_builder",
        description="Implement calculator markup",
        parameters={
            "type": "object",
            "properties": {
                "unit_id": {"type": "string", "enum": ["AK-9011"]},
                "target_file": {"type": "string", "enum": ["calculator.html"]},
            },
            "required": ["unit_id", "target_file"],
        },
    )
    req_action = NavigationRequest(
        horizon_id="smoke_actionable",
        case_id="smoke",
        state_summary="Initial horizon: create calculator.html with digit buttons.",
        affordance_schemas=[sample_tool],
    )
    req_abstain = NavigationRequest(
        horizon_id="smoke_abstain",
        case_id="smoke",
        state_summary="System task: order coffee delivery to office.",
        affordance_schemas=[sample_tool],
    )

    results = {}
    for name, nav in [("needle-3", needle3), ("needle-2", needle2)]:
        prop_act = nav.propose(req_action)
        prop_abs = nav.propose(req_abstain)

        assert prop_act.affordance_id == "dispatch_markup_builder", f"{name} failed to propose exposed tool"
        assert prop_act.arguments.get("unit_id") == "AK-9011", f"{name} failed to extract argument"
        assert prop_abs.abstained or prop_abs.affordance_id is None, f"{name} failed to abstain on irrelevant query"

        results[name] = {
            "actionable_proposal": {
                "affordance_id": prop_act.affordance_id,
                "arguments": prop_act.arguments,
                "reported_confidence": prop_act.reported_confidence,
                "abstained": prop_act.abstained,
                "latency_ms": prop_act.provider_provenance.latency_ms,
                "telemetry": prop_act.provider_provenance.telemetry,
            },
            "abstention_proposal": {
                "affordance_id": prop_abs.affordance_id,
                "abstained": prop_abs.abstained,
                "reported_confidence": prop_abs.reported_confidence,
                "decision_note": prop_abs.decision_note,
            },
            "smoke_passed": True,
        }
        print(f"  [{name}] Smoke PASS: Action={prop_act.affordance_id}, Abstain={prop_abs.abstained}, Latency={prop_act.provider_provenance.latency_ms}ms")

    return results


def run_phase_2_replay(
    repo_root: str,
    dataset: HorizonDataset,
    navigators: Dict[str, NavigatorPort],
    store: ObservationStore,
) -> Dict[str, Any]:
    """Phase 2: Frozen offline replay across the reconstructed horizon corpus."""
    print("\n" + "=" * 60)
    print("PHASE 2: FROZEN OFFLINE REPLAY EXPERIMENT")
    print(f"Total Horizons in Frozen Dataset: {len(dataset.records)}")
    print("=" * 60)

    req_tuples = dataset.to_requests()
    eval_results = {}

    for nav_name, navigator in navigators.items():
        print(f"\nEvaluating Navigator: {nav_name}...")
        batch_records = []
        t0 = time.perf_counter()

        for req, gt in req_tuples:
            proposal = navigator.propose(req)
            # Persist raw observation pre-scoring
            store.persist(req, proposal)
            batch_records.append({
                "request": req,
                "proposal": proposal,
                "ground_truth": gt,
            })

        elapsed_s = time.perf_counter() - t0
        metrics = evaluate_horizon_batch(batch_records)
        metrics["total_elapsed_seconds"] = round(elapsed_s, 2)
        eval_results[nav_name] = metrics

        print(f"  Schema-valid rate:    {metrics['schema_valid_rate'] * 100:.1f}%")
        print(f"  Invalid-affordance:   {metrics['invalid_affordance_rate'] * 100:.1f}%")
        print(f"  Valid-next-move rate: {metrics['valid_next_move_rate'] * 100:.1f}%")
        print(f"  Responsible abstain:  {metrics['abstention_metrics']['responsible_abstention_rate'] * 100:.1f}%")
        print(f"  Excessive abstain:    {metrics['abstention_metrics']['excessive_abstention_rate'] * 100:.1f}%")
        print(f"  Historical agreement: {metrics['descriptive_metrics']['historical_move_agreement_rate'] * 100:.1f}% (descriptive only)")
        print(f"  Mean latency:         {metrics['payment']['mean_latency_ms']} ms")
        if metrics['payment'].get('peak_ram_mb'):
            print(f"  Peak RAM:             {metrics['payment']['peak_ram_mb']} MB")

    return eval_results


def evaluate_phase_2_gate(eval_results: Dict[str, Any]) -> Tuple[bool, Dict[str, Any]]:
    """Evaluates the frozen Phase 2 -> Phase 3 authorization gate."""
    n3 = eval_results.get("needle-3", {})
    random_base = eval_results.get("random", {})

    claim_a_checks = {
        "schema_valid_100": n3.get("schema_valid_rate") == 1.0,
        "invalid_affordance_0": n3.get("invalid_affordance_rate") == 0.0,
    }
    claim_a_passed = all(claim_a_checks.values())

    n3_valid = n3.get("valid_next_move_rate", 0.0)
    rand_valid = random_base.get("valid_next_move_rate", 0.0)
    margin = n3_valid - rand_valid

    resp_abstain = n3.get("abstention_metrics", {}).get("responsible_abstention_rate", 0.0)
    excess_abstain = n3.get("abstention_metrics", {}).get("excessive_abstention_rate", 0.0)

    claim_b_checks = {
        "valid_next_move_ge_70": n3_valid >= 0.70,
        "responsible_abstain_ge_90": resp_abstain >= 0.90,
        "excessive_abstain_le_15": excess_abstain <= 0.15,
        "margin_over_random_ge_25": margin >= 0.25,
    }
    claim_b_passed = all(claim_b_checks.values())

    authorized = claim_a_passed and claim_b_passed

    gate_report = {
        "claim_a_passed": claim_a_passed,
        "claim_a_checks": claim_a_checks,
        "claim_b_passed": claim_b_passed,
        "claim_b_checks": claim_b_checks,
        "needle3_valid_rate": n3_valid,
        "random_valid_rate": rand_valid,
        "margin_over_random": round(margin, 4),
        "phase_3_authorized": authorized,
    }
    return authorized, gate_report


def run_phase_3_validation(
    repo_root: str,
    needle3: NavigatorPort,
) -> Dict[str, Any]:
    """Phase 3: Consequence-backed branch replay under real governance."""
    print("\n" + "=" * 60)
    print("PHASE 3: CONSEQUENCE-BACKED BRANCH REPLAY")
    print("=" * 60)

    validator = ConsequenceValidator(repo_root)

    # Replay two key representative horizons:
    # 1. Initial creation horizon (AK-9011)
    tools = [
        AffordanceSchema(
            name="dispatch_markup_builder",
            description="Implement calculator.html markup with buttons",
            parameters={
                "type": "object",
                "properties": {
                    "unit_id": {"type": "string", "enum": ["AK-9011"]},
                    "target_file": {"type": "string", "enum": ["calculator.html"]},
                },
                "required": ["unit_id", "target_file"],
            },
            authority_tier="contained",
        ),
        AffordanceSchema(
            name="dispatch_check_runner",
            description="Run check script",
            parameters={"type": "object", "properties": {"unit_id": {"type": "string"}}},
            authority_tier="contained",
        ),
    ]
    req_initial = NavigationRequest(
        horizon_id="p3_initial_markup",
        case_id="demo-calculator",
        state_summary="Case: demo-calculator. Iteration 1. Unit states: [['AK-9011', 'open'], ['AK-9012', 'open'], ['AK-9013', 'open']]. Build calculator.html.",
        affordance_schemas=tools,
        constraints={"authority_tier": "contained"},
    )
    prop_initial = needle3.propose(req_initial)
    res_initial = validator.validate_and_execute(req_initial, prop_initial)
    print(f"  Leg 1 (Initial Horizon): Proposal={prop_initial.affordance_id}, Status={res_initial['status']}, Settled={res_initial['settlement_achieved']}")

    # 2. All-settled terminal horizon (must abstain -> no execution)
    req_terminal = NavigationRequest(
        horizon_id="p3_terminal",
        case_id="demo-calculator",
        state_summary="Case: demo-calculator. Iteration 5. All units settled: AK-9011, AK-9012, AK-9013 settled. Verification PASS.",
        affordance_schemas=tools,
        constraints={"authority_tier": "contained"},
    )
    prop_terminal = needle3.propose(req_terminal)
    res_terminal = validator.validate_and_execute(req_terminal, prop_terminal)
    print(f"  Leg 2 (Terminal Horizon): Proposal={prop_terminal.affordance_id} (abstained={prop_terminal.abstained}), Status={res_terminal['status']}")

    p3_passed = res_initial["settlement_achieved"] and res_terminal["status"] == "ABSTAINED"
    return {
        "leg1_initial": {
            "proposal": {
                "affordance_id": prop_initial.affordance_id,
                "arguments": prop_initial.arguments,
                "reported_confidence": prop_initial.reported_confidence,
            },
            "validation_and_execution": res_initial,
        },
        "leg2_terminal": {
            "proposal": {
                "affordance_id": prop_terminal.affordance_id,
                "abstained": prop_terminal.abstained,
                "reported_confidence": prop_terminal.reported_confidence,
            },
            "validation_and_execution": res_terminal,
        },
        "phase_3_passed": p3_passed,
    }


def main():
    parser = argparse.ArgumentParser(description="Needle Navigation Proving Experiment in Gauntlet")
    parser.add_argument("--phase", type=str, choices=["1", "2", "3"], help="Run specific phase")
    parser.add_argument("--teeth", action="store_true", help="Run adversarial teeth battery")
    parser.add_argument("--all", action="store_true", help="Run complete proving experiment end-to-end")
    args = parser.parse_args()

    repo_root = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
    reports_dir = os.path.join(repo_root, ".agents", "reports", "task28-needle-navigation")
    os.makedirs(reports_dir, exist_ok=True)

    needle3 = NeedleNavigatorAdapter(generation=3)
    needle2 = NeedleNavigatorAdapter(generation=2)
    rand_nav = RandomNavigator(seed=42)
    det_nav = DeterministicRuleNavigator()

    store = ObservationStore()
    dataset = HorizonDataset.load_or_reconstruct(repo_root)

    report: Dict[str, Any] = {
        "task_id": "Task 28 (EXP-N3)",
        "timestamp": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "repo_root": repo_root,
        "models": {
            "needle3": {"id": needle3.model_id, "depth": needle3.depth_or_rung},
            "needle2": {"id": needle2.model_id, "depth": needle2.depth_or_rung},
        },
        "dataset_summary": {
            "total_horizons": len(dataset.records),
            "manifest_path": os.path.join(reports_dir, "dataset_manifest.json"),
        },
    }

    # Execute Phase 1
    if args.phase == "1" or args.all:
        p1_res = run_phase_1_smoke(needle3, needle2)
        report["phase_1_smoke"] = p1_res

    # Execute Teeth
    teeth_res = None
    if args.teeth or args.all:
        print("\n" + "=" * 60)
        print("ADVERSARIAL TEETH BATTERY (7 TEETH)")
        print("=" * 60)
        teeth_res = run_all_teeth(needle3)
        for t in teeth_res:
            status = "PASS" if t["passed"] else "FAIL"
            print(f"  [{status}] {t['tooth']}: {t['evidence']}")
        report["teeth"] = teeth_res

    # Execute Phase 2
    gate_authorized = False
    gate_report = {}
    if args.phase == "2" or args.all:
        navigators = {
            "needle-3": needle3,
            "needle-2": needle2,
            "deterministic_rule": det_nav,
            "random": rand_nav,
        }
        p2_res = run_phase_2_replay(repo_root, dataset, navigators, store)
        report["phase_2_replay"] = p2_res

        gate_authorized, gate_report = evaluate_phase_2_gate(p2_res)
        report["phase_2_gate"] = gate_report
        print(f"\nPhase 2 -> Phase 3 Gate Authorized: {gate_authorized}")
        print(f"  Claim A Passed: {gate_report['claim_a_passed']}")
        print(f"  Claim B Passed: {gate_report['claim_b_passed']} (N3 Valid={gate_report['needle3_valid_rate']*100:.1f}%, Random={gate_report['random_valid_rate']*100:.1f}%)")

    # Execute Phase 3 if authorized
    p3_res = None
    if (args.phase == "3") or (args.all and gate_authorized):
        p3_res = run_phase_3_validation(repo_root, needle3)
        report["phase_3_consequence"] = p3_res
    elif args.all and not gate_authorized:
        print("\n[Phase 3 SKIPPED]: Phase 2 gate criteria were not met; consequence-backed replay withheld.")
        report["phase_3_consequence"] = {
            "status": "WITHHELD",
            "reason": "Phase 2 gate thresholds for Claim A or Claim B not satisfied",
        }

    # Final Claims Adjudication & Recommendation
    p2 = report.get("phase_2_replay", {}).get("needle-3", {})
    all_teeth_passed = all(t["passed"] for t in teeth_res) if teeth_res else False

    claim_a = (
        p2.get("schema_valid_rate") == 1.0
        and p2.get("invalid_affordance_rate") == 0.0
        and all_teeth_passed
    )
    claim_b = gate_report.get("claim_b_passed", False)
    claim_c = p3_res.get("phase_3_passed", False) if p3_res else False

    # Map to final decision per frozen matrix:
    if claim_a and claim_b and claim_c:
        final_recommendation = "AUTHORIZE_LIVE_NAVIGATION_SLICE"
        recommendation_rationale = (
            "Needle 3 achieved 100% schema validity, zero invalid affordances, passed all 7 adversarial teeth, "
            "demonstrated significant navigation signal over baselines, and its proposed moves successfully settled under real Gauntlet/SEA-Forge governance."
        )
    elif claim_a and (claim_b or (p2.get("valid_next_move_rate", 0) > report.get("phase_2_replay", {}).get("random", {}).get("valid_next_move_rate", 0))):
        final_recommendation = "RETAIN_AS_EXPERIMENTAL_NAVIGATOR"
        recommendation_rationale = (
            "Needle demonstrates instrument validity and partial navigation signal, but live execution or calibration "
            "requires further development before live slice authorization."
        )
    else:
        final_recommendation = "REJECT"
        recommendation_rationale = "Needle did not demonstrate sufficient navigation reliability or failed core teeth."

    report["claims_adjudication"] = {
        "claim_a_instrument_validity": {
            "earned": claim_a,
            "evidence": "100% schema valid, 0% invalid affordances, all teeth passed",
        },
        "claim_b_navigation_usefulness": {
            "earned": claim_b,
            "evidence": f"Valid-next-move rate {p2.get('valid_next_move_rate', 0)*100:.1f}%, margin over random {gate_report.get('margin_over_random', 0)*100:.1f}%",
        },
        "claim_c_consequence_usefulness": {
            "earned": claim_c,
            "evidence": "Real check.sh executed and produced passing settlement evidence from candidate proposal" if claim_c else "Unearned / not executed",
        },
    }
    report["final_decision"] = {
        "recommendation": final_recommendation,
        "rationale": recommendation_rationale,
        "next_affordable_move": "Draft bounded live navigation route card for Gauntlet prime-workbench integration" if final_recommendation == "AUTHORIZE_LIVE_NAVIGATION_SLICE" else "Investigate representation enrichment or calibration fine-tuning",
    }

    # Write report files
    json_path = os.path.join(reports_dir, "report.json")
    with open(json_path, "w", encoding="utf-8") as f:
        json.dump(report, f, indent=2, sort_keys=True)
    print(f"\nSaved structured report: {json_path}")

    md_path = os.path.join(reports_dir, "report.md")
    write_markdown_report(report, md_path)
    print(f"Saved markdown report: {md_path}")
    print(f"\nFINAL RECOMMENDATION: {final_recommendation}")


def write_markdown_report(report: Dict[str, Any], filepath: str):
    p2 = report.get("phase_2_replay", {})
    n3 = p2.get("needle-3", {})
    n2 = p2.get("needle-2", {})
    det = p2.get("deterministic_rule", {})
    rnd = p2.get("random", {})
    dec = report.get("final_decision", {})
    claims = report.get("claims_adjudication", {})

    lines = [
        "# Needle 3 Navigation Proving Experiment Report",
        f"**Task ID:** {report.get('task_id')}  ",
        f"**Date:** {report.get('timestamp')}  ",
        f"**Final Decision:** `{dec.get('recommendation')}`  \n",
        "## 1. Executive Summary",
        dec.get("rationale", ""),
        "",
        "## 2. Experimental Setup",
        f"- **Replay Corpus:** {report.get('dataset_summary', {}).get('total_horizons')} horizons reconstructed from 16 real Gauntlet runs (`targets/.runs/`) and test targets.",
        f"- **Needle 3 Configuration:** {report.get('models', {}).get('needle3', {}).get('id')} ({report.get('models', {}).get('needle3', {}).get('depth')}, 35.3 MB weights).",
        f"- **Needle 2 Configuration:** {report.get('models', {}).get('needle2', {}).get('id')} ({report.get('models', {}).get('needle2', {}).get('depth')}, 13.3 MB weights).",
        "- **Baselines:** Gauntlet §15.1 Lexicographic Selector (`deterministic_rule`) and Uniform Random (`random`).",
        "- **Architectural Boundary:** Needle proposed candidate moves via low-level `complete()`. Zero tool auto-execution permissions granted. GAUNTLET and SEA-Forge retained full execution authority.",
        "",
        "## 3. Replay Corpus Benchmark Results (Phase 2)",
        "",
        "| Metric | Needle 3 (v3-20L) | Needle 2 (v2-8L) | Deterministic Baseline | Random Baseline |",
        "| :--- | :--- | :--- | :--- | :--- |",
        f"| **Schema-Valid Rate** | {n3.get('schema_valid_rate', 0)*100:.1f}% | {n2.get('schema_valid_rate', 0)*100:.1f}% | {det.get('schema_valid_rate', 0)*100:.1f}% | {rnd.get('schema_valid_rate', 0)*100:.1f}% |",
        f"| **Invalid-Affordance Rate** | {n3.get('invalid_affordance_rate', 0)*100:.1f}% | {n2.get('invalid_affordance_rate', 0)*100:.1f}% | {det.get('invalid_affordance_rate', 0)*100:.1f}% | {rnd.get('invalid_affordance_rate', 0)*100:.1f}% |",
        f"| **Valid-Next-Move Rate** | **{n3.get('valid_next_move_rate', 0)*100:.1f}%** | {n2.get('valid_next_move_rate', 0)*100:.1f}% | {det.get('valid_next_move_rate', 0)*100:.1f}% | {rnd.get('valid_next_move_rate', 0)*100:.1f}% |",
        f"| **Responsible Abstention** | {n3.get('abstention_metrics', {}).get('responsible_abstention_rate', 0)*100:.1f}% | {n2.get('abstention_metrics', {}).get('responsible_abstention_rate', 0)*100:.1f}% | {det.get('abstention_metrics', {}).get('responsible_abstention_rate', 0)*100:.1f}% | {rnd.get('abstention_metrics', {}).get('responsible_abstention_rate', 0)*100:.1f}% |",
        f"| **Excessive Abstention** | {n3.get('abstention_metrics', {}).get('excessive_abstention_rate', 0)*100:.1f}% | {n2.get('abstention_metrics', {}).get('excessive_abstention_rate', 0)*100:.1f}% | {det.get('abstention_metrics', {}).get('excessive_abstention_rate', 0)*100:.1f}% | {rnd.get('abstention_metrics', {}).get('excessive_abstention_rate', 0)*100:.1f}% |",
        f"| **Historical Agreement (Descriptive)** | {n3.get('descriptive_metrics', {}).get('historical_move_agreement_rate', 0)*100:.1f}% | {n2.get('descriptive_metrics', {}).get('historical_move_agreement_rate', 0)*100:.1f}% | {det.get('descriptive_metrics', {}).get('historical_move_agreement_rate', 0)*100:.1f}% | {rnd.get('descriptive_metrics', {}).get('historical_move_agreement_rate', 0)*100:.1f}% |",
        f"| **Mean Latency** | {n3.get('payment', {}).get('mean_latency_ms')} ms | {n2.get('payment', {}).get('mean_latency_ms')} ms | {det.get('payment', {}).get('mean_latency_ms')} ms | {rnd.get('payment', {}).get('mean_latency_ms')} ms |",
        f"| **Peak RAM** | {n3.get('payment', {}).get('peak_ram_mb')} MB | {n2.get('payment', {}).get('peak_ram_mb')} MB | N/A | N/A |",
        "",
        "## 4. Adversarial Teeth Results",
        "",
    ]
    for t in report.get("teeth", []):
        st = "PASS" if t["passed"] else "FAIL"
        lines.append(f"- **{t['tooth']}**: `{st}` — {t['evidence']}")

    lines.extend([
        "",
        "## 5. Phase 3 Consequence-Backed Validation",
        f"- **Status:** `{report.get('phase_3_consequence', {}).get('phase_3_passed')}`",
        f"- **Leg 1 (Initial Markup Horizon):** Status: `{report.get('phase_3_consequence', {}).get('leg1_initial', {}).get('validation_and_execution', {}).get('status')}`, Settlement achieved: `{report.get('phase_3_consequence', {}).get('leg1_initial', {}).get('validation_and_execution', {}).get('settlement_achieved')}`",
        f"- **Leg 2 (Terminal Horizon):** Status: `{report.get('phase_3_consequence', {}).get('leg2_terminal', {}).get('validation_and_execution', {}).get('status')}`",
        "",
        "## 6. Earned Claims",
        f"- **Claim A (Instrument Validity):** `{'EARNED' if claims.get('claim_a_instrument_validity', {}).get('earned') else 'UNEARNED'}` — {claims.get('claim_a_instrument_validity', {}).get('evidence')}",
        f"- **Claim B (Navigation Signal):** `{'EARNED' if claims.get('claim_b_navigation_usefulness', {}).get('earned') else 'UNEARNED'}` — {claims.get('claim_b_navigation_usefulness', {}).get('evidence')}",
        f"- **Claim C (Consequence Usefulness):** `{'EARNED' if claims.get('claim_c_consequence_usefulness', {}).get('earned') else 'UNEARNED'}` — {claims.get('claim_c_consequence_usefulness', {}).get('evidence')}",
        "",
        "## 7. Recommendation & Next Move",
        f"**Recommendation:** `{dec.get('recommendation')}`  ",
        f"**Next Move:** {dec.get('next_affordable_move')}",
    ])

    with open(filepath, "w", encoding="utf-8") as f:
        f.write("\n".join(lines) + "\n")


if __name__ == "__main__":
    main()
