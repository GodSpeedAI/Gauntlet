/**
 * Terminal geometry detection and responsive layout calculator.
 * Conforms strictly to spec REQ-UX-005, DSN-007, and DESIGN.md width classes.
 */

export type GeometryClass = "Wide" | "Full" | "Compact" | "BelowMinimum";

export interface Dimensions {
  readonly cols: number;
  readonly rows: number;
}

export interface ColumnLayout {
  readonly navigatorWidth: number;
  readonly primaryWidth: number;
  readonly inspectorWidth: number;
  readonly isMultiColumn: boolean;
}

export const MIN_COLS = 80;
export const MIN_ROWS = 24;
export const FULL_MIN_COLS = 120;
export const FULL_MIN_ROWS = 40;
export const WIDE_MIN_COLS = 140;

/** Vertical fit band; the boundary (40 rows) is DESIGN "Height constraints". */
export type HeightBand = "tall" | "short";

/**
 * Classifies terminal dimensions into normative geometry classes.
 * Boundaries are fixed by spec REQ-UX-005 and are not design-tunable.
 *
 * Height never drops columns: per DESIGN.md "Responsive Behavior" Full
 * (line 1496), below 40 rows at >=120 cols the Bottom Surface collapses
 * first while all three columns remain, so Full/Wide/Compact key on columns
 * alone above the 24-row floor, and the height ladder is carried separately
 * by heightBand/collapseOrder for the shell to consume.
 */
export function classifyGeometry(dims: Dimensions): GeometryClass {
  if (dims.cols < MIN_COLS || dims.rows < MIN_ROWS) {
    return "BelowMinimum";
  }
  if (dims.cols >= WIDE_MIN_COLS) {
    return "Wide";
  }
  if (dims.cols >= FULL_MIN_COLS) {
    return "Full";
  }
  return "Compact";
}

/**
 * Height band for the DESIGN.md "Height constraints" ladder (lines 1528-1539):
 * 40+ rows is tall; 24-39 rows is short. Consumers must gate on
 * classifyGeometry !== "BelowMinimum" first — below 24 rows the recoverable
 * prompt applies and no band is meaningful.
 */
export function heightBand(dims: Dimensions): HeightBand {
  return dims.rows >= FULL_MIN_ROWS ? "tall" : "short";
}

/**
 * The DESIGN.md height-constraint collapse order as data the shell consumes:
 * Bottom Surface first, then nonessential metadata, then the Navigator and
 * Inspector drawers to overlay — always preserving current Focus, current
 * View, Horizon/location, the attention cue, composer/required approval
 * input, and consequential status. Tall terminals collapse nothing.
 */
export function collapseOrder(band: HeightBand): readonly string[] {
  return band === "short" ? ["bottom", "metadata", "drawers"] : [];
}

/**
 * Computes column allocation across the surfaces.
 * Full/Wide allocate Navigator, Primary, and Inspector simultaneously.
 * Compact yields a single primary pane with switchable tabs.
 */
export function calculateColumns(dims: Dimensions): ColumnLayout {
  const geo = classifyGeometry(dims);

  switch (geo) {
    case "Wide": {
      const nav = 32;
      const inspector = 44;
      const primary = Math.max(40, dims.cols - nav - inspector);
      return {
        navigatorWidth: nav,
        primaryWidth: primary,
        inspectorWidth: inspector,
        isMultiColumn: true,
      };
    }
    case "Full": {
      const nav = 28;
      const inspector = 36;
      const primary = Math.max(36, dims.cols - nav - inspector);
      return {
        navigatorWidth: nav,
        primaryWidth: primary,
        inspectorWidth: inspector,
        isMultiColumn: true,
      };
    }
    case "Compact":
    case "BelowMinimum":
      return {
        navigatorWidth: 0,
        primaryWidth: dims.cols,
        inspectorWidth: 0,
        isMultiColumn: false,
      };
  }
}

/**
 * Renders the recoverable compact-size prompt required by REQ-UX-005 / DSN-007.
 * Must name current and required sizes, reachable exit ('q'), and state that
 * admitted work keeps running.
 */
export function renderBelowMinimumPrompt(dims: Dimensions): string {
  return [
    `Gauntlet needs at least ${MIN_COLS}x${MIN_ROWS}. This terminal is ${dims.cols}x${dims.rows}.`,
    "Resize the window, or press q to quit.",
    "(admitted work keeps running)",
  ].join("\n");
}

/**
 * Terminal cell fitting (TUI-INSPECTION finding 6: fitting counted UTF-16
 * length while the terminal counts cells, so wide glyphs spilled past their
 * column and the right column was cut off the window).
 *
 * Rules, in the safe direction (uncertainty rounds UP, so text can only get
 * narrower, never overflow): East Asian wide/fullwidth and code points past
 * U+1F000 (emoji and symbol planes) count 2; combining marks and C0/C1
 * controls count 0; everything else counts 1. This matches OpenTUI's default
 * unicode width method for all content the workbench renders.
 */
const WIDE_RANGES: ReadonlyArray<readonly [number, number]> = [
  [0x1100, 0x115f],
  [0x2329, 0x232a],
  [0x2e80, 0x303e],
  [0x3041, 0x33ff],
  [0x3400, 0x4dbf],
  [0x4e00, 0x9fff],
  [0xa960, 0xa97c],
  [0xac00, 0xd7a3],
  [0xf900, 0xfaff],
  [0xfe10, 0xfe19],
  [0xfe30, 0xfe4f],
  [0xff00, 0xff60],
  [0xffe0, 0xffe6],
  [0x1f000, 0x10ffff],
];

const MARK_RANGES: ReadonlyArray<readonly [number, number]> = [
  [0x0300, 0x036f],
  [0x1ab0, 0x1aff],
  [0x1dc0, 0x1dff],
  [0x20d0, 0x20ff],
  [0xfe00, 0xfe0f],
  [0xfe20, 0xfe2f],
];

function inRanges(codePoint: number, ranges: ReadonlyArray<readonly [number, number]>): boolean {
  return ranges.some(([lo, hi]) => codePoint >= lo && codePoint <= hi);
}

/** Whether a code point is a non-printing control (C0, C1, or DEL). */
function isNonPrinting(codePoint: number): boolean {
  return codePoint < 0x20 || (codePoint >= 0x80 && codePoint < 0xa0) || codePoint === 0x7f;
}

/** Cells occupied by one grapheme cluster. */
export function graphemeCells(cluster: string): number {
  const codePoint = cluster.codePointAt(0) ?? 0;
  if (isNonPrinting(codePoint) || inRanges(codePoint, MARK_RANGES)) return 0;
  if (inRanges(codePoint, WIDE_RANGES)) return 2;
  return 1;
}

function splitGraphemes(text: string): string[] {
  const segs = new Intl.Segmenter("en", { granularity: "grapheme" }).segment(text);
  return [...segs].map((part) => part.segment);
}

/** Terminal cells occupied by a string (sum over grapheme clusters). */
export function cellWidth(text: string): number {
  let width = 0;
  for (const cluster of splitGraphemes(text)) {
    width += graphemeCells(cluster);
  }
  return width;
}

/**
 * Clip text to whole cells: never splits a wide glyph (a glyph that does not
 * fit the remaining cells is dropped) and never emits a truncation marker,
 * matching the renderer's clip-without-indicator contract.
 */
export function fitCellText(text: string, maxCells: number): string {
  if (maxCells <= 0) return "";
  if (cellWidth(text) <= maxCells) return text;
  let out = "";
  let width = 0;
  for (const cluster of splitGraphemes(text)) {
    const cells = graphemeCells(cluster);
    if (width + cells > maxCells) break;
    out += cluster;
    width += cells;
  }
  return out;
}

/** Pad text with spaces to an exact cell width; never truncates. */
export function padCellText(text: string, width: number): string {
  const current = cellWidth(text);
  return current >= width ? text : text + " ".repeat(width - current);
}

/**
 * Wrap text to whole terminal cells for a fixed value column: breaks at word
 * boundaries; a token wider than the column breaks at its own delimiters
 * (`- : / . _`) so machine values (ids, paths, uuids) stay readable, and a
 * delimiter-free run wider than the column is hard-split at the cell
 * boundary. Nothing is dropped; a glyph wider than the column passes through
 * verbatim (the renderer clips); non-positive widths are a no-op.
 */

/** Split a token into runs that each END with one structural delimiter. */
function splitAtDelimiters(word: string): string[] {
  const parts: string[] = [];
  let current = "";
  for (const char of word) {
    current += char;
    if ("-:/.=_".includes(char)) {
      parts.push(current);
      current = "";
    }
  }
  if (current !== "") {
    parts.push(current);
  }
  return parts;
}

interface WrapAtom {
  readonly text: string;
  /** Whether a space separates this atom from the accumulated line. */
  readonly spaceBefore: boolean;
}

function atomsForWord(word: string, width: number, spaceBefore: boolean): WrapAtom[] {
  if (cellWidth(word) <= width) {
    return [{ text: word, spaceBefore }];
  }
  const atoms: WrapAtom[] = [];
  const first = () => (atoms.length === 0 ? spaceBefore : false);
  for (const part of splitAtDelimiters(word)) {
    if (cellWidth(part) <= width) {
      atoms.push({ text: part, spaceBefore: first() });
      continue;
    }
    let rest = part;
    while (cellWidth(rest) > width) {
      const piece = fitCellText(rest, width);
      if (piece === "") break;
      atoms.push({ text: piece, spaceBefore: first() });
      rest = rest.slice(piece.length);
    }
    if (rest !== "") {
      atoms.push({ text: rest, spaceBefore: first() });
    }
  }
  return atoms;
}

export function wrapCellText(text: string, width: number): string[] {
  if (width <= 0 || cellWidth(text) <= width) return [text];
  const atoms: WrapAtom[] = [];
  text.split(" ").forEach((word, index) => {
    atoms.push(...atomsForWord(word, width, index > 0));
  });
  const lines: string[] = [];
  let current = "";
  for (const atom of atoms) {
    const candidate =
      current === "" ? atom.text : current + (atom.spaceBefore ? " " : "") + atom.text;
    if (current === "" || cellWidth(candidate) <= width) {
      current = candidate;
      continue;
    }
    lines.push(current);
    current = atom.text;
  }
  if (current !== "") {
    lines.push(current);
  }
  return lines;
}

const ESC_CHAR = String.fromCharCode(27);
const BEL_CHAR = String.fromCharCode(7);
const TAB_CHAR = String.fromCharCode(9);
const ANSI_PATTERN = new RegExp(
  `${ESC_CHAR}\\[[0-9;?]*[A-Za-z]` + `|${ESC_CHAR}\\][^]*?(?:${BEL_CHAR}|${ESC_CHAR}\\\\)`,
  "g",
);

/**
 * Remove row-breaking and cursor-moving input from a cell run: ANSI escape
 * sequences, carriage returns, newlines, and other controls. Tabs become two
 * spaces. Plain text (including wide glyphs) passes through untouched.
 */
export function sanitizeCellText(text: string): string {
  const spaced = text.split(TAB_CHAR).join("  ");
  const unescaped = spaced.replace(ANSI_PATTERN, "");
  let out = "";
  for (const char of unescaped) {
    const codePoint = char.codePointAt(0) ?? 0;
    if (isNonPrinting(codePoint)) continue;
    out += char;
  }
  return out;
}

/**
 * Clip or pad one ASCII-preview cell to an exact width. This preserves the
 * legacy code-unit semantics (padEnd then slice); it is a consolidation
 * point, not a Unicode-width upgrade.
 */
export function clipCell(text: string, width: number): string {
  return text.padEnd(width).slice(0, width);
}

/**
 * Single truth for the legacy ASCII preview grid widths (preview.ts).
 * Values reproduce the settled T00A frames exactly, so extraction changes no
 * rendered byte. The normative OpenTUI column allocation stays in
 * calculateColumns above; this table governs only the ASCII preview frames.
 */
export const PREVIEW_FULL_COLUMNS = Object.freeze({
  runHeader: 118,
  nav: 26,
  primary: 42,
  inspector: 41,
  note: 110,
});

/** Compact-frame widths for the legacy ASCII preview (see above). */
export const PREVIEW_COMPACT_COLUMNS = Object.freeze({
  unit: 70,
  ticket: 58,
  fieldId: 54,
  fieldStatus: 52,
});
