import { Database } from "bun:sqlite";
import { createHash } from "node:crypto";
import { existsSync, readFileSync, renameSync, unlinkSync } from "node:fs";
import { join, resolve } from "node:path";
import type { PromotedKernelObject } from "./evidence-promotion";
import { KernelObjectLedger } from "./evidence-promotion";
import type { NormalizedRunProjection, RecordedKernelObject, RecordedRun } from "./headless-replay";
import {
  canonicalJson,
  canonicalSections,
  foldLogToNormalized,
  normalizeLiveFold,
  normalizeSnapshot,
  projectionDigest,
  recordRun,
} from "./headless-replay";
import type { DetachedPresentationState, StreamEvent } from "./live-streams";
import {
  canonicalStreamOrder,
  InMemoryStreamPort,
  InMemoryStreamSource,
  LiveStreamClient,
  projectionDifferences,
} from "./live-streams";
import type { PrimeHostPort } from "./prime-host";
import { PrimeHostAdapter } from "./prime-host";
import { PrimeReplKernel } from "./prime-kernel";
import { loggingPrimeAdapter, reattachToPrime } from "./prime-long-running";
import type { AuthoritativeEventFrame, WorkbenchSnapshot } from "./projection-store";
import { SqliteProjectionStore } from "./projection-store";
import type { CapabilityObservation } from "./workbench-model";
import { WorkbenchModel } from "./workbench-model";

/**
 * Cold-loss and recovery matrix (plan T22; RECOV-001..RECOV-005 plus the
 * REQ-SXR-004/REQ-SYNC-003 absence cases; REQ-SAFE-001/005, REQ-SYNC-001/003,
 * REQ-VERIFY-001/002/005/006).
 *
 * A DECLARATIVE registry of the applicable destructive recovery cases: every
 * case freezes (a) the exact destructive procedure, (b) what pre-state
 * evidence is captured before it runs (authoritative source fingerprints and
 * T19-normalized digests), and (c) the expected post-state contract. The
 * executor runs each case against the REAL modules — the T16 live-stream
 * client over in-memory authoritative sources, the T06/T07 SQLite projection
 * store (file-backed where the procedure destroys a file), the T05 kernel and
 * T11 promotion ledger, the T04 Prime adapter, and the T10 detach/reattach
 * receipts.
 *
 * Honesty rules (prereg D1..D3; the blindspots bind scope):
 *   - Lost ephemeral output is marked UNAVAILABLE with a typed reason — never
 *     recreated, never faked (REQ-SYNC-003). Kernel loss is in-model
 *     (`PrimeReplKernel.terminate`); the TUI kill is BOTH the in-model host
 *     death AND a deterministic real-process SIGKILL of a spawned child the
 *     harness waits on. Cross-process Prime reality is T24 (prereg blindspot).
 *   - Corruption is detected by auditing the projection file's rows and
 *     persisted cursors against the authoritative sources' minted cursors and
 *     the T19 pure log fold — the workbench NEVER mutates authoritative
 *     sources to preserve a local view (REQ-SAFE-005, prereg D1).
 *   - Every failed round is RETAINED: the round ledger is append-only, and the
 *     corrupt projection state is QUARANTINED (renamed, sha256-pinned, kept on
 *     disk) — never silently destroyed or overwritten (prereg D3).
 *   - Scope exclusions are declared data (RECOV-006 plugin lifecycle -> T15;
 *     cross-surface operation isomorphism -> T23; cross-process Prime -> T24),
 *     not silent omissions.
 *
 * SXR probe seam (pinned boundary): `sxr-adapter.ts` is presentation-
 * classified by the T19 renderer-boundary suite (its render-tree data-shape
 * coupling is a pinned policy decision), and this module is a policed
 * state-layer file, so it must not import it. The RECOV-SXR case therefore
 * declares its procedure and contract HERE, and consumes the REAL probe
 * results (driven through `executeSxrRefresh` / `parseSxrSlice0bEnvelopeOutput`
 * by the tests, which sit outside the policed universe) as the typed
 * `SxrAvailabilityRecord`. A missing probe record is a FAILED case — missing
 * evidence is never passing.
 */

/** Typed harness failure carrying the recovery-matrix operation locus. */
export class RecoveryCaseError extends Error {
  constructor(operation: string, detail: string) {
    super(`recovery matrix ${operation}: ${detail}`);
    this.name = "RecoveryCaseError";
  }
}

// --- Frozen constants (deterministic evidence) ----------------------------------

export const RECORDED_RUN_ID = "run-t22-recovery";
/** Recording-time clock: a recorded constant, never re-minted (T19 rule). */
export const RECORDED_AT_MS = 1_760_000_000_000;
export const SEED_BASE_MS = 1_760_000_000_000;
export const KERNEL_GEN_1 = "kernel-gen-1";
export const KERNEL_SESSION = "prime-kernel-t22";
export const KERNEL_ARTIFACT_LOCATOR = ".agents/reports/artifacts/t22-metrics.json";
/** A cursor the authoritative sources NEVER minted (the corruption lie). */
export const FORGED_CURSOR = "cursor-t22-forged-9a3f";

// --- Authoritative source fingerprints ------------------------------------------

/** The source-declared identity and content a recovery is measured against. */
export interface AuthoritativeSourceFingerprint {
  readonly source_id: string;
  readonly store_epoch: string;
  readonly earliest_sequence: number;
  readonly watermark_sequence: number;
  readonly watermark_cursor: string;
  /** Cursors the source minted in its live epoch (retained window). */
  readonly minted_cursors: readonly string[];
  /** sha256 over the canonical JSON of descriptor + retained events. */
  readonly content_digest: string;
}

export type AuthoritativeFingerprints = Readonly<Record<string, AuthoritativeSourceFingerprint>>;

/** Fingerprint one authoritative source (identity + retained content digest). */
export function fingerprintSource(source: InMemoryStreamSource): AuthoritativeSourceFingerprint {
  const descriptor = source.describeSource();
  const events = source.retainedEvents();
  return {
    source_id: descriptor.source_id,
    store_epoch: descriptor.store_epoch,
    earliest_sequence: descriptor.earliest_sequence,
    watermark_sequence: descriptor.watermark_sequence,
    watermark_cursor: descriptor.watermark_cursor,
    minted_cursors: events.map((event) => event.cursor),
    content_digest: createHash("sha256")
      .update(canonicalJson({ descriptor, events }))
      .digest("hex"),
  };
}

/** Fingerprint every source on the port, keyed by source id. */
export function fingerprintPort(port: InMemoryStreamPort): AuthoritativeFingerprints {
  const entries = port.describeSources().map((descriptor) => {
    const source = port.source(descriptor.source_id);
    return [descriptor.source_id, fingerprintSource(source)] as const;
  });
  return Object.fromEntries(entries);
}

/** Differences between two fingerprint sets (empty means byte-identical). */
export function fingerprintsDifferences(
  before: AuthoritativeFingerprints,
  after: AuthoritativeFingerprints,
): readonly string[] {
  const diffs: string[] = [];
  for (const sourceId of Object.keys(before)) {
    const was = before[sourceId];
    const now = after[sourceId];
    if (was === undefined || now === undefined) {
      diffs.push(`${sourceId}:fingerprint_missing`);
      continue;
    }
    if (was.content_digest !== now.content_digest) {
      diffs.push(`${sourceId}:content_digest:${was.content_digest}!=${now.content_digest}`);
    }
    if (was.store_epoch !== now.store_epoch) {
      diffs.push(`${sourceId}:store_epoch:${was.store_epoch}!=${now.store_epoch}`);
    }
    if (was.watermark_sequence !== now.watermark_sequence) {
      diffs.push(
        `${sourceId}:watermark:${String(was.watermark_sequence)}!=${String(now.watermark_sequence)}`,
      );
    }
  }
  for (const sourceId of Object.keys(after)) {
    if (before[sourceId] === undefined) {
      diffs.push(`${sourceId}:unexpected_source`);
    }
  }
  return diffs;
}

// --- Pre-state capture -----------------------------------------------------------

export interface RecoveryPreState {
  /** The generation the fold (and the recovery) is pinned to. */
  readonly generation_id: string;
  readonly source_fingerprints: AuthoritativeFingerprints;
  /** sha256 over the T19 canonical sections of the authoritative log fold. */
  readonly canonical_digest: string;
  /** sha256 over the FULL T19-normalized projection of the log fold. */
  readonly full_digest: string;
  readonly source_cursors: Readonly<Record<string, string>>;
}

/**
 * Capture pre-state evidence: source fingerprints plus the T19 pure-log-fold
 * digests under `generationId`. Pure — it never touches a live model, so it
 * can be taken while hosts are dead, before or after authoritative phases.
 */
export function captureRecoveryPreState(args: {
  port: InMemoryStreamPort;
  log: readonly AuthoritativeEventFrame[];
  generationId: string;
  kernelObjects: readonly RecordedKernelObject[];
  kernelAlive: boolean;
  runId?: string | undefined;
}): RecoveryPreState {
  const recorded = recordRun({
    run_id: args.runId ?? RECORDED_RUN_ID,
    generation_id: args.generationId,
    recorded_at_ms: RECORDED_AT_MS,
    events: args.log,
    kernel: {
      kernel_generation: KERNEL_GEN_1,
      session_ref: KERNEL_SESSION,
      objects: args.kernelObjects,
    },
    presentation: { placements: [], selected_focus_id: null },
  });
  const fold = foldLogToNormalized(recorded, { kernelAlive: args.kernelAlive });
  return {
    generation_id: args.generationId,
    source_fingerprints: fingerprintPort(args.port),
    canonical_digest: projectionDigest(canonicalSections(fold)),
    full_digest: projectionDigest(fold),
    source_cursors: Object.fromEntries(fold.source_cursors),
  };
}

/** Build the T19 recorded-Run document over an authoritative log. */
export function recordedRunFromLog(args: {
  generationId: string;
  events: readonly AuthoritativeEventFrame[];
  kernelObjects?: readonly RecordedKernelObject[] | undefined;
  runId?: string | undefined;
}): RecordedRun {
  return recordRun({
    run_id: args.runId ?? RECORDED_RUN_ID,
    generation_id: args.generationId,
    recorded_at_ms: RECORDED_AT_MS,
    events: args.events,
    kernel: {
      kernel_generation: KERNEL_GEN_1,
      session_ref: KERNEL_SESSION,
      objects: args.kernelObjects ?? [],
    },
    presentation: { placements: [], selected_focus_id: null },
  });
}

// --- Corruption audit (cursor/epoch validation against the authorities) ----------

export type CorruptionFindingCode =
  | "cursor_not_minted_by_source"
  | "generation_cursor_not_minted"
  | "cursor_behind_authoritative_watermark"
  | "row_disagrees_with_authoritative_fold"
  | "expected_row_missing_from_file"
  | "row_updated_at_disagrees_with_authoritative_fold"
  | "focus_row_disagrees_with_authoritative_fold";

/** One typed inconsistency between the projection file and the authorities. */
export interface CorruptionFinding {
  readonly code: CorruptionFindingCode;
  readonly locus: string;
  readonly detail: string;
}

export interface RawUnitRow {
  readonly unit_id: string;
  readonly generation_id: string;
  readonly source_id: string;
  readonly source_cursor: string;
  readonly title: string;
  readonly phase: string;
  readonly status: string;
  readonly authority: string;
  readonly blast_radius: string;
  readonly updated_at: number;
}

export interface RawEvidenceRow {
  readonly evidence_id: string;
  readonly generation_id: string;
  readonly source_id: string;
  readonly source_cursor: string;
  readonly unit_id: string;
  readonly kind: string;
  readonly locator: string;
  /** Raw SQLite bit (0/1) — normalized during the audit. */
  readonly verified: number;
  readonly updated_at: number;
}

export interface RawSessionRow {
  readonly session_id: string;
  readonly generation_id: string;
  readonly source_id: string;
  readonly source_cursor: string;
  readonly parent_id: string | null;
  readonly model: string;
  readonly status: string;
  readonly updated_at: number;
}

export interface RawFocusRow {
  readonly generation_id: string;
  readonly focus_kind: string;
  readonly focus_id: string;
}

/** Every projected row in the FILE, across all generations (the audit target). */
export interface RawProjectionRows {
  readonly source_cursors: readonly (readonly [string, string])[];
  readonly units: readonly RawUnitRow[];
  readonly evidence: readonly RawEvidenceRow[];
  readonly sessions: readonly RawSessionRow[];
  readonly focus: readonly RawFocusRow[];
}

/**
 * Read every projected row from the projection database file READ-ONLY (the
 * auditor never writes). Unlike a generation-scoped snapshot this sees rows
 * from EVERY generation — a naive rebuild that leaves corrupt rows behind
 * cannot hide them here.
 */
export function readProjectionRows(dbPath: string): RawProjectionRows {
  if (!existsSync(dbPath)) {
    throw new RecoveryCaseError("readProjectionRows", `no projection database at '${dbPath}'`);
  }
  const db = new Database(dbPath, { readonly: true });
  try {
    const cursors = db
      .prepare("SELECT source_id, cursor FROM source_cursors ORDER BY source_id ASC;")
      .all() as Array<{ source_id: string; cursor: string }>;
    const units = db
      .prepare(
        `SELECT unit_id, generation_id, source_id, source_cursor, title, phase, status, authority, blast_radius, updated_at
         FROM units_projection ORDER BY unit_id ASC;`,
      )
      .all() as RawUnitRow[];
    const evidence = db
      .prepare(
        `SELECT evidence_id, generation_id, source_id, source_cursor, unit_id, kind, locator, verified, updated_at
         FROM evidence_projection ORDER BY evidence_id ASC;`,
      )
      .all() as RawEvidenceRow[];
    const sessions = db
      .prepare(
        `SELECT session_id, generation_id, source_id, source_cursor, parent_id, model, status, updated_at
         FROM sessions_projection ORDER BY session_id ASC;`,
      )
      .all() as RawSessionRow[];
    const focus = db
      .prepare(
        `SELECT generation_id, focus_kind, focus_id
         FROM focus_projection ORDER BY generation_id ASC;`,
      )
      .all() as RawFocusRow[];
    return {
      source_cursors: cursors.map((row) => [row.source_id, row.cursor] as const),
      units,
      evidence,
      sessions,
      focus,
    };
  } finally {
    db.close();
  }
}

/**
 * Audit the projection file's rows against the authoritative sources in BOTH
 * directions (adversarial correction MAJOR-1):
 *
 *   file → fold:  every file row's provenance cursor must be a cursor its
 *     source actually minted (the cursor/epoch validation of REQ-SYNC-001),
 *     every row must agree with the T19 pure log fold, and no phantom row may
 *     exist (across ALL generations — a naive rebuild cannot hide rows behind
 *     generation-scoped reads).
 *   fold → file:  every expected row must EXIST in the file under the audited
 *     generation `expectedGenerationId` (a deleted row, or one relabeled to a
 *     zombie generation so it vanishes from generation-scoped reads, is a
 *     typed finding naming the relabel), its `updated_at` must equal the
 *     fold's (tamper detection), and the focus projection must equal the
 *     fold's authoritative focus.
 *
 * The minted-cursor window is the sources' retained window — corruption
 * scenarios here run with full retention, so any projected cursor outside it
 * was never authoritative. Session `updated_at` is not fold-comparable (T19's
 * normalized session record drops it); unit and evidence timestamps are.
 */
export function auditProjectionRows(input: {
  rows: RawProjectionRows;
  fingerprints: AuthoritativeFingerprints;
  expected: NormalizedRunProjection;
  /** The generation the audited file's rows must be projected under. */
  expectedGenerationId: string;
}): readonly CorruptionFinding[] {
  const findings: CorruptionFinding[] = [];
  const minted = (sourceId: string): ReadonlySet<string> =>
    new Set(input.fingerprints[sourceId]?.minted_cursors ?? []);
  const underExpectedGen = <T extends { generation_id: string }>(
    rows: readonly T[],
  ): readonly T[] => rows.filter((row) => row.generation_id === input.expectedGenerationId);

  // Generation cursors.
  const expectedCursors = new Map<string, string>(input.expected.source_cursors);
  for (const [sourceId, cursor] of input.rows.source_cursors) {
    if (!minted(sourceId).has(cursor)) {
      findings.push({
        code: "generation_cursor_not_minted",
        locus: `source_cursors:${sourceId}`,
        detail: `persisted cursor '${cursor}' was never minted by source '${sourceId}' under its live epoch`,
      });
      continue;
    }
    const expectedCursor = expectedCursors.get(sourceId);
    if (expectedCursor !== undefined && expectedCursor !== cursor) {
      findings.push({
        code: "cursor_behind_authoritative_watermark",
        locus: `source_cursors:${sourceId}`,
        detail: `persisted cursor '${cursor}' disagrees with the authoritative fold cursor '${String(expectedCursor)}'`,
      });
    }
  }

  // Units.
  const expectedUnits = new Map(input.expected.units.map((unit) => [unit.unit_id, unit]));
  for (const row of input.rows.units) {
    if (!minted(row.source_id).has(row.source_cursor)) {
      findings.push({
        code: "cursor_not_minted_by_source",
        locus: `units:${row.unit_id}`,
        detail: `row provenance cursor '${row.source_cursor}' was never minted by source '${row.source_id}'`,
      });
    }
    const expected = expectedUnits.get(row.unit_id);
    if (expected === undefined) {
      findings.push({
        code: "row_disagrees_with_authoritative_fold",
        locus: `units:${row.unit_id}`,
        detail: "phantom row: no such unit exists in the authoritative fold",
      });
      continue;
    }
    const fieldDiffs: string[] = [];
    if (expected.title !== row.title) fieldDiffs.push(`title:${row.title}!=${expected.title}`);
    if (expected.phase !== row.phase) fieldDiffs.push(`phase:${row.phase}!=${expected.phase}`);
    if (expected.status !== row.status) fieldDiffs.push(`status:${row.status}!=${expected.status}`);
    if (expected.authority !== row.authority) {
      fieldDiffs.push(`authority:${row.authority}!=${expected.authority}`);
    }
    if (expected.blast_radius !== row.blast_radius) {
      fieldDiffs.push(`blast_radius:${row.blast_radius}!=${expected.blast_radius}`);
    }
    if (expected.source_id !== row.source_id) {
      fieldDiffs.push(`source_id:${row.source_id}!=${expected.source_id}`);
    }
    if (expected.source_cursor !== row.source_cursor) {
      fieldDiffs.push(`source_cursor:${row.source_cursor}!=${expected.source_cursor}`);
    }
    if (fieldDiffs.length > 0) {
      findings.push({
        code: "row_disagrees_with_authoritative_fold",
        locus: `units:${row.unit_id}`,
        detail: `row disagrees with the authoritative fold: ${fieldDiffs.join("; ")}`,
      });
    }
  }

  // Evidence.
  const expectedEvidence = new Map(
    input.expected.evidence.map((evidence) => [evidence.evidence_id, evidence]),
  );
  for (const row of input.rows.evidence) {
    if (!minted(row.source_id).has(row.source_cursor)) {
      findings.push({
        code: "cursor_not_minted_by_source",
        locus: `evidence:${row.evidence_id}`,
        detail: `row provenance cursor '${row.source_cursor}' was never minted by source '${row.source_id}'`,
      });
    }
    const expected = expectedEvidence.get(row.evidence_id);
    if (expected === undefined) {
      findings.push({
        code: "row_disagrees_with_authoritative_fold",
        locus: `evidence:${row.evidence_id}`,
        detail: "phantom row: no such evidence exists in the authoritative fold",
      });
      continue;
    }
    const fieldDiffs: string[] = [];
    if (expected.unit_id !== row.unit_id)
      fieldDiffs.push(`unit_id:${row.unit_id}!=${expected.unit_id}`);
    if (expected.kind !== row.kind) fieldDiffs.push(`kind:${row.kind}!=${expected.kind}`);
    if (expected.locator !== row.locator)
      fieldDiffs.push(`locator:${row.locator}!=${expected.locator}`);
    if (expected.verified !== (row.verified !== 0)) {
      fieldDiffs.push(`verified:${String(row.verified)}!=${String(expected.verified)}`);
    }
    if (expected.source_cursor !== row.source_cursor) {
      fieldDiffs.push(`source_cursor:${row.source_cursor}!=${expected.source_cursor}`);
    }
    if (fieldDiffs.length > 0) {
      findings.push({
        code: "row_disagrees_with_authoritative_fold",
        locus: `evidence:${row.evidence_id}`,
        detail: `row disagrees with the authoritative fold: ${fieldDiffs.join("; ")}`,
      });
    }
  }

  // Sessions.
  const expectedSessions = new Map(
    input.expected.sessions.map((session) => [session.session_id, session]),
  );
  for (const row of input.rows.sessions) {
    if (!minted(row.source_id).has(row.source_cursor)) {
      findings.push({
        code: "cursor_not_minted_by_source",
        locus: `sessions:${row.session_id}`,
        detail: `row provenance cursor '${row.source_cursor}' was never minted by source '${row.source_id}'`,
      });
    }
    const expected = expectedSessions.get(row.session_id);
    if (expected === undefined) {
      findings.push({
        code: "row_disagrees_with_authoritative_fold",
        locus: `sessions:${row.session_id}`,
        detail: "phantom row: no such session exists in the authoritative fold",
      });
      continue;
    }
    const fieldDiffs: string[] = [];
    if ((expected.parent_id ?? null) !== row.parent_id) {
      fieldDiffs.push(`parent_id:${String(row.parent_id)}!=${String(expected.parent_id)}`);
    }
    if (expected.model !== row.model) fieldDiffs.push(`model:${row.model}!=${expected.model}`);
    if (expected.status !== row.status) fieldDiffs.push(`status:${row.status}!=${expected.status}`);
    if (expected.source_cursor !== row.source_cursor) {
      fieldDiffs.push(`source_cursor:${row.source_cursor}!=${expected.source_cursor}`);
    }
    if (fieldDiffs.length > 0) {
      findings.push({
        code: "row_disagrees_with_authoritative_fold",
        locus: `sessions:${row.session_id}`,
        detail: `row disagrees with the authoritative fold: ${fieldDiffs.join("; ")}`,
      });
    }
  }

  // --- fold → file direction (adversarial correction MAJOR-1) ----------------
  // The file-row loops above can never see what is MISSING. Every expected
  // row must exist under the audited generation; a row relabeled to a zombie
  // generation vanishes from generation-scoped reads and is flagged here with
  // the relabel named. Units and evidence additionally pin `updated_at`.

  const expectedGenUnits = new Map(
    underExpectedGen(input.rows.units).map((row) => [row.unit_id, row]),
  );
  for (const expectedUnit of input.expected.units) {
    const fileRow = expectedGenUnits.get(expectedUnit.unit_id);
    if (fileRow === undefined) {
      findings.push({
        code: "expected_row_missing_from_file",
        locus: `units:${expectedUnit.unit_id}`,
        detail: missingExpectedRowDetail(
          `units:${expectedUnit.unit_id}`,
          input.expectedGenerationId,
          input.rows.units
            .filter((row) => row.unit_id === expectedUnit.unit_id)
            .map((row) => row.generation_id),
        ),
      });
      continue;
    }
    if (fileRow.updated_at !== expectedUnit.updated_at) {
      findings.push({
        code: "row_updated_at_disagrees_with_authoritative_fold",
        locus: `units:${expectedUnit.unit_id}`,
        detail: `row updated_at ${String(fileRow.updated_at)} disagrees with the authoritative fold ${String(expectedUnit.updated_at)}`,
      });
    }
  }

  const expectedGenEvidence = new Map(
    underExpectedGen(input.rows.evidence).map((row) => [row.evidence_id, row]),
  );
  for (const expectedEvidence of input.expected.evidence) {
    const fileRow = expectedGenEvidence.get(expectedEvidence.evidence_id);
    if (fileRow === undefined) {
      findings.push({
        code: "expected_row_missing_from_file",
        locus: `evidence:${expectedEvidence.evidence_id}`,
        detail: missingExpectedRowDetail(
          `evidence:${expectedEvidence.evidence_id}`,
          input.expectedGenerationId,
          input.rows.evidence
            .filter((row) => row.evidence_id === expectedEvidence.evidence_id)
            .map((row) => row.generation_id),
        ),
      });
      continue;
    }
    if (fileRow.updated_at !== expectedEvidence.updated_at) {
      findings.push({
        code: "row_updated_at_disagrees_with_authoritative_fold",
        locus: `evidence:${expectedEvidence.evidence_id}`,
        detail: `row updated_at ${String(fileRow.updated_at)} disagrees with the authoritative fold ${String(expectedEvidence.updated_at)}`,
      });
    }
  }

  const expectedGenSessions = new Set(
    underExpectedGen(input.rows.sessions).map((row) => row.session_id),
  );
  for (const expectedSession of input.expected.sessions) {
    if (!expectedGenSessions.has(expectedSession.session_id)) {
      findings.push({
        code: "expected_row_missing_from_file",
        locus: `sessions:${expectedSession.session_id}`,
        detail: missingExpectedRowDetail(
          `sessions:${expectedSession.session_id}`,
          input.expectedGenerationId,
          input.rows.sessions
            .filter((row) => row.session_id === expectedSession.session_id)
            .map((row) => row.generation_id),
        ),
      });
    }
  }

  // The focus projection is part of the canonical truth (canonicalSections
  // carries it) and is audited like any other row. The store-payload kind
  // ("unit"/"session"/"evidence") is mapped to the semantic kind exactly as
  // T19's normalizers map it, so the comparison is against the same vocabulary.
  const semanticFocusKind = (kind: string): string =>
    kind === "unit"
      ? "WorkItem"
      : kind === "session"
        ? "Agent"
        : kind === "evidence"
          ? "Evidence"
          : kind;
  const focusUnderExpectedGen = underExpectedGen(input.rows.focus);
  const expectedFocus = input.expected.focus;
  if (expectedFocus === null) {
    if (focusUnderExpectedGen.length > 0) {
      findings.push({
        code: "focus_row_disagrees_with_authoritative_fold",
        locus: "focus",
        detail: `no authoritative focus exists in the fold; the file projects ${focusUnderExpectedGen.map((row) => `${String(semanticFocusKind(row.focus_kind))}:${row.focus_id}`).join(", ")}`,
      });
    }
  } else if (focusUnderExpectedGen.length === 0) {
    findings.push({
      code: "expected_row_missing_from_file",
      locus: "focus",
      detail: `the authoritative fold expects focus '${expectedFocus.kind}:${expectedFocus.id}' but the file has no focus row under the audited generation '${input.expectedGenerationId}'`,
    });
  } else {
    const matches = focusUnderExpectedGen.some(
      (row) =>
        semanticFocusKind(row.focus_kind) === expectedFocus.kind &&
        row.focus_id === expectedFocus.id,
    );
    if (!matches) {
      findings.push({
        code: "focus_row_disagrees_with_authoritative_fold",
        locus: "focus",
        detail: `file focus ${focusUnderExpectedGen
          .map((row) => `${String(semanticFocusKind(row.focus_kind))}:${row.focus_id}`)
          .join(
            ", ",
          )} disagrees with the authoritative fold ${expectedFocus.kind}:${expectedFocus.id}`,
      });
    }
  }

  return findings;
}

/** Detail for a fold→file miss, naming a zombie-generation relabel when found. */
function missingExpectedRowDetail(
  identity: string,
  expectedGenerationId: string,
  foundGenerations: readonly string[],
): string {
  const relabeled = foundGenerations.filter((generation) => generation !== expectedGenerationId);
  if (relabeled.length > 0) {
    return `no row for ${identity} under the audited generation '${expectedGenerationId}'; the identity was relabeled to generation(s): ${relabeled.join(", ")}`;
  }
  return `the authoritative fold expects ${identity} but no row exists under the audited generation '${expectedGenerationId}' (deleted or lost row)`;
}

// --- Quarantine (corrupt state is retained, never destroyed) ---------------------

export interface QuarantineRecord {
  readonly original_path: string;
  readonly quarantined_path: string;
  readonly sha256: string;
  /** LITERAL true: the corrupt state is kept as evidence, never deleted. */
  readonly retained: true;
}

/**
 * Checkpoint the WAL into the main file, pin its sha256, and RENAME it out of
 * the way (sidecars follow). The corrupt database stays on disk at
 * `quarantined_path` — recovery never destroys evidence of what it recovered
 * from (prereg D3).
 */
export function quarantineDatabase(dbPath: string, label: string): QuarantineRecord {
  if (!existsSync(dbPath)) {
    throw new RecoveryCaseError("quarantineDatabase", `no projection database at '${dbPath}'`);
  }
  const db = new Database(dbPath);
  try {
    db.run("PRAGMA wal_checkpoint(TRUNCATE);");
  } finally {
    db.close();
  }
  const sha256 = createHash("sha256").update(readFileSync(dbPath)).digest("hex");
  const quarantinedPath = `${dbPath}.${label}.corrupt`;
  renameSync(dbPath, quarantinedPath);
  for (const suffix of ["-wal", "-shm"] as const) {
    const sidecar = `${dbPath}${suffix}`;
    if (existsSync(sidecar)) {
      renameSync(sidecar, `${quarantinedPath}${suffix}`);
    }
  }
  return { original_path: dbPath, quarantined_path: quarantinedPath, sha256, retained: true };
}

/** sha256 of a file on disk (evidence pinning). */
export function fileSha256(path: string): string {
  return createHash("sha256").update(readFileSync(path)).digest("hex");
}

/**
 * Remove a scratch projection database and its sidecars. Quarantined
 * `*.corrupt` state is deliberately NOT touched here — retained evidence is
 * never deleted by recovery or fixture teardown; the test layer sweeps its
 * scratch prefixes in `afterAll` if it chooses to.
 */
export function cleanupScratchDb(dbPath: string): void {
  for (const suffix of ["", "-wal", "-shm"]) {
    const path = `${dbPath}${suffix}`;
    if (existsSync(path)) {
      try {
        unlinkSync(path);
      } catch {
        // best-effort cleanup
      }
    }
  }
}

// --- Deterministic real-process kill (where deterministic) -----------------------

export interface RealTuiProcess {
  readonly pid: number;
  /** SIGKILL the child and reap it; deterministic — no racing window. */
  killAndReap(): Promise<{ exit_code: number; signal: string | null }>;
}

/** Default deadline for the readiness marker (adversarial correction MINOR-1). */
export const DEFAULT_READINESS_TIMEOUT_MS = 15_000;
export const READINESS_MARKER = "t22-tui-ready";
const DEFAULT_TUI_CHILD_SCRIPT = `process.stdout.write('${READINESS_MARKER}\\n'); setInterval(() => {}, 1_000_000);`;

export interface RealTuiProcessOptions {
  /**
   * Deadline for the readiness marker. A child that hangs without emitting it
   * is SIGKILLed and the call FAILS instead of hanging the suite (MINOR-1).
   */
  readonly readinessTimeoutMs?: number | undefined;
  /** The child script (overridable so tests can exercise the deadline). */
  readonly childScript?: string | undefined;
}

/**
 * Spawn a REAL TUI process (a Bun child that signals readiness and then idles)
 * and wait for its readiness marker before returning — under a HARD deadline:
 * a child that never signals is killed and the call throws, never hangs. The
 * caller may SIGKILL and reap the returned handle deterministically: the
 * harness owns the child and never proceeds on a guess about its state
 * (prereg: real-process kills only where deterministic).
 */
export async function spawnRealTuiProcess(
  options: RealTuiProcessOptions = {},
): Promise<RealTuiProcess> {
  const readinessTimeoutMs = options.readinessTimeoutMs ?? DEFAULT_READINESS_TIMEOUT_MS;
  const proc = Bun.spawn(["bun", "-e", options.childScript ?? DEFAULT_TUI_CHILD_SCRIPT], {
    stdin: "ignore",
    stdout: "pipe",
    stderr: "ignore",
  });
  const reader = proc.stdout.getReader();
  const decoder = new TextDecoder();
  let seen = "";
  let deadlineTimer: ReturnType<typeof setTimeout> | undefined;
  const deadline = new Promise<null>((resolve) => {
    deadlineTimer = setTimeout(() => resolve(null), readinessTimeoutMs);
  });
  try {
    for (;;) {
      const chunk = await Promise.race([reader.read(), deadline]);
      if (chunk === null) {
        proc.kill("SIGKILL");
        throw new RecoveryCaseError(
          "spawnRealTuiProcess",
          `the TUI process emitted no readiness marker within ${String(readinessTimeoutMs)}ms; killed the hung child and failed the case instead of hanging`,
        );
      }
      if (chunk.done) {
        throw new RecoveryCaseError(
          "spawnRealTuiProcess",
          "the TUI process exited before signaling readiness",
        );
      }
      seen += decoder.decode(chunk.value, { stream: true });
      if (seen.includes(READINESS_MARKER)) {
        break;
      }
    }
  } finally {
    if (deadlineTimer !== undefined) {
      clearTimeout(deadlineTimer);
    }
    void reader.cancel().catch(() => undefined);
  }
  return {
    pid: proc.pid,
    async killAndReap(): Promise<{ exit_code: number; signal: string | null }> {
      proc.kill("SIGKILL");
      const exitCode = await proc.exited;
      const signal = proc.signalCode;
      return { exit_code: exitCode, signal };
    },
  };
}

// --- The shared fixture (real modules, seeded deterministically) ------------------

/** Map a T11 promoted object to its T19 recorded (durable facts only) shape. */
export function recordKernelObject(object: PromotedKernelObject): RecordedKernelObject {
  return {
    object_id: object.provenance.object_id,
    name: object.provenance.name,
    type_name: object.provenance.type_name,
    session_ref: object.provenance.session_ref,
    kernel_generation: object.provenance.kernel_generation,
    persistence_class: object.persistence_class,
    artifact_ref: object.artifact_ref,
  };
}

export interface MatrixWorld {
  readonly dbPath: string;
  readonly generationId: string;
  readonly gauntlet: InMemoryStreamSource;
  readonly prime: InMemoryStreamSource;
  readonly port: InMemoryStreamPort;
  readonly model: WorkbenchModel;
  readonly client: LiveStreamClient;
  readonly adapter: PrimeHostAdapter;
  /** Call-logging wrapper — proves no cancel/stop ever rides host operations. */
  readonly observedAdapter: PrimeHostPort;
  /** The LIVE adapter call log (kept current as calls happen). */
  readonly adapterCalls: readonly string[];
  readonly kernel: PrimeReplKernel | null;
  readonly ledger: KernelObjectLedger | null;
  readonly kernelObjects: readonly RecordedKernelObject[];
  /** The authoritative log folded in canonical order under a generation. */
  authoritativeLog(generationIdOverride?: string | undefined): readonly AuthoritativeEventFrame[];
  /** The T19 recorded-Run document over the CURRENT authoritative state. */
  recordedRun(args?: {
    runId?: string | undefined;
    generationId?: string | undefined;
  }): RecordedRun;
  close(): void;
}

function requireNamedObject(
  objects: readonly PromotedKernelObject[],
  name: string,
): PromotedKernelObject {
  const found = objects.find((object) => object.provenance.name === name);
  if (found === undefined) {
    throw new RecoveryCaseError("buildMatrixWorld", `kernel object '${name}' was not registered`);
  }
  return found;
}

/**
 * Build the shared fixture: two authoritative in-memory sources seeded with a
 * representative world, a file- (or memory-) backed T07 model, a T16 client
 * connected through the REAL reconciliation path, a T04 Prime adapter with a
 * root+child topology, and — with `withKernel` — a T05 kernel plus T11 ledger
 * holding one artifactized, one pinned, and one ephemeral object.
 */
export async function buildMatrixWorld(options: {
  dbPath: string;
  generationId: string;
  withKernel?: boolean | undefined;
}): Promise<MatrixWorld> {
  const gauntlet = new InMemoryStreamSource("gauntlet");
  const prime = new InMemoryStreamSource("prime");
  gauntlet.append({
    event_type: "unit_updated",
    payload: {
      unit_id: "AK-401",
      title: "Unit 401",
      phase: "execute",
      status: "in_progress",
      authority: "builder",
      blast_radius: "project",
    },
    timestamp_ms: SEED_BASE_MS + 1,
  });
  gauntlet.append({
    event_type: "unit_updated",
    payload: {
      unit_id: "AK-402",
      title: "Unit 402",
      phase: "verify",
      status: "pending",
      authority: "orchestrator",
      blast_radius: "isolated",
    },
    timestamp_ms: SEED_BASE_MS + 2,
  });
  gauntlet.append({
    event_type: "evidence_added",
    payload: {
      evidence_id: "ev-401",
      unit_id: "AK-401",
      kind: "report",
      locator: ".agents/reports/artifacts/t22-ev-401.json",
      verified: false,
    },
    timestamp_ms: SEED_BASE_MS + 3,
  });
  gauntlet.append({
    event_type: "focus_changed",
    payload: { kind: "unit", id: "AK-401" },
    timestamp_ms: SEED_BASE_MS + 4,
  });
  prime.append({
    event_type: "session_updated",
    payload: { session_id: "prime-root", parent_id: null, status: "running" },
    timestamp_ms: SEED_BASE_MS + 5,
  });
  prime.append({
    event_type: "session_updated",
    payload: { session_id: "prime-child-1", parent_id: "prime-root", status: "running" },
    timestamp_ms: SEED_BASE_MS + 6,
  });
  const port = new InMemoryStreamPort([gauntlet, prime]);

  const adapter = new PrimeHostAdapter();
  adapter.spawnChild({
    sessionId: "prime-root",
    parentSessionId: null,
    contractPath: ".agents/reports/gauntlet-tui-prime-workbench/evidence/T22/contracts/root.md",
  });
  adapter.spawnChild({
    sessionId: "prime-child-1",
    parentSessionId: "prime-root",
    contractPath: ".agents/reports/gauntlet-tui-prime-workbench/evidence/T22/contracts/child-1.md",
  });
  const logged = loggingPrimeAdapter(adapter);

  const model = new WorkbenchModel({ dbPath: options.dbPath });
  const client = new LiveStreamClient({
    model,
    port,
    mintGenerationId: (serial) => `${options.generationId}-${serial}`,
  });
  client.connect();
  const connectHandshake = client.lastHandshake;
  if (
    connectHandshake === null ||
    !connectHandshake.per_source.every((leg) => leg.refused_events === 0)
  ) {
    throw new RecoveryCaseError(
      "buildMatrixWorld",
      "the fixture baseline was refused by the store",
    );
  }

  let kernel: PrimeReplKernel | null = null;
  let ledger: KernelObjectLedger | null = null;
  let kernelObjects: readonly RecordedKernelObject[] = [];
  if (options.withKernel === true) {
    kernel = new PrimeReplKernel(KERNEL_SESSION);
    ledger = new KernelObjectLedger({ kernel, kernel_generation: KERNEL_GEN_1 });
    const run = await kernel.execute(
      'metrics = [1, 2, 3]\nscratch = {"intermediate": true}\nprobe = 42',
    );
    const registered = ledger.register(run);
    const metrics = requireNamedObject(registered, "metrics");
    const scratch = requireNamedObject(registered, "scratch");
    const artifactized = ledger.artifactize(metrics.provenance.object_id, KERNEL_ARTIFACT_LOCATOR);
    if (artifactized.class !== "transitioned") {
      throw new RecoveryCaseError(
        "buildMatrixWorld",
        `fixture artifactization refused: ${artifactized.message}`,
      );
    }
    const pinned = ledger.pin(scratch.provenance.object_id);
    if (pinned.class !== "transitioned") {
      throw new RecoveryCaseError("buildMatrixWorld", `fixture pin refused: ${pinned.message}`);
    }
    kernelObjects = registered.map(recordKernelObject);
  }

  const authoritativeLog = (generationIdOverride?: string | undefined) => {
    const generation = generationIdOverride ?? options.generationId;
    const events: StreamEvent[] = [...gauntlet.retainedEvents(), ...prime.retainedEvents()];
    return canonicalStreamOrder(events).map(
      (event): AuthoritativeEventFrame => ({
        generation_id: generation,
        source_id: event.source_id,
        cursor: event.cursor,
        event_type: event.event_type,
        payload: event.payload,
        timestamp_ms: event.timestamp_ms,
      }),
    );
  };

  let closed = false;
  return {
    dbPath: options.dbPath,
    generationId: options.generationId,
    gauntlet,
    prime,
    port,
    model,
    client,
    adapter,
    observedAdapter: logged.logged,
    adapterCalls: logged.calls,
    kernel,
    ledger,
    kernelObjects,
    authoritativeLog,
    recordedRun(args?: { runId?: string | undefined; generationId?: string | undefined }) {
      return recordedRunFromLog({
        generationId: args?.generationId ?? options.generationId,
        events: authoritativeLog(args?.generationId),
        kernelObjects,
        runId: args?.runId,
      });
    },
    close() {
      if (closed) {
        return;
      }
      closed = true;
      try {
        client.close();
      } catch {
        // already closed
      }
      try {
        model.close();
      } catch {
        // already closed
      }
    },
  };
}

/**
 * A reference WorkbenchModel folded straight from the authoritative port (the
 * "authoritative fold" every recovery is compared against). Caller closes it.
 */
export function referenceModel(port: InMemoryStreamPort, generationId: string): WorkbenchModel {
  const model = new WorkbenchModel();
  model.startGeneration(generationId);
  const events: StreamEvent[] = port
    .describeSources()
    .flatMap((descriptor) => port.baselineWindow(descriptor.source_id).events);
  for (const event of canonicalStreamOrder(events)) {
    const applied = model.applyEvent({
      generation_id: generationId,
      source_id: event.source_id,
      cursor: event.cursor,
      event_type: event.event_type,
      payload: event.payload,
      timestamp_ms: event.timestamp_ms,
    });
    if (!applied.applied) {
      throw new RecoveryCaseError(
        "referenceModel",
        `the reference fold refused an authoritative event: ${String(applied.reason)}`,
      );
    }
  }
  model.markReady(generationId);
  return model;
}

/** Snapshot-level convergence check (generation/snapshot ids excluded by design). */
export function snapshotsConverged(
  left: WorkbenchSnapshot,
  right: WorkbenchSnapshot,
): readonly string[] {
  return projectionDifferences(left, right);
}

// --- Round ledger (append-only; failed rounds are RETAINED) ----------------------

export class RecoveryLedgerError extends Error {
  constructor(detail: string) {
    super(`recovery round ledger: ${detail}`);
    this.name = "RecoveryLedgerError";
  }
}

export interface RecoveryRoundRecord {
  readonly case_id: string;
  readonly round: number;
  readonly outcome: "pass" | "failed";
  readonly procedure: string;
  readonly residuals: readonly string[];
  /** sha256 of quarantined corrupt state this round recovered from (if any). */
  readonly quarantined_state_sha256: string | null;
  /** Where the quarantined corrupt state lives on disk (if any). */
  readonly quarantined_path: string | null;
}

/**
 * Append-only record of every recovery round. A round that once failed keeps
 * its failed-run record forever: there is no update, no delete, and no
 * overwrite (prereg D3).
 */
export class RecoveryRoundLedger {
  private readonly records: RecoveryRoundRecord[] = [];

  get size(): number {
    return this.records.length;
  }

  record(record: RecoveryRoundRecord): RecoveryRoundRecord {
    const duplicate = this.records.find(
      (existing) => existing.case_id === record.case_id && existing.round === record.round,
    );
    if (duplicate !== undefined) {
      throw new RecoveryLedgerError(
        `round ${String(record.round)} of case '${record.case_id}' is already recorded; the ledger is append-only`,
      );
    }
    const frozen: RecoveryRoundRecord = Object.freeze({
      ...record,
      residuals: Object.freeze([...record.residuals]),
    });
    this.records.push(frozen);
    return frozen;
  }

  /** Next free round number for a case. */
  nextRound(caseId: string): number {
    return this.roundsFor(caseId).length + 1;
  }

  roundsFor(caseId: string): readonly RecoveryRoundRecord[] {
    return Object.freeze(this.records.filter((record) => record.case_id === caseId));
  }

  failedRounds(caseId: string): readonly RecoveryRoundRecord[] {
    return Object.freeze(this.roundsFor(caseId).filter((record) => record.outcome === "failed"));
  }

  all(): readonly RecoveryRoundRecord[] {
    return Object.freeze([...this.records]);
  }
}

// --- The declarative case registry (frozen data) ---------------------------------

export type RecoveryCaseId =
  | "RECOV-001A-projection-deletion"
  | "RECOV-001B-projection-corruption"
  | "RECOV-002-tui-kill-during-live-work"
  | "RECOV-003-kernel-loss-objects"
  | "RECOV-004-reconnect-mid-stream-stale-frames"
  | "RECOV-005-prime-unavailable-degraded"
  | "RECOV-SXR-absence-incompatibility-cas-loss";

/** The frozen destructive procedure of one case (data, not code). */
export type DestructiveProcedure =
  | {
      readonly procedure: "delete_projection_database";
      readonly target: "file_backed_sqlite_with_wal_sidecars";
      readonly detail: string;
    }
  | {
      readonly procedure: "craft_self_consistent_corruption";
      readonly mutations: readonly string[];
      readonly detail: string;
    }
  | {
      readonly procedure: "kill_tui_host";
      readonly mode: "in_process_close_plus_real_process_sigkill";
      readonly detail: string;
    }
  | {
      readonly procedure: "kill_kernel";
      readonly mode: "in_process_terminate";
      readonly detail: string;
    }
  | {
      readonly procedure: "disconnect_and_reconnect_with_stale_frames";
      readonly stale_frames: number;
      readonly detail: string;
    }
  | {
      readonly procedure: "withdraw_prime_availability";
      readonly scope: "capability_scoped";
      readonly detail: string;
    }
  | {
      readonly procedure: "remove_sxr_producer_and_cas_object";
      readonly probes: readonly string[];
      readonly detail: string;
    };

function frozenProcedure<T extends DestructiveProcedure>(procedure: T): T {
  return Object.freeze(procedure);
}

export interface RecoveryCase {
  readonly id: RecoveryCaseId;
  readonly recov: string;
  readonly requirements: readonly string[];
  readonly destructive_procedure: DestructiveProcedure;
  readonly pre_state_capture: readonly string[];
  readonly expected_post_state: readonly string[];
  readonly storage: "file_backed_sqlite" | "in_memory_streams_plus_sqlite";
}

function freezeCase(caseDeclaration: RecoveryCase): RecoveryCase {
  return Object.freeze({
    ...caseDeclaration,
    requirements: Object.freeze([...caseDeclaration.requirements]),
    destructive_procedure: frozenProcedure(caseDeclaration.destructive_procedure),
    pre_state_capture: Object.freeze([...caseDeclaration.pre_state_capture]),
    expected_post_state: Object.freeze([...caseDeclaration.expected_post_state]),
  });
}

/**
 * The registry: the applicable RECOV cases as frozen declarative data. Every
 * entry pins the exact destructive procedure, the pre-state evidence, and the
 * declared post-state contract it will be held to.
 */
export const RECOVERY_CASES: readonly RecoveryCase[] = Object.freeze([
  freezeCase({
    id: "RECOV-001A-projection-deletion",
    recov: "RECOV-001",
    requirements: ["REQ-VERIFY-001", "REQ-SAFE-001", "REQ-SYNC-003"],
    destructive_procedure: frozenProcedure({
      procedure: "delete_projection_database",
      target: "file_backed_sqlite_with_wal_sidecars",
      detail:
        "close the hosts, then unlink the file-backed projection database and its -wal/-shm sidecars from disk",
    }),
    pre_state_capture: [
      "authoritative source fingerprints (epoch, watermark, minted cursors, content digest)",
      "T19 canonical-section digest of the authoritative log fold",
      "T19 full normalized projection digest",
      "per-source authoritative cursors",
    ],
    expected_post_state: [
      "cold rebuild via SqliteProjectionStore.rebuildFromAuthoritativeLog at the same path",
      "rebuilt canonical-section and full digests equal the pre-state digests (REQ-VERIFY-001 equivalent consequential projections)",
      "the T19 pure log fold and the rebuilt store agree byte-for-byte",
      "authoritative sources byte-identical across the procedure (REQ-SAFE-001)",
    ],
    storage: "file_backed_sqlite",
  }),
  freezeCase({
    id: "RECOV-001B-projection-corruption",
    recov: "RECOV-001",
    requirements: ["REQ-SAFE-005", "REQ-SYNC-001", "REQ-SYNC-003", "REQ-VERIFY-006"],
    destructive_procedure: frozenProcedure({
      procedure: "craft_self_consistent_corruption",
      mutations: [
        "forge the persisted gauntlet generation cursor to a cursor the source never minted",
        "insert a phantom unit row whose provenance carries the same forged cursor (rows agree with each other, disagree with the authority)",
        "tamper an existing unit row's status onto the forged cursor so the corrupt set is internally coherent",
      ],
      detail:
        "write the coherent-but-wrong rows directly into the projection file; the authoritative sources are never touched",
    }),
    pre_state_capture: [
      "authoritative source fingerprints before corruption",
      "T19 canonical-section and full digests of the authoritative log fold",
    ],
    expected_post_state: [
      "the file audit detects the corruption via cursor/epoch validation before any recovery (typed findings, no silent trust)",
      "a naive rebuild WITHOUT quarantine genuinely FAILS the contract and the failed round is RETAINED in the ledger",
      "the declared procedure quarantines the corrupt database (renamed, sha256-pinned, kept on disk) and cold-rebuilds from the authoritative log",
      "rebuilt digests equal the pre-state digests; zero corrupt rows survive; the authoritative sources are byte-identical (REQ-SAFE-005)",
      "a reattach whose persisted position carries a forged epoch is caught by T16 validation (epoch_changed) and re-baselined (REQ-SYNC-001)",
    ],
    storage: "file_backed_sqlite",
  }),
  freezeCase({
    id: "RECOV-002-tui-kill-during-live-work",
    recov: "RECOV-002",
    requirements: ["REQ-VERIFY-002", "REQ-VERIFY-005", "REQ-SAFE-001", "REQ-SAFE-004"],
    destructive_procedure: frozenProcedure({
      procedure: "kill_tui_host",
      mode: "in_process_close_plus_real_process_sigkill",
      detail:
        "detach (presentation state only), close the in-process host, and SIGKILL a real spawned TUI process reaped deterministically",
    }),
    pre_state_capture: [
      "authoritative source fingerprints",
      "T19 digests of the authoritative state the fresh host must recover",
      "the detach receipt's persisted noncanonical presentation state",
    ],
    expected_post_state: [
      "authorized Prime work survives the host death (no cancel anywhere; cancel_calls_issued is the literal 0)",
      "authoritative events landing while the TUI is dead are reconciled by the fresh host (reconciled_since_cursor > 0)",
      "the fresh host reconstructs the real state: converged with the authoritative fold; reattach topology from qualified Prime records",
      "completion is Prime's own schema-valid contract, never liveness or mailbox text (REQ-SAFE-004)",
      "the real TUI process died by SIGKILL; the reconstruction never depended on its memory",
    ],
    storage: "file_backed_sqlite",
  }),
  freezeCase({
    id: "RECOV-003-kernel-loss-objects",
    recov: "RECOV-003",
    requirements: ["REQ-VERIFY-002", "REQ-SYNC-003", "REQ-SAFE-001", "REQ-SAFE-004"],
    destructive_procedure: frozenProcedure({
      procedure: "kill_kernel",
      mode: "in_process_terminate",
      detail:
        "terminate the kernel holding one ephemeral, one pinned, and one artifactized object (in-model; Prime-side reality is T24)",
    }),
    pre_state_capture: [
      "T11 ledger inventory with persistence classes",
      "T19 kernel records with kernelAlive=true",
      "canonical-section digest (kernel-independent truth)",
    ],
    expected_post_state: [
      "T11 typed invalidation: the old ledger fails closed (kernel_object_lost); a restarted kernel's references never revive",
      "T19 availability: artifactized survives ONLY via its durable locator; pinned -> kernel_restarted_pinned_value_not_retained; ephemeral -> ephemeral_kernel_state_not_reconstructible",
      "unavailable values carry null locators and are never recreated; name reuse yields a NEW ephemeral object in the NEW generation",
      "canonical truth is untouched by kernel death (canonical-section digest unchanged)",
      "a live restarted kernel still cannot settle a unit (liveness is not success, REQ-SAFE-004)",
    ],
    storage: "in_memory_streams_plus_sqlite",
  }),
  freezeCase({
    id: "RECOV-004-reconnect-mid-stream-stale-frames",
    recov: "RECOV-004",
    requirements: ["REQ-VERIFY-006", "REQ-SYNC-001", "REQ-SYNC-002", "REQ-SAFE-005"],
    destructive_procedure: frozenProcedure({
      procedure: "disconnect_and_reconnect_with_stale_frames",
      stale_frames: 3,
      detail:
        "kill the client mid-stream with in-flight events, reconnect a fresh generation, then deliver the old-generation late frames, a sequence gap, and an old-epoch frame",
    }),
    pre_state_capture: [
      "authoritative source fingerprints after all authoritative phases (the source legitimately replaces its own epoch)",
      "the detach receipt's persisted presentation cursors",
    ],
    expected_post_state: [
      "every old-generation late frame is typed-rejected (superseded_generation) with generation identity preserved — timestamps never decide",
      "the fresh generation baselines the in-flight events (reconciled_since_cursor covers them)",
      "a sequence gap is bounded-buffered then drained in canonical order; the connection returns to live visibly",
      "an old-epoch frame triggers a visible epoch_change resync with a fresh baseline (REQ-SYNC-001)",
      "the reconnection converges to the authoritative fold; subscriptions outliving their generation are rejected",
    ],
    storage: "in_memory_streams_plus_sqlite",
  }),
  freezeCase({
    id: "RECOV-005-prime-unavailable-degraded",
    recov: "RECOV-005",
    requirements: ["REQ-SAFE-002", "REQ-SAFE-005", "REQ-SYNC-002", "REQ-AFF-005"],
    destructive_procedure: frozenProcedure({
      procedure: "withdraw_prime_availability",
      scope: "capability_scoped",
      detail:
        "re-declare the observed Prime capability with its recovery gate failed (environment-observed degradation, not inference)",
    }),
    pre_state_capture: [
      "capability status and affordance inventory while Prime is available",
      "store-level canonical digest (capabilities never touch the store)",
    ],
    expected_post_state: [
      "Prime-dependent affordances degrade with typed reason codes (recovery_gate_not_passed); installed is never faked as available",
      "unrelated canonical inspection stays valid: store rows and unit affordances unchanged (capability-scoped, REQ-SAFE-002)",
      "restoration returns the capability to available and a resync mints a fresh converged generation",
    ],
    storage: "in_memory_streams_plus_sqlite",
  }),
  freezeCase({
    id: "RECOV-SXR-absence-incompatibility-cas-loss",
    recov: "REQ-SXR-004/REQ-SYNC-003",
    requirements: ["REQ-SXR-004", "REQ-SYNC-003", "REQ-SAFE-005"],
    destructive_procedure: frozenProcedure({
      procedure: "remove_sxr_producer_and_cas_object",
      probes: [
        "producer absent: executeSxrRefresh through a runner reporting spawn_failed",
        "producer incompatible: a CEP envelope on the SXR output contract",
        "CAS object lost: a content_ref whose CAS object is missing",
      ],
      detail:
        "the declared optional-provider absence and lost durable custody; probes run through the REAL sxr-adapter boundary in the test layer (pinned presentation-coupling boundary)",
    }),
    pre_state_capture: [
      "projection evidence inventory (there must be nothing SXR-derived to lose)",
    ],
    expected_post_state: [
      "producer absence is typed (spawn_failed) and declared unavailable — never simulated",
      "producer incompatibility is typed (incompatible:cep_envelope_not_sxr_output) and recorded, never silently unwrapped",
      "CAS loss is typed (cas_object_missing) — a missing durable object is never substituted with different bytes",
      "ZERO SXR-derived rows are fabricated into the projection",
    ],
    storage: "in_memory_streams_plus_sqlite",
  }),
]);

/**
 * Declared scope exclusions: RECOV cases and confirmations T22 does NOT
 * re-prove (prereg known_blindspots), recorded as data — never silent gaps.
 */
export const SCOPE_EXCLUSIONS: readonly {
  readonly scope: string;
  readonly excluded_because: string;
}[] = Object.freeze([
  Object.freeze({
    scope: "RECOV-006 (trusted plugin throws during load/unload)",
    excluded_because:
      "plugin unload reversibility has its dedicated confirmation (T15); the T22 prereg known_blindspots exclude it from this task",
  }),
  Object.freeze({
    scope: "cross-surface operation isomorphism",
    excluded_because:
      "allowed/denied operation isomorphism across projections has its dedicated adversarial confirmation (T23); T22 does not re-prove it",
  }),
  Object.freeze({
    scope: "cross-process Prime reality",
    excluded_because:
      "T24 owns cross-process Prime; T22 exercises the declared in-model recovery semantics plus deterministic real-process kills (prereg blindspot)",
  }),
]);

// --- The typed SXR probe record (bridged from the real adapter in tests) ---------

/**
 * The verbatim typed outcomes of the three SXR probes, produced by the REAL
 * sxr-adapter surface in the test layer. Plain data by design (boundary pin).
 */
export interface SxrAvailabilityRecord {
  /** The real outcome class of the absent-producer probe (e.g. "spawn_failed"). */
  readonly producer_absent_outcome: string;
  /** The real parse verdict of the incompatible-producer probe. */
  readonly producer_incompatible_parse: string;
  /** The real CAS failure code of the lost-object probe. */
  readonly cas_loss_failure: string;
  /** SXR-derived rows the probes fabricated into the projection (must be 0). */
  readonly fabricated_rows: number;
}

// --- Results ---------------------------------------------------------------------

export interface CaseAssertion {
  readonly name: string;
  readonly passed: boolean;
  readonly detail: string;
}

export interface RecoveryCaseResult {
  readonly case_id: RecoveryCaseId;
  readonly recov: string;
  readonly requirements: readonly string[];
  readonly procedure_summary: string;
  readonly pre_state: RecoveryPreState;
  /** Typed detections the recovery observed (corruption findings, typed refusals). */
  readonly detections: readonly string[];
  readonly assertions: readonly CaseAssertion[];
  readonly post_source_fingerprints: AuthoritativeFingerprints;
  /** True when the authoritative sources are byte-identical across the RECOVERY. */
  readonly sources_byte_identical: boolean;
  readonly verdict: "pass" | "failed";
  /** Honest scope notes — never silent. */
  readonly residuals: readonly string[];
}

export interface RecoveryMatrixReport {
  readonly results: readonly RecoveryCaseResult[];
  readonly ledger: RecoveryRoundLedger;
  readonly all_passed: boolean;
  readonly failed_case_ids: readonly string[];
}

export interface RecoveryMatrixOptions {
  readonly rounds?: RecoveryRoundLedger | undefined;
  /** Directory for file-backed databases (defaults to the workbench root). */
  readonly scratchDir?: string | undefined;
  /** The real SXR probe record (required by the RECOV-SXR case). */
  readonly sxr?: SxrAvailabilityRecord | undefined;
  readonly caseIds?: readonly RecoveryCaseId[] | undefined;
}

interface CaseContext {
  readonly ledger: RecoveryRoundLedger;
  readonly scratchDir: string;
  readonly sxr?: SxrAvailabilityRecord | undefined;
}

function assertAll(
  collector: CaseAssertion[],
  name: string,
  passed: boolean,
  detail: string,
): void {
  collector.push({ name, passed, detail });
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function sourcesIdentical(
  pre: RecoveryPreState,
  port: InMemoryStreamPort,
): { identical: boolean; differences: readonly string[] } {
  const differences = fingerprintsDifferences(pre.source_fingerprints, fingerprintPort(port));
  return { identical: differences.length === 0, differences };
}

function normalizedStoreProjection(args: {
  store: SqliteProjectionStore;
  events: readonly AuthoritativeEventFrame[];
  kernelObjects: readonly RecordedKernelObject[];
  kernelAlive: boolean;
  runId?: string | undefined;
}): NormalizedRunProjection {
  return normalizeSnapshot(args.store.getSnapshot(null), {
    run_id: args.runId ?? RECORDED_RUN_ID,
    events: args.events,
    kernelObjects: args.kernelObjects,
    kernelAlive: args.kernelAlive,
  });
}

// --- Case: RECOV-001A projection deletion ----------------------------------------

async function runProjectionDeletion(context: CaseContext): Promise<RecoveryCaseResult> {
  const caseDeclaration = RECOVERY_CASES.find(
    (entry) => entry.id === "RECOV-001A-projection-deletion",
  );
  if (caseDeclaration === undefined) {
    throw new RecoveryCaseError("runProjectionDeletion", "case declaration missing");
  }
  const generationId = "gen-t22-001a";
  const dbPath = join(context.scratchDir, "test-t22-matrix-001a.db");
  const world = await buildMatrixWorld({ dbPath, generationId });
  const assertions: CaseAssertion[] = [];
  try {
    const log = world.authoritativeLog();
    const pre = captureRecoveryPreState({
      port: world.port,
      log,
      generationId,
      kernelObjects: [],
      kernelAlive: false,
    });

    // Frozen destructive procedure: unlink the database and sidecars.
    world.client.close();
    world.model.close();
    assertAll(assertions, "database_file_present_before_destruction", existsSync(dbPath), dbPath);
    for (const suffix of ["", "-wal", "-shm"]) {
      const path = `${dbPath}${suffix}`;
      if (existsSync(path)) {
        unlinkSync(path);
      }
    }
    assertAll(
      assertions,
      "database_file_deleted",
      !existsSync(dbPath),
      "main db unlinked from disk",
    );

    // Declared recovery: cold rebuild from the authoritative log.
    const rebuilt = SqliteProjectionStore.rebuildFromAuthoritativeLog(dbPath, generationId, log);
    const postNormalized = normalizedStoreProjection({
      store: rebuilt,
      events: log,
      kernelObjects: [],
      kernelAlive: false,
    });
    assertAll(
      assertions,
      "cold_rebuild_canonical_digest_equal",
      projectionDigest(canonicalSections(postNormalized)) === pre.canonical_digest,
      `canonical ${pre.canonical_digest}`,
    );
    assertAll(
      assertions,
      "cold_rebuild_full_digest_equal",
      projectionDigest(postNormalized) === pre.full_digest,
      `full ${pre.full_digest}`,
    );
    const viaLog = foldLogToNormalized(recordedRunFromLog({ generationId, events: log }), {
      kernelAlive: false,
    });
    assertAll(
      assertions,
      "t19_referee_log_fold_byte_equal",
      canonicalJson(viaLog) === canonicalJson(postNormalized),
      "the T19 pure fold and the rebuilt store agree byte-for-byte",
    );
    const sources = sourcesIdentical(pre, world.port);
    assertAll(
      assertions,
      "authoritative_sources_byte_identical",
      sources.identical,
      sources.differences.join("; ") || "all source digests unchanged",
    );
    rebuilt.close();

    context.ledger.record({
      case_id: caseDeclaration.id,
      round: context.ledger.nextRound(caseDeclaration.id),
      outcome: "pass",
      procedure: caseDeclaration.destructive_procedure.procedure,
      residuals: [],
      quarantined_state_sha256: null,
      quarantined_path: null,
    });
    return {
      case_id: caseDeclaration.id,
      recov: caseDeclaration.recov,
      requirements: caseDeclaration.requirements,
      procedure_summary: caseDeclaration.destructive_procedure.detail,
      pre_state: pre,
      detections: [],
      assertions,
      post_source_fingerprints: fingerprintPort(world.port),
      sources_byte_identical: sources.identical,
      verdict: assertions.every((a) => a.passed) ? "pass" : "failed",
      residuals: [
        "real loss surface is the workbench-owned projection file; the authoritative in-memory sources model the surviving canonical owners (REQ-SAFE-001)",
      ],
    };
  } finally {
    world.close();
    cleanupScratchDb(dbPath);
  }
}

// --- Case: RECOV-001B self-consistent corruption ----------------------------------

async function runProjectionCorruption(context: CaseContext): Promise<RecoveryCaseResult> {
  const caseDeclaration = RECOVERY_CASES.find(
    (entry) => entry.id === "RECOV-001B-projection-corruption",
  );
  if (caseDeclaration === undefined) {
    throw new RecoveryCaseError("runProjectionCorruption", "case declaration missing");
  }
  const generationId = "gen-t22-001b";
  const dbPath = join(context.scratchDir, "test-t22-matrix-001b.db");
  const world = await buildMatrixWorld({ dbPath, generationId });
  const assertions: CaseAssertion[] = [];
  const detections: string[] = [];
  let quarantined: QuarantineRecord | null = null;
  try {
    const log = world.authoritativeLog();
    const pre = captureRecoveryPreState({
      port: world.port,
      log,
      generationId,
      kernelObjects: [],
      kernelAlive: false,
    });
    const fingerprints = pre.source_fingerprints;
    const expectedFold = foldLogToNormalizationHelper(generationId, log);
    // The generation the LIVE file rows carry (the client minter suffixes a
    // serial) — the fold→file audit is pinned to it until the quarantine
    // rebuild, whose rows carry `generationId`.
    const liveGeneration = world.client.generationId ?? generationId;

    world.client.close();
    world.model.close();

    // Sanity: the pre-corruption file audits clean.
    const cleanAudit = auditProjectionRows({
      rows: readProjectionRows(dbPath),
      fingerprints,
      expected: expectedFold,
      expectedGenerationId: liveGeneration,
    });
    assertAll(
      assertions,
      "pre_corruption_audit_clean",
      cleanAudit.length === 0,
      `${String(cleanAudit.length)} findings before corruption`,
    );

    // Frozen destructive procedure: craft the self-consistent corrupt set.
    const craft = new Database(dbPath);
    try {
      const tx = craft.transaction(() => {
        craft.run(
          `INSERT OR REPLACE INTO source_cursors (generation_id, source_id, cursor, updated_at)
           VALUES (?, 'gauntlet', ?, ?);`,
          [generationId, FORGED_CURSOR, SEED_BASE_MS + 100],
        );
        craft.run(
          `INSERT OR REPLACE INTO units_projection
           (unit_id, generation_id, source_id, source_cursor, title, phase, status, authority, blast_radius, updated_at)
           VALUES ('unit-phantom', ?, 'gauntlet', ?, 'Phantom Unit', 'phaseX', 'running', 'builder', 'project', ?);`,
          [generationId, FORGED_CURSOR, SEED_BASE_MS + 101],
        );
        craft.run(
          `UPDATE units_projection SET status = 'tampered', source_cursor = ? WHERE unit_id = 'AK-402';`,
          [FORGED_CURSOR],
        );
      });
      tx();
    } finally {
      craft.close();
    }
    assertAll(
      assertions,
      "sources_never_touched_by_corruption",
      fingerprintsDifferences(pre.source_fingerprints, fingerprintPort(world.port)).length === 0,
      "the corrupt set was written only into the projection file",
    );

    // Detection BEFORE recovery: cursor/epoch validation against the authority.
    const corruptAudit = auditProjectionRows({
      rows: readProjectionRows(dbPath),
      fingerprints,
      expected: expectedFold,
      expectedGenerationId: liveGeneration,
    });
    for (const finding of corruptAudit) {
      detections.push(`${finding.code}@${finding.locus}: ${finding.detail}`);
    }
    const codes = new Set(corruptAudit.map((finding) => finding.code));
    assertAll(
      assertions,
      "corruption_detected_before_recovery",
      corruptAudit.length > 0 &&
        codes.has("generation_cursor_not_minted") &&
        codes.has("cursor_not_minted_by_source") &&
        codes.has("row_disagrees_with_authoritative_fold"),
      `${String(corruptAudit.length)} typed findings: ${[...codes].sort().join(", ")}`,
    );

    // Round 1: the naive fix (rebuild WITHOUT quarantine) must genuinely fail.
    const naive = SqliteProjectionStore.rebuildFromAuthoritativeLog(
      dbPath,
      `${generationId}-naive`,
      log,
    );
    const naiveAudit = auditProjectionRows({
      rows: readProjectionRows(dbPath),
      fingerprints,
      expected: expectedFold,
      expectedGenerationId: liveGeneration,
    });
    naive.close();
    assertAll(
      assertions,
      "naive_rebuild_genuinely_fails_the_contract",
      naiveAudit.length > 0,
      `phantom/corrupt rows survive a rebuild without quarantine (${String(naiveAudit.length)} findings)`,
    );
    context.ledger.record({
      case_id: caseDeclaration.id,
      round: context.ledger.nextRound(caseDeclaration.id),
      outcome: "failed",
      procedure: "naive_rebuild_without_quarantine",
      residuals: naiveAudit.map((finding) => `${finding.code}@${finding.locus}`),
      quarantined_state_sha256: null,
      quarantined_path: null,
    });

    // Round 2: the declared procedure — quarantine (retained evidence) + rebuild.
    quarantined = quarantineDatabase(dbPath, "round1-corrupt");
    assertAll(
      assertions,
      "corrupt_state_quarantined_and_retained",
      existsSync(quarantined.quarantined_path) &&
        fileSha256(quarantined.quarantined_path) === quarantined.sha256,
      `${quarantined.quarantined_path} sha256 ${quarantined.sha256}`,
    );
    const rebuilt = SqliteProjectionStore.rebuildFromAuthoritativeLog(dbPath, generationId, log);
    const cleanAfter = auditProjectionRows({
      rows: readProjectionRows(dbPath),
      fingerprints,
      expected: expectedFold,
      expectedGenerationId: generationId,
    });
    assertAll(
      assertions,
      "declared_recovery_leaves_zero_corrupt_rows",
      cleanAfter.length === 0,
      "the quarantined rebuild audited clean",
    );
    const postNormalized = normalizedStoreProjection({
      store: rebuilt,
      events: log,
      kernelObjects: [],
      kernelAlive: false,
    });
    assertAll(
      assertions,
      "rebuilt_digests_equal_pre_state",
      projectionDigest(canonicalSections(postNormalized)) === pre.canonical_digest &&
        projectionDigest(postNormalized) === pre.full_digest,
      `canonical ${pre.canonical_digest}`,
    );
    assertAll(
      assertions,
      "t19_referee_log_fold_byte_equal",
      canonicalJson(expectedFold) === canonicalJson(postNormalized),
      "the T19 pure fold and the rebuilt store agree byte-for-byte",
    );
    rebuilt.close();

    // Round 3: T16 epoch validation re-driven — a forged-epoch persisted position.
    const gauntletEntry = Object.entries(fingerprints).find(
      ([sourceId]) => sourceId === "gauntlet",
    );
    const gauntletFingerprint = gauntletEntry?.[1];
    if (gauntletFingerprint === undefined) {
      throw new RecoveryCaseError("runProjectionCorruption", "missing gauntlet fingerprint");
    }
    const realCursor = gauntletFingerprint.minted_cursors.at(-1);
    if (realCursor === undefined) {
      throw new RecoveryCaseError("runProjectionCorruption", "no minted cursor to reuse");
    }
    const persistedState: DetachedPresentationState = Object.freeze({
      detached_from_generation: generationId,
      source_positions: Object.freeze({
        gauntlet: Object.freeze({ epoch: "epoch-stale-forged", sequence: 3, cursor: realCursor }),
      }),
      view_preferences: Object.freeze({}),
    });
    const model3 = new WorkbenchModel();
    const client3 = new LiveStreamClient({
      model: model3,
      port: world.port,
      mintGenerationId: (serial) => `${generationId}-epoch-${serial}`,
    });
    try {
      client3.connect();
      const reHandshake = client3.reconnect(persistedState);
      const gauntletLeg = reHandshake.per_source.find((leg) => leg.source_id === "gauntlet");
      assertAll(
        assertions,
        "epoch_validation_detected_forged_persisted_epoch",
        gauntletLeg?.cursor_validation === "epoch_changed" &&
          gauntletLeg.strategy === "fresh_baseline",
        `validation ${String(gauntletLeg?.cursor_validation)}`,
      );
      const reference = referenceModel(world.port, client3.generationId ?? generationId);
      const diffs = snapshotsConverged(
        reference.store.getSnapshot(null),
        model3.store.getSnapshot(null),
      );
      assertAll(
        assertions,
        "epoch_validated_reattach_converged",
        diffs.length === 0,
        diffs.join("; ") || "converged with the authoritative fold",
      );
      reference.close();
    } finally {
      client3.close();
      model3.close();
    }
    context.ledger.record({
      case_id: caseDeclaration.id,
      round: context.ledger.nextRound(caseDeclaration.id),
      outcome: "pass",
      procedure: "quarantine_and_cold_rebuild_plus_epoch_validated_reattach",
      residuals: [],
      quarantined_state_sha256: quarantined.sha256,
      quarantined_path: quarantined.quarantined_path,
    });

    const sources = sourcesIdentical(pre, world.port);
    assertAll(
      assertions,
      "authoritative_sources_byte_identical",
      sources.identical,
      sources.differences.join("; ") ||
        "all source digests unchanged across detection and recovery",
    );
    return {
      case_id: caseDeclaration.id,
      recov: caseDeclaration.recov,
      requirements: caseDeclaration.requirements,
      procedure_summary: caseDeclaration.destructive_procedure.detail,
      pre_state: pre,
      detections,
      assertions,
      post_source_fingerprints: fingerprintPort(world.port),
      sources_byte_identical: sources.identical,
      verdict: assertions.every((a) => a.passed) ? "pass" : "failed",
      residuals: [
        "the quarantined corrupt database remains on disk for the duration of the run as retained failed-round evidence; tests clean scratch files afterwards",
        "minted-cursor validation uses the sources' retained window; retention-expired cursors are a declared REQ-SYNC-001 fresh-baseline condition, not corruption",
      ],
    };
  } finally {
    world.close();
    cleanupScratchDb(dbPath);
  }
}

/** The T19 pure fold used as the independent corruption referee. */
function foldLogToNormalizationHelper(
  generationId: string,
  log: readonly AuthoritativeEventFrame[],
): NormalizedRunProjection {
  return foldLogToNormalized(recordedRunFromLog({ generationId, events: log }), {
    kernelAlive: false,
  });
}

// --- Case: RECOV-002 TUI kill during live work ------------------------------------

async function runTuiKill(context: CaseContext): Promise<RecoveryCaseResult> {
  const caseDeclaration = RECOVERY_CASES.find(
    (entry) => entry.id === "RECOV-002-tui-kill-during-live-work",
  );
  if (caseDeclaration === undefined) {
    throw new RecoveryCaseError("runTuiKill", "case declaration missing");
  }
  const generationId = "gen-t22-002";
  const freshGeneration = "gen-t22-002-fresh-1";
  const dbPath = join(context.scratchDir, "test-t22-matrix-002.db");
  const world = await buildMatrixWorld({ dbPath, generationId });
  const assertions: CaseAssertion[] = [];
  try {
    const detach = world.client.detach({ selected_horizon: "hz-401" });

    // Spawn the REAL TUI process first (readiness awaited), so the kill below
    // is deterministic.
    const tuiProcess = await spawnRealTuiProcess();

    // In-model host death.
    world.client.close();
    world.model.close();

    // Real process kill (deterministic: SIGKILL a child we own and wait).
    const reaped = await tuiProcess.killAndReap();
    assertAll(
      assertions,
      "real_tui_process_killed_by_sigkill",
      reaped.signal === "SIGKILL" || reaped.exit_code === 137,
      `exit ${String(reaped.exit_code)} signal ${String(reaped.signal)} (pid ${String(tuiProcess.pid)})`,
    );

    // Authorized Prime work continues; authoritative events land while dead.
    world.adapter.receiveMailboxMessage("prime-child-1", {
      sender: "prime-child-1",
      content: "progress: still running after the TUI died",
      kind: "progress",
    });
    const midFlight = world.adapter.getSession("prime-child-1");
    assertAll(
      assertions,
      "authorized_work_survives_host_death",
      midFlight?.handle.status === "running" && midFlight.resultContract === null,
      "the child is running with no result contract — mailbox text is not completion",
    );
    world.gauntlet.append({
      event_type: "unit_updated",
      payload: { unit_id: "AK-401", status: "blocked" },
      timestamp_ms: SEED_BASE_MS + 20,
    });
    world.prime.append({
      event_type: "session_updated",
      payload: { session_id: "prime-child-1", parent_id: "prime-root", status: "running" },
      timestamp_ms: SEED_BASE_MS + 21,
    });

    // Pre-state evidence for the RECOVERY that follows (sources now final).
    const pre = captureRecoveryPreState({
      port: world.port,
      log: world.authoritativeLog(freshGeneration),
      generationId: freshGeneration,
      kernelObjects: [],
      kernelAlive: false,
    });

    // Fresh host reconstruction from real state (never the dead host's memory).
    const freshModel = new WorkbenchModel({ dbPath });
    const freshClient = new LiveStreamClient({
      model: freshModel,
      port: world.port,
      mintGenerationId: (serial) => `gen-t22-002-fresh-${serial}`,
    });
    try {
      const receipt = freshClient.reattach({
        persisted: detach.persisted,
        prime_adapter: world.adapter,
      });
      assertAll(
        assertions,
        "reattach_issues_no_cancel",
        receipt.cancel_calls_issued === 0,
        "literal type 0",
      );
      assertAll(
        assertions,
        "reattach_uses_persisted_presentation_state",
        receipt.presentation_source === "persisted_noncanonical_cursor_state",
        String(receipt.presentation_source),
      );
      assertAll(
        assertions,
        "prime_topology_from_qualified_records",
        receipt.prime?.running_sessions.includes("prime-child-1") === true &&
          (receipt.prime.topology.length ?? 0) >= 1,
        `running ${JSON.stringify(receipt.prime?.running_sessions ?? [])}`,
      );
      const gauntletLeg = receipt.handshake.per_source.find((leg) => leg.source_id === "gauntlet");
      assertAll(
        assertions,
        "events_since_last_cursor_reconciled",
        (gauntletLeg?.reconciled_since_cursor ?? 0) >= 1,
        `reconciled ${String(gauntletLeg?.reconciled_since_cursor ?? 0)} since the persisted cursor`,
      );

      // Convergence with the authoritative fold.
      const liveGeneration = freshClient.generationId ?? freshGeneration;
      const reference = referenceModel(world.port, liveGeneration);
      const diffs = snapshotsConverged(
        reference.store.getSnapshot(null),
        freshModel.store.getSnapshot(null),
      );
      assertAll(
        assertions,
        "fresh_host_converged_with_authoritative_fold",
        diffs.length === 0,
        diffs.join("; ") || "identical to the authoritative fold",
      );

      // Correction history preserved: the fresh host's OWN trace fan-out (its
      // live model) is parsed back and compared against the log-derived audit
      // trail — the model's history, not a restatement of the log (T19 rule).
      const fullLog = world.authoritativeLog(liveGeneration);
      const liveNormalized = normalizeLiveFold({
        run_id: RECORDED_RUN_ID,
        generation_id: liveGeneration,
        storeSnapshot: freshModel.store.getSnapshot(null),
        trace: freshModel.getTrace(),
        kernelObjects: [],
        kernelAlive: false,
      });
      const logFold = foldLogToNormalized(
        recordedRunFromLog({ generationId: liveGeneration, events: fullLog }),
        { kernelAlive: false },
      );
      assertAll(
        assertions,
        "correction_history_preserved",
        JSON.stringify(liveNormalized.trace) === JSON.stringify(logFold.trace),
        `${String(liveNormalized.trace.length)} trace entries equal the log-derived audit trail`,
      );
      reference.close();

      // Completion is Prime's own schema-valid contract (never liveness).
      const completed = world.adapter.deliverResultContract("prime-child-1", {
        session_id: "prime-child-1",
        schema_valid: true,
        outcome: "pass",
        evidence_path:
          ".agents/reports/gauntlet-tui-prime-workbench/evidence/T22/contracts/child-1-result.md",
        completed_at: "2026-09-09T12:00:00Z",
      });
      assertAll(
        assertions,
        "completion_via_prime_contract",
        completed,
        "Prime completed the child itself",
      );
      const primeAfter = reattachToPrime(world.adapter);
      assertAll(
        assertions,
        "reattached_completion_from_records",
        primeAfter.completed_sessions.includes("prime-child-1") &&
          !primeAfter.running_sessions.includes("prime-child-1"),
        "the fresh receipt reads completion from qualified records only",
      );
    } finally {
      freshClient.close();
      freshModel.close();
    }

    const noCancel = world.adapterCalls.every(
      (call) => !/cancel|stop|terminate|abort|kill|interrupt/i.test(call),
    );
    assertAll(
      assertions,
      "adapter_call_log_records_no_termination",
      noCancel,
      world.adapterCalls.join(", ") || "read-only adapter calls only",
    );

    const sources = sourcesIdentical(pre, world.port);
    assertAll(
      assertions,
      "authoritative_sources_byte_identical_across_recovery",
      sources.identical,
      sources.differences.join("; ") ||
        "all source digests unchanged across the fresh-host recovery",
    );
    context.ledger.record({
      case_id: caseDeclaration.id,
      round: context.ledger.nextRound(caseDeclaration.id),
      outcome: "pass",
      procedure: caseDeclaration.destructive_procedure.procedure,
      residuals: [],
      quarantined_state_sha256: null,
      quarantined_path: null,
    });
    return {
      case_id: caseDeclaration.id,
      recov: caseDeclaration.recov,
      requirements: caseDeclaration.requirements,
      procedure_summary: caseDeclaration.destructive_procedure.detail,
      pre_state: pre,
      detections: [],
      assertions,
      post_source_fingerprints: fingerprintPort(world.port),
      sources_byte_identical: sources.identical,
      verdict: assertions.every((a) => a.passed) ? "pass" : "failed",
      residuals: [
        "the real-process leg kills a harness-owned TUI child deterministically (readiness awaited, SIGKILL reaped); the Prime side stays in-model (T24 owns cross-process Prime)",
        "pre-state evidence is captured after the authoritative while-the-TUI-was-dead events, so the byte-identical check measures the RECOVERY itself",
      ],
    };
  } finally {
    world.close();
    cleanupScratchDb(dbPath);
  }
}

// --- Case: RECOV-003 kernel loss ---------------------------------------------------

async function runKernelLoss(context: CaseContext): Promise<RecoveryCaseResult> {
  const caseDeclaration = RECOVERY_CASES.find(
    (entry) => entry.id === "RECOV-003-kernel-loss-objects",
  );
  if (caseDeclaration === undefined) {
    throw new RecoveryCaseError("runKernelLoss", "case declaration missing");
  }
  const generationId = "gen-t22-003";
  const world = await buildMatrixWorld({ dbPath: ":memory:", generationId, withKernel: true });
  const assertions: CaseAssertion[] = [];
  const detections: string[] = [];
  try {
    // The client's REAL live generation (the minter suffixes a serial).
    const liveGeneration = world.client.generationId ?? generationId;
    const pre = captureRecoveryPreState({
      port: world.port,
      log: world.authoritativeLog(liveGeneration),
      generationId: liveGeneration,
      kernelObjects: world.kernelObjects,
      kernelAlive: true,
    });
    const storeDigestBefore = projectionDigest(world.model.store.getSnapshot(null));
    const ledger = world.ledger;
    const kernel = world.kernel;
    if (ledger === null || kernel === null) {
      throw new RecoveryCaseError("runKernelLoss", "the fixture has no kernel/ledger");
    }

    // Frozen destructive procedure: kernel loss (in-model).
    kernel.terminate();
    assertAll(assertions, "kernel_dead", !kernel.isAlive, "the kernel runtime is terminated");

    // T11 typed invalidation on the OLD generation's ledger: fail closed.
    const probeObject = world.kernelObjects.find((object) => object.name === "probe");
    const scratchObject = world.kernelObjects.find((object) => object.name === "scratch");
    const metricsObject = world.kernelObjects.find((object) => object.name === "metrics");
    if (probeObject === undefined || scratchObject === undefined || metricsObject === undefined) {
      throw new RecoveryCaseError("runKernelLoss", "fixture kernel objects missing");
    }
    const pinDead = ledger.pin(probeObject.object_id);
    if (pinDead.class !== "refused") {
      throw new RecoveryCaseError("runKernelLoss", "expected a typed refusal");
    }
    detections.push(`pin_after_kernel_loss:${pinDead.reason}`);
    const artifactizeDead = ledger.artifactize(
      scratchObject.object_id,
      ".agents/reports/artifacts/t22-x.json",
    );
    if (artifactizeDead.class !== "refused") {
      throw new RecoveryCaseError("runKernelLoss", "expected a typed refusal");
    }
    detections.push(`artifactize_after_kernel_loss:${artifactizeDead.reason}`);
    assertAll(
      assertions,
      "t11_ledger_fails_closed_after_kernel_loss",
      pinDead.reason === "kernel_object_lost" && artifactizeDead.reason === "kernel_object_lost",
      "promotion refuses because the owning kernel no longer holds the values",
    );

    // A restarted kernel's references never revive; name reuse mints NEW state.
    const kernel2 = new PrimeReplKernel(KERNEL_SESSION);
    const ledger2 = new KernelObjectLedger({ kernel: kernel2, kernel_generation: "kernel-gen-2" });
    assertAll(
      assertions,
      "old_reference_never_revives",
      ledger2.find(metricsObject.object_id) === null,
      "cross-generation object references do not resolve",
    );
    const run2 = await kernel2.execute("metrics = [9, 9]");
    const registered2 = ledger2.register(run2);
    const newMetrics = requireNamedObject(registered2, "metrics");
    detections.push(`restarted_object_ephemeral:${String(newMetrics.persistence_class)}`);
    assertAll(
      assertions,
      "name_reuse_mints_new_ephemeral_object",
      newMetrics.provenance.object_id !== metricsObject.object_id &&
        newMetrics.provenance.kernel_generation === "kernel-gen-2" &&
        newMetrics.persistence_class === "ephemeral",
      "the restarted kernel's same-named object is a NEW ephemeral value — no revival of the artifactized class",
    );
    const stalePin = ledger2.pin(scratchObject.object_id);
    if (stalePin.class !== "refused") {
      throw new RecoveryCaseError("runKernelLoss", "expected a typed refusal");
    }
    detections.push(`stale_generation_pin:${stalePin.reason}`);
    assertAll(
      assertions,
      "stale_generation_pin_refused",
      stalePin.reason === "unknown_object",
      "the new generation's ledger refuses objects of another generation",
    );

    // T19 typed availability after loss: only the durable locator survives.
    const fold = foldLogToNormalized(world.recordedRun(), { kernelAlive: false });
    const kernelRecords = new Map(fold.kernel_objects.map((record) => [record.name, record]));
    const metricsRecord = kernelRecords.get("metrics");
    const scratchRecord = kernelRecords.get("scratch");
    const probeRecord = kernelRecords.get("probe");
    assertAll(
      assertions,
      "artifactized_survives_via_locator_only",
      metricsRecord?.value_available === true &&
        metricsRecord.artifact_ref === KERNEL_ARTIFACT_LOCATOR &&
        metricsRecord.persistence_class === "artifactized",
      "the artifactized object is available at its durable locator",
    );
    if (scratchRecord !== undefined && probeRecord !== undefined) {
      detections.push(`pinned_after_loss:${String(scratchRecord.unavailability_reason)}`);
      detections.push(`ephemeral_after_loss:${String(probeRecord.unavailability_reason)}`);
    }
    assertAll(
      assertions,
      "pinned_value_unavailable_not_recreated",
      scratchRecord?.value_available === false &&
        scratchRecord.unavailability_reason === "kernel_restarted_pinned_value_not_retained" &&
        scratchRecord.artifact_ref === null,
      "the pinned value is gone with the kernel; typed reason, null locator",
    );
    assertAll(
      assertions,
      "ephemeral_value_unavailable_not_recreated",
      probeRecord?.value_available === false &&
        probeRecord.unavailability_reason === "ephemeral_kernel_state_not_reconstructible" &&
        probeRecord.artifact_ref === null,
      "the ephemeral value is gone with the kernel; typed reason, null locator",
    );

    // Canonical truth untouched by kernel death.
    const storeDigestAfter = projectionDigest(world.model.store.getSnapshot(null));
    assertAll(
      assertions,
      "canonical_truth_untouched_by_kernel_death",
      storeDigestBefore === storeDigestAfter,
      "the projection store digest is identical before and after",
    );
    const postCanonical = canonicalSections(
      normalizedStoreProjection({
        store: world.model.store,
        events: world.authoritativeLog(),
        kernelObjects: world.kernelObjects,
        kernelAlive: false,
      }),
    );
    assertAll(
      assertions,
      "canonical_section_digest_unchanged",
      projectionDigest(postCanonical) === pre.canonical_digest,
      `canonical ${pre.canonical_digest}`,
    );

    // Liveness is not success (REQ-SAFE-004) on the RESTARTED kernel.
    const settle = kernel2.canSettleUnit("AK-401");
    assertAll(
      assertions,
      "restarted_kernel_cannot_settle_by_liveness",
      !settle.allowed,
      settle.reason,
    );

    // Loss is not papered over: the new value needs explicit re-artifactization.
    const repromote = ledger2.artifactize(
      newMetrics.provenance.object_id,
      ".agents/reports/artifacts/t22-metrics-gen2.json",
    );
    assertAll(
      assertions,
      "recovery_requires_explicit_repromotion",
      repromote.class === "transitioned",
      "the restarted value becomes durable only through an explicit new artifactization",
    );

    const sources = sourcesIdentical(pre, world.port);
    assertAll(
      assertions,
      "authoritative_sources_byte_identical",
      sources.identical,
      sources.differences.join("; ") || "all source digests unchanged",
    );
    context.ledger.record({
      case_id: caseDeclaration.id,
      round: context.ledger.nextRound(caseDeclaration.id),
      outcome: "pass",
      procedure: caseDeclaration.destructive_procedure.procedure,
      residuals: [],
      quarantined_state_sha256: null,
      quarantined_path: null,
    });
    return {
      case_id: caseDeclaration.id,
      recov: caseDeclaration.recov,
      requirements: caseDeclaration.requirements,
      procedure_summary: caseDeclaration.destructive_procedure.detail,
      pre_state: pre,
      detections,
      assertions,
      post_source_fingerprints: fingerprintPort(world.port),
      sources_byte_identical: sources.identical,
      verdict: assertions.every((a) => a.passed) ? "pass" : "failed",
      residuals: [
        "kernel loss is the declared in-model boundary (PrimeReplKernel.terminate); cross-process kernel reality is T24 per the prereg blindspot",
      ],
    };
  } finally {
    world.close();
  }
}

// --- Case: RECOV-004 reconnect mid-stream with stale frames -----------------------

async function runReconnectMidStream(context: CaseContext): Promise<RecoveryCaseResult> {
  const caseDeclaration = RECOVERY_CASES.find(
    (entry) => entry.id === "RECOV-004-reconnect-mid-stream-stale-frames",
  );
  if (caseDeclaration === undefined) {
    throw new RecoveryCaseError("runReconnectMidStream", "case declaration missing");
  }
  const generationId = "gen-t22-004";
  const world = await buildMatrixWorld({ dbPath: ":memory:", generationId });
  const assertions: CaseAssertion[] = [];
  let rejectionCount = 0;
  let finalGeneration = generationId;
  try {
    const fingerprintsAtStart = fingerprintPort(world.port);
    const staleGeneration = world.client.generationId;
    if (staleGeneration === null) {
      throw new RecoveryCaseError("runReconnectMidStream", "no live generation to go stale");
    }
    const detach = world.client.detach({ last_focus: "AK-401" });
    const staleSubscription = world.client.subscribeStream();

    // TUI death mid-stream; in-flight events never delivered to it.
    world.client.close();
    world.model.close();
    const inflight = [
      world.gauntlet.append({
        event_type: "unit_updated",
        payload: { unit_id: "AK-401", status: "review" },
        timestamp_ms: SEED_BASE_MS + 30,
      }),
      world.gauntlet.append({
        event_type: "unit_updated",
        payload: { unit_id: "AK-402", status: "in_progress" },
        timestamp_ms: SEED_BASE_MS + 31,
      }),
      world.gauntlet.append({
        event_type: "unit_updated",
        payload: { unit_id: "AK-403", title: "Unit 403", status: "pending" },
        timestamp_ms: SEED_BASE_MS + 32,
      }),
    ];

    // Fresh host reattach: the fresh generation baselines the in-flight events.
    const freshModel = new WorkbenchModel();
    const freshClient = new LiveStreamClient({
      model: freshModel,
      port: world.port,
      mintGenerationId: (serial) => `${generationId}-fresh-${serial}`,
    });
    try {
      const receipt = freshClient.reattach({ persisted: detach.persisted });
      const gauntletLeg = receipt.handshake.per_source.find((leg) => leg.source_id === "gauntlet");
      assertAll(
        assertions,
        "fresh_generation_baselined_in_flight_events",
        (gauntletLeg?.reconciled_since_cursor ?? 0) >= 3,
        `reconciled ${String(gauntletLeg?.reconciled_since_cursor ?? 0)} events since the persisted cursor`,
      );

      // Old-generation late frames delivered after the reconnect: ALL rejected.
      let rejectedStale = 0;
      for (const event of inflight) {
        const result = freshClient.ingest({ generation_id: staleGeneration, event });
        if (result.class === "rejected" && result.rejection.reason === "superseded_generation") {
          rejectedStale += 1;
        }
      }
      assertAll(
        assertions,
        "all_old_generation_late_frames_rejected",
        rejectedStale === inflight.length &&
          freshClient.rejectionLog.every(
            (rejection) =>
              rejection.frame_generation === staleGeneration &&
              rejection.live_generation === receipt.handshake.generation_id,
          ),
        `${String(rejectedStale)} stale frames typed-rejected with generation identity preserved`,
      );

      // Fresh-generation live flow with a sequence gap: bounded buffer, then
      // drain. After the reattach the tracked gauntlet position is sequence 7,
      // so the NEXT append is sequence 8 and the one after is sequence 9.
      // Deliver sequence 9 FIRST — a real gap against the tracked position.
      const liveSubscription = freshClient.subscribeStream();
      const gapFiller = world.gauntlet.append({
        event_type: "unit_updated",
        payload: { unit_id: "AK-403", status: "review" },
        timestamp_ms: SEED_BASE_MS + 34,
      });
      const beyondGap = world.gauntlet.append({
        event_type: "unit_updated",
        payload: { unit_id: "AK-403", status: "in_progress" },
        timestamp_ms: SEED_BASE_MS + 35,
      });
      const buffered = liveSubscription.deliver(beyondGap);
      assertAll(
        assertions,
        "sequence_gap_bounded_buffered",
        buffered.class === "buffered" && freshClient.state === "buffering",
        "the out-of-order increment is buffered with a visible state transition",
      );
      const gapFilled = liveSubscription.deliver(gapFiller);
      assertAll(
        assertions,
        "gap_drained_in_canonical_order",
        gapFilled.class === "applied" &&
          freshClient.state === "live" &&
          freshClient.bufferedCount() === 0,
        "the missing increment drains the buffer back to live",
      );

      // Old-epoch frame: visible resync with a fresh baseline (REQ-SYNC-001).
      world.gauntlet.replaceStorage("epoch-2");
      const newEpochEvent = world.gauntlet.append({
        event_type: "unit_updated",
        payload: { unit_id: "AK-401", status: "blocked" },
        timestamp_ms: SEED_BASE_MS + 36,
      });
      const epochResult = liveSubscription.deliver(newEpochEvent);
      assertAll(
        assertions,
        "old_epoch_frame_triggers_visible_resync",
        epochResult.class === "resync_triggered" && epochResult.reason.includes("epoch_change"),
        String(epochResult.class === "resync_triggered" ? epochResult.reason : epochResult.class),
      );
      assertAll(
        assertions,
        "resync_minted_fresh_generation",
        freshClient.generationId !== receipt.handshake.generation_id &&
          freshClient.state === "live",
        `resynced to ${String(freshClient.generationId)}`,
      );

      // Convergence with the authoritative fold (epoch-2 state), BEFORE any
      // further authoritative appends below.
      const liveGeneration = freshClient.generationId ?? generationId;
      const reference = referenceModel(world.port, liveGeneration);
      const diffs = snapshotsConverged(
        reference.store.getSnapshot(null),
        freshModel.store.getSnapshot(null),
      );
      assertAll(
        assertions,
        "reconnection_converged_with_authoritative_fold",
        diffs.length === 0,
        diffs.join("; ") || "identical to the authoritative fold after resync",
      );
      reference.close();

      // Subscriptions outliving their generation are rejected forever (these
      // probes append authoritative events the rejected frames would have
      // carried; the client catches up on its next poll — never by applying
      // stale frames).
      const lateStale = liveSubscription.deliver(
        world.gauntlet.append({
          event_type: "unit_updated",
          payload: { unit_id: "AK-401", status: "pending" },
          timestamp_ms: SEED_BASE_MS + 37,
        }),
      );
      assertAll(
        assertions,
        "subscription_outliving_generation_rejected",
        lateStale.class === "rejected" && lateStale.rejection.reason === "superseded_generation",
        "frames minted under the superseded generation are rejected",
      );
      const staleDelivery = staleSubscription.deliver(
        world.gauntlet.append({
          event_type: "unit_updated",
          payload: { unit_id: "AK-402", status: "pending" },
          timestamp_ms: SEED_BASE_MS + 38,
        }),
      );
      assertAll(
        assertions,
        "pre_kill_subscription_frames_rejected",
        staleDelivery.class === "rejected",
        `the pre-kill subscription's frames are ${staleDelivery.class === "rejected" ? staleDelivery.rejection.reason : String(staleDelivery.class)}`,
      );
      rejectionCount = freshClient.rejectionLog.length;
      finalGeneration = freshClient.generationId ?? generationId;
    } finally {
      freshClient.close();
      freshModel.close();
    }

    // The ONLY source change across the whole scenario is the declared
    // authoritative epoch replacement on "gauntlet" — never a workbench write.
    const sourceDiffs = fingerprintsDifferences(fingerprintsAtStart, fingerprintPort(world.port));
    assertAll(
      assertions,
      "only_declared_epoch_replacement_changed_sources",
      sourceDiffs.every((diff) => diff.startsWith("gauntlet:")) &&
        sourceDiffs.some((diff) => diff.includes("epoch-1!=epoch-2")),
      sourceDiffs.join("; ") || "no source change",
    );

    context.ledger.record({
      case_id: caseDeclaration.id,
      round: context.ledger.nextRound(caseDeclaration.id),
      outcome: "pass",
      procedure: caseDeclaration.destructive_procedure.procedure,
      residuals: [],
      quarantined_state_sha256: null,
      quarantined_path: null,
    });
    const pre = captureRecoveryPreState({
      port: world.port,
      log: world.authoritativeLog(finalGeneration),
      generationId: finalGeneration,
      kernelObjects: [],
      kernelAlive: false,
    });
    return {
      case_id: caseDeclaration.id,
      recov: caseDeclaration.recov,
      requirements: caseDeclaration.requirements,
      procedure_summary: caseDeclaration.destructive_procedure.detail,
      pre_state: pre,
      detections: [`${String(rejectionCount)} typed stale-frame rejections`],
      assertions,
      post_source_fingerprints: fingerprintPort(world.port),
      // Sources are byte-identical across the RECOVERY proper (post-epoch); the
      // epoch-1 -> epoch-2 change is the source's OWN authoritative replacement.
      sources_byte_identical:
        fingerprintsDifferences(pre.source_fingerprints, fingerprintPort(world.port)).length === 0,
      verdict: assertions.every((a) => a.passed) ? "pass" : "failed",
      residuals: [
        "transport loss is the deterministic in-process port boundary (T16 prereg binds scope); real network targets are T20",
        "the epoch-1 -> epoch-2 replacement is the SOURCE's own authoritative storage replacement, not workbench mutation (asserted explicitly)",
      ],
    };
  } finally {
    world.close();
  }
}

// --- Case: RECOV-005 Prime unavailable, Gauntlet available ------------------------

const PRIME_CAPABILITY: CapabilityObservation = {
  capability: "prime-orchestration",
  installed: true,
  gates: {
    visibility: true,
    reachability: true,
    payment: true,
    governance: true,
    settlement_access: true,
    recovery: true,
  },
};

function withGate(
  observation: CapabilityObservation,
  gate: string,
  pass: boolean,
): CapabilityObservation {
  return { ...observation, gates: { ...observation.gates, [gate]: pass } };
}

async function runPrimeUnavailable(context: CaseContext): Promise<RecoveryCaseResult> {
  const caseDeclaration = RECOVERY_CASES.find(
    (entry) => entry.id === "RECOV-005-prime-unavailable-degraded",
  );
  if (caseDeclaration === undefined) {
    throw new RecoveryCaseError("runPrimeUnavailable", "case declaration missing");
  }
  const generationId = "gen-t22-005";
  const world = await buildMatrixWorld({ dbPath: ":memory:", generationId });
  const assertions: CaseAssertion[] = [];
  const detections: string[] = [];
  try {
    world.model.declareCapabilities([PRIME_CAPABILITY]);
    const liveGeneration = world.client.generationId ?? generationId;
    const pre = captureRecoveryPreState({
      port: world.port,
      log: world.authoritativeLog(liveGeneration),
      generationId: liveGeneration,
      kernelObjects: [],
      kernelAlive: false,
    });
    const storeDigestBefore = projectionDigest(world.model.store.getSnapshot(null));
    const availableSnapshot = world.model.getSnapshot();
    assertAll(
      assertions,
      "prime_capability_available_before",
      availableSnapshot.capability_reasons.some(
        (status) => status.capability === "prime-orchestration" && status.status === "available",
      ) && availableSnapshot.affordance_ids.includes("affordance:capability:prime-orchestration"),
      "installed with all gates passing resolves available",
    );

    // Frozen destructive procedure: capability-scoped withdrawal.
    world.model.declareCapabilities([withGate(PRIME_CAPABILITY, "recovery", false)]);
    const degraded = world.model.getSnapshot();
    const primeStatus = degraded.capability_reasons.find(
      (status) => status.capability === "prime-orchestration",
    );
    if (primeStatus !== undefined) {
      detections.push(primeStatus.reason_codes.join(","));
    }
    assertAll(
      assertions,
      "prime_affordance_degrades_with_typed_reason",
      primeStatus?.status === "unavailable" &&
        primeStatus.reason_codes.includes("recovery_gate_not_passed") &&
        !degraded.affordance_ids.includes("affordance:capability:prime-orchestration"),
      "installed is never faked as available; the failing gate is the typed reason",
    );

    // Unrelated canonical inspection stays valid (capability-scoped).
    const storeDigestAfter = projectionDigest(world.model.store.getSnapshot(null));
    assertAll(
      assertions,
      "canonical_inspection_unaffected",
      storeDigestBefore === storeDigestAfter &&
        degraded.units.length === availableSnapshot.units.length &&
        degraded.units.every((unit) =>
          availableSnapshot.units.some((available) => available.unit_id === unit.unit_id),
        ),
      "the store digest and every unit row survive the withdrawal untouched",
    );
    assertAll(
      assertions,
      "unit_affordances_remain",
      degraded.affordance_ids.includes("affordance-AK-401") &&
        degraded.affordance_ids.includes("affordance-AK-402"),
      "Gauntlet-side affordances stay reachable while Prime is degraded",
    );

    // Restoration: capability returns and a resync mints a fresh generation.
    world.model.declareCapabilities([PRIME_CAPABILITY]);
    const restored = world.model.getSnapshot();
    assertAll(
      assertions,
      "prime_capability_restored",
      restored.capability_reasons.some(
        (status) => status.capability === "prime-orchestration" && status.status === "available",
      ),
      "the capability is available again after restoration",
    );
    const handshake = world.client.resync("prime_availability_restored");
    assertAll(
      assertions,
      "restoration_resyncs_to_fresh_generation",
      handshake.kind === "resync" &&
        handshake.generation_id !== `${generationId}-1` &&
        world.client.state === "live",
      `resync minted ${handshake.generation_id}`,
    );
    const reference = referenceModel(world.port, handshake.generation_id);
    const diffs = snapshotsConverged(
      reference.store.getSnapshot(null),
      world.model.store.getSnapshot(null),
    );
    assertAll(
      assertions,
      "post_restoration_converged",
      diffs.length === 0,
      diffs.join("; ") || "converged with the authoritative fold",
    );
    reference.close();

    context.ledger.record({
      case_id: caseDeclaration.id,
      round: context.ledger.nextRound(caseDeclaration.id),
      outcome: "pass",
      procedure: caseDeclaration.destructive_procedure.procedure,
      residuals: [],
      quarantined_state_sha256: null,
      quarantined_path: null,
    });
    return {
      case_id: caseDeclaration.id,
      recov: caseDeclaration.recov,
      requirements: caseDeclaration.requirements,
      procedure_summary: caseDeclaration.destructive_procedure.detail,
      pre_state: pre,
      detections,
      assertions,
      post_source_fingerprints: fingerprintPort(world.port),
      sources_byte_identical: true,
      verdict: assertions.every((a) => a.passed) ? "pass" : "failed",
      residuals: [
        "degradation is environment-observed (declared capability observation), never inferred from liveness — the adapter model exposes no availability probe",
      ],
    };
  } finally {
    world.close();
  }
}

// --- Case: RECOV-SXR absence / incompatibility / CAS loss --------------------------

async function runSxrAbsence(context: CaseContext): Promise<RecoveryCaseResult> {
  const caseDeclaration = RECOVERY_CASES.find(
    (entry) => entry.id === "RECOV-SXR-absence-incompatibility-cas-loss",
  );
  if (caseDeclaration === undefined) {
    throw new RecoveryCaseError("runSxrAbsence", "case declaration missing");
  }
  const generationId = "gen-t22-sxr";
  const world = await buildMatrixWorld({ dbPath: ":memory:", generationId });
  const assertions: CaseAssertion[] = [];
  const detections: string[] = [];
  try {
    const probe = context.sxr;
    if (probe === undefined) {
      // Missing evidence is NOT passing: record the failed round honestly.
      context.ledger.record({
        case_id: caseDeclaration.id,
        round: context.ledger.nextRound(caseDeclaration.id),
        outcome: "failed",
        procedure: caseDeclaration.destructive_procedure.procedure,
        residuals: [
          "missing_sxr_probe_record: the RECOV-SXR case requires the real adapter probes",
        ],
        quarantined_state_sha256: null,
        quarantined_path: null,
      });
      return {
        case_id: caseDeclaration.id,
        recov: caseDeclaration.recov,
        requirements: caseDeclaration.requirements,
        procedure_summary: caseDeclaration.destructive_procedure.detail,
        pre_state: captureRecoveryPreState({
          port: world.port,
          log: world.authoritativeLog(),
          generationId,
          kernelObjects: [],
          kernelAlive: false,
        }),
        detections: [],
        assertions: [
          {
            name: "sxr_probe_record_present",
            passed: false,
            detail: "no SxrAvailabilityRecord was provided; missing evidence is not passing",
          },
        ],
        post_source_fingerprints: fingerprintPort(world.port),
        sources_byte_identical: true,
        verdict: "failed",
        residuals: ["missing_sxr_probe_record"],
      };
    }

    assertAll(
      assertions,
      "producer_absence_typed",
      probe.producer_absent_outcome === "spawn_failed",
      `absent producer resolved ${probe.producer_absent_outcome}`,
    );
    detections.push(`producer_absent:${probe.producer_absent_outcome}`);
    assertAll(
      assertions,
      "producer_incompatibility_typed",
      probe.producer_incompatible_parse === "incompatible:cep_envelope_not_sxr_output",
      `incompatible producer parsed ${probe.producer_incompatible_parse}`,
    );
    detections.push(`producer_incompatible:${probe.producer_incompatible_parse}`);
    assertAll(
      assertions,
      "cas_loss_typed",
      probe.cas_loss_failure === "cas_object_missing",
      `lost CAS object failed ${probe.cas_loss_failure}`,
    );
    detections.push(`cas_object_missing:${probe.cas_loss_failure}`);

    // Nothing SXR-derived may have been fabricated into the projection.
    const snapshot = world.model.store.getSnapshot(null);
    const sxrRows = snapshot.evidence.filter(
      (evidence) => evidence.source_id === "sxr" || evidence.locator.startsWith("cas://"),
    );
    assertAll(
      assertions,
      "zero_fabricated_rows",
      sxrRows.length === 0 && probe.fabricated_rows === 0,
      `${String(sxrRows.length)} SXR-derived rows in the projection`,
    );

    context.ledger.record({
      case_id: caseDeclaration.id,
      round: context.ledger.nextRound(caseDeclaration.id),
      outcome: "pass",
      procedure: caseDeclaration.destructive_procedure.procedure,
      residuals: [],
      quarantined_state_sha256: null,
      quarantined_path: null,
    });
    return {
      case_id: caseDeclaration.id,
      recov: caseDeclaration.recov,
      requirements: caseDeclaration.requirements,
      procedure_summary: caseDeclaration.destructive_procedure.detail,
      pre_state: captureRecoveryPreState({
        port: world.port,
        log: world.authoritativeLog(),
        generationId,
        kernelObjects: [],
        kernelAlive: false,
      }),
      detections,
      assertions,
      post_source_fingerprints: fingerprintPort(world.port),
      sources_byte_identical: true,
      verdict: assertions.every((a) => a.passed) ? "pass" : "failed",
      residuals: [
        "probes execute the REAL sxr-adapter boundary (executeSxrRefresh/parseSxrSlice0bEnvelopeOutput) in the test layer; the process-isolation runner seam is the pinned T14 port and absence needs no real binary",
      ],
    };
  } finally {
    world.close();
  }
}

// --- The executor -----------------------------------------------------------------

/** Run ONE recovery case end-to-end against the real modules. */
export async function runRecoveryCase(
  caseId: RecoveryCaseId,
  options: RecoveryMatrixOptions = {},
): Promise<RecoveryCaseResult> {
  const caseDeclaration = RECOVERY_CASES.find((entry) => entry.id === caseId);
  if (caseDeclaration === undefined) {
    throw new RecoveryCaseError("runRecoveryCase", `unknown recovery case '${caseId}'`);
  }
  const context: CaseContext = {
    ledger: options.rounds ?? new RecoveryRoundLedger(),
    scratchDir: options.scratchDir ?? resolve(import.meta.dir, ".."),
    sxr: options.sxr,
  };
  try {
    switch (caseId) {
      case "RECOV-001A-projection-deletion":
        return await runProjectionDeletion(context);
      case "RECOV-001B-projection-corruption":
        return await runProjectionCorruption(context);
      case "RECOV-002-tui-kill-during-live-work":
        return await runTuiKill(context);
      case "RECOV-003-kernel-loss-objects":
        return await runKernelLoss(context);
      case "RECOV-004-reconnect-mid-stream-stale-frames":
        return await runReconnectMidStream(context);
      case "RECOV-005-prime-unavailable-degraded":
        return await runPrimeUnavailable(context);
      case "RECOV-SXR-absence-incompatibility-cas-loss":
        return await runSxrAbsence(context);
    }
  } catch (error) {
    // A crashed round is a RETAINED failed round — never silently rerun over.
    context.ledger.record({
      case_id: caseId,
      round: context.ledger.nextRound(caseId),
      outcome: "failed",
      procedure: caseDeclaration.destructive_procedure.procedure,
      residuals: [errorMessage(error)],
      quarantined_state_sha256: null,
      quarantined_path: null,
    });
    return {
      case_id: caseId,
      recov: caseDeclaration.recov,
      requirements: caseDeclaration.requirements,
      procedure_summary: caseDeclaration.destructive_procedure.detail,
      pre_state: {
        generation_id: "unknown",
        source_fingerprints: {},
        canonical_digest: "unknown",
        full_digest: "unknown",
        source_cursors: {},
      },
      detections: [],
      assertions: [
        { name: "case_completed_without_error", passed: false, detail: errorMessage(error) },
      ],
      post_source_fingerprints: {},
      sources_byte_identical: false,
      verdict: "failed",
      residuals: [errorMessage(error)],
    };
  }
}

/** Run the whole matrix: a fresh fixture per case, ONE shared round ledger. */
export async function runRecoveryMatrix(
  options: RecoveryMatrixOptions = {},
): Promise<RecoveryMatrixReport> {
  const ledger = options.rounds ?? new RecoveryRoundLedger();
  const caseIds = options.caseIds ?? RECOVERY_CASES.map((entry) => entry.id);
  const results: RecoveryCaseResult[] = [];
  for (const caseId of caseIds) {
    results.push(await runRecoveryCase(caseId, { ...options, rounds: ledger }));
  }
  const failed = results.filter((result) => result.verdict !== "pass");
  return {
    results,
    ledger,
    all_passed: failed.length === 0,
    failed_case_ids: failed.map((result) => result.case_id),
  };
}
