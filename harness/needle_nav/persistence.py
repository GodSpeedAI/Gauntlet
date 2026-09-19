"""
Pre-Scoring Observation Persistence for Gauntlet Needle Experiment.

Persists every raw model inference response to disk BEFORE scoring.
Ensures that downstream analysis, metric calculation, or failures do not
require re-executing or paying for model calls again.
"""

from __future__ import annotations

import hashlib
import json
import os
from dataclasses import asdict
from typing import Any, Dict, Optional

from .port import NavigationProposal, NavigationRequest

OBSERVATIONS_DIR = os.path.join(
    os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))),
    ".agents",
    "reports",
    "task28-needle-navigation",
    "observations",
)


def compute_sha256(data: Any) -> str:
    """Deterministic canonical sha256."""
    encoded = json.dumps(data, sort_keys=True, separators=(",", ":")).encode("utf-8")
    return f"sha256:{hashlib.sha256(encoded).hexdigest()}"


class ObservationStore:
    """Thread-safe append-only persistence store for raw model outputs."""

    def __init__(self, base_dir: str = OBSERVATIONS_DIR):
        self.base_dir = base_dir
        os.makedirs(self.base_dir, exist_ok=True)

    def persist(
        self,
        request: NavigationRequest,
        proposal: NavigationProposal,
        raw_output: Optional[Dict[str, Any]] = None,
    ) -> str:
        req_data = {
            "horizon_id": request.horizon_id,
            "case_id": request.case_id,
            "state_summary": request.state_summary,
            "candidates": [s.to_tool_schema() for s in request.affordance_schemas],
            "constraints": request.constraints,
        }
        request_hash = compute_sha256(req_data)
        surface_hash = compute_sha256([s.to_tool_schema() for s in request.affordance_schemas])

        prov = proposal.provider_provenance
        filename = f"{request.horizon_id}_{prov.provider}_{prov.model_id}.json"
        filepath = os.path.join(self.base_dir, filename)

        record = {
            "horizon_id": request.horizon_id,
            "case_id": request.case_id,
            "request_hash": request_hash,
            "surface_hash": surface_hash,
            "provider": prov.provider,
            "model_id": prov.model_id,
            "depth_or_rung": prov.depth_or_rung,
            "latency_ms": prov.latency_ms,
            "reported_confidence": proposal.reported_confidence,
            "abstained": proposal.abstained,
            "affordance_id": proposal.affordance_id,
            "arguments": proposal.arguments,
            "decision_note": proposal.decision_note,
            "telemetry": prov.telemetry,
            "raw_output": raw_output or {},
        }

        with open(filepath, "w", encoding="utf-8") as f:
            json.dump(record, f, indent=2, sort_keys=True)

        return filepath
