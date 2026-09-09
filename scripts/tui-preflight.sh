#!/usr/bin/env bash
# tui-preflight.sh — plan Task T00 (.agents/plans/gauntlet-tui-prime-workbench.plan.yaml),
# gate_binding_rule: "tui-preflight distinguishes document-integrity preflight from
# runtime qualification." Two modes, never a default:
#
#   bootstrap    Document/toolchain integrity only. Verifies: the workbench/ package
#                exists with pinned exact-version devDependencies (no ^, ~, or range
#                operators — AGENTS.md dependency discipline), workbench/bun.lock is
#                committed and in sync with package.json, `bun audit` reports no
#                vulnerabilities, and the spec/DESIGN.md hashes the plan pins in
#                source.spec.sha256/source.design.sha256 still match the live files
#                (the same tripwire verify-ruler.sh applies to the Core spec/answer
#                key, applied here to this plan's own source authority).
#
#   conformance  Full runtime qualification (REQ-CONFIG-010/011/012 and the target
#                integration matrix). NOT_IMPLEMENTED until the corresponding plan
#                tasks (T04 Prime host, T06 projection store, T12 OpenTUI shell, ...)
#                exist to qualify against — this script does not fabricate a runtime
#                check with nothing behind it.
#
# Missing/unknown mode is a usage error, per gate_binding_rule: "no implicit
# weakening to bootstrap."
#
# Usage: scripts/tui-preflight.sh <bootstrap|conformance>

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
workbench_dir="$repo_root/workbench"
plan_path="$repo_root/.agents/plans/gauntlet-tui-prime-workbench.plan.yaml"

if [[ $# -ne 1 ]]; then
  echo "tui-preflight.sh: usage: scripts/tui-preflight.sh <bootstrap|conformance>" >&2
  exit 2
fi
mode="$1"

if [[ "$mode" == "conformance" ]]; then
  echo "tui-preflight.sh: conformance: NOT_IMPLEMENTED — no runtime substrate (Prime host," >&2
  echo "  projection store, OpenTUI shell, target terminal matrix) exists to qualify yet." >&2
  echo "  This is a recorded absence, not a skipped check masquerading as a pass." >&2
  exit 2
fi

if [[ "$mode" != "bootstrap" ]]; then
  echo "tui-preflight.sh: FAIL — unknown mode '$mode' (expected bootstrap or conformance)" >&2
  exit 2
fi

status=0
fail() {
  echo "tui-preflight.sh: bootstrap: FAIL — $1" >&2
  status=1
}
pass() {
  echo "tui-preflight.sh: bootstrap: pass — $1"
}

# --- 1. workbench/ package skeleton exists -------------------------------
for f in package.json biome.json tsconfig.json bun.lock .gitignore; do
  if [[ ! -f "$workbench_dir/$f" ]]; then
    fail "workbench/$f is missing"
  fi
done
if [[ "$status" -eq 0 ]]; then
  pass "workbench/ package skeleton present (package.json, biome.json, tsconfig.json, bun.lock, .gitignore)"
fi

# --- 2. every devDependency is pinned exact (no ^, ~, *, x, range operators) ---
if [[ -f "$workbench_dir/package.json" ]]; then
  unpinned="$(python3 -c '
import json, sys
data = json.load(open(sys.argv[1]))
bad = []
for section in ("dependencies", "devDependencies"):
    for name, spec in data.get(section, {}).items():
        if not spec or not spec[0].isdigit():
            bad.append(f"{name}@{spec}")
print("\n".join(bad))
' "$workbench_dir/package.json")"
  if [[ -n "$unpinned" ]]; then
    fail "unpinned/range dependency spec(s) in package.json (must be exact versions): $unpinned"
  else
    pass "all package.json dependencies are pinned to exact versions"
  fi
fi

# --- 3. bun.lock is committed and in sync with package.json --------------
if [[ -f "$workbench_dir/bun.lock" ]]; then
  if (cd "$workbench_dir" && bun install --frozen-lockfile --dry-run >/tmp/tui-preflight-lockcheck.log 2>&1); then
    pass "workbench/bun.lock is in sync with package.json (frozen-lockfile dry-run)"
  else
    fail "workbench/bun.lock is out of sync with package.json — see /tmp/tui-preflight-lockcheck.log"
  fi
fi

# --- 4. bun audit reports no known vulnerabilities ------------------------
if [[ -f "$workbench_dir/package.json" ]]; then
  if (cd "$workbench_dir" && bun audit >/tmp/tui-preflight-audit.log 2>&1); then
    pass "bun audit reports no known vulnerabilities in workbench/ dependencies"
  else
    fail "bun audit reported findings — see /tmp/tui-preflight-audit.log"
  fi
fi

# --- 5. plan's pinned spec/design hashes match the live files ------------
read_plan_field() {
  python3 -c "
import yaml, sys
d = yaml.safe_load(open(sys.argv[1]))
node = d
for key in sys.argv[2:]:
    node = node[key]
print(node)
" "$plan_path" "$@"
}

for pair in "spec .agents/specs/gauntlet-tui-prime-workbench.spec.yaml" "design DESIGN.md"; do
  set -- $pair
  key="$1"; rel_path="$2"
  expected="$(read_plan_field source "$key" sha256)"
  actual_file="$repo_root/$rel_path"
  if [[ ! -f "$actual_file" ]]; then
    fail "$rel_path (pinned by plan source.$key.sha256) does not exist"
    continue
  fi
  actual="$(sha256sum "$actual_file" | awk '{print $1}')"
  if [[ "$actual" != "$expected" ]]; then
    fail "$rel_path hash mismatch — plan pins $expected, file is $actual (source-authority mismatch)"
  else
    pass "$rel_path matches the plan's pinned source.$key.sha256"
  fi
done

if [[ "$status" -eq 0 ]]; then
  echo "tui-preflight.sh: bootstrap: PASS"
fi
exit "$status"
