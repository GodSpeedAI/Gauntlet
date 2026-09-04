# COR-2 progress log

STARTED 2026-09-04 COR-2 builder: recon complete (brief, falsifier-round1, charter, ports/app/arch sources read).
Baseline check next; RED probes for R2/R3/R4/R5/R6 first (they compile against the current API), then R1's port record extension.
RED ROUND 1 (all probes compiled against the pre-fix API; real runs):
- R2 (falsifier probe A verbatim): promote::tests::an_extra_deterministic_settlement_cannot_shield_a_weakened_unit FAIL —
  Promoted { control_figure: 30000, treatment_figure: 15000, instrument_series_non_weaker: true } — the extra det
  settlement shields the model-rung u1 exactly as the falsifier observed.
- R3 (probe B verbatim): a_margin_above_the_basis_point_ceiling_is_never_a_silent_clamp FAIL —
  Promoted { control_figure: 30000, treatment_figure: 0, margin_bps: 15000 } — 100% improvement passed a declared 150% margin.
- R5 (probe E verbatim): the_governance_target_classification_is_case_normalized FAIL — "Reference-pipeline" classified free.
- R4 (probe C): ports suite revision_ledger_green_is_clean FAIL with both findings — re-append with different declared metric
  AND with emptier protected set each swallowed as the idempotent no-op.
- R6 (probe H verbatim): arch controls the_control_proves_the_raw_write_classes_have_teeth + every_raw_write_env_and_spawn_token_is_reported_individually FAIL — scan reported [] (zero violations) for the raw write/env/spawn fixture.
- promote:: RED run: 43 tests, 40 passed / 3 failed (only the new probes red).
Files touched so far (probes only): tests.rs, contract/revision_ledger.rs, optimizer.rs.
DONE R2: gate.rs instrument_series_verdict pairs BY UNIT (join on unit id; extra det settlement can no longer shield a
weakened unit; missing counterpart = Undecidable evidence gap). Position-based unequal-arm undecidable replaced by the
unit join; module + header docs updated.
DONE R5: gate.rs target_is_governance_bearing lowercases the target before the prefix match; docs updated.
DONE R3: gate.rs PromotionGate::new -> Result<Self, GateConfigError::MarginOutOfRange{margin_bps}> (refuses >10 000bps);
margin_holds doc updated (saturating sub is the 10 000 ceiling's arithmetic, not a clamp); resolve.rs PromotionMargin
branch refuses n>10_000 (ValueOutOfRange, both env and front-matter layers); defaults.rs + docs/reference/configuration.md
updated honestly; new resolve test a_margin_above_ten_thousand_bps_is_a_typed_configuration_error; new boundary test
the_basis_point_ceiling_itself_is_a_legal_exactly_one_hundred_percent_bar; call sites updated (tests.rs gate_with_dyn,
improve_cmd.rs promote cmd, p9 x3, app/tests/promotion.rs x1). Probe B test converted to the constructor-refusal form
(let Err(...) = ... else panic; PromotionGate holds trait objects so expect_err is unimplementable).
GREEN: cargo nextest run -p gauntlet-app --no-fail-fast => 561 tests run: 561 passed (was 40/43 pass + 3 probe FAILs).
Files: gate.rs, tests.rs, resolve.rs, defaults.rs, improve_cmd.rs, p9_self_improvement_containment.rs, promotion.rs,
docs/reference/configuration.md.
NEXT: R4 (facts_agree += metric + protected), R6 (arch tokens), R1.
DONE R4: ports facts_agree now includes the evaluation's DECLARED metric and protected set (plus doc). Suite case
evaluations_are_idempotent_and_rooted pins: re-append with different metric => EvaluationConflict; emptier protected =>
EvaluationConflict; stored record's original declarations stand.
GREEN: cargo nextest run -p gauntlet-ports -E "test(revision_ledger)" => 3/3 (green leg clean incl. new probes;
broken/caps legs still catch).
DONE R6: arch optimizer rule A extended with the raw write/env/spawn classes (std::fs::write, fs::write(, File::create,
OpenOptions, set_var(, Command::new); module docs updated; two new negative controls (probe-H fixture + per-token
planted shapes).
GREEN: cargo nextest run -p gauntlet-arch-tests optimizer => 7/7 (controls report the planted shapes; the REAL promote
module still scans clean).
Files: crates/gauntlet-ports/src/revision_ledger.rs, crates/gauntlet-ports/src/contract/revision_ledger.rs,
crates/gauntlet-arch-tests/src/optimizer.rs.
NEXT: R1 (port record regression digests + gate condition 2 all-four + harness evaluate-time refusal + wiring).

=== CONTINUATION (COR-2 builder #2, 2026-09-04) ===
STARTED CONTINUATION: dispatched to land R1+R5; RECONCILIATION per
.agents/lessons/session-interruption-recovery.md — the tree already carries
R1 AND R5 code the log does not record (orphaned builder state: edits landed,
proof loop never run, log stale):
- R5 present: gate.rs target_is_governance_bearing lowercases before the
  prefix match (doc'ed); tests.rs the_governance_target_classification_is_case_normalized
  pins "Reference-pipeline"/"Settlement-machinery" => true. RED for R5 was
  already checkpointed above (probe E FAIL).
- R1 present: port EvaluationRecord += regression_{control,treatment}_reference_digest
  (fields, facts_agree, to_json, decode FIELDS); harness derive_run (evaluate.rs)
  records all four + typed EvaluationError::ReferenceMoved refusal in run_arm
  BEFORE any append; gate condition 2 checks all four (ReferenceMoved naming
  the offending arms); contract suite case
  evaluations_carry_all_four_arms_reference_digests (green leg + moved
  regression control/treatment re-append => EvaluationConflict); sqlite adapter
  stores the record's canonical JSON payload (four digests durable);
  improve_cmd.rs derives current_reference_digest from the VERIFIED lock for
  both evaluate and promote; p9 + app tests carry the moved-ruler probes
  (a_moved_regression_ruler_is_rejected... / a_moved_regression_ruler_refuses_evaluation_and_records_nothing).
Continuation edits: doc-comment garble fixed in revision_ledger.rs R1 comment
("reference project of its own" -> "frozen reference"). Nothing else changed.
NEXT: proof loop — just check, then the full acceptance battery, then teeth
(mutate->RED->restore, sha256) for the three new mechanisms, then cor2-report.md.
CONTINUATION PROOF LOOP (builder #2):
- just check: GREEN (exit 0, 2m55s).
- Battery legs (first pass): p9 4/4; promote:: 46/46 (was 40 pre-COR-2);
  promotion::candidate_only 1/1; ports revision_ledger 3/3 (green clean +
  broken + caps-broken); sqlite contract_suite 5/5 (incl. the shared
  revision-ledger suite with the new four-arm case).
- arch-tests FULL: 2 FAILURES found (predecessor's edits never ran the full
  package): (a) just lint — clippy result_large_err x3 (EvaluationError
  136 bytes > 128: R1's ReferenceMoved variant); (b) hardcoded-literal scan
  — 10 violations in the R1/R2-era test fixtures ('9000' port-shaped unit
  id; nine '3'-next-to-deadline hits from digit literals in the moved-ruler
  executor units).
- FIXES (no assertion weakened; scan/lint policy untouched):
  * evaluate.rs: ReferenceMoved carries Box<Digest> for carried/current
    (typed shape unchanged; error now 56 bytes).
  * tests.rs: fixture digits named — SHIELD_UNIT ("AK-EXTRA", digit-free
    unit id), EXECUTOR_CONTROL_TAG/EXECUTOR_TREATMENT_TAG (named run tags
    in ALL THREE executor doubles), PathBuf::from -> as_path() == Path::new
    (cmp_owned), assert derefs for the boxed digests.
  * fmt (rustfmt's own suggestions): tests.rs x2, optimizer.rs x2,
    revision_ledger.rs facts_agree line; doc garble in revision_ledger.rs
    R1 comment fixed.
- RE-GREEN: literals scan 2/2; just lint CLEAN (clippy -D warnings +
  fmt --check, 2m57s); ports 3/3; promote:: 46/46; arch FULL 235/235
  (was 232/235 + 1 not-run).
NEXT: teeth (mutate -> RED -> restore byte-identical, sha256) for the three
new mechanisms, then final sweep.
TEETH (mutate -> RED -> restore byte-identically; digests above = pre/post, all MATCH):
- T1 (R1, gate condition 2): arm_digests reduced to the two held-out arms
  (pre-fix shape) => a_moved_regression_ruler_is_rejected_with_reference_moved_
  naming_the_regression_arm RED — "a regression set frozen under a moved ruler
  must never adjudicate clean: Promoted(RevisionPromotionEvidence { metric:
  "iterations_per_settlement", control_figure: 30000, treatment_figure: 20000,
  margin_bps: 0, ..., instrument_series_non_weaker: true, ..., protected_
  capabilities: ["cap-eval"] })" — 1 run: 0 passed, 1 failed. The falsifier's
  F-1 hole reproduces exactly. RESTORED: gate.rs sha256 3daa4ae4... MATCH.
- T2 (R1, harness evaluate-time refusal): run_arm comparison inverted (!= -> ==)
  => a_moved_regression_ruler_refuses_evaluation_and_records_nothing RED —
  1 run: 0 passed, 1 failed (the moved-ruler evaluation is no longer refused).
  RESTORED: evaluate.rs sha256 5c57041c... MATCH.
- T3 (R5, classification normalization): to_lowercase() removed =>
  the_governance_target_classification_is_case_normalized RED —
  '"Reference-pipeline" must classify governance-bearing regardless of case' —
  1 run: 0 passed, 1 failed. Probe E reproduces. RESTORED: gate.rs sha256
  3daa4ae4... MATCH (also matches the pre-T1 value: the file round-trips).
FINAL SWEEP on the restored tree (all five sha256s verified against the
pre-mutation digests): p9 4/4; promotion::candidate_only 1/1; sqlite
contract_suite 5/5; promote:: 46/46; ports revision_ledger 3/3; just check
GREEN (22.85s incremental); just lint GREEN (from the fix round, same tree);
arch-tests FULL 235/235 (from the fix round, same tree).
ROUND CLOSED by builder #2 — nothing staged, nothing committed.
