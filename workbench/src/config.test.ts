import { describe, expect, it } from "bun:test";
import { resolveConfig } from "./config";

/**
 * GUI-redesign increment 2a: the motion / colour-capability env surface
 * (GAUNTLET_WORKBENCH_MOTION, GAUNTLET_WORKBENCH_COLOR) is pinned here as
 * documented config fields (REQ-CONFIG-001 documented-fields-only; DSN-013
 * M15 fix; DSN-002 V6 fix). Invalid values are ignored, never guessed — the
 * theme layer owns detection and defaults.
 */
describe("increment 2a: motion and colour capability config surface", () => {
  it("maps GAUNTLET_WORKBENCH_MOTION to the typed motion field", () => {
    expect(resolveConfig({ env: { GAUNTLET_WORKBENCH_MOTION: "reduced" } }).motion).toBe("reduced");
    expect(resolveConfig({ env: { GAUNTLET_WORKBENCH_MOTION: "full" } }).motion).toBe("full");
  });

  it("maps GAUNTLET_WORKBENCH_COLOR to the typed colour field", () => {
    expect(resolveConfig({ env: { GAUNTLET_WORKBENCH_COLOR: "monochrome" } }).color).toBe(
      "monochrome",
    );
    expect(resolveConfig({ env: { GAUNTLET_WORKBENCH_COLOR: "color256" } }).color).toBe("color256");
    expect(resolveConfig({ env: { GAUNTLET_WORKBENCH_COLOR: "truecolor" } }).color).toBe(
      "truecolor",
    );
  });

  it("unset or invalid values stay undefined instead of guessing", () => {
    expect(resolveConfig({ env: {} }).motion).toBeUndefined();
    expect(resolveConfig({ env: {} }).color).toBeUndefined();
    expect(
      resolveConfig({ env: { GAUNTLET_WORKBENCH_MOTION: "spinning" } }).motion,
    ).toBeUndefined();
    expect(resolveConfig({ env: { GAUNTLET_WORKBENCH_COLOR: "vibrant" } }).color).toBeUndefined();
  });
});
