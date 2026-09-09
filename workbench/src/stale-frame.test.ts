/**
 * T16 stale-frame suites (REQ-DATA-003, REQ-SAFE-005, REQ-SYNC-001/002,
 * REQ-VERIFY-006; prereg A2/D3, transfer L2).
 *
 * The gate selector `just tui-test stale-frame` selects this file by name.
 * Proofs: delayed frames from a superseded generation carrying LATER-looking
 * timestamps are typed-rejected against the live generation and mutate nothing;
 * cursor gaps are bounded-buffered with a VISIBLE degraded state and never a
 * silent cursor advance — the gap either fills (ordered drain in canonical
 * sequence order, never timestamp order) or becomes an EXPLICIT resync
 * (overload overflow, retention expiry); two concurrent clients hold isolated
 * generations and projections, and stale frames cannot cross-contaminate.
 *
 * Determinism: all streams are in-process fakes over the injected
 * `AuthoritativeStreamPort` (prereg blindspot: real network transport is T20).
 */

import { describe, expect, it } from "bun:test";
import {
  canonicalStreamOrder,
  InMemoryStreamPort,
  InMemoryStreamSource,
  LiveStreamClient,
  mintOpaqueCursor,
  projectionDifferences,
} from "./live-streams";
import { WorkbenchModel } from "./workbench-model";

function seed(gauntlet: InMemoryStreamSource, prime: InMemoryStreamSource): void {
  gauntlet.append({
    event_type: "unit_updated",
    payload: { unit_id: "AK-301", title: "Unit 301", status: "blocked" },
    timestamp_ms: 1000,
  });
  gauntlet.append({
    event_type: "unit_updated",
    payload: { unit_id: "AK-302", title: "Unit 302", status: "pending" },
    timestamp_ms: 1100,
  });
  gauntlet.append({
    event_type: "evidence_added",
    payload: {
      evidence_id: "ev-301",
      unit_id: "AK-301",
      kind: "test_report",
      locator: ".agents/reports/ev301.json",
      verified: true,
    },
    timestamp_ms: 1200,
  });
  prime.append({
    event_type: "session_updated",
    payload: { session_id: "prime-s-1", parent_id: null, status: "running" },
    timestamp_ms: 1300,
  });
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

function referenceModel(port: InMemoryStreamPort, generation: string): WorkbenchModel {
  const model = new WorkbenchModel();
  model.startGeneration(generation);
  for (const event of canonicalStreamOrder(
    port
      .describeSources()
      .flatMap((descriptor) => port.baselineWindow(descriptor.source_id).events),
  )) {
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

describe("T16 stale frames (REQ-DATA-003, REQ-SAFE-005, REQ-SYNC-001/002)", () => {
  it("delayed G1 frames with later timestamps against live G2: every one is typed-rejected and the live generation, its cursors, and its projection are untouched", () => {
    const gauntlet = new InMemoryStreamSource("gauntlet");
    const prime = new InMemoryStreamSource("prime");
    seed(gauntlet, prime);
    const port = new InMemoryStreamPort([gauntlet, prime]);
    const { client, model } = makeClient(port, "gen-sf");

    // Generation 1: live, applies one increment.
    client.connect();
    const sub1 = client.subscribeStream();
    const g4 = gauntlet.append({
      event_type: "unit_updated",
      payload: { unit_id: "AK-301", status: "in_progress" },
      timestamp_ms: 2000,
    });
    expect(sub1.deliver(g4).class).toBe("applied");

    // Reconnect: generation 2 goes live and applies its own increments.
    client.reconnect();
    expect(client.generationId).toBe("gen-sf-2");
    const sub2 = client.subscribeStream();
    const g5 = gauntlet.append({
      event_type: "unit_updated",
      payload: { unit_id: "AK-302", status: "in_progress" },
      timestamp_ms: 2100,
    });
    const p2 = prime.append({
      event_type: "session_updated",
      payload: { session_id: "prime-s-1", status: "steering" },
      timestamp_ms: 2200,
    });
    expect(sub2.deliver(g5).class).toBe("applied");
    expect(sub2.deliver(p2).class).toBe("applied");
    const liveSnapshotBefore = model.getSnapshot();

    // Delayed G1 frames arrive LATE, OUT OF ORDER, carrying timestamps that
    // POSTDATE every event the live generation has applied — including g6,
    // a sequence the live generation has never seen (it would have been
    // applied had its generation been current; the generation check
    // precedes everything).
    const g6 = gauntlet.append({
      event_type: "unit_updated",
      payload: { unit_id: "AK-301", status: "resolved" },
      timestamp_ms: 2300,
    });
    const late = 50_000_000;
    for (const event of [g6, g4, g5]) {
      const result = sub1.deliver({ ...event, timestamp_ms: late });
      expect(result.class).toBe("rejected");
      if (result.class === "rejected") {
        expect(result.rejection.reason).toBe("superseded_generation");
        expect(result.rejection.frame_generation).toBe("gen-sf-1");
        expect(result.rejection.live_generation).toBe("gen-sf-2");
        expect(result.rejection.frame_timestamp_ms).toBe(late);
        expect(result.rejection.frame_timestamp_ms).toBeGreaterThan(2300);
      }
    }
    expect(client.rejectionLog.length).toBe(3);

    // Nothing moved: same generation, same cursors, same projection rows.
    expect(client.generationId).toBe("gen-sf-2");
    expect(client.state).toBe("live");
    expect(projectionDifferences(model.getSnapshot(), liveSnapshotBefore)).toEqual([]);
    expect(client.sourcePositions().find((p) => p.epoch === "epoch-1")).toBeDefined();
    // g6 was NOT applied or buffered by the stale delivery.
    expect(model.getSnapshot().units.find((unit) => unit.unit_id === "AK-301")?.status).toBe(
      "in_progress",
    );
    expect(client.bufferedCount()).toBe(0);

    model.close();
  });

  it("cursor-gap detection: bounded backlog with visible degraded state, ordered drain in canonical SEQUENCE order (timestamps inverted), never silent divergence", () => {
    const gauntlet = new InMemoryStreamSource("gauntlet");
    const prime = new InMemoryStreamSource("prime");
    seed(gauntlet, prime);
    const port = new InMemoryStreamPort([gauntlet, prime]);
    const { client, model } = makeClient(port, "gen-gap");
    client.connect();
    const sub = client.subscribeStream();

    const g4 = gauntlet.append({
      event_type: "unit_updated",
      payload: { unit_id: "AK-301", status: "in_progress" },
      timestamp_ms: 2000,
    });
    expect(sub.deliver(g4).class).toBe("applied");
    const gauntletPosition = client.sourcePositions().find((p) => p.epoch === "epoch-1");
    expect(gauntletPosition?.sequence).toBe(4);

    // Gap: sequence 5 is withheld while 6 and 7 arrive first. Timestamps are
    // deliberately inverted against sequence order.
    const g5 = gauntlet.append({
      event_type: "unit_updated",
      payload: { unit_id: "AK-302", status: "verifying" },
      timestamp_ms: 2400,
    });
    const g6 = gauntlet.append({
      event_type: "unit_updated",
      payload: { unit_id: "AK-301", status: "verifying" },
      timestamp_ms: 9000,
    });
    const g7 = gauntlet.append({
      event_type: "unit_updated",
      payload: { unit_id: "AK-301", status: "resolved" },
      timestamp_ms: 100,
    });
    const g8 = gauntlet.append({
      event_type: "unit_updated",
      payload: { unit_id: "AK-302", status: "in_progress" },
      timestamp_ms: 500,
    });

    const first = sub.deliver(g6);
    expect(first.class).toBe("buffered");
    expect(client.state).toBe("buffering");
    expect(client.degradedReasons).toContain("awaiting_increment:gauntlet:expected=5");
    expect(client.bufferedCount()).toBe(1);

    expect(sub.deliver(g7).class).toBe("buffered");
    expect(sub.deliver(g8).class).toBe("buffered");
    expect(client.bufferedCount()).toBe(3);

    // No silent divergence while the gap is open: the cursor has NOT advanced
    // past the missing sequence, and the projection still shows pre-gap state.
    expect(client.sourcePositions().find((p) => p.epoch === "epoch-1")?.sequence).toBe(4);
    expect(model.getSnapshot().units.find((u) => u.unit_id === "AK-301")?.status).toBe(
      "in_progress",
    );

    // The missing frame fills the gap: 5 applies, then 6, 7, 8 drain in
    // SEQUENCE order — even though 6 carries the largest wall-clock timestamp.
    const result = sub.deliver(g5);
    expect(result.class).toBe("applied");
    expect(client.state).toBe("live");
    expect(client.degradedReasons).toEqual([]);
    expect(client.bufferedCount()).toBe(0);
    expect(client.sourcePositions().find((p) => p.epoch === "epoch-1")?.sequence).toBe(8);

    // Canonical order was sequence order: the trace shows 5, 6, 7, 8 — and the
    // FINAL row state is the highest sequence's payload, not the newest ts.
    const traceIds = model.getTrace().map((record) => record.trace_id);
    expect(traceIds.slice(-4)).toEqual([
      `trace-gauntlet-${g5.cursor}`,
      `trace-gauntlet-${g6.cursor}`,
      `trace-gauntlet-${g7.cursor}`,
      `trace-gauntlet-${g8.cursor}`,
    ]);
    expect(model.getSnapshot().units.find((u) => u.unit_id === "AK-301")?.status).toBe("resolved");

    // Converged with the authoritative fold.
    const reference = referenceModel(port, "gen-ref-gap");
    expect(projectionDifferences(model.getSnapshot(), reference.getSnapshot())).toEqual([]);

    model.close();
    reference.close();
  });

  it("bounded backlog overflow under overload: explicit resync (never an unbounded buffer, never a silent advance)", () => {
    const gauntlet = new InMemoryStreamSource("gauntlet");
    const prime = new InMemoryStreamSource("prime");
    seed(gauntlet, prime);
    const port = new InMemoryStreamPort([gauntlet, prime]);
    const { client, model } = makeClient(port, "gen-ovf", 2);
    client.connect();
    const sub = client.subscribeStream();

    // The bounded backlog (capacity 2) fills while the gap head (seq 4) is
    // withheld; a THIRD out-of-order frame overflows it.
    const gapHead = gauntlet.append({
      event_type: "unit_updated",
      payload: { unit_id: "AK-301", status: "verifying" },
      timestamp_ms: 2000,
    });
    const b1 = gauntlet.append({
      event_type: "unit_updated",
      payload: { unit_id: "AK-301", status: "resolved" },
      timestamp_ms: 2100,
    });
    const b2 = gauntlet.append({
      event_type: "unit_updated",
      payload: { unit_id: "AK-302", status: "in_progress" },
      timestamp_ms: 2200,
    });
    const b3 = gauntlet.append({
      event_type: "unit_updated",
      payload: { unit_id: "AK-302", status: "verifying" },
      timestamp_ms: 2300,
    });
    expect(gapHead.sequence).toBe(4);
    expect(sub.deliver(b1).class).toBe("buffered");
    expect(sub.deliver(b2).class).toBe("buffered");
    expect(client.bufferedCount()).toBe(2);
    expect(client.state).toBe("buffering");

    const overflow = sub.deliver(b3);
    expect(overflow.class).toBe("resync_triggered");
    expect(client.resyncCount).toBe(1);
    expect(client.generationId).toBe("gen-ovf-2");
    expect(client.state).toBe("live");
    expect(client.bufferedCount()).toBe(0);

    // The resync's fresh baseline caught everything up: converged, nothing lost.
    const reference = referenceModel(port, "gen-ref-ovf");
    expect(projectionDifferences(model.getSnapshot(), reference.getSnapshot())).toEqual([]);
    expect(client.lastHandshake?.kind).toBe("resync");
    expect(client.lastHandshake?.resync_reason).toContain("buffer_overflow");

    model.close();
    reference.close();
  });

  it("retention expiry makes an awaited gap unfillable: resync with a fresh baseline — the expired event never enters the projection", () => {
    const gauntlet = new InMemoryStreamSource("gauntlet");
    const prime = new InMemoryStreamSource("prime");
    seed(gauntlet, prime);
    const port = new InMemoryStreamPort([gauntlet, prime]);
    const { client, model } = makeClient(port, "gen-ret");
    client.connect();
    const sub = client.subscribeStream();

    // Gap at 4: sequence 4 is withheld while 5 arrives and is buffered.
    const withheld = gauntlet.append({
      event_type: "unit_updated",
      payload: { unit_id: "AK-301", status: "verifying" },
      timestamp_ms: 2000,
    });
    const buffered5 = gauntlet.append({
      event_type: "unit_updated",
      payload: { unit_id: "AK-301", status: "resolved" },
      timestamp_ms: 2100,
    });
    expect(sub.deliver(buffered5).class).toBe("buffered");

    // Retention expires sequence 4 (the new floor is 5): the gap can NEVER
    // be filled by delivery now.
    gauntlet.expireThrough(4);
    expect(gauntlet.earliest_sequence).toBe(5);
    expect(gauntlet.retainedEvents().map((event) => event.sequence)).toEqual([5]);

    // poll() detects the unfillable gap and explicitly resyncs.
    const report = client.poll();
    expect(report.resync_triggered).toContain("retention_expired:gauntlet");
    expect(client.resyncCount).toBe(1);
    expect(client.generationId).toBe("gen-ret-2");
    expect(client.state).toBe("live");

    // The expired event (sequence 4) is NOT in the projection — the fresh
    // baseline laid only retained authoritative state (seq 5 applied).
    expect(model.getSnapshot().units.find((u) => u.unit_id === "AK-301")?.status).toBe("resolved");
    const appliedTraces = model.getTrace().map((record) => record.trace_id);
    expect(appliedTraces).not.toContain(`trace-gauntlet-${withheld.cursor}`);
    expect(appliedTraces).toContain(`trace-gauntlet-${buffered5.cursor}`);
    expect(Object.values(model.getSnapshot().source_cursors)).not.toContain(mintOpaqueCursor(4));

    // Converged with the retained authoritative fold.
    const reference = referenceModel(port, "gen-ref-ret");
    expect(projectionDifferences(model.getSnapshot(), reference.getSnapshot())).toEqual([]);

    model.close();
    reference.close();
  });

  it("two concurrent clients: isolated generations and projections; stale frames cannot cross-contaminate; both converge to the same authoritative fold", () => {
    const gauntlet = new InMemoryStreamSource("gauntlet");
    const prime = new InMemoryStreamSource("prime");
    seed(gauntlet, prime);
    const port = new InMemoryStreamPort([gauntlet, prime]);
    const clientA = makeClient(port, "gen-a");
    const clientB = makeClient(port, "gen-b");

    clientA.client.connect();
    clientB.client.connect();
    expect(clientA.client.generationId).toBe("gen-a-1");
    expect(clientB.client.generationId).toBe("gen-b-1");

    const subA = clientA.client.subscribeStream();
    const subB = clientB.client.subscribeStream();
    expect(subA.generation_id).toBe("gen-a-1");
    expect(subB.generation_id).toBe("gen-b-1");

    // One live increment reaches A; B's projection is unaffected.
    const g4 = gauntlet.append({
      event_type: "unit_updated",
      payload: { unit_id: "AK-301", status: "in_progress" },
      timestamp_ms: 2000,
    });
    expect(subA.deliver(g4).class).toBe("applied");
    expect(clientA.model.getSnapshot().units.find((u) => u.unit_id === "AK-301")?.status).toBe(
      "in_progress",
    );
    expect(clientB.model.getSnapshot().units.find((u) => u.unit_id === "AK-301")?.status).toBe(
      "blocked",
    );
    expect(clientB.client.rejectionLog.length).toBe(0);

    // A reconnects; its old-generation frames are rejected AT A and leave B
    // completely untouched.
    clientA.client.reconnect();
    expect(clientA.client.generationId).toBe("gen-a-2");
    const stale = subA.deliver(g4);
    expect(stale.class).toBe("rejected");
    if (stale.class === "rejected") {
      expect(stale.rejection.live_generation).toBe("gen-a-2");
    }
    expect(clientB.client.generationId).toBe("gen-b-1");
    expect(clientB.client.rejectionLog.length).toBe(0);
    // B still holds exactly its own baseline fold (4 events), untouched by A.
    expect(clientB.model.getTrace().length).toBe(4);
    expect(clientA.model.getSnapshot().units.find((u) => u.unit_id === "AK-301")?.status).toBe(
      "in_progress",
    );

    // Cross-client injection: A's CURRENT-generation frame is stale at B.
    const g5 = gauntlet.append({
      event_type: "unit_updated",
      payload: { unit_id: "AK-302", status: "in_progress" },
      timestamp_ms: 2100,
    });
    const injected = clientB.client.ingest({ generation_id: "gen-a-2", event: g5 });
    expect(injected.class).toBe("rejected");
    if (injected.class === "rejected") {
      expect(injected.rejection.reason).toBe("superseded_generation");
      expect(injected.rejection.live_generation).toBe("gen-b-1");
    }
    // B receives its OWN copies through its own subscription instead — in
    // order, so both apply.
    expect(subB.deliver(g4).class).toBe("applied");
    expect(subB.deliver(g5).class).toBe("applied");

    // Both clients fully reconnect: they converge to the SAME authoritative fold.
    clientA.client.reconnect();
    clientB.client.reconnect();
    const reference = referenceModel(port, "gen-ref-two");
    expect(projectionDifferences(clientA.model.getSnapshot(), reference.getSnapshot())).toEqual([]);
    expect(projectionDifferences(clientB.model.getSnapshot(), reference.getSnapshot())).toEqual([]);
    expect(projectionDifferences(clientA.model.getSnapshot(), clientB.model.getSnapshot())).toEqual(
      [],
    );

    clientA.model.close();
    clientB.model.close();
    reference.close();
  });
});
