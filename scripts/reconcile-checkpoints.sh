#!/usr/bin/env bash
# reconcile-checkpoints.sh — detect work that LANDED IN THE TREE but was never
# RECORDED in a task's checkpoint file.
#
# The failure this exists for (see .agents/lessons/unrecorded-landed-work.md):
# a delegated builder is killed — usage limit, operator stop, harness suspend —
# AFTER it finished editing an item but BEFORE it appended that item's
# checkpoint entry. The checkpoint file then UNDERSTATES what is in the tree,
# the handoff inherits that understatement, and the next agent re-implements
# work that already exists (or, worse, "fixes" it into a different shape).
#
# The diagnostic is mtime, not content: any task-surface file modified AFTER
# the checkpoint file's own last write is, by construction, a change the
# checkpoint file cannot be describing.
#
# This is a RECONCILIATION AID, not a gate. It is deliberately not wired into
# `ruler` or `ci`: it makes no claim about correctness, only about what the
# record can and cannot be describing. Its output is a list of files to look
# at before planning, never a verdict.
#
# Usage:  ./scripts/reconcile-checkpoints.sh <task-number>
# Exit:   0 = nothing modified after the last checkpoint write
#         1 = files modified after it (RECONCILE BEFORE PLANNING)
#         2 = usage/precondition error

set -euo pipefail

if [ $# -ne 1 ]; then
    echo "usage: $0 <task-number>" >&2
    exit 2
fi

task="$1"
case "$task" in
    ''|*[!0-9]*) echo "$0: task number must be a positive integer, got '$task'" >&2; exit 2 ;;
esac

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

reports_dir=".agents/reports/task${task}"
if [ ! -d "$reports_dir" ]; then
    echo "$0: no report directory at $reports_dir — nothing to reconcile against" >&2
    exit 2
fi

# The most recently written checkpoint file in the task's report directory.
# A task may carry several (one per builder slice); the newest is the one that
# bounds "what the record can be describing".
newest_checkpoint=""
newest_mtime=0
while IFS= read -r -d '' candidate; do
    mtime="$(stat -c %Y "$candidate")"
    if [ "$mtime" -gt "$newest_mtime" ]; then
        newest_mtime="$mtime"
        newest_checkpoint="$candidate"
    fi
done < <(find "$reports_dir" -maxdepth 1 -type f -name '*checkpoint*.md' -print0)

if [ -z "$newest_checkpoint" ]; then
    echo "reconcile-checkpoints.sh: no checkpoint file in $reports_dir."
    echo "  Every delegated slice must checkpoint as it goes. Treat ALL task-surface"
    echo "  changes below as unrecorded and reconcile them against the tree."
    newest_mtime=0
    newest_checkpoint="(none)"
fi

# The task surface: everything git reports as modified or untracked, minus the
# paths that churn for reasons unrelated to task work. Agent-tooling state and
# scratch directories are excluded because their mtimes carry no information
# about whether an implementation item landed.
mapfile -t candidates < <(
    git status --porcelain -- . \
    | sed 's/^...//' \
    | sed 's/^.* -> //' \
    | grep -v -E '^(\.jolli/|\.tmp/|\.agents/prompts/|\.serena/|\.wayfinder/|target/)' \
    || true
)

unrecorded=()
for path in "${candidates[@]}"; do
    [ -f "$path" ] || continue
    # The checkpoint file and the report files the slice writes as it works are
    # the record itself, not the work — comparing them to their own mtime is
    # circular.
    case "$path" in
        "$reports_dir"/*) continue ;;
    esac
    mtime="$(stat -c %Y "$path")"
    if [ "$mtime" -gt "$newest_mtime" ]; then
        unrecorded+=("$path")
    fi
done

echo "reconcile-checkpoints.sh — task $task"
echo "  record bound: $newest_checkpoint"
if [ "$newest_mtime" -gt 0 ]; then
    echo "  last written: $(date -d "@$newest_mtime" '+%Y-%m-%d %H:%M:%S %Z')"
fi
echo

if [ ${#unrecorded[@]} -eq 0 ]; then
    echo "  OK — no task-surface file was modified after the last checkpoint write."
    echo "  The checkpoint file can account for every change in the tree. This does"
    echo "  NOT mean the record is correct, only that it is not structurally stale."
    exit 0
fi

echo "  UNRECORDED WORK — ${#unrecorded[@]} file(s) modified AFTER the last checkpoint write:"
echo
for path in "${unrecorded[@]}"; do
    printf '    %s  %s\n' "$(date -d "@$(stat -c %Y "$path")" '+%Y-%m-%d %H:%M:%S')" "$path"
done
echo
echo "  These changes cannot be described by the checkpoint file — it was written"
echo "  before they existed. Reconcile them against the tree BEFORE planning:"
echo "    1. read what actually changed (git diff / read the file);"
echo "    2. decide whether the item LANDED, landed PARTIALLY, or is mid-edit;"
echo "    3. record the finding in the checkpoint file, labelled as recovered-and-"
echo "       verified rather than as the recovering agent's own implementation;"
echo "    4. only then plan the remaining work."
echo
echo "  Never re-implement an item without checking it against this list first."
exit 1
