# Task 23 — COR-1 brief (correction round 1)

You are the COR-1 builder. The blind design critic round 1 returned
NOT_CONFIRM (`.agents/reports/task23/critic-design.md` — read it in
full; it is the source of truth for the findings). The charter is
`.agents/reports/task23/charter.md`; repository law is `AGENTS.md`.
Fix every finding below. **Every correction is RED-first**: for each
item, first add/adjust the test that fails for exactly the found
reason (or, where the item is a fixture/docs record, demonstrate the
wrongness it removes), run it RED, then implement, then GREEN. Never
weaken an assertion to reach green; never disable a test.

## C1 (blocking — critic B1) Incomplete arms are evidence gaps, never sentinels

`TargetMetric::figure` (`crates/gauntlet-app/src/promote/metrics.rs`)
returns `Some(u64::MAX)` when `settled_terminal` is false, and the
gate never reads `settled_terminal` — so (a) an incomplete CONTROL
arm trivially "improves" (`margin_holds(MAX, finite, _)` is true;
condition 8 vacuous) and PROMOTES, and (b) an incomplete TREATMENT
arm durably REJECTS on `MarginNotMet`. Both directions violate the
documented verdict contract and §19.4 ("Insufficient evidence ⟹
provisional"). Also remove the `(0,0)`-fallback path in
`gate.rs` (metric underivable ⇒ gap only, never a FAILED
`MarginNotMet`).

Fix: a non-`settled_terminal` arm makes the condition-7/8 figures
UNDERIVABLE — typed `EvaluationIncomplete` gap naming the arm ⇒
the verdict is Provisional (never promoted, never rejected). The
sentinel disappears (the figure function refuses incomplete arms; no
magic value survives). RED-first: two regression tests reproducing
the critic's empirical probes verbatim in shape — (i) incomplete
control + completing treatment must NOT promote; (ii) no-op delta
over two incomplete arms must adjudicate Provisional (nothing
rejected, reasons recorded).

## C2 (blocking consequence — critic B1/i5) Rebuild the p9 honest-road fixture onto complete arms

Legs 2 and 3 (and leg 1's metric claim) currently derive their
PROMOTED verdicts from the incomplete-control sentinel path — the
proof rests on the defect it exists to exclude. After C1 they would
go red. Rebuild the fixture so BOTH arms reach settled terminal with
genuinely different per-settlement figures, with the improvement
carried by an INERT delta: author the held-out task set baseline
with `budget.max_parallel: 1` and let the candidate's delta raise it
to `2` (parallel dispatch settles more units per iteration —
iterations/settlement genuinely drops between two COMPLETE arms;
payment stays equal). Leg 1 keeps the `review.mode` weakening entry
so it still REJECTS via condition 3 (`settlement_access_weakened`)
while genuinely improving the metric between complete arms — the
strongest honest form of AK-056: throughput improves, the candidate
still loses because verification got weaker. Leg 3 inherits leg 2's
fixture. Fix the leg headers to describe the real mechanism (the
critic's i5). Leg 4 stays (no evaluation ⇒ provisional). All teeth
declarations re-stated to name the mechanisms that actually carry
each leg now.

## C3 (major M1) Admission idempotency must not depend on the clock

`RevisionProposal::facts_agree` compares `admitted_at`, and the
admission service stamps the moving `clock.now()` — replaying the
same document mints the same content-derived id with a different
instant ⇒ `ProposalConflict` with a FALSE message ("different
declared facts"). RED-first: a regression test under a stepping
clock (two appends at different instents, identical document ⇒
idempotent no-op). Fix: admission provenance (`admitted_at`) leaves
the replay-identity comparison (and the conflict message stops
claiming facts differ when they do not).

## C4 (major M2) The isolated-evaluation fact is verified and durable, not conventional

The treatment arm's `parent_run` is set by convention and never
verified: `ExecutionOutcome` carries no parent fact, the harness
never checks it, the `EvaluationRecord` does not record it, the gate
cannot see it — a substituted executor that ignores `parent_run`
produces indistinguishable evaluations, and the p9 leg only asserts
the CLI echoing its own intent. Fix: the harness — which holds the
outcome's state handle — verifies the durable run record of the
treatment arm (`load_run(...).parent_run`) against the declared
parent BEFORE appending the `EvaluationRecord`; the record carries
the verified parent fact; a missing or mismatched parent is an
`EvaluationIncomplete` gap (the evaluation evidence is invalid ⇒
Provisional, re-run properly), never a promotion. RED-first: a test
with an executor that runs the treatment WITHOUT the parent
(isolation not verifiable) ⇒ the harness refuses to record / the
gate adjudicates Provisional; and the honest executor ⇒ recorded +
gates see the fact.

## C5 (major M3) Condition 4 must not fail open on an evidence-store read error

`improve_cmd.rs` converts a rollback-ref store read ERROR into
`None` via `.ok()`; the gate then decides condition 4 "on presence
alone" — a pass. Fix: a store read error aborts `improve promote`
with a typed error BEFORE adjudication (nothing written); only a
successful read drives the condition (`Ok(true)` holds; `Ok(false)`
fails with `RollbackInvalid`). RED-first: an unreadable evidence
store ⇒ the command aborts typed, no decision written.

## C6 (major M4) An infrastructure failure is a typed abort, never a negative verdict

`protected_verdict` maps a capability-ledger read failure to
`ProtectedCapabilityInferior` (FAILED ⇒ Rejected, write-once). Fix:
an infra read failure surfaces as a typed `GateError` (retryable
abort; nothing written; the revision is NOT rejected). RED-first: a
failing capability ledger ⇒ `GateError`, no durable decision.

## C7 (major — orchestrator finding, promoted to blocking-adjacent) No silently unapplied delta entries

The CLI executor applies delta entries ONLY through Annex E env
variables; entries without a variable are "gate-classified but
cannot be applied" — i.e. the evaluation RUNS while the gate
adjudicates a candidate whose declared delta was never applied. That
adjudicates the wrong candidate. Fix (fail-closed, choose the
mechanism): EITHER apply every declared entry mechanically at the
config-resolution layer (no env-var hop), OR refuse evaluation with
a typed error naming every entry the composition cannot apply —
before any arm runs. In both cases the honest rule is: every entry
of an evaluated delta is provably applied, or nothing is evaluated.
RED-first: a delta carrying an unappliable entry ⇒ typed refusal
before any arm runs (no EvaluationRecord, no runs charged).

## C8 (minor m6) Re-derive the `max_attempts_per_unit` Inert justification on C1's ground

The comment's argument ("fewer settlements ⇒ margin refuses") only
held via the sentinel. On C1's ground the honest argument is: the
weakening direction (fewer attempts) starves units ⇒ the arm cannot
reach settled terminal ⇒ C1's completion gate makes the figures
underivable ⇒ Provisional, never promoted. Rewrite the comment to
that argument and add a regression test proving the weakening
direction cannot promote (reduced attempts ⇒ provisional path).

## C9 (minor m1) Remove the dead `ReasonMismatch` contract

`RevisionDecisionRecord::new`'s `carried` computation is `true` in
every arm; `DecisionRecordError::ReasonMismatch` is unreachable and
misdocuments the design (the real stale-claim gate is the ledger's
`StaleDecision`). Remove the dead variant and the dead computation;
the ledger's `StaleDecision` remains the enforced rule (adjust the
port docs to say so).

## C10 (minor m2) Condition-1 docs state the superset truth

gate.rs/mod.rs docs say the treatment mix "equals control's"; the
code checks `control ⊆ treatment` (superset passes; cherry-picking
caught). Fix the docs to the implemented, conservative rule — do not
change the code to match the docs.

## C11 (minor m5 + records) Record the domain-record deviation

The domain `CandidateRevision` record is unused; the floors are
re-derived in the port/gate (D2's append-only design genuinely
conflicts with the domain record's mutable decision field). Record
the deviation in `b1-report.md` (append a COR-1 section) — the
closure will carry it. No code change.

## Records, not code (no fix owed)

- i1 re-evaluation cherry-picking (`evaluations.last()`),
  i2 the governance target prefix heuristic,
  i3 the optimizer dispatch's whole-budget slice,
  i4 the AK-057 double running on the in-memory ledger: trigger-record
  these in the closure inputs (append to `cor1-report.md`), for
  task24/27 and the next transport/composition touch. Do not fix now.

## Discipline

- Build-lock: `flock /home/sprime01/projects/gauntlet/.agents/reports/task23/compile.lock
  -c '<command>'`, and INSIDE the same flock'd command AFTER the
  build run `sccache --stop-server`. Long builds in background, poll
  every 30–60 s. Never raise parallelism.
- Declare-first checkpointing: `cor1-progress.md` (STARTED/DONE per
  item, sha256 of edited files); finish with `cor1-report.md`
  (per-item RED evidence → fix → GREEN, gate results, deviations).
- Never edit the ruler files; never weaken assertions; no new
  dependencies; do not stage/commit.
- After all items: run the slice-scoped gates (`cargo nextest run
  --test p9_self_improvement_containment`, `cargo nextest run -p
  gauntlet-app promote::`, `cargo nextest run -p gauntlet-app --test
  promotion candidate_only`, `cargo nextest run -p gauntlet-ports -E
  "test(revision_ledger)"`, `cargo nextest run -p
  gauntlet-adapter-state-sqlite --test contract_suite`, `cargo
  nextest run -p gauntlet-arch-tests -E "test(optimizer)"`) and
  `just check` — all must be green before you report done.
