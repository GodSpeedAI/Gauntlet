# Task 12 — correction FINISHER F4: the meter on the critic-fan-out leg

You are a correction finisher in `/home/sprime01/projects/gauntlet`.
Slice F4 exists because the fresh blind confirmation round returned NOT
CONFIRM: AK-052 ("The budget meter is consulted before every dispatch and
refuses any dispatch that would consume the recovery reserve") holds on the
builder-dispatch leg but FAILS on the critic-fan-out leg — the review path
the controller itself drives every iteration admits model comparators
under a COUNT budget only, and `ComparisonDeps` has no meter field, so the
run meter is structurally unwireable into review dispatch. Adjudication:
`.agents/reports/task12/confirmation-adjudication-round1.md` (finding 1
FIX NOW, finding 2 doc-truth now). You implement finding 1's gate and
finding 2's doc correction. You are NOT blind — read the adjudication and
`final-confirmation-round1.md` finding 1 first.

## Read first

1. `AGENTS.md`
2. `.agents/reports/task12/confirmation-adjudication-round1.md`
3. `.agents/reports/task12/final-confirmation-round1.md` (finding 1 + 2)
4. `.agents/reports/task12/correction-f2.md` (items 6/7/9 moved the code
   you are about to move — C9.3's release discipline, C9.4's derived-bound
   discipline, N5's forward-headroom semantics) and the F2/F3 checkpoint
   entries
5. Spec: `.agents/specs/Core-spec.md` §10.4 (verbatim, the truncation
   rule), §17.4, §5.3.7, §6.2, §13.3. `compare/review.rs`'s module doc.

## The requirement (spec-grounded, nothing more)

§10.4: "Critic fan-out is bounded by `review.max_critics` **and by the
budget meter**; a fan-out that would breach the recovery reserve is
truncated in dimension-priority order, and the truncation is recorded as
`unavailable_evidence`, not silently dropped." §17.4: the meter is
consulted BEFORE every dispatch; `max_parallel` bounds total concurrent
invocations ACROSS ALL ROLES (§10.4 concurrency rules). AK-052's claim
sentence is §17.4's first sentence.

## Item 1 — wire the run meter into review dispatch (fail-first)

Current shape (re-derive line numbers yourself — F2/F3 moved this file):
`compare/review.rs::schedule_model` admits a model comparator under
`ModelDispatchBudget::lease()` (a count) only; `ReviewDeps` (review.rs:163)
carries three comparator sources and no meter; the controller calls
`review(&deps, mode, ModelDispatchBudget::new(self.comparison.model_dispatches),
…)` at `controller.rs:~2123`.

Design (adopt or improve with recorded justification):

1. `ReviewDeps<'a>` gains `pub meter: &'a dyn gauntlet_ports::budget_meter::BudgetMeter`
   — REQUIRED, not `Option`: AK-052's "every dispatch" means no composition
   may skip the meter by passing `None`. Test callers stub a permissive
   meter (documented test-only; the ports' contract stubs may serve).
2. The consult happens at the admission point (`schedule_model`'s admit
   path, beside the count lease): `meter.reserve(critic_slice)`. A
   reservation is HELD per admitted job (it rides the same lifecycle as
   `DispatchLease` — released on the job's terminal via the zero-payment
   charge, the B1 convention C9.3 uses) so critic and builder dispatches
   share `max_parallel` (§10.4 "across all roles").
3. **The slice must not be vacuous.** A zero slice makes the §5.3.7 gate
   trivially affordable — a vacuous fix is the exact defect class this
   round keeps catching. Ground `critic_slice` in an existing configured
   shape — the indicated candidate is the loop's own per-attempt estimate
   (`per_attempt_estimate`, the same affordability figure the builder leg
   reserves) — and RECORD the adoption reason against §10.4/§17.4. Do NOT
   add a new Annex E / `ControlPolicy` setting (the C9.4 blast-radius
   warning applies; critic-A m2's open minor already covers four REQUIRED
   Annex E control settings).
4. On the meter's refusal (`BudgetMeterError::Reserve { cause, .. }`):
   the dimension is truncated — return the same `false` the count
   exhaustion returns, but the `unavailable_evidence` record the caller
   pushes (`UnavailableEvidence::truncated(dimension)` at review.rs:333/
   354/421) must cite the METER's typed cause (dimension-priority
   truncation per §10.4, never a run error — the C6.1 skip-and-continue
   discipline). A scope refusal (`ScopeExceeded`) is the same truncation
   shape. Never a `ReviewError` for a refused critic dispatch.
5. Controller call site passes `state.meter.as_ref()` in `ReviewDeps`.
6. Composed fail-first tests (in-crate `control::` or `compare::`, through
   the REAL controller compare step — existence is not composition):
   - **Refusal leg**: adaptive/single review with an ADMITTING model
     source, `model_dispatches >= 1`, and a meter at/over the forward
     ceiling → the critic dispatch is refused, the dimension is recorded
     as truncated with the meter's cause in the reason, the run continues
     (no error), and the reservation count shows nothing held after.
   - **No-over-refusal leg**: the same composition with headroom → the
     critic dispatch PROCEEDS, and a subsequent iteration's dispatch also
     proceeds (the slot was released at fan-in — pin against slot leaks,
     which would silently truncate every later iteration).
   - Check both proofs' `model_dispatches` (p4 sets 0 at
     `p4_corrective_loop.rs:1036`; check p6 and the two in-crate
     `ComparisonDeps` fixtures at controller.rs:~4469/~5809) — FakeMeter
     and fixtures move WITH the semantics; PROOF ASSERTIONS DO NOT. If a
     proof assertion would have to change, that is a STOP-and-record.

**Out of scope, recorded not fixed:** payment unification — critic
token/cost payments booking into the run's `budget_spent` (the
compare-model adapter's private `Budget` stays; §17.4's second paragraph;
trigger: Task 15 real model runner / Task 20 composition root). The
count budget stays (§10.4 bounds by BOTH).

## Item 2 — correct the overstated `mint` doc (finding 2, doc-only)

`crates/gauntlet-ports/src/budget_meter.rs:74-112`: the doc says
"arbitrary code cannot forge the parallelism-accounting token". As a
public trait method any meter holder can `mint(any_slice)` + `charge` and
saturating-release a slot never held. Correct the sentence to the true
guarantee: construction requires a meter INSTANCE (authority-minted
inside the trait, not constructible from raw parts outside it), with the
composition-root obligation (meters are handed only to trusted code) and
a pointer that a type-level seal is a recorded ports-revision candidate
(later-scope, trigger: next BudgetMeter ports revision). Doc-only — no
signature, no behavior, no test change. Record the adjudication reference
in the doc comment.

## Method (mandatory, same as F2/F3)

- Run `./scripts/reconcile-checkpoints.sh 12` FIRST; read what it flags.
- DECLARE-FIRST checkpointing into `correction-checkpoints.md`:
  `## <item> — STARTED <timestamp>` + intended design BEFORE editing;
  full `## <item> — DONE <timestamp>` entry after, with verbatim
  fail-first observations, files, tests, gates.
- Fail-first per item. Bounded claims only. Typed errors carrying the
  failure locus. STOP-and-record if anything would need weakening.
- Result contract: `.agents/reports/task12/correction-f4.md` (same shape
  as correction-f2.md/f3.md).
- You hold the single-compiler build lock; one cargo/nextest/just command
  at a time; package-scoped preferred; do not raise parallelism.

## Gates (run personally, sequentially, record verbatim)

```
just check-package gauntlet-app
cargo nextest run -p gauntlet-app control::
cargo nextest run -p gauntlet-app compare::
cargo nextest run --test p4_corrective_loop --test p6_budget_control
cargo nextest run -p gauntlet-ports
cargo nextest run -p gauntlet-adapter-meter-memory
just lint
```

Workspace-wide gates and teeth are the orchestrator's (the teeth re-run
may gain a seventh tooth on your mechanism — say in your report whether
the wiring is tooth-shaped).

## Hard rules

Never edit `.wayfinder/**`, `.agents/specs/**`, `.agents/plans/**`,
`AGENTS.md`, `justfile`, `deny.toml`, `.agents/CURRENT_STATUS.md`, the
frozen prereg files, `.agents/prompts/**`, `.tmp/**`, `.jolli/**`.
**No commits.** No weakened assertions; no proof-assertion edits to
accommodate an implementation. Leave alone everything F1/F2/F3 recorded
as deliberately left alone, plus: payment unification, the mint seal,
§21.3 policy, and the three doctor/lock message nits.
