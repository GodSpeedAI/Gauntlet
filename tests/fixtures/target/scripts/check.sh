#!/usr/bin/env bash
# scripts/check.sh — the fixture target project's real check command.
#
# Exits 0 exactly when the gadget source carries the stable marker
# GADGET_MARKER_V1 (fixture unit AK-901's claim); exits 1 otherwise. The
# E2E's lying-builder leg plants a source file WITHOUT the marker, so this
# script's real exit status is the observed fact that discriminates the
# legs.
set -u

marker="GADGET_MARKER_V1"
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
