import { describe, expect, it } from "bun:test";
import { clipCell, PREVIEW_COMPACT_COLUMNS, PREVIEW_FULL_COLUMNS } from "./geometry";

describe("preview grid extraction (single layout truth)", () => {
  it("clipCell pads short cells and truncates long ones like the legacy inline code", () => {
    expect(clipCell("ab", 5)).toBe("ab   ");
    expect(clipCell("abcdef", 4)).toBe("abcd");
    expect(clipCell("", 3)).toBe("   ");
    expect(clipCell("exact", 5)).toBe("exact");
  });

  it("full-frame widths reproduce the settled T00A preview literals", () => {
    expect(PREVIEW_FULL_COLUMNS).toEqual({
      runHeader: 118,
      nav: 26,
      primary: 42,
      inspector: 41,
      note: 110,
    });
  });

  it("compact-frame widths reproduce the settled T00A preview literals", () => {
    expect(PREVIEW_COMPACT_COLUMNS).toEqual({
      unit: 70,
      ticket: 58,
      fieldId: 54,
      fieldStatus: 52,
    });
  });
});
