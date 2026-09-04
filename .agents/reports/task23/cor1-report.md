# Task 23 — COR-1 report (correction round 1)

Builder: COR-1. Work order: `cor1-brief.md` (items C1–C11 + records-not-code),
amended live by the coordinator's C2 decision (2026-09-04, recorded in
IN-FLIGHT.md). Every code item is RED-first: the test that failed for exactly
the found reason ran RED before the fix, then GREEN after. All acceptance
gates green in my own runs (numbers at the end). Ruler files never touched;
nothing staged or committed; no new dependencies.

**Headline:** the §19.4 verdict contract is restored. The `u64::MAX` sentinel
is gone; an incomplete/undecidable arm is a typed evidence gap ⇒ Provisional
(never promoted, never rejected), verified empirically in both of the critic's
probe directions; the p9 proof no longer rests on the defect — legs 2/3 now
promote on genuine complete-arm figures at the gate level per the recorded
composition deviation, and leg 1 proves AK-056's containment over the real
loop with both arms complete.

---

## C1 (blocking) — Incomplete arms are evidence gaps, never sentinels

- **RED** (4 tests, all failed pre-fix for the found reasons):
  - `an_incomplete_control_arm_never_promotes_and_the_gap_is_named` —
    reproduced critic probe 1: the gate PROMOTED on the sentinel baseline.
  - `two_incomplete_arms_adjudicate_provisional_and_nothing_is_rejected` —
    reproduced critic probe 2: pre-fix failure was
    `Rejected{MarginNotMet control 18446744073709551615 vs treatment
    18446744073709551615}`.
  - `an_arm_that_settled_nothing_makes_the_margin_undecidable_not_failed` —
    the orchestrator's (0,0)-fallback finding: pre-fix failure was
    `Rejected{MarginNotMet control 0 vs treatment 0}`.
  - `a_reduced_attempt_ceiling_cannot_promote_through_the_completion_gate`
    (also C8's pin) — pre-fix `Rejected{control 30000 vs treatment
    18446744073709551615}`.
- **Fix:** `TargetMetric::figure` (`crates/gauntlet-app/src/promote/
  metrics.rs`) returns `None` for a non-`settled_terminal` arm — the
  sentinel value no longer exists anywhere. `gate.rs` conditions 7/8 now
  evaluate the margin/payment comparisons ONLY when both figures derive;
  otherwise they push `GateRefusal::EvaluationIncomplete` naming the
  underivable arm(s) (new `underivable_arms` helper) — the (0,0)
  placeholder path is deleted and an underivable figure can never compose
  a FAILED condition. Gate/module docs updated.
- **GREEN:** promote:: 34/34 (30 original + 4 new), then 40/40 after
  C4–C7 additions.

## C2 — p9 rebuilt (per the coordinator's decision; deviation recorded)

The brief's prescribed mechanism was **empirically impossible**: a probe
over the real binary with baseline `budget.max_parallel: 1` (complete) vs a
candidate delta raising it to `2` gave `control 10000 vs treatment 10000` —
the COR-D2 write-once per-(run, iteration) observation binding pins one
settlement per iteration, so serial dispatch pipelines perfectly at the cap
and parallelism ties. No inert (or any) key can make two complete arms'
figures differ; every cost lever on this loop is verification-bearing.
Coordinator decision: keep C1 strict; land C2 in three parts.

1. **Leg 1 (AK-056), real loop, containment form.** Economics probe first
   (review.mode over complete arms): NO genuine metric movement (10000 vs
   10000). So leg 1 runs the pure weakening delta (`review.mode: adaptive`)
   over the real composed loop with both arms COMPLETE and asserts the real
   gate REJECTS with `settlement_access_weakened` durably recorded, the
   rollback named, non-zero exit, and write-once stickiness across
   processes. Header states the containment form honestly (the candidate
   DECLARES the improvement; the metric does not move — recorded finding).
   Renamed `the_verification_weakening_candidate_is_rejected_over_the_real_loop`.
2. **Legs 2/3, gate level.** REAL `EvaluationHarness`, REAL embedded sqlite
   revision+capability+state stores (one file), REAL `PromotionGate`, REAL
   evidence store; the executor seam authors each arm through the REAL
   StateStore EARNED authorities (p10 pattern: `record_run` with
   `parent_run` on the treatment runs — C4's verification genuinely
   exercised — per-iteration boundary checkpoints, `VerificationRecord-
   Authority` mints, `SettlementCommitAuthority` admits + `commit_settlement`,
   closing checkpoint carrying the arm's `budget_spent`). The producer
   authority forced honest iteration ordering (first attempt failed with
   `IterationUnsupported` — the anchor rule works; fixed by earning each
   unit under its own boundary checkpoint). Improvement carrier: the INERT
   `runners.default` delta to a runner with a genuinely cheaper authored
   per-invocation economy; declared metric `payment_per_settlement`.
   Leg 2 asserts `Promoted` with `control_figure == 30_000_000 >
   treatment_figure == 20_000_000` (both derived from durable checkpoints),
   the durable `Promoted` decision record, and not-rolled-back. Leg 3
   rolls back through the REAL `RollbackService`, reopens the ledger (a
   fresh `EmbeddedRevisionLedger::open` over the same file), asserts the
   restore fact survives (`rolled_back=true`), the second restore refuses
   (`NothingToRestore`), and re-adjudication is `AlreadyFinal`.
3. **Records:** the deviation section below; dated entry appended to
   `.agents/reports/reference-candidates.md` (entry only — ruler files
   untouched, verified via git status).

Leg 4 (real loop, §19.4 floor) stays; its tail was trimmed to the floor
claim (provisional writes nothing; the pending revision can still be
evaluated) because the old tail's post-evaluation promotion on the real
loop would now honestly REFUSE on the tied figures.

## C3 — Admission idempotency off the clock

- **RED:** `admission_is_idempotent_across_a_stepping_clock` FAILED with
  `Ledger(ProposalConflict{rev-91990cd595615d54})` on the identical
  replayed document under a stepping clock; the shared contract suite's
  green leg FAILED on the new different-instant replay case ("already
  admitted with different declared facts" — for identical facts). (Also
  reproduced live against the stale binary before any fix: a re-propose of
  the same document errored with the false conflict.)
- **Fix:** `admitted_at` removed from `RevisionProposal::facts_agree`
  (ports); doc rewritten — the instant is admission provenance, not a
  declared fact. Contract-suite idempotency case extended to pin the
  different-instant replay (runs against the in-memory reference AND the
  sqlite adapter). The conflict message is now always true (it only fires
  on real fact differences).
- **GREEN:** ports revision_ledger 3/3; app admission tests 3/3.

## C4 — The isolated-evaluation fact is verified and durable

- **Fix surface first** (so the tests compile): `EvaluationRecord` gains
  `held_out_treatment_parent` + `regression_treatment_parent` (port struct,
  codec, `facts_agree`, suite fixture, harness record; the harness records
  the verified fact).
- **RED:** `an_unverifiable_treatment_isolation_refuses_to_record` FAILED
  (the harness recorded a full EvaluationSummary though the treatment run's
  durable record carried NO parent);
  `a_mismatched_treatment_parent_refuses_to_record` FAILED (recorded despite
  parent `run(7)` vs declared `run(2)`).
- **Fix:** the harness's `run_arm` reads the treatment run's DURABLE record
  through the outcome's own state handle and requires
  `load_run(run).parent_run == declared parent` BEFORE anything is
  appended; missing/mismatched ⇒ typed `EvaluationError::
  IsolationUnverifiable`, nothing recorded (the gate's MissingEvaluation
  floor then rests at Provisional, never a promotion).
- **GREEN pin:** `the_verified_parent_fact_is_durable_on_the_record` — the
  record carries both verified parent facts, on the returned summary and
  durably in the ledger.

## C5 — Condition 4 must not fail open on a store read error

- **RED:** new CLI integration test
  `improve_promote_gates::an_evidence_store_read_failure_aborts_promote_and_writes_nothing`
  (real binary, real store; the rollback object aliased to a directory —
  the store's documented `ReadFailed` shape) FAILED pre-fix with EMPTY
  stderr: the `.ok()` swallowed the error and the command adjudicated a
  verdict instead of aborting.
- **Fix:** `improve promote` aborts typed before adjudication on a rollback
  read error (nothing written; `Ok(true)` holds / `Ok(false)` fails with
  `RollbackInvalid`); the proposal-read `_ => None` swallow is gone too
  (typed abort). Test also proves nothing was written: after repair, the
  same revision adjudicates normally (PROVISIONAL, no AlreadyFinal).
- **GREEN:** 1/1.

## C6 — An infrastructure failure is a typed abort, never a negative verdict

- **RED:** `a_capability_ledger_read_failure_aborts_the_gate_and_writes_nothing`
  FAILED with `Rejected{ProtectedCapabilityInferior "the capability ledger
  read failed"}` — a transient read failure durably rejecting.
- **Fix:** `PromotionGate::protected_verdict` maps ledger read errors to
  `GateError::CapabilityLedger` (typed, retryable abort; nothing written);
  new `GateOutcome` enum separates condition Holds/Refused from aborts.
- **GREEN:** included in promote:: 40/40; the refusal-on-merits path
  (`an_inferior_protected_capability_is_rejected`) still green.

## C7 — No silently unapplied delta entries

- **RED:** `an_unappliable_delta_entry_refuses_evaluation_before_any_arm_runs`
  FAILED — the harness ran all four arms and recorded the evaluation even
  though the composition declared `runners.default` unappliable.
- **Fix (fail-closed mechanism):** `EvaluationExecutor` gains
  `validate_delta`; the harness calls it after the proposal check and
  BEFORE any arm runs — a refusal is typed `DeltaUnappliable` naming every
  unappliable entry, no run charged, nothing recorded. The CLI's
  `ComposedExecutor` implements it over the Annex E mapping (collecting ALL
  unappliable keys); the per-entry check inside `execute` remains as
  defense in depth.
- **GREEN:** included in promote:: 40/40.

## C8 — `max_attempts_per_unit` Inert justification re-derived on C1's ground

The regression test landed with C1
(`a_reduced_attempt_ceiling_cannot_promote_through_the_completion_gate`:
reduced attempts ⇒ arm cannot reach settled terminal ⇒ figures underivable
⇒ Provisional, never promoted, never rejected; condition 3 still holds).
The delta.rs comment was rewritten to that argument during the round
(applied by the coordinator's session at 10:32; verified present and
correct, referencing the pinning test).

## C9 — Dead `ReasonMismatch` contract removed

`DecisionRecordError::ReasonMismatch` and the always-true `carried`
computation removed from the port; docs now name the ledger's
`StaleDecision` as the enforced stale-claim gate. The suite leg's
subsequently-unreachable catch-all removed (was generating a warning).
Included in ports suite 3/3 + workspace check clean.

## C10 — Condition-1 docs state the superset truth

gate.rs condition-1 doc rewritten to the implemented conservative rule
(treatment's dispatched set CONTAINS every control unit; superset passes;
skipping is cherry-picking). mod.rs carries no contrary claim (verified).
Landed by the coordinator's session mid-round; verified.

## C11 — Domain-record deviation recorded

Appended a "COR-1" section to `b1-report.md`: the domain
`CandidateRevision` record is unused; the floors are re-derived in the
port/gate because D2's append-only, transition-derived design genuinely
conflicts with the domain record's mutable `decision` field; closure /
ruler-owner to retire or re-express. No code change.

---

## C2 recorded deviation + the loop-economics finding

**Deviation from charter D7 (all-legs-real-loop):** p9 legs 2/3 are
gate-level compositions, per the coordinator's 2026-09-04 decision. Their
assertions run through the REAL `PromotionGate::adjudicate`, the REAL
`EvaluationHarness`, and REAL embedded sqlite ledgers/stores; the only
proof-authored element is the executor seam's ECONOMY (per-invocation
payment), authored through the same real earning authorities p10 uses
(checkpoints with `budget_spent`, authority-minted verification records,
authority-admitted + committed settlements — never hand-built rows), plus
the runner manifest the `runners.default` delta selects from.

**The finding (why):** on the current loop, NO inert key can improve
per-settlement economics between two complete arms. Empirical probe (real
binary, real composed loop, scratch worlds since removed): baseline
`max_parallel: 1` vs candidate `max_parallel: 2`, both complete →
`control 10000 vs treatment 10000`; the four runs' durable checkpoints were
byte-identical (3 iterations / 3 rounds / zero payment each). Root cause:
the COR-D2 observation binding is write-once per (run, iteration) and
carried completions absorb one per iteration — N units settle in exactly N
iterations at ANY parallelism (serial pipelines perfectly at that cap).
Every other inert key is a ceiling that only binds (⇒ INCOMPLETE arm ⇒
Provisional under C1); the loop's cost levers are all verification-bearing
(correctly refused by condition 3). Pointer for the ruler-owner:
`reference-candidates.md` 2026-08-31 binding-granularity entry (~line 1358)
and the new 2026-09-04 COR-1 entry — the loop-level promotion demonstration
awaits the binding re-key (candidate revision (a)).

## Records, not code (i1–i4, for task24/27 and the next transport/composition touch)

- **i1 re-evaluation cherry-picking:** the gate adjudicates
  `evaluations.last()` (gate.rs ~196); an operator may re-run
  `improve evaluate` until a favorable record lands, then promote. The
  charter does not forbid re-evaluation. Anti-gaming task 24 candidate.
- **i2 governance target prefix heuristic:** `target_is_governance_bearing`
  is a prefix match over an open vocabulary (gate.rs ~547); adversarial
  target naming could evade it. Real containment lives in the delta
  classification and the ledger; the target check is advisory. Task 24.
- **i3 optimizer dispatch budget slice:** `dispatch_proposal` hands the
  optimizer invocation the whole configured run budget including the
  recovery-reserve percentage (improve_cmd.rs ~288); bounded in practice by
  the invocation deadline and the 64-probe cap, not by a typed slice.
  Task 27 / next composition touch.
- **i4 AK-057 locator realism:** the behavioral `candidate_only` leg runs
  over the in-memory `MemoryRevisions` double (which returns empty
  histories from `evaluations()`/`decisions()`); the real sqlite adapter is
  pinned by the contract suite and now also by p9 legs 2/3's real ledgers.
  The locator divergence remains recorded in reference-candidates.md.

## Operational notes (honest)

- Two self-nested-flock incidents: one deadlocked ~19 min (coordinator
  diagnosed and unwedged, killing the inner waiter), one I caught and
  killed before wedging. Rule now followed: ONE top-level flock per shell
  command, never nested inside another flock's `-c` string.
- The coordinator's session applied the C8 (delta.rs) and C10 (gate.rs)
  doc rewrites mid-round (10:32); I verified both against the implemented
  behavior rather than re-editing.
- Leg-1 test renamed to
  `the_verification_weakening_candidate_is_rejected_over_the_real_loop`
  (the old name claimed a metric improvement the loop cannot demonstrate —
  the header and the deviation record say which shape is proven).
- The legs-2/3 fixture initially tripped the producer authority's
  `IterationUnsupported` refusal (records minted at iteration 3 over a
  checkpoint declaring 1) — fixed by earning each unit under its own
  per-iteration boundary checkpoint, which is the honest order.

## Acceptance gates (my own runs, all green)

| Gate | Result |
|---|---|
| `cargo nextest run -p gauntlet-cli --test p9_self_improvement_containment` | 4/4 PASS |
| `cargo nextest run -p gauntlet-app promote::` | 40/40 PASS |
| `cargo nextest run -p gauntlet-app --test promotion candidate_only` | 1/1 PASS |
| `cargo nextest run -p gauntlet-ports -E "test(revision_ledger)"` | 3/3 PASS |
| `cargo nextest run -p gauntlet-adapter-state-sqlite --test contract_suite` | 5/5 PASS |
| `cargo nextest run -p gauntlet-arch-tests -E "test(optimizer)"` | 5/5 PASS |
| `just check` | clean (`cargo check --workspace --all-targets`) |

Ruler files verified unmodified (`git status` on `.wayfinder/answer-key.md`,
`.gauntlet/reference.lock`, `.wayfinder/reference.yaml`, and
`tests/fixtures/target/` — clean). Nothing staged or committed.

---

## Appendix — second-builder verification (session agent_75ee8e82)

**Concurrency disclosure.** This correction round was executed by TWO live
builder sessions. The orchestrator dispatched `agent_75ee8e82` as "the"
continuation builder after the original builder was reported dead; on
reconcile that session found `agent_fb4e0d63` (the reported-dead builder,
recovered) actively working the same slice, notified the coordinator
(response queued, never received), and coordinated through claim lines in
`cor1-progress.md` plus the Edit tool's stale-read guard (one p9 Write
attempt was safely refused when the peer had just rewritten the file — no
work was lost to trampling). The report above is the primary per-item
record; this appendix adds the second builder's independent contributions
and runs.

**Authorship corrections to the operational notes above.** The C8
(delta.rs comment) and C10 (gate.rs condition-1 docs, three regions)
rewrites were applied by the SECOND builder session (~10:44–10:47, not
10:32, not the coordinator): `delta.rs` comment rewritten to the C1-ground
argument naming the pinning test; `gate.rs` module-doc condition 1 + the
`governance_verdict` doc + its inner comment now state the superset truth
and no longer claim the extra-dispatch direction the check does not
enforce. Additionally, the loop-economics `reference-candidates.md`
2026-09-04 entry (which the C2 section and p9 header cite) was appended by
the second builder at ~10:57 — before that, the file's last touch was
07:28 and the p9 header's pointer was dangling.

**Independent empirical work (second builder's runs).**

- C2 leg-1 probe over the real binary with BOTH arms complete: the
  `review.mode` downgrade → `REJECTED` exit 1 with
  `settlement_access_weakened` AND `margin not met: control 10000 vs
  treatment 10000`, write-once sticky across processes — confirming the
  containment form and independently reproducing the loop-economics
  tie on a second delta key.
- C2 RED evidence capture (post-C1 tree, pre-rewrite): p9 4 run / 1
  passed / 3 FAILED, leg 2 with `PROVISIONAL (written=true)` and the
  "figure is underivable for the control arm" gap — the old fixture
  provably rested on the incompleteness the C1 fix removed.
- First full battery run caught p9 legs 2/3 RED (the peer's gate-level
  legs mid-fix, lib-side) — the battery did the job it exists for; after
  the fix, everything below went green.

**Second builder's final acceptance battery (one coherent run over the
settled tree, `~/.cache/cor1-battery2.sh` log):**

| Gate | Result |
|---|---|
| p9_self_improvement_containment | **4/4 PASS** |
| gauntlet-app promote:: | **40/40 PASS** |
| promotion candidate_only | **1/1 PASS** |
| gauntlet-ports revision_ledger | **3/3 PASS** |
| sqlite contract_suite | **5/5 PASS** |
| arch optimizer | **5/5 PASS** |
| just check | **PASS** — `Finished 'dev' profile in 0.12s`, no errors/warnings |
| improve_promote_gates (C5 teeth, separate run) | **1/1 PASS** |

Operational note for the failure log: a `while pgrep -f PATTERN` wait-loop
self-matches its own command line and spins forever (two instances here,
killed); `flock <lock> -c <script>` already waits its turn — no polling
loop is ever needed. Also: a non-login `flock -c` shell may lack
`~/.local/bin` on PATH — invoke `just` by absolute path in battery
scripts.
