# Task 26 — BLIND design-as-built critique (`critic-design.md`)

**Critic:** blind design-as-built critic, fresh context, READ-ONLY. 2026-09-06.
**Standard:** `.agents/reports/task26/charter.md` (binding); plan §Task 26 + global
guardrails; ruler unit AK-069; AGENTS.md operating invariants.
**Blindness rule honored:** no `b1-*`, `b2-*`, `b3-*`, `cor1-*`, `cor2-*`, or
`IN-FLIGHT.md` file was opened.

## Verdict

**NOT_CONFIRM** — the measurement mechanism itself audits clean (honest figures,
charter-conformant derived matrix, no invented evidence, no circular judge, no
undisclosed scripting), but the deliverable **turns four shipped `just boundaries`
arch gates RED** (residuals 1–3), and AK-069's frozen evidence locator and the
charter-required narrative do not exist in the tree yet (residual 4). The repo's
own closure rule (`just ci` = check test lint deny boundaries ruler) cannot pass
as-built; settlement must wait for a correction round plus the orchestrator's
gate run and narrative.

Residual count: **3 BLOCKING · 1 MAJOR · 4 MINOR · 7 INFORMATIONAL.**

---

## Residuals

### 1. BLOCKING — shipped gate RED: AK-002 adapter denylist; `cli-agent` in application source

- **Obligation:** spec §24.3 via AK-002 ("no … adapter identifier appears in
  domain or application source"); AGENTS.md layering rules; plan final
  acceptance checklist (`just boundaries` exit 0).
- **Observed:** `crates/gauntlet-app/src/baseline/mod.rs` lines 3, 4, 12 (module
  doc: "the real `cli-agent` adapter") and
  `crates/gauntlet-app/src/baseline/report.rs` lines 74, 76 (the
  `POSTURE_DISCLOSURE` string itself names the adapter).
- **Gap:** 5 denylist violations. The charter's own disclosure wording
  ("under the real `cli-agent` adapter") collides with the repo's denylist when
  embedded in app-layer source; the wording must be relocated or re-tokened
  (e.g. "the task project's configured CLI adapter") without weakening the
  disclosure. App-layer strings naming adapters are exactly what AK-002 exists
  to prevent.
- **Evidence I executed:** `cargo nextest run -p gauntlet-arch-tests` →
  `denylist::repo::core_crates_contain_no_denied_identifiers` FAIL: "denied
  identifier: crates/gauntlet-app/src/baseline/mod.rs:12: 'cli-agent' …" (5
  violations, complete list, all task-26 files). Same 5 feed
  `denylist::repo::adapter_selection_is_config_driven` and
  `adapter_registry::selection_is_config_driven` (both FAIL).

### 2. BLOCKING — shipped gate RED: hardcoded-literal scanner, 290 violations, all in task-26 files

- **Obligation:** repo literal policy (spec §8.1/§8.6, AK-016 family) enforced
  repo-wide by `just boundaries` → `cargo nextest run -p gauntlet-arch-tests`.
- **Observed:** `literals::repo::core_crates_contain_no_hardcoded_literals`
  FAIL with 290 violations: `crates/gauntlet-app/src/baseline/harness.rs` (32),
  `metrics.rs` (46), `report.rs` (12), `task.rs` (33),
  `crates/gauntlet-app/tests/baseline_harness.rs` (97),
  `crates/gauntlet-cli/src/improve_cmd.rs` (34), `crates/gauntlet-cli/tests/
  improve_baseline.rs` (36). Classes include the `DECLARED_CHECK_PATH`
  `'scripts/check.sh'` and `'held-out.sh'` path literals (harness.rs:241, 619–621,
  1062–1069), `'/tmp'` absolute-path literals, and numerals adjacent to
  duration-shaped identifiers (`harness.rs:1032` `1_000` next to `_ms`;
  `metrics.rs` test wall_seconds literals). I filtered the full violation list
  by file and confirmed **zero** violations outside task-26 files — the rest of
  the repo contributes nothing, so this gate was green before task 26.
- **Gap:** the new module violates the repo's own configuration-not-code
  literal discipline at scale, including in production paths (the declared-check
  path constant is exactly the "path literal is environment coupling" the gate
  names). `just ci` / `just verify` cannot pass.
- **Evidence I executed:** the arch-test run above; per-file tally via the
  scanner's own violation output.

### 3. BLOCKING — shipped gate RED: §16.4 single-writer inventory, 25 unregistered production write sites

- **Obligation:** spec §16.4 single-writer discipline (AK-043 prevention leg):
  every production write-API site in a core crate is pinned in
  `WRITE_API_INVENTORY` (`crates/gauntlet-arch-tests/src/single_writer.rs:239`)
  with its reason and guard.
- **Observed:** `architecture_boundaries::answer_key_single_writer` FAIL with 25
  violations, all new baseline code in `crates/gauntlet-cli/src/improve_cmd.rs`
  (lines 1223–2650: the judge-shim materialization, scratch-root creation,
  `write_report_atomically`, `write_leg_files`, leg materialization, the D judge
  apparatus, `judge_snapshots`, `copy_tree`).
- **Gap:** the sites are real and mostly well-guarded in design (atomic rename,
  scratch-only paths), but none is inventoried with reason+guard, which is the
  repo's sanctioned mechanism. The gate is mechanical and currently red.
- **Evidence I executed:** the arch-test run above (complete 25-line list).

### 4. MAJOR — AK-069's frozen evidence locator is empty and the charter-required narrative does not exist

- **Obligation:** AK-069 `evidence_required: {kind: metric, locator:
  .agents/reports/baselines.json, producer: instrument}`; plan Done-when "all
  four profiles have recorded results and the narrative states the payment
  ratio explicitly"; charter locator reconciliation ("The narrative file is
  `.agents/reports/baselines.md`"); charter scenario matrix headline ("the
  narrative must state: no posture survives every defect class …").
- **Observed:** `.agents/reports/baselines.json` does not exist anywhere in the
  tree; `.agents/reports/baselines.md` does not exist (verified by
  `find` over the repo). The gate has not been run against the default `--out`.
- **Gap:** as-built, the "D does not win" case IS visible in the harness's
  derived results — my executed run records the dispatch tie D=B=C=10 (D>A=5)
  and the instrument denominators D=4 vs B/C=5, exactly per charter adjudication
  3 — but nothing is RECORDED at the frozen locator and no narrative states the
  payment ratio, the D-loses case, or the headline. Until the orchestrator runs
  the gate and writes the narrative, AK-069 has no admissible evidence and the
  plan's Done-when is unmet. This is the orchestrator's settlement act (my
  report is its input); settling without it would be a BLOCKING violation.
- **Evidence I executed:** repo-wide `find -name 'baselines*'`; my own gate-shape
  run into `/tmp/critic26/baselines.json` (exit 0, keys exactly A/B/C/D + `_meta`).

### 5. MINOR — per-leg settlement reliability is `Some(0)` even when the leg completed nothing

- **Obligation:** charter decision 7 / the metrics module's own rule
  (`metrics.rs:54–63`): "No figure exists when nothing completed."
- **Observed:** `harness.rs` `leg_direct` hardcodes
  `settlement_reliability_per_10k: Some(0)` for every A/B/C leg row, including
  the silent legs that completed nothing (visible in my executed report: A/B/C
  silent rows show `rel=0` while D's non-settling rows correctly show `null`).
  Profile totals pool correctly (C totals `rel=0` only because completions=2>0),
  so no conclusion changes.
- **Gap:** a figure without a denominator on leg rows — inconsistent with the
  module's own missing-evidence discipline.

### 6. MINOR — the D fallback-basis disclosure does not survive into the recorded report

- **Obligation:** charter adjudication 2: "absence of any snapshot … falls back
  to the leg copy and is disclosed."
- **Observed:** the fallback basis is computed and worded
  (`improve_cmd.rs:2297–2302`, "leg project copy (disclosed fallback: …)") but
  `HeldOutVerdict.detail` is dropped by the harness (`harness.rs` `run_leg`
  stores only `verdict.passed`); the report's `held_out` field is bare
  "pass"/"fail", and `_meta` has no D-surface field (the CLI notes the app-owned
  `_meta` has "no extension point" and prints a generic mechanism line to stdout
  instead, `improve_cmd.rs:1568–1577`).
- **Gap:** the disclosure exists at the operator stdout surface only; the
  recorded evidence (the report, the AK-069 artifact) cannot show which basis
  judged the silent D leg's `held_out=fail` (snapshot vs fallback). Per-leg
  basis should travel in the report.

### 7. MINOR — all five D legs report the identical `run_id`

- **Obligation:** honest, diagnosable durable records (AGENTS.md error/evidence
  discipline); charter schema "D adds run_id".
- **Observed:** my executed report's five D rows all carry
  `02FNM7KJAJBEYC1XJ11MQ5CA36`. The seed is deliberately shared
  (`BASELINE_ID_SEED = "baseline"`, improve_cmd.rs:1090–1093) and each leg has
  its own database, so there is no collision — but the report nowhere discloses
  that `run_id` is scoped per-leg-DB, so the field cannot globally identify a
  run and five distinct runs are indistinguishable by id in the evidence.
- **Gap:** a one-line `_meta` or leg-field disclosure is owed.

### 8. MINOR — `measured_wall_ms` is zero for every sub-second leg

- **Obligation:** plan step 2 names wall-clock as a measured metric.
- **Observed:** the harness clock is second-resolution `EpochSeconds`
  (`harness.rs`: `finished - started` in seconds ×1000); in my executed run every
  A/B/C non-silent leg records `measured_wall_ms = 0` and only the silent legs
  (11s/22s) and D legs (2s) are nonzero. The zero-payment disclosure says "at
  the harness clock's resolution", so it is disclosed — but the wall dimension
  of the payment comparison is effectively empty exactly where the postures
  differ least, and D's "pays the most" wall signal rests on 2s-vs-0ms artifacts
  of the loop's 1s poll tick, not on work time.
- **Gap:** resolution makes one named metric near-vacuous; a sub-second clock
  port use (or an explicit figure-basis note per leg) would fix it.

### INFORMATIONAL

9. **D's lying-defect/compromised-critic terminals are `budget_exhausted`
   (bounded-not-converged), not refusal-class.** Charter says "D refuses
   (facet FAIL → bounded honest terminal, no settlement)"; the real loop burns
   its correction budget (2 iterations, zero settlements, reserve intact) and
   stops bounded — the substance matches, and adjudication 4's precedent accepts
   bounded honest terminals, but the recorded classification word differs from
   the app-level scripted test's `Blocked → refused-terminally`. The narrative
   should use the durable terminal words.
10. **`bounded_termination` is a constant `true` for every recorded leg**
    (both `leg_direct` and the D path), not a derived per-leg fact. Its meaning
    is definitional and disclosed (`BOUNDED_DISCLOSURE`: "true for every
    recorded leg"); it cannot discriminate, by construction.
11. **A/B/C ground truth executes the leg workspace's own copy of
    `held-out.sh`** (`leg_copy_held_out`). A non-scripted adversarial child
    could tamper with that copy in its workspace; the bound is the fixture
    disclosure ("never referenced by any child") and the scripted children honor
    it (I verified by hand: every branch). D's snapshot judge uses the
    fixture-root instrument and is tamper-proof.
12. **D's held-out AND-aggregates every snapshot,** so a hypothetical
    multi-attempt leg with a bad early attempt and a correct settled final state
    would judge `fail`. Charter-specified ("AND-aggregated") and inert on this
    fixture (deterministic per-leg children); worth remembering if the fixture
    ever grows correction-round shapes.
13. **`reserve_intact` encodes "no checkpoint = cannot prove" as `false`**
    (`run_full_loop`: `None => false`) — the conservative direction (never
    flatters), though "unproven" and "breached" are different states.
14. **RES-15 reconciled:** `report_manifest.rs` enumerates top-level commands
    only; `improve` was already `wired: true`, so no per-subcommand row was
    owed. The help-snapshot diff is exactly the two new usage lines (verified
    via `git diff`).
15. **State isolation verified live:** after my gate run the fixture tree has no
    runtime residue (13 files, no `evidence/`, no staging files); all scratch
    lived under the temp root; the D legs' fixture `reference.lock` verified
    inside the run (`run_full_loop` calls `reference_lock::verify_lock`; a
    mismatch would have exited 2 — it exited 0).

---

## What I verified positively (the CONFIRM-side record)

Executed personally, not read from claims:

1. **Gate shape:** `cargo run -q -p gauntlet-cli -- improve baseline --profiles
   A,B,C,D --tasks tests/fixtures/baseline --out /tmp/critic26/baselines.json`
   from the repo root under the compile lock → exit 0; report keys exactly
   `A, B, C, D, _meta`.
2. **Derived matrix vs charter, cell by cell** (from my own run):
   honest-success all four complete, nobody escapes; lying-defect A
   false-completes, B/C refuse, D ends with zero durable settlements;
   subtle-defect A/B/D complete-and-escape (D while settling both units with
   reliability 10000 — the shared-criterion escape), C refuses; compromised-critic
   A/C escape, B refuses, D refuses (zero settlements); silent nobody completes,
   all bounded, D `escalated` with reserve intact. Payment: dispatch tie
   10=10=10, D>A=5, instrument_runs D=4 vs B/C=5 — the AK-069 "D does not win"
   case, plainly visible in the totals (not buried).
3. **Fixture scripts by hand** in scratch copies: base source fails held-out;
   subtle-defect builder drops X, reviewer reports real check exit 0 (B would
   complete falsely), critic reports exit 1 (C catches), held-out fails;
   compromised-critic builder forges `exit_code 0`/`state completed` writing
   nothing, declared check exits 1 (B refuses), critic always passes (C would
   complete falsely); honest builder passes held-out; unknown
   `GAUNTLET_BASELINE_MODE` is a typed exit-2 refusal. All match the charter
   matrix.
4. **Suites:** `cargo nextest run -p gauntlet-app` 618/618 PASS;
   `cargo nextest run -p gauntlet-cli --test improve_baseline` 5/5 PASS
   (73.8s, includes the full gate-shape matrix test with the retained-world
   T2/T3/T5 teeth).
5. **Honest-measurement audit:** every plan-named metric traced to a source —
   held-out (fixture instrument executed by the harness / D snapshots),
   false-completion and escape (named, tooth-tested predicates), regressions
   (A/B base-state comparison = 1 on subtle; D honestly `not-measured`),
   dispatches (harness plan; D durable lifecycle facts), payment (fixture zeros
   disclosed; D durable checkpoint counters), wall (harness-measured),
   recovery (bounded facts + domain Budget reserve arithmetic over the durable
   checkpoint), verification burden (structural A=0, B/C=1/leg, D=4 durable
   records), settlement reliability (A/B/C Some(0) by construction, D
   deterministic-instrument join = 10000 on settling legs). No figure is
   invented; no comparison is missing-evidence-tolerant in the passing direction
   (critic verdict `None`→Inconclusive refuses; review exit `None` refuses;
   `.absent` snapshots fail; `.copy-failed` is a typed error; unprovable reserve
   records false). Zero-token and deterministic-child disclosures present in
   `_meta`, fixture README, GAUNTLET.md, child.sh, app module docs, CLI stdout,
   and the gate test; no wording reads as a frontier-model claim (scanned).
6. **D snapshot-wrapper audit (adjudication 2):** the wrapper runs the declared
   check first with inherited cwd/env and no arguments, passes its exit through
   unchanged, and snapshots only AFTER the check; a snapshot failure writes
   `.copy-failed` without touching the loop's verdict, and the judge treats
   `.copy-failed` as a typed execution error (fail-closed, never a pass). The
   loop's decisions cannot be influenced by the wrapper. The gate test's T5
   asserts byte-identical stdout and equal exit over pass and fail shapes. No
   shape grades never-worked-on bytes without a disclosed basis (see residual 6
   for the recorded-evidence gap).
7. **Type/codec discipline:** `baseline_task@v1` strict parser (unknown/
   duplicate fields, absolute `held_out`, empty bodies/lists, tabs, fences and
   comments refused, typed errors carrying 1-based lines); fail-closed profile/
   scenario parse (lowercase `a` refused, empty segments refused, dedup-free);
   three-state `Outcome` preserved (`Option<Outcome>`; no bool conversion);
   typed errors with loci throughout.
8. **Layering:** `gauntlet_app::baseline` performs zero IO, spawns no process,
   touches no adapter (scanned); it depends only on domain + ports; the CLI
   seam wires and the fixture scripts touch no reference/contract/evidence
   artifact (scanned).

## What I did NOT examine (scope bounds)

- Builder/correction history (`b1-*`, `b2-*`, `b3-*`, `cor1-*`, `cor2-*`,
  `IN-FLIGHT.md`) — excluded by the blindness rule.
- `just lint` (clippy), `just deny`, and workspace-wide `just check` / `just
  test` — not run (WSL memory discipline; package-scoped suites only). The
  arch/boundaries gate WAS run and is red (residuals 1–3).
- The composed-loop internals (controller, verification rungs) beyond the
  baseline seam's use of them; the task-20/25 fixtures; the rest of the CLI
  surface; `docs/`; performance beyond the wall observations above.
- Recomputation of the fixture `reference.lock` sha256 digests by hand (the D
  legs' in-run `verify_lock` passing is my evidence the lock holds).
