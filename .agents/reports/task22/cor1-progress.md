# Task 22 — COR-1 correction progress (builder)

Binding contract: `cor1-brief.md` (C1–C5). Acceptance targets: the
falsifier's executed reproductions in `falsifier-round1.md` (B2a, B3a/B3b,
B4, B5). Process: regression test FIRST where practical, RED observed and
recorded, then fix, then GREEN.

## Baseline

- 2026-09-03 `just ruler` → PASS (verify-ruler.sh: answer key and manifest
  digests match the frozen lock). Run before any edit.
- Tree state: task22 work uncommitted in the working tree (orchestrator
  owns integration; nothing staged/committed by COR-1).
- Falsifier restoration baseline sha256s (pre-correction):
  - `crates/gauntlet-app/src/memory/promotion.rs` `befc6663…` (about to change — C1/C2 fix)
  - `crates/gauntlet-ports/src/capability_ledger.rs` `68fc78d4…` (about to change — C1/C2/C3)
- All cargo invocations under
  `flock /tmp/gauntlet-build.lock sh -c '<cmd>; sccache --stop-server >/dev/null 2>&1; true'`.

## C1 — windows decide on substrate chronology, never import order (F-2, BLOCKING)

Status: IN PROGRESS

- [ ] Port surface: `ObservationRecord.occurred_at` (optional substrate
  occurrence fact, distinct from `at`), codec round-trip, `facts_agree`
  includes it.
- [ ] RED tests recorded below.
- [ ] Fix: import populates occurred_at (attempt/failure from the
  verification record's `produced_at`; settlement from its MATCHING
  record's `produced_at`; payment/correction/recovery left unset);
  `evaluate` orders outcome + settlement windows by `(occurred_at, seq)`
  before any window logic; any decided-window row lacking occurred_at is
  a typed refusal, never a pass.
- [ ] GREEN evidence below.

### RED observations

- 2026-09-03, `cargo nextest run -p gauntlet-app memory:: b3b occurrence_instants
  windows_decide missing_occurrence` (under the build lock), against the tree with the
  port surface added but no behavior change:
  - `memory::developmental::tests::the_verdict_is_a_function_of_recorded_facts_never_import_order_b3b`
    FAILED — panicked `newest-first: import order decided the verdict — the newest-first
    import PROMOTED a genuinely deteriorating trajectory (B3b)`. Exactly the falsifier's
    executed B3b (append-order halves inverted a rising rate into a promotion).
  - `memory::promotion::tests::an_outcome_window_missing_occurrence_facts_is_undecidable`
    FAILED — panicked `a window without occurrence facts is undecidable: Promoted {…}`
    (the window decided with no chronology at all).
  - `memory::developmental::tests::occurrence_instants_are_substrate_facts_from_the_verification_records`
    FAILED — `occurred_at` was `None` on attempt/failure/settlement rows (not populated
    from the verification records' `produced_at`).
  - (`windows_decide_on_substrate_chronology_never_append_order` was filtered behind the
    fail-fast window; re-run with the full plan gate after the fix.)

### GREEN evidence

- 2026-09-03 `cargo nextest run -p gauntlet-app memory::` → **28/28 GREEN**
  (`Summary: 28 tests run: 28 passed`), including:
  - `memory::developmental::tests::the_verdict_is_a_function_of_recorded_facts_never_import_order_b3b`
    — newest-first (4,3,2,1) and in-order (1,2,3,4) imports of the same honest
    deteriorating substrate now produce the SAME verdict:
    `Refused([failure_rate_increased { earlier: (0,2), later: (1,2) }])`, Provisional,
    never Promoted; the recorded halves are occurrence-ordered.
  - `memory::promotion::tests::windows_decide_on_substrate_chronology_never_append_order`
    — window-level B3b: newest-fact-first append still refuses with occurrence-ordered
    halves.
  - `memory::promotion::tests::an_outcome_window_missing_occurrence_facts_is_undecidable`
    — refuses `insufficient_outcome_evidence`, never passes (fail-closed).
  - `memory::developmental::tests::occurrence_instants_are_substrate_facts_from_the_verification_records`
    — attempt/failure/settlement rows carry `occurred_at` from the record's
    `produced_at`; payment/correction rows stay `None` (nothing invented).
  - All 17 pre-existing module tests unchanged and green.

## C2 — settlement windows refuse fact-sparseness; witnesses complete (F-1+I-2)

Status: GREEN

- Port: `PromotionRefusal::InsufficientSettlementEvidence { missing: Vec<String> }`
  (landed with C1's pipeline work — C1's brief names the same variant class for the
  settlement window; C2 widens which facts feed it).
- RED: `sparse_settlement_rows_never_promote_and_never_credit_variation_b2a` FAILED —
  panicked `a window with sparse settlement rows PROMOTED — variation was credited to
  absence` (the falsifier's executed B2a, observed against the C1-fixed tree where the
  sparse rows carry `occurred_at`, isolating C2's fact check).
  `all_sparse_identical_settlements_still_refuse_b2b` passed pre-fix (a guard: B2b was
  already refused `insufficient_variation`; post-fix it refuses via the new evidence rule
  — the test accepts either).
- Fix: `settlement_window_facts` pre-validates EVERY settlement row for the full fact set
  (unit, reference_digest, evidence_kind, evidence_locator, instrument_deterministic,
  orchestration_cost, verification, evidence_ref, occurred_at) → ONE
  `InsufficientSettlementEvidence` refusal naming the missing classes (sorted, deduped);
  a sparse window decides no settlement condition (variation/erosion/burden are skipped —
  they never credit variation). `promotion_evidence` builds a witness for EVERY
  settlement row (no `filter_map`); `settlements_observed == witnesses.len()` asserted as
  a structural invariant in `the_full_satisfaction_path_is_the_only_road_to_promoted`.
- GREEN: `cargo nextest run -p gauntlet-app memory::` → **30/30 GREEN**.
- DECLARED CONSEQUENCE (recorded, not smuggled): `an_undecidable_erosion_window_refuses`
  and `an_undecidable_burden_window_refuses` now assert
  `insufficient_settlement_evidence` (naming `instrument_deterministic` /
  `orchestration_cost` among the missing classes) instead of the per-condition
  `insufficient_instrument_evidence` / `insufficient_burden_evidence`. Justification: the
  brief's binding pre-validation list includes instrument determinism and
  orchestration_cost, so those per-condition undecidability arms are subsumed — the
  window still refuses fail-closed for exactly the same fixture; the refusal is stronger
  (more facts demanded, missing classes named). The two variants remain in the refusal
  vocabulary (decode-compat; the erosion/burden functions keep their fail-closed guards).

## C3 — elevating transitions are log-rooted earned writes (F-3)

Status: PENDING

## C3 — elevating transitions are log-rooted earned writes (F-3)

Status: GREEN

- Port: `CapabilityLedgerError::UnrootedPromotion { capability, witness }` +
  `promotion_witness_is_rooted(log, capability, witness)` (the canonical predicate, in
  the ports crate so the reference stub, the sqlite adapter, and the app test double
  share one check); the obligation is documented on the `apply_transition` trait method
  including the RECORDED boundary: log-rooting does not defend against a writer who
  forges the log itself (append-only + auditable by design); the sealed-mint alternative
  (authority type with no public constructor for elevating reasons, `AdmittedSettlement`
  pattern) is a recorded ports-revision candidate (same class as `BudgetMeter::mint`'s
  recorded candidate in `crates/gauntlet-ports/src/budget_meter.rs:124-132`); the
  composition root hands ledgers only to trusted code.
- RED (2026-09-03, suite case `elevating_transitions_are_log_rooted` — the B4
  reproduction, verbatim, run on all legs against the unfixed stubs):
  - `gauntlet-ports contract::suite_runs::capability_ledger_green_is_clean` FAILED —
    the green stub APPLIED the fabricated Observed→Promoted transition.
  - `gauntlet-adapter-state-sqlite …sqlite_capability_ledger_passes_the_shared_contract_suite`
    FAILED with exactly the B4 findings: "a fabricated promotion evidence APPLIED
    directly through the port — the forged status stands as if earned"; "the register
    reads promoted after the refused fabrication"; "the refused fabrication left 1
    transition rows"; and the honest rooted promotion then refused `AlreadyPromoted`.
- Fix: rooting check in the green stub, the sqlite adapter (inside `BEGIN IMMEDIATE`,
  after the stale check, before the insert — witnesses resolve against the same snapshot
  the history insert lands in), and the `MemoryLedger` test double. Suite fixtures
  adapted so honest elevations are rooted (settlement rows carry their evidence ref;
  the suite's promotion-evidence fixture is parameterized by the run it roots to). The
  case includes a positive control: the same shape with a rooted witness applies.
- GREEN: ports `capability_ledger` legs 3/3; sqlite `contract_suite` 3/3;
  `gauntlet-app memory::` 30/30 (the honest service path — whose evidence is derived
  FROM the log — still promotes; p10 leg 3's road untouched).

## C4 — recovery records are rooted to a durable run and a non-zero completion (F-4)

Status: GREEN

- RED (2026-09-03, `cargo nextest run -p gauntlet-app memory:: exists_nowhere
  empty_substrate --no-fail-fast`, CURRENT signature, against the unfixed method):
  - `a_recovery_for_a_run_that_exists_nowhere_is_refused_b5` FAILED — panicked
    `a recovery for a non-durable run is refused: ()` — `record_recovery` returned Ok
    and minted the row for a run `load_run` cannot find (exactly the falsifier's B5).
  - `recovery_credit_never_completes_the_conjunction_on_an_empty_substrate_b5` FAILED —
    panicked `the mint is refused: ()` — the minted row completed the conjunction
    (the second evaluate would have promoted).
- Fix: `record_recovery` gains `completed: usize` (the value §18.3
  `complete_pending_settlements` returned) — `completed == 0` is the typed
  `ImportError::RecoveryNotEarned { completed }`; `state.load_run(run_id)` must be
  `Some`, else the typed `RunNotDurable` — both refuse BEFORE any append. Call sites
  updated with the honest count: the two p10 legs (the §18.3 `completed` value, asserted
  == 1) and the module tests. New test `a_recovery_with_zero_completions_is_not_earned`.
- RECORDED (docs, per brief): the count is caller-supplied; the production caller is the
  recovery-completion call site; deeper sealing (a typed completion receipt minted by
  the store, the house authority pattern) is a recorded ports-revision candidate.
- GREEN: `cargo nextest run -p gauntlet-app memory::` → **33/33**;
  `cargo nextest run --test p10_no_false_learning -p gauntlet-cli` → **4/4**.

## C5 — teeth-declaration wording honesty (docs only)

Status: GREEN (no assertions touched)

- p10 module doc + the leg-1 and leg-2 test doc comments reworded: removing ONE
  condition leaves the refusal to the remaining conditions (defense in depth is real —
  executed mutations A1/A2/A4 never produced a promotion); the named rule-presence
  assertion is the tooth that turns RED. What variation may never rest on (run ids,
  timestamps) stays absolute.

## Final gates (all run under the flock build lock; 2026-09-03)

- `just ruler` START: PASS. `just ruler` END: PASS (verbatim tail in cor1-report.md).
- `just check-package` gauntlet-ports / gauntlet-app / gauntlet-adapter-state-sqlite /
  gauntlet-cli: all Finished, zero warnings-as-errors.
- Plan gates: p10 4/4; `gauntlet-app memory::` 33/33.
- Suite legs: ports `capability_ledger` 3/3; sqlite `contract_suite` 3/3.
- `just boundaries`: 228/228.
- `just lint`: PASS (clippy -D warnings workspace-wide + `cargo fmt --check`; one
  formatting nit of COR-1's own was caught by the gate and fixed with `cargo fmt --all`,
  then the full fast-gate set re-run green).
- Discipline: nothing staged or committed (`git diff --cached` empty); settlement path
  zero diff (`git diff HEAD --stat` over settle/, producer_authority, state-sqlite
  settlement/event-log/store code empty); ruler files never touched; no new
  dependencies; no sub-agents.
