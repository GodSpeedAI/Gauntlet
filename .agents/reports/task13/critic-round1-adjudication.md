# Task 13 — critic round 1 adjudication (orchestrator, 2026-08-27)

Source: `critic-round1.md` (spec critic + falsifier, blind, landed
surfaces). Prereg P2 fired for the third time this task (the R1-class seam,
now at the completion path). Adjudication against spec, plan scope, tree.

## FIX NOW — correction round 2 (COR-R2)

**F-1 → COR2-1 (the round's core). Two guards, one per layer:**
(a) ADMISSION: `SettlementCommitAuthority::admit` refuses when the (run,
unit) already has ANY durable settlement row whose key differs from the
commit's — one settlement per unit per run; the SAME-key case keeps its
idempotent-success semantics (§16.3 replay design untouched). Fail-first:
the falsifier's exact sequence (settled A re-attempted via a fresh honest
PASS → second event + row today) must refuse with NO durable write after
the fix. (b) COMPLETION PATH: the controller's per-completion loop only
processes attempted units that intersect the flight's DISPATCHED unit set
— a builder-declared attempted list naming anything else is recorded as a
pinned governance record (typed, not silent) and never verified/settled.
Fail-first: attempted=["A"] during a flight on B → A untouched, no
production at A's locator, the mismatch recorded.

**m-3 → COR2-2. Operator identity is required for every Operator closure,
whatever the unit's authority** (§20.2 "operator identity logged on every
action"; the (Critic, _) => true arm narrows). Fail-first: Operator{
identity: None } closing a critic unit refuses.

**m-5 → COR2-3. `settle_operator_action` filters records by run too**
(the COR-5 comparison, mirrored; cheap consistency).

**F-5 → COR2-4. The sqlite inherent `append_record` VerificationRecord
bypass closes**: route the verification-record row through the same
test-gating idiom as `apply_settlement_row` (or make the RecordKind
unreachable from the inherent surface). Fail-first: the concrete-type
forgery sequence no longer compiles/writes.

**F-6 → COR2-5. Replay divergence detection**: the anchored replay, on a
same-key event whose fields diverge from the existing row, surfaces a
typed refusal (never silent OR-IGNORE); same-key-same-fields stays the
idempotent no-op. Fail-first: a divergent duplicate event in the log (raw
injection) makes replay loud.

**m-10 → COR2-6. Stale precondition doc trued** (the layer split is
landed reality now; doc-only).

## LATER-SCOPE (recorded with triggers)

- **M-1 (§16.1 run-level minimum_reliability unconsumed)**: the frozen
  reference's own field is §19.2 promotion prose ("minimum N settlements
  under recorded variation"), not the §5.3.1 example's machine-countable
  shape — a typed interpretation does not exist for THIS ruler, and I5
  forbids settlement gates reading model/free prose. Disposition:
  reference-candidates entry (the ruler's minimum_reliability cannot be
  mechanically consumed as §16.1's run-level condition requires — spec's
  §5.3.1 example vs the frozen reference's shape) + deferral trigger: the
  first reference revision OR Task 20's promotion surface, whichever
  gives the field a typed meaning. NOT silently dropped; run-settled
  today requires all-units + boundary re-verify (m-8's stricter shape,
  conservative direction).
- **M-2/F-2 (AK-062 positive half)**: plan-scoped Task 21 (the plan's own
  done-when says "AK-062 partially"); the no-bypass half landed and
  tested; the deferral is documented in-code. Unchanged.
- **M-3 (p11/g10 locators)**: B4 Items 2/3 — in flight; the
  reference-candidates drafts already account for them.
- **F-3 (admit→commit TOCTOU)**: no concurrent settlers exist (single-
  threaded drive; mutex'd connection); COR2-1's one-per-unit admission
  also collapses the exploit's value. Trigger: real concurrent
  composition (Task 15/20).
- **F-4 (transient non-in-flight edit+revert window)**: no settlement
  impact (frozen-grounded verify/settle); planning-window hardening
  trigger: next §11.4/reread touch or Task 20.
- **F-7** = early-probe m3 (trigger: non-content-addressed evidence
  adapter) — unchanged.
- **m-1** (resume order: re-verify before completion — documented
  deviation, keep), **m-2** (anchor > vs >=; record the
  exact-boundary test as a nice-to-have in COR2-1's tests if cheap),
  **m-4/m-6/m-7/m-8/m-9/m-11** (record in the closure residual list).

## REJECTED

None — all findings either fixed or trigger-recorded.
