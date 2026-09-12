import { describe, expect, it } from "bun:test";
import { interpretSxrRefreshOutcome, sxrEvidenceLensToLaneLens } from "./reality-trace-binding";
import {
  parseSxrLedgerExport,
  type SxrEvidenceLens,
  type SxrLedgerRow,
  type SxrRefreshOutcome,
  sxrEntryHash,
  sxrOrdinal,
} from "./sxr-adapter";

/**
 * GUI-redesign increment 6 (RealityTrace binding): the host-side boundary
 * mapping a projected SXR evidence lens onto reality-trace-lane's narrow
 * input, and interpreting a bounded refresh outcome (ICM §16.11: refresh is
 * command-triggered only, never during render).
 */

function h(seed: string): string {
  return `sha256:${seed.padEnd(64, "0").slice(0, 64)}`;
}

function row(ordinal: number, hashSeed: string, kind: string): SxrLedgerRow {
  return {
    ledger_id: "ledger-7",
    ordinal: sxrOrdinal(ordinal),
    record_kind: kind,
    kind_known: true,
    entry_hash: sxrEntryHash(h(hashSeed)),
    payload: {},
    subject_refs: [],
    export_omissions: [],
  };
}

/** Mirrors reality-trace-lane.test.ts's baseLens() fixture style. */
function baseLens(): SxrEvidenceLens {
  const questionRow = row(1, "a1", "question.declared");
  const thresholdRow = row(2, "a2", "threshold.declared");
  const snapshotRow = row(3, "a3", "semantic.snapshot");
  const runRow = row(4, "a4", "run.started");
  const evidenceRow = row(5, "a5", "evidence.created");
  const settlementRow = row(6, "a6", "settlement.registered");
  return {
    status: "projected",
    repo_id: "repo-alpha",
    questions: [
      {
        row: questionRow,
        question_id: "q-1",
        question_form: undefined,
        question_type: undefined,
        target_entity_ref: undefined,
      },
    ],
    thresholds: [
      {
        row: thresholdRow,
        threshold_id: "th-1",
        question_ref: undefined,
        threshold_kind: undefined,
        evaluation_profile: undefined,
      },
    ],
    snapshots: [
      {
        row: snapshotRow,
        semantic_closure_hash: undefined,
        content_hash: undefined,
        preserved_entities: [],
        declaration_names: [],
      },
    ],
    runs: [
      {
        row: runRow,
        run_id: undefined,
        repo_id: undefined,
        question_ref: undefined,
        threshold_ref: undefined,
        snapshot_ref: undefined,
      },
    ],
    evidence: [
      {
        row: evidenceRow,
        evidence_id: "ev-1",
        question_ref: undefined,
        target_entity_ref: undefined,
        direction: undefined,
        evidence_type: undefined,
        relevance: undefined,
        reliability: undefined,
      },
    ],
    settlements: [
      {
        row: settlementRow,
        settlement_id: "set-1",
        question_ref: undefined,
        threshold_ref: undefined,
        sxr_settlement_status: "settled",
      },
    ],
    uninterpreted: [{ record_kind: "debt.recorded", count: 2 }],
    rows: [questionRow, thresholdRow, snapshotRow, runRow, evidenceRow, settlementRow],
  };
}

describe("sxrEvidenceLensToLaneLens — mapping and unwrap", () => {
  it("maps each partition's row ordinal/entry_hash into {ordinal, entryHash}, unwrapped", () => {
    const laneLens = sxrEvidenceLensToLaneLens(baseLens());
    expect(laneLens.status).toBe("projected");
    expect(laneLens.questions).toEqual([{ ordinal: 1, entryHash: h("a1") }]);
    expect(laneLens.thresholds).toEqual([{ ordinal: 2, entryHash: h("a2") }]);
    expect(laneLens.snapshots).toEqual([{ ordinal: 3, entryHash: h("a3") }]);
    expect(laneLens.runs).toEqual([{ ordinal: 4, entryHash: h("a4") }]);
    expect(laneLens.evidence).toEqual([{ ordinal: 5, entryHash: h("a5") }]);
    expect(laneLens.settlements).toEqual([{ ordinal: 6, entryHash: h("a6") }]);
  });

  it("maps uninterpreted 1:1 and totalRecords to rows.length", () => {
    const lens = baseLens();
    const laneLens = sxrEvidenceLensToLaneLens(lens);
    expect(laneLens.uninterpreted).toEqual([{ record_kind: "debt.recorded", count: 2 }]);
    expect(laneLens.totalRecords).toBe(lens.rows.length);
    expect(laneLens.totalRecords).toBe(6);
  });
});

describe("interpretSxrRefreshOutcome", () => {
  it("refreshed + parsed + ledger_export produces both lens and lane", () => {
    const exportJson = JSON.stringify([
      {
        ledger_id: "ledger-9",
        append_ordinal: 1,
        record_kind: "question.declared",
        entry_hash: h("a9"),
        payload: { questions: [{ question_id: "q-1" }] },
      },
    ]);
    const parse = parseSxrLedgerExport(exportJson);
    expect(parse.status).toBe("parsed");
    const outcome: SxrRefreshOutcome = {
      class: "refreshed",
      command: "ledger_export",
      side_effects: {
        rebuilds_projections: false,
        initializes_local_storage: true,
        appends_canonical_ledger: false,
        launches_autonomous_run: false,
      },
      exit_code: 0,
      parse,
      stderr_text: "",
    };
    const result = interpretSxrRefreshOutcome(outcome, "repo-alpha", 3);
    expect(result.lens).toBeDefined();
    expect(result.lens?.status).toBe("projected");
    expect(result.lane).toBeDefined();
    expect(result.lane?.ledgerId).toBe("ledger-9");
    expect(result.lane?.refreshEpoch).toBe(3);
    expect(result.lane?.repoId).toBe("repo-alpha");
  });

  it("producer_refused produces {lens: undefined, lane: undefined}", () => {
    const outcome: SxrRefreshOutcome = {
      class: "producer_refused",
      exit_code: 1,
      stderr_text: "boom",
      detail: "boom",
    };
    expect(interpretSxrRefreshOutcome(outcome, "repo-alpha", 1)).toEqual({
      lens: undefined,
      lane: undefined,
    });
  });

  it("spawn_failed produces {lens: undefined, lane: undefined}", () => {
    const outcome: SxrRefreshOutcome = { class: "spawn_failed", detail: "enoent" };
    expect(interpretSxrRefreshOutcome(outcome, "repo-alpha", 1)).toEqual({
      lens: undefined,
      lane: undefined,
    });
  });

  it("refreshed with a non-parsed parse status produces {lens: undefined, lane: undefined}", () => {
    const outcome: SxrRefreshOutcome = {
      class: "refreshed",
      command: "ledger_export",
      side_effects: {
        rebuilds_projections: false,
        initializes_local_storage: true,
        appends_canonical_ledger: false,
        launches_autonomous_run: false,
      },
      exit_code: 0,
      parse: { status: "invalid", reason: "malformed_json", detail: "not json" },
      stderr_text: "",
    };
    expect(interpretSxrRefreshOutcome(outcome, "repo-alpha", 1)).toEqual({
      lens: undefined,
      lane: undefined,
    });
  });

  it("skips lane projection when the parse carries no ledger id", () => {
    const parse = parseSxrLedgerExport("[]");
    expect(parse.status).toBe("parsed");
    const outcome: SxrRefreshOutcome = {
      class: "refreshed",
      command: "ledger_export",
      side_effects: {
        rebuilds_projections: false,
        initializes_local_storage: true,
        appends_canonical_ledger: false,
        launches_autonomous_run: false,
      },
      exit_code: 0,
      parse,
      stderr_text: "",
    };
    const result = interpretSxrRefreshOutcome(outcome, "repo-alpha", 1);
    expect(result.lens?.status).toBe("projected");
    expect(result.lane).toBeUndefined();
  });
});
