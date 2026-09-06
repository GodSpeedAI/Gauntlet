# IN-FLIGHT — task 25 (E2E real runner + recovery + budget)

Dispatched: 2026-09-05, orchestrator session (task25 build phase).
Charter: `.agents/reports/task25/charter.md` (binding).

## Slices

- **B1** (general-purpose builder, dispatched 2026-09-05): the whole
  task25 slice — fixture `tests/fixtures/target-real/`, the
  live-containment production correction (retire
  `LiveContainmentDeferred`), targets `e2e_full_loop_real_runner`,
  `e2e_recovery`, `e2e_budget`, report `.agents/reports/e2e-real.md`.
  Progress: `.agents/reports/task25/b1-progress.md` (checkpoint
  contract). Completion: `b1-report.md` (the result contract — its
  existence with the slice battery green is B1's completion claim; it
  carries NO settlement authority).

## State

- B1: COMPLETE (build landed 2026-09-05; b1-report.md is its contract).
- Critic: COMPLETE — blind design-as-built round 1 CONFIRM, 0 BLOCKING
  / 3 MAJOR (F2 unit-binding of wrap evidence, F3 recording-runner
  tier hardcode, F6 missing invocation cancel) / 7 minor / 3
  informational. Report: `critic-design.md`.
- COR-1: COMPLETE — all dispositions landed RED-first (F2 mutation-
  proven + byte-identical restore; F3 typed registration channel,
  port not widened; F6 cancel on retry + terminal drain with bounded
  cancel span; F8 AK-954 local unit + fixture lock re-frozen; F9
  scheduler-governance ticket; F1/F4/F5/F10 fixed; F7 recorded).
  Battery green (ruler x3, real 2/2, recovery 3/3, budget 1/1, mock
  12/12, p5 4/4, p7+p6 7/7, p4+p10+p9 12/12, lint 0, boundaries
  237/237). Contract: `cor1-report.md`.
- Falsifier: COMPLETE — blind EXECUTING falsifier round 1 NOT_CONFIRM,
  0 BLOCKING / 2 MAJOR (F-R1 no-seam arm unbound by executed evidence,
  F-R4 report classification basis false) / 2 minor (F-R3
  post-spawn-refusal cancel leak, F-R5 escalated-resume duplicate
  ticket + silent terminal rewrite) / 3 informational. 12 executed
  attacks; all 4 mutants restored byte-identically; the COR-1 fixes
  KILLED at their loci; exactly-once held at a new boundary; report
  store-claims confirmed true. Report: `falsifier-round1.md`.
- COR-2: COMPLETE — all four dispositions landed RED-first (F-R1
  adapter teeth test, mutant-proven; F-R4 record-derived report
  classification over NEW append-only lifecycle events +
  run_lifecycle_facts read + terminal sealing at three loci; F-R3
  group-wide transport stop; F-R5 queue-level dedup + pinned
  re-entry; informational-a recorded). Battery green (14 targets
  incl. escalation/operator regression, boundaries 237/237, lint
  clean, ruler PASS). Incident disclosed + orchestrator-verified: a
  git restore briefly clobbered uncommitted work — caught by sha256
  check, reconstructed byte-exactly (compose.rs 63efd013…, runner.rs
  b20960b5… re-pinned by the orchestrator; settled surfaces zero
  diff). Contract: `cor2-report.md`.
- Falsifier r2: COMPLETE — CONFIRM (the r1/2 corrections held under
  execution) with 3 MAJOR / 3 minor, all executed: F2-1 dedup per-run
  conjunct unwatched, F2-4 KILL-leg group addressing unwatched, F2-7
  operator resolve/retry dead-end for governance-routed tickets;
  F2-2 false-dedup collision via unit-less shipped summary, F2-3
  concurrent-escalate race, F2-5/F2-6 observations (reaper-latency
  idiom; compile.lock fd inheritance wedge). All mutants restored
  byte-identically; new event machinery held (export determinism,
  kill windows, fail-closed reads); A5 disposition-path sweeps CLEAN;
  A6 cancel-during-retry held. Report: `falsifier-round2.md`.
- COR-3: DISPATCHED 2026-09-06 (brief: `cor3-brief.md`; dispositions:
  FIX F2-1 + F2-4 teeth tests, F2-2 unit-bearing summaries, F2-3
  fix-if-mechanical; PIN F2-7's honest refusal posture; RECORD
  F2-5/F2-6; F2-7 ruler-owner residual). Contracts:
  `cor3-progress.md`, `cor3-report.md`.
- After COR-3: orchestrator PERSONALLY executed the F2-1/F2-4 mutant
  recipes (both RED verbatim, both restored sha256-verified) and the
  AK-044 Done-when ladder (3 of 5 defensive layers disabled
  successively — the never-dispatched invariant held through each;
  recorded as over-determination by design, each layer carrying its
  own executed unit pin; all restorations byte-verified; E2E green
  post-restore).
- CLOSURE BATTERY: `just check` green; `just test` run 1 FAILED
  (b1_contract_wins_over_exit7 — agent-cli; passed in isolation and in
  run 2: recorded load-flaky, task22 protocol); `just test` run 2
  FAILED a DIFFERENT test — e2e_recovery's produce-window leg: TWO
  durable production rows for real-marker-report.json (different
  digests, offsets 4+11) after the kill landed POST-production — the
  resume re-dispatched and re-produced instead of adopting the orphan
  production. REAL DEFECT (the leg's "assertions hold either way" is
  executed-false; the critic's F5 disposition is overturned). COR-4
  dispatched.
- COR-4: STOP disposition invoked and UPHeld by the orchestrator — the
  diagnosis verifies (p7_recovery.rs:1454's one-fresh-dispatch retry
  pin; verify/authority.rs:1149/:1182's settled prior-iteration
  staleness machinery): re-production on resume is structurally forced;
  the leg's per-locator exactly-once claim was stronger than the spec.
  Landed: deterministic leg 2b (RED verbatim matches the closure
  failure), leg 2's false comment corrected, zero production changes,
  battery green except the two legs asserting the adjudicated claim.
  ORCHESTRATOR ADJUDICATION: disposition (a) — re-scope both legs to
  the spec-honest semantics (one fresh dispatch; prior production
  intact as a write-once fact; per-iteration production; exactly-once
  SETTLEMENT over the current ref); the §18.3 adoption question is a
  CandidateRevision for the ruler owner. COR-4 resumed with the
  re-scope contract (tests-only).

## Orchestrator adjudications pending (owned at correction/closure)

- AK-066 settleability tension (cross_cutting forced to lead_agent by
  §5.3.2; the unattended controller closes only as Automatic;
  §16.1 condition 3's lead_agent route is Orchestrator-rationale,
  never Automatic — verified by the orchestrator in settle/mod.rs
  1398-1410 + controller.rs 3295): the routed-to-operator behavior is
  the honest pin; the wording tension goes to reference-candidates at
  settlement (ruler owner), never patched here.
- The runner-invocation liveness at transient dispositions and the
  recording-runner tier field are with the critic (neutral questions
  7 and the durable-record sweep); corrections follow its round.

## Recovery note

On session interruption: reconcile `git status` + this file +
`b1-progress.md` against the tree BEFORE treating any deliverable as
unstarted (tree outranks record). Compile lock:
`.agents/reports/task25/compile.lock` — flock top-level only, never
nested; `sccache --stop-server` inside the same flock'd command.
