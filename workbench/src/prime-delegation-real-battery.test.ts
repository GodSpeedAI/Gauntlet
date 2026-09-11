/**
 * Phase C2 gated real-substrate battery (workbench-intent-closure.md §4;
 * gated: GAUNTLET_C2_REAL_PRIME=1). Proves `RealPrimeHost` end to end against
 * the REAL, T24-qualified `prime-agent` binary — not the deterministic
 * stand-in script `prime-delegation-real.test.ts` uses for its unconditional
 * dispatcher-level teeth.
 *
 * Per-run isolation follows T24's own established pattern exactly
 * (prime-fresh-target.test.ts): a unique mktemp root, a 0700 agent
 * directory, the operator's real auth/models COPIED (never mutated) into
 * it, a defensive pkill sweep before/after, and a recursive shred on
 * teardown. Unlike T24's battery, `RealPrimeHost` never spawns a persistent
 * daemon — each delegated child is its own one-shot `Bun.spawn`, so there is
 * no daemon socket to set up here.
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
import { FocusRef } from "./focus";
import { createHorizon, planMaterialization } from "./horizon";
import { OPERATION_IDS } from "./operation-registry";
import { constructBoundedContextEnvelope } from "./prime-context-envelope";
import { PrimeDelegationDispatcher } from "./prime-recursion";
import { RealPrimeHost } from "./real-prime-host";
import type { ProjectionWorld } from "./relations";
import { QUALIFIED_PRIME_BINARY, qualifyPrimeBinary } from "./substrate-inventory";
import { CanonicalSurfaceStub } from "./testing/canonical-surface-stub";

const REAL_PRIME = process.env["GAUNTLET_C2_REAL_PRIME"] === "1";

const QUALIFICATION = REAL_PRIME
  ? await qualifyPrimeBinary().catch((error: unknown) => ({
      qualified: false as const,
      detail: `qualification probe crashed: ${error instanceof Error ? error.message : String(error)}`,
    }))
  : ({
      qualified: false as const,
      detail: "gate off: set GAUNTLET_C2_REAL_PRIME=1 to run",
    } as const);

if (REAL_PRIME && !QUALIFICATION.qualified) {
  console.warn(
    `[C2] real battery NOT qualified on this host — recorded-unqualified: ${QUALIFICATION.detail}`,
  );
}

describe.skipIf(!REAL_PRIME || !QUALIFICATION.qualified)(
  "Phase C2 real-substrate battery (gated: GAUNTLET_C2_REAL_PRIME=1)",
  () => {
    let root = "";
    let agentDir = "";
    let sessionDir = "";
    let modelAvailable = true;

    beforeAll(() => {
      root = mkdtempSync(join(tmpdir(), "c2-real-"));
      agentDir = join(root, "agent");
      sessionDir = join(root, "sessions");
      spawnSync("pkill", ["-f", "c2-real-"], { timeout: 5000 });
      mkdirSync(agentDir, { recursive: true, mode: 0o700 });
      mkdirSync(sessionDir, { recursive: true });
      // Isolated agent dir: the operator's auth/models are COPIED, never
      // modified — RealPrimeHost's own constructor never touches ~/.prime.
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
      spawnSync("pkill", ["-f", "c2-real-"], { timeout: 5000 });
      if (root) rmSync(root, { recursive: true, force: true });
    });

    it("qualification: the installed binary matches the pinned identity and every relied-on surface token is present", () => {
      expect(QUALIFICATION.qualified).toBe(true);
      if (QUALIFICATION.qualified) {
        expect(QUALIFICATION.probe.version).toBe(QUALIFIED_PRIME_BINARY.version);
      }
    });

    it("a real delegated child runs through RealPrimeHost end to end: receipt before spawn, real process, real completion", async () => {
      if (!modelAvailable) {
        console.warn(
          "[C2] no ~/.prime/agent/auth.json on this host — recording record-unqualified for this leg",
        );
        return;
      }
      const host = new RealPrimeHost({
        binary: QUALIFIED_PRIME_BINARY.binary,
        agentDir,
        sessionDir,
        timeoutMs: 60_000,
      });
      const stub = new CanonicalSurfaceStub({
        run_id: "01JC2REALBATTERY000001",
        bound_actor: "operator-c2-real",
        session_token: "c2-real-battery-token",
      }).addUnit({
        unit_id: "AK-999",
        status: "blocked",
        tier: "P3",
        settled: false,
        authority: "granted",
        authority_visible_in_reads: true,
        governance_denied: false,
        governance_visible_in_reads: true,
        open_ticket: "ESC-999",
      });
      const dispatcher = new PrimeDelegationDispatcher({
        adapter: host,
        canonical_route: stub.asInProcessRoute(),
      });
      const world: ProjectionWorld = {
        generation: "gen-c2-real-battery",
        units: [],
        evidence: [],
        sessions: [],
        source_cursors: {},
      };
      const horizon = createHorizon({
        root_focus: FocusRef.for("gauntlet", "WorkItem", "u-c2-real-battery"),
        generation: "gen-c2-real-battery",
        relation_queries: [],
        horizon_id: "horizon-c2-real-battery",
      });
      const envelope = constructBoundedContextEnvelope({
        horizon,
        plan: planMaterialization(world, horizon),
        role_instructions: ["Reply with the single word: forty-two"],
        constructed_at_ms: Date.now(),
      });

      const outcome = await dispatcher.delegate({
        operation_request: {
          operation: OPERATION_IDS.UNIT_RESOLVE,
          focus: FocusRef.for("gauntlet", "WorkItem", "AK-999"),
          session: { actor: "operator-c2-real", token: "c2-real-battery-token" },
          payload: { note: "C2 real battery", run: "01JC2REALBATTERY000001" },
        },
        envelope,
        child_session_id: "sess-c2-real-battery",
        parent_session_id: null,
        task: "Reply with exactly the single word: forty-two. Do not use any tools.",
        contract_path: join(root, "sess-c2-real-battery.result.json"),
      });

      expect(outcome.class).toBe("admitted");
      // Nonblocking (REQ-PRIME-003): admission returns before the real
      // process necessarily finishes — status is running or already
      // completed depending on scheduling, never anything else.
      const justAfter = host.getSession("sess-c2-real-battery");
      expect(justAfter).not.toBeNull();
      expect(["running", "completed", "failed"]).toContain(justAfter?.handle.status ?? "");

      // Wait for the real process to actually finish.
      const deadline = Date.now() + 55_000;
      let session = host.getSession("sess-c2-real-battery");
      while (session?.handle.status === "running" && Date.now() < deadline) {
        await new Promise((resolve) => setTimeout(resolve, 500));
        session = host.getSession("sess-c2-real-battery");
      }

      expect(session).not.toBeNull();
      // A schema-valid result contract landed — completion came from THAT,
      // never inferred from mailbox text alone (REQ-PRIME-005).
      expect(session?.resultContract).not.toBeNull();
      expect(session?.resultContract?.schema_valid).toBe(true);
      expect(existsSync(join(root, "sess-c2-real-battery.result.json"))).toBe(true);
      expect(host.liveSessionIds()).toEqual([]); // the real process exited; watch() cleaned it up
    }, 65_000);
  },
);
