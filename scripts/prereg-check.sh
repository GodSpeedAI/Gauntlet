#!/usr/bin/env bash
# prereg-check.sh — research instrumentation only. NOT a spec gate, NOT part of
# `just ruler`, and it never influences task acceptance, gates, or closure rules.
#
# Recomputes the SHA-256 of a task's frozen preregistration and compares it to the
# frozen hash recorded beside it. Non-zero exit on mismatch, on a missing
# preregistration, or on a missing hash file — silence is never agreement.
#
# This script is read-only: it never writes, rewrites, or repairs a preregistration
# or its hash file.
#
# Usage: scripts/prereg-check.sh <task-number>

set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "prereg-check.sh: usage: scripts/prereg-check.sh <task-number>" >&2
  exit 2
fi

task="$1"
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
dir="$repo_root/.agents/research/preregistrations"
prereg="$dir/task-$task.prereg.yaml"
hash_file="$dir/task-$task.prereg.sha256"

if [[ ! -f "$prereg" ]]; then
  echo "prereg-check.sh: FAIL — no preregistration at $prereg" >&2
  exit 1
fi
if [[ ! -f "$hash_file" ]]; then
  echo "prereg-check.sh: FAIL — no frozen hash at $hash_file (task $task was never frozen)" >&2
  exit 1
fi

actual="$(sha256sum "$prereg" | awk '{print $1}')"
frozen="$(awk '{print $1; exit}' "$hash_file")"

if [[ -z "$frozen" ]]; then
  echo "prereg-check.sh: FAIL — $hash_file is empty or unreadable" >&2
  exit 1
fi

if [[ "$actual" != "$frozen" ]]; then
  echo "prereg-check.sh: FAIL — task $task preregistration has changed since it was frozen." >&2
  echo "  frozen:   $frozen" >&2
  echo "  computed: $actual" >&2
  echo "  Do NOT edit the preregistration or the hash to make this pass." >&2
  echo "  Restore the exact frozen bytes; record later observations in task-$task.outcome.yaml." >&2
  exit 1
fi

echo "prereg-check.sh: PASS — task $task preregistration matches its frozen hash ($frozen)"
