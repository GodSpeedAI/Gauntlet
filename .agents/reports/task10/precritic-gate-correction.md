# Task 10 — Pre-critic gate correction

**Date:** 2026-08-20
**Result:** GREEN

## Scope and diagnosis

The required `just boundaries` pre-critic gate initially exited 100 solely on
8 `hardcoded-relative-path` findings: two scanner reports tied to the
manifest-relative comparison-contract test location in
`crates/gauntlet-app/src/compare/contract.rs`, and six reports for three
manifest-relative repository/artifact locations in
`crates/gauntlet-cli/tests/p2_context_isolation.rs`.

## Exact correction

- In `crates/gauntlet-app/src/compare/contract.rs`, the repository-artifact
  parsing test now obtains the repository root through `CARGO_MANIFEST_DIR`
  ancestors and composes the contract location using the declared
  `RUNTIME_DIR_NAME` and `COMPARISON_CONTRACT_NAME` defaults.
- In `crates/gauntlet-cli/tests/p2_context_isolation.rs`, added a single
  manifest-ancestor `repo_root` helper plus `frozen_answer_key_path` and
  `comparison_contract_path` helpers. Those helpers compose locations from
  `WAYFINDER_DIR_NAME`, `ANSWER_KEY_NAME`, `RUNTIME_DIR_NAME`, and
  `COMPARISON_CONTRACT_NAME`; all three previous inline relative locations now
  call them.

The tests still read the same frozen inputs and assert the same P2 behavior.
No frozen Wayfinder content, Core spec, comparison-contract artifact or
criteria, reference candidate, prior record, or unrelated production code was
modified.

## Evidence

Fail-first acceptance evidence:

1. `just boundaries` — **FAIL** (exit 100) before the correction, reporting
   exactly the 8 findings above and no other gate failure.

Green evidence, run sequentially after the correction:

1. `cargo fmt -- crates/gauntlet-app/src/compare/contract.rs crates/gauntlet-cli/tests/p2_context_isolation.rs` — **PASS**.
2. `cargo nextest run -p gauntlet-app the_repository_artifact_parses_into_the_typed_view` — **PASS** (1/1).
3. `cargo nextest run -p gauntlet-cli --test p2_context_isolation` — **PASS** (2/2, including `teeth`).
4. `cargo fmt --check` — **PASS**.
5. `just boundaries` — **PASS** (194/194).

## Residuals

None within the bounded gate-correction scope. This proof is limited to
removing environment-coupled test literals while preserving the existing test
inputs and assertions; it does not re-evaluate Task 10 comparator semantics.
