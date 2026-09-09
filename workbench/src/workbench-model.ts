import {
  type AuthoritativeEventFrame,
  SqliteProjectionStore,
  type UnitProjection,
  type WorkbenchSnapshot,
} from "./projection-store";

/**
 * React-free Workbench Model and immutable snapshot fan-out.
 * Conforms to spec §20.2, REQ-DATA-001, REQ-DATA-005, REQ-DATA-006, REQ-TUI-001, and plan T07.
 *
 * Core Invariants:
 *   - Presentation code consumes immutable snapshots and interaction callbacks; it NEVER
 *     owns business state or communicates directly with backend adapters (REQ-DATA-001, REQ-TUI-001).
 *   - Chat and Trace are separate projections over durable history; Chat is NOT canonical
 *     input for Trace (REQ-DATA-005, Attack 1).
 *   - One source event fans out to multiple projections non-destructively; no view can
 *     consume an event away from another view (REQ-DATA-006, Attack 2).
 */

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

export interface FullWorkbenchSnapshot extends WorkbenchSnapshot {
  readonly trace_records: readonly TraceRecord[];
  readonly chat_messages: readonly ChatMessage[];
  readonly chat_enabled: boolean;
}

export type SnapshotListener = (snapshot: FullWorkbenchSnapshot) => void;

export interface WorkbenchModelOptions {
  readonly dbPath?: string;
  readonly enableChat?: boolean;
}

export class WorkbenchModel {
  readonly store: SqliteProjectionStore;
  private currentHorizon: string | null = null;
  private currentFocus: { kind: string; id: string } | null = null;
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
   * Start a new projection generation.
   */
  startGeneration(generationId: string): void {
    this.store.startGeneration(generationId);
    this.traceRecords = [];
    this.chatMessages = [];
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
   */
  applyEvent(frame: AuthoritativeEventFrame): { applied: boolean; reason?: string } {
    // 1. Apply to SQLite store
    const res = this.store.applyEvent(frame);
    if (!res.applied) {
      return res;
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
   * Interaction callback: Select active Focus.
   */
  selectFocus(kind: string, id: string): void {
    this.currentFocus = { kind, id };
    this.notifySubscribers();
  }

  /**
   * Interaction callback: Select active Horizon.
   */
  selectHorizon(horizon: string | null): void {
    this.currentHorizon = horizon;
    this.notifySubscribers();
  }

  /**
   * Build immutable FullWorkbenchSnapshot.
   */
  getSnapshot(): FullWorkbenchSnapshot {
    const baseSnapshot = this.store.getSnapshot(this.currentHorizon);

    return {
      ...baseSnapshot,
      focus: this.currentFocus ?? baseSnapshot.focus,
      trace_records: Object.freeze([...this.traceRecords]),
      chat_messages: Object.freeze([...this.chatMessages]),
      chat_enabled: this.chatEnabled,
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
