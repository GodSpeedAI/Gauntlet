/**
 * T10 recursive-delegation suites (plan teeth + deny-matrix re-run; REQ-OPS-001,
 * REQ-PRIME-001..005, REQ-GOAL-003; prereg P1/D2/D5, transfer tests L1/L2).
 *
 * The gate selector `just tui-test prime-recursion` selects this file by name.
 * Proofs: nested delegation reflects the ADAPTER's real topology/ids at every
 * depth (nothing minted); the admission handle is immediate and never a result;
 * depth/budget bounds refuse with typed, side-effect-free refusals; deterministic
 * death between canonical admission and spawn (both sides of the boundary)
 * leaves no orphan; and the T09 deny matrix re-runs over the NEW delegation
 * route — a denied/stale operation reaches the SAME typed denial with zero
 * Prime side effect.
 */

import { describe, expect, it } from "bun:test";
import { FocusRef } from "./focus";
import { createHorizon, planMaterialization } from "./horizon";
import type { OperationInvocationRequest, OperatorActionRoute } from "./operation-gateway";
import { OPERATION_IDS } from "./operation-registry";
import {
  constructBoundedContextEnvelope,
  constructWholeCaseEnvelope,
} from "./prime-context-envelope";
import { PrimeHostAdapter } from "./prime-host";
import {
  DEFAULT_RECURSION_POLICY,
  delegationCapabilityFacts,
  PrimeDelegationDispatcher,
  projectHostToSessionRecords,
  sessionDepth,
} from "./prime-recursion";
import { reconstructTopology } from "./prime-topology";
import type { ProjectionWorld } from "./relations";
import { CanonicalSurfaceStub } from "./testing/canonical-surface-stub";

const GEN = "gen-t10-recursion";
const RUN = "01JT10RECURSIONRUN01";
const TOKEN = "t10-recursion-token";
const ACTOR = "operator-dave";

function resolvableStub(unitId = "AK-201", ticket = "ESC-201"): CanonicalSurfaceStub {
  return new CanonicalSurfaceStub({
    run_id: RUN,
    bound_actor: ACTOR,
    session_token: TOKEN,
  }).addUnit({
    unit_id: unitId,
    status: "blocked",
    tier: "P3",
    settled: false,
    authority: "granted",
    authority_visible_in_reads: true,
    governance_denied: false,
    governance_visible_in_reads: true,
    open_ticket: ticket,
  });
}

function stubWithUnit(record: {
  unitId: string;
  ticket: string | null;
  authority: "granted" | "revoked";
  authorityVisible: boolean;
}): CanonicalSurfaceStub {
  return new CanonicalSurfaceStub({
    run_id: RUN,
    bound_actor: ACTOR,
    session_token: TOKEN,
  }).addUnit({
    unit_id: record.unitId,
    status: "blocked",
    tier: "P3",
    settled: false,
    authority: record.authority,
    authority_visible_in_reads: record.authorityVisible,
    governance_denied: false,
    governance_visible_in_reads: true,
    open_ticket: record.ticket,
  });
}

function request(unitId: string): OperationInvocationRequest {
  return {
    operation: OPERATION_IDS.UNIT_RESOLVE,
    focus: FocusRef.for("gauntlet", "WorkItem", unitId),
    session: { actor: ACTOR, token: TOKEN },
    // Anchored on the durable run: nested delegations of the SAME escalation
    // re-enter the §16.3 idempotent settlement (committed_now: false) — exactly
    // one canonical commit, many real Prime children.
    payload: { note: "recursive delegation of the escalation to a Prime child", run: RUN },
  };
}

function envelopeFor() {
  const world: ProjectionWorld = {
    generation: GEN,
    units: [],
    evidence: [],
    sessions: [],
    source_cursors: {},
  };
  const horizon = createHorizon({
    root_focus: FocusRef.for("gauntlet", "WorkItem", "u-delegated"),
    generation: GEN,
    relation_queries: [],
    horizon_id: "horizon-t10-recursion",
  });
  return constructBoundedContextEnvelope({
    horizon,
    plan: planMaterialization(world, horizon),
    role_instructions: ["bounded recursion probe"],
    constructed_at_ms: 1_790_000_000_000,
  });
}

interface DelegationSpec {
  readonly child: string;
  readonly parent: string | null;
}

function delegation(spec: DelegationSpec) {
  return {
    operation_request: request("AK-201"),
    envelope: envelopeFor(),
    child_session_id: spec.child,
    parent_session_id: spec.parent,
    task: `task for ${spec.child}`,
  };
}

describe("T10 Recursive Delegation — real topology, nonblocking admission, bounded depth (REQ-PRIME-001..004, REQ-GOAL-003)", () => {
  it("nested delegation reflects the ADAPTER's real topology/ids at every depth — no synthetic identities are minted anywhere", async () => {
    const adapter = new PrimeHostAdapter();
    // The root orchestrator session is Prime-owned (admitted directly on the
    // adapter), not a workbench invention.
    adapter.spawnChild({
      sessionId: "sess-root-orchestrator",
      parentSessionId: null,
      contractPath: ".agents/reports/root.result.json",
    });
    const dispatcher = new PrimeDelegationDispatcher({
      adapter,
      canonical_route: resolvableStub().asInProcessRoute(),
      policy: { max_depth: 3, max_children_per_parent: 4 },
    });

    const child = await dispatcher.delegate(
      delegation({ child: "sess-child-builder", parent: "sess-root-orchestrator" }),
    );
    const grandchild = await dispatcher.delegate(
      delegation({ child: "sess-grandchild-falsifier", parent: "sess-child-builder" }),
    );
    expect(child.class).toBe("admitted");
    expect(grandchild.class).toBe("admitted");

    // Handles carry the REAL parent links the caller declared and the adapter owns.
    if (child.class !== "admitted" || grandchild.class !== "admitted") {
      throw new Error("unreachable");
    }
    expect(child.handle.parent_session_id).toBe("sess-root-orchestrator");
    expect(grandchild.handle.parent_session_id).toBe("sess-child-builder");
    expect(grandchild.handle.session_id).toBe("sess-grandchild-falsifier");

    // The projected topology equals a fresh reconstruction from the adapter's
    // durable records — same ids, same wiring, nothing synthesized.
    const topology = dispatcher.topology();
    expect(topology).toEqual(reconstructTopologyFrom(adapter));
    const root = topology.find((node) => node.sessionId === "sess-root-orchestrator");
    expect(root?.children.map((node) => node.sessionId)).toEqual(["sess-child-builder"]);
    expect(root?.children[0]?.children.map((node) => node.sessionId)).toEqual([
      "sess-grandchild-falsifier",
    ]);

    // The adapter knows EXACTLY these four sessions... (three, plus nothing else)
    const adapterIds = adapter
      .listSessions()
      .map((session) => session.handle.session_id)
      .sort();
    expect(adapterIds).toEqual([
      "sess-child-builder",
      "sess-grandchild-falsifier",
      "sess-root-orchestrator",
    ]);

    // Depth is computed from the adapter's real parent chains.
    expect(sessionDepth(adapter, "sess-root-orchestrator")).toBe(0);
    expect(sessionDepth(adapter, "sess-child-builder")).toBe(1);
    expect(sessionDepth(adapter, "sess-grandchild-falsifier")).toBe(2);
    expect(sessionDepth(adapter, "sess-unknown")).toBeNull();

    // Each delegation record binds the canonical receipt to the real session.
    expect(child.record.canonical_run_id).toBe(RUN);
    expect(child.record.session_id).toBe("sess-child-builder");
    expect(child.record.operation).toBe(OPERATION_IDS.UNIT_RESOLVE);
    expect(child.record.parent_session_id).toBe("sess-root-orchestrator");
  });

  it("admission is nonblocking: the handle returns immediately, is never a result, and completion is observed independently (REQ-PRIME-003)", async () => {
    const adapter = new PrimeHostAdapter();
    const stub = resolvableStub();
    const dispatcher = new PrimeDelegationDispatcher({
      adapter,
      canonical_route: stub.asInProcessRoute(),
    });
    const start = performance.now();
    const outcome = await dispatcher.delegate(
      delegation({ child: "sess-fast-admission", parent: null }),
    );
    const elapsed = performance.now() - start;
    expect(elapsed).toBeLessThan(50); // immediate, nonblocking admission
    expect(outcome.class).toBe("admitted");
    if (outcome.class !== "admitted") {
      throw new Error("unreachable");
    }
    // The handle is an ADMISSION, not a result: running, no contract.
    expect(outcome.handle.status).toBe("running");
    expect(outcome.handle.contract_path).toBe(
      `${".agents/reports/gauntlet-tui-prime-workbench/evidence/T10/contracts"}/sess-fast-admission.result.json`,
    );
    expect(adapter.getSession("sess-fast-admission")?.resultContract).toBeNull();

    // Completion arrives INDEPENDENTLY, through the T04 result-contract contract.
    const completed = adapter.deliverResultContract("sess-fast-admission", {
      session_id: "sess-fast-admission",
      schema_valid: true,
      outcome: "pass",
      evidence_path:
        ".agents/reports/gauntlet-tui-prime-workbench/evidence/T10/contracts/sess-fast-admission.json",
      completed_at: "2026-09-09T00:00:00Z",
    });
    expect(completed).toBe(true);
    expect(adapter.getSession("sess-fast-admission")?.handle.status).toBe("completed");
  });

  it("depth and budget bounds refuse with TYPED, zero-side-effect refusals; unknown parents and duplicate ids are refused too", async () => {
    const adapter = new PrimeHostAdapter();
    adapter.spawnChild({
      sessionId: "sess-root",
      parentSessionId: null,
      contractPath: "root.result.json",
    });
    const stub = resolvableStub();
    const dispatcher = new PrimeDelegationDispatcher({
      adapter,
      canonical_route: stub.asInProcessRoute(),
      policy: { max_depth: 2, max_children_per_parent: 2 },
    });

    // Build to the depth limit: root(0) -> child(1) -> grandchild(2).
    expect(
      (await dispatcher.delegate(delegation({ child: "sess-child", parent: "sess-root" }))).class,
    ).toBe("admitted");
    expect(
      (await dispatcher.delegate(delegation({ child: "sess-grandchild", parent: "sess-child" })))
        .class,
    ).toBe("admitted");
    const sessionsBefore = adapter.listSessions().length;
    const attemptsBefore = stub.sideEffects.admission_attempts;

    // Depth 3 exceeds the policy: typed refusal, ZERO side effects (no gateway
    // drive, no spawn).
    const tooDeep = await dispatcher.delegate(
      delegation({ child: "sess-too-deep", parent: "sess-grandchild" }),
    );
    expect(tooDeep).toEqual({
      class: "refused_by_policy",
      code: "depth_exceeded",
      detail: expect.any(String),
    });
    expect(adapter.listSessions().length).toBe(sessionsBefore);
    expect(stub.sideEffects.admission_attempts).toBe(attemptsBefore);

    // Child budget: sess-child already holds 1 child; one more is allowed, the
    // third is refused.
    expect(
      (await dispatcher.delegate(delegation({ child: "sess-child-second", parent: "sess-child" })))
        .class,
    ).toBe("admitted");
    const overBudget = await dispatcher.delegate(
      delegation({ child: "sess-child-third", parent: "sess-child" }),
    );
    expect(overBudget).toEqual({
      class: "refused_by_policy",
      code: "child_budget_exhausted",
      detail: expect.any(String),
    });
    expect(
      adapter.listSessions().some((session) => session.handle.session_id === "sess-child-third"),
    ).toBe(false);

    // Unknown parent: no synthetic parent is minted.
    expect(
      await dispatcher.delegate(delegation({ child: "sess-ghost-child", parent: "sess-ghost" })),
    ).toEqual({
      class: "refused_by_policy",
      code: "parent_session_unknown",
      detail: expect.any(String),
    });

    // Duplicate id over an adapter-owned session: identity is adapter-owned.
    expect(
      await dispatcher.delegate(delegation({ child: "sess-child", parent: "sess-root" })),
    ).toEqual({
      class: "refused_by_policy",
      code: "duplicate_session_id",
      detail: expect.any(String),
    });

    // No envelope, no delegation: the envelope is REQUIRED.
    const envelopeless = delegation({ child: "sess-no-envelope", parent: null });
    expect(
      await dispatcher.delegate({ ...envelopeless, envelope: { schema: "bogus" } as never }),
    ).toEqual({
      class: "refused_by_policy",
      code: "envelope_required",
      detail: expect.any(String),
    });

    // None of the refusals touched the adapter or the canonical surface.
    expect(adapter.listSessions().length).toBe(sessionsBefore + 1); // +1 = the allowed second child
    expect(stub.sideEffects.admission_attempts).toBe(attemptsBefore + 1);
  });

  it("capability facts are honest: native depth/budget controls are recorded unsupported, never claimed as Prime capabilities", () => {
    const facts = delegationCapabilityFacts();
    const nativeDepth = facts.find(
      (fact) => fact.capability === "native_nested_delegation_depth_limit",
    );
    const nativeBudget = facts.find((fact) => fact.capability === "native_child_budget_accounting");
    expect(nativeDepth?.supported).toBe(false);
    expect(nativeDepth?.reason).toBe("not_exposed_by_installed_prime_adapter_model");
    expect(nativeBudget?.supported).toBe(false);
    // The workbench-side policy is real but is POLICY, not a Prime capability.
    expect(DEFAULT_RECURSION_POLICY.max_depth).toBeGreaterThan(0);
  });

  it("delegating with an explicitly disclosed whole-Case envelope is RECORDED as degraded — disclosure is provenance, never the default", async () => {
    const adapter = new PrimeHostAdapter();
    const stub = resolvableStub("AK-261", "ESC-261");
    const dispatcher = new PrimeDelegationDispatcher({
      adapter,
      canonical_route: stub.asInProcessRoute(),
    });
    const world: ProjectionWorld = {
      generation: GEN,
      units: [],
      evidence: [],
      sessions: [],
      source_cursors: {},
    };
    const wholeCasePlan = planMaterialization(world, null);
    const disclosed = constructWholeCaseEnvelope({
      plan: wholeCasePlan,
      generation: GEN,
      root_focus: FocusRef.for("gauntlet", "WorkItem", "u-delegated"),
      disclosure_reason: "operator explicitly authorized whole-Case context",
      constructed_at_ms: 1_790_000_000_000,
    });
    const outcome = await dispatcher.delegate({
      operation_request: request("AK-261"),
      envelope: disclosed,
      child_session_id: "sess-whole-case",
      parent_session_id: null,
      task: "disclosed whole-Case delegation",
    });
    expect(outcome.class).toBe("admitted");
    if (outcome.class !== "admitted") {
      throw new Error("unreachable");
    }
    // The degraded disclosure travels on the record and the child received it.
    expect(outcome.record.envelope_kind).toBe("whole_case_disclosed");
    const received = JSON.parse(
      adapter.getSession("sess-whole-case")?.messages[0]?.content ?? "",
    ) as { envelope: { kind: string; disclosures: string[] } };
    expect(received.envelope.kind).toBe("whole_case_disclosed");
    expect(received.envelope.disclosures.some((entry) => entry.includes("DEGRADED"))).toBe(true);
  });
});

describe("T10 Deterministic death between admission and spawn — no orphan on either side (REQ-OPS-001, prereg transfer L1)", () => {
  it("death AFTER canonical admission, BEFORE spawn: typed outcome_unknown under the SAME key; recovery re-drives that key and the child spawns exactly once", async () => {
    const adapter = new PrimeHostAdapter();
    const stub = resolvableStub();
    const inner = stub.asInProcessRoute();
    // Deterministic host death: the canonical admission commits, then the host
    // dies before the spawn boundary — the response is LOST, which is exactly
    // what the T09 gateway's outcome_unknown contract is for. The death is
    // ONE-SHOT: the recovery drive runs over a healthy transport (the TUI died,
    // not the canonical surface).
    let died = false;
    const dyingRoute: OperatorActionRoute = {
      observe: (target) => inner.observe(target),
      perform: async (command) => {
        const outcome = await inner.perform(command);
        if (!died) {
          died = true;
          throw new Error(`host died between admission and spawn: ${outcome.kind}`);
        }
        return outcome;
      },
    };
    const dispatcher = new PrimeDelegationDispatcher({ adapter, canonical_route: dyingRoute });
    const spec = delegation({ child: "sess-doomed", parent: null });
    const outcome = await dispatcher.delegate(spec);

    expect(outcome.class).toBe("refused_by_gateway");
    if (outcome.class !== "refused_by_gateway" || outcome.gateway.class !== "outcome_unknown") {
      throw new Error("expected outcome_unknown");
    }
    const key = outcome.gateway.idempotency_key;
    // The canonical admission IS durable (the receipt exists); Prime holds NO
    // half-spawned child and the ledger holds no record: no orphan state.
    expect(stub.sideEffects.settlements_committed).toBe(1);
    expect(adapter.listSessions().length).toBe(0);
    expect(dispatcher.listRecords().length).toBe(0);

    // Recovery: re-drive the SAME key with the SAME payload and context.
    const recovered = await dispatcher.recoverDelegation(spec, key);
    expect(recovered.class).toBe("admitted");
    if (recovered.class !== "admitted") {
      throw new Error("unreachable");
    }
    // §16.3 idempotent re-entry: the durable truth, nothing committed twice.
    expect(recovered.gateway.committed_now).toBe(false);
    expect(stub.sideEffects.settlements_committed).toBe(1);
    // Exactly ONE child now exists, with the real declared identity.
    expect(adapter.listSessions().length).toBe(1);
    expect(adapter.getSession("sess-doomed")?.handle.parent_session_id).toBeNull();
    expect(dispatcher.listRecords().length).toBe(1);
    expect(recovered.record.idempotency_key).toBe(key);

    // A second recovery attempt is refused: the outcome is no longer unknown.
    expect(await dispatcher.recoverDelegation(spec, key)).toEqual({
      class: "refused_by_policy",
      code: "outcome_not_unknown",
      detail: expect.any(String),
    });
    expect(adapter.listSessions().length).toBe(1); // still exactly one child

    // A never-issued key is refused.
    expect(
      await dispatcher.recoverDelegation(spec, "unit.resolve:AK-201:operator-dave:bogus"),
    ).toEqual({
      class: "refused_by_policy",
      code: "outcome_not_unknown",
      detail: expect.any(String),
    });
  });

  it("death AFTER spawn, BEFORE bookkeeping: the REAL child is recoverable from the adapter's topology — never re-minted, never killed", async () => {
    const adapter = new PrimeHostAdapter();
    const stub = resolvableStub();
    // Host A delegates successfully; the child lives in the ADAPTER (Prime side).
    const dispatcherA = new PrimeDelegationDispatcher({
      adapter,
      canonical_route: stub.asInProcessRoute(),
    });
    const admitted = await dispatcherA.delegate(
      delegation({ child: "sess-survivor", parent: null }),
    );
    expect(admitted.class).toBe("admitted");
    expect(dispatcherA.listRecords().length).toBe(1);

    // Host B = a FRESH host whose workbench ledger lost host A's record (death
    // after spawn, before the record was durable) but reattached to the same
    // Prime adapter.
    const dispatcherB = new PrimeDelegationDispatcher({
      adapter,
      canonical_route: stub.asInProcessRoute(),
    });
    const report = dispatcherB.reconcileLedger();
    expect(report.real_topology).toEqual(reconstructTopologyFrom(adapter));
    expect(report.mapped_delegations).toEqual([]);
    expect(report.unspawned_delegations).toEqual([]);
    expect(report.unmapped_children.map((node) => node.sessionId)).toEqual(["sess-survivor"]);
    expect(report.unmapped_children[0]?.contractPath).toContain("sess-survivor");

    // The survivor is adapter-owned: B cannot mint over it.
    expect(
      await dispatcherB.delegate(delegation({ child: "sess-survivor", parent: null })),
    ).toEqual({
      class: "refused_by_policy",
      code: "duplicate_session_id",
      detail: expect.any(String),
    });
    expect(adapter.listSessions().length).toBe(1);

    // Host A's own ledger maps cleanly: nothing unspawned, nothing unmapped.
    const reportA = dispatcherA.reconcileLedger();
    expect(reportA.mapped_delegations).toEqual([
      admitted.class === "admitted" ? admitted.delegation_id : "",
    ]);
    expect(reportA.unspawned_delegations).toEqual([]);
    expect(reportA.unmapped_children).toEqual([]);
  });
});

/**
 * T09 deny-matrix re-run over the NEW delegation route (one allowed + denied
 * operations). The dispatcher IS a gateway route: a denied/stale operation must
 * reach the SAME typed denial with ZERO Prime side effect — and the route's
 * canonical-before-spawn ordering must keep an admission-level refusal
 * child-free.
 */
describe("T10 deny-matrix re-run over the delegation route (REQ-AFF-002/003, REQ-CONFIG-011, REQ-SEC-002, REQ-VERIFY-004)", () => {
  it("allowed: the delegated operation converges on the canonical acceptance and admits exactly one child", async () => {
    const adapter = new PrimeHostAdapter();
    const stub = resolvableStub("AK-231", "ESC-231");
    const dispatcher = new PrimeDelegationDispatcher({
      adapter,
      canonical_route: stub.asInProcessRoute(),
    });
    const outcome = await dispatcher.delegate({
      operation_request: request("AK-231"),
      envelope: envelopeFor(),
      child_session_id: "sess-allowed",
      parent_session_id: null,
      task: "allowed delegation",
    });
    expect(outcome.class).toBe("admitted");
    if (outcome.class !== "admitted") {
      throw new Error("unreachable");
    }
    expect(outcome.gateway.class).toBe("accepted");
    expect(outcome.record.operation).toBe(OPERATION_IDS.UNIT_RESOLVE);
    expect(outcome.record.canonical_run_id).toBe(RUN);
    expect(outcome.record.envelope_kind).toBe("bounded");
    expect(stub.sideEffects.settlements_committed).toBe(1);
    expect(adapter.listSessions().map((session) => session.handle.session_id)).toEqual([
      "sess-allowed",
    ]);
  });

  it("denied (stale, visible): the SAME typed pre-admission denial as any route, with zero canonical and zero Prime side effect", async () => {
    const adapter = new PrimeHostAdapter();
    const stub = stubWithUnit({
      unitId: "AK-241",
      ticket: "ESC-241",
      authority: "revoked",
      authorityVisible: true,
    });
    const dispatcher = new PrimeDelegationDispatcher({
      adapter,
      canonical_route: stub.asInProcessRoute(),
    });
    const outcome = await dispatcher.delegate({
      operation_request: request("AK-241"),
      envelope: envelopeFor(),
      child_session_id: "sess-denied-stale",
      parent_session_id: null,
      task: "should never spawn",
    });
    expect(outcome.class).toBe("refused_by_gateway");
    if (outcome.class !== "refused_by_gateway") {
      throw new Error("unreachable");
    }
    const gateway = outcome.gateway;
    if (gateway.class !== "refused") {
      throw new Error("expected refused");
    }
    expect(gateway.code).toBe("stale_affordance_denied");
    expect(gateway.stale).toBe(true);
    expect(gateway.reason_codes).toContain("authority_not_permitted");
    // Zero side effect at BOTH boundaries: canonical admission never contacted,
    // no child exists.
    expect(stub.sideEffects.admission_attempts).toBe(0);
    expect(stub.sideEffects.settlements_committed).toBe(0);
    expect(adapter.listSessions().length).toBe(0);
    expect(dispatcher.listRecords().length).toBe(0);
  });

  it("denied (racing past revalidation): refused AT the admission body — and the route spawns NO child behind a canonical refusal", async () => {
    const adapter = new PrimeHostAdapter();
    const stub = stubWithUnit({
      unitId: "AK-251",
      ticket: "ESC-251",
      authority: "revoked",
      authorityVisible: false,
    });
    const dispatcher = new PrimeDelegationDispatcher({
      adapter,
      canonical_route: stub.asInProcessRoute(),
    });
    const outcome = await dispatcher.delegate({
      operation_request: request("AK-251"),
      envelope: envelopeFor(),
      child_session_id: "sess-denied-racing",
      parent_session_id: null,
      task: "should never spawn either",
    });
    expect(outcome.class).toBe("refused_by_gateway");
    if (outcome.class !== "refused_by_gateway") {
      throw new Error("unreachable");
    }
    const gateway = outcome.gateway;
    if (gateway.class !== "refused") {
      throw new Error("expected refused");
    }
    // The canonical refusal class, decided at the ONE admission body.
    expect(gateway.code).toBe("authority_not_permitted");
    expect(gateway.stale).toBe(false);
    expect(stub.sideEffects.admission_attempts).toBe(1);
    expect(stub.sideEffects.settlements_committed).toBe(0);
    // The ordering tooth: canonical admission ran FIRST and refused — the child
    // was never started (no orphan child behind a refusal).
    expect(adapter.listSessions().length).toBe(0);
    expect(dispatcher.listRecords().length).toBe(0);
  });
});

describe("T10 Correction Round 1 (fresh-target findings) — MINOR 3: serialized policy gates", () => {
  it("verifier C8 shape (RED-first): concurrent same-child drives are serialized inside the decision — exactly one spawn, progress never wiped", async () => {
    const adapter = new PrimeHostAdapter();
    const stub = resolvableStub();
    const dispatcher = new PrimeDelegationDispatcher({
      adapter,
      canonical_route: stub.asInProcessRoute(),
    });
    const spec = delegation({ child: "sess-race", parent: null });

    // Both drives fire concurrently: each is the verifier's C8 race.
    const [raceA, raceB] = await Promise.all([
      dispatcher.delegate(spec),
      dispatcher.delegate(spec),
    ]);

    // Exactly ONE drive admitted; the other is the typed duplicate refusal.
    const admitted = [raceA, raceB].filter((outcome) => outcome.class === "admitted");
    expect(admitted.length).toBe(1);
    const refused = raceA.class === "admitted" ? raceB : raceA;
    expect(refused.class).toBe("refused_by_policy");
    if (refused.class === "refused_by_policy") {
      expect(refused.code).toBe("duplicate_session_id");
    }

    // Zero double-spawn: exactly one session, one record, one admission attempt.
    expect(adapter.listSessions().length).toBe(1);
    expect(adapter.getSession("sess-race")).not.toBeNull();
    expect(dispatcher.listRecords().length).toBe(1);
    expect(stub.sideEffects.admission_attempts).toBe(1);
    expect(stub.sideEffects.settlements_committed).toBe(1);

    // The surviving child's progress is intact: exactly its ONE envelope delivery.
    expect(adapter.getSession("sess-race")?.messages.length).toBe(1);
    const received = JSON.parse(adapter.getSession("sess-race")?.messages[0]?.content ?? "") as {
      task: string;
    };
    const admittedDrive = admitted[0];
    if (admittedDrive?.class !== "admitted") {
      throw new Error("unreachable");
    }
    expect(received.task).toBe("task for sess-race");
  });

  it("verifier C8 two-dispatcher shape (RED-first): a racing drive from a SECOND dispatcher is refused by the spawn-time guard — the child is never reset", async () => {
    const adapter = new PrimeHostAdapter();
    const stub = resolvableStub();
    // Two dispatchers = two gateways over ONE adapter (the verifier's C8 setup):
    // separate decision queues, so the spawn-time guard is the last line.
    const dA = new PrimeDelegationDispatcher({
      adapter,
      canonical_route: stub.asInProcessRoute(),
    });
    const dB = new PrimeDelegationDispatcher({
      adapter,
      canonical_route: stub.asInProcessRoute(),
    });
    const spec = delegation({ child: "sess-race-two-hosts", parent: null });
    const [outcomeA, outcomeB] = await Promise.all([dA.delegate(spec), dB.delegate(spec)]);

    const loserIsA = outcomeA.class !== "admitted";
    const winner = loserIsA ? outcomeB : outcomeA;
    const loser = loserIsA ? outcomeA : outcomeB;
    const loserDispatcher = loserIsA ? dA : dB;

    // Exactly one admitted-with-spawn; the loser is the typed outcome_unknown
    // raised by the spawn-time duplicate guard.
    expect(winner.class).toBe("admitted");
    expect(loser.class).toBe("refused_by_gateway");
    if (loser.class !== "refused_by_gateway" || winner.class !== "admitted") {
      throw new Error("unreachable");
    }
    expect(loser.gateway.class).toBe("outcome_unknown");
    const loserKey = loser.gateway.idempotency_key;

    // ZERO double-spawn and ZERO reset: one session, its ONE context delivery
    // intact, exactly one record across both dispatchers.
    expect(adapter.listSessions().length).toBe(1);
    expect(adapter.getSession("sess-race-two-hosts")?.messages.length).toBe(1);
    const records = [...dA.listRecords(), ...dB.listRecords()];
    expect(records.length).toBe(1);
    expect(records[0]?.session_id).toBe("sess-race-two-hosts");
    expect(stub.sideEffects.settlements_committed).toBe(1);

    // Recovery of the lost drive is refused: an existing child is never re-minted.
    const recovered = await loserDispatcher.recoverDelegation(spec, loserKey);
    expect(recovered.class).toBe("refused_by_policy");
    if (recovered.class === "refused_by_policy") {
      expect(recovered.code).toBe("duplicate_session_id");
    }
    expect(adapter.listSessions().length).toBe(1);
    expect(adapter.getSession("sess-race-two-hosts")?.messages.length).toBe(1);
  });
});

/** T04 topology reconstruction over the adapter's projected records. */
function reconstructTopologyFrom(adapter: PrimeHostAdapter) {
  return reconstructTopology(projectHostToSessionRecords(adapter));
}
