# Task 4 — Correction Round Progress

Builder session log. Started: 2026-08-16. Work is UNCOMMITTED in the working tree
(crates/gauntlet-ports/src/, crates/gauntlet-arch-tests/src/). Do NOT commit; do NOT
touch .wayfinder/, the spec, or the plan. Build lock held — one cargo command at a
time, sequential only, never parallel, never cargo clean.

Items (orchestrator-adjudicated residuals):
1. MAJOR — AgentRunner `probe_until_terminal` probe budget (critic A M1)
2. MAJOR — BudgetMeter reserve-refusal case (critic B R1)
3. MAJOR — strict deserialization malformed scripts + role_result.rs unit tests (critic B R2 + critic A m7)
4. MINOR — AgentRunner `capabilities()` getter case (critic A m9)
5. TRIVIAL — duplicate `export_is_byte_identical_and_faithful` case name + `RoleInvocationSpec.budget_slice` doc sentence (critic A m6)
6. ARCH-SUITE — do not edit; only record scanner misfires if observed while running `just boundaries`

Gates (in order, one at a time):
1. `cargo nextest run -p gauntlet-ports` (>51)
2. `cargo nextest run -p gauntlet-arch-tests` (190)
3. `cargo nextest run --workspace`
4. `just lint`
5. `just check && just deny`
6. `just ruler` (must stay PASS)

## Progress

### Final slice (2026-08-16, builder session 2 — PART A confirm + PART B doc/pin residuals)

PART A — confirmation with file:line:

1. MAJOR probe budget — LANDED, no change needed:
   - suite entry takes caller-supplied budget: `agent_runner_suite(factory, budget: ProbeBudget)` at
     `crates/gauntlet-ports/src/contract/agent_runner.rs:809-812`; `ProbeBudget { patience, max_probes }`
     at 47-55.
   - bounded increments: const `PROBE_RETRY_STEP: u64 = 15` at agent_runner.rs:37, used in
     `probe_until_terminal` (`std::thread::sleep` up to budget patience) at 169-218; probe ceiling
     const `SUITE_PROBE_CEILING: u32 = 24` at 42.
   - exhaustion yields named finding: agent_runner.rs:209-218 ("no terminal state within the probe
     budget (… probes, … ms of patience) for case …").
   - in-repo teeth runs pass `ProbeBudget::INSTANT`: `contract/suite_runs.rs:61,70,79`;
     `INSTANT` const at agent_runner.rs:61-64.
2. MAJOR BudgetMeter reserve refusal — LANDED, no change needed:
   - under-raw-ceiling probe: `reserve_consuming_the_recovery_reserve_is_refused` at
     `contract/budget_meter.rs:317-392` reserves `slice(801)` against `TOKEN_CEILING = 1000`
     (20% reserve ⇒ 800 forward ceiling), which is WITHIN every raw limit.
   - distinct class: demands `Err(BudgetMeterError::Reserve { cause: BudgetError::RecoveryReserveBreached })`
     and flags any plain `LimitExhausted` accounting: budget_meter.rs:361-382.
   - refusal books nothing: `meter.remaining() != before` check at budget_meter.rs:365-371.
3. MAJOR strict deserialization reach — LANDED, no change needed:
   - (a) unknown envelope field ⇒ `Failed{malformed_result}`: case `unknown_envelope_field_is_failed`
     at agent_runner.rs:512-523 (script `WriteUnknownEnvelopeField`; stub injects
     `"errors":[],"sneaky_extra":"surprise"` at contract/stubs.rs:283-290; strict decode at
     `role_result.rs:292` `reject_unknown_fields`).
   - (b) unknown field nested in `self_checks[0]`: case `unknown_nested_field_is_failed` at
     agent_runner.rs:529-540 (stub injects `self_checks:[{...,"sneaky_extra":"surprise"}]` at
     stubs.rs:291-298; `SelfCheck::from_json` reject at role_result.rs:743).
   - (c) role_result.rs unit tests asserting unknown-field rejection inside the four nested
     objects: `unknown_artifact_claim_fields_are_mechanically_rejected` (role_result.rs:1125),
     `unknown_self_check_fields_are_mechanically_rejected` (:1139), `unknown_unavailable_evidence_fields_are_mechanically_rejected`
     (:1153), `unknown_payment_fields_are_mechanically_rejected` (:1167); plus envelope (:1101) and
     payload (:1113); all assert `RoleResultError::UnknownField` with the field name and
     `failure_class() == MalformedResult`.
4. MINOR capabilities() case — LANDED, no change needed: `capabilities_are_stable_and_typed` at
   agent_runner.rs:748-800 (total infallible call, stable across two calls `first != second`,
   isolation ladder parse :778, result_delivery parse :789, verbatim echo in the RULE text :750);
   broken stub `FlippingCapsRunner` at stubs.rs:485-551 (isolation flips Process/Container per call),
   teeth test `agent_runner_caps_broken_is_caught` at suite_runs.rs:74-81.
5. TRIVIAL — budget_slice doc LANDED: `RoleInvocationSpec.budget_slice` field doc sentence at
   `crates/gauntlet-ports/src/agent_runner.rs:270-272`. Duplicate case name NOT landed → FIXED now:
   helper `append_events` at `contract/state_store.rs:91-124` still carried
   `const CASE: &str = "export_is_byte_identical_and_faithful"` duplicating the real case at
   state_store.rs:267-268. Renamed the helper's const to `"append_events_feed"` (state_store.rs:92).
   Tooth test unaffected: StateStoreBroken (FrozenStore) trips the export case itself (constant
   export after growth, state_store.rs:309-318), never the helper path.
6. cfg(test) gating — CONFIRMED: `contract/mod.rs:105-106` declares
   `#[cfg(test)] pub(crate) mod stubs;` and `mod.rs:108-109` declares `#[cfg(test)] mod suite_runs;`.
   Neither ships in the library binary. Suite section functions (`agent_runner_suite`,
   `budget_meter_suite`, …) are `pub fn` returning `Vec<ContractFinding>`, panic-free (findings
   instead of asserts), so adapter test targets can run them via `gauntlet_ports::contract::…`.

PART B — doc/pin residuals (all applied):

1. `crates/gauntlet-arch-tests/src/walk.rs` ANNEX_B_WIRE_KEYS doc, receiver-invisibility residual
   paragraph (≈140-153): extended so the `.get`-receiver sentence now also records the module-path
   callee form `helpers::get("command")` / `json::get("command")` — the position shape only checks
   the final callee identifier before the `(`. (walk.rs:144-148 after edit.)
2. Same doc: leading-space spelling `" command"` now recorded as masked at key-access too —
   segment literals are trimmed at both ends. The example list reads `("com\u{6d}and"`,
   `"command "`, `" command"`) (walk.rs:148-150 after edit). Pin: added
   `"object.get(\" command\");\n"` to the pinned fixtures in `annex_b_mask_corners_are_pinned_to_the_documented_residual`
   at `denylist.rs:1259-1277` with the comment noting both-end trimming; assertion semantics
   unchanged (still asserts no `'command'` violation).
3. `crates/gauntlet-arch-tests/src/literals.rs` module doc:
   - table row ≈19 (`hardcoded-command` rule + posture cells) rewritten to the current behavior:
     stopword exemption yields to command SHAPE (flag token or shell metachar); records the new
     known false-positive `"git push --force is dangerous"` (literals.rs:19 after edit).
   - residual paragraph ≈75-80 rewritten to match (stopword guard + F1 shape-yield), states the
     direction-of-error consistently: class errs toward flagging (literals.rs:75-81 after edit).
4. Same file, `find_timeout` doc: added the E4 false-positive shape paragraph — a count returned
   from a function whose OWN name carries a duration hint flags
   (`fn timeout_retry_limit() -> u64 { return 5; }` or `{ 5 }`); direction errs toward flagging;
   recorded, not silently widened (literals.rs:1394-1401 after edit).
5. `crates/gauntlet-arch-tests/src/envfile.rs` `malformed_entry_lines` doc: added the
   bare-`KEY`-as-empty-value future-loader sentence; suite's own parser requires `=`, so the
   fail-closed posture is internally consistent (envfile.rs:266-271 after edit).
6. `crates/gauntlet-ports/src/lib.rs`: added one doc line after the §7.3 bullet — suite code is
   test-support by design, adapter test targets consume it; asserts live in `#[cfg(test)]` fixtures
   (`stubs`, `suite_runs` gated, no stub in the library binary) (lib.rs:20-23 after edit). (PART A 6
   confirmed the gating already existed, so nothing needed gating.)

Gate results (in order):
1. `cargo nextest run -p gauntlet-ports` — PASS: 56/56 (0 failed, 0 skipped)
2. `cargo nextest run -p gauntlet-arch-tests` — PASS: 190/190 (0 failed, 0 skipped)
3. `cargo nextest run --workspace` — PASS: 322/322 (0 failed, 0 skipped)
4. `just lint` — PASS: clippy `-D warnings` clean + `cargo fmt --check` clean (exit=0)
5. `just check && just deny` — PASS: `cargo check --workspace --all-targets` clean; `advisories ok, bans ok, licenses ok, sources ok`
6. `just ruler` — PASS: verify-ruler.sh "PASS — answer key and manifest digests match the frozen lock"

Working tree state at close: uncommitted, as handed over. `crates/gauntlet-ports/src/` (new
port crate incl. `contract/` suites) untracked; `crates/gauntlet-arch-tests/src/{denylist,envfile,literals,text,walk}.rs`
and `crates/gauntlet-ports/src/lib.rs` modified. `.agents/reports/task4/` untracked (this
progress file). No commits made; .wayfinder/, spec, plan untouched.