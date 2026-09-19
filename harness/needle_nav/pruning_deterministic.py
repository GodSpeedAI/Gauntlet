"""
Gauntlet §15.1 Deterministic Candidate Prioritizer for EXP-N3-PRUNE.

Implements Gauntlet's real §15.1 post-gate prioritization dimensions:
(crates/gauntlet-app/src/control/select.rs):
1. SettlementAccessibility / ExpectedInformationGain: Probes producing direct settlement
   or formal verification evidence take precedence over speculative modifications.
2. Payment: Lower probe wall-clock / payment probes are preferred over costly probes.
3. Lexical Tie-Break: Deterministic tie-breaker on candidate affordance_id.

Also exposes ORDER_BASELINE (pure lexical order) as a separate descriptive comparison.
"""

from __future__ import annotations

import time
from typing import Any, Dict, List

from .port import ProviderProvenance
from .pruning_contract import CandidateAffordance, PruningHorizon, PruningRanking


class GauntletSection151Prioritizer:
    """The strongest legitimate cheap deterministic candidate prioritizer in Gauntlet."""

    def prune(self, horizon: PruningHorizon) -> PruningRanking:
        t0 = time.perf_counter()
        k = horizon.probe_budget_k

        # Score each candidate:
        # 1. Verification/Settlement capability (1 if contains check/eval/verify/forensics, else 0)
        # 2. Lowest payment: -probe_wall_clock_ms
        # 3. Lexical tie-breaker
        def priority_key(c: CandidateAffordance):
            is_verification = any(kw in c.affordance_id for kw in ["check", "eval", "verify", "linter", "forensics", "profiler", "dom"])
            # Prefer verification/information gain, then lower payment
            return (1 if is_verification else 0, -c.probe_wall_clock_ms, c.affordance_id)

        sorted_candidates = sorted(horizon.candidates, key=priority_key, reverse=True)
        selected_ids = [c.affordance_id for c in sorted_candidates[:k]]
        latency_ms = (time.perf_counter() - t0) * 1000.0

        provenance = ProviderProvenance(
            provider="deterministic_rule",
            model_id="gauntlet_section_15_1_prioritizer",
            depth_or_rung="post_gate_v1",
            latency_ms=round(latency_ms, 3),
        )

        return PruningRanking(
            ranked_candidate_ids=selected_ids,
            abstained=False,
            reported_confidence=1.0,
            strategy_latency_ms=round(latency_ms, 3),
            provider_provenance=provenance,
            telemetry={"prioritizer_class": "GauntletSection151Prioritizer"},
        )


class PureLexicalOrderBaseline:
    """Descriptive-only order baseline (arbitrary lexical order)."""

    def prune(self, horizon: PruningHorizon) -> PruningRanking:
        t0 = time.perf_counter()
        k = horizon.probe_budget_k
        selected_ids = sorted(horizon.candidate_ids)[:k]
        latency_ms = (time.perf_counter() - t0) * 1000.0

        provenance = ProviderProvenance(
            provider="order_baseline",
            model_id="pure_lexical_order",
            depth_or_rung=None,
            latency_ms=round(latency_ms, 3),
        )

        return PruningRanking(
            ranked_candidate_ids=selected_ids,
            abstained=False,
            reported_confidence=1.0,
            strategy_latency_ms=round(latency_ms, 3),
            provider_provenance=provenance,
            telemetry={"prioritizer_class": "PureLexicalOrderBaseline"},
        )
