# Gauntlet controlled execution

Gauntlet controls extended execution against criteria held outside the work being judged.

## Language

**Frozen Reference**:
A fixed description of the intended result and its evaluation criteria for a controlled run.

**Verification Unit**:
An independently assessable part of a Frozen Reference with a declared criterion and required observations.

**Gauntlet Orchestrator**:
The role accountable for decomposition, coordination, independent evidence, integration, and the judgment that the controlled work has settled.
_Avoid_: Team Lead, Run Controller

**Run Controller**:
The mechanism that advances a controlled run through dispatch, observation, verification, correction, and terminal disposition.

**Role Invocation**:
One bounded dispatch of a Gauntlet role, under the limits and result contract for that invocation.

**Invocation Result Contract**:
The required form and content of a Role Invocation’s attributable result.

**Recursive Execution**:
Execution in which a running agent delegates a subproblem to a nested invocation with its own working context and returns the result into the parent’s work.

**Controlled Run Settlement**:
Gauntlet’s judgment that its controlled run satisfies the Frozen Reference and requires no further correction under that run’s criteria.
_Avoid_: Operational Settlement, process completion

**Runtime Promotion**:
A decision to adopt a candidate runtime revision following evidence about generalization, regressions, and recovery.
_Avoid_: Candidate Promotion, controlled run settlement

**Recursive Execution Provider**:
A provider that executes delegated subproblems in nested invocations and returns attributable results to the caller.

## Sources and boundary

The [Core specification](.agents/specs/Core-spec.md) defines the roles, frozen reference and settlement rules. The [controller](crates/gauntlet-app/src/control/controller.rs) advances the loop. A Gauntlet Orchestrator owns decomposition and evaluation within Gauntlet. A Workbench Team Lead delegates inside a Team. The Run Controller, recursive execution provider and Cognate runtime are execution mechanisms with their own contracts.

A Controlled Run Settlement is scoped to its Frozen Reference. Another authority may use it as evidence, then decide on its own operation. Runtime Promotion requires the separate promotion evaluation. A successful run alone is insufficient.
