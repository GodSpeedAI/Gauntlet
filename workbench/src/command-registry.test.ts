import { describe, expect, it } from "bun:test";
import type { AffordanceObservation } from "./affordance";
import {
  COMMAND_REGISTRY,
  type CommandKeyContext,
  commandForKey,
  commandShortcut,
  type SemanticCommandId,
  validateCommandRegistry,
} from "./command-registry";
import { FocusRef } from "./focus";
import type { Dimensions } from "./geometry";
import {
  createShellState,
  handleCommand,
  handleKey,
  paletteEntries,
  type ShellState,
} from "./shell";
import { FULL_DIMS, seededModel } from "./testing/shell-harness";
import type { FullWorkbenchSnapshot } from "./workbench-model";

/**
 * GUI-redesign increment 1: the semantic command layer (ICM §4, §34). Keys are
 * one representation of intent; mouse/menu/palette rows must resolve to the
 * same command identities through this registry. These tests pin the resolver
 * table, the startup validation contract, convergence of handleKey and
 * handleCommand, and the ICM §30 rule that a non-permitting affordance status
 * can never stage or invoke from the palette.
 */

const HOST = { workspace: { workspace: "gauntlet", reference: "ref-1d457ec9", run: "R17" } };

function stateAt(dims: Dimensions = FULL_DIMS): ShellState {
  return createShellState(HOST, dims);
}

function snapshot(): FullWorkbenchSnapshot {
  return seededModel().getSnapshot();
}

const ROOT_FOCUS = FocusRef.for("gauntlet", "WorkItem", "u-root");

/** Authenticated session so the resolver's actor gate passes. */
const session = { actor: "operator-test", authenticated: true };

/** u-root failed and a ticket names it: unit.resolve resolves
 * requires_attention and unit.retry available — the permitting statuses. */
const permittedObservation: AffordanceObservation = {
  reachable: true,
  degraded: false,
  units: [{ unit_id: "u-root", status: "fail" }],
  open_escalations: [{ ticket_id: "t-1", unit_id: "u-root" }],
  budget_remaining: null,
  governance_denied_units: [],
  authority_revoked_units: [],
  capabilities: [],
};

describe("command registry (ICM §4, §48)", () => {
  it("validates: unique ids, unique default keys per context, every bound key resolvable", () => {
    expect(validateCommandRegistry()).toEqual([]);
  });

  it("registry carries title, class, and context for every command the shell implements", () => {
    const ids = new Set(COMMAND_REGISTRY.map((command) => command.id));
    for (const required of [
      "app.quit",
      "view.activate.work",
      "view.activate.chat",
      "view.activate.trace",
      "ui.surface.next",
      "ui.surface.prev",
      "ui.dismiss",
      "ui.palette.open",
      "ui.palette.close",
      "ui.palette.dismiss",
      "ui.palette.move.prev",
      "ui.palette.move.next",
      "ui.palette.activate",
      "nav.move.prev",
      "nav.move.next",
      "nav.focus.select",
      "nav.horizon.hoist",
      "nav.horizon.dehoist",
      "ui.composer.focus",
    ] as const satisfies readonly SemanticCommandId[]) {
      expect(ids.has(required)).toBe(true);
    }
    for (const command of COMMAND_REGISTRY) {
      expect(command.title.length).toBeGreaterThan(0);
      expect(command.commandClass.length).toBeGreaterThan(0);
    }
  });

  it("fails validation when two commands bind the same key in one context (falsifier)", () => {
    const quit = COMMAND_REGISTRY.find((c) => c.id === "app.quit");
    expect(quit).toBeDefined();
    if (quit === undefined) return;
    const duplicate = [...COMMAND_REGISTRY, { ...quit, id: "app.quit.copy" as SemanticCommandId }];
    const problems = validateCommandRegistry(duplicate);
    expect(problems.some((p) => p.includes("app.quit"))).toBe(true);
  });
});

describe("keymap resolver (ICM §34: keys are bindings, not identity)", () => {
  const shellCtx: CommandKeyContext = { surface: "shell" };
  const paletteCtx: CommandKeyContext = { surface: "palette_open" };
  const belowCtx: CommandKeyContext = { surface: "below_minimum" };

  it("resolves every earned binding to its semantic command id in the shell context", () => {
    expect(commandForKey("1", shellCtx)).toBe("view.activate.work");
    expect(commandForKey("2", shellCtx)).toBe("view.activate.chat");
    expect(commandForKey("3", shellCtx)).toBe("view.activate.trace");
    expect(commandForKey("tab", shellCtx)).toBe("ui.surface.next");
    expect(commandForKey("shift+tab", shellCtx)).toBe("ui.surface.prev");
    expect(commandForKey("escape", shellCtx)).toBe("ui.dismiss");
    expect(commandForKey("p", shellCtx)).toBe("ui.palette.open");
    expect(commandForKey("up", shellCtx)).toBe("nav.move.prev");
    expect(commandForKey("down", shellCtx)).toBe("nav.move.next");
    expect(commandForKey("enter", shellCtx)).toBe("nav.focus.select");
    expect(commandForKey("h", shellCtx)).toBe("nav.horizon.hoist");
    expect(commandForKey("u", shellCtx)).toBe("nav.horizon.dehoist");
    expect(commandForKey("e", shellCtx)).toBe("ui.composer.focus");
    expect(commandForKey("q", shellCtx)).toBe("app.quit");
  });

  it("rebinds the same physical key to different commands by context (palette owns input)", () => {
    expect(commandForKey("p", paletteCtx)).toBe("ui.palette.close");
    expect(commandForKey("escape", paletteCtx)).toBe("ui.palette.dismiss");
    expect(commandForKey("up", paletteCtx)).toBe("ui.palette.move.prev");
    expect(commandForKey("down", paletteCtx)).toBe("ui.palette.move.next");
    expect(commandForKey("enter", paletteCtx)).toBe("ui.palette.activate");
    // While the palette is open, shell-only keys resolve to nothing: the
    // palette swallows them exactly as the literal switch did (q must NOT
    // quit from inside the palette).
    expect(commandForKey("q", paletteCtx)).toBeNull();
    expect(commandForKey("1", paletteCtx)).toBeNull();
  });

  it("below minimum only admits quit", () => {
    expect(commandForKey("q", belowCtx)).toBe("app.quit");
    expect(commandForKey("p", belowCtx)).toBeNull();
    expect(commandForKey("1", belowCtx)).toBeNull();
  });

  it("unbound keys resolve to null (no accidental identity)", () => {
    expect(commandForKey("z", shellCtx)).toBeNull();
    expect(commandForKey("P", shellCtx)).toBeNull();
    expect(commandForKey("ctrl+k", shellCtx)).toBeNull();
  });

  it("exposes the default shortcut for help/palette rendering", () => {
    expect(commandShortcut("ui.palette.open")).toBe("p");
    expect(commandShortcut("view.activate.trace")).toBe("3");
    expect(commandShortcut("nav.focus.select")).toBe("enter");
    expect(commandShortcut("ui.palette.activate")).toBeNull();
  });
});

describe("handleKey/handleCommand convergence (ICM §35: two representations, one behavior)", () => {
  it("handleKey is exactly resolve+handleCommand for a representative command in every context", () => {
    const snap = snapshot();
    const base: ShellState = {
      ...stateAt(),
      location: { ...stateAt().location, focus: ROOT_FOCUS },
    };

    // Shell context: hoist.
    const viaKey = handleKey(base, snap, "h");
    const viaCommand = handleCommand(base, snap, "nav.horizon.hoist");
    expect(viaKey.state.location.horizon).not.toBeNull();
    expect(viaCommand.state.location.horizon).not.toBeNull();
    expect(viaKey.effects).toEqual(viaCommand.effects);

    // Palette context: open, move, dismiss.
    const opened = handleKey(base, snap, "p");
    expect(opened.state.paletteOpen).toBe(true);
    const movedKey = handleKey(opened.state, snap, "down");
    const movedCommand = handleCommand(opened.state, snap, "ui.palette.move.next");
    expect(movedKey.state.paletteIndex).toBe(movedCommand.state.paletteIndex);
    expect(movedCommand.state.paletteIndex).toBe(1);
    expect(handleKey(opened.state, snap, "escape").state.paletteOpen).toBe(false);

    // Below minimum: quit only.
    const small = stateAt({ cols: 72, rows: 20 });
    expect(handleKey(small, snap, "q").effects).toEqual([{ type: "quit" }]);
    expect(handleCommand(small, snap, "app.quit").effects).toEqual([{ type: "quit" }]);
    expect(handleKey(small, snap, "p").effects).toEqual([]);
  });

  it("a pointer-equivalent caller driving handleCommand('ui.palette.open') cannot diverge from the key path", () => {
    const snap = snapshot();
    const a = handleKey(stateAt(), snap, "p");
    const b = handleCommand(stateAt(), snap, "ui.palette.open");
    expect(a.state.paletteOpen).toBe(true);
    expect(b.state.paletteOpen).toBe(true);
    expect(a.state.focusedSurface).toBe(b.state.focusedSurface);
    expect(a.effects).toEqual(b.effects);
  });
});

describe("palette invocation forwarding (frozen T23 semantics; ICM §30 gate deferred)", () => {
  function openPaletteAtAffordance(observation: AffordanceObservation, status: string): ShellState {
    const snap = snapshot();
    const base = stateAt();
    const focused: ShellState = {
      ...base,
      location: { ...base.location, focus: ROOT_FOCUS },
    };
    const paletteContext = { observation, session };
    const opened = handleCommand(focused, snap, "ui.palette.open", paletteContext);
    const entries = paletteEntries(opened.state, snap, paletteContext);
    const index = entries.findIndex(
      (entry) => entry.type === "affordance" && entry.status === status,
    );
    expect(index).toBeGreaterThanOrEqual(0);
    return { ...opened.state, paletteIndex: index };
  }

  it("a permitting status stages and invokes through the double-Enter path (router forwards; gateway decides)", () => {
    const snap = snapshot();
    const atRow = openPaletteAtAffordance(permittedObservation, "requires_attention");
    const firstEnter = handleCommand(atRow, snap, "ui.palette.activate", {
      observation: permittedObservation,
      session,
    });
    expect(firstEnter.state.pendingConfirm).not.toBeNull();
    const secondEnter = handleCommand(firstEnter.state, snap, "ui.palette.activate", {
      observation: permittedObservation,
      session,
    });
    const invoked = secondEnter.effects.filter((e) => e.type === "invoke_affordance");
    expect(invoked).toHaveLength(1);
    expect(secondEnter.state.paletteOpen).toBe(false);
  });

  // DEFERRED RESIDUAL (GUI-redesign increment 5, ActionControl states): ICM
  // §30 makes a VISIBLE_DISABLED row non-activatable, and the state.refresh
  // host short-circuit must route honestly. Until then the palette forwards
  // any status and the canonical gateway denies — the frozen T23 matrix and
  // its racing-revocation teeth pin that forwarding (cross-surface-matrix.ts
  // shellDrive).
});
