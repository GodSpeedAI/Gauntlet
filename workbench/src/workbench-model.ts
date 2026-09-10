import type { FocusPlacement, FocusPlacementInit } from "./focus";
import { createPlacement, FocusRef } from "./focus";
import type {
  AttentionEntry,
  ContextCandidate,
  Horizon,
  MaterializationCounters,
  SavedHorizonRecord,
} from "./horizon";
import {
  createHorizon,
  DEFAULT_HOIST_RELATION_QUERIES,
  deriveAttention,
  isHorizon,
  MalformedSavedHorizonsError,
  type MaterializationPlan,
  planMaterialization,
  SavedHorizonRegistry,
} from "./horizon";
import {
  type AuthoritativeEventFrame,
  type ProjectionGeneration,
  SqliteProjectionStore,
  type UnitProjection,
  type WorkbenchSnapshot,
} from "./projection-store";
import type { ProjectionWorld, RelationQuerySpec } from "./relations";
import { deriveRelations } from "./relations";
import { type FoldExecutor, MaterializationFoldCoordinator } from "./worker-folds";

/**
 * React-free Workbench Model and immutable snapshot fan-out.
 * Conforms to spec §20.2, REQ-DATA-001, REQ-DATA-005, REQ-DATA-006, REQ-TUI-001,
 * REQ-FOCUS-001..005, REQ-GOAL-001, REQ-GOAL-006, REQ-OPS-004, REQ-UX-002, and plan T07/T08.
 *
 * Core Invariants:
 *   - Presentation code consumes immutable snapshots and interaction callbacks; it NEVER
 *     owns business state or communicates directly with backend adapters (REQ-DATA-001, REQ-TUI-001).
 *   - Chat and Trace are separate projections over durable history; Chat is NOT canonical
 *     input for Trace (REQ-DATA-005, Attack 1).
 *   - One source event fans out to multiple projections non-destructively; no view can
 *     consume an event away from another view (REQ-DATA-006, Attack 2).
 *   - Focus selection and every placement resolve to one interned FocusRef identity;
 *     hoist/de-hoist changes the active Horizon only — never source truth or object
 *     identity (REQ-FOCUS-001..003, DSN-006).
 *   - Case-wide attention is derived from the whole Case and stays independent of the
 *     active Horizon; capability observations never fake availability (REQ-GOAL-001,
 *     REQ-AFF-004/005, REQ-OPS-004).
 *   - T20 planner swap (T17 obligation): with a fold executor configured, the
 *     materialization plan refreshes off the render path through the
 *     MaterializationFoldCoordinator; snapshots carry the last-known plan for the
 *     exact current context and the inline fold remains the immediate fallback —
 *     the model never blocks rendering on the worker (REQ-BUN-006).
 */

/** Typed error carrying the failing WorkbenchModel operation (failure locus preserved). */
export class WorkbenchModelError extends Error {
  constructor(operation: string, detail: string) {
    super(`workbench model ${operation}: ${detail}`);
    this.name = "WorkbenchModelError";
  }
}

/**
 * Publish a length-bounded immutable view over an append-only history array
 * under the frozen-array contract, with O(1) publication cost (T21 correction;
 * COR-2 removed the last O(history) per-event cost — the full-history copy).
 *
 * Snapshot arrays are immutable: every mutation attempt — push, index write,
 * delete, in-place sort/reverse, length reassignment, prototype swap,
 * freeze/preventExtensions — throws a TypeError, exactly as Object.freeze
 * makes it. The previous COR-1 mechanism copied the history and handed out a
 * guarded view of the copy: correct, but the copy itself was O(history) per
 * published snapshot (~1 MB and ~10 ms per event at the TARGET fixture's
 * 100k+ record Trace history — the measured HC9a stream-tail contributor).
 * This view instead bounds the model's OWN history array at its
 * publication-time length. That is sound because the history backing is
 * append-only between whole-array replacements: records are pushed, never
 * mutated in place, and resets (generation start, chat disable) replace the
 * array object — so the prefix a view was published at can never change, and
 * every snapshot observes exactly the records that existed at its publication,
 * forever. Appends after publication are invisible (length and out-of-range
 * indices are bounded by the published length). Publication allocates one
 * proxy: zero copies, zero per-event history garbage.
 *
 * Divergences from Object.freeze, both disclosed and swept (see T21 evidence,
 * Correction Round 2): Object.isFrozen(view) reports false (the immutability
 * guard is enforced by traps, not extensibility state), and Object.freeze(view)
 * THROWS (a freeze attempt must not silently non-extensible the shared backing,
 * which would break future appends) where freezing an already-frozen array was
 * previously a silent no-op. No runtime or test observer of snapshot arrays
 * performs either operation.
 */
function immutableHistoryView<T>(backing: readonly T[], lengthAtPublication: number): readonly T[] {
  const publishedLength = lengthAtPublication;
  const refuse = (): never => {
    throw new TypeError("snapshot arrays are immutable (frozen-array contract)");
  };
  const indexKey = (property: PropertyKey): number | null => {
    if (typeof property !== "string") {
      return null;
    }
    const index = Number(property);
    return Number.isInteger(index) && index >= 0 ? index : null;
  };
  return new Proxy(backing, {
    get(target, property, receiver) {
      if (property === "length") {
        return publishedLength;
      }
      const index = indexKey(property);
      if (index !== null) {
        return index < publishedLength ? target[index] : undefined;
      }
      return Reflect.get(target, property, receiver);
    },
    has(target, property) {
      if (property === "length") {
        return true;
      }
      const index = indexKey(property);
      if (index !== null) {
        return index < publishedLength;
      }
      return Reflect.has(target, property);
    },
    ownKeys() {
      const keys: string[] = new Array(publishedLength + 1);
      for (let i = 0; i < publishedLength; i += 1) {
        keys[i] = String(i);
      }
      keys[publishedLength] = "length";
      return keys;
    },
    getOwnPropertyDescriptor(target, property) {
      if (property === "length") {
        return { value: publishedLength, writable: true, enumerable: false, configurable: false };
      }
      const index = indexKey(property);
      if (index !== null) {
        if (index < publishedLength) {
          return { value: target[index], writable: true, enumerable: true, configurable: true };
        }
        return undefined;
      }
      return Reflect.getOwnPropertyDescriptor(target, property);
    },
    set: refuse,
    deleteProperty: refuse,
    defineProperty: refuse,
    setPrototypeOf: refuse,
    preventExtensions: refuse,
  });
}

export interface TraceRecord {
  readonly trace_id: string;
  readonly unit_id: string;
  readonly event_type: string;
  readonly summary: string;
  readonly timestamp_ms: number;
}

export interface ChatMessage {
  readonly message_id: string;
  readonly role: "user" | "assistant" | "system";
  readonly content: string;
  readonly timestamp_ms: number;
}

/** Names of the current-path gates an installed capability must pass to be available. */
export const AFFORDANCE_GATE_NAMES = [
  "visibility",
  "reachability",
  "payment",
  "governance",
  "settlement_access",
  "recovery",
] as const;

export type AffordanceGateName = (typeof AFFORDANCE_GATE_NAMES)[number];

/**
 * An environment-observed capability state declared to the model by the host.
 * The workbench never infers installation; it only represents what it observed
 * (REQ-AFF-004: installed MUST NOT imply available).
 */
export interface CapabilityObservation {
  readonly capability: string;
  readonly installed: boolean;
  readonly gates: Readonly<Record<AffordanceGateName, boolean>>;
}

export interface CapabilityStatus {
  readonly capability: string;
  readonly installed: boolean;
  readonly status: "available" | "unavailable";
  readonly reason_codes: readonly string[];
}

/**
 * Resolve one capability observation honestly: an installed capability is only
 * "available" when every current-path gate passes; otherwise it stays an
 * inspectable unavailable reason (REQ-AFF-004, REQ-AFF-005).
 */
export function resolveCapabilityStatus(observation: CapabilityObservation): CapabilityStatus {
  if (!observation.installed) {
    return Object.freeze({
      capability: observation.capability,
      installed: false,
      status: "unavailable",
      reason_codes: Object.freeze(["not_installed"]),
    });
  }
  const failedGates = AFFORDANCE_GATE_NAMES.filter((gate) => observation.gates[gate] !== true);
  if (failedGates.length === 0) {
    return Object.freeze({
      capability: observation.capability,
      installed: true,
      status: "available",
      reason_codes: Object.freeze([]),
    });
  }
  return Object.freeze({
    capability: observation.capability,
    installed: true,
    status: "unavailable",
    reason_codes: Object.freeze(failedGates.map((gate) => `${gate}_gate_not_passed`)),
  });
}

export interface MaterializationSummary {
  readonly horizon_id: string | null;
  readonly working_set_size: number;
  readonly counters: MaterializationCounters;
}

export interface HoistOptions {
  readonly relation_queries?: readonly RelationQuerySpec[] | undefined;
  readonly filters?: Record<string, string> | undefined;
  readonly saved_name?: string | undefined;
}

export interface FullWorkbenchSnapshot extends WorkbenchSnapshot {
  /** Current primary Focus as the spec-typed semantic identity (spec §WorkbenchSnapshot). */
  readonly focus: FocusRef | null;
  readonly trace_records: readonly TraceRecord[];
  readonly chat_messages: readonly ChatMessage[];
  readonly chat_enabled: boolean;
  /** Rendered occurrences published in this snapshot generation (spec §FocusPlacement). */
  readonly placements: readonly FocusPlacement[];
  /** The active Horizon entity, absent for the whole-Case (unhoisted) world. */
  readonly active_horizon: Horizon | null;
  /** Breadcrumb from Case through each hoisted horizon root (REQ-UX-002). */
  readonly horizon_breadcrumb: readonly string[];
  readonly materialization: MaterializationSummary;
  /** Case-wide urgent attention; independent of the active Horizon (REQ-UX-002). */
  readonly attention: readonly AttentionEntry[];
  /** Eligible context candidates; mandatory closure always included (REQ-OPS-004). */
  readonly context_candidates: readonly ContextCandidate[];
  /** Inspectable capability reasons; installed is never faked as available (REQ-AFF-004/005). */
  readonly capability_reasons: readonly CapabilityStatus[];
}

export type SnapshotListener = (snapshot: FullWorkbenchSnapshot) => void;

export interface WorkbenchModelOptions {
  readonly dbPath?: string;
  readonly enableChat?: boolean;
  /**
   * T20 planner swap (T17 MAJOR obligation): when a fold executor is provided,
   * the materialization plan refreshes OFF the render path through the
   * MaterializationFoldCoordinator while the inline fold remains the immediate
   * fallback. The model NEVER blocks rendering on the worker: snapshots carry
   * the last-known plan for the current (generation, horizon, world) context
   * and refresh when the worker completes; any context change falls back to
   * the inline fold until a fresh worker plan lands. Omitted (the default),
   * the model plans inline exactly as before T20.
   */
  readonly foldExecutor?: FoldExecutor;
}

/**
 * One assembled presentation input set, cached per world version (T21 correction):
 * the raw store snapshot, the ProjectionWorld the folds read, and Case-wide
 * attention — all derived from the same store read. The store's snapshot content
 * is horizon-independent (the horizon only tags snapshot meta and scopes the
 * plan), so the cache key is the world version plus the active generation id.
 */
interface BaseSnapshotCache {
  readonly version: number;
  readonly generationId: string | null;
  readonly base: WorkbenchSnapshot;
  readonly world: ProjectionWorld;
  readonly attention: readonly AttentionEntry[];
}

export class WorkbenchModel {
  readonly store: SqliteProjectionStore;
  private currentHorizon: Horizon | null = null;
  private currentFocus: FocusRef | null = null;
  /**
   * Peer correction MAJOR-2: the FocusRef attributed at admission time for the
   * latest store focus projection. focus_changed frames carry no source
   * provenance, so attribution happens BEFORE admission from the owning
   * projected row's source — never from a default. Unattributable frames are
   * refused and leave this null; nothing default-source is ever minted.
   */
  private storeFocusRef: FocusRef | null = null;
  private horizonStack: Horizon[] = [];
  private placements = new Map<string, FocusPlacement>();
  private capabilities = new Map<string, CapabilityObservation>();
  private savedHorizons = new SavedHorizonRegistry();
  private chatEnabled = true;

  // T20 planner swap: worker fold coordination (off-path plan refresh).
  private readonly foldCoordinator: MaterializationFoldCoordinator | null;
  private foldUnsubscribe: (() => void) | null = null;
  /** Bumped on every world/horizon/context change that stales a computed plan. */
  private planVersion = 0;
  /** The planVersion the last ACCEPTED worker plan was computed from. */
  private acceptedPlanVersion = -1;
  /** Guards against scheduling a second in-flight refresh for the same context. */
  private refreshInFlight = false;
  /** The planVersion captured when the current in-flight fold was dispatched. */
  private dispatchedPlanVersion = -1;

  // T21 correction (measured HC4/HC9a bottleneck): presentation-input caches.
  // Invalidation discipline mirrors the T20 plan cache: a version counter is
  // bumped by exactly the mutations that change the cached content, so identical
  // store/model states reuse identical content and identical store states assemble
  // byte-identical snapshots. No canonical truth is cached — the authoritative
  // store stays the only source; only its assembled presentation inputs are reused.
  /** Bumped when the authoritative world changes (applied events, generation start). */
  private worldVersion = 0;
  /** Bumped when the Trace history changes (applied events, generation start). */
  private traceVersion = 0;
  /** Bumped when the Chat history changes (chat push, generation start, disable). */
  private chatVersion = 0;
  /** Bumped when declared capability observations change. */
  private capabilitiesVersion = 0;
  /** Cached store snapshot + world + Case-wide attention for the current world version. */
  private baseCache: BaseSnapshotCache | null = null;
  /** Immutable bounded Trace history view for the current trace version. */
  private traceCache: { readonly version: number; readonly frozen: readonly TraceRecord[] } | null =
    null;
  /** Immutable bounded Chat history view for the current chat version. */
  private chatCache: { readonly version: number; readonly frozen: readonly ChatMessage[] } | null =
    null;
  /** Frozen resolved capability statuses for the current capabilities version. */
  private capabilityCache: {
    readonly version: number;
    readonly frozen: readonly CapabilityStatus[];
  } | null = null;
  /** Frozen horizon-scoped affordance ids for the current (world, plan, capabilities) context. */
  private affordanceCache: {
    readonly worldVersion: number;
    readonly plan: MaterializationPlan;
    readonly capabilitiesVersion: number;
    readonly frozen: readonly string[];
  } | null = null;

  /**
   * Invalidate the plan cache for a changed context. Clearing the in-flight
   * guard here lets a changed context schedule a fresh refresh even if an
   * earlier worker outcome never arrives (wedged executor): the coordinator
   * discards the superseded task whenever it does surface.
   */
  private invalidatePlan(): void {
    this.planVersion += 1;
    this.refreshInFlight = false;
  }

  // In-memory projection reducers (mirrored/derived from durable log).
  // Append-only discipline (COR-2 soundness invariant): records are pushed,
  // never mutated in place; resets replace the whole array object. Published
  // snapshot views (immutableHistoryView) are length-bounded at their
  // publication time, so their observed prefix is immutable forever.
  private traceRecords: TraceRecord[] = [];
  private chatMessages: ChatMessage[] = [];

  private listeners = new Set<SnapshotListener>();

  constructor(options?: WorkbenchModelOptions) {
    this.store = new SqliteProjectionStore(options?.dbPath ?? ":memory:");
    this.chatEnabled = options?.enableChat ?? true;
    const executor = options?.foldExecutor;
    this.foldCoordinator = executor ? new MaterializationFoldCoordinator(executor) : null;
    if (executor && this.foldCoordinator) {
      const coordinator = this.foldCoordinator;
      // Accept outcomes against the LIVE generation and publish the refreshed
      // plan. Every outcome (applied, discarded, or error) completes the
      // in-flight refresh for the latest task; stale frames were never
      // dispatched for superseded task ids (worker coalescing) or are
      // discarded by the coordinator's own invalidation contract.
      this.foldUnsubscribe = executor.onOutcome((outcome) => {
        const active = this.store.getActiveGeneration();
        if (active === null || active.status === "closed") {
          this.refreshInFlight = false;
          return;
        }
        const result = coordinator.accept(outcome, active.id);
        this.refreshInFlight = false;
        if (result.status === "applied") {
          // The plan was computed from the world as of the dispatch; record
          // that version so validity tracks the context the fold actually read.
          this.acceptedPlanVersion = this.dispatchedPlanVersion;
          // Re-publish so listeners receive the refreshed plan (never a stale one).
          this.notifySubscribers();
        }
      });
    }
  }

  /**
   * Start a new projection generation. Placement occurrences are generation-scoped
   * (spec §FocusPlacement) and horizons are bound to a generation, so both reset;
   * Focus identity is interned and stable across generations and is retained.
   */
  startGeneration(generationId: string): void {
    this.store.startGeneration(generationId);
    this.traceRecords = [];
    this.chatMessages = [];
    this.placements.clear();
    this.horizonStack = [];
    this.currentHorizon = null;
    this.storeFocusRef = null;
    // New generation = new world: any previously accepted plan is for a
    // superseded generation context.
    this.invalidatePlan();
    // New generation = new presentation inputs (T21 correction cache discipline).
    this.worldVersion += 1;
    this.traceVersion += 1;
    this.chatVersion += 1;
  }

  /**
   * Mark generation ready and publish initial snapshot.
   */
  markReady(generationId: string): void {
    this.store.markGenerationReady(generationId);
    this.notifySubscribers();
  }

  /**
   * Toggle Chat projection on or off (used to verify Chat is non-canonical for Trace).
   */
  setChatEnabled(enabled: boolean): void {
    this.chatEnabled = enabled;
    if (!enabled) {
      this.chatMessages = [];
      this.chatVersion += 1;
    }
    this.notifySubscribers();
  }

  isChatEnabled(): boolean {
    return this.chatEnabled;
  }

  /**
   * Subscribe to immutable snapshot updates.
   * Returns an unsubscribe function.
   */
  subscribe(listener: SnapshotListener): () => void {
    this.listeners.add(listener);
    // Emit current snapshot immediately upon subscription
    try {
      listener(this.getSnapshot());
    } catch {
      // ignore if generation not ready
    }

    return () => {
      this.listeners.delete(listener);
    };
  }

  /**
   * Apply an authoritative event frame.
   *
   * ATTACK 2 DEFENSE (Non-destructive Fan-out, REQ-DATA-006):
   * Fans out to:
   *   1. SQLite projection store (Work, Evidence, Sessions)
   *   2. Trace projection (audit/causal history)
   *   3. Chat projection (if enabled)
   * All projections receive the event; neither consumes or starves the other.
   *
   * PEER CORRECTION MAJOR-2: focus_changed frames carry no source provenance, so
   * the referenced object is attributed from the OWNING projected row's source
   * BEFORE admission. A frame that cannot be attributed to exactly one row is
   * refused with a typed reason instead of minting a default-source identity.
   */
  applyEvent(frame: AuthoritativeEventFrame): { applied: boolean; reason?: string } {
    let attributedFocus: FocusRef | null = null;
    if (frame.event_type === "focus_changed" && this.storeWouldApply(frame)) {
      attributedFocus = this.attributeStoreFocus(frame.payload["kind"], frame.payload["id"]);
      if (attributedFocus === null) {
        return {
          applied: false,
          reason: `unattributable_focus_change: '${String(frame.payload["kind"])}:${String(frame.payload["id"])}' does not resolve to exactly one projected row source`,
        };
      }
    }

    // 1. Apply to SQLite store
    const res = this.store.applyEvent(frame);
    if (!res.applied) {
      return res;
    }
    if (attributedFocus !== null) {
      this.storeFocusRef = attributedFocus;
    }
    // The authoritative world changed: a previously accepted plan no longer
    // describes it (T20 planner swap validity tracking), and the presentation
    // inputs derived from it are stale (T21 correction cache discipline).
    this.invalidatePlan();
    this.worldVersion += 1;
    this.traceVersion += 1;

    // 2. Fan-out to Trace projection reducer
    this.traceRecords.push({
      trace_id: `trace-${frame.source_id}-${frame.cursor}`,
      unit_id: String(frame.payload["unit_id"] ?? "unknown"),
      event_type: frame.event_type,
      summary: `[${frame.source_id}] ${frame.event_type} (${frame.cursor})`,
      timestamp_ms: frame.timestamp_ms,
    });

    // 3. Fan-out to Chat projection reducer (if enabled and applicable)
    if (this.chatEnabled && frame.event_type === "unit_updated") {
      this.chatMessages.push({
        message_id: `chat-${frame.cursor}`,
        role: "system",
        content: `Unit ${frame.payload["unit_id"]} updated: status = ${frame.payload["status"]}`,
        timestamp_ms: frame.timestamp_ms,
      });
      this.chatVersion += 1;
    }

    // 4. Notify subscribers with a new immutable snapshot
    this.notifySubscribers();
    return { applied: true };
  }

  /**
   * Interaction callback: select the active Focus by its stable semantic identity.
   * Selection retains identity across views; it never mints or forks an object.
   */
  selectFocus(ref: FocusRef): void {
    if (!(ref instanceof FocusRef)) {
      throw new WorkbenchModelError("selectFocus", "expected a FocusRef");
    }
    this.currentFocus = ref;
    this.notifySubscribers();
  }

  /**
   * Interaction callback: activate a specific Horizon (or none for the whole-Case
   * world). Activating a horizon resets the hoist stack to that horizon.
   * Peer correction MINOR: fabricated horizons are rejected here, at the API
   * boundary, with the typed error — never inside snapshot assembly, where the
   * blanket catch would silently starve subscribers.
   */
  selectHorizon(horizon: Horizon | null): void {
    if (horizon !== null && !isHorizon(horizon)) {
      throw new WorkbenchModelError("selectHorizon", "expected a Horizon or null");
    }
    this.currentHorizon = horizon;
    this.horizonStack = horizon === null ? [] : [horizon];
    this.invalidatePlan();
    this.notifySubscribers();
  }

  /**
   * Hoist to a Focus: build a Horizon rooted at the ref for the active generation,
   * push it onto the hoist stack, and narrow the active projection (REQ-FOCUS-003).
   * Narrowing changes representation/materialization only — never source truth.
   */
  hoistToFocus(ref: FocusRef, options: HoistOptions = {}): Horizon {
    if (!(ref instanceof FocusRef)) {
      throw new WorkbenchModelError("hoistToFocus", "expected a FocusRef");
    }
    const generation = this.requireActiveGeneration("hoistToFocus");
    const horizon = createHorizon({
      root_focus: ref,
      generation: generation.id,
      relation_queries: options.relation_queries ?? DEFAULT_HOIST_RELATION_QUERIES,
      filters: options.filters,
      saved_name: options.saved_name,
    });
    this.horizonStack.push(horizon);
    this.currentHorizon = horizon;
    this.currentFocus = ref;
    this.invalidatePlan();
    this.notifySubscribers();
    return horizon;
  }

  /**
   * One-step return (REQ-UX-002): pop the hoist stack. Popping the last horizon
   * restores the whole-Case world with the broader population, same identities.
   */
  dehoist(): Horizon | null {
    this.horizonStack.pop();
    const top = this.horizonStack[this.horizonStack.length - 1] ?? null;
    this.currentHorizon = top;
    this.invalidatePlan();
    this.notifySubscribers();
    return top;
  }

  /**
   * Register one rendered occurrence of a FocusRef on a surface/view. Placement
   * records carry no canonical state; placing never forks the FocusRef.
   */
  placeFocus(
    surface: string,
    view: string,
    ref: FocusRef,
    init: FocusPlacementInit = {},
  ): FocusPlacement {
    if (surface.length === 0 || view.length === 0) {
      throw new WorkbenchModelError("placeFocus", "surface and view must be non-empty");
    }
    if (!(ref instanceof FocusRef)) {
      throw new WorkbenchModelError("placeFocus", "expected a FocusRef");
    }
    const generation = this.requireActiveGeneration("placeFocus");
    const placement = createPlacement(generation.id, surface, view, ref, init);
    this.placements.set(placement.id, placement);
    this.notifySubscribers();
    return placement;
  }

  /**
   * Mutate ONE placement-local label. Returns a new immutable placement; the
   * FocusRef and every other placement are untouched (REQ-FOCUS-002, DSN-006).
   */
  relabelPlacement(placementId: string, label: string): FocusPlacement {
    const existing = this.placements.get(placementId);
    if (existing === undefined) {
      throw new WorkbenchModelError(
        "relabelPlacement",
        `no placement '${placementId}' in the active generation`,
      );
    }
    const updated = Object.freeze({ ...existing, label_override: label });
    this.placements.set(placementId, updated);
    this.notifySubscribers();
    return updated;
  }

  /**
   * Destroy one placement occurrence. Canonical state and other placements of the
   * same FocusRef are unaffected (spec §FocusPlacement rules).
   */
  removePlacement(placementId: string): boolean {
    const removed = this.placements.delete(placementId);
    if (removed) {
      this.notifySubscribers();
    }
    return removed;
  }

  /**
   * Declare environment-observed capability states. The workbench only represents
   * what it observed; it never infers availability from installation (REQ-AFF-004).
   */
  declareCapabilities(observations: readonly CapabilityObservation[]): void {
    for (const observation of observations) {
      this.capabilities.set(observation.capability, observation);
    }
    this.capabilitiesVersion += 1;
    this.notifySubscribers();
  }

  /** Save a Horizon as a noncanonical preference (REQ-FOCUS-005). */
  saveHorizon(horizon: Horizon, savedName: string): SavedHorizonRecord {
    if (!isHorizon(horizon)) {
      throw new WorkbenchModelError("saveHorizon", "expected a Horizon");
    }
    return this.savedHorizons.save(horizon, savedName, Date.now());
  }

  loadSavedHorizon(savedName: string): SavedHorizonRecord | null {
    return this.savedHorizons.load(savedName);
  }

  getSavedHorizonNames(): readonly string[] {
    return this.savedHorizons.names();
  }

  /** Activate a saved horizon preference. Returns null when the name is unknown. */
  restoreSavedHorizon(savedName: string): Horizon | null {
    const record = this.savedHorizons.load(savedName);
    if (record === null) {
      return null;
    }
    this.selectHorizon(record.horizon);
    return record.horizon;
  }

  exportSavedHorizons(): string {
    return this.savedHorizons.toJSON();
  }

  importSavedHorizons(json: string): void {
    try {
      this.savedHorizons = SavedHorizonRegistry.fromJSON(json);
    } catch (error) {
      if (error instanceof MalformedSavedHorizonsError) {
        throw error;
      }
      throw new WorkbenchModelError("importSavedHorizons", String(error));
    }
  }

  /** Source-typed relation edges derived from the current authoritative projection. */
  getRelations(): readonly ReturnType<typeof deriveRelations>[number][] {
    return Object.freeze([...deriveRelations(this.worldSnapshot())]);
  }

  /** The current materialization plan (working set, subscriptions, queries, prefetch, candidates). */
  getMaterializationPlan(): ReturnType<typeof planMaterialization> {
    return planMaterialization(this.worldSnapshot(), this.currentHorizon);
  }

  /**
   * Build immutable FullWorkbenchSnapshot.
   *
   * T21 correction (measured HC4/HC9a bottleneck): the expensive presentation
   * inputs — the store read + ProjectionWorld, Case-wide attention, the frozen
   * Trace/Chat histories, resolved capability statuses, and the horizon-scoped
   * affordance ids — are cached under the SAME invalidation discipline as the
   * T20 plan cache: version counters bumped by exactly the mutations that change
   * each input, so identical store states assemble byte-identical snapshots
   * without re-reading SQLite or copying O(history) arrays per interaction.
   * Snapshot CONTENT is unchanged; only identical rebuilds are eliminated.
   *
   * T20 planner swap: the materialization plan comes from the fold
   * coordinator's last-accepted worker plan whenever one is valid for the
   * CURRENT (generation, horizon, world-version) context; otherwise it is
   * folded inline synchronously — the immediate fallback. The inline fallback
   * doubles as the refresh trigger: the first snapshot read on a stale context
   * schedules an asynchronous worker refresh, so interactions never wait on
   * the worker and the plan converges when the fold completes.
   */
  getSnapshot(): FullWorkbenchSnapshot {
    const horizonId = this.currentHorizon?.id ?? null;
    const { base: baseSnapshot, world, attention } = this.currentBase();
    const plan = this.currentPlan(world);
    const capabilityStatuses = this.currentCapabilityStatuses();
    const availableCapabilityAffordances = capabilityStatuses
      .filter((capability) => capability.status === "available")
      .map((capability) => `affordance:capability:${capability.capability}`);
    const affordanceIds = this.currentAffordanceIds(
      plan,
      baseSnapshot.affordance_ids,
      availableCapabilityAffordances,
    );

    const breadcrumb = [
      "Case",
      ...this.horizonStack.map((h) => h.root_focus.human_label ?? h.root_focus.source_id),
    ];

    return {
      ...baseSnapshot,
      horizon: horizonId,
      focus: this.currentFocus ?? this.storeFocusRef,
      trace_records: this.currentTraceRecords(),
      chat_messages: this.currentChatMessages(),
      chat_enabled: this.chatEnabled,
      affordance_ids: affordanceIds,
      placements: Object.freeze([...this.placements.values()]),
      active_horizon: this.currentHorizon,
      horizon_breadcrumb: Object.freeze(breadcrumb),
      materialization: Object.freeze({
        horizon_id: plan.horizon_id,
        working_set_size: plan.working_set.length,
        counters: plan.counters,
      }),
      attention,
      context_candidates: plan.context_candidates,
      capability_reasons: capabilityStatuses,
    };
  }

  /**
   * The store snapshot + world + Case-wide attention for the CURRENT world
   * version, re-read from SQLite only when the version (or active generation)
   * changed since the last assembly. The store's snapshot content is
   * horizon-independent — the horizon id only tags snapshot meta, which the
   * returned FullWorkbenchSnapshot overrides — so the base is keyed on
   * (worldVersion, generationId), the same invalidation sites as the plan cache.
   */
  private currentBase(): BaseSnapshotCache {
    const cached = this.baseCache;
    const generationId = this.store.getActiveGeneration()?.id ?? null;
    if (
      cached !== null &&
      cached.version === this.worldVersion &&
      cached.generationId === generationId
    ) {
      return cached;
    }
    const base = this.store.getSnapshot(this.currentHorizon?.id ?? null);
    const world = this.worldFromBase(base);
    const entry: BaseSnapshotCache = {
      version: this.worldVersion,
      generationId,
      base,
      world,
      attention: deriveAttention(world),
    };
    this.baseCache = entry;
    return entry;
  }

  /**
   * Immutable Trace history for the current trace version — a length-bounded
   * view over the append-only backing (COR-2: O(1) publication, no per-snapshot
   * history copy), rebuilt only on append/reset.
   */
  private currentTraceRecords(): readonly TraceRecord[] {
    const cached = this.traceCache;
    if (cached !== null && cached.version === this.traceVersion) {
      return cached.frozen;
    }
    const frozen = immutableHistoryView(this.traceRecords, this.traceRecords.length);
    this.traceCache = { version: this.traceVersion, frozen };
    return frozen;
  }

  /**
   * Immutable Chat history for the current chat version — a length-bounded
   * view over the append-only backing, rebuilt only on push/reset.
   */
  private currentChatMessages(): readonly ChatMessage[] {
    const cached = this.chatCache;
    if (cached !== null && cached.version === this.chatVersion) {
      return cached.frozen;
    }
    const frozen = immutableHistoryView(this.chatMessages, this.chatMessages.length);
    this.chatCache = { version: this.chatVersion, frozen };
    return frozen;
  }

  /** Resolved capability statuses for the current capabilities version. */
  private currentCapabilityStatuses(): readonly CapabilityStatus[] {
    const cached = this.capabilityCache;
    if (cached !== null && cached.version === this.capabilitiesVersion) {
      return cached.frozen;
    }
    const frozen = Object.freeze([...this.capabilities.values()].map(resolveCapabilityStatus));
    this.capabilityCache = { version: this.capabilitiesVersion, frozen };
    return frozen;
  }

  /**
   * Horizon-scoped affordance ids (REQ-FOCUS-003) for the current
   * (world version, plan, capabilities) context. The plan participates by
   * identity: a re-folded or re-admitted plan is a different object, so its
   * working set rescopes the affordances exactly once.
   */
  private currentAffordanceIds(
    plan: MaterializationPlan,
    baseAffordanceIds: readonly string[],
    availableCapabilityAffordances: readonly string[],
  ): readonly string[] {
    const cached = this.affordanceCache;
    if (
      cached !== null &&
      cached.worldVersion === this.worldVersion &&
      cached.plan === plan &&
      cached.capabilitiesVersion === this.capabilitiesVersion
    ) {
      return cached.frozen;
    }
    const workingUnitIds = new Set<string>();
    for (const focus of plan.working_set) {
      if (focus.kind === "WorkItem") {
        workingUnitIds.add(focus.source_id);
      }
    }
    const scopedUnitAffordances = baseAffordanceIds.filter((affordanceId) => {
      if (!affordanceId.startsWith("affordance-")) {
        return true;
      }
      return workingUnitIds.has(affordanceId.slice("affordance-".length));
    });
    const scoped = [...scopedUnitAffordances, ...availableCapabilityAffordances];
    const frozen = immutableHistoryView(scoped, scoped.length);
    this.affordanceCache = {
      worldVersion: this.worldVersion,
      plan,
      capabilitiesVersion: this.capabilitiesVersion,
      frozen,
    };
    return frozen;
  }

  /**
   * Get units from underlying projection.
   */
  getUnits(): readonly UnitProjection[] {
    return this.getSnapshot().units;
  }

  /**
   * T20 planner swap: the plan for this snapshot — the worker's last-accepted
   * plan when it is valid for the current context, else the inline fold.
   * Validity is exact: same active generation, same horizon, same world
   * version (any event/horizon change invalidates). A miss schedules the
   * asynchronous refresh and folds inline for THIS snapshot.
   */
  private currentPlan(world: ProjectionWorld): MaterializationPlan {
    const coordinator = this.foldCoordinator;
    if (coordinator !== null) {
      const cached = coordinator.acceptedPlan;
      const active = this.store.getActiveGeneration();
      const valid =
        cached !== null &&
        this.acceptedPlanVersion === this.planVersion &&
        active !== null &&
        active.status !== "closed" &&
        cached.horizon_id === (this.currentHorizon?.id ?? null);
      if (valid && cached !== null) {
        return cached;
      }
      this.scheduleFoldRefresh();
    }
    return planMaterialization(world, this.currentHorizon);
  }

  /**
   * Fire-and-forget asynchronous plan refresh. Never awaited by the render
   * path (the model must never block on the worker); the outcome handler in
   * the constructor re-publishes when the refreshed plan is admitted.
   */
  private scheduleFoldRefresh(): void {
    const coordinator = this.foldCoordinator;
    if (coordinator === null || this.refreshInFlight) {
      return;
    }
    const active = this.store.getActiveGeneration();
    if (active === null || active.status === "closed") {
      return;
    }
    this.refreshInFlight = true;
    this.dispatchedPlanVersion = this.planVersion;
    const world = this.worldSnapshot();
    const horizon = this.currentHorizon;
    void coordinator.submit(world, horizon, active.id).catch(() => {
      // Submit failure (e.g. world install refused) must not wedge the
      // scheduler; the inline fallback keeps serving fresh folds.
      this.refreshInFlight = false;
    });
  }

  /** The plan currently live for the render path (worker plan when current, else null). */
  get workerPlan(): MaterializationPlan | null {
    const coordinator = this.foldCoordinator;
    if (coordinator === null) {
      return null;
    }
    const cached = coordinator.acceptedPlan;
    const active = this.store.getActiveGeneration();
    return cached !== null &&
      this.acceptedPlanVersion === this.planVersion &&
      active !== null &&
      active.status !== "closed" &&
      cached.horizon_id === (this.currentHorizon?.id ?? null)
      ? cached
      : null;
  }

  /** Coordinator statistics (submitted/accepted/stale_discarded/error_discarded). */
  get foldStats(): Readonly<{
    submitted: number;
    accepted: number;
    stale_discarded: number;
    error_discarded: number;
  }> | null {
    return this.foldCoordinator?.stats ?? null;
  }

  /**
   * Get trace records directly.
   */
  getTrace(): readonly TraceRecord[] {
    return Object.freeze([...this.traceRecords]);
  }

  /**
   * Get chat messages directly.
   */
  getChat(): readonly ChatMessage[] {
    return Object.freeze([...this.chatMessages]);
  }

  private requireActiveGeneration(operation: string): ProjectionGeneration {
    const generation = this.store.getActiveGeneration();
    if (generation === null || generation.status === "closed") {
      throw new WorkbenchModelError(operation, "requires an active projection generation");
    }
    return generation;
  }

  private worldSnapshot(): ProjectionWorld {
    // T21 correction: reuse the cached base when current (same invalidation key
    // as getSnapshot). The fold-refresh path calls this right after snapshot
    // assembly, so this removes a second full store read per interaction/event
    // while still reading fresh state whenever the world version moved.
    return this.currentBase().world;
  }

  private worldFromBase(base: WorkbenchSnapshot): ProjectionWorld {
    return {
      generation: base.generation,
      units: base.units,
      evidence: base.evidence,
      sessions: base.sessions,
      source_cursors: base.source_cursors,
    };
  }

  /**
   * PEER CORRECTION MAJOR-2: attribute a focus_changed reference to the owning
   * projected row's source. Store primitive kinds map onto the semantic kinds,
   * and the referenced row must resolve to EXACTLY ONE row for attribution to
   * succeed; otherwise null is returned and the frame is refused by applyEvent.
   * A default source is never used — that minted a second identity (fork) for
   * objects owned by another source.
   */
  private attributeStoreFocus(kindRaw: unknown, idRaw: unknown): FocusRef | null {
    if (typeof kindRaw !== "string" || typeof idRaw !== "string" || idRaw.length === 0) {
      return null;
    }
    const kind =
      kindRaw === "unit"
        ? "WorkItem"
        : kindRaw === "session"
          ? "Agent"
          : kindRaw === "evidence"
            ? "Evidence"
            : kindRaw;
    const world = this.worldSnapshot();
    let owners: string[] = [];
    if (kind === "WorkItem") {
      owners = world.units.filter((unit) => unit.unit_id === idRaw).map((unit) => unit.source_id);
    } else if (kind === "Agent") {
      owners = world.sessions
        .filter((session) => session.session_id === idRaw)
        .map((session) => session.source_id);
    } else if (kind === "Evidence") {
      owners = world.evidence
        .filter((evidence) => evidence.evidence_id === idRaw)
        .map((evidence) => evidence.source_id);
    } else {
      // Kinds with no projected row source cannot be attributed here.
      return null;
    }
    if (owners.length !== 1) {
      return null;
    }
    const owner = owners[0];
    if (owner === undefined) {
      return null;
    }
    return FocusRef.for(owner, kind, idRaw);
  }

  /**
   * Mirror of SqliteProjectionStore.applyEvent's admission gates, used only so
   * focus_changed attribution runs exclusively for frames the store would apply;
   * all other rejections keep the store's own typed reasons.
   */
  private storeWouldApply(frame: AuthoritativeEventFrame): boolean {
    const active = this.store.getActiveGeneration();
    return active !== null && active.status !== "closed" && frame.generation_id === active.id;
  }

  private notifySubscribers(): void {
    if (this.listeners.size === 0) return;
    try {
      const snap = this.getSnapshot();
      for (const listener of this.listeners) {
        listener(snap);
      }
    } catch {
      // ignore if generation is closed/unready
    }
  }

  /**
   * Clean up resources.
   */
  close(): void {
    this.listeners.clear();
    this.foldUnsubscribe?.();
    this.foldUnsubscribe = null;
    this.foldCoordinator?.dispose();
    this.store.close();
  }
}
