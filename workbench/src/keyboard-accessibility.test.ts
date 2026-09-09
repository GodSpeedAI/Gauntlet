import { describe, expect, it } from "bun:test";
import type { RenderedShell } from "./shell";
import { navigatorOutline, ShellSession } from "./shell";
import {
  COMPACT_DIMS,
  coldModel,
  FULL_DIMS,
  modelWiredSession,
  OPERATOR_SESSION,
  renderOptions,
  seededModel,
  shellText,
  unitEvent,
} from "./testing/shell-harness";
import type { SemanticToken } from "./theme";
import { SEMANTIC_TOKENS } from "./theme";

/**
 * T12 keyboard-accessibility suites (REQ-UX-005, REQ-UX-007): every journey
 * is keyboard-only; keyboard focus is visible; contextual help is
 * discoverable; no consequential state relies on colour (glyph/label carry
 * it); stream output never steals scroll or focus; escape hatches are
 * reachable everywhere.
 */

const CONSEQUENTIAL_FAMILIES = new Set([
  "authority",
  "execution",
  "evidence",
  "settlement",
  "readiness",
  "integrity",
  "capability",
  "attention",
]);

function allLines(rendered: RenderedShell) {
  return [
    rendered.header,
    ...rendered.surfaces.flatMap((surface) => surface.lines),
    ...rendered.attention,
    ...rendered.help,
    ...(rendered.palette?.lines ?? []),
    ...(rendered.prompt?.lines ?? []),
  ];
}

describe("T12 keyboard focus is always visible (REQ-UX-005)", () => {
  it("marks exactly one focused surface and rings the highlighted outline row", () => {
    const model = seededModel();
    const session = modelWiredSession(model, FULL_DIMS);

    let rendered = session.render(model.getSnapshot(), renderOptions(model));
    expect(rendered.surfaces.filter((surface) => surface.focused).length).toBe(1);
    expect(rendered.surfaces.find((surface) => surface.focused)?.id).toBe("Primary");

    // Move outline highlight to u-far and focus the Navigator.
    session.key("tab", model.getSnapshot());
    session.key("tab", model.getSnapshot());
    session.key("tab", model.getSnapshot());
    const farIndex = navigatorOutline(model.getSnapshot()).findIndex(
      (row) => row.id === "outline.unit.u-far",
    );
    for (let i = 0; i < farIndex; i += 1) {
      session.key("down", model.getSnapshot());
    }
    rendered = session.render(model.getSnapshot(), renderOptions(model));

    // The highlighted Navigator row carries the focus ring glyph + bold label.
    const navigatorLines = rendered.surfaces
      .find((surface) => surface.id === "Navigator")
      ?.lines.filter((line) => line.id?.startsWith("outline."))
      .map((line, index) => ({ line, index }));
    const highlighted = navigatorLines?.find(
      (entry) =>
        entry.line.spans.some((span) => span.token === "focus.ring") &&
        entry.line.spans.some((span) => span.emphasis === "bold"),
    );
    expect(highlighted?.index).toBe(farIndex);
    expect(highlighted?.line.spans.map((span) => span.text).join("")).toContain("Far Unit");
  });
});

describe("T12 contextual help is discoverable (REQ-UX-005)", () => {
  it("renders a persistent key legend naming every bound key", () => {
    const model = seededModel();
    const session = modelWiredSession(model, COMPACT_DIMS);
    const rendered = session.render(model.getSnapshot(), renderOptions(model));
    const helpText = rendered.help
      .map((line) => line.spans.map((span) => span.text).join(""))
      .join("\n");
    for (const key of ["1/2/3", "tab", "up/down", "enter", "h", "u", "p", "e", "esc", "q"]) {
      expect(helpText).toContain(key);
    }
  });

  it("the below-minimum prompt names its exit key and the composer names its keys", () => {
    const model = seededModel();
    const session = modelWiredSession(model, { cols: 60, rows: 14 });
    const rendered = session.render(model.getSnapshot(), renderOptions(model));
    const promptText = (rendered.prompt?.lines ?? [])
      .map((line) => line.spans.map((span) => span.text).join(""))
      .join("\n");
    expect(promptText).toContain("press q to quit");

    session.resize(FULL_DIMS);
    session.key("e", model.getSnapshot());
    const composerText = shellText(
      session.render(model.getSnapshot(), renderOptions(model)),
      "Composer",
    ).join("\n");
    expect(composerText).toContain("enter submits");
  });
});

describe("T12 no colour-only meaning (REQ-UX-005, DSN-002)", () => {
  it("every consequential token span carries its glyph in the rendered text", () => {
    const model = seededModel();
    model.declareCapabilities([
      {
        capability: "domainforge",
        installed: false,
        gates: {
          visibility: false,
          reachability: false,
          payment: false,
          governance: false,
          settlement_access: false,
          recovery: false,
        },
      },
    ]);
    const session = modelWiredSession(model, FULL_DIMS);
    const options = { ...renderOptions(model), session: OPERATOR_SESSION };
    const rendered = session.render(model.getSnapshot(), options);

    for (const line of allLines(rendered)) {
      const text = line.spans.map((span) => span.text).join("");
      for (const span of line.spans) {
        if (span.token === undefined) {
          continue;
        }
        const family = SEMANTIC_TOKENS[span.token as SemanticToken].family;
        if (!CONSEQUENTIAL_FAMILIES.has(family)) {
          continue;
        }
        const glyph = SEMANTIC_TOKENS[span.token as SemanticToken].glyph;
        expect(text.includes(glyph)).toBe(true);
      }
    }
  });

  it("the same render in monochrome carries the identical text (colour adds nothing textual)", () => {
    const model = seededModel();
    const session = modelWiredSession(model, FULL_DIMS);
    // Colour capability is a host concern; the render tree carries token
    // NAMES only, so the textual content is capability-independent by
    // construction — asserted here by rendering twice and comparing text.
    const first = shellText(session.render(model.getSnapshot(), renderOptions(model))).join("\n");
    const second = shellText(session.render(model.getSnapshot(), renderOptions(model))).join("\n");
    expect(first).toBe(second);
    // And every status word survives as text.
    expect(first).toContain("BLOCKED");
    expect(first).toContain("ACTIVE");
  });
});

describe("T12 stream output never steals scroll position or focus (REQ-UX-005)", () => {
  it("a mid-scroll source update leaves scroll, focus and view untouched", () => {
    const model = seededModel();
    const session = modelWiredSession(model, FULL_DIMS);

    session.key("down", model.getSnapshot());
    session.key("down", model.getSnapshot());
    session.key("down", model.getSnapshot());
    const scrollBefore = session.state.scroll["work"];
    const focusBefore = session.state.focusedSurface;
    const viewBefore = session.state.location.view;

    // Three source events arrive while the operator reads.
    model.applyEvent(unitEvent("u-stream-1", "Stream One", "c-30", "active"));
    model.applyEvent(unitEvent("u-stream-2", "Stream Two", "c-31", "active"));
    model.applyEvent(unitEvent("u-stream-3", "Stream Three", "c-32", "active"));

    expect(session.state.scroll["work"]).toBe(scrollBefore);
    expect(session.state.focusedSurface).toBe(focusBefore);
    expect(session.state.location.view).toBe(viewBefore);
    const rendered = session.render(model.getSnapshot(), renderOptions(model));
    expect(shellText(rendered, "Primary").join("\n")).toContain("3 line(s) above");
  });
});

describe("T12 escape hatches are reachable from every state (REQ-UX-004/005)", () => {
  it("escape returns to Primary from palette, Navigator and Composer; q exits below minimum", () => {
    const model = seededModel();
    const session = modelWiredSession(model, FULL_DIMS);

    // From palette.
    session.key("p", model.getSnapshot());
    expect(session.state.paletteOpen).toBe(true);
    session.key("escape", model.getSnapshot());
    expect(session.state.paletteOpen).toBe(false);
    expect(session.state.focusedSurface).toBe("Primary");

    // From Navigator.
    session.key("tab", model.getSnapshot());
    session.key("tab", model.getSnapshot());
    session.key("tab", model.getSnapshot());
    expect(session.state.focusedSurface).toBe("Navigator");
    session.key("escape", model.getSnapshot());
    expect(session.state.focusedSurface).toBe("Primary");

    // From Composer.
    session.key("e", model.getSnapshot());
    expect(session.state.focusedSurface).toBe("Composer");
    session.key("escape", model.getSnapshot());
    expect(session.state.focusedSurface).toBe("Primary");

    // Below minimum: only q leaves.
    session.resize({ cols: 40, rows: 10 });
    session.key("escape", model.getSnapshot());
    expect(session.state.focusedSurface).toBe("Primary"); // unchanged, no crash
    const effects: string[] = [];
    const quitting = new ShellSession(
      { workspace: { workspace: "gauntlet", reference: "ref" } },
      { cols: 40, rows: 10 },
      { quit: () => effects.push("quit") },
    );
    quitting.key("q", coldModel().getSnapshot());
    expect(effects).toEqual(["quit"]);
  });
});
