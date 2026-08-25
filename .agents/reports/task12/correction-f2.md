# Task 12 — correction FINISHER slice F2 — result contract

Scope: `correction-brief-f2.md` items 0–9, IN ORDER. Single-compiler
build lock held throughout; no concurrent cargo/nextest; no sub-agents.
No commits. No edits to `.wayfinder/**`, `.agents/specs/**`,
`.agents/plans/**`, `AGENTS.md`, `justfile`, `deny.toml`,
`.agents/CURRENT_STATUS.md`, the frozen prereg files,
`.agents/prompts/**`, `.tmp/**`, `.jolli/**`.

`./scripts/reconcile-checkpoints.sh 12` was run FIRST ("OK — no
task-surface file was modified after the last checkpoint write"), and the
baseline `cargo nextest run -p gauntlet-app control::` was green (64/64)
before any edit. Per-item fail-first evidence, designs, and gate outputs
are in `.agents/reports/task12/correction-checkpoints.md` (entries
"Correction round 1 — FINISHER slice F2"). This file is the contract
summary.

## Status by item

| Item | Subject | State |
|---|---|---|
| 0 | C7 remainder — dispatch membership bounded by the FROZEN reference | **DONE** |
| 1 | C8.1 — real `event_log_offset` at every append site | **DONE** |
| 2 | N3 — strict checkpoint offset READ path (sqlite) | **DONE** |
| 3 | C8.2 — checkpoint at the iteration BOUNDARY | **DONE** (p6 exact arithmetic survived, unedited) |
| 4 | C8.3 — resume restores per-unit attempts | **DONE** (conditional answered: NOT activated) |
| 5 | C9.2 — escape escalation-evidence JSON (one shared helper) | **DONE** |
| 6 | C9.3 — no silent result drop on charge refusal (BOTH sites) + reservation release | **DONE** |
| 7 | C9.4 — bounded drain (derived bound) | **DONE** |
| 8 | N4 — evidence-locator containment at the write boundary | **DONE** |
| 9 | N5 — the durable §15.1 record reports FORWARD headroom | **DONE** |

No STOPPED items; no assertion, gate, or frozen surface was weakened.

## Item 0 — the C7 remainder, per the post-F1 ruling

C7.2/C7.3 were NOT implemented as originally specified (the ruling: they
were mis-specified). The real residual — F1's blocker 2 — is fixed with
ONE guard: `plan_and_dispatch` filters the per-tick file re-read to units
present in the FROZEN reference, before the owes-work predicate and the
plan input. A file-only unit can no longer make the loop owe, estimate,
plan, or dispatch it; the mutation is handled where it already is, by the
boundary abort.

- Frozen-membership surface used (recorded per the brief): the PORT's
  governed `FrozenReferenceProjection::load_version` over the durable
  run's own `ReferenceVersion` (bytes at the pinned ref, verified against
  the pinned digest), parsed by the app's own
  `reference::parse::parse_answer_key` — the identical route
  `GovernedVerification::frozen_unit` takes. No port widened;
  `frozen_unit` stays crate-private to the verify module.
- Derived ONCE per run into `LoopState.frozen_membership` (start/resume)
  — the frozen bytes cannot change mid-run. New typed
  `ControlError::FrozenMembership` for an underivable freeze snapshot.
- Fail-first reproduced F1's blocker 2 verbatim
  (`VerificationRefused(UnitUnavailable { unit: "AK-9103" … })`, no
  terminal) under the §21.3 unavailability window. Honest note, recorded
  in the checkpoint: the first test run (single-unit fixture) PASSED
  against the neutralized guard because `all_closed` ended the run before
  the window opened; the fixture was corrected to the dependency key
  before the defect was observable.
- No pruning of `unit_states` (the run's frozen debts stand), no seeding,
  no §21.3 policy change, nothing weakened.

## Item 1 — C8.1 real `event_log_offset`

Five sites threaded: `checkpoint()` now takes `&mut LoopState` and
assigns `state.last_offset` from its own `CheckpointWritten` append (the
`let _ = offset;` discard is gone); the four `append_terminal_event`
callers that dropped the returned offset (hand-edit cancellation, drain
cancellation, failure disposition, timeout disposition) assign it; the
resume caller already did. Fail-first observed the critic's exact defect
(`…not the fresh-run zero (got 0)`) on a scenario that includes a real
`InvocationTerminal` event, so both the terminal-append threading and the
checkpoint's own append are discriminated.

## Item 2 — N3 strict checkpoint offset decode

The sqlite `resume`'s checkpoint leg decodes strictly: a non-representable
stored offset is `StateStoreError::Corrupt` naming run, checkpoint seq,
and the stored value (fail-first: `…never a silent zero (got 0)` after a
direct row tamper to −1). Bounded to the checkpoint leg ONLY — the
settlement / evidence-production / event-log coercions and `settled_at`
are untouched (Task 13's surface).

## Item 3 — C8.2 the iteration-boundary checkpoint

The recorded design adopted without deviation: the per-completion
mid-pipeline checkpoint is REMOVED; `drive` writes the boundary
checkpoint at the TOP of iteration k, declaring k. One placement
satisfies both rules — the CONTENT is post-dispose of k−1 (§11.1's
iterate order; a crash anywhere in k resumes with all of k−1's outcomes
durable), and the declaration of k is the producer-authority anchor
iteration k's owner writes validate against (the hazard the brief named;
the multi-iteration composed-diagnosis tests and p4/p6 exercise iteration
≥ 2 owner writes and stayed green). The close-out checkpoint stays and
declares the same iteration as the last boundary checkpoint.

- Fail-first: `a_crash_after_a_settlement_never_re_runs_settled_work`
  observed `left: 2, right: 1` — the settled unit re-dispatched after
  resume because the latest durable checkpoint predated the settle.
- **Proof arithmetic survived UNEDITED**: p6's
  `checkpoints == FORWARD_ROUNDS_CEILING + 1` (5 boundary + 1 close-out —
  the rounds terminal fires in `decide_progress` at the end of iteration
  5, so the loop runs exactly 5 iterations) and p4's `>= 2` both pass
  with no proof-assertion edits.

## Item 4 — C8.3 attempts restored on resume

`resume_run` reconstructs per-unit `attempts` from the loop's own durable
producer writes — `evidence_productions_at(run, locator)` over each
unit's FROZEN locator (from item 0's projection load). Fail-first:
`left: 4, right: 2` — without restoration the ladder fully re-arms and
the extra attempts even fed the non-convergence detector. Two caveats
recorded at the code (facet-less completions undercount — conservative;
units sharing a locator share a count); retry-ledger persistence stays
L2 (§18.2 field set is spec-fixed). **The brief's conditional answered:
the "checkpoint-only run projection indistinguishable" ledger item is NOT
activated** — the resume reads exactly two durable shapes, each
self-identifying by kind (`Checkpoint` via `resume`; production rows via
`evidence_productions_at`); nothing needs to know whether other data
exists outside the checkpoint.

## Item 5 — C9.2 escaped escalation evidence

One shared `escape_json_text` (backslash first, then quote — order is
load-bearing) + `ticket_evidence_record`, used by `submit_ticket` — the
single funnel for all three escalation terminals since C1. The two
existing ad-hoc `.replace('"', "'")` sites (reference-unavailability
record, quarantine reason) route through the SAME helper (three copies of
an escaping rule was the next defect); pre-checked that no test pins
their exact bytes, and p4/p6 re-ran green. Fail-first: the record failed
the ports' own strict JSON parser at the raw quote, verbatim.

## Item 6 — C9.3 no silent result drop (both sites)

On EITHER charge refusal, the completed result's facts are PINNED as a
`result_uncharged` forensic record (unit, invocation, requested payment,
typed refusing locus via named consts, disposition) BEFORE the terminal —
kept, recorded, settling nothing (§21.2, §10.4, §12.3's forensics
precedent). The meter-refusal path additionally RELEASES the reservation
(the parallel-slot leak). Site 1 is covered composed (payment past the
per-attempt reservation); site 2 is isolated with a new `PermissiveMeter`
fixture because the shipped mirror meter always reaches the loop budget's
refusal first — both fail-first `left: 0, right: 1` (nothing pinned).

## Item 7 — C9.4 bounded drain

The drain's pass bound is DERIVED from the two settings it already reads
and requires — `stall_timeout_ms.div_ceil(poll_interval_ms)`, one stall
window of polls — NOT a new `ControlPolicy`/Annex E setting (the brief's
blast-radius warning; a fifth REQUIRED setting would compound critic-A
m2's open minor). Past the bound (recorded design): cancel in-flight
(releasing reservations, appending cancelled terminal events with
offsets threaded), PIN a `drain_poll_bound` record, ESCALATE. Fail-first:
the drain spun until the fixture runner's own budget killed it —
`Agent(Transport { detail: "the fixture runner exhausted its poll budget
while the drain kept spinning" })`.

## Item 8 — N4 locator containment at the write boundary

New `contained_locator_path` — component-wise refusal of `ParentDir`,
`RootDir`, and `Prefix` before the `create_dir_all` + `fs::write` at the
canonical root; typed `ControlError::EvidenceLocatorEscapes { locator,
root }`. The domain's `validate_plain_ref` is UNTOUCHED (Task-3-closed
public value grammar; widening it is a ports revision, per the brief's
stop condition — not triggered). Fail-first doubly observed: the run
hard-errored at F1's bind-time containment AND the escaped file
personally confirmed on disk (`/tmp/leak-ctrl-9101.json` held the facet
bytes) — the bind-time refusal fires only AFTER the write, so the
controller-boundary guard is load-bearing.

## Item 9 — N5 forward headroom in the durable record

The fix is in `remaining_of` ITSELF, not a record-site patch: §17.4
("the recovery reserve is never spendable on forward work") makes a
reserve-inclusive `remaining()` self-contradictory next to the meter's
own `would_breach_reserve` gate, and the helper's OWN doc already
promised "forward ceilings … the ceiling arithmetic stays in the domain"
— the code was betraying its doc. Caller check performed (the brief's
conditional): every caller (the shipped meter-memory adapter, the
scheduler/controller/select test meters, the contract stubs) routes
through the helper and uses it only for RECORDS — no caller needs
raw-limit headroom. `Budget::forward_ceiling` made `pub` (single source
of the arithmetic); the helper subtracts spent from the forward ceiling
per dimension (parallel/depth pass through — scope dimensions, no
reserve). Fail-first: the port's own rewritten test observed
`left: 10, right: 8`. Contract-suite knock-ons moved WITH the semantics,
each as a stronger before/after invariant (never a raw-limit constant):
two meter-memory adapter tests and p6's refusal/zero-payment
nothing-booked assertions.

## Files touched

- `crates/gauntlet-app/src/control/controller.rs`
- `crates/adapters/gauntlet-adapter-state-sqlite/src/state_store.rs`
- `crates/gauntlet-domain/src/budget.rs` (`forward_ceiling` pub)
- `crates/gauntlet-ports/src/budget_meter.rs`
- `crates/adapters/gauntlet-adapter-meter-memory/src/lib.rs` (test pins → before/after)
- `crates/gauntlet-cli/tests/p6_budget_control.rs` (test pins → before/after)
- `.agents/reports/task12/correction-checkpoints.md`, this file

## New and changed tests

| Test | Item |
|---|---|
| `control::controller::tests::a_file_only_unit_is_never_dispatched_the_frozen_reference_bounds_membership` | 0 (new) |
| `control::controller::tests::a_fresh_run_checkpoint_carries_a_real_event_log_offset` | 1 (new) |
| `state_store::tests::a_tampered_checkpoint_offset_is_corrupt_never_a_silent_zero` | 2 (new, sqlite crate) |
| `control::controller::tests::a_crash_after_a_settlement_never_re_runs_settled_work` | 3 (new) |
| `control::controller::tests::resume_restores_per_unit_attempts_from_durable_productions` | 4 (new) |
| `control::controller::tests::an_escalation_summary_with_quotes_and_backslashes_stays_parseable_evidence` | 5 (new) |
| `control::controller::tests::a_result_whose_meter_charge_is_refused_is_recorded_not_dropped` | 6 (new) |
| `control::controller::tests::a_result_the_run_budget_refuses_is_recorded_not_dropped` | 6 (new) |
| `control::controller::tests::a_drain_that_never_terminates_is_bounded_cancelled_and_escalated` | 7 (new) |
| `control::controller::tests::an_escaping_evidence_locator_is_refused_at_the_write_boundary` | 8 (new) |
| `budget_meter::tests::remaining_is_forward_ceiling_minus_spent` (was `remaining_is_limits_minus_spent`) | 9 (rewritten) |
| meter-memory `reservations_hold_parallel_slots_until_charged`, `a_reservation_serves_more_than_one_within_bounds_charge` | 9 (pins → before/after) |
| p6 `the_meter_refuses_the_reserve_consuming_slice_and_books_nothing` | 9 (pins → before/after) |

Fixture additions (test-only, same file): `Behavior::{CompleteWithTokens,
PollError, ProgressForever}`; `Composed.unavailable` (exposes the existing
provider switch); a meter-factory parameter on `compose_full` (defaulted
at all existing call sites); `PermissiveMeter`; `payment_tokens` +
`result_document` token parameters. Suite growth: control 64 → 73;
sqlite 40 → 41.

## Teeth note (for the orchestrator's closure re-run)

- **T3 (`reread_answer_key`)**: its executable body is UNCHANGED by F2 —
  item 0's guard is in `plan_and_dispatch`, downstream of the re-read.
  T3's recorded break still compiles; its pinning test (p4
  `a_mid_run_answer_key_edit…`) is green.
- **T5 (`record_selection`'s durable write)**: untouched. N5 changes the
  VALUES inside the durable §15.1 records (`remaining_*` now forward
  headroom) — T5's test counts records, so it is unaffected, but any
  tooth scripted against record CONTENT must expect the new values.
- **T1/T4**: their recorded break site (`RoundsSupplementedMeter` in p6)
  was deleted by C2 before F2; F2 additionally rewrote the surviving
  `the_meter_refuses…` test's `remaining` pins to before/after form (N5).
  Both teeth need re-expression regardless of F2.
- **T2 (`boundary_verify`)**: untouched (C8.2 moved the checkpoint to
  BEFORE it; the verify itself is unchanged).
- **New F2 mechanisms a future tooth could bite**: the frozen-membership
  filter, the offset threading, the top-of-iteration checkpoint (pinned
  by p6's exact count + the crash-resume test), the attempts
  reconstruction, `ticket_evidence_record`'s escaping, the
  `result_uncharged` records, the derived drain bound,
  `contained_locator_path`, and the forward-headroom projection.

## Deliberately left alone

- **critic-A m7 / tier posture** (`dispatchable_tier_capacity`'s global
  bound) — Task 14; touching it regresses C5.5.
- **critic-A m5 / selector fold loser notes** — per brief.
- **The answer-key-read-failure error policy** — newly deferred by
  `residual-readjudication.md`.
- **The completion path's zero forward ask** and
  **`exercisable_authority: Authority::Critic`** — recorded intent (F1);
  L1/Task 14.
- **critic-A m4's vacuous assertion** (`p4_corrective_loop.rs`, the
  `!format!("{:?}", …).is_empty()` in the direct-selector test): NOT
  cleaned — F2 never edited `p4_corrective_loop.rs` (only
  `p6_budget_control.rs`), so the brief's "already in the file" condition
  did not fire. Left for F3/the orchestrator.
- **The frozen-REMOVED-unit leg of C7** (a unit deleted from the file
  under the window terminalled `budget_exhausted` rather than a precise
  label): STOPPED by F1 and the ruling UPHELD — pruning would let
  `all_closed` lie; nothing F2 did changes that disposition (item 0's
  filter affects only file-ADDED units; `unit_states` still holds the
  frozen debts).
- **Task-13 settlement surfaces** (`settled_at` caller-attestation, the
  non-checkpoint sqlite coercions), **L2 retry-ledger persistence**, and
  everything assigned to slices F1/F3.

## Gates (verbatim final lines — all run personally, sequentially)

```
$ just check-package gauntlet-app
cargo check -p gauntlet-app --all-targets
    Checking gauntlet-ports v0.1.0 (/home/sprime01/projects/gauntlet/crates/gauntlet-ports)
    Checking gauntlet-app v0.1.0 (/home/sprime01/projects/gauntlet/crates/gauntlet-app)
    Finished `dev` profile [unoptimized] target(s) in 15.86s

$ cargo nextest run -p gauntlet-app control::
     Summary [   0.644s] 73 tests run: 73 passed, 225 skipped

$ cargo nextest run --test p4_corrective_loop --test p6_budget_control
     Summary [   3.007s] 6 tests run: 6 passed, 0 skipped

$ cargo nextest run -p gauntlet-ports
     Summary [   0.463s] 88 tests run: 88 passed, 0 skipped

$ cargo nextest run -p gauntlet-adapter-state-sqlite
     Summary [   0.988s] 41 tests run: 41 passed, 0 skipped

$ just lint
cargo clippy --workspace --all-targets -- -D warnings
    Finished `dev` profile [unoptimized] target(s) in 1.99s
cargo fmt --check
```

(`just lint` prints nothing after `cargo fmt --check` when formatting
matches; the recipe exited 0. Two red intermediate lint states were fixed
during the item-9 close — an `unused_mut` warning in `bound_the_drain`
and rustfmt drift in the rewritten port test — both recorded in the
checkpoints; the final run above is clean.)

Workspace-wide gates are left to slice F3 / the orchestrator, per brief.
