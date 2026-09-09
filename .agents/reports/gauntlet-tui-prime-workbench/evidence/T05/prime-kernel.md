# T05 Evidence — Prime Python REPL Computation, Control, & Kernel-Object Lifecycle

Date: 2026-09-08
Task: T05
Proof Level: P2
Scope: Prime Python REPL execution serialization, host-mediated control, object lifecycle classification (ephemeral/pinned/artifactized), and separation of kernel liveness from unit settlement.

## Settled Requirements

- **REQ-PRIME-004**: One Prime kernel serializes shell/Python execution; concurrent requests are queued deterministically without interleaving.
- **REQ-PRIME-006**: Kernel objects are classified into explicit lifecycle states: ephemeral (in-memory REPL variables), pinned (retained working set across cells), and artifactized (persisted to disk with evidence locator).
- **REQ-PRIME-007**: Host-mediated input/control and execution interrupt capabilities.
- **REQ-SAFE-004**: Kernel liveness is never confused with work success: an active running REPL session cannot settle a unit without a verified evidence artifact at the canonical locator.

## Implementation Details

1. **`workbench/src/prime-kernel.ts`**:
   - `PrimeReplKernel`: Manages Python REPL sessions with a serialized `Promise` chain execution queue.
   - `execute(code, options)`: Returns `ReplExecuteResult` with strictly monotonic `execution_order` and registered ephemeral variable references.
   - `interrupt()`: Sets an interrupt flag to halt active/pending execution and emit `status: "interrupted"`.
   - `transitionLifecycle(name, target, persistedPath)`: Explicitly transitions variables between `ephemeral`, `pinned`, and `artifactized`.
   - `canSettleUnit(unitId, requiredEvidenceArtifact)`: Enforces `REQ-SAFE-004`. Rejects settlement if kernel is dead, if no evidence artifact is referenced, or if the artifact has not been verified/artifactized from kernel state.

2. **`workbench/src/prime-kernel.test.ts`**:
   - Automated tests exercising REPL execution, serialization, interrupts, object lifecycle transitions, and settlement validation under attack.

## Teeth Falsification & Verification

### Attack 1: Attempt concurrent execute requests against single session
- **Attack description**: Fire two concurrent `kernel.execute(...)` calls without awaiting the first before launching the second.
- **Defense outcome**: The execution queue chains the promises sequentially. The first execution completes with `execution_order: 1` before the second begins with `execution_order: 2`. No stdout or state interleaving occurs.
- **Test**: `src/prime-kernel.test.ts` ("Attack 1: serializes concurrent execution requests without interleaving or race conditions").
- **Result**: **PASS**.

### Attack 2: Kernel liveness without artifactized evidence cannot settle unit (REQ-SAFE-004)
- **Attack description**: A live kernel session with active in-memory variables attempts to claim unit completion/settlement without an artifactized evidence file.
- **Defense outcome**: `canSettleUnit` returns `{ allowed: false }` with explicit refusal ("verified artifactized evidence path is required"). Even if a path is supplied, it is rejected unless the object has been formally transitioned to `artifactized` with the canonical locator.
- **Test**: `src/prime-kernel.test.ts` ("Attack 2 (REQ-SAFE-004): kernel liveness != work success").
- **Result**: **PASS**.

## Gate Results

| Gate Command | Result |
| :--- | :--- |
| `just prereg-check gauntlet-tui-prime-workbench-T05` | PASS (hash matches `dc7df5615900063a6c03b9bb884cadc472605c79e22312ff5f5c8368a3721d1a`) |
| `just tui-test prime-kernel` | PASS (5/5 passed) |
| `cd workbench && bun test` | PASS (38/38 passed across 8 files) |
| `just tui-check` | PASS (0 errors) |
| `just tui-lint` | PASS (0 errors) |

Confirmation: peer CONFIRM (all gates pass, both teeth attacks confirmed load-bearing).
