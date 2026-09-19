"""
Selection-Only Needle Navigator Adapter for Gauntlet / GodSpeed (EXP-N3-SPLIT).

Invariants:
- Present parameterless tool schemas (name, description, parameters={type: object, properties: {}}).
- NEVER registers callable Python functions; uses agent.complete() for selection decoding.
- Invokes agent.reset() before decoding to eliminate C-engine multi-turn session cache bleeding.
- Decodes ONLY tool selection (affordance_id) without extracting or predicting argument values.
- Maps empty tool calls to typed abstention (abstained=True, affordance_id=None).
- Records reported_confidence and runtime telemetry (RAM, latency).
"""

from __future__ import annotations

import os
import time
from typing import Any, Dict, List, Optional

from .descriptions import get_parameterless_tool_schemas
from .port import NavigationRequest, ProviderProvenance
from .split_contract import NavigationSelection

os.environ.setdefault("NEEDLE_TELEMETRY", "0")


class NeedleSelectionAdapter:
    """Selection-only adapter for Cactus Needle 3."""

    def __init__(self, generation: int = 3):
        self.generation = int(generation)
        self.model_id = f"needle-{self.generation}"
        self.depth_or_rung = "v3-20L" if self.generation == 3 else f"v{self.generation}"

    def select(self, request: NavigationRequest) -> NavigationSelection:
        import needle

        candidate_names = request.candidate_names()
        tool_schemas = get_parameterless_tool_schemas(candidate_names)

        t0 = time.perf_counter()

        # Instantiate agent with parameterless schemas
        agent = needle.Needle(
            tools=tool_schemas,
            generation=self.generation,
            auto_date=False,
        )

        # Explicitly reset session buffer to avoid cross-horizon state pollution
        if hasattr(agent, "reset"):
            agent.reset()

        response = agent.complete(text=request.state_summary)
        latency_ms = (time.perf_counter() - t0) * 1000.0

        function_calls = response.get("function_calls") or []
        if function_calls:
            chosen_call = function_calls[0]
            affordance_id = chosen_call.get("name")
            abstained = False
        else:
            affordance_id = None
            abstained = True

        reported_confidence = response.get("confidence")
        if reported_confidence is not None:
            try:
                reported_confidence = float(reported_confidence)
            except (ValueError, TypeError):
                reported_confidence = None

        rationale: Optional[str] = None
        raw_reasoning = response.get("reasoning")
        if raw_reasoning:
            rationale = str(raw_reasoning).strip()[:140]

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

        return NavigationSelection(
            affordance_id=affordance_id,
            abstained=abstained,
            reported_confidence=reported_confidence,
            rationale=rationale,
            provider_provenance=provenance,
        )
