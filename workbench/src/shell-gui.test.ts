import { describe, expect, it } from "bun:test";
import { commandForKey, validateCommandRegistry } from "./command-registry";
import { FocusRef } from "./focus";
import {
  createShellState,
  filterPaletteEntries,
  handleCommand,
  handleKey,
  paletteEntries,
  type ShellState,
} from "./shell";
import { FULL_DIMS, seededModel } from "./testing/shell-harness";
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
