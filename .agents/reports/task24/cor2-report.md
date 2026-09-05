# COR-2 report — task24 E-2 trigger discharge (COMPLETE)

Builder: COR-2 (small correction round — one recorded residual, one
regression test, nothing else). Work order: `cor2-brief.md`.
Progress log with per-phase evidence: `cor2-progress.md`.

## Headline

Residual **E-2** (task23 closure: "the by-unit instrument join takes the
FIRST treatment row per unit — order-sensitive under duplicated unit
rows") is DISCHARGED. `instrument_series_verdict`
(`crates/gauntlet-app/src/promote/gate.rs`) now applies the **any-weaker
join**: for every control-settled unit, EVERY treatment settlement row of
that same unit must carry an instrument at least as strong — any weaker
counterpart row ⇒ `Weakened` (condition 3 failed), regardless of row
position. Missing-counterpart and missing-instrument cases keep today's
exact typed `Undecidable` details; a demonstrated weakness outranks a
missing-instrument gap (a failed condition is never demoted to
undecidable by another row's position — the precedence rule that makes
the verdict order-independent under every row arrangement). The honest
single-row road is unchanged; the change is strictly fail-safe. New
regression test added RED-first; all gates green; ruler PASS before and
after.

## Touched files (sha256)

| File | Change | sha256 (final) |
|---|---|---|
| `crates/gauntlet-app/src/promote/gate.rs` | any-weaker join in `instrument_series_verdict` + `SeriesVerdict` doc block rewritten (rule + E-2 rationale). Nothing else in fn/module. | `a006cb87a4187c1399a1866c6d544e94ca9299a59310194640878baf70b906f0` |
| `crates/gauntlet-cli/tests/antigaming.rs` | ONE new test after the `improving_evaluation_record` fixture: `the_by_unit_instrument_join_is_order_independent_under_duplicated_rows` (suite 18 → 19). | `1e37532bf790aa0e7e9847c028b461a78260b07e25e637e0cea141073963133f` |
| `.agents/reports/task24/cor2-progress.md` | slice checkpoints (contract file) | `de7b240410c40d91003058893b5e64917af19999c89636c9d5eb14735fe0fbb1` |
| `.agents/reports/task24/cor2-report.md` | this report (contract file) | — |

Baseline (pre-slice) digests, for the record: gate.rs
`953cf2a8c6598fee9a3fa49e75ff9f08e15bc1f353674a6ac3109dcabc6a17ff`,
antigaming.rs
`b32649694a7b6a95ec3f4f14171a76e0c1c01ee7239ea541ec28e6a858e09a81`.

NOT touched (verified by tree reconciliation against the pre-slice
baseline): the manifest
(`.agents/reports/defended-mechanisms.txt`), `mutation.json`,
`scripts/run-mutation.sh`, every other test, ruler files, and
`.agents/CURRENT_STATUS.yml`. Per the brief, the fix's fixed lines sit
outside every examined mutation span, so the delivered mutation.json
stays canonical and was NOT regenerated.

## RED-first evidence

1. **Test added against the OLD first-match code.** RED run:
   `cargo nextest run -p gauntlet-cli --test antigaming
   the_by_unit_instrument_join_is_order_independent_under_duplicated_rows`
   → FAILED with exactly the residual's shape:
   `panicked at crates/gauntlet-cli/tests/antigaming.rs:1174: a
   duplicated-unit treatment row shielded its weakened counterpart by
   position (weakened-last) — the by-unit instrument join adjudicated
   Provisional { … }`. The first-match join took the deterministic row
   and never saw the weakened one; the gate therefore never raised
   condition 3. (The weakened-FIRST ordering passes even under old code —
   the failure is order-DEPENDENT, which is precisely E-2.)
2. **Fix applied** → the same single test → PASS (both orderings).
3. **Teeth drill (neutralization):** fixed-state gate.rs sha256 recorded
   (`a006cb87…`); the any-weaker join neutralized back to exact
   first-match semantics with a one-token change
   (`for treatment_fact in counterparts.into_iter().take(1)`) → the new
   test went **RED again** (same shielding panic, weakened-last
   ordering); restored the fixed line; gate.rs sha256 after restore =
   `a006cb87a4187c1399a1866c6d544e94ca9299a59310194640878baf70b906f0` —
   **byte-identical** (cmp-verified against the pre-drill digest).
   The test is the fix's execution teeth, as the brief required.

## The fix (shape, for review)

- Counterpart rows collected with `.filter(|fact| fact.unit ==
  control_fact.unit)` over ALL treatment settlements (was:
  `.find()` first match).
- Empty counterpart set ⇒ typed `Undecidable`, detail byte-identical to
  the old missing-counterpart string.
- Control row missing its instrument fact ⇒ typed `Undecidable`
  ("a control settlement names no verifying instrument"), unchanged and
  still checked before any treatment row is read.
- Over all counterpart rows: any `Some(false)` where control is
  `Some(true)` ⇒ `Weakened` immediately. A `None` instrument fact sets a
  gap flag; if the weakness scan found nothing, the gap is reported with
  the old typed detail ("a treatment settlement names no verifying
  instrument"). Precedence weakness-over-gap is the order-independence
  mechanism and is strictly fail-safe (an extra row can only hurt the
  treatment arm). Note the one deliberate tightening this implies: under
  first-match, a non-first `None` row could be silently skipped;
  any-weaker reports the gap instead.
- `SeriesVerdict` doc block states the any-weaker rule, the E-2 history
  (first-match join was ordering-controlled), and the gap-precedence
  rationale.

## Battery outcomes (all under top-level `flock … -c` scopes,
`sccache --stop-server || true` inside each scope; flock never nested)

| Gate | Result |
|---|---|
| `cargo nextest run --test-threads 4 --test antigaming` | **19/19 PASS** (18 → 19 with the new fn) |
| `cargo nextest run -p gauntlet-app promote::` | **46/46 PASS** |
| `cargo nextest run -p gauntlet-cli --test p9_self_improvement_containment` | **4/4 PASS** |
| `cargo nextest run -p gauntlet-cli --test improve_promote_gates` | **1/1 PASS** |
| `just lint` | PASS (clippy `-D warnings`, `fmt --check` clean) |
| `just boundaries` | **237/237 PASS** |
| `just check-package gauntlet-app` | PASS |
| `just check-package gauntlet-cli` | PASS |
| `just ruler` | **PASS** — before the slice AND after |

Manifest validator (inside the 19) green UNCHANGED — it counts G-IDs,
not test count, as the brief stated.

## Deviations (none silent)

1. **First `just lint` run FAILED** — rustfmt wanted the new test's
   `record.treatment_held_out_metrics.settlements =
   [duplicated, tail].concat();` joined to one line — and the
   `… 2>&1 | tail` pipe masked the exit code, letting the `&&` chain
   continue into `just boundaries` (boundaries passed on its own scope;
   it tests architecture, not formatting). Correction: joined the line
   to the rustfmt-canonical shape; lint re-run with `set -o pipefail` →
   PASS; antigaming re-run after the edit → 19/19. Lesson taken: gate
   invocations piped through `tail` inside `flock -c` must set
   `-o pipefail` (now standard in this slice's invocations).
2. No other deviations. The brief's battery was executed in full; the
   mutation run was deliberately NOT re-run (per brief: fix outside all
   examined spans; manifest unchanged; mutation.json stays canonical).

## Verdict

E-2 discharged as ordered: the join is any-weaker and
order-independent, the regression test proves it RED-first with a
byte-identical sha256 restore, the suite is 19/19, every battery gate is
green, and the ruler never moved.
