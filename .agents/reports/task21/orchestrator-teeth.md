# Task 21 — orchestrator personal verification record

Date: 2026-09-02. All runs personally executed by the orchestrating
session; raw logs under `.agents/reports/task21/orch-*.log`. One compiler
at a time (`flock .agents/reports/task21/compile.lock`; sccache stopped
inside the same flock'd command).

## Gate battery (post a71eba9, clean source tree)

| Gate | Result |
|---|---|
| `cargo nextest run -p gauntlet-cli --features surfaces --test operator_actions_no_bypass` (PLAN GATE) | 5/5 PASS |
| `cargo build -p gauntlet-cli --no-default-features` (PLAN GATE) | exit 0 |
| mock E2E, default config (`e2e_full_loop_mock`) | 12/12 PASS |
| mock E2E, `--features surfaces` | 12/12 PASS |
| `cli_surface` + `escalations_cli` (default) | 7/7 PASS |
| `just ruler` (before work started) | PASS |

## D11 teeth — personally verified (RED → byte-identical restore → GREEN)

Baseline hashes (recorded before any mutation):
- `crates/gauntlet-app/src/settle/mod.rs` =
  `ec5cad46c2ca97d0bbea8a4c2eba71e814258eae443fa6a5b9046e7a75568481`
- `crates/gauntlet-cli/src/operator_action.rs` =
  `5d0ff76f310af0e6cd983aa6f797dbe3112a931fb8920420b9624b5f196f2176`

**Mutation 1 (service side):** deleted the
`self.settle_operator_action_gate(&request, deadline)?;` line inside
`settle_operator_action_earned` (settle/mod.rs:817, the earned arm's
only copy — the posture arm at :792 untouched).

- Sensor `settle::tests::the_earned_arm_refuses_past_the_gate_even_holding_
  a_chain_minted_pass_claim`: **RED** — the earned arm returned
  `Ok(SettlementOutcome { committed_now: true, … })`, i.e. the claim
  settled straight past the missing gate. Tooth bites.
- E2E leg 1 (`resolve_without_a_durable_pass_record_refuses_with_the_gate_
  the_cli_hits`): **stayed GREEN**. NOT a broken sensor: the composition
  root (`operator_action.rs`) calls the gate ITSELF before the earned
  arm, so the composed path still refuses without durable PASS. Defense
  in depth held. (Recorded inaccuracy: the builder's teeth note
  predicted leg 1 RED under this mutation; in fact leg 1 is the sensor
  for mutation 2, and the in-crate test is the sensor for mutation 1.
  The D11 requirement — the refusal machinery fails under a bypass — is
  satisfied by the pair together; each layer's tooth was demonstrated
  against its own layer's bypass.)

**Mutation 2 (composition side):** in `operator_action.rs`, removed the
composition-root gate call AND replaced
`settle_operator_action_earned` with a raw `service.settle(...)`
(bypassing both gate layers — the second-writer class).

- E2E leg 1: **RED** (`4 passed, 1 failed`) — the world's seeded
  evidence genuinely re-verified PASS, a durable settlement row appeared,
  and the refusal-parity/zero-rows assertions caught it. Tooth bites.

**Restores:** `git checkout -- <file>`; sha256 of both files re-hashed
and matched the baselines byte-identically (values above).

**GREEN re-run on the restored tree:** no-bypass suite 5/5; in-crate
earned-arm gate test 1/1 PASS.

## Conclusion

The AK-062 no-bypass posture is enforced at TWO independent layers
(composition root + service earned arm), and each layer's bypass is
caught by a dedicated, personally-proven RED sensor. The plan gate
battery is green on the committed tree.
