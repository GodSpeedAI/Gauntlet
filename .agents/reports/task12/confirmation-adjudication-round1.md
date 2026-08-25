# Task 12 — confirmation round 1 adjudication (orchestrator, 2026-08-25)

Source: `final-confirmation-round1.md` — fresh blind critic, **NOT CONFIRM**
(1 MAJOR + 2 MINOR). The critic's declared bound, executed evidence, and
"what held up" section were checked against the tree personally before
adjudication; every line reference below was re-verified. The
reachable-through-shipped-surface question was asked of each finding, not
assumed from the finding's prose.

## Finding 1 (MAJOR) — AK-052's critic-fan-out leg — **FIX NOW (slice F4)**

**Verified facts (re-derived against the tree):**

- Spec §10.4 verbatim: "Critic fan-out is bounded by `review.max_critics`
  **and by the budget meter**; a fan-out that would breach the recovery
  reserve is truncated in dimension-priority order, and the truncation is
  recorded as `unavailable_evidence`, not silently dropped." §17.4 verbatim:
  "`BudgetMeter` is consulted before every dispatch, not after." §10.4's own
  dispatch diagram puts "fan-out critics by dimension (ReviewMode) —
  admission × N" inside the dispatched-work the budget governs.
- AK-052's claim text is §17.4's first sentence, and the plan's Task-12
  step 3 repeats it ("`would_breach_reserve` consulted before **every**
  dispatch").
- The artifact's own module doc (`compare/review.rs:32-37`) records the
  deferral with **Task 12 as the composition owner**; Task-10's
  `builder-round1.md` recorded the same ("full `BudgetMeter` wiring
  remains Task 12"). The deferral was never entered in the standing ledger
  (`CURRENT_STATUS.md:530+` — searched, no entry), which is why the
  pre-implementation adjudication (`residual-adjudication.md`: 9 composed,
  3 later-scope) never dispositioned it. That omission is itself a process
  finding: a deferral recorded only in a closed task's own reports does not
  survive into the standing ledger. The closure record will carry it.
- Reachability confirmed: the controller drives review every iteration
  (`controller.rs:2123-2128`, `ModelDispatchBudget::new(
  self.comparison.model_dispatches)`); `ComparisonDeps` (`:200-207`) has no
  meter field; p4 sets `model_dispatches: 0` (`p4_corrective_loop.rs:1036`)
  so the proofs exercise only the honest truncation arm — one configuration
  value moves the seam from truncation to unmetered dispatch. The leg is
  composed and config-reachable; it is not fixture-only.

**Disposition: fix now.** The claim is Task-12's own AK unit; the spec
names the exact mechanism; the fix is bounded (review admission consult +
deps plumbing + controller call-site wiring + composed fail-first tests);
no assertion, gate, or frozen surface needs to move. **Bounded out** of
the fix, recorded as later-scope below: payment unification (critic token
costs booking into the run's `budget_spent`).

## Finding 2 (MINOR) — `mint` is a public trait method — **doc-truth now;
type-level seal later-scope**

The port doc (`budget_meter.rs:76-78`) says "arbitrary code cannot forge
the parallelism-accounting token" — as a public trait method, any holder
of a `dyn BudgetMeter` can call `mint` and then `charge` to
saturating-decrement `outstanding` for a slot never held (adapter
`lib.rs:222`), inflating usable parallelism past `max_parallel`. The
recovery reserve itself is not bypassable (the charge still passes the
domain gate) — the critic's own severity (MINOR) is right.

**Disposition:** correct the overstated doc sentence NOW (slice F4): the
C9 guarantee is "construction requires a meter instance — the value is
authority-minted inside the trait, not constructible from raw parts" plus
the composition-root obligation (meters are not handed to callers the root
does not trust). A type-level seal is a ports-revision design question
(Rust cannot distinguish an implementor's `self.mint` call from any other
caller's on a public trait method without restructuring the trait) —
**later-scope, trigger: the next `BudgetMeter` ports revision** (Task 13
P7 resume surface or Task 15 real runner, whichever touches the port
first). No shipped path exploits it today (grep-verified by the critic,
re-verified: no production caller of `mint` outside `reserve`).

## Finding 3 (MINOR) — §21.3 residue: bare `Err` mid-run — **later-scope,
existing deferral extended**

Two loci: the answer-key read/parse failure inside the loop
(`strict_units`, `controller.rs:2810-2819`) and a runner poll transport
error (`:1698`) both end the run with a bare `ControlError` — no terminal,
no close-out checkpoint — where §21.3 prescribes stop-dispatch-keep-alive
and §11.5 row 2 prescribes backoff-retry.

**Disposition: later-scope, unchanged.** The answer-key half IS the
deferral `residual-readjudication.md` already opened ("Answer-key READ
failure inside the loop is a hard `ControlError::Reference` … Trigger:
Task 13 recovery composition, or the next §21.3 degradation-policy
touch"). The poll-error half joins the same deferral family: choosing what
the loop owes on non-integrity mid-run failures is exactly the §21.3
degradation-policy decision this round has twice declined to make
unforced; the crash semantics are pinned by intent
(`a_crash_after_a_settlement_never_re_runs_settled_work`) and recovery is
durable through the top-of-iteration checkpoint (C8.2). The closure
record's residual section will name BOTH loci under the one trigger.

## Consequent work

Correction slice **F4** per `correction-brief-f4.md` (finding 1's gate
wiring + finding 2's doc-truth), then full gates, then confirmation
round 2 (fresh blind critic, same brief).

## What this round's confirmation established (recorded for the outcome)

AK-037, AK-038, AK-039, AK-051 CONFIRMED composed through the production
path; AK-052 confirmed on the builder leg, refuted on the critic leg until
F4 lands. The proofs were judged honest compositions, not self-grading
fixtures. This is the third instance this round of the recurring shape: a
claim's composed surface is narrower than its sentence (cf. C7's
unconstructible attack, L4's absent cohort surface) — but the FIRST
instance where the gap was real rather than vacuous, and it was found by
the confirmation layer doing exactly what the layer exists to do.
