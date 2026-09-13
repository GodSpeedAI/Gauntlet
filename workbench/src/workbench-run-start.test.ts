/**
 * Tests for plan `workbench-agent-interaction.md` increment R: binding ICM
 * §13.1 `run.start` over the canonical `gauntlet run` CLI, spawned detached
 * through the T13 boundary (`validateSpawnRequest`) in `config.project_root`.
 *
 * A FAKE `gauntlet` binary (a recording shell script, the same pattern
 * `workbench-agent-interaction-p1.test.ts` uses for `RealPrimeHost`) stands
 * in for the real CLI: `--dry-run` refusal/clean and a real run's
 * `run: run_id=<id>` first line are both scripted, deterministic, and
 * argv-inspectable — no real `gauntlet` binary is ever invoked here.
 */

import { describe, expect, it } from "bun:test";
import { chmodSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import {
  COMPOSER_MODES,
  type ComposerMode,
  createComposerState,
  cycleComposerMode,
} from "./composer";
import { resolveConfig } from "./config";
import { createShellState, handleCommand, menuBarEntries } from "./shell";
import { FULL_DIMS, seededModel } from "./testing/shell-harness";
import { WorkbenchHost } from "./workbench-host";

/** A tiny mock canonical surface (D6-style): run.start never touches it, but WorkbenchHost's constructor/read loop needs one. */
function serveCanonicalSurface() {
  const server = Bun.serve({
    port: 0,
    hostname: "127.0.0.1",
    fetch(request) {
      const url = new URL(request.url);
      if (url.pathname === "/api/v1/state") {
        return Response.json({ run: null, units: [], escalated: [], budget: { tokens: 99 } });
      }
      if (url.pathname === "/api/v1/capabilities") {
        return Response.json({
          protocol_version: "1.0.0",
          server_version: "test",
          capabilities: [],
          unsupported_operations: [],
        });
      }
      return Response.json({ ok: true });
    },
  });
  return server;
}

function hostWith(options: { gauntlet_binary: string | null; project_root: string | null }) {
  const server = serveCanonicalSurface();
  const host = new WorkbenchHost(
    resolveConfig({
      env: {},
      cli: {
        gauntlet_endpoint: server.url.toString(),
        projection_store: ":memory:",
        session_token: "test-only",
        gauntlet_binary: options.gauntlet_binary,
        project_root: options.project_root,
      },
    }),
    { actor: "operator" },
  );
  return { host, server };
}

/** Fake `gauntlet` CLI: records argv+cwd, then behaves per `mode`. */
function writeFakeGauntlet(
  binPath: string,
  capturePath: string,
  mode: "dry-run-refuse" | "dry-run-clean" | "run-ok" | "run-crash-before-id",
): void {
  mkdirSync(dirname(binPath), { recursive: true });
  let body: string;
  switch (mode) {
    case "dry-run-refuse":
      body = [
        "#!/bin/sh",
        `echo "$@" > "${capturePath}"`,
        'echo "gauntlet run: configuration failure: control.stall_timeout_ms (GAUNTLET_STALL_TIMEOUT_MS): value is REQUIRED" 1>&2',
        "exit 1",
      ].join("\n");
      break;
    case "dry-run-clean":
      body = [
        "#!/bin/sh",
        `echo "$@" >> "${capturePath}"`,
        'if echo "$@" | grep -q -- "--dry-run"; then',
        '  echo "run: dry-run — preflight clean, zero writes ahead"',
        '  echo "run: dry-run complete — no freeze, no state rows, no evidence pins, no dispatch"',
        "  exit 0",
        "fi",
        'echo "run: run_id=run-fake-001"',
        'echo "run: settled"',
        "exit 0",
      ].join("\n");
      break;
    case "run-ok":
      // Only reached post-dry-run in these tests; same script as dry-run-clean.
      body = "";
      break;
    case "run-crash-before-id":
      body = [
        "#!/bin/sh",
        `echo "$@" >> "${capturePath}"`,
        'if echo "$@" | grep -q -- "--dry-run"; then',
        '  echo "run: dry-run — preflight clean, zero writes ahead"',
        "  exit 0",
        "fi",
        'echo "gauntlet run: fatal: something went wrong before admission" 1>&2',
        "exit 1",
      ].join("\n");
      break;
  }
  writeFileSync(binPath, `${body}\n`, { mode: 0o755 });
  chmodSync(binPath, 0o755);
}

async function waitForNotice(
  host: WorkbenchHost,
  predicate: (notice: string | null) => boolean,
  timeoutMs = 10_000,
): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (predicate(host.notice)) return;
    await new Promise((resolve) => setTimeout(resolve, 25));
  }
  throw new Error(`notice never satisfied predicate; last notice: ${host.notice}`);
}

describe("workbench-agent-interaction R: run.start availability", () => {
  it("gauntlet_binary/project_root unset → unavailable with an exact reason, zero spawn", async () => {
    const { host, server } = hostWith({ gauntlet_binary: null, project_root: null });
    try {
      await host.submit("run", "workbench", "do the thing");
      expect(host.notice).toContain("run.start");
      expect(host.notice).toContain("not configured");
      expect(host.notice).toContain("GAUNTLET_WORKBENCH_GAUNTLET_BINARY");
    } finally {
      host.close();
      await server.stop(true);
    }
  });

  it("empty intent → refused, message not sent", async () => {
    const root = mkdtempSync(join(tmpdir(), "run-start-empty-"));
    try {
      const bin = join(root, "bin", "gauntlet");
      writeFakeGauntlet(bin, join(root, "capture.txt"), "dry-run-clean");
      const { host, server } = hostWith({ gauntlet_binary: bin, project_root: root });
      try {
        await host.submit("run", "workbench", "   ");
        expect(host.notice).toContain("run.start");
        expect(host.notice).toContain("intent is required");
        expect(host.notice).toContain("not sent");
      } finally {
        host.close();
        await server.stop(true);
      }
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });
});

describe("workbench-agent-interaction R: dry-run preflight", () => {
  it("dry-run refuses → the exact refusal text surfaces as the notice, and NOTHING further spawns", async () => {
    const root = mkdtempSync(join(tmpdir(), "run-start-refuse-"));
    try {
      const capture = join(root, "capture.txt");
      const bin = join(root, "bin", "gauntlet");
      writeFakeGauntlet(bin, capture, "dry-run-refuse");
      const { host, server } = hostWith({ gauntlet_binary: bin, project_root: root });
      try {
        await host.submit("run", "workbench", "ship the feature");
        expect(host.notice).toContain("run.start: refused");
        expect(host.notice).toContain("GAUNTLET_STALL_TIMEOUT_MS");
        expect(host.notice).toContain("REQUIRED");
        // Only one invocation ever happened (the dry-run itself) — the
        // capture file was written exactly once, never appended a second
        // (real-run) argv line.
        const captured = readFileSync(capture, "utf8").trim().split("\n");
        expect(captured).toHaveLength(1);
        expect(captured[0]).toContain("--dry-run");
        // No workbench-runs log directory was ever created (zero writes).
        expect(() => readFileSync(join(root, ".gauntlet", "workbench-runs"))).toThrow();
      } finally {
        host.close();
        await server.stop(true);
      }
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });

  it("dry-run clean → spawns the real run with the right argv/cwd", async () => {
    const root = mkdtempSync(join(tmpdir(), "run-start-clean-"));
    try {
      const capture = join(root, "capture.txt");
      const bin = join(root, "bin", "gauntlet");
      writeFakeGauntlet(bin, capture, "dry-run-clean");
      const { host, server } = hostWith({ gauntlet_binary: bin, project_root: root });
      try {
        await host.submit("run", "workbench", "ship the feature");
        await waitForNotice(host, (notice) => notice?.includes("run-fake-001") === true);
        expect(host.notice).toContain("run run-fake-001 started");
        expect(host.notice).not.toContain("complete");
        expect(host.notice).not.toContain("completed");

        const lines = readFileSync(capture, "utf8").trim().split("\n");
        expect(lines).toHaveLength(2);
        expect(lines[0]).toContain("--dry-run");
        expect(lines[0]).toContain("ship the feature");
        expect(lines[1]).not.toContain("--dry-run");
        expect(lines[1]).toContain("run");
        expect(lines[1]).toContain("ship the feature");
      } finally {
        host.close();
        await server.stop(true);
      }
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });

  it("process exits before printing a run id → honest stderr text, never a fabricated id", async () => {
    const root = mkdtempSync(join(tmpdir(), "run-start-crash-"));
    try {
      const capture = join(root, "capture.txt");
      const bin = join(root, "bin", "gauntlet");
      writeFakeGauntlet(bin, capture, "run-crash-before-id");
      const { host, server } = hostWith({ gauntlet_binary: bin, project_root: root });
      try {
        await host.submit("run", "workbench", "ship the feature");
        await waitForNotice(
          host,
          (notice) => notice?.includes("exited before printing a run id") === true,
        );
        expect(host.notice).toContain("something went wrong before admission");
        expect(host.notice).not.toContain("run_id");
      } finally {
        host.close();
        await server.stop(true);
      }
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });
});

describe("workbench-agent-interaction R: T13 boundary", () => {
  it("a relative gauntlet_binary path is refused at the T13 boundary before any spawn", async () => {
    const root = mkdtempSync(join(tmpdir(), "run-start-boundary-"));
    try {
      const { host, server } = hostWith({ gauntlet_binary: "gauntlet", project_root: root });
      try {
        await host.submit("run", "workbench", "ship the feature");
        expect(host.notice).toContain("run.start: refused at the T13 spawn boundary");
      } finally {
        host.close();
        await server.stop(true);
      }
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });
});

describe("workbench-agent-interaction R: composer run mode", () => {
  it("`run` joined COMPOSER_MODES and the ctrl+t cycle", () => {
    expect(COMPOSER_MODES).toContain("run" as ComposerMode);
    let state = createComposerState();
    const seen: string[] = [state.mode];
    for (let i = 0; i < COMPOSER_MODES.length; i++) {
      state = cycleComposerMode(state);
      seen.push(state.mode);
    }
    expect(seen).toContain("run");
    // A full cycle returns to the start.
    expect(seen[seen.length - 1]).toBe(seen[0]);
  });
});

describe("workbench-agent-interaction R: palette/MenuBar converge on run.start", () => {
  it("the MenuBar File entry resolves to the run.start command id, same as the shell command", () => {
    const state = createShellState(
      { workspace: { workspace: "gauntlet", reference: "r", run: "R1" } },
      FULL_DIMS,
    );
    const snapshot = seededModel().getSnapshot();
    const entries = menuBarEntries("file", state, snapshot);
    const runRow = entries.find((entry) => entry.commandId === "run.start");
    expect(runRow).toBeDefined();

    // Activating the SAME command id through handleCommand focuses the
    // composer and emits the compose_run effect — the one operation
    // identity every source (palette, MenuBar, shell key) converges on
    // (ICM §13.1/§35).
    const result = handleCommand(state, snapshot, "run.start", {});
    expect(result.state.focusedSurface).toBe("Composer");
    expect(result.effects).toContainEqual({ type: "compose_run" });
  });
});
