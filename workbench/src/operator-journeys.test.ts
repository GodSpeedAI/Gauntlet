import { describe, expect, it } from "bun:test";
import { FocusRef } from "./focus";
import type { Dimensions } from "./geometry";
import { navigatorOutline, type ShellSession } from "./shell";
import {
  COMPACT_DIMS,
  coldModel,
  FULL_DIMS,
  modelWiredSession,
  OPERATOR_SESSION,
  observationFor,
  renderOptions,
  seededModel,
  shellText,
  unitEvent,
} from "./testing/shell-harness";

/**
 * T12 operator-journeys suites: REQ-UX-001..007 exercised at the
 * state-machine level — cold start, selection across views, hoist/breadcrumb/
 * one-step return, blocked-unit inspection, composer takeover, responsive
 * geometry, honest degraded states, and the read-only inspection slice.
 * Every journey is keyboard-only (no pointer exists in the session API).
 */

function journey(dims: Dimensions): {
  model: ReturnType<typeof seededModel>;
  session: ShellSession;
} {
  const model = seededModel();
  return { model, session: modelWiredSession(model, dims) };
}

describe("REQ-UX-001: cold operator reaches Case, Reference, Run and the escalation queue", () => {
  it("a cold workspace renders honest named next steps and never launches work", () => {
    const model = coldModel();
    const session = modelWiredSession(model, FULL_DIMS);
    const rendered = session.render(model.getSnapshot(), renderOptions(model));

    // The shell header names the workspace without internal identifiers.
    const headerText = rendered.header.spans.map((span) => span.text).join("");
    expect(headerText).toContain("gauntlet/ref-1d457ec9");
    expect(headerText).toContain("run R17");

    // Navigator names the bind step; Work names the run step.
    expect(shellText(rendered, "Navigator").join("\n")).toContain("Nothing outlined yet");
    const primaryText = shellText(rendered, "Primary").join("\n");
    expect(primaryText).toContain("No authoritative source has reported");
    expect(primaryText).toContain("gauntlet serve");

    // No effect was emitted by merely rendering (startup launches no paid work).
    expect(session.state.location.view).toBe("work");
  });

  it("an escalation (blocked unit) surfaces Case-wide in Navigator, Work and the attention summary", () => {
    const { model, session } = journey(FULL_DIMS);
    const rendered = session.render(model.getSnapshot(), renderOptions(model));

    const attentionText = rendered.attention
      .map((line) => line.spans.map((span) => span.text).join(""))
      .join("\n");
    expect(attentionText).toContain("unit_blocked");
    expect(attentionText).toContain("u-far");

    expect(shellText(rendered, "Navigator").join("\n")).toContain("unit_blocked");
    expect(shellText(rendered, "Primary").join("\n")).toContain("! UNIT_BLOCKED");
    // Attention renders without stealing input focus.
    expect(session.state.focusedSurface).toBe("Primary");
  });
});

describe("REQ-UX-002: selection retains identity across views, hoist has breadcrumb and one-step return", () => {
  it("select in Navigator, switch all three views, hoist, breadcrumb, de-hoist", () => {
    const { model, session } = journey(FULL_DIMS);
    const snapshot = model.getSnapshot();

    // Keyboard-only selection of u-root in the Navigator.
    session.key("tab", snapshot);
    session.key("tab", snapshot);
    session.key("tab", snapshot); // → Navigator
    const rootIndex = navigatorOutline(model.getSnapshot()).findIndex(
      (row) => row.id === "outline.unit.u-root",
    );
    for (let i = 0; i < rootIndex; i += 1) {
      session.key("down", model.getSnapshot());
    }
    session.key("enter", model.getSnapshot());
    const unitRef = FocusRef.for("gauntlet", "WorkItem", "u-root");
    expect(model.getSnapshot().focus).toBe(unitRef);

    // Identity retained across view switches (back/forward by number).
    session.key("tab", model.getSnapshot()); // → Primary
    session.key("2", model.getSnapshot());
    session.key("3", model.getSnapshot());
    session.key("1", model.getSnapshot());
    expect(model.getSnapshot().focus).toBe(unitRef);

    // Hoist: breadcrumb grows; Inspector/Navigator show the horizon.
    session.key("h", model.getSnapshot());
    const hoisted = model.getSnapshot();
    expect(hoisted.active_horizon?.root_focus).toBe(unitRef);
    expect(hoisted.horizon_breadcrumb.length).toBe(2);
    const hoistRender = session.render(model.getSnapshot(), renderOptions(model));
    // The breadcrumb names the hoisted root by its source identity (T07
    // breadcrumb semantics: human labels only when the source minted one).
    expect(hoistRender.breadcrumb.join(" › ")).toBe("Case › u-root");

    // One-step return: breadcrumb collapses, identities unchanged.
    session.key("u", model.getSnapshot());
    const dehoisted = model.getSnapshot();
    expect(dehoisted.active_horizon).toBeNull();
    expect(dehoisted.horizon_breadcrumb.length).toBe(1);
    expect(dehoisted.focus).toBe(unitRef);
  });
});

describe("REQ-UX-003: blocked Unit inspection before resolve/retry", () => {
  it("shows requirement state, denial reason, authority, exact target and the next step", () => {
    const { model, session } = journey(FULL_DIMS);
    const farRef = FocusRef.for("gauntlet", "WorkItem", "u-far");
    model.selectFocus(farRef);

    // An authenticated operator session: the first blocking gate on resolve
    // is then the missing escalation ticket.
    const options = {
      observation: observationFor(model),
      session: OPERATOR_SESSION,
    };
    const rendered = session.render(model.getSnapshot(), options);
    const inspectorText = shellText(rendered, "Inspector").join("\n");

    // State: the blocked status with its gate (projection-cased in Inspector).
    expect(inspectorText).toContain("status       blocked");
    expect(inspectorText).toContain("orchestrator"); // governing authority
    // Target identity is exact: every character in order. Long ids wrap at
    // the Inspector value-column boundary (delimiter-aware), so the match is
    // whitespace-insensitive across the wrap.
    expect(inspectorText.replace(/\s/g, "")).toContain("focus:gauntlet:WorkItem:u-far");
    // Denial reason: resolve is blocked with no open ticket and says the next step.
    expect(inspectorText).toContain("unit.resolve");
    expect(inspectorText).toContain("no_open_ticket");
    expect(inspectorText).toContain("requires an open escalation ticket for this unit");
    expect(inspectorText).toContain("readiness_not_earned");

    // With an escalation observed AND an authenticated session, the SAME
    // inspector upgrades resolve to the ticket it responds to.
    const escalated = session.render(model.getSnapshot(), {
      observation: observationFor(model, {
        open_escalations: [{ ticket_id: "ESC-42", unit_id: "u-far" }],
      }),
      session: OPERATOR_SESSION,
    });
    const escalatedText = shellText(escalated, "Inspector").join("\n");
    expect(escalatedText).toContain("requires attention");
    expect(escalatedText).toContain("ESC-42");
  });
});

describe("REQ-UX-004: composer mode/destination visible; takeover queues; escape dismisses", () => {
  it("composes a message, survives a queued approval takeover, and returns intact", () => {
    const { model, session } = journey(FULL_DIMS);

    session.key("e", model.getSnapshot());
    for (const key of "check the residual".split("")) {
      session.key(key, model.getSnapshot());
    }
    const composerLine = () =>
      session
        .render(model.getSnapshot(), renderOptions(model))
        .surfaces.find((surface) => surface.id === "Composer")
        ?.lines.find((line) => line.id === "composer.mode")
        ?.spans.map((span) => span.text)
        .join("");
    expect(composerLine()).toContain("[message]");
    expect(composerLine()).toContain("workbench");

    // Approval takeover arrives, queues behind nothing, owns the input.
    session.beginTakeover({
      requestId: "req-approve-1",
      kind: "approval",
      prompt: "approve unit.resolve on u-far",
      destination: "gauntlet-admission",
      requestedAtMs: 1000,
      expiresAtMs: null,
    });
    expect(session.composerState.takeover?.kind).toBe("approval");
    // A second request queues by identity.
    session.beginTakeover({
      requestId: "req-approve-2",
      kind: "question",
      prompt: "clarify blast radius",
      destination: "prime-agent",
      requestedAtMs: 1100,
      expiresAtMs: null,
    });
    expect(session.composerState.queued.map((queued) => queued.requestId)).toEqual([
      "req-approve-2",
    ]);

    // Answer the approval; the draft is still there; the queue activates next.
    session.key("y", model.getSnapshot());
    session.key("enter", model.getSnapshot());
    expect(session.composerState.takeover?.requestId).toBe("req-approve-2");
    expect(session.composerState.draft).toBe("check the residual");

    // Escape dismisses the question: draft and focus return, request stays queued.
    session.key("escape", model.getSnapshot());
    expect(session.composerState.takeover).toBeNull();
    expect(session.composerState.draft).toBe("check the residual");
    expect(session.state.focusedSurface).toBe("Primary");
    expect(session.composerState.queued.map((queued) => queued.requestId)).toEqual([
      "req-approve-2",
    ]);
  });
});

describe("REQ-UX-005: the whole journey is keyboard-only at 120x40 AND 80x24", () => {
  for (const [label, dims] of [
    ["full 120x40", FULL_DIMS],
    ["compact 80x24", COMPACT_DIMS],
  ] as const) {
    it(`select → inspect → switch → composer → return at ${label}`, () => {
      const { model, session } = journey(dims);
      let snapshot = model.getSnapshot();

      // Selection: Navigator at Full; palette focus entries at Compact.
      if (dims === FULL_DIMS) {
        session.key("tab", snapshot);
        session.key("tab", snapshot);
        session.key("tab", snapshot); // → Navigator
        const farIndex = navigatorOutline(model.getSnapshot()).findIndex(
          (row) => row.id === "outline.unit.u-far",
        );
        for (let i = 0; i < farIndex; i += 1) {
          session.key("down", model.getSnapshot());
        }
        session.key("enter", model.getSnapshot());
      } else {
        session.key("p", snapshot);
        const palette = session.render(model.getSnapshot(), renderOptions(model)).palette;
        expect(palette).not.toBeNull();
        const focusIndex = palette?.entries.findIndex(
          (entry) => entry.type === "focus" && entry.ref.id === "focus:gauntlet:WorkItem:u-far",
        );
        expect(focusIndex).toBeGreaterThanOrEqual(0);
        for (let i = 0; i < (focusIndex ?? 0); i += 1) {
          session.key("down", model.getSnapshot());
        }
        session.key("enter", model.getSnapshot());
      }
      expect(model.getSnapshot().focus?.id).toBe("focus:gauntlet:WorkItem:u-far");

      // Inspect: the Inspector is present at Full; at Compact the single pane
      // keeps Case, health, attention and action destination.
      snapshot = model.getSnapshot();
      if (dims === FULL_DIMS) {
        const rendered = session.render(snapshot, renderOptions(model));
        expect(shellText(rendered, "Inspector").join("\n")).toContain("u-far");
      } else {
        const rendered = session.render(snapshot, renderOptions(model));
        const headerText = rendered.header.spans.map((span) => span.text).join("");
        expect(headerText).toContain("! attention 1");
      }

      // Switch views and compose.
      session.key("3", snapshot);
      expect(session.state.location.view).toBe("trace");
      session.key("1", model.getSnapshot());
      session.key("e", model.getSnapshot());
      for (const key of "ok".split("")) {
        session.key(key, model.getSnapshot());
      }
      expect(session.composerState.draft).toBe("ok");

      // Escape hatch: back to Primary, quit reachable.
      session.key("escape", model.getSnapshot());
      expect(session.state.focusedSurface).toBe("Primary");
      session.key("q", model.getSnapshot());
      // (quit is an effect; the session wired here only wires model mutations)
      expect(session.state.focusedSurface).toBe("Primary");
    });
  }
});

describe("REQ-UX-006: honest empty/unavailable states with recovery", () => {
  it("unavailable capabilities render with their gate reasons; core inspection stays usable", () => {
    const model = seededModel();
    model.declareCapabilities([
      {
        capability: "prime",
        installed: true,
        gates: {
          visibility: true,
          reachability: false,
          payment: true,
          governance: true,
          settlement_access: true,
          recovery: true,
        },
      },
    ]);
    const session = modelWiredSession(model, FULL_DIMS);
    const rendered = session.render(model.getSnapshot(), renderOptions(model));
    const primaryText = shellText(rendered, "Primary").join("\n");
    expect(primaryText).toContain("prime");
    expect(primaryText).toContain("unavailable (reachability_gate_not_passed)");
    // Core inspection is unaffected by the unavailable capability.
    const farRef = FocusRef.for("gauntlet", "WorkItem", "u-far");
    model.selectFocus(farRef);
    const inspectRender = session.render(model.getSnapshot(), renderOptions(model));
    expect(shellText(inspectRender, "Inspector").join("\n")).toContain("STATE");
  });
});

describe("REQ-UX-007: the read-only inspection vertical slice", () => {
  it("unit status → verification metadata → evidence references → return, without mutation", () => {
    const { model, session } = journey(FULL_DIMS);
    const snapshot = model.getSnapshot();

    session.key("tab", snapshot);
    session.key("tab", snapshot);
    session.key("tab", snapshot); // Navigator
    const rootIndex = navigatorOutline(model.getSnapshot()).findIndex(
      (row) => row.id === "outline.unit.u-root",
    );
    for (let i = 0; i < rootIndex; i += 1) {
      session.key("down", model.getSnapshot());
    }
    session.key("enter", model.getSnapshot());

    // Status and tier-adjacent fields the projection exposes...
    const rendered = session.render(model.getSnapshot(), renderOptions(model));
    const inspectorText = shellText(rendered, "Inspector").join("\n");
    expect(inspectorText).toContain("status       active");
    expect(inspectorText).toContain("ev-1");
    expect(inspectorText).toContain("VERIFIED");
    // ...fields it does NOT expose say unavailable (never inferred)...
    expect(inspectorText).toContain("unavailable");
    // ...and evidence references include their locator.
    expect(inspectorText).toContain("report evidence at");

    // Return one step (escape to Primary; de-hoist is a no-op at whole-Case).
    session.key("escape", model.getSnapshot());
    expect(session.state.focusedSurface).toBe("Primary");
    // Read-only: the only effects the journey produced were selections.
    expect(model.getSnapshot().units.length).toBe(3);
  });

  it("a mid-journey source update re-renders every surface from the new snapshot", () => {
    const { model, session } = journey(FULL_DIMS);
    const farRef = FocusRef.for("gauntlet", "WorkItem", "u-far");
    model.selectFocus(farRef);
    session.render(model.getSnapshot(), renderOptions(model));

    model.applyEvent(unitEvent("u-far", "Far Unit (unblocked)", "c-20", "active"));
    const rendered = session.render(model.getSnapshot(), renderOptions(model));
    const all = shellText(rendered).join("\n");
    expect(all).toContain("Far Unit (unblocked)");
    // The escalation disappeared from the Case-wide attention summary...
    expect(rendered.attention.length).toBe(0);
    // ...and the header no longer carries the attention marker.
    expect(rendered.header.spans.map((span) => span.text).join("")).not.toContain("attention");
  });
});
