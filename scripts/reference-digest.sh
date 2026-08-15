#!/usr/bin/env bash
# reference-digest.sh — normalize a reference file and print its digest.
#
# This is the deliberately primitive bootstrap digest tool (plan Task 1). It exists
# before any Rust code does. Task 7's ReferenceProvider::normalize + digest MUST
# reproduce the exact rules below, byte for byte — that agreement between two
# independent implementations is itself the proof (plan Task 7 gate: this script's
# output diffed against `gauntlet reference digest` must be identical).
#
# Normalization rules (exhaustive — do not add more):
#   1. Convert CRLF and lone CR line endings to LF.
#   2. Collapse any run of blank lines at end-of-file so the normalized text ends
#      with exactly one trailing LF (an empty input normalizes to zero bytes). This
#      absorbs harmless "editor added/removed the final newline" drift only.
#   3. Do not otherwise touch line content: in-line whitespace (leading, trailing,
#      or internal) and line order are left exactly as written ("sort nothing").
#      This is deliberate, not an oversight: appending so much as a single trailing
#      space to any content line MUST change the digest (plan Task 1 teeth check) —
#      an implementation that also strips per-line trailing whitespace would make
#      the ruler insensitive to real edits and is a defect, not a convenience.
#
# Note on scope: "canonical whitespace" in spec §9.4 is implementation-defined
# (§0.3). This tool defines it narrowly — end-of-file newline drift only — precisely
# so the digest stays maximally sensitive to content edits, which is the whole point
# of freezing a ruler.
#
# Digest: sha256 (algorithm is configuration elsewhere; this bootstrap tool is
# sha256-only) over the normalized UTF-8 bytes.
#
# Output: exactly one line on stdout, "sha256:<lowercase hex>", newline-terminated.
# No other output on success. Non-zero exit and a message on stderr if the path is
# missing, unreadable, or not valid UTF-8 (untrusted input is never silently coerced).
#
# Usage: scripts/reference-digest.sh <path>

set -euo pipefail

path="${1:?usage: reference-digest.sh <path>}"

if [[ ! -f "$path" ]]; then
  echo "reference-digest.sh: no such file: $path" >&2
  exit 1
fi

python3 - "$path" <<'PYEOF'
import hashlib
import sys

path = sys.argv[1]

with open(path, "rb") as f:
    raw = f.read()

try:
    text = raw.decode("utf-8")
except UnicodeDecodeError as e:
    sys.stderr.write(f"reference-digest.sh: {path} is not valid UTF-8: {e}\n")
    sys.exit(1)

# Rule 1: normalize line endings.
text = text.replace("\r\n", "\n").replace("\r", "\n")

# Rule 2: collapse trailing blank lines to exactly one final LF (or zero bytes for
# an empty/all-blank input). Line content itself (rule 3) is left untouched — no
# rstrip here. text.split("\n") on a string ending in "\n" yields a trailing ""
# element representing that final newline; strip all trailing "" entries
# (representing the final newline plus any additional blank lines) then rejoin with
# exactly one trailing newline, unless nothing is left.
lines = text.split("\n")
while lines and lines[-1] == "":
    lines.pop()

normalized = ("\n".join(lines) + "\n") if lines else ""

digest = hashlib.sha256(normalized.encode("utf-8")).hexdigest()
print(f"sha256:{digest}")
PYEOF
