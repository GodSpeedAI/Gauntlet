# Increment 4 remainder: back/forward navigation history (pure module)

Agent slice: NEW `workbench/src/navigation-history.ts` + NEW
`workbench/src/navigation-history.test.ts`. `shell.ts` untouched (the
orchestrator wires the module into the shell). NOT committed (per slice
contract). Branch `task26-dcritic-apparatus-repair`.

Frozen contract:
- ICM `.agents/specs/tui/INTERACTION_COMMAND_MAP.md` §6.6
  (`nav.history.back` / `nav.history.forward`): class NAVIGATION_LOCAL,
  binding LOCAL_DEFINED — restores Case/Horizon/View/Focus (+ optional
  layout state), "Does not revert domain time"; §2.2 (navigation changes
  perception, not domain truth); §49 teeth row "Back navigation | canonical
  world unchanged".
- DESIGN.md "Semantic location / route-like addressing" (lines 1152-1188): a
  semantic location "identifies a reproducible projection/horizon. It does
  not create canonical domain state."

## Changes

### NEW `workbench/src/navigation-history.ts` (zero imports, pure)

Types:
- `SemanticLocation` — narrow structural location type defined LOCALLY:
  `{workspace: unknown; horizon?: unknown; surface?: string; view?: string;
  focus?: unknown}`. Documented as a structural SUPERtype of the real
  `SemanticLocation` in `workbench/src/shell.ts` (workspace: `WorkspaceRef`
  → unknown; horizon/focus: `FocusRef | null` → unknown; surface/view:
  string unions → string) and of the DESIGN.md conceptual shape — real shell
  locations are admitted without importing shell.ts (renderer/shell-agnostic;
  wiring stays with the orchestrator).
- `LocationHistory = {entries: readonly SemanticLocation[]; cursor: number}`,
  frozen by construction (history object and entries array both
  `Object.freeze`d; caller location objects are stored BY REFERENCE and
  never frozen or rewritten).

API (exactly the frozen slice contract):
- `LOCATION_HISTORY_MAX_ENTRIES = 64` (exported bound).
- `createLocationHistory(initial)` — one entry, cursor 0.
- `locationPush(h, location)` — dedupes consecutive identical locations
  (returns `h` itself on duplicate), truncates any forward branch first
  (browser semantics), appends, bounds at 64 by dropping oldest, current
  always retained, cursor lands on the new entry.
- `locationBack(h)` / `locationForward(h)` — cursor ±1 if possible, else
  `h` unchanged; results SHARE `h.entries` by reference, so navigation
  provably never rewrites entries (perceptual reproduction, not mutation).
- `currentLocation(h)`, `canGoBack(h)`, `canGoForward(h)`.

Equality: structural (deep, own enumerable string keys, union of key sets so
extra fields such as ICM §6.6 "optional layout state" participate in dedupe);
`Object.is` fast path covers interned `FocusRef` identities; per-field
`null` ≡ absent so the shell's required-nullable shape (`focus: null`)
dedupes against DESIGN.md's optional shape (field omitted). Location values
assumed acyclic (the DESIGN location shape is flat strings/refs). JSDoc on
the module and every exported symbol cites ICM §6.6 (+ §2.2/§49) and the
DESIGN.md clause.

### NEW `workbench/src/navigation-history.test.ts`

12 tests / 647 expect() calls, written test-first. Pins:
- H1 entries never rewritten: `back`/`forward` results share the previous
  `entries` ARRAY REFERENCE; entries hold the exact caller references pushed.
- H2 push after back truncates the forward branch; forward at the new tip is
  a no-op; truncated locations unreachable.
- H3 consecutive duplicates (identical reference, deep-equal distinct
  objects, interned `FocusRef`, non-interned structural clone) each return
  the SAME history reference; non-consecutive repeats (A→B→A) are kept;
  single-field differences in workspace/view/focus/horizon/surface are NOT
  deduped.
- H4 bound: exactly 64 at the boundary (oldest kept), 65th push drops
  exactly the oldest, 71 visits keep last 64 (`v-7..v-70`), current always
  retained, bound interacts correctly with truncation after 63 backs.
- H5 cursor valid (integer, in range, non-null current) asserted after every
  step of a deterministic mixed push/back/forward sequence; frozen history +
  frozen entries at every step.
- H6 purity / ICM §49: stored entries are the pushed references; prior
  histories unchanged after later operations derive from them; caller
  locations never mutated or frozen; only frozen values returned.
- Structural compatibility: shell-shaped values (`ShellSemanticLocation`,
  type-only import) flow through the whole API, with the type-level
  admission `const narrowAdmission: SemanticLocation = shellA` verified by
  `bun run check`; null-vs-absent cross-shape dedupe pinned.
- Back-at-origin / forward-at-tip / duplicate pushes return the SAME history
  reference (no-op identity); back+forward round-trips to the same cursor,
  entries, and current location.

## Evidence

- RED-first: test file written and run BEFORE the module existed —
  `error: Cannot find module './navigation-history'` (0 pass / 1 fail).
- GREEN: `cd workbench && bun test src/navigation-history.test.ts` —
  **12 pass / 0 fail / 647 expect() calls**.
- `bun run check` (tsc --noEmit, strict incl. noUncheckedIndexedAccess +
  exactOptionalPropertyTypes): my two files type-clean. The run reports 2
  PRE-EXISTING errors in `src/shell.ts` (TS2739: `RenderedShell` missing
  `drawer`/`bottom`) from a CONCURRENT sibling slice's uncommitted shell.ts
  edit (+91/-19, line numbers drift between runs; `git diff` shows the
  sibling adding exactly those fields). My files reference neither symbol;
  not widened into — recorded here for the orchestrator.
- Biome: `biome check src/navigation-history.ts
  src/navigation-history.test.ts` clean (formatter + lint, after applying
  biome's safe import-sort/format fixes to my files only).
- Scope: untracked footprint is exactly the two new files above
  (`reality-trace-lane.*` in `src/` belongs to the sibling slice). Full
  suite intentionally NOT run; nothing committed, per slice contract.

## Residuals / next moves (orchestrator)

1. Wire shell.ts to this module: seed on case open, push on
   NAVIGATION_LOCAL location changes, bind `nav.history.back`/`.forward`
   (ICM §6.6) to `locationBack`/`locationForward`, and surface
   `canGoBack`/`canGoForward` for control enablement (ICM §30).
2. Decide whether `src/index.ts` should re-export navigation-history
   (skipped: index edits were outside this slice's owned files).
3. The concurrent shell.ts `RenderedShell.drawer/bottom` tsc errors block a
   clean package-wide `bun run check` until that slice lands.
