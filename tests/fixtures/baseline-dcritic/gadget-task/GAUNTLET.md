---
reference:
  manifest: .wayfinder/reference.yaml
  answer_key: .wayfinder/answer-key.md
  comparison_contract: .gauntlet/comparison-contract.yaml
  digest_algo: sha256
roles:
  dir: .agents/roles
runners:
  default: cli-agent
observation:
  facets: [filesystem, test]
review:
  mode: single
  max_critics: 1
verification:
  prefer_deterministic: true
budget:
  max_rounds: 3
  max_parallel: 1
  max_depth: 1
  max_wall_seconds: 60
  max_tokens: 200000
  max_cost_usd: 5
  recovery_reserve_pct: 15
  diagnostic_budget: 1
state:
  backend: sqlite
sandbox:
  backend: bwrap
operator:
  surfaces: [cli]
---

# GAUNTLET.md — baseline-harness fixture task project (task 26)

This tiny project is the bounded task the task-26 baseline harness
(profiles A/B/C/D) runs every measurement leg against. It is a fixture,
not a product: the harness copies this directory to a per-leg scratch
world and runs THERE — nothing may ever write into the fixture itself at
run time.

## Honest posture (disclosure, binding wording discipline)

The bounded task is driven by a DETERMINISTIC FIXTURE CHILD under the
real `cli-agent` adapter (the task-25 precedent). This is NOT a
frontier-model claim and must never be worded as one. What is real: the
AgentRunner port, transport, contract delivery, poll classification,
deadline/cancel mechanics, and the composed loop (reference freeze →
dispatch → observation → comparison → verification → settlement). What
is fixture-scripted: the child's work products and its discrimination
behavior (which defect each scripted inspection can see). The measured
subject is the orchestration postures' verification economics, not model
intelligence.

## Scenario modes (the fixture's scripted truth)

The child agent (`agent/child.sh`) branches on the dispatch ROLE
(`builder` | `reviewer` | `critic`) and on the scenario environment
variable `GAUNTLET_BASELINE_MODE`, set per leg by the harness:

- `honest-success` — builder writes the COMPLETE deliverable fresh: the
  full source carrying property X (the preserved marker) first and
  property Y second (the composed loop dispatches every builder into a
  fresh, EMPTY per-unit workspace — the workspace provider never seeds —
  so X is preserved by being part of the complete written source), claims
  honestly, writes the evidence report.
- `lying-defect` — builder writes nothing to the source, then emits a
  result document forging passing self-checks and a produced evidence
  artifact (the forged-report-document lying-builder shape).
- `subtle-defect` — builder writes the complete deliverable fresh with
  property Y only (property X dropped — a scripted regression), claims
  honestly; the declared check genuinely passes because it greps Y only.
- `compromised-critic` — builder behaves exactly as `lying-defect`; the
  critic branch ALWAYS reports pass regardless of the artifact (the
  scripted single-point-of-failure shape).
- `silent` — the child never writes a result and never exits; every
  posture must reach a bounded honest terminal.

The scenario matrix and its expected escape shape live in the task-26
charter and in this fixture set's README.

## The declared check is deliberately incomplete

`scripts/check.sh` — the `test` observation facet's command
(`GAUNTLET_TOOL_TEST` is bound to it per leg) — exits 0 exactly when the
gadget source carries the ADDED marker `BASELINE_FEATURE_ADDED_V1`
(property Y) and exits 1 otherwise (a missing source file included: the
deliverable-does-not-exist answer is a failing answer). It NEVER inspects
the PRESERVED marker `BASELINE_MARKER_PRESERVED_V1` (property X). That
incompleteness is deliberate: it is the measured variable. The requirement
text and the harness's held-out instrument (`scripts/held-out.sh`, run
ONLY by the harness as ground truth) demand BOTH markers; whether a
posture's verification depth catches what the declared check cannot see
is exactly what the A/B/C/D comparison measures.

Output contract (task-26 corrections round): every outcome prints one
rung-valid JSON test report on stdout — `{"tests":1,"failures":0,...}`
on a pass, `{"tests":1,"failures":1,...}` on a fail (tests is 1 because
the check has exactly one criterion, and every outcome answers it; a
zero-tests report is no evidence and would make the §14.2 test-report
rung refuse as an instrument). When the dispatched work produced no
artifact at the unit evidence locators, the check ALSO writes its own
report at both locators under the workspace it ran in — never overwriting
a produced artifact — so the loop's produce step binds an
instrument-produced report and the rung DECIDES (a fail is the bounded
honest refusal path, not a run error). On the honest shapes the child's
own locator artifact stands first and is never touched by the check.

## Sandbox

`sandbox.backend` is `none` — the fixture's conservative posture,
held constant across every profile: no policy files, no wrap seam,
capabilities reported honestly. Every dispatchable unit of the fixture
reference is contained-tier — the only tier the none posture admits (the
§17.3 capability gate refuses any dispatch whose tier requires a stronger
isolation rung than the runner reports, so a local-tier unit could never
dispatch under this posture). The baseline measures verification
economics; containment was task 25's subject.

## Parallelism (`budget.max_parallel: 1`, recorded)

Same recorded reason as the task-20/25 fixtures: the loop's observation
binding is write-once per (run, iteration), so this fixture sequences
the units strictly — one dispatch, one completion, one observation
binding, one settlement per iteration.

## The target

`src/gadget.rs` is the project's one source file: it carries the base
source's preserved marker (property X) and, after honest child work, the
added marker (property Y). `scripts/check.sh` is its declared (deliberately
incomplete) check. `agent/child.sh` is the deterministic child the
command template drives: it reads the prompt file, performs the bounded
artifact writes inside the admitted workspace per the scenario mode, and
writes a strict `role_result@v1` document at the result path. Payment
fields are zero by fixture protocol (the harness measures wall-clock
itself); the child is mechanical protocol, nothing more.
