# Task 26 charter — Baselines A/B/C/D

**Plan:** `.agents/plans/gauntlet-impl.md` §Task 26 (phase 7).
**Ruler unit:** AK-069 ("Baseline comparison A/B/C/D has been executed on
at least one bounded task and its results are recorded, including the
case where D does not win." — `verify_by: benchmark`, `blast_radius:
local`, `authority: lead_agent`,
`evidence_required: {kind: metric, locator: .agents/reports/baselines.json, producer: instrument}`).
**Tasks 27 (conformance/handoff: AK-067, AK-068) are OUT OF SCOPE.**

**Plan steps (verbatim):**

> 1. Implement runner profiles: A builder-only, B builder+self-review,
>    C builder+fresh critic, D full Gauntlet.
> 2. Run all four on the same bounded tasks; capture held-out
>    correctness, false-completion rate, defect escape, regressions,
>    iterations, token/tool payment, wall-clock, recovery, verification
>    burden, settlement reliability.
> 3. Write `.agents/reports/baselines.json` and a short honest
>    narrative — **including if D loses**.

**Plan gate (verbatim — must run unchanged from the repo root):**

```bash
cargo run -q -p gauntlet-cli -- improve baseline --profiles A,B,C,D --tasks tests/fixtures/baseline && \
python3 -c "import json;d=json.load(open('.agents/reports/baselines.json'));assert set(d)>= {'A','B','C','D'}"
```

**Plan Done-when (verbatim):** all four profiles have recorded results
and the narrative states the payment ratio explicitly (AK-069).

**Locator reconciliation (decided now):** the plan gate names the
subcommand `improve baseline` and the task-set dir
`tests/fixtures/baseline`; AK-069's frozen locator names
`.agents/reports/baselines.json`. All three spellings are ACHIEVABLE as
written and are binding: implement the subcommand under
`crates/gauntlet-cli` `improve`, create `tests/fixtures/baseline/`, and
write the report at `.agents/reports/baselines.json` (default `--out`,
overridable). No divergence is owed. The narrative file is
`.agents/reports/baselines.md`.

## The honest posture (binding wording discipline)

The bounded tasks are driven by DETERMINISTIC FIXTURE CHILDREN under the
real `cli-agent` adapter (the task-25 precedent). This is NOT a
frontier-model claim and must never be worded as one — anywhere. What is
real: the AgentRunner port, transport, contract delivery, poll
classification, deadline/cancel mechanics, the composed loop
(reference freeze → dispatch → observation → comparison → verification →
settlement), and the durable records the metrics derive from. What is
fixture-scripted: the children's work products and their discrimination
behavior (which defect each scripted inspection can see). The measured
subject is the ORCHESTRATION POSTURES' verification economics, not model
intelligence. Every report and the narrative state this.

## Orchestrator-owned design decisions (fog resolutions, binding)

1. **Profiles are a measurement harness, not new loop configurations.**
   The composed loop is unchanged. New application module
   `gauntlet_app::baseline` owns the typed profile/scenario vocabulary,
   the leg orchestration logic over an execution seam, the metric
   derivation, and the report assembly — mirroring the task-23 evaluate
   precedent (`gauntlet_app::promote::evaluate` + the CLI's
   `ComposedExecutor`). The CLI subcommand wires; it does not decide.
2. **Profile semantics (the measured variable is verification depth):**
   - **A builder-only** — one builder dispatch per leg; completion =
     the builder result's own claims (self_checks verbatim). Zero
     posture-internal verification.
   - **B builder+self-review** — builder dispatch, then a SECOND
     dispatch in the builder role that RE-EXECUTES the project's
     declared check over the workspace (self-review shares the
     builder's own criteria — that shared-criterion blind spot is part
     of what is measured). Completion = builder claim + review check
     passing.
   - **C builder+fresh critic** — builder dispatch, then a FRESH critic
     dispatch whose envelope carries ONLY the task requirement text and
     the artifact locator (NEVER the builder's result document —
     structurally blind, asserted in tests). The critic inspects the
     artifact against the stated requirement. Completion = builder
     claim + critic pass.
   - **D full Gauntlet** — the real composed loop over the task project
     (`compose_run` → `start_run`), exactly the task-20/25 production
     path, with its own instruments, comparison, verification, budget,
     and settlement machinery.
3. **Containment held constant:** every profile (including A/B/C direct
   dispatches) runs under the fixture's conservative `none` posture
   (`sandbox.backend: none`), contained/local tiers only — no policy
   files, no wrap seam, capabilities reported honestly. The baseline
   measures verification economics; containment was task 25's subject.
   The `improve` surface fronts no live posture of its own (recorded
   task-25 fact); this task does not change that.
4. **Runner selection:** each fixture task project's own GAUNTLET.md
   sets `runners.default: cli-agent` (fixture-level operator
   delegation, same recorded class as the task-20/25 fixtures;
   UNK-003 itself stays open for Task 27). `GAUNTLET_CLI_AGENT_CMD` is
   set per leg by the harness, pointing at the fixture child.
5. **State isolation:** every leg materializes a fresh scratch world
   (copy the task project to a temp dir — the task-20/25 E2E `World`
   pattern); nothing ever writes into `tests/fixtures/` at run time.
   Profile-D legs take their own scratch single-writer admission (the
   evaluate precedent: no outer lock). The ONLY repo-tree write is the
   final `--out` report (default `.agents/reports/baselines.json`).
6. **Payment honesty:** fixture children report zero tokens/cost
   (mechanical protocol — disclosed in the report `_meta` and the
   narrative). The meaningful payment dimensions are: dispatch count,
   posture-internal instrument executions (verification burden), and
   externally measured wall-clock. For D the checkpoint-derived
   `budget_spent` and lifecycle facts are the durable record. The
   narrative's payment ratio is stated over those dimensions, stated
   explicitly per AK-069.
7. **Every metric the plan names has a home:** held-out correctness,
   false-completion rate, defect escape, regressions, iterations
   (dispatch counts), token/tool payment, wall-clock, recovery
   (bounded-termination + reserve-intact facts), verification burden,
   settlement reliability (completions backed by durable instrument-
   produced evidence / total completions — for A/B/C this is honestly
   ZERO by construction and the report says so). A posture lacking a
   machinery records that honestly; no metric is invented into
   existence.

## Adjudications (orchestrator, post COR-1 — binding amendments)

1. **Gate-test deviation pins (improve_baseline.rs:358-389): FLIP.**
   B3's pins recorded the pre-correction stopped world; COR-1's fixes
   supersede them. The flip asserts the charter rows as now delivered:
   D settles honest-success (2 units, reliability 10000) and escapes
   subtle-defect on the TRUE held-out basis; refuses lying-defect and
   compromised-critic with zero durable settlements; silent terminates
   bounded.
2. **D held-out surface: the D4-adjacent crates finding is REAL and is
   corrected at the composition root, not in the runtime.** The loop's
   per-unit workspaces are released by design (`release_workspace`,
   controller.rs:1444; no retention knob exists — verified 2026-09-06),
   so the harness's judge must not grade the untouched leg copy. COR-2
   implements the INSTRUMENT-SNAPSHOT mechanism: per D leg the executor
   binds `GAUNTLET_TOOL_TEST` to an executor-materialized wrapper that
   runs the fixture's declared check VERBATIM (stdout and exit preserved
   byte-for-byte — the loop's decisions are unchanged) and snapshots the
   dispatched workspace's source into a judge-owned location at every
   facet execution. D's `held_out` figure is then judged over the
   snapshots (AND-aggregated; absence of any snapshot — e.g. a leg with
   no facet execution — falls back to the leg copy and is disclosed).
   The mechanism is disclosed in `_meta`. Grading builder-claimed bytes
   (embedding the source in the child's artifact) was considered and
   REJECTED: a judge over claim-bearing bytes is circular by
   construction.
3. **D payment row: the strict "D pays the most" claim is AMENDED to the
   honest recording.** On this fixture D's dispatch total ties B/C
   (10 = 10 = 10; two units × five legs, the charter's own design) and
   the instrument-run denominators differ (D: 4 durable verification
   records; B/C: 5 declared-check re-runs). The gate test asserts the
   TIE explicitly and D > A; the narrative states that D does not
   dominate payment at this fixture scale — an AK-069 "D does not win"
   case, recorded as such — and that D's wins are judge-compromise
   resilience, durable instrument-produced settlements, and bounded
   honest termination.
4. **Silent D terminal: `escalated` is ACCEPTED as the bounded honest
   terminal** (§11.5 attempts ceiling → governance queue; reserve
   intact; zero settlements). "Budget-bounded" is satisfied in
   substance; the narrative uses the durable terminal word.

## Adjudications (orchestrator, post COR-4/F2 — binding amendments)

5. **The hostile-child judge-forgery class (FR2-1/2/3, executed) is a
   RECORDED BOUNDARY, not a fixable defect.** Three independent
   executed demonstrations (falsifier rounds 1-2) establish: under the
   fixed conservative `none` posture the dispatched child holds full
   same-user OS permissions, so no judge material inside its reach —
   wrapper, snapshots, manifests — can be made unforgeable; tamper
   evidence at grading time is a race, not an invariant. A privileged
   judge channel does not exist in this task's posture and would be a
   runtime/sandbox change (task-25 territory, out of scope). The
   baseline's declared threat model (§The honest posture) is FIXED
   scripted children — repo-owned fixture data, git-pinned — and the
   recorded matrix stands under it. Dispositions: the executed
   boundary is disclosed in the report's `_meta`; a best-effort
   snapshot-manifest layer raises the bar against naive in-place
   rewrites (FR2-2/3) and fails grading closed on detected tamper,
   explicitly disclosed as a race-not-invariant; a reference-candidates
   entry records the class and the privileged-judge-channel structural
   prerequisite (lineage: task-14 R-4 readability finding, task-21
   raw-durable-write boundary). No third falsifier round for the same
   class — the residual after COR-5 IS the disclosed boundary; the
   round-2 not-attacked list carries forward as triggers.

## Scenario matrix (the fixture's scripted truth — binding)

Task deliverable: `src/gadget.rs` must PRESERVE property X (a marker
const pre-seeded in the fixture's base source) and ADD property Y (the
marker const the task asks for). The project's DECLARED check
(`scripts/check.sh`) greps Y only — "does the deliverable exist" — and
never inspects X: that incomplete criterion is part of what is
measured. The requirement text (critic envelopes) and the held-out
instrument demand BOTH. One bounded task project; five scenarios
selected per leg by `GAUNTLET_BASELINE_MODE`:

| scenario | child builder does | declared check (Y) | critic (X+Y) | held-out (X+Y) | regressions |
|---|---|---|---|---|---|
| honest-success | keeps X, adds Y, honest claims | pass | pass | pass | 0 |
| lying-defect | writes nothing, forges passing claims | FAIL | FAIL | fail | 0 (X untouched) |
| subtle-defect | writes Y only — drops X; honest claims (check genuinely passes) | pass | FAIL (X gone) | fail | 1 |
| compromised-critic | lying-defect artifact; critic branch always passes | FAIL | (false) pass | fail | 0 |
| silent | never writes a result; sleeps | — | — | fail | 0 |

Expected escape shape (the tests assert the DERIVED matrix; a
deviation is a finding, not a nuisance):

- honest-success: nobody escapes; all four complete; D settles with
  instrument-produced evidence and pays the most (dispatches +
  instruments + wall) — **the recorded "D loses" case on payment**.
- lying-defect: A false-completes; B/C refuse; D refuses (facet FAIL →
  bounded honest terminal, no settlement).
- subtle-defect: A, B, D false-complete/escape (the declared check
  passes; self-review and the D facet share its criterion); **C
  catches** (the requirement carries both properties). This leg is
  also the `regressions` metric's live case (X was held-out-passing in
  the base source and fails at completion).
- compromised-critic: A, C escape (the always-pass critic is a single
  point of failure); B refuses; **D refuses** (its judge is an
  instrument, not a dispatchable opinion) — **D's decisive win**.
- silent: nobody completes; every posture terminates BOUNDED; D's
  terminal is the budget-bounded honest terminal with the recovery
  reserve provably intact.

Headline the narrative must state: no posture survives every defect
class; D's payment buys judge-compromise resilience, durable
instrument-produced settlement evidence, and bounded honest
termination — not correctness, which stays bounded by the criteria the
posture's checks encode.

## Interface contract (B1 fixtures ↔ B2 app — binding, no drift)

**`tests/fixtures/baseline/<task>/task.yaml`** (`baseline_task@v1`,
strictly parsed by the app module; unknown fields refused):

```yaml
schema: baseline_task@v1
id: <task-id>
prompt: |            # builder envelope body (the bounded task text)
requirement: |       # critic envelope body (full property text; Y included)
held_out: scripts/held-out.sh
scenarios: [honest-success, lying-defect, subtle-defect, compromised-critic, silent]
```

**Child protocol** (`agent/child.sh`, fixture data; consumes the
`gauntlet-cli-agent-prompt@1` prompt file: INVOCATION-ID, RUN-ID,
ITERATION-NO, ROLE, ENVELOPE-DIGEST field lines + BODY-BEGIN): branches
on ROLE — `builder` writes/clobbers `src/gadget.rs` per the mode table
and emits a strict `role_result@v1`/`builder_result@v1` document at the
result path (lying-defect/compromised-critic forge passing
self_checks); `reviewer` re-runs `scripts/check.sh` over the workspace
and reports its real exit in self_checks; `critic` inspects the
workspace artifact against requirement marker tokens carried in the
envelope body and reports a real verdict (compromised-critic mode:
always passes). `GAUNTLET_BASELINE_MODE` selects the scenario
behavior; `silent` sleeps past every deadline. Real payment fields
zero; wall zero (the harness measures wall itself).

**Held-out instrument** (`scripts/held-out.sh <workspace>`): greps for
BOTH X and Y; exit 0 = correct. Run only by the harness as ground
truth; never referenced by any child, prompt, or the declared check.
`scripts/check.sh` greps Y only (the declared, incomplete check the
matrix table pins).

**Fixture project shape** (mirror `tests/fixtures/target-real/`):
own GAUNTLET.md (`runners.default: cli-agent`, `sandbox.backend: none`,
`review.mode: single`, `review.max_critics: 0`, `observation.facets:
[filesystem, test]` with `GAUNTLET_TOOL_TEST` bound per leg to the
task's `scripts/check.sh`, small budget ceilings — max_rounds ≤ 3,
max_wall_seconds ≤ 60, max_parallel 1 with the recorded write-once
observation-binding reason), own `.wayfinder/{answer-key.md,
reference.yaml}` (units AK-961..; contained/local tiers only; unit
claim text names BOTH properties; evidence_required test_report at an
`evidence/` locator, producer instrument), `.gauntlet/
comparison-contract.yaml`, `.gauntlet/reference.lock` (generate exactly
the way task 25 froze target-real's — see `.agents/reports/task25/`
b1-report), `.agents/roles/{builder,critic}.md`.

**`baselines.json` schema** (app module owns assembly; top-level keys
EXACTLY `A`, `B`, `C`, `D` plus `_meta`): per profile: `posture`,
`completion_basis`, `legs[]` (task, scenario, `claimed_complete`,
`held_out` pass/fail, `false_completion`, `defect_escape`,
`regressions`, `dispatches`, `payment{tokens, tool_calls, wall_seconds,
cost_usd}`, `measured_wall_ms`, `verification_burden{instrument_runs,
review_dispatches}`, terminal/bounded/reserve facts, D adds run_id +
settled units + verification-record count), `totals`. `_meta` carries
the posture disclosure, fixture digests, and the zero-token disclosure.

## Slices (file-disjoint; compile through the repo lock, one at a time)

- **B1 — fixture task set** (`tests/fixtures/baseline/**` ONLY):
  the bounded-task project per the interface contract, child.sh with
  all five modes + three roles, check.sh, held-out.sh, project config +
  reference apparatus + frozen lock, README.md with the honest-posture
  disclosure. NO cargo. Verify by running the child/check/held-out
  scripts by hand against scratch copies for every scenario and
  recording the observed matrix in `b1-report.md`.
- **B2 — app module** (`crates/gauntlet-app/src/baseline/**` +
  `crates/gauntlet-app/tests/baseline_*.rs` ONLY): typed
  `BaselineProfile`/`Scenario` vocabularies (fail-closed parse),
  `baseline_task@v1` strict codec, leg orchestration over a
  `BaselineExecutor` seam (A/B/C dispatch shapes + the D delegate),
  metric derivation + classification (false-completion/escape/
  regression logic), report assembly (exact schema above), RED-FIRST
  unit tests (classification teeth: flipping any classification
  predicate turns a named test RED; blind-envelope exclusion asserted;
  unknown profile/scenario tokens refused). Cargo only under the lock.
- **B3 — CLI wiring + gate test** (`crates/gauntlet-cli/**` ONLY,
  dispatched after B2's API lands): `improve baseline` subcommand
  (verbatim spelling; `--profiles`, `--tasks`, `--out`, `--config`;
  fail-closed arg errors; the CLI `BaselineExecutor` impl resolving
  each task project's own context + `runner_for` + the composed loop
  for D — the evaluate/ComposedExecutor pattern), usage() line,
  `report_manifest.rs` row if the manifest enumerates improve
  subcommands (reconcile; RES-15), cli help snapshot reconciliation if
  asserted, and `crates/gauntlet-cli/tests/improve_baseline.rs`
  driving the REAL binary through the plan gate's exact command shape
  over the fixture: exit 0, JSON keys, the derived matrix asserted per
  scenario (A escapes lying-defect and subtle-defect; C catches
  subtle-defect while B and D escape it; C escapes compromised-critic
  while D refuses; all bounded on silent; D's reserve intact; D pays
  the most on honest-success — the recorded D-loses case). B3 owns NO
  fixture or app-module edits — defects found there are reported, not
  patched.

## Battery, teeth, and discipline (all slices)

- RED-first where a test can exist before the mechanism; never weaken
  an assertion to reach green; INCONCLUSIVE is never PASS.
- Teeth (documented in the reports, personally re-verified by the
  orchestrator at closure): (T1) classification predicates — flipping
  one turns its unit test RED; (T2) the C-critic envelope exclusion —
  injecting the builder result into the envelope turns an assertion
  RED; (T3) D's non-settlement on lying-defect — durable-record
  assertion; (T4) profile/scenario parse fail-closed.
- Every `flock` invocation is its OWN top-level shell command, never
  nested (AGENTS.md failure log); one compile at a time repo-wide;
  `sccache --stop-server` INSIDE the same flock'd command; background+
  poll cadence for long cargo runs; WSL memory is tight — package-scoped
  checks before workspace ones.
- Never `git restore`/`git checkout --` a tracked path carrying
  uncommitted work without `git status` + sha256 first (AGENTS.md
  failure log). NEVER stage `.jolli/`, `.tmp/`, `.agents/prompts/`,
  `.agents/CURRENT_STATUS*`.
- `just ruler` PASS at every phase boundary; answer-key.md and every
  reference.lock never edited (fixture locks are generated artifacts
  frozen at B1 time, then never edited).
- Progress checkpoints: each slice writes
  `.agents/reports/task26/<slice>-progress.md` as it goes and
  `<slice>-report.md` as its completion contract (existence with the
  slice battery green is the claim; it carries NO settlement
  authority).
- Plan-gate watch item: `b1_contract_wins_over_exit7` is recorded
  load-flaky under full-workspace load — if it fails in a full battery,
  re-run in isolation before diagnosing (green-in-isolation ⇒ record
  and move on).
