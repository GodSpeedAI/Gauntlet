/**
 * TraceStream — the pure presentation state machine behind the Trace view's
 * follow/hold/unseen/load-older behavior (GUI-redesign increment 6, core).
 *
 * Spec anchors:
 * - UESC §16.2 (TraceStream stream machine: COLD → HYDRATING → LIVE_FOLLOW →
 *   INSPECTION_HOLD → LIVE_FOLLOW; "operator inspection outranks streaming
 *   novelty").
 * - ICM §16.4 (trace.follow.suspend: scroll away from the tail suspends follow
 *   without yanking the viewport; unseen arrivals from either source accrue),
 *   §16.5 (trace.follow.resume: explicit return to live), §16.6
 *   (trace.history.load_older: preserve selection/scroll anchor).
 * - Adversarial tooth: "Trace does not jump to live tail while the user
 *   inspects history."
 *
 * Two labelled sources — "gauntlet" and "realitytrace" — are never merged
 * (UESC/ICM §16.0): rows keep their source, per-source order is the caller's
 * responsibility, and cross-source order inside `records` is arrival order
 * only, a presentation fold. Ids are source-qualified by the caller, so a bare
 * `id` string is unambiguous in practice; this module never deduplicates,
 * reorders, or restyles records, and two records that share an id across
 * sources remain two records.
 *
 * Discipline: pure functions over frozen data; no imports, no DOM/terminal/
 * model access. Every operation returns a new frozen state (or the same
 * reference for documented identity no-ops); callers' arrays and records are
 * copied on admission, never mutated or aliased.
 *
 * Semantics decisions (view must respect):
 * - `streamScrollUp`/`streamScrollDown` move the window by ONE row per call;
 *   the view maps its own scroll granularity onto repeated calls.
 * - `unseenCount` counts arrivals from both sources in total; a per-source
 *   breakdown is the view's derived presentation.
 * - During INSPECTION_HOLD the window is anchored at `anchorTopId` and spans
 *   `windowSize` rows. If the held window was partial (stream shorter than the
 *   window), new arrivals may extend it downward inside the fixed frame; the
 *   anchor row and every previously visible row keep their positions — the
 *   window never moves toward the live tail.
 * - COLD/HYDRATING are explicit-only states: no operation other than
 *   `streamHydrated` admits records or leaves them, so nothing can implicitly
 *   fabricate a live stream. HYDRATING is entered by the (future) reconnect
 *   wiring, not by this API.
 */

/** Follow machine states (UESC §16.2 stream machine). */
export type TraceFollowState = "COLD" | "HYDRATING" | "LIVE_FOLLOW" | "INSPECTION_HOLD";

/** The two labelled trace sources; never merged into one identity (UESC §16.0). */
export type TraceSource = "gauntlet" | "realitytrace";

/**
 * Minimal trace row shape the state machine operates on. The Trace view maps
 * real records (Gauntlet trace_records / RealityTrace ledger rows) onto this;
 * `ordinal` and `timestampMs` are carried for the view's per-source ordering
 * and are ignored by the machine itself.
 */
export interface TraceStreamRecord {
  readonly id: string;
  readonly source: TraceSource;
  readonly ordinal?: number;
  readonly timestampMs?: number;
}

/** Frozen presentation state of one trace stream. */
export interface TraceStream {
  readonly followState: TraceFollowState;
  readonly unseenCount: number;
  readonly windowSize: number;
  /** Top row of the held window; null unless INSPECTION_HOLD with records. */
  readonly anchorTopId: string | null;
  /** Selection by stable id only; independent of the viewport. */
  readonly selectedId: string | null;
  /** All admitted records, arrival order (cross-source order is a fold). */
  readonly records: readonly TraceStreamRecord[];
}

const TRACE_SOURCES: readonly TraceSource[] = ["gauntlet", "realitytrace"];

/** Validate and freeze a batch of caller-supplied records (untrusted input). */
function admitRecords(records: readonly TraceStreamRecord[]): readonly TraceStreamRecord[] {
  return Object.freeze(
    records.map((record) => {
      if (typeof record?.id !== "string" || record.id === "") {
        throw new TypeError(
          `trace-stream: record id must be a non-empty string, got ${String(record?.id)}`,
        );
      }
      if (!TRACE_SOURCES.includes(record.source)) {
        throw new TypeError(`trace-stream: unknown trace source ${String(record.source)}`);
      }
      return Object.freeze({ ...record });
    }),
  );
}

/** Freeze a new state with `records` already admitted. */
function freezeState(
  followState: TraceFollowState,
  unseenCount: number,
  windowSize: number,
  anchorTopId: string | null,
  selectedId: string | null,
  records: readonly TraceStreamRecord[],
): TraceStream {
  return Object.freeze({
    followState,
    unseenCount,
    windowSize,
    anchorTopId,
    selectedId,
    records,
  });
}

/** Index of the first row of the current window, by the current state. */
function windowStart(s: TraceStream): number {
  const tailStart = Math.max(0, s.records.length - s.windowSize);
  if (s.followState !== "INSPECTION_HOLD") {
    return tailStart;
  }
  const anchorIndex =
    s.anchorTopId === null ? -1 : s.records.findIndex((row) => row.id === s.anchorTopId);
  // The anchor is always resolvable through this API (records only grow while
  // held); fall back to the tail defensively rather than rendering nothing.
  return anchorIndex === -1 ? tailStart : anchorIndex;
}

/** True when the machine is outside its live half (COLD/HYDRATING). */
function notEstablished(s: TraceStream): boolean {
  return s.followState === "COLD" || s.followState === "HYDRATING";
}

/** Selection survives an operation iff a record with that id still exists. */
function selectionAfter(
  records: readonly TraceStreamRecord[],
  selectedId: string | null,
): string | null {
  if (selectedId === null) {
    return null;
  }
  return records.some((row) => row.id === selectedId) ? selectedId : null;
}

/**
 * Create a COLD, empty stream. `windowSize` is the maximum number of rows the
 * view shows and must be a positive integer.
 */
export function createTraceStream(windowSize: number): TraceStream {
  if (!Number.isInteger(windowSize) || windowSize < 1) {
    throw new RangeError(
      `trace-stream: windowSize must be a positive integer, got ${String(windowSize)}`,
    );
  }
  return freezeState("COLD", 0, windowSize, null, null, Object.freeze([]));
}

/**
 * Hydration-completion event (explicit; the only way out of COLD/HYDRATING).
 * Establishes (or, on reconnect, replaces) the record baseline and resumes
 * live follow at the tail: unseenCount resets, the anchor clears, and the
 * selection survives only if its id still exists in the new baseline.
 */
export function streamHydrated(s: TraceStream, records: readonly TraceStreamRecord[]): TraceStream {
  const admitted = admitRecords(records);
  return freezeState(
    "LIVE_FOLLOW",
    0,
    s.windowSize,
    null,
    selectionAfter(admitted, s.selectedId),
    admitted,
  );
}

/**
 * New arrivals from either source, in arrival order.
 * - LIVE_FOLLOW: the window stays on the tail; unseenCount stays 0.
 * - INSPECTION_HOLD: the visible window is unchanged (no jump to live, no
 *   stolen attention) and unseenCount grows by exactly `records.length`;
 *   the selection survives by identity even when it sits outside the window.
 * - COLD/HYDRATING: identity no-op (no implicit hydration).
 */
export function streamAppend(s: TraceStream, records: readonly TraceStreamRecord[]): TraceStream {
  if (notEstablished(s) || records.length === 0) {
    return s;
  }
  const admitted = admitRecords(records);
  const nextRecords = Object.freeze([...s.records, ...admitted]);
  if (s.followState === "LIVE_FOLLOW") {
    return freezeState("LIVE_FOLLOW", s.unseenCount, s.windowSize, null, s.selectedId, nextRecords);
  }
  return freezeState(
    "INSPECTION_HOLD",
    s.unseenCount + records.length,
    s.windowSize,
    s.anchorTopId,
    s.selectedId,
    nextRecords,
  );
}

/**
 * Meaningful user scroll upward (ICM §16.4 trace.follow.suspend). Suspends
 * follow (LIVE_FOLLOW → INSPECTION_HOLD), moves the window up one row
 * (clamped at the head), and anchors `anchorTopId` to the top row of the
 * resulting window so later arrivals and load-older cannot move it.
 * Identity no-op during COLD/HYDRATING or on an empty stream.
 */
export function streamScrollUp(s: TraceStream): TraceStream {
  if (notEstablished(s) || s.records.length === 0) {
    return s;
  }
  const start = Math.max(0, windowStart(s) - 1);
  const anchor = s.records[start];
  if (anchor === undefined) {
    return s;
  }
  return freezeState(
    "INSPECTION_HOLD",
    s.unseenCount,
    s.windowSize,
    anchor.id,
    s.selectedId,
    s.records,
  );
}

/**
 * Scroll downward while held. Moves the window one row toward the tail; when
 * the window reaches the tail (ICM §16.5), follow resumes: LIVE_FOLLOW,
 * unseenCount reset to 0, anchor cleared. Identity no-op during LIVE_FOLLOW,
 * COLD, or HYDRATING.
 */
export function streamScrollDown(s: TraceStream): TraceStream {
  if (s.followState !== "INSPECTION_HOLD" || s.records.length === 0) {
    return s;
  }
  const maxStart = Math.max(0, s.records.length - s.windowSize);
  const start = Math.min(windowStart(s) + 1, maxStart);
  if (start + s.windowSize >= s.records.length) {
    return freezeState("LIVE_FOLLOW", 0, s.windowSize, null, s.selectedId, s.records);
  }
  const anchor = s.records[start];
  if (anchor === undefined) {
    return s;
  }
  return freezeState(
    "INSPECTION_HOLD",
    s.unseenCount,
    s.windowSize,
    anchor.id,
    s.selectedId,
    s.records,
  );
}

/**
 * Explicit return to live (ICM §16.5 trace.follow.resume — the `↓ N new`
 * affordance, End shortcut, or palette command). The window jumps to the
 * tail, follow resumes, unseenCount resets to 0, the anchor clears, and the
 * selection is preserved by identity. Identity no-op when already live or
 * during COLD/HYDRATING.
 */
export function streamReturnToLive(s: TraceStream): TraceStream {
  if (s.followState !== "INSPECTION_HOLD") {
    return s;
  }
  return freezeState("LIVE_FOLLOW", 0, s.windowSize, null, s.selectedId, s.records);
}

/**
 * Select a row by stable id (ICM §16.1 trace.event.select). Selection never
 * moves the window and never changes the follow state; an id that is not in
 * the stream deselects (null). Selecting a row that later leaves the window
 * keeps the selection: selection is identity, not viewport.
 */
export function streamSelect(s: TraceStream, id: string): TraceStream {
  const selectedId = s.records.some((row) => row.id === id) ? id : null;
  if (selectedId === s.selectedId) {
    return s;
  }
  return freezeState(
    s.followState,
    s.unseenCount,
    s.windowSize,
    s.anchorTopId,
    selectedId,
    s.records,
  );
}

/**
 * Prepend older history (ICM §16.6 trace.history.load_older). The batch keeps
 * its own order and lands ahead of everything loaded so far; the anchor row
 * stays the visible top of a held window (anchor stability), the follow state
 * and unseenCount are unchanged, and the selection survives. Identity no-op
 * during COLD/HYDRATING or for an empty batch.
 */
export function streamLoadOlder(s: TraceStream, older: readonly TraceStreamRecord[]): TraceStream {
  if (notEstablished(s) || older.length === 0) {
    return s;
  }
  const admitted = admitRecords(older);
  const nextRecords = Object.freeze([...admitted, ...s.records]);
  return freezeState(
    s.followState,
    s.unseenCount,
    s.windowSize,
    s.anchorTopId,
    s.selectedId,
    nextRecords,
  );
}

/**
 * The current window rows (derived): the tail in LIVE_FOLLOW, the anchored
 * range in INSPECTION_HOLD, empty in COLD/HYDRATING. Returned frozen.
 */
export function streamVisibleWindow(s: TraceStream): readonly TraceStreamRecord[] {
  if (notEstablished(s) || s.records.length === 0) {
    return Object.freeze([]);
  }
  return Object.freeze(s.records.slice(windowStart(s), windowStart(s) + s.windowSize));
}
