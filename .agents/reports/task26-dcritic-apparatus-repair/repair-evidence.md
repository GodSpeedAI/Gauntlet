# Task 26 D+Critic Apparatus Repair — Evidence

RED → GREEN evidence for every production change, and R1–R7 apparatus
proof. All reruns are against SCRATCH copies of fixtures; nothing
authoritative was overwritten. Commands are exact and reproducible.

Base HEAD for this repair: `e4ca39358c7fbaafce7a5b202672fe332cfd5add`.
Platform: Linux 6.18.35.2-microsoft-standard-WSL2 x86_64. rustc/cargo
1.98.0.

---

## B1 — refusals reproduced before the fixture existed

```
$ ./target/debug/gauntlet improve baseline --profiles D \
    --tasks <scratch-copy-of-gadget-task-with-only-max_critics:1-changed>
gauntlet improve baseline: the baseline run failed to execute at
run_full_loop: scenario honest-success: ... the governed comparison
chain refused: the governed review failed: a dispatched comparator
failed: the comparison envelope is invalid: a model critic requires
process-or-stronger runner isolation; isolation:none cannot establish
fresh-context containment (spec 13.2)

$ # same, plus sandbox.backend: bwrap, no policy artifact
gauntlet improve baseline: the baseline run failed to execute at
run_full_loop(compose): the composition-time wrap was refused: the
contained tier policy could not be resolved for unit AK-961: no sandbox
policy exists for the contained tier — for irreversible that is the
requirement (spec 17.3); for any other tier the policy artifacts are
incomplete
```

Both reproduced verbatim during the independent verification pass and
again during this repair before `tests/fixtures/baseline-dcritic/
gadget-task/.gauntlet/policies/contained.json` was added.

---

## B3a — RED: durable admission with no terminal fact

Command (scratch, `GAUNTLET_BASELINE_SCRATCH_ROOT` retains the leg's
state store for inspection):

```
$ GAUNTLET_BASELINE_SCRATCH_ROOT=<scratch> ./target/debug/gauntlet \
    improve baseline --profiles A,B,C,D --tasks <scratch-tasks-dir> \
    --out <scratch>/result.json
```

Report (before B3a/B3b, D leg, honest-success, single scenario):

```json
"verification_burden": { "instrument_runs": 2, "review_dispatches": 0 }
```

Raw `event_log` from the leg's own state store (before the fix):

```
1  checkpoint_written    {"iteration_no":1,...}
2  invocation_admitted   {"invocation_id":"KR4H16...(builder)"}
3  unit_dispatch         {"unit_id":"AK-961","invocation_id":"KR4H16..."}
4  invocation_admitted   {"invocation_id":"MD1GP5...(critic)"}
5  evidence_produced     {...}
6  settlement_committed  {"unit_id":"AK-961",...}
7  checkpoint_written    {...}
8  invocation_admitted   {"invocation_id":"1HEPS0...(builder)"}
9  unit_dispatch         {"unit_id":"AK-962","invocation_id":"1HEPS0..."}
10 invocation_admitted   {"invocation_id":"QDE1PV...(critic)"}
11 evidence_produced     {...}
12 settlement_committed  {"unit_id":"AK-962",...}
13 checkpoint_written    {...}
14 run_terminal          {"state":"settled"}
```

`records` table: `role_invocation: 4` (2 builder + 2 critic, both
"admitted"), `comparison: 2` (both critic comparisons accepted),
`verification_record: 2`. **No `invocation_terminal` event for EITHER
critic invocation, despite both being admitted and their comparisons
accepted.**

## B3a — GREEN: after the fix

Same command, same fixture, rebuilt binary:

```
14 rows total, including:
5  invocation_terminal  {"invocation_id":"MD1GP5...","state":"completed"}
12 invocation_terminal  {"invocation_id":"QDE1PV...","state":"completed"}
```

(full log, GREEN):

```
checkpoint_written
invocation_admitted   (builder, AK-961)
unit_dispatch          AK-961
invocation_admitted   (critic)
invocation_terminal   (critic, completed)     <- NEW
evidence_produced
settlement_committed   AK-961
checkpoint_written
invocation_admitted   (builder, AK-962)
unit_dispatch          AK-962
invocation_admitted   (critic)
invocation_terminal   (critic, completed)     <- NEW
evidence_produced
settlement_committed   AK-962
checkpoint_written
run_terminal (settled)
```

Focused test, RED→GREEN:

```
$ cargo test -p gauntlet-cli --test p2_context_isolation durable_critic_invocation
# before B3a's fix: FAILED — assertion on run_lifecycle_facts()
#   .invocation_terminals finds no Completed entry
# after: 
test durable_critic_invocation_and_accepted_document_survive_reopen_and_reject_crosswire ... ok
```

## B3b — RED: size-subtraction over disjoint sets

After B3a alone, the SAME report still read `review_dispatches: 0` for
the leg above (2 accepted critic comparisons, both now durably
terminal). Root cause traced to `derive_loop_metrics`
(`gauntlet-app/src/baseline/metrics.rs`): `all_invocations.len() -
dispatched_invocations.len()`, where (in the real runtime)
`all_invocations = {critic1, critic2}` and `dispatched_invocations =
{builder1, builder2}` are DISJOINT — builders never get their own
terminal event. `2 - 2 = 0`.

New focused unit test proves this, RED at the size-subtraction formula:

```
$ cargo test -p gauntlet-app --lib \
    review_dispatches_counts_a_disjoint_terminal_set_not_a_size_difference
# with the old formula (`all.len().saturating_sub(dispatched.len())`):
#   left: `0`, right: `1` — assertion failed
# with the set-difference formula:
test baseline::metrics::tests::review_dispatches_counts_a_disjoint_terminal_set_not_a_size_difference ... ok
```

The pre-existing `derive_loop_metrics_reads_the_durable_record` test
(superset-shape fixture) is UNCHANGED and still green under the new
formula — confirmed:

```
$ cargo test -p gauntlet-app --lib baseline::metrics
running 16 tests
...
test baseline::metrics::tests::derive_loop_metrics_reads_the_durable_record ... ok
test baseline::metrics::tests::review_dispatches_counts_a_disjoint_terminal_set_not_a_size_difference ... ok
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 545 filtered out
```

## B3a+B3b combined — final GREEN, end-to-end

```
$ ./target/debug/gauntlet improve baseline --profiles A,B,C,D \
    --tasks <scratch-tasks-dir> --out <scratch>/result.json
baseline: profile A — legs=1 completions=1 defect_escapes=0 dispatches=1 instrument_runs=0
baseline: profile B — legs=1 completions=1 defect_escapes=0 dispatches=2 instrument_runs=1
baseline: profile C — legs=1 completions=0 defect_escapes=0 dispatches=2 instrument_runs=1
baseline: profile D — legs=1 completions=1 defect_escapes=0 dispatches=2 instrument_runs=2
```

`result.json` D leg:

```json
{
  "scenario": "honest-success",
  "claimed_complete": true,
  "held_out": "pass",
  "false_completion": false,
  "defect_escape": false,
  "dispatches": 2,
  "verification_burden": { "instrument_runs": 2, "review_dispatches": 2 },
  "settlement_reliability_per_10k": 10000,
  "terminal": { "classification": "settled", "durable_state": "settled" },
  "settled_units": ["AK-961", "AK-962"],
  "verification_records": 2,
  "deterministic_verified_settled": 2
}
```

`review_dispatches: 2` — one per unit, matching `dispatches: 2` exactly,
now durably truthful.

---

## Change 1 — comparator polling sleep

```
$ cargo test -p gauntlet-adapter-compare-model the_probe_interval
# with POLL_INTERVAL_MS = 0 (RED, reverted temporarily, then restored):
thread '...the_probe_interval_gives_a_real_child_wall_clock_to_finish' panicked:
drive_to_terminal_with must actually wait between non-terminal polls
(elapsed 489.221µs — a 0ms interval regression would complete this in
microseconds against a scripted runner that answers instantly)
test result: FAILED. 0 passed; 1 failed

# with POLL_INTERVAL_MS = 10 (committed, GREEN):
test tests::the_probe_interval_gives_a_real_child_wall_clock_to_finish ... ok
```

Full adapter suite after the fix:

```
$ cargo test -p gauntlet-adapter-compare-model
running 9 tests
... (all 9) ... ok
test result: ok. 9 passed; 0 failed
```

---

## Change 2 — `workspaces/workspaces` — confirmed NOT a defect

No RED/GREEN needed — no code change was made to this path (it was
already correct in the working tree). Confirming evidence instead:

```
$ grep -n WORKSPACES_DIR_NAME crates/gauntlet-cli/src/compose.rs
compose.rs:813:  workspace_root: state_dir.join(defaults::WORKSPACES_DIR_NAME)
compose.rs:1133: workspace: WorkspaceRef::new(defaults::WORKSPACES_DIR_NAME)...
```

`AnchorRoots.workspace_root` = `<state>/workspaces`; the composition-time
dry-run wrap's synthetic `WorkspaceRef` is literally `"workspaces"`; the
policy resolver's `$WORKSPACE_ROOT` scopes to
`workspace_root.join(<that ref>)` = `<state>/workspaces/workspaces` —
two independently-composed, correctly-named segments. Confirmed against
an EXISTING task-25 precedent that manually pre-creates this same path:

```
$ grep -n "workspaces.*workspaces" crates/gauntlet-cli/tests/e2e_full_loop_real_runner.rs
    std::fs::create_dir_all(self.state_dir().join("workspaces").join("workspaces"))
        .expect("the wrap-time workspace directory");
```

Every apparatus smoke run in this evidence file succeeded WITHOUT this
repair pre-creating that directory anywhere in its own harness — the
production `ensure_configured_dir` call in `compose_run` (the committed
working-tree change) is what creates it. The RED shape this fix
addresses (`WrapRefused`, "no sandbox policy exists...") was
independently reproduced during the earlier verification pass over the
identical composition path, before this line existed.

---

## Change 3 — `RecordingAgentRunner` wraps the critic runner

Confirmed wrapped (compose.rs), confirmed recording (durable
`role_invocation` rows above, `role: "critic"`,
`permissions: "comparison-envelope-only"`, `sandbox_policy_ref: null`),
confirmed unchanged builder semantics (all 1702 workspace tests pass,
including every pre-existing controller/builder-dispatch test),
confirmed the metric sees it (B3a+B3b GREEN above).

---

## R1/R2/R5/R6 — configuration admission, sandbox enforcement, policy resolution

```
# R5 (still refuses, negative control unchanged):
$ sandbox.backend: none, max_critics: 1  ->  EnvelopeInvalid (spec 13.2), as before

# R1/R2/R6 (new fixture, committed):
$ ./target/debug/gauntlet improve baseline --profiles A,B,C,D \
    --tasks <scratch copy of tests/fixtures/baseline-dcritic> \
    --out <scratch>/result.json
baseline: profile D — legs=1 completions=1 defect_escapes=0 dispatches=2 instrument_runs=2
```
Exit 0. Configuration resolves, sandbox policy resolves (bwrap +
`contained.json`), workspace exists, critic runner admits, critic
process launches (durable `role_invocation` row, `runner: "cli-agent"`,
`session_ref: "cli-child:<pid>"`), critic invocation is durably recorded
(B3a), envelope is inspectable (R4 below).

---

## R4 — context isolation

Retained, not reinvented: the existing
`crates/gauntlet-cli/tests/p2_context_isolation.rs::
the_envelope_is_structurally_blind_and_carries_instrument_reality` test
asserts over the SERIALIZED envelope (§13.2 allowlist) — full file
green:

```
$ cargo nextest run -p gauntlet-cli --test p2_context_isolation
13 tests run: 13 passed, 0 skipped
```

Additionally confirmed structurally via the new fixture's own critic
branch: the child process reads ONLY `ENVELOPE-PATH`'s JSON (`run_id`,
`iteration_no`, `reference_digest`, `observation.observation_digest`,
`contract.version`) and `ENVELOPE-DIGEST`; it has no access to, and
never reads, the builder's own result document, prompt, or workspace
outside the dispatched project copy.

---

## R7 — reproducibility

Two runs from clean scratch state, same committed fixture, same task
selection:

```
$ ./target/debug/gauntlet improve baseline --profiles A,B,C,D \
    --tasks <scratch1> --out <scratch1>/result.json
$ ./target/debug/gauntlet improve baseline --profiles A,B,C,D \
    --tasks <scratch2> --out <scratch2>/result.json
```

Nonvolatile fields (scenario, claimed_complete, held_out,
false_completion, defect_escape, dispatches, instrument_runs,
review_dispatches, terminal.durable_state, settled_units,
verification_records) compared programmatically across all four
profiles: **identical** in both runs. Fixture digest identical in both
runs' `_meta.fixture_digests`.

---

## Full battery (this repair's final state)

```
$ cargo fmt --check          # exit 0, no diff
$ cargo clippy --workspace --all-targets    # 0 warnings
$ cargo nextest run --workspace
     Summary [ 127.289s] 1702 tests run: 1702 passed (2 slow), 0 skipped
$ cargo nextest run -p gauntlet-arch-tests   # boundaries
     Summary [  32.190s] 261 tests run: 261 passed, 0 skipped
$ cargo deny check           # (run as part of `just ci`, exit 0)
$ ./scripts/verify-ruler.sh
verify-ruler.sh: PASS — answer key and manifest digests match the frozen lock
$ just ci
ci: all gates green
```

`the_plan_gate_shape_writes_the_charter_report_and_the_matrix_holds`
(the exact task-26 conformance gate test, running the canonical fixture
end to end) is included in the 1702 and passes — the canonical
`tests/fixtures/baseline/` fixture and its consumers are untouched.

No previously `implemented_and_proven` Core unit failed. AK-069 was not
upgraded.
