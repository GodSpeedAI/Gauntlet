import {
  BoxRenderable,
  type CliRenderer,
  type KeyEvent,
  type MouseEvent,
  RGBA,
  StyledText,
  TextAttributes,
  TextRenderable,
} from "@opentui/core";
import { cellWidth, fitCellText, sanitizeCellText } from "./geometry";
import type { RenderedLine, RenderedSpan } from "./render-tree";
import type { ColumnBoundary, PointerGesture, RenderedShell } from "./shell";
import { resolveToken } from "./theme";

/** Native parsing owns escape timeouts, modifiers and Unicode decoding. */
export function terminalKey(key: KeyEvent): string | null {
  if (key.eventType === "release") return null;
  if (key.ctrl) return `ctrl+${key.name}`;
  if (key.meta || key.option) return null;
  if (key.name === "tab") return key.shift ? "shift+tab" : "tab";
  if (key.name === "return" || key.name === "linefeed") return "enter";
  if (
    ["escape", "backspace", "delete", "up", "down", "left", "right", "home", "end"].includes(
      key.name,
    )
  )
    return key.name;
  return key.sequence || key.name;
}

/** Fit one line's spans to whole terminal cells: sanitize control input,
 * then clip span by span so leading spans (focus markers) keep priority and
 * the tail yields. The row can never exceed maxCells cells. */
function fitSpans(spans: readonly RenderedSpan[], maxCells: number): RenderedSpan[] {
  const out: RenderedSpan[] = [];
  let used = 0;
  for (const span of spans) {
    if (used >= maxCells) break;
    const piece = fitCellText(sanitizeCellText(span.text), maxCells - used);
    used += cellWidth(piece);
    out.push({ ...span, text: piece });
  }
  return out;
}

/** Lower one fitted row to a StyledText (semantic tokens resolve here only,
 * DSN-001; identical for initial creation and in-place content updates). */
function styledContent(spans: readonly RenderedSpan[], cols: number): StyledText {
  return new StyledText(
    fitSpans(spans, cols).map((span) => {
      const style = span.token
        ? resolveToken(
            span.token,
            process.env["NO_COLOR"] === undefined ? "truecolor" : "monochrome",
          )
        : null;
      const emphasis = span.emphasis ?? style?.emphasis;
      return {
        __isChunk: true as const,
        text: span.text,
        ...(style?.color ? { fg: RGBA.fromHex(style.color) } : {}),
        attributes:
          emphasis === "bold"
            ? TextAttributes.BOLD
            : emphasis === "dim"
              ? TextAttributes.DIM
              : TextAttributes.NONE,
      };
    }),
  );
}

// --- Frame plan (pure placement; mirrors the per-draw lowering exactly) --------

/** Where one row of one region is placed in this frame. */
interface RowPlacement {
  readonly spans: readonly RenderedSpan[];
  readonly lineId: string | null;
  readonly x: number;
  readonly y: number;
  readonly cols: number;
}

/**
 * One region of the frame: rows share x/cols, row i paints at y + i, and the
 * region key is stable across frames, which is what makes (region, row) a
 * stable renderable identity for mouse hit-testing.
 */
interface RegionPlacement {
  readonly key: string;
  readonly surfaceId: string;
  readonly x: number;
  readonly y: number;
  readonly width: number;
  readonly rows: readonly RowPlacement[];
}

interface OverlayPlacement {
  readonly x: number;
  readonly y: number;
  readonly width: number;
  readonly height: number;
  /** Palette/context-menu box title; unused for the drawer overlay (its
   * title comes from shell.drawer.title, set at render time instead). */
  readonly title?: string;
}

/** Hover tooltip placement (UESC §22.6): a small, NON-hit-tested box near the
 * hovered row — it is never pushed onto `regions`, so resolveAt (and thus
 * every click/double-click/right-click/drag gesture) never resolves to it;
 * a tooltip must never suppress interaction with whatever is under or beside
 * it (ICM §35 rule 7). Positioned one row below the hovered cell (or above,
 * if the hovered row is the last one) so it never covers the cell it
 * describes — the one placement the campaign's own constraint calls out by
 * name ("never...on the same cell"). */
interface TooltipPlacement {
  readonly x: number;
  readonly y: number;
  readonly width: number;
  readonly lines: readonly RenderedLine[];
}

interface FramePlan {
  readonly regions: readonly RegionPlacement[];
  readonly overlay: OverlayPlacement | null;
  readonly drawer: OverlayPlacement | null;
  readonly tooltip: TooltipPlacement | null;
}

/**
 * Rows for one region, as the per-draw lowering would have placed them: only
 * rows that land inside the terminal are placed (the old per-line guard), and
 * because every region's y grows with the row index, out-of-bounds rows are
 * always a tail — so placed rows stay contiguous from `y`.
 */
function planRows(
  lines: readonly RenderedLine[],
  x: number,
  y: number,
  cols: number,
  cap: number,
  height: number,
): RowPlacement[] {
  const rows: RowPlacement[] = [];
  lines.slice(0, cap).forEach((line, index) => {
    const rowY = y + index;
    if (rowY < 0 || rowY >= height || cols <= 0) return;
    rows.push({ spans: line.spans, lineId: line.id ?? null, x, y: rowY, cols });
  });
  return rows;
}

/** Pure lowering plan for one shell state: identical arithmetic to a
 * destroy-and-rebuild draw, expressed as placed regions instead of immediate
 * renderable construction. */
function planFrame(shell: RenderedShell, width: number, height: number): FramePlan {
  const regions: RegionPlacement[] = [];
  regions.push({
    key: "header",
    surfaceId: "header",
    x: 0,
    y: 0,
    width,
    rows: planRows([shell.header], 0, 0, width, 1, height),
  });
  // MenuBar title strip (UESC §22.7): always-visible chrome directly below
  // the header, occupying the row that previously sat blank between the
  // header and the body (y=1) — no other region's placement math changes.
  if (shell.menuBarLine) {
    regions.push({
      key: "menubar",
      surfaceId: "menubar",
      x: 0,
      y: 1,
      width,
      rows: planRows([shell.menuBarLine], 0, 1, width, 1, height),
    });
  }
  if (shell.prompt) {
    regions.push({
      key: "prompt",
      surfaceId: "prompt",
      x: 0,
      y: 2,
      width,
      rows: planRows(shell.prompt.lines, 0, 2, width, shell.prompt.lines.length, height),
    });
    return { regions, overlay: null, drawer: null, tooltip: null };
  }
  const composer = shell.surfaces.find((surface) => surface.id === "Composer");
  const helpRows = Math.min(shell.help.length, 2);
  const composerRows = Math.min(composer?.lines.length ?? 0, Math.max(3, height - 8));
  const attentionRows = Math.min(shell.attention.length, 3);
  // Bottom Surface (UESC §7.5): a collapsed entry row, or the open row plus
  // an honest no-provider note; hidden entirely at short heights because the
  // shell renders bottom: null there (collapse order: bottom first).
  const bottomLines: RenderedLine[] =
    shell.bottom === null
      ? []
      : [
          {
            id: "bottom.entry",
            spans: [
              {
                text: shell.bottom.open
                  ? `▾ bottom (b) · ${shell.bottom.tabs.join(" · ")}`
                  : `▸ bottom (b) · ${shell.bottom.tabs.join(" · ")}`,
                emphasis: "dim" as const,
              },
            ],
          },
          ...(shell.bottom.open
            ? [
                {
                  id: "bottom.note",
                  spans: [
                    {
                      text:
                        shell.bottom.tabs.length > 0
                          ? "select a tab above · providers absent are not shown (REQ-AFF-004)"
                          : "no bottom provider is currently materialized (REQ-AFF-004)",
                      emphasis: "dim" as const,
                    },
                  ],
                },
              ]
            : []),
        ];
  const bottomRows = Math.min(bottomLines.length, 3);
  const footerTop = height - helpRows - composerRows - attentionRows - bottomRows;
  let x = 0;
  const columns = shell.surfaces.filter((surface) => surface.id !== "Composer");
  columns.forEach((surface, columnIndex) => {
    // One-cell negative-space gutter between adjacent columns, spent from
    // inside the existing allocation: the x-advance is unchanged, so the
    // geometry sums still hold exactly and no separator is drawn.
    const gutter = columnIndex < columns.length - 1 ? 1 : 0;
    const cols = Math.max(1, surface.width - gutter);
    const capped = surface.lines.slice(0, Math.max(0, footerTop - 2));
    const lines = capped.map((row, index) =>
      index === 0 && surface.focused
        ? { ...row, spans: [{ text: "▸ ", token: "focus.ring" as const }, ...row.spans] }
        : row,
    );
    regions.push({
      key: `surface:${surface.id}`,
      surfaceId: surface.id,
      x,
      y: 2,
      width: cols,
      rows: planRows(lines, x, 2, cols, lines.length, height),
    });
    x += surface.width;
  });
  if (bottomRows > 0) {
    regions.push({
      key: "bottom",
      surfaceId: "Bottom",
      x: 0,
      y: footerTop,
      width,
      rows: planRows(bottomLines, 0, footerTop, width, bottomRows, height),
    });
  }
  regions.push({
    key: "attention",
    surfaceId: "attention",
    x: 0,
    y: footerTop + bottomRows,
    width,
    rows: planRows(shell.attention, 0, footerTop + bottomRows, width, attentionRows, height),
  });
  const composerLines = (composer?.lines ?? []).map((row, index) =>
    index === 0 && composer?.focused
      ? { ...row, spans: [{ text: "▸ ", token: "focus.ring" as const }, ...row.spans] }
      : row,
  );
  regions.push({
    key: "surface:Composer",
    surfaceId: "Composer",
    x: 0,
    y: footerTop + bottomRows + attentionRows,
    width,
    rows: planRows(
      composerLines,
      0,
      footerTop + bottomRows + attentionRows,
      width,
      composerRows,
      height,
    ),
  });
  regions.push({
    key: "help",
    surfaceId: "help",
    x: 0,
    y: height - helpRows,
    width,
    rows: planRows(shell.help, 0, height - helpRows, width, helpRows, height),
  });
  // Compact drawer (V2): planned before the palette branch so it renders
  // whether or not the palette is open (the palette overlays it, z-tiers
  // keep stacking deterministic).
  const drawerPlacement = drawerPlan(shell, width, footerTop);
  if (shell.drawer !== null && drawerPlacement !== null) {
    const drawerRows = Math.max(1, drawerPlacement.height - 1);
    regions.push({
      key: "drawer",
      surfaceId: "Drawer",
      x: drawerPlacement.x + 1,
      y: drawerPlacement.y + 1,
      width: Math.max(1, drawerPlacement.width - 2),
      rows: planRows(
        shell.drawer.lines,
        drawerPlacement.x + 1,
        drawerPlacement.y + 1,
        Math.max(1, drawerPlacement.width - 2),
        drawerRows,
        height,
      ),
    });
  }
  // Hover tooltip (UESC §22.6): resolved against the rows already planned
  // above (base columns, bottom, attention, composer, help, drawer) — the
  // SAME row placement a click/right-click would hit-test against, so the
  // tooltip can never drift from where the hovered row actually paints.
  // shell.tooltip is already null whenever the palette/context-menu/menu-bar
  // owns the surface (renderShell's own mutual-exclusivity guard), so this
  // is safe to compute unconditionally and reuse in every branch below.
  const tooltipPlacement = tooltipPlan(shell, regions, width, height);
  // MenuBar dropdown (UESC §22.7): a small overlay anchored under the open
  // title, sharing the same Overlay z-tier/hit-testing path as the palette
  // and context menu — mutually exclusive with both (ShellState enforces
  // it), so this branch and the ones below it can never collide.
  if (shell.menuBar) {
    const lines = shell.menuBar.lines;
    const contentWidth = lines.reduce(
      (max, line) =>
        Math.max(
          max,
          line.spans.reduce((sum, span) => sum + span.text.length, 0),
        ),
      16,
    );
    const menuWidth = Math.min(contentWidth + 2, Math.max(1, width - 4));
    const menuHeight = Math.min(lines.length, Math.max(1, height - 6));
    const menuX = Math.min(Math.max(0, shell.menuBar.anchorX), Math.max(0, width - menuWidth - 1));
    const menuY = Math.min(
      Math.max(0, shell.menuBar.anchorY),
      Math.max(0, height - menuHeight - 1),
    );
    regions.push({
      key: "menubar-dropdown",
      surfaceId: "Overlay",
      x: menuX,
      y: menuY,
      width: menuWidth,
      rows: planRows(lines, menuX, menuY, menuWidth, menuHeight, height),
    });
    return {
      regions,
      overlay: {
        x: Math.max(0, menuX - 1),
        y: Math.max(0, menuY - 1),
        width: Math.min(width, menuWidth + 2),
        height: Math.min(height, menuHeight + 2),
        title: ` ${shell.menuBar.titleLabel} · ↑/↓ select · Enter activate · Esc close `,
      },
      drawer: drawerPlacement,
      tooltip: tooltipPlacement,
    };
  }
  // ContextMenu (ICM §6.4, UESC §22.5): a small overlay anchored near the
  // right-click point, sharing the palette's z-tier and hit-testing path but
  // never open at the same time as it (ShellState enforces mutual
  // exclusivity, so this branch and the palette branch below can't collide).
  if (shell.contextMenu) {
    const lines = shell.contextMenu.lines;
    const contentWidth = lines.reduce(
      (max, line) =>
        Math.max(
          max,
          line.spans.reduce((sum, span) => sum + span.text.length, 0),
        ),
      20,
    );
    const menuWidth = Math.min(contentWidth + 2, Math.max(1, width - 4));
    const menuHeight = Math.min(lines.length, Math.max(1, height - 6));
    const menuX = Math.min(
      Math.max(0, shell.contextMenu.anchorX),
      Math.max(0, width - menuWidth - 1),
    );
    const menuY = Math.min(
      Math.max(0, shell.contextMenu.anchorY),
      Math.max(0, height - menuHeight - 1),
    );
    regions.push({
      key: "contextmenu",
      surfaceId: "Overlay",
      x: menuX,
      y: menuY,
      width: menuWidth,
      rows: planRows(lines, menuX, menuY, menuWidth, menuHeight, height),
    });
    return {
      regions,
      overlay: {
        x: Math.max(0, menuX - 1),
        y: Math.max(0, menuY - 1),
        width: Math.min(width, menuWidth + 2),
        height: Math.min(height, menuHeight + 2),
        title: " Context Menu · ↑/↓ select · Enter activate · Esc close ",
      },
      drawer: drawerPlacement,
      tooltip: tooltipPlacement,
    };
  }
  if (!shell.palette)
    return { regions, overlay: null, drawer: drawerPlacement, tooltip: tooltipPlacement };
  // The overlay gets a bounded scrolling viewport, including the selected row.
  const available = Math.max(1, footerTop - 4);
  const selected = Math.min(shell.palette.index, shell.palette.lines.length - 1);
  const start = Math.max(0, selected - available + 1);
  regions.push({
    key: "palette",
    surfaceId: "Overlay",
    x: 2,
    y: 2,
    width: width - 4,
    rows: planRows(
      shell.palette.lines.slice(start, start + available),
      2,
      2,
      width - 4,
      available,
      height,
    ),
  });
  return {
    regions,
    overlay: {
      x: 1,
      y: 1,
      width: Math.max(1, width - 2),
      height: Math.max(1, footerTop - 1),
      title: " Palette · ↑/↓ select · Enter open · Esc dismiss ",
    },
    drawer: drawerPlacement,
    tooltip: tooltipPlacement,
  };
}

/**
 * Placement for the hover tooltip, resolved against rows already placed in
 * `regions` (the SAME row geometry click/right-click hit-test against — see
 * resolveAt). Returns null when nothing is hovered, the hovered row is no
 * longer present in this frame's regions (its surface scrolled/collapsed
 * since the hover started), or the terminal is too narrow for even a
 * one-cell box. Placed one row BELOW the hovered cell (or above it, if the
 * hovered row is the last visible one) — deliberately never on top of the
 * cell it describes, so it can never suppress a click/drag on that cell.
 */
function tooltipPlan(
  shell: RenderedShell,
  regions: readonly RegionPlacement[],
  width: number,
  height: number,
): TooltipPlacement | null {
  if (shell.tooltip === null) return null;
  const region = regions.find((candidate) => candidate.surfaceId === shell.tooltip?.surfaceId);
  const row = region?.rows.find((candidate) => candidate.lineId === shell.tooltip?.lineId);
  if (region === undefined || row === undefined) return null;
  const contentWidth = shell.tooltip.lines.reduce(
    (max, line) =>
      Math.max(
        max,
        line.spans.reduce((sum, span) => sum + span.text.length, 0),
      ),
    4,
  );
  const tooltipWidth = Math.min(contentWidth, Math.max(1, width - row.x));
  const below = row.y + 1;
  const y = below < height ? below : Math.max(0, row.y - 1);
  return { x: row.x, y, width: tooltipWidth, lines: shell.tooltip.lines };
}

/** Compact side drawer placement (V2): bordered left/right overlay sharing
 * the palette's z-tiers. Rows join the region list with surfaceId "Drawer". */
function drawerPlan(
  shell: RenderedShell,
  width: number,
  footerTop: number,
): OverlayPlacement | null {
  if (shell.drawer === null) return null;
  const height = Math.max(1, footerTop - 2);
  const width42 = Math.min(Math.max(24, Math.floor(width / 3)), Math.max(1, width - 4));
  return {
    x: shell.drawer.side === "left" ? 1 : Math.max(1, width - width42 - 1),
    y: 1,
    width: width42,
    height,
  };
}

// --- Hit map (machine-usable placement record for the gesture layer) ----------

/** One hit-testable placed region: rows[i] is the span-tree line id painted
 * at (x, y + i), or null when that line carries no stable id. */
export interface HitMapRegion {
  readonly surfaceId: string;
  readonly x: number;
  readonly y: number;
  readonly width: number;
  readonly height: number;
  readonly rows: readonly (string | null)[];
}

/**
 * The frozen hit map of the last render: (surfaceId, row) and (x, y) → region
 * by containment. Increment 3 lowers pointer gestures onto it; nothing here
 * duplicates the span tree — every row id is derived from RenderedLine.id.
 */
export interface ShellHitMap {
  readonly regions: readonly HitMapRegion[];
}

const EMPTY_HIT_MAP: ShellHitMap = Object.freeze({ regions: Object.freeze([]) });

function freezeHitMap(plan: FramePlan): ShellHitMap {
  return Object.freeze({
    regions: Object.freeze(
      plan.regions.map((region) =>
        Object.freeze({
          surfaceId: region.surfaceId,
          x: region.x,
          y: region.y,
          width: region.width,
          height: region.rows.length,
          rows: Object.freeze(region.rows.map((row) => row.lineId)),
        }),
      ),
    ),
  });
}

/** Lowers immutable shell output to terminal cells; owns no application state.
 *
 * Retained lowering (GUI-redesign increment 2c): the root Box and one
 * TextRenderable per (region, row) survive across renders; content and
 * placement update in place, rows append/remove only at region tails, and the
 * whole tree rebuilds only on resize or a layout-shape change. Paint order is
 * structural, not historical: newly created renderables always append to the
 * end of OpenTUI's z-ordered child list, so cross-region overlap order must
 * not depend on insertion history — base regions occupy disjoint row bands
 * (z 0), the palette background is z 1, and palette rows are z 2, which is
 * exactly the rebuild order's stacking. */
/** Optional pointer plumbing: when onGesture is supplied, row renderables
 * forward native mouse events; the renderer resolves them against the last
 * hit map and emits click/double_click/wheel gestures for the shell's
 * handlePointer (ICM §35 — gestures, not business mutations). */
export interface TerminalShellRendererOptions {
  readonly onGesture?: ((gesture: PointerGesture) => void) | undefined;
}

const DOUBLE_CLICK_MS = 500;

export class TerminalShellRenderer {
  private frame: BoxRenderable | null = null;
  private frameWidth = -1;
  private frameHeight = -1;
  private regionKeys: readonly string[] = [];
  private readonly regionRows = new Map<string, TextRenderable[]>();
  private overlayBox: BoxRenderable | null = null;
  private drawerBox: BoxRenderable | null = null;
  /** Hover tooltip text (UESC §22.6) — a plain TextRenderable, never a
   * BoxRenderable: it carries no onMouse handler and is never pushed into
   * `regions`/the hit map, so it can never occlude a click/drag meant for
   * whatever is under or beside it. */
  private tooltipText: TextRenderable | null = null;
  private createdCount = 0;
  private lastHitMap: ShellHitMap = EMPTY_HIT_MAP;
  private readonly onGesture: ((gesture: PointerGesture) => void) | null;
  private pendingDown: { x: number; y: number; button: number; time: number } | null = null;
  private pendingRightDown: { x: number; y: number } | null = null;
  private lastClick: { x: number; y: number; time: number } | null = null;
  /** Armed on a left-button down over a column-seam hit-zone (ICM §36.1);
   * promoted to `activeResize` once movement crosses the same >1-cell
   * threshold click-vs-drag already uses elsewhere in this handler. */
  private pendingResize: { boundary: ColumnBoundary; startX: number } | null = null;
  /** Non-null for the lifetime of one RESIZING drag (UESC §22.11). */
  private activeResize: ColumnBoundary | null = null;

  constructor(
    private readonly renderer: CliRenderer,
    options: TerminalShellRendererOptions = {},
  ) {
    this.onGesture = options.onGesture ?? null;
  }

  /** Shared row mouse handler: synthesizes click/double/wheel gestures. */
  private readonly rowMouseEvent = (event: MouseEvent): void => {
    if (this.onGesture === null) return;
    if (event.type === "scroll") {
      const direction = event.scroll?.direction === "up" ? -1 : 1;
      const magnitude = Math.max(1, Math.abs(event.scroll?.delta ?? 1));
      const hit = this.resolveAt(event.x, event.y);
      if (hit === null) return;
      this.onGesture({
        kind: "wheel",
        surfaceId: hit.surfaceId,
        delta: direction * magnitude,
      });
      return;
    }
    // Right button (§35.3 context menu, increment 5): its own down/up
    // debounce pair, tracked separately from the left button's so a stray
    // right-down can't be paired against a left-up (or vice versa).
    if (event.button === 2) {
      if (event.type === "down") {
        this.pendingRightDown = { x: event.x, y: event.y };
        return;
      }
      if (event.type !== "up") return;
      const rightDown = this.pendingRightDown;
      this.pendingRightDown = null;
      if (
        rightDown === null ||
        Math.abs(rightDown.x - event.x) > 1 ||
        Math.abs(rightDown.y - event.y) > 1
      ) {
        return;
      }
      const hit = this.resolveAt(event.x, event.y);
      if (hit === null) return;
      this.onGesture({
        kind: "right_click",
        surfaceId: hit.surfaceId,
        lineId: hit.lineId,
        x: event.x - hit.regionX,
        y: event.y,
      });
      return;
    }
    // Left button: ordinary click/double-click, AND column-splitter drag
    // (ICM §36.1 presentation drag, UESC §22.11 SplitPane). Both share the
    // same down/up debounce pair — a resize is just a click whose pointer
    // crossed the click-vs-drag threshold over a column seam, so the two
    // can never both fire for the same gesture.
    if (event.type === "down") {
      if (event.button !== 0) return;
      this.pendingDown = { x: event.x, y: event.y, button: event.button, time: Date.now() };
      const boundary = this.boundaryAt(event.x, event.y);
      this.pendingResize = boundary !== null ? { boundary, startX: event.x } : null;
      return;
    }
    if (event.type === "move" || event.type === "drag") {
      if (this.activeResize !== null) {
        this.onGesture({ kind: "column_resize_move", boundary: this.activeResize, x: event.x });
        return;
      }
      if (this.pendingResize !== null && Math.abs(event.x - this.pendingResize.startX) > 1) {
        const boundary = this.pendingResize.boundary;
        this.activeResize = boundary;
        this.pendingResize = null;
        // `_start` anchors on the ORIGINAL mousedown column (not the column
        // where the threshold happened to be crossed) so the delta the
        // shell computes on every following `_move` is relative to where
        // the operator actually grabbed the splitter.
        this.onGesture({
          kind: "column_resize_start",
          boundary,
          x: this.pendingDown?.x ?? event.x,
        });
        this.onGesture({ kind: "column_resize_move", boundary, x: event.x });
      }
      return;
    }
    if (event.type === "up" || event.type === "drag-end") {
      if (this.activeResize !== null) {
        const boundary = this.activeResize;
        this.activeResize = null;
        this.pendingResize = null;
        this.pendingDown = null;
        this.onGesture({ kind: "column_resize_end", boundary, x: event.x });
        return;
      }
      this.pendingResize = null;
      if (event.type === "drag-end") return; // no active resize: nothing to commit or click
      if (event.button !== 0) return;
      const down = this.pendingDown;
      this.pendingDown = null;
      if (down === null || Math.abs(down.x - event.x) > 1 || Math.abs(down.y - event.y) > 1) {
        return;
      }
      const hit = this.resolveAt(event.x, event.y);
      if (hit === null) return;
      const now = Date.now();
      const isDouble =
        this.lastClick !== null &&
        now - this.lastClick.time <= DOUBLE_CLICK_MS &&
        Math.abs(this.lastClick.x - event.x) <= 2 &&
        Math.abs(this.lastClick.y - event.y) <= 2;
      this.lastClick = { x: event.x, y: event.y, time: now };
      this.onGesture({
        kind: isDouble ? "double_click" : "click",
        surfaceId: hit.surfaceId,
        lineId: hit.lineId,
        x: event.x - hit.regionX,
      });
    }
    // Hover tooltip (UESC §22.6, chrome remainder item 6): OpenTUI's own
    // renderer synthesizes real over/out MouseEvents with no button held,
    // distinct from the drag-tracking move/drag this handler already
    // consumes above (confirmed by reading CliRenderer's own event routing —
    // it recomputes hit-target-changed over/out on every button-less move,
    // then separately dispatches the raw move/drag event afterward, which is
    // why the move/drag branch above must stay a no-op whenever no resize is
    // pending — this branch is where hover-tracking actually lives). Guarded
    // off entirely whenever ANY button interaction is in flight (a held
    // button, or a pending/active column resize) so hover can never race or
    // interfere with click/drag/resize detection on the same gesture.
    if (event.type === "over" || event.type === "out") {
      if (
        this.pendingDown !== null ||
        this.pendingRightDown !== null ||
        this.pendingResize !== null ||
        this.activeResize !== null
      ) {
        return;
      }
      if (event.type === "out") {
        this.onGesture({ kind: "hover", surfaceId: "", lineId: null });
        return;
      }
      const hit = this.resolveAt(event.x, event.y);
      this.onGesture({
        kind: "hover",
        surfaceId: hit?.surfaceId ?? "",
        lineId: hit?.lineId ?? null,
      });
    }
    // `drop` carries no gesture here (only meaningful for drag-and-drop,
    // which this shell has no drop targets for).
  };

  /**
   * Is (x, y) within one cell of a Navigator|Primary or Primary|Inspector
   * seam? The seam itself is a 1-cell gutter with no covering renderable
   * (planFrame spends it "from inside the existing allocation" as
   * negative-space between columns), so the practical grab zone is that
   * gutter plus the one rendered cell on either side of it — exactly the
   * `Math.abs(...) <= 1` tolerance used below. Returns null outside Full/Wide
   * (Compact renders no Navigator/Inspector regions to seam against).
   */
  private boundaryAt(x: number, y: number): ColumnBoundary | null {
    const regions = this.lastHitMap.regions;
    const navigator = regions.find((region) => region.surfaceId === "Navigator");
    const primary = regions.find((region) => region.surfaceId === "Primary");
    const inspector = regions.find((region) => region.surfaceId === "Inspector");
    if (navigator !== undefined && primary !== undefined) {
      const bottom = Math.min(navigator.y + navigator.height, primary.y + primary.height);
      const seam = navigator.x + navigator.width;
      if (y >= navigator.y && y < bottom && Math.abs(x - seam) <= 1) return "navigator";
    }
    if (primary !== undefined && inspector !== undefined) {
      const bottom = Math.min(primary.y + primary.height, inspector.y + inspector.height);
      const seam = primary.x + primary.width;
      if (y >= primary.y && y < bottom && Math.abs(x - seam) <= 1) return "inspector";
    }
    return null;
  }

  /**
   * Resolve terminal coordinates against the last hit map. Regions are
   * appended to `plan.regions` in PAINT order (base columns first, drawer
   * next, palette last — see the z-tiers comment above this class), and
   * OpenTUI's retained renderables always append to the end of the z-ordered
   * child list, so a later-pushed region paints ON TOP of an earlier one at
   * the same cells. Hit-testing must walk in the OPPOSITE (topmost-first)
   * order to match what the operator actually sees and clicks — walking
   * forward let a background surface (e.g. Navigator) silently swallow every
   * click meant for an open palette/drawer overlay drawn above it.
   */
  private resolveAt(
    x: number,
    y: number,
  ): { surfaceId: string; lineId: string | null; regionX: number } | null {
    const regions = this.lastHitMap.regions;
    for (let index = regions.length - 1; index >= 0; index -= 1) {
      const region = regions[index];
      if (
        region !== undefined &&
        x >= region.x &&
        x < region.x + region.width &&
        y >= region.y &&
        y < region.y + region.height
      ) {
        return {
          surfaceId: region.surfaceId,
          lineId: region.rows[y - region.y] ?? null,
          regionX: region.x,
        };
      }
    }
    return null;
  }

  destroy(): void {
    this.frame?.destroyRecursively();
    this.frame = null;
    this.frameWidth = -1;
    this.frameHeight = -1;
    this.regionKeys = [];
    this.regionRows.clear();
    this.overlayBox = null;
    this.drawerBox = null;
    this.tooltipText = null;
    this.lastHitMap = EMPTY_HIT_MAP;
  }

  /** Renderables created by the most recent render() (test-visible reuse
   * proof: 0 for a no-change re-render, 1 for a one-line append). */
  get createdLastRender(): number {
    return this.createdCount;
  }

  /** Frozen placement record of the last render for pointer hit-testing. */
  hitMap(): ShellHitMap {
    return this.lastHitMap;
  }

  render(shell: RenderedShell): void {
    const { width, height } = this.renderer;
    const plan = planFrame(shell, width, height);
    const keys = plan.regions.map((region) => region.key);
    if (
      this.frame === null ||
      width !== this.frameWidth ||
      height !== this.frameHeight ||
      keys.length !== this.regionKeys.length ||
      keys.some((key, index) => key !== this.regionKeys[index])
    ) {
      this.destroy();
      this.frame = new BoxRenderable(this.renderer, {
        id: "workbench",
        width,
        height,
        overflow: "hidden",
      });
      this.renderer.root.add(this.frame);
      this.frameWidth = width;
      this.frameHeight = height;
      this.regionKeys = keys;
    }
    let created = 0;
    for (const region of plan.regions) {
      let retained = this.regionRows.get(region.key);
      if (retained === undefined) {
        retained = [];
        this.regionRows.set(region.key, retained);
      }
      for (let index = 0; index < region.rows.length; index++) {
        const row = region.rows[index];
        if (row === undefined) continue;
        let renderable = retained[index];
        if (renderable === undefined) {
          renderable = new TextRenderable(this.renderer, {
            id: `${region.key}#${index}`,
            position: "absolute",
            left: row.x,
            top: row.y,
            width: row.cols,
            height: 1,
            wrapMode: "none",
            truncate: true,
            zIndex:
              region.key === "palette" ||
              region.key === "menubar-dropdown" ||
              region.key === "contextmenu"
                ? 2
                : 0,
            content: styledContent(row.spans, row.cols),
            // The generic onMouse handler is the only one that receives
            // scroll events (named onScroll does not dispatch in 0.5.11).
            ...(this.onGesture !== null ? { onMouse: this.rowMouseEvent } : {}),
          });
          this.frame.add(renderable);
          retained[index] = renderable;
          created++;
        } else {
          renderable.left = row.x;
          renderable.top = row.y;
          renderable.width = row.cols;
          renderable.content = styledContent(row.spans, row.cols);
        }
      }
      for (let index = region.rows.length; index < retained.length; index++) {
        retained[index]?.destroy();
      }
      retained.length = region.rows.length;
    }
    if (plan.overlay) {
      if (this.overlayBox === null) {
        this.overlayBox = new BoxRenderable(this.renderer, {
          id: "palette-background",
          position: "absolute",
          left: plan.overlay.x,
          top: plan.overlay.y,
          width: plan.overlay.width,
          height: plan.overlay.height,
          backgroundColor: RGBA.defaultBackground(),
          border: true,
          title: plan.overlay.title ?? "",
          zIndex: 1,
        });
        this.frame.add(this.overlayBox);
        created++;
      } else {
        this.overlayBox.left = plan.overlay.x;
        this.overlayBox.top = plan.overlay.y;
        this.overlayBox.width = plan.overlay.width;
        this.overlayBox.height = plan.overlay.height;
        this.overlayBox.title = plan.overlay.title ?? "";
      }
    } else if (this.overlayBox !== null) {
      this.overlayBox.destroy();
      this.overlayBox = null;
    }
    if (plan.drawer !== null && shell.drawer !== null) {
      if (this.drawerBox === null) {
        this.drawerBox = new BoxRenderable(this.renderer, {
          id: "drawer-background",
          position: "absolute",
          left: plan.drawer.x,
          top: plan.drawer.y,
          width: plan.drawer.width,
          height: plan.drawer.height,
          backgroundColor: RGBA.defaultBackground(),
          border: true,
          title: ` ${shell.drawer.title} · Esc closes `,
          zIndex: 1,
        });
        this.frame.add(this.drawerBox);
        created++;
      } else {
        this.drawerBox.left = plan.drawer.x;
        this.drawerBox.top = plan.drawer.y;
        this.drawerBox.width = plan.drawer.width;
        this.drawerBox.height = plan.drawer.height;
      }
    } else if (this.drawerBox !== null) {
      this.drawerBox.destroy();
      this.drawerBox = null;
    }
    if (plan.tooltip !== null) {
      const content = styledContent(plan.tooltip.lines[0]?.spans ?? [], plan.tooltip.width);
      if (this.tooltipText === null) {
        this.tooltipText = new TextRenderable(this.renderer, {
          id: "tooltip",
          position: "absolute",
          left: plan.tooltip.x,
          top: plan.tooltip.y,
          width: plan.tooltip.width,
          height: 1,
          wrapMode: "none",
          truncate: true,
          zIndex: 1,
          content,
          // The tooltip is never part of our OWN hit map (see the field
          // comment / tooltipPlan — it's never pushed onto `regions`), but it
          // is still a real OpenTUI renderable drawn on top of whatever cells
          // it occupies. Without an onMouse handler here, OpenTUI's native
          // dispatch delivers the event to this topmost renderable and drops
          // it — silently swallowing a real click on the row underneath it
          // (confirmed empirically: a click landing on the tooltip's cells
          // produced zero gestures before this fix). Wiring the SAME
          // rowMouseEvent handler here routes it through resolveAt exactly
          // like any other click, which resolves against our own hit map —
          // one that never contains the tooltip — so it falls straight
          // through to whatever real region is actually there.
          ...(this.onGesture !== null ? { onMouse: this.rowMouseEvent } : {}),
        });
        this.frame.add(this.tooltipText);
        created++;
      } else {
        this.tooltipText.left = plan.tooltip.x;
        this.tooltipText.top = plan.tooltip.y;
        this.tooltipText.width = plan.tooltip.width;
        this.tooltipText.content = content;
      }
    } else if (this.tooltipText !== null) {
      this.tooltipText.destroy();
      this.tooltipText = null;
    }
    this.createdCount = created;
    this.lastHitMap = freezeHitMap(plan);
  }
}
