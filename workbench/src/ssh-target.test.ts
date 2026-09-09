import { afterAll, describe, expect, it } from "bun:test";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  InMemoryStreamPort,
  InMemoryStreamSource,
  LiveStreamClient,
  projectionDifferences,
} from "./live-streams";
import { PrimeHostAdapter } from "./prime-host";
import { loggingPrimeAdapter } from "./prime-long-running";
import { findSessionInTopology } from "./prime-topology";
import { WorkbenchModel } from "./workbench-model";

/**
 * T20 SSH/tmux terminal qualification (plan T20 steps 1–2, REQ-CONFIG-005,
 * REQ-UX-005 remote clause, REQ-OUT-003; prereg A1/D2/P2).
 *
 * THE DEPLOYMENT SHAPE (prereg D1): SSH/tmux operates the SAME host — the
 * workbench entry runs on this machine against the same loopback surface,
 * driven from a terminal session that may itself be remote. No remote HTTP
 * server and no new Unix socket transport is invented. What is qualified:
 *
 *   1. SSH CLIENT LEG — a real `ssh localhost` session driving the workbench
 *      entry under tmux. GATED ON REALITY: when this host has no reachable
 *      sshd, the leg is skipped VISIBLY and the skip names exactly what could
 *      not be exercised (never faked). The probe result is recorded by an
 *      always-running test below.
 *   2. TMUX LEG (no SSH required) — the workbench entry under a real tmux
 *      session on this host: the shell renders, resize propagates to the
 *      process (SIGWINCH → re-layout), the compact and below-minimum states
 *      behave, and quitting exits cleanly.
 *   3. CLIENT-DEATH/REATTACH (tmux session groups) — a second client session
 *      sharing the window group is KILLED while the workbench process keeps
 *      running; a fresh client attaches and observes the same live pane state
 *      from the tmux server (authoritative), with the same workbench PID.
 *   4. TERMINAL RESTART MID-RUN (T16 shapes, in-process) — the TUI client
 *      dies while long-running Prime work continues; the fresh client
 *      reattaches from authoritative state with zero cancels and no local-UI
 *      memory (prereg D2).
 */

const WORKBENCH_DIR = join(import.meta.dir, ".."); // the workbench package root
const RUN_ID = `t20-ssh-${process.pid}`;

// --- Availability probes (real, recorded) ------------------------------------

interface ProbeResult {
  readonly available: boolean;
  readonly detail: string;
}

function probeSshLoopback(): ProbeResult {
  try {
    const proc = Bun.spawnSync(
      [
        "ssh",
        "-o",
        "BatchMode=yes",
        "-o",
        "ConnectTimeout=3",
        "-o",
        "StrictHostKeyChecking=no",
        "-o",
        "UserKnownHostsFile=/dev/null",
        "localhost",
        "echo SSH_TARGET_OK",
      ],
      { stdout: "pipe", stderr: "pipe", timeout: 10_000 },
    );
    const stdout = new TextDecoder().decode(proc.stdout ?? new Uint8Array());
    if (proc.exitCode === 0 && stdout.includes("SSH_TARGET_OK")) {
      return { available: true, detail: "ssh localhost succeeded (BatchMode)" };
    }
    const stderr = new TextDecoder().decode(proc.stderr ?? new Uint8Array());
    const reason =
      stderr.trim().split("\n").slice(-2).join("; ") || `exit ${String(proc.exitCode)}`;
    return {
      available: false,
      detail: `ssh localhost failed (exit ${String(proc.exitCode)}): ${reason}`,
    };
  } catch (error) {
    return { available: false, detail: `ssh probe could not run: ${String(error)}` };
  }
}

function probeTmux(): ProbeResult {
  const proc = Bun.spawnSync(["tmux", "-V"], { stdout: "pipe", stderr: "pipe" });
  if (proc.exitCode !== 0) {
    return { available: false, detail: "tmux not runnable on this host" };
  }
  return { available: true, detail: new TextDecoder().decode(proc.stdout).trim() };
}

const SSH = probeSshLoopback();
const TMUX = probeTmux();

// --- tmux orchestration helpers ----------------------------------------------

function tmux(args: string[]): { exitCode: number | null; stdout: string; stderr: string } {
  const proc = Bun.spawnSync(["tmux", ...args], {
    stdout: "pipe",
    stderr: "pipe",
    timeout: 15_000,
  });
  return {
    exitCode: proc.exitCode,
    stdout: new TextDecoder().decode(proc.stdout ?? new Uint8Array()),
    stderr: new TextDecoder().decode(proc.stderr ?? new Uint8Array()),
  };
}

function tmuxOrFail(args: string[], what: string): string {
  const result = tmux(args);
  if (result.exitCode !== 0) {
    throw new Error(`tmux ${args[0]} failed (${what}): ${result.stderr.trim()}`);
  }
  return result.stdout;
}

interface TmuxWorkbench {
  readonly session: string;
  readonly exitFile: string;
  readonly workDir: string;
}

function spawnWorkbenchInTmux(cols: number, rows: number, entryCommand: string): TmuxWorkbench {
  const workDir = mkdtempSync(join(tmpdir(), "gauntlet-t20-tmux-"));
  const exitFile = join(workDir, "exit-code");
  const session = `${RUN_ID}-${Math.random().toString(36).slice(2, 8)}`;
  const paneCommand = `${entryCommand}; printf '\\n__T20_EXIT_%s__' "$?" > ${exitFile}`;
  tmuxOrFail(
    [
      "-f",
      "/dev/null",
      "new-session",
      "-d",
      "-x",
      String(cols),
      "-y",
      String(rows),
      "-s",
      session,
      "-c",
      WORKBENCH_DIR,
      paneCommand,
    ],
    "spawn workbench",
  );
  return { session, exitFile, workDir };
}

function capturePane(session: string): string {
  return tmuxOrFail(["capture-pane", "-t", session, "-p", "-S", "-60"], "capture pane");
}

function panePid(session: string): string {
  return tmuxOrFail(["list-panes", "-t", session, "-F", "#{pane_pid}"], "pane pid").trim();
}

async function untilPane(session: string, marker: string, timeoutMs: number): Promise<string> {
  const deadline = Date.now() + timeoutMs;
  let last = "";
  while (Date.now() < deadline) {
    last = capturePane(session);
    if (last.includes(marker)) {
      return last;
    }
    await Bun.sleep(150);
  }
  throw new Error(
    `tmux pane never showed '${marker}' within ${String(timeoutMs)}ms; last capture:\n${last.slice(0, 2000)}`,
  );
}

async function waitForExitFile(exitFile: string, timeoutMs: number): Promise<string> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const file = Bun.file(exitFile);
    if (await file.exists()) {
      return await file.text();
    }
    await Bun.sleep(150);
  }
  throw new Error(`workbench exit marker never appeared in ${exitFile}`);
}

function killSession(session: string): void {
  tmux(["kill-session", "-t", session]);
}

const spawnedSessions: string[] = [];
const spawnedWorkDirs: string[] = [];

// --- Suites -------------------------------------------------------------------

describe("T20 target probes (recorded honestly)", () => {
  it("the SSH loopback probe runs and records its real result", () => {
    // The probe itself must always EXECUTE ( BatchMode, bounded time) and
    // report a typed result either way; the gated leg below keys off it.
    expect(typeof SSH.available).toBe("boolean");
    expect(SSH.detail.length).toBeGreaterThan(0);
    if (!SSH.available) {
      // Honest recording: the failure detail names the real cause.
      expect(SSH.detail).toMatch(/ssh localhost failed|could not run/);
    }
  });

  it("the tmux probe runs and records its real result", () => {
    expect(typeof TMUX.available).toBe("boolean");
    if (TMUX.available) {
      expect(TMUX.detail).toContain("tmux");
    }
  });
});

describe("T20 SSH client leg (gated on a REAL reachable sshd)", () => {
  it.skipIf(!SSH.available)(
    `drives the workbench entry through a real 'ssh localhost' session under tmux (${SSH.detail})`,
    async () => {
      // Reachable only when the probe above succeeded. The SAME loopback
      // HTTP surface serves the SSH-session terminal — no remote server is
      // invented (prereg D1); the SSH leg proves the terminal path.
      const knownHosts = join(mkdtempSync(join(tmpdir(), "gauntlet-t20-ssh-")), "known_hosts");
      const sshCommand = `ssh -o StrictHostKeyChecking=no -o UserKnownHostsFile=${knownHosts} -o ConnectTimeout=5 localhost 'cd ${WORKBENCH_DIR} && bun run src/main.ts'`;
      const wb = spawnWorkbenchInTmux(120, 40, sshCommand);
      spawnedSessions.push(wb.session);
      spawnedWorkDirs.push(wb.workDir);
      const rendered = await untilPane(wb.session, "Gauntlet", 30_000);
      expect(rendered).toContain("NAVIGATOR");
      expect(rendered).toContain("INSPECTOR");
      tmuxOrFail(["send-keys", "-t", wb.session, "q"], "quit");
      const exitMarker = await waitForExitFile(wb.exitFile, 15_000);
      expect(exitMarker).toContain("__T20_EXIT_0__");
    },
    60_000,
  );
});

describe("T20 tmux leg: the workbench entry under a real tmux session (this host)", () => {
  it.skipIf(!TMUX.available)(
    `renders, resizes, and quits cleanly under tmux (${TMUX.detail})`,
    async () => {
      const wb = spawnWorkbenchInTmux(120, 40, "bun run src/main.ts");
      spawnedSessions.push(wb.session);
      spawnedWorkDirs.push(wb.workDir);

      // 1. The shell renders at 120x40 with the full three-surface layout.
      const full = await untilPane(wb.session, "NAVIGATOR", 30_000);
      expect(full).toContain("INSPECTOR");
      expect(full).toContain("PRIMARY");
      expect(full).toContain("Gauntlet");

      // 2. Resize propagates: 80x24 → compact (one primary pane, no
      //    Navigator/Inspector surfaces) — SIGWINCH reached the process.
      tmuxOrFail(["resize-window", "-t", wb.session, "-x", "80", "-y", "24"], "resize compact");
      const deadline = Date.now() + 15_000;
      let compact = "";
      while (Date.now() < deadline) {
        compact = capturePane(wb.session);
        if (compact.includes("PRIMARY") && !compact.includes("NAVIGATOR")) {
          break;
        }
        await Bun.sleep(200);
      }
      expect(compact).toContain("PRIMARY");
      expect(compact).not.toContain("NAVIGATOR");

      // 3. Below minimum: the recoverable prompt names the required size.
      tmuxOrFail(["resize-window", "-t", wb.session, "-x", "60", "-y", "18"], "resize tiny");
      const tiny = await untilPane(wb.session, "Gauntlet needs at least 80x24", 15_000);
      expect(tiny).toContain("press q to quit");

      // 4. Quit from the prompt is reachable and clean (exit 0).
      tmuxOrFail(["send-keys", "-t", wb.session, "q"], "quit");
      const exitMarker = await waitForExitFile(wb.exitFile, 15_000);
      expect(exitMarker).toContain("__T20_EXIT_0__");
    },
    90_000,
  );
});

describe("T20 client death and fresh-client reattach through the tmux server", () => {
  it.skipIf(!TMUX.available)(
    "killing the client session leaves the workbench process running; a fresh client sees the same live state",
    async () => {
      const wb = spawnWorkbenchInTmux(120, 40, "bun run src/main.ts");
      spawnedSessions.push(wb.session);
      spawnedWorkDirs.push(wb.workDir);
      await untilPane(wb.session, "NAVIGATOR", 30_000);
      const pidBefore = panePid(wb.session);

      // A second client: a session GROUPED with the first (shares windows).
      const clientSession = `${wb.session}-client1`;
      tmuxOrFail(
        ["new-session", "-d", "-t", wb.session, "-s", clientSession, "-x", "120", "-y", "40"],
        "attach grouped client",
      );
      await untilPane(clientSession, "NAVIGATOR", 15_000);

      // CLIENT DEATH: kill the grouped client session — the workbench pane
      // (and its process) keeps running under the original session.
      killSession(clientSession);
      await Bun.sleep(500);
      expect(tmux(["has-session", "-t", clientSession]).exitCode).not.toBe(0);
      const pidAfter = panePid(wb.session);
      expect(pidAfter).toBe(pidBefore); // same workbench process — work continuity

      // FRESH CLIENT: a new grouped session reattaches and observes the live
      // pane state from the tmux server — authoritative, not client memory.
      const client2 = `${wb.session}-client2`;
      tmuxOrFail(
        ["new-session", "-d", "-t", wb.session, "-s", client2, "-x", "120", "-y", "40"],
        "attach fresh client",
      );
      const freshView = await untilPane(client2, "NAVIGATOR", 15_000);
      expect(freshView).toContain("PRIMARY");
      expect(freshView).toContain("INSPECTOR");

      // Cleanup: quit the workbench.
      tmuxOrFail(["send-keys", "-t", wb.session, "q"], "quit");
      await waitForExitFile(wb.exitFile, 15_000);
    },
    90_000,
  );
});

describe("T20 terminal restart mid-run with long-running Prime work (T16 shapes, prereg D2)", () => {
  it("the client dies while Prime work continues; the fresh client reattaches from authoritative state with zero cancels", async () => {
    const gauntlet = new InMemoryStreamSource("gauntlet");
    const prime = new InMemoryStreamSource("prime");
    gauntlet.append({
      event_type: "unit_updated",
      payload: { unit_id: "AK-401", title: "Unit 401", status: "blocked" },
      timestamp_ms: 1000,
    });
    prime.append({
      event_type: "session_updated",
      payload: { session_id: "prime-root", parent_id: null, status: "running" },
      timestamp_ms: 1200,
    });
    const port = new InMemoryStreamPort([gauntlet, prime]);

    const adapter = new PrimeHostAdapter();
    adapter.spawnChild({
      sessionId: "prime-root",
      parentSessionId: null,
      contractPath: ".agents/reports/gauntlet-tui-prime-workbench/evidence/T16/contracts/root.md",
    });

    const model = new WorkbenchModel();
    const client = new LiveStreamClient({
      model,
      port,
      mintGenerationId: (serial) => `gen-t20-ssh-${serial}`,
    });
    client.connect();

    // The SSH/tmux CLIENT TERMINAL dies: detach persists ONLY noncanonical
    // presentation state, issues no cancels.
    const detach = client.detach({ selected_horizon: "none", last_focus: "AK-401" });
    expect(detach.detached).toBe(true);
    expect(detach.cancel_calls_issued).toBe(0);
    client.close();

    // LONG-RUNNING WORK CONTINUES per policy — Prime-side progress lands in
    // the authoritative stream while no TUI exists.
    gauntlet.append({
      event_type: "unit_updated",
      payload: { unit_id: "AK-402", title: "Unit 402", status: "in_progress" },
      timestamp_ms: 5000,
    });
    prime.append({
      event_type: "session_updated",
      payload: { session_id: "prime-child-1", parent_id: "prime-root", status: "running" },
      timestamp_ms: 5100,
    });
    adapter.spawnChild({
      sessionId: "prime-child-1",
      parentSessionId: "prime-root",
      contractPath: ".agents/reports/gauntlet-tui-prime-workbench/evidence/T16/contracts/child1.md",
    });

    // FRESH CLIENT (the restarted terminal): reattaches from authoritative
    // state — Prime topology from qualified records, streams reconciled since
    // the persisted cursors — under a read-recording adapter wrapper.
    const freshModel = new WorkbenchModel();
    const fresh = new LiveStreamClient({
      model: freshModel,
      port,
      mintGenerationId: (serial) => `gen-t20-ssh2-${serial}`,
    });
    const logged = loggingPrimeAdapter(adapter);
    const reattach = fresh.reattach({
      persisted: detach.persisted,
      prime_adapter: logged.logged,
    });

    expect(reattach.cancel_calls_issued).toBe(0);
    for (const call of logged.calls) {
      expect(call).not.toMatch(/cancel|stop|terminate|abort|interrupt|kill/i);
    }
    const primeReceipt = reattach.prime;
    if (primeReceipt === null) {
      throw new Error("reattach did not project the Prime adapter");
    }
    const child = findSessionInTopology(primeReceipt.topology, "prime-child-1");
    expect(child?.parentId).toBe("prime-root");
    expect(primeReceipt.running_sessions).toContain("prime-child-1");

    // Reconciliation covered the events since the last authoritative cursor.
    expect(reattach.handshake.kind).toBe("reconnect");
    const gauntletLeg = reattach.handshake.per_source.find((leg) => leg.source_id === "gauntlet");
    expect(gauntletLeg?.cursor_validation).toBe("valid");
    expect(gauntletLeg?.reconciled_since_cursor).toBe(1);

    // The fresh client's projection equals the authoritative reference fold —
    // never local UI memory.
    const reference = new WorkbenchModel();
    reference.startGeneration("gen-t20-ref");
    for (const event of port
      .describeSources()
      .flatMap((descriptor) => port.baselineWindow(descriptor.source_id).events)) {
      reference.applyEvent({
        generation_id: "gen-t20-ref",
        source_id: event.source_id,
        cursor: event.cursor,
        event_type: event.event_type,
        payload: event.payload,
        timestamp_ms: event.timestamp_ms,
      });
    }
    reference.markReady("gen-t20-ref");
    expect(projectionDifferences(freshModel.getSnapshot(), reference.getSnapshot())).toEqual([]);

    client.close();
    fresh.close();
    model.close();
    freshModel.close();
    reference.close();
  });
});

afterAll(() => {
  for (const session of spawnedSessions) {
    killSession(session);
  }
  for (const dir of spawnedWorkDirs) {
    rmSync(dir, { recursive: true, force: true });
  }
});
