"""
Provider-Neutral Navigation Port for Gauntlet / GodSpeed.

This module defines the abstract interface for System-1 navigation proposals.
It is strictly decoupled from any model provider (Needle, Qwen, DeepSeek, rule-based).
The core contract:
    NavigatorPort: NavigationRequest -> NavigationProposal
"""

from __future__ import annotations

from abc import ABC, abstractmethod
from dataclasses import dataclass, field
from typing import Any, Dict, List, Optional


@dataclass(frozen=True)
class AffordanceSchema:
    """A bounded candidate affordance presented to the navigator."""
    name: str
    description: str
    parameters: Dict[str, Any]
    authority_tier: str = "contained"
    required_role: str = "builder"

    def to_tool_schema(self) -> Dict[str, Any]:
        """Convert to standard JSON tool calling schema."""
        return {
            "name": self.name,
            "description": self.description,
            "parameters": self.parameters,
        }


@dataclass(frozen=True)
class NavigationRequest:
    """A bounded horizon presented to the navigator for tool proposal."""
    horizon_id: str
    case_id: str
    state_summary: str
    affordance_schemas: List[AffordanceSchema]
    constraints: Dict[str, Any] = field(default_factory=dict)
    metadata: Dict[str, Any] = field(default_factory=dict)

    def candidate_names(self) -> List[str]:
        return [s.name for s in self.affordance_schemas]


@dataclass(frozen=True)
class ProviderProvenance:
    """Audit provenance for the navigation proposal."""
    provider: str
    model_id: str
    depth_or_rung: Optional[str]
    latency_ms: float
    telemetry: Dict[str, Any] = field(default_factory=dict)


@dataclass(frozen=True)
class NavigationProposal:
    """The candidate move proposed by the navigator.
    
    Invariants:
    - This is a proposal/evidence, NEVER execution permission.
    - `reported_confidence` is descriptive model output; calibration is evaluated empirically.
    - `decision_note` is an optional brief summary; no hidden chain-of-thought is required.
    """
    affordance_id: Optional[str]
    arguments: Dict[str, Any]
    reported_confidence: Optional[float]
    abstained: bool
    decision_note: Optional[str]
    provider_provenance: ProviderProvenance

    def is_valid_candidate(self, allowed_candidates: List[str]) -> bool:
        if self.abstained:
            return True
        return self.affordance_id in allowed_candidates


class NavigatorPort(ABC):
    """Abstract navigation port."""

    @abstractmethod
    def propose(self, request: NavigationRequest) -> NavigationProposal:
        """Produce a proposed move for the given horizon."""
        pass
