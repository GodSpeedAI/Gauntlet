# B1 progress — task 25 (checkpoint contract)

Updated: 2026-09-05, B1 session 1 (later same day). Contract: charter.md.
Ledger: IN-FLIGHT.md.

## Status: PRODUCTION CORRECTION LANDED — e2e_full_loop_real_runner GREEN (2/2)

## Landed this session

1. **Fixture** `tests/fixtures/target-real/` — GAUNTLET.md (cli-agent +
   bwrap + budget block), answer key AK-951 (contained/critic),
   AK-952 (cross_cutting/lead_agent — coupling forbids critic),
   AK-953 (irreversible/human), reference.yaml, frozen reference.lock
   (scripts/reference-digest.sh digests), 3 tier policies, roles,
   real source + check.sh, deterministic child `agent/child.sh`
   (normal/silent/poison modes, all empirically verified).
2. **RED confirmed** before the correction: the run refused with exactly
   the charter's `LiveContainmentDeferred` message (after two honest
   earlier failures the doctor caught: strict-manifest colon in a list
   item; tier/authority coupling on AK-952 — both fixture bugs, fixed in
   the fixture, lock re-frozen).
3. **Production correction** (all gates intact):
   - adapter: `CommandContainment` trait + `ContainmentRefusal`,
     `CliAgentRunner::with_containment`; policy-carrying dispatch spawns
     `["sh","-c",wrapped]` (sandbox = first process); policy WITHOUT seam
     = typed admission refusal; capabilities derive `Isolation::Process`
     iff the seam is installed; no-policy dispatch unchanged verbatim.
   - compose.rs: `govern_posture` returns (GovernDeps, Option<LivePosture>);
     `LiveContainment` seam over the SAME sandbox+policy Arcs; resolve
     policy by ref over dispatchable tiers, validate (re-earned at spawn),
     wrap via sandbox; `wrap_checked_posture` Sandboxed arm routes;
     `LiveContainmentDeferred` variant RETIRED (deleted); with_backend
     ceiling raised to `Authority::LeadAgent` for the live posture
     (coupling forbids critic on cross_cutting; none posture stays Critic).
   - improve_cmd.rs: `runner_for(.., None)` (no live posture of its own).
4. **e2e_full_loop_real_runner.rs GREEN** — leg 1 (wrap reality:
   pid1=bwrap + tmp-write=denied in the store; durable role_invocation
   carries tier-cross-cutting-real-v1; gate_admission evidence with
   tier/policy/rung; AK-953 zero role_invocation records; §16.1
   authority-gap tickets for AK-952) + leg 2 (AK-047: broken backend in
   scratch policy -> "failed validation ... never an unsandboxed
   fallback" ticket; AK-953's own ticket "never an automatic dispatch";
   governance close terminal=blocked; exactly one dispatch, contained).

## FINDINGS FOR THE ORCHESTRATOR/CRITIC (recorded, not patched around)

- **AK-066 semantics finding**: with the frozen §5.3.2 coupling
  (cross_cutting forbids critic authority) AND the frozen §16.1 condition
  3 (runtime can never auto-close lead_agent work), a cross_cutting unit
  in a seeded reference can NEVER settle in an unattended run: its
  closure escalates to the operator, its re-dispatches exhaust the
  rounds, and the honest terminal is budget_exhausted. The Done-when's
  "every blocking Unit settles" is literally satisfiable only for
  critic-authority units. The E2E pins the honest behavior; the ruler
  owner owns the reconciliation (same class as the AK-066 locator
  divergence already recorded).
- **Latent defect exposed (pre-existing, NOT fixed by me — scope)**:
  `RecordingAgentRunner::admitted` (crates/gauntlet-app/src/compare/
  recording.rs, the `RoleInvocationDefinition` literal) hardcodes
  `tier: BlastRadius::Contained` in the DURABLE role_invocation record.
  Unreachable before this task (LiveContainmentDeferred refused every
  live cross_cutting dispatch); now the record misstates the tier for
  cross_cutting dispatches (sandbox_policy_ref IS accurate). The gate
  admission evidence pinned before the invocation carries the correct
  tier/policy/rung and is asserted in the E2E. Needs a proper fix with a
  tier channel into the recording runner — orchestrator's call.
- **compose_teeth leg 4** pins the retiring refusal; re-pin REQUIRED
  (next step, disclosed as a settled-surface edit forced by the charter's
  own retirement instruction).

## Next commands

1. Re-pin compose_teeth leg 4 to the routed world (assert: no deferral
   refusal; run proceeds; gate admission/policy facts on the record).
2. Run e2e_full_loop_mock + p5 regressions.
3. e2e_recovery.rs then e2e_budget.rs (World harness copies, per-target).


## What is settled (verified this session)

- Charter read in full. Prior art read: `e2e_full_loop_mock.rs` (World
  harness, record-derived readers), `tests/fixtures/target/` shape,
  `compose.rs` (LiveContainmentDeferred declared 141/rendered 169/fired
  1001-1013), `gauntlet-adapter-agent-cli` (template render at
  `runner.rs:invoke`, `ProcessSpec.argv` spawned directly, capabilities
  report `Isolation::None`), `gate.rs` (`wrap_builder_command` ->
  `BuilderCommand::Sandboxed(SandboxedCommand{command, policy,
  validation})`; the controller `dispatch` already admits per dispatch at
  controller.rs:2110 and records the admission before invoke at :2128),
  p7/p6 headers (store-level prior art; my targets go through the real
  binary).
- bwrap live on host. EMPIRICAL confinement discriminators verified with
  the production BASELINE_ARGS: inside, `/proc/1/comm` == `bwrap` and
  `/tmp` writes are denied (read-only root); on the host `/proc/1/comm`
  == `systemd` and `/tmp` writes succeed. The fixture child embeds these
  facts in its locator artifact; the E2E asserts the confined tokens from
  the durable evidence store.
- bwrap `render()`: `<binary> <baseline args> <policy args> -- sh -c
  '<command>'` — ONE shell-invocable string, payload single-quoted. Executing
  it via the runner's `ProcessSpec.argv = ["sh", "-c", wrapped]` is the
  intended invocation shape (dash exec's the single simple command, so the
  spawned pid IS bwrap; the transport's SIGTERM/SIGKILL stop hits bwrap).
- Controller dispatch facts: `gate.admit` per dispatch (tier/authority/
  capability/policy/validation conjuncts), admission durably recorded
  before invoke; `spec.sandbox_policy` carries the admitted policy ref
  (`None` = the §8.6 contained/no-backend shape ONLY). Poll error path =
  dispose_transient (retry/backoff); TimedOut = cancel + retry-once +
  escalate; stall detection cancels and disposes row-4.
- AK-050 constraint discovered: `required_isolation(cross_cutting) ==
  Isolation::Process` while the cli runner reports `Isolation::None` —
  so the wrap installation MUST make the runner's capabilities honestly
  report `Isolation::Process` (derived from configuration, same posture
  as `supports_model_selection`), or every cross_cutting dispatch is
  refused at conjunct 4.

## Design decisions (fixed; full reasons go to b1-report.md)

1. **Wrap seam**: a typed `CommandContainment` seam in the cli adapter,
   injected at construction (builder-style method, no signature break).
   At `invoke`: render argv -> POSIX-quote-join -> if the dispatch's
   `sandbox_policy` is Some(ref), the seam MUST produce the wrapped
   command (spawned as `["sh","-c",wrapped]`); None spawns argv verbatim
   (none posture unchanged — mock E2E + contract suites untouched).
   A wrap failure is a typed admission refusal — never an unwrapped
   fallback. R-2 posture revision recorded for the operator.
2. **Composition**: `govern_posture` computed before runner construction;
   when live, the SAME sandbox+policy Arcs back the injected seam;
   `wrap_checked_posture`'s Sandboxed arm stops refusing;
   `LiveContainmentDeferred` variant retired. WrapRefused arms stay.
3. **compose_teeth.rs leg 4** pins the retiring refusal and MUST be
   re-pinned (its premise is retired BY this task). Re-pin planned:
   live backend composes; the run proceeds; every dispatch carries a
   durable gate_admission record naming policy + validation rung; the
   legacy refusal token is gone. DISCLOSED as a deviation (settled-surface
   edit forced by the charter's own retirement instruction).
4. **Fixture units**: AK-951 contained/critic, AK-952 cross_cutting/
   critic, AK-953 irreversible/human (p5's AK-9301 spelling for the
   irreversible unit). Fixture-local `.gauntlet/policies/{contained,
   local,cross_cutting}.json` (doctor check 5 needs the full set),
   `runners.default: cli-agent`, `sandbox.backend: bwrap` (UNK-003
   operator resolution at fixture level), child script `agent/child.sh`
   parses the prompt file (INVOCATION-ID/RUN-ID/ITERATION-NO/ROLE/
   ENVELOPE-DIGEST + unit id from the body), writes the locator artifact
   (embedding the confinement observations) + a strict
   `role_result@v1`/`builder_result@v1` document
   (attempted: [unit-id]; self_checks: [{command, exit_code}]).
5. **Kill boundaries (recovery)**: observed-durable-fact-then-SIGKILL via
   the sqlite test_support readers polled from the test process (the
   store is journal_mode DELETE, busy_timeout 5s — short polling reads
   are safe); assertions written so either landing side (pre-commit or
   post-commit) satisfies exactly-once + identity.

## Next command

Build `tests/fixtures/target-real/`, freeze its lock via
`scripts/reference-digest.sh`, then RED-first E2E leg.

## Checkpoint update (same session, later)

- **e2e_full_loop_real_runner: GREEN 2/2.** compose_teeth leg 4 re-pinned
  (routes now; disclosed); e2e_full_loop_mock 14/14 + p5_hard_gate 6/6
  regression GREEN after the correction.
- **e2e_recovery: GREEN 3/3.** Leg 1: observed first-settlement-boundary
  SIGKILL -> typed stale-writer-lock refusal (COR-R3-3 path modeled:
  operator removes the lock) -> resume -> exactly-once settlement
  (1 event, 1 AK-951 row), preserved run identity, evidence intact.
  Leg 2: produce-window kill (artifact bytes durable, production row
  short) -> resume -> production recovered exactly once. Leg 3: R16-F2
  DISCHARGED — poison child (directory at contract path, TERM-immune)
  -> run reaches bounded budget_exhausted, typed transient_runner_error
  pins, zero settlements; resume terminates too.
- **NEW FINDING (R16-F2 leak, recorded)**: the composed transient
  disposition (§11.5 row 2) never cancels the runner invocation, so the
  hostile child OUTLIVES the run (p8's explicit-cancel retirement is not
  enforced on this path). The leg pins the fact and reaps survivors
  itself (scoped pkill on the world root). Operational hazard proven
  once already this session: leaked chains held the compile.lock fd and
  wedged the next flock (task20/23 lesson shape — reaper added).
- **Needle lesson**: the evidence store pins the frozen reference
  projection EARLY (contains unit ids); store-content needles must use
  artifact-unique tokens ("kind":"child-work-record"), not unit ids.

## Next

e2e_budget.rs (silent child, wall clock, small stall timeout, reserve
intact from checkpoint budget facts), then slice battery.

## FINAL STATE (session complete)

- Slice battery ALL GREEN: just ruler; just check-package gauntlet-cli;
  e2e_full_loop_real_runner 2/2; e2e_recovery 3/3; e2e_budget 1/1;
  e2e_full_loop_mock 14/14; p5_hard_gate; p7_recovery + p6_budget_control;
  just lint; just boundaries 237/237. Combined 29/29 re-run.
- Scanner-driven hardening (disclosed in b1-report): derived script
  paths, named-constant durations, handle-file reaping through the
  fixture child's `reap` mode; boundaries 237/237 after.
- b1-report.md written (the completion contract). e2e-real.md written.
- Probe hygiene proven: repo .gauntlet/policies untouched (original
  mtimes), fixture tree exactly 13 files, no stray processes, no tmp
  debris, nothing staged or committed.
- Session done; no resume state.
