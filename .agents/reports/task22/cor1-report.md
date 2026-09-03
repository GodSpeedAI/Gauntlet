# Task 22 — COR-1 correction report (builder)

Date: 2026-09-03. Contract: `.agents/reports/task22/cor1-brief.md` (C1–C5,
BINDING). Acceptance targets: the falsifier's executed reproductions
(`falsifier-round1.md`: B2a, B3a/B3b, B4, B5). Process log with RED/GREEN
observations: `cor1-progress.md`. Tree: task22 work remains UNCOMMITTED;
COR-1 staged and committed nothing.

## Per-correction summary — all five corrections landed, each RED→GREEN proven

### C1 (F-2, BLOCKING) — windows decide on substrate chronology, never import order — LANDED

- Port: `ObservationRecord` gained the OPTIONAL substrate occurrence fact
  `occurred_at` (`with_occurred_at`, accessor), distinct from `at` (the
  recording instant, still excluded from every decision). Codec round-trips
  it (strict decode, unknown-field rejection unchanged); `facts_agree`
  includes it, so idempotent re-import stays a no-op.
- Import populates `occurred_at` on every Attempt/Failure observation from
  the verification record's `produced_at`, and on every Settlement
  observation from its MATCHING record's `produced_at`. Payment, correction,
  recovery rows stay unset (nothing invented).
- `PromotionService::evaluate` orders BOTH decided windows by
  `(occurred_at, ledger seq)` (stable sort; the read-back append order is
  the tiebreak) BEFORE any window logic. Any decided-window row lacking
  `occurred_at` makes that window UNDECIDABLE: typed refusal
  (`InsufficientOutcomeEvidence` for the outcome window;
  `InsufficientSettlementEvidence` for the settlement window), never a pass.
  Module docs state the rule.
- REGRESSION (B3b verbatim, shipped):
  `memory::developmental::tests::the_verdict_is_a_function_of_recorded_facts_never_import_order_b3b`
  — honest deteriorating substrate (r1–r3 PASS settlements over three units
  + newest r4 FAIL + one recovery), imported 4,3,2,1 vs 1,2,3,4: SAME
  verdict, `Refused([failure_rate_increased { earlier: (0,2), later: (1,2) }])`,
  Provisional, never Promoted, recorded halves occurrence-ordered.
  RED was observed first: the unfixed tree PANICKED "the newest-first import
  PROMOTED a genuinely deteriorating trajectory (B3b)" — the falsifier's
  executed reproduction exactly. Companions:
  `windows_decide_on_substrate_chronology_never_append_order` (promotion
  level), `an_outcome_window_missing_occurrence_facts_is_undecidable`
  (fail-closed), `occurrence_instants_are_substrate_facts_from_the_verification_records`
  (population + non-invention).

### C2 (F-1 MAJOR + I-2) — settlement windows refuse fact-sparseness; witnesses complete — LANDED

- New `PromotionRefusal::InsufficientSettlementEvidence { missing: Vec<String> }`
  (rule `insufficient_settlement_evidence`, in `ALL_RULES`, codec + Display).
  `evaluate` pre-validates the settlement window: EVERY settlement
  observation must carry unit, reference_digest, evidence_kind,
  evidence_locator, instrument determinism, orchestration_cost, verification
  ref, evidence ref (plus C1's occurred_at). Any sparse row → ONE typed
  refusal naming the missing classes (sorted, deduped); the window decides
  NO settlement condition — it never passes and never credits variation.
  The any-content-axis-differs variation rule is kept over the (now
  comparable) axes.
- `promotion_evidence`: every settlement observation yields a witness — the
  `filter_map` silent drops are gone; `settlements_observed ==
  witnesses.len()` asserted as a structural invariant in the full-path test.
- REGRESSIONS: `sparse_settlement_rows_never_promote_and_never_credit_variation_b2a`
  (B2a verbatim: 1 full + 2 sparse rows + attempts + recovery → refuses,
  naming the missing classes; RED first: the unfixed tree PROMOTED crediting
  variation to absence) and `all_sparse_identical_settlements_still_refuse_b2b`
  (B2b: still refuses — accepts `insufficient_variation` or the new
  evidence rule).

### C3 (F-3 MAJOR) — elevating transitions are log-rooted earned writes — LANDED

- Port obligation, documented on the `apply_transition` trait method and
  enforced by the shared suite on ALL legs: a `TransitionReason::Promoted`
  transition is refused `CapabilityLedgerError::UnrootedPromotion {
  capability, witness }` unless EVERY witness resolves, in the ledger's OWN
  observation log, to a Settlement observation of the same capability with
  matching (run, unit, verification, evidence) facts
  (`promotion_witness_is_rooted`, the canonical predicate in the ports
  crate). Pure provenance — nothing about it is §19.2 logic.
- Implemented in the reference green stub AND the sqlite adapter (inside
  `BEGIN IMMEDIATE`, after the stale check, before the insert — witnesses
  resolve against the same snapshot the history insert lands in) AND the
  app `MemoryLedger` test double (keeps the double at the pinned
  semantics). Suite grew one case, `elevating_transitions_are_log_rooted`,
  run on all legs: fabricated evidence naming an unrooted witness → typed
  refusal, register NOT elevated, no transition row, plus a positive control
  proving a rooted honest promotion still applies.
- The honest service path keeps promoting (its evidence is derived FROM the
  log); p10 leg 3 stayed green UNMODIFIED apart from C4's call-site argument.
- RECORDED (module docs + this closure note): log-rooting does not defend
  against a writer who forges the log itself — the developmental log is
  append-only and auditable by design; the sealed-mint alternative (an
  authority type with no public constructor for elevating reasons, house
  pattern `AdmittedSettlement`) is a recorded ports-revision candidate
  (same class as `BudgetMeter::mint`'s recorded candidate,
  `crates/gauntlet-ports/src/budget_meter.rs:124-132`). The composition
  root hands ledgers only to trusted code.
- REGRESSION (B4 verbatim, as the suite case): RED was observed on the
  unfixed tree on BOTH legs — green stub:
  `capability_ledger_green_is_clean` FAILED ("a fabricated promotion
  evidence APPLIED directly through the port — the forged status stands as
  if earned"); sqlite `sqlite_capability_ledger_passes_the_shared_contract_suite`
  FAILED with the forged promotion applied, register durably Promoted, 1
  transition row, and the honest rooted promotion then refused
  `AlreadyPromoted`.

### C4 (F-4 MAJOR) — recovery records are rooted to a durable run and a non-zero completion — LANDED

- `DevelopmentalImport::record_recovery` gained `completed: usize` (the
  value §18.3 `complete_pending_settlements` returned). Refusals, both
  appending nothing: `completed == 0` → typed
  `ImportError::RecoveryNotEarned { completed }`; `state.load_run(run_id)`
  `None` → typed `ImportError::RunNotDurable`.
- Call sites updated with the honest completion count (their flows complete
  1 row): both p10 legs pass the §18.3 `completed` value they assert == 1;
  module tests pass 1. New test `a_recovery_with_zero_completions_is_not_earned`.
- RECORDED (docs, same class as C3's boundary): the count is caller-
  supplied; the production caller is the recovery-completion call site;
  deeper sealing (a typed completion receipt minted by the store) is a
  recorded ports-revision candidate.
- REGRESSION (B5 verbatim):
  `a_recovery_for_a_run_that_exists_nowhere_is_refused_b5` (typed refusal,
  no row) and
  `recovery_credit_never_completes_the_conjunction_on_an_empty_substrate_b5`
  (full-minus-recovery window refuses `no_recovery_observed`; the mint
  attempt is refused; the evaluation STILL refuses `no_recovery_observed`,
  never Promoted). RED was observed first with the OLD signature: both tests
  failed because `record_recovery` returned Ok and minted the row — the
  falsifier's executed reproduction exactly.

### C5 (I-1) — teeth-declaration wording honesty (docs only) — LANDED

- p10 module doc and the leg-1/leg-2 test doc comments reworded to the true
  mechanism: removing the condition leaves the refusal to the remaining
  conditions (defense in depth is real); the named rule-presence assertion
  is the tooth that turns RED. No assertion changes.

## Files touched (sha256, 2026-09-03 post-gates)

| File | sha256 |
| --- | --- |
| `crates/gauntlet-ports/src/capability_ledger.rs` | `886e8842899aa26d08216ab087f0895ea359afa0148bca178ec243ec5bf09090` |
| `crates/gauntlet-ports/src/contract/capability_ledger.rs` | `4d1c4afec4922cb75135638826e756f74a2f92eecd0c9cf5ad6e22ec97ef0bcc` |
| `crates/gauntlet-ports/src/contract/stubs.rs` | `f06e904d05834bdfec96c7df83824796b9d84484ae9226dad33868fb5132d1cd` |
| `crates/adapters/gauntlet-adapter-state-sqlite/src/capability_ledger.rs` | `609707d72151d276d9e41d8578f64cb5f30dbaaf36d106a88d08554746e1ed3b` |
| `crates/gauntlet-app/src/memory/developmental.rs` | `71c0037d2358961c8099f6587a2fb3b8ed5525198caeb2f5950a3da10a69cf96` |
| `crates/gauntlet-app/src/memory/promotion.rs` | `d5db021ff18af52703d198a6e36c92b8f7caa86b2dddef8bf1ea9c1b6178cdbb` |
| `crates/gauntlet-app/src/memory/mod.rs` | `680356209fe42e9db237cc8489bd1cc9e0a463884f012c5500233b2b74ee84ae` |
| `crates/gauntlet-cli/tests/p10_no_false_learning.rs` | `9877c59dee21ea205affca2daa2ba9738c6b60d0494c0501a9d622a263edd019` |

(`memory/mod.rs` is the `#[cfg(test)]` testing double module — touched only to
keep the in-memory ledger at the port's now-rooted semantics; it is part of
the module-test surface the brief commissions.)

## Gates — verbatim tails (all under `flock /tmp/gauntlet-build.lock sh -c
'<cmd>; sccache --stop-server >/dev/null 2>&1; true'`)

Plan gates:

```text
=== GATE p10 (plan)
     Summary [   0.056s] 4 tests run: 4 passed, 0 skipped
=== GATE app memory (plan)
     Summary [   0.147s] 33 tests run: 33 passed, 480 skipped
```

Suite legs:

```text
=== GATE ports capability_ledger (suite legs)
     Summary [   0.007s] 3 tests run: 3 passed, 112 skipped
=== GATE sqlite contract_suite (suite legs)
     Summary [   0.100s] 3 tests run: 3 passed, 0 skipped
```

Boundaries, lint, ruler:

```text
=== GATE boundaries
     Summary [  42.505s] 228 tests run: 228 passed, 0 skipped
LINT GATE: PASS            (cargo clippy --workspace --all-targets -- -D warnings; cargo fmt --check)
verify-ruler.sh: PASS — answer key and manifest digests match the frozen lock
```

`just ruler`: PASS at start AND end. `just check-package` for gauntlet-ports,
gauntlet-app, gauntlet-adapter-state-sqlite, gauntlet-cli: all Finished.

## Recorded deviations and declared consequences

1. **`InsufficientSettlementEvidence` landed during C1's pipeline work**
   (C1's brief itself names the "InsufficientOutcomeEvidence /
   InsufficientSettlementEvidence class" for the settlement window); C2
   widened which facts feed it. No behavioral gap at any observed point —
   the C2 RED (B2a promotes) was demonstrated against the C1-fixed tree
   where sparse rows carried `occurred_at`, isolating C2's fact check.
2. **Two pre-existing module tests assert the subsumed refusal**
   (`an_undecidable_erosion_window_refuses`,
   `an_undecidable_burden_window_refuses` now expect
   `insufficient_settlement_evidence` naming `instrument_deterministic` /
   `orchestration_cost` instead of the per-condition variants). Justification:
   the brief's binding pre-validation list includes instrument determinism
   and orchestration_cost, so the per-condition undecidability arms are
   subsumed; the window still refuses fail-closed for the same fixture and
   the refusal is stronger (more facts demanded, missing classes named).
   The two variants remain in the refusal vocabulary (decode-compatible;
   the erosion/burden functions keep their fail-closed guards).
3. **Suite fixture adaptation (C3)**: the shared suite's
   `settlement_record` now carries its evidence ref and the
   `promotion_evidence` fixture is parameterized by the run it roots to, so
   the suite's own honest elevations satisfy the new port obligation. Case
   assertions are unchanged; a positive control was ADDED (never removed).
4. **`MemoryLedger` (app test double) implements the rooting check** —
   outside the brief's named file list but required to keep the module-test
   double at the semantics the shared suite now enforces on every leg.
5. **TDD note (C3/C4)**: C3's RED could not compile without the error
   variant, so the mechanical variant landed first and the RED was observed
   behaviorally (the stub/adapter applied the forgery). C4's RED was
   observed under the OLD signature (the mint returned Ok), then the
   signature change landed with the fix.

## Discipline attestation

Nothing staged or committed (`git diff --cached` empty). Settlement path
zero diff (`git diff HEAD --stat` over `crates/gauntlet-app/src/settle/`,
`producer_authority`, and state-sqlite settlement/event-log/store code is
empty). `.wayfinder/*` and `.gauntlet/reference.lock` never touched. No new
dependencies. No sub-agents. Every cargo invocation ran under the flock
build lock with `sccache --stop-server` inside the same command.
