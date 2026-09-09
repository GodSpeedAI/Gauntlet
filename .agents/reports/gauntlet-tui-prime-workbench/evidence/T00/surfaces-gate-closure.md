# T00 Evidence — Surfaces-Feature Gate Closure

Date: 2026-09-08
Task: T00
Scope: Closure of the surfaces-feature gate hole recorded in `gate_binding_rule` and Adversarial Finding F8.

## Problem Addressed

The entire workbench read path (`gauntlet serve`, the HTTP JSON operator surface, and its integration tests) sits behind `--features surfaces` in `crates/gauntlet-cli`.
Previously, `just test` (and consequently `just ci` and `just verify`) ran only `cargo nextest run --workspace` with default features, which never compiled or executed the `surfaces`-gated test suites (`operator_actions_no_bypass` and `cli_surface` serve tests).

## Changes Made

In `justfile`:
1. Updated `test` recipe to execute both the workspace default tests and the surfaces test suite:
   ```just
   test:
       cargo nextest run --workspace
       cargo nextest run -p gauntlet-cli --features surfaces --test operator_actions_no_bypass --test cli_surface
   ```
2. Added a dedicated developer alias:
   ```just
   test-surfaces:
       cargo nextest run -p gauntlet-cli --features surfaces --test operator_actions_no_bypass --test cli_surface
   ```

## Teeth Attack & Verification

1. **Nominal Pass**:
   Command: `just test-surfaces`
   Result: PASS (exit 0) — 17 tests passed in ~2 seconds (13 tests in `operator_actions_no_bypass`, 4 tests in `cli_surface`).

2. **Deliberate Mutation (Teeth)**:
   Attack: In `crates/gauntlet-cli/tests/cli_surface.rs:91`, mutated the expected help output to `"DELIBERATE_FAIL"`.
   Command: `just test-surfaces`
   Result: FAIL (exit 100), catching the broken assertion immediately:
   `error: test run failed`
   `error: recipe test-surfaces failed on line 28 with exit code 100`

3. **Byte-for-Byte Restoration**:
   Restored `crates/gauntlet-cli/tests/cli_surface.rs` to original byte-identical state.
   Command: `just test-surfaces`
   Result: PASS (exit 0).
