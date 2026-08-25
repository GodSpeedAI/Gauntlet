# Task 12 recon — answer-key targets, adjacency scan, plan text, candidates format, prereg status

Read-only reconnaissance for plan Task 12 (Controller: selection, budget, diagnosis — proves P4, P6).
Frozen ruler read only; nothing under `.wayfinder/` was modified. Date: 2026-08-23.

## 1. Task 12's target Units — verbatim, in full (`.wayfinder/answer-key.md`)

### AK-037 (lines 256–261)
```
## AK-037
claim: The controller records the selected correction together with rejected alternatives and their reasons.
verify_by: test
blast_radius: local
authority: critic
evidence_required: {kind: test_report, locator: tests/contract/controller::records_alternatives, producer: instrument}
```

### AK-038 (lines 263–268)
```
## AK-038
claim: An injected controlled defect is exposed by observation and comparison and closed by a routed corrective iteration.
verify_by: test
blast_radius: local
authority: critic
evidence_required: {kind: test_report, locator: tests/proofs/p4_corrective_loop, producer: instrument}
```

### AK-039 (lines 270–275)
```
## AK-039
claim: A repeated identical residual triggers bounded diagnosis rather than another patch, and exhausting the diagnostic budget yields INCONCLUSIVE.
verify_by: test
blast_radius: local
authority: critic
evidence_required: {kind: test_report, locator: tests/contract/diagnosis::nonconvergence, producer: instrument}
```

### AK-051 (lines 355–360)
```
## AK-051
claim: A non-converging task terminates at the declared budget limit with the recovery reserve unspent.
verify_by: test
blast_radius: local
authority: critic
evidence_required: {kind: test_report, locator: tests/proofs/p6_budget_control, producer: instrument}
```

### AK-052 (lines 362–367)
```
## AK-052
claim: The budget meter is consulted before every dispatch and refuses any dispatch that would consume the recovery reserve.
verify_by: test
blast_radius: local
authority: critic
evidence_required: {kind: test_report, locator: tests/contract/budget::reserve_protected, producer: instrument}
```

Notes on the five targets: all are `verify_by: test`, `blast_radius: local`, `authority: critic`, producer `instrument`, kind `test_report`; none carries `depends_on`. Three locators (`tests/contract/controller::…`, `tests/contract/diagnosis::nonconvergence`, `tests/contract/budget::reserve_protected`) are in the repo-root `tests/contract/` shape that the frozen boundary suite forbids for core crates (established locator-materialization defect class); two (`tests/proofs/p4_corrective_loop`, `tests/proofs/p6_budget_control`) are in the repo-root `tests/proofs/` shape already re-declared to composition-root `gauntlet-cli tests/pN_…` targets for AK-022/023/024 and AK-029. The plan's Task-12 gate itself spells `cargo nextest run --test p4_corrective_loop --test p6_budget_control` (i.e., a cargo test-target name, not a repo-root path) and `-p gauntlet-app control::` for the in-crate tests.

## 2. Adjacency scan — other Units touching Task-12 keywords

Genuine keyword hits (controller/scheduler/scheduling/budget/reserve/correction selection/rejected alternative/diagnosis/non-convergence/convergence/hang/deadline/iteration boundary/checkpoint/resume), excluding the five targets:

- **AK-024** (line 165, "iteration boundary") — "The reference digest is re-verified at every iteration boundary and immediately before settlement." Task 12 wires the real controller iteration boundaries; the 2026-08-18 AK-022/023/024 candidates entry says these remain "substrate-level until plan Tasks 12–13 wire the real controller iteration boundaries and the settlement pre-check". Task 12 is on the hook to make the real callsites exist.
- **AK-034** (line 236, "budget") — "Critic fan-out truncated by budget records the missing dimensions as unavailable evidence rather than omitting them." Comparator-side budget truncation (Task 10; already has a reference-candidates entry re-declaring its locator).
- **AK-044** (line 307, "scheduler") — "A unit tagged irreversible is never an automatic dispatch candidate at the scheduler level." Proved under Task 14 (P5) by plan assignment, but the *scheduler level* wording lands on Task 12's `UnitScheduler` eligibility (spec §11.2); watch the boundary between Task 12 eligibility and Task 14's hard gate.
- **AK-053** (line 370, "iteration boundary" + "checkpoint" + "resume") — "Every iteration boundary writes a durable checkpoint sufficient to resume without any conversational context." State-store Unit (Task 6 locator `tests/contract/state::checkpoint_sufficiency` — same unmaterializable shape), but the controller's iteration boundary is what must write the checkpoint.
- **AK-060** (line 419, "resume") — "The CLI exposes doctor, init, reference, run, status, inspect, verify, resume, escalations, roles, skills, improve, and export." CLI surface (Task 20); listed for completeness.

Substring false positives (NOT genuine keyword obligations, verified): AK-025 line 176 ("hang" inside `g2_change_reference`), AK-040 line 278 ("hang" inside "unchanged"), AK-064 line 447 ("reserve" inside "preserves"), AK-066 line 461 ("reserve" inside "preserved"). No Unit mentions "deadline", "scheduling", "correction selection" (as a phrase; AK-037 says "selected correction"), or a genuine "hang".

Near-misses worth awareness (no keyword, but selection/cost semantics): AK-035 "Verification selects the cheapest conclusive instrument…"; AK-014 "Adapter selection resolves from configuration only…". Also AK-023/AK-024/AK-025 mid-run reference discipline interacts with the controller loop.

### Requested full quotes (scheduler authority / no-lowering / reference tripwire units)

### AK-023 (lines 156–162)
```
## AK-023
claim: A builder attempt to modify the active reference fails or is detected and aborts the run with reference_mutated.
verify_by: test
blast_radius: irreversible
authority: human
evidence_required: {kind: test_report, locator: tests/proofs/p1_reference_integrity, producer: instrument}
depends_on: [AK-022]
```

### AK-024 (lines 164–170)
```
## AK-024
claim: The reference digest is re-verified at every iteration boundary and immediately before settlement.
verify_by: test
blast_radius: cross_cutting
authority: lead_agent
evidence_required: {kind: test_report, locator: tests/proofs/p1_reference_integrity::digest_rechecked, producer: instrument}
```

### AK-046 (lines 320–325)
```
## AK-046
claim: An agent role cannot lower a Unit's blast radius or authority; only an operator can raise it.
verify_by: test
blast_radius: cross_cutting
authority: lead_agent
evidence_required: {kind: test_report, locator: tests/antigaming/g7_tier_downgrade, producer: instrument}
```

### AK-047 (lines 327–332)
```
## AK-047
claim: Sandbox policy validation precedes execution, and a validation failure escalates without falling back to unsandboxed execution.
verify_by: test
blast_radius: irreversible
authority: human
evidence_required: {kind: test_report, locator: tests/contract/sandbox::no_unsandboxed_fallback, producer: instrument}
```

### AK-048 (lines 334–339)
```
## AK-048
claim: Containment wraps the builder session from process start; no builder command executes before the authority and containment checks.
verify_by: test
blast_radius: irreversible
authority: human
evidence_required: {kind: test_report, locator: tests/contract/sandbox::wraps_from_process_start, producer: instrument}
```

## 3. Plan Task 12 — verbatim (`.agents/plans/gauntlet-impl.md` lines 422–443)

```
## Task 12 — Controller: selection, budget, diagnosis *(phase 3 · proves P4, P6)*

**Goal:** A controller that selects the next affordable correction, records rejected alternatives, respects the recovery reserve, and diagnoses rather than re-patching.

### Steps

1. `UnitScheduler`: eligibility per spec §11.2, deterministic ordering, slice construction inheriting the highest tier and most restrictive authority.
2. `CorrectionSelector`: score candidates on the ten dimensions of §15.1; reject governance-blocked paths first; record selection **and** rejected alternatives with reasons.
3. `BudgetMeter` adapter: reserve/charge/remaining across all dimensions; `would_breach_reserve` consulted before every dispatch.
4. Non-convergence detection (same residual id N times) routing into bounded diagnosis; diagnostic budget exhaustion ⇒ `INCONCLUSIVE` with quarantined evidence.
5. Proof tests `p4_corrective_loop` (injected defect closed by a routed iteration) and `p6_budget_control` (non-converging task terminates with reserve intact).

### Gate

```bash
cargo nextest run --test p4_corrective_loop --test p6_budget_control && \
cargo nextest run -p gauntlet-app control::
```

**Done when:** both proofs pass on the mock runner; removing the reserve check lets a run spend past the reserve and fails P6 (teeth; AK-037–AK-039, AK-051, AK-052).

**Redesign trigger:** If the selector's scoring becomes unexplainable, reduce it to an explicit priority ordering with recorded tie-breaks. An opaque controller cannot be improved by the optimizer later.
```

Context: plan overview line 37 places Task 12 under "Task 8 mock AgentRunner" branch ("Task 12 controller + budget + diagnosis (P4, P6)"); the proof checklist (line ~839) reads "[ ] P4 corrective loop and P6 budget control; reserve never spent forward *(Task 12)*". Task 20 E2E depends on Task 12 among others.

## 4. `.agents/reports/reference-candidates.md` — existing entries and format

Header pins the discipline: per `.gauntlet/reference.lock`, ".wayfinder/answer-key.md is never edited for any reason … A Unit that seems wrong becomes a dated entry in `.agents/reports/reference-candidates.md` with evidence." Spec revisions go through the normal spec process.

Existing entries (11, newest last):
1. 2026-08-17 — AK-049 evidence locator names an unmaterializable test target (Task 5 correction K8 / critic-B R8) — the founding entry of the locator-materialization class
2. 2026-08-17 — AK-054 … (Task 6, critic-A MINOR-7)
3. 2026-08-18 — AK-021 … (Task 7, deliverable D8)
4. 2026-08-18 — AK-022/AK-023/AK-024 … (Task 7, deliverable D8) — includes "Scope honesty: AK-023/AK-024 remain substrate-level until plan Tasks 12–13 wire the real controller iteration boundaries and the settlement pre-check"
5. 2026-08-18 — AK-025 … (Task 7 correction C4, critic-A MAJOR-1)
6. 2026-08-19 — AK-028 … (Task 9, deliverable D5)
7. 2026-08-19 — AK-029 … (Task 9, deliverable D5)
8. 2026-08-19 — AK-030 … (Task 9, deliverable D5)
9. 2026-08-20 — AK-026 and AK-027 … (Task 10, deliverable D6)
10. 2026-08-20 — AK-032, AK-033, AK-034 … (Task 10, deliverable D6)
11. 2026-08-21 — AK-005, AK-035, AK-036 … (Task 11)

Entry format to follow when appending:
- H2 heading: `## YYYY-MM-DD — <unit(s)> evidence locator(s) name unmaterializable test target(s) (Task N, <origin: deliverable/correction/critic finding>)`
- `- **Unit:**` / `- **Units:**` — unit ids plus `.wayfinder/answer-key.md` line ranges (and spec line ranges when applicable)
- `- **Claim(s) under review (unchanged, …):**` — verbatim claim(s)
- `- **Defect (evidence declaration only):**` — why the locator cannot exist; if the known class, state "**additional instance of the existing locator-materialization class (first recorded 2026-08-17, AK-049)** — not a new defect class"
- `- **Proposed correction:**` — re-declare locator(s) to tests that exist and prove the claim, as sub-bullets with crate-qualified test paths and what each proves (composition-root `gauntlet-cli tests/<target>.rs <case>` precedent for pN proofs; in-crate `gauntlet-app <module>::…` for contract tests, ideally under the plan gate's own filter, here `control::`)
- Optional honesty sections used by precedent: `- **Scope honesty:**`, `- **Disposition honesty:**`, `- **Live-CLI corroboration:**`
- Closing, always: `- **Not changed by this entry:** the answer key, the spec, the frozen lock (…ruler status…). Settlement of these candidates belongs to the ruler's owner, not the builder.`

Prediction for Task 12 (not yet recorded): AK-037 (`tests/contract/controller::records_alternatives`), AK-039 (`tests/contract/diagnosis::nonconvergence`), AK-052 (`tests/contract/budget::reserve_protected`) are repo-root `tests/contract/` shapes (AK-049 class); AK-038 (`tests/proofs/p4_corrective_loop`) and AK-051 (`tests/proofs/p6_budget_control`) are repo-root `tests/proofs/` shapes (AK-022/023/024 precedent → `gauntlet-cli tests/p4_corrective_loop` / `gauntlet-cli tests/p6_budget_control` composition-root targets, matching the plan gate spellings).

## 5. Preregistration status for Task 12

`.agents/research/preregistrations/` exists and contains: `README.md`, `TEMPLATE.prereg.yaml`, `TEMPLATE.outcome.yaml`, `task-11.prereg.yaml`, `task-11.prereg.sha256`, `task-11.outcome.yaml`.

**No Task-12 prereg files exist**: no `task-12.prereg.yaml`, no `task-12.prereg.sha256`, no `task-12.outcome.yaml`.

README rules that matter now: `task-N.prereg.yaml` is "written **before** the task starts; immutable thereafter"; frozen once with `task-N.prereg.sha256` (checked via `just prereg-check N`); everything learned later goes in `.outcome.yaml` only; preregister only what is visible now; `non_discriminating` is first-class. If Task 12 implementation has not yet begun, there is still a window to write and freeze `task-12.prereg.yaml` first.
