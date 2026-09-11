# IN-FLIGHT — TUI GUI-grade redesign (operator-directed, post-plan)

Updated: 2026-09-11 (follow-on: BUSY_SUBMISSION host wiring, c1c7bca — queue was already closed at 11 commits; this is the first of the recorded non-blocking follow-ons).

## LATEST: c1c7bca — BUSY_SUBMISSION host wiring
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
2. RealityTrace production binding: refresh lane off the render loop, refresh-epoch staleness, pending/unverified lanes, Evidence/Trace exposure. (Lane projection module landed in f0523f8; host binding still open.)
3. Chrome remainder: menu bar, breadcrumb-click hoisting, hover tooltips, context menus (right-click gesture plumbing already exists), pane resize gesture.
4. Mouse PTY qualification battery.
5. Final regression: operator-journey sweeps + equivalence matrix + captures refresh.

## Standing notes

- Branch `task26-dcritic-apparatus-repair`. Dev backend: `just tui-dev-up` (builds + serves at 127.0.0.1:4000; captures scripts under baseline/).
- Never stage `.jolli/`, `.agents/CURRENT_STATUS*`, `workbench/.gauntlet/`.
- Two gateways still exist (host + PrimeDelegationDispatcher) — recorded; fix with Prime wiring work.
