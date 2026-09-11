/**
 * TEST-SUPPORT minimal canonical read-surface fixture (plan Phase B,
 * workbench-intent-closure.md §4). Serves exactly the three GET routes
 * `GauntletReadClient`/`CanonicalSurfaceStreamPort` read (`/api/v1/state`,
 * `/api/v1/capabilities`, `/api/v1/units/{id}/inspection`) over a real
 * `Bun.serve` loopback, with mutable in-memory state so a test can move the
 * surface through time (unit changes, run changes, restarts, outages) and
 * observe the port's reaction — unlike `testing/canonical-surface-stub.ts`
 * (T09), which models the ACTION surface (`OperatorActionRoute.observe`/
 * `perform`) and does not serve these three routes at all.
 */

export interface FixtureUnit {
  id: string;
  status: string;
  tier?: string;
}

export interface FixtureState {
  run: string | null;
  units: FixtureUnit[];
  protocol_version: string;
  server_version: string;
  reachable: boolean;
}

export interface CanonicalFixture {
  readonly server: ReturnType<typeof Bun.serve>;
  readonly endpoint: string;
  readonly state: FixtureState;
  stop(): Promise<void>;
}

export function startCanonicalFixture(initial: Partial<FixtureState> = {}): CanonicalFixture {
  const state: FixtureState = {
    run: initial.run ?? null,
    units: initial.units ?? [],
    protocol_version: initial.protocol_version ?? "1.0.0",
    server_version: initial.server_version ?? "test",
    reachable: initial.reachable ?? true,
  };

  const server = Bun.serve({
    port: 0,
    hostname: "127.0.0.1",
    fetch(request) {
      const url = new URL(request.url);
      if (!state.reachable) return new Response("unavailable", { status: 503 });
      if (url.pathname === "/api/v1/state") {
        return Response.json({ run: state.run, units: state.units, escalated: [] });
      }
      if (url.pathname === "/api/v1/capabilities") {
        return Response.json({
          protocol_version: state.protocol_version,
          server_version: state.server_version,
          capabilities: [],
          unsupported_operations: [],
        });
      }
      const inspectionMatch = /^\/api\/v1\/units\/([^/]+)\/inspection$/.exec(url.pathname);
      if (inspectionMatch) {
        const unitId = decodeURIComponent(inspectionMatch[1] ?? "");
        return Response.json({
          unit_id: unitId,
          claim: `claim-${unitId}`,
          authority: "operator",
          blast_radius: "local",
          verifications: [],
        });
      }
      return new Response("not found", { status: 404 });
    },
  });

  return {
    server,
    endpoint: server.url.toString(),
    state,
    stop: () => server.stop(true),
  };
}
