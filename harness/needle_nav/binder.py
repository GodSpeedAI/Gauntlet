"""
Neutral Deterministic Argument Binder for Gauntlet / GodSpeed (EXP-N3-SPLIT).

Invariants:
- Binder-Neutrality Invariant:
  The deterministic binder resolves arguments strictly and solely for the affordance_id supplied to it.
  It may inspect state solely to recover/validate that affordance's parameters and prerequisites.
  It may NOT compare alternatives, rank affordances, substitute another affordance, or encode
  which affordance should have been selected.
- Fails typed if required arguments cannot be uniquely bound (FAILED_MISSING_ARGUMENT, FAILED_AMBIGUOUS, FAILED_PREREQUISITE).
- NEVER fabricates unavailable or ungrounded values.
- NEVER mutates or substitutes the affordance_id.
"""

from __future__ import annotations

from typing import Any, Dict, Optional

from .split_contract import BindingResult, BindingStatus


class DeterministicArgumentBinder:
    """Neutral deterministic binder mapping (affordance_id, horizon_state) -> BindingResult."""

    def bind(
        self,
        affordance_id: Optional[str],
        horizon_state: Dict[str, Any],
    ) -> BindingResult:
        """Resolves arguments strictly for the supplied affordance_id."""
        if affordance_id is None:
            return BindingResult(
                affordance_id=None,
                status=BindingStatus.NOT_APPLICABLE,
                arguments={},
            )

        open_units = horizon_state.get("open_units") or []
        case_id = horizon_state.get("case_id", "demo-calculator")
        iteration_no = horizon_state.get("iteration_no", 0)

        # 1. dispatch_markup_builder (Calculator initial markup)
        if affordance_id == "dispatch_markup_builder":
            if "AK-9011" not in open_units:
                return BindingResult(
                    affordance_id=affordance_id,
                    status=BindingStatus.FAILED_PREREQUISITE,
                    arguments={},
                    error="Unit AK-9011 is not currently open in represented state",
                )
            return BindingResult(
                affordance_id=affordance_id,
                status=BindingStatus.SUCCESS,
                arguments={
                    "unit_id": "AK-9011",
                    "role": "builder",
                    "target_file": "calculator.html",
                },
            )

        # 2. dispatch_check_runner (Calculator check script)
        elif affordance_id == "dispatch_check_runner":
            if "AK-9012" not in open_units:
                return BindingResult(
                    affordance_id=affordance_id,
                    status=BindingStatus.FAILED_PREREQUISITE,
                    arguments={},
                    error="Unit AK-9012 is not currently open in represented state",
                )
            return BindingResult(
                affordance_id=affordance_id,
                status=BindingStatus.SUCCESS,
                arguments={
                    "unit_id": "AK-9012",
                    "role": "builder",
                    "check_script": "scripts/check.sh",
                },
            )

        # 3. dispatch_eval_verifier (Calculator evaluation verification)
        elif affordance_id == "dispatch_eval_verifier":
            if "AK-9013" not in open_units:
                return BindingResult(
                    affordance_id=affordance_id,
                    status=BindingStatus.FAILED_PREREQUISITE,
                    arguments={},
                    error="Unit AK-9013 is not currently open in represented state",
                )
            return BindingResult(
                affordance_id=affordance_id,
                status=BindingStatus.SUCCESS,
                arguments={
                    "unit_id": "AK-9013",
                    "role": "critic",
                },
            )

        # 4. escalate_to_operator (Human escalation)
        elif affordance_id == "escalate_to_operator":
            reason = f"Operator escalation at iteration {iteration_no} for case {case_id}"
            return BindingResult(
                affordance_id=affordance_id,
                status=BindingStatus.SUCCESS,
                arguments={"reason": reason},
            )

        # 5. dispatch_gadget_marker_builder (Target-Real contained builder)
        elif affordance_id == "dispatch_gadget_marker_builder":
            if "AK-951" not in open_units:
                return BindingResult(
                    affordance_id=affordance_id,
                    status=BindingStatus.FAILED_PREREQUISITE,
                    arguments={},
                    error="Unit AK-951 is not open in represented state",
                )
            return BindingResult(
                affordance_id=affordance_id,
                status=BindingStatus.SUCCESS,
                arguments={
                    "unit_id": "AK-951",
                    "role": "builder",
                },
            )

        # 6. dispatch_lead_agent_check (Target-Real cross-cutting check)
        elif affordance_id == "dispatch_lead_agent_check":
            if "AK-952" not in open_units:
                return BindingResult(
                    affordance_id=affordance_id,
                    status=BindingStatus.FAILED_PREREQUISITE,
                    arguments={},
                    error="Unit AK-952 is not open in represented state",
                )
            return BindingResult(
                affordance_id=affordance_id,
                status=BindingStatus.SUCCESS,
                arguments={
                    "unit_id": "AK-952",
                    "role": "lead_agent",
                },
            )

        # 7. dispatch_local_check_critic (Target-Real local critic check)
        elif affordance_id == "dispatch_local_check_critic":
            if "AK-954" not in open_units:
                return BindingResult(
                    affordance_id=affordance_id,
                    status=BindingStatus.FAILED_PREREQUISITE,
                    arguments={},
                    error="Unit AK-954 is not open in represented state",
                )
            return BindingResult(
                affordance_id=affordance_id,
                status=BindingStatus.SUCCESS,
                arguments={
                    "unit_id": "AK-954",
                    "role": "critic",
                },
            )

        # 8. request_human_release_destroy (Target-Real irreversible signoff)
        elif affordance_id == "request_human_release_destroy":
            if "AK-953" not in open_units:
                return BindingResult(
                    affordance_id=affordance_id,
                    status=BindingStatus.FAILED_PREREQUISITE,
                    arguments={},
                    error="Unit AK-953 is not open in represented state",
                )
            return BindingResult(
                affordance_id=affordance_id,
                status=BindingStatus.SUCCESS,
                arguments={
                    "unit_id": "AK-953",
                    "action": "destroy_archived_release",
                },
            )

        # Unknown or unmodeled affordance
        return BindingResult(
            affordance_id=affordance_id,
            status=BindingStatus.FAILED_MISSING_ARGUMENT,
            arguments={},
            error=f"Unrecognized affordance {affordance_id} cannot be deterministically bound",
        )
