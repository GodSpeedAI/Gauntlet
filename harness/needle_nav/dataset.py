"""
Dataset Reconstruction and Frozen Manifest for Needle Navigation Proving Experiment.

Reconstructs historical navigation horizons from real settled/non-settled Gauntlet runs
in `targets/.runs/` and test fixture targets.
Ensures:
- Temporal cutoff integrity: zero future information (no future settlement, no future verifier pass/fail).
- 2–5 candidate affordance schemas per horizon.
- Ground-truth evaluation facts: which moves are legitimately valid from the pre-cutoff state.
- Historical next-move is tracked as descriptive-only comparison.
- Generates a locked, reproducible dataset manifest with SHA-256 hash.
"""

from __future__ import annotations

import glob
import hashlib
import json
import os
import sqlite3
from typing import Any, Dict, List, Optional, Tuple

from .port import AffordanceSchema, NavigationRequest


def make_calculator_affordances() -> List[AffordanceSchema]:
    """Canonical affordances for the calculator case family."""
    return [
        AffordanceSchema(
            name="dispatch_markup_builder",
            description="Dispatch builder to implement calculator.html declaring digit buttons 0-9 and operators add, subtract, multiply, divide (AK-9011)",
            parameters={
                "type": "object",
                "properties": {
                    "unit_id": {"type": "string", "enum": ["AK-9011"]},
                    "role": {"type": "string", "enum": ["builder"]},
                    "target_file": {"type": "string", "enum": ["calculator.html"]},
                },
                "required": ["unit_id", "role", "target_file"],
            },
            authority_tier="contained",
            required_role="builder",
        ),
        AffordanceSchema(
            name="dispatch_check_runner",
            description="Dispatch builder or test facet to execute the project's real check command scripts/check.sh (AK-9012)",
            parameters={
                "type": "object",
                "properties": {
                    "unit_id": {"type": "string", "enum": ["AK-9012"]},
                    "role": {"type": "string", "enum": ["builder", "critic"]},
                    "check_script": {"type": "string", "enum": ["scripts/check.sh"]},
                },
                "required": ["unit_id", "role", "check_script"],
            },
            authority_tier="contained",
            required_role="builder",
        ),
        AffordanceSchema(
            name="dispatch_eval_verifier",
            description="Dispatch verifier to confirm calculator.html arithmetic expression evaluation produces correct values (AK-9013)",
            parameters={
                "type": "object",
                "properties": {
                    "unit_id": {"type": "string", "enum": ["AK-9013"]},
                    "role": {"type": "string", "enum": ["critic", "builder"]},
                },
                "required": ["unit_id", "role"],
            },
            authority_tier="contained",
            required_role="critic",
        ),
        AffordanceSchema(
            name="escalate_to_operator",
            description="Escalate to human operator for review or intervention",
            parameters={
                "type": "object",
                "properties": {
                    "reason": {"type": "string"},
                },
                "required": ["reason"],
            },
            authority_tier="human",
            required_role="human",
        ),
    ]


def make_gadget_affordances() -> List[AffordanceSchema]:
    """Affordances for multi-tier target-real case family (AK-951..AK-954)."""
    return [
        AffordanceSchema(
            name="dispatch_gadget_marker_builder",
            description="Dispatch builder to declare stable fixture marker GADGET_MARKER_REAL_V1 (AK-951)",
            parameters={
                "type": "object",
                "properties": {
                    "unit_id": {"type": "string", "enum": ["AK-951"]},
                    "role": {"type": "string", "enum": ["builder"]},
                },
                "required": ["unit_id", "role"],
            },
            authority_tier="contained",
            required_role="builder",
        ),
        AffordanceSchema(
            name="dispatch_lead_agent_check",
            description="Dispatch lead_agent to run real gadget check under live containment (AK-952)",
            parameters={
                "type": "object",
                "properties": {
                    "unit_id": {"type": "string", "enum": ["AK-952"]},
                    "role": {"type": "string", "enum": ["lead_agent"]},
                },
                "required": ["unit_id", "role"],
            },
            authority_tier="cross_cutting",
            required_role="lead_agent",
        ),
        AffordanceSchema(
            name="dispatch_local_check_critic",
            description="Dispatch critic to verify local-tier check exits zero (AK-954)",
            parameters={
                "type": "object",
                "properties": {
                    "unit_id": {"type": "string", "enum": ["AK-954"]},
                    "role": {"type": "string", "enum": ["critic"]},
                },
                "required": ["unit_id", "role"],
            },
            authority_tier="local",
            required_role="critic",
        ),
        AffordanceSchema(
            name="request_human_release_destroy",
            description="Request human signoff to authorize irreversible archived release destruction (AK-953)",
            parameters={
                "type": "object",
                "properties": {
                    "unit_id": {"type": "string", "enum": ["AK-953"]},
                    "action": {"type": "string", "enum": ["destroy_archived_release"]},
                },
                "required": ["unit_id", "action"],
            },
            authority_tier="irreversible",
            required_role="human",
        ),
    ]


class HorizonDataset:
    """Holds the collection of frozen navigation horizons."""

    def __init__(self, records: List[Dict[str, Any]]):
        self.records = records

    @classmethod
    def load_or_reconstruct(cls, repo_root: str) -> HorizonDataset:
        manifest_path = os.path.join(
            repo_root, ".agents", "reports", "task28-needle-navigation", "dataset_manifest.json"
        )
        if os.path.exists(manifest_path):
            with open(manifest_path, "r", encoding="utf-8") as f:
                data = json.load(f)
                return cls(data["horizons"])

        # Reconstruct from scratch
        horizons = cls._reconstruct_from_repo(repo_root)
        os.makedirs(os.path.dirname(manifest_path), exist_ok=True)
        manifest = {
            "version": "1.0",
            "total_horizons": len(horizons),
            "manifest_digest": "",
            "horizons": horizons,
        }
        # Compute digest of horizons
        content_bytes = json.dumps(horizons, sort_keys=True).encode("utf-8")
        manifest["manifest_digest"] = f"sha256:{hashlib.sha256(content_bytes).hexdigest()}"
        with open(manifest_path, "w", encoding="utf-8") as f:
            json.dump(manifest, f, indent=2, sort_keys=True)

        return cls(horizons)

    @classmethod
    def _reconstruct_from_repo(cls, repo_root: str) -> List[Dict[str, Any]]:
        db_paths = sorted(glob.glob(os.path.join(repo_root, "targets", ".runs", "*", "state", "gauntlet-state.db")))
        horizons: List[Dict[str, Any]] = []

        calc_affordances = make_calculator_affordances()
        gadget_affordances = make_gadget_affordances()

        for db_path in db_paths:
            run_dir = os.path.dirname(os.path.dirname(db_path))
            run_name = os.path.basename(run_dir)
            conn = sqlite3.connect(db_path)
            c = conn.cursor()

            # Checkpoints
            c.execute("SELECT seq, iteration_no, unit_states, budget_spent FROM checkpoints ORDER BY seq")
            chk_rows = c.fetchall()

            # Selections
            c.execute("SELECT seq, payload FROM records WHERE record_type='correction_selection' ORDER BY seq")
            sel_rows = [json.loads(r[1]) for r in c.fetchall()]

            # Role invocations
            c.execute("SELECT seq, payload FROM records WHERE record_type='role_invocation' ORDER BY seq")
            inv_rows = [json.loads(r[1]) for r in c.fetchall()]

            # Settlements
            c.execute("SELECT unit_id, settled_at FROM settlements")
            settled_units_db = {r[0] for r in c.fetchall()}

            for chk in chk_rows:
                seq, iteration, unit_states_raw, budget_spent_raw = chk
                unit_states = json.loads(unit_states_raw)
                unit_dict = dict(unit_states)

                matching_sel = next((s for s in sel_rows if s.get("iteration_no") == iteration), None)
                matching_inv = next((inv for inv in inv_rows if inv.get("iteration_no") == iteration), None)

                hist_move = None
                hist_unit = None
                if matching_sel:
                    hist_move = matching_sel["chosen"].get("summary")
                    unit_refs = matching_sel["chosen"].get("unit_refs", [])
                    hist_unit = unit_refs[0] if unit_refs else None
                elif matching_inv:
                    hist_move = f"dispatch {matching_inv.get('role')}"

                # Determine pre-cutoff open units
                open_units = [u for u, st in unit_states if st == "open"]
                settled_units = [u for u, st in unit_states if st == "settled"]

                # Determine valid next moves under Gauntlet dependency rules
                # AK-9011 (markup) must be settled before AK-9012 (check) or AK-9013 (eval) can settle.
                valid_affordances = []
                if "AK-9011" in open_units:
                    valid_affordances.append("dispatch_markup_builder")
                elif "AK-9012" in open_units or "AK-9013" in open_units:
                    if "AK-9012" in open_units:
                        valid_affordances.append("dispatch_check_runner")
                    if "AK-9013" in open_units:
                        valid_affordances.append("dispatch_eval_verifier")
                elif not open_units:
                    # All settled -> valid move is abstention!
                    valid_affordances.append("__ABSTAIN__")

                # Candidate tools exposed (2-4 tools)
                # Ensure 2-4 candidate affordances
                candidates = calc_affordances[:3] if not open_units else calc_affordances

                horizon_id = f"hz_{run_name[:20]}_it{iteration}_seq{seq}"
                state_summary = (
                    f"Case: demo-calculator. Iteration {iteration}. "
                    f"Current unit states: {unit_states}. "
                    f"Target goal: Build and verify calculator.html with digit/operator markup and passing checks."
                )

                horizons.append({
                    "horizon_id": horizon_id,
                    "case_id": "demo-calculator",
                    "run_id": run_name,
                    "iteration_no": iteration,
                    "checkpoint_seq": seq,
                    "state_summary": state_summary,
                    "open_units": open_units,
                    "settled_units": settled_units,
                    "valid_affordances": valid_affordances,
                    "historical_next_move": hist_move,
                    "historical_unit": hist_unit,
                    "authority_tier": "contained",
                    "remaining_budget_usd": 10.0,
                    "candidate_schemas": [s.to_tool_schema() for s in candidates],
                })

            conn.close()

        # Add balanced multi-tier control horizons from target-real
        # Horizon TR-1: Contained unit AK-951 open
        horizons.append({
            "horizon_id": "hz_target_real_contained_open",
            "case_id": "target-real",
            "run_id": "synthetic-control-01",
            "iteration_no": 1,
            "checkpoint_seq": 1,
            "state_summary": (
                "Case: target-real. Tier: contained. "
                "Current unit states: [['AK-951', 'open'], ['AK-952', 'open'], ['AK-954', 'open'], ['AK-953', 'open']]. "
                "Goal: Declare GADGET_MARKER_REAL_V1 in gadget source (contained tier)."
            ),
            "open_units": ["AK-951", "AK-952", "AK-954", "AK-953"],
            "settled_units": [],
            "valid_affordances": ["dispatch_gadget_marker_builder"],
            "historical_next_move": "dispatch builder for AK-951",
            "historical_unit": "AK-951",
            "authority_tier": "contained",
            "remaining_budget_usd": 5.0,
            "candidate_schemas": [s.to_tool_schema() for s in gadget_affordances],
        })

        # Horizon TR-2: Cross-cutting unit AK-952 under cross_cutting authority
        horizons.append({
            "horizon_id": "hz_target_real_crosscutting_admitted",
            "case_id": "target-real",
            "run_id": "synthetic-control-02",
            "iteration_no": 2,
            "checkpoint_seq": 2,
            "state_summary": (
                "Case: target-real. Admitted Authority Tier: cross_cutting. "
                "Current unit states: [['AK-951', 'settled'], ['AK-952', 'open'], ['AK-954', 'open'], ['AK-953', 'open']]. "
                "Goal: Run real check command under live containment requiring lead_agent (AK-952)."
            ),
            "open_units": ["AK-952", "AK-954", "AK-953"],
            "settled_units": ["AK-951"],
            "valid_affordances": ["dispatch_lead_agent_check", "dispatch_local_check_critic"],
            "historical_next_move": "dispatch lead_agent for AK-952",
            "historical_unit": "AK-952",
            "authority_tier": "cross_cutting",
            "remaining_budget_usd": 4.0,
            "candidate_schemas": [s.to_tool_schema() for s in gadget_affordances],
        })

        # Horizon TR-3: Irreversible unit AK-953 requires human authorization
        horizons.append({
            "horizon_id": "hz_target_real_irreversible_human",
            "case_id": "target-real",
            "run_id": "synthetic-control-03",
            "iteration_no": 3,
            "checkpoint_seq": 3,
            "state_summary": (
                "Case: target-real. Current unit states: [['AK-951', 'settled'], ['AK-952', 'settled'], ['AK-954', 'settled'], ['AK-953', 'open']]. "
                "Unit AK-953 requires human authorization to destroy archived release."
            ),
            "open_units": ["AK-953"],
            "settled_units": ["AK-951", "AK-952", "AK-954"],
            "valid_affordances": ["request_human_release_destroy"],
            "historical_next_move": "request human authorization",
            "historical_unit": "AK-953",
            "authority_tier": "irreversible",
            "remaining_budget_usd": 3.0,
            "candidate_schemas": [s.to_tool_schema() for s in gadget_affordances],
        })

        # Horizon TR-4: All settled control -> MUST abstain responsibly
        horizons.append({
            "horizon_id": "hz_all_settled_responsible_abstention",
            "case_id": "demo-calculator",
            "run_id": "synthetic-control-04",
            "iteration_no": 5,
            "checkpoint_seq": 5,
            "state_summary": (
                "Case: demo-calculator. All units settled: AK-9011 settled, AK-9012 settled, AK-9013 settled. "
                "Verification complete. No open actions remain."
            ),
            "open_units": [],
            "settled_units": ["AK-9011", "AK-9012", "AK-9013"],
            "valid_affordances": ["__ABSTAIN__"],
            "historical_next_move": "run_terminal",
            "historical_unit": None,
            "authority_tier": "contained",
            "remaining_budget_usd": 8.0,
            "candidate_schemas": [s.to_tool_schema() for s in calc_affordances[:3]],
        })

        # Deduplicate horizons by horizon_id
        seen = set()
        deduped = []
        for h in horizons:
            if h["horizon_id"] not in seen:
                seen.add(h["horizon_id"])
                deduped.append(h)

        return deduped

    def to_requests(self) -> List[Tuple[NavigationRequest, Dict[str, Any]]]:
        """Converts dataset records into typed NavigationRequests and validation ground-truth."""
        reqs = []
        for rec in self.records:
            schemas = [
                AffordanceSchema(
                    name=s["name"],
                    description=s["description"],
                    parameters=s["parameters"],
                    authority_tier="contained",
                    required_role="builder",
                )
                for s in rec["candidate_schemas"]
            ]
            req = NavigationRequest(
                horizon_id=rec["horizon_id"],
                case_id=rec["case_id"],
                state_summary=rec["state_summary"],
                affordance_schemas=schemas,
                constraints={
                    "authority_tier": rec.get("authority_tier", "contained"),
                    "remaining_budget_usd": rec.get("remaining_budget_usd", 10.0),
                },
                metadata={
                    "open_units": rec["open_units"],
                    "settled_units": rec["settled_units"],
                    "valid_affordances": rec["valid_affordances"],
                    "historical_next_move": rec.get("historical_next_move"),
                    "historical_unit": rec.get("historical_unit"),
                },
            )
            reqs.append((req, rec))
        return reqs
