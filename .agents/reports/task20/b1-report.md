# Task 20 / Builder B1 final report — containment & durability seams

B1's implementation was interrupted mid-teeth-proof with one compile error
outstanding. This report covers B1's landed work plus the finisher's
resumption (compile fix, teeth red→green proof, gate battery, and the
B1-scope gate fixes the battery surfaced). Builder claims carry no
settlement authority; all evidence below is test output under
`.agents/reports/task20/finish-*.log`.

## File inventory (B1 slice)

Landed by B1 before interruption:

- `crates/adapters/gauntlet-adapter-workspace-fs/` — NEW crate
  (`Cargo.toml`, `src/lib.rs`, `src/provider.rs`, `tests/contract_suite.rs`);
  `FsWorkspaceProvider` (create/cleanup/hooks), containment-checked,
  typed `WorkspaceProviderError`, runs the shared `workspace_provider_suite`.
  Registered in the root `Cargo.toml` (members + workspace.dependencies).
- `crates/gauntlet-app/src/govern/gate.rs` — `GateFacts` gains `workspace:
  WorkspaceRef` (per-dispatch fact); `GovernDeps` loses its lifetime
  workspace field; `admit()` anchors policy resolution at `facts.workspace`;
  new test `the_gate_anchors_policy_resolution_at_the_dispatch_workspace`.
- `crates/gauntlet-app/src/control/controller.rs` — `ControllerDeps.workspaces:
  Option<Arc<dyn WorkspaceProvider>>`; `dispatch()` allocates a FRESH
  workspace per dispatched unit BEFORE admission (typed
  `ControlError::Workspace` on provider refusal; run-derived ref only when
  NO provider is composed); observation targets the flight's own spec
  workspace; `release_flight`/`release_workspace` wired at all 11
  invocation-terminalization sites (best-effort, pinned
  `workspace_cleanup_failed` record on failure, never reordering §16
  settlement); teeth test
  `two_units_of_one_run_are_admitted_into_distinct_workspaces` +
  `without_a_provider_the_dispatch_falls_back_to_the_run_derived_ref`.
- `crates/gauntlet-app/src/roles/install.rs` — item 3 (R17-note-1 / D10):
  `write_atomic` gained the parent-directory fsync after the rename, with
  the mutation tooth `write_atomic_fsyncs_the_directory_entry`. (This was
  landed by B1 but never recorded in the progress file; verified in-tree by
  the finisher.)
- 8 `crates/gauntlet-cli/tests/*.rs` files — mechanical `workspaces: None`
  in every `ControllerDeps` literal + `none_workspace()` → `none_backend()`:
  g10, g7, p4, p5, p6, p7, p11 (p5 has two literals).

Added by the finisher to complete the slice:

- `crates/gauntlet-cli/tests/p5_hard_gate.rs:701` — the one remaining
  compile error: `workspaces: None,` inserted after `govern,` (mirrors the
  sibling pattern). No assertion touched.
- Battery-driven B1-scope gate fixes (all in B1's own files; no shared gate
  file, no assertion, and no spec/ruler file touched):
  - controller.rs teeth fixture `Workspace.root`: `std::env::temp_dir()`
    instead of hardcoded `"/tmp/ws-fixture-{n}"` (hardcoded-literal gate).
  - controller.rs `release_workspace`: removed clippy `needless_return`
    tail `return;` (control flow identical).
  - install.rs: the tooth's `#[cfg(test)]` static + gated statement
    reshaped into cfg-disjoint twin MODULES (`directory_entry_fsync_sensor`,
    production no-op / test counting sensor) — the single-writer gate's
    blanker classifies gated modules/functions only (I8, fail-closed);
    tooth assertions unchanged (1 per write, 2 after the second).
  - install.rs: directory fsync open switched from `OpenOptions` to
    `std::fs::File::open(parent)` — the same read-only open, and the
    gate's documented blessed spelling for read-only opens; consistent
    with the file-fsync half above it. (A first attempt kept the token in
    a comment; the scanner reads comments too — rephrased.)
  - rustfmt applied to controller.rs / gate.rs (B1's files only; B2's
    in-flight report files deliberately NOT reformatted).

Not B1's (seen in the tree, untouched): `crates/gauntlet-app/src/report/`,
`crates/gauntlet-app/tests/report.rs`, `pub mod report;` in
`crates/gauntlet-app/src/lib.rs` — B2's in-flight slice.

## Test counts

- Adapter crate: 6/6 (`just test-package gauntlet-adapter-workspace-fs`,
  contract suite green).
- New app tests: 2 controller teeth tests + 1 gate policy-anchor test +
  1 write_atomic mutation tooth.
- Full workspace (`just test`): **1303 tests run: 1303 passed, 0 skipped**.
- `just boundaries`: 228/228. `just ruler`: green. `just check`: green.

## Teeth red→green proof (R-1b)

Test: `two_units_of_one_run_are_admitted_into_distinct_workspaces`
(controller.rs tests). Logs: `finish-teeth-green.log`, `finish-teeth-red.log`,
`finish-teeth-green2.log`.

1. GREEN (mechanism wired): `2 tests run: 2 passed, 456 skipped`, EXIT=0.
2. RED (minimal bypass: dispatch's provider match made both arms return
   the run-derived ref, provider never consulted): `1 test run: 0 passed,
   1 failed`, EXIT=100 — panicked at the distinctness assertion:
   `got {"01ARZ3NDEKTSV4RRFFQ69G5FAV"} — a shared ref is the R-1b defect
   this tooth exists to refuse`.
3. Restore byte-identical: sha256 before bypass AND after restore =
   `e93a67a753c7c866600779da081c8cfa5dc728adc3e4bba0fe5d418e73e4f6a8`.
   Tree never left reverted beyond the single red run.
4. GREEN again: `2 tests run: 2 passed, 456 skipped`, EXIT=0.

Note: controller.rs's hash today (`ea565d9e…`) differs from the proof-time
snapshot because the battery-driven B1-scope fixes and rustfmt landed
AFTER the proof; the teeth tests were re-run green in the final workspace
battery after those changes.

## Gate battery (final, in required order)

- `just check-package gauntlet-adapter-workspace-fs` = 0
- `just test-package gauntlet-adapter-workspace-fs` = 0 (6/6)
- `just check` = 0
- `just test` = 0 (1303/1303)
- `just lint` = 101 — see deviations; no B1 file implicated
- `just boundaries` = 0 (228/228)
- `just ruler` = 0

## Deviations / honest findings

1. `just lint` is red, entirely from B2's IN-FLIGHT, untracked report
   slice: clippy `redundant_closure` at
   `crates/gauntlet-app/tests/report.rs:213` plus fmt diffs in
   `src/report/mod.rs` / `tests/report.rs`. Zero clippy or fmt findings in
   any B1 file. Out of B1 scope; not fixed; left for B2/orchestrator.
2. `gauntlet-adapter-agent-cli` falsifier test
   `b1_contract_wins_over_exit7_and_process_is_never_consulted` failed ONCE
   under full-workspace load (`expected Completed, got MalformedResult`),
   passed in battery-1's ordering and in isolation, and passed in the final
   full workspace run. Load-flaky live-process test; not a B1 file; not
   fixed; reported for the orchestrator's attention.
3. The brief's item-2 mechanical-update list said "7" CLI test files; p5
   contains TWO ControllerDeps literals, both now fixed.
4. Environment: a sccache server forked from inside a
   `flock compile.lock` session inherits the locked fd and holds the lock
   after the wrapper exits (observed twice; cleared both times with
   `sccache --stop-server`). Suggests an AGENTS.md failure-log entry.

## Deliberately not done

- No edits outside B1's slice: no `.wayfinder/**`, `.gauntlet/**`,
  `.agents/specs/**`, `crates/gauntlet-cli/src/**`, no shared gate file
  (all gate reds were resolved inside B1's own files), no dependency
  additions, no assertion changes, no commit (orchestrator owns commits).
- B2's report slice left exactly as found.

---

## Addendum — second finisher pass (2026-08-31 ~15:05)

A second finisher was dispatched under the stale belief the first had died
before starting; it found the first finisher's battery-4 still running,
stood off until the battery shell exited, then closed the genuinely
remaining gaps. Progress-file trail: end of `b1-progress.md`. Coordinator
was alerted mid-flight.

1. **write_atomic mutation tooth — full three-state proof (was GREEN-only).**
   Test `write_atomic_fsyncs_the_directory_entry` (install.rs). Logs:
   `finish-tooth2-{green1,red,green2}.log`.
   - GREEN: `1 test run: 1 passed, 457 skipped`, EXIT=0.
   - BYPASS: parent-directory fsync half + its sensor `record()` removed
     (the pre-R17-note-1 commit path).
   - RED: `1 test run: 0 passed, 1 failed`, EXIT=100 — `left: 0, right: 1`
     at the tooth's "exactly one directory-entry fsync per committed write"
     assertion (install.rs:1241), exactly as predicted.
   - RESTORE byte-identical: sha256 before bypass and after restore =
     `c0002ef0e6a3dc9a91bc1ebafd721a25ce96fc5925629272f2d429c3c6496fd9`
     (also `diff -q`-identical to a pre-bypass backup).
   - GREEN again: `1 test run: 1 passed, 457 skipped`, EXIT=0.
2. **check-package / test-package / `just check` re-run on the FINAL tree**
   (the body of this report had reasoned them green without re-running after
   the last edits): `finish-battery5-rest.log` — check-package = 0,
   test-package = 0 (6/6), `just check` = 0. The battery-4 results for
   `just test` / `just lint` / `just boundaries` / `just ruler` carry over:
   install.rs is byte-identical to what battery-4 tested and controller.rs
   was not touched in this pass (current sha256 `ea565d9e…` matches the
   hash this report already records).
3. **Lint, current tree, precisely:** battery-4's clippy emitted exactly
   ONE error, `redundant_closure` at `crates/gauntlet-app/tests/report.rs:213`
   (B2's in-flight file). No clippy or fmt finding in any B1 file.
4. **Coordination note for the orchestrator:** two finisher passes were
   briefly concurrent; no source collision occurred (the second pass made
   no source edits at all — only the temporary, restored, hashed tooth
   bypass inside install.rs). Prior pass's progress entries and logs were
   verified against the actual logs and adopted, not redone.

Nothing else in this report changed.
