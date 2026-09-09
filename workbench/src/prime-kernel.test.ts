import { describe, expect, it } from "bun:test";
import { PrimeReplKernel } from "./prime-kernel";

describe("PrimeReplKernel (REQ-PRIME-004, REQ-PRIME-006, REQ-SAFE-004)", () => {
  it("executes code sequentially and tracks ephemeral objects", async () => {
    const kernel = new PrimeReplKernel("test-session-1");
    const result = await kernel.execute("x = [1, 2, 3]\ny = {'a': 1}");

    expect(result.status).toBe("success");
    expect(result.execution_order).toBe(1);
    expect(result.created_objects.length).toBe(2);

    const objX = kernel.getObject("x");
    expect(objX).not.toBeNull();
    expect(objX?.name).toBe("x");
    expect(objX?.lifecycle).toBe("ephemeral");
    expect(objX?.type_name).toBe("list");
  });

  it("Attack 1: serializes concurrent execution requests without interleaving or race conditions", async () => {
    const kernel = new PrimeReplKernel("test-session-concurrent");

    // Launch two concurrent executions without awaiting the first
    const p1 = kernel.execute("a = 1");
    const p2 = kernel.execute("b = 2");

    const [r1, r2] = await Promise.all([p1, p2]);

    expect(r1.status).toBe("success");
    expect(r2.status).toBe("success");
    expect(r1.execution_order).toBe(1);
    expect(r2.execution_order).toBe(2);
    expect(r1.execution_order).toBeLessThan(r2.execution_order);
  });

  it("handles execution interrupt requests", async () => {
    const kernel = new PrimeReplKernel("test-session-interrupt");

    kernel.interrupt();
    const result = await kernel.execute("while True: pass");

    expect(result.status).toBe("interrupted");
    expect(result.error).toContain("interrupted");
  });

  it("transitions object lifecycle: ephemeral -> pinned -> artifactized", async () => {
    const kernel = new PrimeReplKernel("test-session-lifecycle");
    await kernel.execute("model_weights = [0.1, 0.2]");

    const ephemeralObj = kernel.getObject("model_weights");
    expect(ephemeralObj?.lifecycle).toBe("ephemeral");

    // Pin object in memory
    const pinned = kernel.transitionLifecycle("model_weights", "pinned");
    expect(pinned).toBe(true);
    expect(kernel.getObject("model_weights")?.lifecycle).toBe("pinned");

    // Artifactize object to persistent storage
    const evidencePath = ".agents/reports/artifacts/model_weights.json";
    const artifactized = kernel.transitionLifecycle("model_weights", "artifactized", evidencePath);
    expect(artifactized).toBe(true);

    const artObj = kernel.getObject("model_weights");
    expect(artObj?.lifecycle).toBe("artifactized");
    expect(artObj?.persisted_path).toBe(evidencePath);
  });

  it("Attack 2 (REQ-SAFE-004): kernel liveness != work success", async () => {
    const kernel = new PrimeReplKernel("test-session-settlement");
    await kernel.execute("result = {'accuracy': 0.99}");

    // Kernel is alive and has state, but no artifactized evidence path is provided
    const check1 = kernel.canSettleUnit("AK-001");
    expect(check1.allowed).toBe(false);
    expect(check1.reason).toContain("verified artifactized evidence path is required");

    // Providing an unartifactized evidence path is rejected
    const check2 = kernel.canSettleUnit("AK-001", ".agents/reports/artifacts/missing.json");
    expect(check2.allowed).toBe(false);
    expect(check2.reason).toContain("has not been artifactized from kernel state");

    // Artifactize the object to the expected evidence path
    kernel.transitionLifecycle("result", "artifactized", ".agents/reports/artifacts/result.json");
    const check3 = kernel.canSettleUnit("AK-001", ".agents/reports/artifacts/result.json");
    expect(check3.allowed).toBe(true);
    expect(check3.reason).toContain("Artifactized evidence verified");

    // Terminated kernel cannot settle work
    kernel.terminate();
    const check4 = kernel.canSettleUnit("AK-001", ".agents/reports/artifacts/result.json");
    expect(check4.allowed).toBe(false);
    expect(check4.reason).toContain("Kernel is terminated or dead");
  });
});
