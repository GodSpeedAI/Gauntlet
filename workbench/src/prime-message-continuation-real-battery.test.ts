/**
 * Gated real-substrate battery for plan `workbench-agent-interaction.md`
 * increment P1's `prime.message` continuation path (gated:
 * GAUNTLET_P1_REAL_PRIME_MESSAGE=1). Proves `RealPrimeHost.continueChild`
 * against the REAL, T24-qualified `prime-agent` binary: a launched child
 * finishes its first one-shot turn, then a follow-up message resumes the
 * SAME session with a new one-shot turn (`--resume`) — never a second
 * session — and the reply lands in the mailbox as communication.
 *
 * Isolation follows the established C2 pattern exactly
 * (`prime-delegation-real-battery.test.ts`): a unique mktemp root, a 0700
 * agent directory, the operator's real auth/models COPIED (never mutated)
 * into it, and a recursive shred on teardown.
 */

import { afterAll, beforeAll, describe, expect, it } from "bun:test";
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
import { RealPrimeHost } from "./real-prime-host";
import { QUALIFIED_PRIME_BINARY, qualifyPrimeBinary } from "./substrate-inventory";

const REAL_PRIME = process.env["GAUNTLET_P1_REAL_PRIME_MESSAGE"] === "1";

const QUALIFICATION = REAL_PRIME
  ? await qualifyPrimeBinary().catch((error: unknown) => ({
      qualified: false as const,
      detail: `qualification probe crashed: ${error instanceof Error ? error.message : String(error)}`,
    }))
  : ({
      qualified: false as const,
      detail: "gate off: set GAUNTLET_P1_REAL_PRIME_MESSAGE=1 to run",
    } as const);

if (REAL_PRIME && !QUALIFICATION.qualified) {
  console.warn(
    `[P1] real message-continuation battery NOT qualified on this host — recorded-unqualified: ${QUALIFICATION.detail}`,
  );
}

describe.skipIf(!REAL_PRIME || !QUALIFICATION.qualified)(
  "Phase P1 real prime.message continuation battery (gated: GAUNTLET_P1_REAL_PRIME_MESSAGE=1)",
  () => {
    let root = "";
    let agentDir = "";
    let sessionDir = "";
    let modelAvailable = true;

    beforeAll(() => {
      root = mkdtempSync(join(tmpdir(), "p1-real-message-"));
      agentDir = join(root, "agent");
      sessionDir = join(root, "sessions");
      mkdirSync(agentDir, { recursive: true, mode: 0o700 });
      mkdirSync(sessionDir, { recursive: true });
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
      if (root) rmSync(root, { recursive: true, force: true });
    });

    it("a finished child resumes the SAME session with REAL context: turn 1 plants a codeword, turn 2 recalls it (hard failure on a broken resume)", async () => {
      if (!modelAvailable) {
        console.warn(
          "[P1] no ~/.prime/agent/auth.json on this host — recording record-unqualified for this leg",
        );
        return;
      }
      const host = new RealPrimeHost({
        binary: QUALIFIED_PRIME_BINARY.binary,
        agentDir,
        sessionDir,
        timeoutMs: 60_000,
      });
      const sessionId = "sess-p1-real-message";
      const contractPath = join(root, `${sessionId}.result.json`);
      const CODEWORD = "PELICAN";
      host.spawnChild({
        sessionId,
        contractPath,
        task: `Remember the codeword ${CODEWORD}. Reply with exactly the single word: acknowledged. Do not use any tools.`,
      });

      const deadline1 = Date.now() + 55_000;
      let session = host.getSession(sessionId);
      while (session?.handle.status === "running" && Date.now() < deadline1) {
        await new Promise((resolve) => setTimeout(resolve, 500));
        session = host.getSession(sessionId);
      }
      expect(session?.resultContract?.schema_valid).toBe(true);
      const firstMessageCount = session?.messages.length ?? 0;

      const result = host.continueChild(sessionId, {
        sender: "operator",
        message: "What was the codeword? Reply with exactly that one word and nothing else.",
      });
      expect(result.delivered).toBe(true);
      expect(result.mode).toBe("resumed_new_turn");
      expect(result.session_id).toBe(sessionId);
      // The resumed spawn must actually target prime-agent's OWN session id,
      // never this workbench's session id (the bug this battery exists to
      // catch: `--resume <workbench id>` exits 1 against the real binary).
      expect(result.detail).not.toContain(sessionId);

      // The operator's message is recorded immediately.
      const afterSend = host.getSession(sessionId);
      expect(afterSend?.messages.length ?? 0).toBeGreaterThan(firstMessageCount);

      // Wait for the resumed turn's process to actually exit (watch() writes
      // its mailbox entry only after BOTH stdout and the exit code resolve).
      const deadline2 = Date.now() + 110_000;
      while (host.liveSessionIds().includes(sessionId) && Date.now() < deadline2) {
        await new Promise((resolve) => setTimeout(resolve, 500));
      }

      // No second session was ever minted for the follow-up turn — the SAME
      // workbench session id throughout.
      expect(host.listSessions().map((s) => s.handle.session_id)).toEqual([sessionId]);

      // HARD requirements — a broken resume (wrong --resume target, lost
      // context, non-zero exit) fails this test, it does not degrade to a
      // console.warn.
      expect(host.liveSessionIds().includes(sessionId)).toBe(false); // the resumed process actually exited
      const finalSession = host.getSession(sessionId);
      expect(finalSession?.resultContract?.schema_valid).toBe(true); // the resumed turn's own contract landed
      const finalContract = JSON.parse(readFileSync(contractPath, "utf8")) as {
        exit_code?: unknown;
      };
      expect(finalContract.exit_code).toBe(0); // the resumed prime-agent process exited 0
      const messages = finalSession?.messages ?? [];
      const gotCodeword = messages.some(
        (m) => m.kind === "communication" && m.content.toUpperCase().includes(CODEWORD),
      );
      expect(gotCodeword).toBe(true); // real context carried across the resume
    }, 200_000);
  },
);
