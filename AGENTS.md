# AGENTS.md

Durable operating contract for Gauntlet agents. Keep this file project-specific, behavior-changing, and earned; task state belongs in `.agents/`, detailed procedures in `.agents/skills/`, roles in `.agents/roles/`, plans in `.agents/plans/`, and normative behavior in `.agents/specs/`.

Gauntlet is a portable, reference-anchored runtime for:

**Reference → Transform → Observe → Compare → Verify → Select → Correct → Settle → Remember**

Prime-Agent is the initial harness, not the architecture. Models, harnesses, MCP servers, CLIs, stores, sandboxes, and browsers are replaceable adapters.

## 1. Scope, Precedence, and Instruction Topology

The normative specification is `.agents/specs/Core-spec.md`; `.wayfinder/answer-key.md` is the frozen ruler for this effort.

Before substantial work:

1. Read this file.
2. Read `.agents/CURRENT_STATUS.yml`.
3. Read the governing spec and active plan.
4. Reconcile `git status`, task reports, and any `IN-FLIGHT.md` before assuming work is unstarted.
5. Inspect affected code, tests, configuration, and evidence.
6. Load additional context only when required.

Instruction precedence:

1. Runtime/system safety and user constraints.
2. Nearest scoped `AGENTS.md` (`crates/AGENTS.md`, `workbench/AGENTS.md`, `.agents/AGENTS.md`).
3. This root `AGENTS.md`.
4. Frozen reference and normative specs.
5. Active plan and role/result contracts.
6. Current implementation, tests, and evidence.
7. General engineering defaults.

Instructions live at the narrowest scope that completely governs them:
* **`crates/` (`crates/AGENTS.md`)**: Governs 29 Rust workspace crates, inward dependency layering, mechanical boundary enforcement (`just boundaries`), shared adapter contract suites, and Rust build discipline.
* **`workbench/` (`workbench/AGENTS.md`)**: Governs the terminal-native operator workbench (Bun + TypeScript + OpenTUI), TUI verification gates (`just tui-*`), and router-not-authority gateway invariants.
* **`.agents/` (`.agents/AGENTS.md`)**: Governs durable operational memory, directory roles (specs vs. plans vs. reports), handoff contracts, and research preregistrations.

Never silently resolve a conflict by changing the ruler, acceptance criteria, security policy, or architecture. Surface contradictions, hidden assumptions, unnecessary scope, and unresolved decisions.

## 2. Investigation and Retrieval

Discover before asking; ask before inventing. Settle mechanically answerable questions with tools before spending model reasoning.

Use the cheapest tool that can settle the question:

* `graft map` for initial repository orientation.
* `graft ask "<question>" --source` for ranked architectural/behavioral context with source spans.
* `graft callers <symbol>` (`--direction out`, `--depth N`) for call graph and blast radius.
* `graft skeleton <file>` for signatures/spans without whole-file reads.
* `graft grep "<literal>"` for exhaustive indexed literal matches.
* `zvec_grep_search` or `zg` for semantic/conceptual discovery when wording or location is unknown.
* `rg --files` for inventory and `rg` for known paths, symbols, literals, config keys, errors, or regexes.
* `rust-analyzer` for Rust structural/semantic questions before grep-and-recompile loops.
* `$understand-chat`, `$understand-explain`, or `$understand-diff` only when relationships/blast radius require broad reading; verify against source/tests.

Use Graft/zvec to narrow, then verify anchors with `rg` and read only relevant ranges. If Graft truncates a span, open that exact range before finalizing.

Do not invent product behavior, acceptance criteria, comparison standards, public API semantics, security policy, or domain decisions. Record unresolved uncertainty as fog rather than guessing.

## 3. Delegation and Role Boundaries

Role boundaries are architectural:

* **Orchestrator:** owns discovery, decomposition, contracts, acceptance criteria, verification architecture, integration, evidence review, settlement, and promotion. Does not write implementation code while delegation exists. Without delegation, stop blocked unless `GAUNTLET_ALLOW_ORCHESTRATOR_IMPLEMENTATION=1`.
* **Builder:** performs bounded transformations; may not alter frozen references, comparison criteria, acceptance gates, immutable evidence, or prior settlements. Builder completion claims carry no settlement authority.
* **Critic:** read-only, fresh-context comparison against the frozen reference under an explicit contract. Receives no builder reasoning/history/attempt count/identity. Emits residuals; never fixes.
* **Verifier:** decides whether a declared claim meets its evidence requirement; prefer instruments over judgment.
* **Process optimizer:** proposals only.

Delegation is admission-based. `rlm(...)`-style calls return handles, not answers; never await them as result values. Every delegation declares a result-contract path, and completion requires a schema-valid result file there. Mailbox replies are signals, not contracts. Never delegate whether the total system settled.

## 4. Failure-Derived Operating Rules

Permanent rules derived from demonstrated project failures:

* Delegated slices checkpoint/report to files as they work; critics write reports before messaging. Prefer `explore` agents for read-only work.
* At every dispatch, correction round, and interruption recovery, update both task-local `IN-FLIGHT.md` and `.agents/CURRENT_STATUS.yml`; neither substitutes for the other.
* After interruption, reconcile worktree + reports + status before redispatching.
* Every `flock` invocation is its own top-level command; never nest a second `flock` inside another lock's `-c`.
* After clearing an apparently wedged agent, message it and wait for observable progress before dispatching a replacement.
* If multiple agents touched the tree since the last full verification, the orchestrator runs the full battery before settlement.
* Never `git restore` / `git checkout --` a tracked task path without checking status and recording its SHA-256 first; uncommitted task work may be the only copy.
* Timing/race-sensitive behavior requires deterministic boundary variants for each branch; repeated green races prove only the branch that happened to win.

## 5. Architecture and Governance Invariants

* Builder and evaluator never share control of the ruler. Freeze, digest, and re-verify references (`just ruler`).
* Formal comparison uses a fresh context whose envelope is runtime-built from an allowlist.
* Authority decides whether an operation may run; containment decides how constrained it is if it runs. Both precede builder process start; neither substitutes for the other.
* `irreversible` units are never automatically dispatched.
* An agent may never reduce a Unit's blast radius or authority classification.
* Model output is untrusted external input: parse, schema-validate, then admit into consequential state.
* Prefer deterministic verification wherever deterministic evidence exists.
* Select the next affordable, governable, recoverable, settleable correction—not the largest change.
* Self-improvement is `current → candidate → isolated evaluation → held-out/regression comparison → promote or rollback`. The active system never moves its own ruler and then claims improvement.
* Bounded search supports bounded claims only. Do not convert "no issue found" into universal absence without exhausting a declared bounded space.
* Existence is not composition. Integrated capability claims require proof through the intended production/composition path.
* Stable concepts include `Reference`, `Answer Key`, `Unit`, `Observation`, `Comparison`, `Residual`, `Verification`, `Correction`, `Settlement`, `Capability`, and `Role Invocation`.

## 6. Universal Change Boundaries

Always:
* Make the smallest effective change that fully satisfies the requested outcome; no MVP-like, placeholder, partial, or knowingly incomplete work unless explicitly requested.
* Read every file before editing it and inspect one nearby pattern plus its tests.
* Preserve unrelated worktree changes and avoid unrelated formatting/refactoring.
* Keep work task/milestone-scoped.
* Use types to prevent invalid states rather than comments or agent discipline.
* Keep generated artifacts from becoming competing sources of truth.
* Record adjacent issues separately instead of widening scope.

Ask before adding/upgrading dependencies; changing persisted schemas, ID grammar, policy precedence, public interfaces, exit codes, CI, deployment, closure gates, or architecture boundaries; deleting files; or expanding the milestone. Prefer standard library and established dependencies over new packages.

## 7. Commands, Testing, and Verification

Use the repository `justfile` as the canonical command interface. Standard global recipes:
`just ruler`, `just check`, `just test`, `just lint`, `just deny`, `just boundaries`, `just ci`, `just verify`.

* Behavioral changes: identify observable correctness, add/identify focused tests first, confirm expected failure, implement the smallest coherent change, run the narrowest check, then broaden to closure gates.
* Meaningful vertical changes must exercise the real loop:
  `operator request → reference freeze → role dispatch → artifact change → observation → fresh-context comparison → verification → residual → correction → settlement → durable evidence`
* Do not disable failing tests, weaken assertions, update goldens merely because output changed, treat `INCONCLUSIVE` as `PASS`, or change the ruler to make implementation pass.
* Mutation and coverage are settlement/evidence operations, not routine edit-loop checks. Coverage proves reach, not correctness.
* A successful tool call is not settlement.

## 8. State, Settlement, and Completion

Never reconstruct critical run state from conversational memory.

Before declaring completion:
1. Reconcile worktree, reports, task ledgers, and `.agents/CURRENT_STATUS.yml`.
2. Inspect the final diff.
3. Run proof/verification proportional to the claim plus every required closure gate.
4. Confirm no test, ruler, authority gate, containment boundary, or evidence requirement was weakened/bypassed.
5. Record unresolved failure, debt, fog, or `INCONCLUSIVE` state explicitly in `.agents/`.
6. Update `.agents/CURRENT_STATUS.yml`.
7. Run `graft build` after code changes that affect indexing.
8. Leave the next move explicit if the run remains open.

`PASS`, `FAIL`, and `INCONCLUSIVE` are distinct. Missing evidence is not passing evidence.

Hard cap for this file: fewer than 150 lines and no more than 32 KiB.
