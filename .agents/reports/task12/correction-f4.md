# Task 12 — correction FINISHER slice F4 — result contract

Scope: `correction-brief-f4.md` items 1–2, driven by confirmation round 1
(`final-confirmation-round1.md` NOT CONFIRM; adjudication
`confirmation-adjudication-round1.md` — finding 1 FIX NOW, finding 2
doc-truth). **This slice's completion is a RECOVERY record**: the
predecessor F4 session landed the implementation and tests, then died at
the sub-agent usage limit before writing any completion record. The
orchestrator verified the landed tree personally (byte-compare against
pre-break snapshots, suite runs, code reads against the STARTED designs)
and closed the record per `.agents/lessons/unrecorded-landed-work.md`.
The DONE-RECOVERED entries in `correction-checkpoints.md` carry the full
provenance; nothing in this contract claims the orchestrator implemented
the slice. A later recovery-F4 dispatch was CANCELLED before doing any
work (reconciled: zero tree changes) — no double work occurred.

## Status by item

| Item | Subject | State |
|---|---|---|
| 1 | Wire the run meter into review dispatch (critic-fan-out leg of AK-052) | **DONE (recovered-and-verified)** |
| 2 | Correct the overstated `mint` unforgeability doc (finding 2) | **DONE (recovered-and-verified, doc-only)** |

Nothing weakened; no proof assertion edited (p4/p6 byte-unchanged,
6/6); no frozen surface touched.

## Item 1 — the meter on the critic-fan-out leg

The confirmation round's MAJOR: §10.4 "Critic fan-out is bounded by
`review.max_critics` **and by the budget meter**" and §17.4 "consulted
before every dispatch" were satisfied on the builder leg only; the
review path admitted model comparators under a count budget with no
meter consult, and `ReviewDeps` had no meter field.

Landed shape (predecessor; verified against its STARTED design and the
brief):

- `ReviewDeps` gains REQUIRED `meter: &dyn BudgetMeter` and
  `critic_slice: BudgetSlice` — no composition may skip the meter by
  passing `None`.
- `schedule_model` consults `meter.reserve(critic_slice)` AFTER the
  count lease (count-first is load-bearing: proofs set
  `model_dispatches: 0`, so their truncation arm stays count-shaped).
  A Reserve-class refusal — forward-ceiling OR `ScopeExceeded` — is the
  §10.4 truncation: `ModelAdmission::MeterRefused(cause)` and the
  caller records `UnavailableEvidence::truncated_by_meter` with the
  typed cause in the reason. Never a run error (C6.1's
  skip-and-continue discipline, critic leg).
- The reservation is HELD per admitted job (`LeasedPending.reservation`)
  — critic and builder dispatches share `max_parallel` (§10.4 "across
  all roles") — and released at the job's terminal via the zero-payment
  charge (B1 convention); a quarantine keeps it held, mirroring the
  count lease.
- `critic_slice` = `per_attempt_estimate(...)` at the controller call
  site — the loop's own affordability figure, no new Annex E /
  `ControlPolicy` setting (the C9.4 blast-radius warning). A zero slice
  would have been the vacuous-fix trap; the estimate is non-vacuous.
- The module doc's deferral paragraph (the artifact's own "Task-12
  ledgered trigger") is rewritten: the meter is wired; the count stays.

**Fail-first evidence — re-derived independently by the orchestrator**
(the predecessor's staged observations died with its session; three
legs, each snapshot → neutralize → RED → restore → cmp-byte-identical →
green):

1. **Refusal leg** (also recorded as tooth **T7** in `teeth.md`):
   consult bypassed (`meter.mint` behind `Ok`) → composed test FAILED
   `the critic dispatch is refused at admission — the model comparator
   never ran: left: 1, right: 0`.
2. **Release leg**: `release_reservation` no-op'd → no-over-refusal test
   FAILED `every reservation — two builders and two critics — came
   back: nothing is held after fan-in: left: 4, right: 2` (two critic
   slots leaked).
3. **Reason leg**: `truncated_by_meter` neutralized to the count text →
   review test FAILED at `the reason names the meter as the refusing
   authority` with the count text observed in the panic.

**Composed tests:** `control::controller::tests::
a_meter_refused_critic_dispatch_is_truncated_and_the_run_continues` and
`a_critic_reservation_releases_at_fan_in_so_a_later_iteration_still_dispatches`;
review-level record pinned by `compare::review::tests::
a_meter_refusal_truncates_the_dimension_with_the_typed_cause`. Suite
growth over F3's tree: control 73 → 75 (+2 composed); compare +1 test
fn (its package now runs 36, a total that includes prior tasks' compare
tests plus F4's new stubs/helpers — the earlier "19 → 36" framing in
the first draft of this contract over-attributed prior growth to F4);
workspace `just test` 911 → 914 (+3).

## Item 2 — mint doc-truth (finding 2)

`crates/gauntlet-ports/src/budget_meter.rs`: the `Reservation`/`mint`
docs now state the TRUE guarantee — authority-of-CONSTRUCTION (minting
requires a meter instance; the value is minted inside the trait), NOT a
caller guarantee (`mint` is public; any meter holder can mint and
saturating-release a slot it never reserved) — plus the composition-root
obligation (meters handed only to trusted code) and the recorded
ports-revision candidate for a type-level seal (later scope; trigger:
the next `BudgetMeter` ports revision). The adjudication is referenced
in the doc. Doc-only: no signature, behavior, or test change; doc-only
items have no behavioural fail-first to observe (recorded, not
invented).

## Files touched

- `crates/gauntlet-app/src/compare/review.rs` (wiring, records, 17 tests,
  module doc)
- `crates/gauntlet-app/src/control/controller.rs` (call-site wiring, 2
  composed tests, fixture meters)
- `crates/gauntlet-cli/tests/p2_context_isolation.rs` (4 `ReviewDeps`
  literals + local permissive stub)
- `crates/gauntlet-ports/src/budget_meter.rs` (doc-only)
- `.agents/reports/task12/{correction-checkpoints,teeth,IN-FLIGHT}.md`,
  this file (records)

## Deliberately left alone

- **Payment unification** — critic token/cost payments booking into the
  run's `budget_spent` (the compare-model adapter's private `Budget`
  stays). §17.4's second paragraph; trigger: Task 15 real model runner /
  Task 20 composition root. Recorded in the adjudication and the closure
  residuals.
- **The count budget** (stays — §10.4 bounds by BOTH), the **mint seal**
  (later-scope ports revision), **§21.3 policy** (unchanged deferral),
  and everything F1/F2/F3 left alone.

## Teeth

The mechanism is tooth-shaped and the tooth EXISTS: the orchestrator ran
**T7** personally (the refusal-leg neutralization above is its break) —
bite observed, revert cmp-byte-identical, suites green after; recorded
in `teeth.md`. No duplicate tooth was added.

## Gates (final tree, run sequentially by the orchestrator, verbatim)

```
$ just check-package gauntlet-app
    Checking gauntlet-app v0.1.0 (/home/sprime01/projects/gauntlet/crates/gauntlet-app)
    Finished `dev` profile [unoptimized] target(s) in 15.16s

$ cargo nextest run -p gauntlet-app control::
     Summary [   0.442s] 75 tests run: 75 passed, 228 skipped

$ cargo nextest run -p gauntlet-app compare::
     Summary [   0.167s] 36 tests run: 36 passed, 267 skipped

$ cargo nextest run --test p4_corrective_loop --test p6_budget_control
     Summary [   0.858s] 6 tests run: 6 passed, 0 skipped

$ cargo nextest run --test p2_context_isolation
     Summary [   0.268s] 13 tests run: 13 passed, 0 skipped

$ cargo nextest run -p gauntlet-ports
     Summary [   0.366s] 88 tests run: 88 passed, 0 skipped

$ cargo nextest run -p gauntlet-adapter-meter-memory
     Summary [   0.025s] 7 tests run: 7 passed, 0 skipped

$ just lint
    Checking gauntlet-cli v0.1.0 (/home/sprime01/projects/gauntlet/crates/gauntlet-cli)
    Finished `dev` profile [unoptimized] target(s) in 21.13s
cargo fmt --check
(exit 0)
```

(fmt note: the predecessor's drift was exactly `review.rs` +
`controller.rs`, the two files it edited; `cargo fmt` touched only those;
`--check` clean after. Workspace-wide gates belong to the orchestrator's
closure sweep.)
