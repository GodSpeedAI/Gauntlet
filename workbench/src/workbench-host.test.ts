import { describe, expect, it } from "bun:test";
import { existsSync, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { resolveConfig } from "./config";
import { FocusRef } from "./focus";
import { RealPrimeHost } from "./real-prime-host";
import { ShellSession } from "./shell";
import { writeFakePrimeAgent } from "./testing/fake-prime-binary";
import { primeDirs, WorkbenchHost } from "./workbench-host";
import { resolveCapabilityStatus } from "./workbench-model";

describe("WorkbenchHost: Prime dispatcher binding (Phase C2, D6)", () => {
  it("primeDispatcher is null when prime_binary is unconfigured — Gauntlet behavior is unaffected", () => {
    const host = new WorkbenchHost(
      resolveConfig({ env: {}, cli: { projection_store: ":memory:" } }),
    );
    try {
      expect(host.primeDispatcher).toBeNull();
    } finally {
      host.close();
    }
  });

  it("primeDispatcher is bound to a real RealPrimeHost when prime_binary is configured, with isolated agent/session dirs", () => {
    const root = mkdtempSync(join(tmpdir(), "c2-workbench-host-"));
    try {
      const host = new WorkbenchHost(
        resolveConfig({
          env: {},
          cli: {
            projection_store: ":memory:",
            prime_binary: join(root, "bin", "prime-agent"),
          },
        }),
      );
      try {
        expect(host.primeDispatcher).not.toBeNull();
        expect(host.primeDispatcher?.listRecords()).toEqual([]);
        expect(host.primeDispatcher?.topology()).toEqual([]);
      } finally {
        host.close();
      }
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });

  it("D6: a configured-but-unreachable prime_binary (path does not exist) leaves the full Gauntlet read+invoke cycle identical to the unconfigured case", async () => {
    const root = mkdtempSync(join(tmpdir(), "c2-d6-"));
    let posts = 0;
    const server = Bun.serve({
      port: 0,
      hostname: "127.0.0.1",
      fetch(request) {
        const url = new URL(request.url);
        if (request.method === "POST") {
          posts++;
          return Response.json({
            run_id: "run-d6",
            unit_id: "AK-D6",
            committed_now: true,
            tickets_resolved: [],
          });
        }
        if (url.pathname === "/api/v1/state") {
          return Response.json({
            run: null,
            units: [{ id: "AK-D6", status: "fail", tier: "contained" }],
            escalated: [],
            budget: { tokens: 99 },
          });
        }
        if (url.pathname === "/api/v1/capabilities")
          return Response.json({
            protocol_version: "1.0.0",
            server_version: "test",
            capabilities: [],
            unsupported_operations: [],
          });
        return Response.json({
          unit_id: "AK-D6",
          claim: "D6 unit",
          authority: "operator",
          blast_radius: "local",
          verifications: [],
        });
      },
    });
    // config.prime_binary points at a path shape that is VALID but points to
    // nothing on disk — "configured but unreachable" (D6), distinct from
    // "unconfigured" (prime_binary: null).
    const host = new WorkbenchHost(
      resolveConfig({
        env: {},
        cli: {
          gauntlet_endpoint: server.url.toString(),
          projection_store: ":memory:",
          session_token: "test-only",
          prime_binary: join(root, "bin", "prime-agent-does-not-exist"),
        },
      }),
      { actor: "operator" },
    );
    try {
      expect(host.primeDispatcher).not.toBeNull();
      await host.refresh();
      expect(host.status).toBe("connected");
      expect(host.model.getSnapshot().units[0]?.status).toBe("fail");

      // Tooth 6: installed:true, available:false — never loosened into a
      // claimed reachability this read loop never actually probed.
      const primeCapability = host.observation.capabilities.find((c) => c.capability === "prime");
      expect(primeCapability).toBeDefined();
      expect(primeCapability?.installed).toBe(true);
      const resolved = primeCapability ? resolveCapabilityStatus(primeCapability) : null;
      expect(resolved?.status).toBe("unavailable");
      expect(resolved?.installed).toBe(true);

      const ref = FocusRef.for("gauntlet", "WorkItem", "AK-D6");
      await host.invoke("unit.retry", ref, "D6 probe");
      expect(posts).toBe(1);
      expect(host.notice).toContain("accepted");
    } finally {
      host.close();
      await server.stop(true);
      rmSync(root, { recursive: true, force: true });
    }
  });
});

describe("production workbench host", () => {
  it("projects canonical Rust wire, preserves navigation across polls, reconnects and admits operations through fresh state", async () => {
    let status = "open";
    let connected = true;
    let present = true;
    let posts = 0;
    let gets = 0;
    const server = Bun.serve({
      port: 0,
      hostname: "127.0.0.1",
      fetch(request) {
        const url = new URL(request.url);
        if (!connected) return new Response("unavailable", { status: 503 });
        if (request.method === "POST") {
          posts++;
          expect(request.headers.get("X-Gauntlet-Operator")).toBe("operator");
          expect(request.headers.get("Authorization")).toBe("Bearer test-only");
          return Response.json({
            run_id: "run-1",
            unit_id: "AK-901",
            committed_now: true,
            tickets_resolved: [],
          });
        }
        if (url.pathname === "/api/v1/state") {
          gets++;
          return Response.json({
            run: null,
            units: present ? [{ id: "AK-901", status, tier: "contained" }] : [],
            escalated: [],
            budget: { tokens: 99 },
          });
        }
        if (url.pathname === "/api/v1/capabilities")
          return Response.json({
            protocol_version: "1.0.0",
            server_version: "test",
            capabilities: ["action_retry"],
            unsupported_operations: ["launch"],
          });
        return Response.json({
          unit_id: "AK-901",
          claim: "Canonical unit",
          authority: "operator",
          blast_radius: "local",
          verifications: [],
        });
      },
    });
    const host = new WorkbenchHost(
      resolveConfig({
        env: {},
        cli: {
          gauntlet_endpoint: server.url.toString(),
          projection_store: ":memory:",
          session_token: "test-only",
        },
      }),
      { actor: "operator" },
    );
    try {
      await host.refresh();
      expect(host.status).toBe("connected");
      expect(host.runId).toBeNull();
      expect(host.model.getSnapshot().units[0]).toMatchObject({
        unit_id: "AK-901",
        status: "open",
        title: "Canonical unit",
      });
      expect(host.observation.units[0]?.unit_id).toBe("AK-901");
      expect(host.observation.capabilities[0]).toMatchObject({
        capability: "action_retry",
        installed: true,
        gates: { governance: false },
      });
      expect(host.observation.budget_remaining).toBeNull(); // spent budget is not remaining budget
      const ref = FocusRef.for("gauntlet", "WorkItem", "AK-901");
      host.interactions(() => {}).selectFocus?.(ref);
      host.interactions(() => {}).hoist?.(ref);
      const horizon = host.model.getSnapshot().horizon;
      status = "fail";
      await host.refresh();
      expect(host.model.getSnapshot().horizon).toBe(horizon);
      expect(host.model.getSnapshot().focus).toBe(ref);
      expect(host.model.getSnapshot().units[0]?.status).toBe("fail");
      const before = gets;
      await host.invoke("unit.retry", ref, "Operator retry");
      expect(gets).toBeGreaterThan(before);
      expect(posts).toBe(1);
      expect(host.notice).toContain("accepted");
      connected = false;
      await host.refresh();
      expect(host.status).toBe("disconnected");
      expect(host.observation.reachable).toBe(false);
      expect(host.model.getSnapshot().units).toHaveLength(1);
      await host.invoke("unit.retry", ref);
      expect(posts).toBe(1);
      expect(host.notice).toContain("revalidation_unavailable");
      connected = true;
      present = false;
      await host.refresh();
      expect(host.status).toBe("empty");
      expect(host.model.getSnapshot().units).toHaveLength(0);
      expect(host.model.getSnapshot().focus).toBeNull();
      await host.submit("message", "workbench", "do not execute");
      expect(host.notice).toContain("Message was not sent");
      expect(posts).toBe(1);
    } finally {
      host.close();
      await server.stop(true);
    }
  });

  it("names the declared-but-unbound planned operation for prime/message/kernel submissions (Phase C1, REQ-OPS-003)", async () => {
    const host = new WorkbenchHost(
      resolveConfig({ env: {}, cli: { projection_store: ":memory:" } }),
    );
    try {
      await host.submit("message", "workbench", "hello");
      expect(host.notice).toContain("prime.message");
      expect(host.notice).toContain("declared (planned, not yet bound");
      expect(host.notice).toContain("Message was not sent.");

      await host.submit("prime", "prime-child", "goal text");
      expect(host.notice).toContain("prime.launch");
      expect(host.notice).toContain("declared (planned, not yet bound");

      await host.submit("kernel", "repl", "1 + 1");
      expect(host.notice).toContain("prime.steer");
      expect(host.notice).toContain("declared (planned, not yet bound");
    } finally {
      host.close();
    }
  });

  it("routes reverse traversal and pasted command characters as composer text", () => {
    const host = new WorkbenchHost(
      resolveConfig({ env: {}, cli: { projection_store: ":memory:" } }),
    );
    let submitted = 0;
    const shell = new ShellSession(
      { workspace: { workspace: "gauntlet", reference: "test" } },
      { cols: 120, rows: 40 },
      {
        submitComposer: () => {
          submitted++;
        },
      },
    );
    try {
      shell.key("shift+tab", host.model.getSnapshot());
      expect(shell.state.focusedSurface).toBe("Navigator");
      shell.key("e", host.model.getSnapshot());
      shell.paste("q\nunit.retry\u001b");
      expect(shell.composerState.draft).toBe("q\nunit.retry\u001b");
      expect(submitted).toBe(0);
    } finally {
      host.close();
    }
  });
});

describe("WorkbenchHost: Phase D — Prime reattachment survives projection-store deletion (REQ-VERIFY-001 extended over Prime)", () => {
  it("deleting config.projection_store and its sidecars has ZERO effect on Prime session recovery — the qualified records live in a sibling directory, never inside the deletable file", async () => {
    const root = mkdtempSync(join(tmpdir(), "d-recov-projstore-"));
    try {
      const binary = join(root, "bin", "prime-agent");
      writeFakePrimeAgent(binary, { exitCode: 0 });
      const config = resolveConfig({
        env: {},
        cli: {
          projection_store: join(root, "projections.db"),
          prime_binary: binary,
        },
      });

      // WorkbenchHost's OWN dirs derivation — spawn directly against it,
      // bypassing the canonical-dispatch ceremony (irrelevant to this tooth).
      const { agentDir, sessionDir } = primeDirs(config);
      const sessionId = "d-recov-child";
      const contractPath = join(root, `${sessionId}.result.json`);
      const spawner = new RealPrimeHost({ binary, agentDir, sessionDir });
      spawner.spawnChild({ sessionId, parentSessionId: null, contractPath, task: "go" });
      const deadline = Date.now() + 8_000;
      while (spawner.getSession(sessionId)?.handle.status === "running" && Date.now() < deadline) {
        await new Promise((resolve) => setTimeout(resolve, 50));
      }
      expect(spawner.getSession(sessionId)?.handle.status).toBe("completed");

      // Destroy the projection store exactly as RECOV-001A's procedure does
      // (the file and its WAL/SHM sidecars) — never the parent directory.
      for (const suffix of ["", "-wal", "-shm"]) {
        rmSync(`${config.projection_store}${suffix}`, { force: true });
      }
      expect(existsSync(config.projection_store)).toBe(false);

      // A FRESH WorkbenchHost, pointed at the SAME config, recovers the
      // Prime session from qualified records alone — its own projection
      // store construction (SqliteProjectionStore) rebuilds a fresh empty
      // Gauntlet file, entirely independent of Prime's sessionDir.
      const fresh = new WorkbenchHost(config);
      try {
        expect(fresh.primeDispatcher).not.toBeNull();
        const topology = fresh.primeDispatcher?.topology() ?? [];
        expect(topology.map((node) => node.sessionId)).toEqual([sessionId]);
        expect(topology[0]?.status).toBe("completed");
      } finally {
        fresh.close();
      }
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  }, 15_000);
});

describe("WorkbenchHost: ICM §30 BUSY_SUBMISSION wiring (host pendingOperations, follow-on)", () => {
  it("keys `${operation}:${focus.id}` as pending only for the admission round-trip, clearing it for accepted and refused outcomes alike", async () => {
    const gate: { release: (() => void) | null } = { release: null };
    let postMode: "accept" | "refuse" = "accept";
    const server = Bun.serve({
      port: 0,
      hostname: "127.0.0.1",
      async fetch(request) {
        const url = new URL(request.url);
        if (request.method === "POST") {
          // Held open until the test explicitly releases it, so the pending
          // key can be observed mid-flight — the whole point of this suite.
          await new Promise<void>((resolve) => {
            gate.release = resolve;
          });
          if (postMode === "refuse") {
            return Response.json(
              { error: { code: "denied", message: "not permitted right now" } },
              { status: 409 },
            );
          }
          return Response.json({
            run_id: "run-1",
            unit_id: "AK-901",
            committed_now: true,
            tickets_resolved: [],
          });
        }
        if (url.pathname === "/api/v1/state") {
          return Response.json({
            run: null,
            units: [{ id: "AK-901", status: "fail", tier: "contained" }],
            escalated: [],
            budget: { tokens: 99 },
          });
        }
        if (url.pathname === "/api/v1/capabilities")
          return Response.json({
            protocol_version: "1.0.0",
            server_version: "test",
            capabilities: ["action_retry"],
            unsupported_operations: [],
          });
        return Response.json({
          unit_id: "AK-901",
          claim: "Canonical unit",
          authority: "operator",
          blast_radius: "local",
          verifications: [],
        });
      },
    });
    const host = new WorkbenchHost(
      resolveConfig({
        env: {},
        cli: {
          gauntlet_endpoint: server.url.toString(),
          projection_store: ":memory:",
          session_token: "test-only",
        },
      }),
      { actor: "operator" },
    );
    try {
      await host.refresh();
      const ref = FocusRef.for("gauntlet", "WorkItem", "AK-901");
      const key = `unit.retry:${ref.id}`;

      // Accepted outcome: pending mid-flight, cleared once settled.
      expect(host.pendingOperations.has(key)).toBe(false);
      const accepted = host.invoke("unit.retry", ref, "probe 1");
      await new Promise((resolve) => setTimeout(resolve, 20));
      expect(host.pendingOperations.has(key)).toBe(true);
      gate.release?.();
      await accepted;
      expect(host.pendingOperations.has(key)).toBe(false);
      expect(host.notice).toContain("accepted");

      // Refused outcome: same pending window, same clearing guarantee — the
      // `finally` block is not conditioned on the outcome class.
      postMode = "refuse";
      const refused = host.invoke("unit.retry", ref, "probe 2");
      await new Promise((resolve) => setTimeout(resolve, 20));
      expect(host.pendingOperations.has(key)).toBe(true);
      gate.release?.();
      await refused;
      expect(host.pendingOperations.has(key)).toBe(false);
      expect(host.notice).toContain("refused");
    } finally {
      host.close();
      await server.stop(true);
    }
  });

  it("never marks state.refresh as pending — it is a local read short-circuit, not a gateway submission", async () => {
    const host = new WorkbenchHost(
      resolveConfig({ env: {}, cli: { projection_store: ":memory:" } }),
    );
    try {
      const focus = FocusRef.for("gauntlet", "WorkItem", "u-any");
      await host.invoke("state.refresh", focus);
      expect(host.pendingOperations.has(`state.refresh:${focus.id}`)).toBe(false);
      expect(host.pendingOperations.size).toBe(0);
    } finally {
      host.close();
    }
  });

  it("clears the pending key even when the server goes unreachable between admission and dispatch", async () => {
    // A REAL server first, so refresh() seeds a unit whose resolver status
    // permits unit.retry — then the server is stopped before invoking.
    // OperationGateway revalidates reachability at invocation time and
    // refuses pre-admission (surface_unreachable) rather than ever calling
    // fetch; invoke() still never rethrows, and this pins the same
    // finally-clears-regardless guarantee for a failure path distinct from a
    // canonical accept/refuse round trip.
    const server = Bun.serve({
      port: 0,
      hostname: "127.0.0.1",
      fetch(request) {
        const url = new URL(request.url);
        if (url.pathname === "/api/v1/state") {
          return Response.json({
            run: null,
            units: [{ id: "AK-901", status: "fail", tier: "contained" }],
            escalated: [],
            budget: { tokens: 99 },
          });
        }
        if (url.pathname === "/api/v1/capabilities")
          return Response.json({
            protocol_version: "1.0.0",
            server_version: "test",
            capabilities: ["action_retry"],
            unsupported_operations: [],
          });
        return Response.json({
          unit_id: "AK-901",
          claim: "Canonical unit",
          authority: "operator",
          blast_radius: "local",
          verifications: [],
        });
      },
    });
    const host = new WorkbenchHost(
      resolveConfig({
        env: {},
        cli: {
          gauntlet_endpoint: server.url.toString(),
          projection_store: ":memory:",
          session_token: "test-only",
        },
      }),
      { actor: "operator" },
    );
    try {
      await host.refresh();
      const ref = FocusRef.for("gauntlet", "WorkItem", "AK-901");
      const key = `unit.retry:${ref.id}`;
      await server.stop(true);
      await host.invoke("unit.retry", ref, "probe");
      expect(host.pendingOperations.has(key)).toBe(false);
      expect(host.notice).toContain("surface_unreachable");
    } finally {
      host.close();
    }
  });
});

describe("WorkbenchHost: ICM §30 state.refresh honesty (GUI redesign, increment 5)", () => {
  it("activating the VISIBLE_DISABLED state.refresh row surfaces its reason instead of silently refreshing", async () => {
    const host = new WorkbenchHost(
      resolveConfig({ env: {}, cli: { projection_store: ":memory:" } }),
    );
    try {
      // Production default observation is the honest empty read: the
      // registry inventories state.refresh but does not action-admit it.
      const focus = FocusRef.for("gauntlet", "WorkItem", "u-any");
      await host.invoke("state.refresh", focus);
      const refused = host.notice ?? "";
      expect(refused.startsWith("state.refresh:")).toBe(true);
      expect(refused).toContain("not_action_admitted");
    } finally {
      host.close();
    }
  });
});
