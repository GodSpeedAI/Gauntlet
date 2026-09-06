# Task 25 — blind design-as-built critic brief

You are the BLIND design-as-built critic for task 25 (E2E real runner +
recovery + budget) in /home/sprime01/projects/gauntlet. Fresh context,
READ-ONLY: you modify no tracked file. Running tests and inspection
commands is allowed (compile discipline at the bottom). You have no
builder history by design — judge only the delivered tree.

**Blindness rule:** do NOT open `.agents/reports/task25/b1-report.md`,
`b1-progress.md`, or `IN-FLIGHT.md` — those carry the builder's
reasoning. Everything else under `.agents/reports/task25/` (charter,
this brief) is your contract. Do not open
`.agents/reports/task25/compile.lock`.

## Your inputs

1. The charter: `.agents/reports/task25/charter.md` (mission, plan
   steps + gate verbatim, the UNK-003 operator-delegated runner
   resolution, 15 recon facts, deliverables, binding constraints).
2. Spec `.agents/specs/Core-spec.md`: §16 (settlement: the five
   preconditions, the atomic commit, operator actions), §17
   (containment: the §17.3 rung table, §17.5 invariants), §11
   (controller: §11.2 selection, the §11.5 disposition rows), §6.1
   (AgentRunner port), §21.2/§21.3 (typed errors, surface-failure
   isolation), §5.3.2 (Unit schema + tier/authority coupling), §8.5
   (doctor checks).
3. Ruler units (frozen — never edit): `.wayfinder/answer-key.md`
   AK-066 (primary), AK-044, AK-047, AK-048 (real-world confirmation
   legs).
4. The delivered tree:
   - `tests/fixtures/target-real/` (the seeded fixture: reference,
     frozen lock, policies, source, check script, child agent)
   - `crates/gauntlet-cli/tests/e2e_full_loop_real_runner.rs`
   - `crates/gauntlet-cli/tests/e2e_recovery.rs`
   - `crates/gauntlet-cli/tests/e2e_budget.rs`
   - `git diff` on `crates/gauntlet-cli/src/compose.rs`,
     `crates/adapters/gauntlet-adapter-agent-cli/src/{runner.rs,lib.rs}`,
     `crates/gauntlet-cli/src/improve_cmd.rs`, and
     `crates/gauntlet-cli/tests/compose_teeth.rs` (a settled-surface
     edit — see question 6)
   - `.agents/reports/e2e-real.md` (the run report)
5. Prior art / context: `crates/gauntlet-cli/tests/e2e_full_loop_mock.rs`
   and `crates/gauntlet-cli/tests/p7_recovery.rs` /
   `p6_budget_control.rs` (the store-level proofs the E2E targets
   extend — the E2E must not weaken or merely duplicate them),
   `crates/gauntlet-app/src/govern/gate.rs` (the gate whose verdict is
   routed), `crates/gauntlet-app/src/settle/mod.rs` (§16.1 enforcement
   incl. the ClosingActor authority matrix),
   `crates/gauntlet-app/src/control/controller.rs` (the unattended
   settlement path), `.gauntlet/policies/*.json` (repo policies),
   `scripts/reference-digest.sh`.

## What to adjudicate (compare design-as-built against the contract)

1. **AK-066 fidelity.** Does the happy leg prove the frozen claim —
   an E2E run against the seeded bounded task settling with preserved
   evidence, using a real AgentRunner adapter? Trace what the run
   actually does with EACH fixture unit (AK-951/952/953) against §5.3.2
   (coupling), §16.1 (all five conditions; the ClosingActor authority
   matrix), and the §11.5 disposition rows. Flag anywhere a test name,
   assertion message, or report sentence claims "settled" for a unit
   whose durable record says otherwise, and any place the E2E's
   Done-when coverage is thinner than the plan's Done-when sentence.
   Judge whether the fixture+test resolution of the
   settleability question is the honest one (spec-correct behavior
   pinned, divergence recorded) or a dodged one.
2. **AK-048 reality (wrap from process start).** On the live posture,
   is EVERY dispatched child spawned inside the sandbox, with the wrap
   decided after admission and before any process start? Hunt for any
   path that could spawn unwrapped while any record/report claims
   containment — including: seam not installed, seam bypassed on
   retry, mock/other runners under a live posture, the improve surface,
   resume paths. Is the "child observes its own confinement" leg a
   genuine inside-sandbox observation (the child proves bwrap bounds
   it) or a tautology the fixture child could produce unconfined?
3. **AK-047 (validate-before-execute, no unsandboxed fallback).** Is
   policy validation re-run per dispatch (or otherwise proof against a
   policy changing between admission and spawn)? When validation or
   wrap fails, does the composed path escalate fail-closed — never run
   unwrapped, never degrade to none? Check both the composition-time
   check and the per-dispatch seam.
4. **AK-044 (irreversible never dispatched).** Is the never-dispatch
   proven from durable records (absence of invocation/dispatch rows for
   the unit) rather than report wording? Does the unit appear in the
   escalation queue as the plan's Done-when requires?
5. **The production correction.** `git diff compose.rs` +
   `runner.rs` + `lib.rs` + `improve_cmd.rs`: is retiring the task-20
   deferral the smallest typed change? Is the new seam a semantic
   capability (ports-clean, `just boundaries` still meaningful) or a
   leak? Do the fail-closed refusals that existed before still fire
   (bad template, capability-vs-tier mismatch, wrap refusal)? Does any
   previously-refusing route now silently degrade? Is the seam's
   interaction with `BuilderCommand` and the gate's admission record
   spec-clean (§16.2: authority is not containment; §17.5)?
6. **The settled-surface edit** (`compose_teeth.rs`, leg 4). The
   charter ordered the deferral retired, which this leg had pinned.
   Adjudicate: is the replacement leg an honest re-pin (the never-
   unwrap invariant, stronger locus) or a weakening? Mutation-think:
   what single mutation to the routing would keep the new assertions
   green that the old leg would have caught? Is the disclosure comment
   adequate?
7. **The recovery target.** Is the mid-settlement SIGKILL taken at a
   genuinely OBSERVED durable boundary (polled durable fact, no
   sleep-and-hope)? Are the exactly-once + run-identity assertions
   airtight for BOTH possible kill sides? Is the produce-window leg a
   real additional window? For the contract-path-poisoning leg (the
   recorded R16-F2 class): does the run reach a bounded honest
   terminal, is the resume liveness real — and trace what happens to
   the runner INVOCATION and child PROCESS at that terminal: is any
   child left alive after the run ends, is the port's cancel contract
   (P8) enforced on this composed path, and is whatever happens
   asserted or merely survived?
8. **The budget target.** Is the non-converging unit honest (a child
   that never yields a valid result)? Are the reserve-intact and
   clean-termination assertions derived from durable checkpoint facts
   rather than the run's printed claims? Any timing fragility (a leg
   that could flake on a loaded host)?
9. **The run report** (`.agents/reports/e2e-real.md`): every claim
   record-derived and consistent with the tests; the deterministic-
   child posture stated plainly; anything overclaimed, projected
   rather than executed, or silently missing (a leg the charter
   requires that has no evidence)?
10. **Fixture hygiene.** Frozen lock digests actually match the
    normalization (`scripts/reference-digest.sh` over the two files);
    the three policies complete and irreversibly absent; unit set
    validates against §5.3.2; the child agent is fixture data (no
    orchestration logic); no debris; never-touch paths untouched
    (`git status`).
11. **Regressions.** Run: `e2e_full_loop_mock`, `p5_hard_gate`,
    `p7_recovery`, `p6_budget_control`, `just lint`, `just boundaries`,
    `just ruler`. All must be green; any red is a BLOCKING finding.
12. **improve_cmd.rs.** The diff adds a seam argument (None) to the
    improve surface. Judge: honest (improve fronts no live posture and
    this keeps it that way) or a silent posture change?

Severity scale: BLOCKING (the claim is false/unproven or a guardrail
is violated) > MAJOR (the proof does not bind to the mechanism, or a
real defect is exposed) > minor > informational. For every finding,
name the exact file:line and the test or mutation that demonstrates
it. If you run tests to adjudicate, obey the compile discipline below
and restore anything you mutate byte-identically (sha256 before/after).

## Report

Write your adjudication to `.agents/reports/task25/critic-design.md`
(first: the report file; then your final message is a one-paragraph
signal: verdict CONFIRM / NOT_CONFIRM + counts per severity). The
report file is the contract; a lost final message must lose nothing.

## Compile discipline (binding)

- ONE compile at a time: EVERY cargo/just invocation is
  `flock .agents/reports/task25/compile.lock -c '<command>'` — flock is
  its OWN top-level command, NEVER nested inside another flock's -c
  string. Run `sccache --stop-server` inside the same flock'd command
  after the cargo command completes.
- Memory is tight (~1.6 GiB): default test threading only; run the
  larger targets one at a time.
- If `just ruler` ever fails: STOP and record immediately.
- Do not commit or stage anything. Do not modify tracked files. If you
  plant any scratch file anywhere under the repo, remove it and prove
  with `ls` in your report (probe hygiene).
