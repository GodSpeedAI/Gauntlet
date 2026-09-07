# Configuration

Spec §8 is normative. This document describes the configuration surface as
shipped; per-unit conformance truth lives in `.agents/reports/conformance.json`
and `.agents/reports/handoff.md`.

## Precedence

Three layers, one order — later wins (spec §8.1):

```text
built-in defaults  <  GAUNTLET.md front matter  <  .env / process environment  <  CLI flags
```

The effective configuration is digested into `Run.config_fingerprint`, so a
run's behavior is attributable to the exact configuration that produced it.
Nothing is hardcoded outside the declared defaults table (spec §8.6):
paths, models, providers, commands, timeouts, and tier policies are data.

## `GAUNTLET.md`

The repository policy contract (spec §8.2). Discovery: `--config <path>` →
`$GAUNTLET_CONFIG_DIR/GAUNTLET.md` → `./GAUNTLET.md`. The file is OPTIONAL
YAML front matter plus OPTIONAL H2 documentation sections; unknown keys are
ignored for forward compatibility but listed by `gauntlet doctor` so typos
surface. This repository's own `GAUNTLET.md` resolves every value literally
(no `$VAR` indirections) and deliberately configures **no runners** — see
"Runners" below.

## Environment variables

The canonical set is spec Annex E. Groups:

- **Paths:** `GAUNTLET_PROJECT_ROOT`, `_CONFIG_DIR`, `_STATE_DIR`,
  `_ARTIFACT_DIR`, `_PLAN_DIR`, `_REPORT_DIR`, `_TMP_DIR`
- **Reference:** `_REFERENCE_PATH`, `_REFERENCE_MANIFEST`,
  `_REFERENCE_DIGEST`, `_ANSWER_KEY_PATH`, `_MAP_PATH`,
  `_COMPARISON_CONTRACT`, …
- **Runners:** `_AGENT_RUNNER`, `_<ROLE>_RUNNER`, `_<ROLE>_MODEL`
- **Tools:** `_MCP_CONFIG`, `_TOOL_REGISTRY`, `_MCP_ALLOWLIST`,
  `_MCP_REQUIRED`, `_TOOL_<CAPABILITY>`
- **Limits:** `_MAX_ROUNDS`, `_MAX_PARALLEL`, `_MAX_DEPTH`,
  `_MAX_WALL_SECONDS`, `_MAX_TOKENS`, `_MAX_COST_USD`,
  `_RECOVERY_RESERVE_PCT`, `_DIAGNOSTIC_BUDGET`, `_REVIEW_MODE`, …
- **Permissions:** `_ALLOW_INSTALL`, `_ALLOW_NETWORK`, `_ALLOW_DESTRUCTIVE`,
  `_ALLOW_ORCHESTRATOR_IMPLEMENTATION`, `_VERSIONING_STRATEGY`
- **Containment:** `_SANDBOX_BACKEND`, `_SANDBOX_POLICY_DIR`,
  `_SANDBOX_BINARY`, `_SANDBOX_EXPERIMENTAL`
- **Persistence / surfaces / promotion / test hooks:** see Annex E.

All are optional, all defaulted, none required for a bounded run to start
safely. `.env.example` at the repository root lists every recognized
variable with empty values — the contract test
`tests/contract/env_example_complete` keeps it exhaustive.

Secrets are referenced by `$VAR` indirection only: resolved, validated for
presence, never printed, never written into generated files, prompts,
envelopes, or evidence (spec §8.1, §22.4).

## Semantic tool capabilities

Tools bind by semantic capability — `code_nav`, `filesystem`, `versioning`,
`test`, `build`, `browser`, `visual`, `web`, `research`, `security`,
`sandbox`, `memory`, `evidence`, `domain`, `policy`, `benchmark`,
`schema` — never by product (spec §8.4). `GAUNTLET_TOOL_CODE_NAV=rust-analyzer`
and `GAUNTLET_TOOL_CODE_NAV=<some MCP server>` are the same decision at
different addresses. If a capability named by `GAUNTLET_MCP_REQUIRED`
cannot be resolved, `gauntlet doctor` fails and the run does not start.

## Defaults table (conservative bootstrap)

| Setting | Default | Note |
|---|---|---|
| `max_rounds` | 8 | bounded even if unset |
| `max_parallel` | 2 | parallel writers corrupt each other's assumptions |
| `max_depth` | 1 | recursion is raised deliberately |
| `max_wall_seconds` | 3600 | |
| `recovery_reserve_pct` | 15 | a run must always afford to stop cleanly |
| `diagnostic_budget` | 2 | diagnosis is bounded too |
| `review.mode` | `adaptive` | pay for judgment only where instruments cannot decide |
| `sandbox.backend` | `none` | `contained` only; any higher tier without a backend refuses dispatch |
| `interactive` | auto-detect TTY | unattended runs record decisions instead of asking |

## Preflight

`gauntlet doctor` (spec §8.5) runs the eight required checks — config
parse and `$VAR` resolution; reference manifest + answer-key parse with the
tier/authority coupling rule; runner resolution per dispatchable role;
runner capability vs configured tiers; sandbox policy files exist for the
dispatchable tiers and none exists for `irreversible`; `GAUNTLET_MCP_REQUIRED`
resolvability; writable state/evidence/workspace roots with the answer key
outside every builder-writable path; parseable non-zero budget with
recovery reserve below 100%. Output: machine-readable
`$GAUNTLET_REPORT_DIR/doctor.json` plus a human summary; non-zero exit on
any required failure.

## Runners

This repository deliberately names no `runners.default`: which concrete
runner day-to-day runs should use is the operator's standing decision
(unknown UNK-003 in the frozen reference). `gauntlet run` fails closed with
a typed configuration error when no runner resolves; `gauntlet doctor`
measures the real runners it can probe (prime-agent via a version probe,
the generic CLI runner via a parse-and-render template probe) and reports
the selection as honestly unresolved rather than inventing one. The E2E
fixtures resolve a runner at fixture level — the recorded delegation
pattern (tasks 20, 25, 26).

## Proven vs provisional

Status categories are spec §26's six; unit-level claims defer to
`.agents/reports/conformance.json`.

- implemented_but_provisional — AK-015: the four-layer precedence merge and
  the run config fingerprint live in `crates/gauntlet-app/src/config/`
  (resolve, canonical, fingerprint, env-layer, GAUNTLET.md decoder), with
  unit tests at each layer (task 5).
- implemented_but_provisional — AK-016: no path, model, provider, command,
  timeout, or tier-policy literal outside the declared defaults table —
  `tests/contract/no_hardcoded_literals`; the application layer's literal
  inventory went from 290 sites to zero at task 26 COR-3 with 26 inventoried
  write sites carrying reason and guard.
- implemented_but_provisional — AK-017: `gauntlet doctor` performs the §8.5
  checks and exits non-zero on required failure
  (`crates/gauntlet-app/src/doctor/`, `crates/gauntlet-cli/src/doctor_cmd.rs`;
  the AK-049 locator divergence is the recorded 2026-08-17
  reference-candidates entry).
- implemented_but_provisional — AK-019: `.env.example` lists every
  recognized `GAUNTLET_` variable with empty values and no credentials
  (`tests/contract/env_example_complete`).
