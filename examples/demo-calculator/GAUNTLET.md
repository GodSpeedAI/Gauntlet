---
reference:
  manifest: .wayfinder/reference.yaml
  answer_key: .wayfinder/answer-key.md
  comparison_contract: .gauntlet/comparison-contract.yaml
  digest_algo: sha256
roles:
  dir: .agents/roles
runners:
  default: prime-agent
observation:
  facets: [filesystem, test]
review:
  mode: single
  max_critics: 0
verification:
  prefer_deterministic: true
budget:
  max_rounds: 8
  max_parallel: 1
  max_depth: 1
  max_wall_seconds: 2700
  max_tokens: 600000
  max_cost_usd: 6
  recovery_reserve_pct: 15
  diagnostic_budget: 1
state:
  backend: sqlite
operator:
  surfaces: [cli]
---

# GAUNTLET.md — demo-calculator

A tiny, always-available Gauntlet demo project. Its whole job is to give an
operator something they can start a REAL run against in under a minute,
using the real `prime-agent` runner — not the mock replay runner — so the
demo actually dispatches an agent and costs real tokens.

## Runners

`runners.default` is `prime-agent` (a real runner, not `mock-agent`): the
composed run really dispatches to the locally installed `prime-agent`
binary. This is deliberate — the point of this demo is to watch a real
agent attempt real work, not replay a canned scenario. Budgets above are
sized for three units dispatched one at a time with room for one retry
(`max_rounds: 8` → 6 forward rounds after the 15% reserve,
`max_wall_seconds: 2700`); a tighter round budget cannot reach every unit.

## Observation

Same two-instrument shape as this repository's own E2E fixture: the
`filesystem` facet records the per-unit workspace tree, and the `test`
facet is a REAL execution of `scripts/check.sh` — the project's real check
command. A unit only settles when the check actually, honestly passes.

## Sandbox

Shipped conservative `none` sandbox posture: units are tier `contained`
with authority `critic`, same discipline as the repository's own E2E
fixture (only posture the shipped default admits).

## The target

The goal handed to the agent is: build `calculator.html`, a single-page
HTML calculator with digit and operator buttons and a working evaluation
path (add/subtract/multiply/divide), no build step, no network calls.
`scripts/check.sh` is the real check command — see its header comment for
exactly what it verifies. No `calculator.html` ships with this template:
writing it is the point of dispatching an agent, and `scripts/check.sh`
fails cleanly (exit 1, clear message) before any run has produced one.
