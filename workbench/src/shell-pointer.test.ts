import { describe, expect, it } from "bun:test";
import type { AffordanceObservation } from "./affordance";
import { FocusRef } from "./focus";
import {
  contextMenuEntries,
  createShellState,
  handleCommand,
  handlePointer,
  paletteEntries,
  reconcileTrace,
  ShellSession,
  type ShellState,
} from "./shell";
import { FULL_DIMS, seededModel } from "./testing/shell-harness";
import type { FullWorkbenchSnapshot } from "./workbench-model";

/**
 * GUI-redesign increment 3: pointer gestures converge on the SAME semantic
 * commands as keys (ICM §24/§35: single click selects, double-click/Enter
 * opens, wheel scrolls the region under the pointer). These tests pin the pure
 * gesture→command mapping; no second behavior pipeline exists.
 */

const HOST = { workspace: { workspace: "gauntlet", reference: "ref-1d457ec9", run: "R17" } };

function stateAt(): ShellState {
  return createShellState(HOST, FULL_DIMS);
}

function snapshot(): FullWorkbenchSnapshot {
  return seededModel().getSnapshot();
}

describe("pointer click (ICM §35.1: single click selects)", () => {
  it("clicking a Navigator outline row selects that focus — identical effect to keyboard selection", () => {
    const snap = snapshot();
    const base = stateAt();
    const result = handlePointer(base, snap, {
      kind: "click",
      surfaceId: "Navigator",
      lineId: "outline.unit.u-far",
      x: 2,
    });
    expect(result.state.navigatorIndex).toBeGreaterThan(0);
    const selectEffect = result.effects.find((e) => e.type === "select_focus");
    expect(selectEffect).toBeDefined();
    expect(
      selectEffect !== undefined && selectEffect.type === "select_focus" ? selectEffect.ref.id : "",
    ).toContain("u-far");

    // Keyboard equivalence: moving to the same row and pressing enter yields
    // the same select effect (command-level convergence).
    const viaKey = {
      ...base,
      focusedSurface: "Navigator" as const,
      navigatorIndex: result.state.navigatorIndex,
    };
    const keyed = handleCommand(viaKey, snap, "nav.focus.select");
    expect(keyed.effects).toEqual(result.effects);
  });

  it("clicking a palette row selects it (no activation); double-click activates", () => {
    const snap = snapshot();
    const opened = handleCommand(stateAt(), snap, "ui.palette.open");
    const clicked = handlePointer(opened.state, snap, {
      kind: "click",
      surfaceId: "Overlay",
      lineId: "palette.2",
      x: 3,
    });
    expect(clicked.state.paletteIndex).toBe(2);
    expect(clicked.effects).toEqual([]);

    const dbl = handlePointer(opened.state, snap, {
      kind: "double_click",
      surfaceId: "Overlay",
      lineId: "palette.2",
      x: 3,
    });
    expect(dbl.state.paletteIndex).toBe(2);
    // Activation of an available view row closes the palette (same as Enter).
    expect(dbl.state.paletteOpen).toBe(false);
  });

  it("clicking a primary tab span activates that view through the same command as keys", () => {
    const snap = snapshot();
    const base = stateAt();
    // The tab line spans: "PRIMARY " (8 cells) then [WORK] [CHAT] [TRACE]:
    // WORK 8..14, CHAT 15..21, TRACE 22..30. x=24 lands in [TRACE].
    const result = handlePointer(base, snap, {
      kind: "click",
      surfaceId: "Primary",
      lineId: "primary.tabs",
      x: 24,
    });
    expect(result.state.location.view).toBe("trace");
    // Keyboard equivalence: the same activation via the semantic command.
    const viaCommand = handleCommand(base, snap, "view.activate.trace");
    expect(result.state.location.view).toBe(viaCommand.state.location.view);
  });

  it("clicking the Composer surface focuses the composer (same as e)", () => {
    const snap = snapshot();
    const result = handlePointer(stateAt(), snap, {
      kind: "click",
      surfaceId: "Composer",
      lineId: null,
      x: 1,
    });
    expect(result.state.focusedSurface).toBe("Composer");
  });

  it("clicking a trace row selects it by stable id without moving the window", () => {
    const snap = snapshot();
    const state = reconcileTrace(handleCommand(stateAt(), snap, "view.activate.trace").state, snap);
    const ids = snap.trace_records.map((r) => r.trace_id);
    const mid = ids[Math.floor(ids.length / 2)] as string;
    const result = handlePointer(state, snap, {
      kind: "click",
      surfaceId: "Primary",
      lineId: `trace.${mid}`,
      x: 1,
    });
    expect(result.state.trace.selectedId).toBe(mid);
    // Selection never jumps the ledger (identity, not viewport).
    expect(result.state.trace.followState).toBe(state.trace.followState);
  });
});

describe("pointer wheel (ICM §35.4: scrolls the region under the pointer)", () => {
  it("wheel over Primary scrolls the primary view; over Navigator moves the selection", () => {
    const snap = snapshot();
    const base = stateAt();

    const primary = handlePointer(base, snap, { kind: "wheel", surfaceId: "Primary", delta: 2 });
    expect(primary.state.scroll["work"]).toBe(6); // 3 lines per notch

    const nav = handlePointer({ ...base, focusedSurface: "Navigator" }, snap, {
      kind: "wheel",
      surfaceId: "Navigator",
      delta: 2,
    });
    expect(nav.state.navigatorIndex).toBe(2);

    // Wheel over the composer scrolls nothing (region under pointer governs,
    // not the focused surface).
    const composer = handlePointer(base, snap, { kind: "wheel", surfaceId: "Composer", delta: 2 });
    expect(composer.state.scroll["work"]).toBeUndefined();
  });

  it("wheel over the open palette moves the palette selection", () => {
    const snap = snapshot();
    const opened = handleCommand(stateAt(), snap, "ui.palette.open");
    const wheeled = handlePointer(opened.state, snap, {
      kind: "wheel",
      surfaceId: "Overlay",
      delta: 1,
    });
    expect(wheeled.state.paletteIndex).toBe(1);
  });

  it("wheel in the trace view drives the follow/hold machine, not the line scroller", () => {
    const snap = snapshot();
    const state = reconcileTrace(handleCommand(stateAt(), snap, "view.activate.trace").state, snap);
    const wheeled = handlePointer(state, snap, { kind: "wheel", surfaceId: "Primary", delta: -1 });
    expect(wheeled.state.trace.followState).toBe("INSPECTION_HOLD");
    expect(wheeled.state.scroll["trace"]).toBeUndefined();
  });
});

describe("pointer gestures are inert in contexts that own their input", () => {
  it("pointer is inert below the minimum geometry", () => {
    const snap = snapshot();
    const small = createShellState(HOST, { cols: 72, rows: 20 });
    const result = handlePointer(small, snap, {
      kind: "click",
      surfaceId: "Navigator",
      lineId: "outline.unit.u-root",
      x: 1,
    });
    expect(result.effects).toEqual([]);
    expect(result.state).toBe(small);
  });

  it("pointer click on an unknown line id changes nothing", () => {
    const snap = snapshot();
    const base = stateAt();
    const result = handlePointer(base, snap, {
      kind: "click",
      surfaceId: "Primary",
      lineId: "not.a.row",
      x: 1,
    });
    expect(result.state).toBe(base);
    expect(result.effects).toEqual([]);
  });
});

describe("breadcrumb hoisting (click a header segment to jump to that depth)", () => {
  /** Header prefix length ahead of the breadcrumb text (mirrors shell.ts's
   * own header span construction exactly — see headerIdentitySpans). */
  const BREADCRUMB_START =
    "Gauntlet ".length +
    `· ${HOST.workspace.workspace}/${HOST.workspace.reference} `.length +
    "· ".length;

  function twoLevelSnapshot(): FullWorkbenchSnapshot {
    const model = seededModel();
    model.hoistToFocus(FocusRef.for("gauntlet", "unit", "u-root", "Root Unit"));
    model.hoistToFocus(FocusRef.for("gauntlet", "unit", "u-other", "Other Unit"));
    return model.getSnapshot();
  }

  it("clicking the root 'Case' segment two levels deep dehoists all the way (identity, never label-matched)", () => {
    const snap = twoLevelSnapshot();
    expect([...snap.horizon_breadcrumb]).toEqual(["Case", "Root Unit", "Other Unit"]);
    const base = stateAt();
    const result = handlePointer(base, snap, {
      kind: "click",
      surfaceId: "header",
      lineId: "shell.header",
      x: BREADCRUMB_START, // first char of "Case"
    });
    expect(result.effects).toEqual([{ type: "dehoist" }, { type: "dehoist" }]);
  });

  it("clicking a middle breadcrumb segment dehoists exactly one level (position count, not a stack search)", () => {
    const snap = twoLevelSnapshot();
    const base = stateAt();
    // "Case" then " › " (3 chars) then "Root Unit".
    const middleStart = BREADCRUMB_START + "Case".length + " › ".length;
    const result = handlePointer(base, snap, {
      kind: "click",
      surfaceId: "header",
      lineId: "shell.header",
      x: middleStart + 1,
    });
    expect(result.effects).toEqual([{ type: "dehoist" }]);
  });

  it("clicking the current (last) breadcrumb segment is a no-op — already there", () => {
    const snap = twoLevelSnapshot();
    const base = stateAt();
    const lastStart = BREADCRUMB_START + "Case › Root Unit".length + " › ".length;
    const result = handlePointer(base, snap, {
      kind: "click",
      surfaceId: "header",
      lineId: "shell.header",
      x: lastStart + 1,
    });
    expect(result.effects).toEqual([]);
    expect(result.state).toBe(base);
  });

  it("clicking past the end of the breadcrumb text is a no-op", () => {
    const snap = twoLevelSnapshot();
    const base = stateAt();
    const result = handlePointer(base, snap, {
      kind: "click",
      surfaceId: "header",
      lineId: "shell.header",
      x: 900,
    });
    expect(result.effects).toEqual([]);
    expect(result.state).toBe(base);
  });

  it("clicking 'Case' at the unhoisted (single-segment) depth is a no-op", () => {
    const snap = snapshot();
    expect([...snap.horizon_breadcrumb]).toEqual(["Case"]);
    const base = stateAt();
    const result = handlePointer(base, snap, {
      kind: "click",
      surfaceId: "header",
      lineId: "shell.header",
      x: BREADCRUMB_START,
    });
    expect(result.effects).toEqual([]);
  });

  it("end to end: clicking the root segment on a REAL model pops the real horizon stack fully", () => {
    const model = seededModel();
    model.hoistToFocus(FocusRef.for("gauntlet", "unit", "u-root", "Root Unit"));
    model.hoistToFocus(FocusRef.for("gauntlet", "unit", "u-other", "Other Unit"));
    const session = new ShellSession(HOST, FULL_DIMS, {
      dehoist: () => model.dehoist(),
    });
    expect(model.getSnapshot().active_horizon).not.toBeNull();
    session.pointer(
      { kind: "click", surfaceId: "header", lineId: "shell.header", x: BREADCRUMB_START },
      model.getSnapshot(),
    );
    expect(model.getSnapshot().active_horizon).toBeNull();
    expect([...model.getSnapshot().horizon_breadcrumb]).toEqual(["Case"]);
  });
});

/**
 * GUI-redesign increment 5: right-click context menu (ICM §6.4, UESC §22.5
 * ContextMenu / §22.10 TreeItem "right click = contextual Affordances"). The
 * menu is a DISTINCT presentation component from CommandPalette but shares
 * its materialization call, so a menu row's effect and a palette row's
 * effect for the same operation must be identical, not merely both present.
 */
describe("context menu (ICM §6.4: right-click opens contextual Affordances)", () => {
  const session = { actor: "operator-test", authenticated: true };

  /** u-far is BLOCKED with no ticket: every affordance the resolver visits
   * for it stays unavailable — used for the "no permitting row" no-op case
   * is unnecessary here; the equivalence test below needs a PERMITTING row,
   * so it targets u-root under the same observation command-registry.test.ts
   * uses for that purpose (fail + an open ticket naming it). */
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

  it("right-click on a Navigator row opens the menu anchored to that row's ref, without mutating selection", () => {
    const snap = snapshot();
    const base = stateAt();
    const result = handlePointer(base, snap, {
      kind: "right_click",
      surfaceId: "Navigator",
      lineId: "outline.unit.u-far",
      x: 5,
      y: 8,
    });
    expect(result.effects).toEqual([]);
    expect(result.state.contextMenu).not.toBeNull();
    expect(result.state.contextMenu?.ref.source_id).toBe("u-far");
    expect(result.state.contextMenu?.anchorX).toBe(5);
    expect(result.state.contextMenu?.anchorY).toBe(8);
    // Presentation-local only: no selection/inspection identity mutated.
    expect(result.state.navigatorIndex).toBe(base.navigatorIndex);
    expect(result.state.location.focus).toBeNull();
  });

  it("menu contents follow ICM §6.4 order: navigation (Open/Inspect) then current semantic Affordances", () => {
    const ref = FocusRef.for("gauntlet", "WorkItem", "u-root");
    const entries = contextMenuEntries(ref, { observation: permittedObservation, session });
    const navIndex = entries.findIndex(
      (e) => e.type === "navigation" && e.commandId === "nav.focus.open",
    );
    const inspectIndex = entries.findIndex(
      (e) => e.type === "navigation" && e.commandId === "nav.focus.inspect",
    );
    const actionIndex = entries.findIndex((e) => e.type === "affordance");
    expect(navIndex).toBeGreaterThanOrEqual(0);
    expect(inspectIndex).toBeGreaterThan(navIndex);
    expect(actionIndex).toBeGreaterThan(inspectIndex);
  });

  it("selecting an affordance entry produces the IDENTICAL effect the palette's equivalent row would produce", () => {
    const snap = snapshot();
    const rootFocus = FocusRef.for("gauntlet", "WorkItem", "u-root");
    const paletteContext = { observation: permittedObservation, session };

    // Palette path: focus u-root, open the palette, land on the permitting
    // affordance row, then activate through the double-Enter gate.
    const focused: ShellState = {
      ...stateAt(),
      location: { ...stateAt().location, focus: rootFocus },
    };
    const paletteOpened = handleCommand(focused, snap, "ui.palette.open", paletteContext);
    const paletteRows = paletteEntries(paletteOpened.state, snap, paletteContext);
    const paletteIndex = paletteRows.findIndex(
      (entry) => entry.type === "affordance" && entry.status === "requires_attention",
    );
    expect(paletteIndex).toBeGreaterThanOrEqual(0);
    const atPaletteRow: ShellState = { ...paletteOpened.state, paletteIndex };
    const paletteStaged = handleCommand(atPaletteRow, snap, "ui.palette.activate", paletteContext);
    const paletteActivated = handleCommand(
      paletteStaged.state,
      snap,
      "ui.palette.activate",
      paletteContext,
    );
    const paletteEffect = paletteActivated.effects.find((e) => e.type === "invoke_affordance");
    expect(paletteEffect).toBeDefined();

    // Context-menu path: right-click the SAME ref, land on the SAME
    // operation's row, activate (single-step per ICM §6.4 behavior 3).
    const rightClicked = handlePointer(stateAt(), snap, {
      kind: "right_click",
      surfaceId: "Navigator",
      lineId: "outline.unit.u-root",
      x: 3,
      y: 4,
    });
    const menuEntries = contextMenuEntries(rootFocus, paletteContext);
    const menuIndex = menuEntries.findIndex(
      (entry) => entry.type === "affordance" && entry.status === "requires_attention",
    );
    expect(menuIndex).toBeGreaterThanOrEqual(0);
    const atMenuRow: ShellState = {
      ...rightClicked.state,
      contextMenu: rightClicked.state.contextMenu
        ? { ...rightClicked.state.contextMenu, index: menuIndex }
        : null,
    };
    const menuActivated = handleCommand(
      atMenuRow,
      snap,
      "ui.context_menu.activate",
      paletteContext,
    );
    const menuEffect = menuActivated.effects.find((e) => e.type === "invoke_affordance");
    expect(menuEffect).toBeDefined();

    // Equivalence, not just presence: identical operation and ref.
    expect(menuEffect).toEqual(paletteEffect);
  });

  it("Escape closes the context menu with no effect", () => {
    const snap = snapshot();
    const opened = handlePointer(stateAt(), snap, {
      kind: "right_click",
      surfaceId: "Navigator",
      lineId: "outline.unit.u-far",
      x: 1,
      y: 1,
    });
    expect(opened.state.contextMenu).not.toBeNull();
    const dismissed = handleCommand(opened.state, snap, "ui.context_menu.dismiss");
    expect(dismissed.state.contextMenu).toBeNull();
    expect(dismissed.effects).toEqual([]);
  });

  it("right-click on a rowless line (Composer) is a no-op", () => {
    const snap = snapshot();
    const base = stateAt();
    const result = handlePointer(base, snap, {
      kind: "right_click",
      surfaceId: "Composer",
      lineId: null,
      x: 2,
      y: 30,
    });
    expect(result.state).toBe(base);
    expect(result.effects).toEqual([]);
  });

  it("clicking elsewhere while the menu is open closes it without effect", () => {
    const snap = snapshot();
    const opened = handlePointer(stateAt(), snap, {
      kind: "right_click",
      surfaceId: "Navigator",
      lineId: "outline.unit.u-far",
      x: 1,
      y: 1,
    });
    expect(opened.state.contextMenu).not.toBeNull();
    const clickedAway = handlePointer(opened.state, snap, {
      kind: "click",
      surfaceId: "Primary",
      lineId: "primary.tabs",
      x: 0,
    });
    expect(clickedAway.state.contextMenu).toBeNull();
    expect(clickedAway.effects).toEqual([]);
  });
});
