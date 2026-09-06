# Task 25 — COR-1 completion report (corrections round 1)

Written 2026-09-05 by the corrections builder (COR-1). Binding brief:
`cor1-brief.md`; findings ledger: `critic-design.md`; charter:
`charter.md`. Progress ledger: `cor1-progress.md`. Nothing staged,
nothing committed (orchestrator owns commits). `just ruler` PASS before
every phase, mid-run, and after the last phase — it never moved.

## Mission echo

Fix the blind design-as-built critic's findings with the orchestrator's
recorded dispositions: F2 (bind the bwrap confinement evidence to the
AK-952 artifact; RED-first via the tier-conditional seam mutation,
sha256-restored), F3 (thread the real tier through RecordingAgentRunner's
durable record; RED-first via the strengthened E2E record assertion),
F6 (cancel in-flight runner invocations on the composed path's
transient/terminal dispositions via the AgentRunner port's cancel;
RED-first at controller level; p4/p6/p7 and the mock E2E stay green),
F8 (add the missing `local` fixture unit AK-954 and re-freeze the
FIXTURE's own reference lock; the repo ruler untouched), F9 (happy leg
asserts AK-953's escalation ticket too), F1 (rename the overclaiming
test), F4/F5/F10 (wording fixes). F7 RECORD-only. Every production fix
RED-first; full battery green; probe hygiene held.

## Per-finding disposition and evidence

### F2 (MAJOR — FIXED). Confinement evidence bound to the AK-952 artifact

- GAP DEMONSTRATED FIRST: applied the brief's named tier-conditional
  seam mutation to `LiveContainment::wrap` (`crates/gauntlet-cli/src/
  compose.rs`) — return the command verbatim for `BlastRadius::CrossCutting`
  only, wrap `Contained` normally. Ran `cargo nextest run --test
  e2e_full_loop_real_runner` against the mutated tree: **2/2 PASS** —
  the unit-agnostic whole-store scans cannot see the unwrap.
- NEW ASSERTION (happy leg): the stored artifact whose text carries
  `"unit":"AK-952"` and `"kind":"child-work-record"` must itself carry
  `confinement=confined` AND `tmp-write=denied`. RED output under the
  mutation (verbatim):
  ```
  thread 'the_composed_loop_settles_...' panicked at
  crates/gauntlet-cli/tests/e2e_full_loop_real_runner.rs:381:5:
  the AK-952 (cross_cutting) child ran INSIDE the sandbox and observed it —
  its own artifact must carry the verdict: {"unit":"AK-952","tests":1,
  "failures":0,"command":"scripts/check.sh","kind":"child-work-record",
  "check_exit":0,"confinement":"confinement=uncontained pid1=systemd
  tmp-write=ok workspace-write=ok", ...}
  ```
  (`pid1=systemd tmp-write=ok` — the hostile dispatch ran on the HOST,
  exactly the defect the old assertions accepted.)
- RESTORE PROOF: `compose.rs` sha256 before mutation ==
  after restore ==
  `63efd01341fce31b6c33dfeaac00a591bb4aab14d05d09df1d143c918ebc301b`
  (byte-identical; `diff` of the two sha256 outputs: empty).
- GREEN on the real tree: real-runner target 2/2.

### F3 (MAJOR — FIXED). Durable record states the dispatch's REAL tier

- RED: strengthened the happy leg's durable-record assertion to demand
  `"tier":"cross_cutting"` on the AK-952 `role_invocation` record.
  Pre-fix output (verbatim, truncated): `the durable invocation record
  states the dispatch's REAL tier, not a hardcoded default:
  {"record":"role_invocation",...,"workspace_ref":"...AK-952...","tier":
  "contained","sandbox_policy_ref":"tier-cross-cutting-real-v1",...}` —
  the record lied while the policy ref told the truth.
- FIX (smallest typed channel; the port was NOT widened — a
  `RoleInvocationSpec` field was ruled out because the settled p2/p3
  files construct the spec exhaustively and may not be edited):
  - `RecordingAgentRunner` (`crates/gauntlet-app/src/compare/recording.rs`)
    gains `record_dispatch_tier(InvocationId, BlastRadius)` — a typed
    per-invocation registration (RefCell<BTreeMap>; the composed
    controller's recorded single-threaded posture) consumed exactly once
    when `admitted` builds the durable `RoleInvocation`.
    Consumption rules (recorded): tier fact present ⇒ that tier;
    absent + `sandbox_policy: None` ⇒ `Contained` (the ONLY §5.3.5-legal
    shape for a no-policy dispatch); absent + policy present ⇒ typed
    `AgentError::AdmissionRefused` (fail-closed; the invoke path's
    existing cancel-for-recovery arm retires the spawned child).
  - `dispatch_unit` (`crates/gauntlet-app/src/control/controller.rs`)
    registers `admission.tier()` for EVERY dispatch right before
    `invoke` (the gate admission is where the tier fact exists).
- GREEN: real-runner 2/2. Regressions (RecordingAgentRunner users per
  the brief): mock E2E 12/12, p10+p9 8/8, and settled p2 13/13
  (not in the brief list but a RecordingAgentRunner user — run anyway);
  gauntlet-app package 565/565.

### F6 (MAJOR — FIXED). In-flight invocations are cancelled on the composed path

- RED-FIRST at controller level (both tests in
  `crates/gauntlet-app/src/control/controller.rs`'s own test module —
  `ScriptedRunner` gained a cancel ledger):
  - `the_transient_retry_cancels_the_prior_invocation_through_the_port`
    — behaviors [PollError, Complete]; pre-fix `left: []` vs the
    expected prior-invocation id.
  - `a_terminal_reached_with_work_in_flight_cancels_the_stranded_invocation`
    — behaviors [Fail(ContractMismatch), StallForever] over two units;
    pre-fix `left: []`. RECORD: this test initially PASSED for the wrong
    reason — the fixture policy's 30ms quiet window stall-cancels the
    stranded flight on drain pass 1, masking the missing terminal
    cleanup; it was reworked onto `fixture_composed_quiet` (60s window)
    so the stall rule cannot fire inside the test's bounded drain and
    the terminal cancel is what the test isolates.
- FIX (the smallest spec-consistent rule, recorded):
  - `dispose_transient` (§11.5 row 2, poll transport error — fate
    UNKNOWN): the prior invocation is cancelled through the port's own
    idempotent cancel BEFORE the row-2 re-dispatch; `AlreadyTerminal` is
    the typed no-op; a real cancel error propagates. The retirement is
    PINNED evidence: `{"class":"prior_invocation_retired_before_retry",
    ...,"how":"cancelled"|"already_terminal"}`. (Row-3/row-4-adjacent
    arms whose invocation was OBSERVED terminal by the runner —
    Failed/TimedOut — take no new cancel: the runner already retired the
    child at its terminal pin.)
  - `drain` exit: the drain leaves work in flight ONLY when a terminal
    was decided mid-drain; each stranded invocation is then cancelled
    through the port, gets a durable `InvocationTerminal(Cancelled)`
    event, and its reservation is released (mirroring
    `bound_the_drain`), with one pinned
    `{"class":"terminal_stranded_invocations_retired",...}` record.
  - NEW `cancel_operation_bound`: every controller cancel passes a
    deadline at the §11.4 stall-window scale (was `operation_bound` =
    the run's remaining wall horizon — with the fixture's 600s wall a
    SIGTERM-immune child would block ONE graceful stop for up to 600s,
    making the cancel unrunnable and the row-2 retry pace meaningless).
    The backend's stop escalates (SIGKILL + bounded reap) inside the
    same call. All five controller cancel sites use it.
- E2E leg 3 upgrade (`e2e_recovery.rs`): the run's own terminal is
  asserted as before; NEW: the pinned
  `prior_invocation_retired_before_retry` records exist, at least one
  carries `"how":"cancelled"` (the port cancel EXECUTED, not the no-op),
  and a bounded liveness poll (LIVENESS_BOUND=30s, 20ms tick, typed
  panic on expiry) through the fixture child's new mechanical `alive`
  mode (signal 0; the reap mode's symmetric read) proves NO child whose
  id any durable handle file records survives the run or the resume.
  The `reap` hygiene stays — with the fix it has nothing left to reap.
  Leg env tightens `GAUNTLET_STALL_TIMEOUT_MS=2000` (the same knob the
  other legs drive; it now also bounds each cancel's graceful leg at 2s).
- REGRESSION (the brief's hard rule): p4 4/4, p6+p7 7/7, p5 4/4, mock
  E2E 12/12 — the controller's own suite never went red after the fix;
  gauntlet-app package 565/565 (563 prior + the 2 new tests).

### F8 (minor — FIXED). The `local` tier joins the fixture

- AK-954 added to `tests/fixtures/target-real/.wayfinder/answer-key.md`:
  claim (gadget marker + check green over the local-tier dispatch
  workspace), `verify_by: static_check`, `blast_radius: local`,
  `authority: critic`, evidence `{kind: test_report, locator:
  evidence/real-local-report.json, producer: instrument}`.
- `agent/child.sh` locator map gained `AK-954 → evidence/real-local-
  report.json` (and later the `alive` harness mode, see F6).
- `reference.yaml` prose: "three blocking units" → four (contained,
  local, cross_cutting, irreversible); `minimum_reliability` wording
  names the two AUTO-CLOSABLE units honestly (the cross_cutting unit
  cannot settle unattended).
- The FIXTURE's OWN lock re-frozen with `scripts/reference-digest.sh`
  (`.gauntlet/reference.lock`): answer_key_digest
  `sha256:5295e0215f0c228179378d501ef7edf0ef72bab09abaa28eb49c73714c799e81`,
  manifest_digest
  `sha256:d36fc22baf863639e8fc1dc41bb961f852f190c8028177f2270d23b85a9325fe`,
  `unit_count: 4`, annex spelled AK-951..AK-954, note records the COR-1
  re-freeze. THE REPO RULER WAS NEVER TOUCHED (`just ruler` PASS before,
  mid, after).
- Happy leg asserts the local unit like AK-951: report category
  `implemented-and-proven`, durable dispatch record with
  `"tier":"local"` (F3's honesty extends to local), a settlement record
  naming `unit_id AK-954`, per-tier status lines
  (`tier=contained units=1 settled=1`, `tier=local units=1 settled=1`,
  `tier=cross_cutting units=1 settled=0`), and the AK-954 pinned
  artifact carries `confinement=confined` + `tmp-write=denied`.
- Cross-leg updates for the 4-unit fixture (recorded): recovery leg 1
  exactly-once is now TWO settlement events, ONE row per auto-closable
  unit (AK-951 and AK-954); the AK-047 leg proves exactly TWO
  intact-policy dispatches (AK-951 + AK-954, each with its real tier in
  the record) and the broken-policy AK-952 never dispatched; the budget
  leg's never-dispatched scan now also names AK-954. Observed run shape:
  the plan's dimension-priority order (ascending blast risk) dispatches
  AK-951, then AK-954, then AK-952 across the 4 rounds — the terminal
  stays the honest `budget_exhausted`.

### F9 (minor — FIXED). AK-953's ticket stands in the happy run's queue

- RED: the happy leg now finds AK-953's line in `escalations list` and
  demands `irreversible` + `never an automatic dispatch`. Pre-fix
  (verbatim, truncated): `AK-953's escalation ticket stands in this
  run's queue: escalations: ticket=esc-6a9c5db7-0 ... summary="unit
  AK-952 ... requires lead_agent authority for closure ..."` — only
  AK-952 tickets existed.
- FIX (smallest production piece that makes the disposition's assertion
  true; terminal truth untouched): `plan_and_dispatch` runs the
  scheduler's `ineligibility_report` each iteration and mints the
  FIRST-refusal ticket for an OWED unit refused for a scheduler
  GOVERNANCE reason (`Ineligible::is_governance_block`) — text
  `dispatch not scheduled: Unit AK-953 is irreversible: never an
  automatic dispatch candidate — scheduler-level, unconditional (spec
  11.2, 17.1)`. One ticket per unit via a new
  `scheduler_governance_blocked` ledger (classify_no_admission's owed
  filter mirrored: only `None | Open` units not in flight; budget-class
  and concurrency-class refusals never mint). The `blocked`
  classification, `classify_no_admission`, and every terminal are
  byte-for-byte unchanged.
- GREEN: happy leg passes; in the AK-047 leg the queue's first AK-953
  line is the scheduler ticket and its close-out still adds the "run
  blocked:" record (the leg's assertions hold for both texts).
  Regressions for the escalation-surface change: gauntlet-app 565/565,
  p4+p10+p9+p5+p2 29/29, p7+p6+mock 19/19.

### F1 (minor — FIXED). Test rename

`the_composed_loop_settles_every_dispatchable_unit_wrapped_and_escalates_the_irreversible`
→ `the_composed_loop_settles_the_auto_closable_units_wraps_the_cross_cutting_dispatch_and_escalates_the_rest`.
The name no longer claims AK-952 settles; module/leg docs updated to
match.

### F4/F5/F10 (minor — FIXED). Wording

- F4: `compose_teeth.rs` re-pin comment and its `e2e-real.md` mirror now
  say DIFFERENTLY pinned — coverage moved to the routed dispatch; the
  at-locus strength (distinguishing wrapped from unwrapped spawn) lives
  in the real-runner E2E, where the evidence is bound to the dispatched
  unit's own artifact.
- F5: `e2e-real.md` produce-window wording now states the guaranteed
  boundary is ARTIFACT DURABILITY (the in-window branch is explicitly
  optional per the test's own disclosure).
- F10: mock regression count corrected 14/14 → 12/12 (the real count,
  re-verified this round).

### F7 (minor — RECORD-only, per disposition)

The budget leg's terminal is the §11.5 row-4 `escalated` (the silent
unit's second timeout escalates before rounds can exhaust), not plan
step 4's literal `budget_exhausted`; reserve intactness is proven as
booked-rounds-below-ceiling + honest zeros. Disclosed in-test and in
`e2e-real.md`; the exhaustion terminal stays pinned by the settled p6 at
controller level. No change made.

## Design decisions recorded

1. F3's tier channel lives on the CONCRETE `RecordingAgentRunner`
   (which the controller owns) rather than the port or the spec: a
   `RoleInvocationSpec` field would break the exhaustive struct literals
   in settled p2/p3 (never-edit files); a trait method would widen the
   port. The channel is typed end to end, consumed exactly once, and
   fail-closed when a policy-carrying dispatch arrives without a tier
   fact.
2. F6's rule (chosen as the smallest spec-consistent one, per the
   brief): cancel the prior invocation exactly when its fate is UNKNOWN
   (the row-2 poll-error path) before re-dispatching; cancel everything
   still in flight when the run terminalizes. Observed-terminal
   invocations (runner already reported Failed/TimedOut) are not
   re-cancelled — the runner retired the child at its terminal pin.
   Both cancellations pin durable evidence.
3. `cancel_operation_bound` (stall-window scale) replaces
   `operation_bound` for cancels: §6 preamble requires bounded blocking
   control operations; the run's remaining wall horizon was never an
   honest bound for one stop, and with the poison child it made the fix
   unrunnable (600s graceful per cancel).
4. Leg 3's liveness probe runs through the fixture child's new
   mechanical `alive` mode (signal 0) — the argv-contract symmetric read
   of the existing `reap` mode — keeping the test file free of command
   and absolute-path literals (the arch literal scanner's rules, hit
   twice during the round and fixed by construction, see Deviations).
5. F9's ticket is minted at the FIRST plan that skips the owed unit
   (before the rounds gate), so it exists whatever terminal the run
   reaches — the queue is the operator's window, the terminal decision
   is untouched.

## Deliverables (paths + sha256)

| path | sha256 |
|---|---|
| crates/gauntlet-cli/src/compose.rs | 63efd01341fce31b6c33dfeaac00a591bb4aab14d05d09df1d143c918ebc301b (byte-identical to pre-round; only the F2 mutation ever touched it) |
| crates/gauntlet-app/src/compare/recording.rs | 6807fff028341ca76463a6c7b2481509e7de40fc6c5e4c591bd183316a9cf39a |
| crates/gauntlet-app/src/control/controller.rs | 8bf7dbd64a28a94423767cc0601a4dff4aa4efe46c3ff3cdff3ef367a21002e1 |
| crates/gauntlet-cli/tests/e2e_full_loop_real_runner.rs | 3b7e734d87b810c53c7a4f553744118f8db86bf0a9e7e41199197706436c8552 |
| crates/gauntlet-cli/tests/e2e_recovery.rs | ab158c161eb2a63a9156b8462a70d6e364caf975302eb844509a9746b502d44e |
| crates/gauntlet-cli/tests/e2e_budget.rs | 879c051d527ccc062f6faaf0e85b1cd7298f8330d3a7a5253c74313c91e27dd0 |
| crates/gauntlet-cli/tests/compose_teeth.rs | 059b84ffe9c221be5530820eae0a9117e9a3b0040a2188c3e6dd20df63bf662c |
| tests/fixtures/target-real/.wayfinder/answer-key.md | 5295e0215f0c228179378d501ef7edf0ef72bab09abaa28eb49c73714c799e81 |
| tests/fixtures/target-real/.wayfinder/reference.yaml | d36fc22baf863639e8fc1dc41bb961f852f190c8028177f2270d23b85a9325fe |
| tests/fixtures/target-real/.gauntlet/reference.lock | 7dd3a6573f97d0502a0e5615e1a873ac509fcec06f7ebd569b86071c45f7eaf5 |
| tests/fixtures/target-real/.gauntlet/comparison-contract.yaml | 4b3c99529a39655afd704d3c10d5cce22123cf5b17e102524fe5e2f97fcaed92 (untouched) |
| tests/fixtures/target-real/GAUNTLET.md | bfd89ec4343cf3703ff32d87efa501576fb99a4b19818470e408fdf9f3c98534 (untouched) |
| tests/fixtures/target-real/agent/child.sh | e4ce877484a4f82e1ade7b5ed6d9c8bc273d3f7aa5a8243f97905bc343fa93ff |
| tests/fixtures/target-real/scripts/check.sh | dd0c0a41b0af7569b9a248d66fc59c6e115bb806551a232928efc7da452be623 (untouched) |
| tests/fixtures/target-real/src/gadget.rs | 763462831e76e88d0451604d46c8c56346e24e5507ba10a41060bbea12a9d790 (untouched) |
| .agents/reports/e2e-real.md | c432904125272d9b1db6ad8c67e634a56cc628e0104b16b3ced21be24a628b61 |

## Command ledger (every cargo/just invocation, flock'd, in order)

All under `flock .agents/reports/task25/compile.lock -c '...'` with
`sccache --stop-server` inside the same flock'd command (the sccache
"couldn't connect to server" noise after a successful stop is the only
source of nonzero shell exit codes; every cargo/just exit code is the
EXIT_* recorded). Default test threading throughout.

| # | command | outcome |
|---|---|---|
| 1 | just ruler (preflight) | PASS, EXIT_RULER:0 |
| 2 | cargo nextest run --test e2e_full_loop_real_runner (baseline) | 2/2 PASS |
| 3 | (scratch-world probe: run + escalations list + export in /tmp) | F9/F3 facts observed |
| 4 | cargo nextest run --test e2e_full_loop_real_runner (F2 mutation in place) | 2/2 PASS — THE GAP |
| 5 | cargo nextest run --test e2e_full_loop_real_runner (+ F2 assertion, mutation in place) | 1 passed, 1 FAILED — F2 RED (captured) |
| 6 | sha256sum compose.rs (restore proof) + real-runner re-run | identical sha256; 2/2 PASS |
| 7 | cargo nextest run --test e2e_full_loop_real_runner (+ F3 assertion) | 1 passed, 1 FAILED — F3 RED (captured) |
| 8 | just check-package gauntlet-app (after F3 fix) | EXIT 0 |
| 9 | cargo nextest run --test e2e_full_loop_real_runner | 2/2 PASS — F3 GREEN |
| 10 | cargo nextest run --test e2e_full_loop_mock | 12/12 PASS |
| 11 | cargo nextest run --test p10_no_false_learning --test p9_self_improvement_containment | 8/8 PASS |
| 12 | cargo nextest run --test p2_context_isolation | 13/13 PASS |
| 13 | just test-package gauntlet-app | 563/563 PASS |
| 14 | cargo nextest run -p gauntlet-app (the 2 new F6 tests) | 0 passed, 2 failed... first RED run: transient RED + terminal falsely green (stall-masked) |
| 15 | cargo nextest run -p gauntlet-app (fixture_composed_quiet rework) | 2 failed — F6 RED clean (left: [] twice) |
| 16 | cargo nextest run -p gauntlet-app (after F6 fix) | 565/565 PASS |
| 17 | cargo nextest run --test p7_recovery --test p6_budget_control | 7/7 PASS |
| 18 | cargo nextest run --test p4_corrective_loop | 4/4 PASS |
| 19 | cargo nextest run --test e2e_full_loop_mock --test p5_hard_gate | 16/16 PASS |
| 20 | cargo fmt -p gauntlet-app + just check-package gauntlet-app | EXIT 0 |
| 21 | cargo nextest run -p gauntlet-app | 565/565 PASS |
| 22 | cargo nextest run --test e2e_recovery (leg 3 upgrade, pre-F8 fixture) | 3/3 PASS |
| 23 | cargo nextest run --test e2e_full_loop_real_runner (4-unit fixture, first pass) | 2 FAILED — stale assertions (status per-tier; AK-047 leg's 1-dispatch count) → updated per F8 |
| 24 | cargo nextest run --test e2e_full_loop_real_runner | 2/2 PASS |
| 25 | cargo nextest run --test e2e_recovery | 3/3 PASS |
| 26 | cargo nextest run --test e2e_budget | 1/1 PASS |
| 27 | cargo nextest run --test e2e_full_loop_real_runner the_composed_loop... (F9 RED) | FAILED — captured |
| 28 | cargo fmt + real-runner re-run (after F9 fix) | 2/2 PASS |
| 29 | cargo nextest run -p gauntlet-app (F9 regression) | 565/565 PASS |
| 30 | cargo nextest run --test p4/p10/p9/p5/p2 | 29/29 PASS |
| 31 | cargo nextest run --test p7_recovery --test p6_budget_control --test e2e_full_loop_mock | 19/19 PASS |
| 32 | just ruler (mid) | PASS |
| 33 | just check-package gauntlet-cli / gauntlet-app | EXIT 0 / EXIT 0 |
| 34 | e2e_full_loop_real_runner / e2e_recovery / e2e_budget / e2e_full_loop_mock / p5 / p7+p6 / p4+p10+p9 | 2/2, 3/3, 1/1, 12/12, 4/4, 7/7, 12/12 — ALL PASS |
| 35 | just lint (first) | FAILED — fmt on my new lines; cargo fmt -p gauntlet-cli; lint re-run EXIT 0 |
| 36 | e2e_full_loop_real_runner + e2e_recovery (post-fmt re-run) | 5/5 PASS |
| 37 | just boundaries (first) | FAILED — 8 literal violations (my 60_000/_ms const; /proc literals) |
| 38 | const rename + kill -0 via sh | boundaries still FAILED — 'sh' command literal |
| 39 | gauntlet-app 565/565 + recovery leg 3 re-runs after each fix | PASS |
| 40 | fixture child `alive` mode + process_alive through it; just boundaries | 237/237 PASS |
| 41 | FINAL SWEEP: just ruler | PASS |
| 42 | FINAL SWEEP: check-package gauntlet-cli / gauntlet-app | EXIT 0 / EXIT 0 |
| 43 | FINAL SWEEP: e2e_full_loop_real_runner / e2e_recovery / e2e_budget / e2e_full_loop_mock / p5 / p7+p6 / p4+p10+p9 | 2/2, 3/3, 1/1, 12/12, 4/4, 7/7, 12/12 |
| 44 | FINAL SWEEP: just lint | EXIT 0 |
| 45 | FINAL SWEEP: just ruler (after) | PASS |

(boundaries 237/237 ran at step 40 after the last code edit; steps 41-45
touched no code.)

## Deviations and corrections-round notes

1. **Two literal-scanner violations were introduced and fixed during the
   round** (never landed red): my `QUIET_WINDOW_MS` const in the
   controller test module and a `/proc`-based liveness probe in
   e2e_recovery. Final shapes: `QUIET_WINDOW_EXTENDED` (named-constant
   idiom), and the fixture child's `alive` mode (no `/proc`, no command
   literal in the test). `just boundaries` 237/237 after.
2. **The terminal-stranding RED test needed `fixture_composed_quiet`**:
   the default fixture policy's 30ms quiet window stall-cancels a
   stranded flight on the first drain pass, which masked the missing
   terminal cleanup (the test passed for the wrong reason). Documented
   in the test and here; the long-window fixture isolates the terminal
   path.
3. **e2e-real.md**: besides the brief's named lines (117-127 binding
   note, 141-143 F5, 184-188 F4, 105 F10), the report gained a
   "Corrections round 1" section and supersession notes where COR-1's
   fixes made original paragraphs stale (the F3 latent-defect
   paragraph, the R16-F2 RECORDED FINDING, the untested-local-tier
   bullet, the AK-951..AK-953 unit list, the AK-044 ticket quote).
4. **child.sh gained the `alive` harness mode** (fixture data,
   mechanical protocol): required by the arch literal scanner's rules
   for the liveness probe; symmetric to the existing `reap` mode;
   e2e-real.md's mode list updated.
5. **The budget leg's never-dispatched scan was extended with AK-954**
   (F8 cross-leg coherence) — a strengthening, never a weakening.

## Probe hygiene proof

- Repo `.gauntlet/policies/`: exactly `contained.json`,
  `cross_cutting.json`, `local.json`, `.gitkeep` — pre-task mtimes
  (2026-08-14/28), nothing planted:
  `ls -la .gauntlet/policies/` (in the session log).
- `git status --porcelain` over every never-touch path (`.wayfinder/`,
  repo `.gauntlet/`, `GAUNTLET.md`, all settled `p*.rs`,
  `antigaming.rs`, `e2e_full_loop_mock.rs`, `tests/fixtures/target/`,
  `govern/`): EMPTY — untouched.
- `/tmp` scratch worlds: `ls /tmp | grep -c gauntlet-e2e-` → 0 after the
  battery (the harness Drop impls self-destructed everything; the one
  manual probe world was `/tmp/cor1-probe-$$`, removed with its
  directory — nothing planted under the repo).
- Nothing staged (`git diff --cached` empty), nothing committed.

## Battery (final sweep, all green)

```text
just ruler                                              PASS (before/mid/after)
just check-package gauntlet-cli                         EXIT 0
just check-package gauntlet-app                         EXIT 0
cargo nextest run --test e2e_full_loop_real_runner      2/2 PASS
cargo nextest run --test e2e_recovery                   3/3 PASS
cargo nextest run --test e2e_budget                     1/1 PASS
cargo nextest run --test e2e_full_loop_mock             12/12 PASS
cargo nextest run --test p5_hard_gate                   4/4 PASS
cargo nextest run --test p7_recovery --test p6_budget_control  7/7 PASS
cargo nextest run --test p4_corrective_loop --test p10_no_false_learning --test p9_self_improvement_containment  12/12 PASS
just lint                                               EXIT 0
just boundaries                                         237/237 PASS
```

COR-1 is COMPLETE. The tree is ready for the orchestrator's review and
settlement; nothing staged, nothing committed.
