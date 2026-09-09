import { describe, expect, it } from "bun:test";
import { type AuthoritativeEventFrame, SqliteProjectionStore } from "./projection-store";

describe("T06 ProjectionGeneration & Cursor Reconciliation (REQ-DATA-002..004, REQ-STATE-002)", () => {
  it("manages generation lifecycle and atomically commits events and cursors", () => {
    const store = new SqliteProjectionStore(":memory:");
    const gen = store.startGeneration("gen-001");

    expect(gen.id).toBe("gen-001");
    expect(gen.status).toBe("syncing");

    const event1: AuthoritativeEventFrame = {
      generation_id: "gen-001",
      source_id: "gauntlet",
      cursor: "c-001",
      event_type: "unit_updated",
      payload: {
        unit_id: "unit-alpha",
        title: "Alpha Unit",
        phase: "discovery",
        status: "in_progress",
        authority: "normal",
        blast_radius: "isolated",
      },
      timestamp_ms: 1000,
    };

    const res1 = store.applyEvent(event1);
    expect(res1.applied).toBe(true);

    const snapshot1 = store.getSnapshot();
    expect(snapshot1.generation).toBe("gen-001");
    expect(snapshot1.source_cursors["gauntlet"]).toBe("c-001");
    expect(snapshot1.units.length).toBe(1);
    expect(snapshot1.units[0]?.unit_id).toBe("unit-alpha");
    expect(snapshot1.units[0]?.source_cursor).toBe("c-001");

    store.markGenerationReady("gen-001");
    expect(store.getActiveGeneration()?.status).toBe("ready");
    store.close();
  });

  it("Attack 2 (REQ-DATA-003, REQ-STATE-002): rejects delayed frame from superseded generation", () => {
    const store = new SqliteProjectionStore(":memory:");

    // 1. Initial generation G1
    store.startGeneration("gen-g1");
    store.applyEvent({
      generation_id: "gen-g1",
      source_id: "gauntlet",
      cursor: "cur-g1-1",
      event_type: "unit_updated",
      payload: { unit_id: "unit-1", title: "G1 Title", status: "open" },
      timestamp_ms: 1000,
    });

    const snapG1 = store.getSnapshot();
    expect(snapG1.generation).toBe("gen-g1");
    expect(snapG1.source_cursors["gauntlet"]).toBe("cur-g1-1");
    expect(snapG1.units[0]?.title).toBe("G1 Title");

    // 2. Reconnect occurs -> new generation G2 starts
    store.startGeneration("gen-g2");
    store.applyEvent({
      generation_id: "gen-g2",
      source_id: "gauntlet",
      cursor: "cur-g2-1",
      event_type: "unit_updated",
      payload: { unit_id: "unit-1", title: "G2 Reconnected Title", status: "active" },
      timestamp_ms: 2000,
    });

    const snapG2Before = store.getSnapshot();
    expect(snapG2Before.generation).toBe("gen-g2");
    expect(snapG2Before.source_cursors["gauntlet"]).toBe("cur-g2-1");
    expect(snapG2Before.units[0]?.title).toBe("G2 Reconnected Title");

    // 3. ATTACK: Deliver a delayed G1 update with a later-looking clock timestamp (timestamp_ms: 999999)
    const delayedG1Frame: AuthoritativeEventFrame = {
      generation_id: "gen-g1", // superseded generation!
      source_id: "gauntlet",
      cursor: "cur-g1-late",
      event_type: "unit_updated",
      payload: { unit_id: "unit-1", title: "Stale G1 Clashing Title", status: "poisoned" },
      timestamp_ms: 999999, // artificially late timestamp
    };

    const result = store.applyEvent(delayedG1Frame);

    // Defense: update MUST be refused
    expect(result.applied).toBe(false);
    expect(result.reason).toContain("superseded_generation");

    // Defense: Active G2 snapshot, units, and cursors remain completely untouched
    const snapG2After = store.getSnapshot();
    expect(snapG2After.generation).toBe("gen-g2");
    expect(snapG2After.source_cursors["gauntlet"]).toBe("cur-g2-1");
    expect(snapG2After.units[0]?.title).toBe("G2 Reconnected Title");
    expect(snapG2After.units[0]?.status).toBe("active");

    store.close();
  });
});
