/**
 * T10 context-envelope suites (plan teeth — Attack 1; REQ-OPS-004, REQ-PRIME-004,
 * REQ-GOAL-003, prereg P1/D1).
 *
 * The gate selector `just tui-test prime-context-envelope` selects this file by
 * name. Attack 1: two independent investigations spawned while hoisted to ONE
 * WorkItem each receive their OWN bounded envelope — received context is exactly
 * the declared horizon closure plus declared dependencies, with no whole-Case
 * leak; provenance (horizon id, generation, inclusion reasons) is inspectable.
 */

import { describe, expect, it } from "bun:test";
import { FocusRef } from "./focus";
import { createHorizon, planMaterialization } from "./horizon";
import type { OperationInvocationRequest } from "./operation-gateway";
import { OPERATION_IDS } from "./operation-registry";
import type { EnvelopeRoleBudget } from "./prime-context-envelope";
import {
  BUDGET_EXEMPT_ROLES,
  constructBoundedContextEnvelope,
  constructWholeCaseEnvelope,
  DEFAULT_ENVELOPE_ROLE_BUDGET,
  EnvelopeConstructionError,
  envelopeFromJSON,
  envelopeToJSON,
  MalformedEnvelopeError,
  PRIME_CONTEXT_ENVELOPE_SCHEMA,
} from "./prime-context-envelope";
import { PrimeHostAdapter } from "./prime-host";
import { PrimeDelegationDispatcher } from "./prime-recursion";
import type { EvidenceProjection, SessionProjection, UnitProjection } from "./projection-store";
import type { ProjectionWorld } from "./relations";
import { focusForEvidence, focusForSession, focusForUnit } from "./relations";
import { CanonicalSurfaceStub } from "./testing/canonical-surface-stub";

const GEN = "gen-t10-envelope";
const NOW_MS = 1_790_000_000_000;
const RUN = "01JT10ENVELOPERUN01";
const TOKEN = "t10-envelope-token";
const ACTOR = "operator-carol";

function unit(unitId: string, cursor: string, status: string, authority: string): UnitProjection {
  return {
    unit_id: unitId,
    generation_id: GEN,
    source_id: "gauntlet",
    source_cursor: cursor,
    title: unitId,
    phase: "execute",
    status,
    authority,
    blast_radius: "isolated",
    updated_at: NOW_MS,
  };
}

function evidence(
  evidenceId: string,
  unitId: string,
  cursor: string,
  verified: boolean,
): EvidenceProjection {
  return {
    evidence_id: evidenceId,
    generation_id: GEN,
    source_id: "gauntlet",
    source_cursor: cursor,
    unit_id: unitId,
    kind: "report",
    locator: `.agents/reports/${evidenceId}.json`,
    verified,
    updated_at: NOW_MS,
  };
}

function session(sessionId: string, parentId: string | null, cursor: string): SessionProjection {
  return {
    session_id: sessionId,
    generation_id: GEN,
    source_id: "gauntlet",
    source_cursor: cursor,
    parent_id: parentId,
    model: "model-a",
    status: "running",
    updated_at: NOW_MS,
  };
}

/** One Case: 3 units, 3 evidence, 2 sessions — more world than any child envelope may carry. */
function world(): ProjectionWorld {
  return {
    generation: GEN,
    units: [
      unit("u-root", "c-1", "active", "builder"),
      unit("u-other", "c-4", "active", "builder"),
      unit("u-far", "c-6", "blocked", "orchestrator"),
    ],
    evidence: [
      evidence("ev-1", "u-root", "c-2", true),
      evidence("ev-2", "u-root", "c-3", false),
      evidence("ev-3", "u-other", "c-5", true),
    ],
    sessions: [session("s-1", null, "c-7"), session("s-2", "s-1", "c-8")],
    source_cursors: { gauntlet: "c-9" },
  };
}

function hoistedHorizon() {
  return createHorizon({
    root_focus: FocusRef.for("gauntlet", "WorkItem", "u-root"),
    generation: GEN,
    relation_queries: [{ kind: "has_evidence", depth: 1, direction: "outbound" }],
    horizon_id: "horizon-t10-root",
  });
}

/** A canonical surface whose one unit resolves (the delegated operation succeeds). */
function resolvableStub(): CanonicalSurfaceStub {
  return new CanonicalSurfaceStub({
    run_id: RUN,
    bound_actor: ACTOR,
    session_token: TOKEN,
  }).addUnit({
    unit_id: "AK-101",
    status: "blocked",
    tier: "P3",
    settled: false,
    authority: "granted",
    authority_visible_in_reads: true,
    governance_denied: false,
    governance_visible_in_reads: true,
    open_ticket: "ESC-101",
  });
}

function resolveRequest(unitId: string): OperationInvocationRequest {
  return {
    operation: OPERATION_IDS.UNIT_RESOLVE,
    focus: FocusRef.for("gauntlet", "WorkItem", unitId),
    session: { actor: ACTOR, token: TOKEN },
    payload: { note: "delegate the investigation to an independent Prime child" },
  };
}

describe("T10 Prime Context Envelope — Attack 1: bounded envelopes, no whole-Case leak (REQ-OPS-004, REQ-PRIME-004)", () => {
  it("Attack 1: two independent investigations hoisted to one WorkItem each receive their OWN bounded envelope — exactly the declared closure + dependencies, never the whole Case", async () => {
    const caseWorld = world();
    const horizon = hoistedHorizon();
    const plan = planMaterialization(caseWorld, horizon);
    const wholeCase = planMaterialization(caseWorld, null);

    const adapter = new PrimeHostAdapter();
    // Two independent canonical surfaces (one per investigation — the stub's run
    // holds one open escalation at a time by canonical design), both admitting
    // children into the SAME Prime adapter.
    const stubA = resolvableStub();
    const stubB = resolvableStub();
    const dispatcherA = new PrimeDelegationDispatcher({
      adapter,
      canonical_route: stubA.asInProcessRoute(),
    });
    const dispatcherB = new PrimeDelegationDispatcher({
      adapter,
      canonical_route: stubB.asInProcessRoute(),
    });

    // Two INDEPENDENT investigations, same hoisted WorkItem, DIFFERENT explicit
    // dependencies — envelopes are constructed per child, never shared state.
    const envelopeA = constructBoundedContextEnvelope({
      horizon,
      plan,
      role_instructions: ["investigate the focused unit's evidence; report findings only"],
      required_dependencies: [
        {
          focus: FocusRef.for("gauntlet", "Evidence", "ev-shared-fixture"),
          reason: "the preregistered fixture this investigation must read",
        },
      ],
      envelope_id: "envelope-t10-a",
      constructed_at_ms: NOW_MS,
    });
    const envelopeB = constructBoundedContextEnvelope({
      horizon,
      plan,
      role_instructions: ["independently attack the focused unit's evidence closure"],
      required_dependencies: [
        {
          focus: FocusRef.for("gauntlet", "Reference", "answer-key-digest"),
          reason: "the frozen ruler digest the investigation is bounded by",
        },
      ],
      envelope_id: "envelope-t10-b",
      constructed_at_ms: NOW_MS,
    });
    expect(envelopeA.envelope_id).not.toBe(envelopeB.envelope_id);
    expect(envelopeA.declared_dependencies).not.toEqual(envelopeB.declared_dependencies);

    const outcomeA = await dispatcherA.delegate({
      operation_request: resolveRequest("AK-101"),
      envelope: envelopeA,
      child_session_id: "sess-investigation-a",
      parent_session_id: null,
      task: "investigation A",
    });
    // The second investigation is an INDEPENDENT dispatch (own canonical surface,
    // own child, own envelope) — the same hoisted WorkItem scopes both contexts.
    const outcomeB = await dispatcherB.delegate({
      operation_request: resolveRequest("AK-101"),
      envelope: envelopeB,
      child_session_id: "sess-investigation-b",
      parent_session_id: null,
      task: "investigation B",
    });
    expect(outcomeA.class).toBe("admitted");
    expect(outcomeB.class).toBe("admitted");
    if (outcomeA.class !== "admitted" || outcomeB.class !== "admitted") {
      throw new Error("unreachable");
    }
    expect(outcomeA.gateway.committed_now).toBe(true);
    expect(outcomeB.gateway.committed_now).toBe(true);

    // Received context per child: the FIRST mailbox message IS the envelope delivery.
    for (const [sessionId, envelope, task] of [
      ["sess-investigation-a", envelopeA, "investigation A"],
      ["sess-investigation-b", envelopeB, "investigation B"],
    ] as const) {
      const received = adapter.getSession(sessionId)?.messages[0];
      expect(received).toBeDefined();
      expect(received?.kind).toBe("communication");
      const parsed = JSON.parse(received?.content ?? "") as {
        received_context: string;
        task: string;
        envelope: {
          kind: string;
          horizon_id: string | null;
          horizon_generation: string;
          items: { focus: { id: string }; role: string; reason: string; mandatory: boolean }[];
        };
      };
      expect(parsed.received_context).toBe("prime-context-envelope/1");
      expect(parsed.task).toBe(task);

      // Provenance: horizon identity and generation travel with the envelope.
      expect(parsed.envelope.kind).toBe("bounded");
      expect(parsed.envelope.horizon_id).toBe(horizon.id);
      expect(parsed.envelope.horizon_generation).toBe(GEN);

      // Exactly the declared closure + dependencies — no whole-Case leak.
      const receivedIds = parsed.envelope.items.map((item) => item.focus.id).sort();
      const declaredClosure = plan.context_candidates.map((candidate) => candidate.focus.id);
      const declaredDependency = envelope.declared_dependencies.map((ref) => ref.id);
      expect(receivedIds).toEqual([...declaredClosure, ...declaredDependency].sort());

      // The whole Case is strictly bigger and NONE of it leaks.
      expect(wholeCase.context_candidates.length).toBeGreaterThan(parsed.envelope.items.length);
      for (const leaked of [
        "focus:gauntlet:WorkItem:u-other",
        "focus:gauntlet:WorkItem:u-far",
        "focus:gauntlet:Evidence:ev-3",
        "focus:gauntlet:Agent:s-1",
        "focus:gauntlet:Agent:s-2",
      ]) {
        expect(receivedIds).not.toContain(leaked);
      }

      // Mandatory closure survives hoisting (REQ-OPS-004).
      const roles = parsed.envelope.items.map((item) => item.role);
      expect(roles).toContain("horizon_root");
      expect(roles).toContain("mandatory_reference");
      expect(roles).toContain("mandatory_governance");
      expect(roles).toContain("mandatory_evidence");
      for (const item of parsed.envelope.items) {
        expect(item.reason.length).toBeGreaterThan(0);
      }
    }

    // The declared dependency arrived mandatory, with its caller-supplied reason.
    const itemsA = envelopeA.items;
    const dependencyItem = itemsA.find((item) => item.role === "declared_dependency");
    expect(dependencyItem?.focus.id).toBe("focus:gauntlet:Evidence:ev-shared-fixture");
    expect(dependencyItem?.mandatory).toBe(true);
    expect(dependencyItem?.reason).toBe("the preregistered fixture this investigation must read");

    // The canonical side effect of each delegation is exactly one accepted drive;
    // the children are real admitted sessions with nothing completed about them.
    expect(stubA.sideEffects.settlements_committed).toBe(1);
    expect(stubB.sideEffects.settlements_committed).toBe(1);
    expect(adapter.listSessions().length).toBe(2);
    expect(adapter.getSession("sess-investigation-a")?.handle.status).toBe("running");
    expect(adapter.getSession("sess-investigation-a")?.resultContract).toBeNull();
  });

  it("DEFAULT IS BOUNDED: the bounded constructor requires a horizon-matched plan and cannot be parameterized into a whole-Case dump", () => {
    const caseWorld = world();
    const horizon = hoistedHorizon();
    const wholeCasePlan = planMaterialization(caseWorld, null);

    // A whole-Case plan cannot pass as a hoisted envelope's plan.
    expect(() => constructBoundedContextEnvelope({ horizon, plan: wholeCasePlan })).toThrow(
      EnvelopeConstructionError,
    );

    // A malformed horizon cannot pass.
    expect(() =>
      constructBoundedContextEnvelope({
        horizon: { id: "bogus" } as unknown as Parameters<
          typeof constructBoundedContextEnvelope
        >[0]["horizon"],
        plan: planMaterialization(caseWorld, horizon),
      }),
    ).toThrow(EnvelopeConstructionError);

    // The bounded envelope's items are exactly plan candidates (+ declared deps):
    // there is no widening option to assert against — the constructor signature
    // itself has none (verified by construction here: only horizon/plan/
    // instructions/dependencies/ids are accepted).
    const envelope = constructBoundedContextEnvelope({
      horizon,
      plan: planMaterialization(caseWorld, horizon),
      constructed_at_ms: NOW_MS,
    });
    expect(envelope.items.map((item) => item.focus.id).sort()).toEqual(
      planMaterialization(caseWorld, horizon)
        .context_candidates.map((candidate) => candidate.focus.id)
        .sort(),
    );
    expect(envelope.disclosures).toEqual([]);
  });

  it("whole-Case context exists ONLY as an explicit, disclosed, degraded construction — never an implicit default", () => {
    const caseWorld = world();
    const hoistedPlan = planMaterialization(caseWorld, hoistedHorizon());
    const wholeCasePlan = planMaterialization(caseWorld, null);

    // Refuses a hoisted plan (cannot mislabel bounded context as whole-Case).
    expect(() =>
      constructWholeCaseEnvelope({
        plan: hoistedPlan,
        generation: GEN,
        root_focus: FocusRef.for("gauntlet", "WorkItem", "u-root"),
        disclosure_reason: "operator asked for everything",
      }),
    ).toThrow(EnvelopeConstructionError);

    // Refuses an empty disclosure reason (nothing implicit).
    expect(() =>
      constructWholeCaseEnvelope({
        plan: wholeCasePlan,
        generation: GEN,
        root_focus: FocusRef.for("gauntlet", "WorkItem", "u-root"),
        disclosure_reason: "   ",
      }),
    ).toThrow(EnvelopeConstructionError);

    const disclosed = constructWholeCaseEnvelope({
      plan: wholeCasePlan,
      generation: GEN,
      root_focus: FocusRef.for("gauntlet", "WorkItem", "u-root"),
      disclosure_reason: "operator explicitly authorized whole-Case context for this delegation",
      constructed_at_ms: NOW_MS,
    });
    expect(disclosed.kind).toBe("whole_case_disclosed");
    expect(disclosed.horizon_id).toBeNull();
    expect(disclosed.disclosures.length).toBeGreaterThanOrEqual(2);
    expect(disclosed.disclosures.some((entry) => entry.includes("DEGRADED"))).toBe(true);
    expect(disclosed.disclosures.some((entry) => entry.includes("explicit authorization"))).toBe(
      true,
    );
    // It really is the whole working world.
    expect(disclosed.items.length).toBe(wholeCasePlan.context_candidates.length);
  });

  it("envelopes are serializable artifacts: round-trip preserves fields and re-interns FocusRefs (never forks identity)", () => {
    const caseWorld = world();
    const horizon = hoistedHorizon();
    const envelope = constructBoundedContextEnvelope({
      horizon,
      plan: planMaterialization(caseWorld, horizon),
      required_dependencies: [
        {
          focus: FocusRef.for("gauntlet", "Reference", "answer-key-digest"),
          reason: "frozen ruler",
        },
      ],
      constructed_at_ms: NOW_MS,
    });
    const parsed = envelopeFromJSON(envelopeToJSON(envelope));
    expect(parsed).toEqual(envelope);
    expect(parsed.schema).toBe(PRIME_CONTEXT_ENVELOPE_SCHEMA);
    for (const item of parsed.items) {
      expect(item.focus).toBe(
        FocusRef.for(item.focus.source, item.focus.kind, item.focus.source_id),
      );
    }

    // Untrusted documents are validated, not trusted.
    expect(() => envelopeFromJSON('{"schema":"other/1"}')).toThrow(MalformedEnvelopeError);
    const tampered = JSON.parse(envelopeToJSON(envelope)) as Record<string, unknown>;
    tampered["kind"] = "whole_case_disclosed";
    tampered["horizon_id"] = null;
    tampered["disclosures"] = [];
    expect(() => envelopeFromJSON(JSON.stringify(tampered))).toThrow(MalformedEnvelopeError);
  });

  it("construction is deterministic for identical inputs", () => {
    const caseWorld = world();
    const horizon = hoistedHorizon();
    const options = {
      horizon,
      plan: planMaterialization(caseWorld, horizon),
      constructed_at_ms: NOW_MS,
    } as const;
    expect(constructBoundedContextEnvelope(options)).toEqual(
      constructBoundedContextEnvelope(options),
    );
  });
});

describe("T10 Correction Round 1 (fresh-target findings) — MAJOR 1: per-role context budget", () => {
  /** The verifier's smuggling shape: every Case focus OUTSIDE the hoisted plan, declared as dependencies. */
  function smugglingDependencies(
    caseWorld: ProjectionWorld,
    horizon: ReturnType<typeof hoistedHorizon>,
  ) {
    const planIds = new Set(
      planMaterialization(caseWorld, horizon).context_candidates.map(
        (candidate) => candidate.focus.id,
      ),
    );
    const outside = [
      ...caseWorld.units.map(focusForUnit),
      ...caseWorld.evidence.map(focusForEvidence),
      ...caseWorld.sessions.map(focusForSession),
    ].filter((focus) => !planIds.has(focus.id));
    return outside.map((focus) => ({ focus, reason: "smuggling probe" }));
  }

  it("verifier smuggling shape (RED-first): whole-Case content via declared dependencies is a typed budget refusal — never silently re-kinded", () => {
    const caseWorld = world();
    const horizon = hoistedHorizon();
    const plan = planMaterialization(caseWorld, horizon);
    const smuggle = smugglingDependencies(caseWorld, horizon);
    // The shape is real: five Case foci outside the hoisted plan (u-other, u-far, ev-3, s-1, s-2).
    expect(smuggle.length).toBe(5);
    expect(smuggle.length).toBeGreaterThan(DEFAULT_ENVELOPE_ROLE_BUDGET.declared_dependency);

    let detail = "";
    try {
      constructBoundedContextEnvelope({ horizon, plan, required_dependencies: smuggle });
    } catch (error) {
      expect(error).toBeInstanceOf(EnvelopeConstructionError);
      detail = error instanceof Error ? error.message : String(error);
    }
    expect(detail).toContain("per-role context budget exceeded");
    expect(detail).toContain("declared_dependency");
    expect(detail).toContain("never silently re-kinded");
  });

  it("within-budget dependencies still construct, and the accounting is recorded in envelope provenance", () => {
    const caseWorld = world();
    const horizon = hoistedHorizon();
    const withinBudget = constructBoundedContextEnvelope({
      horizon,
      plan: planMaterialization(caseWorld, horizon),
      required_dependencies: [
        {
          focus: FocusRef.for("gauntlet", "Evidence", "ev-shared-fixture"),
          reason: "preregistered fixture",
        },
      ],
      constructed_at_ms: NOW_MS,
    });
    expect(withinBudget.budget).toEqual({
      limits: {
        relation: DEFAULT_ENVELOPE_ROLE_BUDGET.relation,
        declared_dependency: DEFAULT_ENVELOPE_ROLE_BUDGET.declared_dependency,
      },
      relation_count: 0,
      declared_dependency_count: 1,
      exempt_roles: BUDGET_EXEMPT_ROLES,
    });
  });

  it("the caller's role/config may raise the budget explicitly — recorded, and the smuggled deps then land in items", () => {
    const caseWorld = world();
    const horizon = hoistedHorizon();
    const smuggle = smugglingDependencies(caseWorld, horizon);
    const raised: EnvelopeRoleBudget = { relation: 16, declared_dependency: 8 };
    const envelope = constructBoundedContextEnvelope({
      horizon,
      plan: planMaterialization(caseWorld, horizon),
      required_dependencies: smuggle,
      context_budget: raised,
      envelope_id: "envelope-raised-budget",
      constructed_at_ms: NOW_MS,
    });
    expect(envelope.budget).toEqual({
      limits: { relation: 16, declared_dependency: 8 },
      relation_count: 0,
      declared_dependency_count: 5,
      exempt_roles: BUDGET_EXEMPT_ROLES,
    });
    expect(envelope.declared_dependencies.length).toBe(5);
  });

  it("the relation budget is per-role too: exceeding it refuses while the mandatory closure stays exempt", () => {
    // A horizon whose traversal REACHES relation items (not just the root).
    const sessionWorld = world();
    const sessionHorizon = createHorizon({
      root_focus: FocusRef.for("gauntlet", "Agent", "s-1"),
      generation: GEN,
      relation_queries: [{ kind: "child_session", depth: 2, direction: "outbound" }],
      horizon_id: "horizon-t10-relations",
    });
    const sessionPlan = planMaterialization(sessionWorld, sessionHorizon);
    const zeroRelation: EnvelopeRoleBudget = { relation: 0, declared_dependency: 4 };
    expect(() =>
      constructBoundedContextEnvelope({
        horizon: sessionHorizon,
        plan: sessionPlan,
        context_budget: zeroRelation,
      }),
    ).toThrow(/per-role context budget exceeded/);

    // Same plan under the default budget constructs, and counts its relation items.
    const counted = constructBoundedContextEnvelope({
      horizon: sessionHorizon,
      plan: sessionPlan,
      constructed_at_ms: NOW_MS,
    });
    expect(counted.budget?.relation_count).toBe(1); // s-2 reached via child_session
    expect(counted.items.some((item) => item.role === "horizon_root")).toBe(true);
  });

  it("whole-case disclosure carries budget: null; parse validates the accounting round-trip", () => {
    const caseWorld = world();
    const hoistedPlan = planMaterialization(caseWorld, hoistedHorizon());
    const wholeCasePlan = planMaterialization(caseWorld, null);
    const disclosed = constructWholeCaseEnvelope({
      plan: wholeCasePlan,
      generation: GEN,
      root_focus: FocusRef.for("gauntlet", "WorkItem", "u-root"),
      disclosure_reason: "operator explicitly authorized whole-Case context",
      constructed_at_ms: NOW_MS,
    });
    expect(disclosed.budget).toBeNull();

    const bounded = constructBoundedContextEnvelope({
      horizon: hoistedHorizon(),
      plan: hoistedPlan,
      required_dependencies: [
        {
          focus: FocusRef.for("gauntlet", "Reference", "answer-key-digest"),
          reason: "frozen ruler",
        },
      ],
      constructed_at_ms: NOW_MS,
    });
    expect(envelopeFromJSON(envelopeToJSON(bounded)).budget).toEqual(bounded.budget);

    // A bounded document stripped of its accounting is malformed.
    const stripped = JSON.parse(envelopeToJSON(bounded)) as Record<string, unknown>;
    delete stripped["budget"];
    expect(() => envelopeFromJSON(JSON.stringify(stripped))).toThrow(MalformedEnvelopeError);
  });
});
