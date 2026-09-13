/**
 * Tests for plan `workbench-agent-interaction.md` increment P2: Prime child
 * sessions + mailbox + assistant output visible and selectable in the
 * Chat/Navigator surfaces; selecting a child sets the `message` destination
 * and `prime.cancel` target.
 *
 * Dispatcher-level teeth reuse the P1 pattern: the in-memory `PrimeHostAdapter`
 * test double over a real HTTP mock canonical surface, via `WorkbenchHost`'s
 * test-only `primeHostOverride` (see `workbench-agent-interaction-p1.test.ts`).
 */

import { describe, expect, it, test } from "bun:test";
import { createTestRenderer } from "@opentui/core/testing";
import { resolveConfig } from "./config";
import { FocusRef } from "./focus";
import { PrimeHostAdapter } from "./prime-host";
import type { RenderedLine } from "./render-tree";
import type { RenderedShell } from "./shell";
import { createShellState, handleCommand, handlePointer, navigatorOutline } from "./shell";
import { TerminalShellRenderer } from "./terminal-renderer";
import { FULL_DIMS, modelWiredSession, renderOptions, seededModel } from "./testing/shell-harness";
import { renderChatView } from "./views-chat-trace";
import { WorkbenchHost } from "./workbench-host";
import type { FullWorkbenchSnapshot } from "./workbench-model";

/** A tiny mock canonical surface: one unit, configurable status, no escalations. */
function serveCanonicalSurface(options: { unitId: string; status: string }) {
  const server = Bun.serve({
    port: 0,
    hostname: "127.0.0.1",
    fetch(request) {
      const url = new URL(request.url);
      if (request.method === "POST") {
        return Response.json({
          run_id: "run-p2",
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
        claim: "P2 unit",
        authority: "operator",
        blast_radius: "local",
        verifications: [],
      });
    },
  });
  return server;
}

async function focusedHost(options: {
  unitId: string;
  status: string;
  adapter: PrimeHostAdapter;
  pollIntervalMs?: number;
}) {
  const server = serveCanonicalSurface({ unitId: options.unitId, status: options.status });
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
    {
      actor: "operator",
      primeHostOverride: options.adapter,
      ...(options.pollIntervalMs !== undefined ? { pollIntervalMs: options.pollIntervalMs } : {}),
    },
  );
  await host.refresh();
  const ref = FocusRef.for("gauntlet", "WorkItem", options.unitId);
  host.interactions(() => {}).selectFocus?.(ref);
  return { host, server, ref };
}

describe("workbench-agent-interaction P2: listPrimeSessions", () => {
  it("Prime not configured -> empty list", () => {
    const host = new WorkbenchHost(
      resolveConfig({ env: {}, cli: { projection_store: ":memory:" } }),
    );
    try {
      expect(host.primeDispatcher).toBeNull();
      expect(host.listPrimeSessions()).toEqual([]);
    } finally {
      host.close();
    }
  });

  it("after launch: one session with the target unit, status, and admitted_at", async () => {
    const adapter = new PrimeHostAdapter();
    const { host, server } = await focusedHost({ unitId: "AK-P2-1", status: "fail", adapter });
    try {
      await host.submit("prime", "prime-child", "do the thing");
      const sessions = host.listPrimeSessions();
      expect(sessions.length).toBe(1);
      const [firstSession] = sessions;
      if (firstSession === undefined) throw new Error("unreachable");
      expect(firstSession.session_id).toBe(host.lastChildSessionId ?? "");
      expect(firstSession.target_unit).toBe("AK-P2-1");
      expect(["admitted", "running"]).toContain(firstSession.status);
      expect(firstSession.admitted_at).toBeTruthy();
    } finally {
      host.close();
      await server.stop(true);
    }
  });
});

describe("workbench-agent-interaction P2: transcript order and no-completion-claim", () => {
  it("transcript is goal first, then replies in arrival order; a 'done' reply never flips status", async () => {
    const adapter = new PrimeHostAdapter();
    const { host, server } = await focusedHost({ unitId: "AK-P2-2", status: "fail", adapter });
    try {
      await host.submit("prime", "prime-child", "the goal");
      const sessionId = host.lastChildSessionId;
      if (sessionId === null) throw new Error("unreachable");

      adapter.receiveMailboxMessage(sessionId, {
        sender: sessionId,
        recipient: "orchestrator",
        content: "done! all tests pass, completed successfully",
        kind: "communication",
      });

      const session = host.listPrimeSessions().find((s) => s.session_id === sessionId);
      if (session === undefined) throw new Error("unreachable");
      expect(session.messages.length).toBe(2);
      expect(session.messages[0]?.content ?? "").toContain("the goal");
      expect(session.messages[1]?.content ?? "").toContain("done");
      // Attack 1 invariant (AGENTS.md/REQ-PRIME-005): mailbox text saying
      // "done"/"completed" never settles status from communication alone.
      expect(session.status).not.toBe("completed");
      expect(["admitted", "running"]).toContain(session.status);
    } finally {
      host.close();
      await server.stop(true);
    }
  });
});

describe("workbench-agent-interaction P2: live change signal", () => {
  it("a mailbox reply pushes a redraw without waiting for the poll timer", async () => {
    const adapter = new PrimeHostAdapter();
    // A huge poll interval: any redraw beyond the first (start()'s own
    // immediate refresh) must come from the change-listener push, not a
    // second poll tick.
    const { host, server } = await focusedHost({
      unitId: "AK-P2-3",
      status: "fail",
      adapter,
      pollIntervalMs: 999_000,
    });
    try {
      await host.submit("prime", "prime-child", "goal");
      const sessionId = host.lastChildSessionId;
      if (sessionId === null) throw new Error("unreachable");

      let calls = 0;
      host.start(() => {
        calls++;
      });
      // Allow the immediate first poll tick queued by start() to settle.
      await new Promise((resolve) => setTimeout(resolve, 20));
      const callsAfterStart = calls;

      adapter.receiveMailboxMessage(sessionId, {
        sender: sessionId,
        recipient: "orchestrator",
        content: "a reply arrived",
        kind: "communication",
      });

      expect(calls).toBeGreaterThan(callsAfterStart);
    } finally {
      host.close();
      await server.stop(true);
    }
  });
});

describe("workbench-agent-interaction P2: selection sets message/cancel target", () => {
  it("host.selectChildSession sets both the message destination fallback and the cancel target", async () => {
    const adapter = new PrimeHostAdapter();
    const { host, server } = await focusedHost({ unitId: "AK-P2-4", status: "fail", adapter });
    try {
      await host.submit("prime", "prime-child", "first");
      const first = host.lastChildSessionId;
      if (first === null) throw new Error("unreachable");
      await host.submit("prime", "prime-child", "second");
      const second = host.lastChildSessionId;
      if (second === null) throw new Error("unreachable");
      expect(second).not.toBe(first);

      // launch always selects the newest child.
      expect(host.selectedChildSessionId).toBe(second);

      host.selectChildSession(first);
      expect(host.selectedChildSessionId).toBe(first);
      expect(host.lastChildSessionId).toBe(first);
      expect(host.model.getSnapshot().focus?.source_id).toBe(first);

      await host.submit("message", "workbench", "hi");
      const firstMessages = adapter.getSession(first)?.messages ?? [];
      expect(firstMessages.some((m) => m.content === "hi")).toBe(true);
      const secondMessages = adapter.getSession(second)?.messages ?? [];
      expect(secondMessages.some((m) => m.content === "hi")).toBe(false);

      await host.cancelPrimeChild();
      expect(adapter.getSession(first)?.handle.status).toBe("failed");
      expect(adapter.getSession(second)?.handle.status).not.toBe("failed");
    } finally {
      host.close();
      await server.stop(true);
    }
  });

  it("selecting via the keyboard (nav.focus.select) and via a pointer click both go through the SAME select_focus effect and reach the host identically", async () => {
    const adapter = new PrimeHostAdapter();
    const { host, server } = await focusedHost({ unitId: "AK-P2-5", status: "fail", adapter });
    try {
      await host.submit("prime", "prime-child", "goal");
      const sessionId = host.lastChildSessionId;
      if (sessionId === null) throw new Error("unreachable");
      // A launch already selects its own child (P2); reset to isolate what
      // THIS test exercises (Navigator keyboard/pointer selection), not
      // launch's own auto-select.
      host.selectedChildSessionId = null;

      const snapshot: FullWorkbenchSnapshot = {
        ...host.model.getSnapshot(),
        prime_sessions: host.listPrimeSessions(),
        prime_configured: true,
      };
      const rowId = `outline.prime.${sessionId}`;
      const rowIndex = navigatorOutline(snapshot).findIndex((row) => row.id === rowId);
      expect(rowIndex).toBeGreaterThanOrEqual(0);

      // Keyboard path: Navigator focused, cursor on the Prime row, Enter.
      const keyboardHost = host.interactions(() => {});
      const baseState = createShellState(
        { workspace: { workspace: "gauntlet", reference: "ref" } },
        { cols: 120, rows: 40 },
      );
      const keyState = {
        ...baseState,
        focusedSurface: "Navigator" as const,
        navigatorIndex: rowIndex,
      };
      const keyResult = handleCommand(keyState, snapshot, "nav.focus.select");
      expect(keyResult.effects.length).toBe(1);
      const keyEffect = keyResult.effects[0];
      if (keyEffect === undefined || keyEffect.type !== "select_focus") {
        throw new Error("expected a select_focus effect from the keyboard path");
      }
      expect(keyEffect.ref.source_id).toBe(sessionId);
      keyboardHost.selectFocus?.(keyEffect.ref);
      expect(host.selectedChildSessionId as string | null).toBe(sessionId);

      // Reset, then do it again via a pointer click on the same row.
      host.selectedChildSessionId = null;
      const pointerResult = handlePointer(baseState, snapshot, {
        kind: "click",
        surfaceId: "Navigator",
        lineId: rowId,
        x: 0,
      });
      expect(pointerResult.effects.length).toBe(1);
      const pointerEffect = pointerResult.effects[0];
      if (pointerEffect === undefined || pointerEffect.type !== "select_focus") {
        throw new Error("expected a select_focus effect from the pointer path");
      }
      // The SAME semantic command/effect, reached two ways (ICM §35 mouse ≡
      // keyboard): both resolve to the identical FocusRef identity.
      expect(pointerEffect.ref).toBe(keyEffect.ref);
      host.interactions(() => {}).selectFocus?.(pointerEffect.ref);
      expect(host.selectedChildSessionId as string | null).toBe(sessionId);
    } finally {
      host.close();
      await server.stop(true);
    }
  });
});

describe("workbench-agent-interaction P2: Chat surface honesty", () => {
  it("Prime not configured: the Chat surface says so and how", () => {
    const model = seededModel();
    try {
      const snapshot: FullWorkbenchSnapshot = {
        ...model.getSnapshot(),
        prime_configured: false,
      };
      const content = renderChatView({ snapshot, observation: renderOptions(model).observation });
      const text = content.lines.flatMap((line) => line.spans.map((span) => span.text)).join(" ");
      expect(text).toContain("Prime is not configured");
      expect(text).toContain("GAUNTLET_WORKBENCH_PRIME_BINARY");
    } finally {
      model.close();
    }
  });

  it("Prime configured with sessions: rows render with session id, target unit, and status; no completion claim from mailbox text", () => {
    const model = seededModel();
    try {
      const snapshot: FullWorkbenchSnapshot = {
        ...model.getSnapshot(),
        focus: FocusRef.for("prime", "PrimeSession", "prime-u-root-abc"),
        prime_configured: true,
        prime_sessions: [
          {
            session_id: "prime-u-root-abc",
            target_unit: "u-root",
            status: "running",
            admitted_at: "2026-01-01T00:00:00.000Z",
            model: "test-model",
            messages: [
              {
                message_id: "m1",
                session_id: "prime-u-root-abc",
                sender: "orchestrator",
                recipient: "prime-u-root-abc",
                content: JSON.stringify({
                  received_context: "prime-context-envelope/1",
                  task: "do the thing",
                  envelope: { huge: "should not render verbatim" },
                }),
                timestamp: "2026-01-01T00:00:01.000Z",
                kind: "communication",
              },
              {
                message_id: "m2",
                session_id: "prime-u-root-abc",
                sender: "prime-u-root-abc",
                recipient: "orchestrator",
                content: "all done, task completed successfully",
                timestamp: "2026-01-01T00:00:02.000Z",
                kind: "communication",
              },
            ],
          },
        ],
      };
      const content = renderChatView({ snapshot, observation: renderOptions(model).observation });
      const text = content.lines.flatMap((line) => line.spans.map((span) => span.text)).join(" ");
      expect(text).toContain("prime-u-root-abc");
      expect(text).toContain("unit u-root");
      expect(text).toContain("RUNNING");
      expect(text).toContain("goal: do the thing");
      expect(text).not.toContain("should not render verbatim");
      expect(text).toContain("Prime-mailbox communication, not completion");
      // Status text is only ever RUNNING (the handle status), never derived
      // from the "completed successfully" mailbox text (Attack 1).
      expect(text).not.toMatch(/\bCOMPLETED\b/);
    } finally {
      model.close();
    }
  });
});

test("renderer: a chat session row is a clickable Primary region", async () => {
  const terminal = await createTestRenderer({ width: 120, height: 40 });
  const view = new TerminalShellRenderer(terminal.renderer);
  const model = seededModel();
  const session = modelWiredSession(model, FULL_DIMS);
  try {
    const shell = session.render(model.getSnapshot(), renderOptions(model));
    const withChatRow: RenderedShell = {
      ...shell,
      surfaces: shell.surfaces.map((surface) =>
        surface.id === "Primary"
          ? {
              ...surface,
              lines: [
                ...surface.lines,
                {
                  id: "chat.session.prime-u-root-abc",
                  spans: [{ text: "RUNNING prime-u-root-abc unit u-root" }],
                } satisfies RenderedLine,
              ],
            }
          : surface,
      ),
    };
    view.render(withChatRow);
    await terminal.renderOnce();
    const primary = view.hitMap().regions.find((region) => region.surfaceId === "Primary");
    expect(primary?.rows).toContain("chat.session.prime-u-root-abc");
  } finally {
    view.destroy();
    terminal.renderer.destroy();
    model.close();
  }
});
