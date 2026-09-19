"""
Baseline Selectors for Split Navigation Experiment (EXP-N3-SPLIT).

Conforms to parameterless selection contract:
- DeterministicRuleSelector: Gauntlet §15.1 lexicographic ordering (governance filter,
  topological dependency ordering, bounded blast radius, responsible abstention).
- RandomSelector: Uniform random selection among exposed affordance_ids plus abstention.
"""

from __future__ import annotations

import random
import time
from typing import Any, Dict, List, Optional

from .port import NavigationRequest, ProviderProvenance
from .split_contract import NavigationSelection


class DeterministicRuleSelector:
    """Gauntlet §15.1 Lexicographic Rule Selector."""

    def select(self, request: NavigationRequest) -> NavigationSelection:
        t0 = time.perf_counter()

        admitted_tier = request.constraints.get("authority_tier", "contained")
        candidate_names = request.candidate_names()

        # 1. Filter candidates by allowed authority tier
        tier_map = {
            "dispatch_markup_builder": "contained",
            "dispatch_check_runner": "contained",
            "dispatch_eval_verifier": "contained",
            "escalate_to_operator": "human",
            "dispatch_gadget_marker_builder": "contained",
            "dispatch_lead_agent_check": "cross_cutting",
            "dispatch_local_check_critic": "local",
            "request_human_release_destroy": "irreversible",
        }

        eligible_candidates = []
        for name in candidate_names:
            required_tier = tier_map.get(name, "contained")
            if required_tier == "irreversible" and admitted_tier != "irreversible":
                continue
            if required_tier == "cross_cutting" and admitted_tier == "contained":
                continue
            eligible_candidates.append(name)

        if not eligible_candidates:
            latency_ms = (time.perf_counter() - t0) * 1000.0
            return NavigationSelection(
                affordance_id=None,
                abstained=True,
                reported_confidence=1.0,
                rationale="all_candidates_governance_blocked",
                provider_provenance=ProviderProvenance(
                    provider="deterministic_rule",
                    model_id="lexicographic_selector_v1",
                    depth_or_rung=None,
                    latency_ms=round(latency_ms, 3),
                ),
            )

        # 2. Extract open units from state summary
        open_units: List[str] = []
        for u in ["AK-9011", "AK-9012", "AK-9013", "AK-951", "AK-952", "AK-954", "AK-953"]:
            if f"'{u}', 'open'" in request.state_summary or f'"{u}", "open"' in request.state_summary:
                open_units.append(u)

        all_settled = (
            "all units settled" in request.state_summary.lower()
            or (not open_units and "settled" in request.state_summary.lower())
        )

        if all_settled or not open_units:
            latency_ms = (time.perf_counter() - t0) * 1000.0
            return NavigationSelection(
                affordance_id=None,
                abstained=True,
                reported_confidence=1.0,
                rationale="responsible_abstention_all_settled",
                provider_provenance=ProviderProvenance(
                    provider="deterministic_rule",
                    model_id="lexicographic_selector_v1",
                    depth_or_rung=None,
                    latency_ms=round(latency_ms, 3),
                ),
            )

        # 3. Unit-to-affordance topological mapping
        unit_affordance_map = {
            "AK-9011": "dispatch_markup_builder",
            "AK-9012": "dispatch_check_runner",
            "AK-9013": "dispatch_eval_verifier",
            "AK-951": "dispatch_gadget_marker_builder",
            "AK-952": "dispatch_lead_agent_check",
            "AK-954": "dispatch_local_check_critic",
            "AK-953": "request_human_release_destroy",
        }

        selected_affordance = None
        for u in open_units:
            aff = unit_affordance_map.get(u)
            if aff in eligible_candidates:
                selected_affordance = aff
                break

        latency_ms = (time.perf_counter() - t0) * 1000.0
        if selected_affordance:
            return NavigationSelection(
                affordance_id=selected_affordance,
                abstained=False,
                reported_confidence=1.0,
                rationale=f"topological_match_unit_{open_units[0]}",
                provider_provenance=ProviderProvenance(
                    provider="deterministic_rule",
                    model_id="lexicographic_selector_v1",
                    depth_or_rung=None,
                    latency_ms=round(latency_ms, 3),
                ),
            )

        return NavigationSelection(
            affordance_id=None,
            abstained=True,
            reported_confidence=1.0,
            rationale="no_eligible_open_unit_match",
            provider_provenance=ProviderProvenance(
                provider="deterministic_rule",
                model_id="lexicographic_selector_v1",
                depth_or_rung=None,
                latency_ms=round(latency_ms, 3),
            ),
        )


class RandomSelector:
    """Uniform random selection among eligible candidate affordances and abstention."""

    def __init__(self, seed: int = 42):
        self._rng = random.Random(seed)

    def select(self, request: NavigationRequest) -> NavigationSelection:
        t0 = time.perf_counter()
        options = [None] + request.candidate_names()
        choice = self._rng.choice(options)
        latency_ms = (time.perf_counter() - t0) * 1000.0

        if choice is None:
            return NavigationSelection(
                affordance_id=None,
                abstained=True,
                reported_confidence=round(1.0 / len(options), 3),
                rationale="random_abstention",
                provider_provenance=ProviderProvenance(
                    provider="random",
                    model_id="uniform_random",
                    depth_or_rung=None,
                    latency_ms=round(latency_ms, 3),
                ),
            )

        return NavigationSelection(
            affordance_id=choice,
            abstained=False,
            reported_confidence=round(1.0 / len(options), 3),
            rationale="random_selection",
            provider_provenance=ProviderProvenance(
                provider="random",
                model_id="uniform_random",
                depth_or_rung=None,
                latency_ms=round(latency_ms, 3),
            ),
        )
