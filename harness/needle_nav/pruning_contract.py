"""
Candidate Pruning & Search Efficiency Contract for Gauntlet / GodSpeed (EXP-N3-PRUNE).

Defines:
- ConsequenceClassification: ADVANCES, REDUNDANT, DEAD_END, COSTLY_ADVANCE, COUNTERFACTUAL_UNKNOWN
- CandidateAffordance: represented candidate surviving hard gates with measured probe payment and consequence
- PruningHorizon: ambiguous state where N >= 3 candidates survive hard gates and no unique rule dictates the move
- PruningRanking: ordered top-k candidate IDs produced by a strategy
- Exact analytical random expectation calculator for any (N, k, U)
"""

from __future__ import annotations

import itertools
from dataclasses import dataclass, field
from enum import Enum
from typing import Any, Dict, List, Optional, Set, Tuple

from .port import ProviderProvenance


class ConsequenceClassification(str, Enum):
    """Consequence of probing a candidate from a frozen horizon state."""
    ADVANCES = "ADVANCES"              # Produces required evidence / state transition advancing route
    REDUNDANT = "REDUNDANT"            # Valid execution but produces no new state/evidence
    DEAD_END = "DEAD_END"              # Valid/admissible but does not advance route
    COSTLY_ADVANCE = "COSTLY_ADVANCE"  # Advances but with materially higher payment (>2x baseline)
    COUNTERFACTUAL_UNKNOWN = "COUNTERFACTUAL_UNKNOWN"


@dataclass(frozen=True)
class CandidateAffordance:
    """A candidate affordance surviving all deterministic hard gates."""
    affordance_id: str
    description: str
    consequence: ConsequenceClassification
    probe_wall_clock_ms: float
    authority_tier: str = "contained"
    required_role: str = "builder"

    @property
    def is_useful(self) -> bool:
        """A candidate is useful if it ADVANCES or is a COSTLY_ADVANCE."""
        return self.consequence in [
            ConsequenceClassification.ADVANCES,
            ConsequenceClassification.COSTLY_ADVANCE,
        ]

    @property
    def is_advances(self) -> bool:
        return self.consequence == ConsequenceClassification.ADVANCES


@dataclass(frozen=True)
class PruningHorizon:
    """An ambiguous route-search horizon where N >= 3 candidates survive hard gates."""
    horizon_id: str
    family: str
    state_summary: str
    candidates: List[CandidateAffordance]
    probe_budget_k: int = 2
    state_hash: Optional[str] = None

    @property
    def candidate_ids(self) -> List[str]:
        return [c.affordance_id for c in self.candidates]

    @property
    def useful_candidate_ids(self) -> Set[str]:
        return {c.affordance_id for c in self.candidates if c.is_useful}

    @property
    def advances_candidate_ids(self) -> Set[str]:
        return {c.affordance_id for c in self.candidates if c.is_advances}

    def get_candidate(self, affordance_id: str) -> Optional[CandidateAffordance]:
        return next((c for c in self.candidates if c.affordance_id == affordance_id), None)


@dataclass(frozen=True)
class PruningRanking:
    """Ordered top-k candidate IDs produced by a strategy."""
    ranked_candidate_ids: List[str]
    abstained: bool
    reported_confidence: Optional[float]
    strategy_latency_ms: float
    provider_provenance: ProviderProvenance
    telemetry: Dict[str, Any] = field(default_factory=dict)


@dataclass(frozen=True)
class HorizonPruningEvaluation:
    """Evaluation result of a pruning strategy on a single horizon."""
    horizon_id: str
    candidate_count_n: int
    probe_budget_k: int
    selected_candidate_ids: List[str]
    useful_candidates_count: int
    retained_useful_count: int
    useful_recall_at_k: float
    horizon_success_at_k: bool
    retained_advances_count: int
    total_advances_count: int
    diversity_retention_ratio: float
    probes_avoided: int
    probe_wall_clock_avoided_ms: float
    strategy_latency_ms: float
    net_wall_clock_saved_ms: float


def compute_exact_random_expectation(
    horizon: PruningHorizon,
) -> Tuple[float, float, List[Dict[str, Any]]]:
    """Analytically enumerates all C(N, k) combinations to find exact random expectation.

    Returns:
        (expected_useful_recall, expected_horizon_success, all_combinations_data)
    """
    n = len(horizon.candidates)
    k = min(horizon.probe_budget_k, n)
    useful_set = horizon.useful_candidate_ids
    total_useful = len(useful_set)

    all_combs = list(itertools.combinations(horizon.candidate_ids, k))
    total_combs = len(all_combs)

    if total_combs == 0 or total_useful == 0:
        return 0.0, 0.0, []

    recall_sum = 0.0
    success_sum = 0.0
    comb_records = []

    for comb in all_combs:
        retained_useful = len(set(comb) & useful_set)
        rec = retained_useful / total_useful
        suc = 1.0 if retained_useful > 0 else 0.0
        recall_sum += rec
        success_sum += suc
        comb_records.append({
            "combination": list(comb),
            "retained_useful": retained_useful,
            "useful_recall": rec,
            "horizon_success": suc == 1.0,
        })

    expected_recall = round(recall_sum / total_combs, 4)
    expected_success = round(success_sum / total_combs, 4)
    return expected_recall, expected_success, comb_records
