#!/bin/sh
# agent/child.sh — the target-real fixture's deterministic child agent.
#
# This script is FIXTURE DATA and mechanical protocol, nothing more: it
# is what the configured command template (GAUNTLET_CLI_AGENT_CMD) drives
# for every dispatch of the task-25 E2E. It is NOT a frontier-model claim
# and must never be worded as one — the adapter, transport, contract
# delivery, poll classification, deadline and cancel mechanics around it
# are the real production paths; this child is the deterministic "agent"
# a CI-practical real-runner proof runs.
#
# Protocol (gauntlet-cli-agent-prompt@1 consumer):
#   usage: child.sh <prompt_file> <workspace> <result_path>
#   1. reads the prompt file's field lines (INVOCATION-ID, RUN-ID,
#      ITERATION-NO, ROLE, ENVELOPE-DIGEST) and the unit id from the body;
#   2. performs the bounded artifact writes INSIDE the admitted workspace
#      (the marker source the project's check demands, plus the unit's
#      evidence artifact at its locator, embedding what the child observes
#      of its own confinement);
#   3. writes a strict role_result@v1 / builder_result@v1 document at the
#      result path (echoing the dispatch identity exactly — spec 10.3
#      rule 6).
#
# Test modes (environment GAUNTLET_CHILD_MODE, set per leg by the E2E):
#   silent — the honest non-convergence shape: the child never writes any
#            result document and never exits; the runtime's own budget /
#            stall machinery must reach a bounded honest terminal.
#   poison — the R16-F2 hostile-child shape: the child plants a DIRECTORY
#            at its own contract path (so the contract read is a typed
#            transport error forever) and stays alive, ignoring SIGTERM.
#
# Nothing here inspects or modifies the reference, the comparison
# contract, the acceptance gates, or the evidence store.

set -u

# Test-harness mode (the E2E's hygiene reaper): `reap <pid>...` kills the
# named pids — the durable handle files' own child ids — and exits.
# Mechanical protocol, never part of the assertions.
if [ "${1:-}" = "reap" ]; then
  shift
  for pid in "$@"; do
    kill -KILL "$pid" 2>/dev/null || :
  done
  exit 0
fi

# Test-harness mode (the corrections-round-1 liveness probe, the reap
# mode's symmetric read): `alive <pid>` exits 0 exactly when the named
# process exists — signal 0 delivers nothing. Mechanical protocol, never
# part of the assertions.
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

mode="${GAUNTLET_CHILD_MODE:-}"

if [ "$mode" = "silent" ]; then
  # The honest non-convergence shape: a child that never completes.
  exec sleep 300
fi

if [ "$mode" = "poison" ]; then
  # The R16-F2 hostile-child shape: a directory AT the contract path (the
  # contract read becomes a typed transport error forever — it is never
  # NotFound and never readable), and a child that stays alive while
  # ignoring the graceful stop signal. The runtime's own bounded
  # machinery (transport stop escalation, process death with the driver)
  # retires it; this script never exits on its own.
  mkdir -p "$result" 2>/dev/null || :
  trap - TERM
  trap '' TERM
  while :; do
    sleep 1 || exit 0
  done
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
unit=$(awk '/^BODY-BEGIN$/{seen=1; next} seen && match($0, /AK-[0-9]+/) { print substr($0, RSTART, RLENGTH); exit }' "$prompt")

if [ -z "$invocation" ] || [ -z "$run_id" ] || [ -z "$digest" ] || [ -z "$unit" ]; then
  echo "child: the prompt file is missing a required field" >&2
  exit 2
fi

# --- the bounded work, inside the admitted workspace --------------------
# Values are embedded into JSON documents below: strip the two characters
# that could break the strict codec's quoting. Paths are runtime-anchored
# temp segments; this is belt-and-suspenders, not parsing.
json_safe() {
  printf '%s' "$1" | tr -d '"\\'
}

# The unit -> locator map (fixture data; mirrors .wayfinder/answer-key.md).
case "$unit" in
  AK-951) locator="evidence/real-marker-report.json" ;;
  AK-952) locator="evidence/real-check-report.json" ;;
  AK-954) locator="evidence/real-local-report.json" ;;
  *) locator="evidence/real-other-report.json" ;;
esac

# The work itself: the marker source the project's real check demands,
# written into THIS dispatch's workspace.
mkdir -p "$workspace/src" "$workspace/evidence" || exit 2
cat > "$workspace/src/gadget.rs" <<'EOF'
// The target-real fixture's marker source, written by the deterministic
// child agent into its admitted workspace.
pub const GADGET_MARKER_REAL_V1: &str = "GADGET_MARKER_REAL_V1";
EOF

# The project's real check, executed by the child over its own work.
check_exit=1
if [ -f "$workspace/scripts/check.sh" ]; then
  (cd "$workspace" && sh scripts/check.sh) >/dev/null 2>&1 && check_exit=0
else
  # The workspace carries no check script (the runtime's tool binding
  # executes the project's own copy); grep the written source directly.
  if grep -q GADGET_MARKER_REAL_V1 "$workspace/src/gadget.rs"; then
    check_exit=0
  fi
fi

# What the child observes of its own containment (the E2E asserts the
# confined tokens from the durable evidence store):
#   pid1      — /proc/1/comm inside a --unshare-pid sandbox is the
#               sandbox itself ("bwrap"); on the host it is the host init.
#   tmp-write — the baseline binds / read-only, so a write outside the
#               writable workspace root is denied; on the host it is not.
pid1="unknown"
if [ -r /proc/1/comm ]; then
  pid1=$(cat /proc/1/comm)
fi
tmp_probe="${TMPDIR:-/tmp}/gauntlet-target-real-child-probe.$$"
tmp_write="denied"
if touch "$tmp_probe" 2>/dev/null; then
  tmp_write="ok"
  rm -f "$tmp_probe"
fi
# The child's own mechanical verdict from the two observed facts: the
# sandbox is pid 1 of the process namespace AND out-of-root writes are
# denied. Anything else is the uncontained shape.
if [ "$pid1" = "bwrap" ] && [ "$tmp_write" = "denied" ]; then
  confinement="confined"
else
  confinement="uncontained"
fi
confinement="confinement=$confinement pid1=$(json_safe "$pid1") tmp-write=$tmp_write workspace-write=ok"

artifact="$workspace/$locator"
failures=1
if [ "$check_exit" -eq 0 ]; then
  failures=0
fi
{
  printf '{"unit":"%s","tests":1,"failures":%s,"command":"scripts/check.sh","kind":"child-work-record","check_exit":%s,"confinement":"%s","workspace":"%s","run":"%s"}\n' \
    "$(json_safe "$unit")" "$failures" "$check_exit" "$confinement" "$(json_safe "$workspace")" "$(json_safe "$run_id")"
} > "$artifact" || exit 2

# --- the result contract (role_result@v1 / builder_result@v1) -----------
cat > "$result" <<EOF
{"schema":"role_result@v1","invocation_id":"$(json_safe "$invocation")","run_id":"$(json_safe "$run_id")","iteration_no":$(json_safe "$iteration"),"role":"$(json_safe "$role")","context_envelope_digest":"$(json_safe "$digest")","state":"completed","payload_schema":"builder_result@v1","payload":{"attempted":["$(json_safe "$unit")"],"changed_paths":["$(json_safe "$locator")","src/gadget.rs"],"commands_run":["scripts/check.sh"],"artifacts":[{"path":"$(json_safe "$locator")","evidence_ref":"child-work-record"}],"claims":["$(json_safe "$unit") work recorded in its locator artifact"],"self_checks":[{"command":"scripts/check.sh","exit_code":$check_exit}],"blocked_on":[]},"payment":{"input_tokens":0,"output_tokens":0,"tool_calls":0,"wall_seconds":0,"cost_usd":0},"unavailable_evidence":[],"errors":[]}
EOF

exit 0
