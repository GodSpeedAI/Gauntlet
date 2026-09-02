# How-To: Add an Instrumented Verification Method

This guide explains how to add a new verification kind to Gauntlet (for example, a static analysis linter, security scanner, or benchmark harness) and wire its verifier adapter into the verifier selection ladder.

---

## Goal

Introduce a new `VerifyBy` method (e.g. `security_scan`), define its evidence requirement format, and wire an instrumented verifier that produces three-state outcomes (`PASS`, `FAIL`, `INCONCLUSIVE`).

---

## Prerequisites

- Access to `crates/gauntlet-domain`, `crates/gauntlet-ports`, and `crates/gauntlet-app`.
- A clear definition of the instrument output format (e.g. SARIF JSON or tool exit code).

---

## Procedure

### Step 1: Add the Domain Enum Variant

In [`crates/gauntlet-domain/src/values.rs`](../../crates/gauntlet-domain/src/values.rs), add the new variant to `VerifyBy`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VerifyBy {
    Build,
    Test,
    StaticCheck,
    Benchmark,
    VisualDiff,
    RuntimeProbe,
    SchemaCheck,
    SecurityScan, // <-- Add new variant
    HumanSignoff,
}
```

In [`crates/gauntlet-domain/src/wire.rs`](../../crates/gauntlet-domain/src/wire.rs), ensure the wire token mapping exists:

```rust
VerifyBy::SecurityScan => "security_scan",
```

### Step 2: Implement the Verifier Adapter

Create or extend a verifier in `crates/adapters/gauntlet-adapter-verify-security-scan/` implementing the `Verifier` trait:

```rust
use gauntlet_domain::values::{EvidenceKind, Outcome};
use gauntlet_domain::unit::Unit;
use gauntlet_ports::verifier::{Verifier, VerifierError, VerifierResult};
use gauntlet_ports::observer::ObservationBundle;

pub struct SecurityScanVerifier {
    instrument_name: String,
}

impl Verifier for SecurityScanVerifier {
    fn verify(
        &self,
        unit: &Unit,
        bundle: &ObservationBundle,
    ) -> Result<VerifierResult, VerifierError> {
        // 1. Locate the security scan facet in the observation bundle
        let facet = match bundle.get_facet("security_scan") {
            Some(f) => f,
            None => {
                // Missing evidence is INCONCLUSIVE, never FAIL or PASS
                return Ok(VerifierResult {
                    outcome: Outcome::Inconclusive,
                    reason: "Security scan facet not present in bundle".into(),
                    evidence_ref: None,
                });
            }
        };

        // 2. Inspect the SARIF / scan report for vulnerabilities matching unit.claim
        if facet.has_blocking_findings() {
            Ok(VerifierResult {
                outcome: Outcome::Fail,
                reason: "Vulnerabilities detected by scanner".into(),
                evidence_ref: Some(facet.evidence_ref.clone()),
            })
        } else {
            Ok(VerifierResult {
                outcome: Outcome::Pass,
                reason: "0 blocking vulnerabilities found".into(),
                evidence_ref: Some(facet.evidence_ref.clone()),
            })
        }
    }

    fn is_deterministic(&self) -> bool {
        true
    }
}
```

### Step 3: Register in the Verifier Ladder

In [`crates/gauntlet-app/src/verify/ladder.rs`](../../crates/gauntlet-app/src/verify/ladder.rs), map the `VerifyBy::SecurityScan` variant to the new verifier in the evaluation ladder:

```rust
match unit.verify_by {
    VerifyBy::Build => self.build_verifier.verify(unit, bundle),
    VerifyBy::Test => self.test_verifier.verify(unit, bundle),
    VerifyBy::SecurityScan => self.security_verifier.verify(unit, bundle),
    // ...
}
```

---

## Validation

1. Run the verifier contract suite:
   ```bash
   cargo nextest run -p gauntlet-ports verifier
   ```
2. Run architecture boundary tests:
   ```bash
   cargo nextest run -p gauntlet-arch-tests
   ```
3. Test with missing evidence: Verify that omitting the scanner artifact produces `Outcome::Inconclusive` and prevents settlement.

---

## Common Failure Symptoms

| Symptom | Cause | Fix |
|---|---|---|
| Unit fails to parse from `answer-key.md` | Unknown `verify_by` string in YAML/Markdown. | Ensure wire token in `wire.rs` matches spelling in markdown exactly. |
| Settlement succeeds without scan running | Verifier defaulted missing facet to `PASS`. | Invariant I8 violation! Return `Outcome::Inconclusive` when facet is missing. |
