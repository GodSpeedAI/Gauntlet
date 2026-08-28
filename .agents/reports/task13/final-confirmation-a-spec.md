# Task 13 — Fresh Blind Confirmation, Role A (spec critic)

Date: 2026-08-27. Static-only round (no cargo/nextest/just executed — build lock held
by the sibling falsifier; host memory-constrained). Read-only except this file.
Blindness held: nothing under `.agents/reports/task13/**` was read except the brief;
no other task's reports, no CURRENT_STATUS, lessons, research, reference-candidates,
or escalations content. (`git status` was run to confirm the artifact is committed;
its output lists report filenames only — no report content was read.)

## Scope and authorized sources

- Spec: `.agents/specs/Core-spec.md` §3 (I1/I3/I5/I8/I11), §9.4, §10.2, §11.5, §14.3,
  §16.1–§16.4, §17.5, §18.1–§18.4, §20.2, §21.3, §23 (honesty lens).
- Ruler: `.wayfinder/answer-key.md` AK-040, AK-041, AK-042, AK-043, AK-062
  (service-level half), AK-031, AK-024 (settle-time half), AK-053 (support
  assessment only). Read verbatim; no edit proposed or implied.
- Plan: `.agents/plans/gauntlet-impl.md` Task 13 section only.
- Artifact: the committed working tree (last commit `62d1a5f` — Rust tree clean;
  dirty paths are `.agents/`, `.jolli/`, `.tmp/` only).
- Frozen-locator note: the AK `evidence_required` locators spell repo-root paths
  (`tests/proofs/…`, `tests/contract/…`, `tests/antigaming/…`) that do not exist in
  this tree. Per the brief, the real layout wins: the proofs live at
  `crates/gauntlet-cli/tests/{p7_recovery,p11_critic_is_not_verifier,g10_builder_writes_answer_key}.rs`
  and the AK-040 precondition matrix at `gauntlet-app`'s in-module `settle::` tests.
  This is a pre-existing ruler/tree spelling divergence, recorded here, not judged
  and not a Task-13 defect.

## Composition judgment (the brief's central question)

The claimed capability is real through the composed production path, not fixture
isolation:

- The controller (`crates/gauntlet-app/src/control/controller.rs`) composes
  `SettlementService::new` at `compose` (line 722) and settles exclusively through
  `settlement.settle(SettleRequest { claim, actor: ClosingActor::Automatic, … })`
  inside `process_completion` (line 2515). There is no other settle call site in
  production code; `commit_settlement` has exactly one production caller — the
  settle service (verified by repo-wide search).
- `resume_run` (line 842) calls the service's
  `complete_pending_after_checkpoint` (§18.3 step 1) before re-planning, then marks
  units Settled only from durable settlement rows beyond the checkpoint anchor
  (lines 893–904) — the projection is rebuilt from events, not memory.
- The in-memory advance is observably after the durable commit: `state.unit_states
  … Settled` is inserted only in the `Ok(outcome)` arm (lines 2523–2529), after the
  store returned the event offset.
- The three proofs compose the real controller over the real sqlite state store,
  the real fs evidence store, the real file reference provider, the real command
  observer, and the real diff comparator. Proof-local replacements (scripted
  builder runner, verdict/pass rungs, recording escalation surface, delegating
  order/forensic/breach recorder decorators) are each named and justified in the
  proof headers, and the decorators forward verbatim (no behavior change).
- Reachability caveat (recorded, phasing not defect): the shipped CLI exposes only
  `doctor` and `reference digest` (`crates/gauntlet-cli/src/main.rs`); `gauntlet
  run`/`resume` land with plan Task 20. The composed path is therefore
  library-level today — reachable through the composition root the proofs drive,
  not yet through an operator command. See RES-A5.

## Section-by-section verification

### §16.1 — all five preconditions at service entry, against durable/frozen state

`SettlementService::settle` (`crates/gauntlet-app/src/settle/mod.rs:631`):

1. **Condition 1 (PASS record, this run).** Entry gate on the settling claim's own
   outcome (line 646, `VerificationNotPass` naming the outcome); conditions 1–2
   re-derived inside the service via `settlement_precondition`
   (`verify/precondition.rs:137`: PASS + same-run + evidence resolves + pinned,
   all re-derived from the claim's own chain store, fail-closed on unanswerable pin
   state); the ports' `SettlementCommitAuthority::admit`
   (`ports/src/producer_authority.rs:676`) re-checks durable PASS records for
   (run, unit) with evidence ref AND verification ref equal to the commit's
   (COR-3: the key's third component is durable-moored). No readiness parameter
   exists on `SettleRequest` — a token minted over one claim cannot travel to
   another's settlement.
2. **Condition 2 (pinned evidence).** The precondition re-derives resolution and
   pin membership from the store itself; the controller pins chain-minted PASS
   evidence before calling settle (controller.rs:2481–2487), and the pin is a
   durable fact, not the record's say-so.
3. **Condition 3 (authority chain).** Over the FROZEN unit (loaded from the
   durable run's `ReferenceVersion` through the digest-verifying
   `FrozenReferenceProjection`, never a caller snapshot — `frozen_unit`,
   settle/mod.rs:965). Permission table: critic closable by any actor;
   `lead_agent` requires orchestrator + non-blank recorded rationale (operator
   identity also accepted — the escalation model's higher tier); `human` requires
   operator + recorded identity, orchestrator refused outright. Automatic closure
   of lead_agent/human refuses AND routes to the escalation surface with pinned
   evidence (`escalate_authority_gap`, §11.5 row 6). The B.6
   `{required, exercised_by, operator_identity}` triple is recorded as an
   accompanying pinned evidence document (`settlement_authority_decision@v1`) —
   a documented mapping, since migration 0002 is frozen and carries no such
   column.
4. **Condition 4 (no blocking residual).** Tracked view first, then the durable
   checkpoint projection; on divergence the durable state wins with the typed
   `DurableBlockDiverged` refusal (settle/mod.rs:1111–1139). The durable
   residual-row read is honestly recorded as a known ports gap (module doc).
5. **Condition 5 (digest unchanged).** Settle-time `verify_unchanged` against the
   reference provider (see AK-024 below). Mutation pins BOTH digests as pinned
   evidence before the typed abort error; unavailability refuses without
   aborting (§21.3 posture).

The in-module matrix (`each_refused_condition_is_a_distinct_typed_refusal` and
per-condition tests, settle/mod.rs:2184–2916) covers every refused condition with
distinct variants. **AK-040: satisfied, composed and unit-level.**

### §16.3 — commit order, keyed events, idempotent replay

- **Order.** Adapter `commit_settlement`
  (`adapters/gauntlet-adapter-state-sqlite/src/state_store.rs:1882`): step 1
  `append_to_log` (own transaction; connection runs `journal_mode=DELETE`,
  `synchronous=FULL` — the commit is the fsync; AUTOINCREMENT offsets never
  reused; no UPDATE/DELETE anywhere in the crate), then step 2 `insert_settlement`
  (keyed row). A step-2 failure returns an error that names the durable offset and
  forbids re-running the unit (the crash window is recoverable by replay).
- **Keying.** `settlements` PK is exactly `(run_id, unit_id, verification_ref)`
  (migrations/0002_state_records.sql:40–49).
- **Idempotence/divergence.** Same-key same-facts: admission returns
  `AlreadyCommitted` with the existing offset → the service treats it as
  idempotent success (`committed_now=false`). Divergence is loud in three layers:
  ports `AlreadyCommittedDivergence` (same key, different evidence),
  ports `UnitAlreadySettled` (different key, same unit — one settlement per unit
  per run), and store-level `Corrupt` when a keyed `INSERT OR IGNORE` hit hides
  divergent facts (state_store.rs:1077–1125). Replay
  (`complete_pending_settlements`) rebuilds the row from the event's own fields
  and timestamp, bound to the event's offset, cross-validated by
  `verify_settlement_event_at` (the row must name the durable event that made it
  real, with the constructed timestamp agreeing).
- **In-memory last.** Service applies the domain `Unit::settle` only after the
  durable commit and returns the advanced projection beside the offset;
  controller projection follows the return.
- **Structural gating.** `commit_settlement` accepts only the authority-minted
  `AdmittedSettlement` (no constructor outside the ports module); the adapter's
  inherent `apply_settlement_row` is `#[cfg(test)]`; the generic
  `append_record` refuses `RecordKind::VerificationRecord` for every caller
  (COR-1/COR2-4) — the durable PASS substrate cannot be forged through a public
  inherent write.

**AK-041: satisfied** (observed by the `commit_order` proof, below).
**§16.3 keyed replay: satisfied.** Minor teeth note in RES-A1.

### §16.4 — single-writer discipline

- The answer-key writer is confined to NO module: the arch rule
  (`crates/gauntlet-arch-tests/src/single_writer.rs`) pins (A) the surface-module
  inventory (six `gauntlet-app` modules that name the ruler), (B) no unguarded
  write-API token in a surface module, and (C) an inventory of EVERY production
  write-API token in the core crates with containment guards
  (`contained_locator_path`, `write_report`, `probe_writable`). It is wired into
  `just boundaries` via `check_answer_key_single_writer`. Fail-closed blanking of
  `#[cfg(test)]` regions; unclassifiable shapes are violations.
- Unit status is written only through the governed path: the durable settlements
  row via authority-minted admission; the answer-key file itself is never written
  by the runtime (status is a projection rebuilt from events — §5.3.3).
- Placement: the key lives outside the workspace root; the loop's only
  builder-side byte write (facet-locator materialization) sits behind
  `contained_locator_path` (component-wise refusal of `..`/root/prefix — the
  g10 prevention leg proves it against the key itself). The plan's step-3
  wording "mounts the key read-only into workspaces" is delivered as
  outside-placement + write-boundary containment + the mechanical arch rule +
  the detection tripwire; spec §16.4's disjunction ("lives outside … or is
  mounted read-only") is satisfied. No sandbox/mount backend is engaged on this
  path (proof runners are `Isolation::None`, disclosed in the proofs).
- The rule's own threat model honestly records its lexical escapes (aliased
  `use std::fs::write as w`, trait-object indirection in adapters, macro-internal
  concatenation) — a declared bound, not a claim of universal absence.

**AK-043 (prevented-or-detected): satisfied** — prevention leg (composed,
typed refusal, key bytes untouched) and detection leg (composed raw write,
settle-time tripwire, aborted with both digests, no settlement) in
`g10_builder_writes_answer_key.rs`.

### §18.3 — resume completes half-applied settlements exactly once

`resume_run` re-verifies the reference first (mismatch → aborted with both
digests pinned), then `complete_pending_after_checkpoint` (anchored settlement
replay + evidence-production orphan completion re-minted from durable facts only;
unrecoverable orphans are pinned limitations, never fabrications), then
reconciles the projection from settlement rows beyond the anchor, restores budget
through the domain gate, reconstructs per-unit attempts from durable producer
writes (with honestly recorded under/overcount caveats), and re-probes in-flight
invocations (`Failed{orphaned_by_restart}` unless a valid result contract is on
disk). The p7 proof drives this over real `std::process::abort()` child kills at
all three §16.3 windows with a fresh composition, asserting exactly one event and
one row per unit, zero re-dispatch of settled work (zero scripted steps and empty
prompt recordings), and survival of run identity, pinned evidence, and the
observation binding. **Task-13's §18.3 sentence: satisfied.** The section's steps
4 (workspace sweep) and 5 (preflight re-run) are not implemented — adjacent scope,
RES-A4.

### §21.3 — disposition routing; only integrity violations abort

- Reference mutation (integrity): terminal `aborted{reference_mutated}`, both
  digests pinned (boundary path controller.rs:1173; settle-time path
  settle/mod.rs:1145 — the only abort rows on this surface, plus
  `ContractMismatch` as a §11.6 integrity abort in `dispose_failure`).
- Reference unavailability: pinned gap, loop continues, resume-able
  (`boundary_verify`, `plan_and_dispatch` read-failure arm) — the §21.3 row-1
  posture.
- Builder invocation failures: malformed → reminder/escalate; transient (incl.
  `OrphanedByRestart`) → backoff; timeout → retry-once; all via
  `control/disposition.rs`. Agent session failure never marks a unit settled.
- Settlement gate refusals: pinned forensic evidence, unit stays open, run
  continues (controller.rs:2562–2574).

One disposition gap noted: verifier-side mechanical failures (rung breaks; no
rung can decide) unwind `start_run`/`resume_run` as `ControlError` rather than
routing through a §11.5/§21.3 row — RES-A2 (minor).

### AK-024 settle-time half — no tautology

`settle_time_verify` consults `reference.verify_unchanged(run.reference_version())`;
`FileReferenceProvider::verify_unchanged`
(`gauntlet-app/src/reference/provider.rs:292`) re-reads the answer-key file from
disk and re-derives the digest (`digest_of(&bytes)`), comparing against the
frozen version's digest. The current digest is the provider's own file-derived
answer — not a value the controller froze in the same process. Broken/missing
artifacts are typed `Unavailable`, not `ReferenceMutated` (correctly not
conflated). The g10 detection leg composes this end-to-end (builder raw-writes the
key mid-run; run aborts; both digests pinned; no settlement lands over the mutated
ruler — the tampered bytes are preserved as evidence). The iteration-boundary leg
(`boundary_verify` at every iteration top) is also composed. **Satisfied.** Digest
scope note in RES-A3.

### AK-031 / P11 — critic is not verifier

Four honest no-record shapes through the real composed controller (broken rung,
no-rung-can-decide, INCONCLUSIVE, FAIL over a clean comparison) plus a PASS
control leg. Core assertions are durable-absence: no `SettlementCommitted` event,
no settlements row, NO verification record (the comparator's pass never minted
one), projection Open at the last durable checkpoint. The INCONCLUSIVE and FAIL
forensic pins name their outcomes distinctly (three states never collapsed).
**Satisfied.**

### AK-062 — service-level half

`settle_operator_action` (settle/mod.rs:742) hits the same durable-PASS gate
first (`SettleError::VerificationNotPass` — the same variant the automatic
path's admission produces, with FAIL/INCONCLUSIVE named as themselves), and even
with PASS evidence and a recorded identity it refuses with
`OperatorActionRequiresEarnedReadiness` — there is no operator bypass at this
layer, structurally. Unit tests pin both arms plus JSON-injection resistance of
the identity. The positive operator surface (same atomic commit sequence through
action endpoints) is out of scope by plan (Task 21). **Service-level half:
satisfied.**

### AK-053 — support assessment (plan does not target it here)

Supported in substance: the §18.2 checkpoint carries every required field (run
id, reference digest, iteration, unit-state projection, budget spent, in-flight
ids + result-contract paths, event-log offset — `ports/src/state_store.rs:107`,
written at every iteration top plus close-out, controller.rs:3047), and resume
reconstructs from durable state only (no conversational memory). Partial gaps:
§18.3 steps 4–5 absent; retry-ledger halves (timeout retries, reminders, staged
attachments) are recorded as L2 ports-revision limitations with honest caveats.
Not a Task-13 finding; recorded so the tree is not overcredited (RES-A4).

## §23.1 honesty of the three proofs (would each pass with its mechanism removed?)

- **p7 `commit_order`** (AK-041): observed, not inferred — a delegating
  order-probe brackets every `commit_settlement` with before/after event/row
  counts over the store's own export, asserts the row cites the exact returned
  offset, and brackets the commit with checkpoints by global write-order stamp.
  Removing the durable commit (pre-B2 in-memory-only shape) fails
  `commits.len()==2`; reordering the store's steps is refused by
  `verify_settlement_event_at` cross-validation, so the run cannot settle and the
  test fails. Residual subtlety: an in-memory-advance-before-commit reorder
  *within* one iteration, with no intervening checkpoint, is below the bracket's
  granularity (RES-A1, minor). The current code orders it correctly by
  construction.
- **p7 crash legs** (AK-042): real child-process `abort()` kills at all three
  windows against the same database file; fresh composition; the production
  `resume_run`. The between-window leg restarts with ZERO scripted steps and
  asserts empty prompt recordings (reconciliation alone must complete — any
  dispatch is a §18.3 violation). Exactly-once assertions are over the durable
  export and rows, read through the fresh composition's handles. Removing replay
  completion, re-dispatch suppression, or the exactly-once keying each fails a
  named assertion. Real teeth. Honest scope note: the store-internal
  event-vs-row interleave is covered by the real kill plus the adapter's own
  suite, not re-proved at the composition seam.
- **p11** (AK-031/P11): the verdict rung is the controlled variable (the proof's
  declared purpose); every load-bearing assertion is durable-absence downstream
  of the comparison's acceptance, with a PASS control leg attributing the
  no-settle outcomes to the verification gap. Layered defense means removing any
  single PASS gate (service entry, precondition, admission) still leaves the
  others — removing all of them is required to make a leg settle, and then the
  durable-absence assertions fail. Not vacuous: the proof pins the composed
  behavior, and the forensic naming assertions additionally discriminate
  INCONCLUSIVE from FAIL.
- **g10** (AK-043): prevention leg fails without `contained_locator_path`
  (the typed-error match panics; key bytes change), detection leg fails without
  the settle-time provider consult (settlement would land before the next
  boundary check → the no-`settlement_committed` assertion fails; breach records
  missing). A tautological consult (frozen digest vs itself) would return Ok and
  the aborted-terminal assertion would fail. Real teeth on both legs.

Teeth *records* were not re-run (orchestrator's job; I am static-only).

## Residuals

## Residual RES-A1
- **Dimension:** evidence
- **Severity:** minor
- **Locus:** `crates/gauntlet-cli/tests/p7_recovery.rs:1372-1410` (the checkpoint
  brackets of `commit_order`)
- **Contract Ref:** AK-041; plan Task 13 done-when ("reordering the commit steps
  makes P7 fail")
- **Defect:** The commit-order proof discriminates (a) commit-existence, (b)
  event-before-row (store cross-validation), and (c) projection-durability across
  checkpoint boundaries, but an in-memory-advance-before-durable-commit reorder
  occurring inside a single iteration with no intervening checkpoint write would
  produce identical observables. Checkpoints are written at iteration
  boundaries; the settle happens mid-iteration in the drain, so the bracket's
  granularity cannot see an intra-iteration swap of the advance and the commit.
- **Reproduction sketch:** in a mutated controller, move
  `state.unit_states.insert(unit, Settled)` before `self.settlement.settle(…)`
  while keeping the commit; run `commit_order`. The last checkpoint before the
  commit still shows the unit open (it predates both), the first after shows it
  settled — all assertions pass despite the reorder. (Static analysis only; the
  reorder experiment is an open execution question for the orchestrator. The
  store-side reorder IS discriminated — `verify_settlement_event_at` refuses a
  row that precedes its event, so the run cannot settle.)

## Residual RES-A2
- **Dimension:** functional
- **Severity:** minor
- **Locus:** `crates/gauntlet-app/src/control/controller.rs:2456-2458`
  (`verify_claim` error → `ControlError::VerificationRefused`, unwinds the run);
  `crates/gauntlet-cli/tests/p11_critic_is_not_verifier.rs:925-964` (legs 1–2)
- **Contract Ref:** spec §18.4 ("each has a defined disposition in §11.5 and
  §21"), §21.3 (non-integrity failures keep the process alive), §11.5 row 2
  (transient tool failures → backoff)
- **Defect:** A verification instrument's mechanical failure (rung breaks) or an
  undecidable requirement (no rung can decide) surfaces as a hard `ControlError`
  that unwinds `start_run`/`resume_run`, leaving the durable run without a
  terminal state, rather than being routed through a disposition row
  (retry/backoff, or INCONCLUSIVE-with-class as the §12.3/P12 pattern does for
  observation gaps). Recovery is correct — resume re-plans the unit, nothing
  settled, no false terminal — but the degradation table has no explicit row for
  this class, and the failure mode stops the control call rather than degrading.
- **Reproduction sketch:** compose with a rung whose `verify` returns
  `InstrumentFailed`; call `start_run` → `Err(ControlError::VerificationRefused…)`
  (an error, not a `RunOutcome`); the durable run has no terminal and no
  disposition record for the verifier-side failure.

## Residual RES-A3
- **Dimension:** functional
- **Severity:** minor
- **Locus:** `crates/gauntlet-app/src/reference/provider.rs:208-212` (digest over
  answer-key bytes only), `:292-326` (`verify_unchanged` re-reads the key only)
- **Contract Ref:** spec §9.4 step 1 ("manifest + answer key + comparison
  contract"), §9.5 (the comparison contract "is frozen with the reference"),
  I1
- **Defect:** The frozen `ReferenceVersion` digest covers the normalized
  answer-key bytes only. A mid-run mutation of the manifest (or of the
  comparison contract, which is composed separately through `ComparisonDeps`)
  is not caught by `verify_unchanged` at the boundary or at settle time. This is
  a reference-freeze scope property that predates Task 13; AK-024's settle-time
  half is satisfied for the digest that exists, and the settle-time consult is
  genuinely provider-file-derived. Recorded so the ruler's coverage is not
  overclaimed.
- **Reproduction sketch:** freeze a run; edit the manifest's
  `minimum_reliability` mid-run; `verify_unchanged` still returns Ok and
  settlement proceeds.

## Residual RES-A4
- **Dimension:** functional
- **Severity:** minor
- **Locus:** `crates/gauntlet-app/src/control/controller.rs:842-979` (`resume_run`)
- **Contract Ref:** spec §18.3 steps 4–5 ("Sweep workspaces for settled/
  out-of-scope units", "Re-run preflight and continue"); AK-053
- **Defect:** Resume implements §18.3 steps 1–3 (plus budget restore and attempts
  reconstruction) but not step 4 (workspace sweep) or step 5 (preflight re-run).
  AK-053's checkpoint-sufficiency substance is present (all §18.2 fields; resume
  reconstructs from durable state only), so the tree *partially* supports AK-053;
  the plan does not target it in Task 13 and P7's claimed sentence (step 1,
  exactly-once, never re-run) is implemented and proven. Recorded as a scope
  note, not a Task-13 failure.
- **Reproduction sketch:** `rg "sweep|preflight" controller.rs` — no §18.3 step-4/5
  logic exists in `resume_run`.

## Residual RES-A5
- **Dimension:** architecture
- **Severity:** minor
- **Locus:** `crates/gauntlet-cli/src/main.rs:19-40`
- **Contract Ref:** spec §20.1 (`gauntlet run`, `gauntlet resume <run_id>`);
  plan Task 20
- **Defect:** No operator command reaches the composed controller/settle path
  yet — the CLI ships `doctor` and `reference digest` only. The Task-13
  capability is reachable as a library composition root (exactly what the three
  proofs drive), which is the intended pre-Task-20 path, but a confirmation
  reader should not infer an operator-invocable settle/resume surface exists
  today.
- **Reproduction sketch:** `gauntlet resume <id>` → "unknown command" (exit 2).

## Observations (no residual graded)

1. **Post-commit side-record failure shape.** `record_authority_decision` runs
   after the durable commit; if its evidence pin fails, `settle` returns
   `Err(Evidence)` and the controller unwinds without advancing the in-memory
   projection — but the settlement IS durable, and resume reconciliation marks
   the unit Settled from the row beyond the anchor. Self-healing; no §16.3
   violation (the mandate is order, not post-commit side-record infallibility).
2. **Event-level settlement forgery surface (for the falsifier's threat model,
   not a spec violation).** `StateStore::append_event` accepts
   `RunEventKind::SettlementCommitted` with arbitrary facts, and replay completes
   rows from events (demonstrated by `crash_helper settle after-append`). Within
   the declared threat model — model output untrusted, stores held only by
   trusted application-layer code, adapters outside the core boundary — this is
   by-design event sourcing; the single-writer guarantee at the ports level rests
   on process/adapter containment. Worth Role B's attention; I flag, not fix.
3. **Symlink discipline division of labor.** `contained_locator_path` is
   component-wise (no canonicalization/symlink follow at that site); §17.5.2's
   symlink-escape checking is delegated to the WorkspaceProvider contract suite
   (cited by the single-writer module doc). I did not audit that suite (outside
   my declared bound).
4. **Honest disclosure quality.** The proofs' composition inventories name every
   proof-local replacement and why; the controllers/services carry fail-first
   correction annotations (COR-*, C*, N*, F*) tied to named probes; known gaps
   (durable residual-row read, retry-ledger L2, slicing unreachability) are
   recorded in place rather than papered over. This is the §11.6/§23 discipline
   working as specified.

## Declared search bound

**Examined (static):** the sources listed under "Scope"; `settle/mod.rs` service
body + test inventory + selected tests (AK-062, COR-2, matrix);
`control/controller.rs` — compose, start_run, resume_run, drive, boundary_verify,
reference_breach_outcome, plan_and_dispatch (read-failure arm), dispose_failure,
dispose_transient, process_completion settle region, checkpoint,
contained_locator_path, escape_json_text; `verify/precondition.rs` (full);
`verify/authority.rs` — for_run, GovernedVerification, verify_claim, frozen_unit,
binding_class; `ports/src/state_store.rs` (types + trait surface + fail-closed
defaults); `ports/src/producer_authority.rs` (settlement admission section);
sqlite adapter — commit_settlement, settlements, complete_pending_settlements,
insert_settlement, verify_settlement_event_at, apply_settlement_row gating,
append_record gating, event_log.rs (full), migrations/0002 (settlements DDL),
crash_helper.rs (full); arch-tests single_writer.rs (rules + inventory + threat
model) and lib.rs wiring; justfile `boundaries`; all three proof files (p7:
fixture structure + all five legs + helper; p11: full; g10: full);
ports contract/state_store.rs (settlement cases, surface scan); gauntlet-cli
main.rs (full); git status/log (artifact-committed check).

**Not examined:** controller in-module test bodies beyond the settle region;
scheduler/select/diagnosis/disposition module internals; observe/compare module
internals; `ports/src/contract/stubs.rs` in full; `verification_record.rs` port
in full; adapters other than state-sqlite (evidence-fs, observe-command,
compare-diff/model, agent-mock) beyond their proof-facing use; the
WorkspaceProvider contract suite; docs/CONFORMANCE.md; spec sections outside the
brief's list; teeth records; any other task's artifacts. No tests were executed
(static-only round).

**Open execution questions for the orchestrator:**
1. Re-run the plan's teeth record for AK-041 (reorder experiment) — my static
   reading says the store-side reorder fails loudly and the commit-removal tooth
   fails `commit_order`, but RES-A1's narrow intra-iteration reorder is not
   discriminated by any test I read.
2. Whether a verifier-side instrument failure (RES-A2) is intended to gain a
   disposition row in a later task, or is accepted as a control-level error.

## Verdict

**VERDICT: CONFIRM**

Reasons: within the declared bound, every Task-13 claim — AK-040, AK-041, AK-042,
AK-043, AK-062's service-level half, AK-031/P11, AK-024's settle-time half — holds
through the composed production path (controller → SettlementService → ports
authorities → sqlite adapter), each mechanism is reachable and load-bearing on the
path the shipped controller and settle service actually take, and the three proofs
are composed over real durable stores with genuine discrimination teeth on their
named mechanisms (one minor teeth-granularity gap, RES-A1). No blocking or major
residual was found. Five minor residuals recorded (RES-A1..A5), none of which
breaks a claimed unit. Bounded claim: this is a static, read-only confirmation
over the sources named above — not an execution-backed one, and not a claim of
absence beyond that bound.
