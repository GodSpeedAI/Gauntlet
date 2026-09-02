# Documentation Map

This document catalogs every page in the Gauntlet technical knowledge system. It maps each document to its primary purpose, reader need, Diátaxis classification, prerequisites, and related documentation.

---

## Diátaxis Framework Structure

The documentation is organized around four distinct reader modes:

```text
               PRACTICAL                   THEORETICAL
          ┌───────────────────────────┬───────────────────────────┐
          │         TUTORIALS         │        EXPLANATION        │
LEARNING  │   Learning-oriented       │   Understanding-oriented  │
          │   Guided, safe, hands-on  │   Why things are this way │
          ├───────────────────────────┼───────────────────────────┤
          │       HOW-TO GUIDES       │         REFERENCE         │
WORKING   │   Problem-oriented        │   Information-oriented    │
          │   Goal-driven recipes     │   Accurate technical facts│
          └───────────────────────────┴───────────────────────────┘
```

Along with DeepWiki-style architectural guides, subsystem guides, workflows, and source traceability maps.

---

## Complete Page Inventory

### Foundations & Orientation

| Page | Diátaxis | Purpose | Primary Audience Need | Prerequisites | Related Pages |
|---|---|---|---|---|---|
| [`index.md`](index.md) | Orientation | Front door, core problem statement, system diagram, 10 key concepts, navigation by intent. | "What is Gauntlet, why does it exist, and where do I start?" | None | [`mental-model.md`](mental-model.md), [`documentation-map.md`](documentation-map.md) |
| [`documentation-map.md`](documentation-map.md) | Meta / Map | Complete catalog of documentation pages with Diátaxis classification, dependencies, and reader intent. | "Where can I find documentation for X?" | None | [`index.md`](index.md), [`source-map.md`](source-map.md) |
| [`mental-model.md`](mental-model.md) | Explanation | High-level conceptual model, progressive disclosure of the control loop, core invariants, and rationale. | "How should I think about Gauntlet's operation?" | [`index.md`](index.md) | [`concepts.md`](concepts.md), [`architecture.md`](architecture.md) |
| [`concepts.md`](concepts.md) | Reference | Canonical vocabulary definitions, anti-substitutions, and domain concepts. | "What does this specific repository term mean?" | [`mental-model.md`](mental-model.md) | [`reference/domain-types.md`](reference/domain-types.md) |
| [`architecture.md`](architecture.md) | Explanation / Architecture | Canonical architectural model: logical layering, runtime topology, data flow, trust boundaries. | "How is the system structured and how do components communicate?" | [`mental-model.md`](mental-model.md) | [`subsystems/domain.md`](subsystems/domain.md), [`subsystems/ports.md`](subsystems/ports.md) |
| [`source-map.md`](source-map.md) | Traceability / Map | Matrix mapping concepts, capabilities, invariants, and ports to concrete files and symbols. | "Where in the code is concept X implemented?" | [`architecture.md`](architecture.md) | [`documentation-map.md`](documentation-map.md) |
| [`troubleshooting.md`](troubleshooting.md) | How-To / Reference | Diagnostic trees, preflight failures, sandbox errors, and crash recovery procedures. | "Something failed; how do I diagnose and fix it?" | [`index.md`](index.md) | [`workflows/preflight-doctor.md`](workflows/preflight-doctor.md) |

---

### Subsystem Guides (DeepWiki Layer)

| Page | Diátaxis | Purpose | Primary Audience Need | Prerequisites | Related Pages |
|---|---|---|---|---|---|
| [`subsystems/domain.md`](subsystems/domain.md) | Architecture / Reference | Pure domain layer (`crates/gauntlet-domain`): value objects, aggregates, constructor invariants, and zero internal dependencies. | "What domain entities exist and what invariants do they enforce?" | [`architecture.md`](architecture.md) | [`reference/domain-types.md`](reference/domain-types.md) |
| [`subsystems/ports.md`](subsystems/ports.md) | Architecture / Reference | Ports layer (`crates/gauntlet-ports`): 17 capability traits, error enums, deadlines, and shared contract test suites. | "What interfaces do adapters implement and how are they verified?" | [`subsystems/domain.md`](subsystems/domain.md) | [`subsystems/adapters.md`](subsystems/adapters.md), [`how-to/add-an-adapter.md`](how-to/add-an-adapter.md) |
| [`subsystems/application.md`](subsystems/application.md) | Architecture / Explanation | Application layer (`crates/gauntlet-app`): RunController, IterationEngine, UnitScheduler, CorrectionSelector, SettlementService. | "How does the control loop coordinate execution?" | [`subsystems/ports.md`](subsystems/ports.md) | [`workflows/run-lifecycle.md`](workflows/run-lifecycle.md), [`workflows/iteration-loop.md`](workflows/iteration-loop.md) |
| [`subsystems/adapters.md`](subsystems/adapters.md) | Architecture / Reference | Adapters layer (`crates/adapters/*`): 24 modular implementations for agents, stores, observers, comparators, verifiers, and sandboxes. | "Which concrete adapters exist and how do they work?" | [`subsystems/ports.md`](subsystems/ports.md) | [`how-to/add-an-adapter.md`](how-to/add-an-adapter.md) |
| [`subsystems/cli.md`](subsystems/cli.md) | Architecture / Reference | CLI composition root (`crates/gauntlet-cli`): command tree, argument parser, adapter wiring, and thin library architecture. | "How does the binary assemble the runtime from adapters?" | [`subsystems/application.md`](subsystems/application.md) | [`reference/cli.md`](reference/cli.md) |
| [`subsystems/governance.md`](subsystems/governance.md) | Architecture / Explanation | Governance subsystem: AuthorityGate, sandbox tier policies, budget metering, recovery reserves, and escalation queues. | "How are permissions, containment tiers, and budgets enforced?" | [`subsystems/application.md`](subsystems/application.md) | [`how-to/configure-runtime-budgets.md`](how-to/configure-runtime-budgets.md) |

---

### Execution Workflows

| Page | Diátaxis | Purpose | Primary Audience Need | Prerequisites | Related Pages |
|---|---|---|---|---|---|
| [`workflows/run-lifecycle.md`](workflows/run-lifecycle.md) | Explanation / Architecture | End-to-end trace of `gauntlet run`: preflight, freeze, scheduling, dispatch, observation, comparison, verification, settlement, and remember. | "What happens end-to-end when a run executes?" | [`architecture.md`](architecture.md) | [`workflows/iteration-loop.md`](workflows/iteration-loop.md) |
| [`workflows/preflight-doctor.md`](workflows/preflight-doctor.md) | Explanation / Reference | Detailed execution of `gauntlet doctor`, all 8 preflight checks, configuration resolution, and diagnostic exit codes. | "What does doctor check and why did it fail?" | [`architecture.md`](architecture.md) | [`reference/configuration.md`](reference/configuration.md) |
| [`workflows/iteration-loop.md`](workflows/iteration-loop.md) | Explanation / Architecture | Detailed trace of a single corrective iteration: UnitScheduler, EnvelopeBuilder, AgentRunner, Observers, Comparator, Verifier, CorrectionSelector. | "What exact steps occur within one iteration?" | [`workflows/run-lifecycle.md`](workflows/run-lifecycle.md) | [`subsystems/application.md`](subsystems/application.md) |
| [`workflows/crash-recovery.md`](workflows/crash-recovery.md) | Explanation / Architecture | How the runtime resumes from crashes using the append-only event log and SQLite store without conversational memory. | "How does resume work and how is state protected?" | [`subsystems/application.md`](subsystems/application.md) | [`subsystems/domain.md`](subsystems/domain.md) |
| [`workflows/settlement-promotion.md`](workflows/settlement-promotion.md) | Explanation / Architecture | Verification of the 5 settlement preconditions, 3-step atomic commit sequence, developmental memory recording, and capability ledger promotion. | "When does a run settle and how is capability earned?" | [`workflows/run-lifecycle.md`](workflows/run-lifecycle.md) | [`explanations/governed-self-improvement.md`](explanations/governed-self-improvement.md) |

---

### Explanations & Design Rationale

| Page | Diátaxis | Purpose | Primary Audience Need | Prerequisites | Related Pages |
|---|---|---|---|---|---|
| [`explanations/why-separate-critic-verifier.md`](explanations/why-separate-critic-verifier.md) | Explanation | Architectural analysis of v1 Defect D3: why models produce residuals while only instruments verify, and the dangers of collapsing them. | "Why can't a code reviewer model mark a task passed?" | [`mental-model.md`](mental-model.md) | [`subsystems/ports.md`](subsystems/ports.md) |
| [`explanations/why-admission-based-runner.md`](explanations/why-admission-based-runner.md) | Explanation | Architectural analysis of v1 Defect D1: why blocking agent calls fail with recursive harnesses (Prime-Agent) and how admission handles work. | "Why is AgentRunner designed around poll/contract instead of run_turn()?" | [`subsystems/ports.md`](subsystems/ports.md) | [`subsystems/adapters.md`](subsystems/adapters.md) |
| [`explanations/context-isolation-blindness.md`](explanations/context-isolation-blindness.md) | Explanation | Structural context blindness: why prompt-based blindness fails, and how allowlist-constructed comparison envelopes protect critics. | "How does Gauntlet ensure critics aren't swayed by builder reasoning?" | [`mental-model.md`](mental-model.md) | [`subsystems/application.md`](subsystems/application.md) |
| [`explanations/three-state-verification.md`](explanations/three-state-verification.md) | Explanation | Type-theoretic analysis of `Outcome` (`PASS`, `FAIL`, `INCONCLUSIVE`), why bool conversions are forbidden, and why missing is never passing. | "Why is Outcome 3-state and why can't it convert to bool?" | [`concepts.md`](concepts.md) | [`subsystems/domain.md`](subsystems/domain.md) |
| [`explanations/governed-self-improvement.md`](explanations/governed-self-improvement.md) | Explanation | Self-improvement protocol: candidate revisions, isolated evaluations, held-out comparisons, regression checks, and rollback safety. | "How can Gauntlet improve itself without moving its own ruler?" | [`mental-model.md`](mental-model.md) | [`workflows/settlement-promotion.md`](workflows/settlement-promotion.md) |

---

### Tutorials

| Page | Diátaxis | Purpose | Primary Audience Need | Prerequisites | Related Pages |
|---|---|---|---|---|---|
| [`tutorials/first-gauntlet-run.md`](tutorials/first-gauntlet-run.md) | Tutorial | Hands-on walkthrough: initializing a target workspace, running preflight doctor, executing an end-to-end loop with the mock runner, and inspecting evidence. | "Teach me how to run Gauntlet step-by-step." | None | [`index.md`](index.md), [`how-to/configure-runtime-budgets.md`](how-to/configure-runtime-budgets.md) |
| [`tutorials/creating-a-reference.md`](tutorials/creating-a-reference.md) | Tutorial | Guided tutorial on defining a reference manifest (`reference.yaml`) and authoring typed, checkable Units in `answer-key.md`. | "How do I specify a reference for my own project?" | [`tutorials/first-gauntlet-run.md`](tutorials/first-gauntlet-run.md) | [`reference/domain-types.md`](reference/domain-types.md) |

---

### How-To Guides

| Page | Diátaxis | Purpose | Primary Audience Need | Prerequisites | Related Pages |
|---|---|---|---|---|---|
| [`how-to/add-an-adapter.md`](how-to/add-an-adapter.md) | How-To | Step-by-step recipe for implementing a new adapter for any port and executing its shared contract test suite. | "How do I plug in a new agent runner, state store, or sandbox?" | [`subsystems/ports.md`](subsystems/ports.md) | [`subsystems/adapters.md`](subsystems/adapters.md) |
| [`how-to/add-a-verification-method.md`](how-to/add-a-verification-method.md) | How-To | Practical guide to introducing a new instrumented `VerifyBy` kind and wiring its verifier adapter. | "How do I add a new verification instrument?" | [`subsystems/ports.md`](subsystems/ports.md) | [`reference/domain-types.md`](reference/domain-types.md) |
| [`how-to/configure-runtime-budgets.md`](how-to/configure-runtime-budgets.md) | How-To | Recipe for setting token limits, wall time, dollar spend, recovery reserves, and diagnostic budgets via `GAUNTLET.md` and env vars. | "How do I prevent runaway model spend or execution loops?" | [`reference/configuration.md`](reference/configuration.md) | [`subsystems/governance.md`](subsystems/governance.md) |
| [`how-to/handle-escalations.md`](how-to/handle-escalations.md) | How-To | Operator guide for listing, inspecting, resolving, and retrying tickets in the human escalation queue. | "How do I resolve a blocked or escalated unit?" | [`subsystems/governance.md`](subsystems/governance.md) | [`reference/cli.md`](reference/cli.md) |

---

### Technical Reference

| Page | Diátaxis | Purpose | Primary Audience Need | Prerequisites | Related Pages |
|---|---|---|---|---|---|
| [`reference/cli.md`](reference/cli.md) | Reference | Complete specification of CLI commands, options, environment variables, exit codes, and machine-readable output schemas. | "What is the exact syntax and flag set for gauntlet commands?" | None | [`subsystems/cli.md`](subsystems/cli.md) |
| [`reference/configuration.md`](reference/configuration.md) | Reference | Comprehensive reference of `GAUNTLET.md` front-matter fields, Annex E environment variables, and the defaults table. | "What configuration keys exist and what are their defaults?" | None | [`workflows/preflight-doctor.md`](workflows/preflight-doctor.md) |
| [`reference/domain-types.md`](reference/domain-types.md) | Reference | Exhaustive catalog of domain value objects, newtypes, enums, Wire tokens, and aggregate validation rules. | "What are the exact variants and validation rules for domain type X?" | [`concepts.md`](concepts.md) | [`subsystems/domain.md`](subsystems/domain.md) |
| [`reference/ports-api.md`](reference/ports-api.md) | Reference | Trait signatures, input parameters, return types, and typed error enums for all 17 ports in `gauntlet-ports`. | "What is the trait definition and error type for port X?" | [`subsystems/ports.md`](subsystems/ports.md) | [`how-to/add-an-adapter.md`](how-to/add-an-adapter.md) |
| [`reference/error-catalog.md`](reference/error-catalog.md) | Reference | Directory of all typed error enums across domain, ports, app, and adapters, with failure loci and error strings. | "What does error variant X mean and where is it raised?" | None | [`troubleshooting.md`](troubleshooting.md) |
| [`reference/invariants-and-proofs.md`](reference/invariants-and-proofs.md) | Reference | Mapping of Invariants I1–I12, Proof obligations P1–P12, and Anti-Gaming strategies G1–G16 to source tests and enforcement locations. | "How is invariant X enforced and where is its proof test?" | [`architecture.md`](architecture.md) | [`source-map.md`](source-map.md) |
