---
reference:
  manifest: .wayfinder/reference.yaml
  answer_key: .wayfinder/answer-key.md
  map: .wayfinder/map.md
  comparison_contract: .gauntlet/comparison-contract.yaml
  digest_algo: sha256
roles:
  dir: .agents/roles
observation:
  facets: [versioning, filesystem, build, test]
review:
  mode: adaptive
  max_critics: 4
verification:
  prefer_deterministic: true
budget:
  max_rounds: 8
  max_parallel: 2
  max_depth: 1
  max_wall_seconds: 3600
  max_tokens: 2000000
  max_cost_usd: 25
  recovery_reserve_pct: 15
  diagnostic_budget: 2
workspace:
  use_worktrees: true
state:
  backend: postgres
evidence:
operator:
  surfaces: [cli]
---

# GAUNTLET.md

This repository's runtime policy contract (spec §8.2). The front matter above
is the configuration the Gauntlet runtime resolves for every run here; the
sections below are documentation. Every value is literal-resolvable on this
host — no `$VAR` indirections, because this repository sets none of them.

## Runners

This repository deliberately configures no runners yet. Which concrete
AgentRunner the default `runners.default` should name for day-to-day use is
open question UNK-003 in the frozen reference; runner selection arrives via
`$GAUNTLET_AGENT_RUNNER` and the per-role variables once the runner adapters
land (plan Tasks 15-16). Until then `gauntlet doctor` reports the runner
check as honestly unavailable rather than pretending a default exists.

## Sandbox

The sandbox backend deliberately stays at the conservative default (`none`,
spec §8.6): tier policies and a real backend decision land with the
containment task (plan Task 14). Policy files live under
`.gauntlet/policies`, one file per dispatchable tier; no policy file may
ever exist for the top tier — its presence is a configuration error the
doctor fails on (AK-049, spec §17.3).

## Directories

The state and evidence directories are omitted above and take the
conservative defaults; real persistence locations land with the state and
evidence stores (plan Task 6) and are overridable through
`GAUNTLET_STATE_DIR` / `GAUNTLET_ARTIFACT_DIR`. Run reports (including
`doctor.json`) land under `.agents/reports` (spec §8.5).
