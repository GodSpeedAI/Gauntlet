/**
 * T16 detach/reattach suites (plan Attack 2; REQ-PRIME-009/010, REQ-VERIFY-005/006,
 * REQ-UX-004, REQ-SAFE-005, REQ-OUT-003, REQ-DATA-002; prereg P2/D3).
 *
 * The gate selector `just tui-test detach-reattach` selects this file by name.
 * Proofs: closing/killing the TUI host during a running Prime child terminates
 * nothing — the child keeps making Prime-side progress and completes only
 * through Prime's own schema-valid result contract; the detach receipt's
 * `cancel_calls_issued` is the literal 0 and the adapter call log across the
 * whole detach→reattach cycle records READS ONLY (no cancel/stop/terminate/
 * abort/interrupt/kill); a fresh host reattaches with the REAL topology from
 * qualified Prime records (exact session ids and parent links, never liveness
 * inference — a "DONE" mailbox message moves nothing) and reconciles every
 * stream event since the last authoritative cursor; when reconciliation finds
 * inconsistency (epoch change, retention expiry, store refusal) the client
 * enters a VISIBLE degraded/resync state and the authoritative sources are
 * never mutated to make the projection agree.
 *
 * Determinism: in-process fakes stand in for real processes/network (prereg
 * blindspots: real targets are T20; cross-process reality is T24).
 */

import { describe, expect, it } from "bun:test";
import { CanonicalSurfaceStreamPort } from "./canonical-surface-stream-port";
import { GauntletReadClient } from "./client";
import {
  canonicalStreamOrder,
  InMemoryStreamPort,
  InMemoryStreamSource,
  LiveStreamClient,
  projectionDifferences,
} from "./live-streams";
import { PrimeHostAdapter } from "./prime-host";
import { loggingPrimeAdapter } from "./prime-long-running";
import { findSessionInTopology } from "./prime-topology";
import { startCanonicalFixture } from "./testing/canonical-fixture-server";
import { WorkbenchModel } from "./workbench-model";

function seed(gauntlet: InMemoryStreamSource, prime: InMemoryStreamSource): void {
  gauntlet.append({
    event_type: "unit_updated",
    payload: { unit_id: "AK-401", title: "Unit 401", status: "blocked" },
    timestamp_ms: 1000,
  });
  gauntlet.append({
    event_type: "unit_updated",
    payload: { unit_id: "AK-402", title: "Unit 402", status: "pending" },
    timestamp_ms: 1100,
  });
  prime.append({
    event_type: "session_updated",
    payload: { session_id: "prime-root", parent_id: null, status: "running" },
    timestamp_ms: 1200,
  });
}

function makeClient(
  port: InMemoryStreamPort,
  prefix: string,
): { client: LiveStreamClient; model: WorkbenchModel } {
  const model = new WorkbenchModel();
  const client = new LiveStreamClient({
    model,
    port,
    mintGenerationId: (serial) => `${prefix}-${serial}`,
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

function primeWorld(): PrimeHostAdapter {
  const adapter = new PrimeHostAdapter();
  adapter.spawnChild({
    sessionId: "prime-root",
    parentSessionId: null,
    contractPath: ".agents/reports/gauntlet-tui-prime-workbench/evidence/T16/contracts/root.md",
  });
  adapter.spawnChild({
    sessionId: "prime-child-1",
    parentSessionId: "prime-root",
    contractPath: ".agents/reports/gauntlet-tui-prime-workbench/evidence/T16/contracts/child1.md",
  });
  return adapter;
}

describe("T16 detach/reattach (plan Attack 2; REQ-PRIME-009/010, REQ-VERIFY-005/006)", () => {
  it("Attack 2: kill the host during a running child — the child keeps living in Prime and completes through its own contract; the fresh host reattaches with REAL topology from records, no cancel on the adapter call log, and events reconciled since the last cursor", async () => {
    const gauntlet = new InMemoryStreamSource("gauntlet");
    const prime = new InMemoryStreamSource("prime");
    seed(gauntlet, prime);
    const port = new InMemoryStreamPort([gauntlet, prime]);
    const adapter = primeWorld();
    const { client, model } = makeClient(port, "gen-dr");
    const handshake = client.connect();
    expect(handshake.generation_id).toBe("gen-dr-1");

    // Detach: ONLY noncanonical cursor/view preferences are persisted, and no
    // cancel is issued (the receipt's cancel_calls_issued is the literal 0).
    const detach = client.detach({ selected_horizon: "hz-401", last_focus: "AK-401" });
    expect(detach.detached).toBe(true);
    expect(detach.cancel_calls_issued).toBe(0);
    expect(detach.persistence_class).toBe("noncanonical_presentation_state");
    expect(detach.persisted.detached_from_generation).toBe("gen-dr-1");
    expect(detach.persisted.view_preferences["selected_horizon"]).toBe("hz-401");
    expect(Object.keys(detach.persisted.source_positions).sort()).toEqual(["gauntlet", "prime"]);

    // KILL the host: the client (and its TUI) is gone.
    client.close();
    expect(client.state).toBe("closed");

    // Authorized Prime work CONTINUES without the TUI — including mailbox
    // progress that CLAIMS completion (which is not completion).
    adapter.receiveMailboxMessage("prime-child-1", {
      sender: "prime-child-1",
      content: "stream finished, all goals met, DONE",
      kind: "signal",
    });
    const midFlight = adapter.getSession("prime-child-1");
    expect(midFlight?.handle.status).toBe("running");
    expect(midFlight?.resultContract).toBeNull();

    // New authoritative stream events land while the TUI is gone.
    const g3 = gauntlet.append({
      event_type: "unit_updated",
      payload: { unit_id: "AK-401", status: "in_progress" },
      timestamp_ms: 5000,
    });
    const p2 = prime.append({
      event_type: "session_updated",
      payload: { session_id: "prime-child-1", parent_id: "prime-root", status: "running" },
      timestamp_ms: 5100,
    });
    void g3;
    void p2;

    // FRESH host reattaches: Prime topology from qualified records, stream
    // reconciliation since the last authoritative cursor — under a call-
    // recording adapter wrapper so the absence of cancel is PROVEN.
    const fresh = makeClient(port, "gen-dr2");
    const logged = loggingPrimeAdapter(adapter);
    const reattach = fresh.client.reattach({
      persisted: detach.persisted,
      prime_adapter: logged.logged,
    });

    // No cancel was sent, ever: the receipt is typed 0 and the adapter log
    // holds READS ONLY.
    expect(reattach.cancel_calls_issued).toBe(0);
    expect(logged.calls.length).toBeGreaterThanOrEqual(1);
    for (const call of logged.calls) {
      expect(call).not.toMatch(/cancel|stop|terminate|abort|interrupt|kill/i);
    }
    expect(logged.calls.every((call) => call === "listSessions")).toBe(true);

    // REAL topology from qualified records: exact ids and parent links.
    const primeReceipt = reattach.prime;
    if (primeReceipt === null) {
      throw new Error("reattach did not project the Prime adapter");
    }
    expect(primeReceipt.source).toBe("qualified_prime_adapter_records");
    const child = findSessionInTopology(primeReceipt.topology, "prime-child-1");
    expect(child?.parentId).toBe("prime-root");
    expect(child?.contractPath).toContain("child1.md");
    // Completion is the RECORD's, never inferred from liveness or messages:
    // the child said "DONE" and is alive, yet it is running — not completed.
    expect(primeReceipt.running_sessions).toContain("prime-child-1");
    expect(primeReceipt.completed_sessions).not.toContain("prime-child-1");

    // Stream reconciliation: the persisted cursors were valid, and the fresh
    // generation reconciled every event since them.
    expect(reattach.handshake.kind).toBe("reconnect");
    expect(reattach.handshake.generation_id).toBe("gen-dr2-1");
    expect(reattach.presentation_source).toBe("persisted_noncanonical_cursor_state");
    const gauntletLeg = reattach.handshake.per_source.find((leg) => leg.source_id === "gauntlet");
    expect(gauntletLeg?.cursor_validation).toBe("valid");
    expect(gauntletLeg?.strategy).toBe("baseline_with_cursor_validation");
    expect(gauntletLeg?.reconciled_since_cursor).toBe(1);
    const reference = referenceModel(port, "gen-ref-attack2");
    expect(projectionDifferences(fresh.model.getSnapshot(), reference.getSnapshot())).toEqual([]);

    // Prime completes the child through ITS OWN schema-valid contract — no
    // TUI involved. A second fresh host sees the record's completion.
    adapter.deliverResultContract("prime-child-1", {
      session_id: "prime-child-1",
      schema_valid: true,
      outcome: "pass",
      evidence_path:
        ".agents/reports/gauntlet-tui-prime-workbench/evidence/T16/contracts/child1.md",
      completed_at: "2026-09-09T12:00:00Z",
    });
    const secondDetach = fresh.client.detach();
    const secondLogged = loggingPrimeAdapter(adapter);
    const second = makeClient(port, "gen-dr3");
    const reattach2 = second.client.reattach({
      persisted: secondDetach.persisted,
      prime_adapter: secondLogged.logged,
    });
    expect(reattach2.prime?.completed_sessions).toContain("prime-child-1");
    expect(reattach2.prime?.running_sessions).not.toContain("prime-child-1");
    expect(reattach2.cancel_calls_issued).toBe(0);

    model.close();
    fresh.model.close();
    second.model.close();
    reference.close();
  });

  it("detach mutates nothing: sources and Prime sessions are byte-identical across detach and host close; the persisted state is presentation-only", () => {
    const gauntlet = new InMemoryStreamSource("gauntlet");
    const prime = new InMemoryStreamSource("prime");
    seed(gauntlet, prime);
    const port = new InMemoryStreamPort([gauntlet, prime]);
    const adapter = primeWorld();
    const { client, model } = makeClient(port, "gen-dm");
    client.connect();

    const sourceBefore = JSON.stringify({
      gauntlet: gauntlet.retainedEvents(),
      prime: prime.retainedEvents(),
    });
    const sessionsBefore = JSON.stringify(adapter.listSessions().map((session) => session.handle));

    const detach = client.detach({ pane: "work" });
    client.close();

    // Detach + close touched nothing canonical.
    expect(
      JSON.stringify({
        gauntlet: gauntlet.retainedEvents(),
        prime: prime.retainedEvents(),
      }),
    ).toBe(sourceBefore);
    expect(JSON.stringify(adapter.listSessions().map((session) => session.handle))).toBe(
      sessionsBefore,
    );
    expect(gauntlet.watermark_sequence).toBe(2);
    expect(prime.watermark_sequence).toBe(1);

    // The persisted document is presentation-only: cursors + view preferences.
    expect(Object.keys(detach.persisted).sort()).toEqual([
      "detached_from_generation",
      "source_positions",
      "view_preferences",
    ]);
    for (const position of Object.values(detach.persisted.source_positions)) {
      expect(Object.keys(position).sort()).toEqual(["cursor", "epoch", "sequence"]);
    }

    model.close();
  });

  it("reattach reconciliation finds inconsistency: retention-expired and epoch-changed cursors are reported and re-baselined; a mid-stream store refusal enters a visible resync while sources are never mutated (REQ-SAFE-005)", () => {
    const gauntlet = new InMemoryStreamSource("gauntlet");
    const prime = new InMemoryStreamSource("prime");
    seed(gauntlet, prime);
    const port = new InMemoryStreamPort([gauntlet, prime]);

    // (a) Retention expiry while detached: new events land (3, 4), then the
    // source's retention floor moves past the persisted cursor's reach — the
    // gap 3..(floor-1) can no longer be served. Reattach reports it and lays
    // a fresh baseline.
    const host1 = makeClient(port, "gen-ra");
    host1.client.connect();
    const detachA = host1.client.detach();
    gauntlet.append({
      event_type: "unit_updated",
      payload: { unit_id: "AK-401", status: "in_progress" },
      timestamp_ms: 6000,
    });
    const expiredTail = gauntlet.append({
      event_type: "unit_updated",
      payload: { unit_id: "AK-401", status: "verifying" },
      timestamp_ms: 6100,
    });
    gauntlet.expireThrough(3);
    expect(expiredTail.sequence).toBe(4);
    const reattachA = host1.client.reconnect(detachA.persisted);
    const legA = reattachA.per_source.find((leg) => leg.source_id === "gauntlet");
    expect(legA?.cursor_validation).toBe("retention_expired");
    expect(legA?.strategy).toBe("fresh_baseline");
    expect(legA?.skipped_expired_sequences).toBe(3);
    expect(legA?.reconciled_since_cursor).toBe(1);
    const referenceA = referenceModel(port, "gen-ref-ra");
    expect(projectionDifferences(host1.model.getSnapshot(), referenceA.getSnapshot())).toEqual([]);

    // (b) Epoch change while detached: reported, fresh baseline, new truth only.
    gauntlet.replaceStorage("epoch-2");
    gauntlet.append({
      event_type: "unit_updated",
      payload: { unit_id: "AK-409", title: "Unit 409", status: "pending" },
      timestamp_ms: 7000,
    });
    const reattachB = host1.client.reconnect(detachA.persisted);
    const legB = reattachB.per_source.find((leg) => leg.source_id === "gauntlet");
    expect(legB?.cursor_validation).toBe("epoch_changed");
    expect(legB?.strategy).toBe("fresh_baseline");
    const referenceB = referenceModel(port, "gen-ref-rb");
    expect(projectionDifferences(host1.model.getSnapshot(), referenceB.getSnapshot())).toEqual([]);
    const unitIds = host1.model.getSnapshot().units.map((unit) => unit.unit_id);
    expect(unitIds).toEqual(["AK-409"]);

    // (c) Mid-stream store refusal AFTER a good reattach: an event that passes
    // stream identity but fails store admission (an unattributable
    // focus_changed) triggers a VISIBLE resync — and the source is never
    // mutated to make the projection agree.
    const sub = host1.client.subscribeStream();
    const resyncObserved: string[] = [];
    host1.client.onStateChange((transition) => {
      if (transition.to === "resyncing") {
        resyncObserved.push(transition.reason);
        expect(host1.client.degradedReasons.length).toBeGreaterThan(0);
      }
    });
    const beforeBogus = host1.model.getSnapshot();
    const bogus = gauntlet.append({
      event_type: "focus_changed",
      payload: { kind: "unit", id: "no-such-unit" },
      timestamp_ms: 7500,
    });
    const result = sub.deliver(bogus);
    expect(result.class).toBe("resync_triggered");
    expect(resyncObserved.length).toBe(1);
    expect(resyncObserved[0]).toContain("store_refusal");
    expect(host1.client.resyncCount).toBe(1);
    expect(host1.client.state).toBe("live");
    const sourceAfter = JSON.stringify(gauntlet.retainedEvents());
    // epoch-2 storage holds exactly the AK-409 baseline event and the bogus
    // event (seq 2) — the resync consumed neither nor added anything.
    expect(gauntlet.watermark_sequence).toBe(2);
    // The resync re-baselined from the source; the source never changed and
    // the projection is byte-coherent with its pre-inconsistency state (the
    // unadmittable event projects nothing, live or at baseline).
    expect(projectionDifferences(host1.model.getSnapshot(), beforeBogus)).toEqual([]);
    expect(sourceAfter).toBe(JSON.stringify(gauntlet.retainedEvents()));

    host1.model.close();
    referenceA.close();
    referenceB.close();
  });
});

describe("Phase B — detach/reattach against the DERIVED port (workbench-intent-closure.md tooth 4)", () => {
  it("a cursor persisted at detach is honoured on reattach through CanonicalSurfaceStreamPort, not just InMemoryStreamPort — a real content change since detach (same membership, so no epoch change) is reconciled in as an incremental catch-up, not a fresh baseline", async () => {
    const fixture = startCanonicalFixture({
      run: "run-b4",
      units: [{ id: "AK-410", status: "open" }],
    });
    try {
      const port = new CanonicalSurfaceStreamPort(new GauntletReadClient(fixture.endpoint));
      await port.observe();
      const model = new WorkbenchModel();
      const client = new LiveStreamClient({ model, port, mintGenerationId: (n) => `b4-${n}` });
      client.connect();
      expect(model.getUnits().some((unit) => unit.unit_id === "AK-410")).toBe(true);

      const receipt = client.detach();
      expect(receipt.cancel_calls_issued).toBe(0);
      expect(receipt.persisted.source_positions["gauntlet"]?.sequence).toBeGreaterThan(0);
      client.close();

      // Real change to the surface WHILE detached — same membership (AK-410
      // still the only unit), so this is an increment, not a new epoch.
      const unit = fixture.state.units[0];
      if (unit) unit.status = "fail";
      await port.observe();

      const model2 = new WorkbenchModel();
      const client2 = new LiveStreamClient({
        model: model2,
        port,
        mintGenerationId: (n) => `b4-reattach-${n}`,
      });
      const reattachReceipt = client2.reattach({ persisted: receipt.persisted });
      expect(reattachReceipt.presentation_source).toBe("persisted_noncanonical_cursor_state");
      const reconciliation = reattachReceipt.handshake.per_source.find(
        (entry) => entry.source_id === "gauntlet",
      );
      expect(reconciliation?.cursor_validation).toBe("valid");
      expect(reconciliation?.strategy).toBe("baseline_with_cursor_validation");
      expect(reconciliation?.reconciled_since_cursor).toBeGreaterThan(0);
      expect(model2.getUnits().find((u) => u.unit_id === "AK-410")?.status).toBe("fail");

      model.close();
      model2.close();
    } finally {
      await fixture.stop();
    }
  });

  it("a membership change while detached forces epoch_changed on reattach — the derived port never lets a stale cursor validate against a store whose shape moved", async () => {
    const fixture = startCanonicalFixture({
      run: "run-b4b",
      units: [{ id: "AK-420", status: "open" }],
    });
    try {
      const port = new CanonicalSurfaceStreamPort(new GauntletReadClient(fixture.endpoint));
      await port.observe();
      const model = new WorkbenchModel();
      const client = new LiveStreamClient({ model, port, mintGenerationId: (n) => `b4b-${n}` });
      client.connect();
      const receipt = client.detach();
      client.close();

      // Membership changes while detached — same run, different unit set.
      fixture.state.units = [{ id: "AK-421", status: "open" }];
      await port.observe();

      const model2 = new WorkbenchModel();
      const client2 = new LiveStreamClient({
        model: model2,
        port,
        mintGenerationId: (n) => `b4b-reattach-${n}`,
      });
      const reattachReceipt = client2.reattach({ persisted: receipt.persisted });
      const reconciliation = reattachReceipt.handshake.per_source.find(
        (entry) => entry.source_id === "gauntlet",
      );
      expect(reconciliation?.cursor_validation).toBe("epoch_changed");
      expect(reconciliation?.strategy).toBe("fresh_baseline");
      expect(model2.getUnits().some((unit) => unit.unit_id === "AK-420")).toBe(false);
      expect(model2.getUnits().some((unit) => unit.unit_id === "AK-421")).toBe(true);

      model.close();
      model2.close();
    } finally {
      await fixture.stop();
    }
  });
});
