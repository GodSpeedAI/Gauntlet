import { describe, expect, it } from "bun:test";
import { FocusRef } from "./focus";
import {
  canGoBack,
  canGoForward,
  createLocationHistory,
  currentLocation,
  LOCATION_HISTORY_MAX_ENTRIES,
  locationBack,
  locationForward,
  locationPush,
  type SemanticLocation,
} from "./navigation-history";
import type { SemanticLocation as ShellSemanticLocation } from "./shell";

/**
 * Increment 4 remainder: back/forward navigation history over semantic
 * locations (ICM §6.6 `nav.history.back`/`nav.history.forward`).
 *
 * Frozen contract:
 *   - ICM §6.6: back/forward are NAVIGATION_LOCAL — they "Restore: Case;
 *     Horizon; View; Focus; optional layout state" and "Does not revert domain
 *     time". ICM §49 pins the teeth: "Back navigation | canonical world
 *     unchanged".
 *   - DESIGN.md "Semantic location / route-like addressing": a semantic
 *     location "identifies a reproducible projection/horizon. It does not
 *     create canonical domain state."
 *
 * Invariants under test (all RED-first against an absent module):
 *   H1. back/forward move ONLY the cursor; entries are never rewritten
 *       (proven at reference identity: the entries array is shared).
 *   H2. push after back truncates the forward branch (browser semantics).
 *   H3. consecutive duplicate pushes are no-ops (same history reference).
 *   H4. the stack is bounded at 64 with the current location always retained.
 *   H5. the cursor is always a valid index; currentLocation never null.
 *   H6. purity: caller location objects and prior histories are never
 *       mutated; only frozen values are returned.
 */

const WS = { workspace: "journey-integrity", reference: "1d457ec9" } as const;

/** Minimal narrow-shape location (distinct object identity per call). */
function loc(view: string, focusId?: string): SemanticLocation {
  const base: SemanticLocation = { workspace: { ...WS }, view };
  return focusId === undefined ? base : { ...base, focus: { id: focusId } };
}

/** Location in the shell's REAL required-and-nullable shape (shell.ts:134). */
function shellLoc(focus: FocusRef | null, horizon: FocusRef | null): ShellSemanticLocation {
  return {
    workspace: { ...WS, run: "R17" },
    horizon,
    surface: "Primary",
    view: "trace",
    focus,
  };
}

/** Test-local structural equality (independent of the module's implementation). */
function deepEqual(a: unknown, b: unknown): boolean {
  if (Object.is(a, b)) return true;
  if (Array.isArray(a) || Array.isArray(b)) {
    if (!Array.isArray(a) || !Array.isArray(b) || a.length !== b.length) return false;
    return a.every((v, i) => deepEqual(v, b[i]));
  }
  if (typeof a !== "object" || typeof b !== "object" || a === null || b === null) return false;
  const ka = Object.keys(a) as string[];
  const kb = Object.keys(b) as string[];
  if (ka.length !== kb.length) return false;
  const ra = a as Record<string, unknown>;
  const rb = b as Record<string, unknown>;
  return ka.every((k) => k in rb && deepEqual(ra[k], rb[k]));
}

/** H5 + freeze posture: valid cursor, non-null current, frozen history. */
function assertWellFormed(h: ReturnType<typeof createLocationHistory>): void {
  expect(Number.isInteger(h.cursor)).toBe(true);
  expect(h.cursor).toBeGreaterThanOrEqual(0);
  expect(h.cursor).toBeLessThan(h.entries.length);
  expect(currentLocation(h)).not.toBeNull();
  expect(Object.isFrozen(h)).toBe(true);
  expect(Object.isFrozen(h.entries)).toBe(true);
}

describe("Increment 4 navigation history (ICM §6.6 back/forward, NAVIGATION_LOCAL)", () => {
  it("createLocationHistory: one entry, cursor 0, current is the initial location, no back/forward", () => {
    const initial = loc("work", "unit:T23");
    const h = createLocationHistory(initial);
    expect(h.entries.length).toBe(1);
    expect(h.cursor).toBe(0);
    expect(currentLocation(h)).toBe(initial);
    expect(canGoBack(h)).toBe(false);
    expect(canGoForward(h)).toBe(false);
    assertWellFormed(h);
  });

  it("push appends and moves the cursor; back/forward traverse entries without rewriting them (H1)", () => {
    const a = loc("work");
    const b = loc("trace");
    const c = loc("evidence", "evidence:EV-991");
    let h = createLocationHistory(a);
    h = locationPush(h, b);
    h = locationPush(h, c);
    expect(h.entries.length).toBe(3);
    expect(currentLocation(h)).toBe(c);
    expect(canGoBack(h)).toBe(true);
    expect(canGoForward(h)).toBe(false);

    // H1: entries are the very references pushed, and the array reference is
    // shared across back/forward — the stack content is never rewritten.
    const entriesBefore = h.entries;
    h = locationBack(h);
    expect(h.entries).toBe(entriesBefore);
    expect(currentLocation(h)).toBe(b);
    h = locationBack(h);
    expect(h.entries).toBe(entriesBefore);
    expect(currentLocation(h)).toBe(a);
    h = locationForward(h);
    expect(h.entries).toBe(entriesBefore);
    expect(currentLocation(h)).toBe(b);
    expect(h.entries[0]).toBe(a);
    expect(h.entries[1]).toBe(b);
    expect(h.entries[2]).toBe(c);
    assertWellFormed(h);
  });

  it("consecutive duplicate push is a no-op returning the SAME history (H3): identical reference, deep-equal object, interned FocusRef", () => {
    const a = loc("work");
    let h = createLocationHistory(a);
    h = locationPush(h, loc("chat"));
    const afterChat = h;

    // Same reference.
    expect(locationPush(h, currentLocation(h) as SemanticLocation)).toBe(h);
    // Structurally equal but distinct object identities (fresh {workspace} copy).
    expect(locationPush(h, loc("chat"))).toBe(h);
    expect(h).toBe(afterChat); // nothing was appended by either duplicate push
    expect(h.entries.length).toBe(2);

    // Interned FocusRef identity (FocusRef.for mints one identity per triple).
    const ref = FocusRef.for("gauntlet", "WorkItem", "u-root");
    h = locationPush(h, { workspace: { ...WS }, view: "agents", focus: ref });
    const afterRef = h;
    expect(h.entries.length).toBe(3);
    // Deep-equal non-interned focus clone (structural equality, not Object.is).
    expect(locationPush(h, { workspace: { ...WS }, view: "agents", focus: { ...ref } })).toBe(
      afterRef,
    );
    expect(h).toBe(afterRef);
    assertWellFormed(h);
  });

  it("locations differing in any single field are NOT deduped; non-consecutive repeats are kept (browser semantics)", () => {
    const base = loc("work", "unit:T23");
    const variants = [
      {
        workspace: { workspace: "other-case", reference: "1d457ec9" },
        view: "work",
        focus: { id: "unit:T23" },
      },
      { workspace: { ...WS }, view: "chat", focus: { id: "unit:T23" } },
      { workspace: { ...WS }, view: "work", focus: { id: "unit:T24" } },
      {
        workspace: { ...WS },
        view: "work",
        focus: { id: "unit:T23" },
        horizon: { id: "focus:gauntlet:WorkItem:u-run" },
      },
      { workspace: { ...WS }, view: "work", focus: { id: "unit:T23" }, surface: "Inspector" },
    ] as const satisfies readonly SemanticLocation[];
    let h = createLocationHistory(base);
    for (const v of variants) {
      h = locationPush(h, v);
    }
    expect(h.entries.length).toBe(1 + variants.length);

    // A → B → A: the repeat of A is a NEW visit, not a dedupe.
    h = createLocationHistory(loc("work"));
    h = locationPush(h, loc("trace"));
    h = locationPush(h, loc("work"));
    expect(h.entries.length).toBe(3);
    expect(currentLocation(h)).toEqual(loc("work"));
    assertWellFormed(h);
  });

  it("push after back truncates the forward branch (H2)", () => {
    const a = loc("work");
    const b = loc("trace");
    const c = loc("evidence");
    let h = createLocationHistory(a);
    h = locationPush(h, b);
    h = locationPush(h, c);
    h = locationBack(h);
    h = locationBack(h);
    expect(currentLocation(h)).toBe(a);
    expect(canGoForward(h)).toBe(true);

    h = locationPush(h, loc("agents"));
    expect(h.entries.length).toBe(2); // [a, agents] — b and c are gone
    expect(currentLocation(h)).toEqual(loc("agents"));
    expect(canGoForward(h)).toBe(false);
    expect(locationForward(h)).toBe(h); // forward at the new tip is a no-op
    // The truncated entries are unreachable from the new stack.
    expect(h.entries).not.toContain(b);
    expect(h.entries).not.toContain(c);
    assertWellFormed(h);
  });

  it("back at the origin and forward at the tip are no-ops returning the SAME history (H1/H3)", () => {
    const h = createLocationHistory(loc("work"));
    expect(locationBack(h)).toBe(h);
    expect(locationForward(h)).toBe(h);

    const h2 = locationPush(h, loc("trace"));
    expect(locationForward(h2)).toBe(h2); // already at the tip
    const atOrigin = locationBack(h2);
    expect(locationBack(atOrigin)).toBe(atOrigin); // second back at origin is identity
    assertWellFormed(h2);
    assertWellFormed(atOrigin);
  });

  it("back then forward round-trips to the same location and full history (perceptual reproduction)", () => {
    let h = createLocationHistory(loc("work"));
    h = locationPush(h, loc("chat"));
    h = locationPush(h, loc("trace"));
    const original = h;
    h = locationBack(h);
    h = locationForward(h);
    expect(h).not.toBe(original); // a new history value...
    expect(h.cursor).toBe(original.cursor); // ...at the same perceptual location
    expect(h.entries).toBe(original.entries);
    expect(currentLocation(h)).toEqual(loc("trace"));
    assertWellFormed(h);
  });

  it("bound: the stack is capped at 64 entries, oldest dropped, current location always retained (H4)", () => {
    expect(LOCATION_HISTORY_MAX_ENTRIES).toBe(64); // the frozen bound itself

    // Exactly at the bound: initial + 63 pushes = 64 entries, nothing dropped.
    let h = createLocationHistory(loc("v-0"));
    for (let i = 1; i <= 63; i++) {
      h = locationPush(h, loc(`v-${i}`));
      assertWellFormed(h);
    }
    expect(h.entries.length).toBe(64);
    expect(h.entries[0]).toEqual(loc("v-0"));
    expect(h.cursor).toBe(63);

    // One past the bound: exactly the oldest is dropped.
    h = locationPush(h, loc("v-64"));
    expect(h.entries.length).toBe(64);
    expect(h.entries[0]).toEqual(loc("v-1")); // v-0 dropped
    expect(currentLocation(h)).toEqual(loc("v-64")); // current always retained
    expect(h.cursor).toBe(63);
    expect(canGoForward(h)).toBe(false);
    expect(canGoBack(h)).toBe(true);

    // Far past the bound: 71 visits total, last 64 kept.
    let big = createLocationHistory(loc("v-0"));
    for (let i = 1; i <= 70; i++) {
      big = locationPush(big, loc(`v-${i}`));
    }
    expect(big.entries.length).toBe(64);
    expect(big.entries[0]).toEqual(loc("v-7"));
    expect(big.entries[63]).toEqual(loc("v-70"));
    expect(currentLocation(big)).toEqual(loc("v-70"));
    expect(big.cursor).toBe(63);
    assertWellFormed(big);

    // Bound interacts with truncation: back far, push, still capped at 64.
    let mixed = big;
    for (let i = 0; i < 63; i++) {
      mixed = locationBack(mixed);
    }
    expect(mixed.cursor).toBe(0);
    mixed = locationPush(mixed, loc("v-new"));
    expect(mixed.entries.length).toBe(2); // forward branch truncated to [v-7, v-new]
    assertWellFormed(mixed);
  });

  it("cursor stays valid and histories stay frozen across a deterministic mixed sequence (H5/H6)", () => {
    let h = createLocationHistory(loc("work"));
    assertWellFormed(h);
    const views = [
      "chat",
      "trace",
      "evidence",
      "agents",
      "attention",
      "files",
      "kernel",
      "problems",
      "terminal",
      "domain",
    ];
    for (const v of views) {
      h = locationPush(h, loc(v));
      assertWellFormed(h);
    }
    for (let i = 0; i < 4; i++) {
      h = locationBack(h);
      assertWellFormed(h);
    }
    h = locationForward(h);
    assertWellFormed(h);
    h = locationPush(h, loc("work")); // truncates the forward branch
    assertWellFormed(h);
    h = locationBack(h);
    h = locationBack(h);
    assertWellFormed(h);
    // 9 entries [work, chat, trace, evidence, agents, attention, files, kernel, work];
    // two backs from the tip land on cursor 6 ("files") with forward available.
    expect(currentLocation(h)).toEqual(loc("files"));
    expect(canGoBack(h)).toBe(true);
    expect(canGoForward(h)).toBe(true);
    h = locationForward(h);
    h = locationForward(h);
    expect(currentLocation(h)).toEqual(loc("work"));
    expect(canGoForward(h)).toBe(false); // at the tip again
  });

  it("purity (H6/ICM §49 'Back navigation — canonical world unchanged'): stored entries are the pushed references; caller locations and prior histories are never mutated or frozen", () => {
    const a = loc("work", "unit:T1");
    const b = loc("trace", "unit:T2");
    const c = loc("evidence", "unit:T3");
    const originals = [a, b, c];
    const clones = originals.map((o) => ({ ...o, workspace: { ...(o.workspace as object) } }));

    let h = createLocationHistory(a);
    h = locationPush(h, b);
    h = locationPush(h, c);
    // Entries are the exact caller references: the module never copies,
    // rewrites, or replaces stored locations (H1).
    expect(h.entries[0]).toBe(a);
    expect(h.entries[1]).toBe(b);
    expect(h.entries[2]).toBe(c);

    const snapshotCursor = h.cursor;
    const snapshotEntries = h.entries;
    const h2 = locationBack(h);
    const h3 = locationBack(h2);
    const h4 = locationPush(h3, loc("agents"));

    // Prior histories are immutable observations (frozen; unchanged after
    // later operations derived new values from them).
    expect(h.cursor).toBe(snapshotCursor);
    expect(h.entries).toBe(snapshotEntries);
    expect(currentLocation(h)).toBe(c);
    expect(h2.entries.length).toBe(3);
    expect(h4.entries.length).toBe(2);

    // The module never mutates or freezes caller-owned location state.
    for (const [i, o] of originals.entries()) {
      expect(Object.isFrozen(o)).toBe(false);
      expect(deepEqual(o, clones[i])).toBe(true);
    }
    expect(Object.isFrozen(h)).toBe(true);
    expect(Object.isFrozen(h4)).toBe(true);
  });

  it("structural compatibility: the shell's REAL SemanticLocation shape flows through the whole API (documented narrow-supertype admission)", () => {
    // Type-level evidence (verified by `bun run check`): ShellSemanticLocation
    // (workspace: WorkspaceRef, horizon: FocusRef | null, surface: SurfaceId,
    // view: ViewId, focus: FocusRef | null — shell.ts:134) satisfies this
    // module's narrow {workspace: unknown; horizon?; surface?: string;
    // view?: string; focus?} location type.
    const shellA = shellLoc(FocusRef.for("gauntlet", "WorkItem", "u-root"), null);
    const shellB = shellLoc(null, FocusRef.for("gauntlet", "Run", "R17"));
    const narrowAdmission: SemanticLocation = shellA;

    let h = createLocationHistory(narrowAdmission);
    h = locationPush(h, shellB);
    expect(h.entries.length).toBe(2);
    h = locationBack(h);
    expect(currentLocation(h)).toBe(shellA);
    expect(canGoForward(h)).toBe(true);
    h = locationForward(h);
    expect(currentLocation(h)).toBe(shellB);
    assertWellFormed(h);
  });

  it("null and absent mean the same perceptual location (shell required-nullable vs DESIGN optional shapes dedupe)", () => {
    // shell.ts shape: horizon/focus REQUIRED but nullable.
    const shellNullFocus: ShellSemanticLocation = {
      workspace: { workspace: "journey-integrity", reference: "1d457ec9", run: "R17" },
      horizon: null,
      surface: "Primary",
      view: "trace",
      focus: null,
    };
    // DESIGN.md conceptual shape: the same perceptual location with the empty
    // fields simply omitted. Consecutive push must dedupe across conventions.
    const designOmitted: SemanticLocation = {
      workspace: { workspace: "journey-integrity", reference: "1d457ec9", run: "R17" },
      surface: "Primary",
      view: "trace",
    };

    let h = createLocationHistory(shellNullFocus);
    expect(locationPush(h, designOmitted)).toBe(h); // no-op dedupe
    expect(h.entries.length).toBe(1);

    // A present focus is NOT the same location as an absent one.
    const withFocus: ShellSemanticLocation = {
      workspace: { workspace: "journey-integrity", reference: "1d457ec9", run: "R17" },
      horizon: null,
      surface: "Primary",
      view: "trace",
      focus: FocusRef.for("gauntlet", "Evidence", "EV-991"),
    };
    h = locationPush(h, withFocus);
    expect(h.entries.length).toBe(2);
    assertWellFormed(h);
  });
});
