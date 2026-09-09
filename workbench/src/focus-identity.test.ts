import { describe, expect, it } from "bun:test";
import { FocusRef } from "./focus";
import type { AuthoritativeEventFrame } from "./projection-store";
import type { FullWorkbenchSnapshot } from "./workbench-model";
import { WorkbenchModel } from "./workbench-model";

/**
 * T08 Focus identity and placement suites (plan T08, teeth attack 1).
 * Proves REQ-FOCUS-001, REQ-FOCUS-002, REQ-UX-002 (selection identity), and
 * DSN-006: one object, one FocusRef, across every placement; placement-local
 * state never forks canonical identity.
 */

const GEN = "gen-focus-identity";

function unitEvent(unitId: string, title: string, cursor: string): AuthoritativeEventFrame {
  return {
    generation_id: GEN,
    source_id: "gauntlet",
    cursor,
    event_type: "unit_updated",
    payload: {
      unit_id: unitId,
      title,
      phase: "build",
      status: "active",
      authority: "builder",
      blast_radius: "isolated",
    },
    timestamp_ms: 1000,
  };
}

function seededModel(): WorkbenchModel {
  const model = new WorkbenchModel();
  model.startGeneration(GEN);
  model.applyEvent(unitEvent("u-root", "Root Unit", "c-1"));
  return model;
}

describe("T08 Focus Identity & Placements (REQ-FOCUS-001, REQ-FOCUS-002, REQ-UX-002, DSN-006)", () => {
  it("Attack 1 (REQ-FOCUS-001/002, DSN-006): four placements of one WorkItem resolve to one FocusRef; mutating one placement-local label never forks identity", () => {
    const model = seededModel();
    const ref = FocusRef.for("gauntlet", "WorkItem", "u-root");
    model.selectFocus(ref);

    // The same WorkItem rendered under Plan, Current Run, Agent, and Attention.
    const planPlacement = model.placeFocus("plan", "plan_board", ref);
    const runPlacement = model.placeFocus("work", "current_run", ref);
    const agentPlacement = model.placeFocus("agents", "agent_detail", ref);
    const attentionPlacement = model.placeFocus("attention", "attention_inbox", ref, {
      relation_role: "attention_member",
    });

    // Every placement resolves to the SAME identity — referentially and by id.
    for (const placement of [planPlacement, runPlacement, agentPlacement, attentionPlacement]) {
      expect(placement.focus).toBe(ref);
      expect(placement.focus.id).toBe("focus:gauntlet:WorkItem:u-root");
      expect(placement.focus.source).toBe("gauntlet");
      expect(placement.focus.source_id).toBe("u-root");
    }

    // Distinct occurrence identities, none of which is the Focus identity.
    const placementIds = new Set([
      planPlacement.id,
      runPlacement.id,
      agentPlacement.id,
      attentionPlacement.id,
    ]);
    expect(placementIds.size).toBe(4);
    expect(planPlacement.id.startsWith("placement:")).toBe(true);
    expect(ref.id.startsWith("focus:")).toBe(true);

    // Mutate ONLY one placement-local label.
    const relabeled = model.relabelPlacement(planPlacement.id, "Local plan label");
    expect(relabeled.label_override).toBe("Local plan label");
    expect(relabeled.focus).toBe(ref);

    const snap: FullWorkbenchSnapshot = model.getSnapshot();
    expect(snap.placements.length).toBe(4);
    const byId = new Map(snap.placements.map((placement) => [placement.id, placement]));
    expect(byId.get(planPlacement.id)?.label_override).toBe("Local plan label");
    expect(byId.get(runPlacement.id)?.label_override).toBeUndefined();
    expect(byId.get(agentPlacement.id)?.label_override).toBeUndefined();
    expect(byId.get(attentionPlacement.id)?.label_override).toBeUndefined();

    // No placement label touched the canonical FocusRef: the ref carries no placement
    // label and its identity/fields are unchanged after the relabel.
    expect(ref.human_label).toBeUndefined();
    expect(FocusRef.for("gauntlet", "WorkItem", "u-root")).toBe(ref);

    // Every placement in the published snapshot still resolves to the same FocusRef.
    for (const placement of snap.placements) {
      expect(placement.focus).toBe(ref);
    }

    // No canonical/state fork occurred: one WorkItem row, unchanged title, one selected identity.
    expect(snap.focus).toBe(ref);
    expect(snap.units.length).toBe(1);
    expect(snap.units[0]?.title).toBe("Root Unit");
    expect(snap.units[0]?.source_cursor).toBe("c-1");

    model.close();
  });

  it("FocusRef.for returns the same identity for the same (source, kind, sourceId) triple and never derives identity from labels or visual position", () => {
    const first = FocusRef.for("gauntlet", "WorkItem", "u-1");
    const second = FocusRef.for("gauntlet", "WorkItem", "u-1");
    expect(second).toBe(first);
    expect(first.id).toBe("focus:gauntlet:WorkItem:u-1");

    // Different source or kind means a different identity, even at the same source_id.
    expect(FocusRef.for("prime", "WorkItem", "u-1")).not.toBe(first);
    expect(FocusRef.for("gauntlet", "Evidence", "u-1")).not.toBe(first);

    // Identity is independent of the canonical human label: minting without a label
    // after a labeled mint returns the same labeled identity, not a fork.
    const labeled = FocusRef.for("gauntlet", "WorkItem", "u-label", "Labeled");
    expect(FocusRef.for("gauntlet", "WorkItem", "u-label")).toBe(labeled);
    expect(FocusRef.for("gauntlet", "WorkItem", "u-label").human_label).toBe("Labeled");
  });

  it("Placement identity is never substituted for Focus identity; destroying a placement changes neither other placements nor canonical state", () => {
    const model = seededModel();
    const ref = FocusRef.for("gauntlet", "WorkItem", "u-root");
    model.selectFocus(ref);

    const navigatorPlacement = model.placeFocus("navigator", "tree", ref);
    const inspectorPlacement = model.placeFocus("inspector", "detail", ref);
    expect(navigatorPlacement.id).not.toBe(inspectorPlacement.id);
    expect(navigatorPlacement.focus).toBe(inspectorPlacement.focus);

    const removed = model.removePlacement(navigatorPlacement.id);
    expect(removed).toBe(true);

    const snap = model.getSnapshot();
    expect(snap.placements.length).toBe(1);
    expect(snap.placements[0]?.id).toBe(inspectorPlacement.id);
    expect(snap.placements[0]?.focus).toBe(ref);

    // Canonical state and selection unaffected by the placement destruction.
    expect(snap.units.length).toBe(1);
    expect(snap.units[0]?.unit_id).toBe("u-root");
    expect(snap.focus).toBe(ref);

    expect(model.removePlacement(navigatorPlacement.id)).toBe(false);
    model.close();
  });

  it("Selection retains one identity across views and surfaces (REQ-UX-002)", () => {
    const model = seededModel();
    const ref = FocusRef.for("gauntlet", "WorkItem", "u-root");

    model.placeFocus("navigator", "tree", ref);
    model.placeFocus("trace", "timeline", ref);
    model.selectFocus(ref);

    let snap = model.getSnapshot();
    expect(snap.focus).toBe(ref);
    for (const placement of snap.placements) {
      expect(placement.focus).toBe(ref);
    }

    // Re-selecting via the same triple resolves to the same identity — no churn.
    model.selectFocus(FocusRef.for("gauntlet", "WorkItem", "u-root"));
    snap = model.getSnapshot();
    expect(snap.focus).toBe(ref);

    // A different object is a different identity, visibly distinguishable.
    const other = FocusRef.for("gauntlet", "WorkItem", "u-other");
    model.selectFocus(other);
    expect(model.getSnapshot().focus).toBe(other);
    model.close();
  });
});

describe("T08 Correction Round 1 (peer findings)", () => {
  it("MAJOR-2: focus_changed for an object owned by another source resolves to that object's true identity, never a default-source fork", () => {
    const model = new WorkbenchModel();
    model.startGeneration("gen-correction-2");
    // The unit is owned by 'prime'...
    model.applyEvent({
      generation_id: "gen-correction-2",
      source_id: "prime",
      cursor: "p-1",
      event_type: "unit_updated",
      payload: { unit_id: "u-y", title: "Prime Unit", status: "active" },
      timestamp_ms: 1000,
    });
    // ...and the focus_changed snapshot names it WITHOUT source provenance.
    model.applyEvent({
      generation_id: "gen-correction-2",
      source_id: "prime",
      cursor: "p-2",
      event_type: "focus_changed",
      payload: { kind: "unit", id: "u-y" },
      timestamp_ms: 1100,
    });

    const snap = model.getSnapshot();
    // The snapshot focus must be the SAME interned identity the owning source minted.
    expect(snap.focus).toBe(FocusRef.for("prime", "WorkItem", "u-y"));
    expect(snap.focus?.id).toBe("focus:prime:WorkItem:u-y");
    // One identity across selection and materialization — no fork.
    const workingSetMatch = model
      .getMaterializationPlan()
      .working_set.find((f) => f.id === "focus:prime:WorkItem:u-y");
    expect(workingSetMatch).toBeDefined();
    expect(snap.focus).toBe(workingSetMatch ?? null);
    model.close();
  });

  it("MAJOR-2: an unattributable focus_changed is refused with a typed reason and mints no identity", () => {
    const model = new WorkbenchModel();
    model.startGeneration("gen-correction-2b");
    // A unit owned by 'prime' exists so the world is non-empty; unit_id is the
    // store's primary key, so an id can never belong to two sources at once —
    // the real unattributable cases are dangling ids and kinds with no
    // projected row source.
    model.applyEvent({
      generation_id: "gen-correction-2b",
      source_id: "prime",
      cursor: "p-1",
      event_type: "unit_updated",
      payload: { unit_id: "u-z", title: "Prime Z", status: "active" },
      timestamp_ms: 1000,
    });

    // Dangling: focus_changed naming an object with no projected row is refused.
    const dangling = model.applyEvent({
      generation_id: "gen-correction-2b",
      source_id: "gauntlet",
      cursor: "c-1",
      event_type: "focus_changed",
      payload: { kind: "unit", id: "u-ghost" },
      timestamp_ms: 1100,
    });
    expect(dangling.applied).toBe(false);
    expect(dangling.reason).toContain("unattributable_focus_change");

    // Unsupported kind: no projected row source can attribute it — refused.
    const unsupportedKind = model.applyEvent({
      generation_id: "gen-correction-2b",
      source_id: "gauntlet",
      cursor: "c-2",
      event_type: "focus_changed",
      payload: { kind: "file", id: "f-1" },
      timestamp_ms: 1200,
    });
    expect(unsupportedKind.applied).toBe(false);
    expect(unsupportedKind.reason).toContain("unattributable_focus_change");

    // Malformed payload: no usable id — refused.
    const malformed = model.applyEvent({
      generation_id: "gen-correction-2b",
      source_id: "gauntlet",
      cursor: "c-3",
      event_type: "focus_changed",
      payload: {},
      timestamp_ms: 1300,
    });
    expect(malformed.applied).toBe(false);
    expect(malformed.reason).toContain("unattributable_focus_change");

    // No default-source fallback was ever minted for the refused frames.
    expect(model.getSnapshot().focus).toBeNull();

    // Refusals do not poison the model: a well-attributed frame still applies.
    const admitted = model.applyEvent({
      generation_id: "gen-correction-2b",
      source_id: "prime",
      cursor: "p-2",
      event_type: "focus_changed",
      payload: { kind: "unit", id: "u-z" },
      timestamp_ms: 1400,
    });
    expect(admitted.applied).toBe(true);
    expect(model.getSnapshot().focus).toBe(FocusRef.for("prime", "WorkItem", "u-z"));
    model.close();
  });
});
