# Reference: Configuration & Environment (`GAUNTLET.md`)

This document specifies the configuration schema for `GAUNTLET.md`, the environment variable overrides (Annex E), and the conservative defaults table (spec §8.6).

---

## Precedence Hierarchy

Gauntlet resolves settings using a 4-tier precedence ladder:

$$\text{Defaults Table} < \text{GAUNTLET.md Front Matter} < \text{Environment Variables} < \text{CLI Flags}$$

The final merged configuration is hashed using SHA-256 into `config_fingerprint` and recorded on the run.

---

## 1. `GAUNTLET.md` Front Matter Schema

The project root must contain `GAUNTLET.md` with YAML front matter:

```yaml
---
reference:
  manifest: .wayfinder/reference.yaml        # Path to reference manifest
  answer_key: .wayfinder/answer-key.md       # Path to checkable Units
  map: .wayfinder/map.md                     # Path to formation map
  comparison_contract: .gauntlet/comparison-contract.yaml
  digest_algo: sha256                        # Hash algorithm (default: sha256)

roles:
  dir: .agents/roles                         # Role definition directory
  default: null                              # Default runner id (e.g. prime-agent, cli-agent)

observation:
  facets: [versioning, filesystem, build, test]

review:
  mode: adaptive                             # single | fanout | adaptive | high_assurance
  max_critics: 4

verification:
  prefer_deterministic: true                 # Always favor instruments over models

budget:
  max_rounds: 8                              # Maximum corrective iterations
  max_parallel: 2                            # Maximum concurrent builder subagents
  max_depth: 1                               # Maximum recursion depth
  max_wall_seconds: 3600                     # Wall clock limit (1 hour)
  max_tokens: 2000000                        # Maximum combined LLM tokens
  max_cost_usd: 25                           # Maximum financial spend limit in USD
  recovery_reserve_pct: 15                   # Quarantined budget for rollback (15%)
  diagnostic_budget: 2                       # Diagnostic cycles upon non-convergence

workspace:
  use_worktrees: true                        # Use git worktrees for isolation
  root: /tmp/gauntlet/workspaces             # Base path for scratch workspaces

state:
  backend: sqlite                            # State persistence backend (sqlite)
  dir: /tmp/gauntlet/state                   # Directory for state database

evidence:
  dir: /tmp/gauntlet/evidence                # Content-addressed evidence directory

sandbox:
  backend: none                              # none | bwrap

operator:
  surfaces: [cli]                            # [cli] or [cli, json]
---
```

---

## 2. Conservative Defaults Table (Spec §8.6)

If any configuration setting is omitted, the runtime falls back to these hardcoded conservative values declared in [`crates/gauntlet-app/src/config/defaults.rs`](../../crates/gauntlet-app/src/config/defaults.rs):

| Setting | Conservative Default | Spec Reference | Rationale |
|---|---|---|---|
| `budget.max_rounds` | `8` | §8.6, §11.1 | Prevents infinite loops while allowing normal convergence. |
| `budget.max_parallel` | `2` | §8.6 | Bounds thread and memory pressure on constrained hosts. |
| `budget.max_depth` | `1` | §8.6, §10.5 | Forbids unbounded recursive sub-agent spawning. |
| `budget.max_wall_seconds` | `3600` (1 hour) | §8.6 | Guards against hanging background processes. |
| `budget.max_tokens` | `2,000,000` | §8.6 | Bounds total frontier LLM token costs. |
| `budget.max_cost_usd` | `25` USD | §8.6 | Caps financial liability per run. |
| `budget.recovery_reserve_pct` | `15`% | §8.6, §17.4 | Guarantees budget exists to roll back broken runs. |
| `budget.diagnostic_budget` | `2` cycles | §8.6, §15.2 | Bounds diagnostic search upon non-convergence. |
| `review.mode` | `ReviewMode::Adaptive` | §8.6, §13.4 | Skips expensive critics for deterministic checks. |
| `review.max_critics` | `4` | §8.6 | Bounds fan-out critic concurrency. |
| `verification.prefer_deterministic` | `true` | §8.6, §14.1 | Prioritizes compilers and tests over model judgment. |
| `sandbox.backend` | `"none"` | §8.6, §17.2 | Fails closed: refuses execution above `contained`. |
| `reference.digest_algo` | `"sha256"` | §8.6, §9.3 | Standard cryptographic normalization digest. |
| `control.nonconvergence_n` | `3` | §8.6, §15.2 | Triggers diagnostic engine after 3 identical residuals. |
| `promotion.min_settlements` | `3` | §19.2 (inline default) | Minimum settlements under recorded variation a capability promotion requires; a configured 0 is refused. |

---

## 3. Environment Variable Reference (Annex E)

Any configuration key can be overridden via environment variables:

| Variable | Description | Accepted Values |
|---|---|---|
| `GAUNTLET_AGENT_RUNNER` | Selects the active AgentRunner adapter. | `prime-agent`, `cli-agent`, `mock-agent` |
| `GAUNTLET_CLI_AGENT_CMD` | Command template for the generic CLI runner. | E.g. `claude --prompt {prompt_file} --workspace {workspace}` |
| `GAUNTLET_MOCK_SCENARIO` | Fixture stem for the mock runner adapter. | E.g. `happy_path`, `lying_builder`, `mutating_builder` |
| `GAUNTLET_STATE_DIR` | Directory where SQLite state and event logs live. | Filesystem path (default: `/tmp/gauntlet/state`) |
| `GAUNTLET_ARTIFACT_DIR` | Directory for content-addressed immutable evidence. | Filesystem path (default: `/tmp/gauntlet/evidence`) |
| `GAUNTLET_REPORT_DIR` | Directory where run reports and doctor.json land. | Filesystem path (default: `.agents/reports`) |
| `GAUNTLET_SANDBOX_BACKEND` | Active process isolation backend. | `bwrap`, `none` |
| `GAUNTLET_BUDGET_MAX_ROUNDS` | Overrides max iteration rounds. | Positive integer |
| `GAUNTLET_BUDGET_MAX_WALL_SECONDS` | Overrides wall-clock timeout in seconds. | Positive integer |
| `GAUNTLET_BUDGET_MAX_TOKENS` | Overrides max LLM tokens. | Positive integer |
| `GAUNTLET_BUDGET_MAX_COST_USD` | Overrides max dollar spend. | Numeric float or integer |
| `GAUNTLET_RECOVERY_RESERVE_PCT` | Overrides recovery reserve percentage. | `1` to `99` |
| `GAUNTLET_CLOCK` | Test hook for deterministic virtual time. | `deterministic` or unset (wall clock) |
| `GAUNTLET_ID_SEED` | Test hook for deterministic ULID generation. | Integer seed or unset (entropy) |
| `GAUNTLET_API_BIND` | Network bind address for JSON API server. | E.g. `127.0.0.1:8080` (feature: `surfaces`) |

---

## Source Trail

- [`crates/gauntlet-app/src/config/defaults.rs`](../../crates/gauntlet-app/src/config/defaults.rs) — The single declared defaults module.
- [`crates/gauntlet-app/src/config/gauntlet_md.rs`](../../crates/gauntlet-app/src/config/gauntlet_md.rs) — Front-matter parser and validator.
- [`crates/gauntlet-app/src/config/mod.rs`](../../crates/gauntlet-app/src/config/mod.rs) — Precedence resolution engine and `config_fingerprint`.
- [`crates/gauntlet-arch-tests/src/literals.rs`](../../crates/gauntlet-arch-tests/src/literals.rs) — Architectural scanner enforcing that defaults exist only in `defaults.rs`.
