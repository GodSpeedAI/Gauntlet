import { afterAll, describe, expect, it } from "bun:test";
import { classifyTransport, resolveConfig, type WorkbenchConfig } from "./config";
import { FocusRef } from "./focus";
import { canonicalStreamOrder, InMemoryStreamPort, InMemoryStreamSource } from "./live-streams";
import {
  type GatewayOutcome,
  HttpOperatorActionRoute,
  OperationGateway,
  type OperationInvocationRequest,
} from "./operation-gateway";
import { OPERATION_IDS } from "./operation-registry";
import { CanonicalSurfaceStub } from "./testing/canonical-surface-stub";
import { WorkbenchModel } from "./workbench-model";

/**
 * T20 transport-equivalence suites (plan T20 Attack 1, REQ-CONFIG-005,
 * REQ-BUN-005; prereg P1/D1).
 *
 * THE CLAIM UNDER ATTACK: switch local transport to remote/SSH mode for the
 * same canonical operation. On this single host the SSH/tmux mode is the SAME
 * loopback HTTP surface (`gauntlet serve` per T02's config authority) operated
 * from an SSH-session terminal — so the transport difference is represented at
 * the config/client layer exactly as the deployment puts it:
 *
 *   - LOCAL mode: the in-process loopback route over config endpoint
 *     127.0.0.1 (transport classified "local").
 *   - REMOTE-REPRESENTATIVE mode: the REAL HTTP route over the loopback
 *     server — a distinct client process boundary, the same surface an SSH
 *     session drives — with transport declared/classified per REQ-CONFIG-005.
 *
 * Expected result: operation identity, admission decision, and result
 * semantics are IDENTICAL across modes; only transport/config/projection
 * behavior differs. Any semantic divergence is prereg disconfirmation P1.
 */

const RUN = "01J9T20TRANSPORTRUN";
const TOKEN = "t20-session-token";
const ACTOR = "operator-t20";
const TICKET = "ESC-420";
const FOCUS = FocusRef.for("gauntlet", "WorkItem", "AK-420");
const SESSION = { actor: ACTOR, token: TOKEN } as const;

/** One identical Case per mode: blocked unit with authority + an open ticket. */
function scenarioStub(): CanonicalSurfaceStub {
  return new CanonicalSurfaceStub({
    run_id: RUN,
    bound_actor: ACTOR,
    session_token: TOKEN,
  }).addUnit({
    unit_id: "AK-420",
    status: "blocked",
    tier: "P3",
    settled: false,
    authority: "granted",
    authority_visible_in_reads: true,
    governance_denied: false,
    governance_visible_in_reads: true,
    open_ticket: TICKET,
  });
}

function invocation(
  operation: (typeof OPERATION_IDS)[keyof typeof OPERATION_IDS],
  overrides: Partial<OperationInvocationRequest> = {},
): OperationInvocationRequest {
  return {
    operation,
    focus: FOCUS,
    session: SESSION,
    payload: { note: "T20 transport equivalence drive" },
    ...overrides,
  };
}

/** The comparable semantics of any outcome, stripped of transport placement. */
function outcomeSemantics(outcome: GatewayOutcome): Record<string, unknown> {
  switch (outcome.class) {
    case "accepted":
      return {
        class: outcome.class,
        operation: outcome.operation,
        run_id: outcome.run_id,
        unit_id: outcome.unit_id,
        tickets_resolved: [...outcome.tickets_resolved],
        committed_now: outcome.committed_now,
      };
    case "refused":
      return {
        class: outcome.class,
        operation: outcome.operation,
        code: outcome.code,
        reason_codes: [...outcome.reason_codes],
        stale: outcome.stale,
      };
    case "failed":
      return { class: outcome.class, operation: outcome.operation, code: outcome.code };
    case "outcome_unknown":
      return { class: outcome.class, operation: outcome.operation };
  }
}

describe("T20 config axis: transport is a configuration distinction, never a semantic one", () => {
  it("REQ-CONFIG-005 classification: loopback endpoints are local; the same config elsewhere is remote", () => {
    expect(classifyTransport("http://127.0.0.1:4000")).toBe("local");
    expect(classifyTransport("http://localhost:4000")).toBe("local");
    expect(classifyTransport("http://10.1.2.3:4000")).toBe("remote");
    expect(classifyTransport("gauntlet.example.org:4000")).toBe("remote");
  });

  it("the SAME loopback surface resolves to configs differing ONLY in declared transport", () => {
    const local: WorkbenchConfig = resolveConfig({
      cli: { gauntlet_endpoint: "http://127.0.0.1:4000" },
      env: {},
    });
    const remoteDeclared: WorkbenchConfig = resolveConfig({
      cli: { gauntlet_endpoint: "http://127.0.0.1:4000" },
      env: { GAUNTLET_WORKBENCH_TRANSPORT: "remote" },
    });
    // The SSH-session mode re-declares transport at the config layer only:
    // every field except `transport` is byte-identical.
    const { transport: _localTransport, ...localRest } = local;
    const { transport: _remoteTransport, ...remoteRest } = remoteDeclared;
    expect(_remoteTransport).toBe("remote");
    expect(JSON.stringify(remoteRest)).toBe(JSON.stringify(localRest));
    expect(_localTransport).toBe("local");

    // A genuinely different host changes classification WITHOUT touching
    // operation vocabulary: the operation registry has no transport concept.
    const otherHost: WorkbenchConfig = resolveConfig({
      cli: { gauntlet_endpoint: "http://10.1.2.3:4000" },
      env: {},
    });
    expect(otherHost.transport).toBe("remote");
  });

  it("transport never leaks into canonical operation identity or the projection", () => {
    // The registry's operation ids are transport-free by construction.
    for (const operation of Object.values(OPERATION_IDS)) {
      expect(operation).not.toContain("local");
      expect(operation).not.toContain("remote");
      expect(operation).not.toContain("ssh");
    }
    // The snapshot's JSON never carries a transport field.
    const model = new WorkbenchModel();
    model.startGeneration("gen-t20-modeless");
    model.markReady("gen-t20-modeless");
    expect(JSON.stringify(model.getSnapshot())).not.toContain("transport");
    model.close();
  });
});

describe("T20 operation axis (Attack 1): one canonical operation through both modes", () => {
  const servers: ReturnType<typeof Bun.serve>[] = [];

  afterAll(() => {
    for (const server of servers) {
      server.stop(true);
    }
  });

  /** LOCAL mode: the workbench host process on the same host, in-process route. */
  function localGateway(): OperationGateway {
    return new OperationGateway(scenarioStub().asInProcessRoute());
  }

  /**
   * REMOTE-REPRESENTATIVE mode: a REAL HTTP client against the loopback
   * server — the same `gauntlet serve` surface an SSH-session terminal on
   * this host drives, across a process/network boundary.
   */
  function remoteGateway(): OperationGateway {
    const server = scenarioStub().serveHttp();
    servers.push(server);
    return new OperationGateway(new HttpOperatorActionRoute(`127.0.0.1:${server.port}`));
  }

  it("an ALLOWED resolve converges on one admission decision and one result shape across modes", async () => {
    const local = await localGateway().invoke(invocation(OPERATION_IDS.UNIT_RESOLVE));
    const remote = await remoteGateway().invoke(invocation(OPERATION_IDS.UNIT_RESOLVE));

    expect(local.class).toBe("accepted");
    expect(remote.class).toBe("accepted");
    // Operation identity and result semantics are IDENTICAL...
    expect(outcomeSemantics(remote)).toEqual(outcomeSemantics(local));
    // ...including the deterministic idempotency key (same logical invocation).
    expect(remote.idempotency_key).toBe(local.idempotency_key);
    expect(remote.idempotency_key).toContain(OPERATION_IDS.UNIT_RESOLVE);
    expect(remote.idempotency_key).toContain("AK-420");
  });

  it("an ILLEGAL retry (no failed verification) is refused identically across modes", async () => {
    const local = await localGateway().invoke(invocation(OPERATION_IDS.UNIT_RETRY));
    const remote = await remoteGateway().invoke(invocation(OPERATION_IDS.UNIT_RETRY));

    expect(local.class).toBe("refused");
    expect(remote.class).toBe("refused");
    expect(outcomeSemantics(remote)).toEqual(outcomeSemantics(local));
    // The canonical refusal class is the T09 vocabulary, not a transport one.
    if (local.class !== "refused" || remote.class !== "refused") {
      throw new Error("expected refused outcomes on both routes");
    }
    expect(remote.code).toBe(local.code);
    expect(remote.code).not.toContain("transport");
  });

  it("a read-shaped refresh driven through either mode is refused not_action_admitted, zero side effects", async () => {
    const local = await localGateway().invoke(invocation(OPERATION_IDS.STATE_REFRESH));
    const remote = await remoteGateway().invoke(invocation(OPERATION_IDS.STATE_REFRESH));

    expect(local.class).toBe("refused");
    expect(remote.class).toBe("refused");
    expect(outcomeSemantics(remote)).toEqual(outcomeSemantics(local));
    if (local.class !== "refused" || remote.class !== "refused") {
      throw new Error("expected refused outcomes on both routes");
    }
    expect(local.code).toBe("not_action_admitted");
    expect(remote.code).toBe("not_action_admitted");
  });

  it("a REFUSED drive through one mode arms nothing in the other mode (same key, fresh gateway)", async () => {
    const refused = await localGateway().invoke(
      invocation(OPERATION_IDS.UNIT_RETRY, { session: { actor: ACTOR, token: "wrong-token" } }),
    );
    expect(refused.class).toBe("refused");
    const key = refused.idempotency_key;

    // The same key replayed through the REMOTE mode's gateway with the SAME
    // wrong-token payload is refused on its own merits — the local refusal
    // neither authorizes nor poisons the remote drive.
    const server = scenarioStub().serveHttp();
    servers.push(server);
    const remote = await new OperationGateway(
      new HttpOperatorActionRoute(`127.0.0.1:${server.port}`),
    ).invoke(
      invocation(OPERATION_IDS.UNIT_RETRY, {
        session: { actor: ACTOR, token: "wrong-token" },
        idempotency_key: key,
      }),
    );
    expect(remote.class).toBe("refused");
    expect(remote.idempotency_key).toBe(key);
  });
});

describe("T20 projection axis: one authoritative log folds identically in every mode", () => {
  it("events replayed directly (local) and through canonical stream order (remote feed) yield one snapshot", () => {
    const source = new InMemoryStreamSource("gauntlet");
    const frames: Parameters<WorkbenchModel["applyEvent"]>[0][] = [
      {
        generation_id: "gen-t20-fold",
        source_id: "gauntlet",
        cursor: "c-1",
        event_type: "unit_updated",
        payload: { unit_id: "u-1", title: "Unit 1", status: "active", authority: "builder" },
        timestamp_ms: 1000,
      },
      {
        generation_id: "gen-t20-fold",
        source_id: "gauntlet",
        cursor: "c-2",
        event_type: "evidence_added",
        payload: { evidence_id: "ev-1", unit_id: "u-1", kind: "report", verified: true },
        timestamp_ms: 1100,
      },
      {
        generation_id: "gen-t20-fold",
        source_id: "gauntlet",
        cursor: "c-3",
        event_type: "session_updated",
        payload: { session_id: "s-1", parent_id: null, model: "m", status: "running" },
        timestamp_ms: 1200,
      },
    ];
    for (const frame of frames) {
      source.append({
        event_type: frame.event_type,
        payload: frame.payload,
        timestamp_ms: frame.timestamp_ms,
      });
    }

    // LOCAL: events applied in arrival order by the host process.
    const local = new WorkbenchModel();
    local.startGeneration("gen-t20-fold");
    const port = new InMemoryStreamPort([source]);
    const window = port
      .describeSources()
      .flatMap((descriptor) => port.baselineWindow(descriptor.source_id).events);
    // LOCAL feed: the log in its arrival order, cursors as the source owns them.
    for (const event of window) {
      local.applyEvent({
        generation_id: "gen-t20-fold",
        source_id: event.source_id,
        cursor: event.cursor,
        event_type: event.event_type,
        payload: event.payload,
        timestamp_ms: event.timestamp_ms,
      });
    }
    local.markReady("gen-t20-fold");

    // REMOTE FEED: the same log consumed through the canonical orderer (the
    // reconnect/baseline reconstruction path a fresh SSH client runs).
    const remote = new WorkbenchModel();
    remote.startGeneration("gen-t20-fold");
    for (const event of canonicalStreamOrder(window)) {
      remote.applyEvent({
        generation_id: "gen-t20-fold",
        source_id: event.source_id,
        cursor: event.cursor,
        event_type: event.event_type,
        payload: event.payload,
        timestamp_ms: event.timestamp_ms,
      });
    }
    remote.markReady("gen-t20-fold");

    expect(JSON.stringify(remote.getSnapshot())).toBe(JSON.stringify(local.getSnapshot()));
    local.close();
    remote.close();
  });
});
