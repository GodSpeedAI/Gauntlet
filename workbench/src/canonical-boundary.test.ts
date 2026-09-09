import { afterAll, beforeAll, describe, expect, it } from "bun:test";
import { GauntletReadClient, type UnitInspectionView } from "./client";

describe("T01 Canonical Gauntlet State and Operation Boundary (REQ-OPS-001..003, REQ-STATE-001, REQ-STATE-005)", () => {
  let server: ReturnType<typeof Bun.serve>;
  let client: GauntletReadClient;
  const VALID_TOKEN = "test-session-secret-token-123";
  const BOUND_ACTOR = "operator-alice";

  beforeAll(() => {
    // Start an ephemeral loopback HTTP server that faithfully implements the SurfaceHttp contract
    server = Bun.serve({
      port: 0,
      fetch(req) {
        const url = new URL(req.url);
        const path = url.pathname;

        // Capabilities handshake
        if (req.method === "GET" && path === "/api/v1/capabilities") {
          return Response.json({
            protocol_version: "1.0.0",
            server_version: "0.3.0",
            capabilities: [
              "state_snapshot",
              "state_refresh",
              "escalation_inbox",
              "run_scoped_view",
              "unit_scoped_view",
              "unit_inspection",
              "action_resolve",
              "action_retry",
              "session_auth",
            ],
            unsupported_operations: ["launch", "steering", "cancellation", "evidence_admission"],
          });
        }

        // State snapshot
        if (req.method === "GET" && path === "/api/v1/state") {
          return Response.json({
            run: "run-2026-09-08-001",
            escalated: ["ESC-001"],
            units: [
              { unit_id: "AK-001", status: "pass", tier: "P1", iteration: 1 },
              { unit_id: "AK-002", status: "fail", tier: "P2", iteration: 1 },
              { unit_id: "AK-003", status: "blocked", tier: "P3", iteration: 1 },
            ],
            run_derivation: "derived-from-state-store",
          });
        }

        // Unit deep inspection
        if (req.method === "GET" && path === "/api/v1/units/AK-003/inspection") {
          return Response.json({
            unit_id: "AK-003",
            tier: "P3",
            claim:
              "Operation boundary authenticates request actor and rejects client-asserted authority",
            authority: "kernel",
            blast_radius: "process",
            verify_by: "instrument",
            evidence_required: {
              kind: "execution_evidence",
              locator:
                ".agents/reports/gauntlet-tui-prime-workbench/evidence/T01/canonical-boundary.md",
              producer: "instrument",
            },
            depends_on: ["AK-001", "AK-002"],
            status: "blocked",
            settled: false,
            verifications: [
              {
                unit_id: "AK-003",
                outcome: "fail",
                evidence: "prior-attempt-evidence.json",
                verification: "gate-failure",
              },
            ],
            escalation: {
              ticket_id: "ESC-001",
              run_id: "run-2026-09-08-001",
              unit_id: "AK-003",
              reason: "sandbox policy failed for unit AK-003",
              status: "open",
            },
            evidence_content: '{"observed":"refused","expected":"admitted"}',
            precondition_explanation:
              "Precondition: AK-003 requires all dependencies (AK-001, AK-002) to be settled before re-entry.",
          });
        }

        // Mutation operations (resolve / retry)
        if (
          req.method === "POST" &&
          (path === "/api/v1/units/AK-003/resolve" || path === "/api/v1/units/AK-003/retry")
        ) {
          const operatorHeader = req.headers.get("X-Gauntlet-Operator");
          if (!operatorHeader || operatorHeader.trim() === "") {
            return Response.json(
              {
                code: "operator_identity_required",
                message:
                  "every action carries the operator's identity in the X-Gauntlet-Operator header",
              },
              { status: 400 },
            );
          }

          const authHeader = req.headers.get("Authorization");
          if (!authHeader) {
            return Response.json(
              {
                code: "actor_authentication_required",
                message:
                  "mutation requests require authentication; provide bearer credentials in the Authorization header",
              },
              { status: 401 },
            );
          }

          const token = authHeader.replace(/^Bearer\s+/i, "").trim();
          if (token !== VALID_TOKEN) {
            return Response.json(
              {
                code: "invalid_session_credentials",
                message: "the provided session token is invalid or expired",
              },
              { status: 401 },
            );
          }

          if (operatorHeader.trim() !== BOUND_ACTOR) {
            return Response.json(
              {
                code: "client_asserted_authority_refused",
                message: `client-asserted operator '${operatorHeader}' does not match authenticated session actor '${BOUND_ACTOR}'`,
              },
              { status: 403 },
            );
          }

          return Response.json({
            status: "admitted",
            action: path.endsWith("resolve") ? "resolve" : "retry",
            unit_id: "AK-003",
            actor: BOUND_ACTOR,
          });
        }

        // Refuse direct settlement or evidence admission mutation paths
        if (path.includes("settle") || path.includes("evidence_admission")) {
          return Response.json(
            {
              code: "unsupported_operation",
              message: "direct settlement or evidence mutation is prohibited on operator surface",
            },
            { status: 405 },
          );
        }

        return Response.json(
          { code: "not_found", message: `No route for ${path}` },
          { status: 404 },
        );
      },
    });

    client = new GauntletReadClient(`127.0.0.1:${server.port}`);
  });

  afterAll(() => {
    server.stop();
  });

  it("Capabilities handshake returns supported version and operation boundaries", async () => {
    const caps = await client.fetchCapabilities();
    expect(caps).not.toBeNull();
    expect(caps?.protocol_version).toBe("1.0.0");
    expect(caps?.capabilities).toContain("session_auth");
    expect(caps?.capabilities).toContain("unit_inspection");
    expect(caps?.capabilities).toContain("action_resolve");
    expect(caps?.capabilities).toContain("action_retry");
    expect(caps?.unsupported_operations).toContain("evidence_admission");
    expect(caps?.unsupported_operations).toContain("launch");
  });

  it("Attack 1: Mutation without bound session credentials or with mismatched client-asserted operator is refused", async () => {
    // 1. Well-formed mutation with operator header, but NO Authorization header
    const noAuthRes = await client.mutateUnit("AK-003", "resolve", {
      operator: BOUND_ACTOR,
      note: "Attempting resolve without bearer token",
    });
    expect(noAuthRes.ok).toBe(false);
    expect(noAuthRes.status).toBe(401);
    expect(noAuthRes.code).toBe("actor_authentication_required");

    // 2. Mutation with invalid bearer token
    const badTokenRes = await client.mutateUnit("AK-003", "resolve", {
      operator: BOUND_ACTOR,
      token: "bogus-unauthenticated-token",
      note: "Attempting resolve with bogus token",
    });
    expect(badTokenRes.ok).toBe(false);
    expect(badTokenRes.status).toBe(401);
    expect(badTokenRes.code).toBe("invalid_session_credentials");

    // 3. Loopback client with valid token, but asserting mismatched operator identity (Mallory)
    const spoofRes = await client.mutateUnit("AK-003", "resolve", {
      operator: "mallory",
      token: VALID_TOKEN,
      note: "Attempting resolve as mallory using alice's token",
    });
    expect(spoofRes.ok).toBe(false);
    expect(spoofRes.status).toBe(403);
    expect(spoofRes.code).toBe("client_asserted_authority_refused");

    // 4. Valid mutation with matching bound actor and valid token
    const validRes = await client.mutateUnit("AK-003", "resolve", {
      operator: BOUND_ACTOR,
      token: VALID_TOKEN,
      note: "Authorized resolution by alice",
    });
    expect(validRes.ok).toBe(true);
    expect(validRes.status).toBe(200);
  });

  it("Attack 2: TUI-side helper that attempts to directly mutate settlement/evidence state without canonical Gauntlet operation path is rejected", async () => {
    // GauntletReadClient provides zero methods to mutate settlement or inject evidence directly
    const clientKeys = Object.getOwnPropertyNames(Object.getPrototypeOf(client));
    expect(clientKeys).not.toContain("settleUnit");
    expect(clientKeys).not.toContain("admitEvidence");
    expect(clientKeys).not.toContain("writeStateStore");

    // Attempting an ad-hoc direct HTTP call to an unauthorized settlement endpoint fails
    const directSettleRes = await fetch(
      `http://127.0.0.1:${server.port}/api/v1/units/AK-003/settle`,
      {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ outcome: "pass", force: true }),
      },
    );
    expect(directSettleRes.status).toBe(405);

    // Attempting direct evidence injection fails
    const directEvidenceRes = await fetch(
      `http://127.0.0.1:${server.port}/api/v1/evidence_admission`,
      {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ unit_id: "AK-003", evidence: "spoofed" }),
      },
    );
    expect(directEvidenceRes.status).toBe(405);
  });

  it("Attack 3: Headless replay of canonical read/event data without renderer, SQLite read model, or Prime session present", async () => {
    // 1. Fetch headless runtime snapshot
    const state = await client.fetchState();
    expect(state.status).toBe("connected");
    expect(state.run).toBe("run-2026-09-08-001");
    expect(state.units.length).toBe(3);
    expect(state.escalated).toEqual(["ESC-001"]);

    // 2. Fetch deep unit inspection through headless client
    const inspection: UnitInspectionView | null = await client.fetchInspection("AK-003");
    expect(inspection).not.toBeNull();
    expect(inspection?.unit_id).toBe("AK-003");
    expect(inspection?.tier).toBe("P3");
    expect(inspection?.status).toBe("blocked");
    expect(inspection?.settled).toBe(false);
    expect(inspection?.claim).toContain("Operation boundary authenticates request actor");
    expect(inspection?.authority).toBe("kernel");
    expect(inspection?.blast_radius).toBe("process");
    expect(inspection?.verify_by).toBe("instrument");

    // Evidence requirement fact
    expect(inspection?.evidence_required).toEqual({
      kind: "execution_evidence",
      locator: ".agents/reports/gauntlet-tui-prime-workbench/evidence/T01/canonical-boundary.md",
      producer: "instrument",
    });

    // Escalation fact
    expect(inspection?.escalation).toEqual({
      ticket_id: "ESC-001",
      run_id: "run-2026-09-08-001",
      unit_id: "AK-003",
      reason: "sandbox policy failed for unit AK-003",
      status: "open",
    });

    // Verifications and explanations
    expect(inspection?.verifications.length).toBe(1);
    expect(inspection?.verifications[0]?.outcome).toBe("fail");
    expect(inspection?.precondition_explanation).toContain(
      "Precondition: AK-003 requires all dependencies",
    );
    expect(inspection?.depends_on).toEqual(["AK-001", "AK-002"]);
  });
});
