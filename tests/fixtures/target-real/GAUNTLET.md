---
reference:
  manifest: .wayfinder/reference.yaml
  answer_key: .wayfinder/answer-key.md
  comparison_contract: .gauntlet/comparison-contract.yaml
  digest_algo: sha256
roles:
  dir: .agents/roles
runners:
  default: cli-agent
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
  backend: sqlite
sandbox:
  backend: bwrap
operator:
  surfaces: [cli]
---

# GAUNTLET.md — E2E real-runner fixture target project

This tiny project is the seeded target the task-25 E2E proofs
(`e2e_full_loop_real_runner`, `e2e_recovery`, `e2e_budget`) run the real
`gauntlet` binary against. It is a fixture, not a product: the E2E copies
this directory to a per-test scratch world and runs THERE — nothing may
ever write into the fixture itself at test time.

## Runners (UNK-003 resolution, operator-delegated, recorded)

`runners.default` is `cli-agent`: the task-25 charter's recorded operator
resolution for THIS fixture (the same class as the task-20 fixture's
`mock-agent` selection). The repository root's own GAUNTLET.md keeps
`runners.default` unset; UNK-003 itself remains open with terminal
disposition at Task 27. The runner's command template is NOT operator
configuration and stays out of this file: the E2E sets
`GAUNTLET_CLI_AGENT_CMD` per leg, pointing at this project's own
deterministic child (`agent/child.sh`). A deterministic child under the
real adapter is NOT a frontier-model claim and must never be worded as
one: the adapter, transport, contract delivery, poll classification,
deadline and cancel mechanics are the real production paths; the child's
"intelligence" is a fixture script.

## Sandbox

`sandbox.backend` is `bwrap` (the host's real process-isolation backend;
the conservative `none` posture refuses cross_cutting dispatch outright).
The tier policies live in this project's own `.gauntlet/policies/` —
`contained`, `local`, `cross_cutting`, all naming the same backend;
there is NO `irreversible` policy file (by design, AK-049: the
irreversible unit escalates, it never dispatches).

## Observation (the test facet)

The observation plan fronts TWO instruments: the `filesystem` facet and
the `test` facet, which requires the `GAUNTLET_TOOL_TEST` tool binding —
the E2E binds it to THIS project's own check command, `scripts/check.sh`,
so every completion observes the check actually executing over the
workspace where the dispatched work landed. A settlement over an
instrument-produced unit demands an executed, passing test facet.

## Parallelism (`budget.max_parallel: 1`, recorded)

Same recorded reason as the task-20 fixture: the loop's observation
binding is write-once per (run, iteration), so this fixture sequences
the units strictly — one dispatch, one completion, one observation
binding, one settlement per iteration.

## The target

`src/gadget.rs` is the project's one source file and `scripts/check.sh`
is its real check command: it exits 0 exactly when the gadget source
carries the stable marker the fixture reference demands. `agent/child.sh`
is the deterministic child agent the command template drives: it reads
the prompt file, performs the bounded artifact writes inside the admitted
workspace, records what it observes of its own containment, and writes a
strict `role_result@v1` document at the result path. The fixture units'
evidence requirements name locators a real instrument can produce; the
child is mechanical protocol, nothing more.
