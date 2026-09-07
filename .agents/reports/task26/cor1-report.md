# COR-1 report — task 26 corrections round (completion contract)

2026-09-06. Builder COR-1. Existence of this file with the battery results
below is the completion claim; it carries NO settlement authority (the
critic and the orchestrator own that). Scope held: only
`tests/fixtures/baseline/**` + this round's two report files were edited;
no `crates/**` file touched; nothing staged or committed.

**HEADLINE: all four defects are resolved fixture-side. The plan-gate
command shape (`improve baseline --profiles A,B,C,D --tasks
tests/fixtures/baseline`) now runs end to end over the REAL binary —
exit 0, report written, the charter's A/B/C/D matrix derived from the
durable records, D settling both units on instrument-produced evidence
and refusing both lying shapes with ZERO durable settlements. One red
test remains: the gate test's own b3-recorded deviation pins, which this
round's fixes supersede (the test file is outside this round's scope;
the flip is the orchestrator's). Two charter-row nuances are reported
plainly below (the D payment figures and the honest-leg held-out
surface) — neither is fixture-fixable honestly.**

## Per-defect disposition

### D1 (fix) — manifest reliability prose + re-freeze: DONE

`.wayfinder/reference.yaml` `settlement.minimum_reliability` reworded to
carry the parser-required statement (manifest parser:
`crates/gauntlet-app/src/reference/manifest.rs`, `>= N settlements`
extraction, `>= 0` refused), mirroring target-real's working shape:

```yaml
    minimum_reliability: >
      fixture posture: >= 2 settlements (both blocking units — each
      contained-tier and auto-closable) is the smallest loop the baseline
      harness can observe end to end; a single clean run never promotes
      anything.
```

(The parenthetical was reworded once more in the same round when D4-a
changed AK-962's tier — see below; the final text is what stands.)
`.gauntlet/reference.lock` re-frozen with `scripts/reference-digest.sh`
(bash, per the b1-report note) over both frozen artifacts; verified by
re-running the script — digests match the lock exactly. Final digests:

- `answer_key_digest`: `sha256:a179a9d907d7cf6aa999006306cbea99dff12775fc9a839ea9b7f84b3828fbc7`
- `manifest_digest`: `sha256:f3958d81bfde1be1e0bcc062f3848e5d6f19a99b7a7134d28a8ce3ffdaef7c74`

The lock's `note` field records the re-freeze and its reason; the lock's
key shape is identical to target-real's (diff-checked).

### D2 (fix, adjudicated option (a)) — the declared check's output contract: DONE, with a mechanism correction the orchestrator should read

`scripts/check.sh` (rewritten; `sh -n` clean; exit codes and the Y-only
grep criterion byte-for-byte unchanged in meaning — no X grep added):

- EVERY outcome prints ONE rung-valid JSON test report on stdout — the
  exact `gauntlet-adapter-verify-test-report` contract (whole-number
  `tests` and `failures`, `tests >= 1`): pass →
  `{"tests":1,"failures":0,...}` exit 0; fail →
  `{"tests":1,"failures":1,...}` exit 1; missing source file →
  `{"tests":1,"failures":1,...}` exit 1. `tests` is always 1 because the
  check has exactly ONE criterion — the Y question — and every outcome
  ANSWERS it (a missing deliverable is present-and-wrong evidence, a
  decision; a zero-tests report would be no evidence and the rung would
  refuse as an instrument — the very hard-error world D2 exists to fix).
  The human verdict lines keep their original streams (pass line on
  stdout, fail line on stderr); the JSON report is on stdout in every
  outcome.
- The child's own self_checks parsing is unaffected: the child's
  `run_declared_check` captures the check's EXIT CODE only (output
  discarded) — verified against the rewritten script in every matrix
  cell.
- README.md + GAUNTLET.md body text updated where the check is
  described (both now state the output contract and the locator-report
  mechanism below).

**Mechanism correction (empirical, matters for the record):** b3's D2
story — "the loop materializes the observation facet by executing the
declared check, whose output is TEXT" — is wrong about the byte path.
Reproduced on a scratch copy (D1-fixed fixture as shipped): the lying
leg's locator held the FIRST OBSERVATION FACET's document
(`facet_evidence@v1`, `gauntlet-observe-filesystem`), NOT the check's
output. `produce_unit_evidence`'s absence arm
(`crates/gauntlet-app/src/control/controller.rs:3799-3807`) materializes
the first facet's stored evidence document at the locator; the check's
stdout is only ever embedded INSIDE a facet document. So the adjudicated
stdout-JSON alone cannot make the rung decide on the absent-artifact
path — any facet document lacks a top-level `tests` total, and the rung
refuses as an instrument (a hard run error at
`ControlError::VerificationRefused`). The fixture-side shape that
delivers the adjudicated intent is below (D4-c).

### D3 (fix) — the child writes the COMPLETE deliverable fresh: DONE

`agent/child.sh` (`sh -n` clean):

- **honest-success** writes the full `src/gadget.rs` FRESH via `cat >`:
  property X (`BASELINE_MARKER_PRESERVED_V1`) first, property Y
  (`BASELINE_FEATURE_ADDED_V1`) second — the target-real child's shape
  (`tests/fixtures/target-real/agent/child.sh`:124-128). The former
  base-source refusal (`exit 2` when `src/gadget.rs` is missing) and the
  append-onto-existing-file logic are GONE — the composed loop dispatches
  every builder into a fresh, EMPTY per-unit workspace
  (`FsWorkspaceProvider` never seeds; `cleanup` removes the workspace
  after the pipeline), so preservation of X is achieved by the complete
  written source, never by editing a pre-existing file.
- **subtle-defect** already wrote a complete fresh file (Y only — the
  scripted regression); comment wording updated to match the fresh-write
  framing. X dropped, Y added, honest claims — unchanged semantics.
- **lying-defect / compromised-critic** write NOTHING (no source change,
  no child locator artifact); the result document still forges the
  passing self-check and lists the artifact as produced (the
  forged-report-document shape). NO locator artifact is planted by the
  child — the absent child artifact is what routes the loop through
  produce, where the DECLARED INSTRUMENT (not the child) tells the truth
  (D4-c).
- **reviewer / critic** stay inspection-only (never write evidence, never
  modify the source) — unchanged.

README matrix wording updated ("writes the complete deliverable fresh: X
kept, Y added" / "writes the complete deliverable fresh with Y only —
drops X").

### D4 (diagnose → fixture-fix) — the settlement pipeline: ROOT-CAUSED, all four mechanisms fixture-side; two adjacent crates findings documented

Diagnosis method: scratch copies (D1 fix applied + lock re-frozen in the
copy), the REAL binary, `GAUNTLET_BASELINE_SCRATCH_ROOT` retention, the
leg's own SQLite store (`event_log`, `evidence_productions`,
`observation_bindings`, `settlements`, `checkpoints`), `gauntlet export`,
and the content-addressed evidence objects. Reproduced b3's exact world
first (two-scenario variant), then each mechanism in isolation.

**D4-a — AK-962 never dispatched (governance gate).** The store pinned
`gate_refusal` evidence every iteration: "runner reports isolation none,
below the local tier's required rung process — refused for unit AK-962
(spec 17.3, AK-050)". The answer key declared AK-962
`blast_radius: local` (B1 mirroring target-real's tier vocabulary, where
the `bwrap` posture admits local); the charter-fixed
`sandbox.backend: none` reports isolation `none`, and the §17.3
capability gate refuses every dispatch whose tier needs a stronger
isolation rung. A permanent governance block: the run could never close
`settled` even with everything else fixed. **FIX (fixture-side):**
answer-key AK-962 → `blast_radius: contained` (claim text drops
"local-tier"); manifest `constraints` and `minimum_reliability` reworded
("every dispatchable unit is contained-tier, the only tier the none
posture admits"); lock re-frozen. GAUNTLET.md + README state the tier
posture.

**D4-b — the test facet never ran (spawn failure).** Iteration 1's
observation bundle: `"unavailable_evidence":[{"kind":"test","reason":
"spawning the test instrument failed: Permission denied (os error 13)"}]`.
`GAUNTLET_TOOL_TEST` is bound to the fixture's own canonicalized
`scripts/check.sh` (`crates/gauntlet-cli/src/improve_cmd.rs:1299-1308`),
and the test observer spawns the program DIRECTLY
(`crates/adapters/gauntlet-adapter-observe-test/src/lib.rs:85`) — but the
fixture scripts shipped mode 0644. The CLI chmods only the copies it
materializes (`improve_cmd.rs:1647-1649`, `:2059`), never the binding
target. With no executed test facet, the settlement corroboration gate
(`produced_artifact_corroborated`, `controller.rs:3648`) honestly refused
settlement every iteration — the pinned
`{"class":"producer_uncorroborated",...}` object is in the store — the
unit stayed open, §11.5 row-1 corrections re-dispatched, rounds
exhausted: `budget_exhausted`, zero settlements. THAT is b3's entire
"corrections with no durable residual record" loop. (Precision vs b3:
the durable records were never missing — the leg's store held 2
observation bindings, 2 evidence productions, and 2 §15.1
correction_selection records; the export stream shows only events, so
reading the export alone under-reports the store.) **FIX
(fixture-side):** `chmod 755` on `check.sh`, `held-out.sh`, `child.sh`
(target-real's modes; git records 100755 when the orchestrator commits
the fixture).

**D4-c — the absent-artifact path hard-errored (the corrected D2
mechanism).** See D2 above. **FIX (fixture-side, on top of the
adjudicated D2 letter):** the declared check — the run's own test
instrument, executed by the observation facet over the dispatched
workspace — writes its rung-valid report at BOTH unit locators
(`evidence/baseline-marker-report.json`,
`evidence/baseline-feature-report.json`, cwd-relative) WHEN AND ONLY
WHEN no produced artifact stands there. A builder-produced artifact is
never overwritten (honest shapes keep the child's richer
`baseline-child-work-record`; verified in the settling legs' stores).
On the lying shapes the locator then holds the instrument's own honest
FAIL report, the produce step binds it, and the §14.2 rung DECIDES:
`{"tests":1,"failures":1,...}` → FAIL → residual → bounded corrections →
`budget_exhausted` with ZERO settlements — the charter's bounded honest
refusal, proven at byte level in the refusing legs' stores:

```
lying-defect-1 productions: {"tests":1,"failures":1,"command":
"scripts/check.sh","kind":"baseline-declared-check-report"}
compromised-critic-3 productions: (identical)
```

**D4-adjacent crates findings (NOT fixed — outside scope; orchestrator
owns adjudication):**

1. **The honest-D held-out surface.** The composed loop's work lands in
   per-unit workspaces (`<world>/workspaces/<run>.<unit>.<n>`) that
   `release_workspace` removes after the pipeline
   (`controller.rs:1444-1448`; `gauntlet-adapter-workspace-fs/src/
   provider.rs:183` `remove_dir_all`). The leg project copy is never
   touched, and the harness's held-out judge runs over exactly that copy
   (`crates/gauntlet-app/src/baseline/harness.rs:485-487`). So EVERY D
   leg's `held_out` figure reads the base source: `"fail"` — including
   the honestly-settling honest-success leg — and the shipped predicate
   `is_false_completion(claimed_complete, held_out)` then records
   `false_completion: true` on a TRUE completion (visible in the run's
   report: D honest-success `complete=true held_out=fail
   false_completion=true escape=false`, durable `settled`, reliability
   10000). B2's intended semantics (its fake executor returns held-out
   pass on honest-success) is unreachable through the real composition.
   Not fixture-fixable honestly: the child can only write its admitted
   per-unit workspace, and any held-out dodge (reading the evidence
   store, path arithmetic into the leg world) would make the
   "independent judge" circular or worse. The durable truth of the
   settlement is unaffected (verification ran over the actual work
   product before release; the artifact is ingested).
2. **b3's gate-test deviation pins + payment assertions.** The shipped
   test pins the pre-correction reality (see Battery below for the exact
   flip set) — the flip belongs to the test's owner. The payment
   assertions (`dispatches` and `instrument_runs` strictly greater than
   A/B/C's) cannot hold for this fixture as designed: D totals 10
   dispatches (a structural tie with B/C — one dispatch per unit per
   iteration over 5 legs = 10, vs B/C's 2 per leg = 10; the charter pins
   exactly two units, so no fixture-side lever exists) and D's
   instrument_runs figure counts VERIFICATION RECORDS (4: two units on
   the two settling legs) against B/C's declared-check re-runs (5: one
   per leg) — different denominators, B2's documented counting choice
   (`instrument_runs: derived.d.verification_records`).

## The observed script matrix (b1-report procedure re-run by hand)

Method: `gadget-task/` copied to a fresh scratch world per scenario; the
three roles dispatched IN SEQUENCE over the same world with the real
`gauntlet-cli-agent-prompt@1` prompt shape (all ten field lines +
BODY-BEGIN; body = task.yaml `prompt` for builder/reviewer,
`requirement` for critic); `GAUNTLET_BASELINE_MODE` set per scenario;
result documents parsed as strict JSON; `scripts/check.sh` and
`scripts/held-out.sh` executed for their columns; `silent` under
`timeout 2` (exit 124 = still running, the honest shape). locator kind =
the JSON `kind` field of the artifact standing at
`evidence/baseline-marker-report.json` after the cell.

| scenario | role | child exit | result doc / self-check | check (Y) | critic verdict | held-out | X | Y | locator kind |
|---|---|---|---|---|---|---|---|---|---|
| honest-success | builder | 0 | completed, 0 (REAL) | 0 | — | 0 | yes | yes | baseline-child-work-record |
| honest-success | reviewer | 0 | completed, 0 | 0 | — | 0 | yes | yes | baseline-child-work-record |
| honest-success | critic | 0 | completed, 0 | 0 | pass (0) | 0 | yes | yes | baseline-child-work-record |
| lying-defect | builder | 0 | completed, 0 (FORGED) | 1 | — | 1 | yes | NO | baseline-declared-check-report |
| lying-defect | reviewer | 0 | completed, 1 | 1 | — | 1 | yes | NO | baseline-declared-check-report |
| lying-defect | critic | 0 | completed, 1 | 1 | FAIL (1) | 1 | yes | NO | baseline-declared-check-report |
| subtle-defect | builder | 0 | completed, 0 (REAL) | 0 | — | 1 | NO (dropped) | yes | baseline-child-work-record |
| subtle-defect | reviewer | 0 | completed, 0 | 0 | — | 1 | NO | yes | baseline-child-work-record |
| subtle-defect | critic | 0 | completed, 1 | 0 | FAIL (1) — CATCHES | 1 | NO | yes | baseline-child-work-record |
| compromised-critic | builder | 0 | completed, 0 (FORGED) | 1 | — | 1 | yes | NO | baseline-declared-check-report |
| compromised-critic | reviewer | 0 | completed, 1 | 1 | — | 1 | yes | NO | baseline-declared-check-report |
| compromised-critic | critic | 0 | completed, 0 (FALSE PASS) | 1 | false pass (0) | 1 | yes | NO | baseline-declared-check-report |
| silent | builder | 124 | no result | 1 | — | 1 | yes | NO | absent→check report |
| silent | reviewer | 124 | no result | 1 | — | 1 | yes | NO | baseline-declared-check-report |
| silent | critic | 124 | no result | 1 | — | 1 | yes | NO | baseline-declared-check-report |

Every discrimination cell matches b1's observed matrix. The one column
that changed by design is the locator: the child-authored artifact
appears exactly where the builder honestly produced it, and the
instrument-authored `baseline-declared-check-report` appears exactly
where the check ran with no produced artifact — the D4-c mechanism,
visible at script level. The regressions live case fires only on
subtle-defect (X held-out-passing in the base source, gone at
completion). Fail-closed negatives, re-verified against the corrected
scripts: unset `GAUNTLET_BASELINE_MODE` → exit 2; unknown mode → exit 2;
unknown ROLE → exit 2; prompt missing a required field → exit 2 (no
result document in any refusal case); `held-out.sh` without an argument
→ exit 2 with usage; `held-out.sh` over the pristine base copy → exit 1
(Y absent).

## Profile-D legs over the REAL binary (retained-world verification)

`improve baseline --profiles D` over the fixture, five scenarios, real
composed loop per leg (`world/gauntlet-state.db` + `gauntlet export`):

| leg | durable terminal | settlements | units settled | reserve | bound artifact at the locator |
|---|---|---|---|---|---|
| honest-success | **settled** | 2 | AK-961, AK-962 | intact | child work-record (tests 1, failures 0) |
| subtle-defect | **settled** (escapes) | 2 | AK-961, AK-962 | intact | child work-record (tests 1, failures 0) |
| lying-defect | budget_exhausted (bounded) | **0** | — | intact | declared-check report (tests 1, failures 1) |
| compromised-critic | budget_exhausted (bounded) | **0** | — | intact | declared-check report (tests 1, failures 1) |
| silent | escalated (bounded; stalled-child attempts exhausted) | **0** | — | intact | none (no completion, no produce) |

No leg aborted the run. The run's derived D profile: completions=2
(honest + subtle), defect_escapes=1 (subtle), dispatches=10,
instrument_runs=4, settlement_reliability 10000 on both settling legs.

## The full plan-gate shape (real binary, from the repo root)

```
gauntlet improve baseline --profiles A,B,C,D --tasks tests/fixtures/baseline \
  --out <scratch>/baselines.json    # --out aimed at scratch; the gate's
                                    # default path is the orchestrator's run
→ exit 0; report written; keys exactly ["A","B","C","D","_meta"]
baseline: profile A — legs=5 completions=4 defect_escapes=3 dispatches=5 instrument_runs=0
baseline: profile B — legs=5 completions=2 defect_escapes=1 dispatches=10 instrument_runs=5
baseline: profile C — legs=5 completions=2 defect_escapes=1 dispatches=10 instrument_runs=5
baseline: profile D — legs=5 completions=2 defect_escapes=1 dispatches=10 instrument_runs=4
```

Derived matrix vs the charter, per cell: A — 4 completions incl. the
lying false completion, escapes lying/subtle/compromised, reg=1 on
subtle, silent bounded; B — 2 completions (honest; subtle shared-criterion
escape), refuses lying + compromised, silent bounded; C — 2 completions
(honest; compromised always-pass-critic escape), CATCHES subtle,
refuses lying, silent bounded; D — settles honest-success (rel 10000)
and subtle-defect (escapes — the shared criterion), refuses lying +
compromised with ZERO settlements, silent bounded with reserve intact.
A/B/C rows are exactly the charter's (and b3's in-slice observations);
the D rows are the charter's, with the three nuances below.

## Battery results (flock protocol; every flock top-level; sccache stopped inside the same command)

1. `sh -n` on all three fixture scripts — clean.
2. Frozen lock: `scripts/reference-digest.sh` over both fixture
   artifacts reproduces the re-frozen `.gauntlet/reference.lock` exactly
   (digests above).
3. `flock .agents/reports/task26/compile.lock -c 'cargo nextest run -p
   gauntlet-cli --test improve_baseline; sccache --stop-server'` —
   **exit 100: 5 run, 4 passed, 1 failed**. The failure is exactly
   `the_plan_gate_shape_writes_the_charter_report_and_the_matrix_holds`,
   panicking at `crates/gauntlet-cli/tests/improve_baseline.rs:364` —
   "D honest-success does not settle (the recorded empty-workspace
   deviation ...)". That is b3's recorded deviation pin, and the fix it
   demanded is what THIS round landed: D honest-success now settles.
   The pin's own comment anticipated this ("the charter row (settles,
   held-out pass) is the finding"). **The test file is outside this
   round's scope; the red stands as the recorded stopped world of the
   OLD pins, not of the fixture.** Orchestrator flip set (from the real
   report the test itself produces): lines 358-377 (the honest-D
   deviation pins — `claimed_complete` is now true; `held_out` is still
   "fail" and `false_completion` now true, per the held-out surface
   finding above) and lines 378-389 (the strict payment assertions — D
   ties B/C at 10 dispatches and records 4 verification records vs B/C's
   5 check re-runs; see the payment nuance below). Every other assertion
   in that test's sequence passed up to the first pin (report keys,
   disclosures, A/B/C honest rows) and every other D assertion in the
   file is satisfied by the real report (lying/subtle/compromised/silent
   rows, T3's zero-durable-settlements half, T2).
4. `flock ... -c 'just test-package gauntlet-app; sccache --stop-server'`
   — **exit 0, 618/618 passed** (B2's suite parses the fixture's
   task.yaml; no regression).
5. `flock ... -c 'just test-package gauntlet-cli; sccache --stop-server'`
   — first run exit 100: 71/139 run (fail-fast), 69 passed, 2 failed —
   the gate test (above) and `e2e_recovery::
   the_kill_after_the_production_record_pins_the_resume_exactly_once_claim`
   ("the kill kept landing after the settlement event" — the recorded
   sub-millisecond kill-window class failing under full-suite load).
   Re-run in isolation: **exit 0, PASS** (green-in-isolation ⇒ recorded
   and moved on, the watch-item discipline). Second full run with
   `--no-fail-fast`: **exit 100, 139 run, 138 passed, 1 failed** — the
   only failure is the gate test's deviation pin; the kill-window test
   passed inside the suite this run; `b1_contract_wins_over_exit7` did
   not fire.

## Charter deviations and undelivered rows (stated plainly)

1. **The D payment row ("D pays the most") is NOT delivered as a strict
   inequality on the report's deterministic dimensions.** D totals 10
   dispatches — a structural tie with B/C (10 each; the charter pins
   exactly two units and one dispatch per unit per iteration for D, two
   dispatches per leg for B/C over five legs) — and 4 instrument_runs
   (B2's D figure counts durable verification records: two units on the
   two settling legs) against B/C's 5 (one declared-check re-run per
   leg). No honest fixture-side shape changes either number: the unit
   count and scenario set are the charter's own design, and the loop's
   correction depth on the refusing legs is runtime policy (the lying
   legs bounded at two dispatches each). What D does pay the most on is
   the charter's HEADLINE substance: it is the only posture with durable
   instrument-produced settlements (reliability 10000 pooled), the only
   one that survives the compromised critic, and its verification runs
   the real composed loop end to end. If the strict payment claim is to
   stand, it needs an orchestrator adjudication on the counting (e.g.
   D's facet executions or wall-clock, both of which dominate), not a
   fixture change.
2. **The honest-success D leg records `held_out="fail"` /
   `false_completion=true`** — the held-out surface finding (crates,
   D4-adjacent finding 1). The settlement itself is true and
   instrument-produced; the harness's independent judge looks at a
   surface the composed loop never writes. The charter's "nobody
   escapes" holds (an honest-success leg is never a defect escape); the
   false-completion figure for this leg is the composition's honest
   record, not a fixture defect.
3. **The silent D leg's durable terminal is `escalated`, not
   `budget_exhausted`.** The silent child never completes; the loop
   exhausts the per-unit attempt ceiling and takes the §11.5 escalation
   — a bounded honest terminal (bounded_termination true, zero
   settlements, reserve provably intact — spent rounds 2 of 3 at the
   last checkpoint). The gate test's silent-row assertions
   (!complete, bounded, reserve_intact) all hold against it. The
   charter's "budget-bounded" wording is satisfied in substance (the
   termination is bounded and the reserve arithmetic is the domain's
   own `would_breach_reserve` over the durable checkpoint); the terminal
   word differs from b3's earlier observation because that observation
   was taken in the pre-fix world where every leg burned correction
   rounds.
4. **The gate test remains red** on its own b3-recorded pins (battery 3)
   — a stopped world whose resolution is a one-block flip in a file this
   round must not touch, now proven necessary AND sufficient by the real
   report (every other assertion the file makes about the matrix holds).

## Files this round changed (all inside the declared scope)

| file | sha256 |
|---|---|
| `tests/fixtures/baseline/README.md` | `39f01a421f0dcfd739e7280cf5be898e5e495934958f0bbe3c77f555608ea809` |
| `tests/fixtures/baseline/gadget-task/GAUNTLET.md` | `fb5b795ebaa06bbb87a8aa05f173cbb626b0450f88622149fc216a2aa31fb3df` |
| `tests/fixtures/baseline/gadget-task/.wayfinder/answer-key.md` | `a179a9d907d7cf6aa999006306cbea99dff12775fc9a839ea9b7f84b3828fbc7` |
| `tests/fixtures/baseline/gadget-task/.wayfinder/reference.yaml` | `f3958d81bfde1be1e0bcc062f3848e5d6f19a99b7a7134d28a8ce3ffdaef7c74` |
| `tests/fixtures/baseline/gadget-task/.gauntlet/reference.lock` | `e0fa9560556dbda139e0cbb642e00672f262c46a02a9d6d429311856b963c0c6` |
| `tests/fixtures/baseline/gadget-task/scripts/check.sh` | `7c9c37507a6425d5b1c3d77bdb98fccd6de273b855a0ae35f266daea792d6d69` (mode 755) |
| `tests/fixtures/baseline/gadget-task/agent/child.sh` | `fb3114579a37436966261c6a9fb3d6e756fa8d8bcf86ee70c5160c5842c1d017` (mode 755) |
| `tests/fixtures/baseline/gadget-task/scripts/held-out.sh` | content unchanged from B1 (`e6f4a1ac...`), mode 0644 → 755 |

Untouched, verified by digest against b1's ledger: `src/gadget.rs`
(`1b13f8f5...`), `task.yaml` (`cd90a014...`), both role files,
`comparison-contract.yaml`. The fixture tree is exactly the intended 13
files. Every matrix run and diagnosis run wrote only inside scratch
worlds (all removed; /tmp back to 3.5G free). No `git add`/`commit`; no
`git restore`/`git checkout --` anywhere in the round; `git status`
shows the fixture tree exactly as b1 left it (untracked) plus this
round's edits inside it.

## Resume state

None — the round is complete. `cor1-progress.md` checkpoints 1-4 plus
the ledgers above reconstruct the session if anything fails at review.
