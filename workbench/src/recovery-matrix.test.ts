/**
 * T22 recovery-matrix suites (plan T22; RECOV-001..RECOV-005 plus the
 * REQ-SXR-004/REQ-SYNC-003 absence cases; prereg A1/D1..D3, predictions P1/P2).
 *
 * The gate selector `just tui-test recovery-matrix` selects this file by name.
 *
 * Proofs: the DECLARATIVE registry (recovery-matrix.ts) is executed END-TO-END
 * against the real modules — the T06/T07 SQLite projection store (file-backed
 * where destruction targets a file), the T16 live-stream client, the T05
 * kernel + T11 promotion ledger, the T04 Prime adapter, and the T10
 * detach/reattach receipts. Every case asserts its declared
 * rebuild/degraded/reattach contract with correction history preserved. The
 * SXR probes run through the REAL sxr-adapter boundary here (this test file
 * sits outside the renderer-boundary policed universe; the matrix module
 * deliberately does not import the presentation-coupled adapter).
 *
 * Honesty rules: the naive (wrong) recovery attempt genuinely fails its round
 * and that failed round is RETAINED in the append-only ledger; quarantined
 * corrupt databases are sha256-pinned and kept on disk by the recovery (only
 * this file's afterAll sweeps its own scratch prefixes); lost kernel values
 * are marked unavailable, never recreated; a missing SXR probe record is a
 * FAILED case, never a pass.
 */

import { Database } from "bun:sqlite";
import { afterAll, describe, expect, it } from "bun:test";
import { readdirSync, rmSync } from "node:fs";
import { join, resolve } from "node:path";
import { foldLogToNormalized } from "./headless-replay";
import {
  auditProjectionRows,
  buildMatrixWorld,
  type CorruptionFinding,
  type CorruptionFindingCode,
  cleanupScratchDb,
  DEFAULT_READINESS_TIMEOUT_MS,
  FORGED_CURSOR,
  fingerprintPort,
  RECOVERY_CASES,
  RecoveryCaseError,
  type RecoveryCaseId,
  type RecoveryCaseResult,
  RecoveryLedgerError,
  type RecoveryMatrixReport,
  RecoveryRoundLedger,
  readProjectionRows,
  recordedRunFromLog,
  runRecoveryMatrix,
  SCOPE_EXCLUSIONS,
  type SxrAvailabilityRecord,
  spawnRealTuiProcess,
} from "./recovery-matrix";
import {
  executeSxrRefresh,
  parseSxrSlice0bEnvelopeOutput,
  type SxrProcessRunner,
  sxrEntityRef,
} from "./sxr-adapter";

const SCRATCH_DIR = resolve(import.meta.dir, "..");

/** Sweep ONLY this file's task-prefixed scratch artifacts (incl. quarantines). */
function sweepT22Scratch(): void {
  let entries: string[] = [];
  try {
    entries = readdirSync(SCRATCH_DIR);
  } catch {
    return;
  }
  for (const entry of entries) {
    if (!entry.startsWith("test-t22-matrix-")) {
      continue;
    }
    try {
      rmSync(join(SCRATCH_DIR, entry), { force: true });
    } catch {
      // best-effort cleanup
    }
  }
}

afterAll(sweepT22Scratch);

// --- The REAL SXR probes (real sxr-adapter boundary; no binary needed) -----------

async function realSxrProbes(): Promise<SxrAvailabilityRecord> {
  const absentRunner: SxrProcessRunner = {
    kind: "t22-absent-producer-runner",
    run: async () => ({
      class: "spawn_failed",
      detail: "sxr binary not present (declared optional-provider absence)",
    }),
  };
  const absent = await executeSxrRefresh(
    { binary_path: "/usr/bin/sxr", repo_id: "t22-matrix-repo", command: { kind: "ledger_export" } },
    absentRunner,
  );

  const incompatibleRunner: SxrProcessRunner = {
    kind: "t22-incompatible-producer-runner",
    run: async () => ({
      class: "exited",
      exit_code: 0,
      signal: null,
      stdout_bytes: new TextEncoder().encode(
        JSON.stringify({ cep_version: 1, envelope_kind: "domainforge", representations: [] }),
      ),
      stderr_text: "",
    }),
  };
  const incompatible = await executeSxrRefresh(
    {
      binary_path: "/usr/bin/sxr",
      repo_id: "t22-matrix-repo",
      command: { kind: "query_evidence", entity: sxrEntityRef("question:q-22") },
    },
    incompatibleRunner,
  );

  // Lost CAS custody: a content_ref whose object is missing from the store.
  const missingDigest = `sha256:${"0".repeat(64)}`;
  const casLoss = parseSxrSlice0bEnvelopeOutput(
    JSON.stringify({
      representation: null,
      cep_envelope: {
        representations: [{ content_ref: `cas://${missingDigest}`, content_hash: missingDigest }],
      },
    }),
    { read: () => undefined },
  );

  const incompatibleVerdict =
    incompatible.class === "refreshed" && incompatible.parse.status === "incompatible"
      ? `incompatible:${incompatible.parse.reason}`
      : `unexpected:${incompatible.class}`;
  const casVerdict =
    casLoss.status === "invalid" && casLoss.cas_failure !== undefined
      ? casLoss.cas_failure
      : "unexpected";
  return {
    producer_absent_outcome: absent.class,
    producer_incompatible_parse: incompatibleVerdict,
    cas_loss_failure: casVerdict,
    fabricated_rows: 0,
  };
}

// --- Helpers ----------------------------------------------------------------------

let matrixReport: RecoveryMatrixReport | null = null;
let matrixLedger: RecoveryRoundLedger | null = null;

function requireMatrixReport(): RecoveryMatrixReport {
  if (matrixReport === null || matrixLedger === null) {
    throw new Error("the matrix has not run yet; tests below depend on the end-to-end run");
  }
  return matrixReport;
}

function matrixResult(caseId: RecoveryCaseId): RecoveryCaseResult {
  const result = requireMatrixReport().results.find((entry) => entry.case_id === caseId);
  if (result === undefined) {
    throw new Error(`case '${caseId}' is missing from the matrix report`);
  }
  return result;
}

function assertPassed(result: RecoveryCaseResult, name: string): void {
  const assertion = result.assertions.find((entry) => entry.name === name);
  expect(
    assertion === undefined
      ? `missing assertion '${name}' in ${result.case_id}: ${JSON.stringify(result.assertions.map((a) => a.name))}`
      : assertion.passed
        ? true
        : `assertion '${name}' failed: ${assertion.detail}`,
  ).toBe(true);
}

// --- The suites -------------------------------------------------------------------

describe("T22 recovery matrix: the declarative registry (prereg A1)", () => {
  it("freezes every applicable RECOV case with a destructive procedure, pre-state capture, and an expected contract", () => {
    expect(Object.isFrozen(RECOVERY_CASES)).toBe(true);
    const ids = RECOVERY_CASES.map((entry) => entry.id);
    expect(new Set(ids).size).toBe(ids.length);
    expect(ids).toEqual([
      "RECOV-001A-projection-deletion",
      "RECOV-001B-projection-corruption",
      "RECOV-002-tui-kill-during-live-work",
      "RECOV-003-kernel-loss-objects",
      "RECOV-004-reconnect-mid-stream-stale-frames",
      "RECOV-005-prime-unavailable-degraded",
      "RECOV-SXR-absence-incompatibility-cas-loss",
    ]);
    for (const entry of RECOVERY_CASES) {
      expect(Object.isFrozen(entry)).toBe(true);
      expect(Object.isFrozen(entry.destructive_procedure)).toBe(true);
      expect(Object.isFrozen(entry.expected_post_state)).toBe(true);
      expect(entry.expected_post_state.length).toBeGreaterThan(0);
      expect(entry.pre_state_capture.length).toBeGreaterThan(0);
      expect(entry.requirements.length).toBeGreaterThan(0);
    }
    const corruption = RECOVERY_CASES.find(
      (entry) => entry.id === "RECOV-001B-projection-corruption",
    );
    const corruptionProcedure = corruption?.destructive_procedure;
    if (corruptionProcedure?.procedure !== "craft_self_consistent_corruption") {
      throw new Error("the corruption case declaration is missing its procedure");
    }
    expect(corruptionProcedure.mutations.length).toBeGreaterThanOrEqual(3);
    expect(corruption?.recov).toBe("RECOV-001");
  });

  it("records RECOV-006 and isomorphism as DECLARED exclusions, never silent gaps", () => {
    expect(RECOVERY_CASES.map((entry) => entry.recov)).not.toContain("RECOV-006");
    const scopes = SCOPE_EXCLUSIONS.map((entry) => `${entry.scope} ${entry.excluded_because}`).join(
      " | ",
    );
    expect(scopes).toContain("RECOV-006");
    expect(scopes).toContain("T15");
    expect(scopes).toContain("T23");
    expect(scopes).toContain("T24");
    for (const exclusion of SCOPE_EXCLUSIONS) {
      expect(exclusion.excluded_because.length).toBeGreaterThan(20);
    }
  });
});

describe("T22 recovery matrix: end-to-end execution against the real modules", () => {
  let sxrRecord: SxrAvailabilityRecord | null = null;

  it("runs the WHOLE matrix; every case passes with pre-state captured and sources preserved", async () => {
    sxrRecord = await realSxrProbes();
    const ledger = new RecoveryRoundLedger();
    const report = await runRecoveryMatrix({ rounds: ledger, sxr: sxrRecord });
    matrixReport = report;
    matrixLedger = ledger;

    expect(report.failed_case_ids).toEqual([]);
    expect(report.all_passed).toBe(true);
    expect(report.results.length).toBe(RECOVERY_CASES.length);
    for (const result of report.results) {
      const failed = result.assertions.filter((entry) => !entry.passed);
      expect(failed).toEqual([]);
      expect(Object.keys(result.pre_state.source_fingerprints).sort()).toEqual([
        "gauntlet",
        "prime",
      ]);
      expect(result.pre_state.canonical_digest).toMatch(/^[0-9a-f]{64}$/);
      // The reconnect case legitimately ends on the source's OWN new epoch;
      // every other case must leave the sources byte-identical.
      if (result.case_id !== "RECOV-004-reconnect-mid-stream-stale-frames") {
        expect(result.sources_byte_identical).toBe(true);
      }
      // Every case recorded at least one round.
      expect(ledger.roundsFor(result.case_id).length).toBeGreaterThanOrEqual(1);
    }
  }, 120_000);

  it("RECOV-001A teeth: the deleted projection database cold-rebuilds digest-equal with the T19 referee agreeing byte-for-byte", () => {
    const result = matrixResult("RECOV-001A-projection-deletion");
    assertPassed(result, "database_file_deleted");
    assertPassed(result, "cold_rebuild_canonical_digest_equal");
    assertPassed(result, "cold_rebuild_full_digest_equal");
    assertPassed(result, "t19_referee_log_fold_byte_equal");
    assertPassed(result, "authoritative_sources_byte_identical");
  });

  it("RECOV-001B teeth (prereg D1): self-consistent corruption is DETECTED via cursor/epoch validation before any recovery, and the sources are never mutated to preserve it", () => {
    const result = matrixResult("RECOV-001B-projection-corruption");
    assertPassed(result, "corruption_detected_before_recovery");
    assertPassed(result, "pre_corruption_audit_clean");
    assertPassed(result, "sources_never_touched_by_corruption");
    assertPassed(result, "declared_recovery_leaves_zero_corrupt_rows");
    assertPassed(result, "rebuilt_digests_equal_pre_state");
    assertPassed(result, "epoch_validation_detected_forged_persisted_epoch");
    // The forged cursor appears verbatim in the typed detections.
    expect(result.detections.some((detection) => detection.includes(FORGED_CURSOR))).toBe(true);
    expect(result.detections.some((d) => d.startsWith("generation_cursor_not_minted@"))).toBe(true);
    expect(result.detections.some((d) => d.startsWith("cursor_not_minted_by_source@"))).toBe(true);
  });

  it("teeth (prereg D3): the naive rebuild genuinely FAILED, that failed round is RETAINED first in the ledger, and the corrupt state was quarantined by sha256", () => {
    const ledger = requireMatrixReport().ledger;
    const rounds = ledger.roundsFor("RECOV-001B-projection-corruption");
    // Round 1: the naive rebuild — genuinely failed, retained forever.
    // Round 2: the declared procedure (quarantine + cold rebuild + epoch
    // validation) — passed, carrying the quarantined state's sha256.
    expect(rounds.map((round) => round.outcome)).toEqual(["failed", "pass"]);
    const failed = ledger.failedRounds("RECOV-001B-projection-corruption");
    expect(failed.length).toBe(1);
    expect(failed[0]?.procedure).toBe("naive_rebuild_without_quarantine");
    expect(failed[0]?.residuals.length).toBeGreaterThan(0);
    expect(failed[0]?.round).toBe(1);
    expect(rounds[0]?.outcome).toBe("failed");
    const declared = rounds.find((round) => round.quarantined_state_sha256 !== null);
    expect(declared?.quarantined_state_sha256).toMatch(/^[0-9a-f]{64}$/);
    expect(declared?.quarantined_path).toContain(".corrupt");
    expect(declared?.procedure).toContain("quarantine_and_cold_rebuild");
  });

  it("RECOV-002 teeth: the real TUI process died by SIGKILL, authorized work survived, no cancel ever rode the adapter, and completion came from Prime's own contract", () => {
    const result = matrixResult("RECOV-002-tui-kill-during-live-work");
    assertPassed(result, "real_tui_process_killed_by_sigkill");
    assertPassed(result, "authorized_work_survives_host_death");
    assertPassed(result, "events_since_last_cursor_reconciled");
    assertPassed(result, "fresh_host_converged_with_authoritative_fold");
    assertPassed(result, "correction_history_preserved");
    assertPassed(result, "reattach_issues_no_cancel");
    assertPassed(result, "completion_via_prime_contract");
    assertPassed(result, "reattached_completion_from_records");
    assertPassed(result, "adapter_call_log_records_no_termination");
  });

  it("RECOV-003 teeth (honesty rule): lost pinned/ephemeral values are marked UNAVAILABLE with typed reasons and never recreated; the artifactized value survives only via its locator", () => {
    const result = matrixResult("RECOV-003-kernel-loss-objects");
    assertPassed(result, "t11_ledger_fails_closed_after_kernel_loss");
    assertPassed(result, "old_reference_never_revives");
    assertPassed(result, "name_reuse_mints_new_ephemeral_object");
    assertPassed(result, "stale_generation_pin_refused");
    assertPassed(result, "artifactized_survives_via_locator_only");
    assertPassed(result, "pinned_value_unavailable_not_recreated");
    assertPassed(result, "ephemeral_value_unavailable_not_recreated");
    assertPassed(result, "canonical_truth_untouched_by_kernel_death");
    assertPassed(result, "restarted_kernel_cannot_settle_by_liveness");
    expect(result.detections).toContain("pin_after_kernel_loss:kernel_object_lost");
    expect(result.detections).toContain("artifactize_after_kernel_loss:kernel_object_lost");
    expect(result.detections).toContain("stale_generation_pin:unknown_object");
    expect(result.detections).toContain(
      "pinned_after_loss:kernel_restarted_pinned_value_not_retained",
    );
    expect(result.detections).toContain(
      "ephemeral_after_loss:ephemeral_kernel_state_not_reconstructible",
    );
  });

  it("RECOV-004 teeth: old-generation late frames are all rejected, the gap is buffered and drained, and the old epoch forces a visible resync that converges", () => {
    const result = matrixResult("RECOV-004-reconnect-mid-stream-stale-frames");
    assertPassed(result, "fresh_generation_baselined_in_flight_events");
    assertPassed(result, "all_old_generation_late_frames_rejected");
    assertPassed(result, "sequence_gap_bounded_buffered");
    assertPassed(result, "gap_drained_in_canonical_order");
    assertPassed(result, "old_epoch_frame_triggers_visible_resync");
    assertPassed(result, "resync_minted_fresh_generation");
    assertPassed(result, "subscription_outliving_generation_rejected");
    assertPassed(result, "pre_kill_subscription_frames_rejected");
    assertPassed(result, "reconnection_converged_with_authoritative_fold");
    assertPassed(result, "only_declared_epoch_replacement_changed_sources");
  });

  it("RECOV-005 teeth: Prime loss degrades CAPABILITY-SCOPED with typed reason codes while Gauntlet inspection stays valid; restoration resyncs to a fresh converged generation", () => {
    const result = matrixResult("RECOV-005-prime-unavailable-degraded");
    assertPassed(result, "prime_affordance_degrades_with_typed_reason");
    assertPassed(result, "canonical_inspection_unaffected");
    assertPassed(result, "unit_affordances_remain");
    assertPassed(result, "prime_capability_restored");
    assertPassed(result, "restoration_resyncs_to_fresh_generation");
    assertPassed(result, "post_restoration_converged");
    expect(result.detections).toContain("recovery_gate_not_passed");
  });

  it("RECOV-SXR teeth: producer absence, incompatibility, and CAS loss are TYPED through the real adapter and fabricate zero rows", () => {
    expect(sxrRecord?.producer_absent_outcome).toBe("spawn_failed");
    expect(sxrRecord?.producer_incompatible_parse).toBe("incompatible:cep_envelope_not_sxr_output");
    expect(sxrRecord?.cas_loss_failure).toBe("cas_object_missing");
    const result = matrixResult("RECOV-SXR-absence-incompatibility-cas-loss");
    assertPassed(result, "producer_absence_typed");
    assertPassed(result, "producer_incompatibility_typed");
    assertPassed(result, "cas_loss_typed");
    assertPassed(result, "zero_fabricated_rows");
  });
});

describe("T22 recovery round ledger", () => {
  it("is append-only: a round that once failed can never be overwritten or erased", () => {
    const ledger = new RecoveryRoundLedger();
    const failed = ledger.record({
      case_id: "probe-case",
      round: 1,
      outcome: "failed",
      procedure: "probe",
      residuals: ["the original failure"],
      quarantined_state_sha256: null,
      quarantined_path: null,
    });
    expect(() =>
      ledger.record({
        case_id: "probe-case",
        round: 1,
        outcome: "pass",
        procedure: "probe",
        residuals: [],
        quarantined_state_sha256: null,
        quarantined_path: null,
      }),
    ).toThrow(RecoveryLedgerError);
    // The failed record stands, unmodified.
    expect(ledger.roundsFor("probe-case").length).toBe(1);
    expect(ledger.roundsFor("probe-case")[0]?.outcome).toBe("failed");
    expect(ledger.roundsFor("probe-case")[0]?.residuals).toEqual(["the original failure"]);
    expect(failed.outcome).toBe("failed");
  });
});

describe("T22 replay transfer test (prereg L1)", () => {
  it("every destructive case replays with identical verdicts and stable canonical digests", async () => {
    const first = requireMatrixReport();
    const sxr = await realSxrProbes();
    const second = await runRecoveryMatrix({ rounds: new RecoveryRoundLedger(), sxr });
    expect(second.all_passed).toBe(true);
    const signature = (report: RecoveryMatrixReport) =>
      report.results.map((result) => ({
        id: result.case_id,
        verdict: result.verdict,
        canonical: result.pre_state.canonical_digest,
        assertions: result.assertions.map((entry) => entry.name),
      }));
    expect(signature(second)).toEqual(signature(first));
  }, 120_000);
});

// --- Correction round 1 (adversarial verification findings) ----------------------

describe("T22 correction round 1 (adversarial verification findings)", () => {
  // Shared fixture for the MAJOR-1 shapes: build the seeded world, capture the
  // LIVE generation (the file rows carry it) plus the fold/fingerprints, then
  // hand the caller an audit bound to that generation.
  async function auditFixture(shape: string) {
    const dbPath = join(SCRATCH_DIR, `test-t22-matrix-audit-${shape}.db`);
    const world = await buildMatrixWorld({ dbPath, generationId: `gen-t22-audit-${shape}` });
    const liveGeneration = world.client.generationId ?? `gen-t22-audit-${shape}-1`;
    const log = world.authoritativeLog(liveGeneration);
    const fingerprints = fingerprintPort(world.port);
    const expected = foldLogToNormalized(
      recordedRunFromLog({ generationId: liveGeneration, events: log }),
      { kernelAlive: false },
    );
    world.client.close();
    world.model.close();
    const audit = () =>
      auditProjectionRows({
        rows: readProjectionRows(dbPath),
        fingerprints,
        expected,
        expectedGenerationId: liveGeneration,
      });
    return {
      dbPath,
      audit,
      cleanup() {
        world.close();
        cleanupScratchDb(dbPath);
      },
    };
  }

  function findingFor(
    findings: readonly CorruptionFinding[],
    code: CorruptionFindingCode,
    locus: string,
  ): { code: CorruptionFindingCode; locus: string; detail: string } | undefined {
    const found = findings.find((entry) => entry.code === code && entry.locus === locus);
    return found === undefined
      ? undefined
      : { code: found.code, locus: found.locus, detail: found.detail };
  }

  it("MAJOR-1(a): a DELETED real unit row with intact cursors is a typed finding (was audit-silent)", async () => {
    const fixture = await auditFixture("deleted-row");
    try {
      const db = new Database(fixture.dbPath);
      try {
        db.run("DELETE FROM units_projection WHERE unit_id = 'AK-402'");
      } finally {
        db.close();
      }
      const findings = fixture.audit();
      const finding = findingFor(findings, "expected_row_missing_from_file", "units:AK-402");
      expect(
        finding === undefined
          ? "AUDIT-SILENT: no expected_row_missing_from_file@units:AK-402"
          : true,
      ).toBe(true);
      expect(finding?.detail).toContain("deleted or lost row");
      // The untouched sibling is still clean: the finding is precise, not noise.
      expect(
        findingFor(findings, "expected_row_missing_from_file", "units:AK-401"),
      ).toBeUndefined();
    } finally {
      fixture.cleanup();
    }
  });

  it("MAJOR-1(b): a row relabeled to a zombie generation_id is a typed finding naming the relabel (was audit-silent)", async () => {
    const fixture = await auditFixture("zombie-generation");
    try {
      const db = new Database(fixture.dbPath);
      try {
        db.run("UPDATE units_projection SET generation_id = 'gen-zombie' WHERE unit_id = 'AK-402'");
      } finally {
        db.close();
      }
      const findings = fixture.audit();
      const finding = findingFor(findings, "expected_row_missing_from_file", "units:AK-402");
      expect(
        finding === undefined
          ? "AUDIT-SILENT: no expected_row_missing_from_file@units:AK-402"
          : true,
      ).toBe(true);
      expect(finding?.detail).toContain("relabeled to generation(s): gen-zombie");
    } finally {
      fixture.cleanup();
    }
  });

  it("MAJOR-1(c): an updated_at tamper is a typed finding (was audit-silent)", async () => {
    const fixture = await auditFixture("updated-at-tamper");
    try {
      const db = new Database(fixture.dbPath);
      try {
        db.run("UPDATE units_projection SET updated_at = 999999 WHERE unit_id = 'AK-401'");
      } finally {
        db.close();
      }
      const findings = fixture.audit();
      const finding = findingFor(
        findings,
        "row_updated_at_disagrees_with_authoritative_fold",
        "units:AK-401",
      );
      expect(
        finding === undefined ? "AUDIT-SILENT: no updated_at finding@units:AK-401" : true,
      ).toBe(true);
      expect(finding?.detail).toContain("999999");
    } finally {
      fixture.cleanup();
    }
  });

  it("MAJOR-1(d): a focus_projection tamper is a typed finding (was audit-silent)", async () => {
    const fixture = await auditFixture("focus-tamper");
    try {
      const db = new Database(fixture.dbPath);
      try {
        db.run("UPDATE focus_projection SET focus_id = 'AK-402'");
      } finally {
        db.close();
      }
      const findings = fixture.audit();
      const finding = findingFor(findings, "focus_row_disagrees_with_authoritative_fold", "focus");
      expect(finding === undefined ? "AUDIT-SILENT: no focus finding" : true).toBe(true);
      expect(finding?.detail).toContain("disagrees with the authoritative fold WorkItem:AK-401");
    } finally {
      fixture.cleanup();
    }
  });

  it("MAJOR-1 control: the UNtampered fixture still audits completely clean under the fold→file direction", async () => {
    const fixture = await auditFixture("control-clean");
    try {
      expect(fixture.audit()).toEqual([]);
    } finally {
      fixture.cleanup();
    }
  });

  it("MINOR-1: a TUI child that never signals readiness is killed and fails within the deadline instead of hanging the suite", async () => {
    let error: unknown = null;
    try {
      await spawnRealTuiProcess({
        readinessTimeoutMs: 300,
        childScript: "setInterval(() => {}, 1_000_000);",
      });
    } catch (caught) {
      error = caught;
    }
    expect(error).toBeInstanceOf(RecoveryCaseError);
    expect(error instanceof Error ? error.message : String(error)).toContain(
      "no readiness marker within 300ms",
    );
  }, 10_000);

  it("MINOR-1 control: the default readiness deadline is a hard, named bound", () => {
    expect(DEFAULT_READINESS_TIMEOUT_MS).toBe(15_000);
  });
});
