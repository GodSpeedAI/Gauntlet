# Task 12 recon — governing spec text (Core-spec.md)

Source of record: `/home/sprime01/projects/gauntlet/.agents/specs/Core-spec.md` (2156 lines, v2 normative).
All quotes verbatim; line numbers from the current file. Secondary pointers into
`/home/sprime01/projects/gauntlet/.agents/plans/gauntlet-impl.md`, `crates/`, `.agents/CURRENT_STATUS.md`,
and `.agents/reports/task11/correction-adjudication.md` are marked as such.

Plan context (gauntlet-impl.md:422-441, "Task 12 — Controller: selection, budget, diagnosis *(phase 3 · proves P4, P6)*"):

> **Goal:** A controller that selects the next affordable correction, records rejected alternatives, respects the recovery reserve, and diagnoses rather than re-patching.
> 1. `UnitScheduler`: eligibility per spec §11.2, deterministic ordering, slice construction inheriting the highest tier and most restrictive authority.
> 2. `CorrectionSelector`: score candidates on the ten dimensions of §15.1; reject governance-blocked paths first; record selection **and** rejected alternatives with reasons.
> 3. `BudgetMeter` adapter: reserve/charge/remaining across all dimensions; `would_breach_reserve` consulted before every dispatch.
> 4. Non-convergence detection (same residual id N times) routing into bounded diagnosis; diagnostic budget exhaustion ⇒ `INCONCLUSIVE` with quarantined evidence.
> 5. Proof tests `p4_corrective_loop` … and `p6_budget_control` …
> **Done when:** both proofs pass on the mock runner; removing the reserve check lets a run spend past the reserve and fails P6 (teeth; AK-037–AK-039, AK-051, AK-052).

---

## 1. Controller / control loop / orchestration flow

### §2 Governing idea and the control invariant (Core-spec.md:81-115)

Line 87-89:
```text
REFERENCE → TRANSFORM → OBSERVE → COMPARE → VERIFY → SELECT → CORRECT → SETTLE → REMEMBER
```

Lines 91-111 (expanded, with terminal states) — the branch points that name the controller's job:
```text
        ┌──────────────────────────── REFERENCE (frozen, digested) ─────────────┐
        │                                                                       │
        ▼                                                                       │
   TRANSFORM ──► REALITY/ARTIFACTS ──► OBSERVE ──► OBSERVATION BUNDLE ──► COMPARE
                                                                            │
                                                        RESIDUALS ◄─────────┘
                                                            │
                                                          VERIFY (instruments first)
                                                            │
                                             ┌──────────────┴──────────────┐
                                             ▼                             ▼
                                    SELECT NEXT AFFORDABLE            SETTLE + REMEMBER
                                        CORRECTION                         │
                                             │                             ▼
                                             └────► CORRECT ────► (repeat) DONE

terminal: SETTLED | BLOCKED | BUDGET_EXHAUSTED | ESCALATED | ABORTED | INCONCLUSIVE
```

Line 113: "The system's optimization target is **not** maximum autonomy. It is *the smallest trustworthy Gauntlet that can settle enough work to earn its next increase in capability.*"

§1 vocabulary, line 75: "**Iteration** | One `observe → compare → verify → select → correct` cycle within a run. | a round trip"

### §4.1 Layering (Core-spec.md:156-160) — the application-layer components Task 12 builds

```text
 application  │ RunController · IterationEngine · UnitScheduler
              │ CorrectionSelector · SettlementService · PromotionService
              │ ReferenceFreezer · EnvelopeBuilder · DiagnosticEngine
```

### §4.2 Phase structure (Core-spec.md:171-182)

Line 177 (Loop row): "| **Loop** | Reference frozen, budget allocated. | Iterations: observation, comparison, verification, correction | §11–§15 |"

### §11.1 Run lifecycle (Core-spec.md:812-818) — iteration structure

```text
init → doctor → freeze reference → plan slices → [iterate]* → terminal → report
```
Line 818: "`[iterate]` = select → dispatch → observe → compare → verify → dispose residuals → checkpoint."

Note the plan's Task-12 ordering (select first inside an iteration, after the previous iteration's residuals) vs the §2 invariant's macro order (SELECT follows VERIFY). §11.1's bracket is the authoritative per-iteration sequence.

### Re-verify the reference at iteration boundaries (REQUIRED, four places)

- §6.2 `ReferenceProvider` row (Core-spec.md:444): "`resolve(intent) -> ReferenceDraft` · `normalize` · `digest` · `freeze(run_id) -> ReferenceVersion` · `verify_unchanged(version)` | `verify_unchanged` is the I1 tripwire; called before settlement and at every iteration boundary."
- §9.4 Freezing enforcement bullet (Core-spec.md:666): "`verify_unchanged()` runs at every iteration boundary and immediately before settlement. A digest mismatch sets the run terminal state to `aborted` with class `reference_mutated` and preserves both digests as evidence."
- §11.4 Reconciliation step 4 (Core-spec.md:843): "Re-verify the reference digest (I1)."
- §8.5 preflight (Core-spec.md:586): "REQUIRED checks before any run starts, and re-checked at every iteration boundary for the subset that can drift:"
- I11 (Core-spec.md:133): "Durable append-only event log + state store; every iteration boundary is a checkpoint; resume reads the log, not a transcript."

### Dispatch → observe → compare → verify → select → correct → settle sequencing

§10.4 Fan-out and fan-in (Core-spec.md:759-772):
```text
orchestrator
  ├─ dispatch builder(unit_slice)                    ── admission
  │     └─ builder works inside sandbox+workspace
  ├─ on builder result: observe (instruments, parallel facets)
  ├─ fan-out critics by dimension (ReviewMode)       ── admission × N
  │     ├─ critic:functional
  │     ├─ critic:architecture
  │     └─ critic:security          (only dimensions instruments cannot settle)
  ├─ fan-in: poll result contracts until deadline
  ├─ aggregate residuals (deterministic merge, §13.5)
  ├─ verify claims (deterministic verifiers first)
  └─ select correction | settle | escalate
```

### Other flow-relevant obligations

- §11.6 (Core-spec.md:871): "Every terminal state produces a report distinguishing: *implemented and proven · implemented but provisional · configured but untested · proposed · blocked · unknown.* Provisional capability is never reported as complete."
- §10.7 (Core-spec.md:802-806): orchestrator restraint — "The orchestrator owns decomposition, contracts, acceptance, verification architecture, evidence review, and settlement decisions. It **SHOULD NOT** write implementation code itself while a delegation mechanism exists. … The orchestrator never delegates the question of whether the total system actually settled."
- §13.5 aggregation (Core-spec.md:941-947): "Aggregation MUST NOT be performed by a model. It is arithmetic over typed records." Residuals keyed `(dimension, expected, observed_locator)`; blocking residual forces aggregate `fail`; comparator disagreement preserved as `disputed`, routed "to verification or diagnosis, never to a majority vote."
- §16.1 settlement requires (Core-spec.md:1020-1027): PASS VerificationRecord, resolvable pinned evidence ref, authority permits closure, no blocking residual, reference digest unchanged; run settles only when every blocking Unit settled AND `settlement.minimum_reliability` met.

---

## 2. Unit scheduling (§11.2 and surrounding)

### §11.2 Unit scheduling (Core-spec.md:820-831) — verbatim

"A Unit is dispatch-eligible only if **all** hold:

- `status ∈ {open}` and not already claimed or in flight;
- every `depends_on` Unit is `settled` or `out_of_scope`;
- `blast_radius != irreversible` (scheduler-level, unconditional — §17.1);
- global and per-tier concurrency slots available;
- a runner is configured for the roles the slice needs;
- the budget meter will not breach the recovery reserve.

Ordering: ascending blast-radius risk (`contained` first), then dependency depth, then `UnitId` lexicographic. Deterministic ordering matters: a scheduler whose behavior varies run to run cannot be compared against itself."

### §11.3 Slicing (Core-spec.md:833-835) — verbatim

"Units may be grouped into a **slice** dispatched to one builder when they are not meaningfully separable (same file, same invariant). A slice inherits the **highest** blast radius and the **most restrictive** authority of its members. Slices never mix `irreversible` members with anything."

### §11.4 Reconciliation (every tick) (Core-spec.md:837-845) — verbatim

"Every tick (`GAUNTLET_POLL_INTERVAL_MS`, default 5000):
1. Poll in-flight invocations; collect completed results.
2. Detect stalls: no event and no result for `GAUNTLET_STALL_TIMEOUT_MS` → cancel, record, retry or escalate.
3. Re-read the answer key. If an operator hand-edited a Unit that is in flight (tier, authority, claim, or status changed), cancel that attempt and re-evaluate eligibility from scratch. The in-memory snapshot is never authoritative over the file.
4. Re-verify the reference digest (I1).
5. Re-check budget; if a limit is breached, stop dispatching and drive to a terminal state."

### §11.5 Retry and escalation (Core-spec.md:846-858) — verbatim table

| Cause | Response |
|---|---|
| Residual found, attempts < `max_attempts` | Re-dispatch builder with residual text (not critic trajectory) attached; no backoff |
| Transient error (tool, process, network, store) | Exponential backoff `min(base × 2^(n-1), max_backoff)`; `base` and `max` configurable |
| `malformed_result` | One re-dispatch with a schema reminder; second failure escalates |
| `TimedOut` | Cancel, preserve session ref, retry once, then escalate |
| Attempts exhausted | Escalate with `max_attempts_exceeded` |
| Authority ≠ `critic` for the required closure | Escalate; never loop past it |
| Sandbox validation failure | Escalate; never fall back to unsandboxed |
| Reference digest mismatch | Abort the run; do not retry |
| Non-converging residual set (same residual id N times, `N = GAUNTLET_NONCONVERGENCE_N`, default 3) | Stop correcting; enter diagnosis (§15.2); if diagnosis is inconclusive, escalate |

### §11.6 Terminal states (Core-spec.md:860-869) — verbatim table

| Terminal | Requires |
|---|---|
| `settled` | Every blocking Unit `settled` with `PASS` verification and pinned evidence; reference digest unchanged; settlement record durable |
| `blocked` | ≥1 Unit requiring an authority the run cannot exercise; escalation ticket created |
| `budget_exhausted` | A budget dimension breached; recovery reserve used only for checkpoint + report |
| `escalated` | Operator action required and recorded |
| `inconclusive` | Diagnostic budget exhausted without resolving a divergence; evidence quarantined |
| `aborted` | Integrity violation (reference mutation, contract mismatch) or operator abort |

### depends_on handling and DAG validation

- §5.3.2 Unit field (Core-spec.md:287): `depends_on: [AK-011]                  # ordering only; not authority`
- §9.2 delta table (Core-spec.md:640): "| Blocking edges via tracker relations | `depends_on` list | Ordering only. Dependency never confers authority. |"
- **The spec contains NO explicit depends_on existence or acyclicity (DAG) validation requirement.** The only structural gate is §11.2's eligibility rule ("every `depends_on` Unit is `settled` or `out_of_scope`") plus §8.5 doctor check 2 (Core-spec.md:589): "Reference manifest and answer key parse; every Unit validates against §5.3.2 including the tier/authority coupling rule." A dangling `depends_on` would simply never become eligible; a cycle is not spec-addressed. (Code today: `gauntlet-app/src/reference/parse.rs:78-79` has `MalformedDependsOn` for malformed flow sequences, and parse.rs:28-31 notes the answer key's own vocabulary is `AK-NNN`, so "a `depends_on` target could never resolve to a block outside it" — but no cross-block existence/cycle check exists in the spec or the parser.)
- Scheduler-level irreversibility duty: §11.2 bullet 3 (quoted above) and I9 (Core-spec.md:131): "…`irreversible` units are never automatic dispatch candidates at the scheduler level."

---

## 3. Correction selection (§15.1 and surrounding)

### §15.1 The controller (Core-spec.md:986-998) — verbatim in full

"Inputs: reference, residuals, verification results, current project state, payment capacity, risk state, governance constraints, recovery reserve, developmental memory.

For each candidate correction, estimate: impact · dependency order · confidence · payment · reversibility · risk · governance status · settlement accessibility · recovery cost · expected information gain.

Procedure:
1. Reject governance-blocked paths first (authority, tier, budget, irreversibility).
2. Among allowed corrections, select the **next affordable correction** that most usefully reduces error while preserving recovery capacity.
3. Prefer `bounded correction → observable consequence → useful settlement` over large speculative rewrites. A coherent bounded batch is acceptable only when the changes cannot be meaningfully separated.
4. Record the selection **and the rejected alternatives with reasons** on the iteration. A controller whose choices are unexplained cannot be improved.

It does not simply select the largest residual."

**The ten scoring dimensions, exactly (line 990):** impact · dependency order · confidence · payment · reversibility · risk · governance status · settlement accessibility · recovery cost · expected information gain.

Related:
- §2 diagram (line 105): "SELECT NEXT AFFORDABLE CORRECTION" (the settle branch is the alternative at that fork).
- §5.3.4 Iteration aggregate (Core-spec.md:325): `selected_correction:     # what this iteration attempted, and why (§15)`
- Answer key AK-037 (Core-spec.md:1659-1662): "claim: The controller records the selected correction together with rejected alternatives and their reasons. … locator: tests/contract/controller::records_alternatives"

---

## 4. Budget

### §5.3.7 `Budget` value object (Core-spec.md:364-379) — verbatim

```yaml
budget:
  max_rounds:              # iterations per run
  max_parallel:            # concurrent role invocations
  max_depth:               # delegation depth
  max_wall_seconds:
  max_tokens:
  max_cost_usd:
  recovery_reserve_pct:    # portion never spendable on forward work
  diagnostic_budget:       # separate allowance for §15.2 diagnosis
  spent: {...}             # mirrored counters
```

"Invariant: the controller MUST refuse to dispatch any invocation that would consume the recovery reserve. The reserve exists so that a run can always afford to checkpoint, roll back, and report." (Core-spec.md:379)

(Note: the Budget VO lives at §5.3.7, not §5.2; §5.2 is value objects/enums.)

### §6.2 `BudgetMeter` port (Core-spec.md:455) — verbatim table row

"| `BudgetMeter` | `reserve(slice) -> Reservation` · `charge(reservation, payment)` · `remaining()` · `would_breach_reserve(slice) -> bool` | Every dimension in §5.3.7. |"

Budget also reaches invocations via §6.1 `RoleInvocationSpec` (Core-spec.md:418-422): fields `role, prompt, context_envelope, workspace, permissions, result_contract_path, deadline, model_hint, sandbox_policy, budget_slice`.

### §17.4 Budget as governance (Core-spec.md:1080-1084) — verbatim

"`BudgetMeter` is consulted before every dispatch, not after. Dimensions: rounds, parallelism, depth, wall time, tokens, cost, tool invocations, retries. Breach → stop dispatching, drive to terminal, report. The recovery reserve is never spendable on forward work.

Payment includes verification cost and orchestration burden, not just builder tokens. A loop that halves builder cost by tripling orchestration cost has not improved."

(§17.4 adds two dimensions beyond §5.3.7's fields: tool invocations and retries.)

### §8.6 Defaults table (Core-spec.md:599-612) — budget rows verbatim

| Setting | Default | Rationale |
|---|---|---|
| `max_rounds` | 8 | bounded even if unset |
| `max_parallel` | 2 | parallel writers corrupt each other's assumptions |
| `max_depth` | 1 | recursion is raised deliberately, never by default |
| `max_wall_seconds` | 3600 | |
| `recovery_reserve_pct` | 15 | a run must always afford to stop cleanly |
| `diagnostic_budget` | 2 | diagnosis is bounded too |

I10 (Core-spec.md:132): "The runtime is bounded even when configuration is incomplete. | Every budget dimension has a conservative default; absence of a configured budget is never permission for unlimited recursion, parallelism, or spend. | P6"

§8.5 preflight check 8 (Core-spec.md:595): "Budget dimensions parse and are non-zero; recovery reserve < 100%."

§8.2 GAUNTLET.md example budget block (Core-spec.md:543-551): `max_rounds: 8, max_parallel: 3, max_depth: 2, max_wall_seconds: 5400, max_tokens: 2000000, max_cost_usd: 25, recovery_reserve_pct: 15, diagnostic_budget: 2` (note: the example config differs from the §8.6 defaults table — §8.6 is the defaults of last resort).

### Annex E env vars for budgets/limits (Core-spec.md:2092) — verbatim

"| Limits | `GAUNTLET_INTERACTIVE` `GAUNTLET_MAX_ROUNDS` `GAUNTLET_MAX_PARALLEL` `GAUNTLET_MAX_DEPTH` `GAUNTLET_MAX_WALL_SECONDS` `GAUNTLET_MAX_TOKENS` `GAUNTLET_MAX_COST_USD` `GAUNTLET_RECOVERY_RESERVE_PCT` `GAUNTLET_DIAGNOSTIC_BUDGET` `GAUNTLET_REVIEW_MODE` `GAUNTLET_MAX_CRITICS` `GAUNTLET_MAX_ATTEMPTS_PER_UNIT` `GAUNTLET_NONCONVERGENCE_N` `GAUNTLET_POLL_INTERVAL_MS` `GAUNTLET_STALL_TIMEOUT_MS` `GAUNTLET_RETRY_BASE_MS` `GAUNTLET_RETRY_MAX_MS` |"

### Budget in the flow

- §11.2 eligibility bullet (Core-spec.md:829): "the budget meter will not breach the recovery reserve."
- §11.4 step 5 (Core-spec.md:844): "Re-check budget; if a limit is breached, stop dispatching and drive to a terminal state."
- §10.4 concurrency (Core-spec.md:774-777): "`max_parallel` bounds total concurrent invocations across all roles, and `max_depth` bounds delegation depth. Both default low. / Critic fan-out is bounded by `review.max_critics` and by the budget meter; a fan-out that would breach the recovery reserve is truncated in dimension-priority order, and the truncation is recorded as `unavailable_evidence`, not silently dropped."
- §11.6 `budget_exhausted` (Core-spec.md:866): "A budget dimension breached; recovery reserve used only for checkpoint + report."
- Answer key AK-051 (Core-spec.md:1758-1761): "A non-converging task terminates at the declared budget limit with the recovery reserve unspent." AK-052 (1766-1769): "The budget meter is consulted before every dispatch and refuses any dispatch that would consume the recovery reserve."

---

## 5. Non-convergence, bounded diagnosis, INCONCLUSIVE, quarantined evidence

- Trigger definition, §11.5 last row (Core-spec.md:858): "Non-converging residual set (same residual id N times, `N = GAUNTLET_NONCONVERGENCE_N`, default 3) | Stop correcting; enter diagnosis (§15.2); if diagnosis is inconclusive, escalate"
- §15.2 Diagnostic closure (Core-spec.md:1000-1012) — verbatim in full:

"When expected and observed diverge in a way that repeats, do not patch. Run a bounded diagnostic cycle:

```text
diagnosis → prediction → cheapest discriminating test → disconfirming observation → correction
```

Candidate failure loci — enumerate before choosing: bad reference · ambiguous reference · wrong observation · observation failure · incorrect comparison · critic error · verifier error · route-selection error · implementation defect · tool defect · environment drift · insufficient payment · missing capability · integration failure · architectural defect · governance conflict.

Note that six of those sixteen are defects in the *measuring apparatus*, not the implementation. A loop that only ever diagnoses "implementation defect" is not diagnosing.

Spend from `diagnostic_budget`. If nothing resolves within it: mark `INCONCLUSIVE`, quarantine the evidence, stabilize, escalate or defer. Do not invent certainty."

- Terminal-state contract, §11.6 (Core-spec.md:868): "`inconclusive` | Diagnostic budget exhausted without resolving a divergence; evidence quarantined"
- I8 (Core-spec.md:130): "`PASS`, `FAIL`, `INCONCLUSIVE` are distinct states in the type system. `INCONCLUSIVE` never satisfies a settlement requirement. Absent evidence is recorded in `unavailable_evidence`, never defaulted."
- §14.4 (Core-spec.md:980): "`INCONCLUSIVE` is not `FAIL` … and is not `PASS` … It never settles a Unit. It may be resolved by producing the evidence, by improving the instrument, or by escalation — never by re-asking a model until it says yes." (G16 is exactly that gaming move.)
- ComparisonRecord residual fields support diagnosis (Annex B.4, Core-spec.md:1956-1960): each residual carries `"diagnosis": "", "cheapest_discriminating_test": "", "candidate_corrections": []`.
- Disputed blocking residuals route "to verification or diagnosis, never to a majority vote" (§13.5, Core-spec.md:946).
- AK-039 (Core-spec.md:1666-1669): "A repeated identical residual triggers bounded diagnosis rather than another patch, and exhausting the diagnostic budget yields INCONCLUSIVE. … locator: tests/contract/diagnosis::nonconvergence"

---

## 6. §18 checkpoints / resume / hang disposal

### §18.1 Substrate (Core-spec.md:1098-1104)

"Prefer the simplest durable local substrate the environment already supports. … Deterministic export to portable JSON/JSONL is REQUIRED regardless of backend.

Minimum durable records: runs · iterations · role invocations · reference versions · observations · comparisons · verification records · residuals · payments · settlements · failures · recovery events · candidate revisions · promotion decisions · escalations."

### §18.2 Checkpoints (Core-spec.md:1106-1108) — verbatim

"Every iteration boundary writes a durable checkpoint containing: run id, reference digest, iteration number, unit-state projection, budget spent, in-flight invocation ids with their result-contract paths, and the event-log offset. **Critical run state is never reconstructed from conversational memory** (I11)."

### §18.3 Resume (Core-spec.md:1110-1119) — verbatim

"`gauntlet resume <run_id>`:
1. Replay the event log from the last checkpoint offset; complete any half-applied settlement idempotently (§16.3).
2. Re-freeze-verify the reference digest; mismatch → `aborted{reference_mutated}`.
3. Re-probe in-flight invocations: a valid result contract on disk is collected; anything else is marked `Failed{class: orphaned_by_restart}` and re-planned. No in-memory scheduler state is assumed to survive.
4. Sweep workspaces for settled/out-of-scope units.
5. Re-run preflight (§8.5) and continue.

Run identity, prior evidence, and payment history survive restart intact. Re-running settled work is a defect, not a safe default."

### §18.4 Assume these failures (Core-spec.md:1121-1123) — verbatim in full

"Agent process dies · model times out · tool fails · MCP server disappears · network vanishes · test hangs · comparator emits malformed output · verifier disagrees with comparator · workspace becomes dirty · budget exhausted · reference invalid · environment changes mid-run. Each has a defined disposition in §11.5 and §21."

Note: §18.4 does not itself prescribe hang handling; it points at §11.5/§21. The controller-side duties for hung/unresponsive invocations are:

- §11.4 step 2 (Core-spec.md:841): "Detect stalls: no event and no result for `GAUNTLET_STALL_TIMEOUT_MS` → cancel, record, retry or escalate."
- §10.3 rule 5 (Core-spec.md:752): "No result by deadline is `TimedOut`. The runtime cancels the child, preserves the session reference for forensics, and applies retry/escalation policy (§11.5)."
- §11.5 `TimedOut` row (Core-spec.md:853): "Cancel, preserve session ref, retry once, then escalate."
- §7.3 contract suite (Core-spec.md:499): "…a child that never writes a result contract is reported `TimedOut`, not `Completed`; cancellation is idempotent."
- §18.3 step 3 (restart case): anything without a valid on-disk result contract is `Failed{class: orphaned_by_restart}` and re-planned.
- Annex C.4 caveat (Core-spec.md:2035): "Idle children may unload and reload; a poll that finds no process must not conclude failure while the result contract is still pending and the deadline has not expired."
- §6.1 rule (Core-spec.md:436): "`invoke` MUST return promptly after admission. It MUST NOT block on completion." And §6.1 (437): "The runtime determines completion by the **result contract** (§10.3), never by parsing free-form agent prose."

Relevant answer keys: AK-053 (Core-spec.md:1773-1776) checkpoint sufficiency; AK-041/AK-042 (1704-1716) commit order / exactly-once resume (Task 13's P7, but the controller must not release units pre-durability per §16.3).

---

## 7. P4 and P6 — §23.1 verbatim (Core-spec.md:1266, 1268)

| ID | Proof | Method |
|---|---|---|
| **P4** | Corrective loop | Inject a controlled defect. Observer/comparator/verifier expose it; the controller routes a corrective iteration that closes it. |
| **P6** | Budget control | A deliberately non-converging task terminates at the declared limit with the recovery reserve intact. |

Teeth-check rule (Core-spec.md:1276): "Each proof MUST have a **teeth check**: disabling the defense makes the proof fail. A proof that passes with its mechanism removed is testing nothing." (Plan's Task-12 teeth: removing the reserve check lets a run spend past the reserve and fails P6.)

Companion answer keys: AK-038 (Core-spec.md:1673-1676, locator `tests/proofs/p4_corrective_loop`), AK-051 (1758-1761, `tests/proofs/p6_budget_control`), AK-052 (1766-1769, `tests/contract/budget::reserve_protected`), AK-037 (1659-1662), AK-039 (1666-1669).

---

## 8. Capability-gating store writes / evidence-owner / observation-owner

**The literal strings "evidence-owner", "observation-owner", "capability-gated", `record_evidence_production`, and `bind_observation_bundle` do NOT appear in Core-spec.md.** They are repo-native terms from Tasks 10/11. The spec basis Task 12 can cite:

- §10.2 Standard roles table (Core-spec.md:725-734) — role-scoped write ownership:
  - "`verifier` | May modify: its own evidence artifacts | May read: claim + evidence + instrument output | Never: implementation"
  - "`observer` | May modify: its own facet artifacts | May read: target system state | Never: implementation"
  - "`builder` | … | Never: active reference, comparison contract, acceptance gates, evidence store, answer-key status"
  - Line 736: "Enforce with the environment where possible (filesystem permissions, read-only mounts, sandbox policy, separate credentials), not with prompt text alone."
- §16.4 Single-writer discipline (Core-spec.md:1045-1047): "The runtime is the sole writer of Unit status and evidence fields. Builder and critic roles MUST NOT write the answer key even with general filesystem access; the file lives outside builder-writable workspaces or is mounted read-only. This is what makes the audit trail structural rather than aspirational — an agent that can rewrite the line that grades it has no grader."
- §22.5 (Core-spec.md:1248): "**The grader is not writable by the graded.** Answer key, frozen reference, evidence store, and event log are outside every builder-writable path."
- I5 (Core-spec.md:127): "Settlement, authority, budget, security, and irreversibility gates read typed fields, never model prose."
- §14.3 Evidence binding (Core-spec.md:970-976): evidence ref resolves; producing instrument matches `evidence_required.producer`; "the evidence was produced in this run at or after the transform it certifies (no stale evidence)"; locator matches. "Otherwise the outcome is `INCONCLUSIVE` with class `evidence_unresolvable` / `evidence_stale` / `evidence_producer_mismatch`."
- §12.2 (Core-spec.md:890): "A facet with no instrument identity is invalid." §12.3 (894): unavailable probes recorded, never dropped, never PASS.

Where the Task-12 obligation actually lives (repo, not spec):
- `crates/gauntlet-ports/src/state_store.rs` — doc comment on `record_evidence_production` (~line 340-376): "**Recorded residual (mirrors Task 10's ledger note):** when Task 12 wires the production writer into the observation pipeline, this write should be capability-gated to the evidence-owner role exactly as `bind_observation_bundle` should be; today both are composition-root-wired with no role check in the store."
- Same file, `bind_observation_bundle` doc (~line 313): "This is the observation owner's write (spec §5.3.6, §18.1) … The binding is write-once — a second call naming a DIFFERENT binding for the same (run, iteration) is refused, never overwritten."
- Same file, `accept_comparison` doc (~line 405-416): durable comparison state "reachable only from a document a comparator returned under a governed dispatch capability" (`AdmittedComparison` has no constructor outside the comparator port) — the existing pattern to mirror.
- `.agents/CURRENT_STATUS.md:518-526` (ledger): "**Ungated evidence-production write + declared-iteration validation at write time** (critic-b MAJOR-3, adjudicated C7) … Trigger: **Task 12** — capability-gate BOTH writes when the controller wires their production writers (evidence-owner and observation-owner roles)." Also CURRENT_STATUS.md:90-93.
- `.agents/reports/task11/correction-adjudication.md:24` (C7): "RECORDED DEFERRAL, upgrade trigger Task 12 (capability-gate the evidence-owner write + validate declared iteration there). Builder and both critics converge; ports doc already carries the residual."
- Durability spine for these writes: `RunEventKind::EvidenceProduced { evidence, locator }` (state_store.rs ~line 88-96) — "the durable event behind `StateStore::record_evidence_production`. Its append offset IS the ordering anchor of the production binding — the monotonic fact §14.3's no-stale-evidence condition is decided against."

---

## 9. Reference mutation at the controller: classification and abort semantics

The spec spells a snake_case class string and a terminal state, not Rust variants:

- §9.4 (Core-spec.md:666): "`verify_unchanged()` runs at every iteration boundary and immediately before settlement. A digest mismatch sets the run terminal state to `aborted` with class `reference_mutated` and preserves both digests as evidence."
- §11.5 (Core-spec.md:857): "Reference digest mismatch | Abort the run; do not retry."
- §11.6 (Core-spec.md:869): "`aborted` | Integrity violation (reference mutation, contract mismatch) or operator abort."
- §18.3 step 2 (Core-spec.md:1114): "Re-freeze-verify the reference digest; mismatch → `aborted{reference_mutated}`."
- §21.3 (Core-spec.md:1238): "Integrity violation | Abort the run; preserve everything."
- §5.3.6 (Core-spec.md:360): "`ComparisonRecord` MUST carry `reference_digest` and `observation_digest`; a record whose `reference_digest` differs from the run's frozen digest invalidates the iteration (I1)."
- I1 (Core-spec.md:123): "…digest re-verified before settlement; mismatch invalidates the run." Anti-gaming G2 (1285): "Change the reference mid-run". Mock runner must replay "a reference-mutation attempt" (§10.6, line 799; AK-059, line 1800-1805).

Code classification (context, not spec): `ReferenceProviderError::ReferenceMutated { expected: Digest, actual: Digest }` at `crates/gauntlet-ports/src/reference_provider.rs:37` (Display at :57; `verify_unchanged` doc at :141). `Unavailable` is a different, non-fatal classification: `ObserverError::Unavailable { kind: FacetKind, detail }` at `crates/gauntlet-ports/src/observer.rs:46` — "The instrument could not run at all" (vs `InstrumentFailed`, "the instrument ran and failed"). Spec semantics for Unavailable: §12.3 (Core-spec.md:894) — recorded in `unavailable_evidence`, never dropped, never becomes `PASS`; dependent verification becomes `INCONCLUSIVE` (§21.3, line 1236). So: **Unavailable = observation gap → INCONCLUSIVE for dependent claims; ReferenceMutated = integrity violation → abort the run, preserve evidence, do not retry.**

Controller duty summary at each iteration boundary (§6.2:444, §8.5:586, §9.4:666, §11.4:843): call `verify_unchanged`; on mismatch set terminal `aborted{reference_mutated}`, keep both digests as evidence, stop dispatching (§21.3:1233 "Stop new dispatch; keep the process alive; keep reconciliation running").

---

## 10. Governance: no tier/authority lowering; irreversible never auto-dispatched

- §5.3.2 Unit invariant (Core-spec.md:298): "A Unit's tier MAY be raised by an operator. It MUST NOT be lowered by any agent role. Tier downgrade is the canonical gaming move (§23.2, G7)."
- I9 (Core-spec.md:131): "Authority gates whether it runs; sandbox gates how contained it is. | Both checks precede builder process start, in that order, and neither substitutes for the other. `irreversible` units are never automatic dispatch candidates at the scheduler level. | P5"
- §11.2 eligibility bullet (Core-spec.md:826): "`blast_radius != irreversible` (scheduler-level, unconditional — §17.1)"
- §17.1 The authority gate (before dispatch) (Core-spec.md:1053-1061) — verbatim in full:

```text
if unit.blast_radius == irreversible          -> never an automatic dispatch candidate
if unit.authority != critic and closure needed-> escalate before any transform
if operator_context lacks required identity   -> escalate
```

"Scheduler-level, unconditional, independent of what any prompt says. A tier may be raised by an operator; it is never lowered by an agent."

That last sentence (Core-spec.md:1061) is the scheduler-level duty the caller asked to be quoted specifically: the irreversibility check lives at the scheduler, before dispatch, regardless of any prompt content.

- §17.3 tier table `irreversible` row (Core-spec.md:1074): "**No policy exists.** The action never reaches the sandbox port unattended." And line 1076: "The absence of an `irreversible` policy file is a *requirement*, not an omission…"
- §8.5 check 5 (Core-spec.md:592): "Sandbox policies exist for `contained`, `local`, `cross_cutting` when `backend != none`; **no policy exists for `irreversible`** (its presence is a config error, §17.3)."
- Annex F (Core-spec.md:2133): "The action never reaches the sandbox port unattended; the human authority gate fires first. Sandboxing an irreversible action well is still shipping it."
- Tier/authority coupling at parse time, §5.3.2 (Core-spec.md:295-296): "`blast_radius = irreversible` ⟹ `authority = human`. Any parse producing another combination is an error. / `blast_radius ∈ {cross_cutting, irreversible}` ⟹ authority MUST NOT be `critic`, regardless of `verify_by`."
- Slice inheritance reinforces this: §11.3 (835) — slice takes highest blast radius and most restrictive authority; never mixes irreversible members.
- §11.4 step 3 (842): an operator hand-edit mid-flight (tier, authority, claim, or status) cancels the attempt and re-evaluates eligibility — the controller must notice raises, and can never effect lowers.
- Anti-gaming G7 (Core-spec.md:1290): "Downgrade a unit's blast radius or authority to unblock". AK-044 (Core-spec.md:1744ff): "A unit tagged irreversible is never an automatic dispatch candidate at the scheduler level."

---

## Cross-reference quick map for Task 12

| Task-12 concern | Spec sections |
|---|---|
| Iteration sequencing + boundary duties | §2 (85-111), §11.1 (812-818), §11.4 (837-845), §6.2 ReferenceProvider (444), §9.4 (666), I11 (133) |
| Scheduling eligibility/order/slices | §11.2 (820-831), §11.3 (833-835), §10.4 concurrency (774-777) |
| Correction selection | §15.1 (986-998), §5.3.4 (325), AK-037 |
| Budget | §5.3.7 (364-379), §6.2 BudgetMeter (455), §17.4 (1080-1084), §8.6 (599-612), §8.5 check 8 (595), I10 (132), Annex E (2092) |
| Non-convergence/diagnosis | §11.5 row 9 (858), §15.2 (1000-1012), §11.6 (868), I8 (130), §14.4 (980), AK-039 |
| Checkpoints/resume/hangs | §18.1-18.4 (1098-1123), §11.4 step 2 (841), §10.3 rule 5 (752), §7.3 (499), C.4 (2035) |
| P4/P6 | §23.1 (1266, 1268), teeth (1276), AK-038/AK-051/AK-052 |
| Owner-gated writes | §10.2 (725-736), §16.4 (1045-1047), §22.5 (1248), §14.3 (970-976); repo: state_store.rs docs, CURRENT_STATUS.md:518-526, task11 adjudication C7 |
| Reference mutation | §9.4 (666), §11.5 (857), §11.6 (869), §18.3 (1114), §21.3 (1238), G2 |
| Governance floors | §5.3.2 (295-298), I9 (131), §17.1 (1053-1061), §17.3 (1074-1076), G7 |
