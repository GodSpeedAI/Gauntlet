/**
 * Wire-shape honesty (TUI GUI-redesign increment 2b): the workbench's HTTP
 * operator-action transport versus the REAL Rust surface wire
 * (crates/adapters/gauntlet-adapter-surface-json, `gauntlet serve`).
 *
 * Recon recorded four wire defects (recon/canonical-ops-prime.md §4):
 *
 *   W1 the gateway read a FLAT top-level {code,message} error body, but the
 *      Rust surface ALWAYS nests its typed error facts —
 *      {"error":{"code","message"}} (src/lib.rs:205-213 `error_json`; every
 *      400/401/403/404/405/409/500 body in src/http.rs is built from it) —
 *      so every real refusal collapsed to code "surface_error" with an empty
 *      message, breaking the gateway's own "refusal code preserved" claim.
 *   W2 the gateway POSTed the note as raw text/plain, but the Rust action
 *      path accepts ONLY an empty body or a JSON object whose "note" member
 *      is a string (src/http.rs:494-541); anything else is a typed 400
 *      body_unparsable — free-text notes were refused.
 *   W3 the observe path fabricated null-unit escalation tickets from the
 *      real /state's `escalated` id list, but `snapshot_json`
 *      (src/lib.rs:94-150) serves NO `open_escalations` binding list — and a
 *      null-unit ticket matches EVERY unit in the affordance resolver
 *      (affordance.ts), so the fabrication made unit.resolve over-available
 *      whenever any ticket id existed.
 *   W4 the canonical-surface stub spoke the assumed (flat error, raw text
 *      note) wire, hiding W1/W2 from every suite that drives it.
 *
 * The /state and error servers below are hand-rolled FROM the Rust sources
 * (field names, nesting, and status codes pinned to the cited lines) so the
 * gateway is tested against the surface as BUILT, never against the stub's
 * own dialect; the stub-based legs pin the stub itself to that same wire.
 */

import { afterAll, describe, expect, it } from "bun:test";
import { resolveAffordance, statusPermitsInvocation } from "./affordance";
import { FocusRef } from "./focus";
import { HttpOperatorActionRoute, OperationGateway } from "./operation-gateway";
import { OPERATION_IDS } from "./operation-registry";
import { CanonicalSurfaceStub } from "./testing/canonical-surface-stub";

const RUN = "01JAWIRE2BSTATERUN";
const TOKEN = "wire2b-token";
const ACTOR = "operator-wire";
const UNIT = "AK-940";
const TICKET = "ESC-940";
const FOCUS = FocusRef.for("gauntlet", "WorkItem", UNIT);
const SESSION = { actor: ACTOR, token: TOKEN } as const;

const servers: ReturnType<typeof Bun.serve>[] = [];

function serveStateAndAction(
  state: () => Record<string, unknown>,
  action: () => Response,
): ReturnType<typeof Bun.serve> {
  const server = Bun.serve({
    port: 0,
    async fetch(request) {
      const url = new URL(request.url);
      if (request.method === "GET" && url.pathname === "/api/v1/state") {
        return Response.json(state());
      }
      return action();
    },
  });
  servers.push(server);
  return server;
}

/** A healthy superset /state: the unit is visible, its escalation bound and open. */
function healthySupersetState(): Record<string, unknown> {
  return {
    run: RUN,
    escalated: [TICKET],
    units: [{ id: UNIT, status: "blocked", tier: "P3" }],
    open_escalations: [{ ticket_id: TICKET, unit_id: UNIT }],
  };
}

function healthyStub(): CanonicalSurfaceStub {
  return new CanonicalSurfaceStub({
    run_id: RUN,
    bound_actor: ACTOR,
    session_token: TOKEN,
  }).addUnit({
    unit_id: UNIT,
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

describe("Wire-shape honesty — workbench HTTP transport vs the real Rust surface (increment 2b, W1-W4)", () => {
  afterAll(() => {
    for (const server of servers) {
      server.stop(true);
    }
  });

  it("W1 — a 409 in the REAL nested error shape surfaces the exact canonical code and message through the gateway outcome, never surface_error", async () => {
    const refusalMessage = `unit ${UNIT} has no durable earned authority for an operator action`;
    const server = serveStateAndAction(
      healthySupersetState,
      // The REAL error body (lib.rs:205-213): {"error":{"code","message"}}.
      () =>
        Response.json(
          { error: { code: "authority_not_permitted", message: refusalMessage } },
          { status: 409 },
        ),
    );
    const outcome = await new OperationGateway(
      new HttpOperatorActionRoute(`127.0.0.1:${server.port}`),
    ).invoke({
      operation: OPERATION_IDS.UNIT_RESOLVE,
      focus: FOCUS,
      session: SESSION,
      payload: { note: "wire-shape refusal leg" },
    });
    expect(outcome.class).toBe("refused");
    if (outcome.class !== "refused") {
      throw new Error(`the typed 409 resolved ${outcome.class}`);
    }
    expect(outcome.code).toBe("authority_not_permitted");
    expect(outcome.code).not.toBe("surface_error");
    expect(outcome.message).toBe(refusalMessage);
    expect(outcome.message.length).toBeGreaterThan(0);
    expect(outcome.stale).toBe(false);
    expect(outcome.reason_codes).toEqual(["authority_not_permitted"]);
  });

  it("W1 — a 5xx in the REAL nested error shape surfaces the exact failure code and message (class failed)", async () => {
    const failureMessage = "the surface is overloaded (503)";
    const server = serveStateAndAction(healthySupersetState, () =>
      Response.json(
        { error: { code: "surface_overloaded", message: failureMessage } },
        { status: 503 },
      ),
    );
    const outcome = await new OperationGateway(
      new HttpOperatorActionRoute(`127.0.0.1:${server.port}`),
    ).invoke({
      operation: OPERATION_IDS.UNIT_RESOLVE,
      focus: FOCUS,
      session: SESSION,
      payload: { note: "wire-shape failure leg" },
    });
    expect(outcome.class).toBe("failed");
    if (outcome.class !== "failed") {
      throw new Error(`the typed 503 resolved ${outcome.class}`);
    }
    expect(outcome.code).toBe("surface_overloaded");
    expect(outcome.message).toBe(failureMessage);
  });

  it("W1 resilience — a FLAT {code,message} body stays tolerated; the NESTED shape WINS when both are present", async () => {
    // Leg 1 (tolerance, pinned preserved behavior): a flat top-level body
    // still yields the exact typed facts.
    const flat = serveStateAndAction(healthySupersetState, () =>
      Response.json({ code: "run_not_durable", message: "the anchor is stale" }, { status: 409 }),
    );
    const flatOutcome = await new OperationGateway(
      new HttpOperatorActionRoute(`127.0.0.1:${flat.port}`),
    ).invoke({
      operation: OPERATION_IDS.UNIT_RESOLVE,
      focus: FOCUS,
      session: SESSION,
      payload: { note: "flat tolerance leg" },
    });
    expect(flatOutcome.class).toBe("refused");
    if (flatOutcome.class !== "refused") {
      throw new Error(`the flat 409 resolved ${flatOutcome.class}`);
    }
    expect(flatOutcome.code).toBe("run_not_durable");
    expect(flatOutcome.message).toBe("the anchor is stale");

    // Leg 2 (precedence): both shapes present in one body — nested wins.
    const both = serveStateAndAction(healthySupersetState, () =>
      Response.json(
        {
          error: { code: "nested_refusal", message: "the nested message" },
          code: "flat_refusal",
          message: "the flat message",
        },
        { status: 409 },
      ),
    );
    const bothOutcome = await new OperationGateway(
      new HttpOperatorActionRoute(`127.0.0.1:${both.port}`),
    ).invoke({
      operation: OPERATION_IDS.UNIT_RESOLVE,
      focus: FOCUS,
      session: SESSION,
      payload: { note: "precedence leg" },
    });
    expect(bothOutcome.class).toBe("refused");
    if (bothOutcome.class !== "refused") {
      throw new Error(`the two-shape 409 resolved ${bothOutcome.class}`);
    }
    expect(bothOutcome.code).toBe("nested_refusal");
    expect(bothOutcome.message).toBe("the nested message");
  });

  it("W4 — the stub's action endpoint mirrors the REAL body contract (http.rs:494-541): non-object or non-string-note bodies are typed 400 body_unparsable; an empty body is accepted", async () => {
    const stub = healthyStub();
    const server = stub.serveHttp();
    servers.push(server);
    const actionUrl = `http://127.0.0.1:${server.port}/api/v1/units/${UNIT}/resolve`;
    const headers = {
      "X-Gauntlet-Operator": ACTOR,
      Authorization: `Bearer ${TOKEN}`,
    };

    // A raw free-text body — exactly what the gateway used to send — is a
    // typed 400: it is not parsable JSON at all (http.rs:531-536), so only
    // the code is pinned (the message is the parser's own error text).
    const rawText = await fetch(actionUrl, { method: "POST", headers, body: "a free-text note" });
    expect(rawText.status).toBe(400);
    const rawBody = (await rawText.json()) as { error?: { code?: string; message?: string } };
    expect(rawBody.error?.code).toBe("body_unparsable");

    // A body that IS parsable JSON but not an object (a bare JSON string,
    // number, or array) is the other arm with the Rust message verbatim
    // (http.rs:525-530).
    const nonObject = await fetch(actionUrl, {
      method: "POST",
      headers,
      body: JSON.stringify("a json string is not an object"),
    });
    expect(nonObject.status).toBe(400);
    const nonObjectBody = (await nonObject.json()) as {
      error?: { code?: string; message?: string };
    };
    expect(nonObjectBody.error?.code).toBe("body_unparsable");
    expect(nonObjectBody.error?.message).toBe("the action body must be a JSON object");

    // A JSON body whose "note" is not a string is the other typed 400
    // (http.rs:514-519).
    const badNote = await fetch(actionUrl, {
      method: "POST",
      headers,
      body: JSON.stringify({ note: 42 }),
    });
    expect(badNote.status).toBe(400);
    const badNoteBody = (await badNote.json()) as { error?: { code?: string; message?: string } };
    expect(badNoteBody.error?.code).toBe("body_unparsable");
    expect(badNoteBody.error?.message).toBe("the action body's 'note' must be a JSON string");

    // Unparsable JSON is the same typed 400 (http.rs:531-536).
    const garbage = await fetch(actionUrl, { method: "POST", headers, body: "{not json" });
    expect(garbage.status).toBe(400);
    const garbageBody = (await garbage.json()) as { error?: { code?: string } };
    expect(garbageBody.error?.code).toBe("body_unparsable");

    // An EMPTY body is the accepted no-note shape (http.rs:497-498: an empty
    // trimmed body is the empty note).
    const empty = await fetch(actionUrl, { method: "POST", headers, body: "" });
    expect(empty.status).toBe(200);
  });

  it("W2 — the gateway sends the note as a JSON object body the Rust parser accepts; an empty note sends an empty body", async () => {
    // Leg 1: a free-text note rides as {"note":"..."} with content-type
    // application/json — the body the http.rs parser accepts.
    const NOTE = "free-text note drafted in the composer";
    const stub = healthyStub();
    const server = stub.serveHttp();
    servers.push(server);
    const outcome = await new OperationGateway(
      new HttpOperatorActionRoute(`127.0.0.1:${server.port}`),
    ).invoke({
      operation: OPERATION_IDS.UNIT_RESOLVE,
      focus: FOCUS,
      session: SESSION,
      payload: { note: NOTE },
    });
    expect(outcome.class).toBe("accepted");
    if (outcome.class !== "accepted") {
      throw new Error(`the free-text note drive resolved ${outcome.class}`);
    }
    expect(stub.lastActionRequest?.body_text).toBe(JSON.stringify({ note: NOTE }));
    expect(stub.lastActionRequest?.content_type).toContain("application/json");

    // Leg 2: an empty note sends NO body at all.
    const bareStub = healthyStub();
    const bareServer = bareStub.serveHttp();
    servers.push(bareServer);
    const bare = await new OperationGateway(
      new HttpOperatorActionRoute(`127.0.0.1:${bareServer.port}`),
    ).invoke({
      operation: OPERATION_IDS.UNIT_RESOLVE,
      focus: FOCUS,
      session: SESSION,
      payload: { note: "" },
    });
    expect(bare.class).toBe("accepted");
    if (bare.class !== "accepted") {
      throw new Error(`the empty-note drive resolved ${bare.class}`);
    }
    expect(bareStub.lastActionRequest?.body_text).toBe("");
  });

  it("W3 — a /state in the REAL snapshot shape (no gate fields) yields honest absence: no fabricated tickets, unit.resolve unavailable with no_open_ticket", async () => {
    // The REAL /state body, field-for-field from snapshot_json
    // (lib.rs:94-150): run, escalated (BARE ticket ids — no unit binding),
    // in_flight, units {id,status,tier}, budget {rounds,parallel,depth,
    // wall_seconds,tokens,cost_usd_micro}, sandbox_denials, open_residuals,
    // run_derivation. There is NO open_escalations, budget_remaining,
    // governance_denied_units, or authority_revoked_units.
    const server = serveStateAndAction(
      () => ({
        run: RUN,
        escalated: [TICKET],
        in_flight: [],
        units: [{ id: UNIT, status: "blocked", tier: "P3" }],
        budget: {
          rounds: 1,
          parallel: 1,
          depth: 1,
          wall_seconds: 600,
          tokens: 1000,
          cost_usd_micro: 1000,
        },
        sandbox_denials: [],
        open_residuals: [],
        run_derivation: "derived from the durable artifacts",
      }),
      () => new Response(null, { status: 404 }),
    );
    const route = new HttpOperatorActionRoute(`127.0.0.1:${server.port}`);
    const observation = await route.observe({ unit: UNIT });
    expect(observation.reachable).toBe(true);
    expect(observation.units).toEqual([{ unit_id: UNIT, status: "blocked" }]);
    // Honest absence: the escalated id carries no unit binding, so NO ticket
    // is surfaced — never a fabricated null-unit one.
    expect(observation.open_escalations).toEqual([]);
    expect(observation.budget_remaining).toBeNull();
    expect(observation.governance_denied_units).toEqual([]);
    expect(observation.authority_revoked_units).toEqual([]);

    const descriptor = resolveAffordance(OPERATION_IDS.UNIT_RESOLVE, FOCUS, observation, {
      session: { actor: ACTOR, authenticated: true },
    });
    expect(descriptor.status).toBe("unavailable");
    expect(descriptor.status).not.toBe("requires_attention");
    expect(descriptor.reason_codes).toEqual(["no_open_ticket"]);
    expect(statusPermitsInvocation(descriptor.status)).toBe(false);
  });

  it("W3 contrast — an explicitly SERVED open_escalations list still surfaces verbatim: bound tickets bind, and an explicitly-carried unbound ticket keeps the null-matching rule", async () => {
    // Leg 1: a served, unit-bound escalation still maps verbatim (the fix
    // must prune fabrication, not served facts).
    const bound = serveStateAndAction(
      healthySupersetState,
      () => new Response(null, { status: 404 }),
    );
    const boundRoute = new HttpOperatorActionRoute(`127.0.0.1:${bound.port}`);
    const boundObservation = await boundRoute.observe({ unit: UNIT });
    expect(boundObservation.open_escalations).toEqual([{ ticket_id: TICKET, unit_id: UNIT }]);
    const boundDescriptor = resolveAffordance(OPERATION_IDS.UNIT_RESOLVE, FOCUS, boundObservation, {
      session: { actor: ACTOR, authenticated: true },
    });
    expect(boundDescriptor.status).toBe("requires_attention");
    expect(boundDescriptor.required_attention_item).toBe(TICKET);

    // Leg 2: a served UNBOUND ticket (unit_id null) is carried as served, and
    // the resolver's null-matching rule stays available for it — only the
    // observation's own explicit carrier may produce a null-unit ticket,
    // never an absent field (pinned by the W3 test above).
    const unbound = serveStateAndAction(
      () => ({
        run: RUN,
        escalated: [TICKET],
        units: [{ id: UNIT, status: "blocked", tier: "P3" }],
        open_escalations: [{ ticket_id: TICKET, unit_id: null }],
      }),
      () => new Response(null, { status: 404 }),
    );
    const unboundRoute = new HttpOperatorActionRoute(`127.0.0.1:${unbound.port}`);
    const unboundObservation = await unboundRoute.observe({ unit: UNIT });
    expect(unboundObservation.open_escalations).toEqual([{ ticket_id: TICKET, unit_id: null }]);
    const unboundDescriptor = resolveAffordance(
      OPERATION_IDS.UNIT_RESOLVE,
      FOCUS,
      unboundObservation,
      { session: { actor: ACTOR, authenticated: true } },
    );
    expect(unboundDescriptor.status).toBe("requires_attention");
    expect(unboundDescriptor.required_attention_item).toBe(TICKET);
  });
});
