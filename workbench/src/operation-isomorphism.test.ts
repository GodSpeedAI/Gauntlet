import { afterAll, describe, expect, it } from "bun:test";
import { type AffordanceDescriptor, resolveAffordance } from "./affordance";
import { FocusRef } from "./focus";
import {
  type CanonicalOutcome,
  type GatewayOutcome,
  HttpOperatorActionRoute,
  OperationGateway,
  type OperationInvocationRequest,
  type OperatorActionRoute,
} from "./operation-gateway";
import { OPERATION_IDS, OPERATION_REGISTRY, operationIds } from "./operation-registry";
import { CanonicalSurfaceStub } from "./testing/canonical-surface-stub";

const RUN = "01J9T09ISOMORPHICRUN";
const TOKEN = "t09-session-token";
const ACTOR = "operator-alice";
const TICKET = "ESC-901";
const FOCUS = FocusRef.for("gauntlet", "WorkItem", "AK-901");
const SESSION = { actor: ACTOR, token: TOKEN } as const;

function invocation(
  overrides: Partial<OperationInvocationRequest> = {},
): OperationInvocationRequest {
  return {
    operation: OPERATION_IDS.UNIT_RESOLVE,
    focus: FOCUS,
    session: SESSION,
    payload: { note: "operator resolves the escalation" },
    ...overrides,
  };
}

function scenarioStub(): CanonicalSurfaceStub {
  return new CanonicalSurfaceStub({ run_id: RUN, bound_actor: ACTOR, session_token: TOKEN })
    .addUnit({
      unit_id: "AK-901",
      status: "blocked",
      tier: "P3",
      settled: false,
      authority: "granted",
      authority_visible_in_reads: true,
      governance_denied: false,
      governance_visible_in_reads: true,
      open_ticket: TICKET,
    })
    .addUnit({
      unit_id: "AK-902",
      status: "fail",
      tier: "P1",
      settled: false,
      authority: "granted",
      authority_visible_in_reads: true,
      governance_denied: false,
      governance_visible_in_reads: true,
      open_ticket: null,
    });
}

/** The canonical result facts every route must converge on (ActionAccepted vocabulary). */
function acceptedFacts(
  outcome:
    | Extract<GatewayOutcome, { class: "accepted" }>
    | Extract<CanonicalOutcome, { kind: "accepted" }>,
) {
  return {
    run_id: outcome.run_id,
    unit_id: outcome.unit_id,
    tickets_resolved: [...outcome.tickets_resolved],
    committed_now: outcome.committed_now,
  };
}

/** A route wrapper whose perform LOSES the response after the surface answers (transient transport loss). */
class ResponseLosingRoute implements OperatorActionRoute {
  performCalls = 0;
  loseResponses = true;
  constructor(private readonly inner: OperatorActionRoute) {}
  observe(target: { unit: string | null }) {
    return this.inner.observe(target);
  }
  async perform(command: Parameters<OperatorActionRoute["perform"]>[0]): Promise<CanonicalOutcome> {
    this.performCalls += 1;
    const outcome = await this.inner.perform(command);
    if (this.loseResponses) {
      throw new Error("the connection dropped before the response arrived");
    }
    return outcome;
  }
}

describe("T09 Operation Isomorphism (REQ-AFF-001/002, REQ-OPS-002/003, REQ-OUT-004, REQ-VERIFY-003)", () => {
  const servers: ReturnType<typeof Bun.serve>[] = [];

  function httpRoute(stub: CanonicalSurfaceStub): HttpOperatorActionRoute {
    const server = stub.serveHttp();
    servers.push(server);
    return new HttpOperatorActionRoute(`127.0.0.1:${server.port}`);
  }

  afterAll(() => {
    for (const server of servers) {
      server.stop();
    }
  });

  it("registers exactly the canonical surface's operation vocabulary (no invented operations)", () => {
    expect(operationIds()).toEqual(["unit.resolve", "unit.retry", "state.refresh"]);
    // ActionKind wire tokens and the capability-handshake vocabulary.
    expect(OPERATION_REGISTRY["unit.resolve"].action_kind).toBe("resolve");
    expect(OPERATION_REGISTRY["unit.retry"].action_kind).toBe("retry");
    expect(OPERATION_REGISTRY["state.refresh"].action_kind).toBe("refresh");
    expect(OPERATION_REGISTRY["unit.resolve"].surface_capability).toBe("action_resolve");
    expect(OPERATION_REGISTRY["unit.retry"].surface_capability).toBe("action_retry");
    expect(OPERATION_REGISTRY["state.refresh"].surface_capability).toBe("state_refresh");
    // The consequential operations are the §16.3 idempotent-reentry shape; the
    // read-shaped refresh is not admitted through the action path.
    expect(OPERATION_REGISTRY["unit.resolve"].consequential).toBe(true);
    expect(OPERATION_REGISTRY["unit.resolve"].idempotent_reentry).toBe(true);
    expect(OPERATION_REGISTRY["unit.retry"].consequential).toBe(true);
    expect(OPERATION_REGISTRY["state.refresh"].consequential).toBe(false);
  });

  it("one allowed operation converges on one admission/result shape through every implemented route", async () => {
    // Route A: workbench gateway → HTTP surface. Route C: gateway → internal
    // adapter shim. Route B: the direct CLI surface path (no workbench code).
    const httpStub = scenarioStub();
    const shimStub = scenarioStub();
    const cliStub = scenarioStub();

    const gatewayA = new OperationGateway(httpRoute(httpStub));
    const gatewayC = new OperationGateway(shimStub.asInProcessRoute());

    const outcomeA = await gatewayA.invoke(invocation());
    const outcomeC = await gatewayC.invoke(invocation());
    const outcomeB: CanonicalOutcome = cliStub.performViaCli(
      "resolve",
      "AK-901",
      ACTOR,
      "operator resolves the escalation",
      TICKET,
    );

    for (const outcome of [outcomeA, outcomeC]) {
      expect(outcome.class).toBe("accepted");
      expect(outcome.operation).toBe("unit.resolve");
    }
    if (outcomeA.class !== "accepted" || outcomeC.class !== "accepted") {
      throw new Error("the gateway outcomes were not accepted");
    }
    expect(acceptedFacts(outcomeA)).toEqual({
      run_id: RUN,
      unit_id: "AK-901",
      tickets_resolved: [TICKET],
      committed_now: true,
    });
    // All three routes converge on the SAME canonical accepted shape.
    expect(acceptedFacts(outcomeA)).toEqual(acceptedFacts(outcomeC));
    if (outcomeB.kind !== "accepted") {
      throw new Error("the direct CLI surface outcome was not accepted");
    }
    expect(acceptedFacts(outcomeA)).toEqual(acceptedFacts(outcomeB));
    // Each route reached the ONE canonical admission of its scenario exactly once.
    expect(httpStub.sideEffects.settlements_committed).toBe(1);
    expect(shimStub.sideEffects.settlements_committed).toBe(1);
    expect(cliStub.sideEffects.settlements_committed).toBe(1);
    expect(cliStub.sideEffects.admission_attempts).toBe(1);
  });

  it("the same idempotency key re-invoked through a DIFFERENT route commits once (§16.3 re-entry)", async () => {
    // ONE host gateway over TWO routes (HTTP + in-process shim) into ONE
    // canonical admission: the second route's re-invoke of the same key
    // re-enters the same durable settlement instead of committing twice —
    // the isomorphic idempotency shape (REQ-OPS-002). The payload carries the
    // canonical precondition anchor (the run) per REQ-OPS-002.
    const shared = scenarioStub();
    const routeA = httpRoute(shared);
    const routeC = shared.asInProcessRoute();
    const gateway = new OperationGateway(routeA);

    const anchored = invocation({
      payload: { note: "operator resolves the escalation", run: RUN },
    });
    const first = await gateway.invoke(anchored);
    expect(first.class).toBe("accepted");
    const key = first.idempotency_key;
    expect(key.length).toBeGreaterThan(0);

    const second = await gateway.invokeVia(routeC, { ...anchored, idempotency_key: key });
    expect(second.class).toBe("accepted");
    if (second.class !== "accepted") {
      throw new Error("the re-driven invocation was not accepted");
    }
    expect(second.committed_now).toBe(false);
    expect(second.idempotency_key).toBe(key);
    // Exactly one durable commit; the still-open ticket resolved exactly once.
    expect(shared.sideEffects.settlements_committed).toBe(1);
    expect(shared.sideEffects.tickets_resolved).toBe(1);
  });

  it("key reuse with a changed payload is refused before routing on every workbench route", async () => {
    const shared = scenarioStub();
    const routeA = httpRoute(shared);
    const routeC = shared.asInProcessRoute();
    const gateway = new OperationGateway(routeA);

    const first = await gateway.invoke(invocation({ payload: { note: "first payload" } }));
    expect(first.class).toBe("accepted");
    const key = first.idempotency_key;

    for (const route of [routeA, routeC]) {
      const mismatch: GatewayOutcome = await gateway.invokeVia(
        route,
        invocation({ idempotency_key: key, payload: { note: "CHANGED payload" } }),
      );
      expect(mismatch.class).toBe("refused");
      if (mismatch.class === "refused") {
        expect(mismatch.code).toBe("idempotency_key_payload_mismatch");
        expect(mismatch.stale).toBe(true);
      }
    }
    // The mismatch refusals never reached admission — zero additional attempts.
    expect(shared.sideEffects.admission_attempts).toBe(1);
    expect(shared.sideEffects.settlements_committed).toBe(1);
  });

  it("a lost admission response resolves outcome_unknown and reconciles under the SAME key", async () => {
    const stub = scenarioStub();
    const losing = new ResponseLosingRoute(stub.asInProcessRoute());
    const gateway = new OperationGateway(losing);

    const anchored = invocation({
      payload: { note: "operator resolves the escalation", run: RUN },
    });
    const lost = await gateway.invoke(anchored);
    expect(lost.class).toBe("outcome_unknown");
    if (lost.class === "outcome_unknown") {
      expect(lost.operation).toBe("unit.resolve");
      expect(lost.idempotency_key.length).toBeGreaterThan(0);
    }
    expect(gateway.isOutcomeUnknown(lost.idempotency_key)).toBe(true);
    // The command DID reach canonical admission (the settle committed) — the
    // response was lost, so the honest state is unknown, not failed.
    expect(stub.sideEffects.settlements_committed).toBe(1);

    // The transport recovers; reconciliation re-drives the SAME key (never a
    // new key) through the canonical re-entry shape.
    losing.loseResponses = false;
    const reconciled = await gateway.reconcile(lost.idempotency_key, anchored);
    expect(reconciled.class).toBe("accepted");
    if (reconciled.class === "accepted") {
      expect(reconciled.committed_now).toBe(false);
      expect(reconciled.idempotency_key).toBe(lost.idempotency_key);
    }
    expect(gateway.isOutcomeUnknown(lost.idempotency_key)).toBe(false);
    expect(stub.sideEffects.settlements_committed).toBe(1);

    // Reconciling a changed payload under the same key is refused.
    await expect(
      gateway.reconcile(lost.idempotency_key, invocation({ payload: { note: "different" } })),
    ).resolves.toMatchObject({ class: "refused", code: "idempotency_key_payload_mismatch" });
    // Reconciling a never-issued key is refused — no silent new keys.
    await expect(
      gateway.reconcile("unit.resolve:AK-901:forged-key", invocation()),
    ).resolves.toMatchObject({ class: "refused", code: "unknown_idempotency_key" });
  });

  it("affordance resolution converges across routes BEFORE any invocation", async () => {
    const httpStub = scenarioStub();
    const shimStub = scenarioStub();
    const routeA = httpRoute(httpStub);
    const routeC = shimStub.asInProcessRoute();

    const observedA = await routeA.observe({ unit: "AK-901" });
    const observedC = await routeC.observe({ unit: "AK-901" });
    const observedB = scenarioStub().observe({ unit: "AK-901" });

    const descriptors: readonly AffordanceDescriptor[] = [observedA, observedC, observedB].map(
      (observation) =>
        resolveAffordance(OPERATION_IDS.UNIT_RESOLVE, FOCUS, observation, {
          session: { actor: ACTOR, authenticated: true },
        }),
    );
    const reference = descriptors[0];
    if (reference === undefined) {
      throw new Error("no affordance descriptor resolved from any route's observation");
    }
    for (const descriptor of descriptors) {
      expect(descriptor).toEqual(reference);
    }
    expect(reference.status).toBe("requires_attention");
  });
});
