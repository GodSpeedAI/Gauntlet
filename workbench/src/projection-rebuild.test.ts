import { afterEach, describe, expect, it } from "bun:test";
import { existsSync, unlinkSync } from "node:fs";
import { resolve } from "node:path";
import { type AuthoritativeEventFrame, SqliteProjectionStore } from "./projection-store";

describe("T06 Cold Rebuild & Noncanonical SQLite Projections (REQ-DATA-002, Attack 1)", () => {
  const testDbPath = resolve(import.meta.dir, "../test-rebuild-projection.db");

  afterEach(() => {
    for (const suffix of ["", "-wal", "-shm"]) {
      const p = `${testDbPath}${suffix}`;
      if (existsSync(p)) {
        try {
          unlinkSync(p);
        } catch {
          // ignore
        }
      }
    }
  });

  it("Attack 1: complete deletion of local SQLite database allows bit-equivalent cold rebuild from authoritative event log", () => {
    const authoritativeEvents: AuthoritativeEventFrame[] = [
      {
        generation_id: "gen-alpha",
        source_id: "gauntlet",
        cursor: "g-001",
        event_type: "unit_updated",
        payload: {
          unit_id: "unit-101",
          title: "Unit 101 Title",
          phase: "execute",
          status: "in_progress",
          authority: "admin",
          blast_radius: "project",
        },
        timestamp_ms: 1000,
      },
      {
        generation_id: "gen-alpha",
        source_id: "gauntlet",
        cursor: "g-002",
        event_type: "evidence_added",
        payload: {
          evidence_id: "ev-001",
          unit_id: "unit-101",
          kind: "test_report",
          locator: ".agents/reports/ev1.json",
          verified: true,
        },
        timestamp_ms: 1050,
      },
      {
        generation_id: "gen-alpha",
        source_id: "prime",
        cursor: "p-001",
        event_type: "session_updated",
        payload: {
          session_id: "prime-ses-01",
          parent_id: null,
          model: "claude-3-5-sonnet",
          status: "active",
        },
        timestamp_ms: 1100,
      },
      {
        generation_id: "gen-alpha",
        source_id: "gauntlet",
        cursor: "g-003",
        event_type: "focus_changed",
        payload: {
          kind: "unit",
          id: "unit-101",
        },
        timestamp_ms: 1150,
      },
    ];

    // 1. Establish initial SQLite store and populate with authoritative log
    const store1 = new SqliteProjectionStore(testDbPath);
    store1.startGeneration("gen-alpha");
    for (const ev of authoritativeEvents) {
      const res = store1.applyEvent(ev);
      expect(res.applied).toBe(true);
    }
    store1.markGenerationReady("gen-alpha");

    const snap1 = store1.getSnapshot("horizon-main");
    expect(snap1.units.length).toBe(1);
    expect(snap1.evidence.length).toBe(1);
    expect(snap1.sessions.length).toBe(1);
    expect(snap1.focus?.id).toBe("unit-101");
    expect(snap1.source_cursors["gauntlet"]).toBe("g-003");
    expect(snap1.source_cursors["prime"]).toBe("p-001");

    store1.close();

    // 2. ATTACK 1: Completely delete the database file from disk
    expect(existsSync(testDbPath)).toBe(true);
    unlinkSync(testDbPath);
    for (const suffix of ["-wal", "-shm"]) {
      const p = `${testDbPath}${suffix}`;
      if (existsSync(p)) unlinkSync(p);
    }
    expect(existsSync(testDbPath)).toBe(false);

    // 3. DEFENSE: Cold rebuild entirely from authoritative events
    const store2 = SqliteProjectionStore.rebuildFromAuthoritativeLog(
      testDbPath,
      "gen-alpha",
      authoritativeEvents,
    );
    const snap2 = store2.getSnapshot("horizon-main");

    // 4. Verify reconstructed state is semantically identical without cache leakage
    expect(snap2.generation).toBe(snap1.generation);
    expect(snap2.horizon).toBe(snap1.horizon);
    expect(snap2.focus).toEqual(snap1.focus);
    expect(snap2.source_cursors).toEqual(snap1.source_cursors);
    expect(snap2.units).toEqual(snap1.units);
    expect(snap2.evidence).toEqual(snap1.evidence);
    expect(snap2.sessions).toEqual(snap1.sessions);

    store2.close();
  });
});
