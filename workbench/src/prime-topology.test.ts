import { describe, expect, it } from "bun:test";
import type { PrimeSessionRecord } from "./prime-topology";
import { findSessionInTopology, reconstructTopology } from "./prime-topology";

describe("T04 Prime Topology Reconstruction & Durable Identity (REQ-PRIME-002, REQ-PRIME-005)", () => {
  it("Attack 2: Spawn nested Prime children and restart adapter process -> re-reads actual topology without synthetic replacements", () => {
    // 1. Durable session records representing multi-tier nested Prime children
    const durableRecords: PrimeSessionRecord[] = [
      {
        sessionId: "session-root-orchestrator",
        parentId: null,
        model: "claude-3-5-sonnet",
        status: "running",
        contractPath: ".agents/reports/root.result.json",
        messages: [],
        createdAt: "2026-09-08T12:00:00Z",
      },
      {
        sessionId: "session-child-builder-1",
        parentId: "session-root-orchestrator",
        model: "claude-3-5-haiku",
        status: "running",
        contractPath: ".agents/reports/builder-1.result.json",
        messages: [],
        createdAt: "2026-09-08T12:01:00Z",
      },
      {
        sessionId: "session-child-builder-2",
        parentId: "session-root-orchestrator",
        model: "claude-3-5-haiku",
        status: "completed",
        contractPath: ".agents/reports/builder-2.result.json",
        messages: [],
        createdAt: "2026-09-08T12:02:00Z",
      },
      {
        sessionId: "session-grandchild-falsifier-1a",
        parentId: "session-child-builder-1",
        model: "claude-3-5-sonnet",
        status: "running",
        contractPath: ".agents/reports/falsifier-1a.result.json",
        messages: [],
        createdAt: "2026-09-08T12:03:00Z",
      },
    ];

    // 2. Simulate workbench adapter restart by reconstructing topology afresh from durable state
    const topology = reconstructTopology(durableRecords);

    // Root level check
    expect(topology.length).toBe(1);
    const root = topology[0];
    expect(root?.sessionId).toBe("session-root-orchestrator");
    expect(root?.parentId).toBeNull();
    expect(root?.children.length).toBe(2);

    // First child level
    const builder1 = root?.children.find((c) => c.sessionId === "session-child-builder-1");
    const builder2 = root?.children.find((c) => c.sessionId === "session-child-builder-2");
    expect(builder1).toBeDefined();
    expect(builder2).toBeDefined();
    expect(builder1?.parentId).toBe("session-root-orchestrator");
    expect(builder2?.status).toBe("completed");

    // Grandchild level (nested under builder-1)
    expect(builder1?.children.length).toBe(1);
    const grandchild = builder1?.children[0];
    expect(grandchild?.sessionId).toBe("session-grandchild-falsifier-1a");
    expect(grandchild?.parentId).toBe("session-child-builder-1");
    expect(grandchild?.status).toBe("running");

    // Attack 2 invariant: Every node has its exact original ID, zero synthetic IDs minted
    expect(findSessionInTopology(topology, "session-root-orchestrator")).not.toBeNull();
    expect(findSessionInTopology(topology, "session-child-builder-1")).not.toBeNull();
    expect(findSessionInTopology(topology, "session-grandchild-falsifier-1a")).not.toBeNull();
    expect(findSessionInTopology(topology, "synthetic-session-id-1")).toBeNull();
  });
});
