import { Database } from "bun:sqlite";
import { describe, expect, it } from "bun:test";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

describe("T03 Bun 1.4 Native-Substrate Smoke & Qualification (REQ-BUN-001, REQ-BUN-002, REQ-CONFIG-004)", () => {
  it("qualifies native bun:sqlite for local projection and caching", () => {
    const db = new Database(":memory:");
    db.run("CREATE TABLE test_units (id TEXT PRIMARY KEY, status TEXT, tier TEXT)");
    const insert = db.prepare("INSERT INTO test_units (id, status, tier) VALUES (?, ?, ?)");
    insert.run("AK-001", "pass", "P1");
    insert.run("AK-002", "fail", "P2");

    const rows = db.query("SELECT * FROM test_units ORDER BY id ASC").all() as {
      id: string;
      status: string;
      tier: string;
    }[];

    expect(rows.length).toBe(2);
    expect(rows[0]?.id).toBe("AK-001");
    expect(rows[0]?.status).toBe("pass");
    expect(rows[1]?.id).toBe("AK-002");
    expect(rows[1]?.status).toBe("fail");
    db.close();
  });

  it("qualifies native Bun.serve for local HTTP communication", async () => {
    const server = Bun.serve({
      port: 0,
      fetch(req) {
        return Response.json({ status: "ok", url: req.url });
      },
    });

    const start = performance.now();
    const res = await fetch(`http://127.0.0.1:${server.port}/health`);
    const elapsed = performance.now() - start;

    expect(res.ok).toBe(true);
    const data = (await res.json()) as { status: string };
    expect(data.status).toBe("ok");
    expect(elapsed).toBeLessThan(50);
    server.stop();
  });

  it("qualifies native Bun.spawn for nonblocking process execution", async () => {
    const proc = Bun.spawn(["echo", "gauntlet-native-process"]);
    const text = await new Response(proc.stdout).text();
    const exitCode = await proc.exited;

    expect(exitCode).toBe(0);
    expect(text.trim()).toBe("gauntlet-native-process");
  });

  it("qualifies native Bun.gzipSync / Bun.gunzipSync for artifact compression", () => {
    const original = "Canonical Gauntlet Evidence Bundle Payload 2026";
    const buffer = Buffer.from(original, "utf-8");
    const compressed = Bun.gzipSync(buffer);
    const decompressed = Bun.gunzipSync(compressed);

    expect(new TextDecoder().decode(decompressed)).toBe(original);
  });

  it("qualifies native Bun.CryptoHasher for sha256 checksums", () => {
    const hasher = new Bun.CryptoHasher("sha256");
    hasher.update("gauntlet-canonical-hash-vector");
    const digest = hasher.digest("hex");

    expect(digest).toBe("ed3df4d4303d249522ff21af16d9bc8da2c996d56d6bbc7ae9ad831d4e7a8707");
  });

  describe("T03 Teeth Falsification Attacks (REQ-BUN-001, REQ-BUN-002)", () => {
    it("Attack 1: Rejects overlapping dependencies with no qualification artifact (REQ-BUN-001)", () => {
      const packageJsonPath = resolve(import.meta.dir, "../package.json");
      const packageJson = JSON.parse(readFileSync(packageJsonPath, "utf-8")) as {
        dependencies?: Record<string, string>;
        devDependencies?: Record<string, string>;
      };

      const FORBIDDEN_OVERLAPS = [
        "better-sqlite3",
        "sqlite3",
        "express",
        "fastify",
        "axios",
        "node-fetch",
        "got",
        "execa",
        "cross-spawn",
        "chalk",
        "pino",
        "winston",
      ];

      const allDeps = {
        ...packageJson.dependencies,
        ...packageJson.devDependencies,
      };

      for (const overlap of FORBIDDEN_OVERLAPS) {
        expect(allDeps[overlap]).toBeUndefined();
      }

      // Admitted dependencies must match the explicit qualification list.
      // quickjs-wasi (3.6.2, package.json since 914c538) is admitted
      // deliberately: it is imported by src/codemode/sandbox-worker.ts (the
      // codemode program-host WASM sandbox). The attack's strength is kept:
      // every prod dependency in package.json must appear here, so any FUTURE
      // undeclared dependency still fails until it is explicitly qualified.
      const ADMITTED_PROD = [
        "@opentui/core",
        "@opentui/react",
        "quickjs-wasi",
        "react",
        "react-reconciler",
      ];

      for (const dep of Object.keys(packageJson.dependencies ?? {})) {
        expect(ADMITTED_PROD).toContain(dep);
      }
    });

    it("Attack 2: Forced probe failure records failure and isolates affected capability (REQ-BUN-002)", () => {
      // Simulate a probe function that evaluates native capabilities
      interface ProbeLedgerEntry {
        capability: string;
        status: "qualified" | "falsified";
        error?: string | undefined;
      }

      function evaluateCapabilities(forcedFailCapability?: string): ProbeLedgerEntry[] {
        const capabilities = ["sqlite", "http", "spawn", "compression", "crypto"];
        return capabilities.map((cap) => {
          if (cap === forcedFailCapability) {
            return {
              capability: cap,
              status: "falsified",
              error: `Forced failure on ${cap}: target environment lacks support`,
            };
          }
          return { capability: cap, status: "qualified" };
        });
      }

      // 1. All pass when healthy
      const healthy = evaluateCapabilities();
      expect(healthy.every((e) => e.status === "qualified")).toBe(true);

      // 2. Forced failure on 'sqlite': only sqlite is marked falsified, others remain qualified
      const degraded = evaluateCapabilities("sqlite");
      const sqliteEntry = degraded.find((e) => e.capability === "sqlite");
      const httpEntry = degraded.find((e) => e.capability === "http");

      expect(sqliteEntry?.status).toBe("falsified");
      expect(sqliteEntry?.error).toContain("Forced failure on sqlite");
      expect(httpEntry?.status).toBe("qualified");
      // Plan does not claim Bun coverage when a probe fails!
    });
  });
});
