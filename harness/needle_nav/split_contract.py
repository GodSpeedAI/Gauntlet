"""
Split-Contract Navigation Types for Gauntlet / GodSpeed (EXP-N3-SPLIT).

Decomposes the navigation interface into:
1. NavigationSelection: Affordance selection only (affordance_id, abstained, confidence, rationale).
2. BindingResult: Deterministic argument binding from represented horizon state.
3. SelectionValidity: VALID, INVALID, COUNTERFACTUAL_UNKNOWN.
4. IncrementalMatrix: both_valid, needle_only_valid, rule_only_valid, neither_valid.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from enum import Enum
from typing import Any, Dict, List, Optional

from .port import ProviderProvenance


class SelectionValidity(str, Enum):
    """Classification of affordance selection validity."""
    VALID = "VALID"
    INVALID = "INVALID"
    COUNTERFACTUAL_UNKNOWN = "COUNTERFACTUAL_UNKNOWN"


class BindingStatus(str, Enum):
    """Status of deterministic argument binding."""
    SUCCESS = "SUCCESS"
    FAILED_MISSING_ARGUMENT = "FAILED_MISSING_ARGUMENT"
    FAILED_AMBIGUOUS = "FAILED_AMBIGUOUS"
    FAILED_PREREQUISITE = "FAILED_PREREQUISITE"
    NOT_APPLICABLE = "NOT_APPLICABLE"


@dataclass(frozen=True)
class NavigationSelection:
    """The candidate affordance selected by the navigator (parameter-free).

    Invariants:
    - Emits NO arguments.
    - `reported_confidence` is descriptive model output.
    - `abstained` is True if navigator chooses not to select any tool.
    """
    affordance_id: Optional[str]
    abstained: bool
    reported_confidence: Optional[float]
    rationale: Optional[str]
    provider_provenance: ProviderProvenance

    def is_in_candidates(self, allowed_candidates: List[str]) -> bool:
        if self.abstained or self.affordance_id is None:
            return True
        return self.affordance_id in allowed_candidates


@dataclass(frozen=True)
class BindingResult:
    """Outcome of deterministic argument binding for a chosen affordance.

    Invariants:
    - Resolves arguments strictly for the supplied affordance_id.
    - Never changes the affordance_id.
    - Never fabricates missing or ambiguous values.
    """
    affordance_id: Optional[str]
    status: BindingStatus
    arguments: Dict[str, Any] = field(default_factory=dict)
    error: Optional[str] = None

    @property
    def is_success(self) -> bool:
        return self.status == BindingStatus.SUCCESS


@dataclass(frozen=True)
class SplitHorizonEvaluation:
    """Full evaluation record for a single horizon under the split contract."""
    horizon_id: str
    case_id: str
    selection: NavigationSelection
    selection_validity: SelectionValidity
    historical_move_agreement: bool  # descriptive only
    binding_result: BindingResult
    validation_passed: bool
    authority_admitted: bool
    full_valid_next_move: bool


@dataclass
class IncrementalValueMatrix:
    """Comparison matrix between Needle 3 and §15.1 Deterministic Rule baseline."""
    both_valid: int = 0
    needle_only_valid: int = 0
    rule_only_valid: int = 0
    neither_valid: int = 0

    def to_dict(self) -> Dict[str, int]:
        return {
            "both_valid": self.both_valid,
            "needle_only_valid": self.needle_only_valid,
            "rule_only_valid": self.rule_only_valid,
            "neither_valid": self.neither_valid,
        }
