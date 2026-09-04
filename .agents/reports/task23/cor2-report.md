# Task 23 — COR-2 final report (continuation builder, 2026-09-04)

Closes correction round 2. Predecessor landed R2/R3/R4/R6 (checkpointed GREEN
in `cor2-progress.md`) and died mid-round. **Reconciliation finding:** the
tree already carried R1 AND R5 code the progress log did not record —
orphaned builder state (edits landed, proof loop never run, log stale). Per
`.agents/lessons/session-interruption-recovery.md` this continuation
verified the landed code on its own runs, fixed what the round gates caught,
proved the teeth, and closed — it did not redo landed work.

## R1 (F-1, MAJOR) — condition 2 covers ALL FOUR evaluation arms

**Landed (predecessor's orphaned edits, verified line-by-line this session):**

- Port: `EvaluationRecord` (crates/gauntlet-ports/src/revision_ledger.rs)
  gains `regression_control_reference_digest` /
  `regression_treatment_reference_digest`; they are inside `facts_agree`
  (consistent with R4's metric/protected additions), written and read by the
  hand-rolled canonical JSON codec (`to_json` / `decode` FIELDS), so the
  record payload the sqlite adapter stores carries ALL FOUR digests durably
  (adapter stores the record's canonical JSON payload and re-decodes it on
  every read — no migration change needed).
- Harness (crates/gauntlet-app/src/promote/evaluate.rs): the record
  constructor derives the regression arms' digests from the regression runs'
  own outcomes, exactly as the held-out arms'; `run_arm` REFUSES — typed
  `EvaluationError::ReferenceMoved { arm, run, carried, current }`, before
  any record is appended — an arm whose project resolved a frozen reference
  digest different from the promoting project's verified lock. The CLI
  (improve_cmd.rs) derives `current_reference_digest` from the VERIFIED
  reference lock for both `evaluate` and `promote`.
- Gate (crates/gauntlet-app/src/promote/gate.rs): condition 2 checks all
  four arms against the current frozen reference digest; ANY mismatch is one
  typed `GateRefusal::ReferenceMoved` naming the offending arm(s).
- Shared contract suite
  (crates/gauntlet-ports/src/contract/revision_ledger.rs): new case
  `evaluations_carry_all_four_arms_reference_digests` — all four digests
  read back exactly; a re-append whose REGRESSION control or treatment arm
  carries a moved digest is the typed `EvaluationConflict`, stored record
  standing. Runs in all three legs (green/broken/caps-broken).

**RED → GREEN evidence (real runs, this continuation):**

- RED (context): the falsifier executed F-1 through the real binary —
  condition 2 PASSED over held-out digest ebba17… with regression digest
  3f6179… in ONE record; rejection came only from the margin.
- RED T1 (gate-level, this session): with condition 2's arm list reduced to
  the two held-out arms (pre-fix shape), the probe
  `a_moved_regression_ruler_is_rejected_with_reference_moved_naming_the_regression_arm`
  FAILS — `1 run: 0 passed, 1 failed` — with
  `Promoted(RevisionPromotionEvidence { metric: "iterations_per_settlement",
  control_figure: 30000, treatment_figure: 20000, margin_bps: 0, …,
  instrument_series_non_weaker: true, …, protected_capabilities: ["cap-eval"] })`:
  the moved regression ruler adjudicates to a FULL PROMOTION.
- RED T2 (harness-level, this session): with `run_arm`'s digest refusal
  inverted, `a_moved_regression_ruler_refuses_evaluation_and_records_nothing`
  FAILS — `1 run: 0 passed, 1 failed` — the moved-ruler evaluation is no
  longer refused at the source.
- GREEN (restored tree): both probes PASS inside promote:: 46/46; the suite
  case passes inside ports revision_ledger 3/3 and sqlite contract_suite 5/5.
- Byte-identical restores proven: gate.rs sha256
  `3daa4ae44921e769e310aefce6b09513e476ed1e9b04ffa2891eac00f69541e3`,
  evaluate.rs `5c57041c772e7dba0b40b28cee42071458c26b09aaccea335052e37f7f4beb7f`
  — identical before and after the mutation cycles.

## R5 (F-5, minor) — governance-target classification is case-normalized

**Landed:** `target_is_governance_bearing` lowercases the target before the
prefix match; the normalization is documented on the function ("capitalization
is not an escape hatch from governance"). Unit pin
`the_governance_target_classification_is_case_normalized` asserts
"Reference-pipeline", "Settlement-machinery", "Scheduler-machinery",
"Authority-surface", "GOVERNANCE" ⇒ true (probe-E spellings included), and
free targets still ⇒ false.

**RED → GREEN evidence:** RED was reproduced in the predecessor's checkpointed
RED round (probe E verbatim, log line: `"Reference-pipeline" classified free`)
and re-proven this session — T3: with `to_lowercase()` removed the pin FAILS
(`1 run: 0 passed, 1 failed`, `"Reference-pipeline" must classify
governance-bearing regardless of case`); restored, gate.rs sha256 matches
pre-mutation exactly, pin PASSES inside promote:: 46/46.

## What the round gates caught in the orphaned edits (fixed; nothing weakened)

The predecessor never reached the full battery. First run of it failed:

1. `just lint`: clippy `result_large_err` ×3 — R1's `ReferenceMoved` variant
   made `EvaluationError` 136 bytes (> 128 threshold). Fix: `carried`/`current`
   are `Box<Digest>` (56 bytes) — types unchanged, Display unchanged; the two
   test assertions deref. All three flagged functions clean.
2. arch-tests FULL: hardcoded-literal scan — 10 violations in the R1/R2-era
   test fixtures: `"AK-9000"` (port-shaped) and nine `'3' next to 'deadline'`
   hits (digit literals inside the moved-ruler executor's units, which join
   the trait signature's deadline-worded parameter with the body). Fix: named
   fixture constants — `SHIELD_UNIT` ("AK-EXTRA", digit-free),
   `EXECUTOR_CONTROL_TAG`/`EXECUTOR_TREATMENT_TAG` used in ALL THREE executor
   doubles — plus `as_path() == Path::new(...)` for a `cmp_owned` hit. The
   scan policy was NOT touched; the violations were removed by naming data.
3. `cargo fmt --check`: 5 spots from R1/R2/R3/R6 edits, all reformed exactly
   per rustfmt's own diff. Plus one garbled doc comment in revision_ledger.rs's
   R1 block ("reference project of its own").

## Full acceptance battery — all green in this builder's own runs (final tree)

| Gate | Result |
|---|---|
| `cargo nextest run --test p9_self_improvement_containment` | 4 tests run: 4 passed |
| `cargo nextest run -p gauntlet-app promote::` | 46 tests run: 46 passed (517 skipped) |
| `cargo nextest run -p gauntlet-app --test promotion candidate_only` | 1 test run: 1 passed |
| `cargo nextest run -p gauntlet-ports -E "test(revision_ledger)"` | 3 tests run: 3 passed (green leg clean; broken + caps-broken legs catch) |
| `cargo nextest run -p gauntlet-adapter-state-sqlite --test contract_suite` | 5 tests run: 5 passed (incl. `sqlite_revision_ledger_passes_the_shared_contract_suite`) |
| `cargo nextest run -p gauntlet-arch-tests` (FULL) | 235 tests run: 235 passed |
| `just check` | exit 0 (2m55s cold, 22.85s incremental re-run on final tree) |
| `just lint` | clean (clippy `--workspace --all-targets -- -D warnings` + `cargo fmt --check`) |

Teeth proof table (mutate → RED → restore byte-identically):

| Tooth | Mutation | RED observed | Restored |
|---|---|---|---|
| R1 gate condition 2, four arms | arm list → 2 held-out arms | probe FAIL, moved-ruler world PROMOTED | gate.rs sha256 match |
| R1 harness evaluate-time refusal | `!=` → `==` in `run_arm` | probe FAIL, no typed refusal | evaluate.rs sha256 match |
| R5 case normalization | `to_lowercase()` removed | probe FAIL, "Reference-pipeline" classifies free | gate.rs sha256 match |

## Deviations and notes

- **Dispatch vs tree:** dispatch said R1 and R5 were unstarted; the tree held
  both (orphaned builder state). This continuation landed the missing proof
  loop and the gate-caught fixes rather than rewriting; every claim above is
  from its own runs.
- R1's RED evidence combines the falsifier's real-binary F-1 reproduction
  (prior round) with this session's T1/T2 mutations; R5's RED was already
  checkpointed by the predecessor's RED round and was re-proven (T3).
- `EvaluationError::ReferenceMoved` boxes its two digests — a clippy-policy
  accommodation that does not change the variant's typing or semantics.
- Known environment quirk (non-blocking): `sccache --stop-server` exits 2
  when the server is idle; the gates themselves were green.
- Build discipline held throughout: one top-level flock per command, sccache
  stop inside the same flock'd command, background builds polled, parallelism
  untouched. No file under `.wayfinder/` or `.gauntlet/` was opened for write;
  no assertion weakened; no dependency added; nothing staged or committed.

## Not owed by this round (carried)

- The `runners.by_role.<role>.model` Inert classification trigger-record and
  the falsifier's not-attacked list belong to the closure, per the brief.
- R2/R3/R4/R6 evidence stands on the predecessor's checkpointed runs; their
  tests were re-run green in every battery pass of this session.
