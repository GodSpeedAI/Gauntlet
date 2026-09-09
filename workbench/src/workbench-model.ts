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
 */

/** Typed error carrying the failing WorkbenchModel operation (failure locus preserved). */
export class WorkbenchModelError extends Error {
  constructor(operation: string, detail: string) {
    super(`workbench model ${operation}: ${detail}`);
    this.name = "WorkbenchModelError";
  }
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

  // In-memory projection reducers (mirrored/derived from durable log)
  private traceRecords: TraceRecord[] = [];
  private chatMessages: ChatMessage[] = [];

  private listeners = new Set<SnapshotListener>();

  constructor(options?: WorkbenchModelOptions) {
    this.store = new SqliteProjectionStore(options?.dbPath ?? ":memory:");
    this.chatEnabled = options?.enableChat ?? true;
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
   */
  getSnapshot(): FullWorkbenchSnapshot {
    const horizonId = this.currentHorizon?.id ?? null;
    const baseSnapshot = this.store.getSnapshot(horizonId);
    const world = this.worldFromBase(baseSnapshot);
    const plan = planMaterialization(world, this.currentHorizon);
    const attention = deriveAttention(world);
    const capabilityStatuses = [...this.capabilities.values()].map(resolveCapabilityStatus);
    const availableCapabilityAffordances = capabilityStatuses
      .filter((capability) => capability.status === "available")
      .map((capability) => `affordance:capability:${capability.capability}`);

    // Hoisting constrains visible affordances to the horizon working set (REQ-FOCUS-003).
    const workingUnitIds = new Set(
      plan.working_set.filter((focus) => focus.kind === "WorkItem").map((focus) => focus.source_id),
    );
    const scopedUnitAffordances = baseSnapshot.affordance_ids.filter((affordanceId) => {
      if (!affordanceId.startsWith("affordance-")) {
        return true;
      }
      return workingUnitIds.has(affordanceId.slice("affordance-".length));
    });

    const breadcrumb = [
      "Case",
      ...this.horizonStack.map((h) => h.root_focus.human_label ?? h.root_focus.source_id),
    ];

    return {
      ...baseSnapshot,
      horizon: horizonId,
      focus: this.currentFocus ?? this.storeFocusRef,
      trace_records: Object.freeze([...this.traceRecords]),
      chat_messages: Object.freeze([...this.chatMessages]),
      chat_enabled: this.chatEnabled,
      affordance_ids: Object.freeze([...scopedUnitAffordances, ...availableCapabilityAffordances]),
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
      capability_reasons: Object.freeze(capabilityStatuses),
    };
  }

  /**
   * Get units from underlying projection.
   */
  getUnits(): readonly UnitProjection[] {
    return this.getSnapshot().units;
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
    return this.worldFromBase(this.store.getSnapshot(this.currentHorizon?.id ?? null));
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
    this.store.close();
  }
}
