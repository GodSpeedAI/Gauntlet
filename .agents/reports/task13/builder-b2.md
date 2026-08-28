# Task 13 / B2 result contract — `settle::` service + controller wiring + resume completion

**Provenance note (orchestrator-written):** the original B2 builder died
mid-Item-1 (16th interruption instance); a finisher completed Phase 1 +
Items 1–5 (17th instance) but died mid-edit before its result contract,
final gates, and a small refactor completion. This contract is written by
the orchestrator from the builders' checkpoint records
(`builder-b2-checkpoints.md`, including both RECOVERED-AND-VERIFIED
sections and the operator-instruction record) after personally healing the
interrupted refactor (DONE-R) and personally running the full gate battery
(DONE-G). The builders' fail-first evidence is theirs, as recorded; nothing
here implies otherwise.

## Delivered

### Phase 1 (finisher) — honest fixture
The inherited RED fixture (frozen-reference evidence digest mismatch — the
three-way digest equality working as designed, biting the fixture's own
wiring) fixed by earning the governed run honestly (patterns reused from
verify::authority's own tests / p12); settle:: 16/16 GREEN, zero warnings,
the dead helper put to use. Recovered-RED honestly labeled; the original
builder's own fail-first evidence recorded as LOST.

### Item 1 (builder + finisher) — the `settle` module
`crates/gauntlet-app/src/settle/mod.rs`: `SettlementService` as the ONLY
settle path — consumes the chain-minted `VerifiedClaim` + its
`SettlementReadiness` token (conditions 1–2, never re-derived), loads the
FROZEN unit from the durable run's digest-verifying projection (not a
caller snapshot), enforces condition 3 (authority chain: critic auto /
lead_agent recorded-rationale / human refuses automatic closure and
escalates), condition 4 (blocking residuals, tracked view AND durable
checkpoint projection, durable-wins divergence typed), condition 5
(settle-time `verify_unchanged` against the provider's file-derived answer;
mutation → both digests pinned as evidence + typed abort; unavailability →
refuse without aborting, §21.3), then record-verification (idempotent) →
`SettlementCommitAuthority` admission → `commit_settlement`
(AlreadyCommitted-same-key = idempotent success) → in-memory advance LAST
(durable offset observable for the commit-order proof). Documented
mappings: B.6 `authority_decision` rides an accompanying pinned evidence
document (`settlement_authority_decision@v1`) — the frozen settlements
schema carries no such column (NO new migration); condition 4's durable
half grounds on the durable checkpoint's unit-state projection (a
residual-row read through a port is a recorded ports-revision gap).

### Item 2 (finisher) — controller PASS-leg wiring
The in-memory-only settle replaced: verify → loop PINS the chain-minted
PASS evidence → service (conditions 3/4/5, durable commit) → in-memory
advance; the settlement append enters the `state.last_offset` water-mark
(C8.1); the settle-time digest consult stops new dispatch before the first
durable settlement and refuses to settle under an unverifiable reference
(§16.1 cond 5 + §21.2). Fail-first: `the_pass_leg_settles_durably_through_
the_service` (RED under the restored old leg, GREEN under the service).
Deviation (recorded in-entry): `a_file_only_unit…`'s old Settled outcome
became unconstructible under §16.1 cond 5 + §21.2 — re-pinned to the
§21.3-honest outcome, resolution completed under Item 4.

### Item 3 (finisher) — resume completion (§18.3 step 1)
`resume_run`: after `state.resume`, before re-planning — the service's
`complete_pending_after_checkpoint` (anchored keyed settlement replay +
evidence-production orphan completion from durable facts, limitations
pinned, nothing fabricated) + §5.3.3 projection reconciliation, EVENT-
ORDERED (only a settlement whose event sits strictly beyond the checkpoint
water-mark overrides a later Open projection — a legitimate §11.5 re-open
stays; pinned by `resume_restores_per_unit_attempts…` which first failed
under the blanket rule). New typed `ControlError::Settlement(SettleError)`.
Fail-first: `a_resume_completes_half_applied_settlements_and_reconciles_
the_projection` (RED "the durably settled unit is never re-dispatched:
[Attempt unit AK-9102…]" → GREEN: zero-budget resume ends Settled, no
prompt).

### Item 4 (finisher) — §21.3 non-integrity dispositions (A6)
Leg 1: answer-key READ failures degrade at every mid-loop read site (plan
boundary, verify leg, §15.1 selection record, drain step-3 re-read) — stop
new dispatch for the iteration, pinned `answer_key_read_failed` record,
reference-breach gap, bounded by rounds, resume-able; parse failures still
propagate. Leg 2: poll transport errors route through §11.5 row 2
(`dispose_transient`: Failed terminal event + pinned transient record +
virtual-time exponential backoff + re-dispatch). Fail-first:
`an_answer_key_read_failure_degrades_the_iteration_and_stays_resumeable`
(RED: the run ERRORED `Reference(Unavailable…)` → GREEN: BudgetExhausted
with pinned gaps, pre-failure settlement standing, file restored → resume
completes without re-dispatch). Deviations (recorded in-entry): the crash
test re-pinned to the DISPOSITION (resume assertions verbatim);
`resume_restores_per_unit_attempts…` rides a never-passing verifier.

### Item 5 (finisher) — AK-062 service-level no-bypass
`settle_operator_action`: no durable PASS → `VerificationNotPass`; durable
PASS but no chain-earned readiness → `OperatorActionRequiresEarnedReadiness`
— the same gate whichever caller hit it (the shared-variant
`From<SettlementAdmissionError>` mapping pinned); FAIL + INCONCLUSIVE
records refuse naming BOTH (AK-005's three-state rule at the operator
seam). Pure test additions, no behavior change (no fail-first applicable).

### Phase R + G (orchestrator, per operator instruction)
Completed the interrupted `SettleError::ReferenceMutated` boxing refactor
(3 stale use sites + 1 test import) mechanically; fixed 4 never-linted
clippy pedantics; fmt the two predecessor-edited files. Gates personally:
ruler PASS; app **323/323**; proofs p4/p6/p12/p2 **31/31**; lint **0**;
boundaries **197/197**; compile zero warnings.

## Files touched (B2 total)

`crates/gauntlet-app/src/settle/mod.rs` (new, ~2.9k lines incl. tests),
`crates/gauntlet-app/src/lib.rs` (`pub mod settle`),
`crates/gauntlet-app/src/control/controller.rs` (PASS leg through the
service; resume completion + projection reconciliation;
`ControlError::Settlement`; §21.3 disposition legs; `escape_json_text`
pub(crate); ReferenceMutated box mapping), `crates/gauntlet-cli/tests/
p4_corrective_loop.rs` (proof-harness adjustment: chain-minted PASS
evidence pinned by the loop — assertion unchanged),
`crates/gauntlet-cli/tests/export_stack.rs` (evidenced golden growth,
landed with B1's earned path).

## Known gaps / honest limits (carried to critics + closure)

- Condition 4's durable half is the checkpoint projection; no typed durable
  residual-row read exists through any port (recorded ports-revision gap).
- The B.6 authority_decision triple rides a pinned evidence document, not a
  settlements column (schema frozen; recorded mapping).
- `complete_pending_evidence_production` recovers only facts durably
  recoverable; unrecoverable orphans are recorded limitations.
- P7/P11/G10 composed proofs are B4's (this contract covers the service +
  wiring layer; the in-crate legs are necessary, not sufficient).
