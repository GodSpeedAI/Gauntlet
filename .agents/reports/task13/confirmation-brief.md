# Task 13 — FRESH BLIND CONFIRMATION brief (spec critic + falsifier)

You are a **fresh-context confirmation critic**. You have never seen this
task's builders, their reasoning, their attempt counts, their identities, or
any prior critic's findings. Nothing in this brief tells you what was found
before, and you must not go looking: **do not read** any of

- `.agents/reports/task13/**` other than this file,
- `.agents/reports/taskN/**` for any other task N,
- `.agents/CURRENT_STATUS*`, `.agents/lessons/**`, `.agents/research/**`,
- `.agents/reports/reference-candidates.md`, `escalations.md`.

If you find yourself reading a builder's report or a prior critic's findings,
stop — a confirmation round that reads the defence is not a confirmation
round.

## What you are confirming

Task 13 of `.agents/plans/gauntlet-impl.md` — **"Settlement, atomic commit,
single-writer discipline"** — claims to prove **P7** (kill-point recovery:
every kill point resumes correctly with exactly one settlement record) and
**P11** (critic-is-not-verifier: a comparator `pass` with no verification
record does not settle), and to satisfy answer-key Units **AK-040, AK-041,
AK-042, AK-043**, **AK-062 partially** (the service-level half; the
operator-action positive half is out of scope for this task by plan), with
**AK-031** riding P11 and **AK-024**'s settle-time half ("digest re-verified
immediately before settlement") upgraded from substrate to composed.

The plan's done-when: every kill point resumes correctly with exactly one
settlement record; reordering the commit steps makes P7 fail (teeth).

## Your authorized sources

- `.agents/specs/Core-spec.md` — normative. Especially §9.4, §10.2, §11.5,
  §14.3, §16 (settlement: §16.1 preconditions, §16.3 commit order and keyed
  idempotent replay, §16.4 single-writer), §17.5, §18.2–§18.4 (checkpoints,
  resume), §20.2, §21.3, and invariants I1/I3/I5/I8/I11 as cited by §16.
- `.wayfinder/answer-key.md` — the **frozen ruler**. Read AK-040, AK-041,
  AK-042, AK-043, AK-062, AK-031, AK-024, AK-053 verbatim. **Never propose
  editing it.**
- `.agents/plans/gauntlet-impl.md` — the Task 13 section only (Steps, Gate,
  Done-when, Redesign trigger).
- `AGENTS.md` — the repository's structural rules.
- **The artifact**: the working tree. Principally
  `crates/gauntlet-app/src/settle/**`,
  `crates/gauntlet-app/src/control/controller.rs`,
  `crates/gauntlet-app/src/verify/{precondition,authority,ladder}.rs`,
  `crates/gauntlet-ports/src/{state_store,producer_authority,verification_record}.rs`,
  `crates/gauntlet-ports/src/contract/{state_store,stubs}.rs`,
  `crates/adapters/gauntlet-adapter-state-sqlite/**`,
  `crates/gauntlet-arch-tests/src/{lib.rs,single_writer.rs}`,
  `crates/gauntlet-cli/tests/{p7_recovery,p11_critic_is_not_verifier,g10_builder_writes_answer_key}.rs`.

## The question you are answering

**Is the claimed capability real through the intended production/composition
path — or does it exist only in isolated components and fixtures?**

For every claim, ask both:

1. Does the mechanism exist and is it correct in isolation?
2. Is it **reachable and load-bearing on the path the shipped controller and
   settle service actually take** — or reached only by a test fixture that
   supplies its own wiring, its own admission, its own store, or its own
   answer?

A mechanism that passes only because a proof-local fixture supplies both
sides of the decision is a finding, not a pass.

## Role A — spec critic

Verify the landed surface against the normative spec, section by section:

- **§16.1** — all five unit-level preconditions enforced at the service
  entry, against durable/frozen state (not caller snapshots): PASS outcome
  with evidence; verification record admitted through the governed chain;
  authority-chain condition (the unit's frozen authority permits closure by
  the closing actor); no unresolved blocking residual; reference digest
  re-verified through the reference provider.
- **§16.3** — the commit sequence (durable event append → status/evidence
  write → in-memory advance) is observable and ordered; settlement events
  keyed by `(run_id, unit_id, verification_ref)`; replay idempotent —
  same-key same-facts is a no-op, and divergence is loud, never silent.
- **§16.4** — only the governed settle path writes Unit status; the
  answer-key writer is confined to one module, pinned by a mechanical arch
  rule; runtime mounts the key read-only into workspaces.
- **§18.3** — resume completes half-applied settlements exactly once and
  never re-runs settled work.
- **§21.3** — disposition routing for non-integrity failures; only integrity
  violations abort the run.
- **AK-024 settle-time half** — the digest consult at settle time reads the
  provider's verification, not a digest the controller itself froze in the
  same process (no tautology).
- **§23.1 honesty** — judge the proofs' assertions: does each proof actually
  discriminate the claim it names, or would it pass with the mechanism
  removed? (Teeth *records* are out of your scope — the orchestrator re-runs
  those personally. Judge the proof code you can read.)

Write your report to
`.agents/reports/task13/final-confirmation-a-spec.md` — **write the file
first**, then return only a short summary.

## Role B — falsifier

Attack it. You are read-only but may construct and run probes (tests,
one-off cargo runs within the build-lock discipline below). Candidate attack
surfaces, without exhaustiveness claim:

- **Double settlement** — any path that mints a second durable settlement
  for a unit already settled: fresh honest PASS at a settled unit's locator,
  builder-declared attempted lists naming units outside the dispatched
  flight, resume replay, divergent-evidence events, direct trait/inherent
  store calls from anywhere in-process, the sqlite adapter's inherent
  (non-trait) public surface.
- **Admission forgery** — can a settlement commit be constructed without the
  earned admission (caller-attested timestamps, fabricated verification
  refs, PASS docs written raw, readiness tokens ported between claims,
  stale/mutated unit snapshots)?
- **Order violations** — in-memory-before-durable shapes anywhere on the
  production path (not only the proof's probed windows).
- **Single-writer escapes** — masked spellings of the answer-key path
  (formatted construction, env-injected roots, symlinks) on any runtime
  write path; Unit status written outside the governed service.
- **Settle-time blindness** — any path to SettlementCommitted that skips the
  provider digest consult or compares a digest against itself.
- **Non-PASS settling** — INCONCLUSIVE/FAIL outcomes, missing verification
  records, comparator-pass-only shapes reaching settlement.

Write your report to
`.agents/reports/task13/final-confirmation-b-falsifier.md` — **write the
file first**, then return only a short summary.

## Method (both roles)

- **You are read-only except for your one report file.** Do not edit, create,
  or delete any other file. Do not commit. Never fix anything.
- You **may** run tests and the compiler to check a claim, but the
  orchestrator holds a single-compiler build lock: run **one** cargo/nextest
  command at a time, never concurrently, and prefer package-scoped runs
  (`cargo nextest run -p gauntlet-app settle::`) over workspace-wide ones.
  This is a memory-constrained WSL environment; do not raise parallelism,
  and do not run `cargo clean`.
- **Executed evidence beats reading.** Where you can construct a probe that
  demonstrates a bypass, do it and report what actually happened.
- **Bounded claims only.** Report "no bypass found within <declared search
  scope, threat model, and budget>" — never "no bypass exists", never
  "minimal", never universal absence. State your search bound explicitly and
  name what you did not examine.

## Deliverable structure

Structure each finding as: **severity** (CRITICAL / MAJOR / MINOR) · the
claim it breaks · `file:line` · the concrete failure scenario (inputs/state →
wrong outcome) · executed evidence if you have it.

End with an explicit verdict — **CONFIRM** or **NOT CONFIRM** — and the
reason. CONFIRM means: within your declared search bound, the AK claims and
P7/P11 hold through the composed production path, and nothing you found
rises above MINOR. Do not soften a NOT CONFIRM to be agreeable, and do not
manufacture findings to look thorough; if the artifact holds up, say so and
show what you tried.
