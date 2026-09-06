# Task 25 charter — E2E real runner + recovery + budget

**Plan:** `.agents/plans/gauntlet-impl.md` §Task 25 (phase 7).
**Ruler units:** AK-066 ("An end-to-end run against a seeded bounded
task settles with preserved evidence for every blocking Unit, using a
real AgentRunner adapter." — `verify_by: test`, `blast_radius:
cross_cutting`, `authority: lead_agent`,
`evidence_required: {kind: test_report, locator: tests/e2e/full_loop_real_runner, producer: instrument}`)
plus real-world confirmation of AK-044 (irreversible never an automatic
dispatch candidate), AK-047 (policy validation precedes execution;
validation failure escalates, never unsandboxed fallback), AK-048
(containment wraps the builder session from process start).
**Tasks 26 (baselines) and 27 (conformance/handoff) are OUT OF SCOPE.**

**Plan steps (verbatim):**

> 1. Seed a bounded task in the fixture target project whose reference
>    has at least one Unit per tier up to `cross_cutting`, plus one
>    `irreversible` Unit that must escalate rather than dispatch.
> 2. Run with the real adapter and a real containment backend; preserve
>    the full evidence bundle.
> 3. Interrupt mid-settlement; resume; assert exactly-once settlement
>    and preserved run identity.
> 4. Force budget exhaustion on a non-converging Unit; assert clean
>    termination with the recovery reserve intact.
> 5. Record the run report to `.agents/reports/e2e-real.md`.

**Plan gate (verbatim):**

```bash
cargo nextest run --test e2e_full_loop_real_runner --test e2e_recovery --test e2e_budget && \
just ruler
```

**Plan Done-when (verbatim):** every blocking Unit of the seeded
reference settles with pinned instrument evidence; the `irreversible`
Unit appears in the escalation queue and was never dispatched (AK-066,
plus real-world confirmation of AK-044, AK-047, AK-048).

**Locator reconciliation (recorded class, decide now):** the plan gate
names cargo targets `e2e_full_loop_real_runner` / `e2e_recovery` /
`e2e_budget`; AK-066's frozen locator spells
`tests/e2e/full_loop_real_runner`. Materialize the plan's target
spellings (the gate must run verbatim) under
`crates/gauntlet-cli/tests/`; the orchestrator records the AK-066
locator divergence in reference-candidates at settlement (AK-049/AK-063
precedent). Never edit the ruler to reconcile.

## UNK-003 resolution for this task (operator-delegated, recorded)

The frozen reference leaves `runners.default` at the repository root an
open operator question (UNK-003). The operator has instructed this
session to take task 25 to settlement, which forces a runner choice for
the real-runner E2E. Resolution, same class as the task20 fixture: the
E2E FIXTURE's own GAUNTLET.md selects `runners.default: cli-agent` —
explicit operator configuration at the fixture level (the task20
fixture selects `mock-agent` the same way). The repository root's
GAUNTLET.md stays unset; UNK-003 itself remains open with terminal
disposition at Task 27. "Real AgentRunner adapter" (AK-066's claim) is
satisfied by the generic CLI adapter — a REAL runner proven by P8
(AK-013/AK-014) — driven by a deterministic fixture child for
CI-practicality; the adapter, transport, contract delivery, poll
classification, deadline and cancel mechanics are all the real
production code paths. State this posture honestly in every report and
in `e2e-real.md`; a deterministic child under the real adapter is NOT a
frontier-model claim and must never be worded as one.

## Recon facts (verified by the orchestrator 2026-09-05 — do not re-derive)

1. **The live-containment deferral you are retiring.**
   `crates/gauntlet-cli/src/compose.rs`: `CompositionError::LiveContainmentDeferred`
   (declared ~line 141, rendered ~169, fired ~1001-1006): the
   composition-time wrap check runs `gate.wrap_builder_command` for the
   `cli-agent` token and REFUSES the run when the verdict is
   `BuilderCommand::Sandboxed(_)` ("routing a wrapped builder command
   through the composed loop is plan Task 25's leg — refusing here
   rather than running unwrapped"). Task20 closure, R-3: "with a live
   backend the composition refuses fail-closed (LiveContainmentDeferred
   — recorded Task 25 leg, never a silent unwrap). The fully-composed
   LIVE-containment run remains Task 25's E2E." Your production
   correction replaces the refusal with real routing of the wrapped
   command to the spawned child.
2. **The wrap surface.** `gauntlet_app::govern::AuthorityGate::
   wrap_builder_command(&govern, &admission, &command, deadline)` →
   `BuilderCommand::Unwrapped{..} | Sandboxed(_) | Err(refusal)`
   (`crates/gauntlet-app/src/govern/gate.rs`). `admit(...)` precedes it.
   The bwrap adapter lives in `crates/adapters/gauntlet-adapter-sandbox-bwrap/`;
   the tier policies in `.gauntlet/policies/{contained,cross_cutting,local}.json`
   — `cross_cutting.json` names `"backend": "bwrap"`, network
   deny-by-default, `$WORKSPACE_ROOT` writable, the usual denied_paths.
   There is NO `irreversible` policy file (by design; AK-049).
3. **The adapter you will drive.** `crates/adapters/gauntlet-adapter-agent-cli/`:
   runner token `cli-agent` (`lib.rs:74`), command template env
   `GAUNTLET_CLI_AGENT_CMD` (`lib.rs:77`), placeholders `{prompt_file}`,
   `{workspace}`, `{result_path}`, `{model}` (`template.rs:41-49`),
   result contract env `GAUNTLET_RESULT_PATH` (`lib.rs:82`), template
   parse errors typed; the doctor's cli-runner probe is parse-AND-render
   only (`probe_template`, `crates/gauntlet-cli/src/context.rs:184`) —
   no child spawn. R-2 posture (task20 closure): this adapter consumes
   only a command TEMPLATE, never a wrapped SandboxedCommand. Your wrap
   routing must respect or consciously revise that posture — if revised,
   record the re-adjudication in your report for the operator.
4. **The E2E harness prior art.** `crates/gauntlet-cli/tests/e2e_full_loop_mock.rs`
   (task20, SETTLED — never edit it): `World` scratch-copy harness,
   drives the REAL binary (`env!("CARGO_BIN_EXE_gauntlet")`) with per-leg
   env (`GAUNTLET_STATE_DIR`, `GAUNTLET_ARTIFACT_DIR`,
   `GAUNTLET_ID_SEED`, `GAUNTLET_CLOCK=deterministic`,
   `GAUNTLET_STALL_TIMEOUT_MS`, `GAUNTLET_RETRY_*`,
   `GAUNTLET_MAX_ATTEMPTS_PER_UNIT`, `GAUNTLET_TOOL_TEST`),
   record-derived assertions, durable-store readers
   (`store_holds_exact_objects`, `records_containing`),
   `gauntlet_adapter_state_sqlite::test_support` damage/crash
   instruments (`delete_production_rows`,
   `regress_latest_checkpoint_offset`, `evidence_produced_events`,
   `evidence_production_rows`). Copy its STYLE, not its file.
5. **The fixture prior art.** `tests/fixtures/target/` (task20's seeded
   project): its own GAUNTLET.md front matter (`runners.default:
   mock-agent`, budget block, `budget.max_parallel: 1` with the
   recorded write-once observation-binding reason), its own
   `.wayfinder/{answer-key.md,reference.yaml}` (AK-901..903, all
   `contained`/`critic`), its own frozen
   `.gauntlet/{reference.lock,comparison-contract.yaml}`, one source
   file + one real check script. Your new fixture replicates this
   SHAPE. The existing fixture is settled surface — leave it untouched;
   the mock E2E must stay green after your production change.
6. **Store-level recovery prior art.** `crates/gauntlet-cli/tests/p7_recovery.rs`
   (task13) covers all §16.3 crash windows with the real `crash_helper`
   child + fresh controller recomposition, at the STORE level with a
   proof-local runner. `crates/gauntlet-cli/tests/p6_budget_control.rs`
   (task12) proves reserve/non-convergence at the controller level.
   Both are SETTLED — your E2E targets go through the REAL BINARY
   (`gauntlet run` / `gauntlet resume` as child processes), which is the
   delta task25 exists for. Never weaken them; run them as regression.
7. **R16-F2 (trigger discharges HERE).** Task16 closure §4: "a hostile
   child can poison its own contract path (directory at
   `GAUNTLET_RESULT_PATH`) and stay alive — poll returns the typed
   Transport error forever, `TimedOut` unreachable, runner never
   self-stops; explicit cancel retires it (executed, worked). Deliberate
   §21.2 posture (uncertainty never becomes success). Trigger: next
   transport/lifecycle touch or task25's composed recovery E2E." Your
   `e2e_recovery` must contain a leg that EXECUTES this class against
   the composed loop: a child that plants a directory at its result
   path and lives; the run must reach a BOUNDED honest terminal (the
   stall/deadline/cancel machinery, not a hang), and a subsequent
   resume of that run must terminate (liveness preserved). Record the
   observed mechanics; if the composed loop actually hangs, that is a
   REAL defect — fix RED-first, disclose prominently.
8. **Evidence-fs crash injection lead (task12 ledger, trigger task25).**
   `.agents/reports/task12/residual-adjudication.md`: "evidence-fs crash
   injection" named with Task-25 trigger. Cover a produce-side window in
   `e2e_recovery` (interruption between the evidence artifact write and
   its durable production record) in ADDITION to the settlement-commit
   windows, using the observed-durable-fact-then-SIGKILL shape (fact 9).
9. **Deterministic mid-settlement interruption.** Do NOT sleep-and-kill.
   Kill at an OBSERVED durable boundary: start `gauntlet run` as a child,
   poll the state DB / event log (the
   `gauntlet_adapter_state_sqlite::test_support` readers, or the
   on-disk sqlite) until the target window's durable fact appears, then
   SIGKILL the binary. Either reachable sub-state (killed pre-commit or
   post-commit) must satisfy the assertions — assert exactly-once
   settlement and preserved run identity AFTER `gauntlet resume`
   regardless of which side the kill landed on.
10. **Budget control surface.** Budget block keys in GAUNTLET.md
    (`budget.max_rounds`, `max_wall_seconds`, `max_tokens`,
    `max_cost_usd`, `recovery_reserve_pct`, `diagnostic_budget`) +
    `GAUNTLET_MAX_ATTEMPTS_PER_UNIT`, `GAUNTLET_STALL_TIMEOUT_MS`,
    `GAUNTLET_RETRY_*` env hooks (the mock E2E sets them). The reserve
    machinery is `MeteredBudget` / `would_breach_reserve` (task12).
    A non-converging unit = a child that never writes a valid result
    document (silent child) — the honest non-convergence shape; a lying
    child that loops failing results is a second acceptable leg.
11. **Escalation queue spelling.** The CLI exposes `gauntlet escalations`
    (`crates/gauntlet-cli/src/escalations_cmd.rs`; the mock E2E and
    `escalations_cli.rs` are the proven callers). The irreversible unit
    must appear there, never dispatched: assert via the escalation
    listing AND the absence of any dispatch/invocation record for that
    unit in the durable store/export.
12. **bwrap is live on this host** (bubblewrap 0.11.1; verified
    2026-09-05). The sandbox-none default refuses cross_cutting dispatch
    (UNK-001 posture) — the fixture therefore sets the real backend.
    Check `crates/gauntlet-app/src/config/` for the exact
    `sandbox.backend` / `sandbox.policy_dir` key spellings
    (`resolve.rs:152-153` names them `sandbox.backend`,
    `sandbox.policy_dir`).
13. **AK-047 leg.** Policy validation precedes execution and a
    validation failure escalates without unsandboxed fallback — your
    real-runner target needs a leg that breaks containment validation
    in the fixture world (e.g. an invalid/mutated policy copy in the
    SCRATCH world, never in-repo) and asserts escalation + no execution,
    not a degraded run. `.gauntlet/policies/` probe hygiene: any file
    you plant for a test lives in the scratch world; NOTHING is ever
    planted under the repo's `.gauntlet/policies/` (task14 warning: a
    stray `irreversible.json` once made `just doctor` red on main).
14. **Wall-clock budget.** Whole new targets under ~10 minutes
    combined on this host. bwrap spawn cost is tens of milliseconds;
    deterministic children sleep only in the legs that need liveness
    (poisoned child), bounded by the leg's own stall timeout.
15. **Doctor runs inside `gauntlet run`.** The composed run's doctor
    phase must pass in the fixture world (cli-agent selected,
    template set, bwrap posture, roles present). If the doctor phase
    refuses an honest bwrap+cli-agent fixture world, that is a finding
    to record and fix through a correction, never a doctor bypass.

## Deliverables (all paths relative to repo root)

1. **The fixture**: `tests/fixtures/target-real/` — a seeded bounded
   project whose frozen reference carries at least: one `contained`
   unit, one `cross_cutting` unit, one `irreversible` unit (authority
   `human`). Its own GAUNTLET.md (`runners.default: cli-agent`,
   `sandbox.backend: bwrap`, policy dir, budget block sized for the
   E2E legs), its own `.wayfinder/{answer-key.md,reference.yaml}`, its
   own frozen `.gauntlet/reference.lock` + comparison contract (the
   scripts/reference-digest.sh normalization, same as the existing
   fixture), real source + real check script(s) the instruments
   execute, and the deterministic child agent script(s) the command
   template drives (reads the prompt file, performs the bounded
   artifact writes in the workspace, writes a valid `role_result@v1`
   document at the result path). Fixtures are data: no logic beyond the
   child's mechanical protocol.
2. **The production correction** (RED-first): retire
   `LiveContainmentDeferred` by routing the gate's `Sandboxed` verdict
   to the actually-spawned child — the bwrap wrap happens between
   admission and process start (AK-048), every dispatch, for as long as
   the posture demands it. Gate semantics, tier/authority coupling, and
   the capability-vs-tier check are NEVER weakened; p5_hard_gate and
   g7 stay green unchanged. If the correct seam requires the cli
   adapter's R-2 posture to move (template-only consumption), implement
   the smallest typed seam and record the re-adjudication.
3. **`crates/gauntlet-cli/tests/e2e_full_loop_real_runner.rs`** — the
   happy leg through the real binary on the new fixture: every blocking
   unit settles with pinned instrument evidence (durable-store
   assertions, the mock-E2E reader style); the cross_cutting unit
   dispatches WRAPPED (assert the wrapped execution actually happened —
   e.g. the child observes its own confinement, and the wrap verdict is
   recorded in the durable record); the `irreversible` unit appears in
   the escalation queue and was never dispatched (AK-044); a
   policy-validation-failure leg escalates without unsandboxed fallback
   (AK-047); report classification derived from records.
4. **`crates/gauntlet-cli/tests/e2e_recovery.rs`** — through the real
   binary: observed-boundary SIGKILL mid-settlement; `gauntlet resume`
   completes; exactly-once settlement, preserved run identity, prior
   evidence intact (AK-040/AK-042 real-world); the produce-window leg
   (fact 8); the R16-F2 contract-path-poisoning liveness leg (fact 7).
5. **`crates/gauntlet-cli/tests/e2e_budget.rs`** — through the real
   binary: a non-converging (silent) unit under a budget configured to
   exhaust; clean typed termination (non-zero exit, honest
   classification, zero settlements), the recovery reserve intact
   (assert the reserve was never spent — the record-derived budget
   facts), no crash, no hang (AK-037..AK-039/AK-051/AK-052 real-world).
6. **`.agents/reports/e2e-real.md`** — the run report (plan step 5):
   what ran, on which fixture, with which runner/containment posture,
   the honest deterministic-child posture statement (see UNK-003
   section), gate outputs, and any legs recorded as
   configured-but-untested or out of scope. Written from the ACTUAL
   runs you executed, never projected.
7. **Reports**: `b1-progress.md` at every checkpoint; `b1-report.md` at
   completion — mission echo, deliverable paths + sha256, every command
   you ran + outcome, deviations, the design decisions you made (wrap
   seam, kill boundaries, fixture unit set) with reasons, and open
   findings for the critic.

## Constraints (binding)

- **Never touch**: `.wayfinder/**`, `.gauntlet/reference.lock`,
  the repo-root `GAUNTLET.md`, settled proof files (`p*.rs`, `g*.rs`,
  `e2e_full_loop_mock.rs`, `antigaming.rs`, ...), `CURRENT_STATUS.yml`
  (orchestrator-owned), the existing `tests/fixtures/target/`.
- **Never weaken** an existing gate, test, or assertion to make an E2E
  leg pass. A failing prior proof after your change is a REAL defect in
  your change: fix the change, not the proof.
- **No new dependencies.** No network in tests. No frontier-model calls.
- **No timing-fragile assertions** (no sleep-and-hope); poll durable
  facts. Any retry/wait loop carries a bounded deadline and a typed
  failure.
- **Honest status language** everywhere: a leg you could not run is
  recorded `configured-but-untested`, never silently dropped.
- **Probe hygiene**: files planted for a leg live only in scratch
  worlds; `ls` proof in your report that nothing was left under the
  repo's `.gauntlet/policies/` or the fixture source dir.
- **Do not stage** `.jolli/**`, `.tmp/**`, `.agents/prompts/**`,
  `.agents/agents/**`, `mutants.out/**`. You do NOT commit — the
  orchestrator owns commits; leave the tree ready.

## Slice battery (run before reporting completion; all must be green)

```bash
just ruler
just check-package gauntlet-cli
cargo nextest run --test e2e_full_loop_real_runner
cargo nextest run --test e2e_recovery
cargo nextest run --test e2e_budget
cargo nextest run --test e2e_full_loop_mock          # regression: untouched behavior
cargo nextest run --test p5_hard_gate                # regression: gate teeth
cargo nextest run --test p7_recovery --test p6_budget_control   # regression
just lint
just boundaries
```

(`just check` + `just test` workspace-wide are the ORCHESTRATOR's
closure battery.)

## Operational addenda (binding)

- **One compile at a time.** EVERY cargo/just invocation goes under
  `flock .agents/reports/task25/compile.lock -c '<command>'` — and
  `flock` is always its OWN top-level command, NEVER nested inside
  another flock's `-c` string (a nested flock self-deadlocks; two
  builders died of this at task23). Run `sccache --stop-server` INSIDE
  the same flock'd command after the cargo command completes, or the
  daemon holds the lock fd forever (proven at task20).
- **Memory**: WSL2, ~1.6 GiB free. Do not raise nextest thread counts;
  run the three new targets sequentially, not in one invocation.
- **Checkpoint as you go**: `b1-progress.md` after the fixture, after
  the production correction, after each target lands. A session death
  must lose nothing (task4 lesson — 27 interruption instances to date).
- **Report to files, not messages.** Your final message is a signal;
  `b1-report.md` is the contract.
- **The ruler never moves.** `just ruler` before and after every phase;
  if it ever fails, STOP and report immediately.
- If you are blocked by a usage window or environment death, write the
  exact resume state into `b1-progress.md` (what passed, what was
  mid-flight, the next command) and stop cleanly.
