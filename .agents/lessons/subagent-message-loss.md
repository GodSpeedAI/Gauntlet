# Lesson: sub-agent final messages get lost in this environment

**When:** Task 4, 2026-08-16. Four lost/truncated final messages across two `general`
builders and two `general` critics. The Task-3 builder-kill (deviation 4) is the same
family.

**Observed pattern:** the working tree/mutated files persist, but the final report
message arrives empty or truncated — sometimes with the work done (correction round:
gates 1-5 green, no report), sometimes with nothing done (first correction attempt).

## Rules that prevent the failure

1. **Every delegated piece of work writes a durable artifact as it goes.** For builders:
   a progress file (`.agents/reports/<task>/<slice>-progress.md`) appended after each
   numbered item, before the next gate run. The final report is the file; the final
   message is a convenience.
2. **Critics write their report file first**, then summarize in the final message. A lost
   final message never loses the critique.
3. **`explore`-type agents delivered full reports every time this session**; `general`
   lost its tail repeatedly. For read-only/recon/adversarial work, prefer `explore`.
4. **Do not trust "N gates green" from a truncated message alone** — verify from the tree
   (compile state, `git status`, the relevant test counts) before settlement.
5. When a slice dies with nothing written: re-dispatch fresh with the same brief (the
   briefs above survived verbatim). Delegation itself remained available every time.