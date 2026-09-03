# Task 22 — Falsifier Round 2 (fresh blind executing dispatch)

2026-09-03. Round 2 re-executed every round-1 attack family against the
COR-1-corrected tree (C1–C5), over the REAL sqlite ledger + REAL state
store (`EmbeddedCapabilityLedger` + `EmbeddedStateStore` on scratch
database files; earned-path settlement/verification substrate), plus
new-hole hunts and correction-teeth mutations. Blindness held: no
task22 report file read except `charter.md`; all findings derive from
code, spec (§19.1–19.2 quoted in the charter), AK-055, and executed
commands. The dead predecessor's untracked scratch
(`crates/gauntlet-cli/tests/falsifier_round2_scratch.rs`, 1403 lines,
never run) was reviewed, repaired (one judgment bug: it panicked on an
outcome its own comment deemed acceptable; one missing locator fact in a
helper), extended with 3 new attacks (single-axis R-B2a variants ×9,
NH-6 byte-flip, NH-7 post-refusal idempotency), EXECUTED (16/16), and
judged independently — its comments were never trusted; verdicts come
from the executed assertions and captured stdout.

## Executed attacks (one line each)

| id | family | outcome |
|----|--------|---------|
| R-B3b (5 orders: 4321, 1234, 2341, 3142, 2413) | honest deteriorating substrate (3 PASS settlements + newest FAIL, rooted §18.3 recovery), real import, real sqlite | REFUSED `failure_rate_increased{0,2,1,2}` in EVERY order, occurrence-ordered halves, Provisional — import-order-invariant (scratch `rb3b_…`) |
| R-B3b window-level | same mechanism, MemoryLedger module test | PASS (`windows_decide_on_substrate_chronology_never_append_order`) |
| R-B2a core | 1 full + 2 sparse settlement rows + attempts + recovery, real sqlite | REFUSED `insufficient_settlement_evidence` naming unit, reference_digest, evidence_kind, evidence_locator; Provisional; never promotes |
| R-B2a ×9 single-axis | one row missing ONLY locator / instrument / burden / evidence / unit / reference / kind / verification / occurred_at | each REFUSED with exactly that one class named in `insufficient_settlement_evidence`; never promotes; absence never credited |
| R-B2a all-sparse-identical | module test b2b | REFUSED (evidence or variation rule); never promotes |
| R-B2a A,A,B judgment variant | 2 identical units + 1 differing | PROMOTED with sound evidence (witnesses==observed, content-only axes) — recorded as F2-6, charter-D4-conformant |
| R-B4 unrooted | fabricated PromotionEvidence naming a NON-existent witness via `apply_transition`, real sqlite | `UnrootedPromotion`, status stays Observed, ZERO transition rows |
| R-B4 boundary | forged evidence naming ONE REAL settlement witness + forged recovery/attempts via ungated append | elevation APPLIES → Promoted; honest re-evaluation then refuses `AlreadyPromoted` — documented boundary, F2-4 (INFORMATIONAL) |
| R-B4 scoping (NH-3) | witness resolves only to a DIFFERENT capability's real settlement | `UnrootedPromotion` — capability scoping of rooting holds |
| R-B5 | `record_recovery` with completed=0 (durable run) and completed=1 (non-durable run), real sqlite | typed `RecoveryNotEarned{0}` / `RunNotDurable`, nothing appended, evaluation refuses `no_recovery_observed` before AND after both mints |
| NH-1 | occurred_at collision: PASS + FAIL with identical produced_at, opposite landing orders | verdicts FLIP: fail-first `[]` (= Promoted), pass-first `[failure_rate_increased]` — F2-1 (MINOR) |
| NH-2 | occurred_at forged to 0 / u64::MAX / u64::MAX-1 through the ungated append | genuinely-rising failure rate PROMOTES (`[]` refusals) — F2-5 (INFORMATIONAL) |
| NH-4 | Provisional→Provisional re-record with `TransitionReason::Refused(vec![])` | composes AND applies; durable zero-reason refusal record the honest service can never produce — F2-2 (MINOR) |
| NH-5 | 3 settlements, ONE run, identical content | REFUSED `insufficient_variation` (run-id is not an axis; the AK-055 trap stays closed) |
| NH-5b | 3 settlements, ONE run, 3 distinct units | PROMOTED (witnesses=3, content-only axes) — F2-3 (MINOR) |
| NH-6 | sqlite payload byte-flip (`"outcome":"PASS"`→`"QASS"`, in place) | `observations()` → typed `Corrupt`; `evaluate()` → `Ledger(Corrupt)` — fails closed |
| NH-7 | evaluate() immediately after a refusal, real sqlite | same typed refusal, Provisional both times, 2 durable history rows — idempotent refusal shape |
| Baseline re-run on restored tree | p10_no_false_learning 4/4; gauntlet-app memory:: 33/33; scratch 16/16 | ALL GREEN |

## Findings

No blocking finding. No major finding.

- **F2-1 (MINOR) — occurred_at ties decide the verdict by append order, and the flip is Promote-vs-Refuse.** Repro: scratch `nh1_occurred_at_collision_makes_the_verdict_append_order_dependent` — three full settlements + recovery, then a PASS and a FAIL outcome row with IDENTICAL `occurred_at`; FAIL-landed-first evaluates to zero refusals (PROMOTED), PASS-first evaluates to `failure_rate_increased` (REFUSED). The tiebreak is the ledger seq (= append order), so under a same-second produced_at tie the module doc's categorical claim "a verdict must be a function of the recorded facts, not of the call order" does not hold. The `(occurred_at, ledger seq)` tuple IS documented (promotion.rs module docs; port docs), which is why this is minor, not major — but the categorical order-invariance claim overshoots, and EpochSeconds granularity makes same-second decisions across parallel runs realistic. Suggested direction (owner's call): refuse ties inside the decisive pair, or treat a tie-influenced window as undecidable.
- **F2-2 (MINOR) — an empty refusal set composes a durable Provisional re-record.** Repro: scratch `nh4_empty_refusal_re_record_laundering` — `StatusTransition::new(cap, Provisional, Provisional, TransitionReason::Refused(vec![]), …)` composes (ReasonMismatch only checks kind-vs-target) and applies over real sqlite; the history gains a "refused with zero reasons" record, which the honest service can never emit (an all-conditions-pass evaluation promotes instead). Non-elevating — no §19.2 breach — but it pollutes the durable decision record an auditor or task23 consumer reads. Fix direction: reject `Refused(vec![])` at composition.
- **F2-3 (MINOR) — one successful run can promote: settlement-count is observation-counted, not run-counted.** Repro: scratch `nh5b_single_run_distinct_units_verdict_recorded` — three settlements from a SINGLE run over three distinct units + recovery + passing attempts PROMOTE (witnesses=3, content-only axes; reachable through the real import, which projects one row per settlement). This is exactly charter D4's mechanical definition (≥N settlement-class observations), so it is charter-conformant — but it sits in tension with §19.1's verbatim "One successful run is not capability." Recorded for the spec owner as a CandidateRevision-class question (e.g. require settlements from ≥2 distinct runs); NOT a code defect against this task's charter.
- **F2-4 (INFORMATIONAL) — the R-B4 boundary (forged evidence naming a real witness elevates) is a DOCUMENTED boundary.** The rooting check verifies witness existence+identity in the ledger's own log, deliberately not §19.2 counts; the port doc records verbatim: "log-rooting does not defend against a writer who forges the observation log itself … The composition root hands ledgers only to trusted code," with the sealed-mint alternative noted as a ports-revision candidate. The fabricated NON-existent-witness vector is fully closed (UnrootedPromotion, no row).
- **F2-5 (INFORMATIONAL) — an occurrence lie is undetectable in-ledger and inverts the failure-rate condition.** Repro: scratch `nh2_forged_occurred_at_extremes_invert_the_chronology` — FAIL stamped `occurred_at=0`, PASSes stamped `u64::MAX`/`u64::MAX-1` through the ungated append: the rising rate PROMOTES. C1's chronology is only as honest as the appended fact; §19.1's "Raw. Immediate, append-only" ungated append and the trusted-writer boundary (see F2-4) cover this. No in-ledger instrument can detect it; cross-checking against the substrate would be a new mechanism (out of scope).
- **F2-6 (INFORMATIONAL) — the variation rule is any-differs.** 2-of-3 settlements on the same unit + 1 differing promotes (scratch `rb2a_var_two_identical_plus_one_differing…`). Charter D4's wording ("must differ on at least one CONTENT axis") admits this reading; the plan's Done-when only forbids all-identical. Spec-vocabulary note only.

## Prior-hole closure table

| hole | was | status |
|------|-----|--------|
| B3b (import-order-dependent verdict on deteriorating substrate) | BLOCKING | **CLOSED** — occurrence-ordered windows; verdict identical across all 5 tested import orders over the real sqlite ledger+state store, exact `FailureRateIncreased{0,2,1,2}` halves (residual: F2-1, ties only) |
| B2a (sparse settlement rows credited variation-by-absence and promoted) | MAJOR | **CLOSED** — fact-uniformity pre-validation refuses the window by name; verified for 1-full+2-sparse and for each of the 9 single-missing-fact classes; never promotes |
| B4 (fabricated PromotionEvidence through apply_transition) | MAJOR | **CLOSED** for the fabricated/foreign-witness vectors (UnrootedPromotion, nothing written, capability-scoped); the real-witness boundary is documented (F2-4, INFORMATIONAL) |
| B5 (recovery credit minted from nothing) | MAJOR | **CLOSED** — `RecoveryNotEarned` (completed=0) and `RunNotDurable` (ghost run) typed refusals appending nothing; the evaluation still refuses `no_recovery_observed` after both mints |

## Teeth (correction mechanisms neutralized one at a time; detectors observed RED; restore verified by sha256)

| mechanism | neutralization | detectors observed RED | restored sha256 |
|-----------|----------------|------------------------|-----------------|
| C1 occurred_at ordering (`ordered_by_occurrence` sort removed) | promotion.rs | `windows_decide_on_substrate_chronology_never_append_order`; `the_verdict_is_a_function_of_recorded_facts_never_import_order_b3b`; scratch rb3b (newest-first import PROMOTED the rising rate) | `d5db021f…cdbb` |
| C2 fact-uniformity pre-validation (`settlement_window_facts` check removed) | promotion.rs | module b2a, `an_undecidable_erosion_window_refuses`, `an_undecidable_burden_window_refuses`; scratch rb2a core, single-axis ×9, locator-only (sparse windows reached the promote path) | `d5db021f…cdbb` |
| C3 rooting predicate (`promotion_witness_is_rooted` → `true`) | ports capability_ledger.rs | ports `capability_ledger_green_is_clean`; sqlite `sqlite_capability_ledger_passes_the_shared_contract_suite`; scratch rb4 (fabricated witness elevated) and nh3 (foreign-capability witness rooted) | `886e8842…9090` |
| C4a `completed>0` refusal removed | developmental.rs | `a_recovery_with_zero_completions_is_not_earned`; scratch rb5 ("expected RecoveryNotEarned, got: Ok(())") | `71c0037d…cf96` |
| C4b `RunNotDurable` refusal removed | developmental.rs | both `…_b5` module tests; scratch rb5 ("expected RunNotDurable, got: Ok(())") | `71c0037d…cf96` |

Every correction's removal is caught by at least two named detectors — no toothless correction. Pre-mutation sha256s recorded; all three files restored byte-identically (sha256 match verified above); p10 4/4 and memory:: 33/33 re-run GREEN after restoration.

## Restoration declaration

- `crates/gauntlet-app/src/memory/promotion.rs`, `crates/gauntlet-ports/src/capability_ledger.rs`, `crates/gauntlet-app/src/memory/developmental.rs`: mutated five times for teeth, each restore verified byte-identical by sha256 (values above).
- `crates/gauntlet-cli/tests/falsifier_round2_scratch.rs`: predecessor's untracked scratch, repaired and executed, **DELETED** per its own header after execution. Its retained value lives in this report and the module/contract detectors it exercised.
- No other file touched. `.wayfinder/*` and `.gauntlet/reference.lock` untouched; nothing staged or committed; no sub-agents used.
- All cargo/nextest invocations ran under `flock /tmp/gauntlet-build.lock` with `sccache --stop-server` inside the same command; targeted runs only.
- Final `git status --porcelain` equals the pre-existing task22 delta (recorded before any mutation: `.agents/reports/task22/`, `.agents/CURRENT_STATUS.yml`, `.agents/reports/reference-candidates.md`, `docs/reference/configuration.md`, the task22 source/test files under `crates/`, plus pre-existing `.jolli/*` environment noise) MINUS the deleted scratch PLUS this report. AK-055's locator divergence was confirmed already recorded in `.agents/reports/reference-candidates.md` (2026-09-03 entry). `just ruler` PASS after cleanup.

VERDICT: CONFIRM — 0 blocking / 0 major / 3 minor / 3 informational
