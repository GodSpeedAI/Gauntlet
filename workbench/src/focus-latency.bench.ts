/**
 * T17 focus-latency benchmark (plan T17, REQ-BUN-006, REQ-FOCUS-003/004, prereg P2).
 *
 * Workload: worst-case representative folds (whole-Case + horizon-local
 * materialization over the frozen LARGE fixture) during rapid Focus movement —
 * 200 hoist + 200 de-hoist interactions plus 60 focus switches — measured under
 * BOTH placements:
 *   - INLINE: the real WorkbenchModel render path (a subscribed render consumer;
 *     every interaction synchronously pays getSnapshot: SQLite read + fold).
 *   - WORKER: the MaterializationFoldCoordinator over the real Bun worker; every
 *     interaction pays only the descriptor dispatch while folds run off-path
 *     with latest-wins coalescing (intermediate results arrive stale and are
 *     discarded by the coordinator).
 * Also verified off-clock: after the stream, the settled worker plan narrows
 * within the frozen hoisting fraction (B3) and the de-hoist plan restores the
 * whole-Case counters EXACTLY (zero drift). A budget miss exits nonzero.
 *
 * Frozen fixture size, interaction counts, warmups, and budgets live in
 * `.agents/reports/gauntlet-tui-prime-workbench/evidence/T17/horizon-payment.md`
 * (declared before measurement) and are read from FROZEN_BUDGETS.
 */

import { FocusRef } from "./focus";
import { createHorizon } from "./horizon";
import type { ProjectionWorld } from "./relations";
import { WorkbenchModel } from "./workbench-model";
import {
  BunWorkerFoldExecutor,
  createSyntheticCaseEvents,
  type FoldOutcome,
  FROZEN_BUDGETS,
  MaterializationFoldCoordinator,
  peakRssBytes,
  summarizeLatencySamples,
  T17_LARGE_FIXTURE,
  worldFromSlices,
} from "./worker-folds";

// Frozen interaction counts (evidence T17/horizon-payment.md): per placement
// 200 hoist + 200 de-hoist + 60 focus-switch interactions; 10 discarded warmups.
const WARMUPS = 10;
const HOIST_INTERACTIONS = 200;
const DEHOIST_INTERACTIONS = 200;
const FOCUS_SWITCH_INTERACTIONS = 60;
/** Realistic event arrival: yield to the event loop every N interactions. */
const YIELD_EVERY = 10;

interface Scenario {
  readonly name: string;
  readonly samples: number[];
}

function newScenario(name: string): Scenario {
  return { name, samples: [] };
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

function leafRef(i: number): FocusRef {
  return FocusRef.for("gauntlet", "WorkItem", `u-${1000 + (i % 4000)}`);
}

let failures = 0;
function check(ok: boolean, label: string, detail: string): void {
  console.log(`[${ok ? "PASS" : "MISS"}] ${label}: ${detail}`);
  if (!ok) {
    failures += 1;
  }
}

console.log(
  `=== T17 focus-latency bench (frozen budgets: interactive median<=${FROZEN_BUDGETS.interactive_submit_median_ms}ms, ` +
    `p99<=${FROZEN_BUDGETS.interactive_submit_p99_ms}ms, hoisted subscriptions<=` +
    `${FROZEN_BUDGETS.hoist_narrowing_max_fraction * 100}% of whole-Case, RSS<=${FROZEN_BUDGETS.bench_peak_rss_bytes} bytes) ===`,
);

// --- Fixture ---------------------------------------------------------------------

const buildStart = performance.now();
const model = new WorkbenchModel();
model.startGeneration(T17_LARGE_FIXTURE.generation);
const fixtureEvents = createSyntheticCaseEvents(T17_LARGE_FIXTURE);
for (const frame of fixtureEvents) {
  const result = model.applyEvent(frame);
  if (!result.applied) {
    throw new Error(`fixture event rejected: ${result.reason ?? "unknown"}`);
  }
}
const world: ProjectionWorld = (() => {
  const base = model.store.getSnapshot(null);
  return worldFromSlices(
    base.generation,
    base.units,
    base.evidence,
    base.sessions,
    base.source_cursors,
  );
})();
console.log(
  `[fixture] t17-large: ${fixtureEvents.length} events built in ${(performance.now() - buildStart).toFixed(0)}ms ` +
    `(units=${world.units.length}, evidence=${world.evidence.length}, sessions=${world.sessions.length})`,
);

const wholeCasePlan = model.getMaterializationPlan();
const wholeCaseSubscriptions = wholeCasePlan.counters.subscriptions;
const wholeCaseSignature = JSON.stringify(wholeCasePlan.counters);

// --- INLINE arm: the real model render path (subscribed render consumer) ----------

let renders = 0;
const renderConsumer = (): void => {
  renders += 1;
};
const unsubscribe = model.subscribe(renderConsumer);

const inlineHoist = newScenario("inline hoist interaction (model call incl. render)");
const inlineDehoist = newScenario("inline de-hoist interaction (model call incl. render)");
const inlineSwitch = newScenario("inline focus switch (model call incl. render)");

for (let i = 0; i < WARMUPS; i += 1) {
  model.hoistToFocus(leafRef(i), {
    relation_queries: [{ kind: "has_evidence", depth: 1, direction: "outbound" }],
  });
  model.dehoist();
  model.selectFocus(leafRef(i + 100));
}

for (let i = 0; i < HOIST_INTERACTIONS; i += 1) {
  const start = performance.now();
  model.hoistToFocus(leafRef(i), {
    relation_queries: [{ kind: "has_evidence", depth: 1, direction: "outbound" }],
  });
  inlineHoist.samples.push(performance.now() - start);
}
for (let i = 0; i < DEHOIST_INTERACTIONS; i += 1) {
  // One-step return pops the hoist pushed above (T08 stack discipline).
  const start = performance.now();
  model.dehoist();
  inlineDehoist.samples.push(performance.now() - start);
}
for (let i = 0; i < FOCUS_SWITCH_INTERACTIONS; i += 1) {
  const start = performance.now();
  model.selectFocus(leafRef(i + 500));
  inlineSwitch.samples.push(performance.now() - start);
}
report(inlineHoist);
report(inlineDehoist);
report(inlineSwitch);
unsubscribe();
model.dehoist();

// --- WORKER arm: coordinator over the real Bun worker ------------------------------

const executor = new BunWorkerFoldExecutor();
const coordinator = new MaterializationFoldCoordinator(executor);
const pending = new Map<string, (outcome: FoldOutcome) => void>();
executor.onOutcome((outcome) => {
  coordinator.accept(outcome, T17_LARGE_FIXTURE.generation);
  if (outcome.task_id !== null) {
    const resolve = pending.get(outcome.task_id);
    if (resolve !== undefined) {
      pending.delete(outcome.task_id);
      resolve(outcome);
    }
  }
});

const installStart = performance.now();
await executor.installWorld(world);
console.log(
  `[worker] world installed in ${(performance.now() - installStart).toFixed(1)}ms (once per generation)`,
);

const workerHoist = newScenario("worker hoist interaction (descriptor dispatch)");
const workerDehoist = newScenario("worker de-hoist interaction (descriptor dispatch)");
const workerSwitch = newScenario("worker focus switch (cached plan)");

const warmGeneration = T17_LARGE_FIXTURE.generation;
for (let i = 0; i < WARMUPS; i += 1) {
  await coordinator.submit(world, null, warmGeneration);
}

for (let i = 0; i < HOIST_INTERACTIONS; i += 1) {
  const horizon = createHorizon({
    root_focus: leafRef(i),
    generation: T17_LARGE_FIXTURE.generation,
    relation_queries: [{ kind: "has_evidence", depth: 1, direction: "outbound" }],
  });
  const start = performance.now();
  await coordinator.submit(world, horizon, T17_LARGE_FIXTURE.generation);
  workerHoist.samples.push(performance.now() - start);
  if (i % YIELD_EVERY === YIELD_EVERY - 1) {
    await Bun.sleep(0); // realistic event arrival: let the macrotask (and flush) run
  }
}
for (let i = 0; i < DEHOIST_INTERACTIONS; i += 1) {
  const start = performance.now();
  await coordinator.submit(world, null, T17_LARGE_FIXTURE.generation);
  workerDehoist.samples.push(performance.now() - start);
  if (i % YIELD_EVERY === YIELD_EVERY - 1) {
    await Bun.sleep(0);
  }
}
// Prime one fold before the switch loop: a real host has at least one settled
// plan before rendering focus switches (cold start: null plan = "materializing").
let primeSeq = 8000;
{
  primeSeq += 1;
  const primeTask = `prime-${primeSeq}`;
  const primeWait = new Promise<FoldOutcome>((resolve) => {
    pending.set(primeTask, resolve);
  });
  executor.dispatchPlan(T17_LARGE_FIXTURE.generation, null, primeTask);
  const primed = await primeWait;
  if (primed.kind !== "snapshot") {
    throw new Error("prime fold did not produce a snapshot");
  }
  if (coordinator.acceptedPlan === null) {
    throw new Error("prime fold outcome was not accepted into the cached plan");
  }
}

for (let i = 0; i < FOCUS_SWITCH_INTERACTIONS; i += 1) {
  // A focus switch that does not change the Horizon re-folds nothing under
  // worker placement: the render path keeps the cached accepted plan.
  const start = performance.now();
  model.selectFocus(leafRef(i + 500));
  const plan = coordinator.acceptedPlan;
  if (plan === null) {
    throw new Error("worker arm: no accepted plan cached for a focus switch");
  }
  workerSwitch.samples.push(performance.now() - start);
}
report(workerHoist);
report(workerDehoist);
report(workerSwitch);

// --- Settle + frozen B3 checks (off the latency clock) ------------------------------

let settleSeq = 9000;
const settle = async (horizonJson: string | null): Promise<FoldOutcome> => {
  settleSeq += 1;
  const taskId = `settle-${settleSeq}`;
  const waitPromise = new Promise<FoldOutcome>((resolve) => {
    pending.set(taskId, resolve);
  });
  executor.dispatchPlan(T17_LARGE_FIXTURE.generation, horizonJson, taskId);
  return waitPromise;
};

const settledHoisted = await settle(
  JSON.stringify({
    id: `horizon:${T17_LARGE_FIXTURE.generation}:${leafRef(7).id}`,
    root_focus: leafRef(7).toJSON(),
    relation_queries: [{ kind: "has_evidence", depth: 1, direction: "outbound" }],
    generation: T17_LARGE_FIXTURE.generation,
  }),
);
if (settledHoisted.kind !== "snapshot") {
  throw new Error("settled hoisted fold did not produce a snapshot");
}
const hoistedSubscriptions = settledHoisted.result.counters.subscriptions;
check(
  hoistedSubscriptions <= wholeCaseSubscriptions * FROZEN_BUDGETS.hoist_narrowing_max_fraction,
  "B3 hoisted subscriptions within frozen narrowing fraction",
  `${hoistedSubscriptions} <= ${wholeCaseSubscriptions * FROZEN_BUDGETS.hoist_narrowing_max_fraction} (1% of ${wholeCaseSubscriptions})`,
);

const settledWhole = await settle(null);
if (settledWhole.kind !== "snapshot") {
  throw new Error("settled whole-Case fold did not produce a snapshot");
}
check(
  JSON.stringify(settledWhole.result.counters) === wholeCaseSignature,
  "B3 de-hoist restores whole-Case counters exactly (zero drift after the full stream)",
  `worker=${JSON.stringify(settledWhole.result.counters)} model=${wholeCaseSignature}`,
);

// --- B1 over the pooled worker-arm interaction stream ------------------------------

const pooledWorker = [...workerHoist.samples, ...workerDehoist.samples, ...workerSwitch.samples];
const pooledSummary = summarizeLatencySamples(pooledWorker);
check(
  pooledSummary.p50_ms <= FROZEN_BUDGETS.interactive_submit_median_ms,
  "B1 interactive median (worker placement, pooled stream)",
  `${pooledSummary.p50_ms.toFixed(4)}ms <= ${FROZEN_BUDGETS.interactive_submit_median_ms}ms`,
);
check(
  pooledSummary.p99_ms <= FROZEN_BUDGETS.interactive_submit_p99_ms,
  "B1 interactive p99 (worker placement, pooled stream)",
  `${pooledSummary.p99_ms.toFixed(4)}ms <= ${FROZEN_BUDGETS.interactive_submit_p99_ms}ms`,
);

// Inline comparison numbers (placement evidence, not a budget).
const inlineHoistSummary = summarizeLatencySamples(inlineHoist.samples);
const inlineDehoistSummary = summarizeLatencySamples(inlineDehoist.samples);
const inlineSwitchSummary = summarizeLatencySamples(inlineSwitch.samples);
console.log(
  `[comparison] inline placement render-path cost per interaction: hoist p50=${inlineHoistSummary.p50_ms.toFixed(2)}ms ` +
    `de-hoist p50=${inlineDehoistSummary.p50_ms.toFixed(2)}ms switch p50=${inlineSwitchSummary.p50_ms.toFixed(2)}ms ` +
    `(${renders} renders consumed)`,
);

const rss = peakRssBytes();
check(
  rss <= FROZEN_BUDGETS.bench_peak_rss_bytes,
  "B4 peak RSS",
  `${(rss / 1024 / 1024).toFixed(0)}MB <= ${(FROZEN_BUDGETS.bench_peak_rss_bytes / 1024 / 1024).toFixed(0)}MB`,
);

console.log(`[coordinator] stats: ${JSON.stringify(coordinator.stats)}`);

executor.dispose();
model.close();

console.log(
  `\n=== T17 focus-latency bench complete: ${failures === 0 ? "ALL BUDGETS PASS" : `${failures} BUDGET MISS(ES)`} ===`,
);
if (failures > 0) {
  process.exitCode = 1;
}
