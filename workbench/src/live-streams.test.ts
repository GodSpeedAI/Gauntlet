import { describe, expect, it } from "bun:test";
import { CanonicalSurfaceStreamPort } from "./canonical-surface-stream-port";
import { GauntletReadClient } from "./client";
import { startCanonicalFixture } from "./testing/canonical-fixture-server";

/**
 * Tests for `CanonicalSurfaceStreamPort` (plan Phase B,
 * workbench-intent-closure.md §4; REQ-SYNC-001, D1).
 *
 * These exercise the port ALONE, against a real HTTP fixture server, with no
 * `LiveStreamClient`/`WorkbenchModel` involved — `workbench-host.test.ts` and
 * `production-entry.test.ts` cover the host-swap composition; the
 * `cold-rebuild`/`detach-reattach`/`reconnect-races`/`stale-frame` suites
 * cover this port driving `LiveStreamClient`'s reconciliation directly.
 */

describe("CanonicalSurfaceStreamPort: D1 — every declaration is observed, never minted", () => {
  it("declares snapshot_interval granularity on every descriptor and window, never claiming durable per-event delivery", async () => {
    const fixture = startCanonicalFixture({
      run: "run-1",
      units: [{ id: "AK-1", status: "open" }],
    });
    try {
      const port = new CanonicalSurfaceStreamPort(new GauntletReadClient(fixture.endpoint));
      await port.observe();
      for (const descriptor of port.describeSources()) {
        expect(descriptor.granularity).toBe("snapshot_interval");
      }
      const baseline = port.baselineWindow("gauntlet");
      expect(baseline.descriptor.granularity).toBe("snapshot_interval");
      const increments = port.incrementsSince("gauntlet", 0);
      expect(increments.descriptor.granularity).toBe("snapshot_interval");
    } finally {
      await fixture.stop();
    }
  });

  it("reports a retention floor and watermark it can actually honour: baselineWindow never includes an event below earliest_sequence", async () => {
    const fixture = startCanonicalFixture({
      run: "run-1",
      units: [{ id: "AK-1", status: "open" }],
    });
    try {
      const port = new CanonicalSurfaceStreamPort(new GauntletReadClient(fixture.endpoint));
      await port.observe();
      const unit = fixture.state.units[0];
      if (unit) unit.status = "fail";
      await port.observe();
      const descriptor = port.describeSources()[0];
      expect(descriptor).toBeDefined();
      const baseline = port.baselineWindow("gauntlet");
      for (const event of baseline.events) {
        expect(event.sequence).toBeGreaterThanOrEqual(descriptor?.earliest_sequence ?? 0);
        expect(event.sequence).toBeLessThanOrEqual(descriptor?.watermark_sequence ?? 0);
      }
    } finally {
      await fixture.stop();
    }
  });

  it("a run change replaces the epoch and resets the watermark to a fresh baseline (never a silent carry-forward of the old run's sequence)", async () => {
    const fixture = startCanonicalFixture({
      run: "run-1",
      units: [{ id: "AK-1", status: "open" }],
    });
    try {
      const port = new CanonicalSurfaceStreamPort(new GauntletReadClient(fixture.endpoint));
      await port.observe();
      const epochBefore = port.describeSources()[0]?.store_epoch;
      const watermarkBefore = port.describeSources()[0]?.watermark_sequence ?? 0;
      expect(watermarkBefore).toBeGreaterThan(0);

      fixture.state.run = "run-2";
      fixture.state.units = [{ id: "AK-2", status: "open" }];
      await port.observe();

      const after = port.describeSources()[0];
      expect(after?.store_epoch).not.toBe(epochBefore);
      // Fresh epoch means fresh storage: the watermark restarts from the new
      // observation alone, never inheriting run-1's sequence numbers.
      expect(after?.earliest_sequence).toBe(1);
      const baseline = port.baselineWindow("gauntlet");
      expect(baseline.events.every((event) => event.payload["unit_id"] !== "AK-1")).toBe(true);
      expect(baseline.events.some((event) => event.payload["unit_id"] === "AK-2")).toBe(true);
    } finally {
      await fixture.stop();
    }
  });

  it("a membership change within the SAME run also replaces the epoch (the store's observable shape changed, so a stale cursor must not validate)", async () => {
    const fixture = startCanonicalFixture({
      run: "run-1",
      units: [{ id: "AK-1", status: "open" }],
    });
    try {
      const port = new CanonicalSurfaceStreamPort(new GauntletReadClient(fixture.endpoint));
      await port.observe();
      const epochBefore = port.describeSources()[0]?.store_epoch;

      fixture.state.units = [
        { id: "AK-1", status: "open" },
        { id: "AK-2", status: "open" },
      ];
      await port.observe();

      expect(port.describeSources()[0]?.store_epoch).not.toBe(epochBefore);
    } finally {
      await fixture.stop();
    }
  });

  it("a no-op observe (nothing changed) reports changed: false and mints no new event", async () => {
    const fixture = startCanonicalFixture({
      run: "run-1",
      units: [{ id: "AK-1", status: "open" }],
    });
    try {
      const port = new CanonicalSurfaceStreamPort(new GauntletReadClient(fixture.endpoint));
      const first = await port.observe();
      expect(first.changed).toBe(true);
      const watermarkAfterFirst = port.describeSources()[0]?.watermark_sequence;

      const second = await port.observe();
      expect(second.changed).toBe(false);
      expect(port.describeSources()[0]?.watermark_sequence).toBe(watermarkAfterFirst);
    } finally {
      await fixture.stop();
    }
  });

  it("an unreachable surface is reported honestly (reachable: false) rather than silently reusing the last observation as if fresh", async () => {
    const fixture = startCanonicalFixture({
      run: "run-1",
      units: [{ id: "AK-1", status: "open" }],
    });
    try {
      const port = new CanonicalSurfaceStreamPort(new GauntletReadClient(fixture.endpoint));
      await port.observe();
      fixture.state.reachable = false;
      const report = await port.observe();
      expect(report.reachable).toBe(false);
      expect(port.lastReachable).toBe(false);
    } finally {
      await fixture.stop();
    }
  });
});
