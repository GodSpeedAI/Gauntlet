# Task 20 / Builder B3 final report — E2E fixture project landed; e2e_full_loop_mock BLOCKED by a composition defect

Builder claims carry no settlement authority. Every claim below is backed by a
command output under `.agents/reports/task20/b3-*.log`. B3 touched ONLY its
briefed files (`tests/fixtures/target/**`, two new scenario fixtures under
`tests/fixtures/mock_runner_scenarios/`, progress/report files) — zero edits to
CLI source, control logic, specs, the ruler, or any existing test.

## Headline

- **Item 1 (fixture target project): DONE and validated through the real
  binary.** `gauntlet doctor` exits OK on a copy; `gauntlet reference validate
  --path .wayfinder/answer-key.md` prints `OK: 3 units`; `gauntlet run --dry-run
  "seeded bounded task"` exits 0 (3 dispatches planned, lock verified, zero
  writes).
- **Item 2 (`e2e_full_loop_mock`): BLOCKED by a genuine defect in the B2
  composition, proven empirically.** With the shipped composition, NO fixture
  configuration can settle a single unit: the composed verification pipeline
  materializes an observation facet's `facet_evidence@v1` JSON at the unit's
  `evidence_required.locator`, and every composed ladder rung requires a
  different artifact at that locator — so the first completed dispatch errors
  the whole run (`ControlError::VerificationRefused`). Per the brief B3 did NOT
  patch CLI source and did NOT write a partial E2E target that would either
  fail or freeze the defective behavior as expected; B3 stopped after
  recording. The correction round can resume Item 2 from this state — the
  fixture, scenarios, env matrix, and probe logs are all in place.

## What landed (file inventory)

`tests/fixtures/target/` — the seeded E2E target project:

- `GAUNTLET.md` — front matter mirrors the repository's contract with exactly
  three fixture deltas: `runners.default: mock-agent` (charter D2; the repo's
  own file stays unset, UNK-003 untouched), `observation.facets: [filesystem]`
  (the one facet needing no external tool), `review.max_critics: 0` (see
  Finding 3), plus a bounded budget. Body documents the fixture's purpose and
  the never-write-at-test-time rule.
- `.wayfinder/reference.yaml` — strict closed manifest shape, units empty
  (no cohort key anywhere — D7 re-recorded), `settlement.minimum_reliability`
  in the parser's required `>= N settlements` prose form.
- `.wayfinder/answer-key.md` — three BLOCKING units (AK-901..903), each
  tier `contained`, authority `critic`, producer `instrument`, with kinds
  `file` / `log` / `test_report` over locators under `evidence/` (see
  DEVIATION 1 for why not `local`).
- `.agents/roles/critic.md`, `.agents/roles/builder.md` — role files the
  composition can actually load: plain tokens, NO `$VAR` indirections (the
  repo's role files fail to load outside the repo env because
  `expand_indirections` refuses unresolved variables).
- `.gauntlet/comparison-contract.yaml` — the repository's contract, verbatim.
- `.gauntlet/reference.lock` — fixture freeze record; digests produced by
  `scripts/reference-digest.sh`; consumed by the dry-run leg (`verify_lock`).
- `src/gadget.rs`, `scripts/check.sh` (executable) — the project's one source
  file and its REAL check command: exit 0 iff the gadget source carries
  `GADGET_MARKER_V1`, exit 1 otherwise (verified both ways from the shell).

`tests/fixtures/mock_runner_scenarios/` — two NEW scenario fixtures (DEVIATION 2):

- `e2e-happy.yaml` — completes after one probe; `attempted: []` so the
  controller's empty-attempted fallback processes the FLIGHT'S OWN unit (the
  shipped `happy.yaml` hard-names AK-014, which the loop records as
  `attempted_unit_mismatch` and never verifies); workspace effect writes the
  marker-carrying gadget source the reference demands.
- `e2e-lying.yaml` — the discrimination-tooth variant: claims `scripts/check.sh`
  exited 0 while the planted effect content really fails that check (modeled on
  the shipped `lying-builder.yaml`).

Evidence logs: `b3-probe-fixture-doctor-ok.log` (doctor exit OK on a copy),
`b3-build1.log`, dry-run output quoted in `b3-progress.md`.

## The E2E legs, probed by hand in scratch copies (Item 2 status per leg)

All probes: fixture copied to `/tmp/b3-probe-target`, cwd there, per-test env
(`GAUNTLET_STATE_DIR`, `GAUNTLET_ARTIFACT_DIR`, `GAUNTLET_MOCK_SCENARIO`,
`GAUNTLET_ID_SEED`, and the four REQUIRED control env vars — see ENV below).

| leg | status | evidence |
|---|---|---|
| `run` — settles every blocking unit, record-derived report, pinned evidence | **BLOCKED** (Finding 4) | b3-probe-run-verification-rung-refused.log |
| `status --run` | works, record-derived (`tier=contained units=3 settled=0`) | probe transcript in b3-progress.md |
| `inspect --run` | works (`inspect: run_id=…`) | probe transcript |
| `export --run` | works (canonical run/event JSON) | probe transcript |
| `verify --run` | **BLOCKED** for any unit whose production record exists — same rung refusal; others INCONCLUSIVE; `pass=0 … NOTHING settled` | probe transcript in b3-report |
| `run --dry-run` inside the fixture | works: exit 0, zero writes | b3-progress.md |
| discrimination tooth (lying builder must not settle) | **UNREACHABLE**: the happy leg cannot settle, so there is no settlement for the lying leg to fail to reach — both fail at the same composition defect, not at the runtime's judgment of the lie | b3-probe-run-*.log |

The `verify --run` refusal, verbatim:

```
verify: unit=AK-902 REFUSED — the verification ladder refused: the command-exit-code rung
(gauntlet-verify-command-exit-code) failed as an instrument while deciding the claim: the
transcript at evidence/check-transcript.txt is not the two-line command/exit artifact the
instrument's contract declares — selection terminates (improve the instrument; a dearer
rung cannot inherit a broken rung's claim, 14.2)
verify: run_id=RKPPJVG2B7HXKRKDQD6QM90K47 pass=0 fail=0 inconclusive=2 refused=1 —
records printed, NOTHING settled
```

## Composition defects found (for the correction round; NOT fixed by B3)

Reproduction recipe: copy `tests/fixtures/target` to a tempdir, `cd` there, set
the ENV matrix below, run `target/debug/gauntlet run "seeded bounded task"`.
Applying the workarounds one at a time walks through Findings 1–4; the logs are
the four `b3-probe-run-*.log` files in order.

- **Finding 1 (compose.rs — the Annex-E clock hook is never consulted).**
  `compose_run` hardwires `SystemClock`; `GAUNTLET_CLOCK=deterministic` (the
  hook the plan/brief names for exactly this proof) is silently a no-op. The
  mock reports `last_event_at = 1_700_000_000` (fixed Nov-2023 epoch), the
  controller adopts that as `last_progress_at`, and the wall-clock stall
  detector fires on EVERY poll: cancel → retry → escalate. Observed:
  `terminal=escalated` ("AK-901 timed out twice"), zero verifications
  (b3-probe-run-stall-escalated.log). Operator-env workaround used for the
  later probes: `GAUNTLET_STALL_TIMEOUT_MS=200000000000`. (Fail-open hook
  behavior — an env hook that changes nothing — is its own hazard; the hook
  should either work or be refused.)
- **Finding 2 (compose.rs + govern — no fronted critic runner survives §13.2
  at the mock posture).** Any `review.max_critics >= 1` errors the run at the
  first comparison: "a model critic requires process-or-stronger runner
  isolation; isolation:none cannot establish fresh-context containment"
  (b3-probe-run-critic-isolation-refused.log). The three fronted tokens are
  prime-agent/cli-agent (need external binaries the fixture host may not
  have) and mock-agent (isolation none). Workaround used: `max_critics: 0`
  (count-truncated review — honest truncation records, no dispatch).
- **Finding 4 (THE BLOCKER — the composed verification leg can never PASS,
  so the run leg's settlement assertions are unsatisfiable by ANY fixture).**
  Mechanism, from source + both probes: `controller.rs::process_completion`
  takes the FIRST observation facet's evidence bytes (a `facet_evidence@v1`
  JSON document: schema/instrument/exit_status/stdout fields), writes them at
  the unit's `evidence_required.locator` under the canonical workspace root,
  records the production binding over those bytes (three-way digest equality),
  then dispatches B2's `composed_ladder` — whose rungs expect, at that same
  locator, a two-line sha256 manifest (FileDigest rung), a two-line
  command/exit transcript (CommandExitCode rung), or a `tests`/`failures` JSON
  object (TestReport rung). A facet document satisfies none of the three
  contracts ⇒ `VerifierError::InstrumentFailed` ⇒ `LadderError::RungFailed`
  ⇒ `ControlError::VerificationRefused` ⇒ the run dies with exit 1 on the
  first completed dispatch (b3-probe-run-verification-rung-refused.log). The
  case analysis is closed:
  - requirement kind `file` / `log` / `test_report` ⇒ RungFailed ⇒ run error;
  - any other kind ⇒ `NoRungCanDecide` ⇒ run error (ladder.rs);
  - zero observation facets ⇒ the composition refuses;
  - observers configured but all failing ⇒ no facet ⇒ no production ⇒
    INCONCLUSIVE forever ⇒ non-settlement by rounds/budget, never settlement.
  Note the locus split: the locator-bytes behavior is pre-existing control
  logic (controller.rs, pinned by its own tests); the rung set is B2's
  composition choice (`composed_ladder`). The defect is the COMPOSITION of the
  two — no fixture-side artifact can ever sit at the locator, because the
  controller overwrites it with facet bytes before verification and the
  binding equality then forces the rung to read those exact bytes. Fix shapes
  for the orchestrator to adjudicate (one of): (a) the pipeline materializes a
  rung-compatible artifact at the locator (control-logic change), (b) the
  composition fronts a rung whose contract reads `facet_evidence@v1` (adapter
  work), or (c) evidence production at locators is re-owned (spec-level
  question — CandidateRevision territory).
- **Finding 3 (doctor/UX, minor, fixture-side already accommodated).** The
  four control settings (`GAUNTLET_STALL_TIMEOUT_MS`, `GAUNTLET_RETRY_BASE_MS`,
  `GAUNTLET_RETRY_MAX_MS`, `GAUNTLET_MAX_ATTEMPTS_PER_UNIT`) are REQUIRED for
  any composed run but are environment-only — `gauntlet doctor` exits OK
  without them, so the absence surfaces only as a run-time configuration
  failure. Recorded as an operator-experience residual, not a defect in this
  slice's path.

## ENV matrix a correction round needs to resume Item 2

```text
GAUNTLET_STATE_DIR=<tmp>/state        GAUNTLET_ARTIFACT_DIR=<tmp>/evidence
GAUNTLET_MOCK_SCENARIO=e2e-happy      GAUNTLET_ID_SEED=b3-e2e-seed
GAUNTLET_STALL_TIMEOUT_MS=200000000000   GAUNTLET_RETRY_BASE_MS=100
GAUNTLET_RETRY_MAX_MS=2000            GAUNTLET_MAX_ATTEMPTS_PER_UNIT=3
```

(Per-test temp values for STATE/ARTIFACT: the shipped default state dir is the
shared `/tmp/gauntlet/state`, which concurrent nextest processes would collide
on. The huge stall window is Finding 1's workaround and must be deleted once
the clock hook is wired; `DeterministicClock` rests at epoch 0, before the
mock's 1_700_000_000, so no stall is possible there.)

## DEVIATIONS (deliberate, each forced and recorded)

1. **Fixture units are tier `contained`, authority `critic` — not `local` as
   the brief spelled.** The brief's premise ("the shipped `none` sandbox
   posture admits local-tier dispatch") is contradicted by the shipped gate:
   `govern/gate.rs::admit` refuses EVERY above-contained tier under the none
   posture with `NoBackendForTier` (its own tests pin Local+CrossCutting
   refusals), and `none_backend()` is critic-exercisable. A `local` fixture
   unit would never dispatch — it would escalate at the gate. Choosing
   `contained`/`critic` keeps the two constraints that CAN co-hold (shipped
   none posture + dispatchable units); the brief's tier spelling could not be
   honored without violating its own "sandbox backend left at the shipped
   `none` default" constraint.
2. **Two files outside `tests/fixtures/target/**`:**
   `tests/fixtures/mock_runner_scenarios/e2e-happy.yaml` and `e2e-lying.yaml`.
   The mock seam (`load_scenario`) reads ONLY that directory, so the E2E's
   scenario fixtures must live there; the shipped `happy.yaml` cannot serve
   (its `attempted: [AK-014]` is mismatch-skipped for every fixture unit —
   the loop would verify nothing). Additive only; no existing fixture touched;
   no test enumerates that directory.
3. **No `crates/gauntlet-cli/tests/e2e_full_loop_mock.rs` was created.** Per
   the brief's blocked path: the defect above blocks the run leg and therefore
   every settlement-dependent leg including the tooth. A target written now
   would either fail (red battery) or have to assert the defective behavior
   as expected — freezing the defect against the correction round and faking
   the proof the name claims. The brief's stop-after-recording branch is the
   honest terminal state for this slice.
4. **One line added to `.gitignore`** (outside the briefed file list, so
   recorded): `!tests/fixtures/target/` with a comment, directly under the
   `**/target/` rule. Without it the ENTIRE fixture directory is silently
   git-ignored (`**/target/` is the Rust build-target rule and matches any
   directory named `target/`) — the fixture could never have been committed
   and the slice would have landed empty. The negation is scoped to exactly
   this directory; verified with `git check-ignore` (now clean) and
   `git status` (fixture tree now visible as untracked).

## Gate battery (final tree, required order)

| gate | result | evidence |
|---|---|---|
| `just ruler` (before work) | PASS 0 | b3-gates-fast.log (RULER=0) |
| `just ruler` (after work) | PASS 0 | b3-gates-fast.log (RULER=0) |
| `just doctor` | PASS 0 (exit OK) | b3-gates-fast.log (DOCTOR=0) |
| `just check` | PASS 0 | b3-gates-fast.log (CHECK=0) |
| repo-root dry-run gate | exit 0, preview printed, zero writes | b3-dryrun-root.log (DRYRUN_ROOT=0) |
| `just boundaries` | PASS 0 — 228/228 | b3-gate-boundaries.log |
| `just deny` | PASS 0 | b3-gate-deny.log |
| `just lint` | PASS 0 — clippy -D warnings + fmt | b3-gate-lint.log |
| `just test` | **PASS — 1319 tests run: 1319 passed** | b3-gate-test2.log |
| fixture doctor (scratch copy) | exit OK | b3-probe-fixture-doctor-ok.log |
| fixture `reference validate` | `OK: 3 units` | probe transcript |
| fixture dry-run (scratch copy) | exit 0, lock verified | probe transcript |

Known-class recurrence note: the FIRST full battery run (b3-gate-test.log)
failed 3 of 1319 — `f_collect_serves_the_poll_snapshot_not_the_file`,
`c_two_invocations_no_cross_wiring`, `d_env_and_prompt_channels_and_envelope_
sibling_are_exact` (all live-process tests in gauntlet-adapter-agent-cli).
In isolation: 56/56 PASS (b3-gate-agentcli-iso.log). Full re-run: 1319/1319
PASS. This widens the charter's documented load-flaky class from one test to
three in the same live-attacks file; not weakened, not "fixed", recurrence
recorded here per the charter addendum.

Operational: every cargo/just call ran under
`flock .agents/reports/task20/compile.lock` with `sccache --stop-server` inside
the lock (the stop itself errors when no server is running — cosmetic; the
gate exit codes above are read from the echoed markers, never from the wrapper).

## Deliberately not done

- The E2E test target itself (DEVIATION 3) — blocked, recorded, resumable.
- Any change to CLI source, control logic, specs, ruler, snapshots, or
  existing tests — outside B3's boundary; the findings are routed, not patched.
- Commits — orchestrator's. B3's files are uncommitted in the working tree.
