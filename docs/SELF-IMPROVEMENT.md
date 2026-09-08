# Self-improvement

Spec §19 (memory, capability, governed self-improvement) and §23.3
(baselines) are normative. Per-unit conformance truth lives in
`.agents/reports/conformance.json` and `.agents/reports/handoff.md`.

## Two separate stores (spec §19.1)

| Store | Contents | Write policy |
|---|---|---|
| **Developmental memory** | every attempt, failure, correction, comparison, payment, settlement, recovery — raw | immediate, append-only |
| **Capability ledger** | promoted skills, shortcuts, routes, prompts, model-routing rules | governed promotion only |

Tool access is not capability. One successful run is not capability.
Capability is repeated reliable settlement under relevant variation, with
recovery and decreasing orchestration burden.

## Promotion criteria (spec §19.2)

Promote only with: at least `GAUNTLET_PROMOTION_MIN_SETTLEMENTS`
(default 3) settlements across recorded variation axes; at least one
rooted recovery event handled; non-increasing failure rate over substrate
chronology; **no gate erosion** (the settlements did not get easier
because verification grew weaker); stable or decreasing orchestration
burden. All five conditions are typed and adjudicated over durable records
(`gauntlet_app::memory::promotion`, task 22).

**A faster workflow that became faster by skipping verification is a
regression, not learning.** This is the single most important sentence in
this document.

## The optimizer and the gate (spec §19.3–19.4)

The process optimizer (`gauntlet improve propose|evaluate|promote|rollback`)
reads run evidence and produces **proposals only**. It may not modify the
active loop; its output is a strictly-validated candidate revision with no
write path into the active loop's configuration or roles. A candidate
becomes active only when all eight §19.4 conditions hold: governance
preserved · reference integrity preserved · settlement access preserved ·
rollback valid · regression suite passes · protected capabilities
non-inferior · target metric improves by the declared margin · payment
acceptable. Insufficient evidence rests at provisional — never promoted,
never rejected on a guess.

Recorded honest finding (task 23): no inert candidate key can improve
per-settlement economics on the current loop; the §19.4 positive path is
gate-level composition until the ruler owner binds a re-key
(reference-candidates 2026-09-04). One CandidateRevision stands open for
the ruler owner: whether the N settlements backing a promotion may come
from a single run (F2-3, task 22 — charter-conformant, in tension with
§19.1's one-run sentence).

## Dogfooding (spec §19.5)

Never replace the working runtime in place: build the next version as a
candidate and promote it through the gate. Posture honestly: the loop
drives itself in composed fixtures — the mock E2E (task 20), the
real-runner E2E with live bwrap (task 25), and the baselines D legs
through the real composed loop (task 26) — but this repository's own tasks
were orchestrated by the Prime-Agent harness, not by `gauntlet run`, and
the repo root deliberately names no default runner (UNK-003).

## Baselines (spec §23.3)

A/B/C/D were executed on the bounded gadget-task fixture and recorded at
`.agents/reports/baselines.json` with the honest narrative in
`.agents/reports/baselines.md` (task 26). **Do not assume D wins — it was
measured, and the losses are recorded**: the B/C/D dispatch tie 10=10=10,
the not-comparable instrument-run denominators, and D's subtle-defect
escape to C's independent criterion. D's wins are also recorded:
compromised-critic refusal with zero durable settlements (its judge is an
instrument), durable instrument-produced settlements, bounded honest
termination. The record is scoped to the declared FIXED-CHILDREN threat
model (reference-candidates entry 6).

## Mutation evidence (spec §23.1 "teeth")

Every named defense has a teeth check: disabling the defense makes its
proof fail. The task-24 scoped mutation run over the anti-gaming manifest
loci — recorded at `.agents/reports/mutation.json`, 73 examined /
63 caught / 10 unviable / 0 missed — is a dated settlement-boundary
recording pinned by manifest digest, per the task-27 charter's
recorded-artifact posture for it.

## Proven vs provisional

Status categories are spec §26's six; unit-level claims defer to
`.agents/reports/conformance.json`.

- implemented_and_proven — AK-055: one successful run leaves a
  capability Observed and never promotes; the promotion road is the full
  §19.2 conjunction over the real composed ledger
  (`crates/gauntlet-cli/tests/p10_no_false_learning.rs`, task 22).
- implemented_and_proven — AK-056: an optimizer proposal that
  improves a metric by weakening verification fails promotion — executed
  over the real composed loop with the refusal reason itself as the tooth
  (`crates/gauntlet-cli/tests/p9_self_improvement_containment.rs`,
  task 23).
- implemented_and_proven — AK-057: the optimizer cannot modify the
  active loop; its only output is a candidate revision requiring isolated
  evaluation, held-out comparison, and valid rollback
  (`gauntlet_app::promote`, the `promotion::candidate_only` behavioral
  containment and the optimizer arch scan, task 23).
- implemented_but_provisional — AK-069: baselines A/B/C/D executed on the
  bounded fixture and recorded at `.agents/reports/baselines.json`,
  including the cases where D does not win (task 26).
- implemented_but_provisional — AK-070: the recorded task-24 mutation
  artifact `.agents/reports/mutation.json` is the evidence basis — a dated
  recording pinned at settlement, not a fresh execution.
