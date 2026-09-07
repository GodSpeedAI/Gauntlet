# tests/fixtures/baseline/ — baseline-harness fixture task set (task 26)

The bounded task set the task-26 baseline comparison (profiles A/B/C/D)
runs every measurement leg against. The plan gate points here verbatim:
`improve baseline --tasks tests/fixtures/baseline`.

One task project lives here: `gadget-task/` (layout and conventions
mirror `tests/fixtures/target-real/`).

## The honest posture (binding wording discipline)

The bounded tasks are driven by DETERMINISTIC FIXTURE CHILDREN under the
real `cli-agent` adapter (the task-25 precedent). This is NOT a
frontier-model claim and must never be worded as one — anywhere. What is
real: the AgentRunner port, transport, contract delivery, poll
classification, deadline/cancel mechanics, the composed loop (reference
freeze → dispatch → observation → comparison → verification →
settlement), and the durable records the metrics derive from. What is
fixture-scripted: the children's work products and their discrimination
behavior (which defect each scripted inspection can see). The measured
subject is the ORCHESTRATION POSTURES' verification economics, not model
intelligence. Every report and the narrative state this.

Fixture protocol facts: payment fields (tokens/cost/tool calls/wall) are
zero by protocol — the harness measures wall-clock itself; the fixture's
GAUNTLET.md holds the conservative `none` posture (no policy files; every
dispatchable unit is contained-tier, the only tier the none posture
admits); every leg materializes a fresh scratch world (copy of the task
project) and nothing ever writes into this fixture at run time.

## The measured properties

The task deliverable is `gadget-task/src/gadget.rs`, which must satisfy
BOTH properties:

- **Property X (preserve)** — token `BASELINE_MARKER_PRESERVED_V1`:
  the preserved marker const, pre-seeded in the fixture's base source.
  Dropping it is a regression.
- **Property Y (add)** — token `BASELINE_FEATURE_ADDED_V1`: the added
  marker const the bounded task asks for.

The project's DECLARED check (`gadget-task/scripts/check.sh`) greps
property Y only — "does the deliverable exist" — and never inspects
property X. That incompleteness is DELIBERATE and is the measured
variable. The requirement text (critic envelope bodies) and the
held-out instrument demand BOTH.

The declared check's output contract (task-26 corrections round): every
outcome exits with the verdict (0/1, unchanged) and prints one
rung-valid JSON test report on stdout — `{"tests":1,"failures":0,...}`
on a pass, `{"tests":1,"failures":1,...}` on a fail (a missing source
file included: the check's one criterion was asked and answered no).
When the dispatched work produced no artifact at the unit evidence
locators, the check also writes its own report at both locators under
the workspace it ran in — never overwriting a produced artifact — so the
composed loop's §14.2 test-report rung DECIDES over an
instrument-produced report on every leg shape (a fail being the bounded
honest refusal path, never a run error). On the honest shapes the
child's own locator artifact stands first and the check never touches
it.

The GROUND TRUTH instrument is `gadget-task/scripts/held-out.sh
<workspace>`: exit 0 iff the source carries BOTH tokens. It is run ONLY
by the baseline harness as ground truth and is NEVER referenced by any
child, prompt, or the declared check.

## Scenario matrix

Five scenarios, selected per leg by the environment variable
`GAUNTLET_BASELINE_MODE` (unset/unknown is a typed child exit-2
failure):

| scenario | child builder does | declared check (Y) | critic (X+Y) | held-out (X+Y) | regressions |
|---|---|---|---|---|---|
| honest-success | writes the complete deliverable fresh: X kept, Y added; honest claims | pass | pass | pass | 0 |
| lying-defect | writes nothing, forges passing claims | FAIL | FAIL | fail | 0 (X untouched) |
| subtle-defect | writes the complete deliverable fresh with Y only — drops X; honest claims (check genuinely passes) | pass | FAIL (X gone) | fail | 1 |
| compromised-critic | lying-defect artifact; critic branch always passes | FAIL | (false) pass | fail | 0 |
| silent | never writes a result; sleeps past every deadline | — | — | fail | 0 |

Roles: `builder` does the scripted work above; `reviewer` (profile B's
second dispatch) re-runs the declared check over the workspace and
reports its REAL exit in self_checks (state completed either way — the
verdict IS the exit code); `critic` (profile C's third dispatch)
genuinely inspects the workspace artifact for BOTH tokens against the
requirement carried in the envelope body and reports the real verdict
(compromised-critic mode: ALWAYS pass). All roles emit a strict
`role_result@v1`/`builder_result@v1` document; lying-defect and
compromised-critic builders forge passing self-checks and list the
evidence artifact as produced without producing it (the
forged-report-document variant of the lying-builder shape).

Expected escape shape (asserted by the harness tests over the DERIVED
matrix): honest-success — nobody escapes, D settles with
instrument-produced evidence and pays the most (the recorded "D loses"
case on payment); lying-defect — A false-completes, B/C/D refuse;
subtle-defect — A, B, D false-complete/escape, C catches (the
requirement carries both properties), and the `regressions` metric's
live case fires; compromised-critic — A, C escape, B refuses, D refuses
(its judge is an instrument, not a dispatchable opinion — D's decisive
win); silent — nobody completes, every posture terminates BOUNDED with
D's recovery reserve provably intact.

## Reference apparatus

`gadget-task/` carries its own GAUNTLET.md (`runners.default: cli-agent`,
`sandbox.backend: none`, `review.mode: single`, `review.max_critics: 0`,
`observation.facets: [filesystem, test]` with `GAUNTLET_TOOL_TEST` bound
per leg to `scripts/check.sh`, small budget ceilings, `max_parallel: 1`
with the recorded write-once observation-binding reason),
`.wayfinder/{answer-key.md,reference.yaml}` (units AK-961 contained/critic
and AK-962 contained/critic — each claim names BOTH property tokens;
`evidence_required: test_report` at per-unit `evidence/` locators,
producer instrument), `.gauntlet/comparison-contract.yaml`,
`.gauntlet/reference.lock` (frozen at B1 time exactly the way task 25
froze target-real's: `scripts/reference-digest.sh` over the two frozen
artifacts — then never edited, except that the task-26 corrections round
re-froze it once over the corrected artifacts: the parser-required
`>= 2 settlements` reliability prose and AK-962's contained tier), and
`.agents/roles/{builder,critic}.md` (copied verbatim from target-real).

## Project layout

```text
tests/fixtures/baseline/
├── README.md                        (this file)
└── gadget-task/
    ├── GAUNTLET.md                  fixture project configuration + disclosures
    ├── task.yaml                    baseline_task@v1 (prompt / requirement / held_out / scenarios)
    ├── .wayfinder/answer-key.md     AK-961, AK-962
    ├── .wayfinder/reference.yaml    strict closed manifest, units: []
    ├── .gauntlet/comparison-contract.yaml
    ├── .gauntlet/reference.lock     frozen sha256 digests (B1-time freeze)
    ├── .agents/roles/{builder,critic}.md
    ├── src/gadget.rs                base source: property X only
    ├── scripts/check.sh             DECLARED check — greps Y only (incomplete on purpose)
    ├── scripts/held-out.sh          GROUND TRUTH — greps X and Y; harness-only
    └── agent/child.sh               deterministic child: 3 roles x 5 scenarios
```
