import { describe, expect, it } from "bun:test";
import { FocusRef } from "./focus";
import type { AuthoritativeEventFrame } from "./projection-store";
import type { RelationQuerySpec } from "./relations";
import type {
  AffordanceGateName,
  CapabilityObservation,
  FullWorkbenchSnapshot,
} from "./workbench-model";
import { WorkbenchModel } from "./workbench-model";

/**
 * T08 horizon-driven progressive materialization suites.
 * Proves REQ-FOCUS-004 (horizon-local over eager whole-Case), REQ-GOAL-001,
 * REQ-GOAL-006, REQ-AFF-004/005, REQ-OPS-004 (attention independence, capability
 * reasons, mandatory closure), and spec §Relation (source-typed edges with
 * provenance between FocusRefs).
 */

const GEN = "gen-materialization";

function event(
  partial: Omit<AuthoritativeEventFrame, "generation_id" | "source_id">,
): AuthoritativeEventFrame {
  return { generation_id: GEN, source_id: "gauntlet", ...partial };
}

function unitEvent(
  unitId: string,
  title: string,
  cursor: string,
  status: string,
  authority: string,
): AuthoritativeEventFrame {
  return event({
    cursor,
    event_type: "unit_updated",
    payload: { unit_id: unitId, title, status, authority, blast_radius: "isolated" },
    timestamp_ms: 1000,
  });
}

/** Same Case world as the hoist suite minus session s-3: 3 units (u-far blocked), 3 evidence, s-1←s-2 plus the s-a↔s-b cycle. */
function seededModel(): WorkbenchModel {
  const model = new WorkbenchModel();
  model.startGeneration(GEN);
  model.applyEvent(unitEvent("u-root", "Root Unit", "c-1", "active", "builder"));
  model.applyEvent(
    event({
      cursor: "c-2",
      event_type: "evidence_added",
      payload: { evidence_id: "ev-1", unit_id: "u-root", kind: "report", verified: true },
      timestamp_ms: 1100,
    }),
  );
  model.applyEvent(
    event({
      cursor: "c-3",
      event_type: "evidence_added",
      payload: { evidence_id: "ev-2", unit_id: "u-root", kind: "log", verified: false },
      timestamp_ms: 1200,
    }),
  );
  model.applyEvent(unitEvent("u-other", "Other Unit", "c-4", "active", "builder"));
  model.applyEvent(
    event({
      cursor: "c-5",
      event_type: "evidence_added",
      payload: { evidence_id: "ev-3", unit_id: "u-other", kind: "report", verified: true },
      timestamp_ms: 1400,
    }),
  );
  model.applyEvent(unitEvent("u-far", "Far Unit", "c-6", "blocked", "orchestrator"));
  model.applyEvent(
    event({
      cursor: "c-7",
      event_type: "session_updated",
      payload: { session_id: "s-1", parent_id: null, model: "model-a", status: "running" },
      timestamp_ms: 1600,
    }),
  );
  model.applyEvent(
    event({
      cursor: "c-8",
      event_type: "session_updated",
      payload: { session_id: "s-2", parent_id: "s-1", model: "model-a", status: "running" },
      timestamp_ms: 1700,
    }),
  );
  model.applyEvent(
    event({
      cursor: "c-9",
      event_type: "session_updated",
      payload: { session_id: "s-a", parent_id: "s-b", model: "model-c", status: "running" },
      timestamp_ms: 1800,
    }),
  );
  model.applyEvent(
    event({
      cursor: "c-10",
      event_type: "session_updated",
      payload: { session_id: "s-b", parent_id: "s-a", model: "model-c", status: "running" },
      timestamp_ms: 1900,
    }),
  );
  return model;
}

const HAS_EVIDENCE_1: RelationQuerySpec[] = [
  { kind: "has_evidence", depth: 1, direction: "outbound" },
];

function gatesWith(
  overrides: Partial<Record<AffordanceGateName, boolean>>,
): Record<AffordanceGateName, boolean> {
  return {
    visibility: true,
    reachability: true,
    payment: true,
    governance: true,
    settlement_access: true,
    recovery: true,
    ...overrides,
  };
}

describe("T08 Horizon-Driven Progressive Materialization (REQ-FOCUS-004, REQ-GOAL-001, REQ-GOAL-006, REQ-AFF-004, REQ-AFF-005, REQ-OPS-004)", () => {
  it("Negative control: counters are real — hoisting shrinks every counter and the working set is the declared closure, never the whole Case", () => {
    const model = seededModel();
    const eagerPlan = model.getMaterializationPlan();
    expect(eagerPlan.horizon_id).toBeNull();
    expect(eagerPlan.counters.subscriptions).toBe(10); // 3 units + 3 evidence + 4 sessions
    expect(eagerPlan.counters.queries).toBe(3); // whole-Case scans: units, evidence, sessions
    const eagerIds = new Set(eagerPlan.working_set.map((focus) => focus.id));
    expect(eagerIds.has("focus:gauntlet:WorkItem:u-far")).toBe(true);
    expect(eagerIds.has("focus:gauntlet:Evidence:ev-3")).toBe(true);

    const rootRef = FocusRef.for("gauntlet", "WorkItem", "u-root");
    model.hoistToFocus(rootRef, { relation_queries: HAS_EVIDENCE_1 });
    const hoistedPlan = model.getMaterializationPlan();

    // A broken narrowing (or eager behavior) would keep any counter at whole-Case size.
    expect(hoistedPlan.counters.subscriptions).toBe(3);
    expect(hoistedPlan.counters.queries).toBe(2);
    expect(hoistedPlan.counters.prefetches).toBe(3);
    expect(hoistedPlan.counters.context_candidates).toBe(5); // root + 2 evidence + reference + governance
    expect(hoistedPlan.counters.subscriptions).toBeLessThan(eagerPlan.counters.subscriptions);
    expect(hoistedPlan.counters.queries).toBeLessThan(eagerPlan.counters.queries);
    expect(hoistedPlan.counters.prefetches).toBeLessThan(eagerPlan.counters.prefetches);
    expect(hoistedPlan.counters.context_candidates).toBeLessThan(
      eagerPlan.counters.context_candidates,
    );

    // Exact membership, not just smaller numbers.
    expect(hoistedPlan.working_set.map((focus) => focus.id).sort()).toEqual(
      [
        "focus:gauntlet:WorkItem:u-root",
        "focus:gauntlet:Evidence:ev-1",
        "focus:gauntlet:Evidence:ev-2",
      ].sort(),
    );
    const hoistedCandidateIds = hoistedPlan.context_candidates.map(
      (candidate) => candidate.focus.id,
    );
    expect(hoistedCandidateIds).not.toContain("focus:gauntlet:WorkItem:u-far");
    expect(hoistedCandidateIds).not.toContain("focus:gauntlet:Evidence:ev-3");

    // Visible affordances narrow with the horizon (REQ-FOCUS-003).
    const snap = model.getSnapshot();
    expect(snap.affordance_ids).toContain("affordance-u-root");
    expect(snap.affordance_ids).not.toContain("affordance-u-other");
    model.close();
  });

  it("Case-wide urgent attention is independent of the Horizon-local context (REQ-GOAL-001, REQ-UX-002)", () => {
    const model = seededModel();
    const unhoistedAttention = model.getSnapshot().attention;
    expect(unhoistedAttention.length).toBe(1);
    expect(unhoistedAttention[0]?.focus).toBe(FocusRef.for("gauntlet", "WorkItem", "u-far"));
    expect(unhoistedAttention[0]?.severity).toBe("urgent");
    expect(unhoistedAttention[0]?.reason).toBe("unit_blocked");
    expect(unhoistedAttention[0]?.horizon_independent).toBe(true);

    const rootRef = FocusRef.for("gauntlet", "WorkItem", "u-root");
    model.hoistToFocus(rootRef, { relation_queries: HAS_EVIDENCE_1 });
    const snap = model.getSnapshot();

    // Hoisted context candidates exclude the far unit, but attention still surfaces it.
    expect(snap.context_candidates.map((candidate) => candidate.focus.id)).not.toContain(
      "focus:gauntlet:WorkItem:u-far",
    );
    expect(snap.attention).toEqual(unhoistedAttention);
    expect(snap.materialization.working_set_size).toBe(3);
    model.close();
  });

  it("Capabilities: installed is not thereby available; unavailable reasons stay inspectable (REQ-AFF-004, REQ-AFF-005, REQ-OPS-004)", () => {
    const model = seededModel();
    const observations: CapabilityObservation[] = [
      { capability: "cargo-nextest", installed: true, gates: gatesWith({}) },
      { capability: "cargo-mutants", installed: true, gates: gatesWith({ payment: false }) },
      { capability: "domainforge", installed: false, gates: gatesWith({}) },
    ];
    model.declareCapabilities(observations);

    let snap = model.getSnapshot();
    const byName = new Map(
      snap.capability_reasons.map((capability) => [capability.capability, capability]),
    );

    // Installed with every gate passed is available and rendered as an affordance.
    expect(byName.get("cargo-nextest")?.status).toBe("available");
    expect(byName.get("cargo-nextest")?.reason_codes).toEqual([]);
    expect(snap.affordance_ids).toContain("affordance:capability:cargo-nextest");

    // Installed is NOT available: payment gate missing keeps it out of affordances
    // while the reason stays inspectable.
    const mutants = byName.get("cargo-mutants");
    expect(mutants?.installed).toBe(true);
    expect(mutants?.status).toBe("unavailable");
    expect(mutants?.reason_codes).toEqual(["payment_gate_not_passed"]);
    expect(snap.affordance_ids).not.toContain("affordance:capability:cargo-mutants");
    expect(snap.affordance_ids).not.toContain("affordance:capability:domainforge");

    // Not installed is an inspectable reason too.
    expect(byName.get("domainforge")?.status).toBe("unavailable");
    expect(byName.get("domainforge")?.reason_codes).toEqual(["not_installed"]);

    // Hoisting never fakes availability: the same honest statuses carry over.
    model.hoistToFocus(FocusRef.for("gauntlet", "WorkItem", "u-root"), {
      relation_queries: HAS_EVIDENCE_1,
    });
    snap = model.getSnapshot();
    expect(snap.capability_reasons).toEqual(model.getSnapshot().capability_reasons);
    expect(snap.capability_reasons.map((capability) => capability.status)).toEqual([
      "available",
      "unavailable",
      "unavailable",
    ]);
    model.close();
  });

  it("Prefers horizon-local incremental materialization over eager whole-Case loading (REQ-FOCUS-004, REQ-GOAL-006)", () => {
    const model = seededModel();
    const rootRef = FocusRef.for("gauntlet", "WorkItem", "u-root");
    model.hoistToFocus(rootRef, { relation_queries: HAS_EVIDENCE_1 });

    const beforeCounters = model.getSnapshot().materialization.counters;
    const beforeWorkingSet = model.getMaterializationPlan().working_set.map((focus) => focus.id);

    // New Case activity far outside the hoisted horizon.
    model.applyEvent(
      event({
        cursor: "c-11",
        event_type: "evidence_added",
        payload: { evidence_id: "ev-9", unit_id: "u-other", kind: "report", verified: true },
        timestamp_ms: 3000,
      }),
    );

    const afterPlan = model.getMaterializationPlan();
    // The hoisted horizon did NOT eagerly re-materialize the whole Case.
    expect(afterPlan.counters).toEqual(beforeCounters);
    expect(afterPlan.working_set.map((focus) => focus.id)).toEqual(beforeWorkingSet);
    expect(afterPlan.working_set.map((focus) => focus.id)).not.toContain(
      "focus:gauntlet:Evidence:ev-9",
    );

    // Source truth still advanced; the broader world sees it after de-hoist.
    expect(
      model.getRelations().some((edge) => edge.target_ref.id === "focus:gauntlet:Evidence:ev-9"),
    ).toBe(true);
    model.dehoist();
    const eagerIds = model.getMaterializationPlan().working_set.map((focus) => focus.id);
    expect(eagerIds).toContain("focus:gauntlet:Evidence:ev-9");
    model.close();
  });

  it("Replay across hoist/de-hoist cycles is identity-preserving (preregistered transfer expectation)", () => {
    const model = seededModel();
    const observed: FocusRef[] = [];

    for (let cycle = 0; cycle < 3; cycle += 1) {
      const rootRef = FocusRef.for("gauntlet", "WorkItem", "u-root");
      model.hoistToFocus(rootRef, { relation_queries: HAS_EVIDENCE_1 });
      observed.push(...model.getMaterializationPlan().working_set);
      model.dehoist();
      observed.push(...model.getMaterializationPlan().working_set);

      const sessionRef = FocusRef.for("gauntlet", "Agent", "s-1");
      model.hoistToFocus(sessionRef, {
        relation_queries: [{ kind: "child_session", depth: 2, direction: "both" }],
      });
      observed.push(...model.getMaterializationPlan().working_set);
      model.dehoist();
    }

    // No identity churn anywhere in the replay: every observed ref is the interned one.
    expect(observed.length).toBeGreaterThan(0);
    for (const ref of observed) {
      expect(ref).toBe(FocusRef.for(ref.source, ref.kind, ref.source_id));
    }
    // And the whole-Case world is intact after all the cycling.
    expect(model.getMaterializationPlan().working_set.length).toBe(10);
    model.close();
  });

  it("Relations are source-typed edges with provenance between FocusRefs (spec §Relation, REQ-GOAL-001)", () => {
    const model = seededModel();
    const relations = model.getRelations();

    const evidenceEdge = relations.find(
      (edge) =>
        edge.kind === "has_evidence" && edge.target_ref.id === "focus:gauntlet:Evidence:ev-1",
    );
    expect(evidenceEdge?.source_ref.id).toBe("focus:gauntlet:WorkItem:u-root");
    expect(evidenceEdge?.source_ref.kind).toBe("WorkItem");
    expect(evidenceEdge?.target_ref.kind).toBe("Evidence");
    expect(evidenceEdge?.provenance).toEqual({
      generation_id: GEN,
      source_id: "gauntlet",
      source_cursor: "c-2",
    });
    expect(evidenceEdge?.source_version).toBe("c-2");
    expect(evidenceEdge?.valid).toBe(true);

    const sessionEdge = relations.find(
      (edge) => edge.kind === "child_session" && edge.target_ref.id === "focus:gauntlet:Agent:s-2",
    );
    expect(sessionEdge?.source_ref.id).toBe("focus:gauntlet:Agent:s-1");
    expect(sessionEdge?.provenance).toEqual({
      generation_id: GEN,
      source_id: "gauntlet",
      source_cursor: "c-8",
    });
    expect(sessionEdge?.source_version).toBe("c-8");
    expect(sessionEdge?.valid).toBe(true);

    // All edges carry provenance; every endpoint is an interned FocusRef identity.
    expect(relations.length).toBe(6); // 3 evidence links + 3 parent links (s-1, s-a, s-b)
    for (const edge of relations) {
      expect(edge.provenance.generation_id).toBe(GEN);
      expect(edge.source_ref).toBe(
        FocusRef.for(edge.source_ref.source, edge.source_ref.kind, edge.source_ref.source_id),
      );
      expect(edge.target_ref).toBe(
        FocusRef.for(edge.target_ref.source, edge.target_ref.kind, edge.target_ref.source_id),
      );
    }

    // One snapshot carries the coherent local world without owning it: work, agents,
    // evidence, history, attention, and candidates aligned on one generation.
    const snap: FullWorkbenchSnapshot = model.getSnapshot();
    expect(snap.generation).toBe(GEN);
    expect(snap.units.length).toBe(3);
    expect(snap.evidence.length).toBe(3);
    expect(snap.sessions.length).toBe(4);
    expect(snap.trace_records.length).toBeGreaterThan(0);
    expect(snap.attention.length).toBe(1);
    expect(snap.context_candidates.length).toBeGreaterThan(0);
    model.close();
  });
});

describe("T08 Correction Round 1 (peer findings)", () => {
  // Local frame builder: the module-level event() helper pins source_id to
  // 'gauntlet', which would defeat these cross-source scenarios.
  function crossSourceEvent(
    generationId: string,
    sourceId: string,
    cursor: string,
    eventType: AuthoritativeEventFrame["event_type"],
    payload: Record<string, unknown>,
  ): AuthoritativeEventFrame {
    return {
      generation_id: generationId,
      source_id: sourceId,
      cursor,
      event_type: eventType,
      payload,
      timestamp_ms: 1000,
    };
  }

  it("MAJOR-1: cross-source evidence attributes the unit endpoint from the owning unit row and marks source disagreement invalid", () => {
    const model = new WorkbenchModel();
    model.startGeneration("gen-correction-1");
    // The unit is owned by source 'gauntlet'...
    model.applyEvent(
      crossSourceEvent("gen-correction-1", "gauntlet", "c-1", "unit_updated", {
        unit_id: "u-shared",
        title: "Shared Unit",
        status: "active",
      }),
    );
    // ...while the evidence row CLAIMS source 'prime' on the same unit_id.
    model.applyEvent(
      crossSourceEvent("gen-correction-1", "prime", "p-1", "evidence_added", {
        evidence_id: "ev-x",
        unit_id: "u-shared",
        kind: "report",
        verified: true,
      }),
    );

    const relations = model.getRelations();
    const edge = relations.find(
      (e) => e.kind === "has_evidence" && e.target_ref.id === "focus:prime:Evidence:ev-x",
    );
    expect(edge).toBeDefined();
    // The unit endpoint is attributed to the OWNING unit row's source, never the evidence claim.
    expect(edge?.source_ref).toBe(FocusRef.for("gauntlet", "WorkItem", "u-shared"));
    expect(edge?.source_ref.id).toBe("focus:gauntlet:WorkItem:u-shared");
    // Source disagreement is visible, not silently accepted.
    expect(edge?.valid).toBe(false);
    // Exactly ONE identity for the unit across every has_evidence endpoint.
    const unitEndpointIds = relations
      .filter((e) => e.kind === "has_evidence")
      .map((e) => e.source_ref.id);
    expect(new Set(unitEndpointIds)).toEqual(new Set(["focus:gauntlet:WorkItem:u-shared"]));
    model.close();
  });

  it("MAJOR-1: cross-source session parent links follow the same attribution rule", () => {
    const model = new WorkbenchModel();
    model.startGeneration("gen-correction-1b");
    // Parent session owned by 'prime'...
    model.applyEvent(
      crossSourceEvent("gen-correction-1b", "prime", "p-1", "session_updated", {
        session_id: "s-p1",
        parent_id: null,
        model: "model-p",
        status: "running",
      }),
    );
    // ...child session from 'gauntlet' claiming that parent.
    model.applyEvent(
      crossSourceEvent("gen-correction-1b", "gauntlet", "c-1", "session_updated", {
        session_id: "s-g1",
        parent_id: "s-p1",
        model: "model-g",
        status: "running",
      }),
    );

    const relations = model.getRelations();
    const edge = relations.find(
      (e) => e.kind === "child_session" && e.target_ref.id === "focus:gauntlet:Agent:s-g1",
    );
    expect(edge).toBeDefined();
    // The parent endpoint is attributed to the OWNING parent row's source.
    expect(edge?.source_ref).toBe(FocusRef.for("prime", "Agent", "s-p1"));
    expect(edge?.valid).toBe(false);
    // A clean same-source pair stays valid.
    model.applyEvent(
      crossSourceEvent("gen-correction-1b", "prime", "p-2", "session_updated", {
        session_id: "s-p2",
        parent_id: "s-p1",
        model: "model-p",
        status: "running",
      }),
    );
    const cleanEdge = model
      .getRelations()
      .find((e) => e.kind === "child_session" && e.target_ref.id === "focus:prime:Agent:s-p2");
    expect(cleanEdge?.source_ref).toBe(FocusRef.for("prime", "Agent", "s-p1"));
    expect(cleanEdge?.valid).toBe(true);
    model.close();
  });
});
