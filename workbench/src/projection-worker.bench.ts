/**
 * T17 projection-worker benchmark (plan T17, REQ-BUN-006).
 *
 * Measures worker vs inline placement of the materialization fold over the
 * frozen T17 fixtures, using the REAL Bun worker transport:
 *   - inline synchronous fold cost (what the render path pays under inline placement)
 *   - worker world install cost (per generation, off the interaction path)
 *   - worker fold end-to-end (dispatch -> result arrival, off-path)
 *   - worker per-interaction dispatch cost (what the render path pays under worker placement)
 *   - off-path result application (re-admission of the returned plan)
 * Raw samples, percentile summaries, and peak RSS are printed. Placement
 * decisions are computed by the frozen `decidePlacement` rule from these
 * measurements and printed as the record to freeze into worker-folds.ts and
 * the T17 evidence. Fold equivalence (worker result == inline plan) is
 * asserted; a mismatch or a budget miss exits nonzero.
 *
 * Frozen fixture sizes, sample counts, warmups, and budgets are declared in
 * `.agents/reports/gauntlet-tui-prime-workbench/evidence/T17/horizon-payment.md`
 * BEFORE measurement and are read from `FROZEN_BUDGETS` — never tuned here.
 */

import { FocusRef } from "./focus";
import { createHorizon, type Horizon, type MaterializationPlan } from "./horizon";
import type { ProjectionWorld } from "./relations";
import { WorkbenchModel } from "./workbench-model";
import {
  BunWorkerFoldExecutor,
  createSyntheticCaseEvents,
  decidePlacement,
  type FoldOutcome,
  FROZEN_BUDGETS,
  peakRssBytes,
  readmitPlanResult,
  type SyntheticCaseSpec,
  serializePlanResult,
  summarizeLatencySamples,
  T17_LARGE_FIXTURE,
  T17_SMALL_FIXTURE,
  worldFromSlices,
} from "./worker-folds";

// Frozen sample counts / warmups (evidence T17/horizon-payment.md).
const WARMUP_INLINE_LARGE = 2;
const SAMPLES_INLINE_LARGE = 20;
const WARMUP_INSTALL_LARGE = 2;
const SAMPLES_INSTALL_LARGE = 20;
const WARMUP_END_TO_END_LARGE = 5;
const SAMPLES_END_TO_END_LARGE = 20;
const WARMUP_ROUND_TRIP_LARGE = 5;
const SAMPLES_ROUND_TRIP_LARGE = 60;
const WARMUP_APPLY = 2;
const SAMPLES_APPLY = 60;
const WARMUP_INLINE_SMALL = 2;
const SAMPLES_INLINE_SMALL = 60;
const WARMUP_END_TO_END_SMALL = 5;
const SAMPLES_END_TO_END_SMALL = 60;

interface Scenario {
  readonly name: string;
  readonly warmups: number;
  readonly samples: number[];
}

function newScenario(name: string, warmups: number): Scenario {
  return { name, warmups, samples: [] };
}

function report(scenario: Scenario): void {
  const summary = summarizeLatencySamples(scenario.samples);
  console.log(
    `[samples] ${scenario.name}: n=${summary.n} mean=${summary.mean_ms.toFixed(3)}ms ` +
      `p50=${summary.p50_ms.toFixed(3)}ms p95=${summary.p95_ms.toFixed(3)}ms ` +
      `p99=${summary.p99_ms.toFixed(3)}ms max=${summary.max_ms.toFixed(3)}ms`,
  );
  console.log(`[raw] ${scenario.name}: [${scenario.samples.map((s) => s.toFixed(3)).join(", ")}]`);
}

function median(scenario: Scenario): number {
  return summarizeLatencySamples(scenario.samples).p50_ms;
}

function seedModel(spec: SyntheticCaseSpec): WorkbenchModel {
  const model = new WorkbenchModel();
  model.startGeneration(spec.generation);
  for (const frame of createSyntheticCaseEvents(spec)) {
    const result = model.applyEvent(frame);
    if (!result.applied) {
      throw new Error(`fixture event rejected: ${result.reason ?? "unknown"}`);
    }
  }
  return model;
}

function worldOf(model: WorkbenchModel): ProjectionWorld {
  const base = model.store.getSnapshot(null);
  return worldFromSlices(
    base.generation,
    base.units,
    base.evidence,
    base.sessions,
    base.source_cursors,
  );
}

/** Await helper for one dispatched task's outcome (round-trip mode, no coalescing). */
function outcomeWaiter(executor: BunWorkerFoldExecutor): {
  wait: (taskId: string) => Promise<FoldOutcome>;
} {
  const pending = new Map<string, (outcome: FoldOutcome) => void>();
  executor.onOutcome((outcome) => {
    if (outcome.task_id === null) {
      return;
    }
    const resolve = pending.get(outcome.task_id);
    if (resolve !== undefined) {
      pending.delete(outcome.task_id);
      resolve(outcome);
    }
  });
  return {
    wait: (taskId: string) =>
      new Promise<FoldOutcome>((resolve) => {
        pending.set(taskId, resolve);
      }),
  };
}

function planSignature(plan: MaterializationPlan): string {
  return JSON.stringify({
    h: plan.horizon_id,
    w: plan.working_set.map((f) => f.id),
    s: plan.subscriptions,
    q: plan.queries,
    p: plan.prefetch_keys,
    c: plan.context_candidates.map((candidate) => [
      candidate.focus.id,
      candidate.role,
      candidate.detail,
      candidate.mandatory,
    ]),
    n: plan.counters,
  });
}

let failures = 0;
function check(ok: boolean, label: string, detail: string): void {
  console.log(`[${ok ? "PASS" : "MISS"}] ${label}: ${detail}`);
  if (!ok) {
    failures += 1;
  }
}

console.log(
  `=== T17 projection-worker bench (frozen budgets: install p99<=${FROZEN_BUDGETS.world_install_p99_ms}ms, ` +
    `whole-Case fold p99<=${FROZEN_BUDGETS.whole_case_fold_p99_ms}ms, apply p99<=${FROZEN_BUDGETS.fold_apply_p99_ms}ms, ` +
    `RSS<=${FROZEN_BUDGETS.bench_peak_rss_bytes} bytes) ===`,
);

// --- LARGE fixture -----------------------------------------------------------------

const buildStart = performance.now();
const largeModel = seedModel(T17_LARGE_FIXTURE);
const largeWorld = worldOf(largeModel);
const largeBuildMs = performance.now() - buildStart;
console.log(
  `[fixture] t17-large: ${T17_LARGE_FIXTURE.units} units, ${T17_LARGE_FIXTURE.units} evidence, ` +
    `${T17_LARGE_FIXTURE.sessions} sessions (${createSyntheticCaseEvents(T17_LARGE_FIXTURE).length} events) ` +
    `built through the real model in ${largeBuildMs.toFixed(0)}ms`,
);

const largeLeaf = FocusRef.for("gauntlet", "WorkItem", "u-100");
const hoistHorizon: Horizon = createHorizon({
  root_focus: largeLeaf,
  generation: T17_LARGE_FIXTURE.generation,
  relation_queries: [{ kind: "has_evidence", depth: 1, direction: "outbound" }],
  horizon_id: `horizon:${T17_LARGE_FIXTURE.generation}:${largeLeaf.id}`,
});
const hoistHorizonJson = serializeHorizonJson(hoistHorizon);

const inlineWholePlan = largeModel.getMaterializationPlan();
largeModel.hoistToFocus(largeLeaf, {
  relation_queries: [{ kind: "has_evidence", depth: 1, direction: "outbound" }],
});
const inlineHoistedPlan = largeModel.getMaterializationPlan();
largeModel.dehoist();

function serializeHorizonJson(horizon: Horizon): string {
  return JSON.stringify({
    id: horizon.id,
    root_focus: horizon.root_focus.toJSON(),
    relation_queries: [...horizon.relation_queries],
    generation: horizon.generation,
    ...(horizon.filters === undefined ? {} : { filters: horizon.filters }),
  });
}

// --- Scenario 1: inline whole-Case fold, LARGE (render-path cost under inline) -----

const inlineLarge = newScenario("inline whole-Case fold t17-large", WARMUP_INLINE_LARGE);
for (let i = 0; i < WARMUP_INLINE_LARGE + SAMPLES_INLINE_LARGE; i += 1) {
  const start = performance.now();
  const plan = largeModel.getMaterializationPlan();
  const ms = performance.now() - start;
  if (planSignature(plan) !== planSignature(inlineWholePlan)) {
    throw new Error("inline fold nondeterministic across samples");
  }
  if (i >= WARMUP_INLINE_LARGE) {
    inlineLarge.samples.push(ms);
  }
}
report(inlineLarge);

// --- Worker executor + scenarios ----------------------------------------------------

const executor = new BunWorkerFoldExecutor();
const waiter = outcomeWaiter(executor);

const install = newScenario("worker world install t17-large", WARMUP_INSTALL_LARGE);
for (let i = 0; i < WARMUP_INSTALL_LARGE + SAMPLES_INSTALL_LARGE; i += 1) {
  const start = performance.now();
  const receipt = await executor.installWorld(largeWorld);
  const ms = performance.now() - start;
  if (receipt.units !== T17_LARGE_FIXTURE.units) {
    throw new Error("install receipt row count mismatch");
  }
  if (i >= WARMUP_INSTALL_LARGE) {
    install.samples.push(ms);
  }
}
report(install);

const endToEndLarge = newScenario(
  "worker whole-Case fold end-to-end t17-large",
  WARMUP_END_TO_END_LARGE,
);
const dispatchLarge = newScenario("worker dispatch t17-large (render-path cost under worker)", 0);
for (let i = 0; i < WARMUP_END_TO_END_LARGE + SAMPLES_END_TO_END_LARGE; i += 1) {
  const taskId = `bench-e2e-${i}`;
  const wait = waiter.wait(taskId);
  const start = performance.now();
  executor.dispatchPlan(T17_LARGE_FIXTURE.generation, null, taskId);
  const dispatchMs = performance.now() - start;
  const outcome = await wait;
  const totalMs = performance.now() - start;
  if (outcome.kind !== "snapshot") {
    throw new Error(
      `worker fold failed: ${outcome.kind === "error" ? outcome.reason : outcome.kind}`,
    );
  }
  if (planSignature(outcome.result) !== planSignature(inlineWholePlan)) {
    throw new Error(
      "worker whole-Case fold result differs from the inline plan (equivalence MISS)",
    );
  }
  if (i >= WARMUP_END_TO_END_LARGE) {
    endToEndLarge.samples.push(totalMs);
    dispatchLarge.samples.push(dispatchMs);
  }
}
report(endToEndLarge);
report(dispatchLarge);

const roundTripLarge = newScenario(
  "worker hoisted fold round-trip t17-large",
  WARMUP_ROUND_TRIP_LARGE,
);
for (let i = 0; i < WARMUP_ROUND_TRIP_LARGE + SAMPLES_ROUND_TRIP_LARGE; i += 1) {
  const taskId = `bench-rt-${i}`;
  const wait = waiter.wait(taskId);
  const start = performance.now();
  executor.dispatchPlan(T17_LARGE_FIXTURE.generation, hoistHorizonJson, taskId);
  const outcome = await wait;
  const totalMs = performance.now() - start;
  if (outcome.kind !== "snapshot") {
    throw new Error(
      `worker hoisted fold failed: ${outcome.kind === "error" ? outcome.reason : outcome.kind}`,
    );
  }
  if (planSignature(outcome.result) !== planSignature(inlineHoistedPlan)) {
    throw new Error("worker hoisted fold result differs from the inline plan (equivalence MISS)");
  }
  if (i >= WARMUP_ROUND_TRIP_LARGE) {
    roundTripLarge.samples.push(totalMs);
  }
}
report(roundTripLarge);

// --- Off-path result application (readmission), LARGE whole-Case plan --------------

const apply = newScenario("fold result application (readmission) t17-large", WARMUP_APPLY);
const serializedWhole = serializePlanResult(inlineWholePlan);
for (let i = 0; i < WARMUP_APPLY + SAMPLES_APPLY; i += 1) {
  const start = performance.now();
  const plan = readmitPlanResult(serializedWhole);
  const ms = performance.now() - start;
  if (planSignature(plan) !== planSignature(inlineWholePlan)) {
    throw new Error("readmitted plan differs from the inline plan");
  }
  if (i >= WARMUP_APPLY) {
    apply.samples.push(ms);
  }
}
report(apply);

// --- SMALL fixture: inline vs worker end-to-end (placement decision input) ----------

const smallModel = seedModel(T17_SMALL_FIXTURE);
const smallWorld = worldOf(smallModel);
const smallLeaf = FocusRef.for("gauntlet", "WorkItem", "u-0");
const smallHorizon = smallModel.hoistToFocus(smallLeaf, {
  relation_queries: [{ kind: "has_evidence", depth: 1, direction: "outbound" }],
});
const smallInlinePlan = smallModel.getMaterializationPlan();
const smallHorizonJson = serializeHorizonJson(smallHorizon);

const inlineSmall = newScenario("inline hoisted fold t17-small", WARMUP_INLINE_SMALL);
for (let i = 0; i < WARMUP_INLINE_SMALL + SAMPLES_INLINE_SMALL; i += 1) {
  const start = performance.now();
  const plan = smallModel.getMaterializationPlan();
  const ms = performance.now() - start;
  if (planSignature(plan) !== planSignature(smallInlinePlan)) {
    throw new Error("small inline fold nondeterministic");
  }
  if (i >= WARMUP_INLINE_SMALL) {
    inlineSmall.samples.push(ms);
  }
}
report(inlineSmall);

await executor.installWorld(smallWorld);
const endToEndSmall = newScenario(
  "worker hoisted fold end-to-end t17-small",
  WARMUP_END_TO_END_SMALL,
);
const dispatchSmall = newScenario("worker dispatch t17-small", 0);
for (let i = 0; i < WARMUP_END_TO_END_SMALL + SAMPLES_END_TO_END_SMALL; i += 1) {
  const taskId = `bench-small-${i}`;
  const wait = waiter.wait(taskId);
  const start = performance.now();
  executor.dispatchPlan(T17_SMALL_FIXTURE.generation, smallHorizonJson, taskId);
  dispatchSmall.samples.push(performance.now() - start);
  const outcome = await wait;
  const totalMs = performance.now() - start;
  if (outcome.kind !== "snapshot") {
    throw new Error(
      `small worker fold failed: ${outcome.kind === "error" ? outcome.reason : outcome.kind}`,
    );
  }
  if (planSignature(outcome.result) !== planSignature(smallInlinePlan)) {
    throw new Error("small worker fold result differs from the inline plan (equivalence MISS)");
  }
  if (i >= WARMUP_END_TO_END_SMALL) {
    endToEndSmall.samples.push(totalMs);
  }
}
report(endToEndSmall);
report(dispatchSmall);

// --- Placement decisions from the frozen rule ----------------------------------------

const dispatchMedian = median(dispatchLarge);
const largeMeasurement = {
  inline_ms: median(inlineLarge),
  worker_dispatch_ms: dispatchMedian,
  worker_end_to_end_ms: median(endToEndLarge),
};
const smallMeasurement = {
  inline_ms: median(inlineSmall),
  worker_dispatch_ms: median(dispatchSmall),
  worker_end_to_end_ms: median(endToEndSmall),
};
const largeDecision = decidePlacement(largeMeasurement);
const smallDecision = decidePlacement(smallMeasurement);

console.log("\n=== Recorded placement decisions (freeze into worker-folds.ts + evidence) ===");
for (const entry of [
  {
    fold: "materialization_plan",
    fixture: "t17-small",
    decision: smallDecision,
    inline_ms: Number(smallMeasurement.inline_ms.toFixed(4)),
    worker_dispatch_ms: Number(smallMeasurement.worker_dispatch_ms.toFixed(4)),
    worker_end_to_end_ms: Number(smallMeasurement.worker_end_to_end_ms.toFixed(4)),
  },
  {
    fold: "materialization_plan",
    fixture: "t17-large",
    decision: largeDecision,
    inline_ms: Number(largeMeasurement.inline_ms.toFixed(3)),
    worker_dispatch_ms: Number(largeMeasurement.worker_dispatch_ms.toFixed(4)),
    worker_end_to_end_ms: Number(largeMeasurement.worker_end_to_end_ms.toFixed(3)),
  },
]) {
  console.log(JSON.stringify(entry));
}

// --- Budget checks (frozen) -----------------------------------------------------------

const installSummary = summarizeLatencySamples(install.samples);
check(
  installSummary.p99_ms <= FROZEN_BUDGETS.world_install_p99_ms,
  "B2a world install p99",
  `${installSummary.p99_ms.toFixed(2)}ms <= ${FROZEN_BUDGETS.world_install_p99_ms}ms`,
);
const e2eSummary = summarizeLatencySamples(endToEndLarge.samples);
check(
  e2eSummary.p99_ms <= FROZEN_BUDGETS.whole_case_fold_p99_ms,
  "B2b worker whole-Case fold end-to-end p99",
  `${e2eSummary.p99_ms.toFixed(2)}ms <= ${FROZEN_BUDGETS.whole_case_fold_p99_ms}ms`,
);
const applySummary = summarizeLatencySamples(apply.samples);
check(
  applySummary.p99_ms <= FROZEN_BUDGETS.fold_apply_p99_ms,
  "B2c fold result application p99",
  `${applySummary.p99_ms.toFixed(2)}ms <= ${FROZEN_BUDGETS.fold_apply_p99_ms}ms`,
);
const rss = peakRssBytes();
check(
  rss <= FROZEN_BUDGETS.bench_peak_rss_bytes,
  "B4 peak RSS",
  `${(rss / 1024 / 1024).toFixed(0)}MB <= ${(FROZEN_BUDGETS.bench_peak_rss_bytes / 1024 / 1024).toFixed(0)}MB`,
);
check(smallDecision === "inline", "B5 SMALL records inline", `decision=${smallDecision}`);

executor.dispose();
largeModel.close();
smallModel.close();

console.log(
  `\n=== T17 projection-worker bench complete: ${failures === 0 ? "ALL BUDGETS PASS" : `${failures} BUDGET MISS(ES)`} ===`,
);
if (failures > 0) {
  process.exitCode = 1;
}
