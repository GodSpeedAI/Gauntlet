# Adversarial cross-artifact inspection — spec + plan + DESIGN.md

Date: 2026-09-08
Scope: `.agents/specs/gauntlet-tui-prime-workbench.spec.yaml`, `.agents/plans/gauntlet-tui-prime-workbench.plan.yaml`, `DESIGN.md`, read in full against the repository they will be implemented in.
Disposition: **13 findings; all corrected in-artifact.** The three documents now compose. No runtime, integration, or performance claim is made — T00 and every implementation gate remain unexecuted.

This report supplements and does not replace `review.md`, which recorded the 0.2.0 approval and whose document hashes are correct **for 0.2.0**. It also does not supersede `document-critic.md`; D1–D3 were verified as genuinely closed in the 0.2.0 text before this pass began.

## What was already sound

Verified mechanically, not assumed:

- Plan pinned the exact 0.2.0 spec hash.
- 104 spec `REQ-` IDs defined; 104 mapped; zero orphans in either direction; no mapping points at a nonexistent task.
- `dependency_graph.edges`, every task's `depends_on`, and `initial_state.blocked` agree exactly across all 28 tasks. Acyclic, single root (`T00`), and every `ready_waves` entry has its dependencies satisfied by a prior wave.
- Every confirmation mode the spec's `high_risk_requirement_groups` demand is met or exceeded by the owning task.
- `just prereg-check <task>` resolves the plan's preregistration filenames correctly.
- The plan's claim that the JSON surface is a "seven-route" adapter is accurate (`http.rs:256`).

The defects were at the seams between documents, not inside the plan's graph.

## Findings

### F1 — DESIGN.md was unbound (critical)

`DESIGN.md` declared itself normative authority, mandated an agent reading order, and carried ~40 unnumbered MUST clauses. Neither the spec's `sources` block nor any plan task, hash pin, gate, or traceability entry referenced it. It would have been read and then silently ignored, and any drift between it and the implementation would have been undetectable.

**Fixed:** bound as `sources.design_contract` in the spec and `source.design` in the plan with its own SHA-256; added to `T00`'s hash-binding step and its teeth; added a `Design conformance index` (`DSN-001`…`DSN-015`) to `DESIGN.md`; added `verification.design_clause_owners` to the plan mapping every DSN clause to an owning task; `T25` now recomputes DSN coverage and re-verifies the bound hash.

### F2 — Geometry conflict: DESIGN.md contradicted `REQ-UX-005` (critical, implementable)

`REQ-UX-005` requires the **full layout at 120x40**, one primary pane at **80x24**, and a **recoverable compact-size prompt below that**. `DESIGN.md` put the three-surface layout at `>= 140 columns`, made 120 a two-pane "Standard" class, and specified a working single-surface TUI below 80 columns where the spec requires a prompt. `T12`'s gate tests exactly 120x40 and 80x24, so a builder following the design contract would have shipped a layout that fails the spec's own teeth.

**Fixed:** `DESIGN.md` width classes rewritten to Wide (`>=140`) / **Full (`120–139`, `>=40 rows`, normative floor)** / Compact (`80–119`) / Below-minimum prompt, with the class boundaries marked as spec-fixed and not design-tunable; height collapse order made explicit; a concrete prompt shape and its recoverability (resize re-enters without losing the semantic location; admitted work is never cancelled) specified. `DSN-007` indexes it and `T12` gained teeth for both ends.

### F3 — DESIGN.md specified a Harness View the plan forbids (material)

`DESIGN.md` gave the Harness View a section, a View-grammar row, a Primary-Surface slot, a refinement lifecycle UI, and `Skill`/`HarnessRevision` Focus kinds. The spec places harness metabolization UI in `roadmap_boundaries`, and the plan's `guardrails` name "harness-metabolization UI" as a roadmap item not to expand.

**Fixed:** demoted to a new *Roadmap-only design surfaces* section (design preserved, construction forbidden) alongside rich semantic overlays and alternate renderers; removed from the Focus-kind examples and the View table; `DSN-015` makes "no roadmap surface was built" a checkable clause with `T12` teeth and `T25` ownership.

### F4 — Primary-vs-lower View scope was ambiguous (material)

`DESIGN.md` listed ten Views as Primary-Surface defaults. `REQ-OUT-001`/`REQ-TUI-003` require exactly three primary projections (Work/Chat/Trace) and name Evidence/Agents/Kernel/Terminal/Problems/Changes as *lower* surfaces. `T12`'s scope differed by seven Views depending on which document a builder read.

**Fixed:** `DESIGN.md` now carries a scope table (primary-required / lower-promotable / provider-gated / roadmap) with each row's spec basis, plus the `REQ-AFF-004` rule that a promotable View is absent — not an empty tab — when its provider is absent. `T12` gained a matching step.

### F5 — Stale Jupyter framing re-imported a falsified assumption (material)

The spec's own `protocol_boundary` establishes that Prime's kernel is a JSON-lines Python REPL and explicitly *not* Jupyter sockets — yet the spec's section key was `prime_and_jupyter_integration`, the plan's acceptance key was `prime_jupyter_and_long_running_execution`, and `DESIGN.md` referred to "Prime/Jupyter" input requests and "Jupyter/IPython outputs" three times. `document-critic.md` recorded this class of error as already corrected; the vocabulary survived it.

**Fixed:** keys renamed to `prime_and_kernel_integration` / `prime_kernel_and_long_running_execution` (`REQ-PRIME-*` IDs unchanged); all three `DESIGN.md` references rewritten; the Kernel section now states the actual protocol and the serialization rule.

### F6 — `SemanticLocation` and `FocusPlacement` were unowned (material)

Both are design-normative in `DESIGN.md` and required in substance by `REQ-UX-002` (back/forward, deep links) and `REQ-FOCUS-002` (multiple placements), but neither appeared in the spec's `core_domain_model.entities` nor in any task's settled scope. Two stable workbench objects had no ownership rules.

**Fixed:** both added to the spec domain model with required fields and rules — notably "placement identity MUST NOT be substituted for Focus identity" and "resolving a SemanticLocation MUST NOT create or imply canonical state".

### F7 — `gauntlet://case/C17` minted a Case identifier (material)

The design's URI scheme made a short Case id the addressing primitive. The spec binds a Case to *workspace identity + canonical reference identity/digest + selected RunId*, and `identifiers_and_normalization.primary_id_rule` forbids minting a replacement identity where a canonical one exists.

**Fixed:** `CaseRef` typed and the example locations rewritten to carry workspace + `ref` digest + `run`, with the no-minting rule stated inline and in the new spec entity.

### F8 — The plan asserted a false property of `just verify` (critical)

`gate_binding_rule` said `just verify` is the closure gate "plus applicable feature-enabled surface tests." It is not. `verify` → `ci` → `cargo nextest run --workspace` with **default features**; `--features surfaces` appears nowhere in `justfile` or `scripts/`. The entire workbench read path (`gauntlet serve`, the seven-route surface, its CLI integration tests) is behind that feature, so the repository closure gate never observes the surface `T00A`/`T12`/`T20` depend on.

**Fixed:** the clause now records the observation and the correction; `T00` owns closing the hole before `T00A` dispatches, and must prove it by breaking a surfaces-gated test and watching the gate go red.

### F9 — `T00` bound four recipes where the rule requires six (material)

`gate_binding_rule` requires `tui-bench` and `tui-build` to be bound at `T00`, but `T00`'s gate list and `done_when` covered only check/test/lint/preflight — so the NOT_IMPLEMENTED discipline had no task enforcing it.

**Fixed:** `T00`'s step, `done_when`, and teeth now cover all six, including an explicit attack on an unimplemented bench/build mode.

### F10 — `T00A` admits dependencies before the ledger that governs them exists (material ordering)

`T03` owns the dependency-admission ledger and `REQ-BUN-001`/`002`, but `T03` **depends on** `T00A`. `T00A` therefore necessarily admits the first runtime dependencies (Bun, OpenTUI, the TS toolchain, a React reconciler) before any admission gate exists — contradicting the plan's own operating-contract rule.

**Fixed:** made the carve-out explicit and bounded rather than accidental. `T00A` must list every bootstrap dependency with version and reason, must add nothing from the `REQ-BUN-001` overlap set (a teeth check enforces this), and must hand the list to `T03`; `T03` gained a step to qualify or falsify each one retroactively and a `done_when` requiring it.

### F11 — Operation-bridge authentication was in no task's `changes` (material)

`security_safety_trust_boundaries.local_transport` requires per-request actor/session binding, host/origin validation, and version negotiation before mutations. The existing surface has none of this — loopback-bind refusal is reachability control, not actor authentication. Only a section-path `clause_owners` entry implied ownership; no task's `changes` claimed the work, and it is a Rust change to the canonical boundary.

**Fixed:** named explicitly in `T01`'s `changes` and steps, with a teeth check for an unbound/client-asserted actor, and stated in the spec's `local_transport` clause as new work that must not be assumed to exist.

### F12 — Rust-heavy tasks declared only TypeScript inner-loop gates (medium)

`T01` and `T09` are predominantly Rust work in `gauntlet-app`/`gauntlet-cli`/the surface adapter, yet their narrow gates were `just tui-test …` + `just tui-check`. The Rust coverage arrived only via the global gate at the end of the task.

**Fixed:** package-scoped Rust recipes added to both gate lists, matching `gate_binding_rule`'s own "Rust changes use package-scoped recipes in the inner loop".

### F13 — Undefined confirmation vocabulary and stale references (minor, corrected)

`confirmation: operator_journey` appeared once (`T00A`) with no definition anywhere; `builder`/`peer` were likewise undefined; the plan uses `independent_adversarial` where the spec says `adversarial`. `T00` instructed the implementer to inspect "task26 report/status" — task26 is committed and task27 is the live workstream — and the spec's `substrate_baseline.gauntlet_head` is seven commits behind HEAD.

**Fixed:** `verification.confirmation_modes` now defines all six modes and maps `independent_adversarial` to the spec's `adversarial`; `T00`'s step references the concurrent workstream generically and requires re-observing the baseline commit rather than trusting the recorded one.

## Consequential gap left open, deliberately

`T00` must now **decide and record the workbench package root**. No artifact previously said where the Bun/TypeScript workbench lives in what is otherwise a Cargo workspace, nor how `boundaries`, `deny`, and `lint` reach it (they do not), nor what replaces them for TypeScript. `DESIGN.md` had assumed a repository-root `src/tui/`, which `AGENTS.md` *Project structure* forbids. This is a real decision with tree-wide consequences and it is not mine to make silently; it is now an explicit `T00` step and `done_when`, with `DESIGN.md`'s tree made relative to whatever `T00` records.

## Verification performed on the corrected artifacts

- Both YAML documents parse.
- Plan's spec pin equals the current spec SHA-256; plan's design pin equals the current `DESIGN.md` SHA-256; pinned spec version equals the spec's declared version.
- 104 `REQ-` IDs defined, 104 mapped, symmetric difference empty, declared counts consistent, no mapping to an unknown task.
- 15 `DSN-` clauses in `DESIGN.md`, 15 owned in the plan, symmetric difference empty, all owners are real tasks, declared counts consistent.
- Edges ≡ `depends_on` ≡ `initial_state.blocked` across all 28 tasks; graph acyclic; single root; every wave's dependencies satisfied by prior waves.
- Every `confirmation:` value used by a task is defined in `confirmation_modes`.
- Neither document retains the `jupyter` section key.

## Evidence limits

Document and repository-source inspection only. No build, no test execution, no Bun/OpenTUI/Prime/DomainForge/SXR runtime qualification, no performance measurement. The corrections remove document-level contradictions and unowned obligations; they do not establish that the substrate claims hold, and every `partially_proven`/`assumption` claim in the spec's `claim_discipline` remains exactly as unproven as it was. Approval of these edits is a design-revision decision, not settlement.

## Document hashes at inspection completion

```text
spec    .agents/specs/gauntlet-tui-prime-workbench.spec.yaml  d66203651461fd1189a8d10b6649b09473b423d8f2f501e3508034d52cb9f00b  (0.3.0)
design  DESIGN.md                                             315e2577bdfe9ef226b84f46aa6d7026ca4e6719fceebccb07862235722c2be1
plan    .agents/plans/gauntlet-tui-prime-workbench.plan.yaml   (recompute after this report lands; the plan pins the two above, not itself)
```

## Addendum — workbench package-root/toolchain decision settled

Date: 2026-09-08 (same day, follow-up)

The one open decision this report deferred to the user — where the Bun/TypeScript workbench lives and what enforces it — is now settled and written into the plan rather than left as a T00 discretion call:

- **Package root:** `workbench/`, a new top-level directory, sibling to `crates/`, never nested inside a Cargo crate. Verified reason, not preference: `crates/gauntlet-arch-tests/src/walk.rs`'s `core_tree` walks every file under the four core crates with no extension filter and treats a symlink, non-UTF-8, or oversized file as a hard violation — nesting `node_modules` under any core crate fails `just boundaries` on the first `bun install`. `REQ-TUI-005` additionally forbids naming the root after the current renderer.
- **Toolchain:** TypeScript strict mode (`tsc --noEmit`) + Biome (lint and format-check combined) + `bun test`, pinned exact-version devDependencies with a committed `bun.lock`. Biome was chosen over ESLint+Prettier on the same axes AGENTS.md's Dependencies section already asks: one dependency instead of two-plus-a-bridge, a single native binary, no separate Node toolchain to audit.
- **Gate parity:** each Rust global gate now has a named TypeScript counterpart (`tui-check`→`tsc`, `tui-test`→`bun test` including the T07/T19 boundary tests, `tui-lint`→`biome check`, `tui-preflight`→ T03's dependency ledger + `bun audit` + lockfile integrity) — recorded as `verification.workbench_package_root` / `workbench_toolchain` / `workbench_gate_composition` in the plan.

`T00` now scaffolds `workbench/` and its pinned toolchain directly (previously this was left to T00A's bootstrap carve-out); `T00A`'s carve-out is narrowed accordingly to the runtime rendering dependencies (OpenTUI, a React reconciler) it still admits ahead of T03's ledger. `DESIGN.md`'s React implementation structure now shows the concrete `workbench/src/` path and drops the stray `harness/` directory the earlier revision left in the `views/` scaffold despite demoting the Harness View to roadmap-only.

New teeth added to `T00`: a core-crate-nesting attack (proves the placement is load-bearing, not stylistic), a formatting-only-diff attack (proves `tui-lint` enforces format, not only lint rules), and a tampered/missing-lockfile attack (proves `tui-preflight` treats `bun.lock` the way `deny` treats `Cargo.lock`).

Verification re-run after these edits: both documents still parse; the plan's `source.spec.sha256` and `source.design.sha256` pins match the current file hashes; 104/104 `REQ-` IDs and 15/15 `DSN-` clauses still map with no orphans; the dependency graph is still consistent (edges ≡ `depends_on` ≡ `blocked`, acyclic); every `confirmation:` value used is still defined; ruler still PASS.

## Document hashes after this addendum

```text
spec    .agents/specs/gauntlet-tui-prime-workbench.spec.yaml  d66203651461fd1189a8d10b6649b09473b423d8f2f501e3508034d52cb9f00b  (0.3.0)
design  DESIGN.md                                             02ce58a257ec1240840338535cab218be77e0507abe22ea836dda77cad9944e3
plan    .agents/plans/gauntlet-tui-prime-workbench.plan.yaml   9f0e8b2b93117e26ba864d5009a695e88dfd6a21f1b9ebd58fe836731c1529fd
```

## Addendum 2 — Biome/TypeScript toolchain wired and configured (execution, not just decision)

Date: 2026-09-08 (same day, second follow-up)

The decision in Addendum 1 is now executed: `workbench/` exists with pinned exact-version Biome 2.5.12 + TypeScript 7.0.2 + `@types/bun`, a committed `bun.lock`, a configured `biome.json` (formatter + linter with `noExplicitAny`/`noUnusedVariables`/`noUnusedImports` as errors) and `tsconfig.json` (strict), and `scripts/tui-preflight.sh` operationalizing the T00 hash-pin teeth that were previously prose only. All six `just tui-*` recipes are wired in the `justfile` and behave per contract, including honest `NOT_IMPLEMENTED` (nonzero exit) for `tui-test all`, `tui-preflight conformance`, `tui-bench`, and `tui-build`. Full detail and command transcript: `.agents/reports/gauntlet-tui-prime-workbench/evidence/T00/workbench-toolchain.md`.

Verified: `tui-check`/`tui-lint`/`tui-test` pass on the real (placeholder) tree; deliberately introduced `any`, unused-variable, formatting, and type-error defects were each caught and the scratch files removed; a one-byte mutation of `DESIGN.md` was caught by `tui-preflight bootstrap` and the restore re-passed; `just -n verify` still resolves to the unchanged Rust closure gate with no `tui-*` step folded in; the full plan/spec/DESIGN.md consistency suite (hash pins, 104/104 REQs, 15/15 DSN clauses, dependency graph) still passes; `just ruler` still PASS.

This remains a slice of T00, not its completion — the surfaces-feature gate closure, Prime/OpenTUI version pinning, and DomainForge/SXR inventory steps are still open.

## Document hashes after this addendum

```text
spec    .agents/specs/gauntlet-tui-prime-workbench.spec.yaml  d66203651461fd1189a8d10b6649b09473b423d8f2f501e3508034d52cb9f00b  (0.3.0, unchanged)
design  DESIGN.md                                             02ce58a257ec1240840338535cab218be77e0507abe22ea836dda77cad9944e3  (unchanged)
plan    .agents/plans/gauntlet-tui-prime-workbench.plan.yaml   e19c8394d900b5ca5615706cd771fa6122a84fc991c03cf33938754dc695b7e3
```
