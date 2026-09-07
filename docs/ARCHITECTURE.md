# Architecture

The Gauntlet runtime orchestrates

```text
Reference → Transform → Observe → Compare → Verify → Select → Correct → Settle → Remember
```

under one control invariant: the entity whose work is judged never modifies
the standard it is judged against, never observes the judge's context, and
never certifies its own completion. This document describes the shipped
architecture. Per-unit conformance truth lives ONLY in
`.agents/reports/conformance.json` and `.agents/reports/handoff.md`; this
file is capability prose that references those artifacts.

The normative specification is `.agents/specs/Core-spec.md` (§4 system
overview, §5 domain model, §6 ports). The operator's lowercase `docs/` tree
(`docs/architecture.md`, `docs/concepts.md`, …) is a separate explanatory
system; where the two disagree, the spec and the canonical reports win.

## Layers

Ports and adapters (hexagonal), with dependency direction enforced by test,
not review (spec §24.3):

```text
crates/gauntlet-domain       entities, value objects, typed enums, invariants — no infra
crates/gauntlet-ports        trait definitions, typed errors, shared contract suites
crates/gauntlet-app          controller, engines, services — ports only
crates/adapters/*            concrete implementations of one port each
crates/gauntlet-cli          composition root: binary + thin lib, wires adapters
crates/gauntlet-arch-tests   test-only crate owning the mechanical boundary scans
```

Dependencies point inward: `adapters → application → domain`. The domain has
zero dependencies. The application layer names no adapter. Vendor and
product names live only in adapter crates and configuration files.

## Ports

The stable semantic capabilities of spec §6 — `AgentRunner`, `Observer`,
`Comparator`, `Verifier`, `ReferenceProvider`, `PolicyProvider`,
`StateStore`, `EvidenceStore`, `EventLog`, `ToolProvider`, `Sandbox`,
`WorkspaceProvider`, `BudgetMeter`, `Clock`, `IdGenerator`,
`HumanEscalation`, `OperatorSurface` — plus two the memory and
self-improvement work added: `CapabilityLedger` (task 22) and
`RevisionLedger` (task 23). Every port carries typed errors and a shared
contract suite under `crates/gauntlet-ports/src/contract/`; an adapter is
admitted by passing the unmodified suite, never by writing a bespoke one.

`AgentRunner::invoke` returns an admission handle and never a result.
Completion means a schema-valid result document exists at the declared
result-contract path (spec §10.3); mailbox replies are signals, not
contracts.

## Adapters

Every adapter is a substitutable choice resolved from configuration
(spec §7, §8.4):

| Capability | Shipped adapters |
|---|---|
| AgentRunner | `agent-mock` (scripted playback), `agent-cli` (generic command template), `agent-prime` (Prime-Agent kernel) |
| Observation facets | `observe-build`, `observe-command`, `observe-filesystem`, `observe-git`, `observe-test` |
| Comparison | `compare-diff` (deterministic residual producer), `compare-model` (model critic behind the blind envelope) |
| Verification | `verify-command-exit-code`, `verify-file-digest`, `verify-schema-check`, `verify-test-report` |
| Sandbox | `sandbox-none` (refuses above-`contained`), `sandbox-bwrap` (bubblewrap) |
| Policy | `policy-fs` (strict decode of `.gauntlet/policies/<tier>.json`) |
| State / evidence / events | `state-sqlite` (rusqlite), `evidence-fs` (content-addressed), event log inside the state adapter |
| Workspace | `workspace-fs` (per-unit directory workspaces) |
| Escalation / ids / metering | `escalation-fs`, `id-entropy` (ULID-class), `meter-memory` |
| Operator surface | `surface-json` (feature-gated `surfaces` build; loopback JSON API) |

## Mechanical boundary enforcement

The layering above is enforced by `crates/gauntlet-arch-tests` via the
cargo test targets declared at `tests/contract/` (the frozen answer key's
evidence addresses):

- `tests/contract/architecture_boundaries` — dependency direction over a
  filesystem-discovered member set, scope integrity (no `#[path]`/`include!`
  escapes), the provider/harness/database/browser/sandbox denylist, and the
  answer-key single-writer scan.
- `tests/contract/adapter_registry` — adapter selection is
  configuration-driven; no adapter is chosen by a literal in domain or
  application code.
- `tests/contract/no_hardcoded_literals` — no path, model, provider,
  command, timeout, or tier-policy literal outside the declared defaults
  table (spec §8.1).
- `tests/contract/env_example_complete` — `.env.example` covers every
  recognized `GAUNTLET_*` variable with empty values.

Run them with `just boundaries` (the recipe runs the whole crate). The
suite was 237/237 green through the task-26 closure battery.

## Control flow

A run: resolve config → preflight (`doctor`) → form/freeze the reference
(digest locked) → per unit: authority gate and containment tier check →
dispatch the builder inside its workspace → observe with instruments →
build a blind comparison envelope → comparator emits residuals → verifier
binds evidence and yields `PASS`/`FAIL`/`INCONCLUSIVE` → controller selects
a correction or settles → durable settlement + evidence. Every iteration
boundary writes a checkpoint; resume replays the event log and re-freezes
the reference digest (spec §18).

## Proven vs provisional

Status categories are the six of spec §26. Where a line below names a ruler
unit (`AK-NNN`), the status word is the working ledger's claim; the
authoritative per-unit status is whatever
`.agents/reports/conformance.json` records when the task-27 sweep runs, and
this section defers to it.

- implemented_but_provisional — AK-001: the domain crate has zero
  compile-time dependencies on ports, adapters, or infrastructure;
  enforced by `tests/contract/architecture_boundaries`, green in every
  closure battery through task 26.
- implemented_but_provisional — AK-002: application source depends on port
  abstractions only and no adapter identifier appears in domain or
  application code (dependency-direction + denylist checks, same target).
- implemented_but_provisional — AK-014: adapter selection resolves from
  configuration only (`tests/contract/adapter_registry`; the doctor
  measures the configured runners rather than declaring them).
- implemented_but_provisional — AK-013: the full shared port-contract
  suite runs against two `AgentRunner` adapters — mock and generic-CLI —
  with zero test-body changes
  (`crates/gauntlet-cli/tests/p8_adapter_substitution.rs`, task 16; the
  Prime-Agent adapter is the second real adapter from task 15).
