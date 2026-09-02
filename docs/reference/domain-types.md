# Reference: Domain Types & Aggregates

This document provides a comprehensive technical reference for all value objects, newtypes, fixed-value-set enums, and aggregates defined in `crates/gauntlet-domain`.

---

## 1. Value Objects & Newtypes (`values.rs`)

All identifiers and domain primitives are modeled as strongly-typed newtypes to prevent stringly-typed state.

| Type | Underlying Representation | Validation Constraints | String Formatting / Wire Format |
|---|---|---|---|
| `RunId` | 128-bit ULID | Lexicographically sortable, collision-resistant. | 26-character Crockford Base32 string. |
| `UnitId` | String | Must match regex `/^[A-Z]{2,8}-[0-9]{3,6}$/`. | E.g. `AK-014`, `INV-001`. |
| `Digest` | `(Algorithm, String)` | Must carry algorithm prefix (`sha256:...`). Bare hex is invalid. | E.g. `sha256:d82e4a91b...`. |
| `IterationNo` | `u32` | 1-based monotonic integer; 0 is invalid. | Displayed as `1`, `2`, `3`... |
| `RoleId` | Enum + Custom | Standard roles: `orchestrator`, `builder`, `critic`, `verifier`, `observer`, `optimizer`. Custom matches `/^[a-z][a-z0-9-]{1,31}$/`. | Wire token strings. |
| `EvidenceRef` | `Digest` | Content-addressed SHA-256 hash of the evidence file. | E.g. `sha256:4f8a91...`. |
| `MicroUsd` | `u64` | Financial amount in millionths of a USD ($1.00 = 1,000,000). | Safe integer math without floating-point errors. |

---

## 2. Fixed-Value-Set Enums (`values.rs`)

Unknown enum strings in parsed input are treated as **parse errors**, never defaulted.

### `Outcome` (Three-State Logic)
```rust
pub enum Outcome {
    Pass,
    Fail,
    Inconclusive,
}
```
- **Constraints**: No boolean conversion trait (`From<Outcome> for bool`) is implemented. `Inconclusive` cannot settle a Unit.

### `BlastRadius` (Safety Tiers)
```rust
pub enum BlastRadius {
    Contained,     // Scratch files in temporary workspace only
    Local,         // Source code within target repository
    CrossCutting,  // Repo-wide config, toolchains, CI/CD
    Irreversible,  // Production deploy, cloud changes, secret rotation
}
```

### `Authority`
```rust
pub enum Authority {
    Critic,      // Read-only model evaluation
    LeadAgent,   // Automated multi-agent dispatch
    Human,       // Requires operator signoff
}
```

### `VerifyBy`
```rust
pub enum VerifyBy {
    Build,         // Compiler exit status
    Test,          // Unit/integration test report
    StaticCheck,   // Linter / static analysis
    Benchmark,     // Performance measurement
    VisualDiff,    // Screenshot / visual comparison
    RuntimeProbe,  // Live daemon health check
    SchemaCheck,   // JSON / YAML schema validation
    SecurityScan,  // SAST / vulnerability scanner
    HumanSignoff,  // Attested human review
}
```

### `TerminalState`
```rust
pub enum TerminalState {
    Settled,          // All blocking units verified PASS
    Blocked,          // Dependency deadlock or missing prerequisite
    BudgetExhausted,  // Recovery reserve reached or limits exceeded
    Escalated,        // Ticket queued for operator review
    Aborted,          // Reference tampering or safety violation
    Inconclusive,     // Evidence permanently unavailable
}
```

---

## 3. Core Aggregates

### `Unit` ([`unit.rs`](../../crates/gauntlet-domain/src/unit.rs))
Represents one checkable claim.
- **Fields**:
  - `id: UnitId`: Unique identifier.
  - `claim: String`: Natural language falsifiable assertion.
  - `verify_by: VerifyBy`: Verification method.
  - `blast_radius: BlastRadius`: Tier safety constraint.
  - `authority: Authority`: Minimum authority required to execute.
  - `depends_on: Vec<UnitId>`: Dependencies that must settle before this unit can run.
  - `evidence_required: EvidenceRequirement`: Kind, locator, and producer.
  - `evidence: Option<EvidenceRef>`: Pinned evidence artifact upon passing.
  - `status: UnitStatus`: `Open`, `InProgress`, `Settled`, `Blocked`, `OutOfScope`.
- **Constructor Invariants**:
  - `blast_radius == Irreversible` $\implies$ `authority == Human`.
  - `blast_radius in {CrossCutting, Irreversible}` $\implies$ `authority != Critic`.
  - `status == Settled` requires `evidence.is_some()`.

### `Reference` ([`reference.rs`](../../crates/gauntlet-domain/src/reference.rs))
Represents the comparison standard.
- **Fields**:
  - `id: ReferenceId`: Stable identifier.
  - `version: u32`: Version number.
  - `digest: Digest`: Cryptographic hash over normalized manifest.
  - `purpose: String`: High-level transformation objective.
  - `desired_state: String`: Target end-state description.
  - `invariants: Vec<InvariantDef>`: Invariant statements that must hold true.
  - `constraints: Vec<ConstraintDef>`: Operational and architectural constraints.
  - `anti_goals: Vec<AntiGoalDef>`: Explicitly rejected outcomes.
  - `quality_dimensions: Vec<QualityDimension>`: Non-blocking subjective rubrics.
  - `settlement: SettlementCriteria`: Conditions required for certified conclusion.

### `Budget` ([`budget.rs`](../../crates/gauntlet-domain/src/budget.rs))
Tracks multi-dimensional resource consumption.
- **Fields**:
  - `limits: Limits`: Configured ceilings (rounds, parallel, depth, wall time, tokens, USD).
  - `spent: Spent`: Accumulated consumption.
  - `recovery_reserve_pct: u8`: Percentage reserved for rollback and cleanup (1–99%).
  - `diagnostic_budget: u32`: Allowable diagnostic exploration cycles.
- **Methods**:
  - `would_breach_reserve(increment) -> bool`: Checks if next step enters reserve.
  - `charge(payment) -> Result<(), BudgetError>`: Debits spent counters safely.

---

## Source Trail

- [`crates/gauntlet-domain/src/values.rs`](../../crates/gauntlet-domain/src/values.rs) — Value objects, newtypes, and enums.
- [`crates/gauntlet-domain/src/unit.rs`](../../crates/gauntlet-domain/src/unit.rs) — `Unit` struct and invariant checks.
- [`crates/gauntlet-domain/src/reference.rs`](../../crates/gauntlet-domain/src/reference.rs) — `Reference` aggregate.
- [`crates/gauntlet-domain/src/budget.rs`](../../crates/gauntlet-domain/src/budget.rs) — `Budget`, `Limits`, and `Spent`.
- [`crates/gauntlet-domain/src/wire.rs`](../../crates/gauntlet-domain/src/wire.rs) — Wire token string mappings.
