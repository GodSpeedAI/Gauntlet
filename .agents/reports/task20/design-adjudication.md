# Task 20 — design-as-built adjudication (orchestrator)

Critic verdict: NOT_CONFIRM (0 blocking / 4 major / 6 minor / 5 informational),
report: critic-design.md. Adjudication of every residual:

## Routed to correction (COR-D1..D10)

- **RES-01 → COR-D1** `roles list` added (read-only, over registry/lock
  reads). §20.1 spells it; D8's "roles (exists)" shorthand does not override
  the spec text.
- **RES-02 → COR-D2** parallel-completion collision fixed LOOP-LOCALLY: at
  most one observation binding per (run, iteration); surplus completions of
  the same drain are carried to the following iteration and bound there
  (never a raw store error escaping drain; never a second bundle for one
  key). The §12/§14-vs-§11.2 binding-granularity question (re-keying per
  invocation) is a RULER-OWNER decision — recorded in
  reference-candidates.md as a candidate-revision question, NOT implemented.
  Fixture GAUNTLET.md must state why `max_parallel` is what it is; a test
  proves `max_parallel: 2` completes multi-unit without collision.
- **RES-03 → COR-D3** R-1b behavioral half CLOSED: builder execution
  anchors at the admitted per-unit workspace (`spec.workspace`) — mock
  effects root, CLI child cwd + `{workspace}` placeholder. E2E artifacts
  must land under the per-unit workspace segment. Teeth: execution-in-
  admitted-workspace test, red when bypassed. This is the actual §17.5.1
  invariant-1 closure; the honest-record alternative is refused.
- **RES-04 → COR-D4** teeth battery: D2 fail-closed selection (unset token,
  unknown token), mock-without-scenario refusal, D3 live-wrap refusal
  (RES-05). Tests only.
- **RES-05 → COR-D4** (folded, above).
- **RES-06 → COR-D5** composed escalations resolve/retry CLI test: refusal
  without PASS evidence (ticket stays open), success with earned PASS (one
  settlement row — D9 visible), settle-committed-but-queue-write-failed
  branch pinned.
- **RES-07 → COR-D6** dry-run zero-write tooth hashes file CONTENTS, not
  just paths.
- **RES-08 → COR-D7** usage lines fixed (status `--run` required; inspect
  full flag set); golden regenerated DELIBERATELY with the change recorded
  in cor report.
- **RES-09 → COR-D8** doctor check-3 `(true, None)` reason reworded
  token-neutral ("no measurement fronted for this runner token"); no mock
  probe mechanism (recorded choice: the mock is a test hook).
- **RES-10 → COR-D9** diagnostic-allowance mapping moved into gauntlet_app;
  CLI passes the config scalar through.
- **RES-14 → COR-D10** E2E assertions: vacuous `contains("configured-but-
  untested")` removed (real load stays on the per-unit negative + settled=0
  + export zero-settlement); brittle exact-prose assertion replaced with
  parsed unit→category pairs.
- Doc nits (RES-15 + the cli_surface.rs:17 golden-path comment): fixed with
  the COR-D7 golden regeneration.

## Recorded, no code change

- **RES-11** ProducePolicy seam: legitimate compatibility seam (critic's
  judgment accepted). RECORD: `evidence_required.locator` CONTENT semantics
  are composition-bound (facet-document world vs produced-artifact world);
  ruler owner to adjudicate whether §14.3 should pin one world. Recorded in
  the task closure + reference-candidates.
- **RES-12** mock token not a live gaming surface (operator-config
  selected, fail-closed scenario hook); E2E verification depth bounded by
  mock expressiveness; G3/G12 fabrication-class coverage belongs to Task
  24. Recorded.
- **RES-13** arch-suite deviations correctly decided and recorded. No
  action.
- **RES-02 (record half)** the fixture's parallelism note + the binding-
  granularity ruler-owner question (see COR-D2).

Constraint for the correction builder: every proof p1–p12, g6/g7/g10,
contract suites, and the existing E2E legs stay green UNMODIFIED unless a
correction strictly requires a mechanical adaptation (constructor/usage
text); any assertion change is listed in the correction report with its
reason. `just ruler` before/after. Battery after each major item, full
battery at the end.
