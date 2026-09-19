"""
Needle Candidate-Pruning Adapter for Gauntlet / GodSpeed (EXP-N3-PRUNE).

Implements the frozen Top-K Needle Protocol:
1. Presents surviving candidate affordances with parameterless tool schemas.
2. Invokes agent.complete().
3. If response contains >= k unique function calls: takes the first k unique candidate IDs.
4. If response contains 1 call (when k=2): takes candidate 1, removes it from candidate set,
   invokes agent.reset(), and performs a second selection over the remaining (N-1) candidates.
5. If response contains 0 calls: records typed abstention.
"""

from __future__ import annotations

import os
import time
from typing import Any, Dict, List, Optional

from .port import ProviderProvenance
from .pruning_contract import CandidateAffordance, PruningHorizon, PruningRanking

os.environ.setdefault("NEEDLE_TELEMETRY", "0")


class NeedlePruningAdapter:
    """Adapter for Cactus Needle 3 candidate pruning and prioritization."""

    def __init__(self, generation: int = 3):
        self.generation = int(generation)
        self.model_id = f"needle-{self.generation}"
        self.depth_or_rung = "v3-20L" if self.generation == 3 else f"v{self.generation}"

    def prune(self, horizon: PruningHorizon) -> PruningRanking:
        import needle

        k = horizon.probe_budget_k
        candidates = horizon.candidates
        candidate_ids = horizon.candidate_ids

        tool_schemas = [
            {
                "name": c.affordance_id,
                "description": c.description,
                "parameters": {"type": "object", "properties": {}},
            }
            for c in candidates
        ]

        t0 = time.perf_counter()

        # Step 1: Initial invocation
        agent = needle.Needle(
            tools=tool_schemas,
            generation=self.generation,
            auto_date=False,
        )
        if hasattr(agent, "reset"):
            agent.reset()

        prompt_text = (
            f"{horizon.state_summary} "
            f"Select the top {k} candidate affordances to probe."
        )

        response = agent.complete(text=prompt_text)
        initial_latency_ms = (time.perf_counter() - t0) * 1000.0

        function_calls = response.get("function_calls") or []
        selected_ids: List[str] = []
        for call in function_calls:
            name = call.get("name")
            if name in candidate_ids and name not in selected_ids:
                selected_ids.append(name)
            if len(selected_ids) == k:
                break

        reported_confidence = response.get("confidence")
        if reported_confidence is not None:
            try:
                reported_confidence = float(reported_confidence)
            except (ValueError, TypeError):
                reported_confidence = None

        total_latency_ms = initial_latency_ms

        # Step 2: Fallback if model returned only 1 candidate and k=2 requested
        if len(selected_ids) == 1 and k > 1 and len(candidate_ids) > 1:
            first_choice = selected_ids[0]
            remaining_schemas = [s for s in tool_schemas if s["name"] != first_choice]
            t1 = time.perf_counter()
            agent2 = needle.Needle(
                tools=remaining_schemas,
                generation=self.generation,
                auto_date=False,
            )
            if hasattr(agent2, "reset"):
                agent2.reset()
            prompt_step2 = (
                f"{horizon.state_summary} "
                f"Candidate '{first_choice}' has been selected. Select the next candidate affordance to probe."
            )
            resp2 = agent2.complete(text=prompt_step2)
            total_latency_ms += (time.perf_counter() - t1) * 1000.0
            calls2 = resp2.get("function_calls") or []
            for call in calls2:
                name = call.get("name")
                if name in candidate_ids and name not in selected_ids:
                    selected_ids.append(name)
                    break

        abstained = len(selected_ids) == 0

        telemetry = {
            "peak_ram_mb": response.get("peak_ram_mb"),
            "decode_tps": response.get("decode_tps"),
            "prefill_tps": response.get("prefill_tps"),
            "function_calls_count": len(function_calls),
            "step2_used": len(selected_ids) > 1 and len(function_calls) == 1,
        }

        provenance = ProviderProvenance(
            provider="needle",
            model_id=self.model_id,
            depth_or_rung=self.depth_or_rung,
            latency_ms=round(total_latency_ms, 2),
            telemetry=telemetry,
        )

        return PruningRanking(
            ranked_candidate_ids=selected_ids,
            abstained=abstained,
            reported_confidence=reported_confidence,
            strategy_latency_ms=round(total_latency_ms, 2),
            provider_provenance=provenance,
            telemetry=telemetry,
        )
