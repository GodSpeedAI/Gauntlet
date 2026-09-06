# Task 25 — COR-1 brief (corrections round 1)

You are the corrections builder for task 25 in
/home/sprime01/projects/gauntlet. The blind design-as-built critic
returned CONFIRM with 0 BLOCKING / 3 MAJOR / 7 minor / 3 informational
(`.agents/reports/task25/critic-design.md` — read it FIRST; it is your
findings ledger). The orchestrator has adjudicated the dispositions
below. Every production fix is RED-FIRST: demonstrate the defect with a
failing test (or the named mutation, byte-identically restored), then
fix, then green.

## Dispositions (orchestrator-owned, binding)

FIX in this round: F1, F2, F3, F6, F8, F9, F10, plus the F4 and F5
wording halves. RECORD, no change: F7 (the budget terminal divergence
is already disclosed in-test and in e2e-real.md; the exhaustion
terminal itself stays pinned by the settled p6 at controller level).

- **F2 (MAJOR — the wrap evidence must bind to the unit).** In
  `crates/gauntlet-cli/tests/e2e_full_loop_real_runner.rs` (~360-367):
  the confinement scan is unit-agnostic — a tier-conditional seam
  mutation (wrap `contained`, spawn `cross_cutting` verbatim) keeps
  both legs green. Fix: assert that the stored artifact carrying
  `"unit":"AK-952"` ALSO carries `confinement=confined` and
  `tmp-write=denied`. RED-FIRST: mutate `LiveContainment::wrap` in
  `crates/gauntlet-cli/src/compose.rs` to return the command verbatim
  for the cross_cutting tier only, show the CURRENT assertions stay
  green (the gap) and your NEW assertion goes RED, restore compose.rs
  byte-identically (sha256 before/after in your report), then green on
  the real tree. Also fix the report sentence at e2e-real.md:117-127
  if it implies more than the fixed test proves.
- **F3 (MAJOR — durable tier misstated).**
  `RecordingAgentRunner::admitted`
  (`crates/gauntlet-app/src/compare/recording.rs`) hardcodes
  `tier: BlastRadius::Contained` in the durable role_invocation
  record; the composed E2E now dispatches cross_cutting, so the
  record lies. Fix: the smallest typed tier channel from the dispatch
  facts (the controller knows the unit's tier at admission time —
  find the minimal honest path; do NOT widen the runner port beyond
  what the record needs; keep every existing RecordingAgentRunner
  user compiling and green). RED-first: strengthen the happy leg's
  durable-record assertion to demand the AK-952 invocation record's
  tier == cross_cutting — it must FAIL before the fix and PASS after.
  Run `p10_no_false_learning`, `p9_self_improvement_containment`, and
  the mock E2E as regression (RecordingAgentRunner users).
- **F6 (MAJOR — in-flight invocations are never cancelled).** The
  composed loop's transient disposition (§11.5 row 2) retries the
  typed error and terminal dispositions end the run, but the port's
  `cancel` is never called on this path — the hostile child outlives
  the run AND the resume (the R16-F2 discharge exposed it; leg 3 of
  e2e_recovery currently reaps survivors as test hygiene). Fix in
  `crates/gauntlet-app/src/control/controller.rs`: in-flight runner
  invocations are cancelled through the AgentRunner port's own
  `cancel` (idempotent, per its contract suite) when the run leaves
  them behind — at minimum at every terminal disposition; if a retry
  re-dispatches while a prior invocation may still be alive, cancel
  the prior first (read the §11.5 rows and the port contract; choose
  the smallest spec-consistent rule and RECORD it). RED-first: a
  controller-level test with a recording/spy runner asserting cancel
  fires on the transient→retry path and/or at the terminal (your
  choice, justified), FAILING before the fix; plus upgrade e2e_recovery
  leg 3: the survivors-reaping stays as hygiene, but add a durable
  assertion that the invocation was cancelled/retired (and, if
  mechanically honest on this host, a bounded process-liveness check
  that no child outlives the run — no sleep-and-hope, bounded poll).
  Regression: p4/p6/p7 proofs and the mock E2E MUST stay green — they
  are the controller's own suite; any red there is a defect in your
  change.
- **F8 (minor — the `local` tier is missing from the fixture).** The
  plan's step 1 says "at least one Unit per tier up to `cross_cutting`"
  — that includes `local`, and the charter's omission of it was the
  orchestrator's drafting error, not a license. Fix: add a `local`
  unit (e.g. AK-954, authority `critic`, `producer: instrument`) to
  `tests/fixtures/target-real/.wayfinder/answer-key.md` + manifest
  wiring, RE-FREEZE the fixture lock with
  `scripts/reference-digest.sh` (update
  `.gauntlet/reference.lock`'s digests + `unit_count` — the fixture's
  own lock, never the repo ruler), and make the happy leg assert the
  local unit dispatches, settles, and pins evidence like AK-951. Keep
  the fixture's docs coherent (GAUNTLET.md fixture prose if it
  enumerates units).
- **F1 (minor)**: rename the happy-leg test — AK-952 does not settle;
  the name must not claim it (critic's suggestion is fine).
- **F9 (minor)**: extend the happy leg's escalation assertions to
  cover AK-953's ticket (queue presence + never-dispatched) in the
  SAME run as the never-dispatched scan, closing the Done-when
  one-run reading.
- **F4 (minor, wording)**: compose_teeth.rs:355-366 re-pin comment —
  "stronger" → honest phrasing ("differently pinned: coverage moved to
  the routed dispatch; the at-locus strength lives in the E2E"). Mirror
  in e2e-real.md:184-188.
- **F5 (minor, wording)**: e2e-real.md:141-143 — "at the OBSERVED
  produce window" → the artifact-durability boundary (the in-window
  branch is explicitly optional per the test's own disclosure).
- **F10 (minor)**: e2e-real.md:105 — mock regression count "14/14" →
  "12/12" (the real count).

## Constraints (all binding; unchanged from the charter)

- Never touch: `.wayfinder/**` at the REPO root, the repo
  `.gauntlet/**`, repo-root `GAUNTLET.md`, any settled `p*.rs`/`g*.rs`/
  `antigaming.rs`/`e2e_full_loop_mock.rs` test file (you RUN them as
  regression, never edit), `.agents/CURRENT_STATUS.yml`. The FIXTURE's
  own `.wayfinder/`/`.gauntlet/` under `tests/fixtures/target-real/`
  IS editable (it is this task's deliverable).
- No new dependencies; no network; no frontier-model calls; no
  sleep-and-hope (bounded polls, typed failure).
- RED-first evidence for F2, F3, F6 goes in your report (the failing
  output, the mutation sha256-restore proof for F2).
- Checkpoint as you go: `cor1-progress.md` (what landed, what's
  mid-flight, next command). Report contract: `cor1-report.md` —
  mission echo, per-finding disposition + evidence, deliverable paths
  + sha256, every command + outcome, deviations. Your final message is
  a signal only.
- NOTHING staged or committed (orchestrator owns commits).

## Battery (run before reporting completion; all green)

```bash
just ruler
just check-package gauntlet-cli
just check-package gauntlet-app
cargo nextest run --test e2e_full_loop_real_runner
cargo nextest run --test e2e_recovery
cargo nextest run --test e2e_budget
cargo nextest run --test e2e_full_loop_mock
cargo nextest run --test p5_hard_gate
cargo nextest run --test p7_recovery --test p6_budget_control
cargo nextest run --test p4_corrective_loop --test p10_no_false_learning --test p9_self_improvement_containment
just lint
just boundaries
```

## Operational addenda (binding — identical to the charter's)

- ONE compile at a time: EVERY cargo/just invocation is
  `flock .agents/reports/task25/compile.lock -c '<command>'`, flock
  always its OWN top-level command, never nested; `sccache
  --stop-server` inside the same flock'd command after the cargo
  command. Default test threading only (WSL2, ~1.6 GiB free).
- `just ruler` before and after every phase; STOP and report if it
  ever fails.
- Probe hygiene: scratch/fixture worlds only; nothing planted under
  the repo's `.gauntlet/policies/`; `ls` proof in your report.
