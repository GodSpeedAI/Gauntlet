import { describe, expect, it } from "bun:test";
import { FocusRef } from "./focus";
import { createShellState, handleKey, paletteEntries, ShellSession } from "./shell";
import {
  COMPACT_DIMS,
  FULL_DIMS,
  observationFor,
  renderOptions,
  seededModel,
  shellText,
} from "./testing/shell-harness";

const HOST = {
  workspace: { workspace: "gauntlet", reference: "ref-1d457ec9", run: "R17" },
} as const;

/** Palette entries for a freshly opened palette, with a real observation. */
function openedPalette() {
  const model = seededModel();
  model.selectFocus(FocusRef.for("gauntlet", "WorkItem", "u-root"));
  const snapshot = model.getSnapshot();
  const context = { observation: observationFor(model) };
  const state = handleKey(createShellState(HOST, FULL_DIMS), snapshot, "p").state;
  return { model, snapshot, context, state };
}

describe("palette grouping (layout)", () => {
  it("labels each non-empty group with a header row carrying its count", () => {
    const { snapshot, context, state } = openedPalette();
    const entries = paletteEntries(state, snapshot, context);
    const headers = entries.filter((entry) => entry.type === "header");
    expect(headers.length).toBeGreaterThanOrEqual(1);
    const views = entries.filter((entry) => entry.type === "view").length;
    const actions = entries.filter((entry) => entry.type === "affordance").length;
    expect(headers.map((entry) => entry.type === "header" && entry.label)).toContain(
      `Views (${views})`,
    );
    if (actions > 0) {
      expect(headers.map((entry) => entry.type === "header" && entry.label)).toContain(
        `Actions (${actions})`,
      );
    }
    // Headers lead their groups: the row after a header belongs to that group.
    for (let i = 0; i < entries.length; i += 1) {
      const entry = entries[i];
      if (entry?.type !== "header" || entry.label.startsWith("Views")) continue;
      const next = entries[i + 1];
      expect(next?.type).toBe("affordance");
    }
  });

  it("Enter on a header row is a noop: palette stays open, nothing fires", () => {
    const { model, snapshot, context, state } = openedPalette();
    const entries = paletteEntries(state, snapshot, context);
    const headerIndex = entries.findIndex((entry) => entry.type === "header");
    expect(headerIndex).toBeGreaterThanOrEqual(0);
    const session = new ShellSession(HOST, FULL_DIMS);
    session.key("p", snapshot);
    for (let i = 0; i < headerIndex; i += 1) {
      session.key("down", snapshot, context);
    }
    const result = handleKey(
      { ...session.state, paletteIndex: headerIndex },
      snapshot,
      "enter",
      context,
    );
    expect(result.effects).toEqual([]);
    expect(result.state.paletteOpen).toBe(true);
    expect(model.getSnapshot().focus?.id).toBe("focus:gauntlet:WorkItem:u-root");
  });

  it("down-walk arithmetic still lands on the intended row with headers present", () => {
    const model = seededModel();
    const session = new ShellSession(HOST, FULL_DIMS);
    session.key("p", model.getSnapshot());
    const entries = paletteEntries(session.state, model.getSnapshot(), {});
    const agentsIndex = entries.findIndex(
      (entry) => entry.type === "view" && entry.view === "agents",
    );
    expect(agentsIndex).toBeGreaterThanOrEqual(0);
    for (let i = 0; i < agentsIndex; i += 1) {
      session.key("down", model.getSnapshot());
    }
    session.key("enter", model.getSnapshot());
    expect(session.state.location.view).toBe("agents");
  });

  it("down clamps at the last row instead of running past the list", () => {
    const { snapshot, context, state } = openedPalette();
    const entries = paletteEntries(state, snapshot, context);
    let current = state;
    for (let i = 0; i < entries.length + 5; i += 1) {
      current = handleKey(current, snapshot, "down", context).state;
    }
    expect(current.paletteIndex).toBe(entries.length - 1);
  });
});

describe("legend and title rhythm (layout)", () => {
  it("Compact help wraps to two short lines that still name every bound key", () => {
    const model = seededModel();
    const session = new ShellSession(HOST, COMPACT_DIMS);
    const rendered = session.render(model.getSnapshot(), renderOptions(model));
    expect(rendered.help.length).toBe(2);
    const helpText = rendered.help
      .map((line) => line.spans.map((span) => span.text).join(""))
      .join("\n");
    for (const key of ["1/2/3", "tab", "up/down", "enter", "h", "u", "p", "e", "esc", "q"]) {
      expect(helpText).toContain(key);
    }
  });

  it("Full help stays a single legend line", () => {
    const model = seededModel();
    const session = new ShellSession(HOST, FULL_DIMS);
    const rendered = session.render(model.getSnapshot(), renderOptions(model));
    expect(rendered.help.length).toBe(1);
  });

  it("primary title names the view without restating the view question", () => {
    const model = seededModel();
    const session = new ShellSession(HOST, FULL_DIMS);
    const rendered = session.render(model.getSnapshot(), renderOptions(model));
    const title = shellText(rendered, "Primary")[0] ?? "";
    // GUI redesign tab strip: fixed-order tabs, ▸ marks the active tab.
    expect(title).toContain("▸[WORK]");
    expect(title).toContain("[CHAT]");
    expect(title).toContain("[TRACE]");
    expect(title).not.toContain("What are we trying");
  });
});

describe("affordance confirm staging (harden)", () => {
  function stagedConfirm() {
    const { model, snapshot, context } = openedPalette();
    const session = new ShellSession(HOST, FULL_DIMS);
    session.key("p", snapshot, context);
    const entries = paletteEntries(session.state, snapshot, context);
    const index = entries.findIndex((entry) => entry.type === "affordance");
    expect(index).toBeGreaterThanOrEqual(0);
    for (let i = 0; i < index; i += 1) {
      session.key("down", snapshot, context);
    }
    return { model, snapshot, context, session, index };
  }

  it("first Enter stages a confirm: no effect fires, the row names the invoke", () => {
    const { model, snapshot, context, session } = stagedConfirm();
    const before = session.state;
    session.key("enter", snapshot, context);
    expect(session.state.paletteOpen).toBe(true);
    expect(session.state.pendingConfirm).not.toBeNull();
    const confirm = session.state.pendingConfirm;
    expect(before.paletteIndex).toBe(session.state.paletteIndex);
    const rendered = session.render(snapshot, renderOptions(model));
    const paletteText = (rendered.palette?.lines ?? [])
      .map((line) => line.spans.map((span) => span.text).join(""))
      .join("\n");
    expect(paletteText).toContain(`Invoke ${confirm?.operation ?? ""}`);
    expect(paletteText).toContain("Enter confirms");
  });

  it("second Enter on the same row invokes and closes the palette", () => {
    const { snapshot, context, session } = stagedConfirm();
    session.key("enter", snapshot, context);
    const staged = session.state.pendingConfirm;
    expect(staged).not.toBeNull();
    let invoked: { value: string | null } = { value: null };
    const wired = new ShellSession(HOST, FULL_DIMS, {
      invokeAffordance: (operation, ref) => {
        invoked = { value: `${operation}:${ref.id}` };
      },
    });
    wired.key("p", snapshot, context);
    const entries = paletteEntries(wired.state, snapshot, context);
    const index = entries.findIndex((entry) => entry.type === "affordance");
    for (let i = 0; i < index; i += 1) {
      wired.key("down", snapshot, context);
    }
    wired.key("enter", snapshot, context);
    wired.key("enter", snapshot, context);
    expect(wired.state.paletteOpen).toBe(false);
    expect(invoked.value).toBe(`${staged?.operation ?? ""}:${staged?.ref.id ?? ""}`);
  });

  it("Escape backs out of a staged confirm before closing the palette", () => {
    const { snapshot, context, session } = stagedConfirm();
    session.key("enter", snapshot, context);
    expect(session.state.pendingConfirm).not.toBeNull();
    session.key("escape", snapshot, context);
    expect(session.state.pendingConfirm).toBeNull();
    expect(session.state.paletteOpen).toBe(true);
    session.key("escape", snapshot, context);
    expect(session.state.paletteOpen).toBe(false);
  });

  it("moving the selection clears a staged confirm", () => {
    const { snapshot, context, session } = stagedConfirm();
    session.key("enter", snapshot, context);
    expect(session.state.pendingConfirm).not.toBeNull();
    session.key("down", snapshot, context);
    expect(session.state.pendingConfirm).toBeNull();
    expect(session.state.paletteOpen).toBe(true);
  });
});
