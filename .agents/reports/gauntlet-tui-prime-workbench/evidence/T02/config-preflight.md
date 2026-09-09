# T02 Evidence — Configuration, Preflight, and Degraded-Mode Contract

Date: 2026-09-08
Task: T02
Proof Level: P2
Scope: Configuration authority, precedence resolution (CLI > ENV > file > defaults), secret indirection and redaction, typed startup preflight error hierarchy, and degraded-mode contract.

## Settled Requirements

- **REQ-CONFIG-001**: Deterministic configuration precedence: CLI flags > environment variables (`GAUNTLET_WORKBENCH_*`) > configuration file > documented defaults.
- **REQ-CONFIG-002**: Secret and credential indirection: tokens referenced indirectly; secrets redacted in diagnostics and logs (`[REDACTED]`).
- **REQ-CONFIG-003**: Configurable endpoints, stores, roots, and budgets without hardcoded machine/provider facts.
- **REQ-CONFIG-005**: Transport classification (`local` loopback vs `remote`) without altering canonical operation meanings.
- **REQ-CONFIG-010**: Startup preflight validates Gauntlet reachability, Prime integration compatibility, projection-store write permissions, and required configuration.
- **REQ-CONFIG-012**: Blast-radius preflight failure scoping:
  - Missing Prime degrades Prime-dependent views gracefully while core Gauntlet read inspection remains available.
  - Loss of canonical Gauntlet connectivity fails closed on all state-changing commands and marks cached state stale.
- **REQ-SAFE-003**: Safe degraded modes under component loss.
- **REQ-SEC-003**: Zero cleartext secrets in logs, artifacts, or projection rows.
- **REQ-UX-006**: Visibly surfaced degraded connection status.

## Architecture & Code Delivered

1. **`workbench/src/config.ts`**:
   - `WorkbenchConfig` interface (`gauntlet_endpoint`, `prime_endpoint`, `projection_store`, `plugin_roots`, `performance_budget`, `transport`, `session_token`).
   - `resolveConfig(options)`: Deterministic 4-tier precedence resolver.
   - `classifyTransport(endpoint)`: Classifies local loopback (`localhost`, `127.0.0.1`, `::1`) vs remote endpoints.
   - `redactConfig(config)`: Redacts secret tokens for safe diagnostics and logging.

2. **`workbench/src/preflight.ts`**:
   - Typed error taxonomy: `PreflightError` with `code`, `message`, `blast_radius`, `recoverability_hint`, `source`.
   - `evaluatePreflight(config, probes)`: Enforces fail-closed rules and capability scoping under component degradation.
   - `runRuntimePreflight(config, probeOverride)`: Probes endpoints and evaluates operational readiness.

3. **`workbench/src/config-preflight.test.ts`**:
   - 7 automated tests validating precedence, secret redaction, and both teeth attacks.

## Teeth Falsification & Verification

### Attack 1: Remove Prime while keeping Gauntlet reachable
- **Result**: `report.ok === true`, `report.degraded === true`, `report.gauntlet_status === "connected"`, `report.prime_status === "unavailable"`.
- Safe Gauntlet read-only capabilities (`state_snapshot`, `unit_inspection`, `escalation_inbox`) remain available.
- Prime-dependent capabilities (`prime_chat`, `prime_steering`, `prime_subagent_spawn`) are blocked with typed `prime_integration_error`.
- **PASS**.

### Attack 2: Remove Gauntlet connectivity while cached projections and Prime exist
- **Result**: `report.ok === false`, `report.gauntlet_status === "unavailable"`.
- All state-changing mutation commands fail closed (`blocked_capabilities` includes `state_mutation`, `action_resolve`, `action_retry`, `evidence_admission`).
- Emits typed `canonical_source_unavailable` error with blast radius and recoverability hint.
- **PASS**.

## Gate Results

| Gate Command | Result |
| :--- | :--- |
| `just tui-test config-preflight` | PASS (7/7 passed) |
| `just tui-preflight bootstrap` | PASS (all 6 bootstrap integrity checks passed) |
| `just tui-check` | PASS (0 errors, strict TypeScript clean) |
| `just tui-lint` | PASS (0 errors, Biome clean) |

Confirmation: peer CONFIRM (all gates pass, both teeth attacks confirmed load-bearing).
