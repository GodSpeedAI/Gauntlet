# Task 12 — B3 FINISHER brief (recover mid-Item-1 death)

You are finishing a partially-landed builder slice. Read `AGENTS.md`,
`.agents/reports/task12/builder-brief-b3.md` (the authoritative slice brief),
and `.agents/reports/task12/builder-b3-checkpoints.md` (predecessor
progress) first.

## Reconciled state (orchestrator-verified)

- Items 3 (control config) and 2 (§11.5 disposition table) are LANDED and
  were green when checkpointed (`config::` 45/45, `control::disposition`
  11/11). Do NOT redo them.
- The predecessor died mid-Item 1: `crates/gauntlet-app/src/control/
  controller.rs` exists with NO checkpoint entry, and
  `crates/gauntlet-app/src/reference/provider.rs` +
  `crates/gauntlet-app/src/verify/ladder.rs` carry modifications not present
  in the B1/B2 footprint — intent unknown, possibly non-compiling.
- Item 4 (observer deadlines) and Item 5 (gates) are unstarted (observe
  adapters untouched).

## Procedure

1. **Diagnose before completing**: `git diff crates/gauntlet-app/src/
   reference/provider.rs crates/gauntlet-app/src/verify/ladder.rs` — infer
   the predecessor's intent; `just check-package gauntlet-app` to see
   whether controller.rs compiles. Keep sound partial work; repair or
   remove unsound work — record which and why in the checkpoints file.
2. Complete Item 1 per the B3 brief (controller: §11.1/§11.4 loop, per-tick
   answer-key re-read, boundary verify_unchanged with both digests
   persisted, stall/TimedOut dispositions, summed-slice meter reserve,
   gated owner-writes through the authorities, §18.2 checkpoints, §18.3
   budget-scope resume via Budget::restore, empty-ruler refusal, ONE
   workspace root, Clock-only time). If provider.rs/ladder.rs edits were
   the predecessor's approach to a real need (e.g. exposing the built
   Ladder or the provider's verify path to the controller), validate them
   against the brief's constraints — do NOT widen the composition seal.
3. Item 4 per the B3 brief: deadline enforcement at the four Command call
   sites (command/build/test/git) + bounded capture; filesystem observer's
   existing caps verified by test, not rewritten. Fail-first incl.
   already-expired deadline refuses without spawn.
4. Item 5: the B3 brief's gate list, run personally, one at a time.
5. Append a checkpoint entry after EVERY step; write the result contract
   `.agents/reports/task12/builder-b3.md` covering the WHOLE slice
   (two-session shape noted, predecessor work credited).

Hard rules: same as the B3 brief (no frozen/operator file edits, no
commits, single-compiler lock, fail-first, no weakened gates, named-const
literal idiom — the predecessor's checkpoints record the timeout-hint
scanner trick: duration-shaped names + figures on one unit get flagged;
use intermediate consts like the landed `TICK_WINDOW` precedent).
