# Task 13 / Builder brief B1 — ports settlement surface + earned admission + store disciplines

You are a BUILDER. Read `AGENTS.md` first. This brief is your complete scope.
Anything not here is out of scope; record adjacent findings in your report,
do not fix them.

## Context you need (read these, in order)

1. `AGENTS.md` — runtime rules (type discipline, error taxonomy, testing loop).
2. `.agents/reports/task13/recon-code.md` §2–§3 — the exact substrate you are
   changing (signatures, line references, migrations, error enums).
3. `.agents/reports/task13/recon-spec.md` §16 — the normative contract.
4. `.agents/reports/task13/residual-adjudication.md` — items A1, A2, A3, A5
   are YOUR scope (store legs); A6/A7 are B2's.
5. `crates/gauntlet-ports/src/producer_authority.rs` — the earned-admission
   pattern you must follow (Task 12 B1's precedent: authority validates
   against durable state, mints an unforgeable admission, the store write
   consumes it).

## Deliverables (each with fail-first evidence where it changes behavior)

**Item 1 — `StateStore` port trait: settlement surface.**
Move the settlement write surface from `SqliteStateStore` inherent methods to
the `StateStore` port trait (`crates/gauntlet-ports/src/state_store.rs`):
`commit_settlement`, `settlements` (read-back), and the resume-completion
primitive. Design constraint (preregistered R1/A5): the trait's
`commit_settlement` must take an **earned admission** — an
`AdmittedSettlement`-shaped value minted by a new
`SettlementCommitAuthority` in ports (following `producer_authority.rs`:
`new(Arc<dyn StateStore>)`; `admit(...)` validates against DURABLE state —
the verification record exists in the records table with `Outcome::Pass` for
this (run, unit), the settlement is not already committed (idempotence key
`(run_id, unit_id, verification_ref)`), the evidence ref is pinned — and mints
the unforgeable token; no public constructor outside the module). The
`SettlementCommit` struct's caller-supplied `at` stays as the event
construction timestamp ONLY (see Item 3). The store method signature consumes
the admission. Keep `complete_pending_settlements` reachable for resume (B2
composes it) — as a trait method taking the run id + the replay anchor.
Decide and document: what the authority checks vs what the store
cross-validates (the store keeps its offset cross-validation — defense in
depth, both must hold).

**Item 2 — replay anchored at the §18.3 checkpoint offset (A2 store leg).**
`complete_pending_settlements` gains the checkpoint event-log offset as its
replay start (read the run's latest checkpoint inside the store, or accept
the anchor explicitly — follow the existing `resume` selection semantics;
document the choice). Fail-first: a test that demonstrates the anchor is
respected (settlement events BELOW the checkpoint offset are not re-scanned
for completion — construct a store where a pre-checkpoint settlement event
exists whose row was applied, then a post-checkpoint half-applied one;
completion must add exactly the missing one and report the correct count).
The Task-6 `crash_recovery.rs` tests must stay green (superset behavior was
safe; the anchor must not break the existing exactly-once assertions).

**Item 3 — `settled_at` becomes event-derived (A3).**
The settlements row's `settled_at` must come from the DURABLE event's stored
timestamp, not the caller's `commit.at`: `verify_settlement_event_at` must
decode the stored `at` and (a) use it for the row insert, (b) refuse a commit
whose constructed event timestamp disagrees with what is stored at the offset
(for the replay path this is inherent; for the direct path it closes the
unchecked seam). Fail-first: today's behavior test (caller `at` lands in the
row even when it disagrees with the event) must be shown RED under the old
code shape or by a discrimination test asserting the new refusal.

**Item 4 — strict `settlements_of` offset decode (A3).**
`u64::try_from(offset).unwrap_or(0)` → typed `Corrupt` (mirror the Task-12 N3
checkpoint decode discipline and message shape). Fail-first with a tampered
negative offset row (follow `tamper_evidence.rs` patterns).

**Item 5 — evidence-production crash-window completion (A5 store leg).**
Resume-side completion for the `EvidenceProduced` window (event appended, row
missing — the window Task 11's error string names): a store primitive
`complete_pending_evidence_productions(run_id, anchor)` following the
settlement replay pattern, idempotent on the existing production identity.
Fail-first: half-applied production (event without row) is completed exactly
once; already-applied adds zero.

**Item 6 — shared contract suite extension (R2/A2).**
Extend the Task-4 `StateStore` shared contract suite with the settlement
surface: commit-requires-admission (a hand-built `SettlementCommit` without
the mint does not type-check or is refused — per your design), idempotent
replay, offset cross-validation refusal, anchored replay. The suite must fail
against a stub that fakes the surface (teeth per the suite's existing
conventions). `state-sqlite` is the conforming adapter; keep the suite
adapter-agnostic.

**Item 7 — export discipline.**
Settlements rows now carry event-derived `settled_at`; verify the export
segment ordering/pinning still holds (`crates/gauntlet-app/src/state/export.rs`
+ `export_stack.rs`): if a pinned golden changes, the change must be evidenced
as intended new behavior in your report (never silent).

## Hard constraints

- NEVER touch `.wayfinder/`, `.gauntlet/reference.lock`, `deny.toml`,
  nextest config, AGENTS.md, `.jolli/`, `.tmp/`, `.agents/prompts/`, or any
  `migrations/*.sql` already applied (a NEW migration is allowed only if a
  schema change is genuinely required — justify it; the settlements schema
  needs NO change for Items 1–5).
- Do not modify the frozen Task-4 suite's existing cases; only ADD.
- `just ruler` before you start and after you finish (must PASS both times).
- You hold the build lock EXCLUSIVELY: do not spawn sub-agents that compile.
- Respect repository parallelism settings; no `--jobs` increases (WSL memory).
- No `unwrap`/`expect` in production paths; typed errors with loci.

## Checkpoint discipline (mandatory — lessons from 15 interruption instances)

Append to `.agents/reports/task13/builder-b1-checkpoints.md`:
- BEFORE editing each item: a `STARTED B1.<n>` line with one-line intent.
- AFTER each item: a `DONE B1.<n>` line with what landed + test names +
  fail-first evidence (command + observed RED then GREEN).
- After EACH item, not at the end. If you die mid-item, the STARTED line is
  the reconciliation anchor.

## Result contract

Write `.agents/reports/task13/builder-b1.md` (the contract — completion means
this file exists and is accurate): items delivered, deviations, fail-first
evidence per item, gate output (verbatim tails), files touched, known gaps.

## Gates (run personally, record verbatim tails)

```bash
just ruler
just check-package gauntlet-ports
just test-package gauntlet-ports
just check-package gauntlet-adapter-state-sqlite
just test-package gauntlet-adapter-state-sqlite
just check-package gauntlet-app
just lint
```
(app `just check-package` because the export pin lives there; run
`just test-package gauntlet-app` if you touched anything app-side — you
should NOT need to outside the export pin check.)
