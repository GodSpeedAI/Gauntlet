/**
 * T09 deny-bypass matrix (plan teeth; REQ-AFF-002/003, REQ-CONFIG-011,
 * REQ-OUT-004, REQ-SAFE-002, REQ-SEC-002, REQ-VERIFY-003/004, REQ-UX-003).
 *
 * The SAME operation — unit.resolve on one blocked unit with one open
 * escalation — is driven through EVERY route the workbench currently
 * implements, over the in-process canonical-surface-stub harness (the suite's
 * single admission point, standing in for the ONE Rust write path):
 *
 *   1. `gateway-http`  — workbench OperationGateway → HTTP surface transport
 *                        (the production loopback route).
 *   2. `gateway-shim`  — workbench OperationGateway → in-process adapter shim.
 *   3. `direct-api`    — the surface's action endpoint shape, called directly
 *                        with no workbench code in between.
 *   4. `direct-cli`    — the surface's CLI shape (operator names the ticket),
 *                        called directly with no workbench code in between.
 *
 * Matrix scope is the prereg's declared blindspot, honored honestly: routes
 * NOT yet implemented (kernel wrapper T10/T11, plugin callback T15, eligible
 * Prime context T10) are absent here; those tasks re-run this matrix as they
 * land. Within the implemented set the matrix demands convergence: one
 * canonical operation identity, one refusal class, one result shape, and a
 * measurable zero-side-effect ledger on every route — with a negative control
 * proving the assertions distinguish a bypassing shim from a conforming one.
 */

import { afterAll, describe, expect, it } from "bun:test";
import { resolveAffordance, statusPermitsInvocation } from "./affordance";
import { FocusRef } from "./focus";
import {
  type CanonicalActionCommand,
  type CanonicalOutcome,
  type GatewayOutcome,
  HttpOperatorActionRoute,
  OperationGateway,
  type OperationInvocationRequest,
  type OperatorActionRoute,
} from "./operation-gateway";
import { OPERATION_IDS } from "./operation-registry";
import { CanonicalSurfaceStub, type StubSideEffects } from "./testing/canonical-surface-stub";

const RUN = "01J9T09DENYBYPASSRUN";
const TOKEN = "t09-denybypass-token";
const ACTOR = "operator-bob";
const TICKET = "ESC-911";
const UNIT = "AK-911";
const FOCUS = FocusRef.for("gauntlet", "WorkItem", UNIT);
const SESSION = { actor: ACTOR, token: TOKEN } as const;
const NOTE = "operator resolves the escalation";

type DenialClass = "authority_revoked" | "governance_denied" | "stale_snapshot";
type ScenarioDenial = DenialClass | "unaffordable" | "none";

/**
 * The canonical refusal code each denial class earns at the ONE admission
 * body. The vocabulary is pinned to the REAL operator-action admission path —
 * `crates/gauntlet-cli/src/operator_action.rs:500-528` `refusal_code()` plus
 * the surface's own anchoring codes (`no_open_ticket`/`ambiguous_ticket`,
 * operator_action.rs:232/242) — which has NO budget refusal: affordability is
 * a workbench-side RESOLVER policy (see the Correction Round 1 tests), never
 * an admission class.
 */
const CANONICAL_CODE: Record<DenialClass, string> = {
  authority_revoked: "authority_not_permitted",
  governance_denied: "blocking_residual_open",
  stale_snapshot: "run_not_durable",
};

/**
 * One scenario: the denial knobs and whether the canonical READS expose the
 * denial. `visibleInReads: false` models a denial that RACES PAST
 * revalidation — no projection could see it coming; `true` models a stale
 * projection a fresh observation does expose.
 */
interface MatrixSetup {
  readonly denial: ScenarioDenial;
  readonly visibleInReads: boolean;
}

function scenarioStub(setup: MatrixSetup): CanonicalSurfaceStub {
  const stub = new CanonicalSurfaceStub({
    run_id: RUN,
    bound_actor: ACTOR,
    session_token: TOKEN,
  }).addUnit({
    unit_id: UNIT,
    status: "blocked",
    tier: "P3",
    settled: false,
    authority: setup.denial === "authority_revoked" ? "revoked" : "granted",
    authority_visible_in_reads: setup.denial === "authority_revoked" ? setup.visibleInReads : true,
    governance_denied: setup.denial === "governance_denied",
    governance_visible_in_reads: setup.denial === "governance_denied" ? setup.visibleInReads : true,
    open_ticket: TICKET,
  });
  if (setup.denial === "unaffordable") {
    stub.exhaustBudget(setup.visibleInReads);
  }
  if (setup.denial === "stale_snapshot") {
    stub.retireRun();
  }
  return stub;
}

/**
 * The SAME denied invocation every route receives.
 */
function deniedInvocation(setup: MatrixSetup): OperationInvocationRequest {
  return {
    operation: OPERATION_IDS.UNIT_RESOLVE,
    focus: FOCUS,
    session: SESSION,
    payload: setup.denial === "stale_snapshot" ? { note: NOTE, run: RUN } : { note: NOTE },
  };
}

type MatrixRoute = "gateway-http" | "gateway-shim" | "direct-api" | "direct-cli";
const MATRIX_ROUTES: readonly MatrixRoute[] = [
  "gateway-http",
  "gateway-shim",
  "direct-api",
  "direct-cli",
];

/** One route's normalized denial record plus its scenario's side-effect ledger. */
interface DenialRecord {
  readonly route: MatrixRoute;
  readonly operation: string;
  readonly code: string;
  readonly message: string;
  /** True only for the workbench gateway's typed PRE-admission refusals. */
  readonly stale: boolean;
  readonly reason_codes: readonly string[];
  readonly side_effects: StubSideEffects;
}

const servers: ReturnType<typeof Bun.serve>[] = [];

async function invokeDenialRoute(
  route: MatrixRoute,
  setup: MatrixSetup,
): Promise<{ raw: GatewayOutcome | CanonicalOutcome; stub: CanonicalSurfaceStub }> {
  const stub = scenarioStub(setup);
  const request = deniedInvocation(setup);
  switch (route) {
    case "gateway-http": {
      const server = stub.serveHttp();
      servers.push(server);
      const gateway = new OperationGateway(new HttpOperatorActionRoute(`127.0.0.1:${server.port}`));
      return { raw: await gateway.invoke(request), stub };
    }
    case "gateway-shim": {
      const gateway = new OperationGateway(stub.asInProcessRoute());
      return { raw: await gateway.invoke(request), stub };
    }
    case "direct-api": {
      return {
        raw: stub.perform(
          "resolve",
          UNIT,
          ACTOR,
          request.payload.note,
          request.payload.run ?? null,
        ),
        stub,
      };
    }
    case "direct-cli": {
      return {
        raw: stub.performViaCli("resolve", UNIT, ACTOR, request.payload.note, TICKET),
        stub,
      };
    }
  }
}

function denialRecord(
  route: MatrixRoute,
  raw: GatewayOutcome | CanonicalOutcome,
  stub: CanonicalSurfaceStub,
): DenialRecord {
  const side_effects: StubSideEffects = { ...stub.sideEffects };
  if ("class" in raw) {
    if (raw.class !== "refused") {
      throw new Error(`route ${route} was not denied: ${raw.class}`);
    }
    return {
      route,
      operation: raw.operation,
      code: raw.code,
      message: raw.message,
      stale: raw.stale,
      reason_codes: [...raw.reason_codes],
      side_effects,
    };
  }
  if (raw.kind !== "refused") {
    throw new Error(`route ${route} was not denied: ${raw.kind}`);
  }
  return {
    route,
    operation: OPERATION_IDS.UNIT_RESOLVE,
    code: raw.code,
    message: raw.message,
    stale: false,
    reason_codes: [raw.code],
    side_effects,
  };
}

async function driveDenialMatrix(setup: MatrixSetup): Promise<readonly DenialRecord[]> {
  const records: DenialRecord[] = [];
  for (const route of MATRIX_ROUTES) {
    const { raw, stub } = await invokeDenialRoute(route, setup);
    records.push(denialRecord(route, raw, stub));
  }
  return records;
}

function routeRecord(records: readonly DenialRecord[], route: MatrixRoute): DenialRecord {
  const record = records.find((entry) => entry.route === route);
  if (record === undefined) {
    throw new Error(`the matrix has no record for route ${route}`);
  }
  return record;
}

/**
 * The zero-side-effect ledger assertion: a denied operation NEVER commits a
 * settlement or resolves a ticket on any route; admission_attempts counts the
 * routes whose denial was decided at the canonical admission body (racing
 * denials and the direct surface routes) versus refused before it.
 */
function assertNoSideEffects(
  records: readonly DenialRecord[],
  reachedAdmission: (record: DenialRecord) => boolean,
): void {
  for (const record of records) {
    expect(record.side_effects.settlements_committed).toBe(0);
    expect(record.side_effects.tickets_resolved).toBe(0);
    expect(record.side_effects.admission_attempts).toBe(reachedAdmission(record) ? 1 : 0);
  }
}

describe("T09 Deny-Bypass Matrix (REQ-AFF-002/003, REQ-CONFIG-011, REQ-OUT-004, REQ-SAFE-002, REQ-SEC-002, REQ-VERIFY-003/004, REQ-UX-003)", () => {
  afterAll(() => {
    for (const server of servers) {
      server.stop();
    }
  });

  it("Tooth 1 — a control rendered available whose authority is revoked before activation is denied BEFORE side effect, and the projection refresh is observable", async () => {
    const stub = scenarioStub({ denial: "none", visibleInReads: false });
    const server = stub.serveHttp();
    servers.push(server);
    const route = new HttpOperatorActionRoute(`127.0.0.1:${server.port}`);
    const gateway = new OperationGateway(route);

    // 1. The projection renders the control from a fresh observation: available.
    const rendered = resolveAffordance(
      OPERATION_IDS.UNIT_RESOLVE,
      FOCUS,
      await route.observe({ unit: UNIT }),
      {
        session: { actor: ACTOR, authenticated: true },
      },
    );
    expect(rendered.status).toBe("requires_attention");
    expect(statusPermitsInvocation(rendered.status)).toBe(true);

    // 2. Authority is revoked AFTER the render (and the reads expose it).
    stub.revokeAuthority(UNIT, true);

    // 3. Invocation from the STALE control: typed refusal, never a throw.
    const outcome = await gateway.invoke({
      operation: rendered.operation,
      focus: FOCUS,
      session: SESSION,
      payload: { note: NOTE },
    });
    expect(outcome.class).toBe("refused");
    if (outcome.class !== "refused") {
      throw new Error("the stale control was not refused");
    }
    expect(outcome.operation).toBe(OPERATION_IDS.UNIT_RESOLVE);
    expect(outcome.code).toBe("stale_affordance_denied");
    expect(outcome.stale).toBe(true);
    expect(outcome.reason_codes).toContain("authority_not_permitted");
    expect(outcome.descriptor?.status).toBe("unavailable");
    expect(outcome.descriptor?.reason_codes).toContain("authority_not_permitted");
    // The denial landed BEFORE canonical admission: the ledger never moved.
    expect(stub.sideEffects.admission_attempts).toBe(0);
    expect(stub.sideEffects.settlements_committed).toBe(0);
    expect(stub.sideEffects.tickets_resolved).toBe(0);

    // 4. The projection refresh is observable through the SAME route: the
    //    fresh descriptor degrades to unavailable with the same reason, so
    //    the projection can re-render the control as denied.
    const refreshed = resolveAffordance(
      OPERATION_IDS.UNIT_RESOLVE,
      FOCUS,
      await route.observe({ unit: UNIT }),
      {
        session: { actor: ACTOR, authenticated: true },
      },
    );
    expect(refreshed.status).toBe("unavailable");
    expect(refreshed.reason_codes).toContain("authority_not_permitted");
    expect(statusPermitsInvocation(refreshed.status)).toBe(false);
    expect(refreshed.status).not.toBe(rendered.status);
  });

  it("Tooth 1b — a revocation racing past revalidation still denies at canonical admission (defense in depth), before side effect", async () => {
    const stub = scenarioStub({ denial: "none", visibleInReads: false });
    const gateway = new OperationGateway(stub.asInProcessRoute());

    const rendered = resolveAffordance(
      OPERATION_IDS.UNIT_RESOLVE,
      FOCUS,
      stub.observe({ unit: UNIT }),
      { session: { actor: ACTOR, authenticated: true } },
    );
    expect(statusPermitsInvocation(rendered.status)).toBe(true);

    // The revocation is INVISIBLE to reads: the fresh revalidation cannot see
    // it coming; the admission boundary enforces it regardless.
    stub.revokeAuthority(UNIT, false);

    const outcome = await gateway.invoke({
      operation: OPERATION_IDS.UNIT_RESOLVE,
      focus: FOCUS,
      session: SESSION,
      payload: { note: NOTE },
    });
    expect(outcome.class).toBe("refused");
    if (outcome.class !== "refused") {
      throw new Error("the racing revocation was not refused");
    }
    expect(outcome.operation).toBe(OPERATION_IDS.UNIT_RESOLVE);
    expect(outcome.code).toBe("authority_not_permitted");
    expect(outcome.stale).toBe(false);
    expect(outcome.reason_codes).toEqual(["authority_not_permitted"]);
    // The attempt reached the ONE admission body and was refused there —
    // still zero consequential side effect.
    expect(stub.sideEffects.admission_attempts).toBe(1);
    expect(stub.sideEffects.settlements_committed).toBe(0);
    expect(stub.sideEffects.tickets_resolved).toBe(0);
  });

  it("REQ-SAFE-002 — an unobservable surface fails closed with a typed refusal on every gateway route, never a throw", async () => {
    // In-process shim: the observation itself throws (transport loss) —
    // the gateway converts it into a typed refusal, never an exception.
    const shimStub = scenarioStub({ denial: "none", visibleInReads: false });
    shimStub.setUnreachable();
    const shimGateway = new OperationGateway(shimStub.asInProcessRoute());
    const shimOutcome = await shimGateway.invoke({
      operation: OPERATION_IDS.UNIT_RESOLVE,
      focus: FOCUS,
      session: SESSION,
      payload: { note: NOTE },
    });
    expect(shimOutcome.class).toBe("refused");
    if (shimOutcome.class !== "refused") {
      throw new Error("the unreachable shim surface did not refuse");
    }
    expect(shimOutcome.code).toBe("revalidation_unavailable");
    expect(shimOutcome.reason_codes).toEqual(["surface_unreachable"]);
    expect(shimOutcome.stale).toBe(true);
    expect(shimStub.sideEffects.admission_attempts).toBe(0);
    expect(shimStub.sideEffects.settlements_committed).toBe(0);

    // HTTP surface: the read answers 500 — the transport THROWS (Correction
    // Round 1 BLOCKING 2: no non-throwing "unreachable" observation a
    // re-drive could route past), so the gateway refuses with the same typed
    // class as the shim leg, and zero admission contact.
    const httpStub = scenarioStub({ denial: "none", visibleInReads: false });
    httpStub.setUnreachable();
    const server = httpStub.serveHttp();
    servers.push(server);
    const httpGateway = new OperationGateway(
      new HttpOperatorActionRoute(`127.0.0.1:${server.port}`),
    );
    const httpOutcome = await httpGateway.invoke({
      operation: OPERATION_IDS.UNIT_RESOLVE,
      focus: FOCUS,
      session: SESSION,
      payload: { note: NOTE },
    });
    expect(httpOutcome.class).toBe("refused");
    if (httpOutcome.class !== "refused") {
      throw new Error("the unreachable HTTP surface did not refuse");
    }
    expect(httpOutcome.code).toBe("revalidation_unavailable");
    expect(httpOutcome.reason_codes).toEqual(["surface_unreachable"]);
    expect(httpOutcome.stale).toBe(true);
    expect(httpStub.sideEffects.admission_attempts).toBe(0);
    expect(httpStub.sideEffects.settlements_committed).toBe(0);
  });

  it("Tooth 2a — denials the fresh reads expose: every route refuses with zero side effect, and the gateway's refusal preserves the canonical class the direct surface earns", async () => {
    for (const denial of ["authority_revoked", "governance_denied"] as const) {
      const setup: MatrixSetup = { denial, visibleInReads: true };
      const records = await driveDenialMatrix(setup);
      const canonical = CANONICAL_CODE[denial];

      for (const record of records) {
        expect(record.operation).toBe(OPERATION_IDS.UNIT_RESOLVE);
        if (record.route === "gateway-http" || record.route === "gateway-shim") {
          // Typed PRE-admission refusal: the canonical class is preserved
          // verbatim in the machine-readable reason codes.
          expect(record.code).toBe("stale_affordance_denied");
          expect(record.stale).toBe(true);
          expect(record.reason_codes).toContain(canonical);
        } else {
          // Direct surface routes: the canonical class itself.
          expect(record.code).toBe(canonical);
        }
      }
      // Both direct entries reach the ONE admission body and receive the
      // IDENTICAL canonical denial.
      const api = routeRecord(records, "direct-api");
      const cli = routeRecord(records, "direct-cli");
      expect(api.code).toBe(cli.code);
      expect(api.message).toBe(cli.message);
      expect(api.reason_codes).toEqual(cli.reason_codes);
      // Side-effect ledger: refusals before admission on the gateway routes,
      // refusals AT admission on the direct routes — never a side effect.
      assertNoSideEffects(
        records,
        (record) => record.route === "direct-api" || record.route === "direct-cli",
      );
    }
  });

  it("Tooth 2b — denials racing past revalidation: all four routes converge on the identical canonical denial shape with zero side effect", async () => {
    for (const denial of ["authority_revoked", "governance_denied", "stale_snapshot"] as const) {
      const setup: MatrixSetup = { denial, visibleInReads: false };
      const records = await driveDenialMatrix(setup);
      const canonical = CANONICAL_CODE[denial];

      for (const record of records) {
        expect(record.operation).toBe(OPERATION_IDS.UNIT_RESOLVE);
        expect(record.code).toBe(canonical);
      }
      // Deep equality of the canonical denial shape across every route: the
      // direct API entry is the canonical reference. The three API-shaped
      // entries (HTTP, shim, direct) must match it exactly — class, code, and
      // message. The CLI shape carries the same canonical class; only for
      // run_not_durable does its message disclose its own anchor instead.
      const api = routeRecord(records, "direct-api");
      const shape = { class: "refused", code: api.code, message: api.message };
      for (const route of ["gateway-http", "gateway-shim", "direct-api"] as const) {
        const record = routeRecord(records, route);
        expect({ class: "refused", code: record.code, message: record.message }).toEqual(shape);
      }
      const cli = routeRecord(records, "direct-cli");
      if (denial === "stale_snapshot") {
        expect(cli.message).toContain("no longer durable");
      } else {
        expect({ class: "refused", code: cli.code, message: cli.message }).toEqual(shape);
      }
      // Every route's denial was decided at the ONE admission body; none of
      // them moved the consequential ledger.
      assertNoSideEffects(records, () => true);
    }
  });

  it("control — one allowed operation converges on one admission/idempotency/result shape through every route, including outcome_unknown reconciliation re-driving the SAME key", async () => {
    // Leg 1: fresh scenario per route — all four accepted shapes are one.
    const acceptedFacts: {
      route: MatrixRoute;
      facts: {
        run_id: string;
        unit_id: string;
        tickets_resolved: readonly string[];
        committed_now: boolean;
      };
    }[] = [];
    for (const route of MATRIX_ROUTES) {
      const stub = scenarioStub({ denial: "none", visibleInReads: false });
      const request: OperationInvocationRequest = {
        operation: OPERATION_IDS.UNIT_RESOLVE,
        focus: FOCUS,
        session: SESSION,
        payload: { note: NOTE },
      };
      if (route === "gateway-http") {
        const server = stub.serveHttp();
        servers.push(server);
        const outcome = await new OperationGateway(
          new HttpOperatorActionRoute(`127.0.0.1:${server.port}`),
        ).invoke(request);
        if (outcome.class !== "accepted") {
          throw new Error(`route ${route} did not accept: ${outcome.class}`);
        }
        acceptedFacts.push({
          route,
          facts: {
            run_id: outcome.run_id,
            unit_id: outcome.unit_id,
            tickets_resolved: outcome.tickets_resolved,
            committed_now: outcome.committed_now,
          },
        });
      } else if (route === "gateway-shim") {
        const outcome = await new OperationGateway(stub.asInProcessRoute()).invoke(request);
        if (outcome.class !== "accepted") {
          throw new Error(`route ${route} did not accept: ${outcome.class}`);
        }
        acceptedFacts.push({
          route,
          facts: {
            run_id: outcome.run_id,
            unit_id: outcome.unit_id,
            tickets_resolved: outcome.tickets_resolved,
            committed_now: outcome.committed_now,
          },
        });
      } else if (route === "direct-api") {
        const outcome = stub.perform("resolve", UNIT, ACTOR, NOTE, null);
        if (outcome.kind !== "accepted") {
          throw new Error(`route ${route} did not accept: ${outcome.kind}`);
        }
        acceptedFacts.push({
          route,
          facts: {
            run_id: outcome.run_id,
            unit_id: outcome.unit_id,
            tickets_resolved: outcome.tickets_resolved,
            committed_now: outcome.committed_now,
          },
        });
      } else {
        const outcome = stub.performViaCli("resolve", UNIT, ACTOR, NOTE, TICKET);
        if (outcome.kind !== "accepted") {
          throw new Error(`route ${route} did not accept: ${outcome.kind}`);
        }
        acceptedFacts.push({
          route,
          facts: {
            run_id: outcome.run_id,
            unit_id: outcome.unit_id,
            tickets_resolved: outcome.tickets_resolved,
            committed_now: outcome.committed_now,
          },
        });
      }
      expect(stub.sideEffects.settlements_committed).toBe(1);
      expect(stub.sideEffects.tickets_resolved).toBe(1);
    }
    const reference = acceptedFacts[0];
    if (reference === undefined) {
      throw new Error("the matrix produced no allowed-operation result");
    }
    expect(reference.facts).toEqual({
      run_id: RUN,
      unit_id: UNIT,
      tickets_resolved: [TICKET],
      committed_now: true,
    });
    for (const entry of acceptedFacts) {
      expect(entry.facts).toEqual(reference.facts);
    }

    // Leg 2: one shared admission — the SAME key re-driven through a
    // DIFFERENT route re-enters the durable settlement (committed_now false),
    // never commits twice.
    const shared = scenarioStub({ denial: "none", visibleInReads: false });
    const httpServer = shared.serveHttp();
    servers.push(httpServer);
    const routeA = new HttpOperatorActionRoute(`127.0.0.1:${httpServer.port}`);
    const routeC = shared.asInProcessRoute();
    const gateway = new OperationGateway(routeA);
    const anchored: OperationInvocationRequest = {
      operation: OPERATION_IDS.UNIT_RESOLVE,
      focus: FOCUS,
      session: SESSION,
      payload: { note: NOTE, run: RUN },
    };
    const first = await gateway.invoke(anchored);
    if (first.class !== "accepted") {
      throw new Error("the anchored invocation was not accepted");
    }
    const second = await gateway.invokeVia(routeC, {
      ...anchored,
      idempotency_key: first.idempotency_key,
    });
    if (second.class !== "accepted") {
      throw new Error("the same-key re-drive was not accepted");
    }
    expect(second.committed_now).toBe(false);
    expect(second.idempotency_key).toBe(first.idempotency_key);
    expect(shared.sideEffects.settlements_committed).toBe(1);
    expect(shared.sideEffects.tickets_resolved).toBe(1);

    // Leg 3: a lost response is outcome_unknown under the SAME key, and
    // reconciliation re-drives that key — never a fresh one.
    const losingStub = scenarioStub({ denial: "none", visibleInReads: false });
    const inner = losingStub.asInProcessRoute();
    let loseResponses = true;
    const losing: OperatorActionRoute = {
      observe: (target) => inner.observe(target),
      perform: (command) =>
        inner.perform(command).then(() => {
          if (loseResponses) {
            throw new Error("the connection dropped before the response arrived");
          }
          return inner.perform(command);
        }),
    };
    const losingGateway = new OperationGateway(losing);
    const lost = await losingGateway.invoke(anchored);
    expect(lost.class).toBe("outcome_unknown");
    if (lost.class !== "outcome_unknown") {
      throw new Error("the lost response did not resolve outcome_unknown");
    }
    expect(lost.operation).toBe(OPERATION_IDS.UNIT_RESOLVE);
    expect(losingGateway.isOutcomeUnknown(lost.idempotency_key)).toBe(true);
    // The command DID reach the admission; only the response was lost.
    expect(losingStub.sideEffects.settlements_committed).toBe(1);
    // The transport recovers; reconciliation re-drives the SAME key through
    // canonical re-entry.
    loseResponses = false;
    const reconciled = await losingGateway.reconcile(lost.idempotency_key, anchored);
    if (reconciled.class !== "accepted") {
      throw new Error(`reconciliation did not accept: ${reconciled.class}`);
    }
    expect(reconciled.committed_now).toBe(false);
    expect(reconciled.idempotency_key).toBe(lost.idempotency_key);
    expect(losingGateway.isOutcomeUnknown(lost.idempotency_key)).toBe(false);
    expect(losingStub.sideEffects.settlements_committed).toBe(1);
  });

  it("negative control — a deliberately bypassing shim that skips the gateway is caught by the matrix and contained by the authority", async () => {
    /** A bypassing projection: forges an accepted shape without touching admission. */
    class ForgingBypassShim implements OperatorActionRoute {
      performCalls = 0;
      constructor(private readonly inner: OperatorActionRoute) {}
      observe(target: { unit: string | null }) {
        return this.inner.observe(target);
      }
      perform(command: CanonicalActionCommand): Promise<CanonicalOutcome> {
        this.performCalls += 1;
        return Promise.resolve({
          kind: "accepted",
          run_id: "forged-run",
          unit_id: command.unit,
          tickets_resolved: ["forged-ticket"],
          committed_now: true,
        });
      }
    }

    // Part 1: the bypass projection calls the surface transport directly,
    // skipping the gateway, on a healthy scenario — and forges success.
    const bypassStub = scenarioStub({ denial: "none", visibleInReads: false });
    const bypassShim = new ForgingBypassShim(bypassStub.asInProcessRoute());
    const forged = await bypassShim.perform({
      operation: OPERATION_IDS.UNIT_RESOLVE,
      action_kind: "resolve",
      unit: UNIT,
      operator: ACTOR,
      session_token: TOKEN,
      note: NOTE,
      run: null,
      payload_digest: "forged",
      idempotency_key: "forged-key",
    });
    expect(forged.kind).toBe("accepted");
    if (forged.kind !== "accepted") {
      throw new Error("the bypass shim did not forge an acceptance");
    }
    // The matrix's two discriminating assertions BOTH fire on the bypass:
    // the forged result shape diverges from the converged canonical shape...
    const conformingStub = scenarioStub({ denial: "none", visibleInReads: false });
    const conforming = conformingStub.perform("resolve", UNIT, ACTOR, NOTE, null);
    if (conforming.kind !== "accepted") {
      throw new Error("the conforming control did not accept");
    }
    expect({
      run_id: forged.run_id,
      unit_id: forged.unit_id,
      tickets_resolved: forged.tickets_resolved,
      committed_now: forged.committed_now,
    }).not.toEqual({
      run_id: conforming.run_id,
      unit_id: conforming.unit_id,
      tickets_resolved: conforming.tickets_resolved,
      committed_now: conforming.committed_now,
    });
    // ...and the durable ledger exposes the forgery: nothing was admitted,
    // nothing settled, the escalation is still open on a fresh observation.
    expect(bypassStub.sideEffects.admission_attempts).toBe(0);
    expect(bypassStub.sideEffects.settlements_committed).toBe(0);
    expect(bypassStub.sideEffects.tickets_resolved).toBe(0);
    expect(bypassStub.observe({ unit: UNIT }).open_escalations).toEqual([
      { ticket_id: TICKET, unit_id: UNIT },
    ]);

    // Part 2: even plugged IN as a gateway route, the bypassing transport
    // cannot execute a denied operation — the gateway's pre-admission
    // revalidation refuses before any perform call reaches the shim.
    const deniedStub = scenarioStub({ denial: "authority_revoked", visibleInReads: true });
    const containedShim = new ForgingBypassShim(deniedStub.asInProcessRoute());
    const deniedGateway = new OperationGateway(containedShim);
    const contained = await deniedGateway.invoke({
      operation: OPERATION_IDS.UNIT_RESOLVE,
      focus: FOCUS,
      session: SESSION,
      payload: { note: NOTE },
    });
    expect(contained.class).toBe("refused");
    if (contained.class !== "refused") {
      throw new Error("the bypassing route was not contained");
    }
    expect(contained.code).toBe("stale_affordance_denied");
    expect(contained.reason_codes).toContain("authority_not_permitted");
    // The forged transport never executed: zero perform calls, zero ledger.
    expect(containedShim.performCalls).toBe(0);
    expect(deniedStub.sideEffects.admission_attempts).toBe(0);
    expect(deniedStub.sideEffects.settlements_committed).toBe(0);
  });
});

describe("T09 Correction Round 1 (adversarial verification findings)", () => {
  const correctionServers: ReturnType<typeof Bun.serve>[] = [];

  afterAll(() => {
    for (const server of correctionServers) {
      server.stop();
    }
  });

  function httpRoute(stub: CanonicalSurfaceStub): HttpOperatorActionRoute {
    const server = stub.serveHttp();
    correctionServers.push(server);
    return new HttpOperatorActionRoute(`127.0.0.1:${server.port}`);
  }

  it("C1 — a refused invocation never arms a gate skip: byte-identical retries stay refused with the gate RE-RUN, zero side effects", async () => {
    // Scenario A: unaffordable-by-declaration (workbench resolver policy).
    const budgetStub = scenarioStub({ denial: "unaffordable", visibleInReads: true });
    const budgetGateway = new OperationGateway(httpRoute(budgetStub));
    const unaffordable: OperationInvocationRequest = {
      operation: OPERATION_IDS.UNIT_RESOLVE,
      focus: FOCUS,
      session: SESSION,
      payload: { note: NOTE },
      idempotency_key: "c1-budget-key",
      resolution: {
        payment_estimates: {
          [OPERATION_IDS.UNIT_RESOLVE]: { tokens: 2_000_000, cost_usd_micro: 9_000_000 },
        },
      },
    };
    const first = await budgetGateway.invoke(unaffordable);
    expect(first.class).toBe("refused");
    if (first.class !== "refused") {
      throw new Error("the oversized-estimate invocation was not refused");
    }
    expect(first.reason_codes).toContain("budget_exhausted");
    expect(budgetStub.sideEffects.admission_attempts).toBe(0);
    // Byte-identical retry under the SAME key: the refusal must NOT have
    // armed a re-drive; the fresh gate re-runs and refuses again.
    const retry = await budgetGateway.invoke(unaffordable);
    expect(retry.class).toBe("refused");
    if (retry.class !== "refused") {
      throw new Error("the byte-identical retry of a refused invocation was not refused");
    }
    expect(retry.code).toBe("stale_affordance_denied");
    expect(retry.reason_codes).toContain("budget_exhausted");
    expect(budgetStub.sideEffects.admission_attempts).toBe(0);
    expect(budgetStub.sideEffects.settlements_committed).toBe(0);

    // Scenario B: authority-revoked denial; the retry must re-run the gate,
    // not surface an admission-level refusal through a gate skip.
    const authorityStub = scenarioStub({ denial: "authority_revoked", visibleInReads: true });
    const authorityGateway = new OperationGateway(authorityStub.asInProcessRoute());
    const authorityRequest: OperationInvocationRequest = {
      operation: OPERATION_IDS.UNIT_RESOLVE,
      focus: FOCUS,
      session: SESSION,
      payload: { note: NOTE },
      idempotency_key: "c1-authority-key",
    };
    const deniedFirst = await authorityGateway.invoke(authorityRequest);
    expect(deniedFirst.class).toBe("refused");
    if (deniedFirst.class !== "refused") {
      throw new Error("the authority-denied invocation was not refused");
    }
    expect(deniedFirst.code).toBe("stale_affordance_denied");
    expect(authorityStub.sideEffects.admission_attempts).toBe(0);
    const deniedRetry = await authorityGateway.invoke(authorityRequest);
    expect(deniedRetry.class).toBe("refused");
    if (deniedRetry.class !== "refused") {
      throw new Error("the authority-denied retry was not refused");
    }
    expect(deniedRetry.code).toBe("stale_affordance_denied");
    expect(deniedRetry.stale).toBe(true);
    expect(authorityStub.sideEffects.admission_attempts).toBe(0);
    expect(authorityStub.sideEffects.settlements_committed).toBe(0);
  });

  it("C2 — EVERY drive fails closed when the surface cannot be observed: invoke, caller-supplied re-drive, recomputed-key re-drive, and reconcile", async () => {
    // (a) An unknown-outcome key reconciled while the HTTP reads are dead:
    //     observability fail-closed applies to reconciliation too.
    const reconStub = scenarioStub({ denial: "none", visibleInReads: false });
    const reconHttp = httpRoute(reconStub);
    let dropped = false;
    const flakyThenHttp: OperatorActionRoute = {
      observe: (target) => reconHttp.observe(target),
      perform: (command) => {
        if (!dropped) {
          dropped = true;
          return Promise.reject(new Error("the response was lost"));
        }
        return reconHttp.perform(command);
      },
    };
    const reconGateway = new OperationGateway(flakyThenHttp);
    const anchored: OperationInvocationRequest = {
      operation: OPERATION_IDS.UNIT_RESOLVE,
      focus: FOCUS,
      session: SESSION,
      payload: { note: NOTE, run: RUN },
    };
    const lost = await reconGateway.invoke(anchored);
    expect(lost.class).toBe("outcome_unknown");
    if (lost.class !== "outcome_unknown") {
      throw new Error("the dropped response did not resolve outcome_unknown");
    }
    expect(reconStub.sideEffects.settlements_committed).toBe(0);
    reconStub.setUnreachable();
    const reconciled = await reconGateway.reconcile(lost.idempotency_key, anchored);
    expect(reconciled.class).toBe("refused");
    if (reconciled.class !== "refused") {
      throw new Error("reconciliation under dead reads was not refused");
    }
    expect(reconciled.code).toBe("revalidation_unavailable");
    expect(reconStub.sideEffects.admission_attempts).toBe(0);
    expect(reconStub.sideEffects.settlements_committed).toBe(0);

    // (b) A caller-supplied LEDGERED key re-invoked while unreachable: no
    //     skip exists, and observability fails closed.
    const deadStub = scenarioStub({ denial: "none", visibleInReads: false });
    const deadGateway = new OperationGateway(httpRoute(deadStub));
    const armed: OperationInvocationRequest = {
      operation: OPERATION_IDS.UNIT_RESOLVE,
      focus: FOCUS,
      session: SESSION,
      payload: { note: NOTE },
      idempotency_key: "c2-caller-key",
      resolution: {
        payment_estimates: {
          [OPERATION_IDS.UNIT_RESOLVE]: { tokens: 2_000_000, cost_usd_micro: 9_000_000 },
        },
      },
    };
    const armingRefusal = await deadGateway.invoke(armed);
    expect(armingRefusal.class).toBe("refused");
    deadStub.setUnreachable();
    const reDriven = await deadGateway.invoke({ ...armed, resolution: undefined });
    expect(reDriven.class).toBe("refused");
    if (reDriven.class !== "refused") {
      throw new Error("the re-drive under dead reads was not refused");
    }
    expect(reDriven.code).toBe("revalidation_unavailable");
    expect(deadStub.sideEffects.admission_attempts).toBe(0);
    expect(deadStub.sideEffects.settlements_committed).toBe(0);

    // (c) A DETERMINISTICALLY RECOMPUTED minted key (the key was never
    //     secret) re-invoked while unreachable: the fresh gate pass fails
    //     closed on observability — the gate, not key secrecy, is the defense.
    const recomputeStub = scenarioStub({ denial: "none", visibleInReads: false });
    const recomputeGateway = new OperationGateway(httpRoute(recomputeStub));
    recomputeStub.setUnreachable();
    const fresh = await recomputeGateway.invoke({
      operation: OPERATION_IDS.UNIT_RESOLVE,
      focus: FOCUS,
      session: SESSION,
      payload: { note: NOTE },
    });
    expect(fresh.class).toBe("refused");
    if (fresh.class !== "refused") {
      throw new Error("the fresh unreachable invocation was not refused");
    }
    // The minted key is deterministic and is NOT hidden: the defense is the
    // observability gate, never key secrecy.
    expect(fresh.idempotency_key.length).toBeGreaterThan(0);
    const recomputed = await recomputeGateway.invoke({
      operation: OPERATION_IDS.UNIT_RESOLVE,
      focus: FOCUS,
      session: SESSION,
      payload: { note: NOTE },
      idempotency_key: fresh.idempotency_key,
    });
    expect(recomputed.class).toBe("refused");
    if (recomputed.class !== "refused") {
      throw new Error("the recomputed-key re-drive under dead reads was not refused");
    }
    expect(recomputed.code).toBe("revalidation_unavailable");
    expect(recomputeStub.sideEffects.admission_attempts).toBe(0);
    expect(recomputeStub.sideEffects.settlements_committed).toBe(0);
  });

  it("C3 — concurrent double-invoke of ONE request never splits refusal+execution", async () => {
    // Denied pair: both outcomes are denials, zero side effects.
    const deniedStub = scenarioStub({ denial: "unaffordable", visibleInReads: true });
    const deniedGateway = new OperationGateway(deniedStub.asInProcessRoute());
    const deniedRequest: OperationInvocationRequest = {
      operation: OPERATION_IDS.UNIT_RESOLVE,
      focus: FOCUS,
      session: SESSION,
      payload: { note: NOTE },
      resolution: {
        payment_estimates: {
          [OPERATION_IDS.UNIT_RESOLVE]: { tokens: 2_000_000, cost_usd_micro: 9_000_000 },
        },
      },
    };
    const deniedPair = await Promise.all([
      deniedGateway.invoke(deniedRequest),
      deniedGateway.invoke(deniedRequest),
    ]);
    for (const outcome of deniedPair) {
      expect(outcome.class).toBe("refused");
      if (outcome.class === "refused") {
        expect(outcome.reason_codes).toContain("budget_exhausted");
      }
    }
    expect(deniedStub.sideEffects.admission_attempts).toBe(0);
    expect(deniedStub.sideEffects.settlements_committed).toBe(0);

    // Allowed pair: ONE canonical execution; the second concurrent drive is
    // the §16.3 re-entry (committed_now false), never a second commit.
    const allowedStub = scenarioStub({ denial: "none", visibleInReads: false });
    const allowedGateway = new OperationGateway(allowedStub.asInProcessRoute());
    const allowedRequest: OperationInvocationRequest = {
      operation: OPERATION_IDS.UNIT_RESOLVE,
      focus: FOCUS,
      session: SESSION,
      payload: { note: NOTE, run: RUN },
    };
    const allowedPair = await Promise.all([
      allowedGateway.invoke(allowedRequest),
      allowedGateway.invoke(allowedRequest),
    ]);
    for (const outcome of allowedPair) {
      expect(outcome.class).toBe("accepted");
      if (outcome.class !== "accepted") {
        throw new Error(`the allowed concurrent pair contained ${outcome.class}`);
      }
    }
    const committed = allowedPair.filter(
      (outcome) => outcome.class === "accepted" && outcome.committed_now,
    );
    expect(committed).toHaveLength(1);
    expect(allowedPair[0]?.idempotency_key).toBe(allowedPair[1]?.idempotency_key);
    expect(allowedStub.sideEffects.settlements_committed).toBe(1);
    expect(allowedStub.sideEffects.tickets_resolved).toBe(1);
  });

  it("C4 — a registered read-shaped operation refuses with the typed not_action_admitted reason, never unsupported_operation", async () => {
    const stub = scenarioStub({ denial: "none", visibleInReads: false });
    const gateway = new OperationGateway(stub.asInProcessRoute());
    const refresh = await gateway.invoke({
      operation: OPERATION_IDS.STATE_REFRESH,
      focus: FOCUS,
      session: SESSION,
      payload: { note: NOTE },
    });
    expect(refresh.class).toBe("refused");
    if (refresh.class !== "refused") {
      throw new Error("the read-shaped operation was not refused");
    }
    expect(refresh.code).toBe("not_action_admitted");
    expect(refresh.reason_codes).toEqual(["not_action_admitted"]);
    expect(refresh.operation).toBe(OPERATION_IDS.STATE_REFRESH);
    expect(stub.sideEffects.admission_attempts).toBe(0);
  });

  it("C5 — a lying 2xx with an empty body is a failed outcome; committed_now is never fabricated", async () => {
    const stub = scenarioStub({ denial: "none", visibleInReads: false });
    const lying = Bun.serve({
      port: 0,
      async fetch(request) {
        const url = new URL(request.url);
        if (request.method === "GET" && url.pathname === "/api/v1/state") {
          const observation = stub.observe({ unit: UNIT });
          return Response.json({
            run: RUN,
            units: observation.units,
            open_escalations: observation.open_escalations,
            budget_remaining: observation.budget_remaining,
            governance_denied_units: observation.governance_denied_units,
            authority_revoked_units: observation.authority_revoked_units,
          });
        }
        return new Response("{}", { status: 200, headers: { "Content-Type": "application/json" } });
      },
    });
    correctionServers.push(lying);
    const gateway = new OperationGateway(new HttpOperatorActionRoute(`127.0.0.1:${lying.port}`));
    const outcome = await gateway.invoke({
      operation: OPERATION_IDS.UNIT_RESOLVE,
      focus: FOCUS,
      session: SESSION,
      payload: { note: NOTE },
    });
    expect(outcome.class).toBe("failed");
    if (outcome.class !== "failed") {
      throw new Error(`the lying 2xx resolved ${outcome.class}`);
    }
    expect(outcome.code).toBe("malformed_acceptance");
    expect(stub.sideEffects.admission_attempts).toBe(0);
    expect(stub.sideEffects.settlements_committed).toBe(0);
  });

  it("C6 — the admission body has NO budget gate (MAJOR 1): unaffordable is a workbench resolver refusal, and the canonical surface never sees a gateway-denied request", async () => {
    // Through BOTH gateway transports the oversized estimate refuses BEFORE
    // admission (typed reason budget_exhausted, zero perform calls).
    for (const transport of ["http", "shim"] as const) {
      const stub = scenarioStub({ denial: "unaffordable", visibleInReads: true });
      const gateway =
        transport === "http"
          ? new OperationGateway(httpRoute(stub))
          : new OperationGateway(stub.asInProcessRoute());
      const outcome = await gateway.invoke({
        operation: OPERATION_IDS.UNIT_RESOLVE,
        focus: FOCUS,
        session: SESSION,
        payload: { note: NOTE },
        resolution: {
          payment_estimates: {
            [OPERATION_IDS.UNIT_RESOLVE]: { tokens: 2_000_000, cost_usd_micro: 9_000_000 },
          },
        },
      });
      expect(outcome.class).toBe("refused");
      if (outcome.class !== "refused") {
        throw new Error(`the unaffordable invocation via ${transport} was not refused`);
      }
      expect(outcome.code).toBe("stale_affordance_denied");
      expect(outcome.reason_codes).toContain("budget_exhausted");
      expect(stub.sideEffects.admission_attempts).toBe(0);
      expect(stub.sideEffects.settlements_committed).toBe(0);
    }
    // The canonical admission itself carries NO budget refusal: on identical
    // scenarios, the direct surface routes (which bypass the workbench
    // resolver entirely) are ACCEPTED — the workbench can refuse to route its
    // own projections; it cannot and must not fabricate canonical denials.
    for (const route of ["direct-api", "direct-cli"] as const) {
      const stub = scenarioStub({ denial: "unaffordable", visibleInReads: true });
      const outcome =
        route === "direct-api"
          ? stub.perform("resolve", UNIT, ACTOR, NOTE, null)
          : stub.performViaCli("resolve", UNIT, ACTOR, NOTE, TICKET);
      expect(outcome.kind).toBe("accepted");
      if (outcome.kind !== "accepted") {
        throw new Error(`the direct ${route} drive was not accepted: ${outcome.kind}`);
      }
      expect(outcome.committed_now).toBe(true);
      expect(stub.sideEffects.settlements_committed).toBe(1);
    }
  });
});

describe("T09 Correction Round 2 (adversarial verification, failed-arm finding)", () => {
  const round2Servers: ReturnType<typeof Bun.serve>[] = [];

  afterAll(() => {
    for (const server of round2Servers) {
      server.stop();
    }
  });

  it("C8a — a FAILED drive arms nothing: budget shrinks visibly, same-key retry with a now-oversized estimate is refused PRE-admission", async () => {
    const stub = scenarioStub({ denial: "none", visibleInReads: false });
    const inner = stub.asInProcessRoute();
    let failFirst = true;
    const flaky: OperatorActionRoute = {
      observe: (target) => inner.observe(target),
      perform: (command) =>
        failFirst
          ? Promise.resolve({
              kind: "failed" as const,
              code: "surface_error",
              message: "simulated 503",
            })
          : inner.perform(command),
    };
    const gateway = new OperationGateway(flaky);

    // Drive 1: healthy world, AFFORDABLE estimate — the gate passes, the
    // admission reports a failed outcome (no settlement, no durable record).
    const drive1 = await gateway.invoke({
      operation: OPERATION_IDS.UNIT_RESOLVE,
      focus: FOCUS,
      session: SESSION,
      payload: { note: NOTE },
      resolution: {
        payment_estimates: {
          [OPERATION_IDS.UNIT_RESOLVE]: { tokens: 100_000, cost_usd_micro: 1_000_000 },
        },
      },
    });
    expect(drive1.class).toBe("failed");
    if (drive1.class !== "failed") {
      throw new Error("drive 1 did not fail");
    }
    expect(stub.sideEffects.settlements_committed).toBe(0);
    failFirst = false;

    // The world changes: budget exhausted, VISIBLE to reads — a fresh gate
    // would refuse. Estimates are not part of the payload digest, so the
    // retry mints the SAME key.
    stub.exhaustBudget(true);
    const drive2 = await gateway.invoke({
      operation: OPERATION_IDS.UNIT_RESOLVE,
      focus: FOCUS,
      session: SESSION,
      payload: { note: NOTE },
      resolution: {
        payment_estimates: {
          [OPERATION_IDS.UNIT_RESOLVE]: { tokens: 2_000_000, cost_usd_micro: 9_000_000 },
        },
      },
    });
    expect(drive2.class).toBe("refused");
    if (drive2.class !== "refused") {
      throw new Error(`the failed-drive retry was ${drive2.class}, not refused`);
    }
    expect(drive2.code).toBe("stale_affordance_denied");
    expect(drive2.stale).toBe(true);
    expect(drive2.reason_codes).toContain("budget_exhausted");
    expect(stub.sideEffects.admission_attempts).toBe(0);
    expect(stub.sideEffects.settlements_committed).toBe(0);
  });

  it("C8b — a FAILED drive arms nothing: authority revoked visibly, same-key retry is refused at the fresh gate, admission never contacted", async () => {
    const stub = scenarioStub({ denial: "none", visibleInReads: false });
    const inner = stub.asInProcessRoute();
    let failFirst = true;
    const flaky: OperatorActionRoute = {
      observe: (target) => inner.observe(target),
      perform: (command) =>
        failFirst
          ? Promise.resolve({
              kind: "failed" as const,
              code: "surface_error",
              message: "simulated 503",
            })
          : inner.perform(command),
    };
    const gateway = new OperationGateway(flaky);
    const plain: OperationInvocationRequest = {
      operation: OPERATION_IDS.UNIT_RESOLVE,
      focus: FOCUS,
      session: SESSION,
      payload: { note: NOTE },
    };
    const drive1 = await gateway.invoke(plain);
    expect(drive1.class).toBe("failed");
    failFirst = false;
    stub.revokeAuthority(UNIT, true);
    const drive2 = await gateway.invoke(plain);
    expect(drive2.class).toBe("refused");
    if (drive2.class !== "refused") {
      throw new Error(`the failed-drive retry was ${drive2.class}, not refused`);
    }
    expect(drive2.code).toBe("stale_affordance_denied");
    expect(drive2.stale).toBe(true);
    expect(drive2.reason_codes).toContain("authority_not_permitted");
    expect(stub.sideEffects.admission_attempts).toBe(0);
    expect(stub.sideEffects.settlements_committed).toBe(0);
  });

  it("C8c — a FAILED drive over the real HTTP transport (503 on perform) arms nothing: the same-key retry is refused pre-admission", async () => {
    const stub = scenarioStub({ denial: "none", visibleInReads: false });
    let posts = 0;
    const server = Bun.serve({
      port: 0,
      async fetch(request) {
        const url = new URL(request.url);
        if (request.method === "GET" && url.pathname === "/api/v1/state") {
          const observation = stub.observe({ unit: UNIT });
          return Response.json({
            run: RUN,
            units: observation.units,
            open_escalations: observation.open_escalations,
            budget_remaining: observation.budget_remaining,
            governance_denied_units: observation.governance_denied_units,
            authority_revoked_units: observation.authority_revoked_units,
          });
        }
        posts += 1;
        if (posts === 1) {
          return Response.json(
            { code: "surface_error", message: "the surface is overloaded (503)" },
            { status: 503 },
          );
        }
        const outcome = stub.perform(
          "resolve",
          decodeURIComponent(url.pathname.split("/")[4] ?? ""),
          request.headers.get("X-Gauntlet-Operator") ?? "",
          await request.text(),
          url.searchParams.get("run"),
        );
        if (outcome.kind === "accepted") {
          return Response.json({
            action: "resolve",
            run_id: outcome.run_id,
            unit_id: outcome.unit_id,
            committed_now: outcome.committed_now,
            tickets_resolved: outcome.tickets_resolved,
          });
        }
        return Response.json(
          { code: outcome.code, message: outcome.message },
          { status: outcome.kind === "refused" ? 409 : 500 },
        );
      },
    });
    round2Servers.push(server);
    const gateway = new OperationGateway(new HttpOperatorActionRoute(`127.0.0.1:${server.port}`));

    const drive1 = await gateway.invoke({
      operation: OPERATION_IDS.UNIT_RESOLVE,
      focus: FOCUS,
      session: SESSION,
      payload: { note: NOTE },
    });
    expect(drive1.class).toBe("failed");
    if (drive1.class !== "failed") {
      throw new Error("the 503 drive did not resolve failed");
    }
    expect(posts).toBe(1);

    // The world becomes unaffordable (visible); the same-key retry carries a
    // now-oversized estimate. The failed outcome must arm nothing.
    stub.exhaustBudget(true);
    const drive2 = await gateway.invoke({
      operation: OPERATION_IDS.UNIT_RESOLVE,
      focus: FOCUS,
      session: SESSION,
      payload: { note: NOTE },
      resolution: {
        payment_estimates: {
          [OPERATION_IDS.UNIT_RESOLVE]: { tokens: 2_000_000, cost_usd_micro: 9_000_000 },
        },
      },
    });
    expect(drive2.class).toBe("refused");
    if (drive2.class !== "refused") {
      throw new Error(`the failed-drive HTTP retry was ${drive2.class}, not refused`);
    }
    expect(drive2.code).toBe("stale_affordance_denied");
    expect(drive2.reason_codes).toContain("budget_exhausted");
    // The retry never reached admission: still exactly the one 503 POST.
    expect(posts).toBe(1);
    expect(stub.sideEffects.admission_attempts).toBe(0);
    expect(stub.sideEffects.settlements_committed).toBe(0);
  });
});
