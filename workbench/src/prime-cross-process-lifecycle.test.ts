/**
 * Phase D — long-running lifecycle and reattachment in production
 * (workbench-intent-closure.md §4; REQ-VERIFY-005, REQ-PRIME-009/010,
 * REQ-OUT-003). Unconditional teeth (no gate): a deterministic fake
 * `prime-agent` script drives `RealPrimeHost`'s REAL `Bun.spawn` pipeline —
 * the qualified-record reattachment machinery under test here is identical
 * code regardless of which binary is spawned; the gated real-substrate
 * battery separately re-proves the same shapes against the qualified real
 * binary.
 *
 * Core claim under test: a FRESH `RealPrimeHost` instance (standing in for a
 * fresh workbench process after the prior one died) reconstructs topology
 * from its own qualified `*.qualified.json` manifests ONLY — never from
 * directory presence, a prior instance's memory, or inferred liveness
 * (REQ-PRIME-010's explicit prohibition) — and an explicit `stopChild()` is
 * the only thing that ever terminates a real child.
 */

import { describe, expect, it } from "bun:test";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { isPidAlive, RealPrimeHost } from "./real-prime-host";
import { writeFakePrimeAgent } from "./testing/fake-prime-binary";

function tempRoot(): string {
  return mkdtempSync(join(tmpdir(), "d-cross-process-"));
}

describe("Phase D — qualified-record reattachment (REQ-PRIME-010)", () => {
  it("a fresh host sees a still-running child as RUNNING before completion, then folds in the real completion — from the manifest alone, never shared memory", async () => {
    const root = tempRoot();
    try {
      const binary = join(root, "fake-prime-agent.sh");
      writeFakePrimeAgent(binary, { assistantText: "working", sleepMs: 1200 });
      const sessionDir = join(root, "sessions");
      const agentDir = join(root, "agent");
      const sessionId = "sess-d-midflight";
      const contractPath = join(root, `${sessionId}.result.json`);

      const hostA = new RealPrimeHost({ binary, agentDir, sessionDir });
      hostA.spawnChild({ sessionId, parentSessionId: null, contractPath, task: "go" });

      // hostA is never referenced again below — standing in for "the TUI
      // process died"; the underlying child process is NOT tied to hostA's
      // JS object (Unix does not cascade-kill orphaned children), so a fresh
      // instance pointed at the same directories must see it independently.
      const hostB = new RealPrimeHost({ binary, agentDir, sessionDir });
      expect(hostB.getSession(sessionId)?.handle.status).toBe("running");
      expect(hostB.listSessions().map((s) => s.handle.session_id)).toEqual([sessionId]);
      expect(hostB.staleRecords()).toEqual([]);

      const deadline = Date.now() + 8_000;
      while (hostB.getSession(sessionId)?.handle.status === "running" && Date.now() < deadline) {
        await new Promise((resolve) => setTimeout(resolve, 100));
      }
      expect(hostB.getSession(sessionId)?.handle.status).toBe("completed");
      expect(hostB.getSession(sessionId)?.resultContract?.schema_valid).toBe(true);
      void hostA;
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  }, 15_000);

  it("a fresh host started AFTER completion folds the result directly from the contract file — synchronously, no polling needed", async () => {
    const root = tempRoot();
    try {
      const binary = join(root, "fake-prime-agent.sh");
      writeFakePrimeAgent(binary, { assistantText: "done fast", exitCode: 0 });
      const sessionDir = join(root, "sessions");
      const agentDir = join(root, "agent");
      const sessionId = "sess-d-postcompletion";
      const contractPath = join(root, `${sessionId}.result.json`);

      const hostA = new RealPrimeHost({ binary, agentDir, sessionDir });
      hostA.spawnChild({ sessionId, parentSessionId: null, contractPath, task: "go" });
      const deadline = Date.now() + 8_000;
      while (hostA.getSession(sessionId)?.handle.status === "running" && Date.now() < deadline) {
        await new Promise((resolve) => setTimeout(resolve, 50));
      }
      expect(hostA.getSession(sessionId)?.handle.status).toBe("completed");

      const hostC = new RealPrimeHost({ binary, agentDir, sessionDir });
      // Synchronous: the constructor folds an already-landed contract with
      // no polling delay at all.
      expect(hostC.getSession(sessionId)?.handle.status).toBe("completed");
      expect(hostC.getSession(sessionId)?.resultContract?.evidence_path).toBe(contractPath);
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  }, 15_000);

  it("directory presence WITHOUT a qualified record never fabricates a running session (garbage files are ignored, not stale)", () => {
    const root = tempRoot();
    try {
      const binary = join(root, "fake-prime-agent.sh");
      writeFakePrimeAgent(binary, {});
      const sessionDir = join(root, "sessions");
      const agentDir = join(root, "agent");
      mkdirSync(sessionDir, { recursive: true });
      // Unrelated file, unparseable JSON, and well-formed-but-wrong-shape JSON
      // — none of these is a qualified record.
      writeFileSync(join(sessionDir, "notes.txt"), "hello");
      writeFileSync(join(sessionDir, "broken.qualified.json"), "{not json");
      writeFileSync(join(sessionDir, "wrong-shape.qualified.json"), JSON.stringify({ foo: "bar" }));

      const host = new RealPrimeHost({ binary, agentDir, sessionDir });
      expect(host.listSessions()).toEqual([]);
      expect(host.staleRecords()).toEqual([]);
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });

  it("a qualified manifest whose process is gone with no result lands as a typed-unavailable stale record — never as running", async () => {
    const root = tempRoot();
    try {
      const binary = join(root, "fake-prime-agent.sh");
      writeFakePrimeAgent(binary, {});
      const sessionDir = join(root, "sessions");
      const agentDir = join(root, "agent");
      mkdirSync(sessionDir, { recursive: true });

      // A definitely-dead pid: spawn a trivial process and await its exit.
      const dead = Bun.spawn(["true"], { stdout: "ignore", stderr: "ignore" });
      await dead.exited;
      expect(isPidAlive(dead.pid)).toBe(false);

      const sessionId = "sess-d-stale";
      writeFileSync(
        join(sessionDir, `${sessionId}.qualified.json`),
        JSON.stringify({
          session_id: sessionId,
          parent_session_id: null,
          model: "claude-3-5-sonnet",
          contract_path: join(root, `${sessionId}.result.json`), // never created
          pid: dead.pid,
          spawned_at: new Date().toISOString(),
        }),
      );

      const host = new RealPrimeHost({ binary, agentDir, sessionDir });
      expect(host.getSession(sessionId)).toBeNull();
      expect(host.listSessions()).toEqual([]);
      expect(host.staleRecords()).toEqual([
        { session_id: sessionId, reason: "process_not_found_no_result_contract" },
      ]);
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });
});

describe("Phase D — an explicit stop is the ONLY thing that terminates real admitted work (REQ-PRIME-009)", () => {
  it("stopChild on a session this SAME instance directly owns actually kills the real process; the session settles as non-success, never silently completed", async () => {
    const root = tempRoot();
    try {
      const binary = join(root, "fake-prime-agent.sh");
      writeFakePrimeAgent(binary, { sleepMs: 5000 });
      const sessionDir = join(root, "sessions");
      const agentDir = join(root, "agent");
      const sessionId = "sess-d-stop-owned";
      const contractPath = join(root, `${sessionId}.result.json`);

      const host = new RealPrimeHost({ binary, agentDir, sessionDir });
      host.spawnChild({ sessionId, parentSessionId: null, contractPath, task: "go" });
      const manifest = JSON.parse(
        readFileSync(join(sessionDir, `${sessionId}.qualified.json`), "utf8"),
      ) as { pid: number };
      expect(isPidAlive(manifest.pid)).toBe(true);

      const outcome = host.stopChild(sessionId);
      expect(outcome.stopped).toBe(true);

      const deadline = Date.now() + 5_000;
      while (isPidAlive(manifest.pid) && Date.now() < deadline) {
        await new Promise((resolve) => setTimeout(resolve, 50));
      }
      expect(isPidAlive(manifest.pid)).toBe(false);

      // watch()'s own natural completion path records the outcome — a
      // schema-valid contract still lands (the process's real exit was
      // observed and written, same as any other exit), but the SIGKILL
      // must never be recorded as success (REQ-PRIME-009: nothing but an
      // explicit stop terminates work, and the stop must never launder into
      // a fabricated "pass").
      const settleDeadline = Date.now() + 5_000;
      while (
        host.getSession(sessionId)?.handle.status === "running" &&
        Date.now() < settleDeadline
      ) {
        await new Promise((resolve) => setTimeout(resolve, 50));
      }
      expect(host.getSession(sessionId)?.handle.status).toBe("completed");
      expect(host.getSession(sessionId)?.resultContract?.outcome).not.toBe("pass");
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  }, 15_000);

  it("stopChild on a REATTACHED session (no live handle, only a qualified pid) still kills the real process and settles it deterministically", async () => {
    const root = tempRoot();
    try {
      const binary = join(root, "fake-prime-agent.sh");
      writeFakePrimeAgent(binary, { sleepMs: 5000 });
      const sessionDir = join(root, "sessions");
      const agentDir = join(root, "agent");
      const sessionId = "sess-d-stop-reattached";
      const contractPath = join(root, `${sessionId}.result.json`);

      const hostA = new RealPrimeHost({ binary, agentDir, sessionDir });
      hostA.spawnChild({ sessionId, parentSessionId: null, contractPath, task: "go" });
      const manifest = JSON.parse(
        readFileSync(join(sessionDir, `${sessionId}.qualified.json`), "utf8"),
      ) as { pid: number };

      const hostB = new RealPrimeHost({ binary, agentDir, sessionDir });
      expect(hostB.getSession(sessionId)?.handle.status).toBe("running");

      const outcome = hostB.stopChild(sessionId);
      expect(outcome.stopped).toBe(true);
      // Synchronous: no natural watch() task exists for a reattached session
      // — this call itself records the terminal state.
      expect(hostB.getSession(sessionId)?.handle.status).toBe("failed");
      expect(hostB.getSession(sessionId)?.resultContract).toBeNull();

      const deadline = Date.now() + 5_000;
      while (isPidAlive(manifest.pid) && Date.now() < deadline) {
        await new Promise((resolve) => setTimeout(resolve, 50));
      }
      expect(isPidAlive(manifest.pid)).toBe(false);
      void hostA;
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  }, 15_000);

  it("stopChild refuses cleanly on an unknown session and on an already-terminal one — zero blast radius, no signal sent anywhere", async () => {
    const root = tempRoot();
    try {
      const binary = join(root, "fake-prime-agent.sh");
      writeFakePrimeAgent(binary, { exitCode: 0 });
      const sessionDir = join(root, "sessions");
      const agentDir = join(root, "agent");
      const sessionId = "sess-d-stop-terminal";
      const contractPath = join(root, `${sessionId}.result.json`);

      const host = new RealPrimeHost({ binary, agentDir, sessionDir });
      expect(host.stopChild("sess-never-existed").stopped).toBe(false);

      host.spawnChild({ sessionId, parentSessionId: null, contractPath, task: "go" });
      const deadline = Date.now() + 8_000;
      while (host.getSession(sessionId)?.handle.status === "running" && Date.now() < deadline) {
        await new Promise((resolve) => setTimeout(resolve, 50));
      }
      expect(host.getSession(sessionId)?.handle.status).toBe("completed");

      const outcome = host.stopChild(sessionId);
      expect(outcome.stopped).toBe(false);
      expect(outcome.detail).toContain("already terminal");
      // The completed record is untouched by the refused stop attempt.
      expect(host.getSession(sessionId)?.handle.status).toBe("completed");
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  }, 15_000);
});
