# Task 21 — builder B1 report (operator surfaces, feature-gated)

**Status:** complete. Every charter step-5 gate green with evidence taken
AFTER the last source edit (see Gate evidence). A builder claim of
completion carries no settlement authority; the orchestrator re-verifies.

## What landed

**New adapter crate — `crates/adapters/gauntlet-adapter-surface-json/`**
(D2; deps: `gauntlet-ports` + `gauntlet-domain` ONLY — verified by the
boundaries suite):
- `src/lib.rs` — §20.2-order JSON encoding of `RuntimeSnapshot` (escalated
  FIRST), `budget`/`units`/`unit` objects, the `{"error":{"code","message"}}`
  body, and the suite-facing `MemorySurfaceFactory` (P8 pattern).
- `src/http.rs` — std-only HTTP/1.1 over `TcpListener`: loopback-only
  bind (typed `BindRefused` before any socket), bounded request
  (16 KiB head / 1 MiB body caps), per-request deadline (socket timeouts
  AND the port deadline), §20.2 routes exactly
  (`GET /state`, `/runs/{id}`, `/units/{id}`, `/escalated`;
  `POST /refresh`, `/units/{id}/resolve|retry`), 405 + `Allow`, 404,
  typed 400s, 409 refusals, 500 infra failures. Serving is SERIAL by
  design (see Deviations — the port traits carry no Send/Sync; the
  per-request deadline bounds each connection; `serve_while` gives an
  ordered stop for tests without thread bounds on the seams).
- `src/action.rs` — the narrow action-handler seam
  (`ActionCommand{kind, unit, operator, note, run?}` →
  `Accepted/Refused/Failed`); NO settlement type crosses it.
- `src/views.rs` — `RunScopedReader` + `RunView`/`UnitView` (fields each
  trace to a named durable read).
- `src/memory.rs` — the echo source. `tests/contract_suite.rs` runs the
  UNMODIFIED `operator_surface_suite` (7/7 adapter tests green).
- `tests/http.rs` — routes/405/404/400/409/500 + field-order + loopback
  bind tests over real sockets.

**Service — `crates/gauntlet-app/src/settle/mod.rs`** (D4):
- `pub settle_operator_action_gate` — the AK-062 gate (durable PASS
  records → identity), extracted verbatim from the old body.
- `settle_operator_action` — behavior UNCHANGED (gate + the
  `OperatorActionRequiresEarnedReadiness` tail); the pre-existing
  in-crate test `an_operator_action_without_pass_evidence_hits_the_same_
  gate_as_the_loop` stays green untouched.
- `pub settle_operator_action_earned(request, claim, deadline)` — the
  positive arm: the gate runs FIRST inside the one method that pairs it
  with `settle`, then `SettleRequest{claim, actor: Operator{identity},
  tracked_residuals: &[]}` — the SAME settle, conditions 1–5 re-derived
  inside, §16.3 commit by the service. Three new in-crate tests:
  gate-refuses-even-with-a-chain-minted-PASS-claim (the D4 no-bypass
  tooth at service level), positive-arm settle with operator identity on
  the B.6 decision, identity-requirement-inside-the-gate.

**CLI — `crates/gauntlet-cli/`** (D1, D3, D5, D6, D7, D9):
- `src/operator_action.rs` — the ONE shared path used by BOTH surfaces:
  gate first (`settle_operator_action_gate` — refusal parity in the
  service's own words) → governed re-mint (`verify_cmd` pattern:
  frozen unit's `evidence_required`, latest durable iteration,
  `composed_ladder`, `VerificationAuthority::for_run`) →
  `settle_operator_action_earned` → ONLY on acceptance the queue
  ticket(s) resolve (fail-closed on queue-write failure). Plus the §16.3
  idempotent re-entry read (see Design decisions) and the D7 §20.3 log
  line for EVERY outcome (accepted/refused/failed; the note is never
  logged). Stable refusal-code vocabulary (one token per `SettleError`).
- `src/escalations_cmd.rs` — argument parsing preserved; execution
  replaced by the shared path; refusal output text unchanged
  ("the settlement service refused — the ticket stays open: …").
- `src/serve_cmd.rs` + `src/durable_surface.rs` (`#[cfg(feature =
  "surfaces")]`) — `gauntlet serve`: bind from `GAUNTLET_API_BIND`
  (typed fail-closed when unset/empty/unparsable; non-loopback
  refused); the durable `OperatorSurface` port impl + `RunScopedReader`
  + `SnapshotDisclosure` + the action handler over the shared path;
  serves until interrupted; a surface failure never touches run state.
- `src/lib.rs` dispatch/usage cfg-gated; `src/report_manifest.rs` gains
  the `serve` row only under the feature; `src/compose.rs`: `Clone` for
  the read-side `OperatorSurface`.
- `Cargo.toml` (crate + root): `[features] default = [], surfaces =
  ["dep:gauntlet-adapter-surface-json"]` — the ONLY feature edge; the
  adapter is an optional, non-default dependency.
- Tests: `tests/cli_surface.rs` feature-aware (default build asserts
  `serve` is an UNKNOWN command, exit 2; the frozen `cli_help.txt` is
  BYTE-IDENTICAL — `git diff` empty); `tests/cli_help_surfaces.txt` is
  the surfaces build's own frozen snapshot (generated from the real
  binary). `tests/escalations_cli.rs` test 3 updated to the Task-21
  posture (see Test evolution). `tests/operator_actions_no_bypass.rs`
  is the D10/D11 proof (below).

## The D10 proof (`operator_actions_no_bypass.rs`) and D11 teeth

Drives the REAL binary's `serve` over a real loopback socket against a
hand-seeded durable world: the fixture target project, a REAL frozen
reference (frozen + pinned through `FileReferenceProvider`), a recorded
run + iteration-1 checkpoint, the happy scenario's locator artifact
ingested AND pinned (condition 2) AND written at the canonical workspace
root (the §22.2 locator half), an evidence production bound through the
production authority, an optional durable PASS substrate, and one open
escalation ticket. Five legs:

1. **No durable PASS record** ⇒ 409, code `verification_not_pass`, the
   gate's exact message (asserted equal-family with the CLI's own stderr
   in the SAME world — refusal parity), ticket untouched (record
   compared), ZERO settlement rows. **D11 sensor:** the world is seeded
   so a bypassed gate would have COMMITTED — the pinned evidence
   genuinely re-verifies PASS through the chain, so deleting the gate
   call inside `settle_operator_action_earned` (the one-line
   revert-class mutation) produces a durable settlement row and turns
   this leg red. The service-level mirror of the same mutation is
   toothed in-crate (`the_earned_arm_refuses_past_the_gate_even_holding_
   a_chain_minted_pass_claim`).
2. **Positive leg** ⇒ 200 with `committed_now: true`, exactly ONE
   settlement row, the ticket resolved with the recorded operator
   identity; the idempotent re-POST (with the explicit `?run=` anchor)
   returns `committed_now: false` and commits NOTHING (row stays 1);
   the explicit-ticket CLI replay hits the queue's OWN typed refusal
   ("already resolved — a queue record resolves exactly once").
3. **Frozen-ruler refusal** ⇒ a PASS-recorded unit OUTSIDE the frozen
   key is refused typed (`unit_not_frozen`, 409), ticket open, zero rows
   (see Deviations for the authority-leg substitution).
4. **Shape** ⇒ 405 + `Allow: POST`; 404 unknown route; missing identity
   ⇒ typed 400 `operator_identity_required` in the §20.2 error body;
   the §20.2 reads serve the snapshot (escalated first, `run_derivation`
   disclosed), the escalated inbox, the run view, the unit view.
5. **Order/corruption** ⇒ a corrupt ticket file stays the queue's own
   typed refusal; nothing settles; the corrupt file is never rewritten.

## Design decisions (within charter discretion; none relax a gate)

- **Serial serving.** The port traits carry no Send/Sync, so the server
  serves connections serially on one thread with `serve_while(stop)` for
  an ordered stop. Loopback observability, bounded per request — the
  listener backlog is the only queue.
- **The §16.3 idempotent re-entry read** (shared path): when the unit's
  settlement row is ALREADY durable, the action returns
  `Accepted{committed_now: false}` after resolving only still-open
  tickets. Read-only (it never writes); it exists because a re-mint
  would otherwise trip the records-integrity refusal by minting a second
  document over the same (unit, evidence, outcome) identity. The
  alternative — surfacing `verification_record_disagrees` for a
  consistent replay — was rejected as alarming-but-uninformative; the
  row-count sensor (exactly once) is identical either way.
- **`?run=` on actions + optional `run` on `ActionCommand`**: extends D6's
  run-anchoring to actions so a replay can address its run after its
  ticket resolved. A recorded deviation from D3's exact field list.
- **API ticket mapping**: the action body carries no ticket (D3); the
  handler acts on the run's single OPEN ticket; 0 (un-anchored) → typed
  `no_open_ticket`; >1 → typed `ambiguous_ticket` (never a silent pick).
- **Run derivation (D6)**: distinct run ids of OPEN tickets; exactly one
  → anchor; several → greatest ULID (ULID order is time order);
  disclosed verbatim in `run_derivation` on every snapshot.
- **`sandbox_denials`**: queue tickets whose summary names "sandbox"
  (the runtime's own governance-gate escalation vocabulary); EMPTY when
  nothing names one. `open_residuals` is EMPTY — no durable read in the
  read surface names residuals (honest absence, no invention).

## Deviations from the charter (recorded, not smuggled)

1. **D10 leg 3 substituted.** The literal leg — "a unit whose frozen
   authority forbids operator closure" — is UNREACHABLE: `check_authority`
   (settle/mod.rs) permits `ClosingActor::Operator{identity: Some}` on
   EVERY `Authority` variant (`Critic`, `LeadAgent`, `Human`). The leg
   exercises the closest service-refused §16.1 family member reachable
   through the operator route: the FROZEN RULER's membership refusal
   (PASS record present, unit absent from the frozen key ⇒ the governed
   chain refuses, ticket open, zero rows). Fog for the orchestrator: if
   AK-062's intent requires an authority level the identified operator
   cannot close, that is a `CandidateRevision` against the spec, not a
   builder change.
2. **D3 input vocabulary + one field.** `ActionCommand` carries the
   optional `run` anchor (above).
3. **Serial HTTP serving** (no thread-per-connection) — a mechanism
   choice inside D2's "minimal HTTP/1.1 server"; nothing in the charter
   requires concurrency.
4. **Replay semantics split**: the API replay is the §16.3 idempotent
   re-entry (200, `committed_now: false`); the CLI replay with an
   explicit ticket surfaces the queue's own `AlreadyResolved` refusal.
   D10 leg 5's "replay stays the queue's own typed refusal" is pinned by
   the CLI replay; the charter's leg-2 "idempotent re-POST" is pinned by
   the API replay. Both settle exactly once.
5. **Test evolution (task-20 posture)**: `escalations_cli.rs` test 3 now
   pins the refusal's NEW locus — with a durable PASS record but a
   never-frozen reference, the shared path refuses at the GOVERNED OPEN
   — replacing the old always-err wording ("carries no chain-earned
   readiness"), which was Task 20's honest temporary posture. The
   no-settlement sensor (zero rows, ticket open) is unchanged and stays
   green; the test header records the evolution.

## Gate evidence (all post-last-source-edit; one compiler under
`flock .agents/reports/task21/compile.lock`; raw logs under
`.agents/reports/task21/finisher2-*.log`)

- Plan gate: `cargo nextest run -p gauntlet-cli --features surfaces
  --test operator_actions_no_bypass` — 6/6 runs "5 tests run: 5 passed";
  B1 re-ran it personally once more: 5/5.
- `cargo build -p gauntlet-cli --no-default-features` — exit 0 (D1).
- `cargo nextest run -p gauntlet-cli --test cli_surface --test
  escalations_cli` (default config) — 7 tests run: 7 passed.
- Mock E2E BOTH configs: `cargo nextest run -p gauntlet-cli --features
  surfaces --test e2e_full_loop_mock` → 12/12; same without the feature
  → 12/12.
- `just test-package gauntlet-app` — 477 tests run: 477 passed
  (includes the 3 new `the_earned_arm_*` tests and the untouched
  pre-existing AK-062 gate test).
- Workspace gates in order: `just check` exit 0 · `just test` →
  **1387 tests run: 1387 passed** · `just lint` exit 0 (clippy
  `-D warnings` + `cargo fmt --check`) · `just deny` exit 0 ·
  `just boundaries` → 228 tests run: 228 passed · `just ruler` — PASS,
  run TWICE (both PASS; B1 re-ran it personally once more: PASS).
- Frozen surfaces help snapshot: `crates/gauntlet-cli/tests/
  cli_help_surfaces.txt`; `git diff` on `tests/cli_help.txt` is EMPTY.

## D11 teeth notes for the orchestrator's personal re-verification

- Primary mutation: delete the `self.settle_operator_action_gate(&request,
  deadline)?;` line inside `settle_operator_action_earned`
  (crates/gauntlet-app/src/settle/mod.rs) → the no-bypass leg 1 goes RED
  (a durable settlement row appears — the world's pinned evidence
  genuinely re-verifies PASS) and the in-crate
  `the_earned_arm_refuses_past_the_gate_even_holding_a_chain_minted_pass_
  claim` goes RED with it.
- Secondary mutation: call `service.settle(...)` directly from
  `operator_action.rs` (skip the earned arm) → leg 1 goes RED on the
  refusal-code/message parity assertions even where the claim would not
  commit (the gate's wording vs the claim-outcome wording).
- The queue-write-only-after-acceptance sensor: every refusal leg
  asserts the ticket record byte-equal/`is_open` AND `settlement_count()
  == 0`; the corruption leg additionally asserts the corrupt ticket file
  is never rewritten.

## Honest unknowns / risks

1. **Untracked external writes during the gate window**: a `docs/` tree
   (a documentation site, ~36 markdown files) and
   `.agents/reports/doctor.json` appeared from OUTSIDE this slice while
   the finisher ran. Not produced by any task21 command; cannot affect
   the recorded gates; the ORCHESTRATOR must not stage them with the
   task21 commit and should confirm their provenance with the operator.
2. Leg 3's substitution (deviation 1) is the settlement interview's main
   judgment call; the D11 teeth do not depend on it.
3. The surface's `sandbox_denials` derivation is textual (summary
   contains "sandbox") because the durable queue record carries no
   structured class field; a structured escalation class would be a
   ports change (recorded as a candidate, not made).
4. D12 dogfooding remains blocked on `runners.default` (UNK-003,
   operator's decision) — recorded as fog in the charter itself.
5. The two finisher subagents (dispatched after two session
   interruptions, per the operator's instruction) applied: `cargo fmt
   --all` (whitespace only) after the lint gate exposed pre-existing
   formatting drift, and literals-scanner/clippy respellings in
   `serve_cmd.rs`, `operator_action.rs`, and the test file. B1 re-read
   the D10/D11-critical assertions after those edits (intact — see the
   teeth notes) and re-ran the decisive gates personally (plan gate 5/5,
   ruler PASS ×2).
