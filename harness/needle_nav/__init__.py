"""
Needle Navigation Proving Harness for Gauntlet / GodSpeed.

ARCHIVED RESEARCH EXPERIMENT CODE — DECOMMISSIONED (2026-09-19)
FINAL DISPOSITION: REMOVE_NEEDLE
See README.md and .agents/reports/task28-pruning-experiment/audit_report.md
"""

from .port import (
    AffordanceSchema,
    NavigationProposal,
    NavigationRequest,
    NavigatorPort,
    ProviderProvenance,
)

__all__ = [
    "AffordanceSchema",
    "NavigationProposal",
    "NavigationRequest",
    "NavigatorPort",
    "ProviderProvenance",
]
