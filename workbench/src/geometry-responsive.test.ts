import { describe, expect, it } from "bun:test";
import {
  calculateColumns,
  classifyGeometry,
  collapseOrder,
  heightBand,
  renderBelowMinimumPrompt,
} from "./geometry";

/**
 * Increment 7 responsive-geometry honesty suites (DESIGN.md "Responsive
 * Behavior": Width classes ~1469-1526, Height constraints ~1528-1539).
 *
 * Repairs recon violation V1 (.agents/reports/tui-gui-redesign/recon/
 * shell-render-opentui.md §3, §8.3): classifyGeometry gated Full/Wide on
 * rows>=40, so 120-139 cols at <40 rows wrongly collapsed to single-pane
 * Compact. DESIGN Full (line 1496): "Below 40 rows at this width, collapse
 * the Bottom Surface first (see Height constraints), keeping all three
 * columns." Columns are never dropped for height at >=120 cols; the height
 * ladder (DESIGN 1530) degrades surfaces in order — Bottom Surface, then
 * nonessential metadata, then Navigator/Inspector to overlay — while
 * preserving Focus, View, Horizon/location, attention cue, composer/required
 * input, and consequential status.
 *
 * Floors stay exactly DSN-007 / REQ-UX-005: full layout at 120x40, one
 * primary pane at 80x24, recoverable prompt below 80x24.
 */

describe("increment 7: width classes keep columns at short heights (DESIGN Full, line 1496)", () => {
  it("classifies 130x30 as Full — three columns survive rows<40, only the Bottom Surface may collapse", () => {
    const dims = { cols: 130, rows: 30 };
    expect(classifyGeometry(dims)).toBe("Full");
    expect(heightBand(dims)).toBe("short");

    const columns = calculateColumns(dims);
    expect(columns.isMultiColumn).toBe(true);
    expect(columns.navigatorWidth).toBe(28);
    expect(columns.inspectorWidth).toBe(36);
    expect(columns.primaryWidth).toBe(66); // max(36, 130 - 28 - 36)
  });

  it("classifies 139x39 as Full — the whole 120-139 band keeps columns down to the 24-row floor", () => {
    const dims = { cols: 139, rows: 39 };
    expect(classifyGeometry(dims)).toBe("Full");
    expect(heightBand(dims)).toBe("short");

    const columns = calculateColumns(dims);
    expect(columns.isMultiColumn).toBe(true);
    expect(columns.navigatorWidth).toBe(28);
    expect(columns.inspectorWidth).toBe(36);
    expect(columns.primaryWidth).toBe(75); // max(36, 139 - 28 - 36)
  });

  it("classifies 200x39 as Wide — Wide renders no surface Full lacks, so it shares the no-column-drop rule (DESIGN Wide, lines 1473-1485)", () => {
    const dims = { cols: 200, rows: 39 };
    expect(classifyGeometry(dims)).toBe("Wide");
    expect(heightBand(dims)).toBe("short");

    const columns = calculateColumns(dims);
    expect(columns.isMultiColumn).toBe(true);
    expect(columns.navigatorWidth).toBe(32);
    expect(columns.inspectorWidth).toBe(44);
    expect(columns.primaryWidth).toBe(124); // max(40, 200 - 32 - 44)
  });

  it("width bands are keyed on columns alone above the 24-row floor: 119x30 Compact, 120x30 Full, 139x30 Full, 140x30 Wide", () => {
    expect(classifyGeometry({ cols: 119, rows: 30 })).toBe("Compact");
    expect(classifyGeometry({ cols: 120, rows: 30 })).toBe("Full");
    expect(classifyGeometry({ cols: 139, rows: 30 })).toBe("Full");
    expect(classifyGeometry({ cols: 140, rows: 30 })).toBe("Wide");
  });

  it("Compact at short height stays a single primary pane — Compact degrades columns for width, not height (DESIGN Compact, lines 1499-1513)", () => {
    const dims = { cols: 100, rows: 30 };
    expect(classifyGeometry(dims)).toBe("Compact");
    expect(heightBand(dims)).toBe("short");

    const columns = calculateColumns(dims);
    expect(columns.isMultiColumn).toBe(false);
    expect(columns.navigatorWidth).toBe(0);
    expect(columns.inspectorWidth).toBe(0);
    expect(columns.primaryWidth).toBe(100);
  });
});

describe("increment 7: height bands and the collapse ladder (DESIGN Height constraints, lines 1528-1539)", () => {
  it("bands at the 40-row boundary: 39 rows is short, 40 rows is tall", () => {
    expect(heightBand({ cols: 130, rows: 39 })).toBe("short");
    expect(heightBand({ cols: 130, rows: 40 })).toBe("tall");
    expect(heightBand({ cols: 80, rows: 24 })).toBe("short");
  });

  it("documents the ladder as data the shell consumes: bottom collapses first, then metadata, then drawers to overlay", () => {
    // DESIGN 1530: "collapse in this order — Bottom Surface first, then
    // nonessential metadata, then Navigator/Inspector to overlay" — while
    // always preserving Focus, View, Horizon/location, attention cue,
    // composer/required approval input, and consequential status.
    expect(collapseOrder("short")).toEqual(["bottom", "metadata", "drawers"]);
    expect(collapseOrder("tall")).toEqual([]);
  });

  it("the ladder never reclassifies geometry: Full/short and Wide/short keep their class and columns", () => {
    // Collapsing surfaces is a shell rendering concern driven by the band;
    // the class and the three-column allocation stay stable so a later
    // growth back to tall restores the full layout without recomputation.
    for (const dims of [
      { cols: 130, rows: 30 },
      { cols: 200, rows: 30 },
    ]) {
      const before = classifyGeometry(dims);
      expect(collapseOrder(heightBand(dims)).length).toBeGreaterThan(0);
      expect(classifyGeometry(dims)).toBe(before);
      expect(calculateColumns(dims).isMultiColumn).toBe(true);
    }
  });
});

describe("increment 7: DSN-007 / REQ-UX-005 floors are unchanged", () => {
  it("120x40 remains the Full/tall normative floor with three columns", () => {
    const dims = { cols: 120, rows: 40 };
    expect(classifyGeometry(dims)).toBe("Full");
    expect(heightBand(dims)).toBe("tall");
    expect(calculateColumns(dims).isMultiColumn).toBe(true);
  });

  it("80x24 remains Compact: one primary pane", () => {
    const dims = { cols: 80, rows: 24 };
    expect(classifyGeometry(dims)).toBe("Compact");
    const columns = calculateColumns(dims);
    expect(columns.isMultiColumn).toBe(false);
    expect(columns.primaryWidth).toBe(80);
  });

  it("below 80x24 the recoverable compact-size prompt applies regardless of columns", () => {
    expect(classifyGeometry({ cols: 72, rows: 20 })).toBe("BelowMinimum");
    expect(classifyGeometry({ cols: 79, rows: 30 })).toBe("BelowMinimum");
    // Rows below 24 gate even wide terminals: no degraded workbench (DESIGN 1539).
    expect(classifyGeometry({ cols: 120, rows: 23 })).toBe("BelowMinimum");
    expect(classifyGeometry({ cols: 140, rows: 23 })).toBe("BelowMinimum");

    const prompt = renderBelowMinimumPrompt({ cols: 72, rows: 20 });
    expect(prompt).toContain("Gauntlet needs at least 80x24. This terminal is 72x20.");
    expect(prompt).toContain("press q to quit");
  });
});

describe("column-splitter drag-to-resize (ICM §36.1, UESC §22.11 SplitPane)", () => {
  it("an override within range replaces only the width(s) it names", () => {
    const dims = { cols: 120, rows: 40 }; // Full: default nav 28, inspector 36
    const columns = calculateColumns(dims, { navigatorWidth: 34, inspectorWidth: 36 });
    expect(columns.navigatorWidth).toBe(34);
    expect(columns.inspectorWidth).toBe(36);
    expect(columns.primaryWidth).toBe(50); // 120 - 34 - 36
  });

  it("clamps a dragged-to-zero navigator to its floor rather than collapsing Primary", () => {
    const dims = { cols: 120, rows: 40 };
    const columns = calculateColumns(dims, { navigatorWidth: 0, inspectorWidth: 36 });
    expect(columns.navigatorWidth).toBe(20); // MIN_NAVIGATOR_WIDTH
    expect(columns.primaryWidth).toBeGreaterThanOrEqual(36);
  });

  it("clamps a dragged-to-zero inspector to its floor rather than collapsing Primary", () => {
    const dims = { cols: 120, rows: 40 };
    const columns = calculateColumns(dims, { navigatorWidth: 28, inspectorWidth: 0 });
    expect(columns.inspectorWidth).toBe(24); // MIN_INSPECTOR_WIDTH
    expect(columns.primaryWidth).toBeGreaterThanOrEqual(36);
  });

  it("clamps an absurdly wide navigator so Primary and Inspector never go negative or below their floors", () => {
    const dims = { cols: 120, rows: 40 };
    const columns = calculateColumns(dims, { navigatorWidth: 10_000, inspectorWidth: 36 });
    expect(columns.primaryWidth).toBeGreaterThanOrEqual(36);
    expect(columns.inspectorWidth).toBeGreaterThanOrEqual(24);
    expect(columns.navigatorWidth + columns.primaryWidth + columns.inspectorWidth).toBe(120);
  });

  it("clamps simultaneous extreme overrides on both sides — the sum never exceeds the terminal width", () => {
    const dims = { cols: 120, rows: 40 };
    const columns = calculateColumns(dims, { navigatorWidth: 10_000, inspectorWidth: 10_000 });
    expect(columns.navigatorWidth + columns.primaryWidth + columns.inspectorWidth).toBe(120);
    expect(columns.primaryWidth).toBeGreaterThanOrEqual(36);
  });

  it("Compact ignores an override entirely — it has no Navigator/Inspector columns to resize", () => {
    const dims = { cols: 100, rows: 30 };
    const columns = calculateColumns(dims, { navigatorWidth: 40, inspectorWidth: 40 });
    expect(columns.isMultiColumn).toBe(false);
    expect(columns.navigatorWidth).toBe(0);
    expect(columns.inspectorWidth).toBe(0);
    expect(columns.primaryWidth).toBe(100);
  });

  it("omitting the override reproduces the exact unoverridden defaults (backward compatible)", () => {
    const dims = { cols: 200, rows: 39 }; // Wide
    expect(calculateColumns(dims, null)).toEqual(calculateColumns(dims));
    expect(calculateColumns(dims, undefined)).toEqual(calculateColumns(dims));
  });
});
