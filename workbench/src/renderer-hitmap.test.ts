import { expect, test } from "bun:test";
import { createTestRenderer } from "@opentui/core/testing";
import type { RenderedLine } from "./render-tree";
import type { RenderedShell } from "./shell";
import { TerminalShellRenderer } from "./terminal-renderer";
import { FULL_DIMS, modelWiredSession, renderOptions, seededModel } from "./testing/shell-harness";

/**
 * Increment 2c (GUI redesign step 2/3): retained render tree + mouse hit-map
 * foundation. The renderer must (a) reuse renderables across consecutive
 * renders instead of destroy-and-rebuild (recon shell-render-opentui.md R1),
 * (b) keep a stable (region, row) renderable identity, and (c) expose a
 * machine-usable map from placed rows back to span-tree line ids so increment
 * 3 can lower pointer gestures onto semantic rows. Visible output stays
 * byte-identical to the rebuild path (pinned again here against a fresh
 * single-shot build).
 */

/** Append one line to exactly one surface: a single-region tail mutation. */
function withAppendedLine(
  shell: RenderedShell,
  surfaceId: string,
  line: RenderedLine,
): RenderedShell {
  return {
    ...shell,
    surfaces: shell.surfaces.map((surface) =>
      surface.id === surfaceId ? { ...surface, lines: [...surface.lines, line] } : surface,
    ),
  };
}

test("retained tree: a no-change re-render creates zero renderables and is frame-identical", async () => {
  const terminal = await createTestRenderer({ width: 120, height: 40 });
  const view = new TerminalShellRenderer(terminal.renderer);
  const model = seededModel();
  const session = modelWiredSession(model, FULL_DIMS);
  try {
    const shell = session.render(model.getSnapshot(), renderOptions(model));
    view.render(shell);
    await terminal.renderOnce();
    const firstFrame = terminal.captureCharFrame();
    const frameBefore = terminal.renderer.root.getChildren()[0];
    const childrenBefore = frameBefore?.getChildrenCount() ?? -1;

    view.render(shell);
    await terminal.renderOnce();

    expect(view.createdLastRender).toBe(0);
    expect(terminal.captureCharFrame()).toBe(firstFrame);
    // Stable identity: the same frame renderable hosts the same child count.
    expect(terminal.renderer.root.getChildren()[0]).toBe(frameBefore);
    expect(frameBefore?.getChildrenCount()).toBe(childrenBefore);
  } finally {
    view.destroy();
    terminal.renderer.destroy();
    model.close();
  }
});

test("retained tree: a one-line append creates exactly one renderable at the region tail", async () => {
  const terminal = await createTestRenderer({ width: 120, height: 40 });
  const view = new TerminalShellRenderer(terminal.renderer);
  const model = seededModel();
  const session = modelWiredSession(model, FULL_DIMS);
  try {
    const shell = session.render(model.getSnapshot(), renderOptions(model));
    view.render(shell);
    await terminal.renderOnce();
    const appended = withAppendedLine(shell, "Primary", {
      id: "work.unit.u-appended",
      spans: [{ text: "APPENDED-ROW u-appended" }],
    });

    view.render(appended);
    await terminal.renderOnce();

    expect(view.createdLastRender).toBe(1);
    expect(terminal.captureCharFrame()).toContain("APPENDED-ROW u-appended");
    const primary = view.hitMap().regions.find((region) => region.surfaceId === "Primary");
    expect(primary?.rows.at(-1)).toBe("work.unit.u-appended");
  } finally {
    view.destroy();
    terminal.renderer.destroy();
    model.close();
  }
});

test("hit-map derives placed regions with line ids from the span tree", async () => {
  const terminal = await createTestRenderer({ width: 120, height: 40 });
  const view = new TerminalShellRenderer(terminal.renderer);
  const model = seededModel();
  const session = modelWiredSession(model, FULL_DIMS);
  try {
    const shell = session.render(model.getSnapshot(), renderOptions(model));
    view.render(shell);
    await terminal.renderOnce();
    const hit = view.hitMap();

    // The structure is frozen machine input for the gesture layer.
    expect(Object.isFrozen(hit)).toBe(true);
    for (const region of hit.regions) expect(Object.isFrozen(region)).toBe(true);

    const bySurface = new Map(hit.regions.map((region) => [region.surfaceId, region]));
    // Column surfaces keep their allocation and their span-tree line ids.
    const navigator = bySurface.get("Navigator");
    expect(navigator?.x).toBe(0);
    expect(navigator?.width).toBeGreaterThan(0);
    expect(navigator?.rows.length).toBe(navigator?.height);
    const navigatorLines =
      shell.surfaces.find((surface) => surface.id === "Navigator")?.lines ?? [];
    for (const [index, line] of navigatorLines.slice(0, navigator?.height ?? 0).entries()) {
      expect(navigator?.rows[index]).toBe(line.id ?? null);
    }
    const primary = bySurface.get("Primary");
    expect(primary).toBeDefined();
    expect(primary?.rows).toContain("primary.tabs");
    // Chrome regions are addressable too.
    expect(bySurface.get("header")?.rows).toEqual(["shell.header"]);
    expect(bySurface.get("Composer")).toBeDefined();
    expect(hit.regions.some((region) => region.surfaceId === "help")).toBe(true);
    // Placed rows paint where the hit-map says: row ids sit on the y their
    // region claims.
    const frame = terminal.captureCharFrame().split("\n");
    const composer = bySurface.get("Composer");
    if (composer && composer.height > 0 && composer.rows[0]) {
      expect(frame[composer.y + 0]?.slice(composer.x)).toContain("COMPOSER");
    }
  } finally {
    view.destroy();
    terminal.renderer.destroy();
    model.close();
  }
});

test("hit-map exposes the palette overlay region while it is open", async () => {
  const terminal = await createTestRenderer({ width: 120, height: 40 });
  const view = new TerminalShellRenderer(terminal.renderer);
  const model = seededModel();
  const session = modelWiredSession(model, FULL_DIMS);
  try {
    const shell = session.render(model.getSnapshot(), renderOptions(model));
    view.render(shell);
    await terminal.renderOnce();
    expect(view.hitMap().regions.some((region) => region.surfaceId === "Overlay")).toBe(false);

    session.key("p", model.getSnapshot());
    const paletteShell = session.render(model.getSnapshot(), renderOptions(model));
    view.render(paletteShell);
    await terminal.renderOnce();

    const overlay = view.hitMap().regions.find((region) => region.surfaceId === "Overlay");
    expect(overlay).toBeDefined();
    expect(overlay?.x).toBe(2);
    expect(overlay?.rows.length).toBeGreaterThan(0);
    const paletteLines = paletteShell.palette?.lines ?? [];
    expect(overlay?.rows.length).toBeLessThanOrEqual(paletteLines.length);
    for (const [index, id] of overlay?.rows.entries() ?? []) {
      expect(id).toBe(paletteLines[index]?.id ?? null);
    }
  } finally {
    view.destroy();
    terminal.renderer.destroy();
    model.close();
  }
});

test("retained lowering paints the same frame as a fresh full build", async () => {
  const fresh = await createTestRenderer({ width: 120, height: 40 });
  const retained = await createTestRenderer({ width: 120, height: 40 });
  const freshView = new TerminalShellRenderer(fresh.renderer);
  const retainedView = new TerminalShellRenderer(retained.renderer);
  const model = seededModel();
  const session = modelWiredSession(model, FULL_DIMS);
  try {
    // Final state: one appended row, one attention row added, palette open.
    session.key("p", model.getSnapshot());
    const finalShell = session.render(model.getSnapshot(), renderOptions(model));
    const appendedLine: RenderedLine = {
      id: "work.unit.u-final",
      spans: [{ text: "FINAL-ROW" }],
    };
    const mutated = {
      ...withAppendedLine(finalShell, "Primary", appendedLine),
      attention: [
        {
          id: "shell.attention.host",
          spans: [{ text: "! host notice present", token: "attention.required" as const }],
        },
        ...finalShell.attention,
      ],
    };
    freshView.render(mutated);
    await fresh.renderOnce();

    // The retained terminal walks there through intermediate states, including
    // the sharpest retained hazard: a base-row append and a footer shift while
    // the palette overlay is open (a newly appended renderable must not paint
    // over the overlay the way a plain tail append would).
    session.key("escape", model.getSnapshot());
    const initial = session.render(model.getSnapshot(), renderOptions(model));
    retainedView.render(initial);
    await retained.renderOnce();
    session.key("p", model.getSnapshot());
    const paletteShell = session.render(model.getSnapshot(), renderOptions(model));
    retainedView.render(paletteShell);
    await retained.renderOnce();
    retainedView.render(withAppendedLine(paletteShell, "Primary", appendedLine));
    await retained.renderOnce();
    retainedView.render(mutated);
    await retained.renderOnce();

    expect(retained.captureCharFrame()).toBe(fresh.captureCharFrame());
  } finally {
    freshView.destroy();
    retainedView.destroy();
    fresh.renderer.destroy();
    retained.renderer.destroy();
    model.close();
  }
});

test("resize rebuilds the retained frame and destroy clears the hit-map", async () => {
  const terminal = await createTestRenderer({ width: 120, height: 40 });
  const view = new TerminalShellRenderer(terminal.renderer);
  const model = seededModel();
  const session = modelWiredSession(model, FULL_DIMS);
  try {
    view.render(session.render(model.getSnapshot(), renderOptions(model)));
    await terminal.renderOnce();
    const frameBefore = terminal.renderer.root.getChildren()[0];

    terminal.resize(80, 24);
    session.resize({ cols: 80, rows: 24 });
    view.render(session.render(model.getSnapshot(), renderOptions(model)));
    await terminal.renderOnce();

    expect(view.createdLastRender).toBeGreaterThan(0);
    expect(terminal.renderer.root.getChildren()[0]).not.toBe(frameBefore);

    view.destroy();
    expect(view.hitMap().regions).toEqual([]);
  } finally {
    terminal.renderer.destroy();
    model.close();
  }
});
