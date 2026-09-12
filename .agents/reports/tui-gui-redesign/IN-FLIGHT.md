# IN-FLIGHT — TUI GUI-grade redesign (operator-directed, post-plan)

Updated: 2026-09-11 (follow-on: mouse PTY qualification battery, e2f8bfe — third of the recorded non-blocking follow-ons).

## LATEST: e2f8bfe — mouse PTY qualification battery
- `operator-journeys-pty.test.ts` (gated `GAUNTLET_T20_REAL_PTY=1`) had zero mouse coverage — the settled mouse-convergence increment (618616a) was pinned only at the pure `handlePointer`/`handleKey` equivalence level, never through the real byte path.
- Two new gated tests: a real SGR click on the Primary tab strip converges on the same `▸[CHAT]` marker the keyboard `"2"` journey produces; a real SGR wheel gesture over the Navigator moves the selection the same way the down-arrow key does. Wire format verified directly against `@opentui/core`'s bundled SGR mouse parser (no "enable mouse" handshake needed from the test side — raw bytes are parsed unconditionally).
- Double-click sub-case dropped after finding a REAL bug: `terminal-renderer.ts`'s `resolveAt()` hit-tests in a fixed first-match region order (header → columns → bottom/attention/composer/help/drawer → palette LAST), but z-index governs PAINT order only — a real click on an open command-palette row is swallowed by whatever surface is drawn underneath it. **The palette is not clickable through the real pointer path today.** Not fixed here (terminal-renderer.ts change, out of scope for a test-only increment) — residual.
- Running the gated suite for real (previously rarely run, off by default) surfaced two stale pre-existing assertions, both corrected: the header workspace-reference literal changed to "canonical surface" in increment 2a (516f5cb) without updating this test; the blocked-count "!" glyph is conditional on `blocked > 0` (views-work.ts) while the test unconditionally expected it.
- A third discrepancy found and deliberately left unresolved: the Composer surface never gets the "▸ " focus-ring marker when Tab-cycled to it — `planFrame` (terminal-renderer.ts) explicitly excludes `"Composer"` from the column loop that applies the marker. Undetermined whether this is a REQ-UX-005 visible-focus gap or intentional (Composer's focus may be conveyed some other way) — **residual, needs its own investigation**, left as one known, gated, documented failure (4/5 real pass) rather than guessed at.
- Gates: tsc clean; 917 tests / 24 gated skips / 0 fail (was 917/22/0); biome 0 new findings.
- Implemented by a delegated Sonnet subagent (wire-format investigation + both new tests debugged to a real pass); the two stale-assertion fixes and the Composer-marker triage were done by the orchestrator after independently re-running the gated suite past the subagent's reported state and finding it still red.

## Prior: 5483fe6 — RealityTrace (SXR) host binding
- `sxr-adapter.ts` (adapter: `executeSxrRefresh`, `bunSxrProcessRunner`, `projectSxrEvidenceLens`, `renderSxrEvidenceSection`) and `reality-trace-lane.ts` (pure lane projector, landed f0523f8) were both fully built but zero-wired to the live host — `snapshot.sxr_lens` was never populated in production.
- New env-driven `config.ts` field `sxrBinding` (`GAUNTLET_SXR_BINARY` + `GAUNTLET_SXR_REPO_ID`, both required or unbound — unbound is a no-op, unchanged current behavior; `SxrLensProjection`'s status vocabulary has no "unbound" case, so none was invented).
- New pure module `reality-trace-binding.ts`: maps `SxrEvidenceLens` onto `reality-trace-lane.ts`'s narrow structural input (unwrapping branded `SxrOrdinal`/`SxrEntryHash` at this boundary, since that module never imports the adapter), and `interpretSxrRefreshOutcome` (only `refreshed`+`ledger_export`+`parsed` produces data; every other outcome/parse shape returns `undefined`/`undefined`, host keeps the prior lens/lane and marks it stale).
- New `SemanticCommandClass` value `RUNTIME_ADAPTER` (filling in the ICM §5 "runtime" slot the taxonomy comment already documented but nothing implemented) and command `trace.realitytrace.refresh` (bound to `r`, Trace-view-scoped, command-triggered only — ICM §16.11 "never during render").
- `WorkbenchHost.refreshRealityTrace()`: epoch-guarded bounded refresh (`sxrRefreshEpoch` increments per attempt; a result is discarded if a newer refresh started before this one resolved — ICM §16.11's "a refresh result that completes after a newer one is discarded").
- Trace view: new source-typed RealityTrace segment appended after Gauntlet history at all three `renderTraceView` return points, staleness-labeled via `laneStale`, never merged with Gauntlet rows (UESC §16.0). Inspector: `main.ts` now threads `host.sxrLens` through `publishLensData` onto the snapshot, wiring the pre-existing (previously dead) `renderSxrEvidenceSection` consumer.
- Gates: tsc clean; 917 tests / 22 gated skips / 0 fail (was 906); biome 0 new findings across all 10 changed/new files.
- Implemented by a delegated Sonnet subagent from a bounded, file/line-anchored brief; every diff independently reviewed and all gates independently re-run by the orchestrator before commit. The orchestrator's review caught a real gap — the subagent's first pass tested the pure mapping function but not `WorkbenchHost.refreshRealityTrace()` itself — and sent the agent back for host-level tests (fake-binary-backed: unbound no-op, single success, epoch-discard race, close-before-resolve) before committing.
- Explicit residuals (not in scope): TraceStream follow/unseen-count integration for RealityTrace rows; no "unbound" visible source status; no automatic polling.

## Prior: c1c7bca — BUSY_SUBMISSION host wiring
- `busySubmission()` (action-control.ts) existed but was unused since the ActionControl increment landed — nothing ever minted it.
- `WorkbenchHost.invoke()` now tracks `${operation}:${focus.id}` in a `pendingOperationKeys` set: added before `this.gateway.invoke(...)`, removed in `finally` (covers accepted/refused/outcome_unknown/throw alike). `state.refresh`'s local-read short-circuit is untouched — never marked busy.
- Threaded read-only as `ShellRenderOptions.pendingOperations` → `ViewRenderArgs`/`InspectorArgs` → `main.ts`'s `session.render(...)`.
- Work bar + Inspector AFFORDANCES substitute `busySubmission(operation)` for the resolver-derived control AFTER derivation (never descriptor-derived — action-control.ts's invariant held), rendering on the existing `readiness.degraded` token with a `"… "` glyph and no reasons/next-step text (not a domain-success or blocked claim).
- Gates: tsc clean; 906 tests / 22 gated skips / 0 fail (was 848); biome 6-error pre-existing baseline, 0 new findings in the 7 changed files.
- Implemented by a delegated Sonnet subagent from a bounded, file/line-anchored brief; independently re-verified (diff read, gates re-run) by the orchestrator before commit.

## Prior FINAL STATE (queue-close push): c624f60 + f0523f8
- §30 activation gate LANDED with the T23 coherent update (shellDrive synthesizes pre-activation refusals from the fresh descriptor's canonical reason vocabulary; racing teeth preserved; staging fixtures now activatable).
- ActionControl object (action-control.ts) renders the Work bar + Inspector AFFORDANCES (byte-identical text, all DSN pins green).
- V2 closed: Compact drawers (n/i, Esc closes, clicks route through the same select edge) — Inspector reachable at 80–119 cols for the first time.
- Bottom surface renders (available-candidates tabs, REQ-AFF-004) and collapses FIRST at short heights (heightBand consumed; three columns survive).
- Back/forward ([/]) over the pure navigation-history module; session commits every location change (key + pointer).
- Navigator viewport follow (V4) — plus a latent window-index off-by-start bug fixed.
- Host notice carries refusal reason codes (V5 partial).
- RealityTrace lane projection module ready for host binding.
- 19 after-state captures incl. drawers, bottom, live back/forward, affordance query.
- Gates: tsc clean; 922 tests / 22 gated skips / 0 fail; biome 6-error pre-existing baseline; preflight PASS.

## What this task is

Radically revise the Gauntlet TUI into a GUI-grade, direct-manipulation, trace-native workbench rendered through the terminal, per `DESIGN.md` + `.agents/specs/tui/INTERACTION_COMMAND_MAP.md` (ICM) + `.agents/specs/tui/UI_ELEMENT_STATE_CATALOG.md` (UESC). Keyboard path preserved; mouse/menus/tabs/overlays converge on one semantic command path. Commit after every meaningful change. Governing artifacts always win over Hallmark proposals; DESIGN.md + spec are sha256-frozen (C9).

## Settled (chronological; see CURRENT_STATUS.yml for the full list with hashes)

1. `1d4d46e` ICM/UESC: Trace = one source-typed ledger (Gauntlet history + RealityTrace).
2. `16e90a9` Phase 1: recon + contract conflicts C1–C10 + characterization.
3. `fedbad1` Hallmark redesign design doc (03-hallmark-redesign.md).
4. `6e84eff` Increment 1: semantic command layer (registry/resolver/handleCommand).
5. `516f5cb` 2a: full UESC §23 tokens, DESIGN surface/fg tokens, capability detection (V6), motion wiring (M15).
6. `f984245` 2b: wire honesty W1–W4 + client.ts mutateUnit twin-fix.
7. `1740dd6` 2c: retained render tree + hitMap + reuse proof.
8. `b4bbd27` 7-core: V1 fixed (three columns below 40 rows at ≥120 cols) + heightBand/collapseOrder.
9. `5ad863a` 6-core: TraceStream pure machine (46 tests).
10. `ed32694` 4/5/6 integration: trace ledger live (follow/hold/unseen/return-to-live), palette query filter, bottom/inspector-pin commands, V3 header clip-priority, primary tab strip.
11. `618616a` 3: mouse convergence — useMouse:true, PointerGesture → handlePointer (same commands/effects as keys; equivalence pinned), renderer gesture synthesis via generic onMouse (named onScroll does NOT dispatch scroll in OpenTUI 0.5.11).
12. `4d12b73` 5a: state.refresh §30 host-side honesty + V3 main.ts header tail integration.
13. `7b8fe60` 8-evidence: 12 after-state tmux captures vs the real `gauntlet serve` (baseline/after/) + five agent reports + lint baseline repair.

Gates at settle: tsc clean; **848 tests / 22 gated skips / 0 fail** (baseline 697); biome at the 6-error pre-existing baseline; `just tui-preflight bootstrap` PASS.

## How the parallel run worked (reusable pattern)

Five agents partitioned BY FILE OWNERSHIP (theme/config/main; gateway+stub+affordance; renderer; geometry; trace-stream) while the orchestrator worked the hot shared files (shell.ts, command-registry.ts) serially. No conflicts; agents ran targeted suites only; orchestrator ran full gates + committed per increment. Reports under `.agents/reports/tui-gui-redesign/agents/`.

## Key decisions standing

- Command identity is the convergence point (`handleCommand`); keys AND pointer are representations. No second pipeline exists — pinned by equivalence tests.
- T23 frozen matrix semantics: the shell forwards post-render-mutated affordances so the GATEWAY denies (racing teeth). The palette-row §30 gate lands with ActionControl as one coherent change INCLUDING the T23 expectation update — not piecemeal.
- OpenTUI quirks pinned: scroll only dispatches via generic `onMouse`; useMouse must be explicitly enabled; `capabilities.rgb` for color detection.
- biome useLiteralKeys vs tsc TS4111 conflict on Record-cast types: resolve via declared-property shapes (see client.ts readSnapshot comment).

## Next (non-blocking follow-ons, priority order)

1. ~~ActionControl BUSY_SUBMISSION host wiring~~ — DONE (c1c7bca).
2. ~~RealityTrace production binding (refresh command, epoch staleness, Trace/Inspector exposure)~~ — DONE (5483fe6). Residual not covered: TraceStream unseen-count integration for RealityTrace rows; no visible "unbound" source status; no automatic polling (refresh is command-triggered only, by design — ICM §16.11).
3. Chrome remainder — CORRECTION (2026-09-11, orchestrator investigation): the "right-click gesture plumbing already exists" claim in this note was STALE/WRONG. `PointerGesture` (shell.ts) has only `click`/`double_click`/`wheel` — no `right_click`/`hover`/`drag` kind exists. `TerminalShellRenderer.rowMouseEvent` (terminal-renderer.ts) explicitly drops non-left buttons (`if (event.button !== 0) return; // left button only for now`). Each remaining chrome item needs real new primitives, not just wiring:
   - **breadcrumb-click hoisting**: blocked on more than hit-testing — `WorkbenchModel.getSnapshot().horizon_breadcrumb` is `readonly string[]` (display LABELS only, e.g. `["Case", "u-root"]`), carrying no FocusRef per segment. Hoisting to an arbitrary breadcrumb segment needs (a) the model to expose the ancestor FocusRef chain (`this.horizonStack[i].root_focus`, already tracked internally, just not on the snapshot) alongside labels — label-based click-to-hoist would be exactly the "label match ≠ binding" violation this campaign forbids elsewhere (RealityTrace bindings); (b) a new "hoist to depth/ref" primitive (`hoist`/`dehoist` today only push/pop ONE level); (c) the header line isn't part of the hit-map's `surfaces` at all today, so it needs to become hit-testable, with x-offset → breadcrumb-segment resolution (span-level, not line-level — today's hit map is line-granular).
   - **hover tooltips / context menus**: need new `PointerGesture` kinds (`hover`, `right_click`) plus new terminal-renderer.ts mouse-event handling (`rowMouseEvent` currently only synthesizes click/double_click/wheel).
   - **pane resize gesture**: needs drag semantics (mousedown + move + mouseup), also absent from `PointerGesture`.
   - **menu bar**: net-new rendered surface + new hit-testable regions.
   Each is its own real increment; none is safely "already half-done." Estimate: 4 separate increments, roughly BUSY_SUBMISSION-to-RealityTrace-binding sized each.
4. ~~Mouse PTY qualification battery~~ — DONE (e2f8bfe). Real click/wheel coverage now passes through the actual byte-level path. Three residuals it surfaced, none fixed here: (a) the command palette is not clickable through the real pointer path at all (`resolveAt`'s hit-test order lets background surfaces swallow clicks meant for the palette overlay); (b) the Composer surface never shows a `▸ ` focus-ring marker when Tab-focused (`planFrame` explicitly excludes it) — one gated real-PTY test is left intentionally red (4/5) documenting this until it's investigated; (c) two now-fixed stale assertions in this same suite are evidence the gated suite drifts silently because it's off by default — consider running it periodically, not only when directly relevant.
5. Final regression: operator-journey sweeps + equivalence matrix + captures refresh.

## Standing notes

- Branch `task26-dcritic-apparatus-repair`. Dev backend: `just tui-dev-up` (builds + serves at 127.0.0.1:4000; captures scripts under baseline/).
- Never stage `.jolli/`, `.agents/CURRENT_STATUS*`, `workbench/.gauntlet/`.
- Two gateways still exist (host + PrimeDelegationDispatcher) — recorded; fix with Prime wiring work.
