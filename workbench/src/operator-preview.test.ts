import { describe, expect, it } from "bun:test";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import type { RuntimeSnapshot } from "./client";
import { calculateColumns, classifyGeometry, renderBelowMinimumPrompt } from "./geometry";
import { createInitialState, previewReducer, renderPreview } from "./preview";

describe("T00A Operator Preview: Geometry & Layout Floors (REQ-UX-005, DSN-007)", () => {
  it("classifies 120x40 as Full layout with 3 simultaneous surfaces", () => {
    const geo = classifyGeometry({ cols: 120, rows: 40 });
    expect(geo).toBe("Full");

    const cols = calculateColumns({ cols: 120, rows: 40 });
    expect(cols.isMultiColumn).toBe(true);
    expect(cols.navigatorWidth).toBe(28);
    expect(cols.inspectorWidth).toBe(36);
    expect(cols.primaryWidth).toBe(56);
  });

  it("classifies 80x24 as Compact layout with single primary pane", () => {
    const geo = classifyGeometry({ cols: 80, rows: 24 });
    expect(geo).toBe("Compact");

    const cols = calculateColumns({ cols: 80, rows: 24 });
    expect(cols.isMultiColumn).toBe(false);
    expect(cols.primaryWidth).toBe(80);
  });

  it("classifies < 80x24 as BelowMinimum and renders the recoverable compact prompt", () => {
    const belowCols = classifyGeometry({ cols: 72, rows: 24 });
    const belowRows = classifyGeometry({ cols: 80, rows: 20 });
    expect(belowCols).toBe("BelowMinimum");
    expect(belowRows).toBe("BelowMinimum");

    const prompt = renderBelowMinimumPrompt({ cols: 72, rows: 20 });
    expect(prompt).toContain("Gauntlet needs at least 80x24. This terminal is 72x20.");
    expect(prompt).toContain(
      "Resize the window, or press q to quit.  (admitted work keeps running)",
    );
  });

  it("teeth: terminal resize down to <80x24 yields prompt; resize back restores layout and location", () => {
    let state = createInitialState({ cols: 120, rows: 40 });
    const dummySnapshot: RuntimeSnapshot = {
      run: "run-alpha-42",
      escalated: ["ESC-001"],
      units: [
        { unit_id: "AK-001", status: "pass", tier: "P1" },
        { unit_id: "AK-002", status: "fail", tier: "P2" },
      ],
      status: "connected",
      fetched_at: new Date().toISOString(),
    };
    state = previewReducer(state, { type: "LOAD_SNAPSHOT", snapshot: dummySnapshot });
    state = previewReducer(state, { type: "NAVIGATE_DOWN" }); // focus AK-002
    expect(state.focusedUnitId).toBe("AK-002");

    // 1. Shrink below minimum
    state = previewReducer(state, { type: "RESIZE", dimensions: { cols: 70, rows: 18 } });
    const renderedPrompt = renderPreview(state);
    expect(renderedPrompt).toContain("Gauntlet needs at least 80x24. This terminal is 70x18.");

    // 2. Grow back to 80x24 (Compact floor)
    state = previewReducer(state, { type: "RESIZE", dimensions: { cols: 80, rows: 24 } });
    const compactRender = renderPreview(state);
    expect(compactRender).toContain("80x24 COMPACT");
    expect(compactRender).toContain("AK-002"); // semantic location preserved!
    expect(state.focusedUnitId).toBe("AK-002");

    // 3. Grow to 120x40 (Full floor)
    state = previewReducer(state, { type: "RESIZE", dimensions: { cols: 120, rows: 40 } });
    const fullRender = renderPreview(state);
    expect(fullRender).toContain("120x40 FULL");
    expect(fullRender).toContain("NAVIGATOR");
    expect(fullRender).toContain("PRIMARY (WORK UNITS)");
    expect(fullRender).toContain("INSPECTOR");
    expect(fullRender).toContain("Unit: AK-002"); // Inspector reflects focused unit!
  });
});

describe("T00A Operator Preview: Keyboard Journey & Escalation-First Flow", () => {
  it("navigates units, inspects details, and returns with keyboard controls", () => {
    let state = createInitialState({ cols: 120, rows: 40 });
    const dummySnapshot: RuntimeSnapshot = {
      run: "run-beta-99",
      escalated: ["ESC-TICKET-7"],
      units: [
        { unit_id: "AK-101", status: "running", tier: "P0" },
        { unit_id: "AK-102", status: "blocked", tier: "P1" },
        { unit_id: "AK-103", status: "pass", tier: "P2" },
      ],
      status: "connected",
      fetched_at: new Date().toISOString(),
    };

    state = previewReducer(state, { type: "LOAD_SNAPSHOT", snapshot: dummySnapshot });
    expect(state.selectedUnitIndex).toBe(0);
    expect(state.focusedUnitId).toBe("AK-101");

    // Down navigation
    state = previewReducer(state, { type: "NAVIGATE_DOWN" });
    expect(state.selectedUnitIndex).toBe(1);
    expect(state.focusedUnitId).toBe("AK-102");

    // Inspect action
    state = previewReducer(state, { type: "INSPECT_FOCUSED" });
    expect(state.isInspecting).toBe(true);
    expect(state.activeSurface).toBe("Inspector");

    // Return action (Esc)
    state = previewReducer(state, { type: "RETURN_TO_LIST" });
    expect(state.isInspecting).toBe(false);
    expect(state.activeSurface).toBe("Primary");
    expect(state.focusedUnitId).toBe("AK-102"); // focus preserved on return
  });

  it("renders escalation-first in Navigator column (spec §20.2 / DSN-006)", () => {
    let state = createInitialState({ cols: 120, rows: 40 });
    state = previewReducer(state, {
      type: "LOAD_SNAPSHOT",
      snapshot: {
        run: "run-esc-1",
        escalated: ["ESC-A", "ESC-B"],
        units: [{ unit_id: "AK-01", status: "pass" }],
        status: "connected",
        fetched_at: new Date().toISOString(),
      },
    });

    const rendered = renderPreview(state);
    expect(rendered).toContain("! Escalations (2)");
    expect(rendered).toContain("• ESC-A");
    expect(rendered).toContain("• ESC-B");
  });

  it("renders explanation fields as unavailable in preview mode (T00A requirement)", () => {
    let state = createInitialState({ cols: 120, rows: 40 });
    state = previewReducer(state, {
      type: "LOAD_SNAPSHOT",
      snapshot: {
        run: "run-preview",
        escalated: [],
        units: [{ unit_id: "AK-999", status: "fail", tier: "P3" }],
        status: "connected",
        fetched_at: new Date().toISOString(),
      },
    });

    const rendered = renderPreview(state);
    expect(rendered).toContain("Explanation: unavailable");
    expect(rendered).toContain("(T01 read contract required)");
  });
});

describe("T00A Operator Preview: Empty & Disconnected Handling (Teeth)", () => {
  it("teeth: disconnected state provides clear next steps and zero crashes", () => {
    let state = createInitialState({ cols: 120, rows: 40 });
    state = previewReducer(state, {
      type: "LOAD_SNAPSHOT",
      snapshot: {
        run: null,
        escalated: [],
        units: [],
        status: "disconnected",
        fetched_at: new Date().toISOString(),
      },
    });

    const output = renderPreview(state);
    expect(output).toContain("DISCONNECTED (127.0.0.1:4000 unreachable)");
    expect(output).toContain("GAUNTLET_API_BIND=127.0.0.1:4000 gauntlet serve");
    expect(output).toContain("[q] Quit");
  });

  it("teeth: empty workspace provides clear next steps and zero crashes", () => {
    let state = createInitialState({ cols: 120, rows: 40 });
    state = previewReducer(state, {
      type: "LOAD_SNAPSHOT",
      snapshot: {
        run: "run-empty",
        escalated: [],
        units: [],
        status: "empty",
        fetched_at: new Date().toISOString(),
      },
    });

    const output = renderPreview(state);
    expect(output).toContain("EMPTY WORKSPACE (0 units found)");
    expect(output).toContain(
      "Execute a gauntlet run to materialize verification records and units.",
    );
  });
});

describe("T00A Operator Preview: Read-Only Contract", () => {
  it("teeth: mutation attempts are refused with explicit read-only notice", () => {
    let state = createInitialState({ cols: 120, rows: 40 });
    state = previewReducer(state, {
      type: "LOAD_SNAPSHOT",
      snapshot: {
        run: "run-ro",
        escalated: [],
        units: [{ unit_id: "AK-01", status: "pass" }],
        status: "connected",
        fetched_at: new Date().toISOString(),
      },
    });

    state = previewReducer(state, { type: "ATTEMPT_MUTATION", actionName: "resolve_ticket" });
    expect(state.lastActionMessage).toContain(
      "[READ-ONLY PREVIEW] Action 'resolve_ticket' is disabled",
    );

    const rendered = renderPreview(state);
    expect(rendered).toContain("[READ-ONLY PREVIEW]");
  });
});

describe("T00A Bootstrap Dependency Carve-out Discipline (Teeth)", () => {
  it("teeth: package.json admits zero REQ-BUN-001 overlap dependencies", () => {
    const pkgPath = resolve(import.meta.dir, "../package.json");
    const pkg = JSON.parse(readFileSync(pkgPath, "utf-8"));
    const allDeps = {
      ...pkg.dependencies,
      ...pkg.devDependencies,
    };

    // REQ-BUN-001 forbidden overlap set in bootstrap:
    // no local sqlite database, no PTY library, no process spawner, no external config parser
    const forbiddenPatterns = [
      "sqlite",
      "better-sqlite",
      "node-pty",
      "pty.js",
      "cross-spawn",
      "execa",
      "dotenv",
      "yaml",
      "toml",
      "express",
      "fastify",
      "ws",
    ];

    for (const dep of Object.keys(allDeps)) {
      for (const pattern of forbiddenPatterns) {
        expect(dep.toLowerCase()).not.toContain(pattern);
      }
    }
  });
});
