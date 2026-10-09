# Source Map

This document establishes canonical traceability between architectural concepts, invariants, ports, capabilities, and concrete repository implementation artifacts (files, modules, types, traits, functions, and tests).

---

## 1. Non-Negotiable Invariants & Proofs Traceability

Every invariant defined in `Core-spec.md` §3 has a structural enforcement mechanism in code and an executable proof test.

| Invariant | Concept | Structural Enforcement Mechanism | Source Locus | Proof Test & Location |
|---|---|---|---|---|
| **I1** | Reference Integrity | Digest locked at start in `.gauntlet/reference.lock`; re-verified before settlement; answer-key outside builder workspace. | [`ReferenceFreezer`](../crates/gauntlet-app/src/reference/manifest.rs), [`verify_unchanged`](../crates/gauntlet-app/src/reference/provider.rs) | `P1` — [`crates/gauntlet-cli/tests/p1_reference_integrity.rs`](../crates/gauntlet-cli/tests/p1_reference_integrity.rs) |
| **I2** | Real Observation | Observers probe reality; builder-authored text relegated to `claims` facet explicitly excluded from verification inputs. | [`Assembly::assemble_bundle`](../crates/gauntlet-app/src/observation/assembly.rs), [`ObservationBundle`](../crates/gauntlet-domain/src/observation.rs) | `P3` — [`crates/gauntlet-cli/tests/p3_real_observation.rs`](../crates/gauntlet-cli/tests/p3_real_observation.rs) |
| **I3** | Critic $\neq$ Verifier | Distinct ports (`Comparator` vs `Verifier`); distinct records (`ComparisonRecord` vs `VerificationRecord`). Settlement requires `PASS` from verifier. | [`SettlementService::evaluate_preconditions`](../crates/gauntlet-app/src/settle/mod.rs), [`Verifier`](../crates/gauntlet-ports/src/verifier.rs) | `P11` — [`crates/gauntlet-cli/tests/p11_critic_is_not_verifier.rs`](../crates/gauntlet-cli/tests/p11_critic_is_not_verifier.rs) |
| **I4** | Context Isolation | `EnvelopeBuilder` builds comparison context from an allowlist; builder trajectory, reasoning, identity, and attempt counts are structurally stripped. | [`EnvelopeBuilder::build_envelope`](../crates/gauntlet-app/src/compare/envelope.rs) | `P2` — [`crates/gauntlet-cli/tests/p2_context_isolation.rs`](../crates/gauntlet-cli/tests/p2_context_isolation.rs) |
| **I5** | Similarity Never Authorizes | Similarity and model scores produce candidates only; settlement, authority, and blast radius gates read typed enum fields. | [`AuthorityGate::admit`](../crates/gauntlet-app/src/govern/gate.rs), [`Unit`](../crates/gauntlet-domain/src/unit.rs) | `P5` — [`crates/gauntlet-cli/tests/p5_hard_gate.rs`](../crates/gauntlet-cli/tests/p5_hard_gate.rs) |
| **I6** | No False Capability Credit | Capability ledger requires $N$ distinct settlements under real variation plus recovery; single settlements write developmental memory only. | [`PromotionService::evaluate`](../crates/gauntlet-app/src/control/controller.rs), [`CapabilityLedger`](../crates/gauntlet-domain/src/capability.rs) | `P10` — [`crates/gauntlet-cli/tests/p10_no_false_learning.rs`](../crates/gauntlet-cli/tests/p10_no_false_learning.rs) |
| **I7** | Governed Self-Improvement | Optimizer produces `CandidateRevision`; promotion requires isolated evaluation against held-out tasks and regression non-inferiority. | [`CandidateRevision`](../crates/gauntlet-domain/src/capability.rs), [`gauntlet improve`](../crates/gauntlet-cli/src/improve_cmd.rs) | `P9` — [`crates/gauntlet-cli/tests/p9_self_improvement_containment.rs`](../crates/gauntlet-cli/tests/p9_self_improvement_containment.rs) |
| **I8** | Missing $\neq$ Passing | `Outcome` has three states (`PASS`, `FAIL`, `INCONCLUSIVE`); no bool coercion. Missing evidence yields `INCONCLUSIVE` and cannot settle. | [`Outcome`](../crates/gauntlet-domain/src/values.rs), [`EvidenceBinding`](../crates/gauntlet-app/src/verify/ladder.rs) | `P12` — [`crates/gauntlet-cli/tests/p12_missing_is_not_passing.rs`](../crates/gauntlet-cli/tests/p12_missing_is_not_passing.rs) |
| **I9** | Authority vs. Containment | Authority decides whether execution starts; sandbox decides how contained it is. `irreversible` units never auto-dispatch. | [`AuthorityGate`](../crates/gauntlet-app/src/govern/gate.rs), [`Sandbox`](../crates/gauntlet-ports/src/sandbox.rs) | `P5` — [`crates/gauntlet-cli/tests/p5_hard_gate.rs`](../crates/gauntlet-cli/tests/p5_hard_gate.rs) |
| **I10** | Bounded Runtime | Every budget dimension has conservative defaults; missing config never allows infinite loops. Recovery reserve is never spent forward. | [`MeteredBudget`](../crates/adapters/gauntlet-adapter-meter-memory/src/lib.rs), [`ResolvedBudget`](../crates/gauntlet-app/src/config/mod.rs) | `P6` — [`crates/gauntlet-cli/tests/p6_budget_control.rs`](../crates/gauntlet-cli/tests/p6_budget_control.rs) |
| **I11** | Durable State & Recovery | State persists to PostgreSQL and append-only event log; resumes reconstruct state from logs, never conversational memory. | [`EmbeddedStateStore`](../crates/adapters/gauntlet-adapter-state-postgres/src/lib.rs), [`RunController::resume`](../crates/gauntlet-app/src/control/controller.rs) | `P7` — [`crates/gauntlet-cli/tests/p7_recovery.rs`](../crates/gauntlet-cli/tests/p7_recovery.rs) |
| **I12** | Product-Agnostic Ports | Core control logic depends on semantic capability traits, never on concrete vendor SDKs. Adapters pass shared contract suites. | [`AgentRunner`](../crates/gauntlet-ports/src/agent_runner.rs), [`contract::agent_runner`](../crates/gauntlet-ports/src/contract/agent_runner.rs) | `P8` — [`crates/gauntlet-cli/tests/p8_adapter_substitution.rs`](../crates/gauntlet-cli/tests/p8_adapter_substitution.rs) |

---

## 2. Core Ports & Implementing Adapters

Gauntlet's 17 capability ports are defined in `crates/gauntlet-ports` and implemented by 24 adapters in `crates/adapters/*`.

| Port Name | Trait Definition | Shared Contract Suite | Implementing Adapters | Primary Symbols |
|---|---|---|---|---|
| **`AgentRunner`** | [`crates/gauntlet-ports/src/agent_runner.rs`](../crates/gauntlet-ports/src/agent_runner.rs) | [`contract/agent_runner.rs`](../crates/gauntlet-ports/src/contract/agent_runner.rs) | `adapter-agent-mock`<br>`adapter-agent-cli`<br>`adapter-agent-prime` | `MockAgentRunner`<br>`CliAgentRunner`<br>`PrimeAgentRunner` |
| **`Observer`** | [`crates/gauntlet-ports/src/observer.rs`](../crates/gauntlet-ports/src/observer.rs) | [`contract/observer.rs`](../crates/gauntlet-ports/src/contract/observer.rs) | `adapter-observe-build`<br>`adapter-observe-command`<br>`adapter-observe-filesystem`<br>`adapter-observe-git`<br>`adapter-observe-test` | `BuildObserver`<br>`CommandObserver`<br>`FilesystemObserver`<br>`GitObserver`<br>`TestObserver` |
| **`Comparator`** | [`crates/gauntlet-ports/src/comparator.rs`](../crates/gauntlet-ports/src/comparator.rs) | [`contract/comparator.rs`](../crates/gauntlet-ports/src/contract/comparator.rs) | `adapter-compare-diff`<br>`adapter-compare-model` | `DiffComparator`<br>`ModelComparator` |
| **`Verifier`** | [`crates/gauntlet-ports/src/verifier.rs`](../crates/gauntlet-ports/src/verifier.rs) | [`contract/verifier.rs`](../crates/gauntlet-ports/src/contract/verifier.rs) | `adapter-verify-command-exit-code`<br>`adapter-verify-file-digest`<br>`adapter-verify-schema-check`<br>`adapter-verify-test-report` | `CommandExitVerifier`<br>`FileDigestVerifier`<br>`SchemaCheckVerifier`<br>`TestReportVerifier` |
| **`StateStore`** | [`crates/gauntlet-ports/src/state_store.rs`](../crates/gauntlet-ports/src/state_store.rs) | [`contract/state_store.rs`](../crates/gauntlet-ports/src/contract/state_store.rs) | `adapter-state-postgres` | `EmbeddedStateStore` |
| **`EvidenceStore`** | [`crates/gauntlet-ports/src/evidence_store.rs`](../crates/gauntlet-ports/src/evidence_store.rs) | [`contract/evidence_store.rs`](../crates/gauntlet-ports/src/contract/evidence_store.rs) | `adapter-evidence-fs` | `FsEvidenceStore` |
| **`EventLog`** | [`crates/gauntlet-ports/src/event_log.rs`](../crates/gauntlet-ports/src/event_log.rs) | [`contract/event_log.rs`](../crates/gauntlet-ports/src/contract/event_log.rs) | `adapter-state-postgres` | `EmbeddedStateStore::append_event` |
| **`WorkspaceProvider`** | [`crates/gauntlet-ports/src/workspace_provider.rs`](../crates/gauntlet-ports/src/workspace_provider.rs) | [`contract/workspace_provider.rs`](../crates/gauntlet-ports/src/contract/workspace_provider.rs) | `adapter-workspace-fs` | `FsWorkspaceProvider` |
| **`BudgetMeter`** | [`crates/gauntlet-ports/src/budget_meter.rs`](../crates/gauntlet-ports/src/budget_meter.rs) | [`contract/budget_meter.rs`](../crates/gauntlet-ports/src/contract/budget_meter.rs) | `adapter-meter-memory` | `MeteredBudget` |
| **`Sandbox`** | [`crates/gauntlet-ports/src/sandbox.rs`](../crates/gauntlet-ports/src/sandbox.rs) | [`contract/sandbox.rs`](../crates/gauntlet-ports/src/contract/sandbox.rs) | `adapter-sandbox-bwrap`<br>`adapter-sandbox-none` | `ProcessIsolationBackend`<br>`NoneIsolationBackend` |
| **`PolicyProvider`** | [`crates/gauntlet-ports/src/policy_provider.rs`](../crates/gauntlet-ports/src/policy_provider.rs) | [`contract/policy_provider.rs`](../crates/gauntlet-ports/src/contract/policy_provider.rs) | `adapter-policy-fs` | `PolicyFs` |
| **`ReferenceProvider`** | [`crates/gauntlet-ports/src/reference_provider.rs`](../crates/gauntlet-ports/src/reference_provider.rs) | [`contract/reference_provider.rs`](../crates/gauntlet-ports/src/contract/reference_provider.rs) | `gauntlet-app::reference` | `ReferenceFreezer`, `ReferenceValidator` |
| **`HumanEscalation`** | [`crates/gauntlet-ports/src/human_escalation.rs`](../crates/gauntlet-ports/src/human_escalation.rs) | [`contract/human_escalation.rs`](../crates/gauntlet-ports/src/contract/human_escalation.rs) | `adapter-escalation-fs` | `FsEscalationQueue` |
| **`IdGenerator`** | [`crates/gauntlet-ports/src/id_generator.rs`](../crates/gauntlet-ports/src/id_generator.rs) | [`contract/id_generator.rs`](../crates/gauntlet-ports/src/contract/id_generator.rs) | `adapter-id-entropy`<br>`adapter-agent-mock` | `UlidIdGenerator`<br>`SeededIdGenerator` |
| **`Clock`** | [`crates/gauntlet-ports/src/clock.rs`](../crates/gauntlet-ports/src/clock.rs) | [`contract/clock.rs`](../crates/gauntlet-ports/src/contract/clock.rs) | `std::time`<br>`adapter-agent-mock` | `SystemClock`<br>`DeterministicClock` |
| **`ToolProvider`** | [`crates/gauntlet-ports/src/tool_provider.rs`](../crates/gauntlet-ports/src/tool_provider.rs) | [`contract/tool_provider.rs`](../crates/gauntlet-ports/src/contract/tool_provider.rs) | `gauntlet-app::roles` | `RoleToolProvider` |
| **`OperatorSurface`** | [`crates/gauntlet-ports/src/operator_surface.rs`](../crates/gauntlet-ports/src/operator_surface.rs) | [`contract/operator_surface.rs`](../crates/gauntlet-ports/src/contract/operator_surface.rs) | `adapter-surface-json`<br>`gauntlet-cli` | `JsonOperatorSurface`<br>`CliSurface` |

---

## 3. Application Services & Control Flow

The orchestration engine lives in `crates/gauntlet-app`.

| Service / Component | Source File | Key Responsibilities | Primary Functions & Structs |
|---|---|---|---|
| **`RunController`** | [`crates/gauntlet-app/src/control/controller.rs`](../crates/gauntlet-app/src/control/controller.rs) | Manages overall run lifecycle, preflight execution, iteration dispatches, terminal states, and resumption. | `RunController::run_loop()`, `RunController::execute_iteration()`, `RunController::resume()` |
| **`UnitScheduler`** | [`crates/gauntlet-app/src/control/scheduler.rs`](../crates/gauntlet-app/src/control/scheduler.rs) | Computes dependency order for Units, checks tier safety, and yields eligible dispatch slices. | `UnitScheduler::schedule_slice()`, `UnitScheduler::filter_eligible()` |
| **`CorrectionSelector`** | [`crates/gauntlet-app/src/control/select.rs`](../crates/gauntlet-app/src/control/select.rs) | Scores candidate fixes across 10 dimensions, filters out governance-blocked paths, selects minimal fix, and records rejected alternatives. | `CorrectionSelector::select_correction()`, `CorrectionSelector::score_candidate()` |
| **`DiagnosticEngine`** | [`crates/gauntlet-app/src/control/diagnosis.rs`](../crates/gauntlet-app/src/control/diagnosis.rs) | Handles non-converging residuals; designs cheap discriminating tests within diagnostic budget. | `DiagnosticEngine::diagnose()`, `DiagnosticEngine::discriminate()` |
| **`SettlementService`** | [`crates/gauntlet-app/src/settle/mod.rs`](../crates/gauntlet-app/src/settle/mod.rs) | Enforces 5 preconditions before settlement; executes 3-step atomic commit sequence. | `SettlementService::settle_unit()`, `SettlementService::evaluate_preconditions()` |
| **`ReferenceFreezer`** | [`crates/gauntlet-app/src/reference/manifest.rs`](../crates/gauntlet-app/src/reference/manifest.rs) | Normalizes reference YAML and answer-key markdown, computes SHA-256 digest, and writes `.gauntlet/reference.lock`. | `ReferenceFreezer::freeze()`, `ReferenceFreezer::verify_digest()` |
| **`EnvelopeBuilder`** | [`crates/gauntlet-app/src/compare/envelope.rs`](../crates/gauntlet-app/src/compare/envelope.rs) | Builds comparison envelopes using strict allowlists, stripping builder trajectory and reasoning. | `EnvelopeBuilder::build_envelope()`, `EnvelopeBuilder::digest()` |
| **`AuthorityGate`** | [`crates/gauntlet-app/src/govern/gate.rs`](../crates/gauntlet-app/src/govern/gate.rs) | Evaluates unit blast radius against role permissions before process start; creates escalation tickets. | `AuthorityGate::admit()`, `AuthorityGate::wrap_builder_command()` |
| **`Doctor`** | [`crates/gauntlet-app/src/doctor/mod.rs`](../crates/gauntlet-app/src/doctor/mod.rs) | Executes the 8 preflight checks before any run begins; verifies writable paths, policies, and budgets. | `run_doctor()`, `DoctorReport`, `CheckOutcome` |
| **`ConfigResolver`** | [`crates/gauntlet-app/src/config/mod.rs`](../crates/gauntlet-app/src/config/mod.rs) | Layered config resolution (`defaults < GAUNTLET.md < env < flags`), computes `config_fingerprint`. | `resolve()`, `EffectiveConfig`, `config_fingerprint()` |

---

## 4. CLI Surface & Composition Root

All runtime assembly occurs in `crates/gauntlet-cli`.

| Subcommand | Source Implementation | Underlying Application Service |
|---|---|---|
| `gauntlet doctor` | [`crates/gauntlet-cli/src/doctor_cmd.rs`](../crates/gauntlet-cli/src/doctor_cmd.rs) | `gauntlet_app::doctor::run_doctor` |
| `gauntlet init` | [`crates/gauntlet-cli/src/init_cmd.rs`](../crates/gauntlet-cli/src/init_cmd.rs) | Skeleton generation for `.wayfinder/`, `GAUNTLET.md`, and `.gauntlet/` |
| `gauntlet reference` | [`crates/gauntlet-cli/src/reference_cmd.rs`](../crates/gauntlet-cli/src/reference_cmd.rs) | `gauntlet_app::reference::{freeze, normalize, validate, diff}` |
| `gauntlet run` | [`crates/gauntlet-cli/src/run_cmd.rs`](../crates/gauntlet-cli/src/run_cmd.rs) | `gauntlet_cli::compose::build_controller` → `RunController::run_loop` |
| `gauntlet status` | [`crates/gauntlet-cli/src/state_cmd.rs`](../crates/gauntlet-cli/src/state_cmd.rs) | `StateStore::get_run_summary` |
| `gauntlet inspect` | [`crates/gauntlet-cli/src/state_cmd.rs`](../crates/gauntlet-cli/src/state_cmd.rs) | `StateStore::inspect_iteration`, `StateStore::inspect_unit` |
| `gauntlet verify` | [`crates/gauntlet-cli/src/verify_cmd.rs`](../crates/gauntlet-cli/src/verify_cmd.rs) | `gauntlet_app::verify::ladder::verify_all` |
| `gauntlet resume` | [`crates/gauntlet-cli/src/run_cmd.rs`](../crates/gauntlet-cli/src/run_cmd.rs) | `RunController::resume` via event log replay |
| `gauntlet escalations` | [`crates/gauntlet-cli/src/escalations_cmd.rs`](../crates/gauntlet-cli/src/escalations_cmd.rs) | `HumanEscalation` port & `SettlementService` |
| `gauntlet roles` | [`crates/gauntlet-cli/src/roles_cmd.rs`](../crates/gauntlet-cli/src/roles_cmd.rs) | `gauntlet_app::roles::{install, verify, list}` |
| `gauntlet skills` | [`crates/gauntlet-cli/src/skills_cmd.rs`](../crates/gauntlet-cli/src/skills_cmd.rs) | `gauntlet_app::skills::{install, verify, list}` |
| `gauntlet export` | [`crates/gauntlet-cli/src/state_cmd.rs`](../crates/gauntlet-cli/src/state_cmd.rs) | `gauntlet_app::state::export_run` (deterministic JSONL) |
| `gauntlet serve` | [`crates/gauntlet-cli/src/serve_cmd.rs`](../crates/gauntlet-cli/src/serve_cmd.rs) | `gauntlet_adapter_surface_json::JsonOperatorSurface` (feature `surfaces`) |

---

## 5. Architectural Boundary Enforcement

Layering and vendor-freedom rules are strictly enforced by `crates/gauntlet-arch-tests`.

| Architectural Gate | Test File | What It Prevents |
|---|---|---|
| **Dependency Direction** | [`crates/gauntlet-arch-tests/src/deps.rs`](../crates/gauntlet-arch-tests/src/deps.rs) | Domain importing ports/app/adapters; app importing adapters; adapters importing cli. |
| **Vendor Denylist** | [`crates/gauntlet-arch-tests/src/denylist.rs`](../crates/gauntlet-arch-tests/src/denylist.rs) | Vendor-specific identifiers (`sqlite`, `bwrap`, `prime_agent`, etc.) appearing in domain or application sources. |
| **Literal Scanner** | [`crates/gauntlet-arch-tests/src/literals.rs`](../crates/gauntlet-arch-tests/src/literals.rs) | Hardcoded timeouts, model names, absolute paths, or URLs outside `config/defaults.rs`. |
| **Single Writer** | [`crates/gauntlet-arch-tests/src/single_writer.rs`](../crates/gauntlet-arch-tests/src/single_writer.rs) | Multiple modules writing to `.wayfinder/answer-key.md` or `.gauntlet/reference.lock`. |
| **Scope Escapes** | [`crates/gauntlet-arch-tests/src/escapes.rs`](../crates/gauntlet-arch-tests/src/escapes.rs) | `#[path]` and `include!` directives bypassing crate boundary scans. |

---

## 6. Operator Workbench Subsystem (`workbench/`)

Canonical mapping of the Bun + TypeScript + OpenTUI operator workbench modules, interfaces, state objects, and test suites.

| Subsystem Component | Source Implementation | Primary Symbols & Roles | Test & Benchmark Locus |
|---|---|---|---|
| **Workbench Host Loop** | [`workbench/src/main.ts`](../workbench/src/main.ts) | `run()`, `renderFrame()`, `enableRawMode()`, `restoreTerminal()` | [`operator-journeys.test.ts`](../workbench/src/operator-journeys.test.ts) |
| **Semantic Shell** | [`workbench/src/shell.ts`](../workbench/src/shell.ts) | `ShellSession`, `renderShell()`, `handleKey()`, `SurfaceId`, `ViewId` | [`opentui-shell.test.ts`](../workbench/src/opentui-shell.test.ts) |
| **Workbench Model** | [`workbench/src/workbench-model.ts`](../workbench/src/workbench-model.ts) | `WorkbenchModel`, `FullWorkbenchSnapshot`, `immutableHistoryView()` | [`workbench-model.test.ts`](../workbench/src/workbench-model.test.ts) |
| **Projection Store** | [`workbench/src/projection-store.ts`](../workbench/src/projection-store.ts) | `SqliteProjectionStore`, `ProjectionGeneration`, `AuthoritativeEventFrame` | [`cold-rebuild.test.ts`](../workbench/src/cold-rebuild.test.ts), [`stale-frame.test.ts`](../workbench/src/stale-frame.test.ts) |
| **Operation Gateway** | [`workbench/src/operation-gateway.ts`](../workbench/src/operation-gateway.ts) | `OperationGateway::drive()`, `idempotencyKey()`, `revalidate()` | [`canonical-boundary.test.ts`](../workbench/src/canonical-boundary.test.ts) |
| **Operation Registry** | [`workbench/src/operation-registry.ts`](../workbench/src/operation-registry.ts) | `lookupOperation()`, `operationIds`, `OperationSpec` | [`operation-isomorphism.test.ts`](../workbench/src/operation-isomorphism.test.ts) |
| **Affordance Resolver** | [`workbench/src/affordance.ts`](../workbench/src/affordance.ts) | `resolveAffordance()`, `AffordanceDescriptor`, `ReasonCode` | [`affordance-resolver.test.ts`](../workbench/src/affordance-resolver.test.ts) |
| **Focus & Placement** | [`workbench/src/focus.ts`](../workbench/src/focus.ts) | `FocusRef`, `FocusPlacement`, `createPlacement()` | [`focus-identity.test.ts`](../workbench/src/focus-identity.test.ts) |
| **Horizon & Hoist** | [`workbench/src/horizon.ts`](../workbench/src/horizon.ts) | `createHorizon()`, `deriveAttention()`, `SavedHorizonRegistry` | [`horizon-hoist.test.ts`](../workbench/src/horizon-hoist.test.ts) |
| **Composer & Takeover** | [`workbench/src/composer.ts`](../workbench/src/composer.ts) | `createComposerState()`, `beginTakeover()`, `completeTakeover()` | [`composer-takeover.test.ts`](../workbench/src/composer-takeover.test.ts) |
| **Inspector & Lenses** | [`workbench/src/inspector.ts`](../workbench/src/inspector.ts) | `renderInspector()`, `InspectorSection`, `renderEvidenceRecord()` | [`inspector-lens-wiring.test.ts`](../workbench/src/inspector-lens-wiring.test.ts) |
| **Work View** | [`workbench/src/views-work.ts`](../workbench/src/views-work.ts) | `renderWorkView()`, `workUnitOrder()`, `statusToken()` | [`work-chat-trace.test.ts`](../workbench/src/work-chat-trace.test.ts) |
| **Chat & Trace Views** | [`workbench/src/views-chat-trace.ts`](../workbench/src/views-chat-trace.ts) | `renderChatView()`, `renderTraceView()`, `defaultViewRegistry()` | [`work-chat-trace.test.ts`](../workbench/src/work-chat-trace.test.ts) |
| **Geometry Engine** | [`workbench/src/geometry.ts`](../workbench/src/geometry.ts) | `classifyGeometry()`, `calculateColumns()`, `renderBelowMinimumPrompt()` | [`opentui-shell.test.ts`](../workbench/src/opentui-shell.test.ts) |
| **Theme & Tokens** | [`workbench/src/theme.ts`](../workbench/src/theme.ts) | `resolveToken()`, `SEMANTIC_TOKENS`, `statusDescriptor()` | [`theme.ts`](../workbench/src/theme.ts) |
| **Prime Host Bridge** | [`workbench/src/prime-host.ts`](../workbench/src/prime-host.ts) | `PrimeHost`, `SessionHandle`, `admitPrimeCall()` | [`prime-host.test.ts`](../workbench/src/prime-host.test.ts) |
| **Prime Kernel Bridge** | [`workbench/src/prime-kernel.ts`](../workbench/src/prime-kernel.ts) | `PrimeKernel`, `KernelState`, `serializeKernel()` | [`prime-kernel.test.ts`](../workbench/src/prime-kernel.test.ts) |
| **DomainForge Adapter** | [`workbench/src/domainforge-adapter.ts`](../workbench/src/domainforge-adapter.ts) | `DomainForgeAdapter`, `renderSemanticSection()`, `ASTProjection` | [`domainforge-adapter.test.ts`](../workbench/src/domainforge-adapter.test.ts) |
| **SXR RealityTrace Lens** | [`workbench/src/sxr-adapter.ts`](../workbench/src/sxr-adapter.ts) | `SxrAdapter`, `renderSxrEvidenceSection()`, `VerificationPath` | [`sxr-evidence-adapter.test.ts`](../workbench/src/sxr-evidence-adapter.test.ts) |
| **Plugin Lifecycle** | [`workbench/src/plugins.ts`](../workbench/src/plugins.ts) | `PluginRegistry`, `registerPlugin()`, `unloadPlugin()` | [`plugin-lifecycle.test.ts`](../workbench/src/plugin-lifecycle.test.ts) |
| **Preflight Verifier** | [`scripts/tui-preflight.sh`](../scripts/tui-preflight.sh) | `bootstrap`, `conformance` mode verification | [`config-preflight.test.ts`](../workbench/src/config-preflight.test.ts) |
| **Normative Test Runner** | [`workbench/src/tui-test-all.ts`](../workbench/src/tui-test-all.ts) | `just tui-test all` full normative conformance execution | All normative task suites (T00A..T24) |
| **Design Contract** | [`DESIGN.md`](../DESIGN.md) | DSN-001 through DSN-015 design conformance index | Preflight hash binding (`02ce58a2...`) |

