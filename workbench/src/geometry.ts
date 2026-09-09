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

/**
 * Classifies terminal dimensions into normative geometry classes.
 * Boundaries are fixed by spec REQ-UX-005 and are not design-tunable.
 */
export function classifyGeometry(dims: Dimensions): GeometryClass {
  if (dims.cols < MIN_COLS || dims.rows < MIN_ROWS) {
    return "BelowMinimum";
  }
  if (dims.cols >= WIDE_MIN_COLS && dims.rows >= FULL_MIN_ROWS) {
    return "Wide";
  }
  if (dims.cols >= FULL_MIN_COLS && dims.rows >= FULL_MIN_ROWS) {
    return "Full";
  }
  return "Compact";
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
    "Resize the window, or press q to quit.  (admitted work keeps running)",
  ].join("\n");
}
