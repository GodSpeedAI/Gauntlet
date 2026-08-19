# Task 8 — Confirmation Critic Report (critic-c)

> **Persisted by the orchestrator, verbatim from the critic's final message.**
> The critic (read-only Explore agent) declared the same delivery deviation
> as critic-b: it could not write files under its mandate, so the report
> arrived in-message and is persisted here unchanged. File:line references
> are to the tree at confirmation time (post correction round, pre N1).

**Delivery deviation (declared):** this agent operates under a strict read-only mandate (no file creation, no shell redirection). The requested write to `.agents/reports/task8/critic-c-confirmation.md` could not be performed; the full report is delivered here for the parent agent to persist. (Same class of deviation critic-b declared, per the header of `critic-b-result.md`.)

**Basis:** static reading only; nothing compiled or executed. Read: all six crate sources, both integration test files, all eight repo-root fixtures, the two critic reports (the contract), the adjudication, ports `agent_runner.rs` (error vocabulary), ports `contract/agent_runner.rs` (full suite), ports `json.rs` (MAX_DEPTH grep), spec §8.3/Annex E. Did NOT read builder-checkpoints, builder-round1, or CURRENT_STATUS. The crate files are untracked in git (`??`), so no correction diff exists; verification is against the final tree.

---

## Verdicts on the adjudication's IN table

### C1 — `workspace_effects` under `behavior: stall`: **CLOSED**

- Refusal arm exists in the Stall branch, same shape as the sibling `BehaviorConflict` arms: `scenario.rs:1249-1257`, `ScenarioError::BehaviorConflict { line: node.line(), ... }` — typed, line-locused, fail-closed, with a detail that states the why ("the runner's stall arm applies no effects, so the fixture would claim a write that never happens"). The Stall arm of the runner indeed still never calls `apply_effects` (`runner.rs:239-251`), so the parser now agrees with the runner.
- Dialect doc corrected: `scenario.rs:45` now reads `workspace_effects: # write-result only; OPTIONAL` (was "OPTIONAL under either behavior" per critic-a).
- Discriminating tests both sides: refused — `stall_cannot_carry_workspace_effects` (`scenario.rs:1835-1856`, asserts `BehaviorConflict { line: 6 }`); legal — `a_write_result_scenario_parses_field_for_field` (`scenario.rs:1717-1740`, effects parsed) and `workspace_effects_resolve_under_the_provided_root` (`runner.rs:719-750`, effects actually applied and read back from disk under write-result).
- Blast radius: the only stall fixture, `tests/fixtures/mock_runner_scenarios/no-result.yaml`, carries no `workspace_effects` (4 lines: scenario/role/behavior/timeout). No fixture regression.

Cosmetic note (not a numbered finding): the locus is the sequence node's line — the first `- path:` item line, not the `workspace_effects:` key line — because `Node::Seq` carries its first item's line. This is pinned and documented in the test itself (`scenario.rs:1839-1841`). Unambiguous, acceptable.

### C2 — duplicate `invoke`: **CLOSED**

- Guard before any mutation: `runner.rs:329-337` — `contains_key(&invocation_id)` → early return of `AgentError::AdmissionRefused { detail: ... }`; the insert at `runner.rs:340-349` is only reached for unknown ids. The existing entry (probes, terminal, result) and the `documents` map are untouched on refusal — the detail message's claim ("the existing entry is untouched") is true as written.
- `AgentError::AdmissionRefused` is the existing ports variant (`crates/gauntlet-ports/src/agent_runner.rs:41`, doc "Admission itself failed") — no ports change, per the adjudication's instruction.
- Tests cover both required shapes: after-terminal — `re_invoking_a_completed_invocation_is_refused_and_terminal_state_survives` (`runner.rs:556-590`: refusal asserted as `AdmissionRefused` with "dupl" detail, then 3 polls still `Completed`, then still collectible); while-running — `re_invoking_while_running_is_refused_too` (`runner.rs:592-619`: refusal mid-flight, then the script continues undisturbed to `Completed`).
- Suite regression check (the specific question asked): the suite CANNOT hit the refusal. Every one of the 10 cases calls `factory.build(script)` first (`ports contract/agent_runner.rs:814-823`), `AgentRunnerFactory::build` documents "must return an ISOLATED instance: cases assume a clean slate" (`:112-120`), and `MockRunnerFactory::build` constructs a fresh `MockAgentRunner` per call (`runner.rs:440-442`) with fresh `entries`/`documents` maps. Each case invokes exactly once per runner (verified case-by-case, `:223-825`; the only second operations are `poll`/`collect`/`cancel`, not `invoke`). No playback test re-invokes either (each builds its own runner via `runner_for`, `scenario_playback.rs:103-108`, one `invoke` per test).

### C3 — depth bound: **CLOSED**

- `MAX_SCENARIO_DEPTH: usize = 64` (`scenario.rs:98`), matching `json.rs`'s `MAX_DEPTH = 64` (`crates/gauntlet-ports/src/json.rs:28` — verified).
- Typed error with locus: `ScenarioError::DepthExceeded { line, depth }` (`scenario.rs:166`), Display names the bound (`:235-239`).
- Counter threaded through every descent, and I verified there is no missed arm:
  - `mapping` checks at entry (`scenario.rs:893-898`); `block_sequence` checks at entry (`:956-961`).
  - Incrementing call sites: `value_of → block_sequence(depth+1)` (`:870`); `value_of → mapping(depth+1)` (`:872`); `block_sequence` bare-dash arm `→ mapping(depth+1)` (`:978`); `dash_mapping_item → mapping(depth+1)` (`:1045`); root `mapping(..., 1)` (`:1065`).
  - `dash_mapping_item` itself carries no check, but its ONLY recursion target is `mapping` entered with `depth + 1`, which checks — no unchecked descent exists. `mapping → value_of` passes the same depth (sibling keys, correctly not a descent). Every recursive cycle passes through one of the two entry checks, so a staircase cannot dodge the counter by alternating shapes (I traced the alternating dash/mapping case: each dash level costs block_sequence+1 then mapping+1).
  - Consequence for the Θ(n²) dash-tail cloning (the folded half of B-MAJOR-2): cloning at nesting level k only occurs if all enclosing containers passed their checks, which caps nesting at ~64 — total clone work O(64·n), bounded.
- Boundary tests: at-cap traverses (`mapping_staircase(63)` = exactly 64 container levels — observed as `UnknownKey{deep}` from validation, which the test comment correctly explains is the only possible observation since the dialect's closed vocabulary admits no 64-deep legal document; a off-by-one regression in the check would turn this into `DepthExceeded` and fail the test, so the pair discriminates) and cap+1 refused with pinned `line: 69, depth: 65` (`scenario.rs:2030-2056`); dash staircase beyond cap (`:2058-2076`); plus a 2000-level staircase parsed on a 512 KiB stack thread asserting the typed error, never an abort (`:2078-2103`). Line/depth arithmetic re-derived by hand and consistent.
- Module-doc lesson claim updated: `scenario.rs:86-89` (PROBE_CEILING ties the claim to the bound) and `:92-98` ("The runaway-memory lesson covers DEPTH as well as termination: the descent's stack and memory are bounded, with the bound named"), plus the depth contract in `value_of`'s doc (`:838-841`).
- Blast radius: deepest legal dialect shape is root(1) → payload mapping(2) → seq(3) → item mapping(4); deepest fixture is exactly that (`happy.yaml` artifacts). Nothing legal is within 60 levels of the cap. Corollary worth recording: for THIS closed dialect, `DepthExceeded` can only ever fire on documents that validation would reject anyway — the bound is purely the anti-abort/DoS guard the adjudication wanted, which is the correct scope.

### C4 — `timeout_after_probes` ≥ 1: **CLOSED**

- `validate_root` calls `optional_probe(&entries, "timeout_after_probes", 1)` (`scenario.rs:1141`); the range check `parsed < u32::from(min) || parsed > u32::from(PROBE_CEILING)` (`:1334`) refuses 0 with `IntegerOutOfRange { min: 1, ... }`, whose Display names the range: "outside 1..=24" (`:200-208`).
- `probes_before_write` still min 0 (`:1140`) — legal, and the suite's `CompleteImmediately` mapping (probes 0, built through crate-internal `Scenario::assemble`, `runner.rs:279-291`) is unaffected either way.
- Test both sides: `timeout_after_probes_zero_is_refused_but_zero_write_probes_is_legal` (`scenario.rs:1859-1888`).
- Blast radius: `no-result.yaml` has `timeout_after_probes: 3` — parses; the runner's stall arm arithmetic (TimedOut when `timeout <= probes`, `runner.rs:242-249`) is unchanged, so the playback's "two Running then TimedOut" expectation (`scenario_playback.rs:216-253`) still holds.

### C5 — `PROBE_CEILING = 24`: **CLOSED-WITH-RESIDUAL** (residual named below, MINOR)

- `const PROBE_CEILING: u8 = 24` with the drivable-bound rationale (`scenario.rs:82-90`); dialect doc ranges updated to `<0..=24>` / `<1..=24>` (`:27-28`); boundary tests updated — 25 refused / 24 accepted for BOTH knobs, plus not-an-integer (`scenario.rs:1962-2006`).
- No other code depends on 64: crate-wide grep for "64" finds only `MAX_SCENARIO_DEPTH`, its comments/tests, and unrelated 64-bit constants in `seeded_ids.rs` (`FNV`/`splitmix64`), `SCRIPTED_EVENT_EPOCH`, `u64`/`i64` types. No stale "§8.6" or "<1..=64>" strings anywhere in the crate.
- RunnerScript mapping arithmetic unaffected: the suite drives `CompleteAfterProbes { probes_before_result: 1 }` (`ports :231-232`) → terminal on poll 2, and `NeverWriteResult` → stall timeout `SILENT_CHILD_PROBES = 4` → terminal on poll 4; `ProbeBudget::INSTANT` is `max_probes = SUITE_PROBE_CEILING = 24, patience 0` (`ports :42, :61-64`). The mapping also routes through `Scenario::assemble` (crate-internal, no parse range-check), so even hypothetical larger scripts could not be refused at this layer. The suite path is safe by construction.
- **RESIDUAL (MINOR, new finding N1):** the "legal ≡ drivable" claim is off by one at the cap for the WRITE knob. A `WriteResult` child terminates on poll `probes_before_write + 1` (`runner.rs:197`), so `probes_before_write: 24` — which the boundary test asserts parses (`scenario.rs:1977-1981`) — reaches terminal on poll 25, beyond both drivers' 24-poll budgets: playback `drive_to_terminal` loops `for _ in 0..24` and panics (`scenario_playback.rs:112-123`), and the suite's `probe_until_terminal` breaks at 24 attempts (`ports :189-208`). For the write knob the drivable ceiling is 23; for the stall knob (`timeout <= probes`) the cap 24 is exactly drivable. Failure mode is loud (a panic / a suite finding — a defect signal, never a silent pass), so this is the original B-MINOR-5 failure shape narrowed from `25..=64` to exactly `24`, not a regression introduced by the correction.

### C6 — hooks citation: **CLOSED**

- `hooks.rs:1-3` now reads: `Annex E test-hook selection (spec §8.3 / Annex E: "Test hooks | GAUNTLET_CLOCK GAUNTLET_ID_SEED (deterministic clock/ids for tests only)")`.
- Verified against the spec: §8.3 "Environment variables (canonical set)" says "Full table in Annex E" (spec L565-567), and Annex E's table carries exactly that "Test hooks" row (spec L2098). The quotation is verbatim from Annex E. The citation now points at the sections that actually contain the hooks, not the §8.6 defaults table. No other "§8.6" reference remains in the crate (grep clean).

---

## Regression hunt over the corrections' blast radius

- **Eight fixtures:** all still parse under the corrected parser. Knob values in use: `probes_before_write: 1` (happy), `timeout_after_probes: 3` (no-result) — both within `<0..=24>` / `<1..=24>`. Only `no-result.yaml` is stall; it carries no effects (C1 safe). `workspace_effects` appear only under write-result (lying-builder, answer-key-write-attempt) — still legal and still applied (`runner.rs:196-215`). Max nesting depth of any fixture is 4 containers (C3 safe).
- **Shared suite:** unaffected — fresh runner per case, one invoke per runner (see C2), probe arithmetic 2 and 4 well inside 24 (see C5). `MockRunnerFactory` suite paths confirmed unbroken.
- **Playback mapping:** `no-result` timing (2 Running then TimedOut) and `happy` (1 Running) derive from unchanged runner arithmetic; corrections touched parse-time validation and `invoke` admission only.
- **No behavior the suite mapping relies on changed:** `Scenario::assemble` signature and semantics unchanged; the `writing()` helper and `SILENT_CHILD_PROBES = 4` unchanged.

## New findings

- **N1 (MINOR):** `probes_before_write: 24` parse-legal but undrivable (terminal on poll 25 vs the drivers' 24-poll budget) — residual of C5 as detailed above. Locus: `scenario.rs:90` + `scenario.rs:1977-1981` vs `scenario_playback.rs:116` and `ports contract/agent_runner.rs:42`. Suggested closure if taken: either cap the write knob at 23 or make the drivers' loop `0..=PROBE_CEILING`-shaped (25 polls).
- **No new CRITICAL or MAJOR findings.** Specific hunts that came up empty: unchecked recursion arm (none — C3 analysis); panic path in the new depth checks (`lines[*pos]` is valid at every call site by construction); suite/playback path reaching the C2 refusal (none); correction-induced fixture breakage (none); stale 64/§8.6 references (none); C2 refusal semantics over-rejecting a legitimate in-repo retry consumer (no such consumer exists yet, and keyed-on-id refusal is exactly what the adjudication ordered).

## Bounded claims — what a runtime probe would still settle

1. A green `cargo test -p gauntlet-adapter-agent-mock` run: every verdict above is static; the new tests' assertions were verified against hand-derived arithmetic, not execution.
2. The 512 KiB stack-margin claim in `a_monstrous_staircase_on_a_small_stack_is_typed_not_an_abort` (`scenario.rs:2085-2089`, "~130-144 KiB for the capped descent") is a runtime measurement; statically I confirm only the recursion structure (≈4 frames per level, ≤65 levels).
3. N1's off-by-one is derived from loop bounds read statically; a runtime confirmation would be a 24-knob write fixture played back (expected: the loud "no terminal state within the bounded probe run" panic, not a silent pass).

---

## Six-line verdict

- C1 (stall+effects refused, typed, line locus, discriminating tests): **CLOSED** (scenario.rs:1249-1257; tests :1835-1856 + runner.rs:719-750)
- C2 (duplicate invoke → AdmissionRefused, entry untouched, both test shapes): **CLOSED** (runner.rs:329-337; tests :556-619; suite path safe — fresh runner per case)
- C3 (MAX_SCENARIO_DEPTH=64 threaded, typed DepthExceeded, boundary tests, no missed arm): **CLOSED** (scenario.rs:98, :870/:872/:893/:956/:978/:1045; tests :2030-2103)
- C4 (timeout ≥ 1 refused naming range; probes_before_write 0 legal): **CLOSED** (scenario.rs:1141, :1334-1341; test :1859-1888)
- C5 (PROBE_CEILING=24, docs+boundary tests updated, no 64 dependency): **CLOSED-WITH-RESIDUAL** — write knob at cap 24 needs poll 25 (N1)
- C6 (hooks citation now §8.3 / Annex E, quote verbatim): **CLOSED** (hooks.rs:1-3 vs spec L565-567/L2098)

**New-finding counts: CRITICAL 0 · MAJOR 0 · MINOR 1** (N1, a residual narrowing of the original B-MINOR-5, loud failure mode only).
