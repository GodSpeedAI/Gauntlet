/**
 * Teeth for kernel callable affordances (plan T11 gate suite).
 *
 * Attacks the claim: kernel functions representing Gauntlet operations invoke
 * the SAME canonical gateway as other projections; unavailable operations
 * cannot be called through a privileged kernel side path; wrapper
 * invalidation/refresh never bypasses gates; kernel input returns through the
 * Prime mediation path.
 *
 * The canonical admission point of this suite is the T09 CanonicalSurfaceStub
 * (the ONE admission body) — side-effect counters make "zero side effect"
 * measurable, exactly as in T09's deny-bypass matrix.
 */

import { describe, expect, it } from "bun:test";
import { FocusRef } from "./focus";
import {
  type KernelAffordanceContext,
  KernelAffordanceDirectory,
  KernelInputBroker,
} from "./kernel-affordances";
import { OperationGateway } from "./operation-gateway";
import { lookupOperation, OPERATION_IDS } from "./operation-registry";
import { PrimeReplKernel } from "./prime-kernel";
import { CanonicalSurfaceStub } from "./testing/canonical-surface-stub";

const SESSION = { actor: "operator-1", token: "token-1" } as const;

function makeUnit(stub: CanonicalSurfaceStub, unitId: string, ticket: string | null): void {
  stub.addUnit({
    unit_id: unitId,
    status: "blocked",
    tier: "P2",
    settled: false,
    authority: "granted",
    authority_visible_in_reads: true,
    governance_denied: false,
    governance_visible_in_reads: true,
    open_ticket: ticket,
  });
}

function makeDirectory(
  kernel: PrimeReplKernel,
  gateway: OperationGateway,
  stub: CanonicalSurfaceStub,
  overrides: Partial<KernelAffordanceContext> = {},
): KernelAffordanceDirectory {
  const context: KernelAffordanceContext = {
    generation: overrides.generation ?? "gen-1",
    horizon_id: overrides.horizon_id !== undefined ? overrides.horizon_id : null,
    kernel_generation: overrides.kernel_generation ?? "kgen-1",
    session_ref: overrides.session_ref ?? kernel.sessionId,
  };
  return new KernelAffordanceDirectory({
    kernel,
    gateway,
    route: stub.asInProcessRoute(),
    context,
  });
}

describe("kernel affordance namespace (REQ-AFF-002, REQ-OUT-004)", () => {
  it("generates callables FROM the registry: consequential actions and read helpers are separate, disjoint surfaces", () => {
    const kernel = new PrimeReplKernel("k-ns");
    const stub = new CanonicalSurfaceStub({
      run_id: "run-1",
      bound_actor: SESSION.actor,
      session_token: SESSION.token,
    });
    const gateway = new OperationGateway(stub.asInProcessRoute());
    const directory = makeDirectory(kernel, gateway, stub);
    const set = directory.current();

    // Consequential callables: exactly the registry's action-admitted operations.
    expect(set.actions.map((callable) => callable.operation)).toEqual([
      OPERATION_IDS.UNIT_RESOLVE,
      OPERATION_IDS.UNIT_RETRY,
    ]);
    // Read helpers: exactly the registry's read-shaped operations, separate list.
    expect(set.reads.map((helper) => helper.operation)).toEqual([OPERATION_IDS.STATE_REFRESH]);

    // Disjoint types: an action carries no read method; a read helper carries no
    // invoke (the `in` checks cover the whole prototype chain).
    const action = set.action(OPERATION_IDS.UNIT_RESOLVE);
    const read = set.read(OPERATION_IDS.STATE_REFRESH);
    expect(action).not.toBeNull();
    expect(read).not.toBeNull();
    if (action === null || read === null) {
      throw new Error("unreachable");
    }
    expect(action.kind).toBe("consequential");
    expect(read.kind).toBe("read");
    expect(read.affects_canonical_state).toBe(false);
    expect("invoke" in read).toBe(false);
    expect("read" in action).toBe(false);

    // Every callable carries the canonical registry spec, verbatim.
    const resolveSpec = lookupOperation(OPERATION_IDS.UNIT_RESOLVE);
    const refreshSpec = lookupOperation(OPERATION_IDS.STATE_REFRESH);
    if (resolveSpec === null || refreshSpec === null) {
      throw new Error("registry lookup failed");
    }
    expect(action.spec).toEqual(resolveSpec);
    expect(read.spec).toEqual(refreshSpec);
    expect(read.spec.consequential).toBe(false);

    // There is no callable for an operation the canonical surface does not serve.
    expect(set.action("state.refresh")).toBeNull();
    expect(set.read(OPERATION_IDS.UNIT_RESOLVE)).toBeNull();
    expect(set.action("session.cancel")).toBeNull();
  });

  it("routes one allowed invocation through the canonical gateway with the identical accepted shape as a direct projection", async () => {
    // Two identically-configured admissions: one serving the kernel route, one
    // serving a direct (non-kernel) projection. The outcomes must converge
    // byte-for-byte on the canonical accepted shape (REQ-AFF-002).
    const kernelStub = new CanonicalSurfaceStub({
      run_id: "run-1",
      bound_actor: SESSION.actor,
      session_token: SESSION.token,
    });
    makeUnit(kernelStub, "ESC-100", "TKT-100");
    const directStub = new CanonicalSurfaceStub({
      run_id: "run-1",
      bound_actor: SESSION.actor,
      session_token: SESSION.token,
    });
    makeUnit(directStub, "ESC-100", "TKT-100");

    const kernel = new PrimeReplKernel("k-allowed");
    const directory = makeDirectory(
      kernel,
      new OperationGateway(kernelStub.asInProcessRoute()),
      kernelStub,
    );
    const wrapper = directory.current().action(OPERATION_IDS.UNIT_RESOLVE);
    if (wrapper === null) {
      throw new Error("unit.resolve wrapper missing");
    }
    const focus = FocusRef.for("gauntlet", "WorkItem", "ESC-100");

    const kernelOutcome = await wrapper.invoke({
      focus,
      session: SESSION,
      payload: { note: "resolve from kernel" },
    });
    const directOutcome = await new OperationGateway(directStub.asInProcessRoute()).invoke({
      operation: OPERATION_IDS.UNIT_RESOLVE,
      focus,
      session: SESSION,
      payload: { note: "resolve from kernel" },
    });

    // EXACT convergence — including the deterministic idempotency key.
    expect(kernelOutcome).toEqual(directOutcome);
    expect(kernelOutcome).toMatchObject({
      class: "accepted",
      committed_now: true,
      unit_id: "ESC-100",
    });

    // The kernel route reached the ONE admission body — once.
    expect(kernelStub.sideEffects).toEqual({
      settlements_committed: 1,
      tickets_resolved: 1,
      admission_attempts: 1,
    });
    expect(directStub.sideEffects).toEqual({
      settlements_committed: 1,
      tickets_resolved: 1,
      admission_attempts: 1,
    });
  });
});

describe("Tooth 1 — authority revoked after wrapper creation (plan falsifier)", () => {
  it("the stale wrapper re-enters canonical admission, is denied BEFORE any side effect, and the wrapper is invalidated", async () => {
    const stub = new CanonicalSurfaceStub({
      run_id: "run-1",
      bound_actor: SESSION.actor,
      session_token: SESSION.token,
    });
    makeUnit(stub, "ESC-200", "TKT-200");
    const kernel = new PrimeReplKernel("k-tooth1");
    const directory = makeDirectory(kernel, new OperationGateway(stub.asInProcessRoute()), stub);
    const staleSet = directory.current();
    const staleWrapper = staleSet.action(OPERATION_IDS.UNIT_RESOLVE);
    if (staleWrapper === null) {
      throw new Error("unit.resolve wrapper missing");
    }
    expect(staleWrapper.state).toBe("current");

    // Authority is revoked AFTER the wrapper was created.
    stub.revokeAuthority("ESC-200", true);

    const outcome = await staleWrapper.invoke({
      focus: FocusRef.for("gauntlet", "WorkItem", "ESC-200"),
      session: SESSION,
      payload: { note: "stale kernel callable" },
    });

    // Same typed canonical denial semantics as every other projection: refused
    // pre-admission, canonical reason preserved.
    expect(outcome.class).toBe("refused");
    if (outcome.class !== "refused") {
      throw new Error("unreachable");
    }
    expect(outcome.stale).toBe(true);
    expect(outcome.code).toBe("stale_affordance_denied");
    expect(outcome.reason_codes).toContain("authority_not_permitted");

    // Denied BEFORE any side effect — the admission body was never reached.
    expect(stub.sideEffects).toEqual({
      settlements_committed: 0,
      tickets_resolved: 0,
      admission_attempts: 0,
    });

    // Wrapper existence never bypassed the gates, and the projection records it.
    expect(staleWrapper.state).toBe("invalidated");
    expect(staleWrapper.invalidation?.canonical_code).toBe("stale_affordance_denied");
    expect(staleSet.state).toBe("invalidated");
  });

  it("a revoked wrapper stays revoked for every later call, and the refreshed set regenerates from the registry", async () => {
    const stub = new CanonicalSurfaceStub({
      run_id: "run-1",
      bound_actor: SESSION.actor,
      session_token: SESSION.token,
    });
    makeUnit(stub, "ESC-210", "TKT-210");
    stub.revokeAuthority("ESC-210", true);
    const kernel = new PrimeReplKernel("k-tooth1b");
    const directory = makeDirectory(kernel, new OperationGateway(stub.asInProcessRoute()), stub);

    const first = await directory
      .current()
      .action(OPERATION_IDS.UNIT_RESOLVE)
      ?.invoke({
        focus: FocusRef.for("gauntlet", "WorkItem", "ESC-210"),
        session: SESSION,
        payload: { note: "first" },
      });
    expect(first?.class).toBe("refused");

    // The refreshed projection still routes through the gateway — and the
    // canonical surface still denies the revoked unit.
    const refreshed = directory.notifyAuthorityChanged();
    expect(refreshed.isCurrent()).toBe(true);
    expect(directory.current()).toBe(refreshed);
    const second = await refreshed.action(OPERATION_IDS.UNIT_RESOLVE)?.invoke({
      focus: FocusRef.for("gauntlet", "WorkItem", "ESC-210"),
      session: SESSION,
      payload: { note: "second" },
    });
    expect(second?.class).toBe("refused");
    if (second?.class === "refused") {
      expect(second.reason_codes).toContain("authority_not_permitted");
    }
    expect(stub.sideEffects).toEqual({
      settlements_committed: 0,
      tickets_resolved: 0,
      admission_attempts: 0,
    });
  });
});

describe("deny-matrix re-run over the kernel route", () => {
  it("a denied operation through the kernel callable converges on the IDENTICAL typed canonical refusal as a direct projection", async () => {
    const kernelStub = new CanonicalSurfaceStub({
      run_id: "run-1",
      bound_actor: SESSION.actor,
      session_token: SESSION.token,
    });
    makeUnit(kernelStub, "ESC-300", "TKT-300");
    kernelStub.revokeAuthority("ESC-300", true);
    const directStub = new CanonicalSurfaceStub({
      run_id: "run-1",
      bound_actor: SESSION.actor,
      session_token: SESSION.token,
    });
    makeUnit(directStub, "ESC-300", "TKT-300");
    directStub.revokeAuthority("ESC-300", true);

    const kernel = new PrimeReplKernel("k-denied");
    const directory = makeDirectory(
      kernel,
      new OperationGateway(kernelStub.asInProcessRoute()),
      kernelStub,
    );
    const wrapper = directory.current().action(OPERATION_IDS.UNIT_RETRY);
    if (wrapper === null) {
      throw new Error("unit.retry wrapper missing");
    }
    const focus = FocusRef.for("gauntlet", "WorkItem", "ESC-300");

    const kernelOutcome = await wrapper.invoke({
      focus,
      session: SESSION,
      payload: { note: "kernel deny" },
    });
    const directOutcome = await new OperationGateway(directStub.asInProcessRoute()).invoke({
      operation: OPERATION_IDS.UNIT_RETRY,
      focus,
      session: SESSION,
      payload: { note: "kernel deny" },
    });

    // Same typed canonical denial semantics — byte-for-byte, key included.
    expect(kernelOutcome).toEqual(directOutcome);
    expect(kernelOutcome).toMatchObject({ class: "refused", code: "stale_affordance_denied" });
    if (kernelOutcome.class === "refused") {
      expect(kernelOutcome.reason_codes).toContain("authority_not_permitted");
    }

    // Zero side effect on both routes; the kernel path contacted admission zero times.
    expect(kernelStub.sideEffects).toEqual({
      settlements_committed: 0,
      tickets_resolved: 0,
      admission_attempts: 0,
    });
    expect(directStub.sideEffects).toEqual({
      settlements_committed: 0,
      tickets_resolved: 0,
      admission_attempts: 0,
    });
  });

  it("a read-shaped operation cannot be driven through the action path from the kernel: typed not_action_admitted, zero side effect", async () => {
    const stub = new CanonicalSurfaceStub({
      run_id: "run-1",
      bound_actor: SESSION.actor,
      session_token: SESSION.token,
    });
    makeUnit(stub, "ESC-310", "TKT-310");
    const kernel = new PrimeReplKernel("k-read");
    const gateway = new OperationGateway(stub.asInProcessRoute());
    const directory = makeDirectory(kernel, gateway, stub);
    const set = directory.current();

    // The namespace never generated an action callable for the read-shaped op.
    expect(set.action(OPERATION_IDS.STATE_REFRESH)).toBeNull();

    // Even forcing the read operation through the action admission path earns
    // the typed canonical refusal — never a fallback, zero side effects.
    const forced = await gateway.invoke({
      operation: OPERATION_IDS.STATE_REFRESH,
      focus: FocusRef.for("gauntlet", "WorkItem", "ESC-310"),
      session: SESSION,
      payload: { note: "kernel read-as-action attempt" },
    });
    expect(forced).toMatchObject({ class: "refused", code: "not_action_admitted", stale: true });
    expect(stub.sideEffects).toEqual({
      settlements_committed: 0,
      tickets_resolved: 0,
      admission_attempts: 0,
    });
  });

  it("read helpers are served by the canonical READ path only, and an unobservable read fails closed without touching admission", async () => {
    const stub = new CanonicalSurfaceStub({
      run_id: "run-1",
      bound_actor: SESSION.actor,
      session_token: SESSION.token,
    });
    makeUnit(stub, "ESC-320", "TKT-320");
    const kernel = new PrimeReplKernel("k-read2");
    const directory = makeDirectory(kernel, new OperationGateway(stub.asInProcessRoute()), stub);
    const read = directory.current().read(OPERATION_IDS.STATE_REFRESH);
    if (read === null) {
      throw new Error("state.refresh helper missing");
    }

    const outcome = await read.read({ unit: null });
    expect(outcome.class).toBe("observation");
    expect(outcome.affects_canonical_state).toBe(false);
    if (outcome.class === "observation") {
      expect(outcome.observation.reachable).toBe(true);
      expect(outcome.observation.units.map((unit) => unit.unit_id)).toContain("ESC-320");
    }
    // A read never reached the action admission body.
    expect(stub.sideEffects).toEqual({
      settlements_committed: 0,
      tickets_resolved: 0,
      admission_attempts: 0,
    });

    // Transport loss: typed read_unavailable, no fabricated observation.
    stub.setUnreachable();
    const lost = await read.read({ unit: null });
    expect(lost).toMatchObject({
      class: "refused",
      code: "read_unavailable",
      affects_canonical_state: false,
    });
    expect(stub.sideEffects).toEqual({
      settlements_committed: 0,
      tickets_resolved: 0,
      admission_attempts: 0,
    });
  });
});

describe("wrapper invalidation/refresh on generation/Horizon/authority change", () => {
  it("invalidates the old set on generation change, regenerates for the new generation, and stale wrappers still route through the gateway", async () => {
    const stub = new CanonicalSurfaceStub({
      run_id: "run-1",
      bound_actor: SESSION.actor,
      session_token: SESSION.token,
    });
    makeUnit(stub, "ESC-400", "TKT-400");
    const kernel = new PrimeReplKernel("k-gen");
    const directory = makeDirectory(kernel, new OperationGateway(stub.asInProcessRoute()), stub);
    const oldSet = directory.current();
    expect(oldSet.context.generation).toBe("gen-1");

    const newSet = directory.notifyGenerationChanged("gen-2");
    expect(newSet).not.toBe(oldSet);
    expect(newSet.context.generation).toBe("gen-2");
    expect(newSet.isCurrent()).toBe(true);
    expect(oldSet.isCurrent()).toBe(false);
    expect(oldSet.state).toBe("stale");
    expect(oldSet.stale_reasons.join(" ")).toContain("gen-1 -> gen-2");

    // The OLD set's wrappers still re-enter canonical admission: stale wrapper
    // + denied unit = the same typed refusal, never a bypass and never a crash.
    stub.revokeAuthority("ESC-400", true);
    const staleWrapper = oldSet.action(OPERATION_IDS.UNIT_RESOLVE);
    if (staleWrapper === null) {
      throw new Error("unit.resolve wrapper missing");
    }
    const outcome = await staleWrapper.invoke({
      focus: FocusRef.for("gauntlet", "WorkItem", "ESC-400"),
      session: SESSION,
      payload: { note: "stale generation callable" },
    });
    expect(outcome).toMatchObject({
      class: "refused",
      code: "stale_affordance_denied",
      stale: true,
    });
    expect(stub.sideEffects).toEqual({
      settlements_committed: 0,
      tickets_resolved: 0,
      admission_attempts: 0,
    });

    // A same-generation notification is a no-op (no needless churn).
    expect(directory.notifyGenerationChanged("gen-2")).toBe(newSet);
  });

  it("invalidates and refreshes on Horizon change", () => {
    const stub = new CanonicalSurfaceStub({
      run_id: "run-1",
      bound_actor: SESSION.actor,
      session_token: SESSION.token,
    });
    const kernel = new PrimeReplKernel("k-horizon");
    const directory = makeDirectory(kernel, new OperationGateway(stub.asInProcessRoute()), stub);
    const oldSet = directory.current();
    expect(oldSet.context.horizon_id).toBeNull();

    const newSet = directory.notifyHorizonChanged("horizon:hoisted");
    expect(newSet).not.toBe(oldSet);
    expect(newSet.context.horizon_id).toBe("horizon:hoisted");
    expect(oldSet.state).toBe("stale");
    expect(oldSet.stale_reasons.join(" ")).toContain("horizon changed");
    expect(directory.notifyHorizonChanged("horizon:hoisted")).toBe(newSet);
  });

  it("carries kernel/session provenance on every generated set", () => {
    const stub = new CanonicalSurfaceStub({
      run_id: "run-1",
      bound_actor: SESSION.actor,
      session_token: SESSION.token,
    });
    const kernel = new PrimeReplKernel("k-prov");
    const directory = makeDirectory(kernel, new OperationGateway(stub.asInProcessRoute()), stub, {
      kernel_generation: "kgen-42",
    });
    const set = directory.current();
    expect(set.context.kernel_generation).toBe("kgen-42");
    expect(set.context.session_ref).toBe("k-prov");
    expect(directory.describe()).toEqual({
      kernel_session: "k-prov",
      kernel_generation: "kgen-42",
      sets_generated: 1,
    });
  });
});

describe("kernel input through Attention/Prime mediation (REQ-PRIME-006)", () => {
  it("surfaces input needs through the registered Attention sink and returns the answer through the serialized kernel path", async () => {
    const kernel = new PrimeReplKernel("k-input");
    const captured: string[] = [];
    const broker = new KernelInputBroker({
      kernel,
      attention_sink: (request) => {
        captured.push(request.request_id);
      },
      kernel_generation: "kgen-1",
    });

    // Request: routed to Attention, NEVER directly into the kernel.
    const delivery = broker.request("operator_choice", "resolve or retry?");
    expect(delivery.class).toBe("queued_for_attention");
    if (delivery.class === "queued_for_attention") {
      expect(delivery.mediation_path).toBe("workbench_attention");
      expect(delivery.returns_through).toBe("prime_python_repl_mediation");
    }
    expect(captured).toHaveLength(1);
    expect(kernel.getObject("operator_choice")).toBeNull(); // nothing executed yet

    // Response: delivered INTO the kernel through the Prime-owned mediation path.
    const requestId = captured[0] ?? "";
    const resolution = await broker.respond(requestId, "True");
    expect(resolution.class).toBe("delivered_via_kernel_mediation");
    if (resolution.class === "delivered_via_kernel_mediation") {
      expect(resolution.execution.status).toBe("success");
      expect(resolution.execution.execution_order).toBe(1);
    }
    expect(kernel.getObject("operator_choice")?.name).toBe("operator_choice");

    // One answer per request; unknown requests are refused, not invented.
    const again = await broker.respond(requestId, "False");
    expect(again).toMatchObject({ class: "refused", code: "already_responded" });
    const unknown = await broker.respond("kir-never-issued", "1");
    expect(unknown).toMatchObject({ class: "refused", code: "unknown_request" });
  });

  it("fails closed with NO attention sink: the request stays pending and nothing is prompted or executed", async () => {
    const kernel = new PrimeReplKernel("k-nosink");
    const broker = new KernelInputBroker({
      kernel,
      attention_sink: null,
      kernel_generation: "kgen-1",
    });

    const delivery = broker.request("missing_decision", "need operator input");
    expect(delivery).toMatchObject({
      class: "unroutable",
      reason: "no_attention_sink_registered",
      mediation_path: "none",
    });
    expect(broker.pending().map((request) => request.name)).toEqual(["missing_decision"]);
    // No direct prompting path exists: the kernel saw nothing.
    expect(kernel.getObject("missing_decision")).toBeNull();
  });
});
