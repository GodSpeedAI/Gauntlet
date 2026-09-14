#!/usr/bin/env bash
# scripts/check.sh — the demo-calculator project's real check command.
#
# Deterministic, no network. Judges calculator.html IN THE CURRENT WORKING
# DIRECTORY and writes one test_report per answer-key unit.
#
# Why the current directory, not the project root: Gauntlet's `test` facet
# runs this command with cwd set to the dispatched unit's WORKSPACE, which is
# where the builder's work product lives. (An earlier version `cd`-ed to the
# project root and judged whatever calculator.html sat THERE, so a stale
# artifact could pass while the workspace's actual output was broken.) Bind
# it by absolute path (`GAUNTLET_TOOL_TEST=/abs/.../scripts/check.sh`): a
# relative binding would be resolved inside the workspace, where no scripts/
# exists. Run by hand from the project root, it checks that directory.
#
# Checks:
#   1. structure: calculator.html exists and declares <button> markup for
#      digits 0-9 and the four operators (ASCII glyphs, x/X/× for multiply,
#      ÷ for divide);
#   2. evaluation: if `node` is on PATH, scripts/eval-calculator.js builds a
#      minimal DOM around the page's inline <script>, clicks the buttons for
#      a few expressions, and checks the displayed results.
#
# Reports (the verifier's artifact contract: JSON `tests` + `failures` +
# `command`; PASS iff tests > 0 and failures == 0), written atomically to
# the answer key's locators under ./evidence/:
#   markup-check-report.json  AK-9011  one test per structural check
#   eval-check-report.json    AK-9013  one test per evaluated expression
#   check-report.json         AK-9012  the whole check as one test
# Evaluation that cannot run at all (no node) writes tests=0, which the
# verifier refuses to decide over. Missing evidence is not passing evidence.
#
# Exit status: 0 exactly when every check passed, 1 otherwise.
set -u

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PAGE="calculator.html"
COMMAND="$SCRIPT_DIR/check.sh"
mkdir -p evidence

# write_report <file> <tests> <failures> — atomic (tmp + rename), so a
# reader never sees a torn document.
write_report() {
  local file="evidence/$1" tmp
  tmp="$(mktemp "evidence/.${1}.XXXXXX")"
  printf '{"tests": %d, "failures": %d, "command": "%s"}\n' "$2" "$3" "$COMMAND" >"$tmp"
  mv -f "$tmp" "$file"
}

# --- 1. structure ----------------------------------------------------------
structure_tests=0
structure_failures=0
check() { # check <description> <command...>
  local description="$1"
  shift
  structure_tests=$((structure_tests + 1))
  if ! "$@"; then
    echo "check: $description" >&2
    structure_failures=$((structure_failures + 1))
  fi
}

if [[ ! -f "$PAGE" ]]; then
  echo "check: missing $PAGE in $(pwd) — no work product to judge" >&2
  write_report markup-check-report.json 1 1
  write_report eval-check-report.json 1 1
  write_report check-report.json 1 1
  exit 1
fi

check "$PAGE has no <button> elements" grep -qiE '<button' "$PAGE"
for digit in 0 1 2 3 4 5 6 7 8 9; do
  check "$PAGE is missing a digit button for '$digit'" \
    bash -c 'grep -qE "<button[^>]*>[^<]*\b$1\b[^<]*</button>" "$2" || grep -qE ">\s*$1\s*<" "$2"' _ "$digit" "$PAGE"
done
buttons="$(grep -oE '<button[^>]*>[^<]*</button>' "$PAGE" || true)"
check "$PAGE has no add operator button" grep -qE '[+]' <<<"$buttons"
check "$PAGE has no subtract operator button" grep -qE '(^|[^-])-([^-]|$)' <<<"$buttons"
check "$PAGE has no multiply operator button" grep -qE '[*xX×]' <<<"$buttons"
check "$PAGE has no divide operator button" grep -qE '[/÷]' <<<"$buttons"

write_report markup-check-report.json "$structure_tests" "$structure_failures"
if [[ "$structure_failures" -eq 0 ]]; then
  echo "check: structural check passed ($structure_tests checks)"
else
  echo "check: structural check FAILED ($structure_failures of $structure_tests)" >&2
fi

# --- 2. evaluation ---------------------------------------------------------
eval_ok=1
if command -v node >/dev/null 2>&1; then
  eval_output="$(node "$SCRIPT_DIR/eval-calculator.js" "$PAGE" 2>&1)"
  eval_status=$?
  printf '%s\n' "$eval_output"
  totals="$(grep -oE 'totals tests=[0-9]+ failures=[0-9]+' <<<"$eval_output" | tail -1)"
  if [[ -n "$totals" ]]; then
    eval_tests="$(sed -E 's/.*tests=([0-9]+).*/\1/' <<<"$totals")"
    eval_failures="$(sed -E 's/.*failures=([0-9]+).*/\1/' <<<"$totals")"
  else
    # The page could not even be loaded or driven (no inline script, a
    # throw on load, a missing button): one attempted evaluation, failed.
    eval_tests=1
    eval_failures=1
  fi
  write_report eval-check-report.json "$eval_tests" "$eval_failures"
  [[ "$eval_status" -eq 0 && "$eval_failures" -eq 0 ]] || eval_ok=0
else
  echo "check: 'node' not found on PATH — evaluation did not run (tests=0: no evidence, not a pass)" >&2
  write_report eval-check-report.json 0 0
  eval_ok=0
fi

# --- 3. overall ------------------------------------------------------------
if [[ "$structure_failures" -eq 0 && "$eval_ok" -eq 1 ]]; then
  write_report check-report.json 1 0
  echo "check: PASS — $PAGE is structurally sound and evaluates correctly"
  exit 0
fi
write_report check-report.json 1 1
echo "check: FAILED" >&2
exit 1
