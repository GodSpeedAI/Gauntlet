# Reference: Error Catalog & Failure Loci

In Gauntlet, error handling is strictly typed and carries the failure locus. Swallowing errors, returning bare strings, or converting uncertainty into passing outcomes is forbidden.

---

## The 16 Failure Classes (Spec §21.2)

Every error in Gauntlet maps to one of sixteen architectural failure loci:

| Failure Locus | Description | Typical Trigger |
|---|---|---|
| `configuration` | Malformed or conflicting runtime configuration. | Invalid `GAUNTLET.md` front matter or missing required secret. |
| `reference` | Broken reference manifest, malformed Units, or digest mismatch. | Inconsistent Answer Key or reference tampering during execution. |
| `formation` | Inability to form a stable reference due to unresolved fog. | Ambiguous user requirements or missing destination. |
| `workspace` | Filesystem or git worktree provisioning failure. | Disk full, invalid permissions, or git worktree conflict. |
| `agent_execution` | Failure during builder or critic invocation. | Sub-agent process crash, malformed result contract, timeout. |
| `tool_execution` | Instrument failure during observation. | Compiler crash, missing build tool, or syntax error in test. |
| `observation` | Inability to extract or normalize evidence facets. | Git diff failure, unreadable files, or missing timestamps. |
| `comparison` | Model or diff comparison error. | Invalid JSON payload from critic, malformed envelope. |
| `verification` | Claim cannot be checked against declared evidence. | Missing JUnit XML, stale evidence, or locator mismatch. |
| `settlement` | Failure during settlement precondition evaluation. | Open blocking residuals, unverified units, or missing pass record. |
| `persistence` | PostgreSQL or event log I/O failure. | Database lock contention, corrupt or unreachable PostgreSQL database, disk write error. |
| `environment` | Host toolchain or platform incompatibility. | Missing Rust toolchain, unsupported OS, missing Bubblewrap. |
| `budget` | Exhaustion of iteration, token, wall-clock, or cost limits. | Run exceeded $25.00 limit or breached 15% recovery reserve. |
| `governance` | Security, permission, or blast-radius violation. | Agent attempting to execute irreversible action without human authority. |
| `recovery` | Failure during crash replay or resumption. | Event log truncation, missing pinned evidence artifact. |
| `inconclusive_evidence` | Truth of a claim cannot be resolved. | Test skipped, tool timed out, or sensor disconnected. |

---

## Domain Error Enums

### `ValueError` ([`crates/gauntlet-domain/src/values.rs`](../../crates/gauntlet-domain/src/values.rs))
- `InvalidDigest(String)`: Digest is missing algorithm prefix (`sha256:...`) or contains non-hex characters.
- `InvalidUnitId(String)`: Unit ID does not match regex `/^[A-Z]{2,8}-[0-9]{3,6}$/`.
- `UnknownWireToken(String)`: Encountered an unrecognized string token when parsing a fixed-value-set enum.
- `InvalidIteration(u32)`: Iteration number is 0 (must be $\ge 1$).

### `UnitError` ([`crates/gauntlet-domain/src/unit.rs`](../../crates/gauntlet-domain/src/unit.rs))
- `IrreversibleRequiresHuman`: A unit with `blast_radius: irreversible` was constructed with non-human authority.
- `CriticCannotAuthorizeTier`: A unit with `cross_cutting` or `irreversible` blast radius was constructed with `authority: critic`.
- `MissingEvidenceOnSettlement`: Attempted to transition a unit to `Settled` without attaching an `EvidenceRef`.

### `BudgetError` ([`crates/gauntlet-domain/src/budget.rs`](../../crates/gauntlet-domain/src/budget.rs))
- `LimitExceeded(Dimension)`: Ceilings breached for rounds, tokens, wall time, or USD.
- `ReserveBreached`: Expenditure would consume into the protected 15% recovery reserve.
- `InvalidReservePercentage(u8)`: Recovery reserve configured outside 1–99%.

---

## Port Error Enums

### `AgentError` ([`crates/gauntlet-ports/src/agent_runner.rs`](../../crates/gauntlet-ports/src/agent_runner.rs))
- `SpawnFailed(String)`: Subprocess could not be executed.
- `TimedOut(Duration)`: Invocation exceeded declared `Deadline`.
- `MalformedResult(String)`: Result contract file on disk does not validate against `role_result@v1`.
- `ContractMismatch { expected, actual }`: Result contract file has mismatched `invocation_id` or envelope digest.
- `MissingResultDocument`: Process exited cleanly but wrote no result document to the declared path.

### `VerifierError` ([`crates/gauntlet-ports/src/verifier.rs`](../../crates/gauntlet-ports/src/verifier.rs))
- `EvidenceUnresolvable(EvidenceLocator)`: Locator does not point to an existing file in the evidence store.
- `LocatorNotFound(String)`: Declared test name was not found in the test report XML/JSON.
- `ProducerMismatch`: Evidence producer does not match declared producer (`instrument` vs `human`).
- `StaleEvidence`: Evidence artifact timestamp predates the builder transform it certifies.

### `StateStoreError` ([`crates/gauntlet-ports/src/state_store.rs`](../../crates/gauntlet-ports/src/state_store.rs))
- `DatabaseError(String)`: PostgreSQL query or migration failure.
- `RecordNotFound(String)`: Specified run, iteration, or unit does not exist in the database.
- `ConcurrentWriteDetected`: Another Gauntlet process is currently writing to this state directory.

---

## Application Error Enums

### `SettlementPreconditionError` ([`crates/gauntlet-app/src/settle/mod.rs`](../../crates/gauntlet-app/src/settle/mod.rs))
- `OpenResidualsRemain`: One or more blocking residuals are unresolved.
- `MissingPassingVerification`: One or more blocking units lack a `PASS` verification record.
- `ReferenceTampered { expected, actual }`: Reference digest re-computed at settlement does not match `.gauntlet/reference.lock`.
- `EvidenceMissingFromStore`: Pinned evidence artifact has vanished from the filesystem.

---

## Source Trail

- [`crates/gauntlet-domain/src/values.rs`](../../crates/gauntlet-domain/src/values.rs) — `ValueError` and failure loci.
- [`crates/gauntlet-domain/src/unit.rs`](../../crates/gauntlet-domain/src/unit.rs) — `UnitError`.
- [`crates/gauntlet-domain/src/budget.rs`](../../crates/gauntlet-domain/src/budget.rs) — `BudgetError`.
- [`crates/gauntlet-ports/src/agent_runner.rs`](../../crates/gauntlet-ports/src/agent_runner.rs) — `AgentError`.
- [`crates/gauntlet-ports/src/verifier.rs`](../../crates/gauntlet-ports/src/verifier.rs) — `VerifierError`.
- [`crates/gauntlet-app/src/settle/mod.rs`](../../crates/gauntlet-app/src/settle/mod.rs) — `SettlementPreconditionError`.
