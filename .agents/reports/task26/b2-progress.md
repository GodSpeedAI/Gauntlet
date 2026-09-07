# B2 progress — task 26 (app `baseline` module) — COMPLETE

Checkpoint file, kept for the orchestrator's reconciliation. Completion
contract: `b2-report.md` (existence + slice battery green is the claim;
NO settlement authority).

## Design decisions taken (documented per charter "your call" items)

1. **Workspace materialization** — seam method:
   `BaselineExecutor::materialize_workspace(task, scenario, parent)`.
   The harness calls it once per leg BEFORE any dispatch; the CLI owns
   copying (the task-20/25 scratch-world pattern). Chosen over handing
   per-scenario paths so freshness is structurally one-call-per-leg.
2. **measured_wall_ms** — the HARNESS times each leg with the injected
   `Clock` (one read before materialization, one after the held-out run),
   not the seam. Resolution is the Clock's (production clock = 1 s;
   disclosed in `_meta`). Keeps the app layer free of std::time and tests
   deterministic.
3. **Critic envelope body** = `task.requirement` EXACTLY (verbatim; the
   requirement text itself names the artifact and carries the marker
   tokens — no separate locator concatenation). Structural blindness:
   `DispatchRequest` has NO field that can carry a result document;
   `critic_request()` is a named pub function the tests pin verbatim
   (tooth T2's app half).
4. **Role tokens** = `builder` / `reviewer` / `critic` — matching B1's
   child protocol (it branches on ROLE). Charter's "second dispatch in
   the builder role" for B is realized as the `reviewer` branch (the
   same shared-criterion posture; the child protocol is binding for the
   wire token).
5. **regressions** — `Option<u64>` on `DispatchOutcome` (A/B/C: the
   executor's optional base-state comparison; `None` ⇒ recorded as 0
   with basis `not-measured`) and on `LoopRunOutcome` (D: executor's
   optional durable-facts derivation; same honest fallback). Basis
   travels beside every count (`regressions_basis`) and in `_meta`.
6. **settlement reliability** = per-10k fixed point (the repo's
   TargetMetric precedent, no floats in evidence): A/B/C = `Some(0)` BY
   CONSTRUCTION (no durable substrate — stated in the report); D =
   deterministic-instrument PASS-backed settled units / total settled
   units, `None` when nothing settled (missing evidence is never a
   figure).
7. **D facts are store-derived in the app** — `run_full_loop` returns
   the run handle + `Arc<dyn StateStore>` (the evaluate
   `ExecutionOutcome` shape); `derive_loop_metrics` reads the DURABLE
   record (resume/settlements/verification_records/
   run_lifecycle_facts). The executor's claimed terminal is
   cross-checked against the durable last `RunTerminal`; disagreement =
   typed INCONCLUSIVE evidence, never a figure.
8. **D verification_burden** from durable counts:
   `instrument_runs` = verification-record count;
   `review_dispatches` = recorded invocations beyond the
   unit-attributed ones (lifecycle facts), saturating.
9. **bounded/terminal facts** — A/B/C: no loop machinery ⇒ terminal
   classification null, `bounded_termination` true (structurally finite
   posture), `reserve_intact` null. D: `classify_terminal` over the
   DURABLE terminal (Settled → settled; Blocked/Escalated/Aborted/
   Inconclusive → refused-terminally; BudgetExhausted →
   bounded-not-converged), `reserve_intact` from the seam's
   executor-computed flag. Disclosed in `_meta`.
10. **JSON key names** follow the charter schema exactly; two unit-
    honesty notes disclosed in `_meta`: `payment.cost_usd` carries the
    durable MicroUsd integer (micro-dollars), and `held_out` is the
    `"pass"`/`"fail"` token.

## Status — ALL LANDED

- [x] Recon (evaluate/metrics precedent, ports JSON, StateStore reads,
      domain vocab, workspace lints/justfile).
- [x] Design decisions recorded (above).
- [x] `baseline/{mod,profiles,task,metrics,harness,report}.rs` +
      one-line `lib.rs` registration (`pub mod baseline;`).
- [x] Unit teeth: T1 predicates (false-completion, defect-escape,
      settlement-reliability, terminal classification, posture burden),
      T4 parse fail-closed (profiles + scenarios), task codec refusals
      (schema/unknown/duplicate/missing fields, absolute held-out,
      scenario vocabulary, empty/duplicate scenario list, comments/
      fences/tabs/stray lines), D-loop durable derivation over a fake
      StateStore (full derivation, model-instrument exclusion, terminal
      disagreement, not-durable, missing terminal).
- [x] Integration test `tests/baseline_harness.rs`: fake executor over
      all 4 profiles x 5 scenarios — dispatch shapes, fresh workspaces,
      T2 blind envelope at composed level, the charter's derived matrix,
      D-pays-most + reliability figures, report JSON key set
      {A,B,C,D,_meta} + per-leg metric fields + disclosures.
- [x] `just check-package gauntlet-app` — exit 0, no warnings (flock'd,
      own top-level command, `sccache --stop-server` inside the same
      flock).
- [x] `just test-package gauntlet-app` — exit 0, 618/618 passed.

## RED events observed during development (the suite bites)

- `measured_wall_is_harness_measured` FAILED first (assertion expected
  >= 2 ticks; an A leg reads the clock exactly twice) — fixed by pinning
  the exact figure (1_000 ms), i.e. the assertion was TIGHTENED, not
  weakened.
- `d_pays_the_most_and_carries_the_reliability_figures` FAILED first —
  the fake store scripted D with one unit dispatch (equal to A). Fixed
  by scripting the loop's correction round (2 dispatches) in the FIXTURE,
  not by touching the assertion.

Predicate-flip mutation re-verification of the named teeth is the
orchestrator's closure step per charter §Battery.

## Scope guard — verified clean

`git status` shows exactly: `crates/gauntlet-app/src/lib.rs` (one-line
`pub mod baseline;`), `crates/gauntlet-app/src/baseline/` (new),
`crates/gauntlet-app/tests/baseline_harness.rs` (new),
`.agents/reports/task26/` (report files). The `.jolli/*` and
`.agents/CURRENT_STATUS.yml` entries predate/are outside this slice
(orchestrator/tooling state; not touched by B2). No git add/commit, no
git restore. `tests/fixtures/baseline/` is B1's in-flight untracked
work — not touched.
