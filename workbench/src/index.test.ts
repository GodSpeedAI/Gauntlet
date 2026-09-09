import { expect, test } from "bun:test";
import {
  classifyGeometry,
  createInitialState,
  GauntletReadClient,
  renderBelowMinimumPrompt,
  renderPreview,
} from "./index";

test("workbench exports are importable and functional", () => {
  expect(typeof GauntletReadClient).toBe("function");
  expect(classifyGeometry({ cols: 120, rows: 40 })).toBe("Full");
  expect(renderBelowMinimumPrompt({ cols: 70, rows: 20 })).toContain("70x20");
  const state = createInitialState();
  expect(renderPreview(state)).toContain("DISCONNECTED");
});
