# T12 Evidence — OpenTUI Semantic Shell and Case-Centric Projections

Date: 2026-09-09
Task: T12
Proof Level: P2 (confirmation: peer)
Scope: The terminal-native semantic shell over immutable Workbench snapshots — Environment → Workspace → Surface → View → Focus vocabulary, Navigator/Primary/Inspector/Composer surfaces, the three required primary projections (Work/Chat/Trace), provider-gated lower views, Inspector record objects, the Composer with temporary takeovers, keyboard routing, geometry floors, and the theme token adapter — proving that an operator can navigate a Case, see multiple placements of the same Focus, hoist/de-hoist, switch Work/Chat/Trace questions over the same history, inspect relations/evidence/affordances, and complete temporary composer takeovers without presentation owning domain state.
Preregistration: frozen at `.agents/research/preregistrations/task-gauntlet-tui-prime-workbench-T12.prereg.yaml` — verified byte-identical this session (sha256 `6d0425a8226d3f6fbbe3627b305622c84971ba519a1b6ac441c88e0afdd9e14c` via `just prereg-check`), never modified.

## Settled Requirements

Every REQ and DSN this task settles, cited to its test. Test files: `opentui-shell.test.ts` (12 tests), `composer-takeover.test.ts` (10), `work-chat-trace.test.ts` (19), `operator-journeys.test.ts` (10), `keyboard-accessibility.test.ts` (7) — 58 new tests, all deterministic in-memory over a real `WorkbenchModel` (no terminal, no I/O).

### Design clauses (DSN)

| Clause | Requirement anchor | Evidence (test file → describe) |
|---|---|---|
| DSN-001 raw colour literals only in the theme adapter | REQ-TUI-001 | `work-chat-trace.test.ts` → "DSN-001: no raw colour literals outside the theme adapter" (grep-style scan of `shell.ts`, `views-work.ts`, `views-chat-trace.ts`, `inspector.ts`, `composer.ts`, `render-tree.ts` for hex/rgb()/ANSI escapes + a sanity leg proving the scanner works against `theme.ts`) |
| DSN-002 glyph+label+emphasis survive colour degradation; truecolor/256/monochrome lowering | REQ-UX-005 | `work-chat-trace.test.ts` → "DSN-002" (all-token lowering matrix; `family\|glyph\|label\|emphasis` uniqueness in monochrome; Work rows distinguishable as plain text); `keyboard-accessibility.test.ts` → "no colour-only meaning" (every consequential span carries its glyph in the text; text is capability-independent) |
| DSN-003 gold reserved for proof/sanctioned authority | REQ-UX-005 | `work-chat-trace.test.ts` → "DSN-003" (`goldReservedTokens()` ≡ {`authority.sanctioned`, `proof.independent`}; focus/selection/ordinary-success tokens never gold) |
| DSN-004 distinct token families | REQ-UX-003 | `work-chat-trace.test.ts` → "DSN-004" (seven required families present; amber-coinciding tokens keep distinct family+label) |
| DSN-005 no component-local authority predicate | REQ-AFF-001/003 | `work-chat-trace.test.ts` → "DSN-005" (rendered affordance states deep-equal fresh `resolveAffordance` results; flipping ONLY the observation flips the rendered state `requires_attention` → `unavailable`/`authority_not_permitted` — no view-local pathway exists) |
| DSN-006 placements over copies | REQ-FOCUS-001/002 | `opentui-shell.test.ts` → "T12 falsifier" (prereg P1: Navigator + Work + Agents-view alias placement + Inspector all resolve ONE interned `FocusRef`; after ONE `unit_updated` every placement shows the new state and no placement keeps a stale copy; agent-context placement renders as an `≈` alias row, not a second object) |
| DSN-007 geometry floors | REQ-UX-005 | `opentui-shell.test.ts` → "T12 shell geometry floors" (120x40 → Navigator+Primary+Inspector with `calculateColumns`-exact widths, Inspector asserted present across the whole 120+ band; 80x24 → exactly [Primary, Composer] at width 80 with Case/attention retained in the header; 70x18 → recoverable prompt naming current+required sizes with exit `q`; shrink→grow restores the semantic location and emits zero effects — nothing cancelled) |
| DSN-008 evidence/authority as inspectable record objects | REQ-UX-003 | `opentui-shell.test.ts` → "T12 Inspector sections" (evidence records carry identity/claim/producer/result/reference/provenance; VERIFIED and UNVERIFIED both rendered — nothing erased; authority record fields the projection does not expose say `unavailable`; `renderEvidenceRecord`/`renderAuthorityRecord` in `inspector.ts`) |
| DSN-009 blocked-with-reason vs absent-but-palette-discoverable | REQ-AFF-005 | `work-chat-trace.test.ts` → "DSN-009" (`unit.resolve` on a visible unit → `blocked_with_reason` with gate + next step; `state.refresh` → `palette_only`, absent from the action bar, present in palette entries WITH reason; escalation observation upgrades the same affordance to `requires_attention` naming its ticket); `operator-journeys.test.ts` → REQ-UX-003 journey |
| DSN-010 Chat subordinate to durable execution history | REQ-DATA-005 | `work-chat-trace.test.ts` → "DSN-010" (after `setChatEnabled(false)`, Trace lines AND records are deep-equal to the pre-removal render; Chat renders an honest disabled state and never impersonates history) |
| DSN-013 optional motion + reduced-motion mode | REQ-UX-005 | `opentui-shell.test.ts` → "T12 reduced motion mode" (`runningIndicatorFrames("reduced")` empty; `containsMotionFrames` false under reduced motion; help names the mode; renders are deterministic) |
| DSN-015 no roadmap surface | roadmap_boundaries | `opentui-shell.test.ts` → "T12 view registry and DSN-015 roadmap tooth" (registry has no `harness` key; `registerShellView(registry, "harness", …)` throws `RoadmapSurfaceError`; `assertNoRoadmapSurfaces` guard); `work-chat-trace.test.ts` → "DSN-015" (availability table + registry key scan) |

### Requirements (REQ)

| Requirement | Evidence |
|---|---|
| REQ-UX-001 cold operator, escalation-first, named next steps, no startup work | `operator-journeys.test.ts` → REQ-UX-001 (cold workspace renders bind/run next steps; header names workspace/reference/run; escalation surfaces in Navigator + Work + attention summary without stealing focus; rendering emits no effects) |
| REQ-UX-002 selection identity across views; hoist breadcrumb + one-step return; Case-wide attention | `operator-journeys.test.ts` → REQ-UX-002 (select in Navigator; identity retained across chat/trace/work; `h` hoists → breadcrumb `Case › u-root`, `u` de-hoists one step with identities unchanged); `opentui-shell.test.ts` → scroll/focus preservation |
| REQ-UX-003 blocked-unit inspection before resolve/retry | `operator-journeys.test.ts` → REQ-UX-003 (exact target ref, status, authority, `no_open_ticket` + `readiness_not_earned` reasons, first-gate next step; with an escalation observed the same inspector names `ESC-42` as the attention item the action responds to); `opentui-shell.test.ts` → Inspector sections |
| REQ-UX-004 composer mode/destination always visible; takeovers queue by stable identity; expired cannot accept input; escape dismisses | `composer-takeover.test.ts` (10 tests: draft restoration, canonical replacement, queue ordering + dedupe by request identity, expiry refusal + inspectability, dismissal back to queue, always-visible mode line, paste-never-executes) |
| REQ-UX-005 keyboard-only at every geometry; scroll anchoring; nothing steals focus | `operator-journeys.test.ts` → REQ-UX-005 (the full journey at 120x40 AND 80x24 — Navigator selection at Full, palette focus entries at Compact); `keyboard-accessibility.test.ts` (visible focus, help legend, glyph-carrying spans, stream updates leave scroll/focus untouched, escape hatches); `opentui-shell.test.ts` → geometry floors + per-view scroll offsets |
| REQ-UX-006 honest empty/unavailable/degraded states with recovery; core inspection without Prime/DomainForge/SXR | `operator-journeys.test.ts` → REQ-UX-006 (declared-unavailable capability renders with its gate reason while inspection stays usable); `work-chat-trace.test.ts` → empty-Case and unreachable-observation next steps |
| REQ-UX-007 read-only inspection vertical slice; fields not exposed say unavailable | `operator-journeys.test.ts` → REQ-UX-007 (unit status → verification metadata → evidence references → return, mutation-free; `unavailable` for unexposed authority fields); `opentui-shell.test.ts` → honest sections for kinds without projected rows |
| REQ-TUI-001 OpenTUI consumes snapshots + callbacks; no business state in presentation | Structural + tests: `shell.ts`/views import model TYPES only (never a runtime model — asserted by the DSN-001 source scan discipline and by the falsifier/scroll tests where snapshots flow in and only `ShellEffect`s flow out); `testing/shell-harness.ts` wires effects to a real model |
| REQ-TUI-002 Environment → Workspace → Surface → View → Focus vocabulary; orthogonal Cue/Attention/Relation/Affordance | `shell.ts` type vocabulary (`SurfaceId`/`ViewId`/`WorkspaceRef`/`SemanticLocation`/`formatLocation`); attention/relation/affordance projections render orthogonally in Navigator/Work/Inspector/attention view; `operator-journeys.test.ts` → REQ-UX-002 |
| REQ-TUI-003 distinct Work/Chat/Trace questions over the same Case | `work-chat-trace.test.ts` → "three questions over the same history" |
| REQ-TUI-004 temporary takeover preserving underlying composer state | `composer-takeover.test.ts` → "session-level takeover owns the interaction" + restoration/canonical-replacement tests (prereg P2) |
| REQ-DATA-005 Chat/Trace separate projections, neither canonical input for the other | `work-chat-trace.test.ts` → DSN-010 test |
| REQ-GOAL-001 Case/Focus/relations/evidence/attention/next-actions locally coherent, not authoritative | `operator-journeys.test.ts` (attention summary, Inspector relations/evidence/affordances with provenance; every consequential move is an effect) |
| REQ-GOAL-005 OpenTUI a replaceable projection over React-free state | The render tree (`render-tree.ts`) is a renderer-neutral structural description; `shell.ts` consumes snapshots and emits intents only — replacement lowering cannot touch domain state |
| REQ-OUT-001 Case-centric navigation, multiple placements, hoistable Horizons, primary Work/Chat/Trace, Inspector, Composer, contextual lower surfaces where providers exist | `opentui-shell.test.ts` → registry provider gating (evidence/agents/attention available; files/kernel/problems/terminal/domain ABSENT with computed provider reasons) + falsifier + journeys |

## Implementation Details

All files are under `workbench/src/` (my parallel-wave ownership). No file outside `theme.ts`/`geometry.ts`-adjacent shell files was modified; `workbench-model.ts`, `focus.ts`, `horizon.ts`, `relations.ts`, `affordance.ts`, `operation-*.ts`, `projection-store.ts`, `index.ts`, `preview.ts`, `client.ts`, `config.ts`, `preflight.ts`, `prime-*.ts` are untouched.

1. **`render-tree.ts`** (NEW): the renderer-neutral render tree — `RenderedSpan` (carries a semantic token NAME, never a colour), `RenderedLine`, `RenderedRecord` (DSN-008 record objects), `ViewContent`, plus `lineText`/`renderToText`/`linesUseToken` helpers. Render functions produce this tree; a later OpenTUI/React adapter lowers it (DSN-011 direction).
2. **`theme.ts`** (EXTENDED, existing exports intact): the ONE place raw colour values may live (DSN-001). `SEMANTIC_TOKENS` — 35 tokens across the authority/execution/evidence/settlement/readiness/integrity/capability/attention/focus/structure families with DESIGN.md default colours, recommended glyphs, uppercase labels, emphasis; `resolveToken(token, capability)` lowering to truecolor / maintained 256-anchor indexes / colourless monochrome (DSN-002); `goldReservedTokens()` enforcing the DSN-003 discipline set; `runningIndicatorFrames(motion)` + `STATIC_RUNNING_MARKER` for DSN-013.
3. **`shell.ts`** (NEW): the semantic vocabulary (`SurfaceId`, `ViewId` with NO harness member, `WorkspaceRef`, `SemanticLocation`, `formatLocation`), the view registry (`registerShellView` refusing roadmap ids at runtime, `assertNoRoadmapSurfaces`, `availableViews` computing provider-gated availability from the snapshot's own published collections), `navigatorOutline` (Case → units → evidence → sessions → Navigator alias placements → Case-wide attention, every row keyed by a `FocusRef`), the pure state machine (`createShellState`, `handleKey` → `{state, effects}` with view switching, surface cycling, scroll-anchored per-view offsets, palette incl. Compact-mode focus entries, `h`/`u` hoist/one-step-return, below-minimum prompt ownership), `renderShell` (geometry-floor-exact surface assembly; Inspector sections via `renderInspector`; header retaining Case/health/attention; palette; prompt), and `ShellSession` (presentation state + composer + effect dispatch; an active takeover owns the keyboard; resize touches ONLY dimensions).
4. **`views-work.ts`** (NEW): the Work projection — escalation-first ordering, honest `budget: unavailable` summary (the projection carries no budget/tier), unavailable-capability lines with gate reasons, and `materializeAffordances` implementing DSN-009's Progressive Affordance Materialization policy ON TOP of T09's `resolveAffordance` (the resolver decides existence/enablement — DSN-005; the policy only decides WHERE a resolved descriptor materializes: bar / blocked-with-reason / palette-only).
5. **`views-chat-trace.ts`** (NEW): Chat and Trace as separate projections over the same snapshot (DSN-010), plus the provider-backed lower views (Evidence record objects, Agents sessions + `≈` alias placements, Attention summary) and `defaultViewRegistry()`.
6. **`inspector.ts`** (NEW): Inspector sections from the selected Focus — Identity (the one ref), State (projection-honest; `unavailable` for kinds without rows), Relations (`deriveRelations` over the snapshot with provenance), Authority record, Evidence records, Provenance (generation/source/cursor), Affordances (every operation with status/reasons/next-step/attention item) — plus `reasonNextStep`, the disclosed presentation mapping over T09's canonical refusal vocabulary.
7. **`composer.ts`** (NEW): the composer state machine — `beginTakeover` (holds the default state; queues by stable request identity with dedupe; refuses expired/answered identities), `completeTakeover` (restores the held state unless canonical replacement is explicit; promotes the next queued request), `dismissTakeover` (escape: UI dismissed, request returns to the queue), `expireTakeovers` (stale active/queued requests become inspectable and can never accept input), `composeKeypress` (typing never executes; `enter` PRODUCES a submission for the host's canonical path), `renderComposer` (mode+destination always visible; takeover banner/input/hints; queued and expired lines).
8. **`testing/shell-harness.ts`** (NEW): deterministic fixtures — `seededModel()` (3 units incl. one blocked, verified+unverified evidence, 2 sessions), `coldModel()`, `observationFor()` (honest canonical observation builder), `OPERATOR_SESSION`, `recordingInteractions()` (effect journal), `modelWiredSession()` (effects applied to a real model — the composition path).
9. **`workbench-model.ts`**: ZERO changes. The task permitted additive changes, but none was required — placements, attention, capability reasons, hoist/de-hoist and snapshot fan-out from T07/T08/T09 already provided everything the shell consumes.

**Needed `index.ts` exports** (barrel is read-only this wave; orchestrator to append): `export * from "./render-tree"; export * from "./shell"; export * from "./views-work"; export * from "./views-chat-trace"; export * from "./inspector"; export * from "./composer";`

## Teeth

Every tooth declared in the plan's T12 block, with its outcome:

1. **120x40 render** → full three-surface layout present; the test additionally walks the whole 120–140 column band asserting the Inspector never drops, and pins widths to `calculateColumns`. PASS (opentui-shell "FULL three-surface layout").
2. **Shrink below 80x24 and grow back** → recoverable prompt naming both sizes with exit `q`; re-entry restores the semantic location exactly (`toEqual` on the location) with ZERO effects emitted (nothing cancelled, nothing re-admitted). PASS (opentui-shell "shrink below minimum and grow back").
3. **Force monochrome and read every consequential state** → every token lowers colourless with glyph+label+emphasis preserved and unique; Work/Attention/Trace states render distinguishable as plain text (`BLOCKED`, `ACTIVE`, `UNIT_UPDATED`, `×`, `●`, `!`); every consequential span carries its glyph in the text. PASS (work-chat-trace DSN-002 ×3; keyboard-accessibility "no colour-only meaning" ×2).
4. **Grep presentation packages for raw colour literals** → scanner over all six shell source files: hex, `rgb(`, escaped and literal ANSI escapes all absent; sanity leg proves the scanner fires on `theme.ts`. PASS (work-chat-trace DSN-001 ×2).
5. **Look for a Harness View / Skill / HarnessRevision renderer / refinement UI** → absent from the registry, the availability table, and the `ViewId` union (the id is not expressible); runtime registration attempt throws `RoadmapSurfaceError`. PASS (opentui-shell + work-chat-trace DSN-015).
6. **Open the same Focus in Navigator, Work, Agent placement, Inspector; update source once** → all four placements reference the one interned `FocusRef`; after one event every placement shows the new state and NO line anywhere still shows the old one; the alias placement stays an alias (`≈ executed-by`). PASS (opentui-shell falsifier — prereg P1 held).
7. **Kernel-input takeover while the default composer holds text** → takeover owns the interaction (even `tab` cannot steal focus), submits to the host's canonical path, then the draft/mode/destination return byte-identical unless canonical state explicitly replaces them; queue/expiry/dismissal variants hold at both the reducer and session level. PASS (composer-takeover ×10 — prereg P2 held).

Additional discriminating checks written as tests: affordance rendering deep-equals fresh resolver output and flips only when the observation flips (DSN-005); Chat removal leaves Trace deep-identical at the view layer (DSN-010); new stream events leave scroll/focus/view untouched (REQ-UX-005).

## Gate Results

All exit 0, this session, in order:

```text
just prereg-check gauntlet-tui-prime-workbench-T12
  → PASS — matches frozen hash 6d0425a8226d3f6fbbe3627b305622c84971ba519a1b6ac441c88e0afdd9e14c

just tui-test opentui-shell          → exit 0 — 12 tests, 124 expect() calls
just tui-test composer-takeover      → exit 0 — 10 tests,  67 expect() calls
just tui-test work-chat-trace        → exit 0 — 19 tests, 718 expect() calls
just tui-test operator-journeys      → exit 0 — 10 tests,  65 expect() calls
just tui-test keyboard-accessibility → exit 0 —  7 tests,  55 expect() calls

just tui-check                       → exit 0 (tsc --noEmit, strict)
just tui-lint                        → exit 0 (biome check . — 0 errors; 10 pre-existing
                                        noConsole warnings in T03 bench files; 228 infos are
                                        the documented useLiteralKeys↔noPropertyAccessFrom-
                                        IndexSignature conflict recorded in T09's recovery)
just tui-preflight bootstrap         → exit 0 — PASS (skeleton, pinned deps, lockfile sync,
                                        bun audit clean, spec + DESIGN.md hashes match plan)

cd workbench && bun test             → exit 0 — 179 tests / 26 files / 2368 expect() calls,
                                        0 fail (98 pre-existing + 58 T12 + 23 concurrent T10)
```

No existing test was modified or weakened; the full suite passes with all T12 files present. The tree also carries the concurrent T10 builder's `prime-*` files (untouched by this task); `bun test` is green with both waves in the tree.

## Known Limitations

- **Real-PTY journey evidence is T20's.** These suites assert the state machine and rendered structures, not pixels or a rendered terminal; T00A's geometry/keyboard precedent is followed, and the plan's T12 early slice "responsive layout and real-PTY journey acceptance" completes in T20's real-target qualification.
- **Embedded terminal is T13** (DSN-014 escape-proofing likewise): the Terminal lower view is computed absent with reason `provider 'terminal' is not published by the current snapshot`.
- **Plugin slots are T15**: no plugin registration path exists yet; the registry/DSN-015 guard is the seam T15 will extend.
- Kernel / Problems / Files(Changes) / Domain views are absent with computed provider reasons until T13/T16/T14/T15/T23 bind providers; Domain additionally awaits a bound DomainForge/SEA model.
- The affordance resolver's canonical observation is HOST-supplied (`ShellRenderOptions.observation`) per REQ-TUI-001; a palette invoked with no observation lists views only — honest absence, never a fabricated availability.
- Navigator breadcrumb labels come from T07's `horizon_breadcrumb` (source identity; human labels only when the owning source minted one) — `Case › u-root`, not `Case › Root Unit`.
- At 80x24 there is no Navigator surface (the spec floor mandates one primary pane); keyboard selection runs through palette focus entries, which the Compact palette appends.
- Work's budget/tier aggregates render as explicitly `unavailable` because the T07 projection carries no such fields (REQ-UX-007 honesty); they will populate when the host binds those reads.
- Presentation is React-free model types with a render-tree description; the OpenTUI/React lowering adapter itself (REQ-TUI-005's replaceable renderer seam) is future work beyond this task's bounded claim.

## Correction Round 1 (peer findings)

Peer verdict: **CONFIRM** with one MAJOR (closed) and two MINOR trigger-records. No assertion was weakened and no frozen artifact was touched during this round.

### MAJOR (closed, RED-first): dismissed-then-re-admitted takeover requests stayed in the queue

- **Finding:** `beginTakeover`'s final admission branch (`workbench/src/composer.ts`, pre-fix lines 157-163) admitted a request without removing any stale queue entry carrying the same stable identity. A dismissed takeover returns to `queued` by design (REQ-UX-004 escape); re-admitting it left BOTH an active takeover and its queued ghost. Peer repro: `begin(a)` → `dismiss(a)` → `begin(a)` → `begin(b)` → `complete(a)` promoted the ghost `a` back to active (with empty `takeoverInput`), demanding a second answer — contradicting the module header invariant "a request identity is admitted at most once" (`composer.ts:11-13`).
- **Fix:** the admission branch now purges the admitted identity from `queued` (`queued.filter((queued) => queued.requestId !== request.requestId)`), and the function's contract comment records the purge so the invariant and the code cannot drift apart again. `dismissTakeover`/`expireTakeovers`/`completeTakeover` semantics are unchanged.
- **RED-first evidence:** regression test `composer-takeover.test.ts` → "REGRESSION (peer MAJOR): re-admitting a dismissed request removes its stale queue entry — it cannot be answered twice" reproduces the peer's exact ordering and failed BEFORE the fix for exactly the reported reason:
  ```text
  error: expect(received).not.toContain(expected)
  Expected to not contain: "req-a"
  Received: [ "req-a" ]
  (fail) … REGRESSION (peer MAJOR): re-admitting a dismissed request removes its stale
  queue entry — it cannot be answered twice
  1 fail / 10 pass, exit 1
  ```
  After the fix the same test is GREEN and the chain ends with each identity answered once and the draft restored.
- **Post-fix gates (verbatim):**
  ```text
  just tui-test composer-takeover → exit 0 — 11 pass / 0 fail / 78 expect() calls
  cd workbench && bun test        → exit 0 — 180 tests / 26 files / 2379 expect() calls, 0 fail
  just tui-check                  → exit 0
  just tui-lint                   → exit 0 — 0 errors; 10 warnings (pre-existing T03 bench
                                    noConsole), 228 infos (documented useLiteralKeys conflict)
  ```
  (One transient `tui-lint` failure during the round was a format-only diff in the edited `composer.ts`, fixed via `biome check --write src/composer.ts` before the final gate run; no behavioural change.)

### MINOR trigger-record 1 (for T20): DSN-004 latent label collision in the unused token vocabulary

`authority.escalated` and `settlement.escalated` share `#C58A2A` + `!` + `ESCALATED`, and `execution.failed` / `evidence.failed` / `integrity.failed` share `#9E2F2F` + `×` + `FAILED` (theme.ts token table, peer-cited ranges 124-130/166-172/196-207/285-291). The DSN-002 monochrome-uniqueness test passes because the identity key includes the token FAMILY, but glyph+label alone collide. None of these tokens renders in a T12 surface today; T20 must revisit the labels (e.g. `EXECUTION FAILED` / `EVIDENCE FAILED`) when they first render, so the distinction never rests on colour or family metadata alone.

### MINOR trigger-record 2: `RenderedRecord.kind` taxonomy stretch

`views-chat-trace.ts` records chat messages as `kind: "attention"` and trace records as `kind: "relation"` (peer-cited lines 101-110/145-156) because the `RenderedRecord` union (evidence/authority/affordance/relation/attention) lacks honest members for communication and event rows. No consumer keys on those two kinds today; when the record registry next changes, add honest members (e.g. `"message"`, `"event"`) instead of reusing the nearest existing kind.

### Integration note

The `index.ts` barrel exports requested by the builder (`./render-tree`, `./shell`, `./views-work`, `./views-chat-trace`, `./inspector`, `./composer`) were applied by the ORCHESTRATOR at integration, together with the concurrent T10 `prime-*` module exports; the builder did not edit `index.ts`.

Confirmation: BUILDER evidence pending peer confirmation.
