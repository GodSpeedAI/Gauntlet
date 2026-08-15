# AGENTS.md

Portable, reference-anchored Gauntlet runtime for agentic work.

The runtime orchestrates:

**Reference → Transform → Observe → Compare → Verify → Select → Correct → Settle → Remember**

Prime-Agent is the initial orchestration harness, not the architecture. Models, agent harnesses, MCP servers, CLIs, databases, sandboxes, and browsers are replaceable adapters.

The normative specification is `.agents/specs/Core-spec.md`. The frozen ruler for this effort is `.wayfinder/answer-key.md`. Implementation plans live in `.agents/plans/`. Repeatable workflows belong in `.agents/skills/`. Role definitions live in `.agents/roles/`. **Do not duplicate those documents here.**

This file is loaded into every session. Keep it short enough that that stays worth paying for.

## Self-improvement

Treat this file as a project-specific failure log, not a wishlist.

When an agent makes a mistake, gets corrected, or repeatedly wastes effort:

1. Determine whether the failure is likely to recur.
2. If one concise repository rule would prevent it, add that rule here.
3. If prevention requires a procedure, create or update a skill in `.agents/skills/`.
4. If the failure exposes a specification or architecture defect, revise the source through its normal process — a `CandidateRevision` against the spec — rather than patching around it here.

Do not add speculative rules for failures that have never occurred.

### Failure log

- Empty until an observed repository-specific failure earns an entry.

## Delegation and role boundaries

Role boundaries are architectural boundaries, not etiquette.

- **Orchestrator**: owns discovery, decomposition, interface contracts, acceptance criteria, verification architecture, evidence review, integration, settlement, and promotion decisions. **Does not write implementation code while delegation is available.** If no delegation mechanism exists, stop with an explicit blocked condition unless `GAUNTLET_ALLOW_ORCHESTRATOR_IMPLEMENTATION=1`.
- **Builder**: implements bounded transformations. May not modify the frozen reference, comparison criteria, acceptance gates, immutable evidence, or prior settlement records. A builder's claim that work is finished carries no settlement authority.
- **Critic**: read-only. Compares an observation bundle against the frozen reference under an explicit comparison contract, in a **fresh context**, and emits residuals. Never receives builder history, reasoning, attempt counts, or identity. Never fixes anything.
- **Verifier**: decides whether a specific claim meets its declared evidence requirement. Prefers instruments over judgment.
- **Process optimizer**: proposals only.

Delegation is admission-based. `rlm(...)`-style calls return a handle, **not an answer** — never `await` a result from them. Every delegated invocation declares a result-contract path; completion means a schema-valid result file exists there. A mailbox reply is a signal, not the contract.

Never delegate the question of whether the total system actually settled.

## The testing loop

Testing is part of the development loop, not a final cleanup step.

Use the repository `justfile` as the normal human-and-agent CLI. For Rust, prefer:

```bash
cargo nextest run
```

For each bounded behavioral change:

1. Identify what observable result distinguishes correct from incorrect behavior.
2. Add or identify the relevant test first when practical.
3. Confirm the test fails for the expected reason before implementing.
4. Implement the smallest coherent change.
5. Run focused tests immediately; run broader relevant tests before declaring the slice complete.

Every port has a **shared contract suite**. Adding an adapter means passing the existing suite, not writing a bespoke one.

Use `cargo-mutants` at settlement boundaries to check that tests discriminate correct from incorrect behavior; coverage alone proves nothing. Use `cargo-deny` for dependency policy.

Never: disable a failing test to reach green; weaken an assertion because the implementation fails it; update a golden artifact merely because output differs; report success with a red relevant test; change the reference or an acceptance criterion to make implementation pass; treat `INCONCLUSIVE` as `PASS`.

If a test or a reference is wrong, demonstrate why before changing it. Changing the ruler and changing the implementation are separate operations.

## Ask before you assume

Discover before asking. Ask before inventing.

Use `rg --files` then targeted `rg` before reading large amounts of source. For Rust structural questions prefer `rust-analyzer` semantics over grep-and-recompile.

Do not invent: product behavior, acceptance criteria, comparison standards, security policy, public API semantics, or domain decisions.

Resolve ambiguity from specifications, tests, code, configuration, environment, or tools where possible. If two materially different interpretations remain and the choice affects the outcome, ask when interactive. For unattended work, record the unresolved decision as fog — never silently pick one.

Do not widen scope to fix adjacent issues. Record them separately.

## Type checking

Use the type system to prevent invalid states rather than relying on comments or agent discipline.

Rust:

- Explicit domain types, enums, and newtypes over stringly-typed state. Every fixed-value-set field in the spec is a type.
- `Outcome` is three-state; do not implement a boolean conversion for it.
- Typed errors carrying the failure locus. No bare strings, no swallowed errors.
- `unsafe` narrowly scoped with a documented invariant; avoid `unwrap`/`expect` in production paths unless impossibility is an established invariant.
- `clippy` runs as part of normal verification.

TypeScript, if introduced: strict mode, no `any`, use `unknown` then validate and narrow, no casts to silence the compiler.

Python, if introduced: type public interfaces and domain structures; typed structures over unstructured dicts for stable contracts.

**Model output is untrusted external input.** Parse, validate against the declared schema, then admit into consequential state — never before.

## Dependencies

Prefer mature, maintained dependencies over rebuilding infrastructure. Before adding one, check: does the standard library or an existing dependency solve it; maintenance activity; advisories; licence; transitive cost; coupling introduced; whether it belongs behind an adapter.

Do not add dependencies incidentally during unrelated work.

Prefer the existing environment stack: `rustup`, `rust-analyzer`, a reproducible environment manager, `uv` for Python, current Bun for TypeScript, `just` for the repository CLI, `rusqlite` for local SQLite, and an observability backend only when warranted. Keep dependency policy explicit with `cargo-deny`.

Before adopting any framework, answer: what problem does it solve; does the environment already solve it; what payment does it add; what coupling; can a smaller mechanism preserve the invariant. Prefer boring, inspectable mechanisms.

## Naming

One canonical name per concept. Search specs and code before introducing terminology.

Preserve these distinctions (full vocabulary in the spec, §1):

`Reference` · `Answer Key` · `Unit` · `Observation` · `Comparison` · `Residual` · `Verification` · `Correction` · `Settlement` · `Capability` · `Role Invocation`.

Do not substitute: goal for reference; review for verification; output for settlement; access for capability; opinion for evidence; plan for ruler.

Name ports for semantic capabilities, never vendors: `AgentRunner`, not `ClaudeRunner`. Vendor names belong in adapters and configuration.

Rust conventions: `UpperCamelCase` types/traits/enums, `snake_case` functions/modules/variables, `SCREAMING_SNAKE_CASE` constants.

## Project structure

Keep architecture discoverable from the tree. Use existing conventions once established; do not create competing layouts. Do not place new files at the repository root when an existing location fits.

```text
.agents/{specs,plans,skills,roles,reports}
.wayfinder/{map.md,answer-key.md,reference.yaml}
.gauntlet/{policies,comparison-contract.yaml}
domain/ application/ ports/ adapters/ cli/
tests/{contract,proofs,antigaming,e2e,fixtures}
```

Dependencies point inward: `adapters → application → domain`. The domain is framework- and infrastructure-independent. Provider-, model-, MCP-, persistence-, browser-, sandbox-, and observability-specific code lives behind adapters.

Generated artifacts must not become competing sources of truth.

## End-to-end verification

Unit tests are not sufficient for claims about the Gauntlet loop itself. For meaningful vertical changes, exercise the real control path:

```text
operator request → reference resolution and freeze → role dispatch → artifact change
→ independent observation → fresh-context comparison → verification → residual
→ correction → settlement → durable evidence
```

Test unhappy paths deliberately: builder failure; critic failure; malformed result document; missing result document; verifier disagreement; unavailable tool; attempted reference mutation; interrupted run; exhausted budget; non-converging correction; missing evidence; resume/recovery; adapter substitution.

A successful tool call is not settlement. Do not work around a failed E2E step and call the run successful.

### Rust compilation discipline

Compilation is expensive; do not use it as a substitute for reasoning. Use the cheapest operation that settles the current question:

```text
rust-analyzer semantics → targeted source inspection → cargo check
→ focused cargo-nextest → broader suite → cargo build only for a real linked artifact
```

## UI testing

This project's primary surface is a CLI. If a TUI, dashboard, or web surface is introduced, automated code tests are not sufficient evidence that it works: exercise the rendered interface with realistic data, inspect actual rendered state, capture screenshots where visual comparison matters, and test loading, error, empty, and boundary states. Do not infer visual correctness from source. Keep substantial UI procedures in a `.agents/skills/` workflow.

## Performance

Performance includes computational cost **and** agentic payment. Measure: wall-clock latency; CPU/memory; model tokens; provider cost; tool invocations; retries; verification cost; orchestration burden; recovery cost.

Run cheap deterministic checks before expensive model judgment. Do not invoke a frontier critic for a distinction already settled conclusively by an instrument. Conversely, do not assign an inadequate cheap judge to a comparison whose correctness determines the rest of the loop.

Do not optimize one dimension by weakening correctness or evidence. A loop that halved builder cost by tripling orchestration cost has not improved. **A workflow that got faster by skipping verification is a regression, not learning.**

Autonomous loops must be bounded: respect configured iteration, parallelism, depth, time, token, cost, and recovery-reserve limits. Absence of a configured budget is never permission for unlimited recursion.

### Rust build-artifact budget

Build caches are valuable but bounded. Do not solve disk pressure with routine `cargo clean`; preserve hot incremental state. If artifacts exceed the configured ceiling (`RUST_BUILD_CACHE_MAX_GB`, supplied by the environment — never a machine-specific literal in this repo): inventory first, preserve active build state, remove stale artifacts first, and use a full clean only when a true clean rebuild is required. Never run cleanup concurrently with an active build directory. Record unusual repeated growth as an environment failure.

## Error handling

Fail explicitly and preserve the failure locus. Do not swallow errors or convert uncertainty into success.

Distinguish failure classes: configuration · reference · formation · workspace · agent execution · tool execution · observation · comparison · verification · settlement · persistence · environment · budget · governance · recovery · inconclusive evidence.

Errors preserve enough context to diagnose, recover, retry safely, or abort.

`PASS`, `FAIL`, and `INCONCLUSIVE` are different states. **Missing evidence is not passing evidence.**

Never reconstruct critical run state from conversational memory.

## Architecture

Ports and adapters. Core Gauntlet logic depends on semantic capabilities, never on products.

Stable ports:

```text
AgentRunner  Observer  Comparator  Verifier  ReferenceProvider  PolicyProvider
StateStore   EvidenceStore  EventLog  ToolProvider  Sandbox  WorkspaceProvider
BudgetMeter  Clock  IdGenerator  HumanEscalation  OperatorSurface
```

Prime-Agent, Claude, OpenAI, MCP, SQLite, rust-analyzer, browsers, sandbox backends, and observability systems are adapters. If replacing one requires rewriting core control logic, the boundary is wrong — and a test enforces this, not a reviewer.

Non-negotiable structural rules:

- The builder never modifies the standard its work is evaluated against. Freeze, digest, re-verify.
- Formal comparison uses a fresh context, with the envelope built by the runtime from an allowlist.
- **Authority gates whether something runs; containment gates how contained it is if it does.** Both precede builder process start. Neither substitutes for the other.
- `irreversible` units are never automatic dispatch candidates, at the scheduler, regardless of any prompt.
- An agent may never lower a Unit's blast radius or authority.
- Prefer deterministic verification wherever deterministic evidence exists.
- When a residual is found, select the next affordable, governable, recoverable, settleable correction — not the largest possible change.
- Self-improvement follows candidate revision: `current → candidate → isolated evaluation → held-out/regression comparison → promote or rollback`.

The active system never moves its own ruler and then claims improvement.

Commit often and meaningful milestones amd keep .agents/CURRENT_STATUS.md current.