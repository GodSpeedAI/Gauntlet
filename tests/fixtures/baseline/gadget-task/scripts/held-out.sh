#!/bin/sh
# scripts/held-out.sh — the baseline-harness fixture's GROUND TRUTH
# instrument (task 26). FIXTURE DATA.
#
# usage: held-out.sh <workspace>
#
# Exits 0 exactly when <workspace>/src/gadget.rs carries BOTH the
# preserved marker BASELINE_MARKER_PRESERVED_V1 (property X) AND the
# added marker BASELINE_FEATURE_ADDED_V1 (property Y) — i.e. the full
# requirement held_out correctness; exits 1 otherwise.
#
# Run ONLY by the baseline harness as ground truth over each leg's final
# artifact. NEVER referenced by any child, prompt, or the declared check
# (scripts/check.sh greps property Y only — its incompleteness is the
# baseline's measured variable). No child branch reads or executes this
# script; the deterministic children inspect the artifact directly.
set -u

workspace="${1:?usage: held-out.sh <workspace>}"

source="$workspace/src/gadget.rs"
preserved="BASELINE_MARKER_PRESERVED_V1"
added="BASELINE_FEATURE_ADDED_V1"

if [ ! -f "$source" ]; then
  echo "held-out: missing $source" >&2
  exit 1
fi

if grep -q -- "$preserved" "$source" && grep -q -- "$added" "$source"; then
  echo "held-out: $source carries both $preserved and $added"
  exit 0
fi

if ! grep -q -- "$preserved" "$source"; then
  echo "held-out: $source does not carry $preserved" >&2
fi
if ! grep -q -- "$added" "$source"; then
  echo "held-out: $source does not carry $added" >&2
fi
exit 1
