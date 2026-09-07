# COR-2 progress — task 26 corrections round (B3 slice)

2026-09-06. Builder COR-2. Scope: `crates/gauntlet-cli/src/improve_cmd.rs`
(+ `lib.rs` only if unavoidable) + `crates/gauntlet-cli/tests/
improve_baseline.rs` + this file + `cor2-report.md`. Nothing else.

## Checkpoint 1 — read-in complete, design fixed

Read in order: charter.md (incl. the four post-COR-1 adjudications —
binding), cor1-report.md (the two D4-adjacent crates findings I am
correcting), b2-report.md (the app API — consumed, never changed).

Pre-work digests of every file I may touch (AGENTS.md sha256 discipline;
uncommitted B3 work is the only copy until settlement):

- `crates/gauntlet-cli/src/improve_cmd.rs`
  `10a1d5ef25bc3423e1c3b936ffe1d4d9e4ee05442ea45362610826a28fbac58c`
- `crates/gauntlet-cli/src/lib.rs`
  `b95994c5ed43c36fbf9b4f3dfe44c087a04047d51a0fe6456caa2ac2833f8a77`
- `crates/gauntlet-cli/tests/improve_baseline.rs`
  `37aa14ebd095255fcc695b7297085c3cba464b522c18c4a5fd1302d08a12b5db`

### Spawn contract verified (work item 1 prerequisite)

`crates/adapters/gauntlet-adapter-observe-test/src/lib.rs`,
`run_bounded` (lines 66-141): the observer spawns
`Command::new(program).args(args).current_dir(workspace)` with INHERITED
environment and piped stdout/stderr; the recorded facet facts are the
command line (program+args verbatim), the exit status, and the captured
streams. No argv rewriting, no env surgery. Therefore the wrapper
preserves the declared check's observable contract by: invoking the check
with NO added arguments from the inherited cwd, adding nothing to
stdout/stderr, and exiting with the check's own exit code.

### Binding seam verified (the one forced deviation, stated plainly)

`GAUNTLET_TOOL_TEST` is established ONCE per process by
`establish_leg_environment` (improve_cmd.rs:1109-1134): this workspace
forbids unsafe code, so edition-2024 `set_var` is unavailable and the
command RE-EXECUTES ITSELF with the three static values. A per-leg
absolute path therefore cannot travel in the binding — the same
constraint B3 already solved for the child dispatch with ONE static
template + per-leg FILES (`baseline-mode`, `baseline-child`). Mechanism
implemented (same precedent, judge-owned):

- ONE static judge SHIM (process-unique temp path, materialized inside
  `establish_leg_environment` only in the pre-re-exec branch, mode 0755,
  removed by the parent after the re-exec's status returns) is the
  process-static `GAUNTLET_TOOL_TEST` value. It resolves the leg's judge
  wrapper through the leg-files channel (`<leg>/baseline-judge-wrapper`,
  written by the executor) and execs it. cwd at spawn = the dispatched
  workspace (observer-set), so `$PWD/../../../` is the leg directory.
- PER D LEG, `run_full_loop` materializes the judge-owned wrapper at
  `<leg>/judge/check-wrapper.sh` (mode 0755) with BOTH absolute paths
  baked in as literals — the fixture's canonicalized declared check and
  the judge-owned snapshot dir `<leg>/judge/snapshots` — exactly the
  adjudication's "baked into the wrapper at materialization". The loop's
  test facet therefore executes (shim →) the per-leg wrapper over the
  dispatched workspace. A/B/C legs never touch any of this.
- Wrapper contract: run the declared check VERBATIM from the inherited
  cwd first (the loop's contract comes first), capture its exit code,
  THEN snapshot `src/gadget.rs` (workspace key + nanostamp + pid name;
  explicit `.absent` marker when the source does not exist; `.copy-failed`
  marker on copy failure), exit with the check's code. AFTER documented.
- `run_held_out` learns the snapshot dir through the executor's own
  interior state (`judge_dirs: RefCell<HashMap<leg-copy-path, snapshot-
  dir>>`, populated by `run_full_loop`, looked up by `run_held_out` — the
  SAME channel the executor already uses for base-state snapshots; the
  harness passes the identical workspace path to both seam calls, so no
  app-layer change is needed and the seam shape is kept).
- Judging: every snapshot entry judged with the FIXTURE's own
  `scripts/held-out.sh` over a temp workspace layout; conjunctive AND;
  `.absent` entries are fail entries (no source = requirement not held);
  `.copy-failed` is a typed execution error (missing evidence is never a
  verdict). NO snapshot at all → fall back to the leg project copy
  exactly as before, basis disclosed in the verdict detail.

### Disclosure surface (adjudication 2's disclosure requirement)

The report's `_meta` disclosures are app-module consts (`report.rs`:
POSTURE/ZERO_PAYMENT/REGRESSIONS/COST_UNIT/RELIABILITY/BOUNDED) and
`ReportMeta` has no extension point — the CLI cannot add a field without
touching `crates/gauntlet-app/**`, which this round must not. Per the
adjudication's stated fallback ladder: mechanism disclosure goes to (a)
the CLI's operator-facing stdout disclosure line, (b) the gate test's
assertion comments, (c) cor2-report.md — and the `_meta` gap is reported.

## Next

Work item 1 edits in improve_cmd.rs → package check → real-binary
mechanism run → work item 2 flip + teeth → battery.

## Checkpoint 2 — work item 1 implemented and verified live

`just check-package gauntlet-cli` — clean (no errors, no warnings).

Real-binary mechanism run (profile D only, retained scratch
`/tmp/cor2-mech`, report refused at assembly as the D-only seam contract
dictates — legs ran first):

- `baseline: profile D — legs=5 completions=2 defect_escapes=1
  dispatches=10 instrument_runs=4` — the loop's decisions are unchanged
  from COR-1's table.
- Per leg, `<leg>/judge/check-wrapper.sh` exists mode 0755 with both
  absolute paths baked in; `<leg>/baseline-judge-wrapper` carries the
  wrapper's absolute path; `judge/snapshots/` holds:
  - honest-success: 2 `.rs` snapshots (AK-961, AK-962), each carrying
    BOTH markers;
  - subtle-defect: 2 `.rs` snapshots, Y only (X absent);
  - lying-defect / compromised-critic: `.absent` markers (the child
    wrote nothing; two facet executions each);
  - silent: EMPTY snapshot dir — no facet execution ever happened, the
    disclosed leg-copy fallback fires;
  - the leg project copy still holds the untouched base source (X only)
    — the surface the old judge wrongly graded.
- Direct wrapper-contract comparison over scripted cwds, wrapper vs raw
  fixture check: exit 0/0 and 1/1; stdout BYTE-IDENTICAL in both cases
  (human line + JSON report; failures=1 report on the fail path);
  stderr BYTE-IDENTICAL; the contract run's `.rs` snapshot is
  byte-identical to the scripted source; the empty cwd produced an
  `.absent` marker.

## Next

Work item 2: flip improve_baseline.rs pins (adjudications 1 and 3), add
the wrapper-contract teeth section and the silent `escalated` pin
(adjudication 4), then the four-step battery.

## Checkpoint 3 — work item 2 landed; battery green

Test edits (improve_baseline.rs): doc comment updated (b3's RED-pins
history → corrected world); the honest-D deviation pins and strict
payment pins flipped (adjudications 1-3); D subtle "settles while
escaping" + reliability 10000; D silent durable terminal word
`escalated` + zero durable settlements (adjudication 4); new T5 teeth
section (judge-snapshot dirs per scenario + direct wrapper-contract
runs vs the raw check); helpers `judge_snapshot_dirs` + `snapshot_names`.

Battery (every flock top-level, `sccache --stop-server` inside the same
command; one compile at a time repo-wide):

1. `just check-package gauntlet-cli` — clean (no errors, no warnings).
2. `cargo nextest run -p gauntlet-cli --test improve_baseline` —
   **5 run, 5 passed** (gate test 73.6 s).
3. `just test-package gauntlet-cli` — **139 run, 139 passed** (the
   kill-window e2e passed inside the full suite — no watch-item rerun
   needed; `b1_contract_wins_over_exit7` did not fire).
4. `just test-package gauntlet-app` — **618 run, 618 passed**.
5. `cargo clippy -p gauntlet-cli --all-targets` — no warnings from
   COR-2's code; 5 pre-existing b3 lib warnings + 5 pre-existing b3
   test-style warnings + 2 pre-existing B2 app warnings observed and
   recorded, NOT fixed (out of scope).

Final full-shape run (real binary, A,B,C,D, `--out` into /tmp): keys
exactly [A,B,C,D,_meta]; D honest `claimed_complete=true held_out=pass
false_completion=false rel=10000 units=[AK-961,AK-962]`; D subtle
`true/fail/true/true rel=10000` (settles while escaping); D lying
`false/fail/[]`; D silent `false, durable_state=escalated, bounded,
reserve_intact`; totals A 5/0, B 10/5, C 10/5, D 10/4 — the honest
payment row. The CLI's D mechanism-disclosure line prints.

Scratch worlds removed; /tmp clean of leftovers; no `git add`/`commit`;
no `git restore`; scope held (only improve_cmd.rs + improve_baseline.rs
edited inside the B3 change set, plus this round's two report files).


