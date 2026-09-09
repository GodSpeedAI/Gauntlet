import { describe, expect, it } from "bun:test";
import { FocusRef } from "./focus";
import { DEFAULT_HOIST_RELATION_QUERIES, horizonToJSON, type MaterializationPlan } from "./horizon";
import type { WorkbenchSnapshot } from "./projection-store";
import type { ProjectionWorld } from "./relations";
import { WorkbenchModel } from "./workbench-model";
import {
  BunWorkerFoldExecutor,
  createSyntheticCaseEvents,
  decidePlacement,
  type FoldOutcome,
  FROZEN_BUDGETS,
  InlineFoldExecutor,
  isFoldWorkerResponse,
  MaterializationFoldCoordinator,
  placementDecisionsAreConsistent,
  RECORDED_PLACEMENT_DECISIONS,
  readmitPlanResult,
  recordedPlacementFor,
  ScriptedFoldExecutor,
  SYNTHETIC_CASE_SEED,
  type SyntheticCaseSpec,
  serializeFocusRef,
  serializeHorizonForTask,
  serializePlanResult,
  T17_LARGE_FIXTURE,
  T17_SMALL_FIXTURE,
  worldFromSlices,
} from "./worker-folds";

/**
 * T17 worker-handoff suites (REQ-BUN-006, REQ-GOAL-006, REQ-FOCUS-003/004).
 *
 * Determinism discipline (task/prereg): the deterministic fold semantics run
 * through the synchronous in-process executors (InlineFoldExecutor /
 * ScriptedFoldExecutor); the real Bun worker is exercised end-to-end once so
 * the handoff contract is proven against the actual transport, with the bulk
 * of load/falsifier coverage on the real model at unit-test scale. The LARGE
 * frozen fixture's 200-cycle stream and percentile budgets live in the two
 * T17 benches, not here.
 */

const HAS_EVIDENCE_1 = DEFAULT_HOIST_RELATION_QUERIES;

/** Declared unit-test load fixture (smaller than the frozen LARGE bench fixture). */
const LOAD_FIXTURE: SyntheticCaseSpec = Object.freeze({
  generation: "gen-t17-load",
  units: 800,
  evidence_per_unit: 1,
  sessions: 60,
  blocked_units: 5,
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

function collectOutcomes(executor: InlineFoldExecutor | ScriptedFoldExecutor): FoldOutcome[] {
  const outcomes: FoldOutcome[] = [];
  executor.onOutcome((outcome) => outcomes.push(outcome));
  return outcomes;
}

/** noUncheckedIndexedAccess-safe accessor that preserves the failure locus. */
function outcomeAt(outcomes: readonly FoldOutcome[], index: number): FoldOutcome {
  const outcome = outcomes[index];
  if (outcome === undefined) {
    throw new Error(`expected a scripted outcome at index ${index}`);
  }
  return outcome;
}

async function waitFor(predicate: () => boolean, timeoutMs = 5000): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  while (!predicate()) {
    if (Date.now() > deadline) {
      throw new Error("waitFor: condition not met within timeout");
    }
    await new Promise((resolve) => setTimeout(resolve, 5));
  }
}

describe("T17 deterministic synthetic Case generator", () => {
  it("identical specs yield identical events; counts match the frozen fixture declarations", () => {
    const small = createSyntheticCaseEvents(T17_SMALL_FIXTURE);
    const again = createSyntheticCaseEvents(T17_SMALL_FIXTURE);
    expect(small).toEqual(again);
    expect(small.length).toBe(10); // 3 units + 3 evidence + 4 sessions + 0 focus

    const large = createSyntheticCaseEvents(T17_LARGE_FIXTURE);
    expect(large).toEqual(createSyntheticCaseEvents(T17_LARGE_FIXTURE));
    // Frozen declared totals (evidence T17/horizon-payment.md): 6,000 units +
    // 6,000 evidence + 200 sessions + 10 focus_changed = 12,210 events.
    expect(large.length).toBe(12_210);

    // Blocked band sits at the head of the id space; the rest never blocked.
    const blockedIds = new Set(
      large
        .filter((frame) => frame.event_type === "unit_updated")
        .filter((frame) => frame.payload["status"] === "blocked")
        .map((frame) => frame.payload["unit_id"]),
    );
    expect(blockedIds.size).toBe(T17_LARGE_FIXTURE.blocked_units);
    expect(blockedIds.has("u-0")).toBe(true);
    expect(blockedIds.has(`u-${T17_LARGE_FIXTURE.blocked_units}`)).toBe(false);

    // Session trees: two parent chains, roots parentless.
    const sessionEvents = large.filter((frame) => frame.event_type === "session_updated");
    expect(sessionEvents.length).toBe(T17_LARGE_FIXTURE.sessions);
    const root = sessionEvents.find((frame) => frame.payload["session_id"] === "s-0-0");
    expect(root?.payload["parent_id"]).toBeNull();
    const child = sessionEvents.find((frame) => frame.payload["session_id"] === "s-1-5");
    expect(child?.payload["parent_id"]).toBe("s-1-4");
  });
});

describe("T17 fold handoff: snapshot semantics (deterministic inline executor)", () => {
  it("a hoisted fold through the contract is identical to the model's inline fold, with interned identities", async () => {
    const model = seedModel(T17_SMALL_FIXTURE);
    const horizon = model.hoistToFocus(FocusRef.for("gauntlet", "WorkItem", "u-0"), {
      relation_queries: HAS_EVIDENCE_1,
    });
    const modelPlan = model.getMaterializationPlan();

    const executor = new InlineFoldExecutor();
    const receipt = await executor.installWorld(worldOf(model));
    expect(receipt).toEqual({
      generation: T17_SMALL_FIXTURE.generation,
      units: 3,
      evidence: 3,
      sessions: 4,
    });
    const outcomes = collectOutcomes(executor);
    executor.dispatchPlan(
      T17_SMALL_FIXTURE.generation,
      serializeHorizonForTask(horizon),
      "task-eq-hoisted",
    );

    expect(outcomes.length).toBe(1);
    const outcome = outcomes[0];
    expect(outcome?.kind).toBe("snapshot");
    if (outcome?.kind !== "snapshot") {
      throw new Error("expected a snapshot outcome");
    }
    expect(outcome.task_id).toBe("task-eq-hoisted");
    expect(outcome.generation).toBe(T17_SMALL_FIXTURE.generation);

    // Full boundary round trip: the wire document re-admitted into a plan is
    // indistinguishable from the model's own inline fold.
    const roundtrip = readmitPlanResult(serializePlanResult(outcome.result));
    expect(roundtrip).toEqual(modelPlan);
    expect(roundtrip.counters).toEqual(modelPlan.counters);
    expect(roundtrip.working_set.map((focus) => focus.id)).toEqual(
      modelPlan.working_set.map((focus) => focus.id),
    );
    expect(roundtrip.subscriptions).toEqual(modelPlan.subscriptions);
    expect(roundtrip.queries).toEqual(modelPlan.queries);
    expect(roundtrip.prefetch_keys).toEqual(modelPlan.prefetch_keys);
    expect(roundtrip.context_candidates).toEqual(modelPlan.context_candidates);

    // Identity discipline: every re-admitted ref is THE interned instance the
    // model holds — the boundary never forks identity (DSN-006).
    for (const ref of roundtrip.working_set) {
      expect(ref).toBe(FocusRef.for(ref.source, ref.kind, ref.source_id));
    }
    for (const ref of modelPlan.working_set) {
      expect(roundtrip.working_set.find((candidate) => candidate.id === ref.id)).toBe(ref);
    }
    executor.dispose();
    model.close();
  });

  it("a whole-Case (null horizon) fold through the contract equals the model's unhoisted plan", async () => {
    const model = seedModel(T17_SMALL_FIXTURE);
    const modelPlan = model.getMaterializationPlan();
    expect(modelPlan.horizon_id).toBeNull();

    const executor = new InlineFoldExecutor();
    await executor.installWorld(worldOf(model));
    const outcomes = collectOutcomes(executor);
    executor.dispatchPlan(
      T17_SMALL_FIXTURE.generation,
      serializeHorizonForTask(null),
      "task-eq-case",
    );

    expect(outcomes.length).toBe(1);
    const outcome = outcomes[0];
    if (outcome?.kind !== "snapshot") {
      throw new Error("expected a snapshot outcome");
    }
    expect(readmitPlanResult(serializePlanResult(outcome.result))).toEqual(modelPlan);
    executor.dispose();
    model.close();
  });

  it("a fold dispatched for an uninstalled generation is a preserved error, never applied", async () => {
    const executor = new InlineFoldExecutor();
    const outcomes = collectOutcomes(executor);
    executor.dispatchPlan("gen-never-installed", null, "task-missing");
    expect(outcomes.length).toBe(1);
    expect(outcomes[0]?.kind).toBe("error");
    if (outcomes[0]?.kind === "error") {
      expect(outcomes[0].reason).toBe("world_not_installed");
    }
    executor.dispose();
  });

  it("the serialized horizon crosses the boundary in its canonical JSON form", () => {
    const model = seedModel(T17_SMALL_FIXTURE);
    const horizon = model.hoistToFocus(FocusRef.for("gauntlet", "WorkItem", "u-0"), {
      relation_queries: HAS_EVIDENCE_1,
    });
    expect(serializeHorizonForTask(null)).toBeNull();
    expect(serializeHorizonForTask(horizon)).toBe(horizonToJSON(horizon));
    model.close();
  });
});

describe("T17 fold handoff: invalidation semantics", () => {
  it("results for a superseded generation, a superseded task, errors, and explicit invalidations are discarded; only current snapshots are applied", async () => {
    const model = seedModel(T17_SMALL_FIXTURE);
    const world = worldOf(model);
    const executor = new ScriptedFoldExecutor("worker", [
      { kind: "snapshot", task_id: "fold:gen-A:1", generation: "gen-A", result: inlinePlan(model) },
      { kind: "snapshot", task_id: "fold:gen-A:1", generation: "gen-A", result: inlinePlan(model) },
      {
        kind: "snapshot",
        task_id: "fold:gen-A:999",
        generation: "gen-A",
        result: inlinePlan(model),
      },
      { kind: "error", task_id: "fold:gen-A:4", generation: "gen-A", reason: "fold_failed: boom" },
      { kind: "invalidation", task_id: null, generation: "gen-A", reason: "world_dropped" },
    ]);
    const collected = collectOutcomes(executor);
    const coordinator = new MaterializationFoldCoordinator(executor);

    // 1. Current task, current generation: applied.
    await coordinator.submit(world, null, "gen-A");
    expect(coordinator.accept(outcomeAt(collected, 0), "gen-A").status).toBe("applied");
    const appliedPlan = coordinator.acceptedPlan;
    expect(appliedPlan).not.toBeNull();

    // 2. Result computed for the superseded generation: stale, never applied.
    await coordinator.submit(world, null, "gen-A");
    const staleGen = coordinator.accept(outcomeAt(collected, 1), "gen-B");
    expect(staleGen.status).toBe("stale_discarded");
    expect(staleGen.reason).toContain("superseded_generation");
    expect(staleGen.plan).toBe(appliedPlan);

    // 3. Result for a task that is no longer the latest: stale.
    await coordinator.submit(world, null, "gen-A");
    const staleTask = coordinator.accept(outcomeAt(collected, 2), "gen-A");
    expect(staleTask.status).toBe("stale_discarded");
    expect(staleTask.reason).toContain("superseded_task");
    expect(staleTask.plan).toBe(appliedPlan);

    // 4. Errors are preserved, not converted into success.
    await coordinator.submit(world, null, "gen-A");
    const error = coordinator.accept(outcomeAt(collected, 3), "gen-A");
    expect(error.status).toBe("error_discarded");
    expect(error.reason).toBe("fold_failed: boom");
    expect(error.plan).toBe(appliedPlan);

    // 5. Explicit invalidation outcomes are discarded with their reason.
    await coordinator.submit(world, null, "gen-A");
    const invalidated = coordinator.accept(outcomeAt(collected, 4), "gen-A");
    expect(invalidated.status).toBe("stale_discarded");
    expect(invalidated.reason).toBe("world_dropped");

    expect(coordinator.stats).toEqual({
      submitted: 5,
      accepted: 1,
      stale_discarded: 3,
      error_discarded: 1,
    });
    expect(coordinator.acceptedPlan).toBe(appliedPlan);
    executor.dispose();
    model.close();
  });

  it("attach() accepts against the LIVE generation: results arriving after a generation advance are dropped automatically", async () => {
    const model = seedModel(T17_SMALL_FIXTURE);
    const planA = model.getMaterializationPlan();
    // Outcome #2 is a LATE result for the old generation: it is scripted onto
    // the second dispatch and delivered after the generation advanced —
    // exactly the in-flight race the worker placement must survive.
    const executor = new ScriptedFoldExecutor("worker", [
      { kind: "snapshot", task_id: "fold:gen-A:1", generation: "gen-A", result: planA },
      { kind: "snapshot", task_id: "fold:gen-A:1", generation: "gen-A", result: planA },
    ]);
    const coordinator = new MaterializationFoldCoordinator(executor);

    let activeGeneration = "gen-A";
    coordinator.attach(() => activeGeneration);
    await coordinator.submit(worldOf(model), null, activeGeneration);
    expect(coordinator.acceptedPlan).toBe(planA);

    // The model advances its generation; the stale in-flight result for the
    // old one lands and must be discarded without regressing the plan.
    activeGeneration = "gen-B";
    await coordinator.submit(worldOf(model), null, activeGeneration);
    expect(coordinator.acceptedPlan).toBe(planA);
    expect(coordinator.stats.stale_discarded).toBe(1);
    expect(coordinator.stats.accepted).toBe(1);
    executor.dispose();
    model.close();
  });

  it("worlds are installed once per generation and superseded generations are dropped", async () => {
    const model = seedModel(T17_SMALL_FIXTURE);
    const executor = new ScriptedFoldExecutor("worker", []);
    const coordinator = new MaterializationFoldCoordinator(executor);
    const world = worldOf(model);

    await coordinator.submit(world, null, "gen-A");
    await coordinator.submit(world, null, "gen-A"); // same generation: no reinstall, no drop
    expect(executor.droppedWorlds).toEqual([]);

    await coordinator.submit(world, null, "gen-B");
    expect(executor.droppedWorlds).toEqual(["gen-A"]);
    executor.dispose();
    model.close();
  });
});

function inlinePlan(model: WorkbenchModel): MaterializationPlan {
  return model.getMaterializationPlan();
}

describe("T17 counters instrument: data logistics follow the Horizon under load", () => {
  const CYCLES = 60;

  it("through the real model: hoisting narrows every counter below the frozen fraction; de-hoist restores exactly, with zero drift and no identity churn", () => {
    const model = seedModel(LOAD_FIXTURE);
    const whole = model.getMaterializationPlan();
    expect(whole.horizon_id).toBeNull();
    const wholeCounters = whole.counters;
    const wholeIds = whole.working_set.map((focus) => focus.id);

    const seenRefs: FocusRef[] = [];
    for (let cycle = 0; cycle < CYCLES; cycle += 1) {
      const leafUnit = 100 + cycle;
      const leaf = FocusRef.for("gauntlet", "WorkItem", `u-${leafUnit}`);
      model.hoistToFocus(leaf, { relation_queries: HAS_EVIDENCE_1 });
      const hoisted = model.getMaterializationPlan();

      // B3 (frozen): hoisted subscriptions are at most 1% of whole-Case.
      expect(
        hoisted.counters.subscriptions,
        `cycle ${cycle}: hoisted subscriptions must stay within the frozen narrowing fraction`,
      ).toBeLessThanOrEqual(
        wholeCounters.subscriptions * FROZEN_BUDGETS.hoist_narrowing_max_fraction,
      );
      expect(hoisted.counters.subscriptions).toBeLessThan(wholeCounters.subscriptions);
      // The hoisted closure contains this cycle's leaf and its evidence — and
      // never the far Case (prereg P1 falsifier, asserted positively here).
      const hoistedIds = hoisted.working_set.map((focus) => focus.id);
      expect(hoistedIds).toContain(`focus:gauntlet:WorkItem:u-${leafUnit}`);
      expect(hoistedIds).toContain(`focus:gauntlet:Evidence:ev-${leafUnit}-0`);
      expect(hoistedIds).not.toContain("focus:gauntlet:WorkItem:u-799");
      expect(hoistedIds.length).toBeLessThan(10);

      seenRefs.push(...hoisted.working_set);

      model.dehoist();
      const restored = model.getMaterializationPlan();
      // Zero drift: every de-hoist restores the exact whole-Case plan.
      expect(restored.counters).toEqual(wholeCounters);
      expect(restored.working_set.map((focus) => focus.id)).toEqual(wholeIds);
    }

    // No identity churn anywhere in the replay (T08 invariant under load).
    for (const ref of seenRefs) {
      expect(ref).toBe(FocusRef.for(ref.source, ref.kind, ref.source_id));
    }
    model.close();
  });

  it("through the coordinator: accepted hoisted plans narrow and accepted whole-Case plans restore, cycle after cycle", async () => {
    const model = seedModel(LOAD_FIXTURE);
    const executor = new InlineFoldExecutor();
    const coordinator = new MaterializationFoldCoordinator(executor);
    coordinator.attach(() => LOAD_FIXTURE.generation);
    const world = worldOf(model);
    const wholeCounters = model.getMaterializationPlan().counters;

    for (let cycle = 0; cycle < 30; cycle += 1) {
      const horizon = model.hoistToFocus(FocusRef.for("gauntlet", "WorkItem", `u-${200 + cycle}`), {
        relation_queries: HAS_EVIDENCE_1,
      });
      const dispatch = await coordinator.submit(world, horizon, LOAD_FIXTURE.generation);
      expect(dispatch.task_id).toContain(LOAD_FIXTURE.generation);
      const hoisted = coordinator.acceptedPlan;
      expect(hoisted).not.toBeNull();
      expect(hoisted?.horizon_id).toBe(horizon.id);
      expect(hoisted?.counters.subscriptions ?? 0).toBeLessThan(
        wholeCounters.subscriptions * FROZEN_BUDGETS.hoist_narrowing_max_fraction,
      );

      model.dehoist();
      await coordinator.submit(world, null, LOAD_FIXTURE.generation);
      expect(coordinator.acceptedPlan?.counters).toEqual(wholeCounters);
    }
    // Every submit under inline placement paid the synchronous fold (that IS
    // the honest inline render-path cost the worker placement is compared to).
    expect(coordinator.stats.submitted).toBe(60);
    expect(coordinator.stats.accepted).toBe(60);
    expect(coordinator.stats.stale_discarded).toBe(0);
    executor.dispose();
    model.close();
  });

  it("Falsifier by construction (prereg P1, plan tooth 1): asserting whole-graph subscription after a hoist FAILS on this implementation", () => {
    const model = seedModel(LOAD_FIXTURE);
    const whole = model.getMaterializationPlan();
    model.hoistToFocus(FocusRef.for("gauntlet", "WorkItem", "u-100"), {
      relation_queries: HAS_EVIDENCE_1,
    });
    const hoisted = model.getMaterializationPlan();

    // An eager whole-graph implementation would satisfy this assertion; ours
    // must fail it — the tooth only bites if the eager assertion actually throws.
    const eagerBehavior = () => {
      expect(hoisted.counters.subscriptions).toBe(whole.counters.subscriptions);
    };
    expect(eagerBehavior).toThrow();

    // Positive form: the hoisted world is the declared horizon closure.
    expect(hoisted.counters.subscriptions).toBeLessThanOrEqual(
      whole.counters.subscriptions * FROZEN_BUDGETS.hoist_narrowing_max_fraction,
    );
    model.close();
  });
});

describe("T17 placement honesty (prereg D1, plan redesign_trigger)", () => {
  it("decidePlacement implements the frozen rule", () => {
    // 1. A fold within one frame's budget blocks nothing: inline, always.
    expect(
      decidePlacement({ inline_ms: 0.05, worker_dispatch_ms: 0.001, worker_end_to_end_ms: 1 }),
    ).toBe("inline");
    // 2a. A blocking fold with cheap dispatch and bounded completion: worker.
    expect(
      decidePlacement({ inline_ms: 300, worker_dispatch_ms: 0.05, worker_end_to_end_ms: 400 }),
    ).toBe("worker");
    // 2b. Dispatch costs at least the fold it would remove: inline.
    expect(
      decidePlacement({ inline_ms: 300, worker_dispatch_ms: 450, worker_end_to_end_ms: 400 }),
    ).toBe("inline");
    // 2c. Completion exceeds the frozen off-path budget (B2b): inline.
    expect(
      decidePlacement({
        inline_ms: 300,
        worker_dispatch_ms: 0.05,
        worker_end_to_end_ms: FROZEN_BUDGETS.whole_case_fold_p99_ms + 1,
      }),
    ).toBe("inline");
  });

  it("the recorded decision table is consistent with its own numbers, and SMALL records inline", () => {
    expect(placementDecisionsAreConsistent()).toBe(true);
    const small = recordedPlacementFor("t17-small");
    expect(small).not.toBeNull();
    expect(small?.decision).toBe("inline");
    expect(small?.decision).toBe(
      decidePlacement({
        inline_ms: small?.inline_ms ?? 0,
        worker_dispatch_ms: small?.worker_dispatch_ms ?? 0,
        worker_end_to_end_ms: small?.worker_end_to_end_ms ?? 0,
      }),
    );
    for (const entry of RECORDED_PLACEMENT_DECISIONS) {
      expect(entry.decision).toBe(
        decidePlacement({
          inline_ms: entry.inline_ms,
          worker_dispatch_ms: entry.worker_dispatch_ms,
          worker_end_to_end_ms: entry.worker_end_to_end_ms,
        }),
      );
    }
  });

  it("executor placements are labeled honestly", () => {
    expect(new InlineFoldExecutor().placement).toBe("inline");
    expect(new ScriptedFoldExecutor("worker", []).placement).toBe("worker");
    expect(new ScriptedFoldExecutor("inline", []).placement).toBe("inline");
  });
});

describe("T17 boundary validation (model output is untrusted input)", () => {
  it("isFoldWorkerResponse accepts schema-valid responses and rejects malformed ones", () => {
    const validPlan = {
      horizon_id: null,
      working_set: [serializeFocusRef(FocusRef.for("gauntlet", "WorkItem", "u-0"))],
      subscriptions: ["sub:x"],
      queries: [{ kind: "case_scan", scope: "units" }],
      prefetch_keys: [],
      context_candidates: [],
      counters: { subscriptions: 1, queries: 1, prefetches: 0, context_candidates: 0 },
    };
    expect(
      isFoldWorkerResponse({
        type: "fold_snapshot",
        task_id: "t",
        generation: "g",
        result: validPlan,
      }),
    ).toBe(true);
    expect(
      isFoldWorkerResponse({
        type: "world_installed",
        generation: "g",
        rows: { units: 1, evidence: 2, sessions: 3 },
      }),
    ).toBe(true);
    expect(
      isFoldWorkerResponse({ type: "fold_error", task_id: "t", generation: "g", reason: "r" }),
    ).toBe(true);

    expect(isFoldWorkerResponse(null)).toBe(false);
    expect(isFoldWorkerResponse("fold_snapshot")).toBe(false);
    expect(isFoldWorkerResponse({ type: "mystery", generation: "g" })).toBe(false);
    expect(isFoldWorkerResponse({ type: "fold_snapshot", task_id: "t" })).toBe(false);
    // Layered validation: a counters-drift document is SHAPE-valid (the shape
    // validator checks types only) but is rejected at readmission, so it can
    // never be admitted into model state.
    const driftedResponse = {
      type: "fold_snapshot",
      task_id: "t",
      generation: "g",
      result: {
        ...validPlan,
        counters: { subscriptions: 99, queries: 1, prefetches: 0, context_candidates: 0 },
      },
    };
    expect(isFoldWorkerResponse(driftedResponse)).toBe(true);
    expect(() => readmitPlanResult(driftedResponse.result)).toThrow(/counters do not match/);
    expect(
      isFoldWorkerResponse({
        type: "fold_snapshot",
        task_id: "t",
        generation: "g",
        result: { ...validPlan, working_set: [{ id: "mismatch", kind: "WorkItem" }] },
      }),
    ).toBe(false);
    expect(
      isFoldWorkerResponse({ type: "world_installed", generation: "g", rows: { units: "many" } }),
    ).toBe(false);
  });

  it("readmitPlanResult throws a typed FoldContractError on counters drift and on forged focus identity", () => {
    const refDoc = serializeFocusRef(FocusRef.for("gauntlet", "WorkItem", "u-0"));
    const base = {
      horizon_id: null,
      working_set: [refDoc],
      subscriptions: ["sub:gauntlet:WorkItem:u-0"],
      queries: [],
      prefetch_keys: ["prefetch:x"],
      context_candidates: [],
      counters: { subscriptions: 1, queries: 0, prefetches: 1, context_candidates: 0 },
    };
    expect(
      isFoldWorkerResponse({ type: "fold_snapshot", task_id: "t", generation: "g", result: base }),
    ).toBe(true);
    expect(readmitPlanResult(base).working_set[0]).toBe(
      FocusRef.for("gauntlet", "WorkItem", "u-0"),
    );

    // Counters that disagree with the delivered arrays are rejected.
    const drifted = {
      ...base,
      counters: { subscriptions: 5, queries: 0, prefetches: 1, context_candidates: 0 },
    };
    expect(() => readmitPlanResult(drifted)).toThrow(/counters do not match/);

    // A forged focus document (declared id != derived identity) is rejected.
    const forged = {
      ...base,
      working_set: [{ ...refDoc, id: "focus:gauntlet:WorkItem:someone-else" }],
    };
    expect(() => readmitPlanResult(forged)).toThrow(/focus ref rejected/);

    expect(() => readmitPlanResult("not a plan")).toThrow(/declared schema/);
  });
});

describe("T17 real Bun worker handoff (end-to-end transport)", () => {
  it("install -> dispatch -> snapshot outcome arrives through the real worker, identical to the model's inline fold", async () => {
    const model = seedModel(T17_SMALL_FIXTURE);
    const horizon = model.hoistToFocus(FocusRef.for("gauntlet", "WorkItem", "u-0"), {
      relation_queries: HAS_EVIDENCE_1,
    });
    const modelPlan = model.getMaterializationPlan();

    const executor = new BunWorkerFoldExecutor();
    try {
      const receipt = await executor.installWorld(worldOf(model));
      expect(receipt).toEqual({
        generation: T17_SMALL_FIXTURE.generation,
        units: 3,
        evidence: 3,
        sessions: 4,
      });

      const outcomes: FoldOutcome[] = [];
      executor.onOutcome((outcome) => outcomes.push(outcome));
      executor.dispatchPlan(
        T17_SMALL_FIXTURE.generation,
        serializeHorizonForTask(horizon),
        "task-real-worker",
      );
      await waitFor(() => outcomes.length > 0);

      expect(outcomes.length).toBe(1);
      const outcome = outcomes[0];
      expect(outcome?.kind).toBe("snapshot");
      if (outcome?.kind !== "snapshot") {
        throw new Error(`expected snapshot outcome, got: ${outcome?.kind}`);
      }
      expect(outcome.task_id).toBe("task-real-worker");
      expect(outcome.result).toEqual(modelPlan);
      // The worker's plan resolves to the SAME interned identities.
      for (const ref of outcome.result.working_set) {
        expect(ref).toBe(FocusRef.for(ref.source, ref.kind, ref.source_id));
      }
      expect(outcome.result.working_set.find((r) => r.id === modelPlan.working_set[0]?.id)).toBe(
        modelPlan.working_set[0],
      );
    } finally {
      executor.dispose();
      model.close();
    }
  });

  it("a plan task for an uninstalled generation returns a preserved fold_error from the real worker", async () => {
    const executor = new BunWorkerFoldExecutor();
    try {
      const outcomes: FoldOutcome[] = [];
      executor.onOutcome((outcome) => outcomes.push(outcome));
      executor.dispatchPlan("gen-never", null, "task-missing-real");
      await waitFor(() => outcomes.length > 0);
      expect(outcomes[0]?.kind).toBe("error");
      if (outcomes[0]?.kind === "error") {
        expect(outcomes[0].reason).toBe("world_not_installed");
        expect(outcomes[0].task_id).toBe("task-missing-real");
      }
    } finally {
      executor.dispose();
    }
  });
});
