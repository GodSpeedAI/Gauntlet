# Reference: Ports API (`gauntlet-ports`)

This document specifies the method signatures, parameters, return types, and typed error enums for the 17 semantic capability ports defined in `crates/gauntlet-ports`.

---

## Governing Port Rules (Spec §6)

1. **Provider-Agnostic**: No port method accepts or returns a provider-specific type (e.g. no OpenAI, Anthropic, PostgreSQL, or Bubblewrap structs).
2. **Typed Errors**: Every port returns a typed error enum carrying the failure locus (e.g. `AgentError`, `ObserverError`, `StateStoreError`). Bare strings or untyped errors are forbidden.
3. **Deadlines on Blocking Calls**: Every method that can block takes an explicit `Deadline` parameter.

---

## 1. `AgentRunner` ([`agent_runner.rs`](../../crates/gauntlet-ports/src/agent_runner.rs))

Admission-based runner interface for executing agent roles.

```rust
pub trait AgentRunner: Send + Sync {
    /// Admits a role invocation, binds sandbox permissions and contract paths,
    /// and returns an InvocationHandle immediately.
    fn invoke(&self, spec: &RoleInvocationSpec) -> Result<InvocationHandle, AgentError>;

    /// Probes execution state without blocking.
    fn poll(&self, handle: &InvocationHandle, deadline: Deadline) -> Result<InvocationStatus, AgentError>;

    /// Collects and parses the role_result@v1 payload from disk.
    fn collect(&self, handle: &InvocationHandle) -> Result<RoleResultDoc, AgentError>;

    /// Cancels a running invocation.
    fn cancel(&self, handle: &InvocationHandle, reason: CancelReason) -> Result<(), AgentError>;

    /// Reports static runner capabilities and isolation rungs.
    fn capabilities(&self) -> RunnerCapabilities;
}
```

- **Error Enum**: `AgentError` (`ProcessSpawnFailed`, `TimedOut`, `MalformedResult`, `ContractMismatch`, `Cancelled`).

---

## 2. `Observer` ([`observer.rs`](../../crates/gauntlet-ports/src/observer.rs))

Interface for non-model instruments inspecting reality.

```rust
pub trait Observer: Send + Sync {
    /// Collects raw facts for a specific facet from the workspace.
    fn observe_facet(
        &self,
        request: &ObservationRequest,
        deadline: Deadline,
    ) -> Result<ObservationFacet, ObserverError>;

    /// Declares which facet kinds this observer supports.
    fn supported_facets(&self) -> &[FacetKind];
}
```

- **Error Enum**: `ObserverError` (`ProbeExecutionFailed`, `FacetUnavailable`, `Timeout`, `MalformedOutput`).

---

## 3. `Comparator` ([`comparator.rs`](../../crates/gauntlet-ports/src/comparator.rs))

Interface for independent, isolated reviewers evaluating observations against the reference.

```rust
pub trait Comparator: Send + Sync {
    /// Evaluates a blind comparison envelope and emits materialized residuals.
    fn compare(
        &self,
        envelope: &ComparisonEnvelope,
        deadline: Deadline,
    ) -> Result<ComparisonRecordDoc, ComparatorError>;
}
```

- **Error Enum**: `ComparatorError` (`ModelFailure`, `MalformedEnvelope`, `EvaluationTimeout`).

---

## 4. `Verifier` ([`verifier.rs`](../../crates/gauntlet-ports/src/verifier.rs))

Interface for certifying specific Unit claims against hard instrument evidence.

```rust
pub trait Verifier: Send + Sync {
    /// Evaluates a Unit claim against an ObservationBundle.
    fn verify(
        &self,
        unit: &Unit,
        bundle: &ObservationBundle,
    ) -> Result<VerifierResult, VerifierError>;

    /// Declares whether this verifier is deterministic.
    fn is_deterministic(&self) -> bool;
}
```

- **Error Enum**: `VerifierError` (`EvidenceUnresolvable`, `LocatorNotFound`, `ProducerMismatch`).

---

## 5. `StateStore` ([`state_store.rs`](../../crates/gauntlet-ports/src/state_store.rs))

Persistence interface for runs, iterations, invocations, residuals, and units.

```rust
pub trait StateStore: Send + Sync {
    fn record_run(&self, run: &Run) -> Result<(), StateStoreError>;
    fn get_run(&self, run_id: &RunId) -> Result<Option<Run>, StateStoreError>;
    fn record_iteration(&self, iter: &Iteration) -> Result<(), StateStoreError>;
    fn settle_unit(&self, unit_id: &UnitId, evidence: &EvidenceRef) -> Result<(), StateStoreError>;
    fn export_jsonl(&self, run_id: &RunId, writer: &mut dyn std::io::Write) -> Result<(), StateStoreError>;
}
```

---

## 6. `EvidenceStore` ([`evidence_store.rs`](../../crates/gauntlet-ports/src/evidence_store.rs))

Content-addressed, write-once evidence storage.

```rust
pub trait EvidenceStore: Send + Sync {
    /// Writes an evidence artifact; returns its content-addressed SHA-256 Digest.
    fn store_evidence(&self, content: &[u8]) -> Result<EvidenceRef, EvidenceStoreError>;

    /// Reads an evidence artifact by its Digest.
    fn get_evidence(&self, reference: &EvidenceRef) -> Result<Vec<u8>, EvidenceStoreError>;

    /// Pins an evidence artifact to protect it from garbage collection.
    fn pin(&self, run_id: &RunId, reference: &EvidenceRef) -> Result<(), EvidenceStoreError>;
}
```

---

## 7. `EventLog` ([`event_log.rs`](../../crates/gauntlet-ports/src/event_log.rs))

Monotonic append-only log of runtime transitions.

```rust
pub trait EventLog: Send + Sync {
    /// Appends a typed event record with fsync durability.
    fn append_event(&self, event: &LogEntry) -> Result<u64, EventLogError>;

    /// Reads all events starting from offset.
    fn read_events(&self, from_offset: u64) -> Result<Vec<LogEntry>, EventLogError>;
}
```

---

## 8. `Sandbox` ([`sandbox.rs`](../../crates/gauntlet-ports/src/sandbox.rs))

Process isolation and containment backend.

```rust
pub trait Sandbox: Send + Sync {
    /// Pre-execution dry-run validation.
    fn validate(&self, policy: &SandboxPolicy) -> Result<(), SandboxError>;

    /// Wraps a builder command line in isolation arguments before execution.
    fn wrap_command(
        &self,
        command: &[String],
        policy: &SandboxPolicy,
        workspace: &Path,
    ) -> Result<Vec<String>, SandboxError>;

    /// Cleans up isolated mounts and namespaces.
    fn cleanup(&self, workspace: &Path) -> Result<(), SandboxError>;
}
```

---

## 9. `BudgetMeter` ([`budget_meter.rs`](../../crates/gauntlet-ports/src/budget_meter.rs))

Multi-dimensional resource metering.

```rust
pub trait BudgetMeter: Send + Sync {
    /// Checks whether an expenditure would breach the recovery reserve.
    fn would_breach(&self, slice: &BudgetSlice) -> bool;

    /// Debits spent counters atomically.
    fn charge(&self, slice: &BudgetSlice) -> Result<(), BudgetMeterError>;

    /// Returns remaining allowance across all dimensions.
    fn remaining(&self) -> Remaining;
}
```

---

## Source Trail

- [`crates/gauntlet-ports/src/lib.rs`](../../crates/gauntlet-ports/src/lib.rs) — Port definitions index.
- [`crates/gauntlet-ports/src/agent_runner.rs`](../../crates/gauntlet-ports/src/agent_runner.rs) — `AgentRunner` trait.
- [`crates/gauntlet-ports/src/observer.rs`](../../crates/gauntlet-ports/src/observer.rs) — `Observer` trait.
- [`crates/gauntlet-ports/src/comparator.rs`](../../crates/gauntlet-ports/src/comparator.rs) — `Comparator` trait.
- [`crates/gauntlet-ports/src/verifier.rs`](../../crates/gauntlet-ports/src/verifier.rs) — `Verifier` trait.
- [`crates/gauntlet-ports/src/state_store.rs`](../../crates/gauntlet-ports/src/state_store.rs) — `StateStore` trait.
- [`crates/gauntlet-ports/src/evidence_store.rs`](../../crates/gauntlet-ports/src/evidence_store.rs) — `EvidenceStore` trait.
- [`crates/gauntlet-ports/src/sandbox.rs`](../../crates/gauntlet-ports/src/sandbox.rs) — `Sandbox` trait.
