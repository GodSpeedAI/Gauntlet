# T01 Evidence — Canonical Gauntlet State and Operation Boundary

Date: 2026-09-08
Task: T01
Proof Level: P3
Scope: Headless application boundary establishing canonical Gauntlet state/event reads and consequential operation intents, actor/session authentication, capability negotiation, and deep Unit inspection.

## Settled Requirements

- **REQ-AFF-001**: Stable affordance path and operation identities between TUI and Gauntlet kernel.
- **REQ-GOAL-002**: Verification, evidence, and settlement truth remains exclusively owned by Gauntlet.
- **REQ-OPS-001**: Consequential actions flow strictly through canonical Gauntlet admission pipeline.
- **REQ-OPS-002**: Operation boundary rejects client-side presentation side effects.
- **REQ-OPS-003 / D1**: Request-level actor and session authentication (`Authorization: Bearer <token>`, `X-Gauntlet-Operator: <actor>`). Loopback reachability alone never confers mutation authority.
- **REQ-STATE-001**: Deep Unit inspection over Answer Key, verification records, escalation tickets, and bounded evidence content.
- **REQ-STATE-005**: Projections for Case, WorkItem, and AttentionItem bound to source-spec vocabulary without inventing new canonical aggregates.
- **REQ-SYNC-001**: Version negotiation and capability handshake (`GET /api/v1/capabilities`).

## Architecture & Code Delivered

1. **App Domain & State Inspection (`crates/gauntlet-app/src/state/inspection.rs`)**:
   - `inspect_unit(unit_id, answer_key, state_store, escalation_queue, evidence_store)`:
     - Parses Answer Key requirement facts (`claim`, `tier`, `authority`, `blast_radius`, `verify_by`, `evidence_required`, `depends_on`).
     - Extracts durable verification records and settlements (`settled: bool`).
     - Resolves active escalation tickets and reasons.
     - Derives canonical status (`Settled`, `Blocked`, `Failed`, `Running`, `Pending`).
     - Extracts bounded evidence content and explains denial/preconditions.
   - Projections: `CaseProjection`, `WorkItemProjection`, `AttentionItemProjection`.

2. **HTTP Surface & Auth (`crates/adapters/gauntlet-adapter-surface-json/src/http.rs`, `views.rs`)**:
   - Capabilities handshake (`GET /api/v1/capabilities`):
     - Protocol version: `1.0.0`
     - Server version: `0.3.0`
     - Capabilities: `state_snapshot`, `state_refresh`, `escalation_inbox`, `run_scoped_view`, `unit_scoped_view`, `unit_inspection`, `action_resolve`, `action_retry`, `session_auth`.
     - Unsupported operations: `launch`, `steering`, `cancellation`, `evidence_admission`.
   - Unit deep inspection endpoint: `GET /api/v1/units/{id}/inspection`.
   - Operation boundary session authentication:
     - `SessionAuthenticator` trait with `TokenSessionAuthenticator`.
     - Mutation endpoints (`POST /api/v1/units/{id}/resolve` and `/retry`) require valid `Authorization: Bearer <token>` and `X-Gauntlet-Operator`.
     - Unauthenticated requests: 401 `actor_authentication_required`.
     - Invalid token: 401 `invalid_session_credentials`.
     - Mismatched client-asserted actor: 403 `client_asserted_authority_refused`.

3. **CLI Durable Surface Wiring (`crates/gauntlet-cli/src/durable_surface.rs`, `serve_cmd.rs`)**:
   - Implemented `unit_inspection` on `RunScopedReader` for `DurableSurface`.
   - Wired `GAUNTLET_API_TOKEN` / `GAUNTLET_SESSION_TOKEN` and `GAUNTLET_SESSION_ACTOR` into `SurfaceHttp`.

4. **TypeScript Workbench Client & Tests (`workbench/src/client.ts`, `workbench/src/canonical-boundary.test.ts`)**:
   - Added `CapabilitiesResponse`, `UnitInspectionView`, `fetchCapabilities()`, `fetchInspection()`, and `mutateUnit()`.
   - Automated tests covering the 3 teeth attacks.

## Teeth Falsification & Verification

### Attack 1: Loopback Mutation without Bound Session or with Asserted Actor Mismatch
- **Test**: `src/canonical-boundary.test.ts` (Attack 1) and `tests/http.rs` (`attack1_mutation_without_bound_session_or_with_asserted_actor_is_refused`).
- **Result**:
  - Request with no `Authorization` header -> HTTP 401 (`actor_authentication_required`).
  - Request with invalid bearer token -> HTTP 401 (`invalid_session_credentials`).
  - Request with token for Alice but header `X-Gauntlet-Operator: mallory` -> HTTP 403 (`client_asserted_authority_refused`).
  - Loopback reachability alone never authorizes mutation. **PASS**.

### Attack 2: TUI-Side Helper Attempting Direct State/Evidence Mutation
- **Test**: `src/canonical-boundary.test.ts` (Attack 2).
- **Result**:
  - `GauntletReadClient` exposes zero direct settlement or evidence admission write methods.
  - Direct calls to `/api/v1/units/{id}/settle` or `/evidence_admission` are rejected with HTTP 405 (`unsupported_operation`).
  - No side path exists to mint settlements outside the canonical Gauntlet engine. **PASS**.

### Attack 3: Headless Replay of Canonical Read/Event Data
- **Test**: `src/canonical-boundary.test.ts` (Attack 3).
- **Result**:
  - Replayed canonical snapshot and deep unit inspection in headless environment without renderer, OpenTUI, SQLite read model, or Prime session.
  - All fields (Answer Key requirements, verification facts, escalation ticket, precondition explanation, bounded evidence) parsed with 100% fidelity. **PASS**.

## Gate Results

| Gate Command | Result |
| :--- | :--- |
| `just tui-test canonical-boundary` | PASS (4/4 passed) |
| `just tui-check` | PASS (0 errors, strict TypeScript clean) |
| `just tui-lint` | PASS (0 errors, Biome clean) |
| `just check-package gauntlet-app` | PASS (0 warnings, 0 errors) |
| `cargo test -p gauntlet-app --lib state::inspection` | PASS (6/6 passed) |
| `just test-package gauntlet-adapter-surface-json` | PASS (13/13 passed, HTTP + contract) |
| `just test-surfaces` | PASS (operator_actions_no_bypass & cli_surface pass) |

Confirmation: builder CONFIRM (all 6 gate commands PASS; all 3 teeth attacks validated load-bearing).
