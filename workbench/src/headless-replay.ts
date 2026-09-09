import { createHash } from "node:crypto";
import type { KernelPersistenceClass } from "./evidence-promotion";
import { deriveAttention } from "./horizon";
import {
  type AuthoritativeEventFrame,
  type EvidenceProjection,
  type SessionProjection,
  SqliteProjectionStore,
  type UnitProjection,
  type WorkbenchSnapshot,
} from "./projection-store";
import type { TraceRecord } from "./workbench-model";

/**
 * Headless replay/reconstruction harness (plan T19, REQ-VERIFY-002,
 * REQ-STATE-004, REQ-GOAL-005, REQ-TUI-005, REQ-SAFE-001, DSN-011).
 *
 * Core invariants:
 *   - Reconstruction consumes authoritative sources DIRECTLY: the recorded
 *     authoritative event log, folded either through T06's
 *     `rebuildFromAuthoritativeLog` store path or through a pure log fold in
 *     this module. No OpenTUI/React/presentation module is imported, and no
 *     presentation state can enter the normalized projection (Attack 1,
 *     DSN-011: no renderer holds canonical truth).
 *   - Deleting renderer, kernel, or local SQLite state never changes the
 *     normalized Case/Run/Evidence/Attention/settlement truth; the order of
 *     removal is irrelevant (REQ-SAFE-001, REQ-VERIFY-002).
 *   - Kernel values are computation state (T05/T11): only an explicit durable
 *     artifact locator survives kernel death. Ephemeral and pinned values are
 *     marked UNAVAILABLE with a typed reason after kernel loss — never
 *     recreated, never faked (REQ-PRIME-006, REQ-SYNC-003, REQ-SAFE-004).
 *   - The normalized projection is a plain JSON value with deterministic
 *   ordering and a canonical-JSON sha256 digest; two reconstructions of the
 *     same recorded Run are byte-equivalent in consequential content.
 *
 * BUN-SQLITE INTERPRETATION (pinned, plan T19 "changes:" wording):
 *   REQ-STATE-004's "reconstructible headlessly without OpenTUI, React, Bun
 *   SQLite, or an active notebook view" forbids depending on the WORKBENCH's
 *   local SQLite read-model (or any UI over it) as a truth source — NONGOAL-001
 *   explicitly keeps the Bun SQLite database non-canonical. It cannot forbid
 *   the `bun:sqlite` module inside T06's store: that store is the project's
 *   canonical rebuildable read-model adapter and the plan points T19 at the
 *   "rebuildFromAuthoritativeLog path" (plan T19 steps, hot_context "projection
 *   rebuild T06"). Accordingly this harness proves the stronger form: the
 *   normalized projection is derivable from the bare event log ALONE (the pure
 *   fold below, zero SQLite), AND from a cold `rebuildFromAuthoritativeLog`
 *   into a fresh (possibly deleted-and-recreated) SQLite file — and both folds
 *   agree. Pre-existing local SQLite state is never an input to truth.
 */

/** Schema tag of the frozen recorded-Run document. */
export const RECORDED_RUN_SCHEMA = "gauntlet.workbench.recorded-run/1";

/** Schema tag of the normalized headless projection. */
export const NORMALIZED_PROJECTION_SCHEMA = "gauntlet.workbench.headless-projection/1";

/** One kernel object as recorded at recording time (durable facts only). */
export interface RecordedKernelObject {
  readonly object_id: string;
  readonly name: string;
  readonly type_name: string;
  readonly session_ref: string;
  readonly kernel_generation: string;
  readonly persistence_class: KernelPersistenceClass;
  /** Durable locator when artifactized; null otherwise (never invented). */
  readonly artifact_ref: string | null;
}

/** Presentation-side ephemera captured at recording time. NEVER canonical. */
export interface RecordedPresentationState {
  /** Rendered occurrences (placement ids and placement-local labels). */
  readonly placements: readonly {
    readonly id: string;
    readonly surface: string;
    readonly view: string;
    readonly label_override: string | undefined;
  }[];
  /** The operator's current visual selection, if any (never canonical). */
  readonly selected_focus_id: string | null;
}

/**
 * One recorded Run: the authoritative event log plus the durable kernel-side
 * facts and (non-authoritative) presentation ephemera observed while recording.
 */
export interface RecordedRun {
  readonly schema: typeof RECORDED_RUN_SCHEMA;
  readonly run_id: string;
  readonly generation_id: string;
  /** Recording-time clock (a recorded constant, never re-minted on replay). */
  readonly recorded_at_ms: number;
  readonly events: readonly AuthoritativeEventFrame[];
  readonly kernel: {
    readonly kernel_generation: string;
    readonly session_ref: string;
    readonly objects: readonly RecordedKernelObject[];
  };
  readonly presentation: RecordedPresentationState;
}

/** Freeze-and-clone a recorded Run so later mutation cannot rewrite history. */
export function recordRun(init: {
  run_id: string;
  generation_id: string;
  recorded_at_ms: number;
  events: readonly AuthoritativeEventFrame[];
  kernel: {
    kernel_generation: string;
    session_ref: string;
    objects: readonly RecordedKernelObject[];
  };
  presentation: RecordedPresentationState;
}): RecordedRun {
  return Object.freeze({
    schema: RECORDED_RUN_SCHEMA,
    run_id: init.run_id,
    generation_id: init.generation_id,
    recorded_at_ms: init.recorded_at_ms,
    events: Object.freeze(init.events.map((event) => Object.freeze({ ...event }))),
    kernel: Object.freeze({
      kernel_generation: init.kernel.kernel_generation,
      session_ref: init.kernel.session_ref,
      objects: Object.freeze(init.kernel.objects.map((object) => Object.freeze({ ...object }))),
    }),
    presentation: Object.freeze({
      placements: Object.freeze(
        init.presentation.placements.map((placement) => Object.freeze({ ...placement })),
      ),
      selected_focus_id: init.presentation.selected_focus_id,
    }),
  });
}

// --- Normalized projection -------------------------------------------------------

export interface NormalizedUnitRecord {
  readonly unit_id: string;
  readonly title: string;
  readonly phase: string;
  readonly status: string;
  readonly authority: string;
  readonly blast_radius: string;
  readonly source_id: string;
  readonly source_cursor: string;
  readonly updated_at: number;
}

export interface NormalizedEvidenceRecord {
  readonly evidence_id: string;
  readonly unit_id: string;
  readonly kind: string;
  readonly locator: string;
  readonly verified: boolean;
  readonly source_id: string;
  readonly source_cursor: string;
  readonly updated_at: number;
}

export interface NormalizedSessionRecord {
  readonly session_id: string;
  readonly parent_id: string | null;
  readonly model: string;
  readonly status: string;
  readonly source_id: string;
  readonly source_cursor: string;
}

export interface NormalizedAttentionRecord {
  readonly focus_id: string;
  readonly focus_kind: string;
  readonly reason: string;
  readonly severity: string;
  readonly horizon_independent: boolean;
}

/**
 * The settlement-INPUT projection for one unit: the canonical answer bits and
 * authority fields settlement would consult. This is NOT a settlement claim —
 * settlement authority lives in Gauntlet alone (NONGOAL-001, T11 admission
 * rules); the workbench can only project its inputs honestly.
 */
export interface NormalizedSettlementInput {
  readonly unit_id: string;
  readonly status: string;
  readonly authority: string;
  readonly blast_radius: string;
  readonly verified_evidence_ids: readonly string[];
  readonly unverified_evidence_ids: readonly string[];
}

/** Availability of one kernel object after a given reconstruction. */
export interface NormalizedKernelRecord {
  readonly object_id: string;
  readonly name: string;
  readonly type_name: string;
  readonly persistence_class: KernelPersistenceClass;
  /** Durable locator when artifactized; always null otherwise (never invented). */
  readonly artifact_ref: string | null;
  readonly value_available: boolean;
  /** Typed reason when the value is unavailable; null when available. */
  readonly unavailability_reason: string | null;
}

/** Audit/run truth: one entry per authoritative event, in log order. */
export interface NormalizedTraceRecord {
  readonly trace_id: string;
  readonly unit_id: string;
  readonly event_type: string;
  readonly source_id: string;
  readonly cursor: string;
  readonly timestamp_ms: number;
}

/**
 * The normalized Case/Run/Evidence/Attention/settlement projection. Every
 * array is deterministically ordered; the whole value is plain JSON.
 */
export interface NormalizedRunProjection {
  readonly schema: typeof NORMALIZED_PROJECTION_SCHEMA;
  readonly run_id: string;
  readonly generation_id: string;
  readonly source_cursors: readonly (readonly [string, string])[];
  /** Authoritative focus (from focus_changed frames), store-kind mapping applied. */
  readonly focus: { readonly kind: string; readonly id: string } | null;
  readonly units: readonly NormalizedUnitRecord[];
  readonly evidence: readonly NormalizedEvidenceRecord[];
  readonly sessions: readonly NormalizedSessionRecord[];
  readonly attention: readonly NormalizedAttentionRecord[];
  readonly settlement_inputs: readonly NormalizedSettlementInput[];
  readonly trace: readonly NormalizedTraceRecord[];
  readonly kernel_objects: readonly NormalizedKernelRecord[];
  /** Always false: presentation state is never part of the truth projection. */
  readonly presentation_present: false;
}

/**
 * The canonical (consequential) sections of a normalized projection: exactly
 * the truth that must survive renderer/kernel/store loss (Reference cursors,
 * authority-bearing unit rows, evidence, settlement inputs, run/audit truth).
 * Kernel availability is deliberately excluded — it legitimately differs when
 * the kernel dies (and only then).
 */
export interface CanonicalSections {
  readonly run_id: string;
  readonly generation_id: string;
  readonly source_cursors: readonly (readonly [string, string])[];
  readonly focus: { readonly kind: string; readonly id: string } | null;
  readonly units: readonly NormalizedUnitRecord[];
  readonly evidence: readonly NormalizedEvidenceRecord[];
  readonly sessions: readonly NormalizedSessionRecord[];
  readonly attention: readonly NormalizedAttentionRecord[];
  readonly settlement_inputs: readonly NormalizedSettlementInput[];
  readonly trace: readonly NormalizedTraceRecord[];
}

/** Project the canonical sections out of a full normalized projection. */
export function canonicalSections(projection: NormalizedRunProjection): CanonicalSections {
  return {
    run_id: projection.run_id,
    generation_id: projection.generation_id,
    source_cursors: projection.source_cursors,
    focus: projection.focus,
    units: projection.units,
    evidence: projection.evidence,
    sessions: projection.sessions,
    attention: projection.attention,
    settlement_inputs: projection.settlement_inputs,
    trace: projection.trace,
  };
}

/**
 * Store-primitive focus kind → semantic kind, the SAME mapping
 * `WorkbenchModel.attributeStoreFocus` documents (unit→WorkItem,
 * session→Agent, evidence→Evidence); unknown kinds pass through.
 */
function semanticFocusKind(kind: string): string {
  if (kind === "unit") return "WorkItem";
  if (kind === "session") return "Agent";
  if (kind === "evidence") return "Evidence";
  return kind;
}

function kernelRecords(
  objects: readonly RecordedKernelObject[],
  kernelAlive: boolean,
): NormalizedKernelRecord[] {
  return [...objects]
    .sort((a, b) => a.object_id.localeCompare(b.object_id))
    .map((object) => {
      if (object.persistence_class === "artifactized" && object.artifact_ref !== null) {
        return {
          object_id: object.object_id,
          name: object.name,
          type_name: object.type_name,
          persistence_class: object.persistence_class,
          artifact_ref: object.artifact_ref,
          value_available: true,
          unavailability_reason: null,
        };
      }
      // Only an explicit durable locator survives kernel death. Pinned keeps a
      // value alive INSIDE the living kernel (T05 terminate() clears objects);
      // after kernel loss a pinned value is gone as surely as an ephemeral one.
      const reason =
        object.persistence_class === "pinned"
          ? kernelAlive
            ? null
            : "kernel_restarted_pinned_value_not_retained"
          : kernelAlive
            ? null
            : "ephemeral_kernel_state_not_reconstructible";
      return {
        object_id: object.object_id,
        name: object.name,
        type_name: object.type_name,
        persistence_class: object.persistence_class,
        artifact_ref: null,
        value_available: kernelAlive,
        unavailability_reason: reason,
      };
    });
}

function normalizedTrace(events: readonly AuthoritativeEventFrame[]): NormalizedTraceRecord[] {
  return events.map((frame) => ({
    trace_id: `trace-${frame.source_id}-${frame.cursor}`,
    unit_id: String(frame.payload["unit_id"] ?? "unknown"),
    event_type: frame.event_type,
    source_id: frame.source_id,
    cursor: frame.cursor,
    timestamp_ms: frame.timestamp_ms,
  }));
}

/**
 * Pure fold of the recorded event log to the normalized projection — the
 * strongest "without Bun SQLite" form: no SQLite, no store, no UI, only the
 * authoritative frames. Row semantics mirror the store's own applyEvent
 * defaults exactly (last frame per identity wins; the store's documented
 * fallbacks for absent payload fields).
 */
export function foldLogToNormalized(
  recorded: RecordedRun,
  options: { kernelAlive?: boolean } = {},
): NormalizedRunProjection {
  const kernelAlive = options.kernelAlive ?? false;

  const units = new Map<string, UnitProjection>();
  const evidence = new Map<string, EvidenceProjection>();
  const sessions = new Map<string, SessionProjection>();
  const cursors = new Map<string, string>();
  let focus: { kind: string; id: string } | null = null;

  for (const frame of recorded.events) {
    cursors.set(frame.source_id, frame.cursor);
    const payload = frame.payload;
    switch (frame.event_type) {
      case "unit_updated": {
        const unitId = String(payload["unit_id"]);
        units.set(unitId, {
          unit_id: unitId,
          generation_id: frame.generation_id,
          source_id: frame.source_id,
          source_cursor: frame.cursor,
          title: String(payload["title"] ?? "Unit"),
          phase: String(payload["phase"] ?? "phase1"),
          status: String(payload["status"] ?? "pending"),
          authority: String(payload["authority"] ?? "default"),
          blast_radius: String(payload["blast_radius"] ?? "isolated"),
          updated_at: frame.timestamp_ms,
        });
        break;
      }
      case "evidence_added": {
        const evidenceId = String(payload["evidence_id"]);
        evidence.set(evidenceId, {
          evidence_id: evidenceId,
          generation_id: frame.generation_id,
          source_id: frame.source_id,
          source_cursor: frame.cursor,
          unit_id: String(payload["unit_id"]),
          kind: String(payload["kind"] ?? "report"),
          locator: String(payload["locator"] ?? ""),
          verified: Boolean(payload["verified"]),
          updated_at: frame.timestamp_ms,
        });
        break;
      }
      case "session_updated": {
        const sessionId = String(payload["session_id"]);
        sessions.set(sessionId, {
          session_id: sessionId,
          generation_id: frame.generation_id,
          source_id: frame.source_id,
          source_cursor: frame.cursor,
          parent_id: payload["parent_id"] ? String(payload["parent_id"]) : null,
          model: String(payload["model"] ?? "default-model"),
          status: String(payload["status"] ?? "running"),
          updated_at: frame.timestamp_ms,
        });
        break;
      }
      case "focus_changed": {
        focus = { kind: String(payload["kind"]), id: String(payload["id"]) };
        break;
      }
    }
  }

  return assembleProjection({
    run_id: recorded.run_id,
    generation_id: recorded.generation_id,
    world: {
      generation: recorded.generation_id,
      units: [...units.values()].sort((a, b) => a.unit_id.localeCompare(b.unit_id)),
      evidence: [...evidence.values()].sort((a, b) => a.evidence_id.localeCompare(b.evidence_id)),
      sessions: [...sessions.values()].sort((a, b) => a.session_id.localeCompare(b.session_id)),
      source_cursors: Object.fromEntries(
        [...cursors.entries()].sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0)),
      ),
    },
    focus: focus === null ? null : { kind: semanticFocusKind(focus.kind), id: focus.id },
    trace: normalizedTrace(recorded.events),
    kernel: kernelRecords(recorded.kernel.objects, kernelAlive),
  });
}

/** Assemble the final projection from rows plus derived attention. */
function assembleProjection(args: {
  run_id: string;
  generation_id: string;
  world: {
    generation: string;
    units: readonly UnitProjection[];
    evidence: readonly EvidenceProjection[];
    sessions: readonly SessionProjection[];
    source_cursors: Record<string, string>;
  };
  focus: { kind: string; id: string } | null;
  trace: readonly NormalizedTraceRecord[];
  kernel: readonly NormalizedKernelRecord[];
}): NormalizedRunProjection {
  const { world } = args;
  const attention = deriveAttention(world).map((entry) => ({
    focus_id: entry.focus.id,
    focus_kind: entry.focus.kind,
    reason: entry.reason,
    severity: entry.severity,
    horizon_independent: entry.horizon_independent,
  }));
  attention.sort((a, b) => a.focus_id.localeCompare(b.focus_id));

  const verifiedByUnit = new Map<string, string[]>();
  const unverifiedByUnit = new Map<string, string[]>();
  for (const record of world.evidence) {
    const target = record.verified ? verifiedByUnit : unverifiedByUnit;
    const list = target.get(record.unit_id) ?? [];
    list.push(record.evidence_id);
    target.set(record.unit_id, list);
  }
  for (const list of verifiedByUnit.values()) list.sort((a, b) => a.localeCompare(b));
  for (const list of unverifiedByUnit.values()) list.sort((a, b) => a.localeCompare(b));

  const settlementInputs = [...world.units]
    .sort((a, b) => a.unit_id.localeCompare(b.unit_id))
    .map((unit) => ({
      unit_id: unit.unit_id,
      status: unit.status,
      authority: unit.authority,
      blast_radius: unit.blast_radius,
      verified_evidence_ids: [...(verifiedByUnit.get(unit.unit_id) ?? [])],
      unverified_evidence_ids: [...(unverifiedByUnit.get(unit.unit_id) ?? [])],
    }));

  return {
    schema: NORMALIZED_PROJECTION_SCHEMA,
    run_id: args.run_id,
    generation_id: args.generation_id,
    source_cursors: Object.entries(world.source_cursors)
      .sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0))
      .map(([source, cursor]) => [source, cursor] as const),
    focus: args.focus,
    units: [...world.units]
      .sort((a, b) => a.unit_id.localeCompare(b.unit_id))
      .map((unit) => ({
        unit_id: unit.unit_id,
        title: unit.title,
        phase: unit.phase,
        status: unit.status,
        authority: unit.authority,
        blast_radius: unit.blast_radius,
        source_id: unit.source_id,
        source_cursor: unit.source_cursor,
        updated_at: unit.updated_at,
      })),
    evidence: [...world.evidence]
      .sort((a, b) => a.evidence_id.localeCompare(b.evidence_id))
      .map((record) => ({
        evidence_id: record.evidence_id,
        unit_id: record.unit_id,
        kind: record.kind,
        locator: record.locator,
        verified: record.verified,
        source_id: record.source_id,
        source_cursor: record.source_cursor,
        updated_at: record.updated_at,
      })),
    sessions: [...world.sessions]
      .sort((a, b) => a.session_id.localeCompare(b.session_id))
      .map((session) => ({
        session_id: session.session_id,
        parent_id: session.parent_id,
        model: session.model,
        status: session.status,
        source_id: session.source_id,
        source_cursor: session.source_cursor,
      })),
    attention,
    settlement_inputs: settlementInputs,
    trace: args.trace,
    kernel_objects: args.kernel,
    presentation_present: false,
  };
}

/**
 * Normalize a T06 store snapshot (the with-store fold) into the projection.
 * Takes the BASE WorkbenchSnapshot — the store's own projection — never a
 * presentation snapshot: operator selection, placements, and horizons cannot
 * enter by construction.
 */
export function normalizeSnapshot(
  snapshot: WorkbenchSnapshot,
  context: {
    run_id: string;
    events: readonly AuthoritativeEventFrame[];
    kernelObjects: readonly RecordedKernelObject[];
    kernelAlive: boolean;
  },
): NormalizedRunProjection {
  return assembleProjection({
    run_id: context.run_id,
    generation_id: snapshot.generation,
    world: {
      generation: snapshot.generation,
      units: snapshot.units,
      evidence: snapshot.evidence,
      sessions: snapshot.sessions,
      source_cursors: snapshot.source_cursors,
    },
    focus:
      snapshot.focus === null
        ? null
        : { kind: semanticFocusKind(snapshot.focus.kind), id: snapshot.focus.id },
    trace: normalizedTrace(context.events),
    kernel: kernelRecords(context.kernelObjects, context.kernelAlive),
  });
}

/** One headless reconstruction: both fold paths plus their equality facts. */
export interface HeadlessReplayResult {
  /** Pure log fold — zero SQLite, zero store, zero UI. */
  readonly via_log: NormalizedRunProjection;
  /** T06 cold rebuild fold (`rebuildFromAuthoritativeLog`). */
  readonly via_store: NormalizedRunProjection;
  /** The store produced by the cold rebuild (disposable; delete at will). */
  readonly store: SqliteProjectionStore;
  /** Full-projection equality of the two folds. */
  readonly equal: boolean;
  /** Canonical-section equality (see `canonicalSections`). */
  readonly canonical_equal: boolean;
  /** sha256 over the canonical JSON of `via_log`. */
  readonly digest: string;
}

/**
 * Reconstruct the recorded Run headlessly: presentation is absent by
 * construction (this module imports none), kernel values are claimed only
 * when `kernelAlive` (default false — a headless reconstructor asserts only
 * what durable sources hold), and the SQLite path starts from NOTHING but the
 * log (`rebuildFromAuthoritativeLog` into `dbPath`).
 */
export function replayHeadless(
  recorded: RecordedRun,
  options: { dbPath?: string; kernelAlive?: boolean } = {},
): HeadlessReplayResult {
  const kernelAlive = options.kernelAlive ?? false;
  const viaLog = foldLogToNormalized(recorded, { kernelAlive });

  const store = SqliteProjectionStore.rebuildFromAuthoritativeLog(
    options.dbPath ?? ":memory:",
    recorded.generation_id,
    recorded.events,
  );
  const viaStore = normalizeSnapshot(store.getSnapshot(null), {
    run_id: recorded.run_id,
    events: recorded.events,
    kernelObjects: recorded.kernel.objects,
    kernelAlive,
  });

  const viaLogJson = canonicalJson(viaLog);
  return {
    via_log: viaLog,
    via_store: viaStore,
    store,
    equal: viaLogJson === canonicalJson(viaStore),
    canonical_equal:
      canonicalJson(canonicalSections(viaLog)) === canonicalJson(canonicalSections(viaStore)),
    digest: projectionDigest(viaLog),
  };
}

/**
 * The model's own trace fan-out formats `summary` as
 * `[source] event_type (cursor)` (workbench-model applyEvent). Parsing it back
 * here means the LIVE fold's audit truth comes from the model itself — so the
 * equality check below genuinely proves the model's fan-out trace matches the
 * log-derived trace, instead of assuming it.
 */
function parseTraceSummary(record: TraceRecord): NormalizedTraceRecord {
  const match = record.summary.match(/^\[(.+)\] (.+) \((.+)\)$/);
  if (match === null) {
    throw new Error(
      `headless replay: malformed model trace summary for '${record.trace_id}': '${record.summary}'`,
    );
  }
  const sourceId = match[1];
  const eventType = match[2];
  const cursor = match[3];
  if (sourceId === undefined || eventType === undefined || cursor === undefined) {
    throw new Error(
      `headless replay: malformed model trace summary for '${record.trace_id}': '${record.summary}'`,
    );
  }
  return {
    trace_id: record.trace_id,
    unit_id: record.unit_id,
    event_type: record.event_type,
    source_id: sourceId,
    cursor,
    timestamp_ms: record.timestamp_ms,
  };
}

/** Normalize the LIVE (with-presentation) fold for comparison. */
export function normalizeLiveFold(args: {
  run_id: string;
  generation_id: string;
  storeSnapshot: WorkbenchSnapshot;
  trace: readonly TraceRecord[];
  kernelObjects: readonly RecordedKernelObject[];
  kernelAlive: boolean;
}): NormalizedRunProjection {
  return assembleProjection({
    run_id: args.run_id,
    generation_id: args.generation_id,
    world: {
      generation: args.storeSnapshot.generation,
      units: args.storeSnapshot.units,
      evidence: args.storeSnapshot.evidence,
      sessions: args.storeSnapshot.sessions,
      source_cursors: args.storeSnapshot.source_cursors,
    },
    focus:
      args.storeSnapshot.focus === null
        ? null
        : {
            kind: semanticFocusKind(args.storeSnapshot.focus.kind),
            id: args.storeSnapshot.focus.id,
          },
    trace: args.trace.map((record) => parseTraceSummary(record)),
    kernel: kernelRecords(args.kernelObjects, args.kernelAlive),
  });
}

// --- Canonical JSON and digest ---------------------------------------------------

/**
 * Canonical JSON: object keys sorted, arrays in order, no whitespace. Two
 * reconstructions of the same recorded Run serialize byte-identically.
 */
export function canonicalJson(value: unknown): string {
  if (value === null || typeof value !== "object") {
    return JSON.stringify(value);
  }
  if (Array.isArray(value)) {
    return `[${value.map((element) => canonicalJson(element)).join(",")}]`;
  }
  const entries = Object.entries(value as Record<string, unknown>)
    .filter(([, field]) => field !== undefined)
    .sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0));
  return `{${entries.map(([key, field]) => `${JSON.stringify(key)}:${canonicalJson(field)}`).join(",")}}`;
}

/** sha256 over the canonical JSON of a projection (or any JSON value). */
export function projectionDigest(value: unknown): string {
  return createHash("sha256").update(canonicalJson(value)).digest("hex");
}
