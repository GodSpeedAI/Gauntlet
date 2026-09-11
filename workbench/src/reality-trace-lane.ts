/**
 * RealityTrace ledger lane — the pure projection from an SXR evidence lens to
 * the Trace view's RealityTrace lane rows, with refresh-epoch staleness
 * (GUI-redesign increment 6, RealityTrace source of the one-ledger/two-source
 * Trace surface).
 *
 * Spec anchors:
 * - UESC §16.0: one ordered ledger over two source-typed streams; rows keep
 *   their source; per-source order is authoritative (ordinal within
 *   `ledger_id` for RealityTrace); sources are NEVER merged.
 * - UESC §16.2 (RealityTrace source machine): a failed refresh keeps the last
 *   CURRENT lens visible and marks it stale — staleness LABELS rows, it never
 *   drops or replaces them (REQ-SXR-001 honesty).
 * - UESC §16.8 (`RealityTraceRecord`): identity is `ledger_id` + `ordinal` +
 *   `entry_hash`; the lens interprets question/threshold/snapshot/run/
 *   evidence/settlement kinds; other kinds are preserved with identity and
 *   never interpreted. Pending durability (spooled / queued_for_commit) is
 *   never rendered as committed; the ledger export carries committed rows
 *   only, so this projection is committed-only BY CONSTRUCTION (`pending:
 *   false` at the type level). Pending records arrive via a future durability
 *   channel and must not be fabricated here.
 *
 * Input discipline: this module accepts a NARROW structural view of the lens
 * (`RealityTraceLaneLens`) — it never imports the adapter; the caller maps
 * `SxrEvidenceLens` (workbench/src/sxr-adapter.ts, `projectSxrEvidenceLens`)
 * onto it, unwrapping the branded `SxrOrdinal` / `SxrEntryHash` values at the
 * mapping boundary. Model output is untrusted input: everything is validated
 * and copied on admission, never aliased.
 *
 * Discipline: pure functions over frozen data; the only import is a TYPE from
 * `./trace-stream` so the stream hand-off is compile-checked against the real
 * `TraceStreamRecord` shape. No DOM/terminal/model/adapter access.
 */

import type { TraceStreamRecord } from "./trace-stream";

/** One committed RealityTrace ledger row as the Trace lane shows it. */
export interface RealityTraceLaneRow {
  /** Stable, collision-free id: `sxr:<ledgerId>#<ordinal>` (never a gauntlet trace id). */
  readonly id: string;
  readonly source: "realitytrace";
  readonly ordinal: number;
  readonly entryHash: string;
  /** The export row's record kind, verbatim (e.g. `question.declared`). */
  readonly kind: string;
  /** Short human label for the ledger row (presentation only, never identity). */
  readonly label: string;
  /**
   * Committed-only by construction: the ledger export contains committed rows
   * only, so this projection can never mint a pending row. Pending records
   * (spooled / queued_for_commit) arrive via a future durability channel.
   */
  readonly pending: false;
}

/** The RealityTrace lane of one bound ledger, at one refresh epoch. */
export interface RealityTraceLane {
  readonly repoId: string;
  readonly ledgerId: string;
  /** The refresh epoch this lane was projected at; compare with `laneStale`. */
  readonly refreshEpoch: number;
  readonly rows: readonly RealityTraceLaneRow[];
  /** Kinds the lens does not interpret — counted upstream, identity preserved. */
  readonly uninterpretedKinds: readonly string[];
  /** Total ledger records the lens saw (interpreted + uninterpreted + omissions). */
  readonly totalRecords: number;
}

/**
 * Minimal structural view of one interpreted lens record (questions,
 * thresholds, snapshots, runs, evidence, settlements). The caller maps each
 * `Sxr*Record.row` to `{ ordinal, entryHash }` — brands do not cross this
 * boundary.
 */
export interface RealityTraceLaneLensRecord {
  readonly ordinal: number;
  readonly entryHash: string;
}

/** Minimal structural view of the lens's uninterpreted accounting. */
export interface RealityTraceLaneLensUninterpreted {
  readonly record_kind: string;
  readonly count: number;
}

/**
 * Minimal structural view of an SXR evidence lens (`SxrEvidenceLens`), only
 * the fields this projection needs. Only the `status: "projected"` variant of
 * `SxrLensProjection` has a lane; absent/invalid lenses are a source status
 * for the shell (UESC §16.2 RealityTrace source machine), not a lane.
 */
export interface RealityTraceLaneLens {
  readonly status: "projected";
  readonly questions: readonly RealityTraceLaneLensRecord[];
  readonly thresholds: readonly RealityTraceLaneLensRecord[];
  readonly snapshots: readonly RealityTraceLaneLensRecord[];
  readonly runs: readonly RealityTraceLaneLensRecord[];
  readonly evidence: readonly RealityTraceLaneLensRecord[];
  readonly settlements: readonly RealityTraceLaneLensRecord[];
  readonly uninterpreted: readonly RealityTraceLaneLensUninterpreted[];
  /** `SxrEvidenceLens.rows.length` — the lossless row count of the export. */
  readonly totalRecords: number;
}

/** Ledger identity + refresh epoch the lane is projected under. */
export interface RealityTraceLaneMeta {
  readonly repoId: string;
  readonly ledgerId: string;
  readonly refreshEpoch: number;
}

/**
 * The lens's interpreted kinds (the only six `projectSxrEvidenceLens`
 * interprets), each with its short human label. Fixed flatten order — it is
 * also the deterministic tie-break when a caller maps the same ordinal into
 * two partitions (a mapping bug collapsed to one row, never expanded).
 */
const LANE_PARTITIONS: readonly {
  readonly key: LanePartitionKey;
  readonly kind: string;
  readonly label: string;
}[] = Object.freeze([
  { key: "questions", kind: "question.declared", label: "Question declared" },
  { key: "thresholds", kind: "threshold.declared", label: "Threshold declared" },
  { key: "snapshots", kind: "semantic.snapshot", label: "Semantic snapshot" },
  { key: "runs", kind: "run.started", label: "Run started" },
  { key: "evidence", kind: "evidence.created", label: "Evidence created" },
  { key: "settlements", kind: "settlement.registered", label: "Settlement registered" },
]);

type LanePartitionKey =
  | "questions"
  | "thresholds"
  | "snapshots"
  | "runs"
  | "evidence"
  | "settlements";

function requireNonEmptyString(value: unknown, what: string): string {
  if (typeof value !== "string" || value.length === 0) {
    throw new TypeError(
      `reality-trace-lane: ${what} must be a non-empty string, got ${String(value)}`,
    );
  }
  return value;
}

function requireInteger(value: unknown, minimum: number, what: string): number {
  if (typeof value !== "number" || !Number.isInteger(value) || value < minimum) {
    throw new TypeError(
      `reality-trace-lane: ${what} must be an integer >= ${String(minimum)}, got ${String(value)}`,
    );
  }
  return value;
}

/**
 * Project a (structural view of a) projected SXR evidence lens into the
 * RealityTrace ledger lane:
 * - interpreted records become committed, labelled rows ordered by ordinal
 *   ascending (the authoritative per-source order, UESC §16.0);
 * - ONE ledger row → ONE lane row: lens records sharing an ordinal (e.g. an
 *   `evidence.created` row carrying several evidence entries) collapse to a
 *   single row — identity is `ledger_id` + `ordinal` + `entry_hash` (UESC
 *   §16.8), and the finer records remain Inspector detail in the lens;
 * - kinds the lens marked uninterpreted never become rows; their identity is
 *   preserved verbatim in `uninterpretedKinds` (counted upstream by the lens,
 *   never guessed here);
 * - row ids are `sxr:<ledgerId>#<ordinal>` — unique within a lane, disjoint
 *   across ledgers, and disjoint from gauntlet trace ids by prefix.
 *
 * Throws `TypeError` on malformed input (untrusted model output is parsed and
 * validated, then admitted).
 */
export function projectRealityTraceLane(
  lens: RealityTraceLaneLens,
  meta: RealityTraceLaneMeta,
): RealityTraceLane {
  const repoId = requireNonEmptyString(meta?.repoId, "meta.repoId");
  const ledgerId = requireNonEmptyString(meta?.ledgerId, "meta.ledgerId");
  const refreshEpoch = requireInteger(meta?.refreshEpoch, 0, "meta.refreshEpoch");
  if (lens?.status !== "projected") {
    throw new TypeError(
      `reality-trace-lane: lens must be a projected evidence lens, got status ${String(lens?.status)}`,
    );
  }
  const totalRecords = requireInteger(lens.totalRecords, 0, "lens.totalRecords");

  // Flatten the six partitions in fixed order, validating each record.
  const flattened: { ordinal: number; entryHash: string; kind: string; label: string }[] = [];
  for (const partition of LANE_PARTITIONS) {
    const records = lens[partition.key];
    if (!Array.isArray(records)) {
      throw new TypeError(`reality-trace-lane: lens.${partition.key} must be an array`);
    }
    for (const record of records) {
      flattened.push({
        ordinal: requireInteger(record?.ordinal, 1, `${partition.key} ordinal`),
        entryHash: requireNonEmptyString(record?.entryHash, `${partition.key} entryHash`),
        kind: partition.kind,
        label: partition.label,
      });
    }
  }
  // Authoritative order: ordinal ascending (stable — equal ordinals keep the
  // fixed partition order, so the collapse below is deterministic).
  flattened.sort((a, b) => a.ordinal - b.ordinal);

  // One ledger row → one lane row: collapse duplicate ordinals.
  const rows: RealityTraceLaneRow[] = [];
  for (const entry of flattened) {
    if (rows.length > 0 && rows[rows.length - 1]?.ordinal === entry.ordinal) {
      continue;
    }
    const row: RealityTraceLaneRow = Object.freeze({
      id: `sxr:${ledgerId}#${entry.ordinal}`,
      source: "realitytrace",
      ordinal: entry.ordinal,
      entryHash: entry.entryHash,
      kind: entry.kind,
      label: entry.label,
      pending: false,
    });
    rows.push(row);
  }
  // Post-condition (structural): ids are unique because ordinals are, after
  // the collapse, and every id embeds ledger + ordinal.
  if (new Set(rows.map((row) => row.id)).size !== rows.length) {
    throw new TypeError("reality-trace-lane: row id collision (invariant violated)");
  }

  // Uninterpreted kinds: identity preserved verbatim, deduped first-seen.
  // Counts stay owned by the lens; the lane preserves kind identity only.
  const uninterpretedKinds: string[] = [];
  const seenKinds = new Set<string>();
  if (!Array.isArray(lens.uninterpreted)) {
    throw new TypeError("reality-trace-lane: lens.uninterpreted must be an array");
  }
  for (const entry of lens.uninterpreted) {
    const kind = requireNonEmptyString(entry?.record_kind, "uninterpreted record_kind");
    requireInteger(entry?.count, 1, "uninterpreted count");
    if (!seenKinds.has(kind)) {
      seenKinds.add(kind);
      uninterpretedKinds.push(kind);
    }
  }

  return Object.freeze({
    repoId,
    ledgerId,
    refreshEpoch,
    rows: Object.freeze(rows),
    uninterpretedKinds: Object.freeze(uninterpretedKinds),
    totalRecords,
  });
}

/**
 * Whether the lane was projected at a different refresh epoch than the
 * current one (any mismatch — older OR newer — is staleness). Staleness is a
 * LABEL (UESC §16.2: a failed refresh keeps the last CURRENT lens visible and
 * marks it stale); it never drops, hides, or replaces rows.
 */
export function laneStale(lane: RealityTraceLane, currentEpoch: number): boolean {
  return lane.refreshEpoch !== currentEpoch;
}

/**
 * The lane's rows as `TraceStreamRecord`-shaped records, so the shell can
 * append the RealityTrace lane into a TraceStream without merging identity:
 * each record keeps its lane id (`sxr:<ledgerId>#<ordinal>`), carries
 * `source: "realitytrace"`, and carries its ordinal for the view's per-source
 * ordering (no `timestampMs` — the ledger export omits `committed_at`, UESC
 * §16.0). Works identically on stale lanes: staleness labels, never drops.
 */
export function laneRowsForStream(lane: RealityTraceLane): readonly TraceStreamRecord[] {
  return Object.freeze(
    lane.rows.map((row) =>
      Object.freeze({ id: row.id, source: "realitytrace", ordinal: row.ordinal }),
    ),
  );
}
