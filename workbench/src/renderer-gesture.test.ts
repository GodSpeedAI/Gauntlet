import { expect, test } from "bun:test";
import { createMockMouse, createTestRenderer } from "@opentui/core/testing";
import type { PointerGesture } from "./shell";
import { TerminalShellRenderer } from "./terminal-renderer";
import { FULL_DIMS, modelWiredSession, renderOptions, seededModel } from "./testing/shell-harness";

/**
 * GUI-redesign increment 3 (renderer side): native mouse events over row
 * renderables synthesize click/double_click/wheel gestures resolved against
 * the last hit map (ICM §35). The shell's pure handlePointer semantics are
 * pinned in shell-pointer.test.ts; this file pins the event→gesture plumbing.
 */

test("click on a Navigator row emits a click gesture with the row's line id", async () => {
  const terminal = await createTestRenderer({ width: 120, height: 40 });
  const gestures: PointerGesture[] = [];
  const view = new TerminalShellRenderer(terminal.renderer, {
    onGesture: (gesture) => gestures.push(gesture),
  });
  const model = seededModel();
  const session = modelWiredSession(model, FULL_DIMS);
  try {
    view.render(session.render(model.getSnapshot(), renderOptions(model)));
    await terminal.renderOnce();

    const hit = view.hitMap().regions.find((region) => region.surfaceId === "Navigator");
    expect(hit).toBeDefined();
    const row = hit?.rows.findIndex((id) => id !== null) ?? -1;
    expect(row).toBeGreaterThanOrEqual(0);
    const mouse = createMockMouse(terminal.renderer);
    await mouse.click((hit?.x ?? 0) + 2, (hit?.y ?? 0) + row);

    const clicks = gestures.filter(
      (g): g is Extract<PointerGesture, { kind: "click" | "double_click" }> =>
        g.kind === "click",
    );
    expect(clicks.length).toBeGreaterThanOrEqual(1);
    const last = clicks.at(-1);
    if (last !== undefined) {
      expect(last.surfaceId).toBe("Navigator");
      expect(last.lineId).toBe(hit?.rows[row] ?? null);
    }
  } finally {
    view.destroy();
    terminal.renderer.destroy();
  }
});

test("doubleClick emits a double_click gesture", async () => {
  const terminal = await createTestRenderer({ width: 120, height: 40 });
  const gestures: PointerGesture[] = [];
  const view = new TerminalShellRenderer(terminal.renderer, {
    onGesture: (gesture) => gestures.push(gesture),
  });
  const model = seededModel();
  const session = modelWiredSession(model, FULL_DIMS);
  try {
    view.render(session.render(model.getSnapshot(), renderOptions(model)));
    await terminal.renderOnce();
    const hit = view.hitMap().regions.find((region) => region.surfaceId === "Primary");
    expect(hit).toBeDefined();
    const mouse = createMockMouse(terminal.renderer);
    await mouse.doubleClick((hit?.x ?? 0) + 4, hit?.y ?? 0);

    const doubles = gestures.filter((g) => g.kind === "double_click");
    expect(doubles.length).toBe(1);
  } finally {
    view.destroy();
    terminal.renderer.destroy();
  }
});

test("scroll over a region emits a wheel gesture with the surface id", async () => {
  const terminal = await createTestRenderer({ width: 120, height: 40 });
  const gestures: PointerGesture[] = [];
  const view = new TerminalShellRenderer(terminal.renderer, {
    onGesture: (gesture) => gestures.push(gesture),
  });
  const model = seededModel();
  const session = modelWiredSession(model, FULL_DIMS);
  try {
    view.render(session.render(model.getSnapshot(), renderOptions(model)));
    await terminal.renderOnce();
    const hit = view.hitMap().regions.find((region) => region.surfaceId === "Primary");
    const mouse = createMockMouse(terminal.renderer);
    await mouse.scroll((hit?.x ?? 0) + 4, (hit?.y ?? 0) + 2, "down");

    const wheels = gestures.filter(
      (g): g is Extract<PointerGesture, { kind: "wheel" }> => g.kind === "wheel",
    );
    expect(wheels.length).toBeGreaterThanOrEqual(1);
    const last = wheels.at(-1);
    if (last !== undefined) {
      expect(last.surfaceId).toBe("Primary");
      expect(last.delta).toBeGreaterThan(0);
    }
  } finally {
    view.destroy();
    terminal.renderer.destroy();
  }
});

test("no onGesture callback: mouse events are simply not forwarded (no crash)", async () => {
  const terminal = await createTestRenderer({ width: 120, height: 40 });
  const view = new TerminalShellRenderer(terminal.renderer);
  const model = seededModel();
  const session = modelWiredSession(model, FULL_DIMS);
  try {
    view.render(session.render(model.getSnapshot(), renderOptions(model)));
    await terminal.renderOnce();
    const hit = view.hitMap().regions.find((region) => region.surfaceId === "Primary");
    const mouse = createMockMouse(terminal.renderer);
    await mouse.click((hit?.x ?? 0) + 4, (hit?.y ?? 0) + 1);
    expect(true).toBe(true);
  } finally {
    view.destroy();
    terminal.renderer.destroy();
  }
});
