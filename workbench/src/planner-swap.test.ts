import { describe, expect, it } from "bun:test";
import { FocusRef } from "./focus";
import { planMaterialization } from "./horizon";
import type { WorkbenchSnapshot } from "./projection-store";
import type { ProjectionWorld } from "./relations";
import { GEN } from "./testing/shell-harness";
import { WorkbenchModel } from "./workbench-model";
import {
  BunWorkerFoldExecutor,
  createSyntheticCaseEvents,
  type FoldOutcome,
  InlineFoldExecutor,
  ScriptedFoldExecutor,
  SYNTHETIC_CASE_SEED,
  type SyntheticCaseSpec,
  worldFromSlices,
} from "./worker-folds";

/**
 * T20 planner-swap regression suites (T17 MAJOR obligation discharged here).
 *
 * The MaterializationFoldCoordinator is wired into the PRODUCTION snapshot
 * path of WorkbenchModel: the first snapshot read on a stale context folds
 * inline (the immediate fallback — the model NEVER blocks rendering on the
 * worker) and schedules an asynchronous worker refresh; once the worker's
 * plan is admitted, snapshots are served from that cache for the exact
 * current (generation, horizon, world) context until the context changes.
 *
 * The coordinator's own semantics stay pinned by T17's worker-folds.test.ts;
 * these suites pin the MODEL-side wiring. The default model (no executor) is
 * byte-identical to its pre-T20 behavior.
 */

const SMALL: SyntheticCaseSpec = Object.freeze({
  generation: "gen-t20-swap",
  units: 12,
  evidence_per_unit: 1,
  sessions: 4,
  blocked_units: 2,
  focus_events: 0,
  seed: SYNTHETIC_CASE_SEED,
});

function seedModel(spec: SyntheticCaseSpec): WorkbenchModel {
  const model = new WorkbenchModel();
  model.startGeneration(spec.generation);
  for (const frame of createSyntheticCaseEvents(spec)) {
    const result = model.applyEvent(frame);
    if (!result.applied) {
      throw new Error(`fixture event rejected by the model: ${result.reason ?? "unknown"}`);
    }
  }
  return model;
}

function worldOf(model: WorkbenchModel): ProjectionWorld {
  const base: WorkbenchSnapshot = model.store.getSnapshot(null);
  return worldFromSlices(
    base.generation,
    base.units,
    base.evidence,
    base.sessions,
    base.source_cursors,
  );
}

/** The inline plan an unwired model would compute right now (the reference). */
function inlinePlanOf(model: WorkbenchModel): ReturnType<typeof planMaterialization> {
  return planMaterialization(worldOf(model), null);
}

/** A scripted snapshot outcome for the first task of a generation, carrying a real plan. */
function acceptedOutcome(
  generation: string,
  plan: ReturnType<typeof planMaterialization>,
): FoldOutcome {
  return {
    kind: "snapshot",
    task_id: `fold:${generation}:1`,
    generation,
    result: plan,
  };
}

describe("T20 planner swap: default model stays byte-identical inline", () => {
  it("without a fold executor the snapshot equals the inline fold exactly and no worker plan exists", () => {
    const model = seedModel(SMALL);
    const reference = inlinePlanOf(model);
    const snapshot = model.getSnapshot();

    expect(model.workerPlan).toBeNull();
    expect(model.foldStats).toBeNull();
    expect(snapshot.materialization.horizon_id).toBe(reference.horizon_id);
    expect(snapshot.materialization.working_set_size).toBe(reference.working_set.length);
    expect(snapshot.materialization.counters).toEqual(reference.counters);
    expect(snapshot.context_candidates).toEqual(reference.context_candidates);

    // Byte-identical to a second, unwired model over the same fixture.
    const twin = seedModel(SMALL);
    expect(JSON.stringify(twin.getSnapshot().materialization)).toBe(
      JSON.stringify(snapshot.materialization),
    );
  });
});

describe("T20 planner swap: the wired snapshot path", () => {
  it("the first read on a stale context folds INLINE and schedules exactly one worker refresh", async () => {
    const executor = new ScriptedFoldExecutor("worker", []); // no outcomes queued
    const model = new WorkbenchModel({ foldExecutor: executor });
    model.startGeneration(SMALL.generation);
    for (const frame of createSyntheticCaseEvents(SMALL)) {
      model.applyEvent(frame);
    }

    const reference = inlinePlanOf(model);
    const snapshot = model.getSnapshot();
    // Inline fallback served THIS snapshot completely, never a placeholder.
    expect(snapshot.materialization.working_set_size).toBe(reference.working_set.length);
    expect(snapshot.materialization.counters).toEqual(reference.counters);
    expect(model.workerPlan).toBeNull();

    // The refresh is dispatched through the coordinator (asynchronously —
    // the submit handoff installs the world first).
    await Bun.sleep(1);
    expect(executor.dispatched.length).toBe(1);
    const dispatch = executor.dispatched[0];
    expect(dispatch?.generation).toBe(SMALL.generation);
    expect(dispatch?.horizonJson).toBeNull();

    // Reads that do not change the plan context do NOT reschedule: the
    // refresh for this context is already in flight.
    model.getSnapshot();
    model.getSnapshot();
    await Bun.sleep(1);
    expect(executor.dispatched.length).toBe(1);
  });

  it("once the worker plan lands, interactions are served FROM CACHE with no re-dispatch", async () => {
    const executor = new ScriptedFoldExecutor("worker", [
      acceptedOutcome(SMALL.generation, inlinePlanOf(seedModel(SMALL))),
    ]);
    const model = new WorkbenchModel({ foldExecutor: executor });
    model.startGeneration(SMALL.generation);
    for (const frame of createSyntheticCaseEvents(SMALL)) {
      model.applyEvent(frame);
    }

    // First read: inline fallback + dispatch; the scripted outcome is
    // delivered (asynchronously) by the executor and admitted.
    model.getSnapshot();
    await Bun.sleep(1);
    expect(executor.dispatched.length).toBe(1);
    expect(model.workerPlan).not.toBeNull();
    expect(model.foldStats?.accepted).toBe(1);

    // Interactions (focus selection, repeated reads) hit the cache.
    const firstUnit = model.getUnits()[0];
    if (firstUnit === undefined) throw new Error("fixture has no units");
    model.selectFocus(FocusRef.for(firstUnit.source_id, "WorkItem", firstUnit.unit_id));
    const afterInteraction = executor.dispatched.length;
    model.getSnapshot();
    model.getSnapshot();
    expect(executor.dispatched.length).toBe(afterInteraction);

    // The served snapshot IS the cached worker plan, equal to the inline fold
    // over the same world (worker placement changes placement, not meaning).
    const snapshot = model.getSnapshot();
    const reference = inlinePlanOf(model);
    expect(snapshot.materialization.counters).toEqual(reference.counters);
    expect(snapshot.context_candidates.map((candidate) => candidate.focus.id)).toEqual(
      reference.context_candidates.map((candidate) => candidate.focus.id),
    );
  });

  it("a world change invalidates the cache immediately: the fresh snapshot folds inline and re-refreshes", async () => {
    const executor = new ScriptedFoldExecutor("worker", [
      acceptedOutcome(SMALL.generation, inlinePlanOf(seedModel(SMALL))),
    ]);
    const model = new WorkbenchModel({ foldExecutor: executor });
    model.startGeneration(SMALL.generation);
    for (const frame of createSyntheticCaseEvents(SMALL)) {
      model.applyEvent(frame);
    }
    model.getSnapshot();
    await Bun.sleep(1);
    expect(model.workerPlan).not.toBeNull();

    // New authoritative event: the accepted plan no longer describes the
    // world — it must NOT be served.
    model.applyEvent({
      generation_id: SMALL.generation,
      source_id: "gauntlet",
      cursor: "t20-extra",
      event_type: "unit_updated",
      payload: { unit_id: "u-new", title: "Post-plan unit", status: "blocked" },
      timestamp_ms: 9999,
    });
    expect(model.workerPlan).toBeNull();

    const snapshot = model.getSnapshot();
    expect(snapshot.units.some((unit) => unit.unit_id === "u-new")).toBe(true);
    // The re-fold includes the new blocked unit in attention, and a new
    // refresh was scheduled for the changed context.
    expect(snapshot.attention.some((entry) => entry.focus.source_id === "u-new")).toBe(true);
    await Bun.sleep(1);
    expect(executor.dispatched.length).toBe(2);
  });

  it("listener re-publish: an admitted worker plan notifies subscribers with the refreshed snapshot", async () => {
    const executor = new InlineFoldExecutor();
    const model = new WorkbenchModel({ foldExecutor: executor });
    model.startGeneration(GEN);
    for (const frame of harnessFrames()) {
      model.applyEvent(frame);
    }
    model.markReady(GEN);

    let notifications = 0;
    let lastWorkingSet = -1;
    model.subscribe((snapshot) => {
      notifications += 1;
      lastWorkingSet = snapshot.materialization.working_set_size;
    });

    // The subscriber's initial synchronous read folded inline and scheduled
    // the refresh; the inline executor delivers its outcome asynchronously
    // (one microtask later), so the re-publication arrives after a yield.
    await Bun.sleep(10);

    expect(model.workerPlan).not.toBeNull();
    expect(notifications).toBeGreaterThan(1); // initial + refreshed
    expect(lastWorkingSet).toBeGreaterThan(0);
    model.close();
    executor.dispose();
  });

  it("a stale worker result is discarded and the inline path keeps serving (never regresses)", async () => {
    const stale: FoldOutcome = {
      kind: "snapshot",
      task_id: "fold:gen-superseded:1",
      generation: "gen-superseded",
      result: inlinePlanOf(seedModel(SMALL)),
    };
    const executor = new ScriptedFoldExecutor("worker", [stale]);
    const model = new WorkbenchModel({ foldExecutor: executor });
    model.startGeneration(SMALL.generation);
    for (const frame of createSyntheticCaseEvents(SMALL)) {
      model.applyEvent(frame);
    }

    const snapshot = model.getSnapshot();
    await Bun.sleep(1);
    const reference = inlinePlanOf(model);
    expect(model.foldStats?.stale_discarded).toBe(1);
    expect(model.workerPlan).toBeNull();
    // The snapshot is still complete and correct through the inline fallback.
    expect(snapshot.materialization.working_set_size).toBe(reference.working_set.length);
  });

  it("a worker that never answers never blocks the render path (inline serves every read)", () => {
    const executor = new ScriptedFoldExecutor("worker", []); // delivers nothing
    const model = new WorkbenchModel({ foldExecutor: executor });
    model.startGeneration(SMALL.generation);
    for (const frame of createSyntheticCaseEvents(SMALL)) {
      model.applyEvent(frame);
    }

    for (let i = 0; i < 5; i += 1) {
      const snapshot = model.getSnapshot();
      const reference = inlinePlanOf(model);
      expect(snapshot.materialization.counters).toEqual(reference.counters);
    }
    expect(model.workerPlan).toBeNull();
  });

  it("hoisting invalidates a whole-Case worker plan until the hoisted refresh lands", async () => {
    const executor = new InlineFoldExecutor();
    const model = new WorkbenchModel({ foldExecutor: executor });
    model.startGeneration(GEN);
    for (const frame of harnessFrames()) {
      model.applyEvent(frame);
    }
    model.markReady(GEN);
    model.getSnapshot();
    await Bun.sleep(10);
    const wholeCase = model.workerPlan;
    expect(wholeCase).not.toBeNull();

    // Hoist: the whole-Case plan must NOT serve for the hoisted horizon.
    const target = model.getUnits().find((unit) => unit.unit_id === "u-root");
    if (target === undefined) throw new Error("fixture unit u-root missing");
    const ref = FocusRef.for(target.source_id, "WorkItem", target.unit_id);
    model.selectFocus(ref);
    model.hoistToFocus(ref);
    await Bun.sleep(20);

    const hoisted = model.workerPlan;
    if (hoisted !== null) {
      // Only a plan for the HOISTED horizon may serve; the horizon_id pins
      // the context, so the whole-Case plan can never leak into the hoist.
      expect(hoisted.horizon_id).not.toBeNull();
      expect(hoisted.horizon_id).not.toBe(wholeCase?.horizon_id ?? null);
    }
    const snapshot = model.getSnapshot();
    expect(snapshot.active_horizon).not.toBeNull();
    expect(snapshot.materialization.horizon_id).toBe(snapshot.active_horizon?.id ?? null);

    model.close();
    executor.dispose();
  });
});

describe("T20 planner swap: real Bun worker end-to-end through the model", () => {
  it("the production executor serves the first read inline, then the worker plan lands and equals the inline fold", async () => {
    const executor = new BunWorkerFoldExecutor();
    try {
      const model = new WorkbenchModel({ foldExecutor: executor });
      model.startGeneration(GEN);
      for (const frame of harnessFrames()) {
        model.applyEvent(frame);
      }
      model.markReady(GEN);

      // The FIRST read is synchronous and complete (inline fallback) — the
      // render path never waits for the worker.
      const immediate = model.getSnapshot();
      expect(immediate.materialization.working_set_size).toBeGreaterThan(0);
      expect(model.workerPlan).toBeNull();

      // The worker plan lands and is admitted; it equals the reference inline
      // fold over the same world, with interned FocusRef identities.
      const deadline = Date.now() + 5000;
      while (model.workerPlan === null && Date.now() < deadline) {
        await Bun.sleep(5);
      }
      const workerPlan = model.workerPlan;
      expect(workerPlan).not.toBeNull();
      const reference = planMaterialization(worldOf(model), null);
      expect(workerPlan?.working_set.map((focus) => focus.id)).toEqual(
        reference.working_set.map((focus) => focus.id),
      );
      expect(workerPlan?.counters).toEqual(reference.counters);
      // Identity interning: the worker's plan refs ARE the model's identities.
      const firstRef = workerPlan?.working_set[0];
      if (firstRef !== undefined) {
        expect(FocusRef.for(firstRef.source, firstRef.kind, firstRef.source_id).id).toBe(
          firstRef.id,
        );
      }
      model.close();
    } finally {
      executor.dispose();
    }
  });
});

/** The shell-harness seeded fixture's exact frame list (kept for wired-model replay). */
function harnessFrames(): Parameters<WorkbenchModel["applyEvent"]>[0][] {
  return [
    {
      generation_id: GEN,
      source_id: "gauntlet",
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
    },
    {
      generation_id: GEN,
      source_id: "gauntlet",
      cursor: "c-2",
      event_type: "evidence_added",
      payload: { evidence_id: "ev-1", unit_id: "u-root", kind: "report", verified: true },
      timestamp_ms: 1100,
    },
    {
      generation_id: GEN,
      source_id: "gauntlet",
      cursor: "c-3",
      event_type: "evidence_added",
      payload: { evidence_id: "ev-2", unit_id: "u-root", kind: "log", verified: false },
      timestamp_ms: 1200,
    },
    {
      generation_id: GEN,
      source_id: "gauntlet",
      cursor: "c-4",
      event_type: "unit_updated",
      payload: {
        unit_id: "u-other",
        title: "Other Unit",
        status: "active",
        authority: "builder",
        blast_radius: "isolated",
      },
      timestamp_ms: 1000,
    },
    {
      generation_id: GEN,
      source_id: "gauntlet",
      cursor: "c-5",
      event_type: "evidence_added",
      payload: { evidence_id: "ev-3", unit_id: "u-other", kind: "report", verified: true },
      timestamp_ms: 1400,
    },
    {
      generation_id: GEN,
      source_id: "gauntlet",
      cursor: "c-6",
      event_type: "unit_updated",
      payload: {
        unit_id: "u-far",
        title: "Far Unit",
        status: "blocked",
        authority: "orchestrator",
        blast_radius: "isolated",
      },
      timestamp_ms: 1000,
    },
    {
      generation_id: GEN,
      source_id: "gauntlet",
      cursor: "c-7",
      event_type: "session_updated",
      payload: { session_id: "s-1", parent_id: null, model: "model-a", status: "running" },
      timestamp_ms: 1600,
    },
    {
      generation_id: GEN,
      source_id: "gauntlet",
      cursor: "c-8",
      event_type: "session_updated",
      payload: { session_id: "s-2", parent_id: "s-1", model: "model-a", status: "running" },
      timestamp_ms: 1700,
    },
    {
      generation_id: GEN,
      source_id: "gauntlet",
      cursor: "c-9",
      event_type: "unit_updated",
      payload: {
        unit_id: "u-root",
        title: "Root Unit",
        status: "active",
        authority: "builder",
        blast_radius: "isolated",
      },
      timestamp_ms: 1800,
    },
  ];
}
