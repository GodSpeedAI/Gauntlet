import { afterEach, describe, expect, it } from "bun:test";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { buildRealPrimeEnv, RealPrimeHost, RealPrimeHostSpawnError } from "./real-prime-host";
import { TerminalBoundaryError } from "./terminal-session";

/**
 * Non-gated `RealPrimeHost` teeth (plan Phase C2, workbench-intent-closure.md
 * §4). These never spawn the real `prime-agent` binary — the gated
 * `prime-fresh-target-real.test.ts` battery (GAUNTLET_C2_REAL_PRIME=1) covers
 * the live-process path over the qualified installed binary.
 */

let roots: string[] = [];
function tempRoot(): string {
  const root = mkdtempSync(join(tmpdir(), "c2-real-prime-host-"));
  roots.push(root);
  return root;
}

afterEach(() => {
  for (const root of roots) {
    try {
      rmSync(root, { recursive: true, force: true });
    } catch {
      /* best-effort cleanup */
    }
  }
  roots = [];
});

describe("RealPrimeHost: construction fails closed on an unbindable binary path (C2 tooth 4)", () => {
  it("refuses a relative binary path that contains a separator (./bin/prime-agent)", () => {
    const root = tempRoot();
    expect(
      () =>
        new RealPrimeHost({
          binary: "./bin/prime-agent",
          agentDir: join(root, "agent"),
          sessionDir: join(root, "sessions"),
        }),
    ).toThrow(TerminalBoundaryError);
  });

  it("refuses a binary path carrying a shell metacharacter (forged/injected, not resolved)", () => {
    const root = tempRoot();
    expect(
      () =>
        new RealPrimeHost({
          binary: "/usr/bin/prime-agent; rm -rf /",
          agentDir: join(root, "agent"),
          sessionDir: join(root, "sessions"),
        }),
    ).toThrow(TerminalBoundaryError);
  });

  it("refuses a bare program name (no path separator) — no programResolution table is ever supplied, so a bare name can never be spawned via a child-side PATH lookup", () => {
    const root = tempRoot();
    expect(
      () =>
        new RealPrimeHost({
          binary: "prime-agent",
          agentDir: join(root, "agent"),
          sessionDir: join(root, "sessions"),
        }),
    ).toThrow(TerminalBoundaryError);
  });

  it("a bound host's program can never be substituted per-call — PrimeSpawnRequest names no program/binary field", () => {
    // Structural guarantee, not a runtime probe: the ONLY spawn entry point
    // is PrimeHostPort.spawnChild(PrimeSpawnRequest), whose fields
    // (sessionId/parentSessionId/model/contractPath/task) name no program or
    // binary path at all — `this.binary`, validated once at construction, is
    // the only value ever passed to Bun.spawn.
    const root = tempRoot();
    const host = new RealPrimeHost({
      binary: join(root, "bin", "prime-agent"),
      agentDir: join(root, "agent"),
      sessionDir: join(root, "sessions"),
    });
    expect(host).toBeInstanceOf(RealPrimeHost);
  });
});

describe("RealPrimeHost: spawnChild refuses without a task (a real process needs a goal)", () => {
  it("throws RealPrimeHostSpawnError when task is absent", () => {
    const root = tempRoot();
    const host = new RealPrimeHost({
      binary: join(root, "bin", "prime-agent"),
      agentDir: join(root, "agent"),
      sessionDir: join(root, "sessions"),
    });
    expect(() =>
      host.spawnChild({ sessionId: "s1", contractPath: join(root, "contract.json") }),
    ).toThrow(RealPrimeHostSpawnError);
  });

  it("throws RealPrimeHostSpawnError when task is blank", () => {
    const root = tempRoot();
    const host = new RealPrimeHost({
      binary: join(root, "bin", "prime-agent"),
      agentDir: join(root, "agent"),
      sessionDir: join(root, "sessions"),
    });
    expect(() =>
      host.spawnChild({ sessionId: "s1", contractPath: join(root, "contract.json"), task: "   " }),
    ).toThrow(RealPrimeHostSpawnError);
  });
});

describe("buildRealPrimeEnv: loader-injection stripped unconditionally (D3 attack, defense in depth)", () => {
  it("strips every LD_*/DYLD_* name regardless of source", () => {
    const env = buildRealPrimeEnv(
      {
        PATH: "/usr/bin:/bin",
        LD_PRELOAD: "/tmp/evil.so",
        LD_LIBRARY_PATH: "/tmp/evil",
        DYLD_INSERT_LIBRARIES: "/tmp/evil.dylib",
        HOME: "/home/operator",
      },
      "/isolated/agent",
    );
    expect(env["LD_PRELOAD"]).toBeUndefined();
    expect(env["LD_LIBRARY_PATH"]).toBeUndefined();
    expect(env["DYLD_INSERT_LIBRARIES"]).toBeUndefined();
    expect(env["PATH"]).toBe("/usr/bin:/bin");
    expect(env["HOME"]).toBe("/home/operator");
  });

  it("overlays PRIME_AGENT_CODING_AGENT_DIR to the isolated agent dir, never the caller's value", () => {
    const env = buildRealPrimeEnv(
      { PRIME_AGENT_CODING_AGENT_DIR: "/home/operator/.prime" },
      "/isolated/agent",
    );
    expect(env["PRIME_AGENT_CODING_AGENT_DIR"]).toBe("/isolated/agent");
  });

  it("sets a default TERM only when absent", () => {
    expect(buildRealPrimeEnv({}, "/isolated/agent")["TERM"]).toBe("xterm");
    expect(buildRealPrimeEnv({ TERM: "screen-256color" }, "/isolated/agent")["TERM"]).toBe(
      "screen-256color",
    );
  });

  it("drops undefined-valued entries (Object.entries safety over process.env's optional values)", () => {
    const env = buildRealPrimeEnv({ SOMETHING: undefined, PATH: "/bin" }, "/isolated/agent");
    expect("SOMETHING" in env).toBe(false);
    expect(env["PATH"]).toBe("/bin");
  });
});

describe("RealPrimeHost: getSession/listSessions/receiveMailboxMessage/deliverResultContract delegate to the SAME bookkeeping the fake uses (D2)", () => {
  it("reads on an unpopulated host are honestly empty", () => {
    const root = tempRoot();
    const host = new RealPrimeHost({
      binary: join(root, "bin", "prime-agent"),
      agentDir: join(root, "agent"),
      sessionDir: join(root, "sessions"),
    });
    expect(host.getSession("nonexistent")).toBeNull();
    expect(host.listSessions()).toEqual([]);
    expect(host.liveSessionIds()).toEqual([]);
  });

  it("receiveMailboxMessage on an unknown session throws — the SAME prime-host.ts fake behavior, not a re-implementation of it", () => {
    const root = tempRoot();
    const host = new RealPrimeHost({
      binary: join(root, "bin", "prime-agent"),
      agentDir: join(root, "agent"),
      sessionDir: join(root, "sessions"),
    });
    expect(() =>
      host.receiveMailboxMessage("nonexistent", { sender: "x", content: "hello" }),
    ).toThrow();
  });

  it("deliverResultContract on an unknown session returns false, never throws or fabricates a row", () => {
    const root = tempRoot();
    const host = new RealPrimeHost({
      binary: join(root, "bin", "prime-agent"),
      agentDir: join(root, "agent"),
      sessionDir: join(root, "sessions"),
    });
    expect(
      host.deliverResultContract("nonexistent", {
        session_id: "nonexistent",
        schema_valid: true,
        outcome: "pass",
        evidence_path: "/tmp/x.json",
        completed_at: new Date().toISOString(),
      }),
    ).toBe(false);
  });
});
