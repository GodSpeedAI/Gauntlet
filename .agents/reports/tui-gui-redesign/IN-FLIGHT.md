# IN-FLIGHT — TUI GUI-grade redesign (operator-directed, post-plan)

Updated: 2026-09-11 (ZCode continuation of the usage-limited Claude session 30e76b66).

## What this task is

Radically revise the Gauntlet TUI into a GUI-grade, direct-manipulation, trace-native workbench rendered through the terminal, per `DESIGN.md` + `.agents/specs/tui/INTERACTION_COMMAND_MAP.md` (ICM) + `.agents/specs/tui/UI_ELEMENT_STATE_CATALOG.md` (UESC). Keyboard path preserved; mouse/menus/tabs/overlays converge on one semantic command path. Commit after every meaningful change. Hallmark `redesign` skill used for presentation thinking only; governing artifacts win.

Full operator brief: recovered verbatim from the prior session (see `.agents/reports/tui-gui-redesign/` — the brief's requirements are folded into the reports and design doc).

## Done (chronological)

1. `1d4d46e` (prior session) — ICM/UESC revised: Trace = one source-typed ledger over Gauntlet execution history + RealityTrace.
2. `16e90a9` — Phase 1 complete: recon (4/5 areas; 5th lost to usage limit, artifacts in `baseline/`), contract conflicts C1–C8/C10 resolved (C9 standing: DESIGN.md + spec sha256-frozen, drafts editable), consolidated characterization `02-substrate-characterization.md`. Draft edits: Prime Python REPL wording (C2), harness roadmap-only (C7), PRODUCT.md mouse context (C8).
3. `fedbad1` — Hallmark redesign pass recorded: `03-hallmark-redesign.md` (shell anatomy, overlay layer, mouse gesture map, object presentation, token extensions, responsive ladder, 8-step implementation order) + `.hallmark/log.json`.
4. `6e84eff` — Increment 1: semantic command layer. `workbench/src/command-registry.ts` + `handleKey` = resolve + `handleCommand` single dispatcher; palette rows expose command id + shortcut; 11 new tests; all gates green (708/22/0, tsc clean, biome 6 pre-existing only, preflight PASS).

## Key decisions made

- **Command identity over keys** (ICM §4/§34/§35): `handleCommand(state, snapshot, commandId, paletteContext)` is THE convergence point; pointer gestures will call it with the same ids. Registry carries title/class/surfaces/defaultKeys; `validateCommandRegistry` is startup validation.
- **ICM §30 activation gate deferred to increment 5 (ActionControl)**: the frozen T23 cross-surface matrix pins shell-route forwarding of post-render-mutated affordances (gateway revalidation denies; racing-revocation teeth depend on it). The gate was implemented, RED-proven, then reconciled against T23 and deferred — one coherent change with `state.refresh` host short-circuit + reason grammar. See commit `6e84eff` message and `command-registry.test.ts` deferred-residual note.
- DESIGN.md + spec are FROZEN (C9): never edit to fit implementation; `just tui-preflight bootstrap` re-verifies hashes.

## Next (in order, from `03-hallmark-redesign.md` §8)

2. **GUI shell primitives** — retained-tree renderer (OpenTUI boxes/ScrollBox/zIndex; inventory in recon `shell-render-opentui.md` §9), surface/border theme tokens (`surface.*`, `focus.ring`, freshness/danger families — DESIGN.md depth levels), terminal color-capability detection (`renderer.capabilities`), operator-reachable reduced motion. W1–W3 wire fixes (nested `{error:{code,message}}` parse, JSON note body, `/state` gate-field honesty vs `http.rs`) belong here too.
3. **Mouse convergence** — `useMouse:true`, hit-map over stable element ids (navigator rows, `palette.${i}`, `work.affordance.${op}`, `trace.${id}`), gestures → `handleCommand` (§4 of design doc).
4. Chrome + tabs + bottom + pane resize (§2). 5. Overlays (§3) + ActionControl states (§5, incl. the deferred §30 gate). 6. Surfaces redesign (Navigator/Chat/Trace/Inspector/Work — §5). 7. Responsive ladder (§7, fixes V1/V2/V3). 8. Equivalence + adversarial verification (§8 list; capture evidence under `baseline/after/`).

## Verification discipline per increment

`just tui-check && just tui-test` (+ affected suites), biome on touched files (6 pre-existing errors elsewhere are the baseline), `just tui-preflight bootstrap`, ANSI capture for layout changes, explicit-pathspec commits.

## Standing notes

- Branch `task26-dcritic-apparatus-repair` (carrying the TUI work; do not switch without operator).
- Never stage `.jolli/`, `.agents/CURRENT_STATUS*` (operator-owned), `workbench/.gauntlet/`.
- Two gateways exist (host + PrimeDelegationDispatcher) — recorded recon note; fix when Prime wiring lands.
- `.hallmark/log.json` is project memory for Hallmark; DESIGN.md is the locked system (no theme rotation).
