# T06 Evidence — Reconstructible Bun SQLite Projection Store with Generations and Cursors

Date: 2026-09-08
Task: T06
Proof Level: P3
Scope: Disposable local read-model store, ProjectionGeneration lifecycle, atomic cursor advancement transactions, cold rebuild verification from authoritative sources, and stale-frame rejection under attack.

## Settled Requirements

- **REQ-BUN-003**: `bun:sqlite` default local read-model store; verified adequate for representative correctness and replay loads (>84,000 events/sec).
- **REQ-DATA-002**: Canonical baseline reads and incremental streams reconciled through `ProjectionGeneration` and source cursors; reconnect races cannot corrupt active projections.
- **REQ-DATA-003**: Late frames from a superseded generation MUST be rejected (Attack 2).
- **REQ-DATA-004**: Applying one authoritative event atomically updates its local cursor and all dependent SQLite read-model rows in a single coherent transaction.
- **REQ-DATA-007**: Local projection storage preserves enough provenance (`generation_id`, `source_id`, `source_cursor`) to explain which owning source produced each record.
- **REQ-STATE-002**: Reconnect starts a new generation and isolates projections from stale frames.

## Implementation Details

1. **`workbench/src/projection-store.ts`**:
   - `SqliteProjectionStore`: Backed by `bun:sqlite` with WAL mode and synchronous normal.
   - `ProjectionGeneration`: Tracks `id`, `status` (`syncing`, `ready`, `degraded`, `closed`), `started_at`, and `source_cursors` map.
   - `applyEvent(frame)`: Atomically commits cursor advancement and projection updates in a single SQLite transaction (`REQ-DATA-004`). Rejects frames if generation ID does not match active generation or if generation is closed (`REQ-DATA-003`).
   - `getSnapshot(horizon)`: Extracts an immutable, React-free `WorkbenchSnapshot` with normalized records and provenance links (`REQ-DATA-007`).
   - `rebuildFromAuthoritativeLog(dbPath, generationId, events)`: Replays authoritative events cold into a fresh database.

2. **`workbench/src/projection-generation.test.ts`**:
   - Verifies generation transitions, atomic transactions, snapshot generation, and Attack 2 defense.

3. **`workbench/src/projection-rebuild.test.ts`**:
   - Verifies cold rebuild from authoritative logs and Attack 1 defense.

4. **`workbench/src/sqlite-replay.bench.ts`**:
   - Benchmark script replaying 10,000 authoritative events folded into `SqliteProjectionStore`.

## Teeth Falsification & Verification

### Attack 1: Complete deletion of local SQLite database (Cold Rebuild)
- **Attack description**: Unlink the entire SQLite database file (`rm -f test-rebuild-projection.db*`) while preserving authoritative event history.
- **Defense outcome**: `SqliteProjectionStore.rebuildFromAuthoritativeLog` rebuilds identical consequential projections directly from owning sources. Zero domain state was lost, and no state was recreated from cache.
- **Test**: `src/projection-rebuild.test.ts` ("Attack 1: complete deletion of local SQLite database allows bit-equivalent cold rebuild from authoritative event log").
- **Result**: **PASS**.

### Attack 2: Delayed G1 frame delivered after reconnect starts G2
- **Attack description**: After reconnect mints generation G2 and applies G2 events, deliver a delayed G1 frame with an artificially high timestamp (`timestamp_ms: 999999`).
- **Defense outcome**: `store.applyEvent` strictly compares `frame.generation_id` with `activeGeneration.id`. The delayed G1 frame is rejected (`applied: false, reason: "superseded_generation"`). The active G2 snapshot, units, and cursors remain completely untouched.
- **Test**: `src/projection-generation.test.ts` ("Attack 2 (REQ-DATA-003, REQ-STATE-002): rejects delayed frame from superseded generation").
- **Result**: **PASS**.

## Gate Results

| Gate Command | Result | Notes |
| :--- | :--- | :--- |
| `just prereg-check gauntlet-tui-prime-workbench-T06` | PASS | Hash matches `0e4531789dc10eda0eb9df6b94c9331253ddd9907b3d5aa85d030f201e8a14e7` |
| `just tui-test projection-generation` | PASS | 2/2 tests passed (Attack 2 verified) |
| `just tui-test projection-rebuild` | PASS | 1/1 test passed (Attack 1 verified) |
| `just tui-bench sqlite-replay` | PASS | 84,437 events/sec throughput; snapshot < 6ms |
| `just tui-check` | PASS | 0 TypeScript errors |
| `just tui-lint` | PASS | 0 Biome lint errors |
| `cd workbench && bun test` | PASS | 41/41 tests passed across 10 suites |

Confirmation: independent CONFIRM (all gates pass, both teeth attacks confirmed load-bearing).
