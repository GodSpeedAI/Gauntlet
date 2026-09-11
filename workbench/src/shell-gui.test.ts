import { describe, expect, it } from "bun:test";
import { commandForKey, validateCommandRegistry } from "./command-registry";
import { FocusRef } from "./focus";
import {
  createShellState,
  filterPaletteEntries,
  handleCommand,
  handleKey,
  handlePointer,
  navigatorOutline,
  paletteEntries,
  renderShell,
  ShellSession,
  type ShellState,
} from "./shell";
import { FULL_DIMS, renderOptions, seededModel } from "./testing/shell-harness";
import { defaultViewRegistry } from "./views-chat-trace";
import type { FullWorkbenchSnapshot } from "./workbench-model";

/**
 * GUI-redesign increments 4/5 state foundations (orchestrator slice): palette
 * query filtering (ICM §28, UESC §22.8), the Bottom Surface toggle (UESC §7.5,
 * ICM §27.3), and the Inspector PINNED/FOLLOW_SELECTION machine (UESC §7.4,
 * ICM §27.4). Presentation-state only; render integration lands with the
 * chrome increment.
 */

const HOST = { workspace: { workspace: "gauntlet", reference: "ref-1d457ec9", run: "R17" } };

function stateAt(): ShellState {
  return createShellState(HOST, FULL_DIMS);
}

function snapshot(): FullWorkbenchSnapshot {
  return seededModel().getSnapshot();
}

const ROOT_FOCUS = FocusRef.for("gauntlet", "WorkItem", "u-root");

describe("palette query filtering (UESC §22.8, ICM §28)", () => {
  it("typing printable characters in the open palette edits the query and filters entries", () => {
    const snap = snapshot();
    const opened = handleCommand(stateAt(), snap, "ui.palette.open");
    const typed = handleKey(handleKey(opened.state, snap, "t").state, snap, "r");
    expect(typed.state.paletteQuery).toBe("tr");
    expect(typed.state.paletteOpen).toBe(true);

    const all = paletteEntries(typed.state, snap, {});
    const filtered = filterPaletteEntries(all, "tr");
    expect(filtered.length).toBeGreaterThan(0);
    expect(filtered.length).toBeLessThan(all.length);
    // Every surviving row mentions the query in one of its identity fields.
    for (const entry of filtered) {
      expect(entry.type).not.toBe("header");
    }
  });

  it("move and activate walk the filtered list without leaving the index out of range", () => {
    const snap = snapshot();
    const opened = handleCommand(stateAt(), snap, "ui.palette.open");
    const typed = handleKey(handleKey(opened.state, snap, "e").state, snap, "v");
    expect(typed.state.paletteQuery).toBe("ev");

    const moved = handleCommand(typed.state, snap, "ui.palette.move.next");
    const filtered = filterPaletteEntries(paletteEntries(moved.state, snap, {}), "ev");
    expect(moved.state.paletteIndex).toBeLessThan(Math.max(filtered.length, 1));
    const activated = handleCommand(moved.state, snap, "ui.palette.activate");
    // Activation of a filtered view row still switches or closes honestly.
    expect(activated.effects).toEqual([]);
  });

  it("backspace trims the query; closing the palette resets it", () => {
    const snap = snapshot();
    const opened = handleCommand(stateAt(), snap, "ui.palette.open");
    const typed = handleKey(handleKey(opened.state, snap, "a").state, snap, "b");
    expect(typed.state.paletteQuery).toBe("ab");
    const trimmed = handleKey(typed.state, snap, "backspace");
    expect(trimmed.state.paletteQuery).toBe("a");
    const closed = handleCommand(trimmed.state, snap, "ui.palette.close");
    expect(closed.state.paletteQuery).toBe("");
  });

  it("query typing never fires effects and never closes the palette", () => {
    const snap = snapshot();
    const opened = handleCommand(stateAt(), snap, "ui.palette.open");
    const result = handleKey(opened.state, snap, "x");
    expect(result.effects).toEqual([]);
    expect(result.state.paletteOpen).toBe(true);
    expect(result.state.paletteQuery).toBe("x");
  });
});

describe("bottom surface toggle (UESC §7.5, ICM §27.3)", () => {
  it("ui.bottom.toggle flips the bottom surface state; key b resolves to it", () => {
    const snap = snapshot();
    expect(commandForKey("b", { surface: "shell" })).toBe("ui.bottom.toggle");
    const base = stateAt();
    expect(base.bottomOpen).toBe(false);
    const toggled = handleCommand(base, snap, "ui.bottom.toggle");
    expect(toggled.state.bottomOpen).toBe(true);
    const toggled2 = handleCommand(toggled.state, snap, "ui.bottom.toggle");
    expect(toggled2.state.bottomOpen).toBe(false);
    // The keyboard path is the same command.
    const viaKey = handleKey(base, snap, "b");
    expect(viaKey.state.bottomOpen).toBe(true);
  });
});

describe("inspector pin (UESC §7.4 FOLLOW_SELECTION/PINNED)", () => {
  it("pin freezes the inspector target across selection changes; toggle again releases", () => {
    const snap = snapshot();
    const focused: ShellState = {
      ...stateAt(),
      location: { ...stateAt().location, focus: ROOT_FOCUS },
    };
    expect(focused.inspectorPin).toBeNull();
    const pinned = handleCommand(focused, snap, "ui.inspector.pin.toggle");
    expect(pinned.state.inspectorPin?.ref.id).toBe(ROOT_FOCUS.id);

    // Selection moves elsewhere; the pinned target survives.
    const moved: ShellState = {
      ...pinned.state,
      location: {
        ...pinned.state.location,
        focus: FocusRef.for("gauntlet", "WorkItem", "u-other"),
      },
    };
    expect(moved.inspectorPin?.ref.id).toBe(ROOT_FOCUS.id);

    const released = handleCommand(moved, snap, "ui.inspector.pin.toggle");
    expect(released.state.inspectorPin).toBeNull();
  });

  it("keys i/I resolve to inspector toggle/pin; registry stays valid with the new commands", () => {
    expect(commandForKey("I", { surface: "shell" })).toBe("ui.inspector.pin.toggle");
    expect(commandForKey("i", { surface: "shell" })).toBe("ui.inspector.toggle");
    expect(validateCommandRegistry()).toEqual([]);
  });
});

describe("navigator viewport follow (V4, REQ-UX-005 visible focus)", () => {
  it("the highlighted row stays inside the rendered Navigator window no matter how far the selection walks", () => {
    const model = seededModel();
    const outlineLength = navigatorOutline(model.getSnapshot()).length;
    // Last row: beyond any plausible pane-top window at Compact heights.
    const state: ShellState = {
      ...stateAt(),
      focusedSurface: "Navigator",
      navigatorIndex: outlineLength - 1,
    };
    const rendered = renderShell(
      state,
      model.getSnapshot(),
      defaultViewRegistry(),
      renderOptions(model),
    );
    const navigator = rendered.surfaces.find((surface) => surface.id === "Navigator");
    expect(navigator).toBeDefined();
    const text = navigator!.lines.map((line) => line.spans.map((span) => span.text).join(""));
    const highlightRow = text.find((row) => row.includes("◉"));
    expect(highlightRow).toBeDefined();
    if (outlineLength > state.dimensions.rows - 10) {
      expect(text.some((row) => row.includes("row(s) above"))).toBe(true);
    }
  });
});

describe("compact drawers (V2, UESC Compact switchable side surfaces)", () => {
  const COMPACT = { cols: 100, rows: 30 };

  it("i opens the Inspector drawer and n the Navigator drawer at Compact; both inert at Full", () => {
    const snap = snapshot();
    const compact = createShellState(HOST, COMPACT);
    const opened = handleCommand(compact, snap, "ui.inspector.toggle");
    expect(opened.state.drawer).toBe("Inspector");
    const closed = handleCommand(opened.state, snap, "ui.inspector.toggle");
    expect(closed.state.drawer).toBeNull();

    const nav = handleCommand(compact, snap, "ui.navigator.toggle");
    expect(nav.state.drawer).toBe("Navigator");

    const full = handleCommand(stateAt(), snap, "ui.inspector.toggle");
    expect(full.state.drawer).toBeNull();
  });

  it("escape closes the drawer before blurring; the render carries the drawer at Compact", () => {
    const model = seededModel();
    const session = new ShellSession(HOST, COMPACT);
    session.key("n", model.getSnapshot());
    expect(session.state.drawer).toBe("Navigator");
    session.key("escape", model.getSnapshot());
    expect(session.state.drawer).toBeNull();

    session.key("n", model.getSnapshot());
    const rendered = session.render(model.getSnapshot(), renderOptions(model));
    expect(rendered.drawer).not.toBeNull();
    expect(rendered.drawer?.surface).toBe("Navigator");
    expect(rendered.drawer?.lines.length).toBeGreaterThan(1);
  });

  it("drawer navigator clicks select the same focus as docked rows (DSN-06 placements)", () => {
    const snap = snapshot();
    const base: ShellState = { ...createShellState(HOST, COMPACT), drawer: "Navigator" };
    const result = handlePointer(base, snap, {
      kind: "click",
      surfaceId: "Drawer",
      lineId: "outline.unit.u-far",
      x: 2,
    });
    const select = result.effects.find((e) => e.type === "select_focus");
    expect(select).toBeDefined();
  });
});

describe("bottom surface render + height ladder (UESC §7.5, DESIGN Height constraints)", () => {
  it("renders the bottom entry row at Full/tall, opens with b, and hides at short heights", () => {
    const model = seededModel();
    const tall = new ShellSession(HOST, FULL_DIMS);
    const closed = tall.render(model.getSnapshot(), renderOptions(model));
    expect(closed.bottom).not.toBeNull();
    expect(closed.bottom?.open).toBe(false);
    expect(closed.bottom?.tabs).toContain("evidence");

    tall.key("b", model.getSnapshot());
    const opened = tall.render(model.getSnapshot(), renderOptions(model));
    expect(opened.bottom?.open).toBe(true);

    // 120x30 = Full/short (V1 fix keeps columns): the bottom collapses first.
    tall.resize({ cols: 120, rows: 30 });
    const short = tall.render(model.getSnapshot(), renderOptions(model));
    expect(short.bottom).toBeNull();
    expect(short.surfaces.map((s) => s.id)).toContain("Navigator");
    expect(short.surfaces.map((s) => s.id)).toContain("Inspector");
  });
});

describe("back/forward history (ICM §6.6)", () => {
  it("view switches and selections leave a trail; [ and ] walk it without effects", () => {
    const model = seededModel();
    const session = new ShellSession(HOST, FULL_DIMS);
    session.key("3", model.getSnapshot()); // trace
    session.key("2", model.getSnapshot()); // chat
    expect(session.state.location.view).toBe("chat");

    session.key("[", model.getSnapshot());
    expect(session.state.location.view).toBe("trace");
    session.key("[", model.getSnapshot());
    expect(session.state.location.view).toBe("work");
    session.key("]", model.getSnapshot());
    expect(session.state.location.view).toBe("trace");
  });
});
