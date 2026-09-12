/**
 * RealityTrace (SXR) binding — the host-side boundary between the adapter's
 * refresh outcome (`sxr-adapter.ts`) and the pure lane projector
 * (`reality-trace-lane.ts`), wiring a real, bounded, on-demand refresh
 * (GUI-redesign increment 6, RealityTrace source of the one-ledger/two-source
 * Trace surface, ICM §16.11).
 *
 * Discipline: pure functions only — no I/O. `reality-trace-lane.ts` never
 * imports the adapter (its input discipline is a NARROW structural view), so
 * this module owns the mapping and unwraps the branded `SxrOrdinal` /
 * `SxrEntryHash` values at the boundary.
 */

import {
  projectRealityTraceLane,
  type RealityTraceLane,
  type RealityTraceLaneLens,
  type RealityTraceLaneLensRecord,
} from "./reality-trace-lane";
import {
  projectSxrEvidenceLens,
  requireSxrEntryHash,
  requireSxrOrdinal,
  type SxrEntryHash,
  type SxrEvidenceLens,
  type SxrLedgerExportParse,
  type SxrLensProjection,
  type SxrOrdinal,
  type SxrRefreshOutcome,
} from "./sxr-adapter";

type LensRecordLike = {
  readonly row: { readonly ordinal: SxrOrdinal; readonly entry_hash: SxrEntryHash };
};

function toLaneRecords(records: readonly LensRecordLike[]): readonly RealityTraceLaneLensRecord[] {
  return records.map((record) =>
    Object.freeze({
      ordinal: requireSxrOrdinal(record.row.ordinal),
      entryHash: requireSxrEntryHash(record.row.entry_hash),
    }),
  );
}

/**
 * Maps a projected SXR evidence lens onto reality-trace-lane's narrow
 * structural input, unwrapping branded ordinal/entry-hash values at this
 * boundary (reality-trace-lane.ts never imports the adapter).
 */
export function sxrEvidenceLensToLaneLens(lens: SxrEvidenceLens): RealityTraceLaneLens {
  return Object.freeze({
    status: "projected",
    questions: Object.freeze(toLaneRecords(lens.questions)),
    thresholds: Object.freeze(toLaneRecords(lens.thresholds)),
    snapshots: Object.freeze(toLaneRecords(lens.snapshots)),
    runs: Object.freeze(toLaneRecords(lens.runs)),
    evidence: Object.freeze(toLaneRecords(lens.evidence)),
    settlements: Object.freeze(toLaneRecords(lens.settlements)),
    uninterpreted: Object.freeze(
      lens.uninterpreted.map((entry) =>
        Object.freeze({ record_kind: entry.record_kind, count: entry.count }),
      ),
    ),
    totalRecords: lens.rows.length,
  });
}

export interface RealityTraceRefreshResult {
  readonly lens: SxrLensProjection | undefined;
  readonly lane: RealityTraceLane | undefined;
}

/**
 * Interpret one refresh outcome: only a `refreshed` outcome whose parse
 * PARSED under the `ledger_export` command produces data — this refresh only
 * ever issues that command (`workbench-host.ts`). Every other outcome class
 * (spawn_failed / deadline_exceeded / output_limit_exceeded / producer_refused)
 * or a non-parsed/non-ledger-export parse returns `{lens: undefined, lane:
 * undefined}`: the host keeps the previous lens/lane and marks it stale via
 * `laneStale` — this function never synthesizes a fake `SxrLensProjection`.
 */
export function interpretSxrRefreshOutcome(
  outcome: SxrRefreshOutcome,
  repoId: string,
  refreshEpoch: number,
): RealityTraceRefreshResult {
  if (
    outcome.class !== "refreshed" ||
    outcome.command !== "ledger_export" ||
    outcome.parse.status !== "parsed"
  ) {
    return { lens: undefined, lane: undefined };
  }
  // `command === "ledger_export"` binds `parse` to `SxrLedgerExportParse` at
  // runtime (this refresh only ever issues that command), which the
  // `SxrRefreshOutcome` union does not express as a type-level correlation.
  const parse = outcome.parse as SxrLedgerExportParse & { readonly status: "parsed" };
  const lens = projectSxrEvidenceLens(parse, repoId);
  if (lens.status !== "projected") {
    return { lens, lane: undefined };
  }
  const ledgerId = parse.ledger_ids[0];
  if (ledgerId === undefined) {
    return { lens, lane: undefined };
  }
  const lane = projectRealityTraceLane(sxrEvidenceLensToLaneLens(lens), {
    repoId,
    ledgerId,
    refreshEpoch,
  });
  return { lens, lane };
}
