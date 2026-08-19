# Task 8 — teeth demonstrations (orchestrator-run, all reverted, all re-verified)

Method per plan rule 5 / AGENTS.md "The testing loop": break the mechanism,
watch the gate fail, revert, re-verify green. All three teeth run by the
orchestrator personally after the lint-closure slices landed.

## T1 — failure-classification neutered (runner.rs `advance`, Err arm)

**Break:** `class: error.failure_class()` replaced with a fixed
`class: InvocationFailure::Execution` (decode failures no longer map to
their typed classes).

**Observed (cargo nextest run -p gauntlet-adapter-agent-mock):**
- `contract_suite the_mock_runner_passes_the_unmodified_shared_contract_suite`
  FAILED — **the unmodified Task-4 shared suite catches the neutering** (the
  AK-059 pin has teeth).
- `scenario_playback malformed_fails_and_is_never_collectible` FAILED
  (`Failed{malformed_result}` expected, got `Execution`).
- `scenario_playback wrong_invocation_id_fails_contract_mismatch` confirmed
  failing when run individually (`Failed{contract_mismatch}` expected, got
  `Execution`). In the full run it was among 4 playback tests **cancelled by
  nextest's documented per-binary fail-cancellation** after `malformed`
  failed in the same binary (57/61 run, 0 skipped) — not silently passing;
  individually runnable and failing. Siblings `stale_envelope_digest_*`,
  `no_result_*`, `the_required_fixture_inventory_*` were the other
  cancellations (all green in every full green run).

**Revert:** exact inverse edit. Re-verify: 61/61 PASS.

## T2 — REQUIRED fixture removed from AK-059's locator

**Break:** `tests/fixtures/mock_runner_scenarios/no-result.yaml` renamed to
`no-result.yaml.tooth`.

**Observed:** `scenario_playback the_required_fixture_inventory_exists_and_parses`
FAILED with the typed error
`no-result: fixture "no-result" could not be read: No such file or directory`.

**Revert:** renamed back. Re-verify: 61/61 PASS.

## T3 — capability honesty (isolation rung inflated)

**Break:** `capabilities()` `isolation: Isolation::None` → `Isolation::Process`
(the lie that would let a future scheduler dispatch contained tiers to this
unsandboxed runner).

**Observed:** `runner::tests capabilities_report_honest_containment_and_delivery`
FAILED (`left: Process, right: None`).

**Honest observation (recorded, bounded):** the shared Task-4 suite pin
(`the_mock_runner_passes_the_unmodified_shared_contract_suite`) **PASSED
under this break** — the shared suite does not currently pin adapter
capability honesty; the tooth is the adapter's own in-crate test.
Capability-vs-tier dispatch checking is a recorded Task-14 deferral
(Task-4 closure ledger), so this is a recorded residual, not a new defect:
the enforcement point lands at Task 14; until then the in-crate honesty
test is the only guard. Revisit if any task composes runner selection
before Task 14.

**Revert:** exact inverse edit. Re-verify: 61/61 PASS, ports agent_runner
3/3, `just ruler` PASS.

## Final state after all teeth

Mock 61/61 · ports agent_runner 3/3 · ruler PASS · tree restored (inverse
edits; fixture restored byte-identical by rename).
