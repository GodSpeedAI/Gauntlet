/**
 * Tests for plan `workbench-agent-interaction.md` increment P1: composer
 * `prime` -> `prime.launch`, `message` -> `prime.message`
 * (resume/steer a child turn-by-turn), and `prime.cancel` ->
 * `RealPrimeHost.stopChild`. Composer-launched children run in work mode
 * (tools enabled, `--cwd` = project root, T13-bound).
 *
 * Dispatcher-level teeth use the in-memory `PrimeHostAdapter` test double
 * (via `WorkbenchHost`'s test-only `primeHostOverride`) over a real HTTP
 * mock canonical surface (the same pattern `workbench-host.test.ts` already
 * uses for D6) so admission is exercised for real, without spawning a
 * process. The work-mode argv teeth spawn a REAL tiny recording script
 * through `RealPrimeHost` directly.
 */

import { describe, expect, it } from "bun:test";
import { chmodSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { createComposerState, cycleComposerMode } from "./composer";
import { resolveConfig } from "./config";
import { FocusRef } from "./focus";
import { PrimeHostAdapter } from "./prime-host";
import { RealPrimeHost } from "./real-prime-host";
import { WorkbenchHost } from "./workbench-host";

/** A tiny mock canonical surface: one unit, configurable status, no escalations. */
function serveCanonicalSurface(options: { unitId: string; status: string }) {
  let posts = 0;
  const server = Bun.serve({
    port: 0,
    hostname: "127.0.0.1",
    fetch(request) {
      const url = new URL(request.url);
      if (request.method === "POST") {
        posts++;
        return Response.json({
          run_id: "run-p1",
          unit_id: options.unitId,
          committed_now: true,
          tickets_resolved: [],
        });
      }
      if (url.pathname === "/api/v1/state") {
        return Response.json({
          run: null,
          units: [{ id: options.unitId, status: options.status, tier: "contained" }],
          escalated: [],
          budget: { tokens: 99 },
        });
      }
      if (url.pathname === "/api/v1/capabilities") {
        return Response.json({
          protocol_version: "1.0.0",
          server_version: "test",
          capabilities: [],
          unsupported_operations: [],
        });
      }
      return Response.json({
        unit_id: options.unitId,
        claim: "P1 unit",
        authority: "operator",
        blast_radius: "local",
        verifications: [],
      });
    },
  });
  return { server, postCount: () => posts };
}

async function focusedHost(options: { unitId: string; status: string; adapter: PrimeHostAdapter }) {
  const { server, postCount } = serveCanonicalSurface({
    unitId: options.unitId,
    status: options.status,
  });
  const host = new WorkbenchHost(
    resolveConfig({
      env: {},
      cli: {
        gauntlet_endpoint: server.url.toString(),
        projection_store: ":memory:",
        session_token: "test-only",
        // A placeholder path is fine — primeHostOverride replaces the adapter
        // RealPrimeHost would otherwise construct; nothing here ever spawns.
        prime_binary: "/nonexistent/prime-agent",
      },
    }),
    { actor: "operator", primeHostOverride: options.adapter },
  );
  await host.refresh();
  const ref = FocusRef.for("gauntlet", "WorkItem", options.unitId);
  host.interactions(() => {}).selectFocus?.(ref);
  return { host, server, postCount, ref };
}

describe("workbench-agent-interaction P1: prime.launch", () => {
  it("launch admitted → the notice is a receipt (child session id), never a completion claim", async () => {
    const adapter = new PrimeHostAdapter();
    const { host, server } = await focusedHost({ unitId: "AK-P1-1", status: "fail", adapter });
    try {
      await host.submit("prime", "prime-child", "do the thing");
      expect(host.notice).toContain("prime.launch");
      expect(host.notice).toContain("admitted");
      expect(host.notice).toContain("receipt only, not a completion claim");
      expect(host.lastChildSessionId).not.toBeNull();
      const session = host.lastChildSessionId ? adapter.getSession(host.lastChildSessionId) : null;
      expect(session).not.toBeNull();
      // Nonblocking admission: never "completed"/"done" claimed by the notice.
      expect(host.notice?.toLowerCase() ?? "").not.toContain("completed");
      expect(host.notice?.toLowerCase() ?? "").not.toContain("done");
    } finally {
      host.close();
      await server.stop(true);
    }
  });

  it("launch refused (no available admission) → zero spawn, exact resolver reason codes surfaced", async () => {
    const adapter = new PrimeHostAdapter();
    // status "open": unit.retry -> readiness_not_earned; unit.resolve ->
    // no_open_ticket (no escalation observed). Neither is available.
    const { host, server } = await focusedHost({ unitId: "AK-P1-2", status: "open", adapter });
    try {
      const before = adapter.listSessions().length;
      await host.submit("prime", "prime-child", "do the thing");
      expect(host.notice).toContain("prime.launch: refused");
      expect(host.notice).toContain("no_open_ticket");
      expect(host.notice).toContain("readiness_not_earned");
      expect(adapter.listSessions().length).toBe(before); // zero spawn
      expect(host.lastChildSessionId).toBeNull();
    } finally {
      host.close();
      await server.stop(true);
    }
  });

  it("prime not configured → a clear notice, nothing else degrades", async () => {
    const host = new WorkbenchHost(
      resolveConfig({ env: {}, cli: { projection_store: ":memory:" } }),
    );
    try {
      expect(host.primeDispatcher).toBeNull();
      await host.submit("prime", "prime-child", "do the thing");
      expect(host.notice).toContain("prime.launch");
      expect(host.notice).toContain("Prime is not configured");
      expect(host.notice).toContain("GAUNTLET_WORKBENCH_PRIME_BINARY");
    } finally {
      host.close();
    }
  });

  it("launch with no Focus selected refuses without touching the adapter", async () => {
    const adapter = new PrimeHostAdapter();
    const { server } = serveCanonicalSurface({ unitId: "AK-P1-3", status: "fail" });
    const host = new WorkbenchHost(
      resolveConfig({
        env: {},
        cli: {
          gauntlet_endpoint: server.url.toString(),
          projection_store: ":memory:",
          session_token: "test-only",
          prime_binary: "/nonexistent/prime-agent",
        },
      }),
      { actor: "operator", primeHostOverride: adapter },
    );
    try {
      await host.submit("prime", "prime-child", "do the thing");
      expect(host.notice).toContain("select a unit Focus");
      expect(adapter.listSessions()).toEqual([]);
    } finally {
      host.close();
      await server.stop(true);
    }
  });
});

describe("workbench-agent-interaction P1: prime.message", () => {
  it("message to a finished child resumes the SAME session with a new turn (never a second session)", async () => {
    const adapter = new PrimeHostAdapter();
    const { host, server } = await focusedHost({ unitId: "AK-P1-4", status: "fail", adapter });
    try {
      await host.submit("prime", "prime-child", "first turn");
      const sessionId = host.lastChildSessionId;
      expect(sessionId).not.toBeNull();
      if (sessionId === null) throw new Error("unreachable");

      // Finish the turn (as watch() would after a real process exits).
      adapter.deliverResultContract(sessionId, {
        session_id: sessionId,
        schema_valid: true,
        outcome: "pass",
        evidence_path: "/dev/null",
        completed_at: new Date().toISOString(),
      });
      expect(adapter.getSession(sessionId)?.handle.status).toBe("completed");

      await host.submit("message", "workbench", "follow-up turn");
      expect(host.notice).toContain("prime.message");
      expect(host.notice).toContain("delivered");
      expect(host.notice).toContain("resumed_new_turn");
      expect(host.notice).toContain("communication, not completion");

      // Same session — no second session minted.
      expect(adapter.listSessions().map((s) => s.handle.session_id)).toEqual([sessionId]);
      const messages = adapter.getSession(sessionId)?.messages ?? [];
      expect(
        messages.some((m) => m.content === "follow-up turn" && m.kind === "communication"),
      ).toBe(true);
    } finally {
      host.close();
      await server.stop(true);
    }
  });

  it("message routing is idempotent per delivery: repeated sends record separate communication entries against the SAME session, never a duplicate session", async () => {
    const adapter = new PrimeHostAdapter();
    const { host, server } = await focusedHost({ unitId: "AK-P1-5", status: "fail", adapter });
    try {
      await host.submit("prime", "prime-child", "first turn");
      const sessionId = host.lastChildSessionId;
      if (sessionId === null) throw new Error("unreachable");

      await host.submit("message", "workbench", "message one");
      await host.submit("message", "workbench", "message one"); // same text resent

      expect(adapter.listSessions().length).toBe(1);
      expect(host.lastChildSessionId).toBe(sessionId);
      const messages = (adapter.getSession(sessionId)?.messages ?? []).filter(
        (m) => m.content === "message one",
      );
      // Both explicit sends are recorded (at-least-once delivery per C1); the
      // invariant under test is "same session, not a duplicate session or an
      // uncontrolled spawn" — never a fabricated single-record de-dup this
      // adapter does not implement.
      expect(messages.length).toBe(2);
    } finally {
      host.close();
      await server.stop(true);
    }
  });

  it("message with no launched child yet → clear notice, nothing sent", async () => {
    const adapter = new PrimeHostAdapter();
    const { host, server } = await focusedHost({ unitId: "AK-P1-6", status: "fail", adapter });
    try {
      await host.submit("message", "workbench", "hello");
      expect(host.notice).toContain("prime.message");
      expect(host.notice).toContain("no child session to message yet");
      expect(adapter.listSessions()).toEqual([]);
    } finally {
      host.close();
      await server.stop(true);
    }
  });
});

describe("workbench-agent-interaction P1: prime.cancel", () => {
  it("cancel is idempotent: stops once, then reports already-terminal with zero further effect", async () => {
    const adapter = new PrimeHostAdapter();
    const { host, server } = await focusedHost({ unitId: "AK-P1-7", status: "fail", adapter });
    try {
      await host.submit("prime", "prime-child", "long task");
      const sessionId = host.lastChildSessionId;
      if (sessionId === null) throw new Error("unreachable");

      await host.cancelPrimeChild();
      expect(host.notice).toContain("prime.cancel");
      expect(host.notice).toContain("stopped");
      expect(adapter.getSession(sessionId)?.handle.status).toBe("failed");

      await host.cancelPrimeChild();
      expect(host.notice).toContain("already terminal");
    } finally {
      host.close();
      await server.stop(true);
    }
  });

  it("cancel with nothing launched yet → clear notice, no throw", async () => {
    const adapter = new PrimeHostAdapter();
    const { host, server } = await focusedHost({ unitId: "AK-P1-8", status: "fail", adapter });
    try {
      await host.cancelPrimeChild();
      expect(host.notice).toContain("prime.cancel");
      expect(host.notice).toContain("no child session to cancel");
    } finally {
      host.close();
      await server.stop(true);
    }
  });
});

describe("workbench-agent-interaction P1: composer mode cycle", () => {
  it("cycling visits every declared mode and back, always visible via state.mode/notice", () => {
    let state = createComposerState();
    expect(state.mode).toBe("message");
    const seen: string[] = [state.mode];
    for (let i = 0; i < 4; i++) {
      state = cycleComposerMode(state);
      seen.push(state.mode);
      expect(state.notice).toContain(state.mode);
    }
    expect(seen).toEqual(["message", "operation", "prime", "kernel", "message"]);
  });

  it("cycling is a no-op while a takeover owns the input", () => {
    const state = createComposerState();
    const withTakeover = {
      ...state,
      takeover: {
        requestId: "req-1",
        kind: "approval" as const,
        prompt: "approve?",
        destination: "workbench",
        requestedAtMs: 0,
        expiresAtMs: null,
      },
    };
    const next = cycleComposerMode(withTakeover);
    expect(next).toBe(withTakeover);
  });
});

describe("workbench-agent-interaction P1: RealPrimeHost work-mode argv", () => {
  it("a default spawn keeps --no-tools; a work-mode spawn drops it and uses the project-root cwd", async () => {
    const root = mkdtempSync(join(tmpdir(), "p1-argv-"));
    try {
      const defaultCapture = join(root, "default-argv.txt");
      const workCapture = join(root, "work-argv.txt");
      const defaultBin = join(root, "default-bin", "prime-agent");
      const workBin = join(root, "work-bin", "prime-agent");
      writeRecordingScript(defaultBin, defaultCapture);
      writeRecordingScript(workBin, workCapture);

      const defaultHost = new RealPrimeHost({
        binary: defaultBin,
        agentDir: join(root, "default-agent"),
        sessionDir: join(root, "default-sessions"),
        timeoutMs: 5000,
      });
      defaultHost.spawnChild({
        sessionId: "default-1",
        contractPath: join(root, "default-1.result.json"),
        task: "hello",
      });
      await waitForTerminal(defaultHost, "default-1");
      const defaultArgv = readFileSync(defaultCapture, "utf8");
      expect(defaultArgv).toContain("--no-tools");

      const projectRoot = join(root, "project");
      mkdirSync(projectRoot, { recursive: true });
      const workHost = new RealPrimeHost({
        binary: workBin,
        agentDir: join(root, "work-agent"),
        sessionDir: join(root, "work-sessions"),
        projectRoot,
        timeoutMs: 5000,
      });
      workHost.spawnChild({
        sessionId: "work-1",
        contractPath: join(root, "work-1.result.json"),
        task: "hello",
        workMode: { toolsEnabled: true, cwd: projectRoot },
      });
      await waitForTerminal(workHost, "work-1");
      const workArgv = readFileSync(workCapture, "utf8");
      expect(workArgv).not.toContain("--no-tools");
      expect(workArgv).toContain(projectRoot);
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });
});

describe("workbench-agent-interaction P1 bugfix: --resume uses prime-agent's OWN session id", () => {
  it("continueChild resumes with the id parsed from the session event, never the workbench session id", async () => {
    const root = mkdtempSync(join(tmpdir(), "p1-resume-id-"));
    try {
      const capture = join(root, "argv.txt");
      const bin = join(root, "bin", "prime-agent");
      const primeSessionId = "prime-real-id-xyz";
      writeRecordingScript(bin, capture, primeSessionId);

      const host = new RealPrimeHost({
        binary: bin,
        agentDir: join(root, "agent"),
        sessionDir: join(root, "sessions"),
        timeoutMs: 5000,
      });
      const workbenchSessionId = "ws-1";
      host.spawnChild({
        sessionId: workbenchSessionId,
        contractPath: join(root, "ws-1.result.json"),
        task: "turn one",
      });
      await waitForTerminal(host, workbenchSessionId);
      const firstArgv = readFileSync(capture, "utf8");
      expect(firstArgv).not.toContain("--resume");

      const result = host.continueChild(workbenchSessionId, {
        sender: "operator",
        message: "turn two",
      });
      expect(result.delivered).toBe(true);
      expect(result.mode).toBe("resumed_new_turn");
      expect(result.detail).toContain(primeSessionId);

      const deadline = Date.now() + 5000;
      while (host.liveSessionIds().includes(workbenchSessionId) && Date.now() < deadline) {
        await new Promise((resolve) => setTimeout(resolve, 25));
      }
      const secondArgv = readFileSync(capture, "utf8");
      const args = secondArgv.trim().split(/\s+/);
      const resumeIndex = args.indexOf("--resume");
      expect(resumeIndex).toBeGreaterThanOrEqual(0);
      // The --resume VALUE is prime-agent's own id — never the workbench id.
      expect(args[resumeIndex + 1]).toBe(primeSessionId);
      expect(secondArgv).not.toContain(workbenchSessionId);
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });

  it("refuses to resume (delivered:false, zero spawn) when no session event was ever observed", async () => {
    const root = mkdtempSync(join(tmpdir(), "p1-resume-unknown-"));
    try {
      const capture = join(root, "argv.txt");
      const bin = join(root, "bin", "prime-agent");
      // No session line: this script only ever emits assistant text, never
      // `{"type":"session",...}` — an honest simulation of a turn whose
      // prime session id was never learned.
      mkdirSync(dirname(bin), { recursive: true });
      writeFileSync(
        bin,
        `#!/bin/sh\necho "$@" > "${capture}"\nprintf '%s\\n' '{"type":"message_end","message":{"role":"assistant","content":[{"type":"text","text":"ok"}]}}'\nexit 0\n`,
        { mode: 0o755 },
      );
      chmodSync(bin, 0o755);

      const host = new RealPrimeHost({
        binary: bin,
        agentDir: join(root, "agent"),
        sessionDir: join(root, "sessions"),
        timeoutMs: 5000,
      });
      const workbenchSessionId = "ws-2";
      host.spawnChild({
        sessionId: workbenchSessionId,
        contractPath: join(root, "ws-2.result.json"),
        task: "turn one",
      });
      await waitForTerminal(host, workbenchSessionId);
      const firstArgv = readFileSync(capture, "utf8");

      const result = host.continueChild(workbenchSessionId, {
        sender: "operator",
        message: "turn two",
      });
      expect(result.delivered).toBe(false);
      expect(result.mode).toBe("resumed_new_turn");
      expect(result.detail).toContain("not yet known");

      // Nothing new was spawned: the capture file is unchanged.
      expect(readFileSync(capture, "utf8")).toBe(firstArgv);
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });
});

function writeRecordingScript(binPath: string, capturePath: string, sessionId = "s"): void {
  mkdirSync(dirname(binPath), { recursive: true });
  const script = `#!/bin/sh\necho "$@" > "${capturePath}"\nprintf '%s\\n' '{"type":"session","id":"${sessionId}"}'\nexit 0\n`;
  writeFileSync(binPath, script, { mode: 0o755 });
  chmodSync(binPath, 0o755);
}

async function waitForTerminal(host: RealPrimeHost, sessionId: string): Promise<void> {
  const deadline = Date.now() + 5000;
  while (Date.now() < deadline) {
    const status = host.getSession(sessionId)?.handle.status;
    if (status === "completed" || status === "failed") return;
    await new Promise((resolve) => setTimeout(resolve, 25));
  }
  throw new Error(`session '${sessionId}' never reached a terminal state`);
}
