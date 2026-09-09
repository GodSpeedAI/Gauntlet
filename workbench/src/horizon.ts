/**
 * Horizon entity, saved-horizon (noncanonical) persistence, and horizon-driven
 * progressive materialization planning.
 * Conforms to spec §core_domain_model entity Horizon, REQ-FOCUS-003, REQ-FOCUS-004,
 * REQ-FOCUS-005, REQ-GOAL-001, REQ-GOAL-006, REQ-OPS-004, REQ-UX-002, and plan T08.
 *
 * Core Invariants:
 *   - Hoisting narrows representation and context construction; it never deletes or
 *     rewrites canonical (source) state, and de-hoist restores the broader world
 *     without reidentifying objects (REQ-FOCUS-003).
 *   - Materialization planning prefers the horizon-local working set over eager
 *     whole-Case loading (REQ-FOCUS-004); every eligible context candidate set keeps
 *     mandatory Reference, governance, and evidence closure (REQ-OPS-004).
 *   - Saved Horizons are plain serializable preferences, persisted separately from the
 *     projection store; they are never independent domain truth (REQ-FOCUS-005).
 *   - Case-wide urgent attention is derived from the whole Case and is independent of
 *     the active Horizon (REQ-GOAL-001, REQ-UX-002).
 */

import { FocusRef, MalformedFocusRefError } from "./focus";
import type { UnitProjection } from "./projection-store";
import type { ProjectionWorld, RelationQuerySpec } from "./relations";
import {
  deriveRelations,
  focusForEvidence,
  focusForSession,
  focusForUnit,
  traverseRelations,
} from "./relations";

export interface Horizon {
  /** Stable identity for the active or saved horizon projection. */
  readonly id: string;
  /** Focus around which the horizon is constructed. */
  readonly root_focus: FocusRef;
  /** Declared relation traversals eligible for projection/context construction. */
  readonly relation_queries: readonly RelationQuerySpec[];
  /** Workbench generation for which this active horizon projection is valid. */
  readonly generation: string;
  readonly filters: Readonly<Record<string, string>> | undefined;
  readonly saved_name: string | undefined;
}

export interface CreateHorizonOptions {
  readonly root_focus: FocusRef;
  readonly generation: string;
  readonly relation_queries?: readonly RelationQuerySpec[] | undefined;
  readonly horizon_id?: string | undefined;
  readonly filters?: Record<string, string> | undefined;
  readonly saved_name?: string | undefined;
}

export function createHorizon(options: CreateHorizonOptions): Horizon {
  const id = options.horizon_id ?? `horizon:${options.generation}:${options.root_focus.id}`;
  return Object.freeze({
    id,
    root_focus: options.root_focus,
    relation_queries: Object.freeze([...(options.relation_queries ?? [])]),
    generation: options.generation,
    filters: options.filters === undefined ? undefined : Object.freeze({ ...options.filters }),
    saved_name: options.saved_name,
  });
}

/** Default declared traversal for an unqualified hoist: the focused unit's evidence closure. */
export const DEFAULT_HOIST_RELATION_QUERIES: readonly RelationQuerySpec[] = Object.freeze([
  { kind: "has_evidence", depth: 1, direction: "outbound" },
]);

/**
 * Structural guard for Horizon values arriving at the model boundary (peer
 * correction MINOR): a fabricated horizon is rejected at the API with a typed
 * error instead of crashing inside snapshot assembly, where notifySubscribers'
 * blanket catch would silently starve subscribers.
 */
export function isHorizon(value: unknown): value is Horizon {
  if (typeof value !== "object" || value === null) {
    return false;
  }
  const candidate = value as Record<string, unknown>;
  if (typeof candidate["id"] !== "string" || typeof candidate["generation"] !== "string") {
    return false;
  }
  if (!(candidate["root_focus"] instanceof FocusRef)) {
    return false;
  }
  if (!Array.isArray(candidate["relation_queries"])) {
    return false;
  }
  for (const query of candidate["relation_queries"]) {
    if (typeof query !== "object" || query === null) {
      return false;
    }
    const spec = query as Record<string, unknown>;
    if (typeof spec["kind"] !== "string" || typeof spec["depth"] !== "number") {
      return false;
    }
    if (
      spec["direction"] !== "outbound" &&
      spec["direction"] !== "inbound" &&
      spec["direction"] !== "both"
    ) {
      return false;
    }
  }
  return true;
}

/** Typed error for malformed saved-horizon payloads (model output is untrusted input). */
export class MalformedSavedHorizonsError extends Error {
  constructor(detail: string) {
    super(`malformed saved horizons payload: ${detail}`);
    this.name = "MalformedSavedHorizonsError";
  }
}

export interface SavedHorizonRecord {
  readonly saved_name: string;
  readonly horizon: Horizon;
  readonly saved_at_ms: number;
}

/**
 * Noncanonical persistence for saved Horizon preferences (REQ-FOCUS-005).
 * A plain, JSON-serializable registry deliberately separate from the
 * SqliteProjectionStore: restoring a saved horizon re-resolves FocusRefs through
 * the identity registry, so preferences can never mint forked identities or
 * become independent domain truth.
 */
export class SavedHorizonRegistry {
  private records = new Map<string, SavedHorizonRecord>();

  save(horizon: Horizon, savedName: string, savedAtMs: number): SavedHorizonRecord {
    const named = createHorizon({
      root_focus: horizon.root_focus,
      generation: horizon.generation,
      relation_queries: horizon.relation_queries,
      horizon_id: horizon.id,
      filters: horizon.filters === undefined ? undefined : { ...horizon.filters },
      saved_name: savedName,
    });
    const record: SavedHorizonRecord = Object.freeze({
      saved_name: savedName,
      horizon: named,
      saved_at_ms: savedAtMs,
    });
    this.records.set(savedName, record);
    return record;
  }

  load(savedName: string): SavedHorizonRecord | null {
    return this.records.get(savedName) ?? null;
  }

  names(): readonly string[] {
    return Object.freeze([...this.records.keys()].sort());
  }

  /** Plain serializable form, explicitly not a projection-store artifact. */
  toJSON(): string {
    return JSON.stringify({
      schema: "gauntlet.workbench.saved-horizons/1",
      records: [...this.records.values()],
    });
  }

  /** Validate an untrusted payload and rebuild the registry, re-resolving every FocusRef. */
  static fromJSON(json: string): SavedHorizonRegistry {
    let parsed: unknown;
    try {
      parsed = JSON.parse(json);
    } catch (error) {
      throw new MalformedSavedHorizonsError(`payload is not valid JSON: ${String(error)}`);
    }
    if (typeof parsed !== "object" || parsed === null) {
      throw new MalformedSavedHorizonsError("expected an object");
    }
    const doc = parsed as Record<string, unknown>;
    if (doc["schema"] !== "gauntlet.workbench.saved-horizons/1") {
      throw new MalformedSavedHorizonsError(`unsupported schema '${String(doc["schema"])}'`);
    }
    const rawRecords = doc["records"];
    if (!Array.isArray(rawRecords)) {
      throw new MalformedSavedHorizonsError("records must be an array");
    }
    const registry = new SavedHorizonRegistry();
    for (const raw of rawRecords) {
      if (typeof raw !== "object" || raw === null) {
        throw new MalformedSavedHorizonsError("each record must be an object");
      }
      const record = raw as Record<string, unknown>;
      const savedName = record["saved_name"];
      const savedAtMs = record["saved_at_ms"];
      if (typeof savedName !== "string" || typeof savedAtMs !== "number") {
        throw new MalformedSavedHorizonsError(
          "each record requires string saved_name and number saved_at_ms",
        );
      }
      registry.save(horizonFromJSON(record["horizon"]), savedName, savedAtMs);
    }
    return registry;
  }
}

export function horizonToJSON(horizon: Horizon): string {
  return JSON.stringify(horizon);
}

export function horizonFromJSON(value: unknown): Horizon {
  if (typeof value !== "object" || value === null) {
    throw new MalformedSavedHorizonsError("horizon must be an object");
  }
  const h = value as Record<string, unknown>;
  const id = h["id"];
  const generation = h["generation"];
  if (typeof id !== "string" || typeof generation !== "string") {
    throw new MalformedSavedHorizonsError("horizon requires string id and generation");
  }
  let root: FocusRef;
  try {
    root = FocusRef.fromJSON(h["root_focus"]);
  } catch (error) {
    if (error instanceof MalformedFocusRefError) {
      throw new MalformedSavedHorizonsError(error.message);
    }
    throw error;
  }
  const rawQueries = h["relation_queries"];
  if (!Array.isArray(rawQueries)) {
    throw new MalformedSavedHorizonsError("relation_queries must be an array");
  }
  const queries: RelationQuerySpec[] = rawQueries.map((rawQuery) => {
    if (typeof rawQuery !== "object" || rawQuery === null) {
      throw new MalformedSavedHorizonsError("each relation query must be an object");
    }
    const spec = rawQuery as Record<string, unknown>;
    const kind = spec["kind"];
    const depth = spec["depth"];
    const direction = spec["direction"];
    if (
      typeof kind !== "string" ||
      typeof depth !== "number" ||
      (direction !== "outbound" && direction !== "inbound" && direction !== "both")
    ) {
      throw new MalformedSavedHorizonsError(
        "each relation query requires kind:string, depth:number, direction:outbound|inbound|both",
      );
    }
    return { kind, depth, direction };
  });
  const rawFilters = h["filters"];
  let filters: Record<string, string> | undefined;
  if (rawFilters !== undefined && rawFilters !== null) {
    if (typeof rawFilters !== "object") {
      throw new MalformedSavedHorizonsError("filters must be an object");
    }
    filters = {};
    for (const [key, val] of Object.entries(rawFilters)) {
      filters[key] = String(val);
    }
  }
  return createHorizon({
    root_focus: root,
    generation,
    relation_queries: queries,
    horizon_id: id,
    filters,
  });
}

// --- Progressive materialization planning ------------------------------------

export type ContextCandidateRole =
  | "relation"
  | "mandatory_reference"
  | "mandatory_governance"
  | "mandatory_evidence";

export interface ContextCandidate {
  readonly focus: FocusRef;
  readonly role: ContextCandidateRole;
  readonly detail: string;
  readonly mandatory: boolean;
}

export interface AttentionEntry {
  readonly focus: FocusRef;
  readonly severity: "urgent" | "warning";
  readonly reason: string;
  /** Always true: Case-wide attention never depends on the active Horizon. */
  readonly horizon_independent: boolean;
  readonly detected_at_ms: number;
}

export interface PlannedQuery {
  readonly kind: "case_scan" | "focus_load" | "relation_traversal";
  readonly scope: string;
}

export interface MaterializationCounters {
  readonly subscriptions: number;
  readonly queries: number;
  readonly prefetches: number;
  readonly context_candidates: number;
}

export interface MaterializationPlan {
  /** Active horizon identity; null plans the whole-Case (unhoisted) world. */
  readonly horizon_id: string | null;
  readonly working_set: readonly FocusRef[];
  /** Focus-scoped event subscription keys in the working set. */
  readonly subscriptions: readonly string[];
  readonly queries: readonly PlannedQuery[];
  /** Record prefetch keys warmed for the working set. */
  readonly prefetch_keys: readonly string[];
  readonly context_candidates: readonly ContextCandidate[];
  readonly counters: MaterializationCounters;
}

/**
 * Case-wide urgent attention, derived from the whole Case regardless of any
 * active Horizon (REQ-GOAL-001, REQ-UX-002). Blocked Units are escalation-shaped
 * attention items with owner, target, freshness, and a resolution path.
 */
export function deriveAttention(world: ProjectionWorld): readonly AttentionEntry[] {
  const entries = world.units
    .filter((unit) => unit.status === "blocked")
    .map((unit) =>
      Object.freeze({
        focus: focusForUnit(unit),
        severity: "urgent" as const,
        reason: "unit_blocked",
        horizon_independent: true,
        detected_at_ms: unit.updated_at,
      }),
    );
  return Object.freeze(entries);
}

function dedupeById(refs: readonly FocusRef[]): FocusRef[] {
  const byId = new Map<string, FocusRef>();
  for (const ref of refs) {
    if (!byId.has(ref.id)) {
      byId.set(ref.id, ref);
    }
  }
  return [...byId.values()];
}

function dedupeCandidates(candidates: readonly ContextCandidate[]): readonly ContextCandidate[] {
  const byFocus = new Map<string, ContextCandidate>();
  for (const candidate of candidates) {
    if (!byFocus.has(candidate.focus.id)) {
      byFocus.set(candidate.focus.id, candidate);
    }
  }
  return [...byFocus.values()].sort((a, b) => a.focus.id.localeCompare(b.focus.id));
}

/** Mandatory Reference closure: one candidate per authoritative source at its cursor. */
function referenceCandidates(world: ProjectionWorld): ContextCandidate[] {
  const out: ContextCandidate[] = [];
  for (const source of Object.keys(world.source_cursors).sort()) {
    const cursor = world.source_cursors[source] ?? "unknown";
    out.push({
      focus: FocusRef.for(source, "Reference", `generation:${world.generation}`),
      role: "mandatory_reference",
      detail: `authoritative source '${source}' at cursor '${cursor}' (generation ${world.generation})`,
      mandatory: true,
    });
  }
  return out;
}

/** Mandatory governance closure: distinct authority gates over the scoped units. */
function governanceCandidates(units: readonly UnitProjection[]): ContextCandidate[] {
  const seen = new Set<string>();
  const out: ContextCandidate[] = [];
  for (const unit of units) {
    const key = `${unit.source_id}:${unit.authority}`;
    if (seen.has(key)) {
      continue;
    }
    seen.add(key);
    out.push({
      focus: FocusRef.for(unit.source_id, "Governance", `authority:${unit.authority}`),
      role: "mandatory_governance",
      detail: `authority gate '${unit.authority}' governs '${unit.blast_radius}' blast radius (unit ${unit.unit_id})`,
      mandatory: true,
    });
  }
  return out.sort((a, b) => a.focus.id.localeCompare(b.focus.id));
}

function wholeCasePlan(world: ProjectionWorld): MaterializationPlan {
  const caseFoci = dedupeById([
    ...world.units.map(focusForUnit),
    ...world.evidence.map(focusForEvidence),
    ...world.sessions.map(focusForSession),
  ]);
  const relationCandidates: ContextCandidate[] = [];
  for (const unit of world.units) {
    relationCandidates.push({
      focus: focusForUnit(unit),
      role: "relation",
      detail: "case-wide working set",
      mandatory: false,
    });
  }
  for (const session of world.sessions) {
    relationCandidates.push({
      focus: focusForSession(session),
      role: "relation",
      detail: "case-wide working set",
      mandatory: false,
    });
  }
  for (const evidence of world.evidence) {
    relationCandidates.push({
      focus: focusForEvidence(evidence),
      role: "mandatory_evidence",
      detail: `evidence dependency of unit ${evidence.unit_id}`,
      mandatory: true,
    });
  }
  const candidates = dedupeCandidates([
    ...relationCandidates,
    ...referenceCandidates(world),
    ...governanceCandidates(world.units),
  ]);
  const subscriptions = caseFoci.map((focus) => `sub:${focus.id}`);
  const prefetchKeys = caseFoci.map((focus) => `prefetch:${focus.id}`);
  const queries: PlannedQuery[] = [
    { kind: "case_scan", scope: "units" },
    { kind: "case_scan", scope: "evidence" },
    { kind: "case_scan", scope: "sessions" },
  ];
  return Object.freeze({
    horizon_id: null,
    working_set: Object.freeze(caseFoci),
    subscriptions: Object.freeze(subscriptions),
    queries: Object.freeze(queries),
    prefetch_keys: Object.freeze(prefetchKeys),
    context_candidates: Object.freeze(candidates),
    counters: Object.freeze({
      subscriptions: subscriptions.length,
      queries: queries.length,
      prefetches: prefetchKeys.length,
      context_candidates: candidates.length,
    }),
  });
}

function hoistedPlan(world: ProjectionWorld, horizon: Horizon): MaterializationPlan {
  const relations = deriveRelations(world);
  const traversal = traverseRelations(relations, horizon.root_focus, horizon.relation_queries);

  // Mandatory evidence closure for a WorkItem root: hoisting never removes it,
  // even when no relation query declares it (REQ-OPS-004).
  const mandatoryEvidence =
    horizon.root_focus.kind === "WorkItem"
      ? world.evidence.filter(
          (evidence) =>
            evidence.source_id === horizon.root_focus.source &&
            evidence.unit_id === horizon.root_focus.source_id,
        )
      : [];
  const mandatoryEvidenceCandidates: ContextCandidate[] = mandatoryEvidence.map((evidence) => ({
    focus: focusForEvidence(evidence),
    role: "mandatory_evidence",
    detail: `mandatory evidence dependency of focused unit ${horizon.root_focus.source_id}`,
    mandatory: true,
  }));

  const mandatoryIds = new Set(mandatoryEvidenceCandidates.map((c) => c.focus.id));
  const workingSet = dedupeById([
    ...traversal.reached,
    ...mandatoryEvidenceCandidates.map((c) => c.focus),
  ]);

  const scopedUnits = world.units.filter((unit) =>
    workingSet.some(
      (focus) =>
        focus.kind === "WorkItem" &&
        focus.source === unit.source_id &&
        focus.source_id === unit.unit_id,
    ),
  );
  const governanceScope = scopedUnits.length > 0 ? scopedUnits : world.units;

  const relationCandidates: ContextCandidate[] = workingSet
    .filter((focus) => !mandatoryIds.has(focus.id))
    .map((focus) => ({
      focus,
      role: "relation" as const,
      detail:
        focus.id === horizon.root_focus.id
          ? "horizon root focus"
          : "reached via declared relation traversal",
      mandatory: false,
    }));

  const candidates = dedupeCandidates([
    ...mandatoryEvidenceCandidates,
    ...relationCandidates,
    ...referenceCandidates(world),
    ...governanceCandidates(governanceScope),
  ]);

  const subscriptions = workingSet.map((focus) => `sub:${focus.id}`);
  const prefetchKeys = workingSet.map((focus) => `prefetch:${focus.id}`);
  const queries: PlannedQuery[] = [
    { kind: "focus_load", scope: horizon.root_focus.id },
    ...horizon.relation_queries.map((query) => ({
      kind: "relation_traversal" as const,
      scope: `${query.kind}:${query.depth}:${query.direction}`,
    })),
  ];
  return Object.freeze({
    horizon_id: horizon.id,
    working_set: Object.freeze(workingSet),
    subscriptions: Object.freeze(subscriptions),
    queries: Object.freeze(queries),
    prefetch_keys: Object.freeze(prefetchKeys),
    context_candidates: Object.freeze(candidates),
    counters: Object.freeze({
      subscriptions: subscriptions.length,
      queries: queries.length,
      prefetches: prefetchKeys.length,
      context_candidates: candidates.length,
    }),
  });
}

/**
 * Plan the working world for the active horizon: which focus-scoped event
 * subscriptions, scoped queries, record prefetches, and context candidates are in
 * the working set. A null horizon plans the whole-Case (unhoisted, eager) world.
 * Counters are derived from real projection rows so tests can compare narrowed
 * versus whole-Case behavior (REQ-FOCUS-004, REQ-GOAL-006).
 */
export function planMaterialization(
  world: ProjectionWorld,
  horizon: Horizon | null,
): MaterializationPlan {
  return horizon === null ? wholeCasePlan(world) : hoistedPlan(world, horizon);
}
