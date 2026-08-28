# Task 13 — fresh blind confirmation round 1, adjudication (orchestrator, 2026-08-27)

Sources: `final-confirmation-a-spec.md` (Role A, spec critic, static-only —
**VERDICT: CONFIRM**, 0 blocking / 0 major / 5 minors RES-A1..A5) and
`final-confirmation-b-falsifier.md` (Role B, falsifier, executed probes —
**VERDICT: NOT CONFIRM**, 0 CRITICAL / 1 MAJOR F-1 / 2 MINORS F-2, F-3).
Both agents ran under the operator's role charters
(`.agents/agents/{critic,falsifier}.agent.md` — adopted verbatim; transport
general-purpose because this session's Agent tool does not expose typed role
agents; re-probed twice this session: "Agent type not found"). Blindness held
in both reports (one disclosed nuance: source comments narrating prior
corrections — part of the authorized artifact, probes constructed
independently).

## Combined verdict: NOT CONFIRM → REVISE → correction round 3 (COR3)

The brief's confirmation bar is "nothing above MINOR". F-1 is MAJOR with
executed evidence, so round 1 cannot confirm regardless of Role A's CONFIRM.
The two roles CONVERGED on the same seam from opposite directions: Role A's
Observation 2 names the ungated `append_event(SettlementCommitted)` surface
and defers to the falsifier's threat model; Role B demonstrated it.

## F-1 — FIX NOW (COR3). Ungated `SettlementCommitted` append + replay mints a durable settlement with zero conditions

Adjudication reasoning:

1. **Executed, not speculative.** Real `SqliteStateStore` through the public
   `StateStore::append_event` trait method AND through the separate
   `SqliteEventLog` (`EventLog` port) over the same DB file; replay
   (`complete_pending_settlements`, driven by honest `resume_run`) rebuilt
   the settlement row from the forged event. Zero verification records
   before and after. The forged row also poisons the resume projection and
   permanently refuses honest settlement (`UnitAlreadySettled`).
2. **The artifact's own standard convicts it.** This task already gated the
   identical attacker (in-process owner-write forgery) twice on the sibling
   surfaces: COR-1's capability-gated `record_verification` +
   `apply_settlement_row` made test-only, and COR2-4's `append_record`
   refusing `RecordKind::VerificationRecord` "for every caller". The
   `SettlementCommitted` event kind is the third owner-write fact in the
   same trio and the only one left ungated. The asymmetry is the defect,
   not a design choice.
3. **Against "by-design event sourcing" (Role A's framing):** the design
   needs a generic event append for ordinary runtime facts — it does not
   need an ungated append for the authority-minted settlement fact. The
   earned producer (`commit_settlement`, taking `AdmittedSettlement`) has
   its own internal append path. Replay-trust semantics stay untouched:
   the fix gates ENTRY of the forged kind into the log, not replay.
4. **Scope:** AK-040 as written — the settlements table is the canonical
   settlement surface every consumer (admission keys, resume projection,
   export, promotion counting) reads; a public path that mints a row in it
   with none of the five §16.1 conditions is a substance-level break of
   AK-040's guarantee, and it undermines AK-042's lockout wall in
   consequence (honest settlement permanently refused).

Correction shape (bounded — see `correction-brief-r3.md`): refuse
`RunEventKind::SettlementCommitted` on BOTH public append surfaces
(`StateStore::append_event` and the `EventLog` port) with a typed error
naming the earned path; `commit_settlement`'s internal append is the only
producer; shared contract suites pin the refusal (conforming stubs updated);
fail-first = both legs of the falsifier's executed sequence refuse with zero
durable writes. No replay-semantics change. No other event kind is gated
(this round).

## Disposition of the remaining findings (none blocks confirmation)

- **F-2 (MINOR, crash window loses the B.6 authority-decision pin on
  replay-recovery)** — later-scope, trigger: next settle/recovery touch or
  Task 20 resume/completion work. Settlement facts durable; accompanying
  evidence only; narrow window. Recorded in the closure residual list.
- **F-3 (MINOR, `contained_locator_path` lexical-only vs §17.5.2
  canonicalization)** — later-scope, trigger: the §17.5.2/workspace-provider
  write-boundary touch (matches Role A Obs 3's division-of-labor note). Not
  builder-reachable as shipped (locator ruler-frozen; builder writes bytes
  only); operator-level attack.
- **RES-A1 (MINOR, p7 bracket granularity cannot discriminate an
  intra-iteration in-memory-first reorder)** — recorded minor residual. The
  plan's done-when teeth ("reordering the commit steps makes P7 fail") is
  discharged: the store-side reorder is refused by
  `verify_settlement_event_at` cross-validation and the commit-removal tooth
  fails `commit_order` (teeth-b4.md); the intra-iteration shape is
  correct-by-construction (the advance happens in the `Ok` arm after the
  durable offset returns) and is now a recorded residual rather than an
  unproven claim. The orchestrator re-runs the teeth personally at closure.
- **RES-A2 (MINOR, verifier-side instrument failures unwind `start_run` as
  `ControlError` instead of a §11.5/§21.3 disposition row)** — later-scope,
  trigger: next controller disposition touch or Task 20. Recovery is correct
  (nothing settles, resume re-plans).
- **RES-A3 (MINOR, `ReferenceVersion` digest covers answer-key bytes only,
  not manifest/comparison-contract)** — predates Task 13; spec-scope
  question (§9.4/§9.5 freeze scope). Recorded in the closure residual list
  with trigger: reference revision or Task 20's freeze surface. NOT a
  reference-candidates entry (the ruler's AK-024 claim is about "the
  reference digest", which is satisfied for the digest that exists).
- **RES-A4 (MINOR/scope, §18.3 steps 4–5 absent)** — plan does not target
  them in Task 13; AK-053 assessed partially supported (all §18.2 fields
  present, durable-only resume). Feeds the closure record's AK-053 scope
  wording and the deferral ledger (trigger: resume touch / Task 20).
- **RES-A5 (MINOR/phasing, no operator CLI reaches the composed path yet)**
  — known Task-20 phasing; closure note only.
- **Same-class note (falsifier's "not examined" list):** the `checkpoint()`
  forged-row projection is the ungated-trait-call class but is
  planning-layer (not settlement minting); trigger-recorded at the next
  state-store/ports-revision touch, NOT folded into COR3 (bounded rounds).

## Environment notes

- `/tmp` (tmpfs 3.9G) is ~95% full — 3.5G is `/tmp/opencode` (another
  tool's cache; not touched). The falsifier relocated its probe
  `CARGO_TARGET_DIR` to `~/.cache/task13-probe-target` (228M, outside the
  repo). Builders: keep build temps off /tmp where possible; do not raise
  parallelism; do not clean /tmp/opencode.
- Prereg P2 fired a FOURTH time this task (the R1-class ungated-owner-write
  seam, now at the event layer) and P4's class is adjacent (durable-vs-
  admission grounding). Both recorded for task-13.outcome.yaml.

## Next

1. COR3 per `correction-brief-r3.md` (builder dispatch; build lock
   exclusive).
2. Orchestrator gate battery + teeth re-verification.
3. ONE fresh blind confirmation round 2 (new agents, same brief, incremented
   deliverable filenames). On CONFIRM → closure per next_action.
