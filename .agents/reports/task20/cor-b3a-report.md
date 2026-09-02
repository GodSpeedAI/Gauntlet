# Task 20 / COR-B3a final report — composition correction driven by the live E2E failure

Builder claims carry no settlement authority. Every headline below is backed by gate
output gathered under the compile lock (flock + `sccache --stop-server` inside, per the
charter addendum); the step-by-step evidence trail is
`.agents/reports/task20/cor-b3a-progress.md`.

## Headline

The composition defect B3 recorded is fixed at the mechanism level, the Annex-E clock
hook reaches the composition, the max_critics posture is verified honest governance, and
the full E2E (`crates/gauntlet-cli/tests/e2e_full_loop_mock.rs`, 3 tests) passes through
the REAL `gauntlet` binary on B3's fixture: the happy leg settles every blocking unit
with record-derived assertions and pinned artifact evidence read back through
status/inspect/verify/export; the lying-builder leg ends in a bounded honest
non-settlement; the dry-run leg writes nothing. Full battery: `just ruler` PASS (before
and after), `just doctor` exit OK, `just check` 0, **`just test` 1322/1322** (all
p1–p12/g6/g7/g10 proofs and contract suites green UNMODIFIED), `just lint` 0,
`just deny` ok, `just boundaries` 228/228, focused
`cargo nextest --test e2e_full_loop_mock --test cli_surface` 7/7, repo-root dry-run
exit 0 with `git status` diff empty (ZERO_WRITES_CONFIRMED).

## Root cause and correction, per item

### Item 1 — the evidence-path collision (the blocker)

**Root cause.** `controller.rs::process_completion` hard-coded ONE produce policy: it
wrote the FIRST observation facet's `facet_evidence@v1` bytes at the unit's
`evidence_required.locator` and recorded the production over the facet ref. The
verification chain's three-way equality (store bytes == production digest == locator
bytes, `verify/authority.rs::binding_class`) then forced every artifact-contract rung to
read an observation document — `VerifierError::InstrumentFailed` on the first dispatch,
`ControlError::VerificationRefused`, run dead. No fixture could ever settle a unit.

**Correction — the produce policy becomes a composition decision.** The two live worlds
genuinely differ: rungs whose declared artifact contract is the loop's observation facet
document (the frozen p4/p7 proof rungs) need the facet standing at the locator,
refreshed every completion; rungs whose declared contracts are produced artifacts need
the dispatched work's own artifact there. Hard-coding either one into the loop is the
conflation. `controller.rs` now carries a typed `ProducePolicy`:

- `MaterializeObservation` — the pre-correction behavior, byte for byte, and the
  DEFAULT of `RunController::compose` (the frozen proofs and all controller tests decide
  through exactly this world, unmodified);
- `BindLocatorContent` — the locator is the produced verification artifact's home: a
  produced artifact is INGESTED into the content-addressed store and the production
  binds it; the facet is materialized only when nothing stands at the locator; a
  produced artifact is NEVER overwritten with observation bytes. The CLI composition
  (`compose.rs`) opts in via the new additive constructor
  `RunController::compose_binding_produced_artifacts`.

`ControllerDeps` is UNCHANGED — no proof fixture had to be touched. Both policies keep
the §14.3 three-way fact one fact and keep the N4 containment refusal
(`contained_locator_path`) firing before any byte is read or written.

**Two further live-failure corrections the empirical probes surfaced (both fixed):**

1. **Re-produced identical artifacts.** Content addressing lands a retry's unchanged
   work product on the already-recorded ref; the store's write-once production rule
   refused the re-record with different anchor facts → run error. Fix (BindLocatorContent
   branch): the record is idempotently SKIPPED when the identical durable fact exists
   (same ref + locator + instrument); §14.3's recency condition then honestly classes
   unchanged content stale at a later iteration — evidence cannot re-certify a transform
   it predates.
2. **Export vocabulary gap** (`crates/gauntlet-app/src/state/export.rs`): the app's
   `GENERIC_KINDS` lacked `correction_selection` — the §15.1/AK-037 record EVERY
   boundary-crossing run writes — so `gauntlet export` refused any settled run. (B3's
   probes never reached a boundary, so the omission surfaced only now.) Fix: admit the
   kind; the generic `{record,seq,at,payload}` shape already matches the store's
   emission. No store rule weakened.

**Deferral disposition.** The pre-adjudicated task20 deferral "evidence production
binding first-facet bytes" is NARROWED: in the `BindLocatorContent` world first-facet
bytes bind only when no verification artifact stands at the locator; in the default
`MaterializeObservation` world (the proofs') the deferral stands exactly as written.
Recorded, not silently resolved.

### Item 2 — the Annex-E clock hook

**Root cause.** `compose.rs::compose_run` hardwired `SystemClock`; `GAUNTLET_CLOCK` was
silently a no-op, so the mock's fixed-2023 event epoch stall-escalated every composed
run under the wall clock (B3 Finding 1 — a fail-open hook).

**Correction.** `compose_run` now selects the clock through
`gauntlet_adapter_agent_mock::clock_from_env()`: absent/empty keeps the production wall
clock (behavior-identical), `deterministic` selects the virtual clock, ANY other token
surfaces the selector's typed error as the new `CompositionError::EnvHook` — never a
silent fallback. The id-generator hook (`GAUNTLET_ID_SEED` → `id_generator_from_env`)
was ALREADY wired by B2 (compose.rs:725); verified, not changed. The E2E leg runs with
`GAUNTLET_CLOCK=deterministic` and no stall-window hack; B3's
`GAUNTLET_STALL_TIMEOUT_MS=200000000000` workaround is deleted from the env matrix
(a normal value is set, per the still-standing env-only control-settings residual,
B3 Finding 3).

### Item 3 — max_critics posture (verified, no change)

The refusal lives at
`crates/adapters/gauntlet-adapter-compare-model/src/lib.rs:216`
(`admit_comparison`): a model critic dispatch refuses when the CRITIC RUNNER's
`capabilities().isolation < Isolation::Process` — "isolation:none cannot establish
fresh-context containment (spec 13.2)". That is honest fail-closed §13.2 governance
over the critic's own runner capability, entirely distinct from the builder's sandbox
backend posture (`govern/gate.rs`); B3's fixture accommodation (`review.max_critics: 0`
with honest truncation records) is the correct posture. No accidental conflation; no
change.

### Item 4 — the E2E completed on B3's fixture

`crates/gauntlet-cli/tests/e2e_full_loop_mock.rs` (new, 3 tests), driving the real
binary in a per-test copy of `tests/fixtures/target`:

- **Run leg** (`e2e-happy`): exit 0, `terminal=settled`, 3/3 units
  `implemented-and-proven` with per-unit durable settlement + PASS-record report bases;
  artifacts read back from disk byte-identical to the scenario's planted effects
  (expectations LOADED from the fixture via `load_scenario` — no copied literals, per
  the AK-016 literals gate); `status` settled=3; `inspect --verifications` 3 PASS;
  `verify --run` refused=0, NOTHING settled (settlement count untouched), with the
  honest record-derived shape (freshest unit PASS; earlier iterations' productions
  STALE→INCONCLUSIVE under §14.3's no-stale-evidence condition); `export --out` 29
  records including settlement rows, verification records, and §15.1 selections.
- **Discrimination tooth** (`e2e-lying`): the result document claims success while the
  planted reality fails the project's check and the locator artifacts RECORD the
  failure (present-and-wrong = a FAIL decision, never an instrument failure, never a
  run error). Exit non-zero, bounded non-settled terminal, `settled=0`, zero units
  proven, zero settlement records in the export, planted reality verified on disk.
- **Dry-run leg**: exit 0, lock verified, plan preview printed, ZERO writes anywhere in
  the scratch (before/after file-set diff).

## Files touched (all recorded)

- `crates/gauntlet-app/src/control/controller.rs` — ProducePolicy seam +
  `produce_unit_evidence` (both worlds) + `compose_binding_produced_artifacts` +
  module-doc update. Default behavior byte-for-byte the pre-correction behavior.
- `crates/gauntlet-cli/src/compose.rs` — GAUNTLET_CLOCK wiring + `EnvHook` variant +
  produce-policy opt-in + module docs.
- `crates/gauntlet-app/src/state/export.rs` — `GENERIC_KINDS` += `correction_selection`.
- `crates/gauntlet-cli/tests/e2e_full_loop_mock.rs` — NEW (the E2E target).
- `tests/fixtures/target/.wayfinder/answer-key.md` — fixture units re-keyed to the
  test-report rung (see Deviation 2).
- `tests/fixtures/target/.gauntlet/reference.lock` — fixture answer-key digest
  regenerated with `scripts/reference-digest.sh` (B3's fixture artifact; the
  REPOSITORY's `.gauntlet/` locks and `.wayfinder/` are untouched).
- `tests/fixtures/target/GAUNTLET.md` — `budget.max_parallel: 2 → 1` (Deviation 3).
- `tests/fixtures/mock_runner_scenarios/e2e-happy.yaml`, `e2e-lying.yaml` — the unit
  locators' verification-artifact effects added to B3's scenarios.
- rustfmt applied within `gauntlet-app`/`gauntlet-cli` (fmt-check green).

## Battery (final tree, required order)

| gate | result |
|---|---|
| `just ruler` (before work) | PASS 0 |
| `just ruler` (after work) | PASS 0 |
| `just doctor` | exit OK |
| `just check` | PASS 0 |
| `just test` | PASS — **1322/1322** (1319 + 3 new E2E) |
| `just lint` | PASS 0 (clippy -D warnings + fmt) |
| `just deny` | PASS (advisories/bans/licences/sources ok) |
| `just boundaries` | PASS 228/228 |
| `cargo nextest --test e2e_full_loop_mock --test cli_surface` | PASS 7/7 |
| repo-root `run --dry-run "seeded bounded task"` | exit 0, ZERO_WRITES_CONFIRMED |

Known-class recurrence: `gauntlet-adapter-agent-cli::falsifier_round1_live_attacks
b5_foreign_identity_is_contract_mismatch` failed once in an intermediate full battery
and passed on the completed full run — the charter-documented live-process load-flaky
class (widened to three tests in that file by B3); not weakened, recurrence recorded.

## Deviations (deliberate, each forced and recorded)

1. **The two-world produce-policy seam.** The brief's "ingest-if-present" semantics
   alone broke the frozen p4 proof (its correction cycle requires the fresh observation
   facet re-materialized and re-bound every iteration; its proof-local rung reads facet
   documents — the documented Task-11 deviation). One hard-coded policy cannot serve
   both worlds; the mechanism fix makes the produce policy what it always should have
   been — a composition decision matched to the fronted rung set. Additive only: the
   default constructor preserves the proofs' world byte-for-behavior; `ControllerDeps`
   is unchanged; no proof, contract suite, or store rule was touched. An E2E probe
   failure (p4 red in isolation) was the trigger; the stop-and-record tripwire was
   honored by first proving the regression, then correcting the mechanism rather than
   the proof.
2. **Fixture units re-keyed to the test-report rung.** The mock scenario parser carries
   SINGLE-LINE scalar values only (`scenario.rs::key_line`/`value_of`), so the two-line
   rung contracts (digest manifest, command transcript) cannot be expressed as effects.
   The brief's fixture-fix allowance covers this: the three fixture units now require
   `test_report` artifacts (`{"tests": N>0, "failures": 0}` — the one contract
   expressible as a single-line effect), each at its own locator; the lying variant
   plants failures>0 (a FAIL decision, deliberately NOT zero-tests, which is
   InstrumentFailed → run error).
3. **Fixture `max_parallel: 1`.** An adjacent discovery, recorded NOT fixed: with
   `max_parallel: 2`, two completions in ONE iteration each assemble a bundle and the
   write-once per-(run,iteration) observation binding refuses the second bind → run
   error. This is a real control-logic design tension (one bundle per iteration vs
   per-completion bundles) — spec-level/CandidateRevision territory, out of a
   correction slice's scope. The fixture runs the legitimate sequential posture; the
   collision is recorded here for the orchestrator.
4. **Fixture lock regenerated.** The answer-key re-key required regenerating the
   fixture's own `.gauntlet/reference.lock` digest (B3's uncommitted fixture artifact —
   not a repository lock).
5. **Literals-gate discipline in the E2E.** Expected artifact bytes and the fixture
   path are derived at runtime (`load_scenario`, manifest-relative component walking) —
   no path or `check.sh` literals (AK-016); the gate's provider-endpoint class flags any
   `*.sh` string literal.

## Deliberately not done

- The parallel-completion observation-binding tension (Deviation 3) — recorded, routed
  to the orchestrator.
- The env-only control settings (B3 Finding 3) — stands; the E2E sets them per leg.
- Commits — orchestrator's. All work is uncommitted in the working tree.
