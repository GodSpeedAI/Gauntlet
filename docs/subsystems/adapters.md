# Subsystem: Adapters Layer (`crates/adapters/*`)

The adapters layer provides concrete, technology-specific implementations of Gauntlet's semantic capability ports.

---

## 1. Purpose

Adapters bridge Gauntlet's abstract ports to real-world infrastructure: filesystem hierarchies, PostgreSQL databases, operating system sandboxes (Bubblewrap), external process runners, test harnesses, and model APIs. By keeping adapters strictly at the periphery, Gauntlet can swap runtime dependencies without altering core control logic.

---

## 2. Responsibilities

- **Implement Port Traits**: Translate port trait calls into operating system, library, or process interactions.
- **Pass Shared Contract Suites**: Every adapter must satisfy the unmodified contract test suite for its port located in `crates/gauntlet-ports/src/contract/`.
- **Maintain Architectural Containment**: Never expose vendor-specific types inward to `gauntlet-domain` or `gauntlet-app`.
- **Fail Explicitly**: Convert underlying I/O, database, or process failures into typed port error enums carrying the exact failure locus.

---

## 3. Catalog of the 24 Adapters

The repository includes 24 modular adapter crates organized by capability family:

```text
crates/adapters/
├── Agent Runners (3)
│   ├── gauntlet-adapter-agent-mock
│   ├── gauntlet-adapter-agent-cli
│   └── gauntlet-adapter-agent-prime
├── State, Evidence & Workspace (4)
│   ├── gauntlet-adapter-state-postgres
│   ├── gauntlet-adapter-evidence-fs
│   ├── gauntlet-adapter-workspace-fs
│   └── gauntlet-adapter-escalation-fs
├── Governance, ID & Metering (3)
│   ├── gauntlet-adapter-id-entropy
│   ├── gauntlet-adapter-meter-memory
│   └── gauntlet-adapter-policy-fs
├── Observers (5)
│   ├── gauntlet-adapter-observe-build
│   ├── gauntlet-adapter-observe-command
│   ├── gauntlet-adapter-observe-filesystem
│   ├── gauntlet-adapter-observe-git
│   └── gauntlet-adapter-observe-test
├── Comparators (2)
│   ├── gauntlet-adapter-compare-diff
│   └── gauntlet-adapter-compare-model
├── Verifiers (4)
│   ├── gauntlet-adapter-verify-command-exit-code
│   ├── gauntlet-adapter-verify-file-digest
│   ├── gauntlet-adapter-verify-schema-check
│   └── gauntlet-adapter-verify-test-report
├── Sandboxing (2)
│   ├── gauntlet-adapter-sandbox-bwrap
│   └── gauntlet-adapter-sandbox-none
└── Operator Surfaces (1)
    └── gauntlet-adapter-surface-json
```

---

## 4. Subsystem Details by Family

### 4.1 Agent Runners
- **`gauntlet-adapter-agent-mock`**: Scripted, deterministic runner driven by scenario fixtures in `tests/fixtures/mock_runner_scenarios/*.yaml`. Replays happy paths, timeouts, malformed result docs, contract mismatches, and reference mutation attacks. Essential for deterministic testing without token costs.
- **`gauntlet-adapter-agent-cli`**: Generic process-backed runner that executes any command-line agent tool (e.g. `claude`, `aider`, `codex`) based on a configured command template (`GAUNTLET_CLI_AGENT_CMD`). Polls process status and checks for the existence of the result contract file.
- **`gauntlet-adapter-agent-prime`**: Adapter for recursive agent harnesses (such as Prime-Agent). Generates the kernel invocation, captures admission handles (`rlm_child_id`, `session_dir`), and polls the result contract asynchronously without blocking.

### 4.2 State, Evidence & Workspace
- **`gauntlet-adapter-state-postgres`**: Embedded PostgreSQL persistence storing runs, iterations, role invocations, residuals, verification records, and settlements. Implements `StateStore` and `EventLog`. Supports crash-injection testing and deterministic JSONL export.
- **`gauntlet-adapter-evidence-fs`**: Content-addressed filesystem store (`/tmp/gauntlet/evidence/sha256/...`). Implements write-once immutability; has no overwrite path in its API.
- **`gauntlet-adapter-workspace-fs`**: Creates and destroys isolated workspaces (using git worktrees or filesystem mirrors) rooted outside the host repository.
- **`gauntlet-adapter-escalation-fs`**: Filesystem-backed human escalation queue storing pending, resolved, or rejected tickets in `.gauntlet/escalations/`.

### 4.3 Governance & Metering
- **`gauntlet-adapter-id-entropy`**: Generates production ULIDs (lexicographically sortable 128-bit identifiers).
- **`gauntlet-adapter-meter-memory`**: In-memory thread-safe `BudgetMeter` enforcing token, wall-clock, round, depth, parallel, and USD limits, while safeguarding the 15% recovery reserve.
- **`gauntlet-adapter-policy-fs`**: Reads and parses tier policy files from `.gauntlet/policies/<tier>.json`. Ensures no top-tier policy file exists for `irreversible`.

### 4.4 Observers
- **`gauntlet-adapter-observe-build`**: Probes compilation reality (e.g. `cargo check`, `npm run build`), capturing exit codes and diagnostic outputs.
- **`gauntlet-adapter-observe-test`**: Executes tests through `cargo nextest` or configured test runners, generating structured `test_report` facets.
- **`gauntlet-adapter-observe-filesystem`**: Takes directory snapshots, computing SHA-256 digests of modified files.
- **`gauntlet-adapter-observe-git`**: Interrogates git worktrees for committed changes, branch state, and uncommitted diffs.
- **`gauntlet-adapter-observe-command`**: Generic observer executing arbitrary shell commands and capturing structured stdout/stderr streams.

### 4.5 Comparators
- **`gauntlet-adapter-compare-diff`**: Deterministic comparator calculating textual and structural diffs against expected reference baselines.
- **`gauntlet-adapter-compare-model`**: Model-assisted comparator that consumes a blind comparison envelope and emits structured `Residual`s under strict JSON schemas.

### 4.6 Verifiers
- **`gauntlet-adapter-verify-command-exit-code`**: Verifies that a declared command executed with the expected exit status (usually 0).
- **`gauntlet-adapter-verify-test-report`**: Inspects JUnit/nextest JSON reports to confirm that the specific test named in `evidence_required.locator` passed.
- **`gauntlet-adapter-verify-file-digest`**: Compares the SHA-256 digest of an artifact against an expected value.
- **`gauntlet-adapter-verify-schema-check`**: Validates generated JSON/YAML artifacts against declared JSON schemas.

### 4.7 Sandboxes
- **`gauntlet-adapter-sandbox-bwrap`**: Linux process isolation using Bubblewrap (`bwrap`). Restricts network access, mounts root read-only, and binds only designated workspace directories.
- **`gauntlet-adapter-sandbox-none`**: Conservative default sandbox that permits execution only for tier `contained`. Refuses execution for higher tiers, triggering an escalation ticket.

### 4.8 Operator Surfaces
- **`gauntlet-adapter-surface-json`**: HTTP JSON API server (feature `surfaces`) providing read endpoints (`/state`, `/runs/:id`) and action endpoints (`/resolve`, `/retry`). Shares the exact same `SettlementService` validation path as the CLI.

---

## 5. Shared Contract Verification

To ensure strict substitutability, each adapter crate contains a contract test under its `tests/` directory:

```rust
// In crates/adapters/gauntlet-adapter-agent-mock/tests/contract_suite.rs
use gauntlet_adapter_agent_mock::MockAgentRunner;
use gauntlet_ports::contract::agent_runner;

#[test]
fn mock_runner_passes_shared_contract() {
    agent_runner::test_runner_contract(|spec| {
        Box::new(MockAgentRunner::from_scenario(spec))
    });
}
```

---

## Source Trail

- [`crates/adapters/gauntlet-adapter-agent-mock/src/lib.rs`](../../crates/adapters/gauntlet-adapter-agent-mock/src/lib.rs) — Mock runner implementation.
- [`crates/adapters/gauntlet-adapter-agent-cli/src/lib.rs`](../../crates/adapters/gauntlet-adapter-agent-cli/src/lib.rs) — Process-backed CLI agent runner.
- [`crates/adapters/gauntlet-adapter-agent-prime/src/lib.rs`](../../crates/adapters/gauntlet-adapter-agent-prime/src/lib.rs) — Recursive Prime-Agent runner adapter.
- [`crates/adapters/gauntlet-adapter-state-postgres/src/lib.rs`](../../crates/adapters/gauntlet-adapter-state-postgres/src/lib.rs) — PostgreSQL state store and event log.
- [`crates/adapters/gauntlet-adapter-evidence-fs/src/lib.rs`](../../crates/adapters/gauntlet-adapter-evidence-fs/src/lib.rs) — Content-addressed filesystem evidence store.
- [`crates/adapters/gauntlet-adapter-sandbox-bwrap/src/lib.rs`](../../crates/adapters/gauntlet-adapter-sandbox-bwrap/src/lib.rs) — Bubblewrap process isolation backend.
