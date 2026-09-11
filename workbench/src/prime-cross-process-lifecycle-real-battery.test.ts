/**
 * Phase D gated real-substrate battery (workbench-intent-closure.md §4;
 * gated: GAUNTLET_D_REAL_PRIME_LIFECYCLE=1). Proves the qualified-record
 * reattachment and explicit-stop mechanism
 * (`prime-cross-process-lifecycle.test.ts`) end to end against the REAL,
 * T24-qualified `prime-agent` binary — not the deterministic fake script
 * those unconditional teeth use.
 *
 * Per-run isolation follows T24/C2's established pattern exactly: a unique
 * mktemp root, a 0700 agent directory, the operator's real auth/models
 * COPIED (never mutated) into it, a defensive pkill sweep before/after, and a
 * recursive shred on teardown.
 */

import { afterAll, beforeAll, describe, expect, it } from "bun:test";
import { spawnSync } from "node:child_process";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { homedir, tmpdir } from "node:os";
import { join } from "node:path";
import { isPidAlive, RealPrimeHost } from "./real-prime-host";
import { QUALIFIED_PRIME_BINARY, qualifyPrimeBinary } from "./substrate-inventory";

const REAL_PRIME = process.env["GAUNTLET_D_REAL_PRIME_LIFECYCLE"] === "1";

const QUALIFICATION = REAL_PRIME
  ? await qualifyPrimeBinary().catch((error: unknown) => ({
      qualified: false as const,
      detail: `qualification probe crashed: ${error instanceof Error ? error.message : String(error)}`,
    }))
  : ({
      qualified: false as const,
      detail: "gate off: set GAUNTLET_D_REAL_PRIME_LIFECYCLE=1 to run",
    } as const);

if (REAL_PRIME && !QUALIFICATION.qualified) {
  console.warn(
    `[D] real lifecycle battery NOT qualified on this host — recorded-unqualified: ${QUALIFICATION.detail}`,
  );
}

describe.skipIf(!REAL_PRIME || !QUALIFICATION.qualified)(
  "Phase D real-substrate lifecycle battery (gated: GAUNTLET_D_REAL_PRIME_LIFECYCLE=1)",
  () => {
    let root = "";
    let agentDir = "";
    let sessionDir = "";
    let modelAvailable = true;

    beforeAll(() => {
      root = mkdtempSync(join(tmpdir(), "d-real-lifecycle-"));
      agentDir = join(root, "agent");
      sessionDir = join(root, "sessions");
      spawnSync("pkill", ["-f", "d-real-lifecycle-"], { timeout: 5000 });
      mkdirSync(agentDir, { recursive: true, mode: 0o700 });
      mkdirSync(sessionDir, { recursive: true, mode: 0o700 });
      const operatorDir = join(homedir(), ".prime", "agent");
      for (const name of ["auth.json", "models.json"]) {
        const source = join(operatorDir, name);
        if (existsSync(source) && statSync(source).isFile()) {
          writeFileSync(join(agentDir, name), readFileSync(source));
        } else if (name === "auth.json") {
          modelAvailable = false;
        }
      }
      writeFileSync(
        join(agentDir, "settings.json"),
        JSON.stringify({
          onboardingShown: true,
          defaultProvider: "openai-codex",
          defaultModel: "gpt-5.6-terra",
          telemetry: { enabled: false },
        }),
      );
    });

    afterAll(() => {
      spawnSync("pkill", ["-f", "d-real-lifecycle-"], { timeout: 5000 });
      if (root) rmSync(root, { recursive: true, force: true });
    });

    it("qualification: the installed binary matches the pinned identity", () => {
      expect(QUALIFICATION.qualified).toBe(true);
      if (QUALIFICATION.qualified) {
        expect(QUALIFICATION.probe.version).toBe(QUALIFIED_PRIME_BINARY.version);
      }
    });

    it("a fresh RealPrimeHost reattaches a REAL child mid-flight from the qualified manifest alone, then folds in its real completion", async () => {
      if (!modelAvailable) {
        console.warn("[D] no ~/.prime/agent/auth.json on this host — recording record-unqualified");
        return;
      }
      const sessionId = "d-real-lifecycle-midflight";
      const contractPath = join(root, `${sessionId}.result.json`);
      const hostA = new RealPrimeHost({
        binary: QUALIFIED_PRIME_BINARY.binary,
        agentDir,
        sessionDir,
        timeoutMs: 60_000,
      });
      hostA.spawnChild({
        sessionId,
        parentSessionId: null,
        contractPath,
        task: "Reply with exactly the single word: forty-two. Do not use any tools.",
      });

      // A SECOND, independent instance — standing in for the fresh workbench
      // process after the first one died — reconstructs the same session
      // purely from the qualified manifest on disk, not from hostA's memory.
      const hostB = new RealPrimeHost({
        binary: QUALIFIED_PRIME_BINARY.binary,
        agentDir,
        sessionDir,
        timeoutMs: 60_000,
      });
      expect(["running", "completed"]).toContain(hostB.getSession(sessionId)?.handle.status ?? "");
      expect(hostB.staleRecords()).toEqual([]);

      const deadline = Date.now() + 55_000;
      let session = hostB.getSession(sessionId);
      while (session?.handle.status === "running" && Date.now() < deadline) {
        await new Promise((resolve) => setTimeout(resolve, 500));
        session = hostB.getSession(sessionId);
      }
      expect(session?.handle.status).toBe("completed");
      expect(session?.resultContract?.schema_valid).toBe(true);
      expect(existsSync(contractPath)).toBe(true);
      void hostA;
    }, 65_000);

    it("an explicit stop terminates a REAL running prime-agent process", async () => {
      if (!modelAvailable) {
        console.warn("[D] no ~/.prime/agent/auth.json on this host — recording record-unqualified");
        return;
      }
      const sessionId = "d-real-lifecycle-stop";
      const contractPath = join(root, `${sessionId}.result.json`);
      const host = new RealPrimeHost({
        binary: QUALIFIED_PRIME_BINARY.binary,
        agentDir,
        sessionDir,
        timeoutMs: 60_000,
      });
      host.spawnChild({
        sessionId,
        parentSessionId: null,
        contractPath,
        task: "Count slowly from 1 to 1000000, one number per line, with no other text.",
      });
      const manifest = JSON.parse(
        readFileSync(join(sessionDir, `${sessionId}.qualified.json`), "utf8"),
      ) as { pid: number };

      const outcome = host.stopChild(sessionId);
      expect(outcome.stopped).toBe(true);

      const deadline = Date.now() + 15_000;
      while (isPidAlive(manifest.pid) && Date.now() < deadline) {
        await new Promise((resolve) => setTimeout(resolve, 200));
      }
      expect(isPidAlive(manifest.pid)).toBe(false);
    }, 25_000);
  },
);
