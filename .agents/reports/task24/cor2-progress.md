# COR-2 progress — E-2 trigger discharge (task24)

Work order: `cor2-brief.md` (this directory). Scope: discharge task23
residual E-2 — `instrument_series_verdict`
(`crates/gauntlet-app/src/promote/gate.rs`) joins treatment rows by-unit
FIRST-match; replace with any-weaker + ONE order-independence regression
test in `crates/gauntlet-cli/tests/antigaming.rs`. Nothing else.

## Phase log

- Recon complete 2026-09-04:
  - `git status` reconciled against IN-FLIGHT.md: task24 B1 + COR-1
    state on tree as recorded (gate.rs + antigaming.rs modified/new,
    mutation.json + run-mutation.sh delivered, cor1 report complete).
    No orphaned state; single-builder discipline holds (no other agent
    active).
  - `just ruler` (under compile lock, sccache stop inside scope): PASS —
    "answer key and manifest digests match the frozen lock".
  - Read `instrument_series_verdict` (gate.rs:738) + both callers
    (adjudicate condition 3 at :343, earlier_record_failures at :862),
    `SeriesVerdict` doc block (:719), the gate's failed/gaps precedence
    (failed → REJECTED short-circuits gaps), the g13 gate leg +
    `improving_evaluation_record` fixture (antigaming.rs:1008/1060 —
    the model for the new test), and the manifest validator
    (counts G-IDs, not test fns — 18 → 19 tests is invisible to it).
- Baseline sha256 (pre-edit):
  - gate.rs `953cf2a8c6598fee9a3fa49e75ff9f08e15bc1f353674a6ac3109dcabc6a17ff`
  - antigaming.rs `b32649694a7b6a95ec3f4f14171a76e0c1c01ee7239ea541ec28e6a858e09a81`
- Plan: (1) add test → RED against current first-match code
  (the weakened-ROW-LAST ordering is the shielding one; weakened-first
  already passes even under old code — the failure is order-dependent,
  which IS the residual). (2) Apply any-weaker fix + doc update → GREEN
  both orderings. (3) Teeth drill: neutralize the any-weaker check back
  to first-match, show RED again, restore byte-identically, sha256
  proof. (4) Slice battery per brief. (5) `just ruler` after.

## Checkpoint 1 — test added, RED proven (2026-09-04)

- Test `the_by_unit_instrument_join_is_order_independent_under_duplicated_rows`
  added to `crates/gauntlet-cli/tests/antigaming.rs` right after the
  `improving_evaluation_record` fixture (both orderings asserted in one
  test: weakened-first, weakened-last; expected `Rejected` with exactly
  `settlement_access_weakened` in both).
- RED run (old first-match still in gate.rs):
  `cargo nextest run -p gauntlet-cli --test antigaming
  the_by_unit_instrument_join_is_order_independent_under_duplicated_rows`
  → **FAILED**, exactly on the shielding ordering:
  `panicked at antigaming.rs:1174: a duplicated-unit treatment row
  shielded its weakened counterpart by position (weakened-last) — the
  by-unit instrument join adjudicated Provisional {…}` — the gate took
  the first (deterministic) row and never saw the weakened one.
- Next: any-weaker fix in gate.rs.

## Checkpoint 2 — fix applied, GREEN; teeth drill done (2026-09-04)

- Any-weaker join applied to `instrument_series_verdict`
  (counterparts collected by filter over ALL rows; missing-counterpart
  and control-missing-instrument keep the exact typed `Undecidable`
  details; treatment missing-instrument reported only after the
  weakness scan — a demonstrated weakness outranks a gap, so the
  verdict is order-independent under every row arrangement);
  `SeriesVerdict` doc block updated with the any-weaker rule and the
  E-2 rationale. Nothing else in the fn or module restructured.
- Fixed-state gate.rs sha256:
  `a006cb87a4187c1399a1866c6d544e94ca9299a59310194640878baf70b906f0`
- New test vs fix: **PASS** (both orderings).
- Teeth drill: neutralized the any-weaker join to first-match with a
  one-token change (`for treatment_fact in
  counterparts.into_iter().take(1)`) → new test **RED** again
  (`shielded … by position (weakened-last) … adjudicated Provisional`),
  restored the fixed line, sha256 after restore =
  `a006cb87a4187c1399a1866c6d544e94ca9299a59310194640878baf70b906f0`
  — **byte-identical** (cmp-verified).
- Next: slice battery.

## Checkpoint 3 — battery complete, ruler PASS, DONE (2026-09-04)

- Slice battery (every cargo/just invocation a top-level
  `flock .agents/reports/task24/compile.lock -c '…'` with
  `sccache --stop-server || true` inside the same scope; flock never
  nested):
  1. `cargo nextest run --test-threads 4 --test antigaming` —
     **19/19 PASS** (re-run after the fmt fix below).
  2. `cargo nextest run -p gauntlet-app promote::` — **46/46 PASS**.
  3. `cargo nextest run -p gauntlet-cli
     --test p9_self_improvement_containment` — **4/4 PASS**.
  4. `cargo nextest run -p gauntlet-cli --test improve_promote_gates`
     — **1/1 PASS**.
  5. `just lint` — PASS (see deviation 1: first run failed on a
     rustfmt line-wrap nit in the new test; fixed and re-run clean
     under `set -o pipefail` — clippy `-D warnings` + `fmt --check`).
  6. `just boundaries` — **237/237 PASS**.
  7. `just check-package gauntlet-app` — PASS.
  8. `just check-package gauntlet-cli` — PASS.
  9. `just ruler` — **PASS** (before the slice AND after; frozen-lock
     digests match both times).
- Deviation 1 (recorded, not silent): the first `just lint` invocation
  failed — rustfmt wanted the new test's
  `record.treatment_held_out_metrics.settlements = [duplicated, tail].concat();`
  on one line — and the `… | tail` pipe masked the failure so the `&&`
  chain continued into `just boundaries` (which passed on its own
  scope). Corrected: line joined to the rustfmt-canonical shape, lint
  re-run with `set -o pipefail` → PASS, antigaming re-run → 19/19.
  Lesson: gate invocations inside `flock -c` must set `-o pipefail`
  when piping.
- Final sha256:
  - gate.rs `a006cb87a4187c1399a1866c6d544e94ca9299a59310194640878baf70b906f0`
    (identical to the post-drill restore digest — final state IS the
    fixed bytes)
  - antigaming.rs `1e37532bf790aa0e7e9847c028b461a78260b07e25e637e0cea141073963133f`
- Scope: touched ONLY gate.rs (the E-2 fix + doc block), antigaming.rs
  (the one new test), and this slice's two report files. No manifest,
  no mutation script, no mutation.json regeneration, no other tests,
  no ruler files, no CURRENT_STATUS.yml.
- Completion contract: `.agents/reports/task24/cor2-report.md` written.
