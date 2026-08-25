# Task 12 — Builder slice B3 brief — controller composition, policies, observer deadlines

You are a builder agent in /home/sprime01/projects/gauntlet (WSL2, 6 cores,
7.8 GiB RAM). Read `AGENTS.md` first; it binds you. Then
`.agents/reports/task12/recon-spec.md` (spec quotes + line refs),
`.agents/reports/task12/recon-code.md`, `.agents/reports/task12/builder-b1.md`
and `.agents/reports/task12/builder-b2.md` (landed slices — read the actual
APIs from source: ports `producer_authority.rs`, adapter
`gauntlet-adapter-meter-memory`, app `control/{scheduler,select,diagnosis}.rs`,
domain `correction.rs`, `Budget::restore`, StateStore
`record_correction_selection`).

Task 12 (plan): controller proving P4/P6 (AK-037..039, 051, 052). B1 landed
owner-write gating + meter; B2 landed the control core. **B3 composes the
real controller** — the production iteration loop and its policies — plus
observer-side deadline enforcement. Proofs p4/p6 are B4; do NOT write them.

## Hard rules (same as B1/B2)

- Never edit `.wayfinder/**`, `.agents/specs/**`, `.agents/plans/**`,
  `AGENTS.md`, `justfile`, `deny.toml`, `.agents/CURRENT_STATUS.md`,
  `.agents/research/preregistrations/*`, existing task12
  recon/adjudication/brief/report files, `.agents/prompts/**`, `.tmp/**`,
  `.jolli/**`. No commits.
- Single-compiler build lock; one Cargo command at a time; package-scoped
  recipes first; no raised parallelism; no `cargo clean`; `du -sh target`
  < ~4 GiB.
- Fail-first where practical; typed errors; no `unwrap`/`expect` in
  production paths; clippy `-D warnings`; fmt; the repository's named-const
  idiom for any literal (denylist/literal scans bit a previous session —
  26+68 violations; do not repeat that).
- **Checkpoint discipline: append a dated entry to
  `.agents/reports/task12/builder-b3-checkpoints.md` after EACH item.**
- Result contract: `.agents/reports/task12/builder-b3.md` (per-item summary,
  exact APIs, files, fail-first evidence, gate outputs, deviations, open
  questions). If you recover an interrupted predecessor's partial work,
  reconcile per `.agents/lessons/session-interruption-recovery.md` and say
  so in the report.

## Activated residuals this slice closes (adjudication on record)

- Task-7 M4: production `verify_unchanged` caller at every iteration
  boundary (mismatch ⇒ terminal `aborted{reference_mutated}`, BOTH digests
  persisted as evidence; §9.4, §11.4 step 4, §11.5 last-ish rows).
- Task-7 m3: typed Unavailable-vs-ReferenceMutated controller policy
  (§21.3/§22.2): mutated ⇒ abort, preserve, never retry; provider/instrument
  Unavailable ⇒ non-fatal gap ⇒ INCONCLUSIVE for dependent claims (§12.3).
- Task-7 F18: the controller never trusts `verify_unchanged` solo — refuse
  an empty parsed Unit set at composition (typed refusal).
- task11 NOTE-2: ONE canonical workspace root field on the controller; every
  derived consumer (verify `for_run`, observation requests, workspace refs)
  flows from it.
- Task-9 D1: observer deadline enforcement + output-size caps in the
  observe adapters' command path.
- Task-8 A-m2 / §10.3 rule 5 / §18.4: real deadline composition — bounded
  poll loops, TimedOut disposition (cancel, preserve session ref, retry
  once, then escalate), §11.4 stall detection. All time flows through the
  injected `Arc<dyn Clock>` — **no `std::thread::sleep` / wall-clock waits
  in the controller or its tests** (DeterministicClock advances virtual
  time; the mock runner is time-free by design).
- Task-6 MAJOR-2 wiring: resume composes `Budget::restore` from the
  checkpoint's `budget_spent`.
- B2 residuals (in `builder-b2.md`): re-consult the meter on the SUMMED
  slice immediately before dispatch; wire `QuarantineRecord` persistence
  into the store.

## Item 1 — `control/controller.rs` (RunController + iteration engine)

A composed controller over injected ports (app layer — domain+ports deps
only; real adapters are B4's CLI-composition concern):

- **Constructor** takes: state `Arc<dyn StateStore>`, evidence
  `Arc<dyn EvidenceStore>`, ids, `Arc<dyn Clock>`, runner
  `Arc<dyn AgentRunner>` (wrap with `compare::RecordingAgentRunner`
  discipline), pre-built observers, comparison deps (contract + ReviewDeps
  sources), **pre-built `Ladder`** (do NOT widen the composition-seal —
  `Ladder::new` stays feature-gated; the controller receives the ladder),
  the reference provider inputs (frozen pair + lock path or the provider
  itself), `Arc<dyn HumanEscalation>`, `Arc<dyn BudgetMeter>`, and the
  control config (see Item 3). ONE canonical workspace root field.
- **Composition guards** (fail-fast, typed): empty parsed Unit set refused
  (F18); `depends_on` graph validated (B2 scheduler does this — surface its
  error); budget knobs present.
- **Loop §11.1/§11.4** — `run()` drives: freeze/verify → plan → iterate:
  1. **Tick**: poll in-flight via `AgentRunner::poll`; **stall detection**
     (no event and no result for `stall_timeout_ms` ⇒ cancel, record,
     retry-or-escalate per §11.5) — virtual-time driven.
  2. **Re-read the answer key EVERY tick** (§11.4 step 3): re-parse from
     the provider's file bytes; an operator hand-edit to an in-flight Unit
     (tier/authority/claim/status) cancels the affected attempt and
     re-evaluates eligibility — the in-memory snapshot is never
     authoritative over the file. Fail-first test with a mid-run fixture
     edit (this is prereg P2 — make it impossible to pass by caching).
  3. **Boundary re-verify**: `verify_unchanged` at EVERY iteration boundary;
     mismatch ⇒ terminal `aborted{reference_mutated}` with both digests
     written to the evidence store + pinned (durable evidence, not a log
     line). Also re-check the §8.5 drift subset that is re-checkable at
     runtime — ground the subset from §8.5 text and the doctor module's
     callable checks (D16); document which checks you re-run and why others
     cannot drift mid-run.
  4. **Re-check budget** before dispatching: meter `would_breach_reserve` on
     the SUMMED pending slice (B2 residual); breach ⇒ stop dispatching and
     drive to terminal `budget_exhausted` — "recovery reserve used only for
     checkpoint + report" (§11.6): after the terminal decision, checkpoint +
     report actions remain affordable by construction; forward dispatch
     never touches the reserve.
  5. **Dispatch**: scheduler plan → meter `reserve` → `RoleInvocationSpec`
     (whole-budget `budget_slice` per the Task-4 adjudication,
     Clock-derived deadline, result-contract path) → invoke (admission
     returns immediately — never await).
  6. **On result**: strict `read_result_contract`; then observation —
     `ObservationAssembler::assemble` + **bind via
     `ObservationBindingAuthority`** (the gated write); then comparison —
     `ComparisonAuthority::for_run` → `issue` → `review` → `accept`; then
     verification — `VerificationAuthority::for_run` + `verify_claim`, with
     instrument evidence recorded through **`EvidenceProductionAuthority`**
     (the gated write; the controller is the evidence-owner wiring point);
     then **§11.5 retry-table disposition** (typed table — see Item 2);
     then `CorrectionSelector::select` + `record_correction_selection`;
     then **§18.2 checkpoint** (run id, reference digest, iteration number,
     unit-state projection, `budget_spent`, in-flight ids + contract paths,
     event-log offset).
- **Payment/charging**: charge the meter per completed invocation
  (`Reservation` → `charge(payment)`); zero-payment charge = reservation
  release for cancelled/failed invocations (B1 adapter convention).
- **Resume (§18.3, budget/iteration scope)**: `Budget::restore` from
  checkpoint `budget_spent`; re-probe in-flight (valid result contract on
  disk ⇒ collect; else `Failed{orphaned_by_restart}` + re-plan); re-verify
  reference first. Full crash-injection/E2E recovery is Task 13's P7 —
  record that boundary in the report; do not build it here.
- **Unavailable-vs-ReferenceMutated**: a typed disposition enum on the
  controller; both classes reachable in tests with distinct outcomes
  (abort-and-preserve vs gap-and-INCONCLUSIVE-for-dependent-claims).

In-crate tests over fakes (app cannot depend on adapters): full-iteration
loop over a scripted fake runner; boundary abort on mid-run reference edit
(fail-first); stall → cancel → retry-once → escalate; budget exhaustion →
terminal with reserve intact; resume restores budget; both gated writes flow
through the authorities (a fake store that counts calls proves no direct
write path exists — and direct calls are compile-refused by B1's types).

## Item 2 — §11.5 retry/disposition table (typed)

`control/disposition.rs` (or inside controller.rs if small): the §11.5
table as typed logic, each row tested: residual+attempts-left ⇒ re-dispatch
with residual TEXT ONLY (never critic trajectory) — assert the second
dispatch's prompt carries the residual and not the critic's reasoning;
transient failure ⇒ exponential backoff between `retry_base_ms` and
`retry_max_ms` (virtual-time asserts, no sleeps); `malformed_result` ⇒ one
re-dispatch with schema reminder then escalate; `TimedOut` ⇒ cancel,
preserve session ref, retry once, then escalate; attempts exhausted ⇒
`max_attempts_exceeded` escalation; `authority ≠ critic` needing closure ⇒
escalate, never loop; sandbox failure ⇒ escalate, never unsandboxed
fallback; reference digest mismatch ⇒ abort, no retry. Non-convergence
routing into B2's DiagnosticEngine (stop correcting; diagnosis; inconclusive
diagnosis ⇒ escalate; exhaustion ⇒ INCONCLUSIVE + quarantine persisted +
escalation item).

## Item 3 — control config (spec-backed only)

Resolve through the existing config layer: `poll_interval_ms` default 5000
(§11.4 inline — spec-backed); `stall_timeout_ms`, `retry_base_ms`,
`retry_max_ms`, `max_attempts_per_unit` have NO spec default ⇒ REQUIRED
settings with typed absence errors (no invented literals — §8.6 table is
the only literal home and it has no rows for these). Reuse B2's
`nonconvergence_n` (default 3). `.env.example` already lists all these
names; `env_example_complete` must stay green.

## Item 4 — observer deadline enforcement + output caps (Task-9 D1)

In the observe adapters (`crates/adapters/gauntlet-observe-*/`), find the
shared command-execution path (build/test/command/versioning): enforce the
`observe` deadline — wait-with-deadline, kill on expiry, report
`ObserverError::Unavailable` (which the assembler records into BOTH gap
arrays — the §18.4 "test hangs" disposition); bound captured output size
(capped constant from config or a named adapter const with rationale —
respect the literal-scan idiom); the filesystem observer's walk gets an
entry cap if unbounded today. Fail-first: a command script that sleeps past
a short deadline (use small real timeouts ONLY where virtual time is
impossible — a child process is real; keep the bound tight, e.g. ≤2s, and
assert the facet/observation outcome, not wall-clock); an already-expired
deadline refuses without spawning. Versioning/build/test/command adapters
share the fix — no per-adapter drift.

## Item 5 — Gates (run personally, record)

```
just check-package gauntlet-app && cargo nextest run -p gauntlet-app control::
just check-package gauntlet-adapter-observe-command && just test-package gauntlet-adapter-observe-command
(similarly observe-build, observe-test, observe-filesystem, observe-git)
just check-package gauntlet-ports && just test-package gauntlet-ports
just check && just lint && just deny && just boundaries && just ruler
```

Red gate at session end ⇒ report loudly; never weaken.

## Out of scope

p4/p6 proofs and the CLI composition root (B4), Task-13 settlement commit
path (a settlement precondition assertion is fine; `commit_settlement`
wiring is Task 13), HumanEscalation real adapters, sandbox/authority gate
(Task 14), widening the composition seal, doctor changes.
