# Concepts and Canonical Vocabulary

In the Gauntlet runtime, names are load-bearing architectural constructs across code, schemas, event logs, and prompts. Substituting near-synonyms degrades the control system into ordinary, ungrounded agent execution.

---

## The Forbidden Substitutions

| Do Not Use (Near-Synonym) | Canonical Gauntlet Term | Why the Distinction Matters |
|---|---|---|
| *Goal*, *Prompt*, *Plan* | **Reference** | A goal is aspirational and easily rewritten. A Reference is frozen, normalized, and content-addressed before work begins. |
| *Review*, *Criticism* | **Verification** | A code review expresses a model or human opinion. Verification evaluates an assertion against concrete instrument evidence. |
| *Output*, *Green Build* | **Settlement** | A command producing exit code 0 or an agent printing text is not settlement. Settlement is an atomic commit supported by certified evidence. |
| *Tool Access*, *One Success* | **Capability** | Having access to an API or getting code to work once is not capability. Capability is earned reliability across varied conditions with failure recovery. |
| *Opinion*, *Model Assessment* | **Evidence** | A model claiming "the test passed" is opinion. An instrument-produced exit code or test report JSON is evidence. |
| *Plan*, *Checklist* | **Ruler** | A plan describes intended sequence of work. A ruler (the Answer Key) defines the falsifiable standard the finished system must satisfy. |

---

## Core Domain Vocabulary

### Reference
- **What it means**: The frozen, immutable standard against which all transformations are judged for a given run. Defined in `.wayfinder/reference.yaml`. Contains purpose, desired state, invariants, constraints, anti-goals, settlement criteria, and unknown questions.
- **What it does not mean**: It is not a mutable backlog, a conversation prompt, or a temporary draft. Once frozen, it cannot be modified by any builder.
- **Implementation locus**: [`Reference`](../crates/gauntlet-domain/src/reference.rs), [`ReferenceFreezer`](../crates/gauntlet-app/src/reference/manifest.rs).

### Answer Key & Unit
- **What it means**: The projection of the Reference into checkable, typed claims ($AK\text{-}001 \dots AK\text{-}070$). Each **Unit** has:
  - `id`: Unique identifier (e.g. `AK-014`).
  - `claim`: A falsifiable assertion that is true or false of the finished system.
  - `verify_by`: Verification method (`build`, `test`, `schema_check`, `runtime_probe`, `human_signoff`, etc.).
  - `blast_radius`: Potential blast radius (`contained`, `local`, `cross_cutting`, `irreversible`).
  - `authority`: Minimum required authority (`critic`, `lead_agent`, `human`).
  - `evidence_required`: Strict definition of the required instrument evidence kind, locator, and producer.
- **What it does not mean**: A Unit is not a task on a Jira board or an activity ("write 10 lines of code"). It is an assertion about the end state.
- **Implementation locus**: [`Unit`](../crates/gauntlet-domain/src/unit.rs), [`.wayfinder/answer-key.md`](../.wayfinder/answer-key.md).

### Transform
- **What it means**: An attempted modification to the target system performed by a builder (code edits, configuration changes, refactoring) in an isolated workspace.
- **What it does not mean**: It is not an assertion of correctness or a final commit.
- **Implementation locus**: [`RoleInvocation`](../crates/gauntlet-domain/src/role_invocation.rs), [`AgentRunner`](../crates/gauntlet-ports/src/agent_runner.rs).

### Observation & Observation Bundle
- **What it means**: Objective facts collected from resulting reality by non-model instruments (git diffs, build exits, test reports). Packaged into a normalized, content-addressed `ObservationBundle`.
- **What it does not mean**: The builder's text report or completion claim. Builder text enters only into a dedicated `claims` facet that is structurally excluded from verification.
- **Implementation locus**: [`ObservationBundle`](../crates/gauntlet-domain/src/observation.rs), [`Observer`](../crates/gauntlet-ports/src/observer.rs).

### Comparison & Residual
- **What it means**: An evaluation by an isolated `Comparator` comparing the Observation Bundle against the Reference. Deviations requiring resolution are materialized as **Residuals**.
- **What it does not mean**: A comparison is not a verification pass. A comparator cannot close a Unit; it can only identify discrepancies.
- **Implementation locus**: [`ComparisonRecord`](../crates/gauntlet-domain/src/comparison.rs), [`Residual`](../crates/gauntlet-domain/src/comparison.rs), [`Comparator`](../crates/gauntlet-ports/src/comparator.rs).

### Verification & Outcome
- **What it means**: An evaluation by a `Verifier` checking a specific Unit claim against its declared `evidence_required` locator and producer. Yields an `Outcome`:
  - `Outcome::PASS`: The claim was tested by the declared instrument and held true.
  - `Outcome::FAIL`: The claim was tested and failed.
  - `Outcome::INCONCLUSIVE`: The required evidence is missing, unrun, or unresolvable.
- **What it does not mean**: `Outcome` is not a boolean. It cannot be converted to `bool` (`impl From<Outcome> for bool` is forbidden). Missing evidence is never rounded up to `PASS`.
- **Implementation locus**: [`VerificationRecord`](../crates/gauntlet-domain/src/verification.rs), [`Outcome`](../crates/gauntlet-domain/src/values.rs), [`Verifier`](../crates/gauntlet-ports/src/verifier.rs).

### Correction
- **What it means**: The specific transformation selected by the `CorrectionSelector` to close an observed residual. Scored across 10 dimensions to pick the smallest affordable, governable change.
- **What it does not mean**: A total rewrite of the system or an unconstrained retry.
- **Implementation locus**: [`CorrectionSelection`](../crates/gauntlet-domain/src/correction.rs), [`CorrectionSelector`](../crates/gauntlet-app/src/control/select.rs).

### Settlement
- **What it means**: The consequential conclusion of a Unit or Run. Requires verified `PASS` evidence for all blocking units, re-verifies that the Reference digest did not change, and commits an atomic event to the event log.
- **What it does not mean**: An agent saying "I am done" or a clean terminal exit.
- **Implementation locus**: [`SettlementRecord`](../crates/gauntlet-domain/src/settlement.rs), [`SettlementService`](../crates/gauntlet-app/src/settle/mod.rs).

### Capability & Developmental Memory
- **What it means**:
  - **Developmental Memory**: Raw, append-only logs of attempts, failures, payments, and recoveries.
  - **Capability**: A reusable skill, route, or prompt template promoted to the `CapabilityLedger` only after achieving $N$ distinct settlements across verified variation axes with successful failure recovery.
- **What it does not mean**: An agent claiming it can do something because it succeeded once in a demo.
- **Implementation locus**: [`CapabilityLedger`](../crates/gauntlet-domain/src/capability.rs), [`CandidateRevision`](../crates/gauntlet-domain/src/capability.rs).

---

## Governance & Safety Vocabulary

### Authority vs. Containment
- **Authority**: Decides *whether* an operation is allowed to run at all, based on role permissions and unit blast radius. Evaluated before any process begins.
- **Containment**: Decides *how isolated* the process will be if it runs (e.g. filesystem isolation, network sandboxing via Bubblewrap). Sandboxing is initialized before the first token executes.
- **Implementation locus**: [`AuthorityGate`](../crates/gauntlet-app/src/govern/gate.rs), [`Sandbox`](../crates/gauntlet-ports/src/sandbox.rs).

### Blast Radius Tiers
- `contained`: Affects only scratch files within the temporary workspace.
- `local`: Modifies source files within the repository.
- `cross_cutting`: Modifies repository-wide configuration, toolchains, or boundaries.
- `irreversible`: Actions that cannot be automatically rolled back (e.g. production deployments, secret rotations, publishing packages). **Always requires human authority.**
- **Implementation locus**: [`BlastRadius`](../crates/gauntlet-domain/src/values.rs).
