# Task 13 / Builder B2 checkpoints (append-only, per-item)

Pre-start: `just ruler` PASS (2026-08-25). Read order: AGENTS.md, brief, recon-code §4–§5,
recon-spec §16/§18/B.6, residual-adjudication A1/A2/A4/A6/A7, builder-b1, precondition.rs,
controller.rs. Build lock held exclusively; no sub-agents.

## Item 1 — the `settle` module

- STARTED (2026-08-25): creating `crates/gauntlet-app/src/settle/mod.rs` — the ONLY settle
  path: SettlementService over (VerifiedClaim, consumed SettlementReadiness, frozen unit
  loaded from the durable run's own frozen projection, ClosingActor, tracked blocking
  residuals) enforcing §16.1 conditions 3/4/5 in order with typed refusals (class
  `settlement`), then record_verification → SettlementCommit → admit → commit_settlement,
  AlreadyCommitted-as-idempotent-success, durable offset observable for B4's commit-order
  proof. Documented mapping decisions (recorded in module docs, not silent):
  (a) B.6 authority_decision rides an accompanying pinned evidence record — the
  settlements schema (frozen, no new migration) carries no such column;
  (b) condition 4's durable half grounds on the durable checkpoint's unit-state
  projection (`blocked` = durably blocked, §5.3.3) — no typed durable residual-row read
  exists through any port today (residuals live inside comparison payloads); divergence
  durable-wins is the typed refusal cause; a residual-row read remains a ports revision.

## RECOVERED-AND-VERIFIED (orchestrator, 2026-08-26 08:4x EDT — 16th interruption instance)

The B2 builder died to the sub-agent usage limit PAST its last checkpoint
write (00:51:51). Reconciled per `.agents/lessons/unrecorded-landed-work.md`
(verify-and-record; never redo; never trust the record over the tree):

**What landed (unrecorded):**
- `crates/gauntlet-app/src/settle/mod.rs` (113,818 bytes, mtime 01:08,
  untracked) — the Item-1 SettlementService COMPLETE IN SHAPE per the
  STARTED design: `ClosingActor`/`ActorKind`/`OperatorIdentity`,
  `BlockingResidual`, `SettleRequest`, `AuthorityDecision`,
  `SettlementOutcome`, `OperatorActionRequest` (Item 5 surface),
  `ResumeCompletion` (Item 3 surface), `SettleError`, `SettlementService::
  {settle, settle_operator_action, complete_pending_after_checkpoint}`,
  module docs carrying the documented mappings, 16 in-module tests.
- `crates/gauntlet-app/src/lib.rs` (01:00) — `pub mod settle;` declared.
- `crates/gauntlet-app/src/control/controller.rs` (01:05) — doc + visibility
  only: `escape_json_text` → `pub(crate)` ("since Task 13 B2 … the settle
  service's recovery-limitation records"). The PASS-leg wiring (Item 2) has
  NOT started — no other controller change.

**Orchestrator verification (personal, build lock held):**
- `just ruler` PASS; `just check-package gauntlet-app` GREEN (3m10s, ONE
  dead-code warning: test helper `pinned_records_containing` never used —
  written for tests that don't call it yet, mid-flight artifact).
- `cargo nextest run -p gauntlet-app settle:: --no-fail-fast`:
  **4 passed / 12 failed**. ALL observed failures are FIXTURE-level, not
  service-logic: 10 panic at `earned_claim`'s
  `for_run(...).expect("the fixture run opens")` with
  `ReferenceRefused(ContractMismatch { frozen reference evidence digest
  sha256:c116… differs from ReferenceVersion sha256:2abc… })` — the
  fixture's frozen-reference evidence bytes disagree with the
  ReferenceVersion it constructed (Task 11's three-way digest equality
  working as designed, biting the fixture's own wiring); 2 more at fixture
  loci mod.rs:2623 and :2700 (same fixture family).

**Verdict:** Item 1 LANDED-PARTIAL — module complete in shape, its own
tests NEVER GREEN (the never-run-tests class); Items 2–5 UNSTARTED. The
dying builder's fail-first evidence for Item 1 is LOST (no DONE entry) and
must be re-derived by the finisher against the RED fixture, honestly
recorded as recovered, never implied.

**Resume path:** finisher dispatch per `finisher-brief-b2.md` (fix the
fixture's frozen-reference wiring to the honest chain; make the 16 tests
green fail-first-honestly; then Items 2–5 of `builder-brief-b2.md`
verbatim). Sub-agent delegation refused until 2026-08-26 16:23:55 EDT;
GAUNTLET_ALLOW_ORCHESTRATOR_IMPLEMENTATION unset — orchestrator does not
implement.

## Phase 1 (finisher, 2026-08-25) — make the inherited RED fixture honest

- STARTED (2026-08-25): pre-start `just ruler` PASS (re-run personally). Inherited RED
  re-derived personally: `cargo nextest run -p gauntlet-app settle:: --no-fail-fast`
  → 4 passed / 12 failed, all fixture-family (10 × `earned_claim` → `for_run`
  `ReferenceRefused(ContractMismatch …)`, 2 × `complete_pending_after_checkpoint`
  completion loci mod.rs:2623/:2700). Root cause: the fixture KEY ends `\n\n` while
  `FrozenReferenceProjection::load_version` digests the NORMALIZED text (trailing
  blank lines folded to exactly one `\n`, comparator.rs:1062-1080) — the
  ReferenceVersion carried `content_digest(KEY.as_bytes())`, so the three-way
  equality can never hold on these bytes. FIX = fixture only, following
  verify's own pattern (`verify/mod.rs` `ANSWER_KEY`/`answer_key_digest()`/
  `authority_parts()`): the frozen bytes become ALREADY-NORMALIZED (KEY's final
  `\n\n` → `\n`), so digest(actual frozen bytes) == digest(normalized) ==
  ReferenceVersion digest. The three-way equality is untouched. Also: use the
  dead `pinned_records_containing` helper (fixing it to check pins, matching its
  name) to STRENGTHEN the mutation test — both digests in ONE pinned abort
  record. NOTE: the original builder's fail-first evidence is LOST; the RED here
  is the recovered inherited RED, honestly labeled, never implied otherwise.
- DONE (2026-08-25): `cargo nextest run -p gauntlet-app settle:: --no-fail-fast` →
  **16 passed / 0 failed** (was the inherited 4/12). `just check-package gauntlet-app`
  GREEN with ZERO warnings (the dead `pinned_records_containing` helper is now USED —
  fixed to check pins, matching its name, and strengthening the mutation test: both
  digests asserted to ride ONE pinned abort record).
  Changes: (1) fixture KEY trailing `\n\n` → `\n` (frozen bytes now normal-form, the
  verify-fixture pattern — the three-way digest equality now genuinely HOLDS, never
  loosened); (2) ONE service fix, genuinely wrong vs its own documented taxonomy:
  `check_authority` now mints `OperatorIdentityRequired` for the operator route with
  absent identity (was generic `AuthorityNotPermitted`); fail-first evidence = the
  inherited-RED assertion at mod.rs:2151 (matched before my fixture fix as
  AuthorityNotPermitted, still RED after fixture fix alone, GREEN only after this
  change). Original builder's own fail-first evidence: LOST (recovered RED only).
  Now-green tests discriminate: full critic happy path (frozen-unit digest-verified
  load, record→admit→commit at one offset, advance last, B.6 pinned decision);
  rationale absent/blank/present trio; human-under-automatic refusal + escalation +
  operator-with-identity closure + anonymous-operator typed refusal; tracked residual
  for/against; durable-block divergence (durable wins); settle-time mutation (both
  digests, one pinned record) and unavailability (no abort, no escalation);
  idempotent re-entry (existing offset, no second event/row); disagreeing durable
  record; undurable run; anchored idempotent settlement replay; orphan evidence
  production (orphan offset, event-derived produced_at, derived digest);
  unrecoverable orphan (limitation recorded, nothing fabricated); AK-062 no-bypass.

## Item 2 (finisher, 2026-08-25) — controller PASS-leg wiring through the service

- STARTED (2026-08-25): replacing the in-memory-only settle (precondition asserted,
  readiness DISCARDED, `Unit::settle` + `unit_states.insert`, no durable event, and
  settle-anyway even when the precondition failed) with the service call in order
  verify → precondition(readiness) → service(conditions 3/4/5 → record → admit →
  commit) → in-memory advance; threading the settlement append into
  `state.last_offset` (C8.1). Plan: `RunController` gains a composed
  `SettlementService` field (house pattern — compose builds the authorities);
  SettleError mapping: ReferenceMutated → the run's terminal `aborted{reference_
  mutated}` path; State/Evidence/Escalation/Admission(State)/Reference → propagate
  typed ControlError; every §16.1 gate refusal → pinned forensic evidence, unit
  stays open (the precondition-failure pattern); precondition failure keeps its
  pinned-forensic behavior and now settles NOTHING. FakeState gains the honest
  in-memory settlement surface (harness extension, no assertion changes). New
  fail-first test: durable settlement row + event + PASS record + water-mark
  coverage through the COMPOSED loop (RED today: the loop writes none of them).
- DONE (2026-08-25): PASS leg = verify → loop PINS the chain-minted PASS evidence (the
  §16.1 condition-2 run-side act, p12's earning pattern — NEVER once performed before;
  the precondition had ALWAYS refused EvidenceUnpinned and the old code settled anyway)
  → precondition(readiness) → SettlementService (composed once in RunController::compose,
  house pattern) → in-memory advance; settlement append threaded into `state.last_offset`
  (C8.1). SettleError mapping: ReferenceMutated → terminal aborted{reference_mutated};
  State/Evidence/Escalation/Admission(State)/Reference → propagate typed; every §16.1
  gate refusal → pinned forensic evidence + unit stays open. Precondition failure keeps
  pinned-forensic behavior and now settles NOTHING. Condition-4 tracked view carries the
  BLOCKING subset only (§16.1's own word; §13.5 forces FAIL verdicts alongside blocking
  residuals, so blocking+PASS is the §18.4 disagreement shape).
  Fail-first: `the_pass_leg_settles_durably_through_the_service` RED under the restored
  old leg ("exactly one settlement row: [] — left: 0 right: 1", personally re-derived by
  temporarily restoring the old in-memory leg), GREEN under the service wiring; asserts
  row+event+PASS record+water-mark coverage through the COMPOSED loop.
  Harness adjustments (assertions unchanged unless named): FakeState gained the honest
  settlement surface + is_pinned + on_first_settlement lever; CountingState (p4) gained
  verbatim settlement-surface delegations (trait defaults fail closed);
  `reference_unavailability_is_a_recorded_gap_not_an_abort` trigger moved from first
  dispatch to first durable settlement — REQUIRED by §16.1 condition 5 + §21.2 ("uncertainty
  is never converted into success"): the old shape settled a unit whose condition-5 fact
  was unanswerable; every assertion preserved, proof stronger (settlement provably durable
  before the outage). p4's "is not pinned to the settling run" assertion — which named its
  own expiry ("the commit is Task 13's boundary") — replaced by the durable-settlement
  assertions (row + event offset + pinned authority_decision); recorded as a DEVIATION.
  KNOWN TRANSITION STATE: `a_file_only_unit_is_never_dispatched_the_frozen_reference_
  bounds_membership` is RED pending Item 4 — its shape (mid-run file edit + permanent
  outage + all units settle + terminal Settled) is unconstructible under condition 5
  (any post-edit settle consult answers either Unavailable → refuse, or Mutated → abort);
  Item 4's §21.3 stop-dispatch disposition completes it. control:: 75/76, settle:: 16/16,
  p4/p6/p12/p2 31/31, package 319/320.

## Item 3 (finisher, 2026-08-25) — resume completion (§18.3 step 1)

- STARTED (2026-08-25): `resume_run` gains, after `state.resume` and BEFORE
  re-planning: the service's `complete_pending_after_checkpoint(run_id,
  anchor = checkpoint.event_log_offset)` (anchored settlement replay +
  evidence-production orphan completion from durable facts; limitations pinned
  by the service), then (c) reconciliation — the in-memory projection is
  rebuilt against the durable settlement rows (a unit durable-settled but
  projected Open becomes Settled, §5.3.3). SettleError mapping gains a typed
  `ControlError::Settlement` variant for completion-path refusals. Harness:
  FakeState implements `complete_pending_settlements` (anchored keyed replay).
  Fail-first test: inject the §16.3 crash window (settlement event durable,
  row + projection behind) for the owed unit of a crashed dependency run;
  resume with ZERO dispatch budget must complete the row, reconcile the
  projection, and never re-dispatch.
- DONE (2026-08-25): `resume_run` now runs, after `state.resume` and before
  re-planning: the service's `complete_pending_after_checkpoint(run_id, anchor =
  checkpoint.event_log_offset)` (anchored keyed settlement replay + evidence-production
  orphan completion from durable facts; SettleError mapping: State/Evidence/Escalation
  propagate their ControlError loci; new typed `ControlError::Settlement(SettleError)`
  variant + Display + From for the rest), then §5.3.3 projection reconciliation against
  the durable settlement rows — EVENT-ORDERED: only a settlement whose event sits
  STRICTLY BEYOND the checkpoint's water-mark overrides the projection (the §16.3 crash
  window); a settlement at/below the anchor was already inside the checkpoint's
  knowledge and a later Open projection stands (a §11.5 residual retry's legitimate
  re-open, pinned by `resume_restores_per_unit_attempts_from_durable_productions`,
  which first failed under the blanket rule — first=2/second=0/Settled — and is green
  under the ordered rule).
  Fail-first: `a_resume_completes_half_applied_settlements_and_reconciles_the_
  projection` — injected §16.3 crash window (settlement event durable at/after the
  anchor, row + projection behind) for the owed unit of a crashed dependency run;
  RED before wiring ("the durably settled unit is never re-dispatched: [Attempt unit
  AK-9102…]"), GREEN after: resume with ZERO dispatch budget ends Settled, row
  completed, projection reconciled, no prompt. Harness: FakeState implements
  `complete_pending_settlements` (anchored keyed replay). Package 320/321 (the known
  Item-4 interlock remains).

## Item 4 (finisher, 2026-08-25) — §21.3 non-integrity mid-run dispositions (A6)

- STARTED (2026-08-25): leg 1 — answer-key READ failure (strict_units' Unavailable
  class) becomes §21.3 row 1 non-terminal degradation: pinned evidence, stop new
  dispatch for the iteration, keep the process alive and reconciliation running,
  bounded by rounds (BudgetExhausted), resume-able; the drain's step-3 reread skips
  cancellation for the pass instead of erroring. leg 2 — runner poll transport error
  routes through §11.5 row 2 (TransientError → RetryAfterBackoff: Failed terminal
  event + release + pinned transient record + backoff sleep + re-dispatch queue;
  per-unit transient-retry ledger in LoopState for the exponential sequence) instead
  of `Err(ControlError::Agent)`. `a_crash_after_a_settlement_never_re_runs_settled_
  work` re-pins the DISPOSITION (repeated transient errors → bounded rounds terminal
  with close-out) while preserving its resume-recovery assertion (settled unit never
  re-prompted; owed unit re-planned and completed). The Item-2 interlock
  (`a_file_only_unit…`) resolves here: under §16.1 condition 5 + §21.2 its old
  Settled outcome is unconstructible (demonstrated from spec text in the Item-2
  entry); re-pinned to the §21.3-honest outcome (BudgetExhausted, guard exercised
  across every post-edit plan, outage + refusal records pinned, pre-outage
  settlement durable) — assertion change recorded as a DEVIATION. New fail-first
  tests: read-failure degradation (file vanishes mid-run → no error, gap pinned,
  bounded terminal; file restored → resume completes) and the transient disposition.
- DONE (2026-08-25): leg 1 — answer-key READ failures (strict_units' Unavailable class)
  degrade at every mid-loop read site: the plan boundary (stop new dispatch for the
  iteration, pinned `answer_key_read_failed` record, reference_breach gap set, bounded
  by rounds), the verify leg (no unit verified/settled under the gap — I8), the §15.1
  selection record (skipped with a pinned gap), and the drain's step-3 reread (skips
  cancellation for the pass). Parse failures still propagate (not read failures).
  leg 2 — poll transport errors route through §11.5 row 2 via `dispose_transient`
  (Failed terminal event + pinned `transient_runner_error` record + virtual-time
  exponential backoff + row-2 re-dispatch), mirroring `dispose_failure`'s landed
  transient shape (flight.attempt feeds the sequence; no second ledger convention).
  Fail-first: `an_answer_key_read_failure_degrades_the_iteration_and_stays_resumeable`
  RED — the run ERRORED with `Reference(Unavailable{reading the answer key … No such
  file or directory})` before the full degradation (the record_selection site was the
  first escape observed); GREEN — Path-source key vanishes after the first durable
  settlement → BudgetExhausted with pinned gaps and the pre-failure settlement standing;
  file restored → resume (same Path source) completes AK-9102 and never re-dispatches
  AK-9101.
  Test re-shapes (deviations, each with its reason recorded in-test): the crash test
  now pins the DISPOSITION (repeated PollErrors → pinned transient records + row-2
  re-dispatches + bounded BudgetExhausted WITH close-out) and keeps its resume
  assertions verbatim (settled unit prompted once, owed unit re-planned → Settled);
  `resume_restores_per_unit_attempts…` rides a never-passing verifier (a PASS now
  settles durably — the old ladder shape is unconstructible) and a 4-round bounded
  first run, asserting the SAME 3-prompt/Escalated outcome with attempts restored
  from durable productions; `a_file_only_unit…` (the Item-2 interlock) re-pinned to
  the §21.3-honest outcome — BudgetExhausted (was Settled), outage opening moved to
  the first durable settlement, guard exercised across every post-edit plan (AK-9103
  never prompted), pre-outage settlement durable, boundary gaps + settle-refusals
  pinned. `a_resume_completes_half_applied…` trigger adapted to a 2-round bounded
  first run (PollError no longer ends runs). Package 322/322; p4/p6/p12/p2 31/31.

## Item 5 (finisher, 2026-08-25) — AK-062 service-level no-bypass (the half Task 13 owns)

- STARTED (2026-08-25): the service surface (`settle_operator_action`) and the
  no-bypass test landed with Item 1 and are GREEN since Phase 1
  (`an_operator_action_without_pass_evidence_hits_the_same_gate_as_the_loop`:
  no durable PASS → `VerificationNotPass`; durable PASS but no chain-earned
  readiness → `OperatorActionRequiresEarnedReadiness`; nothing settles). Item 5
  verifies that proof is real and adds the strengthening legs: (a) the SHARED-VARIANT
  mapping is pinned (`From<SettlementAdmissionError>` maps the admission's
  non-PASS refusal onto the SAME `SettleError::VerificationNotPass` — the mechanism
  behind "the same gate, whichever caller hit it"); (b) a run whose records hold
  FAIL and INCONCLUSIVE outcomes refuses naming BOTH — INCONCLUSIVE is never
  collapsed into FAIL through the gate (AK-005's three-state rule at the operator
  seam). Pure test additions — NO behavior change, so no fail-first applies (the
  brief's fail-first rule is per behavioral change); recorded as such, never implied.
- DONE (2026-08-25): `operator_action_refusals_name_the_three_states_and_the_shared_
  states_variant` added (name in tree: `operator_action_refusals_name_the_three_
  states_and_the_shared_gate_variant`): pins (a) the shared-variant mapping
  (`SettlementAdmissionError::VerificationNotPass` → `SettleError::VerificationNotPass`
  — one variant, both callers) and (b) the three-state refusal wording — FAIL and
  INCONCLUSIVE records on the queried unit refuse naming BOTH ("hold outcomes FAIL,
  INCONCLUSIVE — none is PASS"), INCONCLUSIVE never collapsed into FAIL. Two
  assertion-shape corrections during authoring (per-unit detail; uppercase outcome
  tokens) — test-only, no production change. `cargo nextest run -p gauntlet-app
  settle::` 17/17. No CLI surface built (Task 21's).
