import { describe, expect, it } from "bun:test";
import type { TraceStream, TraceStreamRecord } from "./trace-stream";
import {
  createTraceStream,
  streamAppend,
  streamHydrated,
  streamLoadOlder,
  streamReturnToLive,
  streamScrollDown,
  streamScrollUp,
  streamSelect,
  streamVisibleWindow,
} from "./trace-stream";

/**
 * GUI-redesign increment 6 (core): the TraceStream presentation state
 * machine (UESC §16.2). These tests pin the follow/hold/unseen/load-older
 * contract the Trace view will consume, including the adversarial tooth:
 * "Trace does not jump to live tail while the user inspects history."
 *
 * Pinned invariants:
 * (a) append during INSPECTION_HOLD never changes the visible window;
 * (b) unseenCount is exact;
 * (c) selection survives appends and window moves (identity-based);
 * (d) loadOlder preserves the anchor row's visibility;
 * (e) reaching the tail via scrollDown returns to LIVE_FOLLOW;
 * (f) sources never merge: same id from different sources = two records;
 * (g) COLD/HYDRATING transitions are explicit only.
 */

function g(id: string, ordinal?: number): TraceStreamRecord {
  return ordinal === undefined ? { id, source: "gauntlet" } : { id, source: "gauntlet", ordinal };
}

function r(id: string, ordinal?: number): TraceStreamRecord {
  return ordinal === undefined
    ? { id, source: "realitytrace" }
    : { id, source: "realitytrace", ordinal };
}

function ids(records: readonly TraceStreamRecord[]): string[] {
  return records.map((record) => record.id);
}

/** Hand-built HYDRATING state, as the future reconnect wiring will enter it. */
function asHydrating(s: TraceStream): TraceStream {
  return Object.freeze({ ...s, followState: "HYDRATING" as const });
}

function liveStream(records: readonly TraceStreamRecord[], windowSize = 3): TraceStream {
  return streamHydrated(createTraceStream(windowSize), records);
}

describe("createTraceStream", () => {
  it("starts COLD, empty, unseen 0, no anchor, no selection", () => {
    const s = createTraceStream(3);
    expect(s.followState).toBe("COLD");
    expect(s.unseenCount).toBe(0);
    expect(s.windowSize).toBe(3);
    expect(s.anchorTopId).toBeNull();
    expect(s.selectedId).toBeNull();
    expect(s.records).toEqual([]);
    expect(streamVisibleWindow(s)).toEqual([]);
  });

  it("rejects non-positive or non-integer window sizes", () => {
    expect(() => createTraceStream(0)).toThrow(RangeError);
    expect(() => createTraceStream(-1)).toThrow(RangeError);
    expect(() => createTraceStream(2.5)).toThrow(RangeError);
    expect(() => createTraceStream(Number.NaN)).toThrow(RangeError);
  });

  it("returns a frozen state with frozen records", () => {
    const s = createTraceStream(2);
    expect(Object.isFrozen(s)).toBe(true);
    expect(Object.isFrozen(s.records)).toBe(true);
    expect(() => {
      (s as { unseenCount: number }).unseenCount = 5;
    }).toThrow();
  });
});

describe("streamHydrated", () => {
  it("COLD -> LIVE_FOLLOW with the baseline, unseen reset, no anchor", () => {
    const s = streamHydrated(createTraceStream(3), [g("a"), g("b"), g("c"), g("d")]);
    expect(s.followState).toBe("LIVE_FOLLOW");
    expect(ids(s.records)).toEqual(["a", "b", "c", "d"]);
    expect(s.unseenCount).toBe(0);
    expect(s.anchorTopId).toBeNull();
    expect(ids(streamVisibleWindow(s))).toEqual(["b", "c", "d"]);
  });

  it("an empty baseline still establishes LIVE_FOLLOW", () => {
    const s = streamHydrated(createTraceStream(3), []);
    expect(s.followState).toBe("LIVE_FOLLOW");
    expect(s.records).toEqual([]);
    expect(streamVisibleWindow(s)).toEqual([]);
  });

  it("HYDRATING -> LIVE_FOLLOW after first hydration", () => {
    const s = streamHydrated(asHydrating(createTraceStream(3)), [g("a"), g("b")]);
    expect(s.followState).toBe("LIVE_FOLLOW");
    expect(ids(s.records)).toEqual(["a", "b"]);
  });

  it("replaces content on re-hydration: selection survives only by identity", () => {
    const s = streamSelect(liveStream([g("a"), g("b"), g("c")]), "a");
    const kept = streamHydrated(s, [g("a"), g("x"), g("y")]);
    expect(kept.selectedId).toBe("a");
    const gone = streamHydrated(kept, [g("x"), g("y"), g("z")]);
    expect(gone.selectedId).toBeNull();
  });

  it("resets unseenCount and returns to the tail even from a deep hold", () => {
    let s = liveStream([g("a"), g("b"), g("c"), g("d"), g("e")]);
    s = streamScrollUp(s);
    s = streamScrollUp(s);
    s = streamAppend(s, [g("f"), g("g")]);
    expect(s.followState).toBe("INSPECTION_HOLD");
    expect(s.unseenCount).toBe(2);
    const rehydrated = streamHydrated(s, [g("1"), g("2"), g("3")]);
    expect(rehydrated.followState).toBe("LIVE_FOLLOW");
    expect(rehydrated.unseenCount).toBe(0);
    expect(rehydrated.anchorTopId).toBeNull();
    expect(ids(streamVisibleWindow(rehydrated))).toEqual(["1", "2", "3"]);
  });

  it("freezes admitted records and ignores later mutation of the input batch", () => {
    const batch = [g("a"), g("b")];
    const s = streamHydrated(createTraceStream(3), batch);
    const first = batch[0] as { id: string } | undefined;
    batch.push(g("c"));
    if (first !== undefined) {
      first.id = "mutated";
    }
    expect(ids(s.records)).toEqual(["a", "b"]);
    expect(Object.isFrozen(s.records)).toBe(true);
    for (const record of s.records) {
      expect(Object.isFrozen(record)).toBe(true);
    }
  });

  it("admits only the two labelled sources", () => {
    expect(() =>
      streamHydrated(createTraceStream(2), [{ id: "x", source: "chat" as never }]),
    ).toThrow(TypeError);
  });
});

describe("streamAppend during LIVE_FOLLOW", () => {
  it("keeps the window on the tail and unseenCount at 0", () => {
    let s = liveStream([g("a"), g("b"), g("c")]);
    s = streamAppend(s, [g("d"), g("e")]);
    expect(s.followState).toBe("LIVE_FOLLOW");
    expect(s.unseenCount).toBe(0);
    expect(ids(streamVisibleWindow(s))).toEqual(["c", "d", "e"]);
  });

  it("is an identity no-op for an empty batch", () => {
    const s = liveStream([g("a")]);
    expect(streamAppend(s, [])).toBe(s);
  });
});

describe("streamAppend during INSPECTION_HOLD (invariants a, b, c)", () => {
  it("(a) never changes the visible window and never steals attention", () => {
    let s = liveStream([g("1"), g("2"), g("3"), g("4"), g("5"), g("6")], 3);
    s = streamScrollUp(s);
    const heldWindow = ids(streamVisibleWindow(s));
    expect(heldWindow).toEqual(["3", "4", "5"]);
    s = streamAppend(s, [g("7")]);
    s = streamAppend(s, [g("8"), g("9"), g("10")]);
    expect(s.followState).toBe("INSPECTION_HOLD");
    expect(ids(streamVisibleWindow(s))).toEqual(heldWindow);
    expect(s.anchorTopId).toBe("3");
  });

  it("(b) unseenCount is exact across batches from both sources", () => {
    let s = liveStream([g("1"), g("2"), g("3"), g("4"), g("5"), g("6")], 3);
    s = streamScrollUp(s);
    s = streamAppend(s, [r("s1")]);
    expect(s.unseenCount).toBe(1);
    s = streamAppend(s, [g("7"), r("s2"), g("8")]);
    expect(s.unseenCount).toBe(4);
    s = streamAppend(s, []);
    expect(s.unseenCount).toBe(4);
  });

  it("(c) selection survives appends even when the row leaves the window", () => {
    let s = liveStream([g("1"), g("2"), g("3"), g("4"), g("5"), g("6")], 3);
    s = streamSelect(s, "2");
    s = streamScrollUp(s);
    s = streamAppend(s, [g("7"), g("8"), g("9")]);
    expect(streamVisibleWindow(s).some((row) => row.id === "2")).toBe(false);
    expect(s.selectedId).toBe("2");
  });

  it("(c) selection survives tail-window turnover during live append", () => {
    let s = liveStream([g("1"), g("2"), g("3")], 2);
    s = streamSelect(s, "1");
    s = streamAppend(s, [g("4"), g("5")]);
    expect(ids(streamVisibleWindow(s))).toEqual(["4", "5"]);
    expect(s.selectedId).toBe("1");
  });
});

describe("streamAppend outside the live machine (invariant g)", () => {
  it("is an identity no-op during COLD", () => {
    const s = createTraceStream(3);
    expect(streamAppend(s, [g("a")])).toBe(s);
  });

  it("is an identity no-op during HYDRATING", () => {
    const s = asHydrating(createTraceStream(3));
    expect(streamAppend(s, [g("a")])).toBe(s);
  });
});

describe("streamScrollUp", () => {
  it("suspends follow and anchors the top row of the resulting window", () => {
    const s = liveStream([g("1"), g("2"), g("3"), g("4"), g("5")], 3);
    const held = streamScrollUp(s);
    expect(held.followState).toBe("INSPECTION_HOLD");
    expect(ids(streamVisibleWindow(held))).toEqual(["2", "3", "4"]);
    expect(held.anchorTopId).toBe("2");
    expect(held.selectedId).toBeNull();
    expect(held.unseenCount).toBe(0);
  });

  it("keeps scrolling up one row per call and clamps at the head", () => {
    let s = liveStream([g("1"), g("2"), g("3"), g("4"), g("5")], 3);
    s = streamScrollUp(s);
    s = streamScrollUp(s);
    expect(ids(streamVisibleWindow(s))).toEqual(["1", "2", "3"]);
    expect(s.anchorTopId).toBe("1");
    const clamped = streamScrollUp(s);
    expect(ids(streamVisibleWindow(clamped))).toEqual(["1", "2", "3"]);
    expect(clamped.anchorTopId).toBe("1");
    expect(clamped.followState).toBe("INSPECTION_HOLD");
  });

  it("preserves selection and does not touch unseenCount", () => {
    let s = streamSelect(liveStream([g("1"), g("2"), g("3"), g("4")], 2), "1");
    s = streamScrollUp(s);
    expect(s.selectedId).toBe("1");
    expect(s.unseenCount).toBe(0);
  });

  it("is an identity no-op on empty records", () => {
    const s = streamHydrated(createTraceStream(3), []);
    expect(streamScrollUp(s)).toBe(s);
  });

  it("is an identity no-op during COLD and HYDRATING", () => {
    const cold = createTraceStream(3);
    expect(streamScrollUp(cold)).toBe(cold);
    const hydrating = asHydrating(cold);
    expect(streamScrollUp(hydrating)).toBe(hydrating);
  });
});

describe("streamScrollDown (invariant e)", () => {
  it("moves the window toward the tail one row per call", () => {
    let s = liveStream([g("1"), g("2"), g("3"), g("4"), g("5"), g("6"), g("7")], 3);
    s = streamScrollUp(s);
    s = streamScrollUp(s);
    // tail ["5","6","7"], two rows up: window ["3","4","5"]
    expect(ids(streamVisibleWindow(s))).toEqual(["3", "4", "5"]);
    const down = streamScrollDown(s);
    expect(ids(streamVisibleWindow(down))).toEqual(["4", "5", "6"]);
    expect(down.followState).toBe("INSPECTION_HOLD");
  });

  it("(e) returns to LIVE_FOLLOW with unseenCount reset when the window reaches the tail", () => {
    let s = liveStream([g("1"), g("2"), g("3"), g("4"), g("5"), g("6")], 3);
    s = streamScrollUp(s);
    s = streamScrollUp(s);
    s = streamScrollUp(s);
    s = streamAppend(s, [g("7"), g("8")]);
    // 8 records, window held at the head ["1","2","3"], 2 unseen
    expect(s.followState).toBe("INSPECTION_HOLD");
    expect(s.unseenCount).toBe(2);
    s = streamScrollDown(s);
    expect(s.followState).toBe("INSPECTION_HOLD");
    expect(s.unseenCount).toBe(2);
    s = streamScrollDown(s);
    s = streamScrollDown(s);
    s = streamScrollDown(s);
    expect(s.followState).toBe("INSPECTION_HOLD");
    expect(s.unseenCount).toBe(2);
    // one more row down puts the window on the tail ["6","7","8"]
    s = streamScrollDown(s);
    expect(s.followState).toBe("LIVE_FOLLOW");
    expect(s.unseenCount).toBe(0);
    expect(s.anchorTopId).toBeNull();
    expect(ids(streamVisibleWindow(s))).toEqual(["6", "7", "8"]);
  });

  it("(e) a hold on a stream shorter than the window returns to live immediately", () => {
    let s = liveStream([g("1"), g("2")], 5);
    s = streamScrollUp(s);
    expect(s.followState).toBe("INSPECTION_HOLD");
    const down = streamScrollDown(s);
    expect(down.followState).toBe("LIVE_FOLLOW");
    expect(down.unseenCount).toBe(0);
  });

  it("preserves selection while scrolling down", () => {
    let s = streamSelect(liveStream([g("1"), g("2"), g("3"), g("4"), g("5")], 2), "1");
    s = streamScrollUp(s);
    s = streamScrollDown(s);
    expect(s.followState).toBe("LIVE_FOLLOW");
    expect(s.selectedId).toBe("1");
  });

  it("is an identity no-op during LIVE_FOLLOW, COLD, and HYDRATING", () => {
    const live = liveStream([g("1"), g("2"), g("3")]);
    expect(streamScrollDown(live)).toBe(live);
    const cold = createTraceStream(3);
    expect(streamScrollDown(cold)).toBe(cold);
    const hydrating = asHydrating(cold);
    expect(streamScrollDown(hydrating)).toBe(hydrating);
  });
});

describe("streamReturnToLive", () => {
  it("jumps the window to the tail, LIVE_FOLLOW, unseen 0, anchor cleared", () => {
    let s = streamSelect(liveStream([g("1"), g("2"), g("3"), g("4"), g("5")], 3), "2");
    s = streamScrollUp(s);
    s = streamScrollUp(s);
    s = streamAppend(s, [g("6"), g("7"), g("8")]);
    expect(s.unseenCount).toBe(3);
    const resumed = streamReturnToLive(s);
    expect(resumed.followState).toBe("LIVE_FOLLOW");
    expect(resumed.unseenCount).toBe(0);
    expect(resumed.anchorTopId).toBeNull();
    expect(ids(streamVisibleWindow(resumed))).toEqual(["6", "7", "8"]);
    expect(resumed.selectedId).toBe("2");
  });

  it("is an identity no-op when already live, COLD, or HYDRATING", () => {
    const live = liveStream([g("1")]);
    expect(streamReturnToLive(live)).toBe(live);
    const cold = createTraceStream(3);
    expect(streamReturnToLive(cold)).toBe(cold);
    const hydrating = asHydrating(cold);
    expect(streamReturnToLive(hydrating)).toBe(hydrating);
  });
});

describe("streamSelect", () => {
  it("selects by stable id without moving the window or the follow state", () => {
    let s = liveStream([g("1"), g("2"), g("3"), g("4"), g("5")], 3);
    s = streamScrollUp(s);
    const beforeWindow = ids(streamVisibleWindow(s));
    const selected = streamSelect(s, "1");
    expect(selected.selectedId).toBe("1");
    expect(selected.followState).toBe("INSPECTION_HOLD");
    expect(selected.unseenCount).toBe(0);
    expect(ids(streamVisibleWindow(selected))).toEqual(beforeWindow);
  });

  it("nulls selection for an id that is not in the stream", () => {
    const s = streamSelect(liveStream([g("1")]), "nope");
    expect(s.selectedId).toBeNull();
  });

  it("re-selects to another id and back", () => {
    let s = streamSelect(liveStream([g("1"), g("2")]), "1");
    s = streamSelect(s, "2");
    expect(s.selectedId).toBe("2");
    s = streamSelect(s, "1");
    expect(s.selectedId).toBe("1");
  });
});

describe("streamLoadOlder (invariant d)", () => {
  it("(d) prepends history while the anchor row stays the visible top", () => {
    let s = liveStream([g("4"), g("5"), g("6"), g("7"), g("8")], 3);
    s = streamScrollUp(s);
    expect(ids(streamVisibleWindow(s))).toEqual(["5", "6", "7"]);
    const loaded = streamLoadOlder(s, [g("1"), g("2"), g("3")]);
    expect(loaded.followState).toBe("INSPECTION_HOLD");
    expect(loaded.anchorTopId).toBe("5");
    expect(ids(streamVisibleWindow(loaded))).toEqual(["5", "6", "7"]);
    expect(ids(loaded.records)).toEqual(["1", "2", "3", "4", "5", "6", "7", "8"]);
  });

  it("keeps the follow state and unseen count unchanged during hold and live", () => {
    let s = liveStream([g("4"), g("5"), g("6"), g("7"), g("8")], 3);
    s = streamScrollUp(s);
    s = streamAppend(s, [g("9")]);
    const heldLoaded = streamLoadOlder(s, [g("1")]);
    expect(heldLoaded.followState).toBe("INSPECTION_HOLD");
    expect(heldLoaded.unseenCount).toBe(1);
    const liveLoaded = streamLoadOlder(liveStream([g("4"), g("5"), g("6")], 3), [
      g("1"),
      g("2"),
      g("3"),
    ]);
    expect(liveLoaded.followState).toBe("LIVE_FOLLOW");
    expect(liveLoaded.unseenCount).toBe(0);
    expect(ids(streamVisibleWindow(liveLoaded))).toEqual(["4", "5", "6"]);
  });

  it("preserves selection across prepended history", () => {
    let s = streamSelect(liveStream([g("4"), g("5"), g("6")], 3), "5");
    s = streamLoadOlder(s, [g("1"), g("2"), g("3")]);
    expect(s.selectedId).toBe("5");
  });

  it("is an identity no-op during COLD and HYDRATING", () => {
    const cold = createTraceStream(3);
    expect(streamLoadOlder(cold, [g("a")])).toBe(cold);
    const hydrating = asHydrating(cold);
    expect(streamLoadOlder(hydrating, [g("a")])).toBe(hydrating);
  });

  it("is an identity no-op for an empty older batch", () => {
    const s = liveStream([g("1")]);
    expect(streamLoadOlder(s, [])).toBe(s);
  });
});

describe("sources never merge (invariant f)", () => {
  it("keeps two records that share an id across sources as two records", () => {
    let s = liveStream([g("evt-1")], 4);
    s = streamAppend(s, [r("evt-1")]);
    expect(s.records.length).toBe(2);
    const bySource = new Map(s.records.map((record) => [record.source, record]));
    expect(bySource.get("gauntlet")).toMatchObject({ id: "evt-1", source: "gauntlet" });
    expect(bySource.get("realitytrace")).toMatchObject({ id: "evt-1", source: "realitytrace" });
    expect(ids(streamVisibleWindow(s))).toEqual(["evt-1", "evt-1"]);
  });

  it("never reorders: cross-source order is arrival order only", () => {
    let s = liveStream([g("g1"), g("g2")], 6);
    s = streamAppend(s, [r("r1"), r("r2")]);
    s = streamAppend(s, [g("g3")]);
    expect(s.records.map((record) => `${record.source}:${record.id}`)).toEqual([
      "gauntlet:g1",
      "gauntlet:g2",
      "realitytrace:r1",
      "realitytrace:r2",
      "gauntlet:g3",
    ]);
  });

  it("carries optional per-source metadata without merging it away", () => {
    const s = liveStream([g("g1", 7), r("r1", 3)], 4);
    expect(s.records[0]).toEqual({ id: "g1", source: "gauntlet", ordinal: 7 });
    expect(s.records[1]).toEqual({ id: "r1", source: "realitytrace", ordinal: 3 });
  });
});

describe("COLD/HYDRATING transitions are explicit only (invariant g)", () => {
  it("every non-hydration operation is an identity no-op during COLD", () => {
    const cold = createTraceStream(3);
    expect(streamAppend(cold, [g("a")])).toBe(cold);
    expect(streamScrollUp(cold)).toBe(cold);
    expect(streamScrollDown(cold)).toBe(cold);
    expect(streamReturnToLive(cold)).toBe(cold);
    expect(streamLoadOlder(cold, [g("a")])).toBe(cold);
  });

  it("every non-hydration operation is an identity no-op during HYDRATING", () => {
    const hydrating = asHydrating(createTraceStream(3));
    expect(streamAppend(hydrating, [g("a")])).toBe(hydrating);
    expect(streamScrollUp(hydrating)).toBe(hydrating);
    expect(streamScrollDown(hydrating)).toBe(hydrating);
    expect(streamReturnToLive(hydrating)).toBe(hydrating);
    expect(streamLoadOlder(hydrating, [g("a")])).toBe(hydrating);
  });

  it("nothing in the API produces COLD or HYDRATING except construction", () => {
    const cold = createTraceStream(3);
    const states = [
      streamHydrated(cold, [g("a"), g("b"), g("c"), g("d")]),
      streamScrollUp(streamHydrated(cold, [g("a"), g("b")])),
      streamSelect(streamHydrated(cold, [g("a")]), "a"),
      streamLoadOlder(streamHydrated(cold, [g("a")]), [g("z")]),
      streamReturnToLive(streamScrollUp(streamHydrated(cold, [g("a"), g("b")]))),
    ];
    for (const state of states) {
      expect(["LIVE_FOLLOW", "INSPECTION_HOLD"]).toContain(state.followState);
    }
  });
});

describe("purity", () => {
  it("never mutates the input state", () => {
    const base = liveStream([g("1"), g("2"), g("3"), g("4"), g("5")], 3);
    const snapshotOf = (s: TraceStream) =>
      JSON.stringify({
        followState: s.followState,
        unseenCount: s.unseenCount,
        anchorTopId: s.anchorTopId,
        selectedId: s.selectedId,
        records: s.records,
      });
    const before = snapshotOf(base);
    streamAppend(base, [g("6")]);
    streamScrollUp(base);
    streamScrollDown(base);
    streamSelect(base, "1");
    streamLoadOlder(base, [g("0")]);
    streamReturnToLive(base);
    streamVisibleWindow(base);
    expect(snapshotOf(base)).toBe(before);
  });

  it("returns frozen states and frozen derived windows", () => {
    let s = liveStream([g("1"), g("2"), g("3")], 2);
    s = streamScrollUp(s);
    s = streamAppend(s, [g("4")]);
    s = streamSelect(s, "1");
    expect(Object.isFrozen(s)).toBe(true);
    expect(Object.isFrozen(s.records)).toBe(true);
    for (const record of s.records) {
      expect(Object.isFrozen(record)).toBe(true);
    }
    const window = streamVisibleWindow(s);
    expect(Object.isFrozen(window)).toBe(true);
    expect(() => {
      (window as unknown as TraceStreamRecord[]).push(g("x"));
    }).toThrow();
  });
});
