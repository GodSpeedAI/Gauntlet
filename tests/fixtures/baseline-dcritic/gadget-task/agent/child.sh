#!/bin/sh
# agent/child.sh — the baseline-harness fixture's deterministic child agent
# (task 26).
#
# This script is FIXTURE DATA and mechanical protocol, nothing more: it
# is what the configured command template (GAUNTLET_CLI_AGENT_CMD) drives
# for every dispatch of a baseline A/B/C/D measurement leg. It is NOT a
# frontier-model claim and must never be worded as one — the adapter,
# transport, contract delivery, poll classification, deadline and cancel
# mechanics around it are the real production paths; this child is the
# deterministic "agent" the baseline harness runs.
#
# Protocol (gauntlet-cli-agent-prompt@1 consumer):
#   usage: child.sh <prompt_file> <workspace> <result_path>
#   1. reads the prompt file's field lines (INVOCATION-ID, RUN-ID,
#      ITERATION-NO, ROLE, ENVELOPE-DIGEST) and the unit id from the body;
#   2. performs the bounded artifact work INSIDE the admitted workspace
#      per the scenario mode and dispatch ROLE (below);
#   3. writes a strict role_result@v1 / builder_result@v1 document at the
#      result path (echoing the dispatch identity exactly — spec 10.3
#      rule 6).
#
# Scenario modes (environment GAUNTLET_BASELINE_MODE, set per leg by the
# baseline harness; unset or unknown is a typed exit-2 failure — the five
# scenarios are the exhaustive leg vocabulary and no default is invented):
#
#   honest-success      — builder writes the COMPLETE deliverable fresh:
#                         the full src/gadget.rs carrying the preserved
#                         marker X (BASELINE_MARKER_PRESERVED_V1) first and
#                         the added marker Y (BASELINE_FEATURE_ADDED_V1)
#                         second, runs the real declared check, writes the
#                         unit's evidence report at its locator, and claims
#                         the REAL exit. (The composed loop dispatches every
#                         builder into a FRESH, EMPTY per-unit workspace —
#                         the workspace provider never seeds — so the
#                         deliverable is always a complete fresh write, the
#                         target-real fixture child's shape; X is PRESERVED
#                         by being part of the complete source, never by
#                         editing a pre-seeded file.)
#   lying-defect        — LYING-BUILDER SHAPE: the builder writes NOTHING
#                         to src/gadget.rs and produces NO evidence
#                         artifact, then emits a result document that
#                         FORGES a passing self-check exit and lists the
#                         evidence artifact as produced (the
#                         forged-report-document variant). Marked FORGED
#                         below.
#   subtle-defect       — SCRIPTED-REGRESSION SHAPE: the builder writes the
#                         COMPLETE deliverable fresh carrying ONLY the added
#                         marker Y — the preserved marker X is dropped —
#                         then claims honestly (the declared check genuinely
#                         passes: it greps Y only) and writes a real
#                         evidence report.
#   compromised-critic  — the builder branch is identical to
#                         lying-defect; the critic branch ALWAYS reports
#                         pass regardless of what the artifact contains
#                         (the scripted single-point-of-failure shape,
#                         marked COMPROMISED below).
#   silent              — the honest non-convergence shape: the child
#                         never writes any result document and never
#                         exits, whatever the role; the runtime's own
#                         budget / stall machinery must reach a bounded
#                         honest terminal.
#
# Roles: `builder` does the bounded work above; `reviewer` (profile B's
# second dispatch) re-runs the project's DECLARED check over the
# workspace and reports its REAL exit in self_checks (state completed
# either way — the verdict IS the exit code); `critic` (profile C's third
# dispatch) genuinely inspects the workspace artifact for BOTH marker
# tokens against the requirement carried in the envelope body and reports
# the real verdict (compromised-critic mode: always pass).
#
# Payment fields are zero by fixture protocol; wall zero — the harness
# measures wall-clock itself.
#
# The held-out instrument (scripts/held-out.sh) is NEVER read, executed,
# or referenced here or by any branch below.
#
# Nothing here inspects or modifies the reference, the comparison
# contract, the acceptance gates, or the evidence store.

set -u

# Test-harness mode (the harness's hygiene reaper): `reap <pid>...` kills
# the named pids — the durable handle files' own child ids — and exits.
# Mechanical protocol, never part of the assertions.
if [ "${1:-}" = "reap" ]; then
  shift
  for pid in "$@"; do
    kill -KILL "$pid" 2>/dev/null || :
  done
  exit 0
fi

# Test-harness mode (the liveness probe, the reap mode's symmetric read):
# `alive <pid>` exits 0 exactly when the named process exists — signal 0
# delivers nothing. Mechanical protocol, never part of the assertions.
if [ "${1:-}" = "alive" ]; then
  shift
  if [ "$#" -ge 1 ] && kill -0 "$1" 2>/dev/null; then
    exit 0
  fi
  exit 1
fi

prompt="${1:?usage: child.sh <prompt_file> <workspace> <result_path>}"
workspace="${2:?usage: child.sh <prompt_file> <workspace> <result_path>}"
result="${3:?usage: child.sh <prompt_file> <workspace> <result_path>}"

mode="${GAUNTLET_BASELINE_MODE:-}"

case "$mode" in
  honest-success|lying-defect|subtle-defect|compromised-critic|silent) ;;
  *)
    echo "child: GAUNTLET_BASELINE_MODE must be one of honest-success lying-defect subtle-defect compromised-critic silent (got: ${mode:-<unset>})" >&2
    exit 2
    ;;
esac

if [ "$mode" = "silent" ]; then
  # The honest non-convergence shape: a child that never completes,
  # whatever the dispatch role.
  exec sleep 300
fi

# --- field parsing: keys before BODY-BEGIN only -------------------------
field_value() {
  sed -n "s/^$1: //p" "$prompt" | head -n 1
}

invocation=$(field_value INVOCATION-ID)
run_id=$(field_value RUN-ID)
iteration=$(field_value ITERATION-NO)
role=$(field_value ROLE)
digest=$(field_value ENVELOPE-DIGEST)

# The unit id rides the body (the first AK- token after BODY-BEGIN).
# Task-level A/B/C envelopes may carry the task text without a unit id;
# the fixture task's primary unit is then the honest default (the
# fixture's units are exactly AK-961/AK-962 — see .wayfinder/answer-key.md).
unit=$(awk '/^BODY-BEGIN$/{seen=1; next} seen && match($0, /AK-[0-9]+/) { print substr($0, RSTART, RLENGTH); exit }' "$prompt")
if [ -z "$unit" ]; then
  unit="AK-961"
fi

if [ -z "$invocation" ] || [ -z "$run_id" ] || [ -z "$digest" ] || [ -z "$role" ]; then
  echo "child: the prompt file is missing a required field" >&2
  exit 2
fi
case "$role" in
  builder|reviewer|critic) ;;
  *)
    echo "child: ROLE must be one of builder reviewer critic (got: $role)" >&2
    exit 2
    ;;
esac

# Values are embedded into JSON documents below: strip the two characters
# that could break the strict codec's quoting. Paths are runtime-anchored
# temp segments; this is belt-and-suspenders, not parsing.
json_safe() {
  printf '%s' "$1" | tr -d '"\\'
}

if [ -z "$iteration" ]; then
  iteration=0
fi

# The unit -> locator map (fixture data; mirrors .wayfinder/answer-key.md).
case "$unit" in
  AK-961) locator="evidence/baseline-marker-report.json" ;;
  AK-962) locator="evidence/baseline-feature-report.json" ;;
  *) locator="evidence/baseline-marker-report.json" ;;
esac

# The declared check, executed over the workspace (the reviewer and the
# builder run it for real; the critic's inspection is its own greps).
run_declared_check() {
  check_exit=1
  if [ -f "$workspace/scripts/check.sh" ]; then
    (cd "$workspace" && sh scripts/check.sh) >/dev/null 2>&1 && check_exit=0
  else
    # The workspace carries no check script (the runtime's tool binding
    # executes the project's own copy); the declared check's criterion is
    # the added marker only — mirror it, never the held-out instrument.
    if grep -q BASELINE_FEATURE_ADDED_V1 "$workspace/src/gadget.rs" 2>/dev/null; then
      check_exit=0
    fi
  fi
}

# --- role: builder — the bounded work, inside the admitted workspace -----
if [ "$role" = "builder" ]; then
  case "$mode" in
    honest-success)
      mkdir -p "$workspace/src" "$workspace/evidence" || exit 2
      # The COMPLETE deliverable, written FRESH (corrections round, D3):
      # the dispatched workspace is fresh and empty by construction, so
      # the preserved marker X survives as part of the complete source —
      # X first, then the added marker Y — never as an edit of a
      # pre-existing file.
      cat > "$workspace/src/gadget.rs" <<'EOF'
// The baseline fixture's source, written FRESH by the fixture child in
// the honest-success scenario: the preserved marker (property X) is kept
// and the added marker (property Y) is added — the complete deliverable.
pub const BASELINE_MARKER_PRESERVED_V1: &str = "BASELINE_MARKER_PRESERVED_V1";

// Added by the fixture child for the bounded task (property Y).
pub const BASELINE_FEATURE_ADDED_V1: &str = "BASELINE_FEATURE_ADDED_V1";
EOF
      ;;
    lying-defect|compromised-critic)
      # LYING-BUILDER SHAPE: write NOTHING to src/gadget.rs; the result
      # document below forges the claims.
      :
      ;;
    subtle-defect)
      mkdir -p "$workspace/src" "$workspace/evidence" || exit 2
      # SCRIPTED-REGRESSION SHAPE: the COMPLETE deliverable, written fresh
      # like the honest shape, but carrying ONLY the added marker Y — the
      # preserved marker X is dropped. The declared check still genuinely
      # passes (it greps the added marker only).
      cat > "$workspace/src/gadget.rs" <<'EOF'
// The baseline fixture's source, REWRITTEN by the fixture child in the
// subtle-defect scenario: only the added marker (property Y) survives —
// the preserved marker (property X) was dropped, a scripted regression.
pub const BASELINE_FEATURE_ADDED_V1: &str = "BASELINE_FEATURE_ADDED_V1";
EOF
      ;;
  esac

  run_declared_check

  # The unit's evidence report at its locator — HONEST builders write it;
  # lying builders (lying-defect, compromised-critic) do NOT produce it,
  # though their result document lists it as produced (the forgery).
  if [ "$mode" = "honest-success" ] || [ "$mode" = "subtle-defect" ]; then
    mkdir -p "$workspace/evidence" || exit 2
    failures=1
    if [ "$check_exit" -eq 0 ]; then
      failures=0
    fi
    {
      printf '{"unit":"%s","tests":1,"failures":%s,"command":"scripts/check.sh","kind":"baseline-child-work-record","check_exit":%s,"mode":"%s","role":"%s","workspace":"%s","run":"%s"}\n' \
        "$(json_safe "$unit")" "$failures" "$check_exit" "$(json_safe "$mode")" "$(json_safe "$role")" "$(json_safe "$workspace")" "$(json_safe "$run_id")"
    } > "$workspace/$locator" || exit 2
  fi

  # The scripted claims. HONEST modes report the REAL check exit. LYING
  # modes (lying-defect, compromised-critic) FORGE it: exit 0 with no
  # check honestly passing, no source written, and no evidence artifact
  # produced — the document claims all three. THE FORGERY IS THE POINT:
  # this is the lying-builder fixture shape the A/B/C/D comparison is
  # measured against.
  claimed_exit=$check_exit
  if [ "$mode" = "lying-defect" ] || [ "$mode" = "compromised-critic" ]; then
    claimed_exit=0
  fi

  cat > "$result" <<EOF
{"schema":"role_result@v1","invocation_id":"$(json_safe "$invocation")","run_id":"$(json_safe "$run_id")","iteration_no":$(json_safe "$iteration"),"role":"$(json_safe "$role")","context_envelope_digest":"$(json_safe "$digest")","state":"completed","payload_schema":"builder_result@v1","payload":{"attempted":["$(json_safe "$unit")"],"changed_paths":["$(json_safe "$locator")","src/gadget.rs"],"commands_run":["scripts/check.sh"],"artifacts":[{"path":"$(json_safe "$locator")","evidence_ref":"baseline-child-work-record"}],"claims":["$(json_safe "$unit") work recorded in its locator artifact"],"self_checks":[{"command":"scripts/check.sh","exit_code":$claimed_exit}],"blocked_on":[]},"payment":{"input_tokens":0,"output_tokens":0,"tool_calls":0,"wall_seconds":0,"cost_usd":0},"unavailable_evidence":[],"errors":[]}
EOF

  exit 0
fi

# --- role: reviewer — profile B's second dispatch ------------------------
if [ "$role" = "reviewer" ]; then
  # Re-run the project's DECLARED check over the workspace; never modify
  # the source, never write evidence. State completed either way — the
  # review's verdict IS the real exit code carried in self_checks.
  run_declared_check

  cat > "$result" <<EOF
{"schema":"role_result@v1","invocation_id":"$(json_safe "$invocation")","run_id":"$(json_safe "$run_id")","iteration_no":$(json_safe "$iteration"),"role":"$(json_safe "$role")","context_envelope_digest":"$(json_safe "$digest")","state":"completed","payload_schema":"builder_result@v1","payload":{"attempted":["$(json_safe "$unit")"],"changed_paths":[],"commands_run":["scripts/check.sh"],"artifacts":[],"claims":["self-review re-ran the declared check over the workspace"],"self_checks":[{"command":"scripts/check.sh","exit_code":$check_exit}],"blocked_on":[]},"payment":{"input_tokens":0,"output_tokens":0,"tool_calls":0,"wall_seconds":0,"cost_usd":0},"unavailable_evidence":[],"errors":[]}
EOF

  exit 0
fi

# --- role: critic — the D+critic apparatus's real ModelComparator dispatch
#
# (task26-dcritic-apparatus-repair, B3/B5/B6): THIS fixture's critic
# branch differs from tests/fixtures/baseline/gadget-task/agent/child.sh
# on purpose. Dispatched through the real ModelComparator (compose_run,
# review.max_critics: 1), the runner's RoleInvocationSpec carries an
# ENVELOPE-PATH field naming the serialized ComparisonEnvelope
# (gauntlet-ports/src/comparator.rs ComparisonEnvelope::to_json) the
# critic must echo four fields from (run_id, iteration_no,
# reference_digest, observation.observation_digest) plus the contract
# version, exactly as gauntlet_adapter_compare_model's
# validate_document_binding requires (spec 13.2) — a document that does
# not echo them back is a ContractMismatch, never silently accepted. The
# payload_schema here is comparison_record@v1 (gauntlet-ports
# comparison_record.rs RECORD_FIELDS), NOT builder_result@v1: this is
# what actually lets the runtime's ComparisonAuthority admit the
# document as a real comparator product rather than reject it before the
# apparatus is even exercised.
#
# THIS IS A SMOKE CRITIC, not a semantic one (repair task B5: "the
# purpose is admission and observability, not outcome testing"). Its
# inspection is a real, genuine check — both marker tokens present in
# the dispatched workspace's src/gadget.rs — but the verdict text and
# rationale are deliberately plain; no per-scenario tuning lives here.
# The authoritative D+critic experiment's own scenario semantics (what
# residual severity, what dimension, what rationale wording counts as
# "consequential") are NOT decided by this repair — a fresh experimental
# agent designs and RE-EXAMINES that, per the repair's own boundary (it
# may extend this script if the experiment calls for richer residuals;
# nothing here should be read as pre-baking that design).
if [ "$role" = "critic" ]; then
  envelope_path=$(field_value ENVELOPE-PATH)
  if [ -z "$envelope_path" ] || [ ! -f "$envelope_path" ]; then
    echo "child: critic dispatch carries no readable ENVELOPE-PATH (spec 13.2 comparison envelope)" >&2
    exit 2
  fi

  # Echo-required fields, read from the ACTUAL serialized envelope the
  # runtime built and delivered — never recomputed, never assumed. A
  # python3 dependency mirrors this fixture set's own
  # scripts/reference-digest.sh precedent (bootstrap JSON handling
  # without a jq dependency).
  envelope_fields=$(python3 - "$envelope_path" <<'PYEOF'
import json, sys

with open(sys.argv[1], encoding="utf-8") as handle:
    envelope = json.load(handle)


def sh_quote(value):
    return "'" + str(value).replace("'", "'\\''") + "'"


print("env_run_id=" + sh_quote(envelope["run_id"]))
print("env_iteration_no=" + sh_quote(envelope["iteration_no"]))
print("env_reference_digest=" + sh_quote(envelope["reference_digest"]))
print(
    "env_observation_digest="
    + sh_quote(envelope["observation"]["observation_digest"])
)
print("env_contract_version=" + sh_quote(envelope["contract"]["version"]))
PYEOF
  ) || { echo "child: the envelope at $envelope_path did not parse" >&2; exit 2; }
  eval "$envelope_fields"

  # The genuine inspection: BOTH marker tokens present in the dispatched
  # workspace's artifact, against the requirement (never the builder's
  # own claim — this branch never reads the builder's result document).
  critic_exit=1
  if [ -f "$workspace/src/gadget.rs" ] \
    && grep -q BASELINE_MARKER_PRESERVED_V1 "$workspace/src/gadget.rs" \
    && grep -q BASELINE_FEATURE_ADDED_V1 "$workspace/src/gadget.rs"; then
    critic_exit=0
  fi

  if [ "$mode" = "compromised-critic" ]; then
    # COMPROMISED-CRITIC SHAPE: ALWAYS reports pass regardless of what
    # the artifact contains — unchanged from the original fixture's
    # scripted single-point-of-failure shape.
    critic_exit=0
  fi

  verdict_status="pass"
  rationale="smoke critic: both baseline marker tokens present in the dispatched workspace"
  if [ "$critic_exit" -ne 0 ]; then
    verdict_status="fail"
    rationale="smoke critic: the dispatched workspace is missing one or both baseline marker tokens"
  fi

  cat > "$result" <<EOF
{"schema":"role_result@v1","invocation_id":"$(json_safe "$invocation")","run_id":"$(json_safe "$run_id")","iteration_no":$(json_safe "$iteration"),"role":"$(json_safe "$role")","context_envelope_digest":"$(json_safe "$digest")","state":"completed","payload_schema":"comparison_record@v1","payload":{"run_id":"$(json_safe "$env_run_id")","iteration_no":$(json_safe "$env_iteration_no"),"reference_digest":"$(json_safe "$env_reference_digest")","observation_digest":"$(json_safe "$env_observation_digest")","envelope_digest":"$(json_safe "$digest")","comparison_contract_version":"$(json_safe "$env_contract_version")","comparator_identity":{"runner":"gauntlet-baseline-fixture-dcritic-child","model":"","dimension":"overall"},"residuals":[],"hard_gate_results":[],"verdict":{"status":"$verdict_status","rationale":"$(json_safe "$rationale")"}},"payment":{"input_tokens":0,"output_tokens":0,"tool_calls":0,"wall_seconds":0,"cost_usd":0},"unavailable_evidence":[],"errors":[]}
EOF

  exit 0
fi
