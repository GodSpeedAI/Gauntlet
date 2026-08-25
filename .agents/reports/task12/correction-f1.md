# Task 12 — correction FINISHER slice F1 — result contract

Scope: `correction-brief-f1.md` items 0–6. Single-compiler build lock held
throughout; no concurrent cargo/nextest; no sub-agents. No commits. No edits
to `.wayfinder/**`, `.agents/specs/**`, `.agents/plans/**`, `AGENTS.md`,
`justfile`, `deny.toml`, `.agents/CURRENT_STATUS.md`, the frozen prereg
files, `.agents/prompts/**`, `.tmp/**`, `.jolli/**`.

Per-item evidence, fail-first observations, and gate outputs are in
`.agents/reports/task12/correction-checkpoints.md` (entries "Correction
round 1 — FINISHER slice F1"). This file is the contract summary.

## Status by item

| Item | Subject | State |
|---|---|---|
| 0 | C5 — verify and record | **VERIFIED AND RECORDED** (implemented by the cancelled builder, not by F1) |
| 1 | C6.1 — over-capacity scope refusal skips-and-continues | **DONE** |
| 2 | C6.2 — full graph membership, separate eligibility | **DONE** |
| 3 | C7.1 — per-tick re-read, status leg | **RECORDED** (SPEC_ONLY_NO_SHIPPED_SURFACE, pinned) |
| 3 | C7.2 / C7.3 — membership legs | **STOPPED — CONFLICT** (nothing weakened; see below) |
| 4 | C6.4 — record L4 honestly | **DONE** (record + pinning test) |
| 5 | C6.3 — cohort charge/prompt consistency | **DONE** (typed refusal) |
| 6 | N6 — remove dead `compose_with_policy`, lint green | **DONE** |

## Item 0 — C5 verified, not re-implemented

All five sub-parts are present **and asserted**. F1 wrote no code here; the
checkpoint entry is labelled as landed-by-the-cancelled-builder and
verified by F1.

- C5.1 `correction.rs:448-450` refusal, `:381-387` variant, `:572-596`
  pinning test (matches the typed variant AND asserts the Display names
  the vacuity).
- C5.2 `controller.rs:1793-1798` (correcting iteration, inside the
  `RetryWithResidual` arm) and `:1806-1810` (completion path).
- C5.3 `controller.rs:2004-2018` non-zero `per_attempt_estimate` forward
  ask on the residual path; `:2043-2077` a real `Bounded` + `Speculative`
  candidate pair through one `candidate_of` closure.
- C5.4 `controller.rs:2053` `required_authority: domain_unit.authority()`
  vs `:2085-2089` `exercisable_authority: Authority::Critic`. Not raised.
- C5.5 `controller.rs:2090-2093` reads `dispatchable_tier_capacity`
  (`:2309-2316`); the `[Contained, Local]` literal is gone.
- Composed: `p4:1252-1296`, `p6:933-953`.

Bounded claim: no sub-part was found absent within the five named loci and
the two composed proof sites. Not a claim that C5 has no other unexamined
facet.

## Item 1 — C6.1

Two fail-first observations. The first exposed a **fixture-locality defect
that had to be closed before the real defect was observable**: the in-crate
`FakeMeter` answered only the §5.3.7 reserve conjunct and carried no
outstanding-reservation accounting, so it could never reproduce the shipped
`MeteredBudget`'s `check_dispatch_scope` refusal — which is why critic-B's
A3 escape was invisible in this crate. `FakeMeter` now models the shipped
contract (reserve gate first, then `check_dispatch_scope(outstanding + 1,
1)`; a booked charge releases the slot). The same test then observed
`Meter(Reserve { cause: ScopeExceeded { parallel: 3, depth: 1 } })`.

Fix: new `DispatchAdmission { Admitted, TruncatedAtCapacity, EmptySlice }`;
`dispatch` matches the refusal **by cause** — only
`BudgetError::ScopeExceeded` becomes a truncation, every other
`BudgetMeterError` still propagates; `record_dispatch_truncation` pins a
`dispatch_truncated` JSON record naming run, iteration, unit, and the
refused scope numbers; `plan_and_dispatch` breaks on truncation. The
genuine forward-ceiling breach is untouched — it is caught earlier by the
summed pre-dispatch consult and still routes to C1/C2's
`budget_exhausted`.

Grounding quoted in the fix's doc comment: §10.4 (`Core-spec.md:777`,
verbatim), §21.3 (`Core-spec.md:1238` — "Integrity violation" is the only
abort row), §11.2 (the meter check is a per-Unit eligibility conjunct).

## Item 2 — C6.2

Fail-first (composed): `Scheduler(UnknownDependency { unit: "AK-9102",
dependency: "AK-9101" })`. Fail-first (the warned-about trap, **observed
rather than assumed**): with full membership restored but `status_of`
temporarily neutralized to the parsed status, the new scheduler test failed
`DependencyUnresolved { ... status: Some(Open) }` — `UnknownDependency`
traded for `DependencyUnresolved`, exactly as predicted. Scheduler restored
byte-identically (`diff` clean).

Design: the cancelled builder's recorded `statuses` projection, adopted,
with two recorded refinements — `validate_graph` is left unchanged (full
membership is what fixes it; a second status-aware membership check would
be a second source of truth), and `status_of` **falls back** to the parsed
status when the projection carries no entry, so a caller planning over an
unstarted run needs no bookkeeping.

**Which source wins for a queued unit** (the answer shared with item 3):
durable closure is **sticky**, otherwise the file wins; neither source can
re-open what the other closed. §11.4 step 3 gives the file the open
direction; §5.3.2 (`status = settled` requires a `VerificationRecord` with
`Outcome = PASS`) plus §18.3 ("Re-running settled work is a defect") give
durable settlement the closed direction. `state.unit_states` holds the
reconciliation; `PlanInput.statuses` is its projection.

## Item 3 — C7

### C7.1 — recorded, not composed

Bounded search over two shipped surfaces: the answer-key parser's field set
is frozen at `{claim, verify_by, blast_radius, authority,
evidence_required, depends_on}` and typed-refusing (`reference/parse.rs:40-49`),
and `Unit::new` hardwires `status: UnitStatus::Open`
(`gauntlet-domain/src/unit.rs:351`). **Not found within those two
surfaces** — not universal absence. Consequence: an operator hand-edit
marking a queued unit `settled`/`out_of_scope` is not "silently ignored";
it is a hard parse refusal, and no shipped parse can yield another status.

Deliverable per the L4 precedent and §11.6: a doc record on
`reread_answer_key` plus a **pinning test** so the unreachability is
asserted, not prose. Also recorded: every other field the file can express
is already authoritative for queued units, because eligibility re-runs each
iteration over the freshly parsed `Unit` values.

### C7.2 / C7.3 — STOPPED, conflict recorded, nothing weakened

Fail-first was done first, under §21.3's unavailability window (the only
window in which a mid-run answer-key edit changes what the loop does rather
than aborting it, since §9.4 puts the answer key inside the frozen digest):
C7.2 observed `left: BudgetExhausted, right: Settled`; C7.3 observed
`left: 1, right: 2` prompts against a `Settled` terminal. **Both mechanism
defects are real.**

The briefed fixes were then implemented and produced a third observation
that changes the disposition: with the seed in place, C7.3's run failed
`VerificationRefused(UnitUnavailable { unit: "AK-9102", detail: "no block
with that id exists in the frozen reference (digest sha256:88eca9a6…)" })`.

**The conflict.** The run's settleable membership is the **frozen
reference's**, not the file's: `VerificationAuthority::frozen_unit`
(`verify/authority.rs:368-389`) parses the freeze snapshot and refuses any
unit absent from it — a standing assertion in
`verify::authority::tests::a_unit_absent_from_the_frozen_reference_is_refused`
(`authority.rs:1044-1060`). Therefore:

- **Pruning** `unit_states` when a unit leaves the FILE lets `all_closed`
  report `settled` for a unit **of the frozen ruler** the run never
  observed, compared, or verified — a settlement lie, worse than the
  imprecise `budget_exhausted` it replaces, and a weakening of §5.3.2/§9.4.
- **Seeding** a unit that entered the FILE admits work the verification
  authority correctly refuses; observed result is a hard `ControlError`
  with no terminal and no close-out checkpoint (the critic-B A2 shape).

Neither leg is fixable without weakening a standing assertion or **deciding
a §21.3 degradation policy** — and `residual-readjudication.md` explicitly
deferred that surface this round ("an unforced policy change to a surface
no critic finding requires moving. Trigger: Task 13 recovery composition,
or the next §21.3 degradation-policy touch"). Choosing what the loop owes
when the file and the frozen ruler disagree *is* that decision.

Per the brief's hard rule I stopped. The prune/seed implementation and its
two tests were removed from the tree; the observations above are the
evidence of record. `reread_answer_key`'s doc now states the boundary: the
re-read governs **cancellation** and what is dispatchable now — never what
the run owes.

**New deferral candidate raised by this work** (recorded here, not written
into any ledger F1 may not edit): a unit added mid-run under the §21.3
unavailability window is dispatched today and then hard-errors at
verification — a terminal-truth defect of the C1 family (no terminal, no
close-out checkpoint, no report), sitting on the deferred §21.3 surface.
Suggested trigger: the same one as the answer-key-read-failure item —
Task 13 recovery composition, or the next §21.3 degradation-policy touch.

## Item 4 — C6.4

Record landed at **both** `cohorts: BTreeMap::new()` sites: §11.3
(`Core-spec.md:835`) states the criterion and names no marker; the frozen
ruler's field set is closed; `reference.yaml` carries no grouping key; the
parser's field set is closed and typed-refusing; `domain::Unit` carries no
cohort. The consumer is real and §11.3-cited in `scheduler.rs`; the
**producer** does not exist, so the map is empty by structure rather than
by choice. Bounded claim, Task 20's E2E reference fixture named, §11.6
cited as why the record is mandatory.

Deliberate small extension of the briefed deliverable: one pinning test
(`the_answer_key_cannot_express_a_cohort_on_any_shipped_surface`) so the
record cannot go stale silently — same reasoning as C7.1's. No cohort
source invented; `.wayfinder/**` and the parser's frozen field set
untouched.

## Item 5 — C6.3

Fail-first: the guard was first landed **permissive** (today's exact
behaviour) and the test observed the silent drop —
`a slice this controller cannot dispatch is refused, never truncated:
Some(UnitId { raw: "AK-9101" })`, with AK-9102 dropped while the summed
estimate for both was reserved.

Choice: **loudly refused**, typed, carrying the failure locus. New
`ControlError::SliceNotDispatchable { units }` and
`fn dispatchable_member(&PlannedDispatch)`, consulted **before** the
reserve so a refused slice charges nothing. Rejected alternatives, and why:
composing real cohort dispatch is §11.3 slicing, which item 4 showed has no
shipped input and which the plan assigns to Task 20; making a multi-member
`PlannedDispatch` unconstructible would delete the scheduler's §11.3
capability to serve the controller's current limitation. A silent drop
would stop being latent the moment Task 20 composes a cohort source, and it
would stop silently — a typed refusal makes that a visible failing change.

## Item 6 — N6 and the rest of the lint gate

`compose_with_policy` deleted after confirming zero references repo-wide.
Fail-first is the warning itself, captured verbatim in the checkpoints.

`just lint` was red for three further pre-existing reasons the brief did
not enumerate; all three are dead-code/mechanical and none weakens
anything:

1. `p6_budget_control.rs` — two unused imports (`BudgetError`,
   `Remaining`) left behind when C2 deleted `RoundsSupplementedMeter`.
2. `p4_corrective_loop.rs:990` — `arc_with_non_send_sync` on C4's
   `Arc::new(CommandObserver::from_parts(...))`. Annotated with the
   repository's own established idiom for this exact case, already used two
   statements earlier in the same file and in p2, p6, p12, and
   `controller.rs`. A missed annotation, not a new decision.
3. `cargo fmt --check` diffs (some mine, some pre-existing C3/C4 drift) in
   `controller.rs`, `scheduler.rs`, `diagnosis.rs`, `p4_corrective_loop.rs`,
   `p6_budget_control.rs`. `cargo fmt` touched only those five files.

## Files touched

- `crates/gauntlet-app/src/control/controller.rs`
- `crates/gauntlet-app/src/control/scheduler.rs`
- `crates/gauntlet-app/src/control/diagnosis.rs` (rustfmt only)
- `crates/gauntlet-cli/tests/p4_corrective_loop.rs`
- `crates/gauntlet-cli/tests/p6_budget_control.rs`
- `.agents/reports/task12/correction-checkpoints.md`, this file

## New and changed tests

| Test | Item |
|---|---|
| `control::controller::tests::an_over_capacity_fan_out_is_truncated_and_recorded_never_a_run_error` | 1 |
| `control::controller::tests::a_settled_dependency_stays_in_the_graph_while_eligibility_filters_it` | 2 |
| `control::scheduler::tests::the_durable_projection_decides_status_for_membership_and_dependencies` | 2 |
| `control::controller::tests::the_answer_key_cannot_express_a_unit_status_on_any_shipped_surface` | 3 (C7.1) |
| `control::controller::tests::the_answer_key_cannot_express_a_cohort_on_any_shipped_surface` | 4 |
| `control::controller::tests::a_multi_member_slice_is_refused_rather_than_charged_for_and_dropped` | 5 |

Fixtures: `answer_key` extended to a third unit; new
`answer_key_with_dependency()`; new named figure `OVER_CAPACITY_UNITS =
FIXTURE_PARALLELISM + 1`; `FakeMeter` aligned with the shipped meter's
scope contract.

## Teeth note

Tooth **T3** breaks `reread_answer_key`. Its executable body is
**unchanged** by F1 — the prune/seed was reverted and only the doc comment
was added — so T3's break and its pinning test
(`p4_corrective_loop a_mid_run_answer_key_edit…`, still passing) are
unaffected. Note for the re-record: the p4 target now holds 4 tests, not
the 3 T3's revert line cites; that count changed under C4, not here.

No other tooth's break site was touched. `dispatch`'s signature changed
(returns `DispatchAdmission`) and `PlanInput` gained a field — any tooth
scripted against those exact lines needs its break re-expressed, but no
tooth in `teeth.md` names them.

## Deliberately left alone

- **critic-A m7 / tier posture**: `dispatchable_tier_capacity` still admits
  cross_cutting at the global parallel bound. Declared Task-14 scope;
  touching it regresses C5.5.
- **The answer-key-read-failure error policy** (`ControlError::Reference`
  inside the loop) — newly deferred by `residual-readjudication.md`.
- **The completion path's zero forward ask** — recorded intent, per brief.
- **`exercisable_authority: Authority::Critic`** — the §17.1 ceiling today;
  raising it is L1/Task 14.
- Everything assigned to slices F2/F3 (C8.\*, C9.2–C9.4, N1–N5).

## Gates (verbatim final lines)

```
$ just check-package gauntlet-app
cargo check -p gauntlet-app --all-targets
    Checking gauntlet-app v0.1.0 (/home/sprime01/projects/gauntlet/crates/gauntlet-app)
    Finished `dev` profile [unoptimized] target(s) in 8.90s

$ cargo nextest run -p gauntlet-app control::
     Summary [   0.403s] 64 tests run: 64 passed, 225 skipped

$ cargo nextest run --test p4_corrective_loop --test p6_budget_control
     Summary [   1.042s] 6 tests run: 6 passed, 0 skipped

$ just lint
cargo clippy --workspace --all-targets -- -D warnings
    Checking gauntlet-app v0.1.0 (/home/sprime01/projects/gauntlet/crates/gauntlet-app)
    Checking gauntlet-cli v0.1.0 (/home/sprime01/projects/gauntlet/crates/gauntlet-cli)
    Finished `dev` profile [unoptimized] target(s) in 28.26s
cargo fmt --check
```

Workspace-wide gates are left to slice 3 / the orchestrator, per brief.
