/**
 * T13 embedded-terminal suite (plan T13, REQ-BUN-004, REQ-UX-005/006, DSN-014).
 *
 * Deterministic by construction: every lifecycle fact is driven through the
 * FakeProcessProvider (the TerminalProcessProvider port), so no real process
 * is needed here — the REAL Bun PTY path is qualified by explicitly-gated
 * probes at the bottom of this file and recorded in the T13 evidence.
 *
 * Declared teeth (plan T13):
 *   - Attack 2: resize/kill/restart churn changes ONLY the terminal-session
 *     projection; canonical snapshots stay byte-identical.
 *   - exit/cancel provenance is retained after the child is gone.
 */

import { describe, expect, it } from "bun:test";
import { mkdirSync } from "node:fs";
import { availableViews } from "./shell";
import {
  bunPtyProcessProvider,
  createChunkSanitizer,
  FakeProcessProvider,
  publishTerminalProvider,
  sanitizeTerminalOutput,
  TerminalBoundaryError,
  TerminalSession,
} from "./terminal-session";
import { seededModel } from "./testing/shell-harness";
import type { FullWorkbenchSnapshot } from "./workbench-model";

const HOST = {
  pathPolicy: {
    allowedProgramRoots: ["/bin", "/usr/bin"],
    allowedPathRoots: ["/tmp/gauntlet-t13"],
  },
  dimensions: { cols: 80, rows: 24 },
  maxScrollback: 1000,
} as const;

const CLOCK = { tick: 1000 };
function tickClock(): number {
  CLOCK.tick += 10;
  return CLOCK.tick;
}

function freshProvider(): FakeProcessProvider {
  return new FakeProcessProvider();
}

/** Poll until the predicate holds (providers deliver exit asynchronously). */
async function until(predicate: () => boolean, timeoutMs = 5000): Promise<void> {
  const deadline = performance.now() + timeoutMs;
  while (!predicate()) {
    if (performance.now() > deadline) {
      throw new Error("until(): predicate never became true");
    }
    await Bun.sleep(2);
  }
}

describe("T13 terminal session lifecycle (REQ-BUN-004, plan T13 step 2)", () => {
  it("binds a validated spawn and tracks running state with provenance", () => {
    const provider = freshProvider();
    const session = new TerminalSession(
      {
        program: "/bin/sh",
        args: ["-c", "echo hello"],
        cwd: "/tmp/gauntlet-t13",
        env: { T13_LIFECYCLE: "1" },
        title: "lifecycle probe",
      },
      provider,
      { ...HOST, nowMs: tickClock },
    );

    expect(session.isRunning()).toBe(true);
    expect(provider.spawnCount).toBe(1);

    const provenance = session.provenance;
    expect(provenance.argv).toEqual(["/bin/sh", "-c", "echo hello"]);
    expect(provenance.cwd).toBe("/tmp/gauntlet-t13");
    expect(provenance.envKeys).toEqual(["T13_LIFECYCLE"]);
    expect(provenance.pid).toBe(42000);
    expect(provenance.providerKind).toBe("fake");
    expect(provenance.cancelled).toBe(false);
    expect(provenance.exitedAtMs).toBeNull();

    const projected = session.project();
    expect(projected.status).toBe("running");
    expect(projected.title).toBe("lifecycle probe");
    expect(projected.provenance.argv).toEqual(["/bin/sh", "-c", "echo hello"]);
  });

  it("forwards input to the PTY, records only counts, and resize reaches the provider", () => {
    const provider = freshProvider();
    const session = new TerminalSession(
      { program: "/bin/sh", cwd: "/tmp/gauntlet-t13" },
      provider,
      { ...HOST, nowMs: tickClock },
    );

    session.write("echo secret-input\n");
    session.write(new TextEncoder().encode("raw-bytes"));
    expect(provider.writes).toEqual(["echo secret-input\n", "raw-bytes"]);

    session.resize(100, 30);
    session.resize(120, 40);
    expect(provider.resizes).toEqual([
      { cols: 100, rows: 30 },
      { cols: 120, rows: 40 },
    ]);
    expect(session.project().cols).toBe(120);
    expect(session.project().rows).toBe(40);

    // Input CONTENT is never recorded — only byte counts (secret discipline).
    const eventJson = JSON.stringify(session.events);
    expect(eventJson).not.toContain("secret-input");
    expect(eventJson).toContain('"kind":"input"');
    const inputEvents = session.events.filter((event) => event.kind === "input");
    expect(inputEvents).toHaveLength(2);
    expect([...inputEvents]).toEqual([
      { atMs: expect.any(Number), kind: "input", detail: { bytes: 18 } },
      { atMs: expect.any(Number), kind: "input", detail: { bytes: 9 } },
    ]);
  });

  it("records exit provenance: code, timestamp, and full ordered lifecycle", () => {
    const provider = freshProvider();
    const session = new TerminalSession(
      { program: "/bin/sh", cwd: "/tmp/gauntlet-t13" },
      provider,
      { ...HOST, nowMs: tickClock },
    );
    provider.runScript([{ emit: "bye\n" }, { exit: { exitCode: 0, signal: null } }]);

    expect(session.isRunning()).toBe(false);
    expect(session.status).toBe("exited");
    expect(session.provenance.exitCode).toBe(0);
    expect(session.provenance.signal).toBeNull();
    expect(session.provenance.exitedAtMs).not.toBeNull();
    expect(session.project().scrollback).toEqual(["bye", ""]);

    const kinds = session.events.map((event) => event.kind);
    expect(kinds).toEqual(["spawn_requested", "spawn_bound", "spawned", "output", "exit"]);
  });

  it("keeps cancellation DISTINCT from exit and retains the reason (teeth)", () => {
    const provider = freshProvider();
    const session = new TerminalSession(
      { program: "/bin/sh", cwd: "/tmp/gauntlet-t13" },
      provider,
      { ...HOST, nowMs: tickClock },
    );
    session.cancel("churn attack: replacing child");
    expect(provider.kills).toEqual(["SIGTERM"]);
    provider.runScript([{ exit: { exitCode: null, signal: "SIGTERM" } }]);

    expect(session.status).toBe("cancelled");
    expect(session.status).not.toBe("exited");
    expect(session.provenance.cancelled).toBe(true);
    expect(session.provenance.cancelReason).toBe("churn attack: replacing child");
    expect(session.provenance.exitCode).toBeNull();
    expect(session.provenance.signal).toBe("SIGTERM");
    // Provenance survives the terminal outcome.
    expect(session.provenance.argv).toEqual(["/bin/sh"]);
    expect(session.cancelReason).toBe("churn attack: replacing child");
  });

  it("maps a signal death without cancel to failed (third distinct outcome)", () => {
    const provider = freshProvider();
    const session = new TerminalSession(
      { program: "/bin/sh", cwd: "/tmp/gauntlet-t13" },
      provider,
      { ...HOST, nowMs: tickClock },
    );
    provider.runScript([{ exit: { exitCode: null, signal: "SIGSEGV" } }]);
    expect(session.status).toBe("failed");
    expect(session.provenance.cancelled).toBe(false);
    expect(session.provenance.signal).toBe("SIGSEGV");
  });

  it("records a synchronous provider refusal as a failed session with no writes", () => {
    const provider = FakeProcessProvider.refusing(new Error("ENOTSUP: no PTY on this platform"));
    const session = new TerminalSession(
      { program: "/bin/sh", cwd: "/tmp/gauntlet-t13" },
      provider,
      { ...HOST, nowMs: tickClock },
    );
    expect(session.status).toBe("failed");
    expect(session.events.some((event) => event.kind === "spawn_refused")).toBe(true);
    expect(session.events.some((event) => event.kind === "spawned")).toBe(false);
  });

  it("refuses input, resize, and cancel after exit (typed, state-locus)", () => {
    const provider = freshProvider();
    const session = new TerminalSession(
      { program: "/bin/sh", cwd: "/tmp/gauntlet-t13" },
      provider,
      { ...HOST, nowMs: tickClock },
    );
    provider.runScript([{ exit: { exitCode: 0, signal: null } }]);

    expect(() => session.write("late")).toThrow(TerminalBoundaryError);
    expect(() => session.resize(80, 24)).toThrow(TerminalBoundaryError);
    // cancel is idempotent after exit — no throw, no extra kill.
    session.cancel("late cancel");
    expect(provider.kills).toEqual([]);
  });

  it("caps scrollback at maxScrollback, dropping oldest lines", () => {
    const provider = freshProvider();
    const session = new TerminalSession(
      { program: "/bin/sh", cwd: "/tmp/gauntlet-t13" },
      provider,
      { ...HOST, maxScrollback: 5, nowMs: tickClock },
    );
    const lines = Array.from({ length: 20 }, (_, i) => `line-${i}`).join("\n");
    provider.runScript([{ emit: `${lines}\n` }]);
    const scrollback = session.project().scrollback;
    expect(scrollback).toHaveLength(5); // line-16 .. line-19 plus the trailing split
    expect(scrollback[0]).toBe("line-16");
    expect(scrollback[3]).toBe("line-19");
    expect(scrollback[4]).toBe("");
  });

  it("mints unique session identities across restart churn", () => {
    const providerA = freshProvider();
    const sessionA = new TerminalSession(
      { program: "/bin/sh", cwd: "/tmp/gauntlet-t13" },
      providerA,
      { ...HOST, nowMs: tickClock },
    );
    const providerB = freshProvider();
    const sessionB = new TerminalSession(
      { program: "/bin/sh", cwd: "/tmp/gauntlet-t13" },
      providerB,
      { ...HOST, nowMs: tickClock },
    );
    expect(sessionA.sessionId).not.toBe(sessionB.sessionId);
  });
});

describe("T13 Attack 2: terminal churn changes ONLY the terminal-session projection", () => {
  it("canonical snapshots stay byte-identical across input/resize/output/exit/restart churn", () => {
    const model = seededModel();
    const before: FullWorkbenchSnapshot = model.getSnapshot();
    const beforeJson = JSON.stringify(before);

    // --- terminal churn: full lifecycle including a kill/restart cycle ---
    const provider = freshProvider();
    const session = new TerminalSession(
      { program: "/bin/sh", args: ["-i"], cwd: "/tmp/gauntlet-t13", title: "churn" },
      provider,
      { ...HOST, nowMs: tickClock },
    );
    const projectionBefore = session.project();

    session.write("ls\n");
    provider.runScript([
      {
        emit: "\x1b[1;32mfile.txt\x1b[0m\x1b]52;c;aGVsbG8=\x07\x1b]8;;http://evil.example\x07link\x1b]8;;\x07\r\n",
      },
    ]);
    session.resize(132, 43);
    session.resize(80, 24);
    session.cancel("attack-2 churn");
    provider.runScript([{ exit: { exitCode: null, signal: "SIGTERM" } }]);

    // Restart: a second session on a fresh provider, exit it cleanly.
    const provider2 = freshProvider();
    const session2 = new TerminalSession(
      { program: "/bin/sh", cwd: "/tmp/gauntlet-t13" },
      provider2,
      { ...HOST, nowMs: tickClock },
    );
    provider2.runScript([{ emit: "ready\r\n" }, { exit: { exitCode: 0, signal: null } }]);

    // --- the terminal-session projection DID change ---
    const projectionAfter = session.project();
    expect(projectionAfter.generation).toBeGreaterThan(projectionBefore.generation);
    expect(projectionAfter.status).toBe("cancelled");
    expect(projectionAfter.scrollback).not.toEqual(projectionBefore.scrollback);
    expect(session2.status).toBe("exited");

    // --- the canonical snapshot did NOT ---
    const after: FullWorkbenchSnapshot = model.getSnapshot();
    expect(JSON.stringify(after)).toBe(beforeJson);
  });

  it("publishes the terminal provider without mutating canonical fields", () => {
    const model = seededModel();
    const snapshot = model.getSnapshot();
    const provider = freshProvider();
    const session = new TerminalSession(
      { program: "/bin/sh", cwd: "/tmp/gauntlet-t13" },
      provider,
      { ...HOST, nowMs: tickClock },
    );

    // Without publication the terminal lower surface is ABSENT with its
    // inspectable reason (shell.ts provider gating — consumed read-only).
    const absent = availableViews(snapshot).find((entry) => entry.view === "terminal");
    expect(absent?.status).toBe("absent");
    expect(absent?.reason).toContain("terminal");

    const published = publishTerminalProvider(snapshot, session.project());
    const available = availableViews(published).find((entry) => entry.view === "terminal");
    expect(available?.status).toBe("available");

    // The published object keeps every canonical field intact.
    expect(published.units).toEqual(snapshot.units);
    expect(published.generation).toBe(snapshot.generation);
    expect(JSON.stringify({ ...published, terminal: null })).toBe(
      JSON.stringify({ ...snapshot, terminal: null }),
    );
  });
});

describe("T13 DSN-014: untrusted PTY output is inert text outside the viewport", () => {
  it("neutralizes OSC 52 clipboard, OSC 8 hyperlinks, CSI, and C1 in the projection", () => {
    const provider = freshProvider();
    const session = new TerminalSession(
      { program: "/bin/curl", cwd: "/tmp/gauntlet-t13" },
      provider,
      { ...HOST, nowMs: tickClock },
    );
    provider.runScript([
      {
        emit:
          "\x1b]52;c;Q0xJUEJPQVJE\x07clip\r\n" +
          "\x1b]8;;http://evil.example\x07click me\x1b]8;;\x07\r\n" +
          "\x1b[31mred\x1b[0m plain \x1bP+q544e\x1b\\ tail",
      },
    ]);

    const projected = session.project();
    const joined = projected.scrollback.join("\n");
    expect(joined).not.toContain("\x1b");
    expect(joined).not.toContain("Q0xJUEJPQVJE");
    expect(joined).not.toContain("http://evil.example");
    expect(joined).toContain("clip");
    expect(joined).toContain("click me");
    expect(joined).toContain("red");
    expect(joined).toContain("plain");
    expect(joined).toContain("tail");

    expect(projected.neutralized.oscClipboard).toBe(1);
    expect(projected.neutralized.oscHyperlink).toBe(2); // open + close
    expect(projected.neutralized.csi).toBe(2);
    expect(projected.neutralized.stringSequence).toBe(1);
  });

  it("carries control sequences SPLIT across chunks without leaking a tail", () => {
    const sanitizer = createChunkSanitizer({ mode: "strip" });
    const osc52 = "\x1b]52;c;U1BMaXQ=\x07after";
    const cut = 4; // split inside the OSC payload: "ESC ] 5 2 | ; c ; ..."
    const first = sanitizer.push(osc52.slice(0, cut));
    const second = sanitizer.push(osc52.slice(cut));

    expect(first.text).not.toContain("\x1b");
    expect(second.text).toBe("after");
    expect(second.neutralized.oscClipboard).toBe(1);

    // A trailing bare ESC also carries to the next chunk.
    const a = sanitizer.push("head\x1b");
    const b = sanitizer.push("[31mx\x1b[0m");
    expect(a.text).toBe("head");
    expect(b.text).toBe("x");
    expect(b.neutralized.csi).toBe(2);

    expect(sanitizer.flush().text).toBe("");
  });

  it("caret mode renders sequences as visible TEXT without control functions", () => {
    const sanitized = sanitizeTerminalOutput("\x1b]52;c;QQ==\x07title\x1b[2Jclear", {
      mode: "caret",
    });
    expect(sanitized.text).toContain("^[");
    expect(sanitized.text).toContain("52;c;QQ==");
    expect(sanitized.text).toContain("title");
    expect(sanitized.text).toContain("clear");
    expect(sanitized.text).not.toContain("\x1b");
  });
});

// --- Real Bun PTY qualification (REQ-BUN-004; T03 admission-ledger discipline) ----
// Gated behind GAUNTLET_T13_REAL_PTY=1 so the default suite stays deterministic;
// the T13 evidence records the verbatim run WITH the gate set.

const REAL_PTY = process.env["GAUNTLET_T13_REAL_PTY"] === "1";

describe("T13 real Bun PTY qualification (gated: GAUNTLET_T13_REAL_PTY=1)", () => {
  it.skipIf(!REAL_PTY)(
    "spawns, writes, resizes, and cancels a REAL child through Bun's PTY facility",
    async () => {
      // A real spawn needs a real cwd.
      mkdirSync("/tmp/gauntlet-t13", { recursive: true });
      const session = new TerminalSession(
        {
          program: "/bin/sh",
          args: ["-c", "echo pty-marker-$((40+2)); sleep 30"],
          cwd: "/tmp/gauntlet-t13",
        },
        bunPtyProcessProvider({ cols: 80, rows: 24 }),
        HOST,
      );
      expect(session.isRunning()).toBe(true);
      expect(session.provenance.providerKind).toBe("bun-pty");
      expect(typeof session.provenance.pid).toBe("number");

      session.write("echo typed-in\n");
      session.resize(100, 30);
      await until(() => session.project().scrollback.join("\n").includes("pty-marker-42"), 10000);
      await until(() => session.project().cols === 100, 5000);

      session.cancel("probe complete");
      await until(() => session.status === "cancelled", 5000);
      expect(session.provenance.cancelled).toBe(true);
      expect(session.provenance.argv).toEqual([
        "/bin/sh",
        "-c",
        "echo pty-marker-$((40+2)); sleep 30",
      ]);
      // DSN-014: even REAL PTY output is stored inert outside the viewport.
      expect(session.project().scrollback.join("\n")).not.toContain("\x1b");
    },
    20000,
  );
});
