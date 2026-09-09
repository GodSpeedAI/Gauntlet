# T04 Evidence — Prime Structured Host Adapter and Session Topology

Date: 2026-09-08
Task: T04
Proof Level: P3
Scope: Prime Agent headless host adapter, nonblocking child admission handles, mailbox communication vs task completion separation, and recursive parent-child session topology reconstruction without terminal scraping or synthetic replacement IDs.

## Settled Requirements

- **REQ-GOAL-003**: Native terminal integration through structured headless host protocols without terminal screen scraping.
- **REQ-OPS-001**: Consequential actions flow through canonical paths; conversational mailbox messages do not settle tasks.
- **REQ-PRIME-001**: Structured host adapter lifecycle managing child Prime Agent sessions.
- **REQ-PRIME-002**: Recursive parent-child topology reconstruction from durable records without synthetic ID generation.
- **REQ-PRIME-003**: Mailbox protocol with clear separation between communication signals and verified task completion contracts.
- **REQ-PRIME-005**: Durable identity preservation across adapter restarts.

## Architecture & Code Delivered

1. **`workbench/src/prime-host.ts`**:
   - `PrimeAdmissionHandle`: `session_id`, `parent_session_id`, `model`, `admitted_at`, `contract_path`, `status`.
   - `PrimeMailboxMessage`: Structured messages (`communication`, `progress`, `signal`).
   - `PrimeResultContract`: `session_id`, `schema_valid`, `outcome`, `evidence_path`, `completed_at`.
   - `PrimeHostAdapter`:
     - `spawnChild(...)`: Nonblocking child admission, returns handle immediately in `< 50ms`.
     - `receiveMailboxMessage(...)`: Appends message to session log. Child status explicitly remains `running` even if message says "done".
     - `deliverResultContract(...)`: Only transitions child to `completed` when verified result contract is delivered.

2. **`workbench/src/prime-topology.ts`**:
   - `PrimeSessionRecord`: Durable record format for session state.
   - `PrimeTopologyNode`: Recursive tree structure with parent-child links.
   - `reconstructTopology(records)`: Reconstructs exact hierarchy from durable records, preserving original session IDs and relations without synthetic replacements.
   - `findSessionInTopology(...)`: Helper for tree traversal.

3. **Automated Test Suites**:
   - `workbench/src/prime-host.test.ts`: Nonblocking admission and Attack 1 (mailbox message containing "done" does not settle child).
   - `workbench/src/prime-topology.test.ts`: Attack 2 (restart adapter and verify exact topology reconstruction without synthetic IDs).

## Teeth Falsification & Verification

### Attack 1: Send a Prime mailbox message containing "done" before a valid child result exists
- **Test**: `src/prime-host.test.ts` (Attack 1).
- **Result**:
  - Subagent sends `"Task execution is fully DONE and settled! You can mark me complete now."`.
  - Message is recorded in mailbox history.
  - Child status remains `running` (`expect(session.handle.status).toBe("running")`), and `resultContract` remains `null`.
  - Child transitions to `completed` only after `deliverResultContract` delivers a schema-valid contract.
  - **PASS**.

### Attack 2: Spawn nested Prime children and restart only the workbench adapter process
- **Test**: `src/prime-topology.test.ts` (Attack 2).
- **Result**:
  - Multi-tier hierarchy: Root -> Builder 1 -> Grandchild Falsifier 1a, and Root -> Builder 2.
  - Fresh adapter instance reconstructs topology directly from durable records.
  - Reconstructed tree matches original IDs byte-for-byte (`session-root-orchestrator`, `session-child-builder-1`, `session-grandchild-falsifier-1a`).
  - Zero synthetic replacement IDs minted. Zero terminal scraping used.
  - **PASS**.

## Gate Results

| Gate Command | Result |
| :--- | :--- |
| `just tui-test prime-host` | PASS (2/2 passed) |
| `just tui-test prime-topology` | PASS (1/1 passed) |
| `just tui-check` | PASS (0 errors, strict TypeScript clean) |
| `just tui-lint` | PASS (0 errors, Biome clean) |

Confirmation: fresh_target_reproduction CONFIRM (all gates pass, both teeth attacks confirmed load-bearing).
