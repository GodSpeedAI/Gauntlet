# Increment 5 core: the ActionControl presentation object (ICM §30)

Agent slice: NEW `workbench/src/action-control.ts` + NEW
`workbench/src/action-control.test.ts`; edits to `workbench/src/views-work.ts`,
`workbench/src/inspector.ts` (and zero edits to `work-chat-trace.test.ts` — no
pin needed updating).
Frozen contract: `INTERACTION_COMMAND_MAP.md` §30 (ActionControl state
contract), `UI_ELEMENT_STATE_CATALOG.md` §22.9, `03-hallmark-redesign.md` §5.
Branch `task26-dcritic-apparatus-repair`; NOT committed (per slice contract).

## Changes

### NEW `workbench/src/action-control.ts`

1. **`ActionControlState = "HIDDEN" | "VISIBLE_DISABLED" | "VISIBLE_ENABLED"
   | "BUSY_SUBMISSION"`** and **`interface ActionControl { operation; state;
   reasons: readonly string[]; nextStep?; attentionItem? }`** (readonly,
   frozen; optional facts attach only when defined — the affordance.ts
   exactOptionalPropertyTypes idiom).
2. **`actionControlFor(descriptor, outcome): ActionControl`** — pure, total,
   never BUSY:
   - `outcome === "palette_only"` → **HIDDEN** with reasons (HIDDEN takes
     precedence over status, so no palette-only resolution can leak an
     enabled-looking bar row; the palette keeps discovering it — DSN-009
     unchanged);
   - `statusPermitsInvocation(status)` (available/requires_attention) →
     **VISIBLE_ENABLED**, `attentionItem` carried when present;
   - otherwise (degraded/unavailable) → **VISIBLE_DISABLED** with the
     resolver's reason codes verbatim and the grammar's `nextStep`.
   Enablement vocabulary is the resolver's own (`statusPermitsInvocation`) —
   no second predicate (DSN-005).
3. **`busySubmission(operation): ActionControl`** — the ONLY source of
   BUSY_SUBMISSION. Documented in-module: BUSY is submission lifecycle, never
   affordance status; the host mints it while an invoke is in flight and must
   not read it as domain success. It carries no reasons/nextStep/attentionItem.
4. **Moved from inspector.ts (with re-export there — API stable):
   `reasonNextStep`** — the canonical reason→next-step grammar. Rationale: the
   ActionControl carries `nextStep`, so the control object and its grammar
   belong at one source; the move also keeps the module graph acyclic
   (inspector imports action-control; action-control imports only
   affordance.ts).
5. **Moved from views-work.ts (with type re-export there — API stable):
   `MaterializationOutcome` + `PALETTE_ONLY_REASONS` + NEW
   `materializationOutcomeFor(descriptor)`** — the DSN-009 WHERE policy as one
   pure classifier (`.some` any-of semantics preserved exactly). Rationale:
   the §30 HIDDEN state is defined by this outcome, and inspector + views-work
   + palette must derive identical outcomes (REQ-AFF-002). Dependency graph is
   acyclic: views-work → action-control → affordance; inspector →
   action-control (views-work's old reasonNextStep import from inspector is
   gone, so no cycle).

`src/index.ts` was NOT edited (outside ownership): the package barrel surface
is preserved structurally — `reasonNextStep` still flows through
`export * from "./inspector"` (re-export) and `MaterializationOutcome` through
`export * from "./views-work"` (type re-export). Direct `action-control`
exports are not yet on the barrel; wiring them is a one-line orchestrator
choice.

### `workbench/src/views-work.ts`

- The **ACTIONS bar renders through `actionControlFor`**: HIDDEN rows are
  skipped (exactly the old `palette_only` skip); VISIBLE_ENABLED rows keep the
  current glyph/label language verbatim (`✓ OP ready`, `! OP responds to
  <attentionItem>`); VISIBLE_DISABLED rows keep `× OP BLOCKED: reasons ·
  next: …` byte-identical; the palette-only footer ("N more in command
  palette (op: reason…)") is untouched.
- Bar `records` gain **`control_state`** (field addition; `operation`,
  `status`, `reasons`, `next_step`, `materialization`, `target` preserved).
- `materializeAffordances` now delegates WHERE-classification to
  `materializationOutcomeFor` (identical outputs, single source); its
  docstring credits the moved policy.

### `workbench/src/inspector.ts`

- The AFFORDANCES section records gain **`control_state`** derived through
  `actionControlFor(descriptor, materializationOutcomeFor(descriptor))` — the
  SAME state the bar derives (pinned by a convergence test). Existing fields
  preserved; line rendering untouched (palette-only rows still inspectable
  with reasons + next step, now also recording HIDDEN — DSN-009).
- `reasonNextStep` definition removed; re-exported from action-control so
  inspector's public surface (and the barrel) is unchanged.

### `workbench/src/work-chat-trace.test.ts`

**Zero edits.** All DSN-005/DSN-009 pins pass against the rewritten bar
unchanged — that is the preservation proof: the bar text, skip behavior, and
palette discoverability are byte-identical through the new derivation path.

## Invariants pinned (NEW `action-control.test.ts`, 18 tests / 164 expect())

RED-first evidence: the file was written before the module; initial run failed
at module load (`Cannot find module './action-control'`). After the factory
landed but before bar/inspector wiring, exactly the 3 integration tests were
RED (re-export missing; `control_state` missing) — then GREEN.

- Status×outcome mapping **total and correct**: the full 4×3 grid over the
  AffordanceStatus × MaterializationOutcome vocabularies maps to exactly the
  contract state; no pair is unmapped; none is BUSY; HIDDEN precedence pinned.
- **BUSY only via the explicit constructor**: the grid plus a reasons-variant
  sweep (12 combos × 3 reason shapes) can never mint BUSY; `busySubmission`
  pins the one BUSY shape (empty reasons; absent nextStep/attentionItem keys;
  frozen).
- Real-resolver derivations (not just synthetic descriptors):
  requires_attention+materialized → VISIBLE_ENABLED + attentionItem ESC-9;
  no_open_ticket → VISIBLE_DISABLED + grammar next step; degraded read →
  VISIBLE_DISABLED + `observation_degraded`; state.refresh → HIDDEN +
  `not_action_admitted`; every `materializeAffordances` outcome equals
  `materializationOutcomeFor` on the same descriptor.
- Outcome policy: the three palette-only reasons pinned; any-of semantics
  pinned; blocked otherwise; `reasonNextStep` vocabulary pinned and identical
  through the inspector re-export.
- Bar: exact `× UNIT.RESOLVE BLOCKED: no_open_ticket · next: …` /
  `✓ UNIT.RETRY ready` / `! UNIT.RESOLVE responds to ESC-9` text; no
  `work.affordance.state.refresh` line; palette footer intact; record
  `control_state` equals a fresh factory-over-fresh-resolution derivation and
  is never HIDDEN in bar records.
- **Gold/attention token discipline unchanged** (DSN-003): every token on the
  control-derived bar rows is in {readiness.ready, attention.required,
  readiness.blocked}; none is in `goldReservedTokens()`.
- Inspector: all `operationIds()` records carry `control_state` (palette-only
  → HIDDEN), existing fields preserved; **cross-surface convergence**: Work
  bar records and Inspector records derive the SAME state per operation.

## Evidence

- RED: `bun test src/action-control.test.ts` → module-load failure (0 pass);
  second RED stage: 15 pass / 3 fail (integration tests) before wiring.
- GREEN (named verify; `src/views-work.test.ts` does not exist in the tree —
  the views-work-pinning suites are `work-chat-trace.test.ts` +
  `work-rhythm.test.ts`, both run):
  - `bun test src/action-control.test.ts src/work-chat-trace.test.ts` →
    **37 pass / 0 fail** (18 + 19).
  - views-work/inspector + structural battery: `work-rhythm`,
    `inspector-lens-wiring`, `headless-replay`, `renderer-boundary`,
    `cross-surface-equivalence`, `palette-grouping-confirm`, `shell-gui`,
    `index`, `operator-journeys`, `affordance-resolver`, `shell-trace`,
    `keyboard-accessibility` → **150 pass / 0 fail / 2040 expect() across 14
    files** (includes the two named).
  - `renderer-boundary.test.ts` green: action-control.ts imports only
    `affordance.ts`, so it joins the policed state-layer universe cleanly
    (presentation/state classification + gateway whole-tree scan).
  - `cross-surface-equivalence.test.ts` green: the minimal-projection vs
    materializeAffordances equivalence holds over the moved policy.
- `bun run check` (tsc --noEmit): **clean** (exit 0).
- `bun x biome check --write` on the five owned/touched files: safe fixes
  applied; final `biome check` exit **0** — remaining diagnostics are infos of
  the documented `useLiteralKeys` class (biome vs tsc
  `noPropertyAccessFromIndexSignature`, the standing repo pattern; 16 in the
  new test file following `work-chat-trace.test.ts`'s existing bracket-access
  convention, 4 pre-existing in inspector.ts).
- Full suite intentionally not run (slice discipline); no commit.
- `graft build` not run: workbench TS is not part of this repo's graft index
  scope per prior increments; orchestrator runs full closure gates at settle.

## Integration notes for the orchestrator (NOT done here — outside ownership)

1. **shell.ts §30 palette gate**: shell.ts's `ui.palette.activate` already
   refuses non-permitting fresh resolutions via `statusPermitsInvocation`
   (shell.ts:1217-1227, comment already anticipates ActionControl). To finish
   the §30 language, it can derive rows through `actionControlFor` and key the
   refusal on `control.state !== "VISIBLE_ENABLED"` — semantics identical
   today; the T23 racing teeth stay with the gateway either way.
2. **Host BUSY_SUBMISSION wiring**: `busySubmission(operation)` is provided
   for main.ts/workbench-host.ts to hold while an invoke is in flight and
   drop on authoritative projection update or denial (ICM §30 lifecycle;
   UESC toast row is the 03-report §3 rendering home).
3. **Barrel**: optionally add `export * from "./action-control"` to
   src/index.ts (not in this slice's ownership; no current importer needs it).
4. The worktree also carries the orchestrator's concurrent uncommitted changes
   (shell.ts, terminal-renderer.ts, command-registry, workbench-host, …);
   untouched and preserved.

## State

Worktree left uncommitted. Files created/changed by this slice exactly:
`workbench/src/action-control.ts` (NEW), `workbench/src/action-control.test.ts`
(NEW), `workbench/src/views-work.ts`, `workbench/src/inspector.ts`.
`work-chat-trace.test.ts` required no edits. Open move: the orchestrator's §30
activation gate + T23 expectation update in the same coherent change.
