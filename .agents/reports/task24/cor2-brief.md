# Task 24 — COR-2 brief (E-2 trigger discharge; small, one correction)

You are the COR-2 builder for task 24 in /home/sprime01/projects/gauntlet.
Task23 recorded residual **E-2** had trigger "next gate touch"; task24
touched `crates/gauntlet-app/src/promote/gate.rs` substantively, so the
trigger fires and E-2 is discharged IN THIS ROUND. Nothing else changes.

## E-2 (from task23 closure, verbatim intent)

"The by-unit instrument join takes the FIRST treatment row per unit —
order-sensitive for duplicated unit rows (unreachable through the
shipped loop: settlement commits are idempotent by run+unit)."

The join is `instrument_series_verdict`
(`crates/gauntlet-app/src/promote/gate.rs` ~739): for each
control-settled unit it `.find()`s the FIRST treatment settlement of
the same unit. If one arm's metrics ever carry TWO rows for a unit
(different instruments), whichever row sorts first decides the verdict —
an ordering-controlled row could shield a weakened counterpart.

## The correction (smallest, fail-safe, order-independent)

Replace first-match with **any-weaker**: for every control-settled
unit, EVERY treatment settlement row of that same unit must carry an
instrument at least as strong — if ANY counterpart row is weaker, the
series is `Weakened`; a unit with no treatment counterpart stays
`Undecidable` exactly as today. This is strictly fail-safe (extra rows
can only hurt the treatment arm; no ordering can shield a weakened
row; the honest single-row road is unchanged). Preserve the existing
typed gap details for the missing-counterpart / missing-instrument
cases. Update the fn's doc comment to state the any-weaker rule and
why (order-independence under duplicated rows). Do NOT restructure
anything else in the fn or module.

## Regression test (RED-first)

Add ONE test to `crates/gauntlet-cli/tests/antigaming.rs` near the
existing gate legs, named clearly (e.g.
`the_by_unit_instrument_join_is_order_independent_under_duplicated_rows`):
a control arm with one deterministic-settled unit; a treatment arm with
TWO rows for that same unit — one weakened, one deterministic, in BOTH
orders (parametrize the two orderings or assert both) — must adjudicate
`Weakened` (condition 3 failed), never shielded by the deterministic
row regardless of position. RED-first drill: apply the OLD first-match
behavior temporarily (or mutate the new any-weaker check away), show
the new test RED, restore byte-identically (sha256 proof). The new fn
joins the suite count (18 → 19; the manifest validator counts G-IDs,
not test count — it must stay green unchanged).

## Teeth honesty

The fixed lines sit OUTSIDE every examined mutation span (the F-3
class): this test is the fix's execution teeth, and you prove it
RED-first. Do NOT add manifest lines or mutation spans for it — the
manifest maps the sixteen strategies, and E-2 is not one of them.

## Discipline (binding)

- RED-first with byte-identical restore + sha256 in your report.
- Every cargo/just invocation:
  `flock .agents/reports/task24/compile.lock -c '<command>'` — flock
  never nested; `sccache --stop-server || true` inside the same
  scope. (`./scripts/run-mutation.sh` is NOT needed this round — the
  fix is outside all examined spans and the manifest is unchanged, so
  the delivered mutation.json stays canonical.)
- `just ruler` before and after; STOP if it fails.
- Slice battery: `cargo nextest run --test-threads 4 --test antigaming`
  19/19 · `cargo nextest run -p gauntlet-app promote::` 46/46 ·
  `cargo nextest run -p gauntlet-cli --test p9_self_improvement_containment`
  4/4 · `cargo nextest run -p gauntlet-cli --test improve_promote_gates`
  1/1 · `just lint` · `just boundaries` (237/237) ·
  `just check-package gauntlet-app` + `gauntlet-cli`.
- Checkpoint `.agents/reports/task24/cor2-progress.md`; completion
  contract `.agents/reports/task24/cor2-report.md` (sha256 of touched
  files, RED-first evidence, battery outcomes, deviations). Touch
  NOTHING else (no manifest, no script, no other tests, no ruler,
  no CURRENT_STATUS.yml).
