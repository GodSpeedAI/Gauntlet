# Task 22 — COR-1 correction brief (builder)

Tree: /home/sprime01/projects/gauntlet, task22 work UNCOMMITTED in the
working tree (do not commit; the orchestrator owns integration). The
falsifier's round 1 (NOT_CONFIRM: 1 blocking / 3 major) and the design
critic's round 1 (CONFIRM: 0/0/4m/4i) are at:

- `.agents/reports/task22/falsifier-round1.md` (executed reproductions
  B2a, B3a/B3b, B4, B5 — your acceptance targets)
- `.agents/reports/task22/critic-design.md` (RES-01 == F-1, RES-02 ==
  F-2, RES-04 == F-3, RES-06 == F-4)

Read both, plus the charter `.agents/reports/task22/charter.md` D1–D11.
The corrections below are BINDING; deviations get recorded, not
smuggled. Every correction lands with a regression test built from the
falsifier's exact reproduction, RED-checked against the current tree
where practical (test first, confirm it fails for the named reason,
then fix).

## C1 (F-2, BLOCKING) — windows decide on substrate chronology, never import order

Root cause: the outcome/settlement windows order by ledger append order
(= `DevelopmentalImport::import_run` call order); importing newest-first
inverted the failure-rate halves and PROMOTED a genuinely deteriorating
trajectory (B3b: same honest substrate, in-order refuses
`failure_rate_increased`, newest-first promotes with
`earlier=(1,2)/later=(0,2)`).

Fix (binding):
- `ObservationRecord` (ports) gains an OPTIONAL substrate occurrence
  fact, `occurred_at` (EpochSeconds) — distinct from `at` (the recording
  instant, which stays excluded from every decision). Codec round-trips
  it; `facts_agree` includes it (it is deterministic from the substrate,
  so idempotent re-import stays a no-op).
- The import populates `occurred_at` on every Attempt/Failure
  observation from the verification record's `produced_at`, and on every
  Settlement observation from its MATCHING verification record's
  `produced_at` (the import already finds the match). Payment,
  correction, recovery rows: not decided on — leave unset.
- `PromotionService::evaluate` orders the outcome window and the
  settlement window by `(occurred_at, ledger seq)` — the seq tiebreak is
  the stable read-back order — BEFORE any window logic. ANY observation
  in a decided window lacking `occurred_at` makes that window
  UNDECIDABLE: typed refusal (`InsufficientOutcomeEvidence` /
  `InsufficientSettlementEvidence` class), never a pass. Fail-closed.
- Module docs state the rule: windows decide on substrate chronology.
- REGRESSION TEST (ship it): B3b verbatim — honest deteriorating
  substrate (r1–r3 PASS settlements over three units + newest r4 FAIL
  record + a recovery), imported in order 4,3,2,1, must produce the SAME
  verdict as in-order import (`failure_rate_increased` recorded,
  `Provisional`, never `Promoted`). Assert the recorded halves in the
  refusal are occurrence-ordered.

## C2 (F-1 MAJOR + I-2) — settlement windows refuse fact-sparseness; witnesses are complete

Root cause: `settled_with_real_variation` credits axis-SET inequality —
a sparse row (no content axes) "varies" from a full row; and
`promotion_evidence`'s witness `filter_map` silently dropped sparse rows
while `settlements_observed` counted them (B2a promoted; 1 witness for 3
settlements).

Fix (binding):
- `evaluate` pre-validates the settlement window: EVERY settlement
  observation must carry the full fact set — unit, reference_digest,
  evidence_kind, evidence_locator, instrument determinism,
  orchestration_cost, verification ref, evidence ref. Any row missing
  any of them → ONE typed refusal naming the missing class (new
  `PromotionRefusal::InsufficientSettlementEvidence` variant or the
  equivalent — builder picks the exact spelling; the invariant is:
  sparse rows make the window undecidable, they never pass, and they
  never credit variation).
- With uniform facts, the variation rule decides over the (now
  comparable) axes; keep the any-content-axis-differs rule.
- `promotion_evidence`: every settlement observation yields a witness —
  no silent drops; `settlements_observed == witnesses.len()` becomes a
  structural invariant with a test.
- REGRESSION TESTS: B2a verbatim (1 full + 2 sparse settlement rows +
  attempts + recovery) MUST refuse (never promote); B2b (3 all-sparse
  identical) still refuses `insufficient_variation` or the new
  insufficient-evidence rule — assert whichever fires, both are
  acceptable refusals.

## C3 (F-3 MAJOR) — elevating transitions are log-rooted earned writes

Executed hole (B4): a fabricated `TransitionReason::Promoted(evidence)`
in a legal `StatusTransition` applied directly through
`apply_transition` — the forged status stood "as if earned".

Fix (binding):
- PORT OBLIGATION (documented on the trait + enforced by the shared
  suite, so every adapter carries it): `apply_transition` refuses a
  transition whose reason is `TransitionReason::Promoted(evidence)`
  unless EVERY witness in `evidence.settlements` resolves, in the
  ledger's OWN observation log, to a Settlement observation of the same
  capability with matching (run, unit, verification, evidence) facts.
  Typed new error (e.g. `UnrootedPromotion`). Nothing about the refusal
  is §19.2 logic — it is purely "the promotion's witnesses must be
  memories that exist".
- Implement in the reference green stub AND the sqlite adapter; the
  suite grows one case run on all legs (fabricated evidence naming an
  unrooted witness → typed refusal, no transition row).
- The honest service path keeps promoting (its evidence is derived FROM
  the log) — leg 3 of p10 must stay green unmodified.
- RECORD (module docs + closure note, not code): log-rooting does not
  defend against a writer who forges the log itself — the developmental
  log is append-only and auditable by design; the sealed-mint
  alternative (an authority type with no public constructor for
  elevating reasons, house pattern `AdmittedSettlement`) is a recorded
  ports-revision candidate (same class as `BudgetMeter::mint`'s recorded
  candidate in `crates/gauntlet-ports/src/budget_meter.rs:124-132`).
  The composition root hands ledgers only to trusted code.
- REGRESSION TEST: B4 verbatim (fabricated full-satisfaction evidence
  over a one-observation ledger) → typed `UnrootedPromotion`, register
  NOT elevated, no transition row appended.

## C4 (F-4 MAJOR) — recovery records are rooted to a durable run and a non-zero completion

Executed hole (B5): `record_recovery` minted condition-3 credit for a
run that exists nowhere.

Fix (binding):
- `DevelopmentalImport::record_recovery` gains the completion receipt:
  parameter `completed: usize` — the value §18.3
  `complete_pending_settlements` returned — refused typed
  (`RecoveryNotEarned`) when `completed == 0`; AND it verifies
  `state.load_run(run_id)` is `Some` before appending (refused typed
  when the run is not durable). Both refusals append nothing.
- Update the p10 legs and module tests at the call sites to pass the
  honest completion count (their flows already complete 1 row).
- RECORD (docs, same class as C3's boundary): the count is
  caller-supplied; the production caller is the recovery-completion
  call site; deeper sealing (a typed completion receipt minted by the
  store) is a ports-revision candidate.
- REGRESSION TEST: B5 verbatim — `record_recovery` on a non-durable run
  → typed refusal, no row, and the evaluation still refuses
  `no_recovery_observed`.

## C5 (I-1) — teeth-declaration wording honesty (docs only)

p10 module doc: the leg-1/leg-2 declarations' "remove it and the run
promotes" wording overstates — defense in depth is real (A1/A2 executed:
removing one condition still refuses). Reword to the true mechanism:
"removing the condition leaves the refusal to the remaining conditions;
the named rule-presence assertion is the tooth that turns RED." No
assertion changes.

## Discipline

- Build protocol: `flock /tmp/gauntlet-build.lock sh -c '<cmd>;
  sccache --stop-server >/dev/null 2>&1; true'`; package-scoped gates
  first (`just check-package`, `just test-package`); the plan gates
  (`cargo nextest run --test p10_no_false_learning -p gauntlet-cli`,
  `cargo nextest run -p gauntlet-app memory::`) plus
  `just boundaries` must be green before you report.
- Write `.agents/reports/task22/cor1-progress.md` as you go; finish with
  `cor1-report.md` (files + sha256s + verbatim gate tails + any
  recorded deviations).
- Never touch the ruler files; `just ruler` at start and end. Never
  commit or stage. Do not spawn sub-agents. Zero new dependencies.
