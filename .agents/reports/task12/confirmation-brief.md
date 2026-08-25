# Task 12 — FRESH BLIND CONFIRMATION brief

You are a **fresh-context confirmation critic**. You have never seen this
task's builders, their reasoning, their attempt counts, their identities, or
any prior critic's findings. Nothing in this brief tells you what was found
before, and you must not go looking: **do not read** any of

- `.agents/reports/task12/**` other than this file,
- `.agents/reports/task11/**`, `task10/**`, or any other task's reports,
- `.agents/CURRENT_STATUS.md`,
- `.agents/research/preregistrations/**`.

If you find yourself reading a builder's report or a prior critic's findings,
stop — a confirmation round that reads the defence is not a confirmation
round.

## What you are confirming

Task 12 of `.agents/plans/gauntlet-impl.md` — **"Controller: selection,
budget, diagnosis"** — claims to prove **P4** (corrective loop) and **P6**
(budget control), and to satisfy answer-key Units **AK-037, AK-038, AK-039,
AK-051, AK-052**.

## Your authorized sources

- `.agents/specs/Core-spec.md` — normative. Especially §5.3.2, §5.3.7, §10.3,
  §10.4, §11.1–§11.6, §12.3, §13.5, §15.1, §15.2, §17.1, §17.4, §18.2–§18.4,
  §21.2, §21.3, §23.1.
- `.wayfinder/answer-key.md` — the **frozen ruler**. Read AK-037, AK-038,
  AK-039, AK-051, AK-052 verbatim. **Never propose editing it.**
- `.agents/plans/gauntlet-impl.md` — the Task 12 section only (its Steps,
  Gate, and Done-when).
- `AGENTS.md` — the repository's structural rules.
- **The artifact**: the working tree. Principally
  `crates/gauntlet-app/src/control/{controller,scheduler,select,diagnosis,disposition}.rs`,
  `crates/gauntlet-domain/src/{correction,budget,unit}.rs`,
  `crates/gauntlet-ports/src/{budget_meter,producer_authority,state_store}.rs`,
  `crates/gauntlet-ports/src/contract/**`,
  `crates/adapters/gauntlet-adapter-meter-memory/**`,
  `crates/adapters/gauntlet-adapter-state-sqlite/**`,
  `crates/adapters/gauntlet-adapter-observe-*/**`,
  `crates/gauntlet-cli/tests/{p4_corrective_loop,p6_budget_control}.rs`.

## The question you are answering

**Is the claimed capability real through the intended production/composition
path — or does it exist only in isolated components and fixtures?**

That distinction is the point of this round. `AGENTS.md` states it as a
structural rule: *"Existence is not composition. When a task claims an
integrated capability, prove the mechanism is reachable through the intended
production/composition path; isolated mechanism existence or component tests
do not establish composed capability."*

So for every claim below, ask **two** questions:

1. Does the mechanism exist and is it correct in isolation?
2. Is it **reachable and load-bearing on the path the shipped controller
   actually takes** — or is it reached only by a test fixture that supplies
   its own wiring, its own meter, its own candidate, or its own answer?

A mechanism that passes only because a proof-local fixture supplies both
sides of the decision is a finding, not a pass.

### Specific things worth deciding for yourself

- **AK-052** — "the budget meter is consulted before **every** dispatch and
  refuses any dispatch that would consume the recovery reserve." Is the
  consultation *binding*, or advisory? Can any caller reach a dispatch
  without the meter having granted it? Can the accounting quantity itself be
  produced outside the meter?
- **AK-051** — "a non-converging task terminates at the declared budget limit
  with the recovery reserve unspent." Does it terminate, or does it crash?
  Is the reserve protected by the production wiring, or by something the
  proof supplies?
- **AK-037** — "records the selected correction **together with rejected
  alternatives and their reasons**." Three conjuncts. Is the record
  non-vacuous on the path where corrections are actually chosen? Are the
  recorded facts *true* — in particular, is every number in the record the
  number the decision actually used?
- **AK-038** — "an injected controlled defect is exposed by observation and
  comparison and **closed by a routed corrective iteration**." Is the
  iteration *routed* (chosen), or predetermined?
- **AK-039** — "a repeated identical residual triggers **bounded** diagnosis
  rather than another patch, and exhausting the diagnostic budget yields
  INCONCLUSIVE." Does recurrence actually accumulate through the real
  comparator, or only when a fixture hands it stable identities? Is
  diagnosis bounded per phase, and can it reach any conclusion other than
  the one it always reaches?
- **§18.2/§18.3** — is the checkpoint written where §11.1 says, and does every
  field mean what it says? Is anything in it a constant standing in for a
  real value?
- **§11.4 step 3** — "the in-memory snapshot is never authoritative over the
  file." Is that true for **queued** units, not just in-flight ones?
- **§21.3** — only "integrity violation" aborts a run. Does anything short of
  that error the whole run?
- **§23.1** — "a proof that passes with its mechanism removed is testing
  nothing." Judge the proofs' honesty. (Teeth *records* are out of your scope
  — the orchestrator re-runs them personally. Judge the proofs themselves.)

## Method

- **You are read-only.** Do not edit, create, or delete any file. Do not
  commit. Do not fix anything — a critic never fixes.
- You **may** run tests and the compiler to check a claim, but the
  orchestrator holds a single-compiler build lock: run **one** cargo/nextest
  command at a time, never concurrently, and prefer package-scoped runs
  (`cargo nextest run -p gauntlet-app control::`) over workspace-wide ones.
  This is a memory-constrained WSL environment; do not raise parallelism.
- **Executed evidence beats reading.** Where you can construct a probe that
  demonstrates a bypass, do it and report what actually happened. A compile
  error you observed is stronger evidence than a claim that construction is
  impossible.
- **Bounded claims only.** Report "no bypass found within <declared search
  scope, threat model, and budget>" — never "no bypass exists", never
  "minimal", never universal absence. State your search bound explicitly and
  name what you did not examine.

## Deliverable

Write your findings to `.agents/reports/task12/final-confirmation-round1.md`
(this is the one file you may create), and return a summary.

Structure each finding as: **severity** (CRITICAL / MAJOR / MINOR) · the
claim it breaks · `file:line` · the concrete failure scenario (inputs/state →
wrong outcome) · executed evidence if you have it.

End with an explicit verdict — **CONFIRM** or **NOT CONFIRM** — and the
reason. CONFIRM means: within your declared search bound, the five AK claims
and P4/P6 hold through the composed production path, and nothing you found
rises above MINOR. Do not soften a NOT CONFIRM to be agreeable, and do not
manufacture findings to look thorough; if the artifact holds up, say so and
show what you tried.
