# Task 20 / Builder B3 brief — E2E fixture project + e2e_full_loop_mock

You are Builder B3 for plan Task 20 in /home/sprime01/projects/gauntlet.
Read first, in order: `AGENTS.md` (repo root), `.agents/reports/task20/charter.md`
(decisions D1–D10 + operational addenda — binding), then this brief.
B1, B2a, and B2 are DONE — read `b1-report.md`, `b2a-report.md`,
`b2-report.md` (especially the exact composition behavior: runner tokens,
mock scenario mechanism, dry-run semantics, the report output, the
`[[bin]] name = "gauntlet"` change).

## Goal

Plan Task 20 step 4: an E2E test on a seeded bounded task — a tiny target
project under `tests/fixtures/target/` — using the mock runner and REAL
instruments, completing the full loop from freeze to settlement to report.
Done-when (plan): "the E2E run settles every blocking Unit of the fixture
reference with pinned evidence, and the report's status categories are
populated from actual records rather than hardcoded (AK-060, AK-061)."

## Item 1 — the fixture target project (`tests/fixtures/target/`)

A self-contained tiny project: a source file or two, a real check command
(e.g. a script the test facet can run), its own `GAUNTLET.md`
(`runners.default: mock-agent`; sandbox backend left at the shipped `none`
default), a `.wayfinder/reference.yaml` + `.wayfinder/answer-key.md` fixture
reference that the REAL parser and formation invariants accept (strict
closed field set — NO cohort key, D7; no fog in the key; every unit
validates). Design the units:

- 2–3 BLOCKING units, tier `local` (the shipped `none` sandbox posture
  admits local-tier dispatch; cross_cutting refuses — do not use it), each
  with `evidence_required` whose `kind`/`locator` real instruments satisfy:
  e.g. a `file_digest` requirement over a file the change produces, and a
  `command` exit-code / test-report requirement running the project's real
  check script.
- Mock scenarios: the happy-path unit(s) replay the mock's happy scenario
  (B2's mechanism); the mock's `changed_paths`/workspace effects must
  produce exactly what the real observers/verifiers then measure — the E2E
  proves instrument-verified settlement, not claim-verified.
- The reference must be INSUFFICIENT-proof too: keep it minimal but valid;
  `gauntlet reference validate` passes on it (run this to check).

Never write into `tests/fixtures/target/` at test time — the E2E copies the
fixture to a per-test tempdir and runs there (the run writes state/evidence).

## Item 2 — `e2e_full_loop_mock` test target

`crates/gauntlet-cli/tests/e2e_full_loop_mock.rs` (in-crate placement, same
D5 class as cli_surface — no frozen locator names this target). The test
drives the REAL BINARY via `env!("CARGO_BIN_EXE_gauntlet")` with cwd = the
tempdir project copy and inherited/env-set Annex-E hooks
(`GAUNTLET_CLOCK=deterministic`, `GAUNTLET_ID_SEED=<fixed>` for stability).
Legs:

1. `run "<intent>"` — exit 0; parse the printed report: every blocking unit
   of the fixture reference appears settled, `implemented-and-proven`
   populated, and the categories come from records (mutate nothing in the
   assertion strings — assert against the printed report + the on-disk
   state: pinned evidence refs exist under the state/evidence dirs and
   `gauntlet inspect`/`verify --run` legs read them back).
2. `status`, `inspect --run <id>`, `export`, `verify --run <id>` — each
   exits 0 against the settled run and prints record-derived content
   (spot-assert a couple of record-derived fields, not just exit codes).
3. `run --dry-run` inside the fixture — exits 0, zero new writes.
4. DISCRIMINATION TEETH (required): a second test in the same target using
   a lying-builder variant (mock `lying-builder` scenario selected by the
   variant's GAUNTLET.md or scenario env — B2's mechanism) must NOT settle:
   the run ends non-settled (FAIL/INCONCLUSIVE/blocked as the runtime
   honestly reports), `implemented-and-proven` stays unpopulated for the
   attacked unit. This proves the E2E can fail — an E2E that cannot is not
   a proof.

If the E2E exposes a genuine defect in the COMPOSITION (B2's files), do NOT
patch CLI source: record it precisely in b3-report.md and, if it blocks all
legs, stop after recording — the orchestrator routes it to a correction
round. Fixing the FIXTURE is yours.

## Boundaries

- Your files: `tests/fixtures/target/**`,
  `crates/gauntlet-cli/tests/e2e_full_loop_mock.rs`, plus progress/report
  files. Nothing else without recording a deviation (golden/regenerated
  files a run legitimately produces under tests/fixtures are committed
  fixtures, not artifacts — the E2E itself must not write there).
- Do not edit the snapshot `cli_surface` target (B2's, already green).
- Never edit `.wayfinder/**` (repo root), `.agents/specs/**`, CLI src, or
  any control logic; never weaken assertions.
- `just ruler` before/after. All gates stay green.

## Operational constraints (binding, see charter addenda)

- flock + `sccache --stop-server` inside the lock for every cargo/just
  call; background+poll cadence; declare-first checkpoints.

## Checkpoint protocol (mandatory)

`.agents/reports/task20/b3-progress.md`: STARTED/DONE per item with
evidence; update it FIRST if your window runs short. Finish with
`.agents/reports/task20/b3-report.md`: what landed, fixture inventory, the
E2E legs with observed outputs, teeth evidence, any composition defects
found (precise, for the correction round), deviations, anything not done.

## Done when

`cargo nextest run --test e2e_full_loop_mock` (and `--test cli_surface`)
pass; `cargo run -q -p gauntlet-cli -- run --dry-run "seeded bounded task"`
still exits 0 at the repo root; `just check/test/lint/deny/boundaries/
ruler/doctor` all green; the fixture project's reference validates through
the real parser; the discrimination tooth has been observed failing the
settlement (both tests green in the final run, with the teeth's RED state
evidenced by the lying-builder run's non-settlement, not by disabling
anything).
