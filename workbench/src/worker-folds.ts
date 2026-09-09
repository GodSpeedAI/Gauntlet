/**
 * Worker handoff contract for pure/local projection folds.
 * Conforms to REQ-BUN-006, REQ-GOAL-006, REQ-FOCUS-003/004, and plan T17.
 *
 * Core Invariants:
 *   - Expensive pure/local folding (relation derivation + materialization planning)
 *     moves off the interactive render path through a typed handoff: task descriptor
 *     -> fold executor -> snapshot/invalidation outcome back to the Workbench Model
 *     layer. Worker placement is MEASURED, not ideological (prereg D1): a fold that
 *     is slower through the worker than inline stays inline (plan redesign_trigger).
 *   - Fold results cross the boundary as plain serializable documents and are
 *     re-admitted through the FocusRef identity registry, so a worker-computed plan
 *     resolves to the SAME interned identities as the model's inline fold — never
 *     a fork (REQ-FOCUS-001, DSN-006).
 *   - Outcomes are untrusted external input: they are validated against the declared
 *     contract before any consequential state admits them, and results computed for
 *     a superseded generation or a superseded task are discarded as invalidations,
 *     never applied (REQ-DATA-002 discipline applied to fold artifacts).
 *   - Data logistics stay horizon-shaped under load: hoisting narrows the fold's
 *     working set, subscriptions, and context candidates; de-hoist restores them
 *     exactly, with zero accumulation drift (REQ-FOCUS-003/004, prereg D2).
 */

import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { FocusRef, MalformedFocusRefError } from "./focus";
import type {
  ContextCandidate,
  ContextCandidateRole,
  Horizon,
  MaterializationCounters,
  MaterializationPlan,
  PlannedQuery,
} from "./horizon";
import { horizonFromJSON, horizonToJSON, planMaterialization } from "./horizon";
import type {
  AuthoritativeEventFrame,
  EvidenceProjection,
  SessionProjection,
  UnitProjection,
} from "./projection-store";
import type { ProjectionWorld } from "./relations";

// --- Frozen budgets (declared BEFORE measurement; see evidence T17/horizon-payment.md) ---

/**
 * Mirror of the frozen budget table in
 * `.agents/reports/gauntlet-tui-prime-workbench/evidence/T17/horizon-payment.md`.
 * Declared before any T17 measurement; never tuned after seeing a number.
 */
export const FROZEN_BUDGETS = Object.freeze({
  /** B1: interactive submit latency under worker placement (60 Hz frame / three frames). */
  interactive_submit_median_ms: 16.6,
  interactive_submit_p99_ms: 50.0,
  /** B2a: off-path world install across the worker boundary, LARGE fixture. */
  world_install_p99_ms: 500,
  /** B2b: off-path worst-case whole-Case fold end-to-end, LARGE fixture. */
  whole_case_fold_p99_ms: 1500,
  /** B2c: off-path re-admission of a returned whole-Case plan. */
  fold_apply_p99_ms: 100,
  /** B3: hoisted subscriptions must be at most this fraction of whole-Case subscriptions. */
  hoist_narrowing_max_fraction: 0.01,
  /** B4: peak RSS ceiling for each bench process (1.5 GiB). */
  bench_peak_rss_bytes: 1_610_612_736,
});

/** Typed error for contract violations at the fold boundary (failure locus preserved). */
export class FoldContractError extends Error {
  constructor(operation: string, detail: string) {
    super(`fold contract ${operation}: ${detail}`);
    this.name = "FoldContractError";
  }
}

// --- Serializable fold documents (the worker-boundary representation) -------------

/** Plain serializable form of a FocusRef (exactly FocusRef.toJSON's shape). */
export interface SerializedFocusRefDocument {
  readonly id: string;
  readonly kind: string;
  readonly source: string;
  readonly source_id: string;
  readonly human_label?: string | undefined;
}

export interface SerializedContextCandidate {
  readonly focus: SerializedFocusRefDocument;
  readonly role: ContextCandidateRole;
  readonly detail: string;
  readonly mandatory: boolean;
}

/**
 * The plan as it crosses the fold boundary: identical content to
 * `MaterializationPlan` with every FocusRef in its plain serializable form.
 */
export interface SerializedMaterializationResult {
  readonly horizon_id: string | null;
  readonly working_set: readonly SerializedFocusRefDocument[];
  readonly subscriptions: readonly string[];
  readonly queries: readonly PlannedQuery[];
  readonly prefetch_keys: readonly string[];
  readonly context_candidates: readonly SerializedContextCandidate[];
  readonly counters: MaterializationCounters;
}

export function serializeFocusRef(ref: FocusRef): SerializedFocusRefDocument {
  return ref.toJSON();
}

/** Map a plan produced in this process onto its boundary document. */
export function serializePlanResult(plan: MaterializationPlan): SerializedMaterializationResult {
  return {
    horizon_id: plan.horizon_id,
    working_set: plan.working_set.map(serializeFocusRef),
    subscriptions: [...plan.subscriptions],
    queries: plan.queries.map((query) => ({ ...query })),
    prefetch_keys: [...plan.prefetch_keys],
    context_candidates: plan.context_candidates.map((candidate) => ({
      focus: serializeFocusRef(candidate.focus),
      role: candidate.role,
      detail: candidate.detail,
      mandatory: candidate.mandatory,
    })),
    counters: { ...plan.counters },
  };
}

function isSerializedFocusRefDocument(value: unknown): value is SerializedFocusRefDocument {
  if (typeof value !== "object" || value === null) {
    return false;
  }
  const doc = value as Record<string, unknown>;
  return (
    typeof doc["id"] === "string" &&
    typeof doc["kind"] === "string" &&
    typeof doc["source"] === "string" &&
    typeof doc["source_id"] === "string" &&
    (doc["human_label"] === undefined || typeof doc["human_label"] === "string")
  );
}

function isPlannedQuery(value: unknown): value is PlannedQuery {
  if (typeof value !== "object" || value === null) {
    return false;
  }
  const doc = value as Record<string, unknown>;
  return (
    (doc["kind"] === "case_scan" ||
      doc["kind"] === "focus_load" ||
      doc["kind"] === "relation_traversal") &&
    typeof doc["scope"] === "string"
  );
}

function isSerializedMaterializationResult(
  value: unknown,
): value is SerializedMaterializationResult {
  if (typeof value !== "object" || value === null) {
    return false;
  }
  const doc = value as Record<string, unknown>;
  if (doc["horizon_id"] !== null && typeof doc["horizon_id"] !== "string") {
    return false;
  }
  for (const field of [
    "working_set",
    "subscriptions",
    "queries",
    "prefetch_keys",
    "context_candidates",
  ]) {
    if (!Array.isArray(doc[field])) {
      return false;
    }
  }
  if (
    !Array.isArray(doc["working_set"]) ||
    !doc["working_set"].every(isSerializedFocusRefDocument)
  ) {
    return false;
  }
  if (
    !Array.isArray(doc["subscriptions"]) ||
    !doc["subscriptions"].every((s) => typeof s === "string")
  ) {
    return false;
  }
  if (!Array.isArray(doc["queries"]) || !doc["queries"].every(isPlannedQuery)) {
    return false;
  }
  if (
    !Array.isArray(doc["prefetch_keys"]) ||
    !doc["prefetch_keys"].every((s) => typeof s === "string")
  ) {
    return false;
  }
  if (!Array.isArray(doc["context_candidates"])) {
    return false;
  }
  for (const raw of doc["context_candidates"]) {
    if (typeof raw !== "object" || raw === null) {
      return false;
    }
    const candidate = raw as Record<string, unknown>;
    if (
      !isSerializedFocusRefDocument(candidate["focus"]) ||
      typeof candidate["detail"] !== "string" ||
      typeof candidate["mandatory"] !== "boolean" ||
      !(
        candidate["role"] === "relation" ||
        candidate["role"] === "mandatory_reference" ||
        candidate["role"] === "mandatory_governance" ||
        candidate["role"] === "mandatory_evidence"
      )
    ) {
      return false;
    }
  }
  const counters = doc["counters"];
  if (typeof counters !== "object" || counters === null) {
    return false;
  }
  const c = counters as Record<string, unknown>;
  return (
    typeof c["subscriptions"] === "number" &&
    typeof c["queries"] === "number" &&
    typeof c["prefetches"] === "number" &&
    typeof c["context_candidates"] === "number"
  );
}

/**
 * Re-admit a boundary document into a MaterializationPlan: every FocusRef is
 * resolved through the identity registry, so the result refers to the SAME
 * interned identities an inline fold would have produced. The document is
 * untrusted input: it is validated before admission, and its counters must
 * match the delivered arrays exactly (integrity, not decoration).
 */
export function readmitPlanResult(raw: unknown): MaterializationPlan {
  if (!isSerializedMaterializationResult(raw)) {
    throw new FoldContractError("readmitPlanResult", "response does not match the declared schema");
  }
  const readmitFocus = (doc: SerializedFocusRefDocument): FocusRef => {
    try {
      return FocusRef.fromJSON(doc);
    } catch (error) {
      if (error instanceof MalformedFocusRefError) {
        throw new FoldContractError("readmitPlanResult", `focus ref rejected: ${error.message}`);
      }
      throw error;
    }
  };
  const counters: MaterializationCounters = Object.freeze({ ...raw.counters });
  const plan: MaterializationPlan = Object.freeze({
    horizon_id: raw.horizon_id,
    working_set: Object.freeze(raw.working_set.map(readmitFocus)),
    subscriptions: Object.freeze([...raw.subscriptions]),
    queries: Object.freeze(raw.queries.map((query) => Object.freeze({ ...query }))),
    prefetch_keys: Object.freeze([...raw.prefetch_keys]),
    context_candidates: Object.freeze(
      raw.context_candidates.map(
        (candidate): ContextCandidate =>
          Object.freeze({
            focus: readmitFocus(candidate.focus),
            role: candidate.role,
            detail: candidate.detail,
            mandatory: candidate.mandatory,
          }),
      ),
    ),
    counters,
  });
  if (
    plan.counters.subscriptions !== plan.subscriptions.length ||
    plan.counters.queries !== plan.queries.length ||
    plan.counters.prefetches !== plan.prefetch_keys.length ||
    plan.counters.context_candidates !== plan.context_candidates.length
  ) {
    throw new FoldContractError(
      "readmitPlanResult",
      "declared counters do not match the delivered arrays",
    );
  }
  return plan;
}

// --- Wire contract: task descriptor in, snapshot/invalidation outcome out ---------

/**
 * Requests the host sends to a fold executor. Worlds cross once per generation
 * (off the interaction path); plan tasks then reference the installed world by
 * generation and carry only the tiny serialized horizon (or null for the
 * whole-Case world).
 */
export type FoldWorkerRequest =
  | { readonly type: "install_world"; readonly generation: string; readonly world: ProjectionWorld }
  | { readonly type: "drop_world"; readonly generation: string }
  | {
      readonly type: "plan_materialization";
      readonly task_id: string;
      readonly generation: string;
      readonly horizon_json: string | null;
    };

/** Responses a fold executor delivers back to the host. */
export type FoldWorkerResponse =
  | {
      readonly type: "world_installed";
      readonly generation: string;
      readonly rows: {
        readonly units: number;
        readonly evidence: number;
        readonly sessions: number;
      };
    }
  | {
      readonly type: "fold_snapshot";
      readonly task_id: string;
      readonly generation: string;
      readonly result: SerializedMaterializationResult;
    }
  | {
      readonly type: "fold_error";
      readonly task_id: string;
      readonly generation: string;
      readonly reason: string;
    };

/** Untrusted worker output is validated against the declared schema before admission. */
export function isFoldWorkerResponse(value: unknown): value is FoldWorkerResponse {
  if (typeof value !== "object" || value === null) {
    return false;
  }
  const doc = value as Record<string, unknown>;
  if (typeof doc["generation"] !== "string") {
    return false;
  }
  switch (doc["type"]) {
    case "world_installed": {
      const rows = doc["rows"];
      if (typeof rows !== "object" || rows === null) {
        return false;
      }
      const r = rows as Record<string, unknown>;
      return (
        typeof r["units"] === "number" &&
        typeof r["evidence"] === "number" &&
        typeof r["sessions"] === "number"
      );
    }
    case "fold_snapshot": {
      return typeof doc["task_id"] === "string" && isSerializedMaterializationResult(doc["result"]);
    }
    case "fold_error": {
      return typeof doc["task_id"] === "string" && typeof doc["reason"] === "string";
    }
    default:
      return false;
  }
}

export interface FoldWorldReceipt {
  readonly generation: string;
  readonly units: number;
  readonly evidence: number;
  readonly sessions: number;
}

/**
 * A fold outcome is the Model-layer view of a worker response: snapshots carry an
 * already-re-admitted MaterializationPlan; invalidations and errors carry the
 * reason. Errors are preserved, never converted into success.
 */
export type FoldOutcome =
  | {
      readonly kind: "snapshot";
      readonly task_id: string;
      readonly generation: string;
      readonly result: MaterializationPlan;
    }
  | {
      readonly kind: "invalidation";
      readonly task_id: string | null;
      readonly generation: string;
      readonly reason: string;
    }
  | {
      readonly kind: "error";
      readonly task_id: string | null;
      readonly generation: string;
      readonly reason: string;
    };

export type FoldOutcomeHandler = (outcome: FoldOutcome) => void;

/**
 * A fold execution placement. `inline` computes on the render path synchronously;
 * `worker` computes in a Bun worker and hands snapshots back. Which placement a
 * given fold uses is decided by measurement (prereg D1), never preference.
 */
export interface FoldExecutor {
  readonly placement: "inline" | "worker";
  /** Install (or replace) the world snapshot for a generation. Off the interaction path. */
  installWorld(world: ProjectionWorld): Promise<FoldWorldReceipt>;
  /**
   * Fire-and-forget dispatch of one fold task. The snapshot/invalidation outcome
   * arrives through the registered outcome handlers — never as a return value.
   */
  dispatchPlan(generation: string, horizonJson: string | null, taskId: string): void;
  /** Drop a superseded generation's world from the executor. */
  dropWorld(generation: string): void;
  /** Register an outcome handler; returns an unsubscribe function. */
  onOutcome(handler: FoldOutcomeHandler): () => void;
  dispose(): void | Promise<void>;
}

/** Serialize a horizon to its boundary form (null stays null: the whole-Case world). */
export function serializeHorizonForTask(horizon: Horizon | null): string | null {
  return horizon === null ? null : horizonToJSON(horizon);
}

// --- Inline executor (the synchronous in-process placement / deterministic fake) --

export class InlineFoldExecutor implements FoldExecutor {
  readonly placement = "inline" as const;
  private worlds = new Map<string, ProjectionWorld>();
  private handlers = new Set<FoldOutcomeHandler>();

  installWorld(world: ProjectionWorld): Promise<FoldWorldReceipt> {
    this.worlds.set(world.generation, world);
    return Promise.resolve({
      generation: world.generation,
      units: world.units.length,
      evidence: world.evidence.length,
      sessions: world.sessions.length,
    });
  }

  /**
   * Synchronous computation delivered synchronously: deterministic in tests, and
   * its dispatch cost IS the honest render-path cost of inline placement.
   */
  dispatchPlan(generation: string, horizonJson: string | null, taskId: string): void {
    const world = this.worlds.get(generation);
    if (world === undefined) {
      this.emit({
        kind: "error",
        task_id: taskId,
        generation,
        reason: "world_not_installed",
      });
      return;
    }
    try {
      const horizon = horizonJson === null ? null : horizonFromJSON(JSON.parse(horizonJson));
      const plan = planMaterialization(world, horizon);
      this.emit({ kind: "snapshot", task_id: taskId, generation, result: plan });
    } catch (error) {
      this.emit({
        kind: "error",
        task_id: taskId,
        generation,
        reason: `fold_failed: ${String(error)}`,
      });
    }
  }

  dropWorld(generation: string): void {
    this.worlds.delete(generation);
  }

  onOutcome(handler: FoldOutcomeHandler): () => void {
    this.handlers.add(handler);
    return () => {
      this.handlers.delete(handler);
    };
  }

  dispose(): void {
    this.worlds.clear();
    this.handlers.clear();
  }

  private emit(outcome: FoldOutcome): void {
    for (const handler of [...this.handlers]) {
      handler(outcome);
    }
  }
}

/**
 * Scripted executor for determinstic invalidation/error tests: it delivers
 * pre-canned outcomes in order and records every dispatched task.
 */
export class ScriptedFoldExecutor implements FoldExecutor {
  readonly placement: "inline" | "worker";
  private outcomes: FoldOutcome[];
  private handlers = new Set<FoldOutcomeHandler>();
  readonly dispatched: { generation: string; horizonJson: string | null; taskId: string }[] = [];
  readonly droppedWorlds: string[] = [];

  constructor(placement: "inline" | "worker", outcomes: readonly FoldOutcome[]) {
    this.placement = placement;
    this.outcomes = [...outcomes];
  }

  installWorld(world: ProjectionWorld): Promise<FoldWorldReceipt> {
    return Promise.resolve({
      generation: world.generation,
      units: world.units.length,
      evidence: world.evidence.length,
      sessions: world.sessions.length,
    });
  }

  dispatchPlan(generation: string, horizonJson: string | null, taskId: string): void {
    this.dispatched.push({ generation, horizonJson, taskId });
    const outcome = this.outcomes.shift();
    if (outcome !== undefined) {
      for (const handler of [...this.handlers]) {
        handler(outcome);
      }
    }
  }

  dropWorld(generation: string): void {
    this.droppedWorlds.push(generation);
  }

  onOutcome(handler: FoldOutcomeHandler): () => void {
    this.handlers.add(handler);
    return () => {
      this.handlers.delete(handler);
    };
  }

  dispose(): void {
    this.handlers.clear();
  }
}

// --- Coordinator: the Model-layer consumer of fold outcomes -----------------------

export type FoldAcceptStatus = "applied" | "stale_discarded" | "error_discarded" | "unknown_task";

export interface FoldAcceptResult {
  readonly status: FoldAcceptStatus;
  readonly reason: string | null;
  /** The plan now live for the render path (unchanged when an outcome was discarded). */
  readonly plan: MaterializationPlan | null;
}

export interface FoldDispatchResult {
  readonly task_id: string;
  /** Main-thread cost of the interaction under this executor's placement. */
  readonly dispatch_ms: number;
}

/**
 * Model-layer coordinator for fold tasks. Interactions submit; results arrive
 * asynchronously and are accepted only while both the generation and the task
 * are still current — everything else is a preserved invalidation. The render
 * path consumes `acceptedPlan`, which never regresses to a stale fold.
 */
export class MaterializationFoldCoordinator {
  private sequence = 0;
  private latestTaskId: string | null = null;
  private installedGeneration: string | null = null;
  private accepted: MaterializationPlan | null = null;
  private lastDiscard: { status: FoldAcceptStatus; reason: string | null } | null = null;
  readonly stats = { submitted: 0, accepted: 0, stale_discarded: 0, error_discarded: 0 };

  constructor(private readonly executor: FoldExecutor) {}

  /**
   * Submit one fold task for the model's active generation. The first submit for
   * a generation carries the world across the boundary (measured separately,
   * off the interaction path); subsequent submits in that generation pay only
   * the descriptor dispatch.
   */
  async submit(
    world: ProjectionWorld,
    horizon: Horizon | null,
    activeGeneration: string,
  ): Promise<FoldDispatchResult> {
    const start = performance.now();
    if (this.installedGeneration !== activeGeneration) {
      const previous = this.installedGeneration;
      await this.executor.installWorld(world);
      if (previous !== null) {
        this.executor.dropWorld(previous);
      }
      this.installedGeneration = activeGeneration;
    }
    this.sequence += 1;
    const taskId = `fold:${activeGeneration}:${this.sequence}`;
    this.latestTaskId = taskId;
    this.stats.submitted += 1;
    this.executor.dispatchPlan(activeGeneration, serializeHorizonForTask(horizon), taskId);
    return { task_id: taskId, dispatch_ms: performance.now() - start };
  }

  /**
   * Apply one outcome against the model's CURRENT generation. A snapshot is
   * admitted only when its generation and its task are both still current;
   * anything else is discarded as a preserved invalidation or error.
   */
  accept(outcome: FoldOutcome, activeGeneration: string): FoldAcceptResult {
    if (outcome.kind === "error") {
      this.stats.error_discarded += 1;
      this.lastDiscard = { status: "error_discarded", reason: outcome.reason };
      return { status: "error_discarded", reason: outcome.reason, plan: this.accepted };
    }
    if (outcome.kind === "invalidation") {
      this.stats.stale_discarded += 1;
      this.lastDiscard = { status: "stale_discarded", reason: outcome.reason };
      return { status: "stale_discarded", reason: outcome.reason, plan: this.accepted };
    }
    if (outcome.generation !== activeGeneration) {
      this.stats.stale_discarded += 1;
      const reason = `superseded_generation: fold computed for '${outcome.generation}', active is '${activeGeneration}'`;
      this.lastDiscard = { status: "stale_discarded", reason };
      return { status: "stale_discarded", reason, plan: this.accepted };
    }
    if (outcome.task_id !== this.latestTaskId) {
      this.stats.stale_discarded += 1;
      const reason = `superseded_task: '${outcome.task_id}' is not the latest submitted task`;
      this.lastDiscard = { status: "stale_discarded", reason };
      return { status: "stale_discarded", reason, plan: this.accepted };
    }
    this.stats.accepted += 1;
    this.accepted = outcome.result;
    return { status: "applied", reason: null, plan: this.accepted };
  }

  /** Wire outcome acceptance to a live generation getter (the model's current generation). */
  attach(getActiveGeneration: () => string): () => void {
    return this.executor.onOutcome((outcome) => {
      this.accept(outcome, getActiveGeneration());
    });
  }

  /** The plan the render path should currently consume (null until one is accepted). */
  get acceptedPlan(): MaterializationPlan | null {
    return this.accepted;
  }

  get lastDiscardReason(): { status: FoldAcceptStatus; reason: string | null } | null {
    return this.lastDiscard;
  }

  dispose(): void {
    void this.executor.dispose();
  }
}

// --- Real Bun worker executor ------------------------------------------------------

/**
 * Generate the worker entry module. The entry is a thin adapter: it imports the
 * canonical fold implementation (horizon.ts) and the boundary serializers (this
 * module) by absolute file URL, so the worker runs the SAME code as the inline
 * path — there is no second fold implementation to drift.
 */
export function buildFoldWorkerEntryModule(): string {
  const horizonUrl = new URL("./horizon.ts", import.meta.url).href;
  const selfUrl = new URL("./worker-folds.ts", import.meta.url).href;
  return `
import { horizonFromJSON, planMaterialization } from ${JSON.stringify(horizonUrl)};
import { serializePlanResult } from ${JSON.stringify(selfUrl)};
const worlds = new Map();
self.onmessage = (event) => {
  const msg = event.data;
  if (typeof msg !== "object" || msg === null) {
    return;
  }
  if (msg.type === "install_world") {
    worlds.set(msg.generation, msg.world);
    self.postMessage({
      type: "world_installed",
      generation: msg.generation,
      rows: {
        units: msg.world.units.length,
        evidence: msg.world.evidence.length,
        sessions: msg.world.sessions.length,
      },
    });
    return;
  }
  if (msg.type === "drop_world") {
    worlds.delete(msg.generation);
    return;
  }
  if (msg.type === "plan_materialization") {
    const world = worlds.get(msg.generation);
    if (world === undefined) {
      self.postMessage({
        type: "fold_error",
        task_id: msg.task_id,
        generation: msg.generation,
        reason: "world_not_installed",
      });
      return;
    }
    try {
      const horizon = msg.horizon_json === null ? null : horizonFromJSON(JSON.parse(msg.horizon_json));
      const plan = planMaterialization(world, horizon);
      self.postMessage({
        type: "fold_snapshot",
        task_id: msg.task_id,
        generation: msg.generation,
        result: serializePlanResult(plan),
      });
    } catch (error) {
      self.postMessage({
        type: "fold_error",
        task_id: msg.task_id,
        generation: msg.generation,
        reason: "fold_failed: " + String(error),
      });
    }
    return;
  }
};
`;
}

/**
 * Real Bun Worker placement. Requests are posted as structured-clone-safe
 * documents; responses are validated (`isFoldWorkerResponse`) before they are
 * converted into outcomes, so malformed worker output becomes a preserved
 * error outcome instead of admitted state.
 */
export class BunWorkerFoldExecutor implements FoldExecutor {
  readonly placement = "worker" as const;
  private worker: Worker;
  private entryDir: string;
  private handlers = new Set<FoldOutcomeHandler>();
  private pendingInstalls = new Map<string, (receipt: FoldWorldReceipt) => void>();
  private outstandingTasks = new Set<string>();
  private queuedPlans = new Map<string, { horizonJson: string | null; taskId: string }>();
  private flushScheduled = false;
  private disposed = false;

  constructor() {
    this.entryDir = mkdtempSync(join(tmpdir(), "gauntlet-t17-fold-worker-"));
    const entryPath = join(this.entryDir, "fold-worker-entry.ts");
    writeFileSync(entryPath, buildFoldWorkerEntryModule());
    this.worker = new Worker(entryPath);
    this.worker.onmessage = (event: MessageEvent) => this.handleMessage(event.data);
    this.worker.onerror = (event: ErrorEvent) => {
      const reason = `worker_error: ${event.message}`;
      for (const taskId of [...this.outstandingTasks]) {
        this.emit({ kind: "error", task_id: taskId, generation: "", reason });
      }
      this.outstandingTasks.clear();
    };
  }

  installWorld(world: ProjectionWorld): Promise<FoldWorldReceipt> {
    return new Promise<FoldWorldReceipt>((resolve) => {
      this.pendingInstalls.set(world.generation, resolve);
      const request: FoldWorkerRequest = {
        type: "install_world",
        generation: world.generation,
        world,
      };
      this.worker.postMessage(request);
    });
  }

  dispatchPlan(generation: string, horizonJson: string | null, taskId: string): void {
    this.outstandingTasks.add(taskId);
    // Latest-wins coalescing (semantics-preserving): a newer dispatch for a
    // generation replaces any not-yet-posted task for that generation, because
    // the coordinator only ever accepts the latest submitted task — a skipped
    // intermediate task is exactly the result the coordinator would have
    // discarded as `superseded_task`. The post is flushed at the end of the
    // current macrotask, so rapid interaction bursts collapse into the folds
    // that can still matter, and the fold backlog behind a movement stream
    // stays bounded.
    // A superseded, never-posted task id is no longer outstanding.
    const superseded = this.queuedPlans.get(generation);
    if (superseded !== undefined) {
      this.outstandingTasks.delete(superseded.taskId);
    }
    this.queuedPlans.set(generation, { horizonJson, taskId });
    if (this.flushScheduled) {
      return;
    }
    this.flushScheduled = true;
    queueMicrotask(() => {
      this.flushScheduled = false;
      for (const [queuedGeneration, queued] of this.queuedPlans) {
        const request: FoldWorkerRequest = {
          type: "plan_materialization",
          task_id: queued.taskId,
          generation: queuedGeneration,
          horizon_json: queued.horizonJson,
        };
        this.worker.postMessage(request);
      }
      this.queuedPlans.clear();
    });
  }

  dropWorld(generation: string): void {
    const request: FoldWorkerRequest = { type: "drop_world", generation };
    this.worker.postMessage(request);
  }

  onOutcome(handler: FoldOutcomeHandler): () => void {
    this.handlers.add(handler);
    return () => {
      this.handlers.delete(handler);
    };
  }

  dispose(): void {
    if (this.disposed) {
      return;
    }
    this.disposed = true;
    this.worker.terminate();
    this.handlers.clear();
    this.pendingInstalls.clear();
    this.outstandingTasks.clear();
    this.queuedPlans.clear();
    rmSync(this.entryDir, { recursive: true, force: true });
  }

  private emit(outcome: FoldOutcome): void {
    for (const handler of [...this.handlers]) {
      handler(outcome);
    }
  }

  private handleMessage(data: unknown): void {
    if (!isFoldWorkerResponse(data)) {
      this.emit({
        kind: "error",
        task_id: null,
        generation: "",
        reason: "malformed_worker_response: did not match the declared fold response schema",
      });
      return;
    }
    if (data.type === "world_installed") {
      const resolve = this.pendingInstalls.get(data.generation);
      this.pendingInstalls.delete(data.generation);
      resolve?.({
        generation: data.generation,
        units: data.rows.units,
        evidence: data.rows.evidence,
        sessions: data.rows.sessions,
      });
      return;
    }
    if (data.type === "fold_error") {
      this.outstandingTasks.delete(data.task_id);
      this.emit({
        kind: "error",
        task_id: data.task_id,
        generation: data.generation,
        reason: data.reason,
      });
      return;
    }
    // fold_snapshot: re-admit identities through the registry BEFORE emitting.
    this.outstandingTasks.delete(data.task_id);
    try {
      const plan = readmitPlanResult(data.result);
      this.emit({
        kind: "snapshot",
        task_id: data.task_id,
        generation: data.generation,
        result: plan,
      });
    } catch (error) {
      this.emit({
        kind: "error",
        task_id: data.task_id,
        generation: data.generation,
        reason: `readmit_failed: ${String(error)}`,
      });
    }
  }
}

// --- Placement policy: measured, recorded, and honest (prereg D1 / redesign_trigger)

export interface PlacementMeasurement {
  readonly inline_ms: number;
  readonly worker_dispatch_ms: number;
  readonly worker_end_to_end_ms: number;
}

/**
 * Placement is decided from measurements against the FROZEN budgets (declared
 * in evidence T17/horizon-payment.md before any measurement):
 *   1. A fold whose inline cost is within one frame's interactive budget (B1
 *      median) blocks nothing — worker serialization/copying payment buys
 *      nothing, so it stays inline.
 *   2. A fold that DOES block the render path moves to a worker only if the
 *      dispatch cost is strictly below the inline fold it removes, and the
 *      end-to-end completion stays inside the frozen off-path fold budget (B2b).
 *   3. Otherwise the fold stays inline and the model contract keeps a different
 *      execution placement (plan redesign_trigger: workers are not a goal).
 */
export function decidePlacement(measurement: PlacementMeasurement): "inline" | "worker" {
  if (measurement.inline_ms <= FROZEN_BUDGETS.interactive_submit_median_ms) {
    return "inline";
  }
  if (measurement.worker_dispatch_ms >= measurement.inline_ms) {
    return "inline";
  }
  if (measurement.worker_end_to_end_ms > FROZEN_BUDGETS.whole_case_fold_p99_ms) {
    return "inline";
  }
  return "worker";
}

export interface RecordedPlacementDecision {
  readonly fold: "materialization_plan";
  readonly fixture: string;
  readonly decision: "inline" | "worker";
  readonly inline_ms: number;
  readonly worker_dispatch_ms: number;
  readonly worker_end_to_end_ms: number;
  readonly rationale: string;
}

/**
 * Placement decisions RECORDED FROM the frozen bench runs of this task (see
 * evidence T17/horizon-payment.md). Numbers are recorded after measurement;
 * the thresholds and fixture sizes they were measured against were frozen
 * before it. `placementDecisionsAreConsistent` holds every entry to its own
 * numbers, so the table can never quietly contradict its measurements.
 */
export const RECORDED_PLACEMENT_DECISIONS: readonly RecordedPlacementDecision[] = Object.freeze([
  {
    fold: "materialization_plan",
    fixture: "t17-small",
    decision: "inline",
    inline_ms: 0.0952,
    worker_dispatch_ms: 0.0026,
    worker_end_to_end_ms: 0.2722,
    rationale:
      "Measured (projection-worker.bench, frozen fixture): the inline fold costs ~0.1ms — five orders of magnitude below one frame's interactive budget — so it blocks nothing; the worker round trip (0.27ms end-to-end) adds serialization/copying payment without removing a perceptible blocker. Workers are not a goal (redesign_trigger honored).",
  },
  {
    fold: "materialization_plan",
    fixture: "t17-large",
    decision: "worker",
    inline_ms: 31.716,
    worker_dispatch_ms: 0.0097,
    worker_end_to_end_ms: 123.856,
    rationale:
      "Measured (projection-worker.bench, frozen fixture): the inline whole-Case fold blocks the render path ~32ms (p50; p99 40ms) per fold — beyond the 16.6ms frame budget; worker dispatch removes it from the render path at ~0.01ms per interaction while fold completion (124ms p50, 137ms p99) stays inside the frozen off-path budget (B2b).",
  },
]);

/** Every recorded decision must equal the placement its own numbers select. */
export function placementDecisionsAreConsistent(): boolean {
  return RECORDED_PLACEMENT_DECISIONS.every((entry) => entry.decision === decidePlacement(entry));
}

export function recordedPlacementFor(fixture: string): RecordedPlacementDecision | null {
  return RECORDED_PLACEMENT_DECISIONS.find((entry) => entry.fixture === fixture) ?? null;
}

// --- Deterministic synthetic Case generator (declared fixtures; prereg discipline) -

export const SYNTHETIC_CASE_SEED = 0x5eedc0de;

export interface SyntheticCaseSpec {
  readonly generation: string;
  readonly units: number;
  readonly evidence_per_unit: number;
  readonly sessions: number;
  readonly blocked_units: number;
  readonly focus_events: number;
  readonly seed?: number | undefined;
}

/** Frozen LARGE fixture (see evidence T17/horizon-payment.md — declared before measurement). */
export const T17_LARGE_FIXTURE: SyntheticCaseSpec = Object.freeze({
  generation: "gen-t17-large",
  units: 6000,
  evidence_per_unit: 1,
  sessions: 200,
  blocked_units: 30,
  focus_events: 10,
  seed: SYNTHETIC_CASE_SEED,
});

/** Frozen SMALL fixture: T08-scale parity world. */
export const T17_SMALL_FIXTURE: SyntheticCaseSpec = Object.freeze({
  generation: "gen-t17-small",
  units: 3,
  evidence_per_unit: 1,
  sessions: 4,
  blocked_units: 1,
  focus_events: 0,
  seed: SYNTHETIC_CASE_SEED,
});

/**
 * Deterministic synthetic Case: identical spec always yields identical events.
 * Seeded LCG, fixed emission order (units, evidence, sessions, focus), stable id
 * scheme (u-<i>, ev-<unit>-<k>, s-<tree>-<depth>). No Math.random anywhere.
 */
export function createSyntheticCaseEvents(spec: SyntheticCaseSpec): AuthoritativeEventFrame[] {
  let state = (spec.seed ?? SYNTHETIC_CASE_SEED) >>> 0;
  const next = (): number => {
    state = (Math.imul(state, 1664525) + 1013904223) >>> 0;
    return state;
  };
  const authorities = ["normal", "builder", "orchestrator"] as const;
  const blastRadii = ["isolated", "task", "workspace"] as const;
  const events: AuthoritativeEventFrame[] = [];
  for (let i = 0; i < spec.units; i += 1) {
    const blocked = i < spec.blocked_units;
    const status = blocked ? "blocked" : next() % 2 === 0 ? "active" : "settled";
    events.push({
      generation_id: spec.generation,
      source_id: "gauntlet",
      cursor: `u-cur-${i}`,
      event_type: "unit_updated",
      payload: {
        unit_id: `u-${i}`,
        title: `Synthetic unit ${i}`,
        phase: "execution",
        status,
        authority: authorities[next() % authorities.length],
        blast_radius: blastRadii[next() % blastRadii.length],
      },
      timestamp_ms: 1000 + i,
    });
  }
  for (let i = 0; i < spec.units; i += 1) {
    for (let k = 0; k < spec.evidence_per_unit; k += 1) {
      events.push({
        generation_id: spec.generation,
        source_id: "gauntlet",
        cursor: `e-cur-${i}-${k}`,
        event_type: "evidence_added",
        payload: {
          evidence_id: `ev-${i}-${k}`,
          unit_id: `u-${i}`,
          kind: "execution_evidence",
          locator: `.agents/reports/synthetic/ev-${i}-${k}.json`,
          verified: next() % 2 === 0,
        },
        timestamp_ms: 2000 + i,
      });
    }
  }
  const trees = 2;
  const perTree = Math.ceil(spec.sessions / trees);
  let emitted = 0;
  for (let tree = 0; tree < trees && emitted < spec.sessions; tree += 1) {
    for (let depth = 0; depth < perTree && emitted < spec.sessions; depth += 1) {
      emitted += 1;
      events.push({
        generation_id: spec.generation,
        source_id: "prime",
        cursor: `s-cur-${tree}-${depth}`,
        event_type: "session_updated",
        payload: {
          session_id: `s-${tree}-${depth}`,
          parent_id: depth === 0 ? null : `s-${tree}-${depth - 1}`,
          model: depth % 2 === 0 ? "model-a" : "model-b",
          status: "running",
        },
        timestamp_ms: 3000 + emitted,
      });
    }
  }
  for (let f = 0; f < spec.focus_events; f += 1) {
    events.push({
      generation_id: spec.generation,
      source_id: "gauntlet",
      cursor: `f-cur-${f}`,
      event_type: "focus_changed",
      payload: { kind: "unit", id: `u-${f % spec.units}` },
      timestamp_ms: 4000 + f,
    });
  }
  return events;
}

// --- Payment instrumentation helpers (shared by both T17 benches) -----------------

export interface LatencySampleSummary {
  readonly n: number;
  readonly mean_ms: number;
  readonly p50_ms: number;
  readonly p95_ms: number;
  readonly p99_ms: number;
  readonly max_ms: number;
}

export function summarizeLatencySamples(samples: readonly number[]): LatencySampleSummary {
  if (samples.length === 0) {
    return Object.freeze({ n: 0, mean_ms: 0, p50_ms: 0, p95_ms: 0, p99_ms: 0, max_ms: 0 });
  }
  const sorted = [...samples].sort((a, b) => a - b);
  const percentile = (fraction: number): number => {
    const index = Math.min(sorted.length - 1, Math.ceil(fraction * sorted.length) - 1);
    return sorted[index] ?? 0;
  };
  const total = sorted.reduce((sum, value) => sum + value, 0);
  return Object.freeze({
    n: sorted.length,
    mean_ms: total / sorted.length,
    p50_ms: percentile(0.5),
    p95_ms: percentile(0.95),
    p99_ms: percentile(0.99),
    max_ms: sorted[sorted.length - 1] ?? 0,
  });
}

export function peakRssBytes(): number {
  return process.memoryUsage().rss;
}

/** Project a WorkbenchSnapshot slice into the ProjectionWorld the folds read. */
export function worldFromSlices(
  generation: string,
  units: readonly UnitProjection[],
  evidence: readonly EvidenceProjection[],
  sessions: readonly SessionProjection[],
  source_cursors: Readonly<Record<string, string>>,
): ProjectionWorld {
  return { generation, units, evidence, sessions, source_cursors };
}
