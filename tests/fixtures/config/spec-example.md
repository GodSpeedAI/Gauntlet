---
reference:
  manifest: .wayfinder/reference.yaml
  answer_key: .wayfinder/answer-key.md
  map: .wayfinder/map.md
  comparison_contract: .gauntlet/comparison-contract.yaml
  digest_algo: sha256
roles:
  dir: .agents/roles                  # role definitions (§10.1)
runners:
  default: $GAUNTLET_AGENT_RUNNER
  by_role:
    builder:  { runner: prime-agent, model: $GAUNTLET_BUILDER_MODEL }
    critic:   { runner: prime-agent, model: $GAUNTLET_CRITIC_MODEL }
    verifier: { runner: none }        # deterministic verifiers need no model
observation:
  facets: [versioning, filesystem, build, test]
review:
  mode: adaptive                      # ReviewMode
  max_critics: 4
verification:
  prefer_deterministic: true
sandbox:
  backend: $GAUNTLET_SANDBOX_BACKEND  # none | process | bwrap | seatbelt | mxc | microvm
  policy_dir: .gauntlet/policies      # one file per tier; irreversible MUST be absent
budget:
  max_rounds: 8
  max_parallel: 3
  max_depth: 2
  max_wall_seconds: 5400
  max_tokens: 2000000
  max_cost_usd: 25
  recovery_reserve_pct: 15
  diagnostic_budget: 2
workspace:
  root: $GAUNTLET_TMP_DIR/workspaces
  use_worktrees: true
state:
  backend: postgres
  dir: $GAUNTLET_STATE_DIR
evidence:
  dir: $GAUNTLET_ARTIFACT_DIR
operator:
  surfaces: [cli]                     # + json-api, tui, dashboard
---

## Notes

Documentation section: the parser accepts and ignores H2 content after the
front matter (spec 8.2 structure).
