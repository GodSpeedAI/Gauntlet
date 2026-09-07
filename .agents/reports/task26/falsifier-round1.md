# Task 26 — BLIND EXECUTING FALSIFIER, round 1

Date: 2026-09-06/07. Blind posture held: no `b1*/b2*/b3*/cor1*/cor2*/critic-design*`
or `IN-FLIGHT.md` files read; the claim standard was the charter
(`.agents/reports/task26/charter.md` incl. the four adjudications), plan §Task 26,
AK-069, and repo invariants (missing evidence is never passing; INCONCLUSIVE never
PASS; provisional reported as provisional).

## Verdict: **NOT_CONFIRM**

Every dishonest-conclusion attack that reached the recorded report is documented
below with executed evidence. The harness repelled a large fraction of the
attempted attacks (fail-closed everywhere else), and all five named teeth bite —
but the profile-D judge channel admits forgery, and one deterministic variant makes
the harness RECORD a conclusion that is false about the world (F1).

Finding counts: **CRITICAL 1 · MAJOR 2 · MINOR 2 · INFORMATIONAL 2.**
Teeth: T1–T5 all verified (mutation → named test RED → byte-identical restore →
GREEN; sha256 before/after match).

---

## Findings

### F1 — CRITICAL: the D judge channel is forgeable; the harness records a false completion verdict for defective settled work

- **Attack.** The fixture child (the measured party, in a scratch copy) overwrites
  the leg's materialized judge wrapper before any facet execution, so every facet
  execution reports a passing instrument and snapshots a FORGED complete
  deliverable (both markers) instead of the real (Y-only) workspace source.
- **Repro (executed).** Scratch fixture `/tmp/falsifier/ow-set` (copy of the repo
  fixture; `scenarios: [subtle-defect]`; `agent/child.sh` subtle-defect branch
  extended): the child resolves the leg dir with the same `dirname`×3 chain the
  shipped child wrapper teaches (`crates/gauntlet-cli/src/improve_cmd.rs:1107-1113`),
  reads the wrapper path from the plaintext pointer file
  (`$leg/baseline-judge-wrapper`, written at `improve_cmd.rs:2295-2307`), and
  rewrites `<leg>/judge/check-wrapper.sh` to: fill missing evidence locators with
  the check's constant pass report (mirroring `check.sh`'s own
  write-only-if-missing rule), print the rung-valid pass report, snapshot a forged
  X+Y source, exit 0. Then:
  `GAUNTLET_BASELINE_SCRATCH_ROOT=/tmp/falsifier/ow-world2 ./target/debug/gauntlet improve baseline --profiles A,B,C,D --tasks /tmp/falsifier/ow-set --out /tmp/falsifier/ow-world2/baselines.json` → exit 0.
- **Observed.** Real work: Y-only deliverable (property X dropped). A and B
  correctly record `false_completion=true, defect_escape=true`; C refuses. **D
  records** `claimed_complete=true, held_out="pass", held_out_basis="instrument-snapshot",
  false_completion=false, defect_escape=false, settled_units=[AK-961, AK-962],
  settlement_reliability_per_10k=10000`. The judge dir contains only
  `forged-by-overwrite.rs`. The recorded report asserts D settled both units on
  durable instrument-produced evidence with the ground-truth judge passing the
  work — every clause of that is false about the world, and the leg's
  `held_out_basis` makes the `_meta` mechanism disclosure
  ("judged over judge-owned instrument snapshots of the dispatched work, taken at
  every facet execution") false for this leg. This defeats exactly D's claimed
  decisive win ("its judge is an instrument, not a dispatchable opinion").
- **Two incidental defenses observed and bypassed.** (a) A first overwrite variant
  that clobbered a produced locator artifact was refused by the loop's write-once
  evidence binding ("an owner write was refused … recording it again with
  different facts", exit 1, NO report) — fail-closed, but the refined variant
  mirrors `check.sh`'s own locator rule and steps around it. (b) Two race-based
  daemon variants (post-facet wipe+replant, 50 ms and 10 ms polling) LOST the race
  both times — the judge reads within milliseconds of the last facet write — and
  those runs recorded honestly. The deterministic wrapper overwrite needs no race.
- **Locus.** `crates/gauntlet-cli/src/improve_cmd.rs`: judge wrapper materialized
  INSIDE the leg scratch world at a child-derivable path (`:2237-2294`) with its
  path published in a plaintext leg-file (`:2295-2307`); `judge_snapshots`
  (`:2597-2709`) accepts any `*.rs` in the dir — no provenance, no
  authentication of wrapper or snapshots; `run_held_out` (`:2386-2424`) trusts the
  dir's contents. The shipped child wrapper itself demonstrates the
  `dirname`×3 leg-resolution (`:1107-1113`).
- **Note.** The honest posture scopes the fixture children as deterministic data,
  but the charter's attack families direct exactly this poisoning attempt, and the
  recorded `held_out_basis` is a statement about the world that the harness cannot
  substantiate. A defense would keep the judge dir (and pointer) at a path not
  derivable from anything the child sees (per-leg random secret held by the
  executor), or add wrapper/snapshot provenance (judge-held secret stamped into a
  manifest the executor verifies before judging).

### F2 — MAJOR: forged snapshots on a no-facet leg flip the recorded held_out figure AND its basis disclosure

- **Attack.** Scratch child (silent mode) plants a well-formed snapshot
  (`planted-by-child.rs`, both markers) into the D leg's judge-owned dir before
  sleeping — the same `dirname`×3 resolution as F1.
- **Repro (executed).** `/tmp/falsifier/plant-set` (all five scenarios), full
  `A,B,C,D` run → `/tmp/falsifier/plant-world/baselines.json`, exit 0.
- **Observed.** D silent leg: `held_out="pass", held_out_basis="instrument-snapshot"`.
  The honest world (gate run) records `held_out="fail", held_out_basis="leg-copy-fallback"`
  for this leg. No facet execution ever happened; the graded bytes were written by
  the child, and the recorded basis claims they are "instrument snapshots of the
  dispatched work". `claimed_complete` stays false (the durable record governs), so
  no false completion was manufactured here — but the ground-truth column and the
  basis disclosure are corrupted, and the empty-dir→fallback rule is defeated by
  any planted `*.rs`.
- **Locus.** Same channel as F1 (`improve_cmd.rs:2401-2423`, `:2659`).

### F3 — MAJOR: A/B/C leg materialization escapes the per-leg scratch world via a `../`-carrying task id (containment asymmetry vs D)

- **Attack.** A scratch task.yaml with `id: ../../../tmp/falsifier/ESCAPED`
  (the codec accepts any non-empty scalar for `id`).
- **Repro (executed).** `/tmp/falsifier/trav-set`, full `A,B,C,D` run.
- **Observed.** `materialize_workspace` builds the leg dir as
  `<legs-root>/<task.id>.<scenario>-<n>` with no containment check: A/B/C
  materialized and dispatched inside `/tmp/tmp/falsifier/ESCAPED.lying-defect-{0,1,2}`
  — OUTSIDE the scratch root (leg-files, project copy, child dispatch, held-out
  run all there). The run then aborted at the D leg with the workspace provider's
  refusal ("escapes the workspace root (spec 17.5.2)", exit 1, no report). So no
  false REPORT is reachable (assemble requires D, and D always refuses on an
  escaping id) — but the charter's scratch-world containment invariant is broken
  at the seam for A/B/C, and only D carries the §17.5.2 check.
- **Locus.** `crates/gauntlet-cli/src/improve_cmd.rs:1896-1920`
  (`leg_dir = parent.join(format!("{}.{}-{}", task.id(), …))` — fixture-supplied
  `task.id()` flows into path construction unvalidated; contrast the provider's
  refusal in `run_full_loop`).

### F4 — MINOR: `instrument_runs` is incomparable across profiles and the recorded JSON does not disclose the denominator difference

- **Attack (audit).** Family-6 probe: compute the payment ratio from
  `/tmp/falsifier/gate-world/baselines.json` (the AK-069 locator shape) alone.
- **Observed.** `totals.instrument_runs`: A=0, B=5, C=5, D=4. D's figure counts
  durable verification records on SETTLING legs only (the refusing legs' facet
  executions count 0); B/C count one declared-check re-run per leg. A reader
  comparing 4 vs 5 concludes D pays LESS verification burden; the true instrument
  executions are not comparable on those denominators. Charter adjudication 3
  states the difference for the NARRATIVE and the gate test comments state it, but
  `_meta` (posture, zero-payment, regressions, cost-units, reliability, bounded,
  D-surface, run-id-scope disclosures all present) carries NO instrument-run
  denominator disclosure. The AK-069 locator JSON is the frozen evidence; the
  qualifying fact lives outside it.
- **Locus.** `crates/gauntlet-cli/src/improve_cmd.rs:2451-2457` (D burden =
  `verification_records`) vs `crates/gauntlet-app/src/baseline/metrics.rs:209-222`
  (`posture_burden` B/C = 1/leg); `_meta` assembly
  `crates/gauntlet-app/src/baseline/report.rs:253-327`.

### F5 — MINOR: a broken ground-truth instrument records a substantive `fail` — a false completion on correct work, indistinguishable from a real one

- **Attack.** Scratch copy with `scripts/held-out.sh` replaced by a crashing
  variant (stderr note, `exit 2`); `scenarios: [honest-success]`; full `A,B,C,D`.
- **Repro (executed).** `/tmp/falsifier/ho-set` → `/tmp/falsifier/ho-world/report.json`, exit 0.
- **Observed.** All four profiles record `held_out="fail"` and
  `false_completion=true` on genuinely correct work (escapes stay false — the
  honest-success exemption holds). Direction is fail-closed (never a false pass,
  and a signal-killed spawn degrades to `-1`→fail likewise), and the app docs
  declare INCONCLUSIVE unreachable for the held-out by design — but the recorded
  report cannot distinguish "instrument crashed" from "work is wrong", and the
  false-completion headline absorbs the crash silently. A `_meta` note or a
  distinct exit-band convention (e.g. exit 2 = typed inconclusive) would close it.
- **Locus.** `crates/gauntlet-cli/src/improve_cmd.rs:2577-2586` (`code == 0` is the
  only verdict boundary), `crates/gauntlet-app/src/baseline/metrics.rs:264-268`
  (doc: "INCONCLUSIVE is not reachable").

### F6 — INFORMATIONAL: the adapter admits a result document whose `run_id` does not echo the dispatch (invocation_id is enforced; run_id is not)

- **Executed.** Family-4 matrix, `wrong-run` shape (builder result with a fixed
  foreign ULID in `run_id`): A records `claimed_complete=true` (and is then
  correctly exposed by the held-out: `false_completion=true`); B/C/D refuse
  honestly. The spec 10.3 rule-6 identity echo is evidently enforced for
  `invocation_id` (the `wrong-inv` shape refuses everywhere) but not for `run_id`.
  No dishonest baseline conclusion is reachable (A's claim-basis completion is the
  charter's own design and the held-out exposes it), but the contract leniency is
  observable through this harness. The adapter is task-25 territory — recorded
  here as an observation, not patched.

### F7 — INFORMATIONAL: recorded D terminal words drift from the charter matrix's prose ("refuses") — `budget_exhausted` on lying-defect/compromised-critic

- **Executed.** Gate-run JSON: D lying-defect and compromised-critic record
  `terminal.durable_state="budget_exhausted"` (classification
  `bounded-not-converged`). The charter matrix prose says D "refuses"; adjudication
  4 named only `escalated` (silent) as the accepted bounded honest terminal word.
  The durable word is recorded honestly, zero settlements in every case, and
  "bounded honest termination" holds in substance — vocabulary drift only, but the
  narrative should say "bounded-not-converged" for these legs, not "refused".

---

## Repelled attacks (executed, honest behavior recorded)

- **Profile-list seam (8 shapes, all exit 2 typed):** `A,,D`, `A,`, `,A`, ``,
  `a` (lowercase rescue refused, vocabulary named), `A,Z`, missing `--profiles`,
  missing `--tasks`, nonexistent `--tasks`.
- **task.yaml codec (9 shapes, typed refusals with line numbers):** duplicate
  key (`line 3: field "id" appears twice`), unknown field, absolute `held_out`
  (`/etc/passwd` refused), unknown scenario token (vocabulary named), comment
  line, second document fence, inline-on-block-key, missing `held_out`,
  missing-field refusal by name.
- **Fixture-child contract (8 shapes through the REAL adapter, all honest
  outcomes — an outcome, never a crash, never a false completion):** truncated
  JSON, wrong invocation_id, empty result file, wrong-path result, missing
  result with exit 0 → claimed=false everywhere, D refuses with zero settlements;
  result with `self_checks: []` → A completes on the claim (the charter's own
  A basis; held-out exposes it as `false_completion=true`), B/C refuse (missing
  evidence never passes); critic with empty self_checks → verdict INCONCLUSIVE →
  C refuses (**INCONCLUSIVE never PASS, verified through the real adapter**);
  broken child script (syntax error) → every dispatch an honest failure.
- **Race-based snapshot daemons (50 ms and 10 ms):** lost the race twice; the
  judge read lands within milliseconds of the last facet write. Recorded honestly.
- **`--out` to an unwritable directory:** typed error ("Permission denied"),
  exit 1, no report, no `.staging` residue.
- **Write-once evidence binding (incidental):** both the clobbering wrapper
  overwrite and the weakened-declared-check control were refused by the loop's
  owner-write check (exit 1, no report) — a real defense the refined F1 variant
  steps around.
- **AK-069 done-when probe (family 6).** Real gate shape into scratch
  (`/tmp/falsifier/gate-world/baselines.json`, exit 0; repo-root default path
  never written — the locator `.agents/reports/baselines.json` did not exist
  before this round and was not created by me). Audited from the JSON alone:
  top-level keys exactly `A,B,C,D,_meta`; the "D does not win" case is plainly
  visible WITHOUT narrative — D dispatches tie B/C (10=10=10, D>A=5), D
  `instrument_runs` 4 vs B/C 5, D `measured_wall_ms` 18454 vs B/C ≈22000, D
  reliability 10000 vs A/B/C 0, and D's subtle-defect leg records
  `false_completion=true, defect_escape=true` while still settling — a reader can
  compute the payment story. All eight `_meta` disclosures present; every D leg
  carries a truthful-in-the-honest-world `held_out_basis` (instrument-snapshot ×4,
  leg-copy-fallback on silent); zero-completion legs carry `null` reliability (no
  invented figures). Omissions: F4 (instrument-run denominators) and F5/F7 above.

## Teeth table (mutation at locus → named test RED → restore → GREEN)

| Tooth | Locus mutated | Mutation | Named test that went RED | Restore sha256 match | Green re-proof |
|---|---|---|---|---|---|
| T1 | `crates/gauntlet-app/src/baseline/metrics.rs:32-34` | `is_false_completion` body `claimed_complete && !held_out_passed` → `claimed_complete && held_out_passed` | `baseline::metrics::tests::t1_false_completion_distinguishes_claimed_failed_rows` FAIL | `0fca1203…78df84771` OK | PASS |
| T2 | `crates/gauntlet-app/src/baseline/harness.rs` `critic_request` | body `task.requirement()` → `task.prompt()` (builder text into the critic envelope) | `baseline::harness::tests::t2_the_critic_envelope_is_blind_to_the_builder_result` FAIL | `044809f4…55c9d77578fd` OK | PASS |
| T3 | `crates/gauntlet-app/src/baseline/harness.rs` D leg | `claimed_complete: classification == LoopTerminal::Settled` → `claimed_complete: true` | `the_derived_scenario_matrix_matches_the_charter` FAIL (lying-defect D must not complete) | `044809f4…55c9d77578fd` OK | PASS |
| T4 | `crates/gauntlet-app/src/baseline/profiles.rs` `BaselineProfile::parse` | added `"a" => Ok(Self::A)` (case-insensitive rescue) | `baseline::profiles::tests::unknown_profile_tokens_are_refused_fail_closed` FAIL | `af45186a…d62256639938` OK | PASS |
| T5 | `crates/gauntlet-cli/src/improve_cmd.rs` `BASELINE_JUDGE_WRAPPER_BODY` | `exit "$code"` → `exit 0` (wrapper no longer preserves the check's verdict) | `gauntlet-cli::improve_baseline the_plan_gate_shape_writes_the_charter_report_and_the_matrix_holds` FAIL — "assertion `left == right` failed: the wrapper preserves the check's exit code" | `d2c1ec81…42ef3e0255dc` OK | PASS (73.5 s) |

Evidence files: `/tmp/falsifier/teeth-red.log`, `/tmp/falsifier/teeth-green.log`,
`/tmp/falsifier/teeth-t5-red.log`, `/tmp/falsifier/teeth-t5-green.log`,
`/tmp/falsifier/teeth-backup/sha256.{before,after}`. Batch-RED discipline: the four
app teeth were mutated together and run in one package-scoped `cargo nextest run -p
gauntlet-app <4 filters>` (each named test bitten individually in the output), then
all restored and re-proven in one run; T5 was mutated and proven separately
(package-scoped `--test improve_baseline <filter>`). Every `cargo` invocation ran
as its own top-level `flock .agents/reports/task26/compile.lock -c '…;
sccache --stop-server'`. All four files verified byte-identical post-restore; the
`git status` modifications under `crates/` are the task's own uncommitted
deliverable (sha256 proves my edits are gone).

## NOT-ATTACKED (explicit bounds for a future round)

1. **Cross-run stale snapshots on a retained scratch root.** Leg dirs are wiped by
   `remove_dir_all` at materialization (code-evident, and leg-dir names restart
   with the per-process counter), but a two-run staleness probe was not executed;
   in any case F1/F2 subsume the judge-surface impact.
2. **Concurrent runs sharing one `GAUNTLET_BASELINE_SCRATCH_ROOT`** (same leg-dir
   names from two processes) — not executed; likely INCONCLUSIVE-by-error, unproven.
3. **Held-out spawn failure** (non-executable/binary held-out script) — only the
   exit-2 crash was executed; the spawn-failure path (typed `Execution` error) is
   code-read, not run.
4. **`--config` override interactions per leg**, the custom `reviewer` role-id
   path, and multi-task sets with A/B/C only (assemble refusal) — not executed.
5. **`--profiles A,A` end-to-end** (parser documents duplicates-honored; the CLI's
   `contains` iteration silently runs the profile once) — read, not executed.
6. **The composed loop's internals** (controller, budget/stall mechanics,
   settlement machinery) — task-20/25 territory; probed here only through D-leg
   behavior. F6's adapter leniency belongs to that side too.
7. **Frontier-model or wording audit of the final narrative** —
   `.agents/reports/baselines.{json,md}` do not exist yet (locator not yet
   written); the AK-069 audit here used the gate-shape JSON in scratch. The
   orchestrator's real gate run and narrative wording remain to be produced and
   should be re-audited against F4/F5/F7 at settlement.
8. **Deeper D-judge defenses:** wrapper provenance/HMAC designs, judge-dir
   placement secrets, and manifest-verified snapshots — proposed under F1, not
   implemented or tested here.

## Scratch cleanup

All scratch worlds, attack fixtures, and logs live under `/tmp/falsifier/**`
(small, ~1 MB total); remove with `rm -rf /tmp/falsifier`. Nothing was written
into the repo fixture, the frozen ruler, any `reference.lock`, the justfile, or
the repo-root `.agents/reports/baselines.json` locator. No `git add`/`commit` was
performed.
