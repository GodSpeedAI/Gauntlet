# Explanation: Three-State Verification Logic

This document details why Gauntlet's verification subsystem is grounded in three-state logic (`PASS`, `FAIL`, `INCONCLUSIVE`), and why boolean coercion is strictly forbidden.

---

## 1. The Flaw of Two-State Logic in Agent Systems

Traditional software testing often treats test outcomes as binary: true or false. If a test passes, it is true; if it fails, it is false.

In autonomous agent environments, this binary assumption breaks down catastrophically:
- A test was not executed because the build timed out.
- A test was skipped because a network dependency was unreachable.
- A test was commented out by an agent.
- A test runner crashed before emitting results.
- A tool returned an empty response.

In a two-state system, unexecuted tests are either defaulted to `FAIL` (causing uninformative retries) or, worse, treated as `PASS` because "no failure was reported" (null-pointer fallacy).

---

## 2. Invariant I8: Missing Evidence is Not Passing Evidence

Gauntlet models truth using a strict three-state type:

```rust
// In crates/gauntlet-domain/src/values.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Outcome {
    Pass,
    Fail,
    Inconclusive,
}
```

| Outcome | Strict Meaning | Can It Settle a Unit? |
|---|---|---|
| **`PASS`** | The required instrument evidence was collected, inspected, and held true. | **Yes** |
| **`FAIL`** | The required instrument evidence was collected and contradicted the claim. | **No** |
| **`INCONCLUSIVE`** | The evidence needed to check the claim is unavailable, stale, or malformed. | **No** (Blocks settlement) |

> **The Non-Negotiable Rule**: *`INCONCLUSIVE` is never a pass.* If a test timed out, an assertion was skipped, or an evidence file is missing, the Unit remains inconclusive. A run cannot settle with any blocking unit in an inconclusive state.

---

## 3. Why `impl From<Outcome> for bool` is Forbidden

A subtle vulnerability in Rust codebases is implementing convenient boolean conversion helpers:

```rust
// THIS IS FORBIDDEN IN GAUNTLET (Anti-Gaming G12)
impl From<Outcome> for bool {
    fn from(o: Outcome) -> bool {
        match o {
            Outcome::Pass => true,
            Outcome::Fail | Outcome::Inconclusive => false,
        }
    }
}
```

Why is this forbidden?
Because a developer or agent writing control logic will eventually write:

```rust
if outcome.into() {
    settle_unit();
} else {
    retry();
}
```

This collapses the crucial distinction between:
- A known defect that requires a specific code correction (`Outcome::Fail`).
- A missing test fixture or broken toolchain that requires diagnostic repair (`Outcome::Inconclusive`).

To prevent this gaming vector, `crates/gauntlet-arch-tests` scans the codebase and fails the build if any boolean conversion trait or `is_ok()` shortcut is added to `Outcome`.

---

## 4. Evidence Binding and Staleness

A verifier does not merely look for any passing test report; it performs **evidence binding**:
1. **Producer Check**: If `evidence_required.producer` is `instrument`, a model assertion is rejected.
2. **Locator Match**: The test name in the JUnit report must match `evidence_required.locator` (e.g. `tests/auth.rs::test_account_lockout`).
3. **Temporal Freshness**: The evidence artifact must have been created *during or after* the builder transform it certifies. If a builder modifies `src/auth.rs` in iteration 3, a test report generated during iteration 2 is rejected as stale.

---

## 5. Proof Test (P12)

The three-state verification invariant is proven in `crates/gauntlet-cli/tests/p12_missing_is_not_passing.rs`:
- Tests verify that deleting an evidence file immediately yields `Outcome::Inconclusive`.
- Tests assert that `SettlementService` refuses settlement for any unit with an inconclusive outcome.
- Tests prove that stale evidence from an earlier iteration is rejected.

---

## Source Trail

- [`crates/gauntlet-domain/src/values.rs`](../../crates/gauntlet-domain/src/values.rs) — `Outcome` definition and value rules.
- [`crates/gauntlet-app/src/verify/ladder.rs`](../../crates/gauntlet-app/src/verify/ladder.rs) — Verifier selection and evidence binding.
- [`crates/gauntlet-cli/tests/p12_missing_is_not_passing.rs`](../../crates/gauntlet-cli/tests/p12_missing_is_not_passing.rs) — Proof test P12.
- [`crates/gauntlet-arch-tests/src/denylist.rs`](../../crates/gauntlet-arch-tests/src/denylist.rs) — Architecture boundary scanner preventing boolean conversions.
