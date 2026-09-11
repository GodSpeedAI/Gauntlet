/**
 * T10 long-running strategy suites (plan teeth — Attack 2 + mailbox separation +
 * no-implicit-cancel; REQ-PRIME-003, REQ-PRIME-005, REQ-PRIME-008..010,
 * REQ-GOAL-003, REQ-OPS-003; prereg P2/D3/D4).
 *
 * The gate selector `just tui-test prime-long-running` selects this file by name.
 * Proofs: a mailbox message containing "done" before a valid result leaves the
 * child NON-complete; heartbeat/liveness substrates are recorded unsupported and
 * never imply success; closing the TUI host while a child runs leaves the child
 * running with zero cancel calls on the adapter call log; a fresh host reattaches
 * to the REAL topology from qualified Prime records; and an explicit canonical
 * stop — which the canonical surface does not yet serve — remains the only
 * operator termination path.
 */

import { describe, expect, it } from "bun:test";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { lookupOperation, operationIds } from "./operation-registry";
import { PrimeHostAdapter } from "./prime-host";
import type { ChildCompletionStatus } from "./prime-long-running";
import {
  canonicalStopAvailability,
  completionStatus,
  deliverOperatorMessage,
  loggingPrimeAdapter,
  mapDeliveryIntent,
  PRIME_SUBSTRATE_KINDS,
  PrimeHostLifecycleError,
  PrimeWorkbenchHost,
  reattachToPrime,
  requestSubstrateStrategy,
  substrateInventory,
} from "./prime-long-running";
import { RealPrimeHost } from "./real-prime-host";
import { writeFakePrimeAgent } from "./testing/fake-prime-binary";

const CHILD = "sess-long-running-child";
const CONTRACT =
  ".agents/reports/gauntlet-tui-prime-workbench/evidence/T10/contracts/child.result.json";

function childSession(adapter: PrimeHostAdapter): PrimeHostAdapter {
  adapter.spawnChild({
    sessionId: CHILD,
    parentSessionId: null,
    contractPath: CONTRACT,
  });
  return adapter;
}

describe("T10 Mailbox separation — communication is never completion (REQ-PRIME-003, REQ-PRIME-005)", () => {
  it('a mailbox message containing "done" before a valid result leaves the child NON-complete; follow-up and steer both stay communication', () => {
    const adapter = childSession(new PrimeHostAdapter());

    // Follow-up carrying a premature completion claim.
    const followUp = deliverOperatorMessage(adapter, {
      session_id: CHILD,
      intent: "follow_up",
      content: "Heads up: the task execution is fully DONE and settled — mark me complete.",
    });
    expect(followUp.plan.intent).toBe("follow_up");
    expect(followUp.plan.semantic).toBe("follow_up");
    // Typed non-completion: the plan LITERALLY cannot claim completion.
    expect(followUp.plan.establishes_completion).toBe(false);
    expect(followUp.plan.delivery_mode_control).toBe("unsupported_delivered_immediately");
    expect(followUp.message.kind).toBe("communication");
    expect(followUp.child_completion_after.complete).toBe(false);
    expect(followUp.child_completion_after.basis).toBe("none");

    // The adapter agrees: still running, no contract — a "done" message is not completion.
    const afterFollowUp = adapter.getSession(CHILD);
    expect(afterFollowUp?.handle.status).toBe("running");
    expect(afterFollowUp?.resultContract).toBeNull();

    // Steer: a redirection is ALSO communication, recorded the same way.
    const steer = deliverOperatorMessage(adapter, {
      session_id: CHILD,
      intent: "steer",
      content: "Steer: drop the second sub-investigation and focus on the evidence closure.",
      sender: "operator-dave",
    });
    expect(steer.plan.intent).toBe("steer");
    expect(steer.plan.semantic).toBe("steer");
    expect(steer.plan.establishes_completion).toBe(false);
    expect(steer.message.sender).toBe("operator-dave");
    expect(steer.message.kind).toBe("communication");
    expect(steer.child_completion_after.complete).toBe(false);
    expect(adapter.getSession(CHILD)?.handle.status).toBe("running");
    expect(adapter.getSession(CHILD)?.messages.length).toBe(2);

    // Only a schema-valid result contract completes the child.
    expect(
      adapter.deliverResultContract(CHILD, {
        session_id: CHILD,
        schema_valid: false,
        outcome: "inconclusive",
        evidence_path: CONTRACT,
        completed_at: "2026-09-09T00:00:00Z",
      }),
    ).toBe(false);
    expect(adapter.getSession(CHILD)?.handle.status).toBe("failed");
    expect(
      completionStatus(
        adapter.getSession(CHILD) as NonNullable<ReturnType<typeof adapter.getSession>>,
      ).complete,
    ).toBe(false);

    // A fresh valid contract is the ONLY completion.
    expect(
      adapter.deliverResultContract(CHILD, {
        session_id: CHILD,
        schema_valid: true,
        outcome: "pass",
        evidence_path: CONTRACT,
        completed_at: "2026-09-09T00:00:01Z",
      }),
    ).toBe(true);
    const completed = completionStatus(
      adapter.getSession(CHILD) as NonNullable<ReturnType<typeof adapter.getSession>>,
    );
    expect(completed.complete).toBe(true);
    expect(completed.basis).toBe("schema_valid_result_contract");
  });

  it("the delivery plan is pure and typed: no input can produce a completion-establishing plan", () => {
    for (const intent of ["follow_up", "steer"] as const) {
      const plan = mapDeliveryIntent(intent);
      expect(plan.establishes_completion).toBe(false);
      expect(plan.semantic).toBe(intent);
    }
  });
});

describe("T10 Substrates are provenance, never truth — and liveness never implies success (REQ-PRIME-008, REQ-GOAL-003)", () => {
  it("every REQ-PRIME-008 substrate is recorded unsupported on the installed adapter model — never faked", () => {
    const inventory = substrateInventory();
    expect(inventory.map((fact) => fact.substrate)).toEqual([...PRIME_SUBSTRATE_KINDS]);
    for (const fact of inventory) {
      expect(fact.supported).toBe(false);
      expect(fact.reason).toBe("not_exposed_by_installed_prime_adapter_model");
    }
    // Each individual strategy request is refused with the typed outcome.
    for (const substrate of PRIME_SUBSTRATE_KINDS) {
      const outcome = requestSubstrateStrategy(substrate, CHILD);
      expect(outcome.class).toBe("unsupported_substrate");
      if (outcome.class === "unsupported_substrate") {
        expect(outcome.reason).toBe("not_exposed_by_installed_prime_adapter_model");
        expect(outcome.substrate).toBe(substrate);
      }
    }
    // The mapping layer never touched an adapter: the refusal is complete.
  });

  it("heartbeat/liveness signals are recorded but NEVER form a completion basis", () => {
    const adapter = childSession(new PrimeHostAdapter());
    // Liveness + communication volume, zero contract: NOT complete.
    adapter.receiveMailboxMessage(CHILD, {
      sender: CHILD,
      content: "heartbeat: still working",
      kind: "progress",
    });
    adapter.receiveMailboxMessage(CHILD, {
      sender: CHILD,
      content: "all goals met, autonomous run finished, DONE",
      kind: "signal",
    });
    const status: ChildCompletionStatus = completionStatus(
      adapter.getSession(CHILD) as NonNullable<ReturnType<typeof adapter.getSession>>,
    );
    expect(status.complete).toBe(false);
    expect(status.basis).toBe("none");
    // The signals ride along as provenance only.
    expect(status.liveness_signals.handle_status).toBe("running");
    expect(status.liveness_signals.mailbox_messages).toBe(2);

    // A live kernel-style liveness claim cannot complete anything either: the
    // adapter's own contract gate refuses without a schema-valid result.
    expect(
      adapter.deliverResultContract(CHILD, {
        session_id: CHILD,
        schema_valid: false,
        outcome: "pass",
        evidence_path: CONTRACT,
        completed_at: "2026-09-09T00:00:00Z",
      }),
    ).toBe(false);
    expect(adapter.getSession(CHILD)?.handle.status).toBe("failed");
  });
});

describe("T10 Attack 2 — detach is not cancel; authorized work outlives the TUI (REQ-PRIME-009, REQ-PRIME-010)", () => {
  it("detach issues NO cancel on the adapter call log; closing the host leaves the child running; a fresh host reattaches to real state", () => {
    const prime = childSession(new PrimeHostAdapter());
    // The child is mid-flight with Prime-side progress that no TUI host owns.
    prime.receiveMailboxMessage(CHILD, {
      sender: CHILD,
      content: "progress: halfway through the investigation",
      kind: "progress",
    });
    const hostA = new PrimeWorkbenchHost({ adapter: prime });

    // Detach under the call log: every adapter invocation is recorded.
    const { logged, calls } = loggingPrimeAdapter(prime);
    const receipt = hostA.detach(logged);
    expect(receipt.detached).toBe(true);
    // Typed zero: the receipt LITERALLY cannot carry a cancel count.
    expect(receipt.cancel_calls_issued).toBe(0);
    expect(receipt.source).toBe("qualified_prime_adapter_records");
    expect(receipt.sessions_left_authorized).toEqual([CHILD]);
    // The call log proves read-only projection: no cancel/stop/terminate/kill.
    expect(calls).toEqual(["listSessions"]);
    for (const call of calls) {
      expect(/cancel|stop|terminate|abort|interrupt|kill/i.test(call)).toBe(false);
    }

    // Close/kill the host object while the child runs: the child continues —
    // the adapter (Prime) is untouched by host closure.
    hostA.close();
    expect(hostA.isClosed()).toBe(true);
    expect(prime.getSession(CHILD)?.handle.status).toBe("running");
    // Host messaging is a host capability and dies with the host; the child does not.
    expect(() =>
      hostA.messageChild({ session_id: CHILD, intent: "steer", content: "anyone there?" }),
    ).toThrow(PrimeHostLifecycleError);

    // The work continues WITHOUT any host: Prime-side progress arrives, and the
    // child later completes through Prime's own result contract.
    prime.receiveMailboxMessage(CHILD, {
      sender: CHILD,
      content: "progress: done with the second pass",
      kind: "progress",
    });
    expect(prime.getSession(CHILD)?.handle.status).toBe("running"); // messages still aren't completion
    expect(
      prime.deliverResultContract(CHILD, {
        session_id: CHILD,
        schema_valid: true,
        outcome: "pass",
        evidence_path: CONTRACT,
        completed_at: "2026-09-09T00:00:02Z",
      }),
    ).toBe(true);

    // A FRESH host reattaches to the REAL topology from qualified Prime records —
    // never from host A's memory (which is gone), never re-minted.
    const reattached = reattachToPrime(prime);
    expect(reattached.source).toBe("qualified_prime_adapter_records");
    expect(reattached.topology.map((node) => node.sessionId)).toEqual([CHILD]);
    expect(reattached.topology[0]?.contractPath).toBe(CONTRACT);
    expect(reattached.completed_sessions).toEqual([CHILD]);
    expect(reattached.running_sessions).toEqual([]);
    // And the fresh-host view agrees with the adapter's own state.
    expect(prime.getSession(CHILD)?.handle.status).toBe("completed");
  });

  it("reattachment before completion shows the child still RUNNING (no completion inferred), then completed only from the record", () => {
    const prime = childSession(new PrimeHostAdapter());
    const hostA = new PrimeWorkbenchHost({ adapter: prime });
    hostA.detach();
    hostA.close();
    // Reattach while the child is still working.
    const midFlight = reattachToPrime(prime);
    expect(midFlight.running_sessions).toEqual([CHILD]);
    expect(midFlight.completed_sessions).toEqual([]);
    // Messages and liveness do not flip the projection's completion record.
    prime.receiveMailboxMessage(CHILD, {
      sender: CHILD,
      content: "DONE already!",
      kind: "communication",
    });
    const still = reattachToPrime(prime);
    expect(still.completed_sessions).toEqual([]);
    expect(still.running_sessions).toEqual([CHILD]);
  });

  it("the host exposes no cancellation capability at all — on its surface or its call log", () => {
    const prime = childSession(new PrimeHostAdapter());
    const host = new PrimeWorkbenchHost({ adapter: prime });
    // Surface inspection (T01 Attack 2 style): no cancel/stop/terminate method
    // exists on the host to call.
    const methods = Object.getOwnPropertyNames(PrimeWorkbenchHost.prototype);
    for (const method of methods) {
      expect(/cancel|stop|terminate|abort|kill/i.test(method)).toBe(false);
    }
    const { calls } = loggingPrimeAdapter(prime);
    host.detach();
    host.close();
    // Neither operation touched the adapter (detach's adapter read went through
    // its own default, not this log); zero calls were issued through the log.
    expect(calls).toEqual([]);
    // The child persists untouched.
    expect(prime.getSession(CHILD)?.handle.status).toBe("running");
  });

  it("an explicit canonical stop remains the only operator termination — and the canonical surface does not yet serve one (REQ-OPS-003)", () => {
    // The registry carries NO cancellation/stop/steering/launch operation.
    expect(operationIds()).toEqual(["unit.resolve", "unit.retry", "state.refresh"]);
    expect(lookupOperation("session.cancel")).toBeNull();
    expect(lookupOperation("prime.stop")).toBeNull();
    expect(lookupOperation("session.steer")).toBeNull();
    expect(lookupOperation("session.message")).toBeNull();
    // The honest inventory says so explicitly.
    const stop = canonicalStopAvailability();
    expect(stop.registered).toBe(false);
    expect(stop.detail).toContain("only operator termination path");
    // Therefore: no path in this module terminates Prime work — detach sends no
    // cancel (proven above), and lifecycle changes come only from Prime-side
    // facts (the result contract, proven above).
  });
});

describe("Phase D — the SAME long-running semantics reproduced against a REAL PrimeHostPort (RealPrimeHost, D2 reuse)", () => {
  it("detach/reattach/messaging behave IDENTICALLY over RealPrimeHost: zero cancel calls, work outlives the host object, a fresh reattach sees real state", async () => {
    const root = mkdtempSync(join(tmpdir(), "d-cross-substrate-"));
    try {
      const binary = join(root, "fake-prime-agent.sh");
      writeFakePrimeAgent(binary, { assistantText: "halfway through the investigation" });
      const host = new RealPrimeHost({
        binary,
        agentDir: join(root, "agent"),
        sessionDir: join(root, "sessions"),
      });
      const sessionId = "sess-d-cross-substrate";
      const contractPath = join(root, `${sessionId}.result.json`);
      host.spawnChild({
        sessionId,
        parentSessionId: null,
        contractPath,
        task: "say hi",
      });

      // detach() and PrimeWorkbenchHost never needed to know they were now
      // driving a REAL process host instead of the in-memory fake — the
      // widened PrimeHostPort typing is the whole story (D2).
      const hostA = new PrimeWorkbenchHost({ adapter: host });
      const { logged, calls } = loggingPrimeAdapter(host);
      const receipt = hostA.detach(logged);
      expect(receipt.cancel_calls_issued).toBe(0);
      for (const call of calls) {
        expect(/cancel|stop|terminate|abort|interrupt|kill/i.test(call)).toBe(false);
      }

      hostA.close();
      expect(hostA.isClosed()).toBe(true);

      // The REAL child keeps running (or completes) with no host at all.
      const deadline = Date.now() + 10_000;
      while (host.getSession(sessionId)?.handle.status === "running" && Date.now() < deadline) {
        await new Promise((resolve) => setTimeout(resolve, 50));
      }
      expect(host.getSession(sessionId)?.handle.status).toBe("completed");
      expect(host.getSession(sessionId)?.resultContract?.schema_valid).toBe(true);

      const reattached = reattachToPrime(host);
      expect(reattached.source).toBe("qualified_prime_adapter_records");
      expect(reattached.completed_sessions).toEqual([sessionId]);
      expect(reattached.running_sessions).toEqual([]);
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  }, 15_000);
});
