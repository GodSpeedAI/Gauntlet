# Task 20 charter — shared context, recorded decisions, guardrails

Every builder, critic, and falsifier for task20 reads this file first. It is
written by the orchestrator. Builder claims have no settlement authority.

## What task20 is

Plan Task 20 (`.agents/plans/gauntlet-impl.md`): `gauntlet` exposes the full
§20.1 command set and completes an end-to-end loop against the mock runner,
from freeze to settlement to report. Ruler: AK-060 (CLI exposes doctor, init,
reference, run, status, inspect, verify, resume, escalations, roles, skills,
improve, export — frozen locator `tests/e2e/cli_surface`), AK-061 (core builds
and completes a full loop with operator surfaces limited to the CLI — frozen
locator `build/cli-only-profile`).

**The loop driver already exists**: `gauntlet_app::control::controller::RunController`
(`compose(deps) -> start_run()/resume_run()`, per-iteration order at
controller.rs:24-38). Task 20 does NOT write a new loop. It wires the
composition root, the command surface, the report, and the E2E proof, and
closes the residuals explicitly triggered by the first composed builder
execution.

## Orchestrator-recorded decisions (fog resolved; critic may attack these)

- **D1 `--dry-run` semantics.** `gauntlet run --dry-run "<intent>"` performs
  the full read-only preflight — doctor must exit OK, config resolves,
  reference parses and `verify_unchanged` passes against the lock, scheduler
  produces a plan preview — and performs ZERO writes: no freeze, no state
  rows, no evidence pins, no runner selection required. Exits 0 with the
  preview on stdout. Rationale: the plan gate runs this from the repo root,
  where `runners.default` is deliberately unset (UNK-003) and any persisted
  artifact would pollute the settled tree.
- **D2 runner selection (UNK-003), fail-closed.** Non-dry-run `gauntlet run`
  requires `runners.default` (GAUNTLET.md `runners.default` / env
  `GAUNTLET_AGENT_RUNNER`); unset or unknown token ⇒ typed configuration
  failure, exit non-zero, no dispatch. The composition roots three runner
  tokens: the prime harness adapter, the generic CLI adapter, and the mock
  adapter (explicitly selected only by operator config; the E2E fixture
  selects it). The repository's own GAUNTLET.md stays unset — UNK-003 remains
  the operator's decision and stays open in CURRENT_STATUS.
- **D3 R-3 discharge shape.** The composition path calls
  `AuthorityGate::wrap_builder_command` after admission, before invocation,
  for process-backed runners (the generic CLI token). Exercised through the
  composition with the recorded `none` backend posture (`BuilderCommand::
  Unwrapped` leg) plus the adapter-level live-bwrap leg that already exists;
  the fully-composed live-containment run remains Task 25's E2E. The report
  records this as a partial discharge of AK-048's BOUNDED CLAIM, honestly.
- **D4 R-2 re-adjudication.** With the wrap-consuming composition in place,
  re-record the task16 D3 adjudication: the CLI adapter still consumes a
  command TEMPLATE from configuration; nothing in its path consumes a
  `SandboxedCommand`. Written into the task report for operator
  re-adjudication; no code change asserted beyond D3.
- **D5 locator handling (AK-060).** The frozen locator is `tests/e2e/
  cli_surface`. First check `crates/gauntlet-arch-tests/src/escapes.rs`'s
  `[[test]] path` rule: if repo-root `tests/e2e/` targets are permitted under
  the same locator-anchored exemption as `tests/contract/`, host the targets
  there and honor the ruler exactly. If the exemption is arch-tests-only,
  host in `crates/gauntlet-cli/tests/` and record the divergence in
  `.agents/reports/reference-candidates.md` (the established AK-063/AK-065
  class). **Never edit the ruler.**
- **D6 AK-061 evidence.** The E2E runs on the default (CLI-only) build —
  there are no operator surfaces beyond the CLI in this tree. Evidence =
  `e2e_full_loop_mock` + `cli_surface` passing on the default profile,
  recorded in the task report; locator `build/cli-only-profile` divergence
  recorded alongside if the artifact lives elsewhere.
- **D7 cohort-slicing guardrail (PINNED record, hard).**
  `controller.rs` compose() pins that §11.3 slicing has NO producer on any
  shipped surface and that the answer-key parser's field set is closed
  (`the_answer_key_cannot_express_a_cohort_on_any_shipped_surface`). Do NOT
  widen the parser, do NOT invent a cohort field, do NOT touch the pinned
  test's meaning. The E2E runs with empty cohorts; its report re-records the
  pinned note with the fixture evidence. Inventing a cohort source is a
  ruler change wearing an implementation's clothes.
- **D8 §20.1 command depth — honest exposure.** Commands with existing
  backing services are real: status, inspect, verify, resume, escalations
  (list/resolve/retry), export, reference (form/freeze/show/diff/verify,
  digest/validate already exist), roles (exists), skills (list/verify real;
  install mirrors roles install with a skills lock), init (scaffold),
  run. `improve` parses its subcommands and returns a typed not-wired
  failure naming plan Task 23 — honest exposure (AK-060 claims exposure),
  never a fake success, never a panic, never a usage error disguised as a
  runtime result.
- **D9 operator actions share one write path.** `escalations resolve/retry`
  go through `SettlementService::settle_operator_action` (the same atomic
  commit sequence as automatic settlement, §16.3) with an operator identity;
  no second writer may appear.
- **D10 R17-note-1.** `write_atomic` gains a directory-entry fsync (locate
  the helper; fix in place with a test). Composition-critical durability is
  the point; do not redesign the helper.

## Guardrails (all roles)

- Never edit `.wayfinder/answer-key.md`, `.wayfinder/reference.yaml`,
  `.gauntlet/reference.lock`, `.gauntlet/roles.lock` content semantics, or
  anything under `.agents/specs/`. `just ruler` must pass before and after.
- Builder/critic/falsifier role boundaries per AGENTS.md: builders do not
  weaken assertions or gates; critics are read-only and blind (no builder
  history); the falsifier EXECUTES attacks.
- Model output and agent claims are untrusted input. Evidence is what tests
  print, not what reports claim.
- Compile discipline: wrap every cargo invocation in
  `flock .agents/reports/task20/compile.lock -c '<command>'` — one compiler
  at a time (WSL2, ~1.6 GiB free memory). Prefer `just check-package` /
  `just test-package` over workspace-wide runs during development.
- Declare-first checkpointing: each builder appends to its progress file a
  `STARTED` line before each item and a `DONE` line after, in
  `.agents/reports/task20/<slice>-progress.md`. A lost final message must
  never lose landed work.
- One task, one commit (orchestrator commits; builders do not commit).
  Never stage `.jolli/`, `.tmp/`, `.agents/prompts/`, operator files.
- Known gate-syntax class: nextest filters are space-joined; use bare test
  names, not `module::path` spellings, when a plan filter matches zero tests.
- A failing test is never disabled to reach green. `INCONCLUSIVE` is never
  `PASS`. Missing evidence is not passing evidence.

## Operational addenda (proven this session, binding on all agents)

- **sccache inherits the flock'd lock fd.** The sccache daemon keeps the
  compile-lock fd open after the lock-holding shell exits, deadlocking every
  later waiter. Protocol: INSIDE the same flock'd command, AFTER the
  cargo/just call, run `sccache --stop-server`. Observed and worked around
  5× on 2026-08-31.
- **Keep tool-call cadence high.** Sub-agents die on a 10-minute inactivity
  timeout when a single synchronous call sits on a long compile. Run long
  cargo/just commands with run_in_background and poll the output file every
  30–60 s; never one synchronous call over ~3 minutes.
- **A "dead" agent's child processes may still be running.** Before editing
  any file another agent may have touched, check for live compile.lock
  holders and running cargo processes; stand off gate-evidence runs rather
  than interleaving edits into them (proven collision-free finish, 2026-08-31).
- **One load-flaky live-process attack test** exists in
  gauntlet-adapter-agent-cli (failed once under full-battery load, passed in
  isolation and in the final full run). Known class; do not "fix" by
  weakening; note recurrence.
