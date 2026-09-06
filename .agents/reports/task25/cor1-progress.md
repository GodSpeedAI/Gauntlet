# Task 25 — COR-1 progress checkpoint

Updated: 2026-09-05 (corrections builder COR-1). Contract file: `cor1-report.md`
(written at completion). Nothing staged, nothing committed at any point.

## Preflight (DONE)

- `git status`: builder's task-25 tree as the critic saw it (compose.rs,
  agent-cli runner/lib, improve_cmd, compose_teeth modified; the three new
  e2e targets + fixture untracked; CURRENT_STATUS.yml orchestrator-owned).
- `just ruler` (flock'd): PASS (EXIT_RULER:0).
- Baseline `cargo nextest run --test e2e_full_loop_real_runner`: 2/2 PASS.
- Empirical probe (scratch world under /tmp, replica of the happy leg):
  - the happy run's escalation queue holds ONLY AK-952 tickets — AK-953's
    ticket is ABSENT today (F9's assertion therefore needs the smallest
    production piece: a first-refusal ticket when the plan skips the
    irreversible unit; terminal truth untouched);
  - all 3 `role_invocation` records carry `"tier":"contained"` (F3 live);
  - happy terminal: `budget_exhausted iterations=3`, AK-952 dispatched twice.
- Recon decisions recorded (full detail in cor1-report.md):
  - F3 channel: typed per-invocation tier registration on the CONCRETE
    `RecordingAgentRunner` (controller holds it), consumed in `admitted`;
    NO spec/port widening (a `RoleInvocationSpec` field would break the
    settled p2/p3 struct literals — dead end, recorded).
  - F6 rule (smallest spec-consistent): (a) the §11.5 row-2 transient
    retry CANCELS the prior invocation through the port before the
    re-dispatch (fate unknown); (b) when the run terminalizes with
    invocations still in flight, each is cancelled at drain exit
    (mirrors bound_the_drain). Both pin durable evidence records naming
    the cancellation; observed-terminal invocations (Failed/TimedOut
    reported by the runner) are NOT re-cancelled (the runner already
    retired them). e2e_recovery leg 3 gains: the pinned cancellation
    records + a bounded `kill -0` liveness poll on the durable handle
    files' child pids (R16-F2/P8 proved cancel retires the hostile
    child; the poll is the teeth).
  - F8: AK-954 (local/critic/instrument/static_check, locator
    `evidence/real-local-report.json`), child.sh locator map + answer-key
    + reference.yaml prose + lock re-freeze (digests + unit_count 4 +
    annex spelling). e2e_recovery leg 1 exactly-once updates to TWO
    settlement events (AK-951 + AK-954), one row each. e2e_budget and
    recovery legs 2/3 unaffected (verified against their assertions).

## Status

- [x] Preflight
- [x] F2 — DONE. Mutation gap shown (2/2 green with cross_cutting spawned
  verbatim), new unit-binding assertion RED (captured verbatim:
  `confinement=uncontained pid1=systemd tmp-write=ok` on the AK-952
  artifact), compose.rs restored byte-identically
  (`63efd01341fce31b6c33dfeaac00a591bb4aab14d05d09df1d143c918ebc301b`
  before == after), then 2/2 green on the real tree.
- [x] F3 — DONE. Strengthened durable-record assertion RED (captured
  verbatim: AK-952 record `"tier":"contained"` with
  `"sandbox_policy_ref":"tier-cross-cutting-real-v1"`), then fix landed:
  typed `record_dispatch_tier` channel on the concrete RecordingAgentRunner,
  consumed in `admitted` (no-fact + no-policy ⇒ Contained, the only
  §5.3.5-legal shape; no-fact + policy ⇒ fail-closed AdmissionRefused),
  controller registers `admission.tier()` at dispatch. Green on the real
  tree. Regressions: mock E2E 12/12, p10+p9 8/8, settled p2 13/13.
- [x] F6 — DONE. RED (captured): two controller-level tests
  (`the_transient_retry_cancels_the_prior_invocation_through_the_port`,
  `a_terminal_reached_with_work_in_flight_cancels_the_stranded_invocation`)
  both `left: []`. NOTE: the terminal test initially passed for the WRONG
  reason — the fixture policy's 30ms quiet window stall-cancels a
  stranded flight on drain pass 1; reworked onto `fixture_composed_quiet`
  (60s window) to isolate the terminal cleanup. Fix:
  `dispose_transient` cancels the prior through the port before the
  row-2 re-dispatch (pinned
  `prior_invocation_retired_before_retry{how:cancelled|already_terminal}`);
  drain exit cancels in-flight work at any terminal (pinned
  `terminal_stranded_invocations_retired`); new `cancel_operation_bound`
  (stall-window scale) bounds every controller cancel — the old
  `operation_bound` (run wall horizon, 600s) would let a SIGTERM-immune
  child block a cancel ~600s. Leg 3 upgraded: durable retirement records
  asserted, `"how":"cancelled"` asserted, bounded /proc liveness poll
  over all handle-file child pids; leg env tightens
  GAUNTLET_STALL_TIMEOUT_MS=2000 (graceful leg 2s). Green: gauntlet-app
  565/565, p7+p6 7/7, p4 4/4, mock+p5 16/16, e2e_recovery 3/3.
- [x] F8 — DONE. AK-954 (local/critic/instrument/static_check,
  `evidence/real-local-report.json`) added to the fixture answer key +
  child.sh locator map + reference.yaml prose; lock re-frozen
  (answer_key sha256:5295e021..., manifest sha256:d36fc22b..., unit_count
  4, annex AK-951..AK-954; note records the re-freeze; the repo ruler
  was never touched). Happy leg asserts: AK-954 proven in the report,
  per-tier status lines (contained=1/local=1/cross_cutting=0 settled),
  AK-954 dispatch record `"tier":"local"`, AK-951+AK-954 settlement
  records, AK-954 pinned artifact confined. Recovery leg 1:
  exactly-once now 2 settlement events / 1 row per auto-closable unit.
  AK-047 leg: exactly TWO intact-policy dispatches (AK-951+AK-954),
  AK-952 never. Budget leg: never-dispatched scan extended with AK-954.
  Green: real-runner 2/2, recovery 3/3, budget 1/1.
- [x] F9 — DONE. RED (captured): happy queue held only AK-952 tickets —
  `AK-953's escalation ticket stands in this run's queue` panic. Fix:
  first-refusal ticket when a plan skips an owed unit for a scheduler
  GOVERNANCE reason (`scheduler_governance_blocked` ledger, one ticket
  per unit, classify's owed filter, terminal decision untouched; ticket
  text "dispatch not scheduled: Unit AK-953 is irreversible: never an
  automatic dispatch candidate — ..."). Green on the happy leg.
- [x] F1 — DONE (rename in the F8 edit round):
  `the_composed_loop_settles_the_auto_closable_units_wraps_the_cross_cutting_dispatch_and_escalates_the_rest`.
- [x] F4/F5/F10 wording — DONE (compose_teeth.rs comment + e2e-real.md
  mirrors; F10 count; F5 artifact-durability boundary wording; F2
  binding note at the AK-066 bullet; COR-1 addendum appended to
  e2e-real.md with supersession notes).
- [x] F7 record-only — DONE (COR-1 addendum in e2e-real.md +
  cor1-report.md).
- [x] Full battery — ALL GREEN (final sweep; see cor1-report.md ledger):
  ruler before/mid/after PASS; check-package cli+app EXIT 0;
  real-runner 2/2; recovery 3/3; budget 1/1; mock 12/12; p5 4/4;
  p7+p6 7/7; p4+p10+p9 12/12; lint EXIT 0; boundaries 237/237.
  Two literal-scanner violations introduced mid-round were fixed in
  place (QUIET_WINDOW_EXTENDED rename; `alive` fixture mode replacing
  /proc + sh) — boundaries green after.
- [x] cor1-report.md written (mission echo, per-finding evidence,
  deliverable sha256 table, full command ledger, deviations, hygiene).
- [x] Probe hygiene: repo .gauntlet/policies/ untouched (ls proof in
  report), never-touch paths clean in git status, 0 scratch worlds in
  /tmp, nothing staged or committed.

## Final state

COR-1 COMPLETE. The completion contract is cor1-report.md. No open
work; no resume state needed.
