# Task 13 — inherited-deferral adjudication (orchestrator, 2026-08-25, pre-implementation)

Scope rule (CURRENT_STATUS): a deferred finding upgrades to blocking the moment
a task walks into its trigger. Each item below is adjudicated against the
current tree (recon-code.md), not against prose memory — the Task-12 C7 lesson
(reachability must be checked against the tree, not assumed from prose).

## COMPOSE into Task 13 (blocking)

**A1 — Task-12 L3 (settlement crash-window replay) + Task-3 store-binding +
Task-6 §16.3 swap discrimination.** One composition: the controller's PASS leg
gains the durable §16.3 sequence (event append → row → in-memory advance) and
`resume_run` gains §18.3 step-1 completion. The swap-deferral's designated
closure vehicle is p7's `commit_order` leg with abort injection (crash_helper
`settle` script already has the four abort points). The Task-3 "store-binding"
deferral resolves by the service composing `commit_settlement` behind a
governed admission — the domain type stays store-blind.

**A2 — Task-12 L2, P7-load-bearing legs: replay anchored at the checkpoint
offset** (Task-6 MINOR-4) **+ mid-resume settlement completion** (A1) **+
run-identity/evidence preservation on resume.** `complete_pending_settlements`
replay start moves from offset 0 to the §18.3 checkpoint offset (superset-safe
today, but §18.3 names the anchor and P7 exercises it). L2's remaining legs —
workspace sweep (§18.3 step 4), preflight re-run (step 5), retry-ledger
persistence beyond attempts — are adjudicated per-leg in B2: compose any leg
the composed resume path can express without a ports-revision shape change;
anything else stays deferred with a re-grounded trigger (the §18.2 field set
is spec-fixed; the retry-ledger half is explicitly a ports-revision question).

**A3 — Task-6 R1/R3 settlement-side coercions.** `settled_at` becomes
event-derived (the durable event's `at` is the authority; the commit's
caller-supplied `at` may construct the event but the row must take the stored
event timestamp — and `verify_settlement_event_at` must compare against the
stored `at`, closing the unchecked-seam half). `settlements_of` offset decode
becomes strict (`Corrupt`, never `unwrap_or(0)`). Fail-first evidence
required for both.

**A4 — Task-7 M4 settlement half (AK-024's "immediately before settlement").**
The settle path gains an explicit `verify_unchanged` consult immediately
before the §16.3 commit (the boundary tripwire already brackets the iteration;
this is the settle-time leg the spec names separately). Both digests persist
as evidence on mismatch per the deferral's text (the typed error already
carries them — persist them into the run's evidence at the abort path).

**A5 — Task-11 production-row crash-window replay (EvidenceProduced).** Same
recovery-composition class as L3: resume gains completion of half-applied
evidence-production rows (event appended, row missing). The replay helper
follows the settlement pattern; idempotence key is the existing
`(run_id, evidence_ref, locator)` production identity.

**A6 — Task-12 §21.3 non-integrity mid-run failures.** Trigger fired (Task 13
recovery composition). Answer-key read failure mid-loop moves from hard
`ControlError::Reference` to the §21.3 disposition (stop new dispatch, keep
the process alive, keep reconciliation running) as a typed non-terminal
degradation; runner poll transport error routes through the §11.5
disposition table instead of `Err`-propagating (the existing in-crate test
pinning crash semantics is updated to pin the disposition, with resume-based
recovery preserved). If either leg proves to require redesign beyond
disposition routing, record and defer with evidence rather than widening.

**A7 — §16.1 conditions 3/4 at the service.** Authority chain (cond 3:
critic→auto; lead_agent→orchestrator+recorded rationale; human→operator
action+recorded identity — AK-043's tier makes this live for any human-
authority unit) and blocking-residual check (cond 4: no blocking residual
references the unit) compose into the service. `Unit::settle`
authority-blindness (Task-12 L1's observation) is thereby addressed at the
service layer; L1's scheduler-side authority gate itself remains Task 14
(re-grounded, not consumed).

## NOT COMPOSED (with re-grounded triggers)

**N1 — Task-7 m1 `ReferenceVersion.evidence_ref`.** The settlement commit
binds `reference_digest` (B.6 field set); the version's frozen-copy ref
already lives durably in the runs row (`reference_evidence_ref`) and the
freeze path pins it. Adding the field to the domain type is a domain/ports
shape change with no load-bearing consumer in Task 13's claims. Trigger
re-grounded to: the ports revision or the first consumer that needs the
linkage in-type.

**N2 — Task-12 MINOR-6 (first-facet evidence binding).** Settlement consumes
the verifier's chosen binding; the coarseness is in the controller's evidence
production, exercised by no shipped proof (multi-attempt units). Composing it
here would widen a green surface mid-task without a fired §16 consequence.
Trigger stays Task 20 (composition exercise) unless a Task-13 critic finding
makes it load-bearing.

**N3 — Task-12 conditional `BudgetMeter::mint` seal.** Task 13 does not touch
the `BudgetMeter` port (resume already restores via `Budget::restore`, Task 12
B3). Trigger not fired; stays "next BudgetMeter ports revision".

**N4 — doctor check 7 / workspace placement.** Recon shows the placement
invariant (answer key outside builder-writable workspaces) is already enforced
surface-wise (Task 5 doctor check 7; containment prefixes; provider
read-only). B3 verifies and pins rather than rebuilds.

## Standing obligations riding along

- **Arch-suite touch ⇒ re-litigation** (standing rule): B3 touches
  gauntlet-arch-tests (new single-writer/no-writer check). Re-litigate on the
  record: E4/F1/F3 (closed Task 4 — confirm still closed), Task-9 B.5a masked
  codec spellings (pinned-as-masking), Task-6 A5.2 adapter-allowlist
  granularity, Task-7 B-F16 scanner shapes. Documented adjudication with
  bounded searches; widen nothing without deliberate inventory edits.
- **Boundary-suite growth** (if any) follows the Task-4/9 discipline: central
  declaration, fail-closed, adversarial review, residuals ledgered.
- **AK-024/AK-023 scope honesty**: the 2026-08-18 reference-candidates entry
  says both stay substrate-level "until plan Tasks 12–13 wire … the settlement
  pre-check". Task 13 wires it — the closure upgrades the recorded scope and
  the entry gains a dated addendum via the outcome record (never edited
  history).
- **AK-062**: service-level no-bypass half only (full operator-action surface
  is Task 21).
- **AK-053**: claim at closure only if the composed resume genuinely proves
  checkpoint sufficiency; otherwise record as substrate-proven, locator
  materialization class.

## Escalation check

AK-043 is `irreversible`/`human`. Rule: irreversible units are never automatic
dispatch candidates — that governs the RUNTIME's treatment of Units at
dispatch; AK-043's evidence here is an instrument-run test report (producer:
instrument), not a human sign-off artifact. The unit's claim is not marked
settled by this task; it moves to provisional (instrument-proven) exactly like
prior irreversible-posture units handled by test evidence. No HITL escalation
required to build; the closure record states the provisional posture.
