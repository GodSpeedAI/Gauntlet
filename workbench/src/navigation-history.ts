/**
 * Pure back/forward navigation history over semantic locations.
 *
 * Frozen contract:
 *   - ICM (`.agents/specs/tui/INTERACTION_COMMAND_MAP.md`) §6.6
 *     `nav.history.back` / `nav.history.forward`: class NAVIGATION_LOCAL,
 *     binding LOCAL_DEFINED. Back/forward "Restore: Case; Horizon; View;
 *     Focus; optional layout state" and "Does not revert domain time" — this
 *     module therefore stores and reproduces perceptual locations and owns NO
 *     domain state whatsoever (ICM §2.2: navigation changes current
 *     perception/location without mutating domain truth; ICM §49 teeth row:
 *     "Back navigation | canonical world unchanged").
 *   - DESIGN.md "Semantic location / route-like addressing": a semantic
 *     location "identifies a reproducible projection/horizon. It does not
 *     create canonical domain state." Entries are addresses, not mutations.
 *
 * Core invariants (all pinned test-first in navigation-history.test.ts):
 *   H1. back/forward move ONLY the cursor; entries are never rewritten — the
 *       returned history SHARES the previous `entries` array reference.
 *   H2. push after back truncates the forward branch (standard browser
 *       semantics).
 *   H3. consecutive duplicate pushes are no-ops returning the SAME history.
 *   H4. the stack is bounded at {@link LOCATION_HISTORY_MAX_ENTRIES} (oldest
 *       dropped) and the current location is always retained.
 *   H5. the cursor is always a valid index into `entries`.
 *   H6. purity: no input is mutated; every returned value is frozen. Caller
 *       location objects are stored BY REFERENCE and are never frozen or
 *       rewritten by this module.
 *
 * Equality is structural (deep, own enumerable string keys) so interned
 * `FocusRef` identities hit an `Object.is` fast path and plain-object
 * locations compare by content. Location values are assumed acyclic — the
 * DESIGN.md location shape is flat strings/refs.
 */

/**
 * Narrow structural location shape for this module. It is deliberately a
 * structural SUPERtype of the real renderer-independent `SemanticLocation`
 * exported by `workbench/src/shell.ts` ({workspace: WorkspaceRef;
 * horizon: FocusRef | null; surface: SurfaceId; view: ViewId;
 * focus: FocusRef | null}) and of the conceptual shape in DESIGN.md
 * ("Semantic location / route-like addressing", `{workspace: CaseRef;
 * horizon?: FocusRef; surface?: SurfaceId; view?: ViewId; focus?: FocusRef}`):
 * every field of the real type is assignable to the field below, so real
 * shell locations are admitted WITHOUT this module importing shell.ts (the
 * orchestrator wires the two; navigation stays renderer- and shell-agnostic).
 */
export interface SemanticLocation {
  readonly workspace: unknown;
  readonly horizon?: unknown;
  readonly surface?: string;
  readonly view?: string;
  readonly focus?: unknown;
}

/** Immutable back/forward stack: visited locations plus the current cursor. */
export interface LocationHistory {
  readonly entries: readonly SemanticLocation[];
  readonly cursor: number;
}

/** Maximum retained entries; older entries are dropped, never the current one (H4). */
export const LOCATION_HISTORY_MAX_ENTRIES = 64;

function freezeHistory(entries: readonly SemanticLocation[], cursor: number): LocationHistory {
  return Object.freeze({ entries: Object.freeze(entries), cursor });
}

/** `null` and `undefined` both mean "no value" for optional location fields. */
function isEmpty(value: unknown): value is null | undefined {
  return value === null || value === undefined;
}

function deepEqual(a: unknown, b: unknown): boolean {
  if (Object.is(a, b)) return true;
  if (typeof a !== "object" || typeof b !== "object" || a === null || b === null) return false;
  if (Array.isArray(a) || Array.isArray(b)) {
    if (!Array.isArray(a) || !Array.isArray(b) || a.length !== b.length) return false;
    return a.every((value, index) => deepEqual(value, b[index]));
  }
  const recordA = a as Record<string, unknown>;
  const recordB = b as Record<string, unknown>;
  const keysA = Object.keys(recordA);
  const keysB = Object.keys(recordB);
  return (
    keysA.length === keysB.length &&
    keysA.every((key) => key in recordB && deepEqual(recordA[key], recordB[key]))
  );
}

/**
 * Structural location equality over the union of own enumerable keys, so
 * extra fields (e.g. ICM §6.6 "optional layout state") participate in dedupe.
 * Per-field, `null` and absent are the same perceptual location: the shell's
 * required-nullable shape (`focus: null`) and DESIGN.md's optional shape
 * (field omitted) dedupe against each other.
 */
function locationsEqual(a: SemanticLocation, b: SemanticLocation): boolean {
  const recordA = a as unknown as Record<string, unknown>;
  const recordB = b as unknown as Record<string, unknown>;
  const keys = new Set([...Object.keys(recordA), ...Object.keys(recordB)]);
  for (const key of keys) {
    const va = recordA[key];
    const vb = recordB[key];
    if (isEmpty(va) && isEmpty(vb)) continue;
    if (isEmpty(va) || isEmpty(vb)) return false;
    if (!deepEqual(va, vb)) return false;
  }
  return true;
}

/** Seed the history with one location: one entry, cursor 0. */
export function createLocationHistory(initial: SemanticLocation): LocationHistory {
  return freezeHistory([initial], 0);
}

/**
 * Record a visit (ICM §6.6 back/forward support; NAVIGATION_LOCAL).
 *
 * Consecutive duplicates are no-ops returning `h` unchanged (H3). Any forward
 * branch is truncated first (H2), the location is appended, and the cursor
 * lands on it. Bounds: at most {@link LOCATION_HISTORY_MAX_ENTRIES} entries,
 * oldest dropped, current always retained (H4). Pure: `h` and `location` are
 * never mutated (H6).
 */
export function locationPush(h: LocationHistory, location: SemanticLocation): LocationHistory {
  const current = h.entries[h.cursor];
  if (current !== undefined && locationsEqual(current, location)) {
    return h;
  }
  const appended = [...h.entries.slice(0, h.cursor + 1), location];
  const bounded =
    appended.length > LOCATION_HISTORY_MAX_ENTRIES
      ? appended.slice(appended.length - LOCATION_HISTORY_MAX_ENTRIES)
      : appended;
  return freezeHistory(bounded, bounded.length - 1);
}

/**
 * `nav.history.back` (ICM §6.6): cursor − 1 if possible, else `h` unchanged.
 * Entries are never rewritten — the result shares `h.entries` (H1). Moving
 * the cursor reproduces a perceptual location; it never reverts domain time.
 */
export function locationBack(h: LocationHistory): LocationHistory {
  if (h.cursor <= 0) return h;
  return freezeHistory(h.entries, h.cursor - 1);
}

/**
 * `nav.history.forward` (ICM §6.6): cursor + 1 if possible, else `h`
 * unchanged. Shares `h.entries` (H1). Pure (H6).
 */
export function locationForward(h: LocationHistory): LocationHistory {
  if (h.cursor >= h.entries.length - 1) return h;
  return freezeHistory(h.entries, h.cursor + 1);
}

/** The location under the cursor, or null only for a malformed empty history (H5). */
export function currentLocation(h: LocationHistory): SemanticLocation | null {
  return h.entries[h.cursor] ?? null;
}

/** Whether `nav.history.back` would move the cursor (ICM §6.6). */
export function canGoBack(h: LocationHistory): boolean {
  return h.cursor > 0;
}

/** Whether `nav.history.forward` would move the cursor (ICM §6.6). */
export function canGoForward(h: LocationHistory): boolean {
  return h.cursor < h.entries.length - 1;
}
