import { describe, expect, it } from "bun:test";
import type { FullWorkbenchSnapshot } from "./workbench-model";
import { WorkbenchModel } from "./workbench-model";

describe("T07 WorkbenchModel & Immutable Snapshot Publishing (REQ-DATA-001, REQ-TUI-001)", () => {
  it("publishes immutable snapshots to presentation subscribers", () => {
    const model = new WorkbenchModel();
    model.startGeneration("gen-model-1");

    const snapshots: FullWorkbenchSnapshot[] = [];
    const unsubscribe = model.subscribe((snap) => {
      snapshots.push(snap);
    });

    // Initial snapshot published on subscription
    expect(snapshots.length).toBe(1);
    expect(snapshots[0]?.generation).toBe("gen-model-1");

    // Apply authoritative event
    model.applyEvent({
      generation_id: "gen-model-1",
      source_id: "gauntlet",
      cursor: "c-100",
      event_type: "unit_updated",
      payload: {
        unit_id: "u-1",
        title: "Model Test Unit",
        phase: "plan",
        status: "active",
      },
      timestamp_ms: 1000,
    });

    expect(snapshots.length).toBe(2);
    const latest = snapshots[1];
    expect(latest?.units.length).toBe(1);
    expect(latest?.units[0]?.title).toBe("Model Test Unit");
    expect(latest?.trace_records.length).toBe(1);

    // Verify snapshot immutability: attempts to push to arrays throw or fail
    expect(() => {
      // @ts-expect-error - testing runtime immutability
      latest?.trace_records.push({ trace_id: "fake" });
    }).toThrow();

    unsubscribe();

    // After unsubscribe, no further snapshots are received
    model.selectFocus("unit", "u-1");
    expect(snapshots.length).toBe(2);

    model.close();
  });

  it("handles interaction callbacks and updates snapshot focus/horizon", () => {
    const model = new WorkbenchModel();
    model.startGeneration("gen-model-2");

    let currentSnap = model.getSnapshot();
    model.subscribe((snap) => {
      currentSnap = snap;
    });

    expect(currentSnap.focus).toBeNull();
    expect(currentSnap.horizon).toBeNull();

    model.selectFocus("unit", "u-42");
    expect(currentSnap.focus).toEqual({ kind: "unit", id: "u-42" });

    model.selectHorizon("horizon-milestone-1");
    expect(currentSnap.horizon).toBe("horizon-milestone-1");

    model.close();
  });
});
