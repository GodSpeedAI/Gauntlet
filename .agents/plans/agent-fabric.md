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

## E2 design — Gauntlet-owned invocation graph (frozen 2026-10-08)

Port `InvocationGraph` (gauntlet-ports), implemented by the PostgreSQL adapter inside the same database/transactions as run state, plus an in-memory reference implementation that the same contract suite also runs against (proves backend-independent semantics and serves broker unit tests).

Distinct identities (never collapsed): `RunId`, `InvocationId`, `parent: Option<InvocationId>`, `root: InvocationId`, `RunnerProfileId`, `HarnessId`, `NativeSessionRef` (+ per-invocation `session_epoch`), `WorkspaceRef`; OS/process identity stays adapter forensics, not graph truth.

`InvocationNode { invocation_id, run_id, parent, root, depth, role, runner_profile, harness, workspace, context_digest, result_contract, admission_key, budget (reserved BudgetSlice), status, native_session: Option<(ref, epoch)>, admitted_at, terminal_at }`. Status = Admitted | Running | Completed | Failed(class) | TimedOut | Cancelled. Harness lifecycle states are NOT node status.

Atomic operations (one transaction each; PostgreSQL: row lock `FOR UPDATE` on the parent, UNIQUE(run_id, admission_key), recursive CTEs, FKs/CHECKs):
* `admit(AdmissionRequest) -> Admitted(node) | AlreadyAdmitted(node) | Refused(reason)`: verifies parent exists and is non-terminal, depth < max_depth, live children < max_children, sum of live children's reserved budget + slice ≤ parent budget (componentwise), then inserts. Same key + same facts = idempotent replay; same key + different facts = refusal. Exactly-once logical admission under crash/retry. Root admission (parent = None) takes the run's budget slice.
* `attach_native_session(id, ref)` bumps `session_epoch`; later observations must name (ref, epoch).
* `complete(id, ResultBinding{contract_path, envelope_digest, result_digest})`: refused unless node non-terminal AND the binding equals the node's admitted contract path and context digest. A child can never complete another child's contract.
* `fail/time_out`, `cancel_subtree(id)`: marks every non-terminal descendant Cancelled in one transaction and returns them for physical cancellation; settled descendants stay historical facts. No detached children.
* Queries: node, children, descendants, ancestors, active (non-terminal) nodes of a run, for restart reconstruction.
Reconstruction order after restart: PostgreSQL graph -> active/recoverable invocations -> reattach/reconcile native sessions. Never from harness topology.
Slices: E2a ports+reference+suite, E2b PostgreSQL adapter + migration, E2c DelegationBroker (app), E3 lifecycle observation sequencing (session-epoch aware).

## Phase I result — slice 1b: per-invocation MCP bridge

**Credential finding (read first).** The user's Prime agent dir (`~/.prime/agent`, the default when `PRIME_AGENT_CODING_AGENT_DIR` is unset) holds `auth.json` (OAuth-style logins for `openai-codex` and `clinepass`), `models.json` (provider definitions), and `settings.json` (defaults plus `mcpServers`, which may carry secrets). It also holds stateful directories (`sessions`, `logs`, `session-leases`, `daemon-workers`, `session-artifacts`). Replacing that dir with a bare generated one would drop the logins and the state. Copying the files would duplicate credentials. So the bridge copies NO credential and NO user `mcpServers`:

* `auth.json` and `models.json` are SYMLINKS to the user's files, created only when they exist. Provider auth still resolves through the link.
* `settings.json` is GENERATED: `mcpServers.gauntlet` plus three allowlisted non-secret defaults (`defaultModel`, `defaultProvider`, `defaultThinkingLevel`). Nothing else is carried. A malformed user settings file refuses admission.
* Caveat, OPEN (OD-11): Prime writes `auth.json` by temp-file-then-rename, which replaces the symlink with a local file. A token refreshed inside a bridged invocation does not reach the user's file. Needs an operator decision before bridged profiles use OAuth providers.

**Design.** Profile key `mcp_bridge = true` (recursive token only; strict key). A bridged invocation gets `<state>/runs/<run>/inv/<invocation>/agent/`. Its `settings.json` names the server as `{"type":"stdio","command":<gauntlet exe>,"args":[fabric,serve,--state,…,--run,<run>,--caller,<invocation id>]}`. The fixed prefix comes from the `fabric serve` process's own flags (state, fabric-dir, workspace, answer-key, optional program host), with absolute paths. `--caller` comes from the runner contract (`RoleInvocationSpec.invocation_id`), never from the model or a profile. The harness is launched with `PRIME_AGENT_CODING_AGENT_DIR` set to that dir, and so are its stop/list control calls. No other variable is added. `admit-root` and `doctor` compose without a serve launch, so a bridged profile there is reported UNAVAILABLE and selection refuses it. Composition still succeeds.

**Live-verified (2026-10-09).** `cargo test -p gauntlet-adapter-agent-prime --test mcp_bridge_live -- --ignored --nocapture` passes: `prime-agent mcp list` under a provisioned dir lists `gauntlet: stdio`. No model was called.

**Hermetic-verified.** Exact `settings.json` text and exact argv; no user secret in the generated file; credentials are symlinks, not copies; two invocations get distinct dirs and callers; the submission carries the agent dir only when bridged; the rpc command adds only the agent-dir variable; the bridged runner path end to end through a recording transport; strict `mcp_bridge` key (recursive only); unavailable without a launch; serve-launch argv.

**NOT live-verified.** (1) A real rpc run that starts the bridged server and makes `mcp.call_tool("gauntlet", "agents.spawn", ...)` from Python. This needs a model call and was not run. (2) That a real child's `stop`/`list` answers under the per-invocation dir; the scoping follows the harness's agent-dir resolution, read from source only. (3) That provider auth resolves through the symlinks in a real model run.

**Native-child limitation (OD-10).** A harness-native `rlm.spawn` child reuses its parent's MCP settings, so its `agents.spawn` calls carry the PARENT's caller id. Children admitted through `agents.spawn` get their own graph node and id.

**Deviations from the brief.** (a) The adapter did not have an env allowlist. It inherits the full environment, unchanged (OD-12). (b) `--run` is in the fixed prefix, because `fabric serve` requires it. The per-invocation addition is `--caller` only. (c) No new `limits.toml` key: the bridge's paths come from the serving process's own flags, so there is no second copy of them to drift. (d) `HarnessControl::stop`/`list` take the agent dir. This is an adapter-internal port change; the shared ports contract suite is untouched.

**Risks.** Credential refresh divergence (OD-11). The model can read the generated `settings.json` and the symlinked files, because containment is still `isolation: none`. Caller identity is only as strong as the serve process's admission check (`caller ... is not admitted`). Native-child attribution (OD-10).
