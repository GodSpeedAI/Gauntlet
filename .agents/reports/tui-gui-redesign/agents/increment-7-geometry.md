# Increment 7 core: responsive geometry honesty (V1 fix, V2 groundwork)

Agent slice: `workbench/src/geometry.ts` + NEW `workbench/src/geometry-responsive.test.ts`.
Frozen contract: `DESIGN.md` "Responsive Behavior" — Width classes (lines 1469-1526), Height constraints (1528-1539).
Violation repaired: recon `shell-render-opentui.md` §3 table + §8.3 V1.
Branch `task26-dcritic-apparatus-repair`; NOT committed (per slice contract).

## Changes

### `workbench/src/geometry.ts`

1. **`classifyGeometry` — dropped the `rows >= 40` gate on Full/Wide (V1).**
   New ladder, exactly per the slice contract and DESIGN:
   - `BelowMinimum`: cols < 80 OR rows < 24 (unchanged; DSN-007 recoverable prompt below).
   - `Wide`: cols >= 140 AND rows >= 24.
   - `Full`: cols in [120, 140) AND rows >= 24.
   - `Compact`: cols in [80, 120) AND rows >= 24.

   Rationale (DESIGN Full, line 1496): "Below 40 rows at this width, collapse the
   Bottom Surface first (see *Height constraints*), keeping all three columns."
   Columns are never dropped for height at >=120 cols; height degrades surfaces,
   not the column allocation. Docstring on the function cites this clause.
2. **NEW `export type HeightBand = "tall" | "short"`.**
3. **NEW `export function heightBand(dims): HeightBand`** — `rows >= 40` → `"tall"`,
   else `"short"` (24..39). Documented contract: consumers must gate on
   `classifyGeometry !== "BelowMinimum"` first; below 24 rows the recoverable
   prompt applies and no band is meaningful (DESIGN line 1539).
4. **NEW `export function collapseOrder(band): readonly string[]`** — the DESIGN
   height-constraint ladder (line 1530) as data the shell can consume and tests
   can pin: `["bottom", "metadata", "drawers"]` for `"short"`, `[]` for `"tall"`.
   Docstring names the always-preserved essentials (Focus, View, Horizon/location,
   attention cue, composer/required approval input, consequential status).

Constants untouched: `MIN_COLS=80`, `MIN_ROWS=24`, `FULL_MIN_COLS=120`,
`FULL_MIN_ROWS=40` (now doubles as the tall/short band boundary), `WIDE_MIN_COLS=140`.
`calculateColumns` needed no change — it keys off class, so Full/Wide at short
heights already allocate the three-column layout (verified and pinned by tests).
DSN-007 floors unchanged: full layout at 120x40, one primary pane at 80x24,
recoverable prompt below 80x24.

`src/index.ts` does `export * from "./geometry"`, so `heightBand`/`collapseOrder`
are re-exported automatically; no index edit needed.

### NEW `workbench/src/geometry-responsive.test.ts`

11 tests / 51 expect() calls, written test-first (RED observed before the fix —
see Evidence). Pins:
- 130x30 → Full/short, 3 columns (28/66/36); 139x39 → Full/short (28/75/36);
  200x39 → Wide/short (32/124/44, DESIGN Wide 1473-1485 shares Full's rule).
- Width bands keyed on columns alone above 24 rows: 119x30 Compact, 120x30 Full,
  139x30 Full, 140x30 Wide.
- 100x30 → Compact single pane (Compact degrades columns for width, not height).
- heightBand boundary: 39 → short, 40 → tall; 80x24 → short.
- collapseOrder("short") = ["bottom","metadata","drawers"], collapseOrder("tall") = [].
- Ladder never reclassifies geometry: Full/short + Wide/short keep class + columns.
- DSN-007 floors: 120x40 Full/tall; 80x24 Compact one pane; 72x20, 79x30,
  120x23, 140x23 BelowMinimum with the recoverable prompt text pinned.

## Test-expectation edits in existing files

**None required.** Sweep performed as directed:
- `rg "\b130\b|\b139\b|\b200\b"` over `workbench/src/*.test.ts`: only unrelated
  values (HTTP status 200 in `canonical-boundary.test.ts`, `ESC-200` unit ids in
  `kernel-affordances.test.ts`, timestamps, cycle counts) and the new file itself.
- Exhaustive dimension inventory across all workbench tests + `src/testing/`:
  `{120x40, 80x24, 100x30, 72x24, 72x20, 70x20, 70x18, 80x20, 60x18, 60x16, 60x14, 40x10}`
  plus embedded-terminal PTY sizes (100x30, 132x43 — PTY passthrough, not
  classification). Every one of these classifications is identical under the old
  and new rule, so no existing expectation was falsified by the fix.
- `opentui-shell.test.ts` geometry-floor suites pin 120x40 (Full), 80x24
  (Compact, ids `["Primary","Composer"]`), 70x18 (BelowMinimum) and the column
  loop `[120,125,130,135,140]` **at rows:40** — all DSN-007 pins, kept untouched
  as required (at rows 40, 130 → Full and 140 → Wide under both rules).
- `index.test.ts` (120x40 → Full) and `operator-preview.test.ts` (120x40 Full /
  80x24 Compact / 72x24+80x20 BelowMinimum): floor pins, unchanged, still green.

The falsified behavior existed in code and was observed only by the recon probe
(§7: `geo 130x30: Compact | geo 139x39: Compact | geo 200x39: Compact`); no test
had pinned it. This is expected for a violation that was never exercised.

## Evidence

- RED (before fix): `bun test src/geometry-responsive.test.ts` failed at module
  load (`Export named 'collapseOrder' not found`). Classification falsification
  demonstrated independently by evaluating the old rule (geometry.ts@HEAD logic):
  `old 130x30: Compact | old 139x39: Compact | old 200x39: Compact` — matching
  recon §7 exactly.
- GREEN (after fix):
  - `bun test src/geometry-responsive.test.ts` → 11 pass / 0 fail.
  - `bun test src/opentui-shell.test.ts src/keyboard-accessibility.test.ts src/operator-preview.test.ts src/index.test.ts` → 31 pass / 0 fail.
  - `bun test src/geometry-responsive.test.ts src/opentui-shell.test.ts src/keyboard-accessibility.test.ts src/cell-fit.test.ts src/preview-grid.test.ts` → 37 pass / 0 fail.
- `bun x biome check --write src/geometry.ts src/geometry-responsive.test.ts` → clean, no fixes.
- `bun run check` (tsc --noEmit): **fails only in untracked `src/theme-capability.test.ts`**
  (a parallel slice's RED test importing nonexistent `theme.ts` exports
  `SURFACE_TOKENS`/`TEXT_ROLE_TOKENS`). Zero errors reference geometry files;
  the file is foreign to this slice and was not touched. Note: the task's verify
  line names `src/geometry.test.ts`, which does not exist in the tree; the
  geometry-adjacent existing suites (`cell-fit.test.ts`, `operator-preview.test.ts`,
  `preview-grid.test.ts`) were run instead.
- Full suite intentionally not run (slice discipline).

## Integration notes for shell.ts (NOT done here — outside ownership)

To realize the DESIGN height ladder in rendering (and complete V1 end-to-end),
`shell.ts` should consume the new exports; today it only reads
`classifyGeometry`/`calculateColumns` (shell.ts:60, 614, 649, 673, 966-967, 1346),
so a Full/short terminal now renders the three-column layout with no
height-driven surface collapsing:

1. **Bottom-first collapse at short heights**: in the layout pass (near
   shell.ts:966-967 where `geo`/`columns` are computed), call
   `heightBand(state.dimensions)`; when `"short"`, suppress the Bottom Surface
   region first per `collapseOrder("short")[0] === "bottom"`. Today Bottom is a
   `SurfaceId` member that is never rendered (recon §4, shell.ts:69), so the
   immediate effect is nil — the ladder is groundwork that becomes binding once
   Bottom Surface rendering lands (V2/increment follow-ups).
2. **Deeper rungs**: if rows tighten further inside 24..39, consume the remaining
   `collapseOrder` entries ("metadata" → trim nonessential metadata lines;
   "drawers" → Navigator/Inspector become overlays rather than dropped columns,
   preserving the DESIGN always-preserve list: Focus, View, Horizon/location,
   attention cue, composer/required input, consequential status). The rung choice
   can key off `rows` bands below 40 (e.g. <32 rows → metadata rung) once the
   shell has metadata trimming; `collapseOrder` deliberately carries only the
   ordering, not thresholds, so thresholds stay a shell rendering decision.
3. **Compact overlay (V2 groundwork)**: Compact still renders no Navigator/
   Inspector (recon §8.3 V2 — switchable side surfaces/overlay missing). The
   "drawers" rung machinery (overlay treatment of Nav/Insp) should be shared
   between the short-height Full/Wide path and the Compact side-surface path.
4. **Preview parity**: `preview.ts` (ASCII journey renderer, line 152) switches
   on class; it now renders the full frame at >=120 cols x <40 rows. If the
   preview needs to mirror bottom-collapse, consume the same
   `heightBand`/`collapseOrder` pair rather than re-deriving.
5. `main.ts`/terminal-renderer need no change for classification; the composer's
   `min(lines, max(3, h-8))` budget at short heights should be re-derived against
   the preserved-essentials list once Bottom actually occupies rows.

## State

Worktree left uncommitted. Unrelated tracked changes (`.agents/CURRENT_STATUS.yml`,
`.jolli/*`, `workbench/.gauntlet/*`) and the foreign untracked
`workbench/src/theme-capability.test.ts` were preserved untouched. `.agents/CURRENT_STATUS.yml`
not updated by this slice (orchestrator-owned settlement state; slice contract
scoped the report to this file). Open move: shell.ts consumption of the ladder
(notes above) and the V2 Compact overlay work.
