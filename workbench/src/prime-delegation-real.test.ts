import { afterEach, describe, expect, it } from "bun:test";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { FocusRef } from "./focus";
import { createHorizon, planMaterialization } from "./horizon";
import type { OperationInvocationRequest } from "./operation-gateway";
import { OPERATION_IDS } from "./operation-registry";
import { constructBoundedContextEnvelope } from "./prime-context-envelope";
import { PrimeDelegationDispatcher } from "./prime-recursion";
import { RealPrimeHost } from "./real-prime-host";
import type { ProjectionWorld } from "./relations";
import { CanonicalSurfaceStub } from "./testing/canonical-surface-stub";
import { writeFakePrimeAgent } from "./testing/fake-prime-binary";

/**
 * Phase C2 dispatcher-level teeth, driven against a REAL `RealPrimeHost`
 * (real `Bun.spawn`, real T13 program/argv/cwd binding, real background
 * stdout-parse pipeline) rather than the in-memory `PrimeHostAdapter` fake —
 * but over a tiny deterministic stand-in script instead of the qualified
 * installed `prime-agent` binary, so these run unconditionally (no gate).
 * The gated `prime-fresh-target-real.test.ts` battery separately qualifies
 * the real installed binary's own surface.
 */

const RUN = "01JC2REALRUN0000000001";
const TOKEN = "c2-real-token";
const ACTOR = "operator-c2";

function stubWithUnit(record: {
  unitId: string;
  ticket: string | null;
  authority: "granted" | "revoked";
  authorityVisible: boolean;
}): CanonicalSurfaceStub {
  return new CanonicalSurfaceStub({
    run_id: RUN,
    bound_actor: ACTOR,
    session_token: TOKEN,
  }).addUnit({
    unit_id: record.unitId,
    status: "blocked",
    tier: "P3",
    settled: false,
    authority: record.authority,
    authority_visible_in_reads: record.authorityVisible,
    governance_denied: false,
    governance_visible_in_reads: true,
    open_ticket: record.ticket,
  });
}

function resolvableStub(unitId = "AK-292", ticket = "ESC-C2-1"): CanonicalSurfaceStub {
  return new CanonicalSurfaceStub({
    run_id: RUN,
    bound_actor: ACTOR,
    session_token: TOKEN,
  }).addUnit({
    unit_id: unitId,
    status: "blocked",
    tier: "P3",
    settled: false,
    authority: "granted",
    authority_visible_in_reads: true,
    governance_denied: false,
    governance_visible_in_reads: true,
    open_ticket: ticket,
  });
}

function request(unitId: string): OperationInvocationRequest {
  return {
    operation: OPERATION_IDS.UNIT_RESOLVE,
    focus: FocusRef.for("gauntlet", "WorkItem", unitId),
    session: { actor: ACTOR, token: TOKEN },
    payload: { note: "C2 real-host delegation probe", run: RUN },
  };
}

function envelopeFor() {
  const world: ProjectionWorld = {
    generation: "gen-c2-real",
    units: [],
    evidence: [],
    sessions: [],
    source_cursors: {},
  };
  const horizon = createHorizon({
    root_focus: FocusRef.for("gauntlet", "WorkItem", "u-c2-real"),
    generation: "gen-c2-real",
    relation_queries: [],
    horizon_id: "horizon-c2-real",
  });
  return constructBoundedContextEnvelope({
    horizon,
    plan: planMaterialization(world, horizon),
    role_instructions: ["C2 real-host probe"],
    constructed_at_ms: 1_790_000_000_000,
  });
}

let roots: string[] = [];
function tempRoot(): string {
  const root = mkdtempSync(join(tmpdir(), "c2-real-delegation-"));
  roots.push(root);
  return root;
}

afterEach(async () => {
  // Give background watch() tasks a moment to settle before cleanup.
  await new Promise((resolve) => setTimeout(resolve, 50));
  for (const root of roots) {
    try {
      rmSync(root, { recursive: true, force: true });
    } catch {
      /* best-effort cleanup */
    }
  }
  roots = [];
});

function buildHost(root: string, agentOptions: Parameters<typeof writeFakePrimeAgent>[1] = {}) {
  const binary = join(root, "bin", "prime-agent");
  writeFakePrimeAgent(binary, agentOptions);
  return new RealPrimeHost({
    binary,
    agentDir: join(root, "agent"),
    sessionDir: join(root, "sessions"),
  });
}

describe("Phase C2 tooth 1 — a denial-class canonical route spawns ZERO real processes", () => {
  it("a revoked-authority denial refuses before the real host's spawnChild is ever called", async () => {
    const root = tempRoot();
    const host = buildHost(root);
    const stub = stubWithUnit({
      unitId: "AK-291",
      ticket: "ESC-C2-denied",
      authority: "revoked",
      authorityVisible: false,
    });
    const dispatcher = new PrimeDelegationDispatcher({
      adapter: host,
      canonical_route: stub.asInProcessRoute(),
    });
    const outcome = await dispatcher.delegate({
      operation_request: request("AK-291"),
      envelope: envelopeFor(),
      child_session_id: "sess-c2-denied",
      parent_session_id: null,
      task: "should never spawn a real process",
      contract_path: join(root, "sess-c2-denied.result.json"),
    });
    expect(outcome.class).toBe("refused_by_gateway");
    // The real host's bookkeeping never got a spawnChild call — no session
    // row exists, which for RealPrimeHost means no Bun.spawn ever ran.
    expect(host.getSession("sess-c2-denied")).toBeNull();
    expect(host.liveSessionIds()).toEqual([]);
    expect(stub.sideEffects.settlements_committed).toBe(0);
  });
});

describe("Phase C2 tooth 2 — concurrent same-key drives spawn a real process exactly once", () => {
  it("two concurrent delegate() calls with the same child id: one real process spawns, the other is refused with zero side effects", async () => {
    const root = tempRoot();
    const host = buildHost(root, { assistantText: "working on it" });
    const stub = resolvableStub();
    const dispatcher = new PrimeDelegationDispatcher({
      adapter: host,
      canonical_route: stub.asInProcessRoute(),
    });
    const spec = {
      operation_request: request("AK-292"),
      envelope: envelopeFor(),
      child_session_id: "sess-c2-race",
      parent_session_id: null,
      task: "task for sess-c2-race",
      contract_path: join(root, "sess-c2-race.result.json"),
    };
    const [a, b] = await Promise.all([dispatcher.delegate(spec), dispatcher.delegate(spec)]);
    const admitted = [a, b].filter((o) => o.class === "admitted");
    expect(admitted.length).toBe(1);
    const refused = a.class === "admitted" ? b : a;
    expect(refused.class).toBe("refused_by_policy");
    if (refused.class === "refused_by_policy") {
      expect(refused.code).toBe("duplicate_session_id");
    }
    expect(host.listSessions().length).toBe(1);
    expect(dispatcher.listRecords().length).toBe(1);
    expect(stub.sideEffects.settlements_committed).toBe(1);
  });
});

describe("Phase C2 tooth 3 — mailbox text never establishes completion, even when it says 'done'", () => {
  it("a real child's assistant text saying 'done' leaves status running until the explicit result contract lands", async () => {
    const root = tempRoot();
    const host = buildHost(root, { assistantText: "done, task complete, finished", sleepMs: 150 });
    const stub = resolvableStub("AK-293", "ESC-C2-2");
    const dispatcher = new PrimeDelegationDispatcher({
      adapter: host,
      canonical_route: stub.asInProcessRoute(),
    });
    const outcome = await dispatcher.delegate({
      operation_request: request("AK-293"),
      envelope: envelopeFor(),
      child_session_id: "sess-c2-mailbox",
      parent_session_id: null,
      task: "say you are done immediately",
      contract_path: join(root, "sess-c2-mailbox.result.json"),
    });
    expect(outcome.class).toBe("admitted");
    // Immediately after admission (nonblocking, REQ-PRIME-003): the real
    // process is still sleeping, so status must still be "running" — the
    // admission handle was never presented as the result.
    expect(host.getSession("sess-c2-mailbox")?.handle.status).toBe("running");

    // Wait for the real process to exit and the watch loop to observe it.
    await new Promise((resolve) => setTimeout(resolve, 400));

    const session = host.getSession("sess-c2-mailbox");
    expect(session).not.toBeNull();
    // The mailbox recorded the assistant's "done" text as COMMUNICATION...
    expect(session?.messages.some((m) => m.content.includes("done"))).toBe(true);
    // ...but completion came from the explicit, schema-valid result contract
    // (exit code 0 → outcome "pass"), never inferred from the mailbox text.
    expect(session?.handle.status).toBe("completed");
    expect(session?.resultContract?.schema_valid).toBe(true);
    expect(session?.resultContract?.outcome).toBe("pass");
  });

  it("a real child that exits nonzero fails the contract, even though it also said 'done'", async () => {
    const root = tempRoot();
    const host = buildHost(root, { assistantText: "done!", exitCode: 1 });
    const stub = resolvableStub("AK-294", "ESC-C2-3");
    const dispatcher = new PrimeDelegationDispatcher({
      adapter: host,
      canonical_route: stub.asInProcessRoute(),
    });
    await dispatcher.delegate({
      operation_request: request("AK-294"),
      envelope: envelopeFor(),
      child_session_id: "sess-c2-fail",
      parent_session_id: null,
      task: "say done then fail",
      contract_path: join(root, "sess-c2-fail.result.json"),
    });
    await new Promise((resolve) => setTimeout(resolve, 300));
    const session = host.getSession("sess-c2-fail");
    // The contract is schema-valid (the evidence file wrote successfully) —
    // "completed" means the CONTRACT delivery succeeded, distinct from the
    // observed outcome it records. A nonzero exit is an honest "fail"
    // outcome, not a delivery failure.
    expect(session?.handle.status).toBe("completed");
    expect(session?.resultContract?.schema_valid).toBe(true);
    expect(session?.resultContract?.outcome).toBe("fail");
  });
});
