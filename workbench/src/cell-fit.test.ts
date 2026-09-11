import { describe, expect, it } from "bun:test";
import { createTestRenderer } from "@opentui/core/testing";
import {
  cellWidth,
  fitCellText,
  sanitizeCellText,
} from "./geometry";
import { TerminalShellRenderer } from "./terminal-renderer";
import {
  FULL_DIMS,
  modelWiredSession,
  renderOptions,
  seededModel,
  unitEvent,
} from "./testing/shell-harness";
import { FocusRef } from "./focus";

/** ESC as code, so no raw escape byte ever lives in test sources. */
const ESC = String.fromCharCode(27);

describe("cell-width fitting (layout: columns never leave the window)", () => {
  it("measures CJK and emoji as two cells, combining marks as zero", () => {
    expect(cellWidth("hello")).toBe(5);
    expect(cellWidth("界")).toBe(2);
    expect(cellWidth("🙂")).toBe(2);
    expect(cellWidth("é".normalize("NFD"))).toBe(1);
    expect(cellWidth("a界b🙂c")).toBe(1 + 2 + 1 + 2 + 1);
  });

  it("fitCellText clips to whole cells and never splits a wide glyph", () => {
    expect(fitCellText("abcdef", 4)).toBe("abcd");
    expect(fitCellText("ab界c", 4)).toBe("ab界");
    expect(fitCellText("ab界c", 3)).toBe("ab");
    expect(fitCellText("界", 1)).toBe("");
    expect(fitCellText("short", 80)).toBe("short");
  });

  it("sanitizeCellText removes row-breaking and cursor-moving input", () => {
    expect(sanitizeCellText("a\nb")).toBe("ab");
    expect(sanitizeCellText("a\tb")).toBe("a  b");
    expect(sanitizeCellText(`a${ESC}[31mb`)).toBe("ab");
    expect(sanitizeCellText("plain")).toBe("plain");
  });

  it("a CJK-heavy snapshot fits every row inside 120 terminal cells", async () => {
    const terminal = await createTestRenderer({ width: 120, height: 40 });
    const view = new TerminalShellRenderer(terminal.renderer);
    const model = seededModel();
    model.applyEvent(unitEvent("u-cjk", "界".repeat(60), "c-99", "active"));
    model.selectFocus(FocusRef.for("gauntlet", "WorkItem", "u-cjk"));
    const session = modelWiredSession(model, FULL_DIMS);
    try {
      view.render(session.render(model.getSnapshot(), renderOptions(model)));
      await terminal.renderOnce();
      const rows = terminal.captureCharFrame().split("\n");
      expect(rows.length).toBe(41);
      for (const row of rows.slice(0, 40)) {
        expect(cellWidth(row)).toBeLessThanOrEqual(120);
      }
      // The focus marker keeps priority: it survives fitting.
      expect(rows.join("\n")).toContain("▸");
    } finally {
      view.destroy();
      terminal.renderer.destroy();
      model.close();
    }
  });
});
