#!/usr/bin/env bash
# scripts/check.sh — the target-real fixture project's real check command.
#
# Exits 0 exactly when the gadget source carries the stable marker
# GADGET_MARKER_REAL_V1 (fixture unit AK-951's claim); exits 1 otherwise.
# The E2E binds the run's `test` observation facet to THIS script
# (GAUNTLET_TOOL_TEST), so the runtime's own instrument executes it over
# each dispatched workspace — a settlement over an instrument-produced
# unit demands this executed, passing facet.
set -u

marker="GADGET_MARKER_REAL_V1"
source="src/gadget.rs"

if [[ ! -f "$source" ]]; then
  echo "check: missing $source" >&2
  exit 1
fi

if grep -q -- "$marker" "$source"; then
  echo "check: $source carries $marker"
  exit 0
fi

echo "check: $source does not carry $marker" >&2
exit 1
