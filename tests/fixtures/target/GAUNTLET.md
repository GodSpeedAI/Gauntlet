---
reference:
  manifest: .wayfinder/reference.yaml
  answer_key: .wayfinder/answer-key.md
  comparison_contract: .gauntlet/comparison-contract.yaml
  digest_algo: sha256
roles:
  dir: .agents/roles
runners:
  default: mock-agent
observation:
  facets: [filesystem, test]
review:
  mode: single
  max_critics: 0
verification:
  prefer_deterministic: true
budget:
  max_rounds: 4
  max_parallel: 1
  max_depth: 1
  max_wall_seconds: 600
  max_tokens: 200000
  max_cost_usd: 5
  recovery_reserve_pct: 15
  diagnostic_budget: 1
state:
  backend: postgres
operator:
  surfaces: [cli]
---

# GAUNTLET.md — E2E fixture target project

This tiny project is the seeded target the `e2e_full_loop_mock` proof runs the
real `gauntlet` binary against (plan Task 20, builder B3). It is a fixture, not
a product: the E2E copies this directory to a per-test tempdir and runs THERE —
nothing may ever write into the fixture itself at test time.

## Runners

`runners.default` is `mock-agent` (charter D2): the composed run replays the
scripted scenario named by the `GAUNTLET_MOCK_SCENARIO` test hook. The hook is
deliberately NOT operator configuration and stays out of this file; the E2E
test sets it per leg (`e2e-happy`, `e2e-lying`, `e2e-forged-report`). The
repository root's own GAUNTLET.md keeps `runners.default` unset (UNK-003 stays
open).

## Observation (the test facet, COR-F1)

The observation plan fronts TWO instruments: the `filesystem` facet (records
the per-unit workspace tree) and the `test` facet — a REAL instrument
execution of the project's check command. The `test` facet requires a tool
binding (`GAUNTLET_TOOL_TEST`); the E2E binds it to THIS project's own check
command, `scripts/check.sh`, so every completion observes the check actually
executing over the workspace where the dispatched work landed. This is the
COR-F1 correction's fixture half: the settlement gate demands an executed,
passing test facet before an instrument-produced unit settles, so a report-
forging liar (planted passing reports, source that fails the real check) can
no longer settle — the instrument's own run contradicts the forged bytes.

## Sandbox

The sandbox backend stays at the shipped conservative default (`none`): only
`contained`-tier dispatches are admissible at that posture, so every unit in
the fixture reference is tier `contained` with authority `critic`.

## Parallelism (`budget.max_parallel: 1`, recorded)

`max_parallel` is pinned to 1 because the loop's observation binding is
write-once per (run, iteration) — §12/§14 key the bundle to the iteration —
so this fixture sequences the units strictly: one dispatch, one completion,
one observation binding, one settlement per iteration. (The controller
correction COR-D2 makes surplus parallel completions CARRY to the following
iteration instead of erroring, so a higher value now completes honestly; the
`max_parallel: 2` behavior is proven by a dedicated multi-unit test, and this
fixture keeps 1 so the E2E legs keep their one-unit-per-iteration shape. The
binding-granularity question itself — re-key the binding per invocation — is
a ruler-owner decision, recorded in
`.agents/reports/reference-candidates.md`, not a fixture detail.)

## The target

`src/gadget.rs` is the project's one source file and `scripts/check.sh` is its
real check command: it exits 0 exactly when the gadget source carries the
stable marker the fixture reference demands. The fixture units' evidence
requirements name locators a real instrument can produce in principle; whether
the shipped composed loop actually produces such evidence is what the E2E
observes and reports.
