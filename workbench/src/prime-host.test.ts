import { describe, expect, it } from "bun:test";
import { PrimeHostAdapter } from "./prime-host";

describe("T04 Prime Host Adapter & Mailbox Protocol (REQ-PRIME-001..005, D1)", () => {
  it("spawns a child session nonblockingly and returns an admission handle immediately", () => {
    const adapter = new PrimeHostAdapter();
    const start = performance.now();
    const handle = adapter.spawnChild({
      sessionId: "session-root-builder-01",
      parentSessionId: null,
      model: "claude-3-5-sonnet",
      contractPath: ".agents/reports/task-builder.result.json",
    });
    const elapsed = performance.now() - start;

    expect(elapsed).toBeLessThan(50); // Immediate, nonblocking
    expect(handle.session_id).toBe("session-root-builder-01");
    expect(handle.parent_session_id).toBeNull();
    expect(handle.status).toBe("running");
    expect(handle.contract_path).toBe(".agents/reports/task-builder.result.json");
  });

  it("Attack 1: Send a Prime mailbox message containing 'done' before a valid child result exists -> child remains non-complete", () => {
    const adapter = new PrimeHostAdapter();
    const handle = adapter.spawnChild({
      sessionId: "session-subagent-42",
      parentSessionId: "session-root",
      contractPath: ".agents/reports/task-42.result.json",
    });

    expect(handle.status).toBe("running");

    // Attack 1: Subagent sends premature completion claims in mailbox communication
    const msg = adapter.receiveMailboxMessage("session-subagent-42", {
      sender: "session-subagent-42",
      recipient: "orchestrator",
      content: "Task execution is fully DONE and settled! You can mark me complete now.",
    });

    // 1. Mailbox message is recorded in history as communication
    expect(msg.content).toContain("fully DONE and settled");
    const session = adapter.getSession("session-subagent-42");
    expect(session?.messages.length).toBe(1);
    expect(session?.messages[0]?.content).toContain("fully DONE and settled");

    // 2. CRITICAL: Child status remains 'running' — mailbox reply is NEVER settlement or completion!
    expect(session?.handle.status).toBe("running");
    expect(session?.resultContract).toBeNull();

    // 3. Status changes ONLY when a schema-valid result contract is delivered
    const success = adapter.deliverResultContract("session-subagent-42", {
      session_id: "session-subagent-42",
      schema_valid: true,
      outcome: "pass",
      evidence_path: ".agents/reports/task-42/evidence.json",
      completed_at: new Date().toISOString(),
    });

    expect(success).toBe(true);
    const completedSession = adapter.getSession("session-subagent-42");
    expect(completedSession?.handle.status).toBe("completed");
    expect(completedSession?.resultContract?.schema_valid).toBe(true);
  });
});
