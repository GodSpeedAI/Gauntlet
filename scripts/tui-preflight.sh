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
#                integration matrix; plan T25 owns the real implementation).
#                Re-verifies document integrity WITHOUT rebinding, then verifies
#                every declared runtime capability, required actual integration,
#                and settled evidence artifact. The rule (prereg D2): a REQUIRED
#                integration may be (a) EXECUTED here and passing, (b) GATED on an
#                opt-in env var whose settled evidence is on record, or (c)
#                RECORDED-UNQUALIFIED with its owning task and trigger-record on
#                file — each GATED/UNQUALIFIED row is printed, named, and checked
#                against its backing evidence. Missing/unbacked/skipped REQUIRED
#                behavior FAILS. An env-gated battery declared by no conformance
#                row, or a row whose backing evidence vanished, is an unbacked
#                skip and FAILS.
#
# Missing/unknown mode is a usage error, per gate_binding_rule: "no implicit
# weakening to bootstrap."
#
# Usage: scripts/tui-preflight.sh <bootstrap|conformance>

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
workbench_dir="$repo_root/workbench"
plan_path="$repo_root/.agents/plans/gauntlet-tui-prime-workbench.plan.yaml"
evidence_root="$repo_root/.agents/reports/gauntlet-tui-prime-workbench/evidence"

if [[ $# -ne 1 ]]; then
  echo "tui-preflight.sh: usage: scripts/tui-preflight.sh <bootstrap|conformance>" >&2
  exit 2
fi
mode="$1"

if [[ "$mode" != "bootstrap" && "$mode" != "conformance" ]]; then
  echo "tui-preflight.sh: FAIL — unknown mode '$mode' (expected bootstrap or conformance)" >&2
  exit 2
fi

status=0
fail() {
  echo "tui-preflight.sh: $mode: FAIL — $1" >&2
  status=1
}
pass() {
  echo "tui-preflight.sh: $mode: pass — $1"
}

# ===========================================================================
# Shared section: document/toolchain integrity (conformance "reverifies
# document hashes without rebinding" — same checks, never weaker).
# ===========================================================================

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

if [[ "$mode" == "bootstrap" ]]; then
  if [[ "$status" -eq 0 ]]; then
    echo "tui-preflight.sh: bootstrap: PASS"
  fi
  exit "$status"
fi

# ===========================================================================
# Conformance section: runtime qualification (REQ-CONFIG-010/011/012).
#
# Row classes:
#   EXECUTED   — the check runs here, live, and must pass.
#   GATED      — capability whose qualifying battery is opt-in via an env var;
#                backed by the owning task's settled evidence record.
#   UNQUALIFIED— honestly recorded limitation; backed by the owning task's
#                evidence/trigger-record. Never silently promoted to pass.
# ===========================================================================

echo "tui-preflight.sh: conformance: runtime capability qualification (target: this host)"

# --- E1. Bun runtime (T03, REQ-BUN-001/002) --------------------------------
if command -v bun >/dev/null 2>&1; then
  bun_version="$(bun --version 2>/dev/null || echo missing)"
  if [[ "$(printf '%s\n' "1.4.0" "$bun_version" | sort -V | head -1)" == "1.4.0" ]]; then
    pass "EXECUTED bun runtime present: $bun_version (>= 1.4.0 qualified by T03; REQ-BUN-001/002)"
  else
    fail "EXECUTED bun runtime $bun_version is older than the qualified 1.4.0 (REQ-BUN-001/002)"
  fi
else
  fail "EXECUTED bun runtime not found on PATH (REQ-BUN-001/002)"
fi

# --- E2. bun:sqlite native substrate (T06, REQ-BUN-003) --------------------
sqlite_probe="$(cd "$workbench_dir" && bun -e 'import { Database } from "bun:sqlite";
const db = new Database(":memory:");
db.run("CREATE TABLE t (x INTEGER)");
db.run("INSERT INTO t VALUES (42)");
const row = db.query("SELECT x FROM t").get();
console.log(row && row.x === 42 ? "SQLITE_OK" : "SQLITE_BAD");' 2>/dev/null || echo PROBE_CRASHED)"
if [[ "$sqlite_probe" == "SQLITE_OK" ]]; then
  pass "EXECUTED bun:sqlite native substrate opens, writes, reads (T06 projection store basis; REQ-BUN-003)"
else
  fail "EXECUTED bun:sqlite probe failed ($sqlite_probe) — the T06 projection substrate is unavailable (REQ-BUN-003)"
fi

# --- E3. Bun PTY facility (T13, REQ-BUN-004) --------------------------------
pty_probe="$(cd "$workbench_dir" && bun -e 'const p = Bun.spawn(["/bin/true"], { terminal: { cols: 80, rows: 24, data() {} } });
const ok = p.terminal !== undefined;
if (p.terminal && !p.terminal.closed) p.terminal.close();
await p.exited;
console.log(ok ? "PTY_OK" : "PTY_MISSING");' 2>/dev/null || echo PROBE_CRASHED)"
if [[ "$pty_probe" == "PTY_OK" ]]; then
  pass "EXECUTED Bun PTY facility attaches a real terminal to a spawned child (T13 embedded-terminal basis; REQ-BUN-004)"
else
  fail "EXECUTED Bun PTY probe failed ($pty_probe) — the embedded-terminal substrate is unavailable (REQ-BUN-004)"
fi

# --- E4. Pinned renderer stack importable at exact versions (T12, REQ-TUI-002/003/004) ---
renderer_probe="$(cd "$workbench_dir" && bun -e 'const pkg = await Bun.file("package.json").json();
let ok = true;
for (const name of ["@opentui/core", "@opentui/react", "react", "react-reconciler"]) {
  const want = pkg.dependencies[name];
  let got;
  try { got = (await Bun.file(`node_modules/${name}/package.json`).json()).version; }
  catch { got = "ABSENT"; }
  if (want !== got) { ok = false; console.error(`MISMATCH ${name}: pinned ${want}, installed ${got}`); }
}
console.log(ok ? "PINS_OK" : "PINS_BAD");' 2>/dev/null || echo PROBE_CRASHED)"
if [[ "$renderer_probe" == "PINS_OK" ]]; then
  pass "EXECUTED OpenTUI/React renderer stack installed at the exact pinned versions (T12 shell basis; REQ-TUI-002/003/004)"
else
  fail "EXECUTED renderer stack probe failed ($renderer_probe) — pinned OpenTUI/React not correctly installed (REQ-TUI-002/003/004)"
fi

# --- E5. Prime substrate identity (T04/T24, REQ-PRIME-001/002; T24 inventory) ---
prime_bin="$(command -v prime-agent || command -v prime || true)"
if [[ -z "$prime_bin" ]]; then
  fail "EXECUTED prime-agent binary not found on PATH — the qualified Prime substrate (0.8.0) is missing (REQ-PRIME-001/002; T24 substrate-inventory)"
else
  prime_version="$("$prime_bin" --version 2>&1 | head -1 || echo unreadable)"
  if [[ "$prime_version" == "0.8.0" ]]; then
    pass "EXECUTED prime-agent at $prime_bin reports $prime_version — matches the T24-qualified substrate inventory"
  else
    fail "prime-agent reports '$prime_version', the T24-qualified inventory pins 0.8.0 — SUBSTRATE DRIFT, requalification required (T24 owns the inventory re-run)"
  fi
fi

# --- E6. Release artifact + manifest digest (T20, REQ-UX-005/REQ-BUN-005) ----
if [[ -f "$workbench_dir/dist/release-manifest.json" && -f "$workbench_dir/dist/gauntlet-workbench" ]]; then
  manifest_probe="$(cd "$workbench_dir" && bun -e 'import { readFileSync } from "node:fs";
import { verifyReleaseManifest, RELEASE_MANIFEST } from "./src/release-manifest";
const doc = JSON.parse(readFileSync(RELEASE_MANIFEST, "utf8"));
const res = verifyReleaseManifest(doc);
if (!res.valid) console.error(res.failures.join("; "));
console.log(res.valid ? "MANIFEST_OK" : "MANIFEST_INVALID");' 2>/dev/null || echo PROBE_CRASHED)"
  if [[ "$manifest_probe" == "MANIFEST_OK" ]]; then
    pass "EXECUTED release artifact present and dist/release-manifest.json digest + required fields verify (T20 release qualification; REQ-UX-005)"
  else
    fail "EXECUTED release manifest check failed ($manifest_probe) — artifact drifted from its recorded digest; re-run \`just tui-build release\` (T20)"
  fi
else
  fail "EXECUTED release artifact or manifest missing under workbench/dist/ — run \`just tui-build release\` (T20; REQ-UX-005)"
fi

# --- E7/E8. Support tooling for the qualified transport legs (T20) ----------
if command -v tmux >/dev/null 2>&1; then
  pass "EXECUTED tmux present: $(tmux -V 2>/dev/null || echo 'version unreadable') (T20 tmux transport legs)"
else
  fail "tmux not found — the T20 tmux transport legs cannot run on this host"
fi

# --- G. GATED capabilities: opt-in env batteries, evidence-backed ------------
echo "tui-preflight.sh: conformance: gated capabilities (opt-in env batteries, each backed by settled evidence)"

check_backing() {
  local row_name="$1" evidence_rel="$2" marker="$3"
  local evidence_file="$evidence_root/$evidence_rel"
  if [[ ! -f "$evidence_file" ]]; then
    fail "GATED/UNQUALIFIED row '$row_name' is UNBACKED — evidence file $evidence_rel missing"
    return
  fi
  if ! grep -qF -- "$marker" "$evidence_file"; then
    fail "GATED/UNQUALIFIED row '$row_name' is UNBACKED — marker '$marker' not found in $evidence_rel"
    return
  fi
  echo "tui-preflight.sh: conformance: backed — $row_name (evidence: $evidence_rel)"
}

# G1: T13 real-PTY battery.
if [[ "${GAUNTLET_T13_REAL_PTY:-}" == "1" ]]; then
  pass "GATED GAUNTLET_T13_REAL_PTY=1 is SET — the T13 real-PTY battery runs inside \`just tui-test\` on this invocation"
else
  echo "tui-preflight.sh: conformance: GATED — T13 real Bun PTY qualification battery: set GAUNTLET_T13_REAL_PTY=1 to execute (owns REQ-BUN-004 real-PTY legs)"
  check_backing "T13 real-PTY battery" "T13/embedded-terminal.md" "GAUNTLET_T13_REAL_PTY=1 bun test embedded-terminal"
fi

# G2: T20 real-PTY operator journeys.
if [[ "${GAUNTLET_T20_REAL_PTY:-}" == "1" ]]; then
  pass "GATED GAUNTLET_T20_REAL_PTY=1 is SET — the T20 real-PTY operator journeys run inside \`just tui-test\` on this invocation"
else
  echo "tui-preflight.sh: conformance: GATED — T20 real-PTY operator journeys (120x40 / 80x24 / below-min): set GAUNTLET_T20_REAL_PTY=1 to execute (owns REQ-UX-001/002/003 real-PTY legs)"
  check_backing "T20 real-PTY journeys" "T20/release-qualification.md" "GAUNTLET_T20_REAL_PTY=1 bun test src/operator-journeys-pty.test.ts"
fi

# G3: T24 real-substrate battery.
if [[ "${GAUNTLET_T24_REAL_PRIME:-}" == "1" ]]; then
  pass "GATED GAUNTLET_T24_REAL_PRIME=1 is SET — the T24 real-substrate lifecycle battery runs inside \`just tui-test\` on this invocation"
else
  echo "tui-preflight.sh: conformance: GATED — T24 real-substrate Prime/Python lifecycle battery: set GAUNTLET_T24_REAL_PRIME=1 to execute (owns REQ-PRIME-003/004/005/009/010 real integration)"
  check_backing "T24 real-substrate battery" "T24/prime-fresh-target.md" "GAUNTLET_T24_REAL_PRIME=1"
fi

# G4: real-sshd SSH transport leg — environment-gated. If sshd IS reachable the
# required integration is executed HERE (skipping it then would be a silent gap).
ssh_probe_output="$(ssh -o BatchMode=yes -o ConnectTimeout=3 -o StrictHostKeyChecking=no \
  -o UserKnownHostsFile=/dev/null localhost 'echo SSH_TARGET_OK' 2>/dev/null || true)"
if [[ "$ssh_probe_output" == *"SSH_TARGET_OK"* ]]; then
  echo "tui-preflight.sh: conformance: sshd reachable — running the required SSH-target integration now"
  if (cd "$workbench_dir" && bun test src/ssh-target.test.ts >/tmp/tui-preflight-ssh.log 2>&1); then
    pass "EXECUTED real-sshd SSH transport leg through src/ssh-target.test.ts (T20; REQ-CONFIG-005/REQ-UX-005 remote clause)"
  else
    fail "sshd is reachable but the SSH-target suite FAILED — see /tmp/tui-preflight-ssh.log (T20)"
  fi
else
  echo "tui-preflight.sh: conformance: GATED-UNAVAILABLE — real-sshd SSH transport leg: no reachable sshd on this host (probe: connection refused/timeout); executes unchanged via src/ssh-target.test.ts once sshd is reachable (owns REQ-UX-005 remote clause)"
  check_backing "T20 real-sshd leg" "T20/release-qualification.md" "SSH leg did not run"
fi

# --- U. RECORDED-UNQUALIFIED capabilities: honest limitations, evidence-backed --
echo "tui-preflight.sh: conformance: recorded-unqualified capabilities (honest limitations, each backed by its owning task's evidence)"

echo "tui-preflight.sh: conformance: UNQUALIFIED — T14B role_binding declaration fixture: no local .sea fixture emits a role_binding declaration on the slice0b grammar; projection exercised structurally only (owner: T14B; SXR_LIVE_QUALIFICATION.role_binding_declaration_fixture_real=false)"
check_backing "T14B role_binding fixture" "T14B/sxr-evidence.md" "declaration fixtures**: no local"
if grep -q "role_binding_declaration_fixture_real: false" "$workbench_dir/src/sxr-adapter.ts"; then
  pass "EXECUTED T14B qualification constant records role_binding fixture as not-real (fail-closed honesty)"
else
  fail "T14B sxr-adapter qualification constant no longer records role_binding_declaration_fixture_real: false — the recorded limitation and the code disagree"
fi

echo "tui-preflight.sh: conformance: UNQUALIFIED — T24 \`schedule list\`/\`cancel\` against a non-default daemon socket: validator rejects the option while \`add\` accepts it; schedule ADD was executed for real (owner: T24, trigger-record)"
check_backing "T24 schedule socket gap" "T24/prime-fresh-target.md" "cannot target a non-default daemon socket"

echo "tui-preflight.sh: conformance: UNQUALIFIED — T24 send --steer/--follow-up HELP/IMPLEMENTATION drift in the installed substrate: documented by --help, refused by the parser (owner: T24, trigger-record)"
check_backing "T24 send-mode drift" "T24/prime-fresh-target.md" "documented_but_refused"

echo "tui-preflight.sh: conformance: UNQUALIFIED — release entry is a cold-start host loop: no live \`gauntlet serve\` endpoint binding; the embedded OpenTUI React viewport remains recorded later-task wiring (owner: T20/T13 recorded deferrals; REQ-UX-007 live-endpoint breadth)"
check_backing "T20 release-entry breadth" "T20/release-qualification.md" "cold-start host loop"

# --- Sweep: every boolean env gate declared in the test tree must have a row ----
# The sweep matches the gating idiom `process.env["GAUNTLET_T.."] === "1"` — a
# comparison that ENABLES hidden behavior. Env vars that merely parameterize a
# test (e.g. a capture-output directory) are not gates and do not match.
declared_env_gates="GAUNTLET_T13_REAL_PTY GAUNTLET_T20_REAL_PTY GAUNTLET_T24_REAL_PRIME"
found_env_gates="$(grep -rhoE 'GAUNTLET_T[0-9]+_[A-Z_]+"?\]? *=== *"1"' "$workbench_dir/src" --include='*.test.ts' 2>/dev/null \
  | grep -oE 'GAUNTLET_T[0-9]+_[A-Z_]+' | sort -u || true)"
if [[ -n "$found_env_gates" ]]; then
  while IFS= read -r env_gate; do
    [[ -z "$env_gate" ]] && continue
    if [[ " $declared_env_gates " != *" $env_gate "* ]]; then
      fail "UNBACKED GATE — test tree declares env gate $env_gate that has no conformance row (a gated skip with no evidence backing)"
    fi
  done <<< "$found_env_gates"
  if [[ "$status" -eq 0 ]]; then
    pass "EXECUTED env-gate sweep: every boolean env-gated battery in the tree ($(echo "$found_env_gates" | tr '\n' ' ') ) has a declared conformance row"
  fi
fi

# --- Evidence artifacts: every settled task has evidence with a Confirmation ----
echo "tui-preflight.sh: conformance: settled evidence artifacts"
settled_tasks="T00 T00A T01 T02 T03 T04 T05 T06 T07 T08 T09 T10 T11 T12 T13 T14 T14B T15 T16 T17 T18 T19 T20 T21 T22 T23 T24"
missing_evidence=""
for task in $settled_tasks; do
  task_dir="$evidence_root/$task"
  ok=0
  if [[ -d "$task_dir" ]]; then
    if grep -rlq "^Confirmation:" "$task_dir" --include='*.md' 2>/dev/null; then
      ok=1
    fi
  fi
  if [[ "$ok" -ne 1 ]]; then
    missing_evidence+=" $task"
  fi
done
if [[ -z "$missing_evidence" ]]; then
  pass "EXECUTED all 27 settled task evidence directories exist and carry a Confirmation record (T25's own evidence is produced by this run and is not self-checked here)"
else
  fail "settled evidence missing or Confirmation-less for:$missing_evidence"
fi

if [[ "$status" -eq 0 ]]; then
  echo "tui-preflight.sh: conformance: PASS — every declared runtime capability is executed-and-passing, or visibly GATED/UNQUALIFIED with its settled evidence record; no REQUIRED integration is silently missing"
fi
exit "$status"
