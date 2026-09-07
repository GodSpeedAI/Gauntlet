# Task 26 — FOCUSED EXECUTING FALSIFIER, round 2

Date: 2026-09-06/07. Blind posture held: only `falsifier-round1.md` +
`charter.md` read; no `b1*/b2*/b3*/cor1*/cor2*/cor3*/cor4*/critic-design*` or
`IN-FLIGHT.md`. Round-1 vectors re-run verbatim against the corrected tree,
plus six round-2 extensions on the same loci, a regression sweep of
previously-repelled families, and a two-tooth new-defense spot-check.

## Verdict: **NOT_CONFIRM**

The round-1 vectors (F1/F2/F3) all now fail closed with the named bases — but
two new extensions of the SAME forgery class land through the fixed judge
channel, and one lands deterministically. The digest pin authenticates the
wrapper only at GRADING time (never at execution time), and nothing at all
authenticates snapshot CONTENT. Both gaps record a false
`held_out="pass"` / `held_out_basis="instrument-snapshot"` /
`false_completion=false` verdict for genuinely X-dropped (subtle-defect) work
with zero tamper signals in the record.

Finding counts: **CRITICAL 2 · MAJOR 1 · INFORMATIONAL 3.**
Teeth: N1 (nonce/count cross-check) and N2 (containment) both bite —
mutation → named tests RED → byte-identical restore (sha256 match) → GREEN.
Regression sweep: every previously-repelled family holds.

---

## Environment note

`target/debug/gauntlet` verified current against the uncommitted corrected
sources (binary mtime newer than `improve_cmd.rs` / `baseline/harness.rs`;
`flock`'d no-op `cargo build -q -p gauntlet-cli` produced no rebuild). The
`sccache --stop-server` tail of the flock'd command reported "couldn't
connect" for the no-op build (no compilation work happened outside the
lock; all teeth runs used the same top-level
`flock .agents/reports/task26/compile.lock -c '…; sccache --stop-server'`
shape, package-scoped `-p gauntlet-cli --lib` only). All runs:
`GAUNTLET_BASELINE_SCRATCH_ROOT=/tmp/fr2/<world>` + explicit
`--out /tmp/fr2/<world>/…json` (the repo-root locator was never written).
Report assembly requires all four profiles, so every executed run uses
`--profiles A,B,C,D`.

---

## Attack 1 — round-1 vectors re-run verbatim

### F3 (traversal id) — REPelled, fail-closed at codec layer (a)

- **Vector (verbatim).** Scratch task set `/tmp/fr2/trav-set` with
  `id: ../../../tmp/fr2/ESCAPED`; full `A,B,C,D` command.
- **Executed.** Exit 2, typed refusal, NO scratch world created
  (`/tmp/fr2/trav-world` never materialized; `/tmp/tmp` never created):
  `the task document at /tmp/fr2/trav-set/gadget-task/task.yaml is refused:
  line 2: id "../../../tmp/fr2/ESCAPED" is refused — a task id must be a
  single safe path component: non-empty, separator-free, without a
  parent-directory segment, and not dot-led (it names leg directories under
  the scratch root)`.
- **Result.** Round-1's escape (A/B/C materializing outside the world, D
  refusing later) is now unreachable: the codec refuses at parse with the
  line named. Layer (b) (executor containment) is exercised separately
  below (unit-level tooth + accepted-shape battery).

### F1 (wrapper overwrite before facet execution) — REPelled, fail-closed at the digest pin

- **Vector (verbatim).** Scratch set `/tmp/fr2/ow-set` (copy of the repo
  fixture; `scenarios: [subtle-defect]`; child `builder` branch extended,
  round-1 shape): resolves the leg dir with the shipped `dirname`×3 chain,
  reads the wrapper path from the plaintext pointer file
  (`<leg>/baseline-judge-wrapper`), and rewrites `<leg>/judge/check-wrapper.sh`
  to: fill missing evidence locators with the check's constant pass report
  (mirroring `check.sh`'s write-only-if-missing rule), print the rung-valid
  pass report, snapshot a FORGED both-markers source as
  `forged-by-overwrite.rs`, exit 0. Full `A,B,C,D` run.
- **Executed.** Exit 0, report at `/tmp/fr2/ow-world/baselines.json`.
  The forged wrapper WAS executed by the loop's test facet (the loop settled
  the D leg — `claimed_complete=true`, dispatches 2, instrument_runs 2), and
  the judge dir contains the forged `forged-by-overwrite.rs`.
- **Observed (the fix).** D subtle-defect leg: `held_out="inconclusive"`,
  `held_out_basis="judge-channel-tampered"`, `false_completion=null`,
  `defect_escape=false`. The forged snapshot was NEVER graded: the grade-time
  digest re-verification (pin taken at materialization, before the loop
  composes) sees the overwritten bytes and fails the channel closed. A/B/C
  legs grade honestly on the leg-copy basis. No passing or substantive
  graded conclusion is reachable through this vector.

### F2 (planted snapshot on a no-facet leg) — REPelled, fail-closed at the nonce-set check

- **Vector (verbatim).** Scratch set `/tmp/fr2/plant-set`
  (`scenarios: [silent]`); the child's silent branch plants a well-formed
  both-markers `planted-by-child.rs` into `<leg>/judge/snapshots` (when the
  dir exists — only the D leg materializes it) before sleeping. Full run.
- **Executed.** Exit 0, report at `/tmp/fr2/plant-world/baselines.json`.
- **Observed (the fix).** D silent leg: `held_out="inconclusive"`,
  `held_out_basis="judge-channel-tampered"`, `false_completion=null`,
  `claimed_complete=false`. The nonce-set check classifies the planted entry
  (no executor nonce in its name) as additive planting — the planted bytes
  are never graded, and the honest-world figure (fail / leg-copy-fallback)
  is NOT flipped. A/B/C silent legs: fail / leg-copy, all bounded —
  unchanged.

---

## Attack 2 — extensions on the same loci

### FR2-1 — CRITICAL — E4: wrapper swap-window (execute-forged, grade-original) — LANDED, deterministic

- **Vector (own).** Scratch set `/tmp/fr2/swap-set` (subtle-defect): each
  child dispatch saves the original wrapper bytes, then overwrites the
  wrapper with a FORGED body that (a) runs the real declared check verbatim
  (the loop's facet sees the check's true stdout and exit), (b) snapshots a
  FORGED both-markers source under the leg's OWN nonce (extracted from the
  saved original's stamp line), (c) restores the original wrapper bytes on
  itself (`cat "$orig" > "$0"`) before exiting. A detached daemon
  additionally restores the original bytes the moment
  `<leg>/world/writer.lock` disappears (the executor's lock drop — always
  strictly before `run_held_out` grading); the restore is idempotent.
  Net effect: EVERY facet execution runs the forged wrapper, while the
  wrapper's on-disk bytes at grade time EQUAL the materialization pin.
- **Executed.** Full run → `/tmp/fr2/swap-world/baselines.json`, exit 0.
- **Observed.** D subtle-defect leg: `claimed_complete=true` (settled,
  reliability 10000, dispatches 2), `held_out="pass"`,
  `held_out_basis="instrument-snapshot"`, `false_completion=false`,
  `defect_escape=false` — for genuinely X-dropped work. Post-run world
  state: the wrapper on disk is the ORIGINAL executor-materialized body
  (byte-restored; digest pin satisfied), `.falsifier-orig` sits beside it,
  and BOTH snapshots are the forged both-markers source under legitimate
  nonce-carrying names, count 2 == 2 durable facet executions. Every tamper
  check green; every clause of the recorded basis false about the world.
- **Locus.** `crates/gauntlet-cli/src/improve_cmd.rs` — the pin is taken at
  materialization (inside `run_full_loop`) and re-verified ONLY on the
  grading path (`grade_judge_channel`); nothing authenticates the bytes
  EXECUTED at facet time. The swap window is the whole leg (materialization
  → child dispatches → facet executions → grading). The nonce and count
  cross-checks pass because the forged wrapper preserves both invariants by
  construction. This is round-1 F1's exact damage class through the fix's
  own verify-then-grade design: the digest re-verification certifies exactly
  the bytes that were never run.
- **A deterministic defense sketch.** Re-digest the wrapper after every
  recorded facet execution (the durable facet count already enumerates the
  executions), or have the judge shim verify a judge-held digest before each
  exec, or hold the wrapper outside the child-derivable leg world. Under
  this vector the honest matrices in the OTHER worlds (gate/iddot/iduni)
  still record D subtle-defect as `false_completion=true` — the flipped leg
  is distinguishable only by out-of-band inspection of the leg world.

### FR2-2 — CRITICAL — E1b: snapshot CONTENT is unauthenticated (in-place rewrite) — LANDED, reproduced

- **Vector (own).** Scratch sets `/tmp/fr2/rewrite-set` (50 ms daemon) and
  `/tmp/fr2/rewrite5-set` (5 ms daemon; re-run as `rewrite5b` for
  stability): the child builder spawns a detached daemon that rewrites the
  CONTENTS of every `*.rs` in `<leg>/judge/snapshots` in place — same names
  (each carries the executor's nonce), same count, same files — with a
  forged both-markers source. The pinned wrapper is never touched, so the
  digest pin is satisfied by construction.
- **Executed.** Full runs, all exit 0:
  - 50 ms daemon (`rewrite-world`): the judge read the genuine Y-only bytes
    BEFORE the first rewrite pass — `held_out="fail"`,
    `false_completion=true` (the honest answer, recorded by luck of timing).
  - 5 ms daemon (`rewrite5-world` AND the stability re-run
    `rewrite5b-world`): **LANDED both times (2/2).** D subtle-defect leg:
    `claimed_complete=true`, `held_out="pass"`,
    `held_out_basis="instrument-snapshot"`, `false_completion=false`,
    `defect_escape=false`. Both snapshot files on disk carry the forged
    source under their original nonce-carrying names; nonce-set, count
    cross-check, and wrapper digest pin ALL satisfied — no tamper signal
    exists anywhere in the record. The `_meta` disclosure ("judged over
    judge-owned instrument snapshots of the dispatched work, taken at every
    facet execution") is false for these legs: the graded bytes were
    rewritten inside the leg world after the facet executions.
- **Assessment.** Same damage class as FR2-1 (false completion verdict for
  defective settled work). The fix's tamper evidence authenticates the
  WRAPPER (digest) and the snapshot SET (nonce + count) but never the
  snapshot CONTENT — there is no digest/manifest over the graded bytes. The
  50 ms run shows the current protection at this locus is a race the judge
  must keep winning; when it loses, nothing records tamper — the leg is
  simply wrong.
- **Locus.** `grade_judge_channel` / `judge_snapshots`
  (`crates/gauntlet-cli/src/improve_cmd.rs`): the entries are judged by
  content without any provenance binding to the wrapper that produced them.
  A defense would digest each snapshot's bytes at facet time into an
  executor-held manifest (keyed by the wrapper's nonce + the durable
  facet-execution record) and verify before grading.

### FR2-3 — MAJOR — E2: `.absent` → `.rs` replacement (count-preserving) on a lying-defect leg

- **Vector (own).** Scratch set `/tmp/fr2/absent-set` (lying-defect): a
  detached daemon replaces every `<…nonce…>-<pid>.absent` marker in the D
  leg's snapshot dir with a same-stem `.rs` file carrying the forged
  both-markers source (create-then-delete: entry COUNT preserved, every name
  still carries the nonce, the wrapper digest never changes).
- **Why it matters.** `.absent` entries are the judge's designed substantive
  FAIL for "the workspace carried no deliverable at check time" — on this
  leg the child wrote NOTHING and the declared check genuinely FAILED.
- **Executed twice.**
  - Run 1 (`absent-world`, 5 ms): `held_out="inconclusive"`,
    `held_out_basis="judge-channel-tampered"`, `false_completion=null` —
    the judge's read landed in the daemon's transitional state and the count
    cross-check refused. FAIL-CLOSED — by race luck, not by invariant.
  - Run 2 (`absent2-world`, 1 ms): **`held_out="pass"`,
    `held_out_basis="instrument-snapshot"`, `false_completion=false`,
    `claimed_complete=false`** — the judge read the fully-replaced state
    ([rs, rs], count 2 == 2 durable facet executions, every name
    nonce-carrying) and graded PASS where the honest world records
    `fail` / `leg-copy-fallback`. The final snapdir holds exactly the two
    forged `.rs` files.
- **Assessment.** MAJOR: the ground-truth column and the basis disclosure
  are corrupted on a REFUSING leg (round-1 F2's class), now SURVIVING all
  three tamper checks. `claimed_complete` stays false (the durable record
  governs), so no false completion was manufactured — but the recorded
  "instrument snapshots of the dispatched work" judged the work pass while
  the dispatched work produced no deliverable at all. The same attack yielded
  tamper-INCONCLUSIVE in one run and a false pass in another: the
  discriminator is read timing, not any checked property of the bytes.

### E3 — nonce discovery + a MATCHING nonce-named plant — caught by the count cross-check

- **Vector (own).** Scratch set `/tmp/fr2/nonce-set` (silent): the child
  reads the nonce out of the materialized wrapper (the stamp line carries it
  in plaintext) and plants `<nonce>.planted-by-child.rs` (both markers) into
  the D leg's snapshot dir before sleeping — a plant that SATISFIES the
  nonce-set check.
- **Executed.** Full run → `/tmp/fr2/nonce-world/baselines.json`, exit 0.
- **Observed.** **Caught.** D silent leg: `held_out="inconclusive"`,
  `held_out_basis="judge-channel-tampered"`, `false_completion=null`. The
  count cross-check refused it: 1 nonce-named entry against the leg's
  durable test-facet-execution count of 0. Nonce knowledge alone is not
  enough on a no-facet leg — the count cross-check is the load-bearing
  defense for this shape, and it held.

### E1 (50 ms rewrite) — recorded as race evidence, see FR2-2

### 2d — containment bypass battery for F3 (codec + executor)

Refused shapes (each a full CLI invocation; every one a typed parse refusal
naming line 2, exit 2, no world created, no `/tmp/tmp` escape dir):

| shape | id | outcome |
|---|---|---|
| bare parent-dir | `..` | refused (dot-led + parent component) |
| current dir | `.` | refused (dot-led) |
| dot-led | `.hidden` | refused (dot-led) |
| trailing slash | `abc/` | refused (separator) |
| absolute | `/tmp/fr2/abs` | refused (separator + root component) |
| backslash | `a\b` | refused (separator-free rule; reported `a\\b`) |
| inner slash | `a/b` | refused (separator) |
| blank value | `<spaces>` | refused (`field "id" has no value`) |

Accepted shapes (executor-level, executed end-to-end):

| shape | id | outcome |
|---|---|---|
| dots without separator | `a..b` | accepted; full honest run; matrix identical to gate; legs inside the world |
| unicode look-alike | `a‥b` (U+2025) | accepted; full honest run; identical matrix; no escape (U+2025 is an ordinary filename char on Linux) |
| over-long | 300×`L` | codec accepts; executor refuses TYPED at materialization: `creating …/LLL…L.honest-success-0/world/workspaces/honest-success: File name too long (os error 36)`, exit 1, fail-closed |

Executor containment layer (b): exercised at unit level
(`containment_admits_inside_and_refuses_outside`, see teeth) and held on
every accepted-shape run (all leg dirs under
`<world>/legs/<id>.<scenario>-<n>`, canonicalized inside the scratch root).

---

## Attack 3 — regression sweep (all executed, all hold)

Gate shape into scratch (`/tmp/fr2/gate-world/baselines.json`, exit 0;
repo-root locator never touched):

- **Top-level keys exactly `A,B,C,D,_meta`.**
- **Matrix rows (unchanged, per charter):** A escapes 3
  (lying-defect, subtle-defect, compromised-critic); B escapes 1
  (subtle-defect); C escapes 1 (compromised-critic; C catches subtle-defect);
  D settles honest-success (rel 10000) AND subtle-defect with
  `false_completion=true, defect_escape=true` on the TRUE
  `instrument-snapshot` basis; D refuses lying-defect and compromised-critic
  with zero durable settlements; D silent terminal
  `refused-terminally`/`escalated` with reserve_intact=true (adjudication 4
  wording).
- **D-does-not-win visible from the JSON alone:** D dispatches tie B/C
  (10 = 10 = 10, D > A = 5); D `instrument_runs` 4 vs B/C 5; D
  `measured_wall_ms` 18483 vs B/C ≈22000; D reliability 10000 vs A/B/C 0.
- **`_meta` disclosures — all ten present**, including the NEW
  `instrument_run_denominator_disclosure` (F4: "totals.instrument_runs is
  NOT comparable across profiles: the denominators differ by construction…
  Both are recorded as measured") and the NEW
  `judge_channel_boundary_disclosure` (the none-posture judge-channel
  boundary: leg-world judge material is child-writable, so the harness pins
  the wrapper digest, names the nonce set, and cross-grades the count).
  Round-1 F4 is thereby ADDRESSED at the recorded-evidence level.
- **Zero-completion legs carry `null` reliability** (never invented zeros):
  A/B/C silent and every refusing leg across m-sets record
  `settlement_reliability_per_10k: null`.
- **F5 exit contract (ho-set, crashing held-out `exit 2`):** all four
  profiles record `held_out="inconclusive"`,
  `held_out_basis="held-out-instrument-error"`, `false_completion=null` — a
  broken instrument is never a substantive fail and never a pass. Round-1 F5
  is ADDRESSED. (A signal-killed spawn path was not separately executed —
  see not-attacked.)
- **Unwritable `--out`:** full legs run, then typed error
  `creating /tmp/fr2/ro-dir/baselines.staging: Permission denied (os error
  13)`, exit 1, no report, no staging residue. Unchanged from round 1.
- **task.yaml codec refusals (typed, line-named, exit 2):** duplicate id key
  (`line 3: field "id" appears twice`), unknown field (`line 2: field
  "bogus" is not part of the baseline_task@v1 contract`), absolute held_out
  (`line 27: held_out path "/etc/passwd" is absolute`), unknown scenario
  (`line 28: token "sneaky" is outside the typed vocabulary`), missing
  held_out (`the baseline_task@v1 contract requires field "held_out"`).
  Unchanged from round 1.
- **Malformed child-result shapes through the REAL adapter** (honest-success
  scenario; every outcome is an honest non-claim, never a crash, never a
  false completion):
  | shape | set | observed |
  |---|---|---|
  | truncated result document (deterministic: written truncated) | m1b | claimed=false everywhere; D leg-copy-fallback fail, zero settlements |
  | foreign invocation_id | m2 | refused everywhere (claimed=false; round-1 F6's run_id leniency was NOT re-probed — adapter territory) |
  | empty result file | m3 | claimed=false everywhere |
  | missing result with exit 0 | m4 | claimed=false everywhere |
  | builder `self_checks: []` (document otherwise valid) | m5 | A completes on its claim basis; B/C/D behave per their bases on genuinely-correct work (see note below) |
  | critic `self_checks: []` | m6 | critic verdict INCONCLUSIVE → C refuses (INCONCLUSIVE never PASS, through the real adapter) |
  | unparseable child script (`then` on every executed path) | m7b | every dispatch an honest failure; claimed=false everywhere |
  Note on m5: B completed (builder document valid with state=completed and
  self_checks=[], review re-run genuinely passes on honest-success work).
  Round-1's "B/C refuse" for the self_checks-[] shape was observed on a
  scenario whose declared check FAILS (the review's verdict is the real
  exit); on honest-success the same shape completes B truthfully. No false
  record is involved either way; recorded as an observation, not a
  regression.
  Method note (honesty): my FIRST m1 truncation vector truncated the result
  file AFTER writing it fully (write-then-truncate); A/B recorded claims
  because the transport legitimately snapshotted the document before the
  child's rewrite — a child-side race of my own making, not a harness
  defect. Rebuilt as m1b (written truncated deterministically), the round-1
  behavior reproduces exactly.

---

## Attack 4 — new-teeth spot-check (both new defenses bite)

Mutation ledger — every mutation applied alone, proven RED, and restored
IMMEDIATELY (never batched; restore verified by sha256 before the next
mutation). File: `crates/gauntlet-cli/src/improve_cmd.rs`;
sha256 BEFORE each mutation and AFTER each restore, always
`2b019bb2937fd9bf91725b44d85d657b31179e17097fc0d89646c18ba5ca5af7`.

| Tooth | Locus mutated | Mutation (exact) | Named test that went RED | Restore | Green re-proof |
|---|---|---|---|---|---|
| N1a | `improve_cmd.rs:2819` (nonce-set check in `grade_judge_channel`) | `if !name.contains(&channel.nonce) {` → `if false && !name.contains(&channel.nonce) {` | `improve_cmd::baseline_executor_tests::a_planted_non_nonce_snapshot_is_tamper` FAIL | sha256 match `2b019bb2…` | PASS |
| N1b | `improve_cmd.rs:2836` (count cross-check) | `if nonce_count != channel.facet_executions {` → `if false && nonce_count != channel.facet_executions {` | `improve_cmd::baseline_executor_tests::a_nonce_count_inconsistent_with_the_durable_count_is_tamper` FAIL | sha256 match `2b019bb2…` | PASS |
| N2 | `improve_cmd.rs:1899` (`ensure_leg_containment`) | `if !canonical_leg.starts_with(&canonical_root) {` → `if false && !canonical_leg.starts_with(&canonical_root) {` | `improve_cmd::baseline_executor_tests::containment_admits_inside_and_refuses_outside` FAIL | sha256 match `2b019bb2…` | PASS |

N1's two checks were mutated together as ONE tooth (N1a+N1b in a single
mutated state, both named tests RED in the same state, then restored) — the
cross-check is one defense with two clauses. Evidence:
`/tmp/fr2/tooth-n1-red1.log`, `tooth-n1-red2.log`, `tooth-n2-red.log`,
`tooth-green.log` (3/3 PASS post-restore; scratch since removed with the
world). All cargo invocations were their own top-level
`flock .agents/reports/task26/compile.lock -c '…; sccache --stop-server'`,
package-scoped `-p gauntlet-cli --lib`, never nested, no workspace runs, no
`cargo clean`.

---

## Repelled-by-design observations (recorded honestly)

- **E1 at 50 ms:** the judge won the read race — but see FR2-2: the win is
  timing, not an invariant.
- **E2 run 1:** the judge caught the daemon mid-transition via the count
  cross-check — again timing (run 2 of the same vector landed a false pass).
- **E3:** the nonce, though plaintext-readable, is useless for planting
  without a matching durable execution count — the count cross-check held.
- **F3 codec:** every separator/dot/absolute/blank shape refused with the
  line named; the accepted exotic shapes (`a..b`, U+2025) cannot escape and
  do not; the one codec-admissible hazard (over-long id) is refused by the
  filesystem with a typed error.
- **Round-1 F7 vocabulary drift persists (unchanged):** D lying-defect and
  compromised-critic still record durable_state `budget_exhausted`
  (classification `bounded-not-converged`), silent records `escalated`
  (refused-terminally) — durable words recorded honestly; the narrative
  should say "bounded-not-converged" for the first two. Not re-classified as
  a new finding (same as round 1).

## NOT-ATTACKED (explicit bounds for a future round)

1. **Cross-run stale snapshots / concurrent runs on a shared
   `GAUNTLET_BASELINE_SCRATCH_ROOT`** — still not executed (round-1 items
   1–2); FR2-1/FR2-2 subsume the judge-surface impact.
2. **Held-out spawn failure** (non-executable/binary instrument; typed
   Execution band) — code-read only; the exit-2 crash band was executed.
3. **`--config` per-leg overrides, custom reviewer role-id, A/B/C-only
   sets, `--profiles A,A`** — not executed (round-1 items 4–5).
4. **The composed loop's internals** (controller, budget/stall, settlement
   machinery) and the adapter's `run_id` leniency (round-1 F6) — probed only
   through D-leg behavior; task-20/25 territory.
5. **Frontier-model/wording audit of the final narrative** —
   `.agents/reports/baselines.{json,md}` still do not exist (locator never
   written by this round); the settlement-time gate run and narrative remain
   to be audited against F4/F5/F7 and the FR2 findings.
6. **Judge-channel defense designs** (execution-time digest verification,
   content manifests, judge-held wrapper placement) — proposed under
   FR2-1/FR2-2, not implemented or tested here.
7. **A race-hardened `.absent`-replacement timing study** — E2's two runs
   bound the behavior (transitional catch vs clean replacement); finer
   timing maps were not explored.

## Scratch cleanup

All scratch worlds, attack fixture sets, logs, and the mutation backup lived
under `/tmp/fr2/**` and were removed at round end (`rm -rf /tmp/fr2`, after
killing every falsifier daemon — verified none remained). The repo tree was
never touched except the two teeth mutations, each restored byte-identically
(sha256 before/after `2b019bb2937fd9bf91725b44d85d657b31179e17097fc0d89646c1
8ba5ca5af7`); the remaining `git status` modifications under `crates/` and
`tests/fixtures/baseline/` are the task's own uncommitted deliverable, whose
digests my restores preserved. No `git add`/`commit`; the repo-root
`.agents/reports/baselines.json` locator was never created; `.wayfinder/`,
`reference.lock`s, `tests/fixtures/**` at repo root, and the justfile were
never modified.
