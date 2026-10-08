# Agent Fabric — frozen design and phase plan

Status: Phase A complete (2026-10-07). Governs the multi-harness evolution. Normative spec remains `.agents/specs/Core-spec.md`; nothing here weakens it.

## Donor pins (see `.tmp/donor/manifest.toml`, ignored, not a runtime dependency)

| donor | commit | license |
|---|---|---|
| prime-agent 0.9.8 (HEAD; upstream also tags 0.9.9-beta.*) | 2ed8356 | MIT |
| herdr | 4e624cd | Apache-2.0 (NOTICE needed if code lifted; ideas/data re-expressed) |
| pi (incl. packages/codemode 1.0.4, quickjs-wasi 3.6.2) | f10993b | MIT |
| herdr-orchestrate (secondary) | 281a462 | MIT |

Local `prime-agent` upgraded 0.8.0 → 0.9.8 via the vendor installer (`install-rust.sh`); old launcher kept in `.tmp/recon/`.

## Recon findings that drive the design

1. Prime 0.9.8: `--version` prints to stderr (existing probe already merges streams); `await rlm(...)`, `rlm.run`, `rlm.background` are **removed**; `rlm.spawn(name)` returns an admission handle; long-lived parent = `--mode rpc` (protocol v3, JSONL) not `--print`.
2. `host_request(type, payload)` has a **hardcoded** type table — no pluggable handler. Phase I therefore cannot use host_request without forking Prime. Route instead: Gauntlet exposes an **MCP stdio server**; Prime Python reaches it with `rlm.mcp.call_tool(...)`. (To be verified by probe, not assumed.)
3. Codemode: QuickJS-WASI in a worker, `tools.*` injected as async host callbacks, JSON bridge, no ambient fs/net, timeout via interrupt flag, `store/load` state, no built-in concurrency cap (we add one), schemas only shape declarations (we validate in the invoker).
4. Herdr: 24 agents; detection = TOML manifests (screen/OSC rules) + `AgentSessionRef {Id|Path}` + argv-array resume with dedupe; `skip_state_update` for alt-screen. Manifests are data → the catalog can be re-expressed as Gauntlet-owned TOML.
5. Existing Gauntlet surfaces confirmed by reading source: `AgentRunner` (invoke/poll/collect/cancel/capabilities), three hardcoded runner tokens in `gauntlet-cli/src/compose.rs`, result-contract completion, `OperationGateway` (workbench), `prime-recursion.ts` (T09–T11).

## Layering decisions

* `gauntlet-domain`: new pure types only — `HarnessId`, `RunnerProfileId`, `SupportLevel`, `LifecycleSource`, `EffectClass`, `InvocationLineage` (parent, root, depth). No vendor strings.
* `gauntlet-ports`: `RunnerRegistry` port, `DelegationBroker` request/decision types, `ToolProgramRuntime` port, `ToolInvoker`/`ToolCatalog` ports.
* `gauntlet-app`: DelegationBroker logic (envelope, eligibility, selection, depth/child/budget admission, lineage persistence), deterministic selection policy.
* adapters: `gauntlet-adapter-harness-catalog` (data-driven HarnessDefinition loader + Herdr-derived TOML + custom-definition validation), `gauntlet-adapter-agent-interactive` (InteractiveAgentRunner over a PTY/process transport), Prime dialect layer inside the existing prime adapter, Codemode host under `workbench/` headless (Bun) speaking JSONL to a Rust `ToolProgramRuntime` adapter.
* Harness/vendor identifiers remain adapter data + config (AK-002/AK-014 stay green).
* Terminology: executable-tool vocabulary is `ToolCatalog / ToolOperation / ToolInvoker / SemanticToolRuntime / ToolProgramRuntime`; "Capability" is untouched.

## Phases (each = reviewed checkpoint; one compiling agent at a time — 7.8 GB WSL)

* **B** Prime dialect + capability probe + doctor (rewrite `probe.rs` to behavioral probing; `--mode rpc`; `rlm.spawn` in program.rs; version = metadata only).
* **C** HarnessDefinition TOML schema + validator + Herdr-derived catalog (24) + RunnerProfile/Instance + RunnerRegistry; compose.rs resolves profiles dynamically, old three tokens kept as built-in profiles.
* **D** InteractiveAgentRunner (levels 0–2 first; Level 3 where harness reports structured state), lifecycle = forensics only.
* **E** DelegationBroker + durable invocation graph (new state-store tables → ASK-BEFORE schema change; additive migration only).
* **F** `agents.spawn/status/collect/cancel` affordances through the shared registry; generalize `prime-recursion.ts`.
* **G** ToolCatalog/ToolInvoker + effect classes.
* **H** Codemode host + `ToolProgramRuntime` adapter + mock.
* **I** Prime→Codemode via MCP stdio bridge.
* **J** hardening: conformance/proofs, doctor, `just sync-harness-catalog` (report-only), docs.

## Invariants every phase must keep (tests named in the task brief §28)

Harness lifecycle (`done`/`idle`) never completes an invocation; only a schema-valid result contract bound to invocation id + envelope digest does. Fresh context is runtime-built from an allowlist. Gauntlet owns the invocation graph (depth, children, budget, cancel, recovery). No side doors around `OperationGateway`. Unknown/unavailable runner or capability mismatch fails closed. Resume argv are arrays, never shell strings.

## Process rules for this effort

Orchestrator designs, reviews, runs integration gates; Haiku subagents implement in tightly scoped slices with explicit file lists. Orchestrator verifies every slice by diff + the narrowest `just` target. Only one agent compiles Rust/Bun at a time.

## Amendment 2026-10-07 (E0) — PostgreSQL is the canonical durable store

Operator decision: SQLite is replaced, not migrated (single user; no upgrade path, no dual store). PostgreSQL is the persistence contract; pg0 (vectorize-io/pg0 v0.15.2, MIT, PostgreSQL 18.1, binary `pg0-linux-x86_64-gnu` sha256 `06c4678ff7b461dba8ed1f95c5e202ead3b57b70f549a22c17a94985121c0bf5`, fetched with `gh release download`, installed at `.tmp/bin/pg0`) is only the local substrate. CI/server use ordinary PostgreSQL via the same URL.

* Baseline (post-cleanup): `gauntlet-arch-tests` 261/261. Clean.
* Client: sync `postgres` crate (ports are synchronous/single-threaded), declared in the adapter manifest and recorded in `ADAPTER_EXTERNAL_ALLOWLIST`. No vendor word outside `crates/adapters/**`.
* Connection: server URL from env hook `GAUNTLET_DATABASE_URL` (Annex E + `.env.example`), default = the adapter's local-pg0 URL (adapter-owned literal). Isolation unit = one PostgreSQL schema (`gauntlet_<hash of state dir>`), so every test/state dir is isolated without churn.
* Event-log offsets are gapless via an in-transaction counter row (a sequence would leave gaps after rollback and the tamper-evidence suite treats gaps as damage).
* Migrations collapse to one final-form `initial` migration (no history to preserve), applied under an advisory lock; migration machinery stays for the future.
* Slices: E1a scaffold+event-log+ledgers → E1b state_store → E1c crash/tamper/gate tests → E1d CLI/compose/docs/Annex E/justfile `db-*`/arch allowlist, delete the sqlite crate + rusqlite.
* Invocation graph (E2) lives in the same database/transactions; no independent LineageStore truth. Identities kept distinct: run, invocation, parent, root, runner profile, harness kind, native session ref, workspace, OS process.
* Lifecycle sequencing (E3) is mandatory, not a limitation: observations carry (invocation, native_session_ref, source, sequence, observed_at); stale sequences and observations of a replaced native session can never mutate current state.
