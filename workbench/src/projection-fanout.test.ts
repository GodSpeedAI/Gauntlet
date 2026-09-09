import { describe, expect, it } from "bun:test";
import type { AuthoritativeEventFrame } from "./projection-store";
import { WorkbenchModel } from "./workbench-model";

describe("T07 Multi-Projection Fan-Out & Teeth Attacks (REQ-DATA-005, REQ-DATA-006)", () => {
  const authoritativeStream: AuthoritativeEventFrame[] = [
    {
      generation_id: "gen-fanout",
      source_id: "gauntlet",
      cursor: "c-001",
      event_type: "unit_updated",
      payload: {
        unit_id: "unit-A",
        title: "Alpha Unit",
        phase: "discovery",
        status: "in_progress",
      },
      timestamp_ms: 1000,
    },
    {
      generation_id: "gen-fanout",
      source_id: "gauntlet",
      cursor: "c-002",
      event_type: "evidence_added",
      payload: {
        evidence_id: "ev-001",
        unit_id: "unit-A",
        kind: "report",
        locator: ".agents/reports/ev1.json",
        verified: true,
      },
      timestamp_ms: 1050,
    },
    {
      generation_id: "gen-fanout",
      source_id: "prime",
      cursor: "p-001",
      event_type: "session_updated",
      payload: {
        session_id: "ses-1",
        model: "claude-3-5-sonnet",
        status: "active",
      },
      timestamp_ms: 1100,
    },
    {
      generation_id: "gen-fanout",
      source_id: "gauntlet",
      cursor: "c-003",
      event_type: "unit_updated",
      payload: {
        unit_id: "unit-A",
        title: "Alpha Unit",
        phase: "settle",
        status: "settled",
      },
      timestamp_ms: 1200,
    },
  ];

  it("Attack 1 (REQ-DATA-005): Chat is not the canonical source of Trace; disabling Chat leaves Trace identical", () => {
    // 1. Model with Chat enabled
    const modelWithChat = new WorkbenchModel({ enableChat: true });
    modelWithChat.startGeneration("gen-fanout");
    for (const ev of authoritativeStream) {
      modelWithChat.applyEvent(ev);
    }
    const traceWithChat = modelWithChat.getTrace();
    const chatWithChat = modelWithChat.getChat();

    expect(chatWithChat.length).toBeGreaterThan(0);
    expect(traceWithChat.length).toBe(authoritativeStream.length);

    // 2. ATTACK 1: Model with Chat completely disabled / removed
    const modelWithoutChat = new WorkbenchModel({ enableChat: false });
    modelWithoutChat.startGeneration("gen-fanout");
    for (const ev of authoritativeStream) {
      modelWithoutChat.applyEvent(ev);
    }
    const traceWithoutChat = modelWithoutChat.getTrace();
    const chatWithoutChat = modelWithoutChat.getChat();

    expect(chatWithoutChat.length).toBe(0);

    // 3. DEFENSE: Trace projection remains 100% identical in consequential content
    expect(traceWithoutChat).toEqual(traceWithChat);

    modelWithChat.close();
    modelWithoutChat.close();
  });

  it("Attack 2 (REQ-DATA-006): Work and Trace both depend on one canonical event without starvation", () => {
    const model = new WorkbenchModel();
    model.startGeneration("gen-fanout-starve");

    const singleEvent: AuthoritativeEventFrame = {
      generation_id: "gen-fanout-starve",
      source_id: "gauntlet",
      cursor: "c-starve-1",
      event_type: "unit_updated",
      payload: {
        unit_id: "unit-critical",
        title: "Critical Invariant Unit",
        phase: "settlement",
        status: "settled",
        authority: "orchestrator",
        blast_radius: "project",
      },
      timestamp_ms: 5000,
    };

    // Apply the single event
    const res = model.applyEvent(singleEvent);
    expect(res.applied).toBe(true);

    const snap = model.getSnapshot();

    // 1. Work projection updated
    expect(snap.units.length).toBe(1);
    expect(snap.units[0]?.unit_id).toBe("unit-critical");
    expect(snap.units[0]?.status).toBe("settled");

    // 2. Trace projection updated
    expect(snap.trace_records.length).toBe(1);
    expect(snap.trace_records[0]?.unit_id).toBe("unit-critical");
    expect(snap.trace_records[0]?.event_type).toBe("unit_updated");

    // 3. Source cursor updated for gauntlet
    expect(snap.source_cursors["gauntlet"]).toBe("c-starve-1");

    // Neither view starved the other
    model.close();
  });
});
