# Task 23 — executing falsifier, round 2 (post-COR-2)

VERDICT: **CONFIRM** — all six round-1 findings (F-1..F-6) are closed: each
round-1 reproduction was re-executed and now dies at the corrected locus, and
the mandated new-surface attacks on each fix did not reach a promotion, a
record, or a silent pass. Seven declared teeth (the four p9 legs plus the
three correction mechanisms) each went RED under individual neutralization
and the tree was restored byte-identically (sha256-proven). Two MINOR
residuals and several INFORMATIONAL notes are recorded below — none is a
reproducible hole in the composed system and no tooth failed to bite.

Attack surface: blind over builder/critic/correction reports; knowledge =
falsifier-brief (round 2), charter, spec §19.3/§19.4/§5.3.8/B.7/§23.1 P9,
AK-056/AK-057, the round-1 attack ledger (`falsifier-round1.md`), the tree
as it stands. All attacks executed against the REAL adapters (embedded-sqlite
revision/capability/state stores), the REAL §19.4 gate, and the REAL
`gauntlet` binary in /tmp scratch worlds (removed after).

Baseline (my own runs, pre-attack): p9 4/4 PASS; promote:: 49/49 PASS;
promotion::candidate_only 1/1 PASS; arch optimizer:: 7/7 PASS.

---

## Round-2 mandates (re-attacks)

### R-1 (round-1 F-1, moved regression ruler) — CLOSED

The round-1 binary reproduction, re-executed verbatim (scratch world
`/tmp/gauntlet-falsifier2/binworld`; `held` = pristine fixture copy with its
lock, `reg` = fixture copy with `.gauntlet/reference.lock` removed and
`.wayfinder/answer-key.md` amended):

```
sha256sum held/.wayfinder/answer-key.md reg/.wayfinder/answer-key.md
# ebba17b091b2c66b161a0196d3fd3497759a27cc747c5b7550f0430f48a44ce5  held/...
# 3f6179169ba2bd92d9cf20b810b89fc86b2bb51361c35c82783eba227e8404d9  reg/...
gauntlet improve propose --from-result candidate.json
# improve: revision=rev-d537428324f57137 admitted (idempotent=false)
gauntlet improve evaluate rev-d537428324f57137 --held-out $W/held \
    --regression $W/reg --protected cap-eval --metric iterations_per_settlement
# gauntlet improve evaluate: the control arm (run C4SWZCJJ76MPWFFAC08CZ6VD21)
#   settled against frozen reference sha256:3f617916..., but the promoting
#   project's verified lock carries sha256:ebba17b0... — a moved ruler is
#   refused at evaluation time and nothing was recorded (spec 19.4, condition 2)
# exit=1
gauntlet improve promote rev-d537428324f57137
# improve: revision rev-d537428324f57137 PROVISIONAL (written=false) —
#   insufficient evidence is never a promotion (spec 19.4)
# improve: gap — missing evaluation: no isolated-evaluation record is on the
#   ledger ... exit=1
```

Round 1 the evaluation RECORDED and the gate passed condition 2 seeing 2 of 4
arms; now the harness refuses at evaluate time (per-arm
`outcome.reference_digest != request.current_reference_digest`, before any
record), `current_reference_digest` deriving from `verify_lock`
(re-hashes the artifacts against the lock — not a trust-the-lock read), and
the gate independently checks ALL FOUR arms' digests in the durable record.

New-surface attacks, all held:
- Moved HELD-OUT ruler (`--held-out $W/reg --regression $W/reg`): refused at
  the first arm, same typed error, exit 1, nothing recorded.
- Crafted record with moved REGRESSION digests inside (probe J1, real gate +
  real sqlite ledger): `REJECTED reasons=["reference_moved"]`.
- Record payload missing a regression digest field (probe J2): decode refuses
  — `Corrupt (the ledger field "regression_treatment_reference_digest" is
  absent)`. An arm's digest cannot go unrecorded; a "fifth arm" field is
  refused as an unknown field (probe J3).
- Ruler tampered AFTER evaluation, at promote time (answer key edited without
  re-freezing): `gauntlet improve promote: the reference lock does not
  verify: the answer key MUTATED against the reference lock: frozen
  sha256:ebba17..., current sha256:527c1407... — the ruler has moved (spec
  9.4, I1)`; exit 1. (Key restored byte-identically in the scratch world.)
- A digest mismatched inside a record with all four arms present cannot
  promote: the gate compares every arm against the verified current digest.
- Regression set sharing the held-out directory (`--held-out $W/held
  --regression $W/held`): full 4-arm evaluation records cleanly (4 real
  composed runs, `parent_run` set on both treatments, 40 memory rows
  imported) and the inert candidate is REJECTED on the margin with the
  rollback ref standing — legal redundancy, not a bypass.

### R-2 (round-1 F-2, positional instrument pairing) — CLOSED (core), residual noted

Probe A re-executed verbatim (real `EmbeddedRevisionLedger` +
`EmbeddedCapabilityLedger` + real `PromotionGate`): control held-out
`[(u1, det)]`, treatment `[(u0, det), (u1, MODEL)]`, inert delta, all digests
current → **REJECTED reasons=["settlement_access_weakened"]** (round 1:
Promoted). The join is by unit.

Variants (same harness):
- A3 weakened unit buried among extra settlements — REJECTED
  `settlement_access_weakened` (order-independent for single rows).
- A4 control settlement names NO instrument — `PROVISIONAL
  written=true reasons=["evaluation_incomplete"]` (an evidence gap, never a
  failure, never a pass; with figures that hold the margin).
- A5 treatment missing a control-settled unit — REJECTED
  (`governance_altered` among the recorded reasons).
- A2a/A2b duplicate unit rows — see extension finding E-2 (order
  sensitivity, unreachable through the real loop).

### R-3 (round-1 F-3, margin saturation) — CLOSED

- Constructor (probe B1): `PromotionGate::new(..., 15_000)` →
  `Err(MarginOutOfRange { margin_bps: 15000 })` (typed; the former silent
  `.min(10_000)` clamp is gone).
- Resolution (binary): `GAUNTLET_PROMOTION_MARGIN=15000 gauntlet improve
  promote ...` → `the configuration did not resolve: configuration:
  'promotion.margin' carries a value outside its setting's domain (spec
  8.2)`; exit 1. (resolve.rs refuses `> 10_000`.)
- Ceiling arithmetic exact (probes B3/B4): at exactly 10 000 bps a 100%
  improvement (figure 0) PROMOTES and a 50% improvement REJECTS
  `margin_not_met`; margin 0 keeps strict improvement (equal figures refuse).

### R-4 (round-1 F-4, evaluation re-declaration) — CLOSED

Probe C over the real sqlite adapter: first append `idempotent=false`;
agreeing re-append `idempotent=true`; re-append of the SAME four-run set with
a different declared metric → **`EvaluationConflict`**; with an emptier
protected set → **`EvaluationConflict`**; with differing figures →
**`EvaluationConflict`**. `facts_agree` now covers the operator's
declarations plus all four arms' digests, metrics, parents, and the record
instant.

### R-5 (round-1 F-5, governance case spelling) — CLOSED

Probe E: `target_is_governance_bearing("Reference-pipeline") == true`,
`("SETTLEMENT-machinery") == true`, `("Authority-flow") == true`,
`("Scheduler-tuning") == true`, `("governance") == true`,
`("referenceX") == true` (prefix semantics retained, documented),
`("pipeline-reference") == false`, `("Iteration-economy") == false`,
`("") == false`. Capitalization is no longer an escape from the
classification.

### R-6 (round-1 F-6, containment scan raw write classes) — CLOSED for the declared classes

Probe H over the real `scan_optimizer_containment`:
- H1: the real promote module → 0 violations.
- H2: the round-1 probe-H fixture (raw `fs::write` of GAUNTLET.md and the
  roles dir, `set_var`, `Command::new`) → every class REPORTED;
  `File::create`/`OpenOptions` each carry their own planted in-suite
  negative control (`every_raw_write_env_and_spawn_token_is_reported_individually`).
- H3: NEW spellings escape — see extension finding E-1.

---

## Extension findings

### E-1 MINOR — the containment scan's token list still misses raw-write spellings beyond the ones round 1 named

`fs::rename`, `fs::copy`, `File::options()` (the `OpenOptions` alias),
`fs::hard_link`, `fs::remove_file`, and the `write!` macro each plant a
config/roles-overwriting line in a fixture promote-module file with ZERO
reported violations (probe H3, verbatim output in the progress log). Same
class as round-1 F-6, one spelling generation later; the correction closed
exactly the named classes with per-token negative controls. Mitigations keep
this MINOR: the behavioral half of AK-057 (`promotion::candidate_only`)
proves the real services write nothing, and the service structs hold only
ledger+clock. Suggested locus: add the rename/copy/hard_link/remove_file/
`File::options` tokens (with negative controls), or replace the token list
with an import-level allowlist for the module.

### E-2 MINOR (port-level residual; unreachable through the shipped harness) — the by-unit join takes the FIRST treatment row per unit, so a duplicated unit's verdict is store-order sensitive

Probes A2a/A2b: control `[(u1, det)]`; treatment `[(u1, det), (u1, model)]`
→ **PROMOTED** with `instrument_series_non_weaker=true` (`.find` picks the
first row); the SAME rows in the other order → REJECTED
`settlement_access_weakened`. Reachability: `derive_run_metrics` builds
settlement facts from `state.settlements(run)`, and `commit_settlement` is
idempotent by its (run, unit) key (adapter test
`commit_settlement_is_the_spec_sequence_and_idempotent_by_key`), so a real
run can never carry two settlement rows for one unit — only a hand-appended
`EvaluationRecord` (direct port use) can. Suggested locus: refuse or
undecide a unit settled more than once on either side (pair on
unit + `occurred_at`, or treat duplicate rows as `Undecidable`).

### INFORMATIONAL — verified-holding surfaces (negative controls and boundary probes)

- §19.4 state machine over the real adapter (probes G1–G9): pending→
  provisional legal; provisional→rejected (concluding) legal; stale claims
  refuse (`StaleDecision`); restore of a non-promoted revision refuses
  (`StaleRestore`); re-adjudication past a final state aborts typed
  (`AlreadyFinal`); a decision record claiming a final prior state refuses to
  compose; empty reason sets refuse to compose.
- Substrate (probes I1–I5, two connections on one db file):
  cross-connection double-promotion refuses (`StaleDecision` — the claim is
  validated inside the `BEGIN IMMEDIATE` transaction against the same
  snapshot the record lands in); restore is durable and derived
  (`rolled_back=true` across connections); second restore refuses;
  disagreeing re-admission is `ProposalConflict`; evaluation of an unknown
  revision is `UnknownRevision`. Migration 0010 and the adapter carry no
  UPDATE/DELETE/DROP/ALTER (INSERT-only verified by grep).
- Re-evaluation supersedes (probe G1): with two evaluation records the gate
  adjudicates the LAST; the decision is then write-once (G2). Re-running
  evaluations until one promotes is operator prerogative over REAL evidence —
  the spec's protection is evidence quality, not evaluation-once.
- Appending an evaluation record to an already-PROMOTED revision is not
  refused by the harness (it checks only proposal existence), but cannot flip
  anything: adjudication of a final revision aborts and the rollback path
  reads only the derived entry. Fidelity note, no exploit.
- Rejected verdicts record only the FAILED conditions' reasons; concurrent
  evidence GAPS are subsumed (probe A4's first shape). Correct per the
  charter's verdict contract; the operator does not see the gap detail on a
  rejection. Fidelity note only.
- Condition 6's per-capability figure is `max(held-out, regression)` per arm.
  A cross-set mask (treatment weaker on held-out, stronger on regression)
  cannot complete a promotion: terminal arms on the same frozen plan settle
  the same unit set, so per-set settlement counts are equal whenever the
  margin is decidable; any non-terminal arm makes the figures underivable →
  Provisional. Analyzed, unreachable.
- CLI refusal shapes, real binary: decision-carrying payload refused
  (`candidate_revision@v1 rejects unknown field "decision"`);
  `candidate_revision@v2` refused (no validator); unknown metric token
  refused with the typed vocabulary (exit 2); rollback of a pending revision
  = `NothingToRestore`; rollback of an unknown revision = typed refusal;
  promote/evaluate of a moved or tampered ruler refuse typed; only a
  `Promoted` verdict exits 0 (fail-closed operator script surface).
- `runners.by_role.<role>.model` remains classified Inert (the round-1 latent
  policy note persists) but stays unreachable through the binary:
  `validate_delta` refuses any delta key with no Annex E override variable
  before any arm runs.
- `SettlementFact.occurred_at` is recorded and durable but read by no gate
  condition today (available for E-2's unit+instant pairing).
- `RevisionPromotionEvidence` carries the held-out digests and regression
  unit counts but not the regression digests; the durable `EvaluationRecord`
  (the evidence the verdict rests on) carries all four — no gap.

---

## Teeth — every declared tooth BITES (neutralize → RED → restore byte-identically)

Seven single-mechanism neutralizations across four files (sha256 recorded
before/after), observed over the REAL suites:

| Tooth | Neutralization | Observed RED |
|---|---|---|
| Leg 1 / AK-056: the delta verification classification (`preserves_verification`) | → `true` (delta.rs) | p9 `the_verification_weakening_candidate_is_rejected_over_the_real_loop` RED; `a_weakening_candidate_improves_the_metric_yet_is_rejected_with_settlement_access_weakened` RED; `a_verification_bearing_downgrade_is_the_weak_class` RED |
| Leg 2's condition-7 floor (`margin_holds`) | → `true` (gate.rs) | `margin_zero_demands_strict_improvement`, `a_configured_margin_the_improvement_does_not_meet_refuses`, `a_margin_the_improvement_does_not_meet_is_rejected`, `an_equal_metric_refuses_at_margin_zero`, `the_basis_point_ceiling_itself_is_a_legal_exactly_one_hundred_percent_bar` RED (5/5). Leg 2 itself stayed GREEN (control leg unaffected) |
| Leg 3: transition-derived restore history (`entry.rolled_back` ignored in `RollbackService::restore`) | rollback.rs | p9 `the_promoted_revision_rolls_back_and_the_restore_survives_a_reopen` RED; `rollback_flips_the_derived_state_and_refuses_the_unpromoted` RED |
| Leg 4: the missing-evidence floor (`MissingEvaluation` → Provisional, nothing written) | early return → `Promoted` (gate.rs) | p9 `a_revision_without_evaluation_is_provisional_and_never_promotes` RED; `missing_evaluation_adjudicates_provisional_and_stays_pending` RED |
| R-1 gate side: condition 2's four-arm check (`if !moved_arms.is_empty()`) | → `if false &&` (gate.rs) | `a_moved_reference_is_rejected` RED; `a_moved_regression_ruler_is_rejected_with_reference_moved_naming_the_regression_arm` RED |
| R-1 harness side: the evaluate-time per-arm refusal | → `if false &&` (evaluate.rs) | `a_moved_regression_ruler_refuses_evaluation_and_records_nothing` RED — with the failure output showing the round-1 hole resurrected verbatim (a record with regression digests `cd00…` recorded despite moved arms) |
| R-2: the by-unit instrument-series join | → unconditional `NonWeaker` (gate.rs) | `an_extra_deterministic_settlement_cannot_shield_a_weakened_unit` RED; `an_instrument_downgrade_is_rejected_as_settlement_access_weakened` RED |

Mutated full promote suite: 49 run, 35 passed, **14 failed** — every failure
attributable 1:1 to one of the seven neutralized mechanisms; no unrelated
failures. p9 under mutation: 3 RED (legs 1/3/4), leg 2 green as expected.

POST-RESTORE REGRESSION SWEEP (all green): p9 4/4; promote:: 49/49;
promotion::candidate_only 1/1; arch optimizer:: 7/7.

## Not attacked (next round's benefit)

- The orchestrator's full battery (check/test/lint/deny/boundaries/ruler over
  1507) was not re-run by me post-restore — slice suites + the arch optimizer
  suite were, all green.
- Real-runner optimizer dispatch (`propose` without `--from-result`) —
  mock/scripted paths only; task 25 territory.
- Fuzzing the hand-rolled JSON codecs (structurally strict by reading; not
  fuzzed).
- The 49 promote-module tests were exercised under mutation but not
  individually audited for assertion strength beyond the 14 teeth-relevant
  ones.
- A true cross-PROCESS race (two OS processes on one db) — probed with two
  connections in one process; SQLite's write lock makes the two cases the
  same invariant, but two-process timing was not exercised.
- `DevelopmentalImport` internals (task-22 surface) consumed, not re-attacked.

## Restore ledger

| File | sha256 before | sha256 after | verdict |
|---|---|---|---|
| `crates/gauntlet-app/src/promote/gate.rs` | 3daa4ae44921e769e310aefce6b09513e476ed1e9b04ffa2891eac00f69541e3 | 3daa4ae44921e769e310aefce6b09513e476ed1e9b04ffa2891eac00f69541e3 | byte-identical |
| `crates/gauntlet-app/src/promote/delta.rs` | 4276075ececd7e80b08b0995a8162786ca6c1e264301cceb7995a5d37b30b9bc | 4276075ececd7e80b08b0995a8162786ca6c1e264301cceb7995a5d37b30b9bc | byte-identical |
| `crates/gauntlet-app/src/promote/evaluate.rs` | 5c57041c772e7dba0b40b28cee42071458c26b09aaccea335052e37f7f4beb7f | 5c57041c772e7dba0b40b28cee42071458c26b09aaccea335052e37f7f4beb7f | byte-identical |
| `crates/gauntlet-app/src/promote/rollback.rs` | 037c8b2d84ad913857ea814ad70d46f8ec72bc30077b9e69339cac88201c51fc | 037c8b2d84ad913857ea814ad70d46f8ec72bc30077b9e69339cac88201c51fc | byte-identical |

Repository files CREATED by the falsifier: this report only. Scratch worlds
exclusively under /tmp/gauntlet-falsifier2 (probe crate + binary world),
removed after the sweep (`rm -rf` verified). Nothing planted under
`.gauntlet/` (directory listing proof: only the pre-existing
`comparison-contract.yaml`, `policies`, `reference.lock`). The frozen ruler
(`.wayfinder/answer-key.md`), `.gauntlet/reference.lock`, and
`.wayfinder/reference.yaml` were never opened for write in the repository
(the answer-key tamper probe ran on a /tmp fixture copy and was restored
there byte-identically, sha256-proven). `git status` delta vs. dispatch is
the builder's uncommitted task work, untouched.

## Progress log

- Initialized. Brief, charter, spec sections, AK-056/AK-057, round-1 ledger read.
- Code survey complete: gate.rs (4-arm condition 2, by-unit series join,
  constructor margin refusal, case-normalized governance match, u128 margin
  arithmetic); evaluate.rs (per-arm ReferenceMoved refusal at evaluate time,
  verified parent isolation, verified-lock digest); metrics.rs (fixed-point
  figures, underivable = None); proposal.rs (content-derived revision id);
  rollback.rs; optimizer.rs scan (raw-write tokens + per-token negative
  controls); resolve.rs:1122 (margin ceiling at resolution); improve_cmd.rs
  (verify_lock at evaluate AND promote; fail-closed exit codes).
- Baseline (my own runs): p9 4/4 PASS; promote:: 49/49 PASS; candidate_only
  1/1 PASS; arch optimizer:: 7/7 PASS.
- Probe crate /tmp/gauntlet-falsifier2 (release binary over the real sqlite
  adapters + real gate + real scan function). PROBE RESULTS:
  A   round-1 shield (extra det u0, weakened u1) ... REJECTED settlement_access_weakened  [R-2 CLOSED]
  A2a duplicate unit rows, weak row SECOND ........ PROMOTED series_non_weaker=true       [E-2]
  A2b duplicate unit rows, weak row FIRST ......... REJECTED settlement_access_weakened   [E-2]
  A3  weakened unit buried among extras ........... REJECTED settlement_access_weakened
  A4  control instrument fact missing ............. PROVISIONAL written=true (evaluation_incomplete)
  A5  treatment missing control unit .............. REJECTED (governance_altered among reasons)
  B1  constructor margin 15000 .................... MarginOutOfRange
  B2  constructor margin 10000 .................... Ok
  B3  10000bps, 100% improvement (figure 0) ....... PROMOTED (ceiling exact)
  B4  10000bps, 50% improvement ................... REJECTED margin_not_met
  C1–C5 evaluation re-declaration ................. idempotent=true on agreement; EvaluationConflict on metric/protected/figures disagreement  [R-4 CLOSED]
  E   governance-target classification ............ case-normalized  [R-5 CLOSED]
  G1  two evaluations, good LAST .................. PROMOTED on last record; G2 re-adjudication → AlreadyFinal
  G3–G9 state machine / stale claims / empty reasons — all refuse typed
  J1  moved regression digests inside a record .... REJECTED reference_moved  [R-1 gate side]
  J2  record missing a regression digest field .... Corrupt (field absent)
  J3  unknown fifth-arm field ..................... Corrupt (unknown field)
  H1  real promote module ......................... 0 violations
  H2  round-1 raw-write fixture ................... all declared classes REPORTED  [R-6 CLOSED]
  H3  rename/copy/File::options/hard_link/remove_file/write! — ALL ESCAPE  [E-1]
  I1–I5 cross-connection staleness, restore durability, conflicts, unknown revision — all hold
- BINARY WORLD (real gauntlet binary, /tmp scratch fixture world):
  * R-1 verbatim repro: evaluate REFUSES the moved regression ruler at
    evaluate time ("a moved ruler is refused at evaluation time and nothing
    was recorded"); promote then rests at the provisional floor.  [R-1 CLOSED]
  * Moved held-out ruler: refused at the first arm. Tampered promoting-project
    ruler at promote time: verify_lock aborts typed ("the answer key MUTATED
    against the reference lock").
  * Margin 15000 through the binary: config resolution refuses ("carries a
    value outside its setting's domain").  [R-3 CLOSED]
  * CLI refusals all hold: decision-carrying payload, candidate_revision@v2,
    unknown metric token, unknown-revision rollback, pending-rollback.
  * Full 4-arm evaluation with a shared held/reg directory: recorded cleanly
    (parent_run set, 40 memory rows imported); promote of the inert candidate
    REJECTED margin_not_met with the rollback ref standing (the recorded
    scale-invariance), exit 1 (fail-closed).
- TEETH: 7 neutralizations (T1 delta classification, T2 by-unit join, T3
  four-arm condition 2, T4 evaluate-time refusal, T5 margin floor, T6
  provisional floor, T7 rolled_back check) → 14 promote tests RED + 3 p9 legs
  RED (leg 2 green, expected), each attributable 1:1; RESTORED byte-identically
  (sha256 ledger above); post-restore sweep p9 4/4, promote 49/49,
  candidate_only 1/1, arch 7/7 — all green.
- Scratch world removed; .gauntlet/ listing proof; repository residue re-checked
  with git status (untracked = the builder's task work + .agents/reports/task23/).
