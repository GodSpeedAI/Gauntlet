#!/usr/bin/env bash
# verify-ruler.sh — the I1 tripwire (spec §9.4). Recomputes the answer-key and
# manifest digests with scripts/reference-digest.sh and compares them against the
# frozen record in .gauntlet/reference.lock. Non-zero exit on any mismatch, on a
# missing lock file, or on a missing/unreadable source file — silence is never
# treated as agreement.
#
# Wired as `just ruler`. Per AGENTS.md and the plan: if this ever fails, that is a
# higher-priority problem than whatever task is in flight. Stop, diagnose, report —
# do not proceed past a failing ruler check, and never "fix" it by editing the lock
# file or the frozen answer key to match a diverged run.
#
# Usage: scripts/verify-ruler.sh [--repo-root <path>]

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
lock_path="$repo_root/.gauntlet/reference.lock"
digest_tool="$repo_root/scripts/reference-digest.sh"

if [[ ! -f "$lock_path" ]]; then
  echo "verify-ruler.sh: FAIL — no lock file at $lock_path (ruler was never frozen)" >&2
  exit 1
fi

if [[ ! -x "$digest_tool" ]]; then
  echo "verify-ruler.sh: FAIL — digest tool missing or not executable: $digest_tool" >&2
  exit 1
fi

read_lock_field() {
  python3 -c "import json,sys; print(json.load(open(sys.argv[1]))[sys.argv[2]])" "$lock_path" "$1"
}

answer_key_path="$repo_root/$(read_lock_field answer_key_path)"
manifest_path="$repo_root/$(read_lock_field manifest_path)"
expected_ak_digest="$(read_lock_field answer_key_digest)"
expected_manifest_digest="$(read_lock_field manifest_digest)"

status=0

if [[ ! -f "$answer_key_path" ]]; then
  echo "verify-ruler.sh: FAIL — answer key missing at $answer_key_path" >&2
  status=1
else
  actual_ak_digest="$("$digest_tool" "$answer_key_path")"
  if [[ "$actual_ak_digest" != "$expected_ak_digest" ]]; then
    echo "verify-ruler.sh: FAIL — answer-key digest mismatch" >&2
    echo "  expected: $expected_ak_digest" >&2
    echo "  actual:   $actual_ak_digest" >&2
    echo "  path:     $answer_key_path" >&2
    status=1
  fi
fi

if [[ ! -f "$manifest_path" ]]; then
  echo "verify-ruler.sh: FAIL — reference manifest missing at $manifest_path" >&2
  status=1
else
  actual_manifest_digest="$("$digest_tool" "$manifest_path")"
  if [[ "$actual_manifest_digest" != "$expected_manifest_digest" ]]; then
    echo "verify-ruler.sh: FAIL — manifest digest mismatch" >&2
    echo "  expected: $expected_manifest_digest" >&2
    echo "  actual:   $actual_manifest_digest" >&2
    echo "  path:     $manifest_path" >&2
    status=1
  fi
fi

if [[ "$status" -eq 0 ]]; then
  echo "verify-ruler.sh: PASS — answer key and manifest digests match the frozen lock ($lock_path)"
fi

exit "$status"
