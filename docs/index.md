# Gauntlet Documentation System

Welcome to the technical knowledge system for **Gauntlet**: a portable, reference-anchored comparison-and-correction runtime for autonomous agentic work.

> **The Core Thesis**: *An agent claiming "done" is not a control system.* Gauntlet separates the entity producing work from the entity observing reality, the entity discovering differences, and the entity certifying truth against hard evidence.

---

## What This Project Is

Gauntlet is an execution and control runtime written in Rust. It takes a desired software transformation (such as a feature implementation, bug fix, or refactor) and executes it under a closed-loop governance protocol:

$$\text{Reference} \longrightarrow \text{Transform} \longrightarrow \text{Observe} \longrightarrow \text{Compare} \longrightarrow \text{Verify} \longrightarrow \text{Select} \longrightarrow \text{Correct} \longrightarrow \text{Settle} \longrightarrow \text{Remember}$$

Inside Gauntlet:
- A **Reference** specifies the immutable ruler (invariants, acceptance criteria, and checkable claims called **Units**) and is frozen before work begins.
- A **Builder** (an agent, script, or model) modifies code in an isolated workspace.
- Independent **Observers** (compilers, test runners, git, filesystem probes) inspect the resulting reality without asking the builder what it did.
- An isolated **Comparator** (critic) receives a blind comparison envelope and identifies **Residuals** (deviations from the reference).
- A deterministic **Verifier** evaluates specific claims against hard instrument evidence, producing a three-state outcome (`PASS`, `FAIL`, `INCONCLUSIVE`).
- A **Controller** selects the minimal affordable, governable correction to close open residuals until the run settles or stops honestly at a bounded limit.
- Durable state is persisted across an append-only event log and SQLite store, surviving process termination and machine restarts.

---

## What Problem It Solves

When autonomous AI coding agents operate without governed execution:
1. **Self-grading**: The agent that wrote the code certifies that the tests passed, even when tests were never executed or failed silently.
2. **Goal drift**: Under difficult edge cases, prompts or requirements are quietly rewritten mid-run to match what got built instead of holding the line.
3. **Optimistic coercion**: Missing evidence, timeouts, or unavailable tools are treated as passing, hiding defects until production.
4. **Context pollution**: Code review models are given the author's conversational excuses and rationalizations, talking the reviewer into approving broken changes.
5. **Amnesia & thrashing**: Restarts discard prior verification progress, and correction steps rewrite entire codebases rather than making the minimal coherent fix.

Gauntlet eliminates these structural defects not through prompt suggestions, but through compile-time boundaries, content-addressed evidence locking, structural context blindness, and strict state-machine governance.

---

## The System in One Picture

```mermaid
flowchart TB
    subgraph Governance ["0. Governance & Preflight"]
        REF[".wayfinder/reference.yaml + answer-key.md"] -->|Normalize & SHA-256| LOCK[".gauntlet/reference.lock (Frozen Ruler)"]
        CONF["GAUNTLET.md + Env + Flags"] -->|Resolve Defaults| PRE["Preflight Doctor (8 Checks)"]
    end

    subgraph OuterLoop ["1. The Gauntlet Runtime Loop"]
        LOCK --> CTRL["RunController"]
        PRE --> CTRL
        
        CTRL -->|Schedule Unit| SCHED["UnitScheduler & AuthorityGate"]
        SCHED -->|Wrap in Sandbox| BUILD["Builder (AgentRunner)"]
        
        BUILD -->|Transform Code| WORKSPACE["Isolated Workspace / Worktree"]
        
        WORKSPACE -->|Probe Reality| OBS["Observers (Build, Test, FS, Git)"]
        OBS -->|Normalize| BUNDLE["ObservationBundle (Claims Excluded)"]
        
        BUNDLE -->|Allowlist Envelope| COMP["Comparator (Structural Blindness)"]
        LOCK -->|Project Reference| COMP
        COMP -->|Emit Residuals| RES["Residual Set"]
        
        RES --> VERIFY["Verifier Ladder (Instruments First)"]
        BUNDLE -->|Evidence Binding| VERIFY
        
        VERIFY -->|Outcomes: PASS/FAIL/INCONC| DECIDE{"All Blocking Units PASS?"}
        
        DECIDE -->|No: Residuals Remain| SELECT["CorrectionSelector (10-Dim Score)"]
        SELECT -->|Minimal Fix| BUILD
        
        DECIDE -->|Yes: Verified| SETTLE["SettlementService (Atomic Commit)"]
    end

    subgraph Memory ["2. Persistence & Capability Promotion"]
        SETTLE --> EVENTLOG["Append-Only EventLog (fsync)"]
        SETTLE --> STATE["SQLite StateStore"]
        SETTLE --> MEM["Developmental Memory"]
        MEM -->|N Settlements + Variation + Recovery| CAP["Capability Ledger (Promoted)"]
    end

    subgraph Interface ["3. Interactive Operator Control Plane"]
        CTRL <==>|Loopback HTTP / JSON\n(JsonOperatorSurface)| WORKBENCH["Operator Workbench (workbench/)\nBun + TypeScript + OpenTUI\n(Router-Not-Authority)"]
        WORKBENCH -.-> DESIGN["Canonical Design System\n(DESIGN.md)"]
    end
```

---

## The 10 Concepts You Need First

1. [**Reference**](concepts.md#reference): The immutable specification of desired state, invariants, and constraints. Once normalized and digested, it cannot be modified by any builder.
2. [**Answer Key & Unit**](concepts.md#answer-key--unit): The projection of the Reference into checkable claims ($AK\text{-}001 \dots AK\text{-}070$). Each Unit declares an `evidence_required` specification, `blast_radius`, and `authority`.
3. [**Transform**](concepts.md#transform): An attempted modification of code or artifacts performed by a Builder inside an isolated workspace.
4. [**Observation Bundle**](concepts.md#observation-bundle): The normalized collection of facts gathered by non-model instruments (git diffs, build exits, test outputs). Builder self-claims are placed in an excluded facet that verifiers cannot read.
5. [**Comparison & Residual**](concepts.md#comparison--residual): An independent comparison of the Observation Bundle against the Reference in a fresh context. Differences requiring disposition are called Residuals.
6. [**Verifier & Outcome**](concepts.md#verifier--outcome): An evaluation of a specific claim against concrete evidence. Yields a three-state `Outcome`: `PASS`, `FAIL`, or `INCONCLUSIVE`. A critic's opinion can never produce a `PASS`.
7. [**Correction**](concepts.md#correction): The smallest affordable, governable modification selected to eliminate an observed residual without rewriting unaffected code.
8. [**Settlement**](concepts.md#settlement): The atomic commit marking a Unit or Run as complete, requiring proven `PASS` evidence for all blocking units and re-verifying the Reference digest.
9. [**Authority vs. Containment**](concepts.md#authority-vs-containment): Orthogonal safety checks: *Authority* decides if work is permitted to run; *Containment* (Sandbox) wraps the process from start to constrain its environment.
10. [**Capability**](concepts.md#capability): Reliable settlement under variation with recovery from failure. Single runs only create developmental memory; capability is earned over time.

---

## A Representative Journey: Executing a Task

1. **Invocation**: The operator runs:
   ```bash
   gauntlet run "Implement account lockout after repeated failed logins"
   ```
2. **Preflight**: Gauntlet executes `doctor` checks: validates `GAUNTLET.md`, checks sandbox policies, verifies that `.wayfinder/answer-key.md` is outside any builder-writable workspace, and calculates budget limits.
3. **Ruler Freeze**: The reference manifest and answer-key units are normalized, digested via SHA-256, and locked. If `.gauntlet/reference.lock` exists, the runtime verifies zero drift.
4. **Unit Scheduling**: `UnitScheduler` inspects open units, calculates dependencies, checks the `AuthorityGate`, and applies containment (Bubblewrap sandbox or none).
5. **Builder Turn**: The `AgentRunner` adapter invokes the assigned builder in an isolated worktree. The builder makes edits and exits, producing a result contract file.
6. **Instrument Observation**: Observers probe the worktree: `BuildObserver` runs compilation, `TestObserver` executes tests, and `GitObserver` computes diffs. Results are packaged into an `ObservationBundle`.
7. **Blind Comparison**: An `EnvelopeBuilder` constructs a fresh comparison envelope containing only the reference and instrument observations. All builder trajectories, explanations, and attempt counts are strictly excluded. The `Comparator` reports open residuals.
8. **Deterministic Verification**: `Verifier` inspects test reports and exit codes against the unit's `evidence_required` locators. If a test passed, it marks `Outcome::PASS`. If missing or unrun, it marks `Outcome::INCONCLUSIVE`.
9. **Correction or Settlement**:
   - If residuals remain and budget allows, `CorrectionSelector` picks the smallest targeted correction and repeats the loop.
   - If all blocking units are `PASS`, `SettlementService` re-verifies the reference digest, writes the atomic settlement event to the append-only event log, commits the state to SQLite, and outputs a certified run report.

---

## Navigation by Intent

Find what you need based on what you are trying to accomplish:

| Intent | Recommended Starting Point | Description |
|---|---|---|
| **Learn the System** | [Mental Model](mental-model.md) → [Architecture](architecture.md) | Form an accurate conceptual model and understand the layered architecture. |
| **Get Something Done** | [First Gauntlet Run Tutorial](tutorials/first-gauntlet-run.md) → [How-To Guides](how-to/add-an-adapter.md) | Run a guided scenario, implement an adapter, or configure budgets. |
| **Understand Why** | [Explanations Directory](explanations/why-separate-critic-verifier.md) | Architectural rationale: why critic $\neq$ verifier, structural blindness, and 3-state logic. |
| **Look Something Up** | [CLI Reference](reference/cli.md) · [Config](reference/configuration.md) · [Domain Types](reference/domain-types.md) · [Ports API](reference/ports-api.md) | Exact syntax, trait signatures, error enums, environment variables, and proof obligations. |
| **Follow Execution** | [Run Lifecycle Workflow](workflows/run-lifecycle.md) → [Iteration Loop](workflows/iteration-loop.md) | Trace end-to-end execution paths, state transitions, and sequence diagrams. |
| **Operate the TUI** | [How-To: Operate the TUI](how-to/operate-the-tui.md) → [TUI Subsystem](subsystems/tui.md) · [TUI Workflow](workflows/tui-session.md) | Launch the workbench, navigate cases and units, hoist horizons, and handle escalations. |
| **TUI Design & Tokens** | [Design Contract](../DESIGN.md) → [TUI Reference](reference/tui.md) | Normative visual grammar, semantic tokens, color degradation, and geometry thresholds. |
| **Trace the Code** | [Source Map](source-map.md) → [Subsystems](subsystems/domain.md) | Connect every concept directly to concrete files, structs, traits, and tests. |
| **Debug a Problem** | [Troubleshooting Guide](troubleshooting.md) | Diagnostic flows, preflight error resolutions, quarantine inspection, and crash recovery. |
| **Full Map of Docs** | [Documentation Map](documentation-map.md) | Complete inventory of every page classified by Diátaxis and audience need. |

