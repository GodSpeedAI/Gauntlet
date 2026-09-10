/**
 * T21 long-horizon shape benchmark (plan T21, REQ-FOCUS-003/004, REQ-GOAL-006,
 * REQ-SYNC-002, prereg P1/D2, plan teeth 1).
 *
 * The horizon-shape proof at TARGET scale: a one-Focus Horizon is forced over
 * Cases of growing history (the frozen small/medium/large ladder — 4,110,
 * 41,505, and 103,610 authoritative events), and the HOT SET — focus-scoped
 * subscriptions, planned queries, prefetch keys, and context candidates the
 * T17/T08 materialization machinery actually derives — is inspected at every
 * size.
 *
 *   PASS shape: the hot set stays bounded by the DECLARED relations (a
 *   constant ceiling independent of Case size) and is byte-flat from medium to
 *   large (2.5x objects, 2.5x history) — data logistics, not visuals.
 *
 *   FAIL shape (by construction): a whole-world eager implementation's
 *   subscriptions equal the Case's total distinct object count, which violates
 *   the SAME frozen ceiling at the SMALLEST ladder size — the eager plan is
 *   computed and checked against the ceiling at every size so the bound is
 *   proven discriminating, not decorative (prereg P1 falsifier).
 *
 * Fold wall-time is also recorded per size WITHOUT a budget: it is the known
 * adjacent-scope confound (deriveRelations is O(E*U); T17 evidence gap 1), and
 * this bench's claim is about the hot SET, which stays flat while fold work
 * does not shrink proportionally — both facts are printed.
 *
 * All thresholds/ceilings come from payment-budgets.ts (frozen before any
 * measurement); no threshold lives in this file.
 */

import { FocusRef } from "./focus";
import { type MaterializationCounters, planMaterialization } from "./horizon";
import {
  countT21Objects,
  createT21CaseEvents,
  T21_HOT_SET,
  T21_LARGE_FIXTURE,
  T21_MEDIUM_FIXTURE,
  T21_SMALL_FIXTURE,
  type T21CaseSpec,
} from "./payment-budgets";
import type { ProjectionWorld } from "./relations";
import { WorkbenchModel } from "./workbench-model";
import { summarizeLatencySamples } from "./worker-folds";

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
      `p50=${summary.p50_ms.toFixed(3)}ms max=${summary.max_ms.toFixed(3)}ms ` +
      `[raw: ${scenario.samples.map((s) => s.toFixed(2)).join(", ")}]`,
  );
}

let failures = 0;
function check(ok: boolean, label: string, detail: string): void {
  console.log(`[${ok ? "PASS" : "MISS"}] ${label}: ${detail}`);
  if (!ok) {
    failures += 1;
  }
}

const LADDER: readonly { readonly label: string; readonly spec: T21CaseSpec }[] = Object.freeze([
  { label: "small", spec: T21_SMALL_FIXTURE },
  { label: "medium", spec: T21_MEDIUM_FIXTURE },
  { label: "large", spec: T21_LARGE_FIXTURE },
]);

console.log(
  `=== T21 long-horizon bench (frozen ceilings: subscriptions<=` +
    `${T21_HOT_SET.subscription_ceiling}, context candidates<=${T21_HOT_SET.context_ceiling}, ` +
    `queries<=${T21_HOT_SET.query_ceiling} — constants independent of Case size) ===`,
);

interface SizeObservation {
  readonly label: string;
  readonly events: number;
  readonly objects: number;
  readonly loadedRows: number;
  readonly unitRoot: {
    readonly counters: MaterializationCounters;
    readonly workingSet: readonly string[];
    readonly farCaseExcluded: boolean;
  };
  readonly sessionRoot: {
    readonly counters: MaterializationCounters;
  };
  readonly eager: {
    readonly subscriptions: number;
    readonly contextCandidates: number;
    readonly queries: number;
  };
  readonly foldMs: {
    readonly unitRootMs: number;
    readonly sessionRootMs: number;
    readonly eagerMs: number;
  };
}

const observations: SizeObservation[] = [];
const unitRootFold = newScenario("hoisted fold wall-time, unit root (recorded, no budget)");
const sessionRootFold = newScenario("hoisted fold wall-time, session root (recorded, no budget)");
const eagerFold = newScenario("eager whole-Case fold wall-time (recorded, no budget)");

for (const { label, spec } of LADDER) {
  const model = new WorkbenchModel({ enableChat: false });
  model.startGeneration(spec.generation);
  const events = createT21CaseEvents(spec);
  for (const frame of events) {
    const result = model.applyEvent(frame);
    if (!result.applied) {
      throw new Error(`ladder event rejected (${label}): ${result.reason ?? "unknown"}`);
    }
  }
  model.markReady(spec.generation);
  const snapshot = model.getSnapshot();
  const world: ProjectionWorld = {
    generation: snapshot.generation,
    units: snapshot.units,
    evidence: snapshot.evidence,
    sessions: snapshot.sessions,
    source_cursors: snapshot.source_cursors,
  };

  // One-Focus horizon #1: unit root, declared evidence closure (default hoist).
  model.hoistToFocus(FocusRef.for("gauntlet", "WorkItem", "u-0"), {});
  const unitPlan = (() => {
    const start = performance.now();
    const plan = model.getMaterializationPlan();
    unitRootFold.samples.push(performance.now() - start);
    return plan;
  })();
  const farCaseId = `u-${spec.units - 1}`;
  const unitObservation = {
    counters: unitPlan.counters,
    workingSet: unitPlan.working_set.map((focus) => focus.id),
    farCaseExcluded: !unitPlan.working_set.some((focus) => focus.source_id === farCaseId),
  };

  // One-Focus horizon #2: session root over a deep parent chain (60 declared hops
  // at medium/large) — the deep-relation-graph shape.
  model.hoistToFocus(FocusRef.for("prime", "Agent", "s-0-0"), {
    relation_queries: [{ kind: "child_session", depth: 60, direction: "outbound" }],
  });
  const sessionPlan = (() => {
    const start = performance.now();
    const plan = model.getMaterializationPlan();
    sessionRootFold.samples.push(performance.now() - start);
    return plan;
  })();
  const sessionObservation = { counters: sessionPlan.counters };

  // The eager whole-world shape (what REQ-FOCUS-004 forbids): computed so the
  // frozen ceiling is proven to reject it.
  model.selectHorizon(null);
  const eagerPlan = (() => {
    const start = performance.now();
    const plan = planMaterialization(world, null);
    eagerFold.samples.push(performance.now() - start);
    return plan;
  })();

  observations.push({
    label,
    events: events.length,
    objects: countT21Objects(spec),
    loadedRows: world.units.length + world.evidence.length + world.sessions.length,
    unitRoot: unitObservation,
    sessionRoot: sessionObservation,
    eager: {
      subscriptions: eagerPlan.counters.subscriptions,
      contextCandidates: eagerPlan.counters.context_candidates,
      queries: eagerPlan.counters.queries,
    },
    foldMs: {
      unitRootMs: unitRootFold.samples[unitRootFold.samples.length - 1] ?? 0,
      sessionRootMs: sessionRootFold.samples[sessionRootFold.samples.length - 1] ?? 0,
      eagerMs: eagerFold.samples[eagerFold.samples.length - 1] ?? 0,
    },
  });
  model.close();
}

console.log("\n=== Hot-set observations across the frozen size ladder ===");
for (const observation of observations) {
  const unitJson = JSON.stringify(observation.unitRoot.counters);
  const sessionJson = JSON.stringify(observation.sessionRoot.counters);
  console.log(
    `[ladder:${observation.label}] events=${observation.events} objects=${observation.objects} ` +
      `loadedRows=${observation.loadedRows}\n` +
      `    unit-root horizon:    counters=${unitJson} ` +
      `workingSet=${observation.unitRoot.workingSet.length} farCaseExcluded=${observation.unitRoot.farCaseExcluded}\n` +
      `    session-root horizon: counters=${sessionJson}\n` +
      `    eager whole-world:    subscriptions=${observation.eager.subscriptions} ` +
      `contextCandidates=${observation.eager.contextCandidates} queries=${observation.eager.queries}\n` +
      `    fold wall-time (confound, no budget): unit-root ${observation.foldMs.unitRootMs.toFixed(1)}ms, ` +
      `session-root ${observation.foldMs.sessionRootMs.toFixed(1)}ms, eager ${observation.foldMs.eagerMs.toFixed(1)}ms`,
  );
}
report(unitRootFold);
report(sessionRootFold);
report(eagerFold);

// --- Frozen assertions ---------------------------------------------------------------

for (const observation of observations) {
  const unitCounters = observation.unitRoot.counters;
  const sessionCounters = observation.sessionRoot.counters;
  check(
    unitCounters.subscriptions <= T21_HOT_SET.subscription_ceiling &&
      sessionCounters.subscriptions <= T21_HOT_SET.subscription_ceiling,
    `HS1 hot-set subscriptions within ceiling at '${observation.label}' ` +
      `(${observation.events} events, ${observation.objects} objects)`,
    `unit=${unitCounters.subscriptions} session=${sessionCounters.subscriptions} <= ` +
      `${T21_HOT_SET.subscription_ceiling}`,
  );
  check(
    unitCounters.context_candidates <= T21_HOT_SET.context_ceiling &&
      sessionCounters.context_candidates <= T21_HOT_SET.context_ceiling,
    `HS1 hot-set context candidates within ceiling at '${observation.label}'`,
    `unit=${unitCounters.context_candidates} session=${sessionCounters.context_candidates} <= ` +
      `${T21_HOT_SET.context_ceiling}`,
  );
  check(
    unitCounters.queries <= T21_HOT_SET.query_ceiling &&
      sessionCounters.queries <= T21_HOT_SET.query_ceiling,
    `HS1 hot-set queries within ceiling at '${observation.label}'`,
    `unit=${unitCounters.queries} session=${sessionCounters.queries} <= ${T21_HOT_SET.query_ceiling}`,
  );
  check(
    observation.eager.subscriptions > T21_HOT_SET.subscription_ceiling &&
      observation.eager.contextCandidates > T21_HOT_SET.context_ceiling,
    `HS2 eager whole-world shape violates the SAME ceiling at '${observation.label}' ` +
      "(the bound discriminates; an eager implementation fails by construction)",
    `eager subscriptions=${observation.eager.subscriptions} > ${T21_HOT_SET.subscription_ceiling}, ` +
      `candidates=${observation.eager.contextCandidates} > ${T21_HOT_SET.context_ceiling}`,
  );
}

// Flat under growing history (prereg P1 supporting observation): medium and
// large share the declared relation topology, so the hot set must be
// BYTE-IDENTICAL across a 2.5x object / 2.5x history growth.
const medium = observations.find((observation) => observation.label === "medium");
const large = observations.find((observation) => observation.label === "large");
if (medium === undefined || large === undefined) {
  throw new Error("ladder observations missing medium/large");
}
const mediumUnitJson = JSON.stringify(medium.unitRoot.counters);
const largeUnitJson = JSON.stringify(large.unitRoot.counters);
const mediumSessionJson = JSON.stringify(medium.sessionRoot.counters);
const largeSessionJson = JSON.stringify(large.sessionRoot.counters);
check(
  mediumUnitJson === largeUnitJson,
  "HS3 unit-root hot set FLAT from medium to large (2.5x objects, 2.5x history)",
  `medium=${mediumUnitJson} large=${largeUnitJson}`,
);
check(
  mediumSessionJson === largeSessionJson,
  "HS3 session-root hot set FLAT from medium to large (60-hop declared chain at both sizes)",
  `medium=${mediumSessionJson} large=${largeSessionJson}`,
);
check(
  large.unitRoot.farCaseExcluded,
  "HS4 hoisted working set excludes far-Case ids (declared closure, not whole-world)",
  `working set (${large.unitRoot.workingSet.length} ids) excludes ${`u-${T21_LARGE_FIXTURE.units - 1}`}`,
);
check(
  medium.eager.subscriptions < large.eager.subscriptions,
  "HS5 eager hot set GROWS with the Case (the contrast the ceiling rejects)",
  `${medium.eager.subscriptions} < ${large.eager.subscriptions}`,
);

console.log(
  `\n=== T21 long-horizon bench complete: ${failures === 0 ? "ALL SHAPE CHECKS PASS" : `${failures} SHAPE MISS(ES)`} ===`,
);
if (failures > 0) {
  process.exitCode = 1;
}
