import { expect, test } from "bun:test";
import { createMockMouse, createTestRenderer, MouseButtons } from "@opentui/core/testing";
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
      (g): g is Extract<PointerGesture, { kind: "click" | "double_click" }> => g.kind === "click",
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

test("click on an open palette row (overlapping Navigator's cells) resolves to the palette, not the surface underneath it", async () => {
  const terminal = await createTestRenderer({ width: 120, height: 40 });
  const gestures: PointerGesture[] = [];
  const view = new TerminalShellRenderer(terminal.renderer, {
    onGesture: (gesture) => gestures.push(gesture),
  });
  const model = seededModel();
  const session = modelWiredSession(model, FULL_DIMS);
  try {
    // Open the palette (default "p" binding) before rendering: the overlay
    // paints on top of the base columns at overlapping cells (x=2,y=2), so a
    // click there must resolve to "Overlay", never to whatever background
    // surface (e.g. Navigator) occupies those same cells underneath it.
    session.key("p", model.getSnapshot(), renderOptions(model));
    view.render(session.render(model.getSnapshot(), renderOptions(model)));
    await terminal.renderOnce();

    const overlayHit = view.hitMap().regions.find((region) => region.surfaceId === "Overlay");
    expect(overlayHit).toBeDefined();
    const navigatorHit = view.hitMap().regions.find((region) => region.surfaceId === "Navigator");
    expect(navigatorHit).toBeDefined();
    // Sanity: the two regions genuinely overlap at the click point, or this
    // test would not exercise the bug at all.
    const clickX = (overlayHit?.x ?? 0) + 2;
    const clickY = overlayHit?.y ?? 0;
    expect(clickX).toBeGreaterThanOrEqual(navigatorHit?.x ?? 0);
    expect(clickX).toBeLessThan((navigatorHit?.x ?? 0) + (navigatorHit?.width ?? 0));

    const mouse = createMockMouse(terminal.renderer);
    await mouse.click(clickX, clickY);

    const clicks = gestures.filter(
      (g): g is Extract<PointerGesture, { kind: "click" | "double_click" }> => g.kind === "click",
    );
    expect(clicks.length).toBeGreaterThanOrEqual(1);
    expect(clicks.at(-1)?.surfaceId).toBe("Overlay");
  } finally {
    view.destroy();
    terminal.renderer.destroy();
  }
});

test("click on the header emits a click gesture on shell.header (breadcrumb hoisting entry point)", async () => {
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

    const hit = view.hitMap().regions.find((region) => region.surfaceId === "header");
    expect(hit).toBeDefined();
    const mouse = createMockMouse(terminal.renderer);
    await mouse.click((hit?.x ?? 0) + 5, hit?.y ?? 0);

    const clicks = gestures.filter(
      (g): g is Extract<PointerGesture, { kind: "click" | "double_click" }> => g.kind === "click",
    );
    expect(clicks.length).toBeGreaterThanOrEqual(1);
    const last = clicks.at(-1);
    expect(last?.surfaceId).toBe("header");
    expect(last?.lineId).toBe("shell.header");
  } finally {
    view.destroy();
    terminal.renderer.destroy();
  }
});

test("click on the MenuBar title strip emits a click gesture on shell.menubar (UESC §22.7)", async () => {
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

    const hit = view.hitMap().regions.find((region) => region.surfaceId === "menubar");
    expect(hit).toBeDefined();
    const mouse = createMockMouse(terminal.renderer);
    await mouse.click((hit?.x ?? 0) + 1, hit?.y ?? 0);

    const clicks = gestures.filter(
      (g): g is Extract<PointerGesture, { kind: "click" | "double_click" }> => g.kind === "click",
    );
    expect(clicks.length).toBeGreaterThanOrEqual(1);
    const last = clicks.at(-1);
    expect(last?.surfaceId).toBe("menubar");
    expect(last?.lineId).toBe("shell.menubar");
  } finally {
    view.destroy();
    terminal.renderer.destroy();
  }
});

test("a real mouse click on an open MenuBar dropdown row resolves to its menubar.<index> line id", async () => {
  const terminal = await createTestRenderer({ width: 120, height: 40 });
  const gestures: PointerGesture[] = [];
  const view = new TerminalShellRenderer(terminal.renderer, {
    onGesture: (gesture) => gestures.push(gesture),
  });
  const model = seededModel();
  const session = modelWiredSession(model, FULL_DIMS);
  try {
    // Open the "File" menu directly through the shell reducer (mouse-only
    // entry point — there is no keyboard shortcut to open a title, UESC
    // §22.7 CLOSED -> click -> MENU_OPEN), then render the dropdown so the
    // real hit map carries its rows.
    session.pointer(
      { kind: "click", surfaceId: "menubar", lineId: "shell.menubar", x: 1 },
      model.getSnapshot(),
    );
    view.render(session.render(model.getSnapshot(), renderOptions(model)));
    await terminal.renderOnce();

    const overlayHit = view.hitMap().regions.find((region) => region.surfaceId === "Overlay");
    expect(overlayHit).toBeDefined();
    const row = overlayHit?.rows.findIndex((id) => id !== null) ?? -1;
    expect(row).toBeGreaterThanOrEqual(0);
    const mouse = createMockMouse(terminal.renderer);
    await mouse.click((overlayHit?.x ?? 0) + 2, (overlayHit?.y ?? 0) + row);

    const clicks = gestures.filter(
      (g): g is Extract<PointerGesture, { kind: "click" | "double_click" }> => g.kind === "click",
    );
    expect(clicks.length).toBeGreaterThanOrEqual(1);
    const last = clicks.at(-1);
    expect(last?.surfaceId).toBe("Overlay");
    expect(last?.lineId).toBe(overlayHit?.rows[row] ?? null);
    expect(last?.lineId).toMatch(/^menubar\.\d+$/);
  } finally {
    view.destroy();
    terminal.renderer.destroy();
  }
});

test("right-click on a Navigator row emits a right_click gesture with the row's line id (§35.3 context menu, increment 5)", async () => {
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
    await mouse.click((hit?.x ?? 0) + 2, (hit?.y ?? 0) + row, MouseButtons.RIGHT);

    const rightClicks = gestures.filter(
      (g): g is Extract<PointerGesture, { kind: "right_click" }> => g.kind === "right_click",
    );
    expect(rightClicks.length).toBeGreaterThanOrEqual(1);
    const last = rightClicks.at(-1);
    expect(last?.surfaceId).toBe("Navigator");
    expect(last?.lineId).toBe(hit?.rows[row] ?? null);
    // A left click at the exact same cell must NOT also have produced a
    // right_click — the button debounce state must not cross-contaminate.
    expect(gestures.some((g) => g.kind === "click")).toBe(false);
  } finally {
    view.destroy();
    terminal.renderer.destroy();
  }
});

test("a real mouse click on an open context-menu row resolves to its contextmenu.<index> line id (regression: overlay border box was swallowing these clicks)", async () => {
  const terminal = await createTestRenderer({ width: 120, height: 40 });
  const gestures: PointerGesture[] = [];
  const view = new TerminalShellRenderer(terminal.renderer, {
    onGesture: (gesture) => gestures.push(gesture),
  });
  const model = seededModel();
  const session = modelWiredSession(model, FULL_DIMS);
  try {
    const outlineRow = { surfaceId: "Navigator", lineId: "outline.unit.u-far" };
    session.pointer(
      {
        kind: "right_click",
        surfaceId: outlineRow.surfaceId,
        lineId: outlineRow.lineId,
        x: 1,
        y: 2,
      },
      model.getSnapshot(),
    );
    view.render(session.render(model.getSnapshot(), renderOptions(model)));
    await terminal.renderOnce();

    const overlayHit = view.hitMap().regions.find((region) => region.surfaceId === "Overlay");
    expect(overlayHit).toBeDefined();
    const row =
      overlayHit?.rows.findIndex((id) => id !== null && /^contextmenu\.\d+$/.test(id)) ?? -1;
    expect(row).toBeGreaterThanOrEqual(0);
    const mouse = createMockMouse(terminal.renderer);
    await mouse.click((overlayHit?.x ?? 0) + 2, (overlayHit?.y ?? 0) + row);

    const clicks = gestures.filter(
      (g): g is Extract<PointerGesture, { kind: "click" | "double_click" }> => g.kind === "click",
    );
    expect(clicks.length).toBeGreaterThanOrEqual(1);
    const last = clicks.at(-1);
    expect(last?.surfaceId).toBe("Overlay");
    expect(last?.lineId).toBe(overlayHit?.rows[row] ?? null);
    expect(last?.lineId).toMatch(/^contextmenu\.\d+$/);
  } finally {
    view.destroy();
    terminal.renderer.destroy();
  }
});

/**
 * Column-splitter drag-to-resize (ICM §36.1 presentation drag; UESC §22.11
 * SplitPane). The grab zone is the existing 1-cell inter-column gutter
 * planFrame already reserves as negative space PLUS the one rendered column
 * on either side of it (the gutter itself carries no renderable, so a raw
 * mouse event landing exactly there never reaches rowMouseEvent at all — the
 * two adjacent covered cells are the real hit-zone, confirmed against the
 * actual hit map below rather than assumed).
 */
test("dragging the Navigator|Primary seam emits column_resize_start, one or more _move, then _end", async () => {
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
    const navigatorHit = view.hitMap().regions.find((region) => region.surfaceId === "Navigator");
    expect(navigatorHit).toBeDefined();
    // The rightmost RENDERED column of Navigator — one cell short of the
    // true seam, but a real renderable, so the native down event actually
    // dispatches (see the file-level comment above).
    const grabX = (navigatorHit?.x ?? 0) + (navigatorHit?.width ?? 0) - 1;
    const grabY = navigatorHit?.y ?? 0;

    const mouse = createMockMouse(terminal.renderer);
    await mouse.drag(grabX, grabY, grabX + 20, grabY);

    const starts = gestures.filter(
      (g): g is Extract<PointerGesture, { kind: "column_resize_start" }> =>
        g.kind === "column_resize_start",
    );
    const moves = gestures.filter(
      (g): g is Extract<PointerGesture, { kind: "column_resize_move" }> =>
        g.kind === "column_resize_move",
    );
    const ends = gestures.filter(
      (g): g is Extract<PointerGesture, { kind: "column_resize_end" }> =>
        g.kind === "column_resize_end",
    );
    expect(starts.length).toBe(1);
    expect(starts[0]?.boundary).toBe("navigator");
    expect(moves.length).toBeGreaterThanOrEqual(1);
    expect(moves.every((m) => m.boundary === "navigator")).toBe(true);
    expect(ends.length).toBe(1);
    expect(ends[0]?.boundary).toBe("navigator");
    // A drag that crossed the threshold must never also register as a click
    // — the two gesture families are mutually exclusive for one down/up pair.
    expect(gestures.some((g) => g.kind === "click" || g.kind === "double_click")).toBe(false);
  } finally {
    view.destroy();
    terminal.renderer.destroy();
  }
});

test("a plain click at the Navigator|Primary seam (no drag past the threshold) never emits a resize gesture", async () => {
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
    const navigatorHit = view.hitMap().regions.find((region) => region.surfaceId === "Navigator");
    const grabX = (navigatorHit?.x ?? 0) + (navigatorHit?.width ?? 0) - 1;
    const grabY = navigatorHit?.y ?? 0;

    const mouse = createMockMouse(terminal.renderer);
    await mouse.click(grabX, grabY);

    expect(
      gestures.some(
        (g) =>
          g.kind === "column_resize_start" ||
          g.kind === "column_resize_move" ||
          g.kind === "column_resize_end",
      ),
    ).toBe(false);
    expect(gestures.some((g) => g.kind === "click")).toBe(true);
  } finally {
    view.destroy();
    terminal.renderer.destroy();
  }
});

/**
 * Hover tooltip (UESC §22.6, chrome remainder item 6, final increment).
 * `createMockMouse`'s `moveTo` with no button held emits a raw ANSI "move"
 * event with the SGR no-button motion code (confirmed by reading
 * testing.js's `generateMouseEvent`: `case "move": buttonCode = 32 | 3`) —
 * the exact same encoding a real terminal sends for a plain hover move.
 * OpenTUI's CliRenderer synthesizes real over/out MouseEvents from that (its
 * own `if (!sameElement && (type === "drag" || type === "move"))` hit-target
 * check in chunk-bun-*.js), so `moveTo` genuinely exercises hover, not a
 * simulated approximation — confirmed against the actual source the same
 * way prior increments checked mock-mouse capability before assuming a gap.
 */
test("moving the mouse (no button held) over a Navigator row emits a hover gesture and renders the tooltip in the hit map", async () => {
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
    // A row carrying a resolvable FocusRef, not "navigator.title" (rowless
    // chrome) — outline rows are the "outline.*" ids navigatorOutline emits.
    const row = hit?.rows.findIndex((id) => id?.startsWith("outline.unit.")) ?? -1;
    expect(row).toBeGreaterThanOrEqual(0);
    const mouse = createMockMouse(terminal.renderer);
    await mouse.moveTo((hit?.x ?? 0) + 2, (hit?.y ?? 0) + row);

    const hovers = gestures.filter(
      (g): g is Extract<PointerGesture, { kind: "hover" }> => g.kind === "hover",
    );
    expect(hovers.length).toBeGreaterThanOrEqual(1);
    const last = hovers.at(-1);
    expect(last?.surfaceId).toBe("Navigator");
    expect(last?.lineId).toBe(hit?.rows[row] ?? null);

    // Feed the gesture through the real reducer and re-render: the tooltip
    // must show up in the NEXT frame's RenderedShell, and never register as
    // a hit-map region of its own (a tooltip must never suppress a
    // click/drag on whatever it sits near — see tooltipPlan's own comment).
    if (last !== undefined) {
      session.pointer(last, model.getSnapshot());
    }
    const rendered = session.render(model.getSnapshot(), renderOptions(model));
    expect(rendered.tooltip).not.toBeNull();
    view.render(rendered);
    await terminal.renderOnce();
    expect(view.hitMap().regions.some((region) => region.surfaceId === "Tooltip")).toBe(false);
  } finally {
    view.destroy();
    terminal.renderer.destroy();
  }
});

test("a click on the row occluded by an open tooltip still registers (regression: the tooltip's own renderable was swallowing it)", async () => {
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

    const navigatorHit = view.hitMap().regions.find((region) => region.surfaceId === "Navigator");
    expect(navigatorHit).toBeDefined();
    const rows = navigatorHit?.rows ?? [];
    let hoverRow = -1;
    for (let i = 0; i < rows.length - 1; i += 1) {
      if (rows[i]?.startsWith("outline.unit.") === true && rows[i + 1] !== null) {
        hoverRow = i;
        break;
      }
    }
    expect(hoverRow).toBeGreaterThanOrEqual(0);
    const mouse = createMockMouse(terminal.renderer);
    await mouse.moveTo((navigatorHit?.x ?? 0) + 2, (navigatorHit?.y ?? 0) + hoverRow);

    const hovers = gestures.filter(
      (g): g is Extract<PointerGesture, { kind: "hover" }> => g.kind === "hover",
    );
    expect(hovers.length).toBeGreaterThanOrEqual(1);
    for (const hover of hovers) session.pointer(hover, model.getSnapshot());
    const rendered = session.render(model.getSnapshot(), renderOptions(model));
    expect(rendered.tooltip).not.toBeNull();
    view.render(rendered);
    await terminal.renderOnce();

    // tooltipPlan places the tooltip one row below the hovered cell — click
    // that exact cell (a real, different Navigator row) while it's showing.
    gestures.length = 0;
    await mouse.click((navigatorHit?.x ?? 0) + 2, (navigatorHit?.y ?? 0) + hoverRow + 1);
    const clicks = gestures.filter(
      (g): g is Extract<PointerGesture, { kind: "click" | "double_click" }> => g.kind === "click",
    );
    expect(clicks.length).toBeGreaterThanOrEqual(1);
    expect(clicks.at(-1)?.surfaceId).toBe("Navigator");
  } finally {
    view.destroy();
    terminal.renderer.destroy();
  }
});

test("moving the mouse off the hovered row (a subsequent move elsewhere) clears the tooltip", async () => {
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

    const navigatorHit = view.hitMap().regions.find((region) => region.surfaceId === "Navigator");
    const composerHit = view.hitMap().regions.find((region) => region.surfaceId === "Composer");
    expect(navigatorHit).toBeDefined();
    expect(composerHit).toBeDefined();
    const row = navigatorHit?.rows.findIndex((id) => id?.startsWith("outline.unit.")) ?? -1;
    const mouse = createMockMouse(terminal.renderer);
    await mouse.moveTo((navigatorHit?.x ?? 0) + 2, (navigatorHit?.y ?? 0) + row);
    await mouse.moveTo((composerHit?.x ?? 0) + 1, composerHit?.y ?? 0);

    const hovers = gestures.filter(
      (g): g is Extract<PointerGesture, { kind: "hover" }> => g.kind === "hover",
    );
    expect(hovers.length).toBeGreaterThanOrEqual(2);
    // The renderer forwards whatever row it hit-tested (composer.mode has no
    // FocusRef); resolving "no ref here" is the pure reducer's job, not the
    // renderer's — so feed both gestures through the real reducer and assert
    // the RESULTING state, the same layering every other gesture uses here.
    for (const hover of hovers) {
      session.pointer(hover, model.getSnapshot());
    }
    const state = session.render(model.getSnapshot(), renderOptions(model));
    expect(state.tooltip).toBeNull();
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
