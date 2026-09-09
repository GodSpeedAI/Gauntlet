import { describe, expect, it } from "bun:test";
import { classifyTransport, DEFAULT_CONFIG, redactConfig, resolveConfig } from "./config";
import { evaluatePreflight } from "./preflight";

describe("T02 Configuration, Preflight, and Degraded-Mode Contract (REQ-CONFIG-001..012)", () => {
  describe("Configuration Precedence & Secret Indirection", () => {
    it("preserves documented defaults when no overrides are provided", () => {
      const config = resolveConfig({
        cli: {},
        env: {},
        file: {},
      });
      expect(config.gauntlet_endpoint).toBe(DEFAULT_CONFIG.gauntlet_endpoint);
      expect(config.prime_endpoint).toBeNull();
      expect(config.transport).toBe("local");
      expect(config.session_token).toBeNull();
    });

    it("prioritizes CLI flags over ENV and config file (REQ-CONFIG-001)", () => {
      const config = resolveConfig({
        cli: {
          gauntlet_endpoint: "http://127.0.0.1:9999",
          prime_endpoint: "http://127.0.0.1:8888",
        },
        env: {
          GAUNTLET_WORKBENCH_ENDPOINT: "http://127.0.0.1:7777",
          GAUNTLET_WORKBENCH_PRIME_ENDPOINT: "http://127.0.0.1:6666",
        },
        file: {
          gauntlet_endpoint: "http://127.0.0.1:5555",
          prime_endpoint: "http://127.0.0.1:4444",
        },
      });

      // CLI wins over both ENV and file
      expect(config.gauntlet_endpoint).toBe("http://127.0.0.1:9999");
      expect(config.prime_endpoint).toBe("http://127.0.0.1:8888");
    });

    it("prioritizes ENV over config file when CLI flag is absent", () => {
      const config = resolveConfig({
        cli: {},
        env: {
          GAUNTLET_WORKBENCH_ENDPOINT: "http://127.0.0.1:7777",
          GAUNTLET_SESSION_TOKEN: "secret-env-token",
        },
        file: {
          gauntlet_endpoint: "http://127.0.0.1:5555",
        },
      });

      expect(config.gauntlet_endpoint).toBe("http://127.0.0.1:7777");
      expect(config.session_token).toBe("secret-env-token");
    });

    it("correctly classifies local loopback vs remote transport (REQ-CONFIG-005)", () => {
      expect(classifyTransport("http://127.0.0.1:4000")).toBe("local");
      expect(classifyTransport("http://localhost:4000")).toBe("local");
      expect(classifyTransport("http://192.168.1.50:4000")).toBe("remote");
      expect(classifyTransport("https://gauntlet.internal.corp:8443")).toBe("remote");
    });

    it("redacts secrets and credentials in logs and diagnostics (REQ-CONFIG-002)", () => {
      const config = resolveConfig({
        cli: {
          session_token: "super-secret-production-token",
        },
      });
      const redacted = redactConfig(config);
      expect(redacted["session_token"]).toBe("[REDACTED]");
      expect(JSON.stringify(redacted)).not.toContain("super-secret-production-token");
    });
  });

  describe("T02 Teeth Falsification Attacks (REQ-CONFIG-010..012)", () => {
    it("Attack 1: Remove Prime while keeping Gauntlet reachable -> safe read-only inspection survives", () => {
      const config = resolveConfig({
        cli: {
          gauntlet_endpoint: "http://127.0.0.1:4000",
          prime_endpoint: "http://127.0.0.1:5000",
        },
      });

      // Simulate Prime being down/unreachable, but Gauntlet is reachable
      const report = evaluatePreflight(config, {
        gauntlet_reachable: true,
        gauntlet_protocol_version: "1.0.0",
        prime_reachable: false,
        projection_store_writable: true,
      });

      // Workbench remains operational in degraded mode
      expect(report.ok).toBe(true);
      expect(report.degraded).toBe(true);
      expect(report.gauntlet_status).toBe("connected");
      expect(report.prime_status).toBe("unavailable");

      // Safe Gauntlet inspection capabilities remain available
      expect(report.allowed_capabilities).toContain("state_snapshot");
      expect(report.allowed_capabilities).toContain("unit_inspection");
      expect(report.allowed_capabilities).toContain("escalation_inbox");

      // Prime-dependent capabilities are cleanly blocked
      expect(report.blocked_capabilities).toContain("prime_chat");
      expect(report.blocked_capabilities).toContain("prime_steering");
      expect(report.blocked_capabilities).toContain("prime_subagent_spawn");

      // Typed error specifically scoped to Prime integration
      const primeError = report.errors.find((e) => e.code === "prime_integration_error");
      expect(primeError).toBeDefined();
      expect(primeError?.source).toBe("prime");
      expect(primeError?.blast_radius).toContain("core Gauntlet inspection unaffected");
    });

    it("Attack 2: Remove Gauntlet connectivity while cached projections and Prime exist -> all state-changing commands fail closed", () => {
      const config = resolveConfig({
        cli: {
          gauntlet_endpoint: "http://127.0.0.1:4000",
          prime_endpoint: "http://127.0.0.1:5000",
        },
      });

      // Simulate Gauntlet losing connection, while Prime and cached projections exist
      const report = evaluatePreflight(config, {
        gauntlet_reachable: false,
        prime_reachable: true,
        projection_store_writable: true,
      });

      // Startup fails / blocks privileged actions
      expect(report.ok).toBe(false);
      expect(report.gauntlet_status).toBe("unavailable");

      // All state-changing mutation commands fail closed (REQ-CONFIG-012)
      expect(report.blocked_capabilities).toContain("state_mutation");
      expect(report.blocked_capabilities).toContain("action_resolve");
      expect(report.blocked_capabilities).toContain("action_retry");
      expect(report.blocked_capabilities).toContain("evidence_admission");

      // Emits canonical_source_unavailable error
      const gauntletError = report.errors.find((e) => e.code === "canonical_source_unavailable");
      expect(gauntletError).toBeDefined();
      expect(gauntletError?.source).toBe("gauntlet");
      expect(gauntletError?.blast_radius).toContain("All state-changing commands fail closed");
      expect(gauntletError?.recoverability_hint).toContain("gauntlet serve");
    });
  });
});
