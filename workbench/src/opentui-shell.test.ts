import { describe, expect, it } from "bun:test";
import { FocusRef } from "./focus";
import type { Dimensions } from "./geometry";
import { calculateColumns, classifyGeometry } from "./geometry";
import type { ShellInteractions, ViewId } from "./shell";
import {
  assertNoRoadmapSurfaces,
  availableViews,
  containsMotionFrames,
  navigatorOutline,
  ROADMAP_VIEW_IDS,
  RoadmapSurfaceError,
  registerShellView,
  ShellSession,
} from "./shell";
import {
  COMPACT_DIMS,
  FULL_DIMS,
  modelWiredSession,
  recordingInteractions,
  renderOptions,
  seededModel,
  shellText,
  unitEvent,
} from "./testing/shell-harness";
import { runningIndicatorFrames } from "./theme";
import { defaultViewRegistry } from "./views-chat-trace";

/**
 * T12 opentui-shell suites: geometry floors (DSN-007, REQ-UX-005), the
 * one-identity placements falsifier (DSN-006, prereg P1), Inspector record
 * objects (DSN-008), keyboard-only routing, the provider-gated registry with
 * the DSN-015 roadmap-surface tooth, and the DSN-013 reduced-motion mode.
 */

function sessionAt(dims: Dimensions, interactions: ShellInteractions = {}): ShellSession {
  return new ShellSession(
    { workspace: { workspace: "gauntlet", reference: "ref-1d457ec9", run: "R17" } },
    dims,
    interactions,
  );
}

describe("T12 shell geometry floors (DSN-007, REQ-UX-005)", () => {
  it("renders the FULL three-surface layout at 120x40 — dropping the Inspector above 120 cols is nonconforming", () => {
    const model = seededModel();
    const session = modelWiredSession(model, FULL_DIMS);
    const rendered = session.render(model.getSnapshot(), renderOptions(model));

    expect(classifyGeometry(FULL_DIMS)).toBe("Full");
    expect(rendered.geometryClass).toBe("Full");
    const ids = rendered.surfaces.map((surface) => surface.id);
    expect(ids).toContain("Navigator");
    expect(ids).toContain("Primary");
    expect(ids).toContain("Inspector");
    expect(ids).toContain("Composer");

    // Column allocation matches the geometry calculator exactly.
    const columns = calculateColumns(FULL_DIMS);
    const navigator = rendered.surfaces.find((surface) => surface.id === "Navigator");
    const primary = rendered.surfaces.find((surface) => surface.id === "Primary");
    const inspector = rendered.surfaces.find((surface) => surface.id === "Inspector");
    expect(navigator?.width).toBe(columns.navigatorWidth);
    expect(primary?.width).toBe(columns.primaryWidth);
    expect(inspector?.width).toBe(columns.inspectorWidth);

    // The three surfaces persist across the whole 120+ column band.
    for (const cols of [120, 125, 130, 135, 140]) {
      const atCols: Dimensions = { cols, rows: 40 };
      session.resize(atCols);
      const renderedAtCols = session.render(model.getSnapshot(), renderOptions(model));
      expect(classifyGeometry(atCols)).toBe(cols >= 140 ? "Wide" : "Full");
      expect(renderedAtCols.surfaces.map((surface) => surface.id)).toContain("Inspector");
    }
  });

  it("renders ONE primary pane at 80x24 (Navigator/Inspector absent, Case+attention retained)", () => {
    const model = seededModel();
    const session = modelWiredSession(model, COMPACT_DIMS);
    const rendered = session.render(model.getSnapshot(), renderOptions(model));

    expect(classifyGeometry(COMPACT_DIMS)).toBe("Compact");
    const ids = rendered.surfaces.map((surface) => surface.id);
    expect(ids).toEqual(["Primary", "Composer"]);
    expect(rendered.surfaces.find((surface) => surface.id === "Primary")?.width).toBe(80);

    // REQ-UX-005: Case, health, attention and action destination survive.
    const headerText = rendered.header.spans.map((span) => span.text).join("");
    expect(headerText).toContain("gauntlet/ref-1d457ec9");
    expect(headerText).toContain("gen gen-t12");
    expect(shellText(rendered, "Primary").join("\n")).toContain("BLOCKED");
  });

  it("renders the recoverable compact-size prompt below 80x24 naming sizes with a reachable exit", () => {
    const model = seededModel();
    const session = modelWiredSession(model, { cols: 70, rows: 18 });
    const rendered = session.render(model.getSnapshot(), renderOptions(model));

    expect(rendered.geometryClass).toBe("BelowMinimum");
    expect(rendered.surfaces).toEqual([]);
    expect(rendered.prompt).not.toBeNull();
    const promptText = (rendered.prompt?.lines ?? [])
      .map((line) => line.spans.map((span) => span.text).join(""))
      .join("\n");
    expect(promptText).toContain("Gauntlet needs at least 80x24. This terminal is 70x18.");
    expect(promptText).toContain("press q to quit");
    expect(rendered.prompt?.exitKey).toBe("q");
  });

  it("teeth: shrink below minimum and grow back re-enters with the semantic location intact and no admitted work cancelled", () => {
    const { interactions, journal } = recordingInteractions();
    const model = seededModel();
    const session = sessionAt(FULL_DIMS, interactions);

    // Establish a semantic location: focus u-far in the Navigator.
    // Cycle Primary → Inspector → Composer → Navigator.
    session.key("tab", model.getSnapshot());
    session.key("tab", model.getSnapshot());
    session.key("tab", model.getSnapshot());
    expect(session.state.focusedSurface).toBe("Navigator");
    const farIndex = navigatorOutline(model.getSnapshot()).findIndex(
      (row) => row.id === "outline.unit.u-far",
    );
    for (let i = 0; i < farIndex; i += 1) {
      session.key("down", model.getSnapshot());
    }
    session.key("enter", model.getSnapshot());
    const before = session.state.location;
    expect(before.focus?.id).toBe("focus:gauntlet:WorkItem:u-far");
    const effectsBeforeResize = journal.calls.length;
    expect(effectsBeforeResize).toBeGreaterThan(0);

    // Shrink below minimum: prompt; only q acts; no further effect fires.
    session.resize({ cols: 60, rows: 16 });
    session.key("enter", model.getSnapshot());
    session.key("p", model.getSnapshot());
    expect(session.render(model.getSnapshot(), renderOptions(model)).prompt).not.toBeNull();
    expect(journal.calls.length).toBe(effectsBeforeResize);

    // Grow back to Compact, then Full: location intact.
    session.resize(COMPACT_DIMS);
    expect(session.state.location).toEqual(before);
    session.resize(FULL_DIMS);
    expect(session.state.location).toEqual(before);
    expect(session.state.location.view).toBe("work");
    // Resize never emits effects (nothing cancelled, nothing re-admitted).
    expect(journal.calls.length).toBe(effectsBeforeResize);
  });
});

describe("T12 falsifier: one source identity across four placements (DSN-006, prereg P1)", () => {
  it("Navigator, Work, Agent placement and Inspector all update through ONE snapshot when source state changes once", () => {
    const model = seededModel();
    const unitRef = FocusRef.for("gauntlet", "WorkItem", "u-root");

    // The host declares three placements of ONE FocusRef: Navigator outline,
    // Work row, and an agent-context alias placement.
    model.placeFocus("Navigator", "outline", unitRef);
    model.placeFocus("Primary", "work", unitRef);
    model.placeFocus("Primary", "agents", unitRef, { relation_role: "executed-by" });
    model.selectFocus(unitRef);

    const session = modelWiredSession(model, FULL_DIMS);
    const snapshot = model.getSnapshot();

    // ONE identity: all placements reference the same interned FocusRef.
    for (const placement of snapshot.placements) {
      expect(placement.focus).toBe(unitRef);
    }
    expect(snapshot.focus).toBe(unitRef);

    const rendered = session.render(snapshot, renderOptions(model));
    const navigatorText = shellText(rendered, "Navigator").join("\n");
    const primaryText = shellText(rendered, "Primary").join("\n");
    const inspectorText = shellText(rendered, "Inspector").join("\n");
    expect(navigatorText).toContain("Root Unit");
    expect(primaryText).toContain("Root Unit");
    expect(inspectorText).toContain("Root Unit");

    // Switch to the Agents view through the palette: same placement, same
    // identity, rendered as an ALIAS row — not a second object.
    session.key("p", model.getSnapshot());
    const entries =
      session.render(model.getSnapshot(), renderOptions(model)).palette?.entries ?? [];
    const agentsIndex = entries.findIndex(
      (entry) => entry.type === "view" && entry.view === "agents",
    );
    expect(agentsIndex).toBeGreaterThanOrEqual(0);
    for (let i = 0; i < agentsIndex; i += 1) {
      session.key("down", model.getSnapshot());
    }
    session.key("enter", model.getSnapshot());
    expect(session.state.location.view).toBe("agents");
    const agentsLines = shellText(
      session.render(model.getSnapshot(), renderOptions(model)),
      "Primary",
    );
    const agentsAlias = agentsLines.find((line) => line.includes("≈"));
    expect(agentsAlias).toBeDefined();
    expect(agentsAlias).toContain("executed-by");
    expect(agentsAlias).toContain("Root Unit");

    // Update source state ONCE.
    model.applyEvent(unitEvent("u-root", "Root Unit v2", "c-10", "active"));

    const updated = session.render(model.getSnapshot(), renderOptions(model));
    const updatedSnapshot = model.getSnapshot();
    // Identity unchanged through the update.
    expect(updatedSnapshot.focus).toBe(unitRef);
    for (const placement of updatedSnapshot.placements) {
      expect(placement.focus).toBe(unitRef);
    }
    // Every placement reflects the new state; no placement kept a stale copy:
    // any line still naming the unit names the NEW state.
    for (const surfaceId of ["Navigator", "Primary", "Inspector"] as const) {
      for (const line of shellText(updated, surfaceId)) {
        if (line.includes("Root Unit")) {
          expect(line).toContain("Root Unit v2");
        }
      }
    }
    expect(shellText(updated, "Primary").join("\n")).toContain("Root Unit v2");
  });
});

describe("T12 Inspector sections (DSN-008, REQ-UX-003)", () => {
  it("renders evidence and authority as inspectable record objects, never badges", () => {
    const model = seededModel();
    const session = modelWiredSession(model, FULL_DIMS);
    // Cycle Primary → Inspector → Composer → Navigator.
    session.key("tab", model.getSnapshot());
    session.key("tab", model.getSnapshot());
    session.key("tab", model.getSnapshot());
    expect(session.state.focusedSurface).toBe("Navigator");
    const rootIndex = navigatorOutline(model.getSnapshot()).findIndex(
      (row) => row.id === "outline.unit.u-root",
    );
    for (let i = 0; i < rootIndex; i += 1) {
      session.key("down", model.getSnapshot());
    }
    session.key("enter", model.getSnapshot()); // select u-root

    const rendered = session.render(model.getSnapshot(), renderOptions(model));
    const inspectorText = shellText(rendered, "Inspector").join("\n");

    for (const section of [
      "IDENTITY",
      "STATE",
      "RELATIONS",
      "AUTHORITY",
      "EVIDENCE",
      "PROVENANCE",
      "AFFORDANCES",
    ]) {
      expect(inspectorText).toContain(section);
    }

    // Evidence records carry claim/producer/result/identity/provenance.
    expect(inspectorText).toContain("VERIFIED");
    expect(inspectorText).toContain("UNVERIFIED");
    expect(inspectorText).toContain("ev-1");
    expect(inspectorText).toContain("ev-2");
    expect(inspectorText).toContain("report evidence at");

    // Authority record: fields the projection does not expose say unavailable.
    expect(inspectorText).toContain("unavailable");

    // Relations derived from the snapshot with provenance.
    expect(inspectorText).toContain("has_evidence");
  });

  it("exposes honest sections for focus kinds without projected rows", () => {
    const model = seededModel();
    const referenceRef = FocusRef.for("gauntlet", "Reference", "generation:gen-t12");
    model.selectFocus(referenceRef);
    const session = modelWiredSession(model, FULL_DIMS);
    const rendered = session.render(model.getSnapshot(), renderOptions(model));
    const inspectorText = shellText(rendered, "Inspector").join("\n");
    expect(inspectorText).toContain("no projected state for this focus kind");
  });
});

describe("T12 keyboard-only navigation (REQ-UX-005)", () => {
  it("routes surfaces, selection, view switching, hoist/de-hoist, palette and quit without any pointer", () => {
    const { interactions, journal } = recordingInteractions();
    const model = seededModel();
    const session = sessionAt(FULL_DIMS, interactions);

    // Tab cycles Primary → Inspector → Composer → Navigator → Primary.
    session.key("tab", model.getSnapshot());
    expect(session.state.focusedSurface).toBe("Inspector");
    session.key("tab", model.getSnapshot());
    expect(session.state.focusedSurface).toBe("Composer");
    session.key("tab", model.getSnapshot());
    expect(session.state.focusedSurface).toBe("Navigator");
    session.key("tab", model.getSnapshot());
    expect(session.state.focusedSurface).toBe("Primary");
    session.key("tab", model.getSnapshot());
    expect(session.state.focusedSurface).toBe("Inspector");
    // Composer → Navigator for outline selection.
    session.key("tab", model.getSnapshot());
    session.key("tab", model.getSnapshot());
    expect(session.state.focusedSurface).toBe("Navigator");

    // Select u-far through the outline.
    const farIndex = navigatorOutline(model.getSnapshot()).findIndex(
      (row) => row.id === "outline.unit.u-far",
    );
    for (let i = 0; i < farIndex; i += 1) {
      session.key("down", model.getSnapshot());
    }
    session.key("enter", model.getSnapshot());
    expect(
      journal.calls.some((call) => call.effect === "select_focus" && call.detail.includes("u-far")),
    ).toBe(true);

    // View switching (Navigator → Primary first).
    session.key("tab", model.getSnapshot());
    session.key("2", model.getSnapshot());
    expect(session.state.location.view).toBe("chat");
    session.key("3", model.getSnapshot());
    expect(session.state.location.view).toBe("trace");
    session.key("1", model.getSnapshot());
    expect(session.state.location.view).toBe("work");

    // Hoist (shell focus falls back to location focus), then one-step return.
    session.key("h", model.getSnapshot());
    expect(journal.calls.at(-1)?.effect).toBe("hoist");
    session.key("u", model.getSnapshot());
    expect(journal.calls.at(-1)?.effect).toBe("dehoist");

    // Palette summon/dismiss.
    session.key("p", model.getSnapshot());
    expect(session.state.paletteOpen).toBe(true);
    session.key("escape", model.getSnapshot());
    expect(session.state.paletteOpen).toBe(false);

    // Quit is reachable.
    session.key("q", model.getSnapshot());
    expect(journal.calls.at(-1)?.effect).toBe("quit");
  });

  it("preserves per-view scroll offsets and focus across view switches and stream updates", () => {
    const model = seededModel();
    const session = modelWiredSession(model, FULL_DIMS);

    // Scroll the Work view down twice.
    session.key("down", model.getSnapshot());
    session.key("down", model.getSnapshot());
    expect(session.state.scroll["work"]).toBe(2);

    // Switch to Chat, scroll there once.
    session.key("2", model.getSnapshot());
    session.key("down", model.getSnapshot());
    expect(session.state.scroll["chat"]).toBe(1);

    // Back to Work: scroll anchored at 2.
    session.key("1", model.getSnapshot());
    expect(session.state.scroll["work"]).toBe(2);
    const rendered = session.render(model.getSnapshot(), renderOptions(model));
    expect(shellText(rendered, "Primary").join("\n")).toContain("2 line(s) above");

    // New stream output must not steal scroll position or focus.
    model.applyEvent(unitEvent("u-late", "Late Unit", "c-11", "active"));
    expect(session.state.scroll["work"]).toBe(2);
    expect(session.state.focusedSurface).toBe("Primary");
    const after = session.render(model.getSnapshot(), renderOptions(model));
    expect(shellText(after, "Primary").join("\n")).toContain("2 line(s) above");
  });
});

describe("T12 view registry and DSN-015 roadmap tooth", () => {
  it("gates lower surfaces on published providers with inspectable reasons", () => {
    const model = seededModel();
    const availability = availableViews(model.getSnapshot());
    const byView = new Map(availability.map((entry) => [entry.view, entry]));

    expect(byView.get("work")?.status).toBe("available");
    expect(byView.get("chat")?.status).toBe("available");
    expect(byView.get("trace")?.status).toBe("available");
    // Providers the model publishes.
    expect(byView.get("evidence")?.status).toBe("available");
    expect(byView.get("agents")?.status).toBe("available");
    expect(byView.get("attention")?.status).toBe("available");
    // Providers that do not exist yet: absent WITH reason, never fake tabs.
    for (const view of ["files", "kernel", "problems", "terminal", "domain"] as const) {
      expect(byView.get(view)?.status).toBe("absent");
      expect(byView.get(view)?.reason).toContain("provider");
    }
  });

  it("contains no Harness View and refuses to register roadmap surfaces (DSN-015)", () => {
    const registry = defaultViewRegistry();
    expect(Object.keys(registry)).not.toContain("harness");
    expect(ROADMAP_VIEW_IDS).toEqual(["harness"]);
    assertNoRoadmapSurfaces(registry); // must not throw

    // Runtime defence for a typed-impossible id.
    const roadmapId = "harness" as unknown as ViewId;
    expect(() => registerShellView(registry, roadmapId, registry.work)).toThrow(
      RoadmapSurfaceError,
    );
    expect(() => registerShellView(registry, roadmapId, registry.work)).toThrow("roadmap");
  });
});

describe("T12 reduced motion mode (DSN-013)", () => {
  it("exists, suppresses motion frames, and keeps every state statically labelled", () => {
    expect(runningIndicatorFrames("reduced")).toEqual([]);
    expect(runningIndicatorFrames("full").length).toBeGreaterThan(0);

    const model = seededModel();
    const session = new ShellSession(
      { workspace: { workspace: "gauntlet", reference: "ref-1d457ec9" }, motion: "reduced" },
      FULL_DIMS,
    );
    const rendered = session.render(model.getSnapshot(), renderOptions(model));
    expect(rendered.motion).toBe("reduced");
    expect(containsMotionFrames(rendered)).toBe(false);
    const helpText = rendered.help
      .map((line) => line.spans.map((span) => span.text).join(""))
      .join("\n");
    expect(helpText).toContain("reduced motion");

    // Deterministic: two renders of the same state are identical without motion.
    const again = session.render(model.getSnapshot(), renderOptions(model));
    expect(again.surfaces).toEqual(rendered.surfaces);
  });
});
