/**
 * T16 reconnect-race suites (plan Attack 1 + the kill-after-acceptance and
 * epoch-change legs; REQ-DATA-002/003, REQ-OPS-002, REQ-OUT-003, REQ-SYNC-001/002,
 * REQ-VERIFY-006; prereg P1/D1, transfer L1/L2).
 *
 * The gate selector `just tui-test reconnect-races` selects this file by name.
 * Proofs: disconnect after baseline but before several incremental events
 * commit, reconnect to a NEW generation, then deliver old-generation frames
 * OUT OF ORDER (with later-looking timestamps) — the rebuilt generation
 * converges to the authoritative fold and EVERY old-generation frame is typed-
 * rejected; deterministic death between canonical acceptance and receipt
 * resolves the typed outcome_unknown on the gateway side and the stream
 * reconciles the committed event exactly once (no loss, no duplication); a
 * source epoch change mid-stream forces a VISIBLE fresh-baseline resync while
 * the authoritative source is never mutated to make the projection agree.
 *
 * Determinism: all streams are in-process fakes over the injected
 * `AuthoritativeStreamPort` (prereg blindspot: real network transport is T20;
 * million-event scale is T21).
 */

import { describe, expect, it } from "bun:test";
import { FocusRef } from "./focus";
import {
  canonicalStreamOrder,
  InMemoryStreamPort,
  InMemoryStreamSource,
  LiveStreamClient,
  projectionDifferences,
  type StreamStateTransition,
} from "./live-streams";
import type { OperationInvocationRequest, OperatorActionRoute } from "./operation-gateway";
import { OperationGateway } from "./operation-gateway";
import { OPERATION_IDS } from "./operation-registry";
import { CanonicalSurfaceStub } from "./testing/canonical-surface-stub";
import { WorkbenchModel } from "./workbench-model";

const ACTOR = "operator-dave";
const TOKEN = "t16-reconnect-token";

function seedBaseline(gauntlet: InMemoryStreamSource, prime: InMemoryStreamSource): void {
  gauntlet.append({
    event_type: "unit_updated",
    payload: { unit_id: "AK-201", title: "Unit 201", phase: "execute", status: "blocked" },
    timestamp_ms: 1000,
  });
  gauntlet.append({
    event_type: "unit_updated",
    payload: { unit_id: "AK-202", title: "Unit 202", phase: "verify", status: "pending" },
    timestamp_ms: 1100,
  });
  gauntlet.append({
    event_type: "evidence_added",
    payload: {
      evidence_id: "ev-1",
      unit_id: "AK-201",
      kind: "test_report",
      locator: ".agents/reports/ev1.json",
      verified: true,
    },
    timestamp_ms: 1200,
  });
  prime.append({
    event_type: "session_updated",
    payload: { session_id: "prime-root", parent_id: null, model: "prime-main", status: "running" },
    timestamp_ms: 1300,
  });
}

function referenceModel(port: InMemoryStreamPort, generation: string): WorkbenchModel {
  const model = new WorkbenchModel();
  model.startGeneration(generation);
  const events = canonicalStreamOrder(
    port
      .describeSources()
      .flatMap((descriptor) => port.baselineWindow(descriptor.source_id).events),
  );
  for (const event of events) {
    const applied = model.applyEvent({
      generation_id: generation,
      source_id: event.source_id,
      cursor: event.cursor,
      event_type: event.event_type,
      payload: event.payload,
      timestamp_ms: event.timestamp_ms,
    });
    if (!applied.applied) {
      throw new Error(`reference fold refused an authoritative event: ${String(applied.reason)}`);
    }
  }
  model.markReady(generation);
  return model;
}

function makeClient(
  port: InMemoryStreamPort,
  prefix: string,
  maxBufferedFrames?: number,
): { client: LiveStreamClient; model: WorkbenchModel } {
  const model = new WorkbenchModel();
  const client = new LiveStreamClient({
    model,
    port,
    mintGenerationId: (serial) => `${prefix}-${serial}`,
    ...(maxBufferedFrames === undefined ? {} : { maxBufferedFrames }),
  });
  return { client, model };
}

describe("T16 reconnect races (plan Attack 1; REQ-DATA-002/003, REQ-SYNC-001/002)", () => {
  it("Attack 1: disconnect after baseline but before several incremental events commit; reconnect to a new generation; old-generation frames delivered OUT OF ORDER with later-looking timestamps are ALL rejected and the rebuilt generation converges to the authoritative fold", () => {
    const gauntlet = new InMemoryStreamSource("gauntlet");
    const prime = new InMemoryStreamSource("prime");
    seedBaseline(gauntlet, prime);
    const port = new InMemoryStreamPort([gauntlet, prime]);
    const { client, model } = makeClient(port, "gen-t16a");

    // Generation-minted connect: baseline through the watermark, per-source cursors.
    const handshake1 = client.connect();
    expect(handshake1.kind).toBe("connect");
    expect(handshake1.generation_id).toBe("gen-t16a-1");
    expect(client.state).toBe("live");
    expect(handshake1.applied_events).toBe(4);

    const sub1 = client.subscribeStream();
    expect(sub1.generation_id).toBe("gen-t16a-1");

    // Increments that DO commit before the disconnect.
    const g4 = gauntlet.append({
      event_type: "unit_updated",
      payload: { unit_id: "AK-201", status: "in_progress" },
      timestamp_ms: 2000,
    });
    const g5 = gauntlet.append({
      event_type: "unit_updated",
      payload: { unit_id: "AK-202", status: "in_progress" },
      timestamp_ms: 2100,
    });
    const p2 = prime.append({
      event_type: "session_updated",
      payload: { session_id: "prime-root", status: "steering" },
      timestamp_ms: 2200,
    });
    for (const event of [g4, g5, p2]) {
      const result = sub1.deliver(event);
      expect(result.class).toBe("applied");
    }

    // DISCONNECT: several incremental events commit at the sources but are
    // never delivered to the (now gone) generation.
    const g6 = gauntlet.append({
      event_type: "unit_updated",
      payload: { unit_id: "AK-201", status: "resolved" },
      timestamp_ms: 3000,
    });
    const g7 = gauntlet.append({
      event_type: "evidence_added",
      payload: {
        evidence_id: "ev-2",
        unit_id: "AK-201",
        kind: "proof",
        locator: ".agents/reports/ev2.json",
        verified: false,
      },
      timestamp_ms: 3100,
    });
    const p3 = prime.append({
      event_type: "session_updated",
      payload: { session_id: "prime-child-1", parent_id: "prime-root", status: "running" },
      timestamp_ms: 3200,
    });

    // RECONNECT: a NEW generation is minted; its baseline reconciles every
    // event since the last authoritative cursor.
    const handshake2 = client.reconnect();
    expect(handshake2.kind).toBe("reconnect");
    expect(handshake2.generation_id).toBe("gen-t16a-2");
    expect(client.generationId).toBe("gen-t16a-2");
    const gauntletLeg = handshake2.per_source.find((leg) => leg.source_id === "gauntlet");
    const primeLeg = handshake2.per_source.find((leg) => leg.source_id === "prime");
    expect(gauntletLeg?.cursor_validation).toBe("valid");
    expect(gauntletLeg?.strategy).toBe("baseline_with_cursor_validation");
    expect(gauntletLeg?.reconciled_since_cursor).toBe(2);
    expect(gauntletLeg?.reconfirmed_events).toBe(5);
    expect(primeLeg?.reconciled_since_cursor).toBe(1);

    // The rebuilt generation converges to the authoritative fold.
    const reference = referenceModel(port, "gen-ref-attack1");
    const diffs = projectionDifferences(model.getSnapshot(), reference.getSnapshot());
    expect(diffs).toEqual([]);
    const unit201 = model.getSnapshot().units.find((unit) => unit.unit_id === "AK-201");
    expect(unit201?.status).toBe("resolved");

    // Attack: deliver OLD-GENERATION frames OUT OF ORDER with LATER-looking
    // timestamps (each stale frame's timestamp postdates every applied event).
    const late = 99_999_999;
    const staleDeliveries = [p3, g7, g6, g4].map((event) => ({ ...event, timestamp_ms: late }));
    const results = staleDeliveries.map((event) => sub1.deliver(event));
    for (const result of results) {
      expect(result.class).toBe("rejected");
      if (result.class === "rejected") {
        expect(result.rejection.reason).toBe("superseded_generation");
        expect(result.rejection.live_generation).toBe("gen-t16a-2");
        expect(result.rejection.frame_timestamp_ms).toBe(late);
      }
    }
    expect(client.rejectionLog.length).toBe(4);

    // Rejection changed nothing: the live generation, its cursors, and the
    // converged projection are all untouched.
    expect(client.generationId).toBe("gen-t16a-2");
    expect(client.state).toBe("live");
    expect(projectionDifferences(model.getSnapshot(), reference.getSnapshot())).toEqual([]);
    expect(client.sourcePositions().find((p) => p.epoch === "epoch-1")).toBeDefined();

    model.close();
    reference.close();
  });

  it("deterministic kill after canonical acceptance, before receipt: the gateway resolves typed outcome_unknown, and the stream reconnect reconciles the committed event exactly once (no loss, no duplication)", async () => {
    const stub = new CanonicalSurfaceStub({
      run_id: "01JT16RECONNECTRUN1",
      bound_actor: ACTOR,
      session_token: TOKEN,
    }).addUnit({
      unit_id: "AK-201",
      status: "blocked",
      tier: "P3",
      settled: false,
      authority: "granted",
      authority_visible_in_reads: true,
      governance_denied: false,
      governance_visible_in_reads: true,
      open_ticket: "ESC-201",
    });
    const healthy = stub.asInProcessRoute();
    // The dying transport: canonical admission COMMITS, then the response is
    // lost (the host was killed between acceptance and receipt).
    const dyingRoute: OperatorActionRoute = {
      observe: (target) => healthy.observe(target),
      perform: async (command) => {
        const outcome = await healthy.perform(command);
        throw new Error(`transport lost after canonical admission (${String(outcome.kind)})`);
      },
    };
    const gateway = new OperationGateway(healthy);

    const gauntlet = new InMemoryStreamSource("gauntlet");
    const prime = new InMemoryStreamSource("prime");
    seedBaseline(gauntlet, prime);
    const port = new InMemoryStreamPort([gauntlet, prime]);
    const { client, model } = makeClient(port, "gen-t16k");
    client.connect();

    const request: OperationInvocationRequest = {
      operation: OPERATION_IDS.UNIT_RESOLVE,
      focus: FocusRef.for("gauntlet", "WorkItem", "AK-201"),
      session: { actor: ACTOR, token: TOKEN },
      // Anchored on the durable run: reconciliation re-drives the SAME key
      // through the §16.3 idempotent re-entry (committed_now: false).
      payload: { note: "resolve the escalation", run: "01JT16RECONNECTRUN1" },
    };

    const outcome = await gateway.invokeVia(dyingRoute, request);
    if (outcome.class !== "outcome_unknown") {
      throw new Error(`expected typed outcome_unknown, got '${outcome.class}'`);
    }
    expect(outcome.idempotency_key.length).toBeGreaterThan(0);
    expect(outcome.detail).toContain("lost");
    expect(gateway.isOutcomeUnknown(outcome.idempotency_key)).toBe(true);
    // The canonical commit happened EXACTLY once, even though no receipt arrived.
    expect(stub.sideEffects.settlements_committed).toBe(1);

    // The commit surfaces as a new authoritative event the disconnected
    // client has not seen.
    const commitEvent = gauntlet.append({
      event_type: "unit_updated",
      payload: { unit_id: "AK-201", status: "resolved" },
      timestamp_ms: 5000,
    });

    // Reconciliation re-drives the SAME key with the SAME payload through
    // canonical re-entry: accepted, committed_now false (idempotent — the
    // first drive's commit already settled the unit and resolved its ticket).
    const reconciled = await gateway.reconcile(outcome.idempotency_key, request);
    if (reconciled.class !== "accepted") {
      throw new Error(`expected idempotent re-entry acceptance, got '${reconciled.class}'`);
    }
    expect(reconciled.committed_now).toBe(false);
    expect(stub.sideEffects.settlements_committed).toBe(1);
    // The durable truth from the FIRST drive: the escalation is resolved.
    const durableTruth = await healthy.observe({ unit: "AK-201" });
    expect(durableTruth.open_escalations.length).toBe(0);

    // The stream reconnect folds the committed event EXACTLY ONCE: the trace
    // holds one entry for its cursor, and the projection converges.
    const handshake = client.reconnect();
    expect(handshake.kind).toBe("reconnect");
    const traceCursors = model
      .getTrace()
      .filter((record) => record.trace_id === `trace-gauntlet-${commitEvent.cursor}`);
    expect(traceCursors.length).toBe(1);
    const reference = referenceModel(port, "gen-ref-kill");
    expect(projectionDifferences(model.getSnapshot(), reference.getSnapshot())).toEqual([]);
    const unit201 = model.getSnapshot().units.find((unit) => unit.unit_id === "AK-201");
    expect(unit201?.status).toBe("resolved");
    const report = client.poll();
    expect(report.applied).toBe(0);

    model.close();
    reference.close();
  });

  it("source epoch change mid-stream: visible resync with a fresh baseline, superseded-generation frames rejected, and the authoritative source never mutated to make the projection agree", () => {
    const gauntlet = new InMemoryStreamSource("gauntlet");
    const prime = new InMemoryStreamSource("prime");
    seedBaseline(gauntlet, prime);
    const port = new InMemoryStreamPort([gauntlet, prime]);
    const { client, model } = makeClient(port, "gen-t16e");
    client.connect();
    const sub1 = client.subscribeStream();

    const observed: StreamStateTransition[] = [];
    client.onStateChange((transition) => {
      observed.push(transition);
      if (transition.to === "resyncing") {
        // Degraded reasons are visible AT the resync transition.
        expect(client.degradedReasons.length).toBeGreaterThan(0);
        expect(client.degradedReasons[0]).toContain("epoch_change");
      }
    });

    const delivered = gauntlet.append({
      event_type: "unit_updated",
      payload: { unit_id: "AK-201", status: "in_progress" },
      timestamp_ms: 2000,
    });
    expect(sub1.deliver(delivered).class).toBe("applied");

    // Retention replacement at the source: the epoch changes and the old
    // history is gone. The old-generation subscription frame is now stale.
    const oldEpochFrame = gauntlet.append({
      event_type: "unit_updated",
      payload: { unit_id: "AK-202", status: "in_progress" },
      timestamp_ms: 2100,
    });
    gauntlet.replaceStorage("epoch-2");
    const newEpochEvent = gauntlet.append({
      event_type: "unit_updated",
      payload: { unit_id: "AK-203", title: "Unit 203", status: "pending" },
      timestamp_ms: 2200,
    });

    // poll() detects the epoch change proactively and resyncs.
    const report = client.poll();
    expect(report.resync_triggered).toContain("epoch_change:gauntlet");
    expect(client.resyncCount).toBe(1);
    expect(client.generationId).toBe("gen-t16e-2");
    expect(client.state).toBe("live");
    expect(
      observed.some(
        (transition) => transition.to === "resyncing" && transition.reason.includes("epoch_change"),
      ),
    ).toBe(true);

    // The projection reflects ONLY the new epoch's authoritative state —
    // the resync re-baselined from the source, never the reverse.
    const unitIds = model.getSnapshot().units.map((unit) => unit.unit_id);
    expect(unitIds).toEqual(["AK-203"]);
    const reference = referenceModel(port, "gen-ref-epoch");
    expect(projectionDifferences(model.getSnapshot(), reference.getSnapshot())).toEqual([]);

    // The pre-resync subscription now carries a superseded generation: rejected.
    const stale = sub1.deliver({ ...oldEpochFrame, timestamp_ms: 99_999_999 });
    expect(stale.class).toBe("rejected");
    if (stale.class === "rejected") {
      expect(stale.rejection.reason).toBe("superseded_generation");
      expect(stale.rejection.live_generation).toBe("gen-t16e-2");
    }

    // An old-EPOCH frame on a CURRENT-generation subscription triggers the
    // epoch-change resync path (frame-level detection, still visible).
    const sub2 = client.subscribeStream();
    expect(sub2.generation_id).toBe("gen-t16e-2");
    const before = gauntlet.retainedEvents().map((event) => event.cursor);
    const result = sub2.deliver({ ...delivered, timestamp_ms: 88_888_888 });
    expect(result.class).toBe("resync_triggered");
    expect(client.resyncCount).toBe(2);
    expect(client.state).toBe("live");
    expect(client.generationId).toBe("gen-t16e-3");
    // REQ-SAFE-005: the source is untouched by the resync.
    expect(gauntlet.retainedEvents().map((event) => event.cursor)).toEqual(before);
    expect(gauntlet.watermark_sequence).toBe(1);
    void newEpochEvent;

    model.close();
    reference.close();
  });

  it("correction round 1 (verifier E9, MAJOR): a store-refused event IN THE BASELINE is counted, reasoned, and visibly degraded — never a silent advance", () => {
    const gauntlet = new InMemoryStreamSource("gauntlet");
    const prime = new InMemoryStreamSource("prime");
    seedBaseline(gauntlet, prime);
    // The BASELINE contains an event the projection store will refuse
    // (a focus_changed that attributes to no projected row).
    const refused = gauntlet.append({
      event_type: "focus_changed",
      payload: { kind: "unit", id: "no-such-unit" },
      timestamp_ms: 1400,
    });
    const port = new InMemoryStreamPort([gauntlet, prime]);
    const { client, model } = makeClient(port, "gen-t16r");
    const handshake = client.connect();
    const gauntletLeg = handshake.per_source.find((leg) => leg.source_id === "gauntlet");

    // The store received ONLY the admitted events (3 of the 4-event window).
    const gauntletTraces = model
      .getTrace()
      .filter((record) => record.trace_id.startsWith("trace-gauntlet-")).length;
    expect(gauntletTraces).toBe(3);

    // applied_events counts ONLY store-admitted frames; the refusal is counted
    // and carries the store's typed reason — no silent advance, no overcount.
    expect(gauntletLeg?.applied_events).toBe(3);
    expect(gauntletLeg?.refused_events).toBe(1);
    expect(gauntletLeg?.refusal_reasons.length).toBe(1);
    expect(gauntletLeg?.refusal_reasons[0]).toContain("unattributable_focus_change");

    // The condition is VISIBLE: the client reports the refusal and the
    // degraded reasons surface it while the projection carries the hole.
    expect(client.refusals.length).toBe(1);
    expect(client.refusals[0]?.source_id).toBe("gauntlet");
    expect(client.refusals[0]?.sequence).toBe(refused.sequence);
    expect(client.refusals[0]?.reason).toContain("unattributable_focus_change");
    expect(
      client.degradedReasons.some((reason) => reason.startsWith("baseline_refusal:gauntlet")),
    ).toBe(true);
    expect(client.degradedReasons[0]).toContain(`sequence=${refused.sequence}`);

    // The stream position marks the refused frame consumed-and-refused (so
    // gap detection stays live — a position frozen before the refusal would
    // livelock gap/resync against a frame the source will re-serve identically
    // forever); the refusal itself is the visible, counted record of the hole.
    expect(client.sourcePositions().find((p) => p.epoch === "epoch-1")?.sequence).toBe(4);

    // No projection row was invented for the refused frame.
    expect(model.getSnapshot().focus).toBeNull();

    // Liveness: the next live event applies normally (no permanent gap).
    const sub = client.subscribeStream();
    const live = gauntlet.append({
      event_type: "unit_updated",
      payload: { unit_id: "AK-201", status: "in_progress" },
      timestamp_ms: 1500,
    });
    expect(sub.deliver(live).class).toBe("applied");

    // Contrast: the same refusal on the LIVE path still triggers its visible
    // store_refusal resync; afterwards the persistent hole is reported by the
    // fresh baseline (once), not looped.
    const bogusLive = gauntlet.append({
      event_type: "focus_changed",
      payload: { kind: "unit", id: "still-not-there" },
      timestamp_ms: 1600,
    });
    const result = sub.deliver(bogusLive);
    expect(result.class).toBe("resync_triggered");
    expect(client.resyncCount).toBe(1);
    expect(client.state).toBe("live");
    // The resync's fresh baseline now re-encounters BOTH refused events (the
    // original baseline hole, still retained, and the bogus live frame, which
    // the source retained too): both reported, nothing looped.
    expect(client.refusals.length).toBe(2);
    expect(client.refusals[0]?.sequence).toBe(refused.sequence);
    expect(client.refusals[1]?.sequence).toBe(bogusLive.sequence);
    expect(
      client.degradedReasons.some((reason) => reason.startsWith("baseline_refusal:gauntlet")),
    ).toBe(true);

    model.close();
  });
});
