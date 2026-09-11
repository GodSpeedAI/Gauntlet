import { describe, expect, it } from "bun:test";
import type {
  RealityTraceLane,
  RealityTraceLaneLens,
  RealityTraceLaneLensRecord,
  RealityTraceLaneMeta,
  RealityTraceLaneRow,
} from "./reality-trace-lane";
import { laneRowsForStream, laneStale, projectRealityTraceLane } from "./reality-trace-lane";
import type { TraceStreamRecord } from "./trace-stream";

/**
 * GUI-redesign increment 6 (RealityTrace lane): the pure projection from an
 * SXR evidence lens to ledger lane rows with refresh-epoch staleness (UESC
 * §16.0/§16.8: one ledger, two labelled sources, NEVER merged; REQ-SXR-001
 * honesty — staleness labels, it never drops rows).
 *
 * Pinned invariants:
 * (a) every row id embeds ledger + ordinal (`sxr:<ledgerId>#<ordinal>`) and
 *     is unique; ids never collide across ledgers nor with gauntlet trace ids;
 * (b) two lanes from different ledgers never share row ids;
 * (c) epoch mismatch → laneStale true, and laneRowsForStream STILL returns
 *     rows (staleness labels, never drops);
 * (d) uninterpreted kinds are counted, not invented: they never become rows
 *     and their identity is preserved verbatim from the lens;
 * (e) committed-only: no pending rows are ever constructed (type-level
 *     `pending: false` and runtime `pending === false`).
 */

function h(seed: string): string {
  return `sha256:${seed.padEnd(64, "0").slice(0, 64)}`;
}

const META: RealityTraceLaneMeta = Object.freeze({
  repoId: "repo-alpha",
  ledgerId: "ledger-7",
  refreshEpoch: 4,
});

/** A realistic lens: every interpreted partition, a shared-ordinal evidence
 * row (one ledger row → two lens records), and two uninterpreted kinds. */
function baseLens(): RealityTraceLaneLens {
  return {
    status: "projected",
    questions: [{ ordinal: 1, entryHash: h("a1") }],
    thresholds: [{ ordinal: 2, entryHash: h("a2") }],
    snapshots: [{ ordinal: 3, entryHash: h("a3") }],
    runs: [{ ordinal: 4, entryHash: h("a4") }],
    evidence: [
      { ordinal: 5, entryHash: h("a5") },
      { ordinal: 5, entryHash: h("a5") },
      { ordinal: 6, entryHash: h("a6") },
    ],
    settlements: [{ ordinal: 7, entryHash: h("a7") }],
    uninterpreted: [
      { record_kind: "debt.recorded", count: 2 },
      { record_kind: "residual.computed", count: 1 },
    ],
    totalRecords: 10,
  };
}

function project(lens: RealityTraceLaneLens = baseLens(), meta = META): RealityTraceLane {
  return projectRealityTraceLane(lens, meta);
}

function rowIds(lane: RealityTraceLane): string[] {
  return lane.rows.map((row) => row.id);
}

describe("projectRealityTraceLane — row construction", () => {
  it("projects every interpreted kind to a labelled row (kind + short human label)", () => {
    const lane = project();
    expect(lane.rows.map((row) => row.kind)).toEqual([
      "question.declared",
      "threshold.declared",
      "semantic.snapshot",
      "run.started",
      "evidence.created",
      "evidence.created",
      "settlement.registered",
    ]);
    for (const row of lane.rows) {
      expect(typeof row.label).toBe("string");
      expect(row.label.length > 0).toBe(true);
    }
    const labels = lane.rows.map((row) => `${row.kind} → ${row.label}`);
    expect(labels).toEqual([
      "question.declared → Question declared",
      "threshold.declared → Threshold declared",
      "semantic.snapshot → Semantic snapshot",
      "run.started → Run started",
      "evidence.created → Evidence created",
      "evidence.created → Evidence created",
      "settlement.registered → Settlement registered",
    ]);
  });

  it("orders rows by ordinal ascending regardless of partition order", () => {
    const lane = project({
      ...baseLens(),
      questions: [{ ordinal: 9, entryHash: h("q9") }],
      thresholds: [],
      snapshots: [],
      runs: [],
      evidence: [{ ordinal: 2, entryHash: h("e2") }],
      settlements: [{ ordinal: 5, entryHash: h("s5") }],
    });
    expect(lane.rows.map((row) => row.ordinal)).toEqual([2, 5, 9]);
  });

  it("collapses multiple lens records sharing one ordinal into ONE lane row", () => {
    // One evidence.created ledger row carrying two evidence entries: the row,
    // not the entries, is the lane identity (ledger_id + ordinal + entry_hash).
    const lane = project();
    expect(lane.rows.filter((row) => row.ordinal === 5)).toHaveLength(1);
    expect(rowIds(lane).filter((id) => id === "sxr:ledger-7#5")).toHaveLength(1);
    expect(lane.rows.map((row) => row.ordinal)).toEqual([1, 2, 3, 4, 5, 6, 7]);
  });

  it("embeds ledger id and ordinal in every row id (grammar sxr:<ledgerId>#<ordinal>)", () => {
    const lane = project();
    for (const row of lane.rows) {
      expect(row.id).toBe(`sxr:${META.ledgerId}#${row.ordinal}`);
      expect(row.id.startsWith("sxr:")).toBe(true);
      expect(row.id.includes("#")).toBe(true);
      expect(row.id.includes(META.ledgerId)).toBe(true);
    }
  });

  it("row ids are unique within the lane", () => {
    const ids = rowIds(project());
    expect(new Set(ids).size).toBe(ids.length);
  });

  it("row ids are disjoint from gauntlet-style trace ids (trace-<source>-<cursor>)", () => {
    const gauntletIds = ["trace-loopback-12", "trace-sxr-0", "trace:ledger-7#1"];
    const ids = rowIds(project());
    for (const id of ids) {
      expect(gauntletIds.includes(id)).toBe(false);
      expect(id.startsWith("trace")).toBe(false);
    }
  });

  it("carries each row's entry hash from the lens", () => {
    const lane = project();
    expect(lane.rows[0]?.entryHash).toBe(h("a1"));
    expect(lane.rows[4]?.entryHash).toBe(h("a5"));
  });

  it("carries meta repoId, ledgerId, and refreshEpoch unchanged", () => {
    const lane = project();
    expect(lane.repoId).toBe(META.repoId);
    expect(lane.ledgerId).toBe(META.ledgerId);
    expect(lane.refreshEpoch).toBe(META.refreshEpoch);
  });

  it("totalRecords is the lens total (interpreted + uninterpreted), independent of rows.length", () => {
    const lane = project();
    expect(lane.totalRecords).toBe(10);
    expect(lane.rows.length).toBe(7);
  });

  it("empty lens projects to an empty lane with zero totals", () => {
    const lane = project({
      status: "projected",
      questions: [],
      thresholds: [],
      snapshots: [],
      runs: [],
      evidence: [],
      settlements: [],
      uninterpreted: [],
      totalRecords: 0,
    });
    expect(lane.rows).toEqual([]);
    expect(lane.uninterpretedKinds).toEqual([]);
    expect(lane.totalRecords).toBe(0);
    expect(laneRowsForStream(lane)).toEqual([]);
  });
});

describe("uninterpreted kinds — counted, never invented", () => {
  it("unknown kinds never become rows; identity preserved verbatim", () => {
    const lane = project();
    expect(lane.rows.some((row) => row.kind === "debt.recorded")).toBe(false);
    expect(lane.rows.some((row) => row.kind === "residual.computed")).toBe(false);
    expect(lane.uninterpretedKinds).toEqual(["debt.recorded", "residual.computed"]);
  });

  it("preserves kind identity without normalization or guessing", () => {
    const lane = project({
      ...baseLens(),
      uninterpreted: [{ record_kind: "Diagnostic.Hypothesis ", count: 1 }],
      totalRecords: 8,
    });
    // Verbatim: no case folding, no trimming, no expansion.
    expect(lane.uninterpretedKinds).toEqual(["Diagnostic.Hypothesis "]);
  });

  it("dedupes repeated kinds, preserving first-seen order", () => {
    const lane = project({
      ...baseLens(),
      uninterpreted: [
        { record_kind: "zeta.kind", count: 1 },
        { record_kind: "alpha.kind", count: 3 },
        { record_kind: "zeta.kind", count: 2 },
      ],
    });
    expect(lane.uninterpretedKinds).toEqual(["zeta.kind", "alpha.kind"]);
  });

  it("takes the lens's word: a kind the lens diverted stays uninterpreted even when the same kind is interpreted elsewhere", () => {
    // The adapter counts a malformed question.declared row as uninterpreted;
    // the lane must not re-derive interpretation from the kind name.
    const lane = project({
      ...baseLens(),
      uninterpreted: [{ record_kind: "question.declared", count: 1 }],
      totalRecords: 11,
    });
    expect(lane.uninterpretedKinds).toContain("question.declared");
    expect(lane.rows.filter((row) => row.kind === "question.declared")).toHaveLength(1);
  });
});

describe("committed-only construction", () => {
  it("every row is pending === false and source realitytrace (runtime)", () => {
    for (const row of project().rows) {
      expect(row.pending).toBe(false);
      expect(row.source).toBe("realitytrace");
    }
  });

  it("pending is the literal false at the type level", () => {
    const row = project().rows[0];
    expect(row).toBeDefined();
    if (row === undefined) {
      return;
    }
    const committedProof: false = row.pending;
    expect(committedProof).toBe(false);
    // @ts-expect-error — `true` must not be assignable: the export is
    // committed-only by construction; pending rows arrive via a future
    // durability channel, never from this projection.
    const mustNotCompile: RealityTraceLaneRow = { ...row, pending: true };
    void mustNotCompile;
  });
});

describe("laneStale — staleness labels, never drops (REQ-SXR-001)", () => {
  it("same epoch → fresh (false)", () => {
    expect(laneStale(project(), META.refreshEpoch)).toBe(false);
  });

  it("older lane epoch than current → stale (true)", () => {
    expect(laneStale(project(), META.refreshEpoch + 1)).toBe(true);
  });

  it("newer lane epoch than current → stale (true): any mismatch is staleness", () => {
    expect(laneStale(project(), META.refreshEpoch - 1)).toBe(true);
    expect(laneStale(project(), 0)).toBe(true);
  });

  it("a stale lane still returns its rows from laneRowsForStream", () => {
    const lane = project();
    expect(laneStale(lane, META.refreshEpoch + 3)).toBe(true);
    const streamRows = laneRowsForStream(lane);
    expect(streamRows).toHaveLength(lane.rows.length);
    expect(streamRows.map((record) => record.id)).toEqual(rowIds(lane));
  });

  it("rows are byte-identical across epochs; only refreshEpoch differs", () => {
    const atFour = project(baseLens(), { ...META, refreshEpoch: 4 });
    const atNine = project(baseLens(), { ...META, refreshEpoch: 9 });
    expect(atFour.rows).toEqual(atNine.rows);
    expect(atFour.uninterpretedKinds).toEqual(atNine.uninterpretedKinds);
    expect(atFour.refreshEpoch).toBe(4);
    expect(atNine.refreshEpoch).toBe(9);
    expect(laneStale(atFour, 9)).toBe(true);
    expect(laneStale(atNine, 9)).toBe(false);
  });
});

describe("laneRowsForStream — TraceStreamRecord-shaped output", () => {
  it("maps rows to exactly {id, source: 'realitytrace', ordinal}", () => {
    const streamRows = laneRowsForStream(project());
    expect(streamRows).toHaveLength(7);
    expect(streamRows[0]).toStrictEqual({
      id: "sxr:ledger-7#1",
      source: "realitytrace",
      ordinal: 1,
    });
    expect(streamRows[6]).toStrictEqual({
      id: "sxr:ledger-7#7",
      source: "realitytrace",
      ordinal: 7,
    });
  });

  it("is structurally TraceStreamRecord[] (compile-time) with runtime source checks", () => {
    const streamRows: readonly TraceStreamRecord[] = laneRowsForStream(project());
    for (const record of streamRows) {
      expect(record.source).toBe("realitytrace");
      expect(typeof record.id === "string" && record.id.length > 0).toBe(true);
      expect(Number.isInteger(record.ordinal)).toBe(true);
    }
  });

  it("preserves lane row order and id correspondence", () => {
    const lane = project();
    const streamRows = laneRowsForStream(lane);
    expect(streamRows.map((record) => record.id)).toEqual(rowIds(lane));
    expect(streamRows.map((record) => record.ordinal)).toEqual(lane.rows.map((row) => row.ordinal));
  });

  it("admits into a real TraceStream without merging identity", () => {
    // Cross-check against the actual trace-stream machine: gauntlet rows and
    // realitytrace rows coexist; ids never collide; sources never merge.
    const gauntletRecords: TraceStreamRecord[] = [
      { id: "trace-loopback-1", source: "gauntlet", ordinal: 1 },
      { id: "trace-loopback-2", source: "gauntlet", ordinal: 2 },
    ];
    const stream = laneRowsForStream(project());
    const all = [...gauntletRecords, ...stream];
    expect(new Set(all.map((record) => record.id)).size).toBe(all.length);
    expect(all.filter((record) => record.source === "realitytrace")).toHaveLength(7);
    expect(all.filter((record) => record.source === "gauntlet")).toHaveLength(2);
  });
});

describe("cross-lane identity", () => {
  it("two lanes from different ledgers never share row ids", () => {
    const laneA = project(baseLens(), { ...META, ledgerId: "ledger-A" });
    const laneB = project(baseLens(), { ...META, ledgerId: "ledger-B" });
    const idsA = new Set(rowIds(laneA));
    for (const id of rowIds(laneB)) {
      expect(idsA.has(id)).toBe(false);
    }
    expect(rowIds(laneA)[0]).toBe("sxr:ledger-A#1");
    expect(rowIds(laneB)[0]).toBe("sxr:ledger-B#1");
  });

  it("the same ledger and ordinal always map to the same id", () => {
    const first = project(baseLens(), META);
    const second = project(baseLens(), META);
    expect(rowIds(first)).toEqual(rowIds(second));
  });
});

describe("validation — malformed input is rejected, not guessed", () => {
  it("rejects a non-projected lens status", () => {
    const absent = { status: "absent", reason: "no_ledger_export", detail: "none" };
    expect(() => projectRealityTraceLane(absent as unknown as RealityTraceLaneLens, META)).toThrow(
      TypeError,
    );
  });

  it("rejects empty repoId / ledgerId and non-integer refreshEpoch", () => {
    expect(() => project(baseLens(), { ...META, repoId: "" })).toThrow(TypeError);
    expect(() => project(baseLens(), { ...META, ledgerId: "" })).toThrow(TypeError);
    expect(() => project(baseLens(), { ...META, refreshEpoch: 1.5 })).toThrow(TypeError);
    expect(() => project(baseLens(), { ...META, refreshEpoch: -1 })).toThrow(TypeError);
  });

  it("rejects ordinals that are not positive integers", () => {
    expect(() =>
      project({ ...baseLens(), questions: [{ ordinal: 0, entryHash: h("z") }] }),
    ).toThrow(TypeError);
    expect(() =>
      project({ ...baseLens(), questions: [{ ordinal: -3, entryHash: h("z") }] }),
    ).toThrow(TypeError);
    expect(() =>
      project({ ...baseLens(), questions: [{ ordinal: 2.5, entryHash: h("z") }] }),
    ).toThrow(TypeError);
  });

  it("rejects empty entry hashes", () => {
    expect(() => project({ ...baseLens(), settlements: [{ ordinal: 9, entryHash: "" }] })).toThrow(
      TypeError,
    );
  });

  it("rejects malformed uninterpreted entries (empty kind, non-positive count)", () => {
    expect(() =>
      project({ ...baseLens(), uninterpreted: [{ record_kind: "", count: 1 }] }),
    ).toThrow(TypeError);
    expect(() =>
      project({ ...baseLens(), uninterpreted: [{ record_kind: "k", count: 0 }] }),
    ).toThrow(TypeError);
  });

  it("rejects a negative totalRecords", () => {
    expect(() => project({ ...baseLens(), totalRecords: -1 })).toThrow(TypeError);
  });
});

describe("purity and freezing", () => {
  it("returns a frozen lane with frozen rows, uninterpretedKinds, and frozen stream rows", () => {
    const lane = project();
    expect(Object.isFrozen(lane)).toBe(true);
    expect(Object.isFrozen(lane.rows)).toBe(true);
    expect(Object.isFrozen(lane.uninterpretedKinds)).toBe(true);
    for (const row of lane.rows) {
      expect(Object.isFrozen(row)).toBe(true);
    }
    const streamRows = laneRowsForStream(lane);
    expect(Object.isFrozen(streamRows)).toBe(true);
    for (const record of streamRows) {
      expect(Object.isFrozen(record)).toBe(true);
    }
  });

  it("copies input on admission: later caller mutation cannot change the lane", () => {
    const lens = baseLens();
    const lane = projectRealityTraceLane(lens, META);
    const mutableQuestions = lens.questions as RealityTraceLaneLensRecord[];
    mutableQuestions.push({ ordinal: 42, entryHash: h("x") });
    (lens.uninterpreted[0] as { record_kind: string }).record_kind = "mutated.kind";
    expect(lane.rows.some((row) => row.ordinal === 42)).toBe(false);
    expect(lane.uninterpretedKinds).toEqual(["debt.recorded", "residual.computed"]);
  });

  it("never mutates the caller's lens (sorting is done on a copy)", () => {
    const lens = baseLens();
    const before = JSON.stringify(lens);
    projectRealityTraceLane(lens, META);
    expect(JSON.stringify(lens)).toBe(before);
  });
});
