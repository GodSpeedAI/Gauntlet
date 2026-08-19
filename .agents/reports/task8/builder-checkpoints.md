# Task 8 — builder checkpoints (message-loss discipline: this file is the contract)

## Checkpoint 1 — D4 (deterministic clock + seeded ids + env hooks)

- **Landed:**
  - `crates/adapters/gauntlet-adapter-agent-mock/src/deterministic_clock.rs` —
    `DeterministicClock` (virtual time; `sleep_until` = monotonic max, never real
    sleep, never backwards) implementing `gauntlet_ports::clock::Clock`.
  - `crates/adapters/gauntlet-adapter-agent-mock/src/seeded_ids.rs` —
    `SeededIdGenerator` (FNV-1a seed hash → splitmix64 avalanche → 5-bit windows
    over a 128-bit stream → 26 Crockford chars; derivation documented in the
    module header) implementing `gauntlet_ports::id_generator::IdGenerator`.
  - `crates/adapters/gauntlet-adapter-agent-mock/src/hooks.rs` — Annex E
    selection: `select_clock(Option<&str>)` (exact token `deterministic`,
    absent/empty → None keeps production clock, unknown → typed
    `EnvHookError::UnknownClockToken`, never silent fallback),
    `select_id_generator(Option<&str>)`, plus `clock_from_env()` /
    `id_generator_from_env()` wrappers.
  - `crates/adapters/gauntlet-adapter-agent-mock/src/lib.rs` — module docs +
    exports for the landed slice.
- **Design decision (recorded):** env mutation under edition 2024 is `unsafe`
  (`set_var` races) and the workspace forbids `unsafe` — so the selection
  semantics are PURE functions over the raw hook value and the env wrappers
  only read+delegate. Tests exercise the pure selectors; no env mutation
  anywhere in the crate. The "nextest runs each test in its own process" note
  lives in the hooks module header for future harness users.
- **Ran:** `cargo nextest run -p gauntlet-adapter-agent-mock` — 15/15 PASS
  (4 clock, 6 hooks, 5 id-generator; 0.02s). First run surfaced two compile
  errors (tuple-struct syntax; edition-2024 unsafe env vars) — both fixed by
  design change, not suppression; re-run green.
- **Next:** D2 scenario parser (`src/scenario.rs`), then D1 runner, then
  fixtures + playback tests.

## Checkpoint 2 — D2 (scenario fixture frontend)

- **Landed:** `crates/adapters/gauntlet-adapter-agent-mock/src/scenario.rs`
  (~1000 lines incl. tests) — strict YAML-subset parser (dialect documented
  in the module header: plain scalars, flow seqs of scalars, block seqs of
  scalars/mappings; tabs-in-indent refused; unknown/duplicate keys typed
  with line loci), typed `Scenario`/`ChildBehavior`/`Derailment`/
  `PayloadParts`/`WorkspaceEffect` model, Annex-B.1/B.2 document renderer
  (valid by construction; derailments: foreign-invocation-id /
  stale-envelope-digest / unknown-envelope-field / unknown-nested-field /
  raw-malformed), fixture loader `load_scenario(name)` with stem↔identity
  cross-check, `scenario_fixtures_dir()` via CARGO_MANIFEST_DIR join (no
  absolute literals). lib.rs exports updated.
- **Position contract enforced** (Task-7 lesson): `value_of` entered ON the
  key line leaves past the whole value block on EVERY path; dash-mapping
  items collect their subtree and verify exhaustion; root parse turns any
  leftover line into typed `UnplaceableLine`; probe knobs hard-bounded
  `0..=64` (`PROBE_CEILING`).
- **Ran:** `cargo nextest run -p gauntlet-adapter-agent-mock` — 42/42 PASS
  (15 prior + 27 new scenario tests; 0.04s). Intermediate compile errors
  (tuple-destructure on `find`, partial-move `matches!` guards, one test
  that dropped state before payload and asserted the wrong MissingKey)
  all fixed; one test corrected to drop only what it names — no
  assertions weakened.
- **Next:** D1 runner (`src/runner.rs` + `tests/contract_suite.rs` AK-059
  pin), then D3 fixtures + playback tests.

## Checkpoint 3 — D1 (mock AgentRunner + factory)

- **Landed:**
  - `crates/adapters/gauntlet-adapter-agent-mock/src/runner.rs` —
    `MockAgentRunner` (invoke records dispatch facts and returns promptly;
    poll advances the script, never blocks; completion ONLY through
    `read_result_contract` with `failure_class()` mapping; terminal
    statuses stable forever; idempotent cancel with typed
    AlreadyTerminal/UnknownHandle; collect only after Completed, same
    document every time; effect-application failure →
    Failed{agent_execution}, never a panic) + `MockRunnerFactory`
    (`RunnerScript → Scenario` mapping, per-case isolated runners,
    scratch temp roots, no effects ever written by suite scripts).
    Capabilities HONEST: `isolation: None` (Task 14 must be able to
    refuse for contained tiers), `result_delivery: File` as an EMULATION
    (raw document text held keyed by the declared ResultContractPath —
    retrievable via `written_document` for forensics),
    `supports_native_subagents/streaming_events/model_selection: false`,
    `max_depth: 0`.
  - `crates/adapters/gauntlet-adapter-agent-mock/tests/contract_suite.rs`
    — `the_mock_runner_passes_the_unmodified_shared_contract_suite`
    (AK-059 cited in the test; `agent_runner_suite(&MockRunnerFactory,
    ProbeBudget::INSTANT)` must return EMPTY findings).
- **Ran:** `cargo nextest run -p gauntlet-adapter-agent-mock` — 52/52 PASS
  (0.06s), including the suite pin. One test corrected (in-flight-work
  assertion needed `probes_before_write: 1`; the default-0 scenario
  completes on first poll by design, as the suite's
  CompleteImmediately case requires).
- **Next:** D3 — the eight REQUIRED fixtures under
  `tests/fixtures/mock_runner_scenarios/` + playback tests
  (`tests/scenario_playback.rs`).

## Checkpoint 4 — D3 (eight REQUIRED fixtures + playback tests)

- **Landed:**
  - `tests/fixtures/mock_runner_scenarios/{happy,malformed,no-result,
    wrong-invocation-id,stale-envelope-digest,lying-builder,
    answer-key-write-attempt,critic-pass-without-evidence}.yaml` —
    AK-059's frozen locator spelling, materialized as written. All encode
    CHILD behavior only.
  - `crates/adapters/gauntlet-adapter-agent-mock/tests/scenario_playback.rs`
    — 9 tests: the fixture-inventory pin (all eight present, parse, and
    declare their stems) + one playback per scenario, each asserting a
    distinct runtime state: happy (Running probe then Completed, identity
    echo, repeated-collect equality); malformed (Failed{malformed_result},
    stable, never collectible); no-result (2× Running then TimedOut,
    never Completed); wrong-invocation-id / stale-envelope-digest
    (Failed{contract_mismatch}); lying-builder (Completed with the
    divergence observable on BOTH sides — artifact bytes on disk fail the
    declared check AND the document's self_check claims exit 0);
    answer-key-write-attempt (stand-in bytes changed + changed_paths owns
    the path + the REAL `.wayfinder/answer-key.md` byte-identical
    before/after); critic-pass-without-evidence (Completed + collectible,
    role critic, claims carry "passes", artifacts AND self_checks empty —
    P11 shape pinned at the envelope level, design note in report).
- **Ran:** `cargo nextest run -p gauntlet-adapter-agent-mock` — 61/61 PASS
  (0.07s). One fixture defect found by its own playback test (the
  lying-builder artifact content contained the substring "PASS", which
  the divergence assertion correctly rejected); fixture content fixed,
  assertion untouched.
- **Next:** D6 gates — ports suite run, just check/lint/deny/boundaries/
  ruler, target-size watch, memory-peak spot check.

## Checkpoint 5 — lint closure

- **Changed (2 lines, no behavior change), both in
  `crates/adapters/gauntlet-adapter-agent-mock/src/scenario.rs`:**
  1. Line 591 `redundant_closure`: `.map(|check| self_check_object(check))`
     → `.map(self_check_object)`.
  2. Lines 677-681 `collapsible_if`: nested `if let Some(byte) … { if
     byte.is_ascii() … }` collapsed to a single edition-2024 let-chain
     `if let Some(byte) = bytes.get(index) && byte.is_ascii() { … }`.
     No `#[allow]` added; nothing else touched.
- **Gates (exclusive build lock held):**
  - `cargo clippy --workspace --all-targets -- -D warnings` → **exit 0**.
    Tail: `Checking gauntlet-domain v0.1.0 … Finished \`dev\` profile
    [unoptimized] target(s) in 6.02s`.
  - `cargo nextest run -p gauntlet-adapter-agent-mock` → **exit 0,
    61 tests run: 61 passed, 0 skipped** (0.095s).
  - `cargo fmt --check` → **exit 1 — PRE-EXISTING, not caused by this
    slice.** 50 diff hunks across this crate's untracked files
    (`runner.rs` 6, `scenario.rs` 38, `seeded_ids.rs` 4,
    `tests/scenario_playback.rs` 4), e.g. `zero_payment()` tuple
    lines (656/666) and `enum Node` (700) — code written before this
    slice that was never fmt'd. Verified the two edited spots
    (≈591 and ≈674-678) appear in NO hunk; the edits are fmt-clean.
    Out of scope for this slice per its "touch nothing else / no
    formatting churn" rule — left for a dedicated formatting slice or
    the D6 gate owner.

## Checkpoint 6 — fmt normalization

- **Command:** `cargo fmt -p gauntlet-adapter-agent-mock` (exit 0,
  formatting only, zero semantic changes).
- **Files touched by fmt (4, matching Checkpoint 5's 50 drift hunks;
  verified by mtime — lib.rs/deterministic_clock.rs/hooks.rs/
  contractsuite.rs untouched):**
  - `crates/adapters/gauntlet-adapter-agent-mock/src/runner.rs` (6 hunks)
  - `crates/adapters/gauntlet-adapter-agent-mock/src/scenario.rs` (38 hunks)
  - `crates/adapters/gauntlet-adapter-agent-mock/src/seeded_ids.rs` (4 hunks)
  - `crates/adapters/gauntlet-adapter-agent-mock/tests/scenario_playback.rs`
    (4 hunks)
- **Gates (exclusive build lock held), all green:**
  1. `cargo fmt --check` (workspace-wide) → **exit 0**.
  2. `cargo clippy --workspace --all-targets -- -D warnings` →
     **exit 0**. Tail: `Checking gauntlet-adapter-agent-mock v0.1.0 …
     Finished \`dev\` profile [unoptimized] target(s) in 0.81s`.
  3. `cargo nextest run -p gauntlet-adapter-agent-mock` → **exit 0,
     61 tests run: 61 passed, 0 skipped** (0.104s).
- No test expectations changed by formatting (61/61 identical to
  Checkpoint 5's run); no assertions adjusted.

## Checkpoint 7 — correction round C1–C6

Baseline before any edit: `cargo nextest run -p gauntlet-adapter-agent-mock`
→ 61/61 PASS. Every correction below followed fail-first (test added and
observed failing for the defect's own reason, then implemented, then
re-run green). Final battery at the end of this checkpoint. File:line
references are POST-fmt final state.

### C1 — stall + `workspace_effects` refused at parse time

- **Fail-first:** added `stall_cannot_carry_workspace_effects` (a stall
  scenario carrying one effect). `cargo nextest run -p
  gauntlet-adapter-agent-mock stall_cannot_carry_workspace_effects` →
  FAIL: panicked "a silent child writes no files: Scenario { … behavior:
  Stall, … workspace_effects: [WorkspaceEffect { path: "out/never.txt",
  content: "unreachable" }] }" — the parser admitted the effect the
  runner's Stall arm never applies (the silent lie, observed).
- **Change:** `crates/adapters/gauntlet-adapter-agent-mock/src/scenario.rs`
  Stall arm of `validate_root` (~:1246-1257): new `BehaviorConflict`
  refusal for `effects_node`, wording "behavior stall cannot carry
  workspace_effects; the runner's stall arm applies no effects…", line =
  the sequence node's first item line (the node carries the first `- `
  line, not the key line — noted in the test). Dialect doc (module
  header :45): "OPTIONAL under either behavior" → "write-result only;
  OPTIONAL"; behavior-agreement comment above the match updated to "a
  silent child writes nothing at all".
- **Green:** full package 62/62 (write-result-with-effects still parses —
  covered by the existing `a_write_result_scenario_parses_field_for_field`
  and runner effects tests).

### C2 — duplicate `invoke` refused, never silently replaced

- **Fail-first:** added `re_invoking_a_completed_invocation_is_refused_and_terminal_state_survives`
  and `re_invoking_while_running_is_refused_too`. `cargo nextest run -p
  gauntlet-adapter-agent-mock re_invoking` → 2 FAIL: "duplicate
  admission is refused mid-flight: InvocationHandle { … }" — `invoke`
  returned `Ok(handle)` for a known id (the resurrect-capable insert,
  observed).
- **Change:** `crates/adapters/gauntlet-adapter-agent-mock/src/runner.rs`
  `invoke` (:318-347): pre-insert guard — if `entries` already contains
  the id (terminal or not), return `Err(AgentError::AdmissionRefused)`
  with detail "invocation … is already admitted to this runner;
  duplicate admission is refused and the existing entry is untouched";
  the existing entry is never touched. Doc comment states the
  pinned-terminal-stability rationale (§6.1).
- **Green:** full package 64/64 — terminal entry still polls Completed
  forever after the refusal and stays collectible; the Running entry
  finishes its script undisturbed.

### C3 — parser depth bound (`MAX_SCENARIO_DEPTH = 64`)

- **Scaffolding note:** the `DepthExceeded` variant + const were added
  FIRST (purely additive; no check sites) so the fail-first tests compile
  and fail observationally rather than by compile error.
- **Fail-first:** added `mapping_staircase` helper + 3 tests.
  `cargo nextest run -p gauntlet-adapter-agent-mock staircase nesting_at`
  → 3 FAIL:
  1. boundary test: "got UnknownKey { line: 6, key: "deep" }" — the
     65-deep staircase traversed uncapped to validation instead of the
     typed refusal;
  2. dash-staircase test: same class (traversed, then UnknownKey);
  3. monstrous-staircase test: **SIGABRT — "thread '<unknown>' has
     overflowed its stack / fatal runtime error: stack overflow,
     aborting"** — the recorded defect class demonstrated live (process
     abort, not a typed error).
- **Change:** `scenario.rs`: `const MAX_SCENARIO_DEPTH: usize = 64`
  (:98, the ports json reader `MAX_DEPTH` precedent, json.rs:28);
  `ScenarioError::DepthExceeded { line, depth }` (:166, Display :235);
  depth counter threaded `value_of`(:842) / `mapping`(:887) /
  `block_sequence`(:951) / `dash_mapping_item`(:1018), container-entry
  checks at :894 and :957 (`depth > MAX_SCENARIO_DEPTH` → typed error
  naming the line), children handed `depth + 1`, root mapping entered at
  depth 1; `value_of` doc gains a depth-contract paragraph; the
  lesson-claim comment now covers depth/memory as well as termination
  (both consts, :82-99).
- **Margin measurement (recorded):** fixed code flips pass/abort between
  128 and 144 KiB of thread stack (capped descent ≈ 65 container levels);
  the permanent monstrous test uses a 512 KiB stack + a 2000-level
  staircase. Verified the test still discriminates: with the cap
  temporarily neutered (`MAX_SCENARIO_DEPTH = 64_000_000`), the 512 KiB
  run SIGABRTs (stack overflow) — then reverted. Capped margin ≈ 3.6x,
  uncapped usage ≈ 8x over the stack.
- **Green:** full package 67/67 — at-cap (63 nested + root + `deep` = 64
  container levels) traverses and is refused only by validation's
  UnknownKey (the dialect's closed vocabulary means no 64-deep document
  can validate; "parses" is observed as the traversal completing), one
  deeper yields `DepthExceeded { line: 69, depth: 65 }`.

### C4 — `timeout_after_probes` must be ≥ 1

- **Fail-first:** added
  `timeout_after_probes_zero_is_refused_but_zero_write_probes_is_legal`.
  `cargo nextest run -p gauntlet-adapter-agent-mock
  timeout_after_probes_zero` → FAIL: "zero probes is not a deadline:
  Scenario { … timeout_after_probes: Some(0) … }" — zero was accepted.
- **Change:** `scenario.rs`: `IntegerOutOfRange` gains a `min: u8` field
  (:134; Display :202 now says "outside {min}..={PROBE_CEILING}" so the
  message names the knob's real declared range); `optional_probe`
  takes `min` (:1312-1315) and refuses `parsed < min`; call sites
  `probes_before_write`→0, `timeout_after_probes`→1 (:1140-1141).
  Existing matches on the variant (`key, ..`) unaffected.
- **Green:** full package 68/68 — `timeout_after_probes: 0` →
  `IntegerOutOfRange { line: 4 }`; explicit `probes_before_write: 0`
  still parses; the suite's CompleteImmediately path (assembly, not
  parse) untouched.

### C5 — `PROBE_CEILING` aligned to the drivable bound (24)

- **Fail-first:** rewrote `probe_knobs_are_bounded_integers` to the new
  boundary (25 refused / 24 accepted, both knobs). `cargo nextest run -p
  gauntlet-adapter-agent-mock probe_knobs` → FAIL: "bounded: Scenario {
  … probes_before_write: 25 … }" — 25 was parse-legal but undrivable
  (every driver stops at 24: the suite's INSTANT budget
  `SUITE_PROBE_CEILING = 24`, the playback tests' 24-iteration loop).
- **Change:** `scenario.rs`: `const PROBE_CEILING: u8 = 24` (:90) with
  the drivable-bound rationale ("legal means drivable") and the lesson
  claim now spanning both bounds; dialect doc ranges updated to
  `<0..=24>` / `<1..=24>` (:27-28). `rg` confirmed nothing else depends
  on 64: `PROBE_CEILING` appears only in scenario.rs (error text and
  bound check derive from the const); fixtures use knob values 1 and 3;
  ports `json.rs MAX_DEPTH = 64` is a separate crate's depth const,
  deliberately untouched.
- **Green:** full package 68/68.

### C6 — hooks doc citation

- **Evidence (doc defect, observed):** `rg -n "8\.6" hooks.rs` → line 1
  cited "spec §8.6 / Annex E"; in `.agents/specs/Core-spec.md`, §8.6 is
  "Defaults table (conservative bootstrap)", §8.3 is "Environment
  variables (canonical set)", and the Test-hooks row
  (`GAUNTLET_CLOCK`/`GAUNTLET_ID_SEED`) lives in Annex E (:2098).
- **Change:** `crates/adapters/gauntlet-adapter-agent-mock/src/hooks.rs:1`
  — "spec §8.6 / Annex E" → "spec §8.3 / Annex E". Citation line only.

### Final gates (exclusive build lock held)

`cargo fmt -p gauntlet-adapter-agent-mock` run first (formatting only —
it normalized newly touched spots in `scenario.rs`/`runner.rs`; suite
identical 68/68 before and after; no assertions adjusted). Then:

1. `cargo nextest run -p gauntlet-adapter-agent-mock` → **68/68 PASS**
   (61 baseline + 7 new: C1×1, C2×2, C3×3, C4×1 = 7; C5 rewrote one
   existing test, C6 none).
2. `cargo nextest run -p gauntlet-ports agent_runner` → **3/3 PASS**
   (the shared suite drives a fresh runner per case, so C2's
   duplicate-admission refusal cannot affect it — confirmed by the run).
3. `cargo clippy --workspace --all-targets -- -D warnings` → **exit 0**.
4. `cargo fmt --check` → **exit 0**.
5. `just boundaries` → **190/190 PASS**.

No fixtures added or modified; no fixture files for the depth tests
(programmatic strings in-test, per the round's constraint); no new
dependencies; no scope beyond the six adjudicated corrections.

## Checkpoint 8 — N1 micro-fix (write-knob cap 23)

Confirmation-critic finding N1: Checkpoint 7's "legal ≡ drivable" claim
was off by one for the WRITE knob — a `write-result` child terminates on
poll `probes_before_write + 1` (`advance`: probes `1..=N` report
Running, poll `N+1` writes), so `probes_before_write: 24` parsed but
reached terminal on poll 25, beyond both drivers' 24-poll budgets. The
stall knob's 24 was already exact (timeout ≤ probes).

- **Fail-first:** rewrote the write-knob half of
  `probe_knobs_are_bounded_integers` to the corrected boundary (24
  refused / 23 accepted; the stall half — 24 accepted / 25 refused —
  kept as-is). `cargo nextest run -p gauntlet-adapter-agent-mock
  probe_knobs` → FAIL: "bounded: Scenario { … probes_before_write: 24 …
  }" — 24 was still admitted (terminal would land on poll 25,
  undrivable), observed.
- **Change (all in
  `crates/adapters/gauntlet-adapter-agent-mock/src/scenario.rs`):**
  1. New derived const `WRITE_PROBE_CEILING: u8 = PROBE_CEILING - 1`
     (:100; derived, not a second literal to keep in step) with the
     why (write children terminate one poll after their last in-flight
     probe); `PROBE_CEILING` doc re-scoped to "drivers' budget and the
     STALL knob's ceiling" (:82-92).
  2. `IntegerOutOfRange` gains `max: u8` (:131-146) so the message
     names the knob's real range — Display now prints
     "outside {min}..={max}" (:212-221) instead of a shared ceiling.
  3. `optional_probe` takes `(min, max)` (:1326-1331), refuses
     `parsed < min || parsed > max` (:1353-1360).
  4. Call sites: `probes_before_write` → `(0, WRITE_PROBE_CEILING)`,
     `timeout_after_probes` → `(1, PROBE_CEILING)` (:1153-1154).
  5. Dialect doc ranges corrected (:27-31):
     `probes_before_write: <0..=23>` with the one-line why (terminal
     poll stays inside 24), `timeout_after_probes: <1..=24>` ("times
     out ON its probe").
- **Green (exclusive build lock held):**
  1. `cargo nextest run -p gauntlet-adapter-agent-mock` → **68/68 PASS**
     (test count unchanged — one existing test rewritten, none added).
  2. `cargo clippy --workspace --all-targets -- -D warnings` →
     **exit 0**.
  3. `cargo fmt -p gauntlet-adapter-agent-mock` then
     `cargo fmt --check` → **exit 0**; post-fmt suite re-run 68/68, no
     assertions adjusted.
- No fixtures touched (both fixtures' knob values — 1 and 3 — are
  inside every bound); existing matches on `IntegerOutOfRange { key, .. }`
  unaffected by the new field.
