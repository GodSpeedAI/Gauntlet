/**
 * Live authoritative streams: generation-minted connect/reconnect handshake,
 * canonical cursor/sequence reconciliation, stale-frame rejection at the
 * stream layer, and Prime detach/reattach over live streams (plan T16;
 * REQ-DATA-002/003/004, REQ-OPS-001/002, REQ-OUT-003, REQ-PRIME-009/010,
 * REQ-SAFE-005, REQ-SYNC-001/002, REQ-UX-004, REQ-VERIFY-005/006).
 *
 * Core invariants:
 *   - Every connect/reconnect/resync mints a NEW ProjectionGeneration over the
 *     T06 store and reconciles a baseline plus per-source cursors. Canonical
 *     order is the source-declared per-source sequence (ascending; sources
 *     folded in sorted source-id order) — NEVER wall-clock timestamps alone
 *     (REQ-SYNC-001/002). Cross-source order is a deterministic presentation
 *     fold, not an implied cross-source causal order.
 *   - Frames minted under a superseded generation are typed-rejected and leave
 *     the live generation untouched — T06's in-store rejection rule extended
 *     to the stream layer (REQ-DATA-003, REQ-VERIFY-006).
 *   - One authoritative event is folded through the T06 store's single
 *     transaction, so its local cursor and all dependent read-model rows
 *     commit atomically (REQ-DATA-004). A frame the store REFUSES is never a
 *     silent cursor advance: on the baseline it is counted and reasoned in the
 *     handshake (`refused_events`/`refusal_reasons`) and held in the client's
 *     visible refusals/degraded reasons; on the live path it triggers the
 *     visible store_refusal resync (Correction Round 1 MAJOR).
 *   - Missing/out-of-order increments are bounded-buffered or explicitly
 *     resynced — never a silent cursor advance. A source epoch change or
 *     retention expiry forces a fresh baseline (REQ-SYNC-001). Inconsistency
 *     becomes VISIBLE degraded/resync state; authoritative sources are never
 *     mutated to make the projection agree (REQ-SAFE-005).
 *   - Detach persists ONLY noncanonical presentation cursor/view preferences
 *     and issues no cancel — `cancel_calls_issued` is the literal type 0 (the
 *     T10 receipt pattern; REQ-PRIME-009, REQ-UX-04 detach-leaves-work-running).
 *     Reattach rebuilds Prime topology/session refs from qualified Prime
 *     records (never from liveness or stale UI, REQ-PRIME-010) and reconciles
 *     events since the last authoritative cursor through a fresh generation
 *     (REQ-VERIFY-005/006). Presentation cursor persistence is not execution
 *     recovery: canonical execution lifetime stays with Prime.
 *
 * Determinism contract (prereg blindspots bind scope): transport loss is
 * exercised as deterministic in-process stream failure through the injected
 * `AuthoritativeStreamPort`; `InMemoryStreamSource`/`InMemoryStreamPort` are
 * the reference in-process port implementations. Real network targets are T20;
 * million-event scale is T21.
 */

import type { PrimeHostAdapter } from "./prime-host";
import type { ReattachmentReceipt } from "./prime-long-running";
import { reattachToPrime } from "./prime-long-running";
import type { AuthoritativeEventFrame, WorkbenchSnapshot } from "./projection-store";
import type { WorkbenchModel } from "./workbench-model";

// --- Stream port contract (REQ-SYNC-001: source identity, epoch, cursors) -----

/**
 * One event on an authoritative stream. `sequence` is the owning source's
 * declared canonical order (monotonic per source). `cursor` is an OPAQUE,
 * source-minted token: clients hand it back verbatim and never compare it
 * lexically (REQ-SYNC-001). `timestamp_ms` is presentation metadata — it never
 * participates in canonical ordering.
 */
export interface StreamEvent {
  readonly source_id: string;
  /** The owning source's store epoch at emit time (changes on storage replacement). */
  readonly source_epoch: string;
  readonly sequence: number;
  readonly cursor: string;
  readonly event_type: AuthoritativeEventFrame["event_type"];
  readonly payload: Record<string, unknown>;
  readonly timestamp_ms: number;
}

/** The source-declared stream position metadata a client may rely on. */
export interface SourceDescriptor {
  readonly source_id: string;
  readonly store_epoch: string;
  /** Retention floor: sequences below this are expired at the source. */
  readonly earliest_sequence: number;
  /** Every event with sequence <= this watermark exists (retained) at the source. */
  readonly watermark_sequence: number;
  /** Opaque cursor at the watermark (baseline position for a cold client). */
  readonly watermark_cursor: string;
}

/**
 * A window of authoritative events with its descriptor. Baseline windows
 * include ALL retained events through the watermark; increment windows begin
 * STRICTLY AFTER the requested sequence — never inclusive (REQ-SYNC-001).
 */
export interface SourceWindow {
  readonly descriptor: SourceDescriptor;
  readonly events: readonly StreamEvent[];
}

/**
 * The authoritative stream port (injected; deterministic in-process fakes stand
 * in for live network transport per the T16 prereg blindspot). Each adapter
 * source declares identity, epoch, cursor semantics, retention and baseline
 * watermark through its descriptors and windows.
 */
export interface AuthoritativeStreamPort {
  describeSources(): readonly SourceDescriptor[];
  /** Fresh baseline: all retained events through the current watermark. */
  baselineWindow(sourceId: string): SourceWindow;
  /** Increments strictly after `afterSequence`, through the current watermark. */
  incrementsSince(sourceId: string, afterSequence: number): SourceWindow;
}

/** The per-source position a client tracks (and the only thing detach persists). */
export interface SourcePosition {
  readonly epoch: string;
  readonly sequence: number;
  readonly cursor: string;
}

// --- Typed failures and rejections ---------------------------------------------

/** Typed stream failure carrying the client-operation locus. */
export class LiveStreamError extends Error {
  constructor(operation: string, detail: string) {
    super(`live stream ${operation}: ${detail}`);
    this.name = "LiveStreamError";
  }
}

export type FrameRejectionReason =
  | "superseded_generation"
  | "generation_closed"
  | "subscription_closed"
  | "already_applied"
  | "invalid_frame";

/**
 * A typed stream-frame rejection. `frame_timestamp_ms` rides along to prove
 * rejections are decided by generation/sequence identity — never by timestamps
 * (a late frame with a later-looking timestamp is still rejected).
 */
export interface FrameRejection {
  readonly class: "stale_frame";
  readonly reason: FrameRejectionReason;
  readonly frame_generation: string;
  readonly live_generation: string | null;
  readonly source_id: string;
  readonly sequence: number;
  readonly frame_timestamp_ms: number;
  readonly detail: string;
}

export type FrameIngestResult =
  | {
      readonly class: "applied";
      readonly source_id: string;
      readonly sequence: number;
      readonly cursor: string;
      readonly generation_id: string;
    }
  | {
      readonly class: "buffered";
      readonly source_id: string;
      readonly sequence: number;
      readonly detail: string;
    }
  | {
      readonly class: "resync_triggered";
      readonly source_id: string;
      readonly sequence: number;
      readonly reason: string;
    }
  | {
      readonly class: "rejected";
      readonly rejection: FrameRejection;
    };

/** A frame envelope tagged with the generation its delivery was minted under. */
export interface StreamFrameEnvelope {
  readonly generation_id: string;
  readonly event: StreamEvent;
}

// --- Canonical ordering and convergence -----------------------------------------

/**
 * The canonical fold order over stream events: sources in ascending source-id
 * order, each source's events in ascending source-declared sequence. Wall-clock
 * timestamps are NEVER consulted (REQ-SYNC-001/002). Cross-source adjacency is
 * a deterministic presentation fold, not a causal claim.
 */
export function canonicalStreamOrder(events: readonly StreamEvent[]): readonly StreamEvent[] {
  return [...events].sort((a, b) => {
    if (a.source_id !== b.source_id) {
      return a.source_id < b.source_id ? -1 : 1;
    }
    return a.sequence - b.sequence;
  });
}

/**
 * Compare two workbench snapshots for projection convergence. Semantic row
 * content and source provenance (source_id, source_cursor) must match; the
 * generation id and snapshot id differ by construction across generations and
 * `updated_at` timestamps are presentation metadata, so none of the three take
 * part. Returns the list of differences; empty means converged.
 */
export function projectionDifferences(
  left: WorkbenchSnapshot,
  right: WorkbenchSnapshot,
): readonly string[] {
  const diffs: string[] = [];

  const unitFields = [
    "source_id",
    "source_cursor",
    "title",
    "phase",
    "status",
    "authority",
    "blast_radius",
  ] as const;
  const leftUnits = new Map(left.units.map((u) => [u.unit_id, u]));
  const rightUnits = new Map(right.units.map((u) => [u.unit_id, u]));
  for (const [id, unit] of leftUnits) {
    const other = rightUnits.get(id);
    if (!other) {
      diffs.push(`unit:${id}:missing_in_right`);
      continue;
    }
    for (const field of unitFields) {
      if (unit[field] !== other[field]) {
        diffs.push(`unit:${id}.${field}:${String(unit[field])}!=${String(other[field])}`);
      }
    }
  }
  for (const id of rightUnits.keys()) {
    if (!leftUnits.has(id)) {
      diffs.push(`unit:${id}:missing_in_left`);
    }
  }

  const evidenceFields = [
    "source_id",
    "source_cursor",
    "unit_id",
    "kind",
    "locator",
    "verified",
  ] as const;
  const leftEvidence = new Map(left.evidence.map((e) => [e.evidence_id, e]));
  const rightEvidence = new Map(right.evidence.map((e) => [e.evidence_id, e]));
  for (const [id, evidence] of leftEvidence) {
    const other = rightEvidence.get(id);
    if (!other) {
      diffs.push(`evidence:${id}:missing_in_right`);
      continue;
    }
    for (const field of evidenceFields) {
      if (evidence[field] !== other[field]) {
        diffs.push(`evidence:${id}.${field}:${String(evidence[field])}!=${String(other[field])}`);
      }
    }
  }
  for (const id of rightEvidence.keys()) {
    if (!leftEvidence.has(id)) {
      diffs.push(`evidence:${id}:missing_in_left`);
    }
  }

  const sessionFields = ["source_id", "source_cursor", "parent_id", "model", "status"] as const;
  const leftSessions = new Map(left.sessions.map((s) => [s.session_id, s]));
  const rightSessions = new Map(right.sessions.map((s) => [s.session_id, s]));
  for (const [id, session] of leftSessions) {
    const other = rightSessions.get(id);
    if (!other) {
      diffs.push(`session:${id}:missing_in_right`);
      continue;
    }
    for (const field of sessionFields) {
      if (session[field] !== other[field]) {
        diffs.push(`session:${id}.${field}:${String(session[field])}!=${String(other[field])}`);
      }
    }
  }
  for (const id of rightSessions.keys()) {
    if (!leftSessions.has(id)) {
      diffs.push(`session:${id}:missing_in_left`);
    }
  }

  const cursorKeys = new Set([
    ...Object.keys(left.source_cursors),
    ...Object.keys(right.source_cursors),
  ]);
  for (const key of cursorKeys) {
    if (left.source_cursors[key] !== right.source_cursors[key]) {
      diffs.push(
        `source_cursor:${key}:${String(left.source_cursors[key])}!=${String(right.source_cursors[key])}`,
      );
    }
  }

  const leftFocus = left.focus ? `${left.focus.kind}:${left.focus.id}` : "none";
  const rightFocus = right.focus ? `${right.focus.kind}:${right.focus.id}` : "none";
  if (leftFocus !== rightFocus) {
    diffs.push(`focus:${leftFocus}!=${rightFocus}`);
  }

  return diffs;
}

// --- Reference in-process port ---------------------------------------------------

/**
 * Opaque, source-minted cursor token. Deliberately NOT lexically ordered with
 * respect to the sequence (REQ-SYNC-001: opaque cursors must not be lexically
 * compared) — a client that lexically compares these produces wrong order.
 */
export function mintOpaqueCursor(sequence: number): string {
  const scrambled = (sequence * 7919) % 104729;
  return `cursor-${(104729 - scrambled).toString(36)}`;
}

const GENESIS_CURSOR = "cursor-genesis";

/**
 * Deterministic in-memory authoritative source: the reference implementation of
 * one adapter stream. Retention (`expireThrough`) and epoch replacement
 * (`replaceStorage`) model the REQ-SYNC-001 boundary conditions; append order
 * is the canonical order, and timestamps are whatever the caller supplies.
 */
export class InMemoryStreamSource {
  readonly source_id: string;
  private epoch: string;
  private earliest: number;
  private watermark: number;
  private watermarkCursor: string;
  private readonly events = new Map<number, StreamEvent>();

  constructor(sourceId: string, epoch = "epoch-1") {
    this.source_id = sourceId;
    this.epoch = epoch;
    this.earliest = 1;
    this.watermark = 0;
    this.watermarkCursor = GENESIS_CURSOR;
  }

  get store_epoch(): string {
    return this.epoch;
  }

  get watermark_sequence(): number {
    return this.watermark;
  }

  get earliest_sequence(): number {
    return this.earliest;
  }

  /** Append one event; mints the next sequence and an opaque cursor. */
  append(input: {
    event_type: StreamEvent["event_type"];
    payload: Record<string, unknown>;
    timestamp_ms: number;
  }): StreamEvent {
    this.watermark += 1;
    const event: StreamEvent = {
      source_id: this.source_id,
      source_epoch: this.epoch,
      sequence: this.watermark,
      cursor: mintOpaqueCursor(this.watermark),
      event_type: input.event_type,
      payload: input.payload,
      timestamp_ms: input.timestamp_ms,
    };
    this.events.set(event.sequence, event);
    return event;
  }

  /** Retention expiry (same epoch): sequences through `sequence` are dropped. */
  expireThrough(sequence: number): void {
    this.earliest = Math.max(this.earliest, sequence + 1);
    for (const seq of this.events.keys()) {
      if (seq < this.earliest) {
        this.events.delete(seq);
      }
    }
  }

  /**
   * Storage replacement (REQ-SYNC-001: epoch replacement requires a fresh
   * baseline): the epoch changes, retained history is gone, sequences restart.
   */
  replaceStorage(epoch: string): void {
    this.epoch = epoch;
    this.events.clear();
    this.watermark = 0;
    this.earliest = 1;
    this.watermarkCursor = GENESIS_CURSOR;
  }

  /** The retained authoritative log, ascending by sequence. */
  retainedEvents(): readonly StreamEvent[] {
    return [...this.events.values()]
      .filter((event) => event.sequence >= this.earliest)
      .sort((a, b) => a.sequence - b.sequence);
  }

  describeSource(): SourceDescriptor {
    return {
      source_id: this.source_id,
      store_epoch: this.epoch,
      earliest_sequence: this.earliest,
      watermark_sequence: this.watermark,
      watermark_cursor: this.watermarkCursor,
    };
  }

  baselineWindow(): SourceWindow {
    return { descriptor: this.describeSource(), events: this.retainedEvents() };
  }

  incrementsSince(afterSequence: number): SourceWindow {
    return {
      descriptor: this.describeSource(),
      events: this.retainedEvents().filter((event) => event.sequence > afterSequence),
    };
  }
}

/** Reference multi-source port over `InMemoryStreamSource`s. */
export class InMemoryStreamPort implements AuthoritativeStreamPort {
  private readonly sources: Map<string, InMemoryStreamSource>;

  constructor(sources: readonly InMemoryStreamSource[]) {
    this.sources = new Map(sources.map((source) => [source.source_id, source]));
  }

  source(sourceId: string): InMemoryStreamSource {
    const source = this.sources.get(sourceId);
    if (!source) {
      throw new LiveStreamError("port", `unknown source '${sourceId}'`);
    }
    return source;
  }

  describeSources(): readonly SourceDescriptor[] {
    return [...this.sources.keys()].sort().map((id) => {
      const source = this.sources.get(id);
      if (!source) {
        throw new LiveStreamError("port", `source '${id}' vanished from the port map`);
      }
      return source.describeSource();
    });
  }

  baselineWindow(sourceId: string): SourceWindow {
    return this.source(sourceId).baselineWindow();
  }

  incrementsSince(sourceId: string, afterSequence: number): SourceWindow {
    return this.source(sourceId).incrementsSince(afterSequence);
  }
}

// --- Handshake and reconciliation records ---------------------------------------

export type CursorValidation =
  | "no_persisted_cursor"
  | "valid"
  | "epoch_changed"
  | "retention_expired";

export interface SourceReconciliation {
  readonly source_id: string;
  readonly store_epoch: string;
  /** A fresh generation starts with an empty T06 projection, so every
   *  reconciliation lays the full baseline through the watermark; the persisted
   *  cursor decides whether that baseline is a cold rebuild or a validated
   *  catch-up, and how much of it is "since the last authoritative cursor". */
  readonly strategy: "fresh_baseline" | "baseline_with_cursor_validation";
  readonly cursor_validation: CursorValidation;
  readonly persisted_sequence: number | null;
  readonly baseline_watermark_sequence: number;
  /** Baseline events the projection store ADMITTED (refused events are
   *  reported separately, never counted here). */
  readonly applied_events: number;
  /** Baseline events the projection store REFUSED (e.g. an unattributable
   *  focus_changed). Their sequences are consumed-and-refused in the tracked
   *  position, and every refusal is reported here and in the client's
   *  degraded reasons — never a silent advance. */
  readonly refused_events: number;
  /** The store's typed refusal reason per refused baseline event, in fold order. */
  readonly refusal_reasons: readonly string[];
  /** Baseline events at or before the persisted cursor (re-confirmed state). */
  readonly reconfirmed_events: number;
  /** Baseline events strictly after the persisted cursor — the reconciled
   *  delta the last authoritative cursor could not cover. */
  readonly reconciled_since_cursor: number;
  /** Sequences below the source retention floor, expired at the source. */
  readonly skipped_expired_sequences: number;
  readonly position: SourcePosition;
}

/** One store-refused baseline event, reported for the life of its generation. */
export interface BaselineRefusal {
  readonly generation_id: string;
  readonly source_id: string;
  readonly sequence: number;
  readonly cursor: string;
  /** The store's typed refusal reason (failure locus preserved). */
  readonly reason: string;
}

export interface ConnectHandshake {
  readonly kind: "connect" | "reconnect" | "resync";
  readonly generation_id: string;
  readonly per_source: readonly SourceReconciliation[];
  readonly applied_events: number;
  readonly resync_reason: string | null;
}

// --- Visible connection state (degraded/resync, REQ-SAFE-005) -------------------

export type StreamConnectionState =
  | "idle"
  | "connecting"
  | "live"
  | "buffering"
  | "resyncing"
  | "closed";

export interface StreamStateTransition {
  readonly from: StreamConnectionState;
  readonly to: StreamConnectionState;
  readonly reason: string;
  readonly generation_id: string | null;
  readonly serial: number;
}

export type StateListener = (transition: StreamStateTransition) => void;

// --- Detach / reattach (T10 receipt pattern over live streams) ------------------

export type DetachedViewPreferences = Readonly<Record<string, string>>;

/**
 * What a detach persists: ONLY noncanonical presentation state — per-source
 * read cursors and view preferences. This is presentation cursor persistence,
 * not execution recovery: it never carries, restarts, or cancels Prime work.
 */
export interface DetachedPresentationState {
  readonly detached_from_generation: string;
  readonly source_positions: Readonly<Record<string, SourcePosition>>;
  readonly view_preferences: DetachedViewPreferences;
}

/** Typed detach receipt. `cancel_calls_issued` is the literal type 0. */
export interface StreamDetachReceipt {
  readonly detached: true;
  readonly cancel_calls_issued: 0;
  readonly persisted: DetachedPresentationState;
  readonly persistence_class: "noncanonical_presentation_state";
}

export interface ReattachOptions {
  readonly persisted?: DetachedPresentationState | undefined;
  /** When provided, Prime topology/session refs are rebuilt from these
   *  qualified records via T10's `reattachToPrime` — never from liveness. */
  readonly prime_adapter?: PrimeHostAdapter | undefined;
}

export interface StreamReattachReceipt {
  readonly cancel_calls_issued: 0;
  readonly handshake: ConnectHandshake;
  readonly prime: ReattachmentReceipt | null;
  /** Completion is only ever read from schema-valid result contracts in the
   *  qualified records — never inferred from liveness or message volume. */
  readonly completion_basis: "schema_valid_result_contracts_only";
  readonly presentation_source:
    | "persisted_noncanonical_cursor_state"
    | "fresh_baseline_no_persisted_state";
}

export interface PollReport {
  readonly polled_sources: readonly string[];
  readonly applied: number;
  readonly buffered: number;
  readonly rejected: number;
  readonly resync_triggered: string | null;
}

export interface LiveStreamClientOptions {
  readonly model: WorkbenchModel;
  readonly port: AuthoritativeStreamPort;
  /** Injected generation-id minting (deterministic in tests). */
  readonly mintGenerationId?: (serial: number) => string;
  /** Bounded backlog capacity before overload resync (REQ-SYNC-001/002). */
  readonly maxBufferedFrames?: number;
}

/**
 * One live-stream client over one WorkbenchModel. Each client owns its own
 * generation on its own model, so two clients over one port stay isolated.
 */
export class LiveStreamClient {
  private readonly model: WorkbenchModel;
  private readonly port: AuthoritativeStreamPort;
  private readonly mintGenerationId: (serial: number) => string;
  private readonly maxBufferedFrames: number;
  private generationSerial = 0;
  private transitionSerial = 0;
  private liveGeneration: string | null = null;
  private currentState: StreamConnectionState = "idle";
  private readonly positions = new Map<string, SourcePosition>();
  private readonly buffered = new Map<string, Map<number, StreamEvent>>();
  private bufferedTotal = 0;
  private readonly gappedSources = new Set<string>();
  private readonly gapExpected = new Map<string, number>();
  private readonly transitions: StreamStateTransition[] = [];
  private readonly rejections: FrameRejection[] = [];
  private refusalList: BaselineRefusal[] = [];
  private readonly stateListeners = new Set<StateListener>();
  private degraded: readonly string[] = [];
  private handshake: ConnectHandshake | null = null;
  private resyncs = 0;

  constructor(options: LiveStreamClientOptions) {
    this.model = options.model;
    this.port = options.port;
    this.mintGenerationId = options.mintGenerationId ?? ((serial) => `gen-${serial}`);
    this.maxBufferedFrames = options.maxBufferedFrames ?? 64;
  }

  // --- observability ---

  get state(): StreamConnectionState {
    return this.currentState;
  }

  get generationId(): string | null {
    return this.liveGeneration;
  }

  get degradedReasons(): readonly string[] {
    return this.degraded;
  }

  get stateLog(): readonly StreamStateTransition[] {
    return [...this.transitions];
  }

  get rejectionLog(): readonly FrameRejection[] {
    return [...this.rejections];
  }

  /** Store-refused baseline events reported for the CURRENT generation
   *  (correction round 1: a refusal is never silent — see BaselineRefusal). */
  get refusals(): readonly BaselineRefusal[] {
    return [...this.refusalList];
  }

  get resyncCount(): number {
    return this.resyncs;
  }

  get lastHandshake(): ConnectHandshake | null {
    return this.handshake;
  }

  bufferedCount(): number {
    return this.bufferedTotal;
  }

  sourcePositions(): readonly SourcePosition[] {
    return [...this.positions.values()].map((position) => ({ ...position }));
  }

  onStateChange(listener: StateListener): () => void {
    this.stateListeners.add(listener);
    return () => {
      this.stateListeners.delete(listener);
    };
  }

  // --- connect / reconnect / resync (generation-minted handshake) ---

  /** Mint a generation, lay the authoritative baseline, go live. */
  connect(): ConnectHandshake {
    if (this.currentState === "closed") {
      throw new LiveStreamError("connect", "the client is closed");
    }
    return this.reconcileAll("connect", undefined, null);
  }

  /**
   * Mint a NEW generation and reconcile it. With no argument, the client's own
   * tracked per-source cursors are the last authoritative positions; an
   * explicit persisted state (from a detach, possibly another process) is
   * validated per source (epoch, retention) instead. Either way the fresh T06
   * generation lays the baseline through the live watermark, which includes
   * every event since the last authoritative cursor.
   */
  reconnect(persisted?: DetachedPresentationState | undefined): ConnectHandshake {
    if (this.currentState === "closed") {
      throw new LiveStreamError("reconnect", "the client is closed");
    }
    const persistedState =
      persisted ??
      Object.freeze({
        detached_from_generation: this.liveGeneration ?? "none",
        source_positions: Object.freeze(Object.fromEntries([...this.positions])),
        view_preferences: Object.freeze({}) as DetachedViewPreferences,
      });
    return this.reconcileAll("reconnect", persistedState, null);
  }

  private reconcileAll(
    kind: "connect" | "reconnect" | "resync",
    persisted: DetachedPresentationState | undefined,
    resyncReason: string | null,
  ): ConnectHandshake {
    this.generationSerial += 1;
    const generationId = this.mintGenerationId(this.generationSerial);
    this.liveGeneration = generationId;
    if (kind !== "resync") {
      this.transition("connecting", `${kind}_started`);
    }
    this.model.startGeneration(generationId);
    this.positions.clear();
    this.refusalList = [];

    const perSource: SourceReconciliation[] = [];
    let appliedTotal = 0;
    for (const descriptor of this.port.describeSources()) {
      const reconciliation = this.reconcileSource(generationId, descriptor, persisted);
      perSource.push(reconciliation);
      appliedTotal += reconciliation.applied_events;
    }

    this.model.markReady(generationId);
    const handshake: ConnectHandshake = {
      kind,
      generation_id: generationId,
      per_source: Object.freeze(perSource),
      applied_events: appliedTotal,
      resync_reason: resyncReason,
    };
    this.handshake = handshake;
    this.transition("live", `${kind}_complete`);
    return handshake;
  }

  private reconcileSource(
    generationId: string,
    descriptor: SourceDescriptor,
    persisted: DetachedPresentationState | undefined,
  ): SourceReconciliation {
    const persistedPosition = persisted?.source_positions[descriptor.source_id];
    let validation: CursorValidation;
    if (!persistedPosition) {
      validation = "no_persisted_cursor";
    } else if (persistedPosition.epoch !== descriptor.store_epoch) {
      validation = "epoch_changed";
    } else if (persistedPosition.sequence < descriptor.earliest_sequence - 1) {
      validation = "retention_expired";
    } else {
      validation = "valid";
    }
    const strategy = validation === "valid" ? "baseline_with_cursor_validation" : "fresh_baseline";

    const window = this.port.baselineWindow(descriptor.source_id);
    // Baseline events are folded in canonical order; each admitted one commits
    // its cursor and dependent rows atomically through the T06 store. A
    // store-REFUSED event is never silent: it is counted, reasoned, reported
    // on this leg and in the client's degraded reasons. Its sequence is marked
    // consumed-and-refused in the tracked position — a position frozen before
    // the refusal would livelock gap detection against a frame the source
    // re-serves identically forever (Correction Round 1 MAJOR).
    const refusalReasons: string[] = [];
    let admitted = 0;
    for (const event of canonicalStreamOrder(window.events)) {
      const result = this.applyToGeneration(generationId, event);
      if (result.applied) {
        admitted += 1;
      } else {
        const reason = result.reason ?? "store_refused";
        refusalReasons.push(reason);
        this.refusalList.push({
          generation_id: generationId,
          source_id: descriptor.source_id,
          sequence: event.sequence,
          cursor: event.cursor,
          reason,
        });
      }
    }

    const lastEvent = window.events.at(-1);
    const position: SourcePosition = lastEvent
      ? { epoch: descriptor.store_epoch, sequence: lastEvent.sequence, cursor: lastEvent.cursor }
      : {
          epoch: descriptor.store_epoch,
          sequence: descriptor.watermark_sequence,
          cursor: descriptor.watermark_cursor,
        };
    this.positions.set(descriptor.source_id, position);

    const persistedSequence = persistedPosition ? persistedPosition.sequence : null;
    const reconciledSinceCursor =
      persistedSequence === null
        ? window.events.length
        : window.events.filter((event) => event.sequence > persistedSequence).length;
    const reconfirmed =
      persistedSequence === null
        ? 0
        : window.events.filter((event) => event.sequence <= persistedSequence).length;
    const skippedExpired = Math.max(0, descriptor.earliest_sequence - 1);

    return {
      source_id: descriptor.source_id,
      store_epoch: descriptor.store_epoch,
      strategy,
      cursor_validation: validation,
      persisted_sequence: persistedSequence,
      baseline_watermark_sequence: descriptor.watermark_sequence,
      applied_events: admitted,
      refused_events: refusalReasons.length,
      refusal_reasons: Object.freeze(refusalReasons),
      reconfirmed_events: reconfirmed,
      reconciled_since_cursor: reconciledSinceCursor,
      skipped_expired_sequences: skippedExpired,
      position,
    };
  }

  /** Fold one event into the current generation (assumes identity pre-checked).
   *  Returns the store's admission result, carrying the typed refusal reason. */
  private applyToGeneration(
    generationId: string,
    event: StreamEvent,
  ): { applied: boolean; reason?: string } {
    const frame: AuthoritativeEventFrame = {
      generation_id: generationId,
      source_id: event.source_id,
      cursor: event.cursor,
      event_type: event.event_type,
      payload: event.payload,
      timestamp_ms: event.timestamp_ms,
    };
    return this.model.applyEvent(frame);
  }

  // --- live delivery (subscriptions, ingest, bounded buffering, resync) ---

  /** Mint a live subscription under the CURRENT generation. */
  subscribeStream(): StreamSubscription {
    if (this.currentState === "closed" || this.generationId === null) {
      throw new LiveStreamError("subscribeStream", "the client has no live generation");
    }
    return new StreamSubscription(this, this.generationId);
  }

  ingest(envelope: StreamFrameEnvelope): FrameIngestResult {
    if (this.currentState === "closed" || this.generationId === null) {
      return this.reject(envelope, "generation_closed", "the client is closed");
    }
    return this.ingestLive(envelope);
  }

  /** Ingest path for frames from a CLOSED subscription: typed rejection, never applied. */
  ingestClosedSubscription(envelope: StreamFrameEnvelope): FrameIngestResult {
    return this.reject(envelope, "subscription_closed", "the delivering subscription is closed");
  }

  private ingestLive(envelope: StreamFrameEnvelope): FrameIngestResult {
    const event = envelope.event;
    if (envelope.generation_id !== this.generationId) {
      return this.reject(
        envelope,
        "superseded_generation",
        `frame generation '${envelope.generation_id}' is not the live generation '${this.generationId}'`,
      );
    }
    if (this.currentState === "resyncing") {
      return this.reject(envelope, "generation_closed", "a resync is replacing this generation");
    }
    const position = this.positions.get(event.source_id);
    if (!position) {
      return this.reject(envelope, "invalid_frame", `unknown source '${event.source_id}'`);
    }
    if (event.source_epoch !== position.epoch) {
      const reason = `epoch_change:${event.source_id}:frame=${event.source_epoch}:tracked=${position.epoch}`;
      this.beginResync(reason);
      return {
        class: "resync_triggered",
        source_id: event.source_id,
        sequence: event.sequence,
        reason,
      };
    }
    if (event.sequence <= position.sequence) {
      return this.reject(
        envelope,
        "already_applied",
        `sequence ${event.sequence} is at or behind the applied position ${position.sequence}`,
      );
    }
    if (event.sequence > position.sequence + 1) {
      return this.bufferFrame(event, position.sequence);
    }
    if (!this.applyInOrder(event)) {
      const reason = this.transitions.at(-1)?.reason ?? "store_refusal";
      return {
        class: "resync_triggered",
        source_id: event.source_id,
        sequence: event.sequence,
        reason,
      };
    }
    this.drain(event.source_id);
    this.settleBufferingState();
    return {
      class: "applied",
      source_id: event.source_id,
      sequence: event.sequence,
      cursor: event.cursor,
      generation_id: envelope.generation_id,
    };
  }

  private bufferFrame(event: StreamEvent, appliedSequence: number): FrameIngestResult {
    if (this.bufferedTotal >= this.maxBufferedFrames) {
      const reason = `buffer_overflow:${event.source_id}:capacity=${this.maxBufferedFrames}`;
      this.beginResync(reason);
      return {
        class: "resync_triggered",
        source_id: event.source_id,
        sequence: event.sequence,
        reason,
      };
    }
    let perSource = this.buffered.get(event.source_id);
    if (!perSource) {
      perSource = new Map<number, StreamEvent>();
      this.buffered.set(event.source_id, perSource);
    }
    if (!perSource.has(event.sequence)) {
      perSource.set(event.sequence, event);
      this.bufferedTotal += 1;
    }
    this.gappedSources.add(event.source_id);
    this.gapExpected.set(event.source_id, appliedSequence + 1);
    if (this.currentState === "live") {
      this.transition(
        "buffering",
        `gap_detected:${event.source_id}:expected=${appliedSequence + 1}:got=${event.sequence}`,
      );
    }
    return {
      class: "buffered",
      source_id: event.source_id,
      sequence: event.sequence,
      detail: `awaiting sequence ${appliedSequence + 1}`,
    };
  }

  /** Apply one in-order event; advance the tracked position atomically with the store.
   *  Returns false when the store refused the frame (a resync was triggered). */
  private applyInOrder(event: StreamEvent): boolean {
    const result = this.applyToGeneration(this.generationId ?? "", event);
    if (!result.applied) {
      // The store refused an identity-checked frame; that is a projection
      // inconsistency — visible resync state, never a silent advance.
      this.beginResync(`store_refusal:${event.source_id}:sequence=${event.sequence}`);
      return false;
    }
    this.positions.set(event.source_id, {
      epoch: event.source_epoch,
      sequence: event.sequence,
      cursor: event.cursor,
    });
    return true;
  }

  /** Drain contiguous buffered frames for one source, in canonical order. */
  private drain(sourceId: string): void {
    for (;;) {
      const position = this.positions.get(sourceId);
      if (!position) {
        return;
      }
      const perSource = this.buffered.get(sourceId);
      const next = perSource?.get(position.sequence + 1);
      if (!next) {
        return;
      }
      perSource?.delete(next.sequence);
      this.bufferedTotal -= 1;
      this.applyInOrder(next);
      if (this.currentState === "resyncing") {
        return;
      }
    }
  }

  /** Return to live when no gap is awaited and nothing is buffered. */
  private settleBufferingState(): void {
    for (const [sourceId, expected] of this.gapExpected) {
      const reached = this.positions.get(sourceId);
      if (reached && reached.sequence >= expected) {
        this.gapExpected.delete(sourceId);
        this.gappedSources.delete(sourceId);
      }
    }
    if (
      this.currentState === "buffering" &&
      this.bufferedTotal === 0 &&
      this.gappedSources.size === 0
    ) {
      this.transition("live", "gap_filled");
    }
  }

  /**
   * Deterministic catch-up pull: per source, fetch increments strictly after
   * the applied position and run them through the SAME ingest path as live
   * delivery (generation tagging, gap buffering, epoch checks). Detects epoch
   * replacement and retention expiry proactively from the descriptors.
   */
  poll(): PollReport {
    const generationId = this.generationId;
    if (this.currentState === "closed" || generationId === null) {
      throw new LiveStreamError("poll", "the client is closed");
    }
    if (this.currentState === "resyncing") {
      return {
        polled_sources: [],
        applied: 0,
        buffered: 0,
        rejected: 0,
        resync_triggered: "already_resyncing",
      };
    }
    let applied = 0;
    let buffered = 0;
    let rejected = 0;
    let resyncReason: string | null = null;
    const polled: string[] = [];
    for (const descriptor of this.port.describeSources()) {
      if (resyncReason !== null) {
        break;
      }
      const position = this.positions.get(descriptor.source_id);
      if (!position) {
        continue;
      }
      polled.push(descriptor.source_id);
      if (descriptor.store_epoch !== position.epoch) {
        resyncReason = `epoch_change:${descriptor.source_id}:tracked=${position.epoch}:current=${descriptor.store_epoch}`;
        break;
      }
      if (position.sequence < descriptor.earliest_sequence - 1) {
        resyncReason = `retention_expired:${descriptor.source_id}:applied=${position.sequence}:earliest=${descriptor.earliest_sequence}`;
        break;
      }
      const window = this.port.incrementsSince(descriptor.source_id, position.sequence);
      for (const event of window.events) {
        const result = this.ingest({ generation_id: generationId, event });
        if (result.class === "applied") {
          applied += 1;
        } else if (result.class === "buffered") {
          buffered += 1;
        } else if (result.class === "resync_triggered") {
          resyncReason = result.reason;
          break;
        } else {
          rejected += 1;
        }
      }
    }
    if (resyncReason !== null) {
      this.beginResync(resyncReason);
    }
    return {
      polled_sources: Object.freeze(polled),
      applied,
      buffered,
      rejected,
      resync_triggered: resyncReason,
    };
  }

  /**
   * Explicit resync: mint a fresh generation and re-baseline every source
   * (REQ-SYNC-001 epoch/retention rule; REQ-SAFE-005 resync-not-mutate).
   */
  resync(reason: string): ConnectHandshake {
    if (this.currentState === "closed") {
      throw new LiveStreamError("resync", "the client is closed");
    }
    this.beginResync(reason);
    const handshake = this.handshake;
    if (handshake === null) {
      throw new LiveStreamError("resync", "resync produced no handshake");
    }
    return handshake;
  }

  private beginResync(reason: string): void {
    if (
      this.currentState === "resyncing" ||
      this.currentState === "closed" ||
      this.currentState === "idle" ||
      this.currentState === "connecting"
    ) {
      return;
    }
    this.buffered.clear();
    this.bufferedTotal = 0;
    this.gappedSources.clear();
    this.gapExpected.clear();
    this.transition("resyncing", reason);
    this.reconcileAll("resync", undefined, reason);
    this.resyncs += 1;
  }

  // --- detach / reattach (REQ-PRIME-009/010, REQ-UX-004) ---

  /**
   * Detach: persist ONLY noncanonical presentation state (per-source cursors +
   * view preferences). No cancel is issued — the receipt's `cancel_calls_issued`
   * is the literal type 0, and no adapter call happens here at all.
   */
  detach(viewPreferences: DetachedViewPreferences = {}): StreamDetachReceipt {
    if (this.currentState === "closed") {
      throw new LiveStreamError("detach", "the client is closed");
    }
    const sourcePositions: Record<string, SourcePosition> = {};
    for (const [sourceId, position] of this.positions) {
      sourcePositions[sourceId] = { ...position };
    }
    return Object.freeze({
      detached: true as const,
      cancel_calls_issued: 0 as const,
      persisted: Object.freeze({
        detached_from_generation: this.generationId ?? "none",
        source_positions: Object.freeze(sourcePositions),
        view_preferences: Object.freeze({ ...viewPreferences }),
      }),
      persistence_class: "noncanonical_presentation_state" as const,
    });
  }

  /**
   * Reattach: rebuild Prime topology/session refs from qualified records when a
   * Prime adapter is supplied (never from liveness), and reconcile the stream
   * by minting a fresh generation whose baseline includes every event since
   * the last authoritative cursor.
   */
  reattach(options: ReattachOptions = {}): StreamReattachReceipt {
    if (this.currentState === "closed") {
      throw new LiveStreamError("reattach", "the client is closed");
    }
    const prime = options.prime_adapter ? reattachToPrime(options.prime_adapter) : null;
    const handshake = this.reconcileAll("reconnect", options.persisted, null);
    return Object.freeze({
      cancel_calls_issued: 0 as const,
      handshake,
      prime,
      completion_basis: "schema_valid_result_contracts_only" as const,
      presentation_source: options.persisted
        ? "persisted_noncanonical_cursor_state"
        : "fresh_baseline_no_persisted_state",
    });
  }

  /** Close the client (the TUI going away). Marks subscriptions stale. */
  close(): void {
    if (this.currentState === "closed") {
      return;
    }
    this.transition("closed", "client_closed");
    this.buffered.clear();
    this.bufferedTotal = 0;
    this.gappedSources.clear();
    this.gapExpected.clear();
  }

  // --- internals ---

  private transition(to: StreamConnectionState, reason: string): void {
    this.transitionSerial += 1;
    const record: StreamStateTransition = {
      from: this.currentState,
      to,
      reason,
      generation_id: this.generationId,
      serial: this.transitionSerial,
    };
    this.currentState = to;
    this.transitions.push(record);
    this.degraded = this.computeDegradedReasons();
    for (const listener of this.stateListeners) {
      listener(record);
    }
  }

  private computeDegradedReasons(): readonly string[] {
    if (
      this.currentState === "closed" ||
      this.currentState === "idle" ||
      this.currentState === "connecting"
    ) {
      return Object.freeze([]);
    }
    // Baseline refusals are a persistent projection hole: they stay visible in
    // the degraded reasons for as long as the generation that re-baselined
    // over them is live (Correction Round 1 MAJOR).
    const refusalReasons = this.refusalList.map(
      (refusal) =>
        `baseline_refusal:${refusal.source_id}:sequence=${refusal.sequence}:${refusal.reason}`,
    );
    if (this.currentState === "resyncing") {
      const last = this.transitions.at(-1);
      const resyncReason = last ? [last.reason] : [];
      return Object.freeze([...resyncReason, ...refusalReasons]);
    }
    if (this.currentState === "buffering") {
      const reasons: string[] = [];
      for (const [sourceId, expected] of this.gapExpected) {
        reasons.push(`awaiting_increment:${sourceId}:expected=${expected}`);
      }
      return Object.freeze([...reasons, ...refusalReasons]);
    }
    return Object.freeze(refusalReasons);
  }

  private reject(
    envelope: StreamFrameEnvelope,
    reason: FrameRejectionReason,
    detail: string,
  ): FrameIngestResult {
    const rejection: FrameRejection = {
      class: "stale_frame",
      reason,
      frame_generation: envelope.generation_id,
      live_generation: this.generationId,
      source_id: envelope.event.source_id,
      sequence: envelope.event.sequence,
      frame_timestamp_ms: envelope.event.timestamp_ms,
      detail,
    };
    this.rejections.push(rejection);
    return { class: "rejected", rejection };
  }
}

/** A live delivery channel minted under ONE generation; frames it delivers
 *  carry that generation forever, so a subscription that outlives its
 *  generation is exactly how stale frames arrive. */
export class StreamSubscription {
  readonly generation_id: string;
  private readonly client: LiveStreamClient;
  private closed = false;

  constructor(client: LiveStreamClient, generationId: string) {
    this.client = client;
    this.generation_id = generationId;
  }

  isClosed(): boolean {
    return this.closed || this.client.state === "closed";
  }

  deliver(event: StreamEvent): FrameIngestResult {
    if (this.isClosed()) {
      return this.client.ingestClosedSubscription({
        generation_id: this.generation_id,
        event,
      });
    }
    return this.client.ingest({ generation_id: this.generation_id, event });
  }

  close(): void {
    this.closed = true;
  }
}
