import { afterAll, describe, expect, it } from "bun:test";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import type { AffordanceObservation } from "./affordance";
import {
  authoritativeEvidenceFrame,
  type CanonicalEvidenceAdmissionDecision,
  EvidenceAdmissionGate,
  type EvidenceOffer,
  type KernelObjectProvenance,
} from "./evidence-promotion";
import { FocusRef } from "./focus";
import {
  canonicalJson,
  canonicalSections,
  foldLogToNormalized,
  normalizeLiveFold,
  projectionDigest,
  type RecordedKernelObject,
  type RecordedRun,
  recordRun,
  replayHeadless,
} from "./headless-replay";
// Presentation comparison surfaces for Attack 2. READ-ONLY for this task: they
// are imported ONLY by this test to prove answer-equivalence between the
// minimal consumer and the OpenTUI views. Neither the harness
// (headless-replay.ts) nor the consumer (testing/minimal-projection.ts) may
// import them — that ban is policed by renderer-boundary.test.ts.
import { renderInspector } from "./inspector";
import type { AuthoritativeEventFrame } from "./projection-store";
import {
  minimalAffordanceReasons,
  minimalEvidenceFor,
  minimalUnitStatus,
  renderMinimalProjection,
} from "./testing/minimal-projection";
import { materializeAffordances, workUnitOrder } from "./views-work";
import { WorkbenchModel } from "./workbench-model";

/**
 * T19: Headless equivalence and renderer replaceability (plan T19, P3,
 * REQ-VERIFY-002, REQ-STATE-004, REQ-GOAL-005, REQ-TUI-005, REQ-SAFE-001,
 * DSN-011).
 *
 * Attack 1 (prereg P1/falsifier 1): with every OpenTUI/React/presentation
 * module bypassed, the recorded Run reconstructs to the normalized projection
 * — canonical content equal to the with-presentation fold — and deletion
 * orderings (renderer first, kernel first, store first) leave Reference,
 * authority, evidence, settlement, and Run truth unchanged, with lost
 * ephemeral kernel output explicitly marked unavailable and never recreated.
 *
 * Attack 2 (prereg P2): the minimal non-OpenTUI consumer drives the same model
 * operations and derives the same answers the shell views derive from the
 * same snapshots — unit status, evidence presence, and affordance reasons.
 */

// --- The deterministic recorded-Run fixture -------------------------------------

const GEN = "gen-t19-recorded";
const SOURCE = "gauntlet";
const RECORDED_AT_MS = 1_789_000_000_000; // recorded clock constant; never re-minted

const METRICS_ARTIFACT = ".agents/reports/artifacts/t19-metrics.json";
const METRICS_EVIDENCE_ID = "kev:kernel-t19:kg-1:metrics:1";

/** One authoritative event at a fixed recorded timestamp. */
function recordedFrame(
  cursor: string,
  timestampOffsetMs: number,
  eventType: AuthoritativeEventFrame["event_type"],
  payload: Record<string, unknown>,
): AuthoritativeEventFrame {
  return {
    generation_id: GEN,
    source_id: SOURCE,
    cursor,
    event_type: eventType,
    payload,
    timestamp_ms: RECORDED_AT_MS + timestampOffsetMs,
  };
}

const KERNEL_OBJECTS: readonly RecordedKernelObject[] = [
  {
    object_id: "obj-1-metrics",
    name: "metrics",
    type_name: "dict",
    session_ref: "kernel-t19",
    kernel_generation: "kg-1",
    persistence_class: "artifactized",
    artifact_ref: METRICS_ARTIFACT,
  },
  {
    object_id: "obj-2-intermediate",
    name: "intermediate",
    type_name: "number",
    session_ref: "kernel-t19",
    kernel_generation: "kg-1",
    persistence_class: "pinned",
    artifact_ref: null,
  },
  {
    object_id: "obj-3-scratch",
    name: "scratch",
    type_name: "list",
    session_ref: "kernel-t19",
    kernel_generation: "kg-1",
    persistence_class: "ephemeral",
    artifact_ref: null,
  },
];

/**
 * The recorded authoritative log: a realistic Run sequence whose evidence
 * admission goes through the REAL canonical path (offer -> canonical decision
 * -> admission gate -> authoritative frame) — the only writer path for
 * evidence rows (T11, REQ-SYNC-003).
 */
function buildEventLog(): AuthoritativeEventFrame[] {
  const metricsProvenance: KernelObjectProvenance = {
    object_id: "obj-1-metrics",
    name: "metrics",
    session_ref: "kernel-t19",
    kernel_generation: "kg-1",
    type_name: "dict",
    created_at_execution_order: 3,
  };
  const offer: EvidenceOffer = {
    offer_id: `offer:${METRICS_EVIDENCE_ID}`,
    provenance: metricsProvenance,
    target: { unit_id: "u-core", source_id: SOURCE },
    admission_contract: {
      evidence_id: METRICS_EVIDENCE_ID,
      unit_id: "u-core",
      kind: "report",
      locator: METRICS_ARTIFACT,
      source_id: SOURCE,
    },
    establishes_evidence: false,
    establishes_verification: false,
    establishes_settlement: false,
    admission_route: "canonical_gauntlet_evidence_apis",
    verified_source: "canonical_verification_only",
  };
  // The canonical side's OWN answer — an input, never derivable from kernel state.
  const decision: CanonicalEvidenceAdmissionDecision = {
    class: "admitted",
    evidence_id: METRICS_EVIDENCE_ID,
    verified: true,
    source_cursor: "c-07",
  };
  const gate = new EvidenceAdmissionGate();
  const outcome = gate.admit(offer, decision);
  if (outcome.class !== "admitted") {
    throw new Error(`fixture admission failed: ${JSON.stringify(outcome)}`);
  }
  const admittedFrame = authoritativeEvidenceFrame(offer, decision, {
    generation_id: GEN,
    timestamp_ms: RECORDED_AT_MS + 700,
  });

  return [
    recordedFrame("c-01", 100, "unit_updated", {
      unit_id: "u-core",
      title: "Core Unit",
      status: "running",
      authority: "builder",
      blast_radius: "isolated",
    }),
    recordedFrame("c-02", 200, "unit_updated", {
      unit_id: "u-edge",
      title: "Edge Unit",
      status: "running",
      authority: "builder",
      blast_radius: "isolated",
    }),
    recordedFrame("c-03", 300, "unit_updated", {
      unit_id: "u-far",
      title: "Far Unit",
      status: "blocked",
      authority: "orchestrator",
      blast_radius: "workspace",
    }),
    recordedFrame("c-04", 400, "evidence_added", {
      evidence_id: "ev-src-1",
      unit_id: "u-core",
      kind: "log",
      locator: ".agents/reports/artifacts/t19-src-log.txt",
      verified: false,
    }),
    recordedFrame("c-05", 500, "session_updated", {
      session_id: "s-prime",
      parent_id: null,
      model: "prime-model-a",
      status: "running",
    }),
    recordedFrame("c-06", 600, "session_updated", {
      session_id: "s-child",
      parent_id: "s-prime",
      model: "prime-model-a",
      status: "running",
    }),
    admittedFrame,
    recordedFrame("c-08", 800, "focus_changed", { kind: "unit", id: "u-core" }),
    recordedFrame("c-09", 900, "unit_updated", {
      unit_id: "u-core",
      title: "Core Unit",
      status: "fail",
      authority: "builder",
      blast_radius: "isolated",
    }),
  ];
}

/** Record the representative Run (events + durable kernel facts + ephemera). */
function buildRecordedRun(): RecordedRun {
  return recordRun({
    run_id: "run-t19-001",
    generation_id: GEN,
    recorded_at_ms: RECORDED_AT_MS,
    events: buildEventLog(),
    kernel: {
      kernel_generation: "kg-1",
      session_ref: "kernel-t19",
      objects: KERNEL_OBJECTS,
    },
    presentation: {
      placements: [
        {
          id: "placement:gen-t19-recorded:work:unit-row:focus:gauntlet:WorkItem:u-core",
          surface: "work",
          view: "unit-row",
          label_override: "CORE (operator label)",
        },
      ],
      selected_focus_id: "focus:gauntlet:WorkItem:u-core",
    },
  });
}

/** The LIVE fold: a real WorkbenchModel with presentation state attached. */
function buildLiveModel(recorded: RecordedRun): WorkbenchModel {
  const model = new WorkbenchModel();
  model.startGeneration(recorded.generation_id);
  for (const event of recorded.events) {
    const result = model.applyEvent(event);
    if (!result.applied) {
      throw new Error(`fixture event not applied: ${result.reason ?? "unknown"}`);
    }
  }
  // Presentation flourishes the headless path will never have:
  const selected = FocusRef.for(SOURCE, "WorkItem", "u-core");
  model.placeFocus("work", "unit-row", selected, { label_override: "CORE (operator label)" });
  model.selectFocus(selected);
  model.hoistToFocus(selected);
  // Environment capability inventory (observed; not Gauntlet canonical truth).
  model.declareCapabilities([
    {
      capability: "kernel.python",
      installed: true,
      gates: {
        visibility: true,
        reachability: true,
        payment: true,
        governance: true,
        settlement_access: true,
        recovery: false,
      },
    },
  ]);
  model.markReady(recorded.generation_id);
  return model;
}

function liveProjection(model: WorkbenchModel, recorded: RecordedRun) {
  return normalizeLiveFold({
    run_id: recorded.run_id,
    generation_id: recorded.generation_id,
    storeSnapshot: model.store.getSnapshot(null),
    trace: model.getTrace(),
    kernelObjects: recorded.kernel.objects,
    kernelAlive: true,
  });
}

/** A reachable, healthy canonical observation over the live model's reads. */
function observationFor(model: WorkbenchModel): AffordanceObservation {
  const snapshot = model.getSnapshot();
  return {
    reachable: true,
    degraded: false,
    units: snapshot.units.map((unit) => ({ unit_id: unit.unit_id, status: unit.status })),
    open_escalations: [{ ticket_id: "ESC-19-1", unit_id: "u-far" }],
    budget_remaining: { usd_micro: 5_000_000, tokens: 1_000_000 },
    governance_denied_units: [],
    authority_revoked_units: [],
    capabilities: [],
  };
}

const AFFORDANCE_OPTIONS = {
  session: { actor: "operator", authenticated: true },
  payment_estimates: { "unit.retry": { tokens: 100, cost_usd_micro: 1_000 } },
} as const;

const RECORDED = buildRecordedRun();

// --- Attack 1: headless equivalence under presentation/kernel/store loss -------

describe("T19 Attack 1: the recorded Run reconstructs headlessly (REQ-VERIFY-002, REQ-STATE-004)", () => {
  it("with presentation attached, the live fold carries renderer state the projection ignores", () => {
    const model = buildLiveModel(RECORDED);
    const snapshot = model.getSnapshot();
    expect(snapshot.placements.length).toBeGreaterThan(0);
    expect(snapshot.placements[0]?.label_override).toBe("CORE (operator label)");
    expect(snapshot.horizon).not.toBeNull();
    expect(snapshot.capability_reasons.length).toBe(1);
    model.close();
  });

  it("headless replay equals the with-presentation fold in canonical content", () => {
    const model = buildLiveModel(RECORDED);
    const live = liveProjection(model, RECORDED);
    const replay = replayHeadless(RECORDED);

    // Canonical truth: Reference cursors, authority-bearing units, evidence,
    // settlement inputs, attention, and run/audit truth.
    expect(canonicalJson(canonicalSections(live))).toBe(
      canonicalJson(canonicalSections(replay.via_log)),
    );
    // The two headless fold paths (pure log fold vs T06 cold rebuild) agree fully.
    expect(canonicalJson(replay.via_log)).toBe(canonicalJson(replay.via_store));
    expect(replay.equal).toBe(true);
    expect(replay.canonical_equal).toBe(true);
    // Only presentation is absent: kernel facts recorded as durable survive
    // (kernel alive in this comparison is only relevant to Attack 2's ordering).
    model.close();
    replay.store.close();
  });

  it("reconstruction is deterministic: the same recording yields the same digest", () => {
    const first = replayHeadless(RECORDED);
    const second = replayHeadless(RECORDED);
    expect(first.digest).toBe(second.digest);
    expect(projectionDigest(first.via_log)).toBe(first.digest);
    first.store.close();
    second.store.close();
  });

  it("the harness itself imports no presentation module (prereg falsifier 1)", () => {
    for (const path of ["headless-replay.ts", join("testing", "minimal-projection.ts")]) {
      const source = readFileSync(join(import.meta.dir, path), "utf8");
      const specs = [...source.matchAll(/from\s*["']([^"']+)["']/g)].map((m) => m[1] ?? "");
      const banned = specs.filter((spec) =>
        /theme|geometry|render-tree|shell|views-|inspector|composer|@opentui|^react$/.test(spec),
      );
      expect({ path, banned }).toEqual({ path, banned: [] });
    }
  });

  it("deletion order (a) renderer-state first: selection, placements, and hoists never touch truth", () => {
    const model = buildLiveModel(RECORDED);
    const before = canonicalJson(canonicalSections(liveProjection(model, RECORDED)));

    // Destroy every piece of renderer state: placements, hoist stack, and the
    // operator's visual selection (moved to a DIFFERENT unit with no
    // authoritative event behind it).
    for (const placement of model.getSnapshot().placements) {
      model.removePlacement(placement.id);
    }
    model.dehoist();
    model.selectFocus(FocusRef.for(SOURCE, "WorkItem", "u-edge"));

    // Renderer state really did change...
    const snapshot = model.getSnapshot();
    expect(snapshot.placements).toEqual([]);
    expect(snapshot.horizon).toBeNull();
    expect(snapshot.focus?.source_id).toBe("u-edge");

    // ...and canonical truth did not.
    const after = canonicalJson(canonicalSections(liveProjection(model, RECORDED)));
    expect(after).toBe(before);

    const replay = replayHeadless(RECORDED);
    expect(after).toBe(canonicalJson(canonicalSections(replay.via_log)));
    model.close();
    replay.store.close();
  });

  it("deletion order (b) kernel first: unretained values are marked unavailable, never recreated", () => {
    const alive = foldLogToNormalized(RECORDED, { kernelAlive: true });
    const dead = replayHeadless(RECORDED).via_log; // headless default: kernel not assumed alive

    // Canonical truth identical across kernel death; only the kernel-availability
    // section of the projection legitimately moves.
    const aliveFull = canonicalJson(alive);
    expect(aliveFull).not.toBe(canonicalJson(dead));
    expect(projectionDigest(canonicalSections(alive))).toBe(
      projectionDigest(canonicalSections(dead)),
    );

    // Kernel availability differs, and ONLY in the typed direction:
    const byId = new Map(dead.kernel_objects.map((record) => [record.object_id, record]));
    expect(dead.kernel_objects.length).toBe(alive.kernel_objects.length);
    for (const liveRecord of alive.kernel_objects) {
      const deadRecord = byId.get(liveRecord.object_id);
      expect(deadRecord).toBeDefined();
      if (deadRecord === undefined) continue;
      if (liveRecord.persistence_class === "artifactized") {
        expect(deadRecord.value_available).toBe(true);
        expect(deadRecord.artifact_ref).toBe(METRICS_ARTIFACT);
        expect(deadRecord.unavailability_reason).toBeNull();
      } else if (liveRecord.persistence_class === "pinned") {
        expect(liveRecord.value_available).toBe(true);
        expect(deadRecord.value_available).toBe(false);
        expect(deadRecord.artifact_ref).toBeNull();
        expect(deadRecord.unavailability_reason).toBe("kernel_restarted_pinned_value_not_retained");
      } else {
        expect(liveRecord.value_available).toBe(true);
        expect(deadRecord.value_available).toBe(false);
        expect(deadRecord.artifact_ref).toBeNull();
        expect(deadRecord.unavailability_reason).toBe("ephemeral_kernel_state_not_reconstructible");
      }
    }
    // Nothing unavailable claims a value or a locator.
    for (const record of dead.kernel_objects) {
      if (!record.value_available) {
        expect(record.artifact_ref).toBeNull();
        expect(record.unavailability_reason).not.toBeNull();
      }
    }
  });

  it("deletion order (c) store first, then rebuild: deleting the SQLite file loses nothing", () => {
    const dir = mkdtempSync(join(tmpdir(), "t19-store-delete-"));
    const dbPath = join(dir, "projection.db");
    try {
      const first = replayHeadless(RECORDED, { dbPath });
      first.store.close();
      // Delete ALL local SQLite projection state (database plus WAL/SHM).
      for (const suffix of ["", "-wal", "-shm"]) {
        rmSync(`${dbPath}${suffix}`, { force: true });
      }
      // Rebuild at the same path from the log alone.
      const rebuilt = replayHeadless(RECORDED, { dbPath });
      expect(canonicalJson(rebuilt.via_store)).toBe(canonicalJson(first.via_store));
      expect(rebuilt.digest).toBe(first.digest);
      rebuilt.store.close();
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });

  it("every deletion ordering leaves canonical truth identical (prereg transfer L2)", () => {
    const rendererFirst = replayHeadless(RECORDED);
    const kernelFirst = replayHeadless(RECORDED, { kernelAlive: true });
    const logOnly = foldLogToNormalized(RECORDED); // store AND kernel AND renderer gone
    const base = canonicalJson(canonicalSections(rendererFirst.via_log));
    expect(canonicalJson(canonicalSections(kernelFirst.via_log))).toBe(base);
    expect(canonicalJson(canonicalSections(logOnly))).toBe(base);
    rendererFirst.store.close();
    kernelFirst.store.close();
  });

  it("teeth: the digest discriminates any consequential tampering with the recording", () => {
    const base = projectionDigest(canonicalSections(replayHeadless(RECORDED).via_log));

    // Tamper 1a: rewrite an authority fact in a frame the last-write-wins fold
    // SUPERSEDES (u-core is re-stated at c-09) — the final truth legitimately
    // does not move, exactly like the store's INSERT OR REPLACE semantics.
    const supersededTampered = recordRun({
      ...RECORDED,
      events: RECORDED.events.map((event) =>
        event.cursor === "c-01"
          ? { ...event, payload: { ...event.payload, authority: "orchestrator" } }
          : event,
      ),
    });
    expect(projectionDigest(canonicalSections(replayHeadless(supersededTampered).via_log))).toBe(
      base,
    );

    // Tamper 1b: rewrite an authority fact in the frame that WINS (u-edge's
    // only update, c-02) — the digest must move and the change must show.
    const authorityTampered = recordRun({
      ...RECORDED,
      events: RECORDED.events.map((event) =>
        event.cursor === "c-02"
          ? { ...event, payload: { ...event.payload, authority: "orchestrator" } }
          : event,
      ),
    });
    const authorityDigest = projectionDigest(
      canonicalSections(replayHeadless(authorityTampered).via_log),
    );
    expect(authorityDigest).not.toBe(base);
    const tamperedUnit = replayHeadless(authorityTampered).via_log.units.find(
      (unit) => unit.unit_id === "u-edge",
    );
    expect(tamperedUnit?.authority).toBe("orchestrator");

    // Tamper 2: flip a canonical verified bit (settlement input).
    const verifiedTampered = recordRun({
      ...RECORDED,
      events: RECORDED.events.map((event) =>
        event.event_type === "evidence_added" && event.cursor === "c-07"
          ? { ...event, payload: { ...event.payload, verified: false } }
          : event,
      ),
    });
    const verifiedReplay = replayHeadless(verifiedTampered);
    expect(projectionDigest(canonicalSections(verifiedReplay.via_log))).not.toBe(base);
    const tamperedEvidence = verifiedReplay.via_log.evidence.find(
      (record) => record.evidence_id === METRICS_EVIDENCE_ID,
    );
    expect(tamperedEvidence?.verified).toBe(false);
    const settlement = verifiedReplay.via_log.settlement_inputs.find(
      (input) => input.unit_id === "u-core",
    );
    expect(settlement?.verified_evidence_ids).not.toContain(METRICS_EVIDENCE_ID);
    expect(settlement?.unverified_evidence_ids).toContain(METRICS_EVIDENCE_ID);

    // Tampering with PRESENTATION-only fields, by contrast, moves nothing.
    const presentationTampered = recordRun({
      ...RECORDED,
      presentation: {
        placements: [
          {
            id: "placement:elsewhere",
            surface: "work",
            view: "unit-row",
            label_override: "REWRITTEN",
          },
        ],
        selected_focus_id: "focus:gauntlet:WorkItem:u-edge",
      },
    });
    expect(projectionDigest(canonicalSections(replayHeadless(presentationTampered).via_log))).toBe(
      base,
    );
  });
});

// --- Attack 2: the minimal consumer derives the views' answers ------------------

describe("T19 Attack 2: minimal non-OpenTUI consumer matches the shell views (DSN-011, REQ-TUI-005)", () => {
  const model = buildLiveModel(RECORDED);
  const snapshot = model.getSnapshot(); // ONE snapshot consumed by both renderers
  const observation = observationFor(model);

  it("Q1 unit status: consumer answer equals the inspector STATE section and the snapshot row", () => {
    for (const unit of snapshot.units) {
      const consumer = minimalUnitStatus(snapshot, unit.unit_id);
      expect(consumer).not.toBeNull();
      if (consumer === null) continue;
      // Independent expected answer: read the snapshot row directly.
      expect(consumer.status).toBe(unit.status);
      expect(consumer.authority).toBe(unit.authority);

      // What the inspector view derives for the same unit:
      const inspector = renderInspector({
        snapshot,
        focus: FocusRef.for(SOURCE, "WorkItem", unit.unit_id),
        observation,
      });
      const stateSection = inspector.sections.find((section) => section.id === "state");
      expect(stateSection).toBeDefined();
      const fields = new Map<string, string>();
      for (const line of stateSection?.lines ?? []) {
        const key = line.spans[0]?.text.trim();
        const value = line.spans[1]?.text;
        if (key !== undefined && value !== undefined) fields.set(key, value);
      }
      expect(fields.get("status")).toBe(consumer.status);
      expect(fields.get("authority")).toBe(consumer.authority);
    }
    // Unknown unit: honest absence.
    expect(minimalUnitStatus(snapshot, "u-missing")).toBeNull();
  });

  it("Q1b unit ordering: the consumer's statuses classify units exactly as the Work view orders them", () => {
    const ordered = workUnitOrder(snapshot.units);
    const classOf = (status: string): number =>
      status === "blocked" ? 0 : status === "running" || status === "active" ? 1 : 2;
    const orderedClasses = ordered.map((unit) =>
      classOf(minimalUnitStatus(snapshot, unit.unit_id)?.status ?? "missing"),
    );
    expect(orderedClasses).toEqual([...orderedClasses].sort((a, b) => a - b));
    // The blocked unit surfaces first, exactly as the view's escalation-first order.
    expect(orderedClasses[0]).toBe(0);
  });

  it("Q2 evidence presence: consumer equals the inspector EVIDENCE records", () => {
    const consumer = minimalEvidenceFor(snapshot, "u-core");
    expect(consumer.length).toBeGreaterThan(0);

    const inspector = renderInspector({
      snapshot,
      focus: FocusRef.for(SOURCE, "WorkItem", "u-core"),
      observation,
    });
    const evidenceSection = inspector.sections.find((section) => section.id === "evidence");
    const viewRecords = evidenceSection?.records ?? [];
    expect(viewRecords.length).toBe(consumer.length);

    const byId = new Map(viewRecords.map((record) => [record.id, record]));
    for (const record of consumer) {
      const viewRecord = byId.get(`evidence:${record.evidence_id}`);
      expect(viewRecord).toBeDefined();
      expect(viewRecord?.fields["identity"]).toBe(record.evidence_id);
      expect(viewRecord?.fields["result"]).toBe(record.verified ? "VERIFIED" : "UNVERIFIED");
      expect(viewRecord?.fields["reference"]).toBe(record.locator || "unavailable");
    }
    // The admitted kernel-derived evidence is present with its durable locator;
    // the unverified source log is present and honestly UNVERIFIED.
    expect(consumer.some((r) => r.evidence_id === METRICS_EVIDENCE_ID && r.verified)).toBe(true);
    expect(consumer.some((r) => r.evidence_id === "ev-src-1" && !r.verified)).toBe(true);
    // A unit with no evidence: both agree it is empty (view renders "! none").
    expect(minimalEvidenceFor(snapshot, "u-edge")).toEqual([]);
  });

  it("Q3 affordance reasons: consumer equals the inspector AFFORDANCES records", () => {
    const focus = FocusRef.for(SOURCE, "WorkItem", "u-core");
    const consumer = minimalAffordanceReasons(focus, observation, AFFORDANCE_OPTIONS);

    const inspector = renderInspector({
      snapshot,
      focus,
      observation,
      session: AFFORDANCE_OPTIONS.session,
      paymentEstimates: AFFORDANCE_OPTIONS.payment_estimates,
    });
    const affordanceSection = inspector.sections.find((section) => section.id === "affordances");
    const viewRecords = affordanceSection?.records ?? [];
    expect(viewRecords.length).toBe(consumer.length);

    const byOperation = new Map(viewRecords.map((record) => [record.fields["operation"], record]));
    for (const reason of consumer) {
      const viewRecord = byOperation.get(reason.operation);
      expect(viewRecord).toBeDefined();
      expect(viewRecord?.fields["status"]).toBe(reason.status);
      expect(viewRecord?.fields["reasons"]).toBe(reason.reason_codes.join(",") || "none");
      if (reason.required_attention_item !== undefined) {
        expect(viewRecord?.fields["required_attention_item"]).toBe(reason.required_attention_item);
      }
    }

    // The known canonical answers for this fixture state:
    const byName = new Map(consumer.map((reason) => [reason.operation, reason]));
    // retry: u-core is failed and everything else passes -> available.
    expect(byName.get("unit.retry")?.status).toBe("available");
    // resolve: no open escalation on u-core -> the disclosed strict refusal.
    expect(byName.get("unit.resolve")?.reason_codes).toContain("no_open_ticket");
    // refresh: read-shaped -> never an action affordance.
    expect(byName.get("state.refresh")?.reason_codes).toEqual(["not_action_admitted"]);
  });

  it("Q3b attention-scoped resolve: both surfaces bind the open escalation ticket", () => {
    const focus = FocusRef.for(SOURCE, "WorkItem", "u-far");
    const consumer = minimalAffordanceReasons(focus, observation, AFFORDANCE_OPTIONS);
    const resolve = consumer.find((reason) => reason.operation === "unit.resolve");
    expect(resolve?.status).toBe("requires_attention");
    expect(resolve?.required_attention_item).toBe("ESC-19-1");

    const inspector = renderInspector({
      snapshot,
      focus,
      observation,
      session: AFFORDANCE_OPTIONS.session,
      paymentEstimates: AFFORDANCE_OPTIONS.payment_estimates,
    });
    const records = inspector.sections.find((section) => section.id === "affordances")?.records;
    const viewResolve = records?.find((record) => record.fields["operation"] === "unit.resolve");
    expect(viewResolve?.fields["status"]).toBe("requires_attention");
    expect(viewResolve?.fields["required_attention_item"]).toBe("ESC-19-1");
  });

  it("the full minimal projection and the Work view agree on the same snapshot's records", () => {
    const minimal = renderMinimalProjection(snapshot, observation, AFFORDANCE_OPTIONS);
    const materialized = materializeAffordances(
      snapshot.focus ?? FocusRef.for(SOURCE, "WorkItem", "u-core"),
      observation,
      AFFORDANCE_OPTIONS,
    );

    const minimalAffordanceRecords = minimal.records.filter(
      (record) => record.kind === "affordance",
    );
    // The Work view hides palette-only affordances from its action bar; the
    // same predicate (the canonical palette-only reasons) selects the minimal
    // action-bar records, and both must agree on the rest.
    const PALETTE_ONLY = ["target_not_visible", "unsupported_operation", "not_action_admitted"];
    const inActionBar = (reasons: string): boolean =>
      !reasons.split(",").some((reason) => PALETTE_ONLY.includes(reason));
    const viewAction = materialized.filter((item) => item.outcome !== "palette_only");
    const minimalAction = minimalAffordanceRecords.filter((record) =>
      inActionBar(record.fields["reasons"] ?? ""),
    );
    expect(minimalAction.map((record) => record.fields["operation"])).toEqual(
      viewAction.map((item) => item.descriptor.operation),
    );
    for (const record of minimalAction) {
      const item = materialized.find((i) => i.descriptor.operation === record.fields["operation"]);
      expect(item).toBeDefined();
      expect(record.fields["status"]).toBe(item?.descriptor.status);
      expect(record.fields["reasons"]).toBe(item?.descriptor.reason_codes.join(",") || "none");
    }
    // Palette-only ones (state.refresh) are absent from the action bar on both
    // sides, yet stay discoverable WITH their reason in the minimal projection.
    const refresh = minimalAffordanceRecords.find(
      (record) => record.fields["operation"] === "state.refresh",
    );
    expect(refresh?.fields["reasons"]).toBe("not_action_admitted");
    expect(viewAction.some((item) => item.descriptor.operation === "state.refresh")).toBe(false);

    // Every unit appears in the minimal text exactly with its snapshot status.
    for (const unit of snapshot.units) {
      expect(
        minimal.lines.some(
          (line) => line.startsWith(`${unit.unit_id}:`) && line.includes(`status=${unit.status}`),
        ),
      ).toBe(true);
    }
    expect(minimal.lines.some((line) => line.includes("ATTENTION urgent"))).toBe(true);
  });

  it("environment capability inventory never enters the normalized truth", () => {
    const replay = replayHeadless(RECORDED);
    const json = canonicalJson(replay.via_log);
    expect(json).not.toContain("kernel.python");
    expect(json).not.toContain("capability");
    // And presentation capture in the recording is similarly absent from truth.
    expect(json).not.toContain("CORE (operator label)");
    expect(json).not.toContain("placement:");
    expect(replay.via_log.presentation_present).toBe(false);
    replay.store.close();
  });

  afterAll(() => {
    model.close();
  });
});
