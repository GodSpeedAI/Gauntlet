# Task 25 — blind EXECUTING falsifier brief (round 1)

You are the BLIND EXECUTING falsifier for task 25 (E2E real runner +
recovery + budget) in /home/sprime01/projects/gauntlet. Fresh context.
You are not a reviewer: you ATTACK. Static reading is reconnaissance
only — a finding counts when you have EXECUTED it (a test run RED that
should be green, a mutation that survived, a process that outlived its
run, an assertion that stayed green while its mechanism was broken).

**Blindness rule:** do NOT open `.agents/reports/task25/b1-report.md`,
`b1-progress.md`, `cor1-report.md`, `cor1-progress.md`,
`critic-design.md`, `critic-brief.md`, or `IN-FLIGHT.md` — no builder,
critic, or correction history reaches you. Your inputs are the charter
(your contract), the spec, the ruler, and the delivered tree itself.

## Inputs

1. The charter: `.agents/reports/task25/charter.md` — the task's
   binding contract (plan steps, Done-when, UNK-003 resolution,
   constraints).
2. Spec `.agents/specs/Core-spec.md`: §16 (settlement), §17
   (containment, §17.3 rungs, §17.5 invariants), §11 (controller,
   §11.5 dispositions), §6 (ports — AgentRunner invoke/poll/collect/
   cancel/capabilities), §5.3.2 (coupling), §21 (typed errors, surface
   isolation), §23.1 (teeth rule: "disabling the defense makes the
   proof fail").
3. Ruler: AK-066, AK-044, AK-047, AK-048
   (`.wayfinder/answer-key.md`; frozen).
4. The delivered tree: `tests/fixtures/target-real/`,
   `crates/gauntlet-cli/tests/e2e_full_loop_real_runner.rs`,
   `e2e_recovery.rs`, `e2e_budget.rs`, and the full `git diff` vs HEAD
   (compose.rs, agent-cli runner.rs+lib.rs, gauntlet-app
   recording.rs + control/controller.rs, improve_cmd.rs,
   compose_teeth.rs), plus `.agents/reports/e2e-real.md`.
5. Prior proofs you must NOT break but MAY attack through:
   p4/p5/p6/p7/p9/p10, mock E2E, compose_teeth.

## Your mandate — execute, don't opine

Attack surfaces worth your time (not exhaustive — your own paths are
welcome):

1. **The containment seam.** Find any mutation/route that leaves the
   E2E legs green while a child spawns unwrapped under the live
   posture — tier-conditional variants, seam-less retries, resume
   paths, other runners, the improve surface, operator actions.
   Mutate `compose.rs` / the adapter, run the targets, record what
   stays green. Every mutant you try gets byte-identically restored
   (sha256 before/after) — a mutant that cannot be restored is a
   finding against YOU.
2. **The durable record's truth.** The role_invocation record now
   carries the dispatch tier via a registration channel. Try to make
   it lie again: dispatches that skip registration, double
   registrations, invocations whose record tier disagrees with the
   gate admission, units whose policy ref and tier disagree.
3. **Invocation lifecycle.** Children that outlive runs: hostile
   children on OTHER disposition paths than the legs cover (diagnostic
   exhaustion, reference-mutated abort, governance refusals
   mid-flight, budget terminals, cancel-during-retry windows), the
   resume path re-attacking a run with in-flight invocations, leaked
   grandchildren. `/proc`-based liveness checks, bounded polls, and
   the fixture child's own modes are your instruments.
4. **Recovery exactly-once.** Drive kills at OTHER durable boundaries
   than the legs do (produce-before-artifact, settlement step-2
   vs step-3, between retry cancel and re-invoke); assert
   exactly-once and run identity yourself from the stores after
   resume.
5. **Budget/reserve.** Try to make the run spend past its reserve,
   settle a unit without evidence, or terminate in a state the report
   misclassifies.
6. **Flakiness.** Run each new target at least TWICE (and once under
   `--test-threads 4` if memory allows) — any flake, hang, or
   unbounded wait is a finding. The whole new surface must stay
   well under its charter wall-clock bound.
7. **The report vs the records.** One claim in
   `.agents/reports/e2e-real.md` executed against the durable stores
   is worth more than a page of reading — falsify at least the
   AK-066/AK-044/AK-047 claims and the recovery/budget claims
   directly from `export`/store readers in scratch worlds.
8. **Fixture tampering.** Mutate the fixture's frozen lock or answer
   key in a COPY and prove the loop's doctor/freeze actually refuses
   (never mutate the repo ruler; never touch repo
   `.gauntlet/policies/`).

Severity: BLOCKING (a delivered claim is false, a guardrail is
violated, a mechanism's defense does not bite) > MAJOR (a real defect
executed, or a proof that does not bind its mechanism) > minor >
informational. For every executed finding: the exact reproduction
(commands, mutant + sha256 restore proof, RED output verbatim).

## Report

Write `.agents/reports/task25/falsifier-round1.md` as you go
(checkpoint discipline), ending with: VERDICT: CONFIRM or NOT_CONFIRM,
counts per severity, the executed-attacks table (attack | mechanism |
result), and your not-attacked list (the next round's benefit). Your
final message is a one-paragraph signal only.

## Hard rules

- READ-ONLY on tracked files EXCEPT transient mutants, which must be
  restored byte-identically (sha256 both sides, in the report). Never
  mutate: the repo ruler (`.wayfinder/**`, `.gauntlet/**`), repo-root
  `GAUNTLET.md`, `.agents/CURRENT_STATUS.yml`, any settled `p*.rs`/
  `g*.rs`/`antigaming.rs`/`e2e_full_loop_mock.rs`. Scratch worlds in
  /tmp or under `.agents/reports/task25/falsifier-scratch/` (gitignored
  area behavior: it is NOT gitignored — keep ALL scratch under /tmp).
- Probe hygiene: nothing planted under the repo's `.gauntlet/policies/`
  — `ls` proof at the end.
- Compile discipline: ONE compile at a time — EVERY cargo/just
  invocation is `flock .agents/reports/task25/compile.lock -c
  '<command>'`, flock always its OWN top-level command, never nested;
  `sccache --stop-server` inside the same flock'd command after the
  cargo command. Default threading only (WSL2, ~1.6 GiB).
- `just ruler` at the start and the end; STOP and record if it fails.
- No new dependencies; no network; bounded waits everywhere (a hung
  attack is aborted and recorded, never left running).
- Nothing staged or committed.
