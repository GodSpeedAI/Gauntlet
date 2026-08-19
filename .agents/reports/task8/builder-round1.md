# Task 8 — builder round 1 final report

> **Provenance note (epistemic honesty):** the round-1 builder's session was
> interrupted after checkpoint 4 but before this report, the D6 gate run, and
> the critic loop. Per `.agents/lessons/session-interruption-recovery.md`,
> this slice was reconstructed by the orchestrator **from the checkpoint
> contract** (`builder-checkpoints.md`, the on-disk tree) plus the
> orchestrator's own independent gate runs — never from conversational
> memory. Builder claims below are cross-checked against tree state; gate
> outputs cited are the orchestrator's personal runs unless attributed.

## Artifact

`crates/adapters/gauntlet-adapter-agent-mock` (adapter layer) +
`tests/fixtures/mock_runner_scenarios/` (repository root — AK-059's frozen
`evidence_required.locator` spelling, materialized as written):

- `src/deterministic_clock.rs` — `DeterministicClock`: virtual time,
  monotonic-max `sleep_until`, never a real sleep, never backwards.
  Implements `gauntlet_ports::clock::Clock`.
- `src/seeded_ids.rs` — `SeededIdGenerator`: FNV-1a seed hash → splitmix64
  avalanche → 5-bit windows over `w0‖w1` → 26 Crockford chars; derivation
  documented in the module header; every mint validated by the domain's
  ULID grammar at the boundary. Implements
  `gauntlet_ports::id_generator::IdGenerator`.
- `src/hooks.rs` — Annex E selection: `GAUNTLET_CLOCK` token `deterministic`
  selects the virtual clock; unset/empty keeps production; any other value
  is typed `EnvHookError::UnknownClockToken` (never a silent fallback).
  `GAUNTLET_ID_SEED`: non-empty seeds the generator. Pure selectors over
  the raw value + env-reading wrappers; **no env mutation anywhere** —
  edition-2024 `set_var` is `unsafe` and the workspace forbids `unsafe`
  (design decision recorded in the module header).
- `src/scenario.rs` — strict YAML-subset fixture parser (documented
  dialect; tabs-in-indent refused; unknown/duplicate keys typed with line
  loci), typed `Scenario`/`ChildBehavior`/`Derailment`/`PayloadParts`/
  `WorkspaceEffect` model, Annex-B.1/B.2 document renderer with the five
  derailments (foreign-invocation-id / stale-envelope-digest /
  unknown-envelope-field / unknown-nested-field / raw-malformed),
  `load_scenario(name)` with stem↔identity cross-check,
  `scenario_fixtures_dir()` via CARGO_MANIFEST_DIR join. **Position
  contract enforced on one convention** (the Task-7 runaway-memory lesson):
  `value_of` entered ON the key line leaves past the whole value block on
  every path; leftover lines are typed `UnplaceableLine`; probe knobs
  hard-bounded `0..=64`.
- `src/runner.rs` — `MockAgentRunner` (invoke records dispatch facts and
  returns promptly; poll advances the script, never blocks; completion
  ONLY through `read_result_contract` — strict decode plus the §10.3
  rule-6 identity check — with failures mapped by `failure_class`;
  terminal statuses stable forever; idempotent typed cancel; collect only
  after Completed, same document every time; effect-application failure →
  `Failed{agent_execution}`, never a panic) + `MockRunnerFactory`
  (`RunnerScript → Scenario` mapping for the unmodified Task-4 suite).
  Capabilities HONEST: `isolation: None` (Task 14's tier check must be
  able to refuse this adapter), `result_delivery: File` as a documented
  EMULATION (raw document text keyed by `ResultContractPath`, retrievable
  via `written_document`), `max_depth: 0`.
- `tests/contract_suite.rs` — AK-059 pin:
  `agent_runner_suite(&MockRunnerFactory, ProbeBudget::INSTANT)` must
  return EMPTY findings (suite lives unmodified in `gauntlet-ports`).
- `tests/scenario_playback.rs` — 9 tests: fixture-inventory pin (all
  eight REQUIRED stems present, parsing, self-identifying) + one playback
  per scenario, each asserting a distinct runtime state (see checkpoint 4
  for the per-scenario state table).

The eight REQUIRED fixtures encode **child behavior only** — the mock
returns results, it decides nothing (the plan's redesign trigger is not
tripped).

## Gates (orchestrator-run, this recovery session)

| Gate | Result |
|---|---|
| `cargo nextest run -p gauntlet-adapter-agent-mock` | **61/61 PASS**, 0.09 s test time, ~196 MiB peak RSS (cargo driver) — no runaway memory |
| `cargo nextest run -p gauntlet-ports agent_runner` (suite lives at `contract::suite_runs::agent_runner_*`) | **3/3 PASS** incl. broken-is-caught teeth |
| `just check` | 0 |
| `just lint` | **RED at recovery start**: 2 clippy errors in `scenario.rs` (redundant closure :591; collapsible-if :677) — the interrupted session's D6 never ran, so these were never observed. Fixed by a delegated mechanical slice (checkpoint 5); re-run green. |
| `just deny` | ok (advisories/bans/licenses/sources) |
| `just boundaries` | **190/190 PASS** — the new crate passes layering, denylist, and literal scans |
| `just ruler` | PASS (session start; the answer-key-write-attempt playback also byte-compares the real ruler before/after) |

**Plan-text correction (recorded):** the plan's Task-8 gate line spells the
ports filter `contract::agent_runner`, which matches zero tests — the
Task-4 suite landed as `contract::suite_runs::agent_runner_{green,broken,caps}_is_caught`.
The working filter is the substring `agent_runner`. Corrected in
`.agents/plans/gauntlet-impl.md` (plan corrected to match reality; the
spec is untouched and outranks both).

## Design notes carried forward

- **P11 shape pinned at the envelope level** (critic-pass-without-evidence):
  Annex B defines exactly one role-result payload schema
  (`builder_result@v1`); whether a critic's comparison-record output flows
  through the runner port is a Task-10 question. The fixture pins the
  shape available HERE: schema-valid envelope, role critic, claims carry
  the pass story, both evidence-bearing arrays empty.
- **Suite-script mapping is total**: all seven `RunnerScript` variants map
  to scenario configurations; the suite's
  `CompleteAfterProbes{probes_before_result}` maps through the validated
  `Scenario::assemble` path.
- **Effect containment is double-guarded**: the parser refuses
  absolute/`..` paths at parse time; `apply_effects` re-checks
  `starts_with(workspace_root)` at resolution time.

## Declared residuals (builder's own honest list, from checkpoints)

- The suite's default silent-child stall (`SILENT_CHILD_PROBES = 4`) is an
  adapter constant, not a fixture knob — fixtures carry their own
  `timeout_after_probes`.
- `scratch_root()` for suite-driven runners creates nothing on disk (path
  uniqueness is all that is needed; suite scripts have no effects).

## Round-1 closure (orchestrator-appended, post loop)

- **Lint closure (delegated, checkpoints 5–6):** 2 clippy errors fixed
  exactly as flagged (redundant closure, collapsible-if); fmt drift across
  four files normalized; no semantic changes; 61/61 held throughout.
- **Teeth (orchestrator-run, all reverted, `teeth.md`):** T1 failure-class
  neutering → suite pin + malformed playback fail (wrong-invocation-id
  confirmed individually; 4 siblings cancelled by nextest per-binary
  cancellation — semantics documented in `teeth.md`); T2 REQUIRED fixture
  removed → inventory pin fails typed; T3 isolation rung inflated →
  honesty test fails. **Honest observation:** the shared suite does NOT
  pin adapter capability honesty (T3 passed under the lie) — Task-14
  deferral class, recorded.
- **Critic loop:** A (spec, blind): 0 critical / 1 major / 7 minor.
  B (adversarial, blind): 0 critical / 2 major / 6 minor. Adjudication +
  rationale for every no-change/deferral: `correction-adjudication.md`.
- **Correction round (delegated, fail-first per item, checkpoint 7):**
  C1 stall+effects parse-time refusal; C2 duplicate-invoke typed
  `AdmissionRefused` (entry untouched, terminal stability preserved);
  C3 `MAX_SCENARIO_DEPTH = 64` threaded through every descent arm with
  typed `DepthExceeded` — the fail-first run reproduced a **live stack
  overflow**, confirming the recorded defect class was real;
  C4 `timeout_after_probes ≥ 1`; C5 `PROBE_CEILING` 64→24 (legal ≡
  drivable); C6 hooks citation §8.3/Annex E.
- **Confirmation (blind):** C1–C4, C6 **CLOSED**; C5
  **CLOSED-WITH-RESIDUAL** (N1: write knob at cap 24 terminates on poll
  25) + 0 new critical/major. N1 closed by micro-fix (checkpoint 8):
  `WRITE_PROBE_CEILING = PROBE_CEILING - 1`, per-knob ranges in errors.
- **Final gates (orchestrator-run):** mock 68/68 (35 MiB peak RSS),
  ports agent_runner 3/3, workspace **547/547**, check/lint/deny 0,
  boundaries **190/190**, ruler PASS.

**Deferrals carried forward (from adjudication):** symlink pre-placement
under a test-provided root (B-m3; §17.5 placement is the primary guard;
trigger: effects against non-test-provided roots) · `documents`-map
forensic clobber on shared contract paths (B-m6; trigger: a second
`written_document` consumer or Task-9+ forensic needs) · shared suite not
pinning capability honesty (T3 observation; trigger: Task 14).
