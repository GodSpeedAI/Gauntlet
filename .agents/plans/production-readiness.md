# Production readiness — close every unproven item, then prove it live

Date: 2026-09-14. Owner: orchestrator (planning, review, verification).
Builders: one Sonnet subagent at a time (RAM: 7.8 GB VM). Recon: Haiku.
Discipline: RED-first per item; `just boundaries`, clippy `-D warnings`,
fmt, and the touched crates' suites re-run by the orchestrator before
each commit. Nothing is "done" until the orchestrator re-ran the gates.

## Outcome sought

A long-horizon run started from the CLI or the TUI dispatches a real
prime-agent builder reliably, pays honestly against its budget, keeps the
builder inside its workspace (or records that it did not), carries each
settled unit's work forward to the next unit, and delivers the settled
work product back to the project — all proven by one real demo run whose
stored state is checked field by field.

## Grounding (verified 2026-09-14)

- prime-agent's own system prompt (core/prompts/rlm.js) orders "start the
  work, record its handle or output location, then end your turn … Do not
  keep the turn open by polling with time.sleep()". Our `--print` parent
  cell carried exactly that loop, because a `--print` session's end aborts
  its children. Hence the refusals.
- `--mode rpc` keeps the process alive until stdin EOF (rpc-mode.js
  onInputEnd → waitForIdle → shutdown). Experiment
  (scratchpad/pa_exp/rpc_exp.py): a loop-free C.1 cell was executed
  without refusal, the child finished its file in 20 s while the parent
  idled, EOF exited cleanly in 2.4 s. Events carry parent usage
  (`message_end.message.usage{input,output,cacheRead,cacheWrite,
  totalTokens,cost.total}`) and child telemetry (`rlm_child_update.child
  {status,tokenCount,toolUseCount,durationMs,sessionDir}`); the child's
  session JSONL under `sessionDir` carries per-message `usage.cost.total`.
- Spec §10.5 names daemon-owned parents; Annex C.1's cell has no loop.
  The rpc parent is the conformant shape.
- bwrap backend denies network by default and refuses allowlists; a
  prime-agent parent needs model egress, so prime-agent stays
  `isolation: none` (§10.5 Trust row) under the `contained` tier, which
  permits `backend: none` (§17.3). Containment for it is therefore
  detective + advisory, recorded as such (§17.2).

## Workstreams (in order)

### W1 — prime-agent rpc parent (the blocker)
- transport: spawn `<binary> --mode rpc` in the workspace, stdin piped;
  send one `{"type":"prompt","id":…,"message":…}` line; a stdout reader
  thread parses JSONL (bounded line length) into shared telemetry; a
  prompt `response` with `success:false` → `HandleAbsent`-class refusal
  with the error text.
- parent message: run the cell once exactly as written, end the turn; on a
  later worker reply, take no action and end the turn. No keep-alive
  justification text.
- program.rs: Annex C.1 shape only (envelope, `rlm`, handle write). The
  linger/result-poll loop and its config are removed.
- `ParentHandle`: holds stdin; `terminate` closes stdin, bounded grace,
  then the existing bounded stop; new `telemetry()` accessor (zero for
  `NullParentHandle`).
- runner: `Running.payment_so_far` = measured parent + child tokens and
  tool calls, wall since admission; `collect` returns the result with a
  payment that is per-dimension max(reported, measured), child cost summed
  from the child session JSONL (read-only, bounded).
- probe (`doctor --probe-runners`) moves to the same rpc shape.

### W2 — admission failure is a bounded retry, never a crashed run
- `dispatch`: `runner.invoke` → `AdmissionRefused`/`Transport` pins an
  `invocation_admission_failed` record, counts the attempt, and takes
  §11.5 row 2 (backoff); exhaustion escalates with a ticket. Workspace is
  released (already). The run no longer exits 1 on one refusal.

### W3 — honest metering
- controller: charged wall = max(reported, clock elapsed dispatch →
  completion) for every runner.
- in-flight guard: a `Running.payment_so_far` that already exceeds the
  meter's remaining forward capacity cancels the invocation, pins
  `budget_breach_in_flight`, and drives to `budget_exhausted`.

### W4 — true timestamps
- `RecordingAgentRunner` takes the clock; `invocation_admitted` events and
  `role_invocation.admitted_at` carry the admission instant.
- `record_correction_selection` takes `at`; the controller passes
  `clock.now()`; sqlite stores it (export determinism tests adapt).

### W5 — work product: carry forward and deliver
- workspace-fs: per run, a filtered snapshot of the project root
  (`<state>/runs/<run>/integration/`, atomic) seeds every workspace; seed
  manifest kept per workspace. Default excludes: `.git`, `.gauntlet`,
  `.wayfinder`, `.agents`, the state/artifact dirs; plus GAUNTLET.md
  `workspace.exclude`.
- port: `WorkspaceProvider::promote(workspace, deadline)` (default:
  unsupported). The controller promotes a flight whose unit settled,
  before releasing the workspace, and pins `work_product_promoted`
  (changed/deleted paths with digests; conflicts listed, integration kept).
- CLI: `gauntlet deliver [<run_id>] [--apply]` writes
  `<artifacts>/runs/<run>/delivery/` (files + manifest) and, with
  `--apply`, writes changed files into the project root only when each
  touched path still matches the run-start snapshot (drift → refuse,
  paths listed). `gauntlet run` prints the delivery hint at terminal.

### W6 — containment guard (detective)
- A (size, mtime, kind) manifest of the project root (minus `.git` and
  state/artifact dirs) is taken at dispatch and compared at completion;
  differences pin `containment_breach` and add a BLOCKING residual naming
  the paths, so that attempt cannot settle and its retry is told why.
  Invocation records keep `enforcement: advisory` for prime-agent.

### W7 — small correctness items
- schema reminder names the parse failure (bounded, sanitized detail).
- AK-9011 claim: "at the workspace root" wording; re-digest the demo
  reference lock.
- `/api/v1/state` `run`: derive from the latest run, not only tickets.
- doctor workspace-root check uses the runtime `state/workspaces` path.
- flakes: `improve_cmd` baseline_executor tests (shared scratch), the
  unidentified agent-prime adapter failure (loop the suite to find it).

### W8 — docs and status
- demo README/GAUNTLET.md, docs (OPERATOR-GUIDE, CONFIGURATION, cli.md,
  FAILURE-RECOVERY), `.agents/CURRENT_STATUS.yml`, conformance/handoff
  rows touched by W1–W7.

### W9 — live proof
- Checkpoint run after W1+W2 (dispatch reliability alone).
- Final run: `just demo-down && just demo-up`, run the goal, then check
  from stored state: terminal settled; 3 settlements, none re-dispatched;
  every dispatch admitted (no refusal) or refusals retried; nonzero
  payment and decreasing remaining budget; monotone real timestamps;
  `dispatch_truncated` records with an empty `state/workspaces/` after;
  `work_product_promoted` per unit; no `containment_breach`; `gauntlet
  deliver --apply` puts `calculator.html` in the target and
  `scripts/check.sh` passes there. Anything not observed is reported as
  unproven, not claimed.
