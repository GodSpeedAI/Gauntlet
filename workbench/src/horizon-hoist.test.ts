import { describe, expect, it } from "bun:test";
import { FocusRef } from "./focus";
import type { Horizon } from "./horizon";
import type { AuthoritativeEventFrame } from "./projection-store";
import type { RelationQuerySpec } from "./relations";
import { WorkbenchModel, WorkbenchModelError } from "./workbench-model";

/**
 * T08 Horizon hoist/de-hoist suites (plan T08, teeth attack 2).
 * Proves REQ-FOCUS-003, REQ-FOCUS-004, REQ-FOCUS-005, REQ-OPS-004, REQ-UX-002:
 * hoisting narrows the active horizon (representation, queries, context) without
 * changing source truth or reidentifying objects; mandatory closure survives
 * narrowing; saved horizons stay noncanonical; breadcrumb and one-step return.
 */

const GEN = "gen-hoist";

function event(
  partial: Omit<AuthoritativeEventFrame, "generation_id" | "source_id">,
): AuthoritativeEventFrame {
  return { generation_id: GEN, source_id: "gauntlet", ...partial };
}

/**
 * Case world:
 *   units:    u-root, u-other, u-far (blocked)
 *   evidence: ev-1, ev-2 on u-root; ev-3 on u-other
 *   sessions: s-1 ← s-2 ← s-3 chain, plus the cyclic pair s-a ↔ s-b
 */
function seededModel(): WorkbenchModel {
  const model = new WorkbenchModel();
  model.startGeneration(GEN);
  model.applyEvent(
    event({
      cursor: "c-1",
      event_type: "unit_updated",
      payload: {
        unit_id: "u-root",
        title: "Root Unit",
        status: "active",
        authority: "builder",
        blast_radius: "isolated",
      },
      timestamp_ms: 1000,
    }),
  );
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
  model.applyEvent(
    event({
      cursor: "c-4",
      event_type: "unit_updated",
      payload: {
        unit_id: "u-other",
        title: "Other Unit",
        status: "active",
        authority: "builder",
        blast_radius: "project",
      },
      timestamp_ms: 1300,
    }),
  );
  model.applyEvent(
    event({
      cursor: "c-5",
      event_type: "evidence_added",
      payload: { evidence_id: "ev-3", unit_id: "u-other", kind: "report", verified: true },
      timestamp_ms: 1400,
    }),
  );
  model.applyEvent(
    event({
      cursor: "c-6",
      event_type: "unit_updated",
      payload: {
        unit_id: "u-far",
        title: "Far Unit",
        status: "blocked",
        authority: "orchestrator",
        blast_radius: "workspace",
      },
      timestamp_ms: 1500,
    }),
  );
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
      payload: { session_id: "s-3", parent_id: "s-2", model: "model-b", status: "running" },
      timestamp_ms: 1800,
    }),
  );
  model.applyEvent(
    event({
      cursor: "c-10",
      event_type: "session_updated",
      payload: { session_id: "s-a", parent_id: "s-b", model: "model-c", status: "running" },
      timestamp_ms: 1900,
    }),
  );
  model.applyEvent(
    event({
      cursor: "c-11",
      event_type: "session_updated",
      payload: { session_id: "s-b", parent_id: "s-a", model: "model-c", status: "running" },
      timestamp_ms: 2000,
    }),
  );
  return model;
}

const HAS_EVIDENCE_1: RelationQuerySpec[] = [
  { kind: "has_evidence", depth: 1, direction: "outbound" },
];

describe("T08 Horizon Hoist/De-hoist (REQ-FOCUS-003, REQ-FOCUS-004, REQ-FOCUS-005, REQ-OPS-004, REQ-UX-002)", () => {
  it("Attack 2 (REQ-FOCUS-003/004): hoisting to a leaf narrows the working set to the declared relations; de-hoist restores the broader world without reidentifying objects", () => {
    const model = seededModel();
    const beforePlan = model.getMaterializationPlan();
    const beforeUnits = model.getSnapshot().units;
    const beforeIds = beforePlan.working_set.map((focus) => focus.id).sort();
    expect(beforeIds.length).toBe(11); // 3 units + 3 evidence + 5 sessions (whole Case)

    const rootRef = FocusRef.for("gauntlet", "WorkItem", "u-root");
    const horizon = model.hoistToFocus(rootRef, { relation_queries: HAS_EVIDENCE_1 });

    const hoistedPlan = model.getMaterializationPlan();
    expect(hoistedPlan.horizon_id).toBe(horizon.id);

    // Exact declared closure: root + its two evidence items — NOT the whole Case.
    expect(hoistedPlan.working_set.map((focus) => focus.id).sort()).toEqual(
      [
        "focus:gauntlet:WorkItem:u-root",
        "focus:gauntlet:Evidence:ev-1",
        "focus:gauntlet:Evidence:ev-2",
      ].sort(),
    );
    const candidateIds = hoistedPlan.context_candidates.map((candidate) => candidate.focus.id);
    expect(candidateIds).not.toContain("focus:gauntlet:WorkItem:u-other");
    expect(candidateIds).not.toContain("focus:gauntlet:WorkItem:u-far");
    expect(candidateIds).not.toContain("focus:gauntlet:Evidence:ev-3");
    expect(candidateIds).not.toContain("focus:gauntlet:Agent:s-1");
    expect(candidateIds).not.toContain("focus:gauntlet:Agent:s-a");

    // Every counter narrowed.
    expect(hoistedPlan.counters.subscriptions).toBe(3);
    expect(hoistedPlan.counters.queries).toBe(2); // 1 focus_load + 1 declared traversal
    expect(hoistedPlan.counters.prefetches).toBe(3);
    expect(hoistedPlan.counters.context_candidates).toBeLessThan(
      beforePlan.counters.context_candidates,
    );
    expect(hoistedPlan.counters.subscriptions).toBeLessThan(beforePlan.counters.subscriptions);
    expect(hoistedPlan.counters.prefetches).toBeLessThan(beforePlan.counters.prefetches);

    // Source truth untouched while hoisted.
    expect(model.getSnapshot().units).toEqual(beforeUnits);

    const dehoisted = model.dehoist();
    expect(dehoisted).toBeNull();

    // The broader world is restored — exactly the same population.
    const restoredPlan = model.getMaterializationPlan();
    expect(restoredPlan.horizon_id).toBeNull();
    expect(restoredPlan.working_set.map((focus) => focus.id).sort()).toEqual(beforeIds);

    // No object was reidentified: every restored ref is the SAME interned identity.
    for (const focus of restoredPlan.working_set) {
      expect(focus).toBe(FocusRef.for(focus.source, focus.kind, focus.source_id));
    }

    // Source truth identical across the whole cycle; selection identity retained.
    expect(model.getSnapshot().units).toEqual(beforeUnits);
    expect(model.getSnapshot().focus).toBe(rootRef);

    model.close();
  });

  it("Mandatory Reference/governance/evidence closure survives narrowing even with no declared relations (REQ-OPS-004)", () => {
    const model = seededModel();
    const rootRef = FocusRef.for("gauntlet", "WorkItem", "u-root");

    // Declare NO relation traversals at all: the only narrowing allowed is to the root.
    model.hoistToFocus(rootRef, { relation_queries: [] });

    const plan = model.getMaterializationPlan();
    // Working set: the root plus its mandatory evidence closure (never dropped).
    expect(plan.working_set.map((focus) => focus.id).sort()).toEqual(
      [
        "focus:gauntlet:WorkItem:u-root",
        "focus:gauntlet:Evidence:ev-1",
        "focus:gauntlet:Evidence:ev-2",
      ].sort(),
    );

    const roles = new Map(
      plan.context_candidates.map((candidate) => [candidate.focus.id, candidate.role]),
    );
    expect(roles.get("focus:gauntlet:WorkItem:u-root")).toBe("relation"); // the horizon root itself
    expect(roles.get("focus:gauntlet:Evidence:ev-1")).toBe("mandatory_evidence");
    expect(roles.get("focus:gauntlet:Evidence:ev-2")).toBe("mandatory_evidence");
    expect(roles.get("focus:gauntlet:Reference:generation:gen-hoist")).toBe("mandatory_reference");
    expect(roles.get("focus:gauntlet:Governance:authority:builder")).toBe("mandatory_governance");
    for (const candidate of plan.context_candidates) {
      if (candidate.role !== "relation") {
        expect(candidate.mandatory).toBe(true);
      }
    }

    model.close();
  });

  it("Hoisting into agent topology narrows to declared depth and terminates on cyclic relations", () => {
    const model = seededModel();
    const s1 = FocusRef.for("gauntlet", "Agent", "s-1");

    // Depth 1 outbound child_session reaches only the direct child.
    model.hoistToFocus(s1, {
      relation_queries: [{ kind: "child_session", depth: 1, direction: "both" }],
    });
    let plan = model.getMaterializationPlan();
    expect(plan.working_set.map((focus) => focus.id).sort()).toEqual(
      ["focus:gauntlet:Agent:s-1", "focus:gauntlet:Agent:s-2"].sort(),
    );
    model.dehoist();

    // Depth 3 reaches the full chain s-1 ← s-2 ← s-3 but NOT the unrelated cycle.
    model.hoistToFocus(s1, {
      relation_queries: [{ kind: "child_session", depth: 3, direction: "both" }],
    });
    plan = model.getMaterializationPlan();
    expect(plan.working_set.map((focus) => focus.id).sort()).toEqual(
      ["focus:gauntlet:Agent:s-1", "focus:gauntlet:Agent:s-2", "focus:gauntlet:Agent:s-3"].sort(),
    );
    model.dehoist();

    // Traversing INTO a cycle terminates: s-a ↔ s-b, bounded finite working set.
    const cycleStart = FocusRef.for("gauntlet", "Agent", "s-a");
    model.hoistToFocus(cycleStart, {
      relation_queries: [{ kind: "child_session", depth: 50, direction: "both" }],
    });
    plan = model.getMaterializationPlan();
    expect(plan.working_set.map((focus) => focus.id).sort()).toEqual(
      ["focus:gauntlet:Agent:s-a", "focus:gauntlet:Agent:s-b"].sort(),
    );
    expect(Number.isFinite(plan.counters.subscriptions)).toBe(true);
    model.close();
  });

  it("Hoist exposes a breadcrumb and one-step return (REQ-UX-002)", () => {
    const model = seededModel();
    expect(model.getSnapshot().horizon_breadcrumb).toEqual(["Case"]);

    const rootRef = FocusRef.for("gauntlet", "WorkItem", "u-root");
    const firstHorizon = model.hoistToFocus(rootRef, { relation_queries: HAS_EVIDENCE_1 });
    expect(model.getSnapshot().horizon_breadcrumb).toEqual(["Case", "u-root"]);

    const sessionRef = FocusRef.for("gauntlet", "Agent", "s-1");
    const secondHorizon = model.hoistToFocus(sessionRef, {
      relation_queries: [{ kind: "child_session", depth: 2, direction: "both" }],
    });
    expect(model.getSnapshot().horizon_breadcrumb).toEqual(["Case", "u-root", "s-1"]);
    expect(model.getSnapshot().horizon).toBe(secondHorizon.id);

    // One-step return: first dehoist goes back to the previous horizon, not to the Case.
    const steppedBack = model.dehoist();
    expect(steppedBack?.id).toBe(firstHorizon.id);
    expect(model.getSnapshot().horizon).toBe(firstHorizon.id);
    expect(model.getSnapshot().horizon_breadcrumb).toEqual(["Case", "u-root"]);

    // Second dehoist restores the whole-Case world.
    expect(model.dehoist()).toBeNull();
    expect(model.getSnapshot().horizon).toBeNull();
    expect(model.getSnapshot().horizon_breadcrumb).toEqual(["Case"]);
    model.close();
  });

  it("Saved Horizons persist preferences but never become independent domain truth (REQ-FOCUS-005)", () => {
    const model = seededModel();
    const rootRef = FocusRef.for("gauntlet", "WorkItem", "u-root");
    const horizon = model.hoistToFocus(rootRef, { relation_queries: HAS_EVIDENCE_1 });

    model.saveHorizon(horizon, "leaf-focus");
    expect(model.getSavedHorizonNames()).toEqual(["leaf-focus"]);
    const saved = model.loadSavedHorizon("leaf-focus");
    expect(saved?.horizon.saved_name).toBe("leaf-focus");
    expect(saved?.horizon.root_focus).toBe(rootRef);
    expect(model.dehoist()).toBeNull();

    // Source truth advances AFTER saving; the saved preference did not freeze it.
    model.applyEvent(
      event({
        cursor: "c-12",
        event_type: "evidence_added",
        payload: { evidence_id: "ev-4", unit_id: "u-other", kind: "report", verified: true },
        timestamp_ms: 2100,
      }),
    );

    const restored = model.restoreSavedHorizon("leaf-focus");
    expect(restored?.id).toBe(horizon.id);
    expect(restored?.root_focus).toBe(rootRef); // same identity, not a fork

    const plan = model.getMaterializationPlan();
    expect(plan.horizon_id).toBe(horizon.id);
    // The horizon still narrows by its declared queries against the CURRENT world.
    expect(plan.working_set.map((focus) => focus.id).sort()).toEqual(
      [
        "focus:gauntlet:WorkItem:u-root",
        "focus:gauntlet:Evidence:ev-1",
        "focus:gauntlet:Evidence:ev-2",
      ].sort(),
    );

    // Saved horizons are plain serializable preferences, separate from the projection
    // store: the registry exports JSON, and importing it into a fresh model resolves
    // the SAME identities (no fork across serialization).
    const json = model.exportSavedHorizons();
    expect(json).toContain("leaf-focus");
    const fresh = new WorkbenchModel();
    fresh.startGeneration("gen-other");
    fresh.importSavedHorizons(json);
    const freshRestored = fresh.restoreSavedHorizon("leaf-focus");
    expect(freshRestored?.root_focus).toBe(rootRef);
    // The projection store of the fresh model never gained the saved preference as rows.
    expect(fresh.getSnapshot().units.length).toBe(0);
    expect(fresh.getSnapshot().evidence.length).toBe(0);

    // Malformed payloads are rejected, not partially admitted.
    expect(() => fresh.importSavedHorizons("{not json")).toThrow();
    expect(() => fresh.importSavedHorizons('{"schema":"other/1","records":[]}')).toThrow();

    model.close();
    fresh.close();
  });
});

describe("T08 Correction Round 1 (peer findings)", () => {
  it("MINOR: a fabricated Horizon fails at the API boundary with the typed error and never starves subscribers", () => {
    const model = seededModel();
    const rootRef = FocusRef.for("gauntlet", "WorkItem", "u-root");
    let notifications = 0;
    model.subscribe(() => {
      notifications += 1;
    });

    // A fabricated horizon-like object must be rejected at the boundary, not
    // silently swallowed inside notifySubscribers' blanket catch.
    const fabricated = { id: 42, root_focus: undefined } as unknown as Horizon;
    expect(() => model.selectHorizon(fabricated)).toThrow(WorkbenchModelError);

    // Subscribers are alive: the next valid interaction still notifies.
    model.hoistToFocus(rootRef, { relation_queries: HAS_EVIDENCE_1 });
    expect(notifications).toBeGreaterThanOrEqual(2); // initial emission + hoist
    model.close();
  });
});
