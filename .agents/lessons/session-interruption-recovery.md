# Lesson: the orchestrating session itself gets interrupted — resume from files, never from memory

**When:** Tasks 4–6, 2026-08-16/17. Five instances across three sessions: the Task-4
orphan (~4.4k lines of uncompiling uncommitted ports work), the Task-5 double suspension
plus the cancelled M1/M2 agent (code landed, proof missing), and the Task-6 orphan
(builder round complete per its checkpoints, but final report, gates, teeth, and the
entire critic loop never ran — CURRENT_STATUS.md still said "brief staged").

**Observed pattern:** the harness suspends or kills the orchestrating session between
tool calls (or a delegated agent is cancelled mid-slice). Everything the interrupted
work already wrote to disk persists — code, checkpoint files, partial reports. What goes
missing is the tail of the loop: the final report, the gate run, teeth, critics,
correction, confirmation, the status update, the commit. CURRENT_STATUS.md then
**understates** progress ("next: task N") while task N's implementation is already in
the tree.

## Detecting it (before assuming anything about task state)

- Run `git status` and reconcile it against CURRENT_STATUS.md **before treating any task
  as unstarted** (the Task-4 lesson). Task-scoped untracked/modified files the status
  file does not explain are the signature.
- A checkpoints file that is complete while the promised final report does not exist.
- Source mtimes newer than the HEAD commit that CURRENT_STATUS.md points at.

## Recovery procedure (worked every time: Tasks 4, 5, 6)

1. `just ruler` first. If it fails, that outranks the recovery.
2. Reconstruct state from `git log`, `git status`, and the task's report files — never
   from conversational memory and never from what "should" have happened next.
3. Cheapest question first: does the tree compile (`cargo check --workspace
   --all-targets`)?
4. Run the task's gates **personally** — orchestrator verification is independent of the
   builder's claims anyway — before dispatching anything new.
5. Classify the orphan and resume at the missing step; never redo work that files
   already recorded:
   - code + proofs landed, only reports/critics missing → add the missing report slice,
     resume the loop at critics (Task 6);
   - code landed, proofs missing → finisher brief: prove only, change no behavior
     (Task 5 M1/M2);
   - code partial or uncompiling → recon gap reports (read-only explore agents), then a
     finisher brief against the gap (Task 4).
6. The interrupted agent's unverifiable claims are not evidence. The tree is the
   contract; a checkpoint file is the agent's report.

## Prevention (cheap, procedural)

- When dispatching a slice, the orchestrator writes an **IN-FLIGHT line** into the
  task's report directory naming the slice, its result-contract path, and the loop steps
  still ahead. One line written before the agent runs turns later resume from inference
  into lookup.
- Checkpoint-as-you-go stays mandatory for every delegated slice. This failure class
  compounds with [subagent-message-loss.md](subagent-message-loss.md): a cancelled agent
  that also lost its final message is recoverable *only* through its checkpoint file.
