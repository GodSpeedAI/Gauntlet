# Increment 2c — retained render tree + mouse hit-map foundation

Date: 2026-09-11. Scope: `workbench/src/terminal-renderer.ts` (retained lowering), `workbench/src/renderer-hitmap.test.ts` (new). No other production file touched (`main.ts` untouched; constructor signature preserved). Never committed (per task contract).

Implements `03-hallmark-redesign.md` §8 step 2 (GUI shell primitives, retained-tree renderer slice) and the foundation for step 3 (mouse convergence): recon `shell-render-opentui.md` §8.2 R1 identified the destroy-and-rebuild draw as the blocker for stable element identity.

## 1. Design of the retained structure

The pure span tree stays THE source (`renderShell` unchanged). Only the lowering became retained:

**Pure frame plan.** `planFrame(shell, width, height)` reproduces the old per-draw placement arithmetic exactly (header row 0; below-minimum prompt rows from row 2 with early return; columns x-advanced with the 1-cell internal gutter and `footerTop - 2` cap plus the focused-surface `▸ ` prefix; attention ≤3 rows at `footerTop`; composer ≤ `max(3, height-8)` rows; help ≤2 rows at the bottom; palette windowed `slice(start, start+available)` rows at x=2 over the bordered overlay box). Output: an ordered list of regions, each `{key, surfaceId, x, y, width, rows[]}` where row i paints at `(x, y+i)` with shared `cols`, plus a separate overlay-box placement. `planRows` keeps the old per-line guard (`y < 0 || y >= height || cols <= 0`); because every region's y grows with the row index, out-of-bounds rows are always a tail, so placed rows stay contiguous — identity "(region, row index)" is well-defined.

**Retention.** `TerminalShellRenderer` keeps: the root `BoxRenderable` ("workbench", overflow hidden), `regionRows: Map<regionKey, TextRenderable[]>`, and the palette overlay `BoxRenderable`. Each `render()`:
1. computes the plan;
2. full-rebuilds only when the renderer dimensions changed, the frame is missing, or the ordered region-key list changed (resize, geometry-class transition, prompt on/off — the palette is NOT a rebuild trigger);
3. per region: existing row renderables update `left/top/width/content` in place; missing tail rows are created and appended; surplus tail rows are destroyed;
4. overlay box: created (z 1) / geometry-updated / destroyed with the palette;
5. publishes `createdLastRender` and the frozen hit map.

Region keys: `header`, `prompt`, `surface:${SurfaceId}` per column, `attention`, `surface:Composer`, `help`, `palette`. Renderable ids are `${regionKey}#${index}` (e.g. `surface:Primary#3`) — stable across frames, replacing the old per-frame `cell-${x}-${y}`.

**Paint order is structural, not historical (the one non-obvious hazard).** OpenTUI appends every NEW child to the END of the z-ordered child list — even `insertBefore` splices the layout list at the anchor but pushes the z-list tail (`chunk-bun-bwjmgnxw.js:1259-1260` vs `:1265-1270`), and the render walk iterates `_childrenInZIndexOrder` (`:1354-1369`, stable sort `:753-756`). So a row appended mid-history (say a composer line while the palette is open) would paint ON TOP of the palette background, unlike a rebuild. Fix: z-tiers — base rows `zIndex 0`, palette background `zIndex 1`, palette rows `zIndex 2`. Base regions occupy disjoint row bands by construction (header=0, columns=[2, footerTop), attention/composer/help partition [footerTop, height)), so their relative order cannot affect pixels; the only cross-region overlaps are palette-over-base and palette-rows-over-overlay, both pinned by the tiers — exactly the rebuild stacking. Proven by mutation: removing the tiers fails the equivalence test (see §4).

## 2. Mutable OpenTUI API used (read before writing, per task)

| API | Declaration | Runtime behavior |
|---|---|---|
| `TextRenderable.content` setter | `node_modules/@opentui/core/renderables/Text.d.ts:24` | `chunk-bun-bwjmgnxw.js:4015-4023` → `updateTextBuffer` → `textBuffer.setStyledText`; `updateTextInfo` (`:3082-3089`) does `yogaNode.markDirty(); requestRender()`. Same code path the constructor uses, so in-place updates are pixel-identical to fresh construction. |
| `TextBufferRenderable.fg/.bg/.attributes` setters | `renderables/TextBufferRenderable.d.ts:72-81` | available if per-renderable styling is ever needed; current styling is per-chunk in `StyledText`, so only `content` is exercised. |
| `Renderable.top/.left` setters | `core/Renderable.d.ts:197-204` | `setPosition` → `updateYogaPosition` → yoga `setPosition` + `requestRender()` (`chunk-bun-bwjmgnxw.js:902-939`). |
| `Renderable.width/.height` setters | `core/Renderable.d.ts:207-210` | guard on unchanged value, else yoga set + `requestRender()` (`:712-736`). |
| `Renderable.add(child)` | `core/Renderable.d.ts:262` | appends to both child lists + yoga insertChild + `requestRender()` (`:1174-1214`). |
| `Renderable.remove(child)` / `destroy()` | `:265` / `:283` | `remove` detaches from both lists + yoga and requests render (`:1281-1303`); `destroy` removes itself from its parent (`:1420-1446`), so destroying a surplus row detaches it cleanly. |
| `RenderableOptions.zIndex` | `core/Renderable.d.ts:67` | stable sort in `ensureZIndexSorted` (`:753-756`); render walk iterates z-order (`:1354`). |

Also noted for increment 3: OpenTUI already maintains an internal hit grid (`addToHitGrid`, `:1379`) with `CliRenderer.hitTest` (`core/renderer.d.ts:531`) — but it is keyed by renderable number, while the exported `hitMap()` carries semantic line ids, which is what gesture→command lowering needs.

## 3. Hit-map shape

```ts
interface HitMapRegion {
  readonly surfaceId: string;          // SurfaceId where one exists ("Navigator" | "Primary" | "Inspector" | "Composer" | "Overlay"),
                                       // else chrome names: "header" | "prompt" | "attention" | "help"
  readonly x: number; readonly y: number;   // top-left cell of row 0
  readonly width: number; readonly height: number;  // height = rendered row count
  readonly rows: readonly (string | null)[];        // rows[i] = RenderedLine.id of the row painted at (x, y+i); null when the line carries no id
}
interface ShellHitMap { readonly regions: readonly HitMapRegion[]; }
```

`hitMap()` returns the deep-frozen map of the last render (empty before the first render and after `destroy()`). Row ids are DERIVED from the span tree (`RenderedLine.id ?? null`) — ids like `palette.${i}`, `work.unit.${id}`, `outline.*`, `primary.tabs`, `shell.header` flow through unchanged; nothing is duplicated. (x, y) → region resolves by containment over `regions`. Palette windowing is honest: `rows` are the ids of the VISIBLE window (`palette.${start + i}`), since identity is the placed row.

Public API is additive: constructor, `render()`, `destroy()` unchanged; new `hitMap()` and `createdLastRender` getter. `main.ts` needs no edit.

## 4. Verification

- **Test-first (RED)**: `renderer-hitmap.test.ts` written before implementation — 5/6 failed (`view.hitMap is not a function`, `createdLastRender` undefined).
- **Reuse proof**: no-change re-render → `createdLastRender === 0`, byte-identical frame, same frame renderable object and child count; one-line append to one surface → exactly 1 created and visible at the region tail.
- **Visible-output equivalence**: (a) existing suites stay green — `terminal-renderer.test.ts`, `cell-fit.test.ts`, `production-entry.test.ts` (real spawned `main.ts` PTY loop: same-geometry source updates, palette open/close, focus moves, resize 120→80→70→120, below-minimum prompt, recovery); (b) new adversarial test: a fresh single-shot build of a final state (palette open + appended primary row + attention row shifting the footer) is byte-compared (`captureCharFrame`) against a retained terminal that walked there through intermediate states, INCLUDING a base-row append and footer shift while the overlay was open.
- **Mutation check (teeth)**: deleting the z-tiers makes exactly that equivalence test fail (appended row paints over the palette) — the suite detects the paint-order hazard it claims to pin. Implementation restored and re-verified.
- **Final state**: `bun test src/terminal-renderer.test.ts src/renderer-hitmap.test.ts src/renderer-boundary.test.ts src/cell-fit.test.ts src/production-entry.test.ts` → 21 pass / 0 fail (6 new); `bun run check` (tsc) clean; `biome check` clean on owned files (1 info-level env-access style suggestion on a construct identical to the committed original).

## 5. Residuals / next-increment notes

- `set content` compares StyledText by reference, so every row re-lowers its text each frame (zero CREATIONS on no-change, not zero dirty). A chunk-equality fast path is possible but was out of the minimal-change budget.
- Sibling increments were landing concurrently in the same worktree (main.ts, theme.ts, geometry.ts, shell.ts palette-query work, trace-stream, etc.); the shared `bun run check` was transiently red from their mid-edit state and is green at final run. Nothing outside the two owned files was modified by this increment.
- Increment 3 (mouse convergence) should map gestures via `hitMap()` (semantic ids) rather than OpenTUI's internal number-keyed grid, and can now assume (region, row) renderable identity across frames.
