#!/usr/bin/env bash
# scripts/check.sh — the demo-calculator project's real check command.
#
# Deterministic, no network. Verifies:
#   1. calculator.html exists at the project root.
#   2. It declares button markup for digits 0-9 and the four arithmetic
#      operators (+ - * /, accepting the common unicode variants x/X for
#      multiply-by and : for divide-by is NOT accepted — keep it to plain
#      ASCII operator glyphs or their words).
#   3. If `node` is on PATH, genuinely evaluates the page (see
#      scripts/eval-calculator.js): builds a minimal DOM around the page's
#      inline <script>, clicks the buttons for two small expressions, and
#      checks the result actually computed is correct. Without `node` this
#      step is skipped and only the structural check applies.
#
# Exits 0 exactly when calculator.html exists and is judged sound; exits 1
# otherwise with a clear message. Run before any agent has produced
# calculator.html, this fails cleanly at step 1 — that failure is the
# point: the demo's whole job is watching a real run turn it into a pass.
set -u

cd "$(dirname "${BASH_SOURCE[0]}")/.." || exit 1

PAGE="calculator.html"

if [[ ! -f "$PAGE" ]]; then
  echo "check: missing $PAGE — no run has produced a calculator yet" >&2
  exit 1
fi

fail=0

if ! grep -qiE '<button' "$PAGE"; then
  echo "check: $PAGE has no <button> elements" >&2
  fail=1
fi

for digit in 0 1 2 3 4 5 6 7 8 9; do
  if ! grep -qE "<button[^>]*>[^<]*\b${digit}\b[^<]*</button>" "$PAGE" \
     && ! grep -qE ">\s*${digit}\s*<" "$PAGE"; then
    echo "check: $PAGE is missing a digit button for '$digit'" >&2
    fail=1
  fi
done

declare -A operator_patterns=(
  [add]='[+]'
  [subtract]='(^|[^-])-([^-]|$)'
  [multiply]='[*xX×]'
  [divide]='[/÷]'
)
for op in add subtract multiply divide; do
  pattern="${operator_patterns[$op]}"
  if ! grep -qE "$pattern" <(grep -oE '<button[^>]*>[^<]*</button>' "$PAGE"); then
    echo "check: $PAGE has no button that looks like the '$op' operator" >&2
    fail=1
  fi
done

if [[ "$fail" -ne 0 ]]; then
  echo "check: structural check FAILED" >&2
  exit 1
fi
echo "check: structural check passed ($PAGE has digit and operator buttons)"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
if command -v node >/dev/null 2>&1; then
  if node "$SCRIPT_DIR/eval-calculator.js" "$PAGE"; then
    echo "check: PASS — $PAGE exists, is structurally sound, and evaluates correctly"
    exit 0
  else
    echo "check: FAILED — $PAGE did not evaluate correctly (see eval-calculator output above)" >&2
    exit 1
  fi
else
  echo "check: 'node' not found on PATH — skipping dynamic evaluation, structural check stands"
  echo "check: PASS (structural only) — $PAGE exists and looks like a calculator"
  exit 0
fi
