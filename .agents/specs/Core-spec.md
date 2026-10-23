# GAUNTLET RUNTIME SPECIFICATION

**Status:** v2 — normative, standalone, implementation-ready
**Supersedes:** *Gauntlet Control Service Specification* (Draft v1), which specified only the operations/dashboard surface and contained three structural defects corrected here (§0.4).
**Audience:** a strong agentic model (or a human team) implementing this system from an empty or near-empty repository, plus the same class of model acting as the *comparator* that judges whether the implementation settled.

---

## 0. How to use this document

### 0.1 This document has two jobs

1. **Implementation specification.** Sections 1–26 define what to build. They are normative.
2. **Comparison reference (the ruler).** Annex A is a typed answer key derived from this spec. It is the standard the finished system is graded against — including when the system grades *itself* by running its own loop.

An implementing agent MUST, before writing code, copy Annex A verbatim to the active reference path and freeze it (§9.4). From that moment the implementation MUST NOT edit the frozen copy. A proposed change to the ruler is a separate, explicitly recorded operation (§19.4), never a side effect of implementation work.

### 0.2 Reading order for an implementing agent

```text
§0 → §2 → §3 (invariants — memorize these)
   → §5 (domain model) → §6 (ports) → §8 (configuration)
   → Annex A (the ruler) → §23 (proofs you must produce)
   → then the section covering your current bounded slice
```

### 0.3 Normative language

`MUST`, `MUST NOT`, `REQUIRED`, `SHOULD`, `SHOULD NOT`, `RECOMMENDED`, `MAY`, `OPTIONAL` per RFC 2119.

`Implementation-defined` means the behavior is part of the implementation's own contract; this spec does not prescribe one policy, but the implementation MUST document the choice in `docs/CONFORMANCE.md`.

`Instrument` means a non-model source of evidence: compiler, test runner, linter, schema validator, benchmark, browser probe, security scanner, filesystem, git, HTTP client, or human.

### 0.4 Defects in v1 that this version corrects

These are named because an implementer working from the older document will otherwise reproduce them.

| # | v1 defect | Why it fails | Correction |
|---|---|---|---|
| D1 | `BuilderPort::run_turn(workspace, prompt) -> Result` — a blocking call-and-return agent interface. | The primary target harness (Prime-Agent) returns an **admission handle**, not an answer: `rlm(...)` "returns immediately after task admission … it never waits for or returns the child's answer." A blocking port cannot host it without hanging or busy-waiting on an interface that will never resolve. | `AgentRunner` is **admission-based**: `invoke → handle`, `poll(handle) → state`, results delivered out-of-band through a declared **result contract** file plus an optional mailbox signal (§10.3). Blocking CLIs are the trivial adapter case; fire-and-forget harnesses are the design case. |
| D2 | `SandboxGateCheck` placed *after* `BuilderFinished`, gating "the builder's intended command". | An agentic builder executes its own commands *during* its turn. By the time the builder has finished there is nothing left to gate. The tier check was auditing a fait accompli. | The **authority gate** runs before dispatch (whether it may run at all) and the **sandbox** wraps the entire builder session from process start (how contained it is while it runs). Two orthogonal checks, both before the builder's first token (§17). |
| D3 | Critic and verifier collapsed into one "verification" step whose verdict closed units. | A model comparing artifacts to a reference and an instrument deciding whether a claim meets its evidence requirement are different operations with different failure modes. Collapsing them lets a persuasive critic close a unit no instrument ever checked. | `Comparator` produces **residuals**; `Verifier` produces **PASS / FAIL / INCONCLUSIVE** against declared evidence. Settlement requires verifier evidence, never comparator opinion alone (§13, §14, §16). |

Two further changes of posture: the dashboard is demoted from REQUIRED core to one `OperatorSurface` adapter (§20) because it is not part of the control invariant; and the answer key gains explicit evidence *requirements*, not just an evidence *field*, so "evidence: (pending)" can never satisfy a check (§9.6).

### 0.5 Provenance note

Prime-Agent facts in §10.5 and Annex C were verified against the upstream repository documentation at time of writing. Wayfinder facts in §9.2 and Annex D were verified against the upstream skill source. Sandbox backend facts in Annex F are the least stable input in this document and are marked as such; treat them as configuration, not architecture.

---

## 1. Canonical vocabulary

One name per concept. These names are load-bearing across code, schemas, logs, and prompts. Substituting a near-synonym is a defect, not a style choice.

| Term | Meaning | Not |
|---|---|---|
| **Reference** | The frozen representation used as the comparison standard for a run. | goal, plan, prompt, spec-in-progress |
| **Reference Manifest** | The machine-readable serialization of the reference. | the map, the plan |
| **Answer Key** | The projection of the reference into individually checkable, typed **Units**. | a checklist of tasks |
| **Unit** | One checkable claim with a verification method, blast radius, authority, and evidence requirement. | a ticket, a task, an issue |
| **Transform** | An attempted change to the target system by a builder. | a commit, a diff |
| **Observation** | Evidence collected from resulting reality by an instrument. | the builder's report |
| **Observation Bundle** | The normalized, provenance-carrying set of observations for one iteration. | logs |
| **Comparison** | The operation identifying relevant difference between observation and reference. | review, opinion |
| **Residual** | A materialized difference requiring disposition. | a bug, a TODO |
| **Verification** | Evaluation of a specific claim against its declared evidence requirement. | comparison, code review |
| **Correction** | The selected transformation intended to reduce relevant error. | a fix |
| **Settlement** | A consequential outcome supported by the evidence the reference required. | done, output, green build |
| **Capability** | Repeated reliable settlement under variation, with recovery and decreasing orchestration burden. | tool access, one success |
| **Role** | A named, permissioned agent function (orchestrator, builder, critic, verifier, observer, optimizer). | a model, a persona |
| **Role Invocation** | One admitted execution of a role, with its own context, permissions, budget, and result contract. | a turn, a message |
| **Run** | One bounded execution of the loop against one frozen reference. | a session |
| **Iteration** | One `observe → compare → verify → select → correct` cycle within a run. | a round trip |

Forbidden substitutions: goal→reference, review→verification, output→settlement, access→capability, opinion→evidence, plan→ruler.

---

## 2. Governing idea and the control invariant

A Gauntlet is not an agent, a prompt, a critic, a model, a skill, a set of MCP tools, a graph shape, or a test suite. **A Gauntlet is a governed comparison-and-correction protocol.**

The core control invariant, which every conforming implementation preserves:

```text
REFERENCE → TRANSFORM → OBSERVE → COMPARE → VERIFY → SELECT → CORRECT → SETTLE → REMEMBER
```

Expanded, with terminal states:

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

The system's optimization target is **not** maximum autonomy. It is *the smallest trustworthy Gauntlet that can settle enough work to earn its next increase in capability.*

---

## 3. Non-negotiable invariants

Each invariant has an identifier, an enforcement mechanism, and a proof obligation. Prompt-level enforcement alone is never sufficient where a structural mechanism is available.

| ID | Invariant | Structural enforcement (REQUIRED) | Proof |
|---|---|---|---|
| **I1** | The builder never modifies the standard its work is evaluated against. | Reference resolved, normalized, digested, and frozen at run start; frozen copy stored in the evidence store outside every builder-writable path; digest re-verified before settlement; mismatch invalidates the run. | P1 |
| **I2** | Observation is not the builder's completion claim. | The `Observer` port collects from instruments; builder-authored text enters an Observation Bundle only in a `claims` facet explicitly excluded from verification input. | P3 |
| **I3** | Critic ≠ verifier. | Distinct ports, distinct record types, distinct outputs (`Residual` vs `PASS/FAIL/INCONCLUSIVE`). Settlement reads verification records; comparator verdicts alone cannot close a Unit. | P11 |
| **I4** | The builder cannot review itself. | The comparison envelope (§13.2) is constructed by the runtime from a fixed allowlist of fields; builder trajectory, reasoning, identity, and attempt count are structurally absent, not merely omitted by prompt. | P2 |
| **I5** | Similarity never authorizes. | Model judgment, retrieval scores, and resemblance may only produce candidates and residuals. Settlement, authority, budget, security, and irreversibility gates read typed fields, never model prose. | P5 |
| **I6** | No false capability credit. | Promotion to reusable capability requires N distinct settlements under recorded variation (`GAUNTLET_PROMOTION_MIN_SETTLEMENTS`, default 3) plus a recovery event; one success writes developmental memory only. | P10 |
| **I7** | Self-improvement is governed structural revision. | Optimizer output is `CandidateRevision` only. Promotion requires isolated evaluation, held-out comparison, regression non-inferiority, and rollback validity. The active system never moves its own ruler and then claims improvement. | P9 |
| **I8** | Missing evidence is not passing evidence. | `PASS`, `FAIL`, `INCONCLUSIVE` are distinct states in the type system. `INCONCLUSIVE` never satisfies a settlement requirement. Absent evidence is recorded in `unavailable_evidence`, never defaulted. | P12 |
| **I9** | Authority gates whether it runs; sandbox gates how contained it is. | Both checks precede builder process start, in that order, and neither substitutes for the other. `irreversible` units are never automatic dispatch candidates at the scheduler level. | P5 |
| **I10** | The runtime is bounded even when configuration is incomplete. | Every budget dimension has a conservative default; absence of a configured budget is never permission for unlimited recursion, parallelism, or spend. | P6 |
| **I11** | Critical state is never reconstructed from conversational memory. | Durable append-only event log + state store; every iteration boundary is a checkpoint; resume reads the log, not a transcript. | P7 |
| **I12** | Core control logic depends on semantic ports, never on products. | Domain and application layers have zero compile-time dependency on any provider, harness, MCP client, database, browser, or sandbox implementation; conformance suite runs against ≥2 `AgentRunner` adapters. | P8 |

---

## 4. System overview

### 4.1 Layering (dependencies point inward)

```text
              ┌──────────────────────────────────────────────────────────┐
 adapters  →  │  prime-agent · cli-agent · mock-agent · git · test-runner │
              │  browser · postgres · fs-evidence · mxc/bwrap/none sandbox  │
              │  json-api · tui · dashboard · otel/openobserve logging    │
              └───────────────────────────┬──────────────────────────────┘
                                          │ implements
              ┌───────────────────────────▼──────────────────────────────┐
 ports     →  │ AgentRunner Observer Comparator Verifier ReferenceProvider│
              │ StateStore EvidenceStore EventLog ToolProvider Sandbox    │
              │ BudgetMeter Clock IdGenerator WorkspaceProvider           │
              │ HumanEscalation OperatorSurface PolicyProvider            │
              └───────────────────────────┬──────────────────────────────┘
                                          │ used by
              ┌───────────────────────────▼──────────────────────────────┐
 application  │ RunController · IterationEngine · UnitScheduler           │
              │ CorrectionSelector · SettlementService · PromotionService │
              │ ReferenceFreezer · EnvelopeBuilder · DiagnosticEngine     │
              └───────────────────────────┬──────────────────────────────┘
                                          │ operates on
              ┌───────────────────────────▼──────────────────────────────┐
 domain       │ Reference · AnswerKey/Unit · Run · Iteration · Observation│
              │ Comparison/Residual · Verification · Settlement · Budget  │
              │ Authority · CapabilityLedger · CandidateRevision          │
              └──────────────────────────────────────────────────────────┘
```

`domain` MUST NOT import from `application`, `ports`, or `adapters`. `application` MUST NOT import from `adapters`. This is a compile-enforced boundary (§24.3), not a convention.

### 4.2 Phase structure of an operator session

| Phase | Entry condition | Produces | Section |
|---|---|---|---|
| **Formation** | No sufficient reference exists for the desired transformation. | `map.md` (decisions + fog) and `answer-key.md` (typed Units) | §9.2 |
| **Freeze** | A sufficient reference exists. | `ReferenceVersion` + digest, immutable for the run | §9.4 |
| **Loop** | Reference frozen, budget allocated. | Iterations: observation, comparison, verification, correction | §11–§15 |
| **Settlement** | All blocking Units resolved or a terminal condition reached. | `SettlementRecord`, evidence bundle, run report | §16 |
| **Remember** | Run reached any terminal state. | Developmental memory; capability promotion only if earned | §19 |

Formation is not optional bureaucracy: **if the reference is insufficient, the runtime MUST NOT fabricate one silently.** It enters formation, or blocks.

---

## 5. Domain model (DDD)

Everything in this section is typed. Stringly-typed state for any field with a fixed value set is a conformance failure.

### 5.1 Bounded contexts

| Context | Aggregate root | Owns | Talks to other contexts via |
|---|---|---|---|
| Reference | `Reference` | manifest, versions, digests, units, comparison contract | frozen `ReferenceVersion` value |
| Execution | `Run` | iterations, role invocations, workspaces, budgets | `RunId`, events |
| Observation | `ObservationBundle` | facets, provenance, gaps | bundle digest + evidence refs |
| Comparison | `ComparisonRecord` | residuals, hard-gate results, verdict | residual set |
| Verification | `VerificationRecord` | claims, evidence, outcomes | outcome set |
| Settlement | `SettlementRecord` | closures, ledger entries | events |
| Governance | `AuthorityDecision`, `SandboxPolicy`, `Budget` | permissions, tiers, ceilings | decisions (allow/deny/escalate) |
| Memory | `CapabilityLedger`, `CandidateRevision` | developmental memory, promotions | promotion decisions |

Contexts communicate through explicit ports and immutable value objects. No context reaches into another's storage.

### 5.2 Value objects (enums and newtypes — REQUIRED)

```text
RunId              := ULID (lexicographically sortable, collision-resistant)
IterationNo        := u32 (1-based)
UnitId             := /^[A-Z]{2,8}-[0-9]{3,6}$/            e.g. AK-014
Digest             := algo ":" hex        (default sha256; algorithm is configuration)
RoleId             := orchestrator | builder | critic | verifier | observer | optimizer
                      | custom(name)                        -- name matches [a-z][a-z0-9-]{1,31}

VerifyBy           := build | test | static_check | benchmark | visual_diff
                      | runtime_probe | schema_check | security_scan | human_signoff
BlastRadius        := contained | local | cross_cutting | irreversible
Authority          := critic | lead_agent | human
UnitStatus         := open | in_progress | settled | blocked | out_of_scope
Verdict            := pass | fail | inconclusive
Outcome            := PASS | FAIL | INCONCLUSIVE
Severity           := blocking | major | minor | informational
TerminalState      := settled | blocked | budget_exhausted | escalated | aborted | inconclusive
ReviewMode         := single | fanout | adaptive | high_assurance
InvocationState    := admitted | running | completed | failed | timed_out | cancelled
EvidenceKind       := file | diff | test_report | log | metric | screenshot | scan
                      | transcript | external | attestation
```

Rules:
- Unknown enum values in parsed input are a **parse error**, never a coerced default. These fields gate execution safety; lenient coercion is inappropriate.
- `Digest` carries its algorithm. A bare hex string is invalid.
- `Outcome` is a three-state type. No implementation may provide `impl From<Outcome> for bool`.

### 5.3 Aggregates

#### 5.3.1 `Reference`

```yaml
reference:
  id:                 # stable identifier for this reference
  version:            # monotonic integer or semver
  digest:             # Digest over the normalized manifest
  source_refs: []     # paths/URLs the manifest was derived from
  purpose:            # one paragraph: what reaching the destination means
  desired_state:      # narrative destination (from map.md Destination)
  invariants: []      # statements that must hold at all times
  constraints: []     # bounds on how, not what
  anti_goals: []      # explicitly rejected outcomes
  units: []           # AnswerKey entries (§5.3.2) — the checkable projection
  quality_dimensions:
    - id:
      description:
      desired_condition:
      evidence_required: []
      comparison_method:   # how a comparator judges this dimension
      threshold:
      weight:
      blocking:            # bool — a blocking dimension cannot be averaged away
  settlement:
    success_criteria: []
    required_evidence: []
    minimum_reliability:   # e.g. "3 consecutive clean verification passes"
    terminal_conditions: []
  unknowns: []             # named, not silently resolved
  assumptions: []          # recorded, revisitable
```

Aggregate invariants:
- A `Reference` is immutable once frozen into a `ReferenceVersion`. Mutation produces a new version with a new digest.
- Every `blocking: true` quality dimension and every hard-gate Unit MUST be individually representable in the answer key; **a weighted score never absorbs a hard requirement.** 97/100 does not compensate for a violated invariant.
- `unknowns` MUST NOT appear in `units` as if settled.

#### 5.3.2 `Unit` (answer-key entry)

```yaml
id: AK-014
claim: <assertion that is true or false of the finished system>
verify_by: test
blast_radius: local
authority: critic
evidence_required:                    # REQUIRED — what would prove it
  kind: test_report
  locator: tests/leave/date_validation.rs::rejects_past_dates
  producer: instrument                # instrument | human   (never: model assertion)
evidence: null                        # filled at settlement with an EvidenceRef
status: open
depends_on: [AK-011]                  # ordering only; not authority
attempt: 0
last_residual_ref: null
notes: null
```

Aggregate invariants:
- `evidence_required.producer` MUST be `instrument` unless `verify_by = human_signoff`. A Unit whose only possible producer is a model is **not a Unit**; it is a quality dimension.
- `blast_radius = irreversible` ⟹ `authority = human`. Any parse producing another combination is an error.
- `blast_radius ∈ {cross_cutting, irreversible}` ⟹ authority MUST NOT be `critic`, regardless of `verify_by`.
- `status = settled` requires a non-null `evidence` ref **and** a `VerificationRecord` with `Outcome = PASS` for this unit id in this run.
- A Unit's tier MAY be raised by an operator. It MUST NOT be lowered by any agent role. Tier downgrade is the canonical gaming move (§23.2, G7).

#### 5.3.3 `Run`

```yaml
run:
  id: 01J8...              # ULID
  created_at:
  reference_version:       # id + digest, frozen
  intent:                  # operator's requested transformation, verbatim
  config_fingerprint:      # digest of effective config (§8.5)
  environment_fingerprint: # OS, toolchain versions, adapter identities
  budget:                  # Budget VO, see 5.3.7
  iterations: []           # Iteration entities
  units_state:             # UnitId -> UnitStatus (projection, rebuilt from events)
  terminal_state: null
  parent_run: null         # for candidate-revision evaluation runs
```

Invariants: a `Run` has exactly one frozen `reference_version` for its whole life. Changing the reference requires a new run with `parent_run` set.

#### 5.3.4 `Iteration`

```yaml
iteration:
  run_id:
  no: 3
  selected_correction:     # what this iteration attempted, and why (§15)
  role_invocations: []     # RoleInvocation entities
  observation_bundle_ref:
  comparison_record_ref:
  verification_record_refs: []
  residuals_opened: []
  residuals_closed: []
  payment:                 # tokens, wall seconds, tool calls, cost
  checkpoint_at:           # durable checkpoint written here (I11)
```

#### 5.3.5 `RoleInvocation`

```yaml
role_invocation:
  id:                      # ULID
  run_id: ; iteration_no:
  role: builder            # RoleId
  runner: prime-agent      # adapter identity (configuration, not architecture)
  model:                   # resolved model identity, recorded for provenance
  permissions:             # resolved PermissionSet (§17.2)
  context_envelope_digest: # digest of exactly what was sent (I4 evidence)
  workspace_ref:
  sandbox_policy_ref:      # null only when tier=contained and backend=none
  result_contract_path:    # where the child MUST write its result (§10.3)
  state: admitted          # InvocationState
  admitted_at: ; settled_at:
  payment:
  session_ref:             # adapter-native session dir / id, for forensics
```

#### 5.3.6 `ObservationBundle`, `ComparisonRecord`, `VerificationRecord`, `SettlementRecord`

Schemas in Annex B. Domain invariants:
- `ObservationBundle` MUST record `unavailable_evidence` and `observation_failures` explicitly. A bundle that silently omits a failed probe is invalid (I8).
- `ComparisonRecord` MUST carry `reference_digest` and `observation_digest`; a record whose `reference_digest` differs from the run's frozen digest invalidates the iteration (I1).
- `VerificationRecord` binds one claim to one evidence ref and one `Outcome` produced by a named instrument.
- `SettlementRecord` is append-only and references the event-log offset that made it durable.

#### 5.3.7 `Budget`

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

Invariant: the controller MUST refuse to dispatch any invocation that would consume the recovery reserve. The reserve exists so that a run can always afford to checkpoint, roll back, and report.

#### 5.3.8 `CapabilityLedger` / `CandidateRevision`

```yaml
capability:
  id: ; description:
  settlements: []          # run refs demonstrating settlement
  variation_axes: []       # what differed across those settlements
  recovery_events: []
  orchestration_burden: [] # trend, not a single number
  status: observed | provisional | promoted | revoked

candidate_revision:
  id: ; target:            # prompt | role | skill | routing | topology | verifier | ...
  claimed_problem: ; supporting_evidence: []
  predicted_improvement: ; predicted_regression_risk: []
  diff_ref: ; evaluation_run_ids: []
  held_out_result: ; regression_result: ; protected_capabilities_result:
  decision: pending | promoted | rejected | provisional
  rollback_ref:
```

---

## 6. Ports

Signatures are pseudo-typed and language-agnostic. Every port returns a typed error enum (§21.2), never a string. Every port that can block has a deadline parameter. **No port method takes or returns a provider-specific type.**

### 6.1 `AgentRunner` — admission-based (see D1)

```text
invoke(spec: RoleInvocationSpec) -> Result<InvocationHandle, AgentError>
poll(handle: &InvocationHandle) -> Result<InvocationStatus, AgentError>
collect(handle: &InvocationHandle) -> Result<RoleResult, AgentError>   # after Completed
cancel(handle: &InvocationHandle, reason) -> Result<(), AgentError>
capabilities() -> RunnerCapabilities
```

```text
RoleInvocationSpec {
  role, prompt, context_envelope, workspace, permissions,
  result_contract_path, deadline, model_hint, sandbox_policy, budget_slice
}
InvocationStatus := Admitted | Running{last_event_at, payment_so_far}
                  | Completed{result_ref} | Failed{class} | TimedOut | Cancelled
RunnerCapabilities {
  supports_native_subagents: bool,     # can it fan out inside itself?
  supports_streaming_events: bool,
  supports_model_selection: bool,
  isolation: none | process | container | vm,
  result_delivery: file | mailbox | return_value,
  max_depth: u8
}
```

Rules:
- `invoke` MUST return promptly after admission. It MUST NOT block on completion.
- The runtime determines completion by the **result contract** (§10.3), never by parsing free-form agent prose.
- `capabilities()` drives scheduling decisions. A runner reporting `isolation: none` MUST NOT be selected for a tier requiring containment (§17.3); that is a configuration error surfaced by `gauntlet doctor`.

### 6.2 Remaining ports

| Port | Core methods | Notes |
|---|---|---|
| `ReferenceProvider` | `resolve(intent) -> ReferenceDraft` · `normalize` · `digest` · `freeze(run_id) -> ReferenceVersion` · `verify_unchanged(version)` | `verify_unchanged` is the I1 tripwire; called before settlement and at every iteration boundary. |
| `Observer` | `observe(request: ObservationRequest) -> ObservationFacet` · `facet_kind()` | One adapter per instrument class (versioning, fs, build, test, bench, browser, http, scanner). Facets merge into a bundle. |
| `Comparator` | `compare(envelope: ComparisonEnvelope) -> ComparisonRecord` | Envelope is built by the runtime (I4). Comparator implementations MAY be model-backed or deterministic. |
| `Verifier` | `verify(claim: VerificationRequest) -> VerificationRecord` · `is_deterministic()` | Deterministic verifiers are tried first (§14.2). |
| `StateStore` | `append_event` · `load_run` · `checkpoint` · `resume(run_id)` · `export_jsonl` | Default adapter: PostgreSQL (`gauntlet-adapter-state-postgres`; the local default is the pg0 instance from `just db-start`). Export is deterministic. |
| `EvidenceStore` | `put(bytes, kind) -> EvidenceRef` (content-addressed) · `get` · `pin(run_id, ref)` | Immutable. No overwrite. Pinned evidence survives cleanup. |
| `EventLog` | `append(event) -> Offset` · `read_from(offset)` · `last_offset()` | Append-only, fsync-on-append, the durability spine of §16.3. |
| `Sandbox` | `validate(policy) -> PolicyValidation` (dry run) · `wrap(command, policy) -> SandboxedCommand` · `backend_identity()` | Validation precedes every use. Failure escalates; it never falls back to unsandboxed. |
| `PolicyProvider` | `policy_for(tier, workspace) -> SandboxPolicy` · `permissions_for(role, unit) -> PermissionSet` | Resolves repo-owned policy artifacts; no policy is compiled in. |
| `WorkspaceProvider` | `create(run_id, unit_or_slice) -> Workspace` · `cleanup` · `hooks` | Optional git-worktree backing. Path-containment invariants in §17.5. |
| `ToolProvider` | `resolve(capability: SemanticCapability) -> ToolBinding` · `available() -> [SemanticCapability]` | Semantic capability names (§8.4), never vendor names. MCP is one adapter, not the interface. |
| `BudgetMeter` | `reserve(slice) -> Reservation` · `charge(reservation, payment)` · `remaining()` · `would_breach_reserve(slice) -> bool` | Every dimension in §5.3.7. |
| `Clock` | `now()` · `sleep_until` | Injectable; tests use a deterministic clock. |
| `IdGenerator` | `run_id()` · `invocation_id()` | Injectable; tests use a seeded generator. |
| `HumanEscalation` | `escalate(item) -> EscalationTicket` · `poll(ticket) -> EscalationState` | File-based default adapter (`escalations/` + operator CLI); no dependency on a chat product. |
| `OperatorSurface` | `snapshot() -> RuntimeSnapshot` · `actions()` | CLI is the reference surface. JSON API / TUI / dashboard are additional adapters. |

### 6.3 Required adapter set for conformance

| Port | Adapters REQUIRED at conformance | Adapters RECOMMENDED |
|---|---|---|
| `AgentRunner` | `mock` (scripted, deterministic) **and** ≥1 real (`prime-agent` or `cli-agent`) | both real ones |
| `Observer` | `filesystem`, `versioning`, `process/command` (build+test) | `browser`, `http`, `metrics`, `scanner` |
| `Comparator` | `model-backed` + `deterministic-diff` | multi-dimension fanout |
| `Verifier` | `command-exit-code`, `test-report`, `file-exists/digest` | `benchmark`, `schema`, `security`, `visual-diff` |
| `StateStore` | embedded SQL | remote |
| `EvidenceStore` | content-addressed filesystem | object store |
| `Sandbox` | `none` (tier `contained` only) + ≥1 real backend | platform-appropriate second backend |
| `OperatorSurface` | CLI | JSON API, TUI, dashboard |

The mock `AgentRunner` is not optional. Without it the orchestration layer cannot be tested without paying a frontier model, and every test becomes flaky and expensive — which reliably ends in the tests being skipped.

---

## 7. Adapter selection and substitution

### 7.1 Selection is configuration

Adapter choice resolves from environment and config only (§8). No adapter name appears in domain or application code. The binary MAY compile adapters behind feature flags, but selection at runtime is by identifier string resolved through a registry.

```text
GAUNTLET_AGENT_RUNNER=prime-agent          # global default
GAUNTLET_BUILDER_RUNNER=prime-agent        # per-role override
GAUNTLET_CRITIC_RUNNER=cli-agent
GAUNTLET_CRITIC_MODEL=<provider model id>
```

Resolution order for role R: `GAUNTLET_<ROLE>_RUNNER` → `GAUNTLET_AGENT_RUNNER` → the single registered runner if exactly one exists → error `no_runner_configured`.

### 7.2 Substitution proof

The conformance suite (§23) MUST be parameterized over `AgentRunner` and executed against at least two adapters, one of which is `mock`. If any test depends on a specific runner's behavior, that dependency is a boundary defect (I12).

### 7.3 Contract tests

Each port has a **shared contract test suite** that every adapter for that port MUST pass. Adding an adapter means implementing the trait and running the existing suite, not writing new bespoke tests. The contract suite for `AgentRunner` MUST include: admission returns before completion; polling reports a terminal state exactly once; a child that never writes a result contract is reported `TimedOut`, not `Completed`; cancellation is idempotent.

---

## 8. Configuration

### 8.1 Principles

- **Nothing hardcoded.** No path, model, provider, command, port number, timeout, tier policy, or tool identity is a literal in domain or application code. Conservative defaults live in one declared defaults table (§8.6) and are overridable.
- **Three layers, one order.** `built-in defaults` < `GAUNTLET.md` front matter < `.env` / process environment < CLI flags. Later wins. The effective config is digested into `Run.config_fingerprint`.
- **Secrets by indirection only.** Config values may reference `$VAR`; the runtime resolves them, validates presence, and never logs the value. Generated files never contain credentials. A `.env.example` with empty values is REQUIRED.

### 8.2 `GAUNTLET.md` (repository policy contract)

Discovery: `--config <path>` → `$GAUNTLET_CONFIG_DIR/GAUNTLET.md` → `./GAUNTLET.md`. Missing file is not fatal if the environment fully specifies the run; it is an error only when a required value is unresolved.

Structure: OPTIONAL YAML front matter (runtime config) followed by OPTIONAL H2 sections used as documentation. Front matter MUST decode to a map. Unknown keys are ignored for forward compatibility, but `gauntlet doctor` lists them so typos surface.

```yaml
---
reference:
  manifest: .wayfinder/reference.yaml
  answer_key: .wayfinder/answer-key.md
  map: .wayfinder/map.md
  comparison_contract: .gauntlet/comparison-contract.yaml
  digest_algo: sha256
roles:
  dir: .agents/roles                  # role definitions (§10.1)
runners:
  default: $GAUNTLET_AGENT_RUNNER
  by_role:
    builder:  { runner: prime-agent, model: $GAUNTLET_BUILDER_MODEL }
    critic:   { runner: prime-agent, model: $GAUNTLET_CRITIC_MODEL }
    verifier: { runner: none }        # deterministic verifiers need no model
observation:
  facets: [versioning, filesystem, build, test]
review:
  mode: adaptive                      # ReviewMode
  max_critics: 4
verification:
  prefer_deterministic: true
sandbox:
  backend: $GAUNTLET_SANDBOX_BACKEND  # none | process | bwrap | seatbelt | mxc | microvm
  policy_dir: .gauntlet/policies      # one file per tier; irreversible MUST be absent
budget:
  max_rounds: 8
  max_parallel: 3
  max_depth: 2
  max_wall_seconds: 5400
  max_tokens: 2000000
  max_cost_usd: 25
  recovery_reserve_pct: 15
  diagnostic_budget: 2
workspace:
  root: $GAUNTLET_TMP_DIR/workspaces
  use_worktrees: true
state:
  backend: postgres
  dir: $GAUNTLET_STATE_DIR
evidence:
  dir: $GAUNTLET_ARTIFACT_DIR
operator:
  surfaces: [cli]                     # + json-api, tui, dashboard
---
```

### 8.3 Environment variables (canonical set)

Full table in Annex E. All are optional; all have defaults; none may be required for a bounded run to start safely.

Groups: workspace paths (`GAUNTLET_PROJECT_ROOT`, `_CONFIG_DIR`, `_STATE_DIR`, `_ARTIFACT_DIR`, `_PLAN_DIR`, `_REPORT_DIR`, `_TMP_DIR`); reference (`_REFERENCE_PATH`, `_REFERENCE_MANIFEST`, `_REFERENCE_DIGEST`, `_ANSWER_KEY_PATH`, `_MAP_PATH`, `_COMPARISON_CONTRACT`, `_ACCEPTANCE_PATH`, `_CONTEXT_PATHS`, `_CANON_PATHS`, `_HELD_OUT_PATH`, `_REGRESSION_PATH`); runners (`_AGENT_RUNNER`, `_<ROLE>_RUNNER`, `_<ROLE>_MODEL`); tools (`_MCP_CONFIG`, `_TOOL_REGISTRY`, `_MCP_ALLOWLIST`, `_MCP_REQUIRED`, `_TOOL_<CAPABILITY>`); limits (`_INTERACTIVE`, `_MAX_ROUNDS`, `_MAX_PARALLEL`, `_MAX_DEPTH`, `_MAX_WALL_SECONDS`, `_MAX_TOKENS`, `_MAX_COST_USD`, `_RECOVERY_RESERVE_PCT`, `_DIAGNOSTIC_BUDGET`, `_REVIEW_MODE`, `_ALLOW_INSTALL`, `_ALLOW_NETWORK`, `_ALLOW_DESTRUCTIVE`, `_GIT_STRATEGY`, `_ALLOW_ORCHESTRATOR_IMPLEMENTATION`); containment (`_SANDBOX_BACKEND`, `_SANDBOX_POLICY_DIR`); persistence (`_STATE_BACKEND`, `_EVIDENCE_BACKEND`); surfaces (`_OPERATOR_SURFACES`, `_API_BIND`, `_LOG_FORMAT`, `_LOG_SINK`).

### 8.4 Semantic tool capabilities

Tools bind by **semantic capability**, never by product:

```text
code_nav · filesystem · versioning · test · build · browser · visual · web · research
security · sandbox · memory · evidence · domain · policy · benchmark · schema
```

`GAUNTLET_TOOL_CODE_NAV=rust-analyzer` and `GAUNTLET_TOOL_CODE_NAV=<some MCP server>` are the same decision at different addresses. A binding may resolve through an MCP server, a CLI, a local library, or a harness-native tool. **Do not couple semantic capabilities to MCP**; MCP is one projection of a tool interface.

If `GAUNTLET_MCP_REQUIRED` names a capability that cannot be resolved, `gauntlet doctor` fails and the run does not start.

### 8.5 Validation and preflight (`gauntlet doctor`)

REQUIRED checks before any run starts, and re-checked at every iteration boundary for the subset that can drift:

1. Config parses; unknown keys reported; `$VAR` indirections resolve; secrets present but unprinted.
2. Reference manifest and answer key parse; every Unit validates against §5.3.2 including the tier/authority coupling rule.
3. A runner resolves for every role the configured `ReviewMode` can dispatch.
4. Runner capabilities satisfy configured tiers (§17.3); otherwise `sandbox_capability_mismatch`.
5. Sandbox policies exist for `contained`, `local`, `cross_cutting` when `backend != none`; **no policy exists for `irreversible`** (its presence is a config error, §17.3).
6. Every semantic capability in `GAUNTLET_MCP_REQUIRED` resolves.
7. State dir, evidence dir, workspace root writable; answer key path **not** inside any builder-writable workspace.
8. Budget dimensions parse and are non-zero; recovery reserve < 100%.

Output is a machine-readable report at `$GAUNTLET_REPORT_DIR/doctor.json` plus a human summary. Exit non-zero on any REQUIRED failure.

### 8.6 Defaults table (conservative bootstrap)

| Setting | Default | Rationale |
|---|---|---|
| `max_rounds` | 8 | bounded even if unset |
| `max_parallel` | 2 | parallel writers corrupt each other's assumptions |
| `max_depth` | 1 | recursion is raised deliberately, never by default |
| `max_wall_seconds` | 3600 | |
| `recovery_reserve_pct` | 15 | a run must always afford to stop cleanly |
| `diagnostic_budget` | 2 | diagnosis is bounded too |
| `review.mode` | `adaptive` | pay for judgment only where instruments cannot decide |
| `sandbox.backend` | `none` | with tier `contained` only; any higher tier without a backend refuses to dispatch |
| `interactive` | auto-detect TTY | unattended runs record decisions instead of asking |
| `git_strategy` | `worktree` | isolation without cloning cost |

---

## 9. Reference: formation, projection, freezing, integrity

### 9.1 The most important artifact is not the prompt

It is the reference. Everything downstream — what is observed, what is compared, what settles — is a projection of it. A weak reference produces a confident system that settles the wrong thing.

### 9.2 Formation (Wayfinder-Gauntlet)

Formation runs when no sufficient reference exists. It is a **grilling** process that converts a loose intent into (a) a decision map with explicit fog and (b) a typed answer key.

The runtime uses a repo-owned skill, `wayfinder-gauntlet`, derived from the upstream `wayfinder` skill (Annex D specifies the exact delta). The upstream skill's semantics that MUST be preserved:

- **Destination first.** Naming what reaching the end looks like is the first act; it fixes the scope every later question is measured against.
- **Fog of war.** Anything not yet sharp enough to state as a precise question stays in `Not yet specified` rather than being guessed into the answer key. The test for fog-versus-item is *whether you can state the question precisely now*, not whether you can answer it.
- **Out of scope** is a separate section from fog; scope, not sharpness, lands work there, and it never graduates.
- **HITL vs AFK.** Conversational resolution requires a real human. An agent that answers its own grilling questions has broken the skill. Unattended formation MUST record unresolved questions rather than inventing answers.

The delta from upstream:

| Upstream | Wayfinder-Gauntlet | Why |
|---|---|---|
| Map + tickets as issue-tracker child issues | Two local files: `.wayfinder/map.md`, `.wayfinder/answer-key.md` | The Gauntlet's writer-authority model (§16.4) needs a small structured file with a single writer, not a tracker API. |
| Ticket body = a question, answer recorded on resolution | Answer-key entry = a typed `Unit` (§5.3.2) | A bare assertion is gradeable by eyeball. A typed Unit binds the verification method and evidence requirement to the claim. |
| Tracker labels for type | `verify_by` / `blast_radius` / `authority` / `evidence_required` fields | Execution safety fields must be enums, not labels. |
| Blocking edges via tracker relations | `depends_on` list | Ordering only. Dependency never confers authority. |
| Decisions-so-far index links to tickets | `map.md` Decisions section links to Unit ids | Same index semantics, local addressing. |

Formation output MUST satisfy: every entry in `Decisions so far` in `map.md` either produced ≥1 Unit or is recorded as context-only; nothing in `Not yet specified` appears in the answer key; nothing in `Out of scope` appears in either.

Formation ends when the desired state is specific enough to produce useful comparison — not when it is complete. Remaining fog is a first-class recorded output, not a failure.

### 9.3 Projection: reference → answer key

The answer key is a projection, not a rewrite. Every Unit traces to a manifest element (`source_refs`). Projections are pluggable (§9.7): the coordinate system differs by domain, but the semantic contract does not.

Hard requirements MUST NOT disappear into an averaged quality score. If a dimension is `blocking`, it appears as one or more Units with `severity: blocking`.

### 9.4 Freezing (I1)

At run start, in order:

1. `resolve` — read manifest + answer key + comparison contract from configured paths.
2. `normalize` — canonical ordering, canonical whitespace, canonical enum casing. Normalization MUST be deterministic and documented; the same logical reference always digests identically.
3. `digest` — `sha256` (algorithm configurable) over the normalized bytes.
4. `freeze` — write the normalized bytes to the evidence store, content-addressed, and record `ReferenceVersion{id, version, digest, evidence_ref}` on the run.

Enforcement, all REQUIRED:

- The frozen copy lives in the evidence store, which is **outside every builder-writable path**.
- The live answer-key file is either outside every workspace or mounted read-only into it.
- `verify_unchanged()` runs at every iteration boundary and immediately before settlement. A digest mismatch sets the run terminal state to `aborted` with class `reference_mutated` and preserves both digests as evidence.
- A run MAY produce a `CandidateRevision` proposing a new reference version. It MUST NOT adopt it mid-run.

### 9.5 Comparison contract

A separate, versioned artifact declaring *how* comparison is performed for this reference: which dimensions are instantiated, what each comparator receives, what counts as a residual, and the severity rubric. It is frozen with the reference and its version is recorded on every `ComparisonRecord`. Changing the rubric mid-run is ruler movement (I1).

### 9.6 Evidence requirements beat evidence fields

Every Unit declares `evidence_required` *before* any work exists. This is the structural fix for the failure mode where a critic writes `evidence: (pending — needs load test)` and a downstream reader treats the line as satisfied. Missing evidence is `INCONCLUSIVE` (I8), which never settles.

### 9.7 Pluggable comparison projections

| Domain | Reference side | Observed side |
|---|---|---|
| Software behavior | expected behavior | observed behavior |
| UI | design reference | rendered interface |
| Architecture | declared invariants | observed dependency structure |
| Security | security contract | attack/analysis evidence |
| Performance | budget | measured runtime |
| Semantic/domain system | declared domain meaning | observed semantics |
| Research | claim | source evidence |

If the current projection cannot discriminate better from worse, **improve the measurement representation before optimizing the implementation.** A loop that cannot tell improvement from noise is a random walk with a status bar.

---

## 10. Roles, delegation, and the agent protocol

### 10.1 Roles are repository artifacts, not harness state

Each role is a versioned file at `${roles.dir}/<role>.md` with YAML front matter:

```yaml
---
role: critic
description: Compares an observation bundle against the frozen reference and emits residuals.
runner: $GAUNTLET_CRITIC_RUNNER
model: $GAUNTLET_CRITIC_MODEL
permissions:
  filesystem: read-only
  write_paths: ["$RESULT_CONTRACT_DIR"]
  network: deny
  tools: [code_nav, filesystem]
context:
  allow: [reference, observation_bundle, comparison_contract, applicable_constraints,
          open_residuals]
  deny:  [builder_trajectory, builder_reasoning, attempt_count, builder_identity,
          prior_verdicts, operator_chat]
result_contract: comparison_record@v1
deadline_seconds: 900
---
<the role prompt body>
```

Rationale: harness-native role state (for example Prime-Agent's refinable subagent specs) is session-local and mutable by the agent. Making the file the source of truth and *installing* it into the harness keeps role definitions reviewable, diffable, and reproducible across harnesses. `gauntlet roles install` writes them into whichever harness is configured (§10.5, §10.6); `gauntlet roles verify` re-reads them back and reports drift.

The `context.deny` list is enforced by the runtime when it builds the envelope (§13.2). It is a structural allowlist, not an instruction to the model.

### 10.2 Standard roles and permissions

| Role | May modify | May read | Never |
|---|---|---|---|
| `orchestrator` | run state, plans, delegation | everything the run owns | write implementation code when delegation is available (§10.7) |
| `builder` | target implementation, its own tests, its workspace | reference (read-only), map, its unit(s), prior residuals for its unit | active reference, comparison contract, acceptance gates, evidence store, answer-key status |
| `critic` | nothing (read-only) | comparison envelope only | implementation, evidence, other roles' context |
| `verifier` | its own evidence artifacts | claim + evidence + instrument output | implementation |
| `observer` | its own facet artifacts | target system state | implementation |
| `optimizer` | proposals only | run history, metrics | the active loop |

Enforce with the environment where possible (filesystem permissions, read-only mounts, sandbox policy, separate credentials), not with prompt text alone.

### 10.3 The result contract (the fix for fire-and-forget delegation)

Because `AgentRunner::invoke` is admission-based, the runtime cannot learn a result from a return value. Every role invocation therefore declares, up front:

```text
result_contract_path := ${GAUNTLET_STATE_DIR}/runs/<run_id>/inv/<invocation_id>/result.json
```

Contract rules (REQUIRED):

1. The runtime creates the invocation directory and passes the path in the prompt and the environment (`GAUNTLET_RESULT_PATH`).
2. The child writes its typed result **atomically** (write to `result.json.tmp`, fsync, rename) and MAY additionally signal completion through the harness's native mechanism (mailbox message, exit code, event).
3. The runtime treats the invocation as `Completed` only when a well-formed result document exists and validates against the role's declared `result_contract` schema (Annex B).
4. A malformed result is `Failed{class: malformed_result}` — never partially parsed into settlement-bearing state. Model output is untrusted external input: parse, validate, then admit.
5. No result by deadline is `TimedOut`. The runtime cancels the child, preserves the session reference for forensics, and applies retry/escalation policy (§11.5).
6. The result document carries `invocation_id` and `context_envelope_digest`; a mismatch is `Failed{class: contract_mismatch}` (this is what catches a stale or cross-wired child).

This single mechanism makes fan-in uniform across harnesses: the runtime polls a directory with a deadline instead of awaiting heterogeneous handles.

### 10.4 Fan-out and fan-in

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

Concurrency rules:
- Units dispatched in parallel MUST NOT share a workspace. Parallel writers in one checkout overwrite each other's assumptions.
- `max_parallel` bounds total concurrent invocations across all roles, and `max_depth` bounds delegation depth. Both default low.
- Critic fan-out is bounded by `review.max_critics` and by the budget meter; a fan-out that would breach the recovery reserve is truncated in dimension-priority order, and the truncation is recorded as `unavailable_evidence`, not silently dropped.

### 10.5 Prime-Agent adapter (primary real runner)

Verified upstream mechanics this adapter must respect:

| Mechanism | Behavior | Consequence for the adapter |
|---|---|---|
| Delegation | `rlm("task", name=…)` inside the persistent IPython kernel spawns a real child `AgentSession` with independent context and its own session directory | Delegation is code, not a tool schema; the adapter emits a kernel cell, not a JSON tool call |
| Return semantics | The call returns an **admission handle** (`rlm_child_id`, `name`, `session_dir`, `model`) immediately; it never returns the child's answer | Never await a result from `rlm`; use the result contract (§10.3) |
| Result delivery | Children reply through `agent_message.send(..., receiver_role="parent")` or by writing files | Mailbox is a *signal*; the file is the *contract*. Treat a mailbox message with no valid result file as `Running`, not `Completed` |
| Isolation | Child context is fresh; it sees only what the parent passes | Blindness (I4) is cheap here, but still enforced by envelope construction, not by trusting the default |
| Depth | Default depth allows a root agent to create children; deeper recursion requires deliberate configuration | Map to `max_depth`; report through `RunnerCapabilities` |
| Role configuration | Harness state (prompts, memories, skills, subagent specs) is CRUD-able from the trajectory and refinable via `/refine`, session-local, rollback by id | Install roles from repo files; re-verify before each run; treat harness drift as configuration drift |
| Persistence | Daemon-owned sessions, append-only session JSONL, kernel snapshots, detach/reattach, idle children unload and reload | Record `session_dir` as `session_ref`; never rely on it for control state (I11) |
| Trust | The kernel runs model-generated code with the worker's OS permissions; it is a durable control environment, **not a security sandbox** | The adapter MUST report `isolation: none` unless it is itself wrapped by a `Sandbox` adapter. Containment is the Gauntlet's job, not the harness's |

Minimum adapter behavior: render a kernel program that (1) writes the envelope to the child's input path, (2) calls `rlm(...)` with the role prompt and the result-contract path, (3) records the returned handle fields, and (4) returns. Polling reads the result contract; `rlm.list_subagents()` is used for liveness and forensics only.

### 10.6 Other runners

- **`cli-agent`** — a generic adapter for any headless coding agent invoked as a subprocess with a prompt and a working directory (Claude Code, Codex-style CLIs, others). Command template comes from configuration: `GAUNTLET_CLI_AGENT_CMD` with `{prompt_file}`, `{workspace}`, `{result_path}`, `{model}` placeholders. Blocking processes are the easy case: `invoke` spawns, `poll` checks process state *and* the result contract, and completion still requires the contract.
- **`mock`** — replays scripted results from fixtures, including malformed results, timeouts, and reference-mutation attempts. This is the adapter the anti-gaming suite (§23.2) runs against.
- **`none`** — for roles that need no model (deterministic verifiers, filesystem observers).

### 10.7 Orchestrator implementation restraint

The orchestrator owns decomposition, contracts, acceptance, verification architecture, evidence review, and settlement decisions. It **SHOULD NOT** write implementation code itself while a delegation mechanism exists. If the environment genuinely provides none, direct implementation is permitted only when `GAUNTLET_ALLOW_ORCHESTRATOR_IMPLEMENTATION=1`; otherwise the run stops with `blocked{no_delegation_mechanism}`.

The orchestrator never delegates the question of whether the total system actually settled.

---

## 11. Orchestration

### 11.1 Run lifecycle

```text
init → doctor → freeze reference → plan slices → [iterate]* → terminal → report
```

`[iterate]` = select → dispatch → observe → compare → verify → dispose residuals → checkpoint.

### 11.2 Unit scheduling

A Unit is dispatch-eligible only if **all** hold:

- `status ∈ {open}` and not already claimed or in flight;
- every `depends_on` Unit is `settled` or `out_of_scope`;
- `blast_radius != irreversible` (scheduler-level, unconditional — §17.1);
- global and per-tier concurrency slots available;
- a runner is configured for the roles the slice needs;
- the budget meter will not breach the recovery reserve.

Ordering: ascending blast-radius risk (`contained` first), then dependency depth, then `UnitId` lexicographic. Deterministic ordering matters: a scheduler whose behavior varies run to run cannot be compared against itself.

### 11.3 Slicing

Units may be grouped into a **slice** dispatched to one builder when they are not meaningfully separable (same file, same invariant). A slice inherits the **highest** blast radius and the **most restrictive** authority of its members. Slices never mix `irreversible` members with anything.

### 11.4 Reconciliation

Every tick (`GAUNTLET_POLL_INTERVAL_MS`, default 5000):
1. Poll in-flight invocations; collect completed results.
2. Detect stalls: no event and no result for `GAUNTLET_STALL_TIMEOUT_MS` → cancel, record, retry or escalate.
3. Re-read the answer key. If an operator hand-edited a Unit that is in flight (tier, authority, claim, or status changed), cancel that attempt and re-evaluate eligibility from scratch. The in-memory snapshot is never authoritative over the file.
4. Re-verify the reference digest (I1).
5. Re-check budget; if a limit is breached, stop dispatching and drive to a terminal state.

### 11.5 Retry and escalation

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

### 11.6 Terminal states and what each requires

| Terminal | Requires |
|---|---|
| `settled` | Every blocking Unit `settled` with `PASS` verification and pinned evidence; reference digest unchanged; settlement record durable |
| `blocked` | ≥1 Unit requiring an authority the run cannot exercise; escalation ticket created |
| `budget_exhausted` | A budget dimension breached; recovery reserve used only for checkpoint + report |
| `escalated` | Operator action required and recorded |
| `inconclusive` | Diagnostic budget exhausted without resolving a divergence; evidence quarantined |
| `aborted` | Integrity violation (reference mutation, contract mismatch) or operator abort |

Every terminal state produces a report distinguishing: *implemented and proven · implemented but provisional · configured but untested · proposed · blocked · unknown.* Provisional capability is never reported as complete.

---

## 12. Observation

### 12.1 Requirement

Comparison operates on evidence collected from resulting reality — never on the builder's completion claim (I2).

### 12.2 Facets

An `ObservationRequest` names the facets to collect. Each `Observer` adapter produces one facet with provenance:

```text
versioning · filesystem · build · test · lint/static · benchmark · runtime_probe
browser/visual · http/api · logs · metrics · security_scan · external
```

Each facet records: the instrument identity and version, the exact command or query, the environment fingerprint, start/end time, exit status, and a content-addressed evidence ref for raw output. A facet with no instrument identity is invalid.

### 12.3 Gaps are first-class

`unavailable_evidence` and `observation_failures` are REQUIRED arrays. An unavailable browser probe is recorded as unavailable — it is never quietly dropped, and it never becomes a `PASS`. The builder may not fabricate missing evidence; the `claims` facet (builder-authored) is stored for forensics and is **excluded** from verification inputs by construction.

### 12.4 Reproducibility

Where practical, an observation is reproducible: the bundle contains enough to re-run the same probes on the same revision. `observation_digest` covers the normalized bundle minus timestamps.

---

## 13. Comparison

### 13.1 What a comparator does

It answers exactly one question: *how does observed reality differ from the intended reference?* It produces residuals. It never modifies implementation, never closes a Unit, and never issues a settlement.

### 13.2 The comparison envelope (structural blindness — I4)

The runtime constructs the envelope from an allowlist. The comparator receives:

```text
reference (frozen, or the relevant projection of it)
observation bundle (instrument facets only)
comparison contract (dimension, rubric, severity definitions)
applicable constraints
unresolved residuals that remain relevant
```

It does **not** receive builder conversation history, builder reasoning, implementation rationalizations, attempt counts, builder identity (when blind comparison is feasible), prior verdicts on this unit, or operator chat.

The envelope is serialized, digested, and the digest is stored on both the `RoleInvocation` and the `ComparisonRecord`. **The proof of blindness (P2) is a structural assertion over that serialized envelope**, not a claim about the prompt.

Every formal comparison uses a fresh context. A builder running its own local checks while working is useful; it is not comparison.

### 13.3 Review modes

| Mode | Behavior |
|---|---|
| `single` | One comparator over all applicable dimensions. Cheapest; use for `contained` slices. |
| `fanout` | One comparator per applicable dimension, in parallel, merged deterministically. |
| `adaptive` *(default)* | 1. Run cheap deterministic probes. 2. Determine which dimensions remain genuinely undecided. 3. Fan out only across those. 4. Merge. |
| `high_assurance` | Fan out across all dimensions plus an adversarial comparator instructed to find the strongest case that the work does **not** meet the reference. |

Do not spend frontier tokens re-deciding what an instrument already settled conclusively. Conversely, do not assign the cheapest model to judgment-critical dimensions: **judge quality can dominate total loop quality**, and a cheap judge is the most expensive false economy in the system.

### 13.4 Dimensions

Instantiate only dimensions applicable to the active reference. Candidate set: functional behavior · specification fidelity · architecture · correctness · security · performance · reliability · recovery · data integrity · accessibility · visual quality · maintainability · API contracts · domain semantics · policy compliance · evidence completeness.

### 13.5 Deterministic aggregation

Merging N comparison records MUST be deterministic and total:
- Residuals are keyed by `(dimension, expected, observed_locator)`; duplicates merge, keeping the highest severity and the union of evidence refs.
- Any `blocking` residual makes the aggregate verdict `fail` regardless of other dimensions and regardless of weighted scores.
- Disagreement between comparators on the same key is preserved as a `disputed` flag, not averaged away; disputed blocking residuals route to verification or diagnosis, never to a majority vote.
- Aggregation MUST NOT be performed by a model. It is arithmetic over typed records.

---

## 14. Verification

### 14.1 What a verifier does

It answers exactly one question per claim: *does the declared evidence satisfy the declared requirement?* Output is `PASS`, `FAIL`, or `INCONCLUSIVE`, plus the evidence ref and the instrument identity.

### 14.2 Deterministic first

For each claim, select the cheapest verifier that can conclusively decide it:

```text
file/digest check → schema check → compiler/type check → focused test → broad test
→ static analysis → benchmark → runtime probe → visual diff → model judgment → human
```

Never ask a model to judge what a compiler, test, schema validator, policy checker, benchmark, or browser probe establishes more reliably. Model-backed verification is legitimate only where the property is genuinely not deterministically measurable — and such a claim SHOULD be re-expressed as a quality dimension rather than a Unit (§5.3.2).

### 14.3 Evidence binding

A `VerificationRecord` is valid only if:
- the evidence ref resolves in the evidence store;
- the evidence's producing instrument matches `evidence_required.producer`;
- the evidence was produced in this run at or after the transform it certifies (no stale evidence);
- the locator matches `evidence_required.locator` (exact for deterministic kinds; contract-defined for others).

Otherwise the outcome is `INCONCLUSIVE` with class `evidence_unresolvable` / `evidence_stale` / `evidence_producer_mismatch`.

### 14.4 The three-state rule

`INCONCLUSIVE` is not `FAIL` (it does not prove the claim wrong) and is not `PASS` (it does not prove it right). It never settles a Unit. It may be resolved by producing the evidence, by improving the instrument, or by escalation — never by re-asking a model until it says yes.

---

## 15. Correction selection and diagnosis

### 15.1 The controller

Inputs: reference, residuals, verification results, current project state, payment capacity, risk state, governance constraints, recovery reserve, developmental memory.

For each candidate correction, estimate: impact · dependency order · confidence · payment · reversibility · risk · governance status · settlement accessibility · recovery cost · expected information gain.

Procedure:
1. Reject governance-blocked paths first (authority, tier, budget, irreversibility).
2. Among allowed corrections, select the **next affordable correction** that most usefully reduces error while preserving recovery capacity.
3. Prefer `bounded correction → observable consequence → useful settlement` over large speculative rewrites. A coherent bounded batch is acceptable only when the changes cannot be meaningfully separated.
4. Record the selection **and the rejected alternatives with reasons** on the iteration. A controller whose choices are unexplained cannot be improved.

It does not simply select the largest residual.

### 15.2 Diagnostic closure

When expected and observed diverge in a way that repeats, do not patch. Run a bounded diagnostic cycle:

```text
diagnosis → prediction → cheapest discriminating test → disconfirming observation → correction
```

Candidate failure loci — enumerate before choosing: bad reference · ambiguous reference · wrong observation · observation failure · incorrect comparison · critic error · verifier error · route-selection error · implementation defect · tool defect · environment drift · insufficient payment · missing capability · integration failure · architectural defect · governance conflict.

Note that six of those sixteen are defects in the *measuring apparatus*, not the implementation. A loop that only ever diagnoses "implementation defect" is not diagnosing.

Spend from `diagnostic_budget`. If nothing resolves within it: mark `INCONCLUSIVE`, quarantine the evidence, stabilize, escalate or defer. Do not invent certainty.

---

## 16. Settlement

### 16.1 What settlement requires

A Unit settles only when all hold:
1. A `VerificationRecord` with `Outcome = PASS` exists for it in this run.
2. Its evidence ref resolves and is pinned in the evidence store.
3. Its `authority` permits closure by the actor closing it (`critic` → automatic; `lead_agent` → orchestrator with recorded rationale; `human` → operator action with recorded identity).
4. No blocking residual references it.
5. The reference digest is unchanged.

A run settles only when every blocking Unit is settled and the reference's `settlement.minimum_reliability` condition is met.

### 16.2 Authority is not containment

`authority` decides *whether* something may close. Containment decides *how contained the work was*. A perfectly sandboxed `cross_cutting` action that skipped human review is still a `cross_cutting` action that skipped human review — with better cleanup.

### 16.3 Atomic commit sequence

In order, no reordering permitted:

```text
1. EventLog.append(SettlementEvent{unit_id, evidence_ref, verification_ref, digest, ts})  # durable, fsync
2. StateStore/answer-key write: status=settled, evidence=<ref>                             # file/db write
3. In-memory advance: release claim, update projections, notify surfaces                   # volatile
```

Recovery: a crash between 1 and 2 is repaired by replaying the log on restart and completing the write **idempotently** (settlement events are keyed by `(run_id, unit_id, verification_ref)`; replaying twice is a no-op). A crash before 1 means the unit is retried as if verification never happened. The unit MUST NOT be released in memory before step 2 completes durably.

### 16.4 Single-writer discipline

The runtime is the sole writer of Unit status and evidence fields. Builder and critic roles MUST NOT write the answer key even with general filesystem access; the file lives outside builder-writable workspaces or is mounted read-only. This is what makes the audit trail structural rather than aspirational — an agent that can rewrite the line that grades it has no grader.

---

## 17. Governance: authority, containment, budget

### 17.1 The authority gate (before dispatch)

```text
if unit.blast_radius == irreversible          -> never an automatic dispatch candidate
if unit.authority != critic and closure needed-> escalate before any transform
if operator_context lacks required identity   -> escalate
```

Scheduler-level, unconditional, independent of what any prompt says. A tier may be raised by an operator; it is never lowered by an agent.

### 17.2 Permission resolution

`PolicyProvider::permissions_for(role, unit)` returns a `PermissionSet`: filesystem read/write path sets, network posture, tool allowlist, credential scope, destructive-operation flag. The runtime applies it through the strongest available mechanism: sandbox policy > process user/mount options > runner-native permission API > prompt. Prompt-only enforcement MUST be recorded as `enforcement: advisory` in the invocation record so its weakness is visible in evidence.

### 17.3 Blast radius → containment tier

| Tier | Containment posture | Automatic? |
|---|---|---|
| `contained` | Ephemeral workspace, deny-by-default network, read-write limited to the workspace, no policy retention. `backend: none` permitted only here. | yes |
| `local` | Same shape as `contained`, plus explicit deny of credential paths and the repository git directory. | yes |
| `cross_cutting` | Tightest available real backend; explicit host allowlist instead of blanket network only if the claim requires it; **host-mutating fallbacks explicitly refused** rather than left at the backend's default consent. Runner must report `isolation ≥ process`. | only with `authority` satisfied — which for this tier is never `critic` |
| `irreversible` | **No policy exists.** The action never reaches the sandbox port unattended. | no |

The absence of an `irreversible` policy file is a *requirement*, not an omission: writing one implies a path that must not exist. `gauntlet doctor` fails if one is present.

Write a policy per tier. Never ship a backend's bundled default policy unmodified, regardless of backend — the realistic risk in current sandbox tooling is over-permissive default profiles, not weak isolation primitives.

### 17.4 Budget as governance

`BudgetMeter` is consulted before every dispatch, not after. Dimensions: rounds, parallelism, depth, wall time, tokens, cost, tool invocations, retries. Breach → stop dispatching, drive to terminal, report. The recovery reserve is never spendable on forward work.

Payment includes verification cost and orchestration burden, not just builder tokens. A loop that halves builder cost by tripling orchestration cost has not improved.

### 17.5 Workspace safety invariants

1. Builder and critic processes run only inside their assigned workspace path.
2. Workspace paths stay inside `workspace.root`, verified by absolute-path prefix check after canonicalization (symlink escape is checked, not assumed absent).
3. Workspace keys are sanitized to `[A-Za-z0-9._-]`.
4. Every builder invocation above `contained` is wrapped by the tier's sandbox **from process start**.
5. The evidence store, state store, frozen reference, and answer key are outside every builder-writable path.

---

## 18. Persistence, state, and recovery

### 18.1 Substrate

Prefer the simplest durable local substrate the environment already supports. A PostgreSQL server is a reasonable default when structured persistent state is warranted (the local default is a pg0-managed instance, `just db-start`); it is a **`StateStore` adapter choice**, not an architectural commitment. Deterministic export to portable JSON/JSONL is REQUIRED regardless of backend.

Minimum durable records: runs · iterations · role invocations · reference versions · observations · comparisons · verification records · residuals · payments · settlements · failures · recovery events · candidate revisions · promotion decisions · escalations.

`RunId`s are collision-resistant and lexicographically sortable (ULID).

### 18.2 Checkpoints

Every iteration boundary writes a durable checkpoint containing: run id, reference digest, iteration number, unit-state projection, budget spent, in-flight invocation ids with their result-contract paths, and the event-log offset. **Critical run state is never reconstructed from conversational memory** (I11).

### 18.3 Resume

`gauntlet resume <run_id>`:
1. Replay the event log from the last checkpoint offset; complete any half-applied settlement idempotently (§16.3).
2. Re-freeze-verify the reference digest; mismatch → `aborted{reference_mutated}`.
3. Re-probe in-flight invocations: a valid result contract on disk is collected; anything else is marked `Failed{class: orphaned_by_restart}` and re-planned. No in-memory scheduler state is assumed to survive.
4. Sweep workspaces for settled/out-of-scope units.
5. Re-run preflight (§8.5) and continue.

Run identity, prior evidence, and payment history survive restart intact. Re-running settled work is a defect, not a safe default.

### 18.4 Assume these failures

Agent process dies · model times out · tool fails · MCP server disappears · network vanishes · test hangs · comparator emits malformed output · verifier disagrees with comparator · workspace becomes dirty · budget exhausted · reference invalid · environment changes mid-run. Each has a defined disposition in §11.5 and §21.

---

## 19. Memory, capability, and governed self-improvement

### 19.1 Two separate stores

| Store | Contents | Write policy |
|---|---|---|
| **Developmental memory** | Every attempt, failure, correction, comparison, payment, settlement, recovery. Raw. | Immediate, append-only |
| **Capability ledger** | Promoted skills, shortcuts, routes, prompts, model-routing rules. | Governed promotion only |

Tool access is not capability. One successful run is not capability. Capability is repeated reliable settlement under relevant variation, with recovery and decreasing orchestration burden.

### 19.2 Promotion criteria

Promote a capability only with: ≥ `GAUNTLET_PROMOTION_MIN_SETTLEMENTS` (default 3) settlements across recorded variation axes; ≥1 recovery event handled; non-increasing failure rate; **no gate erosion** (the settlements did not become easier because verification became weaker); stable or decreasing orchestration burden.

Tracked dimensions: performance · variation · transfer · recovery · orchestration burden · settlement reliability · failure rate · gate erosion · stability.

**A faster workflow that became faster by skipping verification is a regression, not learning.** This is the single most important sentence in this section.

### 19.3 Process optimizer

Introduce only after several trustworthy runs exist. It reads run evidence and asks what part of the Gauntlet itself caused avoidable payment, missed defects, false positives, false negatives, slow recovery, or unnecessary orchestration.

It produces proposals. It may not modify the active loop. Every proposal declares: claimed problem · supporting evidence · affected component · predicted improvement · potential regression · candidate change · test · held-out evaluation · rollback.

### 19.4 Self-improvement gate

```text
current → candidate → isolated evaluation → held-out comparison → regression checks
        → protected-capability non-inferiority → adopt or rollback
```

A candidate becomes active only when **all** hold: governance preserved · reference integrity preserved · settlement access preserved · rollback valid · regression suite passes · protected capabilities non-inferior · target metric improves by the declared margin · payment remains acceptable.

Evaluate against historical replay, held-out tasks, regression tasks, adversarial/gaming cases, and recovery tests. Insufficient evidence ⟹ `provisional` ⟹ not promoted.

The optimizer never modifies itself in production and then grades its own modification. The active system never moves its own ruler and then claims improvement.

### 19.5 Dogfooding protocol

Once V0 settles, use `Gauntlet(N)` to construct `Gauntlet(N+1)`. Each transition records: parent runtime version · candidate runtime version · reference digest · task · observations · comparison · verification · payment · settlement · regressions · adoption decision. **Never replace the working runtime in place**; build the next version as a candidate and promote it through §19.4.

---

## 20. Operator surfaces

### 20.1 CLI (reference surface, REQUIRED)

```text
gauntlet doctor      # preflight; machine-readable report; non-zero on failure
gauntlet init        # scaffold config, roles, skills, policies, .env.example
gauntlet reference   # form | freeze | show | digest | diff | verify
gauntlet run "<desired transformation>"
gauntlet status      # runs, units by tier, in-flight invocations, budget burn
gauntlet inspect     # run/iteration/invocation/unit/evidence detail
gauntlet verify      # re-run verification for a unit or a whole run
gauntlet resume <run_id>
gauntlet escalations # list | resolve | retry
gauntlet roles       # install | verify | list
gauntlet skills      # install | verify | list
gauntlet improve     # optimizer proposals; evaluate; promote/rollback
gauntlet export      # deterministic JSONL of a run
```

Desired experience:

```text
cd <project>
gauntlet run "<desired transformation>"
```

The runtime then discovers configuration, determines whether a sufficient reference exists, constructs context, initializes a run, freezes the reference, executes the loop, stops at settlement or a bounded terminal state, and prints a concise evidence-backed report. If the reference is insufficient it enters formation (§9.2) — it does not fabricate a reference silently.

Uncertainty resolution order during formation: existing project evidence → canonical specifications → tests → documentation → source/runtime inspection → permitted external research → bounded prototypes → explicit operator decisions. Unresolved questions persist in the map's fog section.

### 20.2 Additional surfaces (adapters, OPTIONAL)

- **JSON API** — `GET /api/v1/state|runs/{id}|units/{id}|escalated`, `POST /api/v1/refresh|units/{id}/resolve|units/{id}/retry`. Action endpoints perform the *same* atomic commit sequence as automatic paths (§16.3); they never bypass it. Errors as `{"error":{"code":"...","message":"..."}}`; unsupported methods return 405; operator identity logged on every action.
- **TUI / dashboard** — priority order: escalation queue (the operator's actionable inbox) · in-flight invocations · answer-key completion grouped by tier · budget burn · sandbox denials · recent residuals.

These are observability and action surfaces. **The control invariant does not depend on them.** The core must build and run with `operator.surfaces: [cli]`.

### 20.3 Logging

Structured, machine-parseable, with required context fields `run_id`, `iteration_no`, `invocation_id`, `unit_id` where applicable. Stable `key=value` phrasing; action outcome always included; large payloads referenced by evidence ref rather than inlined; secrets never printed. Log sinks are configuration (`GAUNTLET_LOG_SINK`), and an external observability backend is an adapter, never a dependency of the core.

---

## 21. Failure model

### 21.1 Failure classes

`configuration` · `reference` · `formation` · `workspace` · `agent_execution` · `tool_execution` · `observation` · `comparison` · `verification` · `settlement` · `persistence` · `environment` · `budget` · `governance` · `recovery` · `inconclusive_evidence`.

### 21.2 Error discipline

- Typed errors carrying the failure locus; never a bare string, never a swallowed error.
- Uncertainty is never converted into success.
- Errors preserve enough context to diagnose, recover, retry safely, or abort.
- `PASS`, `FAIL`, `INCONCLUSIVE` remain distinct through every layer.
- Model output is untrusted external input: parse → validate → then admit into consequential state.

### 21.3 Degradation rules

| Failure | Rule |
|---|---|
| Config/reference invalid | Stop new dispatch; keep the process alive; keep reconciliation running |
| Agent session failure | Retry or escalate per §11.5; never mark the unit settled |
| Sandbox failure | Escalate; **never** fall back to unsandboxed execution |
| Observation failure | Record as unavailable evidence; verification becomes `INCONCLUSIVE` for dependent claims |
| Surface/log failure | Never crash the run |
| Integrity violation | Abort the run; preserve everything |

---

## 22. Security and operational safety

1. **Trust posture is declared.** The implementation states in `docs/CONFORMANCE.md` whether it targets trusted or restrictive environments, which sandbox backends it ships, the tier mapping, and any experimental-mode consent.
2. **The agent harness is not a sandbox.** Model-generated code and shell commands run with the worker's OS permissions unless externally contained. Containment is the runtime's responsibility.
3. **Least privilege per role** (§10.2, §17.2), enforced by mechanism where available, recorded as `advisory` where not.
4. **Secrets** via `$VAR` indirection; presence validated without printing; never written into generated files, prompts, envelopes, or evidence.
5. **The grader is not writable by the graded.** Answer key, frozen reference, evidence store, and event log are outside every builder-writable path.
6. **Untrusted repositories** are run with `GAUNTLET_ALLOW_NETWORK=0`, a real containment backend, and short-lived read-only credentials.
7. **Hooks are trusted configuration**, run inside the workspace with a REQUIRED timeout and truncated output in logs.
8. **Destructive operations** require `GAUNTLET_ALLOW_DESTRUCTIVE=1` *and* an authority decision; either alone is insufficient.

---

## 23. Proof obligations, anti-gaming, and baselines

Running is not succeeding. Conformance requires the following evidence, produced by the implementation's own test suite and preserved in the evidence store.

### 23.1 Proofs

| ID | Proof | Method |
|---|---|---|
| **P1** | Reference integrity | A builder fixture attempts to modify the active reference. The attempt fails, or is detected and invalidates the run with `reference_mutated`. |
| **P2** | Context isolation | Assert structurally over the serialized comparison envelope that no builder trajectory, reasoning, identity, or attempt count is present. Fails if any deny-listed field is added. |
| **P3** | Real observation | With a builder fixture that *claims* success while producing a failing artifact, the bundle records the instrument's failure and comparison operates on it. |
| **P4** | Corrective loop | Inject a controlled defect. Observer/comparator/verifier expose it; the controller routes a corrective iteration that closes it. |
| **P5** | Hard-gate preservation | A result with high aggregate quality that violates one hard requirement is rejected. Also: an `irreversible` unit is never auto-dispatched. |
| **P6** | Budget control | A deliberately non-converging task terminates at the declared limit with the recovery reserve intact. |
| **P7** | Recovery | Kill the process after a committed iteration and mid-settlement. Resume completes the settlement exactly once and loses no run identity or evidence. |
| **P8** | Adapter substitution | The full conformance suite passes against ≥2 `AgentRunner` adapters, one of them not the mock. |
| **P9** | Self-improvement containment | An optimizer proposal that improves a metric by weakening verification fails promotion. |
| **P10** | No false learning | One successful run does not mark a capability promoted. |
| **P11** | Critic/verifier separation | A comparator returning `verdict: pass` with no verification record does not settle a unit. |
| **P12** | Missing ≠ passing | A unit whose required evidence is absent yields `INCONCLUSIVE` and does not settle, at every layer including the operator API's `resolve` action. |

Each proof MUST have a **teeth check**: disabling the defense makes the proof fail. A proof that passes with its mechanism removed is testing nothing.

### 23.2 Anti-gaming suite

Deliberately attempt each strategy; the system must reject the false improvement it produces.

| ID | Gaming strategy |
|---|---|
| G1 | Weaken acceptance criteria |
| G2 | Change the reference mid-run |
| G3 | Skip expensive verification |
| G4 | Reduce variation in evaluation |
| G5 | Overfit held-out examples |
| G6 | Hide failed observations |
| G7 | Downgrade a unit's blast radius or authority to unblock |
| G8 | Drop hard residuals |
| G9 | Let the critic edit implementation |
| G10 | Let the builder write its own answer key |
| G11 | Share context between builder and critic |
| G12 | Claim missing evidence as pass |
| G13 | Optimize score instead of settlement |
| G14 | Remove recovery checks |
| G15 | End early to save tokens while reporting settlement |
| G16 | Re-ask a model until it returns `pass` |

### 23.3 Baselines

Where affordable, evaluate on the same bounded tasks:

```text
A = builder only
B = builder + self-review
C = builder + fresh critic
D = reference-anchored Gauntlet (this system)
```

Compare held-out correctness · false-completion rate · defect escape · regressions · iterations · token/tool payment · wall-clock payment · recovery · verification burden · settlement reliability.

**Do not assume D wins. Measure it.** If D does not outperform a simpler baseline by enough to justify its additional payment, simplify the architecture. A governance apparatus that costs more than the defects it catches is a defect.

---

## 24. Conformance

### 24.1 Profiles

| Profile | Contents |
|---|---|
| **Core** (REQUIRED) | §3 invariants I1–I12; ports §6.1–6.2 with the required adapter set §6.3; config §8; reference freeze/integrity §9.4; result contract §10.3; orchestration §11; observation §12; comparison §13; verification §14; settlement §16; governance §17; persistence/recovery §18; CLI §20.1; failure model §21; proofs P1–P12; anti-gaming G1–G16. |
| **Extended** | Formation via `wayfinder-gauntlet` §9.2; critic fan-out `adaptive`/`high_assurance` §13.3; memory + promotion §19.1–19.2; JSON API/TUI/dashboard §20.2; git-worktree workspaces; observability adapter. |
| **Self-improving** | Optimizer §19.3; self-improvement gate §19.4; dogfooding §19.5; baselines §23.3. |

Extended and Self-improving MUST NOT be started before Core proofs pass. The system earns its own sophistication.

### 24.2 Reference implementation profile (default, non-normative)

Rust workspace; `just` as the repository command surface; `cargo-nextest` for test execution; `cargo-mutants` at settlement boundaries to check that tests discriminate; `clippy` in normal verification; `cargo-deny` for dependency policy; `postgres` for the default `StateStore`; typed model interfaces where a model-facing contract benefits from schema enforcement; a reproducible environment manager for toolchain pinning. Every one of these is an adapter or tool choice recorded in configuration — none appears in the domain layer.

### 24.3 Architecture boundary enforcement (REQUIRED, mechanical)

The layering in §4.1 is enforced by a test that fails the build, not by review:
- the domain crate/module declares zero dependencies on port, adapter, or infrastructure crates;
- the application layer depends on ports only;
- no adapter type name appears in domain or application source;
- a denylist test asserts that provider, harness, database, browser, and sandbox identifiers appear only under `adapters/` and configuration files.

If replacing a provider requires editing the controller, the boundary is wrong.

---

## 25. Deliverables and repository layout

```text
GAUNTLET.md                     # runtime policy contract (config front matter)
AGENTS.md                       # agent operating rules for this repository
.env.example                    # every GAUNTLET_* var, empty values
.agents/
  roles/                        # role definitions (§10.1) — source of truth
  skills/                       # wayfinder-gauntlet + companions (Annex D)
  specs/                        # this document + derived specs
  plans/                        # bounded execution plans
  reports/                      # doctor, run, handoff reports
.wayfinder/
  map.md                        # decisions + fog (formation output)
  answer-key.md                 # typed Units (the ruler projection)
  reference.yaml                # reference manifest
.gauntlet/
  policies/                     # sandbox policy per tier (no irreversible file)
  comparison-contract.yaml
<source tree>
  domain/                       # entities, value objects, invariants — no infra
  application/                  # controller, engines, services — ports only
  ports/                        # trait definitions + shared contract tests
  adapters/                     # prime-agent, cli-agent, mock, git, postgres, …
  cli/
tests/
  contract/                     # per-port adapter contract suites
  proofs/                       # P1–P12
  antigaming/                   # G1–G16
  e2e/                          # full loop, recovery, budget exhaustion
  fixtures/                     # scripted agent results incl. malformed/timeout
docs/
  ARCHITECTURE.md CONFORMANCE.md CONFIGURATION.md SECURITY.md
  FAILURE-RECOVERY.md OPERATOR-GUIDE.md SELF-IMPROVEMENT.md HANDOFF.md
  schemas/                      # Annex B schemas as machine-readable files
```

Do not scatter competing sources of truth. Generated artifacts never become a second ruler.

---

## 26. Settlement criteria for the bootstrap project

The bootstrap settles only when an operator can enter a compatible folder, provide or derive a comparison reference, run the Gauntlet through the CLI, and receive a result whose completion claim is supported by preserved evidence.

Demonstrated, with evidence: portable orchestration · reference integrity · fresh-context comparison · independent observation · explicit verification · bounded correction · recoverability · provider/tool substitution · evidence-preserving settlement · governed self-improvement.

The final report distinguishes **implemented and proven · implemented but provisional · configured but untested · proposed · blocked · unknown**. Provisional capability is never reported as complete.

---

# Annex A — Bootstrap answer key (the ruler)

Copy verbatim to `${GAUNTLET_ANSWER_KEY_PATH:-.wayfinder/answer-key.md}` and freeze before implementation begins (§0.1). Do not edit during implementation. Locators are the *intended* evidence addresses; an implementation MAY relocate a locator only by recording a `CandidateRevision` against the reference — never by silent edit.

Format: one `## <UnitId>` block per Unit; `key: value` lines; `evidence_required` in YAML flow style. `depends_on` and `notes` are optional.

```markdown
## AK-001
claim: The domain layer has zero compile-time dependencies on ports, adapters, or infrastructure.
verify_by: static_check
blast_radius: cross_cutting
authority: lead_agent
evidence_required: {kind: test_report, locator: tests/contract/architecture_boundaries, producer: instrument}

## AK-002
claim: The application layer depends on port abstractions only; no adapter identifier appears in domain or application source.
verify_by: static_check
blast_radius: cross_cutting
authority: lead_agent
evidence_required: {kind: test_report, locator: tests/contract/architecture_boundaries::denylist, producer: instrument}

## AK-003
claim: Every fixed-value-set field (VerifyBy, BlastRadius, Authority, UnitStatus, Verdict, Outcome, InvocationState, TerminalState, Severity, ReviewMode, EvidenceKind) is a distinct type, not a string.
verify_by: static_check
blast_radius: local
authority: critic
evidence_required: {kind: test_report, locator: tests/contract/typed_enums, producer: instrument}

## AK-004
claim: Parsing an unknown enum value in a Unit produces a parse error rather than a coerced default.
verify_by: test
blast_radius: local
authority: critic
evidence_required: {kind: test_report, locator: tests/contract/answer_key_parse::unknown_enum_is_error, producer: instrument}

## AK-005
claim: Outcome is a three-state type with no boolean conversion; INCONCLUSIVE never satisfies a settlement requirement.
verify_by: test
blast_radius: cross_cutting
authority: lead_agent
evidence_required: {kind: test_report, locator: tests/proofs/p12_missing_is_not_passing, producer: instrument}

## AK-006
claim: A Unit with blast_radius irreversible and authority other than human fails validation.
verify_by: test
blast_radius: local
authority: critic
evidence_required: {kind: test_report, locator: tests/contract/answer_key_parse::tier_authority_coupling, producer: instrument}

## AK-007
claim: A Unit with blast_radius cross_cutting or irreversible cannot have authority critic, regardless of verify_by.
verify_by: test
blast_radius: local
authority: critic
evidence_required: {kind: test_report, locator: tests/contract/answer_key_parse::tier_authority_coupling, producer: instrument}

## AK-008
claim: AgentRunner::invoke returns an admission handle without blocking on completion.
verify_by: test
blast_radius: cross_cutting
authority: lead_agent
evidence_required: {kind: test_report, locator: tests/contract/agent_runner::invoke_is_non_blocking, producer: instrument}

## AK-009
claim: An invocation is Completed only when a schema-valid result document exists at the declared result-contract path.
verify_by: test
blast_radius: cross_cutting
authority: lead_agent
evidence_required: {kind: test_report, locator: tests/contract/agent_runner::completion_requires_result_contract, producer: instrument}

## AK-010
claim: A child that never writes a result document is reported TimedOut, never Completed.
verify_by: test
blast_radius: local
authority: critic
evidence_required: {kind: test_report, locator: tests/contract/agent_runner::no_result_is_timeout, producer: instrument}

## AK-011
claim: A malformed result document yields Failed{malformed_result} and no settlement-bearing state is written from it.
verify_by: test
blast_radius: cross_cutting
authority: lead_agent
evidence_required: {kind: test_report, locator: tests/contract/agent_runner::malformed_result_rejected, producer: instrument}

## AK-012
claim: A result document whose invocation_id or context_envelope_digest does not match the dispatch yields Failed{contract_mismatch}.
verify_by: test
blast_radius: local
authority: critic
evidence_required: {kind: test_report, locator: tests/contract/agent_runner::contract_mismatch, producer: instrument}

## AK-013
claim: The full port conformance suite passes against at least two AgentRunner adapters, one of which is not the mock.
verify_by: test
blast_radius: cross_cutting
authority: lead_agent
evidence_required: {kind: test_report, locator: tests/proofs/p8_adapter_substitution, producer: instrument}
depends_on: [AK-008]

## AK-014
claim: Adapter selection resolves from configuration only; no adapter is chosen by a literal in domain or application code.
verify_by: static_check
blast_radius: local
authority: critic
evidence_required: {kind: test_report, locator: tests/contract/adapter_registry::selection_is_config_driven, producer: instrument}

## AK-015
claim: Configuration resolves in the order defaults < GAUNTLET.md < environment < CLI flags, and the effective configuration is digested onto the run.
verify_by: test
blast_radius: local
authority: critic
evidence_required: {kind: test_report, locator: tests/contract/config::precedence, producer: instrument}

## AK-016
claim: No path, model, provider, command, timeout, or tier policy is a hardcoded literal outside the declared defaults table.
verify_by: static_check
blast_radius: cross_cutting
authority: lead_agent
evidence_required: {kind: test_report, locator: tests/contract/no_hardcoded_literals, producer: instrument}

## AK-017
claim: gauntlet doctor performs every preflight check in the specification and exits non-zero on any required failure.
verify_by: test
blast_radius: local
authority: critic
evidence_required: {kind: test_report, locator: tests/e2e/doctor, producer: instrument}

## AK-018
claim: Secrets are referenced by indirection, validated for presence without printing, and never appear in logs, prompts, envelopes, evidence, or generated files.
verify_by: test
blast_radius: cross_cutting
authority: lead_agent
evidence_required: {kind: test_report, locator: tests/contract/secrets::never_leaked, producer: instrument}

## AK-019
claim: A .env.example exists listing every recognized GAUNTLET_ variable with empty values and no credentials.
verify_by: static_check
blast_radius: contained
authority: critic
evidence_required: {kind: test_report, locator: tests/contract/env_example_complete, producer: instrument}

## AK-020
claim: The runtime starts a bounded run when optional configuration is absent, using the conservative defaults table.
verify_by: test
blast_radius: local
authority: critic
evidence_required: {kind: test_report, locator: tests/e2e/bounded_without_config, producer: instrument}

## AK-021
claim: Reference resolution normalizes deterministically: the same logical reference always produces the same digest.
verify_by: test
blast_radius: local
authority: critic
evidence_required: {kind: test_report, locator: tests/contract/reference::normalization_is_deterministic, producer: instrument}

## AK-022
claim: The frozen reference is stored in the evidence store outside every builder-writable path.
verify_by: test
blast_radius: cross_cutting
authority: lead_agent
evidence_required: {kind: test_report, locator: tests/proofs/p1_reference_integrity::frozen_copy_outside_workspace, producer: instrument}

## AK-023
claim: A builder attempt to modify the active reference fails or is detected and aborts the run with reference_mutated.
verify_by: test
blast_radius: irreversible
authority: human
evidence_required: {kind: test_report, locator: tests/proofs/p1_reference_integrity, producer: instrument}
depends_on: [AK-022]

## AK-024
claim: The reference digest is re-verified at every iteration boundary and immediately before settlement.
verify_by: test
blast_radius: cross_cutting
authority: lead_agent
evidence_required: {kind: test_report, locator: tests/proofs/p1_reference_integrity::digest_rechecked, producer: instrument}

## AK-025
claim: A run may record a candidate reference revision but cannot adopt one mid-run.
verify_by: test
blast_radius: cross_cutting
authority: lead_agent
evidence_required: {kind: test_report, locator: tests/antigaming/g2_change_reference, producer: instrument}

## AK-026
claim: The comparison envelope is built from an allowlist and contains no builder trajectory, reasoning, identity, attempt count, or prior verdicts.
verify_by: test
blast_radius: cross_cutting
authority: lead_agent
evidence_required: {kind: test_report, locator: tests/proofs/p2_context_isolation, producer: instrument}

## AK-027
claim: Adding a deny-listed field to the envelope makes the blindness proof fail.
verify_by: test
blast_radius: local
authority: critic
evidence_required: {kind: test_report, locator: tests/proofs/p2_context_isolation::teeth, producer: instrument}
depends_on: [AK-026]

## AK-028
claim: Every observation facet records instrument identity, exact command, environment fingerprint, exit status, and a content-addressed evidence ref.
verify_by: test
blast_radius: local
authority: critic
evidence_required: {kind: test_report, locator: tests/contract/observer::facet_provenance, producer: instrument}

## AK-029
claim: Builder-authored claims are stored in a claims facet that is excluded from verification inputs by construction.
verify_by: test
blast_radius: cross_cutting
authority: lead_agent
evidence_required: {kind: test_report, locator: tests/proofs/p3_real_observation, producer: instrument}

## AK-030
claim: Unavailable evidence and observation failures are recorded explicitly and never dropped or defaulted to success.
verify_by: test
blast_radius: cross_cutting
authority: lead_agent
evidence_required: {kind: test_report, locator: tests/antigaming/g6_hide_failed_observations, producer: instrument}

## AK-031
claim: Comparators produce residuals only; a comparator verdict alone never settles a Unit.
verify_by: test
blast_radius: cross_cutting
authority: lead_agent
evidence_required: {kind: test_report, locator: tests/proofs/p11_critic_is_not_verifier, producer: instrument}

## AK-032
claim: Residual aggregation is deterministic, is not performed by a model, and any blocking residual forces an aggregate fail verdict.
verify_by: test
blast_radius: local
authority: critic
evidence_required: {kind: test_report, locator: tests/contract/comparison::deterministic_aggregation, producer: instrument}

## AK-033
claim: Adaptive review runs deterministic probes first and fans out model critics only across dimensions those probes left undecided.
verify_by: test
blast_radius: local
authority: critic
evidence_required: {kind: test_report, locator: tests/contract/review_mode::adaptive_skips_settled_dimensions, producer: instrument}

## AK-034
claim: Critic fan-out truncated by budget records the missing dimensions as unavailable evidence rather than omitting them.
verify_by: test
blast_radius: local
authority: critic
evidence_required: {kind: test_report, locator: tests/contract/review_mode::truncation_is_recorded, producer: instrument}

## AK-035
claim: Verification selects the cheapest conclusive instrument and prefers deterministic verifiers over model judgment.
verify_by: test
blast_radius: local
authority: critic
evidence_required: {kind: test_report, locator: tests/contract/verifier::deterministic_first, producer: instrument}

## AK-036
claim: A verification record is rejected as INCONCLUSIVE when its evidence is unresolvable, stale relative to the transform, or produced by a non-matching producer.
verify_by: test
blast_radius: cross_cutting
authority: lead_agent
evidence_required: {kind: test_report, locator: tests/contract/verifier::evidence_binding, producer: instrument}

## AK-037
claim: The controller records the selected correction together with rejected alternatives and their reasons.
verify_by: test
blast_radius: local
authority: critic
evidence_required: {kind: test_report, locator: tests/contract/controller::records_alternatives, producer: instrument}

## AK-038
claim: An injected controlled defect is exposed by observation and comparison and closed by a routed corrective iteration.
verify_by: test
blast_radius: local
authority: critic
evidence_required: {kind: test_report, locator: tests/proofs/p4_corrective_loop, producer: instrument}

## AK-039
claim: A repeated identical residual triggers bounded diagnosis rather than another patch, and exhausting the diagnostic budget yields INCONCLUSIVE.
verify_by: test
blast_radius: local
authority: critic
evidence_required: {kind: test_report, locator: tests/contract/diagnosis::nonconvergence, producer: instrument}

## AK-040
claim: Settlement requires a PASS verification record, a resolvable pinned evidence ref, satisfied authority, no blocking residual, and an unchanged reference digest.
verify_by: test
blast_radius: cross_cutting
authority: lead_agent
evidence_required: {kind: test_report, locator: tests/contract/settlement::preconditions, producer: instrument}

## AK-041
claim: The atomic commit sequence appends the durable event before any status write and before any in-memory advance.
verify_by: test
blast_radius: cross_cutting
authority: lead_agent
evidence_required: {kind: test_report, locator: tests/proofs/p7_recovery::commit_order, producer: instrument}

## AK-042
claim: Killing the process mid-settlement and resuming completes the settlement exactly once, with no double commit and no lost evidence.
verify_by: test
blast_radius: cross_cutting
authority: lead_agent
evidence_required: {kind: test_report, locator: tests/proofs/p7_recovery, producer: instrument}
depends_on: [AK-041]

## AK-043
claim: The runtime is the sole writer of Unit status and evidence fields; a builder or critic write to the answer key is prevented or detected.
verify_by: test
blast_radius: irreversible
authority: human
evidence_required: {kind: test_report, locator: tests/antigaming/g10_builder_writes_answer_key, producer: instrument}

## AK-044
claim: A unit tagged irreversible is never an automatic dispatch candidate at the scheduler level.
verify_by: test
blast_radius: irreversible
authority: human
evidence_required: {kind: test_report, locator: tests/proofs/p5_hard_gate::irreversible_never_dispatched, producer: instrument}

## AK-045
claim: A high-quality result that violates one hard requirement is rejected rather than averaged into a pass.
verify_by: test
blast_radius: cross_cutting
authority: lead_agent
evidence_required: {kind: test_report, locator: tests/proofs/p5_hard_gate, producer: instrument}

## AK-046
claim: An agent role cannot lower a Unit's blast radius or authority; only an operator can raise it.
verify_by: test
blast_radius: cross_cutting
authority: lead_agent
evidence_required: {kind: test_report, locator: tests/antigaming/g7_tier_downgrade, producer: instrument}

## AK-047
claim: Sandbox policy validation precedes execution, and a validation failure escalates without falling back to unsandboxed execution.
verify_by: test
blast_radius: irreversible
authority: human
evidence_required: {kind: test_report, locator: tests/contract/sandbox::no_unsandboxed_fallback, producer: instrument}

## AK-048
claim: Containment wraps the builder session from process start; no builder command executes before the authority and containment checks.
verify_by: test
blast_radius: irreversible
authority: human
evidence_required: {kind: test_report, locator: tests/contract/sandbox::wraps_from_process_start, producer: instrument}

## AK-049
claim: A sandbox policy file for the irreversible tier causes doctor to fail.
verify_by: test
blast_radius: local
authority: critic
evidence_required: {kind: test_report, locator: tests/e2e/doctor::irreversible_policy_is_error, producer: instrument}

## AK-050
claim: A runner reporting isolation none is refused for any tier above contained.
verify_by: test
blast_radius: cross_cutting
authority: lead_agent
evidence_required: {kind: test_report, locator: tests/contract/sandbox::capability_mismatch, producer: instrument}

## AK-051
claim: A non-converging task terminates at the declared budget limit with the recovery reserve unspent.
verify_by: test
blast_radius: local
authority: critic
evidence_required: {kind: test_report, locator: tests/proofs/p6_budget_control, producer: instrument}

## AK-052
claim: The budget meter is consulted before every dispatch and refuses any dispatch that would consume the recovery reserve.
verify_by: test
blast_radius: local
authority: critic
evidence_required: {kind: test_report, locator: tests/contract/budget::reserve_protected, producer: instrument}

## AK-053
claim: Every iteration boundary writes a durable checkpoint sufficient to resume without any conversational context.
verify_by: test
blast_radius: cross_cutting
authority: lead_agent
evidence_required: {kind: test_report, locator: tests/contract/state::checkpoint_sufficiency, producer: instrument}

## AK-054
claim: Run records export deterministically to JSONL; exporting the same run twice produces byte-identical output.
verify_by: test
blast_radius: contained
authority: critic
evidence_required: {kind: test_report, locator: tests/contract/state::deterministic_export, producer: instrument}

## AK-055
claim: One successful run does not promote a capability; promotion requires the configured minimum settlements under recorded variation plus a recovery event.
verify_by: test
blast_radius: local
authority: critic
evidence_required: {kind: test_report, locator: tests/proofs/p10_no_false_learning, producer: instrument}

## AK-056
claim: An optimizer proposal that improves a metric by weakening verification fails promotion.
verify_by: test
blast_radius: cross_cutting
authority: lead_agent
evidence_required: {kind: test_report, locator: tests/proofs/p9_self_improvement_containment, producer: instrument}

## AK-057
claim: The optimizer cannot modify the active loop; its output is a candidate revision requiring isolated evaluation, held-out comparison, and valid rollback.
verify_by: test
blast_radius: cross_cutting
authority: lead_agent
evidence_required: {kind: test_report, locator: tests/contract/promotion::candidate_only, producer: instrument}

## AK-058
claim: Each of the sixteen anti-gaming strategies has a test that fails when its defense is removed.
verify_by: test
blast_radius: cross_cutting
authority: lead_agent
evidence_required: {kind: test_report, locator: tests/antigaming, producer: instrument}

## AK-059
claim: The mock AgentRunner can replay scripted results including malformed output, timeout, and a reference-mutation attempt.
verify_by: test
blast_radius: contained
authority: critic
evidence_required: {kind: test_report, locator: tests/fixtures/mock_runner_scenarios, producer: instrument}

## AK-060
claim: The CLI exposes doctor, init, reference, run, status, inspect, verify, resume, escalations, roles, skills, improve, and export.
verify_by: test
blast_radius: contained
authority: critic
evidence_required: {kind: test_report, locator: tests/e2e/cli_surface, producer: instrument}

## AK-061
claim: The core builds and completes a full loop with operator surfaces limited to the CLI.
verify_by: build
blast_radius: local
authority: critic
evidence_required: {kind: log, locator: build/cli-only-profile, producer: instrument}

## AK-062
claim: Operator resolve and retry actions perform the same atomic commit sequence as automatic settlement and cannot settle a Unit lacking PASS evidence.
verify_by: test
blast_radius: cross_cutting
authority: lead_agent
evidence_required: {kind: test_report, locator: tests/contract/operator_actions::no_bypass, producer: instrument}

## AK-063
claim: Role definitions are repository files installed into the configured harness, and roles verify reports drift between the file and installed harness state.
verify_by: test
blast_radius: local
authority: critic
evidence_required: {kind: test_report, locator: tests/contract/roles::install_and_verify, producer: instrument}

## AK-064
claim: The wayfinder-gauntlet skill is installed in-repo, preserves destination-first, fog-of-war, out-of-scope, and HITL semantics, and emits map.md plus a schema-valid answer-key.md.
verify_by: test
blast_radius: local
authority: critic
evidence_required: {kind: test_report, locator: tests/contract/skills::wayfinder_gauntlet_conformance, producer: instrument}

## AK-065
claim: Unattended formation records unresolved questions as fog rather than inventing answers, and no fog entry appears in the answer key.
verify_by: test
blast_radius: local
authority: critic
evidence_required: {kind: test_report, locator: tests/contract/formation::fog_not_in_answer_key, producer: instrument}

## AK-066
claim: An end-to-end run against a seeded bounded task settles with preserved evidence for every blocking Unit, using a real AgentRunner adapter.
verify_by: test
blast_radius: cross_cutting
authority: lead_agent
evidence_required: {kind: test_report, locator: tests/e2e/full_loop_real_runner, producer: instrument}
depends_on: [AK-013, AK-040]

## AK-067
claim: The final report distinguishes implemented-and-proven, implemented-but-provisional, configured-but-untested, proposed, blocked, and unknown.
verify_by: static_check
blast_radius: local
authority: critic
evidence_required: {kind: file, locator: .agents/reports/handoff.md, producer: instrument}

## AK-068
claim: Documentation exists for architecture, conformance, configuration, security, failure/recovery, operator guide, and self-improvement, and each states which parts are proven versus provisional.
verify_by: static_check
blast_radius: contained
authority: critic
evidence_required: {kind: test_report, locator: tests/contract/docs_present, producer: instrument}

## AK-069
claim: Baseline comparison A/B/C/D has been executed on at least one bounded task and its results are recorded, including the case where D does not win.
verify_by: benchmark
blast_radius: local
authority: lead_agent
evidence_required: {kind: metric, locator: .agents/reports/baselines.json, producer: instrument}

## AK-070
claim: Mutation testing at a settlement boundary shows the proof and anti-gaming suites detect deliberately introduced defects in the mechanisms they guard.
verify_by: test
blast_radius: local
authority: lead_agent
evidence_required: {kind: test_report, locator: .agents/reports/mutation.json, producer: instrument}
```

---

# Annex B — Schemas

Machine-readable copies live under `docs/schemas/`. Every schema is versioned; the version string appears in the document (`schema: <name>@v1`) and role definitions reference it by name.

### B.1 `role_result@v1` (envelope for every result contract)

```json
{
  "schema": "role_result@v1",
  "invocation_id": "01J8...",
  "run_id": "01J8...",
  "iteration_no": 3,
  "role": "builder",
  "context_envelope_digest": "sha256:...",
  "state": "completed",
  "payload_schema": "builder_result@v1",
  "payload": { },
  "payment": {"input_tokens": 0, "output_tokens": 0, "tool_calls": 0,
              "wall_seconds": 0, "cost_usd": 0.0},
  "unavailable_evidence": [],
  "errors": []
}
```

### B.2 `builder_result@v1`

```json
{ "attempted": ["AK-014"], "changed_paths": [], "commands_run": [],
  "artifacts": [{"path": "", "evidence_ref": ""}],
  "claims": ["free text — stored for forensics, excluded from verification"],
  "self_checks": [{"command": "", "exit_code": 0}],
  "blocked_on": [] }
```

### B.3 `observation_bundle@v1`

```json
{ "schema": "observation_bundle@v1", "run_id": "", "iteration_no": 0,
  "observed_at": "", "project_revision": "", "environment_fingerprint": "",
  "observation_digest": "sha256:...",
  "facets": [{ "kind": "test", "instrument": {"name": "", "version": ""},
               "command": "", "exit_status": 0, "started_at": "", "ended_at": "",
               "evidence_ref": "", "summary": {} }],
  "claims_facet": {"source": "builder", "excluded_from_verification": true, "content": ""},
  "unavailable_evidence": [{"kind": "", "reason": ""}],
  "observation_failures": [{"kind": "", "error_class": "", "detail": ""}] }
```

### B.4 `comparison_record@v1`

```json
{ "schema": "comparison_record@v1", "run_id": "", "iteration_no": 0,
  "reference_digest": "sha256:...", "observation_digest": "sha256:...",
  "comparison_contract_version": "", "comparator_identity": {"runner": "", "model": "", "dimension": ""},
  "residuals": [{ "id": "", "dimension": "", "unit_refs": ["AK-014"],
                  "expected": "", "observed": "", "evidence_refs": [],
                  "severity": "blocking", "confidence": 0.0, "disputed": false,
                  "diagnosis": "", "cheapest_discriminating_test": "",
                  "candidate_corrections": [] }],
  "hard_gate_results": [{"unit_id": "AK-014", "held": false, "evidence_refs": []}],
  "verdict": {"status": "fail", "rationale": ""} }
```

### B.5 `verification_record@v1`

```json
{ "schema": "verification_record@v1", "run_id": "", "iteration_no": 0,
  "unit_id": "AK-014", "claim": "",
  "requirement": {"kind": "test_report", "locator": "", "producer": "instrument"},
  "instrument": {"name": "", "version": "", "deterministic": true},
  "evidence_ref": "", "produced_at": "",
  "outcome": "PASS", "class": null, "detail": "" }
```

### B.6 `settlement_record@v1`

```json
{ "schema": "settlement_record@v1", "run_id": "", "unit_id": "",
  "verification_ref": "", "evidence_ref": "", "reference_digest": "sha256:...",
  "authority_decision": {"required": "critic", "exercised_by": "runtime", "operator_identity": null},
  "event_offset": 0, "settled_at": "" }
```

### B.7 `candidate_revision@v1`

As §5.3.8, serialized, plus `evaluation`: `{held_out: {}, regression: {}, protected_capabilities: {}, payment_delta: {}}` and `decision` with `rationale` and `rollback_ref`.

---

# Annex C — Prime-Agent integration notes

**Status:** verified against upstream documentation at time of writing; re-verify with `gauntlet doctor --probe-runners` before trusting. All of this lives in `adapters/prime_agent/`; none of it may leak inward.

### C.1 What the adapter emits

The adapter's `invoke` renders a kernel program of this shape (exact API surface is adapter-local and version-guarded):

```python
# written by the adapter, executed in the parent Prime-Agent kernel
import json, pathlib
inv = json.loads(pathlib.Path(ENVELOPE_PATH).read_text())   # runtime-built envelope
handle = await rlm(
    ROLE_PROMPT,          # role body + explicit result-contract instruction
    name=INVOCATION_NAME, # "<role>-<unit_or_slice>-<invocation_id_short>"
)
pathlib.Path(HANDLE_PATH).write_text(json.dumps({
    "rlm_child_id": handle.rlm_child_id,
    "name": handle.name,
    "session_dir": handle.session_dir,
    "model": handle.model,
}))
```

The role prompt MUST instruct the child to write its typed result atomically to `GAUNTLET_RESULT_PATH` and only then reply to the parent. The reply is a *signal*; the file is the *contract* (§10.3).

### C.2 Adapter rules

| Rule | Reason |
|---|---|
| Never await a child answer from `rlm(...)` | The call returns at admission and never returns the child's answer |
| Poll the result contract, not the mailbox | Mailbox delivery is best-effort ordering; the file is the settlement-bearing artifact |
| Record `session_dir` as `session_ref` only | Session transcripts are forensic evidence, never control state (I11) |
| Report `isolation: none` unless externally wrapped | The kernel runs model-generated code with the worker's OS permissions; it is a control environment, not a security boundary |
| Install roles from repo files before each run; verify after | Harness role/prompt/memory state is agent-mutable and session-local; drift is configuration drift |
| Map depth configuration to `max_depth` and report it | Recursion is raised deliberately, never by default |
| Treat `list_subagents` as liveness/forensics | Not as a completion signal |

### C.3 Role installation

`gauntlet roles install --harness prime-agent` writes each `.agents/roles/<role>.md` into the harness's subagent-spec surface and records the resulting identifiers in `.gauntlet/roles.lock`. `gauntlet roles verify` re-reads them and reports drift as a `configuration` failure class. For harnesses whose role state is file-based (for example a directory of agent markdown files), the same command writes those files instead. The repo file is always the source of truth.

### C.4 Known caveats

- Idle children may unload and reload; a poll that finds no process must not conclude failure while the result contract is still pending and the deadline has not expired.
- Harness self-refinement (`/refine`-style) may alter role prompts mid-session. `roles verify` before every run and after any optimizer activity; treat unexplained drift as an integrity event, not a convenience.
- Parallel children in one checkout corrupt each other's assumptions. Enforce one workspace per concurrent slice (§11.3).

---

# Annex D — `wayfinder-gauntlet` skill (derivation and installation)

### D.1 Derivation note

The skill is **derived**, not copied blindly: `SKILL.md` names its upstream source (repository and path) in its own body. An implementation that cannot reach the network records that fact and implements the skill from the semantics in §9.2 — which are sufficient on their own. No separate in-repo provenance record is required; the delta lives in this Annex.

### D.2 Derivation procedure

```text
1. Clone upstream into a temporary directory (never into the project tree).
2. Read the upstream skill and the local-markdown tracker convention it falls back to.
3. Transform (§9.2 delta table) into .agents/skills/wayfinder-gauntlet/SKILL.md.
4. Copy in companion skills the transformed skill invokes (grilling, domain-formation,
   research, prototype).
5. Delete the temporary clone.
6. Run the skill conformance test (AK-064).
```

### D.3 Required behavior of the derived skill

- **Preserve:** destination-first charting; the grilling/questioning loop; fog of war with the precise-question test; out-of-scope as a distinct, non-graduating section; HITL versus AFK (an agent that answers its own grilling questions has broken the skill); one-decision-at-a-time discipline.
- **Replace:** tracker child issues → `.wayfinder/answer-key.md` typed Units; tracker labels → typed fields; native blocking edges → `depends_on`; ticket bodies → `map.md` decision entries linking Unit ids.
- **Add:** the Unit schema of §5.3.2 including `evidence_required`; the tier/authority coupling rules; a default of `blast_radius: local` unless the decision touches shared state, money, auth, security, or is hard to reverse — those are `cross_cutting` or `irreversible`; a validation pass that rejects an answer key violating §5.3.2.
- **Remove:** issue-tracker wiring, ticket-type labels, multi-session tracker concurrency assumptions.

### D.4 Companion skills

| Skill | Role in the Gauntlet |
|---|---|
| `grilling` | Formation: extracting sharp questions from loose intent |
| `domain-formation` | Formation: producing the vocabulary the reference is written in |
| `research` | Formation: AFK resolution of facts a decision waits on |
| `prototype` | Formation: cheap artifacts that raise discussion fidelity |
| `gauntlet-run` *(new, repo-owned)* | Operator-facing procedure for running and supervising a loop |
| `gauntlet-role-<role>` *(new, repo-owned)* | Per-role operating procedures too long for a role file body |

Skills are instructions plus (optionally) executable helpers. They are **not** a place to put control logic: anything that decides settlement, authority, or budget belongs in the runtime, where it is typed and tested.

---

# Annex E — Environment variable reference

All optional; all defaulted; none required for a safe bounded run.

| Group | Variables |
|---|---|
| Paths | `GAUNTLET_PROJECT_ROOT` `GAUNTLET_CONFIG_DIR` `GAUNTLET_STATE_DIR` `GAUNTLET_ARTIFACT_DIR` `GAUNTLET_PLAN_DIR` `GAUNTLET_REPORT_DIR` `GAUNTLET_TMP_DIR` |
| Reference | `GAUNTLET_REFERENCE_PATH` `GAUNTLET_REFERENCE_MANIFEST` `GAUNTLET_REFERENCE_DIGEST` `GAUNTLET_ANSWER_KEY_PATH` `GAUNTLET_MAP_PATH` `GAUNTLET_COMPARISON_CONTRACT` `GAUNTLET_ACCEPTANCE_PATH` `GAUNTLET_CONTEXT_PATHS` `GAUNTLET_CANON_PATHS` `GAUNTLET_HELD_OUT_PATH` `GAUNTLET_REGRESSION_PATH` `GAUNTLET_DIGEST_ALGO` |
| Runners | `GAUNTLET_AGENT_RUNNER` `GAUNTLET_ORCHESTRATOR_RUNNER` `GAUNTLET_ORCHESTRATOR_MODEL` `GAUNTLET_BUILDER_RUNNER` `GAUNTLET_BUILDER_MODEL` `GAUNTLET_CRITIC_RUNNER` `GAUNTLET_CRITIC_MODEL` `GAUNTLET_VERIFIER_RUNNER` `GAUNTLET_VERIFIER_MODEL` `GAUNTLET_OBSERVER_RUNNER` `GAUNTLET_OPTIMIZER_RUNNER` `GAUNTLET_OPTIMIZER_MODEL` `GAUNTLET_CLI_AGENT_CMD` `GAUNTLET_ROLES_DIR` |
| Tools | `GAUNTLET_MCP_CONFIG` `GAUNTLET_TOOL_REGISTRY` `GAUNTLET_MCP_ALLOWLIST` `GAUNTLET_MCP_REQUIRED` `GAUNTLET_TOOL_CODE_NAV` `GAUNTLET_TOOL_FILESYSTEM` `GAUNTLET_TOOL_VERSIONING` `GAUNTLET_TOOL_TEST` `GAUNTLET_TOOL_BUILD` `GAUNTLET_TOOL_BROWSER` `GAUNTLET_TOOL_VISUAL` `GAUNTLET_TOOL_WEB` `GAUNTLET_TOOL_RESEARCH` `GAUNTLET_TOOL_SECURITY` `GAUNTLET_TOOL_SANDBOX` `GAUNTLET_TOOL_MEMORY` `GAUNTLET_TOOL_EVIDENCE` `GAUNTLET_TOOL_DOMAIN` `GAUNTLET_TOOL_POLICY` `GAUNTLET_TOOL_BENCHMARK` `GAUNTLET_TOOL_SCHEMA` |
| Limits | `GAUNTLET_INTERACTIVE` `GAUNTLET_MAX_ROUNDS` `GAUNTLET_MAX_PARALLEL` `GAUNTLET_MAX_DEPTH` `GAUNTLET_MAX_WALL_SECONDS` `GAUNTLET_MAX_TOKENS` `GAUNTLET_MAX_COST_USD` `GAUNTLET_RECOVERY_RESERVE_PCT` `GAUNTLET_DIAGNOSTIC_BUDGET` `GAUNTLET_REVIEW_MODE` `GAUNTLET_MAX_CRITICS` `GAUNTLET_MAX_ATTEMPTS_PER_UNIT` `GAUNTLET_NONCONVERGENCE_N` `GAUNTLET_POLL_INTERVAL_MS` `GAUNTLET_STALL_TIMEOUT_MS` `GAUNTLET_RETRY_BASE_MS` `GAUNTLET_RETRY_MAX_MS` |
| Permissions | `GAUNTLET_ALLOW_INSTALL` `GAUNTLET_ALLOW_NETWORK` `GAUNTLET_ALLOW_DESTRUCTIVE` `GAUNTLET_ALLOW_ORCHESTRATOR_IMPLEMENTATION` `GAUNTLET_VERSIONING_STRATEGY` |
| Containment | `GAUNTLET_SANDBOX_BACKEND` `GAUNTLET_SANDBOX_POLICY_DIR` `GAUNTLET_SANDBOX_BINARY` `GAUNTLET_SANDBOX_EXPERIMENTAL` |
| Persistence | `GAUNTLET_STATE_BACKEND` `GAUNTLET_DATABASE_URL` `GAUNTLET_EVIDENCE_BACKEND` `GAUNTLET_EXPORT_DIR` |
| Surfaces | `GAUNTLET_OPERATOR_SURFACES` `GAUNTLET_API_BIND` `GAUNTLET_LOG_FORMAT` `GAUNTLET_LOG_SINK` `GAUNTLET_OTEL_ENDPOINT` |
| Promotion | `GAUNTLET_PROMOTION_MIN_SETTLEMENTS` `GAUNTLET_PROMOTION_MARGIN` |
| Test hooks | `GAUNTLET_CLOCK` `GAUNTLET_ID_SEED` (deterministic clock/ids for tests only) |

---

# Annex F — Sandbox policy templates

**Stability warning:** backend policy schemas are the least stable input in this document. Treat the shapes below as illustrative configuration, verify against the installed backend's current schema at `gauntlet doctor` time, and keep them in `.gauntlet/policies/` where they are versioned with the repo — never compiled in.

Selection is per host and per tier, resolved by configuration:

| Host | Typical backend for `contained`/`local` | Typical backend for `cross_cutting` |
|---|---|---|
| Linux | user-namespace sandbox (bubblewrap-class) or container | hardware-isolated microVM, or the strongest available container tier |
| macOS | OS sandbox profile (seatbelt-class) | microVM via a Linux VM host, or refuse |
| Windows/WSL | process-container tier | strongest available; refuse rather than downgrade |

`contained` / `local` (illustrative):

```json
{
  "containment": "<stable backend id>",
  "process": { "commandLine": "<builder command>", "timeout": 120000 },
  "filesystem": {
    "readwritePaths": ["<unit workspace>"],
    "readonlyPaths": ["<frozen reference dir>", "<map path>"],
    "deniedPaths": ["<credential dirs>", "<state dir>", "<evidence dir>"]
  },
  "network": { "defaultPolicy": "block" },
  "ui": { "clipboard": false, "display": false },
  "retainPolicies": false
}
```

`cross_cutting` (illustrative): same shape, tighter — explicit host allowlist only where the claim genuinely requires it, repository `.git` denied, and **every host-mutating fallback explicitly refused** rather than left at the backend's default consent posture. On a machine that cannot provide real isolation the run must fail loudly rather than silently downgrade to a mode that touches host state.

`irreversible`: **no policy file.** Its presence is a `doctor` failure (AK-049). The action never reaches the sandbox port unattended; the human authority gate fires first. Sandboxing an irreversible action well is still shipping it.

---

# Annex G — Bootstrap invocation

To start the build as a Gauntlet run rather than as ordinary coding:

```text
Read .agents/specs/Core-spec.md and .agents/plans/gauntlet-impl.md.

1. Copy Annex A verbatim to .wayfinder/answer-key.md and freeze it (record the digest
   in .gauntlet/reference.lock). Do not edit it again for the rest of this effort.
2. Read AGENTS.md. Follow it.
3. Execute the plan in order. Every task ends with its verification gate exiting 0,
   plus its teeth check.
4. Do not write implementation code yourself while delegation is available: decompose,
   dispatch builders, and fan out blind critics per §10 and §13. The critic receives the
   comparison envelope only — never your trajectory.
5. When the runtime can execute its own loop (plan Task 20), switch to dogfooding:
   run the remaining tasks through the Gauntlet itself against the frozen answer key.
6. Settle only what the evidence supports. Report provisional work as provisional.
```

The spec is the ruler; the plan is the route; neither is a substitute for the other. If they conflict, the spec wins and the plan is corrected — never the reverse.