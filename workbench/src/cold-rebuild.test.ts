/**
 * T22 cold-rebuild attacks (plan T22 teeth; prereg P1/P2, falsifiers 1 and 2).
 *
 * The gate selector `just tui-test cold-rebuild` selects this file by name.
 *
 * Attack 1 (falsifier 1): corrupt the local projection rows so they look
 * SELF-CONSISTENT but disagree with the authoritative cursors. Expected: the
 * workbench detects the stale/corrupt state and rebuilds/resyncs; it NEVER
 * mutates the authoritative sources to preserve the local view. The crafted
 * corruption here is crafted INDEPENDENTLY of the matrix case (different
 * mutation set: both sources' generation cursors forged, phantom unit AND
 * phantom evidence rows, a tampered real row).
 *
 * Attack 2 (falsifier 2): kill the TUI AND the kernel simultaneously during a
 * real Run while a Prime child continues. Expected: canonical truth and
 * authorized Prime work survive according to policy; fresh workbench
 * reconstruction recovers the real state.
 *
 * The independent referee for every "equals the authoritative fold" claim is
 * T19's normalized projection (`foldLogToNormalized` / `normalizeSnapshot` /
 * `normalizeLiveFold` + canonical-JSON sha256 digests). Real-process kills are
 * deterministic: the harness spawns the TUI child, awaits its readiness
 * marker, SIGKILLs it, and reaps it — no racing window (prereg blindspots:
 * cross-process Prime reality is T24, so the kernel kill is the declared
 * in-model boundary).
 */

import { Database } from "bun:sqlite";
import { afterAll, describe, expect, it } from "bun:test";
import { readdirSync, rmSync } from "node:fs";
import { join, resolve } from "node:path";
import type { RecordedKernelObject } from "./headless-replay";
import {
  canonicalJson,
  canonicalSections,
  foldLogToNormalized,
  normalizeLiveFold,
  normalizeSnapshot,
  projectionDigest,
  recordRun,
} from "./headless-replay";
import { LiveStreamClient } from "./live-streams";
import { SqliteProjectionStore } from "./projection-store";
import {
  auditProjectionRows,
  buildMatrixWorld,
  captureRecoveryPreState,
  cleanupScratchDb,
  fileSha256,
  fingerprintPort,
  fingerprintsDifferences,
  KERNEL_ARTIFACT_LOCATOR,
  quarantineDatabase,
  RECORDED_AT_MS,
  RECORDED_RUN_ID,
  readProjectionRows,
  recordedRunFromLog,
  recordKernelObject,
  referenceModel,
  snapshotsConverged,
  spawnRealTuiProcess,
} from "./recovery-matrix";
import { WorkbenchModel } from "./workbench-model";

const SCRATCH_DIR = resolve(import.meta.dir, "..");

/** Sweep ONLY this file's task-prefixed scratch artifacts. */
function sweepT22ColdScratch(): void {
  let entries: string[] = [];
  try {
    entries = readdirSync(SCRATCH_DIR);
  } catch {
    return;
  }
  for (const entry of entries) {
    if (!entry.startsWith("test-t22-cold-")) {
      continue;
    }
    try {
      rmSync(join(SCRATCH_DIR, entry), { force: true });
    } catch {
      // best-effort cleanup
    }
  }
}

afterAll(sweepT22ColdScratch);

// --- Attack 1 ---------------------------------------------------------------------

describe("T22 Attack 1 (prereg falsifier 1): crafted self-consistent corruption", () => {
  it("rows that agree with each other but disagree with the authoritative cursors are detected via cursor/epoch validation, never trusted, and the sources stay byte-identical; the quarantine rebuild is digest-equal under the T19 referee", async () => {
    const generationId = "gen-t22-cold-attack1";
    const dbPath = join(SCRATCH_DIR, "test-t22-cold-attack1.db");
    const world = await buildMatrixWorld({ dbPath, generationId });
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
      // The generation the LIVE file rows carry (the client minter suffixes a
      // serial) — the fold→file audit is pinned to it until the quarantine
      // rebuild, whose rows carry `generationId`.
      const liveGeneration = world.client.generationId ?? `${generationId}-1`;
      world.client.close();
      world.model.close();

      // CRAFT (independent mutation set): one internally coherent corrupt
      // world — BOTH sources' generation cursors forged to never-minted
      // tokens, a phantom unit and a phantom evidence row carrying those
      // tokens, and a tampered real row. The rows agree with EACH OTHER;
      // every cursor disagrees with the authoritative source.
      const forgedGauntlet = "cursor-t22-cold-lie-g";
      const forgedPrime = "cursor-t22-cold-lie-p";
      const craft = new Database(dbPath);
      try {
        const tx = craft.transaction(() => {
          craft.run(
            `INSERT OR REPLACE INTO source_cursors (generation_id, source_id, cursor, updated_at)
               VALUES (?, 'gauntlet', ?, ?), (?, 'prime', ?, ?);`,
            [
              generationId,
              forgedGauntlet,
              RECORDED_AT_MS,
              generationId,
              forgedPrime,
              RECORDED_AT_MS,
            ],
          );
          craft.run(
            `INSERT OR REPLACE INTO units_projection
               (unit_id, generation_id, source_id, source_cursor, title, phase, status, authority, blast_radius, updated_at)
               VALUES ('unit-phantom-1', ?, 'gauntlet', ?, 'Fabricated', 'phaseX', 'resolved', 'builder', 'project', ?);`,
            [generationId, forgedGauntlet, RECORDED_AT_MS],
          );
          craft.run(
            `INSERT OR REPLACE INTO evidence_projection
               (evidence_id, generation_id, source_id, source_cursor, unit_id, kind, locator, verified, updated_at)
               VALUES ('ev-phantom-1', ?, 'prime', ?, 'unit-phantom-1', 'report', 'cas://sha256:fabricated', 1, ?);`,
            [generationId, forgedPrime, RECORDED_AT_MS],
          );
          craft.run(
            `UPDATE units_projection SET status = 'fabricated-complete', title = 'Unit 401 (fabricated)'
               WHERE unit_id = 'AK-401';`,
          );
        });
        tx();
      } finally {
        craft.close();
      }

      // The corruption never touched the authorities.
      expect(fingerprintsDifferences(fingerprints, fingerprintPort(world.port))).toEqual([]);

      // DETECTION: cursor/epoch validation against the minted-cursor window,
      // cross-checked against the T19 pure log fold.
      const expected = foldLogToNormalized(recordedRunFromLog({ generationId, events: log }), {
        kernelAlive: false,
      });
      const findings = auditProjectionRows({
        rows: readProjectionRows(dbPath),
        fingerprints,
        expected,
        expectedGenerationId: liveGeneration,
      });
      const codes = new Set(findings.map((finding) => finding.code));
      expect(findings.length).toBeGreaterThanOrEqual(6);
      expect(codes.has("generation_cursor_not_minted")).toBe(true);
      expect(codes.has("cursor_not_minted_by_source")).toBe(true);
      expect(codes.has("row_disagrees_with_authoritative_fold")).toBe(true);
      // The referee names the exact lies (locus carries the row identity).
      const details = findings
        .map((finding) => `${finding.code}@${finding.locus}: ${finding.detail}`)
        .join(" | ");
      expect(details).toContain(forgedGauntlet);
      expect(details).toContain(forgedPrime);
      expect(details).toContain("unit-phantom-1");
      expect(details).toContain("ev-phantom-1");

      // The tempting wrong fix — rebuilding WITHOUT quarantine — leaves the
      // coherent corrupt rows in the file (INSERT OR REPLACE never deletes).
      const naive = SqliteProjectionStore.rebuildFromAuthoritativeLog(
        dbPath,
        `${generationId}-naive`,
        log,
      );
      const naiveFindings = auditProjectionRows({
        rows: readProjectionRows(dbPath),
        fingerprints,
        expected,
        expectedGenerationId: liveGeneration,
      });
      naive.close();
      expect(naiveFindings.length).toBeGreaterThanOrEqual(6);

      // The declared recovery: quarantine (state RETAINED, sha256-pinned),
      // then a cold rebuild from the authoritative log at the same path.
      const quarantined = quarantineDatabase(dbPath, "attack1");
      expect(fileSha256(quarantined.quarantined_path)).toBe(quarantined.sha256);
      const rebuilt = SqliteProjectionStore.rebuildFromAuthoritativeLog(dbPath, generationId, log);
      const postNormalized = normalizeSnapshot(rebuilt.getSnapshot(null), {
        run_id: RECORDED_RUN_ID,
        events: log,
        kernelObjects: [],
        kernelAlive: false,
      });
      // The T19 referee: pure log fold and rebuilt store agree byte-for-byte.
      expect(canonicalJson(postNormalized)).toBe(canonicalJson(expected));
      // Both digests equal the pre-corruption state (REQ-VERIFY-001).
      expect(projectionDigest(canonicalSections(postNormalized))).toBe(pre.canonical_digest);
      expect(projectionDigest(postNormalized)).toBe(pre.full_digest);
      // Zero corrupt rows survive the declared procedure.
      expect(
        auditProjectionRows({
          rows: readProjectionRows(dbPath),
          fingerprints,
          expected,
          expectedGenerationId: generationId,
        }).length,
      ).toBe(0);
      // And the authorities are STILL byte-identical (REQ-SAFE-005).
      expect(fingerprintsDifferences(fingerprints, fingerprintPort(world.port))).toEqual([]);
      rebuilt.close();
    } finally {
      world.close();
      cleanupScratchDb(dbPath);
    }
  }, 60_000);
});

// --- Attack 2 ---------------------------------------------------------------------

describe("T22 Attack 2 (prereg falsifier 2): simultaneous TUI+kernel kill during a live run", () => {
  it("while a Prime child continues, canonical truth and authorized work survive: fresh reconstruction (cold rebuild, pure log fold, fresh host) equals the authoritative fold under the T19 referee, lost kernel values are typed unavailable, and completion comes only from Prime's own contract", async () => {
    const generationId = "gen-t22-cold-attack2";
    const dbPath = join(SCRATCH_DIR, "test-t22-cold-attack2.db");
    const rebuiltPath = join(SCRATCH_DIR, "test-t22-cold-attack2-rebuilt.db");
    const world = await buildMatrixWorld({ dbPath, generationId, withKernel: true });
    const kernel = world.kernel;
    const ledger = world.ledger;
    if (kernel === null || ledger === null) {
      throw new Error("the Attack 2 fixture requires a kernel and ledger");
    }
    let reference: WorkbenchModel | null = null;
    try {
      // The live run produces one more kernel value mid-Run.
      const lateRun = await kernel.execute("late_result = 7");
      const registered = ledger.register(lateRun);
      const allKernelObjects: RecordedKernelObject[] = [
        ...world.kernelObjects,
        ...registered.map(recordKernelObject),
      ];

      // Pre-kill evidence: the authoritative state the reconstruction must cover.
      const prefixLog = world.authoritativeLog();
      const prefixFold = foldLogToNormalized(
        recordedRunFromLog({ generationId, events: prefixLog, kernelObjects: allKernelObjects }),
        { kernelAlive: true },
      );

      // Spawn the REAL TUI process (readiness awaited), then the SIMULTANEOUS
      // kill phase: the SIGKILL is issued, the in-process host is closed, and
      // the kernel is terminated — one synchronous block, no racing window.
      const tui = await spawnRealTuiProcess();
      const reaping = tui.killAndReap();
      world.client.close();
      kernel.terminate();
      world.model.close();
      const reaped = await reaping;
      expect(reaped.signal === "SIGKILL" || reaped.exit_code === 137).toBe(true);

      // The Prime child CONTINUES: progress, a mailbox claim that is NOT
      // completion, and authoritative frames landing on the sources.
      world.adapter.receiveMailboxMessage("prime-child-1", {
        sender: "prime-child-1",
        content: "almost done",
        kind: "signal",
      });
      const midFlight = world.adapter.getSession("prime-child-1");
      expect(midFlight?.handle.status).toBe("running");
      expect(midFlight?.resultContract).toBeNull();
      world.gauntlet.append({
        event_type: "unit_updated",
        payload: { unit_id: "AK-401", status: "review" },
        timestamp_ms: 1_760_000_000_000 + 40,
      });
      world.prime.append({
        event_type: "session_updated",
        payload: { session_id: "prime-child-1", parent_id: "prime-root", status: "running" },
        timestamp_ms: 1_760_000_000_000 + 41,
      });
      // Prime completes the child through ITS OWN schema-valid contract.
      expect(
        world.adapter.deliverResultContract("prime-child-1", {
          session_id: "prime-child-1",
          schema_valid: true,
          outcome: "pass",
          evidence_path:
            ".agents/reports/gauntlet-tui-prime-workbench/evidence/T22/contracts/child-1-result.md",
          completed_at: "2026-09-09T12:30:00Z",
        }),
      ).toBe(true);

      // The authoritative sources are now final; fingerprint them for the
      // byte-identical-across-recovery check.
      const fingerprintsAfterAuthoritativePhases = fingerprintPort(world.port);
      const coldGeneration = "gen-t22-cold-attack2-rebuild";
      const fullLog = world.authoritativeLog(coldGeneration);

      // FRESH RECONSTRUCTION, path 1: cold rebuild into a brand-new SQLite
      // file straight from the authoritative log.
      const rebuilt = SqliteProjectionStore.rebuildFromAuthoritativeLog(
        rebuiltPath,
        coldGeneration,
        fullLog,
      );
      const viaStore = normalizeSnapshot(rebuilt.getSnapshot(null), {
        run_id: RECORDED_RUN_ID,
        events: fullLog,
        kernelObjects: allKernelObjects,
        kernelAlive: false,
      });

      // FRESH RECONSTRUCTION, path 2 (the independent referee): the pure log
      // fold — zero SQLite, zero store, zero UI.
      const viaLog = foldLogToNormalized(
        recordedRunFromLog({
          generationId: coldGeneration,
          events: fullLog,
          kernelObjects: allKernelObjects,
        }),
        { kernelAlive: false },
      );
      expect(canonicalJson(viaStore)).toBe(canonicalJson(viaLog));

      // FRESH RECONSTRUCTION, path 3: a fresh workbench host over the
      // projection file that SURVIVED the TUI death, reattached to the real
      // Prime records.
      const freshModel = new WorkbenchModel({ dbPath });
      const freshClient = new LiveStreamClient({
        model: freshModel,
        port: world.port,
        mintGenerationId: (serial) => `${generationId}-fresh-${serial}`,
      });
      const receipt = freshClient.reattach({ prime_adapter: world.adapter });
      expect(receipt.cancel_calls_issued).toBe(0);
      expect(receipt.presentation_source).toBe("fresh_baseline_no_persisted_state");
      expect(receipt.prime?.completed_sessions).toContain("prime-child-1");
      expect(receipt.prime?.running_sessions).not.toContain("prime-child-1");
      const liveGeneration = freshClient.generationId ?? `${generationId}-fresh-1`;
      reference = referenceModel(world.port, liveGeneration);
      expect(
        snapshotsConverged(reference.store.getSnapshot(null), freshModel.store.getSnapshot(null)),
      ).toEqual([]);

      // Correction history: the fresh host's OWN trace fan-out equals the
      // log-derived audit trail — no history rewritten, none lost.
      const liveNormalized = normalizeLiveFold({
        run_id: RECORDED_RUN_ID,
        generation_id: liveGeneration,
        storeSnapshot: freshModel.store.getSnapshot(null),
        trace: freshModel.getTrace(),
        kernelObjects: allKernelObjects,
        kernelAlive: false,
      });
      expect(JSON.stringify(liveNormalized.trace)).toBe(JSON.stringify(viaLog.trace));
      // The pre-kill audit trail is preserved APPEND-ONLY: per source, the
      // recovered trace starts with exactly the pre-kill entries (canonical
      // order interleaves sources, so the prefix property holds per source).
      const prefixSources = [...new Set(prefixFold.trace.map((entry) => entry.source_id))].sort();
      for (const sourceId of prefixSources) {
        expect(
          viaLog.trace
            .filter((entry) => entry.source_id === sourceId)
            .slice(0, prefixFold.trace.filter((entry) => entry.source_id === sourceId).length),
        ).toEqual(prefixFold.trace.filter((entry) => entry.source_id === sourceId));
      }

      // Canonical truth survived: identities, the post-kill fact, evidence,
      // both sessions, and every cursor at the sources' true watermark.
      const unit401 = viaLog.units.find((unit) => unit.unit_id === "AK-401");
      expect(unit401?.status).toBe("review");
      expect(viaLog.units.some((unit) => unit.unit_id === "AK-402")).toBe(true);
      expect(viaLog.evidence.some((evidence) => evidence.evidence_id === "ev-401")).toBe(true);
      expect(viaLog.sessions.map((session) => session.session_id).sort()).toEqual([
        "prime-child-1",
        "prime-root",
      ]);
      for (const [sourceId, cursor] of viaLog.source_cursors) {
        const retained = world.port.source(sourceId).retainedEvents();
        expect(retained.at(-1)?.cursor).toBe(cursor);
      }
      for (const sourceId of Object.keys(fingerprintsAfterAuthoritativePhases)) {
        const fingerprint = fingerprintsAfterAuthoritativePhases[sourceId];
        const foldCursor = viaLog.source_cursors.find(([id]) => id === sourceId)?.[1];
        expect(fingerprint === undefined ? undefined : foldCursor).toBeDefined();
      }

      // Lost kernel values are TYPED unavailable — never recreated. Only the
      // explicitly artifactized value survives, via its durable locator.
      const kernelRecords = new Map(viaLog.kernel_objects.map((record) => [record.name, record]));
      expect(kernelRecords.get("metrics")?.value_available).toBe(true);
      expect(kernelRecords.get("metrics")?.artifact_ref).toBe(KERNEL_ARTIFACT_LOCATOR);
      expect(kernelRecords.get("metrics")?.persistence_class).toBe("artifactized");
      expect(kernelRecords.get("scratch")?.value_available).toBe(false);
      expect(kernelRecords.get("scratch")?.unavailability_reason).toBe(
        "kernel_restarted_pinned_value_not_retained",
      );
      expect(kernelRecords.get("scratch")?.artifact_ref).toBeNull();
      expect(kernelRecords.get("probe")?.value_available).toBe(false);
      expect(kernelRecords.get("probe")?.unavailability_reason).toBe(
        "ephemeral_kernel_state_not_reconstructible",
      );
      expect(kernelRecords.get("late_result")?.value_available).toBe(false);
      expect(kernelRecords.get("late_result")?.unavailability_reason).toBe(
        "ephemeral_kernel_state_not_reconstructible",
      );

      // No termination ever rode the adapter, and the sources are
      // byte-identical across the whole fresh-host recovery.
      const noCancel = world.adapterCalls.every(
        (call) => !/cancel|stop|terminate|abort|kill|interrupt/i.test(call),
      );
      expect(noCancel).toBe(true);
      expect(
        fingerprintsDifferences(fingerprintsAfterAuthoritativePhases, fingerprintPort(world.port)),
      ).toEqual([]);

      rebuilt.close();
      freshClient.close();
      freshModel.close();
      reference.close();
      reference = null;
      cleanupScratchDb(rebuiltPath);
    } finally {
      if (reference !== null) {
        try {
          reference.close();
        } catch {
          // already closed
        }
      }
      world.close();
      cleanupScratchDb(dbPath);
      cleanupScratchDb(rebuiltPath);
    }
  }, 120_000);
});

// --- The referee's own teeth -------------------------------------------------------

describe("T22 referee teeth: the T19 normalized projection digest discriminates", () => {
  it("moves when a canonical fact is tampered and stays still when only presentation changes", async () => {
    const generationId = "gen-t22-cold-referee";
    const world = await buildMatrixWorld({ dbPath: ":memory:", generationId });
    try {
      const log = world.authoritativeLog();
      const honest = foldLogToNormalized(recordedRunFromLog({ generationId, events: log }), {
        kernelAlive: false,
      });
      const honestDigest = projectionDigest(honest);

      // Tamper 1: flip a canonical verified bit on a SUPERSEDED-but-real frame
      // — the digest MUST move (this referee does not trust its inputs).
      const tamperedLog = log.map((frame) =>
        frame.event_type === "evidence_added"
          ? { ...frame, payload: { ...frame.payload, verified: true } }
          : frame,
      );
      const tampered = foldLogToNormalized(
        recordedRunFromLog({ generationId, events: tamperedLog }),
        { kernelAlive: false },
      );
      expect(projectionDigest(tampered)).not.toBe(honestDigest);
      // The move is visible where it matters: settlement inputs.
      const honestVerified = honest.settlement_inputs
        .flatMap((input) => input.unverified_evidence_ids)
        .sort();
      const tamperedVerified = tampered.settlement_inputs
        .flatMap((input) => input.verified_evidence_ids)
        .sort();
      expect(tamperedVerified).toEqual(honestVerified);

      // Presentation-only change: placements and operator selections NEVER
      // enter the normalized truth — the digest stays still.
      const withPresentation = recordRun({
        run_id: RECORDED_RUN_ID,
        generation_id: generationId,
        recorded_at_ms: RECORDED_AT_MS,
        events: log,
        kernel: { kernel_generation: "kernel-gen-1", session_ref: "s", objects: [] },
        presentation: {
          placements: [
            {
              id: "placement-1",
              surface: "tui",
              view: "work",
              label_override: "operator label",
            },
          ],
          selected_focus_id: "AK-402",
        },
      });
      const withPresentationFold = foldLogToNormalized(withPresentation, { kernelAlive: false });
      expect(projectionDigest(withPresentationFold)).toBe(honestDigest);
      expect(withPresentationFold.presentation_present).toBe(false);

      // Sanity: the honest canonical digest is stable across refolds.
      const refolded = foldLogToNormalized(recordedRunFromLog({ generationId, events: log }), {
        kernelAlive: false,
      });
      expect(projectionDigest(canonicalSections(refolded))).toBe(
        projectionDigest(canonicalSections(honest)),
      );
    } finally {
      world.close();
    }
  });
});
