# Task 25 — COR-4 brief (closure-battery correction: the produce-window recovery)

You are the corrections builder for task 25's fourth round in
/home/sprime01/projects/gauntlet. During the ORCHESTRATOR's closure
battery, the full-workspace run executed a REAL defect in the
recovery path your rounds built. Your findings source is the verbatim
failure below plus the leg itself. Read first:
`.agents/reports/task25/cor3-report.md` (the tree state you inherit),
`crates/gauntlet-cli/tests/e2e_recovery.rs` (leg 2), and the spec:
§14 (evidence, content addressing, write-once), §16.3 (atomic commit +
replay idempotency), §18 (recovery).

## The executed failure (verbatim, orchestrator's run 2 of `just test`)

```text
FAIL gauntlet-cli::e2e_recovery the_kill_in_the_produce_window_resumes_with_the_production_recovered
panicked at crates/gauntlet-cli/tests/e2e_recovery.rs:593:5:
assertion `left == right` failed: the locator's production is durable EXACTLY once after the resume:
  [("sha256:d2ef3eaf…", "evidence/real-marker-report.json", 1788694534, 4),
   ("sha256:491b24bf…", "evidence/real-marker-report.json", 1788694544, 11),
   ("sha256:2a3f0bc1…", "evidence/real-local-report.json",  1788694549, 16)]
  left: 2
 right: 1
```

What happened: the kill landed AFTER the first pass's production row
became durable (the leg's own "missed window" branch — it polls for
`artifact_durable && production_rows_below(1)` and otherwise accepts
`artifact_durable` with the comment "the assertions hold either way").
On resume, AK-951 was still open, so the controller RE-DISPATCHED it;
the child rewrote the artifact with DIFFERENT bytes (different
digest); produce minted a SECOND production row for the same locator.
The orphan first production was never adopted. The leg's
exactly-once claim — and the task-12 ledger's "evidence-fs crash
injection" trigger this leg exists to discharge — is violated on this
branch. (The leg passed every prior battery because on an idle host
the kill almost always lands in the in-window branch; the loaded host
widened the effective window. The flakiness is in the kill placement,
NOT in the assertion — the assertion is doing its job.)

## Dispositions (orchestrator-owned, binding)

1. **Diagnose (required, in the report).** Trace the resume path for
   an OPEN unit whose production row is already durable above the
   checkpoint anchor: what does the event-log replay restore, what
   does the controller's produce step key on (per-iteration
   write-once?), and why the second dispatch re-produces instead of
   adopting. Name the exact file:line where the adoption is missing.
   Compare with the mock-world analog (the settled-run
   recovery-completed-production machinery, COR-R4-1's legs in
   `e2e_full_loop_mock.rs`) and the spec's §16.3 replay-idempotency
   rule.
2. **Fix RED-first.** The smallest spec-consistent change that makes
   the leg's exactly-once claim true on BOTH branches: either the
   resume ADOPTS the durable production for an open unit (skip
   re-dispatch; proceed to observation/verification/settlement over
   the adopted ref), or produce is locator-idempotent per run (a
   re-dispatched produce re-binds the EXISTING artifact instead of
   minting a second production). Choose by the spec, and record the
   choice + reason. Constraints: the write-once/no-overwrite evidence
   invariants are NOT weakened; the settlement must pin exactly one
   evidence ref for the unit; p7's crash windows, the mock E2E's
   recovery legs, and the full battery stay green. RED-first: the
   deterministic reproduction first —
   - extend leg 2 (or add leg 2b) to place the kill DETERMINISTICALLY
     in the missed-window branch: poll for `artifact_durable &&
     production_rows_for_unit >= 1`, then SIGKILL; assert exactly-once
     after resume. It must FAIL on the current tree exactly like the
     closure failure, then PASS after the fix.
   - correct leg 2's false comment ("the assertions hold either way")
     to the now-true statement of both branches.
3. **If — and only if — your diagnosis proves the spec actually
   PRESCRIBES re-production on resume** (adoption would contradict a
   frozen rule), STOP before changing behavior, write the case in
   `cor4-report.md`, and leave the fix to the orchestrator: the leg's
   claim would then be a ruler-owner question, not a code defect.
   Do not silently weaken the assertion either way.
4. **Record (no fix):** `just test` run 1's other failure —
   `b1_contract_wins_over_exit7_and_process_is_never_consulted`
   (agent-cli) — passed in isolation (20/20) and in run 2; recorded
   as the load-flaky class (task22 protocol) unless it recurs in your
   battery. The produce-window leg's sub-millisecond window being
   load-sensitive is ALSO recorded in the leg's docs once your fix
   lands (the deterministic leg 2b is the regression pin; the original
   leg keeps racing the real window).

## Constraints (unchanged, binding)

- Never touch: repo-root `.wayfinder/**`, repo `.gauntlet/**`,
  repo-root `GAUNTLET.md`, settled `p*.rs`/`g*.rs`/`antigaming.rs`/
  `e2e_full_loop_mock.rs` (READ them as prior art, never edit),
  `.agents/CURRENT_STATUS.yml`.
- Production changes are limited to the recovery/produce path your
  diagnosis names (crates/gauntlet-app/src/control/** most likely) —
  anything beyond that is a disclosed deviation.
- No new dependencies; no sleep-and-hope (bounded polls); the
  deterministic leg must be timing-robust (it polls a durable fact,
  never a sleep).
- RED-first evidence verbatim. Checkpoint: `cor4-progress.md`.
  Contract: `cor4-report.md`. Final message is a signal only.
- Nothing staged or committed.

## Battery (all green before reporting)

```bash
just ruler
just check-package gauntlet-cli
just check-package gauntlet-app
cargo nextest run --test e2e_recovery
cargo nextest run --test e2e_full_loop_real_runner
cargo nextest run --test e2e_budget
cargo nextest run --test e2e_full_loop_mock
cargo nextest run --test p7_recovery --test p6_budget_control
cargo nextest run --test p4_corrective_loop
cargo nextest run -p gauntlet-adapter-agent-cli
cargo nextest run -p gauntlet-adapter-escalation-fs
just lint
just boundaries
```

Plus, because the defect only showed under full-workspace load: run
`just test` ONCE at the end (it is the orchestrator's closure gate; a
green full run on your final tree is part of your evidence).

## Operational addenda (binding — identical to prior rounds)

- ONE compile at a time: EVERY cargo/just invocation is
  `flock .agents/reports/task25/compile.lock -c '<command>'`, flock
  its OWN top-level command, never nested; `sccache --stop-server`
  inside the same flock'd command. Default threading only.
- `just ruler` before and after every phase; STOP if it fails.
- Probe hygiene; reap every test child; API-outage checkpoint
  discipline.
