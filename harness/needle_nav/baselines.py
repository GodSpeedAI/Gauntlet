"""
Baseline Navigators for Gauntlet Proving Experiment.

1. RandomNavigator: Uniformly samples among available candidate affordances and abstention.
2. DeterministicRuleNavigator: Gauntlet §15.1 lexicographic selector (governance filtering,
   topological dependency order, bounded-over-speculative shape).
"""

from __future__ import annotations

import random
import time
from typing import Any, Dict, List, Optional

from .port import NavigationProposal, NavigationRequest, NavigatorPort, ProviderProvenance


class RandomNavigator(NavigatorPort):
    """Uniform random selection among eligible candidates + abstention."""

    def __init__(self, seed: int = 42):
        self._rng = random.Random(seed)

    def propose(self, request: NavigationRequest) -> NavigationProposal:
        t0 = time.perf_counter()
        options = [None] + [s.name for s in request.affordance_schemas]
        choice = self._rng.choice(options)
        latency_ms = (time.perf_counter() - t0) * 1000.0

        if choice is None:
            return NavigationProposal(
                affordance_id=None,
                arguments={},
                reported_confidence=None,
                abstained=True,
                decision_note="random_abstention",
                provider_provenance=ProviderProvenance(
                    provider="random",
                    model_id="uniform_random",
                    depth_or_rung=None,
                    latency_ms=round(latency_ms, 3),
                ),
            )

        # Find schema to generate minimal valid arguments
        schema = next(s for s in request.affordance_schemas if s.name == choice)
        args: Dict[str, Any] = {}
        props = schema.parameters.get("properties", {})
        for prop_name, prop_spec in props.items():
            if "enum" in prop_spec and prop_spec["enum"]:
                args[prop_name] = self._rng.choice(prop_spec["enum"])
            elif prop_spec.get("type") == "string":
                args[prop_name] = f"random_{prop_name}"
            elif prop_spec.get("type") == "array":
                args[prop_name] = []

        return NavigationProposal(
            affordance_id=choice,
            arguments=args,
            reported_confidence=round(1.0 / len(options), 3),
            abstained=False,
            decision_note="random_selection",
            provider_provenance=ProviderProvenance(
                provider="random",
                model_id="uniform_random",
                depth_or_rung=None,
                latency_ms=round(latency_ms, 3),
            ),
        )


class DeterministicRuleNavigator(NavigatorPort):
    """Gauntlet §15.1 Lexicographic Rule Navigator.
    
    1. Filter out governance-blocked actions (authority tier mismatch, budget).
    2. Topological dependency ordering among units:
       In calculator: AK-9011 (markup) -> AK-9012 (check command) -> AK-9013 (expression eval).
    3. Prefer bounded contained action over higher blast radius.
    4. Abstain responsibly when no valid actionable move exists.
    """

    def propose(self, request: NavigationRequest) -> NavigationProposal:
        t0 = time.perf_counter()
        
        # 1. Filter candidates by allowed authority tier
        admitted_tier = request.constraints.get("authority_tier", "contained")
        eligible_schemas = []
        for s in request.affordance_schemas:
            if s.authority_tier == "irreversible" and admitted_tier != "irreversible":
                continue
            if s.authority_tier == "cross_cutting" and admitted_tier == "contained":
                continue
            eligible_schemas.append(s)

        if not eligible_schemas:
            latency_ms = (time.perf_counter() - t0) * 1000.0
            return NavigationProposal(
                affordance_id=None,
                arguments={},
                reported_confidence=1.0,
                abstained=True,
                decision_note="all_candidates_governance_blocked",
                provider_provenance=ProviderProvenance(
                    provider="deterministic_rule",
                    model_id="lexicographic_selector_v1",
                    depth_or_rung=None,
                    latency_ms=round(latency_ms, 3),
                ),
            )

        # 2. Extract open units from state summary if present
        # Gauntlet unit dependency order: AK-9011 -> AK-9012 -> AK-9013
        open_units: List[str] = []
        for u in ["AK-9011", "AK-9012", "AK-9013", "AK-951", "AK-952", "AK-954"]:
            if f"'{u}', 'open'" in request.state_summary or f'"{u}", "open"' in request.state_summary or f"{u}: open" in request.state_summary:
                open_units.append(u)

        # If state explicitly indicates all target units are settled or no work needed:
        all_settled = ("all units settled" in request.state_summary.lower() or
                       ("open units: none" in request.state_summary.lower()) or
                       (open_units == [] and "settled" in request.state_summary.lower()))

        if all_settled:
            latency_ms = (time.perf_counter() - t0) * 1000.0
            return NavigationProposal(
                affordance_id=None,
                arguments={},
                reported_confidence=1.0,
                abstained=True,
                decision_note="responsible_abstention_all_settled",
                provider_provenance=ProviderProvenance(
                    provider="deterministic_rule",
                    model_id="lexicographic_selector_v1",
                    depth_or_rung=None,
                    latency_ms=round(latency_ms, 3),
                ),
            )

        # 3. Select first open unit in dependency order
        target_unit = open_units[0] if open_units else None
        
        # Match eligible schema for target unit
        selected_schema = None
        if target_unit:
            for s in eligible_schemas:
                if target_unit in s.name or target_unit in s.description:
                    selected_schema = s
                    break
        
        if not selected_schema and eligible_schemas:
            selected_schema = eligible_schemas[0]

        latency_ms = (time.perf_counter() - t0) * 1000.0

        if not selected_schema:
            return NavigationProposal(
                affordance_id=None,
                arguments={},
                reported_confidence=1.0,
                abstained=True,
                decision_note="no_matching_eligible_affordance",
                provider_provenance=ProviderProvenance(
                    provider="deterministic_rule",
                    model_id="lexicographic_selector_v1",
                    depth_or_rung=None,
                    latency_ms=round(latency_ms, 3),
                ),
            )

        # Deterministically fill arguments
        args: Dict[str, Any] = {}
        props = selected_schema.parameters.get("properties", {})
        for prop_name, prop_spec in props.items():
            if prop_name == "unit_id" and target_unit:
                args["unit_id"] = target_unit
            elif "enum" in prop_spec and prop_spec["enum"]:
                args[prop_name] = prop_spec["enum"][0]
            elif prop_spec.get("type") == "string":
                args[prop_name] = prop_spec.get("default", "default")
            elif prop_spec.get("type") == "array":
                args[prop_name] = []

        return NavigationProposal(
            affordance_id=selected_schema.name,
            arguments=args,
            reported_confidence=1.0,
            abstained=False,
            decision_note=f"selected_{selected_schema.name}_by_dependency_order",
            provider_provenance=ProviderProvenance(
                provider="deterministic_rule",
                model_id="lexicographic_selector_v1",
                depth_or_rung=None,
                latency_ms=round(latency_ms, 3),
            ),
        )
