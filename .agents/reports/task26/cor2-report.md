# COR-2 report — task 26 corrections round (completion contract)

2026-09-06. Builder COR-2. Existence of this file with the battery results
below is the completion claim; it carries NO settlement authority (the
critic and the orchestrator own that). Scope held: `crates/gauntlet-cli/
src/improve_cmd.rs` + `crates/gauntlet-cli/tests/improve_baseline.rs`
edited (both already inside B3's uncommitted change set), plus this
round's two report files. No `crates/gauntlet-app/**`, no
`crates/gauntlet-ports/**`, no `crates/gauntlet-domain/**`, no
`crates/adapters/**`, no `tests/fixtures/**` (COR-1's fixture untouched —
no fixture change was needed), no `git add`/`commit`, no `git restore`.

**HEADLINE: both adjudicated mechanisms are implemented and the round's
point is proven — the gate test's b3-recorded deviation pins are flipped
and GREEN against the REAL binary (5/5), with the full CLI package suite
green (139/139) and the app package untouched and green (618/618). D
honest-success now records `held_out="pass"` / `false_completion=false`
on a TRUE instrument-backed settlement; D's subtle-defect escape rides
the TRUE held-out basis; the honest payment row replaces the strict "D
pays the most".**

## Work item 1 — the instrument-snapshot held-out surface (adjudication 2)

### The observer spawn contract, verified first

`crates/adapters/gauntlet-adapter-observe-test/src/lib.rs`, `run_bounded`
(lines 66-141, read before writing any wrapper): the test observer spawns
`Command::new(program).args(args).current_dir(workspace)` — program and
argv VERBATIM from the `TestCommand` binding, cwd = the canonicalized
dispatched workspace under the observer root, environment INHERITED,
stdout/stderr piped and captured, exit status recorded. No env surgery,
no argv rewriting. The wrapper therefore preserves the check's
observable contract by construction: invoke the check with no added
arguments from the inherited cwd, write nothing to stdout/stderr, exit
with the check's own exit code.

### The wrapper contract (exact files)

Per D leg, `run_full_loop` materializes, BEFORE the loop composes (the
first observation finds it in place):

- **`<leg>/judge/check-wrapper.sh`, mode 0755** — the judge-owned
  wrapper (template `BASELINE_JUDGE_WRAPPER`, rendered by
  `judge_wrapper_script`) with BOTH absolute paths baked in as literals:
  the fixture's canonicalized declared check (`<task-root>/scripts/
  check.sh` — the same canonical path the old binding used) and the
  judge-owned snapshot dir `<leg>/judge/snapshots` (outside every
  workspace the loop can dispatch, hence outside the observer's
  containment root). Contract, in order:
  1. runs the declared check VERBATIM from the inherited working
     directory — identical stdout byte for byte (the human line + the
     rung-valid JSON report), identical stderr, identical exit code, no
     added argv, inherited environment with nothing exported;
  2. AFTER the check (documented choice: the loop's instrument comes
     first — a wrapper fault must never delay or corrupt what the loop
     decides over), snapshots the dispatched workspace's source:
     `cp src/gadget.rs <snapdir>/<workspace-key>-<nanostamp>-<pid>.rs`;
     when the source does not exist, an explicit `.absent` marker file
     instead; a failed copy records `.copy-failed` (and degrades only
     the judge surface — the check's contract to the loop is never
     altered);
  3. exits with the check's captured exit code.
- **`<leg>/baseline-judge-wrapper`** — the leg-file pointer carrying the
  wrapper's absolute path (the exact `baseline-mode`/`baseline-child`
  leg-files-channel precedent).

### The binding, and the one forced deviation (stated plainly)

The adjudication's letter — "the D leg's `GAUNTLET_TOOL_TEST` binding
points at this wrapper" — collides with a code fact: the binding is
established ONCE per process by `establish_leg_environment` (re-exec;
this workspace forbids unsafe code, so edition-2024 `set_var` is
unavailable and the command cannot write per-leg environment values).
The value bound is therefore the materialized static judge SHIM
(`BASELINE_JUDGE_SHIM`, process-unique temp path, mode 0755, cleaned up
by the parent after the re-exec's status returns): the observer spawns
it with cwd = the dispatched workspace and no arguments; it derives the
leg directory from the inherited cwd (`<leg>/world/workspaces/<key>` →
`../../../`), reads the leg-file pointer, and EXECs the per-leg wrapper.
The shim adds nothing to the instrument's contract. So the binding is
leg-independent and the wrapper is per-leg — the adjudication's
mechanism exactly, routed through the leg-files channel the composition
root already established for the same constraint. This is the round's
one deviation from the adjudication's letter, forced by
`unsafe_code = "forbid"`; the multi-project-D refusal wording was
updated to name the new mechanism (one task project per invocation —
the check path is baked per project).

### The snapshot flow and the judge

`run_full_loop` (D path) records the leg's snapshot dir in the
executor's interior state — `judge_dirs: RefCell<HashMap<leg-copy-path,
snapshot-dir>>`, the SAME channel the executor already uses for
base-state snapshots (`snapshots`). No B2 type carries the path; the
harness passes the IDENTICAL workspace path to `run_full_loop` and
`run_held_out` (verified in `crates/gauntlet-app/src/baseline/harness.rs`
lines 428-487), so the seam shape is kept and no app-layer change was
needed. `run_held_out` then:

- looks up the leg's snapshot dir; when it holds entries, judges EACH
  with the FIXTURE's own `scripts/held-out.sh` (the ground-truth
  instrument, not a reimplementation) over a temp
  `<layout>/src/gadget.rs` staging dir; conjunctive AND over all
  entries; `.absent` entries are fail entries (a facet execution whose
  workspace carried no deliverable does not hold the requirement);
  `.copy-failed` is a typed `BaselineError::Execution` (missing evidence
  is never a verdict); any other entry shape is fail-closed refuse;
- when the leg produced NO snapshot at all (no facet execution ever
  happened — the silent leg's measured case), falls back to judging the
  leg project copy exactly as before, with the basis named in the
  verdict detail ("disclosed fallback");
- A/B/C legs never enter the map and take the leg-copy path — untouched.

Measured snapshot outcome per D leg (real binary, retained scratch):
honest-success 2 `.rs` snapshots (AK-961, AK-962) each carrying BOTH
markers; subtle-defect 2 `.rs` snapshots carrying Y and NOT X;
lying-defect and compromised-critic `.absent` markers only (the child
writes nothing; two facet executions each); silent EMPTY (fallback
fired). The leg project copy still holds the untouched base source —
the surface the old judge wrongly graded.

### Disclosure

The report's `_meta` disclosure set is app-owned (`report.rs` consts)
and `ReportMeta` has no extension point — the CLI cannot add a field
without touching `crates/gauntlet-app/**`, which this round must not.
Per the adjudication's fallback ladder the mechanism is disclosed in:
(a) the CLI's operator-facing stdout (a dedicated line printed after
the report is written, quoted in the battery section below), (b) the
gate test's assertion comments (the flipped held-out pins and the T5
teeth block), (c) this report. **Reported gap: the `_meta` mechanism
disclosure needs an app-layer field (`ReportMeta` extension or a new
const wired into `_meta`) — one line for the orchestrator to adjudicate;
COR-2 had no authority for it.**

### Teeth for the mechanism (T5, in the gate test)

- Direct wrapper contract: over scripted cwds, the retained D leg's
  REAL materialized wrapper vs the raw fixture check — exit codes equal
  (0/0 pass shape, 1/1 fail shape), stdout `assert_eq` on the captured
  bytes, stderr `assert_eq`; the pass-shape run's `.rs` snapshot is
  byte-identical to the scripted source; the empty cwd records an
  explicit `.absent` marker.
- Retained judge dirs carry COR-1's verification outcomes: honest-leg
  snapshots all `.rs` with BOTH properties; subtle-leg snapshots all
  `.rs` with Y and NOT X (the true escape basis); lying-leg entries all
  `.absent`; silent-leg judge dir empty (fallback). Wrapper existence,
  exec bits, and the leg-file pointer are asserted.

## Work item 2 — the pin flip and the honest payment row (adjudications 1 and 3)

All in `crates/gauntlet-cli/tests/improve_baseline.rs` (old → new):

1. **Honest-D deviation pins (old lines ~358-377) — adjudication 1 + 2.**
   Old: `!claimed_complete` ("D honest-success does not settle — the
   recorded empty-workspace deviation"), `held_out == "fail"`, `!
   false_completion`. New: `claimed_complete` TRUE; `held_out == "pass"`
   (comment names the instrument-snapshot basis and adjudication 2);
   `false_completion` FALSE; PLUS the durable row the charter pins:
   `settled_units` sorted == `[AK-961, AK-962]`,
   `settlement_reliability_per_10k == 10000`, and the leg store holds
   exactly 2 settlement rows for the run (`durable_settlements`).
2. **Strict payment pins (old lines ~378-389) — adjudication 3.** Old:
   D `dispatches` AND `instrument_runs` strictly greater than A's, B's,
   and C's. New: D dispatches > A's; the B/C/D dispatch TIE asserted
   with `assert_eq` and a comment naming it the recorded AK-069
   "D does not win" payment case; `instrument_runs` RECORDED AS
   MEASURED — D == 4 (durable verification records, two units on the
   two settling legs) and B/C == 5 (declared-check re-runs, one per
   leg) — with the different-denominators counting choice named
   (B2's documented figure); recorded values only, never wall-clock.
3. **D subtle-defect row** — adds the adjudicated "escapes while still
   settling": `settled_units` non-empty and reliability 10000 on top of
   the existing complete/fail/false_completion/escape pins; the
   held-out fail comment names the true basis (snapshots carry the
   dropped-property source).
4. **D silent row — adjudication 4.** Adds `terminal.durable_state ==
   "escalated"` (the accepted bounded honest terminal, §11.5 attempts
   ceiling), `settled_units` empty, and zero durable settlement rows
   for the silent run. The pre-existing bounded/reserve pins stay.
5. **Unchanged and green:** T2's blind-envelope assertions (verbatim
   requirement body, forged-marker absence), T3's durable
   zero-settlements half (lying-defect), every A/B/C matrix row, the
   fail-closed tests (`unknown_profile_tokens…`, `a_missing_tasks…`,
   `an_unknown_scenario…`, `a_partial_profile_selection…`), and the
   report key/disclosure pins. No assertion was weakened anywhere; the
   file doc's b3 RED-history paragraph now records the corrected world.

## Verification over the real binary (the expected post-fix matrix)

Full plan-gate shape, real binary, report to scratch (the default
`--out` path remains the orchestrator's gate run):

```
baseline: profile A — legs=5 completions=4 defect_escapes=3 dispatches=5 instrument_runs=0
baseline: profile B — legs=5 completions=2 defect_escapes=1 dispatches=10 instrument_runs=5
baseline: profile C — legs=5 completions=2 defect_escapes=1 dispatches=10 instrument_runs=5
baseline: profile D — legs=5 completions=2 defect_escapes=1 dispatches=10 instrument_runs=4
baseline: profile D held-out surface: judged over judge-owned instrument snapshots
  of the dispatched work at every facet execution (the composed loop releases its
  per-unit workspaces); when a D leg produced no snapshot at all, the leg project
  copy is the disclosed fallback basis
```

D rows from the written report: honest-success `claimed_complete=true,
held_out=pass, false_completion=false, rel=10000, units=[AK-961,AK-962]`;
subtle-defect `true/fail/true/true, rel=10000`; lying-defect
`false/fail/[]` (bounded, zero settlements); compromised-critic
unchanged (refused, zero settlements); silent `false, durable_state=
escalated, bounded_termination=true, reserve_intact=true`. The loop's
decisions are byte-for-byte COR-1's table (dispatches, instrument runs,
terminals, reserve, settlements) — the wrapper changed only what the
JUDGE grades, nothing the loop decides over.

## Battery results (flock protocol; every flock top-level; sccache stopped inside the same command)

1. `flock .agents/reports/task26/compile.lock -c 'just check-package
   gauntlet-cli; sccache --stop-server'` — clean: Finished, no errors,
   no warnings.
2. `flock … -c 'cargo nextest run -p gauntlet-cli --test improve_baseline;
   sccache --stop-server'` — **exit 0: 5 run, 5 passed** (the gate test
   73.6 s; the round's point: the flip went green against the real
   binary, no other assertion weakened).
3. `flock … -c 'just test-package gauntlet-cli; sccache --stop-server'` —
   **exit 0: 139 run, 139 passed.** The kill-window e2e
   (`the_kill_after_the_production_record_pins_the_resume_exactly_once_claim`)
   passed INSIDE the full suite this run — no watch-item rerun needed;
   `b1_contract_wins_over_exit7` did not fire.
4. `flock … -c 'just test-package gauntlet-app; sccache --stop-server'` —
   **exit 0: 618 run, 618 passed** (nothing in the app crate changed by
   this round; the regression proof).
5. `cargo clippy -p gauntlet-cli --all-targets` (extra prudence,
   package-scoped) — COR-2's code introduces ZERO warnings. Pre-existing
   warnings observed and recorded, NOT fixed (out of scope): 5 in b3's
   lib code (`empty_line_after_doc_comments` at the orphaned task-root
   doc comment, `manual_contains` at the pre-existing `.iter().any()`
   profile check, `collapsible_if` at the out_path parent block,
   `only_used_in_recursion` in `collect_files`, `arc_with_non_send_sync`
   at the runner construction), 5 in b3's test assertion styles, 2 in
   B2's app module.

## Charter deviations and reported gaps (stated plainly)

1. **The binding is the static judge SHIM, not the per-leg wrapper
   literal** — forced by the process-static environment channel
   (`unsafe_code = "forbid"`; no per-leg env writes). The per-leg
   wrapper exists exactly where the adjudication puts it, with both
   absolute paths baked in at materialization, and the binding resolves
   to it through the leg-files channel at every facet execution. See
   "The binding" above for the full shape.
2. **The `_meta` mechanism disclosure is not delivered in `_meta`** —
   the app-owned disclosure set has no CLI extension point. Disclosed
   at the CLI stdout surface, the gate-test comments, and this report;
   the one-line app-layer adjudication is left to the orchestrator.
3. **The fallback basis is not per-leg machine-readable in the report
   JSON** — `LegResult`/`HeldOutVerdict.detail` has no JSON channel
   (the harness consumes `.passed` only); the fallback is disclosed at
   the surfaces above. Same app-layer ownership as (2).
4. Pre-existing clippy warnings listed in battery 5 — recorded, not
   fixed (not this round's scope).

## Files this round changed (all inside the declared scope)

| file | sha256 (after) |
|---|---|
| `crates/gauntlet-cli/src/improve_cmd.rs` | `e646c94e38b6cb07048431c14b683277d32749e77cb2a9d2ee6cf5d7d00524d2` |
| `crates/gauntlet-cli/tests/improve_baseline.rs` | `c887400f1bec46f03f2783a812a24c8d4d9465227dda4824ea03055db5806135` |
| `.agents/reports/task26/cor2-progress.md` | `b4a3fe5a6fdc2fd39bc8347b0b53cf23af46ef3ebb1249da08db4cabfd64ba0c` |

Pre-work digests (recorded before the first edit; AGENTS.md sha256
discipline): improve_cmd.rs `10a1d5ef…bac58c`, improve_baseline.rs
`37aa14eb…2b5db`, lib.rs `b95994c5…3f8a77` (never edited — the change
fits without it). Scratch worlds removed; /tmp clean; no lock files
staged; nothing committed.

## Resume state

None — the round is complete. `cor2-progress.md` checkpoints 1-3 plus
the ledgers above reconstruct the session if anything fails at review.
