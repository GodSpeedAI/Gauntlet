"""
Needle Navigator Adapter for Gauntlet / GodSpeed.

Adapts Cactus Compute Needle 2 and Needle 3 to the provider-neutral NavigatorPort.
Invariants:
- NEVER registers callable Python functions; supplies only raw JSON tool schemas.
- NEVER invokes agent.run(); uses agent.complete() for proposal-only decoding.
- Captures reported_confidence as raw model output (unverified calibration).
- Captures optional decision_note (brief summary, no CoT requirement).
- Extracts adapter-specific telemetry (RAM, TPS) into ProviderProvenance.
"""

from __future__ import annotations

import os
import time
from typing import Any, Dict, List, Optional

from .port import NavigationProposal, NavigationRequest, NavigatorPort, ProviderProvenance

# Disable telemetry by default to keep runs quiet and hermetic
os.environ.setdefault("NEEDLE_TELEMETRY", "0")


class NeedleNavigatorAdapter(NavigatorPort):
    """Adapter for Cactus Needle tool calling models."""

    def __init__(self, generation: int = 3):
        self.generation = int(generation)
        if self.generation == 2:
            self.model_id = "needle-2"
            self.depth_or_rung = "v2-8L"
        elif self.generation == 3:
            self.model_id = "needle-3"
            self.depth_or_rung = "v3-20L"
        else:
            self.model_id = f"needle-{self.generation}"
            self.depth_or_rung = f"v{self.generation}"

    def propose(self, request: NavigationRequest) -> NavigationProposal:
        import needle

        tool_schemas: List[Dict[str, Any]] = [
            schema.to_tool_schema() for schema in request.affordance_schemas
        ]

        t0 = time.perf_counter()
        
        # Instantiate agent with schema dicts only (no executable functions)
        agent = needle.Needle(
            tools=tool_schemas,
            generation=self.generation,
            auto_date=False,
        )

        # Low-level complete: decoding constrained by tool schemas
        response = agent.complete(text=request.state_summary)
        latency_ms = (time.perf_counter() - t0) * 1000.0

        function_calls = response.get("function_calls") or []
        if function_calls:
            call = function_calls[0]
            affordance_id = call.get("name")
            arguments = call.get("arguments") or {}
            abstained = False
        else:
            affordance_id = None
            arguments = {}
            abstained = True

        reported_confidence = response.get("confidence")
        if reported_confidence is not None:
            try:
                reported_confidence = float(reported_confidence)
            except (ValueError, TypeError):
                reported_confidence = None

        decision_note: Optional[str] = None
        raw_reasoning = response.get("reasoning")
        if raw_reasoning:
            decision_note = str(raw_reasoning).strip()[:140]

        telemetry = {
            "peak_ram_mb": response.get("peak_ram_mb"),
            "decode_tps": response.get("decode_tps"),
            "prefill_tps": response.get("prefill_tps"),
            "raw_type": response.get("type"),
        }

        provenance = ProviderProvenance(
            provider="needle",
            model_id=self.model_id,
            depth_or_rung=self.depth_or_rung,
            latency_ms=round(latency_ms, 2),
            telemetry=telemetry,
        )

        return NavigationProposal(
            affordance_id=affordance_id,
            arguments=arguments,
            reported_confidence=reported_confidence,
            abstained=abstained,
            decision_note=decision_note,
            provider_provenance=provenance,
        )
